//! The wish grant runners (the user, 2026-10-20, after five friends quit
//! on the first floor: "the wishes took too long to grant so we need more
//! asynchronous runners in the background ready to add skills when they
//! are done").
//!
//! A wish the phrasebook does not know used to be taught by the host's own
//! angelX, one agent turn at a time: each lesson waited for the turn before
//! it (and for any other turn of the host's), and a fifth wish in flight was
//! handed back. Now each is one call to the host's model on a runner of its
//! own: [`RUNNERS`] threads stand ready from the moment the host opens its
//! door, each takes the next wish, asks the model for the phrasebook entry
//! as a reply (no tools, no conversation), checks it with the game's own
//! checker, has the model mend it once if the checker turns it away, and
//! hands the checked entry back. The host writes it into the workspace's
//! phrasebook and grants it on the next look, while everyone plays.

use crate::services::Oracle;
use crate::together_shooter::phrasebook;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

/// Runners standing ready: six knights each wishing at once, and room for
/// two more while a mend runs long.
pub const RUNNERS: usize = 8;

/// How long a grant's call may take. A whole phrasebook entry in one reply
/// ran about 47 s on DeepSeek Flash, and the clubs' general bound
/// (`ANGEL_HTTP_TIMEOUT`, 60 s, for any buffered call) cut its slow tail:
/// the wish was handed back (wave36's fresh box, 2026-10-21). A longer
/// bound set by the operator stands.
pub const GRANT_CALL: Duration = Duration::from_secs(90);

/// A call that timed out or failed is asked once more on its runner, with
/// this long, while the knight's lamp smokes on; only then is the wish
/// handed back.
pub const GRANT_RETRY: Duration = Duration::from_secs(120);

/// The longest a wish can take on its runner: its first call and the one
/// retry, and the mend. The host hands a wish back after this (and a little
/// more), whatever the runner says.
pub const GRANT_MOST: Duration = Duration::from_secs(GRANT_CALL.as_secs() * 2 + GRANT_RETRY.as_secs());

/// One wish for a runner: the model to ask, the brief, and where the
/// workspace keeps its pictures (for the checker).
pub struct Job {
    pub id: u64,
    pub oracle: Arc<dyn Oracle>,
    pub brief: String,
    pub sheets: PathBuf,
    pub paint: PathBuf,
}

/// Where a wish's time went on its runner.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Timing {
    /// Sent to the pool until a runner took it.
    pub queued: Duration,
    /// The model's first answer.
    pub model: Duration,
    /// The mend, when the checker turned the first answer away.
    pub mend: Option<Duration>,
}

/// A runner's answer: the checked entry and its wish's id, or why not.
pub struct Done {
    pub id: u64,
    pub result: Result<(String, phrasebook::Wish), String>,
    pub timing: Timing,
}

type Queue = Arc<Mutex<mpsc::Receiver<(Job, Instant)>>>;

/// The pool: its runners, the way to them and the way back.
pub struct Runners {
    jobs: mpsc::Sender<(Job, Instant)>,
    // The host polls this; runners only send. A mutex makes `Arc<Runners>` Sync.
    done: Mutex<mpsc::Receiver<Done>>,
    busy: Arc<AtomicUsize>,
    size: usize,
}

impl Runners {
    /// `size` runners, started now and waiting.
    pub fn new(size: usize) -> Self {
        let (jobs, queue) = mpsc::channel::<(Job, Instant)>();
        let (back, done) = mpsc::channel();
        let queue: Queue = Arc::new(Mutex::new(queue));
        let busy = Arc::new(AtomicUsize::new(0));
        for n in 0..size {
            let (queue, back, busy) = (Arc::clone(&queue), back.clone(), Arc::clone(&busy));
            let _ = std::thread::Builder::new()
                .name(format!("delve-grant-{n}"))
                .spawn(move || {
                    loop {
                        // One runner at a time waits at the queue; the rest
                        // wait on the lock, all of them ready.
                        let next = match queue.lock() {
                            Ok(rx) => rx.recv(),
                            Err(_) => return,
                        };
                        let Ok((job, sent)) = next else {
                            return;
                        };
                        busy.fetch_add(1, Ordering::SeqCst);
                        let done = run(job, sent.elapsed());
                        busy.fetch_sub(1, Ordering::SeqCst);
                        if back.send(done).is_err() {
                            return;
                        }
                    }
                });
        }
        Runners {
            jobs,
            done: Mutex::new(done),
            busy,
            size,
        }
    }

