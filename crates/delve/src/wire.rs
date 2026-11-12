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

/// The game's name in `Hello` and `Welcome`.
pub const GAME: &str = "delve";

/// The party's seats on the engine: one a knight.
pub const SEATS: u32 = MAX_PLAYERS;

/// The knight seat `seat` plays.
pub const fn knight_of(seat: u32) -> u32 {
    seat + 1
}

/// The seat that plays knight `knight`, if a seat does.
pub fn seat_of(knight: u32) -> Option<u32> {
    (1..=SEATS).contains(&knight).then(|| knight - 1)
}

/// The run's fields only the server keeps: the dice and the next id. A
/// client never steps the rules, so it never needs them, and they are never
/// sent (they would let a player read the dice ahead). They read back as
/// their defaults (`Run`'s `serde(default)`). The run's seed is sent: the
/// renderer places the merchant's camp and each room's pots by it
/// (`Run::merchant_camp`, `Run::pots_of`). An open floor's companies kept
/// while the party is in a side room (`Run::parked`) are the server's too:
/// nobody sees them until the party is back among them. The floor's tally
/// is the server's as it is counted: a client is sent its results. Words a
/// knight spent a wish on, waiting for the host to teach them (`Run::asked`),
/// are the server's too: a client never teaches them, and a restart reads
/// them from the save, not from the wire.
pub const PRIVATE: [&str; 6] = ["rng", "next_id", "parked", "floor_tally", "asked", "ledger"];

/// The run's fields its saves leave out (`serde(skip)`) that the renderer
/// reads: the marks since the realm last settled (the countryside's and the
/// holdings' stages are drawn from them), the shards of broken pots, and the
/// campaign the heart questline mirrors (the sheet and the Lantern Hall's
/// lanterns are drawn from it), and the finishers of the monsters just felled
/// (`rules::gore`; a seat that predates them ignores the field and draws the
/// ordinary death). The tree carries them beside the run's own
/// fields, and `read` puts them back, so a save's format is unchanged.
pub const DRAWN: [&str; 5] = ["marks", "pot_shards", "campaign", "gore", "module_visuals"];

/// The run as the tick's tree, built once for every seat: everything the
/// renderer reads (`DRAWN` included), nothing in `PRIVATE`, and what moves
/// by rule as where it started (`reckon`, `flight`). Built afresh, its
/// shots with no ids and each on a course from where it is now: a host
/// publishing tick after tick keeps a `Publisher`.
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

/// The tree's field of the run's shots.
const SHOTS: &str = "projectiles";

/// The knights nobody's at all (`leave_out_absent`), as the bytes a field's
/// shape depends on.
fn absent_of(run: &Run) -> Vec<u8> {
    run.players.iter().filter(|(_, h)| h.absent).flat_map(|(&id, _)| id.to_le_bytes()).collect()
}

/// Field `name` of the run's tree as sent rather than as the run holds it:
/// without the knights nobody's at (`leave_out_absent`), and what runs a
/// tick at a time as the tick it began or ends (`reckon`).
fn shape(name: &str, value: &mut Value, tick: u64, absent: &[u8]) {
    match name {
        "players" | "exits" if !absent.is_empty() => leave_out_absent(name, value, absent),
        "enemies" => reckon_ages(value, tick),
        "feel" => reckon_feel(value, tick),
        _ => {}
    }
}

/// What a field's shape depends on beside the field (`shape`): the knights
/// nobody's at. (The monsters and the feel are built every tick.)
fn shape_key(name: &str, absent: &[u8]) -> Vec<u8> {
    match name {
        "players" | "exits" => absent.to_vec(),
        _ => Vec::new(),
    }
}

