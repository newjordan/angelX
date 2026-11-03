use super::*;

fn inks(rows: &[String]) -> Vec<char> {
    let mut seen: Vec<char> = rows.concat().chars().filter(|&c| c != '.' && c != '|').collect();
    seen.sort();
    seen.dedup();
    seen
}

/// A shaded orb: an outline, three stone blues and two highlights.
const ORB: [&str; 8] = [
    "..kkkk..",
    ".kqQQqk.",
    "kqQzzQqk",
    "kQzWWzQk",
    "kQzWVzQk",
    "kqQzzQqk",
    ".kqQQqk.",
    "..kkkk..",
];

#[test]
fn a_ladder_climbs_coarse_then_plain_then_as_drawn() {
    use Step::*;
    let steps = |n| (0..n).map(|t| Step::of(t, n)).collect::<Vec<_>>();
    assert_eq!(steps(3), [Coarse, Plain, Drawn]);
    assert_eq!(steps(5), [Coarse, Coarse, Plain, Plain, Drawn]);
    assert_eq!(steps(2), [Coarse, Drawn]);
    assert_eq!(steps(1), [Drawn]);
}

#[test]
fn the_last_level_is_the_drawing_itself() {
    assert_eq!(rows(&ORB, Step::Drawn), ORB.map(String::from));
}

#[test]
fn a_coarse_level_is_two_by_two_in_three_inks() {
    let coarse = rows(&ORB, Step::Coarse);
    assert_eq!(coarse.len(), ORB.len());
    assert!(coarse.iter().zip(ORB).all(|(a, b)| a.len() == b.len()), "the same size");
    assert!(inks(&coarse).len() <= 3, "{coarse:?}");
    for y in (0..8).step_by(2) {
        for x in (0..8).step_by(2) {
            let c = coarse[y].as_bytes()[x];
            for (dy, dx) in [(0, 1), (1, 0), (1, 1)] {
                assert_eq!(coarse[y + dy].as_bytes()[x + dx], c, "square at {x},{y}");
            }
        }
    }
    assert!(coarse.concat().contains('k'), "the outline holds");
}

#[test]
fn a_plain_level_keeps_the_grain_and_four_inks() {
    let plain = rows(&ORB, Step::Plain);
    assert!(inks(&plain).len() <= 4);
    assert!(inks(&plain).len() > inks(&rows(&ORB, Step::Coarse)).len());
    // The commonest inks stay where they were drawn.
    for (a, b) in plain.iter().zip(ORB) {
        for (p, d) in a.chars().zip(b.chars()) {
            if matches!(d, 'k' | 'Q' | 'q' | 'z' | '.') {
                assert_eq!(p, d);
            }
        }
    }
}

#[test]
fn an_ink_let_go_turns_to_the_kept_ink_nearest_its_colour() {
    // A light blue beside a grey and a deep blue: it goes to the blue.
    assert_eq!(nearest('z', &['j', 'q']), 'q');
    assert_eq!(nearest('W', &['k', 'V']), 'V');
}

#[test]
fn frames_side_by_side_are_each_their_own_picture() {
    let two = ["kk|zz", "kk|zz"];
    assert_eq!(rows(&two, Step::Coarse), ["kk|zz", "kk|zz"]);
}

#[test]
fn a_block_keeps_its_header_and_changes_only_its_rows() {
    let text = format!("draw shot at=body\n{}\n", ORB.join("\n"));
    let coarse = block(&text, Step::Coarse);
    assert!(coarse.starts_with("draw shot at=body\n"));
    assert!(coarse.ends_with('\n'));
    assert_eq!(coarse.lines().count(), text.lines().count());
    assert_eq!(block(&text, Step::Drawn), text);
    assert_eq!(block("look red bolt\n", Step::Coarse), "look red bolt\n");
    // `art` is spelt in inks: still the header.
    assert!(block(&format!("art\n{}", ORB.join("\n")), Step::Coarse).starts_with("art\n"));
}

