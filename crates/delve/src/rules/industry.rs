//! Delve adapter for the pure kingdom economy. One host simulation owns the
//! state; Home settlement persists it by revision, just like the salvage yard.
use super::*;
use crate::kingdom::{self as economy, Command, Tile, catalog};

pub const SHIFT_TICKS: u64 = HZ as u64 * 10;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Board {
    pub open: bool,
    pub tile: Tile,
    pub plan: usize,
    pub message: String,
    pub held: u16,
    pub gallery: bool,
    pub moved: u64,
    pub page: u8,
    pub selection: usize,
    pub zoom: bool,
    pub plan_moved: u64,
    pub camera: Option<Tile>,
    pub pending_release: Option<String>,
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
        }
    }
}
impl Board {
    pub fn valid(&self) -> bool {
        self.tile.0 >= 0
            && self.tile.0 < economy::WIDTH
            && self.tile.1 >= 0
            && self.tile.1 < economy::HEIGHT
            && self.plan <= catalog().menu.len()
            && self.message.len() <= 256
            && self.held < 2048
            && self.page < 9
            && self.selection < 512
            && self
                .pending_release
                .as_ref()
                .is_none_or(|key| !key.is_empty() && key.len() <= 96)
            && self.camera.is_none_or(|(x, y)| {
                (0..economy::WIDTH).contains(&x) && (0..economy::HEIGHT).contains(&y)
            })
    }
}

#[derive(Clone, Debug)]
pub struct Deposit {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub clear: bool,
    pub resource: &'static str,
    pub name: &'static str,
}
/// The region adapter owns where resources occur; the economy owns their use.
/// The same mapping powers actual deposits, floor forecasts and the field guide.
pub fn ore_at(pack: Pack, depth: u32) -> (&'static str, &'static str) {
    match pack {
        Pack::Crypt => ("coal", "Black candle seam"),
        Pack::Cavern if depth.is_multiple_of(2) => ("crystal", "Moon crystal vein"),
        Pack::Cavern => ("iron-ore", "Ironroot vein"),
        Pack::Hellforge => ("ember", "Living ember seam"),
        _ => ("crystal", "Deep crystal vein"),
    }
}
pub const ORES: &[(&str, &str, &str, &str)] = &[
    (
        "iron-ore",
        "IRON ORE",
        "Start in Mines: floor 1",
        "Smelt for weapons and castle building.",
    ),
    (
        "coal",
        "BLACK COAL",
        "Crypt regions",
        "Fuel smelters. Charcoal also works.",
    ),
    (
        "crystal",
        "MOON CRYSTAL",
        "Mines F2 / Archive and beyond",
        "Bind mana for staves and arcane works.",
    ),
    (
        "ember",
        "LIVING EMBER",
        "Dragon Keep: floor 3",
        "Forge Ember Edge with iron and mana.",
    ),
];
impl Run {
    pub(super) fn announce_ore_forecast(&mut self) {
        if self.dungeon.depth == 0 {
            return;
        }
        let (id, name) = ore_at(self.dungeon.pack, self.dungeon.depth);
        let next_depth = self.dungeon.depth.saturating_add(1);
        let (next, _) = ore_at(Pack::at(next_depth, self.first), next_depth);
        if let Some(kind) = ore_threats::OreKind::for_resource(id) {
            let after = if next != id {
                format!(" Next floor: {}.", catalog().resources[next].name)
            } else {
                String::new()
            };
            self.notices.push(format!(
                "Prospector's warning - {name}: {} {}{}",
                kind.warning(),
                kind.counter(),
                after
            ));
        }
    }
    /// Enter the player-built surface with the whole party, as other surface
    /// doors do. The gate remains a route back to the existing campaign.
    pub fn enter_frontier(&mut self) {
        if self.dungeon.depth != 0 || self.room().kind != RoomKind::Kingdom {
            return;
        }
        self.home.industry.found(catalog(), self.seed);
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
        let (resource, name) = ore_at(self.dungeon.pack, self.dungeon.depth);
        let make = |i: usize, x: f32, y: f32, clear: bool| {
            let legacy = format!(
                "{:x}-{}-{}-{}",
                self.seed, self.raid_id, self.dungeon.depth, i
            );
            // A live Run may revisit the same floor index through another
            // region. Resource identity must not alias that earlier claim.
            let id = if self
                .home
                .industry
                .claims
                .get(&legacy)
                .is_some_and(|q| q.resource == resource)
            {
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
                name,
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
                    *i > 0 && (i % 3 == 1 || *i == usize::from(o.guardian)) && ch.stirred
                })
                .map(|(i, ch)| {
                    let (x0, y0, x1, y1) = ch.bounds();
                    make(i, (x0 + x1) / 2.0, (y0 + y1) / 2.0, ch.cleared)
                })
                .collect()
        } else if self.at > 0
            && self.at % 3 == 1
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
            let bits = u16::from(input.cast == 4)
                | (u16::from(input.interact) << 1)
                | (u16::from(input.swing) << 2)
                | (u16::from(input.bomb) << 3)
                | (u16::from(input.fire) << 4)
                | (u16::from(input.cast == 1) << 5)
                | (u16::from(input.cast == 2) << 6)
                | (u16::from(input.cast == 3) << 7)
                | (u16::from(input.cycle) << 8)
                | (u16::from(input.ride) << 9)
                | (u16::from(input.dash) << 10);
            let board = self.kingdom.industry.entry(id).or_default();
            let mut fresh = bits & !board.held;
            board.held = bits;
            if !surface {
                board.open = false;
                continue;
            }
            if fresh & 1 != 0 {
                board.open = !board.open;
                self.home.industry.found(catalog(), self.seed);
            }
            if !board.open && fresh & 1 == 0 {
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
                enter = true;
                continue;
            }
            if fresh & 256 != 0 {
                board.page = (board.page + 1) % 9;
                board.gallery = board.page == 5;
                board.selection = 0;
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
                    1 => catalog().technologies.len(),
                    2 => kingdom_gear::ITEMS.len(),
                    3 => self.home.industry.claims.len(),
                    4 => ORES.len(),
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
                        if let Some(key) = catalog().technologies.keys().nth(board.selection) {
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
        }
        if self.tick > previous_tick && self.active() {
            self.tick_ore_pressure();
        }
        self.reset_claim_capacity_notice();
        let deposits = self.industry_deposits();
        for d in deposits {
            if self
                .players
                .values()
                .any(|h| !h.absent && h.hp > 0 && (h.x - d.x).hypot(h.y - d.y) < 12.0)
            {
                self.home.industry.found(catalog(), self.seed);
                self.discover_frontier_claim(&d);
            }
            self.tick_ore_sentries(&d);
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
                    "Claim secured: {}. Connect a mine in the kingdom [4]",
                    d.name
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

#[cfg(test)]
#[path = "../../tests/rules/industry__tests.rs"]
mod tests;

#[cfg(all(test, feature = "renderer"))]
#[path = "../../tests/render/kingdom_frontier__shots.rs"]
mod shots;
#[cfg(all(test, feature = "renderer"))]
#[path = "../../tests/render/village__shots.rs"]
mod village_shots;
