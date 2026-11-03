//! Pure, generation-pinned agent decisions. The module can rank an admitted
//! catalogue; execution authority still belongs to the native registry.
use super::*;

pub(super) fn rank_tools(
    program: &angel_live::Program,
    task: &str,
    docs: &[(usize, String)],
    catalogue: &[ToolDef],
    limit: usize,
) -> Result<Vec<usize>, String> {
    let candidates: Vec<_> = docs
        .iter()
        .map(|(index, text)| serde_json::json!({"name": catalogue[*index].name, "text": text}))
        .collect();
    let value = program
        .render(&serde_json::json!({"task": task, "candidates": candidates, "max": limit}))?;
    let names: Vec<String> = serde_json::from_value(value).map_err(|error| error.to_string())?;
    if names.len() > limit {
        return Err("live tool router exceeded the selection limit".into());
    }
    let mut ranked = Vec::with_capacity(names.len());
    for name in names {
        let index = docs
            .iter()
            .find_map(|(index, _)| (catalogue[*index].name == name).then_some(*index))
            .ok_or_else(|| format!("live tool router selected an unoffered tool: {name}"))?;
        if ranked.contains(&index) {
            return Err("live tool router selected a tool twice".into());
        }
        ranked.push(index);
    }
    Ok(ranked)
}

/// Which leg made a decision, for receipts: the native leg, a pinned live
/// generation, or the native leg after that generation failed its check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LegSource {
    Native,
    Live { digest: String },
    LiveFallback { digest: String, error: String },
    /// The generation compiled into this binary; no live one was active.
    Shipped { digest: String },
}

impl LegSource {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Live { .. } => "live",
            Self::LiveFallback { .. } => "live-fallback",
            Self::Shipped { .. } => "shipped",
        }
    }
}

static OFFERED: std::sync::LazyLock<
    std::sync::RwLock<std::collections::BTreeMap<String, angel_live::Program>>,
> = std::sync::LazyLock::new(Default::default);

/// Publish the active legs for callers outside a turn (drive, knowledge,
/// game policy). Turn callers use the registry's pinned generation instead.
pub(crate) fn offer(policies: &std::collections::BTreeMap<String, angel_live::Program>) {
    *OFFERED.write().unwrap_or_else(|p| p.into_inner()) = policies.clone();
}

/// One decision from the turn's pinned `name` leg. Any evaluation, decode or
/// `check` failure keeps the native leg, so a bad edit never blocks a turn.
pub(crate) fn decide<T: serde::de::DeserializeOwned>(
    registry: &ToolRegistry,
    name: &str,
    input: &serde_json::Value,
    check: impl FnOnce(&T) -> Result<(), String>,
    native: impl FnOnce() -> T,
) -> (T, LegSource) {
    decide_with(registry.live_policy(name), name, input, check, native)
}

/// `decide` for code with no turn: the newest active generation of `name`.
pub(crate) fn decide_latest<T: serde::de::DeserializeOwned>(
    name: &str,
    input: &serde_json::Value,
    check: impl FnOnce(&T) -> Result<(), String>,
    native: impl FnOnce() -> T,
) -> (T, LegSource) {
    let program = OFFERED
        .read()
        .unwrap_or_else(|p| p.into_inner())
        .get(name)
        .cloned();
    decide_with(program, name, input, check, native)
}

pub(crate) fn decide_with<T: serde::de::DeserializeOwned>(
    program: Option<angel_live::Program>,
    name: &str,
    input: &serde_json::Value,
    check: impl FnOnce(&T) -> Result<(), String>,
    native: impl FnOnce() -> T,
) -> (T, LegSource) {
    let Some(program) = program else {
        return (native(), LegSource::Native);
    };
    let digest = program.digest().to_string();
    let decided = program.render(input).and_then(|value| {
        let value: T = serde_json::from_value(value).map_err(|error| error.to_string())?;
        check(&value)?;
        Ok(value)
    });
    match decided {
        Ok(value) => (value, LegSource::Live { digest }),
        Err(error) => {
            tracing::debug!(%error, leg = name, digest, "live leg retained the native decision");
            (native(), LegSource::LiveFallback { digest, error })
        }
    }
}

