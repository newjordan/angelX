//! The party's way through the dungeon: into a room through its door, down
//! the stairs onto a new floor, and out through an open doorway, down the
//! stairs or into the light home when the room lets them go.

use super::*;

impl Run {
    /// Walk the party into room `index` through its door `from` (the side
    /// they enter on), or onto a new floor's entrance with `None`.
    pub fn enter(&mut self, index: usize, from: Option<usize>) {
        let came_from = self.at;
        // Out of the safe room through a door: its featherbed's ward goes
        // along (`freedom`).
        let out_of_safe_room = from.is_some()
            && index != came_from
            && self.dungeon.rooms.get(came_from).is_some_and(|r| r.kind == RoomKind::Sanctuary);
        // Off an open floor's map, its companies wait where they are.
        if index != self.at && self.open_map() {
            self.parked = std::mem::take(&mut self.enemies).into_iter().filter(|e| e.hp > 0).collect();
        }
        self.remember_activity_entry(index);
        if self
            .dungeon
            .rooms
            .get(self.at)
            .is_some_and(|r| r.kind == RoomKind::Fortune)
            && index != self.at
        {
            if let Some(spin) = &mut self.spin {
                spin.settled = true;
                self.mode = fortune::Mode::WHEEL[usize::from(spin.wedge)];
                self.mode_said = Some(spin.started);
            }
        }
        self.joust = None;
        self.chariot = None;
        self.latch = None;
        self.brasswake = None;
        self.rafterwake = None;
        self.pennon = None;
        self.mossbell = None;
        self.forest.leave();
        self.forest_road = None;
        roundtable::clear(self);
        self.college = Default::default();
        self.pastimes.leave();
        self.build_hold = None;
        // Leaving/recreating a room discards its extra bodies, never refunds
        // provision. Active serialized bodies instead keep their journal IDs.
        if let Some(g) = &mut self.boss_gates {
            for e in &self.enemies {
                g.economy.retire(e.id);
            }
        }
        // Humphrey is never more than a room behind the party, and never
        // arrives mid-game: entering a room restarts his dwell.
        if self.dungeon.rooms[index].kind.in_world()
            || self.enemies.iter().any(|e| e.kind == EnemyKind::Risen)
        {
            self.home.countryside.humphrey_dig = risen::FOLLOW;
        }
        self.bank_coins();
        self.feel.clear_room();
        self.gore.clear();
        self.at = index;
        self.projectiles.clear();
        self.imps.clear();
        self.enemies.clear();
        self.strikes.clear();
        self.spheres.clear();
        self.phantoms.clear();
        self.bard_songs.clear();
        self.scarecrows.clear();
        for h in self.players.values_mut() { h.flee_ticks = 0; }
        self.champions.clear();
        self.heroes = Default::default();
        for h in self.players.values_mut() { h.hero_kit = Default::default(); }
        for h in self.players.values_mut() {
            (h.champ.haul, h.champ.windup) = (0, 0);
        }
        self.kegs.clear();
        self.lobs.clear();
        self.flings.clear();
        self.wells.clear();
        self.hooks.clear();
        self.webs.clear();
        self.slams.clear();
        self.ravages.clear();
        self.holes.clear();
        self.critters.clear();
        self.frogs.wild.clear();
        self.frogs.shown.clear();
        self.slimes_slain = 0;
        if self.dungeon.rooms[index].kind != RoomKind::Lair {
            self.light = None;
        }
        let delving = self.dungeon.depth > 0;
        let living: Vec<u32> = self.players.keys().copied().collect();
        let count = living.len() as f32;
        let (w, h) = (
            self.dungeon.rooms[index].width(),
            self.dungeon.rooms[index].height(),
        );
        // Onto an open map: out of the door just taken, or at the arrival.
        let landing = self
            .open_landing(index, came_from)
            .map(|at| self.open_spots(index, at, living.len()));
        for (slot, id) in living.into_iter().enumerate() {
            let offset = (slot as f32 - (count - 1.0) / 2.0) * stand_apart(count);
            let (x, y) = match from {
                _ if landing.is_some() => landing.as_ref().map_or((0.0, 0.0), |spots| spots[slot]),
                Some(0) => (w / 2.0 + offset, 3.3),
                Some(1) => (w - 3.3, h / 2.0 + offset),
                Some(2) => (w / 2.0 + offset, h - 3.3),
                Some(_) => (3.3, h / 2.0 + offset),
                // Home: on the rug below the Winding Stair's mouth.
                // Home: at the foot of the rug, a walk from the stair.
                None if self.dungeon.depth == 0 => (w / 2.0 + offset * 1.4, h - 4.5),
                None => (w / 2.0 + offset * 2.0, h - 6.0),
            };
            let hero = self.players.get_mut(&id).expect("listed hero");
            (hero.x, hero.y) = (x, y);
            // A skull dropped in the room before goes back to its bones.
            hero.skull_at = None;
            hero.invulnerable = hero.invulnerable.max(ENTRY_WARD);
            hero.dash_ticks = 0;
            hero.privy = 0;
            hero.pit.stop = None;
            hero.pit.result = None;
            // Dressed as they are: what they own, as their health allows.
            hero.dress();
            hero.gear_was = hero.gear;
            hero.pop = 0;
            hero.buying = 0;
            hero.buy_spent = false;
            // The pockets' memory of the room starts fresh, and a delve's
            // door takes its toll (`curios`).
            hero.curio_enters(delving);
        }
        if out_of_safe_room {
            self.freedom_leave_safe_room();
        }
        if matches!(
            self.dungeon.rooms[index].kind,
            RoomKind::Ledge | RoomKind::Wanted
        ) {
            self.arrive_side_on();
        }
        self.tallow_follows();
        self.hireling_follows();
        let sealed = self.guardian_locked();
        let first_visit = !self.dungeon.rooms[index].visited;
        if self.open_map() {
            self.enter_open(index, first_visit);
            return;
        }
        let folly_here = self.folly_room == Some(index)
            && !self.dungeon.rooms[index].cleared
            && !sealed
            && self.folly.is_none()
            && self.bats.is_none()
            && self.powder.is_none()
            && self.hop.is_none();
        if folly_here {
            self.dungeon.rooms[index].visited = true;
            // The halls' follies take turns down the delve: the Wax Maze,
            // the Belfry Shoot, then the Powder Room and Book Hop by turns.
            match self.dungeon.depth % 3 {
                1 => self.start_wax_maze(index, from),
                2 => self.start_bat_shoot(index),
                _ if self.dungeon.depth % 6 == 0 => self.start_book_hop(index),
                _ => self.start_powder(index),
            }
        }
        let gantry_here = self.gantry_room == Some(index)
            && !self.dungeon.rooms[index].cleared
            && !sealed
            && self.gantry.is_none();
        if gantry_here {
            self.dungeon.rooms[index].visited = true;
            self.start_gantry(index);
        }
        let race_here = self.race3d_room == Some(index)
            && !self.dungeon.rooms[index].cleared
            && !sealed
            && self.race3d.is_none();
        if race_here {
            self.dungeon.rooms[index].visited = true;
            self.start_race3d(index);
        }
        // A folly left mid-game and come back to plays on as it stood: its
        // room raises no company.
        let resumed = self.folly_running();
        let room = &mut self.dungeon.rooms[index];
        room.visited = true;
        if !room.cleared && !sealed && !folly_here && !gantry_here && !race_here && !resumed {
            let roster = room.roster.clone();
            let kind = room.kind;
            let pack = self.dungeon.pack;
            // The swamp's guardian is the Mire King, in its last hall only:
            // the stairs rooms above it are its toughest fights.
            let guarded = kind == RoomKind::Stairs
                && !(pack == Pack::Swamp && self.dungeon.depth < swamp::FLOORS);
            // A delve meets one of its pack's guardians, the same one for
            // the whole delve.
            let guardians: Vec<usize> = (0..self.bosses.len())
                .filter(|&i| self.bosses[i].only_in == pack && self.bosses[i].id != nooks::gnome::FLAMINGO && !deep::dread::is(&self.bosses[i]))
                .collect();
            let pick = (mix(self.seed ^ self.raid_id.rotate_left(17))
                % guardians.len().max(1) as u64) as usize;
            if kind == RoomKind::Wanted {
                // The showdown's outlaw is raised by its own hall.
                self.raise_outlaw();
            } else if self
                .boss_gates
                .as_ref()
                .is_some_and(|g| g.economy.camp(index).is_some())
            {
                let (apex, defeated, gate) = {
                    let g = self.boss_gates.as_ref().unwrap();
                    (g.guardian_boss, g.guardian_defeated, g.guardian == index)
                };
                let troops = self
                    .boss_gates
                    .as_mut()
                    .unwrap()
                    .economy
                    .deploy(index)
                    .unwrap();
                let faction = self
                    .boss_gates
                    .as_ref()
                    .unwrap()
                    .economy
                    .camp(index)
                    .unwrap()
                    .faction;
                // A chief's camp is led by its toughest, an elite who carries
                // the Boss Key; the floor's one boss is its guardian.
                let carrier = self.boss_gates.as_ref().unwrap().key_carrier(index);
                // On the first floors that boss stands alone: its troops
                // never come, and the floor's books count them gone.
                let alone = self.dungeon.depth <= BOSS_ALONE_TO
                    && gate
                    && !defeated
                    && (kind == RoomKind::Lair || apex.is_some());
                // So does the key's carrier: on the first floors its camp is
                // its spoils, not its company (a carrier still to come).
                let lone_carrier = self.dungeon.depth <= BOSS_ALONE_TO
                    && carrier.is_some_and(|c| troops.iter().any(|&(slot, _, _)| slot == c));
                for (slot, kind, troop_faction) in troops {
                    if alone || (lone_carrier && carrier != Some(slot)) {
                        self.boss_gates.as_mut().unwrap().economy.casualty(index, slot);
                        continue;
                    }
                    self.spawn(kind);
                    if let Some(e) = self.enemies.last_mut() {
                        e.camp_slot = Some(slot);
                        e.faction = Some(troop_faction);
                        if carrier == Some(slot) {
                            e.raise_elite();
                        }
                    }
                }
                if gate && !defeated {
                    if kind == RoomKind::Lair {
                        self.spawn(EnemyKind::Dragon);
                        self.enemies.last_mut().unwrap().faction = Some(faction);
                    } else if let Some(boss) = apex {
                        self.spawn_boss(usize::from(boss));
                        self.enemies.last_mut().unwrap().faction = Some(faction);
                    }
                }
                // The hoard is the camp's whole company, not just the troops
                // this entry deployed: a chief met again after the party left
                // still carries the spoils.
                if alone || lone_carrier {
                    let hoard = self
                        .boss_gates
                        .as_ref()
                        .unwrap()
                        .economy
                        .camp(index)
                        .unwrap()
                        .roster
                        .clone();
                    let holder = if alone {
                        self.enemies.last_mut()
                    } else {
                        self.enemies.iter_mut().find(|e| e.camp_slot.is_some() && e.camp_slot == carrier)
                    };
                    if let Some(holder) = holder {
                        holder.hoard = hoard;
                    }
                }
            } else if guarded && let Some(boss) = self.dread_at().or_else(|| guardians.get(pick).copied()) {
                self.spawn_boss(boss);
                // A guardian keeps a lighter escort, below the first floors;
                // on them it stands alone with the escort's spoils.
                if self.dungeon.depth > BOSS_ALONE_TO {
                    for kind in roster.into_iter().take(2) {
                        self.spawn(kind);
                    }
                } else if let Some(guardian) = self.enemies.last_mut() {
                    guardian.hoard = roster.into_iter().take(2).collect();
                }
            } else {
                // The monsters-a-room knob: more of the room's own kinds
                // join it, never fewer.
                let more = tune::more_foes(&roster, self.tune.foe_count);
                for kind in roster {
                    if kind == EnemyKind::PitTyrant
                        && self
                            .boss_gates
                            .as_ref()
                            .is_some_and(|g| g.optional_pit == Some(index) && g.pit_defeated)
                    {
                        continue;
                    }
                    if kind != EnemyKind::Dragon
                        || !self
                            .boss_gates
                            .as_ref()
                            .is_some_and(|g| g.guardian == index && g.guardian_defeated)
                    {
                        self.spawn(kind);
                    }
                }
                for kind in more {
                    self.spawn(kind);
                }
            }
        }
        self.swarm_up();
        self.maybe_frog(index, first_visit);
        if !self.enemies.is_empty() && (self.boss_gates.is_none() || first_visit) {
            self.deepen();
            self.maybe_goblin();
        }
        if let Some(caged) = self
            .captive
            .as_ref()
            .filter(|c| c.room == index && c.freed.is_none())
        {
            self.cues.push(format!("caged:{}", caged.who));
        }
        if self.dungeon.rooms[index].kind == RoomKind::Yard {
            self.raise_quintains();
        }
        self.phase = if self.foes_left()
            || (!sealed
                && !self.room().cleared
                && self.boss_gates.as_ref().is_some_and(|g| {
                    g.economy.camp(index).is_some() || g.optional_pit == Some(index)
                })) {
            Phase::Fighting
        } else {
            Phase::Exploring
        };
        self.room_hurt = false;
        if self.phase == Phase::Fighting {
            self.sounds.push("door_seal");
            // A fight closes the window: wishes come between fights.
            self.window = None;
        }
        // The identity start's rooms each give their wish as the party
        // walks in, fight or no fight.
        self.identity_room(index, first_visit);
        if let Some(boss) = self.enemies.iter().find_map(|e| e.boss) {
            let id = self.bosses[usize::from(boss)].id.clone();
            self.cues.push(format!("boss_rise:{id}"));
            self.sounds.push("boss_rise");
        } else if self.enemies.iter().any(|e| e.kind == EnemyKind::Dragon) {
            self.cues.push("lair".into());
        } else if self.enemies.iter().any(|e| e.kind == EnemyKind::PitTyrant) {
            self.cues.push("pit_rise".into());
            self.sounds.push("boss_rise");
        } else if self.phase == Phase::Fighting && self.room().great() {
            self.cues.push("great_hall".into());
        } else if self.phase == Phase::Fighting {
            self.cues.push("room_fight".into());
        }
        if sealed {
            self.rune = None;
            self.window = None;
            self.waves = Waves::default();
            self.traps.clear();
            self.rocks.clear();
            self.light = None;
            self.show_boss_lock();
        } else {
            self.place_rune();
            self.arm_room();
        }
        self.works_enter();
        self.notice_crack(first_visit);
        if self.room().kind == RoomKind::Sanctuary {
            self.hallow();
        }
        self.choose_altars();
    }

