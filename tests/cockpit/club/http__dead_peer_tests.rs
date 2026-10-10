#[cfg(target_os = "linux")]
use super::*;
#[cfg(target_os = "linux")]
use std::io::Read as _;
#[cfg(target_os = "linux")]
use std::net::TcpListener;

/// The client's row in `/proc/net/tcp` for the connection from local port
/// `local` to remote port `remote`: its timer kind and the centiseconds left.
fn timer_of(local: u16, remote: u16) -> Option<(String, u64)> {
    let table = std::fs::read_to_string("/proc/net/tcp").ok()?;
    let local = format!(":{local:04X}");
    let remote = format!(":{remote:04X}");
    table.lines().skip(1).find_map(|row| {
        let cols: Vec<&str> = row.split_whitespace().collect();
        if !(cols.get(1)?.ends_with(&local) && cols.get(2)?.ends_with(&remote)) {
            return None;
        }
        let (kind, left) = cols.get(5)?.split_once(':')?;
        Some((kind.to_string(), u64::from_str_radix(left, 16).ok()?))
    })
}

/// Every provider socket asks the kernel to probe its peer: a host that
/// vanishes mid-stream (powered off, dropped off the tailnet) fails the read
/// about 90 s after its last byte instead of holding the turn forever. The
/// server here reads the request and says nothing, like a model thinking; the
/// probe timer is armed for 30 s of silence, and a live peer answers it.
#[cfg(target_os = "linux")]
#[test]
fn every_provider_socket_probes_whether_its_peer_still_exists() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (peer_tx, peer_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let (mut socket, peer) = listener.accept().unwrap();
        let mut request = [0u8; 4096];
        let _ = socket.read(&mut request);
        peer_tx.send(peer).unwrap();
        std::thread::sleep(Duration::from_secs(5));
    });
    let club = HttpClub::new("probe", format!("http://{addr}/v1"), "m", None);
    let agent = club.agent.clone();
    std::thread::spawn(move || {
        let _ = agent.get(&format!("http://{addr}/v1/models")).call();
    });
    let peer = peer_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    let timer = loop {
        let timer = timer_of(peer.port(), addr.port());
        if timer.as_ref().is_some_and(|(kind, _)| kind == "02") || Instant::now() > deadline {
            break timer;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let (kind, left) = timer.expect("the client socket's row in /proc/net/tcp");
    assert_eq!(kind, "02", "keepalive timer armed on the client socket");
    assert!(
        (2_500..=3_000).contains(&left),
        "probe after ~30 s of silence, got {left} cs"
    );
}
