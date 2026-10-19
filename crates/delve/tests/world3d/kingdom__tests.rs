//! The kingdom's chunks as the game draws them: which are near a view, each
//! chunk the realm's set drawn lit, a page keeping what it was sent.

use super::*;

#[test]
fn a_view_is_near_its_chunks_and_a_ring_round_them() {
    // A knight's view inside one chunk's columns, across two rows.
    let near = near([384.0, 300.0, 768.0, 524.0]);
    let cells: Vec<(i32, i32)> = near.iter().map(|&i| cell(i)).collect();
    assert!(cells.contains(&(1, 1)) && cells.contains(&(1, 2)) && cells.contains(&(0, 0)) && cells.contains(&(2, 3)));
    assert_eq!(near.len(), 3 * 4);
    // Walking the whole kingdom, a view is never near more than 4 x 4.
    let (kw, kh) = (GRID.0 as f32 * STAGE.0, GRID.1 as f32 * STAGE.1);
    let mut most = 0;
    for y in (0..(kh as i32 - 224)).step_by(7) {
        for x in (0..(kw as i32 - 384)).step_by(11) {
            let (x, y) = (x as f32, y as f32);
            most = most.max(super::near([x, y, x + 384.0, y + 224.0]).len());
        }
    }
    assert_eq!(most, RESIDENT, "a view is near at most as many chunks as are kept decoded");
    // At the kingdom's corner, inside it.
    assert!(near_centre((0.0, 0.0), (384.0, 224.0)).iter().all(|&i| cell(i).0 <= 2 && cell(i).1 <= 2));
}

#[test]
fn every_chunk_is_the_realms_set_drawn_lit() {
    let chunks = Chunks::host();
    assert!(chunks.local());
    for index in 0..COUNT as u32 {
        let chunk = chunks.chunk(index).unwrap_or_else(|| panic!("chunk {index}"));
        assert_eq!(chunk.set.id, chunk.id());
        assert_eq!(&*chunk.text, &*super::super::realm::set_text(index), "{}: the realm's set", chunk.id());
        assert!(!chunk.painted(), "plates are retired for the kingdom");
        let (lo, hi) = chunk.bounds;
        let [x0, z0, x1, z1] = chunk.set.camera.stage();
        assert!(lo[0] <= x0 + 1.0 && hi[0] >= x1 - 1.0 && lo[2] <= z0 + 1.0 && hi[2] >= z1 - 1.0, "{}: its ground covers its stage", chunk.id());
        let lit = chunk.lit();
        assert!(Arc::ptr_eq(&lit, &chunk.lit()), "made once");
        let (id, _) = lit.meshes.first().expect("its ground");
        assert_eq!(*id, angel_stage::set::ground_mesh_id(chunk.key));
        assert!(lit.meshes.iter().all(|(_, m)| m.validate() == Ok(())));
        assert_eq!(lit.floor[0].mesh, Some(angel_stage::scene::MeshRef::Uploaded(*id)), "its ground first");
        // Baked: a node a material, every one naming a mesh it carries, its
        // shapes' every triangle in them.
        let nodes = lit.floor.len() + lit.standing.len();
        assert!(nodes <= 12, "{}: {nodes} nodes", chunk.id());
        for n in lit.floor.iter().chain(&lit.standing) {
            let Some(angel_stage::scene::MeshRef::Uploaded(m)) = n.mesh else { panic!("a baked mesh") };
            assert!(lit.meshes.iter().any(|(id, _)| *id == m));
        }
        let triangles: usize = lit.meshes[1..].iter().map(|(_, m)| m.indices.len() / 3).sum();
        let want: usize = chunk.set.shapes.iter().map(|s| s.mesh.generate().map_or(0, |d| d.indices.len() / 3)).sum();
        assert_eq!(triangles, want, "{}: every shape baked", chunk.id());
        assert!(lit.floor.iter().chain(&lit.standing).all(|n| n.id >> 32 == chunk.ids() >> 32), "its own ids");
    }
}

