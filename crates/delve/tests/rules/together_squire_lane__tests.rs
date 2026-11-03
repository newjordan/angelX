use super::*;

#[test]
fn squires_are_part_of_the_realms_home_and_old_saves_get_an_empty_company() {
    let old: home::Home = serde_json::from_str("{}").unwrap();
    let value = serde_json::to_value(&old).unwrap();
    assert!(value.get("squire_company").is_none(), "an unused company must preserve legacy save bytes");
    let populated = home_with_squire();
    let restored: home::Home = serde_json::from_value(serde_json::to_value(&populated).unwrap()).unwrap();
    assert_eq!(restored.squire_company, populated.squire_company);
}

fn home_with_squire() -> home::Home {
    let mut home = home::Home::default();
    home.squire_company.rosters.insert("Ada".into(), squire_company::Roster {
        squires: vec![squire_company::Squire { id: 1, name: "Ash".into(), kind: squire_company::Kind::Ninja, level: 3, trained_on: None }],
        active: Some(1),
        claimed: Vec::new(),
    });
    home
}

#[test]
fn squires_descending_deploys_selected_companion_and_refreshes_board_once_per_delve() {
    let mut run = Run::at_home(61, 1, None, home_with_squire(), Spoils::default());
    run.players.get_mut(&1).unwrap().name = "Ada".into();
    let offers = run.home.squire_company.offers();
    run.descend_for_test();
    assert_eq!(run.squire_field.companions.len(), 1, "the actual stair must bring the selected squire");
    assert_eq!(run.squire_field.companions[&1].squire.name, "Ash");
    assert_eq!(run.home.squire_company.delves, 1);
    assert_ne!(offers, run.home.squire_company.offers());
    run.descend_for_test();
    assert_eq!(run.squire_field.companions.len(), 1);
    assert_eq!(run.home.squire_company.delves, 1, "a floor is not a fresh delve");
}

#[test]
fn squires_yard_e_key_reaches_the_treasury_purchase_path() {
    let mut run = Run::at_home(61, 1, None, home::Home::default(), Spoils::default());
    let yard = world::room_of(&run.dungeon, RoomKind::SquireBarracks).unwrap();
    let (x,y,w,h) = squire_company::company::RECRUIT_PLATES[0];
    run.arrive(yard, ((x as f32+w as f32/2.0), (y as f32+h as f32/2.0)));
    run.step(&BTreeMap::new());
    run.step(&BTreeMap::from([(1, Input { interact: true, ..Input::default() })]));
    assert!(run.peddled.iter().any(|p| p.shop == merchant::Shop::Squires), "normal E input must ask the treasury");
}
