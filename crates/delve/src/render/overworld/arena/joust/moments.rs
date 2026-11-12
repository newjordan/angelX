//! The lists' strips and calls, drawn over the world: the two riders' chips
//! and the pass's board along the top, the title and the pass calls, the
//! herald's RIDE, the call of each meeting (what the lance did, where, who
//! went over), the points flying to the chips, the key prompts along the
//! foot. Plain words, two or three a prompt; every box of them claimed in
//! one ledger (`hud`) so nothing meets.

use super::super::super::chrome::{self, Tone};
use super::super::super::ink::{Img, text_width};
use super::super::juice::{self, Tone as Mood};
use super::super::pfx::{self, Pen, Shape, Spray, ramps};
use super::super::{NATIVE_H, NATIVE_W};
use super::frame::{Cx, Placed, placed, tip_of};
use super::hud::{self, Ledger, Rect};
use super::{finish, stage};
use crate::rules::joust::{Aim, Hit, Stage, Verdict};
use crate::rules::juice::Weight;

/// The chip along the top for one rider: his colour, his name, the passes he
/// has won and the knocks he can still take.
fn chip(im: &mut Img, led: &mut Ledger, cx: &Cx, side: u8, bump: f32) -> Rect {
    let j = cx.j;
    let name = cx.name(side, 10);
    let w = (text_width(&name) + 18).max(72);
    let x = if side == 0 { 3 } else { NATIVE_W - 3 - w };
    let r = (x, 2, w, 22);
    let col = if side == 0 { super::colours_of(cx.run, j.knight) } else { super::far_colours(cx.run, j).0 };
    led.claim("chip", r, &name);
    chrome::panel(im, r, Tone::CHIP);
    if bump > 0.0 {
        im.frame(r.0, r.1, r.2, r.3, '6');
    }
    // The colours: a little shield, and the name beside it.
    let sx = if side == 0 { x + 3 } else { x + w - 9 };
    shield(im, sx, 5, col.tabard, col.field);
    let tx = if side == 0 { x + 12 } else { x + w - 12 - text_width(&name) };
    let me = cx.side() == Some(side);
    im.text(tx, 6, &name, if me { '6' } else { 'i' });
    // Passes won and knocks left.
    let (won, left) = if side == 0 { (j.passes.0, j.balance.0) } else { (j.passes.1, j.balance.1) };
    let row = 15;
    let dir = if side == 0 { 1 } else { -1 };
    let origin = if side == 0 { x + 4 } else { x + w - 4 };
    for k in 0..2i32 {
        let px = origin + dir * (k * 7) - if side == 0 { 0 } else { 5 };
        shield(im, px, row, if k < i32::from(won) { col.tabard } else { 'g' }, if k < i32::from(won) { col.field } else { 'g' });
    }
    let pip = if left > 8 { 3 } else { 4 };
    let start = origin + dir * 18;
    for k in 0..left.min(12) as i32 {
        let px = start + dir * k * pip - if side == 0 { 0 } else { 3 };
        im.rect(px, row + 1, 3, 4, if side == 0 { 'T' } else { 'T' });
        im.put(px, row + 1, '6');
    }
    // The crowd's favour: scarves in the gold, one a knight who threw.
    let roar = if side == 0 { j.roar.0 } else { j.roar.1 };
    for k in 0..roar.min(4) as i32 {
        let px = if side == 0 { x + w - 6 - k * 5 } else { x + 3 + k * 5 };
        im.rect(px, 17, 4, 2, '5');
        im.put(px, 16, '4');
    }
    r
}

fn shield(im: &mut Img, x: i32, y: i32, tabard: char, field: char) {
    for (dy, (a, b)) in [(0, (0, 4)), (1, (0, 4)), (2, (0, 4)), (3, (0, 4)), (4, (1, 3)), (5, (2, 2))] {
        for dx in a..=b {
            im.put(x + dx, y + dy, if dx == 2 || dy == 2 { field } else { tabard });
        }
    }
    im.line(x, y, x + 4, y, 'v');
}

/// The pass's board between the chips.
fn board(im: &mut Img, led: &mut Ledger, cx: &Cx) -> Rect {
    let j = cx.j;
    let title = if j.stage == Stage::Done {
        "BOUT OVER".to_string()
    } else if j.course == 2 {
        "LAST PASS".to_string()
    } else {
        format!("PASS {} OF 3", j.course + 1)
    };
    let purse = if j.purse > 0 { format!("PURSE {}G", j.purse) } else { "FOR HONOUR".to_string() };
    let w = text_width(&title).max(text_width(&purse)) + 14;
    let r = ((NATIVE_W - w) / 2, 2, w, 22);
    led.claim("board", r, &format!("{title} {purse}"));
    chrome::panel(im, r, Tone::CHIP);
    im.text(r.0 + (w - text_width(&title)) / 2, 6, &title, 'T');
    im.text(r.0 + (w - text_width(&purse)) / 2, 15, &purse, if j.purse > 0 { 'O' } else { 'h' });
    r
}

