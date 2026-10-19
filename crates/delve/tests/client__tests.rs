use super::*;
use crate::render::overworld::arena::art_v2::testing::fight;
use crate::render::overworld::kingdom as geography;
use crate::rules::{home, world};
use angel_net::doc::delta::diff;
use angel_net::doc::wire::Writer;
use angel_net::proto::PROTOCOL;

/// The host's half, for feeding a client: a run, and what one client was
/// last sent of it.
struct Host {
    run: Run,
    names: Writer,
    last: Option<Value>,
    tick: u32,
}

impl Host {
    fn new(run: Run) -> Host {
        Host { run, names: Writer::default(), last: None, tick: 0 }
    }

    /// The next snapshot for `seat`: (tick, whole, body).
    fn snapshot(&mut self, seat: Option<u32>) -> (u32, bool, Vec<u8>) {
        self.tick += 1;
        let tree = wire::publish(&self.run).unwrap();
        let view = wire::seat_tree(&tree, seat).into_owned();
        let mut out = Vec::new();
        let whole = match &self.last {
            None => {
                self.names.value(&mut out, &view);
                true
            }
            Some(old) => {
                if let Some(patch) = diff(old, &view) {
                    self.names.patch(&mut out, &patch);
                }
                false
            }
        };
        self.last = Some(view);
        (self.tick, whole, out)
    }

    fn send(&mut self, client: &mut DelveClient) {
        let (tick, whole, body) = self.snapshot(client.seat());
        client.snapshot(tick, whole, &body).expect("the snapshot reads");
    }

    fn step(&mut self, inputs: &[(u32, DelveCmd)]) {
        let inputs = inputs.iter().map(|&(seat, cmd)| (wire::knight_of(seat), cmd.input())).collect();
        self.run.step(&inputs);
    }
}

fn welcome(seat: Option<u32>, params: &[u8]) -> Welcome<'_> {
    Welcome { protocol: PROTOCOL, game: wire::GAME, seat, seat_token: "tok", tick_hz: HZ, snapshot_hz: HZ, tick: 0, params }
}

fn client(seat: Option<u32>) -> DelveClient {
    DelveClient::boot(&welcome(seat, &Params::default().encode())).unwrap()
}

fn view<'a>(c: &DelveClient, pending: &'a [(u32, DelveCmd)], ack: u32, hands: Hands, partial: f32) -> View<'a, DelveCmd> {
    let newest = c.newest_tick().unwrap_or(0);
    View {
        render_tick: f64::from(newest),
        newest_tick: newest,
        ack,
        pending,
        partial,
        hands,
        seat: c.seat(),
        now_ms: 1000.0,
        correct_ms: angel_net::clock::CORRECT_MS,
    }
}

fn held(bits: u32) -> Hands {
    let (x, y) = Hands::stick_from_buttons(bits);
    let (aim_x, aim_y) = Hands::aim_from_buttons(bits);
    Hands { held: bits, pressed: 0, x, y, aim_x, aim_y }
}

fn tap(bits: u32) -> Hands {
    Hands { pressed: bits, ..Hands::default() }
}

#[test]
fn the_welcome_sets_the_art_and_a_stale_page_is_turned_away() {
    let c = DelveClient::boot(&welcome(Some(1), &Params { art_v2: false, world3d: false, deep3d: false }.encode())).unwrap();
    // The party's band over the room's view.
    assert_eq!((c.size(), c.seat()), ((384, 224 + (arena::BAND_H + arena::HUD_H) as u32), Some(1)));
    assert!(DelveClient::boot(&welcome(Some(1), &[9, 0])).is_err());
    assert!(DelveClient::boot(&welcome(Some(1), &[])).is_err());
}

#[test]
fn snapshots_are_patched_read_back_and_kept() {
    let mut host = Host::new(fight());
    let mut c = client(Some(0));
    host.send(&mut c);
    assert_eq!(c.newest().unwrap().tick, host.run.tick);
    for _ in 0..12 {
        host.step(&[]);
        host.send(&mut c);
        assert_eq!(c.newest().unwrap().tick, host.run.tick);
        assert_eq!(c.newest().unwrap().projectiles.len(), host.run.projectiles.len());
    }
    assert_eq!(c.kept.len(), KEEP);
    // Nothing changed: an empty body moves the newest tick on.
    let (tick, whole, body) = host.snapshot(Some(0));
    assert!(!whole && body.is_empty());
    c.snapshot(tick, false, &body).unwrap();
    assert_eq!(c.newest_tick(), Some(tick));
    // A patch that does not fit, or bytes that are not one, are refused
    // and leave the run held as it was.
    let held = c.newest().unwrap().tick;
    assert!(c.snapshot(tick + 1, false, &[0x7f, 1, 2]).is_err());
    assert!(c.snapshot(tick + 1, true, &[]).is_err());
    assert_eq!(c.newest().unwrap().tick, held);
}

#[test]
fn wish_notices_follow_authoritative_windows_for_each_seat_once() {
    let mut host = Host::new(fight());
    for id in 2..=4 { host.run.join_seat(id, &format!("Knight {id}")); }
    let mut clients: Vec<_> = (0..4).map(|seat| (Host::new(host.run.clone()), client(Some(seat)))).collect();
    for (h, c) in &mut clients { h.send(c); }
    host.run.clear_for_test();
    // Wishes are given (a new floor, a floor's boss fell): what the notices
    // follow, wherever they came from.
    for hero in host.run.players.values_mut() {
        hero.wishes = 0;
    }
    host.run.give_wishes(1, 0);
    let read = |h: &mut Host, c: &mut DelveClient, run: &Run| {
        h.run = run.clone();
        h.send(c);
        let mut lines = Vec::new();
        c.lines(&mut lines);
        lines.into_iter().filter(|l| l.starts_with("notice ")).collect::<Vec<_>>()
    };
    for (h, c) in &mut clients {
        assert_eq!(read(h, c, &host.run), ["notice Wishes given. Press T."]);
        assert!(read(h, c, &host.run).is_empty());
    }
    // Learning also spends a wish before a card is granted.
    host.run.spend_wish(4);
    for (index, (h, c)) in clients.iter_mut().enumerate() {
        let lines = read(h, c, &host.run);
        assert_eq!(lines.len(), usize::from(index == 3));
        if index == 3 { assert_eq!(lines, ["notice Wish spent: none held."]); }
        assert!(read(h, c, &host.run).is_empty());
    }
    // A fall/revival and the next fight are not wish spending or earning.
    host.run.players.get_mut(&1).unwrap().hp = 0;
    for (h, c) in &mut clients { assert!(read(h, c, &host.run).is_empty()); }
    host.run.players.get_mut(&1).unwrap().hp = 3;
    host.run.window = None;
    for (h, c) in &mut clients { assert!(read(h, c, &host.run).is_empty()); }
}

#[test]
fn the_keys_make_the_old_seats_inputs() {
    let mut host = Host::new(fight());
    let mut c = client(Some(0));
    host.send(&mut c);
    let walk = c.command(&held(button::UP | button::RIGHT | button::AIM_LEFT | button::FIRE));
    assert_eq!((walk.held.move_x, walk.held.move_y, walk.held.aim_x, walk.held.aim_y), (1, -1, -1, 0));
    assert!(walk.held.fire && !walk.held.dash);
    // Shift with a walk dashes; Space dashes standing.
    assert!(c.command(&held(button::SHIFT | button::LEFT)).held.dash);
    assert!(!c.command(&held(button::SHIFT)).held.dash);
    assert!(c.command(&held(button::JUMP)).held.dash);
    // E meets the world and takes gear; the rest of the keys.
    let e = c.command(&held(button::USE));
    assert!(e.held.interact && e.held.take);
    let keys = c.command(&held(key::BOMB | key::DRINK | key::VIGIL | key::ULT | button::ALT | button::SLOT3 | key::SKILLS[4]));
    let i = keys.held;
    assert!(i.bomb && i.vigil && i.ult && i.swing);
    assert_eq!((i.play, i.cast, i.skill), (1, 3, 5));
    // A tap shorter than a tick is in the command's presses.
    let t = c.command(&tap(button::FIRE | key::SKILLS[1]));
    assert!(!t.held.fire && t.pressed.fire);
    assert_eq!((t.pressed.skill, t.input().skill), (2, 2));
    assert!(t.input().fire);
    // A pad's stick leaning a little is no walk; past the lean it is.
    let mut lean = |x: i8| c.command(&Hands { x, ..Hands::default() }).held.move_x;
    assert_eq!((lean(30), lean(-90), lean(127)), (0, -1, 1));
    // Every command reads back on the server's side.
    for cmd in [walk, e, keys, t] {
        let mut b = Vec::new();
        DelveClient::encode_cmd(&cmd, &mut b);
        assert_eq!(DelveCmd::decode(&b), Some(cmd));
    }
}

