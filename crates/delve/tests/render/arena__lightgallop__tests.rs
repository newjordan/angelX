//! Lightgallop through the real arena: the road, the riders and the
//! heads-up display on the native route and the terminal's, and the pictures
//! the reviewer opens. `ANGEL_ARENA_SHOTS=<dir> cargo test -p angel-delve
//! write_lightgallop_review -- --ignored` writes them.
use super::*;
use crate::rules::{MAX_PLAYERS, Run};
use std::collections::BTreeMap;

fn party(seats: u32) -> Run {
    let mut r = Run::lightgallop_for_test();
    for id in 2..=seats {
        r.join_seat(id, &format!("Knight {id}"));
    }
    r.step(&BTreeMap::new());
    r
}

#[test]
fn the_road_draws_on_both_routes_and_at_every_seat_count() {
    for seats in [1, 2, 4, MAX_PLAYERS] {
        let r = party(seats);
        let (base, fx) = layers(&r, Some(1));
        assert_eq!((base.w, base.h, fx.w, fx.h), (384, 224, 384, 224));
        let (cells, _) = terminal_layers(&r, Some(1));
        assert_eq!(cells.content_hash(), base.content_hash(), "the cells draw the same picture");
        assert!(base.pixels().filter(|p| *p != [0, 0, 0]).count() > 40_000, "a full picture");
    }
}

use super::super::{art_v2, frame_for};
use crate::rules::lightgallop::{bot::Route, rider::Numbers};

/// Step the table with bots until `done`, at most `limit` ticks.
fn ride_until(r: &mut Run, route: Route, limit: u32, done: impl Fn(&Run) -> bool) {
    for _ in 0..limit {
        if done(r) {
            return;
        }
        let s = r.lightgallop.as_ref().unwrap();
        let n = Numbers::default();
        let inputs: BTreeMap<u32, crate::rules::Input> =
            s.seats.keys().map(|&id| (id, s.drive(id, route, &n))).collect();
        r.step(&inputs);
    }
}

fn x_of(r: &Run, id: u32) -> i32 {
    r.lightgallop.as_ref().unwrap().seats[&id].rider.x / 256
}

// ---- the words ----------------------------------------------------------------

/// The frame, as a rectangle.
const FRAME: (i32, i32, i32, i32) = (0, 0, 384, 224);

fn inside(r: (i32, i32, i32, i32), frame: (i32, i32, i32, i32)) -> bool {
    r.0 >= frame.0 && r.1 >= frame.1 && r.0 + r.2 <= frame.0 + frame.2 && r.1 + r.3 <= frame.1 + frame.3
}

/// A table of `seats` bots ridden for `ticks` ticks.
fn ridden(seats: u32, ticks: u32) -> Run {
    let mut r = party(seats);
    ride_until(&mut r, Route::Upper, ticks, |_| false);
    r
}

/// Put `id` on the road at column `x` at pace `gsp`, its camera on it, and the
/// Dimming far behind.
fn put_on_road(r: &mut Run, id: u32, x: i32, gsp: i32) {
    let lvl = crate::rules::lightgallop::level::level();
    let s = r.lightgallop.as_mut().unwrap();
    let rider = &mut s.seats.get_mut(&id).unwrap().rider;
    let at = lvl.terrain.paths[lvl.main].s_at_x(x);
    *rider = crate::rules::lightgallop::rider::Rider::new_on(&lvl.terrain, lvl.main, at);
    rider.gsp = gsp;
    let y = rider.y / 256;
    rider.snap_camera(x, y);
    s.dim = -4000 * 256;
}

/// The pack's tally, `ticks` ticks after the bell.
fn tallied(seats: u32, ticks: u32) -> Run {
    let mut r = party(seats);
    ride_until(&mut r, Route::Upper, 6000, |r| r.lightgallop.as_ref().unwrap().tally.is_some());
    let at = r.lightgallop.as_ref().unwrap().tally.as_ref().expect("the bell rang").at;
    while r.lightgallop.as_ref().unwrap().elapsed < at + ticks {
        r.step(&BTreeMap::new());
    }
    r
}

/// Where the viewer's picture puts seat `id`'s body, as a box.
fn body_box(r: &Run, viewer: u32, id: u32) -> (i32, i32, i32, i32) {
    let s = r.lightgallop.as_ref().unwrap();
    let me = &s.seats[&viewer].rider;
    let v = View { cx: me.cam_x, cy: me.cam_y, zoom: me.zoom, w: 384, h: 224 };
    let c = s.seats[&id].rider.centre(&crate::rules::lightgallop::level::level().terrain);
    let (sx, sy) = (v.qx(c.0), v.qy(c.1));
    (sx - 24, sy - 24, 48, 40)
}

/// Every pair of words in `words` that touch, as a message.
fn collisions(words: &[Word]) -> Vec<String> {
    let mut out = Vec::new();
    for (i, a) in words.iter().enumerate() {
        for b in &words[i + 1..] {
            if meet(a.rect(), b.rect()) {
                out.push(format!("{:?} {:?} runs into {:?} {:?}", a.text, a.rect(), b.text, b.rect()));
            }
        }
    }
    out
}

