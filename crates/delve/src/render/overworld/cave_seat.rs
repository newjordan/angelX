//! A seat's view inside the living cave: the ground under the lamps, rock
//! breaking, the knights, the lamp that leads out, and the cave's own corner
//! of the screen (stone carried, magic left). Also the cave's mouth as it
//! stands in a room, and the warm-up that makes the first frame inside cheap.
//!
//! Everything here is drawn from the host's snapshot, so a guest's screen
//! shows what the host's does.

use std::cell::{Cell, RefCell};
use std::hash::{Hash, Hasher};

use super::cave_fx;
use super::cave_paint::{self, Lamp};
use super::kit::hero::{self, Dress, Held, View};
use crate::ink::{self, Img};
use crate::rules::caves::world::{
    region_of, Cave, Footwear, BLAST_COST, CARRY_LIMIT, CELL, FLUID_ACID, MAGIC_MAX, NATIVE_H, NATIVE_W,
};
use crate::rules::caves::{self, Seat};
use crate::rules::Run;

/// Where a seat's knight stands this tick, in world pixels: its cell, less
/// what is left of the step it is taking.
fn feet(seat: &Seat) -> (i32, i32) {
    let (px, py) = cave_paint::cell_px(i32::from(seat.x), i32::from(seat.y));
    let (sx, sy) = seat.stride();
    (px + sx, py + sy)
}

/// A knight's lamp hangs a little below the middle of its cell.
fn lamp_of(seat: &Seat) -> Lamp {
    let (x, y) = feet(seat);
    Lamp { px: (x, y + 2), cell: (i32::from(seat.x), i32::from(seat.y)), small: false }
}

/// The view a seat is shown, at `w` by `h`. A frame asked for twice in one
/// tick is drawn once.
pub fn cave_seat_frame(run: &Run, focus: u32, w: i32, h: i32) -> Img {
    thread_local! {
        static LAST: RefCell<Option<(u64, Img)>> = const { RefCell::new(None) };
    }
    let key = frame_key(run, focus, w, h);
    if let Some(hit) = LAST.with(|slot| slot.borrow().as_ref().and_then(|(k, im)| (*k == key).then(|| im.copy()))) {
        return hit;
    }
    let im = fit(paint_seat(run, focus), w, h);
    LAST.with(|slot| *slot.borrow_mut() = Some((key, im.copy())));
    im
}

/// Everything a cave frame is drawn from, folded to one word.
fn frame_key(run: &Run, focus: u32, w: i32, h: i32) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (run.tick, focus, w, h, run.caves.seed, run.caves.fingerprint()).hash(&mut hasher);
    for (id, seat) in &run.caves.seats {
        (id, seat.x, seat.y, seat.face, seat.cool, seat.magic, seat.stone).hash(&mut hasher);
    }
    for (id, hero) in &run.players {
        (id, hero.class.as_deref(), hero.knight.as_deref(), hero.deck.len(), hero.set_aside.len()).hash(&mut hasher);
    }
    run.caves.bursts.len().hash(&mut hasher);
    run.notices.hash(&mut hasher);
    run.seat_notices.hash(&mut hasher);
    hasher.finish()
}

/// The native picture in a `w` by `h` frame, as a room's is: whole pixels,
/// as many times over as fit, centred; or shrunk to fit a smaller frame.
fn fit(im: Img, w: i32, h: i32) -> Img {
    if w == im.w && h == im.h {
        return im;
    }
    let mut out = Img::black(w.max(0), h.max(0));
    if w <= 0 || h <= 0 {
        return out;
    }
    let (num, den) = if w >= im.w && h >= im.h {
        ((w / im.w).min(h / im.h), 1)
    } else if w * im.h <= h * im.w {
        (w, im.w)
    } else {
        (h, im.h)
    };
    let (tw, th) = (im.w * num / den, im.h * num / den);
    let (ox, oy) = ((w - tw) / 2, (h - th) / 2);
    for y in 0..th {
        for x in 0..tw {
            if let Some(c) = im.get(x * den / num, y * den / num) {
                out.set(ox + x, oy + y, c);
            }
        }
    }
    out
}

pub(crate) fn paint_seat(run: &Run, focus: u32) -> Img {
    let Some(own) = run.caves.seats.get(&focus) else {
        return Img::black(NATIVE_W, NATIVE_H);
    };
    let seed = run.caves.seed.max(1);
    let tick = run.tick as u32;
    let mut cave = run.caves.view_live();
    #[cfg(feature = "testing")]
    cave_paint::apply_paint_over(&mut cave);
    let cam = cave_paint::camera(feet(own));
    let exit = caves::exit_cell(seed);
    // The seat's own lamp first, then the party's, then the one that leads out.
    let mut lamps = vec![lamp_of(own)];
    lamps.extend(run.caves.seats.iter().filter(|(id, _)| **id != focus).map(|(_, seat)| lamp_of(seat)));
    lamps.push(Lamp { px: (exit.0 * CELL + CELL / 2, exit.1 * CELL), cell: exit, small: true });
    let light = cave_paint::light_field(&cave, &lamps, cam);
    cave_paint::remember(&mut cave, &light, seed, tick);

    let (heads, sunk) = mine_heads(run);
    let mut im = cave_paint::paint(&cave, cam, &light, tick, &heads, sunk);
    if !cave_paint::pools_flat() {
        stamp_materials(&mut im, &cave, cam, tick, false);
    }
    for burst in &run.caves.bursts {
        if tick < burst.at {
            continue;
        }
        let (bx, by) = (i32::from(burst.x), i32::from(burst.y));
        // A crumble's or a blast's pivot is the floor line under the wall's
        // face. A chip flies from where the shovel lands: high on the face,
        // beside the knight. Rock with more rock south of it shows no face,
        // only its top: there a chip flies from the top, and a crumble starts
        // at the frame where the standing wall has already come apart.
        let drawn = cave_paint::cell_drawn(&cave, (bx, by), cam);
        // North chips are stamped after the knight. North rubble is stamped
        // before him. Neither is drawn in this pass.
        if north_digger(run, (bx, by)).is_some()
            && (burst.kind == caves::BURST_CRUMBLE || burst.kind == caves::BURST_CHIP)
        {
            continue;
        }
        let (below, lead) = match (drawn.face.is_some(), burst.kind) {
            (true, caves::BURST_CHIP) => (CELL + 6, 0),
            (true, _) => (CELL + cave_paint::FACE_PX, 0),
            (false, caves::BURST_CHIP) => (CELL - 2, 0),
            (false, caves::BURST_CRUMBLE) => (CELL + 8, 9),
            (false, _) => (CELL, 0),
        };
        let at = (drawn.top.x + CELL / 2, drawn.top.y + below);
        cave_fx::stamp(&mut im, burst, region_of(bx, by).0, tick + lead, at);
    }
    for seat in run.caves.seats.values() {
        if let Some((cell, dir, age)) = seat.skid() {
            let at = if cell == (i32::from(seat.x), i32::from(seat.y)) { feet(seat) } else { cave_paint::cell_px(cell.0, cell.1) };
            cave_paint::paint_skid(&mut im, &light, cam, (at.0, at.1 + 2), dir, age);
        }
    }
    paint_boots(run, &mut im, cam);

    // The lamp and the knights, the ones farther up the screen first.
    let exit_px = (exit.0 * CELL + CELL / 2, (exit.1 + 1) * CELL);
    let mut standing: Vec<(i32, Option<(&u32, &Seat)>)> = vec![(exit_px.1, None)];
    standing.extend(run.caves.seats.iter().map(|(id, seat)| (feet(seat).1 + CELL / 2, Some((id, seat)))));
    standing.sort_by_key(|(y, who)| (*y, who.map(|(id, _)| *id)));
    // The lamp's word goes under the knights, so it never hides one.
    if (i32::from(own.x) - exit.0).abs() + (i32::from(own.y) - exit.1).abs() <= 14 {
        label(&mut im, exit_px.0 - cam.0, exit_px.1 - cam.1 - 25, "OUT", 'h');
    }
    // Knights are drawn over the wall exactly as when nobody is digging.
    let digs: Vec<(i32, i32)> = run
        .caves
        .seats
        .values()
        .filter(|seat| seat.face == 3)
        .map(|seat| (i32::from(seat.x), i32::from(seat.y) - 1))
        .collect();
    stamp_north_rubble(&mut im, run, &cave, cam, tick);
    let plates = seat_plates(run, tick);
    for (_, who) in &standing {
        match who {
            None => stamp_way_out(&mut im, (exit_px.0 - cam.0, exit_px.1 - cam.1)),
            Some((id, seat)) => {
                let (fx, fy) = feet(seat);
                stamp_knight(&mut im, &plates, **id, (fx - cam.0, fy - cam.1));
            }
        }
    }
    let inks = knight_inks(&plates, run, cam);
    cave_paint::paint_north_cracks(&mut im, &cave, cam, &digs, &inks);
    stamp_north_chips(&mut im, run, cam, tick, &plates);
    stamp_north_breaks(&mut im, run, &cave, cam, tick, &inks);
    // A name over each other knight, once it is far enough off to want one.
    for (id, seat) in &run.caves.seats {
        let apart = (i32::from(seat.x) - i32::from(own.x)).abs() + (i32::from(seat.y) - i32::from(own.y)).abs();
        if *id != focus && apart > 3 {
            let name = run.players.get(id).map(|h| h.shown_name().to_uppercase()).unwrap_or_default();
            let (fx, fy) = feet(seat);
            // In the seat's cloth, as on its card in the party's band.
            let cloth = super::arena::seat_colours(run, *id).iter().find(|(from, _)| *from == '7').map_or('7', |(_, to)| *to);
            label(&mut im, fx - cam.0, fy - cam.1 - 23, &name, cloth);
        }
    }
    if !cave_paint::pools_flat() {
        stamp_materials(&mut im, &cave, cam, tick, true);
    }
    mine_prompt(run, focus, &mut im, cam);

    draw_pack(&mut im, own);
    let mut lines: Vec<(String, char)> = Vec::new();
    for line in run.notices.iter().filter(|s| cave_notice(s)).take(2) {
        lines.extend(wrap_notice(line, NATIVE_W - 16).into_iter().map(|l| (l, '9')));
    }
    for (id, line) in &run.seat_notices {
        if (*id == focus || *id == 0) && cave_notice(line) {
            lines.extend(wrap_notice(line, NATIVE_W - 16).into_iter().map(|l| (l, '4')));
        }
    }
    lines.truncate(3);
    if !lines.is_empty() {
        im.stamp(&super::arena::notice_board(&lines), 4, 4);
    }
    im
}

