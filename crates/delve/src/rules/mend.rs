//! A run mended before it goes out or comes back in: what a page would
//! refuse to draw (`wire::drawable`) or a save would be refused for
//! (`Run::valid_snapshot`) is put right where it can be, so one bad record
//! never freezes every seat at once or drops the saved run.
//!
//! Each fix is the smallest that makes its check pass: a way back that
//! points outside its room is pulled into it, a knight who is nowhere
//! stands in the middle of the room, and a game whose state cannot be read
//! ends (the room stays, and the party can walk out of it).
use super::kingdom::State as KingdomState;
use super::map::{MAP_H, MAP_W};
use super::*;

impl Run {
    /// Put right what [`crate::wire::drawable`] or [`Run::valid_snapshot`]
    /// would refuse and can be put right. Returns what was mended, empty
    /// when nothing was wrong.
    pub fn mend(&mut self) -> Vec<&'static str> {
        let mut mended = Vec::new();
        let rooms = self.dungeon.rooms.len();
        if rooms == 0 {
            return mended;
        }
        if self.at >= rooms {
            self.at = 0;
            mended.push("room");
        }
        let (w, h) = (self.room().width(), self.room().height());
        let home = (w / 2.0, h / 2.0);
        for hero in self.players.values_mut() {
            if !place_ok(hero.x, hero.y) {
                hero.x = w / 2.0;
                hero.y = h / 2.0;
                mended.push("knight");
            }
            if hero.camera.is_some_and(|(x, y)| !place_ok(x, y)) {
                hero.camera = None;
                mended.push("camera");
            }
            if mend_hero_bounds(hero) {
                mended.push("health");
            }
            if hero.spin > spin::TICKS {
                hero.spin = spin::TICKS;
                mended.push("spin");
            }
        }
        if !self.exits.valid(rooms) {
            self.mend_exits(rooms);
            mended.push("exits");
        }
        let players = &self.players;
        let before = self.exits.outside.len();
        self.exits.outside.retain(|id, _| players.contains_key(id));
        if self.exits.outside.len() != before {
            mended.push("outside");
        }
        if !self.kingdom.valid() {
            mend_kingdom(&mut self.kingdom);
            mended.push("kingdom");
        }
        if !self.tune.valid() {
            self.tune = tune::Tune::default();
            mended.push("tune");
        }
        // A game whose state cannot be read ends.
        macro_rules! end_unless {
            ($($field:ident),*) => {$(
                if self.$field.as_ref().is_some_and(|s| !s.valid()) {
                    self.$field = None;
                    mended.push(stringify!($field));
                }
            )*};
        }
        end_unless!(cinder, latch, mossbell, rafterwake, forest_road, pennon, brasswake);
        macro_rules! end_unless_here {
            ($($field:ident),*) => {$(
                if self.$field.as_ref().is_some_and(|s| !s.valid() || s.room >= rooms) {
                    self.$field = None;
                    mended.push(stringify!($field));
                }
            )*};
        }
        end_unless_here!(folly, bats, gantry, powder, hop, race3d);
        for (room, name) in [
            (&mut self.gantry_room, "gantry_room"),
            (&mut self.race3d_room, "race3d_room"),
            (&mut self.folly_room, "folly_room"),
        ] {
            if room.is_some_and(|i| i >= rooms) {
                *room = None;
                mended.push(name);
            }
        }
        if self.light.is_some_and(|(x, y)| !place_ok(x, y)) {
            self.light = None;
            mended.push("light");
        }
        if mend_bodies(self, home) {
            mended.push("place");
        }
        mended
    }

    /// Every way back kept to rooms that exist, with its knights inside the
    /// room it leads to; a hold past its length lets go.
    fn mend_exits(&mut self, rooms: usize) {
        let sizes: Vec<(f32, f32)> = self.dungeon.rooms.iter().map(|r| (r.width(), r.height())).collect();
        let size = |at: usize| sizes[at];
        let inside = |(x, y): (f32, f32), (w, h): (f32, f32)| {
            if x.is_finite() && y.is_finite() {
                (x.clamp(0.0, w), y.clamp(0.0, h))
            } else {
                (w / 2.0, h / 2.0)
            }
        };
        let mend_entry = |e: &mut exits::Entry| -> bool {
            if e.at >= rooms {
                return false;
            }
            let room = size(e.at);
            e.party.retain(|id, _| (1..=MAX_PLAYERS).contains(id));
            for spot in e.party.values_mut() {
                *spot = inside(*spot, room);
            }
            while e.party.len() > MAX_PLAYERS as usize {
                e.party.pop_last();
            }
            true
        };
        if self.exits.entry.as_mut().is_some_and(|e| !mend_entry(e)) {
            self.exits.entry = None;
        }
        self.exits.outside.retain(|id, e| (1..=MAX_PLAYERS).contains(id) && mend_entry(e));
        self.exits.holding.retain(|id, t| (1..=MAX_PLAYERS).contains(id) && *t < exits::LEAVE_TICKS);
    }
}

