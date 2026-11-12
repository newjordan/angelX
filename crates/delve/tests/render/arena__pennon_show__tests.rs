//! Pennon Rush's words: walk whole rounds at one to six seats and check, on
//! every screen of every sampled tick, that no two boxes of words meet, that
//! every box sits inside the 384 by 224 canvas, and that `YOU` names only the
//! knight looking while every other knight is called by its own name (`KNIGHT
//! n` for one with the default name or none).

use super::*;
use crate::rules::Input;
use crate::rules::pennon::{self, LENGTH, Lane, Q, Session, show};
use std::collections::BTreeMap;

type Rect = (i32, i32, i32, i32);

fn meets(a: Rect, b: Rect) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

/// The names a table sits under: a real one, the default, none, and real.
const NAMES: [&str; 6] = ["Gawain", "You", "", "Lyra", "Bran", "Elin"];

fn table(seats: u32) -> Run {
    let mut r = Run::pennon_for_test();
    for id in 2..=seats {
        r.join_seat(id, NAMES[(id - 1) as usize]);
    }
    r.step(&BTreeMap::from([(1, Input { interact: true, ..Default::default() })]));
    assert!(r.pennon.is_some());
    r
}

/// What the seat looking should read for knight `id`, worked out on its own.
fn expected(run: &Run, id: u32, me: u32, take: usize) -> String {
    if id == me {
        return "YOU".into();
    }
    let name = run.players[&id].name.trim().to_uppercase();
    if name.is_empty() || name == "YOU" { format!("KNIGHT {id}") } else { name.chars().take(take).collect() }
}

fn claims() -> Vec<(&'static str, Rect, String)> {
    super::hud::CLAIMS.with(|c| c.borrow().clone())
}

/// Draw `me`'s screen and judge the words on it.
fn judge(run: &Run, me: u32, what: &str) {
    let _ = super::layers(run, Some(me));
    let all = claims();
    let s = run.pennon.as_ref().unwrap();
    for (name, b, text) in &all {
        assert!(b.0 >= 0 && b.1 >= 0 && b.0 + b.2 <= 384 && b.1 + b.3 <= 224, "{what}: {name} {text:?} {b:?} leaves 384x224");
    }
    for (i, (an, a, at)) in all.iter().enumerate() {
        for (bn, b, bt) in &all[i + 1..] {
            assert!(!meets(*a, *b), "{what}: {an} {at:?} {a:?} meets {bn} {bt:?} {b:?}");
        }
    }
    // The chips name every seat, the viewer YOU and nobody else.
    let chips: Vec<&str> = all.iter().filter(|(n, ..)| *n == "chip-name").map(|(.., t)| t.as_str()).collect();
    let n = s.lanes.len();
    if !chips.is_empty() {
        assert_eq!(chips.len(), n, "{what}: a chip for every knight");
        assert_eq!(chips.iter().filter(|&&t| t == "YOU").count(), 1, "{what}: YOU is the viewer, once: {chips:?}");
        for (chip, id) in chips.iter().zip(s.lanes.keys()) {
            assert_eq!(*chip, expected(run, *id, me, 12), "{what}: chip of seat {id}");
        }
    }
    // No other word says YOU but the viewer's own calls and the viewer's own place.
    for (name, _, text) in &all {
        let words: Vec<&str> = text.split(' ').collect();
        if words.contains(&"YOU") {
            assert!(
                matches!(*name, "chip-name" | "banner-sub" | "podium-name" | "rest"),
                "{what}: {name} {text:?} says YOU outside a name or a call"
            );
        }
        // A KNIGHT n is a real seat with the default name, never the viewer.
        if let Some(at) = words.iter().position(|w| *w == "KNIGHT") {
            let id: u32 = words.get(at + 1).and_then(|w| w.parse().ok()).unwrap_or(0);
            assert!(id != me && s.lanes.contains_key(&id), "{what}: {name} {text:?} calls a knight who is not at the table");
            assert_eq!(expected(run, id, me, 12), format!("KNIGHT {id}"), "{what}: {text:?}");
        }
    }
    // The rest of the table: "4 NAME 690", the place and the name matching the board.
    for (name, _, text) in &all {
        if *name != "rest" {
            continue;
        }
        let words: Vec<&str> = text.split(' ').collect();
        let place: usize = words[0].parse().expect("a place");
        let who = words[1..words.len() - 1].join(" ");
        assert_eq!(who, expected(run, s.order[place - 1], me, 8), "{what}: {text:?}");
    }
    // The podium names the board's order, the viewer YOU only on their own step.
    let podium: Vec<&str> = all.iter().filter(|(n, ..)| *n == "podium-name").map(|(.., t)| t.as_str()).collect();
    let top: Vec<String> = s.order.iter().take(3).map(|&id| expected(run, id, me, 9)).collect();
    for name in &podium {
        assert!(top.iter().any(|t| t == name), "{what}: podium name {name:?} is not one of {top:?}");
    }
    assert!(podium.iter().filter(|&&t| t == "YOU").count() <= 1, "{what}: {podium:?}");
}

