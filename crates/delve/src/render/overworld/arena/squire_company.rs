//! The squires' company in the realm's pixel language: small companions,
//! owner-coloured cloth, and paper notices in the Training Yard.

use super::super::ink::{Img, text_width};
use super::{at, knights, sprites, stand};
use crate::rules::{
    RoomKind, Run,
    squire_company::{Kind, company},
};

type Plate = (i32, i32, i32, i32);

fn inside((c, r, w, h): Plate, x: f32, y: f32) -> bool {
    let (x, y) = (x / crate::rules::TILE_UNITS, y / crate::rules::TILE_UNITS);
    x >= c as f32 && x < (c + w) as f32 && y >= r as f32 && y < (r + h) as f32
}

fn title(kind: Kind) -> &'static str {
    match kind {
        Kind::Ninja => "NINJA",
        Kind::Healer => "HEALER",
        Kind::Ranger => "RANGER",
        Kind::Shield => "SHIELD",
        Kind::Alchemist => "ALCHEMIST",
    }
}

fn role(kind: Kind) -> (&'static str, &'static str) {
    match kind {
        Kind::Ninja => ("CLOSE / FLANK", "STARS / BACKSTAB / SMOKE"),
        Kind::Healer => ("SUPPORT / MEND", "MENDS WOUNDED FRIENDS"),
        Kind::Ranger => ("RANGED / BOW", "ARROWS FROM THE BACK RANK"),
        Kind::Shield => ("GUARD / TAUNT", "DRAWS FOES OFF YOUR KNIGHT"),
        Kind::Alchemist => ("RANGED / FLASK", "THROWS A BURSTING FLASK"),
    }
}

// Thirteen pixels high against the grown knights' seventeen-plus. The hood,
// bonnet, feather/bow, pavise and bottle read before their colour does.
const NINJA: [&str; 13] = [
    "....gggg.....",
    "...gJJJjg....",
    "...jttKjg....",
    "...ggggggg...",
    "....777g..gg.",
    "...g7777g....",
    "..tg8777gt.i.",
    "..t.7777...H.",
    "....OOOO...j.",
    "....gggg.....",
    "....j..j.....",
    "...jj..jj....",
    "..KKK..KKK...",
];
const HEALER: [&str; 13] = [
    "....HHHH.....",
    "...HiiiiH....",
    "...itKtKi....",
    "....tttt.....",
    "....i77i.....",
    "...7HiiH7....",
    "..t7HiOH7t...",
    "..t.HOOHi.t..",
    "....HiOHi....",
    "....HiiH.....",
    "....iiiii....",
    "....j..j.....",
    "...KK..KK....",
];
const RANGER: [&str; 13] = [
    ".......T.....",
    "....rrT......",
    "...rOOrr.....",
    "..rrrrrrr..O.",
    "...tKtKt..O.j",
    "....ttt..O..j",
    "...77777.O..j",
    "..t78777tOHHj",
    "..t.77O7.O..j",
    "....OOOO..O.j",
    "....j..j...O.",
    "....j..j.....",
    "...KK..KK....",
];
const SHIELD: [&str; 13] = [
    "....jJJj.....",
    "...jHiHJj....",
    "...jKKKKj....",
    "....tttt.....",
    "...J777JJ....",
    "..tJ77jOOOj..",
    "..t.77OHiHO..",
    "....77Oi7iO..",
    "....OOOi7iO..",
    "....jjOi7iO..",
    "....j.OHiHO..",
    "....j..OOO...",
    "...KK..KK....",
];
const ALCHEMIST: [&str; 13] = [
    "....rrrr.....",
    "...rOOOOr....",
    "...tOKOKt....",
    "....tttt.....",
    "....777......",
    "...i777i.....",
    "..ti787it..B.",
    "..t.i77i.t.H.",
    "....iOOi..HiH",
    "....iiPi..HOH",
    "....j..j..jHj",
    "....j..j.....",
    "...KK..KK....",
];

fn figure(kind: Kind, colours: &[(char, char)], down: bool) -> Img {
    let rows = match kind {
        Kind::Ninja => &NINJA,
        Kind::Healer => &HEALER,
        Kind::Ranger => &RANGER,
        Kind::Shield => &SHIELD,
        Kind::Alchemist => &ALCHEMIST,
    };
    let im = Img::from_rows(rows).recolor(colours);
    if down {
        // A body at rest is a low, readable shape, not an upright square
        // merely rotated inside its old canvas.
        let turned = sprites::quarter_turn(&im);
        let mut prone = Img::new(turned.w, 6);
        for y in 0..6 {
            for x in 0..turned.w {
                if let Some(rgb) = turned.get(x, y * turned.h / 6) {
                    prone.set(x, y, rgb);
                }
            }
        }
        prone
    } else {
        im
    }
}

fn colours(run: &Run, owner: u32) -> &[(char, char)] {
    run.players
        .get(&owner)
        .filter(|h| !h.colours().is_empty())
        .map_or_else(|| knights::realm_colours(owner), |h| h.colours())
}

