//! What a run stands in the kingdom that no chunk's set has: the party's
//! builds on its lots, the places a wish moved, the run's finds and the
//! herald's posts, as extra blocks in their chunks.

use super::*;
use crate::world::wishes::lots as wish_lots;

/// The catalog wishes the proofs build, all beside the Village.
const BUILT: [&str; 4] = ["tavern", "wayside-shrine", "fellowship-stone", "beacon-tower"];

/// A run out in the kingdom with the catalog's `ids` built beside `near`.
fn run_with(ids: &[&str], near: &str) -> Run {
    let mut run = kingdom::proof_run(&[[1344.0, 1408.0]]);
    run.kingdom.wishes = Arc::new(proof_wishes(ids, near));
    run
}

#[test]
fn the_catalog_reads_and_each_wish_finds_its_nearest_piece() {
    let wishes = proof_wishes(&["tavern", "wayside-shrine", "fellowship-stone", "beacon-tower", "miners-lodge", "hall-of-the-dragonslayers", "grail-chapel"], "village");
    assert_eq!(wishes.len(), 7, "every catalog wish reads");
    let kits: Vec<Kit> = wishes.iter().map(|w| kit_for(w, (2, 2))).collect();
    assert_eq!(kits, [Kit::Cottage, Kit::Shrine, Kit::Flags, Kit::Tower, Kit::Hall, Kit::Hall, Kit::Chapel]);
    // Nothing the kit names: by its size.
    let mut plain = wishes[0].clone();
    (plain.name, plain.words) = ("Something".into(), "a thing for the realm".into());
    assert_eq!(kit_for(&plain, (1, 1)), Kit::Well);
    assert_eq!(kit_for(&plain, (3, 2)), Kit::Hall);
    assert_eq!(kit_for(&plain, (2, 2)), Kit::Cottage);
}

#[test]
fn a_built_wish_stands_on_its_lots_building_rows_and_clears_the_lot() {
    let run = run_with(&BUILT, "village");
    let lots = of(&run).expect("the party's builds stand");
    let placed = wish_lots(&run.kingdom.wishes);
    assert_eq!(placed.iter().flatten().count(), BUILT.len(), "each wish has its lot");
    for lot in placed.iter().flatten() {
        let (cx, cz) = ((lot.tx + lot.tw / 2).div_euclid(chunks::CHUNK.0), (lot.ty + (lot.th - 1) / 2).div_euclid(chunks::CHUNK.1));
        let share = lots.share(kingdom::index(cx, cz).unwrap()).expect("its chunk's share");
        let [x0, z0, x1, z1] = blockout::px((lot.tx, lot.ty, lot.tw, lot.th));
        let door = blockout::px((lot.tx, lot.ty + lot.th - 1, lot.tw, 1));
        let on_lot: Vec<&Shape> = share
            .extras
            .iter()
            .filter(|s| {
                let [a, b, c, d] = fit::footprint(s);
                a >= x0 - 1.0 && b >= z0 - 1.0 && c <= x1 + 1.0 && d <= z1 + 1.0
            })
            .collect();
        assert!(on_lot.iter().any(|s| s.at[1] + s.size[1] >= 12.0), "{lot:?}: something stands up on it");
        // Its door row stays open: nothing over a boot's height there.
        assert!(
            on_lot.iter().filter(|s| fit::overlaps(fit::footprint(s), door)).all(|s| s.at[1] + s.size[1] <= blockout::LOW || s.at[1] >= blockout::HEADROOM),
            "{lot:?}: its door row open"
        );
        assert!(share.clear.contains(&[x0, z0, x1, z1]), "{lot:?}: the lot cleared of its chunk's own");
        assert!(lots.plaques.iter().any(|p| p.at.1 >= (lot.ty + lot.th - 1) * 16 && p.at.1 < (lot.ty + lot.th) * 16), "its name on its door row");
    }
    // The same run, the same lots.
    assert_eq!(of(&run).unwrap().key, lots.key);
    // A build raised, a build cleared: what stands changes with them.
    let mut fewer = run.clone();
    fewer.kingdom.wishes = Arc::new(run.kingdom.wishes[..2].to_vec());
    let less = of(&fewer).expect("two still stand");
    assert_ne!(less.key, lots.key);
    let count = |l: &Lots| l.shares.iter().map(|s| s.extras.len()).sum::<usize>();
    assert!(count(&less) < count(&lots));
    let mut none = run.clone();
    none.kingdom.wishes = Arc::new(Vec::new());
    assert!(of(&none).is_none_or(|l| l.plaques.is_empty() && l.shares.iter().all(|s| s.clear.is_empty())), "no wishes, no lots");
}

