//! The realm's own wishes: landmarks any party can work toward from the
//! first run, as `.wish` files in `cockpit/assets/realm/wishes/`.

/// Wishes offered only once the party has slain the dragon.
pub(crate) const AFTER_VICTORY: &[(&str, &str)] = &[(
    "grail-chapel",
    include_str!("../../../assets/realm/wishes/grail-chapel.wish"),
)];

pub(crate) const CATALOG: &[(&str, &str)] = &[
    (
        "wayside-shrine",
        include_str!("../../../assets/realm/wishes/wayside-shrine.wish"),
    ),
    (
        "fellowship-stone",
        include_str!("../../../assets/realm/wishes/fellowship-stone.wish"),
    ),
    (
        "beacon-tower",
        include_str!("../../../assets/realm/wishes/beacon-tower.wish"),
    ),
    (
        "miners-lodge",
        include_str!("../../../assets/realm/wishes/miners-lodge.wish"),
    ),
    (
        "tavern",
        include_str!("../../../assets/realm/wishes/tavern.wish"),
    ),
    (
        "hall-of-the-dragonslayers",
        include_str!("../../../assets/realm/wishes/hall-of-the-dragonslayers.wish"),
    ),
];
