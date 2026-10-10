//! A friend's mirror of the host's delve, and smooth motion between ticks.
//!
//! The host's run is the only game. A friend's angelX holds a copy of it:
//! the whole run when the floor (or its book) changes, then each tick only
//! what moves — knights, monsters, shots, the room's loot and doors, its
//! traps — a couple of kilobytes. The friend draws that copy itself, on its
//! own screen, as often as its terminal takes frames.
//!
//! Between two ticks a view may stand anywhere: `Pose` remembers where
//! things stood on the tick before, and `between` places them part of the
//! way on, so 30 ticks a second draw as 60 smooth frames.

use super::*;

/// What one tick changed: everything a view draws, without the floor's
/// layout, the book or the guardians (those come with the whole run).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Live {
    pub(crate) raid_id: u64,
    pub(crate) depth: u32,
    pub(crate) tick: u64,
    pub(crate) phase: Phase,
    #[serde(default)]
    pub(crate) boss_gates: Option<BossGateState>,
    pub(crate) players: BTreeMap<u32, Hero>,
    pub(crate) enemies: Vec<Enemy>,
    pub(crate) projectiles: Vec<Projectile>,
    pub(crate) score: u32,
    pub(crate) at: usize,
    pub(crate) room: RoomLive,
    pub(crate) waves: Waves,
    pub(crate) traps: Vec<Trap>,
    pub(crate) rocks: Vec<Rock>,
    pub(crate) hallowed: Option<u64>,
    pub(crate) wishing: bool,
    pub(crate) shake: u32,
    pub(crate) found: Option<(u64, u32, String)>,
    #[serde(default)]
    pub(crate) sparks: Vec<Spark>,
    #[serde(default)]
    pub(crate) window: Option<overclass::Window>,
    #[serde(default)]
    pub(crate) blasts: Vec<(f32, f32, u8)>,
    #[serde(default)]
    pub(crate) boons: Vec<(u32, u64)>,
    /// The Undercroft as built and the treasury, Fortune's wheel and the
    /// mode it chose, and when this floor collapses.
    #[serde(default)]
    pub(crate) home: home::Home,
    #[serde(default)]
    pub(crate) treasury: crate::drive::together_realm::Spoils,
    #[serde(default)]
    pub(crate) mode: fortune::Mode,
    #[serde(default)]
    pub(crate) spin: Option<fortune::Spin>,
    #[serde(default)]
    pub(crate) collapse_at: Option<u64>,
    /// Ultimates playing out.
    #[serde(default)]
    pub(crate) strikes: Vec<ults::Strike>,
    #[serde(default)]
    pub(crate) spheres: Vec<ults::Sphere>,
    #[serde(default)]
    pub(crate) phantoms: Vec<ults::Phantom>,
    #[serde(default)]
    pub(crate) kegs: Vec<foes::Keg>,
    #[serde(default)]
    pub(crate) lobs: Vec<foes::Lob>,
    #[serde(default)]
    pub(crate) hooks: Vec<hunters::Hook>,
    #[serde(default)]
    pub(crate) webs: Vec<hunters::Web>,
    #[serde(default)]
    pub(crate) slams: Vec<pit::Slam>,
    #[serde(default)]
    pub(crate) ravages: Vec<tide::Ravage>,
    #[serde(default)]
    pub(crate) hireling: Option<hireling::Hireling>,
    /// A bout at the lists, while one is ridden.
    #[serde(default)]
    pub(crate) joust: Option<joust::Joust>,
    #[serde(default)]
    pub(crate) holes: Vec<hollow::Hole>,
    #[serde(default)]
    pub(crate) banner: Option<(u64, String)>,
    #[serde(default)]
    pub(crate) unboxed: Option<(u64, feats::Tier, String)>,
    #[serde(default)]
    pub(crate) stall: Vec<String>,
    #[serde(default)]
    pub(crate) descending: u32,
    #[serde(default)]
    pub(crate) dare: Option<dares::Dare>,
    #[serde(default)]
    pub(crate) cat: Option<cat::Cat>,
    #[serde(default)]
    pub(crate) captive: Option<rescues::Captive>,
    #[serde(default)]
    pub(crate) rune: Option<runes::Rune>,
    /// The floor's vault: whether its wall is down, whether Snibbet paid.
    #[serde(default)]
    pub(crate) secret: Option<layout::Secret>,
    /// Native Dragon's exit marker, including its consumed Won evidence.
    #[serde(default)]
    pub(crate) light: Option<(f32, f32)>,
    #[serde(default)]
    pub(crate) audience: u32,
    #[serde(default)]
    pub(crate) fans: u32,
    #[serde(default)]
    pub(crate) fan_boxes: Vec<audience::FanBox>,
}

