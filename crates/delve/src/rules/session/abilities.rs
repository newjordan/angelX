//! Pure executable-ability admission. Native resources and actor transactions
//! remain native; only their exact retained source facts enter this port.
use super::frame;
use crate::rules::{Run, abilities as port, cards};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_CASTS: usize = 16;
const MAX_ACTIONS: usize = 64;
const MAX_BYTES: usize = 1024 * 1024;

/// Trusted native Runtime handoff, never constructed from executable output.
/// `started` distinguishes an admitted native pending source from a paid cast.
/// The original session/request survive surface replacement and worker reload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActiveCast {
    pub session: String,
    pub request: port::Request,
    pub origin: port::Origin,
    pub started: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerFence {
    pub session: String,
    pub raid: u64,
    pub tick: u64,
    pub seats: BTreeMap<u32, u64>,
}
impl OwnerFence {
    pub fn capture(run: &Run, session: &str) -> Self {
        Self {
            session: session.into(),
            raid: run.raid_id,
            tick: run.tick,
            seats: frame::seats(run),
        }
    }
    fn validate(&self, run: &Run, session: &str) -> Result<(), String> {
        if !text(&self.session, 128)
            || self.tick > super::MAX_TICK
            || self.seats.len() > crate::rules::MAX_PLAYERS as usize
            || self
                .seats
                .keys()
                .any(|id| !(1..=crate::rules::MAX_PLAYERS).contains(id))
            || self != &Self::capture(run, session)
        {
            return Err("stale ability ownership fence".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Replacement {
    pub player: u32,
    pub epoch: u64,
    pub observed: [Option<port::Slot>; cards::SPELL_SLOTS],
    pub slots: [Option<port::Slot>; cards::SPELL_SLOTS],
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub cast: ActiveCast,
    pub command: port::Command,
}
/// Native derives this sound fact from its retained media binding; executable
/// output supplies commands only and cannot mark an existing cast as a start.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Start {
    pub cast: ActiveCast,
    pub default_sound: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Batch {
    pub fence: OwnerFence,
    pub frame: frame::Fence,
    pub starts: Vec<Start>,
    pub actions: Vec<Action>,
    pub acknowledge: Vec<port::Request>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Event {
    ReplaceSlots {
        fence: OwnerFence,
        replacements: Vec<Replacement>,
    },
    /// Native has retained these exact requests with their executable source.
    /// This acknowledges transport admission, without spending game costs.
    Acknowledge {
        fence: OwnerFence,
        requests: Vec<port::Request>,
    },
    /// Explicit stale/cancelled request retirement, never implicit on preview.
    Cancel {
        fence: OwnerFence,
        requests: Vec<port::Request>,
    },
    Batch(Batch),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Claims {
    pub fence: OwnerFence,
    pub requests: Vec<port::Request>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum Reply {
    Changed(bool),
    Acknowledged(usize),
    Applied {
        starts: usize,
        commands: usize,
        acknowledged: usize,
    },
}

fn text(s: &str, max: usize) -> bool {
    !s.is_empty() && s.len() <= max && !s.chars().any(char::is_control)
}
fn slot(s: &port::Slot) -> bool {
    s.validate().is_ok()
}
fn request(r: &port::Request) -> bool {
    r.validate().is_ok()
}
fn bounded<T: Serialize>(value: &T) -> Result<(), String> {
    struct Count(usize);
    impl std::io::Write for Count {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 = self
                .0
                .checked_add(bytes.len())
                .filter(|n| *n <= MAX_BYTES)
                .ok_or_else(|| std::io::Error::other("ability payload exceeds bound"))?;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    serde_json::to_writer(Count(0), value).map_err(|e| e.to_string())
}
fn owns_seat(run: &Run, r: &port::Request) -> bool {
    r.raid == run.raid_id
        && r.tick <= run.tick
        && run.seat_epoch(r.player) == r.epoch
        && run.players.contains_key(&r.player)
}
fn proof(cast: &ActiveCast, run: &Run) -> bool {
    text(&cast.session, 128)
        && request(&cast.request)
        && owns_seat(run, &cast.request)
        && cast.origin.module == cast.request.module
        && cast.origin.generation == cast.request.generation
        && text(&cast.origin.cast, 256)
}
/// Validate native handoff before the frame prelude. Old surface generations
/// are valid, including paid casts whose original owner died. Callers filter
/// seats removed by that prelude without touching native resource lifetime. Session also checks each original session string.
pub fn validate_active_casts(casts: &[ActiveCast], run: &Run) -> Result<(), String> {
    bounded(&casts)?;
    if casts.len() > MAX_CASTS {
        return Err("too many native ability owners".into());
    }
    for (i, cast) in casts.iter().enumerate() {
        if !proof(cast, run)
            || casts[..i]
                .iter()
                .any(|other| other.origin == cast.origin || other.request == cast.request)
        {
            return Err("invalid/duplicate native ability owner".into());
        }
    }
    Ok(())
}
pub fn claim(run: &Run, scope: &str) -> Result<Claims, String> {
    if !text(scope, 128) || port::validate_outbox(&run.module_casts).is_err() {
        return Err("invalid ability outbox".into());
    }
    let claims = Claims {
        fence: OwnerFence::capture(run, scope),
        requests: run.module_casts.clone(),
    };
    bounded(&claims)?;
    Ok(claims)
}
fn retained(run: &Run, requests: &[port::Request]) -> Result<(), String> {
    if requests.len() > MAX_CASTS {
        return Err("too many ability acknowledgements".into());
    }
    for (i, r) in requests.iter().enumerate() {
        if !request(r) || requests[..i].contains(r) || !run.module_casts.contains(r) {
            return Err("unknown/duplicate ability acknowledgement".into());
        }
    }
    Ok(())
}
fn acknowledge(run: &mut Run, requests: &[port::Request]) {
    run.module_casts.retain(|r| !requests.contains(r));
}

pub fn apply(
    run: &mut Run,
    pending: Option<&mut frame::Pending>,
    scope: &str,
    event: Event,
) -> Result<Reply, String> {
    bounded(&event)?;
    if !text(scope, 128) {
        return Err("invalid ability session".into());
    }
    match event {
        Event::ReplaceSlots {
            fence,
            replacements,
        } => {
            if pending.is_some() {
                return Err("slot replacement inside ability frame".into());
            }
            fence.validate(run, scope)?;
            if replacements.len() > crate::rules::MAX_PLAYERS as usize {
                return Err("too many ability slot owners".into());
            }
            let mut seen = BTreeSet::new();
            for replacement in &replacements {
                let hero = run
                    .players
                    .get(&replacement.player)
                    .ok_or("missing ability slot owner")?;
                if !seen.insert(replacement.player)
                    || run.seat_epoch(replacement.player) != replacement.epoch
                    || hero.module_spells != replacement.observed
                    || replacement.slots.iter().flatten().any(|s| !slot(s))
                {
                    return Err("stale/invalid ability slots".into());
                }
            }
            let mut changed = false;
            for replacement in replacements {
                let hero = run.players.get_mut(&replacement.player).unwrap();
                changed |= hero.module_spells != replacement.slots;
                hero.module_spells = replacement.slots;
            }
            Ok(Reply::Changed(changed))
        }
        Event::Acknowledge { fence, requests } | Event::Cancel { fence, requests } => {
            if pending.is_some() {
                return Err("outbox mutation requires atomic frame batch".into());
            }
            fence.validate(run, scope)?;
            retained(run, &requests)?;
            acknowledge(run, &requests);
            Ok(Reply::Acknowledged(requests.len()))
        }
        Event::Batch(batch) => {
            let pending = pending.ok_or("ability batch requires an open frame")?;
            batch.fence.validate(run, scope)?;
            if batch.frame != pending.frame
                || pending.frame.tick != run.tick
                || pending.frame.seats != frame::seats(run)
                || !pending.frame.active
                || pending.abilities_applied
            {
                return Err("stale ability frame".into());
            }
            validate_active_casts(&pending.active_casts, run)?;
            if pending
                .active_casts
                .iter()
                .any(|cast| cast.session != scope)
                || batch.starts.len() > MAX_CASTS
                || batch.actions.len() > MAX_ACTIONS
            {
                return Err("ability batch exceeds owner bounds".into());
            }
            retained(run, &batch.acknowledge)?;
            let mut owners = pending.active_casts.clone();
            let mut starts = BTreeSet::new();
            let mut costs = BTreeMap::<u32, f32>::new();
            for start in &batch.starts {
                let start = &start.cast;
                let r = &start.request;
                if start.session != scope
                    || !start.started
                    || !proof(start, run)
                    || !starts.insert((r.player, r.slot))
                    || owners.iter().any(|cast| {
                        cast.origin == start.origin && (cast.started || cast.request != *r)
                    })
                {
                    return Err("invalid/duplicate ability start".into());
                }
                let native_pending = owners.iter().any(|cast| {
                    !cast.started
                        && cast.session == start.session
                        && cast.request == *r
                        && cast.origin == start.origin
                });
                if !native_pending && !run.module_casts.contains(r) {
                    return Err("ability start has no retained request".into());
                }
                let hero = &run.players[&r.player];
                if hero.hp == 0
                    || hero.module_spells[usize::from(r.slot)].as_ref() != Some(&r.definition)
                    || hero.spell_cooldowns[usize::from(r.slot)] > 1
                {
                    return Err("ability start slot changed or cooling down".into());
                }
                let cost = costs.entry(r.player).or_default();
                *cost += r.definition.mana;
                if !cost.is_finite()
                    || !hero.mana.is_finite()
                    || hero.mana < 0.0
                    || *cost > hero.mana
                {
                    return Err("aggregate ability mana exceeded".into());
                }
                if let Some(prior) = owners.iter_mut().find(|cast| cast.origin == start.origin) {
                    *prior = start.clone();
                } else {
                    owners.push(start.clone());
                }
            }
            validate_active_casts(&owners, run)?;
            for r in &batch.acknowledge {
                if !owners
                    .iter()
                    .any(|cast| cast.session == scope && cast.request == *r)
                {
                    return Err("ability ACK has no native admission owner".into());
                }
            }
            // Starting an outbox-owned request must retire that exact entry in
            // the same commit, so later frames cannot start it a second time.
            if batch.starts.iter().any(|cast| {
                run.module_casts.contains(&cast.cast.request)
                    && !batch.acknowledge.contains(&cast.cast.request)
            }) {
                return Err("ability start did not acknowledge its request".into());
            }
            let mut commands = Vec::with_capacity(batch.actions.len());
            for action in &batch.actions {
                if !action.cast.started
                    || action.cast.session != scope
                    || !owners.contains(&action.cast)
                {
                    return Err("ability command has no exact running owner".into());
                }
                action.command.validate()?;
                commands.push((
                    action.cast.request.player,
                    action.cast.origin.clone(),
                    action.command.clone(),
                ));
            }
            run.validate_ability_commands(&commands)?;
            // All ownership, commands, capacity and aggregate costs validated.
            for (player, cost) in costs {
                let hero = run.players.get_mut(&player).unwrap();
                hero.mana = (hero.mana - cost).max(0.0);
            }
            for start in &batch.starts {
                let r = &start.cast.request;
                run.players.get_mut(&r.player).unwrap().spell_cooldowns[usize::from(r.slot)] =
                    r.definition.cooldown;
                if start.default_sound {
                    run.sounds.push("spell_cast");
                }
            }
            run.apply_ability_commands(commands);
            acknowledge(run, &batch.acknowledge);
            pending.active_casts = owners;
            pending.abilities_applied = true;
            Ok(Reply::Applied {
                starts: batch.starts.len(),
                commands: batch.actions.len(),
                acknowledged: batch.acknowledge.len(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{Shot, runtime_state};
    fn slot(generation: u64, mana: f32) -> port::Slot {
        port::Slot {
            module: "module".into(),
            generation,
            ability: "spark".into(),
            name: "Spark".into(),
            mana,
            cooldown: 90,
            icon: String::new(),
        }
    }
    fn request(run: &Run, index: u8) -> port::Request {
        let definition = run.players[&1].module_spells[usize::from(index)]
            .clone()
            .unwrap();
        port::Request {
            raid: run.raid_id,
            epoch: run.seat_epoch(1),
            definition: definition.clone(),
            player: 1,
            slot: index,
            module: definition.module,
            generation: definition.generation,
            ability: definition.ability,
            tick: run.tick,
        }
    }
    fn setup() -> (Run, frame::Pending) {
        let mut run = Run::new(3, 9, None);
        run.tick = 8;
        let hero = run.players.get_mut(&1).unwrap();
        hero.mana = 20.0;
        hero.module_spells[0] = Some(slot(1, 6.0));
        hero.module_spells[1] = Some(slot(1, 6.0));
        hero.spell_cooldowns = [0; cards::SPELL_SLOTS];
        run.module_casts.push(request(&run, 0));
        let pending = frame::Pending {
            frame: frame::Fence {
                begin_seq: 1,
                network_tick: 3,
                tick: run.tick,
                active: true,
                seats: frame::seats(&run),
            },
            started_tick: run.tick,
            inputs: Default::default(),
            prelude: vec![],
            active_casts: vec![],
            abilities_applied: false,
            arrival: Default::default(),
        };
        (run, pending)
    }
    fn cast(request: port::Request, id: &str, started: bool) -> ActiveCast {
        ActiveCast {
            session: "ability-test".into(),
            origin: port::Origin {
                module: request.module.clone(),
                generation: request.generation,
                cast: id.into(),
            },
            request,
            started,
        }
    }
    fn start(cast: ActiveCast) -> Start {
        Start {
            cast,
            default_sound: false,
        }
    }
    fn batch(run: &Run, pending: &frame::Pending) -> Batch {
        Batch {
            fence: OwnerFence::capture(run, "ability-test"),
            frame: pending.frame.clone(),
            starts: vec![],
            actions: vec![],
            acknowledge: vec![],
        }
    }
    fn snapshot(run: &Run, pending: &frame::Pending) -> (serde_json::Value, serde_json::Value) {
        (
            runtime_state::capture(run).unwrap().into_value(),
            serde_json::to_value(pending).unwrap(),
        )
    }
    fn bolt() -> port::Command {
        port::Command::Projectile {
            offset: [0.0, 0.0],
            velocity: [10.0, 0.0],
            damage: 5,
            ttl: 60,
            pierce: 0,
            kind: Shot::Bolt,
        }
    }
    #[test]
    fn preview_and_rejected_batch_retain_exact_requests_costs_and_world_then_retry_commits() {
        let (mut run, mut pending) = setup();
        let first = claim(&run, "ability-test").unwrap();
        assert_eq!(claim(&run, "ability-test").unwrap(), first);
        let owner = cast(first.requests[0].clone(), "cast-one", true);
        let mut good = batch(&run, &pending);
        good.starts.push(Start {
            cast: owner.clone(),
            default_sound: true,
        });
        good.acknowledge = first.requests;
        good.actions.push(Action {
            cast: owner.clone(),
            command: bolt(),
        });
        let before = snapshot(&run, &pending);
        let mut bad = good.clone();
        bad.actions.push(Action {
            cast: owner.clone(),
            command: port::Command::Move {
                delta: [f32::NAN, 0.0],
            },
        });
        assert!(
            apply(
                &mut run,
                Some(&mut pending),
                "ability-test",
                Event::Batch(bad)
            )
            .is_err()
        );
        assert_eq!(snapshot(&run, &pending), before);
        assert_eq!(
            apply(
                &mut run,
                Some(&mut pending),
                "ability-test",
                Event::Batch(good.clone())
            )
            .unwrap(),
            Reply::Applied {
                starts: 1,
                commands: 1,
                acknowledged: 1
            }
        );
        assert!(run.module_casts.is_empty());
        assert_eq!(run.players[&1].mana, 14.0);
        assert_eq!(run.sounds.iter().filter(|&&s| s == "spell_cast").count(), 1);
        assert_eq!(run.players[&1].spell_cooldowns[0], 90);
        assert_eq!(
            run.projectiles
                .last()
                .unwrap()
                .traits
                .module_origin
                .as_ref(),
            Some(&owner.origin)
        );
        let before = snapshot(&run, &pending);
        assert!(
            apply(
                &mut run,
                Some(&mut pending),
                "ability-test",
                Event::Batch(good)
            )
            .is_err()
        );
        assert_eq!(snapshot(&run, &pending), before);
    }
    #[test]
    fn aggregate_costs_cooldowns_capacity_and_foreign_proofs_reject_all_groups() {
        let (mut run, mut pending) = setup();
        let other = request(&run, 1);
        run.module_casts.push(other.clone());
        let a = cast(run.module_casts[0].clone(), "one", true);
        let b = cast(other, "two", true);
        let mut input = batch(&run, &pending);
        input.starts = vec![start(a.clone()), start(b.clone())];
        input.acknowledge = run.module_casts.clone();
        input.actions = vec![
            Action {
                cast: a,
                command: bolt(),
            },
            Action {
                cast: b,
                command: port::Command::Heal { amount: 1 },
            },
        ];
        run.players.get_mut(&1).unwrap().mana = 10.0;
        let before = snapshot(&run, &pending);
        assert!(
            apply(
                &mut run,
                Some(&mut pending),
                "ability-test",
                Event::Batch(input.clone())
            )
            .is_err()
        );
        assert_eq!(snapshot(&run, &pending), before);
        run.players.get_mut(&1).unwrap().mana = 20.0;
        run.players.get_mut(&1).unwrap().spell_cooldowns[1] = 2;
        let before = snapshot(&run, &pending);
        assert!(
            apply(
                &mut run,
                Some(&mut pending),
                "ability-test",
                Event::Batch(input.clone())
            )
            .is_err()
        );
        assert_eq!(snapshot(&run, &pending), before);
        run.players.get_mut(&1).unwrap().spell_cooldowns[1] = 0;
        let mut foreign = input.clone();
        foreign.starts[1].cast.request.epoch += 1;
        let before = snapshot(&run, &pending);
        assert!(
            apply(
                &mut run,
                Some(&mut pending),
                "ability-test",
                Event::Batch(foreign)
            )
            .is_err()
        );
        assert_eq!(snapshot(&run, &pending), before);
        run.apply_ability_commands(vec![(1, input.starts[0].cast.origin.clone(), bolt())]);
        run.projectiles
            .resize(crate::rules::MAX_PROJECTILES, run.projectiles[0].clone());
        let before = snapshot(&run, &pending);
        assert!(
            apply(
                &mut run,
                Some(&mut pending),
                "ability-test",
                Event::Batch(input)
            )
            .is_err()
        );
        assert_eq!(snapshot(&run, &pending), before);
    }
    #[test]
    fn old_generation_running_owner_survives_surface_swap_but_old_pending_start_refuses() {
        let (mut run, mut pending) = setup();
        let old = cast(run.module_casts[0].clone(), "retained", true);
        let fence = OwnerFence::capture(&run, "ability-test");
        let observed = run.players[&1].module_spells.clone();
        let mut replacement = observed.clone();
        replacement[0] = Some(slot(2, 10.0));
        let epoch = run.seat_epoch(1);
        apply(
            &mut run,
            None,
            "ability-test",
            Event::ReplaceSlots {
                fence,
                replacements: vec![Replacement {
                    player: 1,
                    epoch,
                    observed,
                    slots: replacement,
                }],
            },
        )
        .unwrap();
        assert_eq!(run.module_casts, vec![old.request.clone()]);
        pending.active_casts.push(old.clone());
        let mut ongoing = batch(&run, &pending);
        ongoing.actions.push(Action {
            cast: old.clone(),
            command: bolt(),
        });
        apply(
            &mut run,
            Some(&mut pending),
            "ability-test",
            Event::Batch(ongoing),
        )
        .unwrap();
        assert_eq!(run.players[&1].mana, 20.0);
        assert_eq!(
            run.projectiles
                .last()
                .unwrap()
                .traits
                .module_origin
                .as_ref(),
            Some(&old.origin)
        );
        let mut replayed = batch(&run, &pending);
        replayed.actions.push(Action {
            cast: old.clone(),
            command: bolt(),
        });
        let before = snapshot(&run, &pending);
        assert!(
            apply(
                &mut run,
                Some(&mut pending),
                "ability-test",
                Event::Batch(replayed)
            )
            .is_err()
        );
        assert_eq!(snapshot(&run, &pending), before);
        pending.active_casts.clear();
        pending.abilities_applied = false;
        let mut start = batch(&run, &pending);
        start.starts.push(Start {
            cast: old.clone(),
            default_sound: false,
        });
        start.acknowledge.push(old.request.clone());
        let before = snapshot(&run, &pending);
        assert!(
            apply(
                &mut run,
                Some(&mut pending),
                "ability-test",
                Event::Batch(start)
            )
            .is_err()
        );
        assert_eq!(snapshot(&run, &pending), before);
        // Explicit cancellation retires the stale request without touching an
        // issued projectile or its old-generation origin.
        let fence = OwnerFence::capture(&run, "ability-test");
        apply(
            &mut run,
            None,
            "ability-test",
            Event::Cancel {
                fence,
                requests: vec![old.request],
            },
        )
        .unwrap();
        assert!(run.module_casts.is_empty());
        assert!(!run.projectiles.is_empty());
    }
    #[test]
    fn native_pending_handoff_proves_later_start_after_transport_ack() {
        let (mut run, mut pending) = setup();
        let admitted = cast(run.module_casts[0].clone(), "pending", false);
        let fence = OwnerFence::capture(&run, "ability-test");
        apply(
            &mut run,
            None,
            "ability-test",
            Event::Acknowledge {
                fence,
                requests: vec![admitted.request.clone()],
            },
        )
        .unwrap();
        pending.active_casts.push(admitted.clone());
        let mut start = admitted;
        start.started = true;
        let mut input = batch(&run, &pending);
        input.starts.push(Start {
            cast: start.clone(),
            default_sound: false,
        });
        input.actions.push(Action {
            cast: start,
            command: bolt(),
        });
        apply(
            &mut run,
            Some(&mut pending),
            "ability-test",
            Event::Batch(input),
        )
        .unwrap();
        assert_eq!(run.players[&1].mana, 14.0);
        assert!(pending.active_casts[0].started);
        let restored = runtime_state::restore(runtime_state::capture(&run).unwrap()).unwrap();
        let retained: frame::Pending =
            serde_json::from_value(serde_json::to_value(&pending).unwrap()).unwrap();
        validate_active_casts(&retained.active_casts, &restored).unwrap();
        let mut wrong = retained.active_casts.clone();
        wrong[0].request.raid += 1;
        assert!(validate_active_casts(&wrong, &restored).is_err());
    }
    #[test]
    fn slot_batch_validates_every_observation_before_replacing_any_owner() {
        let (mut run, mut pending) = setup();
        run.join_seat(2, "second");
        pending.frame.seats = frame::seats(&run);
        let fence = OwnerFence::capture(&run, "ability-test");
        let mut first = run.players[&1].module_spells.clone();
        first[0] = Some(slot(2, 8.0));
        let replacements = vec![
            Replacement {
                player: 1,
                epoch: run.seat_epoch(1),
                observed: run.players[&1].module_spells.clone(),
                slots: first,
            },
            Replacement {
                player: 2,
                epoch: run.seat_epoch(2) + 1,
                observed: run.players[&2].module_spells.clone(),
                slots: Default::default(),
            },
        ];
        let before = snapshot(&run, &pending);
        assert!(
            apply(
                &mut run,
                None,
                "ability-test",
                Event::ReplaceSlots {
                    fence,
                    replacements
                }
            )
            .is_err()
        );
        assert_eq!(snapshot(&run, &pending), before);
        let mut oversized = batch(&run, &pending);
        let owner = cast(run.module_casts[0].clone(), "large", true);
        oversized.starts.push(Start {
            cast: owner.clone(),
            default_sound: true,
        });
        oversized.acknowledge = run.module_casts.clone();
        oversized.actions = vec![
            Action {
                cast: owner,
                command: bolt()
            };
            MAX_ACTIONS + 1
        ];
        assert!(
            apply(
                &mut run,
                Some(&mut pending),
                "ability-test",
                Event::Batch(oversized)
            )
            .is_err()
        );
        assert_eq!(snapshot(&run, &pending), before);
    }

    #[test]
    fn paid_cast_continues_after_death_but_dead_pending_start_rejects_atomically() {
        let (mut run, mut pending) = setup();
        let paid = cast(run.module_casts[0].clone(), "paid-before-death", true);
        run.players.get_mut(&1).unwrap().hp = 0;
        pending.active_casts.push(paid.clone());
        validate_active_casts(&pending.active_casts, &run).unwrap();
        let mut ongoing = batch(&run, &pending);
        ongoing.actions.push(Action {
            cast: paid.clone(),
            command: bolt(),
        });
        apply(
            &mut run,
            Some(&mut pending),
            "ability-test",
            Event::Batch(ongoing),
        )
        .unwrap();
        assert_eq!(run.players[&1].mana, 20.0);
        assert_eq!(
            run.projectiles
                .last()
                .unwrap()
                .traits
                .module_origin
                .as_ref(),
            Some(&paid.origin)
        );

        // A later frame can retain a native pending source for cleanup, but
        // neither that source nor an outbox entry authorizes a dead start.
        pending.abilities_applied = false;
        let mut waiting = paid.clone();
        waiting.started = false;
        waiting.origin.cast = "pending-before-death".into();
        pending.active_casts = vec![waiting.clone()];
        validate_active_casts(&pending.active_casts, &run).unwrap();
        let mut starting = waiting;
        starting.started = true;
        let mut input = batch(&run, &pending);
        input.starts.push(Start {
            cast: starting.clone(),
            default_sound: true,
        });
        input.acknowledge.push(starting.request.clone());
        input.actions.push(Action {
            cast: starting,
            command: bolt(),
        });
        let before = snapshot(&run, &pending);
        assert!(
            apply(
                &mut run,
                Some(&mut pending),
                "ability-test",
                Event::Batch(input)
            )
            .is_err()
        );
        assert_eq!(snapshot(&run, &pending), before);
    }

    #[test]
    fn current_frame_tick_seat_and_original_session_are_mandatory() {
        let (mut run, mut pending) = setup();
        let running = cast(run.module_casts[0].clone(), "running", true);
        pending.active_casts.push(running.clone());
        let mut input = batch(&run, &pending);
        input.actions.push(Action {
            cast: running,
            command: bolt(),
        });
        for mode in 0..3 {
            let mut candidate = input.clone();
            match mode {
                0 => candidate.fence.tick += 1,
                1 => candidate
                    .frame
                    .seats
                    .insert(1, 999)
                    .map(|_| ())
                    .unwrap_or(()),
                _ => candidate.actions[0].cast.session = "different-session".into(),
            }
            let before = snapshot(&run, &pending);
            assert!(
                apply(
                    &mut run,
                    Some(&mut pending),
                    "ability-test",
                    Event::Batch(candidate)
                )
                .is_err()
            );
            assert_eq!(snapshot(&run, &pending), before);
        }
    }
}

#[cfg(test)]
mod transport_tests {
    use super::*;
    use crate::rules::session::{self, Envelope};

    #[test]
    fn ability_events_roundtrip_json_value_and_text_in_real_session_envelopes() {
        let mut run = Run::new(3, 9, None);
        let definition = port::Slot {
            module: "transport".into(),
            generation: 1,
            ability: "spark".into(),
            name: "Spark".into(),
            mana: 2.0,
            cooldown: 13,
            icon: "S".into(),
        };
        run.players.get_mut(&1).unwrap().module_spells[0] = Some(definition.clone());
        let request = port::Request {
            raid: run.raid_id,
            epoch: run.seat_epoch(1),
            definition,
            player: 1,
            slot: 0,
            module: "transport".into(),
            generation: 1,
            ability: "spark".into(),
            tick: run.tick,
        };
        run.module_casts.push(request.clone());
        let owner = ActiveCast {
            session: "transport-session".into(),
            request: request.clone(),
            origin: port::Origin {
                module: "transport".into(),
                generation: 1,
                cast: "cast-one".into(),
            },
            started: true,
        };
        let fence = OwnerFence::capture(&run, "transport-session");
        let frame = frame::Fence {
            begin_seq: 1,
            network_tick: 3,
            tick: run.tick,
            active: true,
            seats: frame::seats(&run),
        };
        let events = vec![
            Event::ReplaceSlots {
                fence: fence.clone(),
                replacements: vec![Replacement {
                    player: 1,
                    epoch: run.seat_epoch(1),
                    observed: run.players[&1].module_spells.clone(),
                    slots: run.players[&1].module_spells.clone(),
                }],
            },
            Event::Acknowledge {
                fence: fence.clone(),
                requests: vec![request.clone()],
            },
            Event::Cancel {
                fence: fence.clone(),
                requests: vec![request.clone()],
            },
            Event::Batch(Batch {
                fence,
                frame: frame.clone(),
                starts: vec![Start {
                    cast: owner.clone(),
                    default_sound: true,
                }],
                actions: vec![Action {
                    cast: owner,
                    command: port::Command::Heal { amount: 1 },
                }],
                acknowledge: vec![request],
            }),
        ];
        for event in events {
            let envelope = Envelope {
                api: session::API,
                session: "transport-session".into(),
                raid: run.raid_id,
                seq: 2,
                expected_view_revision: 2,
                event: session::Event::Ability(event),
            };
            let value = serde_json::to_value(&envelope).unwrap();
            let from_value: Envelope = serde_json::from_value(value).unwrap();
            let from_text: Envelope =
                serde_json::from_str(&serde_json::to_string(&envelope).unwrap()).unwrap();
            assert_eq!(from_value, envelope);
            assert_eq!(from_text, envelope);
            let session::Event::Ability(event) = from_value.event else {
                unreachable!()
            };
            let mut candidate = run.clone();
            let mut pending = frame::Pending {
                frame: frame.clone(),
                started_tick: run.tick,
                inputs: Default::default(),
                prelude: vec![],
                active_casts: vec![],
                abilities_applied: false,
                arrival: Default::default(),
            };
            if matches!(event, Event::Batch(_)) {
                apply(
                    &mut candidate,
                    Some(&mut pending),
                    "transport-session",
                    event,
                )
                .unwrap();
                assert!(pending.abilities_applied);
                let context = session::Context {
                    settings: Default::default(),
                    phrasebook: (crate::rules::phrasebook::TEXT.into(), vec![]),
                    wishes: candidate.kingdom.wishes.clone(),
                };
                let journal = crate::rules::host_commands::Journal::new(
                    "transport-session".into(),
                    candidate.raid_id,
                );
                let saved = session::checkpoint::capture_with_frame(
                    &candidate,
                    &journal,
                    &context,
                    2,
                    3,
                    Some(&pending),
                )
                .unwrap();
                let restored: session::checkpoint::SessionCheckpoint =
                    serde_json::from_value(serde_json::to_value(&saved).unwrap()).unwrap();
                assert_eq!(restored.pending_frame, saved.pending_frame);
                assert_eq!(
                    session::checkpoint::decode(&saved.encode().unwrap())
                        .unwrap()
                        .pending_frame,
                    saved.pending_frame
                );
            } else {
                apply(&mut candidate, None, "transport-session", event).unwrap();
            }
        }
    }
}
