//! The Great Shaft and the stockyard at the Delve's gate, as the flat view
//! draws them. The shaft head stands over the gate's rock and is a different
//! machine at each tier: a tripod and bucket for hand carry, a windlass, a
//! horse gin with its horse walking the ring, a water hoist with its wheel,
//! and a ley lift, a column of light with a block rising in it. The stockyard
//! beside it is one stack of rough blocks for every stone waiting, in that
//! stone's colour. (`crate::rules::stockyard`.)
use super::*;
use crate::rules::stockyard::{Stockyard, Tier};

/// The head's picture: this wide and tall, standing on its foot.
pub const W: i32 = 48;
pub const H: i32 = 44;

/// The shaft head of `tier`, foot at the bottom middle.
pub fn head_img(tier: Tier, tick: u64) -> Img {
    let mut im = Img::new(W, H);
    let (cx, foot) = (W / 2, H - 1);
    let spin = (tick / 5) % 4;
    match tier {
        Tier::Hand => {
            // A tripod of poles over the collar, a pulley at its head, a
            // rope and a bucket that bobs, and baskets beside.
            im.line(cx, 12, cx - 9, foot, 'B');
            im.line(cx + 1, 12, cx - 8, foot, 'B');
            im.line(cx, 12, cx + 9, foot, 'B');
            im.line(cx + 1, 12, cx + 10, foot, 'B');
            im.line(cx, 12, cx + 2, foot - 3, 'r');
            im.rect(cx - 2, 9, 5, 5, 'h');
            im.rect(cx - 1, 10, 3, 3, 'k');
            let b = [0, 1, 2, 1][spin as usize];
            im.rect(cx, 14, 1, 16 + b, 't');
            im.rect(cx - 2, 30 + b, 5, 4, 'o');
            im.rect(cx - 2, 30 + b, 5, 1, 'O');
            for (x, y) in [(cx - 17, foot - 5), (cx + 13, foot - 4)] {
                im.rect(x, y, 6, 5, 'P');
                im.rect(x, y, 6, 1, 'O');
                im.rect(x + 1, y + 2, 4, 1, 'b');
            }
        }
        Tier::Windlass => {
            // Two posts, a beam, a drum on its axle and a crank.
            im.rect(cx - 12, 12, 3, foot - 11, 'B');
            im.rect(cx + 10, 12, 3, foot - 11, 'B');
            im.line(cx - 12, foot - 4, cx - 6, 20, 'r');
            im.line(cx + 12, foot - 4, cx + 6, 20, 'r');
            im.rect(cx - 14, 10, 29, 3, 'r');
            im.rect(cx - 14, 10, 29, 1, 'o');
            im.rect(cx - 9, 19, 19, 6, 'P');
            for k in 0..5 {
                im.rect(cx - 9 + ((k + spin as i32) % 5) * 4, 19, 1, 6, 'b');
            }
            // The crank's arm turns round the right post.
            let a = spin as f32 * std::f32::consts::FRAC_PI_2;
            let (hx, hy) = (cx + 15 + (a.cos() * 4.0) as i32, 22 + (a.sin() * 4.0) as i32);
            im.line(cx + 13, 22, hx, hy, 'O');
            im.rect(hx - 1, hy - 1, 3, 3, 'o');
            let b = [0, 1, 2, 1][spin as usize];
            im.rect(cx, 25, 1, 9 + b, 't');
            im.rect(cx - 2, 34 + b, 5, 4, 'o');
            im.rect(cx - 2, 34 + b, 5, 1, 'O');
            im.rect(cx + 16, foot - 6, 6, 6, 'P');
            im.rect(cx + 16, foot - 5, 6, 1, 'g');
        }
        Tier::HorseGin => {
            // A capstan with a long sweep, a horse walking its ring, and a
            // little headframe with the rope run across to it.
            let ring = (17.0, 5.0);
            let ground = foot as f32 - 6.0;
            for k in 0..28 {
                let a = k as f32 / 28.0 * std::f32::consts::TAU;
                im.put((cx as f32 + 6.0 + a.cos() * ring.0) as i32, (ground + a.sin() * ring.1) as i32, if k % 2 == 0 { 'j' } else { 'g' });
            }
            let (px, pz) = (cx + 6, ground as i32);
            im.rect(px - 1, 18, 3, pz - 17, 'B');
            im.rect(px - 2, 16, 5, 3, 'r');
            let a = tick as f32 / 22.0;
            let (ex, ey) = (px as f32 + a.cos() * ring.0, ground + a.sin() * ring.1);
            im.line(px, pz - 6, ex as i32, ey as i32 - 4, 'o');
            // The horse: a body, a neck and head, four legs that step.
            let (hx, hy) = (ex as i32, ey as i32);
            let step = (tick / 4) % 2;
            im.rect(hx - 4, hy - 8, 9, 4, 'P');
            im.rect(hx - 4, hy - 8, 9, 1, 'o');
            im.rect(hx + 3, hy - 11, 3, 4, 'P');
            im.rect(hx + 5, hy - 11, 2, 2, 'o');
            for (k, lx) in [-3, -1, 2, 4].into_iter().enumerate() {
                let lift = i32::from((k as u64 + step) % 2 == 0);
                im.rect(hx + lx, hy - 4 - lift, 1, 4 + lift, 'b');
            }
            im.rect(hx - 5, hy - 8, 1, 3, 'b');
            // The headframe on the left and the rope to it.
            im.rect(cx - 17, 16, 3, foot - 15, 'B');
            im.rect(cx - 8, 16, 3, foot - 15, 'B');
            im.rect(cx - 19, 14, 17, 3, 'r');
            im.rect(cx - 13, 12, 5, 5, 'h');
            im.rect(cx - 12, 13, 3, 3, 'k');
            im.line(cx - 10, 17, px, 22, 't');
            let b = [0, 1, 2, 1][spin as usize];
            im.rect(cx - 11, 17, 1, 12 + b, 't');
            im.rect(cx - 13, 29 + b, 5, 4, 'o');
        }
        Tier::WaterHoist => {
            // A headframe and drum on the left, geared to a wheel that the
            // race turns on the right.
            im.rect(cx - 20, 14, 3, foot - 13, 'B');
            im.rect(cx - 6, 14, 3, foot - 13, 'B');
            im.rect(cx - 22, 12, 21, 3, 'r');
            im.rect(cx - 17, 19, 12, 5, 'P');
            for k in 0..3 {
                im.rect(cx - 17 + ((k + spin as i32) % 3) * 4, 19, 1, 5, 'b');
            }
            let b = [0, 1, 2, 1][spin as usize];
            im.rect(cx - 12, 24, 1, 10 + b, 't');
            im.rect(cx - 14, 34 + b, 5, 4, 'o');
            // The wheel: a rim, spokes that turn, paddles, an axle.
            let (wx, wy, r) = (cx + 13, foot - 13, 11.0f32);
            for k in 0..64 {
                let a = k as f32 / 64.0 * std::f32::consts::TAU;
                im.put((wx as f32 + a.cos() * r) as i32, (wy as f32 + a.sin() * r) as i32, 'P');
                im.put((wx as f32 + a.cos() * (r - 1.0)) as i32, (wy as f32 + a.sin() * (r - 1.0)) as i32, 'r');
                im.put((wx as f32 + a.cos() * (r - 2.0)) as i32, (wy as f32 + a.sin() * (r - 2.0)) as i32, 'P');
            }
            for k in 0..4 {
                let a = (k as f32 / 4.0 + spin as f32 / 16.0) * std::f32::consts::TAU;
                im.line(wx, wy, (wx as f32 + a.cos() * r) as i32, (wy as f32 + a.sin() * r) as i32, 'B');
                im.rect((wx as f32 + a.cos() * r) as i32 - 1, (wy as f32 + a.sin() * r) as i32 - 1, 3, 2, 'o');
            }
            im.rect(wx - 1, wy - 1, 3, 3, 'k');
            // The gear tying it to the drum, and the race falling on the wheel.
            im.line(cx - 4, 21, wx - 2, wy - 2, 'h');
            for k in 0..6 {
                im.rect(wx + 8, 6 + k * 3, 2, 3, if (k + spin as i32) % 2 == 0 { 'Q' } else { 'z' });
            }
            im.rect(wx + 6, 4, 8, 2, 'P');
            im.rect(wx + 4, foot - 3, 14, 3, 'q');
            im.put(wx + 7 + spin as i32, foot - 4, 'V');
            im.put(wx + 12 - spin as i32, foot - 4, 'V');
        }
        Tier::LeyLift => {
            // A ring of pale stone, four standing stones with crystal heads,
            // and a column of light that carries a block up it.
            im.ellipse(cx as f32, foot as f32 - 3.0, 20.0, 6.0, 'J');
            im.ellipse(cx as f32, foot as f32 - 4.0, 17.0, 4.5, 'h');
            im.ellipse(cx as f32, foot as f32 - 4.0, 11.0, 3.0, 'k');
            for (k, (dx, dy)) in [(-17, -1), (17, -1), (-11, -8), (11, -8)].into_iter().enumerate() {
                let (sx, sy) = (cx + dx, foot - 4 + dy);
                im.rect(sx - 2, sy - 13, 4, 14, 'i');
                im.rect(sx - 2, sy - 13, 1, 14, 'H');
                im.rect(sx + 1, sy - 13, 1, 14, 'J');
                let glow = if (tick / 6 + k as u64) % 3 == 0 { 'w' } else { '3' };
                im.rect(sx - 1, sy - 17, 2, 4, glow);
                im.put(sx, sy - 18, '3');
            }
            // The column: dithered light, brighter at its core.
            for y in 2..foot - 6 {
                for x in cx - 4..=cx + 4 {
                    let d = (x - cx).abs();
                    let thin = d <= 1;
                    let flick = (y as u64 + tick / 3) % 4;
                    if thin {
                        im.put(x, y, if flick == 0 { 'w' } else { '3' });
                    } else if d <= 3 && (x + y + (tick / 4) as i32) % 3 == 0 {
                        im.put(x, y, '2');
                    } else if d == 4 && (y + (tick / 2) as i32) % 5 == 0 {
                        im.put(x, y, '0');
                    }
                }
            }
            // The block riding up it, and a mote or two.
            let rise = (tick / 3) % 28;
            let by = foot as i32 - 12 - rise as i32;
            im.rect(cx - 3, by, 7, 5, 'H');
            im.rect(cx - 3, by, 7, 1, 'W');
            im.rect(cx - 3, by + 4, 7, 1, 'h');
            for k in 0..3u64 {
                let my = foot as i32 - 8 - ((tick / 2 + k * 9) % 30) as i32;
                im.put(cx - 6 + (k as i32 * 5) % 11, my, 'w');
            }
        }
    }
    im
}

