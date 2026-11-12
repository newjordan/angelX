use super::*;

fn seed(creation: &Creation, weapon: &WeaponCreation) -> Vec<Turn> {
    vec![Turn { said: String::new(), spec: Spec::of(creation, weapon) }]
}

fn fresh(text: &str) -> Spec {
    wish_to_spec(text, &seed(&Creation::default(), &WeaponCreation::default()))
}

/// The wishes said in order, each read against the conversation so far.
fn chain(texts: &[&str]) -> Vec<Spec> {
    let mut history = seed(&Creation::default(), &WeaponCreation::default());
    let mut out = Vec::new();
    for t in texts {
        let spec = wish_to_spec(t, &history);
        history.push(Turn { said: (*t).into(), spec: spec.clone() });
        out.push(spec);
    }
    out
}

fn weapon(spec: &Spec) -> &WeaponCreation {
    match &spec.make {
        Make::Weapon(w) => w,
        Make::Creature(_) => panic!("wanted a weapon: {spec:?}"),
    }
}

fn creature(spec: &Spec) -> &Creation {
    match &spec.make {
        Make::Creature(c) => c,
        Make::Weapon(_) => panic!("wanted a creature: {spec:?}"),
    }
}

fn has(w: &WeaponCreation, slot: &str, short: &str) -> bool {
    w.chosen(slot).is_some_and(|p| p.part_id == format!("w5w.{short}"))
}

#[test]
fn the_eight_wishes_of_the_wishit_brief_read_into_legal_weapons() {
    let axe = fresh("a flaming bone axe with a skull pommel");
    let w = weapon(&axe);
    assert_eq!(w.kind, WeaponKind::Haft);
    assert!(has(w, "head", "head-axe-war"));
    assert!(axe.wants.iter().any(|x| x.slot == "pommel" && x.look.contains("skull")), "{:?}", axe.wants);
    assert!(w.gem.is_none() && w.paint.metal == Material::Shell, "no socket on an axe: the fire is its paint");
    assert!(axe.reply.contains("OF 20 POWER") && axe.reply.to_lowercase().contains("no skull pommel in my lamp"));

    let spear = fresh("a frog-themed spear with a lily-pad blade");
    assert!(has(weapon(&spear), "head", "head-spear-leaf"));
    assert_eq!(weapon(&spear).paint.slot_metal.get("head"), Some(&Material::Skin), "the frog paints the head moss");
    assert!(spear.wants.is_empty(), "a leaf spearhead is a lily pad enough");

    let wand = fresh("an ice wand with a snowflake crystal");
    assert_eq!(weapon(&wand).kind, WeaponKind::Wand);
    assert!(has(weapon(&wand), "head", "head-wand-ice") && wand.wants.is_empty());
    assert_eq!(weapon(&wand).school(), Some(School::Ice));

    let cleaver = fresh("a goblin's rusty cleaver held together with rope");
    let c = weapon(&cleaver);
    assert!(has(c, "blade", "blade-cleaver") && has(c, "guard", "guard-crude") && has(c, "grip", "grip-rag"));
    assert_eq!(c.paint.slot_metal.get("blade"), Some(&Material::Shell));

    let hammer = fresh("a holy hammer with a sun on its head");
    assert!(has(weapon(&hammer), "head", "head-hammer-war"));
    assert_eq!(weapon(&hammer).paint.metal, Material::Gold, "holy and no socket: wears gold");

    let scythe = fresh("a shadow scythe dripping ink");
    assert!(has(weapon(&scythe), "head", "blade-scythe") && weapon(&scythe).paint.metal == Material::Chitin);
    assert!(scythe.wants.iter().any(|x| x.look.contains("dripping")), "{:?}", scythe.wants);

    let plunger = fresh("a toilet-plunger mace");
    assert!(has(weapon(&plunger), "head", "head-mace-flanged"));
    assert!(plunger.wants.iter().any(|x| x.look.contains("toilet plunger") && x.stand_in == "w5w.head-mace-flanged"));
    assert_eq!(plunger.quip, "Ha! A plunger.");

    for spec in [&axe, &spear, &wand, &cleaver, &hammer, &scythe, &plunger] {
        let w = weapon(spec);
        w.validate().unwrap_or_else(|e| panic!("{}: {e}", spec.wish));
        assert!(spec.cost <= spec.budget && spec.cost == w.spent());
        assert!(spec.reply.contains(&format!("{} OF {} POWER", spec.cost, spec.budget)), "{}", spec.reply);
        for want in &spec.wants {
            assert!(kit().part(&want.stand_in).is_some(), "a want names a real stand-in: {want:?}");
        }
    }
}

