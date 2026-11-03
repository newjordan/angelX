//! Open floors at play (`layout::open`): the companies wait in their
//! chambers until a knight comes, fight where they are met and never chase
//! past their leash; the guardian's chamber shuts behind the party while
//! its guardian lives; a chamber whose company is dead pays as a cleared
//! room does, once.
//!
//! The switch: Dame Fortune's Open Floors wedge, or `Run::open_floors`
//! (the host's `ANGEL_DELVE_OPEN=1`). Without either nothing here runs.

use super::*;
use layout::Open;

/// The most foes an open floor raises at its start.
pub const OPEN_FOES: usize = 96;
/// The most foes awake at once on an open map; the rest wait.
pub const AWAKE_CAP: usize = 32;
/// How near a knight in sight wakes a dormant company (units).
pub const WAKE_REACH: f32 = 16.0;
/// How far past its chamber's stone a foe may go (units).
pub const LEASH: f32 = 12.0;
/// A company with no knight this near dozes off again (units).
pub const SLEEP_REACH: f32 = 60.0;
/// How far a scroll's nova reaches on an open map (bombs use `bombs::at`).
pub const OPEN_BLAST: f32 = 24.0;
/// A knight's bomb this near the crack brings the wall down.
pub const OPEN_CRACK_REACH: f32 = 8.0;
/// The most cards lying about an open map.
pub const OPEN_ITEMS: usize = 96;


/// A floor's leashes, on the stack: chamber index to stone plus `LEASH`.
pub(super) struct Leashes {
    bounds: [(f32, f32, f32, f32); layout::open::MAX_CHAMBERS],
    n: usize,
}

impl Leashes {
    pub(super) fn get(&self, i: usize) -> Option<&(f32, f32, f32, f32)> {
        (i < self.n).then(|| &self.bounds[i])
    }
}

/// Chamber stone copied onto the stack so a tick can read it while the
/// floor itself is borrowed for the fight. Rosters and gates stay put.
#[derive(Clone, Copy)]
struct ChamberSnap {
    bounds: (f32, f32, f32, f32),
    centre: (f32, f32),
    kind: RoomKind,
    cleared: bool,
    sealed: bool,
}

impl ChamberSnap {
    fn holds(self, x: f32, y: f32, inset: f32) -> bool {
        let (x0, y0, x1, y1) = self.bounds;
        x >= x0 + TILE_UNITS + inset
            && x < x1 - TILE_UNITS - inset
            && y >= y0 + TILE_UNITS + inset
            && y < y1 - TILE_UNITS - inset
    }

    fn guarded(self) -> bool {
        matches!(self.kind, RoomKind::Stairs | RoomKind::Lair)
    }

    fn contains(self, x: f32, y: f32) -> bool {
        let (x0, y0, x1, y1) = self.bounds;
        x >= x0 && x < x1 && y >= y0 && y < y1
    }
}

struct OpenView {
    map: usize,
    guardian: usize,
    populated: bool,
    n: usize,
    chambers: [ChamberSnap; layout::open::MAX_CHAMBERS],
}

fn nearest_snap(chambers: &[ChamberSnap], x: f32, y: f32) -> usize {
    chambers.iter().position(|ch| ch.contains(x, y)).unwrap_or_else(|| {
        (0..chambers.len())
            .min_by(|&a, &b| {
                let d = |i: usize| {
                    let (cx, cy) = chambers[i].centre;
                    (cx - x).hypot(cy - y)
                };
                d(a).total_cmp(&d(b))
            })
            .unwrap_or(0)
    })
}

impl Run {
    /// Whether this delve's floors are laid open: the wheel's wedge, or the
    /// host's switch (the run's own, or the host's now, for a save made
    /// before it was thrown).
    pub fn wants_open(&self) -> bool {
        self.open_floors || self.mode == fortune::Mode::OpenFloors || crate::settings::get().open_floors
    }

    /// The open map, when the party stands on it.
    pub fn open_here(&self) -> Option<&Open> {
        self.dungeon
            .open
            .as_ref()
            .filter(|o| o.map == self.at && self.dungeon.rooms.get(self.at).is_some_and(|r| r.kind == RoomKind::Depths))
    }

    /// The party stands on an open floor's map.
    pub fn open_map(&self) -> bool {
        self.open_here().is_some()
    }

    /// Something fights the party: in a room, anything left in it; on an
    /// open map, a company awake.
    pub fn fight_on(&self) -> bool {
        if self.open_map() {
            self.enemies.iter().any(|e| e.hp > 0 && !e.dormant && e.kind != EnemyKind::Dummy)
        } else {
            self.foes_left()
        }
    }

    /// The moment is a fight: a room's `Phase::Fighting`, or a company
    /// awake on an open map.
    pub fn fighting(&self) -> bool {
        self.phase == Phase::Fighting || (self.open_map() && self.fight_on())
    }

