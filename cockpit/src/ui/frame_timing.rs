//! Opt-in terminal-loop phase telemetry; never records contents or key values.
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Default)]
pub(crate) struct Phases {
    pub(crate) poll_us: u128,
    pub(crate) input_us: u128,
    pub(crate) advance_us: u128,
    pub(crate) settle_us: u128,
}

pub(crate) struct FrameTiming {
    file: File,
    origin: Instant,
    previous: Option<Instant>,
    previous_cpu_us: Option<u128>,
    diagnostics: Diagnostics,
}

#[derive(Clone, Copy, Default)]
struct Diagnostics {
    // The write is a SUBSET of record cost, not an additional gap component.
    prior_record_us: Option<u128>,
    prior_record_write_us: Option<u128>,
    thread_cpu_gap_us: Option<u128>,
    record_seq: u64,
    prior_record_write_failures: u64,
    post_draw_us: u128,
    pre_poll_us: u128,
}

impl FrameTiming {
    pub(crate) fn from_env() -> Option<Self> {
        let path = std::env::var_os("ANGEL_FRAME_TIMING_LOG")?;
        if path.is_empty() {
            return None;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .ok()?;
        let origin = Instant::now();
        let epoch_us = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_micros();
        writeln!(
            file,
            "# epoch_us={epoch_us} elapsed_us poll_us input_us advance_us settle_us draw_us gap_us prior_record_us prior_record_write_us post_draw_us pre_poll_us thread_cpu_gap_us record_seq prior_record_write_failures"
        )
        .ok()?;
        Some(Self {
            file,
            origin,
            previous: None,
            previous_cpu_us: None,
            diagnostics: Diagnostics::default(),
        })
    }

    #[cfg(test)]
    pub(crate) fn completed(&mut self, start: Instant, end: Instant, phases: Phases) {
        self.completed_with_boundaries(start, end, phases, 0, 0);
    }

    pub(crate) fn completed_with_boundaries(
        &mut self,
        start: Instant,
        end: Instant,
        phases: Phases,
        post_draw_us: u128,
        pre_poll_us: u128,
    ) -> Instant {
        self.diagnostics.post_draw_us = post_draw_us;
        self.diagnostics.pre_poll_us = pre_poll_us;
        // Sample at the same logical completion boundary as wall time. This
        // includes main-thread CPU spent on the preceding record/flush, not
        // CPU used by decoder/encoder/background threads. Unsupported = '-'.
        let cpu_us = thread_cpu_us();
        self.diagnostics.thread_cpu_gap_us = cpu_us
            .zip(self.previous_cpu_us)
            .and_then(|(current, previous)| current.checked_sub(previous));
        self.diagnostics.record_seq = self.diagnostics.record_seq.saturating_add(1);
        let line = row(
            self.origin,
            self.previous,
            start,
            end,
            phases,
            self.diagnostics,
        );
        let write_started = Instant::now();
        // Fail open: telemetry must never cause the cockpit to exit.
        let result = writeln!(self.file, "{line}");
        let written = Instant::now();
        self.diagnostics.prior_record_write_us =
            Some(written.duration_since(write_started).as_micros());
        self.diagnostics.prior_record_us = Some(written.duration_since(end).as_micros());
        if result.is_err() {
            self.diagnostics.prior_record_write_failures = self
                .diagnostics
                .prior_record_write_failures
                .saturating_add(1);
        }
        self.previous = Some(end);
        self.previous_cpu_us = cpu_us;
        // Post-draw accounting starts at this exact boundary, including the
        // bookkeeping/return above without creating another untimed interval.
        written
    }
}

fn thread_cpu_us() -> Option<u128> {
    #[cfg(target_os = "linux")]
    {
        let mut time = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        // CLOCK_THREAD_CPUTIME_ID observes this thread only and performs no I/O.
        if unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut time) } == 0
            && time.tv_sec >= 0
            && time.tv_nsec >= 0
        {
            return Some(time.tv_sec as u128 * 1_000_000 + time.tv_nsec as u128 / 1_000);
        }
    }
    None
}

fn row(
    origin: Instant,
    previous: Option<Instant>,
    start: Instant,
    end: Instant,
    phases: Phases,
    diagnostics: Diagnostics,
) -> String {
    let optional = |value: Option<u128>| value.map_or_else(|| "-".into(), |v| v.to_string());
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        end.duration_since(origin).as_micros(),
        phases.poll_us,
        phases.input_us,
        phases.advance_us,
        phases.settle_us,
        end.duration_since(start).as_micros(),
        previous
            .map(|p| end.duration_since(p).as_micros().to_string())
            .unwrap_or_else(|| "-".into()),
        optional(diagnostics.prior_record_us),
        optional(diagnostics.prior_record_write_us),
        diagnostics.post_draw_us,
        diagnostics.pre_poll_us,
        optional(diagnostics.thread_cpu_gap_us),
        diagnostics.record_seq,
        diagnostics.prior_record_write_failures,
    )
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/frame_timing__tests.rs"]
mod tests;
