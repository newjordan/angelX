use super::*;
use crate::rules::feel::testing::{a_foe, bare, fire, frame, quiet, save};

/// An arrow knight 1 looses along +x from `(x, y)`, with these gifts.
fn arrow(x: f32, y: f32, traits: ShotTraits) -> Projectile {
    Projectile {
        x,
        y,
        vx: 30.0,
        vy: 0.0,
        hostile: false,
        kind: Shot::Arrow,
        look: None,
        damage: 10,
        pierce: 0,
        last_hit: None,
        ttl: HZ,
        empowered: false,
        traits: ShotTraits {
            owner: 1,
            arm: true,
            ..traits
        },
    }
}

/// A calm open room with one foe of `kind` at `(x, y)` and one arrow in
/// flight toward it.
fn shot_at(kind: EnemyKind, (x, y): (f32, f32), traits: ShotTraits) -> (Run, u32) {
    let mut run = quiet();
    bare(&mut run);
    let foe = a_foe(&mut run, kind, x, y);
    // A live monster far off keeps the room a fight (dummies alone clear it).
    a_foe(&mut run, EnemyKind::Skeleton, 44.0, 3.0);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (8.0, 4.0);
    run.projectiles.push(arrow(x - 4.0, 14.0, traits));
    (run, foe)
}

fn foe(run: &Run, id: u32) -> &Enemy {
    run.enemies.iter().find(|e| e.id == id).expect("still here")
}

fn step(run: &mut Run, ticks: u32) {
    for _ in 0..ticks {
        run.step(&BTreeMap::new());
    }
}

#[test]
fn the_new_words_are_card_lines_with_caps_and_add_up_on_the_bonus() {
    let raw = "name Big Fire\nkind hold\nsize 9\nburn 2\nboomerang\n";
    let checked = cards::check("big-fire", raw).unwrap();
    assert_eq!(
        checked.card.effects,
        vec![cards::Effect::Size(3), cards::Effect::Burn(2), cards::Effect::Boomerang(1)],
        "size clamps to 3; a bare boomerang is the whole of it"
    );
    let other = cards::check("cold", "name Cold\nkind hold\nchill 2\npoison 3\nvelocity 80\n").unwrap();
    let more = cards::check("shove", "name Shove\nkind hold\nknock 3\nsize 2\n").unwrap();
    let bonus = cards::Bonus::of([&checked.card, &other.card, &more.card]);
    assert_eq!(bonus.size, cards::cap("size"), "the deck's size caps at 3");
    assert_eq!(
        (bonus.burn, bonus.chill, bonus.poison, bonus.knock, bonus.boomerang, bonus.velocity),
        (2, 2, 3, 3, 1, 60)
    );
    let rules = cards::hold_effects();
    for word in ["size 1-3", "knock 1-3", "burn 1-3", "chill 1-3", "poison 1-3", "boomerang 1-1", "velocity 10-60"] {
        assert!(rules.contains(word), "the brief's list offers {word}");
    }
    // The traits a knight's shot takes from that bonus.
    let traits = ShotTraits::of(&bonus);
    assert_eq!((traits.size, traits.burn, traits.chill, traits.poison), (3, 2, 2, 3));
    assert!(traits.boomerang);
}

#[test]
fn a_bigger_shot_strikes_what_a_plain_one_flies_past_and_is_drawn_bigger() {
    let probe = {
        let mut run = quiet();
        let id = a_foe(&mut run, EnemyKind::Dummy, 30.0, 14.0);
        foe(&run, id).radius()
    };
    // Just off the arrow's line: a plain arrow misses, a size 2 one hits.
    let at = (30.0, 14.0 + probe + 0.45);
    let (mut plain, a) = shot_at(EnemyKind::Dummy, at, ShotTraits::default());
    let (mut big, b) = shot_at(EnemyKind::Dummy, at, ShotTraits { size: 2, ..Default::default() });
    step(&mut plain, 12);
    step(&mut big, 12);
    assert_eq!(foe(&plain, a).hp, 5000, "a plain arrow flies past");
    assert!(foe(&big, b).hp < 5000, "a bigger arrow strikes it");
    let shot = arrow(0.0, 0.0, ShotTraits { size: 2, ..Default::default() });
    assert_eq!(shot.shot_scale(), 4, "drawn at four times its sprite, not two");
    assert!(shot.wished());
}

