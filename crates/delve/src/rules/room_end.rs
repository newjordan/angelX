//! The room's end on a tick (`Run::play`): the party falls, an open map's
//! chambers fight on, or the room is cleared, paid and opened.

use super::*;

impl Run {
    /// The room's end on a tick: a raid's rout, the party's fall, an
    /// open map's chambers, or the room cleared and its rewards; then
    /// the way on (`travel`).
    pub(super) fn room_end(&mut self) {
        // Beaten by raiders in a castle's hall at home: knocked down, not
        // killed.
        if self.raid_rout() {
            return;
        }
        // Beaten by a toll gang on the road: knocked down, not killed.
        if self.wayfarer_rout() {
            return;
        }
        // A party falls only when every knight someone plays has fallen. A
        // knight turned to stone because its player is away is still in the
        // run, waiting for them, and keeps it up: one knight's fall never
        // ends a run while a friend's knight stands (the fallen one's skull
        // waits to be carried). Only a seat nobody plays (`absent`, a host
        // with no knight of their own) is left out.
        if self.players.values().filter(|h| !h.absent).all(|h| h.hp == 0)
            && self.players.values().any(|h| h.hp == 0)
        {
            self.phase = Phase::Wiped;
            self.close_ledger("wipe");
            self.projectiles.clear();
            self.cues.push("wipe".into());
            self.wishes_home(false);
            self.bank_all(false, "the party fell");
            for id in self.players.keys().copied().collect::<Vec<_>>() { self.restore_gear(id); }
            return;
        }
        // An open map's fights are its chambers' (`depths`).
        if self.open_map() {
            self.tick_open();
            if self.phase == Phase::Exploring {
                self.travel();
            }
            return;
        }
        if !self.foes_left() && self.phase == Phase::Fighting {
            self.heal_lost_carrier();
        }
        if !self.foes_left()
            && self.phase == Phase::Fighting
            && !self.waves.pending()
            && self.boss_room_can_clear()
            && !self.folly_running()
        {
            let first_clear = !self.room().cleared;
            if first_clear {
                self.curios_clear();
            }
            // The monsters' shots still in the air turn to gold.
            self.cancel_shots();
            self.close_ledger("clear");
            self.projectiles.clear();
            self.rocks.clear();
            self.ravages.clear();
            self.holes.clear();
            self.dungeon.rooms[self.at].cleared = true;
            // For King Brannoc's missions: a hall of a delve freed.
            if self.dungeon.depth > 0 {
                *self
                    .marks
                    .entry(barony::clear_mark(self.dungeon.pack))
                    .or_default() += 1;
            }
            self.fair_chest();
            // Two knights standing together earn the realm a bond: one a
            // room for the party, carried by the first of them.
            let standing = self
                .players
                .values()
                .filter(|h| h.hp > 0 && !h.stone)
                .count();
            if standing >= 2 && first_clear {
                if let Some(hero) = self.players.values_mut().find(|h| h.hp > 0 && !h.stone) {
                    hero.carried.add(Spoil::Bond, 1);
                }
                self.cues.push("bond".into());
                self.notice("bonded");
            }
            if !self.room_hurt {
                self.notice("flawless");
            }
            self.cues.push(
                if self.room_hurt {
                    "room_clear"
                } else {
                    "flawless"
                }
                .into(),
            );
            self.sounds.push("door_open");
            // The floor's boss fell here: a wish window, while the floor
            // has one (`overclass`). Asked before the stairs become a well.
            let boss = self.floor_boss_here();
            // The first floor's guardian guarded a Sanctuary: every run
            // reaches a reforge before the descent.
            if self.room().kind == RoomKind::Stairs && self.dungeon.depth == 1 {
                self.dungeon.rooms[self.at].kind = RoomKind::Sanctuary;
                self.stairs_held = true;
                self.hallow();
            }
            if self.room().kind == RoomKind::Lair {
                // The dragon is slain: its floor reclaimed and banked, and
                // the way splits — the stairs into the deep, or the light home.
                if let Some(hero) = self.players.values_mut().find(|h| h.hp > 0 && !h.stone) {
                    hero.carried.add(Spoil::Scale, 1);
                }
                self.reclaimed.push(self.dungeon.pack);
                self.cues.push("victory".into());
                self.fx.push(Fx::Shake(36));
                self.sounds.push("boss_fall");
                self.bank_all(true, "the dragon is slain");
                self.triumph = Some(Triumph::Dragon);
                self.notice("dragonslayer");
                self.unwashed_check();
                let room = &mut self.dungeon.rooms[self.at];
                room.open_stairs();
                self.light = Some((room.width() / 2.0, room.height() / 2.0 - 4.0));
                self.raise_hell_gate();
                self.stairs_held = true;
                self.cues.push("the_deep".into());
            }
            if self.in_mire_kings_hall() {
                // The Mire King is fallen: the pen opens, and once the
                // white horse is out, the light leads home.
                self.mire_king_falls();
            }
            if self.room().kind == RoomKind::Threshold {
                // The Unknown met: the Grail, and home.
                self.phase = Phase::Won;
                for hero in self.players.values_mut().filter(|h| h.hp > 0) {
                    hero.carried.add(Spoil::Gem, 3);
                    hero.carried.add(Spoil::Scale, 1);
                }
                self.reclaimed.push(self.dungeon.pack);
                self.cues.push("grail".into());
                self.fx.push(Fx::Shake(24));
                self.sounds.push("wheel_land");
                self.wishes_home(true);
                self.bank_all(true, "the Grail");
                self.triumph = Some(Triumph::Grail);
                self.notice("the_grail");
                self.unwashed_check();
                return;
            }
            self.phase = Phase::Exploring;
            if boss {
                // The floor's boss fell: a super wish each (`overclass`).
                self.boss_wishes();
                // And the cauldron rises by the stairs (`cauldron`).
                self.raise_cauldron();
            }
        }
        if self.phase == Phase::Exploring {
            self.travel();
        }
    }
}
