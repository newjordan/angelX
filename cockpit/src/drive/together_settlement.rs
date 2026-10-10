//! A loop's miners work in the Delve's home, not in a second generated world.
//! Only completed, durable LoopIterLog receipts buy work. Pictures never call
//! this module. The site ledger is separate from both realm spoils and live
//! dungeon checkpoints: coding cannot mint spendable Delve gold or replace a run.
pub(crate) mod exhibits;
pub(crate) mod planning;
use super::loop_ctl::LoopState;
use super::together_shooter::{COLS, Floor, Pack, ROWS, Room, RoomKind, Tile, layout};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::{Path, PathBuf},
};

const VERSION: u32 = 1;
const MAX_BYTES: u64 = 1024 * 1024;
const MAX_LOOPS: usize = 256;
const ROOMS: [(i32, i32, RoomKind); 6] = [
    (1, 2, RoomKind::Hall),
    (0, 2, RoomKind::Stockpile),
    (0, 1, RoomKind::Workshop),
    (0, 0, RoomKind::Quarters),
    (1, 0, RoomKind::Workshop),
    (1, -1, RoomKind::Stockpile),
];

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Resources {
    pub(crate) stone: u32,
    pub(crate) ore: u32,
    pub(crate) tools: u32,
    pub(crate) mined_stone: u32,
    pub(crate) mined_ore: u32,
    pub(crate) spent_stone: u32,
    pub(crate) spent_ore: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Receipt {
    iteration: usize,
    changed: bool,
    sequence: usize,
    /// Geography is fixed when a room's first cut is paid for. Research cues
    /// never supply a work ticket, and replay does not consult today's map.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    plan: Option<planning::Plan>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Site {
    version: u32,
    pub(crate) id: String,
    pub(crate) player: String,
    seed: u64,
    /// The global sequence preserves interleaved loops and permits replay validation.
    loops: BTreeMap<String, Vec<Receipt>>,
    pub(crate) floor: Floor,
    pub(crate) resources: Resources,
    /// Work tickets, not seconds: three cuts, a build, then a craft per room.
    pub(crate) work: u32,
    pub(crate) built: usize,
    pub(crate) tavern: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) exhibits: Vec<exhibits::Exhibit>,
}

impl Site {
    pub(crate) fn new(seed: u64, player: &str) -> Self {
        Self {
            version: VERSION,
            id: format!(
                "{seed:016x}-{}",
                crate::knowledge::cut::sha256_hex(player.as_bytes())
            ),
            player: player.into(),
            seed,
            loops: BTreeMap::new(),
            floor: layout::undercroft(Pack::Cavern),
            resources: Resources::default(),
            work: 0,
            built: 0,
            tavern: false,
            exhibits: Vec::new(),
        }
    }

    /// One completed iteration does one job. Duplicate/out-of-order receipts
    /// are harmless; missing iteration numbers are NOT inferred as work.
    pub(crate) fn apply(
        &mut self,
        loop_id: &str,
        iteration: usize,
        changed: bool,
    ) -> Result<bool, String> {
        self.apply_with_plan(loop_id, iteration, changed, None)
    }

    pub(crate) fn apply_with_plan(
        &mut self,
        loop_id: &str,
        iteration: usize,
        changed: bool,
        input: Option<&planning::Input>,
    ) -> Result<bool, String> {
        // A duplicate cannot replace its original flag or research provenance.
        if self.has_receipt(loop_id, iteration) {
            return Ok(false);
        }
        let plan = self.next_plan(input)?;
        self.apply_recorded(loop_id, iteration, changed, plan)
    }

    fn active_plan(&self, room: usize) -> Option<&planning::Plan> {
        self.loops
            .values()
            .flatten()
            .filter_map(|receipt| receipt.plan.as_ref().map(|plan| (receipt.sequence, plan)))
            .filter(|(_, plan)| plan.room == room)
            .max_by_key(|(sequence, _)| *sequence)
            .map(|(_, plan)| plan)
    }

    fn next_plan(&self, input: Option<&planning::Input>) -> Result<Option<planning::Plan>, String> {
        let room = (self.work / 5) as usize;
        if room >= ROOMS.len() {
            return Ok(None);
        }
        if !self.work.is_multiple_of(5) {
            return Ok(self.active_plan(room).cloned());
        }
        // Once geography branches, a missing map continues from the last
        // recorded cue rather than colliding with a future legacy coordinate.
        let previous = room.checked_sub(1).and_then(|room| self.active_plan(room));
        input
            .or_else(|| previous.map(|plan| &plan.input))
            .map(|input| planning::choose(self, room, input))
            .transpose()
    }