#[test]
fn the_ballot_takes_the_keys_until_it_shuts() {
    let mut host = Host::new(fight());
    let mut c = client(Some(0));
    host.send(&mut c);
    let open = c.command(&tap(key::BALLOT));
    let vote = open.pressed.vote.expect("the toggle goes out");
    assert_eq!(vote.action, Action::Toggle);
    assert!(c.ballot_open(), "open at once on this screen");
    // Until the run takes it, it is sent again, and nothing else is.
    let again = c.command(&held(button::RIGHT | button::FIRE));
    assert_eq!(again.pressed.vote, Some(vote));
    assert_eq!(again.input().move_x, 0);
    host.step(&[(0, again)]);
    host.send(&mut c);
    assert!(host.run.playtest.open(1) && c.ballot_open());
    // In the ballot the arrows choose and the digits rate.
    let down = c.command(&tap(button::AIM_DOWN));
    assert_eq!(down.pressed.vote.map(|v| v.action), Some(Action::Down));
    host.step(&[(0, down)]);
    host.send(&mut c);
    let rate = c.command(&tap(key::RATE5));
    assert_eq!(rate.pressed.vote.map(|v| v.action), Some(Action::Rate(5)));
    host.step(&[(0, rate)]);
    host.send(&mut c);
    // Esc shuts it, and the keys are the game's again.
    let close = c.command(&tap(button::MENU));
    assert_eq!(close.pressed.vote.map(|v| v.action), Some(Action::Close));
    assert!(!c.ballot_open());
    host.step(&[(0, close)]);
    host.send(&mut c);
    assert!(!host.run.playtest.open(1));
    assert_eq!(c.command(&held(button::RIGHT)).held.move_x, 1);
}

#[test]
fn the_map_key_held_lays_the_floor_over_this_seats_screen_and_nothing_else() {
    let mut host = Host::new(fight());
    let mut c = client(Some(0)).exact();
    host.send(&mut c);
    host.step(&[]);
    host.send(&mut c);
    let draw = |c: &mut DelveClient, hands: Hands| {
        let mut frame = Frame::default();
        c.draw(&view(c, &[], 0, hands, 0.0), &mut frame);
        frame.px
    };
    let plain = draw(&mut c, Hands::default());
    assert!(!c.map_open());
    // Held, the whole floor is over the view.
    let up = draw(&mut c, held(key::MAP_KEY));
    assert!(c.map_open() && up != plain);
    // And the key is no one else's business: the command is as with no key.
    assert_eq!(c.command(&held(key::MAP_KEY)), c.command(&held(0)));
    // Let go, the view is as it was.
    assert!(draw(&mut c, Hands::default()) == plain && !c.map_open());
    // The ballot has the key for "not tried yet": no map behind it.
    let open = c.command(&tap(key::BALLOT));
    host.step(&[(0, open)]);
    host.send(&mut c);
    assert!(c.ballot_open());
    draw(&mut c, held(key::MAP_KEY));
    assert!(!c.map_open());
}

#[test]
fn a_press_during_the_floors_plate_dismisses_it_and_does_nothing_else() {
    let mut run = fight();
    let plate = crate::rules::floor_plates::Playing { kind: crate::rules::floor_plates::Kind::Crypt, depth: 1, age: 0, results: 0 };
    run.floor_plates.insert(1, plate);
    let mut host = Host::new(run);
    let mut c = client(Some(0));
    host.send(&mut c);
    assert!(c.newest().unwrap().floor_plate(Some(1)).is_some());
    let held_still = c.command(&held(button::RIGHT));
    assert_eq!(held_still, DelveCmd::neutral(), "a key held into the plate does nothing");
    let skip = c.command(&tap(button::FIRE));
    assert!(skip.pressed.skip_plate && !skip.input().fire);
    host.step(&[(0, skip)]);
    host.send(&mut c);
    assert!(host.run.floor_plate(Some(1)).is_none());
    assert!(!c.command(&held(0)).pressed.skip_plate, "asked once it is gone");
}

/// The party at home, in the Forest, a game a knight may pause or leave.
fn forest() -> Run {
    let mut home = home::Home::default();
    home.levels.insert(home::Station::Wing, 1);
    let mut r = Run::at_home(41, 1, None, home, Default::default());
    r.walk_kingdom(Some(geography::keep_stand()));
    r.floor_plates.clear();
    let geo = geography::geography(&r.kingdom.wishes);
    if let Some(site) = geo.sites.iter().position(|s| s.room == RoomKind::Forest) {
        r.enter_kingdom_place(site);
    }
    if r.room().kind != RoomKind::Forest {
        let at = world::room_of(&r.dungeon, RoomKind::Forest).unwrap();
        r.arrive(at, (12.0, 10.5));
    }
    r.floor_plates.clear();
    r
}

#[test]
fn escape_pauses_a_rooms_game_and_enter_leaves_it() {
    let mut host = Host::new(forest());
    assert!(host.run.can_leave_activity(1));
    let mut c = client(Some(0));
    host.send(&mut c);
    let pause = c.command(&tap(button::MENU));
    assert!(c.menu_open() && pause.held.activity_menu);
    assert!(!pause.input().fire && pause.input().move_x == 0);
    // Paused, the knight waits; Esc again resumes.
    assert!(c.command(&held(button::RIGHT)).held.activity_menu);
    c.command(&tap(button::MENU));
    assert!(!c.menu_open());
    // Enter leaves, and asks until the run has the knight out.
    let leave = c.command(&tap(key::LEAVE));
    assert!(leave.pressed.leave_activity);
    host.step(&[(0, leave)]);
    host.send(&mut c);
    assert!(!host.run.can_leave_activity(1));
    assert!(!c.command(&held(0)).pressed.leave_activity);
}

#[test]
fn the_own_knight_walks_on_the_next_frame_ahead_of_the_server() {
    let mut host = Host::new(Run::new(7, 1, Some("Friend")));
    let mut c = client(Some(0));
    host.send(&mut c);
    for _ in 0..3 {
        host.step(&[]);
        host.send(&mut c);
    }
    let mut frame = Frame::default();
    let server = {
        let h = &c.newest().unwrap().players[&1];
        (h.x, h.y)
    };
    c.draw(&view(&c, &[], 0, Hands::default(), 0.0), &mut frame);
    assert_eq!((frame.w, frame.h), (384, 224 + (arena::BAND_H + arena::HUD_H) as u32));
    assert_eq!(c.drawn_at(), Some(server), "standing, drawn where the server has it");
    // D held, no command sampled yet: a part of a tick walked at once.
    c.draw(&view(&c, &[], 0, held(button::RIGHT), 0.3), &mut frame);
    let (x, _) = c.drawn_at().unwrap();
    assert!(x > server.0, "the next frame shows the key");
    // Two commands the server has not answered: two ticks on, then more.
    let right = DelveCmd { held: Input { move_x: 1, ..Input::default() }, pressed: Input::default() };
    let pending = [(1, right), (2, right)];
    c.draw(&view(&c, &pending, 0, held(button::RIGHT), 0.0), &mut frame);
    let (two, _) = c.drawn_at().unwrap();
    c.draw(&view(&c, &pending, 0, held(button::RIGHT), 0.5), &mut frame);
    let (more, _) = c.drawn_at().unwrap();
    assert!(two > x && more > two, "{server:?}: a third of a tick {x}, two ticks {two}, and a half {more}");
    // The newest run held is the server's word still.
    assert_eq!(c.newest().unwrap().players[&1].x, server.0);
    // The server takes them and agrees: nothing to correct.
    host.step(&[(0, right)]);
    host.send(&mut c);
    host.step(&[(0, right)]);
    host.send(&mut c);
    c.draw(&view(&c, &[], 2, Hands::default(), 0.0), &mut frame);
    let (x, _) = c.drawn_at().unwrap();
    assert!((x - two).abs() < 1e-3, "drawn {x}, predicted {two}");
}

