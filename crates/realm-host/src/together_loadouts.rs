//! Prepare independent character catalogues and resolve player selections.
//! A resolved resource owns its complete dependency closure; execution still
//! uses the selected provider's factory, never the resolver implementation.
use angel_live::{
    Asset,
    kernel::{Binding, Candidate, Resource, Resources},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};
use super::{
    together_abilities as abilities, together_appearance as appearance, together_media as media,
    together_visuals as visuals,
};

pub const CONTRACT: &str = "character-loadout/1";
pub const CODE: &str = "ability-pack/1";
pub const ART: &str = "character-art-pack/1";
pub const SOUND: &str = "sound-pack/1";
pub const EFFECT: &str = "effect-pack/1";

struct Code(BTreeMap<String, abilities::Definition>);
struct Art(BTreeMap<String, appearance::Appearance>);
struct Sounds(BTreeMap<String, Arc<BTreeMap<String, media::Voice>>>);
struct Effects(BTreeMap<String, Arc<visuals::Bank>>);

fn catalogue<T>(values: &BTreeMap<String, T>, max: usize) -> Result<(), String> {
    if values.is_empty() || values.len() > max || values.keys().any(|id| !abilities::id_valid(id)) {
        return Err("invalid character pack options/count".into());
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CodeSpec {
    abilities: BTreeMap<String, abilities::Definition>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtSpec {
    options: BTreeMap<String, Value>,
    #[serde(default, rename = "legacy_choices")]
    _legacy_choices: BTreeMap<String, String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SoundSpec {
    banks: BTreeMap<String, BTreeMap<String, media::SoundSpec>>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EffectSpec {
    banks: BTreeMap<String, BTreeMap<String, visuals::Specification>>,
    description: String,
}

pub fn bind(
    contract: &str,
    name: &str,
    value: &Value,
    assets: &BTreeMap<String, Arc<Asset>>,
    _factory: &Candidate,
    imports: &Resources,
) -> Result<Option<Binding>, String> {
    let binding: Binding = match contract {
        CODE => {
            let spec: CodeSpec =
                serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
            abilities::validate_definitions(&spec.abilities)?;
            Arc::new(Code(spec.abilities))
        }
        ART => {
            let spec: ArtSpec = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
            catalogue(&spec.options, 16)?;
            let mut options = BTreeMap::new();
            for (id, mut value) in spec.options {
                value
                    .as_object_mut()
                    .ok_or("art option must be an appearance")?
                    .insert("player".into(), json!(1));
                let prepared = appearance::bind(&value, assets)?;
                options.insert(
                    id,
                    prepared
                        .downcast_ref::<appearance::Appearance>()
                        .ok_or("art preparation returned an incompatible binding")?
                        .clone(),
                );
            }
            Arc::new(Art(options))
        }
        SOUND => {
            let spec: SoundSpec =
                serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
            catalogue(&spec.banks, 8)?;
            let mut banks = BTreeMap::new();
            for (id, specs) in spec.banks {
                banks.insert(id, Arc::new(media::prepare(&specs, assets)?));
            }
            Arc::new(Sounds(banks))
        }
        EFFECT => {
            let spec: EffectSpec =
                serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
            catalogue(&spec.banks, 8)?;
            if spec.description.len() > 256 {
                return Err("effect description exceeds budget".into());
            }
            let mut banks = BTreeMap::new();
            let mut graphs = BTreeMap::new();
            for (id, specs) in spec.banks {
                let bank = visuals::prepare(specs, assets)?;
                graphs.extend(
                    bank.values()
                        .map(|graph| (graph.id.clone(), Arc::clone(&graph.graph))),
                );
                if graphs
                    .values()
                    .map(|graph| graph.pixel_count())
                    .sum::<usize>()
                    > angel_delve::realm::effects::MAX_PIXELS
                {
                    return Err("effect catalogue pixel budget exceeded".into());
                }
                banks.insert(id, Arc::new(bank));
            }
            Arc::new(Effects(banks))
        }
        CONTRACT => resolve(name, value, imports)?,
        _ => return Ok(None),
    };
    Ok(Some(binding))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Choice {
    module: String,
    resource: String,
    option: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    player: u32,
    slots: [Option<Choice>; 4],
    #[serde(default)]
    appearance: Option<Choice>,
    sounds: Choice,
    effects: Choice,
    #[serde(default)]
    cancel: u64,
}
fn imported<'a, T: std::any::Any>(
    imports: &'a Resources,
    choice: &Choice,
    contract: &str,
) -> Result<(&'a Resource, &'a T), String> {
    let resource = imports.get(&choice.module, &choice.resource, contract)?;
    let prepared = resource
        .binding::<T>()
        .ok_or("pack native binding does not match its contract")?;
    Ok((resource, prepared))
}
fn resolve(name: &str, value: &Value, imports: &Resources) -> Result<Binding, String> {
    let selection: Selection = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
    if !(1..=4).contains(&selection.player)
        || name != format!("character-seat-{}", selection.player)
    {
        return Err("resolved character resource must match player 1..4".into());
    }
    let mut loadout: abilities::Loadout = serde_json::from_value(json!({
        "player":selection.player,"slots":[null,null,null,null],"abilities":{},"cancel":selection.cancel
    })).map_err(|e| e.to_string())?;
    let (_, sound) = imported::<Sounds>(imports, &selection.sounds, SOUND)?;
    loadout.media = Arc::clone(
        sound
            .0
            .get(&selection.sounds.option)
            .ok_or("selected sound bank is missing")?,
    );
    let (_, effect) = imported::<Effects>(imports, &selection.effects, EFFECT)?;
    loadout.vfx = Arc::clone(
        effect
            .0
            .get(&selection.effects.option)
            .ok_or("selected effect bank is missing")?,
    );
    if let Some(choice) = selection.appearance {
        let (_, art) = imported::<Art>(imports, &choice, ART)?;
        let mut appearance = art
            .0
            .get(&choice.option)
            .ok_or("selected appearance is missing")?
            .clone();
        appearance.player = selection.player;
        loadout.appearance = Some(appearance);
    }
    for (slot, choice) in selection.slots.into_iter().enumerate() {
        if let Some(choice) = choice {
            let (resource, code) = imported::<Code>(imports, &choice, CODE)?;
            let definition = code
                .0
                .get(&choice.option)
                .ok_or("selected ability is missing")?
                .clone();
            let alias = format!("slot{}", slot + 1);
            loadout.slots[slot] = Some(alias.clone());
            loadout.abilities.insert(alias.clone(), definition);
            loadout.executables.insert(
                alias,
                abilities::Executable {
                    resource: resource.clone(),
                    ability: choice.option,
                },
            );
        }
    }
    abilities::probe(&loadout, |alias| {
        loadout.executables[alias]
            .resource
            .instantiate(&json!({"kind":"abilities"}))
    })?;
    Ok(Arc::new(loadout))
}
