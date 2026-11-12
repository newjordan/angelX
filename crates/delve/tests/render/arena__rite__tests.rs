//! The realm's makings on screen (`rules::rite`, `rules::made`): the Rite Wall of the Delve's
//! courtyard, the folk and heroes the Creator made standing in the Griffin and the courtyard, a
//! made friend and foe met in the delve, and the rite's own film: a newcomer arrives in the bar,
//! the Creator pops up, they bring things to life and the world takes them in.
//! `ANGEL_ARENA_SHOTS=<dir> cargo test write_creator_rite -- --ignored`.

use super::rite_wall::{self, PEG_STEP, PEGS, RAIL};
use super::*;
use crate::rules::creation::{Action, Role, Session, Step};
use crate::rules::rite::tests::{blade, creature, realm_with, wished};
use crate::rules::rite::{self, Kind, Made, Rite};
use crate::rules::{Input, RoomKind, Run, home::Home};
use std::collections::BTreeMap;

fn seat() -> (i32, i32) {
    (NATIVE_W, NATIVE_H + super::party::BAND_H + super::party::HUD_H)
}

fn rite_of(knight: &str, made: Made, n: u32) -> Rite {
    Rite { knight: knight.into(), kind: made.kind(), words: made.summary(), card: String::new(), name: made.name().into(), day: n, made: Some(made) }
}

fn home_of(rites: Vec<Rite>) -> Home {
    let mut home = Home::default();
    for r in rites {
        assert!(home.rites.record(r));
    }
    home
}

/// The courtyard as a run stands in it: the host alone, come out of the inn.
fn yard(home: Home) -> Run {
    let mut run = Run::at_home(41, 1, Some("Friend"), home, Default::default());
    run.naked_start = true;
    run.walk_alehouse();
    let gate = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Gate).unwrap();
    run.arrive(gate, crate::rules::world::COURTYARD_ARRIVAL);
    run.floor_plates.clear();
    run.step(&BTreeMap::new());
    run
}

fn griffin(home: Home) -> Run {
    let mut run = Run::at_home(41, 1, Some("Friend"), home, Default::default());
    run.naked_start = true;
    run.walk_alehouse();
    run.floor_plates.clear();
    run.step(&BTreeMap::new());
    run
}

/// The whole room at once, the camera's crop taken off.
fn whole(run: &Run) -> Img {
    let mut cv = (*scenery_cached(run)).clone();
    cv.stamp(&room_layer(run, None, true), 0, 0);
    super::world::boards(&mut cv, run);
    cv
}

fn lit(img: &Img) -> bool {
    img.rgba_bytes().chunks(4).any(|p| p[..3] != [0, 0, 0])
}

fn differs(a: &Img, b: &Img) -> usize {
    a.rgba_bytes().chunks(4).zip(b.rgba_bytes().chunks(4)).filter(|(x, y)| x != y).count()
}

fn place(run: &mut Run, id: u32, (x, y): (f32, f32)) {
    let h = run.players.get_mut(&id).unwrap();
    (h.x, h.y) = (x * 2.0, y * 2.0);
}

#[test]
fn an_empty_wall_draws_nothing_and_a_weapon_hangs_with_its_makers_tag() {
    let empty = yard(Home::default());
    assert!(rite_wall::shown(&empty).is_empty());
    let run = yard(home_of(vec![rite_of("Ada", blade("Oathmark"), 1)]));
    let pieces = rite_wall::shown(&run);
    assert_eq!(pieces.len(), 1);
    let (a, b) = (whole(&empty), whole(&run));
    assert!(differs(&a, &b) > 200, "the rail, the weapon and its tag are drawn");
    // Everything it draws is inside the rail's own box and the tag under it, in the courtyard.
    let tag = rite_wall::tag_at(&pieces[0]);
    assert!(tag.1 + 8 < b.h && RAIL.0 - PEG_STEP < tag.0 && tag.0 < RAIL.0 + PEG_STEP * PEGS as i32);
    // Not in any other room.
    let elsewhere = griffin(home_of(vec![rite_of("Ada", blade("Oathmark"), 1)]));
    assert!(rite_wall::shown(&elsewhere).is_empty());
}

