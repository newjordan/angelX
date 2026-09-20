//! The quest HUD (Zelda overworld Z4): the adventure as one line of chrome.
//!
//! Everything here is a pure function of the [`Quest`](super::adventure::Quest)
//! and the world tick — the world tick *is* the clock, so no composer here ever
//! reads `SystemTime`. Two consumers:
//!
//! - [`World::quest_hud`] replaces the world pane's footer caption while an
//!   adventure is running (`draw/stage_view.rs`).
//! - [`World::quest_border_style`] tints that pane's border: amber while the
//!   fog is deep, gold while a banner flies. Border colour is ratatui chrome,
//!   not plate content, so nothing new is hashed into `cinematic_key`.
//!
//! Laws this module is written to: the HUD never punishes (danger is amber and
//! never coral, an empty chest is a hollow box and never a warning), every
//! string fits the display-cell budget it is handed, and comp / lean mode gets
//! none of it.

#![cfg_attr(not(test), allow(dead_code))]

use super::adventure::{LoopKind, Quest, Region};
use super::{Building, World, cell_width, fit_cells, fit_spans_to_cells, spans_cell_width};
use super::{health_bar, world_budget_status};
use crate::hud::{HUD_AMBER, HUD_DIM, HUD_GOLD, HUD_TEXT};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

/// Health is three hearts. Danger drains them; a nearly spent loop budget
/// drains one more.
const HEARTS: u8 = 3;
/// Chest glyphs shown before the count collapses into `×N`.
const CHEST_CAP: u32 = 6;
/// Party pips shown at most (the quest already clamps the party to 8).
const PARTY_CAP: u8 = 8;
/// The loop-budget fraction past which the party is running on fumes.
const BUDGET_PRESSURE: f32 = 0.8;
/// Narrower than this and the world status strip spends its cells on the
/// literal activity instead of naming the region.
const STATUS_REGION_FLOOR: usize = 32;

/// The banner a settled, accepted submission raises over the Dragon Keep.
/// The pane border flies gold for exactly as long as it is up.
const BANNER_RAISED: &str = "banner raised";

/// One droppable segment of the quest line. Parts leave right-to-left.
struct Part {
    spans: Vec<Span<'static>>,
}

impl Part {
    fn width(&self) -> usize {
        self.spans
            .iter()
            .map(|span| cell_width(span.content.as_ref()))
            .sum()
    }
}

fn dim(text: impl Into<String>) -> Span<'static> {
    Span::styled(text.into(), Style::new().fg(HUD_DIM))
}

fn sep() -> Span<'static> {
    dim(" · ")
}

/// `▣` per treasure, `▢` per empty chest, capped at [`CHEST_CAP`] glyphs and
/// then carrying a `×N` total. An empty chest is never coloured as a fault:
/// it is the same gold as a full one, only hollow.
fn chest_glyphs(treasures: u32, empty: u32) -> String {
    let total = treasures.saturating_add(empty);
    if total <= CHEST_CAP {
        return format!(
            "{}{}",
            "\u{25A3}".repeat(treasures as usize),
            "\u{25A2}".repeat(empty as usize)
        );
    }
    let filled = treasures.min(CHEST_CAP);
    let hollow = CHEST_CAP - filled;
    format!(
        "{}{}",
        "\u{25A3}".repeat(filled as usize),
        "\u{25A2}".repeat(hollow as usize)
    )
}

/// The `×N` tail a chest run past the cap carries.
fn chest_overflow(treasures: u32, empty: u32) -> String {
    let total = treasures.saturating_add(empty);
    if total > CHEST_CAP {
        format!(" \u{00D7}{total}")
    } else {
        String::new()
    }
}

/// `▮` per level of stall, `▯` for the calm that is left. Amber, always —
/// a stalled loop is a foggy road, not a failure.
fn danger_glyphs(level: u8) -> String {
    let level = level.min(3);
    format!(
        "{}{}",
        "\u{25AE}".repeat(level as usize),
        "\u{25AF}".repeat((3 - level) as usize)
    )
}

/// `♥` per full heart, `♡` for what danger and the budget took. Never fewer
/// than one full heart: the hero is always still standing.
fn heart_glyphs(full: u8) -> String {
    let full = full.clamp(1, HEARTS);
    format!(
        "{}{}",
        "\u{2665}".repeat(full as usize),
        "\u{2661}".repeat((HEARTS - full) as usize)
    )
}