    /// Warm `oracle` (its address, its TLS, what the model is) off every
    /// thread here, so the first wish pays none of it.
    pub fn warm(&self, oracle: &Arc<dyn Oracle>) {
        let oracle = Arc::clone(oracle);
        let _ = std::thread::Builder::new()
            .name("delve-grant-warm".into())
            .spawn(move || oracle.warm_up());
    }

    pub fn send(&self, job: Job) {
        let _ = self.jobs.send((job, Instant::now()));
    }

    /// Every answer back since the last look.
    pub fn poll(&self) -> Vec<Done> {
        match self.done.lock() {
            Ok(rx) => rx.try_iter().collect(),
            Err(poisoned) => poisoned.into_inner().try_iter().collect(),
        }
    }

    /// Runners at work now.
    pub fn busy(&self) -> usize {
        self.busy.load(Ordering::SeqCst)
    }

    pub fn size(&self) -> usize {
        self.size
    }
}

/// The host's pool as its game holds it: the runners once started, and the
/// model they ask when not the club in hand (a test's, a bench's).
#[derive(Clone, Default)]
pub struct Pool {
    pub runners: Option<Arc<Runners>>,
    pub oracle: Option<Arc<dyn Oracle>>,
}

impl std::fmt::Debug for Pool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pool")
            .field("runners", &self.runners.as_ref().map(|r| (r.size(), r.busy())))
            .field("oracle", &self.oracle.as_ref().map(|c| c.label().to_string()))
            .finish()
    }
}

impl Pool {
    /// The runners, started (and `oracle` warmed) the first time they are
    /// wanted.
    pub fn ready(&mut self, oracle: &Arc<dyn Oracle>) -> Arc<Runners> {
        if let Some(runners) = &self.runners {
            return Arc::clone(runners);
        }
        let runners = Arc::new(Runners::new(RUNNERS));
        runners.warm(oracle);
        self.runners = Some(Arc::clone(&runners));
        runners
    }
}

/// One call of a wish's to the model, within `GRANT_CALL`; one that times
/// out or fails is asked again, within `GRANT_RETRY`, while the wish still
/// has its one retry (`retried`).
fn ask(oracle: &dyn Oracle, prompt: &str, retried: &mut bool) -> Result<String, String> {
    let first = oracle.respond(prompt, GRANT_CALL);
    match first {
        Err(why) if !*retried => {
            *retried = true;
            tracing::info!(%why, "a wish's call failed; asking once more");
            oracle.respond(prompt, GRANT_RETRY)
        }
        reply => reply,
    }
}

/// One wish on a runner: ask (once more if the call fails), check, mend once.
fn run(job: Job, queued: Duration) -> Done {
    let mut timing = Timing { queued, ..Timing::default() };
    let mut retried = false;
    let asked = Instant::now();
    let reply = ask(job.oracle.as_ref(), &job.brief, &mut retried);
    timing.model = asked.elapsed();
    let first = match reply {
        Ok(reply) => entry_of(&reply).ok_or_else(|| "the model answered without an entry".to_string()),
        Err(why) => Err(format!("the model could not be reached: {why}")),
    };
    let result = match first {
        Err(why) => Err(why),
        Ok(entry) => match definition(&entry, &job.sheets, &job.paint) {
            Ok(id) => Ok((entry, id)),
            Err(errors) => {
                let mended = Instant::now();
                let reply = ask(job.oracle.as_ref(), &mend_brief(&job.brief, &entry, &errors), &mut retried);
                timing.mend = Some(mended.elapsed());
                reply
                    .map_err(|why| format!("the model could not be reached: {why}"))
                    .and_then(|r| entry_of(&r).ok_or_else(|| errors.join("; ")))
                    .and_then(|entry| match definition(&entry, &job.sheets, &job.paint) {
                        Ok(id) => Ok((entry, id)),
                        Err(errors) => Err(errors.join("; ")),
                    })
            }
        },
    };
    Done { id: job.id, result, timing }
}

/// The phrasebook entry in a model's reply: from its `wish` line to the
/// end, out of any code fence, with nothing said before or after it.
pub fn entry_of(reply: &str) -> Option<String> {
    let mut lines: Vec<&str> = reply.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.trim_start().to_ascii_lowercase().starts_with("wish ") && l.contains('|'))?;
    lines.drain(..start);
    // A closing fence ends it, and so does a second entry.
    if let Some(end) = lines.iter().position(|l| l.trim_start().starts_with("```")) {
        lines.truncate(end);
    }
    if let Some(end) = lines
        .iter()
        .skip(1)
        .position(|l| l.trim_start().to_ascii_lowercase().starts_with("wish ") && l.contains('|'))
    {
        lines.truncate(end + 1);
    }
    let entry = lines.join("\n").trim_end().to_string();
    (!entry.is_empty()).then(|| format!("{entry}\n"))
}