#[test]
fn the_wall_shows_the_newest_five_weapons_and_no_minions_stand_on_the_lawn() {
    let mut rites = vec![rite_of("Ada", creature(Role::Minion, "Pip"), 0)];
    for n in 0..PEGS + 2 {
        rites.push(rite_of(&format!("Maker{n}"), blade(&format!("Blade{n}")), n as u32 + 1));
    }
    let run = yard(home_of(rites));
    let pieces = rite_wall::shown(&run);
    assert_eq!(pieces.len(), PEGS);
    assert!(pieces.iter().all(|p| p.rite.kind == Kind::Weapon), "minions live in the dungeons now");
    assert_eq!(pieces[0].rite.knight, "Maker2", "the oldest weapons rolled off the wall");
}

#[test]
fn folk_and_heroes_stand_in_their_rooms_with_their_makers_and_nowhere_else() {
    let rites = vec![
        rite_of("Ada", wished("a barman who stands behind the bar"), 1),
        rite_of("Bors", wished("a pilgrim who wanders the courtyard"), 2),
        rite_of("Cai", creature(Role::Hero, "Dame Wren"), 3),
    ];
    let (bar, plain_bar) = (griffin(home_of(rites.clone())), griffin(Home::default()));
    assert!(differs(&whole(&bar), &whole(&plain_bar)) > 150, "the barman is drawn in the Griffin");
    let (court, plain_court) = (yard(home_of(rites)), yard(Home::default()));
    assert!(differs(&whole(&court), &whole(&plain_court)) > 300, "the pilgrim and the hero are drawn in the courtyard");
    // A knight beside the barman is told who made him and that E speaks to him.
    let mut bar = bar;
    let at = crate::rules::made::placed(&bar, RoomKind::Alehouse, bar.tick)[0].at;
    place(&mut bar, 1, (at.0 + 0.5, at.1));
    let plate = rite_wall::plate_for(&bar, 1);
    assert!(plate.iter().any(|(t, _)| t == "MADE BY ADA") && plate.iter().any(|(t, _)| t == "E: SPEAK"), "{plate:?}");
}

#[test]
fn a_made_friend_and_a_made_foe_are_drawn_from_the_creators_own_figures() {
    let mut run = Run::new(5, 5, None);
    run.home.rites.record(rite_of("Ada", creature(Role::Minion, "Pip"), 1));
    run.home.rites.record(rite_of("Bors", creature(Role::Monster, "Gruk"), 2));
    run.dungeon.depth = 2;
    for index in 1..run.dungeon.rooms.len().min(9) {
        run.enter_for_test(index);
        if run.enemies.iter().any(|e| e.made.is_some()) {
            break;
        }
    }
    let friend = run.hireling.clone().expect("the friend");
    let mut cv = Img::new(1400, 900);
    super::hireling::ally(&mut cv, &run, &friend);
    assert!(lit(&cv), "the made friend is drawn");
    let foe = run.enemies.iter().find(|e| e.made.is_some()).expect("the foe");
    let im = super::figures::foe_figure_with(&run, foe, 3);
    assert!(lit(&im) && im.h > 20, "the made foe is its figure under a health bar and its name");
}

// ─── the rite's film ────────────────────────────────────────────────────────

fn film_dir(name: &str) -> Option<std::path::PathBuf> {
    let dir = std::path::PathBuf::from(std::env::var_os("ANGEL_ARENA_SHOTS")?).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    Some(dir)
}

fn keys(ids: &[u32]) -> BTreeMap<u32, Input> {
    ids.iter().map(|&id| (id, Input { aim_x: 1, ..Default::default() })).collect()
}