/// A short word over something in the cave, centred on `x`, with a dark
/// edge so it reads over lit floor and rock alike.
fn label(im: &mut Img, x: i32, y: i32, word: &str, ch: char) {
    let w = ink::text_width(word);
    let left = (x - w / 2).clamp(2, NATIVE_W - w - 2);
    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)] {
        im.text(left + dx, y + dy, word, 'k');
    }
    im.text(left, y, word, ch);
}

/// The knight standing in the cell south of `wall`, facing north, if one is.
fn north_digger<'a>(run: &'a Run, wall: (i32, i32)) -> Option<(u32, &'a Seat)> {
    run.caves.seats.iter().map(|(id, seat)| (*id, seat)).find(|(_, seat)| {
        seat.face == 3 && i32::from(seat.x) == wall.0 && i32::from(seat.y) == wall.1 + 1
    })
}

/// Screen pixels a knight's figure inks, plus a one-pixel rim: the same
/// figure `stamp_knight` draws (`knight_plate`), so a crack or a cloud never
/// lands on the knight the player sees.
struct SeatPlate {
    id: u32,
    shown: Img,
    foot_x: i32,
    foot_y: i32,
    keep: Option<(Img, i32, i32)>,
}

fn seat_plates(run: &Run, tick: u32) -> Vec<SeatPlate> {
    run.caves
        .seats
        .iter()
        .map(|(id, seat)| {
            let stepping = seat.stepping() && !seat.gliding();
            let (shown, foot_x, foot_y) =
                knight_plate(run, *id, seat.face, stepping, tool_of(run, seat, tick));
            let keep = (seat.face == 3).then(|| knight_plate(run, *id, seat.face, stepping, None));
            SeatPlate { id: *id, shown, foot_x, foot_y, keep }
        })
        .collect()
}

fn knight_inks(plates: &[SeatPlate], run: &Run, cam: (i32, i32)) -> impl Fn(i32, i32) -> bool {
    let sprites: Vec<(i32, i32, Img)> = plates
        .iter()
        .filter_map(|plate| {
            let seat = run.caves.seats.get(&plate.id)?;
            let (fx, fy) = feet(seat);
            let mut rim = Img::new(plate.shown.w + 2, plate.shown.h + 2);
            rim.stamp(&plate.shown, 1, 1);
            rim.outline_outside('k');
            Some((fx - cam.0 - plate.foot_x - 1, fy - cam.1 - plate.foot_y - 1, rim))
        })
        .collect();
    move |sx, sy| sprites.iter().any(|(x, y, sprite)| sprite.get(sx - x, sy - y).is_some())
}

/// The rectangle `stamp_knight` fills for knight `id`, feet at `at` on screen.
fn knight_sprite_rect(plates: &[SeatPlate], id: u32, at: (i32, i32)) -> Option<cave_paint::ScreenRect> {
    let plate = plates.iter().find(|plate| plate.id == id)?;
    let (keep, foot_x, foot_y) = plate.keep.as_ref()?;
    Some(cave_paint::ScreenRect { x: at.0 - foot_x, y: at.1 - foot_y, w: keep.w, h: keep.h })
}

/// Rubble from a north dig, before the knight, at the face-bottom pivot so it
/// lands at his feet and his ink is drawn over it.
fn stamp_north_rubble(im: &mut Img, run: &Run, cave: &Cave, cam: (i32, i32), tick: u32) {
    for burst in &run.caves.bursts {
        if tick < burst.at || burst.kind != caves::BURST_CRUMBLE {
            continue;
        }
        let wall = (i32::from(burst.x), i32::from(burst.y));
        let Some(_) = north_digger(run, wall) else { continue };
        let Some((pivot, region)) = north_break_pivot(cave, cam, wall) else { continue };
        cave_fx::stamp_if(im, burst, region, tick, pivot, |_, _, dy| dy >= 0);
    }
}

/// Chips from a north dig, after the knight. The burst sits at his helmet and
/// flies left and right. It does not land on the middle 8 px of his sprite.
fn stamp_north_chips(im: &mut Img, run: &Run, cam: (i32, i32), tick: u32, plates: &[SeatPlate]) {
    for burst in &run.caves.bursts {
        if tick < burst.at || burst.kind != caves::BURST_CHIP {
            continue;
        }
        let wall = (i32::from(burst.x), i32::from(burst.y));
        let Some((id, seat)) = north_digger(run, wall) else { continue };
        let (fx, fy) = feet(seat);
        let Some(rect) = knight_sprite_rect(plates, id, (fx - cam.0, fy - cam.1)) else { continue };
        // The helmet is four pixels under the sprite's top.
        const HELMET: i32 = 4;
        let pivot = (rect.x + rect.w / 2, rect.y + HELMET);
        let mid0 = rect.x + (rect.w - 8) / 2;
        let mid1 = mid0 + 8;
        let region = region_of(wall.0, wall.1).0;
        // The sheet's chips sit on its pivot. Two pivots put that cloud past
        // each shoulder, still at helmet height, never on the middle 8 px.
        let keep = |x: i32, y: i32, _dy: i32| {
            let over_middle = x >= mid0 && x < mid1 && y >= rect.y && y < rect.y + rect.h;
            !over_middle
        };
        let left = (mid0 - 2, pivot.1);
        let right = (mid1 + 2, pivot.1);
        cave_fx::stamp_if(im, burst, region, tick, left, keep);
        cave_fx::stamp_if(im, burst, region, tick, right, keep);
    }
}

fn north_break_pivot(cave: &Cave, cam: (i32, i32), wall: (i32, i32)) -> Option<((i32, i32), u8)> {
    let drawn = cave_paint::cell_drawn(cave, wall, cam);
    let face = drawn.face?;
    let pivot = (face.x + face.w / 2, face.y + face.h);
    Some((pivot, region_of(wall.0, wall.1).0))
}

/// A north dig's breaking cloud, after the knight. The pivot stays the face
/// bottom. The cloud never covers his ink. Rubble was already drawn before him.
fn stamp_north_breaks(
    im: &mut Img,
    run: &Run,
    cave: &Cave,
    cam: (i32, i32),
    tick: u32,
    inks: &dyn Fn(i32, i32) -> bool,
) {
    for burst in &run.caves.bursts {
        if tick < burst.at || burst.kind != caves::BURST_CRUMBLE {
            continue;
        }
        let wall = (i32::from(burst.x), i32::from(burst.y));
        let Some(_) = north_digger(run, wall) else { continue };
        let drawn = cave_paint::cell_drawn(cave, wall, cam);
        let Some(face) = drawn.face else { continue };
        let pivot = (face.x + face.w / 2, face.y + face.h);
        let region = region_of(wall.0, wall.1).0;
        cave_fx::stamp_if(im, burst, region, tick, pivot, |x, y, dy| {
            if dy >= 0 {
                return false;
            }
            let stone = cave_paint::painted_stone(x + cam.0, y + cam.1);
            let on_wall = drawn.top.contains(x, y) || face.contains(x, y);
            on_wall && stone && !inks(x, y)
        });
    }
}

