//! The Phantom Lancer: a spear fighter who fights beside images of
//! herself. The images thrust when she does (`ults::phantom_strikes`),
//! a thrown lance, a blink and a stance make more of them, and the
//! ultimate rings her with four.

use super::*;
use crate::rules::ults::{PHANTOM_SHARE, Phantom};
use crate::rules::spells::IMPOSTURE_MAX;

/// A knight of `kit` at (12, 14) aiming east, one tough foe at `(x, y)`.
fn arena(kit: &str, x: f32, y: f32) -> Run {
    let mut r = Run::new(21, 1, None);
    let fight = r.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && !r.great()).unwrap();
    r.enter_for_test(fight);
    r.calm_for_test();
    r.enemies.clear();
    r.projectiles.clear();
    assert!(r.take_up(1, kit));
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y, h.aim_x, h.aim_y) = (12.0, 14.0, 1.0, 0.0);
    h.invulnerable = 100_000;
    foe(&mut r, x, y);
    r.phase = Phase::Fighting;
    r
}

fn foe(r: &mut Run, x: f32, y: f32) -> usize {
    r.spawn_at_for_test(EnemyKind::Demon, x, y);
    let i = r.enemies.len() - 1;
    let e = &mut r.enemies[i];
    e.hp = 10000;
    e.max_hp = 10000;
    e.seen_hp = 10000;
    e.age = TELEGRAPH;
    i
}

fn step(r: &mut Run, i: Input) {
    r.step(&BTreeMap::from([(1, i)]));
}

fn idle(r: &mut Run, ticks: u32) {
    for _ in 0..ticks {
        step(r, Input::default());
    }
}

fn cast(r: &mut Run, key: u8) {
    step(r, Input { cast: key, ..Default::default() });
}

fn lost(r: &Run, i: usize) -> u32 {
    r.enemies[i].max_hp - r.enemies[i].hp
}

fn ult(r: &mut Run) {
    r.players.get_mut(&1).unwrap().ult_charge = ults::ULT_FULL;
    step(r, Input { ult: true, ..Default::default() });
}

fn mine(r: &Run) -> Vec<Phantom> {
    r.phantoms.iter().copied().filter(|p| p.owner == 1).collect()
}

fn walkable(r: &Run, (x, y): (f32, f32)) -> bool {
    r.settlement_walkable_for_test(x, y)
}

#[test]
fn the_phantom_lancer_is_a_champion_with_its_own_calling() {
    assert_eq!(champions::Champion::of_kit("phantomlancer"), Some(champions::Champion::PhantomLancer));
    assert_eq!(champions::Champion::PhantomLancer.spells(), ["spirit-lance", "doppelganger", "juxtapose"]);
    let k = knights::knight("phantomlancer").unwrap();
    assert_eq!((k.arm, k.guard, k.max_hp), ("phantom-lance", "dodge-roll", 120));
    assert!(knights::knight("lancer").unwrap().max_hp > k.max_hp, "squishier than the Lancer");
    let r = Run::new(21, 1, None);
    let arm = r.book.get("phantom-lance").expect("the arm is in the book");
    let lancer = r.book.get("war-lance").unwrap();
    assert_eq!(arm.kind, cards::Kind::Arm);
    // The same long straight thrust as the Lancer's.
    let (a, b) = (arm.arm.clone().unwrap(), lancer.arm.clone().unwrap());
    let cards::Arm::Forged(a) = a else { panic!("a forged arm") };
    let cards::Arm::Forged(b) = b else { panic!("a forged arm") };
    assert_eq!(a.melee, b.melee);
    assert!(a.melee.is_some());
}

#[test]
fn the_ultimate_is_appended_so_saved_names_keep() {
    let all = ults::Ult::ALL;
    assert!(all.len() >= 23);
    assert_eq!(all[21], ults::Ult::SkewerDrive, "the old order is untouched");
    assert_eq!(all[22], ults::Ult::PhantomLegion, "appended after the Skewer Drive");
    assert_eq!(ults::Ult::of(Some("phantomlancer")), ults::Ult::PhantomLegion);
    assert_eq!(ults::Ult::PhantomLegion.name(), "Phantom Legion");
    assert_eq!(ults::Ult::PhantomLegion.word(), "phantom_legion");
    assert_eq!(serde_json::to_string(&ults::Ult::PhantomLegion).unwrap(), "\"phantom_legion\"");
    assert_eq!(serde_json::to_string(&ults::Ult::Phantasm).unwrap(), "\"phantasm\"");
    assert_eq!(serde_json::to_string(&ults::Ult::SkewerDrive).unwrap(), "\"skewer_drive\"");
    assert_eq!(ults::Ult::parse("Phantom Legion"), Some(ults::Ult::PhantomLegion));
    assert_eq!(ults::Ult::parse("phantom_legion"), Some(ults::Ult::PhantomLegion));
}

