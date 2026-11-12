//! The travellers on the kingdom's roads (`rules::wayfarers`), drawn in the
//! realm's inks on the black paper: a pilgrim with a lantern on his staff, a
//! peddler with his handcart, a minstrel in violet, a toll gang with a rope
//! across the road, a squire in a torn tabard, a drover in a wide hat. Only
//! what is happening is lit: a lantern burns while its bearer waits to be
//! spoken to, a cart's lamp while it is open for trade, a minstrel's notes
//! while he sings. The talk with one is a panel on the knight's own screen,
//! as a village of the Deep has: their face, their words, the answers on
//! 1, 2 and 3.

use super::*;
use crate::rules::wayfarers::{Kind, Stage, Wayfarer};

/// A body, 12 wide, built from placeholders `colours` turn into inks: `A`
/// the outline, `D` shade, `B` cloth, `C` light cloth, `H` the headgear,
/// `I` its shade, `J` its light, `S` skin, `s` skin in shade, `k` an eye.
/// Outlines are the cloth's own darkest ink, never black.
const HEADS: [[&str; 6]; 5] = [
    // A hood.
    ["....AAAA....", "...AHHHHA...", "..AHHHHHHA..", "..AHSSSSHA..", "..AHSSkSHA..", "...AHSSHA..."],
    // A cap with a brim before it.
    ["....AAAA....", "...AHHHHAA..", "..AHJHHHHAA.", "...ASSSSSA..", "...AsSSkSA..", "....ASSSA..."],
    // Bare, with hair.
    ["....AAAA....", "...AHHHHA...", "..AHJHHHHA..", "..AHSSSSSA..", "...ASSSkSA..", "....ASSSA..."],
    // A wide brim.
    [".AAAAAAAAAA.", "AHJHHHHHHHHA", ".AIHHHHHHIA.", "...ASSSSA...", "...AsSSkA...", "....ASSA...."],
    // A helm with a visor.
    ["....AAAA....", "...AHJHHA...", "..AHHHHHHA..", "..AIIIIIIA..", "..AHSkSSHA..", "...AHSSHA..."],
];

const BODY: [&str; 8] = [
    "..AABBBBAA..",
    ".ADCBBBBBDA.",
    ".ADCBBBBBDA.",
    ".ADBBBBBBDA.",
    ".ADBBBBBBDA.",
    "..ADBBBBDA..",
    "..ADBBBBDA..",
    "..AAAAAAAA..",
];

/// Two strides of legs and boots.
const LEGS: [[&str; 3]; 2] = [
    ["...Ab..bA...", "...bb..bb...", "..bbb..bbb.."],
    ["....Abb.A...", "....bbbbb...", "...bbb.bbb.."],
];

#[derive(Clone, Copy)]
struct Dress {
    head: usize,
    cloth: [char; 4],
    cover: [char; 3],
}

fn dress(kind: Kind) -> Dress {
    match kind {
        // Undyed hemp, a rust hood.
        Kind::Pilgrim => Dress { head: 0, cloth: ['B', 'r', 'o', 'O'], cover: ['r', 'B', 't'] },
        // A green apron over brown, a russet cap.
        Kind::Merchant => Dress { head: 1, cloth: ['F', 'E', 'l', 'm'], cover: ['R', 'p', 'o'] },
        // Violet and rose, as a minstrel dresses.
        Kind::Troubadour => Dress { head: 1, cloth: ['d', '(', ')', '['], cover: ['?', '!', ';'] },
        // A red tabard, an iron helm.
        Kind::Tollkeeper => Dress { head: 4, cloth: ['8', 'p', '7', 'R'], cover: ['G', 'j', 'h'] },
        // A tabard gone dusty blue, her own hair.
        Kind::Squire => Dress { head: 2, cloth: ['s', 'S', 'u', 'Q'], cover: ['I', 'b', 'B'] },
        // A drover's brown coat and wide hat.
        Kind::Gossip => Dress { head: 3, cloth: ['b', 'B', 'R', 'o'], cover: ['P', 'I', 'r'] },
    }
}