/// The fields after the run's own: `DRAWN`, and the room's 3D item, theme
/// and chunks while there are any; each kept from `last` (the last tree)
/// when unchanged.
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
    // Immutable graph identities cache the expensive sprite data. Instances
    // carry birth ticks, so animation needs no per-frame graph/age update.
    if !run.module_visuals.is_empty() {
        let previous = last.and_then(|last| match last.get() {
            Value::Struct(fields) => fields.iter().find(|(name, _)| name == DRAWN[4]).map(|(_, value)| value),
            _ => None,
        });
        more.push((Cow::Borrowed(DRAWN[4]), kept(DRAWN[4], visual_tree(&run.module_visuals, previous)?)));
    }
    // What the last tree sent as the room's item, as it stands there
    // (shared).
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

/// The tree's field holding the lit set of the room on show (`world_item`).
pub const WORLD_ITEM: &str = "world3d";

/// The room on show's lit set (`world3d::rooms_lit`), for a page across a
/// wire, which has no asset folder for a place's landmark: its dressing's
/// id, its key and its set's text, no plate (a page makes an open floor's
/// pieces itself, from the run's tiles). Only while the room stands up in
/// 3D; the tree's delta sends it again only when the room changes.
///
/// `last` is what the last tick sent (a host's `Publisher`): sent again as
/// it was while the set's key is the same, its text not copied again.
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

/// The tree's field holding the kingdom's chunks near any knight's view
/// (`world_chunks`), and the one saying which are near which knight; a
/// seat's tree keeps only its own knight's (`seat_tree`), and not the second.
pub const WORLD_CHUNKS: &str = "world3d_chunks";
pub const WORLD_NEAR: &str = "world3d_near";

/// The kingdom's chunks near each knight's view, for a page across a wire
/// (`world3d::kingdom`, "A page across a wire"): a map of chunk number to
/// its key and its set's text (the realm, lit: no plates, docs/REALM-MONOLITH.md),
/// the chunks near any knight; and a map of knight to the
/// numbers of the chunks near it, the nearest its view's middle first (the
/// order a seat is sent them in, `pace`). Only while the world is drawn in
/// 3D and the party is out in the kingdom. Each entry is the same blob from
/// tick to tick, so the tree's delta sends a chunk once, when it comes near.
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

/// A chunk as a tree carries it, made once a chunk's key (its blobs shared).
fn chunk_value(chunk: &crate::world3d::kingdom::Chunk) -> Value {
    type Kept = std::sync::Mutex<Vec<(u64, Value)>>;
    static KEPT: Kept = std::sync::Mutex::new(Vec::new());
    if let Ok(kept) = KEPT.lock()
        && let Some((_, value)) = kept.iter().find(|(k, _)| *k == chunk.key)
    {
        return value.clone();
    }
    let field = |name: &'static str, value: Value| (Cow::Borrowed(name), value);
    // Set text only: the page fills it lit itself.
    let value = Value::Struct(vec![field("key", Value::U(chunk.key)), field("set", Value::Bytes(chunk.text.as_bytes().into()))]);
    if let Ok(mut kept) = KEPT.lock() {
        if kept.len() >= crate::world3d::kingdom::COUNT {
            kept.remove(0);
        }
        kept.push((chunk.key, value.clone()));
    }
    value
}

/// The chunks a tree carries (`world_chunks`) kept, and only those, so the
/// kingdom can be drawn in 3D where no asset folder is; none when it
/// carries none. How many it carries.
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

/// The lit room's set a tree carries (`world_item`) kept, so a place's
/// landmark is drawn where no asset folder is: false when it carries none,
/// or one that does not read.
pub fn take_world_item(tree: &Value) -> bool {
    let Some(item) = tree.field(WORLD_ITEM) else {
        return false;
    };
    let (Some(Value::U(key)), Some(Value::Str(set))) = (item.field("key"), item.field("set")) else {
        return false;
    };
    crate::world3d::rooms_lit::receive(*key, set)
}

/// A knight nobody's at all (`Hero::absent`: the place of a host who plays
/// no knight) is no part of the party any screen shows: it is not sent.
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

