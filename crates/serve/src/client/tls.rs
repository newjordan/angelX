//! `wss://` for the native client: rustls, with the certificate verified
//! against `Trust` and its name against the link's host (SNI sent).
//!
//! **Why Mozilla's roots compiled in** (`webpki-roots`) and not the OS's
//! (`rustls-native-certs`): the links a friend's angelX opens are Funnel's
//! (`https://<name>.ts.net`, a Let's Encrypt certificate) and a relay's on
//! a public domain, both chained to roots every browser trusts. Compiled
//! in, those roots are the same on Linux, macOS and Windows with nothing to
//! configure. The OS's store is not: Windows fills its root store lazily
//! (a root arrives when the OS's own verifier first asks for it, which
//! rustls never does), and a minimal Linux may have no CA bundle at all.
//! What the OS store adds (a company's interception root) a game link does
//! not need. A private relay's own root is `Trust::Only`.
//!
//! **Two threads, one TLS session.** A link reads on one thread and writes
//! on another. Each half holds the TCP socket and the shared session: a
//! read blocks on the socket with no lock held, then takes the lock only to
//! decrypt what came; a write takes it to encrypt and send. So a blocked
//! read never holds up a command going out.

use super::{Trust, bad, timed_out};
use rustls::pki_types::{CertificateDer, ServerName};
use rustls::{ClientConfig, ClientConnection, RootCertStore};
use std::io::{self, ErrorKind, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::Instant;

/// Bytes read off the socket, and plaintext taken out of the session, at a time.
const RAW: usize = 16 * 1024;

fn config(roots: RootCertStore) -> Arc<ClientConfig> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("ring offers TLS 1.2 and 1.3")
        .with_root_certificates(roots)
        .with_no_client_auth();
    // A WebSocket upgrade is HTTP/1.1: say so, so a door that also speaks
    // HTTP/2 answers in the one this client speaks.
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Arc::new(config)
}

fn trusted(trust: &Trust) -> io::Result<Arc<ClientConfig>> {
    static PUBLIC: OnceLock<Arc<ClientConfig>> = OnceLock::new();
    match trust {
        Trust::Public => {
            Ok(PUBLIC.get_or_init(|| config(RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() })).clone())
        }
        Trust::Only(roots) => {
            let mut store = RootCertStore::empty();
            for der in roots {
                store.add(CertificateDer::from(der.clone())).map_err(|e| bad(&format!("a root does not read: {e}")))?;
            }
            Ok(config(store))
        }
    }
}

type Session = Arc<Mutex<ClientConnection>>;

fn lock(session: &Session) -> MutexGuard<'_, ClientConnection> {
    session.lock().unwrap_or_else(|p| p.into_inner())
}

/// The TLS handshake on a connected socket, done by `deadline`.
pub(super) fn handshake(stream: TcpStream, host: &str, trust: &Trust, deadline: Instant) -> io::Result<(ReadHalf, WriteHalf)> {
    let name = ServerName::try_from(host.to_string()).map_err(|_| bad("the link's host is not a name TLS can check"))?;
    let mut conn = ClientConnection::new(trusted(trust)?, name).map_err(|e| bad(&format!("TLS: {e}")))?;
    let mut io = &stream;
    while conn.is_handshaking() {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(io::Error::new(ErrorKind::TimedOut, "the TLS handshake took too long"));
        }
        stream.set_read_timeout(Some(left))?;
        conn.complete_io(&mut io)?;
    }
    let session = Arc::new(Mutex::new(conn));
    let read = ReadHalf { stream: stream.try_clone()?, session: session.clone(), raw: vec![0; RAW], plain: vec![0; RAW], ended: false };
    Ok((read, WriteHalf { stream, session }))
}

pub(super) struct ReadHalf {
    stream: TcpStream,
    session: Session,
    raw: Vec<u8>,
    plain: Vec<u8>,
    /// The server said it is done (TLS's close_notify).
    ended: bool,
}

impl ReadHalf {
    /// As `ReadWire::fill`: at most `max` bytes off the socket within
    /// `wait`, their plaintext handed to `sink`.
    pub(super) fn fill(&mut self, max: usize, wait: std::time::Duration, sink: &mut dyn FnMut(&[u8])) -> io::Result<usize> {
        if self.ended {
            return Err(io::Error::new(ErrorKind::UnexpectedEof, "the server closed the connection"));
        }
        self.stream.set_read_timeout(Some(wait))?;
        let n = max.min(self.raw.len());
        let got = match self.stream.read(&mut self.raw[..n]) {
            Ok(0) => return Err(io::Error::new(ErrorKind::UnexpectedEof, "the server closed the socket")),
            Ok(got) => got,
            Err(e) if timed_out(&e) => return Ok(0),
            Err(e) => return Err(e),
        };
        let mut conn = lock(&self.session);
        let mut rd = &self.raw[..got];
        let mut handed = 0;
        while !rd.is_empty() && !self.ended {
            if conn.read_tls(&mut rd)? == 0 {
                break;
            }
            conn.process_new_packets().map_err(|e| bad(&format!("TLS: {e}")))?;
            loop {
                match conn.reader().read(&mut self.plain) {
                    Ok(0) => {
                        self.ended = true;
                        break;
                    }
                    Ok(m) => {
                        sink(&self.plain[..m]);
                        handed += m;
                    }
                    Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                    Err(e) => return Err(e),
                }
            }
        }
        // What the session must answer (a key update) goes now.
        while conn.wants_write() {
            conn.write_tls(&mut &self.stream)?;
        }
        Ok(handed)
    }

    pub(super) fn socket(&self) -> &TcpStream {
        &self.stream
    }
}

pub(super) struct WriteHalf {
    stream: TcpStream,
    session: Session,
}

impl WriteHalf {
    pub(super) fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        let mut conn = lock(&self.session);
        conn.writer().write_all(bytes)?;
        while conn.wants_write() {
            conn.write_tls(&mut &self.stream)?;
        }
        Ok(())
    }

    /// Tell the server this side is done, then stop writing.
    pub(super) fn shutdown(&mut self) {
        let mut conn = lock(&self.session);
        conn.send_close_notify();
        while conn.wants_write() {
            if conn.write_tls(&mut &self.stream).is_err() {
                break;
            }
        }
        let _ = self.stream.shutdown(Shutdown::Write);
    }
}
