//! Secret rooms, the way the old dungeons kept them: now and then a fight
//! room has a cracked wall, and behind it a vault no map shows. A knight's
//! bomb in that room brings the wall down, and so does a sapper's keg or a
//! hob's bomb going off beside the crack. The Herald did not know about it,
//! and is furious.

use super::*;

/// A keg or a hob's bomb this close to the crack brings it down too.
pub(crate) const CRACK_REACH: f32 = 7.0;
/// How close a knight comes before Snibbet pays up, and how many piles.
const SNIBBET_REACH: f32 = 5.0;
const HUSH_MONEY: usize = 8;

/// Where Snibbet sits in his vault: its middle, a little north.
pub(crate) fn snibbet_at(room: &Room) -> (f32, f32) {
    (room.width() / 2.0, room.height() / 2.0 - 2.0)
}

impl Run {
    /// A blast in this room: a knight's bomb (`at` none: it fills the
    /// room) or a keg's or a hob's (near the crack, or nothing).
    pub(super) fn blast_wall(&mut self, at: Option<(f32, f32)>) {
        let Some((host, cx, cy)) = self.dungeon.crack() else {
            return;
        };
        if host != self.at || at.is_some_and(|(x, y)| (x - cx).hypot(y - cy) > CRACK_REACH) {
            return;
        }
        if !self.dungeon.open_secret() {
            return;
        }
        self.cues.push("secret_found".into());
        self.sounds.push("rock_land");
        self.sounds.push("door_open");
        self.shake = self.shake.max(16);
        if self.blasts.len() < 16 {
            self.blasts.push((cx, cy, BLAST_TICKS));
        }
        self.found = Some((self.tick, 0, "A secret room!".into()));
        self.notice("secret_room");
        self.thrill(40);
    }

    /// Snibbet's vault: the first knight to walk up to him is paid to
    /// forget he was ever here, gold heaped at their feet.
    pub(super) fn tick_snibbet(&mut self) {
        let Some(secret) = self.dungeon.secret.filter(|s| s.snibbet && !s.paid) else {
            return;
        };
        if self.at != secret.vault {
            return;
        }
        let (sx, sy) = snibbet_at(self.room());
        let near = self
            .players
            .values()
            .any(|h| h.hp > 0 && !h.stone && (h.x - sx).hypot(h.y - sy) < SNIBBET_REACH);
        if !near {
            return;
        }
        if let Some(s) = self.dungeon.secret.as_mut() {
            s.paid = true;
        }
        for k in 0..HUSH_MONEY {
            let a = k as f32 / HUSH_MONEY as f32 * std::f32::consts::TAU;
            self.drop_item(
                "gold".into(),
                sx + a.cos() * 6.0,
                sy + 7.0 + a.sin() * 3.0,
                None,
            );
        }
        self.cues.push("snibbet".into());
        self.sounds.push("card_pickup");
        self.found = Some((self.tick, 0, "It's a secret to everybody.".into()));
        self.thrill(25);
    }

    /// The first walk into the room with the crack: the Herald notices the
    /// wall.
    pub(super) fn notice_crack(&mut self, first_visit: bool) {
        if first_visit
            && self
                .dungeon
                .crack()
                .is_some_and(|(host, ..)| host == self.at)
        {
            self.cues.push("crack_seen".into());
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_secrets__tests.rs"]
mod tests;