/// The tick's tree built from the last one, published every tick by the
/// host's one `Publisher`:
///
/// - each field of the run is built again only when it changed, keeping
///   every unchanged part, and every part is shared between the trees
///   (`kept`), so a seat's view and a client's diff cost what changed;
/// - each shot in flight carries an `id` that stays with it, so a list of
///   shots is sent by id, and is sent when it is loosed and when its flight
///   leaves its course, not every tick (`flight`);
/// - each foe is sent the same way: on a course while it walks a straight
///   line or stands, and its timer as the tick it counts from (`foes`);
/// - each knight is sent the same way: on a course while that step still
///   lands, a counter or a cooldown as the tick it counts from, and a field
///   at its serde default left out (`knights`).
#[derive(Default)]
pub struct Publisher {
    fields: kept::Fields,
    flights: flight::Flights,
    foes: foes::Foes,
    knights: knights::Knights,
    last: Option<Value>,
}

/// How far from where its course has it a shot may be found and still be
/// itself: a curving shot turns, a bouncing one comes off a wall.
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
        // The tick's tree is shared whole too: each seat sent it as it
        // stands holds it for nothing, and the seats sent the same two
        // trees in a row are sent one patch (`angel_serve::rep`).
        let tree = Value::shared(tree);
        self.last = Some(tree.clone());
        Ok(tree)
    }
}

/// A tick past this is not a session a seat can fly or draw. The closed form
/// of a course that far out is not finite, and drawing it does not finish.
const LONGEST_TICK: u64 = crate::rules::runtime_state::MAX_TICK;

/// A tree from the wire read back into a run, `DRAWN` included and what
/// `reckon`, `flight`, `foes` and `knights` sent as its start brought to the tick.
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
    // `shot_stone` indexes the room. A damaged tree is refused here, not
    // panicked inside the stop.
    if !room_can_stop_shot(&run) {
        return Err(serde::de::Error::custom("the room cannot stop a shot"));
    }
    // The first pass flew shots through open air so the room could be
    // read. Place them again on its stone, the same stop the host uses
    // when it decides a course still holds.
    if let Some(shots) = tree.field(SHOTS) {
        let placed = flight::shots_at(shots, tick, &marks, &|from, to, body| run.shot_stone(from, to, body), &aim);
        run.projectiles = value::from_value(&placed)?;
    }
    // An old save has no altar spot. Pick it once, the search the host
    // would have stood when the room was entered.
    run.choose_altars();
    Ok(run)
}

/// A monster's age (one a tick), sent as the tick it was born: it is sent
/// when it comes, not every tick.
fn reckon_ages(enemies: &mut Value, tick: u64) {
    if let Value::Seq(monsters) = enemies.get_mut() {
        for monster in monsters {
            if let Some(&Value::U(age)) = monster.field("age") {
                set(monster, "age", Value::U(tick.wrapping_sub(age) & u64::from(u32::MAX)));
            }
        }
    }
}

/// The lists of what fades a tick at a time (`feel`: impacts, tracers,
/// puffs and the rest), each one's life sent as the tick it ends: an effect
/// is sent when it comes, and the list moves up when the oldest go
/// (`angel_net::doc::delta::Patch::SeqShift`).
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

/// `reckon_feel` undone, on a copy.
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

/// Whether `shot_stone` can run on this run. It indexes the room's grid and,
/// on an open floor, the guardian's chamber. A bad index, a zero side, or a
/// short tile list panics inside the stop; the seat refuses the tree instead.
fn room_can_stop_shot(run: &Run) -> bool {
    crate::rules::session::frame::room_can_stop_shot(run)
}

/// Whether a run read from the wire can be drawn. The pure owner uses the
/// same predicate before publishing, without a dependency on the wire codec.
pub fn drawable(run: &Run) -> bool {
    crate::rules::session::frame::drawable(run)
}