#[test]
fn the_patron_is_pickable_and_arms_the_knight() {
    let mut r = Run::new(21, 1, None);
    let i = alehouse::PATRONS.iter().position(|p| p.kit == "phantomlancer").expect("a seat in the tavern");
    let p = &alehouse::PATRONS[i];
    assert_eq!(alehouse::patron_room(i), RoomKind::Alehouse, "in the hall, not the muster tent");
    assert!(p.label.chars().next().unwrap().is_uppercase() && !p.label.contains(' '));
    assert!(p.pitch.len() < 80);
    r.alehouse.company_page = (0..alehouse::COMPANY_PAGES).find(|&page| alehouse::patron_visible(page, i)).unwrap();
    r.pick_patron(1, i);
    let h = &r.players[&1];
    assert_eq!(h.knight.as_deref(), Some("phantomlancer"));
    assert_eq!(h.arm.as_deref(), Some("phantom-lance"));
    assert!(h.forged.as_ref().is_some_and(|w| w.melee.is_some()), "the thrust is armed");
    assert_eq!(champions::of(h), Some(champions::Champion::PhantomLancer));
    assert_eq!(h.spells[..3], [Some("spirit-lance".to_string()), Some("doppelganger".to_string()), Some("juxtapose".to_string())]);
    assert_eq!(h.ult(), ults::Ult::PhantomLegion);
    assert_eq!(h.max_hp, 120 + h.home_hp);
    // Nobody else's seat is shared with it.
    for (_, q) in alehouse::PATRONS.iter().enumerate().filter(|(j, _)| *j != i && alehouse::patron_room(*j) == RoomKind::Alehouse) {
        assert!((p.at.0 - q.at.0).hypot(p.at.1 - q.at.1) > 1.2, "{} sits apart from {}", p.label, q.label);
    }
}

#[test]
fn an_image_thrusts_when_its_knight_does_for_a_share_once_each() {
    // A foe in front of the knight, and one far to the side beside an image
    // the knight's blade cannot reach.
    let mut r = arena("phantomlancer", 14.0, 14.0);
    let near = 0;
    let far = foe(&mut r, 15.0, 6.0);
    r.phantoms.push(Phantom::standing(1, (12.0, 6.0), 600));
    step(&mut r, Input { fire: true, ..Default::default() });
    let full = lost(&r, near);
    assert!(full > 0, "the knight's own thrust lands");
    assert_eq!(lost(&r, far), (full * PHANTOM_SHARE / 100).max(1), "the image strikes for its share");
    // Held on, the next thrust is a whole beat away: the image waits for it.
    let once = lost(&r, far);
    idle(&mut r, 3);
    assert_eq!(lost(&r, far), once, "once per thrust");
    // Two images, two strokes.
    let mut r = arena("phantomlancer", 14.0, 14.0);
    let far = foe(&mut r, 15.0, 6.0);
    r.phantoms.push(Phantom::standing(1, (12.0, 6.0), 600));
    r.phantoms.push(Phantom::standing(1, (12.0, 6.5), 600));
    step(&mut r, Input { fire: true, ..Default::default() });
    assert_eq!(lost(&r, far), 2 * (lost(&r, 0) * PHANTOM_SHARE / 100).max(1), "each image strikes once");
}

