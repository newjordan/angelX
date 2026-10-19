//! Little things to do in the kingdom. Walk up, press E, and something
//! happens: the chapel bell tolls for everyone, a coin in the village well
//! buys a fortune, a bench is a sit-down and a slow mend, a bucket is for
//! kicking, and a knight who bows to a friend who bows back gets a huzzah.
//! Each is small, quick and a little silly; none pays anything but smiles.
use super::*;
use super::map;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Thing {
    Bell,
    Well,
    Bench,
    Bucket,
    Stocks,
    Goose,
    /// A coach post: E rides to the next post round the realm.
    Coach,
    /// The town crier: E and he cries an errand that wants doing, and the
    /// goal arrow points the way.
    Crier,
}

impl Thing {
    /// What E does, in a word, for the hint beside it.
    pub fn verb(self) -> &'static str {
        match self {
            Thing::Bell => "RING",
            Thing::Well => "WISH",
            Thing::Bench => "SIT",
            Thing::Bucket => "KICK",
            Thing::Stocks => "LOCK IN",
            Thing::Goose => "HONK",
            Thing::Coach => "RIDE",
            Thing::Crier => "LISTEN",
        }
    }
}

/// Which thing stands beside which place's door.
const PLACED: [(Thing, &str); 16] = [
    (Thing::Bell, "Chapel"),
    (Thing::Well, "Village"),
    (Thing::Bench, "Keep"),
    (Thing::Bench, "Village"),
    (Thing::Bench, "Artisan Quarter"),
    (Thing::Bucket, "Village"),
    (Thing::Bucket, "WASHBUCKET QUAY"),
    (Thing::Crier, "Keep"),
    (Thing::Stocks, "Village"),
    (Thing::Goose, "Picnic Green"),
    // The coach round, in riding order; the last runs back to the first.
    (Thing::Coach, "Keep"),
    (Thing::Coach, "Village"),
    (Thing::Coach, "Horse Ranch"),
    (Thing::Coach, "Colosseum"),
    (Thing::Coach, "Labyrinth College"),
    (Thing::Coach, "Delve"),
];

/// The next coach post after post `i` (an index into `things`), its spot
/// and the place it stands by.
pub fn next_coach(geo: &map::Geography, i: usize) -> Option<(usize, (i32, i32), &'static str)> {
    let all = placed(geo);
    let posts: Vec<usize> = (0..all.len()).filter(|&k| all[k].0 == Thing::Coach).collect();
    let at = posts.iter().position(|&k| k == i)?;
    let next = posts[(at + 1) % posts.len()];
    Some((next, all[next].2, all[next].1))
}

/// How long an honked goose gives chase, and how fast (units a second).
pub const GOOSE_CHASE: u64 = 3 * HZ as u64;
pub const GOOSE_SPEED: f32 = 15.0;

/// How near (tiles) a knight stands to use a thing.
pub const REACH: f32 = 1.5;
/// A bench mends one point every this many ticks.
pub const BENCH_MEND: u64 = HZ as u64;
/// Two bows within this many ticks are a huzzah.
pub const BOW_WINDOW: u64 = 2 * HZ as u64;
/// How long a thing's moment shows (a toll's rings, a bucket's flight).
pub const MOMENT: u64 = HZ as u64;

const FORTUNES: [&str; 8] = [
    "A GOOSE WILL RESPECT YOU",
    "YOUR NEXT ARROW IS LUCKY",
    "BEWARE OF SOUP",
    "A DRAGON THINKS OF YOU",
    "YOU WILL FIND A SOCK",
    "THE PRIVY FAVOURS YOU",
    "GOLD, BUT LATER",
    "SIT DOWN MORE",
];

/// Every thing on the kingdom map (tiles): open ground beside its place's
/// door, clear of every door and every class banner.
pub fn things(geo: &map::Geography) -> Vec<(Thing, (i32, i32))> {
    placed(geo).into_iter().map(|(thing, _, at)| (thing, at)).collect()
}

