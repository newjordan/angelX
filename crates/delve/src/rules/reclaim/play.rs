//! The run's half of the reclaiming: a knight's talk with the King and with
//! the keeper of a lost hall, and each hall's set-piece.
//!
//! E beside the King, or beside a keeper, opens a talk on the knight's own
//! screen: their greeting, then their ask, answered with 1, 2 and 3. Walk
//! away and it closes. What a talk or a set-piece settles is a mark
//! (`reclaim:*`); the realm's word on it comes back through the cockpit's
//! settle and `rebuild_home`.

use super::rooms::*;
use super::*;
use crate::rules::barony::{self, SEAT_PLATE};

/// How near a knight stands to talk (tiles).
pub const TALK_REACH: f32 = 3.0;
/// How long a knight holds E on a heap to dig it out.
pub const DIG_HOLD: u32 = HZ * 2;
/// The most monsters a hall's set-piece raises on top of its own, all told.
pub const RAISE_CAP: u8 = 6;
/// A mark the realm has not answered is sent again this often.
const RESEND: u64 = 10 * HZ as u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Who {
    King,
    Keeper,
}

/// A knight's conversation: with whom, and which page they are on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Talk {
    pub who: Who,
    /// 0: their greeting; 1: what they ask; 2: what they tell of the hall.
    pub page: u8,
    pub opened: u64,
}

/// What a knight's talk shows: who speaks, what they say, and the answers
/// on 1, 2 and 3.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TalkView {
    pub who: Who,
    pub name: String,
    pub tag: String,
    pub words: String,
    pub answers: Vec<String>,
}

/// A lost hall's set-piece at play.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HallPlay {
    pub room: usize,
    /// Which hall, by place in `HALLS`.
    pub hall: u8,
    /// 0: the keeper waits; 1: under way; 2: played out.
    pub stage: u8,
    /// How it was won: `fight`, `bargain`, `rite` or `freed`.
    pub how: String,
    /// The tick its mark was last sent.
    pub sent: u64,
    /// The party is in the hall now.
    pub inside: bool,
    /// The fight: who was raised (a body's id, and whether it is the
    /// chief), how many guards have fallen, whether the chief has.
    pub foes: Vec<(u32, bool)>,
    pub slain: u8,
    pub chief_down: bool,
    /// The rite: the lamps kindled so far, in order.
    pub lit: Vec<u8>,
    /// The dig: the heaps dug out (a bit each).
    pub dug: u8,
    /// Monsters raised by the set-piece, all told.
    pub raised: u8,
}

/// The run's reclaiming at play.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Play {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub talks: BTreeMap<u32, Talk>,
    /// Knights holding E now: a press is a new bit.
    #[serde(default, skip_serializing_if = "no_bits")]
    pub held: u8,
    /// The floor's lost hall, once the party is in it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hall: Option<HallPlay>,
}

impl Play {
    pub fn is_empty(&self) -> bool {
        self.talks.is_empty() && self.held == 0 && self.hall.is_none()
    }
}

fn no_bits(bits: &u8) -> bool {
    *bits == 0
}

fn has(bits: u8, id: u32) -> bool {
    bits & 1 << (id & 7) != 0
}

fn inside((col, row, w, h): (i32, i32, i32, i32), x: f32, y: f32) -> bool {
    let (c, r) = (x / TILE_UNITS, y / TILE_UNITS);
    c >= col as f32 && c < (col + w) as f32 && r >= row as f32 && r < (row + h) as f32
}

impl Run {
    /// The floor's lost hall set-piece, while the party is in its room.
    pub fn hall_play(&self) -> Option<&HallPlay> {
        self.reclaim
            .hall
            .as_ref()
            .filter(|p| p.room == self.at && self.room().kind == RoomKind::DwarfHall)
    }

    /// Whether the lost hall of this room is the King's already.
    pub fn hall_held(&self) -> bool {
        hall_of(self.dungeon.pack).is_some_and(|(_, h)| self.home.barony.halls.is_held(h.id))
    }

