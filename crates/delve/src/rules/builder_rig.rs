//! The builder's rig: what a knight with the builder out carries beyond the tool in
//! hand. A hotbar of nine tools (three banks of three, the number keys 1 to 3 choosing
//! within the bank, the arrows up and down changing it), the steps they have laid and
//! may take back, the first corner of a copy, a mouse drag in progress, whether the card
//! of what the cursor points at is up, where the view has been sent to look, and the
//! numbers made for this seat alone (`kingdom::Numbers`: the card, the production
//! screen, the list of what chokes the works).
//!
//! Each seat has its own; the economy and the plans are the party's. Nothing here is
//! the economy's: it is how a seat lays things, and what it is told.
use crate::kingdom::{Kingdom, Laid, Numbers, Tile, catalog};
use serde::{Deserialize, Serialize};

/// Slots of the hotbar, three banks of three.
pub const SLOTS: usize = 9;
pub const BANK: usize = 3;
/// The steps a seat can take back, newest last.
pub const UNDO_DEPTH: usize = 12;

/// What one press of E laid: the things, for taking them back as one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    pub what: Vec<Laid>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rig {
    /// The tool (an index into `hillside::tools()`) each slot holds. The slot in hand
    /// holds whatever is in hand: changing tool changes the slot.
    pub bar: [u16; SLOTS],
    pub slot: u8,
    /// Laying plans for the camp's hands to raise, rather than building at once.
    pub planning: bool,
    pub undo: Vec<Step>,
    /// The first corner of a copy.
    pub mark: Option<Tile>,
    /// A mouse drag in progress: where it began and where it is.
    pub drag: Option<(Tile, Tile)>,
    /// The card of what the cursor points at is up.
    pub card: bool,
    /// The view has been sent to look here (walking brings it back), in tiles.
    pub look: Option<(f32, f32)>,
    /// What this seat is shown, made for it alone: never sent to another.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub numbers: Option<Numbers>,
    /// When, where and for which page the numbers were made.
    pub made: (u64, Tile, u8),
    /// E has been held since a step was laid: the next tiles laid join it.
    pub held_step: bool,
}

impl Default for Rig {
    fn default() -> Self {
        Rig { bar: default_bar(), slot: 0, planning: false, undo: Vec::new(), mark: None, drag: None, card: false, look: None, numbers: None, made: (u64::MAX, (0, 0), 0), held_step: false }
    }
}

impl Rig {
    /// Whether this is a rig nobody has touched, which a save need not carry.
    pub fn is_idle(&self) -> bool {
        *self == Rig::default()
    }
    pub fn valid(&self) -> bool {
        let n = super::hillside::tools().len();
        self.bar.iter().all(|t| usize::from(*t) < n)
            && usize::from(self.slot) < SLOTS
            && self.undo.len() <= UNDO_DEPTH
            && self.undo.iter().all(|s| s.what.len() <= 400)
    }
    /// The bank the slot in hand is in.
    pub fn bank(&self) -> usize {
        usize::from(self.slot) / BANK
    }
    /// Put the tool in hand into the slot in hand.
    pub fn keep(&mut self, plan: usize) {
        self.bar[usize::from(self.slot) % SLOTS] = plan.min(usize::from(u16::MAX)) as u16;
    }
    /// Take slot `n` (0 to 8) in hand: its tool.
    pub fn take(&mut self, n: usize) -> usize {
        self.slot = (n % SLOTS) as u8;
        usize::from(self.bar[usize::from(self.slot)])
    }
    /// Another bank: the same place in it.
    pub fn bank_by(&mut self, delta: i32) -> usize {
        let bank = (self.bank() as i32 + delta).rem_euclid((SLOTS / BANK) as i32) as usize;
        self.take(bank * BANK + usize::from(self.slot) % BANK)
    }
    /// A step laid, to be taken back.
    pub fn laid(&mut self, what: Vec<Laid>) {
        if what.is_empty() {
            return;
        }
        self.undo.push(Step { what });
        if self.undo.len() > UNDO_DEPTH {
            self.undo.remove(0);
        }
    }
}