/// The moments the words are checked at, for a table of `seats`: the title
/// card, the keys, a long ride, a fall, the bell and the tally ticking up.
fn word_moments(seats: u32) -> Vec<(String, Run)> {
    let mut out = vec![];
    for ticks in [1, 12, 40, 70, 400] {
        out.push((format!("{seats} seats, {ticks} ticks"), ridden(seats, ticks)));
    }
    let mut fall = ridden(seats, 200);
    fall.lightgallop.as_mut().unwrap().seats.get_mut(&1).unwrap().rider.down = 12;
    out.push((format!("{seats} seats, a fall"), fall));
    let mut bell = party(seats);
    ride_until(&mut bell, Route::Upper, 6000, |r| r.lightgallop.as_ref().unwrap().bell_t.is_some());
    out.push((format!("{seats} seats, the bell"), bell));
    for ticks in [0, 20, 60, 80, 140, 300, 419] {
        out.push((format!("{seats} seats, the tally at {ticks}"), tallied(seats, ticks)));
    }
    out
}

#[test]
fn every_word_stands_inside_the_frame_at_every_seat_count() {
    for seats in [1, 2, 4, MAX_PLAYERS] {
        for (what, r) in word_moments(seats) {
            for w in said(&r, Some(1)) {
                assert!(inside(w.rect(), FRAME), "{what}: {:?} at {:?} is cut by the frame", w.text, w.rect());
            }
        }
    }
}

#[test]
fn a_riders_tag_is_never_cut_by_the_frame_wherever_the_rider_is() {
    for d in (-330..=330).step_by(7) {
        let mut r = party(2);
        put_on_road(&mut r, 1, 2000, 1300);
        put_on_road(&mut r, 2, 2000 + d, 1300);
        r.lightgallop.as_mut().unwrap().elapsed = 600;
        for w in said(&r, Some(1)) {
            assert!(inside(w.rect(), FRAME), "a rider {d} px from the viewer: {:?} at {:?} is cut", w.text, w.rect());
        }
    }
}

#[test]
fn no_two_words_run_into_each_other_at_any_seat_count() {
    for seats in [1, 2, 4, MAX_PLAYERS] {
        for (what, r) in word_moments(seats) {
            let said = said(&r, Some(1));
            let bad = collisions(&said);
            assert!(bad.is_empty(), "{what}:\n{}", bad.join("\n"));
        }
    }
}

#[test]
fn a_pack_on_one_spot_keeps_its_tags_apart_and_clear_of_the_prompts() {
    for seats in [2, 3, 4, 5, MAX_PLAYERS] {
        // Everyone at the gate, in the first seconds (the title card up), one of
        // them fallen and the bell already rung: every prompt at once.
        let mut r = party(seats);
        for id in 1..=seats {
            put_on_road(&mut r, id, 300 + id as i32, 0);
        }
        {
            let s = r.lightgallop.as_mut().unwrap();
            s.elapsed = 10;
            s.bell_t = Some(5);
            s.seats.get_mut(&1).unwrap().rider.down = 12;
        }
        let said = said(&r, Some(1));
        let bad = collisions(&said);
        assert!(bad.is_empty(), "{seats} seats on one spot:\n{}", bad.join("\n"));
        for w in &said {
            assert!(inside(w.rect(), FRAME), "{seats} seats: {:?} at {:?} is cut", w.text, w.rect());
        }
    }
}

#[test]
fn the_title_and_the_prompts_stand_clear_of_every_rider() {
    let prompts = ["MOONWELL ROAD", "THE DIMMING COMES", "THE LANTERN CARRIES YOU", "RING THE BELL"];
    for seats in [1, 2, 4, MAX_PLAYERS] {
        let mut r = party(seats);
        for id in 1..=seats {
            put_on_road(&mut r, id, 300 + 14 * id as i32, 600);
        }
        {
            let s = r.lightgallop.as_mut().unwrap();
            s.elapsed = 10;
            s.bell_t = Some(5);
            s.seats.get_mut(&1).unwrap().rider.down = 12;
        }
        let words = said(&r, Some(1));
        for w in words.iter().filter(|w| prompts.contains(&w.text.as_str())) {
            for id in 1..=seats {
                let body = body_box(&r, 1, id);
                assert!(!meet(w.rect(), body), "{seats} seats: {:?} at {:?} sits on seat {id}'s rider {body:?}", w.text, w.rect());
            }
        }
        assert!(words.iter().filter(|w| prompts.contains(&w.text.as_str())).count() == 4, "every prompt was up");
    }
}

