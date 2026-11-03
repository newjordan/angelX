use super::*;
use crate::realm::{Spoil, Spoils};
use crate::rules::raisings::Survey;

#[test]
fn the_sign_says_the_rod_is_there_once_motes_are_banked_and_keeps_out_of_its_way() {
    let mut r = Run::at_home(41, 1, None, Default::default(), Default::default());
    r.walk_kingdom(Some(crate::world::kingdom::keep_stand()));
    let geo = map::geography(&r.kingdom.wishes);
    // Out in the open, far from every door.
    let hero = r.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (6.0, 6.0);
    r.treasury = Spoils::default();
    assert!(footer_sign(&r, &geo, Some(1)).is_none());
    r.treasury.add(Spoil::Wishmote, 1);
    assert!(footer_sign(&r, &geo, Some(1)).is_some());
    r.kingdom.survey.insert(1, Survey::default());
    assert!(footer_sign(&r, &geo, Some(1)).is_none());
}
