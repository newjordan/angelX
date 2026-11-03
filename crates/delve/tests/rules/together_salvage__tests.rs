use super::*;
use crate::rules::Run;

/// A run at home with the yard given, its first knight standing.
fn home_run(yard: Salvage) -> Run {
    let mut run = Run::new(7, 1, None);
    run.home.salvage = yard;
    run.dungeon.depth = 0;
    run
}

fn knight(run: &Run) -> (u32, String) {
    let (&id, hero) = run.players.iter().next().expect("a knight");
    (id, crate::rules::talents::knight_key(hero))
}

/// A yard holding one wished piece of every place, at `grade`.
fn full_yard(grade: Grade, by: &str) -> Salvage {
    let mut yard = Salvage::default();
    for place in Place::ALL {
        let id = yard.add(place, grade, by, Pack::Archive);
        yard.pieces.iter_mut().find(|p| p.id == id).unwrap().wish = Some(format!("a purple drank {}", place.word()));
    }
    yard
}

#[test]
fn deeper_floors_and_guardians_give_better_pieces() {
    assert_eq!(grade_of(0, false, 0), Grade::Scrap, "a shallow room's worst is scrap");
    assert_eq!(grade_of(0, false, 99), Grade::Sound, "a Dragon Keep room never gives a relic");
    assert_eq!(grade_of(3, true, 99), Grade::Relic, "the Unknown's guardian can");
    for draw in 0..100 {
        assert!(grade_of(3, true, draw) >= grade_of(0, true, draw), "deeper is never worse");
        assert!(grade_of(1, true, draw) >= grade_of(1, false, draw), "a guardian is never worse");
    }
}

#[test]
fn every_build_needs_the_digging_prow_and_more_pieces_than_the_last() {
    let (places, count) = needs(1);
    assert_eq!(places, Place::CORE.to_vec(), "the first needs the core four");
    assert_eq!(places[0], Place::Prow, "the prow digs");
    assert_eq!(count, 4);
    let mut last = 0;
    for n in 1..40 {
        let (places, count) = needs(n);
        assert!(places.contains(&Place::Prow));
        assert!(count > last, "build {n} is bigger than the one before, ad infinitum");
        assert!(places.len() <= Place::ALL.len());
        last = count;
    }
    assert_eq!(needs(4).0.len(), Place::ALL.len(), "by the fourth, every place");
    assert_eq!(class(1), "bore-skiff");
    assert_eq!(class(6), "leviathan");
    assert_eq!(class(7), "leviathan of the 2nd rank");
    assert_eq!(class(18), "leviathan of the 13th rank");
}

#[test]
fn a_yard_is_only_scrap_until_a_whole_set_is_in() {
    let mut yard = Salvage::default();
    for place in [Place::Hull, Place::Heart, Place::Treads, Place::Crest, Place::Lamps] {
        yard.add(place, Grade::Fine, "You", Pack::Archive);
    }
    assert_eq!(yard.plan(), None, "no prow, nothing digs: not a vessel yet");
    yard.add(Place::Prow, Grade::Scrap, "You", Pack::Hellforge);
    let scrap = yard.add(Place::Prow, Grade::Scrap, "You", Pack::Hellforge);
    let relic = yard.add(Place::Prow, Grade::Relic, "You", Pack::Unknown);
    let plan = yard.plan().expect("the core four are in");
    assert_eq!(plan.n, 1);
    assert_eq!(plan.pieces.len(), 4);
    assert!(plan.pieces.iter().any(|p| p.id == relic), "the best prow goes in");
    assert!(!plan.pieces.iter().any(|p| p.id == scrap));
    for place in Place::CORE {
        assert!(plan.pieces.iter().any(|p| p.place() == place));
    }
}

