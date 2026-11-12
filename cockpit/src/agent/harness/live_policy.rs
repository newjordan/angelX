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

pub(crate) use angel_live::policy::{LegSource, decide_with, offer, offered};

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

/// One decision from `name`: the newest live generation when it answers
/// within `check`, else the generation compiled into this binary. A shipped
/// leg that cannot answer is a build defect, not a runtime condition.
pub(crate) fn decide_shipped<T: serde::de::DeserializeOwned>(
    name: &str,
    input: &serde_json::Value,
    check: impl Fn(&T) -> Result<(), String>,
) -> (T, LegSource) {
    with_shipped(offered(name), name, input, check, true)
}

/// [`decide_shipped`] for callers whose questions do not repeat (the game's
/// per-frame rules): it asks V8 every time and keeps nothing.
pub(crate) fn decide_shipped_unmemoized<T: serde::de::DeserializeOwned>(
    name: &str,
    input: &serde_json::Value,
    check: impl Fn(&T) -> Result<(), String>,
) -> (T, LegSource) {
    with_shipped(offered(name), name, input, check, false)
}

/// `decide_shipped` with the turn's pinned generation instead of the newest.
pub(crate) fn decide_pinned<T: serde::de::DeserializeOwned>(
    registry: &ToolRegistry,
    name: &str,
    input: &serde_json::Value,
    check: impl Fn(&T) -> Result<(), String>,
) -> (T, LegSource) {
    with_shipped(registry.live_policy(name), name, input, check, true)
}

/// What a generation answered for an input, so the same question put to the
/// same generation is not put to V8 again.
///
/// A policy module is a pure projection: it reads its input, never the clock,
/// the filesystem or the host, and every call runs in a fresh context. Its
/// output is therefore a function of the bundle's content digest and the input
/// alone. A turn asks the same few questions again and again (each tool result
/// is classified by several callers, each hop re-reads the same standing
/// policy), and each evaluation costs a context and a script run.
///
/// Only the module's decoded-and-validated answer is kept. The caller's `check`
/// still runs on every use, a failed evaluation is never kept, and the retry
/// after a failed answer bypasses the memo, so nothing a leg can do changes
/// except how often V8 is asked. A generation that is edited has a new digest
/// and starts with no entries.
mod memo {
    use std::collections::{HashMap, VecDeque};
    use std::sync::Mutex;

    const ENTRIES: usize = 256;
    const MAX_INPUT_BYTES: usize = 16 * 1024;
    const MAX_ANSWER_BYTES: usize = 16 * 1024;

    type Key = (String, String);

    #[derive(Default)]
    struct Memo {
        answers: HashMap<Key, serde_json::Value>,
        order: VecDeque<Key>,
        hits: u64,
        misses: u64,
    }

    static MEMO: Mutex<Option<Memo>> = Mutex::new(None);

    fn with<R>(f: impl FnOnce(&mut Memo) -> R) -> R {
        let mut guard = MEMO.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        f(guard.get_or_insert_with(Memo::default))
    }

    pub(super) fn key(digest: &str, input: &serde_json::Value) -> Option<Key> {
        let text = serde_json::to_string(input).ok()?;
        (text.len() <= MAX_INPUT_BYTES).then(|| (digest.to_string(), text))
    }

    pub(super) fn recall(key: &Key) -> Option<serde_json::Value> {
        with(|memo| {
            let found = memo.answers.get(key).cloned();
            if found.is_some() {
                memo.hits += 1;
            } else {
                memo.misses += 1;
            }
            found
        })
    }

    pub(super) fn keep(key: Key, answer: &serde_json::Value) {
        if serde_json::to_string(answer).map_or(true, |text| text.len() > MAX_ANSWER_BYTES) {
            return;
        }
        with(|memo| {
            if memo.answers.insert(key.clone(), answer.clone()).is_none() {
                memo.order.push_back(key);
                while memo.order.len() > ENTRIES {
                    if let Some(oldest) = memo.order.pop_front() {
                        memo.answers.remove(&oldest);
                    }
                }
            }
        });
    }

    /// `(hits, misses)` since the process started.
    #[cfg(test)]
    pub(super) fn counts() -> (u64, u64) {
        with(|memo| (memo.hits, memo.misses))
    }

    /// Entries held now.
    #[cfg(test)]
    pub(super) fn len() -> usize {
        with(|memo| memo.answers.len())
    }

    #[cfg(test)]
    pub(super) fn holds(key: &Key) -> bool {
        with(|memo| memo.answers.contains_key(key))
    }

    #[cfg(test)]
    pub(super) const CAPACITY: usize = ENTRIES;
}

/// `program.render(input)` through the memo. `fresh` asks V8 regardless: the
/// retry after a failed answer must not meet the answer that just failed.
fn render_memoized(
    program: &angel_live::Program,
    input: &serde_json::Value,
    fresh: bool,
) -> Result<serde_json::Value, String> {
    let key = memo::key(program.digest(), input);
    if !fresh && let Some(key) = &key && let Some(answer) = memo::recall(key) {
        return Ok(answer);
    }
    let answer = program.render(input)?;
    if let Some(key) = key {
        memo::keep(key, &answer);
    }
    Ok(answer)
}

fn with_shipped<T: serde::de::DeserializeOwned>(
    live: Option<angel_live::Program>,
    name: &str,
    input: &serde_json::Value,
    check: impl Fn(&T) -> Result<(), String>,
    memoize: bool,
) -> (T, LegSource) {
    let answer = |program: &angel_live::Program, fresh: bool| -> Result<T, String> {
        let rendered = if memoize {
            render_memoized(program, input, fresh)?
        } else {
            program.render(input)?
        };
        let value: T = serde_json::from_value(rendered).map_err(|e| e.to_string())?;
        check(&value)?;
        Ok(value)
    };
    let shipped = angel_live::shipped::program(name)
        .unwrap_or_else(|| panic!("no shipped leg named {name}"));
    let fallback = match live {
        Some(program) => match answer(&program, false) {
            Ok(value) => return (value, LegSource::Live { digest: program.digest().into() }),
            Err(error) => {
                tracing::debug!(%error, leg = name, "live leg kept the shipped decision");
                Some((program.digest().to_string(), error))
            }
        },
        None => None,
    };
    // One retry: a run terminated by the evaluator's deadline rebuilds its isolate.
    let value = answer(shipped, false)
        .or_else(|_| answer(shipped, true))
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
    use crate::app::control::realm::GameHost;
    use angel_plug::drive::DriveWorld;
    fn engine(id: &str, input: &serde_json::Value) -> Result<serde_json::Value, String> {
        Ok(decide_shipped_unmemoized::<serde_json::Value>(id, input, |_| Ok(())).0)
    }
    <GameHost as DriveWorld>::install_scripted_rules(engine);
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/live_policy__memo_tests.rs"]
mod memo_tests;
