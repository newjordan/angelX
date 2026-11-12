//! Who has the words. Every piece of text on a Bannerpeak screen is a box, and no two boxes may
//! meet and none may leave the frame: the banner cards, the calls, the title, the crown bar, the
//! prompts, the finish's panel and the bar across the top are claimed here in one order of rank,
//! and the signs and labels of the world, which rank last, are hung only where nothing else has
//! the ground, shifted in from the edge when they would run off it. The drawing reads these same
//! boxes, so what the test checks is what is drawn.
use super::super::super::ink::text_width;
use crate::rules::bannerpeak::{
    boss::Beat,
    knight::Knight,
    layout::{self, info, world},
    session::{ANNOUNCE_TICKS, BANNER_NAMES, Phase, Session},
    show::{Cry, SHOUT_LIFE},
    world::TILE,
};
use crate::rules::bannerpeak as rules;


use super::{H, W};

/// Ticks the title holds.
pub(super) const TITLE: u32 = 128;
/// The keys line, in ticks of the run.
pub(super) const KEYS: u32 = 420;

/// A box: left, top, width, height.
pub(crate) type Rect = (i32, i32, i32, i32);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Claim {
    pub what: &'static str,
    pub rect: Rect,
    /// The words of a prompt, for the stage to draw where the box is.
    pub text: &'static str,
}

/// The plate a line of words sits on, centred on the screen at row `y` (what `text_centre` draws).
fn plate(words: &str, y: i32) -> Rect {
    let w = text_width(words);
    ((W - w) / 2 - 1, y - 1, w + 2, 9)
}

pub(crate) fn meets(a: Rect, b: Rect) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

pub(crate) fn inside(r: Rect) -> bool {
    r.0 >= 0 && r.1 >= 0 && r.0 + r.2 <= W && r.1 + r.3 <= H
}

/// The pixels the kit's `banner` marks on a canvas `canvas_w` wide, whose origin is `(ox, oy)` on
/// the screen, `age` ticks in: the rails and the words and the line under them, as one box.
/// It reads the kit's own geometry (a slam makes the first ticks larger); the fade at the very
/// end lifts the banner a few pixels while it dissolves, and the placements leave room for it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn banner_rect(canvas_w: i32, (ox, oy): (i32, i32), y: i32, title: &str, sub: &str, age: u32, cap: i32) -> Rect {
    let tw = text_width(title).max(1);
    let rest = ((canvas_w - 24) / tw).clamp(1, cap.max(1));
    let slam = (3 - age as i32).max(0);
    let scale = (rest + slam).min(176 / tw).max(rest);
    let text_w = tw * scale + 4;
    let top = y - (scale - rest) * 3;
    let half = text_w / 2 + 8;
    let (mut x0, mut x1) = (canvas_w / 2 - half, canvas_w / 2 + half);
    let y0 = top.min(y - 5);
    let mut y1 = (top + 7 * scale + 4).max(y + 7 * rest + 10);
    if !sub.is_empty() && age >= 4 {
        let sw = text_width(sub) + 4;
        x0 = x0.min(canvas_w / 2 - sw / 2);
        x1 = x1.max(canvas_w / 2 - sw / 2 + sw);
        y1 = y1.max(y + 7 * rest + 12 + 11);
    }
    (ox + x0, oy + y0, x1 - x0, y1 - y0)
}

/// The calls on show now, loudest first, never two of a kind: `(y, cap, title, sub, age)`.
pub(crate) fn shouts(s: &Session, id: u32) -> Vec<(i32, i32, String, String, u32, Cry, u8)> {
    if s.phase != Phase::Play || s.elapsed < TITLE {
        return Vec::new();
    }
    let mut live: Vec<_> = s.show.shouts.iter().filter(|c| c.age(s.elapsed) < SHOUT_LIFE).collect();
    live.sort_by_key(|c| std::cmp::Reverse((c.cry.rank(), c.born)));
    let mut kinds: Vec<std::mem::Discriminant<Cry>> = Vec::new();
    live.retain(|c| {
        let d = std::mem::discriminant(&c.cry);
        let fresh = !kinds.contains(&d);
        kinds.push(d);
        fresh
    });
    // Out from under the card of a banner raised; the card has the top of the screen.
    let (top, gap, most) = if s.announce.is_some() { (84, 52, 1) } else { (66, 52, 2) };
    live.into_iter()
        .take(most)
        .enumerate()
        .map(|(n, c)| {
            let (t, sub) = words(c.cry, u32::from(c.who), id);
            (top + n as i32 * gap, if n == 0 { 2 } else { 1 }, t, sub, c.age(s.elapsed), c.cry, c.who)
        })
        .collect()
}