#[test]
fn what_a_chunk_stands_on_a_lot_is_cut() {
    // The first grove of the Greenwood's corner chunk, and a box (a hedge
    // or a step of rock) somewhere: cleared, as a lot clears its ground.
    let registry = kingdom::chunks();
    let (chunk, tree, block) = (0..kingdom::COUNT as u32)
        .filter_map(|i| registry.chunk(i))
        .find_map(|c| {
            let tree = c.set.shapes.iter().find(|s| !s.on_floor() && s.kind == Kind::Foliage && s.shape != "box")?.clone();
            let block = c
                .set
                .shapes
                .iter()
                .find(|s| {
                    !s.on_floor() && fit::is_box(s) && matches!(s.kind, Kind::Hedge | Kind::Rock) && s.size[0] >= 24.0 && fit::clip(fit::footprint(s), fit::footprint(&tree)).is_none()
                })?
                .clone();
            Some((c, tree, block))
        })
        .expect("a chunk with a tree and a long box");
    let [a, b, c, d] = fit::footprint(&block);
    let half = [a, b, (a + c) / 2.0, d];
    let piece = |clear: Vec<[f32; 4]>| Piece { chunk: kingdom::cell(chunk.index), clear, ..Piece::default() };
    let lots = assemble(vec![piece(vec![fit::footprint(&tree), half])], Vec::new());
    let cut = lots.chunk(&chunk);
    assert!(!Arc::ptr_eq(&cut, &chunk), "cut");
    assert_ne!(cut.key, chunk.key, "its own set, its own key");
    assert_eq!(cut.set.id, chunk.set.id);
    let tree_at = |set: &Set| set.shapes.iter().any(|s| s.shape == tree.shape && s.at == tree.at);
    assert!(tree_at(&chunk.set) && !tree_at(&cut.set), "the tree on the lot is gone");
    // The box stands on as the half of it off the lot.
    let kept: Vec<&Shape> = cut.set.shapes.iter().filter(|s| s.kind == block.kind && s.at[1] == block.at[1] && fit::overlaps(fit::footprint(s), [a, b, c, d])).collect();
    assert!(kept.iter().all(|s| fit::footprint(s)[0] >= half[2] - 0.5), "nothing of the box left on the lot");
    assert!(!kept.is_empty(), "the rest of the box stands: {block:?} cut {:?} by {:?}", cut.text.lines().filter(|l| l.contains("cut for a lot")).collect::<Vec<_>>(), lots.share(chunk.index).map(|s| s.clear.clone()));
    // Made once; a chunk nothing stands on is itself.
    assert!(Arc::ptr_eq(&lots.chunk(&chunk), &cut));
    let other = registry.chunk((chunk.index + 1) % kingdom::COUNT as u32).unwrap();
    assert!(Arc::ptr_eq(&lots.chunk(&other), &other));
}

