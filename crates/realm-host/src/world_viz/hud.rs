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
use crate::hud_palette::{HUD_AMBER, HUD_GOLD};
use ratatui_core::style::Style;

const BANNER_RAISED: &str = "banner raised";

impl World {
    pub fn quest_border_style(&self) -> Option<Style> {
        if !crate::hooks::ambient_stage_sim_allowed() {
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

    pub fn quest_owns_pane(&self) -> bool {
        self.quest.region() != Region::CastleTown
    }

    pub fn quest_report(&self) -> String {
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
#[path = "../../tests/world_viz/hud__tests.rs"]
mod tests;
