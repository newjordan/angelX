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
//! - **Interest** (`seat_tree`): what of that tree a seat is sent.
//! - **Events** (`Event`): sounds, the chorus's lines and notices, as bytes,
//!   and the shell lines a client makes of them.
//! - **Params** (`Params`): what the welcome carries before the first
//!   snapshot.

use crate::rules::{MAX_PLAYERS, Run, Shot, activity};
use angel_net::codec::{Bytes, Malformed, put_str, put_varint};
use angel_net::doc::value::{self, Value};
use std::borrow::Cow;

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
/// is the server's as it is counted: a client is sent its results.
pub const PRIVATE: [&str; 4] = ["rng", "next_id", "parked", "floor_tally"];

/// The run's fields its saves leave out (`serde(skip)`) that the renderer
/// reads: the marks since the realm last settled (the countryside's and the
/// holdings' stages are drawn from them) and the shards of broken pots.
/// The tree carries them beside the run's own fields, and `read` puts them
/// back, so a save's format is unchanged.
pub const DRAWN: [&str; 2] = ["marks", "pot_shards"];

/// The run as the tick's tree, built once for every seat: everything the
/// renderer reads (`DRAWN` included), nothing in `PRIVATE`, and what moves
/// by rule as where it started (`reckon`).
pub fn publish(run: &Run) -> Result<Value, value::Error> {
    let mut tree = value::to_value(run)?;
    if let Value::Struct(fields) = &mut tree {
        fields.retain(|(name, _)| !PRIVATE.contains(&name.as_ref()) && !DRAWN.contains(&name.as_ref()));
        fields.push((Cow::Borrowed(DRAWN[0]), value::to_value(&run.marks)?));
        fields.push((Cow::Borrowed(DRAWN[1]), value::to_value(&run.pot_shards)?));
        if let Some(item) = world_item(run) {
            fields.push((Cow::Borrowed(WORLD_ITEM), item));
        }
        if let Some(theme) = world_theme(run) {
            fields.push((Cow::Borrowed(WORLD_THEME), theme));
        }
        if let Some((chunks, near)) = world_chunks(run) {
            fields.push((Cow::Borrowed(WORLD_CHUNKS), chunks));
            fields.push((Cow::Borrowed(WORLD_NEAR), near));
        }
    }
    leave_out_absent(&mut tree, run);
    reckon(&mut tree, run.tick);
    Ok(tree)
}

/// The tree's field holding the item of the room on show (`world_item`).
pub const WORLD_ITEM: &str = "world3d";

/// The item of the room on show, for a page across a wire, which has no
/// asset folder of its own (`world3d::assets::receive`): its id, its key,
/// its set's text, and its plate and swatches as the PNGs they are. Only
/// while rooms are drawn in 3D and the room has one; the tree's delta sends
/// it again only when the room's item changes.
///
/// Lit (`world3d::rooms_lit`, the default 3D), a one-screen room's lit set
/// instead: its dressing's id, its key and its set's text, no plate (a page
/// makes an open floor's pieces itself, from the run's tiles).
fn world_item(run: &Run) -> Option<Value> {
    let field = |name: &'static str, value: Value| (Cow::Borrowed(name), value);
    if crate::world3d::room::stands_here(run) && crate::world3d::rooms_lit::on() {
        let lit = crate::world3d::rooms_lit::lit_for(run.room(), run.dungeon.pack)?;
        let text = lit.text()?;
        return Some(Value::Struct(vec![
            field("id", Value::Str(lit.id.into())),
            field("key", Value::U(lit.key)),
            field("set", Value::Str(text.as_ref().into())),
        ]));
    }
    let item = crate::world3d::room::item(run.room(), run.dungeon.pack)?.item;
    Some(Value::Struct(vec![
        field("id", Value::Str(item.id.into())),
        field("key", Value::U(item.key)),
        field("set", Value::Str(item.text.as_ref().into())),
        field("png", Value::Bytes(std::sync::Arc::clone(&item.png))),
        field("swatches", Value::Bytes(std::sync::Arc::clone(&item.swatches_png))),
    ]))
}

