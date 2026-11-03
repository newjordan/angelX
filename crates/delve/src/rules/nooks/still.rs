//! Mother Mort's Still: a cauldron and three jars, BONE, WAX and ORE. E at
//! a jar tips in one of that spoil from the knight's own pack; the second
//! jar brews. Bone and wax make a healing potion, wax and ore a thunder
//! scroll, bone and ore a bomb. The same jar twice makes a frog, briefly.
//! Each knight brews twice a floor; every third brew the still burps up a
//! slime. On a knight's second brew, Mort serves her afterhours menu instead
//! of the usual potion or scroll; full hands retain the usual fallback.
use super::*;

pub const CAULDRON: (i32, i32, i32, i32) = (11, 3, 2, 2);
/// Where Mother Mort stands, behind the cauldron.
pub const MORT: (f32, f32) = (12.0, 2.6);
pub const BLOCKS: [(i32, i32, i32, i32); 3] = [CAULDRON, (2, 1, 5, 2), (17, 1, 5, 2)];
/// The jars (where a knight stands, tiles) and what goes in each.
pub const JARS: [((f32, f32), Spoil, &str); 3] = [
    ((7.5, 7.5), Spoil::Bone, "BONE"),
    ((12.0, 8.5), Spoil::Wax, "WAX"),
    ((16.5, 7.5), Spoil::Ore, "ORE"),
];
/// Brews a knight may brew a floor; brews between burps.
pub const BREWS: u8 = 2;
pub const BURP_EVERY: u16 = 3;
/// What a full hand gets from a potion instead.
pub const SPLASH_MEND: u32 = 20;

/// The first serving teaches the recipe; the second brings Mort's house special.
/// Keep this independent of RNG so saved runs and guests agree on the reward.
fn serving(pair: (usize, usize), brew: u8) -> Option<(&'static str, &'static str)> {
    match (pair, brew >= BREWS) {
        ((0, 1), false) => Some(("potion", "A HEALING POTION!")),
        ((1, 2), false) => Some(("thunder-scroll", "A THUNDER SCROLL!")),
        ((0, 1), true) => Some(("mooncrab-cracker", "MORT SERVES MOONCRAB CRACKER!")),
        ((1, 2), true) => Some(("powderplum-tart", "MORT SERVES POWDERPLUM TART!")),
        ((0, 2), true) => Some(("firefly-sorbet", "MORT SERVES FIREFLY SORBET!")),
        _ => None,
    }
}

/// Announce the reward actually delivered, not the dish that could not fit.
fn serving_message(pair: (usize, usize), brew: u8, full: bool) -> &'static str {
    if full {
        return if pair == (0, 1) {
            "PACK FULL. MORT MENDS YOUR WOUNDS INSTEAD!"
        } else {
            "PACK FULL. A BOMB INSTEAD!"
        };
    }
    serving(pair, brew).map_or("A BOMB! CAREFUL.", |(_, message)| message)
}

