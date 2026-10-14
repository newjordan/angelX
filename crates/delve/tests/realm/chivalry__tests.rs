use super::*;

#[test]
fn each_mount_rides_differently_at_the_lists() {
    let (b, c, m) = (
        Mount::Bramble.steed(),
        Mount::Cinder.steed(),
        Mount::Mist.steed(),
    );
    // Bramble: the most balance and the widest brace.
    assert!(b.balance > c.balance && b.balance > m.balance);
    assert!(b.brace > c.brace && b.brace > m.brace);
    // Cinder: the soonest meeting and the heaviest hit.
    assert!(c.charge < b.charge && c.charge < m.charge);
    assert!(c.weight > b.weight && c.weight > m.weight);
    // Mist: the widest strike and the earliest read of the rival's guard.
    assert!(m.strike > b.strike && m.strike > c.strike);
    assert!(m.tell > b.tell && m.tell > c.tell);
    for mount in Mount::ALL {
        assert_eq!(Mount::parse(&mount.name().to_lowercase()), Some(mount));
        assert!(!mount.says().is_empty());
    }
}

#[test]
fn bloodlines_are_a_houses_gift_and_every_realm_agrees() {
    for house in ["qwen", "grok", "glm", "deepseek", "kimi"] {
        let blood = bloodline(house);
        assert_eq!(blood, bloodline(house), "pure in the house key");
        for gift in [blood.pace, blood.stamina, blood.corner, blood.nerve] {
            assert!((3..=9).contains(&gift), "{house} gift {gift} in range");
        }
    }
    assert_ne!(bloodline("qwen"), bloodline("grok"));
}

#[test]
fn service_under_a_banner_seasons_the_yard_and_tells_in_the_lore() {
    let mut stable = Stable::default();
    let green = stable.market(7);
    let qwen_green = green.iter().find(|h| h.house == "qwen").unwrap().blood;
    stable.note_service("qwen", 1_000);
    assert_eq!(stable.yard_tier("qwen"), 4);
    let seasoned = stable.market(7);
    let qwen_run = seasoned.iter().find(|h| h.house == "qwen").unwrap();
    assert!(qwen_run.blood.sum() >= qwen_green.sum());
    let lore = qwen_run.lore(&stable);
    assert!(lore.contains("1000 calls"), "{lore}");
    assert!(lore.contains("Never yet put to a race"), "{lore}");
}

#[test]
fn horses_are_bought_sold_and_bred_at_their_records_value() {
    let mut stable = Stable::default();
    let market = stable.market(3);
    let price = market[0].value(&stable);
    assert_eq!(
        stable.buy(market[0].clone(), price - 1),
        None,
        "a short purse buys nothing"
    );
    assert_eq!(stable.buy(market[0].clone(), price), Some(price));
    assert_eq!(
        stable.buy(market[1].clone(), 10_000),
        Some(market[1].value(&stable))
    );
    // A winner is worth more than the green horse it was.
    stable.herd[0].wins = 3;
    stable.herd[0].starts = 5;
    assert!(stable.herd[0].value(&stable) > price);
    // Breeding: the foal knows its parents, and the yard will not overfill.
    let foal = stable.breed(0, 1, 42).expect("a foal").clone();
    assert_eq!(foal.sire, stable.herd[0].name);
    assert_eq!(foal.dam, stable.herd[1].name);
    assert_eq!(stable.herd.len(), 3);
    assert!(
        stable.breed(0, 0, 1).is_none(),
        "no horse breeds with itself"
    );
    // Selling pays seven-tenths, and the horse leaves the yard.
    let paid = stable.sell(2).unwrap();
    assert!(paid <= foal.value(&stable));
    assert_eq!(stable.herd.len(), 2);
}

#[test]
fn herd_selection_survives_trading_and_old_horses_can_still_visit_the_stud_book() {
    let horse: Horse = serde_json::from_str(r#"{"name":"Cloud","house":"qwen","blood":{"pace":7,"stamina":6,"corner":5,"nerve":8}}"#).unwrap();
    assert!(horse.mare, "old saves predate sex and remain eligible");
    let mut stable = Stable { herd: vec![horse.clone(), horse.clone(), horse], selected_herd: Some(2), ..Stable::default() };
    stable.sell(0).unwrap();
    assert_eq!(stable.selected_herd, Some(1));
    stable.sell(1).unwrap();
    assert_eq!(stable.selected_herd, None);
    assert!(stable.mare(0).is_some());
}