#[test]
fn a_wish_is_coarse_at_its_first_grant_and_whole_at_its_last() {
    let entry = format!(
        "wish orb | Orb\nsay a blue orb\ntier homing 1 :: An orb.\ntier homing 2 :: A brighter orb.\ntier homing 2; velocity 20 :: The orb.\ndraw shot\n{rows}\n\nart\n{rows}\n",
        rows = ORB.join("\n")
    );
    let (book, errors) = super::super::phrasebook::read_checked(&entry);
    assert_eq!(book.len(), 1, "every tier checks: {errors:?}");
    let cards: Vec<_> = (0..3).map(|t| book[0].card(t).unwrap()).collect();
    let art = |t: usize| -> Vec<String> { cards[t].art_rows().iter().map(|r| r.to_string()).collect() };
    let shot = |t: usize| cards[t].looks.shot.as_ref().unwrap().frames[0].clone();
    assert_eq!(art(2), ORB.map(String::from), "the full gift, as drawn");
    assert_eq!(shot(2), ORB.map(String::from));
    assert!(inks(&art(0)).len() <= 3 && inks(&shot(0)).len() <= 3);
    assert!(inks(&art(0)).len() < inks(&art(1)).len());
    assert!(inks(&art(1)).len() < inks(&art(2)).len());
}

const SHA: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

#[test]
fn only_the_drawing_as_drawn_keeps_its_painting() {
    let text = format!("draw shot paint={SHA} move=stretch,bob\n{}\n", ORB.join("\n"));
    let coarse = block(&text, Step::Coarse);
    assert!(coarse.starts_with("draw shot move=stretch,bob\n"), "{coarse}");
    assert_eq!(coarse.lines().count(), text.lines().count());
    // Plain, its painting is kept for a window deep enough down.
    let plain = block(&text, Step::Plain);
    assert!(plain.starts_with(&format!("draw shot hd={SHA} move=stretch,bob\n")), "{plain}");
    assert_eq!(plain.lines().count(), text.lines().count());
    assert_eq!(block(&plain, Step::Coarse).lines().next(), Some("draw shot move=stretch,bob"));
    assert_eq!(block(&text, Step::Drawn), text);
    // A pixel-art painting's word goes with its `paint=` (kept plain as `hd=`).
    let pixel = text.replacen(" move=", " pixel move=", 1);
    assert!(block(&pixel, Step::Plain).starts_with(&format!("draw shot hd={SHA} move=stretch,bob\n")));
    assert!(block(&pixel, Step::Coarse).starts_with("draw shot move=stretch,bob\n"));
    assert_eq!(block(&pixel, Step::Drawn), pixel);
    // A header with nothing painted is kept to the letter.
    let plain = format!("draw shot  at=body\n{}\n", ORB.join("\n"));
    assert!(block(&plain, Step::Coarse).starts_with("draw shot  at=body\n"));
}

#[test]
fn a_painted_wish_shows_its_painting_at_its_last_grant_and_moves_at_every_one() {
    use super::super::looks::Move;
    let entry = format!(
        "wish orb | Orb\nsay a blue orb\ntier homing 1 :: An orb.\ntier homing 2 :: A brighter orb.\ntier homing 2; velocity 20 :: The orb.\ndraw shot paint={SHA} move=wiggle,pulse\n{rows}\n",
        rows = ORB.join("\n")
    );
    let (book, errors) = super::super::phrasebook::read_checked(&entry);
    assert_eq!(book.len(), 1, "every tier checks: {errors:?}");
    let shot = |t: usize| book[0].card(t).unwrap().looks.shot.unwrap();
    for t in 0..2 {
        assert_eq!(shot(t).paint, None, "tier {t} shows inks");
        assert_eq!(shot(t).moves, [Move::Wiggle, Move::Pulse]);
    }
    // Its middle grant keeps the painting for a deep floor; its first does not.
    assert_eq!(shot(0).hd, None);
    assert_eq!(shot(1).hd.as_deref(), Some(SHA));
    assert_eq!(shot(2).hd, None);
    assert_eq!(shot(2).paint.as_deref(), Some(SHA));
    assert_eq!(shot(2).moves, [Move::Wiggle, Move::Pulse]);
    // Pixel art says so at the last grant only, and every tier checks.
    let pixel = entry.replacen(" move=", " pixel move=", 1);
    let (book, errors) = super::super::phrasebook::read_checked(&pixel);
    assert_eq!(book.len(), 1, "every tier checks: {errors:?}");
    let shot = |t: usize| book[0].card(t).unwrap().looks.shot.unwrap();
    assert!(!shot(1).pixel && shot(1).paint.is_none());
    assert!(shot(2).pixel && shot(2).paint.as_deref() == Some(SHA));
    // A one-tier wish is always drawn whole.
    let one = entry.replace("tier homing 1 :: An orb.\ntier homing 2 :: A brighter orb.\n", "");
    let (book, _) = super::super::phrasebook::read_checked(&one);
    assert_eq!(book[0].card(0).unwrap().looks.shot.unwrap().paint.as_deref(), Some(SHA));
}

