//! The Kernel's evaluator: a small pool of threads, each keeping one warm
//! isolate. Every call still runs in a fresh context with the same 200 ms and
//! 32 MiB bounds; callers on any thread wait for their own reply.
use super::Warm;
use std::sync::{Arc, LazyLock, Mutex, mpsc};
use std::time::Duration;

const EVALUATORS: usize = 4;
const DEADLINE: Duration = Duration::from_millis(200);
const HEAP_MB: usize = 32;

type Job = (String, mpsc::SyncSender<Result<String, String>>);

/// An `angel_live::Evaluator`: run a module script on a warm pooled isolate.
/// A script carrying `CALL_MARKER` keeps its module setup per generation key.
pub fn evaluate(script: &str) -> Result<String, String> {
    static POOL: LazyLock<mpsc::Sender<Job>> = LazyLock::new(|| {
        let (tx, rx) = mpsc::channel::<Job>();
        let rx = Arc::new(Mutex::new(rx));
        for n in 0..EVALUATORS {
            let rx = Arc::clone(&rx);
            std::thread::Builder::new()
                .name(format!("angel-live-eval-{n}"))
                .spawn(move || {
                    let mut warm = Warm::new(HEAP_MB);
                    loop {
                        let job = rx.lock().unwrap_or_else(|p| p.into_inner()).recv();
                        let Ok((script, reply)) = job else { break };
                        let result = match split_call(&script) {
                            Some((key, setup, call)) => warm.run_cached(key, setup, call, DEADLINE),
                            None => warm.run(&script, DEADLINE),
                        };
                        let _ = reply.send(result);
                    }
                })
                .expect("live evaluator thread");
        }
        tx
    });
    let (reply, result) = mpsc::sync_channel(1);
    POOL.send((script.to_string(), reply))
        .map_err(|_| "live evaluator stopped".to_string())?;
    result
        .recv()
        .map_err(|_| "live evaluator stopped".to_string())?
}

/// `(key, setup, call)` of a script carrying `crate::CALL_MARKER`.
pub fn split_call(script: &str) -> Option<(&str, &str, &str)> {
    let at = script.find(crate::CALL_MARKER)?;
    let rest = &script[at + crate::CALL_MARKER.len()..];
    let (key, call) = rest.split_once("*/\n")?;
    Some((key, &script[..at], call))
}
