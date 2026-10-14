//! Joining a friend's delve from your own angelX: `/dungeon join <link>`.
//!
//! The friend's server runs the game; this angelX is one of its clients, as
//! a browser is (`together_seat`): its seat's client sends this knight's
//! keys, predicts its knight, and draws its own screen. The link is the
//! host's share link: `https://castle.ts.net/#KEY` through Funnel or a
//! relay, or `http://192.168.1.5:8767/#KEY` on the host's own network.
//! A lost link is dialed again as a browser's is, taking the same seat back.
//!
//! The words around the room (the party's health, cards, the ballot, the
//! wish keys) are read from the same run the seat's client holds, built
//! here into the shape the joined screen reads (`Hud`).

use super::together_seat::Seat;
use super::together_shooter::{Book, DEEPEST, Phase, Run};
use angel_serve::client::LinkStatus;
use serde::Serialize;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The host's run standing still for this long reads as paused.
const PAUSED_AFTER: Duration = Duration::from_millis(600);

/// What arrived since the last look: notices for this knight, sounds, the
/// chorus's lines.
#[derive(Default)]
pub(crate) struct News {
    pub(crate) replies: Vec<String>,
    pub(crate) sounds: Vec<String>,
    pub(crate) voices: Vec<serde_json::Value>,
}

pub(crate) struct Joined {
    /// Where the host is, for the cockpit to name.
    pub(crate) host: String,
    pub(crate) seat: Seat,
    /// The newest run this seat's client holds, read once a frame.
    run: Option<Arc<Run>>,
    state: Option<serde_json::Value>,
    /// The run's tick, and when it last moved.
    moved: Option<(u64, Instant)>,
}

/// One knight as the joined screen reads it.
#[derive(Clone, Debug, PartialEq, Serialize)]
struct Knight {
    id: u32,
    name: String,
    hp: u32,
    max_hp: u32,
    vigil: bool,
    hand_ids: Vec<String>,
    spell_ids: Vec<Option<String>>,
    deck_ids: Vec<String>,
    arm_id: Option<String>,
    ballot_open: bool,
    ballot_serial: u64,
    can_leave_activity: bool,
    can_reforge: bool,
    can_wish: bool,
    /// The HUD, as the host's own screen draws it.
    hud: Option<super::together_shooter::hud::Hud>,
}

/// The party and the room's words, for the joined screen.
#[derive(Clone, Debug, PartialEq, Serialize)]
struct Hud {
    actor: u32,
    paused: bool,
    phase: Phase,
    floor: u32,
    floors: u32,
    pack: &'static str,
    score: u32,
    viewers: u32,
    players: Vec<Knight>,
    sanctuary: bool,
    side_on: bool,
    notice: String,
    boss_gates: Option<super::together_shooter::BossGateState>,
}

impl Hud {
    fn of(run: &Run, me: u32, paused: bool) -> Hud {
        Hud {
            actor: me,
            paused,
            phase: if run.exits.outside.contains_key(&me) {
                Phase::Exploring
            } else {
                run.phase
            },
            floor: run.floor(),
            floors: DEEPEST,
            pack: run.dungeon.pack.name(),
            score: run.score,
            viewers: run.audience,
            players: run
                .players
                .iter()
                .map(|(&id, hero)| Knight {
                    id,
                    // The host's knight is "You" only on the host's screen.
                    name: if id == 1 && hero.name == "You" {
                        "Host".into()
                    } else {
                        hero.name.clone()
                    },
                    hp: hero.hp,
                    max_hp: hero.max_hp,
                    vigil: hero.vigil,
                    hand_ids: hero.hand.clone(),
                    spell_ids: hero.spells.to_vec(),
                    deck_ids: hero.deck.clone(),
                    arm_id: hero.arm.clone(),
                    ballot_open: run.playtest.open(id),
                    ballot_serial: run.playtest.seats.get(&id).map_or(0, |s| s.serial),
                    can_leave_activity: run.can_leave_activity(id),
                    can_reforge: run.can_reforge(id),
                    can_wish: run.can_wish(id),
                    hud: run
                        .view_for_seat(id)
                        .as_ref()
                        .unwrap_or(run)
                        .hud(id, &super::together_shooter::hud::Keys::FIRST),
                })
                .collect(),
            sanctuary: run.room().kind == super::together_shooter::RoomKind::Sanctuary,
            side_on: run.side_on(),
            notice: run
                .found_line()
                .unwrap_or_default()
                .chars()
                .filter(|c| !c.is_control())
                .take(160)
                .collect(),
            boss_gates: run.boss_gate_state(),
        }
    }
}