#[test]
fn a_frame_is_the_seats_own_view_at_its_art_size() {
    let mut host = Host::new(fight());
    let mut c = client(Some(0)).exact();
    host.send(&mut c);
    host.step(&[]);
    host.send(&mut c);
    let mut frame = Frame::default();
    c.draw(&view(&c, &[], 0, Hands::default(), 0.0), &mut frame);
    let direct = arena::seat_frame(c.newest().unwrap(), 384, 224 + arena::BAND_H + arena::HUD_H, Some(1));
    let paper = |w: u32| if w >> 24 == 0 { 0xff00_0000 } else { w };
    assert!(frame.px.iter().zip(direct.words()).all(|(&a, &b)| a == paper(b)), "the newest tick, drawn as the renderer draws it");
    assert!(frame.px.iter().all(|&p| p >> 24 == 0xff), "opaque");
    let mut lines = Vec::new();
    c.lines(&mut lines);
    let music = c.newest().unwrap().music().map_or("music".to_string(), |m| format!("music music-{m}"));
    assert_eq!(lines, vec![music]);
    lines.clear();
    c.lines(&mut lines);
    assert!(lines.is_empty(), "said once");
}

#[test]
fn the_band_holds_a_card_for_every_knight_wherever_they_stand() {
    let mut run = fight();
    run.join_seat(2, "Wren");
    run.join_seat(3, "Bors");
    let other = (0..run.dungeon.rooms.len()).find(|&i| i != run.at).unwrap();
    for id in [2, 3] {
        let entry = crate::rules::exits::Entry { at: other, party: std::collections::BTreeMap::from([(id, (1.0, 1.0))]) };
        run.exits.outside.insert(id, entry);
    }
    let mut host = Host::new(run);
    let mut c = client(Some(0)).exact();
    host.send(&mut c);
    host.step(&[]);
    host.send(&mut c);
    let mut frame = Frame::default();
    c.draw(&view(&c, &[], 0, Hands::default(), 0.0), &mut frame);
    let newest = c.newest().unwrap();
    assert!(newest.view_for_seat(1).unwrap().players.len() < newest.players.len(), "the seat's view leaves the others out");
    let (w, h) = (384, 224 + arena::BAND_H + arena::HUD_H);
    let paper = |w: u32| if w >> 24 == 0 { 0xff00_0000 } else { w };
    let rows = (arena::band_rows(h) as usize) * w as usize;
    // The band is the whole party's, as it is drawn from the run in full.
    let party = arena::seat_frame(newest, w, h, Some(1));
    assert!(frame.px[..rows].iter().zip(party.words()).all(|(&a, b)| a == paper(*b)), "a card for each of the three knights");
    let alone = arena::seat_frame(&newest.view_for_seat(1).unwrap(), w, h, Some(1));
    assert!(frame.px[..rows].iter().zip(alone.words()).any(|(&a, b)| a != paper(*b)), "not the view's own band");
}

#[test]
fn events_become_the_shells_lines_and_rapid_ones_are_thinned() {
    let mut host = Host::new(fight());
    let mut c = client(Some(0));
    host.send(&mut c);
    let shot = Event::Sound { hear: wire::Hear::Party, name: "shot_bow".into() }.to_vec();
    let grunt = Event::Sound { hear: wire::Hear::Party, name: "knights/a_oof_1".into() }.to_vec();
    let other = Event::Sound { hear: wire::Hear::Knight(2), name: "gold".into() }.to_vec();
    let say = Event::Say { hear: wire::Hear::All, who: "blaise".into(), name: "Old Blaise".into(), words: "Mind the stair.".into() }.to_vec();
    let mut lines = Vec::new();
    c.events(10, &[&shot, &grunt, &other, &say, b"\xff"], &mut lines);
    assert_eq!(lines, vec!["sfx shot_bow", "sfx knights-a_oof_1", "say Old Blaise: Mind the stair."]);
    lines.clear();
    // The bow again a tick later is under its 70 ms gap; three ticks on it plays.
    c.events(11, &[&shot], &mut lines);
    c.events(13, &[&shot], &mut lines);
    assert_eq!(lines, vec!["sfx shot_bow"]);
}


#[test]
fn a_tuned_pace_reaches_the_client_and_its_prediction_walks_the_servers_steps() {
    let mut r = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    r.walk_kingdom(Some(geography::keep_stand()));
    let mut host = Host::new(r);
    let mut c = client(Some(1));
    host.send(&mut c);
    // The host moves the walking pace: the next tick's patch carries it.
    host.run.tune.hero_speed = 21.0;
    host.step(&[]);
    host.send(&mut c);
    assert_eq!(c.newest().unwrap().tune.hero_speed, 21.0);
    let right = DelveCmd { held: Input { move_x: 1, ..Input::default() }, pressed: Input::default() };
    let mut frame = Frame::default();
    let mut sent = Vec::new();
    for seq in 1..=12u32 {
        sent.push((seq, right));
        if sent.len() > 2 {
            let (s, cmd) = sent[sent.len() - 3];
            host.step(&[(1, cmd)]);
            host.send(&mut c);
            let pending: Vec<_> = sent.iter().copied().filter(|(q, _)| *q > s).collect();
            c.draw(&view(&c, &pending, s, Hands::default(), 0.0), &mut frame);
        }
    }
    let mut counts = Vec::new();
    c.dev_stats(&mut counts);
    assert_eq!(counts[0], ("corrections", 0.0), "the server walked where the client drew");
    // And it is the tuned pace: two ticks ahead of the server, at 21 a second
    // doubled on the road.
    let server = c.newest().unwrap().players[&2].x;
    let drawn = c.drawn_at().unwrap().0;
    let road = host.run.tune.road_pace;
    assert!((drawn - server - 2.0 * 21.0 * road * (1.0 / HZ as f32)).abs() < 1e-3 || (drawn - server - 2.0 * 21.0 * (1.0 / HZ as f32)).abs() < 1e-3, "{server} {drawn}");
}

/// One run of a client against a host over a link of some shape: the own
/// knight as it was drawn on each 120 Hz display frame, and what the host
/// and client did. Keys: `keys(t_ms)` is what is held at `t_ms`.
struct Rolled {
    /// (t_ms, drawn x, drawn y, rolling as drawn, keys held)
    frames: Vec<(f64, f32, f32, bool, u32)>,
    /// The largest correction the client eased in.
    worst_correction: f32,
    /// Ticks the host's knight was rolling.
    host_rolled: u32,
    counts: angel_net::cmd::Counts,
}

