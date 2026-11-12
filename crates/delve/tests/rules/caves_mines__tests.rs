//! Ore bodies, the prove line, and a sunk cave node.
use super::*;
use crate::rules::caves::mines::{self, PROVE_AT};
use crate::rules::caves::world::{self, MAP_H, MAP_W};
use crate::rules::home::Home;
use std::collections::{BTreeMap, BTreeSet};

fn clear_of(a: (i32, i32), b: (i32, i32)) -> i32 {
    (a.0 - b.0).abs().max((a.1 - b.1).abs())
}

fn mouths(seed: u32) -> Vec<(i32, i32)> {
    let mut out = Vec::new();
    for y in 0..MAP_H {
        for x in 0..MAP_W {
            if world::frost_mouth(x, y, seed) {
                out.push((x, y));
            }
        }
    }
    out
}

fn marks(seed: u32) -> Vec<(i32, i32)> {
    let mut out = vec![spawn_cell(seed), exit_cell(seed), grip_cell(seed), blades_cell(seed)];
    out.extend(mouths(seed));
    out
}

#[test]
fn eight_bodies_are_the_same_every_time_and_clear_of_the_landmarks() {
    for seed in [1u32, 11, 29, 99] {
        let a = mines::bodies(seed);
        let b = mines::bodies(seed);
        assert_eq!(a.bodies.len(), 8, "seed {seed}");
        assert_eq!(a.at, b.at, "seed {seed} is not stable");
        let mut per = [0u8; 4];
        for body in &a.bodies {
            per[body.region as usize] += 1;
            assert!(body.cells.len() >= 14 && body.cells.len() <= 24, "seed {seed} size {}", body.cells.len());
            assert_eq!(body.depth, if body.index % 2 == 0 { 1 } else { 2 });
            let head = body.head;
            let beside = body.cells.iter().any(|c| (c.0 - head.0).abs() + (c.1 - head.1).abs() == 1);
            assert!(beside, "seed {seed} head not beside its body");
            let cave = base(seed);
            let floor = cave.cave.at(head.0, head.1).is_some_and(|c| c.open());
            assert!(floor, "seed {seed} head is not open floor");
            for cell in &body.cells {
                assert!(clear_of(*cell, marks(seed).into_iter().min_by_key(|m| clear_of(*cell, *m)).unwrap()) > 6);
                assert_eq!(mines::vein_at(seed, cell.0, cell.1), Some(body.index));
            }
        }
        assert_eq!(per, [2, 2, 2, 2], "seed {seed} regions {per:?}");
    }
}

fn mark_seen(host: &mut Run, cells: &[(i32, i32)], n: usize) {
    for &(x, y) in cells.iter().take(n) {
        let cell = (x, y);
        if let Err(i) = host.caves.proven.binary_search(&cell) {
            host.caves.proven.insert(i, cell);
        }
    }
}

fn stand(host: &mut Run, id: u32, x: i16, y: i16) {
    let seat = host.caves.seats.get_mut(&id).unwrap();
    seat.x = x;
    seat.y = y;
}

fn press(host: &mut Run, id: u32) {
    let down = Input { interact: true, ..Input::default() };
    host.tick_caves(&BTreeMap::from([(id, down)]));
    host.tick_caves(&BTreeMap::from([(id, Input::default())]));
}

fn ready_kingdom(host: &mut Run) {
    let c = crate::kingdom::catalog();
    host.home.industry.found_hillside(c, host.seed);
    host.home.industry.researched = c.all_techs().map(|(name, _)| name.clone()).collect();
    for name in c.resources.keys() {
        host.home.industry.stock.insert(name.clone(), 40);
    }
    host.home.industry.stock.insert("fodder".into(), 100_000);
}

