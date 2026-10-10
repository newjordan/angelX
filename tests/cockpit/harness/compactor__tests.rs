use super::*;
use crate::agent::club::{ChatRole, ClubReply, ToolDef};
use std::sync::Mutex;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

/// A compactor that answers with `reply` after `delay` (or fails, when
/// `reply` is `Err`), keeping what it was sent.
struct Scripted {
    reply: Result<&'static str, &'static str>,
    delay: Duration,
    seen: Mutex<Vec<String>>,
}

impl Scripted {
    fn new(reply: &'static str, delay: Duration) -> Arc<Self> {
        Self::answering(Ok(reply), delay)
    }

    fn answering(reply: Result<&'static str, &'static str>, delay: Duration) -> Arc<Self> {
        Arc::new(Self {
            reply,
            delay,
            seen: Mutex::new(Vec::new()),
        })
    }
}

impl Club for Scripted {
    fn label(&self) -> &str {
        "scripted-compactor"
    }
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Err("chat only".into())
    }
    fn chat(&self, messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        self.seen
            .lock()
            .unwrap()
            .extend(messages.iter().map(|m| m.content.to_string()));
        std::thread::sleep(self.delay);
        self.reply
            .map(|text| ClubReply::Text(text.to_string()))
            .map_err(str::to_string)
    }
}