#[test]
fn the_tallys_words_stand_on_its_panel_and_nothing_falls_on_them() {
    use crate::render::overworld::ink::ink;
    let (conf_a, bg) = (ink('5').unwrap(), ink('k').unwrap());
    // The panel's inner frame is at (58, 38) and 268 by 164: the words stay inside it.
    let panel = (59, 39, 266, 162);
    for seats in [1, 2, 4, MAX_PLAYERS] {
        let mut r = tallied(seats, 0);
        let at = r.lightgallop.as_ref().unwrap().tally.as_ref().unwrap().at;
        let mut seen_confetti = false;
        for t in [0, 9, 30, 45, 70, 90, 111, 140, 200, 301, 419] {
            while r.lightgallop.as_ref().unwrap().elapsed < at + t {
                r.step(&BTreeMap::new());
            }
            let words = said(&r, Some(1));
            let from = words.iter().position(|w| w.text == "MOONWELL ROAD CLEAR").expect("the tally is up");
            let (base, _) = layers(&r, Some(1));
            for w in &words[from..] {
                let rect = w.rect();
                assert!(inside(rect, panel), "{seats} seats, tally {t}: {:?} at {rect:?} leaves the panel", w.text);
                let want = ink(w.ink).unwrap();
                for y in rect.1..rect.1 + rect.3 {
                    for x in rect.0..rect.0 + rect.2 {
                        let p = base.get(x, y).unwrap();
                        assert!(
                            p == want || p == bg,
                            "{seats} seats, tally {t}: {:?} has a stray pixel {p:?} at {x},{y}",
                            w.text
                        );
                    }
                }
            }
            // The shower still falls on the panel's free ground (the rank stamp
            // is the only other thing drawn there in this colour).
            if t == 30 {
                let on_panel = (39..201).flat_map(|y| (59..325).map(move |x| (x, y))).filter(|&(x, y)| base.get(x, y).unwrap() == conf_a);
                seen_confetti = on_panel.count() > 0;
            }
        }
        assert!(seen_confetti, "{seats} seats: the shower of sparks still falls over the tally");
    }
}

#[test]
fn the_tally_says_where_the_lantern_hundred_comes_from() {
    for seats in [1, 4] {
        let r = tallied(seats, 140);
        let y = r.lightgallop.as_ref().unwrap().tally.clone().unwrap();
        assert_eq!(y.posts, 6, "the bots light every lantern");
        let words = said(&r, Some(1));
        let row = |name: &str| words.iter().find(|w| w.text == name).unwrap_or_else(|| panic!("{name}: {words:?}"));
        let bonus = row("LANTERN BONUS");
        let plus = words.iter().find(|w| w.text == "+100" && w.y == bonus.y).expect("the +100 on the bonus row");
        assert!(plus.x > bonus.x + text_width(&bonus.text), "the number stands right of its name");
        // It sits with the other rows, above the total it is part of.
        assert!(row("LANTERNS").y < bonus.y && bonus.y < row("POINTS").y);
        // The per-knight line is labelled, so it is not read as the party's total.
        assert!(row("EACH KNIGHT").y > row("POINTS").y);
    }
    // Fewer lanterns lit, a fraction of the hundred.
    let mut r = tallied(1, 140);
    r.lightgallop.as_mut().unwrap().tally.as_mut().unwrap().posts = 3;
    assert!(said(&r, Some(1)).iter().any(|w| w.text == "+50"), "three of six lanterns lit is +50");
}

#[test]
fn the_best_riders_score_wears_a_crown_the_font_cannot_print() {
    use crate::render::overworld::ink::ink;
    for seats in [1, 4, MAX_PLAYERS] {
        let r = tallied(seats, 140);
        let y = r.lightgallop.as_ref().unwrap().tally.clone().unwrap();
        let words = said(&r, Some(1));
        let from = words.iter().position(|w| w.text == "EACH KNIGHT").unwrap();
        let (base, _) = layers(&r, Some(1));
        let mut crowned = Vec::new();
        for w in &words[from + 1..] {
            let digits = w.text.trim_start_matches(['*', ' ']);
            let Some(id) = digits.split(' ').next().and_then(|d| d.parse::<u32>().ok()) else { continue };
            // What the font draws for these letters, and the pixels in the picture beyond it.
            let mut font = Img::new(text_width(&w.text), 7);
            font.text(0, 0, &w.text, 'T');
            let want = ink(w.ink).unwrap();
            let extra = (0..7)
                .flat_map(|dy| (0..font.w).map(move |dx| (dx, dy)))
                .filter(|&(dx, dy)| font.get(dx, dy).is_none() && base.get(w.x + dx, w.y + dy) == Some(want))
                .count();
            if extra > 0 {
                crowned.push((id, extra));
            }
        }
        assert_eq!(crowned.len(), 1, "{seats} seats: one score wears the crown: {crowned:?}");
        assert_eq!(crowned[0].0, y.best, "{seats} seats: the best rider wears it");
        assert!(crowned[0].1 >= 10, "a crown, not a speck: {crowned:?}");
    }
}

