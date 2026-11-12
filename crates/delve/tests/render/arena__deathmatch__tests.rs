use super::*;
fn scene() -> Run {
    let mut r = crate::rules::roundtable::fixture(&dm::ADVENTURE);
    for id in 2..=6 {
        r.join_seat(id, &format!("Knight {id}"));
    }
    // The round seats everyone the town door's room holds; no tick (the leg) is needed to draw it.
    r.deathmatch = Some(dm::Session::new(&r.players, &r.book));
    let s = r.deathmatch.as_mut().unwrap();
    s.ticks = dm::TITLE + 60;
    for (id, f) in &mut s.fighters {
        f.protection = 0;
        if *id == 1 {
            (f.hero.x, f.hero.y) = (13., 23.);
            f.yaw = 0.;
        }
    }
    let f = s.fighters.get_mut(&2).unwrap();
    (f.hero.x, f.hero.y) = (25., 23.);
    r
}
#[test]
fn each_seat_has_its_own_first_person_view_and_stage() {
    let r = scene();
    let a = layers(&r, Some(1)).0;
    let b = layers(&r, Some(6)).0;
    assert_ne!(a.content_hash(), b.content_hash());
    assert_eq!((a.w, a.h), (384, 224));
    let native = super::super::frame_for(&r, 384, 224, Some(1));
    assert_eq!(native, a);
    let parts = super::super::scene_for(&r, 384, 224, Some(1));
    assert!(
        parts.figures.is_empty(),
        "the stage carries its own billboards"
    );
}
#[test]
#[ignore = "writes PNG frames and reports the six-seat frame budget"]
fn write_deathmatch_shots() {
    let dir = &std::path::PathBuf::from(std::env::var_os("DELVE_DEATHMATCH_SHOTS").expect("set DELVE_DEATHMATCH_SHOTS to the output directory"));
    std::fs::create_dir_all(dir).unwrap();
    let save = |r: &Run, name: &str, seat| {
        let im = super::super::frame_for(r, 768, 448, Some(seat));
        image::save_buffer(
            dir.join(format!("{name}.png")),
            &im.rgba_bytes(),
            im.w as u32,
            im.h as u32,
            image::ColorType::Rgba8,
        )
        .unwrap();
    };
    let mut r = scene();
    save(&r, "hub", 1);
    save(&r, "corridor", 6);
    r.deathmatch.as_mut().unwrap().ticks = 1;
    save(&r, "title", 1);
    r.deathmatch.as_mut().unwrap().ticks = dm::TITLE + 1;
    r.deathmatch
        .as_mut()
        .unwrap()
        .fighters
        .get_mut(&1)
        .unwrap()
        .flash = 4;
    save(&r, "fire", 1);
    r.deathmatch
        .as_mut()
        .unwrap()
        .fighters
        .get_mut(&1)
        .unwrap()
        .respawn = dm::RESPAWN;
    r.deathmatch
        .as_mut()
        .unwrap()
        .fighters
        .get_mut(&1)
        .unwrap()
        .hero
        .hp = 0;
    save(&r, "respawn", 1);
    let s = r.deathmatch.as_mut().unwrap();
    s.fighters.get_mut(&2).unwrap().kills = 10;
    s.winner = Some(2);
    s.purse = 60;
    save(&r, "winner", 1);
    for kit in [
        "lynette",
        "gareth",
        "galahad",
        "hammerer",
        "butcher",
        "alchemist",
    ] {
        let mut r = scene();
        assert!(r.take_up(1, kit));
        let mut s = dm::Session::new(&r.players, &r.book);
        s.ticks = dm::TITLE + 1;
        let f = s.fighters.get_mut(&1).unwrap();
        (f.hero.x, f.hero.y) = (13., 23.);
        f.yaw = 0.;
        f.flash = 4;
        f.protection = 0;
        r.deathmatch = Some(s);
        save(&r, &format!("arm-{kit}"), 1);
    }
    let r = scene();
    let im = super::super::frame_for(&r, 768, 448, Some(1));
    let im = super::super::seat_lift_frame(&im, &r, Some(1));
    image::save_buffer(
        dir.join("browser-seat.png"),
        &im.rgba_bytes(),
        im.w as u32,
        im.h as u32,
        image::ColorType::Rgba8,
    )
    .unwrap();
    let start = std::time::Instant::now();
    for _ in 0..10 {
        for id in 1..=6 {
            std::hint::black_box(layers(&r, Some(id)));
        }
    }
    eprintln!(
        "six-seat render: {:.2} ms per seat",
        start.elapsed().as_secs_f64() * 1000. / 60.
    );
}