    pub fn descend(&mut self) {
        // Floors are shared. Returned knights can walk or rejoin at their
        // entry; replacing their floor while they are outside loses it.
        if !self.exits.outside.is_empty() {
            self.found = Some((self.tick, 0, "Friends are outside: E at their entry rejoins the party before taking the next floor.".into()));
            return;
        }
        self.exits = exits::State::default();
        let from_home = self.dungeon.depth == 0;
        // Guard the transition itself, not just the stair tile/UI.
        if !from_home
            && self
                .boss_gates
                .as_ref()
                .is_some_and(|g| !g.unlocked() || !self.dungeon.rooms[g.guardian].cleared)
        {
            if self.guardian_locked() {
                self.show_boss_lock();
            }
            return;
        }
        // Below the Unknown the Deep counts its own floors (`deep`).
        if !from_home && self.deep.is_some() {
            self.descend_deep();
            return;
        }
        if !from_home && self.dungeon.depth >= DEEPEST {
            return;
        }
        // The swamp ends in the Mire King's hall: no way down from it.
        if !from_home && self.dungeon.pack == Pack::Swamp && self.dungeon.depth >= swamp::FLOORS {
            return;
        }
        let (depth, pack) = if from_home {
            // Down the Winding Stair: to the landing chosen, in the delve
            // chosen for the top. A landing below the Unknown is the
            // Deep's (`deep`).
            let landing = self.home.landing();
            if landing > DEEPEST && self.home.deep_road() {
                self.enter_the_deep(landing);
                return;
            }
            self.deep = None;
            let depth = landing.clamp(1, DEEPEST - 1);
            self.first = self.dungeon.pack;
            (depth, Pack::at(depth, self.first))
        } else {
            // The floor behind is closed: its results open the next one's
            // plate.
            self.close_floor(floor_stats::End::Cleared);
            self.settle_dare();
            self.unwashed_toll();
            self.lift_gifts();
            if self.dungeon.depth != FLOORS {
                // A dragon's floor was reclaimed and banked when it fell.
                self.reclaimed.push(self.dungeon.pack);
                self.bank_all(true, &format!("floor {} cleared", self.dungeon.depth));
            }
            // Through a hell gate the delve starts over, a circle deeper.
            let depth = if self.spectacle.circling {
                1
            } else {
                self.dungeon.depth + 1
            };
            let first = if self.dungeon.depth == 1 {
                self.dungeon.pack
            } else {
                self.first
            };
            self.first = first;
            (depth, Pack::at(depth, first))
        };
        if !from_home && self.dungeon.depth % 3 == 0 {
            self.begin_crossroads(depth, pack);
            return;
        }
        self.finish_descent(depth, pack, from_home);
    }