#[test]
fn the_top_bars_words_keep_a_gap_between_them() {
    use crate::render::overworld::ink::ink;
    let bar = ink('k').unwrap();
    for chain in [2, 9, 10, 99] {
        let mut r = party(1);
        ride_until(&mut r, Route::Upper, 30, |_| false);
        r.lightgallop.as_mut().unwrap().seats.get_mut(&1).unwrap().rider.chain = chain;
        let words = said(&r, Some(1));
        let find = |t: &str| words.iter().find(|w| w.text.starts_with(t)).unwrap_or_else(|| panic!("{t}: {words:?}")).rect();
        let (chain_w, spur, gloam) = (find("CHAIN"), find("SPUR"), find("GLOAM"));
        let gap = spur.0 - (chain_w.0 + chain_w.2);
        assert!(gap >= 8, "CHAIN {chain} and the gauge are {gap} px apart");
        // The lantern pips run up to the word GLOAM.
        let (base, _) = layers(&r, Some(1));
        let last = (0..gloam.0).rev().find(|&x| (3..13).any(|y| base.get(x, y).unwrap() != bar)).unwrap();
        assert!(gloam.0 - last - 1 >= 8, "the lantern pips end {} px from GLOAM", gloam.0 - last - 1);
    }
}

#[test]
fn a_blocks_name_stays_plain_when_the_camera_draws_back() {
    use crate::render::overworld::ink::ink;
    use crate::rules::lightgallop::{
        level::level,
        world::{FoeKind, foe_at},
    };
    let lvl = level();
    // (A painted bramble has no block to carry a name.)
    for (kind, name) in [(FoeKind::Snuffer, "SNUF"), (FoeKind::Floater, "WISP"), (FoeKind::Bramble, "BRAM"), (FoeKind::Warden, "WARDEN")]
        .into_iter()
        .filter(|&(kind, _)| kind != FoeKind::Bramble || !painted("lightgallop.bramble"))
    {
        let spot = lvl.foes.iter().find(|f| f.kind == kind && f.min_seats <= 1).unwrap();
        let mut r = party(1);
        put_on_road(&mut r, 1, 400, 0);
        let s = r.lightgallop.as_mut().unwrap();
        s.elapsed = 600;
        let (fx, fy, _) = foe_at(lvl, spot, s.elapsed);
        // The camera drawn back to three quarters, the block at its middle.
        let rider = &mut s.seats.get_mut(&1).unwrap().rider;
        rider.zoom = 192;
        rider.cam_x = fx - 253;
        rider.cam_y = fy - 147;
        let (_, names) = said_with_names(&r, Some(1));
        let w = names
            .iter()
            .find(|w| w.text == name)
            .unwrap_or_else(|| panic!("{name} is set over its block at three quarters: {names:?}"));
        // Every letter is the font's own, a pixel for a pixel, on its plate.
        let mut font = Img::new(text_width(name), 7);
        font.text(0, 0, name, 'T');
        let (base, _) = layers(&r, Some(1));
        let (plate, want) = (ink('k').unwrap(), ink(w.ink).unwrap());
        for dy in 0..7 {
            for dx in 0..font.w {
                let lit = font.get(dx, dy).is_some();
                assert_eq!(base.get(w.x + dx, w.y + dy), Some(if lit { want } else { plate }), "{name} at {dx},{dy}");
            }
        }
    }
}

// ---- the painted slots ------------------------------------------------------

/// A slot is painted whole or not at all: every state of it is the art or
/// every state is the block.
#[test]
fn a_slot_is_painted_whole_or_not_at_all() {
    let slots: Vec<Slot> = serde_json::from_str(include_str!("../../assets/lightgallop/slots.json")).unwrap();
    for s in slots {
        let anims: Vec<bool> = s.states.keys().map(|state| bank()[&(s.id.clone(), state.clone())].painted).collect();
        assert!(anims.iter().all(|&p| p == anims[0]), "{}: {anims:?}", s.id);
        assert_eq!(painted(&s.id), anims[0], "{}", s.id);
        // Every frame is the slot's size, painted or block.
        for (state, ticks) in &s.states {
            let anim = &bank()[&(s.id.clone(), state.clone())];
            assert_eq!(anim.frames.len(), ticks.len(), "{} {state}", s.id);
            assert!(anim.frames.iter().all(|f| [f.w, f.h] == s.size), "{} {state}", s.id);
        }
    }
}

/// The three land strips are 192 by 96, repeat along the road, and the step
/// across the join is no larger than the steps inside the strip.
#[test]
fn the_painted_land_repeats_without_a_seam() {
    let strips = strips().expect("the three layers are painted");
    for (i, strip) in strips.iter().enumerate() {
        let img = &strip.img;
        assert_eq!((img.w, img.h), (192, 96), "strip {i}");
        let step = |a: i32, b: i32| -> Option<i32> {
            let (mut sum, mut n) = (0, 0);
            for y in 0..img.h {
                match (img.get(a, y), img.get(b, y)) {
                    (None, None) => {}
                    // A silhouette's edge is a full step.
                    (Some(_), None) | (None, Some(_)) => {
                        sum += 255;
                        n += 1;
                    }
                    (Some(p), Some(q)) => {
                        sum += (0..3).map(|c| (i32::from(p[c]) - i32::from(q[c])).abs()).sum::<i32>() / 3;
                        n += 1;
                    }
                }
            }
            (n > 0).then(|| sum * 100 / n)
        };
        let mut inner: Vec<i32> = (0..img.w - 1).filter_map(|x| step(x, x + 1)).collect();
        inner.sort_unstable();
        let p95 = inner[inner.len() * 95 / 100];
        let join = step(img.w - 1, 0).unwrap_or(0);
        assert!(join <= p95, "strip {i}: the join steps {join} (x100), 95% of the columns inside step {p95} or less");
    }
}

