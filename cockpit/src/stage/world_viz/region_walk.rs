//! Authored Mines navigation shared by the Dotmax camera and room geometry.
use super::{World, adventure::Region};
const ROOM: (u32, u32) = (24, 18);
const GRID: (u32, u32) = (3, 3);
const HALF_TICKS_PER_STEP: u128 = 4;

pub(crate) const fn room_grid(region: Region) -> Option<(usize, usize)> {
    match region {
        Region::TheMines => Some((GRID.0 as usize, GRID.1 as usize)),
        _ => None,
    }
}
fn room_at(station: usize) -> (u32, u32) {
    let row = station as u32 / GRID.0;
    let col = station as u32 % GRID.0;
    (
        if row.is_multiple_of(2) {
            col
        } else {
            GRID.0 - 1 - col
        },
        row,
    )
}
fn waypoint(station: usize) -> (u32, u32) {
    let (x, y) = room_at(station);
    (x * ROOM.0 + ROOM.0 / 2, y * ROOM.1 + ROOM.1 * 2 / 3)
}
// The authored walk goes x first, then y, from the preceding station.
// Fractional steps and rounding preserve chamber ownership at door crossings.
fn station_at(iteration: usize, half_ticks: u128) -> usize {
    let count = (GRID.0 * GRID.1) as usize;
    let destination = iteration % count;
    let from = waypoint((destination + count - 1) % count);
    let to = waypoint(destination);
    let dx = from.0.abs_diff(to.0) as f32;
    let dy = from.1.abs_diff(to.1) as f32;
    let steps = (half_ticks as f64 / HALF_TICKS_PER_STEP as f64).min((dx + dy) as f64) as f32;
    let x = from.0 as f32 + (to.0 as f32 - from.0 as f32).signum() * steps.min(dx);
    let y = from.1 as f32 + (to.1 as f32 - from.1 as f32).signum() * (steps - dx).max(0.0).min(dy);
    let room = (x.round() as u32 / ROOM.0, y.round() as u32 / ROOM.1);
    (0..count)
        .find(|&i| room_at(i) == room)
        .expect("authored walk stays inside Mines")
}
pub(crate) fn hero_station(world: &World) -> Option<usize> {
    let quest = world.quest();
    room_grid(quest.region())?;
    Some(station_at(
        quest.iteration(),
        quest.walk_half_ticks(world.tick),
    ))
}
#[cfg(test)]
#[path = "../../../../tests/cockpit/world_viz/region_walk__tests.rs"]
mod tests;