#[test]
fn the_runs_finds_stand_on_their_tiles_and_change_as_they_are_found() {
    let mut run = kingdom::proof_run(&[[1344.0, 1408.0]]);
    let finds = run.standing_finds();
    assert!(finds.len() >= 20, "the run's finds");
    let lots = of(&run).expect("the finds stand");
    for (_, find) in &finds {
        let index = kingdom::index(find.at.0.div_euclid(chunks::CHUNK.0), find.at.1.div_euclid(chunks::CHUNK.1)).unwrap();
        let share = lots.share(index).expect("its chunk's share");
        let (x, front) = (find.at.0 as f32 * 16.0 + 8.0, (find.at.1 + 1) as f32 * 16.0);
        let on_it: Vec<&Shape> = share
            .extras
            .iter()
            .filter(|s| {
                let [a, b, c, d] = fit::footprint(s);
                a >= x - 22.0 && c <= x + 22.0 && b >= front - 18.0 && d <= front + 1.0
            })
            .collect();
        assert!(on_it.iter().any(|s| s.at[1] + s.size[1] >= 5.0), "{find:?} stands up");
        assert!(share.clear.is_empty() || share.clear.iter().all(|r| !(r[0] < x && x < r[2] && r[1] < front - 1.0 && front - 1.0 < r[3])), "{find:?}: nothing cleared under it");
    }
    // A ruin's cache goes when it is found; a shrine's candle burns.
    let (i, ruin) = *finds.iter().find(|(_, f)| f.kind == crate::world::finds::Kind::Ruin).expect("a ruin");
    let index = kingdom::index(ruin.at.0.div_euclid(chunks::CHUNK.0), ruin.at.1.div_euclid(chunks::CHUNK.1)).unwrap();
    let before = lots.share(index).unwrap().extras.len();
    run.kingdom.found |= 1 << i;
    let after = of(&run).unwrap();
    assert_eq!(after.share(index).unwrap().extras.len(), before - 1, "the cache taken");
}

#[test]
fn a_window_and_a_page_are_sent_the_lots_as_extra_blocks_that_follow_the_party() {
    use crate::render::overworld::arena::{self, stage_frame::StageMaker};
    let run = run_with(&BUILT, "village");
    let lot = wish_lots(&run.kingdom.wishes)[0].expect("the tavern's lot");
    let feet = [(lot.tx * 16 + lot.tw * 8) as f32, ((lot.ty + lot.th) * 16 + 20) as f32];
    let mut run = run;
    {
        let hero = run.players.values_mut().next().unwrap();
        (hero.x, hero.y) = (feet[0] / arena::UNIT, feet[1] / arena::UNIT);
        hero.camera = Some((hero.x, hero.y - 1.0));
    }
    let scene_of = |run: &Run| {
        arena::props_meshes(true);
        crate::world3d::set_enabled(true);
        let (w, h) = arena::seat_size();
        let scene = Arc::new(arena::seat_scene(run, w as i32, h as i32, Some(1)));
        crate::world3d::set_enabled(false);
        arena::props_meshes(false);
        scene
    };
    let scene = scene_of(&run);
    let lots = scene.lots.clone().expect("the run's lots with the scene");
    assert!(!scene.chunks.is_empty());
    assert!(scene.under.is_some(), "the party's names on the ground");
    for png_plates in [false, true] {
        let mut maker = StageMaker::default();
        maker.png_plates = png_plates;
        let frame = maker.make(&scene, |_| None);
        let world = frame.props.as_ref().and_then(|p| p.world.clone()).expect("the world");
        let extra: Vec<&Node> = world.extra.iter().filter(|n| n.id & 0xffff_ffff >= EXTRA_IDS).collect();
        // To a window as lit extra blocks; to a page as a set of their own
        // a chunk, which it fills lit (a window's or a fire's accent kept).
        let by_set = world.set.iter().chain(&world.more).any(|r| lots.shares.iter().any(|s| s.key == r.key && r.text.as_deref() == Some(&*s.text)));
        assert!(if png_plates { extra.is_empty() && by_set } else { !extra.is_empty() && !by_set }, "the party's builds go with the blocks");
        let mut ids: Vec<u64> = world.floor.iter().chain(&world.blocks).chain(&world.extra).map(|n| n.id).collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), n, "a node an id");
        // Every patch they wear went as a picture, or none does.
        let sent: Vec<u32> = frame.pictures.iter().map(|p| p.id).collect();
        for node in &extra {
            if let angel_stage::scene::TextureSource::Picture(id) = node.material.source {
                assert!(sent.contains(&id), "patch {id:#x} sent with them");
            }
        }
        if png_plates {
            // Across the wire they go by their sets, read back as they went;
            // a chunk cut for a lot goes as its own set.
            let mut out = Vec::new();
            angel_stage::wire::encode(&frame, &[], &mut out);
            let back = angel_stage::wire::decode(&out).expect("it reads").frame.props.unwrap().world.unwrap();
            assert_eq!((back.set.clone(), back.more.clone()), (world.set.clone(), world.more.clone()));
            let keys: Vec<u64> = world.set.iter().chain(&world.more).map(|r| r.key).collect();
            let shares: Vec<u64> = lots.shares.iter().map(|s| s.key).collect();
            assert!(keys.iter().all(|k| scene.chunks.iter().any(|c| c.key == *k) || shares.contains(k)), "a chunk cut for a lot by its own set");
        }
    }
    // The party clears its last build: the next frame stands one fewer (a
    // build cleared before others would let a later one take its lot).
    let last = wish_lots(&run.kingdom.wishes)[BUILT.len() - 1].expect("the last one's lot");
    let mut cleared = run.clone();
    cleared.kingdom.wishes = Arc::new(run.kingdom.wishes[..BUILT.len() - 1].to_vec());
    let after = scene_of(&cleared);
    let after_lots = after.lots.clone().expect("the others stand");
    assert_ne!(after_lots.key, lots.key);
    let foot = blockout::px((last.tx, last.ty, last.tw, last.th - 1));
    let standing = |l: &Lots| l.shares.iter().flat_map(|s| &s.extras).filter(|s| fit::overlaps(fit::footprint(s), foot) && s.at[1] + s.size[1] > 8.0).count();
    assert!(standing(&lots) > 0 && standing(&after_lots) == 0, "its blocks gone with it");
}

