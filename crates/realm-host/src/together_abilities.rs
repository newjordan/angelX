//! Executable ability instances. The rules own input admission and physics;
//! this host adapter owns cast identities, executable generations and commits.
use angel_delve::rules::{
    Run,
    abilities::{self, Command, Origin, Request},
};
use angel_live::kernel::{Binding, Candidate, Instance, Resource};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub const CONTRACT: &str = "game-abilities/1";
const MAX_CASTS: usize = 16;
const MAX_GENERATIONS: usize = 8;
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    name: String,
    mana: f32,
    cooldown: u32,
    icon: String,
    #[serde(default)]
    pub parameters: Value,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Loadout {
    pub player: u32,
    pub slots: [Option<String>; 4],
    pub abilities: BTreeMap<String, Definition>,
    #[serde(default)]
    cancel: u64,
    #[serde(default)]
    sounds: BTreeMap<String, super::together_media::SoundSpec>,
    #[serde(default)]
    banks: BTreeMap<String, BTreeMap<String, super::together_media::SoundSpec>>,
    #[serde(default)]
    sound_bank: Option<String>,
    #[serde(skip)]
    pub media: Arc<BTreeMap<String, super::together_media::Voice>>,
    #[serde(skip)]
    prepared_banks: BTreeMap<String, Arc<BTreeMap<String, super::together_media::Voice>>>,
    #[serde(default)]
    visuals: BTreeMap<String, super::together_visuals::Specification>,
    #[serde(default)]
    effect_banks: BTreeMap<String, BTreeMap<String, super::together_visuals::Specification>>,
    #[serde(default)]
    effect_bank: Option<String>,
    #[serde(skip)]
    pub vfx: Arc<super::together_visuals::Bank>,
    #[serde(skip)]
    prepared_effects: BTreeMap<String, Arc<super::together_visuals::Bank>>,
    #[serde(skip)]
    pub executables: BTreeMap<String, Executable>,
    #[serde(skip)]
    pub appearance: Option<super::together_appearance::Appearance>,
}
#[derive(Clone, Debug)]
pub struct Executable {
    pub resource: Resource,
    pub ability: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Action {
    cast: String,
    command: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Batch {
    #[serde(default)]
    actions: Vec<Action>,
    #[serde(default)]
    finished: Vec<String>,
}
struct PreparedBatch {
    commands: Vec<(u32, Origin, Command)>,
    voices: Vec<super::together_media::Voice>,
    visuals: Vec<super::together_visuals::Emission>,
    finished: Vec<String>,
}
fn batch(value: &Value, casts: &BTreeMap<String, Cast>) -> Result<PreparedBatch, String> {
    let batch: Batch =
        serde_json::from_value(value.clone()).map_err(|e| format!("ability output: {e}"))?;
    if batch.actions.len() > 64 || batch.finished.len() > MAX_CASTS {
        return Err("ability batch exceeds capacity".into());
    }
    let mut finished = BTreeSet::new();
    for id in &batch.finished {
        if !casts.contains_key(id) || !finished.insert(id) {
            return Err("ability finished an unknown/duplicate cast".into());
        }
    }
    let mut commands = Vec::new();
    let mut voices = Vec::new();
    let mut visuals = Vec::new();
    for action in batch.actions {
        let cast = casts
            .get(&action.cast)
            .ok_or("ability action has no owning cast")?;
        if action.command["type"] == "sound" || action.command["type"] == "visual" {
            #[derive(Deserialize)]
            #[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
            enum Media {
                Sound {
                    cue: String,
                },
                Visual {
                    cue: String,
                    #[serde(default)]
                    offset: [f32; 2],
                    #[serde(default)]
                    follow: bool,
                },
            }
            match serde_json::from_value(action.command)
                .map_err(|e| format!("presentation action: {e}"))?
            {
                Media::Sound { cue } => voices.push(
                    cast.media
                        .get(&cue)
                        .ok_or("sound cue is absent from the owning generation")?
                        .clone(),
                ),
                Media::Visual {
                    cue,
                    offset,
                    follow,
                } => {
                    if !offset.iter().all(|v| v.is_finite() && v.abs() <= 32.0) {
                        return Err("invalid effect offset".into());
                    }
                    let prepared = cast
                        .vfx
                        .get(&cue)
                        .ok_or("effect cue is absent from the owning generation")?;
                    if prepared
                        .graph
                        .nodes
                        .iter()
                        .any(|node| node.keys.iter().any(|key| key.opacity > 0))
                    {
                        visuals.push(super::together_visuals::Emission {
                            player: cast.player,
                            origin: Origin {
                                module: cast.source.module.clone(),
                                generation: cast.source.generation,
                                cast: action.cast,
                            },
                            prepared: Arc::clone(prepared),
                            offset,
                            follow,
                            resource: cast.resource.clone(),
                        });
                    }
                }
            }
            continue;
        }
        let command: Command =
            serde_json::from_value(action.command).map_err(|e| format!("game action: {e}"))?;
        command.validate()?;
        commands.push((
            cast.player,
            Origin {
                module: cast.source.module.clone(),
                generation: cast.source.generation,
                cast: action.cast,
            },
            command,
        ));
    }
    Ok(PreparedBatch {
        commands,
        voices,
        visuals,
        finished: batch.finished,
    })
}
pub fn id_valid(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
}
fn location(run: &Run) -> Value {
    json!([run.raid_id, run.dungeon.depth, run.at])
}
fn player_context(run: &Run) -> Value {
    json!(run.players.iter().map(|(id,hero)|(id.to_string(),json!({"position":[hero.x,hero.y],"aim":[hero.aim_x,hero.aim_y],"hp":hero.hp,"mana":hero.mana}))).collect::<BTreeMap<_,_>>())
}
fn start(id: &str, cast: &Cast) -> Value {
    json!({"cast":id,"ability":cast.ability,"player":cast.player,"slot":cast.slot,"parameters":cast.definition.parameters})
}

pub fn bind(
    name: &str,
    value: &Value,
    assets: &BTreeMap<String, Arc<angel_live::Asset>>,
    factory: &Candidate,
) -> Result<Binding, String> {
    let mut loadout: Loadout = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
    if !(1..=4).contains(&loadout.player)
        || name != format!("ability-seat-{}", loadout.player)
        || loadout.abilities.len() > 16
    {
        return Err("invalid ability resource owner/catalogue".into());
    }
    validate_definitions(&loadout.abilities)?;
    if loadout
        .slots
        .iter()
        .flatten()
        .any(|id| !loadout.abilities.contains_key(id))
    {
        return Err("ability slot names a missing definition".into());
    }
    if loadout.banks.is_empty() {
        if loadout.sound_bank.is_some() {
            return Err("selected sound bank is missing".into());
        }
        loadout.media = Arc::new(super::together_media::prepare(&loadout.sounds, assets)?);
    } else {
        if loadout.banks.len() > 8 || !loadout.sounds.is_empty() {
            return Err(
                "sound catalogue: at most eight banks; choose catalogue or direct cues".into(),
            );
        }
        for (name, specs) in &loadout.banks {
            if !id_valid(name) {
                return Err("invalid sound bank name".into());
            }
            loadout.prepared_banks.insert(
                name.clone(),
                Arc::new(super::together_media::prepare(specs, assets)?),
            );
        }
        loadout.media = Arc::clone(
            loadout
                .sound_bank
                .as_ref()
                .and_then(|name| loadout.prepared_banks.get(name))
                .ok_or("selected sound bank is missing")?,
        );
    }
    if loadout.effect_banks.is_empty() {
        if loadout.effect_bank.is_some() {
            return Err("selected effect bank is missing".into());
        }
        loadout.vfx = Arc::new(super::together_visuals::prepare(
            std::mem::take(&mut loadout.visuals),
            assets,
        )?);
    } else {
        if loadout.effect_banks.len() > 8 || !loadout.visuals.is_empty() {
            return Err(
                "effect catalogue: at most eight banks; choose catalogue or direct cues".into(),
            );
        }
        let mut prepared = BTreeMap::new();
        for (name, specs) in std::mem::take(&mut loadout.effect_banks) {
            if !id_valid(&name) {
                return Err("invalid effect bank name".into());
            }
            let bank = super::together_visuals::prepare(specs, assets)?;
            prepared.extend(
                bank.values()
                    .map(|graph| (graph.id.clone(), Arc::clone(&graph.graph))),
            );
            if prepared
                .values()
                .map(|graph| graph.pixel_count())
                .sum::<usize>()
                > angel_delve::realm::effects::MAX_PIXELS
            {
                return Err("effect catalogue pixel budget exceeded".into());
            }
            loadout.prepared_effects.insert(name, Arc::new(bank));
        }
        loadout.vfx = Arc::clone(
            loadout
                .effect_bank
                .as_ref()
                .and_then(|name| loadout.prepared_effects.get(name))
                .ok_or("selected effect bank is missing")?,
        );
    }
    probe(&loadout, |_| {
        factory.probe_instance(&json!({"kind":"abilities"}))
    })?;
    Ok(Arc::new(loadout))
}

pub fn validate_definitions(
    definitions: &BTreeMap<String, Definition>,
) -> Result<(), String> {
    if definitions.len() > 16 {
        return Err("ability catalogue exceeds sixteen definitions".into());
    }
    for (id, definition) in definitions {
        if !id_valid(id)
            || definition.name.is_empty()
            || definition.name.len() > 48
            || !definition.mana.is_finite()
            || !(0.0..=1000.0).contains(&definition.mana)
            || !(1..=3600).contains(&definition.cooldown)
            || definition.parameters.to_string().len() > 4096
            || definition.icon.split('/').count() > 12
            || definition.icon.split('/').any(|row| {
                row.len() > 12
                    || !row
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b".@$".contains(&c))
            })
        {
            return Err("invalid ability definition".into());
        }
    }
    Ok(())
}

pub fn probe(
    loadout: &Loadout,
    instantiate: impl Fn(&str) -> Result<Instance, String>,
) -> Result<(), String> {
    // Probe every executable in this loadout against the actual game-port schema,
    // before the kernel publishes code, state and resources together.
    let mut run = Run::new(1, 1, Some("Two"));
    run.join_seat(3, "Three");
    run.join_seat(4, "Four");
    for ability in loadout.abilities.keys() {
        let mut instance = instantiate(ability)?;
        let source = Source {
            module: "probe".into(),
            generation: 0,
        };
        let cast = Cast {
            source,
            player: loadout.player,
            slot: 0,
            ability: loadout
                .executables
                .get(ability)
                .map(|code| code.ability.clone())
                .unwrap_or_else(|| ability.clone()),
            definition: loadout.abilities[ability].clone(),
            media: Arc::clone(&loadout.media),
            vfx: Arc::clone(&loadout.vfx),
            resource: None,
        };
        let starts = vec![start("probe", &cast)];
        let casts = BTreeMap::from([("probe".into(), cast)]);
        let staged=instance.stage(&json!({"type":"tick","tick":0,"starts":starts,"cancelled":[],"location":location(&run),"players":player_context(&run)}))?;
        let output = batch(&staged.output, &casts)?;
        run.validate_ability_commands(&output.commands)?;
        instance.commit(staged)?;
    }
    Ok(())
}
// Cast ownership is deliberately independent of registry admission. A pending
// or running cast leases the exact source even after a newer source is published.
#[derive(Clone)]
struct Source {
    module: String,
    generation: u64,
}
#[derive(Clone)]
pub struct Cast {
    source: Source,
    player: u32,
    slot: u8,
    ability: String,
    pub definition: Definition,
    media: Arc<BTreeMap<String, super::together_media::Voice>>,
    vfx: Arc<super::together_visuals::Bank>,
    resource: Option<Resource>,
}
pub struct Pending {
    resource: Resource,
    cast: Cast,
}
pub struct Group {
    _resource: Resource,
    pub instance: Instance,
    pub casts: BTreeMap<String, Cast>,
    cancelled: Vec<String>,
    pub fault: Option<String>,
}
// The cockpit's tests of the live code run the scripts (V8 lives there) and read
// the casts' bookkeeping, so `sources`, `groups`, `pending`, `started` and
// `effects` are open.
#[derive(Default)]
pub struct Runtime {
    pub media: super::together_media::Media,
    pub visuals: super::together_visuals::Runtime,
    pub sources: BTreeMap<u32, Resource>,
    pub groups: BTreeMap<(String, u64), Group>,
    pub pending: BTreeMap<String, Pending>,
    cancelled: BTreeMap<(String, u32), u64>,
    last: Option<(u64, u64)>,
    init_faults: BTreeMap<(String, u64), String>,
    next: u64,
    pub started: u64,
    pub effects: u64,
}
impl Runtime {
    pub fn set_sources(&mut self, sources: Vec<Resource>, run: &mut Run) {
        self.sources = sources
            .into_iter()
            .filter_map(|resource| {
                resource
                    .binding::<Loadout>()
                    .map(|loadout| loadout.player)
                    .map(|player| (player, resource))
            })
            .collect();
        for resource in self.sources.values() {
            let loadout = resource.binding::<Loadout>().unwrap();
            if let Some(appearance) = &loadout.appearance {
                appearance.install(run);
            }
            let key = (resource.module.clone(), loadout.player);
            let last = self.cancelled.entry(key).or_default();
            if loadout.cancel > *last {
                self.visuals.cancel(loadout.player, run);
                self.pending
                    .retain(|_, pending| pending.cast.player != loadout.player);
                for group in self.groups.values_mut() {
                    let ids = group
                        .casts
                        .iter()
                        .filter_map(|(id, cast)| {
                            (cast.player == loadout.player).then_some(id.clone())
                        })
                        .collect::<Vec<_>>();
                    for id in ids {
                        group.casts.remove(&id);
                        group.cancelled.push(id);
                    }
                }
                *last = loadout.cancel;
            }
        }
        self.retire();
        self.install(run);
    }
    fn retire(&mut self) {
        // An uncommitted failed start is still owned by its quarantined
        // generation. Unrelated source/selection changes must not retry it.
        let pending = &self.pending;
        let owns_pending = |key: &(String, u64)| {
            pending.values().any(|pending| {
                pending.resource.module == key.0 && pending.resource.generation == key.1
            })
        };
        self.groups
            .retain(|key, group| !group.casts.is_empty() || owns_pending(key));
        self.init_faults.retain(|key, _| owns_pending(key));
    }
    fn install(&self, run: &mut Run) {
        for (player, hero) in &mut run.players {
            hero.module_spells = self
                .sources
                .get(player)
                .map(|source| {
                    let loadout = source.binding::<Loadout>().unwrap();
                    std::array::from_fn(|slot| {
                        loadout.slots[slot].as_ref().map(|id| {
                            let definition = &loadout.abilities[id];
                            abilities::Slot {
                                module: source.module.clone(),
                                generation: source.generation,
                                ability: id.clone(),
                                name: definition.name.clone(),
                                mana: definition.mana,
                                cooldown: definition.cooldown,
                                icon: definition.icon.clone(),
                            }
                        })
                    })
                })
                .unwrap_or_default();
        }
    }
    pub fn before_step(&mut self, run: &mut Run) {
        self.visuals.advance(run);
        self.media.retire();
        if self.last.is_some_and(|(raid, _)| raid != run.raid_id) {
            self.groups.clear();
            self.pending.clear();
            self.init_faults.clear();
            self.last = None;
            self.media.reset();
        }
        for group in self.groups.values_mut() {
            let missing = group
                .casts
                .iter()
                .filter_map(|(id, cast)| {
                    (!run.players.contains_key(&cast.player)).then_some(id.clone())
                })
                .collect::<Vec<_>>();
            for id in missing {
                group.casts.remove(&id);
                group.cancelled.push(id);
            }
        }
        self.retire();
        self.install(run);
    }
    pub fn after_step(&mut self, run: &mut Run) {
        self.visuals.advance(run);
        let now = (run.raid_id, run.tick);
        if self.last == Some(now) {
            return;
        }
        self.last = Some(now);
        self.pending.retain(|_, pending| {
            run.players
                .get(&pending.cast.player)
                .is_some_and(|hero| hero.hp > 0)
        });
        for request in std::mem::take(&mut run.module_casts) {
            self.admit(request, run);
        }
        let mut starts: BTreeMap<(String, u64), Vec<String>> = BTreeMap::new();
        let mut available = run
            .players
            .iter()
            .map(|(id, hero)| (*id, hero.mana))
            .collect::<BTreeMap<_, _>>();
        for (id, pending) in &self.pending {
            let key = (pending.resource.module.clone(), pending.resource.generation);
            if self.init_faults.contains_key(&key)
                || self
                    .groups
                    .get(&key)
                    .is_some_and(|group| group.fault.is_some())
            {
                continue;
            }
            let Some(mana) = available.get_mut(&pending.cast.player) else {
                continue;
            };
            if *mana < pending.cast.definition.mana {
                continue;
            }
            *mana -= pending.cast.definition.mana;
            starts.entry(key).or_default().push(id.clone());
        }
        for key in starts.keys() {
            if self.groups.contains_key(key)
                || self.init_faults.contains_key(key)
                || self.groups.len() >= MAX_GENERATIONS
            {
                continue;
            }
            let resource = self.pending[&starts[key][0]].resource.clone();
            match resource.instantiate(&json!({"kind":"abilities"})) {
                Ok(instance) => {
                    self.groups.insert(
                        key.clone(),
                        Group {
                            _resource: resource,
                            instance,
                            casts: BTreeMap::new(),
                            cancelled: vec![],
                            fault: None,
                        },
                    );
                }
                Err(error) => {
                    self.init_faults.insert(key.clone(), error);
                }
            }
        }
        for (key, group) in &mut self.groups {
            if group.fault.is_some() {
                continue;
            }
            let ids = starts.get(key).cloned().unwrap_or_default();
            let mut casts = group.casts.clone();
            let added = ids
                .iter()
                .map(|id| (id.clone(), self.pending[id].cast.clone()))
                .collect::<BTreeMap<_, _>>();
            casts.extend(added.clone());
            if casts.is_empty() {
                continue;
            }
            let event = json!({"type":"tick","tick":run.tick,"starts":added.iter().map(|(id,cast)|start(id,cast)).collect::<Vec<_>>(),"cancelled":group.cancelled,"location":location(run),"players":player_context(run)});
            let result = (|| {
                let staged = group.instance.stage(&event)?;
                let PreparedBatch {
                    commands,
                    voices,
                    visuals,
                    finished,
                } = batch(&staged.output, &casts)?;
                if run.projectiles.len()
                    + commands
                        .iter()
                        .filter(|(_, _, command)| matches!(command, Command::Projectile { .. }))
                        .count()
                    > angel_delve::rules::MAX_PROJECTILES
                {
                    return Ok(false);
                }
                run.validate_ability_commands(&commands)?;
                // Stage all costs before committing either actor or game data.
                let mut costs = BTreeMap::<u32, f32>::new();
                for cast in added.values() {
                    *costs.entry(cast.player).or_default() += cast.definition.mana;
                }
                if costs
                    .iter()
                    .any(|(player, cost)| run.players[player].mana < *cost)
                {
                    return Ok(false);
                }
                let Some(visuals) = self.visuals.stage(run, visuals)? else {
                    return Ok(false);
                };
                match self.media.admit(&voices) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        return Ok(false);
                    }
                    Err(error) => return Err(error.to_string()),
                }
                group.instance.commit(staged)?;
                for (id, cast) in &added {
                    let hero = run.players.get_mut(&cast.player).unwrap();
                    hero.mana = (hero.mana - cast.definition.mana).max(0.0);
                    hero.spell_cooldowns[usize::from(cast.slot)] = cast.definition.cooldown;
                    self.pending.remove(id);
                    self.started += 1;
                    if cast.media.is_empty() {
                        run.sounds.push("spell_cast");
                    }
                }
                self.effects += commands.len() as u64;
                run.apply_ability_commands(commands);
                self.media.commit(voices);
                self.visuals.commit(visuals, run);
                for id in finished {
                    casts.remove(&id);
                }
                group.casts = casts;
                group.cancelled.clear();
                Ok::<bool, String>(true)
            })();
            if let Err(error) = result {
                group.fault = Some(error);
            }
        }
        self.retire();
    }
    fn admit(&mut self, request: Request, run: &Run) {
        if self.pending.len()
            + self
                .groups
                .values()
                .map(|group| group.casts.len())
                .sum::<usize>()
            >= MAX_CASTS
            || self.pending.values().any(|pending| {
                pending.cast.player == request.player
                    && pending.cast.slot == request.slot
                    && pending.cast.source.module == request.module
                    && pending.cast.source.generation == request.generation
            })
        {
            return;
        }
        let Some(source) = self.sources.get(&request.player).filter(|source| {
            source.module == request.module && source.generation == request.generation
        }) else {
            return;
        };
        let loadout = source.binding::<Loadout>().unwrap();
        if loadout
            .slots
            .get(usize::from(request.slot))
            .and_then(Option::as_deref)
            != Some(request.ability.as_str())
            // Turbo can advance twice in one rules request. Coalesce its
            // duplicate slot admission, while rejecting older queued input.
            || request.tick > run.tick || run.tick - request.tick > 1
            || !run
                .players
                .get(&request.player)
                .is_some_and(|hero| hero.hp > 0)
        {
            return;
        }
        let Some(next) = self.next.checked_add(1) else {
            return;
        };
        self.next = next;
        let executable = loadout.executables.get(&request.ability);
        let cast = Cast {
            source: Source {
                module: source.module.clone(),
                generation: source.generation,
            },
            player: request.player,
            slot: request.slot,
            ability: executable
                .map(|code| code.ability.clone())
                .unwrap_or_else(|| request.ability.clone()),
            definition: loadout.abilities[&request.ability].clone(),
            media: Arc::clone(&loadout.media),
            vfx: Arc::clone(&loadout.vfx),
            resource: Some(source.clone()),
        };
        self.pending.insert(
            format!("{}-{}", run.raid_id, next),
            Pending {
                resource: executable
                    .map(|code| code.resource.clone())
                    .unwrap_or_else(|| source.clone()),
                cast,
            },
        );
    }
    pub fn status(&self) -> String {
        let active = self
            .groups
            .values()
            .map(|group| group.casts.len())
            .sum::<usize>();
        let faults = self
            .groups
            .values()
            .filter(|group| group.fault.is_some())
            .count()
            + self.init_faults.len();
        let mut status = format!(
            "abilities: active={active} pending={} generations={} started={} effects={} faults={faults}",
            self.pending.len(),
            self.groups.len(),
            self.started,
            self.effects
        );
        status.push_str(&format!(" sounds={}", self.media.emitted));
        status.push_str(&format!(
            " vfx={} vfx-active={} vfx-generations={}",
            self.visuals.emitted,
            self.visuals.active(),
            self.visuals.generations()
        ));
        for (key, error) in self
            .groups
            .iter()
            .filter_map(|(key, group)| group.fault.as_ref().map(|error| (key, error)))
            .chain(self.init_faults.iter())
        {
            status.push_str(&format!(
                "\n  {} generation {}: {}",
                key.0,
                key.1,
                error.chars().take(512).collect::<String>()
            ));
        }
        status
    }
}
