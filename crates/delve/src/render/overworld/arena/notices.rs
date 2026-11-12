//! The boards and banners drawn over the view: a finished run's verdict,
//! a Sanctuary's sign and field banner, the heat's standings, the way on,
//! what lies beyond the view, a feat's banner and the dare board.

use super::*;

/// A finished run's verdict, on a board clear of the knights, the fallen
/// too: a verdict never lies over the bones it is about. A finished run's
/// clock has stopped (a wiped party's bones are already down, `bones`), so
/// it comes in on the verdict's own: the steps it has stood
/// (`floor_stats::Results::stood`). A run with no results to stand (one
/// ended at home) shows it still.
pub(super) fn verdict(cv: &mut Img, run: &Run) {
    let text = match run.phase {
        Phase::Won => "VICTORY",
        Phase::Wiped => "FALLEN",
        _ => return sanctuary_sign(cv, run),
    };
    let age = run
        .floor_results
        .as_ref()
        .filter(|r| r.end != crate::rules::floor_stats::End::Cleared)
        .map_or(f32::MAX, |r| r.stood as f32);
    let sign = district_sign(text);
    let hint = again_hint(run);
    let w = super::super::ink::text_width(&hint);
    let mut plate = Img::new(w + 7, 12);
    chrome::panel(&mut plate, (0, 0, w + 6, 11), chrome::Tone::KEY);
    plate.text(3, 2, &hint, 'O');
    let y = board_row(run, sign.h + 14, true);
    board_in(cv, &sign, ((NATIVE_W - sign.w) / 2, y), age, None);
    board_from(cv, &plate, ((NATIVE_W - plate.w) / 2, y + sign.h + 2), age - HINT_LAG, None, 6.0);
}

/// A guardian's entrance, over the view: a moment after it comes, a rule
/// draws out both ways from GUARDIAN, its name comes down onto it
/// (`board_in`), they hold, and they lift away (`BOSS_TITLE` ticks in all),
/// in the band clear of the knights. Its own plate comes up over its head
/// meanwhile (`wanted::entrance`).
pub(super) fn boss_title(cv: &mut Img, run: &Run) {
    use crate::motion::{ease, share};
    let Some((e, boss)) = run
        .enemies
        .iter()
        .filter(|e| e.hp > 0 && e.age < BOSS_TITLE && super::wanted::plated(run, e))
        .find_map(|e| Some((e, run.bosses.get(usize::from(e.boss?))?)))
    else {
        return;
    };
    let age = e.age as f32 - 4.0;
    if age < 0.0 {
        return;
    }
    let life = (BOSS_TITLE - 4) as f32;
    let sign = district_sign(&boss.name);
    let y = board_row(run, sign.h + 12, false).min(NATIVE_H / 2 - sign.h);
    let caption = "GUARDIAN";
    let cw = super::super::ink::text_width(caption);
    let cx = (NATIVE_W - cw) / 2;
    let leave = share(age, life - 8.0, 8.0);
    // The rule: out from the word both ways, then back in as it goes.
    let reach = (sign.w / 2 + 14) as f32 * ease::out(share(age, 0.0, 9.0)) * (1.0 - ease::inward(leave));
    let mid = NATIVE_W / 2;
    let ry = y - 5;
    for dx in (cw / 2 + 3)..(cw / 2 + 3 + reach as i32) {
        for x in [mid - dx, mid + dx - 1] {
            if super::super::ink::bayer(x, ry) >= leave {
                cv.put(x, ry, if dx % 2 == 0 { 'O' } else { 'o' });
            }
        }
    }
    let mut word = Img::new(cw + 2, 9);
    for (dx, dy) in [(0, 1), (2, 1), (1, 0), (1, 2)] {
        word.text(dx, dy, caption, 'k');
    }
    word.text(1, 1, caption, 'O');
    board_in(cv, &word, (cx - 1, ry - 4), age, Some(life));
    board_in(cv, &sign, ((NATIVE_W - sign.w) / 2, y), age - 3.0, Some(life - 3.0));
}

/// Ticks a guardian's title stands over the view as it comes.
pub(super) const BOSS_TITLE: u32 = 60;

/// Ticks a board's key line follows the board (up into place from
/// under it).
const HINT_LAG: f32 = 5.0;

/// A board put up `age` ticks ago, resting at `(x, y)`, on the shared
/// clock (docs/MOTION.md): it drops in from above, past its rest and back
/// (`ease::back`), dithering on over its first ticks; it holds; and in the
/// last ticks of its `life` (`None`: it stays) it lifts and dithers away.
/// Nothing before it is put up.
pub(super) fn board_in(cv: &mut Img, im: &Img, at: (i32, i32), age: f32, life: Option<f32>) {
    board_from(cv, im, at, age, life, -12.0);
}

