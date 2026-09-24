//! The quest's chrome around the world pane (Zelda overworld Z4).
//!
//! Everything here is a pure function of the [`Quest`](super::adventure::Quest)
//! and the world tick — the world tick *is* the clock, so no composer here ever
//! reads `SystemTime`. [`World::quest_border_style`] tints the world pane's
//! border (`draw/stage_view.rs`): amber while the fog is deep, gold while a
//! banner flies. Border colour is ratatui chrome, not plate content, so nothing
//! new is hashed into `cinematic_key`. `/world quest` prints
//! [`World::quest_report`].
//!
//! The quest line of hearts, chest glyphs and danger pips that once replaced
//! the pane's footer was retired with the Zelda HUD band: the overworld frame
//! is the world only. The border never punishes (danger is amber and never
//! coral) and comp / lean mode gets none of it.

use super::World;
use super::adventure::{LoopKind, Quest, Region};
use crate::ui::hud::{HUD_AMBER, HUD_GOLD};
use ratatui::style::Style;

/// The banner a settled, accepted submission raises over the Dragon Keep.
/// The pane border flies gold for exactly as long as it is up.
const BANNER_RAISED: &str = "banner raised";

impl World {
    /// The world pane's border tint, or `None` for the shipped chrome.
    ///
    /// A pure function of the quest: gold while an accepted submission's
    /// banner flies, amber while the fog is deep. Never coral — a stall is
    /// weather, not a fault.
    pub(crate) fn quest_border_style(&self) -> Option<Style> {
        if !crate::drive::comp_mode::ambient_stage_sim_allowed() {
            return None;
        }
        if self.quest.banner() == Some(BANNER_RAISED) {
            return Some(Style::new().fg(HUD_GOLD));
        }
        if self.quest.danger().level() >= 2 {
            return Some(Style::new().fg(HUD_AMBER));
        }
        None
    }

    /// Whether the adventure owns the world pane.
    ///
    /// Z5: off Castle Town the party is somewhere the tool traffic does not
    /// live, so an automatic landmark **arrival** must not take the pane away
    /// from the region plate — the Smithy is in town and the hero is in the
    /// mines. Homecoming still counts: the walk back with the loot is part of
    /// the quest. The moment the quest is back in Castle Town (Homecoming
    /// expiry, or the loop stopping) arrivals resume exactly as they were.
    ///
    /// Operator-chosen routes are unaffected — this only gates the *automatic*
    /// arrival overlay.
    pub(crate) fn quest_owns_pane(&self) -> bool {
        self.quest.region() != Region::CastleTown
    }

    /// `/world quest` — the adventure model as plain text. The debugging
    /// window: every field the renderers read, in one line.
    pub(crate) fn quest_report(&self) -> String {
        let quest: &Quest = &self.quest;
        let region = quest.region();
        let waypoint = super::world3d::region::waypoint(quest);
        let count = super::world3d::region::waypoint_count(region);
        let mark = super::world3d::region::waypoint_label(region, waypoint)
            .map(|name| format!(" · {name}"))
            .unwrap_or_default();
        let kind: LoopKind = quest.kind();
        let banner = quest
            .banner_text()
            .map(|text| format!(" · banner \"{text}\""))
            .unwrap_or_default();
        format!(
            "quest · {} · {} · iter {} · danger {}/3 · treasure {} · empty chests {} · \
             party {} · waypoint {}/{count}{mark}{banner}",
            region.label(),
            kind.label(),
            quest.iteration(),
            quest.danger().level(),
            quest.treasures(),
            quest.empty_chests(),
            quest.party(),
            waypoint + 1,
        )
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/world_viz/hud__tests.rs"]
mod tests;