/// Who plays how: a perfect knight, a late one, one who naps now and then,
/// one who sits out the opening, a masher, and a slow one.
fn hands(run: &Run, tick: u32) -> BTreeMap<u32, Input> {
    let Some(s) = run.pennon.as_ref() else { return BTreeMap::new() };
    if s.hold > 0 {
        return BTreeMap::new();
    }
    let elapsed = LENGTH - s.left;
    let mut keys = BTreeMap::new();
    for (&id, l) in &s.lanes {
        let bot = l.bot_input(elapsed);
        let h = |k: u32| (tick.wrapping_mul(2654435761).wrapping_add(id * 40503 + k) >> 7) % 100;
        let input = match id {
            1 => bot,
            2 if elapsed > 40 => bot,
            3 if (elapsed / 20) % 5 != 3 => bot,
            4 if elapsed > 240 => bot,
            5 => Input { move_x: 1, fire: h(1) < 60, interact: h(2) < 12, ..Default::default() },
            6 if elapsed > 90 => bot,
            _ => Input::default(),
        };
        keys.insert(id, input);
    }
    keys
}

/// Every sampled tick of whole rounds at one to six seats: the three rules hold.
#[test]
fn pennon_words_never_meet_and_yous_belong_to_the_viewer() {
    for seats in 1..=6u32 {
        let mut r = table(seats);
        let views: Vec<u32> = if seats == 1 { vec![1] } else { vec![1, seats] };
        let mut seen = 0;
        let mut t = 0u32;
        let mut over_at = None;
        while t < LENGTH + 66 + 280 {
            let keys = hands(&r, t);
            r.step(&keys);
            t += 1;
            let ended = r.pennon.as_ref().is_some_and(|s| s.left == 0);
            if ended && over_at.is_none() {
                over_at = Some(t);
            }
            // The finish plays through: every tick for the first moments, then sparser.
            let every = if over_at.is_some_and(|o| t < o + 130) { 3 } else { 7 };
            if t % every == 0 {
                for &v in &views {
                    judge(&r, v, &format!("{seats} seats, seat {v}, tick {t}"));
                    seen += 1;
                }
            }
            if over_at.is_some_and(|o| t > o + 240) {
                break;
            }
        }
        assert!(over_at.is_some(), "{seats} seats: the siege ends");
        assert!(seen > 60, "{seats} seats: {seen} screens judged");
    }
}