/// The kingdom's way back kept on its map; its lists kept to their sizes.
fn mend_kingdom(k: &mut KingdomState) {
    let (w, h) = (MAP_W as f32 * TILE_UNITS - 1.0, MAP_H as f32 * TILE_UNITS - 1.0);
    let on_map = |x: f32, y: f32, (fx, fy): (f32, f32)| {
        if x.is_finite() && y.is_finite() { (x.clamp(0.0, w), y.clamp(0.0, h)) } else { (fx, fy) }
    };
    if let Some(r) = k.return_to.as_mut() {
        let (x, y) = on_map(r.x, r.y, (w / 2.0, h / 2.0));
        r.x = x;
        r.y = y;
        r.name.truncate(256);
        r.party.retain(|id, _| (1..=MAX_PLAYERS).contains(id));
        for spot in r.party.values_mut() {
            *spot = on_map(spot.0, spot.1, (x, y));
        }
    }
    k.seen.truncate(128);
    k.class_held.retain(|id| (1..=MAX_PLAYERS).contains(id));
    k.field_held.retain(|id| (1..=MAX_PLAYERS).contains(id));
    if k.wishes.len() > crate::realm::MAX_WISHES {
        std::sync::Arc::make_mut(&mut k.wishes).truncate(crate::realm::MAX_WISHES);
    }
    if !k.valid() {
        // What is left (a wish's art, the moments) cannot be cut to size here:
        // the way back goes, the kingdom's own state stays as it was.
        k.return_to = None;
    }
}

/// Farther than any map (the kingdom is 480 across, a pennon 1200). Past this
/// a coordinate is a damaged save, not a knock into the wall.
const POS_LIMIT: f32 = 8192.0;
/// High enough for any kit and gift, low enough that `hp * 100` fits in `u32`.
const HP_ABS: u32 = 100_000;
const FOE_HP_ABS: u32 = 1_000_000;
const BOON_ABS: u32 = 1_000;

fn place_ok(x: f32, y: f32) -> bool {
    x.is_finite() && y.is_finite() && x.abs() <= POS_LIMIT && y.abs() <= POS_LIMIT
}

fn pull(x: &mut f32, y: &mut f32, home: (f32, f32)) -> bool {
    if place_ok(*x, *y) {
        return false;
    }
    *x = home.0;
    *y = home.1;
    true
}

/// Clamp a restored knight's hp and gifts so a huge number never reaches play.
fn mend_hero_bounds(hero: &mut Hero) -> bool {
    let before = (hero.hp, hero.max_hp, hero.boon.plus, hero.boon.cut);
    hero.boon.plus = hero.boon.plus.min(BOON_ABS);
    hero.boon.cut = hero.boon.cut.min(BOON_ABS);
    if hero.max_hp == 0 || hero.max_hp > HP_ABS {
        hero.max_hp = hero.max_hp.clamp(1, HP_ABS);
    }
    hero.hp = hero.hp.min(HP_ABS);
    hero.fit_hp();
    // A censer pulse is a few dozen. Past this the heal multiply overflows.
    const CENSER_ABS: u32 = 1_000;
    let censer = hero.censer.min(CENSER_ABS);
    let censer_cut = hero.censer != censer;
    hero.censer = censer;
    (hero.hp, hero.max_hp, hero.boon.plus, hero.boon.cut) != before || censer_cut
}

fn mend_foe_hp(enemy: &mut Enemy) -> bool {
    let before = (enemy.hp, enemy.max_hp);
    if enemy.max_hp > FOE_HP_ABS {
        enemy.max_hp = FOE_HP_ABS;
    }
    enemy.hp = enemy.hp.min(FOE_HP_ABS);
    if enemy.max_hp > 0 {
        enemy.hp = enemy.hp.min(enemy.max_hp);
    }
    (enemy.hp, enemy.max_hp) != before
}

/// Pull bodies whose coordinates are nowhere, or far outside any map, back
/// into the room. A foe only a little outside (a knock) is left where it is;
/// the shot grid tolerates that on its own.
fn mend_bodies(run: &mut Run, home: (f32, f32)) -> bool {
    let mut fixed = false;
    for enemy in run.enemies.iter_mut().chain(run.parked.iter_mut()) {
        fixed |= pull(&mut enemy.x, &mut enemy.y, home);
        fixed |= pull(&mut enemy.origin_x, &mut enemy.origin_y, home);
        fixed |= mend_foe_hp(enemy);
    }
    for shot in &mut run.projectiles {
        fixed |= pull(&mut shot.x, &mut shot.y, home);
    }
    for blast in &mut run.blasts {
        fixed |= pull(&mut blast.0, &mut blast.1, home);
    }
    fixed
}

#[cfg(test)]
#[path = "../../tests/rules/mend__tests.rs"]
mod tests;