// --- the proof harness: a real scripted match -------------------------------
// `cargo test -p angel-delve --lib --features bend-engine --release? ...`
// drives the real `Run` (the Bend leg answers) with scripted seats, renders
// chosen windows to PNG frames and writes `events.txt`. Ignored: it writes files.

fn bot_inputs(r: &Run) -> std::collections::BTreeMap<u32, crate::rules::Input> {
    use crate::rules::Input;
    let Some(s) = r.deathmatch.as_ref() else {
        return Default::default();
    };
    let mut out = std::collections::BTreeMap::new();
    for (&id, f) in &s.fighters {
        let mut input = Input::default();
        if !f.alive() {
            out.insert(id, input);
            continue;
        }
        let me = (f.hero.x, f.hero.y);
        let rival = s
            .fighters
            .iter()
            .filter(|(o, g)| **o != id && g.alive() && dm::sight(me, (g.hero.x, g.hero.y)))
            .map(|(&o, g)| ((g.hero.x - me.0).hypot(g.hero.y - me.1), o, g))
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let wrap = |a: f32| a.sin().atan2(a.cos());
        match rival {
            Some((dist, _, g)) => {
                let want = (g.hero.y - me.1).atan2(g.hero.x - me.0);
                let err = wrap(want - f.yaw);
                input.aim_x = if err.abs() < 0.04 { 0 } else if err > 0. { 1 } else { -1 };
                input.move_y = if dist > 5. { -1 } else if dist < 2.5 { 1 } else { 0 };
                input.move_x = if (s.ticks / 25 + id) % 2 == 0 { 1 } else { -1 };
                if err.abs() < 0.35 {
                    input.fire = true;
                }
                if dist < 3.2 && err.abs() < 0.8 {
                    input.swing = true;
                }
            }
            None => {
                input.move_y = -1;
                input.aim_x = if (s.ticks / 40 + id) % 3 == 0 { 1 } else if (s.ticks / 40 + id) % 3 == 1 { -1 } else { 0 };
            }
        }
        out.insert(id, input);
    }
    out
}

/// The real leg: the shipped Bend generation answers `deathmatch-rules`.
fn real_leg() {
    #[cfg(feature = "bend-engine")]
    crate::scripted::install_for_test("deathmatch-rules", angel_live::shipped::engine);
}

fn bot_match(seats: u32) -> Run {
    real_leg();
    let mut r = crate::rules::roundtable::fixture(&dm::ADVENTURE);
    for id in 2..=seats {
        r.join_seat(id, &format!("Knight {id}"));
    }
    let kits = ["lynette", "gareth", "galahad", "hammerer", "butcher", "alchemist"];
    for (i, kit) in kits.iter().enumerate().take(seats as usize) {
        r.take_up(i as u32 + 1, kit);
    }
    // A pot to play for: a tenth of each knight's gold goes in.
    for (id, h) in r.players.iter_mut() {
        h.name = format!("Knight {id}");
        h.carried.add(crate::realm::Spoil::Gold, 500);
    }
    r.start_deathmatch();
    r
}