fn who(id: u32, seat: u32) -> String {
    if seat == 0 {
        String::new()
    } else if seat == id {
        "YOU".into()
    } else {
        format!("KNIGHT {seat}")
    }
}

/// What a call says: its words and the line under them.
pub(crate) fn words(cry: Cry, seat: u32, id: u32) -> (String, String) {
    match cry {
        Cry::Saved => ("SAVED!".into(), who(id, seat)),
        Cry::Lift => ("LIFT!".into(), who(id, seat)),
        Cry::Together(n) => ("ALL TOGETHER".into(), format!("{n} KNIGHTS")),
        Cry::Chain(n) => (format!("STOMP CHAIN {n}"), who(id, seat)),
        Cry::Plate => ("PLATE OFF!".into(), who(id, seat)),
        Cry::Crown => ("CROWN DOWN".into(), who(id, seat)),
        Cry::Hundred => ("100 SPARKS".into(), who(id, seat)),
    }
}

/// The crown bar over King Cinder: shown while he is awake, his crown is up, he is within sight and
/// nothing louder has the top of the screen.
pub(crate) fn crown_bar_on(s: &Session, me: Option<&Knight>) -> bool {
    let b = &s.boss;
    let Some(k) = me else { return false };
    s.phase == Phase::Play
        && s.announce.is_none()
        && s.elapsed >= TITLE
        && !matches!(b.beat, Beat::Dormant | Beat::Down)
        && (k.body.foot().0 - b.x).abs() <= 330
}

/// The crown bar's box and its name under it.
pub(crate) const CROWN_BAR: Rect = (150, 22, 84, 17);
pub(crate) fn crown_name() -> Rect {
    let w = text_width("KING CINDER");
    (192 - w / 2, 42, w, 7)
}

/// The scoreboard of the finish and the title beside the pole.
pub(crate) fn panel(seats: usize) -> Rect {
    (196, 64, 176, 20 + seats as i32 * 12)
}

pub(crate) fn finale_title_rect(words: &str, age: u32) -> Rect {
    banner_rect(232, (150, 24), 6, words, "", age, 2)
}

fn boxed(what: &'static str, rect: Rect) -> Claim {
    Claim { what, rect, text: "" }
}

fn prompt(what: &'static str, text: &'static str, y: i32) -> Claim {
    Claim { what, rect: plate(text, y), text }
}