#[test]
fn a_body_is_not_a_mine_at_seven_proven_cells_and_is_at_eight() {
    let mut host = Run::new(11, 1, Some("Ash"));
    host.enter_one(1, false);
    ready_kingdom(&mut host);
    let seed = host.caves.seed;
    let body = mines::bodies(seed).bodies[0].clone();
    stand(&mut host, 1, body.head.0 as i16, body.head.1 as i16);
    mark_seen(&mut host, &body.cells, 7);
    press(&mut host, 1);
    assert!(host.home.mines.is_empty(), "seven proven cells sank a mine");
    mark_seen(&mut host, &body.cells, 8);
    assert!(mines::proven(&host.caves, &body) >= PROVE_AT);
    press(&mut host, 1);
    assert_eq!(host.home.mines.len(), 1, "{:?}", host.seat_notices);
    let sunk = host.home.mines.values().next().unwrap();
    assert_eq!(sunk.claim, format!("cave-{seed}-{}", body.index));
    assert_eq!(sunk.who, 1);
    let line = format!("A MINE IS SUNK: {} NODE {}", mines::stone_name(body.region).to_ascii_uppercase(), body.depth);
    assert!(host.seat_notices.iter().any(|(_, text)| text == &line), "{:?}", host.seat_notices);
}

#[test]
fn sinking_needs_the_press_on_or_beside_the_head_works_once_and_survives_a_save() {
    let mut host = Run::new(11, 1, Some("Ash"));
    host.enter_one(1, false);
    ready_kingdom(&mut host);
    let seed = host.caves.seed;
    let body = mines::bodies(seed).bodies[0].clone();
    mark_seen(&mut host, &body.cells, 8);
    stand(&mut host, 1, (body.head.0 + 3) as i16, body.head.1 as i16);
    press(&mut host, 1);
    assert!(host.home.mines.is_empty(), "a press away from the head sank a mine");
    stand(&mut host, 1, (body.head.0 + 1) as i16, body.head.1 as i16);
    let hold = Input { interact: true, ..Input::default() };
    host.tick_caves(&BTreeMap::from([(1, hold)]));
    assert_eq!(host.home.mines.len(), 1);
    host.tick_caves(&BTreeMap::from([(1, hold)]));
    assert_eq!(host.home.mines.len(), 1, "holding interact sank it again");
    let claim_key = host.home.mines.values().next().unwrap().claim.clone();
    let claim = host.home.industry.claims.get(&claim_key).unwrap();
    assert!(claim.secured);
    assert!(claim.surface);
    let seam = crate::rules::geology::seam_at(mines::pack_of(body.region), u32::from(body.depth));
    assert_eq!(claim.yields, seam.yields());
    let linked = host.home.industry.buildings.values().any(|b| b.plan == "claim-quarry" && b.claim.as_deref() == Some(claim_id(&host, body.index).as_str()));
    assert!(linked, "the quarry is not linked");
    let wire = serde_json::to_string(&host.home).unwrap();
    let back: Home = serde_json::from_str(&wire).unwrap();
    assert_eq!(back.cave_seed, host.home.cave_seed);
    assert_eq!(back.mines, host.home.mines);
    let old = host.home.cave_seed;
    let mut later = Run::new(99, 1, Some("Ash"));
    later.home.cave_seed = old;
    later.enter_one(1, false);
    assert_eq!(later.caves.seed, old as u32, "a later entry used a new cave");
}

fn claim_id(host: &Run, index: u8) -> String {
    format!("cave-{}-{index}", host.caves.seed)
}

#[test]
fn a_guest_snapshot_carries_the_sunk_mines() {
    let mut host = Run::new(11, 1, Some("Ash"));
    host.enter_one(1, false);
    ready_kingdom(&mut host);
    let body = mines::bodies(host.caves.seed).bodies[0].clone();
    stand(&mut host, 1, body.head.0 as i16, body.head.1 as i16);
    mark_seen(&mut host, &body.cells, 8);
    press(&mut host, 1);
    let snap = crate::wire::publish(&host).expect("publish");
    let text = format!("{snap:?}");
    let claim = host.home.mines.values().next().unwrap().claim.clone();
    assert!(text.contains(&claim), "the guest snapshot has no mine");
}