#[test]
fn a_wish_painted_at_its_top_is_that_painting_downgraded_at_every_grant() {
    // Drawn by the learner at the first tier, painted (the painter's cut)
    // at the last: every grant shows the painting's inks, crisper each one.
    let learner = [".1.", "111", ".1."];
    let entry = format!(
        "wish orb | Orb\nsay a blue orb\ntier homing 1 :: An orb.\ntier homing 2 :: A brighter orb.\ntier homing 2; velocity 20 :: The orb.\ndraw shot\n{learner}\ndraw shot 3 paint={SHA}\n{rows}\n",
        learner = learner.join("\n"),
        rows = ORB.join("\n")
    );
    let (book, errors) = super::super::phrasebook::read_checked(&entry);
    assert_eq!(book.len(), 1, "every tier checks: {errors:?}");
    let shot = |t: usize| book[0].card(t).unwrap().looks.shot.unwrap();
    assert_eq!(shot(2).frames[0], ORB.map(String::from), "the whole gift, as painted");
    assert_eq!(shot(2).paint.as_deref(), Some(SHA));
    for t in 0..2 {
        assert_ne!(shot(t).frames[0], learner.map(String::from), "tier {t}: the painting's inks, not the learner's");
        assert_eq!(shot(t).frames[0].len(), ORB.len());
    }
    assert!(inks(&shot(0).frames[0]).len() < inks(&shot(1).frames[0]).len());
    assert_eq!((shot(0).hd.as_deref(), shot(1).hd.as_deref()), (None, Some(SHA)));
}

#[test]
fn a_look_drawn_for_a_tier_of_its_own_is_shown_as_drawn_there() {
    // The learner's look, coarsened by level, and a look per level named
    // for its tier (`draw shot 1`), shown as drawn at every level.
    let entry = format!(
        "wish orb | Orb\nsay a blue orb\ntier homing 1 :: An orb.\ntier homing 2 :: A brighter orb.\ntier homing 2; velocity 20 :: The orb.\ndraw shot\n{rows}\ndraw shot 1 paint={SHA} pixel\n{rows}\ndraw shot 3 paint={SHA} pixel\n{rows}\n",
        rows = ORB.join("\n")
    );
    let (book, errors) = super::super::phrasebook::read_checked(&entry);
    assert_eq!(book.len(), 1, "every tier checks: {errors:?}");
    for t in 0..3 {
        let shot = book[0].card(t).unwrap().looks.shot.unwrap();
        assert_eq!(shot.frames[0], ORB.map(String::from), "tier {t} as drawn");
        assert_eq!(shot.paint.as_deref(), Some(SHA), "tier {t} keeps its painting");
        assert!(shot.pixel);
    }
    // The painted looks never move the grade: the learner's drawing does.
    let plain = entry[..entry.find("draw shot 1").unwrap()].to_string();
    let (alone, _) = super::super::phrasebook::read_checked(&plain);
    assert_eq!(book[0].grade(), alone[0].grade());
}

#[test]
fn a_wishs_own_colours_saturate_and_brighten_with_its_power() {
    // The player's hue at every step; saturation and brightness are power's.
    let text = "draw orbit pal=%c020ff,&ffe0ff\n.%.\n%&%\n.%.\n";
    let pal_at = |step| super::super::looks::header_pal(block(text, step).lines().next().unwrap());
    let (coarse, plain, drawn) = (pal_at(Step::Coarse), pal_at(Step::Plain), pal_at(Step::Drawn));
    assert_eq!(drawn[0].1, [0xc0, 0x20, 0xff], "as chosen once drawn whole");
    let sv = |[r, g, b]: [u8; 3]| {
        let (hi, lo) = (f32::from(r.max(g).max(b)), f32::from(r.min(g).min(b)));
        ((hi - lo) / hi, hi / 255.0)
    };
    let ((s0, v0), (s1, v1), (s2, v2)) = (sv(coarse[0].1), sv(plain[0].1), sv(drawn[0].1));
    assert!(s0 <= 0.36 && v0 <= 0.56, "coarse: dull and dim ({s0}, {v0})");
    assert!(s0 < s1 && s1 < s2 && v0 < v1 && v1 < v2, "power climbs: {s0} {s1} {s2} / {v0} {v1} {v2}");
    for (_, [r, g, b]) in [coarse[0], plain[0]] {
        assert!(b > r && r > g, "the hue holds");
    }
    // Its own inks narrow by their colours like any other.
    assert!(block(text, Step::Coarse).lines().skip(1).all(|l| !l.is_empty()));
}
