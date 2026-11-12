use super::*;
use crate::render::overworld::ink::PALETTE;
use crate::render::world3d::raster::{self, Backdrop, Firelight};
use crate::rules::{Run, deathmatch as dm};

fn is_ink(c: [u8; 3]) -> bool {
    c == [0, 0, 0] || PALETTE.iter().any(|&p| [(p >> 16) as u8, (p >> 8) as u8, p as u8] == c)
}

fn look(x: f32, y: f32, yaw: f32) -> View3 {
    View3 { pos: v3(x, y, 1.35), heading_rad: yaw, pitch: 0.0, fov_rad: 1.45 }
}

fn raster_of(view: &View3) -> (image::RgbaImage, Vec<f32>) {
    raster::render_meshes(&[mesh()], view, 192, 90, Firelight::STEADY, 24., Backdrop::Dark([8, 8, 9]))
}

fn painted(view: &View3, ticks: u32) -> crate::render::overworld::ink::Img {
    let (rgba, depth) = raster_of(view);
    paint(rgba.as_raw(), &depth, view, ticks, 0.0, &[], &[], 192, 90)
}

#[test]
fn the_pit_has_a_face_for_every_edge_of_the_map() {
    let pit = pit();
    let mut edges = 0;
    for r in 0..ROWS as i32 {
        for c in 0..=COLS as i32 {
            if raw_wall(c - 1, r) != raw_wall(c, r) {
                edges += 1;
                assert_ne!(pit.xf[c as usize * ROWS + r as usize], NONE, "x plane {c} row {r}");
            }
        }
    }
    for c in 0..COLS as i32 {
        for r in 0..=ROWS as i32 {
            if raw_wall(c, r - 1) != raw_wall(c, r) {
                edges += 1;
                assert_ne!(pit.yf[r as usize * COLS + c as usize], NONE, "y plane {r} col {c}");
            }
        }
    }
    assert!(edges > 60);
    // A gate in each end's notch, a box at each end of the long wall, pillars inside.
    let kinds = |k: Kind| pit.faces.iter().filter(|f| f.kind == k).count();
    assert_eq!(kinds(Kind::Gate), 2);
    assert_eq!(kinds(Kind::Box), 2);
    assert!(kinds(Kind::Pillar) >= 20);
    assert!(pit.faces.iter().all(|f| f.hi > f.lo && f.h >= PILLAR_H));
}

#[test]
fn every_dot_of_the_pit_is_one_of_the_realms_inks() {
    for (x, y, yaw) in [(13.0, 23.0, 0.0), (24.0, 18.0, 1.0), (10.0, 17.0, 3.1), (40.0, 8.0, -2.0), (9.0, 9.0, 0.6)] {
        let im = painted(&look(x, y, yaw), 100);
        for py in 0..im.h {
            for px in 0..im.w {
                let c = im.get(px, py).expect("every dot painted");
                assert!(is_ink(c), "{c:?} at {px},{py} from ({x},{y})");
            }
        }
    }
}

#[test]
fn the_paint_is_a_pure_function_of_the_view_and_the_tick() {
    let v = look(13.0, 23.0, 0.4);
    let (a, b) = (painted(&v, 77), painted(&v, 77));
    assert_eq!(a.content_hash(), b.content_hash());
    assert_ne!(a.content_hash(), painted(&v, 91).content_hash(), "the braziers and the crowd move");
}

#[test]
fn walls_and_sand_stay_darker_than_the_knights_who_stand_against_them() {
    // Knights are toned up to 175; nothing in the pit but a flame may rival them.
    for (x, y, yaw) in [(13.0, 23.0, 0.0), (24.0, 18.0, 1.0), (8.0, 14.0, 3.0), (30.0, 28.0, -1.5)] {
        let im = painted(&look(x, y, yaw), 40);
        let (mut bright, mut total) = (0, 0);
        for py in 0..im.h {
            for px in 0..im.w {
                let c = im.get(px, py).unwrap();
                let l = (u32::from(c[0]) * 3 + u32::from(c[1]) * 5 + u32::from(c[2]) * 2) / 10;
                total += 1;
                bright += i32::from(l > 150);
            }
        }
        assert!(bright * 10 < total, "{bright} of {total} dots are brighter than a knight at ({x},{y})");
    }
}

