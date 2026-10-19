//! One frame's instances: the quad every draw is made of, the texture a range
//! of them is drawn with, and the frame that joins the parts back to front.

use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Instance {
    pub rect: [f32; 4],
    pub uv: [f32; 4],
    pub color: [f32; 4],
    pub kind: u32,
    /// Rotation, in radians, about `pivot` (window pixels); transitions use it.
    pub pivot: [f32; 2],
    pub angle: f32,
    /// A tilted stage's floor (`Tilt`); zero everywhere else.
    pub tilt: [f32; 4],
    /// How this stands on it: [mode, foot x, foot y, foreshortening].
    pub stand: [f32; 4],
}

const SOLID: u32 = 0;
pub const MASK: u32 = 1;
pub const COLOR: u32 = 2;
pub const PICTURE: u32 = 3;
/// A picture whose colours are already multiplied by its alpha (a scene
/// target): sampled as it is.
pub const PICTURE_PREMULTIPLIED: u32 = 4;
/// An uploaded picture stored straight (not premultiplied) and
/// premultiplied in the shader instead: only under
/// ANGEL_GLASS_GPU_PREMULTIPLY=1, a measured prototype lever (off by
/// default; uploads are otherwise premultiplied on the CPU).
pub const PICTURE_STRAIGHT: u32 = 5;
/// Curved foreground parchment: a solid whose `uv` carries fixed material
/// coordinates on the sheet (`stand.x` 3 bends it round the rollers).
pub const PARCHMENT: u32 = 11;
/// One textured triangle of a host's mesh in window pixels: `rect` holds
/// the first two points, `pivot` the third, `uv` and `tilt.xy` their texels.
pub const MESH_TRIANGLE: u32 = 12;
/// The first of the seven ray-marched solids (`LayerKind::Primitive`):
/// cube, sphere, pyramid, cylinder, cone, torus, octahedron.
pub const PRIMITIVE: u32 = 16;

/// What a range of instances draws with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Draw {
    Wallpaper,
    Atlas,
    Picture { id: u32, linear: bool },
}

/// Emptied instance buffers kept for the next build. A build pops five
/// working buffers (plus one per picture draw); keeping a few dozen
/// covers every consumer without retaining a frame's worth per frame.
const POOL_MAX: usize = 32;

/// One frame's instances, in draw order, and the ranges that need their own
/// texture.
#[derive(Default)]
pub struct Frame {
    pub(crate) instances: Vec<Instance>,
    pub(crate) draws: Vec<(Draw, std::ops::Range<u32>)>,
}

impl Frame {
    /// Join the parts back to front into buffers this frame already owns,
    /// one range per part that has instances, so a renderer can recycle
    /// one frame's allocations into the next instead of allocating the
    /// instance list (the frame's biggest buffer) afresh each time.
    pub fn assemble_into(
        &mut self,
        parts: &mut Vec<(Draw, Vec<Instance>)>,
        pool: &mut Vec<Vec<Instance>>,
    ) {
        self.instances.clear();
        self.draws.clear();
        self.instances
            .reserve(parts.iter().map(|(_, p)| p.len()).sum());
        for (draw, mut part) in parts.drain(..) {
            if !part.is_empty() {
                let start = self.instances.len() as u32;
                // Append moves the whole run in one copy and leaves the
                // buffer empty for the pool; a drained iterator copies
                // element by element and costs measurably more.
                self.instances.append(&mut part);
                let end = self.instances.len() as u32;
                match self.draws.last_mut() {
                    Some((last, range)) if *last == draw && range.end == start => range.end = end,
                    _ => self.draws.push((draw, start..end)),
                }
            }
            // The emptied buffer goes back for the next build to refill —
            // up to a bound: a build takes only a handful back out, so an
            // uncapped pool retained the difference (and its capacity)
            // every frame, forever.
            if pool.len() < POOL_MAX {
                pool.push(part);
            }
        }
    }
}

impl Frame {
    /// The lights among this frame's draws (what adds to what is under it:
    /// premultiplied colour with no coverage, from the atlas — a glow, a
    /// mote, a light's pool) into `out`, in order, as one draw.
    pub(crate) fn lights_into(&self, out: &mut Frame) {
        out.instances.clear();
        out.draws.clear();
        for (draw, range) in &self.draws {
            if *draw != Draw::Atlas {
                continue;
            }
            let lights = self.instances[range.start as usize..range.end as usize]
                .iter()
                .filter(|i| i.color[3] == 0.0 && i.color[..3].iter().any(|&c| c > 0.0));
            out.instances.extend(lights);
        }
        if !out.instances.is_empty() {
            out.draws.push((Draw::Atlas, 0..out.instances.len() as u32));
        }
    }
}

