//! Read-only first-person survey of the same Floor a playable Delve enters.
//! Animation moves hands/lamps only; the model owns every cut and item.
use super::{
    dungeon::{Dungeon, Prop},
    render::{self, Cell, Eye, Kit, Tex, Thing, Torch},
    textures,
};
use crate::drive::{together_settlement::Site, together_shooter::RoomKind};
use std::sync::OnceLock;

type Point = (i32, i32);

#[derive(Clone, Copy, Debug)]
enum Job {
    Dig,
    Fit,
}

/// Side-on, leaning into a tool, not an armed figure looking at the viewer.
/// Both poses keep the feet planted: time can move a hand, never the rock face.
fn miner(job: Job, swing: bool) -> &'static Tex {
    static POSES: OnceLock<[Tex; 4]> = OnceLock::new();
    &POSES.get_or_init(|| {
        [
            Tex::from_rows(&[
                "..................",
                "....rrrr..........",
                "...rOOOOrr........",
                "...rBBBhh.........",
                "....BBhhh.........",
                "....rBhh..........",
                "...rrrBr..........",
                "..rrrrrBhh........",
                "..rrrBB..hh.......",
                "...rrBB....rr.....",
                "...BBBB.....rr....",
                "...BBBB......rr...",
                "...rr.rr......GiiG",
                "...rr.rr......G..G",
                "..ggg.ggg.........",
                "..ggg.ggg.........",
            ]),
            Tex::from_rows(&[
                "..............GiiG",
                "....rrrr......G..G",
                "...rOOOOrr....rr..",
                "...rBBBhh....rr...",
                "....BBhhh...rr....",
                "....rBhh...hh.....",
                "...rrrBr..hh......",
                "..rrrrrBhh........",
                "..rrrBB...........",
                "...rrBB...........",
                "...BBBB...........",
                "...BBBB...........",
                "...rr.rr..........",
                "...rr.rr..........",
                "..ggg.ggg.........",
                "..ggg.ggg.........",
            ]),
            Tex::from_rows(&[
                "..................",
                "..................",
                "....rrrr..........",
                "...rOOOOrr........",
                "...rBBBhh.........",
                "....BBhhh.........",
                "....rBhh..........",
                "...rrrBr..........",
                "..rrrrrBhhrrGG....",
                "..rrrBB....GG.....",
                "...rrBB...........",
                "...BBBB...........",
                "...rr.rr..........",
                "...rr.rr..........",
                "..ggg.ggg.........",
                "..ggg.ggg.........",
            ]),
            Tex::from_rows(&[
                "..................",
                "..................",
                "....rrrr.....GG...",
                "...rOOOOrr...GG...",
                "...rBBBhh...rr....",
                "....BBhhh..hh.....",
                "....rBhh..hh......",
                "...rrrBrhh........",
                "..rrrrrB..........",
                "..rrrBB...........",
                "...rrBB...........",
                "...BBBB...........",
                "...rr.rr..........",
                "...rr.rr..........",
                "..ggg.ggg.........",
                "..ggg.ggg.........",
            ]),
        ]
    })[match job {
        Job::Dig => 0,
        Job::Fit => 2,
    } + usize::from(swing)]
}

pub(super) fn furnishing(prop: Prop) -> (&'static Tex, f32) {
    static ART: OnceLock<[Tex; 3]> = OnceLock::new();
    let art = ART.get_or_init(|| {
        [
            Tex::from_rows(&[
                "rrrrrrrrrr",
                "rBrrrrrrBr",
                "rrBrrrrBrr",
                "rrrBBBBrrr",
                "rrBrrrrBrr",
                "rBrrrrrrBr",
                "rrrrrrrrrr",
            ]),
            Tex::from_rows(&[
                "........................",
                "...GG....rr.............",
                "...ii....rr....HH.......",
                "..rr.....rr...HiiH......",
                "rrrrrrrrrrrrrrrrrrrrrrrr",
                "rOOOOOOOOOOOOOOOOOOOOOOr",
                "rBBBBBBBBBBBBBBBBBBBBBBr",
                "rr..rr............rr..rr",
                "rr..rr............rr..rr",
                "rr..rrrrrrrrrrrrrrrr..rr",
                "rr....................rr",
                "rr....................rr",
                "BB....................BB",
                "........................",
            ]),
            Tex::from_rows(&[
                "rrHHHHrr", "rHHHHHrr", "rBBBBBrr", "rBBBBBrr", "rrrrrrrr", "r......r",
            ]),
        ]
    });
    match prop {
        Prop::Exhibit => (exhibit_art(), 0.90),
        Prop::Crate => (&art[0], 0.5),
        Prop::Workbench => (&art[1], 0.50),
        Prop::Bed => (&art[2], 0.35),
        _ => (&textures::sprites().boulder, 0.45),
    }
}

