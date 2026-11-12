//! The Creator is another of the genie's scrolls, in the game's own pixels.
//! Its paper, rolls, ink reveal, smoke and seal are the wish scroll's actual
//! drawing functions. A local recipe never reaches the live fight.

use super::genie::{self, Variant, bold, centred, paper, roll};
use super::overworld::arena::pfx::{Pen, ease, ramps};
use super::overworld::ink::{Img, bayer, text_width};
use crate::rules::creation::{
    Creation, Material, Pigment, Role, Session, Slot, Step, WeaponCreation, Who, kit, part, part_name,
};

mod baked;
mod drive;
mod weapon;
pub use baked::{Baked, Motion, Reel, Way};
pub use drive::{FRIEND_LINES, Floor};

/// How the art that shows this creation times its clips (the drive's own timing for it): the host
/// says it to the session before it steps the Test drive (`Session::timing`).
pub fn timing(creation: &crate::rules::creation::Creation) -> crate::rules::creation::Timing {
    baked::for_creation(creation).timing()
}

const WIDTH: i32 = 384;
const HEIGHT: i32 = 214;
const SCROLL_W: i32 = 232;
const SCROLL_H: i32 = 188;
const INK: char = 'n';
const SOFT: char = 'P';
const RED: char = 'B';
pub const REVEAL_TICK: u64 = 18;
pub const SEAL_TICK: u64 = 32;

/// The future bake supplies native pixel frames through this boundary. Empty,
/// oversized or zero-duration atlases never reach the frame painter. The
/// feet are at the bottom centre of each cell (the biped's stage anchor).
#[derive(Clone, Debug)]
pub struct PreviewAtlas {
    frames: Vec<Img>,
    frame_ticks: u64,
}

impl PreviewAtlas {
    pub fn new(frames: Vec<Img>, frame_ticks: u64) -> Result<Self, &'static str> {
        if frames.is_empty() || frames.len() > 128 || frame_ticks == 0 {
            return Err("An atlas needs 1-128 timed frames");
        }
        let size = (frames[0].w, frames[0].h);
        if size.0 < 1
            || size.1 < 1
            || size.0 > 96
            || size.1 > 88
            || frames.iter().any(|f| (f.w, f.h) != size)
            || size.0 as usize * size.1 as usize * frames.len()
                > usize::from(crate::rules::creation::MAX_ATLAS_SIDE).pow(2)
        {
            return Err("Atlas cells must share a bounded native size");
        }
        Ok(Self {
            frames,
            frame_ticks,
        })
    }
}

/// What the world draws of a made creature (the rite): its baked figure, idling or walking east or
/// west, scaled to stand no taller than `high` pixels. The game's camera, the Creator's own art.
pub fn world_creature(c: &Creation, east: bool, walking: bool, tick: u64, high: i32) -> Img {
    let way = if east { Way::East } else { Way::West };
    let motion = if walking { Motion::Walk(way) } else { Motion::Idle };
    let im = baked::figure_facing(c, motion, way, tick, tick as f32 * 0.08, None);
    shrunk(&im, high)
}

/// A made weapon as the world hangs it: the Creator's own assembled picture, upright, no taller than
/// `high` and no wider than `wide`.
pub fn world_weapon(w: &WeaponCreation, wide: i32, high: i32) -> Option<Img> {
    let (im, _) = weapon::display(w, None, (wide, high), (wide, high))?;
    Some(im)
}

/// `im` scaled by the nearest whole ratio down so it is no taller than `high`.
fn shrunk(im: &Img, high: i32) -> Img {
    if im.h <= high || im.h <= 0 {
        return im.clone();
    }
    let (w, h) = ((im.w * high / im.h).max(1), high);
    let mut out = Img::new(w, h);
    for y in 0..h {
        for x in 0..w {
            if let Some(c) = im.get(x * im.w / w, y * im.h / h) {
                out.set(x, y, c);
            }
        }
    }
    out
}

/// One feed point for real creature atlases. Until then a baked set stands in (`baked`): the
/// registry picks it by role (the in-game Bailiff, or a real creature's set for the roles that
/// have one), drawn through the boss library's own dressing, reshaped by height and bulk, dyed by
/// the main pigment, turned through its eight ways when the set has them. Assembled anatomy belongs
/// to the future bake, and the stage says so.
pub fn creature_preview(creation: &Creation, tick: u64, atlas: Option<&PreviewAtlas>) -> Img {
    if let Some(atlas) = atlas {
        return atlas.frames[(tick / atlas.frame_ticks % atlas.frames.len() as u64) as usize].clone();
    }
    baked::preview(creation, tick)
}

