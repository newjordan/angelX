//! The magic on screen, frame by frame: each spell from its cast to its
//! last ember, an ultimate, the genie's lamp rubbed and a wish landing,
//! the fairies paying out in the Gnome's Garden, and a row of wished arms
//! firing, each in the look its own words give it.
//!
//! `ANGEL_ARENA_SHOTS=<dir> cargo test write_magic_shots -- --ignored`
//! writes a folder a scene, a PNG a frame (a window of the native room,
//! three times its size) and the whole room at twice its size for the key
//! moments. `write_magic2_shots` does the same for every knight's
//! ultimate, what ails a monster or a knight, and wished arms that kill.

use super::super::*;
use crate::rules::{Input, Run, phrasebook};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// The window round a scene's middle, in native pixels.
const CROP: (i32, i32) = (192, 120);
/// How much bigger a window is saved.
const K: u32 = 3;

struct Film {
    dir: PathBuf,
}

impl Film {
    fn scene(&self, name: &str) -> PathBuf {
        let dir = self.dir.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The window round arena point `at` (and the whole room, `whole`).
    fn frame(&self, run: &Run, scene: &str, n: usize, at: (f32, f32), whole: bool) {
        let dir = self.scene(scene);
        let img = frame_for(run, NATIVE_W, NATIVE_H, Some(1));
        let (ox, oy) = view_origin(run, Some(1));
        let (cx, cy) = ((at.0 * UNIT) as i32 - ox, (at.1 * UNIT) as i32 - oy);
        let x0 = (cx - CROP.0 / 2).clamp(0, (img.w - CROP.0).max(0));
        let y0 = (cy - CROP.1 / 2).clamp(0, (img.h - CROP.1).max(0));
        let crop = Img::from_view(&img, x0, y0, CROP.0, CROP.1);
        save(&crop, K, &dir.join(format!("{n:03}.png")));
        if whole {
            save(&img, 2, &dir.join(format!("whole-{n:03}.png")));
        }
    }
}

fn save(img: &Img, k: u32, path: &std::path::Path) {
    let rgba = img.rgba_scaled(k);
    image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, rgba).unwrap().save(path).unwrap();
}

/// A fight room with three skeletons standing in a loose line east of
/// the knight, who stands west of the middle facing them; the room's own
/// monsters are gone, and nothing has been shot yet.
fn range(knight: usize) -> Run {
    let mut run = Run::new(11, 1, Some("Knight"));
    let i = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && !r.great()).unwrap();
    run.dungeon.rooms[i].roster = vec![EnemyKind::Skeleton; 3];
    run.enter_for_test(i);
    run.outfit(1, &crate::rules::knights::COMPANY[knight]);
    for _ in 0..60 {
        hold(&mut run);
        run.step(&BTreeMap::new());
    }
    run.enemies.truncate(3);
    run.projectiles.clear();
    run
}

/// Where the knight stands on the range, and where the skeletons stand.
fn marks(run: &Run) -> ((f32, f32), [(f32, f32); 3]) {
    let (w, h) = (run.room().width(), run.room().height());
    let me = (w * 0.3, h * 0.55);
    (me, [(me.0 + 11.0, me.1 - 3.0), (me.0 + 13.0, me.1), (me.0 + 10.0, me.1 + 3.5)])
}

/// Everyone back on their marks, the knight whole, no monster's shot in
/// the air: the range keeps still between frames.
fn hold(run: &mut Run) {
    let (me, foes) = marks(run);
    if let Some(h) = run.players.get_mut(&1) {
        (h.x, h.y) = me;
        h.hp = h.max_hp;
    }
    for (e, at) in run.enemies.iter_mut().zip(foes) {
        (e.x, e.y) = at;
        e.age = e.age.max(100);
    }
    run.projectiles.retain(|p| !p.hostile);
}

fn step_with(run: &mut Run, input: Input) {
    hold(run);
    run.step(&BTreeMap::from([(1, input)]));
}

/// The wishes these shots grant: the phrasebook's own spells, and wishes
/// in players' words with five tiers each.
const WISHES: &str = "\
wish frost-nova | Frost Nova
kind spell
say frost nova
tier nova 40; cooldown 5 :: Frost bursts off you.

wish holy-ward | Holy Ward
kind spell
say holy ward
tier ward 3; cooldown 5 :: A ward of gold.

