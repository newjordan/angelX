use super::*;

/// Built-in pieces the checker finds under the bar that are not mended yet,
/// each with why: the suite holds the list to exactly these, so a new piece
/// under the bar fails it, and mending one fails it until its line goes.
const FLAGGED: &[(&str, &str)] = &[
    ("card last-call-saucers", "Astra's 10-30 night: its picture grades Crude (30 of 100); a redraw, not a number, mends it"),
    ("card keelbreaker", "Astra's 10-30 night: its picture grades Crude (56 of 100, Fine from 58); needs a darker edge and a shaded ramp"),
    ("foe the-mire-king", "no charge-up in its file: the swamp's last fight is its own (`swamp`); a windup line would change it, the owner's call"),
];

#[test]
fn every_template_passes_with_nothing_to_say() {
    let failing: Vec<String> = templates().iter().filter(|r| !r.passes()).map(Report::explain).collect();
    assert!(failing.is_empty(), "templates under the bar:\n{}", failing.join("\n"));
    for (name, text) in TEMPLATES {
        let report = check_template(name, text);
        assert!(report.grade.is_none_or(|(g, _)| g >= TEMPLATE_GRADE), "{}", report.explain());
    }
    // One of every kind.
    for kind in Kind::ALL {
        assert!(TEMPLATES.iter().any(|(n, _)| Kind::of_file(n) == Some(kind)), "no {} template", kind.word());
    }
}

#[test]
fn a_templates_example_is_its_piece_alone_and_reads_the_same() {
    for (name, text) in TEMPLATES {
        let e = example(name).unwrap();
        assert!(!e.lines().any(|l| l.trim_start().starts_with('#')) || name.ends_with(".room"), "{name}");
        assert_eq!(check_file(name, &e).unwrap().faults, check_file(name, text).unwrap().faults, "{name}");
    }
    let (wishes, errors) = phrasebook::read_checked(&example("wish-worn.wish").unwrap());
    assert!(errors.is_empty() && wishes.len() == 1 && wishes[0].id == "spiked-pavise");
}

#[test]
fn every_built_in_piece_is_at_the_bar_or_flagged() {
    let reports = builtin();
    let key = |r: &Report| format!("{} {}", r.kind.word(), r.id);
    let unflagged: Vec<String> = reports
        .iter()
        .filter(|r| !r.passes() && !FLAGGED.iter().any(|(k, _)| *k == key(r)))
        .map(Report::explain)
        .collect();
    assert!(unflagged.is_empty(), "built-in pieces under the bar (mend them, or flag them with why):\n{}", unflagged.join("\n"));
    let mended: Vec<&str> = FLAGGED
        .iter()
        .filter(|(k, _)| !reports.iter().any(|r| key(r) == *k && !r.passes()))
        .map(|(k, _)| *k)
        .collect();
    assert!(mended.is_empty(), "flagged but at the bar now: drop them from FLAGGED: {mended:?}");
    assert!(!reports.iter().any(Report::refused), "nothing built in is turned away");
}

#[test]
fn every_content_file_on_disk_is_checked() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../cockpit/assets/dungeon");
    let stems = |dir: &str, ext: &str| -> Vec<String> {
        let mut out: Vec<String> = std::fs::read_dir(root.join(dir))
            .unwrap()
            .filter_map(|e| e.ok()?.file_name().into_string().ok())
            .filter_map(|n| n.strip_suffix(ext).map(str::to_string))
            .collect();
        out.sort();
        out
    };
    let cards: Vec<&str> = cards::BUILTIN.iter().map(|(id, _)| *id).collect();
    for stem in stems("cards", ".card") {
        assert!(cards.contains(&stem.as_str()), "cards/{stem}.card is in no catalogue (cards::BUILTIN), so nothing checks it");
    }
    let foes: Vec<String> = foe_files().into_iter().map(|(id, _)| id).collect();
    for stem in stems("bosses", ".boss") {
        assert!(foes.contains(&stem), "bosses/{stem}.boss is in no catalogue (bosses::BUILTIN, CHECKED, a dread), so nothing checks it");
    }
    let names: Vec<&str> = TEMPLATES.iter().map(|(n, _)| *n).collect();
    for entry in std::fs::read_dir(root.join("templates")).unwrap() {
        let name = entry.unwrap().file_name().into_string().unwrap();
        assert!(names.contains(&name.as_str()), "templates/{name} is not in content::TEMPLATES");
    }
}

