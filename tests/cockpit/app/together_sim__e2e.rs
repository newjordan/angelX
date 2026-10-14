//! The whole party on the engine, headless: the host's server on its own
//! thread with the host's terminal as one of its clients, a friend's angelX
//! joined over a real socket by the link (a `NativeLink`), and a browser
//! seat spoken to as a browser speaks, byte for byte, on its own socket.
use crate::app::App;
use crate::drive::together_shooter::command::DelveCmd;
use crate::drive::together_shooter::{Input, Run};
use angel_delve::wire;
use angel_net::codec::Bytes;
use angel_net::doc::delta::apply;
use angel_net::doc::value::Value;
use angel_net::doc::wire::Reader;
use angel_net::proto::{PROTOCOL, ToClient, ToServer};
use angel_serve::client::WsClient;
use angel_serve::ws::Message;
use ratatui::crossterm::event::{self, KeyCode, KeyModifiers};
use std::time::{Duration, Instant};

/// A browser seat as the page speaks: the hello with the link's key, a
/// command each pump with what it applied (so its writer never holds back),
/// and the run read back from each snapshot's tree.
struct Page {
    ws: WsClient,
    seat: Option<u32>,
    reader: Reader,
    tree: Option<Value>,
    newest: u32,
    seq: u32,
    held: Input,
    bytes: usize,
}

impl Page {
    fn open(link: &str) -> Page {
        let (url, key) = angel_serve::client::share_link(link).unwrap();
        let mut ws = WsClient::connect(&url, Duration::from_secs(5)).unwrap();
        ws.send(
            &ToServer::Hello {
                protocol: PROTOCOL,
                game: wire::GAME,
                join: &key,
                resume: "",
                name: "Page",
                caps: 0,
            }
            .to_vec(),
        )
        .unwrap();
        let mut page = Page {
            ws,
            seat: None,
            reader: Reader::default(),
            tree: None,
            newest: 0,
            seq: 0,
            held: Input::default(),
            bytes: 0,
        };
        let until = Instant::now() + Duration::from_secs(5);
        while page.seat.is_none() && Instant::now() < until {
            page.pump(Duration::from_millis(50));
        }
        assert!(page.seat.is_some(), "the page is seated");
        page
    }

    /// One command with the keys held now, then everything that arrived
    /// within `wait`.
    fn pump(&mut self, wait: Duration) {
        if self.seat.is_some() {
            self.command();
        }
        let until = Instant::now() + wait;
        loop {
            let left = until.saturating_duration_since(Instant::now());
            let Ok(Some(message)) = self.ws.recv(left.max(Duration::from_millis(1))) else {
                if Instant::now() >= until {
                    return;
                }
                continue;
            };
            let Message::Binary(bytes) = message else {
                continue;
            };
            self.bytes += bytes.len();
            match ToClient::decode(&bytes).unwrap() {
                ToClient::Welcome(w) => self.seat = w.seat,
                ToClient::Snapshot {
                    tick, whole, body, ..
                } => {
                    if whole {
                        self.reader = Reader::default();
                        self.tree = Some(self.reader.value(&mut Bytes::new(body)).unwrap());
                    } else if !body.is_empty() {
                        apply(
                            self.tree.as_mut().unwrap(),
                            self.reader.patch(&mut Bytes::new(body)).unwrap(),
                        )
                        .unwrap();
                    }
                    self.newest = tick;
                }
                _ => {}
            }
            if Instant::now() >= until {
                return;
            }
        }
    }

    /// Hold these keys from the next command on.
    fn hold(&mut self, held: Input) {
        self.held = held;
    }

    /// One tick's command, saying which tick the page has applied.
    fn command(&mut self) {
        self.seq += 1;
        let mut body = Vec::new();
        DelveCmd {
            held: self.held,
            pressed: Input::default(),
        }
        .encode(&mut body);
        self.ws
            .send(
                &ToServer::Cmds {
                    applied: self.newest,
                    cmds: vec![(self.seq, &body[..])],
                }
                .to_vec(),
            )
            .unwrap();
    }

    fn run(&self) -> Option<Run> {
        self.tree.as_ref().and_then(|t| wire::read(t).ok())
    }
}

