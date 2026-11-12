//! Captured transport facts around one rules step and its native ability gap.
//! Presence timers, room arrival tracking, native media leases and sockets stay
//! native. This module owns only their bounded game consequences and drains.
use super::{Context, Policies};
use crate::rules::{Hold, Input, RoomKind, Run, tune::Tune};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Seat {
    pub player: u32,
    pub epoch: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeatInput {
    pub epoch: u64,
    pub input: Input,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Away {
    pub seat: Seat,
    pub away: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputAge {
    pub seat: Seat,
    pub ticks: u64,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Controls {
    pub active: bool,
    pub keyboard: bool,
    pub hosted: bool,
    pub wishing: bool,
    pub couch: bool,
    pub watching: bool,
    pub friend_one: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControlChange {
    pub expected_tick: u64,
    pub seats: BTreeMap<u32, u64>,
    pub controls: Controls,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoomKey {
    pub raid: u64,
    pub depth: u32,
    pub room: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Arrival {
    pub tick: u32,
    pub until: u32,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArrivalState {
    pub room: Option<RoomKey>,
    pub arrival: Option<Arrival>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Presence {
    pub playing: bool,
    pub applied: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Begin {
    pub network_tick: u32,
    pub expected_tick: u64,
    /// Exact live seats observed while native derived presence/removal facts.
    pub seats: BTreeMap<u32, u64>,
    pub controls: Controls,
    pub tune: Tune,
    pub input_ages: Vec<InputAge>,
    pub remove: Vec<Seat>,
    /// Empty means presence has not yet been supplied; preserve existing away.
    pub away: Vec<Away>,
    pub arrival: ArrivalState,
    pub presence: BTreeMap<u32, Presence>,
    pub inputs: BTreeMap<u32, SeatInput>,
    pub active_casts: Vec<super::abilities::ActiveCast>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fence {
    pub begin_seq: u64,
    pub network_tick: u32,
    pub tick: u64,
    pub active: bool,
    pub seats: BTreeMap<u32, u64>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Begun {
    pub frame: Fence,
    pub arrival: ArrivalState,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhaseInfo {
    pub begun: Begun,
    pub abilities_applied: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Finish {
    pub begin_seq: u64,
    pub network_tick: u32,
    pub expected_tick: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "player",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum Hear {
    Party,
    All,
    Knight(u32),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Signal {
    Sound { hear: Hear, name: String },
    Notice { hear: Hear, text: String },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Outputs {
    pub frame: Fence,
    /// Departure notices precede retained native ability media/voice events.
    pub prelude: Vec<Signal>,
    /// Then party sounds, private sounds, all notices, private notices.
    pub postlude: Vec<Signal>,
    pub cues: Vec<String>,
    pub feel: Vec<String>,
    /// Native retains the network AGAIN_RETELL pacing and maps this to News.
    pub again_due: bool,
    pub mended: Option<Vec<String>>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pending {
    pub frame: Fence,
    pub started_tick: u64,
    /// Exact input survives the native ability gap and executable handoff.
    pub inputs: BTreeMap<u32, SeatInput>,
    pub prelude: Vec<Signal>,
    pub active_casts: Vec<super::abilities::ActiveCast>,
    pub abilities_applied: bool,
    pub arrival: ArrivalState,
}

pub fn seats(run: &Run) -> BTreeMap<u32, u64> {
    run.players
        .keys()
        .map(|&player| (player, run.seat_epoch(player)))
        .collect()
}
fn player(id: u32) -> bool {
    (1..=crate::rules::MAX_PLAYERS).contains(&id)
}
fn seat(run: &Run, seat: Seat) -> bool {
    run.players.contains_key(&seat.player) && run.seat_epoch(seat.player) == seat.epoch
}
fn distinct<T>(values: &[T], key: impl Fn(&T) -> u32) -> bool {
    let mut seen = BTreeSet::new();
    values.iter().all(|v| seen.insert(key(v)))
}
impl Begin {
    pub(super) fn validate(&self, run: &Run, session: &str) -> Result<(), String> {
        let max = crate::rules::MAX_PLAYERS as usize;
        if self.expected_tick != run.tick
            || self.seats != seats(run)
            || !self.tune.valid()
            || self.presence.len() > max
            || self.presence.keys().any(|&id| !player(id))
            || self.inputs.len() > max
            || self.input_ages.len() > max
            || self.remove.len() > max
            || self.away.len() > max
            || !distinct(&self.input_ages, |v| v.seat.player)
            || !distinct(&self.remove, |v| v.player)
            || self
                .remove
                .windows(2)
                .any(|pair| pair[0].player >= pair[1].player)
            || !distinct(&self.away, |v| v.seat.player)
            || self.inputs.iter().any(|(&player, v)| {
                !seat(
                    run,
                    Seat {
                        player,
                        epoch: v.epoch,
                    },
                )
            })
            || self.input_ages.iter().any(|v| !seat(run, v.seat))
            || self
                .remove
                .iter()
                .any(|&v| v.player == 1 || !seat(run, v) || v.epoch == u64::MAX)
            || self.away.iter().any(|v| {
                !seat(run, v.seat) || self.remove.iter().any(|r| r.player == v.seat.player)
            })
            || (self.controls.active && run.tick >= super::MAX_TICK)
        {
            return Err("stale or invalid frame facts".into());
        }
        super::abilities::validate_active_casts(&self.active_casts, run)?;
        if self
            .active_casts
            .iter()
            .any(|proof| proof.session != session)
        {
            return Err("ability proof belongs to another session".into());
        }
        let host_absent = self.controls.watching && !self.controls.friend_one;
        if host_absent && self.away.iter().any(|v| v.seat.player == 1 && !v.away) {
            return Err("absent host cannot be marked present".into());
        }
        Ok(())
    }
}
impl Pending {
    pub(super) fn validate(&self, run: &Run, event_seq: u64, session: &str) -> Result<(), String> {
        let max = crate::rules::MAX_PLAYERS as usize;
        if self.frame.begin_seq == 0
            || self.frame.begin_seq > event_seq
            || self.started_tick > self.frame.tick
            || self.frame.tick != run.tick
            || self.frame.seats != seats(run)
            || self.inputs.len() > max
            || self.inputs.keys().any(|&id| !player(id))
            || self.prelude.len() > max
            || self.prelude.iter().any(
                |v| !matches!(v, Signal::Notice { hear: Hear::All, text } if text.len() <= 4096),
            )
        {
            return Err("invalid retained frame phase".into());
        }
        super::abilities::validate_active_casts(&self.active_casts, run)?;
        if self
            .active_casts
            .iter()
            .any(|proof| proof.session != session)
        {
            return Err("retained ability proof belongs to another session".into());
        }
        Ok(())
    }
}

pub(super) fn begin(
    run: &mut Run,
    session: &str,
    begin_seq: u64,
    input: Begin,
    policies: &mut impl Policies,
) -> Result<Pending, String> {
    input.validate(run, session)?;
    // Input-age callbacks historically precede DelveServer::step.
    for age in &input.input_ages {
        run.latch_input_age.insert(age.seat.player, age.ticks);
    }
    apply_controls(run, input.controls);
    if run.tune != input.tune {
        run.tune = input.tune;
    }
    run.set_absent(1, input.controls.watching && !input.controls.friend_one);
    let mut prelude = Vec::new();
    for removed in &input.remove {
        let name = run.players[&removed.player].name.clone();
        run.leave(removed.player);
        prelude.push(Signal::Notice {
            hear: Hear::All,
            text: format!("{name} left the party"),
        });
    }
    for away in &input.away {
        run.set_away(away.seat.player, away.away);
    }
    let mut arrival = input.arrival;
    arrival.note_room(run, input.network_tick);
    let hold = arrival.hold_for(run, input.network_tick, &input.presence);
    run.hold(hold);
    let inputs = input
        .inputs
        .iter()
        .map(|(&id, value)| (id, value.input))
        .collect();
    if input.controls.active {
        for (&knight, value) in &input.inputs {
            if let Some(command) = value.input.vote {
                run.vote_command(knight, command);
            }
        }
        policies.step(run, &inputs);
    } else {
        for (&knight, value) in &input.inputs {
            if value.input.valid() && value.input.leave_activity {
                run.leave_activity(knight);
            }
        }
    }
    if input.controls.active {
        arrival.note_room(run, input.network_tick);
    }
    let active_casts = input
        .active_casts
        .into_iter()
        .filter(|proof| {
            proof.request.raid == run.raid_id
                && run.seat_epoch(proof.request.player) == proof.request.epoch
                && run.players.contains_key(&proof.request.player)
        })
        .collect();
    Ok(Pending {
        active_casts,
        abilities_applied: false,
        arrival,
        frame: Fence {
            begin_seq,
            network_tick: input.network_tick,
            tick: run.tick,
            active: input.controls.active,
            seats: seats(run),
        },
        started_tick: input.expected_tick,
        inputs: input.inputs,
        prelude,
    })
}

pub(super) fn finish(
    run: &mut Run,
    pending: Pending,
    finish: Finish,
    context: &Context,
) -> Result<Outputs, String> {
    if (finish.begin_seq, finish.network_tick, finish.expected_tick)
        != (
            pending.frame.begin_seq,
            pending.frame.network_tick,
            pending.frame.tick,
        )
        || run.tick != pending.frame.tick
        || seats(run) != pending.frame.seats
    {
        return Err("stale frame finish fence".into());
    }
    for (&knight, value) in &pending.inputs {
        if run.players.contains_key(&knight)
            && run.seat_epoch(knight) == value.epoch
            && value.input.valid()
            && value.input.again
        {
            run.ask_again(knight);
        }
    }
    let again_due = run.again_due();
    let cues = std::mem::take(&mut run.cues);
    let postlude = take_sounds(run)
        .into_iter()
        .chain(take_notices(run))
        .collect();
    let feel = std::mem::take(&mut run.feel.lines);
    if !Arc::ptr_eq(&run.kingdom.wishes, &context.wishes) {
        let changed = run.kingdom.wishes != context.wishes;
        run.kingdom.wishes = context.wishes.clone();
        if changed {
            run.dungeon.world_revision = run.dungeon.world_revision.wrapping_add(1);
            run.raise_kingdom();
        }
    }
    let mended = (!drawable(run)).then(|| run.mend().into_iter().map(str::to_owned).collect());
    Ok(Outputs {
        frame: pending.frame,
        prelude: pending.prelude,
        postlude,
        cues,
        feel,
        again_due,
        mended,
    })
}

/// Same control projection previously applied by the native server prelude.
fn apply_controls(run: &mut Run, controls: Controls) -> bool {
    let mut changed = false;
    if let Some(seat) = run.playtest.seats.get_mut(&2)
        && seat.couch != controls.couch
    {
        seat.couch = controls.couch;
        changed = true;
    }
    if run.wishing != controls.wishing {
        run.wishing = controls.wishing;
        changed = true;
    }
    let continuous = run.at_home_now() || run.room().kind == RoomKind::Kingdom;
    let companion = run.players.get(&2).filter(|h| h.hp > 0).map(|h| (h.x, h.y));
    let outside = run.exits.outside.contains_key(&1);
    if let Some(host) = run.players.get_mut(&1).filter(|_| !controls.friend_one) {
        let stone =
            (controls.hosted && !controls.keyboard) || host.vigil || host.away || controls.watching;
        if host.stone
            && !continuous
            && !outside
            && !stone
            && let Some((x, y)) = companion
        {
            host.x = x;
            host.y = y;
        }
        if host.stone != stone {
            host.stone = stone;
            changed = true;
        }
    }
    changed
}
pub(super) fn controls_changed(run: &mut Run, input: ControlChange) -> Result<bool, String> {
    if input.expected_tick != run.tick || input.seats != seats(run) {
        return Err("stale control projection".into());
    }
    Ok(apply_controls(run, input.controls))
}
impl ArrivalState {
    fn note_room(&mut self, run: &Run, tick: u32) {
        let room = RoomKey {
            raid: run.raid_id,
            depth: run.dungeon.depth,
            room: run.at,
        };
        if self.room.is_some_and(|old| old != room) {
            self.arrival = Some(Arrival {
                tick,
                until: tick.wrapping_add(3 * crate::rules::HZ),
            });
        }
        self.room = Some(room);
    }
    fn hold_for(&mut self, run: &Run, tick: u32, presence: &BTreeMap<u32, Presence>) -> Hold {
        if !run.active() {
            self.arrival = None;
            return Hold::Open;
        }
        let living = run.players.values().any(|hero| hero.hp > 0);
        if living && !run.players.values().any(|hero| hero.hp > 0 && !hero.stone) {
            return Hold::Empty;
        }
        if let Some(arrival) = self.arrival {
            let waiting = presence.iter().any(|(&id, here)| {
                here.playing
                    && here.applied < arrival.tick
                    && run.players.get(&id).is_some_and(|hero| !hero.away)
            });
            if waiting && tick < arrival.until {
                return Hold::Arriving;
            }
            self.arrival = None;
        }
        Hold::Open
    }
}

/// Shared pure drains; public wire adapts these values without a second queue.
pub fn take_sounds(run: &mut Run) -> Vec<Signal> {
    let bard_for_all = crate::rules::bard::heard_by_all();
    let mine = std::mem::take(&mut run.seat_sounds)
        .into_iter()
        .map(move |(knight, name)| Signal::Sound {
            // The Bard's music is its own ears' unless the host turned it up for the party.
            hear: if bard_for_all && (crate::rules::bard::is_bard_sound(name) || name == crate::rules::bard::HIT) {
                Hear::Party
            } else {
                Hear::Knight(knight)
            },
            name: name.to_string(),
        });
    std::mem::take(&mut run.sounds)
        .into_iter()
        .map(|name| Signal::Sound {
            hear: Hear::Party,
            name: name.to_string(),
        })
        .chain(mine)
        .collect()
}
pub fn take_notices(run: &mut Run) -> Vec<Signal> {
    let all = std::mem::take(&mut run.notices)
        .into_iter()
        .map(|text| Signal::Notice {
            hear: Hear::All,
            text,
        });
    let mine = std::mem::take(&mut run.seat_notices)
        .into_iter()
        .map(|(knight, text)| Signal::Notice {
            hear: Hear::Knight(knight),
            text,
        });
    all.chain(mine).collect()
}

pub fn room_can_stop_shot(run: &Run) -> bool {
    let Some(room) = run.dungeon.rooms.get(run.at) else {
        return false;
    };
    if !room.grid_holds() {
        return false;
    }
    if let Some(open) = run.open_here()
        && usize::from(open.guardian) >= open.chambers.len()
    {
        return false;
    }
    true
}
/// Existing wire drawable predicate, shared by pure Finish and wire decoding.
pub fn drawable(run: &Run) -> bool {
    let rooms = run.dungeon.rooms.len();
    rooms > 0
        && run.at < rooms
        && room_can_stop_shot(run)
        && run.exits.valid(rooms)
        && run
            .exits
            .outside
            .keys()
            .all(|id| run.players.contains_key(id))
        && run.kingdom.valid()
        && run.home.industry.valid(crate::kingdom::catalog())
        && run.squire_academy.valid()
        && run.home.squire_academy.valid()
        && run.tune.valid()
        && run.cinder.as_ref().is_none_or(|s| s.valid())
        && run.latch.as_ref().is_none_or(|s| s.valid())
        && run.mossbell.as_ref().is_none_or(|s| s.valid())
        && run.rafterwake.as_ref().is_none_or(|s| s.valid())
        && run.forest_road.as_ref().is_none_or(|s| s.valid())
        && crate::rules::roundtable::valid(run)
        && run.pennon.as_ref().is_none_or(|s| s.valid())
        && run.players.len() <= crate::rules::MAX_PLAYERS as usize
        && run.tick <= super::MAX_TICK
        && run
            .players
            .values()
            .all(|h| h.x.is_finite() && h.y.is_finite())
        && run
            .enemies
            .iter()
            .all(|e| e.x.is_finite() && e.y.is_finite())
        && run
            .projectiles
            .iter()
            .all(|p| p.x.is_finite() && p.y.is_finite())
}

#[cfg(test)]
mod tests;