fn pigments(p: Pigment) -> [char; 3] {
    match p {
        Pigment::Ochre => ['t', 'o', 'P'],
        Pigment::Moss => ['M', 'm', 'F'],
        Pigment::Slate => ['U', 'u', 'S'],
        Pigment::Russet => ['R', 'r', 'P'],
        Pigment::Bone => ['H', 'i', 'h'],
        Pigment::Charcoal => ['h', 'g', 'K'],
    }
}

/// Paint over the room only, before the client lifts in its party/HUD bands.
/// Terminals keep their own UI. Native pixels are never fractionally scaled.
pub fn paint(out: &mut Img, session: &Session, tick: u64, atlas: Option<&PreviewAtlas>) {
    paint_with(out, session, tick, atlas, None);
}

/// `paint`, with the fixture floor the Test drive stands the creature on (dev builds).
pub fn paint_with(out: &mut Img, session: &Session, tick: u64, atlas: Option<&PreviewAtlas>, floor: Option<&Floor>) {
    if !session.open || out.w < WIDTH || out.h < 150 {
        return;
    }
    if session.step == Step::Drive && let Some(floor) = floor {
        drive::paint(out, session, floor, tick);
        return;
    }
    let k = (out.w / WIDTH).min(out.h / HEIGHT).max(1);
    let native_h = (out.h / k).min(HEIGHT);
    let canvas = frame(session, tick, native_h, atlas);
    for y in 0..out.h {
        for x in 0..out.w {
            if let Some([r, g, b]) = out.get(x, y) {
                out.set(x, y, [r / 5, g / 5, b / 5]);
            }
        }
    }
    out.stamp_times(
        &canvas,
        (out.w - WIDTH * k) / 2,
        (out.h - native_h * k) / 2,
        k,
    );
}

/// The rite's invitation to the Creator on a newcomer's screen (`rite`): `shown` is the slip's age
/// in ticks and whether it is the full slip (the first seconds) or the corner hint (after). Drawn in
/// the Creator's own paper, never while the Creator is open. It takes no key but the one it names.
pub fn paint_invite(out: &mut Img, shown: Option<(u64, bool)>, creator_open: bool) {
    let Some((age, slip)) = shown else { return };
    if creator_open || out.w < WIDTH {
        return;
    }
    let k = (out.w / WIDTH).max(1);
    let (w, h) = if slip { (220, 58) } else { (92, 15) };
    let mut im = Img::new(w, h);
    paper(&mut im, 0, 0, w, h, 0x71e3);
    if slip {
        roll(&mut im, 0, 0, w);
        roll(&mut im, 0, h - 9, w);
        centred(&mut im, 14, "YOUR RITE", INK, true);
        centred(&mut im, 26, "MAKE SOMETHING FOR THE REALM", INK, false);
        centred(&mut im, 38, "C: OPEN THE CREATOR", RED, true);
    } else {
        centred(&mut im, 4, "C: YOUR RITE", RED, true);
    }
    // The slip slides down from above the screen as it arrives, the hint just sits.
    let drop = if slip { (ease::back((age as f32 / 12.0).min(1.0)) * (h + 6) as f32) as i32 - h } else { 0 };
    let x = if slip { (out.w - w * k) / 2 } else { out.w - (w + 5) * k };
    out.stamp_times(&im, x, (6 + drop) * k, k);
}