/// A compact two-line placard AND its lectern. A long, one-line sign must
/// never be given a furniture height: the raycaster preserves its aspect ratio.
fn exhibit_art() -> &'static Tex {
    static ART: OnceLock<Tex> = OnceLock::new();
    ART.get_or_init(|| {
        use super::super::overworld::ink::{Img, text_width};
        let mut im = Img::new(56, 64);
        im.rect(0, 0, 56, 32, 'K');
        im.frame(0, 0, 56, 32, 'r');
        for (text, y) in [("LOCAL", 3), ("RESEARCH", 13), ("E READ", 23)] {
            im.text((56 - text_width(text)) / 2, y, text, '9');
        }
        im.rect(25, 32, 6, 8, 'B');
        im.rect(3, 40, 50, 7, 'r');
        im.rect(5, 41, 46, 2, 'O');
        // Open book and two stout legs, using the existing ink primitives.
        im.rect(16, 36, 23, 6, 'H');
        im.line(27, 36, 27, 41, 'B');
        for y in [37, 39] {
            im.line(18, y, 24, y, 'B');
            im.line(30, y, 36, y, 'B');
        }
        im.rect(9, 47, 5, 15, 'B');
        im.rect(42, 47, 5, 15, 'B');
        im.rect(7, 61, 9, 3, 'r');
        im.rect(40, 61, 9, 3, 'r');
        Tex::from_img(&im)
    })
}

fn anvil() -> &'static Tex {
    static ART: OnceLock<Tex> = OnceLock::new();
    ART.get_or_init(|| {
        Tex::from_rows(&[
            "....................",
            "..iiiiiiiiiiiiiiii..",
            "..GHHHHHHHHHHHHHHGG.",
            "...GGGGGGGGGGGGGGG..",
            "....ggggGGGGgggg....",
            "........GGGG........",
            ".......jGGGGj.......",
            "......jGGGGGGj......",
            ".....GGGGGGGGGG.....",
            "....rrrrrrrrrrrr....",
            "....rBBBBBBBBBBr....",
            "....rBBBBBBBBBBr....",
            "....rrrrrrrrrrrr....",
            "....................",
        ])
    })
}

fn materials() -> &'static Tex {
    static ART: OnceLock<Tex> = OnceLock::new();
    ART.get_or_init(|| {
        Tex::from_rows(&[
            "..................",
            "......iiiiiiii....",
            ".....GHHHHHHHG....",
            "....iiiiiiiiii....",
            "...GHHHHHHHHHG....",
            "...GGGGGGGGGGG....",
            "..rrrrrrrrrrrrrr..",
            "..rOOOOOOOOOOOOr..",
            "..rBBBBBBBBBBBBr..",
            "..rrrrrrrrrrrrrr..",
            "..rOOOOOOOOOOOOr..",
            "..rBBBBBBBBBBBBr..",
            "..rrrrrrrrrrrrrr..",
            "..................",
        ])
    })
}

#[derive(Clone, Copy, Debug)]
struct Worker {
    at: Point,
    target: (f32, f32),
    job: Job,
}

pub(crate) struct Scene {
    dungeon: Dungeon,
    tools: u32,
    focus: usize,
    sign: Tex,
    sign_at: (f32, f32),
    exhibit_focus: bool,
    walk: Option<super::party::Walk>,
    arrival: Point,
    heading: f32,
    workers: Vec<Worker>,
    rubble: Vec<(f32, f32)>,
    receipt_at: Option<Point>,
}

fn local(d: &Dungeon, room: usize, (col, row): Point) -> Point {
    let h = &d.halls[room];
    (h.min.0 + col - 1, h.min.1 + row - 1)
}

fn towards(a: (f32, f32), b: (f32, f32)) -> f32 {
    (b.1 - a.1).atan2(b.0 - a.0)
}