#[test]
fn stone_from_two_nodes_reaches_the_kingdom() {
    let mut host = Run::new(11, 1, Some("Ash"));
    host.enter_one(1, false);
    ready_kingdom(&mut host);
    let seed = host.caves.seed;
    let bodies = mines::bodies(seed);
    let first = bodies.bodies.iter().find(|b| b.region == 0).unwrap().clone();
    let second = bodies.bodies.iter().find(|b| b.region == 1).unwrap().clone();
    for body in [&first, &second] {
        mark_seen(&mut host, &body.cells, 8);
        stand(&mut host, 1, body.head.0 as i16, body.head.1 as i16);
        press(&mut host, 1);
    }
    assert_eq!(host.home.mines.len(), 2, "{:?}", host.seat_notices);
    let c = crate::kingdom::catalog();
    let k = &mut host.home.industry;
    for building in k.buildings.values_mut() {
        if building.plan == "claim-quarry" {
            building.construction = 0;
        }
    }
    let before = k.amount("limestone") + k.amount("flint");
    for _ in 0..120 {
        k.advance_second(c);
    }
    let after = k.amount("limestone") + k.amount("flint") + k.produced.get("limestone").copied().unwrap_or(0) + k.produced.get("flint").copied().unwrap_or(0);
    println!("cave nodes stock-or-made limestone+flint before {before} after {after}");
    let seam0 = crate::rules::geology::seam_at(mines::pack_of(0), 1);
    let day = seam0.rate() * 6;
    let evening = seam0.rate() * 30;
    println!("limestone node 1 at day pace {day} stone a minute, at evening pace {evening} stone a minute");
    assert!(after > before, "no stone arrived, notices {:?}", host.seat_notices);
}

fn px(img: &mut [u8], w: usize, x: usize, y: usize, rgb: [u8; 3]) {
    let i = (y * w + x) * 3;
    img[i..i + 3].copy_from_slice(&rgb);
}

fn write_png(path: &std::path::Path, w: u32, h: u32, rgb: &[u8]) {
    fn crc(data: &[u8]) -> u32 {
        let mut c = 0xffffffffu32;
        for &b in data {
            c ^= u32::from(b);
            for _ in 0..8 {
                c = if c & 1 == 1 { 0xedb88320 ^ (c >> 1) } else { c >> 1 };
            }
        }
        !c
    }
    fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut out = (data.len() as u32).to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        let sum = crc(&out[4..]);
        out.extend_from_slice(&sum.to_be_bytes());
        out
    }
    let mut raw = Vec::new();
    for y in 0..h as usize {
        raw.push(0);
        raw.extend_from_slice(&rgb[y * w as usize * 3..(y + 1) * w as usize * 3]);
    }
    let mut zlib = vec![0x78, 0x01];
    let mut i = 0;
    while i < raw.len() {
        let n = (raw.len() - i).min(65535);
        let last = i + n == raw.len();
        zlib.push(if last { 1 } else { 0 });
        zlib.extend_from_slice(&(n as u16).to_le_bytes());
        zlib.extend_from_slice(&(!n as u16).to_le_bytes());
        zlib.extend_from_slice(&raw[i..i + n]);
        i += n;
    }
    let mut s1 = 1u32;
    let mut s2 = 0u32;
    for &b in &raw {
        s1 = (s1 + u32::from(b)) % 65521;
        s2 = (s2 + s1) % 65521;
    }
    zlib.extend_from_slice(&((s2 << 16) | s1).to_be_bytes());
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.extend(chunk(b"IHDR", &ihdr));
    png.extend(chunk(b"IDAT", &zlib));
    png.extend(chunk(b"IEND", &[]));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, png).unwrap();
}

