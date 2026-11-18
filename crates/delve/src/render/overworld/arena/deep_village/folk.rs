//! The deep people themselves. Each people has its own figure in ink rows
//! (a cap and a round little body; a miner's helmet with its lamp and a
//! beard; soot-dark hair, goggles and a leather apron; a lamplighter's hood
//! with a lantern in hand), coloured six ways for the six who speak, and
//! shortened for their children.
//!
//! As the star falls they look up; when it has landed they come out to the
//! plaza and stand round where it came down, looking, then go back to
//! their doors. They turn to a knight who comes near, bob while they wait,
//! hop when the village cheers, and the children run round the heart once
//! the village is bright enough to play in. In the dark they are dark too.

use super::super::super::ink::{BLACK, Img};
use super::super::super::light::step_down;
use super::super::{at, near_prompt, stand};
use super::px;
use crate::rules::{
    Run,
    deep::{Culture, Village, cultures::WISHKEEPER, village as v},
};

fn rows(culture: Culture) -> (&'static [&'static str], usize) {
    match culture {
        Culture::Capfolk => (
            &[
                "....RRRR....",
                "..RRoRRTRR..",
                ".RRTRRRRRoR.",
                "RRRRRRTRRRRR",
                "BBBBBBBBBBBB",
                "...tTTTTt...",
                "...TKTTKT...",
                "...TTTTTT...",
                "....tTTt....",
                "...AAAAAA...",
                "..TAAAAAAT..",
                "...AAAAAA...",
                "...AA..AA...",
                "...nn..nn...",
            ],
            6,
        ),
        Culture::Glimmerkin => (
            &[
                "....GGGG....",
                "...GG66GG...",
                "..GGG66GGG..",
                "..jjjjjjjj..",
                "...TTTTTT...",
                "...TKTTKT...",
                "..iTTTTTTi..",
                "..iiTiiTii..",
                "...iiiiii...",
                "..UUiiiiUU..",
                ".TUUUUUUUUT.",
                "..PPPPPPPP..",
                "...UU..UU...",
                "...bb..bb...",
            ],
            5,
        ),
        Culture::Emberfolk => (
            &[
                "...KKKKKK...",
                "..KKKKKKKK..",
                "..KzzKKzzK..",
                "..tttttttt..",
                "..tKttttKt..",
                "..tttttttt..",
                "...ttRRtt...",
                "..RRIIIIRR..",
                ".tRIIIIIIRt.",
                ".t.IIIIII.t.",
                "...IIIIII...",
                "...IIIIII...",
                "...nn..nn...",
                "...KK..KK...",
            ],
            4,
        ),
        Culture::Lampwick => (
            &[
                "....QQQQ....",
                "...QQQQQQ...",
                "..QQTTTTQQ..",
                "..QTKTTKTQ..",
                "..QTTTTTTQ..",
                "..QQTTTTQQ..",
                "..uuuuuuuu..",
                ".uuuuuuuuuu.",
                ".uuuuuuuuuuB",
                ".uuuuuuuuu.6",
                "..uuuuuuuu.5",
                "..uuuuuuuu..",
                "...bb..bb...",
                "...nn..nn...",
            ],
            3,
        ),
        Culture::Roostfolk => (
            &[
                "...bbbbbb...",
                "..bBBBBBBb..",
                ".bbzzbbzzbb.",
                "..tTTTTTTt..",
                "..TTKTTKTT..",
                "...TTTTTT...",
                "..RRRRRRRR..",
                ".pPIIIIIIPp.",
                "pPIIIIIIIIPp",
                "p.IIIIIIII.p",
                "...IIIIII...",
                "...II..II...",
                "...bb..bb...",
                "...nn..nn...",
            ],
            4,
        ),
        Culture::Rootkin => (
            &[
                "...l.AA.l...",
                "...AlMAlA...",
                "....oOOo....",
                "...oOOOOo...",
                "...OKOOKO...",
                "...oOOOOo...",
                "....oooo....",
                "...BBBBBB...",
                "..oBBBBBBo..",
                "..o.BBBB.o..",
                "....BBBB....",
                "....B..B....",
                "...bb..bb...",
                "..b.b..b.b..",
            ],
            4,
        ),
        Culture::Chimefolk => (
            &[
                ".....SS.....",
                "....SSSS....",
                "...SSSSSS...",
                "..SSTTTTSS..",
                "..STKTTKTS..",
                "..SSTTTTSS..",
                "..jSSSSSSj..",
                ".jjjjjjjjjj.",
                ".Tjjjjjjjjo.",
                "..jjjjjjjjO.",
                "..jjjrrjjjo.",
                "..jjjjjjjj..",
                "..jjjjjjjj..",
                "...nn..nn...",
            ],
            4,
        ),
        Culture::Saltkin => (
            &[
                "...iiiiii...",
                "..iiHHHHii..",
                "..iTTTTTTi..",
                "..iTKTTKTi..",
                "..iTTTTTTi..",
                "...HTTTTH...",
                "...HHiHHH...",
                "..JJJJJJJJ..",
                ".TJJJJJJJJT.",
                "..JJbbbbJJ..",
                "..JJJJJJJJ..",
                "..JJJJJJJJ..",
                "...JJ..JJ...",
                "...nn..nn...",
            ],
            3,
        ),
    }
}

