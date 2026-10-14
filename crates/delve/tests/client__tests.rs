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
    let c = DelveClient::boot(&welcome(Some(1), &Params { art_v2: false }.encode())).unwrap();
    assert_eq!((c.size(), c.seat()), ((384, 224), Some(1)));
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
fn a_press_during_the_floors_plate_dismisses_it_and_does_nothing_else() {
    let mut run = fight();
    let plate = crate::rules::floor_plates::Playing { kind: crate::rules::floor_plates::Kind::Crypt, depth: 1, age: 0 };
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
    assert_eq!((frame.w, frame.h), (384, 224));
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
    let direct = arena::frame_for(c.newest().unwrap(), 384, 224, Some(1));
    let paper = |w: u32| if w >> 24 == 0 { 0xff00_0000 } else { w };
    assert!(frame.px.iter().zip(direct.words()).all(|(&a, &b)| a == paper(b)), "the newest tick, drawn as the renderer draws it");
    assert!(frame.px.iter().all(|&p| p >> 24 == 0xff), "opaque");
    let mut lines = Vec::new();
    c.lines(&mut lines);
    assert_eq!(lines, vec![format!("music music-{}", c.newest().unwrap().music().unwrap())]);
    lines.clear();
    c.lines(&mut lines);
    assert!(lines.is_empty(), "said once");
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
    run.walk_kingdom(Some(geography::keep_stand()));
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
