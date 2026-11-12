//! Delve adapter for the pure kingdom economy. One host simulation owns the
//! state; Home settlement persists it by revision, just like the salvage yard.
use super::*;
use crate::kingdom::{self as economy, Command, Tile, catalog};

pub const SHIFT_TICKS: u64 = HZ as u64 * 10;
/// One room (or chamber) in this many holds a seam: the second, the fifth,
/// the eighth. Tuned so a delve reads as the realm's mine (docs/KINGDOM-WORKS.md).
pub const SEAM_EVERY: usize = 3;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Board {
    pub open: bool,
    pub tile: Tile,
    pub plan: usize,
    pub message: String,
    pub held: u32,
    pub gallery: bool,
    pub moved: u64,
    pub page: u8,
    pub selection: usize,
    pub zoom: bool,
    pub plan_moved: u64,
    pub camera: Option<Tile>,
    pub pending_release: Option<String>,
    /// On the hillside, the builder out in the world (`hillside`): the way the
    /// knight faces (0 north, 1 east, 2 south, 3 west), the footprint's corner,
    /// a pattern's quarter turns, the line being drawn, the flow overlay, whether
    /// the cursor was last set by the mouse, and the tile last painted.
    pub face: u8,
    pub anchor: Tile,
    pub turn: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft: Option<super::hillside::Draft>,
    pub overlay: bool,
    pub pointed: bool,
    pub painted: Tile,
    /// The hotbar, the steps to take back, the card and the numbers this seat is shown
    /// (`builder_rig`).
    #[serde(skip_serializing_if = "super::builder_rig::Rig::is_idle")]
    pub rig: super::builder_rig::Rig,
    /// A route in the making: the bay the wagons load at, waiting for the bay they unload at.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<String>,
}
impl Default for Board {
    fn default() -> Self {
        Self {
            open: false,
            tile: (8, 8),
            plan: 0,
            message: "Build a forester beside the road".into(),
            held: 0,
            gallery: false,
            moved: 0,
            page: 0,
            selection: 0,
            zoom: true,
            plan_moved: 0,
            camera: None,
            pending_release: None,
            face: 2,
            anchor: (0, 0),
            turn: 0,
            draft: None,
            overlay: false,
            pointed: false,
            painted: (-1, -1),
            rig: super::builder_rig::Rig::default(),
            stop: None,
        }
    }
}
impl Board {
    pub fn valid(&self) -> bool {
        self.tile.0 >= 0
            && self.tile.0 < crate::kingdom::REALM_WIDTH
            && self.tile.1 >= 0
            && self.tile.1 < crate::kingdom::REALM_HEIGHT
            && self.plan < 1024
            && self.face < 4
            && self.turn < 4
            && self.draft.as_ref().is_none_or(|d| d.path.len() <= super::hillside::MAX_PATH)
            && self.message.len() <= 256
            && self.held < (1 << 20)
            && self.rig.valid()
            && self.page < 9
            && self.selection < 512
            && self
                .pending_release
                .as_ref()
                .is_none_or(|key| !key.is_empty() && key.len() <= 96)
            && self.camera.is_none_or(|(x, y)| {
                (0..crate::kingdom::REALM_WIDTH).contains(&x) && (0..crate::kingdom::REALM_HEIGHT).contains(&y)
            })
    }
}