fn roll_through(secs: f64, keys: impl Fn(f64) -> u32, link: impl Fn(u32) -> f64) -> Rolled {
    use angel_net::cmd::SeatInputs;
    use angel_net::input::Sampler;
    let step = 1000.0 / f64::from(HZ);
    let mut run = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    // At the head of a straight stretch of road, as the Keep's street
    // once was: the knight trots, and a roll keeps that pace.
    let geo = geography::geography(&[]);
    let floor = |x: i32, y: i32| geo.room.tile(x, y) == crate::rules::Tile::Floor;
    let start = (2..geo.room.rows as i32 - 2)
        .find_map(|y| {
            (2..geo.room.cols as i32 - 26)
                .find(|&x| (x..x + 24).all(|x| geography::is_road(&geo, (x, y)) && (y - 1..=y + 1).all(|y| floor(x, y))))
                .map(|x| (x, y))
        })
        .expect("a straight stretch of road");
    run.walk_kingdom(Some(start));
    let mut host = Host::new(run);
    let mut c = client(Some(0));
    let mut seat: SeatInputs<DelveCmd> = SeatInputs::default();
    let mut sampler = Sampler::default();
    // In flight: (arrives at, seq, command) to the host; (arrives at,
    // tick, whole, body, ack) to the client. One TCP stream each way.
    let mut up: VecDeque<(f64, u32, DelveCmd)> = VecDeque::new();
    let mut down: VecDeque<(f64, u32, bool, Vec<u8>, u32)> = VecDeque::new();
    let mut pending: Vec<(u32, DelveCmd)> = Vec::new();
    let (mut seq, mut ack, mut last_cmd) = (0u32, 0u32, 0.0f64);
    let mut out = Rolled { frames: Vec::new(), worst_correction: 0.0, host_rolled: 0, counts: Default::default() };
    let (mut next_cmd, mut next_tick, mut t) = (0.0, 7.0, 0.0);
    let mut held = 0u32;
    // The first whole snapshot, as a join sends it.
    let (tick, whole, body) = host.snapshot(Some(0));
    c.snapshot(tick, whole, &body).unwrap();
    while t < secs * 1000.0 {
        // The keys.
        let now_keys = keys(t);
        if now_keys != held {
            sampler.down(now_keys & !held);
            sampler.up(held & !now_keys);
            held = now_keys;
        }
        // The client's command tick.
        while next_cmd <= t {
            let hands = sampler.sample();
            let cmd = c.command(&hands);
            seq += 1;
            pending.push((seq, cmd));
            let at = (next_cmd + link(seq)).max(up.back().map_or(0.0, |u| u.0));
            up.push_back((at, seq, cmd));
            last_cmd = next_cmd;
            next_cmd += step;
        }
        // The host's tick.
        while next_tick <= t {
            while up.front().is_some_and(|u| u.0 <= next_tick) {
                let (_, s, cmd) = up.pop_front().unwrap();
                seat.push(s, cmd);
            }
            let (cmd, acked) = seat.take();
            host.step(&[(0, cmd)]);
            out.host_rolled += u32::from(host.run.players[&1].rolling().is_some());
            let (tick, whole, body) = host.snapshot(Some(0));
            let at = (next_tick + link(tick + 10_000)).max(down.back().map_or(0.0, |d| d.0));
            down.push_back((at, tick, whole, body, acked));
            next_tick += step;
        }
        // The display frame.
        while down.front().is_some_and(|d| d.0 <= t) {
            let (_, tick, whole, body, acked) = down.pop_front().unwrap();
            c.snapshot(tick, whole, &body).unwrap();
            ack = acked;
            pending.retain(|(s, _)| *s > ack);
        }
        let newest = c.newest_tick().unwrap_or(0);
        let v = View {
            render_tick: f64::from(newest),
            newest_tick: newest,
            ack,
            pending: &pending,
            partial: ((t - last_cmd) / step).clamp(0.0, 1.0) as f32,
            hands: sampler.peek(),
            seat: c.seat(),
            now_ms: t,
            correct_ms: angel_net::clock::CORRECT_MS,
        };
        let hero = c.own(&v).expect("the own knight is drawn ahead");
        out.worst_correction = out.worst_correction.max(c.offset.0.hypot(c.offset.1));
        out.frames.push((t, hero.x, hero.y, hero.rolling().is_some(), held));
        t += 1000.0 / 120.0;
    }
    out.counts = seat.take_counts();
    out
}

#[test]
fn a_roll_is_drawn_ahead_with_no_correction_freeze_or_jump_over_a_jittery_link() {
    // D held from 0.2 s to 2.6 s; Space tapped (two ticks) at 0.8 s and,
    // once the roll has rearmed, at 2.1 s.
    let keys = |t: f64| {
        let walk = if (200.0..2600.0).contains(&t) { button::RIGHT } else { 0 };
        let space = if (800.0..860.0).contains(&t) || (2100.0..2160.0).contains(&t) { button::JUMP } else { 0 };
        walk | space
    };
    // A 30 ms trip with up to 50 ms of jitter, each way.
    let jitter = |n: u32| {
        let x = n.wrapping_mul(2_654_435_761).rotate_left(13).wrapping_mul(0x9E37_79B1);
        30.0 + 50.0 * f64::from(x) / f64::from(u32::MAX)
    };
    let r = roll_through(3.0, keys, jitter);
    assert!(r.host_rolled >= 18, "the host rolled twice: {} ticks", r.host_rolled);
    assert_eq!((r.counts.guessed, r.counts.folded), (0, 0), "{:?}", r.counts);
    assert!(r.worst_correction < 1e-3, "corrected by {}", r.worst_correction);
    let rolling = r.frames.iter().filter(|f| f.3).count();
    assert!(rolling > 2 * 20, "the roll drawn on {rolling} frames");
    // Every frame with D held after the first command moves the knight
    // east, by no more than a roll's pace allows: nothing held still,
    // nothing skipped.
    let per_frame_max = 27.0 / 120.0 * 1.2;
    for w in r.frames.windows(2) {
        let (a, b) = (w[0], w[1]);
        if a.0 < 260.0 || b.4 & button::RIGHT == 0 {
            continue;
        }
        let dx = b.1 - a.1;
        assert!(dx > 0.02, "frozen at {:.0} ms: {:.3} -> {:.3} (rolling {})", b.0, a.1, b.1, b.3);
        assert!(dx < per_frame_max, "jumped at {:.0} ms: {:.3} -> {:.3}", b.0, a.1, b.1);
    }
}

#[test]
fn on_a_fallen_runs_board_enter_asks_to_go_back_and_the_board_says_so() {
    let mut run = Run::new(7, 1, Some("Friend"));
    run.phase = crate::rules::Phase::Wiped;
    let mut host = Host::new(run);
    let mut c = client(Some(1));
    host.send(&mut c);
    // A walk or a shot is nothing on the board; Enter is the way back.
    assert_eq!(c.command(&held(button::RIGHT | button::FIRE)), DelveCmd::neutral());
    let enter = c.command(&tap(button::OK));
    assert!(enter.pressed.again && enter.input().again);
    let mut b = Vec::new();
    DelveClient::encode_cmd(&enter, &mut b);
    assert_eq!(DelveCmd::decode(&b), Some(enter), "it reads back on the host");
    // The board shows the way, then the wait once a knight has asked.
    let mut frame = Frame::default();
    c.draw(&view(&c, &[], 0, Hands::default(), 0.0), &mut frame);
    let asking = frame.px.clone();
    host.run.ask_again(2);
    host.step(&[]);
    host.send(&mut c);
    c.draw(&view(&c, &[], 0, Hands::default(), 0.0), &mut frame);
    assert_eq!(c.newest().unwrap().again.as_ref().map(|a| a.asked.len()), Some(1));
    assert_ne!(frame.px, asking, "the board counts down");
}

#[test]
fn the_tab_sheet_shows_held_cards_totals_and_every_wish_with_its_level() {
    let mut run = Run::new(4, 1, Some("Gawain"));
    // The school's locks: the sheet wishes the fork, an Adept tier.
    let key = crate::rules::home::Home::school_key(&run.players[&1]);
    run.home.school.knights.insert(
        key,
        crate::rules::school::Standing {
            rank: crate::rules::school::Rank::Adept,
            ..Default::default()
        },
    );
    run.players.get_mut(&1).unwrap().wishes = 1;
    run.wish(1, &crate::rules::phrasebook::get("fork").unwrap()).unwrap();
    let shown = sheet(&run, 1);
    if std::env::var_os("SHOW_SHEET").is_some() {
        println!("{shown}");
    }
    assert!(shown.contains("HELD CARDS"));
    assert!(shown.contains("fork 1"), "the totals carry the fork");
    assert!(shown.contains("Awakened Fork      1/3"), "the wish shows its level");
    assert!(shown.contains("Fart Bombs         0/3"));
    assert!(shown.contains("No wishes held"));
    assert!(sheet(&run, 9).is_empty(), "no knight, no sheet");
}

