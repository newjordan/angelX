//! The pilot's blockouts agree with the game: walls covered, open ground
//! open and in view of the camera, blocked tiles blocked, hazards low, doors
//! where the door tiles are; and the sets in `assets/world3d` are the
//! generator's.

use super::*;
use crate::rules::RoomKind;
use crate::world3d::pilot::{Item, PILOT, Source, every};

fn assets() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/world3d")
}

fn set_path(root: &std::path::Path, id: &str) -> std::path::PathBuf {
    root.join(id).join(format!("{id}.set"))
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

/// The item's landmark overlay as the group it adds to the room's, when it
/// has one: every check of the room holds it too.
fn landmark_group(item: &Item) -> Option<Group> {
    let text = landmark(item.id())?;
    let solids = landmark_solids(&text).unwrap_or_else(|why| panic!("{}: {}", item.id(), why.join("; ")));
    Some(Group::new(Role::Prop, "landmark", solids))
}

fn each_room(mut check: impl FnMut(&Item, &Room, &[Group])) {
    for item in &every() {
        let room = item.room();
        let mut groups = blockout(&room, &item.dressing);
        groups.extend(landmark_group(item));
        check(item, &room, &groups);
    }
}

fn tiles(room: &Room) -> impl Iterator<Item = (i32, i32)> + '_ {
    (0..room.rows as i32).flat_map(move |r| (0..room.cols as i32).map(move |c| (c, r)))
}