    /// Knight `id` is within reach of tile point `at`.
    fn reclaim_near(&self, id: u32, at: (f32, f32), reach: f32) -> bool {
        self.players
            .get(&id)
            .is_some_and(|h| h.hp > 0 && !h.stone && (h.x - at.0 * TILE_UNITS).hypot(h.y - at.1 * TILE_UNITS) < reach * TILE_UNITS)
    }

    fn reclaim_standing(&self, id: u32, area: (i32, i32, i32, i32)) -> bool {
        self.players.get(&id).is_some_and(|h| h.hp > 0 && !h.stone && inside(area, h.x, h.y))
    }

    /// The keeper's spot now, if there is one in this room.
    pub fn keeper_at(&self) -> Option<(f32, f32)> {
        let play = self.hall_play()?;
        let (i, _) = hall_of(self.dungeon.pack)?;
        // A freed smith stands; a hall's fight has no keeper to talk to.
        let present = match (i, play.stage) {
            (0, 1) => false,
            (0, 2) => self.home.barony.halls.how("gallery") != Some("fight") || self.hall_held(),
            _ => true,
        };
        present.then_some(KEEPER_AT[i])
    }

    /// Who knight `id` could speak to where they stand.
    fn speaker_near(&self, id: u32) -> Option<Who> {
        let b = &self.home.barony;
        if b.court >= Court::Met
            && self.at_home_now()
            && let Some((room, at)) = barony::king_at(b)
            && room == self.room().kind
            && self.reclaim_near(id, at, TALK_REACH)
            && !self.reclaim_standing(id, barony::HALL_PLATE)
            && !self.reclaim_standing(id, SEAT_PLATE)
        {
            return Some(Who::King);
        }
        let at = self.keeper_at()?;
        // Heaps are dug with E, not talked over.
        let digging = hall_of(self.dungeon.pack).is_some_and(|(i, _)| i == 2) && DIGS.iter().any(|d| self.reclaim_standing(id, *d));
        (self.reclaim_near(id, at, TALK_REACH) && !digging).then_some(Who::Keeper)
    }

    /// The keys a knight in talk gives up before the rest of the tick sees
    /// them: 1, 2 and 3 answer rather than cast.
    pub(in crate::rules) fn reclaim_inputs(&mut self, inputs: &BTreeMap<u32, Input>) -> Option<BTreeMap<u32, Input>> {
        if self.reclaim.talks.is_empty() {
            return None;
        }
        let mut out = inputs.clone();
        let mut changed = false;
        for (&id, input) in out.iter_mut() {
            if input.valid() && input.cast != 0 && self.reclaim.talks.contains_key(&id) {
                let pick = input.cast;
                input.cast = 0;
                changed = true;
                self.reclaim_answer(id, pick);
            }
        }
        changed.then_some(out)
    }

    /// One tick of the reclaiming: talks, then the hall the party is in.
    pub(in crate::rules) fn tick_reclaim(&mut self, inputs: &BTreeMap<u32, Input>) {
        let held = self
            .players
            .keys()
            .filter(|id| inputs.get(id).is_some_and(|i| i.valid() && i.interact))
            .fold(0u8, |bits, &id| bits | 1 << (id & 7));
        let pressed = held & !self.reclaim.held;
        self.reclaim.held = held;
        // Whoever has walked off has ended their talk.
        let gone: Vec<u32> = self
            .reclaim
            .talks
            .iter()
            .filter(|&(&id, t)| self.speaker_near(id) != Some(t.who))
            .map(|(&id, _)| id)
            .collect();
        for id in gone {
            self.reclaim.talks.remove(&id);
        }
        let ids: Vec<u32> = self.players.keys().copied().collect();
        for id in ids {
            if !has(pressed, id) {
                continue;
            }
            if let Some(talk) = self.reclaim.talks.get(&id).cloned() {
                self.talk_on(id, talk);
            } else if let Some(who) = self.speaker_near(id) {
                self.reclaim.talks.insert(id, Talk { who, page: 0, opened: self.tick });
                self.sounds.push("play_card");
            }
        }
        self.tick_hall(inputs, pressed);
    }

