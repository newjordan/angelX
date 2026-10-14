//! Fixtures these tests share with the cockpit's (the `testing` feature).

use super::*;
use crate::realm::Spoil;

/// A party of `seats` knights in a race hall holding `course`, past the
/// countdown.
pub fn racing(course: Course, seats: u32) -> Run {
    let mut run = (0..80u64)
        .find_map(|seed| {
            let mut r = Run::new(seed, 1, None);
            for _ in 0..6 {
                r.descend_for_test();
                if Course::of_floor(r.dungeon.depth) == course && r.race3d_room.is_some() {
                    return Some(r);
                }
            }
            None
        })
        .expect("a floor with that race");
    for id in 2..=seats {
        run.join_seat(id, ["", "", "Lynette", "Gareth", "Galahad"][id as usize]);
    }
    run.enemies.clear();
    for hero in run.players.values_mut() {
        hero.carried.add(Spoil::Gold, 60);
    }
    run.enter_for_test(run.race3d_room.unwrap());
    run.race3d.as_mut().unwrap().t = TITLE;
    run
}

/// Each seat a different class: a swordsman, a mage, a berserker, a healer.
pub fn classed(run: &mut Run) {
    for (id, class) in [(1u32, "Swordsman"), (2, "Mage"), (3, "Berserker"), (4, "Healer")] {
        if let Some(hero) = run.players.get_mut(&id) {
            hero.class = Some(class.into());
        }
    }
}