/// One knight, feet at `at`. The body is the figure the delve room draws
/// for that hero, at the room's own pixel scale: cropped to its ink and
/// stood on the foot point. A hero with no look of their own is the kit
/// knight the room draws for them. A dig holds a short spade in the hand;
/// a blast reaches one hand toward the faced cell.
fn stamp_knight(im: &mut Img, plates: &[SeatPlate], id: u32, at: (i32, i32)) {
    let Some(plate) = plates.iter().find(|plate| plate.id == id) else { return };
    im.stamp(&Img::from_rows(SHADOW), at.0 - 7, at.1 - 1);
    im.stamp(&plate.shown, at.0 - plate.foot_x, at.1 - plate.foot_y);
}

/// What the hands are doing this tick, if the blade or the cast is still out.
fn tool_of(run: &Run, seat: &Seat, tick: u32) -> Option<Tool> {
    if let Some(age) = seat.swinging(tick) {
        if age < 5 {
            if seat.digging(tick).is_some() {
                let phase = if age < 2 { 0 } else if age < 3 { 1 } else { 2 };
                return Some(Tool::Dig(phase));
            }
            let _ = run;
            return Some(Tool::Cast);
        }
        return None;
    }
    if seat.stepping() {
        Some(Tool::Carry)
    } else {
        None
    }
}

enum Tool {
    Dig(u8),
    Cast,
    /// Slanted over the shoulder while walking.
    Carry,
}

/// The figure the cave stamps, before the cave's light, and where its foot
/// sits in that picture. `tool` is the shovel or the cast, never a rescaled body.
fn knight_plate(run: &Run, id: u32, face: u8, stepping: bool, tool: Option<Tool>) -> (Img, i32, i32) {
    let Some(hero) = run.players.get(&id) else {
        return kit_plate(face, stepping, tick_step(run), tool);
    };
    let mut posed = posed(hero, face, stepping);
    // A cast is an open hand, not the room's carried blade.
    if matches!(tool, Some(Tool::Cast)) {
        posed.sword = false;
        posed.swing = 0;
        posed.fire_cooldown = 0;
        posed.forged = None;
        posed.weapon = crate::rules::Weapon::Fists;
    }
    let (image, pivot) = super::arena::room_figure(run, id, &posed);
    let (mut sprite, mut foot_x, mut foot_y) = crop_feet(&image, pivot);
    if sprite.w <= 1 && sprite.h <= 1 && !opaque(&sprite) {
        return kit_plate(face, stepping, tick_step(run), tool);
    }
    if let Some(tool) = tool {
        let (held, ox, oy) = hold(&sprite, face, tool);
        sprite = held;
        foot_x += ox;
        foot_y += oy;
    }
    (sprite, foot_x, foot_y)
}

fn tick_step(run: &Run) -> u32 {
    run.tick as u32
}

/// The kit knight, only when this seat has no hero of its own.
fn kit_plate(face: u8, stepping: bool, tick: u32, tool: Option<Tool>) -> (Img, i32, i32) {
    let mut dress = Dress::WHOLE;
    dress.view = match face & 3 {
        3 => View::Away,
        1 | 2 => View::Across,
        _ => View::Toward,
    };
    dress.held = Held::Nothing;
    dress.step = if stepping { 1 + ((tick / 3) % 4) as u8 } else { 0 };
    let mut sprite = hero::draw(dress);
    if face & 3 == 1 {
        sprite = sprite.flip_h();
    }
    let (mut sprite, mut foot_x, mut foot_y) = crop_feet(&sprite, None);
    if let Some(tool) = tool {
        let (held, ox, oy) = hold(&sprite, face, tool);
        sprite = held;
        foot_x += ox;
        foot_y += oy;
    }
    (sprite, foot_x, foot_y)
}

/// Aim and stride the room reads, so its figure faces and walks as this seat does.
fn posed(hero: &crate::rules::Hero, face: u8, stepping: bool) -> crate::rules::Hero {
    let mut hero = hero.clone();
    match face & 3 {
        1 => { hero.aim_x = -1.0; hero.aim_y = 0.0; }
        2 => { hero.aim_x = 1.0; hero.aim_y = 0.0; }
        3 => { hero.aim_x = 0.0; hero.aim_y = -1.0; }
        _ => { hero.aim_x = 0.0; hero.aim_y = 1.0; }
    }
    hero.stride = if stepping { 0.85 } else { 0.0 };
    hero.swing = 0;
    hero.sword = false;
    hero.fire_cooldown = 0;
    hero.shielding = false;
    hero
}

fn opaque(im: &Img) -> bool {
    (0..im.h).any(|y| (0..im.w).any(|x| im.get(x, y).is_some()))
}

fn opaque_box(im: &Img) -> Option<(i32, i32, i32, i32)> {
    let mut l = im.w;
    let mut t = im.h;
    let mut r = -1;
    let mut b = -1;
    for y in 0..im.h {
        for x in 0..im.w {
            if im.get(x, y).is_some() {
                l = l.min(x);
                t = t.min(y);
                r = r.max(x);
                b = b.max(y);
            }
        }
    }
    (r >= 0).then_some((l, t, r, b))
}

/// Crop to the ink. The foot is the room's pivot when it sits in the ink,
/// otherwise the bottom centre of the crop.
fn crop_feet(im: &Img, pivot: Option<[i32; 2]>) -> (Img, i32, i32) {
    let Some((l, t, r, b)) = opaque_box(im) else {
        return (Img::new(1, 1), 0, 0);
    };
    let mut out = Img::new(r - l + 1, b - t + 1);
    for y in t..=b {
        for x in l..=r {
            if let Some(c) = im.get(x, y) {
                out.set(x - l, y - t, c);
            }
        }
    }
    let (fx, fy) = match pivot {
        Some([px, py]) if py >= t && py <= b => (px.clamp(l, r) - l, py - t),
        _ => ((r - l) / 2, b - t),
    };
    (out, fx, fy)
}

/// Timber ink `o`, grey stone `v`, and the next stone step `V` on the cutting edge.
const WOOD: [u8; 3] = [169, 118, 57];
const BLADE: [u8; 3] = [143, 163, 168];
const EDGE: [u8; 3] = [179, 194, 196];
const GLOVE: [u8; 3] = [232, 198, 164];

/// A short spade, or a hand reaching forward, drawn on the figure so the
/// grip starts at the hand and never covers the face.
fn hold(fig: &Img, face: u8, tool: Tool) -> (Img, i32, i32) {
    let marks = match tool {
        Tool::Dig(phase) => shovel_marks(fig, face, phase),
        Tool::Carry => shovel_marks(fig, face, 3),
        Tool::Cast => cast_marks(fig, face),
    };
    paint_marks(fig, &marks)
}

fn paint_marks(fig: &Img, marks: &[(i32, i32, [u8; 3])]) -> (Img, i32, i32) {
    if marks.is_empty() {
        return (fig.clone(), 0, 0);
    }
    let mut l = 0;
    let mut t = 0;
    let mut r = fig.w - 1;
    let mut b = fig.h - 1;
    for &(x, y, _) in marks {
        l = l.min(x);
        t = t.min(y);
        r = r.max(x);
        b = b.max(y);
    }
    let (ox, oy) = (-l.min(0), -t.min(0));
    let mut out = Img::new(r.max(fig.w - 1) - l.min(0) + 1, b.max(fig.h - 1) - t.min(0) + 1);
    out.stamp(fig, ox, oy);
    // Every mark is painted. Dropping one is what left the blade floating.
    // The shaft is bent off the face before it arrives here.
    for &(x, y, c) in marks {
        out.set(x + ox, y + oy, c);
    }
    (out, ox, oy)
}

fn face_box(fig: &Img) -> Option<(i32, i32, i32, i32)> {
    let (l, t, r, b) = opaque_box(fig)?;
    let bottom = t + ((b - t + 1) * 32 / 100).max(1);
    Some((l, t, r, bottom.min(b)))
}

fn hand_of(fig: &Img, face: u8) -> (i32, i32) {
    let Some((l, t, r, b)) = opaque_box(fig) else { return (0, 0) };
    // The arm, not the hat and not a forward foot.
    let y0 = t + (b - t + 1) * 42 / 100;
    let y1 = t + (b - t + 1) * 58 / 100;
    let aim_x = match face & 3 {
        1 => l + (r - l) / 6,
        2 => r - (r - l) / 6,
        _ => r - (r - l) / 4,
    };
    let aim_y = (y0 + y1) / 2;
    let mut best: Option<(i32, i32, i32)> = None;
    for y in y0..=y1 {
        for x in l..=r {
            if fig.get(x, y).is_none() {
                continue;
            }
            let near = (x - aim_x).abs() + (y - aim_y).abs();
            let solid = neighbors(fig, x, y);
            // Prefer a solid pixel on the facing side, so the grip is a hand, not a speck.
            let score = solid * 8 - near;
            if best.map_or(true, |(_, _, s)| score > s) {
                best = Some((x, y, score));
            }
        }
    }
    best.map(|(x, y, _)| (x, y)).unwrap_or((aim_x, aim_y))
}