/// One decision from `name`: the newest live generation when it answers
/// within `check`, else the generation compiled into this binary. A shipped
/// leg that cannot answer is a build defect, not a runtime condition.
pub(crate) fn decide_shipped<T: serde::de::DeserializeOwned>(
    name: &str,
    input: &serde_json::Value,
    check: impl Fn(&T) -> Result<(), String>,
) -> (T, LegSource) {
    let live = OFFERED
        .read()
        .unwrap_or_else(|p| p.into_inner())
        .get(name)
        .cloned();
    with_shipped(live, name, input, check)
}

/// `decide_shipped` with the turn's pinned generation instead of the newest.
pub(crate) fn decide_pinned<T: serde::de::DeserializeOwned>(
    registry: &ToolRegistry,
    name: &str,
    input: &serde_json::Value,
    check: impl Fn(&T) -> Result<(), String>,
) -> (T, LegSource) {
    with_shipped(registry.live_policy(name), name, input, check)
}

fn with_shipped<T: serde::de::DeserializeOwned>(
    live: Option<angel_live::Program>,
    name: &str,
    input: &serde_json::Value,
    check: impl Fn(&T) -> Result<(), String>,
) -> (T, LegSource) {
    let answer = |program: &angel_live::Program| -> Result<T, String> {
        let value: T = serde_json::from_value(program.render(input)?).map_err(|e| e.to_string())?;
        check(&value)?;
        Ok(value)
    };
    let shipped = angel_live::shipped::program(name)
        .unwrap_or_else(|| panic!("no shipped leg named {name}"));
    let fallback = match live {
        Some(program) => match answer(&program) {
            Ok(value) => return (value, LegSource::Live { digest: program.digest().into() }),
            Err(error) => {
                tracing::debug!(%error, leg = name, "live leg kept the shipped decision");
                Some((program.digest().to_string(), error))
            }
        },
        None => None,
    };
    // One retry: a run terminated by the evaluator's deadline rebuilds its isolate.
    let value = answer(shipped)
        .or_else(|_| answer(shipped))
        .unwrap_or_else(|error| panic!("shipped leg {name} failed: {error}"));
    let source = match fallback {
        Some((digest, error)) => LegSource::LiveFallback { digest, error },
        None => LegSource::Shipped { digest: shipped.digest().into() },
    };
    (value, source)
}

/// The shipped `cockpit/live/<id>` leg in the embedded engine, for parity tests.
#[cfg(test)]
pub(crate) fn shipped_leg(id: &str) -> angel_live::Program {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("live");
    let mut registry = angel_live::Registry::validated(
        root,
        crate::agent::hotload::evaluate,
        |_, _| Ok(()),
    );
    registry.refresh(true);
    let snapshot = registry.snapshot();
    snapshot.policies.get(id).cloned().unwrap_or_else(|| {
        let status = snapshot.statuses.iter().find(|s| s.id == id);
        panic!("shipped leg {id} is not an active policy module: {status:?}")
    })
}

#[cfg(test)]
#[test]
fn every_shipped_leg_is_the_directory_generation_and_answers_its_probe() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("live");
    let mut on_disk: Vec<String> = std::fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|dir| {
            std::fs::read_to_string(dir.join("module.json"))
                .is_ok_and(|m| m.split_whitespace().collect::<String>().contains("\"kind\":\"policy\""))
        })
        .map(|dir| dir.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    on_disk.sort();
    let shipped: Vec<String> = angel_live::shipped::ids().map(str::to_string).collect();
    assert_eq!(shipped, on_disk);
    for id in angel_live::shipped::ids() {
        let program = angel_live::shipped::program(id).unwrap();
        assert_eq!(program.digest(), shipped_leg(id).digest(), "{id}");
    }
}

/// The engine `angel_delve::scripted` rules run on in this process: the newest
/// live generation of the module, else the shipped one.
pub(crate) fn install_scripted_rules() {
    fn engine(id: &str, input: &serde_json::Value) -> Result<serde_json::Value, String> {
        Ok(decide_shipped::<serde_json::Value>(id, input, |_| Ok(())).0)
    }
    angel_delve::scripted::install(engine);
}
