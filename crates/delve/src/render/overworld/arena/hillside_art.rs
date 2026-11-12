//! The hillside works drawn in the world you walk in (the Works, `world::hillside`):
//! the production sim's buildings, roads, tracks and ropeways, the carts and
//! wagons and buckets that run on them, the people at their posts and walking
//! between them, the ghost of what a knight is about to lay, the flow overlay
//! and the builder's words. The flat view stamps the kit's pictures
//! (`works_kit`, 16 px a tile); in 3D the buildings and tracks stand up as the
//! kit's blocks (`world3d::hillside`) and what moves stands up as figures, the
//! ghost and the overlay lie on the ground as marks.

use super::*;
use crate::kingdom::Status;
use crate::render::overworld::works_kit::{self as kit, Load, State, hands as khands, lines as klines, rollers as kroll};
use crate::rules::builder_rig;
use crate::rules::hillside::{self as build, Ghost, Tool, Verdict};
use crate::world::hillside::{self as hill, Heat, HillView, Rider};

const T: i32 = 16;

/// Whether a tile rectangle `[x0, y0, x1, y1]` is near the view `(origin, size)`.
fn seen(rect: [i16; 4], origin: (i32, i32), size: (i32, i32)) -> bool {
    let (x0, y0, x1, y1) = (i32::from(rect[0]) * T, i32::from(rect[1]) * T, (i32::from(rect[2]) + 1) * T, (i32::from(rect[3]) + 1) * T);
    x1 > origin.0 - 32 && x0 < origin.0 + size.0 + 32 && y1 > origin.1 - 48 && y0 < origin.1 + size.1 + 96
}

fn in_view((x, y): (f32, f32), origin: (i32, i32), size: (i32, i32), slack: i32) -> bool {
    let (px, py) = ((x * 16.0) as i32 - origin.0, (y * 16.0) as i32 - origin.1);
    px > -slack && py > -slack - 40 && px < size.0 + slack && py < size.1 + slack + 40
}

/// The roads, shafts, leylines and the tracks of tramways, chutes, ropeways and
/// barges, laid on the ground: flat view only (in 3D they stand as blocks).
pub(super) fn ground(run: &Run, view: &HillView, origin: (i32, i32), size: (i32, i32)) -> Img {
    let mut im = Img::new(size.0, size.1);
    let frame = hill::frame(run);
    let tile_at = |im: &mut Img, pic: &Img, t: (i16, i16)| {
        let (x, y) = (i32::from(t.0) * T - origin.0, (i32::from(t.1) + 1) * T - origin.1 - pic.h);
        if x > -T && y > -pic.h && x < size.0 && y < size.1 {
            im.stamp(pic, x, y);
        }
    };
    for w in &view.roads {
        tile_at(&mut im, &klines::line_flat(klines::Kind::Cart, w.links, false, 0), w.tile);
    }
    for (set, plan) in [(&view.shafts, "drive-shaft"), (&view.leylines, "leyline")] {
        if let Some(pic) = kit::flat(plan, State::Idle, 0) {
            for w in set {
                tile_at(&mut im, &pic, w.tile);
            }
        }
    }
    for line in &view.lines {
        let n = line.path.len();
        for (i, t) in line.path.iter().enumerate() {
            if i == 0 || i + 1 == n {
                continue;
            }
            let near = |j: usize| line.path[j];
            let mut links = 0;
            for j in [i - 1, i + 1] {
                let o = near(j);
                links |= match (o.0 - t.0, o.1 - t.1) {
                    (0, -1) => klines::N,
                    (1, 0) => klines::E,
                    (0, 1) => klines::S,
                    (-1, 0) => klines::W,
                    _ => 0,
                };
            }
            // A ropeway's pylon stands every fourth tile.
            let pic = klines::line_flat(line.kind, links, line.kind == klines::Kind::Rope && i % 4 == 1, frame);
            tile_at(&mut im, &pic, *t);
        }
    }
    rails(&mut im, run, view, origin, size);
    belts(&mut im, run, view, origin, size);
    im
}

/// The rail network: track a tile at a time, the lever where it parts, the bay's platform and lantern.
fn rails(im: &mut Img, run: &Run, view: &HillView, origin: (i32, i32), size: (i32, i32)) {
    if view.rails.is_empty() {
        return;
    }
    let frame = hill::frame(run);
    for r in &view.rails {
        let (x, y) = (i32::from(r.tile.0) * T - origin.0, i32::from(r.tile.1) * T - origin.1);
        if x <= -T || y <= -2 * T || x >= size.0 || y >= size.1 + T {
            continue;
        }
        let pic = klines::line_flat(klines::Kind::Tram, r.links, false, 0);
        im.stamp(&pic, x, y + T - pic.h);
        if let Some(bay) = &r.bay {
            let pic = kroll::bay_flat(bay.side, bay.held, frame);
            im.stamp(&pic, x, y + T - pic.h);
        } else if r.switch {
            let pic = kroll::switch_flat();
            im.stamp(&pic, x, y + T - pic.h);
        }
    }
}

/// The rollerways: each tile of the way, the goods riding it and a blinking mark over the head of a jam.
/// The goods are drawn a second behind the sim and slide forward through the second, a slot for every
/// beat the tile moved on, so a stream flows and a stood line stands.
fn belts(im: &mut Img, run: &Run, view: &HillView, origin: (i32, i32), size: (i32, i32)) {
    if view.belts.is_empty() {
        return;
    }
    let (frame, frac) = (hill::frame(run), hill::frac(run));
    let on_screen = |t: (i16, i16)| {
        let (x, y) = (i32::from(t.0) * T - origin.0, (i32::from(t.1) + 1) * T - origin.1);
        x > -T && y > -2 * T && x < size.0 && y < size.1 + T
    };
    for b in view.belts.iter().filter(|b| on_screen(b.tile)) {
        let (x, y) = (i32::from(b.tile.0) * T - origin.0, i32::from(b.tile.1) * T - origin.1);
        let pic = kroll::piece_flat(b.piece, b.dir, b.moved > 0, b.crewed, frame);
        im.stamp(&pic, x, y + T - pic.h);
    }
    for b in view.belts.iter().filter(|b| on_screen(b.tile)) {
        let (x, y) = (i32::from(b.tile.0) * T - origin.0, i32::from(b.tile.1) * T - origin.1);
        let back = f32::from(b.moved) * (1.0 - frac);
        let lanes: &[(&[Option<Load>; 3], u8, i32)] = if b.piece == crate::kingdom::Piece::Trestle {
            &[(&b.across, (b.dir + 1) % 4, 0), (&b.slots, b.dir, 4)]
        } else {
            &[(&b.slots, b.dir, 0)]
        };
        for &(slots, dir, up) in lanes {
            for (i, g) in slots.iter().enumerate() {
                let Some(load) = g else { continue };
                let (px, py) = kroll::slot_at(dir, i, back);
                // The first slot's good has come in from behind: not drawn before its tile's edge.
                let pic = kroll::good_flat(*load);
                im.stamp(&pic, x + px as i32 - 3, y + py as i32 - 2 - up);
            }
        }
        if b.stuck >= 8 && view.jams.iter().any(|(t, _)| *t == b.tile) {
            let jam = kroll::jam_flat(frame);
            im.stamp(&jam, x + 2, y - 10);
        }
    }
}

