//! Complete, versioned in-memory state for implementation handoff. Durable saves
//! deliberately omit queues and latches; this codec must never be used for saves
//! or the public game wire. The scope is thread local and restored on unwind.
use super::Run;
use serde::{Deserialize, Deserializer, Serialize};
use std::cell::Cell;
use std::collections::BTreeSet;
use std::sync::{Mutex, OnceLock};

pub const SCHEMA: u32 = 8;
/// Shared live-session bound, including rules-only worker builds and Glass views.
pub const MAX_TICK: u64 = 1 << 32;
// Hide the reference syntax from serde's implicit-borrow inference: the
// deserializer interns owned text and never borrows from the input frame.
pub type Symbol = &'static str;
thread_local! {
    static CAPTURING: Cell<bool> = const { Cell::new(false) };
    static RULES_ONLY: Cell<bool> = const { Cell::new(false) };
}
struct Scope(bool);
impl Scope {
    fn enter() -> Self {
        Self(CAPTURING.replace(true))
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        CAPTURING.set(self.0);
    }
}

pub fn omit<T>(_: &T) -> bool {
    !CAPTURING.get()
}
pub fn feel_quiet(v: &super::feel::Feel) -> bool {
    omit(v) && v.quiet()
}
pub fn shoulders_quiet(v: &super::shoulders::Shoulders) -> bool {
    omit(v) && v.quiet()
}
pub fn altar_quiet(v: &super::altar::Lamp) -> bool {
    omit(v) && v.quiet()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint {
    pub schema: u32,
    pub run: serde_json::Value,
}

impl Checkpoint {
    /// Move an already-serialized state tree into its wire envelope.
    pub fn into_value(self) -> serde_json::Value {
        serde_json::Value::Object(serde_json::Map::from_iter([
            ("schema".into(), self.schema.into()),
            ("run".into(), self.run),
        ]))
    }
}

pub fn capture(run: &Run) -> Result<Checkpoint, String> {
    let _scope = Scope::enter();
    Ok(Checkpoint {
        schema: SCHEMA,
        run: serde_json::to_value(run).map_err(|e| e.to_string())?,
    })
}

/// Rules do not consume character appearance bytes. Keep them in the host's
/// Glass resource owner instead of sending every animation frame on each tick.
/// Full capture/saves/wire remain complete and unchanged.
pub fn capture_for_rules(run: &Run) -> Result<Checkpoint, String> {
    struct RulesScope(bool);
    impl Drop for RulesScope {
        fn drop(&mut self) {
            RULES_ONLY.set(self.0);
        }
    }
    let _scope = RulesScope(RULES_ONLY.replace(true));
    capture(run)
}
pub fn serialize_avatar<S: serde::Serializer>(
    avatar: &Option<crate::realm::Avatar>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    if RULES_ONLY.get() {
        Option::<&crate::realm::Avatar>::None.serialize(serializer)
    } else {
        avatar.serialize(serializer)
    }
}
pub fn retain_appearance(next: &mut Run, host: &Run) {
    next.module_visuals = host.module_visuals.clone();
    for (id, hero) in &mut next.players {
        hero.avatar = host
            .players
            .get(id)
            .and_then(|previous| previous.avatar.clone());
    }
}

pub fn serialize_visuals<S: serde::Serializer>(
    scene: &crate::realm::effects::Scene,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    if RULES_ONLY.get() {
        crate::realm::effects::Scene::default().serialize(serializer)
    } else {
        scene.serialize(serializer)
    }
}

pub fn restore(state: Checkpoint) -> Result<Run, String> {
    restore_parts(state.schema, &state.run)
}

/// Decode directly from an RPC envelope without cloning its entire JSON tree.
/// Retain the same schema, completeness and unknown-envelope-field checks.
pub fn restore_value(state: &serde_json::Value) -> Result<Run, String> {
    let fields = state
        .as_object()
        .filter(|fields| {
            fields.len() == 2 && fields.contains_key("schema") && fields.contains_key("run")
        })
        .ok_or("invalid Delve checkpoint envelope")?;
    let schema = fields["schema"]
        .as_u64()
        .and_then(|schema| u32::try_from(schema).ok())
        .ok_or("invalid Delve checkpoint schema")?;
    restore_parts(schema, &fields["run"])
}
fn restore_parts(schema: u32, state: &serde_json::Value) -> Result<Run, String> {
    if schema == 1 {
        if state.get("module_visuals").is_some()
            || state.get("module_casts").is_some()
            || state
                .get("players")
                .and_then(serde_json::Value::as_object)
                .is_some_and(|players| {
                    players
                        .values()
                        .any(|hero| hero.get("module_spells").is_some())
                })
        {
            return Err("schema 1 contains schema 2 ability state".into());
        }
        let mut migrated = state.clone();
        migrated
            .as_object_mut()
            .ok_or("invalid schema 1 Run")?
            .insert("module_casts".into(), serde_json::json!([]));
        return restore_parts(2, &migrated);
    }
    if schema == 2 {
        if state.get("module_visuals").is_some() {
            return Err("schema 2 contains schema 3 visual state".into());
        }
        let mut migrated = state.clone();
        migrated
            .as_object_mut()
            .ok_or("invalid schema 2 Run")?
            .insert(
                "module_visuals".into(),
                serde_json::to_value(crate::realm::effects::Scene::default()).unwrap(),
            );
        return restore_parts(3, &migrated);
    }
    if schema == 3 {
        if state
            .get("players")
            .and_then(serde_json::Value::as_object)
            .is_some_and(|players| {
                players
                    .values()
                    .any(|hero| hero.get("reforge_job").is_some())
            })
        {
            return Err("schema 3 contains schema 4 reforge state".into());
        }
        return restore_parts(4, state);
    }
    if schema == 4 {
        if state.get("seat_epochs").is_some() || state.get("players").and_then(serde_json::Value::as_object).is_some_and(|players| players.values().any(|hero| hero.get("wish_jobs").is_some())) {
            return Err("schema 4 contains schema 5 wish state".into());
        }
        return restore_parts(5, state);
    }
    if schema == 5 {
        let mut migrated = state.clone();
        if let Some(players) = migrated.get_mut("players").and_then(serde_json::Value::as_object_mut) {
            let raid = state.get("raid_id").and_then(serde_json::Value::as_u64).unwrap_or(0);
            let epochs = state.get("seat_epochs").and_then(serde_json::Value::as_object);
            for (id, hero) in players {
                if let Some(offer) = hero.get_mut("offer").and_then(serde_json::Value::as_object_mut) {
                    if offer.contains_key("binding") { return Err("schema 5 contains schema 6 retained scroll sources".into()); }
                    let player = id.parse::<u32>().map_err(|_| "invalid scroll player")?;
                    let epoch = epochs.and_then(|e| e.get(id)).and_then(serde_json::Value::as_u64).unwrap_or(0);
                    offer.insert("binding".into(), serde_json::to_value(super::scrolls::Binding::legacy(raid, player, epoch)).map_err(|e| e.to_string())?);
                }
            }
        }
        return restore_parts(6, &migrated);
    }
    if schema == 6 {
        // Epoch/definition cannot be reconstructed from today's occupants or
        // slot catalogue. The old owner must drain its own requests first.
        if !state.get("module_casts").and_then(serde_json::Value::as_array)
            .is_some_and(Vec::is_empty)
        { return Err("schema 6 has unresolved ability requests; drain before reload".into()); }
        return restore_parts(7, state);
    }
    if schema == 7 {
        let mut migrated = state.clone();
        let fields = migrated.as_object_mut().ok_or("invalid schema 7 Run")?;
        // A downgraded clean envelope may contain the new neutral defaults,
        // but an older schema must never carry live reservation state.
        if fields.get("squire_company_reserved").is_some_and(|v| v != &serde_json::Value::Bool(false))
            || fields.get("squire_company_claimed").is_some_and(|v| v != &serde_json::Value::Bool(false))
            || fields.get("squire_pending_delves").is_some_and(|v| v.as_u64() != Some(0))
        {
            return Err("schema 7 contains schema 8 squire reservation state".into());
        }
        fields.insert("squire_company_reserved".into(), false.into());
        fields.insert("squire_company_claimed".into(), false.into());
        fields.insert("squire_pending_delves".into(), 0.into());
        return restore_parts(SCHEMA, &migrated);
    }
    if schema != SCHEMA {
        return Err(format!(
            "unsupported Delve live state {schema} (expected {SCHEMA})"
        ));
    }
    let _scope = Scope::enter();
    let run = Run::deserialize(state).map_err(|e| format!("invalid Delve live state: {e}"))?;
    run.module_visuals.validate()?;
    super::abilities::validate_outbox(&run.module_casts)?;
    for slot in run.players.values().flat_map(|hero| hero.module_spells.iter().flatten()) {
        slot.validate()?;
    }
    super::scrolls::validate_run(&run)?;
    // Missing transient fields would otherwise take durable-save defaults.
    if !same_shape(&capture(&run)?.run, state) {
        return Err("incomplete or incompatible Delve live checkpoint".into());
    }
    Ok(run)
}

fn same_shape(a: &serde_json::Value, b: &serde_json::Value) -> bool {
    use serde_json::Value;
    match (a, b) {
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(k, v)| b.get(k).is_some_and(|w| same_shape(v, w)))
        }
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(v, w)| same_shape(v, w))
        }
        // JSON transport may change the last f64 bit representing an f32.
        // Typed deserialization has already checked values; completeness is
        // about fields, not spelling or float formatting.
        (Value::Number(_), Value::Number(_))
        | (Value::String(_), Value::String(_))
        | (Value::Bool(_), Value::Bool(_))
        | (Value::Null, Value::Null) => true,
        _ => false,
    }
}