fn neighbors(fig: &Img, x: i32, y: i32) -> i32 {
    let mut n = 0;
    for dy in -1..=1 {
        for dx in -1..=1 {
            if (dx, dy) != (0, 0) && fig.get(x + dx, y + dy).is_some() {
                n += 1;
            }
        }
    }
    n
}

/// Haft: 1 px wide, 6 px long, wood ink, starting at the hand. Blade: 3 by 3,
/// grey stone-light with a one-pixel lighter edge, set square on the end.
fn shovel_marks(fig: &Img, face: u8, phase: u8) -> Vec<(i32, i32, [u8; 3])> {
    let (hx, hy) = hand_of(fig, face);
    let offsets = match phase {
        0 => raise_offsets(face),
        1 => strike_offsets(face),
        2 => recover_offsets(face),
        _ => carry_offsets(face),
    };
    let (haft, blade) = clear_of_helmet(fig, face, (hx, hy), offsets);
    let mut marks = Vec::new();
    for (x, y) in haft {
        marks.push((x, y, WOOD));
    }
    let end = haft[5];
    let prev = haft[4];
    let dir = ((end.0 - prev.0).signum(), (end.1 - prev.1).signum());
    // A spade, not a hammer head: three across and three long from the haft,
    // then a point, so the silhouette tapers away from the hand. The lit edge
    // runs down one side and onto the tip.
    let base = (end.0 + dir.0, end.1 + dir.1);
    let _ = blade;
    for (along, across, lit) in spade(dir) {
        marks.push((base.0 + along, base.1 + across, if lit { EDGE } else { BLADE }));
    }
    marks
}

/// The spade's pixels from the first one past the haft, for a haft running in
/// `dir`: (x, y, lit edge).
fn spade((dx, dy): (i32, i32)) -> Vec<(i32, i32, bool)> {
    if dx == 0 || dy == 0 {
        // Straight: u along the haft, v across it.
        let (ux, uy) = (dx, dy);
        let (vx, vy) = (-uy, ux);
        let mut px = Vec::new();
        for u in 0..3 {
            for v in -1..=1 {
                px.push((u * ux + v * vx, u * uy + v * vy, v == 1 && u > 0));
            }
        }
        px.push((3 * ux, 3 * uy, true));
        px
    } else {
        // Diagonal: a fat blade two pixels thick along the diagonal, a point at the end.
        vec![
            (0, 0, false),
            (dx, 0, false),
            (0, dy, false),
            (dx, dy, false),
            (2 * dx, dy, true),
            (dx, 2 * dy, false),
            (2 * dx, 2 * dy, true),
            (3 * dx, 3 * dy, true),
        ]
    }
}

fn carry_offsets(face: u8) -> [(i32, i32); 6] {
    match face & 3 {
        1 => six(1, -1),
        2 => six(-1, -1),
        _ => six(1, -1),
    }
}

fn raise_offsets(face: u8) -> [(i32, i32); 6] {
    let back = if face & 3 == 2 { -1 } else { 1 };
    [(0, 0), (0, -1), (0, -2), (back, -3), (back, -4), (back, -5)]
}

fn strike_offsets(face: u8) -> [(i32, i32); 6] {
    match face & 3 {
        1 => six(-1, 0),
        2 => six(1, 0),
        3 => six(1, -1),
        _ => six(1, 1),
    }
}

fn recover_offsets(face: u8) -> [(i32, i32); 6] {
    let dx = if face & 3 == 1 { -1 } else { 1 };
    [(0, 0), (dx, 1), (dx, 2), (dx, 3), (dx, 4), (dx, 5)]
}

fn six(dx: i32, dy: i32) -> [(i32, i32); 6] {
    let mut pts = [(0, 0); 6];
    for i in 0..6 {
        pts[i] = (dx * i as i32, dy * i as i32);
    }
    pts
}

fn clear_of_helmet(fig: &Img, face: u8, hand: (i32, i32), offsets: [(i32, i32); 6]) -> ([(i32, i32); 6], (i32, i32)) {
    let side = if face & 3 == 1 { -1 } else { 1 };
    let helm = face_box(fig);
    let mut haft = offsets.map(|(dx, dy)| (hand.0 + dx, hand.1 + dy));
    let mut blade = blade_centre(hand, offsets);
    let shove = if offsets[5].0 != 0 { offsets[5].0.signum() } else { side };
    for shift in 0..6 {
        let ox = shove * (shift + 2);
        haft = offsets.map(|(dx, dy)| (hand.0 + dx + ox, hand.1 + dy));
        blade = blade_centre((hand.0 + ox, hand.1), offsets);
        if haft.iter().all(|p| !on_face(helm, fig, p.0, p.1)) && blade_clear(helm, fig, blade) {
            break;
        }
    }
    (haft, blade)
}

fn blade_centre(hand: (i32, i32), offsets: [(i32, i32); 6]) -> (i32, i32) {
    let end = offsets[5];
    let prev = offsets[4];
    let dir = ((end.0 - prev.0).signum(), (end.1 - prev.1).signum());
    (hand.0 + end.0 + dir.0 * 2, hand.1 + end.1 + dir.1 * 2)
}

fn blade_clear(helm: Option<(i32, i32, i32, i32)>, fig: &Img, blade: (i32, i32)) -> bool {
    for dy in -1..=1 {
        for dx in -1..=1 {
            if on_face(helm, fig, blade.0 + dx, blade.1 + dy) {
                return false;
            }
        }
    }
    true
}
fn off_face(fig: &Img, face: u8, mut x: i32, y: i32) -> (i32, i32) {
    let dir = if face & 3 == 1 { -1 } else { 1 };
    let helm = face_box(fig);
    for _ in 0..8 {
        if !on_face(helm, fig, x, y) {
            return (x, y);
        }
        x += dir;
    }
    (x, y)
}

fn on_face(helm: Option<(i32, i32, i32, i32)>, fig: &Img, x: i32, y: i32) -> bool {
    let Some((l, t, r, b)) = helm else {
        return false;
    };
    x >= l && x <= r && y >= t && y <= b && fig.get(x, y).is_some()
}

fn cast_marks(fig: &Img, face: u8) -> Vec<(i32, i32, [u8; 3])> {
    let (hx, hy) = hand_of(fig, face);
    // A short forearm and a small rounded fist, toward the faced cell.
    // Not a blade: no long edge, no metal.
    let (dx, dy) = match face & 3 {
        1 => (-1, 0),
        2 => (1, 0),
        3 => (0, -1),
        _ => (0, 1),
    };
    let mut marks = vec![(hx, hy, GLOVE), (hx + dx, hy + dy, GLOVE)];
    let (fx, fy) = off_face(fig, face, hx + dx * 2, hy + dy * 2);
    for (ox, oy) in [(0, 0), (-dy, dx), (dy, -dx), (dx, dy)] {
        let (x, y) = off_face(fig, face, fx + ox, fy + oy);
        if !marks.iter().any(|&(mx, my, _)| mx == x && my == y) {
            marks.push((x, y, GLOVE));
        }
    }
    marks
}

/// Opaque pixels of the figure the cave stamps, before light and before a tool.
fn figure_inks(run: &Run, id: u32, face: u8, stepping: bool) -> Vec<(i32, i32, [u8; 3])> {
    let (sprite, _, _) = knight_plate(run, id, face, stepping, None);
    inks_of(&sprite)
}

fn room_inks(run: &Run, id: u32, face: u8, stepping: bool) -> Vec<(i32, i32, [u8; 3])> {
    let Some(hero) = run.players.get(&id) else {
        return inks_of(&kit_plate(face, stepping, tick_step(run), None).0);
    };
    let posed = posed(hero, face, stepping);
    let (image, pivot) = super::arena::room_figure(run, id, &posed);
    inks_of(&crop_feet(&image, pivot).0)
}

fn inks_of(im: &Img) -> Vec<(i32, i32, [u8; 3])> {
    let mut out = Vec::new();
    for y in 0..im.h {
        for x in 0..im.w {
            if let Some(c) = im.get(x, y) {
                out.push((x, y, c));
            }
        }
    }
    out
}

fn ink_mismatch(a: &[(i32, i32, [u8; 3])], b: &[(i32, i32, [u8; 3])]) -> usize {
    use std::collections::BTreeMap;
    let left: BTreeMap<_, _> = a.iter().map(|(x, y, c)| ((*x, *y), *c)).collect();
    let right: BTreeMap<_, _> = b.iter().map(|(x, y, c)| ((*x, *y), *c)).collect();
    let mut n = 0;
    for (k, c) in &left {
        if right.get(k) != Some(c) {
            n += 1;
        }
    }
    for k in right.keys() {
        if !left.contains_key(k) {
            n += 1;
        }
    }
    n
}