    pub(super) fn finish_descent(&mut self, depth: u32, pack: Pack, from_home: bool) {
        if !from_home && depth == 1 {
            self.spectacle.card = Some((self.tick, self.spectacle.circle));
        }
        if from_home {
            self.notice("first_delve");
        } else {
            if self.mode == fortune::Mode::GlassJaw {
                self.notice("fragile");
            }
            if self.collapse_at.is_some_and(|at| self.tick >= at) {
                self.notice("out_of_time");
            }
        }
        if depth == FLOORS + 1 {
            self.notice("below_the_bottom");
        }
        self.settlement_site = None;
        self.settlement_exhibits.clear();
        // Down out of the upper floors into the deep: the dungeon stands up.
        if depth >= DEEP_FLOOR && self.dungeon.depth < DEEP_FLOOR {
            self.deep_beat = Some(self.tick);
        }
        self.dungeon = if self.identity && depth == 1 {
            // The identity start: character creation, three rooms, or the
            // streamlined one room.
            if identity::simple() {
                layout::identity_room(pack, self.party_size(), &mut self.rng)
            } else {
                layout::identity_floor(pack, self.party_size(), &mut self.rng)
            }
        } else if let Some(village) = self.deep_village_floor() {
            // A village of the Deep: its one great cavern (`deep::village`).
            village
        } else if self.wants_open() {
            layout::open_floor(depth, pack, self.party_size(), &mut self.rng)
        } else {
            layout::floor_for_party(self.mode, depth, pack, self.party_size(), &mut self.rng)
        };
        self.parked.clear();
        self.freedom.new_floor();
        // A floor's stars are its own (`starfall`).
        self.starfall_new_floor();
        self.stock_gear_cache();
        self.at = 0;
        self.kingdom.return_to = None;
        // A village of the Deep is its cavern and no more.
        let village = self.dungeon.rooms.first().is_some_and(|r| r.kind == RoomKind::DeepVillage);
        // A delve the King holds has its forge-hall off its first hall.
        if !(self.identity && depth == 1) && !village {
            barony::add_forge_hall(&mut self.dungeon, &self.home.barony);
            // A floor the realm holds has its steward's hall, as built.
            holdings::add_holding_hall(&mut self.dungeon, &self.home.holdings);
        }
        // Every floor below keeps a nook, a lived-in dead end; the wheel's
        // one-hall horde and straight gauntlet keep their bare shapes.
        // The identity start's first floor is its three rooms and no more.
        let creation = self.identity && depth == 1;
        if !matches!(self.mode, fortune::Mode::HoldTheStair | fortune::Mode::Gauntlet) && !creation && !village {
            nooks::add_nook(&mut self.dungeon);
        }
        self.ready_nook();
        self.holding_used = 0;
        self.holding_raid = None;
        self.populate_boss_gates();
        if self.simple_creation() {
            // The streamlined room has no guardian and no key.
            self.boss_gates = None;
        }
        self.cage_someone();
        self.pick_folly();
        self.pick_gantry();
        self.pick_race3d();
        if self.dungeon.open.is_some() || creation || village {
            // The rooms' games wait for open floors (docs/OPEN-FLOORS.md),
            // and keep out of the identity start's three rooms.
            self.folly_room = None;
            self.gantry_room = None;
            self.race3d_room = None;
        }
        self.pots_broken.clear();
        self.pips_map();
        if from_home {
            if self.squire_company_reserved || self.squire_company_claimed {
                self.squire_pending_delves = self.squire_pending_delves.saturating_add(1);
            } else {
                self.home.squire_company.delves = self.home.squire_company.delves.saturating_add(1);
                for roster in self.home.squire_company.rosters.values_mut() {
                    roster.claimed.clear();
                }
            }
            self.kit_out();
            self.harden();
            self.deal_ults();
            self.bring_tallow();
            self.residents_help();
            self.runes_taken = 0;
            self.hexes = 0;
            self.drink_round();
            self.sing_song();
            self.hire();
            self.countryside_squires.clear();
            if !self.home.countryside.squires.is_empty() {
                self.marks.insert(format!("country:squire-day:{}", self.home.countryside.squires.day), 1);
            }
            self.deploy_squires();
            self.deploy_company_squires();
            self.squire_delve();
            self.embark_squires();
            self.depart_squires();
            self.iron();
            self.cues.push("run_start".into());
            // A delve starts: three wishes each, fresh (`overclass`).
            for hero in self.players.values_mut() {
                (hero.wishes, hero.great_wishes) = (0, 0);
            }
            // The identity start gives its wishes a room at a time.
            if !self.identity || depth != 1 {
                self.give_wishes(overclass::START_WISHES, 0);
            }
            self.raised_gifts(true);
        } else {
            self.refill_winds();
            // A new floor: two more wishes each (three once the genie is free).
            self.give_wishes(self.floor_wishes(), 0);
            self.raised_gifts(false);
        }
        // A village of the Deep never falls in on its people.
        self.collapse_at = (self.mode == fortune::Mode::Collapse && !village)
            .then(|| self.tick + u64::from(fortune::Mode::collapse_secs(depth) * HZ));
        self.cues.push(format!("descend:{depth}"));
        self.sounds.push("descend");
        if self.players.values().any(|h| h.hp == 0) {
            self.cues.push("revive".into());
        }
        for hero in self.players.values_mut() {
            // The stairs bring a fallen companion back to their feet.
            if hero.hp == 0 {
                hero.hp = (hero.max_hp * REVIVE_SHARE / 100).max(1);
                hero.invulnerable = hero.invulnerable.max(REVIVE_WARD);
            } else {
                hero.mend(10, stakes::Mend::Rest);
            }
        }
        self.provision_bombs(true);
        self.half_charge();
        self.enter(0, None);
        self.deploy_squires();
        self.deploy_company_squires();
        if village {
            self.dare = None;
        } else {
            self.call_dare();
        }
        // A floor of the Deep: its village met, or its errands' signs laid.
        self.ready_deep_floor();
        // A village's own arrival is its falling star, not a title plate.
        if !village {
            self.begin_floor_plate(floor_plates::Kind::of(pack));
        }
        if from_home {
            self.floor_results = None;
            self.floor_tally.new_delve(self.tick);
            self.deeds.clear();
        } else {
            self.floor_tally.next_floor(self.tick);
            for plate in self.floor_plates.values_mut() {
                plate.results = floor_stats::SHOWN;
            }
        }
    }