/// A henchman of the gang: dull and dark.
const HENCH: Dress = Dress { head: 0, cloth: ['K', 'g', 'j', 'G'], cover: ['g', 'K', 'j'] };

/// The body of `d`, 12 by 17, at `stride`.
fn body(d: Dress, stride: usize) -> Img {
    let map = |c: char| match c {
        'A' => d.cloth[0],
        'D' => d.cloth[1],
        'B' => d.cloth[2],
        'C' => d.cloth[3],
        'H' => d.cover[0],
        'I' => d.cover[1],
        'J' => d.cover[2],
        'S' => 't',
        's' => 'o',
        'k' => 'b',
        'b' => 'b',
        other => other,
    };
    let rows: Vec<String> = HEADS[d.head]
        .iter()
        .chain(BODY.iter())
        .chain(LEGS[stride % 2].iter())
        .map(|r| r.chars().map(map).collect())
        .collect();
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    Img::from_rows(&refs)
}

/// A soft pool of light round `(cx, cy)`: dithered, in two warm inks, the
/// way the realm lights fire.
fn glow(over: &mut Img, cx: i32, cy: i32, r: i32, core: char, rim: char) {
    for dy in -r..=r {
        for dx in -r..=r {
            let d = ((dx * dx) as f32 + (dy * dy) as f32 * 1.6).sqrt() / r as f32;
            if d < 1.0 && bayer(cx + dx, cy + dy) < (1.0 - d) * 0.8 {
                over.put(cx + dx, cy + dy, if d < 0.45 { core } else { rim });
            }
        }
    }
}

/// A place for a figure's feet: the figure stands with its feet at `(x, y)`.
fn stand_on(over: &mut Img, im: &Img, x: i32, y: i32, east: bool) {
    let im = if east { im.clone() } else { im.flip_h() };
    knights::shadow(over, x, y);
    over.stamp(&im, x - im.w / 2, y - im.h + 1);
}

/// The traveller as a picture, its feet at the bottom middle, facing east
/// (flip it west): the body and what it carries, and the light it makes.
pub(in super::super) fn traveller(kind: Kind, stride: usize, lit: bool, tick: u32) -> Img {
    let d = dress(kind);
    let man = body(d, stride);
    let (w, h) = (26, 24);
    let mut im = Img::new(w, h);
    let (bx, by) = (7, h - man.h);
    im.stamp(&man, bx, by);
    match kind {
        Kind::Pilgrim => {
            // A staff, taller than he is, and the lantern hung at its head.
            im.line(bx + 12, by - 4, bx + 12, h - 1, 'P');
            im.put(bx + 12, by - 5, 'o');
            im.rect(bx + 13, by - 4, 3, 4, 'o');
            im.put(bx + 14, by - 5, 'r');
            im.rect(bx + 14, by - 3, 1, 2, if lit { '5' } else { 'g' });
            // His badge: a scallop on the breast.
            im.put(bx + 7, by + 9, 't');
            im.put(bx + 6, by + 10, 't');
            im.put(bx + 8, by + 10, 't');
            // His hand on the staff.
            im.put(bx + 11, by + 8, 't');
        }
        Kind::Merchant => {
            // A pack on his back, a coil of rope over it.
            im.rect(bx + 1, by + 6, 3, 6, 'B');
            im.rect(bx, by + 7, 2, 4, 'r');
            im.put(bx + 1, by + 7, 'O');
            im.rect(bx + 1, by + 12, 3, 1, 'b');
        }
        Kind::Troubadour => {
            // A lute across him, its neck up and out.
            im.rect(bx + 6, by + 8, 5, 5, 'O');
            im.rect(bx + 7, by + 9, 3, 3, 'o');
            im.put(bx + 8, by + 10, 'b');
            im.line(bx + 10, by + 8, bx + 14, by + 3, 'P');
            im.put(bx + 14, by + 2, 'r');
            // A plume in the cap.
            im.line(bx + 3, by - 1, bx + 1, by - 4, ';');
            im.put(bx + 2, by - 3, '%');
        }
        Kind::Tollkeeper => {
            // A cudgel over the shoulder.
            im.line(bx + 11, by + 6, bx + 14, by - 1, 'P');
            im.rect(bx + 13, by - 3, 3, 3, 'r');
            im.put(bx + 14, by - 3, 'o');
        }
        Kind::Squire => {
            // A practice sword at the hip, a bandaged hand, the tabard torn.
            im.line(bx + 9, by + 11, bx + 14, by + 14, 'P');
            im.put(bx + 14, by + 14, 'o');
            im.put(bx + 9, by + 10, 'o');
            im.rect(bx + 10, by + 8, 2, 2, 'c');
            im.put(bx + 4, by + 13, '.');
            im.put(bx + 5, by + 12, '.');
        }
        Kind::Gossip => {
            // A crook for the herd, and a cowbell on it.
            im.line(bx + 12, by + 1, bx + 12, h - 1, 'P');
            im.line(bx + 12, by + 1, bx + 14, by, 'P');
            im.put(bx + 14, by + 1, 'P');
            im.rect(bx + 13, by + 3, 2, 2, 'O');
            im.put(bx + 13, by + 5, 'r');
        }
    }
    let _ = tick;
    im
}

