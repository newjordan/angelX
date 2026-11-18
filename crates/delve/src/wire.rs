//! The Delve on the engine's wire (`docs/ENGINE.md`, "The Delve on the
//! engine"): the pure pieces the host calls and the client reads back.
//!
//! - **Seats and knights.** The engine counts seats from 0; the run's
//!   knights are 1 to 4. Seat `n` plays knight `n + 1`, so the host's own
//!   in-process seat 0 is knight 1, the one `Run::new` starts with.
//! - **The tree** (`publish`): the run as a value tree, built once a tick,
//!   without the server's own fields (`PRIVATE`), with the fields saves
//!   leave out but the renderer reads (`DRAWN`), and with what moves by a
//!   rule the client knows sent as where it started (`reckon`). The client
//!   reads it back into a `Run` with the run's own `Deserialize` (`read`).
//!   A seat that already has the run puts back only the fields the patch
//!   named (`Guest`), and flies what a tick moves without reading it again.
//! - **Interest** (`seat_tree`): what of that tree a seat is sent.
//! - **Events** (`Event`): sounds, the chorus's lines and notices, as bytes,
//!   and the shell lines a client makes of them.
//! - **Params** (`Params`): what the welcome carries before the first
//!   snapshot.

use crate::rules::{MAX_PLAYERS, Run, activity};
use angel_net::codec::{Bytes, Malformed, put_str, put_varint};
use angel_net::doc::value::{self, Value};
use serde::Serialize;
use std::borrow::Cow;

mod flight;
mod foes;
mod guest;
mod kept;
mod knights;

pub use guest::{Guest, Shots};

pub use flight::{Course, LONGEST_TURN, SLIP, SLIP_SPEED};

pub const GAME: &str = "delve";

pub const SEATS: u32 = MAX_PLAYERS;

pub const fn knight_of(seat: u32) -> u32 {
    seat + 1
}

pub fn seat_of(knight: u32) -> Option<u32> {
    (1..=SEATS).contains(&knight).then(|| knight - 1)
}

pub const PRIVATE: [&str; 6] = ["rng", "next_id", "parked", "floor_tally", "asked", "ledger"];

pub const DRAWN: [&str; 5] = ["marks", "pot_shards", "campaign", "gore", "module_visuals"];

pub fn publish(run: &Run) -> Result<Value, value::Error> {
    let _scope = crate::rules::scrolls::public_scope();
    let mut fields = Vec::new();
    let absent = absent_of(run);
    run.serialize(kept::EachField(|name: &'static str, field: &dyn kept::erased::Field| {
        if PRIVATE.contains(&name) {
            return Ok(());
        }
        let value = if name == SHOTS {
            flight::shots_now(run)?
        } else {
            let mut value = field.build()?;
            shape(name, &mut value, run.tick, &absent);
            value
        };
        fields.push((Cow::Borrowed(name), value));
        Ok(())
    }))?;
    let mut tree = Value::Struct(fields);
    finish(&mut tree, run, None)?;
    Ok(tree)
}

const SHOTS: &str = "projectiles";

fn absent_of(run: &Run) -> Vec<u8> {
    run.players.iter().filter(|(_, h)| h.absent).flat_map(|(&id, _)| id.to_le_bytes()).collect()
}

fn shape(name: &str, value: &mut Value, tick: u64, absent: &[u8]) {
    match name {
        "players" | "exits" if !absent.is_empty() => leave_out_absent(name, value, absent),
        "enemies" => reckon_ages(value, tick),
        "feel" => reckon_feel(value, tick),
        _ => {}
    }
}

fn shape_key(name: &str, absent: &[u8]) -> Vec<u8> {
    match name {
        "players" | "exits" => absent.to_vec(),
        _ => Vec::new(),
    }
}

