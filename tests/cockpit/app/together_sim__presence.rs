//! Who is here: a friend's knight while its page loads, while its link is
//! gone and when it comes back; a room holding for the party to arrive;
//! and a host who plays no knight. The host's server is stepped by hand;
//! the friend is a page's socket, spoken to byte for byte.
use super::*;
use crate::drive::together_shooter::{EnemyKind, Hold, Phase, RoomKind};
use angel_net::codec::Bytes;
use angel_net::proto::{PROTOCOL, ToClient, ToServer};
use angel_serve::client::WsClient;
use angel_serve::ws::Message;

const KEY: &str = "PRESENCEKEY";

/// A fight room with foes about the party.
fn fight() -> Run {
    let mut run = Run::new(3, 9, None);
    let fight = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    run.enter_for_test(fight);
    run.calm_for_test();
    let (x, y) = (run.players[&1].x, run.players[&1].y);
    for (i, kind) in [EnemyKind::Skeleton, EnemyKind::Imp, EnemyKind::Bat, EnemyKind::Flesher]
        .into_iter()
        .enumerate()
    {
        run.spawn_at_for_test(kind, x + 6.0 + i as f32 * 2.0, y - 4.0);
    }
    run.phase = Phase::Fighting;
    run
}

fn hosting(watching: bool) -> Controls {
    Controls {
        active: true,
        keyboard: true,
        hosted: true,
        watching,
        ..Default::default()
    }
}

/// The run hosted for two friends behind `KEY`.
fn hosted(run: Run, watching: bool) -> Sim {
    let mut sim = Sim::from(Some(run));
    sim.host(Door {
        bind: "127.0.0.1:0".parse().unwrap(),
        seats: 3,
        key: Some(KEY.into()),
        bundle: None,
    })
    .unwrap();
    sim.set_controls(hosting(watching));
    sim
}

/// A page's seat: its hello, and from then a command each pump saying the
/// newest tick it has (or, `stale`, the one it had then).
struct Friend {
    ws: WsClient,
    seat: Option<u32>,
    token: String,
    newest: u32,
    seq: u32,
    held: Input,
    /// Pressed with the next command only.
    pressed: Input,
    /// Commands go out (a page in view); none (still loading, hidden).
    sending: bool,
    /// The tick the commands say was applied, held while set.
    stale: Option<u32>,
}

impl Friend {
    fn open(sim: &Sim, resume: &str) -> Friend {
        let addr = sim.server().unwrap().addr();
        let mut ws = WsClient::connect(&format!("ws://{addr}/ws"), Duration::from_secs(5)).unwrap();
        ws.send(
            &ToServer::Hello {
                protocol: PROTOCOL,
                game: angel_delve::wire::GAME,
                join: KEY,
                resume,
                name: "Page",
                caps: 0,
            }
            .to_vec(),
        )
        .unwrap();
        Friend {
            ws,
            seat: None,
            token: String::new(),
            newest: 0,
            seq: 0,
            held: Input::default(),
            pressed: Input::default(),
            sending: true,
            stale: None,
        }
    }

    fn knight(&self) -> u32 {
        angel_delve::wire::knight_of(self.seat.expect("seated"))
    }

    /// Its command, if it sends, then what arrived within a few ms.
    fn pump(&mut self) {
        if self.seat.is_some() && self.sending {
            self.seq += 1;
            let mut body = Vec::new();
            DelveCmd {
                held: self.held,
                pressed: std::mem::take(&mut self.pressed),
            }
            .encode(&mut body);
            let applied = self.stale.unwrap_or(self.newest);
            self.ws
                .send(&ToServer::Cmds { applied, cmds: vec![(self.seq, &body[..])] }.to_vec())
                .unwrap();
        }
        while let Ok(Some(message)) = self.ws.recv(Duration::from_millis(2)) {
            let Message::Binary(bytes) = message else {
                continue;
            };
            match ToClient::decode(&bytes).unwrap() {
                ToClient::Welcome(w) => {
                    self.seat = w.seat;
                    self.token = w.seat_token.to_string();
                }
                ToClient::Snapshot { tick, .. } => self.newest = self.newest.max(tick),
                _ => {}
            }
            let _ = Bytes::new(&bytes);
        }
    }
}

/// Step the hosted run in time with the friend, `ticks` ticks of it, with
/// the friend's news taken in as the cockpit would (its knight joins).
fn play(sim: &mut Sim, friends: &mut [&mut Friend], ticks: u32) -> Vec<News> {
    let mut told = Vec::new();
    for _ in 0..ticks {
        for friend in friends.iter_mut() {
            friend.pump();
        }
        for news in sim.drain_news() {
            if let News::Joined { knight, name } = &news
                && let Some(mut run) = sim.as_mut()
            {
                run.join_seat(*knight, name);
            } else {
                told.push(news);
            }
        }
        std::thread::sleep(STEP);
        let now = Instant::now();
        if let Some(seat) = sim.seat(1) {
            seat.tick(now);
        }
        sim.pump(now);
    }
    sim.latch();
    told
}