/// The tree's field holding the patches of the theme of the room on show
/// (`world_theme`).
pub const WORLD_THEME: &str = "world3d_theme";

/// The patches of the theme of the room on show (`world3d::theme`), for a
/// page across a wire, which has no asset folder of its own: what it draws a
/// room with no item with (`world3d::plain`). Its name, its key, and the
/// patches as the one PNG they are (`swatches.png`'s layout). Only while
/// rooms are drawn in 3D and the room is one drawn so, whether it has an
/// item or not: a floor's rooms share a theme, so the tree's delta sends it
/// once a floor, not once a room.
fn world_theme(run: &Run) -> Option<Value> {
    if !crate::world3d::enabled() || !crate::world3d::plain::on() || crate::world3d::rooms_lit::on() {
        return None;
    }
    let theme = crate::world3d::plain::theme_for(run.room(), run.dungeon.pack)?;
    let patches = crate::world3d::assets::themes().get(theme)?;
    let field = |name: &'static str, value: Value| (Cow::Borrowed(name), value);
    Some(Value::Struct(vec![
        field("theme", Value::Str(theme.name().into())),
        field("key", Value::U(patches.key)),
        field("png", Value::Bytes(std::sync::Arc::clone(&patches.png))),
    ]))
}

/// The theme's patches a tree carries (`world_theme`) kept, so a room with no
/// item can be drawn in 3D where no asset folder is: false when it carries
/// none, or patches that do not read.
pub fn take_world_theme(tree: &Value) -> bool {
    let Some(theme) = tree.field(WORLD_THEME) else {
        return false;
    };
    let (Some(Value::Str(name)), Some(Value::U(key)), Some(Value::Bytes(png))) = (theme.field("theme"), theme.field("key"), theme.field("png")) else {
        return false;
    };
    crate::world3d::assets::themes().receive(name, *key, png)
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
    // Set text only: the page fills it lit itself. A chunk with a plate
    // (none has one now) would send it too.
    let mut fields = vec![field("key", Value::U(chunk.key)), field("set", Value::Bytes(chunk.text.as_bytes().into()))];
    if chunk.painted() {
        fields.push(field("png", Value::Bytes(std::sync::Arc::clone(&chunk.png))));
        fields.push(field("swatches", Value::Bytes(std::sync::Arc::clone(&chunk.swatches_png))));
    }
    let value = Value::Struct(fields);
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
        let blob = |name: &str| match chunk.field(name) {
            Some(Value::Bytes(b)) => &b[..],
            _ => &[][..],
        };
        let (png, swatches) = (blob("png"), blob("swatches"));
        let Ok(set) = std::str::from_utf8(set) else { continue };
        if registry.receive(index as u32, *key, set, png, swatches) {
            kept.push(index as u32);
        }
    }
    registry.keep(&kept);
    kept.len()
}

/// The item a tree carries (`world_item`) kept, so the run's room can be
/// drawn in 3D where no asset folder is: false when it carries none, or one
/// that does not read.
pub fn take_world_item(tree: &Value) -> bool {
    let Some(item) = tree.field(WORLD_ITEM) else {
        return false;
    };
    // A lit room's set: no plate.
    if item.field("png").is_none() {
        let (Some(Value::U(key)), Some(Value::Str(set))) = (item.field("key"), item.field("set")) else {
            return false;
        };
        return crate::world3d::rooms_lit::receive(*key, set);
    }
    let (Some(Value::Str(id)), Some(Value::U(key)), Some(Value::Str(set)), Some(Value::Bytes(png)), Some(Value::Bytes(swatches))) = (
        item.field("id"),
        item.field("key"),
        item.field("set"),
        item.field("png"),
        item.field("swatches"),
    ) else {
        return false;
    };
    crate::world3d::assets::receive(id, *key, set, png, swatches)
}

