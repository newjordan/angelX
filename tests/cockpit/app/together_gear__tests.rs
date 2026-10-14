use super::*;
use angel_net::proto::{MAX_CLIENT_MESSAGE, ToServer};

#[test]
fn a_small_action_goes_whole_and_a_large_one_in_pieces_that_each_fit_a_message() {
    assert_eq!(pieces(BOON, b"{}"), vec![(BOON, b"{}".to_vec())]);
    let card: Vec<u8> = (0..20_000u32).map(|i| b'a' + (i % 26) as u8).collect();
    let sent = pieces(REFORGE_CARD, &card);
    assert!(sent.len() > 20);
    for (kind, body) in &sent {
        assert_eq!(*kind, PIECE);
        let message = ToServer::Action { kind: *kind, body }.to_vec();
        assert!(
            message.len() <= MAX_CLIENT_MESSAGE,
            "{} bytes",
            message.len()
        );
    }
    let mut uploads = Uploads::default();
    let mut whole = None;
    for (kind, body) in &sent {
        assert!(whole.is_none(), "whole only after the last piece");
        whole = uploads.take(3, *kind, body);
    }
    assert_eq!(whole, Some((REFORGE_CARD, card)));
}

#[test]
fn a_piece_out_of_order_or_a_whole_past_its_limit_drops_the_upload() {
    let body = vec![7u8; 5000];
    let sent = pieces(CARD, &body);
    let mut uploads = Uploads::default();
    assert!(uploads.take(1, sent[0].0, &sent[0].1).is_none());
    // The third before the second: what came is dropped.
    assert!(uploads.take(1, sent[2].0, &sent[2].1).is_none());
    assert!(uploads.take(1, sent[1].0, &sent[1].1).is_none());
    // Each seat's upload is its own, and a whole action in between drops it.
    for (kind, piece) in &sent[..2] {
        uploads.take(2, *kind, piece);
    }
    assert_eq!(uploads.take(2, GRANT, b"w1"), Some((GRANT, b"w1".to_vec())));
    assert!(
        sent[2..]
            .iter()
            .all(|(kind, piece)| uploads.take(2, *kind, piece).is_none())
    );
    // Too large to be any action once whole.
    let huge = vec![1u8; MAX_WHOLE + 4000];
    let mut whole = None;
    for (kind, piece) in pieces(CARD, &huge) {
        whole = whole.or(uploads.take(4, kind, &piece));
    }
    assert!(whole.is_none());
}

#[test]
fn actions_are_checked_as_the_hosts_routes_checked_them() {
    assert!(matches!(read(2, GRANT, b" w1 "), Ok(Gear::Grant(id)) if id == "w1"));
    assert!(read(2, BOON, br#"{"wish":"no-such-wish"}"#).is_err());
    assert!(
        read(2, LEARN, br#"{"words":"ok"}"#).is_err(),
        "a few words, at least"
    );
    assert!(
        matches!(read(2, LEARN, br#"{"words":"a lantern that sings"}"#), Ok(Gear::Learn(w)) if w == "a lantern that sings")
    );
    assert!(read(2, WISH, b"   ").is_err());
    assert!(read(2, 77, b"").is_err());
}
