//! Powers change dimensions, the hook: a knight's shot lies low in the 2D
//! black dungeon and stands up on a floor that stands up in 3D, or once its
//! knight has been taught a wish's third tier; a monster's shot keeps low;
//! a raised shot is its low self, grown, lifted and rolling.

use super::*;
use crate::rules::Run;
use crate::world3d::room::{self, Mode};

/// A run on floor `depth` with a ring of knight 1's shots in flight, and
/// one monster's.
fn shots_at(depth: u32) -> Run {
    let mut run = Run::new(9, 1, Some("Friend"));
    while run.dungeon.depth < depth {
        run.descend_for_test();
    }
    run.projectiles.clear();
    run.volley_for_test(20.0, 12.0, 0.0);
    for shot in &mut run.projectiles {
        shot.traits.owner = 1;
    }
    run.projectiles[0].hostile = true;
    run.projectiles[0].traits.owner = 0;
    run
}

#[test]
fn a_knights_shots_lie_low_on_the_upper_floors_and_stand_up_in_the_deep() {
    room::set_mode(Mode::Deep);
    let upper = shots_at(1);
    let d = Dimensions::of(&upper);
    assert!(upper.projectiles.iter().all(|s| d.shot(s) == Dimension::Low), "the black dungeon's shots");
    let deep = shots_at(room::DEEP);
    let d = Dimensions::of(&deep);
    assert_eq!(d.shot(&deep.projectiles[0]), Dimension::Low, "a monster's shot keeps low");
    assert!(deep.projectiles[1..].iter().all(|s| d.shot(s) == Dimension::Raised));
    assert_eq!(of_shot(&deep, &deep.projectiles[1]), Dimension::Raised);
    room::set_mode(Mode::Off);
}

#[test]
fn a_knight_taught_a_wishs_third_tier_raises_their_shots_on_any_floor() {
    room::set_mode(Mode::Deep);
    let mut run = shots_at(1);
    let hero = run.players.get_mut(&1).unwrap();
    hero.deck.push("wish-ember-2".into());
    assert_eq!(wish_tier(hero), 2);
    assert_eq!(of_shot(&run, &run.projectiles[1]), Dimension::Low, "wished, not yet taught");
    run.players.get_mut(&1).unwrap().deck.push(format!("wish-ember-{RAISED_TIER}"));
    assert_eq!(of_shot(&run, &run.projectiles[1]), Dimension::Raised);
    assert_eq!(of_shot(&run, &run.projectiles[0]), Dimension::Low, "not the monster's");
    room::set_mode(Mode::Off);
}

#[test]
fn a_raised_shot_is_its_low_self_grown_lifted_and_rolling() {
    assert!(Dimension::Raised.lift() > Dimension::Low.lift());
    assert!(Dimension::Raised.scale() > 1.0 && Dimension::Low.scale() == 1.0);
    assert_eq!(Dimension::Low.roll(), 0.0);
    let mut shot = super::super::room_scene::SceneShot {
        x: 40.0,
        y: 40.0,
        vx: 30.0,
        vy: 0.0,
        hue: super::super::room_scene::Hue::Fire,
        bright: true,
        hostile: false,
        empowered: false,
        form: super::super::room_scene::ShotForm::Kind(crate::rules::Shot::Arrow),
        dimension: Dimension::Low,
        mark: super::super::Img::new(1, 1),
    };
    let (mut low, mut raised) = (Vec::new(), Vec::new());
    super::super::shot_meshes::build(&mut low, 0, &shot, (0.0, 0.0));
    shot.dimension = Dimension::Raised;
    super::super::shot_meshes::build(&mut raised, 0, &shot, (0.0, 0.0));
    assert_eq!(low.len(), raised.len(), "the same shapes");
    let mean_height = |n: &[angel_stage::scene::Node]| n.iter().map(|n| n.transform.translation[1]).sum::<f32>() / n.len() as f32;
    assert!(mean_height(&raised) > mean_height(&low) + 3.0, "lifted to a knight's chest");
    let size = |n: &[angel_stage::scene::Node]| n.iter().map(|n| n.transform.scale[0]).sum::<f32>();
    assert!(size(&raised) > size(&low) * 1.2, "grown");
}