wish poop-worms | Poop Worms
say poop worms
tier damage 5 :: Worms of it.
tier damage 10 :: More worms.
tier damage 15 :: Many worms.
tier damage 20 :: A plague of worms.
tier damage 25 :: The worms are legion.

wish purple-drank-armor | Purple Drank Armor
say purple drank armor
tier armor 1 :: A sip.
tier armor 1; max_hp 10 :: A cup.
tier armor 2; max_hp 10 :: A jug.
tier armor 2; max_hp 20 :: A barrel.
tier armor 2; max_hp 30 :: The whole well.

wish buttcheek-bullets | Buttcheek Bullets
say buttcheek bullets
tier damage 5; size 1 :: Cheeky.
tier damage 10; size 1 :: Cheekier.
tier damage 15; size 1 :: Cheekiest.
tier damage 20; size 2 :: A full moon.
tier damage 25; size 2 :: Two full moons.

wish ice-shatter | Ice Shatter
say ice shatter
tier chill 1 :: Cold shots.
tier chill 1; damage 5 :: Colder.
tier chill 2; damage 5 :: Frozen.
tier chill 2; damage 10 :: Glacier.
tier chill 2; damage 15 :: The long winter.

wish life-steal-bullets | Life Steal Bullets
say life steal bullets
tier vamp 1 :: A sip of life.
tier vamp 1; damage 5 :: A draught.
tier vamp 2; damage 5 :: A feast.
tier vamp 2; damage 10 :: The red thirst.
tier vamp 2; damage 15 :: The crimson court.

wish storm-bolts | Storm Bolts
say lightning bullets
tier chain 1 :: Sparks.
tier chain 1; damage 5 :: Lightning.
tier chain 1; damage 10 :: Thunder.

wish holy-arrows | Holy Arrows
say holy arrows
tier pierce 1 :: Blessed.
tier pierce 1; damage 5 :: Sanctified.
tier pierce 2; damage 10 :: Divine.

wish dragon-fire | Dragon Fire
say dragon fire
tier burn 1 :: Hot.
tier burn 1; damage 5 :: Hotter.
tier burn 1; damage 10 :: Dragon hot.
tier burn 1; damage 15 :: The furnace.
tier burn 1; damage 20 :: The sun itself.

wish bee-swarm | Bee Swarm
say bee swarm shots
tier homing 1 :: Buzz.
tier homing 2 :: Buzz buzz.

wish shadow-darts | Shadow Darts
say shadow darts
tier pierce 1 :: From the dark.

wish lucky-coins | Lucky Coin Shot
say gold coin bullets
tier damage 5 :: Coins.

wish bubble-gun | Bubble Gun
say bubble gun
tier knock 1 :: Pop.

wish rose-petals | Rose Petal Storm
say rose petals
tier damage 5 :: Petals.

wish comet-lance | Comet Lance
say comet lance
tier damage 5; vfx trail stars arcane spiral count=5 wobble=3 spin=4; vfx fire rays holy back count=8 speed=4; vfx hit glyph arcane back speed=6; vfx hit stars genie count=14 speed=5 life=18; vfx kill ring holy back speed=8 :: A lance of starlight, written by hand.

wish song-shots | Song Shots
say music note bullets
tier damage 5 :: La.
";

fn wish(id: &str) -> phrasebook::Wish {
    let (read, faults) = phrasebook::read_checked(WISHES);
    phrasebook::get(id)
        .or_else(|| read.into_iter().find(|w| w.id == id))
        .unwrap_or_else(|| panic!("no wish {id}: {faults:?}"))
}

/// Grant `id` to knight `seat` `times` times (one tier a grant).
fn grant(run: &mut Run, seat: u32, id: &str, times: u32) {
    let w = wish(id);
    for _ in 0..times {
        let _ = run.grant(seat, &w);
    }
}

/// A spell wished, cast at the skeletons, filmed every `every` ticks for
/// `ticks`.
fn spell(film: &Film, scene: &str, id: &str, knight: usize, ticks: u32, every: u32) {
    let mut run = range(knight);
    grant(&mut run, 1, id, 1);
    if run.players[&1].spells.iter().all(Option::is_none) {
        eprintln!("{scene}: no spell from {id}");
        return;
    }
    let slot = run.players[&1].spells.iter().position(Option::is_some).unwrap() as u8 + 1;
    if let Some(h) = run.players.get_mut(&1) {
        h.hp = h.max_hp / 2;
    }
    let (me, foes) = marks(&run);
    let mid = ((me.0 + foes[1].0) / 2.0, me.1);
    step_with(&mut run, Input { aim_x: 1, cast: slot, ..Default::default() });
    for t in 0..ticks {
        if t % every == 0 {
            film.frame(&run, scene, t as usize, mid, t == 6);
        }
        step_with(&mut run, Input::default());
    }
}

