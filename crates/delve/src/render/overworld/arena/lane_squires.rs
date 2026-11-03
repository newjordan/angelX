//! The company in ink: small companions in the crowd, and three quiet
//! counters in the yard. Geometry and prices belong to the rules; this
//! module only reads them. Black paper, grey edges, one ochre highlight.

use super::super::ink::{Img, text_width};
use super::{TILE, at, knights, shoppers, stand, stand_at};
use crate::realm::Spoil;
use crate::rules::lane_squires::{self, Action, Building, Kind, Recruit, Squire};
use crate::rules::{RoomKind, Run, TILE_UNITS};

type Rect = (i32, i32, i32, i32);

fn here(run: &Run) -> bool {
    run.at_home_now() && run.room().kind == RoomKind::SquireLanes
}

fn inside((c, r, w, h): Rect, x: f32, y: f32) -> bool {
    let (x, y) = (x / TILE_UNITS, y / TILE_UNITS);
    x >= c as f32 && x < (c + w) as f32 && y >= r as f32 && y < (r + h) as f32
}

/// A cut-pixel border, deliberately not the blue/slate chrome treatment.
fn paper(cv: &mut Img, (x, y, w, h): Rect, lit: bool) {
    cv.rect(x, y, w, h, 'k');
    cv.line(x + 1, y, x + w - 2, y, if lit { 'O' } else { 'j' });
    cv.line(x + 1, y + h - 1, x + w - 2, y + h - 1, 'g');
    cv.line(x, y + 1, x, y + h - 2, 'g');
    cv.line(x + w - 1, y + 1, x + w - 1, y + h - 2, 'g');
}

fn label(cv: &mut Img, x: i32, y: i32, width: i32, words: &str, ink: char) {
    // Names are player/persistence data. Keep even an unusually long one
    // inside its column rather than painting through the next price.
    let mut s = String::new();
    for ch in words.to_uppercase().chars() {
        let mut next = s.clone();
        next.push(ch);
        if text_width(&next) > width {
            break;
        }
        s = next;
    }
    cv.text(x, y, &s, ink);
}

fn building_name(building: Building) -> &'static str {
    match building {
        Building::Board => "BOARD",
        Building::Hall => "HALL",
        Building::Yard => "YARD",
    }
}

fn talents(kind: Kind) -> (&'static str, Option<&'static str>) {
    match kind {
        Kind::Ninja => ("AT L2 BACKSTAB", Some("AT L4 SMOKE")),
        Kind::Healer => ("AT L2 CLEANSE", Some("AT L4 SKULL ASSIST")),
        Kind::Ranger => ("AT L2 MARK", None),
        Kind::Shield => ("AT L2 GUARD", None),
        Kind::Alchemist => ("AT L2 SPLASH", None),
    }
}

