use super::*;
use crate::rules::creation::{Role, Step};
use crate::rules::rite::{self, Made};

#[cfg(not(feature = "creator-dev"))]
#[test]
fn the_dev_chord_does_not_replace_controls_in_ordinary_builds() {
    let mut host = Host::new(fight());
    let mut c = client(Some(0));
    host.send(&mut c);
    let keys = Hands {
        held: button::MAP | button::X12,
        pressed: button::X12,
        ..Default::default()
    };
    assert!(c.command(&keys).input().cycle);
    assert!(!c.creation.session.open);
}

#[test]
fn creator_dev_entry_consumes_all_gameplay_and_does_not_touch_the_run() {
    let mut host = Host::new(fight());
    let mut c = client(Some(0));
    host.send(&mut c);
    let original = serde_json::to_value(c.newest().unwrap()).unwrap();
    #[cfg(feature = "creator-dev")]
    let keys = Hands {
        held: button::MAP | button::X12,
        pressed: button::X12,
        ..Default::default()
    };
    #[cfg(feature = "creator-dev")]
    assert_eq!(c.command(&keys), DelveCmd::neutral());
    #[cfg(not(feature = "creator-dev"))]
    assert_eq!(c.set_knob("creator_open", 1.0), Some(1.0));
    assert!(c.creation.session.open);
    c.menu = true;
    c.ballot.open = true;
    c.picked = Some(1);
    let all = Hands {
        held: u32::MAX,
        pressed: button::DOWN,
        x: 127,
        aim_x: 127,
        ..Default::default()
    };
    assert_eq!(c.command(&all), DelveCmd::neutral());
    assert_eq!(c.creation.session.creation.role, Role::Minion);
    assert!(!c.menu && !c.ballot.open && c.picked.is_none());
    assert!(c.walk(&all).is_none() && c.coming(&all).is_none());
    let v = view(&c, &[], 0, all, 0.5);
    let hero = c.own(&v).unwrap();
    assert_eq!(
        (hero.x, hero.y),
        (host.run.players[&1].x, host.run.players[&1].y)
    );
    assert_eq!(serde_json::to_value(c.newest().unwrap()).unwrap(), original);
    assert!(c.choices().is_empty());
    // No key that closes the scroll leaks through to a room's pause menu.
    assert_eq!(c.command(&tap(button::MENU)), DelveCmd::neutral());
    assert!(!c.creation.session.open && !c.menu);
    assert_ne!(c.command(&held(button::RIGHT)), DelveCmd::neutral());
}

#[test]
fn creator_actions_take_real_presses_and_throttle_held_sticks() {
    let mut editor = creation::Editor::default();
    editor.session.open(0, "Ada");
    editor.session.go(Step::Body, 0);
    let right = Hands {
        x: 127,
        ..Default::default()
    };
    for tick in 0..5 {
        editor.hands(&right, tick);
    }
    assert_eq!(editor.session.creation.sculpt.height, 55);
    editor.hands(&right, 5);
    assert_eq!(editor.session.creation.sculpt.height, 60);
    editor.hands(&held(button::USE), 6);
    assert_eq!(
        editor.session.step,
        Step::Body,
        "holding confirm must not skip steps"
    );
    editor.hands(&tap(button::USE), 7);
    assert_eq!(editor.session.step, Step::Parts);
    editor.hands(&tap(button::ALT), 8);
    assert_eq!(editor.session.step, Step::Body);
}

#[test]
fn creator_is_opt_in_and_observers_cannot_open_it() {
    let mut host = Host::new(fight());
    let mut c = client(Some(0));
    host.send(&mut c);
    assert!(!c.creation.session.open);
    let command = c.command(&tap(button::X11));
    assert!(command.input().ride, "ordinary C keeps its ride action");
    assert!(!c.creation.session.open);
    assert_eq!(c.set_knob("creator_open", 1.0), Some(1.0));
    assert_eq!(c.set_knob("creator_open", 0.0), Some(0.0));
    let mut observer = client(None);
    host.send(&mut observer);
    assert_eq!(observer.set_knob("creator_open", 1.0), Some(0.0));
}