/// The hotbar a new builder has: the road, then the works a camp starts with, in the
/// order a first quarter of an hour wants them.
pub fn default_bar() -> [u16; SLOTS] {
    let tools = super::hillside::tools();
    let want = ["road", "cottage", "well", "sawpit", "claim-quarry", "banker", "carters-yard", "lodge", "wall"];
    let mut bar = [0u16; SLOTS];
    for (slot, id) in want.iter().enumerate() {
        bar[slot] = tools
            .iter()
            .position(|t| match t {
                super::hillside::Tool::Road => *id == "road",
                super::hillside::Tool::Plan(p) => p == id,
                _ => false,
            })
            .unwrap_or(slot.min(tools.len().saturating_sub(1))) as u16;
    }
    bar
}

/// What was there before a command, to find what it laid: the next ids, and the roads.
pub struct Snap {
    next: u32,
    next_line: u32,
    next_ghost: u32,
    roads: std::collections::BTreeSet<Tile>,
    rollers: std::collections::BTreeSet<Tile>,
    rails: std::collections::BTreeSet<Tile>,
}

impl Snap {
    pub fn of(k: &Kingdom) -> Self {
        Snap {
            next: k.next,
            next_line: k.next_line,
            next_ghost: k.plans.next,
            roads: k.roads.clone(),
            rollers: k.rollers.cells.keys().copied().collect(),
            rails: k.rails.cells.keys().copied().collect(),
        }
    }
    /// Everything laid since: buildings, plans, lines, roads (and bridges), rollerway and rail.
    pub fn laid(&self, k: &Kingdom) -> Vec<Laid> {
        let mut v = Vec::new();
        for t in k.roads.difference(&self.roads) {
            v.push(if k.bridges.contains(t) { Laid::Bridge(*t) } else { Laid::Road(*t) });
        }
        v.extend(k.buildings.keys().filter(|id| **id >= self.next).map(|id| Laid::Building(*id)));
        v.extend(k.lines.keys().filter(|id| **id >= self.next_line).map(|id| Laid::Line(*id)));
        v.extend(k.plans.ghosts.keys().filter(|id| **id > self.next_ghost).map(|id| Laid::Ghost(*id)));
        v.extend(k.rollers.cells.keys().filter(|t| !self.rollers.contains(*t)).map(|t| Laid::Roller(*t)));
        v.extend(k.rails.cells.keys().filter(|t| !self.rails.contains(*t)).map(|t| Laid::Rail(*t)));
        v
    }
}

/// Make this seat's numbers for what it has up, once a sim second (or when the cursor
/// or the page changed): the card of what the cursor points at, the production screen,
/// the list of chokes. Nothing when none is up.
pub(super) fn refresh(board: &mut super::industry::Board, k: &Kingdom) {
    use super::hillside::{PAGE_CHOKES, PAGE_PRODUCTION};
    let c = catalog();
    let page = if board.open { board.page } else { 0 };
    let want_card = board.open && page == 0 && board.rig.card;
    let want_production = board.open && page == PAGE_PRODUCTION;
    let want_chokes = board.open && page == PAGE_CHOKES;
    if !(want_card || want_production || want_chokes) {
        board.rig.numbers = None;
        board.rig.made = (u64::MAX, (0, 0), 0);
        return;
    }
    let key = (k.second, board.tile, page);
    if board.rig.made == key && board.rig.numbers.is_some() {
        return;
    }
    let mut n = Numbers { at: k.second, ..Numbers::default() };
    if want_card {
        n.card = k.card(c, board.tile);
    }
    if want_production {
        n.production = Some(k.production(c, 12));
    }
    if want_chokes {
        n.chokes = k.bottlenecks(c, 12);
    }
    board.rig.made = key;
    board.rig.numbers = Some(n);
}
