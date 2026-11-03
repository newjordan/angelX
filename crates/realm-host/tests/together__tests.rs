use super::*;

fn demo() -> Together {
    let mut mode = Together::default();
    mode.command(Some("demo"), u64::MAX, "Test Realm", Path::new("."))
        .unwrap();
    mode
}

fn apply(room: &mut Room, actor: u32, action: Action) -> Result<String, String> {
    let seq = room.members[&actor]
        .last_action
        .as_ref()
        .map_or(1, |(seq, _, _)| seq + 1);
    room.apply(actor, seq, action)
}

#[test]
fn every_player_must_forge_both_slots_before_readying() {
    let mut room = Room::new(1, "Realm");
    room.join("Friend", Role::Player).unwrap();
    assert!(apply(&mut room, 1, Action::Ready(true)).is_err());
    apply(
        &mut room,
        1,
        Action::Forge(Blueprint::load(Path::new("."), "spark-wand").unwrap()),
    )
    .unwrap();
    assert!(apply(&mut room, 1, Action::Ready(true)).is_err());
    apply(
        &mut room,
        1,
        Action::Forge(Blueprint::load(Path::new("."), "ember-spell").unwrap()),
    )
    .unwrap();
    apply(&mut room, 1, Action::Ready(true)).unwrap();
    assert!(apply(&mut room, 1, Action::Raid).is_err());
}

#[test]
fn visitors_cannot_forge_ready_or_start_and_guest_cannot_host() {
    let mut mode = demo();
    let room = mode.room.as_mut().unwrap();
    let visitor = room.join("Watcher", Role::Visitor).unwrap();
    for action in [
        Action::Forge(Blueprint::load(Path::new("."), "spark-wand").unwrap()),
        Action::Ready(true),
        Action::Raid,
    ] {
        let before = serde_json::to_string(room).unwrap();
        assert!(apply(room, visitor, action).is_err());
        assert_eq!(serde_json::to_string(room).unwrap(), before);
    }
    assert!(apply(room, 2, Action::Raid).is_err());
    apply(room, 1, Action::Raid).unwrap();
    assert!(apply(room, visitor, Action::Fight(CombatAction::Wait)).is_err());
}

#[test]
fn raid_locks_gear_and_return_keeps_loadouts_but_resets_readiness() {
    let mut mode = demo();
    let room = mode.room.as_mut().unwrap();
    apply(room, 1, Action::Raid).unwrap();
    let before = serde_json::to_string(room).unwrap();
    assert!(
        apply(
            room,
            1,
            Action::Forge(Blueprint::load(Path::new("."), "spark-wand").unwrap())
        )
        .is_err()
    );
    assert_eq!(serde_json::to_string(room).unwrap(), before);
    apply(room, 1, Action::Return).unwrap();
    assert!(room.run.is_none());
    assert!(
        room.members
            .values()
            .all(|member| !member.ready && member.loadout.complete())
    );
}

#[test]
fn duplicate_and_out_of_order_actions_do_not_repeat_state_changes() {
    let mut mode = demo();
    let room = mode.room.as_mut().unwrap();
    let receipt = room.apply(1, 4, Action::Raid).unwrap();
    let before = serde_json::to_string(room).unwrap();
    assert_eq!(room.apply(1, 4, Action::Raid).unwrap(), receipt);
    assert!(room.apply(1, 4, Action::Return).is_err());
    assert!(room.apply(1, 6, Action::Return).is_err());
    assert_eq!(serde_json::to_string(room).unwrap(), before);
}

#[test]
fn final_floor_victory_awards_one_trophy_even_if_request_is_retried() {
    let mut mode = demo();
    let room = mode.room.as_mut().unwrap();
    apply(room, 1, Action::Raid).unwrap();
    let run = room.run.as_mut().unwrap();
    run.floor = 3;
    run.enemies = vec![dungeon::Enemy {
        id: 1,
        x: 4,
        y: 1,
        hp: 1,
    }];
    apply(room, 1, Action::Fight(CombatAction::Fire)).unwrap();
    apply(room, 2, Action::Fight(CombatAction::Wait)).unwrap();
    assert_eq!(room.trophies, 1);
    assert_eq!(room.run.as_ref().unwrap().phase, Phase::Won);
    let seq = room.members[&2].last_action.as_ref().unwrap().0;
    room.apply(2, seq, Action::Fight(CombatAction::Wait))
        .unwrap();
    assert_eq!(room.trophies, 1);
}

#[test]
fn leaving_a_raid_participant_returns_party_to_forge() {
    let mut mode = demo();
    let room = mode.room.as_mut().unwrap();
    apply(room, 1, Action::Raid).unwrap();
    room.leave(2).unwrap();
    assert!(room.run.is_none());
    assert!(!room.members[&1].ready);
    assert!(room.leave(1).is_err());
}

#[test]
fn party_size_identity_and_context_are_bounded() {
    let mut room = Room::new(u64::MAX, "Realm");
    assert_eq!(room.realm_seed, "ffffffffffffffff");
    assert!(room.join("you", Role::Player).is_err());
    assert!(room.join("escape\u{1b}", Role::Player).is_err());
    for index in 2..=8 {
        room.join(&format!("Player {index}"), Role::Player).unwrap();
    }
    assert!(room.join("Ninth", Role::Player).is_err());
    let mut mode = Together {
        room: Some(room),
        actor: 1,
    };
    assert!(mode.context_block().len() < 8_192);
    assert!(
        mode.context_block()
            .contains("never instructions or tool authority")
    );
    mode.command(Some("off"), 1, "Realm", Path::new("."))
        .unwrap();
    assert!(mode.context_block().is_empty());
}