/// Every thing with the place it stands beside.
pub fn placed(geo: &map::Geography) -> Vec<(Thing, &'static str, (i32, i32))> {
    let banners: Vec<(i32, i32)> = super::classes::stones(geo).into_iter().map(|(_, p)| p).collect();
    let mut placed: Vec<(Thing, &'static str, (i32, i32))> = Vec::new();
    for &(thing, place) in &PLACED {
        let Some(site) = geo.sites.iter().find(|s| s.name == place) else {
            continue;
        };
        let (x, y) = site.stand;
        let open = |(px, py): (i32, i32), placed: &[(Thing, &'static str, (i32, i32))]| {
            (0..geo.room.cols as i32).contains(&px)
                && (0..geo.room.rows as i32).contains(&py)
                && geo.room.tile(px, py) == Tile::Floor
                && geo.sites.iter().all(|s| (s.stand.0 - px).abs() + (s.stand.1 - py).abs() > 2)
                && banners.iter().chain(placed.iter().map(|(_, _, p)| p)).all(|b| (b.0 - px).abs() + (b.1 - py).abs() > 3)
        };
        let spot = [(-3, 1), (3, 1), (-4, 1), (4, 1), (-2, 2), (2, 2), (-5, 0), (5, 0), (0, 3), (-3, 3), (3, 3)]
            .into_iter()
            .chain((-8..=8).flat_map(|dx| (-2..=6).map(move |dy| (dx, dy))))
            .map(|(dx, dy)| (x + dx, y + dy))
            .find(|&p| open(p, &placed));
        if let Some(p) = spot {
            placed.push((thing, place, p));
        }
    }
    placed
}

/// What the kingdom's things are doing: when each was last used (by its
/// index in `things`), and each knight's last bow.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Moments {
    pub used: BTreeMap<u8, u64>,
    pub bows: BTreeMap<u32, u64>,
    /// The last fortune the well told, for its board.
    pub fortune: u8,
    /// The goose, when honked at: who it is after and since when, and where
    /// it has got to (units).
    pub goose: Option<(u32, u64)>,
    pub goose_at: Option<(i32, i32)>,
    /// The errand the crier last cried: its place, until the party gets
    /// there; and which errand he cries next.
    pub errand: Option<RoomKind>,
    pub cried: u8,
}

impl Moments {
    pub fn valid(&self) -> bool {
        self.used.len() <= PLACED.len()
            && self.used.keys().all(|&i| usize::from(i) < PLACED.len())
            && self.bows.len() <= MAX_PLAYERS as usize
            && self.bows.keys().all(|id| (1..=MAX_PLAYERS).contains(id))
            && usize::from(self.fortune) < FORTUNES.len()
            && self.goose.is_none_or(|(id, _)| (1..=MAX_PLAYERS).contains(&id))
    }
    /// The fortune the well last told.
    pub fn told(&self) -> &'static str {
        FORTUNES[usize::from(self.fortune) % FORTUNES.len()]
    }
}