fn colours(culture: Culture, who: usize) -> &'static [(char, char)] {
    match (culture, who % 6) {
        (Culture::Capfolk, 0) => &[('A', 'J')],
        (Culture::Capfolk, 1) => &[('R', 'Q'), ('o', 'z'), ('B', 'q'), ('A', 'e')],
        (Culture::Capfolk, 2) => &[('R', 'r'), ('o', 'O'), ('B', 'b'), ('A', 'l')],
        (Culture::Capfolk, 3) => &[('A', 'M')],
        (Culture::Capfolk, 4) => &[('R', 'p'), ('o', 'R'), ('A', 'y')],
        (Culture::Capfolk, _) => &[('R', 'q'), ('o', 'Q'), ('B', 's'), ('A', 'Q')],
        (Culture::Glimmerkin, 0) => &[('G', 'O'), ('j', 'o'), ('U', 'B')],
        (Culture::Glimmerkin, 1) => &[('U', 'q'), ('i', 'r')],
        (Culture::Glimmerkin, 2) => &[('U', 'p'), ('i', 'B')],
        (Culture::Glimmerkin, 3) => &[('U', 'e'), ('i', 'h')],
        (Culture::Glimmerkin, 4) => &[('U', 'S'), ('i', 'T')],
        (Culture::Glimmerkin, _) => &[('U', 'Q'), ('i', 'W'), ('G', 'u')],
        (Culture::Emberfolk, 0) => &[('I', 'p'), ('K', 'h')],
        (Culture::Emberfolk, 1) => &[('I', 'B')],
        (Culture::Emberfolk, 2) => &[('I', 'P'), ('K', 'B')],
        (Culture::Emberfolk, 3) => &[('I', 'I'), ('t', 'o')],
        (Culture::Emberfolk, 4) => &[('I', 'r')],
        (Culture::Emberfolk, _) => &[('I', 'X'), ('K', 'i')],
        (Culture::Lampwick, 0) => &[('Q', 'S'), ('u', 'S')],
        (Culture::Lampwick, 1) => &[('Q', 'x'), ('u', 'x')],
        (Culture::Lampwick, 2) => &[('Q', 'q'), ('u', 'q')],
        (Culture::Lampwick, 3) => &[('Q', 'B'), ('u', 'b')],
        (Culture::Lampwick, 4) => &[('Q', 'u'), ('u', 'U')],
        (Culture::Lampwick, _) => &[('Q', 'Q'), ('u', 'z')],
        (Culture::Roostfolk, 0) => &[('R', 'j'), ('I', 'g'), ('B', 'X'), ('P', 'G')],
        (Culture::Roostfolk, 1) => &[('R', 'R')],
        (Culture::Roostfolk, 2) => &[('R', 'A'), ('I', 'B')],
        (Culture::Roostfolk, 3) => &[('I', 'K'), ('P', 'X'), ('R', 'o'), ('p', 'g')],
        (Culture::Roostfolk, 4) => &[('R', 'Q'), ('I', 'p')],
        (Culture::Roostfolk, _) => &[('I', 'S'), ('P', 'u'), ('R', 'h'), ('p', 'x')],
        (Culture::Rootkin, 0) => &[('A', 'E'), ('M', 'l'), ('o', 'b'), ('O', 'I'), ('B', 'n')],
        (Culture::Rootkin, 1) => &[('B', 'R')],
        (Culture::Rootkin, 2) => &[('A', 'N'), ('M', 'L'), ('B', 'p'), ('O', 'P')],
        (Culture::Rootkin, 3) => &[('B', 'I'), ('A', 'l')],
        (Culture::Rootkin, 4) => &[('B', 'e'), ('A', 'M'), ('M', 'Y')],
        (Culture::Rootkin, _) => &[('B', 'r'), ('A', 'y'), ('M', 'C')],
        (Culture::Chimefolk, 0) => &[('S', 'p'), ('j', 'B')],
        (Culture::Chimefolk, 1) => &[('S', 'x'), ('j', 'S')],
        (Culture::Chimefolk, 2) => &[('S', 'K'), ('j', 'X')],
        (Culture::Chimefolk, 3) => &[('S', 'u'), ('j', 'x')],
        (Culture::Chimefolk, 4) => &[('S', 'e'), ('j', 'e')],
        (Culture::Chimefolk, _) => &[('S', 'h'), ('j', 'J')],
        (Culture::Saltkin, 0) => &[('i', 'H'), ('J', 'h')],
        (Culture::Saltkin, 1) => &[('J', 'G'), ('i', 'J')],
        (Culture::Saltkin, 2) => &[('J', 'Q'), ('i', 'W')],
        (Culture::Saltkin, 3) => &[('J', 'j'), ('i', 'h')],
        (Culture::Saltkin, 4) => &[('J', 'z'), ('i', 'i')],
        (Culture::Saltkin, _) => &[('J', 't'), ('i', 'T')],
    }
}

