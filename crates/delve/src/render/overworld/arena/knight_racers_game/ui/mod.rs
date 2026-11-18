//! The game's screens: the title, the modes, the classes, the courses (a menu over a CPU race),
//! the pod select, the garage, the records and the podium (a whole screen of their own), and the
//! time-trial ghost on the road. Each is a pure function of the session and the seat that looks:
//! the seat's own screens (garage, records) and the shared ones (everything else) read the
//! flow's state (`rules::Flow`) and draw nothing of their own.
use super::*;

pub(super) mod kit;
mod ghost;
mod pick;
mod podium;
mod title;

fn viewer(s: &rules::Session, focus: Option<u32>) -> Option<u32> {
    focus.filter(|id| s.flow.seats.contains_key(id)).or_else(|| s.flow.host())
}

pub(super) fn who(s: &rules::Session, seat: u32) -> String {
    let name: String = s.flow.seats.get(&seat).map(|sf| sf.name.to_uppercase()).unwrap_or_default().chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == ' ' || *c == '-').take(12).collect();
    let name = name.trim().to_string();
    if name.is_empty() { pods::livery(seat).name.to_string() } else { name }
}

pub(super) fn full(s: &rules::Session, focus: Option<u32>) -> Option<Img> {
    let seat = viewer(s, focus)?;
    let sf = s.flow.seats.get(&seat)?;
    match (sf.local, s.flow.stage) {
        (rules::flow::Local::Garage, _) => Some(pick::garage_shop(s, seat)),
        (rules::flow::Local::Records, _) => Some(pick::records(s, seat)),
        (_, rules::Stage::Pods) => Some(pick::pods(s, seat)),
        (_, rules::Stage::Podium) => Some(podium::draw(s, seat)),
        _ => None,
    }
}

pub(super) fn overlay(im: &mut Img, race: &rules::Session, seat: u32) -> bool {
    race.flow.attracting() && title::draw(im, race, seat)
}

pub(super) fn ghost(im: &mut Img, race: &rules::Session, car: &rules::Car, seat: u32, view: &View3, depth: &[f32]) {
    ghost::draw(im, race, car, seat, view, depth);
}

#[cfg(test)]
pub(super) fn route_for_proof(im: &mut Img, id: rules::CourseId, area: (i32, i32, i32, i32)) { title::route(im, id, area); }