/// What of the tick's tree `seat` is sent (`None`: a watcher, who sees the
/// party's room).
///
/// - Another knight's own heat (`pastimes.heats`) is that knight's alone.
/// - A seat standing outside the party's room (`exits.outside`) is sent
///   none of the shots flying in that room, and none of the games played
///   there (`activity::all`, those under way).
///
/// Everything else is the tree as published, borrowed.
pub fn seat_tree(tree: &Value, seat: Option<u32>) -> Cow<'_, Value> {
    let knight = seat.map(knight_of);
    // The kingdom's chunks in 3D: only those near this seat's knight.
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
            // A seat that plays no knight is near none.
            _ => Vec::new(),
        };
        // In the knight's order, the nearest its view first (`pace`).
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
    // The numbers made for a seat (its card, its production screen, its list of chokes) are
    // that seat's alone: the builder boards of the others go without.
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
        // The games in the party's room are not this seat's to see. Only a
        // game under way (`Some`) is taken out, so the run still reads.
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

/// The most bytes of the kingdom's chunks one snapshot adds to what a seat
/// holds (`pace`). A painted chunk is its plate and patches, 0.9 to 1.4 MB;
/// sixteen are near one view, 18 MB with every chunk painted, and a client
/// reads no message over `angel_net::proto::MAX_SERVER_MESSAGE` (8 MB): the
/// page refused the snapshot a knight came out into the kingdom with, and
/// its host held every one after it, the page never having read it.
pub const CHUNK_BYTES_A_SNAPSHOT: usize = 2 << 20;

/// `seat_tree` as this snapshot sends it to a client holding `held` (`None`:
/// nothing yet): the kingdom's chunks it holds kept, and of those new to it
/// (or sent again with a new key) as many as `CHUNK_BYTES_A_SNAPSHOT` takes
/// in the seat's order, the nearest its view first, at least one; the rest
/// in the snapshots after (`angel_serve::rep::DocView::pace`). A chunk held
/// back that the client holds an older key of stays as it holds it.
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

/// What a chunk's entry (`chunk_value`) costs to send: its blobs.
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

/// Who hears an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hear {
    /// Everyone in the party's room, and the watchers: the run's own
    /// sounds. A seat standing outside the room does not hear them.
    Party,
    /// Every seat and watcher.
    All,
    /// One knight's seat alone.
    Knight(u32),
}

/// Something a tick made for the clients to play or show. Sent in the
/// tick's `Events`, each one these bytes:
///
/// | bytes | what |
/// |---|---|
/// | 1 | the kind: 1 a sound, 2 a line spoken, 3 a notice |
/// | 1 (+ varint) | who hears it: 0 the party's room, 1 everyone, 2 one knight (then the knight) |
/// | text | a sound: its name (`hit`, `knights/a_cheer_1`) |
/// | text, text, text | a line: the speaker's id (`blaise`), their name as shown, the words |
/// | text | a notice: the words |
///
/// Text is a varint length then UTF-8.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// One of the run's sounds (`Run::sounds`, the knights' chatter
    /// included).
    Sound { hear: Hear, name: String },
    /// A line the host's chorus speaks for a moment (`Run::cues`), shown as
    /// a subtitle.
    Say { hear: Hear, who: String, name: String, words: String },
    /// A line about the game to show.
    Notice { hear: Hear, text: String },
}

const SOUND: u8 = 1;
const SAY: u8 = 2;
const NOTICE: u8 = 3;
const HEAR_PARTY: u8 = 0;
const HEAR_ALL: u8 = 1;
const HEAR_KNIGHT: u8 = 2;
/// The longest a sound's name, a speaker's id or name, and the words of a
/// line or notice may be.
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

    /// The shell's line for this event, as heard by `knight` (`None`: a
    /// watcher) standing where `run` has them: `sfx <name>`, `say <words>`
    /// or `notice <words>`, or nothing when it is not theirs to hear. A
    /// sound's folders become dashes (`knights/a_cheer_1` is the bundle's
    /// `knights-a_cheer_1`), since the bundle's files sit in one folder.
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