/// The parts of the party's room that change while they stand in it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct RoomLive {
    pub(crate) kind: RoomKind,
    pub(crate) cleared: bool,
    pub(crate) visited: bool,
    pub(crate) items: Vec<Item>,
    pub(crate) chest: Option<layout::Chest>,
}

/// Where everything stood on one tick, for drawing part of the way on.
#[derive(Clone, Debug, Default)]
pub(crate) struct Pose {
    tick: u64,
    at: usize,
    players: Vec<(u32, f32, f32)>,
    enemies: Vec<(u32, f32, f32)>,
}

impl Run {
    /// Which whole run a mirror needs: a new raid, floor or book.
    pub(crate) fn mirror_key(&self) -> (u64, u32, usize) {
        (self.raid_id, self.dungeon.depth, self.book.revision)
    }

    /// This tick's changes, for a friend's mirror.
    pub(crate) fn live(&self) -> Live {
        let room = self.room();
        Live {
            raid_id: self.raid_id,
            depth: self.dungeon.depth,
            tick: self.tick,
            phase: self.phase,
            boss_gates: self.boss_gates.clone(),
            players: self.players.clone(),
            enemies: self.enemies.clone(),
            projectiles: self.projectiles.clone(),
            score: self.score,
            at: self.at,
            room: RoomLive {
                kind: room.kind,
                cleared: room.cleared,
                visited: room.visited,
                items: room.items.clone(),
                chest: room.chest,
            },
            waves: self.waves.clone(),
            traps: self.traps.clone(),
            rocks: self.rocks.clone(),
            hallowed: self.hallowed,
            wishing: self.wishing,
            shake: self.shake,
            found: self.found.clone(),
            sparks: self.sparks.clone(),
            window: self.window.clone(),
            blasts: self.blasts.clone(),
            boons: self.boons.clone(),
            home: self.home.clone(),
            treasury: self.treasury.clone(),
            mode: self.mode,
            spin: self.spin,
            collapse_at: self.collapse_at,
            strikes: self.strikes.clone(),
            spheres: self.spheres.clone(),
            phantoms: self.phantoms.clone(),
            kegs: self.kegs.clone(),
            lobs: self.lobs.clone(),
            hooks: self.hooks.clone(),
            webs: self.webs.clone(),
            slams: self.slams.clone(),
            ravages: self.ravages.clone(),
            hireling: self.hireling.clone(),
            joust: self.joust.clone(),
            holes: self.holes.clone(),
            banner: self.banner.clone(),
            unboxed: self.unboxed.clone(),
            stall: self.stall.clone(),
            descending: self.descending,
            dare: self.dare.clone(),
            cat: self.cat.clone(),
            captive: self.captive.clone(),
            rune: self.rune,
            secret: self.dungeon.secret,
            light: self.light,
            audience: self.audience,
            fans: self.fans,
            fan_boxes: self.fan_boxes.clone(),
        }
    }

