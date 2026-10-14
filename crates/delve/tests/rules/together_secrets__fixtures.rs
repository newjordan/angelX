//! Fixtures these tests share with the cockpit's (the `testing` feature).

use super::*;

/// The first delve (of 400 seeds, its first floor of `pack`) that, walked
/// down to `depth`, has the room `pick` names; the party in it, calmed.
pub fn on_a_generated_floor(pack: Pack, depth: u32, pick: impl Fn(&Run) -> Option<usize>) -> Run {
    (0..400u64)
        .find_map(|seed| {
            let mut run = Run::new(seed, 1, None);
            run.begin_in(pack);
            while run.dungeon.depth < depth {
                run.descend_for_test();
            }
            let room = pick(&run)?;
            run.enter_for_test(room);
            run.calm_for_test();
            run.rune = None;
            Some(run)
        })
        .expect("a generated floor with the room")
}

pub fn first_of(kind: RoomKind) -> impl Fn(&Run) -> Option<usize> {
    move |run: &Run| run.dungeon.rooms.iter().position(|r| r.kind == kind)
}

/// Knight one, standing at `(x, y)`.
pub fn stand(run: &mut Run, (x, y): (f32, f32)) {
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y, hero.hp) = (x, y, hero.max_hp);
}