#[test]
fn a_knock_wish_shoves_a_monster_further_than_the_plain_arrow() {
    let at = (30.0, 14.0);
    let (mut plain, a) = shot_at(EnemyKind::Skeleton, at, ShotTraits::default());
    let (mut hard, b) = shot_at(EnemyKind::Skeleton, at, ShotTraits { knock: 2, ..Default::default() });
    step(&mut plain, 8);
    step(&mut hard, 8);
    let (pa, pb) = (foe(&plain, a), foe(&hard, b));
    assert!(pa.hp < 5000 && pb.hp < 5000, "both hit");
    assert!(
        pb.x - pa.x > 2.0 * KNOCK_STEP * 0.9,
        "knock 2 shoves {} further, not {}",
        2.0 * KNOCK_STEP,
        pb.x - pa.x
    );
}

#[test]
fn burn_chill_and_poison_hits_brand_the_monster_with_its_bane_and_its_slow() {
    let at = (30.0, 14.0);
    // Burn: Fire Arrows' burn, twice as hot at 2.
    let (mut run, id) = shot_at(EnemyKind::Dummy, at, ShotTraits { burn: 2, ..Default::default() });
    step(&mut run, 8);
    let burn = run.works.banes.iter().find(|b| b.foe == id && b.kind == skills::Bane::Burn);
    let base = run.players[&1].scaled(skills::BURN_BASE);
    assert_eq!(burn.map(|b| b.power), Some(base * 2), "it burns, hotter with N");
    // Poison: the gas skill's poison, three times over at 3.
    let (mut run, id) = shot_at(EnemyKind::Dummy, at, ShotTraits { poison: 3, ..Default::default() });
    step(&mut run, 8);
    let poison = run.works.banes.iter().find(|b| b.foe == id && b.kind == skills::Bane::Poison);
    let base = run.players[&1].scaled(skills::POISON_BASE);
    assert_eq!(poison.map(|b| b.power), Some(base * 3), "it is poisoned, stronger with N");
    let hit = foe(&run, id).hp;
    step(&mut run, 2 * HZ);
    assert!(foe(&run, id).hp < hit, "and the poison bites on its beat");
    // Chill: a monster that walks walks slower.
    let (mut cold, a) = shot_at(EnemyKind::Imp, at, ShotTraits { chill: 1, ..Default::default() });
    let (mut warm, b) = shot_at(EnemyKind::Imp, at, ShotTraits::default());
    step(&mut cold, 8);
    step(&mut warm, 8);
    assert!(foe(&cold, a).chilled > 0, "it is chilled");
    assert_eq!(foe(&warm, b).chilled, 0);
    let (from_a, from_b) = ((foe(&cold, a).x, foe(&cold, a).y), (foe(&warm, b).x, foe(&warm, b).y));
    step(&mut cold, HZ / 2);
    step(&mut warm, HZ / 2);
    let went = |run: &Run, id: u32, (x, y): (f32, f32)| (foe(run, id).x - x).hypot(foe(run, id).y - y);
    let (slow, fast) = (went(&cold, a, from_a), went(&warm, b, from_b));
    assert!(fast > 0.5, "an imp runs ({fast})");
    assert!(slow < fast * 0.8, "a chilled imp crawls: {slow} against {fast}");
    // A guardian shrugs the chill.
    let mut boss = foe(&cold, a).clone();
    boss.boss = Some(0);
    boss.chilled = 0;
    chill(&mut boss, &arrow(0.0, 0.0, ShotTraits { chill: 3, ..Default::default() }));
    assert_eq!(boss.chilled, 0);
}

#[test]
fn fired_shots_take_their_wishes_as_they_leave_faster_aflame_and_coming_back() {
    let loose = |set: fn(&mut cards::Bonus)| {
        let mut run = quiet();
        bare(&mut run);
        set(&mut run.players.get_mut(&1).unwrap().bonus);
        for _ in 0..HZ {
            run.step(&BTreeMap::from([(1, fire(1, 0))]));
            if let Some(shot) = run.projectiles.iter().find(|p| !p.hostile) {
                return shot.clone();
            }
        }
        panic!("no shot")
    };
    let plain = loose(|_| {});
    let fast = loose(|b| b.velocity = 50);
    let speed = |p: &Projectile| p.vx.hypot(p.vy);
    assert!((speed(&fast) / speed(&plain) - 1.5).abs() < 0.01, "50% faster");
    // Units it flies before it falls: within a tick's flight of the plain.
    let reach = |p: &Projectile| speed(p) * p.ttl as f32 / HZ as f32;
    let (a, b) = (reach(&fast), reach(&plain));
    assert!((a - b).abs() <= speed(&fast) / HZ as f32, "as far: {a} against {b}");
    let hot = loose(|b| b.burn = 1);
    assert!(hot.traits.fire && hot.traits.burn == 1, "a burning shot is aflame");
    let back = loose(|b| b.boomerang = 1);
    assert_eq!(back.traits.back, 1, "a boomerang flies out to turn");
}