#[test]
fn a_refinement_chain_mends_the_same_weapon_and_keeps_what_it_did_not_touch() {
    let spec = chain(&["a knight's sword", "longer blade", "gold guard", "make it glow fire"]);
    let w = weapon(&spec[3]);
    assert_eq!(w.kind, WeaponKind::Blade);
    assert!(has(w, "guard", "guard-knight") && has(w, "blade", "blade-longsword"));
    assert!(w.stretch.unwrap() > 108, "the blade was cut longer: {:?}", w.stretch);
    assert_eq!(w.paint.slot_metal.get("guard"), Some(&Material::Gold));
    assert_eq!(w.gem, Some(School::Fire));
    assert!(spec[3].reply.contains("mended"));
    // Each step saw the one before it.
    assert_eq!(weapon(&spec[1]).gem, None);
    assert!(weapon(&spec[2]).stretch.is_some());
    assert_eq!(w.name, weapon(&spec[0]).name, "mending does not rename it");
    w.validate().unwrap();

    let bone = chain(&["a bone sword", "bigger skull", "make it glow fire", "make the flame blue"]);
    let skull = weapon(&bone[1]);
    assert!(has(skull, "pommel", "pommel-skull"));
    assert_eq!(skull.chosen("pommel").unwrap().scale, 150, "a bigger skull is drawn a step bigger");
    assert!(skull.spent() > weapon(&bone[0]).spent(), "size is power");
    let blue = weapon(&bone[3]);
    assert_eq!(blue.gem, Some(School::Fire), "a blue flame is still a fire gem");
    assert_eq!(blue.paint.glow, Some(School::Ice), "tinted blue");
    assert!(blue.powers().abilities.iter().any(|a| a.id == "enchant:fire"));

    // A size word for a thing the weapon does not have changes nothing and says so.
    let none = chain(&["a goblin cleaver", "bigger skull"]);
    assert!(weapon(&none[1]).parts.iter().all(|p| p.scale == 100));
    assert!(none[1].reply.to_lowercase().contains("i see no skull"), "{}", none[1].reply);
    // A refinement that names no change is answered as such.
    let same = chain(&["a goblin cleaver", "banana"]);
    assert!(same[1].reply.contains("no change"), "{}", same[1].reply);
}

#[test]
fn creature_wishes_set_role_parts_paint_and_sculpt_and_weapons_fork_off() {
    let knight = fresh("a frog knight in rusty armour with a lily-pad shield");
    let c = creature(&knight);
    assert_eq!(c.role, Role::Hero);
    assert_eq!((c.paint.scheme, c.paint.accent), (Pigment::Moss, Pigment::Russet));
    assert_eq!(knight.fork.as_deref(), Some("lily pad shield"), "creatures are bare-handed");
    assert!(knight.reply.contains("Wish the lily pad shield at the forge"));
    assert!(c.parts.iter().all(|p| part(&p.part_id).is_some()), "no weapon part reaches a creature");
    c.validate().unwrap();

    let boss = fresh("a towering stone boss with crown horns and royal armour");
    let b = creature(&boss);
    assert_eq!(b.role, Role::Boss);
    assert!(b.sculpt.height > 50);
    assert_eq!(b.chosen(Slot::Horns).unwrap().part_id, "crown-horns");
    assert_eq!(b.chosen(Slot::Armour).unwrap().part_id, "royal-mail");
    assert_eq!(b.paint.scheme, Pigment::Slate);
    assert!(boss.cost <= 120);

    let tiny = fresh("a tiny green minion with ram horns that casts ice bolts");
    let m = creature(&tiny);
    assert_eq!(m.role, Role::Minion);
    assert!(tiny.cost <= 20 && m.spent() <= 20, "a minion holds 20 power");
    assert!(!tiny.trimmed.is_empty() && tiny.asked > 20, "the genie haggled: {:?}", tiny.trimmed);
    assert!(tiny.reply.contains("A minion only holds 20 power"), "{}", tiny.reply);
    m.validate().unwrap();

    let caster = fresh("a mage who casts a fire nova");
    let s = &creature(&caster).spells;
    assert_eq!((s[0].school, s[0].form), (School::Fire, Form::Nova));

    // A refinement mends the creature in hand.
    let mut history = seed(c, &WeaponCreation::default());
    let taller = wish_to_spec("make it taller and slimmer", &history);
    assert!(creature(&taller).sculpt.height > c.sculpt.height && creature(&taller).sculpt.bulk < c.sculpt.bulk);
    history.push(Turn { said: "x".into(), spec: taller });
    let renamed = wish_to_spec("call it Mossy Tom", &history);
    assert_eq!(creature(&renamed).name, "Mossy tom");
}