/// Add one instance to draw parts, joining the last part when it draws
/// with the same texture. Layers arrive back to front and mostly share
/// the atlas, so this keeps a frame of thousands of layers to a handful
/// of buffers instead of an allocation each.
pub(crate) fn push_part(parts: &mut Vec<(Draw, Vec<Instance>)>, draw: Draw, instance: Instance) {
    match parts.last_mut() {
        Some((last, group)) if *last == draw => group.push(instance),
        _ => parts.push((draw, vec![instance])),
    }
}

/// Add a run of instances to draw parts (`push_part` for a whole emitter).
pub(crate) fn extend_part(parts: &mut Vec<(Draw, Vec<Instance>)>, draw: Draw, instances: Vec<Instance>) {
    if instances.is_empty() {
        return;
    }
    match parts.last_mut() {
        Some((last, group)) if *last == draw => group.extend(instances),
        _ => parts.push((draw, instances)),
    }
}

impl Instance {
    pub fn solid(x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) -> Self {
        Self {
            rect: [x, y, w, h],
            uv: [0.0; 4],
            color,
            kind: SOLID,
            pivot: [0.0; 2],
            angle: 0.0,
            ..Zeroable::zeroed()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(n: usize) -> Vec<Instance> {
        vec![Instance::solid(0.0, 0.0, 1.0, 1.0, [0.0; 4]); n]
    }

    #[test]
    fn frame_ranges_follow_draw_order() {
        let a = Draw::Picture {
            id: 7,
            linear: false,
        };
        let b = Draw::Picture {
            id: 9,
            linear: true,
        };
        let mut frame = Frame::default();
        let mut parts = vec![
            (Draw::Atlas, solid(3)),
            (a, solid(2)),
            (b, Vec::new()),
            (Draw::Atlas, solid(4)),
            (Draw::Atlas, solid(1)),
        ];
        let mut pool = Vec::new();
        frame.assemble_into(&mut parts, &mut pool);
        assert!(parts.is_empty() && pool.len() == 5, "buffers back in the pool");
        assert_eq!(frame.instances.len(), 10);
        assert_eq!(
            frame.draws,
            [(Draw::Atlas, 0..3), (a, 3..5), (Draw::Atlas, 5..10)]
        );
    }

    #[test]
    fn the_buffer_pool_stays_bounded_across_frames() {
        // A build pops a handful of buffers but assembly returns one per
        // part: uncapped, the pool (and its retained capacity) grew by
        // the difference every frame forever — a 90-second blast soak
        // took the sampler from 1.5 GB to 7.4 GB.
        let mut frame = Frame::default();
        let mut pool: Vec<Vec<Instance>> = Vec::new();
        for _ in 0..50 {
            let mut parts: Vec<(Draw, Vec<Instance>)> =
                (0..13).map(|_| (Draw::Atlas, solid(4))).collect();
            frame.assemble_into(&mut parts, &mut pool);
            assert_eq!(frame.instances.len(), 52);
            // The next build takes its working buffers back out.
            for _ in 0..5 {
                pool.pop();
            }
        }
        assert!(pool.len() <= POOL_MAX, "pool holds {}, grows per frame", pool.len());
    }

    #[test]
    fn parts_join_their_neighbours_by_texture() {
        let mut parts = Vec::new();
        let one = Instance::solid(0.0, 0.0, 1.0, 1.0, [0.0; 4]);
        for _ in 0..1000 {
            push_part(&mut parts, Draw::Atlas, one);
        }
        push_part(&mut parts, Draw::Picture { id: 3, linear: false }, one);
        extend_part(&mut parts, Draw::Picture { id: 3, linear: false }, solid(5));
        extend_part(&mut parts, Draw::Atlas, Vec::new());
        assert_eq!(parts.len(), 2, "two buffers, not a thousand");
        assert_eq!(parts[0].1.len(), 1000);
        assert_eq!(parts[1].1.len(), 6);
    }
}