/// The buildings, in two pictures: what stands behind a knight whose feet are at
/// `feet` (pixels down the map) and what stands before them.
pub(super) fn buildings(run: &Run, view: &HillView, origin: (i32, i32), size: (i32, i32), feet: f32) -> (Img, Img) {
    let (mut back, mut front) = (Img::new(size.0, size.1), Img::new(size.0, size.1));
    let frame = hill::frame(run);
    for b in &view.buildings {
        if (b.art.is_empty() && b.plan != "swape") || !seen(b.rect, origin, size) {
            continue;
        }
        let pic = if b.plan == "swape" {
            kroll::swape_flat(b.facing, b.state, frame)
        } else {
            let Some(pic) = kit::flat(b.art, b.state, frame) else { continue };
            pic
        };
        let (x, bottom) = (i32::from(b.rect[0]) * T - origin.0, (i32::from(b.rect[3]) + 1) * T - origin.1);
        let to = if (i32::from(b.rect[3]) + 1) as f32 * 16.0 <= feet + 2.0 { &mut back } else { &mut front };
        to.stamp(&pic, x, bottom - pic.h);
        if b.paused {
            to.text(x + 2, bottom - pic.h + 2, "PAUSED", 'w');
        }
        // A works the Deep has set alight burns over its whole roof.
        if b.status == Status::Fire {
            to.stamp(&kit::blaze(pic.w, pic.h, frame), x, bottom - pic.h);
        }
        // A defence the Deep has battered: cracks, and how much of it holds.
        if let Some((left, max)) = b.hurt {
            to.stamp(&kit::cracks(pic.w, pic.h, u32::from(max - left), u32::from(max)), x, bottom - pic.h);
            let bw = pic.w.clamp(10, 24);
            let (bx, by) = (x + (pic.w - bw) / 2, bottom - pic.h - 4);
            to.rect(bx - 1, by - 1, bw + 2, 4, 'k');
            to.rect(bx, by, bw, 2, 'g');
            to.rect(bx, by, (bw as f32 * f32::from(left) / f32::from(max.max(1))).ceil() as i32, 2, if left * 3 < max { '4' } else { '3' });
        }
    }
    (back, front)
}

/// A picture stood in the world: stamped on the ground's picture in the flat
/// view, stood up as a figure in 3D.
struct Put<'a> {
    over: &'a mut Img,
    figures: Option<&'a mut Vec<Figure>>,
    origin: (i32, i32),
}

impl Put<'_> {
    fn stand(&mut self, im: &Img, (ax, ay): (i32, i32), shadow: bool, bias: f32) {
        self.stand_at(im, (ax, ay), shadow, bias, 0);
    }

    /// `lift` pixels off the ground (a mark over a roof, a bucket on its cable).
    fn stand_at(&mut self, im: &Img, (ax, ay): (i32, i32), shadow: bool, bias: f32, lift: i32) {
        let (fx, fy) = (ax - self.origin.0, ay - self.origin.1);
        match self.figures.as_deref_mut() {
            Some(figures) => {
                let mut img = Img::new(im.w + 4, im.h + 3);
                if shadow {
                    img.ellipse(img.w as f32 / 2.0, im.h as f32 - 0.5, im.w as f32 / 3.0, 1.6, 'k');
                }
                img.stamp(im, 2, 0);
                figures.push(Figure {
                    img,
                    x: ax - im.w / 2 - 2,
                    y: ay - im.h,
                    frac: (0.0, 0.0),
                    depth: ay as f32 / UNIT + bias,
                    height: lift as f32,
                    additive: false,
                    bodies: Vec::new(),
                    foe: false,
                    paints: Vec::new(),
                });
            }
            None => {
                if shadow {
                    self.over.ellipse(fx as f32, fy as f32, im.w as f32 / 3.0, 1.6, 'k');
                }
                self.over.stamp(im, fx - im.w / 2, fy - im.h + 1 - lift);
            }
        }
    }
}

/// Carts, the buckets, wagons and barges on the lines, and the people.
pub(super) fn movers(over: &mut Img, run: &Run, view: &HillView, origin: (i32, i32), size: (i32, i32), figures: Option<&mut Vec<Figure>>) {
    let (frame, frac) = (hill::frame(run), hill::frac(run));
    // In 3D the carts bound for the Keep's yard that are near it stand in the bailey (`world3d::works_yard`): not
    // here on the road too.
    let drawn_as_blocks = figures.is_some();
    let mut put = Put { over, figures, origin };
    let foot = |p: (f32, f32)| ((p.0 * 16.0) as i32, (p.1 * 16.0) as i32 + 4);
    // In 3D the hillside already stands carts, wagons and line riders up as blocks
    // (`world3d::hillside`). A flat figure beside that block shows each one twice.
    if !drawn_as_blocks {
    let relayed: std::collections::BTreeSet<usize> = if crate::world3d::works_yard::shows_real_carts(run) { view.relayed().collect() } else { Default::default() };
    for r in view.carts.iter().enumerate().filter(|(i, _)| !relayed.contains(i)).map(|(_, r)| r) {
        let p = r.place(frac);
        if !in_view(p, origin, size, 24) {
            continue;
        }
        let pic = klines::vehicle_flat(klines::Kind::Cart, r.dir, r.load, if r.moving { frame } else { 0 });
        put.stand(&pic, foot(p), true, 0.02);
    }
    }
    if !drawn_as_blocks {
    for line in &view.lines {
        for r in &line.riders {
            let p = r.place(frac);
            if !in_view(p, origin, size, 24) {
                continue;
            }
            let pic = klines::vehicle_flat(line.kind, r.dir, r.load, if r.moving { frame } else { 0 });
            // A bucket hangs from its cable: a pylon's height up.
            let lift = if line.kind == klines::Kind::Rope { 14 } else { 0 };
            let (fx, fy) = foot(p);
            put.stand(&pic, (fx, fy - lift), line.kind != klines::Kind::Rope, 0.02);
        }
    }
    }
    // The camp's hands at the party's plans: a labourer carrying, a carpenter raising.
    for (i, w) in view.crews.iter().enumerate() {
        for (k, (rider, trade)) in [(&w.labourer, khands::Trade::Labourer), (&w.carpenter, khands::Trade::Carpenter)].into_iter().enumerate() {
            let p = rider.place(frac);
            if !in_view(p, origin, size, 24) {
                continue;
            }
            let act = match (rider.load, w.working, rider.moving) {
                (Some(load), _, _) => khands::Act::Carry(load),
                (_, true, _) if k == 1 => khands::Act::Work,
                (_, _, true) => khands::Act::Walk,
                _ => khands::Act::Idle,
            };
            let step = frame + (i * 3 + k) as u32;
            let hand = khands::hand(trade, act, step, matches!(rider.dir, klines::Dir::West));
            put.stand(&hand.img, ((p.0 * 16.0) as i32, (p.1 * 16.0) as i32 + 6), true, 0.035);
        }
    }
    if !drawn_as_blocks {
    for w in &view.wagons {
        let p = w.rider.place(frac);
        if !in_view(p, origin, size, 24) {
            continue;
        }
        let pic = klines::vehicle_flat(klines::Kind::Tram, w.rider.dir, w.rider.load, if w.rider.moving { frame } else { 0 });
        let (fx, fy) = foot(p);
        put.stand(&pic, (fx, fy), true, 0.02);
        // A wagon held up carries a tag, and a jammed one the red ring.
        match w.state {
            crate::kingdom::WagonState::Queued => put.stand_at(&kroll::waiting_flat(frame), (fx, fy), false, 0.04, 12),
            crate::kingdom::WagonState::Jammed => put.stand_at(&kroll::jam_flat(frame), (fx, fy), false, 0.04, 12),
            _ => {}
        }
    }
    }
    for f in &view.folk {
        let p = f.rider.place(frac);
        if !in_view(p, origin, size, 24) {
            continue;
        }
        let trade = f.trade.unwrap_or(khands::Trade::Labourer);
        let act = if f.working { khands::Act::Work } else if f.rider.moving { khands::Act::Walk } else { khands::Act::Idle };
        let step = frame + f.id;
        let hand = khands::hand(trade, act, step, f.left);
        put.stand(&hand.img, ((p.0 * 16.0) as i32, (p.1 * 16.0) as i32 + 6), true, 0.03);
    }
}

/// Teal: it is laid now. Blue: it is laid as a plan for the camp's hands. Red: the land refuses it.
fn ink_of(v: Verdict) -> char {
    match v {
        Verdict::Fits => '2',
        Verdict::Short => 'z',
        Verdict::Refused => '7',
    }
}

