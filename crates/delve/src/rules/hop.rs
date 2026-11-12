//! Book Hop, a folly and a nod to Frogger: a hall of the Drowned Archive
//! flooded ink-dark, with lanes of floating books drifting across it, each
//! lane its own way and pace, and a dry bank at either end with a calm
//! landing in the middle. Everyone hops at once from the near bank: stand
//! on a book and it carries you; step in the ink, or ride a book into the
//! wall, and you sink back to the near bank (back to the landing, if you
//! had got past it). Each crossing to the far bank scores and sends you
//! round again. Most crossings when the minute is up takes the pot and the
//! crown; a gold kept for each one, and two more each time the whole table
//! has crossed (`hop_show`). The hall is always one screen, so every knight
//! sees every other.
use super::hop_show::HopShow;
use super::*;

pub const HOP_TIME: u32 = 60 * HZ;
/// Each lane's pace (units a second), by lane number, cycling.
pub const PACES: [f32; 4] = [4.0, 6.5, 5.0, 8.0];
/// One book every this many tiles along a lane, and how long books are.
pub const SPACING: f32 = 6.0;
pub const BOOK: [f32; 2] = [3.0, 2.0];
/// Crossings that make a gold to keep.
pub const CROSSINGS_PER_GOLD: u16 = 1;
/// How far (units) a book's ends reach under a knight's feet: a toe over the
/// edge is still afloat.
pub const EDGE_GRACE: f32 = 0.35;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hop {
    pub room: usize,
    pub t: u32,
    pub crossings: BTreeMap<u32, u16>,
    pub ante: BTreeMap<u32, u32>,
    /// Where someone sank, and how long its ripples show.
    pub splashes: Vec<(f32, f32, u32)>,
    pub done: bool,
    /// What lately happened, for the picture to make a fuss of
    /// (`hop_show`).
    #[serde(default, skip_serializing_if = "HopShow::quiet")]
    pub show: HopShow,
}

impl Hop {
    pub fn valid(&self) -> bool {
        self.t <= folly::TITLE + HOP_TIME + HZ
            && self.crossings.len() <= MAX_PLAYERS as usize
            && self.crossings.keys().all(|id| (1..=MAX_PLAYERS).contains(id))
            && self.splashes.len() <= 16
            && self.show.valid()
    }
    pub fn left(&self) -> u32 {
        (folly::TITLE + HOP_TIME).saturating_sub(self.t).min(HOP_TIME).div_ceil(HZ)
    }
    pub fn titled(&self) -> bool {
        self.t < folly::TITLE
    }
}

/// The rows (tiles) of a hall `rows` tall: the far bank, the lanes (with
/// the calm landing between them), and the near bank.
pub fn far_bank(_rows: i32) -> i32 {
    1
}
pub fn near_bank(rows: i32) -> i32 {
    rows - 2
}
pub fn landing(rows: i32) -> i32 {
    rows / 2
}
/// The lane number of row `r`, if it is ink.
pub fn lane(rows: i32, r: i32) -> Option<usize> {
    (r > far_bank(rows) && r < near_bank(rows) && r != landing(rows)).then(|| (r - 2) as usize)
}

/// Where lane `l`'s books lie at time `t` (ticks): their left edges and
/// their length, in units, wrapping across a room `width` units wide.
pub fn books(l: usize, t: u32, width: f32) -> (Vec<f32>, f32) {
    let dir = if l % 2 == 0 { 1.0 } else { -1.0 };
    let pace = PACES[l % PACES.len()];
    let len = BOOK[l % BOOK.len()] * TILE_UNITS;
    let gap = SPACING * TILE_UNITS;
    let drift = dir * pace * t as f32 / HZ as f32 + l as f32 * 2.3 * TILE_UNITS;
    let n = (width / gap).floor().max(1.0) as usize;
    let span = n as f32 * gap;
    ((0..n).map(|k| (k as f32 * gap + drift).rem_euclid(span)).collect(), len)
}

/// Whether a body at `x` stands on one of lane `l`'s books at `t`.
pub fn afloat(l: usize, t: u32, width: f32, x: f32) -> bool {
    afloat_with(l, t, width, x, 0.0)
}

/// As `afloat`, with `grace` units of each book's ends counting too.
pub fn afloat_with(l: usize, t: u32, width: f32, x: f32, grace: f32) -> bool {
    let (lefts, len) = books(l, t, width);
    let n = lefts.len() as f32;
    let span = n * SPACING * TILE_UNITS;
    lefts.iter().any(|&b| (x - b + grace).rem_euclid(span) < len + 2.0 * grace)
}