#[derive(Clone, Debug)]
pub struct Deposit {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub clear: bool,
    /// The seam's quarry stone: the resource its claim is of.
    pub resource: &'static str,
    pub name: String,
    pub seam: geology::Seam,
}
/// What the field guide says of each part of the Delve's rock: one row a
/// pack, from `geology`, in the order the desk lists them.
pub fn guide() -> &'static [geology::Row] {
    &geology::ROWS
}
impl Run {
    pub(super) fn announce_ore_forecast(&mut self) {
        if self.dungeon.depth == 0 {
            return;
        }
        let seam = geology::seam_at(self.dungeon.pack, self.dungeon.depth);
        let next_depth = self.dungeon.depth.saturating_add(1);
        let next = geology::seam_at(Pack::at(next_depth, self.first), next_depth);
        let kind = seam.guard();
        let after = if next.stone() != seam.stone() {
            let also = next.also().trim_start_matches("Also ").trim_end_matches('.').to_string();
            format!(
                " Next floor: {}{}.",
                geology::rock_name(next.stone()),
                if also.is_empty() { String::new() } else { format!(", with {also}") }
            )
        } else {
            String::new()
        };
        self.notices.push(format!(
            "Prospector's warning - {}: {} {}{}",
            seam.name(),
            kind.warning(),
            kind.counter(),
            after
        ));
    }
    /// Enter the player-built surface with the whole party, as other surface
    /// doors do. The gate remains a route back to the existing campaign.
    pub fn enter_frontier(&mut self) {
        if self.dungeon.depth != 0 || self.room().kind != RoomKind::Kingdom {
            return;
        }
        self.found_industry();
        self.kingdom.frontier = true;
        self.kingdom.return_to = None;
        self.refresh_frontier();
        for (i, h) in self.players.values_mut().enumerate() {
            h.x = (f32::from(economy::GATE.0) + 0.5) * TILE_UNITS + i as f32 * 0.05;
            h.y = (f32::from(economy::GATE.1) + 0.5) * TILE_UNITS;
            h.camera = None;
        }
        for b in self.kingdom.industry.values_mut() {
            b.open = false;
        }
        self.enemies.clear();
        self.projectiles.clear();
        self.phase = Phase::Exploring;
    }
    pub(super) fn refresh_frontier(&mut self) {
        let Some(i) = world::room_of(&self.dungeon, RoomKind::Kingdom) else {
            return;
        };
        let tiles = (0..economy::HEIGHT)
            .flat_map(|y| (0..economy::WIDTH).map(move |x| (x, y)))
            .map(|tile| {
                if self.home.industry.walkable(catalog(), tile) {
                    layout::Tile::Floor
                } else {
                    layout::Tile::Wall
                }
            })
            .collect();
        self.dungeon.rooms[i] =
            layout::Room::kingdom(tiles, economy::WIDTH as usize, economy::HEIGHT as usize);
        // Construction cannot imprison a knight who was on the plot. Relocate
        // to the closest open tile rather than allowing a collision overlap.
        if self.at == i {
            for h in self.players.values_mut() {
                let tile = ((h.x / TILE_UNITS) as i16, (h.y / TILE_UNITS) as i16);
                if !self.home.industry.walkable(catalog(), tile) {
                    if let Some((x, y)) = (0..economy::HEIGHT)
                        .flat_map(|y| (0..economy::WIDTH).map(move |x| (x, y)))
                        .filter(|&t| self.home.industry.walkable(catalog(), t))
                        .min_by_key(|&(x, y)| (x - tile.0).pow(2) + (y - tile.1).pow(2))
                    {
                        h.x = (f32::from(x) + 0.5) * TILE_UNITS;
                        h.y = (f32::from(y) + 0.5) * TILE_UNITS;
                    }
                }
            }
        }
    }
    /// The frontier is its own surface adapter. Old authored-place triggers do
    /// not run against new coordinates. It still uses the same hero, collision,
    /// input, fixed tick, Home state and network snapshot as the campaign.
    pub(super) fn step_frontier(&mut self, inputs: &BTreeMap<u32, Input>) {
        if self.room().cols != economy::WIDTH as usize
            || self.room().rows != economy::HEIGHT as usize
        {
            self.refresh_frontier();
        }
        let before = self.tick;
        self.tick += 1;
        let gate = (
            (f32::from(economy::GATE.0) + 0.5) * TILE_UNITS,
            (f32::from(economy::GATE.1) + 0.5) * TILE_UNITS,
        );
        let room = &self.dungeon.rooms[self.at];
        let grid = Grid {
            room,
            barred: false,
        };
        let mut leave = false;
        for (&id, h) in &mut self.players {
            let Some(input) = inputs.get(&id).filter(|i| i.valid()) else {
                continue;
            };
            if h.hp == 0 || h.absent || h.stone {
                continue;
            }
            let scale = if input.move_x != 0 && input.move_y != 0 {
                0.70710677
            } else {
                1.0
            };
            let speed = 0.16 * scale;
            let x = (h.x + f32::from(input.move_x) * speed)
                .clamp(HERO_RADIUS, room.width() - HERO_RADIUS);
            let y = (h.y + f32::from(input.move_y) * speed)
                .clamp(HERO_RADIUS, room.height() - HERO_RADIUS);
            if grid.clear(x, h.y, HERO_RADIUS, Mover::Hero) {
                h.x = x;
            }
            if grid.clear(h.x, y, HERO_RADIUS, Mover::Hero) {
                h.y = y;
            }
            if input.interact && (h.x - gate.0).hypot(h.y - gate.1) < 3.0 {
                leave = true;
            }
        }
        self.tick_industry(inputs, before);
        if leave {
            self.kingdom.frontier = false;
            self.raise_kingdom();
            self.walk_kingdom(None);
            for b in self.kingdom.industry.values_mut() {
                b.open = false;
            }
        }
    }
    /// Deposits occupy real combat rooms/chambers, never the entrance or a
    /// peaceful shop. Stable identities survive revisits and checkpoint restore.
    pub fn industry_deposits(&self) -> Vec<Deposit> {
        if self.dungeon.depth == 0 || self.dungeon.depth > DEEPEST || self.over() {
            return Vec::new();
        }
        let seam = geology::seam_at(self.dungeon.pack, self.dungeon.depth);
        let (resource, name) = (seam.stone(), seam.name());
        let make = |i: usize, x: f32, y: f32, clear: bool| {
            let legacy = format!(
                "{:x}-{}-{}-{}",
                self.seed, self.raid_id, self.dungeon.depth, i
            );
            // A live Run may revisit the same floor index through another
            // region. Resource identity must not alias that earlier claim.
            // A claim from before geology is of the ore the seam still gives.
            let id = if self.home.industry.claims.get(&legacy).is_some_and(|q| {
                q.resource == resource || (!geology::is_stone(&q.resource) && seam.gives(&q.resource))
            }) {
                legacy
            } else {
                format!("{legacy}-{resource}")
            };
            Deposit {
                id,
                x,
                y,
                clear,
                resource,
                name: name.clone(),
                seam,
            }
        };
        if self.open_map() {
            let Some(o) = &self.dungeon.open else {
                return Vec::new();
            };
            o.chambers
                .iter()
                .enumerate()
                .filter(|(i, ch)| {
                    *i > 0 && (i % SEAM_EVERY == 1 || *i == usize::from(o.guardian)) && ch.stirred
                })
                .map(|(i, ch)| {
                    let (x0, y0, x1, y1) = ch.bounds();
                    make(i, (x0 + x1) / 2.0, (y0 + y1) / 2.0, ch.cleared)
                })
                .collect()
        } else if self.at > 0
            && self.at % SEAM_EVERY == 1
            && matches!(
                self.room().kind,
                RoomKind::Fight | RoomKind::Stairs | RoomKind::Lair
            )
        {
            vec![make(
                self.at,
                self.room().width() / 2.0,
                self.room().height() / 2.0,
                self.room().cleared,
            )]
        } else {
            Vec::new()
        }
    }

