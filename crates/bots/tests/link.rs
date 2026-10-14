//! The native link end to end, on the muster: the host's own terminal in
//! process beside clients on sockets, a link that loses its socket or hears
//! nothing coming back to its seat, and the ends that must not retry.

mod common;

use angel_bots::{BotConfig, Headless, run};
use angel_muster::MusterClient;
use angel_net::proto::Reject;
use angel_serve::client::{LinkOptions, LinkStatus, NativeLink};
use common::{Clock, KEY, Relay, frames_for, frames_until, share, start};
use std::thread;
use std::time::{Duration, Instant};

const WAIT: Duration = Duration::from_secs(5);

fn playing<G: angel_net::client::GameClient>(link: &NativeLink<G>) -> bool {
    *link.status() == LinkStatus::Playing && link.runtime().seat().is_some()
}

#[test]
fn the_host_in_process_and_clients_on_sockets_see_each_other() {
    let server = start();
    let clock = Clock::new();
    let mut host = NativeLink::<MusterClient>::local(server.connect_local().unwrap(), "Host", LinkOptions::default());
    assert!(frames_until(&mut host, &clock, WAIT, playing), "{:?}", host.status());
    let host_seat = host.runtime().seat().unwrap();
    // Five friends join over sockets and stand where they spawned, near the host.
    let until = Instant::now() + Duration::from_millis(2500);
    let bots: Vec<_> = (0..5u32)
        .map(|i| {
            let cfg = BotConfig {
                url: format!("ws://{}/ws", server.addr()),
                join: KEY.into(),
                name: format!("Friend {i}"),
                seed: u64::from(i) + 1,
                wander: false,
                slow: false,
                fps: 30,
                warmup: Duration::from_millis(500),
                stray: angel_bots::STRAY,
            };
            thread::spawn(move || run::<Headless>(&cfg, until))
        })
        .collect();
    // The host draws its frames meanwhile, as a terminal does.
    frames_for(&mut host, &clock, Duration::from_millis(2000));
    let mirror = host.runtime().game().unwrap().mirror();
    let seen: Vec<u32> = (0..6).filter(|s| *s != host_seat && mirror.get(*s).is_some()).collect();
    let frame = host.frame(clock.ms());
    assert_eq!((frame.w, frame.h), (320, 180));
    for bot in bots {
        let (report, rt) = bot.join().unwrap();
        assert!(report.error.is_none() && report.rejected.is_none(), "{report:?}");
        let seat = report.seat.unwrap();
        assert!(seen.contains(&seat), "the host sees friend at seat {seat}: {seen:?}");
        assert!(rt.game().unwrap().0.mirror().get(host_seat).is_some(), "the friend at seat {seat} sees the host");
    }
    assert!(host.bytes().0 > 0 && host.runtime().rtt_ms().is_some());
    host.bye();
    let deadline = Instant::now() + WAIT;
    while !server.players().is_empty() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(20));
    }
    assert!(server.players().is_empty(), "{:?}", server.players());
    server.stop();
}

#[test]
fn a_local_link_closed_takes_its_seat_back_with_its_token() {
    let server = start();
    let clock = Clock::new();
    let mut host = NativeLink::<Headless>::local(server.connect_local().unwrap(), "Host", LinkOptions::default());
    assert!(frames_until(&mut host, &clock, WAIT, playing));
    let (seat, token) = (host.runtime().seat(), host.seat_token().to_string());
    host.close();
    assert_eq!(*host.status(), LinkStatus::Closed);
    let deadline = Instant::now() + WAIT;
    while server.players().iter().any(|p| p.connected) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(!server.players()[0].connected, "the seat is held");
    let options = LinkOptions { resume: token.clone(), ..LinkOptions::default() };
    let mut again = NativeLink::<Headless>::local(server.connect_local().unwrap(), "Host", options);
    assert!(frames_until(&mut again, &clock, WAIT, playing));
    assert_eq!((again.runtime().seat(), again.seat_token()), (seat, token.as_str()));
    assert_eq!(server.players().len(), 1);
    server.stop();
}