impl Scene {
    pub(crate) fn of(site: &Site) -> Self {
        let mut dungeon = Dungeon::of(&site.floor);
        for e in &site.exhibits {
            let p = crate::drive::together_settlement::exhibits::crawl_cell(
                &site.floor,
                &dungeon,
                e.marker.room,
                (e.marker.col, e.marker.row),
            );
            let (x, y) = super::party::centre(p);
            dungeon.props.retain(|(px, py, _)| *px != x || *py != y);
            dungeon.props.push((x, y, Prop::Exhibit));
            // This changes the visual wall only; Dungeon's blocked mask stays
            // intact, so the exhibit is still solid to the camera's feet.
            let index = p.1 as usize * dungeon.grid.w + p.0 as usize;
            dungeon.grid.cells[index] = Cell::Open(render::Ground::Flags);
        }
        let focus = site
            .exhibits
            .last()
            .map_or_else(|| site.focus(), |e| e.marker.room);
        let home = &dungeon.halls[0];
        let start = (home.min.0 + 11, home.max.1 - 2);
        let hall = &dungeon.halls[focus];
        let in_hall = |p: Point| {
            p.0 >= hall.min.0 && p.0 <= hall.max.0 && p.1 >= hall.min.1 && p.1 <= hall.max.1
        };
        // A station is chosen on real reachable floor, never at an arbitrary
        // offset into furniture or the uncut part of a partial excavation.
        let mut connected = std::collections::HashSet::from([start]);
        let mut frontier = std::collections::VecDeque::from([start]);
        while let Some((x, y)) = frontier.pop_front() {
            for p in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
                if dungeon.walkable(p) && connected.insert(p) {
                    frontier.push_back(p);
                }
            }
        }
        let reachable: Vec<Point> = (hall.min.1..=hall.max.1)
            .flat_map(|y| (hall.min.0..=hall.max.0).map(move |x| (x, y)))
            .filter(|p| connected.contains(p))
            .collect();
        let nearest = |want: Point| {
            reachable
                .iter()
                .copied()
                .min_by_key(|p| (p.0 - want.0).pow(2) + (p.1 - want.1).pow(2))
                .unwrap_or(start)
        };
        let kind = site.floor.rooms[focus].kind;
        let desired = if focus == 0 {
            start
        } else if kind == RoomKind::Hall {
            local(&dungeon, focus, (10, 7))
        } else if kind == RoomKind::Workshop {
            local(&dungeon, focus, (6, 7))
        } else {
            local(&dungeon, focus, (6, 6))
        };
        let mut arrival = nearest(desired);
        let mut target = super::party::centre(local(
            &dungeon,
            focus,
            if kind == RoomKind::Hall {
                (12, 7)
            } else if kind == RoomKind::Workshop {
                (5, 3)
            } else {
                (4, 3)
            },
        ));
        if focus == 0 {
            target = super::party::centre(local(&dungeon, focus, (3, 7)));
        }
        if let Some(e) = site.exhibits.last() {
            let cell = |at| {
                crate::drive::together_settlement::exhibits::crawl_cell(
                    &site.floor,
                    &dungeon,
                    focus,
                    at,
                )
            };
            let stand = cell(e.marker.stand);
            let prop = cell((e.marker.col, e.marker.row));
            // The inspection stand is real, reachable floor. The compact
            // placard and its legs fit even a tiny pane at this one-pace view.
            arrival = stand;
            target = super::party::centre(prop);
        }
        let route = dungeon.path(start, arrival).unwrap_or_else(|| vec![start]);
        let walk =
            (arrival != start).then(|| super::party::Walk::along(&route, std::f32::consts::PI));
        let heading = towards(super::party::centre(arrival), target);
        let reserved = |p: Point| {
            route.contains(&p)
                || site.exhibits.iter().any(|e| {
                    p == crate::drive::together_settlement::exhibits::crawl_cell(
                        &site.floor,
                        &dungeon,
                        e.marker.room,
                        e.marker.stand,
                    )
                })
        };
        let mut workers = Vec::new();
        // Builders stand BESIDE a real workbench/anvil, with their tool aimed
        // at it. They do not occupy the inspection stand or camera route.
        let benches: Vec<_> = dungeon
            .props
            .iter()
            .filter(|&&(x, y, prop)| {
                prop == Prop::Workbench && in_hall((x.floor() as i32, y.floor() as i32))
            })
            .copied()
            .collect();
        if !benches.is_empty() {
            let anvil_cell = super::party::centre(local(&dungeon, focus, (4, 4)));
            let mut stations = benches.clone();
            stations.sort_by_key(|&(x, y, _)| (x != anvil_cell.0 || y != anvil_cell.1) as u8);
            'station: for &(x, y, _) in &stations {
                for (dx, dy) in [(0, 1), (1, 0), (-1, 0), (0, -1)] {
                    let at = (x.floor() as i32 + dx, y.floor() as i32 + dy);
                    if reachable.contains(&at) && !reserved(at) {
                        workers.push(Worker {
                            at,
                            target: (x, y),
                            job: Job::Fit,
                        });
                        break 'station;
                    }
                }
            }
        }
        // The second station is an actual surviving rock face. For a partial
        // cut prefer its internal frontier; furnished rooms use their perimeter.
        let desired_face = local(
            &dungeon,
            focus,
            if kind == RoomKind::Hall {
                (12, 7)
            } else {
                (1, 4)
            },
        );
        let mut faces: Vec<_> = reachable
            .iter()
            .flat_map(|&at| {
                [(1, 0), (0, 1), (-1, 0), (0, -1)]
                    .into_iter()
                    .filter_map(|(dx, dy)| {
                        let rock = (at.0 + dx, at.1 + dy);
                        (matches!(dungeon.grid.at(rock.0, rock.1), Cell::Wall(_))
                            && !reserved(at)
                            && at != arrival
                            && workers.iter().all(|w| w.at != at))
                        .then_some((at, rock))
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        faces.sort_by_key(|(at, rock)| {
            (
                !in_hall(*rock),
                (at.0 - desired_face.0).pow(2) + (at.1 - desired_face.1).pow(2),
            )
        });
        for (at, rock) in faces {
            if workers.len() >= 2 {
                break;
            }
            if workers
                .iter()
                .any(|w| (w.at.0 - at.0).abs() + (w.at.1 - at.1).abs() < 3)
            {
                continue;
            }
            let a = super::party::centre(at);
            let b = super::party::centre(rock);
            workers.push(Worker {
                at,
                target: (a.0 * 0.65 + b.0 * 0.35, a.1 * 0.65 + b.1 * 0.35),
                job: Job::Dig,
            });
        }
        // Low, nonblocking chips dress rock already mined by receipts. Larger
        // benches, anvils and stacks remain within the existing Block cells.
        let mut rubble = Vec::new();
        if focus != 0 && site.resources.mined_stone > 0 {
            for worker in &workers {
                let (x, y) = super::party::centre(worker.at);
                for (dx, dy) in [(0.28, 0.22), (-0.27, 0.25), (0.22, -0.26)] {
                    rubble.push((x + dx, y + dy));
                }
            }
        }
        let (ax, ay) = super::party::centre(arrival);
        let (dx, dy) = (heading.cos(), heading.sin());
        let receipt_at = reachable
            .iter()
            .copied()
            .filter(|&p| {
                let (x, y) = super::party::centre(p);
                let depth = (x - ax) * dx + (y - ay) * dy;
                let across = (y - ay) * dx - (x - ax) * dy;
                (1.25..=3.75).contains(&depth)
                    && across.abs() < depth * 0.6
                    && !reserved(p)
                    && workers.iter().all(|w| w.at != p)
            })
            .min_by_key(|&p| {
                let (x, y) = super::party::centre(p);
                (((x - ax - dx * 2.0).powi(2) + (y - ay - dy * 2.0).powi(2)) * 100.0) as i32
            });
        // Hang the room plaque over the back bench, not through the ceiling
        // and not behind the anvil worker. It has no new floor footprint.
        let sign_at = if kind == RoomKind::Workshop && !benches.is_empty() {
            let (x, y) = super::party::centre(local(&dungeon, focus, (7, 4)));
            (x - 0.40, y)
        } else {
            workers
                .iter()
                .find(|w| matches!(w.job, Job::Fit))
                .map_or(target, |w| w.target)
        };
        Self {
            dungeon,
            tools: site.resources.tools,
            focus,
            sign: Tex::from_img(&super::arena::crawl_sign(
                site.floor.rooms[focus].kind.settlement_label(),
            )),
            sign_at,
            exhibit_focus: !site.exhibits.is_empty(),
            walk,
            arrival,
            heading,
            workers,
            rubble,
            receipt_at,
        }
    }

    fn eye(&self, elapsed: u32, (w, h): (u32, u32)) -> Eye {
        let ((x, y), heading, bob) = self
            .walk
            .as_ref()
            .and_then(|walk| walk.at(elapsed))
            .unwrap_or_else(|| {
                let mut heading = self.heading;
                if let Some(walk) = &self.walk {
                    // Finish the walk's heading before easing toward the work.
                    // No position changes during the final look, hence no clip.
                    let from = walk.last_heading(self.heading);
                    let by = (self.heading - from + std::f32::consts::PI)
                        .rem_euclid(std::f32::consts::TAU)
                        - std::f32::consts::PI;
                    let k = (elapsed.saturating_sub(walk.ticks()) as f32 / 16.0).min(1.0);
                    heading = from + by * k * k * (3.0 - 2.0 * k);
                }
                (super::party::centre(self.arrival), heading, 0.0)
            });
        // Keep vertical coverage on wide/tiny panes. The old fixed horizontal
        // FOV magnified everything when the pane was only 160 pixels tall.
        let focal = (w.max(1) as f32 / (2.0 * (std::f32::consts::FRAC_PI_3 * 0.5).tan()))
            .min(h.max(1) as f32 * 0.95);
        Eye {
            x,
            y,
            heading,
            fov: 2.0 * (w.max(1) as f32 / (2.0 * focal)).atan(),
            z: 0.52 + (bob * std::f32::consts::TAU).sin().abs() * 0.02,
            pitch: -0.04,
        }
    }

    #[cfg(test)]
    pub(crate) fn frame(&self, elapsed: u32, size: (u32, u32)) -> image::RgbaImage {
        self.events_frame(elapsed, 0, None, size)
    }

    #[cfg(test)]
    pub(crate) fn assert_exhibit_staging_for_test(&self, site: &Site) {
        let e = site.exhibits.last().unwrap();
        let cell = |at| {
            crate::drive::together_settlement::exhibits::crawl_cell(
                &site.floor,
                &self.dungeon,
                e.marker.room,
                at,
            )
        };
        assert_eq!(self.arrival, cell(e.marker.stand));
        assert!(
            self.workers
                .iter()
                .all(|w| site.exhibits.iter().all(|e| w.at
                    != crate::drive::together_settlement::exhibits::crawl_cell(
                        &site.floor,
                        &self.dungeon,
                        e.marker.room,
                        e.marker.stand,
                    )))
        );
        let prop = cell((e.marker.col, e.marker.row));
        assert!(
            !self.dungeon.walkable(prop),
            "the visible stand remains solid"
        );
        let (tex, height) = furnishing(Prop::Exhibit);
        assert!(
            height * tex.w as f32 / tex.h as f32 <= 1.0,
            "fits its Block cell"
        );
        for size in [(48, 32), (144, 80), (288, 160), (384, 224)] {
            let eye = self.eye(3600, size);
            let (x, y) = super::party::centre(prop);
            let depth = (x - eye.x) * eye.heading.cos() + (y - eye.y) * eye.heading.sin();
            assert!(depth > 0.0);
            let focal = size.0 as f32 / (2.0 * (eye.fov * 0.5).tan());
            let bottom = size.1 as f32 * (0.5 + eye.pitch) + eye.z * focal / depth;
            let top = bottom - height * focal / depth;
            let half_width = height * tex.w as f32 / tex.h as f32 * focal / depth * 0.5;
            assert!(top >= 1.0 && bottom < size.1 as f32 - 1.0);
            assert!(half_width < size.0 as f32 * 0.5 - 1.0);
        }
        if let Some(walk) = &self.walk {
            for t in 0..=walk.ticks() + 16 {
                let eye = self.eye(t, (288, 160));
                assert!(
                    self.dungeon
                        .walkable((eye.x.floor() as i32, eye.y.floor() as i32))
                );
            }
        }
    }

    /// Measurement/stall ceremonies affect light and a receipt chest only.
    /// They do not mint resources or call the model's apply/job functions.
    pub(crate) fn events_frame(
        &self,
        elapsed: u32,
        danger: u8,
        found: Option<u32>,
        size: (u32, u32),
    ) -> image::RgbaImage {
        let d = &self.dungeon;
        let eye = self.eye(elapsed, size);
        let faces = textures::faces();
        let art = textures::sprites();
        let decals = [&art.sconce[(elapsed / 6 % 2) as usize], &art.banner];
        let kit = Kit {
            walls: &faces.walls,
            decals: &decals,
            floor: &faces.floor,
            water: &faces.water,
            lava: &faces.lava,
            stairs: &faces.stairs,
            ceiling: &faces.ceiling,
        };
        let torch = Torch {
            reach: 9.0 - f32::from(danger.min(3)),
            gain: 1.05 + (elapsed as f32 * 0.13).sin() * (0.025 + f32::from(danger) * 0.025),
            glow: Some((self.sign_at.0, self.sign_at.1, 4.0)),
            sconces: super::dungeon_sconces_near(d, (eye.x, eye.y)),
        };
        let mut things = Vec::new();
        for worker in &self.workers {
            let (cx, cy) = super::party::centre(worker.at);
            let dir = towards((cx, cy), worker.target);
            // Keep the feet within this Floor cell, close enough for the head
            // of the hammer to meet the anvil rather than floating in midair.
            let (x, y) = (cx + dir.cos() * 0.27, cy + dir.sin() * 0.27);
            let right = (-eye.heading.sin(), eye.heading.cos());
            things.push(Thing {
                x,
                y,
                tex: miner(worker.job, elapsed / 12 % 2 == 1),
                height: 0.74,
                lift: 0.0,
                lit: false,
                flip: (worker.target.0 - x) * right.0 + (worker.target.1 - y) * right.1 < 0.0,
            });
            if matches!(worker.job, Job::Dig) && !self.rubble.is_empty() {
                // A small, fixed worked seam at the very face being picked.
                things.push(Thing {
                    x: worker.target.0,
                    y: worker.target.1,
                    tex: &art.boulder,
                    height: 0.22,
                    lift: 0.0,
                    lit: false,
                    flip: false,
                });
            }
        }
        if !self.exhibit_focus {
            let cut = self.focus != 0 && self.dungeon.halls[self.focus].kind == RoomKind::Hall;
            things.push(Thing {
                x: self.sign_at.0,
                y: self.sign_at.1,
                tex: &self.sign,
                height: if cut { 0.26 } else { 0.46 },
                lift: if cut { 0.72 } else { 0.52 },
                lit: false,
                flip: false,
            });
        }
        for &(px, py, prop) in &d.props {
            if (px - eye.x).hypot(py - eye.y) > 16.0 {
                continue;
            }
            let (mut tex, mut height) = furnishing(prop);
            if prop == Prop::Workbench {
                let room = d
                    .halls
                    .iter()
                    .find(|hall| {
                        px >= hall.min.0 as f32
                            && px <= hall.max.0 as f32 + 1.0
                            && py >= hall.min.1 as f32
                            && py <= hall.max.1 as f32 + 1.0
                    })
                    .unwrap();
                let (col, row) = (
                    px.floor() as i32 - room.min.0 + 1,
                    py.floor() as i32 - room.min.1 + 1,
                );
                // Dress the four paid-for collision cells as one workshop,
                // not four identical little tables spread across a dungeon.
                if (col, row) == (4, 4) {
                    tex = anvil();
                    height = 0.44;
                } else if (col, row) == (4, 3) || (col, row) == (7, 3) {
                    tex = materials();
                    height = 0.62;
                }
            }
            if prop == Prop::Crate && found.is_some_and(|t| t >= 100) {
                tex = &art.chest_open;
            }
            things.push(Thing {
                x: px,
                y: py,
                tex,
                height,
                lift: 0.0,
                lit: false,
                flip: false,
            });
        }
        for &(x, y) in &self.rubble {
            things.push(Thing {
                x,
                y,
                tex: &art.boulder,
                height: 0.07,
                lift: 0.0,
                lit: false,
                flip: false,
            });
        }
        if found.is_some_and(|t| t >= 100)
            && let Some(at) = self.receipt_at
        {
            let (x, y) = super::party::centre(at);
            // A small, transient measurement receipt, not paid-for storage.
            // It sits off the camera route and working/inspection stations
            // on real Floor; it never changes collision or the resource ledger.
            things.push(Thing {
                x,
                y,
                tex: &art.chest_open,
                height: 0.32,
                lift: 0.0,
                lit: false,
                flip: false,
            });
        }
        if self.tools > 0
            && let Some(&(x, y, _)) = d.props.iter().find(|&&(x, y, p)| {
                let hall = &d.halls[self.focus];
                p == Prop::Workbench
                    && x >= hall.min.0 as f32
                    && x <= hall.max.0 as f32
                    && y >= hall.min.1 as f32
                    && y <= hall.max.1 as f32
            })
        {
            static TOOL: OnceLock<Tex> = OnceLock::new();
            let tex = TOOL.get_or_init(|| {
                Tex::from_rows(&[
                    "GGG....GGG",
                    ".ii....ii.",
                    ".rr....rr.",
                    ".rr....rr.",
                    "rrrrrrrrrr",
                    ".rr....rr.",
                ])
            });
            things.push(Thing {
                x,
                y,
                tex,
                height: 0.20,
                lift: 0.50,
                lit: false,
                flip: false,
            });
        }
        let mut out = render::render(&d.grid, &eye, &things, &kit, torch, size);
        // Surveyor's torch only: routine coding no longer swings a sword.
        let s = size.1 as f32 / 180.0;
        let lamp = &art.torch[(elapsed / 6 % 2) as usize];
        super::stamp(&mut out, lamp, (2.0, size.1 as f32 - lamp.h as f32 * s), s);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settlement_camera_starts_on_the_hall_rug_and_walks_to_the_excavation() {
        let mut site = Site::new(77, "Miner");
        let initial = Scene::of(&site);
        assert!(initial.walk.is_none());
        assert!(initial.dungeon.walkable(initial.arrival));
        let rug = initial.arrival;
        for n in 1..=30 {
            site.apply("A", n, true).unwrap();
            let scene = Scene::of(&site);
            let hall = &scene.dungeon.halls[0];
            let start = (hall.min.0 + 11, hall.max.1 - 2);
            assert_eq!(start.0 - hall.min.0, 11);
            assert!(scene.dungeon.walkable(start));
            let walk = scene.walk.as_ref().expect("continuous route from the hall");
            assert_eq!(walk.at(0).unwrap().0, super::super::party::centre(start));
            assert!(walk.at(walk.ticks()).is_none());
            assert!(scene.dungeon.walkable(scene.arrival));
            for t in 0..=walk.ticks() + 16 {
                let eye = scene.eye(t, (288, 160));
                assert!(
                    scene
                        .dungeon
                        .walkable((eye.x.floor() as i32, eye.y.floor() as i32))
                );
            }
            assert!(scene.workers.iter().all(|w| scene.dungeon.walkable(w.at)));
            for worker in &scene.workers {
                if matches!(worker.job, Job::Fit) {
                    assert!(
                        scene
                            .dungeon
                            .props
                            .iter()
                            .any(|&(x, y, p)| p == Prop::Workbench && worker.target == (x, y))
                    );
                }
            }
            for &(x, y, prop) in &scene.dungeon.props {
                if matches!(prop, Prop::Crate | Prop::Workbench | Prop::Bed) {
                    assert!(!scene.dungeon.walkable((x.floor() as i32, y.floor() as i32)));
                    let (tex, height) = furnishing(prop);
                    assert!(height * tex.w as f32 / tex.h as f32 <= 1.0);
                }
            }
            let route = scene.dungeon.path(start, scene.arrival).unwrap();
            if let Some(at) = scene.receipt_at {
                assert!(scene.dungeon.walkable(at));
                assert!(!route.contains(&at) && scene.workers.iter().all(|w| w.at != at));
            }
            assert!(scene.workers.iter().all(
                |w| !route.contains(&w.at) && scene.dungeon.path(scene.arrival, w.at).is_some()
            ));
        }
        assert_ne!(rug, (0, 0));
    }

    #[test]
    fn settlement_empty_tiny_still_and_animated_frames_are_read_only() {
        let mut site = Site::new(77, "Miner");
        for receipts in [0, 1, 3, 4, 9, 14, 19, 30] {
            for n in 1..=receipts {
                site.apply("A", n, true).unwrap();
            }
            let before = serde_json::to_value(&site).unwrap();
            let scene = Scene::of(&site);
            for size in [(0, 0), (1, 1), (48, 32), (144, 80), (288, 160)] {
                for t in [0, 12, 3600, u32::MAX] {
                    let img = scene.frame(t, size);
                    assert_eq!(img.dimensions(), (size.0.max(1), size.1.max(1)));
                    assert_eq!(img, scene.frame(t, size));
                }
            }
            assert_eq!(serde_json::to_value(&site).unwrap(), before);
            if receipts == 0 {
                assert!(scene.rubble.is_empty());
            }
        }
    }
}