#[test]
fn a_fault_is_said_plainly() {
    // A wish whose ladder does not climb, and that is drawn as a blob.
    let r = check(Kind::Wish, "x", "wish blob | Blob\nsay blob\ntier damage 10 :: Harder.\ntier damage 10 :: The same.\ndraw shot\n44\n44\n");
    assert!(r.refused(), "{}", r.explain());
    assert!(r.explain().contains("gives nothing more than tier 1"), "{}", r.explain());
    // Under the bar: one way to say it, a blob for a look.
    let r = check(Kind::Wish, "x", "wish blob | Blob\nsay blob\ntier damage 10 :: Harder.\ndraw shot\n44\n44\n");
    assert!(!r.refused() && !r.passes(), "{}", r.explain());
    assert!(r.explain().contains("1 way to say it"), "{}", r.explain());
    assert!(r.explain().contains("grade Crude"), "{}", r.explain());
    // A worn look too big for its place.
    let big = "wish cape | Big Cape\nsay big cape | huge cape | giant cape\ntier max_hp 10 :: A cape.\ndraw worn at=head\n".to_string()
        + &"hhhhhhhhhhhhhhhh\n".repeat(10);
    assert!(check(Kind::Wish, "x", &big).explain().contains("squeezed to fit"));
    // A foe with no charge-up whose numbers are held to their bounds.
    let r = check(Kind::Foe, "x", "name X\nwhere crypt\nhp 99999\nattack aimed shots=3 every=60\nart\nhH\nHh\n");
    let e = r.explain();
    assert!(e.contains("hp 99999 clamped to 900") && e.contains("no `windup`") && e.contains("blown up"), "{e}");
    // A room shut at a door, and one of no room's size.
    let mut shut = vec!["..........."; 5];
    shut.push("#..........");
    assert!(room_rows("x", &shut).explain().contains("doorway lane is shut"));
    assert!(room_rows("x", &["....", "...."]).explain().contains("4 by 2"));
    // An effect that outlives its moment, a line that does not read.
    let e = check(Kind::Effect, "x", "vfx kill sparks fire life=30\nvfx hit glitter\n").explain();
    assert!(e.contains("keeps a kill for 14") && e.contains("write life=9"), "{e}");
    assert!(e.contains("[refused]") && e.contains("needs a form"), "{e}");
    // A pastime that pays gold, and one whose banner is too wide.
    let e = check(Kind::Pastime, "x", "name A VERY LONG NAME FOR A PASTIME GAME\nroom village\nstart 3 7\ntime 40\nneed 3\npays gold\n").explain();
    assert!(e.contains("never gold") && e.contains("shorten the name"), "{e}");
}

#[test]
fn the_stature_numbers_are_the_renderers() {
    // `arena::stature` stands foes by these; the checker reads them.
    assert_eq!(stature::CHIEF, stature::KNIGHT * 3 / 2);
    assert_eq!(stature::GUARDIAN, stature::KNIGHT * 2);
    assert_eq!(stature::UNIT, 16.0 / super::super::layout::TILE_UNITS);
}

#[test]
fn every_recipe_the_generator_makes_ends_inside_its_moments() {
    for (id, raw) in cards::BUILTIN {
        let card = cards::check(id, raw).unwrap().card;
        for l in vfx::generated(&card).layers {
            if let Some(most) = vfx::fits(l.on) {
                assert!(l.life <= most, "{id}: {} life {} past {most}", l.on.word(), l.life);
            }
        }
    }
}

/// `ANGEL_CONTENT_CENSUS=<file> cargo test write_content_census -- --ignored`:
/// every built-in piece's report, and every template's, as text.
#[test]
#[ignore]
fn write_content_census() {
    let Some(path) = std::env::var_os("ANGEL_CONTENT_CENSUS") else { return };
    let mut out = String::new();
    for r in templates().iter().chain(builtin().iter()) {
        out.push_str(&r.explain());
        out.push('\n');
    }
    std::fs::write(path, out).unwrap();
}

#[test]
#[ignore]
fn print_boss_numbers() {
    let Some(path) = std::env::var_os("ANGEL_BOSS_NUMBERS") else { return };
    let mut out = String::new();
    for (id, raw) in foe_files().into_iter().chain([("foe-guardian".to_string(), template("foe-guardian.boss").unwrap())]) {
        let (b, notes) = bosses::check(&id, raw).unwrap();
        if notes.is_empty() {
            continue;
        }
        out.push_str(&format!("{id}: {notes:?}\n"));
        for a in &b.attacks {
            out.push_str(&format!("  attack {:?} shots={} arc={} speed={} every={} damage={} from={:?}\n", a.pattern, a.shots, a.arc, a.speed, a.every, a.damage, a.from));
        }
        if let Some(p) = &b.phases {
            out.push_str(&format!("  phases {:?} every={} volleys={} damage={} from={:?}\n", p.chain, p.every, p.volleys, p.damage, p.from));
        }
        for w in &b.windups {
            out.push_str(&format!("  windup {:?} wind={} every={} damage={} shots={} speed={} arc={} from={:?}\n", w.kind, w.wind, w.every, w.damage, w.shots, w.speed, w.arc, w.from));
        }
    }
    std::fs::write(path, out).unwrap();
}

/// The Archive's floor `seed`, as a party walks down to it.
fn archive_floor(seed: u64) -> crate::rules::Run {
    use crate::rules::{Run, home::Home};
    let mut run = Run::at_home(seed, 1, None, Home::default(), Default::default());
    run.begin_in(Pack::Crypt);
    while run.dungeon.pack != Pack::Archive && run.dungeon.depth < 6 {
        run.descend_for_test();
    }
    run
}

#[test]
fn the_drowned_reeve_is_met_in_the_drowned_archive() {
    // Made from the chief template, it is one of the Archive's own: a camp
    // fights under its name, and the Wanted hall puts its poster up.
    let (mut led, mut wanted) = (0, 0);
    for seed in 1..=24u64 {
        let mut run = archive_floor(seed);
        assert_eq!(run.dungeon.pack, Pack::Archive);
        if let Some(gates) = &run.boss_gates {
            led += gates.leaders.iter().filter(|l| run.bosses[usize::from(l.boss)].id == "archive-drowned-reeve").count();
            if let Some(g) = gates.guardian_boss {
                assert_ne!(run.bosses[usize::from(g)].id, "archive-drowned-reeve", "a chief, not the stairs' guardian");
            }
        }
        if let Some(hall) = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Wanted) {
            run.enter(hall, None);
            wanted += run
                .enemies
                .iter()
                .filter_map(|e| e.boss)
                .filter(|&b| run.bosses[usize::from(b)].id == "archive-drowned-reeve")
                .count();
        }
    }
    assert!(led > 0 && wanted > 0, "led {led} camps, wanted in {wanted} halls");
}

