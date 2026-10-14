//! The Bone Dominoes: a winding line of skulls and long bones on end round
//! a bone coffer. E at the first sets them falling, one after another.
//! Three places in the line have no bone: a knight standing there when the
//! line arrives falls over in its place and the line runs on. An empty gap
//! stalls it, and the bones that would not fall get up as skeletons. The
//! whole line down rings the coffer open: coin for everyone, and the crown
//! for whoever fell over most.
use super::*;

/// The line, in falling order (tiles): along the top, down the east side,
/// back along the bottom.
pub const PATH: [(i32, i32); 42] = {
    let mut path = [(0, 0); 42];
    let mut k = 0;
    while k < 18 {
        path[k] = (3 + k as i32, 3);
        k += 1;
    }
    while k < 24 {
        path[k] = (20, 4 + (k - 18) as i32);
        k += 1;
    }
    while k < 42 {
        path[k] = (20 - (k - 24) as i32, 10);
        k += 1;
    }
    path
};
/// The places where a knight must stand in for a bone.
pub const GAPS: [usize; 3] = [12, 21, 33];
/// The coffer in the middle, and two skull pillars.
pub const COFFER: (i32, i32, i32, i32) = (11, 6, 2, 2);
pub const BLOCKS: [(i32, i32, i32, i32); 3] = [COFFER, (7, 6, 1, 2), (16, 6, 1, 2)];
/// Ticks between one bone and the next; how long a stalled line takes to
/// stand again; the coin when the coffer rings.
pub const FALL: u32 = 4;
pub const STAND_AGAIN: u32 = 2 * HZ;
pub const COFFER_GOLD: u32 = 8;

pub fn is_gap(i: usize) -> bool {
    GAPS.contains(&i)
}

/// The middle of bone `i`'s tile.
pub fn at(i: usize) -> (f32, f32) {
    (PATH[i].0 as f32 + 0.5, PATH[i].1 as f32 + 0.5)
}

impl Run {
    pub(super) fn tick_ossuary(&mut self, nook: &mut Nook, pressed: u8) {
        if nook.stalled > 0 {
            nook.stalled -= 1;
            if nook.stalled == 0 {
                nook.fallen = 0;
            }
            return;
        }
        if !nook.running {
            if let Some(id) = self.nook_players().into_iter().find(|&id| has(pressed, id) && self.nook_near(id, at(0), REACH)) {
                nook.running = true;
                nook.fallen = 0;
                nook.next = FALL;
                self.sounds.push("bones_clatter");
                self.found = Some((self.tick, id, "CLACK! FILL THE GAPS!".into()));
            }
            return;
        }
        nook.next = nook.next.saturating_sub(1);
        if nook.next > 0 {
            return;
        }
        nook.next = FALL;
        let i = usize::from(nook.fallen);
        if is_gap(i) {
            // A knight in the gap falls over in its place, or the line stops.
            let standing = self.nook_players().into_iter().find(|&id| self.nook_near(id, at(i), 1.6));
            match standing {
                Some(id) => {
                    *nook.score.entry(id).or_default() += 1;
                    self.sounds.push("knight_fall");
                    self.found = Some((self.tick, id, "TIMBER!".into()));
                }
                None => {
                    nook.running = false;
                    nook.stalled = STAND_AGAIN;
                    self.sounds.push("bones_clatter");
                    self.found = Some((self.tick, 0, "THE LINE STOPS. BONES RISE!".into()));
                    let (x, y) = at(i);
                    self.nook_raise(nook, EnemyKind::Skeleton, (x, y));
                    self.nook_raise(nook, EnemyKind::Skeleton, (12.0, 9.0));
                    return;
                }
            }
        } else if i % 3 == 0 {
            self.sounds.push("bones_clatter");
        }
        nook.fallen += 1;
        if usize::from(nook.fallen) == PATH.len() {
            nook.running = false;
            nook.done = true;
            for id in self.nook_players() {
                if let Some(hero) = self.players.get_mut(&id) {
                    hero.carried.add(Spoil::Gold, COFFER_GOLD);
                }
            }
            self.nook_crown(nook);
            self.sounds.push("circle_toll");
            self.sounds.push("chest_open");
            self.found = Some((self.tick, 0, format!("DONG! THE COFFER OPENS: {COFFER_GOLD} GOLD EACH.")));
        }
    }
}
