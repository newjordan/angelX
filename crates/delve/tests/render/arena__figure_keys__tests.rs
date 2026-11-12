//! Named figure keys: the rig's knight is named by what it is as it is
//! stamped (design, clip, frame, seat, facing), with its dress and its feet;
//! turned over, its name and feet turn with it; bent from its frame, it
//! goes back to its pixel hash alone.
use super::*;
use crate::render::overworld::arena::{NATIVE_H, NATIVE_W, scene_for};
use crate::render::overworld::ink::Img;
use crate::rules::{Pack as Kit, RoomKind, Rng, Run, layout};

#[test]
fn a_key_says_what_a_figure_is() {
    let key = knight_key("realm", Clip::Slash, 3, &colours_part(2, &[]));
    assert_eq!(key, "knight/realm/slash/3/seat2/r");
    assert!(is_key(&key));
    assert_eq!(parts(&key), Some(["knight", "realm", "slash", "3", "seat2", "r"]));
    assert!(!is_key("knight/realm/slash/3/seat2"), "six parts");
    assert!(!is_key("knight/realm/slash/3/seat2/up"), "facing r, l, u or d");
    assert!(!is_key("knight/realm/slash/3/seat2/x"));
    for facing in ["u", "d"] {
        assert!(is_key(&format!("knight/realm/slash/3/seat2/{facing}")), "a knight facing up or down: {facing}");
    }
    assert!(!is_key("knight/Realm/slash/3/seat2/r"), "lower case");
    assert!(!is_key("knight/../slash/3/seat2/r"));
    let company = colours_part(2, &[('7', 'A')]);
    assert!(company.starts_with("colours-") && company != colours_part(2, &[('7', 'B')]));
    assert!(is_key(&knight_key("realm", Clip::Idle, 0, &company)));
    for clip in Clip::ALL {
        assert!(is_key(&knight_key("realm", clip, 0, "seat1")), "{clip:?}");
    }
}

#[test]
fn each_seats_rig_is_named_by_its_design_and_the_old_knight_is_realm() {
    for d in quick_rig::Design::ALL {
        let rig = quick_rig::design(d).expect("each design's rig reads");
        assert_eq!(design_of(rig), Some(d.name()));
    }
    assert_eq!(design_of(quick_rig::knight().unwrap()), Some("realm"));
    let wished = Rig::parse(include_str!("../../assets/quick-rig/knight.json")).unwrap();
    assert_eq!(design_of(&wished), None, "a rig the game does not know by name");
}

#[test]
fn turned_over_a_name_faces_the_other_way_and_its_feet_mirror() {
    let mut bare = Img::new(51, 3);
    bare.put(2, 1, '7');
    let n = Named { key: "knight/realm/idle/0/seat1/r".into(), dress: "bare".into(), pivot: [20, 40], rung: crate::rules::resolution::Rung::BASE, bare: Some(Arc::new(bare)), vertical: None };
    let l = n.mirrored(51);
    assert_eq!(&*l.key, "knight/realm/idle/0/seat1/l");
    assert_eq!(l.pivot, [30, 40]);
    assert!(l.bare.as_ref().unwrap().get(48, 1).is_some(), "its bare body turned over with it");
    assert_eq!(l.mirrored(51), n, "twice is as it was");
}

fn crypt_fight(depth: u32) -> Run {
    for seed in 3..11u64 {
        let floor = layout::floor_party(depth, Kit::Crypt, 2, &mut Rng::new(seed));
        if let Some(at) = floor.rooms.iter().position(|r| r.kind == RoomKind::Fight) {
            let mut run = Run::new(seed, seed, Some("Friend"));
            run.join_seat(2, "Goose Inspector");
            run.dungeon = floor;
            run.enter_for_test(at);
            run.enemies.clear();
            return run;
        }
    }
    panic!("no fight room");
}

/// The named bodies of a frame, by the seat their key says.
fn named_bodies(run: &Run) -> Vec<crate::render::overworld::arena::Body> {
    let scene = scene_for(run, NATIVE_W, NATIVE_H, Some(1));
    scene.figures.iter().flat_map(|f| f.bodies.iter().filter(|b| b.named.is_some()).cloned()).collect()
}