#[test]
#[ignore = "writes the ex1 proof map; run with --ignored"]
fn write_bodies_map() {
    let seeds = [11u32, 29];
    let scale = 2usize;
    let gap = 8usize;
    let margin = 4usize;
    let map_w = MAP_W as usize * scale;
    let map_h = MAP_H as usize * scale;
    let w = margin + map_w + gap + map_w + margin;
    let h = margin + map_h + margin;
    let mut img = vec![0u8; w * h * 3];
    let body_ink = [
        [220u8, 170, 40],
        [50, 110, 220],
        [210, 50, 40],
        [40, 180, 80],
    ];
    for (n, seed) in seeds.iter().enumerate() {
        let ox = margin + n * (map_w + gap);
        let cave = base(*seed);
        let laid = mines::bodies(*seed);
        for y in 0..MAP_H {
            for x in 0..MAP_W {
                let region = world::region_of(x, y).0 as usize;
                let rock = cave.cave.at(x, y).is_some_and(|c| c.wall());
                let mut rgb: [u8; 3] = if rock { [18, 18, 22] } else { [214, 208, 190] };
                rgb[region % 3] = rgb[region % 3].saturating_add(8);
                if let Some(index) = mines::vein_at(*seed, x, y) {
                    if let Some(body) = laid.bodies.iter().find(|body| body.index == index) {
                        rgb = body_ink[body.region as usize];
                    }
                }
                for dy in 0..scale {
                    for dx in 0..scale {
                        px(&mut img, w, ox + x as usize * scale + dx, margin + y as usize * scale + dy, rgb);
                    }
                }
            }
        }
        for body in &laid.bodies {
            let hx = ox + body.head.0 as usize * scale;
            let hy = margin + body.head.1 as usize * scale;
            for dy in 0..scale {
                for dx in 0..scale {
                    px(&mut img, w, hx + dx, hy + dy, [255, 0, 255]);
                }
            }
        }
        let spawn = spawn_cell(*seed);
        let exit = exit_cell(*seed);
        for (cell, ink) in [(spawn, [255, 0, 255]), (exit, [0, 255, 255])] {
            for dy in 0..scale {
                for dx in 0..scale {
                    px(&mut img, w, ox + cell.0 as usize * scale + dx, margin + cell.1 as usize * scale + dy, ink);
                }
            }
        }
    }
    let dir = std::env::temp_dir().join("delve-proof/ex1");
    std::fs::create_dir_all(&dir).unwrap();
    let path_buf = dir.join("bodies-map.png");
    let path = path_buf.as_path();
    write_png(path, w as u32, h as u32, &img);
    assert!(path.is_file());
}

#[test]
fn eight_sites_pass_the_kingdoms_own_build_and_do_not_overlap() {
    let mut host = Run::new(3, 1, Some("Ash"));
    ready_kingdom(&mut host);
    for name in crate::kingdom::catalog().resources.keys() {
        host.home.industry.stock.insert(name.clone(), 4000);
    }
    let mut covered = BTreeSet::new();
    for site in mines::SURFACE_SITE {
        for dy in 0..3 {
            for dx in 0..3 {
                let tile = ((site.0 + dx) as i16, (site.1 + dy) as i16);
                assert!(covered.insert(tile), "sites overlap at {tile:?}");
            }
        }
        let catalog = crate::kingdom::catalog();
        host.home
            .industry
            .apply(
                &catalog,
                crate::kingdom::Command::Build {
                    plan: "claim-quarry".into(),
                    tile: (site.0 as i16, site.1 as i16),
                },
            )
            .unwrap_or_else(|why| panic!("site {site:?} refused: {why}"));
    }
    assert_eq!(host.home.industry.buildings.values().filter(|b| b.plan == "claim-quarry").count(), 8);
}