/// A knight nobody's at all (`Hero::absent`: the place of a host who plays
/// no knight) is no part of the party any screen shows: it is not sent.
fn leave_out_absent(tree: &mut Value, run: &Run) {
    let absent: Vec<u64> = run.players.iter().filter(|(_, h)| h.absent).map(|(&id, _)| u64::from(id)).collect();
    if absent.is_empty() {
        return;
    }
    let gone = |key: &Value| key.as_id().is_some_and(|id| absent.contains(&id));
    if let Some(Value::Map(players)) = tree.field_mut("players") {
        players.retain(|(key, _)| !gone(key));
    }
    if let Some(Value::Map(outside)) = tree.field_mut("exits").and_then(|e| e.field_mut("outside")) {
        outside.retain(|(key, _)| !gone(key));
    }
}

/// `publish`, a tick after another: each shot in flight also carries an
/// `id` that stays with it from tick to tick, so a list of shots is sent
/// by id (`angel_net::doc::delta`'s keyed lists). Without one a shot that
/// lands moves every shot after it up the list, and each of those is sent
/// again as if it were new: in a busy fight, most of what a tick sent.
///
/// The run's shots have no ids of their own, so a shot is known again by
/// where it would be: within `SAME_SHOT` of where a shot of the same kind
/// stood a tick ago, moved on by its speed. A shot taken for another only
/// costs bytes: the tree each client holds is the run's whatever the ids
/// say. The host keeps one `Publisher` and publishes every tick with it.
#[derive(Default)]
pub struct Publisher {
    /// Last simulation tick's shots: id, position, kind, hostility, velocity.
    shots: Vec<(u64, f32, f32, Shot, bool, f32, f32)>,
    tick: Option<u64>,
    next: u64,
}

/// How far from where it would be a shot may be found and still be itself:
/// a curving shot turns, a bouncing one comes off a wall.
pub const SAME_SHOT: f32 = 0.75;

impl Publisher {
    pub fn publish(&mut self, run: &Run) -> Result<Value, value::Error> {
        let mut tree = publish(run)?;
        let step = run.tick.saturating_sub(self.tick.unwrap_or(run.tick)) as f32 * TICK_S as f32;
        let mut was = std::mem::take(&mut self.shots);
        for shot in &mut was { shot.1 += shot.5 * step; shot.2 += shot.6 * step; }
        self.tick = Some(run.tick);
        let mut ids = Vec::with_capacity(run.projectiles.len());
        for shot in &run.projectiles {
            let near = was
                .iter()
                .enumerate()
                .filter(|(_, w)| w.3 == shot.kind && w.4 == shot.hostile)
                .map(|(i, w)| (i, (w.1 - shot.x).hypot(w.2 - shot.y)))
                .filter(|&(_, d)| d <= SAME_SHOT)
                .min_by(|a, b| a.1.total_cmp(&b.1));
            let id = match near {
                Some((i, _)) => was.swap_remove(i).0,
                None => {
                    self.next += 1;
                    self.next
                }
            };
            ids.push(id);
            self.shots.push((id, shot.x, shot.y, shot.kind, shot.hostile, shot.vx, shot.vy));
        }
        if let Some(Value::Seq(shots)) = tree.field_mut("projectiles") {
            for (shot, id) in shots.iter_mut().zip(ids) {
                if let Value::Struct(fields) = shot {
                    fields.push((Cow::Borrowed("id"), Value::U(id)));
                }
            }
        }
        Ok(tree)
    }
}

/// A tree from the wire read back into a run, `DRAWN` included and what
/// `reckon` sent as its start brought to the tick.
pub fn read(tree: &Value) -> Result<Run, value::Error> {
    let mut run: Run = value::from_value(tree)?;
    if let Some(marks) = tree.field(DRAWN[0]) {
        run.marks = value::from_value(marks)?;
    }
    if let Some(shards) = tree.field(DRAWN[1]) {
        run.pot_shards = value::from_value(shards)?;
    }
    unreckon(&mut run);
    Ok(run)
}

/// Ticks a shot's start is counted from: the start of the latest span of
/// this many ticks. Every shot is sent again once a span (8.5 s), and a
/// start stays small enough for an `f32` to hold it to `RECKON_GRID`.
pub const RECKON_SPAN: u64 = 256;
/// A shot's start is sent to this many parts of a unit (a 512th of a unit
/// off at most: under a fiftieth of a pixel at the art's scale).
pub const RECKON_GRID: f64 = 256.0;
/// One tick, in seconds.
const TICK_S: f64 = 1.0 / crate::rules::HZ as f64;