/// Separate from summoned minions: one little companion per knight, using
/// the ordinary Img/stand capture path, never a replacement for Beaumains.
pub(super) fn delve(cv: &mut Img, run: &Run) {
    let mut companions: Vec<_> = run.squire_field.companions.iter().collect();
    companions.sort_by(|(aid, a), (bid, b)| a.actor.y.total_cmp(&b.actor.y).then(aid.cmp(bid)));
    for (&owner, companion) in companions {
        let actor = &companion.actor;
        let down = actor.down();
        let mut im = figure(companion.squire.kind, colours(run, owner), down);
        if actor.facing < 0.0 {
            im = im.flip_h();
        }
        let (x, y) = at(actor.x, actor.y);
        // Do not blink an entire companion out in a crowded fight. An injured
        // one has a tiny grey health stitch; an ability moves just its tool ink.
        if !down && companion.effect > 0 {
            im.put(im.w - 2, im.h / 2, 'T');
        }
        stand(cv, &im, x, y + if down { 2 } else { 0 });
        if !down && actor.hp < actor.max_hp {
            cv.rect(x - 3, y - im.h - 2, 7, 2, 'K');
            let fill = (actor.hp as u64 * 5 / u64::from(actor.max_hp.max(1))) as i32;
            cv.rect(x - 2, y - im.h - 2, fill, 1, 'h');
        }
    }
}

/// Three pixels of the very same cloth as the body, with a grey pennant
/// point. Painted after the crowd: a knight may cover the small body, not its
/// allegiance. No nameplate, halo, colour cycling or second body recolour.
pub(super) fn foot_tabs(cv: &mut Img, run: &Run) {
    for (&owner, companion) in &run.squire_field.companions {
        if companion.actor.down() {
            continue;
        }
        let (x, y) = at(companion.actor.x, companion.actor.y);
        let cloth = colours(run, owner).iter().fold(
            '7',
            |cloth, &(from, to)| {
                if cloth == from { to } else { cloth }
            },
        );
        cv.rect(x - 2, y + 1, 5, 3, 'K');
        cv.rect(x - 1, y + 2, 3, 1, cloth);
        cv.put(x, y + 3, 'h');
    }
}

/// Paper prompts stay on the economy's hit rectangles, but their ink belongs
/// to Boards, never Ground: standing on an action must not hide its verb.
fn plate(cv: &mut Img, rect: Plate, label: &str, lit: bool) {
    let (c, r, w, h) = rect;
    let (x, y, w, h) = (
        c * super::TILE,
        r * super::TILE,
        w * super::TILE,
        h * super::TILE,
    );
    paper(cv, (x + 1, y + 1, w - 2, h - 2), lit);
    debug_assert!(text_width(label) <= w - 6);
    cv.text(
        x + (w - text_width(label)) / 2,
        y + 5,
        label,
        if lit { 'H' } else { 'i' },
    );
}

fn paper(cv: &mut Img, (x, y, w, h): Plate, lit: bool) {
    cv.rect(x, y, w, h, 'k');
    cv.frame(x, y, w, h, if lit { 'O' } else { 'G' });
}

/// Only the timber and slate live under actors. All paper, figures printed
/// on that paper, and prices are drawn together by `yard_notices` on Boards.
/// Nothing paints in Ector's x<160,y>=160 area.
pub(super) fn yard_ground(cv: &mut Img, _run: &Run) {
    for p in company::RECRUIT_PLATES {
        let (x, y) = (p.0 * super::TILE, p.1 * super::TILE);
        cv.rect(x, y - 2, 48, 2, 'r');
    }
    for p in company::UPGRADE_PLATES {
        let x = p.0 * super::TILE - 4;
        cv.rect(x + 4, 96, 34, 2, 'G');
        cv.rect(x + 1, 98, 40, 2, 'j');
        cv.rect(x, 100, 2, 28, 'G');
        cv.rect(x + 40, 100, 2, 28, 'G');
    }
}

fn selected_recruit(run: &Run, focus: Option<u32>) -> Option<usize> {
    super::shoppers(run, focus)
        .filter(|(_, h)| h.hp > 0)
        .find_map(|(_, h)| {
            company::RECRUIT_PLATES
                .iter()
                .position(|&p| inside(p, h.x, h.y))
        })
}