#[test]
fn sinking_all_eight_bodies_raises_eight_linked_quarries() {
    let mut host = Run::new(11, 1, Some("Ash"));
    host.enter_one(1, false);
    ready_kingdom(&mut host);
    for name in crate::kingdom::catalog().resources.keys() {
        host.home.industry.stock.insert(name.clone(), 4000);
    }
    for name in host.home.industry.stock.keys().cloned().collect::<Vec<_>>() {
        host.home.industry.stock.insert(name, 4000);
    }
    let bodies = mines::bodies(host.caves.seed).bodies.clone();
    assert_eq!(bodies.len(), 8);
    for body in &bodies {
        mark_seen(&mut host, &body.cells, 8);
        stand(&mut host, 1, body.head.0 as i16, body.head.1 as i16);
        press(&mut host, 1);
        assert!(
            host.home.mines.contains_key(&body.index),
            "body {} was not sunk: {:?}",
            body.index,
            host.seat_notices
        );
    }
    assert_eq!(host.home.mines.len(), 8);
    let linked = host
        .home
        .industry
        .buildings
        .values()
        .filter(|b| b.plan == "claim-quarry" && b.claim.is_some())
        .count();
    assert_eq!(linked, 8);
}

#[test]
fn a_refused_quarry_keeps_the_claim_and_a_later_press_retries() {
    let mut host = Run::new(11, 1, Some("Ash"));
    host.enter_one(1, false);
    ready_kingdom(&mut host);
    for value in host.home.industry.stock.values_mut() {
        *value = 0;
    }
    let body = mines::bodies(host.caves.seed).bodies[0].clone();
    mark_seen(&mut host, &body.cells, 8);
    stand(&mut host, 1, body.head.0 as i16, body.head.1 as i16);
    press(&mut host, 1);
    assert!(host.home.mines.is_empty(), "a refused quarry was recorded");
    assert!(
        host.seat_notices.iter().any(|(_, line)| line.starts_with("THE QUARRY WAITS")),
        "missing THE QUARRY WAITS: {:?}",
        host.seat_notices
    );
    let claim = claim_id(&host, body.index);
    assert!(host.home.industry.claims.contains_key(&claim), "the claim was dropped");
    for name in crate::kingdom::catalog().resources.keys() {
        host.home.industry.stock.insert(name.clone(), 4000);
    }
    press(&mut host, 1);
    assert!(
        host.home.mines.contains_key(&body.index),
        "the retry did not sink the mine: {:?}",
        host.seat_notices
    );
    assert!(
        host.home.industry.buildings.values().any(|b| b.plan == "claim-quarry" && b.claim.as_deref() == Some(claim.as_str()))
    );
}

#[test]
fn a_hosted_sync_keeps_a_sunk_mine_and_a_second_press_does_nothing() {
    let mut host = Run::new(11, 1, Some("Ash"));
    host.enter_one(1, false);
    ready_kingdom(&mut host);
    let body = mines::bodies(host.caves.seed).bodies[0].clone();
    mark_seen(&mut host, &body.cells, 8);
    stand(&mut host, 1, body.head.0 as i16, body.head.1 as i16);
    press(&mut host, 1);
    assert_eq!(host.home.mines.len(), 1);
    let seed = host.home.cave_seed;
    let mines = host.home.mines.clone();
    let quarries = host.home.industry.buildings.values().filter(|b| b.plan == "claim-quarry").count();
    let mut journal = crate::rules::host_commands::Journal::new("hosted".into(), host.raid_id);
    journal
        .sync_home(&mut host, Home::default(), crate::realm::Spoils::default())
        .expect("hosted sync");
    assert_eq!(host.home.cave_seed, seed, "the hosted cycle wiped the cave seed");
    assert_eq!(host.home.mines, mines, "the hosted cycle wiped the mine");
    press(&mut host, 1);
    assert_eq!(host.home.mines.len(), 1, "a second press sank another mine");
    assert_eq!(
        host.home.industry.buildings.values().filter(|b| b.plan == "claim-quarry").count(),
        quarries
    );
}