impl Run {
    /// Into the archive: the hall floods (and shrinks to one screen, so
    /// every knight sees every other), the antes go in, and everyone lines
    /// up on the near bank.
    pub(super) fn start_book_hop(&mut self, index: usize) {
        let room = &mut self.dungeon.rooms[index];
        room.regrow(layout::COLS, layout::ROWS);
        let (cols, rows) = (room.cols as i32, room.rows as i32);
        for y in 1..rows - 1 {
            for x in 1..cols - 1 {
                room.set(x as usize, y as usize, Tile::Floor);
            }
        }
        // What lay on the floor of the bigger hall is set on the near bank.
        let shore = (near_bank(rows) as f32 + 0.5) * TILE_UNITS;
        let spill = |x: f32| x.clamp(2.0 * TILE_UNITS, (cols as f32 - 2.0) * TILE_UNITS);
        for item in &mut room.items {
            (item.x, item.y) = (spill(item.x), shore);
        }
        if let Some(chest) = &mut room.chest {
            (chest.x, chest.y) = (spill(chest.x), shore);
        }
        self.dungeon.world_revision = self.dungeon.world_revision.wrapping_add(1);
        // The river is the hall's only hazard: the spikes and vents the hall
        // was armed with (laid for its old size) are gone.
        self.traps.clear();
        self.rocks.clear();
        self.waves = Default::default();
        let (ante, scores) = folly::take_antes(&mut self.players);
        let seated = self.players.len().max(1) as f32;
        for (k, hero) in self.players.values_mut().enumerate() {
            // Spread along the near bank, two tiles apart, round its middle.
            hero.x = (cols as f32 / 2.0 + (k as f32 - (seated - 1.0) / 2.0) * 2.0) * TILE_UNITS;
            hero.y = shore;
        }
        self.hop = Some(Hop {
            room: index,
            t: 0,
            crossings: scores.keys().map(|&id| (id, 0)).collect(),
            ante,
            splashes: Vec::new(),
            done: false,
            show: HopShow::default(),
        });
        // Those who follow the party find their feet on the bank.
        self.hireling_follows();
        self.tallow_follows();
        self.sounds.push("door_seal");
        self.cues.push("folly".into());
    }