/// All five use the kit's ordinary indexed sprite path. The coloured
/// tabard is a few pixels, not a second UI accent or an aura.
fn upright(kind: Kind) -> Img {
    Img::from_rows(match kind {
        Kind::Ninja => &[
            "....gggg.....",
            "...gjjjjg....",
            "..gjjjjjjg...",
            "..gHkHkHjg...",
            "...gjjjjg....",
            "....g77g.....",
            ".H..j77j..H..",
            ".i.gj77jg.i..",
            ".i.gj77jg.i..",
            "...gjjjjg....",
            "....jggj.....",
            "....j..j.....",
            "...gg..gg....",
            "...jj..jj....",
        ],
        Kind::Healer => &[
            "....hHHh.....",
            "...hHjjHh....",
            "...hjHHjh..h.",
            "...hjHkjh.hHh",
            "....hHHh...h.",
            "...hH77Hh..j.",
            "..hHH77HHh.j.",
            "..hHH77HHhHj.",
            "..hHj77jHh.j.",
            "...HjjjjH..j.",
            "...HjjjjH..j.",
            "...HjjjjH..j.",
            "...hHHHHh..j.",
            "....gggg...j.",
        ],
        Kind::Ranger => &[
            "....jjj......",
            "...jhhjg.....",
            "..jhhhjgg..h.",
            "..jHkHjgg...h",
            "...jHHjg....h",
            "....j77j..h.h",
            "...jj77jj.h.h",
            "..hjj77jjHh.h",
            "..hgj77jg.h.h",
            "...gjjjjg.h.h",
            "....jggj....h",
            "....j..j...h.",
            "...gg..gg....",
            "...jj..jj....",
        ],
        Kind::Shield => &[
            "...jHHHj.....",
            "..jHhhhHj....",
            "..jHkkkHj....",
            "...jHHHj.....",
            "....jHj......",
            "...j777j.....",
            "..jj777jHHHh.",
            "..Hj77jHjjjHh",
            "..Hj77jHjHjHh",
            "...jjjjHjHjHh",
            "....jgjHjjjHh",
            "....j..hHHHh.",
            "...gg..gg....",
            "...jj..jj....",
        ],
        Kind::Alchemist => &[
            "....jjjj.....",
            "...jhHHhj....",
            "...HjHHjH....",
            "...HHkkHH....",
            "....hHHh.....",
            "...j777j.....",
            "..jj777jj..h.",
            "..Hj777jH..H.",
            "..HjjjjjH.hHh",
            "...jhhjj..HjH",
            "...jhHjj..hHh",
            "....j..j.....",
            "...gg..gg....",
            "...jj..jj....",
        ],
    })
}

pub(super) fn sprite(recruit: &Recruit, owner: u32, down: bool, tick: u32) -> Img {
    let mut im = if down {
        Img::from_rows(&[
            ".............",
            "..jjj........",
            ".jhHhjgggg...",
            ".jHHjj77jjjj.",
            "..jjjjjjjjjj.",
        ])
    } else {
        upright(recruit.kind)
    }
    .recolor(knights::seat_colours(owner));
    if !down {
        let cloth = knights::seat_cloth(owner);
        // Broad role cues survive the full-room zoom: crossed knives,
        // a cross-headed staff, an open bow, a shield, and a round flask.
        im.rect(5, 6, 3, 3, cloth);
        match recruit.kind {
            Kind::Ninja => {
                im.line(0, 10, 3, 5, 'H');
                im.line(9, 5, 12, 10, 'H');
                im.line(0, 4, 3, 4, cloth);
            }
            Kind::Healer => {
                im.rect(9, 2, 4, 2, 'H');
                im.rect(10, 1, 2, 5, 'H');
                im.line(11, 6, 11, 13, 'h');
            }
            Kind::Ranger => {
                im.line(9, 3, 12, 6, 'H');
                im.line(12, 6, 12, 9, 'H');
                im.line(12, 9, 9, 12, 'H');
                im.line(9, 4, 9, 11, 'g');
                im.line(7, 7, 12, 7, 'h');
            }
            Kind::Shield => {
                im.rect(8, 6, 5, 6, 'h');
                im.rect(9, 7, 3, 4, cloth);
                im.line(10, 7, 10, 10, 'H');
            }
            Kind::Alchemist => {
                im.rect(10, 5, 2, 2, 'H');
                im.rect(9, 7, 4, 4, 'h');
                im.rect(10, 8, 2, 2, cloth);
            }
        }
    }
    // A tiny alternating heel, not a bob that changes foot registration.
    if !down && ((tick / 12 + recruit.id) & 1) == 1 {
        im.put(4, im.h - 1, 'g');
    }
    im
}