/// The ghost on the ground under the focused knight's builder: its footprint, the
/// piece it would raise (half there, so the ground shows through), the ground a
/// line has crossed. Marks on the ground, in the flat view and in 3D alike.
pub(super) fn ghost(over: &mut Img, run: &Run, origin: (i32, i32), focus: Option<u32>, flat: bool) {
    let Some(seat) = focus else { return };
    let Some(g) = run.hill_ghost(seat) else { return };
    let ink = ink_of(g.verdict);
    let (x0, y0) = (i32::from(g.rect.0) * T - origin.0, i32::from(g.rect.1) * T - origin.1);
    let (w, h) = ((i32::from(g.rect.2 - g.rect.0) + 1) * T, (i32::from(g.rect.3 - g.rect.1) + 1) * T);
    // The footprint: a checker of the verdict's ink, a frame, ticks at the corners.
    for yy in 0..h {
        for xx in 0..w {
            if (xx + yy) % 4 == 0 {
                over.put(x0 + xx, y0 + yy, ink);
            }
        }
    }
    over.frame(x0, y0, w, h, ink);
    over.frame(x0 + 1, y0 + 1, w - 2, h - 2, ink);
    // A piece of rollerway or a swape, in its half-tone, facing the way it would be laid.
    if flat && matches!(&g.tool, Tool::Roller(_) | Tool::Plan("swape")) {
        let pic = match &g.tool {
            Tool::Roller(piece) => kroll::piece_flat(*piece, g.dir, false, true, 0),
            _ => kroll::swape_flat(g.dir, State::Idle, 0),
        };
        let mut ghosted = Img::new(pic.w, pic.h);
        for yy in 0..pic.h {
            for xx in 0..pic.w {
                if (xx + yy) % 2 == 0 && let Some(c) = pic.get(xx, yy) {
                    ghosted.set(xx, yy, c);
                }
            }
        }
        over.stamp(&ghosted, x0, y0 + h - pic.h);
        // The way it faces: an arrow over the tile.
        let (cx, cy) = (x0 + 8, y0 + 8);
        let (dx, dy) = [(0, -1), (1, 0), (0, 1), (-1, 0)][usize::from(g.dir % 4)];
        over.line(cx - dx * 5, cy - dy * 5, cx + dx * 5, cy + dy * 5, 'w');
        over.line(cx + dx * 5, cy + dy * 5, cx + dx * 2 - dy * 3, cy + dy * 2 + dx * 3, 'w');
        over.line(cx + dx * 5, cy + dy * 5, cx + dx * 2 + dy * 3, cy + dy * 2 - dx * 3, 'w');
    }
    // The piece, in the half-tone of a ghost: only if the flat view is the picture.
    if flat && let Tool::Plan(id) = &g.tool {
        let art = hill::art_of(id);
        if let Some(pic) = kit::flat(art, State::Idle, 0) {
            over.stamp(&halftone(&pic), x0, y0 + h - pic.h);
        }
    }
    // A patch being copied: its corners, in dashes.
    if let Some((sx0, sy0, sx1, sy1)) = g.select {
        let (rx, ry) = (i32::from(sx0) * T - origin.0, i32::from(sy0) * T - origin.1);
        let (rw, rh) = ((i32::from(sx1 - sx0) + 1) * T, (i32::from(sy1 - sy0) + 1) * T);
        for i in 0..(rw + rh) * 2 {
            if (i / 3) % 2 != 0 {
                continue;
            }
            let (x, y) = match i {
                i if i < rw => (i, 0),
                i if i < rw + rh => (rw - 1, i - rw),
                i if i < 2 * rw + rh => (2 * rw + rh - 1 - i, rh - 1),
                i => (0, 2 * (rw + rh) - 1 - i),
            };
            if (0..rw).contains(&x) && (0..rh).contains(&y) {
                over.put(rx + x, ry + y, 'w');
            }
        }
    }
    // A line in the making: the tiles it has crossed.
    for (i, t) in g.path.iter().enumerate() {
        let (tx, ty) = (i32::from(t.0) * T - origin.0, i32::from(t.1) * T - origin.1);
        over.rect(tx + 5, ty + 5, 6, 6, if i % 2 == 0 { '5' } else { '4' });
    }
    // Where the cursor is: a cross on its tile.
    let (cx, cy) = (i32::from(g.cursor.0) * T - origin.0 + 8, i32::from(g.cursor.1) * T - origin.1 + 8);
    over.line(cx - 3, cy, cx + 3, cy, 'w');
    over.line(cx, cy - 3, cx, cy + 3, 'w');
}

/// A kit piece in the half-tone of a ghost: every other pixel, so the ground shows through.
fn halftone(pic: &Img) -> Img {
    let mut ghosted = Img::new(pic.w, pic.h);
    for yy in 0..pic.h {
        for xx in 0..pic.w {
            if (xx + yy) % 2 == 0 && let Some(c) = pic.get(xx, yy) {
                ghosted.set(xx, yy, c);
            }
        }
    }
    ghosted
}

/// The party's plans on the ground, whoever laid them: the footprint in blue while the hands
/// can take it, amber while the camp lacks the goods, teal as a crew carries and raises it,
/// the piece in the half-tone of a ghost, its urgency marked, and a bar for the raising.
pub(super) fn plans(over: &mut Img, run: &Run, view: &HillView, origin: (i32, i32), size: (i32, i32), flat: bool) {
    let tick = run.tick as i32;
    for p in &view.plans {
        if !seen(p.rect, origin, size) {
            continue;
        }
        let (x0, y0) = (i32::from(p.rect[0]) * T - origin.0, i32::from(p.rect[1]) * T - origin.1);
        let (w, h) = ((i32::from(p.rect[2] - p.rect[0]) + 1) * T, (i32::from(p.rect[3] - p.rect[1]) + 1) * T);
        let ink = if p.raising.is_some() || p.fetched {
            '3'
        } else if p.wait.is_some() || p.deep {
            '4'
        } else {
            'z'
        };
        for yy in 0..h {
            for xx in 0..w {
                if (xx + yy) % 5 == 0 {
                    over.put(x0 + xx, y0 + yy, ink);
                }
            }
        }
        // A frame that marches while the hands are on their way: dashes, moving.
        for i in 0..(w + h) * 2 {
            let on = (i / 3 + if p.fetched { tick / 4 } else { 0 }) % 2 == 0;
            if !on && p.wait.is_none() {
                continue;
            }
            let (x, y) = match i {
                i if i < w => (i, 0),
                i if i < w + h => (w - 1, i - w),
                i if i < 2 * w + h => (2 * w + h - 1 - i, h - 1),
                i => (0, 2 * (w + h) - 1 - i),
            };
            if (0..w).contains(&x) && (0..h).contains(&y) {
                over.put(x0 + x, y0 + y, ink);
            }
        }
        if flat && !p.art.is_empty() {
            if let Some(pic) = kit::flat(p.art, State::Idle, 0) {
                // Rising: the piece stands as high as the hands have got it.
                let part = p.raising.map_or(pic.h, |f| ((pic.h as f32 * f).ceil() as i32).clamp(2, pic.h));
                let ghosted = halftone(&pic);
                let mut shown = Img::new(ghosted.w, part);
                shown.stamp(&ghosted, 0, part - ghosted.h);
                over.stamp(&shown, x0, y0 + h - part);
            }
        }
        // What the Deep broke: a splintered cross over the place, for the hands (or a knight) to set right.
        if p.deep {
            let (cx, cy) = (x0 + w / 2, y0 + h / 2);
            let r = (w.min(h) / 3).max(3);
            over.line(cx - r, cy - r, cx + r, cy + r, 'k');
            over.line(cx - r, cy + r, cx + r, cy - r, 'k');
            over.line(cx - r + 1, cy - r, cx + r + 1, cy + r, '4');
            over.line(cx - r + 1, cy + r, cx + r + 1, cy - r, '4');
        }
        match p.priority {
            2 => {
                over.rect(x0 + 1, y0 + 1, 7, 9, 'k');
                over.text(x0 + 2, y0 + 2, "!", '7');
            }
            0 => {
                over.rect(x0 + 1, y0 + 1, 7, 9, 'k');
                over.text(x0 + 2, y0 + 2, "-", 'v');
            }
            _ => {}
        }
        if let Some(f) = p.raising {
            let bw = (w - 4).max(4);
            over.rect(x0 + 2, y0 + h - 4, bw, 3, 'k');
            over.rect(x0 + 2, y0 + h - 4, (bw as f32 * f) as i32, 3, '3');
        } else if p.wait.is_some() && w >= 12 {
            // Waiting on goods: three dots at its middle.
            for d in 0..3 {
                over.rect(x0 + w / 2 - 5 + d * 4, y0 + h / 2 - 1, 2, 2, if (tick / 10 + d) % 3 == 0 { 'w' } else { '4' });
            }
        }
    }
}