fn frame(session: &Session, tick: u64, h: i32, atlas: Option<&PreviewAtlas>) -> Img {
    let calm = super::viewer::calm();
    // Opaque black also covers the geometry beneath a glass RoomScene's
    // screen layer. Flat frames and GPU scenes must show the same stage.
    let mut out = Img::black(WIDTH, h);
    let compact = h < HEIGHT;
    let scroll_h = if compact { h - 22 } else { SCROLL_H };
    let y0 = 22;
    let age = if calm {
        64.0
    } else {
        tick.saturating_sub(session.granted.unwrap_or(session.opened)) as f32
    };
    let open = ease::out(age / 10.0);
    let seal_age = session
        .granted
        .map(|at| if calm { 64 } else { tick.saturating_sub(at) });
    let sealed = seal_age.is_some_and(|age| age >= SEAL_TICK);
    let motion_age = if sealed { age - SEAL_TICK as f32 } else { age };
    let sheet = scroll(session, tick, scroll_h);
    let shown = genie::unrolled(
        &sheet,
        open,
        sealed,
        motion_age,
        if calm { 0 } else { tick },
        Variant::Plain,
    );
    let mut back = Img::new(WIDTH, h);
    let mut front = Img::new(WIDTH, h);
    // The wish's hanging seal would cover this editor's footer. Keep the
    // margin magic, then land its same wax stamp in the review's reserved bay.
    if !calm {
        genie::magic(
            &mut back,
            &mut front,
            (4, y0, SCROLL_W, scroll_h),
            (352, h - 8),
            Variant::Plain,
            age,
            open,
            false,
            tick,
        );
    }
    out.stamp(&back, 0, 0);
    stage(&mut out, session, tick, h, atlas);
    out.stamp(&shown, 4, y0);
    out.stamp(&front, 0, 0);
    if sealed {
        let at = (SCROLL_W / 2 + 4, y0 + scroll_h - 67);
        if (4.0..30.0).contains(&motion_age) {
            let mut pen = Pen::new(&mut out, 500);
            let since = motion_age - 4.0;
            pen.ring(
                (at.0 as f32, at.1 as f32 + 3.0),
                8.0 + ease::out(since / 12.0) * 13.0,
                1.3,
                0.55,
                ramps::GOLD,
                ease::inward((since / 22.0).min(1.0)),
            );
            for side in [-1.0, 1.0] {
                pen.twinkle(
                    (at.0 as f32 + side * 25.0, at.1 as f32 + 1.0),
                    3.0,
                    (since / 22.0).min(1.0),
                    ramps::GOLD,
                );
            }
        }
        genie::seal_stamp(&mut out, 1, at, motion_age);
    }
    progress(&mut out, session);
    out
}

/// The flow's stops along the top: a creature's seven (the Test drive last), a weapon's six.
fn progress(im: &mut Img, session: &Session) {
    im.line(12, 18, 372, 18, 'P');
    let flow = session.flow();
    let stops: Vec<Step> = if session.forge() { flow.to_vec() } else { flow.iter().copied().chain([Step::Drive]).collect() };
    let at = session.position();
    let xs = if session.forge() { [13, 69, 125, 187, 249, 323, 0] } else { [12, 58, 104, 150, 196, 252, 308] };
    for (i, (s, x)) in stops.into_iter().zip(xs).enumerate() {
        let on = i == at;
        let label = format!("{} {}", i + 1, s.name().to_uppercase());
        im.text(
            x,
            5,
            &label,
            if on {
                'T'
            } else if i < at {
                'o'
            } else {
                'h'
            },
        );
        if on {
            im.line(x, 16, x + text_width(&label), 16, 't');
            im.put(x - 3, 8, 't');
        }
    }
}

/// Words broken at spaces to `cols` characters.
fn wrap(text: &str, cols: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match lines.last_mut() {
            Some(l) if l.chars().count() + 1 + word.chars().count() <= cols => {
                l.push(' ');
                l.push_str(word);
            }
            _ => lines.push(word.chars().take(cols).collect()),
        }
    }
    lines
}

fn scroll(session: &Session, tick: u64, h: i32) -> Img {
    let mut im = Img::new(SCROLL_W, h);
    paper(&mut im, 10, 8, SCROLL_W - 20, h - 16, 0xc4ea);
    roll(&mut im, 8, 0, SCROLL_W - 16);
    roll(&mut im, 8, h - 9, SCROLL_W - 16);
    centred(&mut im, 13, "THE CREATOR", INK, true);
    for (a, b) in [(25, 68), (164, 207)] {
        im.line(a, 16, b, 16, SOFT);
    }
    let title = if session.granted.is_some() {
        "WISH SEALED".into()
    } else if session.chat.open {
        "WISH IT: TELL THE GENIE".to_string()
    } else {
        session.title().to_uppercase()
    };
    centred(
        &mut im,
        26,
        &if session.chat.open {
            title
        } else {
            format!("{} / 6  {title}", session.position() + 1)
        },
        SOFT,
        false,
    );
    let footer = h - 43;
    // The Wish row is pinned under the list on every builder page.
    let wish_y = footer - 17;
    let top = 38;
    if session.step == Step::Life {
        life_words(&mut im, session, footer);
    } else if session.chat.open {
        transcript(&mut im, session, top, wish_y - 3);
        wish_slip(&mut im, session, tick, wish_y, true);
    } else {
        let count = session.row_count();
        let room = wish_y - 3 - top;
        // A creature's Paint page with a fifth row (a Hero's Look) keeps its colour chips: the rows
        // give up the room the chips need (the four-row page is laid out as it always was).
        let chips = if session.step == Step::Paint && !session.forge() && count > 4 { 14 } else { 0 };
        let row_h = ((room - chips) / count as i32).clamp(13, 17);
        // Short views scroll the rows just as the tavern mirror does.
        let visible = (room / row_h).max(1).min(count as i32) as usize;
        let start = session.row.min(count - 1).saturating_sub(visible - 1).min(count - visible);
        for (n, row) in (start..start + visible).enumerate() {
            let y = top + 3 + n as i32 * row_h;
            choice_row(&mut im, session, row, y, row_h);
        }
        let below = top + 3 + visible as i32 * row_h + 3;
        if session.step == Step::Paint && wish_y - below >= 12 {
            swatches(&mut im, session, below);
        }
        if session.step == Step::Enchant && session.forge() {
            weapon_numbers(&mut im, session, top + 3 + row_h + 6, wish_y - 3);
        }
        wish_slip(&mut im, session, tick, wish_y, false);
    }
    for x in (20..SCROLL_W - 20).step_by(3) {
        im.put(x, footer, SOFT);
    }
    power_gauge(&mut im, session, footer + 5);
    let hint = if session.chat.open {
        "T: SPEAK   Q: BACK"
    } else if session.step == Step::Life {
        match (session.granted.is_some(), session.forge() || !session.can_drive) {
            (true, true) => "E: KEEP IT   Q: REVISE",
            (true, false) => "E: TEST DRIVE  Q: REVISE",
            _ => "E: BRING TO LIFE  Q: BACK",
        }
    } else if session.on_wish_row() {
        "E: WISH IT   T: WISH   Q: BACK"
    } else if session.step == Step::Role {
        "ARROWS: ROLE  E: NEXT  Q: CLOSE"
    } else {
        "ARROWS: EDIT  E: NEXT  Q: BACK"
    };
    centred(&mut im, h - 21, hint, SOFT, false);
    im
}

