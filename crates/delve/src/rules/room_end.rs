//! The room's end on a tick (`Run::play`): the party falls, an open map's
//! chambers fight on, or the room is cleared, paid and opened.

use super::*;

impl Run {
    pub(super) fn room_end(&mut self) {
        if self.raid_rout() {
            return;
        }
        if self.wayfarer_rout() {
            return;
        }
        if self.deep_rout() {
            return;
        }
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
            self.cancel_shots();
            self.close_ledger("clear");
            self.projectiles.clear();
            self.rocks.clear();
            self.ravages.clear();
            self.holes.clear();
            self.dungeon.rooms[self.at].cleared = true;
            if self.dungeon.depth > 0 {
                *self
                    .marks
                    .entry(barony::clear_mark(self.dungeon.pack))
                    .or_default() += 1;
            }
            self.fair_chest();
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
            let boss = self.floor_boss_here();
            if self.room().kind == RoomKind::Stairs && self.dungeon.depth == 1 {
                self.dungeon.rooms[self.at].kind = RoomKind::Sanctuary;
                self.stairs_held = true;
                self.hallow();
            }
            if self.room().kind == RoomKind::Lair {
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
                self.mire_king_falls();
            }
            if self.room().kind == RoomKind::Threshold {
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
                self.boss_wishes();
                self.raise_cauldron();
            }
        }
        if self.phase == Phase::Exploring {
            self.travel();
        }
    }
}