#[test]
fn an_image_strikes_nothing_for_an_owner_with_no_images_or_no_melee_arm() {
    let mut r = arena("phantomlancer", 14.0, 14.0);
    let far = foe(&mut r, 15.0, 6.0);
    step(&mut r, Input { fire: true, ..Default::default() });
    assert!(lost(&r, 0) > 0);
    assert_eq!(lost(&r, far), 0, "no images, no second stroke");
    // An archer with an image beside the foe: arrows are copied, nothing is struck.
    let mut r = arena("percival", 40.0, 14.0);
    let far = foe(&mut r, 38.0, 6.0);
    r.phantoms.push(Phantom::standing(1, (12.0, 6.0), 600));
    step(&mut r, Input { fire: true, ..Default::default() });
    assert_eq!(lost(&r, far), 0, "a bow does not thrust (its arrows are still in the air)");
    // Another knight's images do not strike for this one.
    let mut r = arena("phantomlancer", 14.0, 14.0);
    let far = foe(&mut r, 15.0, 6.0);
    r.phantoms.push(Phantom::standing(2, (12.0, 6.0), 600));
    step(&mut r, Input { fire: true, ..Default::default() });
    assert_eq!(lost(&r, far), 0);
}

#[test]
fn an_image_credits_the_knight_with_the_damage_it_did() {
    let mut r = arena("phantomlancer", 40.0, 14.0);
    let far = foe(&mut r, 15.0, 6.0);
    // The knight's thrust must find something: a second foe in front.
    foe(&mut r, 14.0, 14.0);
    r.phantoms.push(Phantom::standing(1, (12.0, 6.0), 600));
    let before = r.players[&1].ult_charge;
    step(&mut r, Input { fire: true, ..Default::default() });
    assert!(lost(&r, far) > 0);
    assert!(r.players[&1].ult_charge > before, "the thrust charges the ultimate");
    assert_eq!(r.enemies[far].wits.foe, 1, "the foe knows whose it was");
}

#[test]
fn images_keep_the_room_cap_and_the_oldest_of_the_owner_gives_way() {
    let mut phantoms = Vec::new();
    for k in 0..10 {
        let made = ults::add_image(&mut phantoms, Phantom::standing(1, (k as f32, 0.0), 100 + k));
        assert!(made, "the knight's own oldest gives way");
        assert!(phantoms.len() <= IMPOSTURE_MAX);
    }
    assert_eq!(phantoms.len(), IMPOSTURE_MAX);
    assert!(phantoms.iter().all(|p| p.left >= 106), "the shortest-lived went first: {phantoms:?}");
    // Another knight's are never taken.
    let mut room: Vec<Phantom> = (0..IMPOSTURE_MAX).map(|k| Phantom::standing(2, (k as f32, 0.0), 50)).collect();
    assert!(!ults::add_image(&mut room, Phantom::standing(1, (0.0, 0.0), 500)));
    assert_eq!(room.len(), IMPOSTURE_MAX);
    assert!(room.iter().all(|p| p.owner == 2));
}

#[test]
fn an_image_fades_when_its_time_is_up() {
    let mut r = arena("phantomlancer", 40.0, 14.0);
    r.enemies.clear();
    r.phantoms.push(Phantom::standing(1, (14.0, 14.0), 5));
    r.phantoms.push(Phantom::beside(1, (12.0, 14.0), (0.0, 2.0), 9));
    idle(&mut r, 4);
    assert_eq!(r.phantoms.len(), 2);
    idle(&mut r, 1);
    assert_eq!(r.phantoms.len(), 1, "the first is gone on its fifth tick");
    idle(&mut r, 4);
    assert!(r.phantoms.is_empty());
}

#[test]
fn a_standing_image_stays_and_a_following_one_keeps_its_place_beside_the_knight() {
    let mut r = arena("phantomlancer", 40.0, 14.0);
    r.enemies.clear();
    r.phantoms.push(Phantom::standing(1, (15.0, 12.0), 300));
    r.phantoms.push(Phantom::beside(1, (12.0, 14.0), (0.0, 2.0), 300));
    for _ in 0..10 {
        step(&mut r, Input { move_x: 1, ..Default::default() });
    }
    let h = r.players[&1].x;
    assert!(h > 12.5, "the knight walked");
    let (standing, following) = (r.phantoms.iter().find(|p| p.fixed).unwrap(), r.phantoms.iter().find(|p| !p.fixed).unwrap());
    assert_eq!((standing.x, standing.y), (15.0, 12.0));
    assert!((following.x - h).abs() < 0.01 && (following.y - 16.0).abs() < 0.01, "{following:?}");
}