#[test]
fn a_host_with_no_asset_folder_makes_every_chunk() {
    let dir = std::env::temp_dir().join(format!("world3d-kingdom-empty-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let chunks = Chunks::scan(&dir);
    let chunk = chunks.chunk(index(3, 3).unwrap()).expect("its set");
    assert!(!chunk.painted());
    assert!(chunks.plate(&chunk).is_none());
    assert!(Arc::ptr_eq(&chunk, &chunks.chunk(index(3, 3).unwrap()).unwrap()), "made once and kept");
    assert_eq!(chunks.unpainted().len(), COUNT);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_plate_is_decoded_off_the_frame_and_kept_with_a_few() {
    let chunks = Chunks::scan(&super::super::blockout::landmark_folder());
    let Some(chunk) = (0..COUNT as u32).filter_map(|i| chunks.chunk(i)).find(|c| c.painted()) else {
        return; // no plates in this checkout's asset pack
    };
    let mut plate = chunks.plate(&chunk);
    for _ in 0..400 {
        if plate.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        plate = chunks.plate(&chunk);
    }
    let plate = plate.expect("decoded on its own thread");
    assert_eq!((plate.width, plate.height), chunk.set.plate);
    assert_eq!(chunks.resident(), 1);
    // Its blocks with the plate cast on them, made once a picture.
    let cast = chunk.cast(42, Patches::NONE);
    assert!(Arc::ptr_eq(&cast, &chunk.cast(42, Patches::NONE)));
    assert!(cast.floor.iter().chain(&cast.standing).all(|n| n.material.projector.is_some()));
}

#[test]
fn each_kinds_dark_colour_is_the_realms_two_shades_down() {
    // The stage spells it as a table (a page's renderer has no inks): the
    // same as the realm's nearest plain ink taken two steps down.
    for kind in Kind::ALL {
        assert_eq!(kind.dark(), darker(kind.capture()), "{kind}");
    }
}

#[test]
fn a_chunks_patches_are_its_swatches_pictures() {
    // Every chunk with patches names them in one picture, each where
    // `swatches.png` lays it, and its blocks cast with them carry them where
    // the plate does not see.
    let Some(chunk) = (0..COUNT as u32).filter_map(|i| chunks().chunk(i)).find(|c| c.painted() && !c.swatches().is_empty()) else {
        println!("no painted chunk with patches in the folder: nothing to check");
        return;
    };
    let patches = chunk.patches(5031);
    let atlas = assets::swatch_atlas(chunk.swatches()).expect("its patches");
    assert_eq!(atlas, assets::decode(&chunk.swatches_png).unwrap(), "laid back as swatches.png lays them");
    for s in chunk.swatches() {
        let (x, y) = angel_stage::set::swatch_cell(s.kind);
        assert_eq!(patches.rect(s.kind), Some([x, y, s.width, s.height]));
    }
    let blocks = chunk.cast(77, patches);
    for node in blocks.floor.iter().chain(&blocks.standing) {
        let kind = chunk.set.shapes[(node.id & 0xffff_ffff) as usize - 1].kind;
        let want = chunk.swatch(kind).map(|s| s.kind);
        assert_eq!(node.material.patch.map(|p| p.picture), want.map(|_| 5031), "{kind}");
        assert_eq!(patches.drawn_as(kind), want, "{kind}");
    }
}

#[test]
fn a_page_keeps_what_it_was_sent() {
    let page = Chunks::scan(Path::new("/nonexistent/world3d"));
    assert!(!page.local());
    let at = index(2, 4).unwrap();
    assert!(page.chunk(at).is_none());
    let text = set_text(at);
    let none: [u8; 0] = [];
    assert!(page.receive(at, 1, &text, &none, &none));
    let chunk = page.chunk(at).expect("kept");
    assert!(!chunk.painted());
    assert!(!page.receive(at, 2, "set broken\n", &none, &none), "a set that does not read");
    assert!(!page.receive(index(3, 4).unwrap(), 3, &text, &none, &none), "another chunk's set");
    page.keep(&[]);
    assert!(page.chunk(at).is_none());
}

/// `cargo test -p angel-delve --release kingdom_costs -- --ignored
/// --nocapture`: what a chunk costs a host, measured: its set made, its
/// plate read and decoded, its blocks made and placed.
#[test]
#[ignore = "prints what the chunks cost"]
fn kingdom_costs() {
    use std::time::Instant;
    let t = Instant::now();
    let _ = set_text(0);
    println!("the land and the first set: {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
    let t = Instant::now();
    for i in 1..COUNT as u32 {
        let _ = set_text(i);
    }
    println!("the other {} sets: {:.2} ms each", COUNT - 1, t.elapsed().as_secs_f64() * 1e3 / (COUNT - 1) as f64);
    let chunks = Chunks::scan(&super::super::blockout::landmark_folder());
    let t = Instant::now();
    let all: Vec<_> = (0..COUNT as u32).filter_map(|i| chunks.chunk(i)).collect();
    println!("read and parse all {}: {:.2} ms each", all.len(), t.elapsed().as_secs_f64() * 1e3 / all.len() as f64);
    let painted: Vec<_> = all.iter().filter(|c| c.painted()).collect();
    println!("{} painted", painted.len());
    let t = Instant::now();
    for c in painted.iter().take(8) {
        let _ = assets::decode(&c.png).unwrap();
    }
    println!("decode a plate: {:.1} ms", t.elapsed().as_secs_f64() * 1e3 / painted.len().clamp(1, 8) as f64);
    let t = Instant::now();
    for c in &all {
        let _ = c.lit();
    }
    println!("make a chunk's lit blocks and ground: {:.3} ms", t.elapsed().as_secs_f64() * 1e3 / all.len() as f64);
    println!("set text: {} KB in all, {} KB most in one", all.iter().map(|c| c.text.len()).sum::<usize>() / 1024, all.iter().map(|c| c.text.len()).max().unwrap() / 1024);
    let shapes: usize = all.iter().map(|c| c.set.shapes.len()).sum();
    println!("shapes: {} in all, {} most in one", shapes, all.iter().map(|c| c.set.shapes.len()).max().unwrap());
    let t = Instant::now();
    for c in &all {
        let _ = c.cast(1, c.patches(2));
    }
    println!("make a chunk's cast blocks: {:.3} ms", t.elapsed().as_secs_f64() * 1e3 / all.len() as f64);
    let t = Instant::now();
    let mut n = 0;
    for _ in 0..10 {
        for c in all.iter().take(16) {
            let b = c.cast(1, c.patches(2));
            n += angel_stage::set::place(&b.floor, 1.0, [-10.0, 0.0, -10.0]).len();
            n += angel_stage::set::place(&b.standing, 1.0, [-10.0, 0.0, -10.0]).len();
        }
    }
    println!("place 16 chunks' blocks: {:.3} ms ({} nodes)", t.elapsed().as_secs_f64() * 1e3 / 10.0, n / 10);
}

#[test]
fn a_terminal_has_each_chunks_plate_laid_down_on_the_kingdom() {
    use crate::render::overworld::arena;
    let feet = [1344.0, 1408.0];
    let run = proof_run(&[feet]);
    let painted: Vec<Arc<Chunk>> = near([feet[0] - 192.0, feet[1] - 120.0, feet[0] + 192.0, feet[1] + 104.0])
        .into_iter()
        .filter_map(|i| chunks().chunk(i))
        .filter(|c| c.painted())
        .collect();
    let flat = arena::frame_for(&run, arena::NATIVE_W, arena::NATIVE_H, Some(1));
    crate::world3d::set_enabled(true);
    for chunk in &painted {
        let _ = chunks().plate_now(chunk);
    }
    let laid = arena::frame_for(&run, arena::NATIVE_W, arena::NATIVE_H, Some(1));
    crate::world3d::set_enabled(false);
    assert_eq!(arena::frame_for(&run, arena::NATIVE_W, arena::NATIVE_H, Some(1)), flat, "off, as it always was");
    if painted.is_empty() {
        assert_eq!(laid, flat, "no plates near: as it was");
        return;
    }
    assert_ne!(laid, flat, "the plates laid down");
    // A pixel of a painted chunk's open ground is its plate laid down there.
    let chunk = &painted[0];
    let img = super::laid(chunk).expect("laid down");
    assert_eq!((img.w, img.h), (STAGE.0 as i32, STAGE.1 as i32));
    assert!((0..img.h).all(|y| (0..img.w).all(|x| img.get(x, y).is_some())), "every pixel of its ground");
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test -p angel-delve --release
/// kingdom_terminal -- --ignored --nocapture`: the kingdom as a terminal
/// draws it at a few places with the world in 3D (plates laid down) and
/// flat, and what a frame costs either way.
#[test]
#[ignore = "writes the kingdom's terminal shots and times them"]
fn kingdom_terminal() {
    use crate::render::overworld::arena;
    use std::time::Instant;
    let dir = std::env::var_os("ANGEL_ARENA_SHOTS").map(std::path::PathBuf::from);
    let places = [("village", [1344.0, 1408.0]), ("castle-town", [1344.0, 896.0]), ("greenwood", [192.0, 128.0]), ("march", [2112.0, 2432.0])];
    for (name, feet) in places {
        let run = proof_run(&[feet]);
        for on in [false, true] {
            crate::world3d::set_enabled(on);
            if on {
                for i in near([feet[0] - 192.0, feet[1] - 120.0, feet[0] + 192.0, feet[1] + 104.0]) {
                    if let Some(c) = chunks().chunk(i) {
                        let t = Instant::now();
                        let _ = chunks().plate_now(&c).map(|_| super::laid(&c));
                        if c.painted() {
                            println!("{name}: {} decoded and laid down in {:.1} ms", c.id(), t.elapsed().as_secs_f64() * 1e3);
                        }
                    }
                }
            }
            let t = Instant::now();
            let n = 60;
            let mut img = None;
            for k in 0..n {
                let mut run = run.clone();
                run.tick += k;
                img = Some(arena::frame_for(&run, arena::NATIVE_W, arena::NATIVE_H, Some(1)));
            }
            println!("{name} {}: {:.2} ms a frame", if on { "3D" } else { "flat" }, t.elapsed().as_secs_f64() * 1e3 / n as f64);
            if let (Some(dir), Some(img)) = (&dir, img) {
                std::fs::create_dir_all(dir).unwrap();
                let k = 3u32;
                let file = dir.join(format!("terminal-{name}-{}.png", if on { "3d" } else { "flat" }));
                image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k)).unwrap().save(file).unwrap();
            }
        }
        crate::world3d::set_enabled(false);
    }
}

/// `ANGEL_WORLD3D_WALK=<file> cargo test -p angel-delve write_kingdom_walk
/// -- --ignored --nocapture`: a run out in the kingdom, one knight at the
/// west end of the longest east-west stretch of open ground that crosses three
/// chunk borders, as a run's JSON for `delve-dev-host --scene first
/// --snapshot <file>`; it prints the stretch (a browser walks it east).
#[test]
#[ignore = "writes a run for a browser to walk the kingdom"]
fn write_kingdom_walk() {
    let Some(file) = std::env::var_os("ANGEL_WORLD3D_WALK") else {
        return;
    };
    let geo = crate::world::kingdom::geography(&[]);
    let cols = geo.room.cols as i32;
    let road = |c: i32, r: i32| super::super::blockout::walkable(geo.room.tile(c, r));
    let mut best = (0, 0, 0);
    for r in 0..geo.room.rows as i32 {
        let mut start = 0;
        for c in 0..=cols {
            if c < cols && road(c, r) {
                continue;
            }
            let crossed = (start + 1..c).filter(|x| x % CHUNK_W == 0).count();
            if crossed >= 3 && c - start > best.2 - best.1 {
                best = (r, start, c);
            }
            start = c + 1;
        }
    }
    let (r, c0, c1) = best;
    assert!(c1 > c0, "open ground crossing three chunk borders");
    println!("row {r}: tiles {c0}..{c1} ({} px), chunks {}..{}", (c1 - c0) * 16, c0 / CHUNK_W, (c1 - 1) / CHUNK_W);
    let run = proof_run(&[[c0 as f32 * 16.0 + 24.0, r as f32 * 16.0 + 10.0]]);
    std::fs::write(&file, serde_json::to_vec(&run).unwrap()).unwrap();
}
