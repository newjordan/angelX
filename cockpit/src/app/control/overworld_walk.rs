//! Walk mode: the operator's own figure on the mini-viz map. An explicit
//! `p` on the focused pane or `/world walk` puts him beside the
//! knight; the arrows or WASD walk him, a click walks him there by road, and
//! Enter or E at a door opens what stands behind it. Esc leaves. The harness's
//! knight goes on with his work throughout.

use super::*;
use crate::app::WorldButton;
use crate::stage::world_viz::overworld::{DoorOpens, Opens};
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
pub(crate) struct Walk {
    pub(crate) on: bool,
    /// Direction keys down: when each was last pressed, and whether it has
    /// come again since (a repeat).
    held: HashMap<KeyCode, (Instant, bool)>,
    /// Keys the terminal has been seen to release.
    releasing: HashSet<KeyCode>,
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

fn lower(code: KeyCode) -> KeyCode {
    match code {
        KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
        code => code,
    }
}

impl App {
    /// The Realm pane showed the map itself on the last draw: not a room,
    /// an asset, the 3D ride or a quest.
    pub(crate) fn walk_map_shown(&self) -> bool {
        self.overworld_map.is_some()
            && self.scryglass.surface == crate::ui::scryglass::StageSurface::WorldMap
            && self.scryglass.active_media().is_none()
            && !self.world.inside_interior()
    }

    fn artifacts_focused(&self) -> bool {
        self.module_host
            .focused()
            .is_some_and(|id| id.as_str() == "artifacts")
    }

    pub(crate) fn start_walk(&mut self) {
        if self.dungeon.joined.is_some() {
            self.expand_dungeon();
            return;
        }
        if self.dungeon.shooter.is_none() {
            self.start_shooter(None);
        }
        let mut opened = false;
        if let Some(mut run) = self.dungeon.shooter.as_mut() {
            if run.room().kind == crate::drive::together_shooter::RoomKind::Kingdom {
                opened = true;
            } else if run.at_home_now() {
                run.kingdom.wishes = crate::drive::together_realm::standing_wishes();
                run.walk_kingdom(Some(
                    crate::stage::world_viz::overworld::kingdom::keep_stand(),
                ));
                opened = true;
            }
        }
        self.world.stop_walk();
        self.walk.on = false;
        self.walk.held.clear();
        if opened {
            self.expand_dungeon();
        }
    }

    pub(crate) fn stop_walk(&mut self) {
        self.world.stop_walk();
        self.walk.on = false;
        self.walk.held.clear();
    }

    /// `/world walk`: open the map and step onto it.
    pub(crate) fn walk_from_command(&mut self) -> String {
        if self.dungeon.expanded {
            self.collapse_dungeon();
        }
        self.scryglass_enabled = true;
        self.scryglass.return_to_world();
        self.start_walk();
        "You step into the kingdom. WASD walks, E enters, F fires, Esc returns to the overview."
            .into()
    }

