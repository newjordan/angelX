use super::*;
use crate::rules::crossroads::{Game, Stage, State, VOTE_TIME};
use crate::rules::Input;

fn crossroads() -> Run {
    let mut run = fight();
    run.join_seat(2, "Friend");
    run.floor_plates.clear();
    run.crossroads = Some(State {
        depth: 4, pack: crate::rules::Pack::Archive,
        choices: [Game::Racers, Game::Powder, Game::Shaft], seats: vec![1, 2],
        votes: Default::default(), stage: Stage::Voting, left: VOTE_TIME,
        chosen: None, order: Vec::new(),
    });
    run
}

#[test]
fn crossroads_fallen_browser_seat_votes_with_retry_and_ack_and_never_predicts_movement() {
    let mut run = crossroads();
    run.players.get_mut(&1).unwrap().hp = 0;
    let mut host = Host::new(run);
    let mut c = client(Some(0));
    host.send(&mut c);
    assert!(c.down.is_none());
    assert_eq!(c.choices().lines().count(), 3);
    let cmd = c.command(&tap(button::SLOT2));
    assert_eq!(cmd.pressed.vote.unwrap().action, Action::Rate(2));
    assert_eq!(c.command(&held(button::RIGHT)).pressed.vote, cmd.pressed.vote);
    assert_eq!(cmd.input().move_x, 0);
    host.step(&[(0, cmd)]);
    host.send(&mut c);
    assert!(c.ballot.queue.is_empty());
    assert_eq!(c.newest().unwrap().crossroads.as_ref().unwrap().counts(), [0, 1, 0]);
    assert!(!host.run.predictable(1));
    c.choose(0);
    let cmd = c.command(&Hands::default());
    host.step(&[(0, cmd)]);
    host.send(&mut c);
    assert_eq!(c.newest().unwrap().crossroads.as_ref().unwrap().counts(), [1, 0, 0]);
    let mut frame = Frame::default();
    c.draw(&view(&c, &[], 0, Hands::default(), 0.0), &mut frame);
    assert!(!frame.px.is_empty());
}

#[test]
fn crossroads_browser_arrows_confirm_and_a_fallen_knight_plays_the_winner() {
    let mut host = Host::new(crossroads());
    host.run.players.get_mut(&1).unwrap().hp = 0;
    let mut c = client(Some(0));
    host.send(&mut c);
    let right = c.command(&tap(button::AIM_RIGHT));
    host.step(&[(0, right)]);
    host.send(&mut c);
    let pick = c.command(&tap(button::FIRE));
    assert_eq!(pick.pressed.vote.unwrap().action, Action::Rate(2));
    host.step(&[(0, pick)]);
    host.run.vote_command(2, Vote { serial: 1, action: Action::Rate(2) });
    host.step(&[]);
    host.send(&mut c);
    assert_eq!(c.newest().unwrap().crossroads.as_ref().unwrap().stage, Stage::Playing);
    assert!(c.down.is_none());
    assert!(c.command(&held(button::FIRE)).held.fire);
    assert!(!host.run.predictable(1));
}

#[test]
fn crossroads_confirm_follows_an_arrow_still_waiting_for_the_host() {
    let mut host = Host::new(crossroads());
    let mut c = client(Some(0));
    host.send(&mut c);
    let arrow = c.command(&tap(button::AIM_RIGHT));
    c.command(&tap(button::FIRE));
    host.step(&[(0, arrow)]);
    host.send(&mut c);
    let confirm = c.command(&Hands::default());
    assert_eq!(confirm.pressed.vote.unwrap().action, Action::Rate(2));
    host.step(&[(0, confirm)]);
    host.send(&mut c);
    assert_eq!(host.run.crossroads.as_ref().unwrap().votes.get(&1), Some(&1));
}

#[test]
fn crossroads_deathmatch_predicts_the_fallen_seats_camera_and_sends_a_browser_stage() {
    let mut run = crossroads();
    run.players.get_mut(&1).unwrap().hp = 0;
    run.crossroads.as_mut().unwrap().choices[0] = Game::Deathmatch;
    for id in 1..=2 { run.vote_command(id, Vote { serial: 1, action: Action::Rate(1) }); }
    run.step(&Default::default());
    run.deathmatch.as_mut().unwrap().ticks = crate::rules::deathmatch::TITLE + 1;
    for seat in 0..2 {
        let mut host = Host::new(run.clone());
        let mut c = client(Some(seat));
        host.send(&mut c);
        assert!(c.down.is_none());
        let id = seat + 1;
        let turn = DelveCmd { held: Input { aim_x: 1, ..Default::default() }, pressed: Input::default() };
        let pending = [(1, turn)];
        let before = c.newest().unwrap().deathmatch.as_ref().unwrap().fighters[&id].yaw;
        let predicted = c.deathmatch_own(&view(&c, &pending, 0, Hands::default(), 0.0)).unwrap().1;
        assert_ne!(predicted.yaw, before);
        assert_eq!(c.newest().unwrap().deathmatch.as_ref().unwrap().fighters[&id].yaw, before,
            "prediction cannot mutate host state");
        assert!(c.own(&view(&c, &pending, 0, Hands::default(), 0.0)).is_none(),
            "dungeon body is not predicted during the interlude");
        assert!(c.command(&held(button::FIRE)).held.fire);
        let mut out = Vec::new();
        let _ = c.stage(1000.0, &mut out);
        c.draw(&view(&c, &pending, 0, Hands::default(), 0.0), &mut Frame::default());
        assert!(c.stage(1000.0, &mut out));
        assert!(angel_stage::wire::decode(&out).is_ok());
    }
}

#[test]
fn team_key_does_not_vote_as_enter_in_crossroads() {
    let mut host=Host::new(crossroads());
    let mut c=client(Some(0)); host.send(&mut c);
    let bits=angel_net::input::button::named("TEAM").unwrap();
    assert!(c.command(&Hands{held:bits,pressed:bits,..Hands::default()}).pressed.vote.is_none());
}