/// What moves by a rule the client knows, sent as where it started, so it
/// is sent when it starts and when the rule changes, not every tick:
///
/// - a shot flying straight (`x += vx` a tick): its place as it was at the
///   start of the span (`RECKON_SPAN`), on a grid (`RECKON_GRID`); its
///   life (`ttl`) as the tick it ends. A shot that curves or bounces
///   changes its course, so it is sent as it changes, as before.
/// - a monster's age (one a tick): as the tick it was born.
///
/// In a busy fight that is two fifths of what a tick sends.
fn reckon(tree: &mut Value, tick: u64) {
    let since = (tick % RECKON_SPAN) as f64 * TICK_S;
    let grid = |v: f64| ((v * RECKON_GRID).round() / RECKON_GRID) as f32;
    if let Some(Value::Seq(shots)) = tree.field_mut("projectiles") {
        for shot in shots {
            let float = |shot: &Value, name: &str| match shot.field(name) {
                Some(Value::F32(v)) => Some(f64::from(*v)),
                _ => None,
            };
            if let (Some(x), Some(y), Some(vx), Some(vy)) =
                (float(shot, "x"), float(shot, "y"), float(shot, "vx"), float(shot, "vy"))
            {
                set(shot, "x", Value::F32(grid(x - vx * since)));
                set(shot, "y", Value::F32(grid(y - vy * since)));
            }
            if let Some(&Value::U(ttl)) = shot.field("ttl") {
                set(shot, "ttl", Value::U(tick.wrapping_add(ttl) & u64::from(u32::MAX)));
            }
        }
    }
    if let Some(Value::Seq(monsters)) = tree.field_mut("enemies") {
        for monster in monsters {
            if let Some(&Value::U(age)) = monster.field("age") {
                set(monster, "age", Value::U(tick.wrapping_sub(age) & u64::from(u32::MAX)));
            }
        }
    }
}

/// `reckon` undone: each shot and monster brought from its start to the
/// run's tick.
fn unreckon(run: &mut Run) {
    let tick = run.tick;
    let since = (tick % RECKON_SPAN) as f64 * TICK_S;
    for shot in &mut run.projectiles {
        shot.x = (f64::from(shot.x) + f64::from(shot.vx) * since) as f32;
        shot.y = (f64::from(shot.y) + f64::from(shot.vy) * since) as f32;
        shot.ttl = shot.ttl.wrapping_sub(tick as u32);
    }
    for monster in &mut run.enemies {
        monster.age = (tick as u32).wrapping_sub(monster.age);
    }
}

fn set(item: &mut Value, name: &str, to: Value) {
    if let Some(field) = item.field_mut(name) {
        *field = to;
    }
}