fn yard_notices(cv: &mut Img, run: &Run, focus: Option<u32>) {
    let company = &run.home.squire_company;
    let offers = company.offers();
    let selected = selected_recruit(run, focus);
    let occupied = |p| {
        run.players
            .values()
            .any(|h| h.hp > 0 && inside(p, h.x, h.y))
    };
    for (slot, &p) in company::RECRUIT_PLATES.iter().enumerate() {
        let (x, y) = (p.0 * super::TILE, p.1 * super::TILE);
        let lit = selected == Some(slot);
        paper(cv, (x + 1, y - 35, 46, 33), lit);
        // Numbered stock keys connect the top card to RECRUIT n in the panel.
        paper(cv, (x - 9, y - 34, 9, 11), lit);
        cv.text(
            x - 7,
            y - 32,
            &(slot + 1).to_string(),
            if lit { 'O' } else { 'h' },
        );
        if lit {
            cv.rect(x + 20, y - 40, 9, 5, 'k');
            cv.rect(x + 22, y - 39, 5, 1, 'O');
            cv.rect(x + 23, y - 38, 3, 1, 'O');
            cv.put(x + 24, y - 37, 'O');
        }
        if let Some(squire) = offers.get(slot) {
            let heading = if squire.kind == Kind::Alchemist {
                "ALCHEM"
            } else {
                title(squire.kind)
            };
            debug_assert!(text_width(heading) <= 40);
            cv.text(x + 4, y - 34, heading, 'i');
            cv.stamp(&figure(squire.kind, &[], false), x + 4, y - 25);
            cv.text(x + 22, y - 26, &format!("L{}", squire.level), 'H');
            if let Some(price) = company.hire_price(slot) {
                // A dedicated full-width row; all seven glyph rows clear the
                // sill. L1 and 110G cannot merge, clip, or be read as L1 10.
                let price = format!("{price}G");
                debug_assert!(text_width(&price) <= 40);
                cv.text(x + (48 - text_width(&price)) / 2, y - 11, &price, 'O');
            }
            plate(cv, p, "E HIRE", occupied(p));
        } else {
            cv.text(x + 6, y - 29, "SEALED", 'G');
            cv.text(x + 6, y - 18, "BOARD+", 'h');
            plate(cv, p, "LOCK", false);
        }
    }
    for (p, label) in [
        (company::SELECT_PLATE, "PICK"),
        (company::TRAIN_PLATE, "SPAR"), // a two-tile plate holds four letters
    ] {
        plate(cv, p, label, occupied(p));
    }
    for (station, (&p, (name, level))) in company::UPGRADE_PLATES
        .iter()
        .zip([
            ("BOARD", company.board),
            ("HALL", company.hall),
            ("YARD", company.yard),
        ])
        .enumerate()
    {
        let x = p.0 * super::TILE - 4;
        paper(cv, (x, 100, 42, 28), occupied(p));
        cv.text(x + 4, 102, name, 'i');
        cv.text(x + 4, 111, &format!("L{level}"), 'H');
        let cost = company
            .upgrade_price(station)
            .map_or_else(|| "MAX".into(), |g| format!("{g}G"));
        debug_assert!(text_width(&cost) <= 34);
        cv.text(x + 4, 120, &cost, 'O');
        plate(cv, p, "UP", occupied(p));
    }
}

fn short_name(name: &str, limit: usize) -> String {
    name.to_uppercase().chars().take(limit).collect()
}

fn roster_lines(run: &Run, name: &str) -> Vec<(String, char)> {
    let company = &run.home.squire_company;
    let roster = company.rosters.get(name);
    let mut lines = vec![(
        format!(
            "{}'S ROSTER {}/{}",
            short_name(name, 12),
            roster.map_or(0, |r| r.squires.len()),
            company.roster_cap()
        ),
        'i',
    )];
    if let Some(roster) = roster.filter(|r| !r.squires.is_empty()) {
        for s in &roster.squires {
            let active = roster.active == Some(s.id);
            lines.push((
                format!(
                    "{} {} L{} {}",
                    if active { ">" } else { "-" },
                    short_name(&s.name, 10),
                    s.level,
                    title(s.kind)
                ),
                if active { 'O' } else { 'h' },
            ));
        }
    } else {
        lines.push(("NO SQUIRE - HIRE AT THE BOARD".into(), 'h'));
    }
    lines
}