/// The flow overlay: arrows between the works that goods lately moved between,
/// as thick as the throughput, red where something starves and amber where it
/// backs up (a mark over the works, and its words for the nearest), and the
/// Clerk of Works' line.
/// Returns the boxes of the marks and their words in the view's pixels, which the goal's arrow keeps clear of.
pub(super) fn overlay(over: &mut Img, figures: Option<&mut Vec<Figure>>, run: &Run, view: &HillView, origin: (i32, i32), size: (i32, i32), focus: Option<u32>) -> Vec<(i32, i32, i32, i32)> {
    let mut boxes = Vec::new();
    let feet = focus.and_then(|id| run.players.get(&id)).map(|h| (h.x / TILE_UNITS, h.y / TILE_UNITS));
    if !flowing(run, focus) {
        // In 3D a bay stands named without the overlay: its plate over it, for the bays near the knight.
        if figures.is_some()
            && let Some(f) = feet
        {
            let mut put = Put { over, figures, origin };
            bay_plates(&mut put, view, origin, size, Some(f));
        }
        return boxes;
    }
    let tick = run.tick as i32;
    for a in &view.arrows {
        if !in_view(a.from, origin, size, 80) && !in_view(a.to, origin, size, 80) {
            continue;
        }
        let ink = match a.heat {
            Heat::Flowing => '3',
            Heat::Starved => '7',
            Heat::Backed => '4',
        };
        let (fx, fy) = ((a.from.0 * 16.0) as i32 - origin.0, (a.from.1 * 16.0) as i32 - origin.1);
        let (tx, ty) = ((a.to.0 * 16.0) as i32 - origin.0, (a.to.1 * 16.0) as i32 - origin.1);
        let (dx, dy) = ((tx - fx) as f32, (ty - fy) as f32);
        let len = dx.hypot(dy).max(1.0);
        let (ux, uy) = (dx / len, dy / len);
        for k in 0..i32::from(a.weight) {
            let off = k - i32::from(a.weight) / 2;
            let (ox, oy) = ((-uy * off as f32).round() as i32, (ux * off as f32).round() as i32);
            over.line(fx + ox, fy + oy, tx + ox, ty + oy, ink);
        }
        // Chevrons running along it, so a flow reads as moving.
        let step = 14.0;
        let phase = (tick as f32 / 3.0) % step;
        let mut s = phase;
        while s < len - 4.0 {
            let (cx, cy) = (fx as f32 + ux * s, fy as f32 + uy * s);
            let back = (cx - ux * 3.0, cy - uy * 3.0);
            over.line(back.0 as i32 - (uy * 3.0) as i32, back.1 as i32 + (ux * 3.0) as i32, cx as i32, cy as i32, 'w');
            over.line(back.0 as i32 + (uy * 3.0) as i32, back.1 as i32 - (ux * 3.0) as i32, cx as i32, cy as i32, 'w');
            s += step;
        }
    }
    // The marks stand over the works, so a roof does not hide them (in 3D they are figures).
    let mut put = Put { over, figures, origin };
    // Every bay by its name, on a small plate over its platform.
    bay_plates(&mut put, view, origin, size, None);
    let mut near: Vec<(f32, &hill::Mark)> = Vec::new();
    for m in &view.marks {
        if !in_view(m.at, origin, size, 24) {
            continue;
        }
        let at = ((m.at.0 * 16.0) as i32, (m.at.1 * 16.0) as i32);
        let ring = mark_img(m.heat, tick);
        boxes.push((at.0 - origin.0 - ring.w / 2, at.1 - origin.1 - ring.h + 1 - 34, ring.w, ring.h));
        put.stand_at(&ring, at, false, 0.5, 34);
        if let Some(f) = feet {
            near.push(((m.at.0 - f.0).hypot(m.at.1 - f.1), m));
        }
    }
    // The words of the two nearest, on dark plates over their marks.
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut plates: Vec<(i32, i32, i32, i32)> = Vec::new();
    for (_, m) in near.into_iter().take(2).filter(|(d, _)| *d < 14.0) {
        let w = crate::render::overworld::ink::text_width(&m.words);
        let mut plate = Img::new(w + 6, 12);
        plate.rect(0, 0, w + 6, 12, 'k');
        plate.frame(0, 0, w + 6, 12, if m.heat == Heat::Starved { '7' } else { '4' });
        plate.text(3, 2, &m.words, 'w');
        // Over its mark, but never off the view's side, and never on the other mark's words:
        // raised a line at a time until it is clear.
        let at = ((m.at.0 * 16.0) as i32, (m.at.1 * 16.0) as i32);
        let middle = (at.0 - origin.0).clamp(4 + plate.w / 2, (size.0 - 4 - plate.w / 2).max(4 + plate.w / 2));
        let mut lift = 50;
        let mut rect = (middle - plate.w / 2, at.1 - origin.1 - plate.h + 1 - lift, plate.w, plate.h);
        for _ in 0..3 {
            if !plates.iter().any(|p| p.0 < rect.0 + rect.2 && rect.0 < p.0 + p.2 && p.1 < rect.1 + rect.3 && rect.1 < p.1 + p.3) {
                break;
            }
            lift += 14;
            rect.1 -= 14;
        }
        plates.push(rect);
        boxes.push(rect);
        put.stand_at(&plate, (middle + origin.0, at.1), false, 0.55, lift);
    }
    boxes
}

/// Every bay by its name, on a small plate over its platform; with `near` (a knight's tile) only the bays
/// within nine tiles of it.
fn bay_plates(put: &mut Put, view: &HillView, origin: (i32, i32), size: (i32, i32), near: Option<(f32, f32)>) {
    for r in &view.rails {
        let Some(bay) = &r.bay else { continue };
        let at = (f32::from(r.tile.0) + 0.5, f32::from(r.tile.1) + 0.5);
        if !in_view(at, origin, size, 24) || near.is_some_and(|f| (at.0 - f.0).hypot(at.1 - f.1) > 9.0) {
            continue;
        }
        let name = bay.name.to_uppercase();
        let w = crate::render::overworld::ink::text_width(&name);
        let mut plate = Img::new(w + 4, 9);
        plate.rect(0, 0, w + 4, 9, 'k');
        plate.frame(0, 0, w + 4, 9, if bay.held { '4' } else { 'v' });
        plate.text(2, 1, &name, 'w');
        put.stand_at(&plate, ((at.0 * 16.0) as i32, (at.1 * 16.0) as i32), false, 0.45, 20);
    }
}

/// Whether the ledger (Charters, Claims, People, Works) is open over the world for `focus`:
/// it covers the view, and the rest of the HUD stands down.
pub(in super::super) fn ledger_open(run: &Run, focus: Option<u32>) -> bool {
    run.sim_hillside() && focus.and_then(|id| run.kingdom.industry.get(&id)).is_some_and(|b| b.open && b.page != 0)
}

fn flowing(run: &Run, focus: Option<u32>) -> bool {
    focus.and_then(|id| run.kingdom.industry.get(&id)).is_some_and(|b| b.overlay && b.open)
}