    /// Take a tick's changes into this mirror. False when they belong to
    /// another raid or floor (the whole run must come first).
    pub(crate) fn apply_live(&mut self, live: Live) -> bool {
        // Bound mutable state/identities before cloning or touching indexed
        // rooms. Ungated projections need the same secret safety and atomicity.
        if !self.live_preflight(&live) {
            return false;
        }
        if (self.boss_gates.is_some() || live.boss_gates.is_some())
            && !self
                .boss_gates
                .as_ref()
                .zip(live.boss_gates.as_ref())
                .is_some_and(|(a, b)| a.permits_progress(b))
        {
            return false;
        }
        let mut next = self.clone();
        if !next.apply_live_unchecked(live) || !next.valid_snapshot() {
            return false;
        }
        *self = next;
        true
    }

    fn live_preflight(&self, live: &Live) -> bool {
        let rooms = self.dungeon.rooms.len();
        let gates_ok =
            |g: &Option<boss_gates::Gates>| g.as_ref().is_none_or(|g| g.within_bounds(rooms));
        let grows_tavern = live.depth == 0
            && self.dungeon.depth == 0
            && live.home.level(home::Station::Wing) >= 1
            && !self
                .dungeon
                .rooms
                .iter()
                .any(|r| r.kind == RoomKind::Tavern)
            && self
                .dungeon
                .rooms
                .iter()
                .any(|r| r.kind == RoomKind::Trophies);
        !self.dungeon.rooms.is_empty()
            && rooms <= 64
            && self.at < rooms
            && live.at < rooms + usize::from(grows_tavern && rooms < 64)
            && self.dungeon.rooms.iter().all(Room::valid_snapshot)
            && self.players.len() <= MAX_PLAYERS as usize
            && self.enemies.len() <= 128
            && self.projectiles.len() <= MAX_PROJECTILES
            && self.bosses.len() <= 64
            && gates_ok(&self.boss_gates)
            && gates_ok(&live.boss_gates)
            && live.raid_id == self.raid_id
            && live.depth == self.dungeon.depth
            && self.dungeon.permits_secret(live.secret)
            && live
                .light
                .is_none_or(|(x, y)| x.is_finite() && y.is_finite())
            && live.players.len() <= MAX_PLAYERS as usize
            && live.enemies.len() <= 128
            && live.projectiles.len() <= MAX_PROJECTILES
            && live.room.items.len() <= MAX_ITEMS
            && live.strikes.len() <= MAX_PROJECTILES
            && live.spheres.len() <= MAX_PROJECTILES
            && live.phantoms.len() <= MAX_PROJECTILES
            && live.kegs.len() <= MAX_PROJECTILES
            && live.lobs.len() <= MAX_PROJECTILES
            && live.hooks.len() <= MAX_PROJECTILES
            && live.webs.len() <= MAX_PROJECTILES
            && live.slams.len() <= MAX_PROJECTILES
            && live.ravages.len() <= MAX_PROJECTILES
            && live.holes.len() <= MAX_PROJECTILES
    }

