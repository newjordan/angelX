//! Small owner-coloured companions and three quiet black-paper yard desks.
use super::super::ink::{Img, Rgb, rgb, text_width};
use super::{TILE, at, knights, sprites, stand};
use crate::realm::squires::{Building, Kind, Squire};
use crate::rules::{
    Run,
    home::Home,
    squire_hall::{PLATES, plate_at},
};

const PAPER: Rgb = rgb(0x11110f);
const EDGE: Rgb = rgb(0x504738);
const PANEL_W: i32 = 216;
const SIGN_Y: i32 = 12;
const MARGIN: i32 = 6;
const ACTIONS: [&str; 9] = [
    "NEXT", "HIRE", "UPGRADE", "SELECT", "REST", "UPGRADE", "TRAIN", "ROSTER", "UPGRADE",
];
const STATIONS: [&str; 3] = ["RECRUIT", "HIRING", "TRAIN"];

fn fill(im: &mut Img, x: i32, y: i32, w: i32, h: i32, colour: Rgb) {
    for yy in y..y + h {
        for xx in x..x + w {
            im.set(xx, yy, colour);
        }
    }
}

fn paper(w: i32, h: i32) -> Img {
    let mut im = Img::new(w, h);
    fill(&mut im, 0, 0, w, h, EDGE);
    fill(&mut im, 1, 1, w - 2, h - 2, PAPER);
    im
}

fn station_actions(i: usize) -> String {
    format!("{} / {} / UP", ACTIONS[i * 3], ACTIONS[i * 3 + 1])
}

// The 5x7 font normally advances six pixels for a blank. Three still leaves
// four clear pixels beside each slash and keeps TRAIN / ROSTER / UP bounded.
fn station_actions_width(actions: &str) -> i32 {
    text_width(actions) - actions.matches(' ').count() as i32 * 3
}

fn station_sign(run: &Run, i: usize) -> Img {
    let c = &run.home.squire_hall;
    let mut sign = paper(108, 34);
    let title = format!("{} LV{}", STATIONS[i], [c.recruit, c.hire, c.train][i] + 1);
    sign.text((sign.w - text_width(&title)) / 2, 3, &title, 'O');
    // Explicit separators keep the three left-to-right slab actions distinct.
    let actions = station_actions(i);
    let mut x = (sign.w - station_actions_width(&actions)) / 2;
    for word in actions.split(' ') {
        x = sign.text(x, 13, word, 'i') + 3;
    }
    let upgrade = format!("UPGRADE {}", STATIONS[i]);
    sign.text((sign.w - text_width(&upgrade)) / 2, 23, &upgrade, 'h');
    sign
}

fn signs(cv: &mut Img, run: &Run) {
    for i in 0..3 {
        cv.stamp(&station_sign(run, i), PLATES[i * 3].0 * TILE - 6, SIGN_Y);
    }
}

/// Words live on the north wall, above a standing knight's silhouette,
/// never on the trigger slabs. Each station names its own improvement.
pub(super) fn yard(cv: &mut Img, _run: &Run) {
    for (i, &(c, r, w, h)) in PLATES.iter().enumerate() {
        let (x, y) = (c * TILE, r * TILE);
        fill(cv, x + 1, y + 1, w * TILE - 2, h * TILE - 2, EDGE);
        fill(cv, x + 2, y + 2, w * TILE - 4, h * TILE - 4, rgb(0x30302b));
        // Brass chevrons distinguish improvements without a floor word
        // that the person using it would erase. The dock spells it out.
        if i % 3 == 2 {
            cv.line(x + 10, y + 9, x + 16, y + 4, 'O');
            cv.line(x + 16, y + 4, x + 22, y + 9, 'O');
        }
    }
}

#[derive(Debug)]
struct Panel {
    lines: Vec<(String, char)>,
    prompt: String,
    enabled: bool,
    progress: u32,
}

fn short_name(name: &str) -> String {
    let name = name.to_uppercase();
    if name.chars().count() <= 14 {
        name
    } else {
        format!("{}...", name.chars().take(11).collect::<String>())
    }
}

fn stats(lines: &mut Vec<(String, char)>, s: &Squire) {
    lines.push((
        format!("{} - {} LV{}", short_name(&s.name), s.kind.name(), s.level),
        'i',
    ));
    lines.push((format!("HP {}   POWER {}", s.health(), s.power()), 'h'));
    lines.push((
        format!("KIT {}", s.kind.abilities(s.level).to_uppercase()),
        'i',
    ));
}

fn payment(lines: &mut Vec<(String, char)>, price: u32, gold: u32) -> bool {
    lines.push((format!("COST {price}G / TREASURY {gold}G"), 'h'));
    if gold < price {
        lines.push((format!("NEED {}G", price - gold), 'O'));
        false
    } else {
        lines.push(("GOLD READY".into(), 'O'));
        true
    }
}

