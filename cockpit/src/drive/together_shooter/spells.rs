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
                traits,
                ttl: 3 * HZ,
            });
        }
    }
}

impl Run {
    pub(super) fn spell_flourish(&mut self, id: u32, name: &str) {
        self.found = Some((self.tick, id, format!("learned {name}")));
        self.sounds.push("card_pickup");
        if let Some(hero) = self.players.get(&id) {
            if self.blasts.len() < 16 {
                self.blasts.push((hero.x, hero.y, BLAST_TICKS));
            }
        }
    }

    /// Poll only on the host, not on a mirror. Book membership does not mean
    /// a spell reached a slot: another watcher or reload may have inserted it.
    /// Unchanged cards do nothing unless the host can equip the missing spell;
    /// malformed/part-written cards are retried on the next poll. Manual
    /// reload remains the line-numbered error-reporting path.
    pub(crate) fn hot_load_spells(&mut self, dir: &std::path::Path) -> usize {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return 0;
        };
        let mut paths: Vec<_> = entries
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "card"))
            .collect();
        paths.sort();
        let mut loaded = 0;
        for path in paths.into_iter().take(cards::MAX_BOOK) {
            if !std::fs::metadata(&path).is_ok_and(|m| m.len() <= cards::MAX_BYTES as u64) {
                continue;
            }
            let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let Ok(raw) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(checked) = cards::check(id, &raw) else {
                continue;
            };
            if checked.card.kind != cards::Kind::Spell {
                continue;
            }
            let needs_slot = self
                .players
                .get(&1)
                .is_some_and(|hero| hero.takes(&checked.card));
            if self.book.get(&checked.card.id) == Some(&checked.card) && !needs_slot {
                continue;
            }
            if self.add_card(checked.card, 1) {
                loaded += 1;
            }
        }
        loaded
    }
}