#[test]
fn the_finder_wishes_the_piece_into_whatever_they_want_without_spending_a_wish() {
    let mut run = home_run(Salvage::default());
    run.dungeon.pack = Pack::Archive;
    run.dungeon.depth = 4;
    let (id, key) = knight(&run);
    let wishes = run.players[&id].wishes;
    let heard = run.cues.len();
    run.cues.push("boss_fall:index".into());
    run.find_salvage(heard);
    assert_eq!(run.home.salvage.pieces.len(), 1, "a guardian of the salvage floors always gives up a piece");
    let piece = run.piece_awaiting(id).expect("the finder's to wish").clone();
    assert_eq!(piece.by, key);
    assert!(piece.wish.is_none());
    let rev = run.home.salvage.rev;
    assert!(run.wish_on_piece(id, "  ").is_err(), "a wish says something");
    let line = run.wish_on_piece(id, "a flaming purple drank drill\u{7}").unwrap();
    assert!(line.contains("a flaming purple drank drill"));
    assert_eq!(run.home.salvage.pieces[0].wish.as_deref(), Some("a flaming purple drank drill"));
    assert!(run.home.salvage.rev > rev, "a wish is a change the host keeps");
    assert!(run.piece_awaiting(id).is_none());
    assert_eq!(run.players[&id].wishes, wishes, "no held wish is spent");
    assert!(run.wish_on_piece(id, "more").is_err(), "one wish a piece");
    // The shallow floors give up nothing.
    run.dungeon.pack = Pack::Crypt;
    let heard = run.cues.len();
    run.cues.push("boss_fall:bone-king".into());
    run.find_salvage(heard);
    assert_eq!(run.home.salvage.pieces.len(), 1);
}

#[test]
fn the_brief_reads_every_piece_and_the_checker_holds_the_answer_to_the_menu() {
    let yard = full_yard(Grade::Fine, "You");
    let plan = yard.plan().unwrap();
    let text = brief(&plan, &[]);
    for p in &plan.pieces {
        assert!(text.contains(&p.says()), "the brief reads {}", p.says());
    }
    assert!(text.contains("the prow is what digs"));
    let good = "```\nvessel The Sizzurp Belle\nsays A swaggering purple bore with a syrup heart and a cup for a stack.\ntraits drunk, vain\nquirks lush, vain\nvoice launch :: Hold my cup.\nvoice breach :: Surprise, rock.\nvoice ore :: Ooh. Shiny.\nvoice hurt :: Not the paint!\nvoice gnawer :: That worm is not invited.\nvoice home :: Somebody polish me.\nvoice fallen :: Up, sugar.\nvoice idle :: Are we digging or what?\n```";
    let d = check(good, &[]).expect("a good answer checks");
    assert_eq!(d.name, "The Sizzurp Belle");
    assert_eq!(d.quirks, ["lush", "vain"]);
    let errs = |reply: &str| check(reply, &[]).unwrap_err().join(" | ");
    assert!(errs(&good.replace("traits drunk, vain", "traits drunk, sparkly")).contains("`sparkly` is not one of"));
    assert!(errs(&good.replace("quirks lush, vain", "quirks lush, sanctified")).contains("needs the trait `holy`"));
    assert!(errs(&good.replace("voice idle :: Are we digging or what?\n", "")).contains("voice idle: exactly one line"));
    let fleet = [Vessel { name: "The Sizzurp Belle".into(), ..Vessel::default() }];
    assert!(check(good, &fleet).unwrap_err().join(" ").contains("already has"));
}

#[test]
fn the_fallback_always_makes_a_vessel_the_checker_takes() {
    for (grade, wish) in [
        (Grade::Scrap, None),
        (Grade::Relic, Some("a cathedral organ of holy fire")),
        (Grade::Sound, Some("bacon, so much bacon")),
        (Grade::Fine, Some("")),
    ] {
        let mut yard = full_yard(grade, "You");
        for p in &mut yard.pieces {
            p.wish = wish.map(str::to_string);
        }
        let plan = yard.plan().unwrap();
        let mut fleet = Vec::new();
        for _ in 0..3 {
            let d = fallback(&plan, &fleet);
            check(&render(&d), &fleet).unwrap_or_else(|e| panic!("{e:?}\n{}", render(&d)));
            fleet.push(Vessel { name: d.name.clone(), ..Vessel::default() });
        }
    }
    let mut yard = full_yard(Grade::Fine, "You");
    for p in &mut yard.pieces {
        p.wish = Some("a holy bell of blessed angels".into());
    }
    assert_eq!(leanings(&yard.plan().unwrap().pieces)[0], "holy", "her temper is what they wished");
}

