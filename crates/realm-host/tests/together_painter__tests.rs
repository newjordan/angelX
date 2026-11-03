use super::*;
use std::time::{Duration, Instant};

const CARD: &str = "name   Fire Bow
kind   arm
rarity rare
text   Fire arrows.
arm    crossbow
draw shot
.22.|.33.
2222|3333
.22.|.33.
";

/// A fresh workspace with the card in its reforged folder.
fn workspace(name: &str) -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!("wish-painter-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let cards = root.join(".angel/dungeon/reforged");
    std::fs::create_dir_all(&cards).unwrap();
    let card = cards.join("fire-bow.card");
    std::fs::write(&card, CARD).unwrap();
    (root, card)
}

/// A two-frame painting on a transparent ground, a disc in each frame.
fn painting(path: &Path, colour: [u8; 3]) {
    let mut image = image::RgbaImage::new(64, 32);
    for (x, y, p) in image.enumerate_pixels_mut() {
        let (dx, dy) = ((x % 32) as i32 - 16, y as i32 - 16);
        if dx * dx + dy * dy < 100 {
            *p = image::Rgba([colour[0], colour[1], colour[2], 255]);
        }
    }
    image.save(path).unwrap();
}

/// A painter that runs `body` in the shell.
fn script(root: &Path, name: &str, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt as _;
    let path = root.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn card_job(root: &Path, card_path: &Path) -> Job {
    let card = cards::check("fire-bow", CARD).unwrap().card;
    let slots = slots(&card);
    assert_eq!(slots.len(), 1);
    assert_eq!(slots[0].0, Slot::Shot);
    assert_eq!(slots[0].1, 2);
    job(
        root,
        Target::Card(card_path.to_path_buf()),
        &card,
        "flaming arrows",
        &slots,
        (1, 1),
    )
}

/// Poll until every painter has ended, with what they painted.
fn settle(painters: &mut Painters) -> Vec<Target> {
    let mut painted = Vec::new();
    let start = Instant::now();
    while painters.running() + painters.queued() > 0 && start.elapsed() < Duration::from_secs(20) {
        painted.extend(painters.poll());
        std::thread::sleep(Duration::from_millis(20));
    }
    // The reader thread may finish a moment after its painter.
    std::thread::sleep(Duration::from_millis(100));
    painted.extend(painters.poll());
    // Each poll deduplicates its own batch, not notifications across polls.
    let mut unique = Vec::new();
    for target in painted {
        if !unique.contains(&target) {
            unique.push(target);
        }
    }
    unique
}

#[test]
fn two_lines_leave_the_second_painting_and_the_card_republished() {
    let (root, card) = workspace("two");
    let job = card_job(&root, &card);
    std::fs::create_dir_all(&job.out).unwrap();
    painting(&job.out.join("one.png"), [200, 40, 20]);
    painting(&job.out.join("two.png"), [240, 200, 40]);
    let painter = script(
        &root,
        "painter",
        r#"cat > job.json
echo '{"slot":"shot","file":"one.png","frames":2,"move":["stretch"]}'
echo '{"slot":"shot","file":"two.png","frames":2,"move":["bob","wobble"]}'"#,
    );
    let mut book = cards::Book::builtin();
    book.insert(cards::check("fire-bow", CARD).unwrap().card);
    let out = job.out.clone();
    let mut painters = Painters::default();
    painters.start(painter, job);
    let painted = settle(&mut painters);
    assert_eq!(painted, vec![Target::Card(card.clone())]);
    // The painter read its job.
    let asked: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("job.json")).unwrap()).unwrap();
    assert_eq!(asked["wish"], "flaming arrows");
    assert_eq!(asked["slots"][0]["slot"], "shot");
    assert_eq!(asked["slots"][0]["frames"], 2);
    assert_eq!(asked["slots"][0]["side"], 16);
    // The card ends with the second painting alone, its draw block gone.
    let text = std::fs::read_to_string(&card).unwrap();
    let sha = angel_live::host::sha256_hex(&std::fs::read(out.join("two.png")).unwrap());
    let name = format!("fire-bow-shot-{}.png", &sha[..12]);
    let line = format!("paint shot {name} frames=2 move=bob,wobble");
    assert!(text.contains(&line), "{text}");
    assert_eq!(text.matches("paint shot").count(), 1, "{text}");
    assert!(!text.contains("draw shot"), "{text}");
    let sheets = together_sheets::dir(&root);
    assert!(sheets.join(&name).is_file());
    assert_eq!(
        std::fs::read_dir(&sheets).unwrap().count(),
        1,
        "the first painting is let go"
    );
    // Republished: read again, the run's book draws the painting.
    let painted = card_from(&card, &sheets, &together_sheets::paint_dir(&root)).unwrap();
    let mut run = crate::together_shooter::Run::new(0, 9, None);
    run.book = book;
    let mut sim = crate::together_sim::Sim::from(Some(run));
    assert!(sim.paint_cards(vec![painted]).unwrap());
    let shot = sim.as_ref().unwrap().book.get("fire-bow").unwrap().looks.shot.clone().unwrap();
    assert_eq!(shot.paint.as_deref(), Some(sha.as_str()));
    assert_eq!(shot.frames.len(), 2);
    assert_eq!(shot.moves, vec![Move::Bob, Move::Wobble]);
    assert!(
        together_sheets::paint_dir(&root)
            .join(format!("{sha}.png"))
            .is_file()
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_failing_or_babbling_painter_changes_nothing() {
    let (root, card) = workspace("fail");
    let job = card_job(&root, &card);
    std::fs::create_dir_all(&job.out).unwrap();
    // Not transparent: the card would turn it away.
    image::RgbaImage::from_pixel(64, 32, image::Rgba([9, 9, 9, 255]))
        .save(job.out.join("flat.png"))
        .unwrap();
    let painter = script(
        &root,
        "painter",
        r#"echo 'painting…'
echo '{"slot":"shot","file":"../../../../../etc/passwd","frames":1}'
echo '{"slot":"shot","file":"missing.png","frames":2}'
echo '{"slot":"hat","file":"flat.png"}'
echo '{"slot":"shot","file":"flat.png","frames":2}'
echo '{"slot":"shot","file":"flat.png","frames":2,"move":["moonwalk"]}'
exit 3"#,
    );
    let mut painters = Painters::default();
    painters.start(painter, job);
    assert!(settle(&mut painters).is_empty());
    assert_eq!(std::fs::read_to_string(&card).unwrap(), CARD);
    assert!(std::fs::read_dir(together_sheets::dir(&root)).map_or(0, |d| d.count()) == 0);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn at_most_max_running_painters_run_and_the_rest_wait() {
    let (root, card) = workspace("queue");
    let painter = script(&root, "painter", "touch started\nsleep 0.4");
    let card_text = cards::check("fire-bow", CARD).unwrap().card;
    let mut painters = Painters::default();
    let mut outs = Vec::new();
    for n in 0..MAX_RUNNING + 1 {
        let mut one = card_text.clone();
        one.id = format!("fire-bow-{n}");
        let job = job(
            &root,
            Target::Card(card.clone()),
            &one,
            "fire",
            &[(Slot::Shot, 2)],
            (1, 1),
        );
        outs.push(job.out.clone());
        painters.start(painter.clone(), job);
    }
    assert_eq!(painters.running(), MAX_RUNNING);
    assert_eq!(painters.queued(), 1);
    settle(&mut painters);
    assert_eq!(painters.running() + painters.queued(), 0);
    for out in outs {
        assert!(out.join("started").is_file(), "{} ran", out.display());
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_learned_wish_is_painted_at_its_top_tier() {
    let raw = "wish fire-shot | Fire Shot\n\
say fire shot\n\
tier burn 1 :: Fire.\n\
tier burn 2 :: More fire.\n\
draw shot\n\
.22.\n\
2222\n\
draw shot 2 move=bob\n\
.33.\n\
3333\n\
\n\
wish other | Other\n\
say other\n\
tier damage 1 :: More.\n";
    let new = with_paint(raw, Some(("fire-shot", 2)), "wish-fire-shot-2", Slot::Shot, &paint("p.png", 1), None).unwrap();
    // The tier-2 look gives way, keeping its motion; tier 1's stays; the
    // next entry is untouched.
    assert_eq!(
        new,
        "wish fire-shot | Fire Shot\n\
say fire shot\n\
tier burn 1 :: Fire.\n\
tier burn 2 :: More fire.\n\
draw shot\n\
.22.\n\
2222\n\
paint shot p.png frames=1 move=bob tier=2\n\
\n\
wish other | Other\n\
say other\n\
tier damage 1 :: More.\n"
    );
    assert!(with_paint(raw, Some(("nowhere", 2)), "x", Slot::Shot, &paint("p.png", 1), None).is_none());
}

fn paint(file: &str, frames: usize) -> Paint {
    Paint {
        file: file.into(),
        frames,
        moves: Vec::new(),
        pixel: false,
    }
}

const ENTRY: &str = "wish fire-shot | Fire Shot\n\
say fire shot\n\
tier burn 1 :: Fire.\n\
tier burn 2 :: More fire.\n\
tier burn 2; damage 2 :: Most fire.\n\
draw shot move=stretch\n\
.k22.\n\
k2222\n\
.k22.\n\
\n\
wish other | Other\n\
say other\n\
tier damage 1 :: More.\n";

#[test]
fn a_levels_look_is_named_for_its_tier_over_the_learners_own() {
    let rows = vec!["..33..".to_string(), ".3333.".to_string(), "..33..".to_string()];
    let mut pixel = paint("wish-fire-shot-3-shot-0123.png", 2);
    pixel.pixel = true;
    let new = with_paint(ENTRY, Some(("fire-shot", 3)), "wish-fire-shot-3", Slot::Shot, &pixel, Some((1, &rows))).unwrap();
    // The learner's look stays (its grade), the painter's comes after it,
    // named for tier 1, its rows as many frames as the painting; the next
    // entry is untouched.
    assert_eq!(
        new,
        "wish fire-shot | Fire Shot\n\
say fire shot\n\
tier burn 1 :: Fire.\n\
tier burn 2 :: More fire.\n\
tier burn 2; damage 2 :: Most fire.\n\
draw shot move=stretch\n\
.k22.\n\
k2222\n\
.k22.\n\
paint shot wish-fire-shot-3-shot-0123.png frames=2 move=stretch pixel tier=1\n\
..33..|..33..\n\
.3333.|.3333.\n\
..33..|..33..\n\
\n\
wish other | Other\n\
say other\n\
tier damage 1 :: More.\n"
    );
    // Another level's look joins it; the same level's replaces it.
    let three = with_paint(&new, Some(("fire-shot", 3)), "wish-fire-shot-3", Slot::Shot, &paint("wish-fire-shot-3-shot-4567.png", 1), Some((3, &rows))).unwrap();
    assert!(three.contains("0123.png") && three.contains("paint shot wish-fire-shot-3-shot-4567.png frames=1 move=stretch tier=3\n..33..\n"), "{three}");
    let again = with_paint(&three, Some(("fire-shot", 3)), "wish-fire-shot-3", Slot::Shot, &paint("wish-fire-shot-3-shot-89ab.png", 1), Some((1, &rows))).unwrap();
    assert!(!again.contains("0123") && again.contains("4567") && again.contains("89ab.png frames=1 move=stretch tier=1"), "{again}");
    assert!(again.contains("draw shot move=stretch\n.k22."), "the learner's look stays: {again}");
    // A top picture alone takes the top tier's place.
    let alone = with_paint(&again, Some(("fire-shot", 3)), "wish-fire-shot-3", Slot::Shot, &paint("wish-fire-shot-3-shot-cdef.png", 1), None).unwrap();
    assert!(!alone.contains("4567") && alone.contains("89ab") && alone.contains("cdef.png frames=1 move=stretch tier=3\n\nwish other"), "{alone}");
}

#[test]
fn a_card_with_rows_is_its_painting_alone() {
    let rows = vec!["..33..".to_string(), ".3333.".to_string()];
    let new = with_paint(CARD, None, "fire-bow", Slot::Shot, &paint("fire-bow-shot-0123.png", 2), Some((1, &rows))).unwrap();
    assert!(!new.contains("draw shot"), "{new}");
    assert!(new.ends_with("paint shot fire-bow-shot-0123.png frames=2\n..33..|..33..\n.3333.|.3333.\n"), "{new}");
    // Four-frame rows under a one-frame painting: the first frame.
    let four = vec!["1|2|3|4".to_string()];
    assert_eq!(fit_rows(&four, 1), vec!["1".to_string()]);
    assert_eq!(fit_rows(&four, 4), four);
}

/// A learned workspace: ENTRY learned in it.
fn learned_workspace(name: &str) -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!("wish-painter-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join(".angel/dungeon")).unwrap();
    let book = root.join(".angel/dungeon/phrasebook.txt");
    std::fs::write(&book, ENTRY).unwrap();
    (root, book)
}

fn learned_job(root: &Path, book: &Path, level: usize) -> Job {
    let (wishes, errors) = phrasebook::read_checked(&format!("{}\n{ENTRY}", phrasebook::LEARNED));
    assert!(errors.is_empty(), "{errors:?}");
    let wish = wishes.iter().find(|w| w.id == "fire-shot").unwrap().clone();
    let card = wish.card(2).unwrap();
    let target = Target::Learned {
        phrasebook: book.to_path_buf(),
        wish: wish.id.clone(),
        top: 3,
    };
    job(root, target, &card, "fire shot", &slots(&card), (level, 3))
}

/// The learned wish as the book reads it from the workspace, its pictures
/// cut and kept.
fn read_back(root: &Path, book: &Path) -> phrasebook::Wish {
    let raw = std::fs::read_to_string(book).unwrap();
    let (inlined, errors) = together_files::phrasebook_with_sheets(&raw, &together_sheets::dir(root), &together_sheets::paint_dir(root));
    assert!(errors.is_empty(), "{errors:?}");
    let (wishes, errors) = phrasebook::read_checked(&format!("{}\n{inlined}", phrasebook::LEARNED));
    assert!(errors.is_empty(), "{errors:?}");
    wishes.into_iter().find(|w| w.id == "fire-shot").unwrap()
}

/// A one-frame picture in the job's folder: a `side` square of `ink`.
fn pixel_picture(path: &Path, side: u32, frames: u32, colour: [u8; 3]) {
    let image = image::RgbaImage::from_fn(side * frames + 2 * frames, side + 2, |x, y| {
        let x = x % (side + 2);
        if (1..=side).contains(&x) && (1..=side).contains(&y) {
            image::Rgba([colour[0], colour[1], colour[2], 255])
        } else {
            image::Rgba([0, 0, 0, 0])
        }
    });
    image.save(path).unwrap();
}

#[test]
fn each_level_shows_its_own_look_as_drawn_and_the_grade_holds() {
    let (root, book) = learned_workspace("levels");
    let job = learned_job(&root, &book, 1);
    // The painter was told the rows as shown, their size, and the levels.
    let asked: serde_json::Value = serde_json::from_str(&job.input).unwrap();
    assert_eq!(asked["slots"][0]["rows"], serde_json::json!([".k22.", "k2222", ".k22."]));
    assert_eq!(asked["slots"][0]["size"], serde_json::json!([5, 3]));
    assert_eq!((asked["level"].as_u64(), asked["tiers"].as_u64()), (Some(1), Some(3)));
    assert_eq!(job.at_level(2).level, 2);
    assert_eq!(job.at_level(9).level, 3);
    let before = read_back(&root, &book).grade();
    std::fs::create_dir_all(&job.out).unwrap();
    pixel_picture(&job.out.join("low.png"), 5, 1, [200, 40, 20]);
    pixel_picture(&job.out.join("high.png"), 20, 1, [40, 200, 20]);
    pixel_picture(&job.out.join("strip.png"), 20, 4, [40, 20, 200]);
    // Bad lines change nothing.
    for bad in [
        r#"{"slot":"shot","level":1,"file":"low.png","rows":["..ZZ..","..qq*."]}"#,
        r#"{"slot":"shot","level":1,"file":"low.png","rows":["kkkkkkkkkkkkkkkkkkkkkkkkkkkk"]}"#,
        r#"{"slot":"shot","level":4,"file":"low.png","rows":["33"]}"#,
        r#"{"slot":"shot","level":1,"file":"low.png"}"#,
        r#"{"slot":"shot","rows":["33"],"file":"low.png"}"#,
        r#"{"slot":"shot","level":1,"rows":["33"]}"#,
    ] {
        assert!(apply(&job, bad).is_err(), "{bad}");
    }
    assert_eq!(std::fs::read_to_string(&book).unwrap(), ENTRY);
    let low = r#"{"slot":"shot","level":1,"file":"low.png","pixel":true,"rows":[".1111.","111111",".1111."]}"#;
    let high = r#"{"slot":"shot","level":2,"file":"high.png","pixel":true,"rows":[".3333.","333333",".3333."]}"#;
    let top = r#"{"slot":"shot","level":3,"file":"high.png","pixel":true,"rows":[".3333.","333333",".3333."]}"#;
    for line in [low, high, top] {
        assert_eq!(apply(&job, line), Ok(Applied::Written), "{line}");
    }
    let wish = read_back(&root, &book);
    assert_eq!(wish.grade(), before, "the learner's drawing is still what is graded");
    let shot = |t: usize| read_back(&root, &book).card(t).unwrap().looks.shot.unwrap();
    // Each level its own look, as drawn: the low one first, the high one
    // above, a picture and the pixel word at every level.
    assert_eq!(shot(0).frames[0], vec![".1111.", "111111", ".1111."]);
    assert_eq!(shot(1).frames[0], vec![".3333.", "333333", ".3333."]);
    for t in 0..3 {
        assert!(shot(t).paint.is_some() && shot(t).pixel, "tier {t}");
        assert_eq!(shot(t).moves, vec![Move::Stretch], "tier {t}");
    }
    assert_eq!(shot(1).paint, shot(2).paint, "one picture, two levels");
    // The top level's animation: four frames, the rows held still.
    let anim = r#"{"slot":"shot","level":3,"file":"strip.png","frames":4,"pixel":true,"rows":[".3333.","333333",".3333."]}"#;
    assert_eq!(apply(&job, anim), Ok(Applied::Written));
    assert_eq!(shot(2).frames.len(), 4);
    assert_eq!(shot(1).frames.len(), 1);
    // Three pictures written, two still named: the one let go is gone.
    let sheets: Vec<_> = std::fs::read_dir(together_sheets::dir(&root)).unwrap().flatten().collect();
    assert_eq!(sheets.len(), 3, "low, high and the strip");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_climb_starts_the_painter_again_first_in_line_and_a_restart_carries_on() {
    let (root, book) = learned_workspace("climb");
    let painter = script(&root, "painter", "cat > asked.json\nsleep 0.5\necho '{\"background\":true}'\nsleep 1");
    let mut painters = Painters::default();
    let grant = learned_job(&root, &book, 1);
    let key = grant.key.clone();
    let out = grant.out.clone();
    painters.start(painter.clone(), grant);
    assert_eq!(painters.level(&key), Some(1));
    assert!(out.join("job.json").is_file(), "kept with the pictures");
    // The same level again asks for nothing; a higher one goes first in line.
    assert!(!painters.climbed(&key, 1, || None));
    for n in 1..MAX_RUNNING - 1 {
        let mut job = learned_job(&root, &book, 1);
        job.key = format!("busy-{n}");
        job.out = job.out.with_file_name(format!("busy-{n}"));
        painters.start(painter.clone(), job);
    }
    let other = {
        let mut job = learned_job(&root, &book, 1);
        job.key = "other".into();
        job.out = job.out.with_file_name("other");
        job
    };
    painters.start(painter.clone(), other.clone());
    painters.start(painter.clone(), other.clone());
    assert!(painters.climbed(&key, 2, || None));
    assert_eq!(painters.level(&key), Some(2));
    assert_eq!(painters.queue.front().map(|(_, j)| j.key.clone()), Some(key.clone()));
    assert_eq!(painters.running(), MAX_RUNNING, "the wish's first painter carries on");
    let start = Instant::now();
    let asked = || std::fs::read_to_string(out.join("asked.json")).unwrap_or_default();
    while !asked().contains("\"level\":2") && start.elapsed() < Duration::from_secs(5) {
        painters.poll();
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(asked().contains("\"level\":2"), "the climb's painter was told its level");
    painters.stop();
    // A restarted host knows the wish from its folder, at the level kept.
    let mut fresh = Painters::default();
    let target = Target::Learned { phrasebook: book.clone(), wish: "fire-shot".into(), top: 3 };
    let kept = Job::kept(&root, &key, target).expect("its job is kept");
    assert_eq!(kept.level, 2);
    assert!(fresh.climbed(&key, 2, || Some((painter.clone(), kept))));
    assert_eq!(fresh.level(&key), Some(2));
    fresh.stop();
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_painter_in_the_background_holds_no_place() {
    let (root, card) = workspace("background");
    let painter = script(&root, "painter", "touch started\nsleep 0.3\necho '{\"background\":true}'\nsleep 2");
    let card_text = cards::check("fire-bow", CARD).unwrap().card;
    let mut painters = Painters::default();
    let mut outs = Vec::new();
    let all = MAX_RUNNING + 2;
    for n in 0..all {
        let mut one = card_text.clone();
        one.id = format!("fire-bow-{n}");
        let job = job(&root, Target::Card(card.clone()), &one, "fire", &[(Slot::Shot, 2)], (1, 1));
        outs.push(job.out.clone());
        painters.start(painter.clone(), job);
    }
    assert_eq!(painters.running(), MAX_RUNNING);
    let start = Instant::now();
    while (painters.running() < all || painters.quick() > 0) && start.elapsed() < Duration::from_secs(5) {
        painters.poll();
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(painters.running(), all, "each quick part done, the next began");
    assert_eq!(painters.quick(), 0);
    for out in &outs {
        assert!(out.join("started").is_file(), "{} ran", out.display());
    }
    // A reforged card asked again stops its older painter.
    let mut again = card_text.clone();
    again.id = "fire-bow-0".into();
    let quiet = script(&root, "quiet", "sleep 2");
    painters.start(quiet, job(&root, Target::Card(card.clone()), &again, "fire", &[(Slot::Shot, 2)], (1, 1)));
    assert_eq!(painters.running(), all, "the old fire-bow-0 stopped, the new one runs");
    painters.stop();
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn painter_worn_job_names_the_reference_body_part_and_anchor() {
    let (root, path) = workspace("fit-contract");
    let card = cards::check("boots", "name Boots\nkind hold\narmor 1\ndraw worn at=feet\n..HH..\n..HH..\n").unwrap().card;
    let job = job(&root, Target::Card(path), &card, "boots", &slots(&card), (1, 3));
    let input: serde_json::Value = serde_json::from_str(&job.input).unwrap();
    let slot = &input["slots"][0];
    assert_eq!(slot["body"]["size"], serde_json::json!([16,16]));
    assert_eq!(slot["body"]["origin"], serde_json::json!([4,4]));
    assert_eq!(slot["part"], "feet");
    assert_eq!(slot["anchor"], serde_json::json!([8,15]));
    assert_eq!(slot["part_size"], serde_json::json!([12,5]));
    assert_eq!(slot["attachment"], "bottom-centre");
    assert_eq!(slot["size"], serde_json::json!([2,2]), "local subject, without the 24x24 placement canvas");
    assert_eq!(slot["rows"], serde_json::json!(["HH", "HH"]));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn painter_crops_a_corner_subject_without_losing_or_mixing_animation_cells() {
    let mut image = image::RgbaImage::new(48, 24);
    for y in 3..20 { image.put_pixel(0,y,image::Rgba([200,10,20,255])); }
    for y in 4..19 { image.put_pixel(24,y,image::Rgba([10,200,20,255])); }
    let mut bytes = std::io::Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    let bytes = together_sheets::fit_picture(bytes.get_ref(), 2).unwrap();
    let cropped = image::load_from_memory(&bytes).unwrap().to_rgba8();
    assert_eq!(cropped.dimensions(), (34,17));
    let red = cropped.enumerate_pixels().filter(|(x,_,p)| *x < 17 && p[3] > 0 && p[0] == 200).count();
    let green = cropped.enumerate_pixels().filter(|(x,_,p)| *x >= 17 && p[3] > 0 && p[1] == 200).count();
    assert_eq!((red,green), (17,15));
    assert_eq!(cropped.pixels().filter(|p| p[3] > 0).count(), 32);
    assert!(together_sheets::fit_picture(&bytes, 3).is_err());
    let mut square = image::RgbaImage::new(24,24);
    for y in 0..5 { for x in 0..5 { square.put_pixel(x,y,image::Rgba([200,10,20,255])); }}
    let mut square_bytes = std::io::Cursor::new(Vec::new());
    square.write_to(&mut square_bytes,image::ImageFormat::Png).unwrap();
    let fitted = together_sheets::fit_picture(square_bytes.get_ref(),1).unwrap();
    let square = image::load_from_memory(&fitted).unwrap().to_rgba8();
    assert_eq!(square.dimensions(), (7,7));
    assert_eq!(square.get_pixel(0,0)[3], 0);
    assert_eq!(square.pixels().filter(|p| p[3] >= 128).count(), 25);
    assert_eq!(together_sheets::fit_picture(&fitted,1).unwrap(), fitted, "a normalized solid subject remains a valid painting");
    let mut empty = std::io::Cursor::new(Vec::new());
    image::RgbaImage::new(24,24).write_to(&mut empty,image::ImageFormat::Png).unwrap();
    assert!(together_sheets::fit_picture(empty.get_ref(),1).unwrap_err().contains("nothing drawn"));
}

#[test]
fn painter_keeps_a_worn_attachment_at_every_tier_including_legacy_rows() {
    let raw = "wish boots | Boots\nsay boots\ntier armor 1 :: Boots.\ndraw worn at=feet\nHH\nHH\n";
    let (root, _) = workspace("fit-level");
    let sheets = together_sheets::dir(&root);
    std::fs::create_dir_all(&sheets).unwrap();
    painting(&sheets.join("wish-boots-1-worn-fit.png"), [200, 40, 20]);
    let rows = vec!["..HH..".into(), "..HH..".into()];
    let text = with_paint(raw, Some(("boots",1)), "wish-boots-1", Slot::Worn, &paint("wish-boots-1-worn-fit.png",2), Some((1,&rows))).unwrap();
    assert!(text.contains("frames=2 at=feet"), "{text}");
    for text in [text.clone(), text.replace("frames=2 at=feet", "frames=2")] {
        let (inlined, errors) = together_files::phrasebook_with_sheets(&text,&sheets,&together_sheets::paint_dir(&root));
        assert!(errors.is_empty(), "{errors:?}");
        let (wishes, errors) = phrasebook::read_checked(&inlined);
        assert!(errors.is_empty(), "{errors:?}");
        let sprite = wishes[0].card(0).unwrap().looks.worn.unwrap();
        assert_eq!(sprite.at.as_deref(), Some("feet"));
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn painter_repairs_a_legacy_part_from_the_target_entry_in_a_shared_phrasebook() {
    let raw = "wish first | First\nsay first\ntier armor 1 :: First.\ndraw worn at=head\nHH\n\nwish cape | Magnetic Cape\nsay cape\ntier armor 1 :: Cape.\ndraw worn\nHH\nHH\n";
    let rows = vec!["HH".into(), "HH".into()];
    let text = with_paint(raw, Some(("cape",1)), "wish-cape-1", Slot::Worn, &paint("wish-cape-1-worn-fit.png",1), Some((1,&rows))).unwrap();
    assert!(text.contains("paint worn wish-cape-1-worn-fit.png frames=1 at=back"), "{text}");
}
