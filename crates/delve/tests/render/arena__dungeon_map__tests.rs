use super::*;
use crate::rules::votes::{Action, Command};
use std::collections::BTreeMap;

/// A run on floor two, the party at its entrance.
fn on_floor(seed: u64) -> Run {
    let mut run = Run::new(seed, 17, None);
    run.descend_for_test();
    assert_eq!(run.dungeon.depth, 2);
    run
}

fn write_png(img: &Img, k: u32, path: &std::path::Path) {
    let (w, h) = (img.w as u32, img.h as u32);
    let mut out = image::RgbImage::new(w * k, h * k);
    for y in 0..h {
        for x in 0..w {
            let c = img.get(x as i32, y as i32).unwrap_or([0, 0, 0]);
            for dy in 0..k {
                for dx in 0..k {
                    out.put_pixel(x * k + dx, y * k + dy, image::Rgb(c));
                }
            }
        }
    }
    out.save(path).unwrap();
}

fn on_palette(img: &Img) -> bool {
    img.pixels()
        .all(|c| c == BLACK || super::super::super::ink::palette_index(c).is_some())
}

/// The seat's whole screen's room view (the band's rows taken off) as parts
/// composed.
fn composed(run: &Run, focus: Option<u32>, map: &MapView) -> Img {
    seat_scene_with(run, NATIVE_W, NATIVE_H + BAND_H + HUD_H, focus, map).compose()
}

#[test]
fn the_minimap_stays_inside_its_box_in_the_corner_and_on_the_palette() {
    for seed in 1..=12 {
        let run = midfloor(seed);
        let seat = run.view_for_seat(1).unwrap();
        let panel = minimap_image(&seat, Some(1))
            .unwrap_or_else(|| panic!("seed {seed}: a floor being explored has its minimap"));
        assert_eq!((panel.w, panel.h), (MINI_W, MINI_H), "its box");
        assert!(on_palette(&panel), "seed {seed}: off the palette");
        // Drawn on a view, it touches nothing but its box, in the top right
        // corner clear of the view's edges.
        let mut view = Img::new(NATIVE_W, NATIVE_H);
        minimap(&mut view, &seat, Some(1));
        let (x0, y0) = (NATIVE_W - MINI_W - MINI_MARGIN, MINI_MARGIN);
        let mut inked = 0;
        for y in 0..NATIVE_H {
            for x in 0..NATIVE_W {
                if view.get(x, y).is_some() {
                    inked += 1;
                    assert!(
                        (x0..x0 + MINI_W).contains(&x) && (y0..y0 + MINI_H).contains(&y),
                        "seed {seed}: a pixel at {x},{y} is out of its box"
                    );
                }
            }
        }
        assert!(inked > 1500, "the panel is drawn ({inked} pixels)");
        assert!(x0 + MINI_W < NATIVE_W && y0 > 0);
        // A corner: a thirtieth of the view, no more.
        const { assert!(MINI_W * MINI_H * 25 < NATIVE_W * NATIVE_H) };
    }
}

#[test]
fn the_minimap_is_on_the_seats_screen_under_the_band_and_clear_of_the_way_ons_label() {
    let mut labelled = 0;
    for seed in 1..=12 {
        let run = midfloor(seed);
        let seat = run.view_for_seat(1).unwrap();
        labelled += usize::from(seat.way_on().is_some());
        let panel = band_minimap(&seat, Some(1)).unwrap_or_else(|| panic!("seed {seed}: no minimap"));
        assert_eq!((panel.w, panel.h), (MINI_W, MINI_BAND_H));
        let whole = composed(&seat, Some(1), &MapView::default());
        // On a seat's screen the minimap is in the HUD's band under the
        // room's view, at its right end, and not on the view at all. Nothing
        // else of the screen's marks (the label by a doorway's arrow, say)
        // lies over it.
        let (x0, y0) = (NATIVE_W - MINI_W - 4, BAND_H + NATIVE_H + (HUD_H - MINI_BAND_H + 1) / 2);
        let in_view = minimap_image(&seat, Some(1)).unwrap();
        let (vx, vy) = (NATIVE_W - MINI_W - MINI_MARGIN, BAND_H + MINI_MARGIN);
        let on_view = (0..MINI_H).flat_map(|y| (0..MINI_W).map(move |x| (x, y))).filter(|&(x, y)| in_view.get(x, y).is_some() && whole.get(vx + x, vy + y) == in_view.get(x, y)).count();
        assert!(on_view < 600, "seed {seed}: the minimap is off the view ({on_view} pixels match)");
        let mut same = 0;
        for y in 0..MINI_BAND_H {
            for x in 0..MINI_W {
                if let Some(c) = panel.get(x, y) {
                    assert_eq!(
                        whole.get(x0 + x, y0 + y),
                        Some(c),
                        "seed {seed}: minimap pixel {x},{y}"
                    );
                    same += 1;
                }
            }
        }
        assert!(same > 1200, "seed {seed}: {same}");
        // And the band above is the band.
        let band = party::band(&seat, Some(1));
        for y in 0..BAND_H {
            for x in 0..NATIVE_W {
                if let Some(c) = band.get(x, y) {
                    assert_eq!(whole.get(x, y), Some(c), "band pixel {x},{y}");
                }
            }
        }
    }
    assert!(labelled > 0, "some floor has a way on to label");
}