    /// Keys while he walks (and `p`, which starts him walking on the
    /// focused map). True when walk mode used the key.
    pub(super) fn walk_key(&mut self, key: event::KeyEvent) -> bool {
        let code = lower(key.code);
        let press = key.kind == event::KeyEventKind::Press;
        if !self.walk.on {
            if code == KeyCode::Char('p')
                && press
                && key.modifiers == KeyModifiers::NONE
                && self.input.is_empty()
                && self.artifacts_focused()
                && self.walk_map_shown()
            {
                self.start_walk();
                return true;
            }
            return false;
        }
        if code == KeyCode::Esc {
            // The global stop contract wins: Esc stops a running turn first.
            if self.thinking.is_some() || self.bg_job.is_some() {
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
            || !self.input.is_empty()
            || !self.artifacts_focused()
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

    /// A key came up.
    pub(crate) fn release_walk_key(&mut self, key: event::KeyEvent) {
        let code = lower(key.code);
        if heading(code).is_some() {
            self.walk.releasing.insert(code);
            self.walk.held.remove(&code);
        }
    }

    /// Where the held keys steer him now, `-1..=1` east and south.
    fn walk_steer(&mut self, now: Instant) -> (i8, i8) {
        if !self.walk.on || !self.terminal_focused {
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
    pub(crate) fn steer_walk(&mut self) {
        let steer = self.walk_steer(Instant::now());
        self.world.steer_visitor(steer);
    }

    /// While already walking, a click chooses a road destination. Otherwise
    /// leave it to visible controls, the dungeon sprite, or camera gestures.
    pub(super) fn walk_click(&mut self, x: u16, y: u16) -> bool {
        let Some((area, view)) = self.overworld_map else {
            return false;
        };
        if !self.walk_map_shown() || !crate::ui::mouse::point_in(area, x, y) {
            return false;
        }
        if !self.walk.on {
            return false;
        }
        self.focus_module("artifacts");
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
    pub(crate) fn go_in(&mut self) {
        let Some(opens) = self.world.visitor_door().and_then(|door| door.opens()) else {
            return;
        };
        match opens {
            Opens::Room(building) => {
                if self.world.visit_interior(building) {
                    self.reset_world_yaw();
                }
            }
            Opens::Formation => self.open_moa_deck(None),
            Opens::Library => self.apply_world_button(WorldButton::ScryglassLibrary),
            Opens::Tower => self.apply_world_button(WorldButton::Tower),
            Opens::Lists => {
                let said = self.visit_horsemanship(crate::drive::chivalry::Place::Tournament);
                self.system_msg(said);
            }
            Opens::Stables => {
                let said = self.visit_horsemanship(crate::drive::chivalry::Place::Stables);
                self.system_msg(said);
            }
            Opens::School => {
                self.world.visit_overworld("school-study");
            }
            Opens::House(id) => {
                let family = crate::stage::houses::get(id).family;
                if !self.open_agent_menu_filtered(
                    crate::ui::agent_panel::controls::AgentMenuKind::Model,
                    family,
                ) {
                    self.system_msg(format!(
                        "The model menu opens for {family} once this turn is done."
                    ));
                }
            }
            Opens::College => self.visit_college(),
            Opens::Delve => self.open_intro(),
        }
    }

    /// Walk mode lasts while its map is on screen and in focus.
    pub(crate) fn settle_walk(&mut self) {
        if self.walk.on && (!self.walk_map_shown() || !self.artifacts_focused()) {
            self.stop_walk();
        }
    }

    /// While he walks with the composer empty, the terminal reports every
    /// key's press and release, as it does for the Delve, so a held WASD
    /// key walks smoothly.
    pub(crate) fn walk_keyboard_active(&self) -> bool {
        self.walk.on
            && self.terminal_focused
            && self.input.is_empty()
            && self.agent_menu.is_none()
            && self.moa_deck.is_none()
            && self.pending_approval.is_none()
    }

    /// The terminal reports keys as a game's: the Delve has the keyboard,
    /// or the visitor is walking the map.
    pub(crate) fn game_keyboard_active(&self) -> bool {
        self.dungeon_keyboard_active() || self.walk_keyboard_active()
    }

    /// While he walks, the map steps at the fast cadence.
    pub(crate) fn walk_wants_fast_tick(&self) -> bool {
        self.walk.on
            && self.world_pane_visible
            && (!self.walk.held.is_empty() || self.world.visitor_busy())
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/overworld_walk__tests.rs"]
mod tests;

impl App {
    pub(crate) fn visit_college(&mut self) {
        use crate::drive::together_shooter::{world, RoomKind};
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
        if let Some(mut run) = self.dungeon.shooter.as_mut() {
            world::raise_world(&mut run.dungeon);
            if let Some(index) = world::room_of(&run.dungeon, RoomKind::CollegeGrounds) {
                run.arrive(index, (12.0, 10.5));
            }
        }
        self.expand_dungeon();
    }
}
