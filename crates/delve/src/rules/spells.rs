//! Reusable spell slots and bounded, data-only live card delivery.

use super::*;

impl Hero {
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
    pub(super) fn spell_shots(&self, shape: cards::Shape, owner: u32, shots: &mut Vec<Projectile>) {
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
                damage: self.scaled(damage),
                pierce: self.bonus.pierce.min(3) as u8,
                last_hit: None,
                empowered: false,
                traits: traits.clone(),
                ttl: 3 * HZ,
            });
        }
    }
}

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
    count: u32,
    room: (f32, f32),
    id: u32,
) {
    let count = count
        .clamp(3, 8)
        .min(MAX_PROJECTILES.saturating_sub(strikes.len()) as u32);
    let (ax, ay) = (hero.aim_x, hero.aim_y);
    let (ux, uy) = if ax == 0.0 && ay == 0.0 {
        (0.0, -1.0)
    } else {
        unit(ax, ay)
    };
    let (tx, ty) = (hero.x + ux * 7.0, hero.y + uy * 7.0);
    let damage = hero.scaled(CABBAGE_DAMAGE);
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
pub(super) fn call_imposture(phantoms: &mut Vec<super::ults::Phantom>, hero: &Hero, id: u32) {
    if phantoms.len() >= IMPOSTURE_MAX {
        return;
    }
    phantoms.retain(|p| p.owner != id || p.left > IMPOSTURE_TICKS);
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
        left: IMPOSTURE_TICKS,
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
            hp: HEDGE_HP,
            age: 0,
            owner: id,
            fall: None,
        });
    }
}
