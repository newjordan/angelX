//! Eastern high roads: a free walking circuit and an unordered paper chase.
//! Only the existing realm settlement transaction grants spoils or paid travel.
//!
//! The errand is pigeon post: the Parliament's three lost tax papers are in
//! the beaks of three pigeons, one circling each room. Any knight's shot
//! bonks a pigeon, which drops its paper to be picked up with E. Pigeons
//! also poop on whoever stands under them. Whoever bonked the most is
//! crowned when the minutes are delivered.
use super::countryside::Country;
use super::{BTreeMap, HZ, Input, Room, RoomKind, Run, Spoil, Spoils, TILE_UNITS, Tile};
use serde::{Deserialize, Serialize};

/// One lap of a pigeon's figure of eight (ticks), how near a shot must come
/// (units), and how often it poops.
const LAP: u32 = 8 * HZ;
const BONK: f32 = 1.3;
const POOP_EVERY: u32 = 3 * HZ;
const POOP_REACH: f32 = 1.2;
/// A bare-handed knight's swing reaches it as it swoops past.
const SWING_REACH: f32 = 2.2;

/// The chase: a paper on the ground (which, where), each knight's bonks,
/// and each knight's poop.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Pigeons {
    pub dropped: Option<(u8, f32, f32)>,
    pub bonks: BTreeMap<u32, u8>,
    pub poop: BTreeMap<u32, u8>,
}

impl Pigeons {
    pub fn valid(&self) -> bool {
        self.bonks.len() <= super::MAX_PLAYERS as usize
            && self.poop.len() <= super::MAX_PLAYERS as usize
            && self.dropped.is_none_or(|(i, x, y)| i < 3 && (0.0..200.0).contains(&x) && (0.0..200.0).contains(&y))
    }
}

/// Where paper `i`'s pigeon flies at `tick`, in units: a figure of eight
/// over the middle of its room.
pub fn pigeon_at(room: &Room, i: usize, tick: u64) -> (f32, f32) {
    let t = ((tick + i as u64 * u64::from(LAP) / 3) % u64::from(LAP)) as f32 / LAP as f32
        * std::f32::consts::TAU;
    let (cx, cy) = (room.cols as f32 / 2.0, room.rows as f32 / 2.0);
    (
        (cx + t.sin() * room.cols as f32 * 0.32) * TILE_UNITS,
        (cy + (2.0 * t).sin() * room.rows as f32 * 0.22) * TILE_UNITS,
    )
}

/// The paper whose pigeon is up in this room, if one is.
pub fn flying(kind: RoomKind, stage: u8, papers: u8, pigeons: Option<&Pigeons>) -> Option<usize> {
    let i = DOCUMENTS.iter().position(|d| d.0 == kind)?;
    (stage == 1 && papers & (1 << i) == 0 && pigeons.is_none_or(|p| p.dropped.is_none_or(|d| usize::from(d.0) != i)))
        .then_some(i)
}

pub const PLACES: [RoomKind; 3] = [RoomKind::Belfry, RoomKind::Aqueduct, RoomKind::Aviary];
pub const VISIT_MARKS: [&str; 3] = ["country:aerie:0", "country:aerie:1", "country:aerie:2"];
pub const QUEST_MARKS: [&str; 5] = [
    "country:minutes:accept",
    "country:minutes:0",
    "country:minutes:1",
    "country:minutes:2",
    "country:minutes:deliver",
];
pub const CLERK: (f32, f32) = (5.0, 10.0);
pub const DOCUMENTS: [(RoomKind, (f32, f32), &str); 3] = [
    (RoomKind::Belfry, (20.0, 10.0), "BELL TAX"),
    (RoomKind::Aqueduct, (5.0, 8.5), "WATER TAX"),
    (RoomKind::Aviary, (19.0, 6.5), "SEED TAX"),
];
pub const FLIGHT: (i32, i32, i32, i32) = (19, 10, 2, 1);
pub const ROOKERY_FLIGHT: (i32, i32, i32, i32) = (4, 10, 2, 1);

/// Commission stage and found-paper mask include pending private intents.
pub fn progress(country: &Country, marks: &BTreeMap<String, u32>) -> (u8, u8) {
    let mut stage = country.post;
    let mut papers = country.minutes;
    if stage == 0 && marks.contains_key(QUEST_MARKS[0]) {
        stage = 1;
    }
    if stage > 0 {
        for i in 0..3 {
            if marks.contains_key(QUEST_MARKS[i + 1]) {
                papers |= 1 << i;
            }
        }
        if papers == 7 && marks.contains_key(QUEST_MARKS[4]) {
            stage = 2;
        }
    }
    (stage, papers)
}

