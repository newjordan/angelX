//! Every class has a figure of its own, in every facing, walking and
//! striking, in its seat's trim.
use super::super::super::ink::{Img, ink};
use super::super::knights;
use super::*;
use crate::rules::{Run, classes::CLASSES};

const FACINGS: [Facing; 3] = [Facing::Down, Facing::Side, Facing::Up];

fn seated(class: &str) -> (Run, u32) {
    let mut run = Run::new(7, 1, Some("Friend"));
    let at = CLASSES.iter().find(|c| c.name == class).unwrap();
    run.become_class(1, at);
    (run, 1)
}

fn same(a: &Img, b: &Img) -> bool {
    a.w == b.w && a.h == b.h && (0..a.h).all(|y| (0..a.w).all(|x| a.get(x, y) == b.get(x, y)))
}

#[test]
fn every_class_in_the_halls_has_a_look() {
    for class in &CLASSES {
        let look = Look::named(class.name).unwrap_or_else(|| panic!("{} has no look", class.name));
        assert_eq!(look.name(), class.name);
        assert_eq!(Look::of_kit(class.kit), Some(look), "{}'s kit", class.name);
    }
    assert_eq!(Look::ALL.len(), CLASSES.len());
}

#[test]
fn each_class_picks_its_own_sprite_set() {
    let knight: Vec<String> = (0..16)
        .map(|y| {
            let k = super::super::super::kit::knight();
            (0..16).map(|x| if k.get(x, y).is_some() { '#' } else { '.' }).collect()
        })
        .collect();
    for (i, a) in Look::ALL.iter().enumerate() {
        let (run, id) = seated(a.name());
        let hero = &run.players[&id];
        assert_eq!(Look::of(hero), Some(*a), "{} after the hall", a.name());
        // The arena draws them as their class, not as the knight.
        let drawn = knights::figure_with(hero, id, false);
        assert!(same(&drawn, &figure(*a, knights::facing(hero.aim_x, hero.aim_y), pose(hero), true).recolor(knights::seat_colours(id))));
        for (j, b) in Look::ALL.iter().enumerate().skip(i + 1) {
            for view in 0..3 {
                assert_ne!(a.bodies()[view], b.bodies()[view], "{} and {} share a body", a.name(), b.name());
            }
            assert!(
                !same(&figure(*a, Facing::Down, Pose::Stand, true), &figure(*b, Facing::Down, Pose::Stand, true)),
                "{i} {j}"
            );
        }
        // No class's silhouette is the knight's.
        let body = Img::from_rows(a.bodies()[0]);
        let shape: Vec<String> = (0..body.h.min(16))
            .map(|y| (0..16).map(|x| if body.get(x, y).is_some() { '#' } else { '.' }).collect())
            .collect();
        assert_ne!(shape, knight, "{} is the knight's shape", a.name());
    }
}

#[test]
fn every_drawing_is_in_the_realm_inks_and_square_on_its_body() {
    for look in Look::ALL {
        for view in look.views() {
            for layer in [view.body, view.item.rows, view.strike.rows] {
                for row in layer {
                    for c in row.chars().filter(|&c| c != '.') {
                        assert!(ink(c).is_some(), "{}: {c:?} is no ink", look.name());
                    }
                }
            }
            assert!(view.body.iter().all(|r| r.chars().count() == 16), "{}", look.name());
        }
    }
}

#[test]
fn facings_steps_and_strikes_are_drawn_apart() {
    for look in Look::ALL {
        let stand: Vec<Img> = FACINGS.iter().map(|&f| figure(look, f, Pose::Stand, true)).collect();
        for (k, f) in FACINGS.iter().enumerate() {
            assert_eq!(stand[k].w, W, "{} keeps its feet centred", look.name());
            for (m, other) in stand.iter().enumerate().skip(k + 1) {
                assert!(!same(&stand[k], other), "{}: facings {k} and {m}", look.name());
            }
            let a = figure(look, *f, Pose::Step(0), true);
            let b = figure(look, *f, Pose::Step(1), true);
            let hit = figure(look, *f, Pose::Strike, true);
            assert!(!same(&a, &stand[k]) && !same(&a, &b), "{} {f:?} walks", look.name());
            assert!(!same(&hit, &stand[k]), "{} {f:?} strikes", look.name());
            assert_eq!(hit.w, W);
        }
    }
}

#[test]
fn the_seat_is_the_trim() {
    for look in Look::ALL {
        let (mut run, _) = seated(look.name());
        let hero = run.players[&1].clone();
        run.players.insert(2, hero);
        let one = knights::figure_with(&run.players[&1], 1, false);
        let two = knights::figure_with(&run.players[&2], 2, false);
        assert!(!same(&one, &two), "{}: two seats look alike", look.name());
        // Only the trim differs: wherever they differ, seat one wears its red.
        let red = [ink('7'), ink('8'), ink('5')];
        for y in 0..one.h {
            for x in 0..one.w {
                if one.get(x, y) != two.get(x, y) {
                    assert!(red.contains(&one.get(x, y)), "{} at {x},{y}", look.name());
                }
            }
        }
    }
}