/// The Wish row (or, in the chat, the field the player speaks into): a seal marked T, the words
/// "WISH IT", and the last thing said.
fn wish_slip(im: &mut Img, s: &Session, tick: u64, y: i32, field: bool) {
    let selected = field || s.on_wish_row();
    if selected {
        for py in y - 2..y + 12 {
            for px in 19..SCROLL_W - 19 {
                if (px + py) % 2 == 0 {
                    im.put(px, py, 't');
                }
            }
        }
        im.line(18, y - 2, 18, y + 11, RED);
    }
    // The seal of the numbered rows, marked T for the shell's own wish key.
    im.ellipse(23.0 + 5.5, y as f32 - 1.0 + 5.5, 6.0, 6.0, if selected { '8' } else { 'p' });
    im.ellipse(23.0 + 5.0, y as f32 - 1.0 + 5.0, 4.5, 4.5, if selected { 'B' } else { 'R' });
    im.text(26, y + 1, "T", 'T');
    if field {
        let blink = (tick / 15) % 2 == 0;
        let said = &s.chat.draft;
        let shown = if said.is_empty() { "PRESS T, THEN SAY IT".to_string() } else { said.to_uppercase() };
        let tail: String = shown.chars().rev().take(26).collect::<Vec<_>>().into_iter().rev().collect();
        im.text(40, y + 1, &tail, if said.is_empty() { SOFT } else { INK });
        if blink && !said.is_empty() {
            im.rect(40 + text_width(&tail) + 2, y, 3, 9, RED);
        }
        return;
    }
    if selected {
        bold(im, 40, y + 1, "WISH IT", INK);
    } else {
        im.text(40, y + 1, "WISH IT", INK);
    }
    let last = s.chat.lines.iter().rev().find(|l| l.who == Who::You);
    let value = last.map_or_else(|| "SAY IT WITH T".to_string(), |l| l.text.to_uppercase());
    let value: String = if value.chars().count() > 19 { format!("{}..", value.chars().take(17).collect::<String>()) } else { value };
    im.text(98, y + 1, &value, if selected { RED } else { SOFT });
}

/// The conversation, newest at the foot: the player's lines in ink, the genie's in red-brown.
fn transcript(im: &mut Img, s: &Session, top: i32, bottom: i32) {
    let mut lines: Vec<(String, char)> = Vec::new();
    for l in &s.chat.lines {
        let (prefix, ink) = match l.who {
            Who::You => ("> ", INK),
            Who::Genie => ("", RED),
        };
        for (i, w) in wrap(&l.text.to_uppercase(), 31 - prefix.len()).into_iter().enumerate() {
            lines.push((if i == 0 { format!("{prefix}{w}") } else { format!("{}{w}", " ".repeat(prefix.len())) }, ink));
        }
    }
    let rows = ((bottom - top) / 9).max(1) as usize;
    let shown = lines.len().saturating_sub(rows);
    if lines.is_empty() {
        centred(im, top + 12, "SAY WHAT YOU WANT MADE.", SOFT, false);
        let hint = if s.forge() { "A FLAMING BONE AXE" } else { "A TINY GREEN MINION WITH HORNS" };
        centred(im, top + 25, hint, INK, false);
        centred(im, top + 40, "THEN: BIGGER, GOLD GUARD,", SOFT, false);
        centred(im, top + 49, "MAKE THE FLAME BLUE.", SOFT, false);
    }
    for (i, (text, ink)) in lines.into_iter().skip(shown).enumerate() {
        im.text(20, top + 2 + i as i32 * 9, &text, ink);
    }
}