/// Plays a real six-seat bot match through the Bend leg and writes what the
/// focus seat sees, one PNG per transport tick (holds included), `events.txt`
/// (kills, holds, calls by transport tick), and with `DM_MONTAGE=a-b` a 3x2
/// sheet of every seat for that range of ticks. `DM_SCOUT=1` plays without
/// drawing, to find the winner and the windows first. `DM_RANGE=a-b` limits
/// the focus frames to that range of transport ticks.
#[test]
#[ignore = "drives a real match and writes PNG frames: set DELVE_DEATHMATCH_MATCH to the output directory"]
fn write_deathmatch_match() {
    let dir = std::path::PathBuf::from(std::env::var_os("DELVE_DEATHMATCH_MATCH").expect("set DELVE_DEATHMATCH_MATCH"));
    std::fs::create_dir_all(&dir).unwrap();
    let seats: u32 = std::env::var("DM_SEATS").ok().and_then(|s| s.parse().ok()).unwrap_or(6);
    let focus: u32 = std::env::var("DM_FOCUS").ok().and_then(|s| s.parse().ok()).unwrap_or(1);
    let scout = std::env::var("DM_SCOUT").is_ok();
    let range = |name: &str| -> Option<(u32, u32)> {
        let v = std::env::var(name).ok()?;
        let (a, b) = v.split_once('-')?;
        Some((a.parse().ok()?, b.parse().ok()?))
    };
    let (frames_in, montage_in) = (range("DM_RANGE"), range("DM_MONTAGE"));
    let save = |im: &crate::render::overworld::ink::Img, name: String| {
        image::save_buffer(dir.join(name), &im.rgba_bytes(), im.w as u32, im.h as u32, image::ColorType::Rgba8).unwrap();
    };
    let mut r = bot_match(seats);
    let mut log = String::new();
    let (mut frame, mut sheet_n) = (0u32, 0u32);
    let mut last_kills = 0u32;
    let mut seen_calls = 0usize;
    let mut won_at: Option<u32> = None;
    let mut times = Vec::new();
    for transport in 0u32..6000 {
        let inputs = bot_inputs(&r);
        r.step(&inputs);
        let Some(s) = r.deathmatch.as_ref() else { break };
        let kills: u32 = s.fighters.values().map(|f| u32::from(f.kills)).sum();
        if kills != last_kills {
            log.push_str(&format!("t{transport} sim{} kills {last_kills}->{kills} winner {:?} scores {:?}\n", s.ticks, s.winner, s.fighters.iter().map(|(i, f)| (*i, f.kills)).collect::<Vec<_>>()));
            last_kills = kills;
        }
        if r.hitstop.left > 0 {
            log.push_str(&format!("t{transport} sim{} HOLD left {}\n", s.ticks, r.hitstop.left));
        }
        let calls: Vec<_> = s.show.calls.iter().filter(|c| c.born == s.ticks).collect();
        if !calls.is_empty() && s.show.calls.len() != seen_calls {
            log.push_str(&format!("t{transport} sim{} CALLS {:?}\n", s.ticks, calls));
        }
        seen_calls = s.show.calls.len();
        if s.winner.is_some() && won_at.is_none() {
            won_at = Some(transport);
            log.push_str(&format!("t{transport} WON by {:?} ended {}\n", s.winner, s.ended));
        }
        if !scout {
            if frames_in.is_none_or(|(a, b)| (a..=b).contains(&transport)) {
                let start = std::time::Instant::now();
                let im = super::super::frame_for(&r, 384, 224, Some(focus));
                times.push(start.elapsed().as_secs_f64() * 1000.);
                save(&im, format!("f{frame:05}.png"));
                frame += 1;
            }
            if montage_in.is_some_and(|(a, b)| (a..=b).contains(&transport)) {
                let mut sheet = crate::render::overworld::ink::Img::black(1152, 448);
                for seat in 1..=seats {
                    let im = super::super::frame_for(&r, 384, 224, Some(seat));
                    sheet.stamp(&im, ((seat - 1) % 3) as i32 * 384, ((seat - 1) / 3) as i32 * 224);
                }
                save(&sheet, format!("m{sheet_n:05}.png"));
                sheet_n += 1;
            }
        }
        if won_at.is_some_and(|w| transport > w + 6 * 30 + 12) {
            break;
        }
    }
    std::fs::write(dir.join("events.txt"), &log).unwrap();
    times.sort_by(f64::total_cmp);
    let med = times.get(times.len() / 2).copied().unwrap_or(0.);
    eprintln!("frames {frame}, montage {sheet_n}, median render {med:.2} ms, winner at {won_at:?}");
}

