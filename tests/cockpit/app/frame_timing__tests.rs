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