/// The colours a stone's blocks are stacked in: face, top.
fn block_inks(stone: &str) -> (char, char) {
    match stone {
        "limestone" => ('i', 'H'),
        "flint" => ('j', 'U'),
        "basalt" => ('g', 'j'),
        "freestone" | "slate-block" => ('u', 'U'),
        "tufa" => ('h', 'C'),
        "clay" => ('R', 'o'),
        "voidstone" => (')', '['),
        _ => ('h', 'i'),
    }
}

/// One rough block as a knight carries it, in its stone's colours.
pub fn carried_img(stone: &str) -> Img {
    let (face, top) = block_inks(stone);
    let mut im = Img::new(10, 8);
    im.rect(0, 1, 10, 7, face);
    im.rect(0, 0, 9, 2, top);
    im.rect(0, 7, 10, 1, 'k');
    im.rect(9, 1, 1, 7, 'k');
    im.put(3, 4, 'k');
    im.put(6, 5, 'k');
    im.rect(1, 2, 2, 1, top);
    im
}

/// One stack of `n` rough blocks of `stone`, foot at its bottom middle: a
/// pyramid three wide, at most nine shown.
pub fn stack_img(stone: &str, n: u32) -> Img {
    let (face, top) = block_inks(stone);
    let shown = n.clamp(1, 9) as i32;
    let mut im = Img::new(26, 20);
    let mut placed = 0;
    for (row, count) in [3, 2, 1].into_iter().enumerate() {
        for k in 0..count {
            if placed >= shown {
                break;
            }
            let x = 13 - count * 4 + k * 8 + if count == 3 { 0 } else { 0 };
            let y = 19 - 6 * (row as i32 + 1) + 1;
            im.rect(x, y, 8, 6, face);
            im.rect(x, y, 8, 2, top);
            im.rect(x, y + 5, 8, 1, 'k');
            im.rect(x + 7, y + 1, 1, 5, 'k');
            im.put(x + 2, y + 3, 'k');
            placed += 1;
        }
    }
    im
}

