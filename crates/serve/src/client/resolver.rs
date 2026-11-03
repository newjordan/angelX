//! Host-owned, bounded name resolution. System DNS can block beyond a socket
//! deadline; callers stop waiting while at most four resolver workers finish.
use std::io;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::Instant;
type Lookup = fn(&str, u16) -> io::Result<Vec<SocketAddr>>;
struct Request {
    host: String,
    port: u16,
    reply: mpsc::SyncSender<io::Result<Vec<SocketAddr>>>,
}
struct Resolver {
    tx: mpsc::SyncSender<Request>,
}
impl Resolver {
    fn new(lookup: Lookup) -> io::Result<Self> {
        let (tx, rx) = mpsc::sync_channel::<Request>(16);
        let rx = Arc::new(Mutex::new(rx));
        for index in 0..4 {
            let rx = Arc::clone(&rx);
            std::thread::Builder::new()
                .name(format!("angel-resolve-{index}"))
                .spawn(move || {
                    loop {
                        let request = rx.lock().unwrap_or_else(|p| p.into_inner()).recv();
                        let Ok(request) = request else {
                            break;
                        };
                        let result = lookup(&request.host, request.port);
                        let _ = request.reply.send(result);
                    }
                })?;
        }
        Ok(Self { tx })
    }
    fn resolve(&self, host: &str, port: u16, deadline: Instant) -> io::Result<Vec<SocketAddr>> {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "name lookup deadline expired",
            ));
        }
        let (tx, rx) = mpsc::sync_channel(1);
        self.tx
            .try_send(Request {
                host: host.into(),
                port,
                reply: tx,
            })
            .map_err(|_| io::Error::new(io::ErrorKind::WouldBlock, "name resolver is occupied"))?;
        rx.recv_timeout(left)
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "name lookup deadline expired"))?
    }
}
pub(super) fn resolve(host: &str, port: u16, deadline: Instant) -> io::Result<Vec<SocketAddr>> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(vec![SocketAddr::new(ip, port)]);
    }
    static RESOLVER: OnceLock<Result<Resolver, String>> = OnceLock::new();
    let resolver = RESOLVER.get_or_init(|| {
        Resolver::new(|host, port| (host, port).to_socket_addrs().map(Iterator::collect))
            .map_err(|e| e.to_string())
    });
    resolver
        .as_ref()
        .map_err(|e| io::Error::other(e.clone()))?
        .resolve(host, port, deadline)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn a_slow_system_lookup_does_not_hold_the_callers_deadline_or_other_lookups() {
        let resolver = Resolver::new(|host, port| {
            if host == "slow" {
                std::thread::sleep(Duration::from_millis(80));
            }
            Ok(vec![SocketAddr::from(([127, 0, 0, 1], port))])
        })
        .unwrap();
        assert_eq!(
            resolver
                .resolve("slow", 1, Instant::now() + Duration::from_millis(5))
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
        assert_eq!(
            resolver
                .resolve("fast", 2, Instant::now() + Duration::from_millis(500))
                .unwrap()[0]
                .port(),
            2
        );
        // Abandoned replies are discarded; the same pool remains usable.
        std::thread::sleep(Duration::from_millis(90));
        assert_eq!(
            resolver
                .resolve("fast", 3, Instant::now() + Duration::from_millis(500))
                .unwrap()[0]
                .port(),
            3
        );
    }
    #[test]
    fn numeric_addresses_bypass_dns_and_expired_named_requests_do_not_queue() {
        assert_eq!(
            resolve("::1", 8, Instant::now()).unwrap()[0],
            SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 1], 8))
        );
        let resolver = Resolver::new(|_, _| panic!("expired lookup was queued")).unwrap();
        assert_eq!(
            resolver
                .resolve("expired", 1, Instant::now())
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
    }
}
