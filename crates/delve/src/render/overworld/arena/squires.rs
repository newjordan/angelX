//! A knight's small company: five hand-pixelled silhouettes, cloth in the
//! owner's seat colour, and the Training Yard's black-paper muster rolls.
//! Geometry and transactions belong to `rules::squires`; these are only ink.

use super::super::ink::{Img, text_width};
use super::{TILE, at, knights, stand};
use crate::realm::Spoil;
use crate::rules::home::BUY_HOLD;
use crate::rules::squires::{self, Action, Kind, Squire, Yard};
use crate::rules::talents::knight_key;
use crate::rules::{RoomKind, Run};

// The same red/shadow/gold cloth keys as the knights. Skin, tools, armour
// and the healer's linen deliberately keep their own inks when recoloured.
const NINJA: [&str; 18] = [
    ".....KKKKK........",
    "....KgjjjgK.......",
    "....KjgggjK.......",
    "....KOKOOKK.......",
    "....KgggggK.......",
    ".....KjjjK........",
    "...88777788.......",
    "..K87757778K......",
    "..KjK777KjgK..i...",
    ".KjgK878KgjK.iHi..",
    ".KOKKjjjKKOK..i...",
    "..K.KbObK.K...B...",
    "....KgggK.........",
    "....KjKjK.........",
    "....KjKjK.........",
    "....Kg.Kg.........",
    "...Kjj.Kjj........",
    "...KKK.KKK........",
];
const HEALER: [&str; 19] = [
    ".....GiiiG........",
    "....GiHHHiG.......",
    "....iH777Hi.......",
    "....iHH7HHi.......",
    "....iOKOKOi.......",
    ".....OOOOO........",
    "....GiiiiiG...T...",
    "...GiH777HiG..B...",
    "..GiHiH7HiHiG.B...",
    "..OiHiHHHiHiOBB...",
    "..O.GH7H7HG...B...",
    "....GbObbPG...B...",
    "....GiHiPTG...B...",
    "....GiHiPPG...B...",
    "....GiHiHiG...B...",
    "....GiiiiiG...B...",
    ".....jJ.jJ....B...",
    "....nnn.nnn...b...",
    "....KKK.KKK.......",
];
const RANGER: [&str; 19] = [
    ".........T........",
    ".....FEEET........",
    "....FEllEF........",
    "...FElOOlEF.......",
    "....EOKOKOE.......",
    ".....OOOOO...I....",
    "..i..F777F..I.I...",
    "..i.FE757EF.I..I..",
    "..BFE7777EEF...I..",
    "..BFEbObbEOIIIIIIi",
    "..BFEl77lEF.I..I..",
    "....FEllEF..I..I..",
    "....FEEEEF...I.I..",
    ".....BPPB.....I...",
    ".....BPBB.........",
    ".....BB.BB........",
    ".....IB.BI........",
    "....nnn.nnn.......",
    "....KKK.KKK.......",
];
const SHIELD: [&str; 19] = [
    ".....GJJG.........",
    "....GJhhJG........",
    "....JhiiJg........",
    "....JKKKKg........",
    "....GhiiJG........",
    ".....GGGG.........",
    "...GJh77hJG.......",
    "..GJh7757hJGGGGG..",
    "..GJhh77hGGiHiiG..",
    "..OJhh77hGi7777iG.",
    "..O.GbbbGGi7757iG.",
    "....GJOJGGi7777iG.",
    "....GJhhGGi8778iG.",
    "....GJGGJGGi88iG..",
    "....GJ.GJG.GiiG...",
    "....GJ.GJG..GG....",
    "....gj.gjg........",
    "...KJJ.KJJ........",
    "...KKK.KKK........",
];
const ALCHEMIST: [&str; 19] = [
    ".....bPPP.........",
    "....bPrrPb........",
    "....Piiiib........",
    "....iKiKiP........",
    "....OOOOOP........",
    ".....OPPO.........",
    "....877778........",
    "...87757778.......",
    "..O8GiHiG78O..h...",
    "..O.GiHiG8.O.GiG..",
    "....GbObG...GmYmG.",
    "....GiHiG...GmYmG.",
    "...PPiHiGP...GGG..",
    "...PTiHiGP........",
    "...PPGGGGP........",
    ".....BB.BB........",
    ".....IB.BI........",
    "....nnn.nnn.......",
    "....KKK.KKK.......",
];