/// The herald on his tower at the stands' left end, trumpet down or blown.
pub(super) fn herald(im: &mut Img, blowing: bool, tick: u32) {
    let (fx, fy) = (30, 76);
    // The tower: two posts and a platform, a pennon.
    im.rect(fx - 12, fy + 1, 26, 3, 'O');
    im.rect(fx - 12, fy + 4, 26, 2, 'r');
    im.rect(fx - 10, fy + 6, 3, 30, 'I');
    im.rect(fx + 8, fy + 6, 3, 30, 'I');
    im.line(fx - 10, fy + 8, fx + 10, fy + 32, 'b');
    // The man: hat and feather, face, a tabard in halves, hose.
    im.rect(fx - 2, fy - 24, 5, 3, 'p');
    im.line(fx + 2, fy - 25, fx + 4, fy - 28, 'c');
    im.rect(fx - 2, fy - 21, 5, 4, 'O');
    im.put(fx, fy - 20, 'k');
    im.rect(fx - 4, fy - 17, 5, 10, 'p');
    im.rect(fx + 1, fy - 17, 5, 10, 'u');
    im.rect(fx - 1, fy - 14, 3, 3, 'T');
    im.rect(fx - 3, fy - 7, 3, 7, 'p');
    im.rect(fx + 1, fy - 7, 3, 7, 'u');
    im.line(fx - 4, fy - 17, fx + 5, fy - 17, 'v');
    // The trumpet and the banner on it.
    if blowing {
        im.line(fx + 2, fy - 18, fx + 20, fy - 18, 'T');
        im.line(fx + 2, fy - 19, fx + 20, fy - 19, 'O');
        im.rect(fx + 20, fy - 22, 3, 8, 'T');
        im.rect(fx + 23, fy - 24, 1, 12, 'O');
        for k in 0..8 {
            im.put(fx + 8 + k, fy - 17 + k / 4, if (k / 2) % 2 == 0 { 'p' } else { 'T' });
            im.put(fx + 8 + k, fy - 16 + k / 4, if (k / 2) % 2 == 0 { 'T' } else { 'p' });
        }
        let mut pen = Pen::new(im, 120);
        let spray = Spray { gravity: -0.03, drag: 0.04, angle: -0.3, spread: 0.9, ..Spray::round(7, 3, ((fx + 25) as f32, (fy - 18) as f32), (0.8, 1.5), (14.0, 20.0)) };
        spray.each((tick % 22) as f32, |m| pfx::mote(&mut pen, &m, Shape::Note, ramps::GOLD, 1, tick as u64));
    } else {
        im.line(fx + 4, fy - 12, fx + 11, fy - 2, 'T');
        im.rect(fx + 10, fy - 3, 3, 3, 'O');
        for k in 0..8 {
            im.put(fx + 9 + k / 2, fy - 1 + k / 2, if (k / 2) % 2 == 0 { 'p' } else { 'T' });
        }
    }
}

