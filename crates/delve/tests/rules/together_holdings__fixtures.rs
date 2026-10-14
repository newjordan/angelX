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
    settle(&mut h, &mut t, &marks(&[&build_mark(&crypt_key(), Family::Tavern)]));
    settle(&mut h, &mut t, &marks(&[&build_mark(&crypt_key(), Family::Chapel)]));
    settle(&mut h, &mut t, &BTreeMap::from([(LABOUR.to_string(), 12)]));
    (h, t)
}
