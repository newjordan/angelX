//! A village of the Deep: one great cavern, its people's houses along the
//! walls, its heart in the plaza (a people's own: a giant glowing cap, a
//! great crystal, a forge, a lamp on the lake), the wishing place, the
//! stair on down and the light home.
//!
//! The party arrives as a star falls: for `FALL` ticks nobody moves while
//! the streak comes down out of the cavern's dark and lands in the plaza,
//! and the village gathers to look. The village starts dim. Every kindness
//! lights it a little more (`hope`): the star's own landing, each of the
//! six heard for the first time, each errand taken, each wish made for the
//! city. At `HOPE_FULL` the whole village shines, and the realm keeps it
//! lit: a lit village is a landing of the Winding Stair from then on.
//!
//! E beside one of the six opens a conversation on the knight's own
//! screen: what they say, and what can be said back with 1, 2 and 3. Walk
//! away and it closes.

use super::cultures::{self, Folk, TALKER, WISHKEEPER};
use super::*;

/// The cavern, in tiles.
pub const COLS: usize = 36;
pub const ROWS: usize = 23;
/// The cavern's back wall: these rows are rock and the dark above it, the
/// sky the star falls out of.
pub const BACK: usize = 7;
/// The houses along the walls (blocks), the great hall at the top, the
/// heart in the plaza and the wishing place.
pub const HALL: (i32, i32, i32, i32) = (15, 7, 6, 3);
pub const HOUSES: [(i32, i32, i32, i32); 8] = [
    (2, 7, 5, 3),
    (9, 7, 4, 3),
    (23, 7, 4, 3),
    (29, 7, 5, 3),
    (2, 13, 4, 3),
    (30, 13, 4, 3),
    (5, 18, 5, 3),
    (26, 18, 5, 3),
];
pub const HEART: (i32, i32, i32, i32) = (17, 15, 2, 2);
pub const WELL: (i32, i32, i32, i32) = (11, 15, 2, 2);
/// The stair on down, at the plaza's south.
pub const STAIRS: (i32, i32) = (17, 19);
/// Where the star lands (tile point): the plaza, under the cavern's dark.
pub const LANDING: (f32, f32) = (18.0, 11.1);
/// The light home: step into it and the party goes up with everything.
pub const LIFT: (f32, f32) = (32.5, 19.5);
/// The six who speak, by place (tile points, their feet): the elder at
/// the great hall's door, three with errands at theirs, one who talks by
/// the heart, the wishkeeper by the wishing place.
pub const SPEAKERS: [(f32, f32); 6] = [(20.6, 10.9), (4.5, 10.9), (25.0, 10.9), (4.0, 16.9), (23.0, 17.2), (12.0, 17.9)];
/// How near (arena units) a knight stands to speak.
pub const SPEAK_REACH: f32 = 3.4;
/// The village's lamps on their posts, lit in this order as hope grows.
pub const LAMPS: [(f32, f32); 10] = [
    (14.5, 11.0),
    (21.5, 11.0),
    (8.0, 11.4),
    (28.0, 11.4),
    (14.2, 16.6),
    (21.8, 16.6),
    (7.2, 16.4),
    (28.8, 16.4),
    (15.6, 18.6),
    (20.4, 18.6),
];
/// How long the star takes to fall, and how long the village gathers to
/// look at where it came down.
pub const FALL: u64 = 54;
pub const GATHER: u64 = 150;
/// Hope, when the whole village shines.
pub const HOPE_FULL: u8 = 8;
/// How long the one sent home takes to run to whoever asked for them, and
/// how long the two of them hold on.
pub const RUN_HOME: u64 = 40;
pub const REUNION: u64 = 150;

/// A knight's conversation with one of the six.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Talk {
    /// Whom, by place among the six.
    pub who: u8,
    /// 0: their greeting; 1: what they ask, or all they say after.
    pub page: u8,
    pub opened: u64,
}