/// A mark over a works: a ring of its heat with a bang (red, starved) or a bar of stock (amber, backed up).
fn mark_img(heat: Heat, tick: i32) -> Img {
    let ink = if heat == Heat::Starved { '7' } else { '4' };
    let mut im = Img::new(15, 15);
    let bob = (tick / 8) % 2;
    im.ellipse(7.5, 7.5 + bob as f32 * 0.0, 6.5, 6.5, ink);
    im.ellipse(7.5, 7.5, 5.0, 5.0, 'k');
    if heat == Heat::Starved {
        im.rect(7, 3, 1, 6, 'w');
        im.put(7, 10, 'w');
    } else {
        for y in [4, 7, 10] {
            im.rect(4, y, 7, 2, '4');
        }
    }
    im
}

/// What the Clerk says when the sim has no complaint to make of the Keep's chain: where the
/// works begin. Stone is won below first, then quarried, and only then is the Keep fed.
fn idle_clerk(run: &Run) -> &'static str {
    let k = &run.home.industry;
    if !k.claims.values().any(|c| c.secured) {
        "NO STONE YET. WIN A SEAM IN THE DELVE AND CARRY A BLOCK UP"
    } else if !k.buildings.values().any(|b| b.plan == "claim-quarry") {
        "A SEAM IS WON. BUILD A QUARRY ON IT"
    } else {
        "NOTHING HOLDS THE KEEP UP"
    }
}

/// The Clerk of Works' line, in the screen's marks, while the overlay is on.
pub(in super::super) fn clerk(ui: &mut Img, run: &Run, view: &HillView, focus: Option<u32>) {
    if !flowing(run, focus) {
        return;
    }
    let said = if view.clerk.is_empty() { format!("CLERK: {}", idle_clerk(run)) } else { format!("CLERK: {}", view.clerk) };
    // Up to three lines over the builder's plate, each broken at a word to what a line of the
    // view holds (the plate is as wide as its text, and the text never passes the view's edge).
    let room = NATIVE_W - 20;
    let mut lines: Vec<String> = vec![String::new()];
    for word in said.split_whitespace() {
        let last = lines.last_mut().expect("a line");
        if !last.is_empty() && crate::render::overworld::ink::text_width(&format!("{last} {word}")) > room {
            lines.push(word.to_string());
        } else {
            if !last.is_empty() {
                last.push(' ');
            }
            last.push_str(word);
        }
    }
    if lines.len() > 3 {
        // More than three lines: the third ends in two dots, not in the view's edge.
        lines.truncate(3);
        let third = lines.last_mut().expect("a line");
        while crate::render::overworld::ink::text_width(third) > room - 12 {
            match third.rfind(' ') {
                Some(at) => third.truncate(at),
                None => break,
            }
        }
        third.push_str("..");
    }
    let top = NATIVE_H - STRIP_H - 2 - 12 * lines.len() as i32 - 2;
    for (i, line) in lines.iter().enumerate() {
        let w = crate::render::overworld::ink::text_width(line);
        super::hud_log::note("clerk", (4, top + i as i32 * 12, w + 6, 12));
        ui.rect(4, top + i as i32 * 12, w + 6, 12, 'k');
        ui.text(7, top + 2 + i as i32 * 12, line, 'T');
    }
}

/// The night's plate, at the view's top left: how the realm's day stands, and the raid if one is on.
pub(super) fn deep_plate(ui: &mut Img, run: &Run, focus: Option<u32>, avoid: &[(i32, i32, i32, i32)]) -> Option<(i32, i32, i32, i32)> {
    if !run.sim_hillside() || !crate::world::plan::tabula() || ledger_open(run, focus) || !run.hill_world() {
        return None;
    }
    let (night, left) = run.deep_clock();
    let clock = format!("{} {}:{:02}", if night { "NIGHT. DAWN IN" } else { "DAY. DUSK IN" }, left / 60, left % 60);
    let mut lines: Vec<(String, char)> = vec![(clock, if night { 'z' } else { 'i' })];
    // What the castle that stands calms: how far, and how much at the camp.
    if let Some(site) = run.sites.keep() {
        let c = crate::kingdom::catalog();
        let calm = run.home.industry.calm_at(c, run.home.industry.camp());
        if calm > 0 {
            lines.push((format!("KEEP CALMS {} TILES: -{calm} STIR", crate::kingdom::pressure::calm_radius(site.laid)), 'i'));
            let yard = run.home.industry.yard_stone();
            if yard > 0 {
                lines.push((format!("YARD: {yard} STONE"), 'Y'));
            }
        }
    }
    if let Some((theme, alive, size)) = run.raid_status() {
        lines.push((format!("RAID: {} ({alive} OF {size})", theme.name()), '4'));
    }
    let tw = crate::render::overworld::ink::text_width;
    let w = lines.iter().map(|(l, _)| tw(l)).max().unwrap_or(40) + 10;
    let h = lines.len() as i32 * 9 + 5;
    // The goal plate and the corner already own the top. Sit under them, and
    // step down if a tall plate still crosses this band. The card and the
    // goal's arrow are told this rect, so they stand clear of it.
    let overlaps = |x, y| {
        avoid.iter().any(|&(a, b, aw, ah)| x < a + aw && x + w > a && y < b + ah && y + h > b)
    };
    let mut y = 40;
    while overlaps(5, y) && y + h < NATIVE_H - STRIP_H - 16 {
        y += 2;
    }
    if overlaps(5, y) {
        return None;
    }
    let rect = (5, y, w, h);
    super::hud_log::note("deep-plate", rect);
    ui.rect(rect.0, rect.1, w, h, 'K');
    for (i, (l, ink)) in lines.iter().enumerate() {
        ui.text(rect.0 + 4, rect.1 + 3 + i as i32 * 9, l, *ink);
    }
    Some(rect)
}

/// The height of the builder's strip at the view's foot.
pub(in super::super) const STRIP_H: i32 = 52;