#[test]
fn the_spirit_lance_flies_through_every_foe_and_an_image_stands_where_it_ends() {
    let mut r = arena("phantomlancer", 16.0, 14.0);
    for x in [19.5, 23.0] {
        foe(&mut r, x, 14.0);
    }
    cast(&mut r, 1);
    assert_eq!(r.champions.throws.len(), 1);
    assert_eq!(r.champions.throws[0].kind, champions::Thrown::SpiritLance);
    idle(&mut r, champions::SPIRIT_LIFE + 2);
    for i in 0..3 {
        assert!(lost(&r, i) >= champions::SPIRIT_DAMAGE, "foe {i} on the line: {}", lost(&r, i));
    }
    assert!(r.champions.throws.is_empty());
    let images = mine(&r);
    assert_eq!(images.len(), 1, "exactly one image: {images:?}");
    let image = images[0];
    assert!(image.fixed);
    assert!(image.left > champions::SPIRIT_IMAGE - 8 && image.left <= champions::SPIRIT_IMAGE, "six seconds: {}", image.left);
    assert!(image.x > 20.0 && walkable(&r, (image.x, image.y)), "where the lance ended, on floor: {image:?}");
    assert_eq!(r.players[&1].spell_cooldowns[0], 8 * HZ - (champions::SPIRIT_LIFE + 2), "its wait is the card's");
}

#[test]
fn a_spirit_lance_that_hits_a_wall_leaves_its_image_on_the_floor() {
    let mut r = arena("phantomlancer", 40.0, 14.0);
    r.enemies.clear();
    let w = r.room().width();
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y) = (w - 4.0, 14.0);
    cast(&mut r, 1);
    idle(&mut r, champions::SPIRIT_LIFE + 2);
    let images = mine(&r);
    assert_eq!(images.len(), 1);
    assert!(walkable(&r, (images[0].x, images[0].y)), "never in stone: {:?}", images[0]);
}

#[test]
fn doppelganger_blinks_into_the_open_and_leaves_three_images() {
    let mut r = arena("phantomlancer", 40.0, 14.0);
    r.enemies.clear();
    cast(&mut r, 2);
    let h = &r.players[&1];
    assert!((h.x - (12.0 + champions::DOPPEL_REACH)).abs() < 0.5 && (h.y - 14.0).abs() < 0.01, "blinked: {} {}", h.x, h.y);
    let images = mine(&r);
    assert_eq!(images.len(), 3, "exactly three images");
    assert!(images.iter().all(|p| p.fixed && p.left <= champions::DOPPEL_IMAGE && p.left + 3 >= champions::DOPPEL_IMAGE));
    let at_start = images.iter().filter(|p| (p.x - 12.0).hypot(p.y - 14.0) < 1.2).count();
    let at_end = images.iter().filter(|p| (p.x - h.x).hypot(p.y - h.y) < 1.2).count();
    assert_eq!((at_start, at_end), (2, 1), "two where she was, one where she is: {images:?}");
    assert_eq!(r.players[&1].spell_cooldowns[1], 12 * HZ, "its wait is the card's");
}

#[test]
fn doppelganger_never_ends_in_a_wall_whichever_way_it_is_aimed() {
    let probe = Run::new(21, 1, None);
    let _ = probe;
    let mut tried = 0;
    for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0), (0.7, 0.7), (-0.7, 0.7), (0.7, -0.7), (-0.7, -0.7)] {
        let mut r = arena("phantomlancer", 40.0, 14.0);
        r.enemies.clear();
        let (w, hgt) = (r.room().width(), r.room().height());
        for (x, y) in [(2.0, hgt / 2.0), (w - 2.0, hgt / 2.0), (w / 2.0, 2.0), (w / 2.0, hgt - 2.0), (w / 2.0, hgt / 2.0)] {
            let mut r = r.clone();
            if !walkable(&r, (x, y)) {
                continue;
            }
            let h = r.players.get_mut(&1).unwrap();
            (h.x, h.y, h.aim_x, h.aim_y) = (x, y, dx, dy);
            let room = r.at;
            cast(&mut r, 2);
            tried += 1;
            if r.at != room {
                // Blinked into a doorway and out of the room: the images stay behind with it.
                continue;
            }
            let h = &r.players[&1];
            assert!(walkable(&r, (h.x, h.y)), "ended in stone at {} {} from {x} {y} aim {dx} {dy}", h.x, h.y);
            assert_eq!(mine(&r).len(), 3, "still exactly three");
            for p in mine(&r) {
                assert!(walkable(&r, (p.x, p.y)), "an image in stone at {} {}", p.x, p.y);
            }
        }
    }
    assert!(tried >= 24, "{tried}");
}

