//! The knights' wounds on a tick (`Run::play`): what touched them, who fell,
//! who was mended, and the guardians' rage.

use super::*;

impl Run {
    /// The knights' wounds: a monster's touch, the fallen marked, vigils
    /// and second winds, and what each knight lost this tick (from
    /// `before`) told; a guardian's rage and its beats (`boss_beats`).
    pub(super) fn tally_wounds(
        &mut self,
        before: Vec<(u32, u32)>,
        boss_beats: Vec<(u32, bool, u32, u32)>,
    ) {
        for hero in self.players.values_mut().filter(|h| h.hp > 0 && !h.stone) {
            let touch = self
                .enemies
                .iter()
                .filter(|e| {
                    e.hp > 0
                        && e.age >= TELEGRAPH
                        && !e.dormant
                        && e.frozen == 0
                        && !e.asleep(self.tick)
                        && e.kind != EnemyKind::Dummy
                        && e.kind != EnemyKind::Goblin
                        && e.hurts_by_touch()
                        && (hero.x - e.x).hypot(hero.y - e.y) < e.radius() + 0.5
                })
                .map(Enemy::touch)
                .max();
            if let Some(damage) = touch {
                hero.attack_hurt(damage);
            }
        }
        // A knight keeping vigil mends the living knights within its reach.
        if self.tick.is_multiple_of(u64::from(VIGIL_MEND_EVERY)) {
            let wards: Vec<(f32, f32)> = self
                .players
                .values()
                .filter(|h| h.vigil && h.hp > 0)
                .map(|h| (h.x, h.y))
                .collect();
            for hero in self.players.values_mut() {
                if hero.hp > 0
                    && hero.hp < hero.max_hp
                    && !hero.vigil
                    && wards
                        .iter()
                        .any(|&(x, y)| (hero.x - x).hypot(hero.y - y) < VIGIL_REACH)
                {
                    hero.mend(1, stakes::Mend::Passive);
                }
            }
        }
        self.second_winds();
        for hero in self.players.values_mut() {
            hero.dress();
        }
        self.tend_furnace();
        self.tend_court();
        self.mark_hits();
        for ((was, max), (&id, hero)) in before.into_iter().zip(self.players.iter()) {
            if hero.hp < was {
                // The floor's tally: the blow taken, and a fall.
                if self.dungeon.depth > 0 {
                    let line = self.floor_tally.line(id);
                    line.taken = line.taken.saturating_add(was - hero.hp);
                    line.falls += u32::from(hero.hp == 0);
                }
                self.room_hurt = true;
                if hero.hp == 0 {
                    self.cues.push("knight_down".into());
                } else if was * 10 >= max * 3 && hero.hp * 10 < max * 3 {
                    self.cues.push("low_hp".into());
                }
            }
        }
        for enemy in self.enemies.iter_mut().filter(|e| !e.raged && e.hp > 0) {
            if let Some(boss) = enemy.boss.and_then(|i| self.bosses.get(usize::from(i)))
                && boss.rage > 0.0
                && (enemy.hp as f32) < boss.rage * enemy.max_hp as f32
            {
                enemy.raged = true;
                self.cues.push(format!("boss_rage:{}", boss.id));
            }
        }
        for (id, raged, frozen, phase) in boss_beats {
            if let Some(e) = self.enemies.iter().find(|e| e.id == id && e.hp > 0)
                && ((!raged && e.raged) || (frozen == 0 && e.frozen > 0) || self.boss_phase(e) != phase)
            {
                self.hitstop.request(&self.tune, hitstop::Impact::Phase);
                // Staggers keep their existing hit response; a new pattern gets
                // its own quiet sign, without adding shake or another pause.
                let enraged = !raged && e.raged;
                if enraged || self.boss_phase(e) != phase {
                    self.spectacle.phase = Some(spectacle::PhaseMark {
                        x: e.x, y: e.y, born: self.tick, room: self.at, rage: enraged,
                    });
                    if !enraged { self.sounds.push("boss_roar"); }
                }
            }
        }
    }
}
