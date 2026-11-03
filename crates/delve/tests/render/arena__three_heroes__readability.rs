//! Additional visual regression checks; the supplied shot tests remain unchanged.
use super::*;

#[test]
fn render_readability_farwatch_raises_both_arms_and_has_a_cast_flash() {
    use crate::render::overworld::ink::ink;
    let idle = draw("farwatch", Facing::Side, false, None, false);
    let cast = draw("farwatch", Facing::Side, true, None, false);
    let changed = (3..13).flat_map(|y| (5..20).map(move |x| (x, y)))
        .filter(|&(x, y)| idle.get(x, y) != cast.get(x, y)).count();
    assert!(changed >= 50, "cast needs a raised aiming silhouette, only {changed} pixels change");
    let flash = (0..10).flat_map(|y| (16..20).map(move |x| (x, y)))
        .filter(|&(x, y)| [ink('5'), ink('6'), ink('w')].contains(&cast.get(x, y))).count();
    assert!(flash >= 6, "aimed shot needs a legible bright muzzle flash");
}

#[test]
fn render_readability_galeweaver_coat_uses_one_foliage_ramp() {
    use crate::render::overworld::ink::{bank_of, BANK};
    let im = draw("galeweaver", Facing::Down, false, None, false);
    let foliage = (2..14).flat_map(|x| (2..14).map(move |y| (x, y)))
        .filter(|&(x, y)| im.get(x, y).and_then(bank_of) == Some(1)).count();
    assert!(foliage >= BANK * 4, "large connected green cloak, not blue/green/red patchwork: {foliage}");
    let red = crate::render::overworld::ink::ink('7');
    assert!((3..14).all(|x| (6..14).all(|y| im.get(x, y) != red)), "no red checker trim on green cloak");
}

#[test]
fn cinderheart_cast_raises_a_free_hand_outside_the_robes_silhouette() {
    let stand = draw("cinderheart", Facing::Down, false, None, false);
    let cast = draw("cinderheart", Facing::Down, true, None, false);
    assert!(stand.get(1, 5).is_none());
    assert!(cast.get(1, 5).is_some(), "a raised free hand must read without counting ember pixels");
}
