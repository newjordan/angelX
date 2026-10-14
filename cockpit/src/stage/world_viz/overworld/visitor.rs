//! The visitor: the operator's own figure on the map, walked by hand while
//! the harness's knight goes on with his work.
//!
//! He keeps to the ground the knight walks (roads, meadow, swamp) and stops
//! at trees, rock, water and every structure. The realm's doors are tiles it
//! already names: each place's stand, each castle's gate, the Delve's stair
//! (from across its stream). Standing at one names where it leads; going in
//! is the cockpit's to do (`app::control::overworld_walk`), since what opens
//! is not the map.

use std::collections::VecDeque;

use super::super::{Building, World};
use super::View;
use super::ink::Img;
use super::kit;
use super::live::{feet, follow_within, muster, route};
use super::map::{MAP_H, MAP_W, Place, TILE};
use super::scene::Scene;
use crate::stage::houses::HouseId;
use angel_delve::world::kingdom::step_cost;

// The realm's doors are the world's; what going in opens is the cockpit's.
pub(crate) use angel_delve::world::doors::{Door, doors, walkable};
pub(crate) use angel_delve::render::overworld::VisitorPose;

/// Walking pace in world pixels per world tick (the world ticks at 40 Hz):
/// about three tiles a second.
const PACE: f32 = 1.25;
/// His feet, in pixels either side of his middle and up from his soles:
/// every corner of them stands on open ground.
const FOOT_W: f32 = 4.0;
const FOOT_H: f32 = 3.0;
/// How far a turn slides him sideways onto a lane he presses toward, so a
/// turn into a road needs no exact aim.
const NUDGE: i32 = 8;
/// How far above his feet the camera looks (his middle, not his boots).
const FOCUS_LIFT: f32 = 8.0;
/// The camera's dead zone while he walks, half-width and half-height:
/// tighter than the knight's, so a small pane keeps him near its middle.
const DEAD: (f32, f32) = (24.0, 12.0);
/// How far around a click he looks for ground he can walk to.
const NEAR: i32 = 3;

/// What going in at a door opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Opens {
    /// The landmark's own room.
    Room(Building),
    /// The Round Table's formation board.
    Formation,
    /// The Scriptorium's shelves.
    Library,
    /// The Observatory's tower.
    Tower,
    /// The lists, where the joust is ridden.
    Lists,
    /// The stables by the tournament ground.
    Stables,
    /// The School of Magic's study.
    School,
    /// The house's model, in the model menu.
    House(HouseId),
    /// The Delve's menu.
    Delve,
    College,
}

/// What going in at a door opens, as the cockpit reads it.
pub(crate) trait DoorOpens {
    fn opens(self) -> Option<Opens>;
}

impl DoorOpens for Door {
    /// What going in opens. A place with nothing behind its door only
    /// says its name.
    fn opens(self) -> Option<Opens> {
        Some(match self {
            Door::Place(Place::Keep) => Opens::Room(Building::Keep),
            Door::Place(Place::Gatehouse) => Opens::Room(Building::Gatehouse),
            Door::Place(Place::Rookery) => Opens::Room(Building::Rookery),
            Door::Place(Place::Smithy) => Opens::Room(Building::Smithy),
            Door::Place(Place::Chapel) => Opens::Room(Building::Chapel),
            Door::Place(Place::RoundTable) => Opens::Formation,
            Door::Place(Place::Scriptorium) => Opens::Library,
            Door::Place(Place::Observatory) => Opens::Tower,
            Door::Place(Place::Lists) => Opens::Lists,
            Door::Place(Place::Tournament) => Opens::Stables,
            Door::Place(Place::School) => Opens::School,
            Door::Castle(id) => Opens::House(id),
            Door::Delve => Opens::Delve,
            Door::College => Opens::College,
            Door::Place(_) => return None,
        })
    }
}

/// The ground he can walk nearest `tile`, within `reach` tiles.
fn landing(tile: (i32, i32), reach: i32) -> Option<(i32, i32)> {
    if walkable(tile) {
        return Some(tile);
    }
    (-reach..=reach)
        .flat_map(|dy| (-reach..=reach).map(move |dx| (tile.0 + dx, tile.1 + dy)))
        .filter(|&t| walkable(t))
        .min_by_key(|&(x, y)| ((x - tile.0).pow(2) + (y - tile.1).pow(2), y, x))
}

