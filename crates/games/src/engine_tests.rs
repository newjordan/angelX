use super::*;
use angel_live::realm::{Facts, Move, Oracle};
use angel_muster::{MusterClient, MusterServer};
use crossterm::event::KeyModifiers;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

static PAGES_ENV: Mutex<()> = Mutex::new(());

struct Mute;

impl Oracle for Mute {
    fn respond(&self, _prompt: &str, _at_least: Duration) -> Result<String, String> {
        Err("no model".into())
    }

    fn label(&self) -> &str {
        "mute"
    }
}

#[derive(Default)]
struct Cx {
    said: Vec<String>,
}

impl HostCx for Cx {
    fn facts(&self) -> Facts {
        Facts::default()
    }

    fn workspace(&self) -> PathBuf {
        std::env::temp_dir()
    }

    fn oracle(&self) -> Arc<dyn Oracle> {
        Arc::new(Mute)
    }

    fn say(&mut self, text: String) {
        self.said.push(text);
    }

    fn redraw(&mut self) {}

    fn take_composer(&mut self) -> (String, usize) {
        (String::new(), 0)
    }

    fn set_composer(&mut self, _text: String, _cursor: usize) {}

    fn make(&mut self, _ask: Move) {}
}

fn muster() -> Engine<MusterServer, MusterClient> {
    Engine::new(Spec {
        id: "muster",
        title: "The Muster",
        help: "arrows walk · space waves · e plants the banner",
        make: || MusterServer::new(8),
        keys: crate::keys::standard,
    })
}

