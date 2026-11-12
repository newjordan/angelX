use super::*;
use crate::render::overworld::{arena, ink::said};
use crate::rules::{
    Run,
    creation::{Action, HubRow, Role, WishGate},
};

fn session() -> Session {
    let mut s = Session::default();
    s.open(0, "Ada");
    s.creation.spells.push(Default::default());
    s
}

fn seat(run: &Run, session: &Session, w: i32, h: i32) -> Img {
    arena::in_seat(|| {
        let mut room = arena::frame_for(run, w, arena::view_rows(h), Some(1));
        paint(&mut room, session, run.tick, None);
        arena::seat_lift_frame_with(&room, run, Some(1), &arena::MapView::default())
    })
}

#[test]
fn closed_and_terminal_screens_are_untouched() {
    for (mut im, s) in [
        (Img::black(384, 214), Session::default()),
        (Img::black(120, 60), session()),
    ] {
        let before = im.clone();
        paint(&mut im, &s, 20, None);
        assert_eq!(im, before);
    }
}

#[test]
fn every_step_is_legible_inside_the_native_scroll() {
    let mut s = session();
    s.creation.name = "A".repeat(24);
    s.weapon.name = "W".repeat(24);
    for forge in [false, true] {
      s.creation.role = if forge { Role::Weapon } else { Role::Hero };
      for step in s.flow() {
        s.go(step, 0);
        for row in 0..=s.row_count() {
            s.row = row;
            s.chat.open = false;
            for h in [188, 214] {
                let (_, text) = said::read(|| frame(&s, 20, h, None));
                for line in text.lines {
                    assert!(
                        line.at.0 >= 0
                            && line.at.1 >= 0
                            && line.at.0 + text_width(&line.text) <= line.on.0
                            && line.at.1 + 7 <= line.on.1,
                        "{step:?} row {row}, h={h}: {line:?}"
                    );
                }
            }
        }
      }
    }
    // The chat page, with a long wish and a long answer.
    for forge in [false, true] {
        let mut c = session();
        c.creation.role = if forge { Role::Weapon } else { Role::Hero };
        c.say("a shadow scythe dripping ink and rusty rope held together with toilet-plunger magic", 3);
        c.say("make it taller and bigger and gold and glow blue with a very long tail of words", 4);
        for h in [188, 214] {
            let (_, text) = said::read(|| frame(&c, 20, h, None));
            assert!(text.lines.iter().any(|l| l.text.starts_with("> ")), "the player's lines show");
            for line in text.lines {
                assert!(
                    line.at.0 >= 0 && line.at.0 + text_width(&line.text) <= line.on.0 && line.at.1 + 7 <= line.on.1,
                    "chat, h={h}: {line:?}"
                );
            }
        }
    }
}

#[test]
fn calm_screen_shows_the_settled_grant_without_flashes_or_unrolling() {
    let _viewer =
        crate::render::viewer::enter(crate::render::viewer::Preferences { calm_screen: true });
    let mut s = session();
    s.go(Step::Life, 10);
    s.action(Action::Confirm, 10);
    assert_eq!(frame(&s, 10, HEIGHT, None), frame(&s, 74, HEIGHT, None));
}

#[test]
fn native_pixels_scale_whole_and_motion_is_deterministic() {
    let mut s = session();
    let mut small = Img::black(384, 214);
    let mut big = Img::black(768, 428);
    paint(&mut small, &s, 20, None);
    paint(&mut big, &s, 20, None);
    for y in 0..big.h {
        for x in 0..big.w {
            assert_eq!(big.get(x, y), small.get(x / 2, y / 2));
        }
    }
    s.go(Step::Life, 20);
    s.action(Action::Confirm, 30);
    let frames: Vec<_> = [0, 4, 10, 20, 26, 32, 50]
        .map(|t| frame(&s, 30 + t, HEIGHT, None).content_hash())
        .into();
    assert!(
        frames.windows(2).all(|p| p[0] != p[1]),
        "unroll, reveal and seal all move"
    );
    assert_eq!(frame(&s, 62, HEIGHT, None), frame(&s, 62, HEIGHT, None));
}

#[test]
fn preview_accepts_bounded_atlases_without_changing_the_recipe() {
    assert!(PreviewAtlas::new(Vec::new(), 3).is_err());
    assert!(PreviewAtlas::new(vec![Img::new(97, 54)], 3).is_err());
    assert!(PreviewAtlas::new(vec![Img::new(64, 54)], 0).is_err());
    let mut a = Img::new(16, 24);
    a.rect(2, 2, 12, 20, 'o');
    let b = a.recolor(&[('o', 'M')]);
    let atlas = PreviewAtlas::new(vec![a.clone(), b.clone()], 4).unwrap();
    let c = Creation::default();
    assert_eq!(creature_preview(&c, 0, Some(&atlas)), a);
    assert_eq!(creature_preview(&c, 4, Some(&atlas)), b);
    assert_eq!(creature_preview(&c, 8, Some(&atlas)), a);
    assert_eq!(c, Creation::default());
    let study = creature_preview(&c, 0, None);
    assert!(study.w <= 64 && study.h <= 64);
    let mut dyed = c;
    // The stand-in is moss green as made: Moss and Ochre leave it, the others dye it.
    dyed.paint.scheme = Pigment::Moss;
    assert_eq!(study, creature_preview(&dyed, 0, None));
    dyed.paint.scheme = Pigment::Slate;
    assert_ne!(study, creature_preview(&dyed, 0, None));
    assert_eq!(study.w, 64);
}

/// Entirely offline. Run with --ignored --exact and optionally set
/// ANGEL_CREATOR_SHOTS; the default is this worktree's target/creator-shots.
#[test]
#[ignore]
fn write_creator_shots() {
    let dir = std::env::var_os("ANGEL_CREATOR_SHOTS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/creator-shots")
        });
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, im: &Img| {
        let path = dir.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        image::save_buffer(
            path,
            &im.rgb_bytes(),
            im.w as u32,
            im.h as u32,
            image::ColorType::Rgb8,
        )
        .unwrap();
    };
    let mut run = Run::new(41, 3, Some("Ada"));
    run.tick = 24;
    let mut s = session();
    for step in Step::ALL {
        s.go(step, 0);
        for (w, h) in [(384, 286), (768, 572)] {
            save(
                &format!(
                    "{:02}-{}-{w}.png",
                    step.index() + 1,
                    step.name().to_lowercase()
                ),
                &seat(&run, &s, w, h),
            );
        }
    }
    s.action(Action::Confirm, run.tick);
    for dt in [0, 4, 8, 12, 20, 26, 32, 38, 50] {
        run.tick = 24 + dt;
        for (w, h) in [(384, 286), (768, 572)] {
            save(
                &format!("bring-to-life-{w}/{dt:03}.png"),
                &seat(&run, &s, w, h),
            );
        }
    }
    std::fs::write(
        dir.join("emberkin.creature-v3.json"),
        serde_json::to_string_pretty(s.sealed.as_ref().unwrap()).unwrap(),
    )
    .unwrap();
    s.action(Action::Back, run.tick);
    s.creation.role = Role::Minion;
    s.creation.spells[0].power = 3;
    s.go(Step::Life, run.tick);
    s.action(Action::Confirm, run.tick);
    save("07-over-budget-768.png", &seat(&run, &s, 768, 572));

    // The wish hub: its open face, each gate's page, a shut scroll, and a wrong-row notice.
    let mut hub = Session::default();
    hub.open_hub(0, "Ada", WishGate { power: true, made: 2, wishes: 2, ..Default::default() }, Some(HubRow::Power));
    for (name, row, gate, note) in [
        ("hub-wish", HubRow::Power, WishGate { power: true, made: 2, wishes: 2, ..Default::default() }, ""),
        ("hub-sanctuary", HubRow::Reforge, WishGate { reforge: true, sanctuary: true, made: 1, ..Default::default() }, ""),
        ("hub-mirror", HubRow::Look, WishGate { look: true, ..Default::default() }, ""),
        ("hub-shut", HubRow::Look, WishGate::default(), ""),
        (
            "hub-wrong",
            HubRow::Reforge,
            WishGate { reforge: true, sanctuary: true, ..Default::default() },
            "THAT IS A CREATURE: PRESS 4",
        ),
    ] {
        hub.open_hub(0, "Ada", gate, Some(row));
        hub.note = note.to_string();
        for (w, h) in [(384, 286), (768, 572)] {
            save(&format!("{name}-{w}.png"), &seat(&run, &hub, w, h));
        }
    }
}

fn shots_dir(sub: &str) -> std::path::PathBuf {
    std::env::var_os("ANGEL_CREATOR_SHOTS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/creator-shots"))
        .join(sub)
}

fn save_png(dir: &std::path::Path, name: &str, im: &Img) {
    let path = dir.join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    image::save_buffer(path, &im.rgb_bytes(), im.w as u32, im.h as u32, image::ColorType::Rgb8).unwrap();
}

/// Slice 2: the weapon flow, the Wish conversations, the Bailiff preview. Offline; run with
/// `--ignored --exact`; the shots go to `target/creator-shots/slice2/`.
#[test]
#[ignore]
fn write_slice2_shots() {
    use crate::rules::creation::{Step, WeaponKind};
    let dir = shots_dir("slice2");
    let mut run = Run::new(41, 3, Some("Ada"));
    let mut shoot = |name: &str, s: &Session, tick: u64| {
        run.tick = tick;
        for (w, h) in [(384, 286), (768, 572)] {
            save_png(&dir, &format!("{name}-{w}.png"), &seat(&run, s, w, h));
        }
    };
    let mut s = Session::default();
    s.open(0, "Ada");
    // The role page now offers the Weapon.
    s.action(Action::Row(3), 0);
    for _ in 0..1 { s.action(Action::Down, 0); }
    shoot("w01-role", &s, 24);
    // Type
    s.go(Step::Type, 0);
    shoot("w02-type-blade", &s, 24);
    // Parts, with the blade highlighted
    s.go(Step::Parts, 0);
    s.row = 3;
    shoot("w03-parts-blade", &s, 24);
    s.row = 2;
    shoot("w03-parts-guard", &s, 24);
    s.row = 4;
    shoot("w03-parts-length", &s, 24);
    s.weapon.set_kind(WeaponKind::Haft);
    s.weapon.equip("head", Some("w5w.head-axe-war")).unwrap();
    s.row = 2;
    shoot("w03-parts-axe", &s, 24);
    s.weapon.set_kind(WeaponKind::Staff);
    s.row = 2;
    shoot("w03-parts-staff", &s, 24);
    s.weapon.set_kind(WeaponKind::Shield);
    s.row = 0;
    shoot("w03-parts-shield", &s, 24);
    s.weapon.set_kind(WeaponKind::Bow);
    shoot("w03-parts-bow", &s, 24);
    // Paint on a gold-and-ice staff
    s.weapon.set_kind(WeaponKind::Staff);
    s.weapon.paint.metal = crate::rules::creation::Material::Gold;
    s.weapon.paint.glow = Some(crate::rules::creation::School::Ice);
    s.go(Step::Paint, 0);
    s.row = 0;
    shoot("w04-paint-staff", &s, 24);
    // Enchant on a blade with a fire gem
    s.weapon.set_kind(WeaponKind::Blade);
    s.weapon.paint = Default::default();
    s.weapon.set_gem(Some(crate::rules::creation::School::Fire)).unwrap();
    s.go(Step::Enchant, 0);
    shoot("w05-enchant-blade", &s, 24);
    s.go(Step::Life, 0);
    s.action(Action::Confirm, 24);
    shoot("w06-life-granted", &s, 24 + 50);
    s.action(Action::Back, 80);
    // The wish: the row, then a conversation
    let mut w = Session::default();
    w.open(0, "Ada");
    w.action(Action::Row(3), 0);
    w.action(Action::Down, 0);
    w.go(Step::Parts, 0);
    w.action(Action::Up, 0);
    shoot("x01-wish-row", &w, 24);
    w.say("a flaming bone axe with a skull pommel", 30);
    shoot("x02-wish-axe", &w, 34);
    w.say("bigger skull", 60);
    w.say("make the flame blue", 90);
    shoot("x03-wish-chat-weapon", &w, 120);
    let mut k = Session::default();
    k.open(0, "Ada");
    k.say("a tiny green minion with ram horns that casts ice bolts", 30);
    k.say("make it taller and give it a tail", 60);
    shoot("x04-wish-chat-creature", &k, 90);
    let mut g = Session::default();
    g.open(0, "Ada");
    g.say("the ultimate unstoppable infinite staff", 30);
    shoot("x05-wish-haggle", &g, 70);
    // The Bailiff preview on the creature pages
    let mut c = Session::default();
    c.open(0, "Ada");
    c.go(Step::Body, 0);
    shoot("p01-body-bailiff", &c, 24);
    c.creation.sculpt.height = 100;
    c.creation.sculpt.bulk = 80;
    c.creation.paint.scheme = Pigment::Slate;
    c.go(Step::Parts, 0);
    shoot("p02-parts-bailiff-slate", &c, 24);
    c.go(Step::Powers, 0);
    shoot("p03-powers", &c, 24);
    drop(shoot);
    let dir_of = |name: &str, w: i32| shots_dir("slice2").join(format!("{name}-{w}"));
    let _ = &dir;
    // Sequences at 12 fps of game time (a frame every 2.5 ticks): the weapon's grant, its idle
    // shimmer, and a whole wish conversation, each at both widths.
    let mut frames = |dir: &str, s: &mut Session, from: u64, to: u64, say: &[(u64, &str)]| {
        for w in [384, 768] {
            let _ = std::fs::remove_dir_all(dir_of(dir, w));
        }
        let mut n = 0;
        let mut next = from as f64;
        for t in from..to {
            for (at, words) in say {
                if *at == t {
                    s.say(words, t);
                }
            }
            if (t as f64) >= next {
                next += 2.5;
                run.tick = t;
                for (w, h) in [(384, 286), (768, 572)] {
                    save_png(&dir_of(&dir, w), &format!("{n:03}.png"), &seat(&run, s, w, h));
                }
                n += 1;
            }
        }
    };
    let mut grant = Session::default();
    grant.open(0, "Ada");
    grant.creation.role = Role::Weapon;
    grant.weapon.equip("guard", Some("w5w.guard-winged")).unwrap();
    grant.weapon.set_gem(Some(crate::rules::creation::School::Holy)).unwrap();
    grant.go(Step::Life, 0);
    grant.action(Action::Confirm, 10);
    frames("bring-to-life-weapon", &mut grant, 10, 100, &[]);
    let mut idle = Session::default();
    idle.open(0, "Ada");
    idle.creation.role = Role::Weapon;
    idle.weapon.set_gem(Some(crate::rules::creation::School::Fire)).unwrap();
    idle.go(Step::Enchant, 0);
    frames("weapon-idle", &mut idle, 24, 24 + 96, &[]);
    let mut talk = Session::default();
    talk.open(0, "Ada");
    talk.creation.role = Role::Weapon;
    talk.go(Step::Parts, 0);
    talk.chat.open = true;
    frames(
        "wish-conversation",
        &mut talk,
        24,
        24 + 190,
        &[
            (40, "a flaming bone axe with a skull pommel"),
            (100, "a bone sword with a skull pommel"),
            (140, "bigger skull, and make the flame blue"),
        ],
    );
}

/// Pixels that are painted (`Img::pixels` walks every cell, painted or not).
fn solid(im: &Img) -> usize {
    (0..im.h).flat_map(|y| (0..im.w).map(move |x| (x, y))).filter(|&(x, y)| im.get(x, y).is_some()).count()
}

#[test]
fn the_bailiff_is_drawn_through_the_boss_librarys_own_dressing() {
    use super::baked::{Baked, Motion, Way, stand_in, tint};
    let b = Baked::bailiff();
    assert_eq!((b.id.as_str(), b.size), ("marsh-bailiff", (64, 54)));
    let (rest, windup, strike) = (
        b.frame(Motion::Idle, 0, 0.0),
        b.frame(Motion::Windup, 0, 0.0),
        b.frame(Motion::Strike, 0, 0.0),
    );
    assert!(rest.content_hash() != windup.content_hash() && windup.content_hash() != strike.content_hash());
    // The library's idle rule: the breathing frame shows on its tick, a pixel shorter at the head.
    let breath = b.frame(Motion::Idle, 18, 0.0);
    assert_ne!(rest.content_hash(), breath.content_hash());
    assert_eq!(b.frame(Motion::Idle, 18 + 72, 0.0).content_hash(), breath.content_hash());
    assert_eq!(rest.content_hash(), b.frame(Motion::Idle, 36, 0.0).content_hash());
    // Dressed as the library dresses it: the raw frame is not what is drawn.
    let raw = {
        let png = image::load_from_memory(include_bytes!("../../assets/creator/baked/marsh-bailiff/library-v1-rest.png")).unwrap().to_rgba8();
        let mut im = Img::new(64, 54);
        for (x, y, c) in png.enumerate_pixels() {
            if c[3] > 127 {
                im.set(x as i32, y as i32, [c[0], c[1], c[2]]);
            }
        }
        im
    };
    assert_ne!(raw.content_hash(), rest.content_hash());
    assert_eq!(arena::library_dress(raw, 320).content_hash(), rest.content_hash(), "the same function the boss library calls");
    // Walk cycles come from the library's strips; a walk is not the idle.
    let frames: std::collections::BTreeSet<u64> = (0..40).map(|i| b.frame(Motion::Walk(Way::East), 0, i as f32 * 0.31).content_hash()).collect();
    assert!(frames.len() >= 10, "a real cycle: {}", frames.len());
    assert!(b.frame(Motion::Walk(Way::South), 0, 1.0).content_hash() != b.frame(Motion::Walk(Way::East), 0, 1.0).content_hash());
    // A frame of the wrong size is refused, not drawn crooked.
    let import = include_str!("../../assets/creator/baked/marsh-bailiff/library-import.json");
    let tiny = {
        let mut out = Vec::new();
        image::DynamicImage::ImageRgba8(image::RgbaImage::new(4, 4))
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    };
    let ok = include_bytes!("../../assets/creator/baked/marsh-bailiff/library-v1-rest.png");
    assert!(Baked::from_library(import, &tiny, ok, ok).is_err());
    assert!(Baked::from_library("{}", ok, ok, ok).is_err());
    // The dye keeps the shading (distinct colours stay distinct) and leaves the outline alone.
    let moss = stand_in(&Creation::default(), Motion::Idle, 0, 0.0);
    let mut slate = Creation::default();
    slate.paint.scheme = Pigment::Slate;
    let dyed = stand_in(&slate, Motion::Idle, 0, 0.0);
    let colours = |im: &Img| {
        (0..im.h).flat_map(|y| (0..im.w).map(move |x| (x, y))).filter_map(|(x, y)| im.get(x, y)).collect::<std::collections::BTreeSet<_>>().len()
    };
    assert!(colours(&dyed) * 10 >= colours(&moss) * 5, "{} vs {}", colours(&dyed), colours(&moss));
    assert_eq!(tint([10, 8, 9], Pigment::Slate), [10, 8, 9]);
    assert_eq!(tint([120, 160, 60], Pigment::Ochre), [120, 160, 60]);
    assert_ne!(tint([120, 160, 60], Pigment::Russet), [120, 160, 60]);
    // Height and bulk reshape it; the recipe itself is untouched.
    let mut tall = Creation::default();
    tall.sculpt.height = 100;
    assert!(stand_in(&tall, Motion::Idle, 0, 0.0).h > moss.h);
}