#[test]
fn the_sand_is_warm_and_the_sky_is_not() {
    let im = painted(&look(24.0, 18.0, 0.0), 10);
    // Bottom row of the view is sand: warmer than it is blue.
    let mut warm = 0;
    for px in 0..im.w {
        let c = im.get(px, im.h - 1).unwrap();
        warm += i32::from(c[0] > c[2]);
    }
    assert!(warm > im.w / 2, "{warm} warm dots on the sand");
}

#[test]
fn painting_a_six_seat_frame_is_cheap() {
    let views: Vec<View3> = (0..6).map(|k| look(10.0 + k as f32 * 5.0, 18.0, k as f32)).collect();
    let rasters: Vec<_> = views.iter().map(raster_of).collect();
    let best = |mut f: Box<dyn FnMut()>| {
        (0..8)
            .map(|_| {
                let start = std::time::Instant::now();
                f();
                start.elapsed().as_secs_f64() * 1000.0 / 6.0
            })
            .fold(f64::MAX, f64::min)
    };
    let rast = best(Box::new(|| {
        for v in &views {
            std::hint::black_box(raster_of(v));
        }
    }));
    eprintln!("raster: {rast:.2} ms per seat");
    let per = best(Box::new(|| {
        for (v, (rgba, depth)) in views.iter().zip(&rasters) {
            std::hint::black_box(paint(rgba.as_raw(), depth, v, 12, 0.0, &[], &[], 192, 90));
        }
    }));
    eprintln!("paint: {per:.2} ms per seat");
    assert!(per < 40.0, "painting took {per:.1} ms a seat");
}

// --- the proof: frames of the pit from where a knight stands ---------------------

fn scene() -> Run {
    let mut r = crate::rules::roundtable::fixture(&dm::ADVENTURE);
    for id in 2..=6 {
        r.join_seat(id, &format!("Knight {id}"));
    }
    r.deathmatch = Some(dm::Session::new(&r.players, &r.book));
    let s = r.deathmatch.as_mut().unwrap();
    s.ticks = dm::TITLE + 60;
    for f in s.fighters.values_mut() {
        f.protection = 0;
    }
    r
}