fn settle(cart: &mut Engine<MusterServer, MusterClient>, cx: &mut Cx) -> bool {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        cart.tick(cx, true);
        if cart.picture(Instant::now()).is_some() && cart.status().contains("playing") {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

#[test]
fn an_engine_game_loads_seats_the_host_and_draws() {
    let mut cx = Cx::default();
    let mut cart = muster();
    assert!(!cart.running());
    cart.start(&mut cx).expect("the muster opens on loopback");
    assert!(cart.running());
    assert!(settle(&mut cart, &mut cx), "{}", cart.status());
    let picture = cart.picture(Instant::now()).expect("a frame");
    assert!(picture.w >= 64 && picture.h >= 36);
    assert!(
        picture.px.iter().any(|&p| p & 0x00ff_ffff != 0),
        "the frame is not all black"
    );
    assert!(cart.status().contains("seat 1"), "{}", cart.status());
    assert!(cart.status().contains("solo"));
    cart.stop(&mut cx);
    assert!(!cart.running());
}

#[test]
fn keys_press_and_lapse_without_release_reports() {
    let mut cx = Cx::default();
    let mut cart = muster();
    cart.start(&mut cx).unwrap();
    assert!(settle(&mut cart, &mut cx));
    let right = KeyEvent::new(KeyCode::Right, KeyModifiers::NONE);
    assert!(cart.key(&mut cx, right));
    assert_eq!(cart.live.as_ref().unwrap().held.len(), 1);
    let unmapped = KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE);
    assert!(cart.key(&mut cx, unmapped));
    assert_eq!(cart.live.as_ref().unwrap().held.len(), 1);
    std::thread::sleep(PRESS_LEASE + Duration::from_millis(20));
    cart.tick(&mut cx, true);
    assert!(cart.live.as_ref().unwrap().held.is_empty());
    cart.stop(&mut cx);
}

#[test]
fn a_release_report_lets_go_at_once_and_lengthens_later_leases() {
    let mut cx = Cx::default();
    let mut cart = muster();
    cart.start(&mut cx).unwrap();
    assert!(settle(&mut cart, &mut cx));
    let mut up = KeyEvent::new(KeyCode::Char('W'), KeyModifiers::SHIFT);
    cart.key(&mut cx, up);
    up.kind = KeyEventKind::Release;
    cart.key(&mut cx, up);
    let live = cart.live.as_ref().unwrap();
    assert!(live.releases && live.held.is_empty());
    let left = KeyEvent::new(KeyCode::Left, KeyModifiers::NONE);
    cart.key(&mut cx, left);
    std::thread::sleep(PRESS_LEASE + Duration::from_millis(20));
    cart.tick(&mut cx, true);
    assert_eq!(
        cart.live.as_ref().unwrap().held.len(),
        1,
        "a terminal that reports releases holds until the release"
    );
    cart.clear_controls(&mut cx);
    assert!(cart.live.as_ref().unwrap().held.is_empty());
    cart.stop(&mut cx);
}

#[test]
fn hidden_carts_let_go_of_held_keys() {
    let mut cx = Cx::default();
    let mut cart = muster();
    cart.start(&mut cx).unwrap();
    assert!(settle(&mut cart, &mut cx));
    cart.key(&mut cx, KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    cart.tick(&mut cx, false);
    assert!(cart.live.as_ref().unwrap().held.is_empty());
    cart.stop(&mut cx);
}

#[test]
fn commands_report_and_unknown_words_list_the_verbs() {
    let mut cx = Cx::default();
    let mut cart = muster();
    assert!(cart.command(&mut cx, "link").contains("not running"));
    assert!(
        cart.command(&mut cx, "dance")
            .contains("start · host · join")
    );
    assert!(cart.command(&mut cx, "join").contains("join <link>"));
    assert!(cart.command(&mut cx, "stop").contains("not running"));
    cart.command(&mut cx, "start");
    assert!(cart.running());
    assert!(cart.command(&mut cx, "stop").contains("closed"));
}

#[test]
fn pages_follow_the_override_then_home() {
    let _env = PAGES_ENV.lock().unwrap_or_else(|e| e.into_inner());
    let dir = std::env::temp_dir().join("angel-games-pages");
    unsafe { std::env::set_var("ANGEL_GAME_PAGES", &dir) };
    assert_eq!(super::pages(), Some(dir));
    assert!(super::page("muster-without-a-page").is_none());
    unsafe { std::env::remove_var("ANGEL_GAME_PAGES") };
    if let Some(home) = std::env::var_os("HOME") {
        assert_eq!(
            super::pages(),
            Some(PathBuf::from(home).join(".angelX").join("pages"))
        );
    }
}

#[test]
fn a_hosted_cartridge_serves_its_page_and_says_its_link() {
    let _env = PAGES_ENV.lock().unwrap_or_else(|e| e.into_inner());
    use std::io::{Read, Write};
    let dir = std::env::temp_dir().join(format!("angel-games-host-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("muster")).unwrap();
    std::fs::write(
        dir.join("muster").join("index.html"),
        "<html><body>the muster page</body></html>",
    )
    .unwrap();
    let mut cx = Cx::default();
    let mut cart = muster();
    unsafe { std::env::set_var("ANGEL_GAME_PAGES", &dir) };
    let reply = cart.command(&mut cx, "host");
    unsafe { std::env::remove_var("ANGEL_GAME_PAGES") };
    assert!(reply.contains("hosting"), "{reply}");
    assert!(!reply.contains("no browser page"), "{reply}");
    let live = cart.live.as_ref().unwrap();
    let link = live
        .links
        .first()
        .expect("a hosted game prints its link")
        .clone();
    let addr = live.server.as_ref().unwrap().addr();
    assert!(crate::door::private_address(addr.ip()), "{addr}");
    assert!(link.contains(&addr.port().to_string()), "{link} {addr}");
    let mut stream = std::net::TcpStream::connect(addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    write!(
        stream,
        "GET / HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut page = String::new();
    let _ = stream.read_to_string(&mut page);
    assert!(page.contains("the muster page"), "{page}");
    assert!(settle(&mut cart, &mut cx), "{}", cart.status());
    cart.stop(&mut cx);
    let _ = std::fs::remove_dir_all(&dir);
}