#[test]
fn the_minimap_marks_every_knight_and_shows_the_room_the_seat_stands_in_at_its_middle() {
    let run = midfloor(5);
    let seat = run.view_for_seat(1).unwrap();
    let map = seat.floor_map(Some(1));
    assert_eq!(
        map.party.len(),
        3,
        "the party's own view still counts the knights waiting outside"
    );
    let panel = minimap_of(&map).unwrap();
    // The seat's own room is lit at the middle: its rim is lit brass.
    let (x, y) = HERE;
    assert_eq!(
        panel.get(x + 4, y),
        Some(super::super::super::ink::ink('O').unwrap()),
        "the seat's room, lit"
    );
    // The seat's own dot has the brass ring, the others' do not.
    let ring = super::super::super::ink::ink('O').unwrap();
    let lit = |m: &Marker| {
        let node = map.node(m.node).unwrap();
        let rect = Frame {
            ox: HERE.0 as f32 - map.node(map.here.unwrap()).unwrap().rect.x * PITCH,
            oy: HERE.1 as f32 - map.node(map.here.unwrap()).unwrap().rect.y * PITCH,
            k: PITCH,
        }
        .rect(&node.rect);
        (rect.0..rect.0 + rect.2)
            .any(|px| (rect.1..rect.1 + rect.3).any(|py| panel.get(px, py) == Some(ring)))
    };
    assert!(map.party.iter().filter(|m| m.own).all(lit));
}

#[test]
fn the_minimap_is_there_only_while_a_floor_is_fought_or_explored() {
    let run = midfloor(5);
    let seat = run.view_for_seat(1).unwrap();
    assert!(shown(&seat, Some(1)));
    assert!(shown(&seat, None));
    // Fighting counts too.
    let mut fighting = seat.clone();
    fighting.phase = Phase::Fighting;
    assert!(shown(&fighting, Some(1)));
    // Not at home or in the kingdom.
    let home = Run::at_home(41, 1, None, Default::default(), Default::default());
    assert!(!shown(&home, Some(1)));
    assert!(minimap_image(&home, Some(1)).is_none());
    // Not after the run is won or lost.
    for phase in [Phase::Won, Phase::Wiped] {
        let mut over = seat.clone();
        over.phase = phase;
        assert!(!shown(&over, Some(1)), "{phase:?}");
    }
    // Not behind the ballot.
    let mut voting = seat.clone();
    voting.vote_command(
        1,
        Command {
            serial: 1,
            action: Action::Toggle,
        },
    );
    assert!(voting.playtest.open(1));
    assert!(!shown(&voting, Some(1)));
    assert!(
        shown(&voting, Some(2)) || !voting.playtest.open(2),
        "a friend's ballot is theirs"
    );
    // Not while a floor's plate holds the screen.
    let mut plate = seat.clone();
    let playing = crate::rules::floor_plates::Playing {
        kind: crate::rules::floor_plates::Kind::Crypt,
        depth: 2,
        age: 0,
        results: 0,
    };
    plate.floor_plates.insert(1, playing);
    assert!(!shown(&plate, Some(1)));
}

#[test]
fn the_minimap_keeps_off_a_follys_standings_and_its_lanes() {
    // The even floors' folly is the Belfry shoot: its standings are top
    // right, where the minimap would be.
    let mut run = on_floor(41);
    run.join_seat(2, "Two");
    let index = run.folly_room.expect("a folly hall on floor two");
    assert!(shown(&run, Some(1)));
    for hero in run.players.values_mut() {
        hero.carried.add(crate::realm::Spoil::Gold, 40);
    }
    run.enter(index, Some(2));
    assert!(run.bats.is_some());
    assert!(!shown(&run, Some(1)), "the standings have the corner");
    assert!(minimap_image(&run, Some(1)).is_none());
    let mut view = Img::new(NATIVE_W, NATIVE_H);
    minimap(&mut view, &run, Some(1));
    assert!(
        view.pixels().all(|c| c == BLACK) && view.words().iter().all(|w| w >> 24 == 0),
        "nothing is drawn"
    );
}