impl Joined {
    /// Open the host's share link and take a seat as `name`. The dial goes
    /// on in the background; the terminal never waits on the network.
    pub(crate) fn connect(link: &str, name: &str) -> Result<Joined, String> {
        let link = link.trim().trim_start_matches("/dungeon join").trim();
        let (url, _) = angel_serve::client::share_link(link)?;
        let host = url
            .split_once("://")
            .map_or(url.as_str(), |(_, rest)| rest)
            .trim_end_matches("/ws")
            .to_string();
        let seat = Seat::connect(link, name)?;
        Ok(Joined {
            host,
            seat,
            run: None,
            state: None,
            moved: None,
        })
    }

    /// Take the seat's newest run, once a frame.
    pub(crate) fn refresh(&mut self, now: Instant) {
        let Some(run) = self.seat.run() else {
            return;
        };
        let tick = run.tick;
        if self.moved.is_none_or(|(was, _)| was != tick) {
            self.moved = Some((tick, now));
        }
        let paused = self
            .moved
            .is_some_and(|(_, at)| now.saturating_duration_since(at) >= PAUSED_AFTER);
        let me = self.actor();
        self.state = serde_json::to_value(Hud::of(&run, me, paused)).ok();
        self.run = Some(Arc::new(run));
    }

    /// The host's delve as this seat holds it, newest tick.
    pub(crate) fn with_latest<T>(&self, f: impl FnOnce(&Run) -> T) -> Option<T> {
        self.run.as_deref().map(f)
    }

    /// The party and the room's words.
    pub(crate) fn state(&self) -> Option<serde_json::Value> {
        self.state.clone()
    }

    /// The host's book of cards.
    pub(crate) fn book(&self) -> Option<Arc<Book>> {
        self.run.as_ref().map(|run| Arc::new(run.book.clone()))
    }

    /// This seat's knight.
    pub(crate) fn actor(&self) -> u32 {
        self.seat.knight().unwrap_or(2)
    }

    /// A move on the playtest's ballot: the key the Delve's client takes
    /// for it, which sends the move in order until the host's run has it.
    pub(crate) fn vote(&self, command: super::together_shooter::votes::Command) {
        self.seat
            .tap(super::together_seat::ballot_bits(command.action));
    }

    /// Leave the activity this knight is in: the client asks until it has.
    /// A menu opened for it and not yet sent never goes.
    pub(crate) fn leave_activity(&self) {
        self.seat.untap(angel_net::input::button::MENU);
        self.seat.tap(angel_net::input::button::OK);
    }

    /// The activity menu, up or down: the client holds the knight while it is up.
    pub(crate) fn menu(&self) {
        self.seat.tap(angel_net::input::button::MENU);
    }

    /// Skip the floor's plate this knight is shown, if one is: any key does.
    pub(crate) fn skip_floor_plate(&self) -> bool {
        let me = self.actor();
        if self.with_latest(|run| run.floor_plate(Some(me)).is_some()) == Some(true) {
            self.seat.tap(angel_net::input::button::USE);
            return true;
        }
        false
    }

    /// Something for the host (`together_gear`): a wish, a reforged card.
    pub(crate) fn send(&self, kind: u32, body: &[u8]) {
        self.seat.action(kind, body);
    }

    /// Why the host can't be reached, while it can't.
    pub(crate) fn error(&self) -> Option<String> {
        match self.seat.status() {
            LinkStatus::Connecting | LinkStatus::Playing => None,
            LinkStatus::Reconnecting => Some("the host can't be reached; trying again".into()),
            LinkStatus::Rejected(_, why) => Some(format!("the host turned this seat away: {why}")),
            LinkStatus::Replaced => Some("another window took this seat".into()),
            LinkStatus::Closed => Some("left the host's delve".into()),
        }
    }

    /// What the seat heard since the last look: its sounds, the chorus's
    /// lines, and notices for this knight.
    pub(crate) fn news(&self) -> News {
        let mut news = News::default();
        for line in self.seat.take_lines() {
            if let Some(sound) = line.strip_prefix("sfx ") {
                news.sounds.push(sound.to_string());
            } else if let Some(said) = line.strip_prefix("say ") {
                let (name, words) = said.split_once(": ").unwrap_or(("", said));
                news.voices
                    .push(serde_json::json!({ "name": name, "words": words }));
            } else if let Some(text) = line.strip_prefix("notice ") {
                news.replies.push(text.to_string());
            }
        }
        news
    }

    /// The track this seat's own view wants.
    pub(crate) fn music(&self) -> Option<&'static str> {
        let me = self.actor();
        self.with_latest(|run| run.view_for_seat(me).as_ref().unwrap_or(run).music())
            .flatten()
    }
}

impl Drop for Joined {
    fn drop(&mut self) {
        self.seat.bye();
    }
}
