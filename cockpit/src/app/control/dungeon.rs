//! The explicit game surface owns its keys; the composer and model keep theirs.

use super::*;
use crate::drive::together::Action;
use crate::drive::together_dungeon::{CombatAction, Direction, Phase};

/// A key that is down: when it last arrived, whether Shift came with it, and
/// whether the terminal is repeating it.
pub(super) struct Held {
    pub(super) at: std::time::Instant,
    pub(super) shift: bool,
    pub(super) repeat: bool,
}

/// Friends a host may seat: a party of four knights.
pub(crate) const MAX_FRIENDS: usize = 3;

/// A delve open to friends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Hosting {
    /// Seats for friends, beside the host's own.
    pub(crate) friends: usize,
    /// The host plays no knight (`/dungeon_host --watch`, or
    /// `ANGEL_DELVE_HOST_KNIGHT=0`): a dedicated machine's, whose own knight
    /// stands as stone with the party.
    pub(crate) watch: bool,
}

impl Hosting {
    /// Whether a host plays no knight unless told: `ANGEL_DELVE_HOST_KNIGHT=0`.
    pub(crate) fn watch_by_default() -> bool {
        std::env::var("ANGEL_DELVE_HOST_KNIGHT").is_ok_and(|v| v.trim() == "0")
    }
}

#[derive(Default)]
pub(crate) struct DungeonView {
    pub(super) vote_serial: u64,
    pub(super) joined_ballot_open: bool,
    pub(crate) expanded: bool,
    pub(super) composer_draft: Option<(String, usize)>,
    pub(crate) controls_visible: bool,
    pub(crate) notice: String,
    /// The delve is open to friends behind one link (`/dungeon host`).
    pub(crate) hosting: Option<Hosting>,
    /// The host's delve: stepped by its server, read here as the snapshot
    /// latched for this frame (`together_sim`), played from this terminal's
    /// own seats.
    pub(crate) shooter: crate::drive::together_sim::Sim,
    /// angelX's work waiting to reach the holdings (`holdings::feed`).
    pub(crate) ops: crate::drive::together_shooter::holdings::Ops,
    /// Tool calls in flight whose success the holdings count, by event id.
    pub(crate) ops_calls: std::collections::BTreeMap<String, crate::drive::together_shooter::holdings::Op>,
    /// The loop iteration the holdings last counted.
    pub(crate) ops_loop: usize,
    pub(crate) key_releases: bool,
    pub(super) held: std::collections::HashMap<KeyCode, Held>,
    /// When the realm last took in what the run banked.
    pub(super) settled: Option<std::time::Instant>,
    pub(super) raid_serial: u64,
    pub(crate) save_path: Option<std::path::PathBuf>,
    pub(super) last_saved: Option<std::time::Instant>,
    /// The card screen (Tab) is open over the room.
    pub(crate) cards_open: bool,
    pub(crate) activity_menu: Option<u32>,
    pub(super) activity_leave_pending: bool,
    /// Never reused as notice/HUD/chorus: those surfaces are sent to guests.
    pub(crate) exhibit_text: Option<String>,
    pub(crate) exhibit_scroll: u16,
    /// Card screen page (0 = your cards, then the book) and selection.
    pub(crate) cards_page: usize,
    pub(crate) cards_sel: usize,
    /// The island's play economy, loaded on first use.
    pub(crate) realm: Option<crate::drive::together_realm::Realm>,
    /// The wish angelX is drafting, when it started, and whether its turn
    /// has been seen running.
    pub(crate) drafting: Option<(String, std::time::Instant, bool)>,
    /// The cast that speaks at the delve's moments.
    pub(crate) chorus: crate::drive::together_chorus::Chorus,
    /// Music and sound effects, under the voices.
    pub(crate) audio: crate::drive::together_audio::Audio,
    /// Sanctuary reforges: the prompt, the one angelX is drafting, the queue.
    pub(crate) forge: super::Forge,
    /// How many wishes the treasury could raise when the party last heard so.
    pub(crate) ready_heard: usize,
    /// The Delve's intro, open while a knight is being chosen.
    pub(crate) intro: Option<crate::ui::viz::delve_intro_viz::Intro>,
    /// The knight's arrival at the gate has been announced.
    pub(crate) gate_said: bool,
    /// Playing in a friend's delve: the connection to their host.
    pub(crate) joined: Option<crate::drive::together_join::Joined>,
    /// Bounty progress not yet saved: it is, between fights.
    pub(crate) bounties_unsaved: bool,
}