    fn apply_recorded(
        &mut self,
        loop_id: &str,
        iteration: usize,
        changed: bool,
        plan: Option<planning::Plan>,
    ) -> Result<bool, String> {
        if loop_id.is_empty() || loop_id.len() > 256 || iteration == 0 {
            return Err("invalid settlement loop receipt".into());
        }
        if self
            .loops
            .get(loop_id)
            .is_some_and(|rs| rs.iter().any(|r| r.iteration == iteration))
        {
            return Ok(false);
        }
        if !self.loops.contains_key(loop_id) && self.loops.len() >= MAX_LOOPS {
            return Err("settlement loop ledger is full".into());
        }
        let sequence = self.loops.values().map(Vec::len).sum::<usize>();
        if sequence >= 4096 {
            return Err("settlement iteration ledger is full".into());
        }
        let input = plan.as_ref().map(|plan| &plan.input);
        if self.next_plan(input)? != plan {
            return Err("settlement excavation plan does not match recorded history".into());
        }
        let receipts = self.loops.entry(loop_id.into()).or_default();
        if receipts.len() >= 4096 {
            return Err("settlement iteration ledger is full".into());
        }
        receipts.push(Receipt {
            iteration,
            changed,
            sequence,
            plan: plan.clone(),
        });
        self.job(changed, plan.as_ref());
        Ok(true)
    }

    fn job(&mut self, changed: bool, plan: Option<&planning::Plan>) {
        let room = (self.work / 5) as usize;
        let phase = self.work % 5;
        if room >= ROOMS.len() {
            // After the bounded wing is complete only actual changed candidates
            // can craft, and only while the site's mined ore lasts.
            if changed {
                self.craft();
            }
            return;
        }
        match phase {
            0..=2 => match plan {
                Some(plan) => self.cut_planned(plan, phase as usize),
                None => self.cut(room, phase as usize),
            },
            3 => {
                if !self.pay(12, 2) {
                    return;
                }
                let r = &mut self.floor.rooms[4 + room];
                r.kind = plan.map_or(ROOMS[room].2, |plan| plan.kind);
                // Distinct furniture in the shared collision map, never across
                // the central through-walk or a doorway.
                match r.kind {
                    RoomKind::Stockpile => {
                        for col in [3, 4, 6, 7, 9, 10] {
                            r.set(col, 3, Tile::Block);
                        }
                    }
                    RoomKind::Workshop => {
                        for (col, row) in [(3, 3), (4, 3), (4, 4), (7, 3)] {
                            r.set(col, row, Tile::Block);
                        }
                    }
                    RoomKind::Quarters => {
                        for col in [3, 6, 9] {
                            r.set(col, 3, Tile::Block);
                            r.set(col, 4, Tile::Block);
                        }
                    }
                    _ => {}
                }
                self.built += 1;
            }
            _ => {
                // An unchanged iteration may finish the build ticket, but it
                // cannot conjure a crafted item. All resources remain banked.
                if changed {
                    self.craft();
                }
            }
        }
        self.work += 1;
    }

    fn pay(&mut self, stone: u32, ore: u32) -> bool {
        let r = &mut self.resources;
        if r.stone < stone || r.ore < ore {
            return false;
        }
        r.stone -= stone;
        r.ore -= ore;
        r.spent_stone += stone;
        r.spent_ore += ore;
        true
    }

    fn craft(&mut self) {
        if self.built >= 3 && self.pay(2, 3) {
            self.resources.tools += 1;
        }
    }