/// The key prompts and what the knight holds on the bout, along the foot.
fn foot(im: &mut Img, led: &mut Ledger, cx: &Cx) {
    let j = cx.j;
    let cxm = NATIVE_W / 2;
    let (status, items): (Option<String>, Vec<(&[&str], &str)>) = match (cx.side(), j.stage) {
        (Some(s), Stage::Ready) => {
            let spurred = if s == 0 { j.spurred } else { j.foe.as_ref().is_some_and(|f| f.spurred) };
            if spurred {
                (Some("READY".into()), vec![])
            } else if j.course == 0 {
                (None, vec![(&["A", "D"][..], "HORSE"), (&["F"][..], "SPUR")])
            } else if j.foe.is_none() {
                (None, vec![(&["F"][..], "SPUR"), (&["SPC"][..], "HOLD LEAVE")])
            } else {
                (None, vec![(&["F"][..], "SPUR")])
            }
        }
        (Some(_), Stage::Charge) if j.hits.is_none() => {
            if j.unset() {
                (None, vec![(&["W", "S"][..], "AIM"), (&["E"][..], "SHIELD")])
            } else {
                (None, vec![(&["F"][..], "STRIKE")])
            }
        }
        (None, _) if cx.me.is_some_and(|id| j.gallery.contains_key(&id)) => {
            let id = cx.me.unwrap_or(0);
            let w = &j.gallery[&id];
            let backing = w.side.map(|s| format!("{}G ON {}", w.stake, cx.name(s, 10)));
            match (j.stage, j.course) {
                (Stage::Ready, 0) => (backing, vec![(&["A", "D"][..], "BACK"), (&["F"][..], "RAISE")]),
                (Stage::Charge, _) if w.side.is_some() && !w.threw && j.t < j.meeting() => (backing, vec![(&["F"][..], "CHEER")]),
                (Stage::Charge | Stage::Ready, _) => (backing.map(|b| if w.threw { format!("{b} CHEERED") } else { b }), vec![]),
                _ => (None, vec![]),
            }
        }
        _ => (None, vec![]),
    };
    if let Some(s) = status {
        let w = text_width(&s) + 10;
        let r = (cxm - w / 2, 196, w, 11);
        if led.free(r) {
            led.claim("status", r, &s);
            chrome::panel(im, r, Tone::CHIP);
            im.text(r.0 + 5, r.1 + 2, &s, 'T');
        }
    }
    if !items.is_empty() {
        let lit = j.striking();
        let _ = hud::prompts(led, im, cxm, 208, &items, lit);
    }
}

/// The wagers on the bench: each gallery knight's name and stake beside
/// him, where there is room.
fn tags(im: &mut Img, led: &mut Ledger, cx: &Cx) {
    for (id, x) in super::frame::gallery_spots(cx) {
        let Some(w) = cx.j.gallery.get(&id) else { continue };
        let name = hud::called(cx.run, id, cx.me, 8);
        let line = match w.won {
            Some(true) => format!("+{}G", w.stake),
            Some(false) => format!("-{}G", w.stake),
            None if w.stake > 0 => format!("{}G", w.stake),
            None => "?".to_string(),
        };
        let text = format!("{name} {line}");
        // Centred under his seat, on a plate just under the stands' rail.
        let at = (x - (text_width(&text) + 7) / 2, 90);
        let who = match w.won {
            Some(true) => '5',
            Some(false) => 'h',
            None if Some(id) == cx.me => '6',
            None => 'i',
        };
        led.plate(im, "gallery", at, &text, who);
    }
}

/// How the viewer's seat reads a lance's hit: the call's tone.
fn mood(w: Weight, mine: bool) -> Mood {
    match (w, mine) {
        (Weight::Graze, _) => Mood::Steel,
        (_, true) => Mood::Warm,
        (_, false) => Mood::Hot,
    }
}

fn hit_word(hit: Hit, aim: Aim) -> &'static str {
    match hit {
        Hit::Miss => "MISSED",
        _ => hit.flash(aim),
    }
}

/// The call of a meeting: what the lance did, who was unhorsed, who the
/// crowd steadied; the points popping and flying to the chips.
fn call(im: &mut Img, led: &mut Ledger, cx: &Cx, placed: &[Placed; 2]) {
    let Some((m, age)) = cx.meet() else { return };
    let life = 46;
    if age >= life {
        return;
    }
    let me = cx.side();
    // The rider the call is about: the viewer's lance, or the heavier.
    let (mine, theirs) = m.hits;
    let focus = match me {
        Some(s) => s,
        None => u8::from(m.taken.1 > m.taken.0),
    };
    let (own, own_aim, other, other_aim) = if focus == 0 { (mine, m.aims.0, theirs, m.aims.1) } else { (theirs, m.aims.1, mine, m.aims.0) };
    let who_down = if m.down.0 {
        Some(0u8)
    } else if m.down.1 {
        Some(1)
    } else {
        None
    };
    let (title, sub, tone) = if let Some(d) = who_down {
        let name = cx.name(d, 10);
        let sub = if cx.side() == Some(d) { "YOU ARE DOWN".to_string() } else { format!("{name} IS DOWN") };
        ("UNHORSED".to_string(), sub, if me == Some(d) { Mood::Hot } else { Mood::Warm })
    } else if own == Hit::Miss && other == Hit::Miss {
        ("BOTH MISS".to_string(), String::new(), Mood::Steel)
    } else {
        let t = hit_word(own, own_aim).to_string();
        let other_name = cx.name(1 - focus, 9);
        let sub = if other != Hit::Miss && own != Hit::Miss {
            format!("{other_name} {}", hit_word(other, other_aim))
        } else if m.steady.0 || m.steady.1 {
            let s = u8::from(m.steady.1);
            format!("{} STEADIED", cx.name(s, 9))
        } else {
            String::new()
        };
        let w = if focus == 0 { m.taken.1 } else { m.taken.0 };
        (t, sub, mood(if own == Hit::Miss { Weight::Graze } else { w }, true))
    };
    led.banner(im, "call", 40, &title, &sub, age, life, tone, 3);
    // The points pop over the struck rider and fly to the striker's chip.
    for (striker, hit) in [(0u8, mine), (1, theirs)] {
        let pts = hit.points();
        if pts == 0 {
            continue;
        }
        let struck = &placed[usize::from(1 - striker)];
        let at = (struck.x, struck.ground - 52);
        led.pop(im, "pop", &format!("+{pts}"), at, age, 26, 2, if striker == 0 { 'T' } else { 'O' });
        if age >= 8 {
            let chip_x = if striker == 0 { 3 + 36 } else { NATIVE_W - 3 - 36 };
            led.fly(im, "fly", &format!("+{pts}"), at, (chip_x, 14), age - 8, 20, 'T');
        }
    }
}

