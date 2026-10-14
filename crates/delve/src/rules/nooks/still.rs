//! Mother Mort's Still: a cauldron and three jars, BONE, WAX and ORE. E at
//! a jar tips in one of that spoil from the knight's own pack; the second
//! jar brews. Bone and wax make a healing potion, wax and ore a thunder
//! scroll, bone and ore a bomb. The same jar twice makes a frog, briefly.
//! Each knight brews twice a floor; every third brew the still burps up a
//! slime.
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
            let said = match pair {
                (a, b) if a == b => {
                    self.hex(id);
                    "RIBBIT. WRONG MIX."
                }
                (0, 1) => {
                    let hero = self.players.get_mut(&id).unwrap();
                    if hero.hand.len() < cards::HAND {
                        hero.hand.push("potion".into());
                    } else {
                        hero.hp = (hero.hp + SPLASH_MEND).min(hero.max_hp);
                    }
                    "A HEALING POTION!"
                }
                (1, 2) => {
                    let hero = self.players.get_mut(&id).unwrap();
                    if hero.hand.len() < cards::HAND {
                        hero.hand.push("thunder-scroll".into());
                    } else {
                        hero.bombs = (hero.bombs + 1).min(MAX_BOMBS);
                    }
                    "A THUNDER SCROLL!"
                }
                _ => {
                    let hero = self.players.get_mut(&id).unwrap();
                    hero.bombs = (hero.bombs + 1).min(MAX_BOMBS);
                    "A BOMB! CAREFUL."
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