fn finish(tree: &mut Value, run: &Run, last: Option<&Value>) -> Result<(), value::Error> {
    let kept = |name: &str, value: Value| -> Value {
        match last {
            Some(last) => kept::intern(last.field(name), value, 1),
            None => value,
        }
    };
    let mut more = vec![
        (Cow::Borrowed(DRAWN[0]), kept(DRAWN[0], value::to_value(&run.marks)?)),
        (Cow::Borrowed(DRAWN[1]), kept(DRAWN[1], value::to_value(&run.pot_shards)?)),
        (Cow::Borrowed(DRAWN[2]), kept(DRAWN[2], value::to_value(&run.campaign)?)),
        (Cow::Borrowed(DRAWN[3]), kept(DRAWN[3], value::to_value(&run.gore)?)),
    ];
    if !run.module_visuals.is_empty() {
        let previous = last.and_then(|last| match last.get() {
            Value::Struct(fields) => fields.iter().find(|(name, _)| name == DRAWN[4]).map(|(_, value)| value),
            _ => None,
        });
        more.push((Cow::Borrowed(DRAWN[4]), kept(DRAWN[4], visual_tree(&run.module_visuals, previous)?)));
    }
    let last_item = last.and_then(|l| match l.get() {
        Value::Struct(fields) => fields.iter().find(|(n, _)| n == WORLD_ITEM).map(|(_, v)| v),
        _ => None,
    });
    if let Some(item) = world_item(run, last_item) {
        more.push((Cow::Borrowed(WORLD_ITEM), kept(WORLD_ITEM, item)));
    }
    if let Some((chunks, near)) = world_chunks(run) {
        more.push((Cow::Borrowed(WORLD_CHUNKS), kept(WORLD_CHUNKS, chunks)));
        more.push((Cow::Borrowed(WORLD_NEAR), kept(WORLD_NEAR, near)));
    }
    if let Value::Struct(fields) = tree {
        fields.extend(more);
    }
    Ok(())
}

fn visual_tree(
    scene: &crate::realm::effects::Scene,
    previous: Option<&Value>,
) -> Result<Value, value::Error> {
    if scene.revision != 0
        && previous
            .and_then(|scene| scene.field("revision"))
            .and_then(Value::as_id)
            == Some(scene.revision)
    {
        return Ok(previous.unwrap().clone());
    }
    let mut graphs = Vec::new();
    for (id, graph) in &scene.graphs {
        let old = previous
            .and_then(|scene| scene.field("graphs"))
            .and_then(|graphs| match graphs.get() {
                Value::Map(fields) => fields
                    .iter()
                    .find(|(key, _)| matches!(key.get(), Value::Str(name) if name.as_ref() == id.as_str()))
                    .map(|(_, value)| value),
                _ => None,
            });
        let graph = match old {
            Some(graph) => Value::shared(graph.clone()),
            None => Value::shared(value::to_value(graph)?),
        };
        graphs.push((Value::Str(id.clone().into_boxed_str()), graph));
    }
    Ok(Value::shared(Value::Struct(vec![
        (Cow::Borrowed("revision"), Value::U(scene.revision)),
        (Cow::Borrowed("graphs"), Value::Map(graphs)),
        (Cow::Borrowed("effects"), value::to_value(&scene.effects)?),
    ])))
}

pub const WORLD_ITEM: &str = "world3d";

fn world_item(run: &Run, last: Option<&Value>) -> Option<Value> {
    let field = |name: &'static str, value: Value| (Cow::Borrowed(name), value);
    if !crate::world3d::room::stands_here(run) {
        return None;
    }
    let lit = crate::world3d::rooms_lit::lit_for(run.room(), run.dungeon.pack)?;
    if let Some(last) = last.filter(|l| l.field("key") == Some(&Value::U(lit.key))) {
        return Some(last.clone());
    }
    let text = lit.text()?;
    Some(Value::Struct(vec![
        field("id", Value::Str(lit.id.into())),
        field("key", Value::U(lit.key)),
        field("set", Value::Str(text.as_ref().into())),
    ]))
}