#[test]
fn the_rigs_knight_is_named_as_it_is_stamped() {
    crate::rules::resolution::set_rule(None);
    let mut run = crypt_fight(1);
    for h in run.players.values_mut() {
        (h.aim_x, h.aim_y) = (1.0, 0.0);
    }
    let bodies = named_bodies(&run);
    assert_eq!(bodies.len(), 2, "both seats' knights");
    for b in &bodies {
        let n = b.named.as_ref().unwrap();
        let p = parts(&n.key).expect("a key");
        assert_eq!((p[0], p[2], p[5]), ("knight", "idle", "r"), "{}", n.key);
        // Each seat's own design (`quick_rig::Design`).
        let seat: u32 = p[4].strip_prefix("seat").and_then(|s| s.parse().ok()).expect("a seat's colours");
        assert_eq!(p[1], quick_rig::Design::of_seat(seat).name(), "{}", n.key);
        assert!(seat == 1 || seat == 2, "{}", n.key);
        assert!(n.dress.contains("shield-across"), "{} carries its shield across: {}", n.key, n.dress);
        assert!(n.dress.starts_with("bow"), "the knight's bow: {}", n.dress);
        assert_eq!(n.rung.px(), 16, "the run's first floor");
        // Its feet on the rig's pivot: the bottom of its own ink, in the middle.
        let [x, y] = n.pivot;
        assert!((0..b.img.w).contains(&x) && (0..b.img.h).contains(&y));
        let lowest = (0..b.img.h).rev().find(|&yy| (0..b.img.w).any(|xx| b.img.get(xx, yy).is_some())).unwrap();
        assert!((y - lowest).abs() <= 1, "feet at {y}, lowest ink {lowest}");
        // Its bare body: the figure's size, every pixel of it the figure's
        // but where what it carries is stamped over it.
        let bare = n.bare.as_ref().expect("its bare body");
        assert_eq!((bare.w, bare.h), (b.img.w, b.img.h));
        let (mut same, mut inked, mut over) = (0, 0, 0);
        for yy in 0..b.img.h {
            for xx in 0..b.img.w {
                match (bare.get(xx, yy), b.img.get(xx, yy)) {
                    (Some(c), Some(d)) => {
                        inked += 1;
                        if c == d { same += 1 }
                    }
                    (None, Some(_)) => over += 1,
                    _ => {}
                }
            }
        }
        assert!(inked > 60 && same * 10 >= inked * 8, "mostly as stamped: {same} of {inked}");
        assert!(over > 4, "the bow and shield are not the body ({over} px)");
    }
    // Faced left: the same frame turned over, its name and feet with it.
    let right: Vec<_> = bodies.iter().map(|b| (b.named.clone().unwrap(), b.img.w)).collect();
    for h in run.players.values_mut() {
        h.aim_x = -1.0;
    }
    for b in named_bodies(&run) {
        let n = b.named.unwrap();
        let (r, w) = right.iter().find(|(r, _)| r.key[..r.key.len() - 1] == n.key[..n.key.len() - 1]).expect("its right");
        assert!(n.key.ends_with("/l"));
        assert_eq!(n, r.mirrored(*w));
    }
}

#[test]
fn a_figure_bent_from_its_frame_keeps_only_its_hash() {
    let mut run = crypt_fight(1);
    let id = *run.players.keys().next().unwrap();
    run.players.get_mut(&id).unwrap().chilled = 30;
    let bodies = named_bodies(&run);
    assert_eq!(bodies.len(), 1, "the chilled knight is not named, the other is");
    assert!(!bodies[0].named.as_ref().unwrap().key.contains(&format!("seat{id}/")));
}

#[test]
fn the_name_carries_the_knights_rung() {
    crate::rules::resolution::set_rule(None);
    let run = crypt_fight(2);
    let bodies = named_bodies(&run);
    assert!(!bodies.is_empty());
    assert!(bodies.iter().all(|b| b.named.as_ref().unwrap().rung.px() == 32), "floor two is the 32 rung");
    crate::rules::resolution::set_rule(Some(crate::rules::resolution::Rule::Fixed(crate::rules::resolution::Rung::of(128).unwrap())));
    assert!(named_bodies(&run).iter().all(|b| b.named.as_ref().unwrap().rung.px() == 128));
    crate::rules::resolution::set_rule(None);
}

#[test]
fn a_knight_facing_up_or_down_carries_it_beside_the_side_it_is_drawn_on() {
    crate::rules::resolution::set_rule(None);
    let mut run = crypt_fight(1);
    let ask = |run: &mut Run, aim: (f32, f32)| {
        for h in run.players.values_mut() {
            (h.aim_x, h.aim_y) = aim;
        }
        named_bodies(run)
    };
    // At the sides: no facing of its own, the key as it was.
    for aim in [(1.0, 0.0), (-1.0, 0.0), (1.0, 0.7), (-1.0, -0.7)] {
        for b in ask(&mut run, aim) {
            let n = b.named.unwrap();
            assert_eq!((n.vertical, n.faced_key()), (None, None), "{aim:?}");
        }
    }
    // Up and down the room: `u` and `d`, and the key still the rig's side.
    for (aim, facing) in [((0.0, -1.0), 'u'), ((0.0, 1.0), 'd'), ((0.3, -1.0), 'u'), ((-0.5, 1.0), 'd')] {
        let bodies = ask(&mut run, aim);
        assert_eq!(bodies.len(), 2, "{aim:?}");
        for b in bodies {
            let n = b.named.unwrap();
            assert_eq!(n.vertical, Some(facing), "{aim:?}");
            let (key, faced) = (n.key.to_string(), n.faced_key().expect("a faced key"));
            assert!(is_key(&faced) && faced.ends_with(&format!("/{facing}")), "{faced}");
            assert_eq!(&key[..key.len() - 1], &faced[..faced.len() - 1], "the same figure");
            assert!(key.ends_with(if aim.0 < -0.3 { "/l" } else { "/r" }), "{aim:?}: still the side it is drawn on: {key}");
            // Turned over, a knight seen from behind is the same drawing on either hand.
            assert_eq!(n.mirrored(b.img.w).faced_key(), n.faced_key());
        }
    }
}

#[test]
fn a_blow_bends_a_name_into_another_width_on_the_same_feet() {
    let mut bare = Img::new(51, 3);
    bare.put(2, 1, '7');
    let n = Named { key: "knight/realm/hurt/1/seat1/r".into(), dress: "bare".into(), pivot: [25, 40], rung: crate::rules::resolution::Rung::BASE, bare: Some(Arc::new(bare)), vertical: Some('d') };
    // The same picture: the same feet, but the body alone is the upright one's, so it goes.
    let same = n.bent(51, 51);
    assert_eq!((same.pivot, same.bare.is_none(), same.vertical), ([25, 40], true, Some('d')));
    // The bend widens the picture and the game stands it by its middle (`stand`): the feet are put where the upright picture's are.
    for to in [52, 53, 55] {
        let b = n.bent(51, to);
        let upright_left = 100 - 51 / 2;
        let bent_left = 100 - to / 2;
        assert_eq!(bent_left + b.pivot[0], upright_left + n.pivot[0], "{to}");
    }
}
