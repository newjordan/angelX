//! `GLASS_STARTUP_TRACE=1`: where the time from launch to the first frame
//! and the program's start goes. Each mark prints, on stderr, the
//! milliseconds since the first mark (the process's start, as `main` makes
//! it), the wall clock in nanoseconds (to line up with a launcher's own
//! clock), the thread and what was reached. Unset, a mark is one relaxed
//! load.

use std::io::Write;
use std::sync::OnceLock;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// The first mark's instant, when tracing is on.
fn origin() -> Option<Instant> {
    static ORIGIN: OnceLock<Option<Instant>> = OnceLock::new();
    *ORIGIN.get_or_init(|| {
        std::env::var_os("GLASS_STARTUP_TRACE")
            .is_some_and(|v| !v.is_empty() && v != "0")
            .then(Instant::now)
    })
}

/// Note that startup reached `what`.
pub fn mark(what: &str) {
    let Some(origin) = origin() else {
        return;
    };
    let wall = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let thread = std::thread::current();
    let _ = writeln!(
        std::io::stderr().lock(),
        "glass-startup {:8.2} ms {wall} [{}] {what}",
        origin.elapsed().as_secs_f64() * 1000.0,
        thread.name().unwrap_or("-"),
    );
}
