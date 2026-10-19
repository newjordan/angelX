use super::*;

fn shot(owner: u8, size: u8) -> Projectile {
    let mut traits = ShotTraits::default();
    traits.owner = owner;
    traits.size = size;
    Projectile {
        x: 10.0,
        y: 10.0,
        vx: 1.0,
        vy: 0.0,
        hostile: false,
        kind: Shot::Arrow,
        look: None,
        damage: 10,
        pierce: 0,
        last_hit: None,
        ttl: 200,
        empowered: false,
        traits,
    }
}

const VIEW: f32 = 1000.0;

#[test]
fn the_party_s_shots_never_cover_more_than_their_share_of_the_screen() {
    let mut shots: Vec<Projectile> = (0..400).map(|i| shot(1 + (i % 4) as u8, 3)).collect();
    assert!(covered(&shots, VIEW) > 100.0, "four hundred giant shots fill the screen");
    let retired = hold(&mut shots, VIEW);
    assert!(retired > 0);
    assert!(covered(&shots, VIEW) <= PARTY_COVER + 0.01, "{}", covered(&shots, VIEW));
}

#[test]
fn the_oldest_shots_go_first_so_a_new_volley_always_flies() {
    let mut shots: Vec<Projectile> = (0..200).map(|_| shot(1, 0)).collect();
    shots.last_mut().unwrap().damage = 99;
    hold(&mut shots, VIEW);
    assert_eq!(shots.last().unwrap().damage, 99, "the newest shot is kept");
    assert!(shots.len() < 200);
}

#[test]
fn bigger_shots_take_more_of_the_share_than_plain_ones() {
    assert!(area(&shot(1, 3)) > 2.0 * area(&shot(1, 0)));
}

#[test]
fn monsters_shots_are_never_retired_for_the_party_s_room() {
    let mut shots: Vec<Projectile> = (0..300).map(|_| shot(1, 3)).collect();
    for p in shots.iter_mut().take(50) {
        p.hostile = true;
        p.traits.owner = 0;
    }
    hold(&mut shots, VIEW);
    assert_eq!(shots.iter().filter(|p| p.hostile).count(), 50);
}

#[test]
fn a_knight_s_shot_lives_three_seconds_and_bounces_twice_at_most() {
    let mut shots = vec![shot(1, 0)];
    shots[0].ttl = 10 * HZ;
    shots[0].traits.bounce = 8;
    hold(&mut shots, VIEW);
    assert_eq!(shots[0].ttl, SHOT_LIFE);
    assert_eq!(shots[0].traits.bounce, BOUNCES);
}

#[test]
fn a_room_smaller_than_the_screen_is_its_own_view() {
    let run = Run::new(1, 701, None);
    let room = run.room();
    let screen = layout::COLS as f32 * layout::TILE_UNITS * layout::ROWS as f32 * layout::TILE_UNITS;
    assert!(view(room) <= screen);
}
