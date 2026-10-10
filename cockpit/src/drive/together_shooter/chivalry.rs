//! Two reachable practice entrances on existing home/yard floor. They open a
//! host-local 3D side stage, not new Run rooms or guest command messages.
use super::{RoomKind, Run, TILE_UNITS};
use crate::drive::chivalry::Place;
pub(crate) fn portals(kind: RoomKind) -> Option<[(Place, f32, f32); 2]> {
    match kind {
        RoomKind::Home => Some([(Place::Stables, 4.5, 4.5), (Place::Tournament, 19.5, 4.5)]),
        RoomKind::Yard => Some([(Place::Stables, 4.5, 12.5), (Place::Tournament, 12.0, 12.5)]),
        _ => None,
    }
}
pub(crate) fn near_portal(run: &Run, id: u32) -> Option<Place> {
    run.chivalry.as_ref()?;
    if !run.at_home_now() {
        return None;
    }
    let hero = run.players.get(&id)?;
    portals(run.room().kind)?
        .into_iter()
        .find(|(_, x, y)| (hero.x / TILE_UNITS - x).hypot(hero.y / TILE_UNITS - y) < 1.25)
        .map(|p| p.0)
}