/// Every box of words the screen is claiming over the world, for seat `id`. `words_on` is false for
/// the terminal's picture, which paints its own rows.
pub(crate) fn overlay(s: &Session, id: u32, me: Option<&Knight>, words_on: bool) -> Vec<Claim> {
    let mut out = Vec::new();
    // The bar across the top: what it prints and where.
    if let Some(k) = me {
        out.push(boxed("purse", (18, 3, text_width(&format!("x{}", k.coins)) + 1, 9)));
        if k.size != rules::knight::Size::Small {
            out.push(boxed("size", (68, 4, text_width(if k.size == rules::knight::Size::Flame { "FLAME" } else { "BIG" }), 8)));
        }
    }
    out.push(boxed("count", (212, 4, text_width(&format!("{}/8", s.count())), 8)));
    if s.seats.len() > 1 {
        for (n, (_, kn)) in s.seats.iter().enumerate() {
            let label = if kn.bubble.is_some() { "O".to_string() } else { format!("{}", kn.pips) };
            let (x, y) = (316 + (n as i32 % 3) * 22, 2 + (n as i32 / 3) * 9);
            out.push(boxed("seat", (x, y, 8 + text_width(&label), 8)));
        }
    }
    if s.elapsed < KEYS {
        let w = text_width(LEGEND);
        out.push(boxed("keys", ((W - w) / 2, 215, w, 7)));
    }
    if !words_on {
        return out;
    }
    match s.phase {
        Phase::Play => {
            if let Some(a) = s.announce {
                let age = ANNOUNCE_TICKS.saturating_sub(a.t);
                let name = BANNER_NAMES[usize::from(a.banner).min(7)];
                out.push(boxed("card", banner_rect(W, (0, 0), 30, name, &format!("BANNER {} OF 8", s.count()), age, 2)));
                // The pennants that climb a staff either side of the card (`moments::raised`).
                for side in [-1, 1] {
                    out.push(boxed("pennant", (192 + side * 118 - 2, 34, 19, 54)));
                }
            } else if s.elapsed < TITLE {
                out.push(boxed("title", banner_rect(W, (0, 0), 34, "EIGHT BANNERS", "RUN. JUMP. RAISE.", s.elapsed, 3)));
            }
            for (y, cap, t, sub, age, _, _) in shouts(s, id) {
                out.push(boxed("call", banner_rect(W, (0, 0), y, &t, &sub, age, cap)));
            }
            if crown_bar_on(s, me) {
                out.push(boxed("crown bar", CROWN_BAR));
                out.push(boxed("crown name", crown_name()));
            }
            if let Some(k) = me {
                if k.bubble.is_some() {
                    out.push(prompt("prompt", "FLOAT TO A FRIEND", 196));
                } else if s.announce.is_none() && s.elapsed > 150 && s.doors & 1 == 0 {
                    let (fx, fy) = k.body.foot();
                    let (ax, ay) = layout::cell_centre(info().arch);
                    if (fx - ax).abs() < 80 && (fy - ay).abs() < 40 {
                        out.push(prompt("prompt", "NEEDS TWO BANNERS", 26));
                    }
                }
            }
            if s.completed.is_some() {
                out.push(prompt("home", "E  HOME", 190));
            }
        }
        Phase::Select => out.push(boxed("board", (84, 38, 216, 138))),
        Phase::Finale => {
            let half = crate::rules::bannerpeak::session::FINALE_TICKS / 2 + 1;
            out.push(boxed("finish", finale_title_rect(finale_words(s.phase_t), s.phase_t.min(half) % half)));
            if s.phase_t > 70 {
                out.push(boxed("scores", panel(s.seats.len())));
            }
            if s.completed.is_some() {
                out.push(prompt("home", "E  HOME", 190));
            }
        }
    }
    out
}

pub(crate) const LEGEND: &str = "SPACE JUMP   S SLAM   F DIVE   E SHIELD";

/// The finish's title at tick `t` of it.
pub(crate) fn finale_words(t: u32) -> &'static str {
    if t < crate::rules::bannerpeak::session::FINALE_TICKS / 2 { "EIGHT BANNERS FLY" } else { "THE COURT IS LIT" }
}

/// A sign or a label of the world, where it hangs and what it says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Hung {
    /// Screen x of the post, the foot of the post, and the box of words.
    pub post: (i32, i32),
    pub rect: Rect,
    pub words: &'static str,
    /// A signboard on a post (else a bare label).
    pub board: bool,
}

/// The signs and labels in view that can hang without meeting any claim, each other, or the
/// edge: a board that would run off the frame is shifted in so its words are whole, and one that
/// would sit on other words or under the bar is left unhung for now.
pub(crate) fn hang(cam: (i32, i32), claims: &[Claim]) -> Vec<Hung> {
    let mut out: Vec<Hung> = Vec::new();
    let mut taken: Vec<Rect> = claims.iter().map(|c| c.rect).collect();
    let mut place = |post: (i32, i32), words: &'static str, w: i32, h: i32, up: i32, board: bool| {
        if post.0 < 3 || post.0 > W - 4 {
            return;
        }
        let bx = (post.0 - w / 2).clamp(2, W - 2 - w);
        let rect = (bx, post.1 - up, w, h);
        if rect.1 < 21 || rect.1 + rect.3 > 212 || taken.iter().any(|&t| meets(rect, t)) {
            return;
        }
        taken.push(rect);
        out.push(Hung { post, rect, words, board });
    };
    for &(cx, cy, g) in &info().signs {
        let (x, foot) = (cx * TILE + 8, (cy + 1) * TILE);
        let words = layout::sign_text(g);
        let w = text_width(words) + 6;
        place((x - cam.0, foot - cam.1), words, w, 14, 28, true);
    }
    for m in world().marks_of('j') {
        let (cx, _) = layout::cell_centre((m.x, m.y));
        let w = text_width("BELL") + 2;
        place((cx - cam.0, m.y * TILE - cam.1), "BELL", w, 9, 55, false);
    }
    out
}