fn swatches(im: &mut Img, s: &Session, y: i32) {
    let on = s.row;
    if s.forge() {
        for (i, m) in Material::METALS.into_iter().enumerate() {
            let x = 42 + i as i32 * 26;
            for (j, step) in [1usize, 3, 4].into_iter().enumerate() {
                let c = kit().ramps.get(m.ramp()).and_then(|r| r.get(step)).map_or([255, 0, 255], |h| {
                    let v = u32::from_str_radix(h.trim_start_matches('#'), 16).unwrap_or(0);
                    [(v >> 16) as u8, (v >> 8) as u8, v as u8]
                });
                for dy in 0..10 {
                    for dx in 0..5 {
                        im.set(x + j as i32 * 5 + dx, y + 1 + dy, c);
                    }
                }
            }
            im.frame(x - 1, y, 17, 12, if m == s.weapon.paint.metal && on == 0 { INK } else { SOFT });
        }
        return;
    }
    for (i, p) in Pigment::ALL.into_iter().enumerate() {
        let x = 42 + i as i32 * 26;
        for (j, ch) in pigments(p).into_iter().enumerate() {
            im.rect(x + j as i32 * 5, y + 1, 5, 10, ch);
        }
        im.frame(
            x - 1,
            y,
            17,
            12,
            if p == s.creation.paint.scheme { INK } else { SOFT },
        );
    }
}

/// A weapon's derived numbers: nothing here is typed in; each comes from its parts.
fn weapon_numbers(im: &mut Img, s: &Session, y: i32, bottom: i32) {
    let p = s.weapon.powers();
    let st = &p.stats;
    centred(im, y, &format!("DAMAGE {}   SPEED {}", st.damage, st.speed), INK, false);
    centred(im, y + 10, &format!("REACH {:.1}   BLOCK {}", st.reach(), st.block), INK, false);
    centred(im, y + 20, &format!("SPELL POWER {}   MASS {:.1}", st.spell_power, st.mass()), INK, false);
    let gifts: Vec<String> = p
        .abilities
        .iter()
        .map(|a| format!("{} {}", a.id.replace(['_', ':'], " ").to_uppercase(), a.level))
        .collect();
    let text = if gifts.is_empty() { "NO GIFTS".to_string() } else { gifts.join(", ") };
    for (i, l) in wrap(&text, 32).into_iter().enumerate().take(((bottom - (y + 34)) / 9).max(0) as usize) {
        centred(im, y + 34 + i as i32 * 9, &l, SOFT, false);
    }
}

fn choice_row(im: &mut Img, s: &Session, row: usize, y: i32, row_h: i32) {
    let selected = s.row == row;
    if selected {
        for py in y - 2..y + row_h - 2 {
            for px in 19..SCROLL_W - 19 {
                if (px + py) % 2 == 0 {
                    im.put(px, py, 't');
                }
            }
        }
        im.line(18, y - 2, 18, y + row_h - 3, RED);
    }
    genie::seal(im, 23, y - 1, row, selected);
    let label = s.row_label(row).to_uppercase();
    if selected {
        bold(im, 40, y + 1, &label, INK);
    } else {
        im.text(40, y + 1, &label, INK);
    }
    if s.step == Step::Body && !s.forge() {
        let value = s.row_value(row);
        let amount = value.parse::<i32>().unwrap_or(50);
        let fill = if row == 5 {
            (amount + 30) * 100 / 60
        } else {
            amount
        };
        im.rect(116, y + 3, 59, 5, SOFT);
        im.rect(117, y + 4, 57, 3, 't');
        im.rect(117, y + 4, 57 * fill.clamp(0, 100) / 100, 3, 'o');
        im.rect(116 + 56 * fill.clamp(0, 100) / 100, y + 2, 3, 7, RED);
        im.text(207 - text_width(&value), y + 1, &value, INK);
    } else {
        let value = s.row_value(row).to_uppercase();
        let ink = if selected { RED } else { SOFT };
        // Parts and kinds get their own column, leaving the full name legible at 384.
        if matches!(s.step, Step::Parts | Step::Type) {
            let x = if s.step == Step::Type { 88 } else { 94 };
            im.text(x, y + 1, &value, ink);
        } else {
            im.text(210 - text_width(&value), y + 1, &value, ink);
        }
    }
}