pub const WORLD_CHUNKS: &str = "world3d_chunks";
pub const WORLD_NEAR: &str = "world3d_near";

fn world_chunks(run: &Run) -> Option<(Value, Value)> {
    use crate::world3d::kingdom;
    if !crate::world3d::enabled() || run.room().kind != crate::rules::RoomKind::Kingdom {
        return None;
    }
    let unit = crate::render::overworld::arena::UNIT;
    let view = (crate::render::overworld::arena::NATIVE_W as f32, crate::render::overworld::arena::NATIVE_H as f32);
    let registry = kingdom::chunks();
    let mut near = Vec::new();
    let mut all: Vec<u32> = Vec::new();
    for (&id, hero) in &run.players {
        let (cx, cy) = hero.camera.unwrap_or((hero.x, hero.y - 1.0));
        let mut mine = kingdom::near_centre((cx * unit, cy * unit), view);
        let away = |i: u32| {
            let (col, row) = kingdom::cell(i);
            let (x, z) = ((col as f32 + 0.5) * kingdom::STAGE.0, (row as f32 + 0.5) * kingdom::STAGE.1);
            (x - cx * unit).hypot(z - cy * unit)
        };
        mine.sort_by(|&a, &b| away(a).total_cmp(&away(b)));
        all.extend(mine.iter().filter(|i| !all.contains(i)).copied().collect::<Vec<_>>());
        near.push((Value::U(u64::from(id)), Value::Seq(mine.iter().map(|&i| Value::U(u64::from(i))).collect())));
    }
    all.sort_unstable();
    let entries = registry.of(&all).iter().map(|c| (Value::U(u64::from(c.index)), chunk_value(c))).collect();
    Some((Value::Map(entries), Value::Map(near)))
}

fn chunk_value(chunk: &crate::world3d::kingdom::Chunk) -> Value {
    type Kept = std::sync::Mutex<Vec<(u64, Value)>>;
    static KEPT: Kept = std::sync::Mutex::new(Vec::new());
    if let Ok(kept) = KEPT.lock()
        && let Some((_, value)) = kept.iter().find(|(k, _)| *k == chunk.key)
    {
        return value.clone();
    }
    let field = |name: &'static str, value: Value| (Cow::Borrowed(name), value);
    let value = Value::Struct(vec![field("key", Value::U(chunk.key)), field("set", Value::Bytes(chunk.text.as_bytes().into()))]);
    if let Ok(mut kept) = KEPT.lock() {
        if kept.len() >= crate::world3d::kingdom::COUNT {
            kept.remove(0);
        }
        kept.push((chunk.key, value.clone()));
    }
    value
}

pub fn take_world_chunks(tree: &Value) -> usize {
    let registry = crate::world3d::kingdom::chunks();
    let Some(Value::Map(entries)) = tree.field(WORLD_CHUNKS) else {
        registry.keep(&[]);
        return 0;
    };
    let mut kept = Vec::with_capacity(entries.len());
    for (index, chunk) in entries {
        let (Some(index), Some(Value::U(key)), Some(Value::Bytes(set))) = (index.as_id(), chunk.field("key"), chunk.field("set")) else {
            continue;
        };
        let Ok(set) = std::str::from_utf8(set) else { continue };
        if registry.receive(index as u32, *key, set) {
            kept.push(index as u32);
        }
    }
    registry.keep(&kept);
    kept.len()
}

pub fn take_world_item(tree: &Value) -> bool {
    let Some(item) = tree.field(WORLD_ITEM) else {
        return false;
    };
    let (Some(Value::U(key)), Some(Value::Str(set))) = (item.field("key"), item.field("set")) else {
        return false;
    };
    crate::world3d::rooms_lit::receive(*key, set)
}