fn tile_of(x: f32, y: f32) -> (i32, i32) {
    (
        (x / TILE as f32).floor() as i32,
        (y / TILE as f32).floor() as i32,
    )
}

/// The door at hand for someone standing at `(x, y)` (feet, world pixels):
/// the nearest door on his tile or one step from it.
pub(crate) fn door_near(x: f32, y: f32) -> Option<Door> {
    let (tx, ty) = tile_of(x, y);
    let far = |&&(door, _): &&((i32, i32), Door)| {
        let (fx, fy) = feet(door);
        (fx - x).powi(2) + (fy - y).powi(2)
    };
    doors()
        .iter()
        .filter(|((dx, dy), _)| (dx - tx).abs() + (dy - ty).abs() <= 1)
        .min_by(|a, b| far(a).total_cmp(&far(b)))
        .map(|&(_, door)| door)
}

/// Whether his feet fit at `(x, y)`: every corner on open ground.
fn clear(x: f32, y: f32) -> bool {
    [
        (x - FOOT_W, y - FOOT_H),
        (x + FOOT_W - 1.0, y - FOOT_H),
        (x - FOOT_W, y),
        (x + FOOT_W - 1.0, y),
    ]
    .into_iter()
    .all(|(px, py)| {
        let (tx, ty) = tile_of(px, py);
        step_cost(tx, ty).is_some()
    })
}

/// The tile a click at `(x, y)` (world pixels) walks him to: ground he can
/// walk as it is, the door of a building (or a gate he cannot reach), or
/// the nearest walkable ground to anything else.
pub(crate) fn click_goal(x: f32, y: f32) -> Option<(i32, i32)> {
    let tile = tile_of(x, y);
    let inside = |(fx, fy, fw, fh): (i32, i32, i32, i32)| {
        tile.0 >= fx && tile.1 >= fy && tile.0 < fx + fw && tile.1 < fy + fh
    };
    if !walkable(tile)
        && let Some(&(stand, _)) = doors().iter().find(|(_, door)| inside(door.footprint()))
    {
        return Some(stand);
    }
    landing(tile, NEAR)
}

/// The visitor's walk.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Visitor {
    x: f32,
    y: f32,
    /// On the map: walk mode is on. Off, he keeps his place for next time.
    out: bool,
    /// The direction keys held, `-1..=1` east and south.
    steer: (i8, i8),
    /// A clicked spot he walks to by road; a direction key drops it.
    path: VecDeque<(i32, i32)>,
    facing_left: bool,
    /// He moved on the last tick.
    moved: bool,
    /// Where the camera looks while he walks: it holds still while he moves
    /// inside a dead zone and follows him out of it, as it does the knight.
    cam: (f32, f32),
    /// What the map shows, published every other world tick like the knight.
    shown: (VisitorPose, (f32, f32)),
}

impl Visitor {
    fn at((x, y): (f32, f32), cam: (f32, f32)) -> Visitor {
        let mut v = Visitor {
            x,
            y,
            out: false,
            steer: (0, 0),
            path: VecDeque::new(),
            facing_left: false,
            moved: false,
            cam,
            shown: (
                VisitorPose {
                    x,
                    y,
                    walking: false,
                    facing_left: false,
                },
                cam,
            ),
        };
        v.publish();
        v
    }

    fn pose(&self) -> VisitorPose {
        VisitorPose {
            x: self.x,
            y: self.y,
            walking: self.moved,
            facing_left: self.facing_left,
        }
    }

    fn publish(&mut self) {
        self.shown = (self.pose(), self.cam);
    }