#[test]
fn latch_counter_stamps_the_drawn_tick_even_when_a_new_snapshot_has_arrived() {
    let mut r = Run::latch_for_test();
    r.step(&BTreeMap::from([(1,Input { interact:true, ..Default::default() })]));
    while r.latch.as_ref().unwrap().duels[&1].beat != crate::rules::latch::Beat::Tell {
        r.step(&BTreeMap::new());
    }
    let mut host = Host::new(r);
    let mut c = client(Some(0));
    host.send(&mut c);
    let seen = host.run.tick;
    let v = view(&c,&[],0,Hands::default(),0.0);
    c.draw(&v,&mut Frame::default());
    for _ in 0..4 { host.run.step(&BTreeMap::new()); }
    host.send(&mut c);
    let cmd = c.command(&Hands { held:button::FIRE, pressed:button::FIRE, ..Hands::default() });
    assert_eq!(cmd.input().latch_tick,Some(seen));
    assert_ne!(cmd.input().latch_tick,Some(host.run.tick));
}

#[test]
fn mossbell_strokes_stamp_the_drawn_tick_without_another_round_trip() {
    let mut r=Run::mossbell_for_test();
    r.step(&BTreeMap::from([(1,Input{interact:true,..Default::default()})]));
    let mut host=Host::new(r);let mut c=client(Some(0));host.send(&mut c);
    let seen=host.run.tick;let v=view(&c,&[],0,Hands::default(),0.0);c.draw(&v,&mut Frame::default());
    for _ in 0..4{host.run.step(&BTreeMap::new());}host.send(&mut c);
    for (key,x) in [(button::FIRE,0),(button::LEFT,-127),(button::RIGHT,127)] {
        let cmd=c.command(&Hands{held:key,pressed:key,x,..Hands::default()});
        assert_eq!(cmd.input().latch_tick,Some(seen));assert_ne!(Some(host.run.tick),cmd.input().latch_tick);
    }
}

#[test]
fn brasswake_punch_and_cask_stamp_the_drawn_tick() {
    let mut r=Run::brasswake_for_test();r.step(&BTreeMap::from([(1,Input{interact:true,..Default::default()})]));
    let mut host=Host::new(r);let mut c=client(Some(0));host.send(&mut c);
    let seen=host.run.tick;let v=view(&c,&[],0,Hands::default(),0.0);c.draw(&v,&mut Frame::default());
    for _ in 0..4{host.run.step(&BTreeMap::new());}host.send(&mut c);
    for key in [button::FIRE,button::USE]{let cmd=c.command(&Hands{held:key,pressed:key,..Hands::default()});assert_eq!(cmd.input().latch_tick,Some(seen));assert_ne!(cmd.input().latch_tick,Some(host.run.tick));}
}

#[test]
fn escape_hold_menu_taps_are_neutral_and_only_held_commands_count() {
    let controls: serde_json::Value = serde_json::from_str(CONTROLS).unwrap();
    assert_eq!(controls["pad"]["9"], "MENU");
    assert_eq!(controls["pad"]["8"], "X3");
    let mut r = Run::brasswake_for_test();
    for id in 2..=4 { r.join_seat(id, &format!("Knight {id}")); }
    r.floor_plates.clear();
    r.step(&BTreeMap::from([(1, Input { interact: true, ..Default::default() })]));
    let mut host = Host::new(r);
    let mut c = client(Some(0));
    host.send(&mut c);
    assert_eq!(c.command(&tap(button::MENU)), DelveCmd::neutral());
    assert!(!c.menu_open());
    assert!(!c.command(&tap(button::OK)).input().leave_activity);
    for _ in 0..crate::rules::exits::LEAVE_TICKS {
        let cmd = c.command(&held(button::MENU));
        assert!(cmd.held.leave_held && !cmd.held.activity_menu);
        let mut encoded = Vec::new(); cmd.encode(&mut encoded);
        assert_eq!(DelveCmd::decode(&encoded), Some(cmd));
        host.step(&[(0, cmd)]);
        host.send(&mut c);
    }
    assert!(host.run.exits.outside.contains_key(&1));
    assert!(host.run.brasswake.is_some());
}

#[test]
fn holding_escape_leaves_a_touge_hall() {
    use crate::rules::race3d::Course;
    let mut run = arena::race3d::testing::racing(Course::Touge, 4);
    run.floor_plates.clear();
    let mut host = Host::new(run);
    let mut c = client(Some(0));
    host.send(&mut c);
    assert!(host.run.can_leave_activity(1));
    assert_eq!(c.command(&tap(button::MENU)), DelveCmd::neutral());
    assert!(!c.menu_open());
    for tick in 0..crate::rules::exits::LEAVE_TICKS {
        let cmd = c.command(&held(button::MENU));
        assert!(cmd.held.leave_held && !cmd.pressed.leave_activity);
        host.step(&[(0, cmd)]);
        host.send(&mut c);
        assert_eq!(host.run.exits.outside.contains_key(&1), tick + 1 == crate::rules::exits::LEAVE_TICKS);
    }
    assert!(host.run.exits.outside.contains_key(&1));
    assert!(!host.run.race3d.as_ref().unwrap().riders.contains_key(&1));
    assert_eq!(host.run.race3d.as_ref().unwrap().riders.len(), 3);
}

#[test]
fn side_on_space_jumps_and_up_shoots_up() {
    use angel_net::input::button;
    // Up held (move_y -1), no Space: aim and fire straight up, no jump.
    let mut input = Input { move_y: -1, move_x: 1, ..Input::default() };
    side_on_keys(&mut input, 0);
    assert_eq!((input.move_y, input.aim_x, input.aim_y, input.fire), (0, 0, -1, true));
    // Space held: a jump (the rules read move_y < 0), not a dash.
    let mut input = Input { dash: true, ..Input::default() };
    side_on_keys(&mut input, button::JUMP);
    assert_eq!((input.move_y, input.dash), (-1, false));
    // Down still drops through a plank.
    let mut input = Input { move_y: 1, ..Input::default() };
    side_on_keys(&mut input, 0);
    assert_eq!(input.move_y, 1);
}

/// The first room with its monsters gone, a friend in it, and plain bows:
/// room to shoot.
fn range() -> Run {
    let mut run = Run::new(7, 1, Some("Friend"));
    run.enemies.clear();
    run.projectiles.clear();
    for hero in run.players.values_mut() {
        hero.weapon = crate::rules::Weapon::Bow;
        hero.forged = None;
    }
    run
}

/// A frame's view at `now_ms`, `render` ticks behind the newest.
fn view_at<'a>(c: &DelveClient, pending: &'a [(u32, DelveCmd)], ack: u32, hands: Hands, partial: f32, now_ms: f64) -> View<'a, DelveCmd> {
    View { now_ms, ..view(c, pending, ack, hands, partial) }
}

/// The own shots the last frame drew.
fn own_drawn(c: &DelveClient) -> Vec<(f32, f32)> {
    c.drawn_all.iter().filter(|s| s.0).map(|s| (s.1, s.2)).collect()
}

/// A command that aims up (and so fires), and one that walks east.
fn aim_up() -> DelveCmd {
    DelveCmd { held: Input { aim_y: -1, ..Input::default() }, pressed: Input::default() }
}

fn east() -> DelveCmd {
    DelveCmd { held: Input { move_x: 1, ..Input::default() }, pressed: Input::default() }
}

/// A host and a client four ticks into `range`.
fn at_the_range() -> (Host, DelveClient) {
    let mut host = Host::new(range());
    let mut c = client(Some(0));
    host.send(&mut c);
    for _ in 0..3 {
        host.step(&[]);
        host.send(&mut c);
    }
    (host, c)
}