/// A wished arm firing at the skeletons: a burst of shots, and the hits.
fn fire(film: &Film, scene: &str, id: &str, tier: u32) {
    let mut run = range(0);
    grant(&mut run, 1, id, tier);
    let (me, foes) = marks(&run);
    let mid = ((me.0 + foes[1].0) / 2.0, me.1);
    for t in 0..30u32 {
        if t % 3 == 0 {
            film.frame(&run, scene, t as usize, mid, t == 15);
        }
        step_with(&mut run, Input { aim_x: 1, fire: t < 20, ..Default::default() });
    }
}

#[test]
#[ignore]
fn write_magic_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let film = Film { dir: PathBuf::from(dir) };
    let only = std::env::var("ANGEL_MAGIC_ONLY").unwrap_or_default();
    let wanted = |s: &str| only.is_empty() || only.split(',').any(|o| s.starts_with(o));
    // The spells, cast and landed.
    for (scene, id, knight, ticks) in [
        ("spell-firebolt", "firebolt", 0usize, 32u32),
        ("spell-spark-ring", "spark-ring", 0, 32),
        ("spell-falling-star", "falling-star", 0, 48),
        ("spell-mending-light", "mending-light", 0, 40),
        ("spell-frost-nova", "frost-nova", 0, 36),
        ("spell-holy-ward", "holy-ward", 0, 40),
    ] {
        if wanted(scene) {
            spell(&film, scene, id, knight, ticks, 2);
        }
    }
    if wanted("thunder-scroll") {
        let mut run = range(0);
        if let Some(h) = run.players.get_mut(&1) {
            h.hand = vec!["thunder-scroll".into()];
        }
        let (me, foes) = marks(&run);
        let mid = ((me.0 + foes[1].0) / 2.0, me.1);
        step_with(&mut run, Input { aim_x: 1, play: 1, ..Default::default() });
        for t in 0..30u32 {
            if t % 2 == 0 {
                film.frame(&run, "thunder-scroll", t as usize, mid, t == 4);
            }
            step_with(&mut run, Input::default());
        }
    }
    // An ultimate: Gareth's Trebuchet, and Lynette's Lady's Veil.
    for (scene, knight) in [("ult-trebuchet", 2usize), ("ult-ladys-veil", 1), ("ult-stillhour", 6)] {
        if !wanted(scene) {
            continue;
        }
        let mut run = range(knight);
        if let Some(h) = run.players.get_mut(&1) {
            h.ult_charge = crate::rules::ults::ULT_FULL;
        }
        let (me, foes) = marks(&run);
        let mid = ((me.0 + foes[1].0) / 2.0, me.1);
        step_with(&mut run, Input { aim_x: 1, ult: true, ..Default::default() });
        for t in 0..60u32 {
            if t % 3 == 0 {
                film.frame(&run, scene, t as usize, mid, t == 12);
            }
            step_with(&mut run, Input { aim_x: 1, ..Default::default() });
        }
    }
    if wanted("lamp") {
        lamp(&film);
    }
    if wanted("fairy") {
        fairy(&film);
    }
    // Wished arms, each from its own words.
    for id in [
        "poop-worms", "buttcheek-bullets", "ice-shatter", "life-steal-bullets", "storm-bolts", "holy-arrows",
        "dragon-fire", "bee-swarm", "shadow-darts", "lucky-coins", "bubble-gun", "rose-petals", "song-shots",
        "purple-drank-armor", "comet-lance",
    ] {
        let scene = format!("arm-{id}");
        if wanted(&scene) {
            fire(&film, &scene, id, 3);
        }
    }
    if wanted("tiers") {
        tiers(&film);
    }
}

