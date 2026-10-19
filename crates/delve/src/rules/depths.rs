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
/// How far a knight's bomb or a scroll's nova reaches on an open map.
pub const OPEN_BLAST: f32 = 24.0;
/// A knight's bomb this near the crack brings the wall down.
pub const OPEN_CRACK_REACH: f32 = 8.0;
/// The most cards lying about an open map.
pub const OPEN_ITEMS: usize = 96;

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
    pub fn open_spots(&self, index: usize, (x, y): (f32, f32), count: usize) -> Vec<(f32, f32)> {
        let grid = Grid { room: &self.dungeon.rooms[index], barred: false };
        let mut out: Vec<(f32, f32)> = Vec::with_capacity(count);
        let rings = [0.0f32, 2.4, 4.8, 7.2];
        'next: for slot in 0..count {
            for r in rings {
                for k in 0..8 {
                    let a = (k as f32 + slot as f32 * 0.37) * std::f32::consts::TAU / 8.0;
                    let (px, py) = (x + a.cos() * r, y + a.sin() * r);
                    if grid.clear(px, py, HERO_RADIUS + 0.1, Mover::Hero)
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
        let far = open.chambers.iter().map(|c| u32::from(c.dist)).max().unwrap_or(0).max(1);
        let gunners: Vec<(EnemyKind, u32)> = pack
            .roster_at(depth)
            .iter()
            .copied()
            .filter(|&(kind, _)| !matches!(kind, EnemyKind::Bat | EnemyKind::Slime | EnemyKind::Spiderling))
            .collect();
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
                let base = (if guarded { 0 } else { stakes::EXTRA_FOES[index] })
                    + stakes::EXTRA_PER_KNIGHT * knights.saturating_sub(1);
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
        let guardians: Vec<usize> = (0..self.bosses.len()).filter(|&i| self.bosses[i].only_in == pack).collect();
        let pick = (mix(self.seed ^ self.raid_id.rotate_left(17)) % guardians.len().max(1) as u64) as usize;
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
            if ch.kind == RoomKind::Stairs
                && let Some(&boss) = guardians.get(pick)
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
        let open = self.open_here()?.clone();
        let living: Vec<(f32, f32)> = self.players.values().filter(|h| h.hp > 0 && !h.stone).map(|h| (h.x, h.y)).collect();
        let (sx, sy) = living.iter().fold((0.0, 0.0), |a, &(x, y)| (a.0 + x, a.1 + y));
        let n = living.len().max(1) as f32;
        let i = open.nearest(sx / n, sy / n);
        let spots = self.chamber_spots(&open, i);
        let far = |s: &(f32, f32)| living.iter().all(|&(x, y)| (x - s.0).hypot(y - s.1) > 8.0);
        let spot = spots.iter().copied().find(far).or_else(|| spots.first().copied())?;
        Some((spot, i as u8))
    }

    /// The leash rect of chamber `home`: its stone and `LEASH` round it.
    pub(super) fn leashes(&self) -> Vec<(f32, f32, f32, f32)> {
        self.open_here().map_or_else(Vec::new, |open| {
            open.chambers
                .iter()
                .map(|ch| {
                    let (x0, y0, x1, y1) = ch.bounds();
                    (x0 - LEASH, y0 - LEASH, x1 + LEASH, y1 + LEASH)
                })
                .collect()
        })
    }

    /// One tick of the open map, after the kills are settled: the
    /// homeless take a home, the chambers are charted, companies wake and
    /// doze, the guardian's chamber seals and opens, and a chamber whose
    /// company is dead pays.
    pub(super) fn tick_open(&mut self) {
        let Some(open) = self.open_here().cloned() else {
            return;
        };
        // Whatever came onto the map mid-fight lives where it stands.
        for e in &mut self.enemies {
            if e.home.is_none() {
                e.home = Some(open.nearest(e.x, e.y) as u8);
            }
        }
        self.chart_open();
        let knights: Vec<(u32, f32, f32)> = self
            .players
            .iter()
            .filter(|(id, h)| h.hp > 0 && !h.stone && !self.away(**id))
            .map(|(&id, h)| (id, h.x, h.y))
            .collect();
        let g = usize::from(open.guardian);
        let grid = Grid { room: &self.dungeon.rooms[open.map], barred: self.barred() };
        let mut wake: Vec<usize> = Vec::new();
        for (i, ch) in open.chambers.iter().enumerate() {
            if ch.cleared || (i == g && ch.guarded()) {
                continue;
            }
            let inside = knights.iter().any(|&(_, x, y)| ch.holds(x, y, 0.0));
            let stirred = inside
                || self.enemies.iter().filter(|e| e.home == Some(i as u8) && e.dormant && e.hp > 0).any(|e| {
                    // Hurt by a knight, or a knight in sight.
                    e.wits.foe != 0
                        || knights.iter().any(|&(_, x, y)| {
                            (e.x - x).hypot(e.y - y) <= WAKE_REACH && grid.shot_stop((x, y), (e.x, e.y)).is_none()
                        })
                });
            if stirred {
                wake.push(i);
            }
        }
        for i in wake {
            self.wake_chamber(i);
        }
        // The guardian's chamber shuts behind the first knight in.
        let gc = &open.chambers[g];
        if gc.guarded()
            && !gc.cleared
            && !gc.sealed
            && let Some(&(id, x, y)) = knights.iter().find(|&&(_, x, y)| gc.holds(x, y, 1.0))
        {
            self.seal_guardian(id, (x, y));
        }
        // A company far from every knight dozes off.
        if self.tick.is_multiple_of(u64::from(HZ)) {
            for (i, ch) in open.chambers.iter().enumerate() {
                if ch.sealed || ch.cleared {
                    continue;
                }
                let (cx, cy) = ch.centre();
                let near = knights.iter().any(|&(_, x, y)| (x - cx).hypot(y - cy) <= SLEEP_REACH);
                if !near {
                    for e in self.enemies.iter_mut().filter(|e| e.home == Some(i as u8) && !e.dormant && !e.is_boss()) {
                        e.dormant = true;
                    }
                }
            }
        }
        // A chamber whose company is dead pays, once.
        for i in 0..open.chambers.len() {
            let ch = &self.dungeon.open.as_ref().unwrap().chambers[i];
            if ch.cleared || !open.populated {
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

    /// Mark the chambers a knight stands in as charted (the map's fog
    /// lifts chamber by chamber).
    pub(super) fn chart_open(&mut self) {
        let knights: Vec<(f32, f32)> = self.players.values().filter(|h| h.hp > 0 && !h.absent).map(|h| (h.x, h.y)).collect();
        if let Some(open) = self.dungeon.open.as_mut().filter(|o| o.map == self.at) {
            for ch in &mut open.chambers {
                if !ch.seen && knights.iter().any(|&(x, y)| ch.holds(x, y, -TILE_UNITS)) {
                    ch.seen = true;
                }
            }
        }
    }

    /// Chamber `i`'s company wakes (as many as `AWAKE_CAP` lets): a short
    /// telegraph, then the fight. The first time, the moment is a fight's.
    fn wake_chamber(&mut self, i: usize) {
        let mut awake = self.enemies.iter().filter(|e| e.hp > 0 && !e.dormant).count();
        let mut woke = 0;
        for e in self.enemies.iter_mut().filter(|e| e.home == Some(i as u8) && e.dormant && e.hp > 0) {
            if awake >= AWAKE_CAP {
                break;
            }
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
        let Some(open) = self.dungeon.open.clone() else {
            return;
        };
        let g = usize::from(open.guardian);
        if let Some(o) = self.dungeon.open.as_mut() {
            o.chambers[g].sealed = true;
            o.chambers[g].stirred = true;
        }
        let ch = &open.chambers[g];
        let outside: Vec<u32> = self
            .players
            .iter()
            .filter(|(other, h)| **other != id && !ch.holds(h.x, h.y, 0.5))
            .map(|(&other, _)| other)
            .collect();
        let spots = self.open_spots(open.map, at, outside.len() + 1);
        for (k, other) in outside.into_iter().enumerate() {
            let spot = spots.get(k + 1).copied().unwrap_or(at);
            if let Some(hero) = self.players.get_mut(&other) {
                (hero.x, hero.y) = spot;
                hero.camera = None;
                hero.invulnerable = hero.invulnerable.max(20);
                hero.dash_ticks = 0;
            }
        }
        self.projectiles.retain(|p| !p.hostile || ch.holds(p.x, p.y, -LEASH));
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
        let Some(open) = self.dungeon.open.clone() else {
            return;
        };
        let ch = open.chambers[i].clone();
        if let Some(o) = self.dungeon.open.as_mut() {
            o.chambers[i].cleared = true;
            o.chambers[i].sealed = false;
        }
        // The monsters' shots still in the air about it turn to gold.
        self.cancel_shots_where(|p| ch.holds(p.x, p.y, -LEASH));
        self.projectiles.retain(|p| !p.hostile || !ch.holds(p.x, p.y, -LEASH));
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
        if usize::from(open.guardian) == i {
            if ch.kind == RoomKind::Stairs && self.dungeon.depth == 1 {
                // The first floor's guardian guarded a rest: the party is
                // hallowed where it stands before the descent.
                self.stairs_held = true;
                self.hallow();
            }
            if ch.kind == RoomKind::Lair {
                self.dragon_slain_open(&ch);
            }
        }
        if let Some(o) = self.dungeon.open.as_ref() {
            let all = o.chambers.iter().all(|c| c.cleared);
            let map = o.map;
            self.dungeon.rooms[map].cleared = all;
        }
        // The floor's boss fell: a super wish each (`overclass`).
        if usize::from(open.guardian) == i {
            self.give_wishes(0, 1);
            // And the cauldron rises by the stairs (`cauldron`).
            self.raise_cauldron();
        }
    }

    /// The dragon slain on an open map: as in its lair, the floor reclaimed
    /// and banked, stairs in its chamber's middle and the light home above
    /// them, the hell gate beside.
    fn dragon_slain_open(&mut self, ch: &layout::Chamber) {
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
        let (x, y, w, h) = (usize::from(ch.rect.0), usize::from(ch.rect.1), usize::from(ch.rect.2), usize::from(ch.rect.3));
        let (c, r) = (x + w / 2 - 1, y + h / 2 - 1);
        let map = self.at;
        for (dc, dr) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            self.dungeon.rooms[map].set(c + dc, r + 2 + dr, Tile::Stairs);
        }
        let (cx, cy) = ch.centre();
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
                (room.tile(col, row) == Tile::Door).then(|| open.door_on(col, row)).flatten().copied()
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
mod tests;