/// The village at play.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Village {
    pub floor: u32,
    pub culture: Culture,
    /// The tick the star fell.
    pub arrived: u64,
    pub hope: u8,
    /// The six heard, a bit each.
    #[serde(default)]
    pub heard: u8,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub talks: BTreeMap<u32, Talk>,
    /// Knights whose wish scroll is open for the city.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub wishing: BTreeSet<u32>,
    /// The tick each wish slot's improvement rose, for its ceremony.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub raised: Vec<(u8, u64)>,
    /// The village cheers until this tick.
    #[serde(default)]
    pub cheer: u64,
    /// The tick the whole village came alight, once it has.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shone: Option<u64>,
    /// The tick the one the party sent home runs out to meet whoever
    /// asked for them, the first time the party comes back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reunion: Option<u64>,
    /// Steps of hope the road earned the village, given as the star lands.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub boon: u8,
    /// The village keeps the star's own light back: its people are wary.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub cold: bool,
    /// What the village says of the road as the star lands.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub news: String,
}

impl Village {
    pub fn valid(&self) -> bool {
        self.hope <= HOPE_FULL
            && self.boon <= 3
            && self.news.len() <= 400
            && self.talks.len() <= MAX_PLAYERS as usize
            && self.talks.iter().all(|(id, t)| (1..=MAX_PLAYERS).contains(id) && usize::from(t.who) < SPEAKERS.len() && t.page <= 1)
            && self.wishing.len() <= MAX_PLAYERS as usize
            && self.raised.len() <= city::SLOTS
    }

    /// Whether the star is still falling: nobody moves.
    pub fn falling(&self, tick: u64) -> bool {
        tick < self.arrived + FALL
    }

    /// How lit the village is, `0..=1`.
    pub fn light(&self) -> f32 {
        f32::from(self.hope) / f32::from(HOPE_FULL)
    }

    /// How many of its lamps burn.
    pub fn lamps_lit(&self) -> usize {
        (usize::from(self.hope) * LAMPS.len()).div_ceil(usize::from(HOPE_FULL))
    }

    /// When the folk are back at their doors after gathering to see the
    /// star come down.
    pub fn settled(&self) -> u64 {
        self.arrived + FALL + GATHER
    }
}

/// The cavern's room: rock round the walls and along the back, the houses
/// and the heart as blocks, the stair on down.
pub fn room(culture: Culture) -> Room {
    let mut room = Room::solid_rock((0, 0));
    room.kind = RoomKind::DeepVillage;
    room.regrow(COLS, ROWS);
    for row in 1..BACK {
        for col in 1..COLS - 1 {
            room.set(col, row, Tile::Wall);
        }
    }
    let mut blocks = HOUSES.to_vec();
    blocks.extend([HALL, HEART, WELL]);
    for rect in blocks {
        nooks::fill(&mut room, rect, Tile::Block);
    }
    // The lake people's plaza is a jetty over black water at its edges.
    if culture == Culture::Lampwick {
        for (c, r, w, h) in [(1, 11, 1, 2), (34, 11, 1, 2)] {
            nooks::fill(&mut room, (c, r, w, h), Tile::Hazard);
        }
    }
    for (dc, dr) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        room.set((STAIRS.0 + dc) as usize, (STAIRS.1 + dr) as usize, Tile::Stairs);
    }
    room.visited = true;
    room.cleared = true;
    room
}

/// A village's floor: the one cavern.
pub fn floor(culture: Culture, depth: u32) -> Floor {
    Floor {
        depth,
        world_revision: 0,
        pack: culture.kin(),
        rooms: vec![room(culture)],
        secret: None,
        open: None,
        altar_at: BTreeMap::new(),
        fort_admit: crate::rules::layout::FortAdmit::default(),
    }
}

/// Tile point `p` in arena units.
pub fn units((x, y): (f32, f32)) -> (f32, f32) {
    (x * TILE_UNITS, y * TILE_UNITS)
}

