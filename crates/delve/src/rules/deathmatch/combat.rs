//! The PvP seam: the Delve's arms and cards, with knights as their targets.
use super::leg;
use super::*;
use cards::Effect;
use serde_json::json;

/// What a knight's arms did this tick, for the leg to settle the ward and the flash.
#[derive(Default)]
struct Acted {
    card: bool,
    bomb: bool,
    swing: bool,
    fire: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldKind {
    Hazard,
    Turret,
    Decoy,
    Hedge,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Field {
    pub owner: u32,
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub damage: u32,
    pub left: u32,
    pub wait: u32,
    pub kind: FieldKind,
}
impl Field {
    pub fn blocks(&self, x: f32, y: f32, r: f32) -> bool {
        if self.kind != FieldKind::Hedge {
            return false;
        }
        let (dx, dy) = ((x - self.x).abs(), (y - self.y).abs());
        // The east gap is a door wide enough for a rolling knight.
        let side = (dx - self.radius).abs() < r + 0.25 && dy < self.radius + r && !(x > self.x && dy < 1.2 - r);
        side || ((dy - self.radius).abs() < r + 0.25 && dx < self.radius + r)
    }
    pub fn valid(&self) -> bool {
        [self.x, self.y, self.radius].iter().all(|n| n.is_finite()) && self.radius > 0. && self.radius <= 20. && self.left <= 20 * HZ
    }
}
impl Session {
    fn visible(&self, a: (f32, f32), b: (f32, f32)) -> bool {
        if !sight(a, b) {
            return false;
        }
        let n = ((b.0 - a.0).hypot(b.1 - a.1) * 5.).ceil().max(1.) as u32;
        (0..=n).all(|i| {
            let t = i as f32 / n as f32;
            !self.fields.iter().any(|f| f.blocks(a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t, 0.))
        })
    }
    pub(super) fn area(&mut self, owner: u32, at: (f32, f32), radius: f32, damage: u32) {
        let targets: Vec<_> = self
            .fighters
            .iter()
            .filter(|(id, f)| **id != owner && f.alive())
            .filter_map(|(&id, f)| {
                let p = (f.hero.x, f.hero.y);
                let d = (p.0 - at.0).hypot(p.1 - at.1);
                (d <= radius && self.visible(at, p)).then_some((d, id, (p.0 - at.0, p.1 - at.1)))
            })
            .collect();
        self.strike_ranked(owner, damage, targets);
    }
    /// Strike the `(distance, id, way)` rivals nearest first, each at its rank's share.
    fn strike_ranked(&mut self, owner: u32, damage: u32, mut targets: Vec<(f32, u32, (f32, f32))>) {
        targets.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for (rank, (_, id, way)) in targets.into_iter().enumerate() {
            self.hit(owner, id, aoe::part(damage, rank), way);
        }
    }
    pub fn orbits(&mut self, owner: u32) {
        let h = &self.fighters[&owner].hero;
        if h.bonus.orbit == 0 {
            return;
        }
        let damage = h.scaled(ORBIT_DAMAGE);
        let swept: Vec<_> = (0..ORBIT_EVERY).flat_map(|back| orbit_points(h, owner, u64::from(self.ticks.saturating_sub(back)))).collect();
        let from = (h.x, h.y);
        let targets: Vec<_> = self
            .fighters
            .iter()
            .filter(|(id, f)| **id != owner && f.alive())
            .filter(|(_, f)| {
                swept.iter().any(|p| (f.hero.x - p.0).hypot(f.hero.y - p.1) <= HERO_RADIUS + 0.8)
                    && self.visible(from, (f.hero.x, f.hero.y))
            })
            .map(|(&id, f)| ((f.hero.x - from.0).hypot(f.hero.y - from.1), id, (f.hero.x - from.0, f.hero.y - from.1)))
            .collect();
        self.strike_ranked(owner, damage, targets);
    }
    fn melee(&mut self, owner: u32, reach: f32, arc: f32, damage: u32) {
        let f = &self.fighters[&owner];
        let h = &f.hero;
        let at = (h.x, h.y);
        let stroke = if h.sword { melee::Stroke::Slash } else if h.unarmed() { melee::Stroke::Fist } else { melee::stroke(h, arc) };
        self.melee.swing(owner, h, stroke, reach, arc);
        let yaw = f.yaw;
        let targets: Vec<_> = self
            .fighters
            .iter()
            .filter(|(id, f)| **id != owner && f.alive())
            .filter_map(|(&id, f)| {
                let (dx, dy) = (f.hero.x - at.0, f.hero.y - at.1);
                let turn = (dy.atan2(dx) - yaw).sin().atan2((dy.atan2(dx) - yaw).cos()).abs();
                (dx.hypot(dy) <= reach + HERO_RADIUS && turn <= arc.to_radians() / 2. && self.visible(at, (f.hero.x, f.hero.y)))
                    .then_some((dx.hypot(dy), id, (dx, dy)))
            })
            .collect();
        self.strike_ranked(owner, damage, targets);
    }
    fn bolt(&mut self, id: u32, kind: Shot, damage: u32, speed: f32, life: u32, burst: u8, stun: u8) {
        let h = &self.fighters[&id].hero;
        self.shots.push(Projectile {
            x: h.x + h.aim_x * 0.6,
            y: h.y + h.aim_y * 0.6,
            vx: h.aim_x * speed,
            vy: h.aim_y * speed,
            hostile: false,
            kind,
            look: None,
            damage,
            pierce: 0,
            last_hit: None,
            ttl: life,
            empowered: false,
            traits: ShotTraits { owner: id as u8, burst, stun, ..ShotTraits::of(&h.bonus) },
        });
    }
    /// A knight's arms for the tick. The leg says whether they may act (`armed`, if the
    /// turn's answer still stands) and what acting does to the spawn ward and the
    /// muzzle flash; the kit does the rest.
    pub(super) fn attack(
        &mut self,
        id: u32,
        input: Input,
        book: &Book,
        seed: u64,
        power_num: u32,
        armed: Option<bool>,
    ) -> Result<(), String> {
        let f = &self.fighters[&id];
        if !(input.cycle || input.cast > 0 || input.play > 0 || input.bomb || input.swing || input.fire || f.hero.spin > 0) {
            return Ok(());
        }
        let arm =
            json!({ "winner": self.winner.is_some(), "connected": f.connected, "hp": f.hero.hp, "respawn": f.respawn, "stun": f.stun });
        let go = match armed {
            Some(go) => go,
            None => leg::ask("arm", json!({ "arm": arm }))? == true,
        };
        if !go {
            return Ok(());
        }
        let a = self.arms(id, input, book, seed, power_num);
        if a.card || a.bomb || a.swing || a.fire {
            let f = &self.fighters[&id];
            let acted =
                json!({ "protection": f.protection, "flash": f.flash, "card": a.card, "bomb": a.bomb, "swing": a.swing, "fire": a.fire });
            let ward = leg::ask("armed_end", json!({ "acted": acted }))?;
            put!(ward => self.fighters.get_mut(&id).unwrap(); protection, flash);
        }
        Ok(())
    }
    fn arms(&mut self, id: u32, input: Input, book: &Book, seed: u64, power_num: u32) -> Acted {
        let mut acted = Acted::default();
        if input.cycle {
            self.fighters.get_mut(&id).unwrap().hero.cycle_hand();
        }
        if let Some(slot) = usize::from(input.cast).checked_sub(1).filter(|&s| s < cards::SPELL_SLOTS) {
            let f = self.fighters.get_mut(&id).unwrap();
            if f.hero.spell_cooldowns[slot] == 0
                && let Some(card) = f.hero.spells[slot].as_deref().and_then(|s| book.get(s))
            {
                f.hero.spell_cooldowns[slot] = f.hero.spell_wait(card);
                self.card(id, card, power_num);
                acted.card = true;
            }
        }
        if let Some(slot) = usize::from(input.play).checked_sub(1) {
            let f = self.fighters.get_mut(&id).unwrap();
            if f.hero.play_cooldown == 0 && slot < f.hero.hand.len() {
                let card = f.hero.hand.remove(slot);
                f.hero.play_cooldown = PLAY_REARM;
                if let Some(card) = book.get(&card) {
                    self.card(id, card, power_num);
                    acted.card = true;
                }
            }
        }
        if input.bomb && self.fighters[&id].hero.bombs > 0 && self.fighters[&id].hero.bomb_cooldown == 0 {
            let h = &mut self.fighters.get_mut(&id).unwrap().hero;
            h.bombs -= 1;
            h.bomb_cooldown = BOMB_REARM;
            let at = (h.x, h.y);
            acted.bomb = true;
            self.area(id, at, BOMB_CLEAR, BOMB_DAMAGE);
        }
        let spin_damage = {
            let h = &self.fighters[&id].hero;
            (h.spin > 0 && spin::sweeping(spin::TICKS.saturating_sub(h.spin + 1))).then(|| h.scaled(26))
        };
        if let Some(damage) = spin_damage {
            let h = &self.fighters[&id].hero;
            self.melee.swing(id,h,melee::Stroke::Chain,spin::REACH,360.0);
            self.area(id, (h.x, h.y), spin::REACH, damage);
        }
        let f = self.fighters.get_mut(&id).unwrap();
        let h = &mut f.hero;
        let guarded = h.rolling().is_some() || h.shielding || h.walling || h.champ.busy();
        if input.swing && h.sword_cooldown == 0 && !guarded {
            h.sword_cooldown = h.cooldown(SWORD_REARM);
            acted.swing = true;
            h.sword = true;
            h.swing = 8;
            let damage = h.scaled(SWORD_DAMAGE);
            self.melee(id, SWORD_REACH, SWORD_ARC, damage);
        }
        let melee = {
            let h = &self.fighters[&id].hero;
            h.forged.as_ref().and_then(|w| {
                w.melee.filter(|m| {
                    (w.bolt.is_none() && w.throw.is_none())
                        || self
                            .fighters
                            .iter()
                            .any(|(&other, f)| other != id && f.alive() && (f.hero.x - h.x).hypot(f.hero.y - h.y) <= m.reach)
                })
            })
        };
        let f = self.fighters.get_mut(&id).unwrap();
        let h = &mut f.hero;
        if !input.fire || h.fire_cooldown > 0 || guarded {
            return acted;
        }
        acted.fire = true;
        let start = self.shots.len();
        if champions::of(h) == Some(champions::Champion::Alchemist) {
            h.fire_cooldown = h.cooldown(champions::FLASK_REARM);
            let damage = h.scaled(champions::FLASK_DAMAGE);
            self.bolt(id, Shot::Ember, damage, 16., 18, 100, 0);
        } else if h.unarmed() {
            h.sword = false;
            h.fire_cooldown = h.cooldown(9);
            let damage = h.scaled(6) * power_num / 2;
            self.melee(id, 1.5, 100., damage);
        } else if let Some(m) = melee {
            h.sword = false;
            h.swing = 6;
            let fury = h.champ.fury > 0;
            h.fire_cooldown = h.cooldown(if fury { m.every / 2 } else { m.every });
            let damage = h.scaled(m.damage) * if fury { 3 } else { 2 } / 2 * power_num / 2;
            self.melee(id, m.reach, m.arc, damage);
        } else {
            let flying = self.shots.iter().filter(|p| u32::from(p.traits.owner) == id).count();
            let f = self.fighters.get_mut(&id).unwrap();
            f.hero.loose(id, book, false, mix(seed ^ self.ticks as u64 * 0x9e37 ^ (id as u64) << 48), flying, &mut self.shots);
        }
        wishcraft::shape(&mut self.shots[start..]);
        for p in &mut self.shots[start..] {
            p.damage = p.damage * power_num / 2;
        }
        self.shots.truncate(MAX_PROJECTILES);
        acted
    }
    fn card(&mut self, id: u32, card: &Card, power_num: u32) {
        let h = &mut self.fighters.get_mut(&id).unwrap().hero;
        let mut score = 0;
        let mut nova = 0;
        h.spend(card, &mut score, &mut nova);
        let at = (h.x, h.y);
        let aim = (h.x + h.aim_x * 6., h.y + h.aim_y * 6.);
        let damage = h.spell_damage(&card.id, 40);
        let start = self.shots.len();
        for effect in &card.effects {
            if let Effect::Cast(shape) = effect {
                h.spell_shots(*shape, &card.id, id, &mut self.shots);
            }
        }
        wishcraft::shape(&mut self.shots[start..]);
        for p in &mut self.shots[start..] {
            p.damage = p.damage * power_num / 2;
        }
        if nova > 0 {
            self.area(id, at, 9., nova);
        }
        for effect in &card.effects {
            match *effect {
                // The colosseum has no ground to burn: an area is one stroke
                // on the rivals it would reach (`zones`).
                Effect::Area(area) => {
                    let (centre, reach, damage) = super::zones::duel_stroke(&area, at, aim, damage);
                    self.area(id, centre, reach, damage);
                }
                Effect::Rip(n) => {
                    self.area(id, aim, 4., n);
                    self.fighters.get_mut(&id).unwrap().hero.mend(n / 2, stakes::Mend::Potion);
                }
                Effect::Cabbages(n) => self.field(id, aim, 3.2, damage, (n.min(8) * 10).max(10), FieldKind::Hazard),
                Effect::Hedge(_) => self.field(id, aim, 3., 0, 5 * HZ, FieldKind::Hedge),
                Effect::Imposture(_) => self.field(id, at, 1., 0, 5 * HZ, FieldKind::Decoy),
                Effect::Summon(n, _) | Effect::Ghoul(n) | Effect::Totem(n) | Effect::Warden(n) => {
                    self.field(id, at, 10., n.max(8), 8 * HZ, FieldKind::Turret)
                }
                _ => {}
            }
        }
        match card.id.as_str() {
            "storm-hammer" => self.bolt(
                id,
                Shot::Sacred,
                self.fighters[&id].hero.spell_damage(&card.id, champions::HAMMER_DAMAGE),
                champions::HAMMER_SPEED,
                champions::HAMMER_LIFE,
                100,
                30,
            ),
            "groundslam" => {
                self.fighters.get_mut(&id).unwrap().hero.champ.windup = champions::SLAM_WINDUP;
            }
            "brute-force" => self.fighters.get_mut(&id).unwrap().hero.champ.fury = self.fighters[&id].hero.spell_step(&card.id).fury,
            "meat-hook" => self.bolt(
                id,
                Shot::Blade,
                self.fighters[&id].hero.spell_damage(&card.id, champions::HOOK_DAMAGE),
                champions::HOOK_SPEED,
                18,
                0,
                30,
            ),
            "rot" => self.fighters.get_mut(&id).unwrap().hero.champ.rot = self.fighters[&id].hero.spell_step(&card.id).rot,
            "chain-haul" => {
                let h = &mut self.fighters.get_mut(&id).unwrap().hero;
                let reach = h.spell_step(&card.id).haul_reach;
                (h.x, h.y) = grid().halted((h.x, h.y), (h.aim_x * reach, h.aim_y * reach));
            }
            "miasma-flask" => self.field(id, aim, 3., champions::FOUL_POWER, 5 * HZ, FieldKind::Hazard),
            "quickfire" => self.field(
                id,
                aim,
                self.fighters[&id].hero.spell_step(&card.id).quick_reach,
                self.fighters[&id].hero.spell_damage(&card.id, champions::QUICK_DAMAGE),
                self.fighters[&id].hero.spell_step(&card.id).quick_life,
                FieldKind::Hazard,
            ),
            "elixir" => {
                let mend = self.fighters[&id].hero.spell_power(&card.id, champions::ELIXIR_MEND);
                self.fighters.get_mut(&id).unwrap().hero.mend(mend, stakes::Mend::Potion);
            }
            _ => {}
        }
        self.shots.truncate(MAX_PROJECTILES);
    }
    fn field(&mut self, owner: u32, at: (f32, f32), radius: f32, damage: u32, left: u32, kind: FieldKind) {
        if self.fields.len() < 32 && open(at.0, at.1) && sight((self.fighters[&owner].hero.x, self.fighters[&owner].hero.y), at) {
            self.fields.push(Field { owner, x: at.0, y: at.1, radius, damage, left, wait: 0, kind });
        }
    }
    pub(super) fn fly(&mut self) -> Result<(), String> {
        let mut shots = std::mem::take(&mut self.shots);
        let mut children = Vec::new();
        let flown = self.fly_over(&mut shots, &mut children);
        for p in shots.iter().filter(|p| p.ttl == 0 && p.kind == Shot::Ember) {
            let owner = u32::from(p.traits.owner);
            if self.fighters.get(&owner).is_some_and(|f| champions::of(&f.hero) == Some(champions::Champion::Alchemist)) {
                self.area(owner, (p.x, p.y), champions::FLASK_BURST, p.damage);
                self.field(owner, (p.x, p.y), champions::FLASK_FIRE, p.damage / 4, champions::FLASK_FIRE_LIFE, FieldKind::Hazard);
            }
        }
        shots.retain(|p| p.ttl > 0);
        shots.extend(children.into_iter().take(MAX_PROJECTILES.saturating_sub(shots.len())));
        self.shots = shots;
        flown
    }
    fn fly_over(&mut self, shots: &mut Vec<Projectile>, children: &mut Vec<Projectile>) -> Result<(), String> {
        let orbits: Vec<_> = self
            .fighters
            .iter()
            .filter(|(_, f)| f.alive() && f.protection == 0)
            .flat_map(|(&id, f)| orbit_points(&f.hero, id, u64::from(self.ticks)).into_iter().map(move |p| (id, p)))
            .collect();
        for p in shots.iter_mut() {
            if p.ttl == 0 || self.winner.is_some() {
                continue;
            }
            p.ttl -= 1;
            let owner = u32::from(p.traits.owner);
            let target = self
                .fighters
                .iter()
                .filter(|(id, f)| **id != owner && f.alive() && f.protection == 0)
                .filter(|(_, f)| sight((p.x, p.y), (f.hero.x, f.hero.y)))
                .min_by(|a, b| (a.1.hero.x - p.x).hypot(a.1.hero.y - p.y).total_cmp(&(b.1.hero.x - p.x).hypot(b.1.hero.y - p.y)));
            if p.traits.back == 1 {
                p.traits.out = p.traits.out.saturating_sub(1);
                if p.traits.out == 0 {
                    p.traits.back = 2;
                    p.last_hit = None;
                }
            }
            if p.traits.back == 2 {
                if let Some(f) = self.fighters.get(&owner) {
                    let d = (f.hero.x - p.x).hypot(f.hero.y - p.y);
                    if d < 0.7 {
                        p.ttl = 0;
                        continue;
                    }
                    let speed = p.vx.hypot(p.vy);
                    (p.vx, p.vy) = ((f.hero.x - p.x) / d * speed, (f.hero.y - p.y) / d * speed);
                }
            } else if p.traits.homing > 0
                && let Some((_, f)) = target
            {
                let mut aim = (f.hero.x, f.hero.y);
                for decoy in self.fields.iter().filter(|f| f.kind == FieldKind::Decoy && f.owner != owner) {
                    if (decoy.x - p.x).hypot(decoy.y - p.y) < (aim.0 - p.x).hypot(aim.1 - p.y) {
                        aim = (decoy.x, decoy.y);
                    }
                }
                let now = p.vy.atan2(p.vx);
                let want = (aim.1 - p.y).atan2(aim.0 - p.x);
                let turn = (want - now).sin().atan2((want - now).cos());
                let a = now + turn.clamp(-HOMING_TURN * p.traits.homing as f32 * DT, HOMING_TURN * p.traits.homing as f32 * DT);
                let speed = p.vx.hypot(p.vy);
                (p.vx, p.vy) = (a.cos() * speed, a.sin() * speed);
            }
            let from = (p.x, p.y);
            let to = (p.x + p.vx * DT, p.y + p.vy * DT);
            if orbits.iter().any(|(id, at)| *id != owner && shield::entry(from, to, *at, 0.9).is_some()) {
                p.ttl = 0;
                continue;
            }
            let stone = grid().shot_stop_body(from, to, 0.10).or_else(|| {
                let n = ((to.0 - from.0).hypot(to.1 - from.1) * 8.).ceil().max(1.) as u32;
                (1..=n).find_map(|i| {
                    let t = i as f32 / n as f32;
                    let p = (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
                    self.fields.iter().any(|f| f.blocks(p.0, p.1, 0.1)).then_some(p)
                })
            });
            let decoy_t = self
                .fields
                .iter()
                .filter(|f| f.kind == FieldKind::Decoy && f.owner != owner)
                .filter_map(|f| shield::entry(from, to, (f.x, f.y), 0.5))
                .min_by(f32::total_cmp)
                .unwrap_or(2.);
            let wall_t = stone.map_or(2., |at| (at.0 - from.0).hypot(at.1 - from.1) / (to.0 - from.0).hypot(to.1 - from.1).max(0.001));
            let stone_t = wall_t.min(decoy_t);
            let mut hits: Vec<_> = self
                .fighters
                .iter()
                .filter(|(id, f)| **id != owner && f.alive() && f.protection == 0 && p.last_hit != Some(**id))
                .filter_map(|(&id, f)| {
                    shield::meets(
                        &f.hero,
                        (f.hero.x, f.hero.y),
                        from,
                        to,
                        HERO_RADIUS + 0.16 + f32::from(p.traits.size) * wishcraft::SIZE_REACH,
                    )
                    .map(|m| (m.t, id, m.shield))
                })
                .filter(|(t, _, _)| *t < stone_t)
                .collect();
            hits.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            for (t, id, blocked) in hits {
                if self.fields.iter().any(|f| {
                    f.kind == FieldKind::Decoy && f.owner != owner && shield::entry(from, to, (f.x, f.y), 0.5).is_some_and(|d| d < t)
                }) {
                    p.ttl = 0;
                    break;
                }
                if blocked {
                    self.fighters.get_mut(&id).unwrap().hero.shield_turned(p.damage);
                    p.ttl = 0;
                    break;
                }
                let at = (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
                if self.hit(owner, id, aoe::part(p.damage, p.traits.struck as usize), (p.vx, p.vy)) {
                    p.traits.struck = p.traits.struck.saturating_add(1);
                    let hook = p.kind == Shot::Blade
                        && p.traits.stun > 0
                        && champions::of(&self.fighters[&owner].hero) == Some(champions::Champion::Butcher);
                    let caster = (self.fighters[&owner].hero.x, self.fighters[&owner].hero.y);
                    // What the hit adds (stun, chill, burn, burst, chain, leech, cloud) is the leg's.
                    let (target, from_owner) = (&self.fighters[&id], &self.fighters[&owner]);
                    let land = json!({ "stun_t": p.traits.stun, "chill_t": p.traits.chill, "chill_unit": lich::CHILL, "fire": p.traits.fire, "poison": p.traits.poison, "burst": p.traits.burst, "chain": p.traits.chain, "mend": p.traits.mend, "fart": p.traits.fart, "damage": p.damage, "vamp": from_owner.hero.bonus.vamp, "owner_alive": from_owner.alive(), "fart_wait": from_owner.fart_wait, "stun": target.stun, "chilled": target.hero.chilled });
                    let l = leg::ask("landed", json!({ "land": land }))?;
                    let f = self.fighters.get_mut(&id).unwrap();
                    put!(l => f; stun);
                    put!(l => f.hero; chilled);
                    if l["burning"] == true {
                        (f.burn, f.burner) = (leg::num(&l["burn"]), owner);
                    }
                    let knock = wishcraft::knock(p);
                    let (dx, dy) = unit(p.vx, p.vy);
                    (f.hero.x, f.hero.y) = grid().halted((f.hero.x, f.hero.y), (dx * knock, dy * knock));
                    if hook {
                        let (dx, dy) = (caster.0 - f.hero.x, caster.1 - f.hero.y);
                        let dist = dx.hypot(dy).max(0.01);
                        let pull = (dist - 1.4).max(0.);
                        (f.hero.x, f.hero.y) = grid().halted((f.hero.x, f.hero.y), (dx / dist * pull, dy / dist * pull));
                    }
                    if l["bursts"] == true {
                        self.area(owner, at, 2.8, leg::num(&l["burst_damage"]));
                    }
                    let near = |other: &u32, f: &Fighter| {
                        *other != owner
                            && *other != id
                            && f.alive()
                            && (f.hero.x - at.0).hypot(f.hero.y - at.1) < 5.
                            && self.visible(at, (f.hero.x, f.hero.y))
                    };
                    let chain: Vec<u32> =
                        self.fighters.iter().filter(|(o, f)| near(o, f)).take(leg::num(&l["chain"]) as usize).map(|(&id, _)| id).collect();
                    for id in chain {
                        self.hit(owner, id, leg::num(&l["chain_damage"]), (p.vx, p.vy));
                    }
                    if l["leech"] == true
                        && let Some(f) = self.fighters.get_mut(&owner).filter(|f| f.alive())
                    {
                        f.hero.leech(leg::num(&l["leech_amount"]), stakes::Mend::Passive);
                    }
                    if l["fart"] == true {
                        self.fighters.get_mut(&owner).unwrap().fart_wait = leg::num(&l["fart_wait"]);
                        let damage = self.fighters[&owner].hero.scaled(skills::POISON_BASE);
                        self.field(
                            owner,
                            at,
                            l["fart_reach"].as_f64().unwrap_or(2.) as f32,
                            damage,
                            leg::num(&l["fart_left"]),
                            FieldKind::Hazard,
                        );
                    }
                }
                p.last_hit = Some(id);
                if p.pierce == 0 {
                    if wishcraft::turn_home(p) {
                        break;
                    }
                    if p.traits.fork > 0 && p.traits.back == 0 {
                        for turn in [-FORK_TURN, FORK_TURN] {
                            let mut child = p.clone();
                            let (s, c) = turn.sin_cos();
                            (child.vx, child.vy) = (p.vx * c - p.vy * s, p.vx * s + p.vy * c);
                            (child.x, child.y) = at;
                            child.traits.fork -= 1;
                            child.ttl = p.ttl.max(FORK_TTL);
                            if children.len() < MAX_PROJECTILES {
                                children.push(child);
                            }
                        }
                    }
                    p.ttl = 0;
                    break;
                }
                p.pierce -= 1;
            }
            if p.ttl == 0 {
                continue;
            }
            if decoy_t < wall_t {
                p.ttl = 0;
                continue;
            }
            if let Some(at) = stone {
                if p.traits.back == 2 {
                    p.ttl = 0;
                } else if p.traits.bounce > 0 {
                    p.traits.bounce -= 1;
                    if grid().shot_stop(from, (to.0, from.1)).is_some() {
                        p.vx = -p.vx;
                    }
                    if grid().shot_stop(from, (from.0, to.1)).is_some() {
                        p.vy = -p.vy;
                    }
                } else {
                    let _ = at;
                    p.ttl = 0;
                }
            } else {
                (p.x, p.y) = to;
            }
        }
        Ok(())
    }
}
