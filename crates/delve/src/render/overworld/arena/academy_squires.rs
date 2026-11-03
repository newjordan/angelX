//! Small painted companions and the yard's three independently raised works.
use super::*;
use crate::render::overworld::ink::text_width;
use crate::rules::squire_academy::{self as sq, Kind};

const NINJA: [&str; 12] = ["...xxxx....", "..xSSSSx...", "..xKOKKx...", "...xSSx....", "..h7777h...", ".h77hh77h..", "..777777.h.", "...hhhh....", "...7..7....", "...7..7....", "..xx..xx...", "..xx..xx..."];
const HEALER: [&str; 12] = ["...WWWW....", "..WW77WW...", "..WOKOOW...", "...OOO.....", "..W777W....", ".WW777WW.W.", "..W777W..W.", "..WWWWW..W.", "..WWWWW..W.", "...W.W...W.", "..xx.xx....", "..xx.xx...."];
const RANGER: [&str; 12] = ["...CCCC....", "..CCCCC....", "..COOOC..b.", "...OOO....b", "..C777C...b", ".CC777CC..b", "..C777C..b.", "...bbb.....", "...C.C.....", "...C.C.....", "..xx.xx....", "..xx.xx...."];
const SHIELD: [&str; 12] = ["...iiii....", "..iKKKKi...", "..iOKOOi...", "...iiii....", "..i777i.iii", ".ii777iiii7", "..i777iii77", "...bbb..i77", "...i.i..ii7", "...i.i...ii", "..xx.xx....", "..xx.xx...."];
const ALCHEMIST: [&str; 12] = ["...vvvv....", "..vKKKKv...", "..vOKOOv...", "...OOO.....", "..v777v....", ".vv777vv.v.", "..v777v.vVv", "...bbb...v.", "...v.v.....", "...v.v.....", "..xx.xx....", "..xx.xx...."];