/// Wherever the camera is, the land fills the whole picture: no hole above the
/// far strip, none below the strips, none between them.
#[test]
fn the_painted_land_covers_the_whole_picture_at_every_height() {
    for cy in [-200, 0, 300, 520, 700, 1200] {
        for cx in [-40, 0, 191, 192, 3000, 19000] {
            let mut im = Img::new(384, 224);
            backdrop(&mut im, &View { cx, cy, zoom: 256, w: 384, h: 224 });
            let holes = (0..224).flat_map(|y| (0..384).map(move |x| (x, y))).filter(|&(x, y)| im.get(x, y).is_none()).count();
            assert_eq!(holes, 0, "cx {cx} cy {cy}");
        }
    }
}

/// The near strip is the painted one at 60 percent: every pixel it has is a
/// pixel the art has (alpha untouched), none brighter.
#[test]
fn the_near_strip_is_the_painted_one_dimmed() {
    let strips = strips().unwrap();
    let art = frame_of("lightgallop.layer-near", "idle", 0);
    let near = &strips[2].img;
    let mut dimmer = 0;
    for y in 0..art.h {
        for x in 0..art.w {
            match (art.get(x, y), near.get(x, y)) {
                (None, None) => {}
                (Some(a), Some(b)) => {
                    let lum = |c: [u8; 3]| u32::from(c[0]) * 3 + u32::from(c[1]) * 6 + u32::from(c[2]);
                    assert!(lum(b) <= lum(a), "{x},{y}: {a:?} became {b:?}");
                    dimmer += u32::from(b != a);
                }
                (a, b) => panic!("{x},{y}: the dim changed what is there: {a:?} then {b:?}"),
            }
        }
    }
    assert!(dimmer > 1000, "most of the fence and grass is dimmed ({dimmer})");
    // The other two strips are as painted.
    for (strip, id) in [(0, "lightgallop.layer-far"), (1, "lightgallop.layer-mid")] {
        assert!(strips[strip].img == *frame_of(id, "idle", 0), "{id} is not dimmed");
    }
}

/// The painted road stands on the very curve the rules ride: its lip is on the
/// surface row, whichever column, and it is the tile's own column.
#[test]
fn the_painted_road_stands_on_the_curve() {
    let lvl = crate::rules::lightgallop::level::level();
    let road = road().expect("the road is painted");
    let p = &lvl.terrain.paths[lvl.main];
    // The first hill: the slope, the flat, the climb.
    for (cx, cy) in [(60, 520), (500, 560), (1700, 700)] {
        let v = View { cx, cy, zoom: 256, w: 384, h: 224 };
        let mut im = Img::new(384, 224);
        terrain(&mut im, &v);
        let mut checked = 0;
        for sx in 0..v.w {
            let x = v.wx(sx);
            let (lo, hi) = p.span_px();
            if x < lo || x > hi {
                continue;
            }
            let y = v.sy(p.y_at_x(x) / Q);
            if (0..v.h).contains(&y) {
                let u = x.rem_euclid(road.tile.w);
                assert_eq!(im.get(sx, y), road.tile.get(u, ROAD_LIP), "column {sx} of view {cx},{cy}: the lip is on the surface");
                assert!(road.tile.get(u, ROAD_LIP).is_some());
                checked += 1;
            }
        }
        assert!(checked > 100, "view {cx},{cy}: {checked} columns checked");
    }
}