fn paint(app: &mut App) {
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 40)).unwrap();
    terminal
        .draw(|frame| crate::ui::draw::ui(frame, app))
        .unwrap();
}

/// One display frame of both terminals and a pump of the page.
fn frame(host: &mut App, friend: &mut App, page: &mut Page) {
    host.advance_shooter_at(Instant::now());
    paint(host);
    friend.advance_joined();
    paint(friend);
    page.pump(Duration::from_millis(16));
}

fn x(run: &Run, knight: u32) -> f32 {
    run.players[&knight].x
}

#[test]
fn a_threaded_host_its_own_seat_a_joined_friend_and_a_browser_seat_play_one_delve() {
    let _guard = crate::tests::env_lock();
    let _public = crate::tests::TestEnvGuard::unset("ANGEL_DUNGEON_PUBLIC_URL");
    let _listen = crate::tests::TestEnvGuard::set("ANGEL_DUNGEON_LISTEN", "127.0.0.1:0");
    let dir = std::env::temp_dir().join(format!("angel-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let save = dir.join(".angel/dungeon/session.json");
    let mut host = App::preview(crate::ui::viewer::Viewer::static_preview());
    host.terminal_focused = true;
    host.dungeon.shooter.prefer_thread(true);
    host.dungeon.save_path = Some(save.clone());
    let message = host.dungeon_command(Some("host --2"));
    assert!(host.dungeon.shooter.threaded(), "{message}");
    let link = message
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("http://"))
        .expect("the link")
        .to_string();
    assert_eq!(
        message
            .lines()
            .filter(|l| l.trim().starts_with("http"))
            .count(),
        1,
        "one link: {message}"
    );
    host.on_key(event::KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    // A friend's angelX pastes the link; a browser opens it.
    let mut friend = App::preview(crate::ui::viewer::Viewer::static_preview());
    friend.terminal_focused = true;
    let joined = friend.dungeon_command(Some(&format!("join {link}")));
    assert!(joined.starts_with("Joined the delve"), "{joined}");
    let mut page = Page::open(&link);

    // Everyone seated, and each client holds the whole party.
    let deadline = Instant::now() + Duration::from_secs(20);
    let gathered = |host: &App, friend: &App, page: &Page| {
        host.dungeon
            .shooter
            .as_ref()
            .is_some_and(|r| r.players.len() == 3)
            && friend
                .dungeon
                .joined
                .as_ref()
                .and_then(|j| j.with_latest(|r| r.players.len()))
                == Some(3)
            && page.run().is_some_and(|r| r.players.len() == 3)
    };
    while !gathered(&host, &friend, &page) {
        assert!(
            Instant::now() < deadline,
            "the party never gathered: host {:?}, friend {:?} ({:?}), page {:?} (seat {:?})",
            host.dungeon.shooter.as_ref().map(|r| r.players.len()),
            friend
                .dungeon
                .joined
                .as_ref()
                .and_then(|j| j.with_latest(|r| r.players.len())),
            friend.dungeon.joined.as_ref().map(|j| j.seat.status()),
            page.run().map(|r| r.players.len()),
            page.seat,
        );
        frame(&mut host, &mut friend, &mut page);
    }
    let page_knight = wire::knight_of(page.seat.unwrap());
    let friend_knight = friend.joined_actor();
    let mut knights = vec![1, friend_knight, page_knight];
    knights.sort();
    assert_eq!(knights, vec![1, 2, 3], "three seats, three knights");

    // The checkpoint is the service's, from a published tick, every three
    // seconds: taken away, it comes back while this thread only plays.
    let until = Instant::now() + Duration::from_secs(8);
    while !save.exists() {
        assert!(Instant::now() < until, "no checkpoint was written");
        frame(&mut host, &mut friend, &mut page);
    }
    std::fs::remove_file(&save).unwrap();
    let until = Instant::now() + Duration::from_secs(8);
    while !save.exists() {
        assert!(
            Instant::now() < until,
            "the checkpoint was not written again"
        );
        frame(&mut host, &mut friend, &mut page);
    }
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&save).unwrap()).unwrap();
    assert_eq!(
        saved["run"]["raid_id"],
        host.dungeon.shooter.as_ref().unwrap().raid_id
    );
    assert!(
        saved["invitation"][1]
            .as_str()
            .is_some_and(|k| link.ends_with(k)),
        "the link is saved: {saved}"
    );

    // Everyone holds a key that walks right: the host's own terminal, the
    // friend's, and the page.
    host.dungeon.shooter.latch();
    let start = host.dungeon.shooter.as_ref().unwrap().clone();
    for knight in 1..=3 {
        assert!(start.players[&knight].hp > 0);
    }
    host.dungeon.key_releases = true;
    friend.dungeon.key_releases = true;
    host.on_key(event::KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
    friend.on_key(event::KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
    page.hold(Input {
        move_x: 1,
        ..Default::default()
    });
    let began = Instant::now();
    while began.elapsed() < Duration::from_millis(1500) {
        frame(&mut host, &mut friend, &mut page);
    }
    page.hold(Input::default());
    host.dungeon.shooter.latch();
    let now = host.dungeon.shooter.as_ref().unwrap().clone();
    let elapsed = began.elapsed().as_secs_f32();
    let ticks = now.tick - start.tick;
    assert!(
        ticks as f32 >= elapsed * 30.0 * 0.5 && ticks as f32 <= elapsed * 30.0 + 3.0,
        "the server kept its own 30 Hz: {ticks} ticks in {elapsed:.2} s"
    );
    for knight in 1..=3 {
        assert!(
            x(&now, knight) > x(&start, knight) + 1.0,
            "knight {knight} walked: {} -> {}",
            x(&start, knight),
            x(&now, knight)
        );
    }
    // Each client sees the others walk, not only itself.
    for _ in 0..20 {
        frame(&mut host, &mut friend, &mut page);
    }
    let seen = friend
        .dungeon
        .joined
        .as_ref()
        .unwrap()
        .with_latest(Run::clone)
        .unwrap();
    let paged = page.run().unwrap();
    for knight in 1..=3 {
        assert!(
            x(&seen, knight) > x(&start, knight) + 1.0,
            "the friend sees knight {knight} walk"
        );
        assert!(
            x(&paged, knight) > x(&start, knight) + 1.0,
            "the page sees knight {knight} walk"
        );
    }
    // The host's own screen is its seat's client's frame.
    let picture = host
        .dungeon
        .shooter
        .seat(1)
        .unwrap()
        .picture()
        .expect("the host's seat drew");
    assert_eq!(
        (picture.w, picture.h),
        crate::stage::world_viz::overworld::arena::art_v2::size()
    );

    // An edit on the host reaches every client.
    host.on_key(event::KeyEvent::new_with_kind(
        KeyCode::Char('d'),
        KeyModifiers::NONE,
        event::KeyEventKind::Release,
    ));
    friend.on_key(event::KeyEvent::new_with_kind(
        KeyCode::Char('d'),
        KeyModifiers::NONE,
        event::KeyEventKind::Release,
    ));
    if let Some(mut run) = host.dungeon.shooter.as_mut() {
        run.score = 4242;
    }
    let until = Instant::now() + Duration::from_secs(5);
    while !(friend
        .dungeon
        .joined
        .as_ref()
        .unwrap()
        .with_latest(|r| r.score)
        == Some(4242)
        && page.run().is_some_and(|r| r.score == 4242))
    {
        assert!(
            Instant::now() < until,
            "the host's edit never reached its clients"
        );
        frame(&mut host, &mut friend, &mut page);
    }

    // What the run banks settles into the host's realm.
    if let Some(mut run) = host.dungeon.shooter.as_mut() {
        let mut spoils = crate::drive::together_realm::Spoils::default();
        spoils.add(crate::drive::together_realm::Spoil::Bone, 4);
        run.bank.push(angel_delve::realm::Haul {
            hero: friend_knight,
            spoils,
            why: "a test".into(),
        });
    }
    let until = Instant::now() + Duration::from_secs(5);
    while host.realm().treasury.label() != "4 bone" {
        assert!(
            Instant::now() < until,
            "the bank never settled: {}",
            host.realm().treasury.label()
        );
        frame(&mut host, &mut friend, &mut page);
    }
    assert!(
        host.dungeon.shooter.as_ref().unwrap().bank.is_empty(),
        "banked once"
    );

    assert!(friend.dungeon_command(Some("leave")).contains("left"));
    drop(host);
    let _ = std::fs::remove_dir_all(&dir);
}