/// The stockyard: a stack for each stone waiting, side by side.
pub fn yard_img(yard: &Stockyard) -> Option<Img> {
    if yard.stone.is_empty() {
        return None;
    }
    let stacks: Vec<Img> = yard.stone.iter().map(|(s, &n)| stack_img(s, n)).collect();
    let w = stacks.len() as i32 * 24 + 4;
    let mut im = Img::new(w, 22);
    // A plank floor under the stacks, so a single block still reads as a yard.
    im.rect(0, 19, w, 3, 'P');
    im.rect(0, 19, w, 1, 'O');
    for k in 0..w / 6 {
        im.put(k * 6 + 2, 21, 'b');
    }
    for (i, s) in stacks.iter().enumerate() {
        im.stamp(s, i as i32 * 24 + 2, 0);
    }
    Some(im)
}

/// The shaft head over the gate and the stockyard beside it, on `over`;
/// `origin` is the view's corner in world pixels.
pub(super) fn paint(over: &mut Img, run: &Run, origin: (i32, i32)) {
    let gate = crate::world::wishes::delve_gate();
    let tier = run.home.stockyard.tier;
    let (gx, gy) = (
        gate.tx * TILE + gate.tw * TILE / 2 - origin.0,
        (gate.ty + gate.th) * TILE - 1 - origin.1,
    );
    if gx < -80 || gx > NATIVE_W + 80 || gy < -20 || gy > NATIVE_H + 120 {
        return;
    }
    // The head stands on the gate's rock, above the arch. On the hillside the sim's own lift is a
    // building of the works (`works_kit::yards`, one piece a tier) and is drawn with them: drawing
    // this one too would stand two heads at the gate.
    if !run.sim_hillside() {
        let head = head_img(tier, run.tick);
        over.stamp(&head, gx - head.w / 2, gy - 26 - head.h);
    }
    // The stockyard, south of the gate's stand.
    let (yx, yy) = crate::rules::quarrying::yard_spot();
    let (px, py) = (
        (yx / TILE_UNITS * TILE as f32) as i32 - origin.0,
        (yy / TILE_UNITS * TILE as f32) as i32 - origin.1 + TILE / 2,
    );
    if let Some(yard) = yard_img(&run.yard_view()) {
        over.stamp(&yard, px - yard.w / 2, py - yard.h);
    } else {
        // An empty yard is a square of trodden ground and a stake.
        over.rect(px - 10, py - 2, 20, 2, 'P');
        over.rect(px - 10, py - 2, 1, 5, 'B');
        over.rect(px + 9, py - 2, 1, 5, 'B');
    }
}