const SHADOW: &[&str] = &[
    " KKKKKKKKKKKK ",
    " KKKKKKKKKKKK ",
    "  nnnnnnnnnn  ",
    "    nnnnnn    ",
];

/// The lamp that leads out: a lantern on a post, its foot at `at`.
fn stamp_way_out(im: &mut Img, at: (i32, i32)) {
    let lamp = Img::from_rows(WAY_OUT);
    im.stamp(&lamp, at.0 - lamp.w / 2, at.1 - lamp.h);
}

const WAY_OUT: &[&str] = &[
    "   kkk   ",
    "  kaaak  ",
    " kaO5Oak ",
    " ka565ak ",
    " kaO5Oak ",
    "  kaaak  ",
    "   knk   ",
    "   kIk   ",
    "   kIk   ",
    "   knk   ",
    "   kIk   ",
    "   kIk   ",
    "   knk   ",
    "  knInk  ",
    " knnnnnk ",
];

/// The cave's corner of the screen: stone carried, and magic left in blasts.
fn draw_pack(im: &mut Img, seat: &Seat) {
    let pack = seat.pack();
    let carried = i32::from(pack.total());
    let boots = match pack.footwear {
        Footwear::Plain => "",
        Footwear::Grip => "GRIP",
        Footwear::Blades => "BLADES",
    };
    let stone_w = ink::text_width("STONE");
    let magic_w = ink::text_width("MAGIC");
    let pips = i32::from(CARRY_LIMIT) * 4;
    let blasts = i32::from(MAGIC_MAX / BLAST_COST);
    let bars = i32::from(MAGIC_MAX) * 3 + (blasts - 1) * 2;
    let boots_w = if boots.is_empty() { 0 } else { ink::text_width(boots) + 8 };
    let w = 4 + stone_w + 4 + pips + 8 + magic_w + 4 + bars + boots_w + 3;
    let (x0, y0) = (3, NATIVE_H - 14);
    im.rect(x0, y0, w, 11, 'k');
    im.frame(x0, y0, w, 11, 'X');
    let mut x = x0 + 4;
    let y = y0 + 2;
    im.text(x, y, "STONE", 'J');
    x += stone_w + 4;
    // A full pack turns amber: the next stone dug is left behind.
    let full = pack.full();
    for i in 0..i32::from(CARRY_LIMIT) {
        let ch = if i >= carried { 'X' } else if full { 'O' } else { 'h' };
        im.rect(x + i * 4, y + 1, 3, 5, ch);
    }
    x += pips + 8;
    im.text(x, y, "MAGIC", 'J');
    x += magic_w + 4;
    // One group a blast.
    for i in 0..i32::from(MAGIC_MAX) {
        let group = i / i32::from(BLAST_COST);
        let ch = if i < i32::from(pack.magic) { 'O' } else { 'X' };
        im.rect(x + i * 3 + group * 2, y + 1, 2, 5, ch);
    }
    x += bars + 8;
    if !boots.is_empty() {
        im.text(x, y, boots, 'z');
    }
}

/// The delve's notices a knight in the cave is shown. The cave's corner
/// already counts the stone, and the delve's own warnings are not about here.
fn mine_heads(run: &Run) -> (Vec<cave_paint::MineHead>, u8) {
    let mut heads = Vec::new();
    let mut sunk = 0u8;
    for (index, mine) in &run.home.mines {
        if *index < 8 {
            sunk |= 1 << index;
        }
        let Some(body) = crate::rules::caves::mines::body(run.caves.seed, *index) else {
            continue;
        };
        let wall = (mine.head_x, mine.head_y - 1);
        if !body.cells.contains(&wall) {
            continue;
        }
        heads.push(cave_paint::MineHead {
            region: mine.region,
            wall,
            head: (mine.head_x, mine.head_y),
        });
    }
    (heads, sunk)
}

/// A knight on or beside a proven, unsunk head is offered the sink. Before
/// the vein is proven they see how much of it has been seen.
/// Flames and fizz sit on the floor, under the knights. Steam rises after them.
/// `steam` selects that second pass.
fn stamp_materials(im: &mut Img, cave: &Cave, cam: (i32, i32), tick: u32, steam: bool) {
    let x0 = cam.0.div_euclid(CELL) - 3;
    let y0 = cam.1.div_euclid(CELL) - 4;
    let x1 = (cam.0 + NATIVE_W).div_euclid(CELL) + 3;
    let y1 = (cam.1 + NATIVE_H).div_euclid(CELL) + 2;
    let seed = cave.seed.max(1);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let Some(cell) = cave.at(x, y) else { continue };
            if !cell.seen() {
                continue;
            }
            let (px, py) = cave_paint::cell_px(x, y);
            let at = (px - cam.0, py - cam.1);
            if steam {
                if cell.steam > 0 {
                    let cover = match cell.steam {
                        1 => 64,
                        2 => 128,
                        3 => 192,
                        _ => 255,
                    };
                    cave_fx::stamp_loop(im, cave_fx::LOOP_STEAM, cave_fx::loop_frame(tick, x, y, cave_fx::LOOP_STEAM, 23), at, cover);
                }
                continue;
            }
            if cell.fire > 0 {
                cave_fx::stamp_loop(im, cave_fx::LOOP_FLAME, cave_fx::loop_frame(tick, x, y, cave_fx::LOOP_FLAME, 11), at, 255);
            }
            if cell.ash() && cave_fx::ash_embers_live(seed, x, y, tick, true) {
                cave_fx::stamp_loop(im, cave_fx::LOOP_ASH, cave_fx::loop_frame(tick, x, y, cave_fx::LOOP_ASH, 41), at, 255);
            } else if !cell.ash() {
                let _ = cave_fx::ash_embers_live(seed, x, y, tick, false);
            }
            let acid = cell.fluid == FLUID_ACID && cell.amount > 0 && !cell.wall();
            let fizz = acid && ink::hash(x, y, 0xF122) % 3 == 0;
            let eaten = cell.wall() && !cell.wood() && cell.integrity > 0 && acid_beside(cave, x, y);
            if fizz || eaten {
                cave_fx::stamp_loop(im, cave_fx::LOOP_FIZZ, cave_fx::loop_frame(tick, x, y, cave_fx::LOOP_FIZZ, 37), at, 255);
            }
        }
    }
}

fn acid_beside(cave: &Cave, x: i32, y: i32) -> bool {
    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        if cave.at(x + dx, y + dy).is_some_and(|c| c.fluid == FLUID_ACID && c.amount > 0 && !c.wall()) {
            return true;
        }
    }
    false
}

fn mine_prompt(run: &Run, focus: u32, im: &mut Img, cam: (i32, i32)) {
    let Some(seat) = run.caves.seats.get(&focus) else {
        return;
    };
    let at = (i32::from(seat.x), i32::from(seat.y));
    let laid = crate::rules::caves::mines::bodies(run.caves.seed);
    let Some(body) = laid.bodies.iter().find(|b| crate::rules::caves::mines::beside(at, b.head)) else {
        return;
    };
    if run.home.mines.contains_key(&body.index) {
        return;
    }
    let n = crate::rules::caves::mines::proven(&run.caves, body);
    let word = if n >= crate::rules::caves::mines::PROVE_AT {
        "E: SINK A MINE".to_string()
    } else {
        format!("VEIN: {n} OF 8 SEEN")
    };
    let (fx, fy) = feet(seat);
    label(im, fx - cam.0, fy - cam.1 - 34, &word, '4');
}

fn cave_notice(line: &str) -> bool {
    let up = line.to_ascii_uppercase();
    if up.contains("A MINE IS SUNK") || up.contains("THE QUARRY WAITS") {
        return true;
    }
    !(up.contains("WARNING")
        || up.contains("PROSPECTOR")
        || up.contains("STALKER")
        || up.contains("STONE")
        || up.contains("FACE"))
}