/// `board_in`, coming in from `from` pixels off its rest (below it when
/// more than nothing: a label that pops up into place).
pub(super) fn board_from(cv: &mut Img, im: &Img, (x, y): (i32, i32), age: f32, life: Option<f32>, from: f32) {
    use crate::motion::{ease, share};
    if age < 0.0 {
        return;
    }
    const ENTER: f32 = 8.0;
    const LEAVE: f32 = 8.0;
    let drop = from * (1.0 - ease::back(share(age, 0.0, ENTER)));
    let (lift, gone) = match life {
        Some(life) => {
            let t = share(age, life - LEAVE, LEAVE);
            (-7.0 * ease::inward(t), ease::in_quad(t))
        }
        None => (0.0, 0.0),
    };
    let on = share(age, 0.0, 3.0);
    #[cfg(any(test, feature = "testing"))]
    if on > 0.0 && gone < 1.0 {
        crate::ink::said::board(im.w, im.h);
    }
    let y = (y as f32 + drop + lift).round() as i32;
    if on >= 1.0 && gone <= 0.0 {
        cv.stamp(im, x, y);
        return;
    }
    for yy in 0..im.h {
        for xx in 0..im.w {
            let Some(c) = im.get(xx, yy) else { continue };
            let b = super::super::ink::bayer(x + xx, y + yy);
            if b < on && b >= gone {
                cv.set(x + xx, y + yy, c);
            }
        }
    }
}

/// The way on from a finished run (`rules::again`): Enter asks, and once a
/// knight has, how long until the party goes.
pub(super) fn again_hint(run: &Run) -> String {
    match &run.again {
        None => "ENTER: BACK TO THE KINGDOM".to_string(),
        Some(again) => {
            let secs = again.left.div_ceil(crate::rules::HZ).max(1);
            format!("BACK TO THE KINGDOM IN {secs}  ENTER: NOW")
        }
    }
}

/// For a few seconds after a Sanctuary opens: its name, and what it is for.
fn sanctuary_sign(cv: &mut Img, run: &Run) {
    let hz = u64::from(crate::rules::HZ);
    // A village of the Deep has the screen while the star falls and its
    // name is up (`deep_village`).
    if run.in_deep_village()
        && run.deep.as_ref().and_then(|d| d.village.as_ref()).is_some_and(|v| run.tick < v.arrived + crate::rules::deep::village::FALL + 150)
    {
        return;
    }
    // A room won opens a wish window: a short sign says so.
    let in_sanctuary = run.room().kind == crate::rules::RoomKind::Sanctuary;
    let (at, title, hint, last) = match (run.hallowed, run.window.as_ref()) {
        // Only in the Sanctuary itself: a knight who walks straight out
        // leaves its sign behind.
        (Some(at), _) if in_sanctuary && run.tick.saturating_sub(at) < 4 * hz => {
            (at, "SANCTUARY", "REST: WOUNDS CLOSE", 4)
        }
        // A folly has the screen: its title card, not the wish sign.
        (_, Some(w))
            if run.room().kind != crate::rules::RoomKind::Sanctuary && !run.folly_running() =>
        {
            // The identity start's first floor: each room asks its question.
            let ask = (run.identity && run.dungeon.depth == 1 && run.at < crate::rules::identity::ROOMS)
                .then(|| crate::rules::identity::Ask::ROOMS[run.at]);
            let title = match (w.great, w.given, ask) {
                (_, _, Some(crate::rules::identity::Ask::Body)) => "WHAT ARE YOU?",
                (_, _, Some(crate::rules::identity::Ask::Weapon)) => "YOUR WEAPON?",
                (_, _, Some(crate::rules::identity::Ask::Ultimate)) => "YOUR ULTIMATE?",
                (true, _, _) => "A SUPER WISH",
                (_, 3, _) => "THREE WISHES",
                (_, 2, _) => "TWO WISHES",
                _ => "A WISH",
            };
            let hint = if ask.is_some() { "T: WISH IT" } else if w.great { "T: TWO STEPS AT ONCE" } else { "T TO MAKE THEM" };
            // A floor's own wishes (the plain ones a floor gives as the
            // party arrives) come with its title card: the sign waits for
            // the card, so its seconds are read, not spent under it
            // (measured 2026-10-29: 1.2 of its 3 s showed on floor 1).
            let card = if w.given > 0 && !w.great && !run.identity { u64::from(crate::rules::floor_plates::DURATION) } else { 0 };
            (w.opened + card, title, hint, if ask.is_some() { 5 } else { 3 })
        }
        _ => return,
    };
    if run.tick < at {
        return;
    }
    let (age, life) = (run.tick.saturating_sub(at) as f32, (last * hz) as f32);
    if age >= life {
        return;
    }
    let sign = district_sign(title);
    let y = sign_row(run, sign.h + 14);
    board_in(cv, &sign, ((NATIVE_W - sign.w) / 2, y), age, Some(life));
    let w = super::super::ink::text_width(hint);
    let mut plate = Img::new(w + 7, 12);
    chrome::panel(&mut plate, (0, 0, w + 6, 11), chrome::Tone::KEY);
    plate.text(3, 2, hint, 'O');
    board_from(cv, &plate, ((NATIVE_W - plate.w) / 2, y + sign.h + 2), age - HINT_LAG, Some(life - HINT_LAG), 6.0);
}

