//! What the game answers without changing anything, from its own state and
//! the cockpit's [`Facts`]: whether the game has the screen and the keys,
//! whether the terminal should report keys as a game's, whether the frame
//! cadence is the fast one. The cockpit asks these while it draws, holding no
//! more than a shared view of its own state, so they are on [`GameHost`]
//! itself, and a [`Session`](crate::host::Session) forwards to them.

use super::*;
use crate::host::View;
use crate::together_shooter as shooter;
use crate::together_sim as sim;
use angel_live::realm::Facts;
use std::time::Instant;

impl View<'_> {
    /// The game has the screen: the delve (or the party room, or the intro, or
    /// a friend's delve) is open over the stage, and nothing else is typing.
    pub fn view_active(&self, f: &Facts) -> bool {
        self.dungeon.expanded
            && (self.dungeon.shooter.is_some()
                || self.together.enabled()
                || self.dungeon.intro.is_some()
                || self.dungeon.joined.is_some())
            && f.composer_empty
            && !f.typing_elsewhere()
    }

    /// An exhibit or a dialog stands between the keys and the game.
    pub fn controls_blocked(&self, f: &Facts) -> bool {
        self.dungeon.exhibit_text.is_some() || self.dialog_controls_blocked(f)
    }

    pub(super) fn dialog_controls_blocked(&self, f: &Facts) -> bool {
        f.dialog_open() || f.typing_elsewhere()
    }

    /// The host's delve as the screen shows it now: the snapshot latched
    /// this frame (the words around the room read it), and the seat whose
    /// own client draws the room. A couch partner's screen is shown while
    /// the host's knight is out of the party's room and theirs is in it.
    pub fn screen(&self) -> Option<(Arc<sim::Snapshot>, u32)> {
        let snapshot = self.dungeon.shooter.snapshot()?;
        let run = &snapshot.run;
        let couch = self.dungeon.hosting.is_none() && self.dungeon.shooter.seat(2).is_some();
        let knight = angel_delve::client::screen_knight(run, couch);
        Some((Arc::clone(snapshot), knight))
    }

    /// The delve plays at the fast cadence.
    pub fn dungeon_wants_fast_tick(&self, f: &Facts) -> bool {
        self.dungeon
            .shooter
            .as_ref()
            .is_some_and(shooter::Run::active)
            && (self.dungeon_keyboard_active(f) || self.friends_seated())
    }

    /// When the delve's next fixed step is due, while it plays and is
    /// stepped on this thread (the server's own thread keeps its own time).
    pub fn step_due(&self, f: &Facts) -> Option<Instant> {
        if !self.dungeon_wants_fast_tick(f) || self.dungeon.shooter.threaded() {
            return None;
        }
        self.dungeon.shooter.clock().map(|last| last + sim::STEP)
    }

    /// A hosted delve with a friend's knight in it plays on while the host is
    /// away; with no friend seated yet it pauses like a solo run.
    pub fn friends_seated(&self) -> bool {
        self.dungeon.hosting.is_some()
            && self
                .dungeon
                .shooter
                .as_ref()
                .is_some_and(|run| {
                    // A host who plays no knight hands knight 1 to a friend.
                    let watch = self.dungeon.hosting.as_ref().is_some_and(|h| h.watch);
                    run.players.iter().any(|(&id, h)| id != 1 || (watch && !h.absent))
                })
    }

    /// The delve has the keyboard: the terminal reports its keys as a game's.
    pub fn dungeon_keyboard_active(&self, f: &Facts) -> bool {
        self.dungeon.shooter.is_some()
            && self.view_active(f)
            && self.dungeon.controls_visible
            && !self.dungeon.cards_open
            && !self.controls_blocked(f)
            && f.terminal_focused
    }

    /// The Realm pane showed the map itself on the last draw: not a room,
    /// an asset, the 3D ride or a quest.
    pub fn walk_map_shown(&self, f: &Facts) -> bool {
        self.overworld_map.is_some() && f.world_map_shown && !self.world.inside_interior()
    }

    /// While he walks with the composer empty, the terminal reports every
    /// key's press and release, as it does for the Delve, so a held WASD
    /// key walks smoothly.
    pub fn walk_keyboard_active(&self, f: &Facts) -> bool {
        self.walk.on
            && f.terminal_focused
            && f.composer_empty
            && !f.agent_menu
            && !f.deck_open
            && !f.approval_pending
    }

    /// The terminal reports keys as a game's: the Delve has the keyboard,
    /// or the visitor is walking the map.
    pub fn game_keyboard_active(&self, f: &Facts) -> bool {
        self.dungeon_keyboard_active(f) || self.walk_keyboard_active(f)
    }

    /// While he walks, the map steps at the fast cadence.
    pub fn walk_wants_fast_tick(&self, f: &Facts) -> bool {
        self.walk.on
            && f.world_pane_visible
            && (!self.walk.held.is_empty() || self.world.visitor_busy())
    }
}

impl Session<'_> {
    pub fn dungeon_view_active(&self) -> bool {
        self.view().view_active(&self.facts())
    }

    pub(crate) fn dungeon_controls_blocked(&self) -> bool {
        self.view().controls_blocked(&self.facts())
    }

    pub(crate) fn dungeon_dialog_controls_blocked(&self) -> bool {
        self.view().dialog_controls_blocked(&self.facts())
    }

    pub(crate) fn walk_map_shown(&self) -> bool {
        self.view().walk_map_shown(&self.facts())
    }

    pub fn dungeon_friends_seated(&self) -> bool {
        self.view().friends_seated()
    }

    pub(crate) fn dungeon_wants_fast_tick(&self) -> bool {
        self.view().dungeon_wants_fast_tick(&self.facts())
    }

    pub fn dungeon_keyboard_active(&self) -> bool {
        self.view().dungeon_keyboard_active(&self.facts())
    }

    pub(crate) fn dungeon_screen(&self) -> Option<(Arc<sim::Snapshot>, u32)> {
        self.view().screen()
    }
}