impl App {
    pub(crate) fn dungeon_harness_status(&self) -> String {
        if self.pending_approval.is_some() {
            "Harness · needs you · Esc to review".into()
        } else if let Some(thinking) = &self.thinking {
            let seconds = thinking.started.elapsed().as_secs();
            if seconds >= 60 {
                format!("Harness · working {}m · Esc to read", seconds / 60)
            } else {
                format!("Harness · working {seconds}s · Esc to read")
            }
        } else if self.pending_turn.is_some() {
            "Harness · queued · Esc to read".into()
        } else if self.last_turn_outcome.is_some() {
            "Harness · done · Esc to read".into()
        } else {
            "Harness · ready · Esc to code".into()
        }
    }

    pub(crate) fn dungeon_view_active(&self) -> bool {
        self.dungeon.expanded
            && (self.dungeon.shooter.is_some()
                || self.together.enabled()
                || self.dungeon.intro.is_some()
                || self.dungeon.joined.is_some())
            && self.input.is_empty()
            && !self.shell_focused
            && self.tutor_draft.is_none()
            && !self.moa_deck_owns_input()
    }

    pub(crate) fn dungeon_controls_blocked(&self) -> bool {
        self.dungeon.exhibit_text.is_some() || self.dungeon_dialog_controls_blocked()
    }

    pub(super) fn dungeon_dialog_controls_blocked(&self) -> bool {
        self.pending_approval.is_some()
            || self.loop_dialog.is_some()
            || self.agent_menu.is_some()
            || self.moa_deck_owns_input()
            || self.shell_focused
            || self.tutor_draft.is_some()
    }

    pub(super) fn expand_dungeon(&mut self) {
        self.startup_intro.dismiss(
            std::time::Instant::now(),
            crate::ui::viz::lifecycle_viz::MotionMode::Off,
        );
        if !self.dungeon.expanded {
            self.dungeon.composer_draft = Some((std::mem::take(&mut self.input), self.cursor));
            self.cursor = 0;
        }
        self.clear_dungeon_controls();
        self.dungeon.expanded = true;
        self.dungeon.controls_visible = false;
        self.selection = None;
        self.scryglass_drag = None;
        self.scryglass_enabled = true;
        self.scryglass.return_to_world();
        self.focus_module("artifacts");
        self.redraw_requested = true;
    }

    pub(crate) fn collapse_dungeon(&mut self) {
        self.dungeon.exhibit_text = None;
        self.dungeon.activity_menu = None;
        if let Some((draft, cursor)) = self.dungeon.composer_draft.take() {
            self.input = draft;
            self.cursor = cursor;
        }
        self.clear_dungeon_controls();
        self.dungeon.expanded = false;
        self.dungeon.controls_visible = false;
        self.focus_module("core");
        self.redraw_requested = true;
    }

    pub(crate) fn can_leave_dungeon_activity(&self, id: u32) -> bool {
        if let Some(run) = self.dungeon.shooter.as_ref() {
            return run.can_leave_activity(id);
        }
        self.joined_knight()
            .is_some_and(|k| k["can_leave_activity"].as_bool() == Some(true))
    }