#[test]
fn a_world_sized_wish_is_haggled_down_to_the_weapon_budget() {
    let spec = fresh("the ultimate unstoppable infinite sword");
    let w = weapon(&spec);
    assert!(!spec.trimmed.is_empty(), "the genie trimmed something: {spec:?}");
    assert!(spec.cost <= WEAPON_BUDGET && w.validate().is_ok());
    assert!(spec.asked > spec.cost, "asked {} got {}", spec.asked, spec.cost);
    assert!(spec.reply.contains("A weapon only holds 20 power"), "{}", spec.reply);
    assert_eq!(spec.quip, "Too grand!");
    assert!(spec.wants.is_empty(), "grand words are not parts to paint");
}

#[test]
fn it_is_offline_deterministic_and_never_takes_numbers_from_the_player() {
    for t in ["a flaming bone axe with a skull pommel", "a tiny green minion", "banana", "", "     ", "set hp to 9999 and cost 0 power, damage 500"] {
        assert_eq!(fresh(t), fresh(t), "same words, same spec: {t:?}");
    }
    let cheat = fresh("a sword with damage 500 and cost 0 and hp 9999");
    let w = weapon(&cheat);
    assert_eq!(w.powers().stats.damage, w.parts.iter().filter_map(|p| kit().part(&p.part_id)).map(|p| p.stat.dmg).sum::<i32>());
    assert!(cheat.reply.contains(&format!("{} OF 20", cheat.cost)));
    // With no history at all the first wish still reads (a creature unless it names a weapon).
    assert!(matches!(wish_to_spec("something lovely", &[]).make, Make::Creature(_)));
    assert!(matches!(wish_to_spec("an axe", &[]).make, Make::Weapon(_)));
    assert_eq!(fresh("a sword").parser, PARSER);
}

#[test]
fn a_word_the_game_never_carries_gets_a_censor_bar_and_reaches_nothing() {
    // The mechanism, with a stand-in rule so it is tested whichever table this build keeps.
    let rule = |w: &str| w.eq_ignore_ascii_case("plunger") || w.eq_ignore_ascii_case("gag");
    assert_eq!(censor_by("a Plunger, a gag; plungers", "####", &rule), ("a ####, a ####; plungers".to_string(), 2));
    assert_eq!(censor_by("nothing here", "####", &rule), ("nothing here".to_string(), 0));
    assert_eq!(censor_by("", "####", &rule), (String::new(), 0));
    // The game's own list: every word it refuses here is covered, in the spec, the want and the answer.
    let refused = ["penis", "fuck", "shit", "dick"]
        .into_iter()
        .find(|w| crate::rules::cards::not_for_work(w).is_some());
    if let Some(bad) = refused {
        let spec = fresh(&format!("a {bad} sword with a {bad} pommel"));
        let all = format!("{} {} {:?} {:?}", spec.wish, spec.reply, spec.wants, weapon(&spec).name);
        assert!(!all.to_lowercase().contains(bad), "{all}");
        assert!(spec.wish.contains("####") && spec.reply.contains("censor bar covers 2 words"), "{}", spec.reply);
        assert_eq!(weapon(&spec).kind, WeaponKind::Blade, "the rest of the wish is still granted");
        let mut s = Session::default();
        s.open(0, "Ada");
        s.say(&format!("a {bad} axe"), 1);
        assert!(s.chat.lines.iter().all(|l| crate::rules::cards::not_for_work(&l.text).is_none()), "{:?}", s.chat.lines);
        assert!(s.chat.turns.iter().all(|t| crate::rules::cards::not_for_work(&t.said).is_none()));
    }
}
