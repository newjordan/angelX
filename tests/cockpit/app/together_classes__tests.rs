//! The classes' shots and the sounds the cockpit has for them.

use crate::drive::together_shooter::*;
use crate::drive::together_shooter::classes::*;
use crate::drive::together_shooter::classes::testing::*;
use std::collections::BTreeMap;

#[test]
fn the_mage_and_healer_loose_their_own_shot_with_the_bows_numbers() {
    let loose = |class: &str| {
        let mut r = kingdom();
        r.become_class(1, CLASSES.iter().find(|c| c.name == class).unwrap());
        r.enemies.clear();
        r.projectiles.clear();
        r.sounds.clear();
        r.step(&BTreeMap::from([(1, Input { aim_x: 1, fire: true, ..Default::default() })]));
        let shot = r.projectiles.iter().find(|p| !p.hostile).cloned().expect("a shot");
        (shot, r.sounds.clone(), r.players[&1].fire_cooldown)
    };
    let (arrow, sounds, rest) = loose("Archer");
    assert_eq!(arrow.look, None);
    assert!(sounds.contains(&"shot_bow"));
    for (class, colour, sound) in [("Mage", ARCANE, "shot_arcane"), ("Healer", HOLY, "shot_spark")] {
        let (shot, sounds, cooldown) = loose(class);
        assert_eq!(shot.look.as_ref().map(|l| l.0.as_str()), Some(colour), "{class}");
        assert!(sounds.contains(&sound), "{class}: {sounds:?}");
        // Look only: the bow's numbers stand.
        assert_eq!(shot.kind, arrow.kind);
        assert_eq!((shot.damage, shot.ttl, shot.pierce), (arrow.damage, arrow.ttl, arrow.pierce), "{class}");
        assert_eq!((shot.vx, shot.vy), (arrow.vx, arrow.vy));
        assert_eq!(cooldown, rest);
    }
    for name in ["shot_arcane", "shot_spark"] {
        assert!(crate::drive::together_audio::SFX.contains(&name));
        assert!(crate::drive::together_audio::sfx_file(name).is_some(), "{name}.mp3");
    }
}