    /// E on an open talk: on to what they ask; at the ask, away when there is
    /// nothing to answer; from the hall's lore, back to the ask.
    fn talk_on(&mut self, id: u32, talk: Talk) {
        let numbered = self
            .reclaim_view(id)
            .is_some_and(|v| v.answers.first().is_some_and(|a| a.as_bytes().first().is_some_and(u8::is_ascii_digit)));
        let rite_begins = talk.who == Who::Keeper && talk.page == 1 && !numbered;
        match talk.page {
            0 => {
                self.reclaim.talks.insert(id, Talk { page: 1, ..talk });
            }
            2 => {
                self.reclaim.talks.insert(id, Talk { page: 1, ..talk });
            }
            _ if numbered => {}
            _ => {
                self.reclaim.talks.remove(&id);
                if rite_begins {
                    self.begin_set_piece();
                }
            }
        }
    }

    /// What knight `id`'s talk shows.
    pub fn reclaim_view(&self, id: u32) -> Option<TalkView> {
        let talk = self.reclaim.talks.get(&id)?;
        Some(match talk.who {
            Who::King => self.king_view(talk.page),
            Who::Keeper => self.keeper_view(talk.page)?,
        })
    }

    fn king_view(&self, page: u8) -> TalkView {
        let b = &self.home.barony;
        let halls = &b.halls;
        let held = halls.count();
        let (words, answers) = match page {
            0 => {
                let words = match held {
                    0 if b.court == Court::Sworn => "Ye're welcome in me own hall, knights. It stands, and me dwarves with it. But under it the goblins still sit in the halls me kin dug, hall below hall. Help me take them back.".to_string(),
                    0 => "Knights! Ye've the look of folk who'd keep digging. Caer Dwfn ran down under this mountain, hall below hall, and goblins sit in every one of them. Help me take them back.".to_string(),
                    1 => "The Delvers' Gallery is mine again, and me grandsire's chair has no goblin in it. Ye've done in a night what me axe couldn't in a hundred years.".to_string(),
                    2 => "Madoc sleeps, and the Hall of Lamps burns for him. Two halls, and the smell of dwarf-smoke on the stair for the first time since me father's day.".to_string(),
                    _ => "The Gallery, the Hall of Lamps, the Anvil Gallery: all three mine, all three lit, and Olwen out from under the roof. Ye're no hirelings. Ye're kin of Caer Dwfn.".to_string(),
                };
                (words, vec!["E: GO ON".to_string()])
            }
            2 => match halls.offer(b) {
                Some((_, hall)) => (hall.lore.to_string(), vec![format!("1: I'LL TAKE {}", hall.name.to_uppercase()), "3: FAREWELL".into()]),
                None => ("There is no more to tell of it.".into(), vec!["E: FAREWELL".into()]),
            },
            _ => {
                if let Some((_, hall)) = halls.offer(b) {
                    (
                        hall.asks.to_string(),
                        vec![format!("1: I'LL TAKE {}", hall.name.to_uppercase()), "2: TELL ME OF THE HALL".into(), "3: FAREWELL".into()],
                    )
                } else if let Some((_, hall)) = halls.asked_hall() {
                    (
                        format!(
                            "Ye're after {}, off the floors of {}. {} holds it. Mind what I told ye, and come back and tell me it's done.",
                            hall.name,
                            hall.pack.name(),
                            hall.keeper
                        ),
                        vec!["E: FAREWELL".into()],
                    )
                } else if halls.count() == HALLS.len() {
                    (
                        "There's rent in me ledger from every hall ye've won, and ore in me stockpile. Ye're welcome at me fire any day. The halls I can name are mine again.".into(),
                        vec!["E: FAREWELL".into()],
                    )
                } else {
                    ("Come back when ye've something to tell me.".into(), vec!["E: FAREWELL".into()])
                }
            }
        };
        TalkView {
            who: Who::King,
            name: "KING BRANNOC ONEHORN".into(),
            tag: if held == 0 { "OF CAER DWFN".into() } else { format!("OF CAER DWFN: {held} OF {} HALLS HIS", HALLS.len()) },
            words,
            answers,
        }
    }

