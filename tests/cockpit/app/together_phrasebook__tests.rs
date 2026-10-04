use super::*;

#[test]
fn every_wish_in_the_phrasebook_is_a_card_the_checker_takes_at_every_tier() {
    assert!(book().len() >= 12, "{}", book().len());
    for wish in book().iter() {
        assert!(!wish.says.is_empty(), "{} has phrases", wish.id);
        for tier in 0..wish.tiers.len() {
            let card = wish
                .card(tier)
                .unwrap_or_else(|e| panic!("{} tier {tier}: {e}", wish.id));
            assert_eq!(card.kind, cards::Kind::Hold);
            assert!(
                !card.effects.is_empty(),
                "{} tier {tier} does something",
                wish.id
            );
        }
    }
}

#[test]
fn players_words_find_the_wish_they_mean() {
    let id = |said: &str| find(said).map(|w| w.id);
    assert_eq!(id("triple wide fire my shots"), Some("volley".to_string()));
    assert_eq!(
        id("make my shots bounce off the walls"),
        Some("ricochet".to_string())
    );
    assert_eq!(id("I want homing shots please"), Some("seeker".to_string()));
    assert_eq!(
        id("fire faster"),
        Some("quick".to_string()),
        "the fuller phrase wins over `faster`"
    );
    assert_eq!(id("move faster"), Some("fleet".to_string()));
    assert_eq!(id("heal my friend when I hit"), Some("mend".to_string()));
    assert_eq!(id("chain lightning"), Some("chain".to_string()));
    assert_eq!(id("make my shots explosive"), Some("powder".to_string()));
    assert_eq!(id("summon a dragon to ride"), None);
}

#[test]
fn a_wish_climbs_its_ladder_and_stops_at_the_top() {
    let volley = get("volley").unwrap();
    let mut deck: Vec<String> = Vec::new();
    assert_eq!(volley.next_tier(&deck), Some(0));
    assert!(volley.card_text(0).contains("volley 5"));
    deck.push(volley.card_id(0));
    assert_eq!(volley.next_tier(&deck), Some(1));
    deck = vec![volley.card_id(2)];
    assert_eq!(volley.next_tier(&deck), None);
}

#[test]
fn drawn_wishes_carry_their_own_art() {
    let volley = get("volley").unwrap().card(0).unwrap();
    let rows = volley.art_rows();
    assert!(
        rows.len() >= 10,
        "the volley icon, not the kind's glyph: {rows:?}"
    );
}

#[test]
fn an_entry_may_carry_its_own_card_art() {
    let book = read(
        "wish glow | Glow\nsay make it glow\ntier homing 1 :: It glows.\nart\n..5..\n.545.\n..5..\n\nwish next | Next\nsay next\ntier rate 5 :: Next.\n",
    );
    assert_eq!(book.len(), 2);
    assert_eq!(book[0].art, vec!["..5..", ".545.", "..5.."]);
    assert!(book[1].art.is_empty());
    let card = book[0].card(0).unwrap();
    assert_eq!(card.art_rows().len(), 3, "the entry's art, not a glyph");
}
