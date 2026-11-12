//! Tiny companions use the same pixel sprites, shadows and stand pipeline
//! as hirelings, without borrowing their slots or Gravecaller equipment.
use super::super::ink::{Img, text_width};
use super::{TILE, at, sprites, stand};
use crate::rules::{
    RoomKind, Run, home,
    squires_lodge::{Kind, View},
    squires_town as yard,
};

pub(super) fn sprite(v: &View) -> Img {
    // Beaumains' small human silhouette, with kind-specific kit. Cloth is
    // the owner's six-seat ink; outline and skin stay legible in a crowd.
    let mut im = Img::from_rows(&super::hireling::BEAUMAINS);
    let cloth = super::super::ink::ink(v.owner_ink).unwrap();
    let old = super::super::ink::ink('i').unwrap();
    for y in 6..15 {
        for x in 0..im.w {
            if im.get(x, y) == Some(old) {
                im.set(x, y, cloth);
            }
        }
    }
    match v.kind {
        Kind::Ninja => {
            im.rect(3, 0, 7, 3, 'g');
            im.rect(3, 3, 7, 2, 'k');
            im.rect(4, 3, 5, 1, 'H');
            im.line(11, 8, 13, 3, 'H');
        }
        Kind::Healer => {
            im.rect(3, 0, 7, 2, 'H');
            im.line(12, 5, 12, 14, 'P');
            im.line(10, 6, 14, 6, 'H');
            im.line(12, 4, 12, 8, 'H');
        }
        Kind::Ranger => {
            im.rect(3, 0, 7, 2, 'E');
            im.line(11, 6, 13, 9, 'P');
            im.line(13, 9, 11, 13, 'P');
            im.line(11, 6, 11, 13, 'i');
        }
        Kind::Shieldbearer => {
            im.rect(3, 0, 7, 3, 'J');
            im.rect(3, 3, 7, 1, 'k');
            im.rect(10, 7, 5, 7, 'g');
            im.rect(11, 8, 3, 5, v.owner_ink);
            im.line(12, 8, 12, 12, 'H');
        }
        Kind::Alchemist => {
            im.rect(3, 0, 7, 2, 'P');
            im.rect(4, 3, 5, 1, 'H');
            im.rect(10, 8, 4, 5, 'J');
            im.rect(11, 7, 2, 1, 'o');
            im.rect(11, 10, 2, 2, 'o');
        }
    }
    if v.downed {
        return sprites::quarter_turn(&im);
    }
    if v.facing < 0.0 {
        im = im.flip_h();
    }
    im
}
#[cfg(test)]
pub(super) fn delve(cv: &mut Img, run: &Run) {
    if run.at_home_now() {
        return;
    }
    for v in run.squire_views() {
        figure(cv, &v);
    }
}

pub(super) fn figure(cv: &mut Img, v: &View) {
    let (x, y) = at(v.x, v.y);
    let mut im = sprite(v);
    if v.vanished {
        // Not invisible: a dim masked outline and owner pip remain.
        for yy in 0..im.h {
            for xx in 0..im.w {
                if im.get(xx, yy).is_some() {
                    im.put(xx, yy, 'g');
                }
            }
        }
    }
    super::knights::shadow(cv, x, y);
    stand(cv, &im, x, y);
    let width = (v.hp * 10 / v.max_hp.max(1)) as i32;
    cv.rect(x - 5, y - im.h - 4, 10, 2, 'k');
    cv.rect(x - 5, y - im.h - 4, width, 1, v.owner_ink);
    cv.text(x - 2, y + 1, &v.owner.to_string(), v.owner_ink);
    if v.downed {
        cv.text(x - 2, y - im.h - 8, "+", 'h');
    }
}