    fn apply_live_unchecked(&mut self, live: Live) -> bool {
        // The host's realm dug the west wing: so does this mirror of it,
        // before the host walks into the tavern.
        if live.depth == 0 && self.dungeon.depth == 0 && live.home.level(home::Station::Wing) >= 1 {
            layout::dig_tavern(&mut self.dungeon);
        }
        if live.raid_id != self.raid_id
            || live.depth != self.dungeon.depth
            || live.at >= self.dungeon.rooms.len()
        {
            return false;
        }
        self.tick = live.tick;
        self.phase = live.phase;
        self.light = live.light;
        self.boss_gates = live.boss_gates;
        // A guest can miss the final death tick before host travel; recorded
        // defeat itself proves the host visited that arena, but not its clear.
        if let Some(g) = &self.boss_gates {
            for w in &g.leaders {
                if w.defeated {
                    self.dungeon.rooms[w.room].visited = true;
                }
            }
            for camp in &g.economy.camps {
                if camp.deployed {
                    self.dungeon.rooms[camp.room].visited = true;
                }
            }
            for event in &g.economy.events {
                if let super::boss_ecology::Event::Emit { room, .. } = *event {
                    self.dungeon.rooms[room].visited = true;
                }
            }
            if g.pit_defeated
                && let Some(room) = g.optional_pit
            {
                self.dungeon.rooms[room].visited = true;
            }
            if g.guardian_defeated {
                self.dungeon.rooms[g.guardian].visited = true;
            }
        }
        // Keep serialized mirror identities usable on load as well as on screen.
        // Old Live packets need no additional next-ID field: the bounded journal
        // and active bodies authoritatively establish the observed high-water mark.
        self.next_id = self
            .next_id
            .max(live.enemies.iter().map(|e| e.id).max().unwrap_or(0));
        if let Some(g) = &self.boss_gates {
            for event in &g.economy.events {
                if let super::boss_ecology::Event::Emit { id, .. } = *event {
                    self.next_id = self.next_id.max(id);
                }
            }
        }
        self.players = live.players;
        self.enemies = live.enemies;
        self.projectiles = live.projectiles;
        self.score = live.score;
        self.at = live.at;
        let room = &mut self.dungeon.rooms[live.at];
        room.kind = live.room.kind;
        room.cleared = live.room.cleared;
        room.visited = live.room.visited;
        room.items = live.room.items;
        room.chest = live.room.chest;
        self.waves = live.waves;
        self.traps = live.traps;
        self.rocks = live.rocks;
        self.hallowed = live.hallowed;
        self.wishing = live.wishing;
        self.shake = live.shake;
        self.found = live.found;
        self.sparks = live.sparks;
        self.window = live.window;
        self.blasts = live.blasts;
        self.boons = live.boons;
        self.home = live.home;
        self.treasury = live.treasury;
        self.mode = live.mode;
        self.spin = live.spin;
        self.collapse_at = live.collapse_at;
        self.strikes = live.strikes;
        self.spheres = live.spheres;
        self.phantoms = live.phantoms;
        self.kegs = live.kegs;
        self.lobs = live.lobs;
        self.hooks = live.hooks;
        self.webs = live.webs;
        self.slams = live.slams;
        self.ravages = live.ravages;
        self.hireling = live.hireling;
        self.joust = live.joust;
        self.holes = live.holes;
        self.banner = live.banner;
        self.unboxed = live.unboxed;
        self.stall = live.stall;
        self.descending = live.descending;
        self.dare = live.dare;
        self.cat = live.cat;
        self.captive = live.captive;
        self.rune = live.rune;
        // A cracked wall the host's bomb brought down comes down here too.
        if let (Some(theirs), Some(mine)) = (live.secret, self.dungeon.secret) {
            if theirs.found && !mine.found && !self.dungeon.open_secret() {
                return false;
            }
            self.dungeon.secret = Some(theirs);
        }
        self.audience = live.audience;
        self.fans = live.fans;
        self.fan_boxes = live.fan_boxes;
        true
    }

    /// Where everything stands now.
    pub(crate) fn pose(&self) -> Pose {
        Pose {
            tick: self.tick,
            at: self.at,
            players: self.players.iter().map(|(&id, h)| (id, h.x, h.y)).collect(),
            enemies: self.enemies.iter().map(|e| (e.id, e.x, e.y)).collect(),
        }
    }

    /// This run drawn `alpha` (0–1) of the way from `before` (the tick just
    /// gone) to now. Only motion is blended, and only across one tick in
    /// the same room: a door, a stair or a long gap draws as it stands.
    pub(crate) fn between(&self, before: &Pose, alpha: f32) -> Run {
        let mut view = self.clone();
        view.blend(before, alpha);
        view
    }