    fn keeper_view(&self, page: u8) -> Option<TalkView> {
        let play = self.hall_play()?;
        let (i, hall) = hall_of(self.dungeon.pack)?;
        let fought = self.home.barony.halls.how(hall.id) == Some("fight") || play.how == "fight";
        let (name, tag, words, answers) = match (i, play.stage, page) {
            (0, 0, 0) => (
                "CHIEF NUBBIN TALLOWTOOTH",
                "OF THE TALLOW-HANDS",
                "Ha! Knights! Come to sit in me chair? It's a good chair. Dwarf-built, and I found it, so it's mine.".to_string(),
                vec!["E: GO ON".to_string()],
            ),
            (0, 0, _) => (
                "CHIEF NUBBIN TALLOWTOOTH",
                "OF THE TALLOW-HANDS",
                format!("Me clan's been warm in this gallery a hundred years. Pay {TOLL} gold and the Tallow-Hands pay rent to your King and dig his ore at the face. Or try me and me boar."),
                vec![format!("1: PAY THE TOLL ({TOLL} GOLD)"), "2: FIGHT HIM AND HIS BOAR".into(), "3: NOT YET".into()],
            ),
            (0, 1, _) => return None,
            (0, _, _) if fought => (
                "GWYN THE DELVER",
                "STEWARD OF THE GALLERY",
                "Chief's gone and the chair's the King's. Delvers are coming home down the shaft. Rent goes up to him, floor by floor. Nobody's cursing the candles now.".to_string(),
                vec!["E: FAREWELL".to_string()],
            ),
            (0, _, _) => (
                "CHIEF NUBBIN TALLOWTOOTH",
                "TENANT OF THE GALLERY",
                "Rent's rent. Gold and ore for your King, every floor ye clear, and no goblin touches his chair. Me clan digs, I count. Nobody's ever been so honest.".to_string(),
                vec!["E: FAREWELL".to_string()],
            ),
            (1, 0, 0) => (
                "THANE MADOC GREYHELM",
                "LAMP-WARDEN OF CAER DWFN",
                "Who walks the Hall of Lamps? I kept them lit. I kept all four. A hundred years the oil has been out, and I have not been able to put my hand to the wick.".to_string(),
                vec!["E: GO ON".to_string()],
            ),
            (1, 0, _) => (
                "THANE MADOC GREYHELM",
                "LAMP-WARDEN OF CAER DWFN",
                "Four sons, four lamps. Kindle them eldest first: their winters are cut in the stones. Light one out of turn and they all gutter, and the dead come to ask why.".to_string(),
                vec!["E: I'LL KINDLE THEM".to_string()],
            ),
            (1, 1, _) => (
                "THANE MADOC GREYHELM",
                "LAMP-WARDEN OF CAER DWFN",
                format!("Eldest first. The winters are on the stones. {} of 4 burn.", play.lit.len()),
                vec!["E: FAREWELL".to_string()],
            ),
            (1, _, _) => (
                "THANE MADOC GREYHELM",
                "AT REST",
                "They burn. All four burn. Tell the King that Madoc kept his father's lamps, and that I may sleep now.".to_string(),
                vec!["E: FAREWELL".to_string()],
            ),
            (_, 0, 0) => (
                "OLWEN ANVILHAND",
                "SMITH TO THE KING",
                "Who's that on the rock? Brannoc's colours, is it? Is it the King's day already? I was at me last bar.".to_string(),
                vec!["E: GO ON".to_string()],
            ),
            (_, 0, _) => (
                "OLWEN ANVILHAND",
                "SMITH TO THE KING",
                "Three heaps of roof on me. Dig them out, one at a time: hold E on the rubble. The dust wakes the Keep's imps, mind.".to_string(),
                vec!["E: I'LL DIG YOU OUT".to_string()],
            ),
            (_, 1, _) => (
                "OLWEN ANVILHAND",
                "SMITH TO THE KING",
                format!("{} of 3 heaps off me. Keep digging. Mind the imps.", (play.dug).count_ones()),
                vec!["E: FAREWELL".to_string()],
            ),
            (_, _, _) => (
                "OLWEN ANVILHAND",
                "SMITH TO THE KING",
                "Olwen Anvilhand, smith to the King, thanks ye. Pell and I will go home to Caer Dwfn and light the first fire we can find.".to_string(),
                vec!["E: FAREWELL".to_string()],
            ),
        };
        Some(TalkView {
            who: Who::Keeper,
            name: name.into(),
            tag: tag.into(),
            words,
            answers,
        })
    }