#[test]
fn a_nook_s_game_holds_the_screen_alone() {
    let mut run = on_floor(7);
    let nook = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind.nook())
        .expect("floor two keeps a nook");
    run.enter(nook, Some(2));
    assert!(!shown(&run, Some(1)));
}

#[test]
fn the_full_map_is_held_not_kept_and_lays_the_floor_over_the_view() {
    let run = midfloor(5);
    let seat = run.view_for_seat(1).unwrap();
    let plain = composed(&seat, Some(1), &MapView::default());
    let held = composed(
        &seat,
        Some(1),
        &MapView {
            full: true,
            model: None,
            ..Default::default()
        },
    );
    assert!(plain.words() != held.words());
    // Released, the view is as it was.
    let released = composed(
        &seat,
        Some(1),
        &MapView {
            full: false,
            model: Some(run.floor_map(Some(1))),
            ..Default::default()
        },
    );
    assert!(plain.words() == released.words());
    // The whole party's map can be handed in. A seat's own view places the
    // knights waiting outside too, but cannot name them.
    let (mine, whole) = (seat.floor_map(Some(1)), run.floor_map(Some(1)));
    let nameless = |m: &FloorMap| {
        m.party
            .iter()
            .map(|k| (k.knight, k.node, k.at))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        (mine.nodes.clone(), mine.links.clone(), nameless(&mine)),
        (whole.nodes.clone(), whole.links.clone(), nameless(&whole))
    );
    assert_eq!(
        whole
            .party
            .iter()
            .map(|k| k.name.as_str())
            .collect::<Vec<_>>(),
        ["You", "Wren", "Bors"]
    );
    let given = composed(
        &seat,
        Some(1),
        &MapView {
            full: true,
            model: Some(whole),
            ..Default::default()
        },
    );
    assert!(
        held.words() != given.words(),
        "the names are the whole party's"
    );
}

#[test]
fn the_full_map_is_on_the_palette_and_the_room_shows_through_it() {
    let run = midfloor(5);
    let map = run.floor_map(Some(1));
    let overlay = overlay_of(&map).expect("a floor with rooms on it");
    assert_eq!((overlay.w, overlay.h), (NATIVE_W, NATIVE_H));
    assert!(on_palette(&overlay));
    // A dither: inside the panel, away from the sidebar, some of the paper
    // is there and some is clear, about half and half.
    let (px, py, _, ph) = PANEL;
    let (mut there, mut clear) = (0, 0);
    for y in py + 6..py + ph - 6 {
        for x in px + 6..SIDEBAR.0 - 6 {
            if overlay.get(x, y).is_some() {
                there += 1;
            } else {
                clear += 1;
            }
        }
    }
    let share = there as f32 / (there + clear) as f32;
    assert!(
        (0.4..0.75).contains(&share),
        "{share:.2} of the panel is paper"
    );
    // Where it is clear the room is what the view shows.
    let seat = run.view_for_seat(1).unwrap();
    let plain = composed(&seat, Some(1), &MapView::default());
    let held = composed(
        &seat,
        Some(1),
        &MapView {
            full: true,
            model: None,
            ..Default::default()
        },
    );
    let mut shown_through = 0;
    for y in py + 6..py + ph - 6 {
        for x in px + 6..SIDEBAR.0 - 6 {
            if overlay.get(x, y).is_none() {
                assert_eq!(
                    held.get(x, y + BAND_H),
                    plain.get(x, y + BAND_H),
                    "play shows at {x},{y}"
                );
                shown_through += 1;
            }
        }
    }
    assert!(shown_through > 5000);
    // The legend sidebar is solid, so its words read.
    let (sx, sy, sw, sh) = SIDEBAR;
    assert!(
        (sy + 4..sy + sh - 4).all(|y| (sx + 4..sx + sw - 4).all(|x| overlay.get(x, y).is_some()))
    );
}

#[test]
fn the_full_map_leaves_the_band_the_skills_bar_s_row_and_the_hud_alone() {
    let run = midfloor(5);
    let seat = run.view_for_seat(1).unwrap();
    let plain = composed(&seat, Some(1), &MapView::default());
    let held = composed(
        &seat,
        Some(1),
        &MapView {
            full: true,
            model: None,
            ..Default::default()
        },
    );
    // The party's band above the view.
    for y in 0..BAND_H {
        for x in 0..NATIVE_W {
            assert_eq!(held.get(x, y), plain.get(x, y), "band pixel {x},{y}");
        }
    }
    // The skills bar's strip along the view's foot.
    let (_, py, _, ph) = PANEL;
    for y in py + ph..NATIVE_H {
        for x in 0..NATIVE_W {
            assert_eq!(
                held.get(x, y + BAND_H),
                plain.get(x, y + BAND_H),
                "foot pixel {x},{y}"
            );
        }
    }
}