// Low, hand-drawn downed silhouettes: gear beside its owner, never a
// standing sprite merely rotated into a still-tall box.
fn fallen(kind: Kind) -> Img {
    let rows: &[&str] = match kind {
        Kind::Ninja => &[
            ".......................",
            ".......................",
            "...............KKKKK...",
            "...KKKK8877778KjggggK..",
            "..Kjjjg7775777KgOKOKK..",
            "..KKKKK88877888KggggK..",
            ".......KKKKKKKKKKKKK...",
            "..i....................",
            ".iHi...................",
        ],
        Kind::Healer => &[
            ".......................",
            ".......................",
            "...............GiiiG...",
            "....GGiiiiiiiiGiHHHiG..",
            "..njGiH77777HHiOOKOOi..",
            "..nnGiiH7H7HHHiOOOOi...",
            ".....GGiiiiiiiGGGGG....",
            "..TBBBBBBBBBBBBBBBBB...",
            ".......................",
        ],
        Kind::Ranger => &[
            "...................T...",
            "...............FEEET...",
            "..............FElEEEF..",
            "..nnBBBB77777FOOKOOF...",
            "..nnBBBP77577FOOOOOF...",
            ".....PPFEEEEEFFEEEF....",
            ".....FFEEEEEEFF........",
            "...IBBBBBBBBBBBI.......",
            "....IIIIIIIIIII........",
        ],
        Kind::Shield => &[
            "....GGGG...............",
            "...GiHiiG.......GJJG...",
            "..Gi7777iG.....GJhhJG..",
            "..Gi7757iGJJ777JhKKKhG..",
            "..Gi8778iGJJ777GJhiiJG..",
            "...Gi88iGGggggGGGGGG...",
            "....GiiGKKKKKKKKKKKK...",
            ".....GG................",
            ".......................",
        ],
        Kind::Alchemist => &[
            ".......................",
            "...............bPPPP...",
            "...............PrrrPb..",
            "..nnBBBB777778OiKiKiP..",
            "..nnBPTP775778OOOOOP...",
            ".....PPP888888PPPPP....",
            "...h..bPTPbbb..........",
            "..GmG..PPPP..mY........",
            "...G........mYmm.......",
        ],
    };
    Img::from_rows(rows)
}

/// Authored at a little under a knight's height, not scaled-down knights.
/// The two-pixel boot swing changes only the legs, never the face or tools.
pub(super) fn sprite(kind: Kind, owner: u32, tick: u64, walking: bool, down: bool) -> Img {
    if down {
        return fallen(kind).recolor(knights::seat_colours(owner));
    }
    let rows: &[&str] = match kind {
        Kind::Ninja => &NINJA,
        Kind::Healer => &HEALER,
        Kind::Ranger => &RANGER,
        Kind::Shield => &SHIELD,
        Kind::Alchemist => &ALCHEMIST,
    };
    let mut im = Img::from_rows(rows).recolor(knights::seat_colours(owner));
    if walking && (tick / 7) % 2 == 1 {
        let source = im.clone();
        for y in im.h - 3..im.h {
            for x in 3..11 {
                im.clear(x, y);
                if let Some(c) = source.get(x, y - i32::from(x < 7)) {
                    im.set(x, y, c);
                }
            }
        }
    }
    im
}

fn short_kind(kind: Kind) -> &'static str {
    match kind {
        Kind::Shield => "SHIELD",
        _ => kind.name(),
    }
}

