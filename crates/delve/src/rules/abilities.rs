//! Portable game port for executable ability modules. These are physics/data
//! primitives, not a catalogue of authored abilities. The host owns executable
//! instances; rules admit requests at the same seam as ordinary spell casts.
use super::*;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Slot {
    pub module: String,
    pub generation: u64,
    pub ability: String,
    pub name: String,
    pub mana: f32,
    pub cooldown: u32,
    pub icon: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub raid: u64,
    pub epoch: u64,
    pub definition: Slot,
    pub player: u32,
    pub slot: u8,
    pub module: String,
    pub generation: u64,
    pub ability: String,
    pub tick: u64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Origin {
    pub module: String,
    pub generation: u64,
    pub cast: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Command {
    Projectile {
        offset: [f32; 2],
        velocity: [f32; 2],
        damage: u32,
        ttl: u32,
        #[serde(default)]
        pierce: u8,
        #[serde(default = "bolt")]
        kind: Shot,
    },
    Heal {
        amount: u32,
    },
    Move {
        delta: [f32; 2],
    },
    StatusArea {
        center: [f32; 2],
        radius: f32,
        status: Status,
        #[serde(default = "sight_by_default")]
        line_of_sight: bool,
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Status {
    Stun { ticks: u32 },
}
fn sight_by_default() -> bool {
    true
}
fn bolt() -> Shot {
    Shot::Bolt
}
pub fn empty_slots(slots: &[Option<Slot>; cards::SPELL_SLOTS]) -> bool {
    slots.iter().all(Option::is_none)
}
impl Slot {
    pub fn validate(&self) -> Result<(), String> {
        let text = |s: &str, max: usize| !s.is_empty() && s.len() <= max && !s.chars().any(char::is_control);
        if text(&self.module, 256) && text(&self.ability, 256) && text(&self.name, 48)
            && self.icon.len() <= 4096 && self.mana.is_finite() && (0.0..=1000.0).contains(&self.mana)
            && (1..=3600).contains(&self.cooldown) { Ok(()) }
        else { Err("invalid ability slot definition".into()) }
    }
}
impl Request {
    pub fn validate(&self) -> Result<(), String> {
        self.definition.validate()?;
        if (1..=MAX_PLAYERS).contains(&self.player) && usize::from(self.slot) < cards::SPELL_SLOTS
            && self.tick <= runtime_state::MAX_TICK
            && self.module == self.definition.module && self.generation == self.definition.generation
            && self.ability == self.definition.ability { Ok(()) }
        else { Err("invalid retained ability request".into()) }
    }
}
/// Outbox entries can be stale after seat/source changes and remain available
/// for explicit retirement. Validation never guesses a replacement owner.
pub fn validate_outbox(requests: &[Request]) -> Result<(), String> {
    if requests.len() > 16 { return Err("too many retained ability requests".into()); }
    for (i, request) in requests.iter().enumerate() {
        request.validate()?;
        if requests[..i].contains(request) { return Err("duplicate retained ability request".into()); }
    }
    Ok(())
}
impl Command {
    pub fn validate(&self) -> Result<(), String> {
        let bounded =
            |v: &[f32; 2], limit: f32| v.iter().all(|v| v.is_finite() && v.abs() <= limit);
        let valid = match self {
            Self::Projectile {
                offset,
                velocity,
                damage,
                ttl,
                pierce,
                ..
            } => {
                bounded(offset, 128.0)
                    && bounded(velocity, 300.0)
                    && *damage <= 100_000
                    && (1..=3600).contains(ttl)
                    && *pierce <= 16
            }
            Self::Heal { amount } => *amount <= 100_000,
            Self::Move { delta } => bounded(delta, 32.0),
            Self::StatusArea {
                center,
                radius,
                status,
                ..
            } => {
                bounded(center, 100_000.0)
                    && radius.is_finite()
                    && (0.1..=16.0).contains(radius)
                    && match status {
                        Status::Stun { ticks } => (1..=600).contains(ticks),
                    }
            }
        };
        if valid {
            Ok(())
        } else {
            Err("ability command exceeds the game port contract".into())
        }
    }
}
impl Run {
    pub fn validate_ability_commands(
        &self,
        commands: &[(u32, Origin, Command)],
    ) -> Result<(), String> {
        if commands.len() > 64
            || self.projectiles.len()
                + commands
                    .iter()
                    .filter(|(_, _, c)| matches!(c, Command::Projectile { .. }))
                    .count()
                > MAX_PROJECTILES
        {
            return Err("ability command/projectile capacity exceeded".into());
        }
        for (player, _, command) in commands {
            if !self.players.contains_key(player) {
                return Err("ability command owner no longer exists".into());
            }
            command.validate()?;
        }
        Ok(())
    }
    /// Called only after the complete batch and instance transition validate.
    /// All operations are in-memory and infallible inside the game commit lock.
    pub fn apply_ability_commands(&mut self, commands: Vec<(u32, Origin, Command)>) {
        let barred = self.barred();
        for (player, origin, command) in commands {
            let Some(hero) = self.players.get_mut(&player) else {
                continue;
            };
            match command {
                Command::Projectile {
                    offset,
                    velocity,
                    damage,
                    ttl,
                    pierce,
                    kind,
                } => self.projectiles.push(Projectile {
                    x: hero.x + offset[0],
                    y: hero.y + offset[1],
                    vx: velocity[0],
                    vy: velocity[1],
                    hostile: false,
                    kind,
                    look: None,
                    damage,
                    pierce,
                    last_hit: None,
                    ttl,
                    empowered: false,
                    traits: ShotTraits {
                        owner: player as u8,
                        module_origin: Some(origin),
                        ..Default::default()
                    },
                }),
                Command::Heal { amount } => {
                    hero.hp = hero.hp.saturating_add(amount).min(hero.max_hp)
                }
                Command::Move { delta } => {
                    let grid = Grid {
                        room: &self.dungeon.rooms[self.at],
                        barred,
                    };
                    (hero.x, hero.y) = grid.slide(
                        (hero.x, hero.y),
                        (delta[0], delta[1]),
                        HERO_RADIUS,
                        Mover::Hero,
                    );
                }
                Command::StatusArea {
                    center,
                    radius,
                    status,
                    line_of_sight,
                } => {
                    let grid = Grid {
                        room: &self.dungeon.rooms[self.at],
                        barred,
                    };
                    for enemy in self.enemies.iter_mut().filter(|enemy| enemy.hp > 0) {
                        if (enemy.x - center[0]).hypot(enemy.y - center[1]) > radius
                            || (line_of_sight
                                && !critters::clear_line(
                                    &grid,
                                    (center[0], center[1]),
                                    (enemy.x, enemy.y),
                                ))
                        {
                            continue;
                        }
                        match status {
                            Status::Stun { ticks } => {
                                // Reuse ordinary stun/anti-chain behavior. A pulse
                                // never shortens a stronger existing hold.
                                let ticks = if enemy.is_boss() {
                                    ticks.div_ceil(3)
                                } else {
                                    ticks
                                };
                                if enemy.wits.steady == 0 || ticks <= press::JOLT {
                                    enemy.frozen = enemy.frozen.max(ticks);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn room() -> Run {
        let mut run = Run::new(3, 9, None);
        let at = run
            .dungeon
            .rooms
            .iter()
            .position(|room| room.kind == RoomKind::Fight)
            .unwrap();
        run.enter_for_test(at);
        run.calm_for_test();
        run.enemies.clear();
        let room = &mut run.dungeon.rooms[at];
        for y in 1..room.rows - 1 {
            for x in 1..room.cols - 1 {
                room.set(x, y, Tile::Floor);
            }
        }
        run.players.get_mut(&1).unwrap().x = 8.0;
        run.players.get_mut(&1).unwrap().y = 8.0;
        run.spawn_at_for_test(EnemyKind::Skeleton, 10.0, 8.0);
        run.spawn_at_for_test(EnemyKind::Skeleton, 15.0, 8.0);
        run
    }
    fn status(ticks: u32) -> Command {
        Command::StatusArea {
            center: [8.0, 8.0],
            radius: 3.5,
            status: Status::Stun { ticks },
            line_of_sight: true,
        }
    }
    fn apply(run: &mut Run, command: Command) {
        let commands = vec![(
            1,
            Origin {
                module: "test".into(),
                generation: 1,
                cast: "field".into(),
            },
            command,
        )];
        run.validate_ability_commands(&commands).unwrap();
        run.apply_ability_commands(commands);
    }
    #[test]
    fn area_status_stops_nearby_enemy_movement_and_attacks_then_expires() {
        let mut run = room();
        let before = run.enemies[0].clone();
        apply(&mut run, status(3));
        assert_eq!(run.enemies[0].frozen, 3);
        assert_eq!(run.enemies[1].frozen, 0);
        for left in (0..3).rev() {
            run.step_sim_for_test(&BTreeMap::new());
            let enemy = &run.enemies[0];
            assert_eq!(
                (enemy.x, enemy.y, enemy.age, enemy.timer, enemy.stage),
                (before.x, before.y, before.age, before.timer, before.stage)
            );
            assert_eq!(enemy.frozen, left);
        }
        run.step_sim_for_test(&BTreeMap::new());
        assert!(
            run.enemies[0].age > before.age,
            "enemy did not resume after the hold"
        );
        assert!(
            run.enemies[1].age > before.age,
            "out-of-range enemy was held"
        );
    }
    #[test]
    fn area_status_obeys_walls_and_can_explicitly_target_through_them() {
        let mut run = room();
        run.enemies[0].x = 14.0;
        run.dungeon.rooms[run.at].set(5, 4, Tile::Wall);
        let mut command = status(12);
        if let Command::StatusArea { radius, .. } = &mut command {
            *radius = 8.0;
        }
        apply(&mut run, command.clone());
        assert_eq!(run.enemies[0].frozen, 0);
        if let Command::StatusArea { line_of_sight, .. } = &mut command {
            *line_of_sight = false;
        }
        apply(&mut run, command);
        assert_eq!(run.enemies[0].frozen, 12);
    }
    #[test]
    fn area_status_respects_boss_resistance_steady_enemies_and_stronger_holds() {
        let mut run = room();
        run.enemies[0].kind = EnemyKind::Dragon;
        apply(&mut run, status(12));
        assert_eq!(run.enemies[0].frozen, 4);
        run.enemies[0].kind = EnemyKind::Skeleton;
        run.enemies[0].frozen = 30;
        apply(&mut run, status(12));
        assert_eq!(run.enemies[0].frozen, 30);
        run.enemies[0].frozen = 0;
        run.enemies[0].wits.steady = 10;
        apply(&mut run, status(12));
        assert_eq!(run.enemies[0].frozen, 0);
    }
    #[test]
    fn area_status_rejects_nonfinite_and_unbounded_candidates_before_the_batch() {
        for value in [
            serde_json::json!({"type":"status-area","center":[0,0],"radius":17,"status":{"type":"stun","ticks":12}}),
            serde_json::json!({"type":"status-area","center":[0,0],"radius":3,"status":{"type":"stun","ticks":0}}),
        ] {
            let command: Command = serde_json::from_value(value).unwrap();
            assert!(command.validate().is_err());
        }
        let mut command = status(12);
        if let Command::StatusArea { center, .. } = &mut command {
            center[0] = f32::NAN;
        }
        assert!(command.validate().is_err());
        assert!(serde_json::from_value::<Command>(serde_json::json!({"type":"status-area","center":[0,0],"radius":3,"status":{"type":"grant-tool-access","ticks":12}})).is_err());
    }
    #[test]
    fn an_applied_area_status_survives_rules_checkpoint_handoff() {
        let mut run = room();
        apply(&mut run, status(12));
        let mut restored =
            runtime_state::restore(runtime_state::capture_for_rules(&run).unwrap()).unwrap();
        assert_eq!(restored.enemies[0].frozen, 12);
        restored.step_sim_for_test(&BTreeMap::new());
        assert_eq!(restored.enemies[0].frozen, 11);
    }
    #[test]
    fn queued_executable_request_captures_epoch_raid_and_exact_definition_at_enqueue() {
        let mut run = room();
        let definition = Slot { module: "source".into(), generation: 7, ability: "old".into(),
            name: "Old".into(), mana: 5.0, cooldown: 60, icon: String::new() };
        let epoch = run.seat_epoch(1);
        let hero = run.players.get_mut(&1).unwrap();
        hero.module_spells[0] = Some(definition.clone());
        hero.spell_cooldowns[0] = 0;
        hero.mana = 100.0;
        run.step_sim_for_test(&BTreeMap::from([(1, Input { cast: 1, ..Default::default() })]));
        let request = run.module_casts[0].clone();
        assert_eq!(request.raid, run.raid_id);
        assert_eq!(request.epoch, epoch);
        assert_eq!(request.definition, definition);
        run.players.get_mut(&1).unwrap().module_spells[0].as_mut().unwrap().mana = 99.0;
        run.seat_epochs.insert(1, epoch + 1);
        assert_eq!(run.module_casts[0], request);
        let restored = runtime_state::restore(runtime_state::capture(&run).unwrap()).unwrap();
        assert_eq!(restored.module_casts[0], request);
    }

    #[test]
    fn held_executable_input_keeps_one_exact_intent_but_new_source_and_seat_are_distinct() {
        let mut run = room();
        let definition = Slot { module: "source".into(), generation: 7, ability: "old".into(),
            name: "Old".into(), mana: 5.0, cooldown: 60, icon: String::new() };
        let hero = run.players.get_mut(&1).unwrap();
        hero.module_spells[0] = Some(definition);
        hero.spell_cooldowns[0] = 0;
        hero.mana = 100.0;
        let held = BTreeMap::from([(1, Input { cast: 1, ..Default::default() })]);
        run.step_sim_for_test(&held);
        let first = run.module_casts[0].clone();
        for _ in 0..20 { run.step_sim_for_test(&held); }
        assert_eq!(run.module_casts, vec![first.clone()]);
        assert_eq!(run.players[&1].spell_cooldowns[0], 1);
        // Even a same-generation definition change is a different intent.
        run.players.get_mut(&1).unwrap().module_spells[0].as_mut().unwrap().mana = 6.0;
        run.step_sim_for_test(&held);
        assert_eq!(run.module_casts.len(), 2);
        assert_eq!(run.module_casts[0], first);
        let second = run.module_casts[1].clone();
        let epoch = run.seat_epoch(1);
        run.seat_epochs.insert(1, epoch + 1);
        run.step_sim_for_test(&held);
        assert_eq!(run.module_casts.len(), 3);
        assert_eq!(run.module_casts[1], second);
        let current = run.module_casts[2].clone();
        let fence = super::super::session::abilities::OwnerFence::capture(&run, "coalescing-test");
        super::super::session::abilities::apply(&mut run, None, "coalescing-test",
            super::super::session::abilities::Event::Cancel { fence, requests: vec![first, second] }).unwrap();
        assert_eq!(run.module_casts, vec![current.clone()]);
        run.step_sim_for_test(&held);
        assert_eq!(run.module_casts, vec![current]);
    }

}