impl Run {
    /// A fresh press of E by knight `id` that no banner took: a thing beside
    /// them, or a bow. Returns whether anything happened.
    pub(super) fn use_trinket(&mut self, id: u32, geo: &map::Geography) -> bool {
        let Some(hero) = self.players.get(&id) else {
            return false;
        };
        let (hx, hy) = (hero.x, hero.y);
        let near = |(tx, ty): (i32, i32)| {
            (hx - (tx as f32 + 0.5) * TILE_UNITS).hypot(hy - (ty as f32 + 0.5) * TILE_UNITS)
                < REACH * TILE_UNITS
        };
        let tick = self.tick;
        let take = (tick % 2) as usize;
        if let Some((i, &(thing, _))) = things(geo).iter().enumerate().find(|(_, (_, p))| near(*p)) {
            self.kingdom.moments.used.insert(i as u8, tick);
            match thing {
                Thing::Bell => {
                    self.sounds.push("circle_toll");
                    self.fx.push(Fx::Shake(2));
                    self.found = Some((tick, 0, "DONG!".into()));
                }
                Thing::Well => {
                    self.sounds.push("gold");
                    self.kingdom.moments.fortune = (tick % FORTUNES.len() as u64) as u8;
                    let said = self.kingdom.moments.told();
                    self.found = Some((tick, id, said.into()));
                    self.sounds.push(super::chatter::clip(id, "ooh", take));
                }
                Thing::Bench => {
                    if let Some(hero) = self.players.get_mut(&id) {
                        hero.sitting = true;
                    }
                    self.sounds.push(super::chatter::clip(id, "aww", take));
                }
                Thing::Bucket => {
                    self.sounds.push("clang");
                    self.sounds.push(super::chatter::clip(id, "hehe", take));
                    self.found = Some((tick, 0, "OI! MY BUCKET!".into()));
                }
                Thing::Stocks => {
                    let (sx, sy) = things(geo)[i].1;
                    if let Some(hero) = self.players.get_mut(&id) {
                        hero.stocked = !hero.stocked;
                        hero.splats = 0;
                        if hero.stocked {
                            (hero.x, hero.y) = ((sx as f32 + 0.5) * TILE_UNITS, (sy as f32 + 0.6) * TILE_UNITS);
                        }
                    }
                    self.sounds.push("door_seal");
                    self.sounds.push(super::chatter::clip(id, "uhoh", take));
                }
                Thing::Goose => {
                    self.kingdom.moments.goose = Some((id, tick));
                    self.sounds.push(super::chatter::clip(id, "uhoh", take));
                    self.found = Some((tick, 0, "HONK!".into()));
                }
                Thing::Crier => {
                    let open = self.open_errands();
                    if open.is_empty() {
                        self.kingdom.moments.errand = None;
                        self.found = Some((tick, id, "OYEZ! ALL IS WELL!".into()));
                    } else {
                        let k = usize::from(self.kingdom.moments.cried) % open.len();
                        let (room, line) = open[k];
                        self.kingdom.moments.cried = self.kingdom.moments.cried.wrapping_add(1);
                        self.kingdom.moments.errand = Some(room);
                        self.found = Some((tick, id, format!("OYEZ! {line}")));
                    }
                    self.sounds.push("circle_toll");
                }
                Thing::Coach => {
                    if let Some((next, (cx, cy), place)) = next_coach(geo, i) {
                        if let Some(hero) = self.players.get_mut(&id) {
                            (hero.x, hero.y) = ((cx as f32 + 0.5) * TILE_UNITS, (cy as f32 + 0.6) * TILE_UNITS);
                            hero.camera = Some((hero.x, hero.y - 1.0));
                            hero.sitting = false;
                        }
                        self.kingdom.moments.used.insert(next as u8, tick);
                        self.sounds.push("roll");
                        self.sounds.push(super::chatter::clip(id, "go", take));
                        self.found = Some((tick, id, format!("TO {}!", place.to_uppercase())));
                    }
                }
            }
            return true;
        }
        if self.use_find(id) {
            return true;
        }
        // A bow to the nearest friend in reach; a bow back is a huzzah.
        let friend = self
            .players
            .iter()
            .filter(|&(&other, h)| other != id && h.hp > 0 && (h.x - hx).hypot(h.y - hy) < 2.5 * TILE_UNITS)
            .map(|(&other, _)| other)
            .next();
        let Some(friend) = friend else {
            return false;
        };
        self.kingdom.moments.bows.insert(id, tick);
        self.sounds.push(super::chatter::clip(id, "hello", take));
        if self
            .kingdom
            .moments
            .bows
            .get(&friend)
            .is_some_and(|&at| tick.saturating_sub(at) <= BOW_WINDOW && at != tick)
        {
            self.kingdom.moments.bows.clear();
            self.sounds.push("wish_grant");
            self.sounds.push(super::chatter::clip(friend, "cheer", take));
            self.found = Some((tick, 0, "HUZZAH!".into()));
            self.score = self.score.saturating_add(5);
        }
        true
    }