/// Static silhouettes and engraved controls, rendered on the real map.
pub(super) fn town(cv: &mut Img, run: &Run, origin: (i32, i32)) {
    if !yard::on_map() {
        return;
    }
    let (ox, oy) = origin;
    let (yx, yy, yw, yh) = yard::YARD;
    if (yx + yw) * TILE < ox
        || (yx - 5) * TILE > ox + cv.w
        || (yy + yh) * TILE < oy
        || yy * TILE > oy + cv.h
    {
        return;
    }
    let (x, y) = (yx * TILE - ox, yy * TILE - oy);
    // Quiet cobbled apron, warm guide line back to the high street.
    cv.rect(x - 4, y - 3, yw * TILE + 8, yh * TILE + 6, 'k');
    for row in 0..yh * 4 {
        for col in 0..yw * 4 {
            if (row + col) % 3 == 0 {
                cv.put(x + col * 4, y + row * 4, 'g');
            }
        }
    }
    cv.line(x - 5 * TILE, y + 6 * TILE, x + yw * TILE, y + 6 * TILE, 'P');
    let sign = scroll(&[("SQUIRES >".into(), 'o')]);
    cv.stamp(&sign, x - 5 * TILE, y + 5 * TILE);
    for s in 0..3 {
        let (bx, by, w, h) = yard::building(s);
        let (bx, by) = (bx * TILE - ox, by * TILE - oy);
        let level = run.home.squires_lodge.level(yard::BUILDINGS[s]);
        cv.rect(bx + 2, by + 8, w * TILE - 4, h * TILE - 8, 'g');
        cv.rect(bx + 4, by + 10, w * TILE - 8, h * TILE - 12, 'j');
        // Three separate tiled roofs and lit level windows.
        for r in 0..12 {
            cv.line(
                bx + 2 + r / 2,
                by + 12 - r,
                bx + w * TILE - 3 - r / 2,
                by + 12 - r,
                if r % 3 == 0 { 'P' } else { 'B' },
            );
        }
        cv.rect(bx + w * TILE / 2 - 5, by + 27, 10, 21, 'k');
        for l in 0..=level {
            cv.rect(bx + 7 + l as i32 * 10, by + 19, 5, 5, 'o');
        }
        cv.text(
            bx + 3,
            by + 36,
            &format!("{} L{}", yard::TITLES[s], level),
            'H',
        );
        for c in 0..3 {
            let (px, py) = yard::plate(s, c);
            let (px, py) = (px * TILE - ox, py * TILE - oy);
            cv.rect(px, py, TILE - 1, TILE - 1, 'g');
            cv.rect(px + 1, py + 1, TILE - 3, TILE - 3, 'k');
            cv.text(px + 4, py + 4, [">", "E", "+"][c], 'o');
        }
        for (c, label) in ["NEXT", ["HIRE", "PICK", "TRAIN"][s], "UP"].iter().enumerate() {
            let (px, py) = yard::plate(s, c);
            cv.text(px * TILE - ox, (py + 1) * TILE - oy + 3, label, 'H');
        }
    }
    cv.text(x + 24, y + 6 * TILE + 4, "SQUIRES' YARD", 'H');
}

