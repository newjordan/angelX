//! The kingdom's chunks agree with the game: walkable ground open and in
//! view of the chunk's camera, every structure under its building, every
//! set parsed whole; and the sets in `assets/world3d` are the generator's.

use super::*;
use crate::world::kingdom::geography;

#[test]
#[ignore = "prints the land the chunks are made from"]
fn dump_the_land() {
    let geo = geography(&[]);
    let land = Land::of(&geo);
    println!("map {}x{} grid {:?}", land.cols, land.rows, land.grid());
    for b in &land.builds {
        let (c, r, w, h) = b.foot;
        let open = (r..r + h).flat_map(|y| (c..c + w).map(move |x| (x, y))).filter(|&(x, y)| land.walkable(x, y)).count();
        let terr: String = (r..r + h).flat_map(|y| (c..c + w).map(move |x| (x, y))).map(|(x, y)| land.terrain(x, y) as char).collect();
        println!("{:?} {:?} {:?} {} | {:?} open {open} terrain {terr} chunk {:?}", b.kit, b.foot, b.face, b.name, b.place, land.owner(b));
        if std::env::var("BUILD").is_ok_and(|n| b.name.contains(&n)) {
            let (cx, cz) = land.owner(b);
            for g in chunk(&land, cx, cz).iter().filter(|g| g.rect == Some(b.foot)) {
                println!("  # {}", g.name);
                for s in &g.solids {
                    println!("  {}", blockout::line(s));
                }
            }
        }
    }
    let mut hedge = 0;
    for y in 0..land.rows {
        for x in 0..land.cols {
            if land.cover(x, y) == Cover::Hedge && x > 0 && y > 0 && x < land.cols - 1 && y < land.rows - 1 {
                hedge += 1;
                println!("inner hedge at {x},{y} terrain {} tile {:?}", land.terrain(x, y) as char, land.tile(x, y));
            }
        }
    }
    println!("inner hedge tiles {hedge}");
}

#[test]
#[ignore = "prints the terrain round a tile"]
fn dump_around() {
    let geo = geography(&[]);
    let land = Land::of(&geo);
    let (x, y): (i32, i32) = std::env::var("AT").ok().and_then(|s| { let v: Vec<i32> = s.split(',').map(|p| p.parse().unwrap()).collect(); Some((v[0], v[1])) }).unwrap_or((5, 25));
    for yy in y - 6..y + 7 {
        let row: String = (x - 12..x + 13).map(|xx| {
            let t = land.terrain(xx, yy) as char;
            if land.built(xx, yy).is_some() { if land.walkable(xx, yy) { 'o' } else { '#' } } else if land.walkable(xx, yy) && t != '=' && t != ':' { if land.geo.roads[(yy * land.cols + xx).max(0) as usize] { '+' } else { t.to_ascii_lowercase() } } else { t }
        }).collect();
        println!("{yy:4} {row}");
    }
}

fn assets() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/world3d")
}

fn set_path(root: &std::path::Path, id: &str) -> std::path::PathBuf {
    root.join(id).join(format!("{id}.set"))
}

/// Every chunk of the kingdom, with its blockout.
fn each_chunk(mut check: impl FnMut(&Land, (i32, i32), &[Group])) {
    let geo = geography(&[]);
    let land = Land::of(&geo);
    let (gw, gh) = land.grid();
    for cz in 0..gh {
        for cx in 0..gw {
            let groups = chunk(&land, cx, cz);
            check(&land, (cx, cz), &groups);
        }
    }
}

fn chunk_tiles(land: &Land, cx: i32, cz: i32) -> impl Iterator<Item = (i32, i32)> {
    let (c, r, w, h) = land.chunk_rect(cx, cz);
    (r..r + h).flat_map(move |y| (c..c + w).map(move |x| (x, y)))
}

/// Points of a tile's floor, `n` by `n`, `inset` pixels in from its edges.
fn samples((col, row): (i32, i32), n: usize, inset: f32) -> Vec<[f32; 2]> {
    let span = TILE - 2.0 * inset;
    (0..n)
        .flat_map(|j| (0..n).map(move |i| (i, j)))
        .map(|(i, j)| {
            [
                col as f32 * TILE + inset + span * (i as f32 + 0.5) / n as f32,
                row as f32 * TILE + inset + span * (j as f32 + 0.5) / n as f32,
            ]
        })
        .collect()
}