    /// Benches mend a sitter slowly; any step stands them up. The stocks
    /// let go off the surface. An honked goose gives chase.
    pub(super) fn tick_sitters(&mut self, inputs: &BTreeMap<u32, Input>) {
        let mend = self.tick % BENCH_MEND == 0;
        let kingdom = self.room().kind == RoomKind::Kingdom;
        if !kingdom {
            for hero in self.players.values_mut() {
                hero.stocked = false;
            }
            self.kingdom.moments.goose = None;
            self.kingdom.moments.goose_at = None;
        } else {
            self.tick_goose();
        }
        for (id, hero) in &mut self.players {
            if !hero.sitting {
                continue;
            }
            let moving = inputs.get(id).is_some_and(|i| i.valid() && (i.move_x != 0 || i.move_y != 0));
            if moving || !kingdom || hero.hp == 0 {
                hero.sitting = false;
            } else if mend {
                hero.hp = (hero.hp + 1).min(hero.max_hp);
            }
        }
    }
}

impl Run {
    /// The run's finds out in the open country that stand clear of the
    /// party's wishes (a wish raised on one hides it), with their index.
    pub fn standing_finds(&self) -> Vec<(usize, crate::world::finds::Find)> {
        let lots = crate::world::wishes::lots(&self.kingdom.wishes);
        crate::world::finds::finds(self.seed)
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, f)| !lots.iter().flatten().any(|l| l.contains(f.at.0, f.at.1)))
            .collect()
    }

    /// E by one of the run's finds (`crate::world::finds`): a shrine mends,
    /// a ruin or a cave gives up its cache once, a wanderer tells of a find
    /// nobody has made, stones hum, a lost pack holds a card or a
    /// consumable, a hermit trades a scroll for a potion, a well takes a
    /// coin of renown for a small boon (each once a run). Returns whether
    /// there was one.
    fn use_find(&mut self, id: u32) -> bool {
        use crate::world::finds::{CACHE, COIN, Kind, REACH};
        let Some(hero) = self.players.get(&id) else {
            return false;
        };
        let (hx, hy) = (hero.x, hero.y);
        let all = self.standing_finds();
        let Some(&(i, find)) = all.iter().find(|(_, f)| {
            (hx - (f.at.0 as f32 + 0.5) * TILE_UNITS).hypot(hy - (f.at.1 as f32 + 0.5) * TILE_UNITS) < REACH * TILE_UNITS
        }) else {
            return false;
        };
        let tick = self.tick;
        let first = self.kingdom.found & (1 << i) == 0;
        // Whether this use counts as the find made (a hermit with nothing
        // to trade, a well with nothing to give, are tried again).
        let mut made = true;
        let line = match find.kind {
            Kind::Shrine => {
                if let Some(hero) = self.players.get_mut(&id) {
                    hero.hp = hero.max_hp;
                }
                self.sounds.push("wish_grant");
                "THE SHRINE IS QUIET. YOU ARE MENDED".to_string()
            }
            Kind::Ruin | Kind::Cave if first => {
                self.score = self.score.saturating_add(CACHE);
                self.sounds.push("gold");
                if find.kind == Kind::Ruin {
                    format!("A CACHE UNDER THE STONES: {CACHE} RENOWN")
                } else {
                    format!("SOMETHING GLINTS IN THE DARK: {CACHE} RENOWN")
                }
            }
            Kind::Ruin => "ONLY OLD STONES NOW".to_string(),
            Kind::Cave => "ONLY BATS NOW".to_string(),
            Kind::Campfire => {
                self.sounds.push(super::chatter::clip(id, "ooh", (tick % 2) as usize));
                let found = self.kingdom.found;
                match all.iter().find(|(k, f)| found & (1 << k) == 0 && f.kind != Kind::Campfire) {
                    Some((_, f)) => format!("A WANDERER: {} {}", f.kind.told(), f.biome.name().to_uppercase()),
                    None => "A WANDERER: YOU HAVE SEEN ALL I KNOW".to_string(),
                }
            }
            Kind::Stones => {
                self.kingdom.woke = Some((i as u8, tick));
                self.sounds.push("circle_toll");
                "THE STONES HUM".to_string()
            }
            Kind::LostPack if first => {
                // What the pack held rests on where it lies.
                let which = (find.at.0 + 3 * find.at.1).rem_euclid(3);
                let Some(hero) = self.players.get_mut(&id) else {
                    return false;
                };
                let gift = match which {
                    0 if hero.hand.len() < cards::HAND => {
                        hero.hand.push("potion".into());
                        "A POTION"
                    }
                    1 if hero.hand.len() < cards::HAND => {
                        hero.hand.push("thunder-scroll".into());
                        "A THUNDER SCROLL"
                    }
                    _ if hero.bombs < MAX_BOMBS => {
                        hero.bombs += 1;
                        "A BOMB"
                    }
                    _ => {
                        self.score = self.score.saturating_add(CACHE);
                        "A PURSE OF RENOWN"
                    }
                };
                self.sounds.push("gold");
                format!("A LOST PACK: {gift}")
            }
            Kind::LostPack => "THE PACK IS EMPTY NOW".to_string(),
            Kind::Hermit if first => {
                let Some(hero) = self.players.get_mut(&id) else {
                    return false;
                };
                if let Some(at) = hero.hand.iter().position(|c| c == "potion") {
                    hero.hand[at] = "thunder-scroll".into();
                    self.sounds.push("gold");
                    self.sounds.push(super::chatter::clip(id, "ooh", (tick % 2) as usize));
                    "THE HERMIT TRADES A THUNDER SCROLL FOR YOUR POTION".to_string()
                } else {
                    made = false;
                    "THE HERMIT WANTS A POTION FOR HIS THUNDER SCROLL".to_string()
                }
            }
            Kind::Hermit => "THE HERMIT HAS NOTHING MORE TO TRADE".to_string(),
            Kind::Well if first => {
                let Some(hero) = self.players.get_mut(&id) else {
                    return false;
                };
                // A mend, a potion or a bomb, in turn from where it stands,
                // whichever the knight can use; none, and no coin is taken.
                let mut boon = None;
                for k in 0..3 {
                    boon = match (i + k) % 3 {
                        0 if hero.hp < hero.max_hp => Some("YOU ARE MENDED"),
                        1 if hero.hand.len() < cards::HAND => Some("A POTION"),
                        2 if hero.bombs < MAX_BOMBS => Some("A BOMB"),
                        _ => None,
                    };
                    if boon.is_some() {
                        break;
                    }
                }
                if self.score < COIN {
                    made = false;
                    format!("THE WELL WANTS A COIN: {COIN} RENOWN")
                } else if let Some(boon) = boon {
                    self.score -= COIN;
                    match boon {
                        "YOU ARE MENDED" => hero.hp = hero.max_hp,
                        "A POTION" => hero.hand.push("potion".into()),
                        _ => hero.bombs += 1,
                    }
                    self.sounds.push("wish_grant");
                    format!("A COIN DOWN THE WELL: {boon}")
                } else {
                    made = false;
                    "THE WELL HAS NOTHING TO GIVE YOU".to_string()
                }
            }
            Kind::Well => "THE WELL IS QUIET NOW".to_string(),
        };
        if made {
            self.kingdom.found |= 1 << i;
        }
        self.found = Some((tick, id, line));
        true
    }

    /// Every countryside errand that still wants doing: its place, and what
    /// the crier calls about it.
    pub fn open_errands(&self) -> Vec<(RoomKind, &'static str)> {
        use super::countryside::{self as c, FORECAST, LAMPS, PICNIC, RELICS, SURVEY};
        let country = &self.home.countryside;
        let mut open = Vec::new();
        // The dwarf king's first mission sends the knights to his Mines.
        let barony = &self.home.barony;
        if matches!(barony.court, super::barony::Court::Met | super::barony::Court::Sworn)
            && !barony.done.iter().any(|m| m == "workings")
        {
            open.push((RoomKind::MineHead, "THE KING WANTS HIS MINES"));
        }
        for (i, relic) in RELICS.iter().enumerate() {
            if country.relics & (1 << i) == 0 {
                open.push((relic.room, relic.clue));
            }
        }
        if country.can_light() {
            for (i, lamp) in LAMPS.iter().enumerate() {
                if country.lamps & (1 << i) == 0 {
                    open.push((lamp.room, "A LANTERN WANTS LIGHTING"));
                }
            }
        }
        for (i, &(room, _)) in SURVEY.iter().enumerate() {
            if country.survey & (1 << i) == 0 {
                open.push((room, "THE SURVEYOR WANTS A NAME"));
            }
        }
        let picnic = usize::from(c::picnic_stage(country, &self.marks));
        if let Some(&(room, _)) = PICNIC.get(picnic) {
            open.push((room, "THE TROLL WANTS LUNCH"));
        }
        let forecast = usize::from(c::forecast_stage(country, &self.marks));
        if let Some(&(room, _)) = FORECAST.get(forecast).filter(|_| forecast < 5) {
            open.push((room, "THE WEATHER CLERK IS STUCK"));
        }
        // The scarecrow on strike, then each piece of his still out.
        let (wardrobe, found) = super::bowcountry::progress(country, &self.marks);
        match wardrobe {
            0 => open.push((RoomKind::Scarecrow, "THE SCARECROW HAS QUIT")),
            1 if found == 7 => open.push((RoomKind::Scarecrow, "THE SCARECROW WANTS DRESSING")),
            1 => {
                for (i, &(room, _, _)) in super::bowcountry::CLOTHES.iter().enumerate() {
                    if found & (1 << i) == 0 {
                        open.push((room, super::bowcountry::WANTED[i]));
                    }
                }
            }
            _ => {}
        }
        open
    }
}

