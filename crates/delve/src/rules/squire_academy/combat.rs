//! Companion tactics, all tie-broken by the existing ordered player/enemy lists.
use super::*;

impl Run {
    fn squire_target(&self, body: &Hireling, reach: f32) -> Option<usize> {
        self.enemies.iter().enumerate()
            .filter(|(_, e)| e.hp > 0 && e.age >= TELEGRAPH && e.kind != EnemyKind::Dummy && !e.asleep(self.tick)
                && (e.x - body.x).hypot(e.y - body.y) <= reach)
            .min_by(|(_, a), (_, b)| (a.x - body.x).hypot(a.y - body.y).total_cmp(&(b.x - body.x).hypot(b.y - body.y)))
            .map(|(i, _)| i)
    }
    fn academy_squire_hit(&mut self, body: &Hireling, i: usize, damage: u32) {
        let foe = &mut self.enemies[i];
        let dealt = damage.min(foe.hp);
        foe.hp -= dealt;
        if let Some(owner) = body.summoner {
            foe.wits.foe = owner;
            self.feel.credit(owner, foe.id, dealt, self.tick);
        }
    }
    fn academy_squire_shot(&mut self, body: &Hireling, i: usize, damage: u32) {
        if self.projectiles.len() >= MAX_PROJECTILES { return; }
        let foe = &self.enemies[i];
        let (ux, uy) = unit(foe.x - body.x, foe.y - body.y);
        let mut shot = Projectile {
            x: body.x, y: body.y, vx: ux * 18., vy: uy * 18., hostile: false,
            look: None, kind: Shot::Blade, damage, pierce: 0, last_hit: None,
            empowered: false, traits: Default::default(), ttl: 2 * HZ,
        };
        shot.traits.owner = body.summoner.unwrap_or(0) as u8;
        self.projectiles.push(shot);
    }
    pub(in crate::rules) fn squire_attack(&mut self, body: &mut Hireling) {
        let Some(kind) = body.kind.squire() else { return; };
        let level = body.power.clamp(1, u32::from(MAX_LEVEL)) as u8;
        let stats = kind.stats(level, self.home.squire_academy.yard);
        match kind {
            Kind::Healer => {
                // Lowest health fraction, not nearest or lowest absolute HP.
                let target = self.players.iter().filter(|(_, h)| h.hp > 0 && !h.absent && !h.stone
                    && (h.hp < h.max_hp || (level >= 2 && (h.hexed > 0 || h.chilled > 0 || h.mired > 0)))
                    && (h.x - body.x).hypot(h.y - body.y) <= stats.reach)
                    .min_by(|(ia, a), (ib, b)| (u64::from(a.hp) * u64::from(b.max_hp)).cmp(&(u64::from(b.hp) * u64::from(a.max_hp))).then(ia.cmp(ib)))
                    .map(|(&id, _)| id);
                if let Some(id) = target {
                    let h = self.players.get_mut(&id).unwrap();
                    h.mend(stats.damage, super::super::stakes::Mend::Party);
                    if level >= 2 { h.hexed = 0; h.chilled = 0; h.mired = 0; }
                    body.cooldown = stats.every;
                    self.cues.push("squire_heal".into());
                }
            }
            Kind::Ninja => {
                if let Some(i) = self.squire_target(body, stats.reach) {
                    let e = &self.enemies[i];
                    // Flank is relative to the knight, never a random teleport.
                    let backstab = body.summoner.and_then(|id| self.players.get(&id))
                        .is_some_and(|h| (h.x - e.x) * (body.x - e.x) + (h.y - e.y) * (body.y - e.y) < 0.);
                    self.academy_squire_hit(body, i, stats.damage * if backstab { 2 } else { 1 });
                    body.cooldown = stats.every;
                    self.cues.push(if backstab { "squire_backstab" } else { "squire_blade" }.into());
                } else if level >= 2 {
                    if let Some(i) = self.squire_target(body, 12.) {
                        self.academy_squire_shot(body, i, stats.damage / 2);
                        body.cooldown = stats.every;
                        self.cues.push("squire_star".into());
                    }
                }
            }
            Kind::Ranger => {
                if let Some(i) = self.squire_target(body, stats.reach) {
                    let id = self.enemies[i].id;
                    let marked = self.squire_academy.marks.get(&id).is_some_and(|(owner, _)| Some(*owner) == body.summoner);
                    self.academy_squire_shot(body, i, stats.damage + if marked { stats.damage / 2 } else { 0 });
                    if level >= 2 && self.squire_academy.marks.len() < 512 { self.squire_academy.marks.insert(id, (body.summoner.unwrap_or(1), 5 * HZ)); }
                    body.cooldown = stats.every;
                    self.cues.push("squire_arrow".into());
                }
            }
            Kind::ShieldBearer => {
                if let Some(i) = self.squire_target(body, stats.reach) {
                    self.academy_squire_hit(body, i, stats.damage);
                    if level >= 2 { body.taunt = HZ; }
                    body.cooldown = stats.every;
                }
            }
            Kind::Alchemist => {
                if let Some(i) = self.squire_target(body, stats.reach) {
                    let (x, y) = (self.enemies[i].x, self.enemies[i].y);
                    // Bombs do not stun bosses or rewrite their movement.
                    let victims: Vec<_> = self.enemies.iter().enumerate().filter(|(_, e)| e.hp > 0 && e.age >= TELEGRAPH && e.kind != EnemyKind::Dummy
                        && (e.x - x).hypot(e.y - y) < 2.8).map(|(i, _)| i).collect();
                    for i in victims { self.academy_squire_hit(body, i, stats.damage); }
                    if self.blasts.len() < 16 { self.blasts.push((x, y, BLAST_TICKS)); }
                    body.cooldown = stats.every;
                    self.cues.push("squire_bomb".into());
                }
            }
        }
    }
    pub(super) fn squire_special(&mut self, c: &mut Companion) {
        let Some(kind) = c.body.kind.squire() else { return; };
        if kind == Kind::Healer && c.level >= 3 && c.special == 0 {
            // Assist a skull where it lies, or in a friend's hands. Never steal
            // it into a separate inventory, and clear a carrier on revival.
            let target = self.players.iter().filter(|(_, h)| h.hp == 0 && !h.absent && h.fell > 0
                && self.tick >= h.fell.saturating_add(super::super::skulls::SKULL_SETTLE))
                .find_map(|(&id, h)| {
                    let at = self.skull_carrier(id).and_then(|carrier| self.players.get(&carrier).map(|h| (h.x, h.y)))
                        .or_else(|| self.skull_lying(id)).unwrap_or((h.x, h.y));
                    ((at.0 - c.body.x).hypot(at.1 - c.body.y) < 2.5).then_some(id)
                });
            if let Some(id) = target {
                let ticks = c.reviving.filter(|(who, _)| *who == id).map_or(1, |(_, ticks)| ticks + 1);
                c.reviving = Some((id, ticks));
                if ticks >= 3 * HZ {
                    let at = self.skull_carrier(id).and_then(|carrier| self.players.get(&carrier).map(|h| (h.x, h.y)))
                        .or_else(|| self.skull_lying(id));
                    let h = self.players.get_mut(&id).unwrap();
                    if let Some(at) = at { (h.x, h.y) = at; }
                    h.hp = (h.max_hp / 4).max(1); h.fell = 0; h.skull_at = None; h.possessed = None;
                    h.invulnerable = 2 * HZ; h.hexed = 0; h.chilled = 0; h.mired = 0; h.dress();
                    for h in self.players.values_mut() { if h.carrying_skull == Some(id) { h.carrying_skull = None; } }
                    c.special = 20 * HZ; c.reviving = None;
                    self.cues.push("squire_revive".into());
                }
            } else { c.reviving = None; }
        } else if c.special == 0 && kind == Kind::Ninja && c.level >= 3 && c.body.hp * 2 < c.body.max_hp {
            // Smoke is a short safe retreat, not permanent stealth.
            c.body.invulnerable = c.body.invulnerable.max(HZ);
            c.special = 8 * HZ;
            self.cues.push("squire_smoke".into());
        } else if c.special == 0 && kind == Kind::Alchemist && c.level >= 2 {
            if let Some(h) = self.players.values_mut().filter(|h| h.hp > 0 && !h.absent && h.hp < h.max_hp
                && (h.x - c.body.x).hypot(h.y - c.body.y) <= 8.)
                .min_by_key(|h| h.hp * 100 / h.max_hp.max(1)) {
                h.mend(16 + 4 * u32::from(c.level), super::super::stakes::Mend::Potion);
                c.special = 6 * HZ;
                self.cues.push("squire_potion".into());
            }
        }
    }
    /// Adjust existing collision-safe hireling movement for class tactics.
    pub(in crate::rules) fn squire_way(&self, body: &Hireling, way: (f32, f32)) -> (f32, f32) {
        let Some(kind) = body.kind.squire() else { return way; };
        if kind == Kind::Ninja {
            if let Some(i) = self.squire_target(body, 16.) {
                let e = &self.enemies[i];
                if let Some(h) = body.summoner.and_then(|id| self.players.get(&id)) {
                    let (ux, uy) = unit(e.x - h.x, e.y - h.y);
                    let (dx, dy) = (e.x + ux * 1.4 - body.x, e.y + uy * 1.4 - body.y);
                    if dx.hypot(dy) > 0.3 { return unit(dx, dy); }
                    return (0., 0.);
                }
            }
        }
        if kind == Kind::Healer && body.power >= 3 {
            if let Some((_, (x, y))) = self.players.iter().filter(|(_, h)| h.hp == 0 && !h.absent)
                .filter_map(|(&id, _)| self.skull_carrier(id).and_then(|carrier| self.players.get(&carrier).map(|h| (id, (h.x, h.y))))
                    .or_else(|| self.skull_lying(id).map(|at| (id, at))))
                .min_by(|(_, a), (_, b)| (a.0 - body.x).hypot(a.1 - body.y).total_cmp(&(b.0 - body.x).hypot(b.1 - body.y))) {
                return if (x - body.x).hypot(y - body.y) > 1.8 { unit(x - body.x, y - body.y) } else { (0., 0.) };
            }
        }
        // Ranger keeps a longer firing lane; healers stay close to their owner.
        if kind == Kind::Ranger {
            if let Some(i) = self.squire_target(body, 22.) {
                let e = &self.enemies[i]; let d = (e.x - body.x).hypot(e.y - body.y);
                let (ux, uy) = unit(e.x - body.x, e.y - body.y);
                return if d < 10. { (-ux, -uy) } else if d > 18. { (ux, uy) } else { (0., 0.) };
            }
        }
        way
    }
}