#[test]
fn a_boomerang_flies_out_turns_and_comes_home_and_turns_off_what_it_strikes() {
    let mut run = quiet();
    bare(&mut run);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (10.0, 14.0);
    hero.bonus.boomerang = 1;
    while run.projectiles.iter().all(|p| p.hostile) {
        run.step(&BTreeMap::from([(1, fire(1, 0))]));
    }
    let (mut far, mut turned, mut caught) = (0.0f32, false, false);
    for _ in 0..(4 * HZ) {
        run.step(&BTreeMap::new());
        match run.projectiles.iter().find(|p| !p.hostile && p.traits.boomerang) {
            Some(p) => {
                far = far.max(p.x - 10.0);
                turned |= p.traits.back == 2 && p.vx < 0.0;
            }
            None => {
                caught = true;
                break;
            }
        }
    }
    assert!(far > 4.0, "it flew out ({far})");
    assert!(turned, "it turned for home");
    assert!(caught, "and came back to the hand");
    // Off a monster it turns home instead of falling, and lives.
    let (mut run, id) = shot_at(EnemyKind::Dummy, (30.0, 14.0), ShotTraits { boomerang: true, ..Default::default() });
    run.projectiles[0].traits.back = 1;
    run.projectiles[0].traits.out = 60;
    step(&mut run, 8);
    assert!(foe(&run, id).hp < 5000, "it struck");
    assert!(
        run.projectiles.iter().any(|p| !p.hostile && p.traits.back == 2),
        "and flies home"
    );
}

#[test]
fn a_wish_a_knight_made_is_theirs_its_card_their_notice_and_their_sheet() {
    let text = "wish bigger-arrows | Bigger Arrows\n\
say bigger arrows | huge arrows\n\
tier size 1 :: Bigger arrows.\n\
tier size 2 :: Bigger still.\n";
    let wish = phrasebook::read(text).pop().unwrap();
    let mut run = Run::new(4, 2, Some("Gareth"));
    run.players.get_mut(&1).unwrap().name = "Ada".into();
    run.window = Some(Default::default());
    run.made(1, &wish.id, "");
    assert_eq!(run.grant(1, &wish).unwrap(), "Bigger Arrows I");
    let card = run.book.get("wish-bigger-arrows-1").unwrap();
    assert_eq!(card.by, "Ada", "the card's by line is the player's");
    assert_eq!(run.players[&1].bonus.size, 1);
    assert_eq!(run.found_line().as_deref(), Some("Ada's Bigger Arrows"));
    let told = crate::wire::take_notices(&mut run);
    assert_eq!(told.len(), 1, "one notice");
    assert_eq!(
        told[0],
        crate::wire::Event::Notice { hear: crate::wire::Hear::All, text: "Ada's Bigger Arrows".into() },
        "for everyone"
    );
    assert!(run.notices.is_empty());
    // Another knight wished for it after: still Ada's card.
    run.grant(2, &wish).unwrap();
    assert_eq!(run.book.get("wish-bigger-arrows-1").unwrap().by, "Ada");
    // The sheet: Ada's wishes under their own heading, apart from the stock.
    let shown = crate::client::sheet(&run, 1);
    if std::env::var_os("SHOW_SHEET").is_some() {
        println!("{shown}");
    }
    let (held, rest) = shown.split_once("YOUR WISHES").expect("the heading");
    assert!(held.contains("HELD CARDS"), "it comes after the held cards");
    let (own, stock) = rest.split_once("\nWISHES").expect("then the scroll's");
    assert!(own.contains("Bigger Arrows I") && own.contains("size 1"), "{own}");
    assert!(!stock.contains("Bigger Arrows"), "not in the stock list");
    assert!(stock.contains("Triple Volley"));
    assert!(shown.contains("size 1"), "the totals carry the size");
    // A knight who made none is told how.
    assert!(crate::client::sheet(&run, 2).contains("YOUR WISHES\n  none yet"));
}