impl Run {
    pub(super) fn tick_still(&mut self, nook: &mut Nook, pressed: u8) {
        for id in self.nook_players() {
            if !has(pressed, id) {
                continue;
            }
            let Some(j) = (0..JARS.len()).find(|&j| self.nook_near(id, JARS[j].0, REACH)) else {
                continue;
            };
            if nook.brews.get(&id).copied().unwrap_or(0) >= BREWS {
                self.found = Some((self.tick, id, "SHE SAYS: ENOUGH.".into()));
                continue;
            }
            let (_, spoil, word) = JARS[j];
            let Some(hero) = self.players.get_mut(&id) else { continue };
            if hero.carried.get(spoil) == 0 {
                self.found = Some((self.tick, id, format!("NO {word} ON YOU.")));
                continue;
            }
            let mut one = Spoils::default();
            one.add(spoil, 1);
            hero.carried.take(&one);
            self.sounds.push("holy_water");
            let Some(first) = nook.jars.remove(&id) else {
                nook.jars.insert(id, j as u8 + 1);
                self.found = Some((self.tick, id, format!("{word} IN. ONE MORE.")));
                continue;
            };
            *nook.brews.entry(id).or_default() += 1;
            *nook.score.entry(id).or_default() += 1;
            let pair = (usize::from(first - 1).min(j), usize::from(first - 1).max(j));
            let special = serving(pair, nook.brews[&id]);
            let said = match pair {
                (a, b) if a == b => {
                    self.hex(id);
                    "RIBBIT. WRONG MIX."
                }
                (0, 1) => {
                    let hero = self.players.get_mut(&id).unwrap();
                    if hero.hand.len() < cards::HAND {
                        hero.hand.push(special.unwrap().0.into());
                        serving_message(pair, nook.brews[&id], false)
                    } else {
                        hero.hp = (hero.hp + SPLASH_MEND).min(hero.max_hp);
                        serving_message(pair, nook.brews[&id], true)
                    }
                }
                (1, 2) => {
                    let hero = self.players.get_mut(&id).unwrap();
                    if hero.hand.len() < cards::HAND {
                        hero.hand.push(special.unwrap().0.into());
                        serving_message(pair, nook.brews[&id], false)
                    } else {
                        hero.give_bombs(1);
                        serving_message(pair, nook.brews[&id], true)
                    }
                }
                _ => {
                    let hero = self.players.get_mut(&id).unwrap();
                    if let Some((card, message)) = special {
                        if hero.hand.len() < cards::HAND {
                            hero.hand.push(card.into());
                            message
                        } else {
                            hero.give_bombs(1);
                            "PACK FULL. A BOMB INSTEAD!"
                        }
                    } else {
                        hero.give_bombs(1);
                        "A BOMB! CAREFUL."
                    }
                }
            };
            self.found = Some((self.tick, id, said.into()));
            let total: u16 = nook.score.values().sum();
            if total.is_multiple_of(BURP_EVERY) {
                self.sounds.push("gas_release");
                self.nook_raise(nook, EnemyKind::Slime, (12.0, 6.2));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exercise the real interaction, not only the recipe lookup table.
    #[test]
    fn mort_second_brew_delivers_special_and_enforces_limit() {
        for (a, b, reward) in [(0, 1, "mooncrab-cracker"), (1, 2, "powderplum-tart"), (0, 2, "firefly-sorbet")] {
            let mut run = Run::new(7, 0, None);
            let mut nook = Nook::default();
            let hero = run.players.get_mut(&1).unwrap();
            hero.hand.clear();
            hero.carried.add(JARS[a].1, 3);
            hero.carried.add(JARS[b].1, 3);
            for j in [a, b, a, b] {
                let hero = run.players.get_mut(&1).unwrap();
                hero.x = JARS[j].0.0 * TILE_UNITS;
                hero.y = JARS[j].0.1 * TILE_UNITS;
                run.tick_still(&mut nook, 1 << 1);
            }
            assert_eq!(nook.brews[&1], BREWS);
            assert_eq!(nook.score[&1], 2);
            assert_eq!(run.players[&1].hand.last().unwrap(), reward);
            assert_eq!(run.players[&1].hand.len(), if (a, b) == (0, 2) { 1 } else { 2 });
            let before = run.players[&1].carried.clone();
            run.tick_still(&mut nook, 1 << 1);
            assert_eq!(run.players[&1].carried, before);
            assert_eq!(run.players[&1].hand.len(), if (a, b) == (0, 2) { 1 } else { 2 });
            assert_eq!(run.found.as_ref().unwrap().2, "SHE SAYS: ENOUGH.");
        }
    }

    #[test]
    fn mort_full_hand_interaction_delivers_fallback() {
        for (a, b) in [(0, 1), (1, 2), (0, 2)] {
            for prior in [0, 1] {
                let mut run = Run::new(7, 0, None);
                let mut nook = Nook::default();
                nook.brews.insert(1, prior);
                let hero = run.players.get_mut(&1).unwrap();
                hero.hand = vec!["potion".into(); cards::HAND];
                hero.hp = 1;
                hero.carried.add(JARS[a].1, 1);
                hero.carried.add(JARS[b].1, 1);
                let bombs = hero.bombs;
                let max_hp = hero.max_hp;
                let hand = hero.hand.clone();
                for j in [a, b] {
                    let hero = run.players.get_mut(&1).unwrap();
                    hero.x = JARS[j].0.0 * TILE_UNITS;
                    hero.y = JARS[j].0.1 * TILE_UNITS;
                    run.tick_still(&mut nook, 1 << 1);
                }
                let hero = &run.players[&1];
                assert_eq!(hero.hand, hand);
                assert_eq!(hero.carried.get(JARS[a].1), 0);
                assert_eq!(hero.carried.get(JARS[b].1), 0);
                assert_eq!(nook.brews[&1], prior + 1);
                assert_eq!(nook.score[&1], 1);
                assert_eq!(hero.hp, if a == 0 && b == 1 { (1 + SPLASH_MEND).min(max_hp) } else { 1 });
                assert_eq!(hero.bombs, bombs + if (a, b) == (0, 1) { 0 } else { 1 });
                let expected = if (a, b) == (0, 2) && prior == 0 {
                    "A BOMB! CAREFUL."
                } else {
                    serving_message((a, b), prior + 1, true)
                };
                assert_eq!(run.found.as_ref().unwrap().2, expected);
            }
        }
    }

    #[test]
    fn mort_full_hand_announces_fallback_not_an_unreceived_dish() {
        for brew in 1..=BREWS {
            assert_eq!(serving_message((0, 1), brew, true),
                "PACK FULL. MORT MENDS YOUR WOUNDS INSTEAD!");
            assert_eq!(serving_message((1, 2), brew, true),
                "PACK FULL. A BOMB INSTEAD!");
            assert_eq!(serving_message((0, 2), brew, true),
                "PACK FULL. A BOMB INSTEAD!");
            for pair in [(0, 1), (1, 2), (0, 2)] {
                if let Some((_, message)) = serving(pair, brew) {
                    assert_eq!(serving_message(pair, brew, false), message);
                }
            }
        }
    }

    #[test]
    fn mort_afterhours_menu_keeps_recipes_and_serves_real_cards() {
        assert_eq!(serving((0, 1), 1).unwrap().0, "potion");
        assert_eq!(serving((1, 2), 1).unwrap().0, "thunder-scroll");
        assert!(serving((0, 2), 1).is_none());
        for (pair, expected) in [
            ((0, 1), "mooncrab-cracker"),
            ((1, 2), "powderplum-tart"),
            ((0, 2), "firefly-sorbet"),
        ] {
            assert_eq!(serving(pair, BREWS).unwrap().0, expected);
            assert!(cards::Book::builtin().get(expected).is_some());
        }
        for j in 0..3 {
            assert!(serving((j, j), BREWS).is_none(), "same jars must still hex");
        }
    }
}