#[test]
fn a_weapon_is_drawn_from_its_parts_and_follows_its_paint() {
    use crate::rules::creation::{Material, School, WeaponCreation, WeaponKind};
    let blade = WeaponCreation::default();
    let base = weapon::assembled(&blade, None).unwrap();
    let a = blade.assemble().unwrap();
    assert_eq!((base.w, base.h), a.size, "the picture is exactly the size the rules placed");
    for kind in WeaponKind::ALL {
        let w = WeaponCreation::of_kind(kind);
        let img = weapon::assembled(&w, None).unwrap_or_else(|| panic!("{kind:?}"));
        assert!(solid(&img) > 100, "{kind:?} is drawn");
        let up = weapon::upright(&img);
        assert_eq!((up.w, up.h), (img.h, img.w));
        let (fit, k) = weapon::fitted(&up, 112, 86);
        assert!(fit.w <= 112 && fit.h <= 86 && (1..=4).contains(&k), "{kind:?} 1:{k} {}x{}", fit.w, fit.h);
    }
    // Every choice shows: a part, a metal, a gem, a length, a glow, a scale.
    let mut variants = vec![base.content_hash()];
    let mut w = blade.clone();
    w.equip("blade", Some("w5w.blade-bone")).unwrap();
    variants.push(weapon::assembled(&w, None).unwrap().content_hash());
    let mut w = blade.clone();
    w.paint.metal = Material::Gold;
    variants.push(weapon::assembled(&w, None).unwrap().content_hash());
    let mut w = blade.clone();
    w.set_gem(Some(School::Fire)).unwrap();
    variants.push(weapon::assembled(&w, None).unwrap().content_hash());
    let mut tinted = w.clone();
    tinted.paint.glow = Some(School::Ice);
    variants.push(weapon::assembled(&tinted, None).unwrap().content_hash());
    let mut w = blade.clone();
    w.set_stretch(Some(140)).unwrap();
    variants.push(weapon::assembled(&w, None).unwrap().content_hash());
    let mut w = blade.clone();
    w.parts[0].scale = 150;
    variants.push(weapon::assembled(&w, None).unwrap().content_hash());
    let all: std::collections::BTreeSet<_> = variants.iter().collect();
    assert_eq!(all.len(), variants.len(), "every choice changes the picture");
    // The marked slot gets its teal halo outside the weapon, and only there.
    let (plain, k) = weapon::display(&blade, None, (112, 86), (128, 80)).unwrap();
    let (marked, k2) = weapon::display(&blade, Some("guard"), (112, 86), (128, 80)).unwrap();
    assert_eq!(k, k2, "marking a part never changes the ratio");
    assert!(solid(&marked) > solid(&plain), "the halo adds pixels at display size: {} vs {}", solid(&marked), solid(&plain));
    assert_eq!((marked.w, marked.h), (plain.w, plain.h));
    // A stretched blade keeps its tip and its pommel apart by exactly the stretch.
    let longer = weapon::assembled(&w_stretch(140), None).unwrap();
    assert_eq!(longer.w, base.w + 32);
    // The gem pulses and the glint travels, deterministically.
    let (mut one, mut two) = (base.clone(), base.clone());
    weapon::shimmer(&mut one, 10, None);
    weapon::shimmer(&mut two, 10, None);
    assert_eq!(one, two);
    weapon::shimmer(&mut two, 60, None);
    assert_ne!(one.content_hash(), two.content_hash());
}

fn w_stretch(len: u16) -> crate::rules::creation::WeaponCreation {
    let mut w = crate::rules::creation::WeaponCreation::default();
    w.set_stretch(Some(len)).unwrap();
    w
}

#[test]
fn the_wish_row_and_chat_sit_inside_the_scroll_and_the_stage_reacts_to_a_wish() {
    let mut s = session();
    s.go(Step::Parts, 0);
    let plain = frame(&s, 40, HEIGHT, None);
    s.row = s.row_count();
    let wishing = frame(&s, 40, HEIGHT, None);
    assert_ne!(plain, wishing, "the wish row lights when it is selected");
    s.chat.open = true;
    let chat = frame(&s, 40, HEIGHT, None);
    assert_ne!(wishing, chat);
    // A wish makes the figure step out of the ink again, then settle.
    s.chat.open = false;
    s.say("a tiny green minion", 50);
    let early = frame(&s, 52, HEIGHT, None);
    let settled = frame(&s, 90, HEIGHT, None);
    assert_ne!(early, settled);
    // Nothing the wish row says can run past the scroll.
    s.say(&"zzzzz ".repeat(40), 100);
    let (_, text) = said::read(|| frame(&s, 120, HEIGHT, None));
    assert!(text.lines.iter().all(|l| l.at.0 + text_width(&l.text) <= l.on.0));
}

#[test]
fn the_test_drive_draws_the_creature_over_the_real_arena_and_marks_a_blow() {
    use crate::rules::creation::{DriveInput, Ground};
    let mut floor = Floor::new("Ada");
    let mut s = session();
    s.can_drive = true;
    s.go(Step::Life, 0);
    s.action(Action::Confirm, 1);
    s.action(Action::Confirm, 2);
    assert_eq!(s.step, Step::Drive);
    let ground = |f: &Floor| (f.spawn, f.foes.clone(), f.friend);
    let (spawn, foes, friend) = ground(&floor);
    let mut t = 3;
    let mut drive = |s: &mut Session, floor: &mut Floor, input: DriveInput, t: &mut u64| {
        let g = Ground { spawn, walkable: &|x, y| floor.walkable(x, y), foes: &foes, friend: Some(friend) };
        s.drive_step(input, *t, &g);
        let d = s.drive.clone().unwrap();
        floor.prepare(&d, *t);
        *t += 1;
    };
    drive(&mut s, &mut floor, DriveInput::default(), &mut t);
    let idle = drive::frame(&s, &floor, t, 384, 214);
    assert_eq!((idle.w, idle.h), (384, 214));
    // Put the creature beside the first foe (on open floor) and strike.
    let beside = floor.standing((foes[0].0 - 4.0, foes[0].1 + 1.0));
    s.drive.as_mut().unwrap().x = beside.0;
    s.drive.as_mut().unwrap().y = beside.1;
    assert!((beside.0 - foes[0].0).hypot(beside.1 - foes[0].1) <= 7.5, "within reach: {beside:?} {:?}", foes[0]);
    drive(&mut s, &mut floor, DriveInput { attack: true, ..Default::default() }, &mut t);
    let at = t - 1;
    let mut seen = std::collections::BTreeMap::new();
    seen.insert(at, drive::frame(&s, &floor, at, 384, 214));
    for _ in 0..26 {
        drive(&mut s, &mut floor, DriveInput::default(), &mut t);
        let now = t - 1;
        seen.insert(now, drive::frame(&s, &floor, now, 384, 214));
    }
    let (windup, strike) = (seen[&(at + 2)].clone(), seen[&(at + 8 + 2)].clone());
    assert!(s.drive.as_ref().unwrap().last_hit.is_some_and(|h| h.foe == 0 && h.damage > 0));
    assert_ne!(windup.content_hash(), strike.content_hash(), "windup and strike are different frames");
    assert_ne!(idle.content_hash(), windup.content_hash());
    // The floor marks the foe through the game's own hurt flash, then lets it go.
    assert!(floor.run.enemies[0].hurt == 0 || floor.run.enemies[0].hurt <= 8);
    floor.prepare(s.drive.as_ref().unwrap(), at + 8 + 1);
    assert!(floor.run.enemies[0].hurt > 0 && floor.run.enemies[1].hurt == 0);
    floor.prepare(s.drive.as_ref().unwrap(), at + 8 + 30);
    assert_eq!(floor.run.enemies[0].hurt, 0);
    // Deterministic: the same moment draws the same frame.
    assert_eq!(drive::frame(&s, &floor, t, 384, 214), drive::frame(&s, &floor, t, 384, 214));
    let hashes: Vec<u64> = seen.values().map(Img::content_hash).collect();
    assert!(hashes.windows(2).any(|p| p[0] != p[1]));
    // The whole screen paints inside the room view, and the plate shows the derived numbers.
    let (_, text) = said::read(|| drive::frame(&s, &floor, t, 384, 214));
    let lines: Vec<_> = text.lines.iter().map(|l| l.text.as_str()).collect();
    assert!(lines.iter().any(|l| l.starts_with("HP ")) && lines.iter().any(|l| l.starts_with("MELEE ")), "{lines:?}");
    assert!(lines.iter().any(|l| l.contains("POWER")), "{lines:?}");
    // The creature stays on the floor: it cannot be walked into a wall.
    let g = Ground { spawn, walkable: &|x, y| floor.walkable(x, y), foes: &foes, friend: Some(friend) };
    for i in 0..400 {
        s.drive_step(DriveInput { dx: -1, dy: -1, ..Default::default() }, t + 100 + i, &g);
    }
    let d = s.drive.as_ref().unwrap();
    assert!(floor.walkable(d.x, d.y), "stopped at ({}, {})", d.x, d.y);
}

// ---- Slice 3: baked sets other than the Bailiff, through the registry ----

/// The last opaque row of a figure.
fn soles(im: &Img) -> i32 {
    (0..im.h).rev().find(|&y| (0..im.w).any(|x| im.get(x, y).is_some())).unwrap_or(-1)
}

/// The opaque bounding box of a figure: (columns, rows).
fn extent(im: &Img) -> (i32, i32) {
    let cols: Vec<i32> = (0..im.w).filter(|&x| (0..im.h).any(|y| im.get(x, y).is_some())).collect();
    let rows: Vec<i32> = (0..im.h).filter(|&y| (0..im.w).any(|x| im.get(x, y).is_some())).collect();
    (cols.last().unwrap() - cols[0] + 1, rows.last().unwrap() - rows[0] + 1)
}

fn hero() -> Creation {
    Creation { role: Role::Hero, ..Creation::default() }
}

#[test]
fn every_baked_set_loads_and_keeps_the_librarys_shape() {
    use super::baked::all_sets;
    let sets = all_sets();
    assert!(sets.len() >= 2, "the stand-in and at least one real set");
    for set in sets {
        let id = set.id.as_str();
        assert_eq!(set.size, (64, 54), "{id}: the library's v1 envelope");
        // Three poses, three pictures; none breaks the 54-row ceiling or the widest shipped boss (68).
        let poses = [Motion::Idle, Motion::Windup, Motion::Strike].map(|m| set.frame(m, 0, 0.0));
        let hashes: std::collections::BTreeSet<u64> = poses.iter().map(|p| p.content_hash()).collect();
        assert_eq!(hashes.len(), 3, "{id}: rest, windup and strike are different frames");
        for p in poses {
            let (cols, rows) = extent(p);
            assert!(rows <= 54 && cols <= 68, "{id}: {cols}x{rows} is over policy");
        }
        // The anchor holds: standing, the soles end on the library's row once the set's own anchor is applied.
        assert_eq!(soles(poses[0]) + set.drop(Motion::Idle), 52, "{id}: soles on row 52");
        for way in [Way::East, Way::South] {
            // A walk is a real cycle, the same length on the ground for every creature; it may bob up
            // (the Bailiff's front walk rises three rows) but its soles never sink more than two rows
            // below the standing row.
            let at = |walked: f32| set.frame(Motion::Walk(way), 0, walked);
            let cycle: std::collections::BTreeSet<u64> = (0..120).map(|i| at(i as f32 * 0.1).content_hash()).collect();
            assert!(cycle.len() >= 8, "{id} {way:?}: {} distinct walk frames", cycle.len());
            assert_eq!(at(0.01).content_hash(), at(0.01 + 0.62 * 12.0).content_hash(), "{id} {way:?}: one cycle is 7.44 units of ground");
            for i in 0..120 {
                let row = soles(at(i as f32 * 0.1)) + set.drop(Motion::Walk(way));
                assert!((46..=54).contains(&row), "{id} {way:?}: soles on row {row}");
            }
        }
        assert_ne!(
            set.frame(Motion::Walk(Way::South), 0, 1.0).content_hash(),
            set.frame(Motion::Walk(Way::East), 0, 1.0).content_hash(),
            "{id}: toward the camera is not along the floor"
        );
        // Idle moves (a breath or a loop) and is never the strike.
        let idle: std::collections::BTreeSet<u64> = (0..240).map(|t| set.frame(Motion::Idle, t, 0.0).content_hash()).collect();
        assert!(idle.len() >= 2 && !idle.contains(&poses[2].content_hash()), "{id}: idle");
    }
}

#[test]
fn a_role_or_a_look_picks_the_baked_set() {
    use super::baked::{Row, for_creation, for_look, pick};
    let with = |role| Creation { role, ..Creation::default() };
    assert_eq!(for_creation(&with(Role::Hero)).id, "knight");
    for role in [Role::Minion, Role::Monster, Role::Boss] {
        assert_eq!(for_creation(&with(role)).id, "marsh-bailiff", "{role:?} keeps the stand-in");
    }
    assert_eq!(for_look(&with(Role::Hero), Some("a look nobody baked")).id, "knight", "an unknown look falls to the role's row");
    assert_eq!(for_look(&with(Role::Monster), Some("a look nobody baked")).id, "marsh-bailiff");
    // A look outranks a role, and a row with no role serves every role.
    let knight: fn() -> &'static Baked = || for_creation(&hero());
    let rows = [
        Row { role: Some(Role::Hero), look: None, set: Baked::bailiff },
        Row { role: None, look: Some("frog"), set: knight },
    ];
    assert_eq!(pick(&rows, &hero(), None).id, "marsh-bailiff");
    assert_eq!(pick(&rows, &hero(), Some("frog")).id, "knight");
    assert_eq!(pick(&rows, &with(Role::Boss), Some("frog")).id, "knight");
    assert_eq!(pick(&rows, &with(Role::Boss), None).id, "marsh-bailiff", "no row: the stand-in");
    // The preview shows what the table says, and nothing else about the recipe changes it.
    assert_ne!(creature_preview(&hero(), 0, None).content_hash(), creature_preview(&with(Role::Monster), 0, None).content_hash());
}

#[test]
fn the_stage_names_its_stand_in() {
    let mut s = session();
    s.go(Step::Body, 0);
    let engraved = |s: &Session| {
        let (_, text) = said::read(|| frame(s, 20, HEIGHT, None));
        text.lines.iter().map(|l| l.text.clone()).collect::<Vec<_>>()
    };
    assert!(engraved(&s).iter().any(|l| l == "STAND-IN: KNIGHT"), "{:?}", engraved(&s));
    s.creation.role = Role::Monster;
    assert!(engraved(&s).iter().any(|l| l == "STAND-IN: BAILIFF"));
}

#[test]
fn the_whole_smash_plays_windup_then_strike_on_the_drives_ticks() {
    use super::baked::all_sets;
    use crate::rules::creation::drive::{ATTACK_TICKS, WINDUP};
    let mut with_smash = 0;
    for set in all_sets() {
        if set.swing(0).is_none() {
            // A set with no smash holds the library's two poses (the Bailiff's blow, as in slice 2).
            assert!(set.swing(WINDUP).is_none() && !set.carries_motion(), "{}", set.id);
            continue;
        }
        with_smash += 1;
        let id = set.id.as_str();
        let pose = |m| set.frame(m, 0, 0.0).content_hash();
        let at = |since| set.swing(since).unwrap().content_hash();
        for since in 0..ATTACK_TICKS {
            assert!(set.swing(since).is_some(), "{id}: tick {since} of the blow has a frame");
        }
        assert!(set.swing(ATTACK_TICKS).is_none(), "{id}: the blow is over");
        // The rise changes tick by tick, holds at the library's windup pose, swings, then the strike
        // frame lands on the first strike tick and not before.
        let rise: Vec<u64> = (0..WINDUP - 1).map(at).collect();
        // The rise plays from the clip's first moving frame to its windup key: as many frames as the clip gives it (up to three).
        let (from, peak) = (set.swing_index(0).unwrap(), set.swing_index(WINDUP - 3).unwrap());
        assert!(rise.windows(2).filter(|p| p[0] != p[1]).count() >= (peak - from).min(3), "{id}: the windup moves ({from} to {peak})");
        assert_eq!(at(WINDUP - 2), pose(Motion::Windup), "{id}: the coil holds at the windup pose");
        assert_ne!(at(WINDUP - 1), pose(Motion::Strike), "{id}: not yet");
        assert_eq!(at(WINDUP), pose(Motion::Strike), "{id}: the blow lands its strike frame");
        assert!(!rise.contains(&pose(Motion::Strike)), "{id}: windup comes first");
        // Recovery leaves the strike, step by step, and ends back on the standing pose. A clip from the
        // pipeline ends on its own last frame (a follow-through, not the stance: the drive's idle takes over after it).
        let recover: Vec<u64> = (WINDUP + 6..ATTACK_TICKS).map(at).collect();
        assert!(recover.windows(2).filter(|p| p[0] != p[1]).count() >= 3, "{id}: recovery moves");
        let settles = match set.way_frames("attack", Way::South) {
            Some(attack) => attack.last().unwrap().content_hash(),
            None => set.frame(Motion::Idle, 0, 0.0).content_hash(),
        };
        assert_eq!(at(ATTACK_TICKS - 1), settles, "{id}: settles where its clip ends");
        assert!(set.carries_motion());
    }
    assert!(with_smash >= 1, "a real set brings the whole smash");
}

#[test]
fn a_state_authored_off_the_standing_row_is_drawn_to_meet_the_ground() {
    // The hero's set authors its front walk two rows high (room for the stepping foot): its record
    // says so, and the drive draws that state two rows lower. Every other state stands on the line.
    let set = super::baked::for_creation(&hero());
    assert_eq!(set.drop(Motion::Walk(Way::South)), 2);
    for m in [Motion::Idle, Motion::Windup, Motion::Strike, Motion::Walk(Way::East)] {
        assert_eq!(set.drop(m), 0, "{m:?}");
    }
    assert_eq!(Baked::bailiff().drop(Motion::Walk(Way::South)), 0, "the stand-in has no animation record");
}