#[test]
fn a_fire_press_shows_an_own_shot_on_the_next_frame_where_the_knight_is_drawn() {
    let (_, mut c) = at_the_range();
    let mut frame = Frame::default();
    let server = c.newest().unwrap().players[&1].x;
    // Running east, two commands the server has not answered, and up
    // pressed since the last command: no command carries it yet.
    let mut hands = held(button::RIGHT | button::AIM_UP);
    hands.pressed = button::AIM_UP;
    c.draw(&view(&c, &[(1, east()), (2, east())], 0, hands, 0.25), &mut frame);
    let (kx, ky) = c.drawn_at().unwrap();
    assert!(kx - server > 0.8, "the knight is drawn ahead of the server's: {server} -> {kx}");
    let shots = own_drawn(&c);
    assert_eq!(shots.len(), 1, "one arrow, on this frame: {shots:?}");
    let (x, y) = shots[0];
    assert!((x - kx).abs() < 0.2, "it leaves the knight as drawn ({kx}), not the server's ({server}): {x}");
    assert!(y < ky && ky - y < 1.2, "just above the knight: {ky} -> {y}");
    let mut counts = Vec::new();
    c.dev_stats(&mut counts);
    assert!(counts.contains(&("shot_ghosts", 1.0)), "{counts:?}");
}

#[test]
fn the_servers_shot_takes_the_ghosts_place_eased_with_no_second_shot() {
    let (mut host, mut c) = at_the_range();
    let mut frame = Frame::default();
    let step = 1000.0 / f64::from(HZ);
    let t0 = 1000.0;
    // Command 1 fires: its ghost flies from the knight.
    let sent = [(1, aim_up())];
    c.draw(&view_at(&c, &sent, 0, Hands::default(), 0.5, t0), &mut frame);
    let ghost = own_drawn(&c);
    assert_eq!(ghost.len(), 1);
    // The server's knight stood half a unit to the east of the client's
    // when it took the command: its arrow leaves from there.
    host.run.players.get_mut(&1).unwrap().x += 0.5;
    host.step(&[(0, aim_up())]);
    assert_eq!(host.run.projectiles.iter().filter(|p| p.loosed_by(1)).count(), 1);
    host.send(&mut c);
    // A tick on, the answer: the server's shot is drawn where the ghost
    // would be (no jump), and it is the only one.
    c.draw(&view_at(&c, &[], 1, Hands::default(), 0.5, t0 + step), &mut frame);
    let handed = own_drawn(&c);
    assert_eq!(handed.len(), 1, "no second shot: {handed:?}");
    let newest = c.newest().unwrap();
    let server = newest.projectiles.iter().find(|p| p.loosed_by(1)).unwrap();
    let now_at = (server.x + server.vx * 0.5 / HZ as f32, server.y + server.vy * 0.5 / HZ as f32);
    // (The wire sends a shot's place to a 256th of a unit.)
    let near = |a: f32, b: f32| (a - b).abs() < 0.01;
    assert!(near(handed[0].0, ghost[0].0), "drawn at the ghost's x: {ghost:?} -> {handed:?}");
    assert!(near(handed[0].0, now_at.0 - 0.5), "half a unit off the server's, to ease out: {handed:?} {now_at:?}");
    let mut counts = Vec::new();
    c.dev_stats(&mut counts);
    assert!(counts.contains(&("shot_handovers", 1.0)) && counts.contains(&("shot_ghosts", 0.0)), "{counts:?}");
    // Half the ease on, half the gap; once it has run, the server's own place.
    c.draw(&view_at(&c, &[], 1, Hands::default(), 0.5, t0 + step + SHOT_EASE_MS / 2.0), &mut frame);
    let mid = own_drawn(&c);
    assert!(near(mid[0].0, now_at.0 - 0.25), "{mid:?} {now_at:?}");
    c.draw(&view_at(&c, &[], 1, Hands::default(), 0.5, t0 + step + SHOT_EASE_MS + 1.0), &mut frame);
    let done = own_drawn(&c);
    assert_eq!(done.len(), 1);
    assert!(near(done[0].0, now_at.0) && near(done[0].1, now_at.1), "{done:?} {now_at:?}");
    // The next tick it is the same shot, flown on: still one.
    host.step(&[(0, DelveCmd::neutral())]);
    host.send(&mut c);
    c.draw(&view_at(&c, &[], 2, Hands::default(), 0.0, t0 + 2.0 * step + SHOT_EASE_MS), &mut frame);
    assert_eq!(own_drawn(&c).len(), 1);
}

#[test]
fn a_ghost_the_server_never_matches_goes_quietly() {
    let (mut host, mut c) = at_the_range();
    let mut frame = Frame::default();
    let step = 1000.0 / f64::from(HZ);
    let t0 = 1000.0;
    c.draw(&view_at(&c, &[(1, aim_up())], 0, Hands::default(), 0.5, t0), &mut frame);
    assert_eq!(own_drawn(&c).len(), 1);
    // The server took command 1 but loosed nothing (its arm was not ready).
    host.run.players.get_mut(&1).unwrap().fire_cooldown = 20;
    host.step(&[(0, aim_up())]);
    assert_eq!(host.run.projectiles.iter().filter(|p| p.loosed_by(1)).count(), 0);
    host.send(&mut c);
    // It flies on a moment, in case the shot comes a tick late, then goes.
    c.draw(&view_at(&c, &[], 1, Hands::default(), 0.5, t0 + step), &mut frame);
    assert_eq!(own_drawn(&c).len(), 1, "a moment's grace");
    c.draw(&view_at(&c, &[], 1, Hands::default(), 0.5, t0 + step + GHOST_GRACE_MS + 1.0), &mut frame);
    assert!(own_drawn(&c).is_empty(), "gone");
    let mut counts = Vec::new();
    c.dev_stats(&mut counts);
    assert!(counts.contains(&("shot_unmatched", 1.0)) && counts.contains(&("shot_ghosts", 0.0)), "{counts:?}");
    // A ghost that meets stone goes there and then: aimed west from the
    // west wall, it is gone before the server answers.
    let west = DelveCmd { held: Input { aim_x: -1, ..Input::default() }, pressed: Input::default() };
    let (mut host, mut c) = at_the_range();
    host.run.players.get_mut(&1).unwrap().x = 1.6;
    host.step(&[]);
    host.send(&mut c);
    let pending: Vec<(u32, DelveCmd)> = (1..=6).map(|s| (s, west)).collect();
    c.draw(&view_at(&c, &pending[..1], 0, Hands::default(), 0.0, t0), &mut frame);
    c.draw(&view_at(&c, &pending, 0, Hands::default(), 0.0, t0 + 5.0 * step), &mut frame);
    assert!(own_drawn(&c).is_empty(), "into the wall: {:?}", own_drawn(&c));
}

#[test]
fn a_friends_shots_and_the_monsters_are_still_drawn_between_two_ticks() {
    let (mut host, mut c) = at_the_range();
    let mut frame = Frame::default();
    // Both knights fire; a monster's shot flies across the room too.
    host.step(&[(0, aim_up()), (1, aim_up())]);
    let mut monster = host.run.projectiles.iter().find(|p| p.loosed_by(2)).unwrap().clone();
    (monster.hostile, monster.traits.owner) = (true, 0);
    (monster.x, monster.vx, monster.vy) = (monster.x + 4.0, 0.0, -6.0);
    host.run.projectiles.push(monster);
    host.send(&mut c);
    for _ in 0..2 {
        host.step(&[]);
        host.send(&mut c);
    }
    let newest = c.newest().unwrap().clone();
    let theirs: Vec<(f32, f32)> = newest
        .projectiles
        .iter()
        .filter(|p| !p.loosed_by(1))
        .map(|p| (p.x - p.vx * 0.5 / HZ as f32, p.y - p.vy * 0.5 / HZ as f32))
        .collect();
    assert_eq!(theirs.len(), 2, "the friend's arrow and the monster's shot");
    let mut v = view(&c, &[], 1, Hands::default(), 0.0);
    v.render_tick -= 0.5;
    c.draw(&v, &mut frame);
    let drawn: Vec<(f32, f32)> = c.drawn_all.iter().filter(|s| !s.0).map(|s| (s.1, s.2)).collect();
    assert_eq!(drawn.len(), 2);
    for (d, t) in drawn.iter().zip(&theirs) {
        assert!((d.0 - t.0).abs() < 1e-4 && (d.1 - t.1).abs() < 1e-4, "half a tick back, as before: {d:?} {t:?}");
    }
    // Ours is drawn from the newest tick, with the knight.
    let ours = newest.projectiles.iter().find(|p| p.loosed_by(1)).unwrap();
    assert_eq!(own_drawn(&c), vec![(ours.x, ours.y)]);
}