/// The lamp: a knight at the altar rubs it, the wish goes out (the lamp
/// smokes), and it lands on them.
fn lamp(film: &Film) {
    let mut run = Run::new(11, 3, Some("Ada"));
    run.outfit(1, &crate::rules::knights::COMPANY[1]);
    for hero in run.players.values_mut() {
        hero.wishes = 3;
    }
    run.floor_plates.clear();
    for _ in 0..90 {
        run.step(&BTreeMap::new());
    }
    let Some((ax, ay)) = run.wish_altar() else {
        eprintln!("lamp: no altar");
        return;
    };
    if let Some(ada) = run.players.get_mut(&1) {
        ada.x = ax + 2.5;
        ada.y = ay + 3.0;
    }
    let at = (ax + 1.0, ay - 1.5);
    for _ in 0..10 {
        run.step(&BTreeMap::new());
    }
    let mut n = 0;
    film.frame(&run, "lamp-rub", n, at, true);
    run.step(&BTreeMap::from([(1, Input { interact: true, ..Default::default() })]));
    for t in 0..30u32 {
        n += 1;
        if t % 2 == 0 {
            film.frame(&run, "lamp-rub", n, at, t == 8);
        }
        run.step(&BTreeMap::new());
    }
    run.lamp_sent(1, "my arrows turn into fireballs");
    for t in 0..36u32 {
        if t % 3 == 0 {
            film.frame(&run, "lamp-smoke", t as usize, at, t == 18);
        }
        run.step(&BTreeMap::new());
    }
    let w = wish("dragon-fire");
    let name = run.grant(1, &w).unwrap_or_else(|_| "Dragon Fire".into());
    run.lamp_landed(1, "my arrows turn into fireballs", &name);
    for t in 0..66u32 {
        if t % 3 == 0 {
            film.frame(&run, "lamp-landed", t as usize, at, t == 6 || t == 24);
        }
        run.step(&BTreeMap::new());
    }
}

/// The Gnome's Garden held: the fairies pay every knight a fairy wish.
fn fairy(film: &Film) {
    use crate::rules::nooks::{self, Nook, gnome};
    let mut run = Run::new(41, 1, None);
    run.join_seat(2, "Seat 2");
    run.enemies.clear();
    run.dungeon.rooms.push(nooks::room(RoomKind::GnomeParty, (12, 12), 3));
    let index = run.dungeon.rooms.len() - 1;
    run.nook = Some(Nook { room: index, ..Default::default() });
    run.enter_for_test(index);
    run.enemies.clear();
    for _ in 0..90 {
        for (id, at) in [(1u32, (10.5f32, 9.0f32)), (2, (15.5, 9.0))] {
            if let Some(h) = run.players.get_mut(&id) {
                (h.x, h.y) = (at.0 * crate::rules::TILE_UNITS, at.1 * crate::rules::TILE_UNITS);
            }
        }
        run.step(&BTreeMap::new());
    }
    let garden = gnome::Garden { party: gnome::Party::Held, house: gnome::HOUSE_HP, ..Default::default() };
    if let Some(nook) = run.nook.as_mut() {
        nook.garden = Some(garden);
        nook.done = true;
    }
    for hero in run.players.values_mut() {
        hero.fairy_wishes += 1;
    }
    run.found = Some((run.tick, 0, gnome::HELD_LINE.into()));
    let at = (13.0 * crate::rules::TILE_UNITS, 7.0 * crate::rules::TILE_UNITS);
    for t in 0..90u32 {
        if t % 3 == 0 {
            film.frame(&run, "fairy-wish", t as usize, at, t == 9 || t == 36);
        }
        run.step(&BTreeMap::new());
    }
}