fn under(s: &Solid, [x, z]: [f32; 2]) -> bool {
    let [x0, z0, x1, z1] = s.footprint();
    x > x0 && x < x1 && z > z0 && z < z1
}

#[test]
fn the_kingdom_cuts_into_whole_chunks_of_its_tiles() {
    let geo = geography(&[]);
    let land = Land::of(&geo);
    let (gw, gh) = land.grid();
    assert!(gw * CHUNK.0 >= land.cols && (gw - 1) * CHUNK.0 < land.cols);
    assert!(gh * CHUNK.1 >= land.rows && (gh - 1) * CHUNK.1 < land.rows);
    assert_eq!(STAGE, (CHUNK.0 as u32 * TILE as u32, CHUNK.1 as u32 * TILE as u32), "a chunk's stage is the chunk");
    assert_eq!(CHUNK, (crate::world::map::CHUNK_W, crate::world::map::CHUNK_H), "the map's own cut");
    // Neighbours' stages share their edges.
    let set = |cx: i32, cz: i32| angel_stage::set::Set::parse(&write(&land, cx, cz, &[])).unwrap();
    assert!(set(1, 1).border_with(&set(2, 1)).is_some() && set(1, 1).border_with(&set(1, 2)).is_some());
}

#[test]
fn walkable_tiles_are_never_under_a_block() {
    each_chunk(|land, (cx, cz), groups| {
        // A fingerpost and the set dressing stand on open ground: each block
        // of them is boot-high, or a slender post a knight passes.
        for s in groups.iter().filter(|g| g.name == "fingerposts" || g.name == "set dressing").flat_map(|g| &g.solids) {
            assert!(s.top() <= LOW || slender(s), "{}: `{}` is boot-high or slender", chunk_id(cx, cz), blockout::line(s));
        }
        let blocks: Vec<&Solid> = groups
            .iter()
            .filter(|g| g.role != Role::Ground)
            .flat_map(|g| &g.solids)
            .filter(|s| s.top() > LOW && s.bottom() < HEADROOM && !slender(s))
            .collect();
        for t in chunk_tiles(land, cx, cz).filter(|&(x, y)| land.walkable(x, y)) {
            for p in samples(t, 6, 2.0) {
                if let Some(s) = blocks.iter().find(|s| under(s, p)) {
                    panic!("{}: walkable tile {t:?} is under `{}` at {p:?}", chunk_id(cx, cz), blockout::line(s));
                }
            }
        }
    });
}

#[test]
fn walkable_tiles_stay_in_view_of_the_chunks_camera() {
    each_chunk(|land, (cx, cz), groups| {
        let view = View::chunk(STAGE, centre(cx, cz));
        let hulls = standing_hulls(&view, groups);
        for t in chunk_tiles(land, cx, cz).filter(|&(x, y)| land.walkable(x, y)) {
            let hidden = view.hidden(&hulls, t);
            assert!(hidden <= blockout::HIDE_ALL + 1e-3, "{}: walkable tile {t:?} is {:.0}% hidden", chunk_id(cx, cz), hidden * 100.0);
        }
    });
}