fn leave_out_absent(name: &str, value: &mut Value, absent: &[u8]) {
    let gone = |key: &Value| {
        key.as_id()
            .is_some_and(|id| absent.chunks_exact(4).any(|a| u64::from(u32::from_le_bytes([a[0], a[1], a[2], a[3]])) == id))
    };
    let map = match name {
        "players" => Some(value.get_mut()),
        _ => value.field_mut("outside").map(Value::get_mut),
    };
    if let Some(Value::Map(entries)) = map {
        entries.retain(|(key, _)| !gone(key));
    }
}

#[derive(Default)]
pub struct Publisher {
    fields: kept::Fields,
    flights: flight::Flights,
    foes: foes::Foes,
    knights: knights::Knights,
    last: Option<Value>,
}

pub const SAME_SHOT: f32 = flight::SAME_SHOT;

impl Publisher {
    pub fn publish(&mut self, run: &Run) -> Result<Value, value::Error> {
        let _scope = crate::rules::scrolls::public_scope();
        let tick = run.tick;
        let absent = absent_of(run);
        let shots = self.flights.publish(run)?;
        let mut shots = Some(shots);
        let enemies = self.foes.publish(run)?;
        let mut enemies = Some(enemies);
        let players = self.knights.publish(run, &absent)?;
        let mut players = Some(players);
        let mut out = Vec::with_capacity(160);
        let fields = &mut self.fields;
        fields.start();
        run.serialize(kept::EachField(|name: &'static str, field: &dyn kept::erased::Field| {
            if PRIVATE.contains(&name) {
                return Ok(());
            }
            let value = if name == SHOTS {
                shots.take().unwrap_or(Value::Seq(Vec::new()))
            } else if name == "enemies" {
                enemies.take().unwrap_or(Value::Seq(Vec::new()))
            } else if name == "players" {
                players.take().unwrap_or(Value::Map(Vec::new()))
            } else {
                match name {
                    "feel" => fields.field(name, field, None, None, |v| reckon_feel(v, tick))?,
                    "caves" => {
                        let key = run.caves.fingerprint().to_le_bytes();
                        fields.field(name, field, Some(&key), None, |v| shape(name, v, tick, &absent))?
                    }
                    _ => fields.field(name, field, Some(&shape_key(name, &absent)), None, |v| shape(name, v, tick, &absent))?,
                }
            };
            out.push((Cow::Borrowed(name), value));
            Ok(())
        }))?;
        let mut tree = Value::Struct(out);
        finish(&mut tree, run, self.last.as_ref())?;
        if let Value::Struct(fields) = &mut tree {
            if run.projectiles.iter().any(|s| !s.hostile && s.traits.homing > 0) {
                fields.push((Cow::Borrowed(flight::AIM), self.flights.aim_sent()));
            }
        }
        let tree = Value::shared(tree);
        self.last = Some(tree.clone());
        Ok(tree)
    }
}

const LONGEST_TICK: u64 = crate::rules::runtime_state::MAX_TICK;