fn panel(run: &Run, id: u32, hero: &crate::rules::Hero, plate: usize) -> Panel {
    let c = &run.home.squire_hall;
    let who = Home::knight_key(hero);
    let roster = c.rosters.get(&who);
    let active = roster.and_then(|r| r.squires.iter().find(|s| Some(s.id) == r.active));
    let count = roster.map_or(0, |r| r.squires.len());
    let gold = run.treasury.get(crate::realm::Spoil::Gold);
    let station = plate / 3;
    let level = [c.recruit, c.hire, c.train][station];
    let mut lines = vec![(format!("{} LV{}", STATIONS[station], level + 1), 'O')];
    let enabled;
    match plate {
        0 | 1 => {
            if let Some(s) = c
                .offers
                .get(run.squire_hall.offers.get(&id).copied().unwrap_or(0) % c.offers.len().max(1))
            {
                stats(&mut lines, s);
                let paid = payment(&mut lines, c.wage(s), gold);
                lines.push((format!("ROSTER {count}/{}", c.cap()), 'h'));
                if count >= c.cap() {
                    lines.push(("ROSTER FULL".into(), 'O'));
                }
                enabled = plate == 0 || (paid && count < c.cap());
            } else {
                lines.push(("NO OFFERS".into(), 'i'));
                enabled = false;
            }
        }
        3 | 4 | 7 => {
            if let Some(s) = active {
                stats(&mut lines, s);
                lines[1].0.insert_str(0, "> ");
            } else {
                lines.push(("NO ACTIVE SQUIRE".into(), 'i'));
            }
            lines.push((format!("ROSTER {count}/{}", c.cap()), 'h'));
            if let Some(r) = roster {
                let start = r
                    .squires
                    .iter()
                    .position(|s| Some(s.id) == r.active)
                    .unwrap_or(0);
                for s in r
                    .squires
                    .iter()
                    .cycle()
                    .skip(start)
                    .take(r.squires.len().min(3))
                {
                    // The active squire already has the detailed lines above.
                    if Some(s.id) != r.active {
                        lines.push((
                            format!("{} {} LV{}", short_name(&s.name), s.kind.name(), s.level),
                            'h',
                        ));
                    }
                }
            }
            enabled = if plate == 4 {
                active.is_some()
            } else {
                count > 0
            };
        }
        6 => {
            if let Some(s) = active {
                lines[0].0.push_str(" / ACTIVE SQUIRE");
                stats(&mut lines, s);
                if s.level < c.ceiling() {
                    let mut next = s.clone();
                    next.level += 1;
                    lines.push((
                        format!(
                            "NEXT LV{} / HP {} / POWER {}",
                            next.level,
                            next.health(),
                            next.power()
                        ),
                        'O',
                    ));
                    let current = s.kind.abilities(s.level);
                    let unlocked: Vec<_> = s
                        .kind
                        .abilities(next.level)
                        .split(" / ")
                        .filter(|ability| !current.split(" / ").any(|old| old == *ability))
                        .collect();
                    lines.push((
                        if unlocked.is_empty() {
                            "KIT UNCHANGED".into()
                        } else {
                            format!("UNLOCK {}", unlocked.join(" / ").to_uppercase())
                        },
                        'i',
                    ));
                    enabled = payment(&mut lines, c.training_price(s), gold);
                } else {
                    lines.push((format!("LEVEL CAP {}", c.ceiling()), 'O'));
                    lines.push(("UPGRADE TRAIN".into(), 'h'));
                    enabled = false;
                }
            } else {
                lines.push(("SELECT A SQUIRE".into(), 'i'));
                enabled = false;
            }
        }
        _ => {
            let b = [Building::Recruit, Building::Hire, Building::Train][station];
            lines[0].0 = format!("UPGRADE {}", STATIONS[station]);
            if level >= 3 {
                lines.push(("LV4 / COMPLETE".into(), 'i'));
                enabled = false;
            } else {
                lines.push((format!("LV{} > LV{}", level + 1, level + 2), 'i'));
                lines.push((
                    match b {
                        Building::Recruit => format!("{} OFFERS / BETTER TALENT", 4 + c.recruit),
                        Building::Hire => format!("{} PLACES / LOWER WAGES", c.cap() + 2),
                        Building::Train => {
                            format!("LEVEL CAP {} / CHEAPER LESSONS", c.ceiling() + 2)
                        }
                    },
                    'h',
                ));
                enabled = payment(&mut lines, c.upgrade_price(b), gold);
            }
        }
    }
    let prompt = if enabled {
        format!("HOLD E {}", ACTIONS[plate])
    } else {
        format!("{} BLOCKED", ACTIONS[plate])
    };
    let progress = if enabled && run.squire_hall.holds.get(&id) == Some(&plate) {
        hero.buying.min(crate::rules::home::BUY_HOLD)
    } else {
        0
    };
    Panel {
        lines,
        prompt,
        enabled,
        progress,
    }
}