pub fn shape(room: &mut Room) {
    // Sculptures leave a two-tile crossroad and every objective approach open.
    let blocks: &[(usize, usize, usize, usize)] = match room.kind {
        RoomKind::Belfry => &[(3, 2, 2, 3), (19, 2, 2, 3)],
        RoomKind::Aqueduct => &[(7, 2, 2, 3), (15, 2, 2, 3), (7, 10, 2, 2), (15, 10, 2, 2)],
        RoomKind::Aviary => &[(3, 2, 5, 2)],
        _ => &[],
    };
    for &(c, r, w, h) in blocks {
        for y in r..r + h {
            for x in c..c + w {
                room.set(x, y, Tile::Block);
            }
        }
    }
    let stair = match room.kind {
        RoomKind::Aviary => Some(FLIGHT),
        RoomKind::Rookery => Some(ROOKERY_FLIGHT),
        _ => None,
    };
    if let Some((c, r, w, h)) = stair {
        for y in r..r + h {
            for x in c..c + w {
                room.set(x as usize, y as usize, Tile::Stairs);
            }
        }
    }
}

impl Run {
    pub(super) fn tick_aerie(&mut self, inputs: &BTreeMap<u32, Input>) {
        let kind = self.room().kind;
        if !PLACES.contains(&kind) {
            return;
        }
        let mut living = self
            .players
            .iter()
            .filter(|(_, h)| h.hp > 0 && !h.stone && h.privy == 0);
        if living.next().is_none() {
            return;
        }
        let i = PLACES.iter().position(|&k| k == kind).unwrap();
        if self.home.countryside.aerie_visited & (1 << i) == 0 {
            self.marks.insert(VISIT_MARKS[i].into(), 1);
        }
        // No quest actions while an arcade round is playing.
        if self.pastimes.round.is_some() {
            return;
        }
        let (stage, papers) = progress(&self.home.countryside, &self.marks);
        if stage == 2 {
            self.pigeons = None;
            return;
        }
        if stage == 1 {
            self.tick_pigeon(kind, papers);
        }
        let mut action = None;
        for (&id, h) in &self.players {
            if h.hp == 0
                || h.stone
                || h.privy != 0
                || !inputs.get(&id).is_some_and(|i| i.valid() && i.interact)
            {
                continue;
            }
            let near =
                |at: (f32, f32)| (h.x - at.0 * TILE_UNITS).hypot(h.y - at.1 * TILE_UNITS) < 1.4;
            if kind == RoomKind::Aviary && near(CLERK) && (stage == 0 || papers == 7) {
                action = Some((id, if stage == 0 { 0 } else { 4 }));
                break;
            }
            // A dropped paper is picked up where it fell.
            if let Some((i, x, y)) = self.pigeons.as_ref().and_then(|p| p.dropped) {
                if DOCUMENTS[usize::from(i)].0 == kind && (h.x - x).hypot(h.y - y) < 1.4 * TILE_UNITS {
                    action = Some((id, usize::from(i) + 1));
                    break;
                }
            }
        }
        let Some((id, action)) = action else {
            return;
        };
        self.marks.insert(QUEST_MARKS[action].into(), 1);
        self.sounds.push("spoil_pickup");
        if (1..4).contains(&action) {
            if let Some(p) = self.pigeons.as_mut() {
                p.dropped = None;
            }
        }
        let message = match action {
            0 => "Lost minutes! Pigeons took them. Bonk them.".to_string(),
            1 => "BELL TAX back. It taxes complaints too.".to_string(),
            2 => "WATER TAX back. Damp paperwork.".to_string(),
            3 => "SEED TAX back. Slightly eaten.".to_string(),
            _ => self.crown_the_bonker(id),
        };
        self.found = Some((self.tick, id, message));
    }