    /// Exit keys have priority over every in-game overlay and control capture.
    pub(super) fn activity_key(&mut self, key: event::KeyEvent) -> bool {
        if !self.dungeon_view_active()
            || key.kind != event::KeyEventKind::Press
            || key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
        {
            return false;
        }
        let code = match key.code {
            KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
            c => c,
        };
        let me = if self.dungeon.joined.is_some() {
            self.joined_actor()
        } else {
            1
        };
        let couch = self.dungeon.joined.is_none() && self.dungeon.hosting.is_none();
        if let Some(seat) = self.dungeon.activity_menu {
            match code {
                KeyCode::Esc => {
                    self.dungeon.activity_menu = None;
                    self.tap_menu(seat);
                    self.clear_dungeon_controls();
                }
                KeyCode::F(2) => {
                    self.dungeon.activity_menu = None;
                    self.tap_menu(seat);
                    self.collapse_dungeon();
                }
                KeyCode::Char('1') if couch && self.can_leave_dungeon_activity(1) => {
                    self.move_menu(seat, 1);
                }
                KeyCode::Char('2') if couch && self.can_leave_dungeon_activity(2) => {
                    self.move_menu(seat, 2);
                }
                KeyCode::Tab if couch => {
                    let other = if seat == 1 { 2 } else { 1 };
                    if self.can_leave_dungeon_activity(other) {
                        self.move_menu(seat, other);
                    }
                }
                KeyCode::Enter | KeyCode::Backspace => self.leave_dungeon_activity(seat),
                _ => {}
            }
            self.redraw_requested = true;
            return true;
        }
        let seat = if couch
            && (code == KeyCode::Delete
                || (code == KeyCode::Esc && !self.can_leave_dungeon_activity(1)))
        {
            2
        } else {
            me
        };
        if !self.can_leave_dungeon_activity(seat) {
            return false;
        }
        match code {
            KeyCode::Esc => {
                self.clear_dungeon_controls();
                self.dungeon.cards_open = false;
                self.dungeon.exhibit_text = None;
                self.dungeon.forge.draft = None;
                self.dungeon.activity_menu = Some(seat);
                self.tap_menu(seat);
            }
            KeyCode::Backspace | KeyCode::Delete => self.leave_dungeon_activity(seat),
            _ => return false,
        }
        self.redraw_requested = true;
        true
    }

    /// The activity menu went up or down for knight `seat`: the menu key on
    /// its seat's client, which holds the knight while the menu is up.
    fn tap_menu(&self, seat: u32) {
        if let Some(joined) = &self.dungeon.joined {
            joined.menu();
        } else if let Some(client) = self.dungeon.shooter.seat(seat) {
            client.tap(angel_net::input::button::MENU);
        }
    }

    /// The menu passes from one couch knight to the other.
    fn move_menu(&mut self, from: u32, to: u32) {
        if from != to {
            self.tap_menu(from);
            self.tap_menu(to);
        }
        self.dungeon.activity_menu = Some(to);
    }

    fn leave_dungeon_activity(&mut self, seat: u32) {
        self.clear_dungeon_controls();
        self.dungeon.activity_menu = None;
        self.dungeon.cards_open = false;
        self.dungeon.exhibit_text = None;
        self.dungeon.forge.draft = None;
        if let Some(client) = self.dungeon.shooter.seat(seat) {
            // The menu's tap, if it has not gone, never holds the knight.
            client.untap(angel_net::input::button::MENU);
        }
        if let Some(mut run) = self.dungeon.shooter.as_mut() {
            run.leave_activity(seat);
        } else if let Some(joined) = &self.dungeon.joined {
            self.dungeon.activity_leave_pending = true;
            joined.leave_activity();
        }
        self.dungeon.notice =
            format!("Knight {seat} left the activity. Friends still inside can play on.");
    }

    /// Solo pauses outside its surface; a friend's controls only
    /// ever drive their own knight and never submit prompts.
    pub(crate) fn advance_dungeon_guest(&mut self) {
        if self.dungeon.shooter.is_some() {
            self.advance_shooter_at(std::time::Instant::now());
        }
    }

