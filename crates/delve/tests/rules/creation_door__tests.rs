use super::*;

fn door_session(row: usize) -> Session {
    let mut s = Session::default();
    s.open = true;
    s.hub = true;
    s.row = row;
    s.gate.power = true;
    s.action(Action::Confirm, 1);
    s
}

#[test]
fn door_rows_open_a_typed_line_and_a_wrong_row_is_not_bent() {
    for row in [HubRow::Power, HubRow::Creature, HubRow::Person, HubRow::Weapon] {
        let mut s = door_session(row.index());
        assert_eq!(s.door.row, Some(row.index() as u8));
        assert!(s.chat.open);
        assert!(s.hub);
        let wrong = match row {
            HubRow::Power => "three minions",
            HubRow::Creature => "a villager",
            HubRow::Person => "a wolf",
            HubRow::Weapon => "three minions",
            _ => "a wolf",
        };
        assert!(s.hear_wish(wrong));
        assert!(s.door.row.is_none(), "{row:?} bent {wrong}");
        assert!(!s.door.waiting);
        assert!(!s.note.is_empty());
    }
}

#[test]
fn a_stub_reply_becomes_the_thing_and_silence_falls_back() {
    let ink = "wish golden-devil | Golden devil\nkind spell\nsay a tiny golden devil | tiny devil\ntier summon 1 swarm :: tiny golden devils\ndraw summon\nkkkkkk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkkkkkk\n";
    let rows = [
        (HubRow::Power, "a fire bolt", "wish fire-bolt | Fire bolt\nkind hold\nsay a fire bolt | fire\ntier damage 2 :: a hotter bolt\n"),
        (HubRow::Creature, "a tiny golden devil", ink),
        (HubRow::Person, "a tall smith", "wish tall-smith | Tall smith\nkind hold\nsay a tall smith | smith\ndraw worn\nkkkkkk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkkkkkk\n"),
        (HubRow::Weapon, "a long spear", "wish long-spear | Long spear\nkind hold\nsay a long spear | spear\ndraw worn\nkkkkkk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkkkkkk\n"),
    ];
    for (row, words, reply) in rows {
        let mut s = door_session(row.index());
        assert!(s.hear_wish(words));
        assert!(s.door.waiting);
        let sent = s.take_ask().expect("one send");
        assert!(sent.starts_with(&format!("creator-wish {} ", row.index())));
        assert!(sent.contains(words));
        assert!(s.take_ask().is_none(), "the draw thread must not send twice");
        assert!(s.take_reply(reply, 4));
        assert_eq!(s.door.answered, "THE MODEL");
        assert!(!s.door.waiting);
        assert!(!s.door.thing.is_empty());
        if row != HubRow::Power {
            assert!(!s.door.ink.is_empty(), "{row:?} kept no ink");
        }
    }
    let mut s = door_session(HubRow::Creature.index());
    assert!(s.hear_wish("a wolf"));
    s.give_up(9);
    assert_eq!(s.door.answered, "OFFLINE KEYWORDS");
    assert!(!s.door.waiting);
    assert!(!s.door.thing.is_empty() || !s.creation.name.is_empty());
}
