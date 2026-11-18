//! A seat's view of the race: whose camera it is, how the screen is laid, and what each knight is
//! called from here. `YOU` names the viewer's own team and nobody else's.
use super::super::TILE;
use super::art;
use crate::rules::chariot::{self as cr, Chariot, Phase, pos_on, show::Kind};
use crate::rules::{HZ, Run};

pub(super) const FOLD: f32 = 24.0;

pub(super) struct View<'a> {
    pub run: &'a Run,
    pub race: &'a Chariot,
    pub seat: Option<u32>,
    pub me: usize,
    pub mine: bool,
    pub now: u32,
    pub size: (i32, i32),
}

impl<'a> View<'a> {
    pub fn new(run: &'a Run, focus: Option<u32>, size: (i32, i32)) -> Option<View<'a>> {
        let race = run.chariot.as_ref()?;
        let focus = focus.or(Some(race.driver).filter(|&d| d != 0));
        let own = focus.and_then(|s| race.racer_of(s)).filter(|&i| i < race.racers.len());
        let me = own.unwrap_or_else(|| race.order().first().copied().unwrap_or(0));
        Some(View { run, race, seat: focus, me, mine: own.is_some(), now: race.clock, size })
    }

    pub fn course(&self) -> &'static cr::Track {
        self.race.course()
    }

    pub fn fan(&self) -> f32 {
        match self.race.phase {
            Phase::Gate => 1.0,
            Phase::Racing => (1.0 - self.race.t as f32 / FOLD).max(0.0),
            Phase::Done => 0.0,
        }
    }

    pub fn lift(&self) -> f32 {
        match self.race.phase {
            Phase::Gate => 1.0,
            Phase::Racing => (1.0 - self.race.t as f32 / 40.0).max(0.0),
            Phase::Done => 0.0,
        }
    }

    pub fn drawn(&self, i: usize) -> f32 {
        let r = &self.race.racers[i];
        let back = f32::from(r.team / 2) * 40.0 + f32::from(r.team % 2) * 14.0;
        r.dist + r.coast - self.fan() * back
    }

    pub fn map_px(&self, i: usize) -> (f32, f32) {
        let r = &self.race.racers[i];
        let (x, y) = pos_on(self.course(), self.drawn(i), r.lane);
        (x * TILE as f32, y * TILE as f32)
    }

    pub fn camera(&self) -> (i32, i32) {
        camera_for(self.race, self.me, self.size.0, self.size.1, self.lift(), self.drawn(self.me))
    }

    pub fn index_of(&self, team: u8) -> Option<usize> {
        self.race.racers.iter().position(|r| r.team == team)
    }

    pub fn seat_of(&self, team: u8) -> Option<u32> {
        let i = self.index_of(team)?;
        self.race.seats().into_iter().find(|&(_, r)| r == i).map(|(s, _)| s)
    }

    pub fn called(&self, team: u8, take: usize) -> String {
        let Some(i) = self.index_of(team) else { return String::new() };
        if self.mine && i == self.me {
            return "YOU".into();
        }
        if let Some(seat) = self.seat_of(team) {
            let full = self.run.players.get(&seat).map_or_else(String::new, |h| h.name.trim().to_uppercase());
            if full.is_empty() || full == "YOU" {
                return format!("KNIGHT {seat}");
            }
            return full.chars().take(take).collect();
        }
        let r = &self.race.racers[i];
        let house = r.house.trim().to_uppercase();
        let name = if i == 0 && r.name != "Yard Nag" { r.name.trim().to_uppercase() } else { house };
        let name = if name == "YOU" { "KEEP".to_string() } else { name };
        name.chars().take(take).collect()
    }

    pub fn ink(&self, i: usize) -> char {
        art::banner_ink(&self.race.racers[i].house)
    }

    pub fn my_team(&self) -> u8 {
        self.race.racers[self.me].team
    }

    pub fn is_me(&self, team: u8) -> bool {
        self.mine && team == self.my_team()
    }

    pub fn place(&self, i: usize) -> usize {
        1 + self.race.order().iter().position(|&r| r == i).unwrap_or(0)
    }

    pub fn since_mine(&self, kind: Kind) -> Option<u32> {
        self.race.show.since(kind, self.my_team(), self.now)
    }

    pub fn seconds(ticks: u32) -> String {
        format!("{}.{}", ticks / HZ, ticks % HZ * 10 / HZ)
    }
}

pub(super) fn camera_for(race: &Chariot, me: usize, w: i32, h: i32, fan: f32, drawn: f32) -> (i32, i32) {
    let course = race.course();
    let r = &race.racers[me.min(race.racers.len() - 1)];
    let (tx, ty) = pos_on(course, drawn, r.lane);
    let (px, py) = (tx * TILE as f32, ty * TILE as f32);
    let (dx, dy) = cr::tangent_on(course, drawn);
    let look = (r.speed.max(0.0) * 2.4).min(30.0);
    let drop = 84.0 * fan * fan;
    let (cx, cy) = (px + dx * look - w as f32 / 2.0, py + dy * look - h as f32 / 2.0 - drop);
    (cx as i32, cy as i32)
}