/// A film: frames of seat 1's view as the client composes it, the Creator over it while its
/// session is open, the rite's invitation over it while the rite is owed.
struct Reel {
    dir: std::path::PathBuf,
    n: usize,
    session: Session,
    invite_since: u64,
    ids: Vec<u32>,
}

impl Reel {
    fn new(dir: std::path::PathBuf, run: &Run) -> Reel {
        Reel { dir, n: 0, session: Session::default(), invite_since: run.tick, ids: vec![1] }
    }

    fn view(&self, run: &Run) -> Img {
        let (w, h) = seat();
        in_seat(|| {
            let mut img = frame_for(run, w, view_rows(h), Some(1));
            crate::render::genie::paint(&mut img, run, Some(1));
            crate::render::creation::paint_with(&mut img, &self.session, run.tick, None, None);
            let owed = run.owes_rite(1);
            let age = run.tick.saturating_sub(self.invite_since);
            crate::render::creation::paint_invite(&mut img, owed.then_some((age, age < 10 * 30)), self.session.open);
            let map = MapView { full: false, model: None, party: None };
            seat_lift_frame_with(&img, run, Some(1), &map)
        })
    }

    fn shoot(&mut self, run: &Run) {
        let img = self.view(run);
        image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.rgba_scaled(1)).unwrap().save(self.dir.join(format!("{:04}.png", self.n))).unwrap();
        self.n += 1;
    }

    /// `frames` frames, two ticks apiece (fifteen to the second).
    fn play(&mut self, run: &mut Run, frames: usize) {
        for _ in 0..frames {
            run.step(&keys(&self.ids));
            run.step(&keys(&self.ids));
            self.shoot(run);
        }
    }

    fn act(&mut self, run: &mut Run, action: Action, hold: usize) {
        self.session.action(action, run.tick);
        self.play(run, hold);
    }

    fn say(&mut self, run: &mut Run, words: &str, hold: usize) {
        self.session.say(words, run.tick);
        self.play(run, hold);
    }

    /// Walk knight `id` to `to` (tiles) at `speed` units a frame.
    fn walk(&mut self, run: &mut Run, id: u32, to: (f32, f32), speed: f32) {
        loop {
            let h = &run.players[&id];
            let (dx, dy) = (to.0 * 2.0 - h.x, to.1 * 2.0 - h.y);
            let d = dx.hypot(dy);
            if d < speed {
                break;
            }
            for _ in 0..2 {
                let h = run.players.get_mut(&id).unwrap();
                h.x += dx / d * speed / 2.0;
                h.y += dy / d * speed / 2.0;
                run.step(&keys(&self.ids));
            }
            self.shoot(run);
        }
    }

    /// The page the Creator is on, with `choice` taken from the role page, then through to Life, sealed.
    fn make(&mut self, run: &mut Run, role: Role, wish: &str) {
        self.session.open(run.tick, "Bors");
        self.play(run, 14);
        self.act(run, Action::Row(Role::ALL.iter().position(|&r| r == role).unwrap()), 8);
        self.say(run, wish, 22);
        for _ in 0..4 {
            if self.session.step == Step::Life {
                break;
            }
            self.act(run, Action::Confirm, 10);
        }
        while self.session.step != Step::Life {
            self.act(run, Action::Confirm, 6);
        }
        self.act(run, Action::Confirm, 30);
        assert!(self.session.granted.is_some(), "sealed: {:?}", self.session.issue);
    }

    /// What the seat sends its host, and what the host does with it: brought to life for the realm.
    fn bring_to_life(&mut self, run: &mut Run) -> String {
        let made = if let Some(w) = self.session.sealed_weapon.is_some().then(|| self.session.weapon.clone()) {
            Made::Weapon(w)
        } else {
            Made::Creature(self.session.creation.clone())
        };
        let line = run.contribute(1, made).expect("the realm keeps it");
        self.act(run, Action::Close, 4);
        line
    }
}