/// The entry as the host's phrasebook will read it: its pictures cut, its
/// every tier checked. Its wish's id, or why it was turned away.
pub fn check(entry: &str, sheets: &std::path::Path, paint: &std::path::Path) -> Result<String, Vec<String>> {
    definition(entry, sheets, paint).map(|wish| wish.id)
}

pub fn definition(entry: &str, sheets: &std::path::Path, paint: &std::path::Path) -> Result<phrasebook::Wish, Vec<String>> {
    let (text, mut errors) = crate::together_files::phrasebook_with_sheets(entry, sheets, paint);
    let (book, rejected) = phrasebook::read_checked(&format!("{}\n{text}", phrasebook::LEARNED));
    errors.extend(rejected);
    match book.as_slice() {
        [wish] if errors.is_empty() => Ok(wish.clone()),
        [] if errors.is_empty() => Err(vec!["no wish entry was found".into()]),
        _ if errors.is_empty() => Err(vec!["write one entry, not several".into()]),
        _ => Err(errors),
    }
}

/// The mend: the brief again (the model's own words cached), its first
/// answer and what the checker said of it.
fn mend_brief(brief: &str, entry: &str, errors: &[String]) -> String {
    format!(
        "{brief}\n\nYOUR FIRST ANSWER:\n{entry}\nTHE GAME'S CHECKER TURNED IT AWAY:\n{}\n\nReply with the whole entry again, fixed so every tier checks (keep its id unless the checker rejected the id), and nothing else.",
        errors.iter().map(|e| format!("- {e}")).collect::<Vec<_>>().join("\n")
    )
}

/// The entry with its wish's id made `id` (when the book already has a
/// wish by the id the model chose).
pub fn renamed(entry: &str, from: &str, to: &str) -> String {
    let mut out = String::with_capacity(entry.len() + 4);
    let mut done = false;
    for line in entry.lines() {
        let trimmed = line.trim_start();
        if !done && trimmed.to_ascii_lowercase().starts_with("wish ") {
            if let Some((head, rest)) = trimmed.split_once('|')
                && head.split_whitespace().nth(1) == Some(from)
            {
                out.push_str(&format!("wish {to} |{rest}\n"));
                done = true;
                continue;
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// A model for tests and benches: it answers each prompt with the next of
/// its replies (the last again when they run out) after `delay`, and keeps
/// every prompt it was asked.
#[cfg(any(test, feature = "testing"))]
pub struct Scripted {
    pub replies: Mutex<std::collections::VecDeque<String>>,
    pub asked: Mutex<Vec<String>>,
    pub delay: Duration,
    /// Answers given at once, at most: how many prompts were in flight together.
    pub inside: AtomicUsize,
    pub most_inside: AtomicUsize,
    /// The whole-call bound each prompt was asked under (a 60 s policy as
    /// a club would read it, raised by the caller's floor).
    pub bounds: Mutex<Vec<Duration>>,
}

#[cfg(any(test, feature = "testing"))]
impl Scripted {
    pub fn new(replies: &[&str], delay: Duration) -> Arc<Self> {
        Arc::new(Scripted {
            replies: Mutex::new(replies.iter().map(|r| r.to_string()).collect()),
            asked: Mutex::new(Vec::new()),
            delay,
            inside: AtomicUsize::new(0),
            most_inside: AtomicUsize::new(0),
            bounds: Mutex::new(Vec::new()),
        })
    }
}

#[cfg(any(test, feature = "testing"))]
impl Oracle for Scripted {
    fn respond(&self, prompt: &str, at_least: Duration) -> Result<String, String> {
        let now = self.inside.fetch_add(1, Ordering::SeqCst) + 1;
        self.most_inside.fetch_max(now, Ordering::SeqCst);
        self.asked.lock().unwrap().push(prompt.to_string());
        self.bounds
            .lock()
            .unwrap()
            .push(Duration::from_secs(60).max(at_least));
        std::thread::sleep(self.delay);
        let reply = {
            let mut replies = self.replies.lock().unwrap();
            if replies.len() > 1 { replies.pop_front() } else { replies.front().cloned() }
        };
        self.inside.fetch_sub(1, Ordering::SeqCst);
        // A reply scripted `ERR: why` is a call that failed: a timeout, a
        // dropped link.
        match reply {
            Some(reply) => match reply.strip_prefix("ERR: ") {
                Some(why) => Err(why.to_string()),
                None => Ok(reply),
            },
            None => Err("no reply scripted".to_string()),
        }
    }

    fn label(&self) -> &str {
        "scripted"
    }
}

#[cfg(test)]
#[path = "../tests/together_grants__tests.rs"]
mod tests;