/// The winch: a post with a crank and a numbered board. `x`, `y` is its
/// foot.
pub(super) fn winch(over: &mut Img, run: &Run, x: i32, y: i32, live: Option<u64>) {
    let tier = run.home.stockyard.tier;
    over.rect(x - 1, y - 14, 3, 14, 'B');
    over.rect(x - 4, y - 2, 9, 2, 'g');
    let turn = live.map_or(0, |t| (t / 3) % 4);
    let (hx, hy) = [(5, -9), (3, -12), (-4, -10), (-3, -7)][turn as usize];
    over.line(x + 1, y - 10, x + hx, y + hy, 'O');
    over.rect(x + hx - 1, y + hy - 1, 3, 3, 'o');
    over.rect(x - 7, y - 24, 15, 9, 'P');
    over.rect(x - 7, y - 24, 15, 1, 'O');
    over.text(x - 3, y - 22, &(tier.index() + 1).to_string(), 'T');
}

/// `text` broken at spaces into lines of at most `width` letters.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match lines.last_mut() {
            Some(line) if line.len() + 1 + word.len() <= width => {
                line.push(' ');
                line.push_str(word);
            }
            _ => lines.push(word.to_string()),
        }
    }
    lines
}

/// The shaft's ledger, at the top of the screen as the survey's panel is,
/// while the knight stands at the winch: in as few lines as will say it.
pub(super) fn ledger(ui: &mut Img, run: &Run, focus: Option<u32>, geo: &super::map::Geography) -> Option<(i32, i32, i32, i32)> {
    use crate::rules::trinkets::{self, Thing};
    let hero = run.players.get(&focus.unwrap_or(1))?;
    // The same thing E would use: the nearest in reach.
    let near = trinkets::things(geo)
        .into_iter()
        .map(|(thing, (tx, ty))| {
            (thing, (hero.x - (tx as f32 + 0.5) * TILE_UNITS).hypot(hero.y - (ty as f32 + 0.5) * TILE_UNITS))
        })
        .filter(|&(_, d)| d < 1.8 * TILE_UNITS)
        .min_by(|a, b| a.1.total_cmp(&b.1));
    if near.map(|(t, _)| t) != Some(Thing::Winch) {
        return None;
    }
    let view = run.yard_view();
    let k = &view;
    let mut lines = vec![
        format!("THE GREAT SHAFT: {}", k.tier.name()),
        format!("LIFTS {} A MINUTE. YARD {}/{}", k.tier.throughput(), k.held(), k.tier.capacity()),
        format!("THE DEEP STIRS {}/{}", run.kingdom.stir.level, crate::rules::stir::MAX),
    ];
    match run.shaft_next() {
        None => lines.push("IT LIFTS AS MUCH AS IT EVER WILL".into()),
        Some((next, cost, ready)) => {
            lines.push(format!("NEXT: {}", next.name()));
            lines.extend(wrap(&crate::rules::stockyard::cost_label(&cost), 28));
            lines.extend(wrap(
                &match ready {
                    Ok(()) => "[E] TWICE TO PAY".to_string(),
                    Err(why) => why.to_uppercase(),
                },
                28,
            ));
        }
    }
    let w = lines.iter().map(|l| crate::ink::text_width(l)).max().unwrap_or(0) + 10;
    let h = lines.len() as i32 * 8 + 7;
    // In the top right, clear of the head over the gate and of the survey's
    // panel in the middle.
    let (px, py) = (NATIVE_W - w - 6, 6);
    chrome::panel(ui, (px, py, w, h), chrome::Tone::FRAMED);
    for (i, l) in lines.iter().enumerate() {
        ui.text(px + 5, py + 4 + 8 * i as i32, l, if i == 0 { 'O' } else { 'i' });
    }
    Some((px, py, w, h))
}

#[cfg(all(test, feature = "renderer"))]
#[path = "../../../../tests/render/shaft__shots.rs"]
mod shots;