/// Black paper scroll, grey lettering and a warm rule. Never blue chrome.
pub(super) fn scroll(lines: &[(String, char)]) -> Img {
    let w = lines.iter().map(|(s, _)| text_width(s)).max().unwrap_or(0) + 16;
    let h = lines.len() as i32 * 9 + 14;
    let mut im = Img::black(w, h);
    im.rect(0, 0, w, 2, 'P');
    im.rect(0, h - 2, w, 2, 'P');
    im.line(2, 3, 2, h - 4, 'g');
    im.line(w - 3, 3, w - 3, h - 4, 'g');
    for (i, (s, c)) in lines.iter().enumerate() {
        im.text(8, 7 + i as i32 * 9, s, *c);
    }
    im
}
pub(super) fn panel(run: &Run, seat: u32) -> Option<Img> {
    if run.room().kind != RoomKind::Kingdom {
        return None;
    }
    let h = run.players.get(&seat)?;
    let s = yard::nearby(h.x, h.y)?;
    let g = &run.home.squires_lodge;
    let roster = run.squire_roster(seat);
    let count = roster.map_or(0, |r| r.members.len());
    let cap = g.roster_cap();
    let page = run.squire_page(seat, s);
    let mut lines = vec![
        (
            format!(
                "SQUIRES / {} L{}",
                yard::TITLES[s],
                g.level(yard::BUILDINGS[s])
            ),
            'o',
        ),
        (
            format!(
                "{}  ROSTER {count}/{cap}",
                h.name.to_uppercase().chars().take(18).collect::<String>()
            ),
            'H',
        ),
        (
            format!("TREASURY {}G", run.treasury.get(crate::realm::Spoil::Gold)),
            'H',
        ),
    ];
    if s == 0 {
        let offers = run.squire_recruits(seat);
        let page = page % offers.len().max(1);
        if let Some(m) = offers.get(page) {
            lines.push((
                format!(
                    "OFFER {}/{}  {}",
                    page + 1,
                    offers.len(),
                    m.kind.name().to_uppercase()
                ),
                'o',
            ));
            member(&mut lines, m);
            let hired = roster.is_some_and(|r| r.members.iter().any(|a| a.id == m.id));
            lines.push((
                if hired {
                    "ALREADY HIRED".into()
                } else {
                    format!("HIRE {}G", g.hire_price(m.kind))
                },
                'H',
            ));
        }
        lines.push((format!("BOARD D{} / RENT {}", g.delves, g.rent_days), 'H'));
        lines.push(("NEW OFFERS: DELVE / RENT DAY".into(), 'H'));
    } else {
        let members = roster.map_or(&[][..], |r| r.members.as_slice());
        let n = members.len() + usize::from(s == 1);
        let page = page % n.max(1);
        lines.push((format!("VIEWING {} OF {}", page + 1, n.max(1)), 'o'));
        if let Some(m) = members.get(page) {
            member(&mut lines, m);
            let active = roster.is_some_and(|r| r.active == Some(m.id));
            lines.push((
                if active {
                    "ACTIVE / ONE ONLY".into()
                } else {
                    "RESTING / ONE ACTIVE".into()
                },
                'H',
            ));
            if s == 2 {
                lines.push((
                    format!("TRAIN {}G / CAP L{}", g.training_price(m), g.training_cap()),
                    'o',
                ));
                if m.level >= g.training_cap() {
                    lines.push((
                        if g.training == crate::rules::squires_lodge::MAX_BUILDING {
                            "MAX TRAINING L8".into()
                        } else {
                            "AT CAP / UPGRADE TRAINING".into()
                        },
                        'H',
                    ));
                } else {
                    lines.push((
                        format!(
                            "NEXT: HP {}  ATK {}",
                            m.kind.health(m.level + 1),
                            m.kind.damage(m.level + 1)
                        ),
                        'H',
                    ));
                }
            } else {
                lines.push(("SELECT FREE".into(), 'H'));
            }
        } else {
            lines.push((
                if s == 1 {
                    "NONE / TRAVEL ALONE".into()
                } else {
                    "NO MEMBERS / RECRUIT FIRST".into()
                },
                'H',
            ));
        }
    }
    let active = roster.and_then(|r| {
        r.active
            .and_then(|id| r.members.iter().find(|m| m.id == id))
    });
    lines.push((
        format!(
            "ACTIVE: {}",
            active.map_or("NONE".into(), |m| m.name.to_uppercase())
        ),
        'H',
    ));
    lines.push((
        match g.upgrade_price(yard::BUILDINGS[s]) {
            Some(cost) => format!("UPGRADE {}G -> L{}", cost, g.level(yard::BUILDINGS[s]) + 1),
            None => "BUILDING MAX L3".into(),
        },
        'o',
    ));
    let benefit = match s {
        0 => "MORE OFFERS / HIGHER LEVELS",
        1 => "LOWER HIRE PRICES",
        _ => "HIGHER TRAINING CAP",
    };
    lines.push((benefit.into(), 'H'));
    lines.push(("STAND ON PLATE / HOLD E".into(), 'H'));
    lines.push((format!("> NEXT / E {} / + UP", ["HIRE", "SELECT", "TRAIN"][s]), 'H'));
    lines.push(("WALK AWAY: CLOSE".into(), 'H'));
    if h.buying > 0 && yard::station_at(h.x, h.y).is_some() {
        let filled = (h.buying.min(home::BUY_HOLD) * 18 / home::BUY_HOLD) as usize;
        lines.push((
            format!("[{}{}]", "=".repeat(filled), "-".repeat(18 - filled)),
            'o',
        ));
    }
    if let Some((when, who, line)) = &run.found
        && *who == seat
        && run.tick.saturating_sub(*when) < 3 * crate::rules::HZ as u64
    {
        lines.push((line.chars().take(48).collect(), 'o'));
    }
    Some(scroll(&lines))
}
fn member(lines: &mut Vec<(String, char)>, m: &crate::rules::squires_lodge::Member) {
    lines.push((m.name.to_uppercase(), 'H'));
    lines.push((
        format!(
            "{} L{}  HP {} ATK {}",
            m.kind.name().to_uppercase(),
            m.level,
            m.kind.health(m.level),
            m.kind.damage(m.level)
        ),
        'H',
    ));
    lines.push((yard::ability(m.kind, m.level), 'H'));
}
