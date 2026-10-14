//! The Bell Loft: four bells and a ghost choirmaster who rings a tune on
//! them, then waits. The party rings it back (E under a bell), any knight
//! any bell, so friends split up and share it. A wrong bell is a GONG, a
//! wraith comes to complain, and the tune is rung again. Three tunes, each
//! longer, and the hymn mends everyone whole with coin for the plate.
use super::*;

/// The bells (tiles, the frame they hang in) and the pews.
pub const BELLS: [(i32, i32); 4] = [(5, 3), (9, 3), (14, 3), (18, 3)];
const PEWS: [(i32, i32, i32, i32); 2] = [(4, 9, 5, 1), (15, 9, 5, 1)];
/// The tunes, by bell.
pub const SEQS: [&[u8]; 3] = [&[0, 2, 1], &[3, 1, 0, 2], &[1, 3, 2, 0, 3]];
/// Ticks a bell sounds while the ghost rings it, and between his notes;
/// how long a rung bell stays lit; the plate's coin.
pub const NOTE: u32 = 18;
pub const REST: u32 = 8;
pub const LIT: u32 = 12;
pub const HYMN_GOLD: u32 = 6;

pub fn blocks() -> Vec<(i32, i32, i32, i32)> {
    BELLS.iter().map(|&(c, r)| (c, r, 1, 1)).chain(PEWS).collect()
}

/// Where a ringer stands under bell `b` (tiles).
pub fn under(b: usize) -> (f32, f32) {
    (BELLS[b].0 as f32 + 0.5, BELLS[b].1 as f32 + 1.8)
}

/// How long the ghost takes to ring round `r`'s tune, with a beat before.
pub fn show_len(r: usize) -> u32 {
    REST * 2 + SEQS[r].len() as u32 * (NOTE + REST)
}

/// While the ghost rings: the bell sounding now, if any.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Show {
    Ringing(Option<u8>),
    YourTurn,
}

pub fn show(nook: &Nook) -> Show {
    let r = usize::from(nook.round);
    if r >= SEQS.len() || nook.shown >= show_len(r) {
        return Show::YourTurn;
    }
    let t = nook.shown.saturating_sub(REST * 2);
    if nook.shown < REST * 2 {
        return Show::Ringing(None);
    }
    let k = (t / (NOTE + REST)) as usize;
    Show::Ringing((t % (NOTE + REST) < NOTE && k < SEQS[r].len()).then(|| SEQS[r][k]))
}

impl Run {
    pub(super) fn tick_loft(&mut self, nook: &mut Nook, pressed: u8) {
        if let Some((_, t)) = nook.lit.as_mut() {
            *t = t.saturating_sub(1);
        }
        nook.lit = nook.lit.filter(|&(_, t)| t > 0);
        let r = usize::from(nook.round);
        if nook.shown < show_len(r) {
            let before = show(nook);
            nook.shown += 1;
            if let Show::Ringing(Some(_)) = show(nook)
                && before != show(nook)
            {
                self.sounds.push("circle_toll");
            }
            return;
        }
        for id in self.nook_players() {
            if !has(pressed, id) {
                continue;
            }
            let Some(b) = (0..BELLS.len()).find(|&b| self.nook_near(id, under(b), REACH)) else {
                continue;
            };
            nook.lit = Some((b as u8, LIT));
            if SEQS[r][usize::from(nook.pos)] == b as u8 {
                nook.pos += 1;
                *nook.score.entry(id).or_default() += 1;
                self.sounds.push("circle_toll");
                if usize::from(nook.pos) == SEQS[r].len() {
                    nook.pos = 0;
                    nook.shown = 0;
                    nook.round += 1;
                    if usize::from(nook.round) == SEQS.len() {
                        nook.round = SEQS.len() as u8 - 1;
                        nook.shown = show_len(SEQS.len() - 1);
                        nook.done = true;
                        for id in self.nook_players() {
                            if let Some(hero) = self.players.get_mut(&id) {
                                hero.hp = hero.max_hp;
                                hero.carried.add(Spoil::Gold, HYMN_GOLD);
                            }
                        }
                        self.nook_crown(nook);
                        self.sounds.push("wish_grant");
                        self.found = Some((self.tick, 0, "THE HYMN! ALL MENDED.".into()));
                    } else {
                        self.found = Some((self.tick, id, "BRAVO! AGAIN, LONGER.".into()));
                    }
                    break;
                }
            } else {
                nook.pos = 0;
                nook.shown = 0;
                self.sounds.push("boss_roar");
                self.found = Some((self.tick, id, "GONG! WRONG BELL.".into()));
                self.nook_raise(nook, EnemyKind::Wraith, (12.0, 7.0));
                break;
            }
        }
    }
}