impl World {
    /// The tightest of the loop budget's three clocks — iterations, tokens,
    /// wall deadline — as a spent fraction. An uncapped clock scores 0.
    fn quest_budget_pressure(&self) -> f32 {
        let Some(budget) = self.loop_budget else {
            return 0.0;
        };
        let spent = |used: u64, cap: u64| {
            if cap == 0 {
                0.0
            } else {
                (used as f32 / cap as f32).clamp(0.0, 1.0)
            }
        };
        spent(budget.iteration as u64, budget.max_iters as u64)
            .max(spent(
                budget.tokens_spent as u64,
                budget.token_budget as u64,
            ))
            .max(spent(budget.elapsed_secs, budget.deadline_secs))
    }

    /// Hearts left: three, drained by the stall, and one more when the loop
    /// budget is past [`BUDGET_PRESSURE`]. Floors at one.
    fn quest_hearts(&self) -> (u8, bool) {
        let pressed = self.quest_budget_pressure() > BUDGET_PRESSURE;
        let drained = HEARTS.saturating_sub(self.quest.danger().level().min(HEARTS));
        let full = drained.saturating_sub(u8::from(pressed)).max(1);
        (full, pressed)
    }

    /// The quest as one styled line, fitted to `width` display cells.
    ///
    /// `None` in Castle Town with no live banner — the pane keeps whatever
    /// caption it composes today — and `None` in comp / lean mode, which gets
    /// none of the adventure chrome.
    ///
    /// The widest form is
    /// `⚔ The Mines · iter 7 · ▣▣▢ chests · danger ▮▯▯ · ♥♥♡ · party ⚉⚉⚉ · "treasure!"`;
    /// parts leave right-to-left (banner, party, hearts, danger, chests, iter)
    /// until it fits, and the region name itself is ellipsized last.
    pub(crate) fn quest_hud(&self, width: usize) -> Option<Line<'static>> {
        // The same gate `stage_view::scryglass_scene_accessories_allowed`
        // delegates to; world_viz cannot reach into `draw`, and this is its
        // one source of truth.
        if !crate::comp_mode::ambient_stage_sim_allowed() || width == 0 {
            return None;
        }
        let quest = &self.quest;
        let region = quest.region();
        let banner = quest.banner_text();
        if region == Region::CastleTown && banner.is_none() {
            return None;
        }

        let mut parts: Vec<Part> = Vec::with_capacity(7);
        parts.push(Part {
            spans: vec![Span::styled(
                format!("{} {}", region.glyph(), region.label()),
                Style::new().fg(HUD_TEXT).add_modifier(Modifier::BOLD),
            )],
        });
        if quest.iteration() > 0 {
            parts.push(Part {
                spans: vec![sep(), dim(format!("iter {}", quest.iteration()))],
            });
        }
        let (treasures, empty) = (quest.treasures(), quest.empty_chests());
        if treasures.saturating_add(empty) > 0 {
            parts.push(Part {
                spans: vec![
                    sep(),
                    Span::styled(chest_glyphs(treasures, empty), Style::new().fg(HUD_GOLD)),
                    dim(format!("{} chests", chest_overflow(treasures, empty))),
                ],
            });
        }
        parts.push(Part {
            spans: vec![
                sep(),
                dim("danger "),
                Span::styled(
                    danger_glyphs(quest.danger().level()),
                    Style::new().fg(HUD_AMBER),
                ),
            ],
        });
        let (hearts, pressed) = self.quest_hearts();
        parts.push(Part {
            spans: vec![
                sep(),
                Span::styled(
                    heart_glyphs(hearts),
                    Style::new().fg(if pressed { HUD_DIM } else { HUD_GOLD }),
                ),
            ],
        });
        parts.push(Part {
            spans: vec![
                sep(),
                dim("party "),
                Span::styled(
                    "\u{2689}".repeat(quest.party().min(PARTY_CAP) as usize),
                    Style::new().fg(HUD_TEXT),
                ),
            ],
        });
        if let Some(text) = banner {
            parts.push(Part {
                spans: vec![
                    sep(),
                    Span::styled(format!("\"{text}\""), Style::new().fg(HUD_GOLD)),
                ],
            });
        }