pub(super) fn companion(cv: &mut Img, squire: &Squire, tick: u32) {
    let down = squire.hp == 0;
    let mut im = sprite(&squire.recruit, squire.owner, down, tick);
    im.outline_outside('k');
    let (x, y) = at(squire.x, squire.y);
    cv.ellipse(x as f32, y as f32, 5.0, 1.5, 'k');
    cv.rect(x - 4, y, 9, 3, 'k');
    cv.line(
        x - 3,
        y + 1,
        x + 3,
        y + 1,
        knights::seat_cloth(squire.owner),
    );
    if squire.smoke > 0 {
        // Neutral, broken wisps. No light pool; the owner stripe remains
        // legible through the ninja's thinning silhouette.
        for n in 0..7 {
            let dx = ((n * 7 + (tick / 4 % 19) as i32) % 19) - 9;
            let dy = ((n * 5 + (tick / 3 % 13) as i32) % 13) - 12;
            let ink = if n % 2 == 0 { 'g' } else { 'j' };
            cv.line(x + dx, y + dy, x + dx + 2, y + dy, ink);
        }
        let owner = super::super::ink::ink(knights::seat_cloth(squire.owner));
        for iy in 0..im.h {
            for ix in 0..im.w {
                if (ix + iy + (tick / 4 % 3) as i32) % 3 == 0 && im.get(ix, iy) != owner {
                    im.clear(ix, iy);
                }
            }
        }
    }
    stand_at(cv, &im, squire.x, squire.y, 0);
    cv.line(
        x - 2,
        y + 2,
        x + 2,
        y + 2,
        knights::seat_cloth(squire.owner),
    );
    if down {
        cv.text(x - 11, y + 5, "REST", 'h');
    } else if squire.hp < squire.recruit.kind.health(squire.recruit.level) {
        let max = squire.recruit.kind.health(squire.recruit.level).max(1);
        cv.rect(x - 5, y - im.h - 3, 10, 2, 'g');
        let width = (squire.hp.min(max) * 10 / max).max(1) as i32;
        cv.rect(x - 5, y - im.h - 3, width, 1, 'O');
    }
}

/// The fixed services live north of the quintains; roster and drills stay
/// west of Grubbins. In particular, no new furniture stands at y=6.5.
pub(super) fn figures(cv: &mut Img, run: &Run, focus: Option<u32>) {
    if !here(run) {
        return;
    }
    let company = &run.home.lane_squires;
    for (building, x, width) in [
        (Building::Board, 32, 100),
        (Building::Hall, 160, 100),
        (Building::Yard, 288, 68),
    ] {
        paper(cv, (x, 6, width, 23), false);
        label(
            cv,
            x + 4,
            9,
            width - 8,
            &format!("{} L{}", building_name(building), company.level(building)),
            'H',
        );
        let price = company
            .upgrade_price(building)
            .map_or_else(|| "MAX".into(), |p| format!("BUILD {p}G"));
        label(cv, x + 4, 20, width - 8, &price, 'O');
    }
    for (action, rect) in lane_squires::plates(company) {
        let (c, r, w, h) = rect;
        let lit = run
            .players
            .values()
            .any(|hero| hero.hp > 0 && inside(rect, hero.x, hero.y));
        let (x, y, pw, ph) = (c * TILE + 1, r * TILE + 1, w * TILE - 2, h * TILE - 2);
        paper(cv, (x, y, pw, ph), lit);
        let words = match action {
            Action::Hire(n) => company
                .offers
                .get(n)
                .map_or_else(|| "-".into(), |offer| format!("{}G", company.wage(offer))),
            Action::Select(n) => format!("{}", n + 1),
            Action::Upgrade(_) => "UP".into(),
            Action::Train => "DRILL".into(),
        };
        label(
            cv,
            x + 3,
            if action == Action::Train {
                y + ph + 2
            } else {
                y + 4
            },
            pw - 6,
            &words,
            if lit { 'O' } else { 'h' },
        );
        if let Action::Hire(n) = action
            && let Some(recruit) = company.offers.get(n)
        {
            // Unhired recruits have no owner's livery yet.
            let im = upright(recruit.kind).recolor(&[('7', 'j')]);
            stand(cv, &im, c * TILE + w * TILE / 2, r * TILE - 3);
        }
    }
    // The focused visitor supplies both the floor preview and the ledger.
    // A shared camera uses the first living visitor, never a merged roster.
    if let Some((id, hero)) = shoppers(run, focus).find(|(_, h)| h.hp > 0)
        && let Some(roster) = company
            .knights
            .get(&crate::rules::home::Home::knight_key(hero))
    {
        for (action, (c, r, w, _)) in lane_squires::plates(company) {
            let Action::Select(n) = action else { continue };
            let Some(recruit) = roster.members.get(n) else {
                continue;
            };
            let im = sprite(recruit, id, false, run.tick as u32);
            stand(cv, &im, c * TILE + w * TILE / 2, r * TILE - 3);
            if roster.active == Some(recruit.id) {
                cv.line(
                    c * TILE + 5,
                    r * TILE - 1,
                    (c + w) * TILE - 6,
                    r * TILE - 1,
                    'O',
                );
            }
        }
    }
}