#[test]
fn creator_draws_in_both_flat_frames_and_the_glass_screen_layer() {
    use crate::render::overworld::ink::{Img, said};
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            arena::draw_parts_outside(self.0);
        }
    }
    let _restore = Restore(arena::parts_outside());
    let mut host = Host::new(fight());
    let mut c = client(Some(0));
    c.exact = true;
    host.send(&mut c);
    c.set_knob("creator_open", 1.0);
    // Settle the opening without advancing the fight or requiring a window.
    c.creation.session.opened = 0;
    for kept in &mut c.kept {
        kept.run.tick = 24;
    }
    let mut frame = Frame::default();
    for parts in [false, true] {
        arena::draw_parts_outside(parts);
        let (_, text) = said::read(|| c.draw(&view(&c, &[], 0, Hands::default(), 0.0), &mut frame));
        assert!(text.lines.iter().any(|l| l.text == "THE CREATOR"));
        let image = if parts {
            c.scene().unwrap().screen.clone()
        } else {
            Img::from_words(frame.w as i32, frame.h as i32, frame.px.clone())
        };
        assert_eq!(image.w, 384);
        assert!(image.h > 214, "the real HUD bands are present");
        assert_eq!(
            image.get(240, 100),
            Some([0, 0, 0]),
            "the stage covers underlying world geometry"
        );
    }
}