fn panel_image(panel: &Panel) -> Img {
    let h = panel.lines.len() as i32 * 10 + 27;
    let mut im = paper(PANEL_W, h);
    for (i, (line, ink)) in panel.lines.iter().enumerate() {
        im.text(8, 5 + i as i32 * 10, line, *ink);
    }
    im.text(
        8,
        h - 20,
        &panel.prompt,
        if panel.enabled { 'O' } else { 'h' },
    );
    let width = PANEL_W - 16;
    fill(&mut im, 8, h - 9, width, 4, rgb(0x393934));
    let filled = width * panel.progress as i32 / crate::rules::home::BUY_HOLD as i32;
    if filled > 0 {
        im.rect(8, h - 9, filled, 4, 'O');
    }
    im
}

fn dock(cv: &Img, im: &Img) -> (i32, i32) {
    ((cv.w - im.w) / 2, cv.h - im.h - MARGIN)
}

pub(super) fn board(cv: &mut Img, run: &Run, focus: Option<u32>) {
    // Sign lettering also belongs above the body layer, not under a helmet
    // if someone walks north of the trigger slabs.
    signs(cv, run);
    // Boards is painted after bodies. Dock in the room's native viewport,
    // not beside a helmet; one seat never reads another seat's company.
    let reading = super::shoppers(run, focus)
        .filter(|(_, h)| h.hp > 0 && !h.absent && !h.stone)
        .filter(|(id, _)| !run.exits.outside.contains_key(id))
        .find_map(|(id, h)| plate_at(h.x, h.y).map(|p| (id, h, p)));
    let Some((id, hero, plate)) = reading else {
        return;
    };
    let im = panel_image(&panel(run, id, hero, plate));
    let (x, y) = dock(cv, &im);
    cv.stamp(&im, x, y);
}

fn figure(kind: Kind, owner: u32) -> Img {
    // The same material/owner palette as a knight, sampled to a smaller
    // silhouette, with equipment that remains distinct without a label.
    let source = sprites::knight(owner);
    let mut im = Img::new(13, 13);
    for y in 0..10 {
        for x in 0..9 {
            if let Some(rgb) = source.get(x * source.w / 9, y * source.h / 10) {
                im.set(x + 2, y + 1, rgb);
            }
        }
    }
    match kind {
        Kind::Ninja => {
            im.rect(3, 4, 6, 2, 'k');
            im.put(4, 4, '9');
            im.put(7, 4, '9');
            im.line(10, 8, 12, 5, 'W');
        }
        Kind::Healer => {
            im.line(11, 5, 11, 14, 'O');
            im.line(9, 6, 12, 6, 'W');
            im.line(11, 4, 11, 8, 'W');
        }
        Kind::Ranger => {
            im.line(10, 4, 12, 7, 'O');
            im.line(12, 7, 10, 12, 'O');
            im.line(10, 4, 10, 12, 'h');
        }
        Kind::Shield => {
            im.rect(8, 7, 5, 6, 'U');
            im.frame(8, 7, 5, 6, 'W');
        }
        Kind::Alchemist => {
            im.rect(9, 9, 4, 4, 'C');
            im.rect(10, 7, 2, 2, 'O');
        }
    }
    // A sash uses exactly the owner's existing seat colour mapping.
    let sash = Img::from_rows(&["777"]).recolor(knights::realm_colours(owner));
    im.stamp(&sash, 4, 10);
    im
}
pub(super) fn delve(cv: &mut Img, run: &Run) {
    if run.at_home_now() {
        return;
    }
    for c in run
        .squire_hall
        .companions
        .iter()
        .filter(|c| run.squire_owner_here(c.owner))
    {
        let (x, y) = at(c.body.x, c.body.y);
        if c.smoke > 0 && !c.body.down() {
            for dx in [-5, 0, 5] {
                cv.line(x + dx - 2, y - 4, x + dx + 2, y - 4, 'h');
            }
            if (run.tick / 3).is_multiple_of(2) {
                continue;
            }
        }
        let mut im = figure(c.squire.kind, c.owner);
        if c.body.down() {
            im = sprites::quarter_turn(&im);
        } else if c.body.facing < 0.0 {
            im = im.flip_h();
        }
        stand(cv, &im, x, y);
        if c.body.hp < c.body.max_hp {
            cv.rect(x - 6, y + 2, 12, 2, 'k');
            cv.rect(
                x - 6,
                y + 2,
                (12 * c.body.hp / c.body.max_hp) as i32,
                1,
                'h',
            );
        }
        if let Some((id, _)) = c.mark {
            if let Some(e) = run.enemies.iter().find(|e| e.id == id && e.hp > 0) {
                let (mx, my) = at(e.x, e.y);
                cv.line(mx - 3, my - 15, mx, my - 12, 'O');
                cv.line(mx, my - 12, mx + 3, my - 15, 'O');
            }
        }
    }
}
#[cfg(test)]
#[path = "../../../../tests/render/arena__squires__tests.rs"]
mod tests;