pub fn read(tree: &Value) -> Result<Run, value::Error> {
    let tick = tree.field("tick").and_then(Value::as_id).unwrap_or(0);
    if tick > LONGEST_TICK {
        return Err(serde::de::Error::custom("tick is not a session"));
    }
    let marks = flight::marks_of_tree(tree);
    let aim = flight::aim_of(tree);
    let open = |_: (f32, f32), _: (f32, f32), _: f32| None;
    let shots = tree.field(SHOTS).map(|shots| flight::shots_at(shots, tick, &marks, &open, &aim));
    let feel = tree.field("feel").map(|feel| unreckon_feel(feel, tick));
    let foes = tree.field("enemies").filter(|e| foes::needs_land(e)).map(|e| foes::land(e, tick));
    let knights = tree.field("players").filter(|p| knights::needs_land(p)).map(|p| knights::land(p, tick));
    let mut over: Vec<(&str, &Value)> = Vec::with_capacity(4);
    if let Some(shots) = &shots {
        over.push((SHOTS, shots));
    }
    if let Some(feel) = &feel {
        over.push(("feel", feel));
    }
    if let Some(foes) = &foes {
        over.push(("enemies", foes));
    }
    if let Some(knights) = &knights {
        over.push(("players", knights));
    }
    let mut run: Run = value::from_value_over(tree, &over)?;
    if let Some(marks) = tree.field(DRAWN[0]) {
        run.marks = value::from_value(marks)?;
    }
    if let Some(shards) = tree.field(DRAWN[1]) {
        run.pot_shards = value::from_value(shards)?;
    }
    if let Some(campaign) = tree.field(DRAWN[2]) {
        run.campaign = value::from_value(campaign)?;
    }
    if let Some(gore) = tree.field(DRAWN[3]) {
        run.gore = value::from_value(gore)?;
    }
    if let Some(scene) = tree.field(DRAWN[4]) {
        run.module_visuals = value::from_value(scene)?;
        run.module_visuals.validate().map_err(serde::de::Error::custom)?;
    }
    for monster in &mut run.enemies {
        monster.age = (run.tick as u32).wrapping_sub(monster.age);
    }
    if !room_can_stop_shot(&run) {
        return Err(serde::de::Error::custom("the room cannot stop a shot"));
    }
    if let Some(shots) = tree.field(SHOTS) {
        let placed = flight::shots_at(shots, tick, &marks, &|from, to, body| run.shot_stone(from, to, body), &aim);
        run.projectiles = value::from_value(&placed)?;
    }
    run.choose_altars();
    Ok(run)
}

fn reckon_ages(enemies: &mut Value, tick: u64) {
    if let Value::Seq(monsters) = enemies.get_mut() {
        for monster in monsters {
            if let Some(&Value::U(age)) = monster.field("age") {
                set(monster, "age", Value::U(tick.wrapping_sub(age) & u64::from(u32::MAX)));
            }
        }
    }
}

const FADES: [&str; 11] = ["ranged", "impacts", "tracers", "puffs", "deaths", "blocks", "glints", "takes", "bolts", "swings", "hits"];

fn reckon_feel(feel: &mut Value, tick: u64) {
    if let Some(melee) = feel.field_mut("melee") { reckon_feel(melee, tick); }
    for list in FADES {
        if let Some(Value::Seq(items)) = feel.field_mut(list).map(Value::get_mut) {
            for item in items {
                if let Some(&Value::U(ttl)) = item.field("ttl") {
                    set(item, "ttl", Value::U(tick + ttl));
                }
            }
        }
    }
}

fn unreckon_feel(feel: &Value, tick: u64) -> Value {
    let mut feel = feel.get().clone();
    for list in FADES {
        if let Some(Value::Seq(items)) = feel.field_mut(list) {
            for item in items {
                if let Some(&Value::U(end)) = item.field("ttl") {
                    set(item, "ttl", Value::U(end.saturating_sub(tick)));
                }
            }
        }
    }
    if let Some(melee) = feel.field_mut("melee") { *melee = unreckon_feel(melee, tick); }
    feel
}

fn set(item: &mut Value, name: &str, to: Value) {
    if let Some(field) = item.field_mut(name) {
        *field = to;
    }
}

fn room_can_stop_shot(run: &Run) -> bool {
    crate::rules::session::frame::room_can_stop_shot(run)
}

pub fn drawable(run: &Run) -> bool {
    crate::rules::session::frame::drawable(run)
}

