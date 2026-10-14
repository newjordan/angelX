//! Fixtures these tests share with the cockpit's (the `testing` feature).

use super::*;
use crate::realm::{Spoil, Spoils};

pub fn rich() -> Spoils {
    let mut s = Spoils::default();
    for (spoil, n) in [
        (Spoil::Gold, 9000),
        (Spoil::Ore, 200),
        (Spoil::Bone, 100),
        (Spoil::Gem, 40),
        (Spoil::Ember, 40),
    ] {
        s.add(spoil, n);
    }
    s
}

pub fn marks(list: &[(&str, u32)]) -> BTreeMap<String, u32> {
    list.iter().map(|&(m, n)| (m.to_string(), n)).collect()
}

pub fn who(id: u32) -> String {
    format!("Knight {id}")
}
