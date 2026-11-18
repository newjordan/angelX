//! The sites: how far each castle on the map has risen (the Works,
//! `world::works`). A site is a castle's plan and how many of its stones
//! stand; everything visible and solid about it is a pure function of the
//! two, so the sim only ever advances `laid`.
//!
//! The wall is raised by hand and by the hired hands beside it (`hands`): a
//! knight sets a stone at the pegged course with a timing game and the mark
//! of who set it goes in the tally; the hired setters set the stones no one
//! has, slower. Until the production game feeds the wall, the stone chain is
//! the yard's stubs (`hands::Yard`). A test or a developer sets `laid` with
//! [`Run::lay_to`].
//!
//! The kingdom's room keeps the site's solid ground: a tile is a wall once
//! the stonework there reaches a knee, a pit once the ditch is dug, and
//! floor otherwise (`world::works::Castle::need`). The static map holds the
//! whole site closed; [`patch_room`] opens what stands open.

use super::*;
use super::kingdom::door_at;
use crate::world::works::{self, Castle};

pub const MAX_SITES: usize = 8;

pub const START: u32 = 1_500;

pub const NEAR_TILES: f32 = 30.0;

pub const CHRONICLE_HOLD: u32 = 45;

pub const CHRONICLE_TICKS: u64 = 30 * HZ as u64;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Site {
    pub plan: String,
    pub laid: u32,
    pub marks: Vec<(u32, u8)>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub trues: Vec<(u32, u32)>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lost: Vec<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bands: Vec<(u32, u8)>,
}

pub const MAX_LOST: usize = 256;

impl Default for Site {
    fn default() -> Self {
        Site::keep()
    }
}

impl Site {
    pub fn keep() -> Site {
        Site { plan: works::KEEP.to_string(), laid: START, marks: vec![(START, 0)], trues: Vec::new(), lost: Vec::new(), bands: Vec::new() }
    }

    pub fn stone_rgb(kind: u8) -> [f32; 3] {
        match kind {
            1 => [0.48, 0.70, 0.42],
            2 => [0.16, 0.15, 0.18],
            3 => [0.45, 0.62, 0.86],
            _ => [0.86, 0.78, 0.62],
        }
    }

    pub fn stone_kind(bands: &[(u32, u8)], index: u32) -> u8 {
        for &(end, kind) in bands {
            if index < end {
                return kind;
            }
        }
        0
    }

    pub fn note_kind(&mut self, n: u32, kind: u8) {
        if n == 0 {
            return;
        }
        let end = self.laid;
        if kind == 0 && self.bands.is_empty() {
            return;
        }
        if let Some(last) = self.bands.last_mut().filter(|b| b.1 == kind) {
            last.0 = end;
        } else {
            if self.bands.is_empty() && end > n {
                self.bands.push((end - n, 0));
            }
            self.bands.push((end, kind));
        }
    }