pub fn seat_tree(tree: &Value, seat: Option<u32>) -> Cow<'_, Value> {
    let knight = seat.map(knight_of);
    if tree.field(WORLD_NEAR).is_some() {
        let mut view = tree.clone();
        let near: Vec<Value> = match (view.field(WORLD_NEAR), knight) {
            (Some(Value::Map(near)), Some(k)) => near
                .iter()
                .find(|(key, _)| key.as_id() == Some(u64::from(k)))
                .and_then(|(_, chunks)| match chunks {
                    Value::Seq(chunks) => Some(chunks.clone()),
                    _ => None,
                })
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        if let Some(Value::Map(chunks)) = view.field_mut(WORLD_CHUNKS) {
            let mut mine = Vec::with_capacity(near.len());
            for index in &near {
                if let Some(at) = chunks.iter().position(|(i, _)| i == index) {
                    mine.push(chunks.swap_remove(at));
                }
            }
            *chunks = mine;
        }
        if let Value::Struct(fields) = view.get_mut() {
            fields.retain(|(name, _)| name != WORLD_NEAR);
        }
        let shared = seat_tree(&view, seat).into_owned();
        return Cow::Owned(shared);
    }
    let knight = seat.map(knight_of);
    let mine = |key: &Value| knight.is_some_and(|k| key.as_id() == Some(u64::from(k)));
    let others_heats = map_of(tree.field("pastimes").and_then(|p| p.field("heats")))
        .is_some_and(|heats| heats.iter().any(|(key, _)| !mine(key)));
    let outside = map_of(tree.field("exits").and_then(|e| e.field("outside")))
        .is_some_and(|outside| outside.iter().any(|(key, _)| mine(key)));
    let others_numbers = map_of(tree.field("kingdom").and_then(|k| k.field("industry")))
        .is_some_and(|boards| boards.iter().any(|(key, b)| !mine(key) && b.field("rig").is_some_and(|r| r.field("numbers").is_some())));
    if !others_heats && !outside && !others_numbers {
        return Cow::Borrowed(tree);
    }
    let mut view = tree.clone();
    if others_numbers
        && let Some(Value::Map(boards)) = view.field_mut("kingdom").and_then(|k| k.field_mut("industry"))
    {
        for (key, board) in boards.iter_mut().filter(|(key, _)| !mine(key)) {
            let _ = key;
            if let Some(Value::Struct(fields)) = board.field_mut("rig").map(Value::get_mut) {
                fields.retain(|(name, _)| name != "numbers");
            }
        }
    }
    if others_heats
        && let Some(Value::Map(heats)) = view.field_mut("pastimes").and_then(|p| p.field_mut("heats"))
    {
        heats.retain(|(key, _)| mine(key));
    }
    if outside {
        if let Some(shots) = view.slot_mut("projectiles")
            && matches!(shots.get(), Value::Seq(_))
        {
            *shots = Value::Seq(Vec::new());
        }
        for name in activity::all().flat_map(|a| a.fields.iter()) {
            if let Some(game) = view.slot_mut(name)
                && matches!(game.get(), Value::Some(_))
            {
                *game = Value::None;
            }
        }
    }
    Cow::Owned(view)
}

pub const CHUNK_BYTES_A_SNAPSHOT: usize = 2 << 20;

pub fn pace<'a>(view: Cow<'a, Value>, held: Option<&Value>) -> Cow<'a, Value> {
    let Some(Value::Map(entries)) = view.field(WORLD_CHUNKS) else {
        return view;
    };
    let had = map_of(held.and_then(|h| h.field(WORLD_CHUNKS))).unwrap_or(&[]);
    let had_of = |index: &Value| had.iter().find(|(i, _)| i == index).map(|(_, chunk)| chunk);
    let (mut room, mut sent, mut held_back) = (CHUNK_BYTES_A_SNAPSHOT, 0, false);
    let mut paced = Vec::with_capacity(entries.len());
    for (index, chunk) in entries {
        let was = had_of(index);
        if was.is_some_and(|was| was.field("key") == chunk.field("key")) {
            paced.push((index.clone(), chunk.clone()));
            continue;
        }
        let size = chunk_bytes(chunk);
        if size <= room || sent == 0 {
            room = room.saturating_sub(size);
            sent += 1;
            paced.push((index.clone(), chunk.clone()));
        } else {
            held_back = true;
            if let Some(was) = was {
                paced.push((index.clone(), was.clone()));
            }
        }
    }
    if !held_back {
        return view;
    }
    let mut view = view.into_owned();
    if let Some(Value::Map(entries)) = view.field_mut(WORLD_CHUNKS) {
        *entries = paced;
    }
    Cow::Owned(view)
}

