//! Reusable spell slots and bounded, data-only live card delivery.

use super::*;

impl Hero {
    pub(super) fn spell_lately<'b>(&self, book: &'b cards::Book, kind: Shot) -> Option<&'b Card> {
        (0..cards::SPELL_SLOTS)
            .filter(|&slot| self.module_spells[slot].is_none() && self.spell_cooldowns[slot] > 0)
            .filter_map(|slot| {
                let card = self.spells[slot].as_deref().and_then(|id| book.get(id))?;
                let age = self.spell_wait(card).checked_sub(self.spell_cooldowns[slot])?;
                let flies = card.effects.iter().find_map(|e| match e {
                    cards::Effect::Cast(cards::Shape::Bolt | cards::Shape::Ring) => Some(Shot::Bolt),
                    cards::Effect::Cast(cards::Shape::Meteor) => Some(Shot::Ball),
                    _ => None,
                })?;
                (flies == kind && age <= SPELL_SHOT_TICKS).then_some((age, card))
            })
            .min_by_key(|&(age, _)| age)
            .map(|(_, card)| card)
    }

    pub(super) fn equip_spell(&mut self, card: &Card) -> bool {
        if card.kind != cards::Kind::Spell || !self.takes(card) {
            return false;
        }
        let slot = self.spells.iter().position(Option::is_none).unwrap();
        self.spells[slot] = Some(card.id.clone());
        self.spell_cooldowns[slot] = 0;
        if let Some(bar) = self.bar.iter_mut().find(|s| s.is_none()) {
            *bar = Some(cards::BarSlot::Spell(card.id.clone()));
        }
        true
    }

    pub fn spell_shots(&self, shape: cards::Shape, spell: &str, owner: u32, shots: &mut Vec<Projectile>) {
        let (count, speed, damage, kind) = match shape {
            cards::Shape::Bolt => (1, 22.0, 32, Shot::Bolt),
            cards::Shape::Ring => (12, 14.0, 20, Shot::Bolt),
            cards::Shape::Meteor => (1, 12.0, 64, Shot::Ball),
        };
        let traits = ShotTraits {
            owner: owner as u8,
            ..ShotTraits::of(&self.bonus)
        };
        let aim = self.aim_y.atan2(self.aim_x);
        for i in 0..count {
            let angle = aim
                + if count > 1 {
                    std::f32::consts::TAU * i as f32 / count as f32
                } else {
                    0.0
                };
            let (ux, uy) = (angle.cos(), angle.sin());
            shots.push(Projectile {
                x: self.x + ux * 0.6,
                y: self.y + uy * 0.6,
                vx: ux * speed,
                vy: uy * speed,
                hostile: false,
                kind,
                look: None,
                damage: self.spell_damage(spell, damage),
                pierce: self.bonus.pierce.min(3) as u8,
                last_hit: None,
                empowered: false,
                traits: traits.clone(),
                ttl: SPELL_SHOT_TICKS,
            });
        }
    }
}

pub(super) const SPELL_SHOT_TICKS: u32 = 3 * HZ;

impl Run {
    pub(super) fn spell_flourish(&mut self, id: u32, name: &str) {
        self.found = Some((self.tick, id, format!("learned {name}")));
        self.sounds.push("card_pickup");
        if let Some(hero) = self.players.get(&id)
            && self.blasts.len() < 16
        {
            self.blasts.push((hero.x, hero.y, BLAST_TICKS));
        }
    }

    pub fn hot_load_spell(&mut self, id: &str, raw: &str) -> bool {
        let Ok(checked) = cards::check(id, raw) else {
            return false;
        };
        if checked.card.kind != cards::Kind::Spell {
            return false;
        }
        let needs_slot = self
            .players
            .get(&1)
            .is_some_and(|hero| hero.takes(&checked.card));
        if self.book.get(&checked.card.id) == Some(&checked.card) && !needs_slot {
            return false;
        }
        self.add_card(checked.card, 1)
    }
}

pub const CABBAGE_FALL: u32 = 26;
pub const CABBAGE_STEP: u32 = 7;
pub const CABBAGE_DAMAGE: u32 = 34;
pub const CABBAGE_SPREAD: f32 = 3.2;
pub const IMPOSTURE_TICKS: u32 = 5 * HZ;
pub const IMPOSTURE_MAX: usize = 4;

pub(super) fn cabbage_storm(
    strikes: &mut Vec<super::ults::Strike>,
    hero: &Hero,
    spell: &str,
    count: u32,
    room: (f32, f32),
    id: u32,
) {
    let count = (count + hero.spell_step(spell).cabbage_extra)
        .clamp(3, 8)
        .min(MAX_PROJECTILES.saturating_sub(strikes.len()) as u32);
    let (ax, ay) = (hero.aim_x, hero.aim_y);
    let (ux, uy) = if ax == 0.0 && ay == 0.0 {
        (0.0, -1.0)
    } else {
        unit(ax, ay)
    };
    let (tx, ty) = (hero.x + ux * 7.0, hero.y + uy * 7.0);
    let damage = hero.spell_damage(spell, CABBAGE_DAMAGE);
    for i in 0..count {
        let a = i as f32 * std::f32::consts::TAU / count.max(1) as f32;
        let r = if i == 0 {
            0.0
        } else {
            CABBAGE_SPREAD * (0.55 + 0.45 * f32::from(i % 2 == 0))
        };
        strikes.push(super::ults::Strike {
            x: (tx + a.cos() * r).clamp(2.5, room.0 - 2.5),
            y: (ty + a.sin() * r).clamp(2.5, room.1 - 2.5),
            fall: CABBAGE_FALL + i * CABBAGE_STEP,
            damage,
            owner: id,
        });
    }
}