/// Neutral, bevelled floor slates. A warm underline means occupied, not a
/// pool of light. Plate positions come from the rules, including empty slots.
fn slate(cv: &mut Img, (c, r, w, h): (i32, i32, i32, i32), label: &str, occupied: bool) {
    let (x, y, w, h) = (c * TILE + 1, r * TILE + 1, w * TILE - 2, h * TILE - 2);
    cv.rect(x, y, w, h, 'K');
    cv.frame(x, y, w, h, 'g');
    cv.line(x + 1, y + 1, x + w - 2, y + 1, 'G');
    cv.text(
        x + (w - text_width(label)) / 2,
        y + 3,
        label,
        if occupied { 'T' } else { 'h' },
    );
    cv.line(
        x + 3,
        y + h - 2,
        x + w - 4,
        y + h - 2,
        if occupied { 'O' } else { 'X' },
    );
}

fn plate_label(action: Action) -> String {
    match action {
        Action::Hire(i) => format!("{}", i + 1),
        Action::Select(i) => format!("{}", i + 1),
        Action::Train(i) => format!("T{}", i + 1),
        Action::Rest => "REST".into(),
        Action::Upgrade(i) => ["BOARD", "HALL", "YARD"].get(i).unwrap_or(&"?").to_string(),
    }
}

/// Added after the yard's dusk: grey stone, black paper and dull brass,
/// without relighting the yard or changing its existing fixtures.
pub(super) fn scenery(cv: &mut Img, _run: &Run) {
    for (action, rect) in squires::plates() {
        slate(cv, rect, &plate_label(action), false);
    }
    cv.text(22, 75, "RECRUITS / HOLD E", 'h');
    cv.text(18, 126, "COMPANY / CHOOSE", 'h');
    // A small rolled muster notice, below the targets on the north wall.
    cv.line(22, 72, 347, 72, 'g');
    for x in [22, 347] {
        cv.rect(x - 1, 70, 3, 5, 'G');
        cv.put(x, 71, 'O');
    }
}

/// Both the waiting recruits and deployed companions use the exact same
/// sprite. No Hireling species stand in for any of the five squire types.
pub(super) fn figures(cv: &mut Img, run: &Run) {
    if run.at_home_now() {
        if run.room().kind != RoomKind::Yard {
            return;
        }
        let yard = &run.home.squires;
        let offers = yard.offers();
        for (action, (c, r, w, _)) in squires::plates() {
            if let Action::Hire(i) = action {
                if let Some(s) = offers.get(i) {
                    let x = c * TILE + w * TILE / 2;
                    stand(
                        cv,
                        &sprite(s.kind, 0, run.tick, false, false),
                        x,
                        r * TILE - 2,
                    );
                    let name = short_kind(s.kind).to_uppercase();
                    cv.text(x - text_width(&name) / 2, r * TILE + 17, &name, 'i');
                } else {
                    let x = c * TILE + w * TILE / 2;
                    cv.text(x - text_width("UPGRADE") / 2, r * TILE + 17, "UPGRADE", 'G');
                }
            }
        }
        // Each seat's own company is read on its board, while occupied slates
        // and their hold affordance remain visible to everybody in the yard.
        for hero in run.players.values().filter(|h| h.hp > 0) {
            let Some(action) = squires::action_at(hero.x, hero.y) else {
                continue;
            };
            if let Some((_, rect)) = squires::plates().into_iter().find(|(a, _)| *a == action) {
                slate(cv, rect, &plate_label(action), true);
            }
        }
        return;
    }
    for companion in &run.squires {
        let body = &companion.body;
        let (x, y) = at(body.x, body.y);
        let moving = body.cooldown > 0;
        let mut im = sprite(
            companion.squire.kind,
            companion.owner,
            run.tick,
            moving,
            body.down(),
        );
        if body.facing < 0.0 {
            im = im.flip_h();
        }
        // A downed companion keeps its cloth and recognisable equipment;
        // unlike a knight it isn't rendered as a skull awaiting resurrection.
        if body.down() {
            stand(cv, &im, x, y + 3);
            continue;
        }
        if body.invulnerable == 0 || (run.tick / 2) % 2 == 1 {
            stand(cv, &im, x, y);
        }
        cv.rect(x - 2, y + 2, 4, 1, knights::seat_cloth(companion.owner));
        if body.hp < body.max_hp {
            cv.rect(x - 7, y - im.h - 4, 14, 2, 'K');
            let health = (u64::from(body.hp) * 12 / u64::from(body.max_hp.max(1))).min(12) as i32;
            cv.rect(x - 6, y - im.h - 4, health, 1, 'h');
        }
        if companion.smoke > 0 {
            // Dry, squared-off smoke. Neither a glow nor an opaque blanket
            // over the ninja: a few charcoal flecks at the silhouette's edge.
            for n in 0..5 {
                let phase = ((run.tick / 3 + n as u64 * 3) % 9) as i32;
                cv.rect(
                    x - 10 + n * 5,
                    y - 3 - phase,
                    2,
                    2,
                    if n % 2 == 0 { 'g' } else { 'X' },
                );
            }
        }
    }
}

