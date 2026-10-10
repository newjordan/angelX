//! Fortune's audience. The Delve is watched, the way a show is: a live
//! count of viewers that climbs with spectacle. The
//! audience reacts to what the Herald calls — a streak of kills, an
//! ultimate, a knight going down and getting up, a guardian falling — and
//! to every kill a little.
//!
//! At each milestone a fan throws a box down to the party: a crate on a
//! little parachute in Fortune's red and cream, which bursts into a card and
//! gold where it lands. The first comes at 250K viewers; each after needs
//! more than the last. A million viewers is Must-See TV; ten million, Prime
//! Time.
//!
//! Every delve is a new show, and the count starts at nothing on the stair
//! (the Undercroft is off the air). The realm keeps its best show.

use super::*;

/// Thousands of viewers for the first fan box; box `n` comes at `n²` times
/// this: 250K, 1M, 2.25M, 4M …
pub(crate) const MILESTONE: u32 = 250;
/// Ticks a fan box takes to float down.
pub(crate) const BOX_FALL: u32 = 2 * HZ;

/// A fan's box on its way down: where it will land, and ticks to go.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct FanBox {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) fall: u32,
}

/// Thousands of viewers fan box `n` (from 1) comes at.
pub(crate) fn milestone(n: u32) -> u32 {
    MILESTONE * n * n
}

/// The audience at `thousands`, as the board shows it: 640K, 1.24M.
pub(crate) fn viewers(thousands: u32) -> String {
    if thousands >= 1000 {
        format!("{:.2}M", thousands as f32 / 1000.0)
    } else {
        format!("{thousands}K")
    }
}

/// Thousands of viewers a moment the Herald calls is worth.
fn reaction(cue: &str) -> u32 {
    let (head, _) = cue.split_once(':').unwrap_or((cue, ""));
    match head {
        "first_blood" | "slay2" | "room_clear" | "necro_raise" | "shaman_wards" => 5,
        "low_hp" | "boar_dazed" | "card_rare" | "goblin" | "great_hall" => 10,
        "slay3" | "boss_rise" | "goblin_escaped" | "collapse_warn" => 15,
        "hooked" => 25,
        "pit_rise" => 30,
        "talisman_used" => 60,
        "pit_fall" => 150,
        "level_up" => 30,
        "ult" | "boss_rage" | "mimic" | "mimic_fall" | "descend" => 20,
        "flawless" | "card_relic" | "revive" | "lair" => 30,
        "slay4" | "second_wind" | "knight_down" | "collapse" => 40,
        "goblin_caught" | "the_deep" => 50,
        "boss_fall" => 90,
        "victory" => 120,
        "grail" => 300,
        _ => 0,
    }
}

impl Run {
    /// One tick of the show: the audience reacts to the cues since
    /// `heard`, and fan boxes float down.
    pub(super) fn tick_audience(&mut self, heard: usize) {
        let roar: u32 = self
            .cues
            .get(heard..)
            .unwrap_or_default()
            .iter()
            .map(|cue| reaction(cue))
            .sum();
        self.thrill(roar);
        let mut landed = Vec::new();
        for parcel in &mut self.fan_boxes {
            parcel.fall = parcel.fall.saturating_sub(1);
            if parcel.fall == 0 {
                landed.push((parcel.x, parcel.y));
            }
        }
        self.fan_boxes.retain(|b| b.fall > 0);
        for (x, y) in landed {
            // A card from the book, and gold either side of it.
            let pack = self.dungeon.pack;
            if let Some(card) = self.book.roll_chest(&mut self.rng, pack).into_iter().next() {
                self.drop_item(card, x, y, None);
            }
            for side in [-1.0f32, 1.0] {
                self.drop_item("gold".into(), x + side * 1.6, y + 0.8, None);
            }
            if self.blasts.len() < 16 {
                self.blasts.push((x, y, BLAST_TICKS));
            }
            self.sounds.push("chest_open");
            self.cues.push("fan_box_open".into());
        }
    }

    /// The audience saw something worth `thousands` more viewers. A new
    /// milestone throws a fan box down beside a knight.
    pub(super) fn thrill(&mut self, thousands: u32) {
        if thousands == 0 || self.dungeon.depth == 0 || !self.active() {
            return;
        }
        // Sponsors' Night: every viewer brings a friend.
        let thousands = if self.mode == fortune::Mode::Sponsors {
            thousands.saturating_mul(2)
        } else {
            thousands
        };
        let before = self.audience;
        self.audience = self.audience.saturating_add(thousands);
        if before < 1000 && self.audience >= 1000 {
            self.notice("must_see_tv");
            self.cues.push("audience_million".into());
        }
        if before < 10_000 && self.audience >= 10_000 {
            self.notice("prime_time");
            self.cues.push("audience_prime".into());
        }
        while self.audience >= milestone(self.fans + 1) {
            self.fans += 1;
            self.throw_fan_box();
            self.cues.push("fan_box".into());
        }
    }

    /// A fan box starts down a step or two from a knight, on open floor.
    pub(super) fn throw_fan_box(&mut self) {
        let living: Vec<(f32, f32)> = self
            .players
            .values()
            .filter(|h| h.hp > 0 && !h.stone)
            .map(|h| (h.x, h.y))
            .collect();
        if living.is_empty() {
            return;
        }
        let (hx, hy) = living[self.rng.below(living.len())];
        let turn = self.rng.below(8) as f32 * std::f32::consts::FRAC_PI_4;
        let room = self.room();
        let (w, h) = (room.width(), room.height());
        let (x, y) = [3.2f32, 2.2, 1.2]
            .into_iter()
            .map(|reach| {
                (
                    (hx + turn.cos() * reach).clamp(3.0, w - 3.0),
                    (hy + turn.sin() * reach).clamp(3.0, h - 3.0),
                )
            })
            .find(|&(x, y)| {
                room.tile((x / TILE_UNITS) as i32, (y / TILE_UNITS) as i32) == Tile::Floor
            })
            .unwrap_or((hx, hy));
        if self.fan_boxes.len() < 4 {
            self.fan_boxes.push(FanBox {
                x,
                y,
                fall: BOX_FALL,
            });
        }
    }

    /// The party walked on with boxes still in the air: they follow, and
    /// come down in the new room.
    pub(super) fn follow_fan_boxes(&mut self) {
        let falling = std::mem::take(&mut self.fan_boxes);
        for parcel in falling {
            self.throw_fan_box();
            if let Some(b) = self.fan_boxes.last_mut() {
                b.fall = parcel.fall;
            }
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_audience__tests.rs"]
mod tests;