/// Text is separate from painting so the seat contract, costs, disabled
/// reasons and verbs have direct tests, not just a pixel hash.
fn notice(run: &Run, focus: Option<u32>) -> Option<Vec<(String, char)>> {
    let company = &run.home.squire_company;
    let hero = super::shoppers(run, focus).map(|(_, h)| h).find(|h| {
        h.hp > 0
            && (company::RECRUIT_PLATES
                .iter()
                .chain(company::UPGRADE_PLATES.iter())
                .any(|&p| inside(p, h.x, h.y))
                || inside(company::SELECT_PLATE, h.x, h.y)
                || inside(company::TRAIN_PLATE, h.x, h.y))
    })?;
    let roster = company.rosters.get(&hero.name);
    let mut lines;
    if let Some(slot) = company::RECRUIT_PLATES
        .iter()
        .position(|&p| inside(p, hero.x, hero.y))
    {
        lines = vec![(
            format!("{} / RECRUIT {}", short_name(&hero.name, 12), slot + 1),
            'i',
        )];
        if let Some(s) = company.offers().get(slot) {
            lines.push((
                s.name.to_uppercase(),
                'H',
            ));
            lines.push((
                format!("LEVEL {} / TRAIN CAP {}", s.level, company.level_cap()),
                'h',
            ));
            let (job, ability) = role(s.kind);
            lines.push((job.into(), 'O'));
            lines.push((ability.into(), 'h'));
            if let Some(price) = company.hire_price(slot) {
                lines.push((format!("E HIRE / {} GOLD", price), 'O'));
            }
            if roster.is_some_and(|r| r.claimed.contains(&s.id)) {
                lines.push(("ALREADY HIRED THIS DELVE".into(), 'i'));
            } else if roster.is_some_and(|r| r.squires.len() >= company.roster_cap()) {
                lines.push(("ROSTER FULL - UPGRADE HALL".into(), 'i'));
            } else {
                lines.push((format!("TREASURY {} GOLD", run.treasury.get(crate::realm::Spoil::Gold)), 'h'));
            }
            lines.push((
                format!(
                    "ROSTER {}/{}",
                    roster.map_or(0, |r| r.squires.len()),
                    company.roster_cap()
                ),
                'h',
            ));
        } else {
            lines.push(("NO APPLICANT AT THIS PLATE".into(), 'H'));
            lines.push(("UPGRADE BOARD FOR MORE OFFERS".into(), 'h'));
        }
    } else if inside(company::SELECT_PLATE, hero.x, hero.y) {
        lines = roster_lines(run, &hero.name);
        lines.push(("E SELECT / NEXT SQUIRE / FREE".into(), 'O'));
        lines.push(("ONE COMPANION NEXT DELVE".into(), 'h'));
        lines.push(("ACTIVE SQUIRE MARKED > ABOVE".into(), 'h'));
    } else if inside(company::TRAIN_PLATE, hero.x, hero.y) {
        lines = roster_lines(run, &hero.name);
        if let Some(price) = company.train_price(&hero.name) {
            lines.push((format!("E TRAIN / {} GOLD", price), 'O'));
            lines.push((format!("NEXT LEVEL / CAP {}", company.level_cap()), 'h'));
        } else {
            lines.push(("E TRAIN / NOT READY".into(), 'i'));
            lines.push((
                if let Some(s) =
                    roster.and_then(|r| r.squires.iter().find(|s| Some(s.id) == r.active))
                {
                    if s.level >= company.level_cap() {
                        "AT LEVEL CAP - UPGRADE YARD"
                    } else {
                        "REST - DELVE THEN RETURN"
                    }
                } else {
                    "SELECT A SQUIRE FIRST"
                }
                .into(),
                'h',
            ));
        }
    } else {
        let station = company::UPGRADE_PLATES
            .iter()
            .position(|&p| inside(p, hero.x, hero.y))?;
        let (name, level, benefit) = [
            (
                "RECRUIT BOARD",
                company.board,
                "MORE OFFERS / VETERAN RECRUITS",
            ),
            (
                "COMPANY HALL",
                company.hall,
                "MORE ROSTER ROOM / LOWER WAGES",
            ),
            (
                "TRAINING YARD",
                company.yard,
                "HIGHER CAP / LESS TRAINING REST",
            ),
        ][station];
        lines = vec![
            (format!("{} / {}", short_name(&hero.name, 12), name), 'i'),
            (format!("STATION LEVEL {} / 3", level), 'H'),
            (benefit.into(), 'h'),
        ];
        if let Some(price) = company.upgrade_price(station) {
            lines.push((format!("E UPGRADE / {} GOLD", price), 'O'));
            lines.push((format!("NEXT STATION LEVEL {}", level + 1), 'h'));
        } else {
            lines.push(("E UPGRADE / MAX LEVEL".into(), 'i'));
        }
        lines.push((
            format!(
                "ROSTER CAP {} / LEVEL CAP {}",
                company.roster_cap(),
                company.level_cap()
            ),
            'h',
        ));
        lines.push((format!("TREASURY {} GOLD", run.treasury.get(crate::realm::Spoil::Gold)), 'h'));
    }
    Some(lines)
}

