//! The native Knights Wish rules, exactly as `crates/delve/src/rules/deathmatch.rs`
//! and `deathmatch/combat.rs` ran them before the match moved into the Bend leg
//! `deathmatch-rules`. They are the oracle of the parity tests: the same
//! bodies, the same order, with every method renamed `native_*` so they sit
//! beside the leg-driven ones on the same `Session` and `Run`. Nothing here is
//! product code.
use angel_delve::realm::{Spoil, Spoils};
use angel_delve::rules::cards::Effect;
use angel_delve::rules::deathmatch::{
    Field, FieldKind, Fighter, Frag, PICKUPS, Pickup, Session, grid, *,
};
use angel_delve::rules::{
    BOMB_CLEAR, BOMB_DAMAGE, BOMB_REARM, Book, Card, DT, FORK_TTL, FORK_TURN, HERO_RADIUS,
    HOMING_TURN, HZ, Input, MAX_ARMOR, MAX_PROJECTILES, ORBIT_DAMAGE, ORBIT_EVERY, PLAY_REARM,
    Projectile, Run, SWORD_ARC, SWORD_DAMAGE, SWORD_REACH, SWORD_REARM, Shot, ShotTraits, Weapon,
    aoe, cards, champions, classes, lich, melee, mix, orbit_points, shield, skills, spin, stakes,
    unit, wishcraft,
};
use std::collections::BTreeMap;