/// The builder's strip, in the screen's marks: the hotbar (nine slots in three banks, the
/// number keys choosing within the bank in hand), the tool in hand and whether it lays
/// plans, what the ghost costs and whether the land suits it, and the last thing said.
pub(super) fn hud(ui: &mut Img, run: &Run, focus: Option<u32>) {
    let Some(seat) = focus else { return };
    let Some(board) = run.kingdom.industry.get(&seat) else { return };
    if !build::building(run, seat) {
        return;
    }
    let all = build::tools();
    let at = board.plan % all.len();
    let g: Option<Ghost> = run.hill_ghost(seat);
    let (w, h) = (NATIVE_W, STRIP_H);
    let top = NATIVE_H - h - 2;
    super::hud_log::note("builder", (0, top, w, h));
    ui.rect(0, top, w, h, 'k');
    ui.frame(0, top, w, h, 'g');
    let tw = crate::render::overworld::ink::text_width;
    // The hotbar: the bank in hand as three slots, the other banks as tabs (up and down change them).
    let rig = &board.rig;
    let bank = rig.bank();
    let slot_w = 97;
    for i in 0..builder_rig::BANK {
        let slot = bank * builder_rig::BANK + i;
        let x = 4 + i as i32 * (slot_w + 2);
        let tool = &all[usize::from(rig.bar[slot]).min(all.len() - 1)];
        let held = slot == usize::from(rig.slot);
        if held {
            ui.rect(x, top + 3, slot_w, 12, 'g');
        }
        ui.frame(x, top + 3, slot_w, 12, if held { 'Y' } else { 'J' });
        ui.text(x + 3, top + 5, &(i + 1).to_string(), 'T');
        let name: String = tool.name().chars().take(((slot_w - 14) / crate::render::overworld::ink::GLYPH_ADVANCE) as usize).collect();
        ui.text(x + 11, top + 5, &name, if held { 'w' } else { 'i' });
    }
    ui.text(4 + 3 * (slot_w + 2) + 4, top + 5, "BANK", 'G');
    for b in 0..builder_rig::SLOTS / builder_rig::BANK {
        let x = 4 + 3 * (slot_w + 2) + 34 + b as i32 * 14;
        ui.rect(x, top + 3, 12, 12, if b == bank { 'g' } else { 'k' });
        ui.frame(x, top + 3, 12, 12, if b == bank { 'Y' } else { 'g' });
        ui.text(x + 4, top + 5, ["A", "B", "C"][b], if b == bank { 'w' } else { 'G' });
    }
    let current = all[at].name();
    ui.text(6, top + 18, "BUILD", 'Y');
    ui.text(42, top + 18, &current, 'w');
    let mut x = 42 + tw(&current) + 8;
    if rig.planning {
        ui.text(x, top + 18, "PLANNING", 'z');
        x += tw("PLANNING") + 8;
    }
    if let Some(m) = rig.mark {
        let t = format!("COPYING FROM {},{}", m.0, m.1);
        ui.text(x, top + 18, &t, 'w');
    }
    let ink = g.as_ref().map_or('w', |g| match g.verdict {
        Verdict::Fits => '3',
        Verdict::Short => 'z',
        Verdict::Refused => 'Y',
    });
    let keys = "Q TOOL  E LAY  Z UNDO";
    let more = "H CARD  C LEDGER";
    ui.text(NATIVE_W - tw(keys) - 6, top + 18, keys, 'v');
    if let Some(g) = &g {
        // One line of the strip, never cut off mid-word: too long, it ends in two dots.
        let room = ((NATIVE_W - 12) / crate::render::overworld::ink::GLYPH_ADVANCE) as usize;
        let note: String = if g.note.chars().count() > room { g.note.chars().take(room - 2).collect::<String>() + ".." } else { g.note.clone() };
        ui.text(6, top + 29, &note, ink);
    }
    // The last thing said, on the foot line, never under the keys at its right and never
    // the ghost's own note over again.
    let free = ((NATIVE_W - tw(more) - 30) / crate::render::overworld::ink::GLYPH_ADVANCE).max(8) as usize;
    let note_said = g.as_ref().is_some_and(|g| g.note.eq_ignore_ascii_case(&board.message));
    if !note_said {
        let said: String = if board.message.chars().count() > free { board.message.chars().take(free - 2).collect::<String>() + ".." } else { board.message.clone() };
        ui.text(6, top + 40, &said, 't');
    }
    ui.text(NATIVE_W - tw(more) - 6, top + 40, more, 'v');
    let _ = Status::Idle;
}

/// `text` broken at words into lines no wider than `px` pixels.
fn wrap(text: &str, px: i32) -> Vec<String> {
    let mut lines: Vec<String> = vec![String::new()];
    for word in text.split_whitespace() {
        let last = lines.last_mut().expect("a line");
        if !last.is_empty() && crate::render::overworld::ink::text_width(&format!("{last} {word}")) > px {
            lines.push(word.to_string());
        } else {
            if !last.is_empty() {
                last.push(' ');
            }
            last.push_str(word);
        }
    }
    lines
}

/// `lines` cut to `n`, the last ending in two dots when more were cut.
fn cut(mut lines: Vec<String>, n: usize, px: i32) -> Vec<String> {
    if lines.len() > n {
        lines.truncate(n);
        let last = lines.last_mut().expect("a line");
        while crate::render::overworld::ink::text_width(last) > px - 12 {
            match last.rfind(' ') {
                Some(at) => last.truncate(at),
                None => break,
            }
        }
        last.push_str("..");
    }
    lines
}

fn tone_ink(t: crate::kingdom::Tone) -> char {
    match t {
        crate::kingdom::Tone::Plain => 'w',
        crate::kingdom::Tone::Good => '3',
        crate::kingdom::Tone::Warn => '4',
        crate::kingdom::Tone::Bad => '%',
    }
}

/// The card of what the cursor points at: what it makes, in and out a minute over the last
/// few, its stock, power and crew, and what it waits for. On a plate at the side of the view
/// away from the cursor, under the corner blocks. Returns its rectangle.
pub(super) fn card(ui: &mut Img, run: &Run, focus: Option<u32>, origin: (i32, i32), clear: &[(i32, i32, i32, i32)]) -> Option<(i32, i32, i32, i32)> {
    let seat = focus?;
    let board = run.kingdom.industry.get(&seat)?;
    if !board.open || board.page != 0 || !run.sim_hillside() {
        return None;
    }
    let card = board.rig.numbers.as_ref()?.card.as_ref()?;
    let (w, label_w) = (190, 54);
    let text_w = w - label_w - 10;
    let mut body: Vec<(String, String, char)> = Vec::new();
    for r in &card.rows {
        let lines = cut(wrap(&r.text.to_uppercase(), text_w), 4, text_w);
        for (i, line) in lines.into_iter().enumerate() {
            body.push((if i == 0 { r.label.clone() } else { String::new() }, line, tone_ink(r.tone)));
        }
    }
    // The side the cursor is not on; on the right it has the view's top (the goal gives way to it),
    // on the left it goes under the corner block.
    let cursor_x = i32::from(board.tile.0) * T + 8 - origin.0;
    let right = cursor_x < NATIVE_W / 2;
    let x = if right { NATIVE_W - w - 5 } else { 5 };
    let y = if right { 5 } else { clear.iter().filter(|r| r.0 < x + w && x < r.0 + r.2).map(|r| r.1 + r.3 + 4).fold(5, i32::max) };
    // As many lines as fit above the builder's strip (and the Clerk's lines over it).
    let floor = NATIVE_H - STRIP_H - 2 - 4 - if board.overlay { 38 } else { 0 };
    let fit = (((floor - y - 18) / 9).max(3)) as usize;
    if body.len() > fit {
        body.truncate(fit);
        if let Some(last) = body.last_mut() {
            last.1 = cut(vec![format!("{} ..", last.1)], 1, text_w).remove(0);
        }
    }
    let h = 14 + body.len() as i32 * 9 + 4;
    let rect = (x, y, w, h);
    super::hud_log::note("card", rect);
    ui.rect(x, y, w, h, 'k');
    ui.frame(x, y, w, h, 'G');
    ui.text(x + 5, y + 3, &card.title.to_uppercase(), 'Y');
    let kind = crate::render::overworld::ink::text_width(&card.kind);
    ui.text(x + w - kind - 5, y + 3, &card.kind, 'G');
    ui.rect(x + 3, y + 12, w - 6, 1, 'g');
    for (i, (label, line, ink)) in body.iter().enumerate() {
        let ly = y + 16 + i as i32 * 9;
        ui.text(x + 5, ly, label, 'v');
        ui.text(x + 5 + label_w, ly, line, *ink);
    }
    Some(rect)
}

/// What to do now and why it is worth it, always up at the view's top right: the next
/// milestone's name, how, and why. Returns its rectangle.
pub(super) fn goal_plate(ui: &mut Img, run: &Run, focus: Option<u32>) -> Option<(i32, i32, i32, i32)> {
    if !run.sim_hillside() || !crate::world::plan::tabula() || ledger_open(run, focus) {
        return None;
    }
    // The card of what the cursor points at takes the view's top right while it is up.
    if focus.and_then(|id| run.kingdom.industry.get(&id)).is_some_and(|b| b.open && b.page == 0 && b.rig.numbers.as_ref().is_some_and(|n| n.card.is_some())) {
        return None;
    }
    let goals = run.home.industry.goals(crate::kingdom::catalog());
    let m = goals.next()?;
    let w = 190;
    let mut lines: Vec<(String, char)> = vec![(format!("GOAL {}", m.name.to_uppercase()), 'Y')];
    let how = cut(wrap(&m.how, w - 10), 2, w - 10);
    lines.extend(how.into_iter().map(|l| (l, 'i')));
    let why = cut(wrap(&format!("WHY: {}", m.why), w - 10), 2, w - 10);
    lines.extend(why.into_iter().map(|l| (l, 'G')));
    let title: String = lines[0].0.chars().take(31).collect();
    lines[0].0 = title;
    let h = lines.len() as i32 * 9 + 5;
    let rect = (NATIVE_W - w - 5, 5, w, h);
    super::hud_log::note("goal-plate", rect);
    ui.rect(rect.0, rect.1, w, h, 'K');
    for (i, (l, ink)) in lines.iter().enumerate() {
        ui.text(rect.0 + 4, rect.1 + 3 + i as i32 * 9, l, *ink);
    }
    if m.need > 0 {
        // How far along a counted goal is.
        let bar = ((w - 8) as f32 * (m.have.min(m.need) as f32 / m.need as f32)) as i32;
        ui.rect(rect.0 + 4, rect.1 + h - 3, w - 8, 1, 'g');
        ui.rect(rect.0 + 4, rect.1 + h - 3, bar, 1, '3');
    }
    Some(rect)
}