#[test]
fn the_full_map_is_the_same_in_a_window_and_in_a_terminal() {
    for seed in [3, 5, 9] {
        let run = midfloor(seed);
        let seat = run.view_for_seat(1).unwrap();
        for full in [false, true] {
            let map = MapView {
                full,
                model: Some(run.floor_map(Some(1))),
                ..Default::default()
            };
            let (w, h) = (NATIVE_W, NATIVE_H + BAND_H + HUD_H);
            let parts = seat_scene_with(&seat, w, h, Some(1), &map).compose();
            let frame = seat_frame_with(&seat, w, h, Some(1), &map);
            assert!(parts.words() == frame.words(), "seed {seed}, held: {full}");
        }
    }
}

#[test]
fn the_full_map_is_drawn_at_the_art_kits_scale_too() {
    let run = midfloor(5);
    let seat = run.view_for_seat(1).unwrap();
    let map = MapView {
        full: true,
        model: None,
        ..Default::default()
    };
    let one = seat_frame_with(&seat, NATIVE_W, NATIVE_H + BAND_H + HUD_H, Some(1), &map);
    let two = party::lift_frame_with(
        &Img::black(NATIVE_W * 2, NATIVE_H * 2),
        &seat,
        Some(1),
        &map,
    );
    assert_eq!((two.w, two.h), (NATIVE_W * 2, NATIVE_H * 2 + (BAND_H + HUD_H) * 2));
    // A pixel of the overlay is a 2 by 2 block there.
    let (px, py, _, _) = PANEL;
    let rim = one.get(px + 8, py + BAND_H);
    assert_eq!(two.get((px + 8) * 2 + 1, (py + BAND_H) * 2 + 1), rim);
}

#[test]
fn every_icon_is_a_five_pixel_mark_on_the_palette_and_each_one_differs() {
    use std::collections::BTreeSet;
    let icons = [
        Icon::Guardian,
        Icon::Chief,
        Icon::Stairs,
        Icon::Sanctuary,
        Icon::Entrance,
        Icon::Treasure,
        Icon::Merchant,
        Icon::Key,
        Icon::Vault,
        Icon::Pit,
        Icon::Wanted,
        Icon::Ledge,
        Icon::Nook,
        Icon::Forge,
        Icon::Hold,
    ];
    let mut seen = BTreeSet::new();
    for icon in icons {
        let g = glyph(icon);
        assert_eq!((g.w, g.h), (5, 5), "{icon:?}");
        assert!(on_palette(&g));
        assert!(
            g.words().iter().filter(|w| *w >> 24 != 0).count() >= 8,
            "{icon:?} has something to see"
        );
        assert!(
            seen.insert(g.words().to_vec()),
            "{icon:?} looks like another"
        );
        assert!(
            !name_of(icon).is_empty() && name_of(icon).len() <= 9,
            "{icon:?}'s name fits the legend"
        );
    }
}

#[test]
fn a_legend_names_only_what_is_on_the_floor() {
    let mut run = on_floor(5);
    let before = overlay_of(&run.floor_map(Some(1))).unwrap();
    for room in &mut run.dungeon.rooms {
        room.visited = true;
    }
    let after = overlay_of(&run.floor_map(Some(1))).unwrap();
    // The legend's rows grow as the floor is known: more inked in the
    // sidebar.
    let (sx, sy, sw, sh) = SIDEBAR;
    let inked = |im: &Img| {
        (sy..sy + sh)
            .flat_map(|y| (sx..sx + sw).map(move |x| (x, y)))
            .filter(|&(x, y)| im.get(x, y) == Some(super::super::super::ink::ink('h').unwrap()))
            .count()
    };
    assert!(inked(&after) > inked(&before));
    // An empty map has no overlay.
    assert!(overlay_of(&FloorMap::default()).is_none());
}

