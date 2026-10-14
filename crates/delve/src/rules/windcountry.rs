//! Wind-country: free footpaths, a goose picnic and the kitewright's signal job.
//! Quest progress is a bounded private intent, settled with the ordinary save.
//!
//! The sail is a heist: the picnic sail on Goose Green is guarded by a very
//! large goose. Whoever grabs it (E) is chased; if the goose catches them it
//! pecks them and takes the sail back. A friend can honk at it (E beside
//! the goose) to draw it off for three seconds. Walk the sail off the green
//! and it's yours, and the crown with it.
use super::*;

/// The goose's pace (a knight walks at 13), how near is caught, how far a
/// peck knocks you, and how long a honk holds its attention.
pub const GOOSE_PACE: f32 = 16.0;
/// How long a grab startles it (it rears and honks) before it gives chase.
pub const STARTLE: u32 = HZ * 3 / 4;
const CAUGHT: f32 = 1.1;
const PECK: f32 = 3.0;
pub const HONK: u32 = 3 * HZ;

/// The heist under way: the goose (units), who carries the sail (0: on
/// its spot), and who it is honked at, for how long.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Heist {
    pub goose: (f32, f32),
    pub carrier: u32,
    pub honked: Option<(u32, u32)>,
    #[serde(default)]
    pub startled: u32,
}

impl Heist {
    pub fn valid(&self) -> bool {
        (0.0..200.0).contains(&self.goose.0)
            && (0.0..200.0).contains(&self.goose.1)
            && self.carrier <= MAX_PLAYERS
            && self.honked.is_none_or(|(id, t)| id <= MAX_PLAYERS && t <= HONK)
            && self.startled <= STARTLE
    }
}

/// Where the goose stands guard: a step west of the sail.
pub fn guard() -> (f32, f32) {
    ((CLOTH.0 - 1.5) * TILE_UNITS, CLOTH.1 * TILE_UNITS)
}

pub const PLACES: [RoomKind; 3] = [RoomKind::GooseGreen, RoomKind::Kitewright, RoomKind::SignalHill];
pub const VISIT_MARKS: [&str; 3] = ["country:wind:0", "country:wind:1", "country:wind:2"];
pub const QUEST_MARKS: [&str; 6] = [
    "country:signal:0", "country:signal:1", "country:signal:2",
    "country:signal:3", "country:signal:4", "country:signal:5",
];
pub const PLOT: usize = 13;
pub const KITE: (i32, i32, i32, i32) = (19, 10, 2, 1);
/// The Windmill's end of the kite: in the open north meadow, clear of the
/// bakery, the goat lift and the corner HUD.
pub const MILL_KITE: (i32, i32, i32, i32) = (14, 4, 2, 1);
pub const MAKER: (f32, f32) = (5.0, 9.0);
pub const CLOTH: (f32, f32) = (19.5, 5.0);
/// The painted flags spell FISH / BOOT / CROWN; the levers are not in that order.
pub const FLAGS: [(f32, f32); 3] = [(5.0, 5.5), (12.0, 9.0), (19.0, 5.5)];
pub const ORDER: [usize; 3] = [2, 0, 1];