/// The Test drive end to end, on scripted hands: the Creator's grant, then the walk, the strikes and
/// a meeting on the fixture floor. Entirely offline; writes `target/creator-shots/slice2/test-drive-384/*.png`
/// (frames at 12 per second of game time) and a 768-wide set; the packager makes the looping GIFs.
#[test]
#[ignore]
fn write_test_drive_frames() {
    use crate::render::overworld::arena;
    use crate::rules::creation::Step;
    let base = std::env::var_os("ANGEL_CREATOR_SHOTS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/creator-shots"))
        .join("slice2");
    for dir in ["test-drive-384", "test-drive-768"] {
        let _ = std::fs::remove_dir_all(base.join(dir));
    }
    let mut run = crate::rules::Run::new(41, 3, Some("Ada"));
    let mut editor = creation::Editor::default();
    editor.session.open(0, "Ada");
    editor.session.go(Step::Life, 0);
    editor.hands(&tap(button::USE), 1);
    assert!(editor.session.granted.is_some(), "the genie grants it");
    editor.hands(&tap(button::USE), 70);
    assert_eq!(editor.session.step, Step::Drive, "E after the grant opens the Test drive");
    let mut tick = 71u64;
    let mut n = 0usize;
    let mut next_frame = tick as f64;
    let mut frames = |editor: &mut creation::Editor, tick: u64, n: &mut usize, next: &mut f64, run: &mut crate::rules::Run| {
        if (tick as f64) < *next {
            return;
        }
        *next += 2.5;
        run.tick = tick;
        for (w, h, dir) in [(384, 286, "test-drive-384"), (768, 572, "test-drive-768")] {
            let img = arena::in_seat(|| {
                let mut room = arena::frame_for(run, w, arena::view_rows(h), Some(1));
                crate::render::creation::paint_with(&mut room, &editor.session, tick, None, editor.floor.as_ref());
                arena::seat_lift_frame_with(&room, run, Some(1), &arena::MapView::default())
            });
            let path = base.join(dir).join(format!("{:03}.png", *n));
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            image::save_buffer(path, &img.rgb_bytes(), img.w as u32, img.h as u32, image::ColorType::Rgb8).unwrap();
        }
        *n += 1;
    };
    // One tick of hands, recorded.
    macro_rules! tick_with {
        ($hands:expr) => {{
            editor.hands(&$hands, tick);
            frames(&mut editor, tick, &mut n, &mut next_frame, &mut run);
            tick += 1;
        }};
    }
    // Waypoints over the floor, breadth-first on a half-unit grid with the drive's own feet box,
    // so the scripted creature walks round the rocks as a player would.
    let route = |editor: &creation::Editor, to: (f32, f32)| -> Vec<(f32, f32)> {
        let floor = editor.floor.as_ref().unwrap();
        let from = editor.session.drive.as_ref().map_or(floor.spawn, |d| (d.x, d.y));
        let fits = |x: f32, y: f32| [(-1.6f32, 0.0f32), (1.6, 0.0), (0.0, 0.0), (0.0, -1.0)].iter().all(|(ox, oy)| floor.walkable(x + ox, y + oy));
        let cell = 0.5f32;
        let (cols, rows) = (100usize, 60usize);
        let at = |p: (f32, f32)| (((p.0 / cell) as usize).min(cols - 1), ((p.1 / cell) as usize).min(rows - 1));
        let centre = |c: (usize, usize)| ((c.0 as f32 + 0.5) * cell, (c.1 as f32 + 0.5) * cell);
        let mut prev = vec![None::<(usize, usize)>; cols * rows];
        let start = at(from);
        let mut seen = vec![false; cols * rows];
        seen[start.1 * cols + start.0] = true;
        let mut queue = std::collections::VecDeque::from([start]);
        let mut goal = None;
        while let Some(c) = queue.pop_front() {
            let p = centre(c);
            if (p.0 - to.0).hypot(p.1 - to.1) < 0.6 {
                goal = Some(c);
                break;
            }
            for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
                let (nx, ny) = (c.0 as i32 + dx, c.1 as i32 + dy);
                if nx < 0 || ny < 0 || nx >= cols as i32 || ny >= rows as i32 {
                    continue;
                }
                let n = (nx as usize, ny as usize);
                let q = centre(n);
                if !seen[n.1 * cols + n.0] && fits(q.0, q.1) {
                    seen[n.1 * cols + n.0] = true;
                    prev[n.1 * cols + n.0] = Some(c);
                    queue.push_back(n);
                }
            }
        }
        let mut path = Vec::new();
        let mut cur = goal;
        while let Some(c) = cur {
            path.push(centre(c));
            cur = prev[c.1 * cols + c.0];
        }
        path.reverse();
        path
    };
    let steer = |editor: &creation::Editor, to: (f32, f32), near: f32| -> Option<Hands> {
        let d = editor.session.drive.as_ref()?;
        let (dx, dy) = (to.0 - d.x, to.1 - d.y);
        if dx.hypot(dy) < near {
            return None;
        }
        let axis = |v: f32| if v.abs() < 0.14 { 0 } else { (v.signum() * 127.0) as i8 };
        Some(Hands { x: axis(dx), y: axis(dy), ..Default::default() })
    };
    let floor = |e: &creation::Editor| {
        let f = e.floor.as_ref().expect("the fixture floor stands once the drive begins");
        (f.spawn, f.friend, f.foes.clone())
    };
    // The floor is built by the first drive tick.
    for _ in 0..14 {
        tick_with!(Hands::default());
    }
    let (spawn, friend, foes) = floor(&editor);
    let _ = (spawn, friend, &foes);
    // Walk to the comparison knight and meet him.
    let go = |editor: &mut creation::Editor, to: (f32, f32), tick: &mut u64, n: &mut usize, next: &mut f64, run: &mut crate::rules::Run, frames: &mut dyn FnMut(&mut creation::Editor, u64, &mut usize, &mut f64, &mut crate::rules::Run)| {
        let way = route(editor, to);
        assert!(!way.is_empty(), "a way to {to:?} over the fixture floor");
        for (i, w) in way.iter().enumerate().skip(1) {
            // Every second cell is a waypoint, and the last one is the goal.
            if i % 2 == 1 && i + 1 < way.len() {
                continue;
            }
            for _ in 0..60 {
                let Some(h) = steer(editor, *w, 0.3) else { break };
                editor.hands(&h, *tick);
                frames(editor, *tick, n, next, run);
                *tick += 1;
            }
        }
    };
    let to = editor.floor.as_ref().unwrap().standing((friend.0 + 7.0, friend.1 + 2.0));
    go(&mut editor, to, &mut tick, &mut n, &mut next_frame, &mut run, &mut frames);
    tick_with!(Hands { pressed: button::USE, ..Default::default() });
    for _ in 0..50 {
        tick_with!(Hands::default());
    }
    // Then to the first foe, and two blows.
    let to = editor.floor.as_ref().unwrap().standing((foes[0].0 - 5.0, foes[0].1 + 1.0));
    go(&mut editor, to, &mut tick, &mut n, &mut next_frame, &mut run, &mut frames);
    for _ in 0..2 {
        tick_with!(Hands { pressed: button::FIRE, ..Default::default() });
        for _ in 0..26 {
            tick_with!(Hands::default());
        }
    }
    let to = editor.floor.as_ref().unwrap().standing((foes[1].0 - 5.0, foes[1].1 + 1.0));
    go(&mut editor, to, &mut tick, &mut n, &mut next_frame, &mut run, &mut frames);
    tick_with!(Hands { pressed: button::FIRE, ..Default::default() });
    for _ in 0..40 {
        tick_with!(Hands::default());
    }
    let d = editor.session.drive.as_ref().unwrap();
    eprintln!("drive ended at ({:.1},{:.1}) after {} frames; strikes {}", d.x, d.y, n, d.strikes);
    assert!(d.strikes >= 1, "a blow reached a foe");
}

