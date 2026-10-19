//! Live navy/brass UI. Text and rating/result values remain simulation-owned.
use super::super::super::ink::{Rgb, ink, text_width};
use super::{super::*, paint::*};
use crate::render::art;
/// A box rounded at this canvas's two samples a pixel: its corners follow
/// `curve` (in samples), `ink` colours each sample by how deep it lies
/// (1 on the edge) and its row, and a soft shadow drops down and right.
fn rounded(
    cv: &mut Canvas,
    (x, y, w, h): (f32, f32, f32, f32),
    curve: &[i32],
    ink: impl Fn(i32, i32, i32) -> Rgb,
) {
    let x0 = ((x - cv.origin.0) * 2.0).round() as i32;
    let y0 = ((y - cv.origin.1) * 2.0).round() as i32;
    let (w2, h2) = ((w * 2.0).round() as i32, (h * 2.0).round() as i32);
    let cut = |r: i32| {
        let e = r.min(h2 - 1 - r);
        if e < 0 { i32::MAX } else { curve.get(e as usize).copied().unwrap_or(0) }
    };
    let inside = |dx: i32, dy: i32| dy >= 0 && dy < h2 && dx >= cut(dy) && dx < w2 - cut(dy);
    let (cw, ch) = (cv.w as i32, cv.h as i32);
    let put = |px: &mut Vec<Rgb>, dx: i32, dy: i32, c: Rgb, a: f32| {
        let (sx, sy) = (x0 + dx, y0 + dy);
        if sx >= 0 && sy >= 0 && sx < cw && sy < ch {
            let i = (sy * cw + sx) as usize;
            px[i] = if a >= 1.0 { c } else { mix(px[i], c, a) };
        }
    };
    for dy in 0..h2 + 3 {
        for dx in 0..w2 + 3 {
            if !inside(dx, dy) && (inside(dx - 2, dy - 2) || inside(dx - 3, dy - 3)) {
                put(&mut cv.px, dx, dy, art::BLACK, if inside(dx - 2, dy - 2) { 0.6 } else { 0.3 });
            }
        }
    }
    for dy in 0..h2 {
        let c = cut(dy);
        for dx in c..w2 - c {
            let depth = (1..=4)
                .find(|&k| !inside(dx - k, dy) || !inside(dx + k, dy) || !inside(dx, dy - k) || !inside(dx, dy + k))
                .unwrap_or(5);
            put(&mut cv.px, dx, dy, ink(depth, dy, h2), 1.0);
        }
    }
}

/// The kit's panel, rounded like the ink kit's boards (`chrome`) at this
/// canvas's two samples a pixel: the corner follows a small circle, the
/// brass rim and the slate line inside it turn the corner with it, the
/// slate falls from a lit top toward black, a soft shadow drops down and
/// right, and a brass stud sits in each corner.
fn well(cv: &mut Canvas, x: f32, y: f32, w: f32, h: f32) {
    let ramp = Ramp {
        dark: art::WELL,
        mid: [16, 27, 37],
        light: [22, 35, 46],
    };
    rounded(cv, (x, y, w, h), &[6, 4, 3, 2, 1, 1], |depth, dy, h2| match depth {
        1 | 2 => art::TRIM,
        3 | 4 => material('S'),
        _ => ramp.at(1.0 - (dy as f32 + 0.5) / h2 as f32),
    });
    for (xx, yy) in [
        (x + 3.0, y + 3.0),
        (x + w - 4.0, y + 3.0),
        (x + 3.0, y + h - 4.0),
        (x + w - 4.0, y + h - 4.0),
    ] {
        cv.ellipse(xx + 0.5, yy + 0.5, 1.0, 1.0, Ramp::inks('P', 'o', 'T'), 4);
    }
}

/// A room's name plate: `fill` inside a one-pixel `rim`, its corners
/// rounded and its shadow dropped as `well`'s are.
pub(super) fn plaque(cv: &mut Canvas, (x, y, w, h): (f32, f32, f32, f32), fill: Rgb, rim: Rgb) {
    rounded(cv, (x, y, w, h), &[3, 2, 1], |depth, _, _| if depth <= 2 { rim } else { fill });
}