        // Widest form first; drop whole parts from the right until one fits.
        let mut kept = parts.len();
        while kept > 1 {
            if parts[..kept].iter().map(Part::width).sum::<usize>() <= width {
                break;
            }
            kept -= 1;
        }
        let spans: Vec<Span<'static>> = parts
            .into_iter()
            .take(kept)
            .flat_map(|part| part.spans)
            .collect();
        // Even the bare region can outrun a very narrow pane; ellipsize it
        // rather than overflow the row.
        Some(Line::from(fit_spans_to_cells(spans, width)))
    }

    /// The world pane's border tint, or `None` for the shipped chrome.
    ///
    /// A pure function of the quest: gold while an accepted submission's
    /// banner flies, amber while the fog is deep. Never coral — a stall is
    /// weather, not a fault.
    pub(crate) fn quest_border_style(&self) -> Option<Style> {
        if !crate::comp_mode::ambient_stage_sim_allowed() {
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

    /// The region name the pane title and the world status strip prefix their
    /// live tail with. Empty in Castle Town — the town is the default place
    /// and naming it every frame says nothing.
    pub(crate) fn quest_region_chrome(&self) -> &'static str {
        match self.quest.region() {
            Region::CastleTown => "",
            region => region.label(),
        }
    }

    /// The status strip's form of the same prefix: a leading separator, and
    /// only once the strip is wide enough to afford it.
    pub(super) fn quest_status_chrome(&self, width: usize) -> String {
        let region = self.quest_region_chrome();
        if region.is_empty() || width < STATUS_REGION_FLOOR {
            String::new()
        } else {
            format!(" · {region}")
        }
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
#[path = "../../../tests/cockpit/world_viz/hud__tests.rs"]
mod tests;

impl World {
    /// Project the complete Realm status into one terminal-cell budget.
    ///
    /// Flavor is removed as whole spans (chatter, then the gain flash) before
    /// operational copy is compacted. The compact form moves the literal
    /// activity ahead of progression chrome, so a narrow pane never spends its
    /// last cells on decoration while hiding what the agent is doing.
    pub(crate) fn status_line(&self, width: usize) -> Line<'static> {
        if width == 0 {
            return Line::default();
        }

        let status_style = Style::new()
            .fg(crate::hud::HUD_TEXT)
            .add_modifier(Modifier::BOLD);
        // Off Castle Town the strip names the region beside the mood, so the
        // world map's own bottom line agrees with the pane title about where
        // the party is. It rides the always-kept prefix, so the existing
        // width-budget rails still bound it, and a narrow strip spends its
        // cells on the literal activity instead.
        let prefix = Span::styled(
            format!(" {}{}", self.mood(), self.quest_status_chrome(width)),
            status_style,
        );
        let memory_status = (self.target == Building::Chapel).then_some(match self.memory_health {
            crate::memory_store::MemoryHealth::Disabled => {
                (" · memory disabled", crate::hud::HUD_DIM)
            }
            crate::memory_store::MemoryHealth::Healthy => {
                (" · memory healthy", crate::hud::HUD_VERIFIED)
            }
            crate::memory_store::MemoryHealth::Degraded => {
                (" · memory degraded", crate::hud::HUD_DANGER)
            }
        });
        let memory_span = memory_status.map(|(label, color)| {
            Span::styled(label, Style::new().fg(color).add_modifier(Modifier::BOLD))
        });
        let atlas_span = (self.target == Building::Chapel
            && self.atlas_health != crate::atlas::AtlasHealth::Disabled)
            .then(|| {
                let color = match self.atlas_health {
                    crate::atlas::AtlasHealth::Healthy => crate::hud::HUD_VERIFIED,
                    crate::atlas::AtlasHealth::Disabled => crate::hud::HUD_DIM,
                    crate::atlas::AtlasHealth::Degraded | crate::atlas::AtlasHealth::Inert => {
                        crate::hud::HUD_DANGER
                    }
                };
                Span::styled(
                    format!(
                        " · atlas {} r{} · clerk {} q{} · conflicts {}",
                        self.atlas_health.label(),
                        self.atlas_review_count,
                        self.clerk_health.label(),
                        self.clerk_queue_depth,
                        self.resource_conflicts
                    ),
                    Style::new().fg(color).add_modifier(Modifier::BOLD),
                )
            });
        let renown_chrome = if width >= 46 {
            self.renown_chrome()
        } else {
            String::new()
        };
        let budget_status = self
            .loop_active
            .then_some(self.loop_budget)
            .flatten()
            .and_then(|budget| world_budget_status(budget, width));

        // Preserve the established roomy composition whenever it fits.
        let full_tail = if let Some(extra) = &budget_status {
            format!(" {extra}  {}", self.activity)
        } else {
            format!(
                "{}{} {}",
                self.reward_chrome(),
                renown_chrome,
                self.activity
            )
        };
        let mut full_core = vec![prefix.clone()];
        if let Some(memory) = memory_span.clone() {
            full_core.push(memory);
        }
        if let Some(atlas) = atlas_span.clone() {
            full_core.push(atlas);
        }
        full_core.push(Span::styled(full_tail, status_style));

        // Optional notes are all-or-nothing. In particular, the closing quote
        // is part of the chatter span, so clipping can never strand an opener.
        let gain = (self.tick < self.gain_until && !self.gain_note.is_empty()).then(|| {
            Span::styled(
                format!("  {}", self.gain_note),
                Style::new()
                    .fg(crate::hud::HUD_GOLD)
                    .add_modifier(Modifier::BOLD),
            )
        });
        let chatter = self.village.as_ref().and_then(|v| {
            (self.tick < v.chatter_until && !v.chatter_note.is_empty() && width >= 40).then(|| {
                Span::styled(
                    format!("  \u{201C}{}\u{201D}", fit_cells(&v.chatter_note, 46)),
                    Style::new()
                        .fg(crate::hud::HUD_BLUE)
                        .add_modifier(Modifier::ITALIC),
                )
            })
        });

        let mut candidate = full_core.clone();
        if let Some(gain) = gain.clone() {
            candidate.push(gain);
        }
        if let Some(chatter) = chatter.clone() {
            candidate.push(chatter);
        }
        if spans_cell_width(&candidate) <= width {
            return Line::from(candidate);
        }

        // Chatter is ambient flavor and leaves first. The short-lived gain
        // note is useful feedback, but it too leaves intact rather than being
        // rendered as an ambiguous fragment. When chatter is the only live
        // note, first try the activity-first compact core: progression chrome
        // should not silence the village's brief voice in an ordinary pane.
        if let Some(chatter) = chatter {
            if gain.is_none() {
                let mut compact_with_chatter = vec![prefix.clone()];
                if let Some(memory) = memory_span.clone() {
                    compact_with_chatter.push(memory);
                }
                if let Some(atlas) = atlas_span.clone() {
                    compact_with_chatter.push(atlas);
                }
                compact_with_chatter
                    .push(Span::styled(format!(" {}", self.activity), status_style));
                compact_with_chatter.push(chatter);
                if spans_cell_width(&compact_with_chatter) <= width {
                    return Line::from(compact_with_chatter);
                }
            }
            candidate.pop();
            if spans_cell_width(&candidate) <= width {
                return Line::from(candidate);
            }
        }
        if gain.is_some() {
            candidate.pop();
            if spans_cell_width(&candidate) <= width {
                return Line::from(candidate);
            }
        }

        // The roomy progression-first tail no longer fits. Rebuild it with the
        // literal operation/status first, then ellipsize the resulting styled
        // spans against the single cell budget.
        let mut compact = vec![prefix];
        if let Some(memory) = memory_span {
            compact.push(memory);
        }
        if let Some(atlas) = atlas_span {
            compact.push(atlas);
        }
        compact.push(Span::styled(format!(" {}", self.activity), status_style));
        if let Some(extra) = budget_status {
            compact.push(Span::styled(format!("  {extra}"), status_style));
        } else {
            compact.push(Span::styled(
                format!("{}{}", self.reward_chrome(), renown_chrome),
                status_style,
            ));
        }
        Line::from(fit_spans_to_cells(compact, width))
    }

    /// Progress toward the next renown construction as a five-notch bar.
    pub(super) fn renown_chrome(&self) -> String {
        let Some((ceil, _)) = self.next_unlock() else {
            return " [#####]".to_string();
        };
        let floor = [0, 10, 24, 42, 64, 96, 132, 172]
            .into_iter()
            .rev()
            .find(|floor| *floor <= self.renown)
            .unwrap_or(0);
        let fraction =
            self.renown.saturating_sub(floor) as f32 / ceil.saturating_sub(floor).max(1) as f32;
        format!(" [{}]", health_bar(fraction, 5))
    }
}