/// A sound's name as the bundle names its file: folders become dashes, and
/// a hook whose own take is still to come plays its stand-in (`STAND_INS`).
pub fn sound_file(name: &str) -> String {
    let name = STAND_INS.iter().find(|(hook, _)| *hook == name).map_or(name, |(_, take)| *take);
    name.replace('/', "-")
}

/// Sounds the rules name before their own take is made, each with the
/// shipped take it plays meanwhile (`cockpit/assets/dungeon/sfx`): the
/// shoulder ride's climb (a leather grip), hop (boots landing), throw (a
/// toss) and slam (a short blast thump; `rules::shoulders`).
pub const STAND_INS: &[(&str, &str)] = &[
    ("shoulder_climb", "hand_grab"),
    ("shoulder_hop", "roll"),
    ("shoulder_throw", "bomb_toss"),
    ("shoulder_slam", "hit_blast"),
];

/// A music track's name as the bundle names its file (`music-crypt`).
pub fn music_file(track: &str) -> String {
    format!("music-{track}")
}

/// The run's sounds this tick, taken out of it, as events the party's room
/// hears (`Run::sounds`; the cues stay for the host's chorus), then those
/// for one knight alone (`Run::seat_sounds`).
pub fn take_sounds(run: &mut Run) -> Vec<Event> {
    crate::rules::session::frame::take_sounds(run).into_iter().map(portable_signal).collect()
}

/// Lines taken once by the shared pure rules drain, adapted to the public wire.
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

/// Text cut to at most `max` bytes, between characters.
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

/// A line the shell splits on newlines keeps to one.
fn one_line(text: &str) -> Cow<'_, str> {
    if text.contains(['\n', '\r']) {
        Cow::Owned(text.replace(['\n', '\r'], " "))
    } else {
        Cow::Borrowed(text)
    }
}

/// What the welcome carries for the client, before its first snapshot:
/// what the host sets once for the whole session (`crate::settings`).
///
/// | bytes | what |
/// |---|---|
/// | varint | `PARAMS_VERSION` |
/// | varint | flags: bit 0, unused (it chose the second art kit, cut 2026-10-20, before version 2); bit 1, draw every room in 3D (`world3d`); bit 2, stand the deep floors up in 3D (`deep3d`); bit 3, the first floor as the Atari board (`atari`, experimental) |
///
/// A client refuses a version it does not know (the page is out of date)
/// and flags it does not know (a newer host).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Params {
    /// Every room drawn in 3D, the kingdom too (`world3d::room::Mode::Always`).
    pub world3d: bool,
    /// The deep floors stand up in 3D, the upper ones 2D
    /// (`world3d::room::Mode::Deep`): which floor a seat stands on comes
    /// with each snapshot, so the host and every page agree room by room.
    pub deep3d: bool,
    /// The first floor drawn as the Atari board (`ANGEL_DELVE_ATARI=1`,
    /// experimental; `render::overworld::arena::atari`). Sent only when on,
    /// so a host without it welcomes an older page as before.
    pub atari: bool,
}

pub const PARAMS_VERSION: u64 = 2;
const WORLD3D: u64 = 2;
const DEEP3D: u64 = 4;
const ATARI: u64 = 8;

impl Params {
    /// The params for the host's settings now (and its `world3d` mode).
    pub fn of(_settings: crate::settings::Settings) -> Params {
        Params::with_mode(crate::world3d::room::mode())
    }

    /// The params for 3D mode `mode`.
    pub fn with_mode(mode: crate::world3d::room::Mode) -> Params {
        use crate::world3d::room::Mode;
        Params {
            world3d: mode == Mode::Always,
            deep3d: mode == Mode::Deep,
            atari: crate::render::overworld::arena::atari::on(),
        }
    }

    /// The 3D mode these params carry: every room, the deep floors, or none.
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