fn center(cv: &mut Canvas, y: f32, label: &str, c: char) {
    cv.text((384 - text_width(label)) as f32 * 0.5, y, label, c);
}
pub(super) fn votes(cv: &mut Canvas, run: &Run, focus: Option<u32>) {
    let id = focus.unwrap_or_else(|| run.playtest.local_seat());
    if !run.playtest.seats.get(&id).is_some_and(|s| s.open) {
        return;
    }
    for c in &mut cv.px {
        for k in c {
            *k = (*k as u16 * 130 / 256) as u8;
        }
    }
    well(cv, 8.0, 8.0, 368.0, 208.0);
    let mut im = Img::new(384, 224);
    super::super::votes::panel(&mut im, run, focus);
    // Carry the live text, stars, row positions and party totals. The old
    // nested frame is deliberately excluded, leaving one brass perimeter.
    for y in 17..212 {
        for x in 17..367 {
            if let Some(c) = im.get(x, y) {
                if crate::render::overworld::chrome::is_slate_fill(c) {
                    continue;
                }
                let c = if crate::render::overworld::chrome::is_highlight(c) {
                    [22, 49, 51]
                } else if Some(c) == ink('u') {
                    material('S')
                } else if Some(c) == ink('i') {
                    art::TEXT
                } else {
                    c
                };
                cv.rect(x as f32, y as f32, 1.0, 1.0, c);
            }
        }
    }
    // Selected row gets a narrow brass edge; the actual seat owns selection.
    let seat = &run.playtest.seats[&id];
    use crate::rules::votes::ITEMS;
    let selected = seat.selected.min(ITEMS.len() - 1);
    let mut row = 0usize;
    let mut group = "";
    let mut selected_row = 0usize;
    let mut count = 0usize;
    for (i, item) in ITEMS.iter().enumerate() {
        if group != item.group {
            row += 1;
            group = item.group;
        }
        if i == selected {
            selected_row = row;
        }
        row += 1;
        count = row;
    }
    let start = selected_row.saturating_sub(4).min(count.saturating_sub(11));
    cv.rect(
        18.0,
        53.0 + (selected_row - start) as f32 * 12.0,
        0.8,
        11.0,
        [237, 191, 99],
    );
}
pub(super) fn result(cv: &mut Canvas, run: &Run, focus: Option<u32>) {
    let id = focus.or_else(|| run.players.keys().next().copied());
    let Some((id, h, r)) = id.and_then(|id| {
        run.players
            .get(&id)
            .and_then(|h| h.pit.result.as_ref().map(|r| (id, h, r)))
    }) else {
        return;
    };
    // A closing result uses the original shrinking theatre, preserving timing.
    if r.left < 10 {
        return;
    }
    for c in &mut cv.px {
        for k in c {
            *k = (*k as u16 * 120 / 256) as u8;
        }
    }
    well(cv, 16.0, 11.0, 352.0, 202.0);
    cv.text(30.0, 24.0, "PRIVY DASH", 'O');
    let name = h
        .shown_name()
        .to_uppercase();
    cv.text(155.0, 24.0, &name.chars().take(16).collect::<String>(), 'i');
    let seconds = |t: u32| {
        format!(
            "{:.2}S",
            (t * 100 / crate::rules::HZ) as f32 / 100.0
        )
    };
    let time = seconds(r.total);
    cv.text(354.0 - text_width(&time) as f32, 24.0, &time, 'O');
    cv.rect(24.0, 42.0, 336.0, 108.0, art::BLACK);
    for i in 0..4 {
        let x = 42.0 + i as f32 * 80.0;
        for j in 0..6 {
            let v = hash(i, j, 83);
            cv.mist(
                x + j as f32 * 9.0,
                70.0 + (v % 9) as f32,
                8.0,
                2.0 + (v % 4) as f32,
                material('S'),
                0.32,
            );
        }
    }
    for i in 0..5 {
        let x = 165.0 + i as f32 * 17.0;
        let y = 82.0 + (i % 2) as f32 * 5.0;
        cv.rect(x, y, 6.0, 23.0, material('s'));
        for dx in [0.0, 2.5, 5.0] {
            cv.rect(x + dx, y - 2.0, 1.0, 3.0, material('s'));
        }
        cv.rect(x + 2.0, y + 8.0, 1.0, 2.0, material('T'));
    }
    cv.ellipse(68.0, 58.0, 8.0, 8.0, Ramp::inks('q', 'U', 'i'), 4);
    cv.flat_ellipse(71.0, 55.0, 7.0, 7.0, art::BLACK, 1.0);
    for i in 0..12 {
        super::scenes::tree(
            cv,
            32.0 + i as f32 * 29.0,
            101.0 + (i % 3) as f32 * 6.0,
            18.0 + (i % 4) as f32 * 3.0,
            i,
            true,
        );
    }
    super::kit::area(cv, "privy-wall", 24.0, 123.0, 336.0, 27.0, 0.8);
    super::kit::sprite(
        cv,
        super::kit::get("privy-house"),
        230.0,
        89.0,
        true,
        0.98,
        None,
        0.0,
    );
    cv.rect(244.0, 140.0, 17.0, 1.0, material('9'));
    cv.glow(Point::new(251.0, 140.0, 26.0, '@', 0.85));
    for x in [35.0, 280.0] {
        cv.gradient(x, 121.0, 3.0, 16.0, Ramp::inks('b', 'P', 'o'), false);
        super::flame(cv, x + 1.0, 120.0, run.tick, x as u32);
    }
    let im = super::kit::get("knight-run");
    cv.shadow(116.0, 140.0, 13.0, 3.0, 0.7);
    super::kit::sprite(cv, im, 100.0, 104.0, true, 1.6, Some(id), 0.0);
    for i in 0..6 {
        let x = 104.0 + i as f32 * 8.0;
        let y = 88.0 + (i % 3) as f32 * 9.0;
        cv.line((x - 1.5, y), (x + 1.5, y), 0.5, material('T'));
        cv.line((x, y - 1.5), (x, y + 1.5), 0.5, material('9'));
    }

    cv.line((165.0, 126.0), (183.0, 126.0), 2.0, material('T'));
    cv.line((178.0, 122.0), (183.0, 126.0), 1.5, material('T'));
    cv.line((178.0, 130.0), (183.0, 126.0), 1.5, material('T'));
    // Wide enough for a seven-letter name and its time, the times flush
    // right, so a long name never runs into its number.
    well(cv, 278.0, 82.0, 82.0, 48.0);
    cv.text(284.0, 87.0, "PARTY BEST", 'O');
    for (i, b) in run.pits.ranked().iter().take(3).enumerate() {
        cv.text(
            284.0,
            99.0 + i as f32 * 9.0,
            &b.name.to_uppercase().chars().take(7).collect::<String>(),
            'i',
        );
        let time = format!("{:.2}", (b.best * 100 / crate::rules::HZ) as f32 / 100.0);
        cv.text(355.0 - text_width(&time) as f32, 99.0 + i as f32 * 9.0, &time, 'i');
    }
    let label = r.quality.word();
    let mut title = Img::new(text_width(label), 7);
    title.text(0, 0, label, 'O');
    let title = super::super::enlarge(&title, 2.0);
    cv.stamp(&title, (384 - title.w) as f32 * 0.5, 164.0, true);
    center(
        cv,
        192.0,
        &format!(
            "{}{}",
            if r.party_record {
                "PARTY RECORD!  "
            } else {
                ""
            },
            time
        ),
        'T',
    );
}
/// Restyle flat legacy action wells and prompt backgrounds only. Critical
/// attack and health colours are left exact; no global colour transfer.
pub(super) fn stamp_overlay(cv: &mut Canvas, over: &Img) {
    let mut styled = over.clone();
    for y in 0..over.h {
        for x in 0..over.w {
            if let Some(c) = over.get(x, y) {
                // These two near-black inks are UI paper/contours, never a cue core.
                if c == ink('K').unwrap() || c == ink('k').unwrap() {
                    styled.set(x, y, art::WELL);
                }
            }
        }
    }
    // The screen's marks keep their samples under the atmosphere.
    cv.stamp_kept(&styled);
}

pub(super) fn race_board(cv: &mut Canvas, race: &crate::rules::chariot::Chariot) {
    use crate::rules::chariot as cr;
    if race.phase != cr::Phase::Racing {
        cv.stamp(&super::super::hippodrome::race_board(race), 4.0, 13.0, true);
        return;
    }
    let p = race.player();
    let order = race.order();
    let place = 1 + order.iter().position(|&i| i == 0).unwrap_or(0);
    well(cv, 4.0, 4.0, 228.0, 39.0);
    cv.text(12.0, 10.0, &race.call().to_uppercase(), 'O');
    cv.text(
        12.0,
        21.0,
        &format!(
            "LAP {}/{}   PLACE {}/4   CALTROPS {}",
            p.lap(race.course()) + 1,
            cr::LAPS,
            place,
            p.caltrops
        ),
        'i',
    );
    cv.text(12.0, 33.0, "A D STEER   SPACE WHIP   X CALTROP", 'i');
    let filled = (p.wind * 10.0).round() as i32;
    for i in 0..10 {
        cv.rect(
            176.0 + i as f32 * 4.0,
            11.0,
            3.0,
            5.0,
            if i < filled {
                material('T')
            } else {
                material('S')
            },
        );
    }
}