    fn cut(&mut self, i: usize, slice: usize) {
        let (x, y, _) = ROOMS[i];
        if slice == 0 {
            let prev = if i == 0 { 0 } else { 4 + i - 1 };
            let (px, py) = self.floor.rooms[prev].cell;
            let dir = layout::DIRS
                .iter()
                .position(|&(dx, dy)| (px + dx, py + dy) == (x, y))
                .expect("adjacent wing");
            let mut r = Room::solid_rock((x, y));
            // A central crossing gives every partial excavation a reachable
            // front, whichever direction the next branch will take.
            for row in 1..ROWS - 1 {
                for col in 1..COLS - 1 {
                    if row.abs_diff(ROWS / 2) <= 1 || col.abs_diff(COLS / 2) <= 1 {
                        self.mine(&mut r, col, row);
                    }
                }
            }
            if i == 0 {
                // The old hall's chapel screen stands behind its west wall.
                // Cut an ingress through the screen as part of this real job;
                // reciprocal door flags alone do not make a usable passage.
                for row in [ROWS / 2 - 1, ROWS / 2] {
                    self.floor.rooms[0].set(1, row, Tile::Floor);
                }
            }
            self.floor.rooms[prev].open_door(dir);
            r.open_door((dir + 2) % 4);
            // Insert before the optional tavern so the wing's indices stay stable.
            self.floor.rooms.insert(4 + i, r);
        }
        let index = 4 + i;
        let mut r = self.floor.rooms[index].clone();
        for row in 1..ROWS - 1 {
            for col in 1..COLS - 1 {
                if (row - 1) * 3 / (ROWS - 2) <= slice {
                    self.mine(&mut r, col, row);
                }
            }
        }
        self.floor.rooms[index] = r;
    }

    fn cut_planned(&mut self, plan: &planning::Plan, slice: usize) {
        let index = 4 + plan.room;
        if slice == 0 {
            let mut room = Room::solid_rock(plan.target);
            // Keep a reachable working face and camera position throughout the
            // partial cut. The selected rock route adds a real connecting cut.
            for row in 1..ROWS - 1 {
                for col in 1..COLS - 1 {
                    if row.abs_diff(ROWS / 2) <= 1 || col.abs_diff(COLS / 2) <= 1 {
                        self.mine(&mut room, col, row);
                    }
                }
            }
            for &(col, row) in &plan.rock_route {
                self.mine(&mut room, col, row);
            }
            if plan.parent == 0 {
                for row in [ROWS / 2 - 1, ROWS / 2] {
                    self.floor.rooms[0].set(1, row, Tile::Floor);
                }
            }
            self.floor.rooms[plan.parent].open_door(plan.direction);
            room.open_door((plan.direction + 2) % 4);
            self.floor.rooms.insert(index, room);
        }
        let mut room = self.floor.rooms[index].clone();
        for row in 1..ROWS - 1 {
            for col in 1..COLS - 1 {
                if plan.axis.includes(col, row, slice) {
                    self.mine(&mut room, col, row);
                }
            }
        }
        self.floor.rooms[index] = room;
    }

    fn mine(&mut self, r: &mut Room, col: usize, row: usize) {
        if r.tile(col as i32, row as i32) != Tile::Wall {
            return;
        }
        r.set(col, row, Tile::Floor);
        let res = &mut self.resources;
        res.stone += 1;
        res.mined_stone += 1;
        if (col + row * COLS).is_multiple_of(7) {
            res.ore += 1;
            res.mined_ore += 1;
        }
    }

    /// A hall alone is not an invented loop association. Empty but real loops
    /// are recorded too, so their home can be resumed before any work receipt.
    pub(crate) fn associated(&self) -> bool {
        !self.loops.is_empty()
    }

    pub(crate) fn focus(&self) -> usize {
        if self.work == 0 {
            0
        } else {
            4 + ((self.work - 1) / 5) as usize
        }
        .min(4 + ROOMS.len() - 1)
    }

    pub(crate) fn activity(&self) -> &'static str {
        if self.work == 0 {
            "PLAYER HALL · miners muster at the west wall"
        } else if self.work >= ROOMS.len() as u32 * 5 {
            "Settlement wing complete · crafting from banked ore"
        } else {
            match self.work % 5 {
                0 => "Room furnished · tools crafted only from changed iterations",
                1..=3 => "Miners excavating · rock face retreats on loop receipts",
                _ => "Builders fitting the room · stone and ore paid from the stockpile",
            }
        }
    }

    fn valid(&self, expected: &Self) -> bool {
        if self.version != VERSION
            || self.id != expected.id
            || self.player != expected.player
            || self.seed != expected.seed
            || self.loops.len() > MAX_LOOPS
        {
            return false;
        }
        let mut events = Vec::new();
        for (id, receipts) in &self.loops {
            if id.is_empty() || id.len() > 256 || receipts.len() > 4096 {
                return false;
            }
            for r in receipts {
                events.push((r.sequence, id, r));
            }
        }
        if events.len() > 4096 {
            return false;
        }
        events.sort_by_key(|e| e.0);
        let mut replay = Self::new(self.seed, &self.player);
        for id in self.loops.keys() {
            replay.loops.insert(id.clone(), Vec::new());
        }
        for (n, (seq, id, r)) in events.into_iter().enumerate() {
            if n != seq
                || replay.apply_recorded(id, r.iteration, r.changed, r.plan.clone()) != Ok(true)
            {
                return false;
            }
        }
        if self.tavern {
            layout::dig_tavern(&mut replay.floor);
            replay.tavern = true;
        }
        if !self.restore_exhibits(&mut replay) {
            return false;
        }
        // Reject malformed maps, invented resources, duplicate receipts, and
        // incompatible saves before any picture or playable run sees the map.
        serde_json::to_value(self).ok() == serde_json::to_value(replay).ok()
    }
}