#[test]
fn off_the_kingdom_in_3d_nothing_changes() {
    use crate::render::overworld::arena;
    let run = run_with(&BUILT, "village");
    let flat = arena::frame_for(&run, arena::NATIVE_W, arena::NATIVE_H, Some(1));
    let scene = arena::scene_for(&run, arena::NATIVE_W, arena::NATIVE_H, Some(1));
    assert!(scene.lots.is_none() && scene.chunks.is_empty(), "off, no lots");
    crate::world3d::set_enabled(true);
    let _ = arena::frame_for(&run, arena::NATIVE_W, arena::NATIVE_H, Some(1));
    crate::world3d::set_enabled(false);
    assert_eq!(arena::frame_for(&run, arena::NATIVE_W, arena::NATIVE_H, Some(1)), flat, "off again, as it always was");
}

/// `cargo test -p angel-delve lots_dump -- --ignored --nocapture`: where the
/// proof's builds and the run's finds stand, chunk by chunk.
#[test]
#[ignore = "prints where the lots stand"]
fn lots_dump() {
    let run = run_with(&BUILT, "village");
    for (wish, lot) in run.kingdom.wishes.iter().zip(wish_lots(&run.kingdom.wishes).iter()) {
        println!("{}: {lot:?} kit {:?}", wish.name, lot.map(|l| kit_for(wish, (l.tw, l.th - 1))));
    }
    for (i, f) in run.standing_finds() {
        println!("find {i}: {:?} at {:?} chunk {:?}", f.kind, f.at, (f.at.0 / 24, f.at.1 / 16));
    }
    let lots = of(&run).unwrap();
    for s in &lots.shares {
        println!("chunk {:?}: {} extras, {} cleared, {} tiles", kingdom::cell(s.index), s.extras.len(), s.clear.len(), s.tiles.len());
    }
}