fn bounded(text: &str, count: usize) -> String {
    text.chars().take(count).collect::<String>().to_uppercase()
}

fn line(lines: &mut Vec<(String, char)>, text: impl Into<String>, ink: char) {
    // Keep unusual player/save names from pushing the scroll off screen.
    let text = text.into();
    let mut row = String::new();
    for word in text.split_whitespace() {
        if !row.is_empty() && row.chars().count() + word.chars().count() + 1 > 48 {
            lines.push((std::mem::take(&mut row), ink));
        }
        if !row.is_empty() {
            row.push(' ');
        }
        row.extend(word.chars().take(48));
    }
    lines.push((row, ink));
}

fn stats(s: &Squire) -> String {
    format!(
        "LEVEL {} / {} HP / {} POWER",
        s.level,
        s.health(),
        s.power()
    )
}

fn upgrade_name(step: usize) -> &'static str {
    ["RECRUIT BOARD", "COMPANY HALL", "TRAINING YARD"]
        .get(step)
        .copied()
        .unwrap_or("BUILDING")
}

fn upgrade_effect(yard: &Yard, step: usize) -> String {
    // Query the rules rather than maintaining a second economy in the UI.
    let sample = Squire {
        id: 0,
        kind: Kind::Ninja,
        name: String::new(),
        level: 1,
    };
    match step {
        0 => {
            let offers = yard.offers();
            format!(
                "{} OFFERS / RECRUIT LEVEL {}",
                offers.len(),
                offers.first().map_or(1, |s| s.level)
            )
        }
        1 => format!(
            "{} PLACES / WAGE {} GOLD AT L1",
            yard.capacity(),
            yard.wage(&sample)
        ),
        _ => format!(
            "LEVEL CAP {} / LESSON {} GOLD AT L1",
            yard.ceiling(),
            yard.lesson(&sample)
        ),
    }
}

/// Square-cut scroll rolls, a double grey edge, black paper, dull warm
/// rulings. Deliberately not `chrome::panel`: no blue fill, gradient or glow.
fn scroll(lines: &[(String, char)], control: &str, buying: u32, enabled: bool) -> Img {
    let (w, h) = (348, 23 + lines.len() as i32 * 9 + 25);
    let mut im = Img::black(w, h);
    im.frame(4, 3, w - 8, h - 6, 'g');
    im.frame(6, 5, w - 12, h - 10, 'X');
    for y in [0, h - 7] {
        im.rect(2, y + 1, w - 4, 5, 'g');
        im.line(5, y + 1, w - 6, y + 1, 'h');
        im.line(5, y + 3, w - 6, y + 3, 'G');
        for x in [0, w - 5] {
            im.rect(x, y + 2, 5, 3, 'G');
            im.put(x + 2, y + 2, 'O');
        }
    }
    im.text(14, 10, "THE SQUIRE COMPANY", 'T');
    im.line(14, 20, w - 15, 20, 'g');
    for (i, (text, ink)) in lines.iter().enumerate() {
        im.text(14, 25 + i as i32 * 9, text, *ink);
    }
    let y = h - 17;
    im.text(14, y, control, if enabled { 'T' } else { 'G' });
    im.frame(14 + text_width(control) + 10, y + 1, 64, 5, 'G');
    if enabled {
        let fill = (u64::from(buying.min(BUY_HOLD)) * 60 / u64::from(BUY_HOLD.max(1))) as i32;
        im.rect(16 + text_width(control) + 10, y + 2, fill, 3, 'O');
    }
    im
}