    /// Consume builder keys per seat while other knights keep playing. Reuses
    /// the existing input wire, so native, browser and couch seats agree.
    pub(super) fn industry_inputs(
        &mut self,
        inputs: &BTreeMap<u32, Input>,
    ) -> BTreeMap<u32, Input> {
        let mut out = inputs.clone();
        for (&id, board) in &mut self.kingdom.industry {
            if !inputs.contains_key(&id) {
                board.held = 0;
            }
        }
        let surface = self.dungeon.depth == 0 && self.room().kind == RoomKind::Kingdom;
        // On the hillside the builder is out in the world (`hillside`), not on a desk.
        let mut enter = false;
        let mut gear_actions = Vec::new();
        let mut claim_releases = Vec::new();
        let previous_revision = self.home.industry.rev;
        for (&id, input) in inputs {
            if !self.kingdom.industry.contains_key(&id) && (!surface || input.cast != 4) {
                continue;
            }
            if !input.valid()
                || !self
                    .players
                    .get(&id)
                    .is_some_and(|h| h.hp > 0 && !h.stone && !h.absent)
            {
                continue;
            }
            // (Bits 0 to 10 are the desk's keys; the builder in the world reads the rest:
            // Z, G, R and H, and the five skill keys M , . ; ' that the world has no use for.)
            let bits = u32::from(input.cast == 4)
                | (u32::from(input.interact) << 1)
                | (u32::from(input.swing) << 2)
                | (u32::from(input.bomb) << 3)
                | (u32::from(input.fire) << 4)
                | (u32::from(input.cast == 1) << 5)
                | (u32::from(input.cast == 2) << 6)
                | (u32::from(input.cast == 3) << 7)
                | (u32::from(input.cycle) << 8)
                | (u32::from(input.ride) << 9)
                | (u32::from(input.dash) << 10)
                | (u32::from(input.play != 0) << 11)
                | (u32::from(input.vigil) << 12)
                | (u32::from(input.ult) << 13)
                | (u32::from(input.team) << 14)
                | (u32::from(input.skill == 1) << 15)
                | (u32::from(input.skill == 2) << 16)
                | (u32::from(input.skill == 3) << 17)
                | (u32::from(input.skill == 4) << 18)
                | (u32::from(input.skill == 5) << 19);
            let board = self.kingdom.industry.entry(id).or_default();
            let mut fresh = bits & !board.held;
            let released = board.held & !bits;
            board.held = bits;
            if !surface {
                board.open = false;
                continue;
            }
            if fresh & 1 != 0 {
                board.open = !board.open;
                Self::found_in(&mut self.home.industry, self.seed);
                if board.open && !self.home.industry.is_empty() && self.home.industry.world.is_hillside() {
                    // The builder comes out where the knight stands.
                    board.page = 0;
                    board.gallery = false;
                    board.pointed = false;
                    board.draft = None;
                    board.message = "WALK TO A SPOT AND PRESS E TO LAY IT".into();
                }
            }
            if !board.open && fresh & 1 == 0 {
                continue;
            }
            // (Founded just now, perhaps: asked after the toggle.)
            let hill = !self.home.industry.is_empty() && self.home.industry.world.is_hillside();
            if hill && board.open && board.page == 0 && !board.gallery {
                // The builder in the world: the knight keeps walking, the rest of
                // their keys are the builder's.
                let mut walk = super::hillside::walking_input(&input);
                if fresh & 1 != 0 {
                    // (The key that brought the builder out is not a tool's.)
                    fresh = 0;
                }
                let hero = self.players.get(&id).map(|h| (h.x, h.y));
                super::hillside::aim_cursor(board, hero, &input);
                let mut released = released;
                if let Some(action) = input.builder {
                    let (press, release) = Run::hill_pointer(board, action);
                    if press {
                        fresh |= 2;
                    }
                    if release {
                        released |= 2;
                    }
                }
                // Walking brings the view back from where it was sent to look.
                if input.move_x != 0 || input.move_y != 0 {
                    board.rig.look = None;
                }
                // The mouse's button held is E held.
                let bits = if board.rig.drag.is_some() { bits | 2 } else { bits };
                super::hillside::world_keys(board, &mut self.home.industry, self.tick, &input, fresh, released, bits, id);
                walk.dash = false;
                out.insert(id, walk);
                continue;
            }
            out.insert(id, Input::default());
            if !board.open {
                continue;
            }
            // Pointer commands are discrete actions on the same authoritative
            // desk as the keyboard. They never borrow the key-repeat latch.
            if let Some(action) = input.builder {
                use super::builder_input::BuilderAction;
                match action {
                    BuilderAction::Page(page) => {
                        board.page = page;
                        board.gallery = page == 5;
                        board.selection = 0;
                    }
                    BuilderAction::Plan(plan) if board.page == 0 => {
                        board.plan = usize::from(plan);
                    }
                    BuilderAction::NextPlan { back } if board.page == 0 => {
                        let count = catalog().menu.len() + 1;
                        board.plan = (board.plan + if back { count - 1 } else { 1 }) % count;
                    }
                    BuilderAction::Selection(selection) if board.page != 0 => {
                        board.selection = usize::from(selection);
                    }
                    BuilderAction::Confirm => fresh |= 2,
                    BuilderAction::Plot { tile, build } if board.page == 0 => {
                        board.camera.get_or_insert(board.tile);
                        board.tile = (i16::from(tile.0), i16::from(tile.1));
                        let occupied = self.home.industry.at(board.tile);
                        let road_on_gate = board.plan == 0
                            && occupied.is_some_and(|id| {
                                catalog().plans[&self.home.industry.buildings[&id].plan].passable
                            });
                        if build && (occupied.is_none() || road_on_gate) {
                            fresh |= 2;
                        }
                    }
                    _ => {}
                }
            }
            if fresh & 1024 != 0 {
                board.zoom = !board.zoom;
            }
            if fresh & 512 != 0 {
                if hill {
                    // The ledger closes over the world; the builder is out again.
                    board.page = 0;
                    board.selection = 0;
                } else {
                    enter = true;
                }
                continue;
            }
            if fresh & 256 != 0 {
                if hill {
                    // The ledger over the world has seven pages: the four of the desk's, the
                    // goals, what the party makes and uses, and what chokes the works.
                    use super::hillside::{PAGE_CHOKES, PAGE_GOALS, PAGE_PRODUCTION};
                    board.page = match board.page {
                        1 => 3,
                        3 => 6,
                        6 => 8,
                        8 => PAGE_GOALS,
                        PAGE_GOALS => PAGE_PRODUCTION,
                        PAGE_PRODUCTION => PAGE_CHOKES,
                        _ => 1,
                    };
                } else {
                    board.page = (board.page + 1) % 9;
                }
                board.gallery = board.page == 5;
                board.selection = 0;
            }
            if hill && matches!(board.page, super::hillside::PAGE_GOALS | super::hillside::PAGE_PRODUCTION | super::hillside::PAGE_CHOKES) {
                super::hillside::ledger_screen(board, &self.home.industry, self.tick, &input, fresh);
                continue;
            }
            if board.page != 3 {
                board.pending_release = None;
            }
            if fresh & 128 != 0 && (board.page != 0 || board.gallery) {
                let command = if board.page == 8 {
                    Command::FoodReserve((self.home.industry.food_reserve.unwrap_or(2) + 1) % 4)
                } else {
                    Command::Provision
                };
                board.message = self
                    .home
                    .industry
                    .apply(catalog(), command)
                    .unwrap_or_else(|e| e);
            }
            if board.page != 0 || board.gallery {
                let count = match board.page {
                    1 if hill => catalog().hill_charters().len(),
                    1 => catalog().technologies.len(),
                    2 => kingdom_gear::ITEMS.len(),
                    3 => self.home.industry.claims.len(),
                    4 => geology::ROWS.len(),
                    5 => 12,
                    6 => self.home.industry.population.citizens.len(),
                    7 => 9,
                    _ => self.home.industry.buildings.len(),
                }
                .max(1);
                board.selection = board.selection.min(count - 1);
                if self.tick.saturating_sub(board.moved) >= 5 {
                    let delta = i32::from(input.move_y) + i32::from(input.move_x);
                    if delta != 0 {
                        board.selection =
                            (board.selection as i32 + delta).rem_euclid(count as i32) as usize;
                        board.moved = self.tick;
                    }
                }
                if fresh & 4 != 0 {
                    board.selection = (board.selection + 1) % count;
                }
                if board.page == 3 {
                    if let Some(key) = self
                        .home
                        .industry
                        .claims
                        .keys()
                        .nth(board.selection)
                        .cloned()
                    {
                        if board
                            .pending_release
                            .as_ref()
                            .is_some_and(|pending| pending != &key)
                        {
                            board.pending_release = None;
                        }
                        let linked = self
                            .home
                            .industry
                            .buildings
                            .iter()
                            .find(|(_, b)| b.claim.as_ref() == Some(&key))
                            .map(|(&id, _)| id);
                        if fresh & 64 != 0 {
                            board.pending_release = None;
                            board.message = if let Some(building) = linked {
                                self.home
                                    .industry
                                    .apply(catalog(), Command::Unlink(building))
                                    .unwrap_or_else(|e| e)
                            } else {
                                "This claim is not linked to an outpost".into()
                            };
                        }
                        if fresh & 8 != 0 {
                            if linked.is_some() {
                                board.message =
                                    "Unlink the outpost with [2] before releasing this claim"
                                        .into();
                            } else if board.pending_release.as_ref() == Some(&key) {
                                board.pending_release = None;
                                claim_releases.push((id, key));
                            } else {
                                board.pending_release = Some(key);
                                board.message = "Press [X] again to release this seam. Delivered ore and charter progress stay".into();
                            }
                        }
                    } else {
                        board.pending_release = None;
                    }
                }
                if fresh & 2 != 0 {
                    if board.page == 1 {
                        let key = if hill { catalog().hill_charters().get(board.selection).map(|(k, _)| *k) } else { catalog().technologies.keys().nth(board.selection) };
                        if let Some(key) = key {
                            board.message = self
                                .home
                                .industry
                                .apply(catalog(), Command::Research(key.clone()))
                                .unwrap_or_else(|e| e);
                        }
                    } else if board.page == 2 {
                        gear_actions.push((
                            id,
                            kingdom_gear::ITEMS[board.selection % kingdom_gear::ITEMS.len()].id,
                        ));
                    } else if board.page == 7 {
                        let parcel = ((board.selection % 3) as i16, (board.selection / 3) as i16);
                        board.message = self
                            .home
                            .industry
                            .apply(catalog(), Command::Survey(parcel))
                            .unwrap_or_else(|e| e);
                    } else if board.page == 8 {
                        if let Some((&building, b)) =
                            self.home.industry.buildings.iter().nth(board.selection)
                        {
                            let priority = (b.priority + 1) % 3;
                            board.message = self
                                .home
                                .industry
                                .apply(catalog(), Command::Priority { building, priority })
                                .unwrap_or_else(|e| e);
                        }
                    }
                }
                if board.page == 8 {
                    if let Some((&building, b)) =
                        self.home.industry.buildings.iter().nth(board.selection)
                    {
                        let cmd = if fresh & 16 != 0 {
                            Some(Command::Pause(building))
                        } else if fresh & 32 != 0 {
                            Some(Command::EnablePower(building))
                        } else if fresh & 64 != 0 {
                            let amount = match b.production_limit {
                                0 => 24,
                                24 => 60,
                                60 => 100,
                                _ => 0,
                            };
                            Some(Command::ProductionLimit { building, amount })
                        } else {
                            None
                        };
                        if let Some(cmd) = cmd {
                            board.message = self
                                .home
                                .industry
                                .apply(catalog(), cmd)
                                .unwrap_or_else(|e| e);
                        }
                    }
                }
                if board.page == 7 && fresh & 64 != 0 {
                    let parcel = ((board.selection % 3) as i16, (board.selection / 3) as i16);
                    let goal = if self.home.industry.survey_goal == Some(parcel) {
                        None
                    } else {
                        Some(parcel)
                    };
                    board.message = self
                        .home
                        .industry
                        .apply(catalog(), Command::SurveyGoal(goal))
                        .unwrap_or_else(|e| e);
                }
                continue;
            }
            let previous_tile = board.tile;
            if (input.move_x != 0 || input.move_y != 0)
                && self.tick.saturating_sub(board.moved) >= 4
            {
                board.camera = None;
                board.tile.0 =
                    (board.tile.0 + i16::from(input.move_x)).clamp(0, economy::WIDTH - 1);
                board.tile.1 =
                    (board.tile.1 + i16::from(input.move_y)).clamp(0, economy::HEIGHT - 1);
                board.moved = self.tick;
            }
            if fresh & 4 != 0 {
                board.plan = (board.plan + 1) % (catalog().menu.len() + 1);
            }
            if (input.aim_x != 0 || input.aim_y != 0)
                && self.tick.saturating_sub(board.plan_moved) >= 6
            {
                let delta = i32::from(input.aim_x) + i32::from(input.aim_y) * 5;
                board.plan = (board.plan as i32 + delta)
                    .rem_euclid((catalog().menu.len() + 1) as i32)
                    as usize;
                board.plan_moved = self.tick;
            }
            let building = self.home.industry.at(board.tile);
            let selected_plan = board
                .plan
                .checked_sub(1)
                .and_then(|i| catalog().menu.get(i))
                .map(|id| &catalog().plans[id]);
            let connector = selected_plan.and_then(|p| p.connector);
            let brush = board.plan == 0
                || connector.is_some()
                || board
                    .plan
                    .checked_sub(1)
                    .and_then(|i| catalog().menu.get(i))
                    .is_some_and(|p| matches!(p.as_str(), "wall" | "brick-wall"));
            let paint = brush
                && input.interact
                && board.tile != previous_tile
                && self.home.industry.at(board.tile).is_none()
                && if let Some(kind) = connector {
                    !self.home.industry.power.contains(kind, board.tile)
                } else {
                    !self.home.industry.roads.contains(&board.tile)
                };
            let command = if fresh & 2 != 0 || paint {
                Some(if board.plan == 0 {
                    if self.home.industry.world.terrain(board.tile) == economy::Terrain::River {
                        Command::Bridge(board.tile)
                    } else {
                        Command::Road(board.tile)
                    }
                } else {
                    Command::Build {
                        plan: catalog().menu[board.plan - 1].clone(),
                        tile: board.tile,
                    }
                })
            } else if fresh & 8 != 0 {
                Some(if let Some(kind) = connector {
                    Command::RemovePower {
                        kind,
                        tile: board.tile,
                    }
                } else {
                    Command::Demolish(board.tile)
                })
            } else if fresh & 16 != 0 {
                building.map(Command::Pause)
            } else if fresh & 32 != 0 {
                building.and_then(|id| {
                    let b = &self.home.industry.buildings[&id];
                    let recipes = &catalog().plans[&b.plan].recipes;
                    if recipes.is_empty() {
                        None
                    } else {
                        let n = b
                            .recipe
                            .as_ref()
                            .and_then(|r| recipes.iter().position(|v| v == r))
                            .map_or(0, |i| (i + 1) % recipes.len());
                        Some(Command::Recipe {
                            building: id,
                            recipe: recipes[n].clone(),
                        })
                    }
                })
            } else if fresh & 64 != 0 {
                building.and_then(|id| {
                    let current = self.home.industry.buildings[&id].claim.as_ref();
                    let claims: Vec<_> =
                        self.home
                            .industry
                            .claims
                            .iter()
                            .filter(|(key, q)| {
                                q.secured
                                    && !self.home.industry.buildings.iter().any(|(&other, b)| {
                                        other != id && b.claim.as_ref() == Some(key)
                                    })
                            })
                            .map(|(key, _)| key.clone())
                            .collect();
                    if claims.is_empty() {
                        board.message = "No free claim. Clear a delve vein first".into();
                        None
                    } else {
                        let n = current
                            .and_then(|q| claims.iter().position(|v| v == q))
                            .map_or(0, |i| (i + 1) % claims.len());
                        Some(Command::Link {
                            building: id,
                            claim: claims[n].clone(),
                        })
                    }
                })
            } else if fresh & 128 != 0 {
                Some(Command::Provision)
            } else {
                None
            };
            if let Some(command) = command {
                board.message = self
                    .home
                    .industry
                    .apply(catalog(), command)
                    .unwrap_or_else(|e| e);
            }
        }
        for (id, gear) in gear_actions {
            let message = self.kingdom_gear_action(id, gear).unwrap_or_else(|e| e);
            if let Some(board) = self.kingdom.industry.get_mut(&id) {
                board.message = message;
            }
        }
        for (id, key) in claim_releases {
            let message = self.release_frontier_claim(&key).unwrap_or_else(|e| e);
            if let Some(board) = self.kingdom.industry.get_mut(&id) {
                board.message = message;
            }
        }
        if enter {
            self.enter_frontier();
        } else if self.kingdom.frontier && self.home.industry.rev != previous_revision {
            self.refresh_frontier();
        }
        out
    }
    pub(super) fn tick_industry(&mut self, inputs: &BTreeMap<u32, Input>, previous_tick: u64) {
        // One carrier step per simulation second; machine jobs complete on
        // ten-second shifts. Pauses never manufacture wall-clock catch-up.
        if self.tick / u64::from(HZ) > previous_tick / u64::from(HZ) && self.active() {
            self.home.industry.advance_second(catalog());
            self.deliver_coin();
            self.seam_second();
        }
        if self.tick > previous_tick && self.active() {
            self.tick_ore_pressure();
            self.tick_stir();
        }
        self.tick_carry(inputs);
        self.reset_claim_capacity_notice();
        let deposits = self.industry_deposits();
        for d in deposits {
            if self
                .players
                .values()
                .any(|h| !h.absent && h.hp > 0 && (h.x - d.x).hypot(h.y - d.y) < 12.0)
            {
                self.found_industry();
                self.discover_frontier_claim(&d);
            }
            self.tick_stir_claim(&d);
            self.tick_ore_sentries(&d);
            self.tick_quarry_face(&d);
            self.tick_claim_guardian(&d, inputs);
            if d.clear
                && self.claim_guardian_defeated(&d.id)
                && self.players.iter().any(|(id, h)| {
                    h.hp > 0
                        && !h.stone
                        && !h.absent
                        && (h.x - d.x).hypot(h.y - d.y) < 3.5
                        && inputs.get(id).is_some_and(|i| i.valid() && i.interact)
                })
                && self.home.industry.secure(&d.id)
            {
                self.notices.push(format!(
                    "Claim secured: {}. {} {} Swing at the face to cut a block, or connect a mine in the kingdom [4]",
                    d.name,
                    d.seam.uses(),
                    d.seam.also()
                ));
                self.sounds.push("door_open");
                self.announce_frontier_objective();
            }
        }
    }
    /// Returning from home generates a fresh dungeon. Keep secured resource
    /// nodes, retire abandoned sightings, and give the new expedition its own
    /// finite encounter budget. Mid-floor checkpoints never call this.
    pub(super) fn prepare_industry_expedition(&mut self) {
        let before = self.home.industry.claims.len();
        self.home.industry.claims.retain(|_, q| q.secured);
        if before != self.home.industry.claims.len() {
            self.home.industry.rev += 1;
        }
        self.kingdom.claim_bosses.clear();
        self.kingdom.ore_sentries.clear();
        self.kingdom.stir.new_expedition();
    }
    pub(super) fn industry_outfit(&mut self) {
        for hero in self.players.values_mut().filter(|h| !h.absent) {
            let (arms, rations, ward) = self.home.industry.outfit();
            if arms {
                hero.armor = hero.armor.saturating_add(2);
            }
            if rations {
                hero.give_bombs(1);
            }
            if ward {
                hero.invulnerable = hero.invulnerable.max(HZ * 10);
            }
        }
    }
}