#[test]
fn the_wishes_learned_on_2026_10_14_check_clean_and_keep_their_promises() {
    let text = include_str!("../../../../cockpit/assets/dungeon/learned-20261014.txt");
    let (wishes, errors) = phrasebook::read_checked(text);
    assert!(errors.is_empty(), "{errors:?}");
    let ids: Vec<&str> = wishes.iter().map(|w| w.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "bigger-arrows",
            "colossal-arrows",
            "bigger-bullets",
            "healing-light",
            "runeforged-edge",
            "friends-shade",
            "quickfire-crank",
            "regrowth-charm",
            "lightning-rounds"
        ]
    );
    // Below the workspace's line, after the delve's own, as the host reads it.
    let whole = format!("{}\n{}\n{text}", phrasebook::TEXT, phrasebook::LEARNED);
    let (book, errors) = phrasebook::read_checked(&whole);
    assert!(errors.is_empty(), "{errors:?}");
    assert!(book.iter().filter(|w| w.learned).count() >= 9);
    let has = |id: &str, f: fn(&cards::Effect) -> bool| {
        let wish = wishes.iter().find(|w| w.id == id).unwrap();
        (0..wish.tiers.len()).all(|t| wish.card(t).unwrap().effects.iter().any(f))
    };
    for id in ["bigger-arrows", "colossal-arrows", "bigger-bullets"] {
        assert!(has(id, |e| matches!(e, cards::Effect::Size(_))), "{id} is bigger");
    }
    assert!(has("lightning-rounds", |e| matches!(e, cards::Effect::Chain(_))));
    assert!(has("quickfire-crank", |e| matches!(e, cards::Effect::Rate(_))));
    for wish in &wishes {
        for tier in 0..wish.tiers.len() {
            let card = wish.card(tier).unwrap();
            assert!(!card.looks.is_empty(), "{} tier {} shows in play", wish.id, tier + 1);
        }
        // Only damage on its own would be a stat bump.
        let first = wish.card(0).unwrap();
        assert!(
            !first.effects.iter().all(|e| matches!(e, cards::Effect::Damage(_))),
            "{} is more than a damage bump",
            wish.id
        );
    }
    // Every card's look is drawn the way the knight would see it.
    let lightning = wishes.iter().find(|w| w.id == "lightning-rounds").unwrap();
    assert!(lightning.card(0).unwrap().looks.shot.is_some(), "lightning is drawn crackling");
}

/// Frames of a knight firing with each wish, for review:
/// `ANGEL_ARENA_SHOTS=dir cargo test -p angel-delve write_wishcraft_shots -- --ignored`.
#[test]
#[ignore]
fn write_wishcraft_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS").map(std::path::PathBuf::from) else {
        return;
    };
    let dir = dir.join("wishcraft");
    let cases: [(&str, fn(&mut cards::Bonus), u32); 6] = [
        ("plain", |_| {}, 5),
        ("size_2", |b| b.size = 2, 5),
        ("burn", |b| b.burn = 2, 10),
        ("chill", |b| b.chill = 2, 10),
        ("poison", |b| b.poison = 2, 10),
        ("boomerang", |b| b.boomerang = 1, 26),
    ];
    for (name, set, ticks) in cases {
        let mut run = quiet();
        bare(&mut run);
        a_foe(&mut run, EnemyKind::Imp, 32.0, 14.0);
        a_foe(&mut run, EnemyKind::Skeleton, 34.0, 10.0);
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = (16.0, 14.0);
        set(&mut hero.bonus);
        for t in 0..ticks + HZ {
            let fire_now = t % 4 == 0 && t < 18;
            let input = if fire_now { fire(1, 0) } else { Input::default() };
            run.step(&BTreeMap::from([(1, input)]));
            if t + 1 == ticks {
                save(&frame(&run), &dir, name);
            }
        }
        // A second later: what the hits left on the monsters.
        save(&frame(&run), &dir, &format!("{name}_after"));
    }
}


/// Each learned wish of 2026-10-14 at its top tier, a knight firing:
/// `ANGEL_ARENA_SHOTS=dir cargo test -p angel-delve write_learned_shots -- --ignored`.
#[test]
#[ignore]
fn write_learned_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS").map(std::path::PathBuf::from) else {
        return;
    };
    let dir = dir.join("learned");
    let text = include_str!("../../../../cockpit/assets/dungeon/learned-20261014.txt");
    for wish in phrasebook::read(text) {
        let mut run = quiet();
        bare(&mut run);
        a_foe(&mut run, EnemyKind::Imp, 32.0, 14.0);
        a_foe(&mut run, EnemyKind::Skeleton, 34.0, 10.0);
        let card = wish.card(wish.tiers.len() - 1).unwrap();
        run.book.insert(card.clone());
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = (16.0, 14.0);
        hero.deck.push(card.id.clone());
        hero.rebonus(&run.book);
        for t in 0..9 {
            let input = if t % 4 == 0 { fire(1, 0) } else { Input::default() };
            run.step(&BTreeMap::from([(1, input)]));
        }
        save(&frame(&run), &dir, &wish.id);
    }
}