/// Whether a run read from the wire can be drawn: the rooms the renderer
/// indexes are there. A client cannot run `Run::valid_snapshot`, which
/// checks the run's ids against the server's next id.
pub fn drawable(run: &Run) -> bool {
    let rooms = run.dungeon.rooms.len();
    rooms > 0
        && run.at < rooms
        && run.exits.valid(rooms)
        && run.exits.outside.keys().all(|id| run.players.contains_key(id))
        && run.kingdom.valid()
        && run.tune.valid()
        && run.cinder.as_ref().is_none_or(|s| s.valid())
        && run.latch.as_ref().is_none_or(|s| s.valid())
        && run.mossbell.as_ref().is_none_or(|s| s.valid())
        && run.rafterwake.as_ref().is_none_or(|s| s.valid())
        && run.forest_road.as_ref().is_none_or(|s| s.valid())
        && crate::rules::roundtable::valid(run)
        && run.pennon.as_ref().is_none_or(|s| s.valid())
        && run.players.len() <= MAX_PLAYERS as usize
        && run.players.values().all(|h| h.x.is_finite() && h.y.is_finite())
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
        if let Value::Struct(fields) = &mut view {
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
    if !others_heats && !outside {
        return Cow::Borrowed(tree);
    }
    let mut view = tree.clone();
    if others_heats
        && let Some(Value::Map(heats)) = view.field_mut("pastimes").and_then(|p| p.field_mut("heats"))
    {
        heats.retain(|(key, _)| mine(key));
    }
    if outside {
        if let Some(shots @ Value::Seq(_)) = view.field_mut("projectiles") {
            *shots = Value::Seq(Vec::new());
        }
        // The games in the party's room are not this seat's to see. Only a
        // game under way (`Some`) is taken out, so the run still reads.
        for name in activity::all().flat_map(|a| a.fields.iter()) {
            if let Some(game @ Value::Some(_)) = view.field_mut(name) {
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

/// A sound's name as the bundle names its file: folders become dashes.
pub fn sound_file(name: &str) -> String {
    name.replace('/', "-")
}

/// A music track's name as the bundle names its file (`music-crypt`).
pub fn music_file(track: &str) -> String {
    format!("music-{track}")
}

/// The run's sounds this tick, taken out of it, as events the party's room
/// hears (`Run::sounds`; the cues stay for the host's chorus).
pub fn take_sounds(run: &mut Run) -> Vec<Event> {
    std::mem::take(&mut run.sounds)
        .into_iter()
        .map(|name| Event::Sound { hear: Hear::Party, name: name.to_string() })
        .collect()
}

/// The lines the run gave everyone this tick, taken out of it, as notices
/// every seat is shown (`Run::notices`: a wish granted, "Ada's Bigger
/// Arrows").
pub fn take_notices(run: &mut Run) -> Vec<Event> {
    std::mem::take(&mut run.notices)
        .into_iter()
        .map(|text| Event::Notice { hear: Hear::All, text })
        .collect()
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
/// | varint | flags: bit 0, draw with the second art kit (`art_v2`); bit 1, draw every room in 3D (`world3d`); bit 2, stand the deep floors up in 3D (`deep3d`) |
///
/// A client refuses a version it does not know (the page is out of date)
/// and flags it does not know (a newer host).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Params {
    pub art_v2: bool,
    /// Every room drawn in 3D, the kingdom too (`world3d::room::Mode::Always`).
    pub world3d: bool,
    /// The deep floors stand up in 3D, the upper ones 2D
    /// (`world3d::room::Mode::Deep`): which floor a seat stands on comes
    /// with each snapshot, so the host and every page agree room by room.
    pub deep3d: bool,
}

pub const PARAMS_VERSION: u64 = 1;
const ART_V2: u64 = 1;
const WORLD3D: u64 = 2;
const DEEP3D: u64 = 4;

impl Params {
    /// The params for the host's settings now (and its `world3d` mode).
    pub fn of(settings: crate::settings::Settings) -> Params {
        Params::with_mode(settings.art_v2, crate::world3d::room::mode())
    }

    /// The params for art kit `art_v2` and 3D mode `mode`.
    pub fn with_mode(art_v2: bool, mode: crate::world3d::room::Mode) -> Params {
        use crate::world3d::room::Mode;
        Params { art_v2, world3d: mode == Mode::Always, deep3d: mode == Mode::Deep }
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
        let flags = if self.art_v2 { ART_V2 } else { 0 }
            | if self.world3d { WORLD3D } else { 0 }
            | if self.deep3d { DEEP3D } else { 0 };
        put_varint(&mut out, flags);
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Params, Malformed> {
        let mut b = Bytes::new(bytes);
        if b.varint()? != PARAMS_VERSION {
            return Err(Malformed);
        }
        let flags = b.varint()?;
        if flags & !(ART_V2 | WORLD3D | DEEP3D) != 0 {
            return Err(Malformed);
        }
        b.end()?;
        Ok(Params { art_v2: flags & ART_V2 != 0, world3d: flags & WORLD3D != 0, deep3d: flags & DEEP3D != 0 })
    }
}

#[cfg(test)]
#[path = "../tests/wire__tests.rs"]
mod tests;