    /// `f` with this run drawn part of the way from `before`, then put back
    /// as it was: no copy of the whole run per frame.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn drawn_between<T>(
        &mut self,
        before: &Pose,
        alpha: f32,
        f: impl FnOnce(&Run) -> T,
    ) -> T {
        self.drawn_ahead(before, alpha, None, f)
    }

    /// As `drawn_between`, with knight `own.0` drawn where their own held
    /// keys (`own.1`) will have taken them `own.2` seconds after this tick:
    /// a friend's own knight answers their keys at once, and the host's
    /// next tick corrects it.
    pub(crate) fn drawn_ahead<T>(
        &mut self,
        before: &Pose,
        alpha: f32,
        own: Option<(u32, Input, f32)>,
        f: impl FnOnce(&Run) -> T,
    ) -> T {
        let now = self.pose();
        let shots: Vec<(f32, f32)> = self.projectiles.iter().map(|p| (p.x, p.y)).collect();
        self.blend(before, alpha);
        if let Some((id, input, ahead)) = own {
            self.predict(id, input, ahead);
        }
        let out = f(self);
        for (id, x, y) in &now.players {
            if let Some(hero) = self.players.get_mut(id) {
                (hero.x, hero.y) = (*x, *y);
            }
        }
        for (id, x, y) in &now.enemies {
            if let Some(enemy) = self.enemies.iter_mut().find(|e| e.id == *id) {
                (enemy.x, enemy.y) = (*x, *y);
            }
        }
        for (shot, (x, y)) in self.projectiles.iter_mut().zip(shots) {
            (shot.x, shot.y) = (x, y);
        }
        out
    }

    /// Walk knight `id` on from the tick's place by `input` for `seconds`
    /// (at most a few ticks), as the sim walks: their pace, sliding along
    /// stone. Side-on halls and rolls are left to the host.
    fn predict(&mut self, id: u32, input: Input, seconds: f32) {
        if self.side_on() || !input.valid() {
            return;
        }
        let grid = Grid {
            room: &self.dungeon.rooms[self.at],
            barred: !self.enemies.is_empty(),
        };
        let Some(hero) = self.players.get_mut(&id).filter(|h| h.hp > 0 && !h.stone) else {
            return;
        };
        if hero.dash_ticks > 0 {
            return;
        }
        let (mx, my) = unit(input.move_x as f32, input.move_y as f32);
        if mx == 0.0 && my == 0.0 {
            return;
        }
        let pace = HERO_SPEED * (100 + hero.bonus.speed) as f32 / 100.0;
        let pace = if input.dash && hero.guard != cards::Guard::Roll {
            pace * SHIELD_PACE
        } else {
            pace
        };
        // A whole tick is 33 ms. A browser frame is 8 ms, so the remainder
        // has to move too, or a local key never shows up before the next step.
        let seconds = seconds.clamp(0.0, 0.25);
        let steps = seconds / DT;
        let whole = steps.floor() as u32;
        let frac = steps - whole as f32;
        let mut at = (hero.x, hero.y);
        for _ in 0..whole {
            at = grid.slide(
                at,
                (mx * pace * DT, my * pace * DT),
                HERO_RADIUS,
                Mover::Hero,
            );
        }
        if frac > 0.001 {
            at = grid.slide(
                at,
                (mx * pace * DT * frac, my * pace * DT * frac),
                HERO_RADIUS,
                Mover::Hero,
            );
        }
        (hero.x, hero.y) = at;
    }

    fn blend(&mut self, before: &Pose, alpha: f32) {
        let view = self;
        if before.tick + 1 != view.tick || before.at != view.at {
            return;
        }
        let k = 1.0 - alpha.clamp(0.0, 1.0);
        let back = |now: f32, then: f32| {
            // A jump of more than a few units is a teleport (a pit, a
            // door): it is not drawn sliding.
            if (now - then).abs() > 4.0 {
                now
            } else {
                now - (now - then) * k
            }
        };
        for (id, x, y) in &before.players {
            if let Some(hero) = view.players.get_mut(id) {
                (hero.x, hero.y) = (back(hero.x, *x), back(hero.y, *y));
            }
        }
        for (id, x, y) in &before.enemies {
            if let Some(enemy) = view.enemies.iter_mut().find(|e| e.id == *id) {
                (enemy.x, enemy.y) = (back(enemy.x, *x), back(enemy.y, *y));
            }
        }
        for shot in &mut view.projectiles {
            shot.x -= shot.vx * DT * k;
            shot.y -= shot.vy * DT * k;
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_mirror__tests.rs"]
mod tests;