    /// Knight `id` answers with 1, 2 or 3.
    pub(in crate::rules) fn reclaim_answer(&mut self, id: u32, pick: u8) {
        let Some(talk) = self.reclaim.talks.get(&id).cloned() else {
            return;
        };
        if talk.page == 0 {
            return;
        }
        match talk.who {
            Who::King => self.answer_king(id, pick, talk.page),
            Who::Keeper => self.answer_hall_keeper(id, pick),
        }
    }

    fn answer_king(&mut self, id: u32, pick: u8, page: u8) {
        let offer = self.home.barony.halls.offer(&self.home.barony);
        match (pick, offer) {
            (1, Some((i, hall))) => {
                self.reclaim.talks.remove(&id);
                *self.marks.entry(ask_mark(i)).or_default() += 1;
                self.sounds.push("play_card");
                self.found = Some((
                    self.tick,
                    id,
                    format!("BRANNOC: TAKE {}. IT HANGS OFF THE FLOORS OF {}.", hall.name.to_uppercase(), hall.pack.name().to_uppercase()),
                ));
            }
            (2, Some(_)) if page == 1 => {
                if let Some(t) = self.reclaim.talks.get_mut(&id) {
                    t.page = 2;
                }
            }
            (3, _) => {
                self.reclaim.talks.remove(&id);
            }
            _ => {}
        }
    }

    fn answer_hall_keeper(&mut self, id: u32, pick: u8) {
        let Some((i, hall)) = hall_of(self.dungeon.pack) else {
            return;
        };
        let stage = self.hall_play().map_or(0, |p| p.stage);
        if i != 0 || stage != 0 {
            return;
        }
        match pick {
            1 => {
                let toll = crate::realm::Spoils(BTreeMap::from([(crate::realm::Spoil::Gold, TOLL)]));
                if !self.treasury.covers(&toll) {
                    self.found = Some((self.tick, id, format!("{}: {}", hall.keeper.to_uppercase(), self.treasury.shortfall(&toll).to_uppercase())));
                    return;
                }
                self.reclaim.talks.remove(&id);
                let tick = self.tick;
                if let Some(p) = self.reclaim.hall.as_mut() {
                    p.stage = 2;
                    p.how = "bargain".into();
                    p.sent = tick;
                }
                *self.marks.entry(toll_mark(i)).or_default() += 1;
                self.sounds.push("play_card");
                self.found = Some((self.tick, id, format!("{TOLL} GOLD TO THE TALLOW-HANDS. THEY PAY RENT TO THE KING NOW.")));
            }
            2 => {
                self.reclaim.talks.remove(&id);
                self.begin_set_piece();
            }
            3 => {
                self.reclaim.talks.remove(&id);
            }
            _ => {}
        }
    }

    /// The keeper's ask taken: the hall's set-piece begins.
    fn begin_set_piece(&mut self) {
        let Some((i, _)) = hall_of(self.dungeon.pack) else {
            return;
        };
        if let Some(p) = self.reclaim.hall.as_mut() {
            if p.stage != 0 {
                return;
            }
            p.stage = 1;
        }
        self.sounds.push("door_open");
        if i == 0 {
            self.raise_chief();
        }
    }

    /// Nubbin takes his boar, and his kin their bombs.
    fn raise_chief(&mut self) {
        let Some(p) = self.reclaim.hall.as_mut() else {
            return;
        };
        p.how = "fight".into();
        let remaining = 3u8.saturating_sub(p.slain);
        let chief = !p.chief_down;
        let spots = [(7.0, 6.0), (17.0, 6.0), (12.0, 8.5)];
        let mut raised = Vec::new();
        for (n, &(x, y)) in spots.iter().enumerate().take(usize::from(remaining)) {
            let kind = if n == 2 { EnemyKind::Sapper } else { EnemyKind::Hob };
            raised.push((kind, x, y, false));
        }
        if chief {
            raised.push((EnemyKind::Warboar, 12.0, 4.4, true));
        }
        let mut ids = Vec::new();
        for (kind, x, y, boss) in raised {
            if let Some(id) = self.spawn_body(kind, x * TILE_UNITS, y * TILE_UNITS) {
                if boss && let Some(e) = self.enemies.iter_mut().find(|e| e.id == id) {
                    e.raise_elite();
                }
                ids.push((id, boss));
            }
        }
        if let Some(p) = self.reclaim.hall.as_mut() {
            p.foes = ids;
        }
        self.cues.push("elite".into());
        self.shake = self.shake.max(6);
    }

