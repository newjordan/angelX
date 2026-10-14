//! The Quiet Room: eight books on eight lecterns. E at a lectern opens its
//! book: a page worth a few coins and a terrible title, or teeth. Three of
//! the eight are mimics, and the floor decides which. Read every real book
//! and the best reader is crowned.
use super::*;

/// The lecterns (tiles): two rows of four.
pub const LECTERNS: [(i32, i32); 8] = [(5, 4), (9, 4), (14, 4), (18, 4), (5, 9), (9, 9), (14, 9), (18, 9)];
/// Shelves along the north wall, clear of the north doorway.
const SHELVES: [(i32, i32, i32, i32); 2] = [(2, 1, 7, 1), (15, 1, 7, 1)];
/// How many books bite, the coin in a page, the bite.
pub const MIMICS: u32 = 3;
pub const PAGE_GOLD: u32 = 4;
pub const BITE: u32 = 10;
pub const TITLES: [&str; 8] = [
    "SOUP FOR KNIGHTS, VOL. II",
    "HOW TO PET A DRAGON (BRIEFLY)",
    "1001 GOOSE GRIEVANCES",
    "ON PRIVIES, AND WHY",
    "A HISTORY OF THE SOCK",
    "BOOKS ARE NOT TEETH",
    "THE WEEK MERLIN WAS A TOAD",
    "MAUD'S TAB, ABRIDGED",
];

pub fn blocks() -> Vec<(i32, i32, i32, i32)> {
    LECTERNS.iter().map(|&(c, r)| (c, r, 1, 1)).chain(SHELVES).collect()
}

/// Where a reader stands at lectern `i` (tiles): just south of it.
pub fn stand(i: usize) -> (f32, f32) {
    (LECTERNS[i].0 as f32 + 0.5, LECTERNS[i].1 as f32 + 1.6)
}

/// Which books bite, from the floor's own shape (no dice).
pub fn mimics(floor: &Floor) -> u16 {
    let shape = floor
        .rooms
        .iter()
        .fold(u64::from(floor.depth) ^ 0xb00c, |h, r| h.wrapping_mul(41).wrapping_add((r.cell.0 * 23 + r.cell.1) as u64));
    let mut mask = 0u16;
    let mut k = 0u64;
    while mask.count_ones() < MIMICS {
        mask |= 1 << (super::super::mix(shape ^ k) % LECTERNS.len() as u64);
        k += 1;
    }
    mask
}

impl Run {
    pub(super) fn tick_reading(&mut self, nook: &mut Nook, pressed: u8) {
        for id in self.nook_players() {
            if !has(pressed, id) {
                continue;
            }
            let Some(i) = (0..LECTERNS.len()).find(|&i| nook.opened & 1 << i == 0 && self.nook_near(id, stand(i), REACH)) else {
                continue;
            };
            nook.opened |= 1 << i;
            if nook.mimics & 1 << i != 0 {
                if let Some(hero) = self.players.get_mut(&id) {
                    hero.invulnerable = 0;
                    hero.hurt(BITE);
                }
                self.sounds.push("mimic");
                self.found = Some((self.tick, id, "IT BITES!".into()));
                let (c, r) = LECTERNS[i];
                self.nook_raise(nook, EnemyKind::Mimic, (c as f32 + 0.5, r as f32 + 2.2));
            } else {
                *nook.score.entry(id).or_default() += 1;
                if let Some(hero) = self.players.get_mut(&id) {
                    hero.carried.add(Spoil::Gold, PAGE_GOLD);
                }
                self.sounds.push("card_pickup");
                self.found = Some((self.tick, id, TITLES[i].into()));
            }
        }
        let real = !nook.mimics & ((1 << LECTERNS.len()) - 1);
        if nook.opened & real == real {
            nook.done = true;
            let best = self.nook_crown(nook);
            let name = best.and_then(|id| self.players.get(&id)).map_or_else(String::new, |h| h.name.to_uppercase());
            self.sounds.push("chest_open");
            self.found = Some((self.tick, 0, format!("ALL READ! {name} READ MOST.")));
        }
    }
}