fn scratch(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("angel-compactor-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    root
}

const RECEIPT: &str = "[handle receipt: hnd_1 kind=tool_result producer=read_file identity=read_file|src/lib.rs bytes=40000 lines=900]\n⠥⠑";

fn digest(helper: &Arc<dyn Club>, root: &std::path::Path, index: usize) -> Digesting {
    start(
        helper,
        root.to_path_buf(),
        index,
        "hnd_1",
        "read_file|src/lib.rs",
        "BODY",
    )
}

#[test]
fn a_digest_that_has_landed_rides_its_receipt_framed_by_its_route() {
    let root = scratch("lands");
    let club = Scripted::new("- src/lib.rs:10 `fn main` calls ⠥⠑", Duration::ZERO);
    let helper: Arc<dyn Club> = club.clone();
    let mut history = vec![ChatMsg::tool("c1", RECEIPT)];
    let mut pending = vec![digest(&helper, &root, 0)];
    settle(&mut pending);
    let (landed, rolling) = attach_ready(&mut history, pending);
    assert_eq!((landed.landed, landed.missed, landed.rolling), (1, 0, 0));
    assert!(rolling.is_empty());
    // The seat reads its route; the call and the output ride beside it as data.
    let seen = club.seen.lock().unwrap().join("\n");
    assert!(seen.contains("⡌⠁\nread_file|src/lib.rs\n\nBODY"), "{seen}");
    // The receipt keeps its handle and gains the framed digest, its braille broken.
    let content = history[0].content.to_string();
    assert_eq!(
        content,
        format!("{RECEIPT}\n⡌⠃\n- src/lib.rs:10 `fn main` calls ·⠥·⠑")
    );
    assert_eq!(landed.bytes, content.len() - RECEIPT.len());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_digest_that_fails_leaves_the_bare_receipt() {
    let root = scratch("fails");
    let helper: Arc<dyn Club> = Scripted::answering(Err("helper unreachable"), Duration::ZERO);
    let mut history = vec![ChatMsg::tool("c1", RECEIPT)];
    let mut pending = vec![digest(&helper, &root, 0)];
    settle(&mut pending);
    let (landed, rolling) = attach_ready(&mut history, pending);
    assert_eq!((landed.landed, landed.missed, landed.bytes), (0, 1, 0));
    assert!(rolling.is_empty());
    assert_eq!(&*history[0].content, RECEIPT);
    let _ = std::fs::remove_dir_all(root);
}

/// A slow digest never holds the hop: the receipt goes out bare, the digest
/// rolls, and when it lands it joins the tail under its handle — once.
#[test]
fn a_slow_digest_rolls_and_lands_at_the_tail_under_its_handle() {
    let root = scratch("slow");
    let helper: Arc<dyn Club> = Scripted::new("- took its time", Duration::from_millis(400));
    let mut history = vec![ChatMsg::tool("c1", RECEIPT)];
    let started = Instant::now();
    let (landed, mut rolling) = attach_ready(&mut history, vec![digest(&helper, &root, 0)]);
    assert!(started.elapsed() < Duration::from_millis(100));
    assert_eq!((landed.landed, landed.rolling), (0, 1));
    assert_eq!(&*history[0].content, RECEIPT);
    // Still in flight: nothing joins the tail yet.
    let early = drain_late(&mut history, &mut rolling);
    assert_eq!((early.landed, early.rolling, history.len()), (0, 1, 1));
    settle(&mut rolling);
    let late = drain_late(&mut history, &mut rolling);
    assert_eq!((late.landed, late.rolling), (1, 0));
    assert_eq!(history.len(), 2);
    assert_eq!(history[1].role, ChatRole::Harness);
    assert_eq!(&*history[1].content, "⡌⠉ hnd_1\n- took its time");
    // The receipt that was already sent is never rewritten.
    assert_eq!(&*history[0].content, RECEIPT);
    // Nothing lands twice.
    let again = drain_late(&mut history, &mut rolling);
    assert_eq!((again.landed, history.len()), (0, 2));
    let _ = std::fs::remove_dir_all(root);
}

/// A helper that never answers (its host vanished mid-stream) holds nothing:
/// the hop and every later drain return at once.
#[test]
fn a_hung_digest_holds_nothing() {
    let root = scratch("hung");
    let helper: Arc<dyn Club> = Scripted::new("- never arrives", Duration::from_secs(30));
    let mut history = vec![ChatMsg::tool("c1", RECEIPT)];
    let started = Instant::now();
    let (_, mut rolling) = attach_ready(&mut history, vec![digest(&helper, &root, 0)]);
    let late = drain_late(&mut history, &mut rolling);
    assert!(started.elapsed() < Duration::from_millis(100));
    assert_eq!((late.landed, late.rolling), (0, 1));
    assert_eq!(&*history[0].content, RECEIPT);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn an_empty_digest_is_a_miss() {
    let root = scratch("empty");
    let helper: Arc<dyn Club> = Scripted::new(" \n ", Duration::ZERO);
    let mut history = vec![ChatMsg::tool("c1", RECEIPT)];
    let mut pending = vec![digest(&helper, &root, 0)];
    settle(&mut pending);
    let (landed, _) = attach_ready(&mut history, pending);
    assert_eq!((landed.landed, landed.missed), (0, 1));
    assert_eq!(&*history[0].content, RECEIPT);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn the_hops_digests_run_side_by_side_and_land_in_place() {
    let root = scratch("parallel");
    let fast: Arc<dyn Club> = Scripted::new("- first", Duration::from_millis(300));
    let also: Arc<dyn Club> = Scripted::new("- second", Duration::from_millis(300));
    let mut history = vec![
        ChatMsg::tool("c1", RECEIPT),
        ChatMsg::tool("c2", "small result"),
        ChatMsg::tool("c3", RECEIPT),
    ];
    let started = Instant::now();
    let mut pending = vec![
        start(&fast, root.clone(), 0, "hnd_1", "a", "BODY"),
        start(&also, root.clone(), 2, "hnd_2", "b", "BODY"),
    ];
    // Side by side, two 300 ms digests land in well under 600 ms.
    settle(&mut pending);
    assert!(started.elapsed() < Duration::from_millis(550));
    let (landed, _) = attach_ready(&mut history, pending);
    assert_eq!((landed.landed, landed.missed), (2, 0));
    assert!(history[0].content.ends_with("\n⡌⠃\n- first"));
    assert_eq!(&*history[1].content, "small result");
    assert!(history[2].content.ends_with("\n⡌⠃\n- second"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_long_digest_is_capped_with_the_middle_mark() {
    let long = "- line of digest\n".repeat(1_000);
    let note = render(Reading::Parked, &long);
    assert!(note.starts_with("\n⡌⠃\n"));
    assert!(
        note.contains(&crate::agent::harness::book::d467_receipts::MIDDLE_BYTES.cells()),
        "{note}"
    );
    assert!(note.len() < DIGEST_MAX_BYTES + 256, "{}", note.len());
}

#[test]
fn the_compactor_routes_resolve_on_the_shelf() {
    use crate::agent::harness::book::st_connected::HANDOFF;
    use crate::agent::harness::book::{ledger, primary};
    let shelf = primary(COMPACTOR.cells().chars().next().unwrap()).unwrap();
    assert_eq!(shelf.name, "compactor");
    let root = scratch("ledger");
    let seat = ledger::read(&root, &COMPACTOR.cells()).unwrap();
    assert!(seat.contains("a handle to the tool output below"), "{seat}");
    for route in [COMPACTOR_DIGEST, COMPACTOR_LATE] {
        let frame = ledger::read(&root, &route.cells()).unwrap();
        assert!(frame.contains("handle_read"), "{frame}");
    }
    let overseer = ledger::read(&root, &OVERSEER.cells()).unwrap();
    assert!(overseer.contains("progress or a stall"), "{overseer}");
    let read = ledger::read(&root, &OVERSEER_READ.cells()).unwrap();
    assert!(read.contains("proc_status"), "{read}");
    let handoff = ledger::read(&root, &HANDOFF.cells()).unwrap();
    assert!(handoff.contains("keeps running"), "{handoff}");
    let _ = std::fs::remove_dir_all(root);
}

const HANDOFF_RECEIPT: &str = "[handoff: job 7 pid 4242 ran 600s, no output 540s]\n$ python3 solve.py\n  4242 R cpu 99% kissat x.cnf\n⡌⠙⠏⠙";

#[test]
fn a_handoff_receipt_names_its_job_and_its_read_rolls_like_a_digest() {
    assert_eq!(handoff_job(HANDOFF_RECEIPT).as_deref(), Some("job 7"));
    assert_eq!(handoff_job(RECEIPT), None);
    let root = scratch("handoff");
    let club = Scripted::new(
        "- kissat SAT solve, CPU-bound, no output: a stall",
        Duration::ZERO,
    );
    let helper: Arc<dyn Club> = club.clone();
    // Landed in time: it rides the hand-off receipt under ⡌⠋.
    let mut history = vec![ChatMsg::tool("c1", HANDOFF_RECEIPT)];
    let mut pending = vec![start_handoff(
        &helper,
        root.clone(),
        0,
        "job 7",
        HANDOFF_RECEIPT,
    )];
    settle(&mut pending);
    let (landed, _) = attach_ready(&mut history, pending);
    assert_eq!(landed.landed, 1);
    assert!(
        history[0]
            .content
            .ends_with("\n⡌⠋\n- kissat SAT solve, CPU-bound, no output: a stall")
    );
    let seen = club.seen.lock().unwrap().join("\n");
    assert!(seen.starts_with("⡌⠑\njob 7\n\n[handoff: job 7"), "{seen}");
    // Late: it joins the tail named by its job.
    let slow: Arc<dyn Club> = Scripted::new("- still solving", Duration::from_millis(200));
    let mut history = vec![ChatMsg::tool("c1", HANDOFF_RECEIPT)];
    let (_, mut rolling) = attach_ready(
        &mut history,
        vec![start_handoff(
            &slow,
            root.clone(),
            0,
            "job 7",
            HANDOFF_RECEIPT,
        )],
    );
    settle(&mut rolling);
    drain_late(&mut history, &mut rolling);
    assert_eq!(&*history[1].content, "⡌⠋ job 7\n- still solving");
    let _ = std::fs::remove_dir_all(root);
}

/// A paid-labelled driver: calls `calls` in order, one per request, thinking
/// for `think` before every call after the first; then answers. Keeps every
/// request's messages.
struct Driver {
    calls: &'static [&'static str],
    think: Duration,
    hops: std::sync::atomic::AtomicUsize,
    requests: Mutex<Vec<Vec<ChatMsg>>>,
}

impl Driver {
    fn new(calls: &'static [&'static str], think: Duration) -> Self {
        Self {
            calls,
            think,
            hops: Default::default(),
            requests: Mutex::new(Vec::new()),
        }
    }
}

impl Club for Driver {
    fn label(&self) -> &str {
        "deepseek-scripted"
    }
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Err("chat only".into())
    }
    fn chat(&self, messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        use crate::agent::club::ToolCall;
        self.requests.lock().unwrap().push(messages.to_vec());
        let hop = self.hops.fetch_add(1, Ordering::SeqCst);
        let Some(name) = self.calls.get(hop) else {
            return Ok(ClubReply::Text("done".into()));
        };
        if hop > 0 {
            std::thread::sleep(self.think);
        }
        Ok(ClubReply::Calls(vec![ToolCall {
            id: format!("{name}-{hop}"),
            name: (*name).into(),
            args: serde_json::json!({}),
        }]))
    }
}

/// A read-only tool with 48 KB of output: bulk the lane parks.
struct Dump;

impl crate::agent::harness::Tool for Dump {
    fn name(&self) -> &str {
        "dump"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "dump".into(),
            description: "bulk fixture".into(),
            params: serde_json::json!({"type": "object"}),
        }
    }
    fn call(&self, _: &serde_json::Value) -> Result<String, String> {
        Ok("one line of bulk output\n".repeat(2_000))
    }
}

struct Noop;

impl crate::agent::harness::Tool for Noop {
    fn name(&self) -> &str {
        "noop"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "noop".into(),
            description: "small fixture".into(),
            params: serde_json::json!({"type": "object"}),
        }
    }
    fn call(&self, _: &serde_json::Value) -> Result<String, String> {
        Ok("ok".into())
    }
}

fn late_note(request: &[ChatMsg]) -> Option<String> {
    request
        .iter()
        .find(|m| m.role == ChatRole::Harness && m.content.starts_with("⡌⠉ hnd_"))
        .map(|m| m.content.to_string())
}

fn registry_with(local: &Arc<Scripted>) -> crate::agent::harness::ToolRegistry {
    let mut registry = crate::agent::harness::ToolRegistry::with_defaults();
    registry.register(Box::new(Dump));
    registry.register(Box::new(Noop));
    registry.set_aux_clubs(vec![local.clone() as Arc<dyn Club>]);
    registry
}

/// End to end: the hop that parks the bulk sends a bare receipt at once (the
/// 300 ms digest is still rolling); the digest lands while the root thinks
/// and reaches it at the tail of the next request.
#[test]
fn a_parked_result_reaches_the_paid_root_and_its_digest_follows_at_the_tail() {
    if !enabled() {
        return; // another lane, or digests switched off in this environment
    }
    let driver = Driver::new(&["dump", "noop"], Duration::from_millis(800));
    let local = Scripted::new(
        "- 2000 identical lines of bulk output",
        Duration::from_millis(300),
    );
    let registry = registry_with(&local);
    let mut history = vec![ChatMsg::user("dump it")];
    let answer = crate::agent::harness::run_turn(
        &driver,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(6),
        &mpsc::channel().0,
    )
    .unwrap();
    assert_eq!(answer, "done");
    // The local seat read the bulk.
    let sent = local.seen.lock().unwrap().join("\n");
    assert!(sent.contains("⡌⠁") && sent.contains("one line of bulk output"));
    let requests = driver.requests.lock().unwrap().clone();
    assert_eq!(requests.len(), 3);
    // Request 2: the receipt, bare — the hop did not wait for the digest.
    let receipt = requests[1]
        .iter()
        .rev()
        .find(|m| m.role == ChatRole::Tool)
        .expect("the dump result")
        .content
        .to_string();
    assert!(
        receipt.contains(crate::agent::harness::handle_store::HANDLE_RECEIPT_MARK),
        "{receipt}"
    );
    assert!(!receipt.contains("⡌⠃"), "{receipt}");
    assert!(
        !receipt.contains("one line of bulk output\none line"),
        "{receipt}"
    );
    assert_eq!(late_note(&requests[1]), None);
    // Request 3: the digest at the tail, named by the receipt's handle.
    let note = late_note(&requests[2]).expect("the late digest");
    assert!(
        note.ends_with("\n- 2000 identical lines of bulk output"),
        "{note}"
    );
    let handle = crate::agent::harness::handle_store::receipt_handle(&receipt).unwrap();
    assert!(note.starts_with(&format!("⡌⠉ {handle}\n")), "{note}");
}

/// A digest still rolling when the driver's turn ends waits in the registry
/// and joins the tail of the next turn's first request.
#[test]
fn a_digest_still_rolling_at_turn_end_lands_in_the_next_turn() {
    if !enabled() {
        return;
    }
    let local = Scripted::new("- the bulk, digested", Duration::from_millis(300));
    let registry = registry_with(&local);
    // dump, then answer at once: the digest is still out when the turn ends.
    let first = Driver::new(&["dump"], Duration::ZERO);
    let mut history = vec![ChatMsg::user("dump it")];
    let answer = crate::agent::harness::run_turn(
        &first,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        None,
        &mpsc::channel().0,
    )
    .unwrap();
    assert_eq!(answer, "done");
    assert_eq!(registry.rolling_digests.lock().unwrap().len(), 1);
    assert!(history.iter().all(|m| !m.content.starts_with("⡌⠉")));
    settle(&mut registry.rolling_digests.lock().unwrap());
    // The next turn's first request carries it, once.
    let second = Driver::new(&[], Duration::ZERO);
    history.push(ChatMsg::user("and now?"));
    crate::agent::harness::run_turn(
        &second,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        None,
        &mpsc::channel().0,
    )
    .unwrap();
    let requests = second.requests.lock().unwrap().clone();
    let note = late_note(&requests[0]).expect("the digest from the previous turn");
    assert!(note.ends_with("\n- the bulk, digested"), "{note}");
    assert!(registry.rolling_digests.lock().unwrap().is_empty());
    assert_eq!(
        history
            .iter()
            .filter(|m| m.content.starts_with("⡌⠉"))
            .count(),
        1
    );
}

/// A tool result that is a hand-off receipt (as the shell returns one).
struct Handed;

impl crate::agent::harness::Tool for Handed {
    fn name(&self) -> &str {
        "handed"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "handed".into(),
            description: "hand-off fixture".into(),
            params: serde_json::json!({"type": "object"}),
        }
    }
    fn call(&self, _: &serde_json::Value) -> Result<String, String> {
        Ok(HANDOFF_RECEIPT.into())
    }
}

/// End to end: a hand-off receipt in the batch starts the helper's read
/// (any lane), and the read reaches the root rolling like a digest.
#[test]
fn a_handed_off_job_is_read_by_the_helper_and_the_read_reaches_the_root() {
    let driver = Driver::new(&["handed", "noop"], Duration::from_millis(600));
    let local = Scripted::new(
        "- CPU-bound solver, no output: likely a stall",
        Duration::from_millis(200),
    );
    let mut registry = registry_with(&local);
    registry.register(Box::new(Handed));
    let (events, notices) = mpsc::channel();
    let mut history = vec![ChatMsg::user("solve it")];
    crate::agent::harness::run_turn(
        &driver,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(6),
        &events,
    )
    .unwrap();
    let sent = local.seen.lock().unwrap().join("\n");
    assert!(sent.contains("⡌⠑\njob 7\n\n[handoff: job 7"), "{sent}");
    let requests = driver.requests.lock().unwrap().clone();
    let read = requests[2]
        .iter()
        .find(|m| m.role == ChatRole::Harness && m.content.starts_with("⡌⠋ job 7"))
        .map(|m| m.content.to_string())
        .expect("the helper's read at the tail");
    assert!(
        read.ends_with("\n- CPU-bound solver, no output: likely a stall"),
        "{read}"
    );
    let said: Vec<String> = notices
        .try_iter()
        .filter_map(|event| match event {
            crate::agent::harness::TurnEvent::Notice(text) => Some(text),
            _ => None,
        })
        .collect();
    assert!(
        said.iter()
            .any(|text| text.starts_with("tool hand-off: handed") && text.contains("job 7")),
        "{said:?}"
    );
}

/// Live: the real helper path (HTTP club → connected seat → server). Run with
/// `ANGEL_TREEBEARD_LIVE_URL=http://host:port/v1 cargo test -- --ignored live_`.
#[test]
#[ignore = "live: needs ANGEL_TREEBEARD_LIVE_URL"]
fn live_compactor_digests_a_parked_output() {
    let Ok(url) = std::env::var("ANGEL_TREEBEARD_LIVE_URL") else {
        return;
    };
    let model = std::env::var("ANGEL_TREEBEARD_LIVE_MODEL").unwrap_or_else(|_| "treebeard".into());
    let helper: Arc<dyn Club> = Arc::new(crate::agent::club::HttpClub::new(
        "compact", url, model, None,
    ));
    let source = include_str!("../../../cockpit/src/agent/harness/compact.rs");
    let body = &source[..source
        .char_indices()
        .nth(40_000)
        .map_or(source.len(), |(i, _)| i)];
    let root = scratch("live");
    let mut history = vec![ChatMsg::tool("c1", RECEIPT)];
    let started = Instant::now();
    let mut pending = vec![start(
        &helper,
        root.clone(),
        0,
        "hnd_1",
        "read_file|compact.rs",
        body,
    )];
    settle(&mut pending);
    let (landed, _) = attach_ready(&mut history, pending);
    eprintln!(
        "live digest: {:?} in {:.1}s\n{}",
        landed,
        started.elapsed().as_secs_f64(),
        history[0].content
    );
    assert_eq!(landed.landed, 1);
    let _ = std::fs::remove_dir_all(root);
}
