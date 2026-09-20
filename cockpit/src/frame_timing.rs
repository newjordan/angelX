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
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn frame_timing_rows_separate_draw_cost_and_completion_gap() {
        let origin = Instant::now();
        let start = origin + Duration::from_millis(30);
        let end = start + Duration::from_millis(4);
        assert_eq!(
            row(
                origin,
                None,
                start,
                end,
                Phases {
                    poll_us: 10,
                    input_us: 20,
                    advance_us: 30,
                    settle_us: 40,
                },
                Diagnostics::default(),
            ),
            "34000\t10\t20\t30\t40\t4000\t-\t-\t-\t0\t0\t-\t0\t0"
        );
        assert_eq!(
            row(
                origin,
                Some(origin + Duration::from_millis(2)),
                start,
                end,
                Phases {
                    poll_us: 10,
                    input_us: 20,
                    advance_us: 30,
                    settle_us: 40,
                },
                Diagnostics {
                    prior_record_us: Some(700),
                    prior_record_write_us: Some(650),
                    thread_cpu_gap_us: Some(900),
                    record_seq: 2,
                    prior_record_write_failures: 1,
                    post_draw_us: 50,
                    pre_poll_us: 60,
                },
            ),
            "34000\t10\t20\t30\t40\t4000\t32000\t700\t650\t50\t60\t900\t2\t1"
        );
    }

    #[test]
    fn frame_timing_unset_is_disabled() {
        let _guard = crate::tests::env_lock();
        let old = std::env::var_os("ANGEL_FRAME_TIMING_LOG");
        unsafe { std::env::remove_var("ANGEL_FRAME_TIMING_LOG") };
        assert!(FrameTiming::from_env().is_none());
        if let Some(old) = old {
            unsafe { std::env::set_var("ANGEL_FRAME_TIMING_LOG", old) };
        }
    }

    #[test]
    fn frame_timing_completed_persists_product_timestamps() {
        let path = std::env::current_dir()
            .unwrap()
            .join(format!(".frame-timing-{}-test.txt", std::process::id()));
        let origin = Instant::now() - Duration::from_millis(20);
        let mut timing = FrameTiming {
            file: File::create(&path).unwrap(),
            origin,
            previous: None,
            previous_cpu_us: None,
            diagnostics: Diagnostics::default(),
        };
        timing.completed(
            origin,
            origin + Duration::from_millis(2),
            Phases {
                poll_us: 1,
                input_us: 2,
                advance_us: 3,
                settle_us: 4,
            },
        );
        timing.completed(
            origin + Duration::from_millis(10),
            origin + Duration::from_millis(13),
            Phases {
                poll_us: 5,
                input_us: 6,
                advance_us: 7,
                settle_us: 8,
            },
        );
        drop(timing);
        let saved = std::fs::read_to_string(&path).unwrap();
        let rows: Vec<Vec<&str>> = saved
            .lines()
            .map(|line| line.split('\t').collect())
            .collect();
        assert_eq!(rows[0][..7].join("\t"), "2000\t1\t2\t3\t4\t2000\t-");
        assert_eq!(rows[1][..7].join("\t"), "13000\t5\t6\t7\t8\t3000\t11000");
        assert_eq!(&rows[0][7..], &["-", "-", "0", "0", "-", "1", "0"]);
        let record = rows[1][7].parse::<u128>().unwrap();
        let write = rows[1][8].parse::<u128>().unwrap();
        assert!(
            record >= write,
            "write is nested within record, not additive"
        );
        assert_eq!(&rows[1][12..], &["2", "0"]);
        #[cfg(target_os = "linux")]
        assert!(rows[1][11].parse::<u128>().is_ok());
        std::fs::remove_file(path).unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn frame_timing_write_failures_are_counted_without_retry_or_recursion() {
        let origin = Instant::now();
        let mut timing = FrameTiming {
            file: OpenOptions::new().write(true).open("/dev/full").unwrap(),
            origin,
            previous: None,
            previous_cpu_us: None,
            diagnostics: Diagnostics::default(),
        };
        timing.completed(origin, Instant::now(), Phases::default());
        assert_eq!(timing.diagnostics.prior_record_write_failures, 1);
        assert_eq!(timing.diagnostics.record_seq, 1);
        timing.completed(origin, Instant::now(), Phases::default());
        assert_eq!(timing.diagnostics.prior_record_write_failures, 2);
        assert_eq!(timing.diagnostics.record_seq, 2);
        let saved = row(
            origin,
            timing.previous,
            origin,
            Instant::now(),
            Phases::default(),
            timing.diagnostics,
        );
        assert_eq!(saved.split('\t').next_back(), Some("2"));
    }
}