/// The title at the bout's start, the pass calls and the herald's RIDE.
fn announce(im: &mut Img, led: &mut Ledger, cx: &Cx) -> bool {
    let j = cx.j;
    let mut blow = false;
    if cx.t < 96 && j.course == 0 && j.stage == Stage::Ready {
        let sub = format!("{} AGAINST {}", cx.name(0, 10), cx.name(1, 10));
        led.banner(im, "title", 110, "THE LISTS", &sub, cx.t.saturating_sub(4), 88, Mood::Warm, 3);
        blow = cx.t < 40;
    } else if j.stage == Stage::Ready && j.course > 0 && j.t < 56 {
        let title = if j.course == 2 { "LAST PASS".to_string() } else { format!("PASS {}", j.course + 1) };
        let sub = if j.passes.0 == j.passes.1 { "ALL SQUARE".to_string() } else { String::new() };
        led.banner(im, "pass", 110, &title, &sub, j.t, 56, Mood::Steel, 3);
    }
    if j.stage == Stage::Charge && j.t < 26 {
        led.count(im, "ride", (NATIVE_W / 2, 122), "RIDE", j.t, 26, true);
        blow = true;
    }
    blow
}

/// What the lists put over the world.
pub(super) fn draw(im: &mut Img, cx: &Cx, led: &mut Ledger) {
    let j = cx.j;
    let placed = placed(cx);
    // The fixed strips claim their places first.
    let bump_of = |side: u8| {
        cx.meet().map_or(0.0, |(m, age)| {
            let hit = if side == 0 { m.hits.0 } else { m.hits.1 };
            if hit.points() > 0 && age >= 28 { juice::bump(age - 28, 8) } else { 0.0 }
        })
    };
    chip(im, led, cx, 0, bump_of(0));
    chip(im, led, cx, 1, bump_of(1));
    board(im, led, cx);
    foot(im, led, cx);
    // The herald, and whether he is blowing.
    let blow_now = announce(im, led, cx)
        || cx.meet().is_some_and(|(m, age)| age < 24 && m.landed())
        || (j.stage == Stage::Done && j.t < 30);
    herald(im, blow_now, cx.tick);
    call(im, led, cx, &placed);
    tags(im, led, cx);
    // The strike: the ring over the tilt, closing, gold when it is time.
    if let Some(k) = j.closing() {
        let (a, b) = (tip_of(&placed[0]), tip_of(&placed[1]));
        let at = ((a.0 + b.0) / 2, (a.1 + b.1) / 2);
        let _ = at;
        let ringc = ((j.meeting_x() * super::frame::UNIT) as i32, stage::TILT_Y - 4);
        super::fx::closing_ring(im, ringc, k, j.striking(), cx.tick);
    }
    // The end of the bout: the iris closes on the lists before the podium.
    if j.stage == Stage::Done && j.t >= finish::HOLD {
        let k = ((j.t - finish::HOLD) as f32 / finish::FADE as f32).min(1.0);
        juice::iris(im, (NATIVE_W / 2, NATIVE_H / 2 - 10), k, 'k');
    }
    // The bout opens out of black.
    if cx.t < 16 {
        juice::iris(im, (NATIVE_W / 2, NATIVE_H / 2), 1.0 - cx.t as f32 / 16.0, 'k');
    }
    let _ = (NATIVE_H, Verdict::Won);
}
