//! Reusable spell slots and bounded, data-only live card delivery.

use super::*;

impl Hero {
    /// The spell this knight cast lately whose shots fly as `kind` (a
    /// bolt's or a ring's bolts, a falling star's ball), the youngest: what
    /// a shot of theirs breaking now was, as the magic pass draws it
    /// (`render::overworld::arena::magic`), for its family's sound.
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
        // The bar mirrors the spells, for older readers.
        if let Some(bar) = self.bar.iter_mut().find(|s| s.is_none()) {
            *bar = Some(cards::BarSlot::Spell(card.id.clone()));
        }
        true
    }

    /// Shapes use the same damage scaling and shot gifts as the knight's bow.
    /// A meteor is a heavy bursting ball; a ring flies out in twelve directions.
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

/// How long a spell's shots fly.
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

    /// One spell card's text (its pictures cut into rows), hot-loaded on
    /// the host: true when it was added. A card already in the book does
    /// nothing unless the host can equip the missing spell.
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

// ─── the second grimoire ─────────────────────────────────────────────────────

/// Cabbage Storm: how long a cabbage falls, the beat between cabbages,
/// how hard one thumps, and how wide the ring scatters round the aim.
pub const CABBAGE_FALL: u32 = 26;
pub const CABBAGE_STEP: u32 = 7;
pub const CABBAGE_DAMAGE: u32 = 34;
pub const CABBAGE_SPREAD: f32 = 3.2;
/// The Imposture: how long the counterfeit keeps the monsters' faith, and
/// the most phantoms (false knights and Phantasm's images together) a room
/// will humour.
pub const IMPOSTURE_TICKS: u32 = 5 * HZ;
pub const IMPOSTURE_MAX: usize = 4;

/// Cabbage Storm: `count` cabbages come down round where the knight aims,
/// the first dead on the spot and the rest a beat apart, each a full
/// stroke where it lands. The spread is fixed angles, not rolls — a raid
/// replays it exactly. Nothing a knight grows hurts a knight.
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
        // A ring round the aim, tight and loose by turns, the first
        // cabbage on the mark itself — where a knight aims, a farmer
        // intends.
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

/// The Imposture: a counterfeit of the knight steps out of them, two
/// strides along their aim, and keeps their pace beside them (a phantom,
/// exactly as Phantasm makes them). Monsters aim at what they see, and
/// the first hostile shot that catches the lie unmakes it. One counterfeit
/// a knight; Phantasm's own images, longer-lived, are left standing.
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

/// Hedge Maze: how tall the box is, how soft its walls, and how many
/// walls a room will humour past its own.
pub const HEDGE_HALF: f32 = 3.0;
pub const HEDGE_HP: u32 = 90;
pub const HEDGE_ROOM: usize = 21;

/// Hedge Maze: a square of hedge rises round where the knight aims —
/// three walls and a door, the door facing the caster. Box a monster in,
/// or box yourself a fort; either way it is topiary, and it will not be
/// pruned for you. The walls are the barricades' own: they age, they
/// crumble, and monsters gnaw them like anything else.
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
    // The four edges of the box; the one facing home stays open, because
    // a maze without a door is just a mistake with shrubs.
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

// ─── the second grimoire, continued ─────────────────────────────────────────

/// Vesper Bell: how long the wings are held to the ground (a second's
/// stumble, and every hazard a flier never feared is suddenly relevant).
pub(crate) const VESPER_GROUNDS: u32 = 30;
/// Monk's Patience: how long the knight may not fire, and mends a little.
pub(crate) const PATIENCE_TICKS: u32 = 6 * HZ;
/// Plague of Frogs: how long the small ones hop, how big is too big to
/// frog, and how wide the ring reaches.
pub(crate) const FROG_TICKS: u32 = 6 * HZ;
pub(crate) const FROG_MAX_HP: u32 = 60;
pub(crate) const FROG_RING: f32 = 9.0;
/// Excommunicate: how near the nearest monster must stand to any knight,
/// how long the shame holds it, and what a boss pays instead of going.
pub(crate) const EXCOMMUNICATE_REACH: f32 = 8.0;
pub(crate) const EXCOMMUNICATE_FREEZE: u32 = 3 * HZ;
pub(crate) const EXCOMMUNICATE_SCOURGE: u32 = 15;

/// Vesper Bell: the evening bell rings once, and everything with wings
/// falls out of the air and MUST WALK. Ground monsters never noticed;
/// a room of bats suddenly has no cavalry.
pub(super) fn vesper_bell(enemies: &mut [Enemy]) {
    for enemy in enemies.iter_mut().filter(|e| e.hp > 0 && e.kind.flies()) {
        enemy.grounded = enemy.grounded.max(VESPER_GROUNDS);
    }
}

/// Monk's Patience: for six seconds the knight may not fire, and needs
/// nothing: half of every blow, a mend every half second. The monk's
/// patience is endless, and boring, and wins.
pub(super) fn monks_patience(hero: &mut Hero) {
    hero.patience = PATIENCE_TICKS;
}

/// Plague of Frogs: every small monster in a wide ring of the knights
/// becomes a frog — the hexer's own medicine, bottled from the other
/// side. Frogs hop away from the knights and can do nothing else; the
/// plague wears off in six seconds.
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

/// Excommunicate: the nearest monster within eight paces of any knight is
/// READ OUT of the room — flung to the farthest corner, shunned three
/// seconds even its own kind will not stand near, and it drops from its
/// kind's drop rolls this death. The Church keeps its peace and its cut.
/// Bosses and guardians take instead a scourging: fifteen percent of all
/// they are, and the shame.
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
    // The farthest corner of the room: the Church is not subtle.
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