fn run(sim: &Sim) -> &Run {
    sim.as_ref().unwrap()
}

#[test]
fn a_friend_whose_page_never_loads_stands_as_stone_and_cannot_die() {
    let mut sim = hosted(fight(), false);
    let mut page = Friend::open(&sim, "");
    // Seated, but its page never shows the game: it says nothing more.
    page.sending = false;
    play(&mut sim, &mut [&mut page], 10);
    let knight = page.knight();
    let full = run(&sim).players[&knight].hp;
    assert!(run(&sim).players[&knight].away, "a page still loading is away");
    play(&mut sim, &mut [&mut page], 30 * 12);
    let hero = &run(&sim).players[&knight];
    assert!(hero.away && hero.stone, "stone while away");
    assert_eq!(hero.hp, full, "twelve seconds in a fight, untouched");
    assert!(run(&sim).players[&1].hp < run(&sim).players[&1].max_hp, "the fight was real: the host's knight was hit");
}

#[test]
fn a_friend_gone_quiet_turns_to_stone_and_comes_back_on_resuming() {
    let mut run0 = fight();
    run0.enemies.clear();
    let mut sim = hosted(run0, false);
    let mut page = Friend::open(&sim, "");
    play(&mut sim, &mut [&mut page], 15);
    let knight = page.knight();
    assert!(!run(&sim).players[&knight].away, "a page playing is here");
    page.held = Input { move_x: 1, ..Input::default() };
    let x = run(&sim).players[&knight].x;
    play(&mut sim, &mut [&mut page], 10);
    assert!(run(&sim).players[&knight].x > x, "it walks");
    // The link goes quiet: a second later the knight is stone.
    page.sending = false;
    play(&mut sim, &mut [&mut page], angel_serve::AWAY_AFTER_TICKS + 5);
    assert!(run(&sim).players[&knight].stone, "stone after a second of nothing");
    // The socket is gone: the seat is held for a resume, the knight stone.
    let token = page.token.clone();
    drop(page);
    play(&mut sim, &mut [], 15);
    assert!(run(&sim).players[&knight].away);
    // The same page comes back with its token and plays the same knight.
    let mut back = Friend::open(&sim, &token);
    back.held = Input { move_x: -1, ..Input::default() };
    play(&mut sim, &mut [&mut back], 15);
    assert_eq!(back.knight(), knight, "the same seat");
    let hero = &run(&sim).players[&knight];
    assert!(!hero.away && !hero.stone, "back, and itself again");
    let x = hero.x;
    play(&mut sim, &mut [&mut back], 10);
    assert!(run(&sim).players[&knight].x < x, "it walks again");
}

#[test]
fn a_new_room_holds_until_the_page_has_it_and_no_longer_than_the_cap() {
    let mut sim = hosted(fight(), false);
    let mut page = Friend::open(&sim, "");
    play(&mut sim, &mut [&mut page], 15);
    assert_eq!(run(&sim).held, Hold::Open);
    let rooms: Vec<usize> = (0..run(&sim).dungeon.rooms.len())
        .filter(|&i| run(&sim).dungeon.rooms[i].kind == RoomKind::Fight && i != run(&sim).at)
        .collect();
    // The page stops taking ticks in (its applied stands still), and the
    // party goes into another room.
    page.stale = Some(page.newest);
    if let Some(mut r) = sim.as_mut() {
        r.enter_for_test(rooms[0]);
    }
    play(&mut sim, &mut [&mut page], 6);
    assert_eq!(run(&sim).held, Hold::Arriving, "the room waits for the page");
    let foes: Vec<(f32, f32)> = run(&sim).enemies.iter().map(|e| (e.x, e.y)).collect();
    play(&mut sim, &mut [&mut page], 20);
    assert_eq!(run(&sim).held, Hold::Arriving);
    assert_eq!(
        run(&sim).enemies.iter().map(|e| (e.x, e.y)).collect::<Vec<_>>(),
        foes,
        "its foes wait"
    );
    // A page that never shows it holds nobody past the cap.
    play(&mut sim, &mut [&mut page], ARRIVAL_CAP);
    assert_eq!(run(&sim).held, Hold::Open, "the cap lets the room go");
    // The next room: the page takes its ticks in, and the hold ends as
    // soon as it has the room.
    page.stale = None;
    play(&mut sim, &mut [&mut page], 10);
    if let Some(mut r) = sim.as_mut() {
        r.enter_for_test(rooms[1]);
    }
    play(&mut sim, &mut [&mut page], 1);
    let mut held = 0;
    for _ in 0..30 {
        play(&mut sim, &mut [&mut page], 1);
        if run(&sim).held == Hold::Arriving {
            held += 1;
        }
    }
    assert!(held < 15, "the room held {held} ticks for a page taking it in");
    assert_eq!(run(&sim).held, Hold::Open);
}