fn newcomer() -> Run {
    let mut run = realm_with("Bors", Home::default());
    run.walk_alehouse();
    run.floor_plates.clear();
    for _ in 0..10 {
        run.step(&BTreeMap::new());
    }
    run
}

#[test]
#[ignore]
fn write_creator_rite_films() {
    let Some(dir) = film_dir("creator-rite") else { return };
    let root = dir.parent().unwrap().join("creator-rite");
    let _ = root;
    // 1. A newcomer arrives in the bar; the Creator pops up; they make a barman.
    let mut run = newcomer();
    assert!(run.owes_rite(1) && run.room().kind == RoomKind::Alehouse);
    let mut reel = Reel::new(dir.join("barman"), &run);
    std::fs::create_dir_all(&reel.dir).unwrap();
    reel.play(&mut run, 40);
    reel.make(&mut run, Role::Folk, "a barman who stands behind the bar and says what will it be traveller");
    let line = reel.bring_to_life(&mut run);
    eprintln!("{line}");
    assert!(!run.owes_rite(1));
    let at = crate::rules::made::placed(&run, RoomKind::Alehouse, run.tick)[0].at;
    reel.play(&mut run, 20);
    reel.walk(&mut run, 1, (at.0 + 1.5, at.1 + 3.5), 0.6);
    reel.walk(&mut run, 1, (at.0 + 1.0, at.1 + 1.2), 0.4);
    // E beside him: he speaks.
    run.step(&BTreeMap::from([(1, Input { interact: true, ..Input::default() })]));
    reel.play(&mut run, 60);
    // 2. Then a minion, met later on a delve floor.
    let mut reel = Reel::new(dir.join("minion"), &run);
    std::fs::create_dir_all(&reel.dir).unwrap();
    reel.play(&mut run, 12);
    reel.make(&mut run, Role::Minion, "a tiny pet imp with ember palms, call it Pip");
    let line = reel.bring_to_life(&mut run);
    eprintln!("{line}");
    reel.play(&mut run, 10);
    // 3. Then a weapon on the wall.
    let mut reel3 = Reel::new(dir.join("weapon"), &run);
    std::fs::create_dir_all(&reel3.dir).unwrap();
    reel3.play(&mut run, 10);
    reel3.make(&mut run, Role::Weapon, "a long flaming sword with a golden crossguard");
    let line = reel3.bring_to_life(&mut run);
    eprintln!("{line}");
    let gate = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Gate).unwrap();
    run.arrive(gate, crate::rules::world::COURTYARD_ARRIVAL);
    run.floor_plates.clear();
    reel3.play(&mut run, 20);
    reel3.walk(&mut run, 1, (22.0, 11.8), 0.6);
    reel3.play(&mut run, 50);
    // The minion is met when the party goes down: the first room of the first floor.
    let mut reel2 = Reel::new(dir.join("minion-delve"), &run);
    std::fs::create_dir_all(&reel2.dir).unwrap();
    run.rite = true;
    let floor = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Home).unwrap();
    let _ = floor;
    run.descend();
    reel2.play(&mut run, 70);
    assert!(run.hireling.as_ref().is_some_and(|h| h.made.is_some()), "met on the floor");
    eprintln!("{} frames", reel.n + reel2.n + reel3.n);
}

#[test]
#[ignore]
fn write_rite_room_maps() {
    let Some(dir) = film_dir("maps") else { return };
    for (name, run) in [("griffin", griffin(Home::default())), ("courtyard", yard(Home::default()))] {
        let mut im = whole(&run);
        for t in (0..60).step_by(2) {
            let ink = if t % 10 == 0 { 'O' } else { 'g' };
            im.line(t * 16, 0, t * 16, im.h, ink);
            im.line(0, t * 16, im.w, t * 16, ink);
        }
        image::RgbaImage::from_raw(im.w as u32, im.h as u32, im.rgba_scaled(1)).unwrap().save(dir.join(format!("{name}.png"))).unwrap();
        eprintln!("{name}: {}x{}", im.w, im.h);
    }
}