/// The peddler's handcart: a bed of planks on two spoked wheels, crates and
/// sacks, a striped awning; its lamp lit while he is open for trade.
pub(in super::super) fn cart(open: bool) -> Img {
    let mut im = Img::new(28, 24);
    // Handles, bed, sideboards.
    im.line(0, 17, 5, 15, 'P');
    im.rect(4, 14, 22, 3, 'B');
    im.rect(4, 14, 22, 1, 'r');
    im.rect(5, 17, 20, 1, 'b');
    // Crates and sacks.
    im.rect(7, 8, 6, 6, 'r');
    im.rect(7, 8, 6, 1, 'o');
    im.rect(7, 11, 6, 1, 'B');
    im.rect(13, 10, 5, 4, 'o');
    im.rect(13, 10, 5, 1, 'O');
    im.rect(19, 9, 5, 5, 'P');
    im.rect(19, 9, 5, 1, 'o');
    im.put(21, 11, 'b');
    // The awning on two poles, in rust and hemp.
    im.line(6, 3, 6, 14, 'P');
    im.line(23, 3, 23, 14, 'P');
    for x in 4..26 {
        let stripe = if (x / 3) % 2 == 0 { 'R' } else { 'T' };
        im.put(x, 3, stripe);
        im.put(x, 4, stripe);
        im.put(x, 5, if stripe == 'R' { 'p' } else { 'o' });
    }
    // A wheel, spoked.
    for cx in [9, 21] {
        for k in 0..16 {
            let a = k as f32 * std::f32::consts::TAU / 16.0;
            im.put(cx + (a.cos() * 4.0).round() as i32, 19 + (a.sin() * 4.0).round() as i32, 'b');
        }
        im.line(cx - 3, 19, cx + 3, 19, 'P');
        im.line(cx, 16, cx, 22, 'P');
        im.put(cx, 19, 'o');
    }
    // The lamp on the awning's edge.
    im.rect(25, 6, 2, 3, if open { 'o' } else { 'g' });
    im.put(25, 5, 'r');
    if open {
        im.put(26, 7, '5');
    }
    im
}

/// What the traveller shows to the screen: where it stands (view pixels),
/// and its sprite.
struct Seen {
    x: i32,
    y: i32,
    east: bool,
}