fn wrap_notice(line: &str, width: i32) -> Vec<String> {
    let width = width.max(24);
    if ink::text_width(line) <= width {
        return vec![line.to_string()];
    }
    let mut out = Vec::new();
    let mut cur = String::new();
    for word in line.split_whitespace() {
        let next = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
        if ink::text_width(&next) <= width {
            cur = next;
        } else {
            if !cur.is_empty() {
                out.push(cur);
            }
            cur = word.to_string();
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    if out.is_empty() { vec![line.to_string()] } else { out }
}

/// Draw-thread work, once a cave: while any knight stands on a mouth, draw the
/// ground it will arrive on and read all twelve effect sheets, so the first
/// frame in every region is an ordinary one. A simulation tick does not call this.
pub fn warm_mouth(run: &Run, focus: Option<u32>) {
    thread_local! {
        static WARMED: Cell<u32> = const { Cell::new(0) };
    }
    let held = run.players.keys().any(|id| !run.caves.inside(*id) && run.stands_on_cave_mouth(*id))
        || focus.is_some_and(|id| !run.caves.inside(id) && run.stands_on_cave_mouth(id));
    if !held {
        return;
    }
    let seed = run.cave_seed();
    if WARMED.with(|w| w.replace(seed)) == seed {
        return;
    }
    let cell = caves::spawn_cell(seed);
    let exit = caves::exit_cell(seed);
    let mut cave = run.caves.view_as(seed);
    let (px, py) = cave_paint::cell_px(cell.0, cell.1);
    let lamps = [
        Lamp { px: (px, py + 2), cell, small: false },
        Lamp { px: (exit.0 * CELL + CELL / 2, exit.1 * CELL), cell: exit, small: true },
    ];
    cave_paint::warm(&mut cave, &lamps, cave_paint::camera((px, py)), seed, run.tick as u32);
    cave_fx::warm_all();
    let _ = hero::draw(Dress::WHOLE);
}

/// The cave mouth: a rough boulder arch over a stone stair going down, the
/// near step's lip catching the room's warm light. 32 by 26, room inks only.
const MOUTH: [&[u8; 32]; 26] = [
    b".........kkkkkkkkkkkkkk.........",
    b".......kkJJGGGjXJJJGGGjkk.......",
    b".....kkjXGGGGjjXJGGGjjjjgkk.....",
    b"....kjjXGGGjjjggXGjjjjgggXXk....",
    b"..kkXXXXGjjjjggXXXjjggggXJJJkk..",
    b".kGGjjjXXjjgkkkkkkkkXggXJJJGGGk.",
    b".kjjjjggXgkkkkkkkkkkkkXJJGGGjjk.",
    b"kXXXggggkkkkkkkkkkkkkkkkGGjjjjgk",
    b"JGGGXXXkkkkkkkkkkkkkkkkkkjjjgggg",
    b"GGjjXXXkkkkkkkkkkkkkkkkkkjjggggg",
    b"jjXXXXXkkkkkkkkkkkkkkkkkkggggggg",
    b"XXjjXXGkkkkkkkkkkkkkkkkkkXgggggg",
    b"jjjjXGGkkKKKKKKKKKKKKKKKkXgggggg",
    b"jjgXGjjkkkkkkkkkkkkkkkkkkXXggggg",
    b"jgXXjjjkkXXXXXXXXXXXXXXKkXXggggg",
    b"gXXjjjgkkkkkkkkkkkkkkkkkkJJXXXgg",
    b"XXXjggXkkggggggggggggggKkGGGjjXX",
    b"XXXXXXJkkkkkkkkkkkkkkkkkkGjjjjgg",
    b"XXXXXJJkjjjjjjjjjjjjjjjKkjjjgggg",
    b"XXXJJGGkkkkkkkkkkkkkkkkkkjjggggg",
    b"XJJGGGjkGJJJJJJJJJJJJJJKkggggggg",
    b"JJGGGjjkrroooooooooooorKkggggggg",
    b"kkkkGGJJJJGJJJJGJJJJGJJJJGJGkkkk",
    b"...kjjjjjjjjjjjjjjjjjjjjjjjjk...",
    b"...kggggggggggggggggggggggggk...",
    b"....kkkkkkkkkkkkkkkkkkkkkkkk....",
];

/// Stamps the mouth with its top-left at (`left`, `top`), each pixel `s` by `s`.
/// For a mouth at tile (`col`, `row`) that is `w` tiles wide, the near step sits
/// on that tile with `left = col * 16 + (w * 16 - 32) / 2` and `top = row * 16 - 9`.
pub fn stamp_mouth(im: &mut Img, left: i32, top: i32, s: i32) {
    let s = s.max(1);
    for (dy, row) in MOUTH.iter().enumerate() {
        for (dx, ch) in row.iter().enumerate() {
            if *ch == b'.' {
                continue;
            }
            let Some(c) = ink::ink(*ch as char) else { continue };
            fill(im, left + dx as i32 * s, top + dy as i32 * s, s, s, c);
        }
    }
}

/// The mouth's name, for every knight standing on it or one tile off it.
/// The arch itself is stamped with the room, whether or not anyone is near.
pub fn mouth_line(im: &mut Img, run: &Run) {
    if run.dungeon.depth != 1 {
        return;
    }
    let kind = run.room().kind;
    let near = run.players.values().any(|hero| {
        if hero.stone || hero.away || hero.absent {
            return false;
        }
        let col = (hero.x / crate::rules::TILE_UNITS) as i32;
        let row = (hero.y / crate::rules::TILE_UNITS) as i32;
        near_mouth(kind, col, row)
    });
    if !near {
        return;
    }
    let mouth = if kind == caves::YARD_MOUTH.room {
        &caves::YARD_MOUTH
    } else if kind == caves::FLOOR_MOUTH.room {
        &caves::FLOOR_MOUTH
    } else {
        return;
    };
    let line = mouth.label;
    let tw = ink::text_width(line);
    const TILE: i32 = 16;
    // Beside the arch, not across the opening, so the way down stays readable.
    let lx = mouth.at.0 * TILE + 34;
    let ly = mouth.at.1 * TILE - 2;
    im.rect(lx, ly, tw + 8, 12, 'k');
    im.frame(lx, ly, tw + 8, 12, 'g');
    im.text(lx + 4, ly + 2, line, 'j');
}

fn near_mouth(kind: crate::rules::RoomKind, col: i32, row: i32) -> bool {
    for mouth in [&caves::FLOOR_MOUTH, &caves::YARD_MOUTH] {
        if mouth.room != kind {
            continue;
        }
        let (x, y, w, h) = mouth.at;
        if col >= x - 1 && col < x + w + 1 && row >= y - 1 && row < y + h + 1 {
            return true;
        }
    }
    false
}

/// An untaken pair, lit by the same lamp that lights the floor under it.
/// A taken pair is gone: no name is left in the cave.
fn paint_boots(run: &Run, im: &mut Img, cam: (i32, i32)) {
    let seed = run.caves.seed;
    if seed == 0 {
        return;
    }
    let pairs = [
        (caves::grip_cell(seed), run.caves.grip_by, false),
        (caves::blades_cell(seed), run.caves.blades_by, true),
    ];
    let glint = run.tick % 60 < 8;
    for ((x, y), by, blades) in pairs {
        if by != 0 {
            continue;
        }
        let sx = x * CELL + CELL / 2 - cam.0 - 3;
        let sy = y * CELL + CELL / 2 - cam.1 - 5;
        paint_boot_pair(im, sx, sy, blades, glint);
    }
}

/// One boot, side on, toe to the right. Grip: brown leather, a folded cuff and
/// studs under the sole. Blades: steel-blue leather on a silver runner whose tip
/// curls past the toe. The outline stays dark at every lamp step so the boot
/// holds on pale frost and on black rock alike.
const GRIP_BOOT: [&[u8; 7]; 9] = [
    b"kkkk...", b"kOrk...", b"kRrk...", b"kBrk...", b"kBIkkk.", b"kBBBrrk", b"kBBBBBk", b"knnnnnk", b"kJkJkJk",
];
const BLADE_BOOT: [&[u8; 7]; 10] = [
    b"kkkk...", b"kVUk...", b"kUuk...", b"kuSk...", b"kuSkkk.", b"kuuuUUk", b"kuuuuuk", b"kxxxxxk", b"khJJJJh", b"......h",
];

fn paint_boot_pair(im: &mut Img, x: i32, y: i32, blades: bool, glint: bool) {
    let rows: &[&[u8; 7]] = if blades { &BLADE_BOOT } else { &GRIP_BOOT };
    let steps = lamp_steps(im, x, y);
    let warm_floor = floor_is_warm(im, x, y);
    // A one-row contact shadow: the floor under the sole at half light.
    let foot = y + rows.len() as i32;
    for dx in 1..7 {
        if let Some(c) = im.get(x + dx, foot) {
            im.set(x + dx, foot, [c[0] / 2, c[1] / 2, c[2] / 2]);
        }
    }
    for (dy, row) in rows.iter().enumerate() {
        for (dx, ch) in row.iter().enumerate() {
            if *ch == b'.' {
                continue;
            }
            let Some(base) = ink::ink(*ch as char) else { continue };
            let c = if *ch == b'k' {
                base
            } else {
                let mut c = super::light::step_down(base, steps);
                if warm_floor && steps > 0 {
                    c = super::light::warm(c);
                }
                c
            };
            im.set(x + dx as i32, y + dy as i32, c);
        }
    }
    if glint {
        let Some(tip) = ink::ink('t') else { return };
        im.set(x + 2, y + 1, tip);
    }
}

/// How many lamp steps the floor under the pair has already taken.
fn lamp_steps(im: &Img, x: i32, y: i32) -> usize {
    let mut best = 0u32;
    for (dx, dy) in [(0, 4), (3, 4), (6, 4), (3, 6), (1, 2)] {
        if let Some(c) = im.get(x + dx, y + dy) {
            let l = u32::from(c[0]) + u32::from(c[1]) + u32::from(c[2]);
            if l > best {
                best = l;
            }
        }
    }
    if best >= 150 {
        0
    } else if best >= 100 {
        1
    } else if best >= 64 {
        2
    } else if best >= 36 {
        3
    } else if best >= 18 {
        4
    } else {
        6
    }
}

fn floor_is_warm(im: &Img, x: i32, y: i32) -> bool {
    im.get(x + 3, y + 4).is_some_and(|c| c[0] > c[2].saturating_add(4))
}

fn fill(im: &mut Img, x: i32, y: i32, w: i32, h: i32, c: [u8; 3]) {
    for py in y..y + h {
        for px in x..x + w {
            im.set(px, py, c);
        }
    }
}

#[cfg(test)]
mod knight_pixels {
    use super::*;
    use crate::rules::cards::{Card, Kind, Rarity};
    use crate::rules::looks::{Looks, Sprite};
    use crate::rules::{Hero, Input, Run};
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::time::Instant;

    fn out_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.lane-out/cf3b")
    }

    fn wish_rows() -> Vec<Vec<String>> {
        // A tall blue cloak and a pale hat, so the wished look is not the kit.
        let rows = [
            "..hhh..",
            ".hhhhh.",
            "..bbb..",
            ".bbbbb.",
            "bbbgbbb",
            "bbbbbbb",
            "bb.b.bb",
            "b.bbb.b",
            "..b.b..",
            "..b.b..",
            ".bb.bb.",
        ];
        vec![rows.into_iter().map(str::to_string).collect()]
    }

    fn card(id: &str, rows: Vec<Vec<String>>) -> Card {
        Card {
            id: id.into(),
            name: id.into(),
            kind: Kind::Hold,
            rarity: Rarity::Common,
            text: String::new(),
            by: String::new(),
            drop: 0,
            chest: 0,
            effects: Vec::new(),
            arm: None,
            art: Vec::new(),
            only_in: None,
            guard: None,
            cooldown: 0,
            looks: Looks {
                worn: Some(Sprite { frames: rows, pixel: true, ..Sprite::default() }),
                ..Looks::default()
            },
            ult: None,
        }
    }

    fn blank(run: &Run) -> Hero {
        let mut hero = run.players.get(&1).expect("starter").clone();
        hero.deck.clear();
        hero.set_aside.clear();
        hero.class = None;
        hero.knight = None;
        hero.swing = 0;
        hero.sword = false;
        hero
    }

    fn four(run: &mut Run) {
        let base = blank(run);
        let wish = card("blue-cloak", wish_rows());
        run.book.insert(wish);
        let mut wished = base.clone();
        wished.deck.push("blue-cloak".into());
        run.players.insert(1, wished);

        let mut bard = base.clone();
        bard.class = Some("Bard".into());
        run.players.insert(2, bard);

        let aside = card("set-aside-cloak", wish_rows());
        run.book.insert(aside);
        let mut pub_pick = base.clone();
        pub_pick.deck.push("set-aside-cloak".into());
        pub_pick.set_aside.push("set-aside-cloak".into());
        pub_pick.knight = Some("great-helm".into());
        run.players.insert(3, pub_pick);

        run.players.insert(4, base);
    }

    #[test]
    fn each_cave_knight_matches_the_room_figure() {
        let mut run = Run::new(7, 1, None);
        four(&mut run);
        let names = [(1, "wish"), (2, "bard"), (3, "pub"), (4, "kit")];
        for (id, name) in names {
            let mut mismatch = 0;
            for face in 0..4 {
                for stepping in [false, true] {
                    mismatch += ink_mismatch(&room_inks(&run, id, face, stepping), &figure_inks(&run, id, face, stepping));
                }
            }
            eprintln!("mismatch {name}: {mismatch}");
            assert_eq!(mismatch, 0, "{name}");
        }
    }

    fn save_png(path: &std::path::Path, im: &Img, scale: i32) {
        let scale = scale.max(1);
        let w = im.w * scale;
        let h = im.h * scale;
        let mut buf = image::RgbaImage::new(w.max(1) as u32, h.max(1) as u32);
        for y in 0..h {
            for x in 0..w {
                let px = im.get(x / scale, y / scale);
                let p = match px {
                    Some([r, g, b]) => image::Rgba([r, g, b, 255]),
                    None => image::Rgba([0, 0, 0, 0]),
                };
                buf.put_pixel(x as u32, y as u32, p);
            }
        }
        std::fs::create_dir_all(path.parent().unwrap()).ok();
        buf.save(path).expect("png");
    }

    fn scale_stamp(dst: &mut Img, src: &Img, ox: i32, oy: i32, scale: i32) {
        for y in 0..src.h {
            for x in 0..src.w {
                if let Some(c) = src.get(x, y) {
                    for dy in 0..scale {
                        for dx in 0..scale {
                            dst.set(ox + x * scale + dx, oy + y * scale + dy, c);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn write_cf3b_proof() {
        let dir = out_dir();
        if std::env::var("ANGEL_CF3B_PROOF").is_err() {
            return;
        }
        std::fs::create_dir_all(&dir).ok();
        let mut run = Run::new(11, 1, None);
        four(&mut run);
        let names = [(1u32, "wish"), (2, "bard"), (3, "pub"), (4, "kit")];
        let mut rows = Vec::new();
        for (id, name) in names {
            let (room, _, _) = {
                let hero = run.players.get(&id).unwrap();
                let posed = posed(hero, 0, false);
                let (image, pivot) = super::super::arena::room_figure(&run, id, &posed);
                crop_feet(&image, pivot)
            };
            let (cave, _, _) = knight_plate(&run, id, 0, false, None);
            eprintln!("{name} room {}x{} cave {}x{}", room.w, room.h, cave.w, cave.h);
            rows.push((room, cave));
        }
        let scale = 3;
        let gap = 8;
        let cell_w = rows.iter().map(|(a, b)| a.w.max(b.w)).max().unwrap_or(1);
        let cell_h = rows.iter().map(|(a, b)| a.h.max(b.h)).max().unwrap_or(1);
        let sheet_w = (cell_w * 2 + gap) * scale;
        let sheet_h = rows.len() as i32 * (cell_h + gap) * scale;
        let mut sheet = Img::new(sheet_w, sheet_h);
        for (i, (room, cave)) in rows.iter().enumerate() {
            let y = i as i32 * (cell_h + gap) * scale;
            scale_stamp(&mut sheet, room, 0, y, scale);
            scale_stamp(&mut sheet, cave, (cell_w + gap) * scale, y, scale);
        }
        save_png(&dir.join("looks.png"), &sheet, 1);

        // Dig and blast on a real cave floor, not on black.
        run.enter_cave(false);
        let focus = *run.caves.seats.keys().next().unwrap_or(&1);
        let floor = cave_seat_frame(&run, focus, 384, 224);
        let patch = crop_floor(&floor);
        let mut dig = Img::new((patch.w * 3 + 12) * 3, (patch.h * 4 + 16) * 3);
        let mut blast = Img::new((patch.w + 4) * 3, (patch.h * 4 + 12) * 3);
        for face in 0..4 {
            for phase in 0..3 {
                let (sprite, fx, fy) = knight_plate(&run, 1, face, false, Some(Tool::Dig(phase)));
                let mut cell = patch.clone();
                cell.stamp(&sprite, patch.w / 2 - fx, patch.h - 8 - fy);
                scale_stamp(&mut dig, &cell, i32::from(phase) * (patch.w + 4) * 3, i32::from(face) * (patch.h + 4) * 3, 3);
            }
            let (sprite, fx, fy) = knight_plate(&run, 2, face, false, Some(Tool::Cast));
            let mut cell = patch.clone();
            cell.stamp(&sprite, patch.w / 2 - fx, patch.h - 8 - fy);
            scale_stamp(&mut blast, &cell, 0, i32::from(face) * (patch.h + 4) * 3, 3);
        }
        save_png(&dir.join("dig-pose.png"), &dig, 1);
        save_png(&dir.join("blast-pose.png"), &blast, 1);

        // Two knights with different looks.
        if run.players.len() >= 2 {
            run.enter_one(2, false);
        }
        let entered = cave_seat_frame(&run, 1, 384, 224);
        save_png(&dir.join("enter.png"), &entered, 1);

        // Walk at 1x: the cave frame while the seat steps, each its own tick.
        let mut frames = Vec::new();
        let t0 = Instant::now();
        for i in 0..12u64 {
            run.tick = 400 + i;
            let mut input = Input::default();
            input.move_x = 1;
            let mut inputs = BTreeMap::new();
            inputs.insert(1, input);
            run.tick_caves(&inputs);
            let frame = cave_seat_frame(&run, 1, 384, 224);
            save_png(&dir.join(format!("walk-{i:02}.png")), &frame, 1);
            frames.push(frame);
        }
        let walk_ms = t0.elapsed().as_secs_f64() * 1000.0 / 12.0;
        eprintln!("walk frame mean including rules: {walk_ms:.1} ms");

        // Mean of 60 mid-walk frames, each its own tick, draw only.
        let t1 = Instant::now();
        for i in 0..60u64 {
            run.tick = 800 + i;
            let _ = cave_seat_frame(&run, 1, 384, 224);
        }
        let draw_us = t1.elapsed().as_micros() as f64 / 60.0;
        eprintln!("mid-walk frame mean: {draw_us:.0} us");
        let _ = std::fs::write(dir.join("timing.txt"), format!("{draw_us:.0}\n"));
        let _ = frames;
    }

    fn crop_floor(im: &Img) -> Img {
        let (x, y, w, h) = (160, 80, 48, 56);
        let mut out = Img::new(w, h);
        for row in 0..h {
            for col in 0..w {
                if let Some(c) = im.get(x + col, y + row) {
                    out.set(col, row, c);
                }
            }
        }
        out
    }

    #[test]
    fn shovel_reads_alone() {
        if std::env::var("ANGEL_CF3C_PROOF").is_err() {
            return;
        }
        let mut run = Run::new(7, 1, None);
        four(&mut run);
        run.enter_one(1, false);
        let dir = std::env::temp_dir().join("delve-proof/cf3c");
        std::fs::create_dir_all(&dir).unwrap();
        let faces = [3u8, 2, 0, 1];
        let fw = 32;
        let fh = 34;
        let scale = 4;
        let gap = 8;
        let mut sheet = Img::new(3 * fw * scale + 2 * gap, 4 * fh * scale + 3 * gap);
        let mut walk = Img::new(4 * fw * scale + 3 * gap, fh * scale);
        let mut east_frames = Vec::new();
        let mut north_frames = Vec::new();
        for (row, face) in faces.iter().copied().enumerate() {
            park(&mut run, face);
            for (col, phase) in [0u8, 1, 2].iter().copied().enumerate() {
                let frame = lone(&run, face, Some(Tool::Dig(phase)), false);
                assert!(has_ink(&frame, WOOD) && has_ink(&frame, BLADE), "face {face} phase {phase}");
                scale_flat(&mut sheet, &frame, (col as i32) * (fw * scale + gap), (row as i32) * (fh * scale + gap), scale);
                if face == 2 {
                    east_frames.push(frame.clone());
                }
                if face == 3 {
                    north_frames.push(frame.clone());
                }
            }
            let carried = lone(&run, face, Some(Tool::Carry), true);
            assert!(has_ink(&carried, WOOD) && has_ink(&carried, BLADE), "carry {face}");
            scale_flat(&mut walk, &carried, (row as i32) * (fw * scale + gap), 0, scale);
        }
        save_flat(&dir.join("shovel-sheet.png"), &sheet, 1);
        save_flat(&dir.join("shovel-walk.png"), &walk, 1);
        let gif_dir = dir.join("gif-frames");
        std::fs::create_dir_all(&gif_dir).unwrap();
        for (i, frame) in east_frames.iter().enumerate() {
            save_flat(&gif_dir.join(format!("east-{i}.png")), frame, 3);
        }
        for (i, frame) in north_frames.iter().enumerate() {
            save_flat(&gif_dir.join(format!("north-{i}.png")), frame, 3);
        }
        let mut input = Input::default();
        input.move_x = 1;
        let mut inputs = BTreeMap::new();
        inputs.insert(1, input);
        run.tick_caves(&inputs);
        let t1 = std::time::Instant::now();
        for i in 0..60u64 {
            run.tick = 800 + i;
            let _ = cave_seat_frame(&run, 1, 384, 224);
        }
        let draw_us = t1.elapsed().as_micros() as f64 / 60.0;
        eprintln!("mid-walk frame mean: {draw_us:.0} us");
        let _ = std::fs::write(dir.join("timing.txt"), format!("{draw_us:.0}\n"));
    }

    fn park(run: &mut Run, face: u8) {
        let (dx, dy) = match face & 3 {
            1 => (-1, 0),
            2 => (1, 0),
            3 => (0, -1),
            _ => (0, 1),
        };
        let exit = caves::exit_cell(run.caves.seed.max(1));
        for y in 12..70 {
            for x in 16..150 {
                if (x - exit.0).abs() + (y - exit.1).abs() < 8 {
                    continue;
                }
                let open = run.caves.cell(x, y).is_some_and(|c| c.open());
                let wall = run.caves.cell(x + dx, y + dy).is_some_and(|c| c.wall());
                if !open || !wall {
                    continue;
                }
                let seat = run.caves.seats.get_mut(&1).unwrap();
                seat.x = x as i16;
                seat.y = y as i16;
                seat.face = face;
                seat.cool = 0;
                return;
            }
        }
        panic!("no faced wall for {face}");
    }

    fn lone(run: &Run, face: u8, tool: Option<Tool>, stepping: bool) -> Img {
        let mut frame = paint_seat(run, 1);
        let seat = run.caves.seats.get(&1).unwrap();
        let at = cave_paint::cell_px(i32::from(seat.x), i32::from(seat.y));
        let cam = cave_paint::camera(feet(seat));
        if let Some(tool) = tool {
            let (plate, fx, fy) = knight_plate(run, 1, face, stepping, Some(tool));
            frame.stamp(&plate, at.0 - cam.0 - fx, at.1 - cam.1 - fy);
        }
        crop_box(&frame, at.0 - cam.0 - 16, at.1 - cam.1 - 22, 32, 34)
    }

    fn crop_box(im: &Img, x: i32, y: i32, w: i32, h: i32) -> Img {
        let mut out = Img::new(w, h);
        for row in 0..h {
            for col in 0..w {
                out.set(col, row, im.get(x + col, y + row).unwrap_or([0, 0, 0]));
            }
        }
        out
    }

    fn has_ink(im: &Img, ink: [u8; 3]) -> bool {
        (0..im.h).any(|y| (0..im.w).any(|x| im.get(x, y) == Some(ink)))
    }

    fn scale_flat(dst: &mut Img, src: &Img, ox: i32, oy: i32, scale: i32) {
        for y in 0..src.h {
            for x in 0..src.w {
                let c = src.get(x, y).unwrap_or([0, 0, 0]);
                for dy in 0..scale {
                    for dx in 0..scale {
                        dst.set(ox + x * scale + dx, oy + y * scale + dy, c);
                    }
                }
            }
        }
    }

    fn save_flat(path: &std::path::Path, im: &Img, scale: i32) {
        let scale = scale.max(1);
        let w = im.w * scale;
        let h = im.h * scale;
        let mut buf = image::RgbaImage::new(w.max(1) as u32, h.max(1) as u32);
        for y in 0..h {
            for x in 0..w {
                let px = im.get(x / scale, y / scale).unwrap_or([0, 0, 0]);
                buf.put_pixel(x as u32, y as u32, image::Rgba([px[0], px[1], px[2], 255]));
            }
        }
        std::fs::create_dir_all(path.parent().unwrap()).ok();
        buf.save(path).expect("png");
    }

    #[test]
    fn print_mid_walk_and_north_dig_frame_cost() {
        let Ok(path) = std::env::var("FX1_FRAME_TIMES") else {
            return;
        };
        let mut run = Run::new(11, 1, Some("Ash"));
        run.enter_one(1, false);
        let walk_start = std::time::Instant::now();
        for i in 0..60 {
            if let Some(seat) = run.caves.seats.get_mut(&1) {
                seat.x = seat.x.wrapping_add(if i % 2 == 0 { 1 } else { -1 });
            }
            let _ = super::paint_seat(&run, 1);
        }
        let walk_ms = walk_start.elapsed().as_secs_f64() * 1000.0 / 60.0;
        if let Some(seat) = run.caves.seats.get_mut(&1) {
            seat.pose_north_dig(run.tick.max(1) as u32);
        }
        let dig_start = std::time::Instant::now();
        let _ = super::paint_seat(&run, 1);
        let dig_ms = dig_start.elapsed().as_secs_f64() * 1000.0;
        let line = format!("walk_mean_ms {walk_ms:.3} north_dig_ms {dig_ms:.3}\n");
        std::fs::write(path, line).expect("frame times");
    }

    #[test]
    fn mine_lines_reach_the_cave_and_other_stone_lines_stay_hidden() {
        assert!(super::cave_notice("A MINE IS SUNK: LIMESTONE NODE 1"));
        assert!(super::cave_notice("A MINE IS SUNK: FREESTONE NODE 2"));
        assert!(super::cave_notice("THE QUARRY WAITS"));
        assert!(!super::cave_notice("+1 STONE"));
        assert!(!super::cave_notice("WARNING: the roof"));
        assert!(!super::cave_notice("a PROSPECTOR waits"));
        assert!(!super::cave_notice("STALKER in the dark"));
        assert!(!super::cave_notice("the FACE of the wall"));
    }
}
