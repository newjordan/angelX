//! Delivered entrance backdrops, masked by the existing room collision grid.
use super::*;
use std::sync::{Arc, Mutex, OnceLock};

fn index(kind: RoomKind) -> Option<usize> {
    Some(match kind {
        RoomKind::Smithy => 0,
        RoomKind::Workshop => 1,
        RoomKind::Muster => 2,
        _ => return None,
    })
}
const SLOTS: [&str; 3] = ["latch.entrance", "rafterwake.entrance", "pennon.entrance"];
fn source(index: usize) -> &'static Img {
    static BANKS: [OnceLock<Img>; 3] = [const { OnceLock::new() }; 3];
    BANKS[index].get_or_init(|| slot_art::load(SLOTS[index], "room", &[]))
}
pub(super) fn supported(run: &Run) -> bool {
    slot_art::enabled() && run.at_home_now() && run.room().cols == 24 && run.room().rows == 14
        && index(run.room().kind).is_some()
        && !run.cinder_showing() && !run.latch_showing() && !run.mossbell_showing()
        && !run.brasswake_showing() && !run.rafterwake_showing() && !run.pennon_showing()
}
pub(super) fn warm() {
    for i in 0..SLOTS.len() {let _ = source(i);}
}
fn original(run: &Run) -> Img {
    match run.room().kind {
        RoomKind::Smithy => world::scenery(run),
        _ => home::settlement_scenery(run),
    }
}
#[derive(PartialEq, Eq)]
struct Key { layout: u64, room: usize, kind: RoomKind, home: u64 }
// Settlement plaques belong to the existing room painter. Preserve their
// whole rectangle, including the border, rather than only the doorway tile.
fn plaque_bounds(run: &Run) -> Option<(i32, i32, i32, i32)> {
    if !matches!(run.room().kind, RoomKind::Workshop | RoomKind::Muster) { return None; }
    let sign = district_sign(run.room().kind.settlement_label());
    Some(((384 - sign.w) / 2, 0, sign.w, sign.h))
}
fn bake(run: &Run) -> Img {
    let src = source(index(run.room().kind).unwrap());
    let old = original(run);
    let mut im = src.clone();
    let plaque = plaque_bounds(run);
    for y in 0..im.h {for x in 0..im.w {
        let (nx, ny) = (x, y);
        let tile = run.room().tile(nx/16, ny/16);
        let title = plaque.is_some_and(|(x, y, w, h)| nx >= x && nx < x+w && ny >= y && ny < y+h);
        // Keep the master at its original coordinates, including its interior
        // stone joints and lighting. Rule-owned furniture and exits stay above it.
        let source_opening = ((176..208).contains(&nx) && (ny < 48 || ny >= 192))
            || ((80..144).contains(&ny) && (nx < 32 || nx >= 352));
        let c = if title || matches!(tile, Tile::Block | Tile::Hazard | Tile::Ledge)
            || (tile == Tile::Wall && source_opening) {
            old.get(nx, ny)
        } else {src.get(x,y)};
        if let Some(c) = c {im.set(x,y,c);}
    }}
    im
}
pub(super) fn scene(run: &Run) -> Arc<Img> {
    type Cache = Mutex<Option<(Key, [Arc<Img>; 4])>>;
    static SCENES: [OnceLock<Cache>; 3] = [const { OnceLock::new() }; 3];
    let i = index(run.room().kind).expect("entrance room");
    let key = Key {layout: run.room().fingerprint(), room: run.at, kind: run.room().kind, home: home_print(run)};
    let pose = (run.tick / 6 % 4) as usize;
    let mut cache = SCENES[i].get_or_init(Default::default).lock().unwrap_or_else(|e|e.into_inner());
    if let Some((old, images)) = &*cache {if *old == key {return images[pose].clone();}}
    // Retain ambient movement from the existing furniture painter. All four
    // poses are baked on admission; display only selects a cached image.
    let images = std::array::from_fn(|frame| {
        let mut sample = run.clone();sample.tick = frame as u64 * 6;
        Arc::new(bake(&sample))
    });
    let im = images[pose].clone();*cache=Some((key,images));im
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn delivered_rooms_keep_master_detail_furniture_and_complete_plaques() {
        for fixture in [Run::latch_for_test as fn()->Run, Run::rafterwake_for_test, Run::pennon_for_test] {
            let run=fixture();let bytes=serde_json::to_vec(&run).unwrap();
            let im=scene(&run);assert_eq!((im.w,im.h),(384,224));
            let src=source(index(run.room().kind).unwrap());
            let old=original(&run);
            for y in 80..176 {for x in 64..320 {
                if run.room().tile(x/16,y/16)==Tile::Floor {
                    assert_eq!(im.get(x,y),src.get(x,y), "interior detail at {x},{y}");
                }
            }}
            if let Some((x,y,w,h))=plaque_bounds(&run) {
                for yy in y..y+h {for xx in x..x+w {
                    assert_eq!(im.get(xx,yy),old.get(xx,yy), "complete title plaque");
                }}
            }
            assert_eq!(bytes,serde_json::to_vec(&run).unwrap());
        }
        for fixture in [Run::cinder_for_test as fn()->Run, Run::mossbell_for_test, Run::brasswake_for_test] {
            assert!(index(fixture().room().kind).is_none(), "retain the designed room painter");
        }
    }
}