    /// Through an open doorway into the next room, down the stairs, or
    /// into the light home.
    pub(super) fn travel(&mut self) {
        if self.room().kind == RoomKind::Kingdom {
            return;
        }
        if self.step_into_gate() {
            return;
        }
        // The light home takes the whole party or no one: one knight
        // brushing it no longer ends the delve for friends who would go on.
        // Those in it wait for the fellowship, and the rest are called.
        let in_light: Vec<u32> = self.light.map_or_else(Vec::new, |(lx, ly)| {
            self.players
                .iter()
                .filter(|(id, h)| self.counts_in_fellowship(**id) && (h.x - lx).hypot(h.y - ly) < 1.6)
                .map(|(&id, _)| id)
                .collect()
        });
        let counted = self.players.keys().filter(|&&id| self.counts_in_fellowship(id)).count();
        if !in_light.is_empty() && in_light.len() < counted {
            self.fellowship.wait(in_light.iter().map(|&id| (id, fellowship::LIGHT)));
        }
        if !in_light.is_empty() && in_light.len() >= counted {
            // Keep the finite, validated exit marker as victory evidence for
            // saves and sequential mirrors. Won no longer runs travel.
            self.phase = Phase::Won;
            self.cues.push("homeward".into());
            self.sounds.push("descend");
            self.wishes_home(true);
            self.bank_all(true, "home with the dragon's hoard");
            return;
        }
        // Through a door off an open map into its side room.
        if self.open_doors() {
            return;
        }
        let mut exit = None;
        let mut exit_seat = None;
        let mut stairs_seat = None;
        let mut stairs = false;
        // At home, stairs that belong to an entrance go where it goes; only
        // the Undercroft's Winding Stair goes down into the delve.
        let mut entrance: Option<&'static world::Entrance> = None;
        let home = self.at_home_now();
        let room = self.room();
        // The fellowship: who counts, and who stands in which way out. A
        // doorway or the stairs move the party only when every knight who
        // counts stands in that same one (`fellowship`).
        let mut counted = 0;
        let mut in_door: BTreeMap<u32, usize> = BTreeMap::new();
        let mut on_stairs: BTreeMap<u32, Option<&'static str>> = BTreeMap::new();
        // An away knight standing in a doorway or on a stair takes the party
        // nowhere, and does not hold the stair for those who come after.
        for (&id, hero) in self.players.iter().filter(|(id, _)| self.counts_in_fellowship(**id)) {
            counted += 1;
            let dir = if hero.y < 0.6 {
                Some(0)
            } else if hero.x > room.width() - 0.6 {
                Some(1)
            } else if hero.y > room.height() - 0.6 {
                Some(2)
            } else if hero.x < 0.6 {
                Some(3)
            } else {
                None
            };
            if dir.is_some() {
                exit = dir;
                exit_seat = Some(id);
            }
            if let Some(dir) = dir {
                // Out of a side-on hall, any way out is its one doorway back.
                in_door.insert(id, room.way_back().unwrap_or(dir));
            }
            let col = (hero.x / TILE_UNITS) as i32;
            let row = (hero.y / TILE_UNITS) as i32;
            if room.tile(col, row) == Tile::Stairs {
                let gate = home
                    .then(|| world::entrance_at(room.kind, col, row))
                    .flatten();
                // A door the realm has not built yet is only a wall.
                if gate.is_some_and(|e| !world::open(e, &self.home)) {
                    continue;
                }
                // Home stairs that are no entrance are the Winding Stair's
                // only in the Undercroft itself.
                if gate.is_some() || !home || room.kind == RoomKind::Home {
                    stairs = true;
                    if entrance.is_none() {
                        stairs_seat = Some(id);
                    }
                    entrance = entrance.or(gate);
                    on_stairs.insert(id, gate.map(|e| e.id));
                }
            }
        }
        // A Colosseum arena's gate stays shut while its heat runs, and the
        // streamlined first floor's while a knight is still wishing.
        if self.arena_heat_on() || self.creation_holds_stairs() {
            stairs = false;
            entrance = None;
            exit = None;
        }
        if !stairs {
            self.stairs_held = false;
            self.descending = 0;
        }
        // Each entrance keeps its own hold: a knight stepping from one plate
        // onto the next begins again rather than taking the wrong one.
        let held_for = entrance.map(|e| e.id);
        if self.descend_for != held_for {
            self.descend_for = held_for;
            self.descending = 0;
        }
        // Out of a side-on hall, any way out is its one doorway back.
        let exit = exit.map(|dir| self.room().way_back().unwrap_or(dir));
        // A way out is open to the party when all of it stands there. A
        // retreat to the kingdom is a seat's own decision, and waits for no
        // one (`exits`).
        let at_stairs = on_stairs.values().filter(|&&g| g == held_for).count() == counted;
        let retreat = entrance.is_some_and(|e| self.returns_to_kingdom(e.id));
        let stairs_ready = at_stairs || retreat;
        // At home the Winding Stair is taken on purpose: a knight stands on
        // it a moment (its ring fills) rather than brushing past it. The ring
        // fills only while the party is on it.
        let deliberate = if self.at_home_now() && stairs && !self.stairs_held {
            if stairs_ready {
                self.descending += 1;
            } else {
                self.descending = 0;
            }
            self.descending >= home::DESCEND_HOLD
        } else {
            true
        };
        // An open floor's stairs wait for its guardian to fall.
        if stairs && self.open_guarded() {
            stairs = false;
        }
        if stairs && self.guardian_locked() {
            // Do not let a stair attempt swallow a simultaneous retreat.
            if self.tick.is_multiple_of(u64::from(HZ)) {
                self.show_boss_lock();
            }
            stairs = false;
        }
        if stairs && !self.stairs_held && !stairs_ready {
            self.fellowship.wait(on_stairs.keys().map(|&id| (id, fellowship::STAIRS)));
        }
        if stairs && !self.stairs_held && deliberate && stairs_ready {
            self.descending = 0;
            match entrance {
                Some(e) if self.returns_to_kingdom(e.id) => {
                    if let Some(id) = stairs_seat {
                        self.leave_activity(id);
                    }
                }
                Some(e) => self.take_entrance(e),
                None => self.descend(),
            }
        } else if exit.is_some() && self.kingdom.return_to.is_some() && self.room().kind.in_world()
        {
            if let Some(id) = exit_seat {
                self.leave_activity(id);
            }
        } else if let Some(dir) = exit
            && let Some(next) = self.dungeon.neighbour(self.at, dir)
        {
            // The boss door stays shut until the key is carried to it.
            if self.boss_door_holds(next, dir) {
                self.fellowship.clear();
                return;
            }
            let returning = exits::room(self.room().kind)
                && !self.room().kind.in_world()
                && self.activity_return().is_some_and(|back| back.at == next);
            if returning {
                if let Some(id) = exit_seat {
                    self.leave_activity(id);
                }
            } else if in_door.values().filter(|&&d| d == dir).count() == counted {
                self.enter(next, Some((dir + 2) % 4));
            } else {
                self.fellowship.wait(in_door.iter().map(|(&id, &d)| (id, d as u8)));
            }
        }
    }
}