#[test]
fn the_pilot_is_six_places_and_four_chambers_of_four_delves() {
    let places: Vec<RoomKind> = PILOT
        .iter()
        .filter_map(|i| match i.source {
            Source::Place(kind) => Some(kind),
            _ => None,
        })
        .collect();
    let packs: Vec<Pack> = PILOT
        .iter()
        .filter_map(|i| match i.source {
            Source::Chamber { pack, .. } => Some(pack),
            _ => None,
        })
        .collect();
    assert_eq!((places.len(), packs.len()), (6, 4));
    for kind in &places {
        assert!(crate::rules::world::ROOMS.iter().any(|(k, _)| k == kind), "{kind:?} is a room of the world");
        assert!(kind.in_world());
    }
    let indoor = PILOT.iter().filter(|i| i.dressing.setting == Setting::Indoor).count();
    assert!((2..=4).contains(&indoor), "indoor and outdoor places: {indoor} indoor");
    for (k, a) in packs.iter().enumerate() {
        assert!(!packs[k + 1..].contains(a), "{a:?} twice");
    }
    for (k, item) in PILOT.iter().enumerate() {
        let id = item.id();
        assert!(!id.is_empty() && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'), "{id}");
        assert!(!PILOT[k + 1..].iter().any(|o| o.id() == id), "{id} twice");
        assert_eq!(crate::world3d::pilot::item(id).map(|i| i.id()), Some(id));
    }
}

#[test]
fn every_dressing_names_tiles_its_room_has() {
    each_room(|item, room, _| {
        for &((c, r, w, h), prop) in item.dressing.props {
            let mut blocked = 0;
            for y in r..r + h {
                for x in c..c + w {
                    let tile = room.tile(x, y);
                    blocked += (tile == Tile::Block) as usize;
                    let fits = match prop {
                        Prop::Reeds => tile == Tile::Hazard,
                        Prop::Plate(_) | Prop::Gate | Prop::Jetty => tile == Tile::Stairs,
                        Prop::Low(..) | Prop::Heap(..) | Prop::Arch | Prop::Deck(_) => walkable(tile),
                        Prop::Site => matches!(tile, Tile::Block | Tile::Stairs),
                        Prop::Column { .. } => matches!(tile, Tile::Block | Tile::Hazard),
                        // A region: what is blocked in it is the prop's.
                        _ => true,
                    };
                    assert!(fits, "{}: {} at {x},{y} stands on {tile:?}", item.id(), prop.word());
                }
            }
            let on_blocks = !(prop.on_open_ground() || matches!(prop, Prop::Reeds | Prop::Site | Prop::Column { .. }));
            assert!(!on_blocks || blocked > 0, "{}: {} at {c},{r} {w}x{h} holds no blocked tile", item.id(), prop.word());
        }
    });
}

/// What the dressing names, a place's every blocked tile is under: nothing
/// is left to the setting's own default block (a boulder, a crate). The
/// chambers' templates are not dressed tile by tile: their packs' own blocks
/// stand for them.
#[test]
fn every_blocked_tile_of_a_place_is_named() {
    each_room(|item, room, _| {
        if matches!(item.source, Source::Chamber { .. } | Source::Great { .. }) {
            return;
        }
        for t in tiles(room).filter(|&(c, r)| room.tile(c, r) == Tile::Block) {
            let named = item.dressing.props.iter().any(|&(rect, prop)| {
                inside(rect, t.0, t.1) && !prop.on_open_ground() && !matches!(prop, Prop::Reeds)
            });
            assert!(named, "{}: blocked tile {t:?} is left to a default block", item.id());
        }
    });
}

#[test]
fn every_room_of_the_world_and_every_chamber_template_has_an_item() {
    let all = every();
    for (kind, _) in crate::rules::world::ROOMS.iter() {
        assert!(
            all.iter().any(|i| matches!(i.source, Source::Place(k) if k == *kind)),
            "{kind:?} has no item"
        );
    }
    for pack in [Pack::Crypt, Pack::Cavern, Pack::Hellforge, Pack::Archive, Pack::Fungal, Pack::Unknown, Pack::Swamp] {
        for small in 0..crate::rules::rooms::small(pack).len() {
            assert!(
                all.iter().any(|i| matches!(i.source, Source::Chamber { pack: p, small: s, .. } if p == pack && s == small)),
                "{pack:?} small {small} has no item"
            );
        }
        for index in 0..crate::rules::rooms::all_great(pack).len() {
            assert!(
                all.iter().any(|i| matches!(i.source, Source::Great { pack: p, index: n } if p == pack && n == index)),
                "{pack:?} great {index} has no item"
            );
        }
    }
    for kind in crate::rules::nooks::KINDS {
        assert!(all.iter().any(|i| matches!(i.source, Source::Nook(k) if k == kind)), "{kind:?} has no item");
    }
    for source in [Source::Sanctuary, Source::Vault, Source::Forge] {
        assert!(all.iter().any(|i| std::mem::discriminant(&i.source) == std::mem::discriminant(&source)), "{source:?}");
    }
    // Ids: lower case letters, digits and `-`, each once.
    for (k, item) in all.iter().enumerate() {
        let id = item.id();
        assert!(!id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'), "{id}");
        assert!(!all[k + 1..].iter().any(|o| o.id() == id), "{id} twice");
        assert!(!item.dressing.title.is_empty(), "{id}: a title");
        assert!(!item.dressing.what.is_empty() || k < PILOT.len(), "{id}: what it is");
    }
}

#[test]
fn every_wall_tile_is_under_a_wall() {
    each_room(|item, room, groups| check_every_wall_tile_is_under_a_wall(item.id(), item.dressing.setting, room, groups));
}

fn check_every_wall_tile_is_under_a_wall(id: &str, _setting: Setting, room: &Room, groups: &[Group]) {
    let walls: Vec<&Solid> = groups
        .iter()
        .filter(|g| g.role == Role::Wall || g.role == Role::Door)
        .flat_map(|g| &g.solids)
        .filter(|s| s.bottom() <= 0.0 && s.top() >= 2.0)
        .collect();
    for t in tiles(room).filter(|&(c, r)| room.tile(c, r) == Tile::Wall) {
        for p in samples(t, 4, 0.5) {
            assert!(walls.iter().any(|s| under(s, p)), "{}: wall tile {t:?} is bare at {p:?}", id);
        }
    }
}

#[test]
fn walkable_tiles_are_never_under_a_block() {
    each_room(|item, room, groups| check_walkable_tiles_are_never_under_a_block(item.id(), item.dressing.setting, room, groups));
}

fn check_walkable_tiles_are_never_under_a_block(id: &str, _setting: Setting, room: &Room, groups: &[Group]) {
    let blocks: Vec<&Solid> = groups
        .iter()
        .filter(|g| g.role != Role::Ground && g.role != Role::Hazard)
        .flat_map(|g| &g.solids)
        .filter(|s| s.top() > LOW && s.bottom() < HEADROOM)
        .collect();
    for t in tiles(room).filter(|&(c, r)| walkable(room.tile(c, r))) {
        for p in samples(t, 6, 2.0) {
            if let Some(s) = blocks.iter().find(|s| under(s, p)) {
                panic!("{}: walkable tile {t:?} is under `{}` at {p:?}", id, line(s));
            }
        }
    }
}

#[test]
fn walkable_tiles_stay_in_view_of_the_game_camera() {
    each_room(|item, room, groups| check_walkable_tiles_stay_in_view_of_the_game_camera(item.id(), item.dressing.setting, room, groups));
}

fn check_walkable_tiles_stay_in_view_of_the_game_camera(id: &str, _setting: Setting, room: &Room, groups: &[Group]) {
    let view = View::of(room);
    assert_eq!(view.size, (room.cols as u32 * 16, room.rows as u32 * 16), "{}: the room's stage", id);
    let hulls = occluders(&view, groups.iter().filter(|g| g.role != Role::Ground && g.role != Role::Hazard).flat_map(|g| &g.solids));
    for t in open_tiles(room) {
        let hidden = view.hidden(&hulls, t);
        assert!(hidden <= HIDE_ALL, "{}: walkable tile {t:?} is {:.0}% hidden", id, hidden * 100.0);
    }
}

#[test]
fn blocked_tiles_are_under_or_behind_what_blocks_them() {
    each_room(|item, room, groups| check_blocked_tiles_are_under_or_behind_what_blocks_them(item.id(), item.dressing.setting, room, groups));
}

fn check_blocked_tiles_are_under_or_behind_what_blocks_them(id: &str, _setting: Setting, room: &Room, groups: &[Group]) {
    let view = View::of(room);
    let props: Vec<&Group> = groups.iter().filter(|g| g.role == Role::Prop).collect();
    for t in tiles(room).filter(|&(c, r)| room.tile(c, r) == Tile::Block) {
        assert!(
            props.iter().any(|g| g.rect.is_some_and(|r| inside(r, t.0, t.1))),
            "{}: blocked tile {t:?} belongs to no prop",
            id
        );
    }
    for g in &props {
        let Some((c, r, w, h)) = g.rect else { continue };
        let hulls = occluders(&view, &g.solids);
        let blocked: Vec<(i32, i32)> =
            (r..r + h).flat_map(|y| (c..c + w).map(move |x| (x, y))).filter(|&(x, y)| room.tile(x, y) == Tile::Block).collect();
        if blocked.is_empty() {
            continue;
        }
        let shown = blocked
            .iter()
            .filter(|t| {
                let centre = [t.0 as f32 * TILE + 8.0, t.1 as f32 * TILE + 8.0];
                g.solids.iter().any(|s| s.top() > LOW && under(s, centre))
                    || hulls.iter().any(|h| h.contains(view.screen([centre[0], 0.0, centre[1]])))
            })
            .count() as f32
            / blocked.len() as f32;
        // A round crown leaves its corners to the grass, as the painter's
        // tree does; a staked site is open ground roped off.
        let least = if g.name.starts_with("tree") {
            0.5
        } else if g.name.starts_with("site") {
            0.0
        } else {
            1.0
        };
        assert!(shown >= least, "{}: `{}` shows {:.0}% of its blocked tiles as blocked", id, g.name, shown * 100.0);
    }
}

#[test]
fn hazards_lie_below_the_floor_line() {
    each_room(|item, room, groups| check_hazards_lie_below_the_floor_line(item.id(), item.dressing.setting, room, groups));
}

fn check_hazards_lie_below_the_floor_line(id: &str, setting: Setting, room: &Room, groups: &[Group]) {
    let ground: Vec<&Solid> = groups.iter().filter(|g| g.role == Role::Ground).flat_map(|g| &g.solids).collect();
    let wet: Vec<&Solid> = groups.iter().filter(|g| g.role == Role::Hazard).flat_map(|g| &g.solids).collect();
    let pits = matches!(setting, Setting::Dungeon(Pack::Crypt | Pack::Unknown));
    for t in tiles(room).filter(|&(c, r)| room.tile(c, r) == Tile::Hazard) {
        let centre = [t.0 as f32 * TILE + 8.0, t.1 as f32 * TILE + 8.0];
        assert!(!ground.iter().any(|s| under(s, centre)), "{}: ground over hazard {t:?}", id);
        if !pits {
            let pool = wet.iter().find(|s| under(s, centre));
            assert!(pool.is_some_and(|s| s.top() < 0.0), "{}: hazard {t:?} is not below the floor", id);
        }
    }
    for t in tiles(room).filter(|&(c, r)| room.tile(c, r) != Tile::Hazard) {
        let centre = [t.0 as f32 * TILE + 8.0, t.1 as f32 * TILE + 8.0];
        let slab = ground.iter().find(|s| under(s, centre));
        assert!(slab.is_some_and(|s| s.top() == 0.0), "{}: tile {t:?} has no floor", id);
    }
}

#[test]
fn doors_are_gaps_where_the_door_tiles_are() {
    each_room(|item, room, groups| check_doors_are_gaps_where_the_door_tiles_are(item.id(), item.dressing.setting, room, groups));
}

fn check_doors_are_gaps_where_the_door_tiles_are(id: &str, setting: Setting, room: &Room, groups: &[Group]) {
    let (w, h) = (room.cols as f32 * TILE, room.rows as f32 * TILE);
    let gaps = door_gaps(room);
    let open = room.doors.iter().filter(|&&d| d).count();
    assert_eq!(gaps.len(), open, "{}: a gap for each open door", id);
    let masonry = !matches!(setting, Setting::Outdoor { .. });
    for (side, from, to) in gaps {
        assert_eq!(to - from, 2, "{}: a door is two tiles", id);
        let (a, b) = (from as f32 * TILE, to as f32 * TILE);
        let group = groups
            .iter()
            .find(|g| g.role == Role::Door && g.name.starts_with(&format!("the {} door", match side {
                Side::North => "north",
                Side::East => "east",
                Side::South => "south",
                Side::West => "west",
            })))
            .unwrap_or_else(|| panic!("{}: no door group on the {side:?}", id));
        if masonry {
            let leaf = group.solids.iter().find(|s| s.kind == Kind::Door).expect("a door leaf or sill");
            let [x0, z0, x1, z1] = leaf.footprint();
            // Just outside the wall's outer face.
            let (lo, hi, edge) = match side {
                Side::North => (x0, x1, z1),
                Side::South => (x0, x1, z0 - h),
                Side::West => (z0, z1, x1),
                Side::East => (z0, z1, x0 - w),
            };
            assert_eq!((lo, hi, edge), (a, b, 0.0), "{}: the {side:?} door spans its tiles at the wall's face", id);
        }
        // Its flanking wall tiles stand.
        for k in [from - 1, to] {
            let (c, r) = match side {
                Side::North => (k, 0),
                Side::South => (k, room.rows as i32 - 1),
                Side::West => (0, k),
                Side::East => (room.cols as i32 - 1, k),
            };
            assert_eq!(room.tile(c, r), Tile::Wall, "{}: a wall beside the {side:?} door", id);
        }
    }
}

#[test]
fn the_far_wall_stands_full_height_and_the_near_wall_low() {
    each_room(|item, room, groups| check_the_far_wall_stands_full_height_and_the_near_wall_low(item.id(), item.dressing.setting, room, groups));
}

fn check_the_far_wall_stands_full_height_and_the_near_wall_low(id: &str, _setting: Setting, room: &Room, groups: &[Group]) {
    let h = room.rows as f32 * TILE;
    let walls: Vec<&Solid> = groups.iter().filter(|g| g.role == Role::Wall).flat_map(|g| &g.solids).collect();
    let far = walls.iter().filter(|s| s.footprint()[3] <= TILE).map(|s| s.top()).fold(0.0, f32::max);
    let near = walls.iter().filter(|s| s.footprint()[1] >= h - TILE).map(|s| s.top()).fold(0.0, f32::max);
    assert!(far >= 24.0, "{}: the far wall reads ({far})", id);
    assert!(near < far / 2.0, "{}: the near wall ({near}) stays low beside the far ({far})", id);
}


/// The rooms the dice lay on seeded floors (each delve, floors 1 to 3, a
/// party of one and of two), each as a room with no item is stood up
/// (`plain::dressing_for`), and the map of an open floor of each delve piece
/// by piece (each piece's tiles and the ring of the map round them, as
/// `plain` cuts it): every one agrees with the game as the items do. Its
/// walls stand on its wall tiles, nothing over ground a knight walks, the
/// floor in view, its blocks over its blocked tiles, its hazards low, a
/// doorway where each door is.
#[test]
fn rooms_with_no_item_on_seeded_floors_agree_with_the_game() {
    use crate::rules::{Rng, layout};
    use crate::world3d::plain;
    let packs = [Pack::Crypt, Pack::Cavern, Pack::Hellforge, Pack::Archive, Pack::Fungal, Pack::Unknown, Pack::Swamp];
    let (mut rooms, mut pieces) = (0, 0);
    for pack in packs {
        for depth in 1..=3u32 {
            for (seed, party) in [(1u64, 1usize), (2, 2)] {
                let floor = layout::floor_party(depth, pack, party, &mut Rng::new(seed * 7 + u64::from(depth)));
                for (i, room) in floor.rooms.iter().enumerate() {
                    let Some((dressing, _)) = plain::dressing_for(room, pack) else { continue };
                    let id = format!("{pack:?} floor {depth} seed {seed} room {i} ({:?}, {})", room.kind, dressing.id);
                    let groups = blockout(room, &dressing);
                    check_every_wall_tile_is_under_a_wall(&id, dressing.setting, room, &groups);
                    check_walkable_tiles_are_never_under_a_block(&id, dressing.setting, room, &groups);
                    check_walkable_tiles_stay_in_view_of_the_game_camera(&id, dressing.setting, room, &groups);
                    check_blocked_tiles_are_under_or_behind_what_blocks_them(&id, dressing.setting, room, &groups);
                    check_hazards_lie_below_the_floor_line(&id, dressing.setting, room, &groups);
                    check_doors_are_gaps_where_the_door_tiles_are(&id, dressing.setting, room, &groups);
                    check_the_far_wall_stands_full_height_and_the_near_wall_low(&id, dressing.setting, room, &groups);
                    rooms += 1;
                }
            }
        }
        let floor = layout::open_floor(2, pack, 2, &mut Rng::new(11));
        let map = floor.rooms.iter().find(|r| r.kind == RoomKind::Depths).expect("an open floor's map");
        let (dressing, _) = plain::dressing_for(map, pack).expect("a map is drawn so");
        let grid = ((map.cols as i32 + plain::PIECE.0 - 1) / plain::PIECE.0, (map.rows as i32 + plain::PIECE.1 - 1) / plain::PIECE.1);
        for cz in 0..grid.1 {
            for cx in 0..grid.0 {
                let (_, crop) = plain::piece_rects(map, true, (cx, cz));
                let cut = plain::crop_room(map, crop);
                let id = format!("{pack:?}'s open map, piece {cx},{cz}");
                let groups = blockout(&cut, &dressing);
                check_every_wall_tile_is_under_a_wall(&id, dressing.setting, &cut, &groups);
                check_walkable_tiles_are_never_under_a_block(&id, dressing.setting, &cut, &groups);
                check_walkable_tiles_stay_in_view_of_the_game_camera(&id, dressing.setting, &cut, &groups);
                check_blocked_tiles_are_under_or_behind_what_blocks_them(&id, dressing.setting, &cut, &groups);
                check_hazards_lie_below_the_floor_line(&id, dressing.setting, &cut, &groups);
                pieces += 1;
            }
        }
    }
    assert!(rooms > 100 && pieces > 100, "{rooms} rooms and {pieces} pieces checked");
}

#[test]
fn every_set_is_tens_to_a_few_hundred_well_formed_lines() {
    let names = angel_stage::shapes::NAMES;
    for shape in Shape::ALL {
        assert!(names.contains(&shape.name()), "{} is a built-in shape", shape.name());
    }
    for item in &every() {
        let set = item.set();
        let lines: Vec<&str> = set.lines().collect();
        let room = item.room();
        // A bigger room has more to it, a tile at a time in places.
        let most = if room.cols * room.rows > 24 * 14 { 2000 } else { 400 };
        assert!((20..=most).contains(&lines.len()), "{}: {} lines", item.id(), lines.len());
        assert_eq!(lines[0], format!("set {}", item.id()));
        let camera = format!("camera room {} {}", room.cols * 16, room.rows * 16);
        assert!(lines.contains(&camera.as_str()), "{}: {camera}", item.id());
        assert!(lines.contains(&"plate 1536 1024"), "{}", item.id());
        let mut shapes = 0;
        for (n, l) in lines.iter().enumerate() {
            let body = l.split('#').next().unwrap().trim();
            if body.is_empty() || body.starts_with("set ") || body.starts_with("camera ") || body.starts_with("plate ") {
                continue;
            }
            let words: Vec<&str> = body.split_whitespace().collect();
            let at = |w: &str| words.iter().position(|x| *x == w);
            assert!(angel_stage::shapes::by_name(words[0]).is_some(), "{}:{}: shape {}", item.id(), n + 1, words[0]);
            assert!(Kind::ALL.iter().any(|k| k.name() == words[1]), "{}:{}: kind {}", item.id(), n + 1, words[1]);
            assert_eq!((at("at"), at("size")), (Some(2), Some(6)), "{}:{}: {l}", item.id(), n + 1);
            for v in &words[3..6].iter().chain(&words[7..10]).collect::<Vec<_>>() {
                assert!(v.parse::<f32>().is_ok_and(f32::is_finite), "{}:{}: {v}", item.id(), n + 1);
            }
            for w in &words[7..10] {
                assert!(w.parse::<f32>().unwrap() > 0.0, "{}:{}: a size is positive: {l}", item.id(), n + 1);
            }
            if let Some(i) = at("id") {
                let id = words[i + 1];
                assert!(id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'), "{id}");
            }
            shapes += 1;
        }
        assert!(shapes >= 6, "{}: {shapes} shapes", item.id());
        // Every id once.
        let ids: Vec<&str> = lines.iter().filter_map(|l| l.split('#').next().unwrap().split(" id ").nth(1)).map(str::trim).collect();
        for (k, id) in ids.iter().enumerate() {
            assert!(!ids[k + 1..].contains(id), "{}: id {id} twice", item.id());
        }
    }
}

#[test]
fn the_committed_sets_are_the_generators() {
    for item in &every() {
        let path = set_path(&assets(), item.id());
        let committed = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            committed == item.set(),
            "{} is stale or missing: regenerate with ANGEL_WORLD3D_SETS=1 cargo test -p angel-delve write_world3d_sets -- --ignored",
            path.display()
        );
    }
}

/// Writes every item's set to `assets/world3d/<item>/<item>.set`, or under
/// the folder `ANGEL_WORLD3D_SETS` names (when it is not `1`).
#[test]
#[ignore = "writes the blockout sets"]
fn write_world3d_sets() {
    let Some(to) = std::env::var_os("ANGEL_WORLD3D_SETS") else {
        return;
    };
    let root = if to == "1" { assets() } else { std::path::PathBuf::from(to) };
    for item in &every() {
        let path = set_path(&root, item.id());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let set = item.set();
        std::fs::write(&path, &set).unwrap();
        println!("{:>4} lines  {}", set.lines().count(), path.display());
    }
}

#[test]
fn merging_makes_few_rectangles_that_cover_every_cell_once() {
    let cells: Vec<Option<u8>> = [
        "aab.",
        "aab.",
        "ccb.",
    ]
    .iter()
    .flat_map(|r| r.bytes().map(|b| (b != b'.').then_some(b)))
    .collect();
    let rects = merge(4, 3, &cells);
    assert_eq!(rects, vec![((0, 0, 2, 2), b'a'), ((2, 0, 1, 3), b'b'), ((0, 2, 2, 1), b'c')]);
}

#[test]
fn the_camera_sees_what_stands_up_over_the_floor_behind_it() {
    let room = PILOT[0].room();
    let view = View::of(&room);
    // A tall block hides the floor north of it, never south of it.
    let wall = Solid::block(Kind::Stone, px((10, 6, 2, 1)), 0.0, 32.0);
    let hulls = occluders(&view, [&wall]);
    assert!(view.hidden(&hulls, (10, 5)) > 0.9, "the tile behind");
    assert!(view.hidden(&hulls, (10, 4)) > 0.5, "and the one behind that");
    assert_eq!(view.hidden(&hulls, (10, 7)), 0.0, "the tile in front");
    // The far wall hides nothing in the room.
    let far = Solid::block(Kind::Stone, px((0, 0, 24, 1)), 0.0, WALL);
    let hulls = occluders(&view, [&far]);
    assert!(open_tiles(&room).iter().all(|&t| view.hidden(&hulls, t) == 0.0));
    // A boot-high thing hides nothing.
    assert!(occluders(&view, [&Solid::block(Kind::Metal, px((5, 5, 1, 1)), 0.0, LOW)]).is_empty());
}

#[test]
fn a_trimmed_group_keeps_its_stacking() {
    let room = PILOT[3].room();
    let groups = blockout(&room, &PILOT[3].dressing);
    let trimmed: Vec<&Group> = groups.iter().filter(|g| g.kept < 1.0).collect();
    assert!(!trimmed.is_empty(), "the village's south cottages are sized for the camera");
    for g in groups.iter().filter(|g| g.name.starts_with("cottage")) {
        let (walls, roof) = (&g.solids[0], &g.solids[1]);
        assert_eq!(walls.top(), roof.bottom(), "{}: the roof sits on the walls", g.name);
    }
}


/// Where an item's landmark begins in its set text.
const LANDMARK_MARK: &str = "\n# landmark (";

#[test]
fn every_set_parses_with_no_refusals() {
    assert_eq!(angel_stage::set::GAME_TILT, crate::render::overworld::arena::stage_frame::TILT, "the set's camera is the game's");
    for item in &every() {
        let text = item.set();
        // The generated part, then the landmark's lines.
        let (generated, landmark) = match text.split_once(LANDMARK_MARK) {
            Some((g, l)) => (format!("{g}\n"), l.split_once('\n').map_or("", |(_, rest)| rest)),
            None => (text.clone(), ""),
        };
        let set = match angel_stage::set::Set::parse(&generated) {
            Ok(set) => set,
            Err(refusals) => panic!(
                "{}: {}",
                item.id(),
                refusals.iter().map(|r| r.to_string()).collect::<Vec<_>>().join("; ")
            ),
        };
        assert_eq!(set.id, item.id());
        let room = item.room();
        assert_eq!(set.camera, angel_stage::set::SetCamera::Room { size: (room.cols as u32 * 16, room.rows as u32 * 16) });
        assert_eq!(set.plate, PLATE);
        let solids: Vec<Solid> = blockout(&room, &item.dressing).into_iter().flat_map(|g| g.solids).collect();
        assert_eq!(set.shapes.len(), solids.len(), "{}: a shape to each solid", item.id());
        for (shape, solid) in set.shapes.iter().zip(&solids) {
            assert_eq!((shape.shape.as_str(), shape.kind), (solid.shape.name(), solid.kind), "{}: line {}", item.id(), shape.line);
            for k in 0..3 {
                assert!((shape.at[k] - solid.at[k]).abs() < 0.01 && (shape.size[k] - solid.size[k]).abs() < 0.01, "{}: line {}", item.id(), shape.line);
            }
        }
        // The landmark: read by the same parser, the whole set reads as one,
        // its landmark's blocks after the generator's.
        let ours = landmark_solids(landmark).unwrap_or_else(|why| panic!("{}: {}", item.id(), why.join("; ")));
        let whole = angel_stage::set::Set::parse(&text).unwrap_or_else(|refusals| {
            panic!("{}: {}", item.id(), refusals.iter().map(|r| r.to_string()).collect::<Vec<_>>().join("; "))
        });
        assert_eq!(whole.shapes.len(), solids.len() + ours.len(), "{}", item.id());
        for (shape, solid) in whole.shapes[solids.len()..].iter().zip(&ours) {
            assert_eq!((shape.kind, shape.turn, shape.id.as_ref()), (solid.kind, solid.turn, solid.id.as_ref()), "{}: line {}", item.id(), shape.line);
            assert_eq!((shape.at, shape.size), (solid.at, solid.size), "{}: line {}", item.id(), shape.line);
        }
    }
}

#[test]
fn the_view_is_the_sets_game_camera() {
    let room = PILOT[0].room();
    let view = View::of(&room);
    let camera = angel_stage::set::SetCamera::Room { size: view.size }.game();
    let angel_stage::scene::Projection::Custom(m) = camera.projection else {
        panic!("the stage camera is a custom projection");
    };
    for p in [[0.0, 0.0, 0.0], [192.0, 0.0, 112.0], [384.0, 48.0, 16.0], [16.0, 30.0, 200.0]] {
        // Column-major: m[column][row].
        let clip: [f32; 4] = std::array::from_fn(|r| (0..3).map(|c| m[c][r] * p[c]).sum::<f32>() + m[3][r]);
        let (w, h) = (view.size.0 as f32, view.size.1 as f32);
        let at = [(clip[0] / clip[3] + 1.0) / 2.0 * w, (1.0 - clip[1] / clip[3]) / 2.0 * h];
        let ours = view.screen(p);
        assert!((at[0] - ours[0]).abs() < 0.05 && (at[1] - ours[1]).abs() < 0.05, "{p:?}: {at:?} vs {ours:?}");
    }
}



/// Writes the second wave's items, for the art lane, to the file
/// `ANGEL_WORLD3D_ITEMS` names: a list of `{ id, set, setting, title,
/// notes }`, the pilot's ten left out.
#[test]
#[ignore = "writes the second wave's item list"]
fn write_world3d_items() {
    let Some(to) = std::env::var_os("ANGEL_WORLD3D_ITEMS") else {
        return;
    };
    let items: Vec<serde_json::Value> = every()
        .iter()
        .skip(PILOT.len())
        .map(|item| {
            serde_json::json!({
                "id": item.id(),
                "set": set_path(&assets(), item.id()).display().to_string(),
                "setting": item.setting_word(),
                "title": item.dressing.title,
                "notes": item.painter_notes(),
            })
        })
        .collect();
    std::fs::write(&to, serde_json::to_string_pretty(&items).unwrap() + "\n").unwrap();
    println!("{} items to {}", items.len(), std::path::Path::new(&to).display());
}

// ------------------------------------------------------------ landmarks ----

/// A landmark overlay on a room of the pilot, as a generator would read it.
fn overlay_on(item: &Item, text: &str) -> (Room, Vec<Group>, Vec<Solid>) {
    let room = item.room();
    let groups = blockout(&room, &item.dressing);
    let solids = landmark_solids(text).unwrap_or_else(|why| panic!("{}", why.join("; ")));
    (room, groups, solids)
}

#[test]
fn a_landmark_is_appended_to_the_generated_set() {
    let item = &PILOT[0];
    let (room, dressing) = (item.room(), &item.dressing);
    let plain = room_set(&room, dressing);
    assert_eq!(room_set_with(&room, dressing, None), plain);
    let overlay = "# a tower\ncylinder stone at 100 0 20 size 20 60 20 id lm-tower\ncone slate at 100 60 20 size 24 20 24\n";
    let with = room_set_with(&room, dressing, Some(overlay));
    assert!(with.starts_with(&plain), "the generated set comes first, untouched");
    let tail = &with[plain.len()..];
    assert!(tail.contains(&format!("# landmark (assets/world3d/{}/landmark.set", dressing.id)));
    assert!(tail.ends_with(overlay), "the overlay's own lines follow, as written");
}

#[test]
fn a_landmark_reads_the_set_syntax_and_refuses_the_rest() {
    let ok = landmark_solids(
        "rock-7 rock at 40 0 20 size 30 20 30\nfrustum stone at 100 0 20 size 24 50 24 turn 45 id lm-a\nbox plaster at 60 0 20 size 8 8 8  # fine\n",
    )
    .unwrap();
    assert_eq!(ok.len(), 3);
    assert_eq!((ok[1].turn, ok[1].id.as_deref(), ok[1].kind), (45.0, Some("lm-a"), Kind::Stone));
    for bad in [
        "set x",
        "camera room 100 100",
        "plate 10 10",
        "castle stone at 0 0 0 size 1 1 1",
        "box stone at 0 0 0 size 1 1 1 id Bad",
        "box-3 stone at 0 0 0 size 1 1 1",
        "rock- rock at 0 0 0 size 1 1 1",
        "box lilac at 0 0 0 size 1 1 1",
        "box stone at 0 0 size 1 1 1",
        "box stone at 0 0 0 size 1 0 1",
        "box stone at 0 0 0 size 1 1 1 turn 4 turn 5",
        "box stone at 0 0 0 size 1 1 1 spin 4",
    ] {
        let why = landmark_solids(bad).expect_err(bad);
        assert!(why[0].starts_with("landmark line 1:"), "{bad}: {why:?}");
    }
    // Every bad line is told, with its number.
    assert_eq!(landmark_solids("box stone at 0 0 0 size 1 1 1\nset a\n\nplate 1 1\n").unwrap_err().len(), 2);
}

#[test]
fn a_landmark_that_breaks_the_rooms_agreement_is_caught() {
    // The village (PILOT[3]): an open green, walls round it.
    let item = &PILOT[3];
    let room = item.room();
    let (c, r) = open_tiles(&room).into_iter().find(|&(c, r)| room.tile(c, r) == Tile::Floor && r > 4).unwrap();
    let centre = (c as f32 * TILE + 8.0, r as f32 * TILE + 8.0);
    let on_floor = format!("box stone at {} 0 {} size 8 30 8", centre.0, centre.1);
    let (room, groups, solids) = overlay_on(item, &on_floor);
    let faults = landmark_faults(&room, &groups, &solids);
    assert!(faults.iter().any(|f| f.contains("stands over walkable tiles")), "{faults:?}");
    assert!(faults.iter().any(|f| f.contains("hides")), "a wall in the open floor hides the floor behind it: {faults:?}");
    // The same block high over the floor is a lintel, but it is out of the plate.
    let (room, groups, solids) = overlay_on(item, &format!("box stone at {} 40 {} size 8 4 8", centre.0, centre.1));
    assert!(landmark_faults(&room, &groups, &solids).iter().all(|f| !f.contains("stands over")), "a lintel may span the floor");
    let (room, groups, solids) = overlay_on(item, "box stone at 40 0 4 size 20 900 8");
    assert!(landmark_faults(&room, &groups, &solids).iter().any(|f| f.contains("out of the plate")));
    // A name the generator already uses is not ours to take.
    let taken = groups.iter().flat_map(|g| &g.solids).find_map(|s| s.id.clone()).expect("the village names its cottages");
    let (room, groups, solids) = overlay_on(item, &format!("box stone at 40 0 4 size 8 8 8 id {taken}"));
    assert!(landmark_faults(&room, &groups, &solids).iter().any(|f| f.contains("already the set's")));
}

/// A landmark stands only on the room's blocked tiles, hides nothing it
/// should not, and keeps to the plate: the rooms that have one.
#[test]
fn landmarks_stand_only_on_blocked_tiles_and_keep_the_floor_in_view() {
    let ids: Vec<&str> = every().iter().map(|i| i.id()).collect();
    let mut broken: Vec<String> = Vec::new();
    for item in &every() {
        let Some(text) = landmark(item.id()) else { continue };
        let (room, groups, solids) = overlay_on(item, &text);
        assert!(solids.len() >= 4, "{}: a landmark is a structure, not a mark", item.id());
        broken.extend(landmark_faults(&room, &groups, &solids).into_iter().map(|f| format!("{}: {f}", item.id())));
        // The set the generator writes carries it, after its own.
        let set = item.set();
        assert!(set.contains(LANDMARK_MARK), "{}", item.id());
    }
    assert!(broken.is_empty(), "{}", broken.join("\n"));
    // No landmark file sits under a name that is no item.
    for entry in std::fs::read_dir(assets()).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if entry.path().join("landmark.set").exists() {
            assert!(ids.contains(&name.as_str()), "assets/world3d/{name}/landmark.set belongs to no item");
        }
    }
}

/// Lit rooms (`rooms_lit`, docs/REALM-MONOLITH.md "Rooms, lit"): every
/// item's room and the rooms the dice lay on seeded floors (each delve,
/// floors 1 to 3, a party of one and of two), and an open floor's map piece
/// by piece, as their masses are made monolithic (`rooms_lit_mass`), agree
/// with the game as the generator's blockouts do: walls on their wall tiles,
/// nothing over ground a knight walks, the floor in view, blocked tiles
/// blocked, hazards low.
#[test]
fn lit_rooms_agree_with_the_game() {
    use crate::rules::{Rng, layout};
    use crate::world3d::{plain, rooms_lit_mass};
    let check = |id: &str, setting: Setting, room: &Room, groups: &[Group]| {
        check_every_wall_tile_is_under_a_wall(id, setting, room, groups);
        check_walkable_tiles_are_never_under_a_block(id, setting, room, groups);
        check_walkable_tiles_stay_in_view_of_the_game_camera(id, setting, room, groups);
        check_blocked_tiles_are_under_or_behind_what_blocks_them(id, setting, room, groups);
        check_hazards_lie_below_the_floor_line(id, setting, room, groups);
    };
    let mut rooms = 0;
    each_room(|item, room, groups| {
        let mut groups = groups.to_vec();
        rooms_lit_mass::monolith(&mut groups, room, &item.dressing);
        check(&format!("{} lit", item.id()), item.dressing.setting, room, &groups);
        rooms += 1;
    });
    let packs = [Pack::Crypt, Pack::Cavern, Pack::Hellforge, Pack::Archive, Pack::Fungal, Pack::Unknown, Pack::Swamp];
    for pack in packs {
        for depth in 1..=3u32 {
            for (seed, party) in [(1u64, 1usize), (2, 2)] {
                let floor = layout::floor_party(depth, pack, party, &mut Rng::new(seed * 7 + u64::from(depth)));
                for (i, room) in floor.rooms.iter().enumerate() {
                    let Some((dressing, _)) = plain::dressing_for(room, pack) else { continue };
                    let id = format!("{pack:?} floor {depth} seed {seed} room {i} ({:?}, {}) lit", room.kind, dressing.id);
                    let mut groups = blockout(room, &dressing);
                    rooms_lit_mass::monolith(&mut groups, room, &dressing);
                    check(&id, dressing.setting, room, &groups);
                    rooms += 1;
                }
            }
        }
        let floor = layout::open_floor(2, pack, 2, &mut Rng::new(11));
        let map = floor.rooms.iter().find(|r| r.kind == RoomKind::Depths).expect("an open floor's map");
        let (dressing, _) = plain::dressing_for(map, pack).expect("a map is drawn so");
        for cz in 0..2 {
            for cx in 0..2 {
                let (_, crop) = plain::piece_rects(map, true, (cx, cz));
                let cut = plain::crop_room(map, crop);
                let mut groups = blockout(&cut, &dressing);
                rooms_lit_mass::monolith_in(&mut groups, &cut, &dressing, map, (crop.0, crop.1));
                check(&format!("{pack:?}'s open map, piece {cx},{cz} lit"), dressing.setting, &cut, &groups);
            }
        }
    }
    assert!(rooms > 300, "{rooms} rooms checked");
}
