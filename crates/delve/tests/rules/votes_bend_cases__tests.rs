//! Ballot cases for the Bend port (games_c).
use crate::rules::hippodrome::games_c_bend_cases_tests as kit;
use super::*;
use serde_json::json;

fn action(d: &mut kit::Dice) -> Action {
    match d.below(10) {
        0 => Action::Toggle,
        1 => Action::Close,
        2 => Action::Up,
        3 => Action::Down,
        4 => Action::Left,
        5 => Action::Right,
        6 => Action::Rate(d.below(7) as u8),
        7 => Action::Clear,
        8 => Action::Untried,
        _ => Action::Toggle,
    }
}

/// The State as the rules worker holds it (the transient fields are written while capturing).
fn cap(run: &mut Run, st: &State) -> serde_json::Value {
    run.playtest = st.clone();
    kit::state(run)["playtest"].clone()
}

#[test]
#[ignore]
fn record_ballot_cases() {
    let mut d = kit::Dice(0xba11);
    let mut run = Run::new(1, 1, None);
    let names = ["Gawain", "Lancelot", "Bors", "Percival"];
    for round in 0..30u64 {
        let mut st = State::default();
        let mut serial = 0u64;
        for _ in 0..90 {
            let id = 1 + d.below(3) as u32;
            let name = names[d.below(4) as usize];
            serial += if d.chance(10) { 0 } else { 1 + d.below(3) };
            let c = Command { serial, action: action(&mut d) };
            let before = cap(&mut run, &st);
            st.command(id, name, c);
            kit::rec("votes.State.command", json!({"st": before, "id": id, "name": name, "command": kit::js(&c)}), json!({"st": cap(&mut run, &st)}));
        }
        for item in ["bow", "hud", "floor-plates", "nothing"] {
            let (avg, n) = st.average(item);
            kit::rec("votes.State.average", json!({"st": cap(&mut run, &st), "item": item}), json!({"ret": [avg, n]}));
        }
        let c = Command { serial: round, action: Action::Rate(1 + (round % 5) as u8) };
        kit::rec("votes.Command.valid", json!({"c": kit::js(&c)}), json!({"ret": c.valid()}));
        let bad = Command { serial: round, action: Action::Rate(6 + (round % 3) as u8) };
        kit::rec("votes.Command.valid", json!({"c": kit::js(&bad)}), json!({"ret": bad.valid()}));
    }
    for round in 0..40u64 {
        let mut st = State::default();
        st.records.entry("Gawain".into()).or_default().insert("bow".into(), Rating::Stars(3));
        let id = 1 + (round % 4) as u32;
        let name = names[(round % 4) as usize];
        let before = cap(&mut run, &st);
        st.arrive(id, name);
        kit::rec("votes.State.arrive", json!({"st": before, "id": id, "name": name}), json!({"st": cap(&mut run, &st)}));
        let before = cap(&mut run, &st);
        st.arrive(id, name);
        kit::rec("votes.State.arrive", json!({"st": before, "id": id, "name": name}), json!({"st": cap(&mut run, &st)}));
    }
}