// --- the juice, drawn ----------------------------------------------------------------

fn struck_scene() -> Run {
    use crate::rules::juice::{Blow, Weight};
    let mut r = scene();
    let s = r.deathmatch.as_mut().unwrap();
    s.ticks = dm::TITLE + 200;
    let f2 = s.fighters.get_mut(&2).unwrap();
    f2.hero.hp = 20;
    let ticks = s.ticks;
    s.show.blow(Blow { owner: 1, target: 2, x: 25., y: 23., from: (13., 23.), lost: 70, weight: Weight::Crushing, born: ticks });
    s.show.blow(Blow { owner: 2, target: 1, x: 13., y: 23., from: (25., 23.), lost: 30, weight: Weight::Solid, born: ticks });
    let scores: Vec<(u32, u16)> = s.fighters.iter().map(|(&id, f)| (id, f.kills)).collect();
    s.show.kill(ticks, 1, 3, &scores, dm::GOAL);
    r
}
#[test]
fn blows_and_calls_change_what_each_seat_sees() {
    let calm = layers(&scene(), Some(1)).0.content_hash();
    let hit = struck_scene();
    let a = layers(&hit, Some(1)).0;
    let b = layers(&hit, Some(2)).0;
    let c = layers(&hit, Some(6)).0;
    assert_ne!(a.content_hash(), calm, "the striker sees the blow");
    assert_ne!(a.content_hash(), b.content_hash());
    assert_ne!(b.content_hash(), c.content_hash(), "the struck seat's view differs from a bystander's");
    // A bystander still sees the call banner; every frame is the screen's size.
    assert!([&a, &b, &c].iter().all(|im| (im.w, im.h) == (384, 224)));
}
#[test]
fn the_title_the_countdown_the_finish_and_the_podium_each_look_their_own_way() {
    let mut r = scene();
    let mut seen = std::collections::BTreeSet::new();
    for t in [1, 12, 24, 50, 80, dm::TITLE + 3, dm::TITLE + 20] {
        r.deathmatch.as_mut().unwrap().ticks = t;
        assert!(seen.insert(layers(&r, Some(1)).0.content_hash()), "tick {t} looks like another");
    }
    let s = r.deathmatch.as_mut().unwrap();
    s.ticks = dm::TITLE + 300;
    s.fighters.get_mut(&2).unwrap().kills = dm::GOAL as u16;
    s.winner = Some(2);
    s.purse = 120;
    for ended in [0, 20, 40, 60, 70, 90, 150, dm::PODIUM] {
        r.deathmatch.as_mut().unwrap().ended = ended;
        assert!(seen.insert(layers(&r, Some(1)).0.content_hash()) || ended == 0, "ended {ended} looks like another");
        let im = layers(&r, Some(3)).0;
        assert_eq!((im.w, im.h), (384, 224));
    }
}
#[test]
fn the_words_on_the_screen_never_overlap_each_other() {
    use crate::render::overworld::ink::said;
    let mut stages: Vec<(&str, Run)> = vec![("brawl", struck_scene())];
    let mut slain = struck_scene();
    {
        let s = slain.deathmatch.as_mut().unwrap();
        let f = s.fighters.get_mut(&1).unwrap();
        (f.hero.hp, f.respawn) = (0, dm::RESPAWN - 20);
        s.feed.push(dm::Frag { killer: 2, victim: 1, tick: s.ticks });
    }
    stages.push(("slain", slain));
    let mut title = scene();
    title.deathmatch.as_mut().unwrap().ticks = 40;
    stages.push(("title", title));
    for (name, r) in stages {
        for seat in 1..=6 {
            let (_, said) = said::read(|| layers(&r, Some(seat)));
            let rects: Vec<_> = said
                .lines
                .iter()
                .filter(|l| l.on == (384, 224))
                .map(|l| (l.at.0, l.at.1, l.at.0 + crate::render::overworld::ink::text_width(&l.text), l.at.1 + 7, l.text.clone()))
                .collect();
            for (i, a) in rects.iter().enumerate() {
                for b in &rects[i + 1..] {
                    let overlap = a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3;
                    assert!(!overlap, "{name} seat {seat}: {:?} collides with {:?}", a.4, b.4);
                }
            }
        }
    }
}
#[test]
fn the_juice_state_rides_the_wire_and_validates() {
    let r = struck_scene();
    let s = r.deathmatch.as_ref().unwrap();
    assert!(s.valid());
    let back: dm::Session = serde_json::from_slice(&serde_json::to_vec(s).unwrap()).unwrap();
    assert_eq!(serde_json::to_vec(&back).unwrap(), serde_json::to_vec(s).unwrap());
    // A snapshot from before the kit existed still loads, with a quiet book.
    let mut old: serde_json::Value = serde_json::to_value(scene().deathmatch.as_ref().unwrap()).unwrap();
    old.as_object_mut().unwrap().remove("show");
    let loaded: dm::Session = serde_json::from_value(old).unwrap();
    assert!(loaded.show.quiet());
}