    /// Where the party steps onto the map: out of the door of the room it
    /// left, or at the arrival.
    pub(super) fn open_landing(&self, index: usize, came_from: usize) -> Option<(f32, f32)> {
        let open = self.dungeon.open.as_ref().filter(|o| o.map == index)?;
        Some(
            open.doors
                .iter()
                .find(|d| d.room == came_from && came_from != index)
                .map_or(open.arrival, layout::SideDoor::landing),
        )
    }

    /// Room for `count` knights round `(x, y)` on the map: each spot clear
    /// of stone.
    pub fn open_spots(&self, index: usize, at: (f32, f32), count: usize) -> Vec<(f32, f32)> {
        self.open_spots_where(index, at, count, |_, _| true)
    }

    /// As `open_spots`, each spot one `keep` allows (`(x, y)` itself when
    /// none is).
    fn open_spots_where(
        &self,
        index: usize,
        (x, y): (f32, f32),
        count: usize,
        keep: impl Fn(f32, f32) -> bool,
    ) -> Vec<(f32, f32)> {
        let grid = Grid { room: &self.dungeon.rooms[index], barred: self.barred() };
        let mut out: Vec<(f32, f32)> = Vec::with_capacity(count);
        let rings = [0.0f32, 2.4, 4.8, 7.2];
        'next: for slot in 0..count {
            for r in rings {
                for k in 0..8 {
                    let a = (k as f32 + slot as f32 * 0.37) * std::f32::consts::TAU / 8.0;
                    let (px, py) = (x + a.cos() * r, y + a.sin() * r);
                    let reached = grid.halted((x, y), (px - x, py - y));
                    let there = (reached.0 - px).hypot(reached.1 - py) < 0.2;
                    if keep(px, py)
                        && there
                        && grid.clear(px, py, HERO_RADIUS + 0.1, Mover::Hero)
                        && !out.iter().any(|&(ox, oy)| (ox - px).hypot(oy - py) < 1.6)
                    {
                        out.push((px, py));
                        continue 'next;
                    }
                }
            }
            out.push((x, y));
        }
        out
    }

    /// The party walks onto the map: its companies as they were left (or
    /// raised, the first time), no door barred, nothing to wait for.
    pub(super) fn enter_open(&mut self, index: usize, first_visit: bool) {
        self.dungeon.rooms[index].visited = true;
        if !self.parked.is_empty() {
            self.enemies = std::mem::take(&mut self.parked);
        } else if self.dungeon.open.as_ref().is_some_and(|o| !o.populated) {
            self.populate_open();
        }
        self.phase = Phase::Exploring;
        self.room_hurt = false;
        self.rune = None;
        self.waves = Waves::default();
        self.traps.clear();
        self.rocks.clear();
        // The dragon's floor, its dragon slain: the light home still burns.
        if let Some(open) = &self.dungeon.open {
            let g = &open.chambers[usize::from(open.guardian)];
            if g.kind == RoomKind::Lair && g.cleared {
                let (cx, cy) = g.centre();
                self.light = Some((cx, cy - 4.0));
            }
        }
        self.works_enter();
        self.notice_crack(first_visit);
        self.chart_open();
        self.choose_altars();
    }

    /// Raise every chamber's company at its spots, dormant: the room's own
    /// roster, the floor's extra foes and the party's
    /// (`stakes::EXTRA_PER_KNIGHT`), more the farther it lies from the
    /// stairs (`stakes::ramp`), the guardian alone on the first floors
    /// (`BOSS_ALONE_TO`), within `OPEN_FOES`.
    pub(super) fn populate_open(&mut self) {
        let Some(open) = self.dungeon.open.clone() else {
            return;
        };
        // Sized for the party as it is now, as a step would size them.
        let before = self.scaled();
        self.rescale_party(before);
        let depth = self.dungeon.depth;
        let pack = self.dungeon.pack;
        let index = (depth.clamp(1, 6) - 1) as usize;
        let knights = self.party_len().max(1);
        let circle = self.spectacle.circle;
        let far = open.chambers.iter().map(|c| u32::from(c.dist)).max().unwrap_or(0).max(1);
        let gunners: Vec<(EnemyKind, u32)> = pack
            .roster_at(depth)
            .iter()
            .copied()
            .filter(|&(kind, _)| !matches!(kind, EnemyKind::Bat | EnemyKind::Slime | EnemyKind::Spiderling))
            .collect();
        // The first fight off the stairs, gentle on the first floors
        // (`stakes::GENTLE_FIRST_TO`): none of the party's gunners in it.
        let first_fight = open
            .chambers
            .iter()
            .filter(|c| c.kind != RoomKind::Start && !c.guarded() && !c.roster.is_empty())
            .map(|c| c.dist)
            .min();
        // Each chamber's own company, and its extras.
        let mut own: Vec<Vec<EnemyKind>> = Vec::with_capacity(open.chambers.len());
        let mut extra: Vec<usize> = Vec::with_capacity(open.chambers.len());
        for ch in &open.chambers {
            let mut roster = ch.roster.clone();
            let more = tune::more_foes(&roster, self.tune.foe_count);
            roster.extend(more);
            let guarded = ch.guarded();
            let alone = guarded && depth <= BOSS_ALONE_TO;
            let ramp = stakes::RAMP_FIRST + (100 - stakes::RAMP_FIRST) * u32::from(ch.dist) / far;
            let n = if roster.is_empty() || ch.kind == RoomKind::Start || alone || ch.kind == RoomKind::Lair {
                0
            } else {
                // Hell keeps no gentle room, and every company is a horde.
                let gentle = depth <= stakes::GENTLE_FIRST_TO && Some(ch.dist) == first_fight && circle == 0;
                let base = (if guarded { 0 } else { stakes::EXTRA_FOES[index] + hell::horde(circle) })
                    + if gentle { 0 } else { stakes::party_extra(knights) };
                let share = if guarded { 100 } else { ramp as usize };
                let n = base * share / 100;
                if ch.kind == RoomKind::Hall { n / 2 } else { n }
            };
            own.push(roster);
            extra.push(n);
        }
        // Within the floor's cap: the extras go first, the biggest first,
        // then the plain rosters' tails; the guardian's own never.
        let total = |own: &[Vec<EnemyKind>], extra: &[usize]| own.iter().map(Vec::len).sum::<usize>() + extra.iter().sum::<usize>();
        while total(&own, &extra) > OPEN_FOES {
            if let Some(i) = (0..extra.len()).filter(|&i| extra[i] > 0).max_by_key(|&i| (extra[i], i)) {
                extra[i] -= 1;
            } else if let Some(i) = (0..own.len())
                .filter(|&i| !open.chambers[i].guarded() && own[i].len() > 1)
                .max_by_key(|&i| (own[i].len(), i))
            {
                own[i].pop();
            } else {
                break;
            }
        }
        let guardians: Vec<usize> = (0..self.bosses.len())
            .filter(|&i| self.bosses[i].only_in == pack && self.bosses[i].id != nooks::gnome::FLAMINGO && !deep::dread::is(&self.bosses[i]))
            .collect();
        let pick = (mix(self.seed ^ self.raid_id.rotate_left(17)) % guardians.len().max(1) as u64) as usize;
        // The Gnome's Garden's floor is the Lawn Flamingo's (`nooks::gnome`).
        let flamingo = (self.dungeon.depth == nooks::gnome::DEPTH
            && self.dungeon.rooms.iter().any(|r| r.kind == RoomKind::GnomeParty))
        .then(|| self.flamingo())
        .flatten();
        // The floor above a village of the Deep: its people's dread holds
        // the stair (`deep::dread`).
        let dread = self.dread_at();
        for (i, ch) in open.chambers.iter().enumerate() {
            let first = self.enemies.len();
            let mut spots = self.chamber_spots(&open, i);
            let mut next = |run: &mut Run| -> (f32, f32) {
                spots.pop().unwrap_or_else(|| {
                    let (cx, cy) = ch.centre();
                    (cx + run.rng.below(5) as f32 - 2.0, cy + run.rng.below(5) as f32 - 2.0)
                })
            };
            let mut roster = std::mem::take(&mut own[i]);
            // On the garden's floor the flamingo holds the guardian's
            // chamber, a lair's dragon gone from it.
            let flamingos = flamingo.is_some() && i == usize::from(open.guardian);
            if flamingos {
                roster.retain(|&k| k != EnemyKind::Dragon);
            }
            if (ch.kind == RoomKind::Stairs || flamingos)
                && let Some(boss) = flamingo.or(dread).or_else(|| guardians.get(pick).copied())
            {
                // The guardian at its chamber's head, facing the way in.
                let head = self.open_head(&open, i);
                let at = self.enemies.len();
                self.spawn_boss(boss);
                if let Some(e) = self.enemies.get_mut(at) {
                    (e.x, e.y) = head;
                    (e.origin_x, e.origin_y) = head;
                }
                if depth > BOSS_ALONE_TO {
                    roster.truncate(2);
                } else {
                    if let Some(e) = self.enemies.get_mut(at) {
                        e.hoard = roster.iter().copied().take(2).collect();
                    }
                    roster.clear();
                }
            }
            for kind in roster {
                let (x, y) = if kind == EnemyKind::Dragon { self.open_head(&open, i) } else { next(self) };
                self.spawn_body(kind, x, y);
            }
            for _ in 0..extra[i] {
                if gunners.is_empty() {
                    break;
                }
                let kind = self.rng.pick(&gunners);
                let (x, y) = next(self);
                self.spawn_body(kind, x, y);
            }
            // Now and then, deep down, one of them a tougher one of its kind.
            if depth >= 3
                && !ch.guarded()
                && self.enemies.len() > first
                && self.rng.chance(stakes::ELITE_CHANCE[index])
                && let Some(elite) = self.enemies[first..]
                    .iter_mut()
                    .filter(|e| !e.elite && e.kind != EnemyKind::Dummy && e.kind != EnemyKind::Ward)
                    .max_by_key(|e| e.kind.hp())
            {
                elite.elite = true;
                let base = elite.base_hp() * 5 / 2;
                elite.base_hp = Some(base);
                elite.max_hp = elite.max_hp * 5 / 2;
                elite.hp = elite.max_hp;
                elite.seen_hp = elite.hp;
                elite.size = Some(elite.kind.radius() * 1.3);
                elite.fury = elite.fury.saturating_add(1);
            }
            // A circle's companies are led by champions.
            if !ch.guarded() {
                self.crown_champions(first);
            }
            for e in &mut self.enemies[first..] {
                e.home = Some(i as u8);
                e.dormant = true;
            }
        }
        if let Some(open) = &mut self.dungeon.open {
            open.populated = true;
            let homes: Vec<u8> = self.enemies.iter().filter_map(|e| e.home).collect();
            for (i, ch) in open.chambers.iter_mut().enumerate() {
                // Nobody to meet: nothing to pay for.
                if !homes.contains(&(i as u8)) {
                    ch.cleared = true;
                }
            }
        }
    }

    /// A chamber's floor spots for its company, shuffled: open floor in
    /// its middle, clear of its mouths and doors and of the arrival.
    fn chamber_spots(&mut self, open: &Open, i: usize) -> Vec<(f32, f32)> {
        let ch = &open.chambers[i];
        let room = &self.dungeon.rooms[open.map];
        let (x, y, w, h) = (i32::from(ch.rect.0), i32::from(ch.rect.1), i32::from(ch.rect.2), i32::from(ch.rect.3));
        let mut spots = Vec::new();
        for r in y + 2..y + h - 2 {
            for c in x + 2..x + w - 2 {
                let open_round = (-1..=1).all(|dr| (-1..=1).all(|dc| room.tile(c + dc, r + dr) == Tile::Floor));
                if !open_round {
                    continue;
                }
                let (px, py) = ((c as f32 + 0.5) * TILE_UNITS, (r as f32 + 0.5) * TILE_UNITS);
                if (px - open.arrival.0).hypot(py - open.arrival.1) < 12.0 {
                    continue;
                }
                spots.push((px, py));
            }
        }
        // Shuffle, then keep them apart.
        for k in (1..spots.len()).rev() {
            let j = self.rng.below(k + 1);
            spots.swap(k, j);
        }
        let mut kept: Vec<(f32, f32)> = Vec::new();
        for s in spots {
            if !kept.iter().any(|k| (k.0 - s.0).hypot(k.1 - s.1) < 3.0) {
                kept.push(s);
            }
        }
        kept
    }

    /// The head of chamber `i`: its north end, the middle, on open floor.
    fn open_head(&self, open: &Open, i: usize) -> (f32, f32) {
        let ch = &open.chambers[i];
        let room = &self.dungeon.rooms[open.map];
        let (cx, _) = ch.centre();
        let (_, y0, _, y1) = ch.bounds();
        let want = (cx, y0 + TILE_UNITS + 6.0);
        let (x, y, w, h) = (i32::from(ch.rect.0), i32::from(ch.rect.1), i32::from(ch.rect.2), i32::from(ch.rect.3));
        let col = (want.0 / TILE_UNITS) as i32;
        let row = (want.1 / TILE_UNITS) as i32;
        let open3 = |c: i32, r: i32| (-1..=1).all(|dc| (-1..=1).all(|dr| room.tile(c + dc, r + dr) == Tile::Floor));
        if open3(col, row) {
            return want;
        }
        (y + 2..y + h - 2)
            .flat_map(|r| (x + 2..x + w - 2).map(move |c| (c, r)))
            .filter(|&(c, r)| open3(c, r))
            .min_by_key(|&(c, r)| (c - col).abs() + (r - row).abs())
            .map_or((cx, (y0 + y1) / 2.0), |(c, r)| ((c as f32 + 0.5) * TILE_UNITS, (r as f32 + 0.5) * TILE_UNITS))
    }

    /// Where a monster comes onto the map mid-fight: in the chamber
    /// nearest the party, on open floor away from the knights.
    pub(super) fn open_spawn_spot(&mut self) -> Option<((f32, f32), u8)> {
        let Some(view) = self.open_view() else {
            return None;
        };
        let mut sx = 0.0;
        let mut sy = 0.0;
        let mut n = 0usize;
        for h in self.players.values().filter(|h| h.hp > 0 && !h.stone) {
            sx += h.x;
            sy += h.y;
            n += 1;
        }
        let n = n.max(1) as f32;
        let i = nearest_snap(&view.chambers[..view.n], sx / n, sy / n);
        let ch = view.chambers[i];
        let (x0, y0, x1, y1) = ch.bounds;
        let spots = self.open_spots_where(view.map, ch.centre, 64, |x, y| x >= x0 && y >= y0 && x < x1 && y < y1);
        let far = |s: &(f32, f32)| {
            self.players.values().filter(|h| h.hp > 0 && !h.stone).all(|h| (h.x - s.0).hypot(h.y - s.1) > 8.0)
        };
        let spot = spots.iter().copied().find(far).or_else(|| spots.first().copied())?;
        Some((spot, i as u8))
    }

    /// The leash rect of chamber `home`: its stone and `LEASH` round it.
    pub(super) fn leashes(&self) -> Leashes {
        let mut bounds = [(0.0, 0.0, 0.0, 0.0); layout::open::MAX_CHAMBERS];
        let Some(open) = self.open_here() else {
            return Leashes { bounds, n: 0 };
        };
        let n = open.chambers.len().min(bounds.len());
        for (i, ch) in open.chambers.iter().take(n).enumerate() {
            let (x0, y0, x1, y1) = ch.bounds();
            bounds[i] = (x0 - LEASH, y0 - LEASH, x1 + LEASH, y1 + LEASH);
        }
        Leashes { bounds, n }
    }

    /// One tick of the open map, after the kills are settled: the
    /// homeless take a home, the chambers are charted, companies wake and
    /// doze, the guardian's chamber seals and opens, and a chamber whose
    /// company is dead pays.
    pub(super) fn tick_open(&mut self) {
        let Some(view) = self.open_view() else {
            return;
        };
        if view.n == 0 {
            return;
        }
        let chambers = &view.chambers[..view.n];
        for e in &mut self.enemies {
            if e.home.is_none() {
                e.home = Some(nearest_snap(chambers, e.x, e.y) as u8);
            }
        }
        self.chart_open();
        let mut knights = [(0u32, 0.0f32, 0.0f32); MAX_PLAYERS as usize];
        let mut nk = 0;
        for (&id, h) in &self.players {
            if h.hp > 0 && !h.stone && !self.away(id) && nk < knights.len() {
                knights[nk] = (id, h.x, h.y);
                nk += 1;
            }
        }
        let knights = &knights[..nk];
        let g = view.guardian;
        let barred = self.barred();
        let mut wake = [0usize; layout::open::MAX_CHAMBERS];
        let mut nw = 0;
        {
            let grid = Grid { room: &self.dungeon.rooms[view.map], barred };
            for (i, ch) in chambers.iter().enumerate() {
                if ch.cleared || (i == g && ch.guarded()) {
                    continue;
                }
                let inside = knights.iter().any(|&(_, x, y)| ch.holds(x, y, 0.0));
                let stirred = inside
                    || self.enemies.iter().filter(|e| e.home == Some(i as u8) && e.dormant && e.hp > 0).any(|e| {
                        e.wits.foe != 0
                            || knights.iter().any(|&(_, x, y)| {
                                (e.x - x).hypot(e.y - y) <= WAKE_REACH && grid.shot_stop((x, y), (e.x, e.y)).is_none()
                            })
                    });
                if stirred {
                    wake[nw] = i;
                    nw += 1;
                }
            }
        }
        for i in &wake[..nw] {
            self.wake_chamber(*i);
        }
        let gc = chambers[g];
        if gc.guarded()
            && !gc.cleared
            && !gc.sealed
            && let Some(&(id, x, y)) = knights.iter().find(|&&(_, x, y)| gc.holds(x, y, 1.0))
        {
            self.seal_guardian(id, (x, y));
        }
        // Nobody left standing inside it (knocked out through its gate,
        // say): it opens again, and shuts behind the next knight in.
        if gc.sealed
            && !gc.cleared
            && !self.players.values().any(|h| h.hp > 0 && !h.absent && gc.holds(h.x, h.y, 0.0))
            && let Some(o) = self.dungeon.open.as_mut()
        {
            o.chambers[g].sealed = false;
        }
        // A company far from every knight dozes off.
        if self.tick.is_multiple_of(u64::from(HZ)) {
            for (i, ch) in chambers.iter().enumerate() {
                if ch.sealed || ch.cleared {
                    continue;
                }
                let (cx, cy) = ch.centre;
                let near = knights.iter().any(|&(_, x, y)| (x - cx).hypot(y - cy) <= SLEEP_REACH);
                if !near {
                    for e in self.enemies.iter_mut().filter(|e| e.home == Some(i as u8) && !e.dormant && !e.is_boss()) {
                        e.dormant = true;
                    }
                }
            }
        }
        // A chamber whose company is dead pays, once.
        for i in 0..view.n {
            let cleared = self.dungeon.open.as_ref().unwrap().chambers[i].cleared;
            if cleared || !view.populated {
                continue;
            }
            let alive = self
                .enemies
                .iter()
                .any(|e| e.home == Some(i as u8) && e.hp > 0 && e.kind != EnemyKind::Dummy);
            if !alive {
                self.chamber_clear(i);
            }
        }
    }

    /// Where the party stands, marked on the chart: a chamber is seen once
    /// a living knight is in it. The stone is read in place; the roster is not.
    pub(super) fn chart_open(&mut self) {
        let at = self.at;
        let players = &self.players;
        let Some(open) = self.dungeon.open.as_mut().filter(|o| o.map == at) else {
            return;
        };
        for ch in &mut open.chambers {
            if !ch.seen && players.values().any(|h| h.hp > 0 && !h.absent && ch.holds(h.x, h.y, -TILE_UNITS)) {
                ch.seen = true;
            }
        }
    }

    /// The floor's stone, copied. Enough to ask who is inside; the lists stay.
    fn open_view(&self) -> Option<OpenView> {
        let open = self.open_here()?;
        let mut chambers = [ChamberSnap {
            bounds: (0.0, 0.0, 0.0, 0.0),
            centre: (0.0, 0.0),
            kind: RoomKind::Hall,
            cleared: true,
            sealed: false,
        }; layout::open::MAX_CHAMBERS];
        let n = open.chambers.len().min(chambers.len());
        for (i, ch) in open.chambers.iter().take(n).enumerate() {
            chambers[i] = ChamberSnap {
                bounds: ch.bounds(),
                centre: ch.centre(),
                kind: ch.kind,
                cleared: ch.cleared,
                sealed: ch.sealed,
            };
        }
        Some(OpenView {
            map: open.map,
            guardian: usize::from(open.guardian),
            populated: open.populated,
            n,
            chambers,
        })
    }

    /// Chamber `i`'s company wakes (as many as `AWAKE_CAP` lets): a short
    /// telegraph, then the fight. The first time, the moment is a fight's.
    /// A big company comes on in waves (`stakes::open_wave`): the rest
    /// wake as those fighting fall.
    fn wake_chamber(&mut self, i: usize) {
        let mut awake = self.enemies.iter().filter(|e| e.hp > 0 && !e.dormant).count();
        let wave = stakes::open_wave(self.party_len());
        let mut fighting = self.enemies.iter().filter(|e| e.home == Some(i as u8) && e.hp > 0 && !e.dormant).count();
        let mut woke = 0;
        for e in self.enemies.iter_mut().filter(|e| e.home == Some(i as u8) && e.dormant && e.hp > 0) {
            if awake >= AWAKE_CAP || fighting >= wave {
                break;
            }
            fighting += 1;
            e.dormant = false;
            e.age = TELEGRAPH / 2;
            awake += 1;
            woke += 1;
        }
        let Some(ch) = self.dungeon.open.as_mut().map(|o| &mut o.chambers[i]) else {
            return;
        };
        if woke > 0 && !ch.stirred {
            ch.stirred = true;
            self.room_hurt = false;
            // A fight closes the window: wishes come between fights.
            self.window = None;
            self.cues.push("room_fight".into());
        }
    }

    /// The guardian's chamber shuts: knight `id` stepped in at `at`. The
    /// rest of the party is drawn in beside them, as a door would bring
    /// them, and the guardian wakes.
    fn seal_guardian(&mut self, id: u32, at: (f32, f32)) {
        let Some((map, g, bounds)) = self.dungeon.open.as_ref().map(|o| {
            let g = usize::from(o.guardian);
            (o.map, g, o.chambers[g].bounds())
        }) else {
            return;
        };
        if let Some(o) = self.dungeon.open.as_mut() {
            o.chambers[g].sealed = true;
            o.chambers[g].stirred = true;
        }
        let holds = |x: f32, y: f32, inset: f32| {
            let (x0, y0, x1, y1) = bounds;
            x >= x0 + TILE_UNITS + inset
                && x < x1 - TILE_UNITS - inset
                && y >= y0 + TILE_UNITS + inset
                && y < y1 - TILE_UNITS - inset
        };
        let outside: Vec<u32> = self
            .players
            .iter()
            .filter(|(other, h)| **other != id && !holds(h.x, h.y, 0.5))
            .map(|(&other, _)| other)
            .collect();
        // Drawn in, not out: every spot within its walls, never in the
        // passage beyond the gate the first knight came through.
        let spots = self.open_spots_where(map, at, outside.len() + 1, |x, y| holds(x, y, HERO_RADIUS));
        for (k, other) in outside.into_iter().enumerate() {
            let spot = spots.get(k + 1).copied().unwrap_or(at);
            if let Some(hero) = self.players.get_mut(&other) {
                (hero.x, hero.y) = spot;
                hero.camera = None;
                hero.invulnerable = hero.invulnerable.max(20);
                hero.dash_ticks = 0;
            }
        }
        self.projectiles.retain(|p| !p.hostile || holds(p.x, p.y, -LEASH));
        for e in self.enemies.iter_mut().filter(|e| e.home == Some(g as u8) && e.hp > 0) {
            e.dormant = false;
            e.age = 0;
        }
        self.room_hurt = false;
        self.window = None;
        self.sounds.push("door_seal");
        if let Some(boss) = self.enemies.iter().filter(|e| e.home == Some(g as u8)).find_map(|e| e.boss) {
            let name = self.bosses[usize::from(boss)].id.clone();
            self.cues.push(format!("boss_rise:{name}"));
            self.sounds.push("boss_rise");
        } else if self.enemies.iter().any(|e| e.home == Some(g as u8) && e.kind == EnemyKind::Dragon) {
            self.cues.push("lair".into());
        }
    }

    /// Chamber `i`'s company is dead: what a cleared room pays, once. The
    /// guardian's opens, and on the first floor hallows the party; the
    /// dragon's opens the way down and the light home.
    fn chamber_clear(&mut self, i: usize) {
        let Some((guardian, kind, rect, bounds)) = self.dungeon.open.as_ref().map(|o| {
            let ch = &o.chambers[i];
            (o.guardian, ch.kind, ch.rect, ch.bounds())
        }) else {
            return;
        };
        if let Some(o) = self.dungeon.open.as_mut() {
            o.chambers[i].cleared = true;
            o.chambers[i].sealed = false;
        }
        let holds = |x: f32, y: f32, inset: f32| {
            let (x0, y0, x1, y1) = bounds;
            x >= x0 + TILE_UNITS + inset
                && x < x1 - TILE_UNITS - inset
                && y >= y0 + TILE_UNITS + inset
                && y < y1 - TILE_UNITS - inset
        };
        // The monsters' shots still in the air about it turn to gold.
        self.cancel_shots_where(|p| holds(p.x, p.y, -LEASH));
        self.projectiles.retain(|p| !p.hostile || !holds(p.x, p.y, -LEASH));
        self.close_ledger("clear");
        if self.dungeon.depth > 0 {
            *self.marks.entry(barony::clear_mark(self.dungeon.pack)).or_default() += 1;
        }
        let standing = self.players.values().filter(|h| h.hp > 0 && !h.stone).count();
        if standing >= 2 {
            if let Some(hero) = self.players.values_mut().find(|h| h.hp > 0 && !h.stone) {
                hero.carried.add(Spoil::Bond, 1);
            }
            self.cues.push("bond".into());
            self.notice("bonded");
        }
        if !self.room_hurt {
            self.notice("flawless");
        }
        self.cues.push(if self.room_hurt { "room_clear" } else { "flawless" }.into());
        self.sounds.push("door_open");
        self.breather();
        if usize::from(guardian) == i {
            if kind == RoomKind::Stairs && self.dungeon.depth == 1 {
                // The first floor's guardian guarded a rest: the party is
                // hallowed where it stands before the descent.
                self.stairs_held = true;
                self.hallow();
            }
            if kind == RoomKind::Lair {
                self.dragon_slain_open(rect);
            }
        }
        if let Some(o) = self.dungeon.open.as_ref() {
            let all = o.chambers.iter().all(|c| c.cleared);
            let map = o.map;
            self.dungeon.rooms[map].cleared = all;
        }
        // The floor's boss fell: a super wish each (`overclass`).
        if usize::from(guardian) == i {
            self.boss_wishes();
            // And the cauldron rises by the stairs (`cauldron`).
            self.raise_cauldron();
        }
    }

    /// The dragon slain on an open map: as in its lair, the floor reclaimed
    /// and banked, stairs in its chamber's middle and the light home above
    /// them, the hell gate beside.
    fn dragon_slain_open(&mut self, rect: (u16, u16, u16, u16)) {
        if let Some(hero) = self.players.values_mut().find(|h| h.hp > 0 && !h.stone) {
            hero.carried.add(Spoil::Scale, 1);
        }
        self.reclaimed.push(self.dungeon.pack);
        self.cues.push("victory".into());
        self.fx.push(Fx::Shake(36));
        self.sounds.push("boss_fall");
        self.bank_all(true, "the dragon is slain");
        self.triumph = Some(Triumph::Dragon);
        self.notice("dragonslayer");
        self.unwashed_check();
        let (x, y, w, h) = (usize::from(rect.0), usize::from(rect.1), usize::from(rect.2), usize::from(rect.3));
        let (c, r) = (x + w / 2 - 1, y + h / 2 - 1);
        let map = self.at;
        for (dc, dr) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            self.dungeon.rooms[map].set(c + dc, r + 2 + dr, Tile::Stairs);
        }
        let (x0, y0, x1, y1) = (
            f32::from(rect.0) * TILE_UNITS,
            f32::from(rect.1) * TILE_UNITS,
            f32::from(rect.0 + rect.2) * TILE_UNITS,
            f32::from(rect.1 + rect.3) * TILE_UNITS,
        );
        let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        self.light = Some((cx, cy - 4.0));
        self.raise_hell_gate();
        if let Some(gate) = &mut self.spectacle.gate {
            (gate.x, gate.y) = (cx + 11.0, cy + 6.0);
        }
        self.stairs_held = true;
        self.cues.push("the_deep".into());
    }

    /// The guardian of this open floor still lives: its stairs stay shut.
    pub fn open_guarded(&self) -> bool {
        self.open_here().is_some_and(|o| {
            let g = &o.chambers[usize::from(o.guardian)];
            g.guarded() && !g.cleared
        })
    }

    /// A knight standing on a door off the map takes the party through it.
    /// True if it did.
    pub(super) fn open_doors(&mut self) -> bool {
        let Some(open) = self.open_here() else {
            return false;
        };
        if self.barred() {
            return false;
        }
        let room = &self.dungeon.rooms[open.map];
        let through = self
            .players
            .iter()
            .filter(|(id, h)| h.hp > 0 && !h.stone && !self.away(**id))
            .find_map(|(_, h)| {
                let (col, row) = ((h.x / TILE_UNITS) as i32, (h.y / TILE_UNITS) as i32);
                (room.tile(col, row) == Tile::Door)
                    .then(|| open.door_on(col, row))
                    .flatten()
                    .filter(|d| layout::hall_open(self.dungeon.rooms[d.room].kind))
                    .copied()
            });
        let Some(door) = through else {
            return false;
        };
        self.enter(door.room, Some((usize::from(door.dir) + 2) % 4));
        true
    }

    /// How far into the open floor the party stands, in percent: its
    /// chamber's steps from the arrival over the farthest's.
    pub(super) fn open_progress(&self) -> Option<u32> {
        let open = self.open_here()?;
        let far = open.chambers.iter().map(|c| u32::from(c.dist)).max().unwrap_or(0);
        if far == 0 {
            return Some(100);
        }
        let deepest = self
            .players
            .values()
            .filter(|h| h.hp > 0 && !h.absent)
            .map(|h| u32::from(open.chambers[open.nearest(h.x, h.y)].dist))
            .max()
            .unwrap_or(0);
        Some(deepest * 100 / far)
    }
}