/// What a client drew of its own shots over a link: each display frame's
/// time, the knight as drawn, and its own shots as drawn.
struct Fired {
    frames: Vec<(f64, f32, f32, Vec<(f32, f32)>)>,
    /// Own shots the host loosed in all.
    loosed: usize,
}

/// `roll_through`'s link, in `range`, drawing the own shots each 120 Hz
/// frame as `draw` does (`own`, then `own_shots`).
fn fire_through(secs: f64, keys: impl Fn(f64) -> u32, one_way_ms: f64) -> Fired {
    use angel_net::cmd::SeatInputs;
    use angel_net::input::Sampler;
    let step = 1000.0 / f64::from(HZ);
    let mut host = Host::new(range());
    let mut c = client(Some(0));
    let mut seat: SeatInputs<DelveCmd> = SeatInputs::default();
    let mut sampler = Sampler::default();
    let mut up: VecDeque<(f64, u32, DelveCmd)> = VecDeque::new();
    let mut down: VecDeque<(f64, u32, bool, Vec<u8>, u32)> = VecDeque::new();
    let mut pending: Vec<(u32, DelveCmd)> = Vec::new();
    let (mut seq, mut ack, mut last_cmd) = (0u32, 0u32, 0.0f64);
    let mut out = Fired { frames: Vec::new(), loosed: 0 };
    let (mut next_cmd, mut next_tick, mut t) = (0.0, 7.0, 0.0);
    let mut held = 0u32;
    let (tick, whole, body) = host.snapshot(Some(0));
    c.snapshot(tick, whole, &body).unwrap();
    while t < secs * 1000.0 {
        let now_keys = keys(t);
        if now_keys != held {
            sampler.down(now_keys & !held);
            sampler.up(held & !now_keys);
            held = now_keys;
        }
        while next_cmd <= t {
            let cmd = c.command(&sampler.sample());
            seq += 1;
            pending.push((seq, cmd));
            up.push_back((next_cmd + one_way_ms, seq, cmd));
            last_cmd = next_cmd;
            next_cmd += step;
        }
        while next_tick <= t {
            while up.front().is_some_and(|u| u.0 <= next_tick) {
                let (_, s, cmd) = up.pop_front().unwrap();
                seat.push(s, cmd);
            }
            let (cmd, acked) = seat.take();
            let had = host.run.projectiles.iter().filter(|p| p.loosed_by(1)).count();
            host.step(&[(0, cmd)]);
            let has = host.run.projectiles.iter().filter(|p| p.loosed_by(1)).count();
            out.loosed += has.saturating_sub(had);
            let (tick, whole, body) = host.snapshot(Some(0));
            down.push_back((next_tick + one_way_ms, tick, whole, body, acked));
            next_tick += step;
        }
        while down.front().is_some_and(|d| d.0 <= t) {
            let (_, tick, whole, body, acked) = down.pop_front().unwrap();
            c.snapshot(tick, whole, &body).unwrap();
            ack = acked;
            pending.retain(|(s, _)| *s > ack);
        }
        let newest = c.newest_tick().unwrap_or(0);
        let v = View {
            render_tick: f64::from(newest),
            newest_tick: newest,
            ack,
            pending: &pending,
            partial: ((t - last_cmd) / step).clamp(0.0, 1.0) as f32,
            hands: sampler.peek(),
            seat: c.seat(),
            now_ms: t,
            correct_ms: angel_net::clock::CORRECT_MS,
        };
        let hero = c.own(&v).expect("the own knight is drawn ahead");
        c.own_shots(&v);
        let shots = c.drawn_shots.iter().map(|s| (s.x, s.y)).collect();
        out.frames.push((t, hero.x, hero.y, shots));
        t += 1000.0 / 120.0;
    }
    out
}

#[test]
fn over_a_150_ms_link_a_running_knights_arrow_leaves_it_at_once_and_flies_on_unbroken() {
    // East held from 0.2 s; up tapped at 0.8 s and at 1.6 s.
    let press = [800.0, 1600.0];
    let keys = |t: f64| {
        let walk = if (200.0..2400.0).contains(&t) { button::RIGHT } else { 0 };
        let aim = if press.iter().any(|&p| (p..p + 20.0).contains(&t)) { button::AIM_UP } else { 0 };
        walk | aim
    };
    let r = fire_through(2.6, keys, 75.0);
    assert_eq!(r.loosed, 2, "the host loosed both arrows");
    let frame_ms = 1000.0 / 120.0;
    // A bow's arrow flies 40 units a second. On the frame of the press it
    // has flown as long as the tick its command plays on has run (the
    // server moves a shot on the tick it looses it): the part of a tick not
    // yet sampled, or a tick and a frame for a press sampled at once.
    let arrow_speed = 40.0;
    let first_flight = 0.6 + arrow_speed * (1.0 / HZ as f32 + 1.0 / 120.0) + 0.1;
    for &p in &press {
        // The first frame drawn after the press shows the arrow, at the
        // knight as drawn.
        let i = r.frames.iter().position(|f| f.0 >= p).unwrap();
        let (t, kx, ky, shots) = &r.frames[i];
        assert!(*t - p < frame_ms, "the frame after the press");
        let near = shots.iter().min_by(|a, b| (a.0 - kx).hypot(a.1 - ky).total_cmp(&(b.0 - kx).hypot(b.1 - ky)));
        let (x, y) = *near.unwrap_or_else(|| panic!("no arrow on the frame after the press at {p} ms"));
        assert!((x - kx).abs() < 0.25 && ky - y < first_flight, "it leaves the knight: knight ({kx}, {ky}), arrow ({x}, {y})");
        // From there on, one arrow for this press, never jumping: each frame
        // it flies on by a frame's flight, through the server's answer.
        let mut was = (x, y);
        // (A third of a second: through the answer, 150 ms on, and its ease,
        // short of the far wall.)
        for f in r.frames[i + 1..].iter().take_while(|f| f.0 < p + 330.0) {
            let mine: Vec<_> = f.3.iter().filter(|s| (s.0 - was.0).abs() < 1.0).collect();
            assert_eq!(mine.len(), 1, "one arrow at {:.0} ms: {:?}", f.0, f.3);
            let (dx, dy) = (mine[0].0 - was.0, mine[0].1 - was.1);
            assert!(dx.abs() < 0.05 && -dy < arrow_speed * 1.6 / 120.0 && -dy > 0.0, "jumped at {:.0} ms: {was:?} -> {:?}", f.0, mine[0]);
            was = *mine[0];
        }
    }
}