/// Atomic site transactions, with an advisory OS lock for concurrent cockpits.
/// A corrupt/unreadable ledger is an error, never a reason to start over.
pub(crate) struct Store {
    pub(crate) site: Site,
    path: Option<PathBuf>,
    seen: Option<(String, Vec<(usize, bool)>, bool)>,
}

impl Store {
    pub(crate) fn beside(rewards: Option<&Path>, seed: u64, player: &str) -> Result<Self, String> {
        let site = Site::new(seed, player);
        let path = rewards.map(|p| {
            p.with_extension(format!(
                "{}.settlement.json",
                crate::knowledge::cut::sha256_hex(player.as_bytes())
            ))
        });
        let site = match path.as_deref() {
            Some(p) => Self::load(p, &site)?.unwrap_or(site),
            None => site,
        };
        Ok(Self {
            site,
            path,
            seen: None,
        })
    }

    fn load(path: &Path, expected: &Site) -> Result<Option<Site>, String> {
        let mut options = std::fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW);
        }
        let file = match options.open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.to_string()),
        };
        let mut bytes = Vec::new();
        file.take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err("settlement ledger too large".into());
        }
        let site: Site =
            serde_json::from_slice(&bytes).map_err(|e| format!("settlement ledger: {e}"))?;
        if !site.valid(expected) {
            return Err("invalid settlement identity or state".into());
        }
        Ok(Some(site))
    }

    /// Entry routes refresh without deriving work or replacing a playable save.
    pub(crate) fn refresh(&mut self) -> Result<(), String> {
        if let Some(path) = &self.path {
            match Self::load(path, &self.site)? {
                Some(site) => self.site = site,
                None if self.site.loops.is_empty() => {}
                None => return Err("settlement ledger disappeared".into()),
            }
        }
        self.seen = None;
        Ok(())
    }

    pub(crate) fn sync(&mut self, st: &LoopState, tavern: bool) -> Result<bool, String> {
        self.sync_with_plan(st, tavern, None)
    }

    /// A current frontier is usable only for one newly completed iteration.
    /// Catch-up must not relabel old receipts with today's research map.
    pub(crate) fn sync_with_plan(
        &mut self,
        st: &LoopState,
        tavern: bool,
        input: Option<&planning::Input>,
    ) -> Result<bool, String> {
        self.sync_planned(st, tavern, |next, id, rows, iteration| {
            let unseen = rows
                .iter()
                .filter(|(n, _)| !next.has_receipt(id, *n))
                .count();
            (unseen == 1 && iteration == st.iteration)
                .then_some(input)
                .flatten()
                .cloned()
        })
    }

    /// Per-iteration snapshots from the canonical loop log can survive resume
    /// and catch-up without attributing a newer frontier to an older job.
    pub(crate) fn sync_with_plans(
        &mut self,
        st: &LoopState,
        tavern: bool,
        inputs: &BTreeMap<usize, planning::Input>,
    ) -> Result<bool, String> {
        if inputs.len() > 4096 {
            return Err("settlement research input ledger is full".into());
        }
        self.sync_planned(st, tavern, |_, _, _, iteration| {
            inputs.get(&iteration).cloned()
        })
    }

    fn sync_planned(
        &mut self,
        st: &LoopState,
        tavern: bool,
        input: impl Fn(&Site, &str, &[(usize, bool)], usize) -> Option<planning::Input>,
    ) -> Result<bool, String> {
        let durable = st.durable_settlement_events();
        let has_loop = durable.is_some();
        // A paid realm wing may be built between loops. Reflect that existing
        // purchase without inventing a loop association or excavation receipt.
        if !has_loop && (!tavern || !self.site.associated() || self.site.tavern) {
            return Ok(false);
        }
        let facts = (
            durable
                .as_ref()
                .map_or_else(String::new, |d| d.loop_id.clone()),
            durable.map_or_else(Vec::new, |d| d.rows),
            tavern,
        );
        if self.seen.as_ref() == Some(&facts) {
            return Ok(false);
        }
        let (_, changed) = self.transaction(|next| {
            if has_loop && !next.loops.contains_key(&facts.0) {
                if next.loops.len() >= MAX_LOOPS {
                    return Err("settlement loop ledger is full".into());
                }
                next.loops.insert(facts.0.clone(), Vec::new());
            }
            if has_loop {
                // Resolve inputs against the pre-transaction site, before any
                // catch-up row makes the remaining row appear to be live.
                let inputs: BTreeMap<_, _> = facts
                    .1
                    .iter()
                    .filter_map(|&(iteration, _)| {
                        input(next, &facts.0, &facts.1, iteration).map(|input| (iteration, input))
                    })
                    .collect();
                for &(iteration, workspace_changed) in &facts.1 {
                    next.apply_with_plan(
                        &facts.0,
                        iteration,
                        workspace_changed,
                        inputs.get(&iteration),
                    )?;
                }
            }
            if tavern && !next.tavern {
                layout::dig_tavern(&mut next.floor);
                next.tavern = true;
            }
            Ok(())
        })?;
        self.seen = Some(facts);
        Ok(changed)
    }

    /// Reload under the site lock; publish neither partial manifests nor failed
    /// writes. The returned bool includes another cockpit's committed changes.
    fn transact<T>(
        &mut self,
        update: impl FnOnce(&mut Site) -> Result<T, String>,
    ) -> Result<T, String> {
        self.transaction(update).map(|(result, _)| result)
    }

    fn transaction<T>(
        &mut self,
        update: impl FnOnce(&mut Site) -> Result<T, String>,
    ) -> Result<(T, bool), String> {
        let _lock = if let Some(p) = &self.path {
            let parent = p.parent().ok_or("invalid settlement path")?;
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            let mut options = std::fs::OpenOptions::new();
            options.read(true).write(true).create(true).truncate(false);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
            }
            let lock = options
                .open(p.with_extension("lock"))
                .map_err(|e| e.to_string())?;
            // A process forked anywhere in the cockpit holds a copy of this
            // descriptor until it execs, so a lock just released can still
            // read as held for a moment. Wait that out before calling it busy.
            let mut tries = 0;
            loop {
                match lock.try_lock() {
                    Ok(()) => break,
                    Err(std::fs::TryLockError::WouldBlock) if tries < 50 => {
                        tries += 1;
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(e) => return Err(format!("settlement is busy: {e}")),
                }
            }
            Some(lock)
        } else {
            None
        };
        let mut next = match self.path.as_deref() {
            Some(p) => match Self::load(p, &self.site)? {
                Some(site) => site,
                None if self.site.loops.is_empty() => self.site.clone(),
                None => return Err("settlement ledger disappeared".into()),
            },
            None => self.site.clone(),
        };
        let before = serde_json::to_vec(&next).map_err(|e| e.to_string())?;
        let result = update(&mut next)?;
        let after = serde_json::to_vec(&next).map_err(|e| e.to_string())?;
        if after.len() as u64 > MAX_BYTES || !next.valid(&self.site) {
            return Err("invalid or oversized settlement transaction".into());
        }
        if before != after
            && let Some(p) = &self.path
        {
            Self::save(p, &next)?;
        }
        let changed = serde_json::to_vec(&self.site).map_err(|e| e.to_string())? != after;
        self.site = next;
        self.seen = None;
        Ok((result, changed))
    }

    fn save(path: &Path, site: &Site) -> Result<(), String> {
        let bytes = serde_json::to_vec(site).map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err("settlement ledger too large".into());
        }
        let temp = path.with_extension(format!("{}.next", std::process::id()));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        // Never remove someone else's temp or overwrite an invalid ledger.
        let mut file = options.open(&temp).map_err(|e| e.to_string())?;
        let result = (|| -> std::io::Result<()> {
            file.write_all(&bytes)?;
            file.sync_all()?;
            std::fs::rename(&temp, path)?;
            std::fs::File::open(path.parent().expect("has parent"))?.sync_all()
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        result.map_err(|e| e.to_string())
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_settlement__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_settlement__artifact_tests.rs"]
mod artifact_tests;

#[cfg(test)]
#[path = "../../../tests/cockpit/together_settlement/planning__tests.rs"]
mod planning_tests;