pub(super) fn call_imposture(phantoms: &mut Vec<super::ults::Phantom>, hero: &Hero, spell: &str, id: u32) {
    if phantoms.len() >= IMPOSTURE_MAX {
        return;
    }
    let duration = hero.spell_step(spell).imposture;
    phantoms.retain(|p| p.owner != id || p.left > duration);
    let (ax, ay) = (hero.aim_x, hero.aim_y);
    let (ux, uy) = if ax == 0.0 && ay == 0.0 {
        (0.0, -1.0)
    } else {
        unit(ax, ay)
    };
    let (dx, dy) = (ux * 2.6, uy * 2.6);
    phantoms.push(super::ults::Phantom {
        owner: id,
        x: hero.x + dx,
        y: hero.y + dy,
        dx,
        dy,
        left: hero.spell_step(spell).imposture,
        ..super::ults::Phantom::default()
    });
}

pub const HEDGE_HALF: f32 = 3.0;
pub const HEDGE_HP: u32 = 90;
pub const HEDGE_ROOM: usize = 21;

pub(super) fn hedge_maze(
    walls: &mut Vec<super::skills::Barricade>,
    hero: &Hero,
    spell: &str,
    id: u32,
    room: (f32, f32),
) {
    let (ax, ay) = (hero.aim_x, hero.aim_y);
    let (ux, uy) = if ax == 0.0 && ay == 0.0 {
        (0.0, -1.0)
    } else {
        unit(ax, ay)
    };
    let cx = (hero.x + ux * 6.0).clamp(3.0, room.0 - 3.0);
    let cy = (hero.y + uy * 6.0).clamp(3.0, room.1 - 3.0);
    let h = HEDGE_HALF;
    let edges = [
        ((cx - h, cy - h), (cx + h, cy - h), ux < -0.5),
        ((cx + h, cy - h), (cx + h, cy + h), uy > 0.5),
        ((cx - h, cy + h), (cx + h, cy + h), ux > 0.5),
        ((cx - h, cy - h), (cx - h, cy + h), uy < -0.5),
    ];
    for (a, b, is_door) in edges {
        if is_door || walls.len() >= HEDGE_ROOM {
            continue;
        }
        walls.push(super::skills::Barricade {
            a,
            b,
            hp: hero.spell_power(spell, HEDGE_HP),
            age: 0,
            owner: id,
            fall: None,
            thick: false,
        });
    }
}

pub(crate) const VESPER_GROUNDS: u32 = 30;
pub(crate) const PATIENCE_TICKS: u32 = 6 * HZ;
pub(crate) const FROG_TICKS: u32 = 6 * HZ;
pub(crate) const FROG_MAX_HP: u32 = 60;
pub(crate) const FROG_RING: f32 = 9.0;
pub(crate) const EXCOMMUNICATE_REACH: f32 = 8.0;
pub(crate) const EXCOMMUNICATE_FREEZE: u32 = 3 * HZ;
pub(crate) const EXCOMMUNICATE_SCOURGE: u32 = 15;

pub(super) fn vesper_bell(enemies: &mut [Enemy]) {
    for enemy in enemies.iter_mut().filter(|e| e.hp > 0 && e.kind.flies()) {
        enemy.grounded = enemy.grounded.max(VESPER_GROUNDS);
    }
}

pub(super) fn monks_patience(hero: &mut Hero) {
    hero.patience = PATIENCE_TICKS;
}

pub(super) fn plague_of_frogs(enemies: &mut [Enemy], knights: &BTreeMap<u32, (f32, f32)>) {
    let spots: Vec<(f32, f32)> = knights.values().copied().collect();
    for enemy in enemies.iter_mut().filter(|e| {
        e.hp > 0
            && e.boss.is_none()
            && !e.kind.elite()
            && e.kind.hp() <= FROG_MAX_HP
            && e.kind != EnemyKind::Ward
            && e.kind != EnemyKind::Dummy
    }) {
        if spots
            .iter()
            .any(|&(kx, ky)| (kx - enemy.x).hypot(ky - enemy.y) <= FROG_RING)
        {
            enemy.frog = FROG_TICKS;
        }
    }
}

pub(super) fn excommunicate(
    enemies: &mut [Enemy],
    knights: &BTreeMap<u32, (f32, f32)>,
    room: (f32, f32),
) {
    let spots: Vec<(f32, f32)> = knights.values().copied().collect();
    let Some(target) = enemies
        .iter_mut()
        .filter(|e| e.hp > 0 && e.kind != EnemyKind::Ward && e.kind != EnemyKind::Dummy)
        .min_by(|a, b| {
            let near = |e: &Enemy| {
                spots
                    .iter()
                    .map(|&(kx, ky)| (kx - e.x).hypot(ky - e.y))
                    .fold(f32::INFINITY, f32::min)
            };
            near(a).total_cmp(&near(b))
        })
        .filter(|e| {
            spots
                .iter()
                .any(|&(kx, ky)| (kx - e.x).hypot(ky - e.y) <= EXCOMMUNICATE_REACH)
        })
    else {
        return;
    };
    if target.boss.is_some() {
        target.hp = target.hp.saturating_sub(target.max_hp * EXCOMMUNICATE_SCOURGE / 100);
        return;
    }
    let (w, h) = room;
    let corners = [(2.5, 2.5), (w - 2.5, 2.5), (2.5, h - 2.5), (w - 2.5, h - 2.5)];
    let (cx, cy) = corners
        .into_iter()
        .max_by(|a, b| {
            (a.0 - target.x)
                .hypot(a.1 - target.y)
                .total_cmp(&((b.0 - target.x).hypot(b.1 - target.y)))
        })
        .unwrap_or((2.5, 2.5));
    target.x = cx;
    target.y = cy;
    target.frozen = EXCOMMUNICATE_FREEZE;
    target.shunned = true;
}