#[test]
#[ignore = "writes Lightgallop's review pictures"]
fn write_lightgallop_review() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.rgba_bytes())
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    save("blocks", &block_sheet(3));
    // The whole road, an act to a strip.
    let r = party(1);
    for (k, from) in [-40, 4260, 11260, 15660].into_iter().enumerate() {
        save(&format!("road-act{}", k + 1), &overview(&r, from, 1760, 128));
    }
    // Moments on the road, a solo bot's ride.
    let moments: [(&str, i32, Route); 14] = [
        ("start", 120, Route::Upper),
        ("downhill", 640, Route::Upper),
        ("trench", 1880, Route::Upper),
        ("loop-run-in", 4640, Route::Upper),
        ("halfpipe", 5600, Route::Upper),
        ("rolling-crest", 6330, Route::Upper),
        ("coil", 7160, Route::Upper),
        ("fork-upper", 9100, Route::Upper),
        ("fork-lower", 9100, Route::Lower),
        ("fen-chain", 11700, Route::Upper),
        ("spark-bridge", 12900, Route::Upper),
        ("loop-two", 16800, Route::Upper),
        ("warden", 18900, Route::Upper),
        ("bell", 19520, Route::Upper),
    ];
    for (name, x, route) in moments {
        let mut r = party(1);
        ride_until(&mut r, route, 4000, |r| x_of(r, 1) >= x);
        save(&format!("{name}-1x"), &frame_for(&r, 384, 224, Some(1)));
        if matches!(name, "loop-run-in" | "fen-chain" | "warden") {
            save(&format!("{name}-3x"), &frame_for(&r, 1152, 672, Some(1)));
        }
    }
    // The loop, frame by frame: the lean through a full turn, 3x crops
    // round the rider, ridden upright (no roll) at a gallop.
    let mut r = party(1);
    {
        let lvl = crate::rules::lightgallop::level::level();
        let s = r.lightgallop.as_mut().unwrap();
        let rider = &mut s.seats.get_mut(&1).unwrap().rider;
        let at = lvl.terrain.paths[lvl.main].s_at_x(4600);
        *rider = crate::rules::lightgallop::rider::Rider::new_on(&lvl.terrain, lvl.main, at);
        rider.gsp = 1700;
        rider.snap_camera(4600, 770);
        s.dim = -800 * 256;
    }
    let mut sheet = Img::black(4 * 288, 3 * 288);
    let mut shots = 0;
    for tick in 0..400 {
        let s = r.lightgallop.as_ref().unwrap();
        let rider = &s.seats[&1].rider;
        if rider.looping && tick % 3 == 0 && shots < 12 {
            let f = frame_for(&r, 1152, 672, Some(1));
            let cen = (rider.x / 256 - rider.cam_x, rider.y / 256 - rider.cam_y);
            let (cx, cy) = (cen.0 * 3 * rider.zoom / 256, (cen.1 - 14) * 3 * rider.zoom / 256);
            let crop = Img::from_view(&f, cx - 144, cy - 144, 288, 288);
            sheet.stamp(&crop, (shots % 4) * 288, (shots / 4) * 288);
            shots += 1;
        }
        let inputs = BTreeMap::from([(1, crate::rules::Input { move_x: 1, ..Default::default() })]);
        let _ = tick;
        r.step(&inputs);
    }
    save("loop-turn", &sheet);
    // The first ten seconds, a frame a second and a half, half size.
    {
        let mut r = party(1);
        let mut strip = Img::black(3 * 192, 4 * 112);
        for n in 0..12 {
            let f = frame_for(&r, 384, 224, Some(1));
            let mut small = Img::new(192, 112);
            for y in 0..112 {
                for x in 0..192 {
                    if let Some(c) = f.get(x * 2, y * 2) {
                        small.set(x, y, c);
                    }
                }
            }
            strip.stamp(&small, (n % 3) * 192, (n / 3) * 112);
            for _ in 0..25 {
                let s = r.lightgallop.as_ref().unwrap();
                let nn = Numbers::default();
                let inputs: BTreeMap<u32, crate::rules::Input> =
                    s.seats.keys().map(|&id| (id, s.drive_as(id, Route::Upper, &nn, crate::rules::lightgallop::bot::Skill::FIRST_RIDE))).collect();
                r.step(&inputs);
            }
        }
        save("first-ten-seconds", &strip);
        let mut r = party(1);
        for t in 0..80 {
            let s = r.lightgallop.as_ref().unwrap();
            let nn = Numbers::default();
            let inputs: BTreeMap<u32, crate::rules::Input> =
                s.seats.keys().map(|&id| (id, s.drive_as(id, Route::Upper, &nn, crate::rules::lightgallop::bot::Skill::FIRST_RIDE))).collect();
            r.step(&inputs);
            if t == 24 || t == 50 {
                let k = &r.lightgallop.as_ref().unwrap().seats[&1].rider;
                eprintln!("t{t}: x {} y {} cam {} {} zoom {} hurt {}", k.x / 256, k.y / 256, k.cam_x, k.cam_y, k.zoom, k.hurt);
                save(&format!("ten-seconds-{t}"), &frame_for(&r, 384, 224, Some(1)));
            }
        }
    }
    // Moves, mid-flight: the spur, a hit, a fall, a lock, the warden, a wake.
    let lvl = crate::rules::lightgallop::level::level();
    let place = |r: &mut Run, id: u32, x: i32, gsp: i32| {
        let s = r.lightgallop.as_mut().unwrap();
        let rider = &mut s.seats.get_mut(&id).unwrap().rider;
        let at = lvl.terrain.paths[lvl.main].s_at_x(x);
        *rider = crate::rules::lightgallop::rider::Rider::new_on(&lvl.terrain, lvl.main, at);
        rider.gsp = gsp;
        rider.snap_camera(x, 700);
        s.dim = -4000 * 256;
    };
    let keys = |x: i8, spur: bool, down: bool, jump: bool| crate::rules::Input {
        move_x: x,
        dash: spur,
        move_y: i8::from(down),
        interact: jump,
        ..Default::default()
    };
    {
        let mut r = party(1);
        place(&mut r, 1, 2640, 1300);
        for _ in 0..26 {
            r.step(&BTreeMap::from([(1, keys(1, true, false, false))]));
        }
        save("move-spur-3x", &frame_for(&r, 1152, 672, Some(1)));
    }
    for (name, sparks) in [("move-scatter", 14), ("move-down", 0)] {
        let mut r = party(1);
        place(&mut r, 1, 470, 900);
        r.lightgallop.as_mut().unwrap().seats.get_mut(&1).unwrap().rider.sparks = sparks;
        for _ in 0..80 {
            if r.lightgallop.as_ref().unwrap().seats[&1].rider.hits > 0 {
                break;
            }
            r.step(&BTreeMap::from([(1, keys(1, false, false, false))]));
        }
        for _ in 0..9 {
            r.step(&BTreeMap::from([(1, keys(1, false, false, false))]));
        }
        save(&format!("{name}-3x"), &frame_for(&r, 1152, 672, Some(1)));
    }
    {
        let mut r = party(1);
        ride_until(&mut r, Route::Upper, 4000, |r| x_of(r, 1) >= 11590);
        let mut got = false;
        for _ in 0..300 {
            let s = r.lightgallop.as_ref().unwrap();
            if s.seats[&1].rider.lock.is_some() && !got {
                save("move-lock-3x", &frame_for(&r, 1152, 672, Some(1)));
                got = true;
            }
            let n = Numbers::default();
            let inputs: BTreeMap<u32, crate::rules::Input> =
                s.seats.keys().map(|&id| (id, s.drive(id, Route::Upper, &n))).collect();
            r.step(&inputs);
        }
        assert!(got, "a lock was in flight");
    }
    {
        let mut r = party(1);
        let w = lvl.foes.iter().find(|f| f.kind == crate::rules::lightgallop::world::FoeKind::Warden).unwrap().x;
        place(&mut r, 1, w - 260, 1400);
        r.lightgallop.as_mut().unwrap().posts = 0b11111;
        let mut got = false;
        for _ in 0..200 {
            let s = r.lightgallop.as_ref().unwrap();
            let n = Numbers::default();
            let inputs: BTreeMap<u32, crate::rules::Input> =
                s.seats.keys().map(|&id| (id, s.drive(id, Route::Upper, &n))).collect();
            r.step(&inputs);
            if !got && r.lightgallop.as_ref().unwrap().warden_inv > 15 {
                save("move-warden-3x", &frame_for(&r, 1152, 672, Some(1)));
                got = true;
            }
        }
        assert!(got, "the warden was struck");
    }
    {
        let mut r = party(1);
        let w = lvl.foes.iter().position(|f| f.kind == crate::rules::lightgallop::world::FoeKind::Warden).unwrap();
        place(&mut r, 1, lvl.foes[w].x - 150, 600);
        let s = r.lightgallop.as_mut().unwrap();
        s.foes[w].hp = 0;
        s.foes[w].pop = 24;
        s.elapsed = 900;
        r.step(&BTreeMap::new());
        save("move-warden-fall-3x", &frame_for(&r, 1152, 672, Some(1)));
    }
    {
        let mut r = party(3);
        for (id, x) in [(1, 640), (2, 610), (3, 575)] {
            place(&mut r, id, x, 1280);
        }
        for _ in 0..40 {
            let inputs = (1..=3).map(|id| (id, keys(1, false, false, false))).collect();
            r.step(&inputs);
        }
        save("move-wake-3x", &frame_for(&r, 1152, 672, Some(3)));
    }
    // The tally, and a full table.
    let mut r = party(1);
    ride_until(&mut r, Route::Upper, 6000, |r| r.lightgallop.as_ref().unwrap().tally.is_some());
    for _ in 0..140 {
        r.step(&BTreeMap::new());
    }
    save("tally-1x", &frame_for(&r, 384, 224, Some(1)));
    save("tally-3x", &frame_for(&r, 1152, 672, Some(1)));
    for seats in [2, 4, MAX_PLAYERS] {
        let mut r = party(seats);
        ride_until(&mut r, Route::Upper, 900, |_| false);
        save(&format!("seats{seats}-1x"), &frame_for(&r, 384, 224, Some(2.min(seats))));
        let (base, _) = art_v2::terminal_layers(&r, Some(1)).unwrap();
        save(&format!("seats{seats}-terminal-layers"), &base);
    }
    // The words: the title card and the keys, a fall, the bell, and the tally
    // as it ticks up, at every seat count (the viewer is the first seat and the
    // last, since the other knights carry number tags).
    for seats in [1, 2, 4, MAX_PLAYERS] {
        for (name, ticks) in [("title", 12), ("title-late", 60), ("hud", 400)] {
            let mut r = party(seats);
            ride_until(&mut r, Route::Upper, ticks, |_| false);
            save(&format!("words-{name}-{seats}seats-1x"), &frame_for(&r, 384, 224, Some(1)));
            if name == "title" {
                save(&format!("words-{name}-{seats}seats-3x"), &frame_for(&r, 1152, 672, Some(1)));
                save(&format!("words-{name}-{seats}seats-last"), &frame_for(&r, 384, 224, Some(seats)));
            }
        }
        let mut r = party(seats);
        ride_until(&mut r, Route::Upper, 200, |_| false);
        r.lightgallop.as_mut().unwrap().seats.get_mut(&1).unwrap().rider.down = 12;
        save(&format!("words-fall-{seats}seats-1x"), &frame_for(&r, 384, 224, Some(1)));
        let mut r = party(seats);
        ride_until(&mut r, Route::Upper, 6000, |r| r.lightgallop.as_ref().unwrap().bell_t.is_some());
        save(&format!("words-bell-{seats}seats-1x"), &frame_for(&r, 384, 224, Some(1)));
        ride_until(&mut r, Route::Upper, 6000, |r| r.lightgallop.as_ref().unwrap().tally.is_some());
        for (name, ticks) in [("a", 20), ("b", 60), ("c", 80), ("d", 140), ("e", 300)] {
            let at = r.lightgallop.as_ref().unwrap().tally.as_ref().unwrap().at;
            let to = at + ticks;
            while r.lightgallop.as_ref().unwrap().elapsed < to {
                r.step(&BTreeMap::new());
            }
            save(&format!("words-tally-{name}-{seats}seats-1x"), &frame_for(&r, 384, 224, Some(1)));
            if name == "d" {
                save(&format!("words-tally-{name}-{seats}seats-3x"), &frame_for(&r, 1152, 672, Some(1)));
            }
        }
    }
}