/// What stands in the road and what the fight leaves on it: the traveller
/// (with the peddler's cart and the gang's rope), the gang in a fight, and
/// what lies on the ground where it fell. In 3D the figures stand up apart
/// (`figures`).
pub(super) fn bodies(over: &mut Img, run: &Run, origin: (i32, i32), focus: Option<u32>, mut figures: Option<&mut Vec<Figure>>) -> Vec<(i32, i32, i32, i32)> {
    let mut taken = Vec::new();
    let tick = run.tick as u32;
    let local = |x: f32, y: f32| {
        let (x, y) = at(x, y);
        (x - origin.0, y - origin.1)
    };
    let on_screen = |x: i32, y: i32, r: i32| x >= -r && y >= -r && x < NATIVE_W + r && y < NATIVE_H + r;
    // Whatever the fallen gang dropped, where it lies.
    for item in run.room().items.iter().filter(|item| run.lies_for(item, focus)) {
        let (x, y) = local(item.x, item.y);
        if on_screen(x, y, 24) {
            let card = cards::turning_card(run.book.get(&item.card), (x, y), tick);
            over.stamp(&card, x - card.w / 2, y - card.h / 2);
        }
    }
    let Some(w) = run.wayfarers.on.as_ref() else {
        return taken;
    };
    let (x, y) = local(w.x, w.y);
    if !on_screen(x, y, 60) {
        return taken;
    }
    let seen = Seen { x, y, east: w.east };
    match w.stage {
        Stage::Fighting(_) => {
            for enemy in run.enemies.iter().filter(|e| w.gang.contains(&e.id)) {
                let (fx, fy) = local(enemy.x, enemy.y);
                if !on_screen(fx, fy, 40) {
                    continue;
                }
                let im = foe_figure_with(run, enemy, tick);
                if let Some(figures) = figures.as_deref_mut() {
                    // Standing up in 3D: its shadow and itself on a picture of their own.
                    let mut img = Img::new(im.w, im.h + 4);
                    foe_material::shadow(&mut img, im.w / 2, im.h - 2, im.w, enemy.kind.flies());
                    img.stamp(&im, 0, 0);
                    figures.push(Figure { img, x: fx - im.w / 2, y: fy - im.h + 3, frac: (0.0, 0.0), depth: enemy.y, height: 0.0, additive: false, bodies: Vec::new(), foe: true, paints: Vec::new() });
                } else {
                    foe_material::shadow(over, fx, fy, im.w, enemy.kind.flies());
                    over.stamp(&im, fx - im.w / 2, fy - im.h + 3);
                }
                marks::enemy_bar(over, run, enemy, fx, fy, &im, (0, 0));
                taken.push((fx - im.w / 2, fy - im.h, im.w, im.h));
            }
        }
        _ => {
            draw_traveller(over, run, w, &seen, tick, figures.as_deref_mut());
            // The road keeps its place names off whoever stands in it.
            let span = match w.kind {
                Kind::Tollkeeper => 34,
                Kind::Merchant => 30,
                _ => 12,
            };
            taken.push((x - span, y - 26, span * 2, 28));
        }
    }
    taken
}