#[test]
fn pigment_dyes_a_sets_main_material_and_leaves_the_rest() {
    use super::baked::{GREENS, stand_in, tint, tint_with};
    let at = |p: Pigment| {
        let mut c = hero();
        c.paint.scheme = p;
        stand_in(&c, Motion::Idle, 0, 0.0)
    };
    let kinds = |im: &Img| (0..im.h).flat_map(|y| (0..im.w).map(move |x| (x, y))).filter_map(|(x, y)| im.get(x, y)).collect::<std::collections::BTreeSet<_>>().len();
    let made = at(Pigment::Ochre);
    assert_eq!(made.content_hash(), at(Pigment::Moss).content_hash(), "the earth pigments leave a baked creature as made");
    let total = solid(&made);
    for p in [Pigment::Slate, Pigment::Russet, Pigment::Bone, Pigment::Charcoal] {
        let dyed = at(p);
        let kept = (0..made.h).flat_map(|y| (0..made.w).map(move |x| (x, y))).filter(|&(x, y)| made.get(x, y).is_some() && made.get(x, y) == dyed.get(x, y)).count();
        assert_ne!(dyed.content_hash(), made.content_hash(), "{p:?} shows");
        assert!(kept * 10 >= total, "{p:?}: trim and outline stay ({kept} of {total})");
        assert!((total - kept) * 10 >= total, "{p:?}: the main material turns ({} of {total})", total - kept);
        assert!(kinds(&dyed) * 10 >= kinds(&made) * 5, "{p:?}: shading survives ({} vs {})", kinds(&dyed), kinds(&made));
    }
    // The window is the set's own: the Bailiff's greens are not the knight's plate.
    let green = tint([120, 160, 60], Pigment::Russet);
    assert_ne!(green, [120, 160, 60]);
    assert_eq!(tint_with([103, 120, 156], Pigment::Russet, GREENS), [103, 120, 156], "blue steel is outside the Bailiff's window");
}

#[test]
fn a_bad_animation_record_is_refused_rather_than_played_crooked() {
    let import = include_str!("../../assets/creator/baked/knight/library-import.json");
    let rest = include_bytes!("../../assets/creator/baked/knight/library-v1-rest.png");
    let attack = include_bytes!("../../assets/creator/baked/knight/library-v1-attack-strip.png");
    let record: serde_json::Value = serde_json::from_str(include_str!("../../assets/creator/baked/knight/animation.json")).unwrap();
    let base = Baked::from_library(import, rest, rest, rest)
        .and_then(|b| b.with_walks(include_bytes!("../../assets/creator/baked/knight/library-v1-walk-east-strip.png"), include_bytes!("../../assets/creator/baked/knight/library-v1-walk-south-strip.png")))
        .and_then(|b| b.with_idle(include_bytes!("../../assets/creator/baked/knight/library-v1-idle-strip.png")))
        .and_then(|b| b.with_swing(attack))
        .unwrap();
    assert!(base.clone().with_animation(&record.to_string()).is_ok());
    let broken = |edit: &dyn Fn(&mut serde_json::Value)| {
        let mut r = record.clone();
        edit(&mut r);
        base.clone().with_animation(&r.to_string())
    };
    assert!(broken(&|r| r["attack_events"]["total_ticks"] = 41.into()).is_err(), "keys must fit the strip");
    assert!(broken(&|r| r["attack_events"]["strike"] = 99.into()).is_err(), "a key past the last frame");
    assert!(broken(&|r| r["attack_events"]["swing"] = 3.into()).is_err(), "keys rise in order");
    assert!(broken(&|r| { r["states"].as_object_mut().unwrap().remove("walk-south"); }).is_err(), "a loaded clip needs its state");
    assert!(broken(&|r| { r["attack_events"] = serde_json::Value::Null; }).is_err(), "the smash and its keys come together");
    assert!(broken(&|r| r["creator"]["name"] = "A NAME THAT IS FAR TOO LONG".into()).is_err());
    assert!(broken(&|r| r["creator"]["name"] = " ".into()).is_err());
    // A strip of the wrong height is refused whatever it is for.
    let tiny = {
        let mut out = Vec::new();
        image::DynamicImage::ImageRgba8(image::RgbaImage::new(64, 12))
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    };
    assert!(base.clone().with_idle(&tiny).is_err() && base.clone().with_swing(&tiny).is_err() && base.clone().with_walks(&tiny, &tiny).is_err());
}

/// Slice 3: the Hero's baked set on the stage and beside the Bailiff and Cinderjaw in the arena.
/// Offline; run with `--ignored --exact`; the shots go to `target/creator-shots/slice3/`.
#[test]
#[ignore]
fn write_slice3_shots() {
    use super::baked::stand_in;
    use crate::rules::creation::Step;
    let dir = shots_dir("slice3");
    let mut run = Run::new(41, 3, Some("Ada"));
    let mut shoot = |name: &str, s: &Session, tick: u64| {
        run.tick = tick;
        for (w, h) in [(384, 286), (768, 572)] {
            save_png(&dir, &format!("{name}-{w}.png"), &seat(&run, s, w, h));
        }
    };
    let mut hero = Session::default();
    hero.open(0, "Ada");
    hero.go(Step::Body, 0);
    shoot("h01-body-knight", &hero, 24);
    hero.go(Step::Parts, 0);
    hero.row = 2;
    shoot("h02-parts-knight", &hero, 24);
    hero.creation.paint.scheme = Pigment::Russet;
    hero.go(Step::Paint, 0);
    shoot("h03-paint-knight-russet", &hero, 24);
    hero.creation.paint.scheme = Pigment::Ochre;
    hero.creation.sculpt.height = 80;
    hero.creation.sculpt.bulk = 70;
    hero.go(Step::Body, 0);
    shoot("h04-body-knight-taller-broader", &hero, 24);
    let mut monster = Session::default();
    monster.open(0, "Ada");
    monster.creation.role = Role::Monster;
    monster.go(Step::Body, 0);
    shoot("h05-body-monster-keeps-the-bailiff", &monster, 24);
    let mut grant = Session::default();
    grant.open(0, "Ada");
    grant.go(Step::Life, 0);
    grant.action(Action::Confirm, 10);
    shoot("h06-life-granted-knight", &grant, 10 + 60);
    // Every pigment, side by side at x4, then the poses (rest, windup, strike) at x6.
    let sheet = |frames: Vec<Img>, k: i32, name: &str| {
        let cell = frames.iter().map(|f| f.w).max().unwrap() + 4;
        let h = frames.iter().map(|f| f.h).max().unwrap() + 8;
        let mut out = Img::black(cell * frames.len() as i32 + 4, h);
        for (i, f) in frames.iter().enumerate() {
            out.stamp(f, 4 + i as i32 * cell, h - 4 - f.h);
        }
        let mut big = Img::black(out.w * k, out.h * k);
        big.stamp_times(&out, 0, 0, k);
        save_png(&dir, name, &big);
    };
    let dyed: Vec<Img> = Pigment::ALL
        .iter()
        .map(|&p| {
            let mut c = Creation::default();
            c.paint.scheme = p;
            stand_in(&c, Motion::Idle, 0, 0.0)
        })
        .collect();
    sheet(dyed, 3, "h07-pigments-x3.png");
    let poses: Vec<Img> = [Motion::Idle, Motion::Windup, Motion::Strike, Motion::Walk(Way::East), Motion::Walk(Way::South)]
        .iter()
        .map(|&m| stand_in(&Creation::default(), m, 0, 0.0))
        .collect();
    sheet(poses, 4, "h08-poses-x4.png");
    // The arena: the knight and the Bailiff stand-in on the real floor, with Cinderjaw from the
    // library's own file at his registered grade, and the camera knight for scale.
    let floor = Floor::new("Ada");
    let at = |x: f32, y: f32| arena::frame_px(&floor.run, 384, 224, Some(1), x, y);
    let cinder = {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../cockpit/assets/dungeon/bosses/cinderjaw/library-v1-rest.png");
        let png = image::open(path).expect("cinderjaw's library art").to_rgba8();
        let mut im = Img::new(png.width() as i32, png.height() as i32);
        for (x, y, c) in png.enumerate_pixels() {
            if c[3] > 127 {
                im.set(x as i32, y as i32, [c[0], c[1], c[2]]);
            }
        }
        arena::library_dress(im, 432)
    };
    let monster_creation = Creation { role: Role::Monster, ..Creation::default() };
    let mut canvas = arena::frame_for(&floor.run, 384, 224, Some(1));
    let spots = [(22.0, 24.0), (30.0, 24.0), (38.0, 24.0)];
    for ((x, y), figure) in spots.iter().zip([cinder, stand_in(&monster_creation, Motion::Idle, 0, 0.0), stand_in(&Creation::default(), Motion::Idle, 0, 0.0)]) {
        let spot = floor.standing((*x, *y));
        let (fx, fy) = at(spot.0, spot.1);
        canvas.ellipse(fx as f32, fy as f32 - 1.0, figure.w as f32 * 0.42, 3.5, 'K');
        canvas.stamp(&figure, fx - figure.w / 2, fy - figure.h);
    }
    save_png(&dir, "h09-arena-cinderjaw-bailiff-knight-384.png", &canvas);
    let mut big = Img::black(canvas.w * 3, canvas.h * 3);
    big.stamp_times(&canvas, 0, 0, 3);
    save_png(&dir, "h09-arena-cinderjaw-bailiff-knight-x3.png", &big);
    drop(shoot);
    // Sequences at 12 fps of game time: the stage's idle loop and the grant's reveal, both widths.
    let dir_of = |name: &str, w: i32| shots_dir("slice3").join(format!("{name}-{w}"));
    let mut frames = |name: &str, s: &mut Session, from: u64, to: u64| {
        for w in [384, 768] {
            let _ = std::fs::remove_dir_all(dir_of(name, w));
        }
        let (mut n, mut next) = (0, from as f64);
        for t in from..to {
            if (t as f64) >= next {
                next += 2.5;
                let mut r = Run::new(41, 3, Some("Ada"));
                r.tick = t;
                for (w, h) in [(384, 286), (768, 572)] {
                    save_png(&dir_of(name, w), &format!("{n:03}.png"), &seat(&r, s, w, h));
                }
                n += 1;
            }
        }
    };
    let mut idle = Session::default();
    idle.open(0, "Ada");
    idle.go(Step::Body, 0);
    frames("knight-idle", &mut idle, 24, 24 + 118);
    let mut life = Session::default();
    life.open(0, "Ada");
    life.go(Step::Life, 0);
    life.action(Action::Confirm, 10);
    frames("bring-to-life-knight", &mut life, 10, 100);
}