/// Wishes at each tier, I to V, on five knights in a row: what the
/// highest tiers wear about them.
fn tiers(film: &Film) {
    for id in ["dragon-fire", "ice-shatter", "poop-worms", "life-steal-bullets", "purple-drank-armor", "buttcheek-bullets"] {
        let mut run = Run::new(11, 5, Some("Ada"));
        for k in 2..=5u32 {
            run.join_seat(k, &format!("Seat {k}"));
        }
        let i = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && !r.great()).unwrap();
        run.dungeon.rooms[i].roster = vec![];
        run.enter_for_test(i);
        run.enemies.clear();
        let (w, h) = (run.room().width(), run.room().height());
        for k in 1..=5u32 {
            grant(&mut run, k, id, k);
            if let Some(hero) = run.players.get_mut(&k) {
                (hero.x, hero.y) = (w * 0.5 + (k as f32 - 3.5) * 3.0, h * 0.55);
                hero.invulnerable = 0;
            }
        }
        let scene = format!("tiers-{id}");
        // The grants' own flourishes go first.
        for _ in 0..70 {
            run.step(&BTreeMap::new());
        }
        for t in 0..36u32 {
            for k in 1..=5u32 {
                if let Some(hero) = run.players.get_mut(&k) {
                    (hero.x, hero.y) = (w * 0.5 + (k as f32 - 3.5) * 3.0, h * 0.55);
                }
            }
            if t % 3 == 0 {
                film.frame(&run, &scene, t as usize, (w * 0.5, h * 0.5), t == 18);
            }
            if t == 18 {
                // The same, for a viewer who keeps the screen calm.
                let calm = crate::render::viewer::Preferences { calm_screen: true };
                crate::render::viewer::with(calm, || film.frame(&run, &format!("{scene}-calm"), 18, (w * 0.5, h * 0.5), false));
            }
            // Walking, to see what trails them.
            let walk = Input { move_x: if (t / 12) % 2 == 0 { 1 } else { -1 }, ..Default::default() };
            run.step(&(1..=5u32).map(|k| (k, walk)).collect());
        }
    }
}

/// Everyone on given marks, by monster id: the knight at `me`, each foe
/// at its own place, the knight whole and no monster's shot in the air.
fn hold_at(run: &mut Run, me: (f32, f32), foes: &[(u32, (f32, f32))], whole: bool) {
    if let Some(h) = run.players.get_mut(&1) {
        (h.x, h.y) = me;
        h.hp = h.max_hp;
    }
    for e in run.enemies.iter_mut() {
        if let Some(&(_, at)) = foes.iter().find(|(id, _)| *id == e.id) {
            (e.x, e.y) = at;
            e.age = e.age.max(100);
            if whole {
                e.hp = e.max_hp;
            }
        }
    }
    run.projectiles.retain(|p| !p.hostile);
}

/// Each knight of the company casting their own ultimate at the
/// skeletons, from the call to its last ember.
fn ultimate(film: &Film, knight: usize, ticks: u32, every: u32) {
    use crate::rules::ults::{ULT_FULL, Ult};
    let ult = Ult::ALL[knight];
    let scene = format!("ult-{:02}-{}", knight, ult.word().replace('_', "-"));
    let mut run = range(knight);
    let (me, mut spots) = marks(&run);
    // The Butcher must have one close to take hold of; the Composer's
    // chord, Midas and the Lullaby reach a little nearer than the range.
    match ult {
        Ult::Dismember => spots[0] = (me.0 + 4.8, me.1),
        Ult::GrandChord | Ult::MidasTouch | Ult::Lullaby => {
            spots = [(me.0 + 6.0, me.1 - 3.0), (me.0 + 7.5, me.1), (me.0 + 5.5, me.1 + 3.0)];
        }
        _ => {}
    }
    // Who the ultimate itself moves is left where it puts them.
    let pin_knight = !matches!(ult, Ult::Bladewind | Ult::Desertion);
    let pin_foes = !matches!(ult, Ult::GrandChord);
    let foes: Vec<(u32, (f32, f32))> = run.enemies.iter().zip(spots).map(|(e, at)| (e.id, at)).collect();
    if let Some(h) = run.players.get_mut(&1) {
        h.ult_charge = ULT_FULL;
    }
    let mid = ((me.0 + spots[1].0) / 2.0, me.1);
    hold_at(&mut run, me, &foes, true);
    run.step(&BTreeMap::from([(1, Input { aim_x: 1, ult: true, ..Default::default() })]));
    for t in 0..ticks {
        if t % every == 0 {
            film.frame(&run, &scene, t as usize, mid, t == 6);
        }
        // Midas's gold is meant to take them: the rest stay whole.
        let here = run.players.get(&1).map_or(me, |h| (h.x, h.y));
        let pinned: Vec<(u32, (f32, f32))> = if pin_foes { foes.clone() } else { Vec::new() };
        hold_at(&mut run, if pin_knight { me } else { here }, &pinned, ult != Ult::MidasTouch);
        if !pin_foes {
            for e in run.enemies.iter_mut() {
                e.hp = e.max_hp;
            }
        }
        run.step(&BTreeMap::from([(1, Input { aim_x: 1, ..Default::default() })]));
    }
}