/// A mid-floor party of three on floor two: most of the floor walked but
/// the guardian's room, knight one in the room with most about it, the
/// other two waiting out in rooms beside it.
fn midfloor(seed: u64) -> Run {
    let mut run = on_floor(seed);
    run.join_seat(2, "Wren");
    run.join_seat(3, "Bors");
    let guardian = run.boss_gates.as_ref().unwrap().guardian;
    let secret = run.dungeon.secret.map(|s| s.vault);
    for i in 0..run.dungeon.rooms.len() {
        if i != guardian && Some(i) != secret {
            run.dungeon.rooms[i].visited = true;
            run.dungeon.rooms[i].cleared =
                run.dungeon.rooms[i].kind != RoomKind::Fight || i % 3 != 0;
        }
    }
    // The room with the most walked rooms in the minimap's window.
    let cells: Vec<(i32, i32)> = run.dungeon.rooms.iter().map(|r| r.cell).collect();
    let near = move |a: usize, b: usize| {
        let (ca, cb) = (cells[a], cells[b]);
        (ca.0 - cb.0).abs() <= 2 && (ca.1 - cb.1).abs() <= 1
    };
    let visited: Vec<usize> = (0..run.dungeon.rooms.len())
        .filter(|&i| run.dungeon.rooms[i].visited)
        .collect();
    let here = *visited
        .iter()
        // A room the minimap is drawn in (not a nook, nor the open world).
        .filter(|&&i| {
            let kind = run.dungeon.rooms[i].kind;
            i != 0 && !kind.in_world() && !kind.nook()
        })
        .max_by_key(|&&i| visited.iter().filter(|&&j| near(i, j)).count())
        .unwrap();
    run.enter(here, Some(2));
    run.enemies.clear();
    run.phase = Phase::Exploring;
    run.dungeon.rooms[here].cleared = true;
    let (w, h) = (run.room().width(), run.room().height());
    for (id, (u, v)) in [(1, (0.35, 0.6)), (2, (0.6, 0.35)), (3, (0.5, 0.8))] {
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y) = (w * u, h * v);
    }
    let beside: Vec<usize> = visited
        .iter()
        .copied()
        .filter(|&j| j != here && near(here, j))
        .collect();
    let (a, b) = (beside[0], beside[beside.len() - 1]);
    for (id, at, spot) in [(2u32, a, (0.25, 0.3)), (3, b, (0.7, 0.75))] {
        let (rw, rh) = (
            run.dungeon.rooms[at].width(),
            run.dungeon.rooms[at].height(),
        );
        let spot = (rw * spot.0, rh * spot.1);
        run.exits.outside.insert(
            id,
            crate::rules::exits::Entry {
                at,
                party: BTreeMap::from([(id, spot)]),
            },
        );
        (
            run.players.get_mut(&id).unwrap().x,
            run.players.get_mut(&id).unwrap().y,
        ) = spot;
    }
    run
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_map_shots -- --ignored`
/// (`MAP_SEED=<n>` picks the floor).
#[test]
#[ignore]
fn write_map_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let seed: u64 = std::env::var("MAP_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3);
    let run = midfloor(seed);
    let seat = run.view_for_seat(1).unwrap_or_else(|| run.clone());
    let (sw, sh) = (NATIVE_W, NATIVE_H + BAND_H + HUD_H);
    let full = run.floor_map(Some(1));
    let mini = seat_scene_with(&seat, sw, sh, Some(1), &MapView::default()).compose();
    write_png(&mini, 3, &dir.join("minimap-midfloor.png"));
    let big = seat_scene_with(
        &seat,
        sw,
        sh,
        Some(1),
        &MapView {
            full: true,
            model: Some(full),
            ..Default::default()
        },
    )
    .compose();
    write_png(&big, 3, &dir.join("full-map.png"));
    // The minimap alone, and a sheet of the marks.
    if let Some(panel) = minimap_image(&seat, Some(1)) {
        write_png(&panel, 8, &dir.join("minimap-alone.png"));
    }
    let mut sheet = Img::new(12 * 8 + 4, 24);
    for (i, icon) in [
        Icon::Guardian,
        Icon::Chief,
        Icon::Stairs,
        Icon::Sanctuary,
        Icon::Entrance,
        Icon::Treasure,
        Icon::Merchant,
        Icon::Key,
        Icon::Vault,
        Icon::Pit,
        Icon::Wanted,
        Icon::Ledge,
        Icon::Nook,
        Icon::Forge,
        Icon::Hold,
    ]
    .into_iter()
    .enumerate()
    {
        sheet.rect(2 + (i as i32 % 8) * 12, 2 + (i as i32 / 8) * 10, 6, 6, 'K');
        sheet.stamp(
            &glyph(icon),
            2 + (i as i32 % 8) * 12,
            2 + (i as i32 / 8) * 10,
        );
    }
    write_png(&sheet, 8, &dir.join("marks.png"));
}

