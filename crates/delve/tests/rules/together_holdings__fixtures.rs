//! Fixtures these tests share with the cockpit's (the `testing` feature).

use super::*;
use crate::realm::{Spoil, Spoils};

pub fn gold(n: u32) -> Spoils {
    let mut s = Spoils::default();
    s.add(Spoil::Gold, n);
    s
}

pub fn marks(list: &[&str]) -> BTreeMap<String, u32> {
    list.iter().map(|&m| (m.to_string(), 1)).collect()
}

pub fn marks_of(list: &[String]) -> BTreeMap<String, u32> {
    list.iter().map(|m| (m.clone(), 1)).collect()
}

/// The Foreman brings his petition for `room` at `holding`, priced at the
/// list: staging his ledge by hand, as the tests do.
pub fn stage(h: &mut Holdings, key: &str, room: &str) {
    let list = holdings::room_def(room).map(|d| holdings::room_price(h, d)).unwrap_or_default();
    h.foreman.queue.push(crate::rules::foreman::Petition {
        key: key.into(),
        room: room.into(),
        gold: list,
        relic: None,
        says: "sound oak and honest nails".into(),
    });
}

pub fn aye() -> String {
    crate::rules::foreman::aye_mark()
}

pub fn nay() -> String {
    crate::rules::foreman::nay_mark()
}

/// Holdings whose Crypt is cleared and whose Sol house has served.
pub fn cleared() -> Holdings {
    let mut h = Holdings::default();
    h.cleared.insert(Pack::Crypt.name().to_string());
    h.served.insert("sol".into());
    h
}

pub fn crypt() -> Deed {
    Deed::Floor(Pack::Crypt)
}

pub fn crypt_key() -> String {
    crypt().key()
}

/// A Crypt bought outright, with a steward and two rooms standing.
pub fn held() -> (Holdings, Spoils) {
    let mut h = cleared();
    let mut t = gold(20_000);
    settle(&mut h, &mut t, &marks(&[&buy_mark(crypt(), false)]));
    stage(&mut h, &crypt_key(), "taproom");
    settle(&mut h, &mut t, &marks_of(&[aye()]));
    stage(&mut h, &crypt_key(), "shrine");
    settle(&mut h, &mut t, &marks_of(&[aye()]));
    settle(&mut h, &mut t, &BTreeMap::from([(LABOUR.to_string(), 12)]));
    (h, t)
}