pub fn stage(country: &countryside::Country, marks: &BTreeMap<String, u32>) -> u8 {
    let mut n = country.signal.min(6);
    while n < 6 && marks.contains_key(QUEST_MARKS[usize::from(n)]) { n += 1; }
    n
}
pub fn shape(room: &mut Room) {
    let rects: &[(usize, usize, usize, usize)] = match room.kind {
        RoomKind::GooseGreen => &[(3,2,5,2), (15,2,6,2), (7,10,3,2)],
        RoomKind::Kitewright => &[(3,2,6,3), (15,2,6,2), (8,10,3,2), (15,11,3,1)],
        RoomKind::SignalHill => &[(3,2,4,2), (16,2,5,2), (7,10,3,2), (15,10,3,2)],
        _ => &[],
    };
    for &(c,r,w,h) in rects { for y in r..r+h { for x in c..c+w { room.set(x,y,Tile::Block); } } }
    let stair = match room.kind {
        RoomKind::Kitewright => Some(KITE), RoomKind::Windmill => Some(MILL_KITE), _ => None,
    };
    if let Some((c,r,w,h)) = stair { for y in r..r+h { for x in c..c+w { room.set(x as usize,y as usize,Tile::Stairs); } } }
}
impl Run {
    pub(super) fn tick_windcountry(&mut self, inputs: &BTreeMap<u32, Input>) {
        let kind = self.room().kind;
        // E as pressed this tick, not held from the last.
        let held = self.players.keys()
            .filter(|id| inputs.get(id).is_some_and(|i| i.valid() && i.interact))
            .fold(0u8, |bits, &id| bits | 1 << (id & 7));
        let pressed = held & !self.lever_held;
        self.lever_held = held;
        // Off the green with the sail: the heist is won.
        if kind != RoomKind::GooseGreen {
            if let Some(h) = self.heist.take() {
                if h.carrier != 0 && self.players.contains_key(&h.carrier) {
                    self.sail_away(h.carrier);
                }
            }
        }
        let Some(i) = PLACES.iter().position(|&k| k==kind) else { return; };
        if !self.players.values().any(|h| h.hp>0 && !h.stone && h.privy==0) { return; }
        if self.home.countryside.wind_visited & (1<<i)==0 { self.marks.insert(VISIT_MARKS[i].into(),1); }
        if self.pastimes.round.is_some() { return; }
        let n=stage(&self.home.countryside,&self.marks);
        if n==6 { return; }
        if n==1 && kind==RoomKind::GooseGreen {
            self.tick_heist(pressed);
            return;
        }
        for (&id,h) in &self.players {
            if h.hp==0 || h.stone || h.privy!=0 || pressed & 1 << (id & 7) == 0 { continue; }
            let near=|at:(f32,f32)| (h.x-at.0*TILE_UNITS).hypot(h.y-at.1*TILE_UNITS)<1.4;
            let ready=match n {
                0 | 5 => kind==RoomKind::Kitewright && near(MAKER),
                2..=4 => kind==RoomKind::SignalHill && near(FLAGS[ORDER[usize::from(n-2)]]),
                _ => false,
            };
            if ready {
                self.marks.insert(QUEST_MARKS[usize::from(n)].into(),1);
                self.sounds.push("spoil_pickup");
                self.found=Some((self.tick,id,match n {
                    0=>"Fetch the picnic sail. Mind the goose.".into(),
                    5=>"Kite-post commissioned! A terrible idea.".into(),
                    _=>format!("Signal {}/3. The lookout waves a sandwich.",n-1),
                }));
                break;
            } else if (2..=4).contains(&n) && kind==RoomKind::SignalHill && FLAGS.iter().any(|&at| near(at)) {
                self.found=Some((self.tick,id,"Wrong flag! Reply: NICE BOOTS.".into()));
                break;
            }
        }
    }
}
impl Run {
    /// The green: grab, chase, honk, peck.
    fn tick_heist(&mut self, pressed: u8) {
        let mut h = self.heist.take().unwrap_or(Heist { goose: guard(), ..Default::default() });
        let cloth = (CLOTH.0 * TILE_UNITS, CLOTH.1 * TILE_UNITS);
        let able = |hero: &hero::Hero| hero.hp > 0 && !hero.stone && hero.privy == 0;
        // E: grab the sail from its spot, or honk at the goose.
        for (&id, hero) in self.players.iter().filter(|(_, x)| able(x)) {
            if pressed & 1 << (id & 7) == 0 {
                continue;
            }
            if h.carrier == 0 && (hero.x - cloth.0).hypot(hero.y - cloth.1) < 1.4 * TILE_UNITS {
                h.carrier = id;
                h.startled = STARTLE;
                self.sounds.push("spoil_pickup");
                self.found = Some((self.tick, id, format!("{} HAS THE SAIL. RUN!", hero.name.to_uppercase())));
            } else if id != h.carrier && (hero.x - h.goose.0).hypot(hero.y - h.goose.1) < 2.5 * TILE_UNITS {
                h.honked = Some((id, HONK));
                self.sounds.push("hit_magic");
                self.found = Some((self.tick, id, format!("HONK! IT WANTS {}.", hero.name.to_uppercase())));
            }
        }
        // Who it goes for: the honker, else the thief, else home to guard.
        if let Some((_, t)) = h.honked.as_mut() {
            *t = t.saturating_sub(1);
        }
        h.honked = h.honked.filter(|&(id, t)| t > 0 && self.players.get(&id).is_some_and(|x| able(x)));
        if h.carrier != 0 && !self.players.get(&h.carrier).is_some_and(|x| able(x)) {
            h.carrier = 0;
        }
        let target = h.honked.map(|x| x.0).or((h.carrier != 0).then_some(h.carrier));
        let to = target.and_then(|id| self.players.get(&id)).map_or(guard(), |x| (x.x, x.y));
        let (ax, ay) = (to.0 - h.goose.0, to.1 - h.goose.1);
        let len = ax.hypot(ay);
        h.startled = h.startled.saturating_sub(1);
        if len > 0.2 && h.startled == 0 {
            let step = (GOOSE_PACE * DT).min(len);
            let room = &self.dungeon.rooms[self.at];
            let open = |x: f32, y: f32| room.tile((x / TILE_UNITS) as i32, (y / TILE_UNITS) as i32) == Tile::Floor;
            let (dx, dy) = (ax / len * step, ay / len * step);
            if open(h.goose.0 + dx, h.goose.1 + dy) {
                h.goose = (h.goose.0 + dx, h.goose.1 + dy);
            } else if open(h.goose.0 + dx, h.goose.1) {
                h.goose.0 += dx;
            } else if open(h.goose.0, h.goose.1 + dy) {
                h.goose.1 += dy;
            }
        }
        // Caught: a peck, and the sail goes home if it was the thief.
        if let Some(id) = target {
            let g = h.goose;
            let room = &self.dungeon.rooms[self.at];
            if let Some(hero) = self.players.get_mut(&id).filter(|x| x.invulnerable == 0) {
                let (bx, by) = (hero.x - g.0, hero.y - g.1);
                if bx.hypot(by) < CAUGHT {
                    let l = bx.hypot(by).max(0.01);
                    let (ux, uy) = if l < 0.2 { (1.0, 0.0) } else { (bx / l, by / l) };
                    let to = (hero.x + ux * PECK, hero.y + uy * PECK);
                    if room.tile((to.0 / TILE_UNITS) as i32, (to.1 / TILE_UNITS) as i32) == Tile::Floor {
                        (hero.x, hero.y) = to;
                    }
                    hero.invulnerable = HZ;
                    let name = hero.name.to_uppercase();
                    self.sounds.push("hero_hurt");
                    // A honker stays its quarry until the honk wears off.
                    if h.carrier == id {
                        h.carrier = 0;
                        self.found = Some((self.tick, id, "HONK! THE GOOSE TOOK IT BACK.".into()));
                    } else {
                        self.found = Some((self.tick, id, format!("PECKED! {name} REGRETS HONKING.")));
                    }
                }
            }
        }
        self.heist = Some(h);
    }

    /// The sail got away: the job moves on, and the thief is crowned.
    fn sail_away(&mut self, carrier: u32) {
        self.marks.insert(QUEST_MARKS[1].into(), 1);
        if self.party_len() > 1 {
            for (&id, hero) in self.players.iter_mut() {
                hero.laurel = id == carrier;
            }
        }
        let name = self.players.get(&carrier).map_or_else(String::new, |h| h.name.to_uppercase());
        self.sounds.push("chest_open");
        self.found = Some((self.tick, carrier, format!("{name} GOT THE SAIL AWAY! NOW FISH, BOOT, CROWN.")));
    }
}

pub fn settle(country: &mut countryside::Country, treasury: &mut Spoils, marks: &BTreeMap<String,u32>, news: &mut Vec<String>) {
    for (i,key) in VISIT_MARKS.iter().enumerate() { if marks.contains_key(*key) { country.wind_visited |= 1<<i; } }
    country.signal=stage(country,marks);
    if country.signal==6 && !country.signal_paid {
        country.signal_paid=true;
        treasury.add(Spoil::Gold,60); treasury.add(Spoil::Wax,3);
        news.push("Signals delivered: 60 gold, 3 wax.".into());
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_heist__tests.rs"]
mod tests;