/// Seat-local, hold-E transactions. Reading never mutates the roster, the
/// purse, the active companion, or another seat's choice.
pub(super) fn boards(cv: &mut Img, run: &Run, focus: Option<u32>) {
    if !run.at_home_now() || run.room().kind != RoomKind::Yard {
        return;
    }
    let Some((_, hero, action)) = super::shoppers(run, focus)
        .filter(|(_, h)| h.hp > 0)
        .find_map(|(id, h)| squires::action_at(h.x, h.y).map(|a| (id, h, a)))
    else {
        return;
    };
    let yard = &run.home.squires;
    let key = knight_key(hero);
    let company = yard.companies.get(&key);
    let roster = company.map_or(&[][..], |c| c.roster.as_slice());
    let active = company.and_then(|c| c.active);
    let gold = run.treasury.get(Spoil::Gold);
    let mut lines = Vec::new();
    line(
        &mut lines,
        format!(
            "{} / {}/{} SQUIRES / {gold} GOLD",
            bounded(&hero.name, 16),
            roster.len(),
            yard.capacity()
        ),
        'h',
    );
    let mut enabled = false;
    let mut control = "HOLD E";
    match action {
        Action::Hire(i) => {
            if let Some(s) = yard.offers().get(i) {
                line(
                    &mut lines,
                    format!(
                        "{} / {}",
                        bounded(&s.name, 20),
                        s.kind.name().to_uppercase()
                    ),
                    'H',
                );
                line(&mut lines, stats(s), 'i');
                line(&mut lines, s.kind.ability(s.level).to_uppercase(), 'h');
                let wage = yard.wage(s);
                line(&mut lines, format!("WAGE {wage} GOLD / REALM PURSE"), 'T');
                let reason = if roster.iter().any(|r| r.kind == s.kind) {
                    Some("TYPE ALREADY HIRED")
                } else if roster.len() >= yard.capacity() {
                    Some("ROSTER FULL / BUILD HALL")
                } else if gold < wage {
                    Some("NOT ENOUGH GOLD")
                } else {
                    None
                };
                enabled = reason.is_none();
                line(
                    &mut lines,
                    reason.unwrap_or("ONE SQUIRE PER KNIGHT IN THE DUNGEON"),
                    if enabled { 'h' } else { 'O' },
                );
                control = "HOLD E: HIRE";
            } else {
                line(&mut lines, "NO RECRUIT / UPGRADE THE BOARD", 'h');
            }
        }
        Action::Select(i) | Action::Train(i) => {
            if let Some(s) = roster.get(i) {
                let selected = active == Some(s.id);
                line(
                    &mut lines,
                    format!(
                        "{} / {}{}",
                        bounded(&s.name, 18),
                        s.kind.name().to_uppercase(),
                        if selected { " / ACTIVE" } else { "" }
                    ),
                    'H',
                );
                line(&mut lines, stats(s), 'i');
                line(&mut lines, s.kind.ability(s.level).to_uppercase(), 'T');
                if matches!(action, Action::Train(_)) {
                    if s.level < yard.ceiling() {
                        let mut next = s.clone();
                        next.level += 1;
                        line(&mut lines, format!("NEXT: {}", stats(&next)), 'h');
                        line(
                            &mut lines,
                            format!("NEXT: {}", next.kind.ability(next.level).to_uppercase()),
                            'h',
                        );
                        let cost = yard.lesson(s);
                        line(
                            &mut lines,
                            format!("LESSON {cost} GOLD / CAP {}", yard.ceiling()),
                            'T',
                        );
                        enabled = gold >= cost;
                        if !enabled {
                            line(&mut lines, "NOT ENOUGH GOLD", 'O');
                        }
                    } else {
                        line(
                            &mut lines,
                            if s.level >= 5 {
                                "MASTER SQUIRE / LEVEL 5"
                            } else {
                                "AT LEVEL CAP / BUILD TRAINING YARD"
                            },
                            'O',
                        );
                    }
                    control = "HOLD E: TRAIN";
                } else {
                    enabled = true;
                    control = "HOLD E: CHOOSE";
                    line(&mut lines, "THIS SQUIRE FOLLOWS ITS KNIGHT", 'h');
                }
                line(&mut lines, "COMPANY ROLL / > ACTIVE", 'G');
                // Eight rule slots fit; long save names cannot widen a row.
                for member in roster.iter().take(8) {
                    line(
                        &mut lines,
                        format!(
                            "{} {} L{} / {} HP / {} POW",
                            if active == Some(member.id) { ">" } else { "-" },
                            bounded(&member.name, 12),
                            member.level,
                            member.health(),
                            member.power()
                        ),
                        if active == Some(member.id) { 'T' } else { 'h' },
                    );
                }
            } else {
                line(&mut lines, "EMPTY ROSTER SLOT", 'H');
                line(&mut lines, "HIRE A SQUIRE AT THE NORTH BOARD", 'h');
            }
        }
        Action::Rest => {
            line(&mut lines, "REST THE COMPANY", 'H');
            line(&mut lines, "NO SQUIRE WILL FOLLOW THIS KNIGHT", 'h');
            line(&mut lines, "ROSTER AND TRAINING ARE KEPT", 'h');
            if let Some(s) = roster.iter().find(|s| active == Some(s.id)) {
                line(
                    &mut lines,
                    format!("ACTIVE: {} / LEVEL {}", bounded(&s.name, 20), s.level),
                    'T',
                );
            } else {
                line(&mut lines, "ALL SQUIRES RESTING", 'G');
            }
            enabled = active.is_some();
            control = "HOLD E: REST";
        }
        Action::Upgrade(chosen) => {
            for step in 0..3 {
                let level = [yard.board, yard.hall, yard.training][step];
                let cost = yard.upgrade_cost(step);
                line(
                    &mut lines,
                    format!(
                        "{} {} {level}/3 / {}",
                        if chosen == step { ">" } else { "-" },
                        upgrade_name(step),
                        cost.map_or("COMPLETE".into(), |c| format!("{c} GOLD"))
                    ),
                    if chosen == step { 'T' } else { 'i' },
                );
                line(
                    &mut lines,
                    format!("NOW: {}", upgrade_effect(yard, step)),
                    'h',
                );
                if cost.is_some() {
                    let mut next = yard.clone();
                    match step {
                        0 => next.board += 1,
                        1 => next.hall += 1,
                        _ => next.training += 1,
                    }
                    line(
                        &mut lines,
                        format!("NEXT: {}", upgrade_effect(&next, step)),
                        'h',
                    );
                }
            }
            enabled = yard.upgrade_cost(chosen).is_some_and(|c| gold >= c);
            if !enabled {
                line(
                    &mut lines,
                    if yard.upgrade_cost(chosen).is_none() {
                        "BUILDING COMPLETE"
                    } else {
                        "NOT ENOUGH GOLD"
                    },
                    'O',
                );
            }
            control = "HOLD E: BUILD";
        }
    }
    // The hold stays visible even for imported, unusually large companies.
    let max_lines = ((cv.h - 50) / 9).max(1) as usize;
    if lines.len() > max_lines {
        lines.truncate(max_lines);
        lines[max_lines - 1] = ("MORE SQUIRES ON THEIR OWN SLATES".into(), 'G');
    }
    let board = scroll(&lines, control, hero.buying, enabled);
    let x = ((cv.w - board.w) / 2).max(0);
    // North-board readers can still see the recruit and the plate they use.
    let y = if matches!(action, Action::Hire(_)) {
        (cv.h - board.h - 4).max(4)
    } else {
        4
    };
    // Black margin keeps half-seen floor labels from reading like broken
    // letters at the scroll's edge. No tint or glow over the room.
    cv.rect(x - 6, y - 3, board.w + 12, board.h + 6, 'K');
    cv.stamp(&board, x, y);
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__squires__shots.rs"]
mod shots;

// The other company has its own rendering and yard notices. Its rules and
// companion state must remain distinct from the Training Yard company.
mod countryside_company {
// Little company figures and the yard's plain paper notices.
use super::super::super::{ink::Img, kit};
use super::super::{TILE, at, stand};
use crate::rules::{
    Run, home,
    countryside_squires::{Kind, PLATES, Squire, plate},
};

fn figure(kind: Kind, cloth: char) -> Img {
    let mut im = Img::new(17, 17);
    im.stamp_times(&kit::squire(cloth), 2, 0, 1);
    // A hood, a cross, a bow, a shield, or a bottle: readable at native scale.
    match kind {
        Kind::Ninja => {
            im.rect(3, 0, 7, 4, 'J');
            im.line(4, 3, 8, 3, 'H');
            im.line(12, 5, 12, 13, 'I');
        }
        Kind::Healer => {
            im.rect(5, 5, 5, 5, 'c');
            im.line(7, 6, 7, 9, 'J');
            im.line(6, 7, 8, 7, 'J');
        }
        Kind::Ranger => {
            im.line(12, 2, 14, 7, 'o');
            im.line(14, 7, 12, 12, 'o');
            im.line(12, 2, 12, 12, 'I');
        }
        Kind::Shieldbearer => {
            im.rect(10, 5, 6, 8, 'J');
            im.frame(10, 5, 6, 8, 'I');
            im.rect(12, 7, 2, 4, cloth);
        }
        Kind::Alchemist => {
            im.rect(11, 7, 5, 5, 'o');
            im.rect(12, 5, 3, 2, 'I');
            im.line(12, 9, 14, 9, 'O');
        }
    }
    im
}

pub(crate) fn delve(cv: &mut Img, run: &Run) {
    for c in &run.countryside_squires {
        let mut im = figure(c.recruit.kind, super::super::knights::seat_cloth(c.owner));
        // The native little figure is enlarged only once, to roughly half a knight.
        let mut large = Img::new(im.w * 2, im.h * 2);
        large.stamp_twice(&im, 0, 0);
        im = large;
        if c.body.down() {
            im = super::super::sprites::quarter_turn(&im);
        } else if c.body.facing < 0.0 {
            im = im.flip_h();
        }
        let (x, y) = at(c.body.x, c.body.y);
        if c.smoke > 0 {
            for n in 0..5 {
                let drift = (run.tick as i32 / 3 + n * 7) % 20;
                cv.rect(x - 12 + drift, y - 12 - n * 3, 4, 3, 'I');
            }
            if (run.tick / 4).is_multiple_of(2) {
                stand(cv, &im, x, y);
            }
        } else {
            stand(cv, &im, x, y);
        }
        if let Some(e) = c
            .marked
            .and_then(|id| run.enemies.iter().find(|e| e.id == id && e.hp > 0))
        {
            let (mx, my) = at(e.x, e.y);
            cv.line(mx - 4, my - 30, mx, my - 26, 'o');
            cv.line(mx, my - 26, mx + 4, my - 30, 'o');
        }
        cv.rect(x - 8, y + 3, 16, 2, 'J');
        cv.rect(
            x - 8,
            y + 3,
            (16 * c.body.hp / c.body.max_hp.max(1)) as i32,
            2,
            super::super::knights::seat_cloth(c.owner),
        );
        cv.text(
            x - 3,
            y - 31,
            &c.owner.to_string(),
            super::super::knights::seat_cloth(c.owner),
        );
    }
}

pub(crate) fn yard(cv: &mut Img, run: &Run) {
    let company = &run.home.countryside.squires;
    let offers = company.offers();
    for (i, &(x, y)) in PLATES.iter().enumerate() {
        if i < 6 && i >= offers.len() {
            continue;
        }
        let (px, py) = ((x * TILE as f32) as i32, (y * TILE as f32) as i32);
        let lit = run.players.values().any(|h| plate(h.x, h.y) == Some(i));
        super::super::super::chrome::floor_plate(cv, (px - 11, py - 4, 22, 8), lit, true);
        if i < 6 {
            let im = figure(
                offers[i].kind,
                if company.hired.contains(&i) { 'I' } else { 'o' },
            );
            stand(cv, &im, px, py - 7);
        } else {
            let level = match i {
                8 => Some(company.board),
                9 => Some(company.hall),
                10 => Some(company.yard),
                _ => None,
            };
            let label = match i {
                6 => "PICK",
                7 => "TRAIN",
                8 => "BOARD",
                9 => "HALL",
                _ => "YARD",
            };
            cv.rect(px - 17, py - 30, 35, 22, 'J');
            cv.frame(px - 17, py - 30, 35, 22, 'I');
            cv.text(px - 15, py - 27, label, 'o');
            if let Some(l) = level {
                cv.text(px - 3, py - 19, &l.to_string(), 'c');
            }
        }
    }
}

fn stats(s: &Squire) -> String {
    format!(
        "{} {}  L{}  HP{}  HIT{}",
        s.name,
        s.kind.name(),
        s.level,
        s.health(),
        s.power()
    )
}

pub(crate) fn board(cv: &mut Img, run: &Run, focus: Option<u32>) {
    let Some((id, hero, index)) =
        super::super::shoppers(run, focus).find_map(|(id, h)| Some((id, h, plate(h.x, h.y)?)))
    else {
        return;
    };
    let c = &run.home.countryside.squires;
    let active = c.active(&home::Home::knight_key(hero));
    let (title, line, detail) = match index {
        0..=5 => {
            let Some(s) = c.offers().get(index).cloned() else {
                return;
            };
            (
                "RECRUIT",
                stats(&s),
                if c.hired.contains(&index) {
                    "Hired. Return next delve.".into()
                } else {
                    format!(
                        "{} gold. Company {}/{}",
                        c.wage(&s),
                        c.roster.len(),
                        c.cap()
                    )
                },
            )
        }
        6 => (
            "COMPANY",
            active.map_or("No active squire".into(), stats),
            format!("{} hired. Cycle, then rest.", c.roster.len()),
        ),
        7 => (
            "TRAIN",
            active.map_or("Pick a squire first".into(), stats),
            active.map_or("".into(), |s| {
                format!(
                    "{} gold. Practice {}. Cap {}",
                    35 * u32::from(s.level),
                    s.drills,
                    2 + c.yard * 2
                )
            }),
        ),
        _ => {
            let l = [c.board, c.hall, c.yard][index - 8];
            let title = ["RECRUITING BOARD", "HIRING HALL", "TRAINING YARD"][index - 8];
            let line = [
                "More recruits, rarer types",
                "Larger roster, lower wages",
                "Faster drills, higher levels",
            ][index - 8];
            (
                title,
                line.into(),
                format!("Level {l}/3. {} gold", 120 * (u32::from(l) + 1)),
            )
        }
    };
    let w = 300.min(cv.w - 8);
    let x = (cv.w - w) / 2;
    let y = cv.h - 71;
    cv.rect(x, y, w, 67, 'J');
    cv.frame(x, y, w, 67, 'I');
    cv.text(x + 8, y + 6, title, 'o');
    cv.text(x + 8, y + 18, &line, 'c');
    cv.text(x + 8, y + 30, &detail, 'I');
    cv.text(x + 8, y + 43, "HOLD E", 'o');
    let ticks = run.countryside_squire_holds.get(&id).map_or(0, |(_, ticks)| *ticks);
    cv.text(x + 65, y + 43, &home::hold_bar(ticks, home::BUY_HOLD), 'I');
    cv.text(
        x + w - 100,
        y + 54,
        &format!("GOLD {}", run.treasury.get(crate::realm::Spoil::Gold)),
        'I',
    );
}

#[cfg(test)]
mod tests {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/render/squires__shots.rs"));
}

}

pub(super) use countryside_company::{board, delve, yard};