fn draw_traveller(over: &mut Img, run: &Run, w: &Wayfarer, seen: &Seen, tick: u32, figures: Option<&mut Vec<Figure>>) {
    let stride = if w.walking(run.tick) { (tick / 8) as usize % 2 } else { 0 };
    let talking = run.wayfarers.talks.values().any(|t| t.page <= 1);
    // Their lights burn for as long as they are about their errand.
    let waiting = w.talkable();
    let (x, y) = (seen.x, seen.y);
    // The gang's men hold a rope across the road, one at each end of it;
    // paid or named, it lies slack on the ground.
    if w.kind == Kind::Tollkeeper {
        let path = &crate::rules::wayfarers::paths()[usize::from(w.road).min(crate::rules::wayfarers::paths().len() - 1)];
        let (a, b) = (path.at(w.along - 2.0), path.at(w.along + 2.0));
        let (tx, ty) = (b.0 - a.0, b.1 - a.1);
        let len = tx.hypot(ty).max(1e-3);
        // Across the road: perpendicular to its course, a knight's reach each way.
        let (ax, ay) = (-ty / len * 24.0, tx / len * 24.0);
        let (e0, e1) = ((x - ax as i32, y - 3 - ay as i32), (x + ax as i32, y - 3 + ay as i32));
        let steps = 48;
        for k in 0..=steps {
            let t = k as f32 / steps as f32;
            let sag = (t * std::f32::consts::PI).sin();
            let (px, py) = (e0.0 as f32 + (e1.0 - e0.0) as f32 * t, e0.1 as f32 + (e1.1 - e0.1) as f32 * t);
            let (px, py) = if w.done {
                (px, py + 2.0 + sag)
            } else if ay.abs() >= ax.abs() {
                (px + sag * 3.0, py - 9.0)
            } else {
                (px, py - 9.0 + sag * 4.0)
            };
            over.put(px.round() as i32, py.round() as i32, if k % 4 < 2 { 'T' } else { 'o' });
            if !w.done && matches!(k, 12 | 24 | 36) {
                over.rect(px.round() as i32, py.round() as i32 + 1, 2, 3, '7');
            }
        }
        let hench = body(HENCH, 0);
        let mut men = [(e0, e1.0 >= e0.0), (e1, e0.0 > e1.0)];
        men.sort_by_key(|((_, my), _)| *my);
        for ((mx, my), east) in men {
            stand_on(over, &hench, mx, my, east);
            // A staff planted at the rope's end.
            over.line(mx + if east { 5 } else { -5 }, my - 8, mx + if east { 5 } else { -5 }, my, 'P');
        }
    }
    let man = traveller(w.kind, stride, waiting || talking, tick);
    let (px, py) = (x, y);
    if w.kind == Kind::Merchant {
        let wares = cart(waiting || talking);
        let side = if seen.east { -1 } else { 1 };
        let cx = px + side * 22;
        let flipped = if seen.east { wares } else { wares.flip_h() };
        knights::shadow(over, cx, py);
        over.stamp(&flipped, cx - flipped.w / 2, py - flipped.h + 2);
        if waiting || talking {
            glow(over, cx + side * -9, py - 14, 9, 'o', 'r');
        }
    }
    // The lantern burns for as long as its bearer waits to be spoken to.
    if w.kind == Kind::Pilgrim && (waiting || talking) {
        let side = if seen.east { 1 } else { -1 };
        glow(over, px + side * 7, py - man.h + 8, 9, 'o', 'r');
    }
    let sprite = if seen.east { man } else { man.flip_h() };
    if let Some(figures) = figures {
        let (fx, fy) = (px - sprite.w / 2, py - sprite.h + 1);
        let mut img = Img::new(sprite.w, sprite.h + 3);
        knights::shadow(&mut img, sprite.w / 2, sprite.h - 1);
        img.stamp(&sprite, 0, 0);
        figures.push(Figure { img, x: fx, y: fy, frac: (0.0, 0.0), depth: w.y, height: 0.0, additive: false, bodies: Vec::new(), foe: false, paints: Vec::new() });
    } else {
        knights::shadow(over, px, py);
        over.stamp(&sprite, px - sprite.w / 2, py - sprite.h + 1);
    }
    // Notes rise from the minstrel's lute while his song is on.
    if w.kind == Kind::Troubadour && run.wayfarers.talks.values().any(|t| t.page == 2) {
        for k in 0..3u32 {
            let t = (tick + k * 11) % 36;
            let nx = px + 6 + (k as i32 - 1) * 5 + (t as i32 / 6 % 2);
            let ny = py - 20 - t as i32 / 2;
            over.put(nx, ny, '6');
            over.put(nx + 1, ny, '6');
            over.put(nx + 1, ny - 1, '5');
            over.put(nx + 1, ny - 2, '5');
        }
    }
}

/// Words broken into lines of at most `n` characters.
fn wrap(s: &str, n: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut row = String::new();
    for word in s.split_whitespace() {
        if !row.is_empty() && row.len() + 1 + word.len() > n {
            out.push(std::mem::take(&mut row));
        }
        if !row.is_empty() {
            row.push(' ');
        }
        row.push_str(word);
    }
    if !row.is_empty() {
        out.push(row);
    }
    out
}