fn power_gauge(im: &mut Img, s: &Session, y: i32) {
    let (used, budget) = s.power();
    bold(im, 22, y + 2, "POWER", INK);
    let count = format!("{used}/{budget}");
    let (x, w) = (63, 100);
    im.rect(x, y, w, 9, 'b');
    im.rect(x + 1, y + 1, w - 2, 7, 't');
    for dx in 0..(w - 2) * i32::from(used.min(budget)) / i32::from(budget) {
        im.rect(
            x + 1 + dx,
            y + 1,
            1,
            7,
            if used > budget {
                'p'
            } else if dx % 10 == 9 {
                'r'
            } else {
                'R'
            },
        );
        im.put(x + 1 + dx, y + 1, 'o');
    }
    im.text(
        211 - text_width(&count),
        y + 2,
        &count,
        if used > budget { 'p' } else { INK },
    );
}

fn life_words(im: &mut Img, s: &Session, footer: i32) {
    let c = &s.creation;
    let forge = s.forge();
    let (name, kind, mid, stats) = if forge {
        let w = &s.weapon;
        let p = w.powers();
        (
            w.name.to_uppercase(),
            format!("WEAPON / {}", weapon::kind_label(w.kind)),
            format!(
                "{} PARTS  {}",
                w.parts.len(),
                w.school().map_or("NO SPARK".to_string(), |g| format!("{} SPARK", g.name().to_uppercase()))
            ),
            format!("{} DMG  {} SPD  {:.1} REACH", p.stats.damage, p.stats.speed, p.stats.reach()),
        )
    } else {
        let st = c.stats();
        (
            c.name.to_uppercase(),
            format!("{} / UPRIGHT BIPED", c.role.name().to_uppercase()),
            format!(
                "{} PART{}  {} SPELL{}",
                c.parts.len(),
                if c.parts.len() == 1 { "" } else { "S" },
                c.spells.len(),
                if c.spells.len() == 1 { "" } else { "S" },
            ),
            format!("{} HP   {} ARMOUR", st.hp, st.armour),
        )
    };
    centred(im, 44, &name, INK, true);
    centred(im, 58, &kind, SOFT, false);
    centred(im, 75, &mid, INK, false);
    if footer > 100 && !(s.granted.is_some() && footer <= 125) {
        centred(im, 89, &stats, SOFT, false);
    }
    if footer > 125 {
        let (valid, spent, budget) = if forge {
            (s.weapon.validate(), s.weapon.spent(), WEAPON_BUDGET_SHOWN)
        } else {
            (c.validate(), c.spent(), c.role.budget())
        };
        if s.granted.is_some() {
            centred(im, 102, if forge { "A NEW EDGE, IN INK." } else { "A NEW SOUL, IN INK." }, RED, false);
            return; // Below is the wax seal's bay, above the power gauge.
        }
        let line = if valid.is_err() {
            "THE WISH NEEDS MENDING"
        } else {
            "A LITTLE INK. A LITTLE SOUL."
        };
        centred(im, 109, line, RED, false);
        if let Some(reason) = s.issue.as_ref().or(valid.as_ref().err()) {
            let line = if spent > budget {
                format!("REMOVE {} POWER TO SEAL", spent - budget)
            } else {
                reason.chars().take(30).collect::<String>().to_uppercase()
            };
            centred(im, 124, &line, RED, false);
        } else {
            centred(im, 124, "READY FOR THE GENIE", SOFT, false);
        }
    }
}

const WEAPON_BUDGET_SHOWN: u16 = crate::rules::creation::WEAPON_BUDGET;

/// The genie's few words beside the stage.
fn words(s: &Session) -> String {
    let (used, budget) = s.power();
    if s.chat.open && !s.chat.quip.is_empty() {
        return s.chat.quip.clone();
    }
    if s.granted.is_some() {
        return if s.forge() { "Shine, little blade!" } else { "Live, little one!" }.into();
    }
    if used > budget && s.step != Step::Role {
        return "Too grand! Trim it.".into();
    }
    if s.on_wish_row() {
        return "Say it with T!".into();
    }
    match s.step {
        Step::Role => s.creation.role.purpose().into(),
        Step::Body => "Give it a shape.".into(),
        Step::Type => "What kind of thing?".into(),
        Step::Parts if s.forge() => "Mind the joints!".into(),
        Step::Parts => "What can it do?".into(),
        Step::Paint => "Make it yours.".into(),
        Step::Powers if s.creation.role == Role::Folk => "Where do they stand?".into(),
        Step::Powers => "Mind the power!".into(),
        Step::Enchant => "A spark, perhaps?".into(),
        Step::Life => "Shall we begin?".into(),
        Step::Drive => "Take it for a walk!".into(),
    }
}