pub(super) fn figure(culture: Culture, who: usize, up: bool, child: bool) -> Img {
    coloured(culture, colours(culture, who), up, child)
}

fn coloured(culture: Culture, pairs: &[(char, char)], up: bool, child: bool) -> Img {
    let (base, eyes) = rows(culture);
    let mut out: Vec<String> = base.iter().map(|r| r.to_string()).collect();
    if up {
        let (row, above) = (out[eyes].clone(), out[eyes - 1].clone());
        let skin = row.chars().find(|&c| c != '.' && c != 'K').unwrap_or('T');
        out[eyes - 1] = row
            .chars()
            .zip(above.chars())
            .map(|(e, a)| if e == 'K' { 'K' } else { a })
            .collect();
        out[eyes] = row.chars().map(|c| if c == 'K' { skin } else { c }).collect();
    }
    if child {
        let drop = [eyes + 2, eyes + 4];
        out = out.into_iter().enumerate().filter(|(i, _)| !drop.contains(i)).map(|(_, r)| r).collect();
    }
    let refs: Vec<&str> = out.iter().map(String::as_str).collect();
    Img::from_rows(&refs).recolor(pairs)
}

fn dimmed(im: &Img, steps: usize) -> Img {
    if steps == 0 {
        return im.clone();
    }
    let mut out = im.clone();
    for y in 0..im.h {
        for x in 0..im.w {
            if let Some(c) = im.get(x, y).filter(|&c| c != BLACK) {
                out.set(x, y, step_down(c, steps));
            }
        }
    }
    out
}

use crate::motion::ease::both as ease;

fn gathered(home: (i32, i32), slot: usize, slots: usize, since: u64) -> ((i32, i32), bool) {
    let (lx, ly) = px(v::LANDING);
    let side = if slot % 2 == 0 { -1 } else { 1 };
    let rank = (slot / 2) as f32 / (slots / 2).max(1) as f32;
    let ring = (lx + side * (34 + (rank * 74.0) as i32), ly + 6 - (rank * 10.0) as i32);
    let t = since.saturating_sub(v::FALL) as f32;
    let span = v::GATHER as f32;
    let out = ease(t / 45.0);
    let back = ease((t - (span - 45.0)) / 45.0);
    let f = (out - back).clamp(0.0, 1.0);
    let at = (home.0 + ((ring.0 - home.0) as f32 * f) as i32, home.1 + ((ring.1 - home.1) as f32 * f) as i32);
    (at, f > 0.0 && f < 1.0)
}