/// What ails them: monsters burning, chilled and poisoned; stunned, asleep
/// and bleeding; and three knights hexed, chilled and mired.
fn ailing(film: &Film) {
    use crate::rules::skills::{Ailment, Bane};
    for (scene, which) in [("ail-foes-burn-chill-poison", 0), ("ail-foes-stun-sleep-bleed", 1)] {
        let mut run = range(0);
        let (me, _) = marks(&run);
        let spots = [(me.0 + 6.0, me.1 - 2.0), (me.0 + 10.0, me.1 + 1.0), (me.0 + 14.0, me.1 - 2.0)];
        let foes: Vec<(u32, (f32, f32))> = run.enemies.iter().zip(spots).map(|(e, at)| (e.id, at)).collect();
        for t in 0..48u32 {
            hold_at(&mut run, me, &foes, true);
            let ids: Vec<u32> = foes.iter().map(|f| f.0).collect();
            run.works.banes.clear();
            run.champions.grips.clear();
            for (k, e) in run.enemies.iter_mut().enumerate() {
                (e.chilled, e.frozen, e.sleep_until) = (0, 0, 0);
                match (which, k) {
                    (0, 1) => e.chilled = 500,
                    (1, 0) => e.frozen = 500,
                    (1, 1) => e.sleep_until = run.tick + 500,
                    _ => {}
                }
            }
            if which == 0 {
                run.works.banes.push(Ailment { foe: ids[0], kind: Bane::Burn, owner: 1, left: 500, power: 0 });
                run.works.banes.push(Ailment { foe: ids[2], kind: Bane::Poison, owner: 1, left: 500, power: 0 });
            } else {
                run.champions.grips.push(crate::rules::champions::Grip { owner: 1, foe: ids[2], left: 60 + t % 60 });
            }
            if t % 2 == 0 {
                film.frame(&run, scene, t as usize, (me.0 + 10.0, me.1), t == 12);
            }
            run.step(&BTreeMap::new());
        }
    }
    // Three knights: hexed, chilled by the Lich, held in a bog.
    let mut run = Run::new(11, 3, Some("Ada"));
    run.join_seat(2, "Bo");
    run.join_seat(3, "Cy");
    let i = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && !r.great()).unwrap();
    run.dungeon.rooms[i].roster = vec![];
    run.enter_for_test(i);
    run.enemies.clear();
    let (w, h) = (run.room().width(), run.room().height());
    for t in 0..48u32 {
        for k in 1..=3u32 {
            if let Some(hero) = run.players.get_mut(&k) {
                (hero.x, hero.y) = (w * 0.5 + (k as f32 - 2.0) * 4.0, h * 0.55);
                hero.hp = hero.max_hp;
                hero.invulnerable = 0;
                match k {
                    1 => hero.hexed = crate::rules::hexer::HEX_IMMUNE + 40,
                    2 => hero.chilled = 300,
                    _ => hero.mired = 300,
                }
            }
        }
        if t % 2 == 0 {
            film.frame(&run, "ail-knights-hex-chill-mire", t as usize, (w * 0.5, h * 0.5), t == 12);
        }
        run.step(&BTreeMap::new());
    }
}

/// Every ailment on a monster and on a knight, side by side, a row a
/// frame: the marks alone over plain figures on the dark.
fn ail_sheet(film: &Film) {
    use super::super::ailments::{Ail, Body, mark};
    let skel = super::super::sprites::enemy(EnemyKind::Skeleton, 0);
    let knight = {
        let run = range(0);
        super::super::knights::portrait(&run.players[&1], 1)
    };
    let dir = film.scene("ail-sheet");
    for f in 0..24u64 {
        let tick = 600 + f * 2;
        let (cw, ch) = (Ail::ALL.len() as i32 * 36, 80);
        let mut cv = Img::new(cw, ch);
        for (i, &a) in Ail::ALL.iter().enumerate() {
            for (row, im) in [(0, &skel), (1, &knight)] {
                let feet = (i as i32 * 36 + 18, 34 + row * 40);
                cv.stamp(im, feet.0 - im.w / 2, feet.1 - im.h + 1);
                let mut pen = super::super::pfx::Pen::new(&mut cv, 400);
                let body = Body { feet: (feet.0 as f32, feet.1 as f32), h: im.h as f32, w: im.w as f32 * 0.8, seed: (i * 7 + row as usize) as u32 };
                mark(&mut pen, a, &body, tick);
            }
        }
        let mut out = Img::black(cw, ch);
        out.stamp(&cv, 0, 0);
        save(&out, 4, &dir.join(format!("{f:03}.png")));
    }
}