#[test]
fn a_page_that_draws_scenes_is_sent_a_room_in_3d_by_its_sets_reference() {
    use angel_stage::wire::{TAG_WORLD, decode};
    // The painted path (`ANGEL_WORLD3D_PLATES=1`).
    crate::world3d::rooms_lit::set_plates(true);
    // The welcome turns rooms in 3D on for this seat's page.
    let mut c = DelveClient::boot(&welcome(Some(1), &Params { art_v2: false, world3d: true, deep3d: false }.encode())).unwrap();
    assert!(crate::world3d::enabled());
    let mut host = Host::new(crate::world3d::room::proof_run());
    host.send(&mut c);
    c.stage_props(true);
    let mut out = Vec::new();
    // The first stage turns frames into parts; the next draw makes them.
    let _ = c.stage(1000.0, &mut out);
    c.draw(&view(&c, &[], 0, Hands::default(), 0.0), &mut Frame::default());
    assert!(c.stage(1000.0, &mut out), "a stage frame");
    assert_eq!(&out[..4], TAG_WORLD);
    let first = decode(&out).expect("it reads");
    let world = first.frame.props.as_ref().and_then(|p| p.world.clone()).expect("the room in 3D");
    // The blocks are not on the wire: the set is, with the plate as its PNG,
    // once (never its pixels).
    assert!(world.floor.is_empty() && world.blocks.is_empty());
    let set = world.set.clone().unwrap();
    assert!(set.text.is_some());
    let item = crate::world3d::assets::item("village").unwrap();
    assert_eq!(set.png.as_deref(), Some(&item.png[..]));
    assert_eq!(set.plate, arena::stage_frame::plate_picture(&item));
    let plates = arena::stage_frame::WORLD_PLATE..arena::stage_frame::WORLD_CARDS;
    assert!(first.frame.pictures.iter().all(|p| !plates.contains(&p.id)), "no plate pixels on the wire");
    assert_eq!(world.figures.len(), 2, "both knights stand in it");
    assert!(world.figures_at <= first.quads.len() && world.at <= world.figures_at);
    // The page makes them from it, as the window has them.
    let mut sets = angel_stage::set::Sets::default();
    let mut made = world.clone();
    assert!(sets.fill(&mut made));
    assert_eq!(made.blocks.len() + made.floor.len(), item.set.shapes.len());
    // The next frame names the set by its key alone, and makes the same.
    c.draw(&view(&c, &[], 0, Hands::default(), 0.0), &mut Frame::default());
    assert!(c.stage(1016.0, &mut out));
    let next = decode(&out).unwrap().frame.props.unwrap().world.unwrap();
    assert_eq!(next.set.as_ref().map(|s| (s.key, s.text.is_none(), s.png.is_none())), Some((set.key, true, true)));
    let mut again = next.clone();
    assert!(sets.fill(&mut again));
    assert_eq!((again.floor, again.blocks), (made.floor, made.blocks));
    crate::world3d::set_enabled(false);
}

/// A lit room (`world3d::rooms_lit`, the default 3D) goes to a page as its
/// set's text and its look, no plate: the page makes the same lit blocks.
#[test]
fn a_page_that_draws_scenes_is_sent_a_lit_room_by_its_set_and_its_look() {
    use angel_stage::wire::{TAG_LOOK, decode};
    let mut c = DelveClient::boot(&welcome(Some(1), &Params { art_v2: false, world3d: true, deep3d: false }.encode())).unwrap();
    let run = crate::world3d::room::proof_run();
    let mut host = Host::new(run.clone());
    host.send(&mut c);
    c.stage_props(true);
    let mut out = Vec::new();
    let _ = c.stage(1000.0, &mut out);
    c.draw(&view(&c, &[], 0, Hands::default(), 0.0), &mut Frame::default());
    assert!(c.stage(1000.0, &mut out), "a stage frame");
    assert_eq!(&out[..4], TAG_LOOK, "a world with its look");
    let first = decode(&out).expect("it reads");
    let world = first.frame.props.as_ref().and_then(|p| p.world.clone()).expect("the room in 3D");
    let look = world.look.clone().expect("its look");
    assert!(look.lights() && look.sun.is_some() && look.reveal.is_some(), "lit by its sun, the knight kept in sight");
    assert!(world.floor.is_empty() && world.blocks.is_empty(), "the set is on the wire, not its blocks");
    let set = world.set.clone().unwrap();
    assert!(set.text.is_some() && set.png.is_none(), "its text, no plate");
    assert!(first.frame.pictures.iter().all(|p| p.width * p.height < 512 * 512), "no plate's pixels either");
    assert_eq!(world.figures.len(), 2, "both knights stand in it");
    // The page makes the lit blocks from it, the room's own.
    let mut sets = angel_stage::set::Sets::default();
    let mut made = world.clone();
    assert!(sets.fill(&mut made));
    let lit = crate::world3d::rooms_lit::lit_for(run.room(), run.dungeon.pack).expect("lit");
    let piece = lit.piece((0, 0)).unwrap();
    assert_eq!(set.key, piece.key);
    assert_eq!(made.blocks.len() + made.floor.len(), piece.set.shapes.len() + 1, "its shapes and its ground");
    assert_eq!(made.meshes.len(), 1, "its ground's mesh");
    // The next frame names the set by its key alone.
    c.draw(&view(&c, &[], 0, Hands::default(), 0.0), &mut Frame::default());
    assert!(c.stage(1016.0, &mut out));
    let next = decode(&out).unwrap().frame.props.unwrap().world.unwrap();
    assert_eq!(next.set.as_ref().map(|s| (s.key, s.text.is_none())), Some((set.key, true)));
    let mut again = next.clone();
    assert!(sets.fill(&mut again));
    assert_eq!((again.floor, again.blocks), (made.floor, made.blocks));
    crate::world3d::set_enabled(false);
}

/// A page told the deep floors stand up (`Params::deep3d`) keeps an upper
/// floor in the 2D black look and stands a deep one up lit, what lies on
/// its floor a turning card of plain shapes a wire carries.
#[test]
fn a_page_stands_up_only_the_floors_its_host_says_are_deep() {
    use angel_stage::wire::decode;
    use crate::world3d::room::{self, Mode};
    let mut c = DelveClient::boot(&welcome(Some(1), &Params::with_mode(false, Mode::Deep).encode())).unwrap();
    assert_eq!(room::mode(), Mode::Deep);
    assert!(!crate::world3d::enabled(), "the kingdom and the upper floors stay 2D");
    let frame = |run: crate::rules::Run, c: &mut DelveClient| {
        let mut host = Host::new(run);
        host.send(c);
        c.stage_props(true);
        let mut out = Vec::new();
        let _ = c.stage(1000.0, &mut out);
        c.draw(&view(c, &[], 0, Hands::default(), 0.0), &mut Frame::default());
        assert!(c.stage(1000.0, &mut out), "a stage frame");
        decode(&out).expect("it reads").frame
    };
    let mut upper = crate::rules::Run::new(9, 1, Some("Friend"));
    upper.calm_for_test();
    let shown = frame(upper, &mut c);
    assert!(shown.props.as_ref().is_none_or(|p| p.world.is_none()), "floor 1: the black look");
    let mut deep = crate::rules::Run::new(9, 1, Some("Friend"));
    while deep.dungeon.depth < room::DEEP {
        deep.descend_for_test();
    }
    deep.calm_for_test();
    let (x, y) = deep.players.values().next().map(|h| (h.x + 3.0, h.y)).unwrap();
    let at = deep.at;
    let card = deep.book.cards.iter().find(|c| c.id != "gold").map(|c| c.id.clone()).expect("a card");
    deep.dungeon.rooms[at].items.push(crate::rules::Item { card, x, y, held_off: None, took: vec![] });
    let mut c = DelveClient::boot(&welcome(Some(1), &Params::with_mode(false, Mode::Deep).encode())).unwrap();
    let shown = frame(deep, &mut c);
    let props = shown.props.expect("props");
    let world = props.world.expect("floor 4 stands up");
    assert!(world.look.is_some_and(|l| l.lights()), "lit");
    let faces = arena::stage_frame::PICKUP_FACES..arena::stage_frame::PICKUP_FACES + 0x100;
    let card = |n: &angel_stage::scene::Node| matches!(n.material.source, angel_stage::scene::TextureSource::Picture(id) if faces.contains(&id));
    assert!(props.nodes.iter().any(card), "what lies on the floor turns as a card");
    assert!(props.nodes.iter().all(|n| n.face_materials.is_empty()), "plain nodes, which the wire carries");
    room::set_mode(Mode::Off);
}