    /// This room's pigeon: shots bonk it (it drops the paper), and it poops
    /// on whoever stands beneath.
    fn tick_pigeon(&mut self, kind: RoomKind, papers: u8) {
        let mut p = self.pigeons.take().unwrap_or_default();
        if let Some(i) = flying(kind, 1, papers, Some(&p)) {
            let (x, y) = pigeon_at(self.room(), i, self.tick);
            let shot = self
                .projectiles
                .iter_mut()
                .find(|s| !s.hostile && s.ttl > 0 && (s.x - x).hypot(s.y - y) < BONK)
                .map(|s| {
                    s.ttl = 0;
                    u32::from(s.traits.owner)
                });
            let swung = self
                .players
                .iter()
                .find(|(_, h)| h.hp > 0 && h.swing > 0 && (h.x - x).hypot(h.y - y) < SWING_REACH)
                .map(|(&id, _)| id);
            if let Some(owner) = shot.or(swung) {
                // The shooter, else the nearest knight, takes the bonk.
                let by = if self.players.contains_key(&owner) {
                    owner
                } else {
                    self.players
                        .iter()
                        .min_by(|a, b| (a.1.x - x).hypot(a.1.y - y).total_cmp(&(b.1.x - x).hypot(b.1.y - y)))
                        .map_or(0, |(&id, _)| id)
                };
                *p.bonks.entry(by).or_default() += 1;
                // The paper lands under it, or at its old spot if that's rock.
                let floor = self.room().tile((x / TILE_UNITS) as i32, (y / TILE_UNITS) as i32) == Tile::Floor;
                let at = if floor { (x, y) } else { (DOCUMENTS[i].1.0 * TILE_UNITS, DOCUMENTS[i].1.1 * TILE_UNITS) };
                p.dropped = Some((i as u8, at.0, at.1));
                self.sounds.push("hit_arrow");
                self.found = Some((self.tick, by, format!("BONK! {} DROPPED.", DOCUMENTS[i].2)));
            } else if self.tick % u64::from(POOP_EVERY) == 0 {
                let under: Vec<u32> = self
                    .players
                    .iter()
                    .filter(|(_, h)| h.hp > 0 && (h.x - x).hypot(h.y - y) < POOP_REACH)
                    .map(|(&id, _)| id)
                    .collect();
                for id in under {
                    let n = p.poop.entry(id).or_default();
                    *n = n.saturating_add(1).min(9);
                    self.sounds.push("hit_magic");
                    self.found = Some((self.tick, id, "SPLAT! THE PIGEON VOTED.".into()));
                }
            }
        }
        self.pigeons = Some(p);
    }

    /// A knight's shot right at this room's pigeon, as if aimed well; the
    /// spot its paper fell, if one did.
    #[cfg(any(test, feature = "testing"))]
    pub fn bonk_pigeon_for_test(&mut self, by: u32) -> Option<(f32, f32)> {
        let kind = self.room().kind;
        let (stage, papers) = progress(&self.home.countryside, &self.marks);
        let i = flying(kind, stage, papers, self.pigeons.as_ref())?;
        let (x, y) = pigeon_at(self.room(), i, self.tick);
        self.projectiles.push(super::Projectile {
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            hostile: false,
            kind: super::Shot::Arrow,
            look: None,
            damage: 1,
            pierce: 0,
            last_hit: None,
            ttl: 5,
            empowered: false,
            traits: super::ShotTraits { owner: by as u8, ..Default::default() },
        });
        self.tick_pigeon(kind, papers);
        self.projectiles.retain(|p| p.ttl > 0);
        self.pigeons.as_ref()?.dropped.map(|d| (d.1, d.2))
    }

    /// The minutes in: the best bonker is crowned and everyone is cleaned.
    fn crown_the_bonker(&mut self, giver: u32) -> String {
        let p = self.pigeons.take().unwrap_or_default();
        let best = p
            .bonks
            .iter()
            .max_by_key(|(id, n)| (**n, std::cmp::Reverse(**id)))
            .map_or(giver, |(&id, _)| id);
        if self.party_len() > 1 {
            for (&id, hero) in self.players.iter_mut() {
                hero.laurel = id == best;
            }
        }
        let name = self.players.get(&best).map_or_else(String::new, |h| h.name.to_uppercase());
        format!("MINUTES IN! {name} BONKED BEST.")
    }
}

pub fn settle(
    country: &mut Country,
    treasury: &mut Spoils,
    marks: &BTreeMap<String, u32>,
    news: &mut Vec<String>,
) {
    for (i, mark) in VISIT_MARKS.iter().enumerate() {
        if marks.contains_key(*mark) {
            country.aerie_visited |= 1 << i;
        }
    }
    (country.post, country.minutes) = progress(country, marks);
    if country.post == 2 && !country.post_paid {
        country.post_paid = true;
        treasury.add(Spoil::Gold, 75);
        treasury.add(Spoil::Wax, 2);
        news.push("Pigeon Parliament's Minutes: 75 gold, 2 wax.".into());
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_pigeons__tests.rs"]
mod tests;