/// The busiest screens there are: a pennon felled, another struck, a call
/// going up, the run of hits, the last seconds and the keys, six at the table.
#[test]
fn pennon_busiest_screens_keep_their_words_apart() {
    for seats in [2, 4, 6] {
        for age in (0..78).step_by(2) {
            for call in [
                show::Cry::Wave(2),
                show::Cry::Combo(10),
                show::Cry::Captain,
                show::Cry::Torn(2),
                show::Cry::Broken,
                show::Cry::Rally,
                show::Cry::Lead,
            ] {
                for who in [1, 2] {
                    let mut r = table(seats);
                    {
                        let s = r.pennon.as_mut().unwrap();
                        s.hold = 0;
                        s.left = 4 * 30 + 9;
                        s.show.t = 700 + age;
                        let born = 700;
                        for d in s.lanes.values_mut() {
                            d.chain = 12;
                            d.score = 410;
                            d.falls = 1;
                        }
                        let d = s.lanes.get_mut(&1).unwrap();
                        d.x = 1000 * Q;
                        d.foes[0].hp = 0;
                        d.foes[1].hp = 0;
                        d.foes[2].hp = 1;
                        d.foes[3].attack = 3;
                        for (foe, kill) in [(2u8, true), (3, false)] {
                            s.show.hits.push(show::Hit {
                                seat: 1,
                                foe,
                                x: if foe == 3 { 1130 } else { 1060 },
                                y: 82,
                                weight: if kill { crate::rules::juice::Weight::Kill } else { crate::rules::juice::Weight::Solid },
                                gain: 18,
                                chain: 12,
                                kill,
                                born: born + if kill { 0 } else { 4 },
                            });
                        }
                        s.show.calls.push(show::Call { cry: call, who: if matches!(call, show::Cry::Rally) { 0 } else { who }, first: true, born });
                        s.show.downs.push(show::Down { seat: 1, x: 990, lost: 12, pit: false, ward: false, born });
                    }
                    judge(&r, 1, &format!("busy {seats} seats {call:?} by {who} age {age}"));
                    judge(&r, 2, &format!("busy {seats} seats {call:?} by {who} age {age} (seat 2)"));
                }
            }
        }
    }
}

/// A final board with every sort of name: the podium and the rest of the table.
#[test]
fn pennon_podium_names_every_place_by_its_own_name() {
    for seats in 1..=6u32 {
        for age in [0, 12, 30, 60, 90, 140, 220] {
            let mut r = table(seats);
            {
                let s: &mut Session = r.pennon.as_mut().unwrap();
                s.hold = 0;
                s.left = 0;
                s.won = true;
                s.show.t = 3000;
                s.show.end = Some(3000 - 54 - age);
                let ids: Vec<u32> = s.lanes.keys().copied().collect();
                for (n, id) in ids.iter().enumerate() {
                    let d: &mut Lane = s.lanes.get_mut(id).unwrap();
                    d.score = 900 - 70 * n as u32;
                    d.falls = n as u32;
                    d.finish = (n == 0).then_some(1800);
                }
                s.order = ids;
            }
            for me in 1..=seats {
                judge(&r, me, &format!("podium {seats} seats age {age} seat {me}"));
            }
        }
    }
    let _ = (pennon::LENGTH, Q);
}

/// The terminal paints the same screen: its small words are written by the
/// terminal itself, in cells four wide and eight tall, and no two meet there
/// either.
#[test]
fn pennon_terminal_path_carries_its_words_in_clear_cells() {
    for seats in [1u32, 4, 6] {
        let mut r = table(seats);
        for t in 0..520u32 {
            let keys = hands(&r, t);
            r.step(&keys);
            if t % 13 != 0 {
                continue;
            }
            for me in [1, seats] {
                let (im, fx) = super::terminal_layers(&r, Some(me));
                assert_eq!((im.w, im.h, fx.w, fx.h), (384, 224, 384, 224));
                let boxes: Vec<(String, Rect)> = super::labels(&r, Some(me))
                    .into_iter()
                    .map(|(x, y, t)| {
                        let w = t.chars().count() as i32 * 4;
                        (t, (x / 4 * 4, y / 8 * 8, w, 8))
                    })
                    .collect();
                for (t, b) in &boxes {
                    assert!(b.0 >= 0 && b.1 >= 0 && b.0 + b.2 <= 384 && b.1 + b.3 <= 224, "{seats} seats tick {t}: {t:?} {b:?} leaves the cells");
                }
                for (i, (an, a)) in boxes.iter().enumerate() {
                    for (bn, b) in &boxes[i + 1..] {
                        assert!(!meets(*a, *b), "{seats} seats seat {me} tick {t}: {an:?} {a:?} meets {bn:?} {b:?} in cells");
                    }
                }
            }
        }
    }
}