fn recruit_lines(recruit: &Recruit) -> Vec<String> {
    let (lv2, lv4) = talents(recruit.kind);
    let mut lines = vec![
        format!("{} LV{}", recruit.kind.name(), recruit.level),
        format!(
            "HP {}  POWER {}",
            recruit.kind.health(recruit.level),
            recruit.kind.power(recruit.level)
        ),
        lv2.into(),
    ];
    if let Some(lv4) = lv4 {
        lines.push(lv4.into());
    }
    lines
}

/// A two-column ledger: the thing being read, and this knight's company.
/// Instructions intentionally stay in ordinary two/three-word language.
pub(super) fn boards(cv: &mut Img, run: &Run, focus: Option<u32>) {
    if !here(run) {
        return;
    }
    let company = &run.home.lane_squires;
    let Some((id, hero, action)) =
        shoppers(run, focus)
            .filter(|(_, h)| h.hp > 0)
            .find_map(|(id, hero)| {
                lane_squires::plates(company)
                    .into_iter()
                    .find(|(_, rect)| inside(*rect, hero.x, hero.y))
                    .map(|(action, _)| (id, hero, action))
            })
    else {
        return;
    };
    let gold = run.treasury.get(Spoil::Gold);
    let who = crate::rules::home::Home::knight_key(hero);
    let roster = company.knights.get(&who);
    let active = company.active(&who);
    let count = roster.map_or(0, |r| r.members.len());
    let title;
    let mut lines = Vec::new();
    let instruction;
    match action {
        Action::Hire(n) => {
            if let Some(recruit) = company.offers.get(n) {
                title = recruit.name.clone();
                lines = recruit_lines(recruit);
                lines.insert(
                    2,
                    format!("PRICE {}G  BANK {}G", company.wage(recruit), gold),
                );
                instruction = if count >= company.cap() {
                    "ROSTER FULL"
                } else if gold < company.wage(recruit) {
                    "NEED GOLD"
                } else {
                    "HOLD E HIRE"
                };
            } else {
                title = "NO OFFER".into();
                lines.push("NEW RECRUITS TOMORROW".into());
                instruction = "COME BACK";
            }
        }
        Action::Upgrade(building) => {
            title = format!(
                "{} LEVEL {}",
                building_name(building),
                company.level(building)
            );
            lines.push(match building {
                Building::Board => format!("{} DAILY OFFERS", company.offer_count()),
                Building::Hall => format!("{} ROSTER SLOTS", company.cap()),
                Building::Yard => format!("TRAIN TO LEVEL {}", company.max_level()),
            });
            lines.push(match building {
                Building::Board => "MORE CHOICE EACH DAY".into(),
                Building::Hall => "MORE SLOTS, LOWER WAGES".into(),
                Building::Yard => "RAISE THE TRAINING LIMIT".into(),
            });
            if let Some(price) = company.upgrade_price(building) {
                lines.push(format!("UPGRADE {price}G  BANK {}G", gold));
                instruction = if gold < price {
                    "NEED GOLD"
                } else {
                    "HOLD E UPGRADE"
                };
            } else {
                instruction = "FULLY UPGRADED";
            }
        }
        Action::Select(n) => {
            if let Some(recruit) = roster.and_then(|r| r.members.get(n)) {
                title = recruit.name.clone();
                lines = recruit_lines(recruit);
                let selected = active.is_some_and(|r| r.id == recruit.id);
                lines.insert(
                    2,
                    if selected {
                        "FOLLOWING THIS KNIGHT".into()
                    } else {
                        "RESTING IN THE HALL".into()
                    },
                );
                instruction = if selected {
                    "HOLD E REST"
                } else {
                    "HOLD E EQUIP"
                };
            } else {
                title = "EMPTY ROSTER SLOT".into();
                lines.push("HIRE AT THE BOARD".into());
                instruction = "CHOOSE A RECRUIT";
            }
        }
        Action::Train => {
            if let Some(recruit) = active {
                title = format!("TRAIN {}", recruit.name);
                lines = recruit_lines(recruit);
                let drills = roster
                    .and_then(|r| r.drills.get(&recruit.id))
                    .copied()
                    .unwrap_or(0);
                lines.insert(
                    2,
                    format!("PRICE {}G  BANK {}G", company.training_price(recruit), gold),
                );
                lines.push(format!("DRILLS {drills}  LIMIT LV{}", company.max_level()));
                instruction = if recruit.level >= company.max_level() {
                    "YARD LIMIT"
                } else if drills == 0 {
                    "DELVE FOR DRILLS"
                } else if gold < company.training_price(recruit) {
                    "NEED GOLD"
                } else {
                    "HOLD E TRAIN"
                };
            } else {
                title = "TRAINING YARD".into();
                lines.push("CHOOSE A FOLLOWER FIRST".into());
                instruction = "EQUIP A SQUIRE";
            }
        }
    }
    let height = 108;
    // Put the ledger away from the row being read and above the existing
    // skills tray. A room board must not put its instructions under HUD ink.
    let y = if matches!(action, Action::Select(_) | Action::Train) {
        4
    } else {
        let bottom = if super::hud_in_band() {
            cv.h
        } else {
            super::tray::bar_rect(run, focus, cv.w, cv.h).map_or(cv.h, |(_, y, _, _)| y)
        };
        (bottom - height - 4).max(4)
    };
    paper(cv, (6, y, cv.w - 12, height), true);
    let split = cv.w - 150;
    cv.line(split - 7, y + 8, split - 7, y + height - 9, 'g');
    label(cv, 14, y + 8, split - 28, &title, 'H');
    for (n, line) in lines.iter().enumerate() {
        label(cv, 14, y + 21 + n as i32 * 11, split - 28, line, 'h');
    }
    label(cv, 14, y + height - 13, split - 28, instruction, 'O');
    label(
        cv,
        split,
        y + 8,
        130,
        &format!("COMPANY {count}/{}", company.cap()),
        'H',
    );
    if let Some(roster) = roster.filter(|r| !r.members.is_empty()) {
        for (n, recruit) in roster.members.iter().take(company.cap()).enumerate() {
            let selected = roster.active == Some(recruit.id);
            let prefix = if selected { ">" } else { " " };
            let name: String = recruit.name.chars().take(12).collect();
            label(
                cv,
                split,
                y + 22 + n as i32 * 11,
                130,
                &format!("{prefix}{} {name} L{}", n + 1, recruit.level),
                if selected { 'O' } else { 'h' },
            );
        }
    } else {
        cv.text(split, y + 24, "NO SQUIRES", 'h');
    }
    cv.line(split, y + 94, split + 4, y + 94, knights::seat_cloth(id));
    cv.text(split + 9, y + 91, "YOUR COMPANY", 'j');
}

#[cfg(test)]
#[path = "../../../../tests/render/overworld__squires_shots.rs"]
mod tests;