    /// One world tick: along the held direction, else along his path.
    fn step(&mut self) {
        let before = (self.x, self.y);
        let (sx, sy) = (f32::from(self.steer.0), f32::from(self.steer.1));
        if self.steer != (0, 0) {
            self.path.clear();
            let pace = if sx != 0.0 && sy != 0.0 {
                PACE * std::f32::consts::FRAC_1_SQRT_2
            } else {
                PACE
            };
            self.shift(sx * pace, 0.0, sy == 0.0);
            self.shift(0.0, sy * pace, sx == 0.0);
        } else {
            self.follow_path();
        }
        if self.x != before.0 {
            self.facing_left = self.x < before.0;
        }
        self.moved = (self.x, self.y) != before;
        self.cam = follow_within(self.cam, (self.x, self.y - FOCUS_LIFT), DEAD);
    }

    /// Move by `(dx, dy)`, one axis at a time, where his feet still fit.
    /// Pressing into a wall beside an opening slides him onto it (`assist`,
    /// when the other axis is not held).
    fn shift(&mut self, dx: f32, dy: f32, assist: bool) {
        if dx == 0.0 && dy == 0.0 {
            return;
        }
        if clear(self.x + dx, self.y + dy) {
            self.x += dx;
            self.y += dy;
            return;
        }
        if !assist {
            return;
        }
        for off in 1..=NUDGE {
            for sign in [-1.0, 1.0] {
                let o = off as f32 * sign;
                let (ox, oy) = if dx != 0.0 { (0.0, o) } else { (o, 0.0) };
                if clear(self.x + ox, self.y + oy) && clear(self.x + ox + dx, self.y + oy + dy) {
                    let slide = PACE.min(off as f32) * sign;
                    let (sx, sy) = if dx != 0.0 {
                        (0.0, slide)
                    } else {
                        (slide, 0.0)
                    };
                    if clear(self.x + sx, self.y + sy) {
                        self.x += sx;
                        self.y += sy;
                    }
                    return;
                }
            }
        }
    }

    fn follow_path(&mut self) {
        let mut stride = PACE;
        while stride > 0.0 {
            let Some(&next) = self.path.front() else {
                break;
            };
            let (gx, gy) = feet(next);
            let (dx, dy) = (gx - self.x, gy - self.y);
            let d = (dx * dx + dy * dy).sqrt();
            if d <= stride {
                (self.x, self.y) = (gx, gy);
                stride -= d;
                self.path.pop_front();
            } else {
                self.x += dx / d * stride;
                self.y += dy / d * stride;
                stride = 0.0;
            }
        }
    }

    /// Set out for `goal` along the roads. A goal no road reaches is left
    /// alone: he never walks through walls.
    fn walk_to(&mut self, goal: (i32, i32)) -> bool {
        let here = tile_of(self.x, self.y);
        let mut path: VecDeque<_> = if goal == here {
            VecDeque::new()
        } else {
            let path = route(here, goal);
            let adjacent = (goal.0 - here.0).abs() + (goal.1 - here.1).abs() == 1;
            // `route` falls back to the goal alone when nothing leads there.
            if path.last() != Some(&goal) || (path.len() == 1 && !adjacent) {
                return false;
            }
            path.into()
        };
        // First onto his own tile's line, so the first step cuts no corner.
        if (self.x, self.y) != feet(here) {
            path.push_front(here);
        }
        self.path = path;
        true
    }
}

impl World {
    /// Walk mode is on: the visitor is out on the map.
    pub(crate) fn walking(&self) -> bool {
        self.overworld_visitor.as_ref().is_some_and(|v| v.out)
    }

    /// Put the visitor on the map, where he last stood or one step beside
    /// the knight. The camera leaves any fixed view and follows him.
    pub(crate) fn start_walk(&mut self) {
        self.follow_overworld();
        let cam = self.overworld.camera();
        let knight = self.overworld.knight();
        let visitor = self.overworld_visitor.get_or_insert_with(|| {
            let here = tile_of(knight.x, knight.y);
            let spot = muster(here, 1).first().copied().unwrap_or(here);
            // Waiting at the Delve's gate, the knight stands where no road
            // goes: the visitor starts on the near bank.
            let spot = landing(spot, MAP_W.max(MAP_H)).unwrap_or(spot);
            Visitor::at(feet(spot), cam)
        });
        visitor.out = true;
        visitor.steer = (0, 0);
        visitor.path.clear();
        visitor.cam = follow_within(cam, (visitor.x, visitor.y - FOCUS_LIFT), DEAD);
        visitor.publish();
    }

