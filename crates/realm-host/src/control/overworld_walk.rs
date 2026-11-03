//! Walk mode: the operator's own figure on the mini-viz map. An explicit
//! `p` on the focused pane or `/world walk` puts him beside the
//! knight; the arrows or WASD walk him, a click walks him there by road, and
//! Enter or E at a door opens what stands behind it. Esc leaves. The harness's
//! knight goes on with his work throughout.

use super::*;
use angel_delve::rules::host_commands::{Command as GameCommand, Destination, Reply as GameReply, VisitState};
use angel_live::realm::{Move, Panel};
use crate::world_viz::overworld::{DoorOpens, Opens};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

/// A key the terminal reports letting go walks until it is let go; this
/// only stops him if a release goes missing (repeats keep renewing it).
const HELD_LEASE: Duration = Duration::from_millis(700);
/// A key the terminal only repeats walks while the repeats keep coming.
const KEY_LEASE: Duration = Duration::from_millis(180);
/// A single press with no repeat or release behind it: a short step.
const TAP_LEASE: Duration = Duration::from_millis(110);

/// The keys walk mode holds.
#[derive(Debug, Default)]
pub struct Walk {
    pub on: bool,
    /// Direction keys down: when each was last pressed, and whether it has
    /// come again since (a repeat).
    pub(crate) held: HashMap<KeyCode, (Instant, bool)>,
    /// Keys the terminal has been seen to release.
    pub(super) releasing: HashSet<KeyCode>,
}

/// Which way a key walks him, east and south.
fn heading(code: KeyCode) -> Option<(i8, i8)> {
    match code {
        KeyCode::Up | KeyCode::Char('w' | 'k') => Some((0, -1)),
        KeyCode::Down | KeyCode::Char('s' | 'j') => Some((0, 1)),
        KeyCode::Left | KeyCode::Char('a' | 'h') => Some((-1, 0)),
        KeyCode::Right | KeyCode::Char('d' | 'l') => Some((1, 0)),
        _ => None,
    }
}

/// Whether a cell is inside a rectangle.
pub(super) fn point_in(r: ratatui_core::layout::Rect, x: u16, y: u16) -> bool {
    x >= r.x && x < r.x.saturating_add(r.width) && y >= r.y && y < r.y.saturating_add(r.height)
}

fn lower(code: KeyCode) -> KeyCode {
    match code {
        KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
        code => code,
    }
}

impl Walk {
    /// A key came up.
    pub(crate) fn release_walk_key(&mut self, key: event::KeyEvent) {
        let code = lower(key.code);
        if heading(code).is_some() {
            self.releasing.insert(code);
            self.held.remove(&code);
        }
    }

}

