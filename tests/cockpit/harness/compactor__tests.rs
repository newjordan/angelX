use super::*;
use crate::agent::club::{ClubReply, ToolDef};
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

#[test]
fn a_digest_lands_on_its_receipt_framed_by_its_route() {
    let root = scratch("lands");
    let club = Scripted::new("- src/lib.rs:10 `fn main` calls ⠥⠑", Duration::ZERO);
    let helper: Arc<dyn Club> = club.clone();
    let mut history = vec![ChatMsg::tool("c1", RECEIPT)];
    let digesting = start(&helper, root.clone(), 0, "read_file|src/lib.rs", "BODY");
    let landed = finish(&mut history, vec![digesting]);
    assert_eq!((landed.landed, landed.missed), (1, 0));
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
    let digesting = start(&helper, root.clone(), 0, "grep|x", "BODY");
    let landed = finish(&mut history, vec![digesting]);
    assert_eq!((landed.landed, landed.missed, landed.bytes), (0, 1, 0));
    assert_eq!(&*history[0].content, RECEIPT);
    let _ = std::fs::remove_dir_all(root);
}

/// A slow digest is waited for: nothing cuts the helper off mid-thought.
#[test]
fn a_slow_digest_is_waited_for() {
    let root = scratch("slow");
    let helper: Arc<dyn Club> = Scripted::new("- took its time", Duration::from_millis(800));
    let mut history = vec![ChatMsg::tool("c1", RECEIPT)];
    let digesting = start(&helper, root.clone(), 0, "grep|x", "BODY");
    let landed = finish(&mut history, vec![digesting]);
    assert_eq!((landed.landed, landed.missed), (1, 0));
    assert!(history[0].content.ends_with("\n⡌⠃\n- took its time"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn an_empty_digest_is_a_miss() {
    let root = scratch("empty");
    let helper: Arc<dyn Club> = Scripted::new(" \n ", Duration::ZERO);
    let mut history = vec![ChatMsg::tool("c1", RECEIPT)];
    let digesting = start(&helper, root.clone(), 0, "grep|x", "BODY");
    let landed = finish(&mut history, vec![digesting]);
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
    let pending = vec![
        start(&fast, root.clone(), 0, "a", "BODY"),
        start(&also, root.clone(), 2, "b", "BODY"),
    ];
    // Side by side, two 300 ms digests land in well under 600 ms.
    let started = Instant::now();
    let landed = finish(&mut history, pending);
    assert!(started.elapsed() < Duration::from_millis(550));
    assert_eq!((landed.landed, landed.missed), (2, 0));
    assert!(history[0].content.ends_with("\n⡌⠃\n- first"));
    assert_eq!(&*history[1].content, "small result");
    assert!(history[2].content.ends_with("\n⡌⠃\n- second"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_long_digest_is_capped_with_the_middle_mark() {
    let long = "- line of digest\n".repeat(1_000);
    let note = render(&long).unwrap();
    assert!(note.starts_with("\n⡌⠃\n"));
    assert!(
        note.contains(&crate::agent::harness::book::d467_receipts::MIDDLE_BYTES.cells()),
        "{note}"
    );
    assert!(note.len() < DIGEST_MAX_BYTES + 256, "{}", note.len());
}

#[test]
fn the_compactor_routes_resolve_on_the_shelf() {
    use crate::agent::harness::book::{ledger, primary};
    let shelf = primary(COMPACTOR.cells().chars().next().unwrap()).unwrap();
    assert_eq!(shelf.name, "compactor");
    let root = scratch("ledger");
    let seat = ledger::read(&root, &COMPACTOR.cells()).unwrap();
    assert!(seat.contains("a handle to the tool output below"), "{seat}");
    let frame = ledger::read(&root, &COMPACTOR_DIGEST.cells()).unwrap();
    assert!(frame.contains("handle_read"), "{frame}");
    let _ = std::fs::remove_dir_all(root);
}

/// A paid-labelled driver: calls `dump` once, then keeps the tool message it
/// is handed on the next request.
struct Driver {
    hops: std::sync::atomic::AtomicUsize,
    saw: Mutex<String>,
}

impl Club for Driver {
    fn label(&self) -> &str {
        "deepseek-scripted"
    }
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Err("chat only".into())
    }
    fn chat(&self, messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        use crate::agent::club::{ChatRole, ToolCall};
        if self.hops.fetch_add(1, Ordering::SeqCst) == 0 {
            return Ok(ClubReply::Calls(vec![ToolCall {
                id: "dump-1".into(),
                name: "dump".into(),
                args: serde_json::json!({}),
            }]));
        }
        let tool = messages
            .iter()
            .rev()
            .find(|m| m.role == ChatRole::Tool)
            .expect("the dump result");
        *self.saw.lock().unwrap() = tool.content.to_string();
        Ok(ClubReply::Text("done".into()))
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

#[test]
fn a_parked_result_reaches_the_paid_root_with_the_local_digest() {
    if !enabled() {
        return; // another lane, or digests switched off in this environment
    }
    let driver = Driver {
        hops: Default::default(),
        saw: Mutex::new(String::new()),
    };
    let local = Scripted::new("- 2000 identical lines of bulk output", Duration::ZERO);
    let mut registry = crate::agent::harness::ToolRegistry::with_defaults();
    registry.register(Box::new(Dump));
    registry.set_aux_clubs(vec![local.clone() as Arc<dyn Club>]);
    let mut history = vec![ChatMsg::user("dump it")];
    let answer = crate::agent::harness::run_turn(
        &driver,
        &registry,
        &mut history,
        &AtomicBool::new(false),
        Some(4),
        &mpsc::channel().0,
    )
    .unwrap();
    assert_eq!(answer, "done");
    // The local seat read the bulk; the root got the receipt and the digest.
    let sent = local.seen.lock().unwrap().join("\n");
    assert!(sent.contains("⡌⠁") && sent.contains("one line of bulk output"));
    let saw = driver.saw.lock().unwrap().clone();
    assert!(
        saw.contains(crate::agent::harness::handle_store::HANDLE_RECEIPT_MARK),
        "{saw}"
    );
    assert!(
        saw.contains("\n⡌⠃\n- 2000 identical lines of bulk output"),
        "{saw}"
    );
    assert!(!saw.contains("one line of bulk output\none line"), "{saw}");
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
    let digesting = start(&helper, root.clone(), 0, "read_file|compact.rs", body);
    let landed = finish(&mut history, vec![digesting]);
    eprintln!(
        "live digest: {:?} in {:.1}s\n{}",
        landed,
        started.elapsed().as_secs_f64(),
        history[0].content
    );
    assert_eq!(landed.landed, 1);
    let _ = std::fs::remove_dir_all(root);
}