#[test]
fn doppelganger_does_not_end_inside_a_foe() {
    let mut r = arena("phantomlancer", 17.5, 14.0);
    cast(&mut r, 2);
    let h = &r.players[&1];
    let e = &r.enemies[0];
    assert!((e.x - h.x).hypot(e.y - h.y) >= e.radius() + HERO_RADIUS - 0.01, "short of the foe: {} vs {}", h.x, e.x);
}

#[test]
fn recasting_doppelganger_keeps_the_cap() {
    let mut r = arena("phantomlancer", 40.0, 14.0);
    r.enemies.clear();
    for _ in 0..3 {
        r.players.get_mut(&1).unwrap().spell_cooldowns = Default::default();
        cast(&mut r, 2);
        assert!(r.phantoms.len() <= IMPOSTURE_MAX, "{}", r.phantoms.len());
        let h = r.players.get_mut(&1).unwrap();
        h.aim_x = -h.aim_x;
    }
    assert_eq!(r.phantoms.len(), IMPOSTURE_MAX);
}

#[test]
fn juxtapose_is_a_timed_stance() {
    let mut r = arena("phantomlancer", 40.0, 14.0);
    cast(&mut r, 3);
    assert_eq!(r.players[&1].champ.juxta, champions::JUXTA_TICKS - 1);
    assert_eq!(r.players[&1].spell_cooldowns[2], 14 * HZ, "its wait is the card's");
    idle(&mut r, champions::JUXTA_TICKS - 1);
    assert_eq!(r.players[&1].champ.juxta, 0, "eight seconds");
}

#[test]
fn juxtapose_makes_an_image_on_about_a_third_of_thrusts_and_never_without_it() {
    let r = arena("phantomlancer", 40.0, 14.0);
    let grid = Grid { room: r.room(), barred: false };
    let mut hero = r.players[&1].clone();
    let mut made = 0;
    for salt in 0..3000u64 {
        let mut phantoms = Vec::new();
        hero.champ.juxta = 10;
        champions::juxtapose(&mut phantoms, &hero, 1, salt, &grid);
        made += phantoms.len();
        if let Some(p) = phantoms.first() {
            assert!(!p.fixed && p.left == champions::JUXTA_IMAGE && (p.dx.hypot(p.dy) - 1.8).abs() < 0.01, "{p:?}");
            assert!(walkable(&r, (p.x, p.y)));
        }
        hero.champ.juxta = 0;
        let mut none = Vec::new();
        champions::juxtapose(&mut none, &hero, 1, salt, &grid);
        assert!(none.is_empty(), "the stance is spent");
    }
    assert!((850..1150).contains(&made), "one in three: {made} of 3000");
}

#[test]
fn juxtapose_is_deterministic_for_a_seed_and_adds_images_through_the_thrust() {
    let run = || {
        let mut r = arena("phantomlancer", 14.5, 14.0);
        cast(&mut r, 3);
        let mut seen = 0;
        let mut trail = Vec::new();
        for _ in 0..champions::JUXTA_TICKS {
            step(&mut r, Input { fire: true, ..Default::default() });
            seen = seen.max(mine(&r).len());
            trail.push(mine(&r).len());
        }
        (serde_json::to_string(&r.phantoms).unwrap(), seen, trail)
    };
    let (a, seen, trail) = run();
    let (b, _, again) = run();
    assert_eq!(a, b, "the same seed makes the same images");
    assert_eq!(trail, again);
    assert!(seen > 0, "eight seconds of thrusts make at least one image");
    assert!(seen <= IMPOSTURE_MAX);
}