impl Session<'_> {
    pub fn start_walk(&mut self) {
        if self.dungeon.joined.is_some() {
            self.expand_dungeon();
            return;
        }
        if self.dungeon.shooter.is_none() {
            self.start_shooter(None);
        }
        let opened = self.dungeon.shooter.command(GameCommand::Visit {
            destination: Destination::Keep,
            wishes: Some(crate::together_realm::standing_wishes()),
        }).is_ok_and(|reply| reply == GameReply::Visited(VisitState::Opened));
        self.world.stop_walk();
        self.walk.on = false;
        self.walk.held.clear();
        if opened {
            self.expand_dungeon();
        }
    }

    pub fn stop_walk(&mut self) {
        self.world.stop_walk();
        self.walk.on = false;
        self.walk.held.clear();
    }

    /// `/world walk`: open the map and step onto it.
    pub fn walk_from_command(&mut self) -> String {
        if self.dungeon.expanded {
            self.collapse_dungeon();
        }
        self.cx.make(Move::ShowWorld);
        self.start_walk();
        "You step into the kingdom. WASD walks, E enters, F fires, Esc returns to the overview."
            .into()
    }

    /// Keys while he walks (and `p`, which starts him walking on the
    /// focused map). True when walk mode used the key.
    pub fn walk_key(&mut self, key: event::KeyEvent) -> bool {
        let code = lower(key.code);
        let press = key.kind == event::KeyEventKind::Press;
        if !self.walk.on {
            if code == KeyCode::Char('p')
                && press
                && key.modifiers == KeyModifiers::NONE
                && self.facts().composer_empty
                && self.facts().artifacts_focused
                && self.walk_map_shown()
            {
                self.start_walk();
                return true;
            }
            return false;
        }
        if code == KeyCode::Esc {
            // The global stop contract wins: Esc stops a running turn first.
            if self.facts().turn_running || self.facts().background_job {
                return false;
            }
            if press {
                self.stop_walk();
            }
            return true;
        }
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
            || !self.facts().composer_empty
            || !self.facts().artifacts_focused
        {
            return false;
        }
        if heading(code).is_some() {
            let now = Instant::now();
            let again = !press
                || self
                    .walk
                    .held
                    .get(&code)
                    .is_some_and(|&(at, _)| now.saturating_duration_since(at) < KEY_LEASE);
            self.walk.held.insert(code, (now, again));
            return true;
        }
        match code {
            // E goes in, as it meets the world inside the Delve; F only fires.
            KeyCode::Enter | KeyCode::Char('e') => {
                if press {
                    self.go_in();
                }
                true
            }
            KeyCode::Char('p') => {
                if press {
                    self.stop_walk();
                }
                true
            }
            // Every other letter stays on the map: none reaches the glass's
            // own keys or the composer while he walks.
            KeyCode::Char(_) => true,
            _ => false,
        }
    }

    /// Where the held keys steer him now, `-1..=1` east and south.
    fn walk_steer(&mut self, now: Instant) -> (i8, i8) {
        if !self.walk.on || !self.facts().terminal_focused {
            self.walk.held.clear();
            return (0, 0);
        }
        let releasing = &self.walk.releasing;
        self.walk.held.retain(|code, &mut (at, again)| {
            let lease = if releasing.contains(code) {
                HELD_LEASE
            } else if again {
                KEY_LEASE
            } else {
                TAP_LEASE
            };
            now.saturating_duration_since(at) < lease
        });
        let (x, y) = self
            .walk
            .held
            .keys()
            .filter_map(|&code| heading(code))
            .fold((0i8, 0i8), |(x, y), (dx, dy)| (x + dx, y + dy));
        (x.clamp(-1, 1), y.clamp(-1, 1))
    }

    /// Hand the held keys to the visitor before the world ticks.
    pub fn steer_walk(&mut self) {
        let steer = self.walk_steer(Instant::now());
        self.world.steer_visitor(steer);
    }

    /// While already walking, a click chooses a road destination. Otherwise
    /// leave it to visible controls, the dungeon sprite, or camera gestures.
    pub fn walk_click(&mut self, x: u16, y: u16) -> bool {
        let Some((area, view)) = *self.overworld_map else {
            return false;
        };
        if !self.walk_map_shown() || !point_in(area, x, y) {
            return false;
        }
        if !self.walk.on {
            return false;
        }
        self.cx.make(Move::Focus("artifacts"));
        let at = |cell: u16, start: u16, cells: u16, origin: i32, span: i32| {
            origin as f32
                + (f32::from(cell.saturating_sub(start)) + 0.5) * span as f32
                    / f32::from(cells.max(1))
        };
        let px = at(x, area.x, area.width, view.x, view.w);
        let py = at(y, area.y, area.height, view.y, view.h);
        self.world.walk_visitor_to(px, py);
        true
    }


    /// Go in at the door at hand.
    pub fn go_in(&mut self) {
        let Some(opens) = self.world.visitor_door().and_then(|door| door.opens()) else {
            return;
        };
        match opens {
            Opens::Room(building) => {
                if self.world.visit_interior(building) {
                    self.cx.make(Move::ResetWorldYaw);
                }
            }
            Opens::Formation => self.cx.make(Move::Open(Panel::Formation)),
            Opens::Library => self.cx.make(Move::Open(Panel::Library)),
            Opens::Tower => self.cx.make(Move::Open(Panel::Tower)),
            Opens::Lists => {
                let said = self.visit_horsemanship(crate::chivalry::Place::Tournament);
                self.system_msg(said);
            }
            Opens::Stables => {
                let said = self.visit_horsemanship(crate::chivalry::Place::Stables);
                self.system_msg(said);
            }
            Opens::School => {
                self.world.visit_overworld("school-study");
            }
            Opens::House(id) => self.cx.make(Move::Open(Panel::ModelMenu(id.0))),
            Opens::College => self.visit_college(),
            Opens::Delve => self.open_intro(),
        }
    }

    /// Walk mode lasts while its map is on screen and in focus.
    pub fn settle_walk(&mut self) {
        if self.walk.on && (!self.walk_map_shown() || !self.facts().artifacts_focused) {
            self.stop_walk();
        }
    }

}

impl Session<'_> {
    pub(crate) fn visit_college(&mut self) {
        if self.dungeon.joined.is_some() {
            self.system_msg("The College is by your party's Delve gate.");
            return;
        }
        if self.dungeon.shooter.is_none() {
            self.start_shooter(None);
        }
        if self
            .dungeon
            .shooter
            .as_ref()
            .is_some_and(|run| !run.at_home_now())
        {
            self.system_msg("The College waits beside the gate when you return.");
            return;
        }
        let _ = self.dungeon.shooter.command(GameCommand::Visit {
            destination: Destination::College, wishes: None,
        });
        self.expand_dungeon();
    }
}