/// Black-paper scroll with pixel-cut corners. Right of the lesson stalls,
/// only while this screen's owner stands on a company plate.
pub(super) fn boards(cv: &mut Img, run: &Run, focus: Option<u32>) {
    if run.room().kind != RoomKind::SquireBarracks {
        return;
    }
    yard_notices(cv, run, focus);
    let Some(lines) = notice(run, focus) else {
        return;
    };
    let (x, y, w, h) = (176, 96, 200, lines.len() as i32 * 9 + 10);
    cv.rect(x + 2, y, w - 4, h, 'k');
    cv.rect(x, y + 2, w, h - 4, 'k');
    cv.frame(x + 1, y + 1, w - 2, h - 2, 'G');
    cv.rect(x + 3, y + 3, w - 6, 1, 'r');
    cv.rect(x + 3, y + h - 4, w - 6, 1, 'r');
    for (row, (line, ink)) in lines.iter().enumerate() {
        debug_assert!(text_width(line) <= w - 14, "company line too wide: {line}");
        cv.text(x + 7, y + 6 + row as i32 * 9, line, *ink);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KINDS: [Kind; 5] = [
        Kind::Ninja,
        Kind::Healer,
        Kind::Ranger,
        Kind::Shield,
        Kind::Alchemist,
    ];

    #[test]
    fn squire_figures_are_small_distinct_and_owner_coloured() {
        let mut hashes = std::collections::BTreeSet::new();
        for kind in KINDS {
            let im = figure(kind, &[], false);
            assert!(
                im.w >= 9 && im.w <= 15 && im.h >= 11 && im.h <= 14,
                "{kind:?} must stay smaller than a knight: {}x{}",
                im.w,
                im.h
            );
            assert!(
                hashes.insert(im.content_hash()),
                "each job needs its own silhouette"
            );
            assert_ne!(
                im,
                figure(kind, &[('7', 'A'), ('8', 'l')], false),
                "{kind:?} must carry its owner's cloth"
            );
            let down = figure(kind, &[], true);
            assert!(down.w > down.h, "a downed {kind:?} lies on the floor");
        }
    }

    fn yard() -> Run {
        let mut run = Run::at_home(61, 1, Some("Ada"), Default::default(), Default::default());
        run.join_seat(2, "Bryn");
        run.players.get_mut(&1).unwrap().name = "Ada".into();
        run.players.get_mut(&2).unwrap().name = "Bryn".into();
        let room = run
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == crate::rules::RoomKind::SquireBarracks)
            .unwrap();
        run.enter_for_test(room);
        run
    }

    fn on_plate(run: &mut Run, owner: u32, rect: Plate) {
        let h = run.players.get_mut(&owner).unwrap();
        h.x = (rect.0 as f32 + rect.2 as f32 / 2.0) * crate::rules::TILE_UNITS;
        h.y = (rect.1 as f32 + rect.3 as f32 / 2.0) * crate::rules::TILE_UNITS;
    }

    #[test]
    fn squire_yard_panels_are_focused_and_leave_lessons_alone() {
        let mut run = yard();
        on_plate(&mut run, 1, company::SELECT_PLATE);
        on_plate(&mut run, 2, company::TRAIN_PLATE);
        let a = notice(&run, Some(1)).unwrap();
        let b = notice(&run, Some(2)).unwrap();
        assert!(a.iter().any(|(s, _)| s.contains("ADA")));
        assert!(!a.iter().any(|(s, _)| s.contains("BRYN")));
        assert!(b.iter().any(|(s, _)| s.contains("BRYN")));
        on_plate(&mut run, 1, crate::rules::talents::LECTERNS[0]);
        assert!(
            notice(&run, Some(1)).is_none(),
            "Ector keeps his own notices"
        );
        let mut cv = Img::new(384, 224);
        yard_ground(&mut cv, &run);
        for y in 10 * super::super::TILE..224 {
            for x in 0..160 {
                assert!(
                    cv.get(x, y).is_none(),
                    "company art must leave lesson furniture alone"
                );
            }
        }
    }

    #[test]
    fn squire_yard_prompts_and_prices_are_from_the_company() {
        let mut run = yard();
        for (plate, prompt) in [
            (company::RECRUIT_PLATES[0], "E HIRE"),
            (company::SELECT_PLATE, "E SELECT"),
            (company::TRAIN_PLATE, "E TRAIN"),
            (company::UPGRADE_PLATES[0], "E UPGRADE"),
        ] {
            on_plate(&mut run, 1, plate);
            let lines = notice(&run, Some(1)).unwrap();
            assert!(
                lines.iter().any(|(s, _)| s.contains(prompt)),
                "missing {prompt}"
            );
            if prompt == "E HIRE" {
                let price = run.home.squire_company.hire_price(0).unwrap().to_string();
                assert!(lines.iter().any(|(s, _)| s.contains(&price)));
            }
        }
    }

    fn assert_text(cv: &Img, x: i32, y: i32, label: &str, ink: char) {
        let mut expected = Img::new(super::super::super::ink::text_width(label), 7);
        expected.text(0, 0, label, ink);
        for py in 0..7 {
            for px in 0..expected.w {
                if let Some(rgb) = expected.get(px, py) {
                    assert_eq!(
                        cv.get(x + px, y + py),
                        Some(rgb),
                        "clipped/occluded {label:?} at {px},{py}"
                    );
                }
            }
        }
    }

    #[test]
    fn squire_recruit_card_has_separate_complete_level_and_price_rows() {
        let mut run = yard();
        // Includes the reported SHIELD L1 / 110G, veterans and hall discounts.
        for board in 0..=3 {
            for hall in 0..=3 {
                run.home.squire_company.board = board;
                run.home.squire_company.hall = hall;
                for (slot, offer) in run.home.squire_company.offers().iter().enumerate() {
                    let p = company::RECRUIT_PLATES[slot];
                    on_plate(&mut run, 1, p);
                    let mut cv = Img::new(384, 224);
                    yard_ground(&mut cv, &run);
                    boards(&mut cv, &run, Some(1));
                    let (x, y) = (p.0 * 16, p.1 * 16);
                    let price = format!("{}G", run.home.squire_company.hire_price(slot).unwrap());
                    assert!(super::super::super::ink::text_width(&price) <= 40);
                    assert_text(&cv, x + 22, y - 26, &format!("L{}", offer.level), 'H');
                    assert_text(
                        &cv,
                        x + (48 - super::super::super::ink::text_width(&price)) / 2,
                        y - 11,
                        &price,
                        'O',
                    );
                    let lines = notice(&run, Some(1)).unwrap();
                    assert!(lines[0].0.ends_with(&format!("RECRUIT {}", slot + 1)));
                    for (row, (line, ink)) in lines.iter().enumerate() {
                        assert!(text_width(line) <= 186, "{line}");
                        assert_text(&cv, 183, 102 + row as i32 * 9, line, *ink);
                    }
                    // Exactly this screen's applicant has the warm arrow;
                    // another player's plate cannot steal the focus marker.
                    for (other, p) in company::RECRUIT_PLATES.iter().enumerate() {
                        assert_eq!(
                            cv.get(p.0 * 16 + 24, p.1 * 16 - 37),
                            if other == slot {
                                super::super::super::ink::ink('O')
                            } else {
                                None
                            }
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn squire_prompts_and_station_prices_are_boards_not_actor_occluded_ground() {
        let mut run = yard();
        for p in company::RECRUIT_PLATES
            .into_iter()
            .chain([company::SELECT_PLATE, company::TRAIN_PLATE])
            .chain(company::UPGRADE_PLATES)
        {
            on_plate(&mut run, 1, p);
            // Actual room paint order, including ambient folk and the knight.
            let mut cv = Img::new(384, 224);
            super::super::room_paint::room_list(&run, Some(1), true).paint(&mut cv);
            let label = if company::RECRUIT_PLATES.contains(&p) {
                if run.home.squire_company.offers().len()
                    > company::RECRUIT_PLATES
                        .iter()
                        .position(|r| *r == p)
                        .unwrap()
                {
                    "E HIRE"
                } else {
                    "LOCK"
                }
            } else if p == company::SELECT_PLATE {
                "PICK"
            } else if p == company::TRAIN_PLATE {
                // The plate holds four letters (`SPAR`, the art pass of
                // 2026-11-02); the station's long name rides its paper.
                "SPAR"
            } else {
                "UP"
            };
            let width = p.2 * 16;
            assert_text(
                &cv,
                p.0 * 16 + (width - super::super::super::ink::text_width(label)) / 2,
                p.1 * 16 + 5,
                label,
                if label == "LOCK" { 'i' } else { 'H' },
            );
            for (station, p) in company::UPGRADE_PLATES.iter().enumerate() {
                let price = format!("{}G", run.home.squire_company.upgrade_price(station).unwrap());
                assert_text(&cv, p.0 * 16, 120, &price, 'O');
            }
        }
    }

    #[test]
    fn squire_foot_tabs_survive_heroes_without_recolouring_their_cloth() {
        let mut run = six_companions();
        for (&owner, companion) in &run.squire_field.companions {
            let hero = run.players.get_mut(&owner).unwrap();
            hero.x = companion.actor.x;
            hero.y = companion.actor.y + 0.25;
        }
        let mut cv = Img::new(384, 224);
        super::super::room_paint::room_list(&run, Some(1), true).paint(&mut cv);
        for (&owner, companion) in &run.squire_field.companions {
            let (x, y) = at(companion.actor.x, companion.actor.y);
            let tab = Img::from_rows(&["777"]).recolor(colours(&run, owner));
            for dx in -1..=1 {
                assert_eq!(
                    cv.get(x + dx, y + 2),
                    tab.get(dx + 1, 0),
                    "owner {owner}'s three-pixel tab must survive the knight"
                );
            }
        }
    }

    fn six_companions() -> Run {
        use crate::rules::{
            hireling::{AllyKind, Hireling},
            squire_company::{Companion, Squire},
        };
        let mut run = super::super::testing::fight();
        for owner in 2..=6 {
            run.join_seat(owner, &format!("Knight {owner}"));
        }
        for owner in 1..=6 {
            let hero = run.players.get_mut(&owner).unwrap();
            hero.x = 10.0 + (owner - 1) as f32 * 4.0;
            hero.y = 18.0;
            run.squire_field.companions.insert(
                owner,
                Companion {
                    squire: Squire {
                        id: owner,
                        name: format!("Squire {owner}"),
                        kind: KINDS[(owner as usize - 1) % KINDS.len()],
                        level: 2,
                        trained_on: None,
                    },
                    actor: Hireling {
                        kind: AllyKind::Beaumains,
                        x: hero.x + 1.0,
                        y: hero.y - 3.0,
                        hp: 100,
                        max_hp: 100,
                        invulnerable: 0,
                        cooldown: 0,
                        facing: 1.0,
                        side: 1.0,
                        summoner: Some(owner),
                        power: 1,
                        taunt: 0,
                        rising: 0,
                    },
                    ability: 0,
                    effect: 0,
                    mark: None,
                },
            );
        }
        run
    }

    #[test]
    fn squire_delve_renders_six_owners_and_a_low_downed_body() {
        let mut run = six_companions();
        let mut before = Img::new(384, 224);
        delve(&mut before, &run);
        for (&owner, companion) in &run.squire_field.companions {
            let (x, y) = at(companion.actor.x, companion.actor.y);
            let expected = figure(companion.squire.kind, knights::realm_colours(owner), false);
            for sy in 0..expected.h {
                for sx in 0..expected.w {
                    if let Some(rgb) = expected.get(sx, sy) {
                        assert_eq!(
                            before.get(x - expected.w / 2 + sx, y - expected.h + 1 + sy),
                            Some(rgb),
                            "owner {owner}'s pixels stay legible"
                        );
                    }
                }
            }
        }
        run.squire_field.companions.get_mut(&2).unwrap().actor.hp = 0;
        let mut after = Img::new(384, 224);
        delve(&mut after, &run);
        assert_ne!(before, after);
        let actor = &run.squire_field.companions[&2].actor;
        let (x, y) = at(actor.x, actor.y);
        for py in y - 13..y - 4 {
            for px in x - 7..x + 7 {
                assert!(after.get(px, py).is_none());
            }
        }
        assert_eq!(run.hireling, None, "squires never borrow the tavern actor");
        let with = super::super::frame_for(&run, 384, 224, Some(1));
        let mut without = run.clone();
        without.squire_field.companions.clear();
        assert_ne!(
            with,
            super::super::frame_for(&without, 384, 224, Some(1)),
            "the real dungeon figure hook must draw squires"
        );
        let mut again = Img::new(384, 224);
        delve(&mut again, &run);
        assert_eq!(after, again, "rendering has no gameplay side effects");
    }

    #[test]
    fn squire_board_ink_is_grey_and_warm_and_leaves_actual_lessons_on_top() {
        let mut run = yard();
        let mut paper = Img::new(384, 224);
        on_plate(&mut run, 1, company::RECRUIT_PLATES[0]);
        boards(&mut paper, &run, Some(1));
        for y in 0..paper.h {
            for x in 0..paper.w {
                if let Some(c) = paper.get(x, y) {
                    assert!(c[2] <= c[0], "no blue/glow in company paper: {c:?}");
                }
            }
        }
        // The lessons are the Yard's; the company has the Barracks
        // (`squire_rooms`): in the Yard nothing of the company covers them.
        let yard = run.dungeon.rooms.iter().position(|r| r.kind == crate::rules::RoomKind::Yard).unwrap();
        run.enter_for_test(yard);
        on_plate(&mut run, 1, crate::rules::talents::LECTERNS[0]);
        on_plate(&mut run, 2, company::RECRUIT_PLATES[0]);
        let mut expected = Img::new(384, 224);
        super::super::lessons::boards(&mut expected, &run, Some(1));
        assert!(expected.words().iter().any(|&p| p != 0));
        let mut actual = Img::new(384, 224);
        super::super::room_paint::room_list(&run, Some(1), true).paint(&mut actual);
        for y in 0..224 {
            for x in 0..384 {
                if let Some(c) = expected.get(x, y) {
                    assert_eq!(
                        actual.get(x, y),
                        Some(c),
                        "lesson must survive company paper at {x},{y}"
                    );
                }
            }
        }
    }

    #[test]
    fn squire_boards_and_foot_tabs_compose_in_the_window_overlay() {
        let mut yard = yard();
        on_plate(&mut yard, 1, company::RECRUIT_PLATES[0]);
        for run in [yard, six_companions()] {
            let parts = super::super::room_scene::scene_for(&run, 384, 224, Some(1));
            let frame = super::super::frame_for(&run, 384, 224, Some(1));
            assert!(
                parts.compose().words() == frame.words(),
                "flat and window overlays must agree"
            );
        }
    }

    #[test]
    fn squire_yard_max_roster_panels_fit_inside_the_room() {
        use crate::rules::squire_company::{Roster, Squire};
        let mut run = yard();
        run.home.squire_company.hall = 3;
        run.players.get_mut(&1).unwrap().name = "ABCDEFGHIJKLMNOPQRSTUVWXYZ123456".into();
        run.home.squire_company.rosters.insert(
            run.players[&1].name.clone(),
            Roster {
                squires: KINDS
                    .into_iter()
                    .enumerate()
                    .map(|(i, kind)| Squire {
                        id: i as u32 + 1,
                        name: format!("Recruit{i}"),
                        kind,
                        level: 3,
                        trained_on: None,
                    })
                    .collect(),
                active: Some(1),
                claimed: vec![],
            },
        );
        for p in [company::SELECT_PLATE, company::TRAIN_PLATE] {
            on_plate(&mut run, 1, p);
            let lines = notice(&run, Some(1)).unwrap();
            assert!(96 + lines.len() as i32 * 9 + 10 <= 224);
            assert_eq!(lines.iter().filter(|(s, _)| s.starts_with("> ")).count(), 1);
            let mut cv = Img::new(384, 224);
            boards(&mut cv, &run, Some(1));
            assert!(cv.get(178, 98).is_some());
            for (row, (line, ink)) in lines.iter().enumerate() {
                assert!(text_width(line) <= 186, "{line}");
                assert_text(&cv, 183, 102 + row as i32 * 9, line, *ink);
            }
        }
    }

    /// Generated six-seat fights using the paired LOOKs' seed/room/bot intent;
    /// no synthetic arena, forced separation, HP overrides or removed foes.
    fn fight_at_tick60(seed: u64, floor: u32) -> Run {
        use crate::rules::{
            knights as kits,
            squire_company::{Roster, Squire},
        };
        let mut run = Run::new(seed, 700 + seed, None);
        // Fresh characters, not a saved realm's gear or training bonuses.
        for id in 2..=6 {
            run.join_seat(id, &format!("K{id}"));
        }
        for id in 1..=6 {
            let hero = run.players.get_mut(&id).unwrap();
            hero.name = format!("K{id}");
            hero.x = 24.0;
            hero.y = 20.0;
            run.outfit(
                id,
                kits::knight(["percival", "lynette", "gareth", "galahad"][(id - 1) as usize % 4])
                    .unwrap(),
            );
        }
        while run.dungeon.depth < floor {
            let before = run.dungeon.depth;
            run.descend_for_test();
            assert!(run.dungeon.depth > before);
        }
        assert!(run.tune.set("foe_shots", 50.0));
        assert!(run.tune.set("foe_shot_speed", 1.5));
        let rooms: Vec<_> = run
            .dungeon
            .rooms
            .iter()
            .enumerate()
            .filter(|(_, r)| r.kind == RoomKind::Fight && !r.cleared)
            .map(|(i, _)| i)
            .collect();
        run.enter(rooms[(seed as usize * 7 + 3) % rooms.len()], Some(2));
        for (id, kind) in (1..=6).zip(KINDS.into_iter().chain([Kind::Shield])) {
            run.home.squire_company.rosters.insert(
                run.players[&id].name.clone(),
                Roster {
                    squires: vec![Squire {
                        id,
                        name: format!("S{id}"),
                        kind,
                        level: 3,
                        trained_on: None,
                    }],
                    active: Some(id),
                    ..Default::default()
                },
            );
        }
        run.deploy_company_squires();
        for _ in 0..60 {
            let inputs = run
                .players
                .keys()
                .map(|&id| (id, crate::rules::balance_sim_tests::bot_input(&run, id)))
                .collect();
            run.step(&inputs);
        }
        assert_eq!(run.phase, crate::rules::Phase::Fighting);
        assert!(run.enemies.iter().any(|e| e.hp > 0));
        run
    }

    #[test]
    #[ignore = "writes native and 3x PNG sheets and focused Yard frames; no host"]
    fn write_squire_pixel_shots() {
        let dir = std::env::var_os("DELVE_SQUIRE_SHOTS")
            .map(std::path::PathBuf::from)
            .expect("set DELVE_SQUIRE_SHOTS to the output directory");
        std::fs::create_dir_all(&dir).unwrap();
        let save = |name: &str, im: &Img| {
            image::save_buffer(
                dir.join(format!("{name}.png")),
                &im.rgba_bytes(),
                im.w as u32,
                im.h as u32,
                image::ColorType::Rgba8,
            )
            .unwrap();
        };
        let mut sheet = Img::black(384, 128);
        for owner in 1..=6 {
            for (slot, kind) in KINDS.into_iter().enumerate() {
                let x = 34 + slot as i32 * 72;
                let y = 18 + (owner as i32 - 1) * 18;
                let im = figure(kind, knights::realm_colours(owner), false);
                sheet.stamp(&im, x, y - im.h);
                let tab = Img::from_rows(&["KKKKK", "K777K", "KKhKK"])
                    .recolor(knights::realm_colours(owner));
                sheet.stamp(&tab, x + im.w / 2 - 2, y);
                let im = figure(kind, knights::realm_colours(owner), true);
                sheet.stamp(&im, x + 20, y - im.h);
            }
        }
        save("squire-silhouettes-native", &sheet);
        let mut zoom = Img::black(sheet.w * 3, sheet.h * 3);
        zoom.stamp_times(&sheet, 0, 0, 3);
        save("squire-silhouettes-3x", &zoom);
        let mut run = yard();
        for (name, p) in [
            ("recruit", company::RECRUIT_PLATES[0]),
            ("select", company::SELECT_PLATE),
            ("train", company::TRAIN_PLATE),
            ("upgrade", company::UPGRADE_PLATES[0]),
            ("lesson", crate::rules::talents::LECTERNS[0]),
        ] {
            on_plate(&mut run, 1, p);
            save(
                &format!("yard-{name}"),
                &super::super::frame_for(&run, 768, 448, Some(1)),
            );
        }
        run.home.squire_company.board = 3;
        run.home.squire_company.hall = 3;
        run.home.squire_company.yard = 3;
        run.home.squire_company.rosters.insert(
            "Ada".into(),
            crate::rules::squire_company::Roster {
                squires: KINDS
                    .into_iter()
                    .enumerate()
                    .map(|(i, kind)| crate::rules::squire_company::Squire {
                        id: i as u32 + 1,
                        name: ["Ash", "Bramble", "Clover", "Dew", "Ember"][i].into(),
                        kind,
                        level: i as u8 + 1,
                        trained_on: None,
                    })
                    .collect(),
                active: Some(2),
                claimed: vec![],
            },
        );
        on_plate(&mut run, 1, company::SELECT_PLATE);
        save(
            "yard-five-focused-roster",
            &super::super::frame_for(&run, 768, 448, Some(1)),
        );
        on_plate(&mut run, 1, company::TRAIN_PLATE);
        save(
            "yard-train-owned",
            &super::super::frame_for(&run, 768, 448, Some(1)),
        );
        run.home.squire_company.hall = 0;
        for slot in 0..5 {
            on_plate(&mut run, 1, company::RECRUIT_PLATES[slot]);
            save(
                &format!("yard-recruit-{}", slot + 1),
                &super::super::frame_for(&run, 768, 448, Some(1)),
            );
        }
        for floor in 3..=6 {
            for seed in [11, 29, 47] {
                let run = fight_at_tick60(seed, floor);
                save(
                    &format!("squire-art-floor{floor}-seed{seed}-tick60-focus1"),
                    &super::super::frame_for(&run, 768, 448, Some(1)),
                );
            }
        }
        let run = six_companions();
        save(
            "six-knights-six-squires",
            &super::super::frame_for(&run, 768, 448, Some(1)),
        );
        eprintln!("squire PNGs: {}", dir.display());
    }

    #[test]
    fn squire_figures_use_only_grey_warm_and_owner_inks_without_a_glow() {
        for kind in KINDS {
            let a = figure(kind, &[], false);
            assert_eq!(a, figure(kind, &[], false));
            for y in 0..a.h {
                for x in 0..a.w {
                    if let Some(rgb) = a.get(x, y) {
                        assert!(rgb[2] <= rgb[0], "{kind:?}: no blue decoration: {rgb:?}");
                    }
                }
            }
        }
    }
    #[test]
    fn squire_recruit_notice_keeps_complete_name_and_treasury_visible() {
        let mut run = yard();
        on_plate(&mut run, 1, company::RECRUIT_PLATES[0]);
        let offer = run.home.squire_company.offers()[0].clone();
        let lines = notice(&run, Some(1)).unwrap();
        assert!(lines.iter().any(|(line, _)| line.contains(&offer.name.to_uppercase()) && text_width(line) <= 186));
        let gold = run.treasury.get(crate::realm::Spoil::Gold);
        assert!(lines.iter().any(|(line, _)| line == &format!("TREASURY {} GOLD", gold)));
    }

}