#[cfg(any(test, feature = "testing"))]
impl Run {
    /// Lay floor `depth` open for the party as it is and walk onto it
    /// (out of the stair's Sanctuary, below the first floor).
    pub fn open_floor_for_test(&mut self, depth: u32) {
        self.open_floors = true;
        let pack = Pack::at(depth, self.first);
        self.dungeon = layout::open_floor(depth, pack, self.party_size(), &mut self.rng);
        self.boss_gates = None;
        self.parked.clear();
        self.enemies.clear();
        self.at = 0;
        self.enter(0, None);
        let map = self.dungeon.open.as_ref().unwrap().map;
        if map != 0 {
            self.enter(map, None);
        }
        self.floor_plates.clear();
    }
}

impl Run {
    /// A chamber won on the first floors' open maps gives a breather
    /// (`Tune::clear_rest`, docs/FIRST-SESSION.md): every knight standing
    /// mends a share of their health, as a rest, and hears so on their own
    /// screen. The first-session bot found a lone beginner walking into the
    /// next fight on a fifth of their health, and falling there.
    pub(super) fn breather(&mut self) {
        let share = self.tune.clear_rest_at(self.dungeon.depth);
        if share == 0 {
            return;
        }
        let mut mended = false;
        for (&id, hero) in self.players.iter_mut().filter(|(_, h)| h.hp > 0 && !h.absent && !h.stone) {
            let gain = hero.mend(hero.max_hp * share / 100, stakes::Mend::Rest);
            if gain > 0 {
                mended = true;
                self.seat_notices.push((id, format!("A breather: {gain} health back.")));
            }
        }
        if mended {
            self.cues.push("breather".into());
            self.sounds.push("potion");
        }
    }
}

