//! Explicit per-request socket cancellation without changing read deadlines.
use std::io;
use std::net::{Shutdown, TcpStream};
use std::sync::{Arc, Mutex};

/// A one-request cancellation handle. Clones share cancellation state.
/// Requests using a handle never borrow from or return to the connection pool.
#[derive(Clone, Debug, Default)]
pub struct AbortHandle(Arc<Mutex<State>>);

#[derive(Debug, Default)]
struct State {
    aborted: bool,
    socket: Option<TcpStream>,
}

impl AbortHandle {
    /// Abort this request's socket, including blocked TLS/header/body operations.
    /// If TCP connection establishment is still pending, the socket is shut down
    /// as soon as it is attached. DNS/connect retain their configured bounds.
    pub fn abort(&self) {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        state.aborted = true;
        if let Some(socket) = &state.socket {
            let _ = socket.shutdown(Shutdown::Both);
        }
    }

    pub(crate) fn attach(&self, socket: &TcpStream) -> io::Result<()> {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if state.aborted {
            let _ = socket.shutdown(Shutdown::Both);
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "request cancelled",
            ));
        }
        state.socket = Some(socket.try_clone()?);
        Ok(())
    }
}