#[test]
fn the_posse_stays_the_realms_knight() {
    let mut run = Run::new(7, 1, Some("Friend"));
    run.players.get_mut(&1).unwrap().knight = Some("composer".into());
    assert_eq!(Look::of(&run.players[&1]), None);
    assert_eq!(knights::figure_with(&run.players[&1], 1, false).w, 16);
}


#[test]
fn every_class_rides_and_drives_in_its_own_head() {
    let mut heads = Vec::new();
    for look in Look::ALL {
        let (head, coat) = saddle(look, 2);
        assert_eq!(head.w, 7, "{}", look.name());
        assert_eq!((coat.w, coat.h), (5, 8), "{}", look.name());
        // A plain rider, then the class's: the helm and tabard are gone.
        let mut im = Img::new(40, 40);
        im.rect(18, 15, 5, 8, '7');
        dress_rider(&mut im, (20, 22), look, 1);
        assert!(!same(&im, &{
            let mut plain = Img::new(40, 40);
            plain.rect(18, 15, 5, 8, '7');
            plain
        }));
        for facing in FACINGS {
            let small = driver_head(look, facing, 1);
            assert_eq!(small.w, 4);
            heads.push((look, facing, small));
        }
    }
    for (i, (a, f, x)) in heads.iter().enumerate() {
        for (b, g, y) in heads.iter().skip(i + 1) {
            if f == g {
                assert!(!same(x, y), "{} and {} drive alike {f:?}", a.name(), b.name());
            }
        }
    }
}

#[test]
fn class_shots_draw_and_others_do_not() {
    use crate::rules::classes::{ARCANE, HOLY};
    for colour in [ARCANE, HOLY] {
        let mut im = Img::new(25, 25);
        assert!(shot(&mut im, (12, 12), (1.0, 0.0), colour, 3));
        assert!(im.pixels().filter(|&c| c != [0, 0, 0]).count() > 10);
    }
    assert!(!shot(&mut Img::new(25, 25), (12, 12), (1.0, 0.0), "ember", 3));
}

/// Every class in every facing, standing, both steps and the strike, then
/// in each of the four seats' trim: `lineup.png` at six times, and
/// `lineup-1x.png` at the arena's own size.
/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_class_lineup -- --ignored`
#[test]
#[ignore]
fn write_class_lineup() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (cell_w, cell_h) = (W + 4, 30);
    let poses = [Pose::Stand, Pose::Step(0), Pose::Step(1), Pose::Strike];
    let cols = 4 * poses.len() as i32 + 1 + 4;
    let mut sheet = Img::new(cols * cell_w, Look::ALL.len() as i32 * cell_h);
    for (row, look) in Look::ALL.iter().enumerate() {
        let y = row as i32 * cell_h + cell_h - 2;
        let mut col = 0;
        for (facing, flip) in [(Facing::Down, false), (Facing::Side, false), (Facing::Side, true), (Facing::Up, false)] {
            for pose in poses {
                let mut im = figure(*look, facing, pose, true);
                if flip {
                    im = im.flip_h();
                }
                sheet.stamp(&im, col * cell_w + 2, y - im.h);
                col += 1;
            }
        }
        col += 1;
        for seat in 1..=4 {
            let im = figure(*look, Facing::Down, Pose::Stand, true).recolor(knights::seat_colours(seat));
            sheet.stamp(&im, col * cell_w + 2, y - im.h);
            col += 1;
        }
    }
    let save = |im: &Img, k: u32, name: &str| {
        let (w, h) = (im.w as u32 * k, im.h as u32 * k);
        let mut out = image::RgbaImage::from_raw(w, h, im.rgba_scaled(k)).unwrap();
        for p in out.pixels_mut() {
            if p[3] == 0 {
                *p = image::Rgba([0, 0, 0, 255]);
            }
        }
        out.save(dir.join(name)).unwrap();
    };
    save(&sheet, 6, "lineup.png");
    // As the upgrade canvas paints them, at its own size, two times up.
    let cell = (W * 3 + 8, 30 * 3);
    let mut big = Img::new(cell.0 * 13, cell.1 * Look::ALL.len() as i32);
    for (row, look) in Look::ALL.iter().enumerate() {
        let y = row as i32 * cell.1 + cell.1 - 2;
        let mut col = 0;
        for (facing, flip) in [(Facing::Down, false), (Facing::Side, false), (Facing::Side, true), (Facing::Up, false)] {
            for pose in [Pose::Stand, Pose::Step(0), Pose::Strike] {
                let mut im = figure(*look, facing, pose, true).recolor(knights::seat_colours(row as u32 % 4 + 1));
                if flip {
                    im = im.flip_h();
                }
                let im = super::super::art_v2::hd_figure(&im);
                big.stamp(&im, col * cell.0 + 2, y - im.h);
                col += 1;
            }
        }
    }
    save(&big, 2, "lineup-v2.png");
    save(&sheet, 1, "lineup-1x.png");
}