pub(super) fn all(cv: &mut Img, run: &Run, vl: &Village) {
    let tick = run.tick as u32;
    let since = run.tick.saturating_sub(vl.arrived);
    let falling = since < v::FALL;
    let gathering = !falling && since < v::FALL + v::GATHER;
    let cheering = run.tick < vl.cheer;
    let dark = ((1.0 - vl.light()) * 2.2).round() as usize;
    let (lx, _) = px(v::LANDING);
    for (who, &spot) in v::SPEAKERS.iter().enumerate() {
        let home = px(spot);
        let (pos, walking) = if gathering { gathered(home, who, 9, since) } else { (home, false) };
        let near = run
            .players
            .values()
            .filter(|h| h.hp > 0)
            .map(|h| at(h.x, h.y))
            .min_by_key(|&(hx, hy)| (hx - pos.0).abs() + (hy - pos.1).abs());
        let toward = if falling || gathering { lx } else { near.map_or(pos.0, |p| p.0) };
        let up = falling || (gathering && since < v::FALL + 70);
        let im = dimmed(&figure(vl.culture, who, up, false), dark);
        let im = if toward < pos.0 { im.flip_h() } else { im };
        let hop = if cheering || (walking && (tick / 4) % 2 == 0) {
            [0, 2, 3, 2][((tick / 3 + who as u32) % 4) as usize]
        } else {
            ((tick / 40 + who as u32) % 6 == 0) as i32
        };
        stand(cv, &im, pos.0, pos.1 - hop);
        accessory(cv, vl.culture, who, pos, hop, toward < pos.0, tick);
        if run.deep.as_ref().and_then(|d| d.village.as_ref()).is_some_and(|vg| vg.talks.values().any(|t| usize::from(t.who) == who)) {
            bubble(cv, pos.0, pos.1 - im.h - 6 - hop, tick);
        } else if !falling && !gathering {
            near_prompt(cv, run, (pos.0, pos.1 + 34), if who == WISHKEEPER { "WISH FOR THE CITY" } else { "TALK" });
        }
    }
    let found = run.home.deep.town(vl.floor).is_some_and(|t| t.deeds.contains(&crate::rules::deep::Errand::Rescue));
    if found && !gathering && !falling {
        reunion(cv, run, vl, dark);
    }
    let (hx, hy) = px((18.0, 16.4));
    for k in 0..5usize {
        let play = vl.hope >= 4 && !falling;
        let (x, y) = if gathering {
            gathered(px(child_home(k)), k + 6, 11, since).0
        } else if play {
            let a = tick as f32 * 0.045 + k as f32 * std::f32::consts::TAU / 5.0;
            (hx + (a.cos() * 62.0) as i32, hy + 10 + (a.sin() * 14.0) as i32)
        } else {
            px(child_home(k))
        };
        let up = falling || gathering;
        let im = dimmed(&figure(vl.culture, (k + 1) % 6, up, true), dark);
        let im = if (tick / 30 + k as u32).is_multiple_of(2) { im } else { im.flip_h() };
        let hop = if play || cheering { [0, 1, 2, 1][((tick / 3 + k as u32) % 4) as usize] } else { 0 };
        stand(cv, &im, x, y - hop);
    }
}

fn reunion(cv: &mut Img, run: &Run, vl: &Village, dark: usize) {
    let tick = run.tick as u32;
    let giver = run
        .home
        .deep
        .missions
        .iter()
        .find(|m| m.village == vl.floor && m.errand == crate::rules::deep::Errand::Rescue)
        .map_or(3, |m| usize::from(m.giver) % v::SPEAKERS.len());
    let (gx, gy) = px(v::SPEAKERS[giver]);
    let beside = (gx + if gx > 288 { 11 } else { -11 }, gy + 1);
    let door = px((v::SPEAKERS[0].0 - 2.4, v::SPEAKERS[0].1 + 0.3));
    let (pos, pose) = match vl.reunion.map(|t| run.tick as i64 - t as i64) {
        Some(age) if age < 0 => (door, Lost::Looking),
        Some(age) if age < v::RUN_HOME as i64 => {
            let f = ease(age as f32 / v::RUN_HOME as f32);
            let at = (door.0 + ((beside.0 - door.0) as f32 * f) as i32, door.1 + ((beside.1 - door.1) as f32 * f) as i32);
            (at, Lost::Running)
        }
        Some(age) if age < (v::RUN_HOME + v::REUNION) as i64 => ((beside.0 + if beside.0 > gx { -4 } else { 4 }, beside.1), Lost::Hugging((age - v::RUN_HOME as i64) as u32)),
        _ => (beside, Lost::Home),
    };
    let im = dimmed(&lost_figure(vl.culture, pose, tick), dark);
    let im = if pos.0 > gx { im.flip_h() } else { im };
    let hop = match pose {
        Lost::Running => [0, 2, 3, 2][(tick / 3 % 4) as usize],
        Lost::Hugging(t) => [0, 1, 2, 1][(t / 4 % 4) as usize],
        _ => ((tick / 24) % 5 == 0) as i32,
    };
    stand(cv, &im, pos.0, pos.1 - hop);
    if let Lost::Hugging(t) = pose {
        for k in 0..5u32 {
            let life = 40;
            let a = (t + k * 9) % life;
            if t + k * 9 < life * 3 {
                let hx = (pos.0 + gx) / 2 + ((k as f32 * 2.1 + a as f32 * 0.15).sin() * 6.0) as i32;
                heart(cv, hx, pos.1 - im.h - 4 - a as i32 / 2, a > life * 2 / 3);
            }
        }
    } else if pose == Lost::Home && (tick / 60) % 4 == 0 {
        heart(cv, pos.0, pos.1 - im.h - 5, false);
    }
}