    /// The hall the party stands in: its set-piece, a tick.
    fn tick_hall(&mut self, inputs: &BTreeMap<u32, Input>, pressed: u8) {
        let in_hall = !self.at_home_now() && self.room().kind == RoomKind::DwarfHall;
        if !in_hall {
            if let Some(p) = self.reclaim.hall.as_mut() {
                p.inside = false;
                p.foes.clear();
            }
            return;
        }
        let Some((i, hall)) = hall_of(self.dungeon.pack) else {
            return;
        };
        let held = self.home.barony.halls.is_held(hall.id);
        let fresh = self.reclaim.hall.as_ref().is_none_or(|p| p.room != self.at);
        if fresh {
            self.reclaim.hall = Some(HallPlay {
                room: self.at,
                hall: i as u8,
                stage: if held { 2 } else { 0 },
                ..Default::default()
            });
        }
        let tick = self.tick;
        let Some(mut p) = self.reclaim.hall.take() else {
            return;
        };
        if held {
            p.stage = 2;
        }
        // Back in after leaving mid-fight: the clan is where it was left.
        if !p.inside {
            p.inside = true;
            self.reclaim.hall = Some(p.clone());
            if i == 0 && p.stage == 1 && !held {
                self.raise_chief();
            }
            p = self.reclaim.hall.take().unwrap_or(p);
        }
        match i {
            0 => self.tick_fight(&mut p),
            1 => self.tick_rite(&mut p, pressed),
            _ => self.tick_dig(&mut p, inputs),
        }
        if p.stage == 2 && !held && !p.how.is_empty() && p.how != "bargain" && tick >= p.sent + RESEND {
            p.sent = tick;
            *self.marks.entry(won_mark(i, &p.how)).or_default() += 1;
        }
        if p.how == "bargain" && p.stage == 2 && !held && tick >= p.sent + RESEND {
            // The realm could not take the toll: ask again at the chair.
            p.stage = 0;
            p.how.clear();
        }
        self.reclaim.hall = Some(p);
    }

    /// The chief's fight: who is down, and the hall won when all are.
    fn tick_fight(&mut self, p: &mut HallPlay) {
        if p.stage != 1 {
            return;
        }
        let mut gone = Vec::new();
        for &(id, chief) in &p.foes {
            if !self.enemies.iter().any(|e| e.id == id && e.hp > 0) {
                gone.push((id, chief));
            }
        }
        for (id, chief) in gone {
            p.foes.retain(|f| f.0 != id);
            if chief {
                p.chief_down = true;
            } else {
                p.slain += 1;
            }
        }
        if p.foes.is_empty() && p.chief_down && p.slain >= 3 {
            self.hall_won(p, "fight", "THE TALLOW-HANDS ARE ROUTED. THE GALLERY IS THE KING'S.");
        }
    }

