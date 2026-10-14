use super::*;
use super::testing::*;

#[test]
fn ballots_are_per_seat_and_block_all_game_inputs_while_the_party_moves() {
    for scene in 0..3 {
        let mut run = if scene == 2 {
            Run::new(91, 7, Some("Friend"))
        } else {
            Run::at_home(
                91,
                7,
                Some("Friend"),
                Default::default(),
                Default::default(),
            )
        };
        if scene == 1 {
            run.walk_kingdom(None);
        }
        command(&mut run, 2, Action::Toggle);
        assert!(!run.playtest.open(1) && run.playtest.open(2));
        let before = run.players[&2].clone();
        let host = (run.players[&1].x, run.players[&1].y);
        let all = Input {
            move_x: 1,
            aim_y: -1,
            fire: true,
            dash: true,
            bomb: true,
            swing: true,
            cast: 1,
            play: 1,
            vigil: true,
            ult: true,
            take: true,
            interact: true,
            skill: 1,
            ..Default::default()
        };
        for _ in 0..10 {
            run.step(&BTreeMap::from([
                (
                    1,
                    Input {
                        move_y: -1,
                        ..Default::default()
                    },
                ),
                (2, all),
            ]));
        }
        let after = &run.players[&2];
        assert_eq!(
            (before.x, before.y, before.bombs, before.vigil),
            (after.x, after.y, after.bombs, after.vigil)
        );
        assert_ne!(host, (run.players[&1].x, run.players[&1].y));
        assert!(run.projectiles.iter().all(|p| p.traits.owner != 2));
        command(&mut run, 1, Action::Toggle);
        command(&mut run, 2, Action::Close);
        assert!(run.playtest.open(1) && !run.playtest.open(2));
        command(&mut run, 1, Action::Toggle);
        assert!(!run.playtest.open(1));
    }
}

#[test]
fn ratings_average_excludes_untried_and_commands_are_idempotent() {
    let mut run = Run::new(91, 7, Some("Friend"));
    for seat in [1, 2] {
        command(&mut run, seat, Action::Toggle);
    }
    command(&mut run, 1, Action::Rate(5));
    command(&mut run, 2, Action::Rate(3));
    assert_eq!(run.playtest.average(ITEMS[0].id), (4.0, 2));
    let serial = run.playtest.seats[&2].serial;
    run.vote_command(
        2,
        Command {
            serial,
            action: Action::Right,
        },
    );
    assert_eq!(run.playtest.average(ITEMS[0].id), (4.0, 2));
    command(&mut run, 2, Action::Untried);
    assert_eq!(run.playtest.average(ITEMS[0].id), (5.0, 1));
    command(&mut run, 2, Action::Clear);
    assert!(!run.playtest.records["Friend"].contains_key(ITEMS[0].id));
    command(&mut run, 2, Action::Rate(9));
    assert!(!run.playtest.records["Friend"].contains_key(ITEMS[0].id));
    command(&mut run, 2, Action::Down);
    assert_eq!(run.playtest.seats[&2].selected, 1);
}

#[test]
fn new_player_hint_is_once_per_name_in_a_realm_and_catalogue_ids_are_unique() {
    let mut state = State::default();
    state.arrive(2, "Friend");
    assert_eq!(state.seats[&2].hint, HZ * 7);
    state.seats.clear();
    state.arrive(3, "Friend");
    assert_eq!(state.seats[&3].hint, 0);
    assert_eq!(
        ITEMS.iter().map(|i| i.id).collect::<BTreeSet<_>>().len(),
        ITEMS.len()
    );
    assert!(
        ITEMS
            .iter()
            .all(|i| (1..=3).contains(&i.label.split_whitespace().count()))
    );
}

#[test]
fn ballot_pixels_cover_surface_and_delve_only_for_the_voting_seat() {
    use crate::render::overworld::arena;
    for surface in [false, true] {
        let mut run = Run::new(91, 7, Some("Friend"));
        if surface {
            run.walk_kingdom(None);
        }
        let before = arena::frame_for(&run, 384, 224, Some(2)).rgb_bytes();
        command(&mut run, 2, Action::Toggle);
        let closed = arena::frame_for(&run, 384, 224, Some(1)).rgb_bytes();
        let opened = arena::frame_for(&run, 384, 224, Some(2)).rgb_bytes();
        assert_ne!(opened, before);
        assert_ne!(opened, closed);
        // An opaque panel fills the middle, rather than being hidden by a
        // scene's early return or the world camera crop.
        let middle = (100 * 384 + 100) * 3;
        assert_ne!(opened[middle..middle + 3], before[middle..middle + 3]);
    }
}

#[test]
#[ignore]
fn write_vote_ballot_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let mut run = Run::at_home(
        91,
        7,
        Some("Friend"),
        Default::default(),
        Default::default(),
    );
    run.walk_kingdom(None);
    run.playtest.arrive(1, "You");
    let save = |name: &str, run: &Run, seat| {
        let img = crate::render::overworld::arena::frame_for(run, 768, 448, seat);
        image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.rgba_bytes())
            .unwrap()
            .save(std::path::Path::new(&dir).join(name))
            .unwrap();
    };
    save("new-player-hint.png", &run, Some(1));
    command(&mut run, 1, Action::Toggle);
    save("surface-ballot.png", &run, Some(1));
    command(&mut run, 1, Action::Rate(4));
    command(&mut run, 1, Action::Down);
    command(&mut run, 1, Action::Untried);
    save("mid-rating.png", &run, Some(1));
    command(&mut run, 2, Action::Toggle);
    run.playtest.seats.get_mut(&2).unwrap().couch = true;
    command(&mut run, 2, Action::Rate(2));
    save("party-averages.png", &run, Some(1));
    run.playtest.seats.get_mut(&1).unwrap().selected = 33;
    save("countryside-ballot.png", &run, Some(1));
    command(&mut run, 1, Action::Close);
    save("couch-ballot.png", &run, None);
}