/// What a flow overlay or a ghost needs of the view this frame.
pub(super) fn rider_at(r: &Rider, frac: f32) -> (f32, f32) {
    r.place(frac)
}

/// The ledger over the world: Charters, Claims, People and Works as a panel laid
/// over the walking view (the desk's pages, kept; the knight is still in the world
/// behind it). Native-size text, a window of rows round the selection.
pub(super) fn ledger(ui: &mut Img, run: &Run, focus: Option<u32>) {
    let Some(board) = focus.and_then(|id| run.kingdom.industry.get(&id)) else { return };
    if !board.open || board.page == 0 || !run.sim_hillside() {
        return;
    }
    let k = &run.home.industry;
    let c = crate::kingdom::catalog();
    let report = k.report(c);
    let (px, py, pw, ph) = (10, 6, NATIVE_W - 20, NATIVE_H - 12);
    super::hud_log::note("ledger", (px, py, pw, ph));
    ui.rect(px, py, pw, ph, 'k');
    ui.frame(px, py, pw, ph, 'G');
    // The tabs.
    let tabs = [(1u8, "CHARTERS"), (3, "CLAIMS"), (6, "PEOPLE"), (8, "WORKS"), (build::PAGE_GOALS, "GOALS"), (build::PAGE_PRODUCTION, "OUTPUT"), (build::PAGE_CHOKES, "CHOKES")];
    let mut x = px + 8;
    for (page, name) in tabs {
        ui.text(x, py + 5, name, if board.page == page { 'Y' } else { 'G' });
        x += crate::render::overworld::ink::text_width(name) + 9;
    }
    ui.rect(px + 4, py + 15, pw - 8, 1, 'g');
    let keys = "P PAGE  C BACK";
    ui.text(px + pw - crate::render::overworld::ink::text_width(keys) - 6, py + ph - 12, keys, 'v');
    if matches!(board.page, build::PAGE_GOALS | build::PAGE_PRODUCTION | build::PAGE_CHOKES) {
        screens(ui, run, board, (px, py, pw, ph));
        return;
    }
    let rows: Vec<(String, String, char)> = match board.page {
        1 => c
            .hill_charters()
            .into_iter()
            .map(|(id, t)| {
                let (state, ink) = if k.researched.contains(id) {
                    ("RESEARCHED".to_string(), 'G')
                } else if t.requires.iter().all(|r| k.researched.contains(r)) {
                    let price = t.cost.iter().map(|(r, n)| format!("{n} {}", r.to_uppercase())).collect::<Vec<_>>().join(" ");
                    (format!("E FUNDS: {price}"), 'w')
                } else {
                    ("NEEDS ".to_string() + &t.requires.iter().filter(|r| !k.researched.contains(*r)).map(|r| r.to_uppercase()).collect::<Vec<_>>().join(" "), 't')
                };
                (t.name.to_uppercase(), state, ink)
            })
            .collect(),
        3 => k
            .claims
            .iter()
            .map(|(key, q)| {
                let linked = k.buildings.values().any(|b| b.claim.as_deref() == Some(key.as_str()));
                (q.name.to_uppercase(), format!("{} {}", q.resource.to_uppercase(), if linked { "WORKED" } else if q.secured { "WON: E AT A QUARRY" } else { "UNWON" }), if q.secured { 'w' } else { 't' })
            })
            .collect(),
        6 => report
            .population
            .citizens
            .iter()
            .map(|p| {
                let post = p.job.or(p.waiting).and_then(|j| k.buildings.get(&j)).map_or("NO POST".to_string(), |b| c.plans[&b.plan].name.to_uppercase());
                (p.name.to_uppercase(), format!("{}  {}", p.activity.label().to_uppercase(), post), 'w')
            })
            .collect(),
        _ => k
            .buildings
            .iter()
            .map(|(id, b)| {
                let status = report.status.get(id).map_or("", |s| s.label());
                (format!("{} {}", c.plans[&b.plan].name.to_uppercase(), if b.paused { "(PAUSED)" } else { "" }), format!("{status}  CREW {}", report.population.assigned.get(id).copied().unwrap_or(0)), 'w')
            })
            .collect(),
    };
    let rows_shown = 12usize;
    let selected = board.selection.min(rows.len().saturating_sub(1));
    let start = selected.saturating_sub(rows_shown / 2).min(rows.len().saturating_sub(rows_shown));
    if rows.is_empty() {
        ui.text(px + 10, py + 24, "NOTHING YET", 't');
    }
    for (i, (name, state, ink)) in rows.iter().enumerate().skip(start).take(rows_shown) {
        let y = py + 21 + (i - start) as i32 * 13;
        if i == selected {
            ui.rect(px + 4, y - 2, pw - 8, 12, 'g');
        }
        let name: String = name.chars().take(30).collect();
        // What the panel's right edge leaves after the column the states start at.
        let state: String = state.chars().take(((pw - 190 - 10) / crate::render::overworld::ink::GLYPH_ADVANCE) as usize).collect();
        ui.text(px + 8, y, &name, if i == selected { 'Y' } else { 'w' });
        ui.text(px + 190, y, &state, *ink);
    }
    let said: String = board.message.chars().take(38).collect();
    ui.text(px + 8, py + ph - 12, &said, 't');
}