/// The row a sign `h` tall stands on: the middle of the view, unless a
/// knight stands there, then the upper band, then the lower, wherever no
/// knight is (the identity start's room questions stay up through a fight
/// and must never hide a knight). The middle when every band has one.
fn sign_row(run: &Run, h: i32) -> i32 {
    board_row(run, h, false)
}

/// `sign_row`, keeping clear of the fallen too (`fallen`): a verdict never
/// lies over the bones it is about.
pub(super) fn board_row(run: &Run, h: i32, fallen: bool) -> i32 {
    let (ox, oy) = view_origin(run, None);
    let knights: Vec<(i32, i32)> = run
        .players
        .values()
        .filter(|k| (fallen || k.hp > 0) && !k.absent)
        .map(|k| {
            let (x, y) = at(k.x, k.y);
            (x - ox, y - oy)
        })
        .collect();
    let clear = |top: i32| {
        knights
            .iter()
            .all(|&(x, y)| x < -12 || x > NATIVE_W + 12 || y + 6 < top || y - 26 > top + h)
    };
    let middle = NATIVE_H / 2 - (h - 14);
    [middle, 22, NATIVE_H - h - 34].into_iter().find(|&top| clear(top)).unwrap_or(middle)
}

/// A Sanctuary's field banner: the next class's colour on a pole, and the
/// key and price when the knight is close.
pub(super) fn field_banner(over: &mut Img, run: &Run, (ox, oy): (i32, i32), focus: Option<u32>) {
    use crate::rules::classes::{self, FIELD_PRICE};
    let Some((bx, by)) = run.room().field_banner() else {
        return;
    };
    let Some(hero) = run.players.get(&focus.unwrap_or(1)) else {
        return;
    };
    let next = classes::next_class(hero);
    let (x, y) = at(bx, by);
    let (x, y) = (x - ox, y - oy);
    over.rect(x - 4, y - 3, 9, 3, 'G');
    over.rect(x - 1, y - 30, 2, 28, 'h');
    let flap = i32::from(run.tick / 8 % 2 == 0);
    over.rect(x + 1, y - 29 + flap, 13, 11, next.ink);
    over.rect(x + 1, y - 29 + flap, 13, 1, 'K');
    if (hero.x - bx).hypot(hero.y - by) < 3.0 * TILE_UNITS {
        let label = format!("E: {} {FIELD_PRICE}G", next.name.to_uppercase());
        let w = super::super::ink::text_width(&label);
        let (lx, ly) = ((x - w / 2).clamp(3, NATIVE_W - w - 3), y + 3);
        chrome::label(over, (lx, ly), &label, 'O', chrome::Tone::KEY);
    }
}

/// Everyone plays at once: the standings, top right, live — each knight's
/// colour, name and how far along they are.
pub(in super::super) fn heat_board(over: &mut Img, run: &Run) {
    let Some(heat) = run.pastimes.heat.as_ref().filter(|h| h.seats.len() > 1) else {
        return;
    };
    let need = i32::from(heat.game.need()).max(1);
    let (w, row) = (118, 11);
    let x0 = NATIVE_W - w - 5;
    let y0 = 5;
    chrome::panel(over, (x0 - 4, y0 - 4, w + 8, row * heat.seats.len() as i32 + 5), chrome::Tone::SLATE);
    for (i, &id) in heat.seats.iter().enumerate() {
        let y = y0 + i as i32 * row;
        let ink = knights::seat_mark(id);
        over.rect(x0, y + 1, 5, 5, ink);
        let name: String = run
            .players
            .get(&id)
            .map_or_else(|| format!("SEAT {id}"), |h| h.name.to_uppercase())
            .chars()
            .take(9)
            .collect();
        over.text(x0 + 8, y, &name, 'O');
        let got = i32::from(heat.standing.get(&id).copied().unwrap_or(0));
        let bar = 40;
        let fill = super::super::ink::ink(ink).unwrap_or(BLACK);
        chrome::meter(over, (x0 + w - bar, y + 1, bar, 5), (bar - 2) * got / need, fill);
    }
}