    pub(crate) fn dungeon_key(&mut self, key: event::KeyEvent) -> bool {
        if self.dungeon.intro.is_some() && self.dungeon_view_active() {
            return self.intro_key(key);
        }
        if self.dungeon.joined.is_some() {
            return self.joined_key(key);
        }
        if self.dungeon.shooter.is_some() {
            return self.shooter_key(key);
        }
        if key.code == KeyCode::F(4)
            && key.kind != event::KeyEventKind::Release
            && self.world.delve_called
            && self.input.is_empty()
        {
            self.open_intro();
            return true;
        }
        if self.dungeon_controls_blocked() {
            return false;
        }
        let command_modifier = key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER);
        if key.code == KeyCode::F(6) && !command_modifier && self.together.enabled() {
            if self.input.is_empty() {
                self.expand_dungeon();
            }
            return true;
        }
        if !self.dungeon_view_active() {
            return false;
        }
        if matches!(key.code, KeyCode::Esc | KeyCode::F(2)) {
            self.collapse_dungeon();
            return true;
        }
        if key.code == KeyCode::Char('/') && !command_modifier {
            self.collapse_dungeon();
            self.input.push('/');
            self.cursor = 1;
            return true;
        }
        // Preserve explicit stop/redraw and model-menu chords. Other text never
        // falls through to an invisible composer while the game owns the screen.
        if (key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c' | 'l' | 'g')))
            || matches!(key.code, KeyCode::F(9) | KeyCode::F(10))
        {
            return false;
        }
        if command_modifier
            || !self.dungeon.controls_visible
            || key.kind == event::KeyEventKind::Release
        {
            return true;
        }
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        let direction = match key.code {
            KeyCode::Char('w' | 'W') => Some((1, Direction::North)),
            KeyCode::Char('s' | 'S') => Some((1, Direction::South)),
            KeyCode::Char('a' | 'A') => Some((1, Direction::West)),
            KeyCode::Char('d' | 'D') => Some((1, Direction::East)),
            KeyCode::Up => Some((2, Direction::North)),
            KeyCode::Down => Some((2, Direction::South)),
            KeyCode::Left => Some((2, Direction::West)),
            KeyCode::Right => Some((2, Direction::East)),
            _ => None,
        };
        let intent = if let Some((actor, direction)) = direction {
            let dash = shift || matches!(key.code, KeyCode::Char('W' | 'A' | 'S' | 'D'));
            Some((
                actor,
                Action::Fight(if dash {
                    CombatAction::Dash(direction)
                } else {
                    CombatAction::Move(direction)
                }),
            ))
        } else {
            match key.code {
                KeyCode::Char('f' | 'F') => Some((1, Action::Fight(CombatAction::Fire))),
                KeyCode::Char('g' | 'G') => Some((1, Action::Fight(CombatAction::Cast))),
                KeyCode::Char('q' | 'Q') => Some((1, Action::Fight(CombatAction::Wait))),
                KeyCode::Enter => Some((2, Action::Fight(CombatAction::Fire))),
                KeyCode::Char(' ') => Some((2, Action::Fight(CombatAction::Cast))),
                KeyCode::Backspace => Some((2, Action::Fight(CombatAction::Wait))),
                KeyCode::Char('n' | 'N') => Some((1, Action::Descend)),
                KeyCode::Char('r' | 'R') => {
                    let phase = self
                        .together
                        .room
                        .as_ref()
                        .and_then(|r| r.run.as_ref())
                        .map(|r| r.phase);
                    if matches!(phase, Some(Phase::Won | Phase::Wiped)) {
                        Some((1, Action::Return))
                    } else if phase.is_none() {
                        self.dungeon.notice = self
                            .together
                            .ready_and_raid()
                            .map(|r| r.message)
                            .unwrap_or_else(|e| e);
                        return true;
                    } else {
                        self.dungeon.notice = "Raid still active. Finish the raid, or Esc then /together return to leave it deliberately.".into();
                        return true;
                    }
                }
                _ => None,
            }
        };
        if let Some((actor, action)) = intent {
            self.dungeon.notice = self
                .together
                .act_for(actor, action)
                .map(|result| result.message)
                .unwrap_or_else(|error| error);
        }
        true
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/dungeon_controls__tests.rs"]
mod tests;