fn chunk_bytes(chunk: &Value) -> usize {
    match chunk {
        Value::Struct(fields) => fields
            .iter()
            .map(|(_, value)| match value {
                Value::Bytes(bytes) => bytes.len(),
                Value::Str(text) => text.len(),
                _ => 8,
            })
            .sum(),
        _ => 0,
    }
}

fn map_of(value: Option<&Value>) -> Option<&[(Value, Value)]> {
    match value {
        Some(Value::Map(entries)) => Some(entries),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hear {
    Party,
    All,
    Knight(u32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Sound { hear: Hear, name: String },
    Say { hear: Hear, who: String, name: String, words: String },
    Notice { hear: Hear, text: String },
}

const SOUND: u8 = 1;
const SAY: u8 = 2;
const NOTICE: u8 = 3;
const HEAR_PARTY: u8 = 0;
const HEAR_ALL: u8 = 1;
const HEAR_KNIGHT: u8 = 2;
const MAX_NAME: usize = 64;
const MAX_WORDS: usize = 512;

impl Event {
    pub fn hear(&self) -> Hear {
        match self {
            Event::Sound { hear, .. } | Event::Say { hear, .. } | Event::Notice { hear, .. } => *hear,
        }
    }

    pub fn encode(&self, out: &mut Vec<u8>) {
        let kind = match self {
            Event::Sound { .. } => SOUND,
            Event::Say { .. } => SAY,
            Event::Notice { .. } => NOTICE,
        };
        out.push(kind);
        match self.hear() {
            Hear::Party => out.push(HEAR_PARTY),
            Hear::All => out.push(HEAR_ALL),
            Hear::Knight(k) => {
                out.push(HEAR_KNIGHT);
                put_varint(out, u64::from(k));
            }
        }
        match self {
            Event::Sound { name, .. } => put_str(out, clip(name, MAX_NAME)),
            Event::Say { who, name, words, .. } => {
                put_str(out, clip(who, MAX_NAME));
                put_str(out, clip(name, MAX_NAME));
                put_str(out, clip(words, MAX_WORDS));
            }
            Event::Notice { text, .. } => put_str(out, clip(text, MAX_WORDS)),
        }
    }

    pub fn to_vec(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.encode(&mut out);
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Event, Malformed> {
        let mut b = Bytes::new(bytes);
        let kind = b.byte()?;
        let hear = match b.byte()? {
            HEAR_PARTY => Hear::Party,
            HEAR_ALL => Hear::All,
            HEAR_KNIGHT => Hear::Knight(b.u32()?),
            _ => return Err(Malformed),
        };
        let event = match kind {
            SOUND => Event::Sound { hear, name: b.str(MAX_NAME)?.to_string() },
            SAY => Event::Say {
                hear,
                who: b.str(MAX_NAME)?.to_string(),
                name: b.str(MAX_NAME)?.to_string(),
                words: b.str(MAX_WORDS)?.to_string(),
            },
            NOTICE => Event::Notice { hear, text: b.str(MAX_WORDS)?.to_string() },
            _ => return Err(Malformed),
        };
        b.end()?;
        Ok(event)
    }

    pub fn line(&self, knight: Option<u32>, run: Option<&Run>) -> Option<String> {
        let heard = match self.hear() {
            Hear::All => true,
            Hear::Knight(k) => knight == Some(k),
            Hear::Party => !knight.is_some_and(|k| run.is_some_and(|r| r.exits.outside.contains_key(&k))),
        };
        if !heard {
            return None;
        }
        Some(match self {
            Event::Sound { name, .. } => format!("sfx {}", sound_file(name)),
            Event::Say { name, words, .. } => format!("say {}: {}", one_line(name), one_line(words)),
            Event::Notice { text, .. } => format!("notice {}", one_line(text)),
        })
    }
}

pub fn sound_file(name: &str) -> String {
    let name = STAND_INS.iter().find(|(hook, _)| *hook == name).map_or(name, |(_, take)| *take);
    name.replace('/', "-")
}

pub const STAND_INS: &[(&str, &str)] = &[
    ("shoulder_climb", "hand_grab"),
    ("shoulder_hop", "roll"),
    ("shoulder_throw", "bomb_toss"),
    ("shoulder_slam", "hit_blast"),
];

pub fn music_file(track: &str) -> String {
    format!("music-{track}")
}

pub fn take_sounds(run: &mut Run) -> Vec<Event> {
    crate::rules::session::frame::take_sounds(run).into_iter().map(portable_signal).collect()
}

pub fn take_notices(run: &mut Run) -> Vec<Event> {
    crate::rules::session::frame::take_notices(run).into_iter().map(portable_signal).collect()
}
fn portable_signal(signal: crate::rules::session::frame::Signal) -> Event {
    use crate::rules::session::frame::{Hear as Audience, Signal};
    let hear = |audience| match audience { Audience::Party => Hear::Party, Audience::All => Hear::All, Audience::Knight(player) => Hear::Knight(player) };
    match signal {
        Signal::Sound { hear: audience, name } => Event::Sound { hear: hear(audience), name },
        Signal::Notice { hear: audience, text } => Event::Notice { hear: hear(audience), text },
    }
}

fn clip(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

fn one_line(text: &str) -> Cow<'_, str> {
    if text.contains(['\n', '\r']) {
        Cow::Owned(text.replace(['\n', '\r'], " "))
    } else {
        Cow::Borrowed(text)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Params {
    pub world3d: bool,
    pub deep3d: bool,
    pub atari: bool,
}

pub const PARAMS_VERSION: u64 = 2;
const WORLD3D: u64 = 2;
const DEEP3D: u64 = 4;
const ATARI: u64 = 8;

impl Params {
    pub fn of(_settings: crate::settings::Settings) -> Params {
        Params::with_mode(crate::world3d::room::mode())
    }

    pub fn with_mode(mode: crate::world3d::room::Mode) -> Params {
        use crate::world3d::room::Mode;
        Params {
            world3d: mode == Mode::Always,
            deep3d: mode == Mode::Deep,
            atari: crate::render::overworld::arena::atari::on(),
        }
    }

    pub fn mode(&self) -> crate::world3d::room::Mode {
        use crate::world3d::room::Mode;
        if self.world3d {
            Mode::Always
        } else if self.deep3d {
            Mode::Deep
        } else {
            Mode::Off
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        put_varint(&mut out, PARAMS_VERSION);
        let flags = if self.world3d { WORLD3D } else { 0 }
            | if self.deep3d { DEEP3D } else { 0 }
            | if self.atari { ATARI } else { 0 };
        put_varint(&mut out, flags);
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Params, Malformed> {
        let mut b = Bytes::new(bytes);
        if b.varint()? != PARAMS_VERSION {
            return Err(Malformed);
        }
        let flags = b.varint()?;
        if flags & !(WORLD3D | DEEP3D | ATARI) != 0 {
            return Err(Malformed);
        }
        b.end()?;
        Ok(Params {
            world3d: flags & WORLD3D != 0,
            deep3d: flags & DEEP3D != 0,
            atari: flags & ATARI != 0,
        })
    }
}

#[cfg(test)]
#[path = "../tests/wire__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/wire__sounds__tests.rs"]
mod sounds_tests;