    pub fn castle(&self) -> Option<&'static Castle> {
        works::castle(&self.plan)
    }

    pub fn valid(&self) -> bool {
        let most = self.castle().map_or(1 << 20, Castle::len);
        self.plan.len() <= 64
            && self.laid <= most
            && self.marks.len() <= 4096
            && self.marks.windows(2).all(|w| w[0].0 < w[1].0)
            && self.marks.last().is_none_or(|&(end, _)| end <= self.laid)
            && self.marks.iter().all(|&(_, seat)| u32::from(seat) <= MAX_PLAYERS)
            && self.trues.len() <= 4096
            && self.trues.iter().all(|&(a, b)| a < b)
            && self.trues.windows(2).all(|w| w[0].1 <= w[1].0)
            && self.trues.last().is_none_or(|&(_, end)| end <= self.laid)
            && self.lost.len() <= MAX_LOST
            && self.lost.windows(2).all(|w| w[0] < w[1])
            && self.lost.last().is_none_or(|&i| i < self.laid)
    }

    pub fn mend(&mut self) {
        self.plan.truncate(64);
        let most = self.castle().map_or(1 << 20, Castle::len);
        self.laid = self.laid.min(most);
        self.marks.retain(|&(_, seat)| u32::from(seat) <= MAX_PLAYERS);
        self.marks.retain(|&(end, _)| end <= self.laid);
        let mut last = 0;
        self.marks.retain(|&(end, _)| {
            let keep = end > last;
            last = last.max(end);
            keep
        });
        self.marks.truncate(4096);
        let mut last = 0;
        let laid = self.laid;
        self.trues.retain(|&(a, b)| {
            let keep = a < b && a >= last && b <= laid;
            if keep {
                last = b;
            }
            keep
        });
        self.trues.truncate(4096);
        let laid = self.laid;
        self.lost.retain(|&i| i < laid);
        self.lost.sort_unstable();
        self.lost.dedup();
        self.lost.truncate(MAX_LOST);
    }

    pub fn setter(&self, index: u32) -> u8 {
        if index >= self.laid {
            return 0;
        }
        let at = self.marks.partition_point(|&(end, _)| end <= index);
        self.marks.get(at).map_or(0, |&(_, seat)| seat)
    }

    pub fn next(&self) -> u32 {
        self.lost.first().copied().unwrap_or(self.laid)
    }

    pub fn stands(&self, index: u32) -> bool {
        index < self.laid && self.lost.binary_search(&index).is_err()
    }

    pub fn lose(&mut self, stones: &[u32]) -> usize {
        let mut n = 0;
        for &i in stones {
            if self.lost.len() >= MAX_LOST || !self.stands(i) {
                continue;
            }
            let at = self.lost.partition_point(|&l| l < i);
            self.lost.insert(at, i);
            self.unmark_true(i);
            n += 1;
        }
        n
    }

    fn unmark_true(&mut self, index: u32) {
        let at = self.trues.partition_point(|&(_, end)| end <= index);
        let Some(&(a, b)) = self.trues.get(at).filter(|&&(a, _)| a <= index) else { return };
        self.trues.remove(at);
        let mut keep = Vec::new();
        if a < index {
            keep.push((a, index));
        }
        if index + 1 < b {
            keep.push((index + 1, b));
        }
        for (k, r) in keep.into_iter().enumerate() {
            self.trues.insert(at + k, r);
        }
    }

    fn mark_true(&mut self, index: u32) {
        if self.is_true(index) {
            return;
        }
        let at = self.trues.partition_point(|&(_, end)| end <= index);
        let joins_before = at > 0 && self.trues[at - 1].1 == index;
        let joins_after = self.trues.get(at).is_some_and(|&(a, _)| a == index + 1);
        match (joins_before, joins_after) {
            (true, true) => {
                let end = self.trues[at].1;
                self.trues[at - 1].1 = end;
                self.trues.remove(at);
            }
            (true, false) => self.trues[at - 1].1 = index + 1,
            (false, true) => self.trues[at].0 = index,
            (false, false) => self.trues.insert(at, (index, index + 1)),
        }
    }

    pub fn lay(&mut self, n: u32, seat: u8) {
        let fill = (n as usize).min(self.lost.len());
        self.lost.drain(..fill);
        let n = n - fill as u32;
        let most = self.castle().map_or(1 << 20, Castle::len);
        let to = (self.laid + n).min(most);
        if to == self.laid {
            return;
        }
        match self.marks.last_mut() {
            Some((end, last)) if *last == seat => *end = to,
            _ => self.marks.push((to, seat)),
        }
        self.laid = to;
        if self.marks.len() > 4000 {
            self.marks.remove(0);
        }
    }

    pub fn lay_set(&mut self, seat: u8, truth: bool) {
        if !self.lost.is_empty() {
            let index = self.lost.remove(0);
            if truth {
                self.mark_true(index);
            }
            return;
        }
        let index = self.laid;
        let before = self.laid;
        self.lay(1, seat);
        if truth && self.laid > before {
            match self.trues.last_mut() {
                Some((_, end)) if *end == index => *end = index + 1,
                _ => self.trues.push((index, index + 1)),
            }
            if self.trues.len() > 4000 {
                self.trues.remove(0);
            }
        }
    }

    pub fn is_true(&self, index: u32) -> bool {
        let at = self.trues.partition_point(|&(_, end)| end <= index);
        self.trues.get(at).is_some_and(|&(start, _)| start <= index)
    }

    pub fn true_count(&self) -> u32 {
        self.trues.iter().map(|&(a, b)| b - a).sum()
    }

    pub fn tally(&self) -> Vec<(u8, u32)> {
        let mut counts: BTreeMap<u8, u32> = BTreeMap::new();
        let mut from = 0;
        for &(end, seat) in &self.marks {
            *counts.entry(seat).or_default() += end - from;
            from = end;
        }
        let mut out: Vec<(u8, u32)> = counts.into_iter().collect();
        out.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        out
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Sites(pub Vec<Site>);

impl Default for Sites {
    fn default() -> Self {
        Sites(vec![Site::keep()])
    }
}

impl Sites {
    pub fn valid(&self) -> bool {
        self.0.len() <= MAX_SITES && self.0.iter().all(Site::valid)
    }

    pub fn mend(&mut self) {
        self.0.truncate(MAX_SITES);
        for s in &mut self.0 {
            s.mend();
        }
    }

    pub fn keep(&self) -> Option<&Site> {
        self.0.iter().find(|s| s.plan == works::KEEP)
    }

    pub fn keep_mut(&mut self) -> Option<&mut Site> {
        self.0.iter_mut().find(|s| s.plan == works::KEEP)
    }

    pub fn is_default(&self) -> bool {
        *self == Sites::default()
    }
}

pub fn patch_room(room: &mut Room, sites: &Sites) -> bool {
    let mut changed = false;
    let gate = crate::world::plan::tabula::DELVE_GATE;
    for site in &sites.0 {
        let Some(castle) = site.castle() else { continue };
        let (x0, y0, w, h) = castle.tiles();
        for ty in y0..y0 + h {
            for tx in x0..x0 + w {
                if tx < 0 || ty < 0 || tx as usize >= room.cols || ty as usize >= room.rows {
                    continue;
                }
                if tx >= gate.0 && ty >= gate.1 && tx < gate.0 + gate.2 && ty < gate.1 + gate.3 {
                    continue;
                }
                let want = if !castle.blocked_without(tx, ty, site.laid, &site.lost) {
                    Tile::Floor
                } else if castle.is_ditch(tx, ty) {
                    Tile::Hazard
                } else {
                    Tile::Wall
                };
                if room.tile(tx, ty) != want {
                    room.set(tx as usize, ty as usize, want);
                    changed = true;
                }
            }
        }
    }
    changed
}

impl Run {
    pub(super) fn tick_sites(&mut self, inputs: &BTreeMap<u32, Input>) {
        if !works::on_map() {
            return;
        }
        let Some(site) = self.sites.keep() else { return };
        let Some(castle) = site.castle() else { return };
        self.tick_chronicle(inputs, castle);
        self.tick_hands(inputs, castle);
        self.follow_site_tiles();
    }

    fn tick_chronicle(&mut self, inputs: &BTreeMap<u32, Input>, castle: &'static Castle) {
        if self.site_replay.is_some_and(|began| self.tick >= began + CHRONICLE_TICKS) {
            self.site_replay = None;
        }
        let (x0, y0, w, h) = castle.tiles();
        let geo = crate::world::kingdom::geography(&self.kingdom.wishes);
        let mut started = false;
        let mut holds = std::mem::take(&mut self.site_hold);
        holds.retain(|id, _| self.players.contains_key(id));
        for (&id, hero) in &self.players {
            let (tx, ty) = ((hero.x / TILE_UNITS) as i32, (hero.y / TILE_UNITS) as i32);
            let here = tx >= x0 && ty >= y0 && tx < x0 + w && ty < y0 + h;
            let held = inputs.get(&id).is_some_and(|i| i.valid() && i.interact);
            let at_door = door_at(&geo, hero.x, hero.y).is_some();
            if here && held && !at_door && hero.hp > 0 && !self.hands.busy.contains(&id) {
                let n = holds.entry(id).or_default();
                *n += 1;
                started |= *n == CHRONICLE_HOLD;
            } else {
                holds.remove(&id);
            }
        }
        self.site_hold = holds;
        if started && self.site_replay.is_none() {
            self.site_replay = Some(self.tick);
        }
    }

    pub fn site_shown(&self, site: &Site) -> u32 {
        match self.site_replay {
            Some(began) if self.tick >= began && self.tick < began + CHRONICLE_TICKS => {
                let t = (self.tick - began) as f32 / CHRONICLE_TICKS as f32;
                let eased = t * t * (3.0 - 2.0 * t);
                (site.laid as f32 * eased).round() as u32
            }
            _ => site.laid,
        }
    }

    pub(super) fn follow_site_tiles(&mut self) {
        if !works::on_map() {
            return;
        }
        let Some(i) = world::room_of(&self.dungeon, RoomKind::Kingdom) else { return };
        if patch_room(&mut self.dungeon.rooms[i], &self.sites) {
            self.dungeon.world_revision = self.dungeon.world_revision.wrapping_add(1);
        }
        self.follow_works_tiles(i);
    }

    pub fn lay_to(&mut self, plan: &str, laid: u32) {
        let site = match self.sites.0.iter().position(|s| s.plan == plan) {
            Some(i) => &mut self.sites.0[i],
            None => {
                self.sites.0.push(Site { plan: plan.to_string(), laid: 0, marks: Vec::new(), trues: Vec::new(), lost: Vec::new(), bands: Vec::new() });
                self.sites.0.last_mut().expect("just pushed")
            }
        };
        let most = site.castle().map_or(1 << 20, Castle::len);
        let laid = laid.min(most);
        if laid >= site.laid {
            site.lay(laid - site.laid, 0);
        } else {
            site.laid = laid;
            site.lost.retain(|&i| i < laid);
            site.trues.retain(|&(_, end)| end <= laid);
            site.marks.retain(|&(end, _)| end < laid);
            if site.marks.last().is_none_or(|&(end, _)| end < laid) {
                site.marks.push((laid, 0));
            }
        }
        self.follow_site_tiles();
    }
}

#[cfg(test)]
#[path = "../../tests/rules/site__tests.rs"]
mod tests;