#[test]
#[ignore = "writes PNG frames: set DELVE_PIT_SHOTS to the output directory"]
fn write_pit_shots() {
    let dir = std::path::PathBuf::from(std::env::var_os("DELVE_PIT_SHOTS").expect("set DELVE_PIT_SHOTS"));
    std::fs::create_dir_all(&dir).unwrap();
    let save = |im: &crate::render::overworld::ink::Img, name: &str| {
        let big = im.clone();
        let mut up = crate::render::overworld::ink::Img::black(big.w * 2, big.h * 2);
        up.stamp_times(&big, 0, 0, 2);
        image::save_buffer(dir.join(format!("{name}.png")), &up.rgba_bytes(), up.w as u32, up.h as u32, image::ColorType::Rgba8).unwrap();
    };
    // The bare pit, from many places, with the knights standing in it.
    let shots: &[(&str, (f32, f32), f32)] = &[
        ("a-centre-west", (24.0, 18.0), std::f32::consts::PI),
        ("b-centre-east", (24.0, 18.0), 0.0),
        ("c-centre-north", (24.0, 18.0), -std::f32::consts::FRAC_PI_2),
        ("d-centre-south", (24.0, 18.0), std::f32::consts::FRAC_PI_2),
        ("e-gate-close", (14.0, 17.0), std::f32::consts::PI),
        ("f-corner", (6.0, 6.0), 0.8),
        ("g-wall-close", (4.0, 26.0), std::f32::consts::PI),
        ("h-east-gate", (36.0, 17.5), 0.0),
        ("i-box", (24.0, 14.0), -std::f32::consts::FRAC_PI_2),
        ("j-pillars", (17.0, 17.0), -0.6),
        ("k-box-close", (24.0, 25.0), std::f32::consts::FRAC_PI_2),
        ("l-lane-east", (12.0, 18.0), 0.0),
        ("m-north-west", (38.0, 12.0), -2.4),
        ("n-north-box", (24.0, 10.0), -std::f32::consts::FRAC_PI_2),
        ("o-corner-far", (9.0, 30.0), -0.8),
        ("p-dais", (25.0, 27.0), -std::f32::consts::FRAC_PI_2),
        ("q-health", (14.0, 13.0), std::f32::consts::PI),
    ];
    for &(name, (x, y), yaw) in shots {
        let mut r = scene();
        let s = r.deathmatch.as_mut().unwrap();
        let f = s.fighters.get_mut(&1).unwrap();
        (f.hero.x, f.hero.y, f.yaw) = (x, y, yaw);
        let place = [(2u32, 5.0f32, 0.0f32), (3, 9.0, 1.2), (4, 14.0, -1.5), (5, 20.0, 0.4), (6, 28.0, -0.7)];
        for (id, dist, off) in place {
            let g = s.fighters.get_mut(&id).unwrap();
            (g.hero.x, g.hero.y) = (x + (yaw + off * 0.18).cos() * dist, y + (yaw + off * 0.18).sin() * dist);
            if !dm::open(g.hero.x, g.hero.y) {
                (g.hero.x, g.hero.y) = (x + yaw.cos() * 3.0, y + yaw.sin() * 3.0 + 0.5 * id as f32);
            }
        }
        let im = super::super::layers(&r, Some(1)).0;
        save(&im, name);
    }
    // Frame time: the whole view for six seats, painted.
    let r = scene();
    let start = std::time::Instant::now();
    for _ in 0..10 {
        for id in 1..=6 {
            std::hint::black_box(super::super::layers(&r, Some(id)));
        }
    }
    eprintln!("six-seat render: {:.2} ms per seat", start.elapsed().as_secs_f64() * 1000. / 60.);
}

fn cpu_ticks() -> f64 {
    let s = std::fs::read_to_string("/proc/self/stat").unwrap();
    let rest = &s[s.rfind(')').unwrap() + 2..];
    let f: Vec<&str> = rest.split_whitespace().collect();
    (f[11].parse::<f64>().unwrap() + f[12].parse::<f64>().unwrap()) * 10.0
}

#[test]
#[ignore = "measures the work a seat's frame costs"]
fn the_cpu_a_seat_costs() {
    let mut r = crate::rules::roundtable::fixture(&crate::rules::deathmatch::ADVENTURE);
    for id in 2..=6 {
        r.join_seat(id, &format!("Knight {id}"));
    }
    r.deathmatch = Some(crate::rules::deathmatch::Session::new(&r.players, &r.book));
    let s = r.deathmatch.as_mut().unwrap();
    s.ticks = crate::rules::deathmatch::TITLE + 60;
    for (id, f) in &mut s.fighters {
        f.protection = 0;
        if *id == 1 {
            (f.hero.x, f.hero.y) = (13., 23.);
            f.yaw = 0.;
        }
    }
    let f = s.fighters.get_mut(&2).unwrap();
    (f.hero.x, f.hero.y) = (25., 23.);
    let first = std::time::Instant::now();
    std::hint::black_box(super::super::layers(&r, Some(1)));
    eprintln!("first frame (bakes the pit): {:.0} ms", first.elapsed().as_secs_f64() * 1000.0);
    for id in 1..=6 {
        std::hint::black_box(super::super::layers(&r, Some(id)));
    }
    let (c0, t0) = (cpu_ticks(), std::time::Instant::now());
    for _ in 0..60 {
        for id in 1..=6 {
            std::hint::black_box(super::super::layers(&r, Some(id)));
        }
    }
    let (cpu, wall) = (cpu_ticks() - c0, t0.elapsed().as_secs_f64() * 1000.0);
    eprintln!("seat frame: CPU {:.2} ms, wall {:.2} ms", cpu / 360.0, wall / 360.0);
}
