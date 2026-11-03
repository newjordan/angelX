//! Run independent native compiles within Cargo's job budget.
//!
//! The caller owns one implicit jobserver slot. Acquire only immediately
//! available extra slots: waiting for one while Cargo waits for this build
//! script can deadlock a `-j1` build. Keep permits until ALL children are reaped,
//! including when an earlier compiler fails. No cache or freshness shortcuts.
use std::io;
use std::process::Command;

pub fn run_commands<T>(commands: Vec<Command>, mut extra_slot: impl FnMut() -> Option<T>) -> io::Result<()> {
    let mut permits = Vec::new();
    for _ in 1..commands.len() {
        match extra_slot() {
            Some(permit) => permits.push(permit),
            None => break,
        }
    }
    // The calling thread consumes the implicit slot; worker threads consume
    // the extra permits. On unsupported jobservers this is simply serial.
    let jobs = std::sync::Mutex::new(commands.into_iter());
    let worker = || -> io::Result<()> {
        let mut first_error = None;
        loop {
            let command = jobs.lock().expect("native compiler queue poisoned").next();
            let Some(mut command) = command else { break };
            let description = format!("{command:?}");
            let result = command.status().and_then(|status| {
                if status.success() { Ok(()) }
                else { Err(io::Error::other(format!("native compiler failed ({status}): {description}"))) }
            });
            if let Err(error) = result {
                if first_error.is_none() { first_error = Some(error); }
            }
        }
        first_error.map_or(Ok(()), Err)
    };
    let result = std::thread::scope(|scope| {
        let threads: Vec<_> = permits.iter().map(|_| scope.spawn(&worker)).collect();
        let mut result = worker();
        for thread in threads {
            let completed = thread.join().expect("native compiler worker panicked");
            if result.is_ok() { result = completed; }
        }
        result
    });
    drop(permits);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("angel-native-jobs-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn command(&self, script: &str) -> Command {
            let mut command = Command::new("sh");
            command.current_dir(&self.0).args(["-c", script]);
            command
        }
    }
    impl Drop for Temp { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }

    #[test]
    fn serial_fallback_never_requests_more_after_no_token() {
        let temp = Temp::new();
        let mut requests = 0;
        let commands = (0..4).map(|n| temp.command(&format!("echo {n} >> order"))).collect();
        run_commands(commands, || { requests += 1; None::<()> }).unwrap();
        assert_eq!(requests, 1);
        assert_eq!(fs::read_to_string(temp.0.join("order")).unwrap(), "0\n1\n2\n3\n");
    }

    #[test]
    fn independent_compilers_overlap_only_with_extra_permits() {
        let temp = Temp::new();
        // Neither compiler can finish before the other has started. Bounded
        // polling makes a serial/incorrect implementation fail, never hang.
        let commands = (0..2).map(|n| temp.command(&format!(
            "touch started-{n}; i=0; while [ ! -f started-{} ]; do i=$((i+1)); [ $i -lt 100 ] || exit 17; sleep .02; done", 1-n
        ))).collect();
        run_commands(commands, || Some(())).unwrap();
    }

    #[test]
    fn errors_reap_running_compilers_before_returning_permits() {
        let temp = Temp::new();
        struct Permit<'a>(&'a PathBuf);
        impl Drop for Permit<'_> {
            fn drop(&mut self) { assert!(self.0.join("finished").exists(), "permit released with child still running"); }
        }
        let commands = vec![temp.command("exit 19"), temp.command("sleep .1; touch finished")];
        assert!(run_commands(commands, || Some(Permit(&temp.0))).is_err());
        assert!(temp.0.join("finished").exists());
    }

    #[test]
    fn spawn_failure_is_not_success_and_other_children_are_reaped() {
        let temp = Temp::new();
        let commands = vec![Command::new(temp.0.join("missing-compiler")), temp.command("touch finished")];
        assert!(run_commands(commands, || Some(())).is_err());
        assert!(temp.0.join("finished").exists());
    }

    #[test]
    fn empty_work_never_requests_tokens() {
        run_commands(Vec::new(), || -> Option<()> { panic!("no work") }).unwrap();
    }
}