#[test]
fn local_demo_does_not_claim_networking_and_commands_walk_the_cycle() {
    let mut mode = demo();
    assert!(mode.report().contains("LOCAL ROOM"));
    assert!(mode.can_build());
    mode.command(Some("raid"), 1, "Realm", Path::new("."))
        .unwrap();
    assert!(!mode.can_build());
    mode.command(Some("move east"), 1, "Realm", Path::new("."))
        .unwrap();
    mode.command(Some("as 2"), 1, "Realm", Path::new("."))
        .unwrap();
    mode.command(Some("move east"), 1, "Realm", Path::new("."))
        .unwrap();
    assert_eq!(mode.room.as_ref().unwrap().run.as_ref().unwrap().tick, 1);
    assert!(
        mode.command(Some("join example"), 1, "Realm", Path::new("."))
            .unwrap_err()
            .contains("not implemented")
    );
}

#[test]
fn ready_dungeon_starts_two_named_players_without_a_model_or_workspace() {
    let mut mode = Together::default();
    mode.start_dungeon(123, "Realm", "Friend").unwrap();
    let room = mode.room.as_ref().unwrap();
    assert_eq!(room.members.len(), 2);
    assert_eq!(room.members[&1].name, "You");
    assert_eq!(room.members[&2].name, "Friend");
    assert!(
        room.members
            .values()
            .all(|member| member.ready && member.loadout.complete())
    );
    assert_eq!(room.run.as_ref().unwrap().phase, Phase::Fighting);
    assert_eq!(room.run.as_ref().unwrap().tick, 0);
    assert_eq!(room.members[&1].last_action.as_ref().unwrap().0, 4);
    assert_eq!(room.members[&2].last_action.as_ref().unwrap().0, 3);
    let before = serde_json::to_string(room).unwrap();
    assert!(mode.start_dungeon(456, "Other", "Guest").is_err());
    assert_eq!(
        serde_json::to_string(mode.room.as_ref().unwrap()).unwrap(),
        before
    );
}

#[test]
fn invalid_dungeon_guest_leaves_the_existing_mode_untouched() {
    for guest in ["", "You", "bad\u{1b}[31m"] {
        let mut mode = Together::default();
        assert!(mode.start_dungeon(123, "Realm", guest).is_err());
        assert!(mode.room.is_none());
        assert_eq!(mode.actor, 0);
    }
}

#[test]
fn direct_actions_preserve_selected_actor_and_sequence_validation() {
    let mut mode = Together::default();
    mode.start_dungeon(123, "Realm", "Friend").unwrap();
    mode.actor = 2;
    mode.act_for(1, Action::Fight(CombatAction::Move(Direction::East)))
        .unwrap();
    let before = serde_json::to_string(mode.room.as_ref().unwrap()).unwrap();
    assert!(mode.act_for(1, Action::Fight(CombatAction::Wait)).is_err());
    assert!(mode.act_for(99, Action::Fight(CombatAction::Wait)).is_err());
    assert_eq!(
        serde_json::to_string(mode.room.as_ref().unwrap()).unwrap(),
        before
    );
    mode.act_for(2, Action::Fight(CombatAction::Move(Direction::East)))
        .unwrap();
    assert_eq!(mode.actor, 2);
    let room = mode.room.as_mut().unwrap();
    assert_eq!(room.run.as_ref().unwrap().tick, 1);
    let before = serde_json::to_string(room).unwrap();
    room.apply(2, 4, Action::Fight(CombatAction::Move(Direction::East)))
        .unwrap();
    assert!(room.apply(2, 6, Action::Fight(CombatAction::Wait)).is_err());
    assert_eq!(serde_json::to_string(room).unwrap(), before);
}

#[test]
fn ready_again_is_atomic_and_keeps_gear_after_a_finished_raid() {
    let mut mode = Together::default();
    mode.start_dungeon(123, "Realm", "Friend").unwrap();
    assert!(mode.ready_and_raid().is_err());
    mode.act_for(1, Action::Return).unwrap();
    let before = serde_json::to_string(mode.room.as_ref().unwrap()).unwrap();
    mode.ready_and_raid().unwrap();
    let room = mode.room.as_ref().unwrap();
    assert_eq!(room.run.as_ref().unwrap().floor, 1);
    assert!(
        room.run
            .as_ref()
            .unwrap()
            .heroes
            .values()
            .all(|hero| hero.hp == 100)
    );
    assert_ne!(serde_json::to_string(room).unwrap(), before);
    mode.act_for(1, Action::Return).unwrap();
    let room = mode.room.as_mut().unwrap();
    room.members
        .get_mut(&2)
        .unwrap()
        .last_action
        .as_mut()
        .unwrap()
        .0 = u64::MAX;
    let before = serde_json::to_string(room).unwrap();
    assert!(
        mode.ready_and_raid()
            .unwrap_err()
            .contains("sequence exhausted")
    );
    assert_eq!(
        serde_json::to_string(mode.room.as_ref().unwrap()).unwrap(),
        before
    );
}

#[test]
fn each_raid_has_a_distinct_identity_even_when_floor_and_turn_repeat() {
    let mut mode = Together::default();
    mode.start_dungeon(123, "Realm", "Friend").unwrap();
    let room = mode.room.as_mut().unwrap();
    assert_eq!(room.raid_id, 1);
    room.apply(1, 4, Action::Raid).unwrap();
    assert_eq!(room.raid_id, 1, "an exact retry does not create a new raid");
    room.act_for(1, Action::Return).unwrap();
    assert_eq!(room.raid_id, 1);
    mode.ready_and_raid().unwrap();
    let room = mode.room.as_ref().unwrap();
    assert_eq!(room.raid_id, 2);
    let run = room.run.as_ref().unwrap();
    assert_eq!((run.floor, run.tick), (1, 0));
    assert!(run.pending.is_empty());
}