pub fn transient<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    if CAPTURING.get() {
        T::deserialize(d)
    } else {
        serde::de::IgnoredAny::deserialize(d).map(|_| T::default())
    }
}
pub fn hit_ward<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    if CAPTURING.get() {
        u32::deserialize(d)
    } else {
        serde::de::IgnoredAny::deserialize(d).map(|_| super::hero::hit_ward())
    }
}

// Rules use static IDs for sounds, feats and entrance names. Decode them once,
// with a hard process-wide memory ceiling, rather than leaking on every tick or
// accepting unbounded strings. Repeated snapshots reuse the same references.
fn symbol(s: String) -> Result<&'static str, String> {
    static SYMBOLS: OnceLock<Mutex<BTreeSet<&'static str>>> = OnceLock::new();
    let mut ids = SYMBOLS
        .get_or_init(Default::default)
        .lock()
        .map_err(|_| "live symbol lock poisoned")?;
    if let Some(&known) = ids.get(s.as_str()) {
        return Ok(known);
    }
    if s.len() > 256 || ids.len() >= 2048 {
        return Err("live symbol table limit exceeded".into());
    }
    let name = &*Box::leak(s.into_boxed_str());
    ids.insert(name);
    Ok(name)
}
pub fn symbols<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<&'static str>, D::Error> {
    if !CAPTURING.get() {
        return serde::de::IgnoredAny::deserialize(d).map(|_| Vec::new());
    }
    Vec::<String>::deserialize(d)?
        .into_iter()
        .map(|s| symbol(s).map_err(serde::de::Error::custom))
        .collect()
}
pub fn symbol_option<'de, D: Deserializer<'de>>(d: D) -> Result<Option<&'static str>, D::Error> {
    if !CAPTURING.get() {
        return serde::de::IgnoredAny::deserialize(d).map(|_| None);
    }
    Option::<String>::deserialize(d)?
        .map(symbol)
        .transpose()
        .map_err(serde::de::Error::custom)
}
pub fn seat_symbols<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<(u32, &'static str)>, D::Error> {
    if !CAPTURING.get() {
        return serde::de::IgnoredAny::deserialize(d).map(|_| Vec::new());
    }
    Vec::<(u32, String)>::deserialize(d)?
        .into_iter()
        .map(|(id, s)| symbol(s).map(|s| (id, s)).map_err(serde::de::Error::custom))
        .collect()
}
pub fn timed_symbols<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<Vec<(u64, &'static str)>, D::Error> {
    if !CAPTURING.get() {
        return serde::de::IgnoredAny::deserialize(d).map(|_| Vec::new());
    }
    Vec::<(u64, String)>::deserialize(d)?
        .into_iter()
        .map(|(id, s)| symbol(s).map(|s| (id, s)).map_err(serde::de::Error::custom))
        .collect()
}