#[test]
fn proving_does_not_touch_flags_or_the_change_log_and_a_guest_decodes_the_set() {
    let mut host = Run::new(11, 1, Some("Ash"));
    host.enter_one(1, false);
    let body = mines::bodies(host.caves.seed).bodies[0].clone();
    let (x, y) = body.cells[0];
    stand(&mut host, 1, x as i16, y as i16);
    let flags = host.caves.cell(x, y).expect("vein cell").flags;
    host.tick_caves(&BTreeMap::new());
    assert_eq!(host.caves.cell(x, y).expect("vein cell").flags, flags);
    assert!(mines::proven(&host.caves, &body) > 0);
    let back: crate::rules::caves::Field = serde_json::from_value(serde_json::to_value(&host.caves).unwrap()).unwrap();
    assert_eq!(back.proven, host.caves.proven);
}

#[test]
fn blades_stay_on_after_leaving_and_the_floor_stays_empty() {
    let mut host = Run::new(11, 1, Some("Ash"));
    host.enter_one(1, false);
    let blades = crate::rules::caves::blades_cell(host.caves.seed);
    stand(&mut host, 1, blades.0 as i16, blades.1 as i16 - 1);
    host.tick_caves(&BTreeMap::from([(1, Input { move_y: 1, ..Input::default() })]));
    assert_eq!(host.caves.seats[&1].footwear, 2);
    assert_eq!(host.caves.blades_by, 1);
    let exit = crate::rules::caves::exit_cell(host.caves.seed);
    stand(&mut host, 1, exit.0 as i16, exit.1 as i16);
    host.tick_caves(&BTreeMap::new());
    assert!(host.caves.seats.get(&1).is_none(), "the knight did not leave");
    host.enter_one(1, false);
    assert_eq!(host.caves.seats[&1].footwear, 2, "the blades did not come back");
    assert_eq!(host.caves.blades_by, 1, "the floor offered the blades again");
    let floor = host.caves.cell(blades.0, blades.1).expect("blades cell");
    assert!(floor.open(), "the blades cell is no longer empty floor");
}

#[test]
fn a_body_that_cannot_grow_is_skipped_instead_of_panicking() {
    for seed in 1u32..64 {
        let laid = mines::bodies(seed);
        assert!(laid.bodies.len() <= 8, "seed {seed}");
        let mut seen = BTreeSet::new();
        for body in &laid.bodies {
            assert!(body.index < 8);
            assert!(seen.insert(body.index), "seed {seed} repeated {}", body.index);
        }
    }
}

#[test]
fn the_head_stands_on_open_floor_south_of_the_body() {
    let mut fell = 0;
    let mut n = 0;
    for seed in 1u32..=50 {
        let cave = base(seed);
        for body in &mines::bodies(seed).bodies {
            n += 1;
            let north = (body.head.0, body.head.1 - 1);
            let open = cave.cave.at(body.head.0, body.head.1).is_some_and(|c| c.open());
            assert!(open, "seed {seed} body {} head is not open floor", body.index);
            let beside = body
                .cells
                .iter()
                .any(|c| (c.0 - body.head.0).abs() + (c.1 - body.head.1).abs() == 1);
            assert!(beside, "seed {seed} body {} head is not beside the body", body.index);
            if body.cells.contains(&north) {
                let south_exists = body.cells.iter().any(|&(x, y)| {
                    let s = (x, y + 1);
                    !body.cells.contains(&s) && cave.cave.at(s.0, s.1).is_some_and(|c| c.open())
                });
                assert!(south_exists);
            } else {
                fell += 1;
            }
        }
    }
    eprintln!("EX2 south-head fallback {fell} of {n} bodies across seeds 1..=50");
    assert_eq!(fell, 0, "a body had no open floor south of its rock");
}

include!("ex2_proof.rs");
include!("ex2b_proof.rs");
#[cfg(feature = "testing")]
include!("mt2_proof.rs");