/// Slice 3, every tick: a creature's blow and its walks cropped from the real arena frame, one
/// PNG per game tick, so the motion can be read frame by frame. Offline; `--ignored --exact`.
#[test]
#[ignore]
fn write_slice3_tick_crops() {
    use crate::rules::creation::{DriveInput, Ground, Step};
    let base = shots_dir("slice3");
    for role in [Role::Hero, Role::Monster] {
        let tag = if role == Role::Hero { "knight" } else { "bailiff" };
        let mut floor = Floor::new("Ada");
        let mut s = session();
        s.creation.role = role;
        s.can_drive = true;
        s.go(Step::Life, 0);
        s.action(Action::Confirm, 1);
        s.action(Action::Confirm, 2);
        let (spawn, foes, friend) = (floor.spawn, floor.foes.clone(), floor.friend);
        let mut t = 3u64;
        let mut tick = |s: &mut Session, floor: &mut Floor, input: DriveInput, t: &mut u64| {
            let g = Ground { spawn, walkable: &|x, y| floor.walkable(x, y), foes: &foes, friend: Some(friend) };
            s.drive_step(input, *t, &g);
            floor.prepare(&s.drive.clone().unwrap(), *t);
            *t += 1;
        };
        tick(&mut s, &mut floor, DriveInput::default(), &mut t);
        // A frame cropped round the creature's feet: 96 wide, 76 high, feet 8 rows from the bottom.
        let crop = |s: &Session, floor: &Floor, t: u64| {
            let canvas = drive::frame(s, floor, t, 384, 224);
            let d = s.drive.as_ref().unwrap();
            let (fx, fy) = arena::frame_px(&floor.run, 384, 224, Some(1), d.x, d.y);
            let mut out = Img::black(96, 76);
            for y in 0..76 {
                for x in 0..96 {
                    if let Some(c) = canvas.get(fx - 48 + x, fy - 68 + y) {
                        out.set(x, y, c);
                    }
                }
            }
            out
        };
        // Every tick of a blow beside the first foe: four before, the twenty of the blow, four after.
        let beside = floor.standing((foes[0].0 - 4.0, foes[0].1 + 1.0));
        s.drive.as_mut().unwrap().x = beside.0;
        s.drive.as_mut().unwrap().y = beside.1;
        for _ in 0..4 {
            tick(&mut s, &mut floor, DriveInput::default(), &mut t);
        }
        let dir = base.join(format!("blow-ticks-{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        let mut n = 0;
        let mut dump = |s: &Session, floor: &Floor, t: u64| {
            save_png(&dir, &format!("{n:03}.png"), &crop(s, floor, t));
            n += 1;
        };
        for _ in 0..4 {
            tick(&mut s, &mut floor, DriveInput::default(), &mut t);
            dump(&s, &floor, t - 1);
        }
        tick(&mut s, &mut floor, DriveInput { attack: true, ..Default::default() }, &mut t);
        dump(&s, &floor, t - 1);
        for _ in 0..26 {
            tick(&mut s, &mut floor, DriveInput::default(), &mut t);
            dump(&s, &floor, t - 1);
        }
        // Walks: east for 40 ticks (facing right), west for 40 (mirrored), south, north, each from open floor.
        for (name, input, from) in [
            ("walk-east", DriveInput { dx: 1, ..Default::default() }, (14.0f32, 20.0f32)),
            ("walk-west", DriveInput { dx: -1, ..Default::default() }, (30.0, 20.0)),
            ("walk-south", DriveInput { dy: 1, ..Default::default() }, (20.0, 12.0)),
            ("walk-north", DriveInput { dy: -1, ..Default::default() }, (20.0, 22.0)),
        ] {
            let start = floor.standing(from);
            {
                let d = s.drive.as_mut().unwrap();
                (d.x, d.y, d.walked, d.attack_at) = (start.0, start.1, 0.0, None);
            }
            for _ in 0..3 {
                tick(&mut s, &mut floor, DriveInput::default(), &mut t);
            }
            let dir = base.join(format!("{name}-ticks-{tag}"));
            let _ = std::fs::remove_dir_all(&dir);
            for i in 0..48 {
                tick(&mut s, &mut floor, input, &mut t);
                save_png(&dir, &format!("{i:03}.png"), &crop(&s, &floor, t - 1));
            }
            let d = s.drive.as_ref().unwrap();
            eprintln!("{tag} {name}: from ({:.1},{:.1}) to ({:.1},{:.1}), walked {:.1}", start.0, start.1, d.x, d.y, d.walked);
        }
    }
}

#[test]
fn the_smash_maps_onto_whatever_keys_a_clip_declares() {
    use crate::rules::creation::drive::{ATTACK_TICKS, WINDUP};
    use super::baked::Set;
    let record: serde_json::Value = serde_json::from_str(include_str!("../../assets/creator/baked/knight/animation.json")).unwrap();
    let base = Baked::from_set(Set {
        import: include_str!("../../assets/creator/baked/knight/library-import.json"),
        animation: include_str!("../../assets/creator/baked/knight/animation.json"),
        rest: include_bytes!("../../assets/creator/baked/knight/library-v1-rest.png"),
        windup: include_bytes!("../../assets/creator/baked/knight/library-v1-windup.png"),
        strike: include_bytes!("../../assets/creator/baked/knight/library-v1-strike.png"),
        walk_east: include_bytes!("../../assets/creator/baked/knight/library-v1-walk-east-strip.png"),
        walk_south: include_bytes!("../../assets/creator/baked/knight/library-v1-walk-south-strip.png"),
        idle: include_bytes!("../../assets/creator/baked/knight/library-v1-idle-strip.png"),
        attack: include_bytes!("../../assets/creator/baked/knight/library-v1-attack-strip.png"),
        ways: &[],
    })
    .unwrap();
    let frames = 40usize;
    // Keys are (anticipation, wind-up, swing, strike, recovery); any rising set of them fits the strip.
    for (a, p, sw, i, rs) in [(0, 1, 2, 3, 4), (2, 5, 8, 9, 11), (4, 10, 16, 17, 22), (0, 0, 0, 0, 0), (10, 10, 20, 20, 30), (5, 20, 21, 22, 23), (0, 38, 38, 39, 39), (3, 3, 4, 38, 39)] {
        let mut r = record.clone();
        for (k, v) in [("anticipation_starts", a), ("windup_key", p), ("swing", sw), ("strike", i), ("recovery_starts", rs)] {
            r["attack_events"][k] = v.into();
        }
        let set = base.clone().with_animation(&r.to_string()).unwrap_or_else(|e| panic!("{:?}: {e}", (a, p, sw, i, rs)));
        let at = |since: u64| set.swing_index(since).unwrap_or_else(|| panic!("{:?}: tick {since}", (a, p, sw, i, rs)));
        let keys = format!("{:?}", (a, p, sw, i, rs));
        let path: Vec<usize> = (0..ATTACK_TICKS).map(at).collect();
        assert!(path.iter().all(|&f| f < frames), "{keys}: inside the strip {path:?}");
        assert!(path.windows(2).all(|w| w[0] <= w[1]), "{keys}: the clip never plays backwards {path:?}");
        assert_eq!((path[0], path[ATTACK_TICKS as usize - 1]), (a, frames - 1), "{keys}: starts at its anticipation, settles on the last frame");
        assert_eq!(at(WINDUP), i, "{keys}: the strike frame lands on the first strike tick");
        assert_eq!(at(WINDUP - 1), sw, "{keys}: the swing is the tick before");
        assert_eq!(at(WINDUP - 2), (sw.saturating_sub(1)).max(p), "{keys}: the coil holds before it");
        assert!(set.swing_index(ATTACK_TICKS).is_none() && set.swing(ATTACK_TICKS).is_none(), "{keys}: over");
    }
}

#[test]
fn the_drive_stamps_the_sets_own_frame_on_the_ground_it_declares() {
    use super::baked::{figure, for_creation};
    use crate::rules::creation::{Drive, Step};
    for role in [Role::Hero, Role::Monster] {
        let floor = Floor::new("Ada");
        let mut s = session();
        s.creation.role = role;
        s.can_drive = true;
        s.go(Step::Life, 0);
        s.action(Action::Confirm, 1);
        s.action(Action::Confirm, 2);
        let set = for_creation(&s.creation);
        let t = 60u64;
        // (what the hands did, the motion it draws, whether it faces left, ticks into a blow)
        let cases: [(&str, Box<dyn Fn(&mut Drive)>, Motion, Option<u64>); 6] = [
            ("idle", Box::new(|_| {}), Motion::Idle, None),
            ("east", Box::new(|d| { d.moving = true; d.walked = 0.1; }), Motion::Walk(Way::East), None),
            ("west", Box::new(|d| { d.moving = true; d.walked = 0.1; d.facing = -1; }), Motion::Walk(Way::East), None),
            ("south", Box::new(|d| { d.moving = true; d.vertical = true; d.walked = 0.1; }), Motion::Walk(Way::South), None),
            ("windup", Box::new(|d| d.attack_at = Some(t - 3)), Motion::Windup, Some(3)),
            // The last strike tick: the blow's own spark (the first five ticks) is gone by then.
            ("strike", Box::new(|d| d.attack_at = Some(t - 13)), Motion::Strike, Some(13)),
        ];
        for (name, hands, motion, since) in cases {
            let mut drive = Drive::new(floor.spawn, &s.creation.stats(), 0);
            hands(&mut drive);
            s.drive = Some(drive.clone());
            let canvas = drive::frame(&s, &floor, t, 384, 224);
            let mut want = figure(&s.creation, motion, t, drive.walked, since);
            if drive.facing < 0 {
                want = want.flip_h();
            }
            let (fx, fy) = arena::frame_px(&floor.run, 384, 224, Some(1), drive.x, drive.y);
            // A stand-in's own bob and lunge stay as slice 2 had them; a set with its own motion adds none
            // and draws by the anchor its record gives.
            let hop = if drive.moving && !set.carries_motion() { i32::from((drive.walked / 0.55) as u64 % 2 == 1) } else { 0 };
            let lunge = if motion == Motion::Strike && !set.carries_motion() { 2 * i32::from(drive.facing) } else { 0 };
            let (x0, y0) = (fx - want.w / 2 + lunge, fy - want.h - hop + set.drop(motion));
            let mut wrong = 0;
            for y in 0..want.h {
                for x in 0..want.w {
                    if let Some(c) = want.get(x, y) {
                        wrong += i32::from(canvas.get(x0 + x, y0 + y) != Some(c));
                    }
                }
            }
            assert_eq!(wrong, 0, "{role:?} {name}: the figure is stamped exactly where its anchor puts it");
        }
    }
}

#[test]
fn a_set_may_lack_clips_and_keeps_the_librarys_fallbacks() {
    use super::baked::Set;
    // The knight's files, with the record saying the set has only the east walk: the other strips
    // are the exporter's blank placeholders and must never be read as frames.
    let blank = {
        let mut out = Vec::new();
        image::DynamicImage::ImageRgba8(image::RgbaImage::new(64, 54))
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        &*Box::leak(out.into_boxed_slice())
    };
    let mut record: serde_json::Value = serde_json::from_str(include_str!("../../assets/creator/baked/knight/animation.json")).unwrap();
    for gone in ["idle", "walk-south", "attack", "walk-west"] {
        record["states"].as_object_mut().unwrap().remove(gone);
    }
    record["attack_events"] = serde_json::Value::Null;
    let record: &'static str = Box::leak(record.to_string().into_boxed_str());
    let set = Baked::from_set(Set {
        import: include_str!("../../assets/creator/baked/knight/library-import.json"),
        animation: record,
        rest: include_bytes!("../../assets/creator/baked/knight/library-v1-rest.png"),
        windup: include_bytes!("../../assets/creator/baked/knight/library-v1-windup.png"),
        strike: include_bytes!("../../assets/creator/baked/knight/library-v1-strike.png"),
        walk_east: include_bytes!("../../assets/creator/baked/knight/library-v1-walk-east-strip.png"),
        walk_south: blank,
        idle: blank,
        attack: blank,
        ways: &[],
    })
    .expect("a set with only the east walk");
    // The east walk is the set's own; every other clip falls back to what the boss library does.
    assert!(set.swing(0).is_none() && set.swing(crate::rules::creation::drive::WINDUP).is_none(), "no smash: the blow is the windup and strike poses");
    let idle: std::collections::BTreeSet<u64> = (0..240).map(|t| set.frame(Motion::Idle, t, 0.0).content_hash()).collect();
    assert_eq!(idle.len(), 2, "the library's breathing rule: rest and breath");
    let east: std::collections::BTreeSet<u64> = (0..120).map(|i| set.frame(Motion::Walk(Way::East), 0, i as f32 * 0.1).content_hash()).collect();
    assert!(east.len() >= 10, "the east walk is the knight's");
    let south: std::collections::BTreeSet<u64> = (0..120).map(|i| set.frame(Motion::Walk(Way::South), 0, i as f32 * 0.1).content_hash()).collect();
    assert_eq!(south.len(), 2, "no front walk: the idle bob at a stride");
    assert_eq!(set.drop(Motion::Walk(Way::South)), 0, "and no anchor for a state it lacks");
    assert!(set.carries_motion(), "it has an animation record, so the drive adds no bob or lunge of its own");
    // A blank clip placeholder that the record does list is refused, not drawn.
    let mut lists_all: serde_json::Value = serde_json::from_str(include_str!("../../assets/creator/baked/knight/animation.json")).unwrap();
    lists_all["states"]["attack"] = lists_all["states"]["idle"].clone();
    let lists_all: &'static str = Box::leak(lists_all.to_string().into_boxed_str());
    let refused = Baked::from_set(Set {
        import: include_str!("../../assets/creator/baked/knight/library-import.json"),
        animation: lists_all,
        rest: include_bytes!("../../assets/creator/baked/knight/library-v1-rest.png"),
        windup: include_bytes!("../../assets/creator/baked/knight/library-v1-windup.png"),
        strike: include_bytes!("../../assets/creator/baked/knight/library-v1-strike.png"),
        walk_east: include_bytes!("../../assets/creator/baked/knight/library-v1-walk-east-strip.png"),
        walk_south: include_bytes!("../../assets/creator/baked/knight/library-v1-walk-south-strip.png"),
        idle: include_bytes!("../../assets/creator/baked/knight/library-v1-idle-strip.png"),
        attack: blank,
        ways: &[],
    });
    assert!(refused.is_err(), "an attack state over one blank cell: its keys do not fit");
}

// ---- Slice 4: the two 3D knights beside the 2D one, as the Hero's looks ----

fn wearing(look: &str) -> Creation {
    Creation { look: Some(look.into()), ..hero() }
}

#[test]
fn every_look_is_served_by_a_baked_set_of_that_id_and_the_first_is_the_default() {
    use super::baked::{for_creation, for_look};
    assert_eq!(for_creation(&hero()).id, Role::Hero.looks()[0].id, "no look: the role's own row, the first look");
    for role in Role::ALL {
        for look in role.looks() {
            let c = Creation { role, look: Some(look.id.into()), ..Creation::default() };
            let set = for_creation(&c);
            assert_eq!(set.id, look.id, "{role:?} {}", look.name);
            assert_eq!(for_look(&c, Some(look.id)).id, look.id);
            // What the scroll says and what the stage engraves fit where they are written.
            assert!(look.name.chars().count() <= 14, "{}: the Look row's value", look.name);
            assert!((1..=12).contains(&set.name().chars().count()), "{}: engraved on the stage", set.name());
        }
    }
    // Looks of the three knights are three different pictures; another role never wears a Hero's look.
    let hashes: std::collections::BTreeSet<u64> = Role::Hero.looks().iter().map(|l| creature_preview(&wearing(l.id), 0, None).content_hash()).collect();
    assert_eq!(hashes.len(), 3);
    let stray = Creation { role: Role::Boss, look: Some("a6-crusader".into()), ..Creation::default() };
    assert_eq!(for_creation(&stray).id, "marsh-bailiff");
}

#[test]
fn the_two_pipeline_knights_load_draw_and_stand_where_their_records_say() {
    use super::baked::for_creation;
    use crate::rules::creation::drive::{ATTACK_TICKS, WINDUP};
    let plain = for_creation(&hero());
    for (id, name, clips) in [
        ("a6-crusader", "CRUSADER A6", &["idle", "walk", "run", "attack", "hit", "death", "cast"][..]),
        ("frog-knight", "FROG KNIGHT", &["idle", "walk", "attack", "hit", "death"][..]),
    ] {
        let set = for_creation(&wearing(id));
        assert_eq!((set.id.as_str(), set.name(), set.size), (id, name, (64, 54)));
        assert!(set.carries_motion() && set.eight_way(), "{id}: it came with its animation record and its eight ways");
        assert_eq!(set.way_clips(), clips, "{id}: the clips its record lists");
        // They draw: a figure of the boss library's weight class, and not the 2D knight.
        let rest = set.frame(Motion::Idle, 0, 0.0);
        let weight = solid(rest);
        // The A6 keeps the boss library's weight class. The frog from W4's run B3 is a lighter figure (37 rows in the cell, 884
        // pixels: its lane lists "B3 size" as the first thing still wrong), so its floor is lower; the test still catches a collapsed one.
        let floor = if id == "frog-knight" { 800 } else { 1100 };
        assert!((floor..2100).contains(&weight), "{id}: {weight} opaque pixels (Cinderjaw 1985, the Bailiff 1729, the 2D knight 1874)");
        assert_ne!(rest.content_hash(), plain.frame(Motion::Idle, 0, 0.0).content_hash());
        // The idle is the set's own loop (one frame per tick of its clip rate), not the library's breath.
        let idle: std::collections::BTreeSet<u64> = (0..1200).map(|t| set.frame(Motion::Idle, t, 0.0).content_hash()).collect();
        assert!(idle.len() >= 8, "{id}: {} distinct idle frames", idle.len());
        // Every picture the set can put on the stage or the drive keeps the 54-row ceiling in its 64x54 cell.
        let mut shown: Vec<&Img> = Vec::new();
        for way in Way::ALL {
            shown.extend((0..400).map(|t| set.frame_facing(Motion::Idle, way, t, 0.0)));
            shown.extend((0..400).map(|i| set.frame_facing(Motion::Walk(way), way, 0, i as f32 * 0.05)));
            shown.extend((0..400).map(|i| set.frame_facing(Motion::Run(way), way, 0, i as f32 * 0.05)));
            shown.extend((0..ATTACK_TICKS).map(|since| set.swing_facing(since, way).unwrap_or_else(|| panic!("{id}: tick {since} of the blow {way:?}"))));
            shown.extend((0..40).flat_map(|t| [Motion::Hit, Motion::Death, Motion::Cast].map(|m| set.frame_facing(m, way, t, 0.0))));
            shown.extend([Motion::Windup, Motion::Strike].map(|m| set.frame_facing(m, way, 0, 0.0)));
        }
        for f in shown {
            assert_eq!((f.w, f.h), (64, 54), "{id}");
            let (cols, rows) = extent(f);
            assert!(rows <= 54 && cols <= 64, "{id}: {cols}x{rows}");
        }
        // The smash: a rise of its own, the coil held, the strike landing on the first strike tick, and
        // the blow registered like the standing pose (the soles do not move when it starts).
        let (from, peak) = (set.swing_index(0).unwrap(), set.swing_index(WINDUP - 3).unwrap());
        assert!((0..WINDUP - 2).map(|s| set.swing(s).unwrap().content_hash()).collect::<std::collections::BTreeSet<_>>().len() > (peak - from).min(1), "{id}: the rise moves ({from} to {peak})");
        assert_eq!(set.swing(WINDUP).unwrap().content_hash(), set.frame(Motion::Strike, 0, 0.0).content_hash(), "{id}: strike on tick {WINDUP}");
        // The anchor holds: standing, the soles end on the library's row once the set's own anchor is applied.
        assert_eq!(soles(rest) + set.drop(Motion::Idle), 52, "{id}");
    }
}

#[test]
fn a_looks_name_is_on_the_stage_and_the_plate_and_its_keys_are_on_the_drive() {
    let mut s = session();
    s.go(Step::Paint, 0);
    s.row = 4;
    let lines = |s: &Session| {
        let (_, text) = said::read(|| frame(s, 20, HEIGHT, None));
        text.lines.iter().map(|l| l.text.clone()).collect::<Vec<_>>()
    };
    let floor = Floor::new("Ada");
    let drive_lines = |s: &Session| {
        let (_, text) = said::read(|| drive::frame(s, &floor, 3, 384, 224));
        for line in &text.lines {
            assert!(line.at.0 >= 0 && line.at.0 + text_width(&line.text) <= line.on.0, "{line:?}: inside what it is written on");
        }
        text.lines.iter().map(|l| l.text.clone()).collect::<Vec<_>>()
    };
    let mut stages = Vec::new();
    for (look, value, caption, plate) in [
        (None, "KNIGHT (2D)", "STAND-IN: KNIGHT", "HERO - KNIGHT STAND-IN"),
        (Some("a6-crusader"), "CRUSADER (A6)", "CRUSADER A6", "HERO - CRUSADER A6"),
        (Some("frog-knight"), "FROG KNIGHT", "FROG KNIGHT", "HERO - FROG KNIGHT"),
    ] {
        s.creation.look = look.map(Into::into);
        let text = lines(&s);
        assert!(text.iter().any(|l| l == "LOOK") && text.iter().any(|l| l == value), "the Look row: {text:?}");
        assert!(text.iter().any(|l| l == caption), "{caption}: {text:?}");
        let on_drive = drive_lines(&s);
        assert!(on_drive.iter().any(|l| l == plate), "{plate}: {on_drive:?}");
        assert!(on_drive.iter().any(|l| l.contains("1-3: LOOK")), "the number keys flip the looks: {on_drive:?}");
        stages.push(frame(&s, 20, HEIGHT, None).content_hash());
    }
    assert_eq!(stages.iter().collect::<std::collections::BTreeSet<_>>().len(), 3, "three looks, three stages");
    // A role with no looks has no Look row and no number keys.
    s.creation.role = Role::Monster;
    s.go(Step::Paint, 0);
    assert!(!lines(&s).iter().any(|l| l == "LOOK"));
    assert!(!drive_lines(&s).iter().any(|l| l.contains("LOOK")));
    assert!(drive_lines(&s).iter().any(|l| l.starts_with("ARROWS: MOVE")), "its keys line is as it was");
}

#[test]
fn the_paint_page_keeps_its_colour_chips_beside_the_fifth_row() {
    // The scroll's rows give up the room the chips need; the chips are the six pigments between the rows and the wish.
    let chips = |s: &Session| {
        let sheet = scroll(s, 40, SCROLL_H);
        let mut seen = std::collections::BTreeSet::new();
        for y in 108..124 {
            for x in 42..200 {
                seen.insert(sheet.get(x, y));
            }
        }
        seen.len()
    };
    let mut hero = session();
    hero.go(Step::Paint, 0);
    let mut monster = session();
    monster.creation.role = Role::Monster;
    monster.go(Step::Paint, 0);
    assert_eq!(hero.row_count(), 5);
    assert!(chips(&hero) >= 12, "a Hero's page: {} colours where the chips go", chips(&hero));
    assert!(chips(&monster) >= 12, "the four-row page: {}", chips(&monster));
}

// ---- Slice 5: eight ways, every clip, the two pipeline looks ----

/// A set from the pipeline's frame sets, with its record as exported (`ways: true`) or as an older
/// reader sees it (the `ways` block gone and the atlas not brought: the legacy files only).
fn a6_set(record: &'static str, ways: &'static [u8]) -> Result<Baked, String> {
    use super::baked::Set;
    Baked::from_set(Set {
        import: include_str!("../../assets/creator/baked/a6-crusader/library-import.json"),
        animation: record,
        rest: include_bytes!("../../assets/creator/baked/a6-crusader/library-v1-rest.png"),
        windup: include_bytes!("../../assets/creator/baked/a6-crusader/library-v1-windup.png"),
        strike: include_bytes!("../../assets/creator/baked/a6-crusader/library-v1-strike.png"),
        walk_east: include_bytes!("../../assets/creator/baked/a6-crusader/library-v1-walk-east-strip.png"),
        walk_south: include_bytes!("../../assets/creator/baked/a6-crusader/library-v1-walk-south-strip.png"),
        idle: include_bytes!("../../assets/creator/baked/a6-crusader/library-v1-idle-strip.png"),
        attack: include_bytes!("../../assets/creator/baked/a6-crusader/library-v1-attack-strip.png"),
        ways,
    })
}

const A6_RECORD: &str = include_str!("../../assets/creator/baked/a6-crusader/animation.json");
const A6_WAYS: &[u8] = include_bytes!("../../assets/creator/baked/a6-crusader/library-v1-ways.png");

/// A record with `edit` applied to it, leaked for the `'static` the loader takes.
fn a6_record_with(edit: impl FnOnce(&mut serde_json::Value)) -> &'static str {
    let mut record: serde_json::Value = serde_json::from_str(A6_RECORD).unwrap();
    edit(&mut record);
    Box::leak(record.to_string().into_boxed_str())
}

#[test]
fn an_eight_way_set_has_every_clip_in_every_direction() {
    use super::baked::for_creation;
    for (id, clips) in [
        ("a6-crusader", &["idle", "walk", "run", "attack", "hit", "death", "cast"][..]),
        ("frog-knight", &["idle", "walk", "attack", "hit", "death"][..]),
    ] {
        let set = for_creation(&wearing(id));
        assert!(set.eight_way(), "{id}");
        assert_eq!(set.way_clips(), clips, "{id}");
        for &clip in clips {
            let lengths: std::collections::BTreeSet<usize> = Way::ALL.iter().map(|&w| set.way_frames(clip, w).unwrap().len()).collect();
            assert_eq!(lengths.len(), 1, "{id} {clip}: one timeline for all eight directions");
            assert!(*lengths.first().unwrap() >= 4, "{id} {clip}");
        }
        // Eight directions are eight pictures: the stance, the first step, the blow's strike.
        for (what, at) in [("idle", 0), ("walk", 0), ("attack", 1)] {
            let seen: std::collections::BTreeSet<u64> = Way::ALL.iter().map(|&w| set.way_frames(what, w).unwrap()[at].content_hash()).collect();
            assert_eq!(seen.len(), 8, "{id} {what}: eight different pictures");
        }
        // A way is a way: toward us is the set's own rest pose, and the set's walk goes the way it is told.
        assert_eq!(set.frame(Motion::Idle, 0, 0.0).content_hash(), set.way_frames("idle", Way::South).unwrap()[0].content_hash());
        for way in Way::ALL {
            let went = set.frame_facing(Motion::Walk(way), Way::South, 0, 0.0);
            assert_eq!(went.content_hash(), set.way_frames("walk", way).unwrap()[0].content_hash(), "{id} {way:?}: a walk faces its own way whatever the caller says");
            assert_eq!(set.frame_facing(Motion::Idle, way, 0, 0.0).content_hash(), set.way_frames("idle", way).unwrap()[0].content_hash(), "{id} {way:?}");
        }
        // The set's soles meet the ground by the sole edge each direction declares.
        for way in Way::ALL {
            let rest = set.frame_facing(Motion::Idle, way, 0, 0.0);
            let off = soles(rest) + set.drop_facing(Motion::Idle, way) - 52;
            let allowed = if id == "a6-crusader" || way == Way::South { 0 } else { 2 };
            assert!(off.abs() <= allowed, "{id} {way:?}: the record puts the soles {off} rows off the library's row");
        }
    }
}

#[test]
fn a_clip_plays_where_the_set_has_it_and_the_nearest_where_not() {
    use super::baked::for_creation;
    let a6 = for_creation(&wearing("a6-crusader"));
    let frog = for_creation(&wearing("frog-knight"));
    let way = Way::NorthEast;
    // Run goes by distance and is its own picture; a set without one walks.
    let run: std::collections::BTreeSet<u64> = (0..200).map(|i| a6.frame_facing(Motion::Run(way), way, 0, i as f32 * 0.1).content_hash()).collect();
    assert_eq!(run.len(), 8, "the A6's run is eight frames");
    assert_eq!(frog.frame_facing(Motion::Run(way), way, 0, 3.3).content_hash(), frog.frame_facing(Motion::Walk(way), way, 0, 3.3).content_hash(), "no run clip: the walk");
    // A hit, a death and a cast play from the tick they begin and hold their last frame.
    for (m, clip) in [(Motion::Hit, "hit"), (Motion::Death, "death"), (Motion::Cast, "cast")] {
        let Some(frames) = a6.way_frames(clip, way) else { panic!("{clip}") };
        assert_eq!(a6.frame_facing(m, way, 0, 0.0).content_hash(), frames[0].content_hash(), "{clip} begins on its first frame");
        assert_eq!(a6.frame_facing(m, way, 5000, 0.0).content_hash(), frames.last().unwrap().content_hash(), "{clip} holds its last");
        let seen: std::collections::BTreeSet<u64> = (0..100).map(|t| a6.frame_facing(m, way, t, 0.0).content_hash()).collect();
        assert!(seen.len() >= 3, "{clip} moves: {} pictures", seen.len());
    }
    // A set without a cast casts as the library's windup does; a hit it takes standing.
    assert!(frog.way_frames("cast", way).is_none());
    assert_eq!(frog.frame_facing(Motion::Cast, way, 3, 0.0).content_hash(), frog.frame_facing(Motion::Windup, way, 0, 0.0).content_hash());
    assert_eq!(frog.way_frames("hit", way).unwrap().len(), 4);
    // The sole edge a state declares is the one it is drawn by: a run and a walk of one way may differ.
    for way in Way::ALL {
        for m in [Motion::Run(way), Motion::Hit, Motion::Death, Motion::Cast] {
            let off = soles(a6.frame_facing(m, way, 0, 0.0)) + a6.drop_facing(m, way);
            assert!((40..=56).contains(&off), "{way:?} {m:?}: soles on row {off}");
        }
    }
}

#[test]
fn a_set_with_two_ways_answers_every_way_as_it_always_did() {
    use super::baked::for_creation;
    let monster = Creation { role: Role::Monster, ..Creation::default() };
    for c in [hero(), monster] {
        let set = for_creation(&c);
        let id = set.id.as_str();
        assert!(!set.eight_way() && set.way_clips().is_empty() && set.way_frames("idle", Way::South).is_none(), "{id}");
        for t in [0, 17, 40, 333, 5000] {
            assert_eq!(set.turntable(t), Way::South, "{id}: the stage does not turn it");
            assert_eq!(creature_preview(&c, t, None).content_hash(), super::baked::figure(&c, Motion::Idle, t, 0.0, None).content_hash(), "{id}: the stage shows what it always showed");
        }
        for way in Way::ALL {
            // North and south are the front walk, the rest the east walk (the caller turns it about for west).
            let (flat, _) = way.flat();
            for walked in [0.0, 0.7, 3.1, 9.9] {
                for m in [Motion::Walk(way), Motion::Run(way)] {
                    assert_eq!(set.frame_facing(m, way, 0, walked).content_hash(), set.frame(Motion::Walk(flat), 0, walked).content_hash(), "{id} {way:?} {m:?}");
                }
            }
            assert_eq!(set.drop_facing(Motion::Walk(way), way), set.drop(Motion::Walk(flat)), "{id} {way:?}");
            // The facing the caller names changes nothing for the other motions.
            for m in [Motion::Idle, Motion::Windup, Motion::Strike] {
                assert_eq!(set.frame_facing(m, way, 9, 0.0).content_hash(), set.frame(m, 9, 0.0).content_hash(), "{id} {way:?} {m:?}");
                assert_eq!(set.drop_facing(m, way), set.drop(m));
            }
            assert_eq!(set.swing_facing(9, way).map(Img::content_hash), set.swing(9).map(Img::content_hash));
        }
        // A clip it does not have is the nearest it does: a hit or a fall the stance, a cast the windup.
        for t in [0, 11, 100] {
            assert_eq!(set.frame(Motion::Hit, t, 0.0).content_hash(), set.frame(Motion::Idle, t, 0.0).content_hash(), "{id}");
            assert_eq!(set.frame(Motion::Death, t, 0.0).content_hash(), set.frame(Motion::Idle, t, 0.0).content_hash(), "{id}");
        }
        assert_eq!(set.frame(Motion::Cast, 0, 0.0).content_hash(), set.frame(Motion::Windup, 0, 0.0).content_hash(), "{id}");
    }
    // A pipeline set read by a reader that knows only the old two ways (its `ways` block gone, its atlas
    // not brought) is a two-way set, and its east and south are the eight-way set's own.
    let eight = for_creation(&wearing("a6-crusader"));
    let old = a6_set(a6_record_with(|r| { r.as_object_mut().unwrap().remove("ways"); }), &[]).expect("the legacy files alone are a set");
    assert!(!old.eight_way() && old.way_clips().is_empty());
    for t in (0..60).step_by(7) {
        assert_eq!(old.frame(Motion::Idle, t, 0.0).content_hash(), eight.frame_facing(Motion::Idle, Way::South, t, 0.0).content_hash(), "idle {t}");
    }
    for walked in [0.0, 1.3, 4.4, 8.0] {
        assert_eq!(old.frame(Motion::Walk(Way::East), 0, walked).content_hash(), eight.frame_facing(Motion::Walk(Way::East), Way::East, 0, walked).content_hash(), "east {walked}");
        assert_eq!(old.frame(Motion::Walk(Way::South), 0, walked).content_hash(), eight.frame_facing(Motion::Walk(Way::South), Way::South, 0, walked).content_hash(), "south {walked}");
    }
    for since in 0..crate::rules::creation::drive::ATTACK_TICKS {
        assert_eq!(old.swing(since).unwrap().content_hash(), eight.swing_facing(since, Way::South).unwrap().content_hash(), "blow {since}");
    }
    assert_eq!(old.drop(Motion::Idle), eight.drop(Motion::Idle));
}

#[test]
fn a_bad_ways_record_is_refused_rather_than_played_crooked() {
    assert!(a6_set(A6_RECORD, A6_WAYS).is_ok());
    let refused = |name: &str, record: &'static str, ways: &'static [u8]| {
        let err = a6_set(record, ways).err().unwrap_or_else(|| panic!("{name}: loaded"));
        assert!(err.contains("ways") || err.contains("animation"), "{name}: {err}");
    };
    refused("record without its atlas", A6_RECORD, &[]);
    refused("atlas without its record", a6_record_with(|r| { r.as_object_mut().unwrap().remove("ways"); }), A6_WAYS);
    refused("seven directions", a6_record_with(|r| { r["ways"]["directions"].as_array_mut().unwrap().pop(); }), A6_WAYS);
    refused("a direction twice", a6_record_with(|r| r["ways"]["directions"][1] = "s".into()), A6_WAYS);
    refused("a direction nobody knows", a6_record_with(|r| r["ways"]["directions"][1] = "up".into()), A6_WAYS);
    refused("a sole edge missing", a6_record_with(|r| { r["ways"]["clips"]["walk"]["anchor_y"].as_object_mut().unwrap().remove("ne"); }), A6_WAYS);
    refused("a clip nobody plays", a6_record_with(|r| { let c = r["ways"]["clips"]["walk"].clone(); r["ways"]["clips"]["dance"] = c; }), A6_WAYS);
    refused("more frames than the atlas holds", a6_record_with(|r| r["ways"]["clips"]["walk"]["frames"] = 99.into()), A6_WAYS);
    refused("a block off the atlas", a6_record_with(|r| r["ways"]["clips"]["cast"]["row"] = 500.into()), A6_WAYS);
    refused("an event past the clip", a6_record_with(|r| r["ways"]["clips"]["walk"]["events"]["step_r"] = 8.into()), A6_WAYS);
    refused("no idle", a6_record_with(|r| { r["ways"]["clips"].as_object_mut().unwrap().remove("idle"); }), A6_WAYS);
    refused("cells of another size", a6_record_with(|r| r["ways"]["cell"] = serde_json::json!([32, 54])), A6_WAYS);
    // The record lists a clip over cells with nothing in them (the walk's block, one row down, ends in the next clip's rows;
    // the idle's last frames, past the 16 it has, are empty).
    refused("a blank cell", a6_record_with(|r| r["ways"]["clips"]["attack"]["frames"] = 16.into()), A6_WAYS);
    refused("a smash that is not the strip's", a6_record_with(|r| r["ways"]["clips"]["attack"]["frames"] = 5.into()), A6_WAYS);
}

#[test]
fn the_stage_turns_an_eight_way_set_through_its_ways_and_holds_a_two_way_set_still() {
    use super::baked::{TURN_TICKS, figure_facing, for_creation};
    let c = wearing("a6-crusader");
    let set = for_creation(&c);
    // Round once, each way for one pass of the idle loop, south first and south again at the end.
    let order: Vec<Way> = (0..9).map(|k| set.turntable(k * TURN_TICKS)).collect();
    assert_eq!(order[..8], Way::ALL, "all eight, in the frame sets' order");
    assert_eq!(order[8], Way::South, "and round again");
    assert_eq!(set.turntable(TURN_TICKS - 1), Way::South, "held for the whole pass");
    assert_eq!(set.turntable(0), Way::South, "the stage opens as it always did: toward the camera");
    for k in 0..8u64 {
        let t = k * TURN_TICKS + 11;
        assert_eq!(creature_preview(&c, t, None).content_hash(), figure_facing(&c, Motion::Idle, Way::nth(k as usize), t, 0.0, None).content_hash(), "way {k}");
    }
    let turns: std::collections::BTreeSet<u64> = (0..8).map(|k| creature_preview(&c, k * TURN_TICKS, None).content_hash()).collect();
    assert_eq!(turns.len(), 8, "eight ways, eight pictures");
    // The whole stage: its figure is the preview's at the turn's way, stood on that way's sole edge.
    let mut s = session();
    s.creation = c.clone();
    s.go(Step::Body, 0);
    let (cx, foot) = (307, (HEIGHT - 76).max(91));
    for k in [0u64, 1, 2, 4, 6] {
        let t = k * TURN_TICKS + 5;
        let stage = frame(&s, t, HEIGHT, None);
        let figure = creature_preview(&c, t, None);
        let lower = set.drop_facing(Motion::Idle, set.turntable(t));
        let wrong = (0..figure.h)
            .flat_map(|y| (0..figure.w).map(move |x| (x, y)))
            .filter(|&(x, y)| figure.get(x, y).is_some() && stage.get(cx - figure.w / 2 + x, foot - figure.h + y + lower) != figure.get(x, y))
            .count();
        assert_eq!(wrong, 0, "way {k}: the stage draws the turn's figure where its sole edge puts it");
    }
    // A set with two ways never turns, and the stage with the calm viewer holds the way it opens on.
    let old = hero();
    let still: std::collections::BTreeSet<usize> = (0..400).map(|t| for_creation(&old).turntable(t).index()).collect();
    assert_eq!(still.len(), 1);
}

#[test]
fn the_test_drive_faces_where_the_creature_walks_in_eight_ways_and_two_way_art_is_as_it_was() {
    use super::baked::{Reel, figure_facing, for_creation};
    use crate::rules::creation::{Drive, Step};
    let floor = Floor::new("Ada");
    for (label, creation) in [("a6", wearing("a6-crusader")), ("frog", wearing("frog-knight")), ("knight", hero())] {
        let set = for_creation(&creation);
        let eight = set.eight_way();
        let mut s = session();
        s.creation = creation.clone();
        s.can_drive = true;
        s.go(Step::Life, 0);
        s.action(Action::Confirm, 1);
        s.action(Action::Confirm, 2);
        let t = 60u64;
        let mut standing = std::collections::BTreeSet::new();
        for way in Way::ALL {
            // (what the hands did: walking, running, ticks into a blow)
            let cases: [(&str, bool, bool, Option<u64>); 6] = [
                ("stands", false, false, None),
                ("walks", true, false, None),
                ("runs", true, true, None),
                ("windup", false, false, Some(3)),
                ("strike", false, false, Some(13)),
                ("recovers", false, false, Some(16)),
            ];
            for (what, moving, running, since) in cases {
                let mut drive = Drive::new(floor.spawn, &s.creation.stats(), 0);
                drive.way = way;
                drive.facing = if way.axes().0 < 0 { -1 } else { 1 };
                drive.vertical = way.axes().0 == 0;
                (drive.moving, drive.running, drive.walked) = (moving, running, 0.4);
                drive.attack_at = since.map(|since| t - since);
                s.drive = Some(drive.clone());
                let canvas = drive::frame(&s, &floor, t, 384, 224);
                // Eight ways face the way; two-way art keeps to east and south and turns about for the left.
                let m = drive_motion(&drive, t, eight);
                let blow = drive.attack_at.map(|at| t - at).filter(|_| !matches!(m, Motion::Walk(_) | Motion::Run(_)));
                let facing = if eight { way } else { Way::South };
                let mut want = figure_facing(&s.creation, m, facing, t - drive.born, drive.walked, blow);
                if drive.facing < 0 && !eight {
                    want = want.flip_h();
                }
                if eight {
                    // The motion itself says which way it goes.
                    match m {
                        Motion::Walk(w) | Motion::Run(w) => assert_eq!(w, way, "{label} {way:?} {what}"),
                        _ => {}
                    }
                }
                let (fx, fy) = arena::frame_px(&floor.run, 384, 224, Some(1), drive.x, drive.y);
                let own = set.carries_motion();
                let hop = if drive.moving && !own { i32::from((drive.walked / 0.55) as u64 % 2 == 1) } else { 0 };
                let lunge = if m == Motion::Strike && !own { 2 * i32::from(drive.facing) } else { 0 };
                // A set with clips of its own plays the blow's clip, which stands on the soles it declares.
                let drop = blow
                    .filter(|_| eight)
                    .and_then(|since| set.shot(Reel::Blow, since, facing, t - drive.born))
                    .map_or_else(|| set.drop_facing(m, facing), |p| p.drop);
                let (x0, y0) = (fx - want.w / 2 + lunge, fy - want.h - hop + drop);
                let wrong = (0..want.h).flat_map(|y| (0..want.w).map(move |x| (x, y))).filter(|&(x, y)| want.get(x, y).is_some() && canvas.get(x0 + x, y0 + y) != want.get(x, y)).count();
                assert_eq!(wrong, 0, "{label} {way:?} {what}: the figure is stamped where its anchor puts it");
                if what == "stands" {
                    standing.insert(canvas.content_hash());
                }
            }
        }
        assert_eq!(standing.len(), if eight { 8 } else { 2 }, "{label}: eight ways draw eight standing pictures; two-way art shows what it always did (right, and left turned about)");
    }
}

/// What `drive.rs`'s `motion` makes of a drive (a copy of its rule, so the test reads it from outside).
fn drive_motion(drive: &crate::rules::creation::Drive, tick: u64, eight: bool) -> Motion {
    use crate::rules::creation::Pose;
    match drive.pose(tick) {
        Pose::Windup => Motion::Windup,
        Pose::Strike => Motion::Strike,
        Pose::Rest if drive.moving => {
            let way = if eight { drive.way } else if drive.vertical { Way::South } else { Way::East };
            if drive.running { Motion::Run(way) } else { Motion::Walk(way) }
        }
        Pose::Rest => Motion::Idle,
    }
}

#[test]
fn the_registry_rows_and_the_hero_looks_hold_together() {
    use super::baked::{REGISTRY, for_creation, for_look};
    let ids: Vec<&str> = Role::Hero.looks().iter().map(|l| l.id).collect();
    assert_eq!(ids, ["knight", "a6-crusader", "frog-knight"]);
    let rows: Vec<&str> = REGISTRY.iter().filter_map(|r| r.look).collect();
    assert_eq!(rows, ids, "one row per look, in the Look row's order");
    assert!(REGISTRY.iter().all(|r| r.role == Some(Role::Hero)), "the rows are the Hero's");
    assert!(REGISTRY[0].look.is_none(), "the first row is the Hero's default");
    for row in REGISTRY {
        let set = (row.set)();
        assert_eq!(Some(set.id.as_str()), row.look.or(Some("knight")), "a row serves the set of its look's id");
        assert_eq!(set.size, (64, 54));
    }
    // The two pipeline looks are eight-way, the older knight and the stand-in are not, and the retired test looks are gone.
    for (id, eight, name) in [("knight", false, "KNIGHT"), ("a6-crusader", true, "CRUSADER A6"), ("frog-knight", true, "FROG KNIGHT")] {
        let set = for_creation(&wearing(id));
        assert_eq!((set.eight_way(), set.name()), (eight, name), "{id}");
    }
    assert!(!Baked::bailiff().eight_way());
    for gone in ["knight-trellis", "knight-spritekiln"] {
        assert_eq!(for_look(&hero(), Some(gone)).id, "knight", "{gone}: a recipe saved with a retired look wears the default");
    }
    // Both new sets carry the exporter's whole contract in their records.
    for id in ["a6-crusader", "frog-knight"] {
        let record: serde_json::Value = serde_json::from_str(match id {
            "a6-crusader" => include_str!("../../assets/creator/baked/a6-crusader/animation.json"),
            _ => include_str!("../../assets/creator/baked/frog-knight/animation.json"),
        })
        .unwrap();
        assert_eq!(record["creator"]["ways"], true, "{id}");
        assert_eq!(record["ways"]["directions"], serde_json::json!(["s", "sw", "w", "nw", "n", "ne", "e", "se"]), "{id}");
        assert_eq!(record["ways"]["cell"], serde_json::json!([64, 54]), "{id}");
        assert!(record["ways"]["clips"]["idle"].is_object() && record["states"]["idle"].is_object(), "{id}: the legacy idle is there too");
    }
}

/// Slice 5: the stage turning through its eight ways, the Test drive walking and running a circle and
/// striking in eight ways, and every clip of both pipeline looks in all eight directions, beside the 2D
/// knight as it was. Offline; run with `--ignored --exact`; the shots go to `target/creator-shots/slice5/`.
#[test]
#[ignore]
fn write_slice5_shots() {
    use super::baked::{TURN_TICKS, for_creation};
    use crate::rules::creation::{Drive, DriveInput, Ground};
    let dir = shots_dir("slice5");
    let looks: [(&str, Option<&str>); 3] = [("a6", Some("a6-crusader")), ("frog", Some("frog-knight")), ("knight2d", None)];
    let clean = |sub: &str| {
        let _ = std::fs::remove_dir_all(dir.join(sub));
    };
    // 1. The stage: the Paint page on its Look row, a whole turn at 12 fps; the stage cropped at native; one still per way at both widths.
    for (tag, look) in looks {
        let mut s = Session::default();
        s.open(0, "Ada");
        s.creation.look = look.map(Into::into);
        s.go(Step::Paint, 0);
        s.row = 4;
        let (full, crop) = (format!("stage-turn-{tag}-384"), format!("stage-crop-{tag}"));
        clean(&full);
        clean(&crop);
        let eight = for_creation(&s.creation).eight_way();
        let turn = if eight { 8 * TURN_TICKS } else { 118 };
        let (mut n, mut next) = (0, 20.0f64);
        for t in 20..20 + turn {
            if (t as f64) >= next {
                next += 2.5;
                let mut r = Run::new(41, 3, Some("Ada"));
                r.tick = t;
                save_png(&dir.join(&full), &format!("{n:03}.png"), &seat(&r, &s, 384, 286));
                let native = frame(&s, t, HEIGHT, None);
                let mut cut = Img::black(112, 96);
                for y in 0..96 {
                    for x in 0..112 {
                        if let Some(c) = native.get(251 + x, 66 + y) {
                            cut.set(x, y, c);
                        }
                    }
                }
                save_png(&dir.join(&crop), &format!("{n:03}.png"), &cut);
                n += 1;
            }
        }
        if eight {
            let mut r = Run::new(41, 3, Some("Ada"));
            for k in 0..8u64 {
                r.tick = k * TURN_TICKS + 20;
                for (w, h) in [(384, 286), (768, 572)] {
                    save_png(&dir, &format!("stage-way-{tag}-{}-{w}.png", Way::nth(k as usize).short()), &seat(&r, &s, w, h));
                }
            }
        }
    }
    // 2. The Test drive: a circle through all eight ways (walking, then running), and a blow facing each way.
    for (tag, look) in looks {
        let mut floor = Floor::new("Ada");
        let mut s = session();
        s.creation.look = look.map(Into::into);
        s.can_drive = true;
        s.go(Step::Life, 0);
        s.action(Action::Confirm, 1);
        s.action(Action::Confirm, 2);
        let (spawn, foes, friend) = (floor.spawn, floor.foes.clone(), floor.friend);
        let mut t = 3u64;
        let tick = |s: &mut Session, floor: &mut Floor, input: DriveInput, t: &mut u64| {
            let g = Ground { spawn, walkable: &|x, y| floor.walkable(x, y), foes: &foes, friend: Some(friend) };
            s.drive_step(input, *t, &g);
            floor.prepare(&s.drive.clone().unwrap(), *t);
            *t += 1;
        };
        tick(&mut s, &mut floor, DriveInput::default(), &mut t);
        let crop = |s: &Session, floor: &Floor, t: u64| {
            let canvas = drive::frame(s, floor, t, 384, 224);
            let d = s.drive.as_ref().unwrap();
            let (fx, fy) = arena::frame_px(&floor.run, 384, 224, Some(1), d.x, d.y);
            let mut out = Img::black(96, 76);
            for y in 0..76 {
                for x in 0..96 {
                    if let Some(c) = canvas.get(fx - 48 + x, fy - 68 + y) {
                        out.set(x, y, c);
                    }
                }
            }
            (out, canvas)
        };
        // Open floor for a whole circle: the spot where a walk through all eight ways and back comes home unobstructed,
        // furthest from the foes and the friend (a run's legs are as long as a walk's: 12 ticks at 1.6 times the pace).
        let circle_home = |at: (f32, f32), leg: u64| {
            let g = Ground { spawn, walkable: &|x, y| floor.walkable(x, y), foes: &[], friend: None };
            let mut d = Drive::new(at, &s.creation.stats(), 0);
            let (mut tk, mut stuck) = (0, false);
            for way in Way::ALL {
                let (dx, dy) = way.axes();
                for _ in 0..leg {
                    d.step(DriveInput { dx, dy, ..Default::default() }, tk, &g);
                    stuck |= !d.moving;
                    tk += 1;
                }
            }
            (!stuck && (d.x - at.0).abs() < 0.05 && (d.y - at.1).abs() < 0.05).then(|| {
                foes.iter().chain(std::iter::once(&friend)).map(|f| (f.0 - at.0).hypot(f.1 - at.1)).fold(f32::MAX, f32::min)
            })
        };
        let rows: Vec<String> = (8..46).map(|y| (0..64).map(|x| if floor.walkable(x as f32, y as f32) { '.' } else { '#' }).collect()).collect();
        eprintln!("{tag}: the floor (x 0..64 across, y 8..46 down)\n{}", rows.join("\n"));
        let (leg, start) = [20u64, 16, 12, 10, 8]
            .into_iter()
            .find_map(|leg| {
                (0..60)
                    .flat_map(|x| (11..45).map(move |y| (x as f32 + 0.5, y as f32 + 0.5)))
                    .filter(|&at| floor.walkable(at.0, at.1))
                    .filter_map(|at| circle_home(at, leg).map(|near| (near, at)))
                    .max_by(|a, b| a.0.total_cmp(&b.0))
                    .map(|(_, at)| (leg, at))
            })
            .expect("the fixture floor has room for a circle");
        eprintln!("{tag}: the circle starts at {start:?}, legs of {leg} ticks");
        {
            let d = s.drive.as_mut().unwrap();
            (d.x, d.y) = start;
        }
        for _ in 0..4 {
            tick(&mut s, &mut floor, DriveInput::default(), &mut t);
        }
        for (what, run, leg) in [("circle", false, leg), ("run", true, leg * 5 / 8)] {
            let (crops, fulls) = (format!("drive-{what}-{tag}"), format!("drive-{what}-full-{tag}-384"));
            clean(&crops);
            clean(&fulls);
            let (mut n, mut next) = (0, 0.0f64);
            let mut at = 0.0f64;
            let mut path = Vec::new();
            for way in Way::ALL {
                let (dx, dy) = way.axes();
                for _ in 0..leg {
                    tick(&mut s, &mut floor, DriveInput { dx, dy, run, ..Default::default() }, &mut t);
                    at += 1.0;
                    if at >= next {
                        next += 2.5;
                        let (c, full) = crop(&s, &floor, t - 1);
                        save_png(&dir.join(&crops), &format!("{n:03}.png"), &c);
                        save_png(&dir.join(&fulls), &format!("{n:03}.png"), &full);
                        n += 1;
                    }
                }
                let d = s.drive.as_ref().unwrap();
                path.push(format!("{} ({:.1},{:.1})", way.short(), d.x, d.y));
            }
            eprintln!("{tag} {what}: {}", path.join(" "));
        }
        // A blow facing each way: one step to turn, then the blow's twenty ticks and a few to settle.
        // Away from the foes and the friend, so the blow's spark is the only mark beside the creature.
        let clear = floor.standing((19.0, 22.0));
        {
            let d = s.drive.as_mut().unwrap();
            (d.x, d.y, d.attack_at, d.last_hit) = (clear.0, clear.1, None, None);
        }
        for _ in 0..4 {
            tick(&mut s, &mut floor, DriveInput::default(), &mut t);
        }
        let (blows, blows_full) = (format!("drive-blows-{tag}"), format!("drive-blows-full-{tag}-384"));
        clean(&blows);
        clean(&blows_full);
        let (mut n, mut next, mut at) = (0, 0.0f64, 0.0f64);
        for way in Way::ALL {
            let (dx, dy) = way.axes();
            tick(&mut s, &mut floor, DriveInput { dx, dy, ..Default::default() }, &mut t);
            tick(&mut s, &mut floor, DriveInput { attack: true, ..Default::default() }, &mut t);
            for i in 0..26 {
                tick(&mut s, &mut floor, DriveInput::default(), &mut t);
                at += 1.0;
                if at >= next {
                    next += 2.5;
                    let (c, full) = crop(&s, &floor, t - 1);
                    save_png(&dir.join(&blows), &format!("{n:03}.png"), &c);
                    save_png(&dir.join(&blows_full), &format!("{n:03}.png"), &full);
                    let _ = i;
                    n += 1;
                }
            }
        }
    }
    // 3. Every clip of the two pipeline looks, all eight ways at once (four by two, south first), at its own rate.
    let order = [Way::South, Way::SouthWest, Way::West, Way::NorthWest, Way::North, Way::NorthEast, Way::East, Way::SouthEast];
    for (tag, look) in &looks[..2] {
        let c = Creation { look: look.map(Into::into), ..Creation::default() };
        let set = for_creation(&c);
        for clip in set.way_clips() {
            let n = set.way_frames(clip, Way::South).unwrap().len();
            let sub = format!("clip-{tag}-{clip}");
            clean(&sub);
            for i in 0..n + 6 {
                let mut wall = Img::black(4 * 64 + 3 * 2, 2 * 54 + 2);
                for (k, way) in order.iter().enumerate() {
                    let frames = set.way_frames(clip, *way).unwrap();
                    wall.stamp(&frames[i.min(n - 1)], (k % 4) as i32 * 66, (k / 4) as i32 * 56);
                }
                save_png(&dir.join(&sub), &format!("{i:03}.png"), &wall);
            }
            // The same clip as one sheet: a row a direction, a column a frame, at x2.
            let mut sheet = Img::black(n as i32 * 64 + 2 * n as i32, 8 * 54 + 2 * 8);
            for (row, way) in order.iter().enumerate() {
                for (i, f) in set.way_frames(clip, *way).unwrap().iter().enumerate() {
                    sheet.stamp(f, i as i32 * 66, row as i32 * 56);
                }
            }
            let mut big = Img::black(sheet.w * 2, sheet.h * 2);
            big.stamp_times(&sheet, 0, 0, 2);
            save_png(&dir, &format!("clipsheet-{tag}-{clip}-x2.png"), &big);
        }
    }
}

// ---- Slice 6: the blow at the clip's own time, a cast, a foe that answers, a fall, a true caption ----

/// How many pixels of two same-sized figures differ (a colour, or filled against empty).
fn differ(a: &Img, b: &Img) -> usize {
    (0..a.h.max(b.h)).map(|y| (0..a.w.max(b.w)).filter(|&x| a.get(x, y) != b.get(x, y)).count()).sum()
}

/// A session at the Test drive for a look, its floor, and a way to step it a tick at a time.
struct Drove {
    s: Session,
    floor: Floor,
    t: u64,
    /// Drawn as the drive drew a set with eight ways before it played its clips at their own times (the fixed 8, 6, 6 blow).
    fixed: bool,
}

impl Drove {
    fn new(look: Option<&str>) -> Self {
        let floor = Floor::new("Ada");
        let mut s = session();
        s.creation.look = look.map(Into::into);
        s.can_drive = true;
        s.go(Step::Life, 0);
        s.action(Action::Confirm, 1);
        s.action(Action::Confirm, 2);
        let mut me = Self { s, floor, t: 3, fixed: false };
        me.tick(Default::default());
        me
    }

    /// The same, as the C2 drive timed and drew it.
    fn before(look: Option<&str>) -> Self {
        let mut me = Self::new(look);
        me.fixed = true;
        me
    }

    /// One tick of the hands, as the client steps it: the art says its timing, the session steps the
    /// drive, the floor is put as the drive has it.
    fn tick(&mut self, input: crate::rules::creation::DriveInput) -> Vec<crate::rules::creation::drive::Event> {
        use crate::rules::creation::Ground;
        self.s.timing = if self.fixed { crate::rules::creation::Timing::FIXED } else { timing(&self.s.creation) };
        let (spawn, foes, friend) = (self.floor.spawn, self.floor.foes.clone(), self.floor.friend);
        let g = Ground { spawn, walkable: &|x, y| self.floor.walkable(x, y), foes: &foes, friend: Some(friend) };
        let events = self.s.drive_step(input, self.t, &g);
        self.floor.prepare(&self.s.drive.clone().unwrap(), self.t);
        self.t += 1;
        events
    }

    fn now(&self) -> u64 {
        self.t - 1
    }

    fn drive(&self) -> &crate::rules::creation::Drive {
        self.s.drive.as_ref().unwrap()
    }

    fn frame(&self) -> Img {
        if self.fixed {
            drive::frame_fixed(&self.s, &self.floor, self.now(), 384, 224)
        } else {
            drive::frame(&self.s, &self.floor, self.now(), 384, 224)
        }
    }

    /// Stand the creature on open floor away from the foes and the friend.
    fn clear(&mut self) {
        let at = self.floor.standing((19.0, 22.0));
        let d = self.s.drive.as_mut().unwrap();
        (d.x, d.y, d.attack_at, d.last_hit) = (at.0, at.1, None, None);
        self.tick(Default::default());
    }

    /// Whether the canvas has this figure stamped where its anchor puts the creature, under the sprite's own pixels.
    fn stamped(&self, canvas: &Img, figure: &Img, drop: i32) -> usize {
        let d = self.drive();
        let (fx, fy) = arena::frame_px(&self.floor.run, 384, 224, Some(1), d.x, d.y);
        let (x0, y0) = (fx - figure.w / 2, fy - figure.h + drop);
        (0..figure.h).flat_map(|y| (0..figure.w).map(move |x| (x, y))).filter(|&(x, y)| figure.get(x, y).is_some() && canvas.get(x0 + x, y0 + y) != figure.get(x, y)).count()
    }
}

#[test]
fn a_pipeline_clip_is_played_at_its_own_frame_times_with_the_blow_on_its_impact_frame() {
    use super::baked::{Reel, for_creation};
    use crate::rules::creation::drive::SETTLE;
    for look in ["a6-crusader", "frog-knight"] {
        let set = for_creation(&wearing(look));
        let t = set.timing();
        let n = set.way_frames("attack", Way::South).unwrap().len();
        let impact = set.way_event("attack", "hit").unwrap();
        // 12 fps at 30 Hz: a frame every two and a half ticks, a frame showing from the first tick that reaches it.
        let first = |frame: usize| (frame as u64 * 30).div_ceil(12);
        assert_eq!(t.blow.windup, first(impact), "{look}: the blow lands the tick the impact frame (frame {impact}) first shows");
        assert_eq!(t.blow.clip(), first(n).max(t.blow.windup + t.blow.strike), "{look}: its spans are the clip's own length");
        assert_eq!((t.blow.settle, t.blow.strike >= 3), (SETTLE, true), "{look}");
        // Every other clip the set has is the clip's own length, and a set without one has no key for it.
        for (name, shot) in [("cast", t.cast), ("hit", t.hit), ("death", t.death)] {
            let frames = set.way_frames(name, Way::South).map(<[Img]>::len);
            assert_eq!(shot.map(|s| s.ticks), frames.map(|f| first(f)), "{look}: the {name} clip");
            assert_eq!(shot.map(|s| s.settle), frames.map(|_| SETTLE), "{look}: the {name} settle");
        }
        assert_eq!(t.cast.is_some(), look == "a6-crusader", "{look}: the A6 casts and the frog has no cast clip");
        if let Some(c) = t.cast {
            assert_eq!(c.event, first(set.way_event("cast", "cast").unwrap()), "{look}: a cast releases on the clip's own cast frame");
        }
        for way in [Way::South, Way::East, Way::North, Way::SouthWest] {
            let frames = set.way_frames("attack", way).unwrap();
            let shown: Vec<usize> = (0..t.blow.clip()).map(|since| (since * 12 / 30) as usize).map(|i| i.min(n - 1)).collect();
            for (since, &i) in shown.iter().enumerate() {
                let p = set.shot(Reel::Blow, since as u64, way, 0).unwrap();
                assert_eq!(p.frame.content_hash(), frames[i].content_hash(), "{look} {way:?}: tick {since} shows frame {i}");
            }
            assert_eq!(shown[t.blow.windup as usize], impact, "{look} {way:?}: the impact frame is on the landing tick");
            assert!(shown[t.blow.windup as usize - 1] < impact, "{look} {way:?}: and not before it");
            // Every frame of the clip is seen, none squeezed out, and none held for more than the clip's own two or three ticks
            // (the fixed blow held the first for eight and gave four frames six ticks).
            for f in 0..n.min(shown.len() * 12 / 30 + 1) {
                let held = shown.iter().filter(|&&i| i == f).count();
                assert!((2..=3).contains(&held) || f + 1 == n, "{look} {way:?}: frame {f} is on for {held} ticks");
            }
            assert_eq!(*shown.last().unwrap(), n - 1);
        }
    }
}

#[test]
fn a_blow_melts_into_the_stance_in_every_facing_and_nothing_snaps() {
    use super::baked::{Reel, for_creation};
    use crate::rules::creation::drive::SETTLE;
    for look in ["a6-crusader", "frog-knight"] {
        let creation = wearing(look);
        let set = for_creation(&creation);
        let total = set.timing().blow.ticks();
        let clip = set.timing().blow.clip();
        for way in Way::ALL {
            // What the creature shows, tick by tick, from the clip's last frame, through the settle, and on into the stance.
            let age0 = 200u64;
            let shown = |since: u64| -> Img {
                set.shot(Reel::Blow, since, way, age0 + since).map_or_else(|| set.frame_facing(Motion::Idle, way, age0 + since, 0.0).clone(), |p| p.frame.into_owned())
            };
            let steps: Vec<usize> = (clip - 1..total + 6).map(|s| differ(&shown(s), &shown(s + 1))).collect();
            let idle = |s: u64| set.frame_facing(Motion::Idle, way, age0 + s, 0.0);
            let beat = (0..set.way_frames("idle", way).unwrap().len() as u64 * 5 / 2).map(|k| differ(set.frame_facing(Motion::Idle, way, k, 0.0), set.frame_facing(Motion::Idle, way, k + 1, 0.0))).max().unwrap();
            // The old timeline cut from the clip's last frame to the stance in one tick.
            let cut = differ(&shown(clip - 1), idle(clip));
            eprintln!("{look} {way:?}: cut {cut}, the stance's own beat {beat}, steps {steps:?}");
            for (k, s) in (clip - 1..total).enumerate() {
                // A tick where the stance itself holds still changes only by the dissolve's share of the cut (at most two fifths of it); where the
                // stance turns to its next frame it may change as much as the stance's own loop does and one dissolve step more, and no more.
                let still = idle(s).content_hash() == idle(s + 1).content_hash();
                if still {
                    assert!(steps[k] * 5 <= cut * 2, "{look} {way:?}: tick {s} to {} changes {} of a {cut}-pixel cut (a snap is all of it): {steps:?}", s + 1, steps[k]);
                } else {
                    // The stance's own change, and one dissolve step's worth on top of it.
                    assert!(steps[k] <= beat + cut / 7, "{look} {way:?}: tick {s} to {} changes {} (the stance's own beat is {beat}, a cut {cut}): {steps:?}", s + 1, steps[k]);
                }
            }
            assert!(cut > beat / 2, "{look} {way:?}: there is a cut here to soften ({cut} against the stance's beat {beat})");
            // The settle ends on the stance itself, exactly, so nothing jumps when the clip lets go.
            assert_eq!(shown(total).content_hash(), set.frame_facing(Motion::Idle, way, age0 + total, 0.0).content_hash(), "{look} {way:?}");
        }
        assert_eq!(SETTLE, 6);
    }
}

#[test]
fn the_dissolve_makes_no_colour_and_only_ever_gives_ground() {
    use super::baked::dissolve;
    let mut a = Img::new(8, 8);
    let mut b = Img::new(8, 8);
    for y in 0..8 {
        for x in 0..8 {
            if x < 6 {
                a.set(x, y, [200, 10, 10]);
            }
            if x > 1 {
                b.set(x, y, [10, 10, 200]);
            }
        }
    }
    let mut last = 0;
    for step in 0..=8 {
        let share = step as f32 / 8.0;
        let mix = dissolve(&a, &b, share);
        let from_b = (0..8).flat_map(|y| (0..8).map(move |x| (x, y))).filter(|&(x, y)| mix.get(x, y) == b.get(x, y) && b.get(x, y).is_some() && a.get(x, y) != b.get(x, y)).count();
        assert!((0..8).all(|y| (0..8).all(|x| mix.get(x, y) == a.get(x, y) || mix.get(x, y) == b.get(x, y))), "every pixel is one frame's or the other's");
        assert!(from_b >= last, "a pixel that has shown the new frame keeps it ({last} then {from_b})");
        last = from_b;
    }
    assert_eq!(dissolve(&a, &b, 0.0), a);
    assert_eq!(dissolve(&a, &b, 1.0), b);
}

#[test]
fn the_2d_knight_and_the_stand_in_keep_the_fixed_timeline_to_the_tick() {
    use super::baked::{Reel, all_sets, for_creation};
    use crate::rules::creation::{Blow, Timing, drive};
    let sets = all_sets();
    for set in sets.iter().filter(|s| !s.eight_way()) {
        assert_eq!(set.timing(), Timing::FIXED, "{}", set.id);
        assert_eq!(set.timing().blow, Blow { windup: 8, strike: 6, recover: 6, settle: 0 });
        for reel in [Reel::Blow, Reel::Cast, Reel::Hit, Reel::Fall, Reel::Rise] {
            assert!(set.shot(reel, 3, Way::South, 0).is_none(), "{}: a set with no clips of its own plays none", set.id);
        }
        assert!(set.stand_in(), "{} is a real stand-in", set.id);
    }
    // The knight's smash is the frames it always played: windup, a held coil, the strike on tick 8 and a long recovery.
    let knight = for_creation(&hero());
    let index: Vec<usize> = (0..drive::ATTACK_TICKS).map(|since| knight.swing_index(since).unwrap()).collect();
    assert_eq!(index, [4, 5, 6, 8, 9, 10, 15, 16, 17, 18, 19, 19, 20, 21, 22, 25, 29, 32, 36, 39]);
    assert!(knight.swing_index(drive::ATTACK_TICKS).is_none());
    // And its drive: the same hands give the same pose on the same ticks whatever the art was told, and the cast, fall and
    // a foe in reach change nothing (no art, no clip).
    let mut plain = Drove::new(None);
    let mut keyed = Drove::new(None);
    plain.clear();
    keyed.clear();
    let beside = plain.floor.standing((plain.floor.foes[0].0 - 4.0, plain.floor.foes[0].1 + 1.0));
    for d in [&mut plain, &mut keyed] {
        let me = d.s.drive.as_mut().unwrap();
        (me.x, me.y) = beside;
    }
    for k in 0..200u64 {
        let attack = k == 3 || k == 60;
        plain.tick(crate::rules::creation::DriveInput { attack, ..Default::default() });
        let ev = keyed.tick(crate::rules::creation::DriveInput { attack, cast: k % 5 == 0, fall: k % 9 == 0, ..Default::default() });
        assert!(ev.iter().all(|e| !matches!(e, drive::Event::Cast | drive::Event::Hurt(_) | drive::Event::Fell | drive::Event::Rose)), "tick {k}: {ev:?}");
        assert_eq!(plain.drive().pose(plain.now()), keyed.drive().pose(keyed.now()), "tick {k}");
        assert_eq!(plain.frame(), keyed.frame(), "tick {k}: the same picture");
    }
    assert_eq!((keyed.drive().hurts, keyed.drive().act), (0, None));
}

#[test]
fn the_cast_key_plays_the_cast_clip_and_releases_on_its_cast_frame() {
    use super::baked::{Reel, figure_shot, for_creation};
    use crate::rules::creation::{Act, DriveInput, drive};
    let creation = wearing("a6-crusader");
    let set = for_creation(&creation);
    let cast = set.timing().cast.expect("the A6 casts");
    let mut d = Drove::new(Some("a6-crusader"));
    d.clear();
    let idle_hash = d.frame().content_hash();
    let ev = d.tick(DriveInput { cast: true, ..Default::default() });
    assert_eq!(ev, vec![drive::Event::Cast]);
    let began = d.now();
    assert_eq!(d.drive().acting(d.now()), Some((Act::Cast, 0)));
    let mut seen = Vec::new();
    for since in 0..cast.ticks + cast.settle + 3 {
        if since > 0 {
            d.tick(Default::default());
        }
        let frame = d.frame();
        let age = d.now() - d.drive().born;
        if let Some((figure, drop)) = figure_shot(&creation, Reel::Cast, since, d.drive().way, age) {
            // Before the release the creature is the clip's frame and nothing else; the ring comes with the cast frame.
            if since < cast.event {
                assert_eq!(d.stamped(&frame, &figure, drop), 0, "tick {since}: the cast clip's frame is where the creature stands");
            }
            seen.push((since, figure.content_hash()));
        } else {
            assert!(since >= cast.ticks + cast.settle, "tick {since}");
        }
    }
    assert!(seen.windows(2).filter(|p| p[0].1 != p[1].1).count() >= 5, "the clip moves: {seen:?}");
    let ring_free = Drove::new(Some("a6-crusader")).frame().content_hash();
    assert_ne!(idle_hash, ring_free, "(the floor differs from the cleared one)");
    // The release: a ring over the room, only while the clip is at its cast frame.
    let mut r = Drove::new(Some("a6-crusader"));
    r.clear();
    r.tick(DriveInput { cast: true, ..Default::default() });
    for _ in 0..cast.event {
        r.tick(Default::default());
    }
    let with = r.frame();
    for _ in 0..cast.ticks + cast.settle {
        r.tick(Default::default());
    }
    let after = r.frame();
    assert!(r.drive().acting(r.now()).is_none() && differ(&with, &after) > 20, "the ring is on the cast frame and gone after");
    let _ = began;
}

#[test]
fn a_foe_in_reach_strikes_back_on_a_timer_and_the_creature_plays_its_hit_clip() {
    use super::baked::{Reel, figure_shot};
    use crate::rules::creation::{Act, DriveInput, drive as rules};
    for (look, creation) in [("a6-crusader", wearing("a6-crusader")), ("frog-knight", wearing("frog-knight"))] {
        let mut d = Drove::new(Some(look));
        d.clear();
        let foe = d.floor.foes[0];
        let beside = d.floor.standing((foe.0 - 4.0, foe.1 + 1.0));
        {
            let me = d.s.drive.as_mut().unwrap();
            (me.x, me.y) = beside;
        }
        let mut telling = Vec::new();
        let mut hurt = None;
        let mut quiet_frame = None;
        for _ in 0..rules::FOE_EVERY + 4 {
            let ev = d.tick(DriveInput::default());
            if !d.drive().tells().is_empty() {
                // The same moment with the foe's tell taken away: what differs is the tell.
                let mut calm = d.s.clone();
                calm.drive.as_mut().unwrap().timing.hit = None;
                quiet_frame.get_or_insert_with(|| drive::frame(&calm, &d.floor, d.now(), 384, 224));
                telling.push(d.frame());
            }
            if let Some(h) = ev.iter().find_map(|e| if let rules::Event::Hurt(h) = e { Some(*h) } else { None }) {
                hurt = Some((h, d.now()));
                break;
            }
        }
        let (h, at) = hurt.unwrap_or_else(|| panic!("{look}: the foe struck back"));
        assert_eq!((h.foe, h.damage), (0, rules::FOE_DAMAGE), "{look}");
        assert!(telling.len() >= rules::FOE_TELL as usize - 2, "{look}: the foe winds up for {} ticks", telling.len());
        assert!(differ(&quiet_frame.unwrap(), &telling[0]) > 6, "{look}: the tell is drawn over the foe");
        assert_eq!(d.drive().acting(at), Some((Act::Hit, 0)));
        // The hit clip, at its own times, stamped where the creature stands; the damage rises over it.
        let hit = d.s.timing.hit.unwrap();
        let mut seen = Vec::new();
        for since in 0..hit.ticks + hit.settle {
            if since > 0 {
                d.tick(DriveInput::default());
            }
            let age = d.now() - d.drive().born;
            let (figure, drop) = figure_shot(&creation, Reel::Hit, since, d.drive().way, age).expect("the hit clip");
            assert_eq!(d.stamped(&d.frame(), &figure, drop), 0, "{look}: tick {since} of the flinch is the clip's frame");
            seen.push(figure.content_hash());
        }
        assert!(seen.windows(2).any(|p| p[0] != p[1]), "{look}: the flinch moves");
        let (_, text) = said::read(|| d.frame());
        assert!(text.lines.iter().any(|l| l.text.starts_with("ARMOUR") && l.text.contains("TOOK -")), "{look}: the plate says what it took");
        // It goes back to what it was doing: standing, and the foe winds up again from nothing.
        d.tick(DriveInput::default());
        assert_eq!(d.drive().acting(d.now()), None);
        assert_eq!(d.drive().hurts, 1);
    }
    // The 2D knight has no hit clip: no foe ever strikes it, and the keys line does not offer what it cannot do.
    let mut k = Drove::new(None);
    k.clear();
    let beside = k.floor.standing((k.floor.foes[0].0 - 4.0, k.floor.foes[0].1 + 1.0));
    {
        let me = k.s.drive.as_mut().unwrap();
        (me.x, me.y) = beside;
    }
    for _ in 0..rules::FOE_EVERY * 3 {
        k.tick(DriveInput::default());
        assert!(k.drive().tells().is_empty());
    }
    assert_eq!(k.drive().hurts, 0);
}

#[test]
fn the_fall_key_lays_the_creature_down_in_its_death_clip_and_a_second_press_gets_it_up() {
    use super::baked::{Reel, figure_shot};
    use crate::rules::creation::{Act, DriveInput, drive};
    for (look, creation) in [("a6-crusader", wearing("a6-crusader")), ("frog-knight", wearing("frog-knight"))] {
        let mut d = Drove::new(Some(look));
        d.clear();
        let idle = d.frame();
        let death = d.s.timing.death.unwrap();
        let ev = d.tick(DriveInput { fall: true, ..Default::default() });
        assert_eq!(ev, vec![drive::Event::Fell], "{look}");
        let mut frames = Vec::new();
        for since in 0..death.ticks + 40 {
            if since > 0 {
                d.tick(DriveInput { dx: 1, attack: true, ..Default::default() });
            }
            let age = d.now() - d.drive().born;
            let (figure, drop) = figure_shot(&creation, Reel::Fall, since, d.drive().way, age).expect("the death clip");
            assert_eq!(d.stamped(&d.frame(), &figure, drop), 0, "{look}: tick {since} of the fall is the clip's frame");
            frames.push(figure.content_hash());
        }
        let n = frames.len();
        assert!(frames[..death.ticks as usize].windows(2).filter(|p| p[0] != p[1]).count() >= 6, "{look}: it falls over several frames");
        assert!(frames[death.ticks as usize..].windows(2).all(|p| p[0] == p[1]), "{look}: and then lies still");
        assert!(d.drive().down(d.now()) && n as u64 > death.ticks);
        assert_ne!(d.frame(), idle);
        // It does not walk off while it is down. A second press gets it up: the clip backwards, then the stance.
        d.tick(DriveInput { fall: true, ..Default::default() });
        assert_eq!(d.drive().acting(d.now()), Some((Act::Rose, 0)));
        let mut rose = Vec::new();
        for since in 0..death.ticks + death.settle {
            if since > 0 {
                d.tick(Default::default());
            }
            let age = d.now() - d.drive().born;
            let (figure, drop) = figure_shot(&creation, Reel::Rise, since, d.drive().way, age).expect("the rise");
            assert_eq!(d.stamped(&d.frame(), &figure, drop), 0, "{look}: tick {since} of the rise");
            rose.push(figure.content_hash());
        }
        let lying = figure_shot(&creation, Reel::Fall, death.ticks, d.drive().way, 0).unwrap().0.content_hash();
        assert_eq!(rose[0], lying, "{look}: it gets up from where it lay");
        assert_ne!(rose[death.ticks as usize - 1], lying);
        d.tick(Default::default());
        assert!(d.drive().acting(d.now()).is_none() && !d.drive().down(d.now()), "{look}: up");
    }
}

#[test]
fn only_a_real_stand_in_is_captioned_one() {
    let mut s = session();
    s.go(Step::Paint, 0);
    s.row = 4;
    let lines = |s: &Session| {
        let (_, text) = said::read(|| frame(s, 20, HEIGHT, None));
        text.lines.iter().map(|l| l.text.clone()).collect::<Vec<_>>()
    };
    let floor = Floor::new("Ada");
    let drive_lines = |s: &Session| {
        let (_, text) = said::read(|| drive::frame(s, &floor, 3, 384, 224));
        text.lines.iter().map(|l| l.text.clone()).collect::<Vec<_>>()
    };
    for (look, caption, plate) in [(Some("a6-crusader"), "CRUSADER A6", "HERO - CRUSADER A6"), (Some("frog-knight"), "FROG KNIGHT", "HERO - FROG KNIGHT")] {
        s.creation.look = look.map(Into::into);
        let (stage, plate_lines) = (lines(&s), drive_lines(&s));
        assert!(stage.iter().any(|l| l == caption), "{caption}: {stage:?}");
        assert!(plate_lines.iter().any(|l| l == plate), "{plate}: {plate_lines:?}");
        assert!(stage.iter().chain(&plate_lines).all(|l| !l.contains("STAND-IN")), "a finished character is not a stand-in: {stage:?} {plate_lines:?}");
    }
    // The 2D knight and the Bailiff are what they were.
    s.creation.look = None;
    assert!(lines(&s).iter().any(|l| l == "STAND-IN: KNIGHT") && drive_lines(&s).iter().any(|l| l == "HERO - KNIGHT STAND-IN"));
    s.creation.role = Role::Monster;
    s.go(Step::Body, 0);
    assert!(lines(&s).iter().any(|l| l == "STAND-IN: BAILIFF"));
    assert!(drive_lines(&s).iter().any(|l| l.starts_with("MONSTER - BAILIFF")));
}

#[test]
fn the_cast_and_the_fall_are_offered_on_the_plate_only_where_the_art_has_the_clips() {
    let floor = Floor::new("Ada");
    let lines = |look: Option<&str>, role: Role| {
        let mut s = session();
        s.creation.role = role;
        s.creation.look = look.map(Into::into);
        let (_, text) = said::read(|| drive::frame(&s, &floor, 3, 384, 224));
        for line in &text.lines {
            assert!(line.at.0 >= 0 && line.at.0 + text_width(&line.text) <= line.on.0, "{line:?}: inside what it is written on");
        }
        text.lines.iter().map(|l| l.text.clone()).collect::<Vec<_>>()
    };
    let keys = |l: &[String]| l.iter().find(|l| l.starts_with("ARROWS")).cloned().unwrap();
    let has = |l: &[String], what: &str| l.iter().any(|l| l == what);
    // The bottom line is as it was for every creature.
    let (knight, bailiff) = (lines(None, Role::Hero), lines(None, Role::Monster));
    assert_eq!(keys(&knight), "ARROWS: MOVE  F: STRIKE  E: MEET  1-3: LOOK  Q: BACK", "the 2D knight's line is as it was");
    assert_eq!(keys(&bailiff), "ARROWS: MOVE   F: STRIKE   E: MEET   Q: BACK", "and the Bailiff's");
    let (a6, frog) = (lines(Some("a6-crusader"), Role::Hero), lines(Some("frog-knight"), Role::Hero));
    assert_eq!(keys(&a6), keys(&knight));
    assert_eq!(keys(&frog), keys(&knight));
    // The plate lists R and X where the art has a cast and a death clip.
    assert!(has(&a6, "R: CAST   X: FALL"), "{a6:?}");
    assert!(has(&frog, "X: FALL"), "the frog has no cast clip: {frog:?}");
    for l in [&knight, &bailiff] {
        assert!(!l.iter().any(|l| l.contains("CAST") || l.contains("FALL")), "no clip, no key: {l:?}");
    }
}

/// Slice 6: the Test drive's blows at the clip's own time in four facings (beside the library's fixed
/// timeline they replace), a cast, a foe that strikes back, a fall and a rise, and the captions. Offline;
/// run with `--ignored --exact`; the shots go to `target/creator-shots/slice6/` (a `steps.json` of every
/// tick's pixel change beside them).
#[test]
#[ignore]
fn write_slice6_shots() {
    use super::baked::{Reel, for_creation};
    use crate::rules::creation::{DriveInput, drive as rules};
    let dir = shots_dir("slice6");
    let clean = |sub: &str| {
        let _ = std::fs::remove_dir_all(dir.join(sub));
    };
    // A window onto the drive's canvas: `w` by `h` native pixels, centred on `mid`, a little above the point.
    let window = |canvas: &Img, mid: (i32, i32), w: i32, h: i32| {
        let mut out = Img::black(w, h);
        for y in 0..h {
            for x in 0..w {
                if let Some(c) = canvas.get(mid.0 - w / 2 + x, mid.1 - (h - 14) + y) {
                    out.set(x, y, c);
                }
            }
        }
        out
    };
    let feet = |d: &Drove| {
        let me = d.drive();
        arena::frame_px(&d.floor.run, 384, 224, Some(1), me.x, me.y)
    };
    // Twelve frames a second of a 30 Hz run of ticks: the shots keep every tick, the sheets are 12 fps.
    let sample = |n: usize| -> Vec<usize> { (0..).map(|k| (k as f64 * 2.5) as usize).take_while(|&i| i < n).collect() };
    let mut steps = serde_json::Map::new();
    let looks = [("a6", "a6-crusader"), ("frog", "frog-knight")];
    let facings = [Way::South, Way::East, Way::North, Way::SouthWest];
    // 1. The blow in four facings, every tick, and the fixed timeline it replaces drawn the same way.
    for (tag, look) in looks {
        let creation = wearing(look);
        let set = for_creation(&creation);
        let total = set.timing().blow.ticks();
        for way in facings {
            let mut d = Drove::new(Some(look));
            d.clear();
            let (dx, dy) = way.axes();
            d.tick(DriveInput { dx, dy, ..Default::default() });
            for _ in 0..6 {
                d.tick(Default::default());
            }
            d.tick(DriveInput { attack: true, ..Default::default() });
            let mut all = vec![window(&d.frame(), feet(&d), 112, 80)];
            for _ in 1..total + 8 {
                d.tick(Default::default());
                all.push(window(&d.frame(), feet(&d), 112, 80));
            }
            let name = format!("blow-{tag}-{}", way.short());
            clean(&name);
            clean(&format!("blowticks-{tag}-{}", way.short()));
            for i in sample(all.len()) {
                save_png(&dir.join(&name), &format!("{i:03}.png"), &all[i]);
            }
            // Every tick, for the sheet that shows nothing was skipped.
            for (i, f) in all.iter().enumerate() {
                save_png(&dir.join(format!("blowticks-{tag}-{}", way.short())), &format!("{i:03}.png"), f);
            }
            // The numbers: how many pixels each tick changes the creature by, the new blow against the fixed timeline it replaced.
            let age0 = 100u64;
            let new: Vec<Img> = (0..total + 8)
                .map(|s| set.shot(Reel::Blow, s, way, age0 + s).map_or_else(|| set.frame_facing(Motion::Idle, way, age0 + s, 0.0).clone(), |p| p.frame.into_owned()))
                .collect();
            // The fixed timeline: the library's smash for twenty ticks, then the stance (the old drive's cut).
            let old: Vec<Img> = (0..total + 8).map(|s| set.swing_facing(s, way).map_or_else(|| set.frame_facing(Motion::Idle, way, age0 + s, 0.0).clone(), Img::clone)).collect();
            let delta = |v: &[Img]| v.windows(2).map(|p| differ(&p[0], &p[1])).collect::<Vec<_>>();
            let beat = (0..40u64).map(|k| differ(set.frame_facing(Motion::Idle, way, k, 0.0), set.frame_facing(Motion::Idle, way, k + 1, 0.0))).max().unwrap();
            steps.insert(
                format!("{tag}-{}", way.short()),
                serde_json::json!({
                    "windup": set.timing().blow.windup, "clip": set.timing().blow.clip(), "total": total,
                    "new": delta(&new), "old": delta(&old), "idle_beat": beat,
                    "old_last_vs_idle": differ(&old[19], set.frame_facing(Motion::Idle, way, age0 + 20, 0.0)),
                    "new_last_vs_idle": differ(&new[set.timing().blow.clip() as usize - 1], set.frame_facing(Motion::Idle, way, age0 + set.timing().blow.clip(), 0.0)),
                }),
            );
            // The same blow as the C2 drive timed and drew it, in the same room: the fixed 8, 6 and 6 ticks over the clip, the cut to the stance.
            let mut b = Drove::before(Some(look));
            b.clear();
            b.tick(DriveInput { dx, dy, ..Default::default() });
            for _ in 0..6 {
                b.tick(Default::default());
            }
            b.tick(DriveInput { attack: true, ..Default::default() });
            let mut before = vec![window(&b.frame(), feet(&b), 112, 80)];
            for _ in 1..total + 8 {
                b.tick(Default::default());
                before.push(window(&b.frame(), feet(&b), 112, 80));
            }
            clean(&format!("blow-before-{tag}-{}", way.short()));
            for i in sample(before.len()) {
                save_png(&dir.join(format!("blow-before-{tag}-{}", way.short())), &format!("{i:03}.png"), &before[i]);
            }
        }
    }
    std::fs::write(dir.join("steps.json"), serde_json::to_string_pretty(&steps).unwrap()).unwrap();
    // 2. A cast, facing south and east (A6 only: the frog has no cast clip).
    for way in [Way::South, Way::East] {
        let mut d = Drove::new(Some("a6-crusader"));
        d.clear();
        let (dx, dy) = way.axes();
        d.tick(DriveInput { dx, dy, ..Default::default() });
        for _ in 0..6 {
            d.tick(Default::default());
        }
        let cast = d.s.timing.cast.unwrap();
        let mut all = vec![];
        d.tick(DriveInput { cast: true, ..Default::default() });
        all.push(window(&d.frame(), feet(&d), 128, 88));
        for _ in 1..cast.ticks + cast.settle + 8 {
            d.tick(Default::default());
            all.push(window(&d.frame(), feet(&d), 128, 88));
        }
        let name = format!("cast-a6-{}", way.short());
        clean(&name);
        for i in sample(all.len()) {
            save_png(&dir.join(&name), &format!("{i:03}.png"), &all[i]);
        }
    }
    // 3. A foe that strikes back: the creature beside the first foe, the tell, the blow and the flinch (both pipeline looks).
    for (tag, look) in looks {
        let mut d = Drove::new(Some(look));
        d.clear();
        let foe = d.floor.foes[0];
        let beside = d.floor.standing((foe.0 - 4.0, foe.1 + 1.0));
        {
            let me = d.s.drive.as_mut().unwrap();
            (me.x, me.y) = beside;
        }
        let hit = d.s.timing.hit.unwrap();
        let (mut all, mut full) = (Vec::new(), Vec::new());
        let mut landed = None;
        for _ in 0..rules::FOE_EVERY * 2 {
            let ev = d.tick(Default::default());
            if landed.is_none() && ev.iter().any(|e| matches!(e, rules::Event::Hurt(_))) {
                landed = Some(d.now());
            }
            if let Some(at) = landed {
                let canvas = d.frame();
                let (fx, fy) = feet(&d);
                let (px, py) = arena::frame_px(&d.floor.run, 384, 224, Some(1), foe.0, foe.1);
                all.push(window(&canvas, ((fx + px) / 2, (fy + py) / 2 + 6), 144, 96));
                full.push(canvas);
                if d.now() > at + hit.ticks + hit.settle + 14 {
                    break;
                }
            } else if d.now() + 22 >= d.drive().born + rules::FOE_EVERY {
                // The tell: the last ticks before the blow.
                let canvas = d.frame();
                let (fx, fy) = feet(&d);
                let (px, py) = arena::frame_px(&d.floor.run, 384, 224, Some(1), foe.0, foe.1);
                all.push(window(&canvas, ((fx + px) / 2, (fy + py) / 2 + 6), 144, 96));
                full.push(canvas);
            }
        }
        let name = format!("hit-{tag}");
        clean(&name);
        clean(&format!("hit-full-{tag}-384"));
        for i in sample(all.len()) {
            save_png(&dir.join(&name), &format!("{i:03}.png"), &all[i]);
            save_png(&dir.join(format!("hit-full-{tag}-384")), &format!("{i:03}.png"), &full[i]);
        }
    }
    // 4. A fall and the rise from it: the clip, a few seconds lying there, the clip backwards, the stance.
    for (tag, look) in looks {
        let mut d = Drove::new(Some(look));
        d.clear();
        let death = d.s.timing.death.unwrap();
        let mut all = vec![];
        for _ in 0..6 {
            d.tick(Default::default());
            all.push(window(&d.frame(), feet(&d), 112, 80));
        }
        d.tick(DriveInput { fall: true, ..Default::default() });
        all.push(window(&d.frame(), feet(&d), 112, 80));
        for _ in 0..death.ticks + 30 {
            d.tick(Default::default());
            all.push(window(&d.frame(), feet(&d), 112, 80));
        }
        d.tick(DriveInput { fall: true, ..Default::default() });
        all.push(window(&d.frame(), feet(&d), 112, 80));
        for _ in 0..death.ticks + death.settle + 14 {
            d.tick(Default::default());
            all.push(window(&d.frame(), feet(&d), 112, 80));
        }
        let name = format!("death-{tag}");
        clean(&name);
        for i in sample(all.len()) {
            save_png(&dir.join(&name), &format!("{i:03}.png"), &all[i]);
        }
    }
    // 5. The captions: the Paint page's stage and the drive's plate and keys, each look, at 384 and 768.
    let floor = Floor::new("Ada");
    for (tag, look) in [("a6", Some("a6-crusader")), ("frog", Some("frog-knight")), ("knight2d", None)] {
        let mut s = session();
        s.creation.role = Role::Hero;
        s.creation.look = look.map(Into::into);
        s.can_drive = true;
        s.go(Step::Paint, 0);
        s.row = 4;
        let mut r = Run::new(41, 3, Some("Ada"));
        r.tick = 20;
        for (w, h) in [(384, 286), (768, 572)] {
            save_png(&dir, &format!("caption-stage-{tag}-{w}.png"), &seat(&r, &s, w, h));
        }
        s.go(Step::Life, 0);
        s.action(Action::Confirm, 1);
        s.action(Action::Confirm, 2);
        s.timing = timing(&s.creation);
        let mut canvas = drive::frame(&s, &floor, 3, 384, 224);
        save_png(&dir, &format!("caption-drive-{tag}-384.png"), &canvas);
        let mut big = Img::black(768, 448);
        big.stamp_times(&canvas, 0, 0, 2);
        canvas = big;
        save_png(&dir, &format!("caption-drive-{tag}-768.png"), &canvas);
    }
}

/// The hub's rows: each opens or greys as the gate says, in the painter's own words.
#[test]
fn the_hub_rows_open_and_grey_as_the_gate_says() {
    let shut = WishGate::default();
    let held = WishGate { power: true, made: 2, wishes: 2, ..shut };
    for row in HubRow::ALL {
        assert_eq!(shut.open(row), matches!(row, HubRow::Creature | HubRow::Person | HubRow::Weapon));
        assert_eq!(held.open(row), matches!(row, HubRow::Power | HubRow::Creature | HubRow::Person | HubRow::Weapon));
    }
    assert!(shut.open(HubRow::Creature) && shut.open(HubRow::Person) && shut.open(HubRow::Weapon));
    assert!(!shut.open(HubRow::Power) && !shut.open(HubRow::Reforge) && !shut.open(HubRow::Look));
    assert_eq!(HubRow::Power.grey(shut), "NO WISH HELD");
    assert_eq!(HubRow::Look.grey(shut), "ONLY AT THE MIRROR");
    let full = WishGate { made: 6, ..shut };
    assert!(!full.open(HubRow::Weapon));
    assert_eq!(HubRow::Weapon.grey(full), "SIX ALREADY MADE");
    // Held wishes show themselves beside the row they feed; none held shows nothing.
    assert_eq!(held.wishes_label().as_deref(), Some("WISHES 2"));
    assert_eq!(shut.wishes_label(), None);
}

/// The hub page: the scroll's header, keys and notice read at both widths; no shut row wears a
/// minus.
#[test]
fn the_hub_page_reads_at_both_widths() {
    for (name, row, gate, note) in [
        ("wish", HubRow::Power, WishGate { power: true, made: 2, wishes: 2, ..Default::default() }, ""),
        ("sanctuary", HubRow::Reforge, WishGate { reforge: true, sanctuary: true, made: 1, ..Default::default() }, ""),
        ("mirror", HubRow::Look, WishGate { look: true, ..Default::default() }, ""),
        ("shut", HubRow::Look, WishGate::default(), ""),
        (
            "wrong",
            HubRow::Reforge,
            WishGate { reforge: true, sanctuary: true, ..Default::default() },
            "THAT IS A CREATURE: PRESS 4",
        ),
    ] {
        let mut s = Session::default();
        s.open_hub(0, "Ada", gate, Some(row));
        s.note = note.to_string();
        for h in [188, 214] {
            let (_, text) = said::read(|| frame(&s, 20, h, None));
            let lines: Vec<&str> = text.lines.iter().map(|l| l.text.as_str()).collect();
            assert!(lines.contains(&"THE WISH SCROLL"), "{name}, h={h}: {:?}", text.lines);
            assert!(lines.contains(&"WHAT DO YOU WISH FOR?"), "{name}, h={h}");
            assert!(lines.contains(&"ARROWS: PICK   E: OPEN   Q: CLOSE"), "{name}, h={h}");
            for r in HubRow::ALL {
                assert!(lines.contains(&r.title()), "{name}, h={h}: {} missing", r.title());
                // The painter cuts a long note to the room beside the name (`cut_to`).
                let cost = if gate.open(r) { r.need() } else { r.grey(gate) };
                assert!(
                    lines.iter().any(|l| l.len() >= cost.len().min(6) && cost.starts_with(l)),
                    "{name}, h={h}: {cost:?} missing"
                );
            }
            assert!(!lines.iter().any(|l| l.starts_with('-')), "{name}: a minus sign on a shut row");
            if !note.is_empty() {
                assert!(lines.contains(&note), "{name}, h={h}: the notice went missing");
            }
            for line in &text.lines {
                assert!(
                    line.at.0 >= 0 && line.at.1 >= 0 && line.at.0 + text_width(&line.text) <= line.on.0 && line.at.1 + 7 <= line.on.1,
                    "{name}, h={h}: {line:?}"
                );
            }
        }
    }
}

/// While the hub is up the tab strip above the scroll is not drawn, and the stage shows a picture
/// for the selected row rather than the stand-in knight (the Creature row alone may borrow one)
/// or the maker's own name plate.
#[test]
#[ignore = "the wish hub is off unless ANGEL_WISH_HUB=1, and its row pictures were never drawn"]
fn the_hub_hides_the_tab_strip_and_the_stand_in_knight() {
    let words = |s: &Session| {
        let (_, text) = said::read(|| frame(s, 20, HEIGHT, None));
        text.lines.into_iter().map(|l| l.text).collect::<Vec<_>>()
    };
    let held = WishGate { power: true, made: 2, wishes: 2, ..Default::default() };
    for row in [HubRow::Power, HubRow::Reforge, HubRow::Look, HubRow::Person, HubRow::Weapon] {
        let mut s = Session::default();
        s.open_hub(0, "Ada", held, Some(row));
        let lines = words(&s);
        assert!(!lines.iter().any(|l| l.starts_with("1 ROLE")), "{row:?}: the tab strip still draws: {lines:?}");
        assert!(!lines.iter().any(|l| l == "STAND-IN: KNIGHT"), "{row:?}: the knight still stands in");
        // The maker's pages stay behind the hub: its name plate never shows.
        assert!(!lines.iter().any(|l| l == "HERO  /  BIPED"), "{row:?}: the maker's name plate shows");
        assert!(lines.contains(&format!("A {}", row.kind_word())), "{row:?}: the plate does not name the row");
    }
}


const WALK: &[&str] = &[
    "......kk......",
    ".....kkkk.....",
    ".....kwwk.....",
    "......kk......",
    "....kkkkkk....",
    "...kkkkkkkk...",
    "..kkkkkkkkkk..",
    "...kkkkkkkk...",
    "....kkkkkk....",
    ".....kkkk.....",
    ".....k..k.....",
    ".....k..k.....",
    ".....k..k.....",
    "....kk..kk....",
    "...kk....kk...",
    "..kk......kk..",
];

fn door_shot(row: HubRow, words: &str, reply: &str, tick: u64) -> Session {
    let mut s = session();
    s.hub = true;
    s.row = row.index();
    s.gate.power = true;
    s.action(Action::Confirm, 1);
    let _ = s.hear_wish(words);
    if reply.is_empty() {
        return s;
    }
    if reply == "fallback" {
        s.give_up(tick);
    } else {
        let _ = s.take_reply(reply, tick);
    }
    s
}

#[test]
#[ignore]
fn write_u2_door_shots() {
    let dir = std::path::PathBuf::from(std::env::var_os("ANGEL_CREATOR_SHOTS").unwrap_or_else(|| ".we-out/unify/u2".into()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut run = Run::new(41, 3, Some("Ada"));
    let ink = WALK.join("\n");
    let creature = format!("wish golden-devil | Golden devil\nkind spell\nsay a tiny golden devil | tiny devil\ndraw summon\n{ink}\n");
    let person = format!("wish tall-smith | Tall smith\nkind hold\nsay a tall smith | smith\ndraw worn\n{ink}\n");
    let weapon = format!("wish long-spear | Long spear\nkind hold\nsay a long spear | spear\ndraw worn\n{ink}\n");
    let power = "wish fire-bolt | Fire bolt\nkind hold\nsay a fire bolt | fire\ntier damage 2 :: a hotter bolt\n";
    let flows = [
        ("power", HubRow::Power, "a fire bolt", power),
        ("creature", HubRow::Creature, "a tiny golden devil", creature.as_str()),
        ("person", HubRow::Person, "a tall smith", person.as_str()),
        ("weapon", HubRow::Weapon, "a long spear", weapon.as_str()),
    ];
    for (name, row, words, reply) in flows {
        for (w, h, tag) in [(384, 286, "384"), (768, 572, "768")] {
            let mut typed = door_shot(row, "", "", 2);
            typed.chat.draft = words.to_string();
            save_png(&dir, &format!("{name}-typed-{tag}.png"), &seat(&run, &typed, w, h));
            let waiting = door_shot(row, words, "", 2);
            save_png(&dir, &format!("{name}-waiting-{tag}.png"), &seat(&run, &waiting, w, h));
            let done = door_shot(row, words, reply, 8);
            save_png(&dir, &format!("{name}-result-{tag}.png"), &seat(&run, &done, w, h));
        }
    }
    let mut walk = door_shot(HubRow::Creature, "a tiny golden devil", &creature, 4);
    for frame in 0..24 {
        run.tick = frame * 3;
        save_png(&dir, &format!("walk-{frame:02}.png"), &seat(&run, &walk, 384, 286));
    }
    let _ = walk;
}