/// The plate over the traveller, and the key beside them when a knight
/// could speak: the rectangles taken, so no other plate lies over them.
pub(super) fn plates(over: &mut Img, run: &Run, geo: &map::Geography, origin: (i32, i32), focus: Option<u32>) -> Vec<(i32, i32, i32, i32)> {
    let mut taken = Vec::new();
    let Some(w) = run.wayfarers.on.as_ref().filter(|w| !matches!(w.stage, Stage::Fighting(_))) else {
        return taken;
    };
    let id = focus.unwrap_or(1);
    let (x, y) = {
        let (x, y) = at(w.x, w.y);
        (x - origin.0, y - origin.1)
    };
    if x < -40 || y < -40 || x > NATIVE_W + 40 || y > NATIVE_H + 40 {
        return taken;
    }
    let Some(hero) = run.players.get(&id) else {
        return taken;
    };
    let near = (hero.x - w.x).hypot(hero.y - w.y) / TILE_UNITS;
    if near > 14.0 {
        return taken;
    }
    let plate = w.plate();
    let pw = crate::ink::text_width(&plate);
    taken.push(chrome::label(over, (x - pw / 2, y + 5), &plate, 'i', chrome::Tone::SLATE));
    let door = crate::rules::kingdom::door_at(geo, hero.x, hero.y).is_some();
    if run.wayfarer_in_reach(id).is_some() && !door && !hero.sitting && w.stage == Stage::Walking {
        // The key is the knight's: it rides over their head.
        let (kx, ky) = at(hero.x, hero.y);
        let (kx, ky) = (kx - origin.0, ky - origin.1);
        let label = "E: TALK";
        let lw = crate::ink::text_width(label);
        taken.push(chrome::label(over, (kx - lw / 2, ky - 34), label, 'O', chrome::Tone::KEY));
    }
    taken
}

/// A knight's portrait: the traveller from the shoulders up, large.
fn portrait(kind: Kind) -> Img {
    let man = traveller(kind, 0, true, 0);
    let (bx, by) = (7, 24 - 17);
    let head = Img::from_view(&man, bx, by, 12, 13);
    resized(&head, 3.0)
}

/// The conversation on the knight's own screen: the traveller's face, their
/// name and words, and the answers on 1, 2 and 3.
pub(super) fn panel(ui: &mut Img, run: &Run, origin: (i32, i32), focus: Option<u32>) {
    let id = focus.unwrap_or(1);
    let Some(speech) = run.wayfarer_speech(id) else {
        return;
    };
    let lines = wrap(&speech.words, 50);
    let h = (lines.len() as i32 * 9 + speech.answers.len() as i32 * 10 + 30).max(56 + 14);
    // Over the foot of the view, unless that is where the traveller stands.
    let low = run.wayfarers.on.as_ref().is_some_and(|w| at(w.x, w.y).1 - origin.1 > ui.h * 45 / 100);
    let y = if low { 6 } else { ui.h - h - 6 };
    chrome::panel(ui, (6, y, ui.w - 12, h), chrome::Tone::FRAMED);
    chrome::ring(ui, (6, y, ui.w - 12, h), 3);
    let face = portrait(speech.kind);
    chrome::inset_panel(ui, (12, y + 8, face.w + 6, face.h + 6), chrome::Tone::WELL);
    ui.stamp(&face, 15, y + 11);
    let tx = 24 + face.w;
    ui.text(tx, y + 7, &speech.who, '9');
    for (i, line) in lines.iter().enumerate() {
        ui.text(tx, y + 19 + i as i32 * 9, line, 'i');
    }
    let ay = y + 22 + lines.len() as i32 * 9;
    for (i, answer) in speech.answers.iter().enumerate() {
        let yy = ay + i as i32 * 10;
        let keyed = answer.as_bytes().first().is_some_and(u8::is_ascii_digit);
        if keyed {
            chrome::keycap(ui, (tx - 1, yy - 2, 9, 10));
            ui.text(tx + 1, yy, &answer[..1], 'K');
            ui.text(tx + 12, yy, answer[2..].trim_start(), 'O');
        } else {
            ui.text(tx, yy, answer, 'O');
        }
    }
}