fn stage(im: &mut Img, session: &Session, tick: u64, h: i32, atlas: Option<&PreviewAtlas>) {
    let c = &session.creation;
    let forge = session.forge();
    let centre = 307;
    let foot = if forge { (h - 72).max(95) } else { (h - 76).max(91) };
    let mut name = Img::new(136, 28);
    paper(&mut name, 0, 0, 136, 28, 0x1afe);
    let label = if forge { session.weapon.name.to_uppercase() } else { c.name.to_uppercase() };
    let label = if label.chars().count() > 20 {
        format!("{}..", label.chars().take(18).collect::<String>())
    } else {
        label
    };
    centred(&mut name, 5, &label, INK, true);
    let sub = if forge {
        format!("WEAPON / {}", weapon::kind_label(session.weapon.kind))
    } else {
        format!("{}  /  BIPED", c.role.name().to_uppercase())
    };
    centred(&mut name, 16, &sub, SOFT, false);
    im.stamp(&name, 242, 26);
    // A low stone stage, from the same overhead camera as a boss's feet.
    im.ellipse(centre as f32, foot as f32 + 3.0, 52.0, 15.0, 'b');
    im.ellipse(centre as f32, foot as f32, 52.0, 14.0, 'P');
    im.ellipse(centre as f32, foot as f32 - 1.0, 49.0, 12.0, 'K');
    for dx in [-34, 0, 34] {
        im.line(centre + dx, foot + 10, centre + dx + 1, foot + 14, 'b');
    }
    let calm = super::viewer::calm();
    let age = session
        .granted
        .map(|at| if calm { 64 } else { tick.saturating_sub(at) });
    let mut reveal = age.map_or(1.0, |a| {
        ease::out(a.saturating_sub(REVEAL_TICK) as f32 / 14.0)
    });
    // What was just wished steps out of the ink again.
    if !calm && let Some(at) = session.chat.said_at {
        let since = tick.saturating_sub(at);
        if since < 16 && session.granted.is_none() {
            reveal = reveal.min(ease::out(since as f32 / 14.0));
        }
    }
    let (figure, caption) = if forge {
        let mark = if session.step == Step::Parts && !session.on_wish_row() {
            session.weapon.kind.slots().get(session.row).copied()
        } else {
            None
        };
        // Standing on its pommel, or lying along the slab: whichever shrinks it less.
        match weapon::display(&session.weapon, mark, (112, foot - 56), (128, foot - 62)) {
            Some((mut fitted, k)) => {
                if !calm {
                    weapon::shimmer(&mut fitted, tick, session.weapon.gem.or(session.weapon.school()));
                }
                (fitted, if k == 1 { "1:1".to_string() } else { format!("1:{k}") })
            }
            None => (Img::new(1, 1), "".to_string()),
        }
    } else {
        // Only a real stand-in is captioned as one: a finished character from the pipeline is named.
        let set = baked::for_creation(c);
        let caption = if atlas.is_some() {
            "YOUR CREATURE".to_string()
        } else if set.stand_in() {
            format!("STAND-IN: {}", set.name())
        } else {
            set.name().to_string()
        };
        (
            fit_preview(
                creature_preview(c, if calm { 0 } else { tick }, atlas),
                104,
                (foot - 66).max(24),
            ),
            caption,
        )
    };
    let mut pen = Pen::new(im, 2400);
    pen.ring(
        (centre as f32, foot as f32),
        41.0,
        1.0,
        0.25,
        ramps::GOLD.cooler(3),
        0.8,
    );
    if !calm && let Some(age) = age {
        pen.glyph(
            (centre as f32, foot as f32),
            44.0,
            0.28,
            0.0,
            ease::out(age as f32 / 18.0),
            6,
            ramps::GENIE.cooler(1),
            0.65,
            72,
        );
        for side in [-1.0, 1.0] {
            pen.wisp(
                (centre as f32 + side * 24.0, foot as f32),
                age as f32,
                (0.0, 26.0),
                28.0,
                58.0,
                5.0,
                ramps::GENIE,
                0x301 + (side + 1.0) as u32,
            );
        }
    }
    // A set whose standing frames are authored off the library's sole row says so in its record.
    // (a set with eight ways turns on the stage, and each way declares its own sole edge).
    let lower = if forge || atlas.is_some() {
        0
    } else {
        let set = baked::for_creation(c);
        set.drop_facing(Motion::Idle, set.turntable(if calm { 0 } else { tick }))
    };
    for y in 0..figure.h {
        for x in 0..figure.w {
            if let Some(rgb) = figure.get(x, y) {
                let dx = centre - figure.w / 2 + x;
                let dy = foot - figure.h + y + lower;
                if bayer(x, y) < reveal {
                    im.set(dx, dy, rgb);
                } else if age.is_some_and(|a| a >= 8) {
                    im.put(dx, dy, 'q');
                }
            }
        }
    }
    // Small engraving, clear about the borrowed figure during this slice. A weapon's names the part
    // the picker is on instead, and its ratio sits at the corner of the plate.
    let mut engraving = caption.clone();
    if forge {
        if !caption.is_empty() {
            let r = format!("HELD {caption}");
            im.text(376 - text_width(&r), 56, &r, 'o');
        }
        engraving = String::new();
        let w = &session.weapon;
        if session.step == Step::Parts && !session.on_wish_row()
            && let Some(slot) = w.kind.slots().get(session.row)
            && let Some(p) = w.chosen(slot).and_then(|p| kit().part(&p.part_id))
        {
            engraving = format!("{} +{}", part_name(p).to_uppercase(), w.part_cost(slot).unwrap_or(0));
        }
    }
    im.text(centre - 6 - text_width(&engraving) / 2, foot + if forge { 14 } else { 19 }, &engraving, if forge { 't' } else { 'o' });
    if session.step == Step::Body && !forge {
        let x = 257;
        let top = foot - figure.h;
        im.line(x, top, x, foot, 'P');
        for y in (top..=foot).step_by(5) {
            im.line(x - 2, y, x + 2, y, 'o');
        }
    }
    // The genie remains at the scroll's foot, the same painted character.
    let cheering = age.is_some_and(|a| a >= SEAL_TICK)
        || session.chat.said_at.is_some_and(|at| !calm && tick.saturating_sub(at) < 24 && session.chat.open);
    let mood = if cheering {
        genie::Mood::Cheer
    } else {
        genie::Mood::Talk
    };
    let genie = genie::figure(mood, if calm { 0 } else { tick }, 48);
    im.stamp(&genie, 375 - genie.w, h - genie.h - 2);
    let bubble = speech(&words(session));
    im.stamp(&bubble, 243, h - bubble.h - 9);
    if session.step == Step::Parts && !session.on_wish_row() && !forge {
        let slot = Slot::ALL[session.row.min(5)];
        if let Some(p) = c.chosen(slot).and_then(|p| part(&p.part_id)) {
            let label = format!("+{} POWER", p.cost);
            im.text(centre - text_width(&label) / 2, 60, &label, 't');
        }
    }
}