/// A design written back as the answer format, for the checker.
fn render(d: &Design) -> String {
    let mut s = format!("vessel {}\nsays {}\ntraits {}\nquirks {}\n", d.name, d.says, d.traits.join(", "), d.quirks.join(", "));
    for (m, l) in &d.voice {
        s.push_str(&format!("voice {m} :: {l}\n"));
    }
    s
}

#[test]
fn a_build_takes_its_pieces_and_the_next_one_must_be_bigger() {
    let mut run = home_run(full_yard(Grade::Sound, "You"));
    let plan = run.salvage_plan().expect("home with a whole set");
    let ids: Vec<u32> = plan.pieces.iter().map(|p| p.id).collect();
    let design = fallback(&plan, &[]);
    assert!(run.build_vessel(2, &ids, design.clone(), false).is_err(), "only the next build");
    let name = run.build_vessel(1, &ids, design.clone(), true).unwrap();
    assert_eq!(name, design.name);
    let yard = &run.home.salvage;
    assert_eq!(yard.vessels.len(), 1);
    assert_eq!(yard.pieces.len(), Place::ALL.len() - 4, "her pieces left the yard");
    assert_eq!(yard.flagship().unwrap().size(), 4 * Grade::Sound.worth());
    assert!(run.build_vessel(1, &ids, design, true).is_err(), "built once");
    // Build two needs six pieces; the yard has six left (no prow among
    // them), so it waits for a prow.
    assert_eq!(run.home.salvage.plan(), None);
    run.home.salvage.add(Place::Prow, Grade::Fine, "You", Pack::Unknown);
    run.home.salvage.add(Place::Hull, Grade::Fine, "You", Pack::Unknown);
    run.home.salvage.add(Place::Heart, Grade::Fine, "You", Pack::Unknown);
    run.home.salvage.add(Place::Treads, Grade::Fine, "You", Pack::Unknown);
    let plan = run.home.salvage.plan().expect("the second build");
    assert_eq!((plan.n, plan.pieces.len()), (2, 6));
    // Away from home nothing is made.
    run.dungeon.depth = 3;
    assert_eq!(run.salvage_plan(), None);
    // The reveal comes once.
    assert!(run.reveal_vessel().is_some());
    assert!(run.cues.iter().any(|c| c == "vessel_reveal:1"));
    assert!(run.reveal_vessel().is_none());
}

#[test]
fn the_newer_yard_survives_the_host_rebuilding_home() {
    let mut run = home_run(Salvage::default());
    let host_home = run.home.clone();
    run.home.salvage.add(Place::Prow, Grade::Relic, "You", Pack::Unknown);
    let treasury = run.treasury.clone();
    run.rebuild_home(host_home, treasury.clone());
    assert_eq!(run.home.salvage.pieces.len(), 1, "a find in the run is not lost to an older home");
    let mut newer = run.home.clone();
    newer.salvage.add(Place::Hull, Grade::Scrap, "You", Pack::Hellforge);
    newer.salvage.rev += 5;
    run.rebuild_home(newer, treasury);
    assert_eq!(run.home.salvage.pieces.len(), 2, "the host's newer yard stands");
    // Saved and read back; a bad yard costs the realm nothing else.
    let json = serde_json::to_string(&run.home).unwrap();
    let back: crate::rules::home::Home = serde_json::from_str(&json).unwrap();
    assert_eq!(back.salvage, run.home.salvage);
    let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
    v["salvage"] = serde_json::json!({"pieces": "not a list"});
    let back: crate::rules::home::Home = serde_json::from_value(v).unwrap();
    assert_eq!(back.salvage, Salvage::default());
}