/// The way on, below: once a room is clear, an arrow in the doorway that
/// leads toward the floor's next aim (`Run::way_on`). It sits on the wall
/// itself, its words along the wall, so it never covers the room's boards.
pub(super) fn way_on_marker(over: &mut Img, run: &Run, (ox, oy): (i32, i32), tick: u32) {
    let Some((dir, name)) = run.way_on() else {
        return;
    };
    let room = run.room();
    let (w, h) = (room.cols as f32 * TILE_UNITS, room.rows as f32 * TILE_UNITS);
    let wall = 0.5 * TILE_UNITS;
    let (ux, uy) = [(0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0)][dir];
    let (x, y) = match dir {
        0 => (w / 2.0, wall),
        1 => (w - wall, h / 2.0),
        2 => (w / 2.0, h - wall),
        _ => (wall, h / 2.0),
    };
    let nudge = [0.0, 1.0, 2.0, 1.0][(tick / 6 % 4) as usize];
    let (px, py) = at(x, y);
    let (sx, sy) = ((px - ox) as f32 + ux * nudge, (py - oy) as f32 + uy * nudge);
    let (sx, sy) = (sx.clamp(10.0, (NATIVE_W - 10) as f32), sy.clamp(10.0, (NATIVE_H - 10) as f32));
    for (ink, grow) in [('K', 1.5), ('O', 0.0)] {
        for dy in -9..=9 {
            for dx in -9..=9 {
                let along = dx as f32 * ux + dy as f32 * uy;
                let across = (dy as f32 * ux - dx as f32 * uy).abs();
                if (-4.0 - grow..=6.0 + grow).contains(&along)
                    && across <= (6.0 + grow - along) * 0.5 + grow * 0.5
                {
                    over.put(sx.round() as i32 + dx, sy.round() as i32 + dy, ink);
                }
            }
        }
    }
    // The words run along the wall beside the doorway.
    let label = name.to_uppercase();
    let tw = super::super::ink::text_width(&label);
    let (lx, ly) = match dir {
        0 | 2 => (sx as i32 + 2 * TILE, sy as i32 - 3),
        _ => (
            if dir == 1 { sx as i32 - tw - 12 } else { sx as i32 + 12 },
            sy as i32 - 2 * TILE,
        ),
    };
    let (mut lx, mut ly) = (lx.clamp(4, NATIVE_W - tw - 4), ly.clamp(4, NATIVE_H - 13));
    // Not under the minimap in the corner: to the other side of the arrow,
    // else below the minimap.
    if let Some((rx, ry, rw, rh)) = dungeon_map::reserved(run) {
        let hits = |x: i32, y: i32| {
            let (x0, y0, w, h) = chrome::label_rect((x, y), &label);
            x0 < rx + rw && x0 + w > rx && y0 < ry + rh && y0 + h > ry
        };
        if hits(lx, ly) && dir % 2 == 0 {
            let across = (sx as i32 - 2 * TILE - tw).clamp(4, NATIVE_W - tw - 4);
            if !hits(across, ly) {
                lx = across;
            }
        }
        if hits(lx, ly) {
            ly = ry + rh + 3;
        }
    }
    chrome::label(over, (lx, ly), &label, 'O', chrome::Tone::KEY);
}