#[test]
fn every_structure_place_and_castle_stands_under_its_building() {
    let geo = geography(&[]);
    let land = Land::of(&geo);
    let mut seen = vec![false; land.builds.len()];
    each_chunk(|land, (cx, cz), groups| {
        let view = View::chunk(STAGE, centre(cx, cz));
        for (i, b) in land.builds.iter().enumerate() {
            if land.owner(b) != (cx, cz) {
                continue;
            }
            let g = groups.iter().find(|g| g.rect == Some(b.foot) && g.role == Role::Prop).unwrap_or_else(|| panic!("{}: {} has no building", chunk_id(cx, cz), b.name));
            assert!(g.solids.iter().any(|s| s.top() > LOW), "{}: {} stands up", chunk_id(cx, cz), b.name);
            let hulls = occluders(&view, &g.solids);
            let (c, r, w, h) = b.foot;
            for t in (r..r + h).flat_map(|y| (c..c + w).map(move |x| (x, y))).filter(|&(x, y)| !land.walkable(x, y)) {
                let covered = covers(&view, &g.solids, &hulls, t);
                assert!(covered, "{}: {}'s tile {t:?} is neither under it nor behind it", chunk_id(cx, cz), b.name);
            }
            seen[i] = true;
        }
    });
    for (b, seen) in land.builds.iter().zip(seen) {
        assert!(seen, "{} at {:?} is in no chunk", b.name, b.foot);
    }
    // Every site with a lot, every castle, every structure has a build.
    for site in geo.sites.iter().filter(|s| s.lot.is_some()) {
        assert!(land.builds.iter().any(|b| b.name == site.name), "{} has a build", site.name);
    }
    assert!(land.builds.len() >= crate::world::map::STRUCTURES.len() + crate::world::houses::ids().count() + 2);
}

#[test]
fn blocked_tiles_read_as_blocked() {
    each_chunk(|land, (cx, cz), groups| {
        let view = View::chunk(STAGE, centre(cx, cz));
        let hulls = standing_hulls(&view, groups);
        let standing: Vec<&Solid> = groups.iter().filter(|g| g.role != Role::Ground).flat_map(|g| &g.solids).collect();
        // A footprint's tiles are its build's, in the chunk that holds it.
        for t in chunk_tiles(land, cx, cz).filter(|&(x, y)| !land.walkable(x, y) && land.built(x, y).is_none()) {
            let cover = land.cover(t.0, t.1);
            if matches!(cover, Cover::Water | Cover::Lava) {
                continue;
            }
            let covered = samples(t, 3, 4.0).iter().any(|&p| standing.iter().any(|s| under(s, p))) || view.hidden(&hulls, t) > 0.3;
            assert!(covered, "{}: blocked tile {t:?} ({cover:?}) is bare", chunk_id(cx, cz));
        }
    });
}

#[test]
fn every_chunk_set_parses_with_no_refusals() {
    let geo = geography(&[]);
    let land = Land::of(&geo);
    let (gw, gh) = land.grid();
    for cz in 0..gh {
        for cx in 0..gw {
            let groups = chunk(&land, cx, cz);
            let text = write(&land, cx, cz, &groups);
            let set = match angel_stage::set::Set::parse(&text) {
                Ok(set) => set,
                Err(refusals) => panic!("{}: {}", chunk_id(cx, cz), refusals.iter().map(|r| r.to_string()).collect::<Vec<_>>().join("; ")),
            };
            assert_eq!(set.id, chunk_id(cx, cz));
            assert_eq!(set.camera, angel_stage::set::SetCamera::Chunk { size: STAGE, centre: centre(cx, cz) });
            assert_eq!(set.plate, PLATE);
            let solids: Vec<&Solid> = groups.iter().flat_map(|g| &g.solids).collect();
            assert_eq!(set.shapes.len(), solids.len(), "{}: a shape to each solid", chunk_id(cx, cz));
            for (shape, solid) in set.shapes.iter().zip(&solids) {
                assert_eq!((shape.shape.as_str(), shape.kind), (solid.shape.written().as_str(), solid.kind), "{}: line {}", chunk_id(cx, cz), shape.line);
            }
        }
    }
}

#[test]
fn the_committed_chunk_sets_are_the_generators() {
    let geo = geography(&[]);
    let land = Land::of(&geo);
    let (gw, gh) = land.grid();
    for cz in 0..gh {
        for cx in 0..gw {
            let path = set_path(&assets(), &chunk_id(cx, cz));
            let committed = std::fs::read_to_string(&path).unwrap_or_default();
            assert!(
                committed == write(&land, cx, cz, &chunk(&land, cx, cz)),
                "{} is stale or missing: regenerate with ANGEL_WORLD3D_CHUNKS=1 cargo test -p angel-delve write_world3d_chunks -- --ignored",
                path.display()
            );
        }
    }
}