#[test]
fn the_wish_line_is_kept_by_the_open_creator_and_never_reaches_the_host() {
    let mut host = Host::new(fight());
    let mut c = client(Some(0));
    host.send(&mut c);
    let words = br#"{"words":"a flaming bone axe with a skull pommel"}"#;
    let original = serde_json::to_value(c.newest().unwrap()).unwrap();
    // Closed: the Creator keeps nothing, so a wish goes to the host as it always did.
    assert!(!c.keep_action(2, words));
    assert!(!c.keep_action(1, br#"{"wish":"x"}"#));
    assert_eq!(c.controls(), CONTROLS);
    c.set_knob("creator_open", 1.0);
    assert!(c.creation.session.open);
    // Open: the shell's text line is the Creator's Wish line, titled so, and its words are kept.
    assert!(c.controls().contains(r#""title":"Wish it""#) && c.controls().contains("skull pommel"));
    assert!(c.keep_action(2, words), "the words were taken here, so nothing is sent");
    let s = &c.creation.session;
    assert!(s.forge() && s.weapon.kind == crate::rules::creation::WeaponKind::Haft);
    assert_eq!(s.chat.lines.len(), 2);
    assert_eq!(s.chat.lines[0].text, "a flaming bone axe with a skull pommel");
    // A pick (kind 1) and unreadable words are swallowed too, changing nothing; other kinds pass.
    let before = c.creation.session.clone();
    assert!(c.keep_action(1, br#"{"wish":"x"}"#) && c.keep_action(2, b"not json"));
    assert_eq!(c.creation.session, before);
    assert!(!c.keep_action(9, br#"{"from":0,"to":1}"#), "a drag in the hand is the host's");
    assert!(!c.keep_action(0x100, b""), "the engine's own kinds are never kept");
    // The run is untouched by all of it.
    assert_eq!(serde_json::to_value(c.newest().unwrap()).unwrap(), original);
}

/// Slice 3: the Hero's Test drive end to end on scripted hands: the grant, the walks in all four
/// directions, two smashes beside a foe and a meeting. Offline; writes
/// `target/creator-shots/slice3/test-drive-384/*.png` (12 frames per second of game time) and a
/// 768-wide set; the packager makes the looping GIFs.
#[test]
#[ignore]
fn write_slice3_drive_frames() {
    use crate::render::overworld::arena;
    use crate::rules::creation::Step;
    let base = std::env::var_os("ANGEL_CREATOR_SHOTS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/creator-shots"))
        .join("slice3");
    for dir in ["test-drive-384", "test-drive-768"] {
        let _ = std::fs::remove_dir_all(base.join(dir));
    }
    let mut run = crate::rules::Run::new(41, 3, Some("Ada"));
    let mut editor = creation::Editor::default();
    editor.session.open(0, "Ada");
    editor.session.go(Step::Life, 0);
    editor.hands(&tap(button::USE), 1);
    assert!(editor.session.granted.is_some(), "the genie grants it");
    editor.hands(&tap(button::USE), 70);
    assert_eq!(editor.session.step, Step::Drive, "E after the grant opens the Test drive");
    let mut tick = 71u64;
    let mut n = 0usize;
    let mut next = tick as f64;
    macro_rules! step {
        ($hands:expr) => {{
            editor.hands(&$hands, tick);
            if (tick as f64) >= next {
                next += 2.5;
                run.tick = tick;
                for (w, h, dir) in [(384, 286, "test-drive-384"), (768, 572, "test-drive-768")] {
                    let img = arena::in_seat(|| {
                        let mut room = arena::frame_for(&run, w, arena::view_rows(h), Some(1));
                        crate::render::creation::paint_with(&mut room, &editor.session, tick, None, editor.floor.as_ref());
                        arena::seat_lift_frame_with(&room, &run, Some(1), &arena::MapView::default())
                    });
                    let path = base.join(dir).join(format!("{n:03}.png"));
                    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                    image::save_buffer(path, &img.rgb_bytes(), img.w as u32, img.h as u32, image::ColorType::Rgb8).unwrap();
                }
                n += 1;
            }
            tick += 1;
        }};
    }
    macro_rules! put_at {
        ($at:expr) => {{
            let at = editor.floor.as_ref().unwrap().standing($at);
            let d = editor.session.drive.as_mut().unwrap();
            (d.x, d.y) = at;
        }};
    }
    // The floor is built by the first drive tick; then the creature stands and breathes.
    for _ in 0..14 {
        step!(Hands::default());
    }
    let (friend, foes) = {
        let f = editor.floor.as_ref().expect("the fixture floor stands once the drive begins");
        (f.friend, f.foes.clone())
    };
    for _ in 0..40 {
        step!(Hands::default());
    }
    // The four walks, each from open floor with ground to cover.
    for (label, hands, from, ticks) in [
        ("east", Hands { x: 127, ..Default::default() }, (13.0f32, 24.0f32), 34),
        ("west", Hands { x: -127, ..Default::default() }, (34.0, 24.0), 34),
        ("south", Hands { y: 127, ..Default::default() }, (22.0, 14.0), 28),
        ("north", Hands { y: -127, ..Default::default() }, (22.0, 24.0), 28),
    ] {
        put_at!(from);
        for _ in 0..6 {
            step!(Hands::default());
        }
        let start = editor.session.drive.as_ref().map(|d| (d.x, d.y)).unwrap();
        for _ in 0..ticks {
            step!(hands);
        }
        let d = editor.session.drive.as_ref().unwrap();
        eprintln!("walk {label}: ({:.1},{:.1}) to ({:.1},{:.1})", start.0, start.1, d.x, d.y);
        assert!((d.x - start.0).hypot(d.y - start.1) > 4.0, "the creature walked {label}");
    }
    // Two smashes beside the first foe.
    put_at!((foes[0].0 - 4.0, foes[0].1 + 1.0));
    for _ in 0..6 {
        step!(Hands::default());
    }
    for _ in 0..2 {
        step!(Hands { pressed: button::FIRE, ..Default::default() });
        for _ in 0..28 {
            step!(Hands::default());
        }
    }
    // A meeting with the comparison knight, then the last breaths.
    put_at!((friend.0 + 7.0, friend.1 + 2.0));
    for _ in 0..4 {
        step!(Hands::default());
    }
    step!(Hands { pressed: button::USE, ..Default::default() });
    for _ in 0..60 {
        step!(Hands::default());
    }
    let d = editor.session.drive.as_ref().unwrap();
    eprintln!("drive ended at ({:.1},{:.1}) after {n} frames; strikes {}", d.x, d.y, d.strikes);
    assert!(d.strikes >= 1, "a blow reached a foe");
}

#[test]
fn the_test_drive_flips_a_heros_looks_with_the_number_keys_and_keeps_walking() {
    let mut editor = creation::Editor::default();
    editor.session.open(0, "Ada");
    editor.session.go(Step::Drive, 0);
    assert!(editor.session.creation.look.is_none());
    editor.hands(&tap(button::SLOT2), 1);
    assert_eq!(editor.session.creation.look.as_deref(), Some("a6-crusader"));
    editor.hands(&tap(button::SLOT3), 2);
    assert_eq!(editor.session.creation.look.as_deref(), Some("frog-knight"));
    // A look flips in the same tick the creature walks: the keys do not swallow one another.
    let before = editor.session.drive.as_ref().map(|d| d.x);
    let mut both = Hands { x: 127, ..Default::default() };
    both.pressed = button::SLOT1;
    editor.hands(&both, 3);
    assert!(editor.session.creation.look.is_none(), "the first look is the default");
    let after = editor.session.drive.as_ref().map(|d| d.x);
    assert!(after > before, "and the creature walked on ({before:?} to {after:?})");
    editor.hands(&tap(button::SLOT4), 4);
    assert!(editor.session.creation.look.is_none(), "there is no fourth look");
    // A role with no looks ignores them.
    editor.session.creation.role = Role::Monster;
    editor.hands(&tap(button::SLOT2), 5);
    assert!(editor.session.creation.look.is_none());
    // The builder pages keep the number keys for their rows.
    let mut pages = creation::Editor::default();
    pages.session.open(0, "Ada");
    pages.session.go(Step::Paint, 0);
    pages.hands(&tap(button::SLOT3), 1);
    assert_eq!((pages.session.row, pages.session.creation.look.is_none()), (2, true));
}

/// Slice 4: each of the Hero's three looks through the same Test drive tour (the grant, four walks, two
/// smashes beside a foe, a meeting), and one drive in which the number keys flip the looks while the
/// knight walks and strikes. Offline; writes `target/creator-shots/slice4/drive-<look>-384/*.png` (12 frames
/// per second of game time) and a 768-wide set; the packager makes the looping GIFs.
#[test]
#[ignore]
fn write_slice4_drive_frames() {
    use crate::render::overworld::arena;
    use crate::rules::creation::Step;
    let base = std::env::var_os("ANGEL_CREATOR_SHOTS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/creator-shots"))
        .join("slice4");
    // (tag, the look's index, whether the keys flip it during the drive)
    for (tag, look, flip) in [("2d", 0usize, false), ("3d", 1, false), ("kiln", 2, false), ("flip", 0, true)] {
        let names = [format!("drive-{tag}-384"), format!("drive-{tag}-768")];
        for dir in &names {
            let _ = std::fs::remove_dir_all(base.join(dir));
        }
        let mut run = crate::rules::Run::new(41, 3, Some("Ada"));
        let mut editor = creation::Editor::default();
        editor.session.open(0, "Ada");
        editor.session.creation.set_look(look);
        editor.session.go(Step::Life, 0);
        editor.hands(&tap(button::USE), 1);
        assert!(editor.session.granted.is_some(), "the genie grants it");
        editor.hands(&tap(button::USE), 70);
        assert_eq!(editor.session.step, Step::Drive, "E after the grant opens the Test drive");
        let mut tick = 71u64;
        let mut n = 0usize;
        let mut next = tick as f64;
        macro_rules! step {
            ($hands:expr) => {{
                editor.hands(&$hands, tick);
                if (tick as f64) >= next {
                    next += 2.5;
                    run.tick = tick;
                    for (w, h, dir) in [(384, 286, &names[0]), (768, 572, &names[1])] {
                        let img = arena::in_seat(|| {
                            let mut room = arena::frame_for(&run, w, arena::view_rows(h), Some(1));
                            crate::render::creation::paint_with(&mut room, &editor.session, tick, None, editor.floor.as_ref());
                            arena::seat_lift_frame_with(&room, &run, Some(1), &arena::MapView::default())
                        });
                        let path = base.join(dir).join(format!("{n:03}.png"));
                        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                        image::save_buffer(path, &img.rgb_bytes(), img.w as u32, img.h as u32, image::ColorType::Rgb8).unwrap();
                    }
                    n += 1;
                }
                tick += 1;
            }};
        }
        macro_rules! put_at {
            ($at:expr) => {{
                let at = editor.floor.as_ref().unwrap().standing($at);
                let d = editor.session.drive.as_mut().unwrap();
                (d.x, d.y) = at;
            }};
        }
        for _ in 0..14 {
            step!(Hands::default());
        }
        let (friend, foes) = {
            let f = editor.floor.as_ref().expect("the fixture floor stands once the drive begins");
            (f.friend, f.foes.clone())
        };
        let key = |n: usize| Hands { pressed: [button::SLOT1, button::SLOT2, button::SLOT3][n], ..Default::default() };
        if flip {
            // One drive, three looks: walk east, flip, walk on, flip, strike, flip back and strike again.
            put_at!((13.0f32, 24.0f32));
            for _ in 0..6 {
                step!(Hands::default());
            }
            for (i, flip_to) in [None, Some(1usize), None, Some(2), None, Some(0)].into_iter().enumerate() {
                if let Some(k) = flip_to {
                    step!(key(k));
                }
                for _ in 0..14 {
                    step!(Hands { x: 127, ..Default::default() });
                }
                if i == 3 {
                    for _ in 0..10 {
                        step!(Hands::default());
                    }
                }
            }
            put_at!((foes[0].0 - 4.0, foes[0].1 + 1.0));
            for _ in 0..6 {
                step!(Hands::default());
            }
            for k in [1usize, 2, 0] {
                step!(key(k));
                step!(Hands { pressed: button::FIRE, ..Default::default() });
                for _ in 0..30 {
                    step!(Hands::default());
                }
            }
            continue;
        }
        for _ in 0..40 {
            step!(Hands::default());
        }
        for (label, hands, from, ticks) in [
            ("east", Hands { x: 127, ..Default::default() }, (13.0f32, 24.0f32), 34),
            ("west", Hands { x: -127, ..Default::default() }, (34.0, 24.0), 34),
            ("south", Hands { y: 127, ..Default::default() }, (22.0, 14.0), 28),
            ("north", Hands { y: -127, ..Default::default() }, (22.0, 24.0), 28),
        ] {
            put_at!(from);
            for _ in 0..6 {
                step!(Hands::default());
            }
            let start = editor.session.drive.as_ref().map(|d| (d.x, d.y)).unwrap();
            for _ in 0..ticks {
                step!(hands);
            }
            let d = editor.session.drive.as_ref().unwrap();
            assert!((d.x - start.0).hypot(d.y - start.1) > 4.0, "the creature walked {label}");
        }
        put_at!((foes[0].0 - 4.0, foes[0].1 + 1.0));
        for _ in 0..6 {
            step!(Hands::default());
        }
        for _ in 0..2 {
            step!(Hands { pressed: button::FIRE, ..Default::default() });
            for _ in 0..28 {
                step!(Hands::default());
            }
        }
        put_at!((friend.0 + 7.0, friend.1 + 2.0));
        for _ in 0..4 {
            step!(Hands::default());
        }
        step!(Hands { pressed: button::USE, ..Default::default() });
        for _ in 0..60 {
            step!(Hands::default());
        }
        let d = editor.session.drive.as_ref().unwrap();
        assert!(d.strikes >= 1, "a blow reached a foe");
        eprintln!("{tag}: {n} frames; strikes {}", d.strikes);
    }
}

#[test]
fn the_test_drive_binds_the_cast_and_the_fall_to_the_games_own_ultimate_and_bomb_keys() {
    use crate::rules::creation::{Act, Step};
    let mut editor = creation::Editor::default();
    editor.session.open(0, "Ada");
    editor.session.go(Step::Drive, 0);
    // The 2D knight has no cast and no death clip: R and X do nothing.
    editor.hands(&tap(button::X4), 1);
    editor.hands(&tap(button::X1), 2);
    assert!(editor.session.drive.as_ref().is_some_and(|d| d.act.is_none()), "no clip, no cast");
    // The A6 casts on R and lies down on X; a second X gets it up again.
    editor.hands(&tap(button::SLOT2), 3);
    assert_eq!(editor.session.creation.look.as_deref(), Some("a6-crusader"));
    editor.hands(&Hands::default(), 4);
    editor.hands(&tap(button::X4), 10);
    assert!(matches!(editor.session.drive.as_ref().unwrap().acting(10), Some((Act::Cast, 0))), "R casts");
    for t in 11..60 {
        editor.hands(&Hands::default(), t);
    }
    assert!(editor.session.drive.as_ref().unwrap().acting(60).is_none(), "the cast is over");
    editor.hands(&tap(button::X1), 61);
    assert!(editor.session.drive.as_ref().unwrap().down(61), "X lays it down");
    for t in 62..130 {
        editor.hands(&Hands::default(), t);
    }
    assert!(editor.session.drive.as_ref().unwrap().down(130), "and it stays down");
    editor.hands(&tap(button::X1), 131);
    assert!(matches!(editor.session.drive.as_ref().unwrap().acting(131), Some((Act::Rose, 0))), "X again gets it up");
    // The keys only count in the Test drive: on a builder page they are not rows.
    let mut pages = creation::Editor::default();
    pages.session.open(0, "Ada");
    pages.session.go(Step::Paint, 0);
    pages.hands(&tap(button::X4), 1);
    assert!(pages.session.drive.is_none());
}

/// A new knight in the Sozzled Griffin, as the game begins a session with the rite on.
fn newcomer_in_the_bar() -> Run {
    let mut run = crate::rules::rite::tests::realm_with("Ada", home::Home::default());
    run.walk_alehouse();
    run.floor_plates.clear();
    for _ in 0..3 {
        run.step(&Default::default());
    }
    run
}

/// The rite's invitation keys: N held, C pressed.
fn rite_keys() -> angel_net::input::Hands {
    angel_net::input::Hands { held: button::MAP, pressed: button::X11, ..Default::default() }
}

#[test]
fn a_newcomers_screen_is_invited_and_n_with_c_opens_the_creator_while_c_alone_still_rides() {
    let mut host = Host::new(newcomer_in_the_bar());
    assert!(host.run.owes_rite(1));
    let mut c = client(Some(0));
    host.send(&mut c);
    // Arriving in the bar: the slip is up, the Creator is not, and the knight plays on.
    assert!(c.creation.invite.shown(host.run.tick).is_some_and(|(_, slip)| slip), "the full slip first");
    assert!(!c.creation.session.open);
    assert_ne!(c.command(&held(button::RIGHT)), DelveCmd::neutral(), "the knight is not held");
    // C alone is the shoulder ride even while the rite is owed; N held and C opens the Creator.
    assert!(c.command(&tap(button::X11)).input().ride, "C alone rides");
    assert!(!c.creation.session.open);
    assert_eq!(c.command(&rite_keys()), DelveCmd::neutral());
    assert!(c.creation.session.open);
    // Esc puts it off; the slip shrinks to its hint and the knight is as free as before.
    assert_eq!(c.command(&tap(button::MENU)), DelveCmd::neutral());
    assert!(!c.creation.session.open);
    assert!(c.creation.invite.shown(host.run.tick + 1000).is_some_and(|(_, slip)| !slip), "then the corner hint");
    assert!(!c.menu, "nothing was left open behind it");
    // It can be opened again.
    c.command(&rite_keys());
    assert!(c.creation.session.open);
}

#[test]
fn a_returning_knights_screen_has_no_invitation_and_c_stays_the_ride_key() {
    let mut run = newcomer_in_the_bar();
    run.contribute(1, crate::rules::rite::tests::blade("Oathmark")).unwrap();
    let mut host = Host::new(run);
    let mut c = client(Some(0));
    host.send(&mut c);
    assert!(c.creation.invite.shown(host.run.tick).is_none());
    let cmd = c.command(&tap(button::X11));
    assert!(!c.creation.session.open && cmd.input().ride, "C is still the ride key");
}

#[test]
fn what_is_brought_to_life_goes_to_the_host_once_and_the_host_keeps_it_credited_to_its_maker() {
    let mut host = Host::new(newcomer_in_the_bar());
    let mut c = client(Some(0));
    host.send(&mut c);
    c.command(&rite_keys());
    // A minion, through the pages to Life, and sealed there.
    c.creation.session.creation.role = Role::Minion;
    c.creation.session.creation.name = "Pip".into();
    let tick = host.run.tick;
    c.creation.session.go(Step::Life, tick);
    assert!(c.take_actions().is_empty(), "nothing is sent before it is sealed");
    c.command(&tap(button::OK));
    let sent = c.take_actions();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, rite::ACTION);
    // The next press closes the scroll and sends nothing twice.
    c.command(&tap(button::OK));
    assert!(c.take_actions().is_empty());
    // The host reads it as the run's command and keeps it for the realm.
    let made: Made = serde_json::from_slice(&sent[0].1).expect("a Creator piece");
    let line = host.run.contribute(1, made).unwrap();
    assert!(line.contains("Pip"), "{line}");
    assert_eq!(host.run.home.rites.kept.len(), 1);
    assert_eq!(host.run.home.rites.kept[0].knight, "Ada");
    host.send(&mut c);
    assert!(c.creation.invite.shown(host.run.tick).is_none(), "the rite is made: the slip is gone");
}

#[test]
fn a_weapon_brought_to_life_goes_the_same_way() {
    let mut host = Host::new(newcomer_in_the_bar());
    let mut c = client(Some(0));
    host.send(&mut c);
    c.command(&rite_keys());
    c.creation.session.creation.role = Role::Weapon;
    let tick = host.run.tick;
    c.creation.session.go(Step::Life, tick);
    c.command(&tap(button::OK));
    let sent = c.take_actions();
    assert_eq!(sent.len(), 1);
    assert!(matches!(serde_json::from_slice::<Made>(&sent[0].1), Ok(Made::Weapon(_))));
}