pub(super) fn beyond(over: &mut Img, run: &Run, (ox, oy): (i32, i32)) {
    let (cx, cy) = ((ox + NATIVE_W / 2) as f32, (oy + NATIVE_H / 2) as f32);
    // Under the Wax Maze's strip, not behind it.
    let top = wax_maze::strip_bottom(run).or_else(|| belfry_shoot::strip_bottom(run)).or_else(|| powder_room::strip_bottom(run)).map_or(0.0, |b| (b + 5) as f32);
    let mut mark = |x: i32, y: i32, ink: char| {
        if (ox..ox + NATIVE_W).contains(&x) && (oy..oy + NATIVE_H).contains(&y) {
            return;
        }
        let (dx, dy) = (x as f32 - cx, y as f32 - cy);
        let k = ((NATIVE_W / 2 - 4) as f32 / dx.abs().max(0.01))
            .min((NATIVE_H / 2 - 4) as f32 / dy.abs().max(0.01));
        let (ex, ey) = (
            (NATIVE_W / 2) as f32 + dx * k,
            ((NATIVE_H / 2) as f32 + dy * k).max(top),
        );
        let len = dx.hypot(dy).max(0.01);
        let (ux, uy) = (dx / len, dy / len);
        for py in -4..=4 {
            for px in -4..=4 {
                // Along the way out (tip at +2) and across it.
                let along = px as f32 * ux + py as f32 * uy;
                let across = (py as f32 * ux - px as f32 * uy).abs();
                if (-3.0..=2.0).contains(&along) && across <= (2.0 - along) * 0.75 + 0.3 {
                    over.put(ex.round() as i32 + px, ey.round() as i32 + py, ink);
                }
            }
        }
    };
    // On an open map, only what is awake and near enough to matter.
    let open = run.open_map();
    for enemy in &run.enemies {
        let (x, y) = at(enemy.x, enemy.y);
        if open && (enemy.dormant || (x as f32 - cx).abs() > NATIVE_W as f32 * 1.5 || (y as f32 - cy).abs() > NATIVE_H as f32 * 1.5) {
            continue;
        }
        mark(x, y, '7');
    }
    for (&id, hero) in run.players.iter().filter(|(_, h)| h.hp > 0) {
        let (x, y) = at(hero.x, hero.y);
        mark(x, y, knights::seat_mark(id));
    }
    // The Wax Maze's wraiths out of sight, in their own inks.
    for ((x, y), ink) in wax_maze::beyond(run) {
        mark(x, y, ink);
    }
}

/// A deed recorded by the Herald, with its coffer reward.
pub(super) fn achievement_banner(cv: &mut Img, run: &Run, feat: &crate::rules::feats::Feat) {
    use super::super::ink::text_width;
    // Three short lines: what it is, its name, what was done.
    let rows: Vec<(String, char)> = vec![
        (format!("{} ACHIEVEMENT", crate::rules::feats::tier_word(feat.tier).to_uppercase()), 'O'),
        (feat.name.to_uppercase(), 'i'),
        (feat.says.to_uppercase(), 'h'),
    ];
    let w = rows.iter().map(|(t, _)| text_width(t)).max().unwrap_or(0) + 12;
    let h = rows.len() as i32 * 9 + 7;
    let mut board = Img::new(w + 1, h + 1);
    chrome::panel(&mut board, (0, 0, w, h), chrome::Tone::FRAMED);
    for (i, (text, ink)) in rows.iter().enumerate() {
        board.text((w - text_width(text)) / 2, 4 + i as i32 * 9, text, *ink);
    }
    // It drops in from the top and settles.
    let (at, _) = run.banner.as_ref().expect("a banner is up");
    let age = run.tick.saturating_sub(*at) as i32;
    let y = (age * 3 - h).min(4);
    cv.stamp(&board, (NATIVE_W - w) / 2, y);
}

/// Fortune's dare as the party arrives on a floor: a small card low in the
/// view's left corner (`fortune::dare`), two lines of plain words and her
/// purse in pictures. It waits behind the floor's own wish sign
/// (`Tune::dare_after`): one board at a time, the wish's key read first.
pub(super) fn dare_board(cv: &mut Img, run: &Run) {
    super::fortune::dare::board(cv, run);
}

/// Where the standings board top right (`folly_board`, `heat_board`)
/// ends, when one is up: its foot's row, at most (a row a knight).
pub(super) fn top_right_board_bottom(run: &Run) -> Option<i32> {
    let here = run.at;
    // The Wax Maze's strip is one row along the top.
    if let Some(bottom) = wax_maze::strip_bottom(run).or_else(|| belfry_shoot::strip_bottom(run)).or_else(|| powder_room::strip_bottom(run)) {
        return Some(bottom);
    }
    let folly = run.bats.as_ref().is_some_and(|f| f.room == here)
        || run.gantry.as_ref().is_some_and(|f| f.room == here)
        || run.powder.as_ref().is_some_and(|f| f.room == here)
        || run.hop.as_ref().is_some_and(|f| f.room == here);
    let row = 11;
    if folly {
        // The headline row and a row a knight, as `folly_board` lays them.
        return Some(1 + row * (run.players.len() as i32 + 1) + 5);
    }
    run.pastimes
        .heat
        .as_ref()
        .filter(|h| h.seats.len() > 1)
        .map(|h| 1 + row * h.seats.len() as i32 + 5)
}