/// A floor of `pack` at `depth` as a proof shows it (the first-stone reel, the
/// quarry's shots): a seam in a cleared, open room, its sentries and guardian
/// fallen and its charter won, a mine standing on it, and knight 1 `from`
/// units left of it and invulnerable, since the proof is of the stone and not
/// of the fights. Not a way for a party to arrive: the room is stood in.
#[cfg(any(test, feature = "testing"))]
pub fn proof_face(pack: Pack, depth: u32, from: f32) -> Run {
    let mut run = Run::new(511, 7, None);
    run.first = if pack == Pack::Crypt { Pack::Crypt } else { Pack::Cavern };
    run.dungeon = layout::floor(depth, pack, &mut Rng::new(10));
    run.boss_gates = None;
    let room = (1..run.dungeon.rooms.len())
        .find(|&i| i % SEAM_EVERY == 1 && run.dungeon.rooms[i].kind == RoomKind::Fight)
        .expect("a fighting room holds a seam");
    run.enter(room, None);
    run.enemies.clear();
    run.waves = Default::default();
    run.dungeon.rooms[room].cleared = true;
    run.phase = Phase::Exploring;
    run.calm_for_test();
    // A review arena with no incidental obstacles.
    for y in 1..run.room().rows - 1 {
        for x in 1..run.room().cols - 1 {
            if matches!(run.room().tile(x as i32, y as i32), layout::Tile::Block | layout::Tile::Hazard | layout::Tile::Bog) {
                run.dungeon.rooms[room].set(x, y, layout::Tile::Floor);
            }
        }
    }
    let d = run.industry_deposits().into_iter().next().expect("the room's seam");
    run.step(&BTreeMap::new());
    for e in &mut run.enemies {
        e.hp = 0;
    }
    run.settle_kills();
    run.enemies.clear();
    run.discover_frontier_claim(&d);
    run.home.industry.secure(&d.id);
    let k = &mut run.home.industry;
    k.stock.insert("timber".into(), 300);
    k.stock.insert("stone".into(), 300);
    k.apply(catalog(), Command::Build { plan: "mine".into(), tile: (15, 8) }).expect("a mine goes up");
    for _ in 0..80 {
        k.work(catalog());
        if k.buildings.values().all(|b| b.construction == 0) {
            break;
        }
    }
    for b in k.buildings.values_mut() {
        b.construction = 0;
    }
    if let Some(mine) = k.buildings.iter().find(|(_, b)| b.plan == "mine").map(|(i, _)| *i) {
        k.apply(catalog(), Command::Link { building: mine, claim: d.id.clone() }).expect("the mine works the face");
    }
    let h = run.players.get_mut(&1).expect("knight 1");
    (h.x, h.y) = (d.x - from, d.y + 1.0);
    h.aim_x = 1.0;
    h.aim_y = 0.0;
    h.invulnerable = HZ * 600;
    h.camera = Some((h.x, h.y - 1.0));
    run
}

#[cfg(test)]
#[path = "../../tests/rules/industry__tests.rs"]
mod tests;

#[cfg(all(test, feature = "renderer"))]
#[path = "../../tests/render/kingdom_frontier__shots.rs"]
mod shots;
#[cfg(all(test, feature = "renderer"))]
#[path = "../../tests/render/village__shots.rs"]
mod village_shots;
#[cfg(all(test, feature = "renderer"))]
#[path = "../../tests/render/quarry__shots.rs"]
mod quarry_shots;
