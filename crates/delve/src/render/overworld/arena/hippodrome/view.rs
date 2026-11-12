//! A seat's view of the race: whose camera it is, how the screen is laid, and what each knight is
//! called from here. `YOU` names the viewer's own team and nobody else's.
use super::super::TILE;
use super::art;
use crate::rules::chariot::{self as cr, Chariot, Phase, pos_on, show::Kind};
use crate::rules::{HZ, Run};

/// How far the field folds in over the first strides, ticks.
pub(super) const FOLD: f32 = 24.0;

pub(super) struct View<'a> {
    pub run: &'a Run,
    pub race: &'a Chariot,
    /// The viewing seat, if it is one.
    pub seat: Option<u32>,
    /// The racer the camera rides: the viewer's own, or the leader's for a watcher.
    pub me: usize,
    /// Whether the viewer drives it.
    pub mine: bool,
    /// The race's clock: what every record's age is read from.
    pub now: u32,
    pub size: (i32, i32),
}

impl<'a> View<'a> {
    pub fn new(run: &'a Run, focus: Option<u32>, size: (i32, i32)) -> Option<View<'a>> {
        let race = run.chariot.as_ref()?;
        // A picture asked for no seat is the one who boarded's.
        let focus = focus.or(Some(race.driver).filter(|&d| d != 0));
        let own = focus.and_then(|s| race.racer_of(s)).filter(|&i| i < race.racers.len());
        let me = own.unwrap_or_else(|| race.order().first().copied().unwrap_or(0));
        Some(View { run, race, seat: focus, me, mine: own.is_some(), now: race.clock, size })
    }

    pub fn course(&self) -> &'static cr::Track {
        self.race.course()
    }

    /// How folded in the gate's grid is: 1 at the gate, gone by the first strides.
    pub fn fan(&self) -> f32 {
        match self.race.phase {
            Phase::Gate => 1.0,
            Phase::Racing => (1.0 - self.race.t as f32 / FOLD).max(0.0),
            Phase::Done => 0.0,
        }
    }

    /// How low the camera stands the team at the gate: 1 while the field is held, drawn up to the
    /// middle of the screen over the first strides.
    pub fn lift(&self) -> f32 {
        match self.race.phase {
            Phase::Gate => 1.0,
            Phase::Racing => (1.0 - self.race.t as f32 / 40.0).max(0.0),
            Phase::Done => 0.0,
        }
    }

    /// The distance the picture draws team `i` at: a stagger back along the grid at the gate,
    /// and past the line for a team that has finished (the victory lap).
    pub fn drawn(&self, i: usize) -> f32 {
        let r = &self.race.racers[i];
        // Two to a row, a stride apart, the second of each pair a little behind: by the team's own
        // number, so every seat's picture grids the same field the same way.
        let back = f32::from(r.team / 2) * 40.0 + f32::from(r.team % 2) * 14.0;
        r.dist + r.coast - self.fan() * back
    }

    /// Where team `i` stands on the map, in pixels: its wheels on the ground.
    pub fn map_px(&self, i: usize) -> (f32, f32) {
        let r = &self.race.racers[i];
        let (x, y) = pos_on(self.course(), self.drawn(i), r.lane);
        (x * TILE as f32, y * TILE as f32)
    }

    /// The camera's corner on the map: on the viewed team, leaning into its run, kept inside the
    /// course's country.
    pub fn camera(&self) -> (i32, i32) {
        camera_for(self.race, self.me, self.size.0, self.size.1, self.lift(), self.drawn(self.me))
    }

    /// The racer index of a team.
    pub fn index_of(&self, team: u8) -> Option<usize> {
        self.race.racers.iter().position(|r| r.team == team)
    }

    /// The seat that drives a team, if one does.
    pub fn seat_of(&self, team: u8) -> Option<u32> {
        let i = self.index_of(team)?;
        self.race.seats().into_iter().find(|&(_, r)| r == i).map(|(s, _)| s)
    }

    /// What this seat calls a team, at most `take` letters: `YOU` for its own team and no other,
    /// a knight by their own name (`KNIGHT n` for one who still carries the default `You`, or
    /// none), a yard by its house.
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

    /// The ink a team is marked in.
    pub fn ink(&self, i: usize) -> char {
        art::banner_ink(&self.race.racers[i].house)
    }

    /// The team the viewer is, by team number.
    pub fn my_team(&self) -> u8 {
        self.race.racers[self.me].team
    }

    /// Whether a record belongs to the viewer's team, as `team` or as `other`.
    pub fn is_me(&self, team: u8) -> bool {
        self.mine && team == self.my_team()
    }

    /// Place of racer `i`, 1 first.
    pub fn place(&self, i: usize) -> usize {
        1 + self.race.order().iter().position(|&r| r == i).unwrap_or(0)
    }

    /// Ticks since the newest record of `kind` that names the viewer's team.
    pub fn since_mine(&self, kind: Kind) -> Option<u32> {
        self.race.show.since(kind, self.my_team(), self.now)
    }

    /// The seconds a count of ticks is, to a tenth.
    pub fn seconds(ticks: u32) -> String {
        format!("{}.{}", ticks / HZ, ticks % HZ * 10 / HZ)
    }
}

/// The camera over racer `me`, `w` by `h`, clamped to the course's country.
pub(super) fn camera_for(race: &Chariot, me: usize, w: i32, h: i32, fan: f32, drawn: f32) -> (i32, i32) {
    let course = race.course();
    let r = &race.racers[me.min(race.racers.len() - 1)];
    let (tx, ty) = pos_on(course, drawn, r.lane);
    let (px, py) = (tx * TILE as f32, ty * TILE as f32);
    let (dx, dy) = cr::tangent_on(course, drawn);
    let look = (r.speed.max(0.0) * 2.4).min(30.0);
    // At the gate the camera stands the team low on the screen, under the lamps and the count, and
    // the first strides draw it up to the middle: a launch.
    let drop = 84.0 * fan * fan;
    let (cx, cy) = (px + dx * look - w as f32 / 2.0, py + dy * look - h as f32 / 2.0 - drop);
    // The camera may ride out past the country's edge: that is black paper.
    (cx as i32, cy as i32)
}