/// A wished arm firing at skeletons that fall to it: the kill layers.
fn fire_to_kill(film: &Film, scene: &str, id: &str, tier: u32) {
    let mut run = range(0);
    grant(&mut run, 1, id, tier);
    let (me, _) = marks(&run);
    let spots = [(me.0 + 7.0, me.1 - 1.0), (me.0 + 9.0, me.1 + 0.5), (me.0 + 11.0, me.1 - 0.5)];
    let foes: Vec<(u32, (f32, f32))> = run.enemies.iter().zip(spots).map(|(e, at)| (e.id, at)).collect();
    for e in run.enemies.iter_mut() {
        e.hp = 1;
    }
    let mid = ((me.0 + spots[1].0) / 2.0, me.1);
    for t in 0..42u32 {
        if t % 2 == 0 {
            film.frame(&run, scene, t as usize, mid, t == 14);
        }
        hold_at(&mut run, me, &foes, false);
        run.step(&BTreeMap::from([(1, Input { aim_x: 1, fire: t < 24, ..Default::default() })]));
    }
}

#[test]
#[ignore]
fn write_magic2_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let film = Film { dir: PathBuf::from(dir) };
    let only = std::env::var("ANGEL_MAGIC_ONLY").unwrap_or_default();
    let wanted = |s: &str| only.is_empty() || only.split(',').any(|o| s.starts_with(o));
    for knight in 0..crate::rules::ults::Ult::ALL.len() {
        use crate::rules::ults::Ult;
        let ult = Ult::ALL[knight];
        if !wanted(&format!("ult-{knight:02}")) && !wanted(&format!("ult-{}", ult.word().replace('_', "-"))) {
            continue;
        }
        let (ticks, every) = match ult {
            Ult::LadysVeil => (132, 4),
            Ult::Phantasm | Ult::Tombstone | Ult::AxeStorm | Ult::NeedleSquall | Ult::Lullaby => (100, 4),
            Ult::Assassinate | Ult::LastLight | Ult::SealedWrits | Ult::Earthsplitter | Ult::Dismember => (72, 3),
            _ => (54, 3),
        };
        ultimate(&film, knight, ticks, every);
    }
    if wanted("ail-sheet") {
        ail_sheet(&film);
    }
    if wanted("ail-foes") || wanted("ail-knights") {
        ailing(&film);
    }
    for id in ["dragon-fire", "ice-shatter", "poop-worms", "storm-bolts", "comet-lance", "life-steal-bullets", "lucky-coins", "holy-arrows"] {
        let scene = format!("kill-{id}");
        if wanted(&scene) {
            fire_to_kill(&film, &scene, id, 5);
        }
    }
}

/// A spell's shots carry its slot's mark and the breaks they make carry
/// it too; the knight's own shots loosed in the same breath carry none, so
/// only the spell's are drawn as the spell's.
#[test]
fn a_spells_shots_and_breaks_carry_its_mark_and_the_knights_own_do_not() {
    let mut run = range(0);
    grant(&mut run, 1, "firebolt", 1);
    let slot = run.players[&1].spells.iter().position(Option::is_some).expect("firebolt is a spell") as u8 + 1;
    step_with(&mut run, Input { aim_x: 1, cast: slot, fire: true, ..Default::default() });
    let mine: Vec<_> = run.projectiles.iter().filter(|p| !p.hostile && p.traits.owner == 1).collect();
    assert!(mine.iter().any(|p| p.traits.spell == slot), "the spell's bolt is marked");
    for _ in 0..12 {
        step_with(&mut run, Input { aim_x: 1, fire: true, ..Default::default() });
    }
    assert!(run.projectiles.iter().filter(|p| !p.hostile && p.traits.spell == 0).any(|p| p.traits.owner == 1), "the knight's own shots fly too");
    assert!(run.projectiles.iter().all(|p| p.traits.spell == 0 || p.traits.spell == slot));
    for e in &run.feel.ranged {
        if e.mark != crate::rules::ranged::Mark::Release && e.spell == slot {
            assert_eq!(e.kind, crate::rules::Shot::Bolt, "only the spell's own breaks carry its mark");
        }
    }
}
