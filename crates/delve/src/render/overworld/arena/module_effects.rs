//! One bounded graph painter for native and browser clients. Feature modules
//! author curves and resources; Glass evaluates only these prepared primitives.
use super::{Img, Run, UNIT};
use crate::realm::effects::{self, Layer, Shape};
use crate::ink::{bayer, hash};

const PIXELS_PER_PASS: usize = 6000;
struct Pen<'a> {
    image: &'a mut Img,
    left: usize,
}
impl Pen<'_> {
    fn put(&mut self, x: i32, y: i32, colour: [u8; 3], opacity: u8) {
        if self.left == 0 {
            return;
        }
        self.left -= 1;
        if opacity > 0 && (opacity == 255 || bayer(x, y) < f32::from(opacity) / 255.0) {
            self.image.set(x, y, colour);
        }
    }
}
pub(super) fn draw(image: &mut Img, run: &Run, layer: Layer) -> usize {
    let mut pen = Pen {
        image,
        left: PIXELS_PER_PASS,
    };
    for playing in run
        .module_visuals
        .effects
        .iter()
        .rev()
        .take(effects::MAX_ACTIVE)
    {
        if pen.left == 0 {
            break;
        }
        let Some(graph) = run.module_visuals.graphs.get(&playing.graph) else {
            continue;
        };
        let Some(age) = run
            .tick
            .checked_sub(playing.born)
            .filter(|age| *age < u64::from(graph.ticks.min(600)))
        else {
            continue;
        };
        let mut position = playing.position;
        if playing.follow {
            let Some(hero) = run.players.get(&playing.player) else {
                continue;
            };
            position[0] += hero.x;
            position[1] += hero.y;
        }
        for (index, node) in graph.nodes.iter().take(effects::MAX_NODES).enumerate() {
            if node.layer != layer || pen.left == 0 {
                continue;
            }
            let Some(key) = node.at(age as u32) else {
                continue;
            };
            if !key.scale.is_finite()
                || key.scale <= 0.0
                || key.scale > 4.0
                || !position
                    .iter()
                    .chain(&key.offset)
                    .all(|v| v.is_finite() && v.abs() <= 10_000.0)
            {
                continue;
            }
            let x = ((position[0] + key.offset[0]) * UNIT).round() as i32;
            let y = ((position[1] + key.offset[1]) * UNIT).round() as i32;
            match &node.shape {
                Shape::Ring { radius, width } => {
                    if !radius.is_finite() {
                        continue;
                    }
                    let radius = (radius.clamp(0.0, 8.0) * key.scale * UNIT).round() as i32;
                    for width in 0..(*width).min(4) {
                        let mut rx = (radius - i32::from(width)).max(0);
                        let mut ry = 0;
                        let mut error = 1 - rx;
                        while rx >= ry && pen.left > 0 {
                            for (dx, dy) in [
                                (rx, ry),
                                (ry, rx),
                                (-ry, rx),
                                (-rx, ry),
                                (-rx, -ry),
                                (-ry, -rx),
                                (ry, -rx),
                                (rx, -ry),
                            ] {
                                pen.put(x + dx, y + dy, key.colour, key.opacity);
                            }
                            ry += 1;
                            if error < 0 {
                                error += 2 * ry + 1;
                            } else {
                                rx -= 1;
                                error += 2 * (ry - rx) + 1;
                            }
                        }
                    }
                }
                Shape::Disc { radius } => {
                    if !radius.is_finite() {
                        continue;
                    }
                    let radius = (radius.clamp(0.0, 4.0) * key.scale * UNIT).round() as i32;
                    for py in (y - radius).max(0)..=(y + radius).min(pen.image.h - 1) {
                        for px in (x - radius).max(0)..=(x + radius).min(pen.image.w - 1) {
                            if pen.left == 0 {
                                break;
                            }
                            if (px - x).pow(2) + (py - y).pow(2) <= radius.pow(2) {
                                pen.put(px, py, key.colour, key.opacity);
                            }
                        }
                        if pen.left == 0 {
                            break;
                        }
                    }
                }
                Shape::Particles {
                    count,
                    spread,
                    gravity,
                } => {
                    if !spread.is_finite() || !gravity.is_finite() {
                        continue;
                    }
                    const DIRS: [[f32; 2]; 8] = [
                        [1.0, 0.0],
                        [0.7071, 0.7071],
                        [0.0, 1.0],
                        [-0.7071, 0.7071],
                        [-1.0, 0.0],
                        [-0.7071, -0.7071],
                        [0.0, -1.0],
                        [0.7071, -0.7071],
                    ];
                    let t = age as f32 / graph.ticks.max(1) as f32;
                    for particle in 0..(*count).min(64) {
                        let seed = hash(i32::from(particle), index as i32, playing.id as u32);
                        let direction = DIRS[seed as usize % DIRS.len()];
                        let distance = spread.clamp(0.0, 32.0)
                            * key.scale
                            * UNIT
                            * t
                            * (0.4 + (seed >> 16) as f32 / 65535.0 * 0.6);
                        let px = x + (direction[0] * distance).round() as i32;
                        let py = y
                            + (direction[1] * distance + gravity.clamp(-16.0, 16.0) * t * t * UNIT)
                                .round() as i32;
                        pen.put(px, py, key.colour, key.opacity);
                    }
                }
                Shape::Sprite { animation } => {
                    let Some(frame) = animation.pixels(age) else {
                        continue;
                    };
                    let size = (16.0 * key.scale).round() as i32;
                    if size == 0 {
                        continue;
                    }
                    for py in 0..size {
                        for px in 0..size {
                            if pen.left == 0 {
                                break;
                            }
                            let pixel = frame[(py * 32 / size * 32 + px * 32 / size) as usize];
                            let colour = std::array::from_fn(|i| {
                                (u16::from(pixel[i]) * u16::from(key.colour[i]) / 255) as u8
                            });
                            let alpha = (u16::from(pixel[3]) * u16::from(key.opacity) / 255) as u8;
                            pen.put(x + px - size / 2, y + py - size / 2, colour, alpha);
                        }
                        if pen.left == 0 {
                            break;
                        }
                    }
                }
            }
        }
    }
    PIXELS_PER_PASS - pen.left
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::realm::effects::{Graph, Key, Node, Playing, Scene};
    use crate::rules::abilities::Origin;
    use std::sync::Arc;
    fn scene(shape: Shape) -> Scene {
        let id = "a".repeat(64);
        Scene {
            revision: 1,
            graphs: std::collections::BTreeMap::from([(
                id.clone(),
                Arc::new(Graph {
                    ticks: 30,
                    nodes: vec![Node {
                        layer: Layer::Over,
                        keys: vec![Key {
                            tick: 0,
                            offset: [0.0, 0.0],
                            scale: 1.0,
                            colour: [255, 80, 20],
                            opacity: 255,
                        }],
                        shape,
                    }],
                }),
            )]),
            effects: vec![Playing {
                id: 1,
                player: 1,
                origin: Origin {
                    module: "test".into(),
                    generation: 1,
                    cast: "one".into(),
                },
                graph: id,
                born: 0,
                position: [8.0, 8.0],
                follow: false,
            }],
        }
    }
    #[test]
    fn graph_paint_survives_wire_roundtrip_and_effects_end_at_the_authored_tick() {
        let frames = Arc::new(crate::realm::appearance::Animation {
            frames: vec![
                vec![[255, 255, 255, 255]; 1024],
                vec![[255, 40, 80, 200]; 1024],
            ],
            frame_ticks: 2,
            phase_offset: 0,
        });
        for shape in [
            Shape::Ring {
                radius: 1.5,
                width: 2,
            },
            Shape::Disc { radius: 1.0 },
            Shape::Particles {
                count: 32,
                spread: 6.0,
                gravity: 1.0,
            },
            Shape::Sprite {
                animation: Arc::clone(&frames),
            },
        ] {
            let mut run = Run::new(3, 9, None);
            run.tick = 3;
            run.module_visuals = scene(shape);
            let guest = crate::wire::read(&crate::wire::publish(&run).unwrap()).unwrap();
            let mut host = Img::new(256, 256);
            let mut remote = host.clone();
            draw(&mut host, &run, Layer::Over);
            draw(&mut remote, &guest, Layer::Over);
            assert_eq!(host, remote);
            assert!(host.words().iter().any(|pixel| *pixel != 0));
            run.tick = 30;
            let mut ended = Img::new(256, 256);
            assert_eq!(draw(&mut ended, &run, Layer::Over), 0);
        }
    }
    #[test]
    fn graph_paint_is_bounded_even_for_a_full_scene_of_large_primitives() {
        let mut run = Run::new(3, 9, None);
        run.module_visuals = scene(Shape::Disc { radius: 4.0 });
        let mut graph = (**run.module_visuals.graphs.values().next().unwrap()).clone();
        graph.nodes[0].keys[0].scale = 4.0;
        graph.nodes = vec![graph.nodes[0].clone(); effects::MAX_NODES];
        run.module_visuals
            .graphs
            .insert("a".repeat(64), Arc::new(graph));
        let effect = run.module_visuals.effects[0].clone();
        run.module_visuals.effects = (1..=effects::MAX_ACTIVE as u64)
            .map(|id| Playing {
                id,
                ..effect.clone()
            })
            .collect();
        assert_eq!(
            draw(&mut Img::new(256, 256), &run, Layer::Over),
            PIXELS_PER_PASS
        );
    }

    #[test]
    #[ignore = "local VFX paint measurement"]
    fn crowded_graph_paint_cost() {
        let mut run = Run::new(3, 9, None);
        run.tick = 10;
        run.module_visuals = scene(Shape::Particles {
            count: 64,
            spread: 8.0,
            gravity: 2.0,
        });
        let effect = run.module_visuals.effects[0].clone();
        run.module_visuals.effects = (1..=64)
            .map(|id| Playing {
                id,
                position: [8.0 + (id % 8) as f32, 8.0 + (id / 8) as f32],
                ..effect.clone()
            })
            .collect();
        let mut image = Img::new(384, 224);
        let mut samples = Vec::new();
        for _ in 0..200 {
            image.clear_all();
            let at = std::time::Instant::now();
            std::hint::black_box(draw(&mut image, &run, Layer::Over));
            samples.push(at.elapsed().as_nanos());
        }
        samples.sort_unstable();
        eprintln!(
            "64 active particle graphs, paint only: p50={}ns p95={}ns max={}ns",
            samples[100], samples[190], samples[199]
        );
    }
}