    /// The hop's tick: books carry their riders, the ink takes the rest,
    /// the far bank scores, and the minute ends it.
    pub(super) fn tick_book_hop(&mut self) {
        let Some(mut h) = self.hop.take() else {
            return;
        };
        if h.room != self.at || h.done {
            self.hop = Some(h);
            return;
        }
        h.t += 1;
        let now = self.tick as u32;
        h.show.tick(now);
        for s in h.splashes.iter_mut() {
            s.2 = s.2.saturating_sub(1);
        }
        h.splashes.retain(|s| s.2 > 0);
        let (cols, rows) = (self.room().cols as i32, self.room().rows as i32);
        let width = cols as f32 * TILE_UNITS;
        let start_y = (near_bank(rows) as f32 + 0.5) * TILE_UNITS;
        if h.titled() {
            // Nobody leaves the bank under the title card.
            for hero in self.players.values_mut() {
                hero.y = hero.y.max(near_bank(rows) as f32 * TILE_UNITS + 0.4).min(start_y + 0.5 * TILE_UNITS - 0.4);
            }
            self.hop = Some(h);
            return;
        }
        if h.t == folly::TITLE {
            // GO: the picture kicks.
            self.fx.push(Fx::Shake(4));
        }
        let t = h.t - folly::TITLE;
        let landing_y = (landing(rows) as f32 + 0.5) * TILE_UNITS;
        let px = |x: f32, y: f32| ((x / TILE_UNITS * 16.0) as i32, (y / TILE_UNITS * 16.0) as i32);
        let mut heaviest: Option<juice::Weight> = None;
        let mut shake = 0;
        for (&id, hero) in self.players.iter_mut().filter(|(_, x)| x.hp > 0 && !x.stone) {
            let row = (hero.y / TILE_UNITS) as i32;
            let came = h.show.stood(id, row);
            if let Some(l) = lane(rows, row) {
                let dir = if l % 2 == 0 { 1.0 } else { -1.0 };
                let afloat_now = afloat_with(l, t, width, hero.x, EDGE_GRACE);
                if afloat_now {
                    // The book carries its rider.
                    hero.x += dir * PACES[l % PACES.len()] * DT;
                }
                let walled = hero.x < 1.3 * TILE_UNITS || hero.x > (cols as f32 - 1.3) * TILE_UNITS;
                if (!afloat_now || walled) && hero.invulnerable == 0 {
                    // Past the landing, the ink sends you back to the
                    // landing; before it, to the near bank.
                    let back = if row < landing(rows) { landing(rows) } else { near_bank(rows) };
                    if h.splashes.len() < 16 {
                        h.splashes.push((hero.x, hero.y, HZ / 2));
                    }
                    let weight = h.show.sank(now, id, px(hero.x, hero.y), (back - row).max(0) as u32, walled && afloat_now);
                    // A long way lost is felt, a little, by the whole table.
                    if back - row >= 6 {
                        shake = shake.max(2);
                    }
                    heaviest = heaviest.max(Some(weight));
                    hero.y = if back == landing(rows) { landing_y } else { start_y };
                    h.show.rows.insert(id, back as i16);
                    hero.x = hero.x.clamp(2.0 * TILE_UNITS, (cols as f32 - 2.0) * TILE_UNITS);
                    hero.invulnerable = HZ / 2;
                    self.sounds.push("step_wet");
                } else if came.is_some() && afloat_now {
                    h.show.landed(now, id, px(hero.x, hero.y), l as u8, !afloat_with(l, t, width, hero.x, 0.0));
                }
            } else if row <= far_bank(rows) {
                let at = px(hero.x, hero.y);
                let n = h.crossings.entry(id).or_default();
                *n += 1;
                let n = *n;
                hero.y = start_y;
                h.show.rows.insert(id, near_bank(rows) as i16);
                let weight = h.show.crossed(now, id, at, n, &h.crossings);
                heaviest = heaviest.max(Some(weight));
                shake = shake.max(if weight >= juice::Weight::Kill { 6 } else { 0 });
                self.sounds.push(if weight >= juice::Weight::Kill { "circle_toll" } else { "spoil_pickup" });
            } else if came.is_some_and(|r| lane(rows, r).is_some()) {
                // Walked off the last book onto a dry bank.
                h.show.landed(now, id, px(hero.x, hero.y), 255, false);
            }
        }
        let left = h.left();
        h.show.standings(now, &h.crossings, left);
        if shake > 0 {
            self.fx.push(Fx::Shake(shake));
        }
        if let Some(weight) = heaviest {
            self.hold_for(weight);
        }
        if t >= HOP_TIME {
            h.done = true;
            let pot: u32 = h.ante.values().sum();
            let best = h
                .crossings
                .iter()
                .max_by_key(|(id, n)| (**n, std::cmp::Reverse(**id)))
                .map(|(&id, _)| id);
            let rivals = h.crossings.len() > 1;
            let waves = u32::from(h.show.waves);
            for (&id, &n) in &h.crossings {
                if let Some(hero) = self.players.get_mut(&id) {
                    hero.carried.add(Spoil::Gold, u32::from(n / CROSSINGS_PER_GOLD) + waves * hop_show::ARCHIVE_GOLD);
                    if rivals {
                        hero.laurel = Some(id) == best;
                    }
                    if Some(id) == best {
                        hero.carried.add(Spoil::Gold, pot);
                    }
                }
            }
            if let Some(room) = self.dungeon.rooms.get_mut(h.room) {
                room.cleared = true;
            }
            let name = best
                .and_then(|id| self.players.get(&id))
                .map_or_else(|| "NOBODY".to_string(), |x| x.name.to_uppercase());
            self.found = Some((self.tick, 0, format!("{name} CROSSED THE MOST!")));
            // The hall's own record: the most crossings anyone made.
            let most = best.and_then(|id| h.crossings.get(&id)).copied().unwrap_or(0);
            let record = &mut self.home.countryside.games.hop;
            let was = record.best;
            record.best = record.best.max(most.min(1000));
            record.plays = record.plays.saturating_add(1);
            if waves > 0 {
                record.wins = record.wins.saturating_add(1).min(record.plays);
            }
            // The last of it holds the room, then the podium (`hop_show`).
            h.show.finish(now, best.unwrap_or(0), pot, most, was);
            self.hold_for(juice::Weight::Finale);
            self.fx.push(Fx::Shake(12));
            self.sounds.push("circle_toll");
            self.sounds.push("play_card");
            self.cues.push("pastime_won".into());
        }
        self.hop = Some(h);
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_hop__tests.rs"]
mod tests;