pub(super) fn sprite(kind: Kind, owner: u32) -> Img {
    Img::from_rows(match kind { Kind::Ninja => &NINJA, Kind::Healer => &HEALER, Kind::Ranger => &RANGER, Kind::ShieldBearer => &SHIELD, Kind::Alchemist => &ALCHEMIST })
        .recolor(&[('7', knights::seat_cloth(owner))])
}
pub(super) fn delve(cv: &mut Img, run: &Run) {
    if run.at_home_now() { return; }
    for (&owner, c) in &run.squire_academy.active {
        if run.players.get(&owner).is_none_or(|h| h.absent) { continue; }
        hireling::ally(cv, run, &c.body);
        let (x, y) = at(c.body.x, c.body.y);
        cv.text(x - 2, y + 2, &c.level.to_string(), knights::seat_mark(owner));
        if c.reviving.is_some() { cv.text(x - 3, y - 20, "+", 'W'); }
        if c.body.hp > 0 && c.body.hp < c.body.max_hp {
            cv.line(x - 6, y - 15, x + 5, y - 15, 'x');
            cv.line(x - 6, y - 15, x - 6 + (11 * c.body.hp / c.body.max_hp.max(1)) as i32, y - 15, knights::seat_mark(owner));
        }
        if c.special > 7 * crate::rules::HZ && c.body.kind == crate::rules::hireling::AllyKind::SquireNinja {
            cv.ellipse(x as f32, y as f32 - 4., 9., 4., 'S');
        }
    }
    for (&enemy, &(owner, _)) in &run.squire_academy.marks {
        if let Some(e) = run.enemies.iter().find(|e| e.id == enemy && e.hp > 0) {
            let (x, y) = at(e.x, e.y); cv.text(x - 3, y - 22, "+", knights::seat_mark(owner));
        }
    }
}
fn near(h: &crate::rules::Hero, p: (f32, f32)) -> bool { (h.x - p.0 * 2.).hypot(h.y - p.1 * 2.) < 0.9 }
fn plate(cv: &mut Img, p: (f32, f32), lit: bool) {
    let (x, y) = at(p.0 * 2., p.1 * 2.);
    super::super::chrome::floor_plate(cv, (x - 8, y - 3, 16, 6), lit, true);
}
pub(super) fn yard(cv: &mut Img, run: &Run) {
    let a = &run.home.squire_academy;
    for (i, p) in sq::BUILD_AT.into_iter().enumerate() {
        let level = [a.board, a.hall, a.yard][i];
        let (x, y) = at(p.0 * 2., p.1 * 2.);
        cv.rect(x - 19, y - 20, 38, 16, 'b');
        cv.rect(x - 16, y - 18, 32, 12, 'P');
        cv.line(x - 21, y - 21, x + 21, y - 21, 'O');
        let label = format!("{} L{level}", ["BOARD", "HALL", "YARD"][i]);
        cv.text(x - text_width(&label) / 2, y - 16, &label, 'T');
        plate(cv, p, run.players.values().any(|h| h.hp > 0 && near(h, p)));
    }
    for (i, p) in sq::RECRUIT_AT.into_iter().enumerate() {
        let (x, y) = at(p.0 * 2., p.1 * 2.);
        stand(cv, &sprite(Kind::ALL[i], 1), x, y - 4);
        plate(cv, p, run.players.values().any(|h| h.hp > 0 && near(h, p)));
    }
    for (i, p) in sq::MANAGE_AT.into_iter().enumerate() {
        let (x, y) = at(p.0 * 2., p.1 * 2.);
        let label = ["SELECT", "TRAIN", "DISMISS"][i];
        cv.text(x - text_width(label) / 2, y - 9, label, 'T');
        plate(cv, p, run.players.values().any(|h| h.hp > 0 && near(h, p)));
    }
}
pub(super) fn boards(cv: &mut Img, run: &Run, focus: Option<u32>) {
    let Some((id, h)) = super::shoppers(run, focus).find(|(_, h)| h.hp > 0 && (
        sq::RECRUIT_AT.iter().chain(sq::MANAGE_AT.iter()).chain(sq::BUILD_AT.iter()).any(|&p| near(h, p)))) else { return; };
    let a = &run.home.squire_academy;
    let key = crate::rules::talents::knight_key(h);
    let roster = a.rosters.get(&key);
    let selected = roster.and_then(|r| r.members.iter().find(|s| Some(&s.recruit.id) == r.selected.as_ref()));
    let mut lines = vec![("SQUIRES - ONE ACTIVE PER KNIGHT".into(), 'T')];
    if let Some(i) = sq::RECRUIT_AT.iter().position(|&p| near(h, p)) {
        let r = &a.recruits()[i]; let stats = r.kind.stats(1, a.yard);
        lines.push((format!("{} - {} - {}G", r.name, r.kind.name(), r.price), 'W'));
        lines.push((format!("HP {}  POWER {}  RANGE {}", stats.hp, stats.damage, stats.reach), 'h'));
        lines.push((format!("ACTS EVERY {}S", stats.every as f32 / crate::rules::HZ as f32), 'h'));
        lines.push((r.kind.abilities(1).to_uppercase(), 'h'));
        lines.push((format!("L3: {}", r.kind.abilities(3).to_uppercase()), 'h'));
        lines.push((format!("E HIRE | ROSTER {}/{}", roster.map_or(0, |r| r.members.len()), a.cap()), 'O'));
        lines.push((format!("NEW NAMES IN {} DELVES", sq::RECRUIT_EVERY - a.departures % sq::RECRUIT_EVERY), 'h'));
    } else if let Some(i) = sq::BUILD_AT.iter().position(|&p| near(h, p)) {
        let level = [a.board, a.hall, a.yard][i];
        lines.push((format!("{} LEVEL {level}/3", ["RECRUIT BOARD", "TRAINING HALL", "SQUIRE YARD"][i]), 'W'));
        lines.push((["+1 ROSTER SLOT, -10G HIRE / LEVEL", "+1 MAX SQUIRE LEVEL (UP TO 5)", "+10 HP TO ALL SQUIRES / LEVEL"][i].into(), 'h'));
        lines.push((if level == 3 { "FULLY UPGRADED".into() } else { format!("E UPGRADE: {} TREASURY GOLD", 250 * (u32::from(level) + 1)) }, 'O'));
        lines.push(("INDEPENDENT, SHARED REALM BUILDINGS".into(), 'h'));
    } else {
        lines.push(("E: CYCLE / TRAIN / DISMISS AT PLATE".into(), 'O'));
        lines.push((format!("PER-KNIGHT ROSTER {}/{}", roster.map_or(0, |r| r.members.len()), a.cap()), 'h'));
        if let Some(r) = roster {
            for s in &r.members {
                lines.push((format!("{} {} {} L{}", if Some(&s.recruit.id) == r.selected.as_ref() { ">" } else { " " }, s.recruit.name, s.recruit.kind.name(), s.level), 'W'));
            }
        }
        if let Some(s) = selected {
            let stats = s.recruit.kind.stats(s.level, a.yard);
            lines.push((format!("HP {} POWER {} RANGE {}", stats.hp, stats.damage, stats.reach), 'h'));
            lines.push((s.recruit.kind.abilities(s.level).to_uppercase(), 'h'));
            lines.push((format!("DELVES {} / NEED {} | CAP L{}", s.delves, s.level, a.level_cap()), 'h'));
            lines.push((if s.training > 0 { format!("TRAINING {}S (TOWN ONLY)", s.training.div_ceil(crate::rules::HZ)) } else { format!("TRAIN: {}G / {}S TOWN", 80 * u32::from(s.level), sq::TRAIN_SECONDS * u32::from(s.level)) }, 'h'));
            lines.push(("TRAINING SQUIRES STAY HOME".into(), 'h'));
        } else { lines.push(("NONE SELECTED - E SELECT CYCLES".into(), 'h')); }
    }
    lines.push((format!("SHARED TREASURY: {}G", run.treasury.get(crate::realm::Spoil::Gold)), 'O'));
    if let Some(text) = run.squire_academy.notices.get(&id) {
        for chunk in text.as_bytes().chunks(47) { lines.push((String::from_utf8_lossy(chunk).to_uppercase(), 'T')); }
    }
    let board = home::board(&lines);
    cv.stamp(&board, (cv.w - board.w) / 2, 2);
}
