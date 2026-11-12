//! Exact read-only host projection. This is not a live/durable checkpoint and
//! cannot restore a rules owner. Ordinary publications carry document deltas.
use super::super::{Hero, Run, scrolls};
use angel_net::doc::{
    delta, kept,
    value::{self, Value},
    wire,
};
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, sync::Arc};

pub const API: u32 = 1;
/// JSON numeric byte arrays expand by at most four times. A view frame also
/// checks its actual encoded JSON size, leaving four MiB of the System's 16 MiB
/// frame for its outer envelope and other bounded session outputs.
pub const MAX_ENCODED_FRAME_BYTES: usize = 12 * 1024 * 1024;
pub const MAX_BASELINE_BYTES: usize = 3 * 1024 * 1024;
pub const MAX_DELTA_BYTES: usize = 512 * 1024;
const MAX_DICTIONARY_NAMES: usize = 4096;
const MAX_VIEW_UNITS: usize = 16 * 1024 * 1024;

const PRIVATE: &[&str] = &[
    "journal",
    "commands",
    "checkpoint",
    "state",
    "run",
    "rng",
    "next_id",
    "parked",
    "floor_tally",
    "asked",
    "ledger",
    "brewing",
    "bank",
    "reclaimed",
    "module_casts",
    "module_visuals",
    "orders",
    "peddled",
    "greeted",
    "triumph",
    "feats",
    "slimes_slain",
    "descend_for",
    "met",
    "latch_input_age",
    "build_hold",
    "lever_held",
    "rooms_held",
    "groom_near",
    "king_near",
    "cues",
    "sounds",
    "seat_sounds",
    "hostile_shots",
    "streak",
    "room_hurt",
    "blooded",
    "spiked",
    "notices",
    "seat_notices",
    "chatter",
    "fx",
    "offer_keys",
    "idle",
];
const HERO_PRIVATE: &[&str] = &["avatar", "reforge_job", "wish_jobs"];
const STATUS: &str = "host_status";