/// The painted props close up, for the reviewer: `ANGEL_ARENA_SHOTS=<dir>
/// cargo test -p angel-delve --lib write_lightgallop_art_review -- --ignored`.
/// Each picture is the whole 384 by 224 frame with the prop in the middle and
/// a rider near it; `art-*` names.
#[test]
#[ignore = "writes Lightgallop's art review pictures"]
fn write_lightgallop_art_review() {
    use crate::rules::lightgallop::world::{CrateKind, FoeKind};
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.rgba_bytes())
            .unwrap()
            .save(dir.join(format!("art-{name}.png")))
            .unwrap();
    };
    let lvl = crate::rules::lightgallop::level::level();
    // A rider `back` pixels before column `x`, everything stopped, the camera
    // on the prop: its foot (at height `foot`) at column 192 and row 150.
    let at_foot = |x: i32, foot: i32, back: i32| {
        let mut r = party(1);
        put_on_road(&mut r, 1, x - back, 0);
        let s = r.lightgallop.as_mut().unwrap();
        s.elapsed = 600;
        s.seats.get_mut(&1).unwrap().rider.snap_camera(x - 40, foot - 16);
        r
    };
    let at = |x: i32, back: i32| at_foot(x, lvl.terrain.paths[lvl.main].y_at_x(x) / Q, back);
    let shot = |r: &Run| frame_for(r, 384, 224, Some(1));
    // The hay: at rest, then the ticks it is sprung.
    let hay = lvl.springs.iter().position(|sp| sp.kind == crate::rules::lightgallop::world::SpringKind::Up).unwrap();
    for age in [None, Some(0), Some(3), Some(6)] {
        let mut r = at(lvl.springs[hay].x, 90);
        let s = r.lightgallop.as_mut().unwrap();
        if let Some(age) = age {
            s.spring_t[hay] = s.elapsed + 1 - age;
        }
        save(&format!("hay-{}", age.map_or("rest".to_owned(), |a| format!("sprung{a}"))), &shot(&r));
    }
    // The post: dark, then lit and catching (the ring closing in), then lit.
    for (name, lit, spin) in [("dark", false, 0), ("catch0", true, 0), ("catch15", true, 15), ("lit", true, 60)] {
        let mut r = at(lvl.posts[0].x, 90);
        let s = r.lightgallop.as_mut().unwrap();
        if lit {
            s.posts |= 1;
            s.post_t[0] = s.elapsed + 1 - spin;
        }
        save(&format!("post-{name}"), &shot(&r));
    }
    // The crates, one of each kind.
    for kind in [CrateKind::Shield, CrateKind::Sparks, CrateKind::Spur] {
        let c = lvl.crates.iter().find(|c| c.kind == kind).unwrap();
        save(&format!("crate-{kind:?}").to_lowercase(), &shot(&at_foot(c.x, c.y, 90)));
    }
    // The bramble.
    let b = lvl.foes.iter().find(|f| f.kind == FoeKind::Bramble && f.min_seats == 0).unwrap();
    save("bramble", &shot(&at(b.x, 90)));
    // The bell: at rest, and each beat of its ring.
    for (name, rung, tick) in [("rest", false, 600), ("ring0", true, 600), ("ring1", true, 604), ("ring2", true, 608), ("ring3", true, 612), ("ring4", true, 616), ("ring5", true, 620)] {
        let mut r = at(lvl.bell.0, 110);
        let s = r.lightgallop.as_mut().unwrap();
        s.elapsed = tick;
        if rung {
            s.bell_t = Some(tick);
        }
        save(&format!("bell-{name}"), &shot(&r));
    }
}
