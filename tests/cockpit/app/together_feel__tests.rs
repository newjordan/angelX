//! The feel of a fight on the cockpit's own screen.

use crate::drive::together_shooter::*;
use crate::drive::together_shooter::feel::testing::*;
use std::collections::BTreeMap;

/// Integrated fights: `ANGEL_ARENA_SHOTS=<dir> cargo test write_integration_shots -- --ignored`.
#[test]
#[ignore]
fn write_integration_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS").map(std::path::PathBuf::from) else {
        return;
    };
    for (weapon, arm) in [
        (Weapon::Bow, "bow"),
        (Weapon::Crossbow, "crossbow"),
        (Weapon::Handgonne, "handgonne"),
    ] {
        let mut run = quiet();
        bare(&mut run);
        run.dungeon.pack = Pack::Archive;
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y, hero.weapon, hero.temper) = (10.0, 14.0, weapon, 2);
        hero.arm = Some(arm.into());
        hero.hand = vec!["potion".into()];
        hero.ult_charge = 650;
        for n in 0..cards::SPELL_SLOTS {
            let id = format!("review-spell-{n}");
            let card = cards::check(
                &id,
                &format!("name Ember {}\nkind spell\ncooldown 4\ncast ring\n", n + 1),
            )
            .unwrap()
            .card;
            run.book.insert(card);
            run.players.get_mut(&1).unwrap().spells[n] = Some(id);
        }
        for (kind, x, y) in [
            (EnemyKind::Wraith, 30.0, 7.0),
            (EnemyKind::Skeleton, 36.0, 18.0),
            (EnemyKind::Imp, 38.0, 10.0),
        ] {
            a_foe(&mut run, kind, x, y);
        }
        // A real fight's incoming patterns develop before the knight opens fire.
        idle(&mut run, 45);
        run.tick_feel(&[(1, 12); 10], &[]);
        for _ in 0..7 {
            run.step(&BTreeMap::from([(1, fire(1, 0))]));
        }
        assert!(
            run.projectiles
                .iter()
                .any(|p| p.hostile && p.kind == Shot::Page)
        );
        assert!(
            run.projectiles
                .iter()
                .any(|p| !p.hostile && p.traits.arm && p.traits.temper == 2)
        );
        save(&frame(&run), &dir, &format!("integrated_{arm}"));
        super::together_polish_tests::write_screen(
            &run,
            &dir,
            &format!("integrated_{arm}_hud"),
            160,
            48,
        );
        std::fs::write(
            dir.join(format!("integrated_{arm}_hud.json")),
            serde_json::to_vec_pretty(&run.hud(1, &hud::Keys::FIRST).unwrap()).unwrap(),
        )
        .unwrap();
    }
}
