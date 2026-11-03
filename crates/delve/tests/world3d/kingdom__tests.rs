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
    assert_eq!(most, RESIDENT, "a view is near at most RESIDENT chunks");
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
    assert!(Arc::ptr_eq(&chunk, &chunks.chunk(index(3, 3).unwrap()).unwrap()), "made once and kept");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn each_kinds_dark_colour_is_the_realms_two_shades_down() {
    // The stage spells it as a table (a page's renderer has no inks): the
    // same as the realm's nearest plain ink taken two steps down.
    use crate::render::overworld::{ink, light};
    for kind in angel_stage::set::Kind::ALL {
        let rgb = kind.capture();
        let c = ink::nearest_plain([f32::from(rgb[0]), f32::from(rgb[1]), f32::from(rgb[2])]);
        assert_eq!(kind.dark(), light::step_down(c, 2), "{kind}");
    }
}

#[test]
fn a_page_keeps_what_it_was_sent() {
    let page = Chunks::scan(Path::new("/nonexistent/world3d"));
    assert!(!page.local());
    let at = index(2, 4).unwrap();
    assert!(page.chunk(at).is_none());
    let text = set_text(at);
    assert!(page.receive(at, 1, &text));
    assert!(page.chunk(at).is_some(), "kept");
    assert!(!page.receive(at, 2, "set broken\n"), "a set that does not read");
    assert!(!page.receive(index(3, 4).unwrap(), 3, &text), "another chunk's set");
    page.keep(&[]);
    assert!(page.chunk(at).is_none());
}

/// `cargo test -p angel-delve --release kingdom_costs -- --ignored
/// --nocapture`: what a chunk costs a host, measured: its set made, its
/// lit blocks made.
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
    let t = Instant::now();
    for c in &all {
        let _ = c.lit();
    }
    println!("make a chunk's lit blocks and ground: {:.3} ms", t.elapsed().as_secs_f64() * 1e3 / all.len() as f64);
    println!("set text: {} KB in all, {} KB most in one", all.iter().map(|c| c.text.len()).sum::<usize>() / 1024, all.iter().map(|c| c.text.len()).max().unwrap() / 1024);
    let shapes: usize = all.iter().map(|c| c.set.shapes.len()).sum();
    println!("shapes: {} in all, {} most in one", shapes, all.iter().map(|c| c.set.shapes.len()).max().unwrap());
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