/// Only summary facts. Exact claims, completions and definitions are obtained
/// through the owner's typed queries, never reconstructed from the read view.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Outbox {
    pub realm: bool,
    pub home: bool,
    pub brews: u16,
    pub reforges: u16,
    pub wishes: u16,
    pub playtest: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Status {
    pub tick: u64,
    pub depth: u32,
    pub room: usize,
    pub job_revision: u64,
    pub outbox: Outbox,
}
impl Status {
    pub fn capture(run: &Run, job_revision: u64, outbox: Outbox) -> Self {
        Self {
            tick: run.tick,
            depth: run.dungeon.depth,
            room: run.at,
            job_revision,
            outbox,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Baseline,
    Delta,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    pub api: u32,
    pub kind: Kind,
    pub session: String,
    pub raid: u64,
    pub base_revision: u64,
    pub revision: u64,
    pub bytes: Vec<u8>,
}
#[derive(Clone, Copy)]
pub struct Limits {
    baseline: usize,
    delta: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            baseline: MAX_BASELINE_BYTES,
            delta: MAX_DELTA_BYTES,
        }
    }
}
impl Limits {
    pub fn new(baseline: usize, delta: usize) -> Result<Self, String> {
        if baseline == 0 || baseline > MAX_BASELINE_BYTES || delta == 0 || delta > MAX_DELTA_BYTES {
            return Err("invalid host view limits".into());
        }
        Ok(Self { baseline, delta })
    }
    fn frame(self, frame: &Frame) -> Result<(), String> {
        let raw = match frame.kind {
            Kind::Baseline => self.baseline,
            Kind::Delta => self.delta,
        };
        if frame.bytes.len() > raw {
            return Err("host view frame exceeds byte bound".into());
        }
        struct Count(usize);
        impl std::io::Write for Count {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0 = self
                    .0
                    .checked_add(bytes.len())
                    .filter(|n| *n <= MAX_ENCODED_FRAME_BYTES)
                    .ok_or_else(|| {
                        std::io::Error::other("host view encoded frame exceeds byte bound")
                    })?;
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        serde_json::to_writer(Count(0), frame).map_err(|e| e.to_string())
    }
}

/// There is deliberately no mutable getter, serde implementation, or authority
/// restore/capture operation. Omitted private state reads only as its defaults.
#[derive(Debug)]
pub struct HostView {
    session: Arc<str>,
    raid: u64,
    revision: u64,
    run: Arc<Run>,
    status: Status,
}
impl HostView {
    pub fn session(&self) -> &str {
        &self.session
    }
    pub fn raid(&self) -> u64 {
        self.raid
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn run(&self) -> &Run {
        &self.run
    }
    pub fn status(&self) -> &Status {
        &self.status
    }
}

pub struct Publisher {
    session: String,
    raid: u64,
    revision: u64,
    fields: kept::Fields,
    tree: Option<Value>,
    budget: BudgetCache,
    writer: wire::Writer,
    limits: Limits,
}
impl Publisher {
    /// Revision comes from the owner at initialization or reload. It cannot be
    /// reset by an ordinary event; its first publication is an explicit baseline.
    /// Construct a fresh Publisher after restoring/replacing Run, even if its
    /// session/raid or Book revision happens to match the previous authority.
    pub fn new(session: String, raid: u64, revision: u64, limits: Limits) -> Result<Self, String> {
        session_valid(&session)?;
        Ok(Self {
            session,
            raid,
            revision,
            fields: Default::default(),
            tree: None,
            budget: BudgetCache::default(),
            writer: Default::default(),
            limits,
        })
    }
    pub fn baseline(&mut self, run: &Run, status: Status) -> Result<Frame, String> {
        self.publish(run, status, Kind::Baseline)
    }
    pub fn delta(&mut self, run: &Run, status: Status) -> Result<Frame, String> {
        if self.tree.is_none() {
            return Err("host view requires a baseline".into());
        }
        self.publish(run, status, Kind::Delta)
    }
    fn publish(&mut self, run: &Run, status: Status, kind: Kind) -> Result<Frame, String> {
        if run.raid_id != self.raid {
            return Err("host view belongs to another raid".into());
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or("host view revision exhausted")?;
        if !run.valid_snapshot() {
            return Err("invalid source for host read view".into());
        }
        status_valid(run, &status)?;
        let tree = project(run, &status, &mut self.fields)?;
        let budget = validate_tree(&tree, &self.budget)?;
        let mut writer = if kind == Kind::Baseline {
            wire::Writer::default()
        } else {
            self.writer.clone()
        };
        let mut bytes = Vec::new();
        if kind == Kind::Baseline {
            writer.value(&mut bytes, &tree);
        } else {
            let patch = delta::diff_exact(self.tree.as_ref().unwrap(), &tree).unwrap_or_else(|| {
                delta::Patch::Struct {
                    set: Vec::new(),
                    gone: Vec::new(),
                }
            });
            writer.patch(&mut bytes, &patch);
        }
        if writer.dictionary_len() > MAX_DICTIONARY_NAMES {
            return Err("host view name dictionary exceeds bound".into());
        }
        let frame = Frame {
            api: API,
            kind,
            session: self.session.clone(),
            raid: self.raid,
            base_revision: if kind == Kind::Baseline {
                0
            } else {
                self.revision
            },
            revision,
            bytes,
        };
        self.limits.frame(&frame)?;
        // Cache fingerprints may advance on refusal, but the externally visible
        // tree, revision and name dictionary commit only after all bounds pass.
        self.budget = budget;
        self.tree = Some(tree);
        self.writer = writer;
        self.revision = revision;
        Ok(frame)
    }
}

#[derive(Clone)]
pub struct Receiver {
    session: String,
    raid: u64,
    reader: wire::Reader,
    tree: Option<Value>,
    budget: BudgetCache,
    view: Option<Arc<HostView>>,
    limits: Limits,
}
impl Receiver {
    pub fn new(session: String, raid: u64, limits: Limits) -> Result<Self, String> {
        session_valid(&session)?;
        Ok(Self {
            session,
            raid,
            reader: Default::default(),
            tree: None,
            budget: BudgetCache::default(),
            view: None,
            limits,
        })
    }
    pub fn view(&self) -> Option<&Arc<HostView>> {
        self.view.as_ref()
    }
    /// Compare complete accepted public state at a checkpoint/recovery boundary.
    /// Job revision is an invalidation hint rebuilt after restore; all public
    /// game fields, outbox facts and the session/view ownership must match.
    pub fn same_public_state(&self, other: &Self) -> bool {
        let (Some(a), Some(b)) = (&self.view, &other.view) else { return false; };
        if self.session != other.session || self.raid != other.raid || a.revision != b.revision {
            return false;
        }
        let mut status = a.status.clone();
        status.job_revision = b.status.job_revision;
        if status != b.status { return false; }
        match (self.tree.as_ref().map(Value::get), other.tree.as_ref().map(Value::get)) {
            (Some(Value::Struct(a)), Some(Value::Struct(b))) => a.len() == b.len()
                && a.iter().all(|(name, value)| name == STATUS
                    || b.iter().find(|(other, _)| name == other).is_some_and(|(_, other)| value == other)),
            _ => false,
        }
    }
    /// Explicit init/resync/reload boundary. A new session creates a new receiver.
    pub fn baseline(&mut self, frame: &Frame) -> Result<Arc<HostView>, String> {
        if frame.kind != Kind::Baseline {
            return Err("expected host view baseline".into());
        }
        self.accept(frame)
    }
    pub fn delta(&mut self, frame: &Frame) -> Result<Arc<HostView>, String> {
        if frame.kind != Kind::Delta {
            return Err("expected host view delta".into());
        }
        self.accept(frame)
    }
    fn accept(&mut self, frame: &Frame) -> Result<Arc<HostView>, String> {
        self.limits.frame(frame)?;
        if frame.api != API || frame.session != self.session || frame.raid != self.raid {
            return Err("host view version/session/raid mismatch".into());
        }
        let previous = self.view.as_ref().map_or(0, |v| v.revision);
        if frame.kind == Kind::Baseline {
            if frame.base_revision != 0 || frame.revision == 0 || frame.revision <= previous {
                return Err("stale host view baseline".into());
            }
        } else if self.tree.is_none()
            || frame.base_revision != previous
            || previous.checked_add(1) != Some(frame.revision)
        {
            return Err("stale or out-of-order host view delta".into());
        }
        let mut reader = if frame.kind == Kind::Baseline {
            wire::Reader::default()
        } else {
            self.reader.clone()
        };
        let mut bytes = wire::Bytes::new(&frame.bytes);
        let mut tree = if frame.kind == Kind::Baseline {
            reader
                .value(&mut bytes)
                .map_err(|_| "malformed host view baseline")?
        } else {
            let patch = reader
                .patch(&mut bytes)
                .map_err(|_| "malformed host view delta")?;
            let mut tree = self.tree.as_ref().unwrap().clone();
            delta::apply(&mut tree, patch)
                .map_err(|_| "host view delta does not match baseline")?;
            tree
        };
        if reader.dictionary_len() > MAX_DICTIONARY_NAMES {
            return Err("host view name dictionary exceeds bound".into());
        }
        if !bytes.rest.is_empty() {
            return Err("trailing host view bytes".into());
        }
        share_top_fields(&mut tree);
        let budget = validate_tree(&tree, &self.budget)?;
        // Status-only changes and private drains do not alter any public game
        // subtree. Keep the previously validated typed Run rather than decoding
        // and cloning its entire book for another accepted revision.
        let retained = match (&self.tree, &self.view) {
            (Some(previous), Some(view))
                if frame.kind == Kind::Delta && same_game_fields(previous, &tree) =>
            {
                Some(Arc::clone(&view.run))
            }
            _ => None,
        };
        let run = match retained {
            Some(run) => run,
            None => {
                let mut run: Run = value::from_value(&tree).map_err(|e| e.to_string())?;
                // Ordinary Run deserialization intentionally discards these
                // transient display fields. Restore their exact public values.
                run.marks = value::from_value(tree.field("marks").ok_or("missing host marks")?)
                    .map_err(|e| e.to_string())?;
                run.campaign = value::from_value(tree.field("campaign").ok_or("missing host campaign")?)
                    .map_err(|e| e.to_string())?;
                if run.raid_id != self.raid {
                    return Err("host read view raid differs".into());
                }
                if !run.valid_read_view() {
                    return Err("invalid host read view".into());
                }
                Arc::new(run)
            }
        };
        let status: Status = value::from_value(tree.field(STATUS).ok_or("missing host status")?)
            .map_err(|e| e.to_string())?;
        status_valid(&run, &status)?;
        let view = Arc::new(HostView {
            session: self.session.clone().into(),
            raid: self.raid,
            revision: frame.revision,
            run,
            status,
        });
        // Reader name additions from bad/truncated frames never escape staging.
        self.reader = reader;
        self.budget = budget;
        self.tree = Some(tree);
        self.view = Some(Arc::clone(&view));
        Ok(view)
    }
}

/// Every top-level public game field is an immutable shared subtree after
/// acceptance. Pointer identity proves it was untouched by this delta; a new
/// baseline or any changed/reordered game field must be decoded normally.
fn same_game_fields(previous: &Value, candidate: &Value) -> bool {
    let (Value::Struct(previous), Value::Struct(candidate)) = (previous.get(), candidate.get()) else {
        return false;
    };
    previous.len() == candidate.len()
        && previous.iter().zip(candidate).all(|((old_name, old), (new_name, new))| {
            old_name == new_name
                && (old_name == STATUS
                    || matches!((old, new), (Value::Shared(a), Value::Shared(b)) if Arc::ptr_eq(a, b)))
        })
}

fn session_valid(session: &str) -> Result<(), String> {
    if session.is_empty() || session.len() > 128 || session.chars().any(char::is_control) {
        return Err("invalid host view session".into());
    }
    Ok(())
}
fn status_valid(run: &Run, status: &Status) -> Result<(), String> {
    if (status.tick, status.depth, status.room) != (run.tick, run.dungeon.depth, run.at) {
        return Err("host status does not match read view".into());
    }
    Ok(())
}
fn project(run: &Run, status: &Status, fields: &mut kept::Fields) -> Result<Value, String> {
    let _public = scrolls::public_scope();
    fields.start();
    let mut out = Vec::new();
    run.serialize(kept::EachField(
        |name: &'static str, field: &dyn kept::erased::Field| {
            if PRIVATE.contains(&name)
                || name == "players"
                || ["marks", "pot_shards", "campaign", "gore"].contains(&name)
            {
                return Ok(());
            }
            out.push((
                Cow::Borrowed(name),
                if name == "book" {
                    fields.revision_field(name, field, run.book.revision as u64, |_| {})?
                } else {
                    fields.field(name, field, Some(&[]), None, |_| {})?
                },
            ));
            Ok(())
        },
    ))
    .map_err(|e| e.to_string())?;
    let mut add = |name, field: &dyn kept::erased::Field| -> Result<(), String> {
        let built = fields
            .field(name, field, Some(&[]), None, |_| {})
            .map_err(|e| e.to_string())?;
        out.push((Cow::Borrowed(name), built));
        Ok(())
    };
    add("players", &Players(&run.players))?;
    add("marks", &kept::erased::Of(&run.marks))?;
    add("pot_shards", &kept::erased::Of(&run.pot_shards))?;
    add("campaign", &kept::erased::Of(&run.campaign))?;
    add("gore", &kept::erased::Of(&run.gore))?;
    add(STATUS, &kept::erased::Of(status))?;
    Ok(Value::shared(Value::Struct(out)))
}
/// Filter at serialization/fingerprint time: native avatar bytes are neither
/// copied nor traversed, and never enter a cache awaiting later stripping.
struct Players<'a>(&'a std::collections::BTreeMap<u32, Hero>);
impl kept::erased::Field for Players<'_> {
    fn print(&self, out: &mut Vec<u8>) -> Result<(), value::Error> {
        static MASK: value::Mask = value::Mask {
            skip: HERO_PRIVATE,
            inside: &[],
        };
        value::fingerprint(&self.0.len(), out)?;
        for (id, hero) in self.0 {
            value::fingerprint(id, out)?;
            value::fingerprint_except(hero, out, &MASK)?;
        }
        Ok(())
    }
    fn build(&self) -> Result<Value, value::Error> {
        let mut players = Vec::with_capacity(self.0.len());
        for (id, hero) in self.0 {
            let mut fields = Vec::new();
            hero.serialize(kept::EachField(
                |name: &'static str, field: &dyn kept::erased::Field| {
                    if !HERO_PRIVATE.contains(&name) {
                        fields.push((Cow::Borrowed(name), field.build()?));
                    }
                    Ok(())
                },
            ))?;
            players.push((value::to_value(id)?, Value::Struct(fields)));
        }
        Ok(Value::Map(players))
    }
    fn cached(
        &self,
        node: &mut kept::Node,
        depth: u32,
        _ages: Option<u64>,
        _print: &mut Vec<u8>,
    ) -> Result<Value, value::Error> {
        Ok(node.retain_built(self.build()?, depth))
    }
}
// Only the accepted tree's bounded top-level set is retained. Pointer identity
// proves that a subtree is exactly the already-counted immutable value; names
// and the root container are counted again on every candidate.
const MAX_BUDGET_FIELDS: usize = 256;
#[derive(Clone, Default)]
struct BudgetCache(Vec<BudgetEntry>);
#[derive(Clone)]
struct BudgetEntry {
    name: value::Name,
    subtree: Arc<Value>,
    units: usize,
}
fn share_top_fields(tree: &mut Value) {
    if let Value::Struct(fields) = tree.get_mut() {
        for (_, field) in fields {
            if !matches!(field, Value::Shared(_)) {
                *field = Value::shared(std::mem::replace(field, Value::Unit));
            }
        }
    }
}
#[cfg(test)]
thread_local! { static UNIT_VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

fn validate_tree(tree: &Value, accepted: &BudgetCache) -> Result<BudgetCache, String> {
    let Value::Struct(fields) = tree.get() else {
        return Err("host view is not a struct".into());
    };
    if fields
        .iter()
        .any(|(name, _)| PRIVATE.contains(&name.as_ref()))
    {
        return Err("private state in host view".into());
    }
    let mut names = std::collections::HashSet::new();
    if fields.iter().any(|(name, _)| !names.insert(name.as_ref())) {
        return Err("duplicate host view field".into());
    }
    for (name, private) in [
        (
            "feel",
            &["ledger", "wounded", "lines", "credits", "blows", "echoes"][..],
        ),
        (
            "playtest",
            &["ready", "dirty", "changes", "local_focus"][..],
        ),
    ] {
        if tree
            .field(name)
            .is_some_and(|value| private.iter().any(|field| value.field(field).is_some()))
        {
            return Err("private output queue in host view".into());
        }
    }
    if let Some(Value::Map(players)) = tree.field("players").map(Value::get) {
        for (_, hero) in players {
            if HERO_PRIVATE.iter().any(|name| hero.field(name).is_some()) {
                return Err("native resource/private job in host view".into());
            }
            let offer = hero.field("offer").map(|v| match v.get() {
                Value::Some(inner) => inner.get(),
                other => other,
            });
            if offer.and_then(|v| v.field("binding")).is_some() {
                return Err("retained scroll source in host view".into());
            }
        }
    }
    fn units(value: &Value, left: &mut usize) -> Result<(), String> {
        #[cfg(test)]
        UNIT_VISITS.with(|visits| visits.set(visits.get() + 1));
        let size = match value.get() {
            Value::Str(s) => s.len(),
            Value::Bytes(b) => b.len(),
            _ => 16,
        };
        *left = left
            .checked_sub(size)
            .ok_or("host view exceeds retained size bound")?;
        match value.get() {
            Value::Struct(fields) => {
                for (name, value) in fields {
                    *left = left
                        .checked_sub(name.len())
                        .ok_or("host view exceeds retained size bound")?;
                    units(value, left)?;
                }
            }
            Value::Map(items) => {
                for (key, value) in items {
                    units(key, left)?;
                    units(value, left)?;
                }
            }
            Value::Seq(items) => {
                for value in items {
                    units(value, left)?;
                }
            }
            Value::Some(value) | Value::Variant(_, value) => units(value, left)?,
            _ => {}
        }
        Ok(())
    }
    let mut remaining = MAX_VIEW_UNITS.checked_sub(16)
        .ok_or("host view exceeds retained size bound")?;
    let mut candidate = BudgetCache::default();
    for (name, field) in fields {
        remaining = remaining.checked_sub(name.len())
            .ok_or("host view exceeds retained size bound")?;
        let cached = if let Value::Shared(subtree) = field {
            accepted.0.iter().find(|entry| entry.name == *name
                && Arc::ptr_eq(&entry.subtree, subtree)).map(|entry| entry.units)
        } else { None };
        let count = if let Some(count) = cached {
            remaining = remaining.checked_sub(count)
                .ok_or("host view exceeds retained size bound")?;
            count
        } else {
            let before = remaining;
            units(field, &mut remaining)?;
            before - remaining
        };
        if candidate.0.len() < MAX_BUDGET_FIELDS
            && let Value::Shared(subtree) = field
        {
            candidate.0.push(BudgetEntry { name: name.clone(), subtree: Arc::clone(subtree), units: count });
        }
    }
    Ok(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{Input, RoomKind, cards, phrasebook};
    use std::collections::BTreeMap;
    #[test]
    fn accepted_shared_budget_skips_recursion_and_preserves_exact_root_name_units() {
        let book = Value::shared(Value::Seq((0..20_000).map(Value::U).collect()));
        let tree = Value::Struct(vec![(Cow::Borrowed("book"), book.clone())]);
        UNIT_VISITS.with(|visits| visits.set(0));
        let accepted = validate_tree(&tree, &BudgetCache::default()).unwrap();
        assert_eq!(UNIT_VISITS.with(|visits| visits.get()), 20_001);
        assert_eq!(accepted.0[0].units, 16 + 20_000 * 16);
        UNIT_VISITS.with(|visits| visits.set(0));
        let next = validate_tree(&tree, &accepted).unwrap();
        assert_eq!(UNIT_VISITS.with(|visits| visits.get()), 0);
        assert!(Arc::ptr_eq(&accepted.0[0].subtree, &next.0[0].subtree));

        // Exactly root(16) + field name(4) + payload fits; one more fails.
        let exact = Value::Struct(vec![(Cow::Borrowed("book"),
            Value::shared(Value::Bytes(vec![0; MAX_VIEW_UNITS - 20].into())))]);
        let edge = validate_tree(&exact, &accepted).unwrap();
        assert_eq!(edge.0[0].units, MAX_VIEW_UNITS - 20);
        assert!(validate_tree(&exact, &edge).is_ok());
        let too_large = Value::Struct(vec![(Cow::Borrowed("book"),
            Value::shared(Value::Bytes(vec![0; MAX_VIEW_UNITS - 19].into())))]);
        assert!(validate_tree(&too_large, &accepted).is_err());
        // Failed candidates do not replace the sole accepted budget cache.
        UNIT_VISITS.with(|visits| visits.set(0));
        assert!(validate_tree(&tree, &accepted).is_ok());
        assert_eq!(UNIT_VISITS.with(|visits| visits.get()), 0);
        let unshared = Value::Struct(vec![(Cow::Borrowed("book"), book.get().clone())]);
        UNIT_VISITS.with(|visits| visits.set(0));
        assert!(validate_tree(&unshared, &accepted).is_ok());
        assert_eq!(UNIT_VISITS.with(|visits| visits.get()), 20_001);
    }

    #[test]
    fn publication_refusal_keeps_accepted_budget_and_receiver_reuses_shared_book() {
        let mut run = game();
        // A changed book travels; the built-in one stays off the view
        // (`Book::wire_omits`), and this test is about a big shared field.
        run.book.revision += 1;
        let (mut publisher, mut receiver) = pair();
        receiver.baseline(&publisher.baseline(&run, status(&run)).unwrap()).unwrap();
        let old_sender = Arc::clone(&publisher.budget.0.iter().find(|e| e.name == "book").unwrap().subtree);
        let old_reader = Arc::clone(&receiver.budget.0.iter().find(|e| e.name == "book").unwrap().subtree);
        run.tick += 1;
        publisher.limits.delta = 1;
        assert!(publisher.delta(&run, status(&run)).is_err());
        assert!(Arc::ptr_eq(&old_sender, &publisher.budget.0.iter().find(|e| e.name == "book").unwrap().subtree));
        publisher.limits = Limits::default();
        let good = publisher.delta(&run, status(&run)).unwrap();
        let mut bad = good.clone();
        bad.bytes.push(255);
        assert!(receiver.delta(&bad).is_err());
        assert!(Arc::ptr_eq(&old_reader, &receiver.budget.0.iter().find(|e| e.name == "book").unwrap().subtree));
        let mut over = receiver.tree.as_ref().unwrap().clone();
        let Value::Struct(fields) = over.get_mut() else { panic!("expected tree") };
        fields.iter_mut().find(|(name, _)| name == "book").unwrap().1 =
            Value::Seq(vec![Value::U(0); 1 << 20]);
        let mut encoded = Vec::new();
        wire::Writer::default().value(&mut encoded, &over);
        let oversized = Frame { kind: Kind::Baseline, base_revision: 0, bytes: encoded, ..good.clone() };
        let previous = Arc::clone(receiver.view().unwrap());
        assert!(receiver.baseline(&oversized).unwrap_err().contains("retained size bound"));
        assert!(Arc::ptr_eq(receiver.view().unwrap(), &previous));
        assert!(Arc::ptr_eq(&old_reader, &receiver.budget.0.iter().find(|e| e.name == "book").unwrap().subtree));
        receiver.delta(&good).unwrap();
        assert!(Arc::ptr_eq(&old_sender, &publisher.budget.0.iter().find(|e| e.name == "book").unwrap().subtree));
        assert!(Arc::ptr_eq(&old_reader, &receiver.budget.0.iter().find(|e| e.name == "book").unwrap().subtree));
    }

    fn status(run: &Run) -> Status {
        Status::capture(
            run,
            3,
            Outbox {
                home: true,
                wishes: 2,
                ..Default::default()
            },
        )
    }
    fn pair() -> (Publisher, Receiver) {
        (
            Publisher::new("view-session".into(), 9, 0, Limits::default()).unwrap(),
            Receiver::new("view-session".into(), 9, Limits::default()).unwrap(),
        )
    }
    fn game() -> Run {
        let mut run = Run::new(13, 9, Some("Two"));
        run.join_seat(3, "Three");
        run.join_seat(4, "Four");
        run
    }
    /// Opt-in receiver-only benchmark; apply independently before/after the reuse change.
    /// ANGELX_EXPECT_VIEW_REUSE=0 measures the original implementation.
    #[test]
    #[ignore = "receiver latency measurement; run explicitly in release mode"]
    fn receiver_large_book_acceptance_latency() {
        use std::time::{Duration, Instant};
        const WARMUP: usize = 64;
        const SAMPLES: usize = 512;
        let expect_reuse = std::env::var("ANGELX_EXPECT_VIEW_REUSE")
            .map(|value| value != "0").unwrap_or(true);
        fn fixture() -> Run {
            let mut run = game();
            assert_eq!(run.players.len(), 4);
            for i in 0..300 {
                let card = cards::check(
                    &format!("latency-card-{i}"),
                    "name View Spark\nkind spell\ncooldown 3\ncast bolt\n",
                ).unwrap().card;
                assert!(run.book.insert(card));
            }
            assert!(run.valid_snapshot());
            run
        }
        fn percentile(samples: &mut [Duration], percent: usize) -> Duration {
            samples.sort_unstable();
            samples[(samples.len() * percent).div_ceil(100) - 1]
        }
        // Build and encode all frames before starting the receiver timer.
        // Separate streams preserve each receiver's sequential dictionary state.
        for game_changes in [false, true] {
            let mut run = fixture();
            let (mut publisher, mut receiver) = pair();
            let baseline = publisher.baseline(&run, status(&run)).unwrap();
            let initial = receiver.baseline(&baseline).unwrap();
            let inputs = (1..=4).map(|id| (id, Input::default()))
                .collect::<BTreeMap<_, _>>();
            let mut frames = Vec::with_capacity(WARMUP + SAMPLES);
            for sample in 0..WARMUP + SAMPLES {
                if game_changes {
                    run.step(&inputs);
                } else {
                    // Owner-only activity must not enter the public Run.
                    run.cues.push("private latency cue".into());
                }
                let mut owner_status = status(&run);
                owner_status.job_revision += sample as u64 + 1;
                frames.push(publisher.delta(&run, owner_status).unwrap());
                run.cues.clear();
            }
            let mut previous = initial;
            let mut samples = Vec::with_capacity(SAMPLES);
            for (index, frame) in frames.iter().enumerate() {
                let started = Instant::now();
                let accepted = std::hint::black_box(receiver.delta(frame).unwrap());
                let elapsed = started.elapsed();
                // Keep the old view alive, so allocation address recycling cannot
                // produce a false positive; assertions stay outside the timer.
                assert_eq!(
                    std::ptr::eq(previous.run(), accepted.run()),
                    !game_changes && expect_reuse,
                    "reuse must match whether this delta changes public game state",
                );
                assert_eq!(accepted.revision(), previous.revision() + 1);
                assert!(accepted.run().cues.is_empty());
                assert_eq!(accepted.run().tick, if game_changes { index as u64 + 1 } else { 0 });
                if index >= WARMUP { samples.push(elapsed); }
                previous = accepted;
            }
            assert_eq!(receiver.tree, publisher.tree);
            let p50 = percentile(&mut samples, 50);
            let p95 = percentile(&mut samples, 95);
            eprintln!(
                "receiver acceptance: mode={} reuse={} seats=4 extra_cards=300 samples={} p50={:?} p95={:?} baseline_bytes={}",
                if game_changes { "game-changing" } else { "status-only" },
                expect_reuse, SAMPLES, p50, p95, baseline.bytes.len(),
            );
            // A fresh baseline must decode even when its logical state is equal.
            let refreshed = receiver.baseline(&publisher.baseline(&run, status(&run)).unwrap()).unwrap();
            assert!(!std::ptr::eq(previous.run(), refreshed.run()));
        }
    }

    fn shared_field<'a>(tree: &'a Value, name: &str) -> &'a Arc<Value> {
        let Value::Struct(fields) = tree.get() else {
            panic!("not a struct")
        };
        let Value::Shared(value) = &fields.iter().find(|(n, _)| n == name).unwrap().1 else {
            panic!("field not cached")
        };
        value
    }
    #[test]
    fn status_only_and_private_drains_reuse_the_accepted_run_but_game_edits_do_not() {
        let mut run = game();
        let (mut publisher, mut receiver) = pair();
        let baseline = publisher.baseline(&run, status(&run)).unwrap();
        let initial = receiver.baseline(&baseline).unwrap();
        let initial_hp = initial.run().players[&1].hp;

        // Private cues are not part of a public view; only the owner status
        // changes. The previous Run must remain usable by an older frame.
        run.cues.push("private cue".into());
        let mut next_status = status(&run);
        next_status.job_revision += 1;
        next_status.outbox.wishes += 1;
        let delta = publisher.delta(&run, next_status.clone()).unwrap();
        let next = receiver.delta(&delta).unwrap();
        assert!(Arc::ptr_eq(&initial.run, &next.run));
        assert_eq!(next.status(), &next_status);
        assert_eq!(next.revision(), initial.revision() + 1);
        assert!(next.run().cues.is_empty());
        assert_ne!(initial.status(), next.status());

        run.players.get_mut(&1).unwrap().hp -= 1;
        let delta = publisher.delta(&run, status(&run)).unwrap();
        let changed = receiver.delta(&delta).unwrap();
        assert!(!Arc::ptr_eq(&next.run, &changed.run));
        assert_eq!(changed.run().players[&1].hp, initial_hp - 1);
        assert_eq!(initial.run().players[&1].hp, initial_hp);
    }

    #[test]
    fn reused_run_never_accepts_an_invalid_status_or_advances_the_reader() {
        let run = game();
        let (mut publisher, mut receiver) = pair();
        let baseline = publisher.baseline(&run, status(&run)).unwrap();
        let initial = receiver.baseline(&baseline).unwrap();
        let mut invalid = status(&run);
        invalid.tick += 1;
        let patch = delta::Patch::Struct {
            set: vec![(Cow::Borrowed(STATUS), delta::Patch::Set(value::to_value(&invalid).unwrap()))],
            gone: vec![],
        };
        let mut writer = publisher.writer.clone();
        let mut bytes = Vec::new();
        writer.patch(&mut bytes, &patch);
        let forged = Frame {
            api: API, kind: Kind::Delta, session: "view-session".into(), raid: 9,
            base_revision: initial.revision(), revision: initial.revision() + 1, bytes,
        };
        assert!(receiver.delta(&forged).is_err());
        assert!(Arc::ptr_eq(receiver.view().unwrap(), &initial));

        let mut changed_status = status(&run);
        changed_status.job_revision += 1;
        let accepted = receiver.delta(&publisher.delta(&run, changed_status).unwrap()).unwrap();
        assert!(Arc::ptr_eq(&initial.run, &accepted.run));
    }

    #[test]
    fn public_view_validates_ecology_without_exposing_the_private_id_allocator() {
        let mut run = game();
        assert!(run.valid_snapshot());
        assert!(run.boss_gates.is_some());
        // The first floor's opening room is bare of foes now (the wishing
        // room, 2026-11-02): walk into a fight room the production way, so
        // the gates stand and the ecology reads a live foe.
        if run.enemies.is_empty() {
            let fight = run
                .dungeon
                .rooms
                .iter()
                .position(|r| r.kind == RoomKind::Fight)
                .expect("a fight room");
            run.enter(fight, Some(2));
        }
        assert!(!run.enemies.is_empty());
        assert!(run.boss_gates.is_some());
        let (mut publisher, mut receiver) = pair();
        let baseline = publisher.baseline(&run, status(&run)).unwrap();
        let view = receiver.baseline(&baseline).unwrap();
        assert!(view.run().valid_read_view());
        // The public mirror cannot pass the private allocation invariant, and
        // must never invent the missing authority just to satisfy that check.
        assert!(!view.run().valid_snapshot());
        assert!(publisher.tree.as_ref().unwrap().field("next_id").is_none());
        assert!(receiver.tree.as_ref().unwrap().field("next_id").is_none());
        let fork = receiver.clone();
        assert!(receiver.same_public_state(&fork));
        assert!(Arc::ptr_eq(receiver.view().unwrap(), fork.view().unwrap()));
    }
    #[test]
    fn four_seat_large_book_views_match_exact_owner_fields_and_keep_unchanged_subtrees() {
        let mut run = game();
        for i in 0..300 {
            let card = cards::check(
                &format!("view-card-{i}"),
                "name View Spark\nkind spell\ncooldown 3\ncast bolt\n",
            )
            .unwrap()
            .card;
            assert!(run.book.insert(card));
        }
        let (mut publisher, mut receiver) = pair();
        let baseline = publisher.baseline(&run, status(&run)).unwrap();
        receiver.baseline(&baseline).unwrap();
        let book = Arc::clone(shared_field(publisher.tree.as_ref().unwrap(), "book"));
        let retained = Arc::clone(receiver.view().unwrap());
        let initial_x = retained.run().players[&1].x;
        for tick in 0..48 {
            let inputs = (1..=4)
                .map(|id| {
                    (
                        id,
                        Input {
                            move_x: if tick < 24 { 1 } else { -1 },
                            ..Default::default()
                        },
                    )
                })
                .collect::<BTreeMap<_, _>>();
            run.step(&inputs);
            let frame = publisher.delta(&run, status(&run)).unwrap();
            assert_eq!(frame.kind, Kind::Delta);
            assert!(frame.bytes.len() < baseline.bytes.len() / 2);
            let view = receiver.delta(&frame).unwrap();
            assert_eq!(receiver.tree, publisher.tree);
            assert!(Arc::ptr_eq(
                shared_field(publisher.tree.as_ref().unwrap(), "book"),
                &book
            ));
            for id in 1..=4 {
                assert_eq!(
                    (
                        view.run().players[&id].x,
                        view.run().players[&id].y,
                        view.run().players[&id].hp,
                        view.run().players[&id].spell_cooldowns
                    ),
                    (
                        run.players[&id].x,
                        run.players[&id].y,
                        run.players[&id].hp,
                        run.players[&id].spell_cooldowns
                    )
                );
            }
            assert_eq!(view.status(), &status(&run));
            let encoded = serde_json::to_value(&frame).unwrap();
            assert!(
                encoded.get("state").is_none()
                    && encoded.get("run").is_none()
                    && encoded.get("checkpoint").is_none()
            );
            assert!(encoded["bytes"].is_array());
        }
        assert_eq!(retained.run().tick, 0);
        assert_eq!(retained.run().players[&1].x, initial_x);
    }
    #[test]
    fn retained_scroll_sources_native_resources_and_queues_never_enter_the_read_view() {
        let mut run = game();
        run.descend_for_test();
        run.players.get_mut(&1).unwrap().wishes = 3;
        let wish = phrasebook::read("wish private-view | Public Choice\nsay public choice\ntier damage 10 :: Private executable tier\n").remove(0);
        run.offer_wish(1, &wish.name, Some(&wish)).unwrap();
        let hero = run.players.get_mut(&1).unwrap();
        hero.avatar = Some(crate::realm::Avatar {
            source_id: "native-only-resource".into(),
            animation: None,
            sprite: vec![255; 1024 * 1024],
            portrait: vec![254; 1024 * 1024],
            colour: [1, 2, 3],
            pixels: vec![[1, 2, 3, 4]; 1024],
        });
        hero.reforge_job = Some("private-reforge".into());
        hero.wish_jobs.push("private-wish".into());
        run.cues.push("native-output-only".into());
        run.feel.lines.push("private-ledger-line".into());
        let (mut publisher, mut receiver) = pair();
        let baseline = publisher.baseline(&run, status(&run)).unwrap();
        assert!(baseline.bytes.len() < 1024 * 1024, "avatar bytes leaked");
        let view = receiver.baseline(&baseline).unwrap();
        assert!(run.players[&1].offer.as_ref().unwrap().binding.is_some());
        assert!(
            view.run().players[&1]
                .offer
                .as_ref()
                .unwrap()
                .binding
                .is_none()
        );
        assert!(view.run().players[&1].avatar.is_none());
        assert!(view.run().players[&1].reforge_job.is_none());
        assert!(view.run().players[&1].wish_jobs.is_empty());
        assert!(view.run().cues.is_empty() && view.run().feel.lines.is_empty());
        assert_eq!(view.status().outbox.wishes, 2);
        let players = publisher.tree.as_ref().unwrap().field("players").unwrap();
        let Value::Map(players) = players.get() else {
            panic!()
        };
        assert!(
            players
                .iter()
                .all(|(_, h)| HERO_PRIVATE.iter().all(|field| h.field(field).is_none()))
        );
        // Changing only native/private payloads leaves projected hero cache shared.
        let kept = Arc::clone(shared_field(publisher.tree.as_ref().unwrap(), "players"));
        run.players
            .get_mut(&1)
            .unwrap()
            .avatar
            .as_mut()
            .unwrap()
            .sprite[0] = 7;
        run.players
            .get_mut(&1)
            .unwrap()
            .wish_jobs
            .push("another-private-job".into());
        let delta = publisher.delta(&run, status(&run)).unwrap();
        receiver.delta(&delta).unwrap();
        assert!(Arc::ptr_eq(
            shared_field(publisher.tree.as_ref().unwrap(), "players"),
            &kept
        ));
    }
    #[test]
    fn malformed_frames_do_not_commit_view_or_reader_dictionary_and_valid_retry_still_decodes() {
        let mut run = game();
        let (mut publisher, mut receiver) = pair();
        receiver
            .baseline(&publisher.baseline(&run, status(&run)).unwrap())
            .unwrap();
        let before = Arc::clone(receiver.view().unwrap());
        let dictionary = receiver.reader.dictionary_len();
        run.tick += 1;
        run.players.get_mut(&1).unwrap().guard = cards::Guard::Shield;
        let good = publisher.delta(&run, status(&run)).unwrap();
        let mut malformed = good.clone();
        malformed.bytes.push(255); // dictionary names decoded before trailing refusal
        assert!(receiver.delta(&malformed).is_err());
        assert_eq!(receiver.reader.dictionary_len(), dictionary);
        assert!(Arc::ptr_eq(receiver.view().unwrap(), &before));
        let accepted = receiver.delta(&good).unwrap();
        assert_eq!(accepted.run().players[&1].guard, cards::Guard::Shield);
        for guard in [cards::Guard::Spin, cards::Guard::Shield, cards::Guard::Spin] {
            run.tick += 1;
            run.players.get_mut(&1).unwrap().guard = guard;
            let frame = publisher.delta(&run, status(&run)).unwrap();
            assert_eq!(
                receiver.delta(&frame).unwrap().run().players[&1].guard,
                guard
            );
            assert_eq!(receiver.tree, publisher.tree);
        }
    }
    #[test]
    fn ownership_revision_and_patch_mismatch_refuse_atomically() {
        let mut run = game();
        let (mut publisher, mut receiver) = pair();
        let baseline = publisher.baseline(&run, status(&run)).unwrap();
        receiver.baseline(&baseline).unwrap();
        run.tick += 1;
        let frame = publisher.delta(&run, status(&run)).unwrap();
        let before = Arc::clone(receiver.view().unwrap());
        let mut bad = frame.clone();
        bad.session = "another-session".into();
        assert!(receiver.delta(&bad).is_err());
        bad = frame.clone();
        bad.raid += 1;
        assert!(receiver.delta(&bad).is_err());
        bad = frame.clone();
        bad.api += 1;
        assert!(receiver.delta(&bad).is_err());
        bad = frame.clone();
        bad.base_revision += 1;
        assert!(receiver.delta(&bad).is_err());
        bad = frame.clone();
        bad.revision += 1;
        assert!(receiver.delta(&bad).is_err());
        bad = frame.clone();
        bad.bytes = vec![255];
        assert!(receiver.delta(&bad).is_err());
        assert!(Arc::ptr_eq(receiver.view().unwrap(), &before));
        receiver.delta(&frame).unwrap();
        assert!(receiver.delta(&frame).is_err());
        assert!(receiver.baseline(&baseline).is_err());
        let reset = publisher.baseline(&run, status(&run)).unwrap();
        assert_eq!(
            receiver.baseline(&reset).unwrap().revision(),
            reset.revision
        );
    }
    #[test]
    fn sender_bound_refusal_retains_revision_and_dictionary_until_a_valid_retry() {
        let mut run = game();
        let (mut publisher, mut receiver) = pair();
        receiver
            .baseline(&publisher.baseline(&run, status(&run)).unwrap())
            .unwrap();
        let old_revision = publisher.revision;
        let added = cards::check(
            "retry-book",
            "name Retry\nkind spell\ncooldown 3\ncast bolt\n",
        )
        .unwrap()
        .card;
        assert!(run.book.insert(added.clone()));
        publisher.limits.delta = 1;
        run.tick += 1;
        run.players.get_mut(&1).unwrap().guard = cards::Guard::Shield;
        assert!(publisher.delta(&run, status(&run)).is_err());
        assert_eq!(publisher.revision, old_revision);
        publisher.limits = Limits::default();
        let frame = publisher.delta(&run, status(&run)).unwrap();
        assert_eq!(
            receiver.delta(&frame).unwrap().run().players[&1].guard,
            cards::Guard::Shield
        );
        assert_eq!(
            receiver.view().unwrap().run().book.get("retry-book"),
            Some(&added)
        );
        // Explicit resync retains the exact current book and resets codec state.
        receiver
            .baseline(&publisher.baseline(&run, status(&run)).unwrap())
            .unwrap();
        assert_eq!(
            receiver.view().unwrap().run().book.get("retry-book"),
            Some(&added)
        );
        let mut exhausted =
            Publisher::new("view-session".into(), 9, u64::MAX, Limits::default()).unwrap();
        assert!(exhausted.baseline(&run, status(&run)).is_err());
        assert!(exhausted.tree.is_none());
    }
    #[test]
    fn raw_and_json_envelope_bounds_refuse_before_receiver_publication() {
        let run = game();
        let (mut publisher, mut receiver) = pair();
        let frame = publisher.baseline(&run, status(&run)).unwrap();
        let mut small =
            Receiver::new("view-session".into(), 9, Limits::new(16, 16).unwrap()).unwrap();
        assert!(small.baseline(&frame).is_err());
        assert!(small.view().is_none());
        let mut huge = frame.clone();
        huge.bytes = vec![255; MAX_BASELINE_BYTES];
        assert!(
            receiver
                .baseline(&huge)
                .unwrap_err()
                .contains("encoded frame")
        );
        assert!(receiver.view().is_none());
        huge.bytes.push(1);
        assert!(receiver.baseline(&huge).unwrap_err().contains("byte bound"));
        assert!(receiver.view().is_none());
    }
    #[test]
    fn malicious_private_or_invalid_delta_is_rejected_without_changing_the_view() {
        let run = game();
        let (mut publisher, mut receiver) = pair();
        receiver
            .baseline(&publisher.baseline(&run, status(&run)).unwrap())
            .unwrap();
        let before = Arc::clone(receiver.view().unwrap());
        let mut writer = publisher.writer.clone();
        let mut bytes = Vec::new();
        writer.patch(
            &mut bytes,
            &delta::Patch::Struct {
                set: vec![(Cow::Borrowed("rng"), delta::Patch::Set(Value::U(99)))],
                gone: Vec::new(),
            },
        );
        let bad = Frame {
            api: API,
            kind: Kind::Delta,
            session: "view-session".into(),
            raid: 9,
            base_revision: before.revision(),
            revision: before.revision() + 1,
            bytes,
        };
        assert!(receiver.delta(&bad).is_err());
        assert!(Arc::ptr_eq(receiver.view().unwrap(), &before));
        let mut bad = bad;
        let mut writer = publisher.writer.clone();
        bad.bytes.clear();
        writer.patch(
            &mut bad.bytes,
            &delta::Patch::Struct {
                set: vec![(Cow::Borrowed("dungeon"), delta::Patch::Set(Value::None))],
                gone: Vec::new(),
            },
        );
        assert!(receiver.delta(&bad).is_err());
        assert!(Arc::ptr_eq(receiver.view().unwrap(), &before));
    }
    #[test]
    fn exact_transient_display_fields_roundtrip_without_prediction_or_hidden_queues() {
        let mut run = game();
        run.marks.insert("display-mark".into(), 7);
        run.pot_shards.push((4.5, 8.25, 13));
        run.campaign = crate::rules::campaign::Campaign::parse(
            br#"{"schema":"vec.board/1","quest":{"vec_data_staged":17}}"#,
        );
        let (mut publisher, mut receiver) = pair();
        let first = receiver
            .baseline(&publisher.baseline(&run, status(&run)).unwrap())
            .unwrap();
        assert_eq!(first.run().marks, run.marks);
        assert_eq!(first.run().pot_shards, run.pot_shards);
        assert_eq!(first.run().campaign, run.campaign);
        run.tick += 37;
        run.marks.insert("display-mark".into(), 9);
        run.pot_shards[0].2 = 5;
        let next = receiver
            .delta(&publisher.delta(&run, status(&run)).unwrap())
            .unwrap();
        assert_eq!(next.run().tick, run.tick);
        assert_eq!(next.run().marks, run.marks);
        assert_eq!(next.run().pot_shards, run.pot_shards);
        assert_eq!(next.run().campaign, run.campaign);
        assert_eq!(first.run().marks["display-mark"], 7);
    }
    #[test]
    fn receiver_overflow_and_dictionary_budget_refusals_retain_the_old_baseline() {
        let run = game();
        let (mut publisher, mut receiver) = pair();
        let mut baseline = publisher.baseline(&run, status(&run)).unwrap();
        baseline.revision = u64::MAX;
        let old = receiver.baseline(&baseline).unwrap();
        let overflow = Frame {
            api: API,
            kind: Kind::Delta,
            session: "view-session".into(),
            raid: 9,
            base_revision: u64::MAX,
            revision: 0,
            bytes: vec![1, 0, 0],
        };
        assert!(receiver.delta(&overflow).is_err());
        assert!(Arc::ptr_eq(receiver.view().unwrap(), &old));
        let (_, mut receiver) = pair();
        baseline.revision = 1;
        receiver.baseline(&baseline).unwrap();
        let before = Arc::clone(receiver.view().unwrap());
        let dictionary = receiver.reader.dictionary_len();
        let mut writer = publisher.writer.clone();
        let mut bytes = Vec::new();
        let set = (0..MAX_DICTIONARY_NAMES)
            .map(|i| {
                (
                    Cow::Owned(format!("unknown-name-{i}")),
                    delta::Patch::Set(Value::Bool(true)),
                )
            })
            .collect();
        writer.patch(
            &mut bytes,
            &delta::Patch::Struct {
                set,
                gone: Vec::new(),
            },
        );
        let bad = Frame {
            api: API,
            kind: Kind::Delta,
            session: "view-session".into(),
            raid: 9,
            base_revision: 1,
            revision: 2,
            bytes,
        };
        assert!(receiver.delta(&bad).unwrap_err().contains("dictionary"));
        assert_eq!(receiver.reader.dictionary_len(), dictionary);
        assert!(Arc::ptr_eq(receiver.view().unwrap(), &before));
    }
}