/// A walk on the open map from `from` toward `to`, for bots and tests: the
/// next point to head for along the floor, by a breadth-first search over
/// the tiles a knight can stand on (none if `to` cannot be reached).
pub fn open_path(room: &Room, from: (f32, f32), to: (f32, f32), barred: bool) -> Option<Vec<(i32, i32)>> {
    let walk = |t: Tile| matches!(t, Tile::Floor | Tile::Stairs) || (t == Tile::Door && !barred);
    let cell = |(x, y): (f32, f32)| ((x / TILE_UNITS) as i32, (y / TILE_UNITS) as i32);
    let (start, goal) = (cell(from), cell(to));
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    let at = |(c, r): (i32, i32)| (r * cols + c) as usize;
    if !(0..cols).contains(&goal.0) || !(0..rows).contains(&goal.1) || !walk(room.tile(goal.0, goal.1)) {
        return None;
    }
    let mut back = vec![u32::MAX; (cols * rows) as usize];
    back[at(start)] = at(start) as u32;
    let mut queue = std::collections::VecDeque::from([start]);
    while let Some((c, r)) = queue.pop_front() {
        if (c, r) == goal {
            let mut path = vec![goal];
            let mut k = at(goal);
            while k != at(start) {
                k = back[k] as usize;
                path.push(((k as i32) % cols, (k as i32) / cols));
            }
            path.reverse();
            return Some(path);
        }
        for (dc, dr) in DIRS {
            let next = (c + dc, r + dr);
            if (0..cols).contains(&next.0) && (0..rows).contains(&next.1) && back[at(next)] == u32::MAX && walk(room.tile(next.0, next.1)) {
                back[at(next)] = at((c, r)) as u32;
                queue.push_back(next);
            }
        }
    }
    None
}

#[cfg(test)]
#[path = "../../tests/rules/together_open_floors__tests.rs"]
pub(crate) mod tests;

#[cfg(test)]
#[path = "../../tests/rules/together_sealed_escapes__tests.rs"]
mod sealed_escapes;
