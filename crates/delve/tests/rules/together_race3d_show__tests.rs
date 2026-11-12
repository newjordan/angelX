//! What the race remembers of a tick: wrecks and their weight, passes and
//! the lead, the line and a photo finish, and the hold a hard one asks for.
use super::*;
use crate::realm::Spoil;
use crate::rules::juice::Weight;
use super::{Fx, Kind, Show};
use crate::rules::race3d::{Course, Race3d, TITLE, Wreck};

fn racing(course: Course, seats: u32) -> Run {
    let mut run = (0..80u64)
        .find_map(|seed| {
            let mut r = Run::new(seed, 1, None);
            for _ in 0..6 {
                r.descend_for_test();
                r.race3d_hall_for_test();
                if Course::of_floor(r.dungeon.depth) == course && r.race3d_room.is_some() {
                    return Some(r);
                }
            }
            None
        })
        .expect("a floor with that race");
    for id in 2..=seats {
        run.join_seat(id, &format!("Knight {id}"));
    }
    run.enemies.clear();
    for hero in run.players.values_mut() {
        hero.carried.add(Spoil::Gold, 60);
    }
    run.enter_for_test(run.race3d_room.unwrap());
    run.race3d.as_mut().unwrap().t = TITLE;
    run
}

fn race(run: &Run) -> Race3d {
    run.race3d.clone().unwrap()
}

#[test]
fn a_wreck_is_written_with_its_weight() {
    let run = racing(Course::Cart, 2);
    let old = race(&run);
    let mut new = old.clone();
    new.t += 1;
    let mut was = old.clone();
    was.riders.get_mut(&1).unwrap().vs = 10.0;
    let r = new.riders.get_mut(&1).unwrap();
    (r.crash, r.wreck) = (45, Wreck::Timber);
    let mut show = Show::default();
    show.observe(&was, &new);
    let f = show.fx.iter().find(|f| f.kind == Kind::Wreck).expect("the wreck is written down");
    assert_eq!((f.seat, f.wreck, f.weight), (1, Wreck::Timber, Weight::Crushing));
    assert_eq!(show.held(new.t), Some(Weight::Crushing), "a hard wreck asks the room to hold");
    // A slow one does not, and a fall is no blow at all.
    let mut slow = old.clone();
    slow.riders.get_mut(&1).unwrap().vs = 3.0;
    let mut show = Show::default();
    show.observe(&slow, &new);
    assert_eq!(show.held(new.t), Some(Weight::Solid), "a crawl is a solid thing and no more: Solid never stops the room");
}

#[test]
fn a_pass_is_called_only_when_it_is_clear_and_the_lead_is_its_own_call() {
    let run = racing(Course::Griffin, 3);
    let mut a = race(&run);
    for (id, s) in [(1u32, 10.0f32), (2, 12.0), (3, 14.0)] {
        a.riders.get_mut(&id).unwrap().s = s;
    }
    let mut show = Show::default();
    // The first sight of the field is silent: they are only lined up.
    show.observe(&a, &a);
    assert!(show.fx.is_empty());
    // Knight one draws level with two: not clear, nothing is called.
    let mut b = a.clone();
    b.t += 1;
    b.riders.get_mut(&1).unwrap().s = 12.4;
    show.observe(&a, &b);
    assert!(show.fx.is_empty(), "wheel to wheel is not a pass");
    // Clear of two: a pass, into second.
    let mut c = b.clone();
    c.t += 1;
    c.riders.get_mut(&1).unwrap().s = 13.4;
    show.observe(&b, &c);
    let pass = show.fx.iter().find(|f| f.kind == Kind::Pass).expect("a pass");
    assert_eq!((pass.seat, pass.other, pass.n), (1, 2, 2));
    // Past three too: the lead.
    let mut d = c.clone();
    d.t += 1;
    d.riders.get_mut(&1).unwrap().s = 15.2;
    show.observe(&c, &d);
    let lead = show.fx.iter().find(|f| f.kind == Kind::Lead).expect("the lead");
    assert_eq!((lead.seat, lead.other), (1, 3));
}

#[test]
fn the_first_home_is_the_finale_and_a_close_second_is_a_photo_finish() {
    let run = racing(Course::Cart, 3);
    let old = race(&run);
    let mut mid = old.clone();
    mid.t += 1;
    mid.riders.get_mut(&1).unwrap().finish = Some(mid.t);
    let mut show = Show::default();
    show.observe(&old, &mid);
    let first = show.fx.iter().find(|f| f.kind == Kind::Finish).unwrap();
    assert_eq!((first.seat, first.n, first.weight), (1, 1, Weight::Finale));
    assert!(show.fx.iter().all(|f| f.kind != Kind::Photo));
    // The second home eight ticks later: a photograph.
    let mut late = mid.clone();
    late.t += 8;
    late.riders.get_mut(&2).unwrap().finish = Some(late.t);
    show.observe(&mid, &late);
    let photo = show.fx.iter().find(|f| f.kind == Kind::Photo).expect("a photo finish");
    assert_eq!((photo.seat, photo.other, photo.n), (2, 1, 8));
    // A third, a long way behind, is not.
    let mut last = late.clone();
    last.t += 60;
    last.riders.get_mut(&3).unwrap().finish = Some(last.t);
    let before = show.fx.len();
    show.observe(&late, &last);
    assert_eq!(show.fx.len(), before + 1, "only the finish");
}

#[test]
fn the_show_is_bounded_and_left_out_of_a_quiet_save() {
    let run = racing(Course::Shaft, 2);
    let mut r = race(&run);
    assert!(r.show.quiet() && r.valid());
    let json = serde_json::to_value(&r).unwrap();
    assert!(json.get("show").is_none(), "a quiet show is not in the save");
    for k in 0..200 {
        r.show.fx.push(Fx {
            kind: Kind::Hop,
            seat: 1,
            other: 0,
            n: 0,
            at: [0.0; 3],
            weight: Weight::Graze,
            wreck: Wreck::None,
            born: k,
        });
    }
    assert!(!r.valid(), "a runaway show is refused");
}

#[test]
fn a_race_played_with_wrecks_keeps_a_valid_show_and_holds_the_room() {
    let mut run = racing(Course::Griffin, 2);
    let t = track(Course::Griffin);
    {
        let r = run.race3d.as_mut().unwrap();
        let a = r.riders.get_mut(&1).unwrap();
        (a.s, a.x, a.z, a.vs) = (t.posts[1] + 22.0 - 9.0, -2.5, 0.0, 10.0);
    }
    let mut held = false;
    for _ in 0..60 {
        run.step(&BTreeMap::new());
        held |= run.hitstop.left > 0;
        assert!(run.race3d.as_ref().unwrap().valid());
    }
    assert!(held, "the room was held for the wreck");
    let race = run.race3d.as_ref().unwrap();
    assert!(race.show.fx.iter().any(|f| f.kind == Kind::Wreck && f.seat == 1));
}