#[test]
fn a_host_without_a_knight_hosts_friends_and_the_room_waits_when_nobody_plays() {
    let mut sim = hosted(fight(), true);
    let mut page = Friend::open(&sim, "");
    play(&mut sim, &mut [&mut page], 15);
    let knight = page.knight();
    assert_eq!(knight, 2, "friends keep their numbering: knight 1 is the host's statue");
    assert!(run(&sim).players[&1].stone && run(&sim).players[&1].away, "the host plays no knight");
    assert!(!run(&sim).players[&knight].stone);
    assert_eq!(run(&sim).held, Hold::Open, "a friend plays: the fight is on");
    page.held = Input { move_x: 1, ..Input::default() };
    let x = run(&sim).players[&knight].x;
    play(&mut sim, &mut [&mut page], 10);
    assert!(run(&sim).players[&knight].x > x, "the friend plays");
    // The friend goes: nobody is here, and the room waits.
    page.sending = false;
    play(&mut sim, &mut [&mut page], angel_serve::AWAY_AFTER_TICKS + 5);
    assert_eq!(run(&sim).held, Hold::Empty);
    let foes: Vec<(f32, f32)> = run(&sim).enemies.iter().map(|e| (e.x, e.y)).collect();
    let hp = run(&sim).players[&knight].hp;
    play(&mut sim, &mut [&mut page], 30 * 4);
    assert_eq!(run(&sim).enemies.iter().map(|e| (e.x, e.y)).collect::<Vec<_>>(), foes);
    assert_eq!(run(&sim).players[&knight].hp, hp);
}

/// The tree a client of `sim` reads: the newest tick, as published.
fn sent(sim: &Sim) -> Run {
    angel_delve::wire::read(&angel_delve::wire::publish(run(sim)).unwrap()).unwrap()
}

#[test]
fn a_host_without_a_knight_is_on_no_screen_and_a_friend_who_says_goodbye_leaves() {
    let mut sim = hosted(fight(), true);
    let mut a = Friend::open(&sim, "");
    let mut b = Friend::open(&sim, "");
    play(&mut sim, &mut [&mut a, &mut b], 20);
    let (ka, kb) = (a.knight(), b.knight());
    // Two people play: the party is the two of them, and no screen shows
    // the host's place.
    assert_eq!(run(&sim).party_len(), 2);
    assert!(run(&sim).players[&1].absent);
    let seen = sent(&sim);
    assert_eq!(seen.players.keys().copied().collect::<Vec<_>>(), vec![ka, kb]);
    let two: Vec<u32> = run(&sim).enemies.iter().map(|e| e.max_hp).collect();
    assert!(!two.is_empty());
    // One says goodbye: their knight leaves the party at once.
    b.ws.send(&ToServer::Bye.to_vec()).unwrap();
    let told = play(&mut sim, &mut [&mut a], 10);
    assert!(told.contains(&News::Left { knight: kb }));
    assert!(!run(&sim).players.contains_key(&kb), "gone, not a statue");
    assert_eq!(run(&sim).party_len(), 1);
    let one: Vec<u32> = run(&sim).enemies.iter().map(|e| e.max_hp).collect();
    assert_eq!(one, two.iter().map(|hp| hp / 2).collect::<Vec<_>>(), "foes sized for one, from two");
    assert!(run(&sim).players.contains_key(&ka));
}

#[test]
fn knights_no_seat_holds_leave_the_party_after_a_while() {
    // A run a restarted host restored: knights 2 to 4 from before, their
    // players' seats gone with the old server.
    let mut restored = fight();
    restored.enemies.clear();
    for (id, name) in [(2, "Old Two"), (3, "Old Three"), (4, "Old Four")] {
        restored.join_seat(id, name);
    }
    let mut sim = hosted(restored, true);
    let mut page = Friend::open(&sim, "");
    play(&mut sim, &mut [&mut page], 20);
    let mine = page.knight();
    assert_eq!(run(&sim).players.len(), 4, "statues for a while, in case they come back");
    play(&mut sim, &mut [&mut page], UNSEATED_LEAVE);
    let knights: Vec<u32> = run(&sim).players.keys().copied().collect();
    assert_eq!(knights, vec![1, mine], "the host's absent place and the one who plays");
    assert_eq!(run(&sim).party_len(), 1);
}

#[test]
fn a_fallen_party_asks_to_go_back_and_the_host_is_told_once() {
    let mut sim = hosted(fight(), true);
    let mut a = Friend::open(&sim, "");
    let mut b = Friend::open(&sim, "");
    play(&mut sim, &mut [&mut a, &mut b], 20);
    if let Some(mut r) = sim.as_mut() {
        r.phase = Phase::Wiped;
    }
    // One asks: the board counts down, the host not yet told.
    a.pressed = Input { again: true, ..Input::default() };
    let told = play(&mut sim, &mut [&mut a, &mut b], 10);
    assert!(!told.contains(&News::Again));
    let again = run(&sim).again.clone().expect("the asking is kept");
    assert_eq!(again.asked.iter().copied().collect::<Vec<_>>(), vec![a.knight()]);
    // The other asks too: the party goes, and the host hears it once.
    b.pressed = Input { again: true, ..Input::default() };
    let told = play(&mut sim, &mut [&mut a, &mut b], 20);
    assert_eq!(told.iter().filter(|n| **n == News::Again).count(), 1);
}
