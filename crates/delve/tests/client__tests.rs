use super::*;
use crate::render::overworld::arena::testing::fight;
use crate::render::overworld::kingdom as geography;
use crate::rules::{home, world};
use angel_net::doc::delta::diff;
use angel_net::doc::wire::Writer;
use angel_net::proto::PROTOCOL;

#[path = "client_down__tests.rs"]
mod down_tests;

#[path = "client_crossroads__tests.rs"]
mod crossroads_tests;

#[path = "client_hd__tests.rs"]
mod hd_tests;

/// The host's half, for feeding a client: a run, its publisher (as a
/// host keeps one), and what one client was last sent of it.
struct Host {
    run: Run,
    publisher: wire::Publisher,
    names: Writer,
    last: Option<Value>,
    tick: u32,
}

impl Host {
    fn new(run: Run) -> Host {
        Host { run, publisher: wire::Publisher::default(), names: Writer::default(), last: None, tick: 0 }
    }

    /// The next snapshot for `seat`: (tick, whole, body).
    fn snapshot(&mut self, seat: Option<u32>) -> (u32, bool, Vec<u8>) {
        self.tick += 1;
        let tree = self.publisher.publish(&self.run).unwrap();
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
fn the_welcome_sets_the_size_and_a_stale_page_is_turned_away() {
    let c = DelveClient::boot(&welcome(Some(1), &Params { world3d: false, deep3d: false, atari: false }.encode())).unwrap();
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
        assert_eq!(read(h, c, &host.run), ["notice Wishes given. Rub the genie's lamp (E) and wish."]);
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
    // Cards in hand: the page is told where their plates are, to drag.
    host.run.players.get_mut(&1).unwrap().hand = vec!["potion".into(), "holy-water".into()];
    host.step(&[]);
    host.send(&mut c);
    c.draw(&view(&c, &[], 0, Hands::default(), 0.0), &mut frame);
    c.lines(&mut lines);
    let plates = arena::seat_hand_plates(c.newest().unwrap(), Some(1), 384, 224 + arena::BAND_H + arena::HUD_H);
    assert_eq!(plates.len(), 2);
    let quads: Vec<String> = plates.iter().map(|&(i, (x, y, w, h))| format!("[{i},{x},{y},{w},{h}]")).collect();
    assert_eq!(lines, vec![format!("slots [{}]", quads.join(","))]);
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
    run.players.get_mut(&1).unwrap().wishes = 1;
    run.wish(1, &crate::rules::phrasebook::get("fork").unwrap()).unwrap();
    let shown = sheet(&run, 1);
    if std::env::var_os("SHOW_SHEET").is_some() {
        println!("{shown}");
    }
    assert!(shown.contains("HELD CARDS"));
    assert!(shown.contains("fork 1"), "the totals carry the fork");
    assert!(shown.contains("Awakened Fork      1/5"), "the wish shows its level");
    assert!(shown.contains("Fart Bombs         0/5"));
    assert!(shown.contains("No wishes held"));
    assert!(sheet(&run, 9).is_empty(), "no knight, no sheet");
}

#[test]
fn the_sheet_locks_exactly_the_tier_the_grant_refuses() {
    // A fresh knight at every step of every wish's ladder: the sheet says
    // LOCKED where the grant refuses, and nowhere else. No rank and no
    // teacher holds a tier back; only the floor keeps the fourth and fifth
    // (`overclass::top_tier`), the third open everywhere.
    let fresh = Run::new(4, 1, Some("Gawain"));
    for depth in [1, 2, crate::rules::FLOORS] {
        let top = crate::rules::overclass::top_tier(depth);
        for wish in crate::rules::phrasebook::book().iter() {
            for level in 0..wish.tiers.len() {
                let mut run = fresh.clone();
                run.dungeon.depth = depth;
                let hero = run.players.get_mut(&1).unwrap();
                for t in 0..level {
                    hero.deck.push(wish.card_id(t));
                }
                let shown = sheet(&run, 1);
                let line = shown
                    .lines()
                    .find(|l| l.starts_with(&format!("  {:<18} {level}/", wish.name)))
                    .unwrap_or_else(|| panic!("{} on the sheet:\n{shown}", wish.name));
                let refused = run.grant(1, wish).is_err_and(|why| why.contains("climbs no higher here"));
                assert_eq!(line.contains("LOCKED"), refused, "{} at {level}, floor {depth}: {line}", wish.name);
                assert_eq!(refused, level > top, "{} tier {} on floor {depth}", wish.name, level + 1);
                assert!(level > 2 || !refused, "{} tier {} is open to every knight", wish.name, level + 1);
            }
        }
    }
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
fn the_pages_windows_keep_off_the_seats_bands() {
    let controls: serde_json::Value = serde_json::from_str(CONTROLS).unwrap();
    use crate::render::overworld::arena::party;
    assert_eq!(controls["view"]["top"], party::BAND_H);
    assert_eq!(controls["view"]["bottom"], party::HUD_H);
    let (_, h) = party::seat_size();
    assert_eq!(h as i32, crate::render::overworld::arena::NATIVE_H + party::BAND_H + party::HUD_H);
    for field in ["key", "hint", "title", "placeholder", "send", "cancel"] {
        assert!(controls["text"][field].is_string(), "{field}");
    }
    assert!(controls["choices"]["back"].is_string());
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

/// A lit room (`world3d::rooms_lit`, the default 3D) goes to a page as its
/// set's text and its look, no plate: the page makes the same lit blocks.
#[test]
fn a_page_that_draws_scenes_is_sent_a_lit_room_by_its_set_and_its_look() {
    use angel_stage::wire::{TAG_LOOK, decode};
    let mut c = DelveClient::boot(&welcome(Some(1), &Params { world3d: true, deep3d: false, atari: false }.encode())).unwrap();
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
    // The page makes the lit blocks from it, the room's own, baked.
    let mut sets = angel_stage::set::Sets::default();
    let mut made = world.clone();
    assert!(sets.fill(&mut made));
    let lit = crate::world3d::rooms_lit::lit_for(run.room(), run.dungeon.pack).expect("lit");
    let piece = lit.piece((0, 0)).unwrap();
    assert_eq!(set.key, piece.key);
    // Baking reduces the node count: the page has far fewer nodes than the set's shapes.
    assert!(made.blocks.len() + made.floor.len() < piece.set.shapes.len() + 1,
        "baking reduces nodes: page {} vs set {}",
        made.blocks.len() + made.floor.len(), piece.set.shapes.len() + 1);
    assert!(made.meshes.len() >= 1, "its ground's mesh plus baked meshes");
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
    let mut c = DelveClient::boot(&welcome(Some(1), &Params::with_mode(Mode::Deep).encode())).unwrap();
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
    // Past the beat of coming down into the deep, which leans the camera in.
    deep.deep_beat = None;
    let (x, y) = deep.players.values().next().map(|h| (h.x + 3.0, h.y)).unwrap();
    let at = deep.at;
    // A bow has no model: it turns as a card. A sword is its model.
    deep.dungeon.rooms[at].items.push(crate::rules::Item::lying("bow".into(), x, y, 0));
    deep.dungeon.rooms[at].items.push(crate::rules::Item::lying("knights-blade".into(), x + 3.0, y, 0));
    let mut c = DelveClient::boot(&welcome(Some(1), &Params::with_mode(Mode::Deep).encode())).unwrap();
    let knights = deep.players.values().filter(|h| h.hp > 0).count();
    let shown = frame(deep, &mut c);
    let props = shown.props.expect("props");
    let world = props.world.expect("floor 4 stands up");
    let look = world.look.expect("its look");
    assert!(look.lights(), "lit");
    // A lit delve lights its play where it stands: a pool at each knight, a
    // glow over each of the two things on its floor (reach in stage pixels).
    let pools = |reach: f32| look.lamps.iter().filter(|l| (l.reach - reach).abs() < 1e-3).count();
    assert_eq!(pools(arena::stage_frame::LOOT_POOL.0), 2, "a glow over the bow and the blade");
    assert_eq!(pools(arena::stage_frame::KNIGHT_POOL.0), knights, "a pool round each standing knight");
    let faces = arena::stage_frame::PICKUP_FACES..arena::stage_frame::PICKUP_FACES + 0x100;
    let card = |n: &angel_stage::scene::Node| matches!(n.material.source, angel_stage::scene::TextureSource::Picture(id) if faces.contains(&id));
    assert!(props.nodes.iter().any(card), "what lies on the floor turns as a card");
    let model = |n: &angel_stage::scene::Node| matches!(n.mesh, Some(angel_stage::scene::MeshRef::Uploaded(_)));
    assert!(props.nodes.iter().any(model), "or lies as its model, across the wire");
    assert!(props.nodes.iter().all(|n| n.face_materials.is_empty()), "plain nodes, which the wire carries");
    room::set_mode(Mode::Off);
}

#[test]
fn a_rub_of_the_lamp_opens_that_seats_wish_line_and_its_grant_is_said_to_it() {
    let mut host = Host::new(Run::new(7, 1, Some("Ada")));
    let mut mine = (Host::new(host.run.clone()), client(Some(0)));
    let mut theirs = (Host::new(host.run.clone()), client(Some(1)));
    let read = |h: &mut Host, c: &mut DelveClient, run: &Run| {
        h.run = run.clone();
        h.send(c);
        let mut lines = Vec::new();
        c.lines(&mut lines);
        lines.into_iter().filter(|l| l == "ask" || l.starts_with("notice Your wish")).collect::<Vec<_>>()
    };
    read(&mut mine.0, &mut mine.1, &host.run);
    read(&mut theirs.0, &mut theirs.1, &host.run);
    // Knight 1 rubs the lamp: only its seat opens its line, and once.
    host.run.lamp.rubbed.insert(1, host.run.tick + 1);
    host.run.step(&BTreeMap::new());
    assert_eq!(read(&mut mine.0, &mut mine.1, &host.run), ["ask"]);
    assert!(read(&mut mine.0, &mut mine.1, &host.run).is_empty());
    assert!(read(&mut theirs.0, &mut theirs.1, &host.run).is_empty());
    // Its wish lands mid-play: said to that seat, in short.
    host.run.lamp_sent(1, "bouncy arrows");
    host.run.step(&BTreeMap::new());
    host.run.lamp_landed(1, "bouncy arrows", "Fine Ricochet I. Bounce waits on Swift");
    host.run.step(&BTreeMap::new());
    assert_eq!(read(&mut mine.0, &mut mine.1, &host.run), ["notice Your wish is granted: Fine Ricochet I"]);
    assert!(read(&mut theirs.0, &mut theirs.1, &host.run).is_empty());
}


/// Six knights, a full room of foes and a few hundred shots, as the host
/// would send it: a whole tree, then a patch a tick (or nothing, when the
/// tree did not change).
fn crowded_stream(ticks: u32) -> (Vec<(u32, bool, Vec<u8>)>, usize, usize) {
    use crate::rules::{EnemyKind, RoomKind, TILE_UNITS};
    let mut run = Run::new(11, 1, Some("Percival"));
    for (id, name) in [(2, "Lynette"), (3, "Gareth"), (4, "Galahad"), (5, "Gawain"), (6, "Kay")] {
        run.join_seat(id, name);
    }
    let index = run
        .dungeon
        .rooms
        .iter()
        .position(|room| room.kind == RoomKind::Fight && !room.great())
        .expect("a fight room");
    run.enter_for_test(index);
    run.enemies.clear();
    let (cols, rows) = {
        let room = &run.dungeon.rooms[run.at];
        (room.cols.max(4), room.rows.max(4))
    };
    let place = |run: &mut Run, n: usize| {
        for i in 0..n {
            let span = (cols - 2).max(1);
            let c = 1 + (i % span);
            let r = 1 + ((i / span) % (rows - 2).max(1));
            run.spawn_at_for_test(EnemyKind::Slime, (c as f32 + 0.5) * TILE_UNITS, (r as f32 + 0.5) * TILE_UNITS);
        }
    };
    place(&mut run, 96);
    let volley = |run: &mut Run, salt: f32| {
        let (x, y) = run.players.values().next().map(|hero| (hero.x, hero.y)).unwrap_or((4.0, 4.0));
        for turn in 0..18 {
            run.volley_for_test(x, y, turn as f32 * 0.35 + salt);
        }
    };
    volley(&mut run, 0.0);
    let foes = run.enemies.len();
    let shots = run.projectiles.len();
    let mut publisher = crate::wire::Publisher::default();
    let mut names = Writer::default();
    let mut last = None;
    let mut frames = Vec::new();
    for n in 0..ticks {
        if n > 0 {
            if run.enemies.len() < 80 {
                let need = 96 - run.enemies.len();
                place(&mut run, need);
            }
            if run.projectiles.len() < 200 {
                volley(&mut run, n as f32);
            }
            run.step(&BTreeMap::new());
        }
        let tree = publisher.publish(&run).expect("the crowded run publishes");
        let view = crate::wire::pace(crate::wire::seat_tree(&tree, Some(1)), last.as_ref()).into_owned();
        let mut body = Vec::new();
        let whole = last.is_none();
        if whole {
            names.value(&mut body, &view);
        } else if let Some(patch) = diff(last.as_ref().unwrap(), &view) {
            names.patch(&mut body, &patch);
        }
        last = Some(view);
        frames.push(((n + 1) as u32, whole, body));
    }
    (frames, foes, shots)
}

/// The apply this seat used to do: decode, patch the tree, read the run whole.
fn old_apply(names: &mut Reader, tree: &mut Option<Value>, whole: bool, body: &[u8]) -> Result<Option<Run>, String> {
    if whole {
        *names = Reader::default();
        let mut bytes = Bytes::new(body);
        *tree = Some(names.value(&mut bytes).map_err(|_| "malformed")?);
        bytes.end().map_err(|_| "malformed")?;
    } else if body.is_empty() {
        return Ok(None);
    } else {
        let held = tree.as_mut().ok_or("a patch before the whole tree")?;
        let mut bytes = Bytes::new(body);
        let patch = names.patch(&mut bytes).map_err(|_| "malformed")?;
        bytes.end().map_err(|_| "malformed")?;
        delta::apply(held, patch).map_err(|_| "mismatch")?;
    }
    let run = crate::wire::read(tree.as_ref().unwrap()).map_err(|e| e.to_string())?;
    Ok(Some(run))
}

fn guest_diff(got: &Run, old: &Run) -> Result<(), String> {
    if got.marks != old.marks {
        return Err("marks".into());
    }
    if got.pot_shards != old.pot_shards {
        return Err("pot shards".into());
    }
    let left = angel_net::doc::value::to_value(got).map_err(|e| e.to_string())?;
    let right = angel_net::doc::value::to_value(old).map_err(|e| e.to_string())?;
    if left == right && got.campaign == old.campaign {
        return Ok(());
    }
    if got.campaign != old.campaign {
        return Err("campaign".into());
    }
    let (Value::Struct(a), Value::Struct(b)) = (left.get(), right.get()) else {
        return Err("the runs are not structs".into());
    };
    for (name, field) in a {
        let other = b.iter().find(|(n, _)| n == name);
        match other {
            Some((_, of)) if of == field => {}
            Some(_) => return Err(format!("field {name}")),
            None => return Err(format!("only the new run has {name}")),
        }
    }
    for (name, _) in b {
        if !a.iter().any(|(n, _)| n == name) {
            return Err(format!("only the old run has {name}"));
        }
    }
    Err("the runs differ".into())
}

#[test]
fn crowded_apply_matches_old_read() {
    let (frames, foes, shots) = crowded_stream(24);
    assert!(foes >= 90, "foes {foes}");
    assert!(shots >= 200, "shots {shots}");
    let mut client = DelveClient::new(Some(1), Params::default());
    let mut names = Reader::default();
    let mut tree = None;
    let mut old = None;
    for (tick, whole, body) in &frames {
        client.snapshot(*tick, *whole, body).expect("the seat takes the snapshot");
        match old_apply(&mut names, &mut tree, *whole, body).expect("the old apply") {
            Some(run) => old = Some(run),
            None => {}
        }
        let got = client.newest().expect("a run");
        let old = old.as_ref().expect("the old run");
        if let Err(diff) = guest_diff(got, old) {
            let (fast, full, miss) = client.guest.stats();
            panic!("tick {tick} ({diff}); fast {fast} full {full} miss {miss}");
        }
    }
    let (fast, full, miss) = client.guest.stats();
    assert!(fast > full, "the crowded stream was read whole too often: fast {fast} full {full} miss {miss}");
}

/// Mean and p95 microseconds to decode and apply one tick of a crowded fight.
#[test]
#[ignore]
fn bench_crowded_apply() {
    let (frames, foes, shots) = crowded_stream(48);
    let time_old = |frames: &[(u32, bool, Vec<u8>)]| {
        let mut names = Reader::default();
        let mut tree = None;
        let mut samples = Vec::with_capacity(frames.len());
        for (tick, whole, body) in frames {
            let started = std::time::Instant::now();
            old_apply(&mut names, &mut tree, *whole, body).expect("old");
            let _ = tick;
            samples.push(started.elapsed().as_micros());
        }
        samples
    };
    let time_new = |frames: &[(u32, bool, Vec<u8>)]| {
        let mut client = DelveClient::new(Some(1), Params::default());
        let mut samples = Vec::with_capacity(frames.len());
        for (tick, whole, body) in frames {
            let started = std::time::Instant::now();
            client.snapshot(*tick, *whole, body).expect("new");
            samples.push(started.elapsed().as_micros());
        }
        let (fast, full, miss) = client.guest.stats();
        (samples, fast, full, miss.to_string())
    };
    // A warm pass, then the one the numbers come from.
    let _ = time_old(&frames);
    let _ = time_new(&frames);
    let old = time_old(&frames);
    let (new, fast, full, miss) = time_new(&frames);
    let report = |samples: &[u128]| {
        let mut sorted = samples.to_vec();
        sorted.sort_unstable();
        let mean = sorted.iter().sum::<u128>() / sorted.len() as u128;
        let p95 = sorted[(sorted.len() * 95 / 100).min(sorted.len() - 1)];
        (mean, p95)
    };
    let (old_mean, old_p95) = report(&old);
    let (new_mean, new_p95) = report(&new);
    println!(
        "crowded apply: foes={foes} shots={shots} ticks={} old mean {old_mean}us p95 {old_p95}us new mean {new_mean}us p95 {new_p95}us fast {fast} full {full} miss {miss}",
        frames.len()
    );
}

/// A knight raised in a monster's body (a skull's possession) mid-stream:
/// that knight's own browser seat keeps reading the run and sees the body.
/// A seat that could not read it froze its player as a "tiny jelly".
#[test]
fn a_possessed_knights_seat_keeps_reading_the_run() {
    use crate::rules::{EnemyKind, RoomKind};
    let mut run = Run::new(11, 1, Some("Percival"));
    run.join_seat(2, "Shlurm");
    let index = run
        .dungeon
        .rooms
        .iter()
        .position(|room| room.kind == RoomKind::Fight && !room.great())
        .expect("a fight room");
    run.enter_for_test(index);
    let mut publisher = crate::wire::Publisher::default();
    let mut names = Writer::default();
    let mut last = None;
    let mut client = DelveClient::new(Some(2), Params::default());
    for n in 0..40u32 {
        if n == 10 {
            run.players.get_mut(&2).unwrap().possessed = Some(EnemyKind::Slime);
        }
        if n > 0 {
            run.step(&BTreeMap::new());
        }
        let tree = publisher.publish(&run).expect("the run publishes");
        let seen_view = crate::wire::pace(crate::wire::seat_tree(&tree, Some(2)), last.as_ref()).into_owned();
        let mut body = Vec::new();
        let whole = last.is_none();
        if whole {
            names.value(&mut body, &seen_view);
        } else if let Some(patch) = diff(last.as_ref().unwrap(), &seen_view) {
            names.patch(&mut body, &patch);
        }
        last = Some(seen_view);
        client.snapshot(n + 1, whole, &body).unwrap_or_else(|why| panic!("tick {}: the seat refused its stream: {why:?}", n + 1));
        // Its own knight drawn as the page draws it, walking.
        let mut frame = Frame::default();
        let walk = Hands { held: 1, ..Hands::default() };
        client.draw(&view(&client, &[], 0, walk, 0.0), &mut frame);
    }
    let seen = client.newest().expect("a run");
    assert_eq!(seen.players[&2].possessed, Some(EnemyKind::Slime), "the seat sees its knight in the slime");
}

#[test]
fn wish_upgrade_picks_follow_each_seats_deck_and_update_from_patches() {
    let mut run = Run::new(7, 1, Some("Ada"));
    run.players.get_mut(&1).unwrap().wishes = 2;
    run.join_seat(2, "Bea");
    let spell = crate::rules::phrasebook::get("firebolt").unwrap();
    let hold = crate::rules::phrasebook::get("volley").unwrap();
    run.grant(1, &hold).unwrap();
    run.grant(1, &spell).unwrap();
    let mut host = Host::new(run);
    let mut c = client(Some(0));
    host.send(&mut c);
    let lines = c.picks();
    let lines: Vec<_> = lines.lines().collect();
    assert_eq!(lines[0], "Upgrade what you hold");
    assert_eq!(lines[1], format!("1\tFirebolt\tI → II\t{}\tupgrade Firebolt", spell.tiers[1].1));
    assert_eq!(lines[2], format!("2\tTriple Volley\tI → II\t{}\tupgrade Triple Volley", hold.tiers[1].1));
    host.run.players.get_mut(&1).unwrap().wishes = 0;
    host.send(&mut c);
    assert!(c.picks().contains("No wishes held: each floor brings two"));
    assert!(c.picks().lines().skip(1).filter(|l| l.contains('\t')).all(|l| l.ends_with('\t')));
    host.run.players.get_mut(&1).unwrap().wishes = 1;
    host.run.grant(1, &spell).unwrap();
    host.run.grant(1, &spell).unwrap();
    host.send(&mut c);
    // The first floor's top is the third: the fourth is shown, faint, with
    // the floor it waits for; at the fifth, deep enough down, MAX.
    assert!(c.picks().contains("\tFirebolt\tIII → IV\tIts tier IV waits for floor 2\t\n"), "{}", c.picks());
    host.run.dungeon.depth = crate::rules::FLOORS;
    host.run.grant(1, &spell).unwrap();
    host.run.grant(1, &spell).unwrap();
    host.send(&mut c);
    assert!(c.picks().contains("\tFirebolt\tV\tMAX\t"), "{}", c.picks());
    let mut other = client(Some(1));
    Host::new(host.run.clone()).send(&mut other);
    assert!(!other.picks().contains("Firebolt"), "another seat sees its own wishes");
    assert!(client(None).picks().is_empty(), "spectators have no picks");
}

#[test]
fn learned_wish_upgrade_lines_arrive_in_the_browser_on_a_run_patch() {
    let mut host = Host::new(Run::new(7, 1, Some("Ada")));
    host.run.players.get_mut(&1).unwrap().wishes = 1;
    let mut c = client(Some(0));
    host.send(&mut c);
    let mut wish = crate::rules::phrasebook::get("firebolt").unwrap();
    wish.id = "silver-bolt".into();
    wish.name = "Silver Bolt".into();
    wish.learned = true;
    host.run.grant(1, &wish).unwrap();
    host.send(&mut c);
    assert!(c.picks().contains(&format!("1\tSilver Bolt\tI → II\t{}\tupgrade Silver Bolt", wish.tiers[1].1)));
}

#[test]
fn spell_upgrades_picks_hud_and_saves_patch_only_the_owning_seat() {
    let mut run = Run::new(7, 1, Some("Ada"));
    run.players.get_mut(&1).unwrap().wishes = 3;
    run.join_seat(2, "Bea");
    assert!(run.take_up(1, "gravecaller"));
    assert!(run.take_up(2, "hammerer"));
    let mut host = Host::new(run);
    let mut own = client(Some(0));
    host.send(&mut own);
    assert!(own.picks().contains("Soul Rip\tI → II\t"));
    assert!(!own.picks().contains("Storm Hammer"));
    let words = host.run.upgrades(1).into_iter().find(|u| u.name == "Soul Rip").unwrap().words();
    host.run.offer_wish(1, &words, None).unwrap();
    host.send(&mut own);
    assert_eq!(own.newest().unwrap().players[&1].offer.as_ref().unwrap().choices[0].gives, "Soul Rip II");
    host.run.choose(1, 0).unwrap();
    host.send(&mut own);
    let run = own.newest().unwrap();
    assert_eq!(run.players[&1].spell_tier("soul-rip"), 1);
    assert!(own.picks().contains("Soul Rip\tII → III\t"));
    assert_eq!(run.hud(1, &crate::rules::hud::Keys::FIRST).unwrap().spells[3].as_ref().unwrap().name, "Soul Rip II");
    assert!(own.sheet().contains("Soul Rip II"));
    assert!(run.players[&2].spell_tiers.is_empty());
    let mut friend = client(Some(1));
    Host::new(host.run.clone()).send(&mut friend);
    assert!(friend.picks().contains("Storm Hammer\tI → II\t"));
    assert!(!friend.picks().contains("Soul Rip"));
    host.run.offer_wish(1, &words, None).unwrap();
    host.run.choose(1, 0).unwrap();
    host.send(&mut own);
    assert!(own.picks().contains("\tSoul Rip\tIII\tMAX\t"));
}

/// Times, for one tick, as the bench in the cockpit does.
struct BenchStats(Vec<std::time::Duration>);

impl BenchStats {
    fn new() -> Self {
        BenchStats(Vec::new())
    }
    fn time<T>(&mut self, f: impl FnOnce() -> T) -> T {
        let at = std::time::Instant::now();
        let out = f();
        self.0.push(at.elapsed());
        out
    }
    fn line(&mut self, what: &str) -> String {
        self.0.sort();
        let n = self.0.len().max(1);
        let ms = |d: std::time::Duration| d.as_secs_f64() * 1000.0;
        let mean = self.0.iter().map(|d| ms(*d)).sum::<f64>() / n as f64;
        let p50 = self.0.get(n / 2).map_or(0.0, |d| ms(*d));
        let p95 = self.0.get(n * 95 / 100).map_or(0.0, |d| ms(*d));
        let max = self.0.last().map_or(0.0, |d| ms(*d));
        format!("{what}: mean {mean:.3} ms, p50 {p50:.3} ms, p95 {p95:.3} ms, max {max:.3} ms (n={})", self.0.len())
    }
}

/// A four-knight fight on the third floor, kept at full pressure: the
/// heroes cannot fall and the foes mend each tick, so the counts a real
/// fight reaches stay up for the whole bench.
fn floor3_fight() -> Run {
    net_perf_fight(3, 4, false)
}

fn net_perf_fight(depth: u32, seats: u32, reduced: bool) -> Run {
    let mut run = Run::new(3, 1, Some("One"));
    for seat in 2..=seats {
        run.join_seat(seat, &format!("Knight {seat}"));
    }
    while run.dungeon.depth < depth {
        run.descend_for_test();
    }
    let fight = (0..run.dungeon.rooms.len())
        .filter(|&i| run.dungeon.rooms[i].kind == RoomKind::Fight)
        .max_by_key(|&i| run.dungeon.rooms[i].roster.len())
        .expect("a fight room on the third floor");
    run.enter_for_test(fight);
    for hero in run.players.values_mut() {
        hero.invulnerable = 1_000_000;
    }
    if reduced {
        run.tune.foe_shots = 50;
        run.tune.foe_shot_speed = 1.5;
    }
    run
}

/// Opt-in final-build fixture: one selected, fully trained companion per seat.
/// All five kinds are represented; a sixth ninja fills the real six-seat party.
fn net_perf_squad(run: &mut Run) {
    use crate::rules::squires_lodge::{self as squires, Kind, Member, Roster};
    for (&seat, hero) in &run.players {
        let member = Member {
            id: u64::from(seat), name: format!("Squire {seat}"),
            kind: Kind::ALL[(seat as usize - 1) % Kind::ALL.len()], level: 8,
        };
        run.home.squires_lodge.rosters.insert(squires::owner_key(hero), Roster {
            active: Some(member.id), members: vec![member],
        });
    }
}

fn net_perf_keep_squad(run: &mut Run) {
    for hero in run.players.values_mut() {
        if let Some(s) = &mut hero.squire {
            s.body.hp = s.body.max_hp;
        }
    }
}

#[test]
fn net_perf_squad_fixture_survives_real_six_seat_wire() {
    use crate::rules::squires_lodge::Kind;
    for depth in 3..=6 {
        for reduced in [false, true] {
            let mut run = net_perf_fight(depth, 6, reduced);
            net_perf_squad(&mut run);
            run.step(&BTreeMap::new());
            assert_eq!(run.squire_views().len(), 6);
            for kind in Kind::ALL { assert!(run.squire_views().iter().any(|s| s.kind == kind)); }
            for seat in 0..6 {
                let mut host = Host::new(run.clone());
                let mut c = client(Some(seat));
                host.send(&mut c);
                let decoded = c.newest().unwrap();
                assert_eq!(decoded.squire_views().len(), 6);
                assert!(decoded.players.values().all(|h| h.squire.as_ref().is_some_and(|s| !s.down())));
            }
        }
    }
}

fn bench_inputs(run: &Run, i: usize) -> BTreeMap<u32, Input> {
    run.players
        .iter()
        .map(|(&id, hero)| {
            let nearest = run
                .enemies
                .iter()
                .min_by(|a, b| {
                    let da = (a.x - hero.x).hypot(a.y - hero.y);
                    let db = (b.x - hero.x).hypot(b.y - hero.y);
                    da.total_cmp(&db)
                });
            let (aim_x, aim_y) = nearest.map_or((1, 0), |e| {
                let (dx, dy) = (e.x - hero.x, e.y - hero.y);
                if dx.abs() >= dy.abs() {
                    (dx.signum() as i8, 0)
                } else {
                    (0, dy.signum() as i8)
                }
            });
            let turn = (i / 20 + id as usize) % 4;
            let (move_x, move_y) = [(1, 0), (0, 1), (-1, 0), (0, -1)][turn];
            (
                id,
                Input { move_x, move_y, aim_x, aim_y, fire: true, ..Default::default() },
            )
        })
        .collect()
}

/// One browser seat's frame, end to end, on the third floor with four
/// knights: the step, the snapshot copy and the tree on the host's sim;
/// each seat's share diffed and encoded on its writer; one seat's client
/// reading each tick, drawing a frame, and (with `stage`) making the
/// stage frame a page draws.
/// `cargo test -p angel-delve --release floor3_bench -- --ignored --nocapture`
fn floor3_scene(name: &str, stage: bool) {
    net_perf_scene(name, stage, floor3_fight());
}

fn net_perf_scene(name: &str, stage: bool, mut run: Run) {
    let seats = run.players.len();
    let depth = run.dungeon.depth;
    let mut publisher = wire::Publisher::default();
    let mut viewers: Vec<(Writer, Option<Value>)> =
        (0..seats).map(|_| (Writer::default(), None)).collect();
    let squad = !run.home.squires_lodge.rosters.is_empty();
    let mut clients: Vec<_> = (0..seats).map(|seat| client(Some(seat as u32))).collect();
    if stage { clients[1].stage_props(true); }
    let (mut step, mut snap, mut tree, mut encode, mut read, mut draw, mut staged) = (
        BenchStats::new(),
        BenchStats::new(),
        BenchStats::new(),
        BenchStats::new(),
        BenchStats::new(),
        BenchStats::new(),
        BenchStats::new(),
    );
    let mut frame = Frame::default();
    let (mut bytes, mut stage_bytes) = (0usize, 0usize);
    let mut wire_sizes = Vec::new();
    let mut stage_sizes = Vec::new();
    let mut windows = vec![Vec::new(); seats];
    let mut stage_pictures: std::collections::BTreeMap<u32, (usize, usize)> = Default::default();
    let (mut foes, mut shots, mut worst_shots) = (0usize, 0usize, 0usize);
    let (warm, ticks) = (30usize, 300usize);
    for i in 0..warm + ticks {
        let timed = i >= warm;
        for e in &mut run.enemies {
            e.hp = e.max_hp.max(1);
        }
        if squad { net_perf_keep_squad(&mut run); }
        let inputs = bench_inputs(&run, i);
        if timed {
            step.time(|| run.step(&inputs));
        } else {
            run.step(&inputs);
        }
        let copy = if timed { snap.time(|| run.clone()) } else { run.clone() };
        let doc = if timed {
            tree.time(|| publisher.publish(&copy).unwrap())
        } else {
            publisher.publish(&copy).unwrap()
        };
        for (seat, (names, last)) in viewers.iter_mut().enumerate() {
            let mut out = Vec::new();
            let put = |names: &mut Writer, last: &mut Option<Value>, out: &mut Vec<u8>| {
                let view = wire::pace(wire::seat_tree(&doc, Some(seat as u32)), last.as_ref());
                let whole = match &*last {
                    None => {
                        names.value(out, &view);
                        true
                    }
                    Some(old) => {
                        if let Some(patch) = diff(old, &view) {
                            names.patch(out, &patch);
                        }
                        false
                    }
                };
                *last = Some(view.into_owned());
                whole
            };
            let whole = if timed {
                encode.time(|| put(names, last, &mut out))
            } else {
                put(names, last, &mut out)
            };
            if timed {
                bytes += out.len();
                wire_sizes.push(out.len());
                windows[seat].push(out.len());
            }
            if squad || seat == 1 {
                let client = &mut clients[seat];
                let tick = (i + 1) as u32;
                if timed {
                    read.time(|| client.snapshot(tick, whole, &out).unwrap());
                } else {
                    client.snapshot(tick, whole, &out).unwrap();
                }
            }
        }
        if squad {
            assert_eq!(run.squire_views().iter().filter(|s| !s.downed).count(), seats);
            for c in &clients { assert_eq!(c.newest().unwrap().squire_views().len(), seats); }
        }
        let client = &mut clients[1];
        let v = view(client, &[], 0, Hands::default(), 0.5);
        if timed {
            draw.time(|| client.draw(&v, &mut frame));
            if stage {
                let mut out = Vec::new();
                staged.time(|| client.stage(i as f64 * 33.3, &mut out));
                stage_bytes += out.len();
                stage_sizes.push(out.len());
                if let Ok(decoded) = angel_stage::wire::decode(&out) {
                    for p in &decoded.frame.pictures {
                        let e = stage_pictures.entry(p.id).or_insert((0, 0));
                        e.0 += 1;
                        e.1 += p.rgba.len();
                    }
                }
            }
            foes += run.enemies.len();
            shots += run.projectiles.len();
            worst_shots = worst_shots.max(run.projectiles.len());
        } else {
            client.draw(&v, &mut frame);
            if stage {
                let mut out = Vec::new();
                client.stage(i as f64 * 33.3, &mut out);
            }
        }
    }
    // These phases are separate from the client's read/draw. Sum their
    // unsorted samples to report the host's six-seat work for each tick.
    let mut host_work = BenchStats((0..ticks).map(|i| {
        step.0[i] + snap.0[i] + tree.0[i]
            + encode.0[i * seats..(i + 1) * seats].iter().sum::<std::time::Duration>()
    }).collect());
    println!(
        "FLOOR{depth} BENCH {name}: {ticks} ticks, {seats} seats, frame {}x{}, foes {:.1}, shots {:.1} (worst {worst_shots})",
        frame.w,
        frame.h,
        foes as f64 / ticks as f64,
        shots as f64 / ticks as f64,
    );
    println!("{}", host_work.line("host work (step + copy + publish + seat encodes)"));
    println!("{}", step.line("step (sim)"));
    println!("{}", snap.line("snapshot copy (sim)"));
    println!("{}", tree.line("tick tree (sim)"));
    println!("{}", encode.line("one seat diffed and encoded (writer)"));
    println!("{}", read.line("a client reads a tick"));
    println!("{}", draw.line("a client draws a frame"));
    if stage {
        println!("{}", staged.line("a client makes its stage frame"));
        println!("stage bytes per frame: {}", stage_bytes / ticks);
        let mut by_bytes: Vec<_> = stage_pictures.iter().collect();
        by_bytes.sort_by_key(|(_, (_, b))| std::cmp::Reverse(*b));
        for (id, (n, b)) in by_bytes.into_iter().take(8) {
            println!("stage picture {id}: sent {n}x, {} bytes a frame", b / ticks);
        }
        let base = angel_stage::LAYER_PICTURES;
        for (name, range) in [
            ("ground tiles", base + 0x1000..base + 0x1400),
            ("under tiles", base + 0x1400..base + 0x1800),
            ("over tiles", base + 0x1800..base + 0x1c00),
            ("screen tiles", base + 0x1c00..base + 0x2000),
            ("whole room pictures", base + 0x21..base + 0x28),
        ] {
            let (n, b) = stage_pictures
                .iter()
                .filter(|(id, _)| range.contains(id))
                .fold((0, 0), |(n, b), (_, (pn, pb))| (n + pn, b + pb));
            if n > 0 {
                println!("stage {name}: {n} sends, {} bytes a frame", b / ticks);
            }
        }
    }
    println!("bytes per tick per seat: {}", bytes / ticks / seats);
    wire_sizes.sort_unstable();
    stage_sizes.sort_unstable();
    let mut seconds: Vec<usize> = windows.iter().flat_map(|v| v.windows(crate::rules::HZ as usize).map(|w| w.iter().sum())).collect();
    seconds.sort_unstable();
    println!("NET PERF {name} floor={depth} seats={seats} wire_mean={} wire_p95={} rolling_1s_p95_mbps={:.4} stage_mean={} stage_p95={}",
        bytes / ticks / seats, wire_sizes[wire_sizes.len() * 95 / 100],
        seconds[seconds.len() * 95 / 100] as f64 * 8.0 / 1_000_000.0,
        stage_bytes / ticks, stage_sizes.get(stage_sizes.len() * 95 / 100).unwrap_or(&0));
    println!("NET MEDIAN {name} floor={depth} seats={seats} wire_p50={} rolling_1s_p50_Bps={} rolling_1s_p95_Bps={} stage_p50={}",
        wire_sizes[wire_sizes.len() / 2], seconds[seconds.len() / 2], seconds[seconds.len() * 95 / 100],
        stage_sizes.get(stage_sizes.len() / 2).unwrap_or(&0));
}

#[test]
#[ignore]
fn floor3_bench() {
    floor3_scene("pixels", false);
    floor3_scene("stage", true);
}

#[test]
fn net_perf_plain_projectiles_omit_empty_traits_and_still_read() {
    let mut run = fight();
    run.projectiles.truncate(1);
    run.projectiles[0].traits = Default::default();
    let tree = wire::publish(&run).unwrap();
    let Value::Seq(shots) = tree.field("projectiles").unwrap().get() else { panic!("shots"); };
    assert!(shots[0].field("traits").is_none(), "empty gifts cost a field for every projectile birth");
    let seen = wire::read(&tree).unwrap();
    assert_eq!(seen.projectiles[0].traits, run.projectiles[0].traits);
    assert_eq!(seen.projectiles[0].ttl, run.projectiles[0].ttl);
}

#[test]
fn net_perf_wire_figures_do_not_resend_a_whole_sheet_when_one_changes() {
    use crate::render::overworld::arena::stage_frame::{StageMaker, FIGURES};
    use std::sync::Arc;
    let run = fight();
    let mut scene = crate::render::overworld::arena::scene_for(&run, arena::NATIVE_W, arena::NATIVE_H, Some(1));
    scene.world = None;
    assert!(scene.figures.len() > 1);
    let mut maker = StageMaker::for_wire();
    let first = maker.make(&Arc::new(scene.clone()), |_| None);
    scene.figures[0].img.set(0, 0, [190, 100, 40]);
    let next = maker.make(&Arc::new(scene), |_| None);
    assert!(next.pictures.iter().all(|p| p.id != FIGURES), "one moving figure must not invalidate the crowd atlas");
    assert!(next.pictures.iter().map(|p| p.rgba.len()).sum::<usize>() < first.pictures.iter().map(|p| p.rgba.len()).sum::<usize>());
}

#[test]
fn net_perf_wire_torches_animate_without_new_ground_pixels() {
    arena::draw_parts_outside(true);
    let run = net_perf_fight(3, 6, false);
    let mut host = Host::new(run);
    let mut c = client(Some(0));
    c.stage_props(true);
    let mut out = Vec::new();
    let mut frame = Frame::default();
    // Read a new tick whole so interpolation cannot keep showing tick zero.
    for tick in [0, 12] {
        host.run.tick = tick;
        let mut publisher = wire::Publisher::default();
        let tree = publisher.publish(&host.run).unwrap();
        let mut names = Writer::default();
        let mut body = Vec::new();
        names.value(&mut body, &tree);
        c.snapshot(tick as u32 + 1, true, &body).unwrap();
        c.draw(&view(&c, &[], 0, Hands::default(), 0.0), &mut frame);
        out.clear();
        assert!(c.stage(tick as f64 * 1000.0 / HZ as f64, &mut out));
        if tick > 0 {
            let decoded = angel_stage::wire::decode(&out).unwrap();
            assert!(decoded.frame.pictures.iter().all(|p| !(angel_stage::LAYER_PICTURES + 0x1000..angel_stage::LAYER_PICTURES + 0x1400).contains(&p.id)), "torch flicker must be an animated light, not a room upload");
        }
    }
}

/// Export repeatable real-wire soak inputs and a pixel proof without a host.
/// DELVE_NET_FIXTURES=/owned/path cargo test ... net_perf_export -- --ignored
#[test]
#[ignore]
fn net_perf_export() {
    let dir = std::path::PathBuf::from(std::env::var("DELVE_NET_FIXTURES").expect("set DELVE_NET_FIXTURES"));
    std::fs::create_dir_all(&dir).unwrap();
    for depth in 3..=6 {
        for reduced in [false, true] {
            let mut run = net_perf_fight(depth, 6, reduced);
            for foe in &mut run.enemies {
                foe.hp = 100_000_000;
                foe.max_hp = foe.hp;
            }
            let name = format!("heavy-f{depth}-{}.json", if reduced { "reduced" } else { "default" });
            std::fs::write(dir.join(name), serde_json::to_vec(&run).unwrap()).unwrap();
        }
    }
    if std::env::var("DELVE_NET_SQUIRES").as_deref() == Ok("1") {
        for depth in 3..=6 {
            for reduced in [false, true] {
                let mut run = net_perf_fight(depth, 6, reduced);
                net_perf_squad(&mut run);
                run.step(&BTreeMap::new());
                let label = if reduced { "reduced" } else { "default" };
                std::fs::write(dir.join(format!("squad-f{depth}-{label}.json")), serde_json::to_vec(&run).unwrap()).unwrap();
                for i in 0..90 {
                    for foe in &mut run.enemies { foe.hp = foe.max_hp.max(1); }
                    net_perf_keep_squad(&mut run);
                    run.step(&bench_inputs(&run, i));
                }
                assert_eq!(run.squire_views().iter().filter(|s| !s.downed).count(), 6);
                // Publish/read before painting: proof of the client's actual replicated fight.
                let mut host = Host::new(run);
                let mut c = client(Some(0));
                host.send(&mut c);
                let decoded = c.newest().unwrap();
                assert_eq!(decoded.squire_views().len(), 6);
                let img = arena::in_seat(|| arena::frame_for(decoded, arena::NATIVE_W, arena::NATIVE_H, Some(1)));
                let rgba: Vec<u8> = img.pixels().flat_map(|[r,g,b]| [r,g,b,255]).collect();
                image::save_buffer(dir.join(format!("squad-f{depth}-{label}.png")), &rgba, img.w as u32, img.h as u32, image::ColorType::Rgba8).unwrap();
                println!("SQUAD PNG floor={depth} tune={label} seats={} squires={} foes={} shots={}", decoded.players.len(), decoded.squire_views().len(), decoded.enemies.len(), decoded.projectiles.len());
            }
        }
    }
    let mut run = net_perf_fight(3, 6, false);
    for i in 0..90 {
        for foe in &mut run.enemies { foe.hp = foe.max_hp.max(1); }
        run.step(&bench_inputs(&run, i));
    }
    let img = arena::in_seat(|| arena::frame_for(&run, arena::NATIVE_W, arena::NATIVE_H, Some(1)));
    let rgba = arena::stage_frame::rgba_of(&img);
    image::save_buffer(dir.join("floor3-heavy-six.png"), &rgba, img.w as u32, img.h as u32, image::ColorType::Rgba8).unwrap();
}

/// Same seeded heavy fight, six seats, all client phases and stage upload bytes.
/// Wire Mbps is the p95 of rolling one-second windows (30 Hz), not stage uploads.
/// Preserve floor3_bench above as the historical four-seat control.
#[test]
#[ignore]
fn net_perf_six_seat_bench() {
    for depth in 3..=6 {
        for reduced in [false, true] {
            let name = if reduced { "50pct-1.5x" } else { "default" };
            let mut run = net_perf_fight(depth, 6, reduced);
            if std::env::var("DELVE_NET_SQUIRES").as_deref() == Ok("1") {
                net_perf_squad(&mut run);
                net_perf_scene(&format!("squad-{name}"), true, run);
            } else {
                net_perf_scene(name, true, run);
            }
        }
    }
}

#[test]
fn alehouse_lamp_opens_only_the_claimants_existing_client_wish_input() {
    let mut run = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    run.walk_alehouse();
    run.floor_plates.clear();
    run.step(&BTreeMap::new());
    let mut mine = (Host::new(run.clone()), client(Some(0)));
    let mut theirs = (Host::new(run.clone()), client(Some(1)));
    let read = |h: &mut Host, c: &mut DelveClient, run: &Run| {
        h.run = run.clone();
        h.send(c);
        let mut lines = Vec::new();
        c.lines(&mut lines);
        lines.into_iter().filter(|l| l == "ask").collect::<Vec<_>>()
    };
    read(&mut mine.0, &mut mine.1, &run);
    read(&mut theirs.0, &mut theirs.1, &run);
    let h = run.players.get_mut(&1).unwrap();
    let at = crate::rules::alehouse::LAMP_STAND;
    (h.x, h.y) = (at.0 * crate::rules::TILE_UNITS, at.1 * crate::rules::TILE_UNITS);
    for _ in 0..crate::rules::home::BUY_HOLD {
        run.step(&BTreeMap::from([(1, crate::rules::Input { interact: true, ..Default::default() })]));
    }
    assert_eq!(read(&mut mine.0, &mut mine.1, &run), ["ask"]);
    assert!(read(&mut mine.0, &mut mine.1, &run).is_empty());
    assert!(read(&mut theirs.0, &mut theirs.1, &run).is_empty());
}

#[test]
fn team_h_key_is_separate_from_dash_and_survives_client_command() {
    let mut host = Host::new(fight());
    let mut c = client(Some(0));
    host.send(&mut c);
    let controls: serde_json::Value = serde_json::from_str(CONTROLS).unwrap();
    assert_eq!(controls["keys"]["KeyH"],"TEAM");
    let bits = angel_net::input::button::named("TEAM").unwrap();
    let cmd = c.command(&Hands { held:bits|button::RIGHT,pressed:bits,x:127,..Hands::default() });
    assert!(cmd.input().team);
    assert!(!cmd.input().dash);
    assert_eq!(cmd.input().move_x,1);
    assert!(!c.command(&held(button::OK)).input().team);
    assert!(!c.command(&held(button::SHIFT)).input().team);
}

#[test]
fn calm_screen_is_a_local_viewer_preference_not_a_command() {
    let mut a = client(Some(0));
    let b = client(Some(1));
    assert!(!a.viewer.calm_screen && !b.viewer.calm_screen);
    assert_eq!(a.set_knob("calm_screen", 1.0), Some(1.0));
    assert!(a.viewer.calm_screen && !b.viewer.calm_screen);
    let knob = a.knobs().into_iter().find(|k| k.name == "calm_screen").unwrap();
    assert_eq!(knob.side, Side::Client);
    let mut hands = held(button::SHIFT | key::MAP_KEY);
    hands.pressed = key::MAP_KEY;
    let cmd = a.command(&hands);
    assert!(!a.viewer.calm_screen);
    assert_eq!(cmd, DelveCmd::neutral());
}

#[test]
fn calm_screen_draws_only_its_own_flat_frame_and_scene() {
    let mut run = crate::rules::feel::testing::quiet();
    run.shake = 12;
    run.tick = 3;
    run.feel.deaths.push(crate::rules::feel::Death { x: 20.0, y: 14.0, r: 2.0,
        ttl: crate::rules::feel::DEATH_TICKS, kind: Some(crate::rules::EnemyKind::Skeleton) });
    let mut host = Host::new(run);
    let mut c = client(Some(0));
    host.send(&mut c);
    c.exact = true;
    let mut loud = Frame::default();
    c.draw(&view(&c, &[], 0, Hands::default(), 0.0), &mut loud);
    c.set_knob("calm_screen", 1.0);
    let mut calm = Frame::default();
    c.draw(&view(&c, &[], 0, Hands::default(), 0.0), &mut calm);
    assert_ne!(loud.px, calm.px);
    assert!(calm.px.iter().all(|p| [p >> 16 & 255, p >> 8 & 255, p & 255].into_iter().all(|c| c <= 150)));
    c.set_knob("calm_screen", 0.0);
    let mut back = Frame::default();
    c.draw(&view(&c, &[], 0, Hands::default(), 0.0), &mut back);
    assert_eq!(back.px, loud.px, "the art caches must not retain calm tints");
    c.set_knob("calm_screen", 1.0);
    arena::draw_parts_outside(true);
    c.draw(&view(&c, &[], 0, Hands::default(), 0.0), &mut calm);
    let scene = c.scene.as_ref().unwrap();
    assert_eq!(scene.impact, 0.0);
    assert!(scene.figures.iter().all(|f| !f.additive && f.bodies.is_empty() && f.paints.is_empty()));
    assert!(!crate::render::viewer::calm(), "scope ends after the picture");
    arena::draw_parts_outside(false);
}

#[test]
fn mirror_creator_owns_navigation_confirm_close_and_never_gameplay_or_menus() {
    let mut host = Host::new(fight());
    host.run.players.get_mut(&1).unwrap().mirror.open = true;
    let mut c = client(Some(0));
    host.send(&mut c);
    // Even stale UI requests cannot escape into an activity menu or ballot.
    c.menu = true;
    c.leaving = 3;
    c.skipping = 3;
    c.ballot.open = true;
    c.vote(Action::Toggle);
    for (bits, action) in [
        (button::UP, 1), (button::DOWN, 2),
        (button::LEFT, 3), (button::RIGHT, 4),
        (button::AIM_UP, 1), (button::AIM_DOWN, 2),
        (button::AIM_LEFT, 3), (button::AIM_RIGHT, 4),
        (button::USE, 5), (button::OK, 5), (button::MENU, 6),
        (button::FIRE | key::BALLOT, 0),
    ] {
        let cmd = c.command(&Hands { held: bits, pressed: bits, ..Hands::default() });
        assert_eq!(cmd.held, Input::default());
        assert_eq!(cmd.input(), Input { mirror: action, ..Input::default() });
        assert!(!c.menu && !c.ballot.open && c.ballot.queue.is_empty());
        assert_eq!((c.leaving, c.skipping), (0, 0));
    }
    for (hands, axes) in [
        (Hands { y: -STICK_LEAN, ..Hands::default() }, (0, -1, 0, 0)),
        (Hands { y: STICK_LEAN, ..Hands::default() }, (0, 1, 0, 0)),
        (Hands { x: -STICK_LEAN, ..Hands::default() }, (-1, 0, 0, 0)),
        (Hands { x: STICK_LEAN, ..Hands::default() }, (1, 0, 0, 0)),
        (Hands { aim_y: -STICK_LEAN, ..Hands::default() }, (0, 0, 0, -1)),
        (Hands { aim_y: STICK_LEAN, ..Hands::default() }, (0, 0, 0, 1)),
        (Hands { aim_x: -STICK_LEAN, ..Hands::default() }, (0, 0, -1, 0)),
        (Hands { aim_x: STICK_LEAN, ..Hands::default() }, (0, 0, 1, 0)),
        (Hands { x: STICK_LEAN - 1, y: 1 - STICK_LEAN,
            aim_x: STICK_LEAN - 1, aim_y: 1 - STICK_LEAN, ..Hands::default() }, (0, 0, 0, 0)),
    ] {
        let (move_x, move_y, aim_x, aim_y) = axes;
        let expected = Input { move_x, move_y, aim_x, aim_y, ..Input::default() };
        for _ in 0..3 {
            let cmd = c.command(&Hands {
                held: button::FIRE | button::ALT | button::SHIFT | key::BALLOT,
                ..hands
            });
            assert_eq!(cmd.held, expected, "only navigation axes are held");
            assert_eq!(cmd.pressed, Input::default(), "a hold is not a mirror tap");
            assert_eq!(cmd.input(), expected);
            assert_eq!(cmd.held_only().input(), expected, "transport guesses keep the repeat axes");
        }
        let cmd = c.command(&Hands { pressed: button::LEFT, ..hands });
        assert_eq!(cmd.held, expected);
        assert_eq!(cmd.pressed, Input { mirror: 3, ..Input::default() }, "true taps stay explicit");
        assert_eq!(cmd.held_only().input(), expected, "transport guesses never replay taps");
    }
    assert_eq!(c.command(&tap(button::MENU | button::OK)).input().mirror, 6, "close wins");
}

#[test]
fn mirror_creator_suppresses_prediction_and_names_only_its_own_seats_ui() {
    let mut host = Host::new(fight());
    host.run.join_seat(2, "Other");
    {
        let hero = host.run.players.get_mut(&1).unwrap();
        hero.mirror.open = true;
        hero.mirror.row = 9;
    }
    let mut c = client(Some(0));
    host.send(&mut c);
    let origin = host.run.players[&1].clone();
    let pending = [(1, DelveCmd {
        held: Input { move_x: 1, fire: true, ..Input::default() }, ..Default::default()
    })];
    let hands = held(button::RIGHT | button::FIRE);
    assert!(c.walk(&hands).is_none());
    assert!(c.coming(&hands).is_none());
    let v = view(&c, &pending, 0, hands, 0.9);
    let drawn = c.own(&v).unwrap();
    assert_eq!((drawn.x, drawn.y), (origin.x, origin.y));
    assert!(c.loosed.is_empty());
    assert!(c.picks().contains("short name"));
    let metadata: serde_json::Value = serde_json::from_str(c.controls()).unwrap();
    assert_eq!(metadata["text"]["title"], "Mirror name");
    assert_eq!(metadata["text"]["kind"], 2);
    let mut lines = Vec::new();
    c.lines(&mut lines);
    assert!(lines.iter().any(|line| line.contains("press T, type a short name")));

    let mut other = client(Some(1));
    let mut other_host = Host::new(host.run.clone());
    other_host.send(&mut other);
    assert!(!other.picks().contains("Mirror name"));
    assert_eq!(other.controls(), CONTROLS);
    let cmd = other.command(&tap(button::USE));
    assert!(cmd.input().interact && cmd.input().mirror == 0, "only the owning seat is captured");

    host.run.players.get_mut(&1).unwrap().mirror.open = false;
    host.send(&mut c);
    assert!(!c.picks().contains("Mirror name"));
    assert_eq!(c.controls(), CONTROLS);
    assert!(c.command(&tap(button::USE)).input().interact, "normal controls resume");
}

#[test]
fn mirror_creator_player_two_uses_its_own_controls_and_name_ui() {
    let mut host = Host::new(fight());
    host.run.join_seat(2, "Other");
    let hero = host.run.players.get_mut(&2).unwrap();
    hero.mirror.open = true;
    hero.mirror.row = 9;
    let mut c = client(Some(1));
    host.send(&mut c);
    assert!(c.sheet().contains("P2 couch: IJKL move · \\ choose · Delete close"));
    assert!(c.sheet().contains("arrows/WASD move"), "joined P2 keeps ordinary controls");
    assert!(c.picks().contains("Mirror name"));
    assert_eq!(c.controls(), NAME_CONTROLS);
    assert_eq!(c.command(&tap(button::OK)).pressed.mirror, 5);
    assert_eq!(c.command(&tap(button::MENU)).pressed.mirror, 6);
    let cmd = c.command(&held(button::DOWN | button::FIRE));
    assert_eq!(cmd.held, Input { move_y: 1, ..Input::default() });
    assert_eq!(cmd.pressed, Input::default());

    host.run.players.get_mut(&2).unwrap().mirror.open = false;
    host.send(&mut c);
    assert_eq!(c.controls(), CONTROLS);
    assert!(!c.sheet().contains("MIRROR  "));
    assert!(c.command(&tap(button::USE)).input().interact);
}

#[test]
fn mirror_creator_shared_screen_prioritizes_open_panels_then_room_location() {
    let mut run = fight();
    run.join_seat(2, "Other");
    for (one_outside, two_outside) in [(false, false), (true, false), (false, true), (true, true)] {
        for (id, outside) in [(1, one_outside), (2, two_outside)] {
            if outside {
                run.exits.outside.insert(id, crate::rules::exits::Entry { at: 0, party: BTreeMap::new() });
            } else {
                run.exits.outside.remove(&id);
            }
        }
        for (one, two, expected) in [
            (false, false, if one_outside && !two_outside { 2 } else { 1 }),
            (true, false, 1),
            (false, true, 2),
            (true, true, 1),
        ] {
            run.players.get_mut(&1).unwrap().mirror.open = one;
            run.players.get_mut(&2).unwrap().mirror.open = two;
            assert_eq!(screen_knight(&run, true), expected);
            assert_eq!(screen_knight(&run, false), 1, "solo/hosted never selects a couch knight");
        }
    }
    run.players.remove(&2);
    run.exits.outside.remove(&2);
    run.players.get_mut(&1).unwrap().mirror.open = false;
    assert_eq!(screen_knight(&run, true), 1, "no missing P2 panel");
}

#[test]
fn squires_same_kind_shot_does_not_take_the_owners_prediction_ghost() {
    let (mut host, mut c) = at_the_range();
    let key = home::Home::knight_key(&host.run.players[&1]);
    host.run.home.squire_hall.rosters.insert(
        key,
        crate::realm::squires::Roster {
            active: Some(1),
            squires: vec![crate::realm::squires::Squire {
                id: 1,
                name: "Pip".into(),
                kind: crate::realm::squires::Kind::Ranger,
                level: 1,
                talent: 1,
            }],
        },
    );
    host.run.enter_for_test(host.run.at);
    host.run.calm_for_test();
    let h = host.run.players[&1].clone();
    let companion = &mut host.run.squire_hall.companions[0];
    companion.body.x = h.x;
    companion.body.y = h.y;
    companion.body.cooldown = 0;
    // A frozen target north: the ranger and the bow fire the same kind,
    // close enough to compete for the same answered ghost.
    host.run
        .spawn_at_for_test(crate::rules::EnemyKind::Slime, h.x, h.y - 6.0);
    host.run.enemies.last_mut().unwrap().frozen = 1000;
    host.send(&mut c);
    let mut frame = Frame::default();
    c.draw(
        &view(&c, &[(1, aim_up())], 0, Hands::default(), 0.5),
        &mut frame,
    );
    assert_eq!(own_drawn(&c).len(), 1);
    host.step(&[(0, aim_up())]);
    let companion_shot = host
        .run
        .projectiles
        .iter()
        .position(|p| p.traits.squire)
        .unwrap();
    assert_eq!(
        host.run.projectiles[companion_shot].kind,
        crate::rules::Shot::Arrow
    );
    // Publish the companion first: a bad classification would steal the
    // ghost before the actual bow shot has a chance to take it over.
    host.run.projectiles.swap(0, companion_shot);
    host.send(&mut c);
    c.draw(&view(&c, &[], 1, Hands::default(), 0.5), &mut frame);
    assert_eq!(own_drawn(&c).len(), 1, "only the knight's arm is predicted");
    let other = c
        .newest()
        .unwrap()
        .projectiles
        .iter()
        .find(|p| p.traits.squire)
        .unwrap();
    assert_eq!(other.traits.owner, 1);
    assert!(!other.loosed_by(1));
    assert_eq!(
        c.drawn_all.iter().filter(|s| !s.0).count(),
        1,
        "companion is drawn once as an ordinary server shot"
    );
    let mut counts = Vec::new();
    c.dev_stats(&mut counts);
    assert!(
        counts.contains(&("shot_handovers", 1.0)) && counts.contains(&("shot_ghosts", 0.0)),
        "{counts:?}"
    );
}

/// Replay a saved run (`REPLAY_RUN=<run.json>`) through a page at every seat
/// for thirty seconds, every knight walking, each frame drawn as a browser
/// seat draws it: a panic here is one a player's page would stop on.
#[test]
#[ignore]
fn replay_a_saved_run_through_every_seats_page() {
    let Some(path) = std::env::var_os("REPLAY_RUN") else {
        return;
    };
    let run: Run = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let ids: Vec<u32> = run.players.keys().copied().collect();
    let mut hosts: Vec<(Host, DelveClient)> = (0..crate::rules::MAX_PLAYERS)
        .map(|seat| (Host::new(run.clone()), client(Some(seat)).exact()))
        .collect();
    let mut frame = Frame::default();
    for t in 0..30 * HZ {
        let inputs: BTreeMap<u32, crate::rules::Input> = ids
            .iter()
            .map(|&id| (id, crate::rules::Input { move_y: if (t / 45) % 2 == 0 { -1 } else { 1 }, ..Default::default() }))
            .collect();
        for (host, c) in &mut hosts {
            host.run.step(&inputs);
            host.send(c);
            c.draw(&view(c, &[], 0, Hands::default(), 0.0), &mut frame);
        }
    }
}
