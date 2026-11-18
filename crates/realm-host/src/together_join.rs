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

const PAUSED_AFTER: Duration = Duration::from_millis(600);

#[derive(Default)]
pub struct News {
    pub replies: Vec<String>,
    pub sounds: Vec<String>,
    pub voices: Vec<serde_json::Value>,
    pub ask: bool,
    pub wish_row: Option<u8>,
}

pub struct Joined {
    pub host: String,
    pub seat: Seat,
    run: Option<Arc<Run>>,
    state: Option<serde_json::Value>,
    moved: Option<(u64, Instant)>,
}

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
    can_hold_leave: bool,
    can_reforge: bool,
    can_wish: bool,
    wish_offer: bool,
    hud: Option<super::together_shooter::hud::Hud>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
struct Hud {
    actor: u32,
    paused: bool,
    phase: Phase,
    floor: u32,
    floors: u32,
    pack: &'static str,
    score: u32,
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
            players: run
                .players
                .iter()
                .map(|(&id, hero)| Knight {
                    id,
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
                    can_hold_leave: run.can_hold_leave(id),
                    can_reforge: run.can_reforge(id),
                    can_wish: run.wish_line_opens(id),
                    wish_offer: hero.offer.is_some(),
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
    pub fn connect(link: &str, name: &str) -> Result<Joined, String> {
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

    pub fn refresh(&mut self, now: Instant) {
        let Some(run) = self.seat.run_arc() else {
            return;
        };
        let tick = run.tick;
        let tick_changed = self.moved.is_none_or(|(was, _)| was != tick);
        if tick_changed {
            self.moved = Some((tick, now));
        }
        let paused = self
            .moved
            .is_some_and(|(_, at)| now.saturating_duration_since(at) >= PAUSED_AFTER);
        let me = self.actor();
        let reuse = !tick_changed
            && self
                .run
                .as_ref()
                .is_some_and(|prev| Arc::ptr_eq(prev, &run))
            && self.state.as_ref().is_some_and(|s| {
                s["actor"].as_u64() == Some(u64::from(me))
                    && s["paused"].as_bool() == Some(paused)
            });
        if !reuse {
            self.state = serde_json::to_value(Hud::of(&run, me, paused)).ok();
            self.run = Some(run);
        }
    }

    pub fn with_latest<T>(&self, f: impl FnOnce(&Run) -> T) -> Option<T> {
        self.run.as_deref().map(f)
    }

    pub fn state(&self) -> Option<serde_json::Value> {
        self.state.clone()
    }

    pub fn run(&self) -> Option<Arc<Run>> {
        self.run.clone()
    }

    pub fn book(&self) -> Option<Arc<Book>> {
        self.run.as_ref().map(|run| Arc::new(run.book.clone()))
    }

    pub fn actor(&self) -> u32 {
        self.seat.knight().unwrap_or(2)
    }

    pub fn vote(&self, command: super::together_shooter::votes::Command) {
        self.seat
            .tap(super::together_seat::ballot_bits(command.action));
    }

    pub fn leave_activity(&self) {
        self.seat.untap(angel_net::input::button::MENU);
        self.seat.tap(angel_net::input::button::OK);
    }

    pub fn menu(&self) {
        self.seat.tap(angel_net::input::button::MENU);
    }

    pub fn skip_floor_plate(&self) -> bool {
        let me = self.actor();
        if self.with_latest(|run| run.floor_plate(Some(me)).is_some()) == Some(true) {
            self.seat.tap(angel_net::input::button::USE);
            return true;
        }
        false
    }

    pub fn send(&self, kind: u32, body: &[u8]) {
        self.seat.action(kind, body);
    }

    pub fn error(&self) -> Option<String> {
        match self.seat.status() {
            LinkStatus::Connecting | LinkStatus::Playing => None,
            LinkStatus::Reconnecting => Some("the host can't be reached; trying again".into()),
            LinkStatus::Rejected(_, why) => Some(format!("the host turned this seat away: {why}")),
            LinkStatus::Replaced => Some("another window took this seat".into()),
            LinkStatus::Closed => Some("left the host's delve".into()),
        }
    }

    pub fn news(&self) -> News {
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
            } else if line == "ask" {
                news.ask = true;
            } else if let Some(row) = line.strip_prefix("wish-row ") {
                news.wish_row = row.trim().parse().ok();
            }
        }
        news
    }

    pub fn music(&self) -> Option<&'static str> {
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