impl Run {
    /// The party has come down into the village on floor `floor`: the
    /// realm meets it (or meets it again), the star falls, the light home
    /// waits by the lift.
    pub(super) fn arrive_at_village(&mut self, floor: u32) {
        let culture = self.home.deep.culture_at(floor);
        let known = self.home.deep.town(floor).cloned();
        if known.is_none() && self.home.deep.towns.len() < MOST_TOWNS {
            self.home.deep.towns.push(Town::new(floor, culture));
        }
        let lit = known.as_ref().is_some_and(|t| t.lit);
        // The one the party sent home last time runs out to meet whoever
        // asked, the first time the party is back to see it.
        let home = known.as_ref().is_some_and(|t| t.deeds.contains(&Errand::Rescue) && !t.reunited);
        if home && let Some(town) = self.home.deep.town_mut(floor) {
            town.reunited = true;
        }
        // What the road brought: a gift or a letter delivered, the people's
        // regard (`road`).
        let (boon, cold, news) = self.road_news(floor, culture, lit);
        let village = Village {
            floor,
            culture,
            arrived: self.tick,
            // A village lit before is lit still; a new one starts dark, and
            // the star's landing is its first light.
            hope: if lit { HOPE_FULL } else { 0 },
            heard: 0,
            talks: BTreeMap::new(),
            wishing: BTreeSet::new(),
            raised: Vec::new(),
            cheer: 0,
            shone: lit.then_some(self.tick),
            reunion: home.then_some(self.tick + FALL + GATHER + HZ as u64),
            boon,
            cold,
            news,
        };
        if let Some(d) = &mut self.deep {
            d.village = Some(village);
            d.signs.clear();
        }
        let (x, y) = units(LANDING);
        let count = self.players.len().max(1) as f32;
        for (slot, hero) in self.players.values_mut().enumerate() {
            let offset = (slot as f32 - (count - 1.0) / 2.0) * 2.2;
            (hero.x, hero.y) = (x + offset, y);
        }
        self.light = Some(units(LIFT));
        self.cues.push(format!("deep_village:{}", culture.people().to_lowercase()));
        self.sounds.push("descend");
        self.found = Some((
            self.tick,
            0,
            format!("FLOOR {floor}: {} OF {}. A STAR FALLS INTO THE DARK.", culture.town(chapter(floor)), culture.people()),
        ));
    }

    /// The village the party stands in, as it leaves: for the results
    /// board (`floor_stats::Left`).
    pub(in crate::rules) fn leaving_village(&self) -> Option<super::super::floor_stats::Left> {
        let v = self.deep.as_ref()?.village.as_ref().filter(|_| self.in_deep_village())?;
        let asked: Vec<&Mission> = self.home.deep.missions.iter().filter(|m| m.village == v.floor).collect();
        let town = self.home.deep.town(v.floor);
        Some(super::super::floor_stats::Left {
            town: v.culture.town(chapter(v.floor)).into(),
            hope: v.hope,
            shines: v.hope >= HOPE_FULL,
            errands: asked.len().min(255) as u8,
            done: asked.iter().filter(|m| m.done()).count().min(255) as u8,
            wishes: town.map_or(0, |t| t.wishes.len().min(255) as u8),
            wish: v
                .raised
                .iter()
                .max_by_key(|(_, at)| *at)
                .and_then(|(slot, _)| town?.wishes.iter().find(|w| w.slot == *slot))
                .map(|w| (w.gift.name().to_string(), w.shown.to_uppercase())),
        })
    }