#[test]
fn a_cut_socket_is_dialed_again_and_takes_the_same_seat() {
    let server = start();
    let before = server.totals();
    let relay = Relay::start(server.addr());
    let clock = Clock::new();
    let mut link = NativeLink::<Headless>::connect(&share(relay.addr), "Gwen", LinkOptions::default()).unwrap();
    assert!(frames_until(&mut link, &clock, WAIT, playing), "{:?}", link.status());
    let seat = link.runtime().seat();
    relay.cut();
    assert!(frames_until(&mut link, &clock, WAIT, |l| *l.status() == LinkStatus::Reconnecting));
    let lost_at = clock.ms();
    assert!(frames_until(&mut link, &clock, WAIT, playing));
    // The first retry waits half a second, as the page's does.
    assert!(clock.ms() - lost_at >= 450.0, "back after {} ms", clock.ms() - lost_at);
    assert_eq!((link.runtime().seat(), link.reconnects()), (seat, 1));
    let counted = server.totals().since(&before);
    assert_eq!((counted.joined, counted.resumed), (1, 1));
    let players = server.players();
    assert_eq!(players.len(), 1);
    assert!(players[0].connected);
    server.stop();
}

#[test]
fn three_seconds_of_silence_is_a_dead_link() {
    let server = start();
    let relay = Relay::start(server.addr());
    let clock = Clock::new();
    let mut link = NativeLink::<Headless>::connect(&share(relay.addr), "Gwen", LinkOptions::default()).unwrap();
    assert!(frames_until(&mut link, &clock, WAIT, playing));
    let seat = link.runtime().seat();
    relay.stall();
    let stalled_at = clock.ms();
    assert!(frames_until(&mut link, &clock, WAIT, |l| *l.status() == LinkStatus::Reconnecting));
    let dead_after = clock.ms() - stalled_at;
    assert!((2900.0..4000.0).contains(&dead_after), "dead after {dead_after} ms");
    assert!(frames_until(&mut link, &clock, WAIT, playing));
    assert_eq!(link.runtime().seat(), seat);
    server.stop();
}

#[test]
fn a_seat_taken_elsewhere_stops_the_old_link_and_a_reject_stops_retrying() {
    let server = start();
    let clock = Clock::new();
    let mut first = NativeLink::<Headless>::connect(&share(server.addr()), "Gwen", LinkOptions::default()).unwrap();
    assert!(frames_until(&mut first, &clock, WAIT, playing));
    // The same seat opened from a second terminal (or the host's own pipe).
    let options = LinkOptions { resume: first.seat_token().to_string(), ..LinkOptions::default() };
    let mut second = NativeLink::<Headless>::local(server.connect_local().unwrap(), "", options);
    assert!(frames_until(&mut second, &clock, WAIT, playing));
    assert_eq!(second.runtime().seat(), first.runtime().seat());
    assert!(frames_until(&mut first, &clock, WAIT, |l| *l.status() == LinkStatus::Replaced));
    frames_for(&mut first, &clock, Duration::from_millis(800));
    assert_eq!((first.status(), first.reconnects()), (&LinkStatus::Replaced, 0));

    // A wrong key, with a stale token: the token is dropped, and the key
    // alone is turned away for good.
    let wrong = format!("http://{}/#NOTTHISSESSIONSKEY", server.addr());
    let options = LinkOptions { resume: "STALETOKEN".into(), ..LinkOptions::default() };
    let mut turned = NativeLink::<Headless>::connect(&wrong, "Gwen", options).unwrap();
    assert!(frames_until(&mut turned, &clock, WAIT, |l| matches!(l.status(), LinkStatus::Rejected(..))));
    assert!(matches!(turned.status(), LinkStatus::Rejected(Reject::Key, _)));
    assert_eq!(turned.seat_token(), "");

    // Kicked: told why, and not back.
    assert!(server.kick(second.runtime().seat().unwrap()));
    assert!(frames_until(&mut second, &clock, WAIT, |l| matches!(l.status(), LinkStatus::Rejected(Reject::Kicked, _))));
    frames_for(&mut second, &clock, Duration::from_millis(800));
    assert_eq!(second.reconnects(), 0);
    server.stop();
}

#[test]
fn a_link_that_cannot_reach_its_host_keeps_trying_on_the_backoff() {
    // Nothing listens here (a port the OS just handed out and took back).
    let addr = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
    let clock = Clock::new();
    let mut link = NativeLink::<Headless>::connect(&share(addr), "Gwen", LinkOptions::default()).unwrap();
    frames_for(&mut link, &clock, Duration::from_millis(2000));
    // Refused at once, then after 0.5 and 1 s: three lost dials in two seconds.
    assert_eq!((link.status(), link.reconnects()), (&LinkStatus::Reconnecting, 3));
    assert!(link.retry_at_ms().is_some());
}