    /// The rite: a knight beside a lamp, pressing E, kindles it.
    fn tick_rite(&mut self, p: &mut HallPlay, pressed: u8) {
        if p.stage != 1 {
            return;
        }
        let order = rite_order(self.seed);
        let ids: Vec<u32> = self.players.keys().copied().collect();
        for id in ids {
            if !has(pressed, id) || self.reclaim.talks.contains_key(&id) {
                continue;
            }
            let Some(lamp) = (0..LAMPS.len()).find(|&l| self.reclaim_near(id, LAMPS[l], LAMP_REACH)) else {
                continue;
            };
            if p.lit.contains(&(lamp as u8)) {
                continue;
            }
            if usize::from(order[p.lit.len()]) == lamp {
                p.lit.push(lamp as u8);
                self.sounds.push("wheel_land");
                self.fx.push(Fx::Shake(3));
            } else {
                p.lit.clear();
                self.sounds.push("rock_land");
                self.fx.push(Fx::Shake(8));
                self.found = Some((self.tick, id, "THE LAMPS GUTTER. THE DEAD STIR. ELDEST SON FIRST.".into()));
                for k in 0..2 {
                    if p.raised < RAISE_CAP {
                        let x = if k == 0 { 6.0 } else { 18.0 };
                        if self.spawn_body(EnemyKind::Wraith, x * TILE_UNITS, 6.0 * TILE_UNITS).is_some() {
                            p.raised += 1;
                        }
                    }
                }
            }
        }
        if p.lit.len() == LAMPS.len() {
            self.hall_won(p, "rite", "THE FOURTH LAMP BURNS. MADOC GREYHELM MAY SLEEP.");
        }
    }

    /// The dig: a knight standing on a heap, holding E, digs it out.
    fn tick_dig(&mut self, p: &mut HallPlay, inputs: &BTreeMap<u32, Input>) {
        if p.stage != 1 {
            return;
        }
        let mut dug = Vec::new();
        for (&id, hero) in self.players.iter_mut() {
            let pressing = inputs.get(&id).copied().filter(|i| i.valid()).is_some_and(|i| i.interact);
            if !pressing {
                hero.buy_spent = false;
            }
            let on = hero.hp > 0 && !hero.stone;
            let site = DIGS.iter().position(|d| on && inside(*d, hero.x, hero.y));
            match site {
                Some(s) if pressing && !hero.buy_spent && p.dug & 1 << s == 0 => {
                    hero.buying = hero.buying.saturating_add(1);
                    if hero.buying >= DIG_HOLD {
                        hero.buying = 0;
                        hero.buy_spent = true;
                        dug.push(s);
                    }
                }
                Some(_) => hero.buying = 0,
                None => {}
            }
        }
        for s in dug {
            if p.dug & 1 << s != 0 {
                continue;
            }
            p.dug |= 1 << s;
            self.sounds.push("rock_land");
            self.fx.push(Fx::Shake(8));
            let (c, r, w, h) = DIGS[s];
            let (cx, cy) = (c as f32 + w as f32 / 2.0, r as f32 + h as f32 / 2.0);
            for k in 0..2 {
                if p.raised < RAISE_CAP {
                    let dx = if k == 0 { -1.2 } else { 1.2 };
                    if self.spawn_body(EnemyKind::Imp, (cx + dx) * TILE_UNITS, (cy + 1.4) * TILE_UNITS).is_some() {
                        p.raised += 1;
                    }
                }
            }
        }
        if p.dug.count_ones() as usize == DIGS.len() {
            self.hall_won(p, "freed", "OLWEN ANVILHAND IS FREE. THE KING'S SMITH STANDS.");
        }
    }

    /// A set-piece played out: the mark goes to the realm.
    fn hall_won(&mut self, p: &mut HallPlay, how: &str, line: &str) {
        p.stage = 2;
        p.how = how.into();
        p.sent = self.tick;
        *self.marks.entry(won_mark(usize::from(p.hall), how)).or_default() += 1;
        self.sounds.push("wish_grant");
        self.fx.push(Fx::Shake(10));
        self.found = Some((self.tick, 0, line.into()));
    }

    /// The realm's word on the halls changed: what stands now shows.
    /// Called from `barony_news` with the halls before.
    pub(in crate::rules) fn reclaim_news(&mut self, before: &Halls) {
        let now = &self.home.barony.halls;
        for w in &now.won {
            if !before.is_held(&w.id) {
                self.fx.push(Fx::Shake(14));
                self.sounds.push("wheel_land");
                if let Some(h) = hall(&w.id) {
                    self.found = Some((self.tick, 0, format!("{} IS THE KING'S. THE LAMP IS LIT IN CAER DWFN.", h.name.to_uppercase())));
                }
            }
        }
        if now.asked.is_some() && before.asked != now.asked {
            self.sounds.push("door_open");
        }
    }
}