pub trait NativeSession {
    fn native_spawn(&mut self, id: u32);
    fn native_step(&mut self, inputs: &BTreeMap<u32, Input>, book: &Book, seed: u64);
    fn native_hit(&mut self, owner: u32, target: u32, damage: u32, way: (f32, f32)) -> bool;
    fn native_visible(&self, a: (f32, f32), b: (f32, f32)) -> bool;
    fn native_area(&mut self, owner: u32, at: (f32, f32), radius: f32, damage: u32);
    fn native_orbits(&mut self, owner: u32);
    fn native_melee(&mut self, owner: u32, reach: f32, arc: f32, damage: u32);
    fn native_bolt(
        &mut self,
        id: u32,
        kind: Shot,
        damage: u32,
        speed: f32,
        life: u32,
        burst: u8,
        stun: u8,
    );
    fn native_attack(&mut self, id: u32, input: Input, book: &Book, seed: u64);
    fn native_card(&mut self, id: u32, card: &Card);
    fn native_field(
        &mut self,
        owner: u32,
        at: (f32, f32),
        radius: f32,
        damage: u32,
        left: u32,
        kind: FieldKind,
    );
    fn native_tick_fields(&mut self);
    fn native_fly(&mut self);
}
impl NativeSession for Session {
    fn native_spawn(&mut self, id: u32) {
        let point = SPAWNS
            .iter()
            .copied()
            .enumerate()
            .max_by(|(ia, a), (ib, b)| {
                let clearance = |p: (f32, f32)| {
                    self.fighters
                        .iter()
                        .filter(|(other, f)| **other != id && f.alive())
                        .map(|(_, f)| (p.0 - f.hero.x).hypot(p.1 - f.hero.y))
                        .fold(100., f32::min)
                };
                clearance(*a).total_cmp(&clearance(*b)).then(ib.cmp(ia))
            })
            .unwrap()
            .1;
        let f = self.fighters.get_mut(&id).unwrap();
        (f.hero.x, f.hero.y) = point;
        f.yaw = (18. - point.1).atan2(24. - point.0);
        f.hero.hp = f.hero.max_hp;
        f.hero.dress();
        f.hero.armor = f.hero.bonus.armor.min(MAX_ARMOR);
        f.hero.invulnerable = 0;
        f.hero.dash_ticks = 0;
        f.hero.shielding = false;
        f.hero.walling = false;
        f.hero.champ = Default::default();
        f.hero.hero_kit = Default::default();
        f.hero.fire_cooldown = 0;
        f.hero.spell_cooldowns = [0; cards::SPELL_SLOTS];
        f.protection = PROTECTION;
        f.power = 0;
        f.stun = 0;
        f.burn = 0;
    }
    fn native_step(&mut self, inputs: &BTreeMap<u32, Input>, book: &Book, seed: u64) {
        if self.winner.is_some() {
            self.ended += 1;
            return;
        }
        if self.fighters.values().filter(|f| f.connected).count() < 2 {
            return;
        }
        self.melee.tick();
        self.ticks = self.ticks.saturating_add(1);
        if self.ticks <= TITLE {
            return;
        }
        for t in &mut self.pickups {
            *t = t.saturating_sub(1);
        }
        let ids: Vec<_> = self.fighters.keys().copied().collect();
        for &id in &ids {
            let f = self.fighters.get_mut(&id).unwrap();
            if !f.connected {
                continue;
            }
            if f.respawn > 0 {
                f.respawn -= 1;
                if f.respawn == 0 {
                    self.native_spawn(id);
                }
                continue;
            }
            f.protection = f.protection.saturating_sub(1);
            f.power = f.power.saturating_sub(1);
            f.flash = f.flash.saturating_sub(1);
            f.stun = f.stun.saturating_sub(1);
            f.fart_wait = f.fart_wait.saturating_sub(1);
            if self.ticks % HZ == 0 {
                f.hero.leeched = 0;
                f.hero.passive = 0;
            }
            f.hero.chilled = f.hero.chilled.saturating_sub(1);
            f.hero.invulnerable = f.hero.invulnerable.saturating_sub(1);
            f.hero.fire_cooldown = f.hero.fire_cooldown.saturating_sub(1);
            f.hero.sword_cooldown = f.hero.sword_cooldown.saturating_sub(1);
            f.hero.play_cooldown = f.hero.play_cooldown.saturating_sub(1);
            f.hero.bomb_cooldown = f.hero.bomb_cooldown.saturating_sub(1);
            f.hero.champ.fury = f.hero.champ.fury.saturating_sub(1);
            for t in &mut f.hero.spell_cooldowns {
                *t = t.saturating_sub(1);
            }
            let input = inputs
                .get(&id)
                .copied()
                .filter(|i| i.valid())
                .unwrap_or_default();
            f.walk_arena(input, &self.fields);
            if f.burn > 0 {
                f.burn -= 1;
                if f.burn % 15 == 0 {
                    let owner = f.burner;
                    self.native_hit(owner, id, 4, (0., 0.));
                }
            }
            let h = &mut self.fighters.get_mut(&id).unwrap().hero;
            let slam = h.champ.windup == 1;
            h.champ.windup = h.champ.windup.saturating_sub(1);
            h.champ.rot = h.champ.rot.saturating_sub(1);
            let rot = h.champ.rot > 0 && h.champ.rot % champions::ROT_EVERY == 0;
            let at = (h.x, h.y);
            let slam_reach = h.spell_step("groundslam").slam_reach;
            let slam_damage = h.spell_damage("groundslam", champions::SLAM_DAMAGE);
            let rot_reach = h.spell_step("rot").rot_reach;
            let rot_damage = h.spell_damage("rot", champions::ROT_DAMAGE);
            if slam {
                self.native_area(id, at, slam_reach, slam_damage);
            }
            if rot {
                self.native_area(id, at, rot_reach, rot_damage);
            }
            if self.fighters[&id].protection == 0 && self.ticks % ORBIT_EVERY == 0 {
                self.native_orbits(id);
            }
            self.native_attack(id, input, book, seed);
            if self.winner.is_some() {
                break;
            }
            let f = self.fighters.get_mut(&id).unwrap();
            if !f.alive() {
                continue;
            }
            for (k, &(x, y, kind)) in PICKUPS.iter().enumerate() {
                if self.pickups[k] > 0 || (x - f.hero.x).hypot(y - f.hero.y) > 1.2 {
                    continue;
                }
                let taken = match kind {
                    Pickup::Health if f.hero.hp < f.hero.max_hp => {
                        f.hero.hp = (f.hero.hp + 35).min(f.hero.max_hp);
                        true
                    }
                    Pickup::Armor if f.hero.armor < MAX_ARMOR => {
                        f.hero.armor = (f.hero.armor + 2).min(MAX_ARMOR);
                        true
                    }
                    Pickup::Power => {
                        f.power = POWER_TIME;
                        true
                    }
                    _ => false,
                };
                if taken {
                    self.pickups[k] = PICKUP_REST;
                }
            }
        }
        if self.winner.is_none() {
            self.native_fly();
            self.native_tick_fields();
        }
        self.feed
            .retain(|f| self.ticks.saturating_sub(f.tick) < 6 * HZ);
    }
    fn native_hit(&mut self, owner: u32, target: u32, damage: u32, way: (f32, f32)) -> bool {
        if owner == target || self.winner.is_some() || !self.fighters.contains_key(&owner) {
            return false;
        }
        let Some(f) = self
            .fighters
            .get_mut(&target)
            .filter(|f| f.alive() && f.protection == 0)
        else {
            return false;
        };
        if shield::turns(&f.hero, way)
            || (f.hero.walling && -(way.0 * f.hero.aim_x + way.1 * f.hero.aim_y) > 0.)
        {
            f.hero.shield_turned(damage);
            self.melee.parry((f.hero.x, f.hero.y), unit(way.0, way.1));
            return false;
        }
        let before = f.hero.hp;
        f.hero.hit_ward = 0;
        f.hero.hurt(damage);
        f.hero.dress();
        if f.hero.hp == before {
            return false;
        }
        let tier = if damage >= 45 {
            melee::Tier::Heavy
        } else if damage >= 20 {
            melee::Tier::Medium
        } else {
            melee::Tier::Light
        };
        self.melee.hit(
            target,
            (f.hero.x, f.hero.y),
            unit(way.0, way.1),
            tier,
            false,
        );
        if f.hero.hp == 0 {
            f.respawn = RESPAWN;
            f.deaths = f.deaths.saturating_add(1);
            let killer = self.fighters.get_mut(&owner).unwrap();
            killer.kills += 1;
            self.feed.push(Frag {
                killer: owner,
                victim: target,
                tick: self.ticks,
            });
            if self.feed.len() > 5 {
                self.feed.remove(0);
            }
            if killer.kills == GOAL {
                self.winner = Some(owner);
                self.ended = 0;
            }
        }
        true
    }
    fn native_visible(&self, a: (f32, f32), b: (f32, f32)) -> bool {
        if !sight(a, b) {
            return false;
        }
        let n = ((b.0 - a.0).hypot(b.1 - a.1) * 5.).ceil().max(1.) as u32;
        (0..=n).all(|i| {
            let t = i as f32 / n as f32;
            !self
                .fields
                .iter()
                .any(|f| f.blocks(a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t, 0.))
        })
    }
    fn native_area(&mut self, owner: u32, at: (f32, f32), radius: f32, damage: u32) {
        let mut targets: Vec<_> = self
            .fighters
            .iter()
            .filter(|(id, f)| **id != owner && f.alive())
            .filter_map(|(&id, f)| {
                let p = (f.hero.x, f.hero.y);
                let d = (p.0 - at.0).hypot(p.1 - at.1);
                (d <= radius && self.native_visible(at, p)).then_some((d, id, p))
            })
            .collect();
        targets.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for (rank, (_, id, p)) in targets.into_iter().enumerate() {
            self.native_hit(owner, id, aoe::part(damage, rank), (p.0 - at.0, p.1 - at.1));
        }
    }
    fn native_orbits(&mut self, owner: u32) {
        let h = &self.fighters[&owner].hero;
        if h.bonus.orbit == 0 {
            return;
        }
        let damage = h.scaled(ORBIT_DAMAGE);
        let swept: Vec<_> = (0..ORBIT_EVERY)
            .flat_map(|back| orbit_points(h, owner, u64::from(self.ticks.saturating_sub(back))))
            .collect();
        let from = (h.x, h.y);
        let mut targets: Vec<_> = self
            .fighters
            .iter()
            .filter(|(id, f)| **id != owner && f.alive())
            .filter(|(_, f)| {
                swept
                    .iter()
                    .any(|p| (f.hero.x - p.0).hypot(f.hero.y - p.1) <= HERO_RADIUS + 0.8)
                    && self.native_visible(from, (f.hero.x, f.hero.y))
            })
            .map(|(&id, f)| {
                (
                    (f.hero.x - from.0).hypot(f.hero.y - from.1),
                    id,
                    (f.hero.x - from.0, f.hero.y - from.1),
                )
            })
            .collect();
        targets.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for (rank, (_, id, way)) in targets.into_iter().enumerate() {
            self.native_hit(owner, id, aoe::part(damage, rank), way);
        }
    }
    fn native_melee(&mut self, owner: u32, reach: f32, arc: f32, damage: u32) {
        let f = &self.fighters[&owner];
        let h = &f.hero;
        let at = (h.x, h.y);
        let stroke = if h.sword {
            melee::Stroke::Slash
        } else if h.unarmed() {
            melee::Stroke::Fist
        } else {
            melee::stroke(h, arc)
        };
        self.melee.swing(owner, h, stroke, reach, arc);
        let yaw = f.yaw;
        let mut targets: Vec<_> = self
            .fighters
            .iter()
            .filter(|(id, f)| **id != owner && f.alive())
            .filter_map(|(&id, f)| {
                let (dx, dy) = (f.hero.x - at.0, f.hero.y - at.1);
                let turn = (dy.atan2(dx) - yaw)
                    .sin()
                    .atan2((dy.atan2(dx) - yaw).cos())
                    .abs();
                (dx.hypot(dy) <= reach + HERO_RADIUS
                    && turn <= arc.to_radians() / 2.
                    && self.native_visible(at, (f.hero.x, f.hero.y)))
                .then_some((dx.hypot(dy), id, (dx, dy)))
            })
            .collect();
        targets.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for (rank, (_, id, way)) in targets.into_iter().enumerate() {
            self.native_hit(owner, id, aoe::part(damage, rank), way);
        }
    }
    fn native_bolt(
        &mut self,
        id: u32,
        kind: Shot,
        damage: u32,
        speed: f32,
        life: u32,
        burst: u8,
        stun: u8,
    ) {
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
            traits: ShotTraits {
                owner: id as u8,
                burst,
                stun,
                ..ShotTraits::of(&h.bonus)
            },
        });
    }
    fn native_attack(&mut self, id: u32, input: Input, book: &Book, seed: u64) {
        if self.winner.is_some() || !self.fighters[&id].alive() || self.fighters[&id].stun > 0 {
            return;
        }
        if input.cycle {
            self.fighters.get_mut(&id).unwrap().hero.cycle_hand();
        }
        if let Some(slot) = usize::from(input.cast)
            .checked_sub(1)
            .filter(|&s| s < cards::SPELL_SLOTS)
        {
            let f = self.fighters.get_mut(&id).unwrap();
            if f.hero.spell_cooldowns[slot] == 0
                && let Some(card) = f.hero.spells[slot].as_deref().and_then(|s| book.get(s))
            {
                f.hero.spell_cooldowns[slot] = f.hero.spell_wait(card);
                self.native_card(id, card);
            }
        }
        if let Some(slot) = usize::from(input.play).checked_sub(1) {
            let f = self.fighters.get_mut(&id).unwrap();
            if f.hero.play_cooldown == 0 && slot < f.hero.hand.len() {
                let card = f.hero.hand.remove(slot);
                f.hero.play_cooldown = PLAY_REARM;
                if let Some(card) = book.get(&card) {
                    self.native_card(id, card);
                }
            }
        }
        if input.bomb
            && self.fighters[&id].hero.bombs > 0
            && self.fighters[&id].hero.bomb_cooldown == 0
        {
            let h = &mut self.fighters.get_mut(&id).unwrap().hero;
            h.bombs -= 1;
            h.bomb_cooldown = BOMB_REARM;
            let at = (h.x, h.y);
            self.fighters.get_mut(&id).unwrap().protection = 0;
            self.native_area(id, at, BOMB_CLEAR, BOMB_DAMAGE);
        }
        let spin_damage = {
            let h = &self.fighters[&id].hero;
            (h.spin > 0 && spin::sweeping(spin::TICKS.saturating_sub(h.spin + 1)))
                .then(|| h.scaled(26))
        };
        if let Some(damage) = spin_damage {
            let h = &self.fighters[&id].hero;
            self.melee
                .swing(id, h, melee::Stroke::Chain, spin::REACH, 360.0);
            self.native_area(id, (h.x, h.y), spin::REACH, damage);
        }
        let f = self.fighters.get_mut(&id).unwrap();
        let h = &mut f.hero;
        let guarded = h.rolling().is_some() || h.shielding || h.walling || h.champ.busy();
        if input.swing && h.sword_cooldown == 0 && !guarded {
            h.sword_cooldown = h.cooldown(SWORD_REARM);
            h.sword = true;
            h.swing = 8;
            f.flash = 5;
            f.protection = 0;
            let damage = h.scaled(SWORD_DAMAGE);
            self.native_melee(id, SWORD_REACH, SWORD_ARC, damage);
        }
        let melee = {
            let h = &self.fighters[&id].hero;
            h.forged.as_ref().and_then(|w| {
                w.melee.filter(|m| {
                    (w.bolt.is_none() && w.throw.is_none())
                        || self.fighters.iter().any(|(&other, f)| {
                            other != id
                                && f.alive()
                                && (f.hero.x - h.x).hypot(f.hero.y - h.y) <= m.reach
                        })
                })
            })
        };
        let f = self.fighters.get_mut(&id).unwrap();
        let h = &mut f.hero;
        if !input.fire || h.fire_cooldown > 0 || guarded {
            return;
        }
        f.protection = 0;
        f.flash = 4;
        let start = self.shots.len();
        if champions::of(h) == Some(champions::Champion::Alchemist) {
            h.fire_cooldown = h.cooldown(champions::FLASK_REARM);
            let damage = h.scaled(champions::FLASK_DAMAGE);
            self.native_bolt(id, Shot::Ember, damage, 16., 18, 100, 0);
        } else if h.unarmed() {
            h.sword = false;
            h.fire_cooldown = h.cooldown(9);
            let damage = h.scaled(6) * if f.power > 0 { 3 } else { 2 } / 2;
            self.native_melee(id, 1.5, 100., damage);
        } else if let Some(m) = melee {
            h.sword = false;
            h.swing = 6;
            let fury = h.champ.fury > 0;
            h.fire_cooldown = h.cooldown(if fury { m.every / 2 } else { m.every });
            let damage = h.scaled(m.damage) * if fury { 3 } else { 2 } / 2
                * if f.power > 0 { 3 } else { 2 }
                / 2;
            self.native_melee(id, m.reach, m.arc, damage);
        } else {
            let flying = self
                .shots
                .iter()
                .filter(|p| u32::from(p.traits.owner) == id)
                .count();
            let f = self.fighters.get_mut(&id).unwrap();
            f.hero.loose(
                id,
                book,
                false,
                mix(seed ^ self.ticks as u64 * 0x9e37 ^ (id as u64) << 48),
                flying,
                &mut self.shots,
            );
        }
        wishcraft::shape(&mut self.shots[start..]);
        if self.fighters[&id].power > 0 {
            for p in &mut self.shots[start..] {
                p.damage = p.damage * 3 / 2;
            }
        }
        self.shots.truncate(MAX_PROJECTILES);
    }
    fn native_card(&mut self, id: u32, card: &Card) {
        let f = self.fighters.get_mut(&id).unwrap();
        f.protection = 0;
        f.flash = 5;
        let h = &mut f.hero;
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
        if self.fighters[&id].power > 0 {
            for p in &mut self.shots[start..] {
                p.damage = p.damage * 3 / 2;
            }
        }
        if nova > 0 {
            self.native_area(id, at, 9., nova);
        }
        for effect in &card.effects {
            match *effect {
                Effect::Rip(n) => {
                    self.native_area(id, aim, 4., n);
                    self.fighters
                        .get_mut(&id)
                        .unwrap()
                        .hero
                        .mend(n / 2, stakes::Mend::Potion);
                }
                Effect::Cabbages(n) => self.native_field(
                    id,
                    aim,
                    3.2,
                    damage,
                    (n.min(8) * 10).max(10),
                    FieldKind::Hazard,
                ),
                Effect::Hedge(_) => self.native_field(id, aim, 3., 0, 5 * HZ, FieldKind::Hedge),
                Effect::Imposture(_) => self.native_field(id, at, 1., 0, 5 * HZ, FieldKind::Decoy),
                Effect::Summon(n, _) | Effect::Ghoul(n) | Effect::Totem(n) | Effect::Warden(n) => {
                    self.native_field(id, at, 10., n.max(8), 8 * HZ, FieldKind::Turret)
                }
                _ => {}
            }
        }
        match card.id.as_str() {
            "storm-hammer" => self.native_bolt(
                id,
                Shot::Sacred,
                self.fighters[&id]
                    .hero
                    .spell_damage(&card.id, champions::HAMMER_DAMAGE),
                champions::HAMMER_SPEED,
                champions::HAMMER_LIFE,
                100,
                30,
            ),
            "groundslam" => {
                self.fighters.get_mut(&id).unwrap().hero.champ.windup = champions::SLAM_WINDUP;
            }
            "brute-force" => {
                self.fighters.get_mut(&id).unwrap().hero.champ.fury =
                    self.fighters[&id].hero.spell_step(&card.id).fury
            }
            "meat-hook" => self.native_bolt(
                id,
                Shot::Blade,
                self.fighters[&id]
                    .hero
                    .spell_damage(&card.id, champions::HOOK_DAMAGE),
                champions::HOOK_SPEED,
                18,
                0,
                30,
            ),
            "rot" => {
                self.fighters.get_mut(&id).unwrap().hero.champ.rot =
                    self.fighters[&id].hero.spell_step(&card.id).rot
            }
            "chain-haul" => {
                let h = &mut self.fighters.get_mut(&id).unwrap().hero;
                let reach = h.spell_step(&card.id).haul_reach;
                (h.x, h.y) = grid().halted((h.x, h.y), (h.aim_x * reach, h.aim_y * reach));
            }
            "miasma-flask" => self.native_field(
                id,
                aim,
                3.,
                champions::FOUL_POWER,
                5 * HZ,
                FieldKind::Hazard,
            ),
            "quickfire" => self.native_field(
                id,
                aim,
                self.fighters[&id].hero.spell_step(&card.id).quick_reach,
                self.fighters[&id]
                    .hero
                    .spell_damage(&card.id, champions::QUICK_DAMAGE),
                self.fighters[&id].hero.spell_step(&card.id).quick_life,
                FieldKind::Hazard,
            ),
            "elixir" => {
                let mend = self.fighters[&id]
                    .hero
                    .spell_power(&card.id, champions::ELIXIR_MEND);
                self.fighters
                    .get_mut(&id)
                    .unwrap()
                    .hero
                    .mend(mend, stakes::Mend::Potion);
            }
            _ => {}
        }
        self.shots.truncate(MAX_PROJECTILES);
    }
    fn native_field(
        &mut self,
        owner: u32,
        at: (f32, f32),
        radius: f32,
        damage: u32,
        left: u32,
        kind: FieldKind,
    ) {
        if self.fields.len() < 32
            && open(at.0, at.1)
            && sight(
                (self.fighters[&owner].hero.x, self.fighters[&owner].hero.y),
                at,
            )
        {
            self.fields.push(Field {
                owner,
                x: at.0,
                y: at.1,
                radius,
                damage,
                left,
                wait: 0,
                kind,
            });
        }
    }
    fn native_tick_fields(&mut self) {
        let mut fields = std::mem::take(&mut self.fields);
        for f in &mut fields {
            f.left = f.left.saturating_sub(1);
            f.wait = f.wait.saturating_sub(1);
            if f.wait == 0 && f.damage > 0 {
                f.wait = if f.kind == FieldKind::Turret { 20 } else { 10 };
                self.native_area(f.owner, (f.x, f.y), f.radius, f.damage);
            }
        }
        fields.retain(|f| f.left > 0);
        self.fields = fields;
    }
    fn native_fly(&mut self) {
        let mut shots = std::mem::take(&mut self.shots);
        let mut children = Vec::new();
        let orbits: Vec<_> = self
            .fighters
            .iter()
            .filter(|(_, f)| f.alive() && f.protection == 0)
            .flat_map(|(&id, f)| {
                orbit_points(&f.hero, id, u64::from(self.ticks))
                    .into_iter()
                    .map(move |p| (id, p))
            })
            .collect();
        for p in &mut shots {
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
                .min_by(|a, b| {
                    (a.1.hero.x - p.x)
                        .hypot(a.1.hero.y - p.y)
                        .total_cmp(&(b.1.hero.x - p.x).hypot(b.1.hero.y - p.y))
                });
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
                for decoy in self
                    .fields
                    .iter()
                    .filter(|f| f.kind == FieldKind::Decoy && f.owner != owner)
                {
                    if (decoy.x - p.x).hypot(decoy.y - p.y) < (aim.0 - p.x).hypot(aim.1 - p.y) {
                        aim = (decoy.x, decoy.y);
                    }
                }
                let now = p.vy.atan2(p.vx);
                let want = (aim.1 - p.y).atan2(aim.0 - p.x);
                let turn = (want - now).sin().atan2((want - now).cos());
                let a = now
                    + turn.clamp(
                        -HOMING_TURN * p.traits.homing as f32 * DT,
                        HOMING_TURN * p.traits.homing as f32 * DT,
                    );
                let speed = p.vx.hypot(p.vy);
                (p.vx, p.vy) = (a.cos() * speed, a.sin() * speed);
            }
            let from = (p.x, p.y);
            let to = (p.x + p.vx * DT, p.y + p.vy * DT);
            if orbits
                .iter()
                .any(|(id, at)| *id != owner && shield::entry(from, to, *at, 0.9).is_some())
            {
                p.ttl = 0;
                continue;
            }
            let stone = grid().shot_stop_body(from, to, 0.10).or_else(|| {
                let n = ((to.0 - from.0).hypot(to.1 - from.1) * 8.).ceil().max(1.) as u32;
                (1..=n).find_map(|i| {
                    let t = i as f32 / n as f32;
                    let p = (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
                    self.fields
                        .iter()
                        .any(|f| f.blocks(p.0, p.1, 0.1))
                        .then_some(p)
                })
            });
            let decoy_t = self
                .fields
                .iter()
                .filter(|f| f.kind == FieldKind::Decoy && f.owner != owner)
                .filter_map(|f| shield::entry(from, to, (f.x, f.y), 0.5))
                .min_by(f32::total_cmp)
                .unwrap_or(2.);
            let wall_t = stone.map_or(2., |at| {
                (at.0 - from.0).hypot(at.1 - from.1)
                    / (to.0 - from.0).hypot(to.1 - from.1).max(0.001)
            });
            let stone_t = wall_t.min(decoy_t);
            let mut hits: Vec<_> = self
                .fighters
                .iter()
                .filter(|(id, f)| {
                    **id != owner && f.alive() && f.protection == 0 && p.last_hit != Some(**id)
                })
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
                    f.kind == FieldKind::Decoy
                        && f.owner != owner
                        && shield::entry(from, to, (f.x, f.y), 0.5).is_some_and(|d| d < t)
                }) {
                    p.ttl = 0;
                    break;
                }
                if blocked {
                    self.fighters
                        .get_mut(&id)
                        .unwrap()
                        .hero
                        .shield_turned(p.damage);
                    p.ttl = 0;
                    break;
                }
                let at = (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
                if self.native_hit(
                    owner,
                    id,
                    aoe::part(p.damage, p.traits.struck as usize),
                    (p.vx, p.vy),
                ) {
                    p.traits.struck = p.traits.struck.saturating_add(1);
                    let hook = p.kind == Shot::Blade
                        && p.traits.stun > 0
                        && champions::of(&self.fighters[&owner].hero)
                            == Some(champions::Champion::Butcher);
                    let caster = (self.fighters[&owner].hero.x, self.fighters[&owner].hero.y);
                    let f = self.fighters.get_mut(&id).unwrap();
                    f.stun = f.stun.max(u32::from(p.traits.stun).min(HZ));
                    f.hero.chilled = f.hero.chilled.max(u32::from(p.traits.chill) * lich::CHILL);
                    if p.traits.fire || p.traits.poison > 0 {
                        f.burn = 2 * HZ;
                        f.burner = owner;
                    }
                    let knock = wishcraft::knock(p);
                    let (dx, dy) = unit(p.vx, p.vy);
                    (f.hero.x, f.hero.y) =
                        grid().halted((f.hero.x, f.hero.y), (dx * knock, dy * knock));
                    if hook {
                        let (dx, dy) = (caster.0 - f.hero.x, caster.1 - f.hero.y);
                        let dist = dx.hypot(dy).max(0.01);
                        let pull = (dist - 1.4).max(0.);
                        (f.hero.x, f.hero.y) = grid()
                            .halted((f.hero.x, f.hero.y), (dx / dist * pull, dy / dist * pull));
                    }
                    if p.traits.burst > 0 {
                        self.native_area(
                            owner,
                            at,
                            2.8,
                            p.damage * u32::from(p.traits.burst) / 100,
                        );
                    }
                    if p.traits.chain > 0 {
                        let chain: Vec<_> = self
                            .fighters
                            .iter()
                            .filter(|(other, f)| {
                                **other != owner
                                    && **other != id
                                    && f.alive()
                                    && (f.hero.x - at.0).hypot(f.hero.y - at.1) < 5.
                                    && self.native_visible(at, (f.hero.x, f.hero.y))
                            })
                            .take(p.traits.chain as usize)
                            .map(|(&id, _)| id)
                            .collect();
                        for id in chain {
                            self.native_hit(owner, id, p.damage / 2, (p.vx, p.vy));
                        }
                    }
                    if let Some(f) = self.fighters.get_mut(&owner).filter(|f| f.alive()) {
                        let vamp = f.hero.bonus.vamp.min(20);
                        f.hero.leech(
                            p.damage * vamp / 100 + u32::from(p.traits.mend),
                            stakes::Mend::Passive,
                        );
                    }
                    if p.traits.fart > 0 && self.fighters[&owner].fart_wait == 0 {
                        self.fighters.get_mut(&owner).unwrap().fart_wait =
                            2 * HZ / u32::from(p.traits.fart);
                        let damage = self.fighters[&owner].hero.scaled(skills::POISON_BASE);
                        self.native_field(owner, at, 2., damage, 3 * HZ, FieldKind::Hazard);
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
        for p in shots.iter().filter(|p| p.ttl == 0 && p.kind == Shot::Ember) {
            let owner = u32::from(p.traits.owner);
            if self
                .fighters
                .get(&owner)
                .is_some_and(|f| champions::of(&f.hero) == Some(champions::Champion::Alchemist))
            {
                self.native_area(owner, (p.x, p.y), champions::FLASK_BURST, p.damage);
                self.native_field(
                    owner,
                    (p.x, p.y),
                    champions::FLASK_FIRE,
                    p.damage / 4,
                    champions::FLASK_FIRE_LIFE,
                    FieldKind::Hazard,
                );
            }
        }
        shots.retain(|p| p.ttl > 0);
        shots.extend(
            children
                .into_iter()
                .take(MAX_PROJECTILES.saturating_sub(shots.len())),
        );
        self.shots = shots;
    }
}
pub trait NativeRun {
    fn native_tick_deathmatch(&mut self, inputs: &BTreeMap<u32, Input>);
}
impl NativeRun for Run {
    fn native_tick_deathmatch(&mut self, inputs: &BTreeMap<u32, Input>) {
        let Some(mut s) = self.deathmatch.take() else {
            return;
        };
        let crossroads = self.crossroads.is_some();
        // Seats arriving late get their real kit, but nobody outside the
        // room, absent or disconnected can be farmed for kills.
        for (&id, h) in &self.players {
            if crossroads || h.absent || self.exits.outside.contains_key(&id) {
                continue;
            }
            if s.winner.is_none() && !s.fighters.contains_key(&id) {
                s.fighters
                    .insert(id, Fighter::new(h, id as usize - 1, &self.book));
                s.ante.insert(id, h.carried.get(Spoil::Gold) / 10);
            }
        }
        for (&id, f) in &mut s.fighters {
            f.connected = self.players.get(&id).is_some_and(|h| !h.away && !h.absent)
                && (crossroads || !self.exits.outside.contains_key(&id));
        }
        let before = s.winner;
        let kills: u32 = s.fighters.values().map(|f| u32::from(f.kills)).sum();
        let tick = s.ticks;
        s.native_step(inputs, &self.book, self.seed);
        if before.is_none()
            && s.ticks > tick
            && s.fighters.values().any(|f| f.alive() && f.flash == 4)
        {
            for f in s.fighters.values().filter(|f| f.alive() && f.flash == 4) {
                self.sounds.push(
                    if f.hero.forged.as_ref().is_some_and(|w| w.melee.is_some()) || f.hero.unarmed()
                    {
                        "swing"
                    } else {
                        classes::shot_sound(&f.hero).unwrap_or(match f.hero.weapon {
                            Weapon::Bow => "shot_bow",
                            Weapon::Crossbow => "shot_crossbow",
                            Weapon::Handgonne => "shot_handgonne",
                            _ => "shot_spark",
                        })
                    },
                );
            }
        }
        if s.fighters.values().map(|f| u32::from(f.kills)).sum::<u32>() > kills {
            self.sounds.push("hit");
        }
        if !crossroads
            && before.is_none()
            && let Some(id) = s.winner
        {
            for (&seat, &stake) in &s.ante {
                if let Some(h) = self.players.get_mut(&seat) {
                    let amount = stake.min(h.carried.get(Spoil::Gold));
                    h.carried.take(&Spoils([(Spoil::Gold, amount)].into()));
                    s.purse += amount;
                    h.laurel = seat == id;
                }
            }
            if let Some(h) = self.players.get_mut(&id) {
                h.carried.add(Spoil::Gold, s.purse);
            }
            self.sounds.push("play_card");
            self.cues.push("pastime_won".into());
        }
        let return_home = !crossroads && s.winner.is_some() && s.ended >= PODIUM;
        self.deathmatch = Some(s);
        if return_home {
            let ids: Vec<_> = self.players.keys().copied().collect();
            for id in ids {
                self.leave_activity(id);
            }
            self.deathmatch = None;
        }
    }
}