    /// Walk mode ends: the visitor leaves the map and the camera goes back
    /// to the knight.
    pub(crate) fn stop_walk(&mut self) {
        if let Some(v) = self.overworld_visitor.as_mut() {
            v.out = false;
            v.steer = (0, 0);
            v.path.clear();
            v.moved = false;
        }
    }

    /// The direction keys held now, `-1..=1` east and south.
    pub(crate) fn steer_visitor(&mut self, steer: (i8, i8)) {
        if let Some(v) = self.overworld_visitor.as_mut().filter(|v| v.out) {
            v.steer = steer;
        }
    }

    /// Walk the visitor toward a clicked point of the realm (world pixels).
    pub(crate) fn walk_visitor_to(&mut self, x: f32, y: f32) -> bool {
        let Some(goal) = click_goal(x, y) else {
            return false;
        };
        self.overworld_visitor
            .as_mut()
            .filter(|v| v.out)
            .is_some_and(|v| v.walk_to(goal))
    }

    /// The door at hand where the map shows him (in whole pixels, as the
    /// map draws him and his sign).
    pub(crate) fn visitor_door(&self) -> Option<Door> {
        let pose = self.shown_visitor()?;
        door_near(pose.x.round(), pose.y.round())
    }

    /// He is on the move, or about to be.
    pub(crate) fn visitor_busy(&self) -> bool {
        self.overworld_visitor.as_ref().is_some_and(|v| {
            v.out && (v.steer != (0, 0) || !v.path.is_empty() || v.moved || v.shown.0 != v.pose())
        })
    }

    /// One world tick for the visitor, published as it is taken.
    pub(super) fn tick_visitor(&mut self) {
        if let Some(v) = self.overworld_visitor.as_mut().filter(|v| v.out) {
            v.step();
            v.publish();
        }
    }

    /// The visitor as the map last published him, while he is out.
    pub(super) fn shown_visitor(&self) -> Option<VisitorPose> {
        self.overworld_visitor
            .as_ref()
            .filter(|v| v.out)
            .map(|v| v.shown.0)
    }

    /// Stand the walking visitor at the door that opens `opens`, as if he
    /// had walked there.
    #[cfg(test)]
    pub(crate) fn stand_visitor_for_test(&mut self, opens: Opens) {
        let &(tile, _) = doors()
            .iter()
            .find(|(_, door)| door.opens() == Some(opens))
            .expect("a door opens it");
        let v = self
            .overworld_visitor
            .as_mut()
            .filter(|v| v.out)
            .expect("the visitor is out");
        (v.x, v.y) = feet(tile);
        v.path.clear();
        v.publish();
    }

    /// Where the camera looks while he walks.
    pub(super) fn visitor_camera(&self) -> Option<(f32, f32)> {
        self.overworld_visitor
            .as_ref()
            .filter(|v| v.out)
            .map(|v| v.shown.1)
    }
}

/// Over the visitor's head: the sign of the door at hand, or a small mark
/// that says which figure is his. Kept inside the view, and below his feet
/// where the top of the view leaves no room above.
pub(super) fn paint_cue(cv: &mut Img, scene: &Scene, view: View) {
    let Some(v) = scene.visitor else {
        return;
    };
    let (img, gap) = match door_near(v.x, v.y) {
        Some(door) => (kit::door_sign(&door.sign(), door.opens().is_some()), 3),
        None => (kit::visitor_mark(), 2 + (scene.tick / 2 % 2) as i32),
    };
    let (vx, vy) = (v.x as i32 - view.x, v.y as i32 - view.y);
    let x = if img.w + 2 > view.w {
        (view.w - img.w) / 2
    } else {
        (vx - img.w / 2).clamp(1, view.w - img.w - 1)
    };
    let above = vy + 2 - 16 - gap - img.h;
    let y = if above >= 1 { above } else { vy + 2 + gap };
    cv.stamp(&img, x, y);
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/world_viz/overworld__visitor_tests.rs"]
mod tests;