fn heart(cv: &mut Img, x: i32, y: i32, fading: bool) {
    let ink = if fading { 'c' } else { '7' };
    cv.put(x - 1, y, ink);
    cv.put(x + 1, y, ink);
    cv.rect(x - 2, y + 1, 5, 1, ink);
    cv.rect(x - 1, y + 2, 3, 1, ink);
    cv.put(x, y + 3, ink);
    if !fading {
        cv.put(x - 1, y + 1, 'c');
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Lost {
    Huddled,
    Waving,
    Glad,
    Looking,
    Running,
    Hugging(u32),
    Home,
}

pub(super) fn lost_figure(culture: Culture, pose: Lost, tick: u32) -> Img {
    let base = coloured(culture, lost_colours(culture), matches!(pose, Lost::Glad | Lost::Looking), true);
    let bedraggled = !matches!(pose, Lost::Home);
    let (bw, bh) = (base.w, base.h);
    let huddled = pose == Lost::Huddled;
    let (pad, top) = (7, 6);
    let mut im = Img::new(bw + pad * 2, bh + top);
    let eyes = rows(culture).1 as i32;
    let shown = if huddled { (eyes + 3).min(bh) } else { bh };
    let lift = if huddled { bh - shown - 4 } else { 0 };
    for y in 0..shown {
        for x in 0..bw {
            if let Some(c) = base.get(x, y) {
                im.set(x + pad, y + top + lift, c);
            }
        }
    }
    let ox = pad;
    let oy = top + lift;
    if bedraggled {
        for y in 0..bh {
            for x in 0..bw {
                if let Some(c) = im.get(x + ox, y + oy).filter(|_| super::super::super::ink::hash(x, y, 9550 + culture as u32) % 6 == 0 && y > 2) {
                    im.set(x + ox, y + oy, step_down(c, 1));
                }
            }
        }
        if !huddled {
            for x in [2, 4, 7, 9] {
                im.clear(x + ox, oy + bh - 3);
            }
        }
        im.put(ox + 4, oy + bh / 2 + 1, 'b');
        im.put(ox + 5, oy + bh / 2 + 1, 'b');
        for x in 3..9 {
            if im.get(x + ox, oy + 2).is_some() {
                im.put(x + ox, oy + 2, if x % 2 == 0 { 'H' } else { 'i' });
            }
        }
    }
    let mid = oy + bh / 2;
    let hand = |up: bool| if up { (oy - 3, oy - 5) } else { (mid, mid + 2) };
    match pose {
        Lost::Huddled => {
            let ky = oy + shown;
            im.rect(ox + 2, ky, 8, 3, 'b');
            im.line(ox + 2, ky, ox + 9, ky, 'I');
            im.line(ox + 1, ky + 1, ox + 10, ky + 1, 'T');
            im.line(ox + 2, ky + 3, ox + 9, ky + 3, 'n');
        }
        Lost::Waving => {
            let flap = (tick / 6) % 2 == 0;
            im.line(ox + bw - 2, mid - 1, ox + bw + 2, if flap { oy - 4 } else { oy - 1 }, 'T');
        }
        Lost::Glad | Lost::Hugging(_) => {
            let (y0, _) = hand(true);
            im.line(ox + 2, mid - 1, ox - 2, y0, 'T');
            im.line(ox + bw - 3, mid - 1, ox + bw + 1, y0, 'T');
        }
        Lost::Running => {
            let swing = if (tick / 3) % 2 == 0 { 2 } else { -2 };
            im.line(ox + 2, mid, ox - 1, mid + swing, 'T');
            im.line(ox + bw - 3, mid, ox + bw, mid - swing, 'T');
        }
        Lost::Looking | Lost::Home => {}
    }
    let (cx, cy) = (ox + bw + 1, mid);
    match culture {
        Culture::Capfolk => {}
        Culture::Glimmerkin => {
            im.line(cx, cy - 1, cx, cy + 2, 'G');
            im.rect(cx - 1, cy + 2, 3, 3, 'z');
            im.put(cx, cy + 3, '3');
        }
        Culture::Emberfolk => {
            im.line(cx, cy - 4, cx, cy + 3, 'P');
            im.rect(cx - 2, cy - 6, 5, 3, 'G');
            im.put(cx - 2, cy - 6, 'J');
        }
        Culture::Lampwick => {
            im.line(ox - 1, oy - 2, ox - 1, oy + bh - 1, 'B');
            im.line(ox - 1, oy - 2, ox + 2, oy - 4, 'B');
            im.rect(ox + 2, oy - 3, 3, 3, if bedraggled { 'g' } else { '5' });
        }
        Culture::Roostfolk => {
            im.put(ox + 4, oy, 'z');
            im.put(ox + 7, oy, if bedraggled { 'K' } else { 'z' });
        }
        Culture::Rootkin => {
            im.ellipse(cx as f32, (cy + 2) as f32, 2.0, 1.5, 'G');
            im.put(cx - 1, cy + 1, 'J');
        }
        Culture::Chimefolk => {}
        Culture::Saltkin => {
            im.line(ox - 2, oy + bh - 2, ox + bw / 2, oy + 1, 'P');
            im.line(ox - 4, oy + bh - 1, ox, oy + bh - 3, 'J');
        }
    }
    im
}

fn lost_colours(culture: Culture) -> &'static [(char, char)] {
    match culture {
        Culture::Capfolk => &[('A', 'l')],
        Culture::Glimmerkin => &[('6', 'z'), ('U', 'u')],
        Culture::Emberfolk => &[('I', 'p')],
        Culture::Lampwick => &[('Q', 'S'), ('u', 'x')],
        Culture::Roostfolk => &[('R', 'o')],
        Culture::Rootkin => &[('A', 'Y'), ('M', 'y'), ('B', 'r')],
        Culture::Chimefolk => &[('S', 'q'), ('j', 'Q')],
        Culture::Saltkin => &[('J', 'G')],
    }
}

fn child_home(k: usize) -> (f32, f32) {
    [(7.6, 10.8), (11.2, 10.8), (27.0, 10.9), (6.8, 16.9), (29.0, 16.8)][k % 5]
}

fn accessory(cv: &mut Img, culture: Culture, who: usize, (x, y): (i32, i32), hop: i32, left: bool, tick: u32) {
    let side = if left { -7 } else { 6 };
    match who {
        0 => {
            cv.line(x + side, y - hop, x + side, y - 18 - hop, 'B');
            cv.put(x + side, y - 19 - hop, 'O');
            if culture == Culture::Capfolk {
                cv.rect(x - 2, y - 7 - hop, 4, 3, 'H');
            }
        }
        WISHKEEPER => {
            let bob = ((tick as f32 * 0.1).sin() * 1.5) as i32;
            let (ox, oy) = (x + side, y - 14 - hop + bob);
            cv.put(ox, oy, 'w');
            cv.put(ox - 1, oy, '3');
            cv.put(ox + 1, oy, '3');
            cv.put(ox, oy - 1, '3');
            cv.put(ox, oy + 1, '3');
        }
        _ => {}
    }
}

pub(super) fn bubble(cv: &mut Img, x: i32, y: i32, tick: u32) {
    cv.rect(x - 7, y - 4, 15, 8, 'W');
    cv.frame(x - 8, y - 5, 17, 10, 'h');
    cv.put(x - 1, y + 5, 'W');
    cv.put(x, y + 6, 'W');
    for k in 0..3 {
        if k < 2 || (tick / 8) % 2 == 0 {
            cv.put(x - 4 + k * 4, y, 'K');
        }
    }
}

pub(super) fn portrait(culture: Culture, who: usize) -> Img {
    let im = figure(culture, who, false, false);
    let k = 3;
    let mut out = Img::new(im.w * k, im.h * k);
    for y in 0..out.h {
        for x in 0..out.w {
            if let Some(c) = im.get(x / k, y / k) {
                out.set(x, y, c);
            }
        }
    }
    out
}