/// Writes every chunk's set to `assets/world3d/chunk-<cx>-<cz>/`, or under
/// the folder `ANGEL_WORLD3D_CHUNKS` names (when it is not `1`); and, when
/// `ANGEL_WORLD3D_ITEMS` names a file, the painters' items list there.
#[test]
#[ignore = "writes the kingdom's chunk sets"]
fn write_world3d_chunks() {
    let Some(to) = std::env::var_os("ANGEL_WORLD3D_CHUNKS") else {
        return;
    };
    let root = if to == "1" { assets() } else { std::path::PathBuf::from(to) };
    let geo = geography(&[]);
    let land = Land::of(&geo);
    let (gw, gh) = land.grid();
    let mut items = Vec::new();
    for cz in 0..gh {
        for cx in 0..gw {
            let id = chunk_id(cx, cz);
            let path = set_path(&root, &id);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let set = write(&land, cx, cz, &chunk(&land, cx, cz));
            std::fs::write(&path, &set).unwrap();
            println!("{:>5} lines  {}", set.lines().count(), path.display());
            let item = item(&land, cx, cz);
            let abs = std::fs::canonicalize(&path).unwrap_or(path.clone());
            items.push(serde_json::json!({
                "id": item.id,
                "set": abs.display().to_string(),
                "setting": "outdoor",
                "title": item.title,
                "notes": item.notes,
            }));
        }
    }
    if let Some(file) = std::env::var_os("ANGEL_WORLD3D_ITEMS") {
        std::fs::write(&file, serde_json::to_string_pretty(&items).unwrap() + "\n").unwrap();
        println!("{} items  {}", items.len(), std::path::Path::new(&file).display());
    }
}

/// Writes the kit's pieces, twelve to a set, under the folder
/// `ANGEL_WORLD3D_KIT_GALLERY` names: a look at every building at once.
#[test]
#[ignore = "writes the kit gallery"]
fn write_kit_gallery() {
    let Some(to) = std::env::var_os("ANGEL_WORLD3D_KIT_GALLERY") else {
        return;
    };
    let root = std::path::PathBuf::from(to);
    let kits = crate::world3d::kit::tests::all();
    for (n, page) in kits.chunks(12).enumerate() {
        let id = format!("kit-gallery-{}", n + 1);
        let mut out = format!("set {id}\ncamera room 384 240\nplate 1536 1024\n");
        for (i, kit) in page.iter().enumerate() {
            let (col, row) = ((i % 4) as f32, (i / 4) as f32);
            let (fw, fd) = match kit {
                kit::Kit::Well | kit::Kit::Quintain => (16.0, 16.0),
                kit::Kit::Stall | kit::Kit::Turret | kit::Kit::Rookery => (16.0, 32.0),
                kit::Kit::Board | kit::Kit::Garden | kit::Kit::Stable | kit::Kit::Gatehouse => (48.0, 16.0),
                kit::Kit::Castle(_) => (48.0, 32.0),
                kit::Kit::Keep | kit::Kit::DragonKeep => (48.0, 48.0),
                kit::Kit::Colosseum | kit::Kit::School | kit::Kit::College => (80.0, 64.0),
                kit::Kit::Cottage | kit::Kit::Barn | kit::Kit::Hall | kit::Kit::Forge | kit::Kit::Workshop | kit::Kit::Ward
                | kit::Kit::Chapel | kit::Kit::RoundTable | kit::Kit::Observatory | kit::Kit::MineMouth | kit::Kit::Pavilion
                | kit::Kit::Windmill | kit::Kit::Tower => (32.0, 32.0),
                _ => (64.0, 48.0),
            };
            let (x0, z0) = (col * 96.0 + (96.0 - fw) / 2.0, row * 80.0 + (80.0 - fd) / 2.0 + 4.0);
            out.push_str(&format!("\n# {kit:?}\nbox grass at {} -4 {} size 92 4 76\n", col * 96.0 + 48.0, row * 80.0 + 40.0));
            let spot = kit::Spot { foot: [x0, z0, x0 + fw, z0 + fd], face: kit::Face::South, seed: 7 + i as u32 };
            for s in kit::build(*kit, &spot).into_iter().flatten() {
                out.push_str(&blockout::line(&s));
                out.push('\n');
            }
        }
        let path = root.join(&id).join(format!("{id}.set"));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, out).unwrap();
    }
}
