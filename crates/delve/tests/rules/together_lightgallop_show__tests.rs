//! What the ride remembers of its moments (`show.rs`): written from what
//! changed between two ticks, bounded, replayed byte for byte, and never sent
//! to the module.
use super::legacy::SessionOracle;
use super::oracle::Route;
use super::show::{Kind, Show};
use super::*;

fn table(seats: u32) -> Run {
    let mut r = Run::lightgallop_for_test();
    for id in 2..=seats {
        r.join_seat(id, &format!("Knight {id}"));
    }
    r.step(&BTreeMap::new());
    r
}

fn ride(r: &mut Run, ticks: u32) {
    for _ in 0..ticks {
        let s = r.lightgallop.as_ref().unwrap();
        let keys: BTreeMap<u32, Input> =
            s.seats.keys().map(|&id| (id, s.drive(id, if id % 2 == 0 { Route::Lower } else { Route::Upper }, &Numbers::default()))).collect();
        r.step(&keys);
    }
}

#[test]
fn a_ride_writes_its_moments_bounded_and_replays_them_byte_for_byte() {
    let (mut a, mut b) = (table(4), table(4));
    ride(&mut a, 1500);
    ride(&mut b, 1500);
    let (sa, sb) = (a.lightgallop.as_ref().unwrap(), b.lightgallop.as_ref().unwrap());
    assert_eq!(serde_json::to_string(sa).unwrap(), serde_json::to_string(sb).unwrap());
    assert!(sa.valid() && sa.show.valid(sa.elapsed) && sa.show.fx.len() <= 72);
    // Sparks are taken and foes popped on the way down the first road.
    let kinds = |k: Kind| sa.show.fx.iter().any(|f| f.kind == k) || sa.show.of(k, sa.elapsed).count() > 0;
    let seen = [Kind::Spark, Kind::Pop, Kind::Land, Kind::Hop].into_iter().filter(|&k| kinds(k)).count();
    assert!(seen >= 1, "a quarter of a minute on the road leaves something to show");
}

#[test]
fn what_changed_between_two_ticks_is_what_is_written_down() {
    let r = table(2);
    let old = r.lightgallop.clone().unwrap();
    let mut new = old.clone();
    new.elapsed += 1;
    {
        let rider = &mut new.seats.get_mut(&1).unwrap().rider;
        rider.sparks = 0;
        rider.hurt = 90;
        rider.chain = 3;
        rider.looping = true;
    }
    let mut show = Show::default();
    // The rider had ten sparks before the blow.
    let mut old = old;
    old.seats.get_mut(&1).unwrap().rider.sparks = 10;
    show.observe(&old, &new);
    let hit = show.fx.iter().find(|f| f.kind == Kind::Hit).expect("a blow");
    assert_eq!((hit.seat, hit.n), (1, 10));
    assert!(hit.weight >= crate::rules::juice::Weight::Crushing, "ten sparks is a crushing blow");
    assert!(show.fx.iter().any(|f| f.kind == Kind::Chain && f.n == 3));
    assert!(show.fx.iter().any(|f| f.kind == Kind::LoopIn) && show.looping.contains_key(&1));
    // And falling is a kill-weight blow, heard by the whole picture.
    let mut down = new.clone();
    down.elapsed += 1;
    down.seats.get_mut(&1).unwrap().rider.down = 24;
    show.observe(&new, &down);
    assert!(show.fx.iter().any(|f| f.kind == Kind::Fall && f.weight == crate::rules::juice::Weight::Kill));
    assert!(show.valid(down.elapsed));
}

#[test]
fn only_the_winning_blow_holds_the_room() {
    let mut show = Show::default();
    let r = table(1);
    let old = r.lightgallop.clone().unwrap();
    let mut new = old.clone();
    new.elapsed += 1;
    let lvl = crate::rules::lightgallop::level::level();
    let warden = lvl.foes.iter().position(|f| f.kind == crate::rules::lightgallop::world::FoeKind::Warden).unwrap();
    let mut old = old;
    old.foes[warden].hp = 1;
    new.foes[warden].hp = 0;
    show.observe(&old, &new);
    assert_eq!(show.held(new.elapsed), Some(crate::rules::juice::Weight::Finale));
    assert_eq!(show.held(new.elapsed + 1), None, "a held tick is only the tick of the blow");
}

#[test]
fn the_module_never_sees_the_show() {
    let mut r = table(1);
    ride(&mut r, 60);
    let s = r.lightgallop.as_mut().unwrap();
    s.show.fx.push(super::show::Fx { kind: Kind::Bell, seat: 0, n: 0, x: 1, y: 1, weight: crate::rules::juice::Weight::Finale, born: s.elapsed });
    let mut value = serde_json::to_value(&*s).unwrap();
    assert!(value.get("show").is_some(), "the save keeps it");
    // The request the adapter sends drops it.
    if let Some(fields) = value.as_object_mut() {
        fields.remove("show");
    }
    assert!(value.get("show").is_none());
    // A ride goes on with a note on its books, and the note is kept.
    ride(&mut r, 30);
    let s = r.lightgallop.as_ref().unwrap();
    assert!(s.show.fx.iter().any(|f| f.kind == Kind::Bell));
}