    /// The village's music: its people's songs.
    pub fn village_music(&self) -> Option<&'static [music::Track]> {
        let v = self.deep.as_ref()?.village.as_ref()?;
        self.in_deep_village().then(|| v.culture.music())
    }

    /// One tick in the village: the star lands, a press of E beside one of
    /// the six speaks with them, a knight who walks off ends the talk.
    pub(super) fn tick_village(&mut self, v: &mut Village, pressed: u8) {
        if self.tick == v.arrived + FALL {
            self.sounds.push("boss_fall");
            self.shake = self.shake.max(10);
            if !v.news.is_empty() {
                self.found = Some((self.tick, 0, v.news.clone()));
            }
            if !v.cold {
                self.raise_hope(v, 1);
            }
            // A village whose dread the party felled on the way down saw it
            // fall: it is a step brighter already.
            if self.home.deep.town(v.floor).is_some_and(|t| t.dread) {
                self.raise_hope(v, 1);
            }
            if v.boon > 0 {
                self.raise_hope(v, v.boon);
            }
        }
        if v.reunion.is_some_and(|t| self.tick == t + RUN_HOME) {
            self.sounds.push("wish_grant");
            self.cues.push("deep_reunion".into());
            v.cheer = v.cheer.max(self.tick + 2 * u64::from(HZ));
            self.found = Some((self.tick, 0, v.culture.rescued().1.into()));
        }
        if v.falling(self.tick) {
            return;
        }
        // Whoever has walked off has ended their talk.
        let gone: Vec<u32> = v
            .talks
            .iter()
            .filter(|(id, t)| !self.near_speaker(**id, usize::from(t.who)))
            .map(|(&id, _)| id)
            .collect();
        for id in gone {
            v.talks.remove(&id);
        }
        for id in self.nook_players() {
            if !nooks::has(pressed, id) {
                continue;
            }
            if let Some(talk) = v.talks.get(&id).cloned() {
                // E on an open talk: on to what they ask, then away.
                if talk.page == 0 {
                    v.talks.insert(id, Talk { page: 1, ..talk });
                } else {
                    v.talks.remove(&id);
                }
                continue;
            }
            let Some(who) = (0..SPEAKERS.len()).find(|&w| self.near_speaker(id, w)) else {
                continue;
            };
            v.talks.insert(id, Talk { who: who as u8, page: 0, opened: self.tick });
            self.sounds.push("play_card");
            if v.heard & 1 << who == 0 {
                v.heard |= 1 << who;
                self.raise_hope(v, 1);
            }
        }
    }

    /// Whether knight `id` stands within speaking reach of speaker `who`.
    fn near_speaker(&self, id: u32, who: usize) -> bool {
        let (sx, sy) = units(SPEAKERS[who]);
        self.players.get(&id).is_some_and(|h| h.hp > 0 && (h.x - sx).hypot(h.y - (sy - 1.0)) < SPEAK_REACH)
    }

    /// The village grows `n` brighter; full, it shines, and the realm keeps
    /// it lit.
    pub(super) fn raise_hope(&mut self, v: &mut Village, n: u8) {
        let before = v.hope;
        v.hope = v.hope.saturating_add(n).min(HOPE_FULL);
        if v.hope > before {
            self.sounds.push("wheel_land");
        }
        if v.hope == HOPE_FULL && before < HOPE_FULL {
            v.shone = Some(self.tick);
            v.cheer = self.tick + 3 * u64::from(HZ);
            if let Some(town) = self.home.deep.town_mut(v.floor) {
                town.lit = true;
            }
            self.cues.push("deep_village_lit".into());
            self.shake = self.shake.max(8);
            self.found = Some((
                self.tick,
                0,
                format!("{} SHINES. THE WINDING STAIR WILL FIND IT AGAIN.", v.culture.town(chapter(v.floor))),
            ));
        }
    }

    /// What knight `id`'s conversation shows: who, their words, and the
    /// answers on 1, 2 and 3.
    pub fn talk_view(&self, id: u32) -> Option<(String, String, Vec<String>)> {
        let v = self.deep.as_ref()?.village.as_ref()?;
        let talk = v.talks.get(&id)?;
        let folk = v.culture.folk();
        let who = usize::from(talk.who);
        let f = &folk[who];
        let asked = self.asked_of(v, who);
        let (words, answers) = if talk.page == 0 {
            // The elder has heard of the party from the road (`road`).
            let heard = if who == cultures::ELDER { self.road_greeting(v.culture) } else { "" };
            (format!("{heard}{}", f.hello), vec!["E: GO ON".to_string()])
        } else if who == WISHKEEPER {
            let wished = self.wished_here(v, id);
            let answers = if wished {
                vec!["YOUR WISH STANDS HERE ALREADY.".into()]
            } else {
                vec!["1: MAKE A WISH FOR THE CITY".into(), "2: NOT YET".into()]
            };
            (f.after.to_string(), answers)
        } else if let (Some(ask), None) = (&f.asks, asked) {
            (
                ask.why.to_string(),
                vec![format!("1: I'LL DO IT ({})", ask.title), "2: TELL ME ABOUT THIS PLACE".into(), "3: FAREWELL".into()],
            )
        } else if let Some(m) = asked {
            let state = if m.done() { "DONE. THANK YOU.".to_string() } else { m.line() };
            // The one who asked for their lost one, with them home again.
            let words = if m.done() && m.errand == Errand::Rescue { v.culture.rescued().1 } else { f.after };
            (words.to_string(), vec![state])
        } else {
            (f.after.to_string(), vec!["E: FAREWELL".into()])
        };
        Some((f.name.to_string(), words, answers))
    }

    /// The errand speaker `who` of village `v` has asked of the realm.
    fn asked_of(&self, v: &Village, who: usize) -> Option<&Mission> {
        self.home.deep.missions.iter().find(|m| m.village == v.floor && usize::from(m.giver) == who)
    }

    /// Knight `id` answers with key `pick` (1 to 3).
    pub(super) fn answer(&mut self, id: u32, pick: u8) {
        let Some(mut deep) = self.deep.take() else {
            return;
        };
        if let Some(mut v) = deep.village.take() {
            self.answer_in(&mut v, id, pick);
            deep.village = Some(v);
        }
        self.deep = Some(deep);
    }

    fn answer_in(&mut self, v: &mut Village, id: u32, pick: u8) {
        let Some(talk) = v.talks.get(&id).cloned() else {
            return;
        };
        if talk.page == 0 {
            return;
        }
        let who = usize::from(talk.who);
        let folk = v.culture.folk();
        if who == WISHKEEPER {
            if pick == 1 && !self.wished_here(v, id) {
                v.talks.remove(&id);
                self.begin_city_wish(v, id);
            } else if pick == 2 {
                v.talks.remove(&id);
            }
            return;
        }
        let asked = self.asked_of(v, who).is_some();
        match (pick, &folk[who].asks) {
            (1, Some(ask)) if !asked => {
                self.take_errand(v, who, ask);
                v.talks.remove(&id);
            }
            (2, Some(_)) if !asked => {
                // A word about the place: the one who only talks says it.
                let lore: &Folk = &folk[TALKER];
                self.found = Some((self.tick, id, lore.after.to_string()));
            }
            (3, _) => {
                v.talks.remove(&id);
            }
            _ => {}
        }
    }

    /// The realm takes speaker `who`'s errand for the chapter below.
    fn take_errand(&mut self, v: &mut Village, who: usize, ask: &cultures::Ask) {
        if self.home.deep.missions.len() >= MOST_MISSIONS {
            // The oldest finished errand makes room.
            if let Some(i) = self.home.deep.missions.iter().position(Mission::done) {
                self.home.deep.missions.remove(i);
            } else {
                return;
            }
        }
        self.home.deep.missions.push(Mission {
            errand: ask.errand,
            village: v.floor,
            culture: v.culture,
            giver: who as u8,
            have: 0,
            need: ask.need,
            paid: false,
        });
        self.sounds.push("card_pickup");
        self.cues.push("deep_errand_taken".into());
        self.found = Some((self.tick, 0, format!("ERRAND TAKEN: {}. IT WAITS ON THE FLOORS BELOW.", ask.title)));
        self.raise_hope(v, 1);
    }
}
