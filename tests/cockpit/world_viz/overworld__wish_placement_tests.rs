//! Wishes placed from the cockpit's realm.

use crate::stage::world_viz::overworld::wishes::*;
use crate::stage::world_viz::overworld::{MAP_H, MAP_W};

#[test]
fn a_realm_without_any_free_lot_reports_every_undrawn_landmark() {
    let occupied = Lot {
        tx: 0,
        ty: 0,
        tw: MAP_W,
        th: MAP_H,
    };
    assert!(
        find(2, 2, (12, 12), &[occupied]).is_none(),
        "both placement passes are full"
    );
    let mut realm = crate::drive::together_realm::Realm::default();
    realm.ask("Jordan", "a golden hall").unwrap();
    realm.ask("Jordan", "a spiked gate").unwrap();
    let names = names_without_ground(&realm.wishes, &[Some(delve_gate()), None]);
    assert_eq!(names, [realm.wishes[1].name.clone()]);
    assert_eq!(names_without_ground(&realm.wishes, &[None, None]).len(), 2);
}
