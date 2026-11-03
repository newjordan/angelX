//! The Delve's intro menu's state: choose a knight of the company, where to
//! begin, who comes along, then enter. Shown in the expanded view when the
//! knight reaches the Delve's gate on the map and the player clicks through.
//! (The cockpit draws it: `ui::viz::delve_intro_viz`.)

use crate::together_shooter::knights::{COMPANY, Knight};

/// Where the party begins.
pub const DELVES: [(&str, &str); 2] = [
    (
        "The Crypt",
        "slate halls under the chapel · bone and candle-wax · the Waxen Warden",
    ),
    (
        "The Mines",
        "timber and iron under the hill · ore and gems · Cinderjaw",
    ),
];

/// Who comes along.
pub const PARTIES: [(&str, &str); 3] = [
    ("Alone", "one knight, your keyboard"),
    (
        "A friend, here",
        "player 2 on this keyboard: IJKL, Enter fires, O rolls",
    ),
    (
        "Friends, invited",
        "an invitation line for a friend's angelX · /dungeon_host --3 for three",
    ),
];

/// One row of the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    /// Back into the run under way.
    Continue,
    Knight,
    Delve,
    Party,
    Voices,
    /// A new delve with the choices above.
    Begin,
}

/// The Delve's menu: continue the run under way, or choose for a new one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Intro {
    pub row: usize,
    pub knight: usize,
    pub delve: usize,
    pub party: usize,
    pub quiet: bool,
    /// The run under way, in a line, when there is one to continue.
    pub resume: Option<String>,
    /// Seats open to invited friends, while hosting.
    pub hosting: Option<usize>,
}

impl Intro {
    pub fn chosen(&self) -> &'static Knight {
        &COMPANY[self.knight % COMPANY.len()]
    }

    pub fn fields(&self) -> &'static [Field] {
        use Field::*;
        if self.resume.is_some() {
            &[Continue, Knight, Delve, Party, Voices, Begin]
        } else {
            &[Knight, Delve, Party, Voices, Begin]
        }
    }

    pub fn field(&self) -> Field {
        let fields = self.fields();
        fields[self.row % fields.len()]
    }

    pub fn step(&mut self, by: isize) {
        let n = self.fields().len() as isize;
        self.row = (self.row as isize + by).rem_euclid(n) as usize;
    }

    /// Put the cursor on `field`.
    pub fn go(&mut self, field: Field) {
        if let Some(row) = self.fields().iter().position(|&f| f == field) {
            self.row = row;
        }
    }

    /// ←/→ on the current row.
    pub fn turn(&mut self, step: isize) {
        let cycle = |v: usize, n: usize| (v as isize + step).rem_euclid(n as isize) as usize;
        match self.field() {
            Field::Knight => self.knight = cycle(self.knight, COMPANY.len()),
            Field::Delve => self.delve = cycle(self.delve, DELVES.len()),
            Field::Party => self.party = cycle(self.party, PARTIES.len()),
            Field::Voices => self.quiet = !self.quiet,
            Field::Continue | Field::Begin => {}
        }
    }
}