impl Run {
    /// The goose's own spot by its green, where it waddles when nobody has
    /// honked at it.
    fn goose_home(&self) -> Option<(f32, f32)> {
        let geo = map::geography(&self.kingdom.wishes);
        let (_, (x, y)) = things(&geo).into_iter().find(|(t, _)| *t == Thing::Goose)?;
        let phase = (self.tick % (6 * HZ as u64)) as f32 / (6 * HZ) as f32 * std::f32::consts::TAU;
        Some(((x as f32 + 0.5 + phase.cos()) * TILE_UNITS, (y as f32 + 0.5 + phase.sin() * 0.5) * TILE_UNITS))
    }

    /// Chasing: straight at the knight who honked, and a peck that shoves
    /// them when it catches up; then back to its green.
    fn tick_goose(&mut self) {
        let Some(home) = self.goose_home() else {
            return;
        };
        let at = self
            .kingdom
            .moments
            .goose_at
            .map_or(home, |(x, y)| (x as f32 / 16.0, y as f32 / 16.0));
        let chase = self
            .kingdom
            .moments
            .goose
            .filter(|&(_, since)| self.tick.saturating_sub(since) < GOOSE_CHASE)
            .and_then(|(id, _)| self.players.get(&id).map(|h| (id, h.x, h.y)));
        let (tx, ty) = chase.map_or(home, |(_, x, y)| (x, y));
        let (dx, dy) = (tx - at.0, ty - at.1);
        let d = dx.hypot(dy);
        let step = GOOSE_SPEED * DT;
        let next = if d <= step { (tx, ty) } else { (at.0 + dx / d * step, at.1 + dy / d * step) };
        if let Some((id, x, y)) = chase
            && (next.0 - x).hypot(next.1 - y) < HERO_RADIUS + 0.5
        {
            // A peck: the knight hops a pace away, and the goose is satisfied.
            if let Some(hero) = self.players.get_mut(&id) {
                let (ux, uy) = if d > 0.01 { (dx / d, dy / d) } else { (1.0, 0.0) };
                let geo = map::geography(&self.kingdom.wishes);
                let to = (x + ux * TILE_UNITS, y + uy * TILE_UNITS);
                if geo.room.tile((to.0 / TILE_UNITS) as i32, (to.1 / TILE_UNITS) as i32) == Tile::Floor {
                    (hero.x, hero.y) = to;
                }
                hero.sitting = false;
            }
            self.kingdom.moments.goose = None;
            self.sounds.push("hit");
            self.sounds.push(super::chatter::clip(id, "oof", 0));
            self.found = Some((self.tick, 0, "HONK! PECKED!".into()));
        }
        self.kingdom.moments.goose_at = Some(((next.0 * 16.0) as i32, (next.1 * 16.0) as i32));
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_trinkets__tests.rs"]
mod tests;
