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
    pub pivot: [f32; 2],
    pub angle: f32,
    pub tilt: [f32; 4],
    pub stand: [f32; 4],
}

const SOLID: u32 = 0;
pub const MASK: u32 = 1;
pub const COLOR: u32 = 2;
pub const PICTURE: u32 = 3;
pub const PICTURE_PREMULTIPLIED: u32 = 4;
pub const PICTURE_STRAIGHT: u32 = 5;
pub const PARCHMENT: u32 = 11;
pub const MESH_TRIANGLE: u32 = 12;
pub const PRIMITIVE: u32 = 16;
pub const RING: u32 = 23;
pub const LINE: u32 = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Draw {
    Wallpaper,
    Atlas,
    Picture { id: u32, linear: bool },
}

const POOL_MAX: usize = 32;

#[derive(Default)]
pub struct Frame {
    pub(crate) instances: Vec<Instance>,
    pub(crate) draws: Vec<(Draw, std::ops::Range<u32>)>,
}

impl Frame {
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
                self.instances.append(&mut part);
                let end = self.instances.len() as u32;
                match self.draws.last_mut() {
                    Some((last, range)) if *last == draw && range.end == start => range.end = end,
                    _ => self.draws.push((draw, start..end)),
                }
            }
            if pool.len() < POOL_MAX {
                pool.push(part);
            }
        }
    }
}

impl Frame {
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

pub(crate) fn push_part(parts: &mut Vec<(Draw, Vec<Instance>)>, draw: Draw, instance: Instance) {
    match parts.last_mut() {
        Some((last, group)) if *last == draw => group.push(instance),
        _ => parts.push((draw, vec![instance])),
    }
}

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
        let mut frame = Frame::default();
        let mut pool: Vec<Vec<Instance>> = Vec::new();
        for _ in 0..50 {
            let mut parts: Vec<(Draw, Vec<Instance>)> =
                (0..13).map(|_| (Draw::Atlas, solid(4))).collect();
            frame.assemble_into(&mut parts, &mut pool);
            assert_eq!(frame.instances.len(), 52);
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