/// The ledger's goals, output and chokes pages.
fn screens(ui: &mut Img, run: &Run, board: &crate::rules::industry::Board, (px, py, pw, ph): (i32, i32, i32, i32)) {
    let c = crate::kingdom::catalog();
    let tw = crate::render::overworld::ink::text_width;
    let said: String = board.message.chars().take(38).collect();
    ui.text(px + 8, py + ph - 12, &said, 't');
    match board.page {
        build::PAGE_GOALS => {
            let goals = run.home.industry.goals(c);
            let selected = board.selection.min(goals.nodes.len().saturating_sub(1));
            let rows = 11usize;
            let start = selected.saturating_sub(rows / 2).min(goals.nodes.len().saturating_sub(rows));
            for (i, n) in goals.nodes.iter().enumerate().skip(start).take(rows) {
                let y = py + 20 + (i - start) as i32 * 11;
                if i == selected {
                    ui.rect(px + 4, y - 2, pw - 8, 11, 'g');
                }
                let is_next = goals.next == Some(i);
                let (mark, ink) = if n.done { ("X", 'G') } else if is_next { (">", 'Y') } else if n.after.iter().all(|a| goals.nodes.iter().any(|m| &m.id == a && m.done)) { ("-", 'w') } else { (".", 't') };
                let indent = i32::from(n.depth.min(6)) * 5;
                ui.text(px + 8 + indent, y, mark, ink);
                let name: String = n.name.to_uppercase().chars().take(((pw - 70 - indent) / crate::render::overworld::ink::GLYPH_ADVANCE) as usize).collect();
                ui.text(px + 18 + indent, y, &name, if n.done { 'G' } else { ink });
                if n.need > 0 && !n.done {
                    let t = format!("{}/{}", n.have, n.need);
                    ui.text(px + pw - tw(&t) - 8, y, &t, 'z');
                } else if n.done {
                    ui.text(px + pw - tw("DONE") - 8, y, "DONE", 'G');
                }
            }
            // What the one selected opens, and what to do.
            if let Some(n) = goals.nodes.get(selected) {
                let y = py + 20 + rows as i32 * 11 + 3;
                ui.rect(px + 4, y - 2, pw - 8, 1, 'g');
                let opens = if n.unlocks.is_empty() { String::new() } else { format!("OPENS: {}", n.unlocks.join(", ")) };
                let mut at = y + 2;
                for (text, ink) in [(n.how.clone(), 'i'), (format!("WHY: {}", n.why), 'G'), (opens, 'z')] {
                    for line in cut(wrap(&text, pw - 16), 1, pw - 16) {
                        ui.text(px + 8, at, &line, ink);
                        at += 9;
                    }
                }
            }
        }
        build::PAGE_PRODUCTION => {
            let Some(p) = board.rig.numbers.as_ref().and_then(|n| n.production.as_ref()) else {
                ui.text(px + 8, py + 24, "READING THE BOOKS...", 't');
                return;
            };
            if p.goods.is_empty() {
                ui.text(px + 8, py + 24, "NOTHING MADE OR USED YET: THE BOOKS CLOSE A MINUTE AT A TIME", 't');
                return;
            }
            ui.text(px + 8, py + 20, "GOOD", 'v');
            ui.text(px + 118, py + 20, "MADE/H", 'v');
            ui.text(px + 170, py + 20, "USED/H", 'v');
            ui.text(px + 222, py + 20, "NOW", 'v');
            let selected = board.selection.min(p.goods.len() - 1);
            for (i, g) in p.goods.iter().enumerate().take(7) {
                let y = py + 30 + i as i32 * 10;
                if i == selected {
                    ui.rect(px + 4, y - 1, pw - 8, 10, 'g');
                }
                let ink = if i == selected { 'Y' } else { 'w' };
                let name: String = g.name.to_uppercase().chars().take(18).collect();
                ui.text(px + 8, y, &name, ink);
                ui.text(px + 118, y, &g.made_hour.to_string(), '3');
                ui.text(px + 170, y, &g.used_hour.to_string(), '%');
                ui.text(px + 222, y, &g.now.to_string(), 'z');
            }
            // The graph of the one selected: a bar a minute for what was made (green) and used
            // (red), and the camp's stock as a line over them.
            let g = &p.goods[selected];
            let (gx, gy, gw, gh) = (px + 8, py + 106, pw - 16, 82);
            ui.frame(gx, gy, gw, gh, 'g');
            let n = g.made.len().max(1);
            let top = g.made.iter().chain(&g.used).copied().max().unwrap_or(1).max(1) as f32;
            let top_stock = g.stock.iter().copied().max().unwrap_or(1).max(1) as f32;
            let step = ((gw - 4) / n as i32).clamp(2, 6);
            let bars = (gh - 24) as f32;
            for i in 0..g.made.len() {
                let x = gx + 2 + i as i32 * step;
                let m = (f32::from(g.made[i]) / top * bars).round() as i32;
                let u = (f32::from(g.used[i]) / top * bars).round() as i32;
                ui.rect(x, gy + gh - 3 - m, (step / 2).max(1), m, 'y');
                ui.rect(x + (step / 2).max(1), gy + gh - 3 - u, (step / 2).max(1), u, '7');
            }
            let mut last: Option<(i32, i32)> = None;
            for (i, st) in g.stock.iter().enumerate() {
                let (x, y) = (gx + 2 + i as i32 * step + step / 2, gy + gh - 3 - (*st as f32 / top_stock * bars).round() as i32);
                if let Some((lx, ly)) = last {
                    ui.line(lx, ly, x, y, 'z');
                }
                last = Some((x, y));
            }
            let title = format!("{}: THE LAST {} MIN", g.name.to_uppercase(), g.made.len());
            ui.text(gx + 4, gy + 3, &title, 'T');
            let mut lx = gx + 4;
            for (ink, word) in [('y', format!("MADE (MOST {})", top as u32)), ('7', "USED".to_string()), ('z', format!("IN STOCK (MOST {})", top_stock as u32))] {
                ui.rect(lx, gy + 13, 5, 5, ink);
                ui.text(lx + 8, gy + 12, &word, 'v');
                lx += 8 + tw(&word) + 10;
            }
        }
        _ => {
            let chokes = board.rig.numbers.as_ref().map(|n| n.chokes.as_slice()).unwrap_or(&[]);
            ui.text(px + 8, py + 20, "WHAT CHOKES THE WORKS, WORST FIRST: E GOES THERE", 'v');
            if chokes.is_empty() {
                ui.text(px + 8, py + 36, "NOTHING IS HELD UP. THE WORKS RUN FREE.", '3');
                return;
            }
            let selected = board.selection.min(chokes.len() - 1);
            let rows = 11usize;
            let start = selected.saturating_sub(rows / 2).min(chokes.len().saturating_sub(rows));
            for (i, ch) in chokes.iter().enumerate().skip(start).take(rows) {
                let y = py + 32 + (i - start) as i32 * 13;
                if i == selected {
                    ui.rect(px + 4, y - 2, pw - 8, 12, 'g');
                }
                let text = format!("{}. {}", i + 1, ch.text);
                let lost = format!("{} S", ch.lost);
                let room = ((pw - 16 - tw(&lost) - 10) / crate::render::overworld::ink::GLYPH_ADVANCE) as usize;
                let line: String = if text.chars().count() > room { text.chars().take(room - 2).collect::<String>() + ".." } else { text };
                ui.text(px + 8, y, &line, if i == selected { 'Y' } else if ch.keep { '4' } else { 'w' });
                ui.text(px + pw - tw(&lost) - 8, y, &lost, if ch.keep { '4' } else { 'G' });
            }
        }
    }
}

/// The tile of the ground a click at `(x, y)` of the 768 by 448 picture lands on, in the
/// flat view of seat `seat`'s knight (the native view, two to a pixel).
pub fn tile_at(run: &Run, seat: u32, x: i32, y: i32) -> Option<(u8, u8)> {
    let origin = camera(run, Some(seat));
    let (px, py) = (origin.0 + x / 2, origin.1 + y / 2);
    let (tx, ty) = (px.div_euclid(T), py.div_euclid(T));
    ((0..crate::kingdom::REALM_WIDTH).contains(&(tx as i16)) && (0..crate::kingdom::REALM_HEIGHT).contains(&(ty as i16))).then_some((tx as u8, ty as u8))
}

/// The hotbar slot (0 to 8) a click at `(x, y)` of the 768 by 448 picture lands on (a slot of the
/// bank in hand, or a bank's tab, which takes the same place in that bank), or `Err(())` for the
/// rest of the builder's strip, where nothing is laid.
pub fn slot_at(x: i32, y: i32, rig: &builder_rig::Rig) -> Result<Option<u8>, ()> {
    let (nx, ny) = (x / 2, y / 2);
    let top = NATIVE_H - STRIP_H - 2;
    if ny < top {
        return Ok(None);
    }
    if (top + 3..top + 15).contains(&ny) && nx >= 4 {
        let slot_w = 97;
        let i = (nx - 4) / (slot_w + 2);
        if i < builder_rig::BANK as i32 && (nx - 4) % (slot_w + 2) < slot_w {
            return Ok(Some((rig.bank() * builder_rig::BANK) as u8 + i as u8));
        }
        let tabs = 4 + 3 * (slot_w + 2) + 34;
        if nx >= tabs && (nx - tabs) % 14 < 12 && (nx - tabs) / 14 < (builder_rig::SLOTS / builder_rig::BANK) as i32 {
            let bank = ((nx - tabs) / 14) as usize;
            return Ok(Some((bank * builder_rig::BANK + usize::from(rig.slot) % builder_rig::BANK) as u8));
        }
    }
    Err(())
}