#[test]
fn phantom_legion_rings_the_knight_with_four_images_for_eight_seconds() {
    let mut r = arena("phantomlancer", 40.0, 14.0);
    r.enemies.clear();
    ult(&mut r);
    let images = mine(&r);
    assert_eq!(images.len(), champions::LEGION_IMAGES);
    assert_eq!(images.len(), 4);
    let h = &r.players[&1];
    for p in &images {
        assert!(!p.fixed, "they keep their place beside her");
        let d = (p.x - h.x).hypot(p.y - h.y);
        assert!(d > 1.0 && d < 2.7, "ringed: {d}");
        assert!(walkable(&r, (p.x, p.y)));
        assert!(p.left <= champions::LEGION_TICKS);
    }
    // They keep the ring as she walks.
    for _ in 0..10 {
        step(&mut r, Input { move_x: 1, ..Default::default() });
    }
    let h = &r.players[&1];
    for p in mine(&r) {
        assert!((p.x - h.x - p.dx).abs() < 0.01 && (p.y - h.y - p.dy).abs() < 0.01);
    }
    // Eight seconds from the cast, no more.
    idle(&mut r, champions::LEGION_TICKS - 12);
    assert_eq!(mine(&r).len(), 4, "still there a tick before");
    idle(&mut r, 1);
    assert!(mine(&r).is_empty(), "gone at eight seconds");
    assert!(r.players[&1].ult_charge < ults::ULT_FULL / 10, "the charge is spent: {}", r.players[&1].ult_charge);
}

#[test]
fn phantom_legion_keeps_the_cap_and_a_recast_replaces_the_ring() {
    let mut r = arena("phantomlancer", 40.0, 14.0);
    r.enemies.clear();
    ult(&mut r);
    idle(&mut r, 5);
    ult(&mut r);
    assert_eq!(mine(&r).len(), 4, "a recast replaces the ring, never adds to it");
    assert!(r.phantoms.len() <= IMPOSTURE_MAX);
    // Another knight's image fills a place: the ring is the room's to humour.
    let mut r = arena("phantomlancer", 40.0, 14.0);
    r.enemies.clear();
    r.phantoms.push(Phantom::standing(2, (30.0, 8.0), 600));
    ult(&mut r);
    assert!(r.phantoms.len() <= IMPOSTURE_MAX);
    assert_eq!(mine(&r).len(), IMPOSTURE_MAX - 1);
}

#[test]
fn phantom_legion_images_strike_with_every_thrust() {
    let mut r = arena("phantomlancer", 40.0, 14.0);
    r.enemies.clear();
    ult(&mut r);
    let near = foe(&mut r, 15.0, 14.0);
    let images = mine(&r);
    let h = &r.players[&1];
    let aimed = |p: &Phantom| {
        let e = &r.enemies[near];
        let (dx, dy) = (e.x - p.x, e.y - p.y);
        dx.hypot(dy) <= 5.6 + e.radius() && dy.atan2(dx).abs() <= 25f32.to_radians()
    };
    let striking = images.iter().filter(|p| aimed(p)).count();
    assert!(striking >= 1, "at least one stands where a thrust east reaches: {images:?} from {} {}", h.x, h.y);
    step(&mut r, Input { fire: true, ..Default::default() });
    let blow = r.players[&1].scaled(30);
    let share = (blow * PHANTOM_SHARE / 100).max(1);
    assert_eq!(lost(&r, near), blow + striking as u32 * share, "the knight's blow and one share from each image that can reach");
}

#[test]
fn a_run_holding_images_the_stance_and_the_new_ultimate_round_trips() {
    let mut r = arena("phantomlancer", 14.5, 14.0);
    cast(&mut r, 3);
    ult(&mut r);
    r.phantoms.push(Phantom::standing(1, (15.0, 12.0), 120));
    r.champions.throws.push(champions::Throw {
        owner: 1,
        kind: champions::Thrown::SpiritLance,
        x: 13.0,
        y: 14.0,
        vx: 28.0,
        vy: 0.0,
        from: (12.0, 14.0),
        to: (12.0, 14.0),
        age: 2,
        life: champions::SPIRIT_LIFE,
        caught: None,
        back: false,
    });
    r.players.get_mut(&1).unwrap().ult = Some(ults::Ult::PhantomLegion);
    let bytes = serde_json::to_vec(&r).unwrap();
    let back: Run = serde_json::from_slice(&bytes).expect("a run with the new state loads");
    assert_eq!(back.phantoms, r.phantoms);
    assert_eq!(back.champions.throws, r.champions.throws);
    assert_eq!(back.players[&1].champ, r.players[&1].champ);
    assert!(back.players[&1].champ.juxta > 0);
    assert_eq!(back.players[&1].ult, Some(ults::Ult::PhantomLegion));
    // And plays on the same.
    let (mut a, mut b) = (r, back);
    for _ in 0..30 {
        step(&mut a, Input { fire: true, ..Default::default() });
        step(&mut b, Input { fire: true, ..Default::default() });
    }
    assert_eq!(serde_json::to_string(&a.phantoms).unwrap(), serde_json::to_string(&b.phantoms).unwrap());
    assert_eq!(a.enemies[0].hp, b.enemies[0].hp);
}