/// The same speech paper, with its little tail facing the genie beside it.
fn speech(words: &str) -> Img {
    let bubble = genie::bubble(words, 88);
    let height = bubble.h - 5;
    let mut out = Img::new(bubble.w + 5, height);
    for y in 0..height {
        for x in 0..bubble.w {
            if let Some(c) = bubble.get(x, y) {
                out.set(x, y, c);
            }
        }
    }
    for dx in 0..5 {
        let half = (4 - dx) / 2;
        out.rect(bubble.w - 1 + dx, height / 2 - half, 1, 2 * half + 1, 'T');
        out.put(bubble.w - 1 + dx, height / 2 - half - 1, 'b');
        out.put(bubble.w - 1 + dx, height / 2 + half + 1, 'b');
    }
    out
}

fn fit_preview(source: Img, max_w: i32, max_h: i32) -> Img {
    if source.w <= max_w && source.h <= max_h {
        return source;
    }
    let scale = (max_w as f32 / source.w as f32).min(max_h as f32 / source.h as f32);
    let (w, h) = (
        (source.w as f32 * scale) as i32,
        (source.h as f32 * scale) as i32,
    );
    let mut out = Img::new(w.max(1), h.max(1));
    for y in 0..out.h {
        for x in 0..out.w {
            if let Some(c) = source.get(x * source.w / out.w, y * source.h / out.h) {
                out.set(x, y, c);
            }
        }
    }
    out
}

#[cfg(test)]
#[path = "../../tests/render/creation__tests.rs"]
mod tests;