/// Film only (Knights Wish film, 2026-11-04): the round a tick a frame from
/// seat 1's eyes, everyone moving and firing.
/// `KW_DM_FILM=<dir> cargo test kw_deathmatch_film -- --ignored`
#[test]
#[ignore = "film export"]
fn kw_deathmatch_film() {
    let dir = &std::path::PathBuf::from(std::env::var_os("KW_DM_FILM").expect("set KW_DM_FILM"));
    std::fs::create_dir_all(dir).unwrap();
    // The round's ticks are the Bend leg's: install it as the proof harness does.
    real_leg();
    let mut r = scene();
    r.deathmatch.as_mut().unwrap().ticks = dm::TITLE - 40;
    let mut seed = 0x2545_f491_u64;
    let mut rnd = move || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; seed };
    let mut plans: std::collections::BTreeMap<u32, (i8, i8, i8, u32)> = Default::default();
    for t in 0..330u32 {
        let mut inputs = std::collections::BTreeMap::new();
        for id in 1..=6u32 {
            let p = plans.entry(id).or_insert((0, 0, 0, 0));
            if p.3 == 0 { *p = ((rnd() % 3) as i8 - 1, -1, (rnd() % 3) as i8 - 1, 20 + (rnd() % 30) as u32); }
            p.3 -= 1;
            let mut i = crate::rules::Input { move_x: p.0, move_y: p.1, aim_x: p.2, fire: rnd() % 4 != 0, ..Default::default() };
            if id == 1 {
                // seat 1 sweeps slowly right, then left, walking on, firing in bursts
                i.move_x = 0; i.move_y = -1; i.aim_x = if (t / 70) % 2 == 0 { 1 } else { -1 }; i.fire = (t / 9) % 3 != 2;
            }
            inputs.insert(id, i);
        }
        r.step(&inputs);
        let im = super::super::frame_for(&r, 768, 448, Some(1));
        image::save_buffer(dir.join(format!("{t:05}.png")), &im.rgba_bytes(), im.w as u32, im.h as u32, image::ColorType::Rgba8).unwrap();
    }
}