#[test]
fn an_old_save_with_none_of_the_new_fields_still_loads() {
    // A phantom as Phantasm saved it, with no `fixed` and no `life`.
    let old = serde_json::json!({ "owner": 1, "x": 3.0, "y": 4.0, "dx": 2.6, "dy": 0.0, "left": 77 });
    let p: Phantom = serde_json::from_value(old).expect("an old image loads");
    assert_eq!((p.fixed, p.life, p.left), (false, 0, 77));
    assert_eq!(p.age(), ults::PHANTOM_TICKS - 77, "an old image ages by the old eight seconds");
    // And an image of the old kind writes itself the old way.
    let written = serde_json::to_value(p).unwrap();
    assert!(written.get("fixed").is_none() && written.get("life").is_none(), "{written}");
    // A stance as it was, with no `juxta`.
    let stance: champions::Stance = serde_json::from_value(serde_json::json!({ "brace": 4, "fury": 2 })).expect("an old stance loads");
    assert_eq!((stance.brace, stance.fury, stance.juxta), (4, 2, 0));
    assert!(serde_json::to_value(champions::Stance { brace: 1, ..Default::default() }).unwrap().get("juxta").is_none());
    // A whole run, written by this build and stripped of the new keys.
    let mut r = arena("lancer", 40.0, 14.0);
    r.phantoms.push(Phantom::beside(1, (12.0, 14.0), (0.0, 2.6), 100));
    let mut value = serde_json::to_value(&r).unwrap();
    for phantom in value["phantoms"].as_array_mut().unwrap() {
        phantom.as_object_mut().unwrap().remove("life");
        phantom.as_object_mut().unwrap().remove("fixed");
    }
    let back: Run = serde_json::from_value(value).expect("a run saved before the Phantom Lancer loads");
    assert_eq!(back.phantoms.len(), 1);
    assert!(!back.phantoms[0].fixed);
}

#[test]
fn the_phantom_lancer_mashes_every_key_through_a_fight_and_replays_exactly() {
    let play = || {
        let mut r = Run::new(23, 1, Some("Friend"));
        let fight = r.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && !r.great()).unwrap();
        r.enter_for_test(fight);
        assert!(r.take_up(1, "phantomlancer"));
        let mut salt = 11u64;
        let (mut most, mut made) = (0, 0);
        for t in 0..600u64 {
            salt = salt.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let roll = (salt >> 33) as u32;
            let axis = |k: u32| ((roll >> k) % 3) as i8 - 1;
            let input = Input {
                move_x: axis(0),
                move_y: axis(2),
                aim_x: axis(4),
                aim_y: axis(6),
                fire: roll % 3 != 0,
                dash: roll % 41 == 0,
                cast: ((roll >> 8) % 7) as u8 % 4,
                ult: roll % 53 == 0,
                ..Default::default()
            };
            if t % 120 == 0 {
                r.players.get_mut(&1).unwrap().ult_charge = ults::ULT_FULL;
            }
            r.step(&BTreeMap::from([(1, input)]));
            let h = &r.players[&1];
            assert!(h.x.is_finite() && h.y.is_finite());
            for p in &r.phantoms {
                assert!(p.x.is_finite() && p.y.is_finite());
            }
            most = most.max(r.phantoms.iter().filter(|p| p.owner == 1).count());
            made += r.phantoms.len();
        }
        assert!(most <= IMPOSTURE_MAX, "{most}");
        (serde_json::to_string(&r.phantoms).unwrap(), serde_json::to_string(&r.players[&1]).unwrap(), made)
    };
    let (phantoms, hero, made) = play();
    assert!(made > 0, "images were made");
    let (phantoms2, hero2, _) = play();
    assert_eq!(phantoms, phantoms2);
    assert_eq!(hero, hero2);
}
