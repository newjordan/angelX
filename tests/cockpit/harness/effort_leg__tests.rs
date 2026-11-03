use super::*;
use crate::agent::club::ClubReply;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

struct Seat {
    levels: Vec<String>,
    effort: Option<String>,
    asked: Mutex<Vec<Option<String>>>,
}

impl Seat {
    fn new(levels: &[&str], effort: Option<&str>) -> Self {
        Self {
            levels: levels.iter().map(|level| level.to_string()).collect(),
            effort: effort.map(str::to_string),
            asked: Mutex::default(),
        }
    }

    fn asked(&self) -> Vec<Option<String>> {
        self.asked.lock().unwrap().clone()
    }
}

impl Club for Seat {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Ok("done".into())
    }
    fn label(&self) -> &str {
        "muse"
    }
    fn model_identity(&self) -> Option<String> {
        Some("muse-spark-1.3".into())
    }
    fn reasoning_levels(&self) -> &[String] {
        &self.levels
    }
    fn reasoning_effort(&self) -> Option<String> {
        self.effort.clone()
    }
    fn chat(&self, _messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        self.asked.lock().unwrap().push(None);
        Ok(ClubReply::Text("done".into()))
    }
    fn chat_streaming_with_effort(
        &self,
        _messages: &[ChatMsg],
        _tools: &[ToolDef],
        effort: Option<&str>,
        _cancel: &AtomicBool,
        _on_delta: &mut dyn FnMut(crate::agent::club::StreamDelta),
    ) -> Result<ClubReply, String> {
        self.asked.lock().unwrap().push(effort.map(str::to_string));
        Ok(ClubReply::Text("done".into()))
    }
}

/// A private copy of the shipped `effort` module under a throwaway live root,
/// so a test can edit the leg exactly as Muse would.
struct Staged {
    root: PathBuf,
    modules: angel_live::Registry,
}

impl Staged {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "angel-effort-leg-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let module = root.join("effort");
        std::fs::create_dir_all(&module).unwrap();
        let shipped = Path::new(env!("CARGO_MANIFEST_DIR")).join("live/effort");
        for entry in std::fs::read_dir(shipped).unwrap() {
            let entry = entry.unwrap();
            std::fs::copy(entry.path(), module.join(entry.file_name())).unwrap();
        }
        let modules = angel_live::Registry::new(root.clone(), crate::agent::hotload::evaluate);
        Self { root, modules }
    }

    fn edit(&self, file: &str, from: &str, to: &str) {
        let path = self.root.join("effort").join(file);
        let source = std::fs::read_to_string(&path).unwrap();
        assert!(source.contains(from), "{file} lacks {from:?}");
        std::fs::write(path, source.replacen(from, to, 1)).unwrap();
    }

    /// Publish the module's current generation to the registry, as the app's
    /// hot-load tick does.
    fn publish(&mut self, tools: &ToolRegistry) -> String {
        self.modules.refresh(true);
        let snapshot = self.modules.snapshot();
        let status = snapshot.statuses.iter().find(|s| s.id == "effort").unwrap();
        assert_eq!(status.error, None);
        tools.set_live_policies(snapshot.policies.clone());
        snapshot.policies["effort"].digest().to_string()
    }
}

impl Drop for Staged {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn turn(tools: &ToolRegistry, club: &Seat, task: &str) -> (Option<String>, Option<LegSource>) {
    let hop = Hop {
        index: 1,
        friction: false,
        tool_errors: 0,
        failed_checks: 0,
        red: false,
        stuck: false,
    };
    decide(tools, club, &TaskHead::new(task), &hop, None)
}

const LADDERS: [&[&str]; 4] = [
    &["low", "medium", "high", "xhigh"],
    &["none", "low", "high"],
    &["minimal", "low", "medium", "high", "xhigh"],
    &["low", "high"],
];

#[test]
fn the_shipped_leg_reproduces_the_native_effort_decision_over_a_table() {
    let mut staged = Staged::new();
    let tools = ToolRegistry::with_defaults();
    let digest = staged.publish(&tools);
    tools.pin_live_policies();
    let tasks = [
        "",
        "fix the failing test in src/lib.rs",
        "design the architecture of the new cache",
        &"explain ".repeat(900),
    ];
    let mut rows = 0;
    for ladder in LADDERS {
        // A configured effort the ladder does not name (a wire spelling) must
        // still pass through unchanged.
        for configured in [None, Some("low"), Some("high"), Some("max")] {
            for adaptive in [false, true] {
                for hop in 0..4 {
                    for friction in [false, true] {
                        for (tool_errors, failed_checks) in [(0, 0), (3, 2)] {
                            for task in tasks {
                                let club = Seat::new(ladder, configured);
                                let native = if adaptive {
                                    resolve_adaptive_reasoning_effort(&club, hop, friction)
                                } else {
                                    None
                                };
                                let hop = Hop {
                                    index: hop,
                                    friction,
                                    tool_errors,
                                    failed_checks,
                                    red: false,
                                    stuck: false,
                                };
                                let (effort, source) = decide(
                                    &tools,
                                    &club,
                                    &TaskHead::new(task),
                                    &hop,
                                    native.clone(),
                                );
                                assert_eq!(effort, native, "{ladder:?} {configured:?} {adaptive}");
                                assert_eq!(
                                    source,
                                    Some(LegSource::Live {
                                        digest: digest.clone()
                                    })
                                );
                                rows += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(rows, 4 * 4 * 2 * 4 * 2 * 2 * 4);
}

#[test]
fn a_seat_without_effort_levels_never_consults_the_leg() {
    let mut staged = Staged::new();
    let tools = ToolRegistry::with_defaults();
    staged.publish(&tools);
    tools.pin_live_policies();
    let club = Seat::new(&[], None);
    assert_eq!(turn(&tools, &club, "fix it"), (None, None));
    assert_eq!(
        decide(
            &tools,
            &club,
            &TaskHead::new("fix it"),
            &Hop {
                index: 0,
                friction: false,
                tool_errors: 0,
                failed_checks: 0,
                red: false,
                stuck: false,
            },
            Some("high".into())
        ),
        (Some("high".into()), None)
    );
}

#[test]
fn without_a_live_module_the_shipped_leg_decides() {
    let tools = ToolRegistry::with_defaults();
    let club = Seat::new(LADDERS[0], Some("medium"));
    let (effort, source) = turn(&tools, &club, "fix it");
    assert_eq!(effort, None);
    assert!(matches!(source, Some(LegSource::Shipped { .. })), "{source:?}");
}

#[test]
fn an_edit_is_pinned_per_turn_and_its_choice_is_used_by_the_next_one() {
    let mut staged = Staged::new();
    let tools = ToolRegistry::with_defaults();
    let first = staged.publish(&tools);
    tools.pin_live_policies();
    let club = Seat::new(LADDERS[2], Some("medium"));
    let (effort, source) = turn(&tools, &club, "fix the failing test");
    assert_eq!(
        (effort, source),
        (
            None,
            Some(LegSource::Live {
                digest: first.clone()
            })
        )
    );

    staged.edit(
        "index.js",
        "return { effort: answer.$ === 'Some' ? answer.value : null }",
        "return { effort: input.hop > 0 ? 'xhigh' : 'minimal' }",
    );
    let second = staged.publish(&tools);
    assert_ne!(first, second);
    // Published mid-turn: this turn still decides with the generation it pinned.
    assert_eq!(
        turn(&tools, &club, "fix the failing test").1,
        Some(LegSource::Live { digest: first })
    );
    tools.pin_live_policies();
    assert_eq!(
        turn(&tools, &club, "fix the failing test"),
        (
            Some("xhigh".into()),
            Some(LegSource::Live { digest: second })
        )
    );
}

#[test]
fn an_edited_leg_steers_the_next_real_turn() {
    let _guard = crate::tests::env_lock();
    let mut staged = Staged::new();
    let tools = ToolRegistry::with_defaults();
    staged.publish(&tools);
    let club = Seat::new(LADDERS[2], Some("medium"));
    let (tx, _rx) = mpsc::channel();
    let ask = |text: &str| {
        let mut history = vec![ChatMsg::user(text)];
        run_turn(
            &club,
            &tools,
            &mut history,
            &AtomicBool::new(false),
            Some(4),
            &tx,
        )
        .unwrap();
    };
    ask("fix the failing test");
    staged.edit(
        "index.js",
        "return { effort: answer.$ === 'Some' ? answer.value : null }",
        "return { effort: input.task.includes('test') ? 'high' : null }",
    );
    staged.publish(&tools);
    ask("fix the failing test");
    ask("write a poem");
    assert_eq!(club.asked(), [None, Some("high".into()), None]);
}

#[test]
fn a_choice_outside_the_allowed_levels_falls_back_to_native() {
    let mut staged = Staged::new();
    let tools = ToolRegistry::with_defaults();
    let club = Seat::new(LADDERS[3], Some("low"));
    for (answer, why) in [
        ("({ effort: 'turbo' })", "turbo"),
        ("({ effort: 7 })", "invalid type"),
        ("({ effort: 'medium' })", "medium"),
    ] {
        staged.edit(
            "index.js",
            "return { effort: answer.$ === 'Some' ? answer.value : null }",
            &format!("return {answer}"),
        );
        let digest = staged.publish(&tools);
        tools.pin_live_policies();
        let native = Some("high".to_string());
        let (effort, source) = decide(
            &tools,
            &club,
            &TaskHead::new("fix it"),
            &Hop {
                index: 0,
                friction: false,
                tool_errors: 0,
                failed_checks: 0,
                red: false,
                stuck: false,
            },
            native.clone(),
        );
        assert_eq!(effort, native, "{answer}");
        let Some(LegSource::LiveFallback {
            digest: kept,
            error,
        }) = source
        else {
            panic!("{answer} must fall back: {source:?}");
        };
        assert_eq!(kept, digest);
        assert!(error.contains(why), "{error}");
        staged.edit(
            "index.js",
            &format!("return {answer}"),
            "return { effort: answer.$ === 'Some' ? answer.value : null }",
        );
    }
}

#[test]
fn receipts_name_the_source_and_the_generation() {
    let digest = "0123456789abcdef".to_string();
    assert_eq!(receipt(&LegSource::Native), ("native".into(), None));
    assert_eq!(
        receipt(&LegSource::Live {
            digest: digest.clone()
        }),
        ("live:01234567".into(), None)
    );
    assert_eq!(
        receipt(&LegSource::LiveFallback {
            digest,
            error: "turbo is not an effort this seat accepts".into()
        }),
        (
            "live-fallback:01234567".into(),
            Some("turbo is not an effort this seat accepts".into())
        )
    );
    let club = Seat::new(LADDERS[0], None);
    crate::agent::harness::reset_turn_ledger(&club);
    crate::agent::harness::trajectory::note_timing_origin(std::time::Instant::now());
    crate::agent::harness::trajectory::begin_model_request();
    crate::agent::harness::trajectory::note_effort_leg(Some((
        "live:01234567".into(),
        Some("why".into()),
    )));
    crate::agent::harness::trajectory::begin_provider_attempt(1).unwrap();
    crate::agent::harness::trajectory::end_model_request();
    let samples = crate::agent::harness::provider_call_samples();
    assert_eq!(samples[0]["effort_leg"], "live:01234567");
    assert_eq!(samples[0]["effort_leg_error"], "why");
}

#[test]
#[ignore = "requires pinned optional Bend developer compiler; run explicitly"]
fn the_bend_source_decides_and_its_guard_holds_for_any_edit() {
    let mut staged = Staged::new();
    let tools = ToolRegistry::with_defaults();
    let club = Seat::new(LADDERS[2], Some("medium"));
    let native = Some("medium".to_string());
    let hop = Hop {
        index: 2,
        friction: true,
        tool_errors: 1,
        failed_checks: 1,
        red: false,
        stuck: false,
    };
    let task = TaskHead::new("the test fails with a panic");
    for (choice, expected) in [
        ("Some{\"high\"}", Some("high")),
        // Not a level of this seat: the Bend guard hands back the native effort.
        ("Some{\"turbo\"}", Some("medium")),
        ("None{}", None),
    ] {
        staged.edit(
            "effort.bend",
            "Facts{seat, hop, friction, tool_errors, failed_checks, task_len, tests, errors, design, red, stuck}}:\n      wake(red || stuck, native)",
            &format!("Facts{{seat, hop, friction, tool_errors, failed_checks, task_len, tests, errors, design, red, stuck}}}}:\n      {choice}"),
        );
        let digest = staged.publish(&tools);
        tools.pin_live_policies();
        let (effort, source) = decide(&tools, &club, &task, &hop, native.clone());
        assert_eq!(effort.as_deref(), expected, "{choice}");
        assert_eq!(source, Some(LegSource::Live { digest }));
        staged.edit(
            "effort.bend",
            &format!("Facts{{seat, hop, friction, tool_errors, failed_checks, task_len, tests, errors, design, red, stuck}}}}:\n      {choice}"),
            "Facts{seat, hop, friction, tool_errors, failed_checks, task_len, tests, errors, design, red, stuck}}:\n      wake(red || stuck, native)",
        );
    }
}

fn median_micros(program: &angel_live::Program, input: &serde_json::Value) -> (u128, u128) {
    for _ in 0..50 {
        program.render(input).unwrap();
    }
    let mut micros: Vec<u128> = (0..500)
        .map(|_| {
            let start = std::time::Instant::now();
            program.render(input).unwrap();
            start.elapsed().as_micros()
        })
        .collect();
    micros.sort_unstable();
    (micros[micros.len() / 2], micros[micros.len() * 95 / 100])
}

#[test]
fn a_decision_cost_is_bounded() {
    let input = serde_json::json!({
        "seat": "muse-spark-1.3",
        "native": "medium",
        "allowed": LADDERS[2],
        "hop": 3,
        "friction": true,
        "tool_errors": 2,
        "failed_checks": 1,
        "task": "fix the failing test ".repeat(95),
        "task_len": 1995,
    });
    let mut staged = Staged::new();
    let tools = ToolRegistry::with_defaults();
    staged.publish(&tools);
    tools.pin_live_policies();
    let (median, p95) = median_micros(&tools.live_policy("effort").unwrap(), &input);
    // The same request through an adapter that answers before reaching Bend:
    // the isolate start-up every live leg pays.
    staged.edit(
        "index.js",
        "render(input) {",
        "render(input) {\n    return { effort: null }",
    );
    staged.publish(&tools);
    tools.pin_live_policies();
    let (floor, _) = median_micros(&tools.live_policy("effort").unwrap(), &input);
    eprintln!(
        "effort leg render over 500 calls: median {median} us, p95 {p95} us; isolate floor {floor} us"
    );
    assert!(median < 50_000);
}

const DEEPSEEK_V4: &[&str] = &["none", "low", "high", "max"];

fn hop_with(red: bool, stuck: bool) -> Hop {
    Hop {
        index: 5,
        friction: false,
        tool_errors: 0,
        failed_checks: usize::from(red),
        red,
        stuck,
    }
}

#[test]
fn a_red_or_a_silent_repeat_wakes_a_thinking_off_seat_for_one_hop() {
    let mut staged = Staged::new();
    let tools = ToolRegistry::with_defaults();
    staged.publish(&tools);
    tools.pin_live_policies();
    let task = TaskHead::new("make the bowling tests pass");
    let off = Seat::new(DEEPSEEK_V4, Some("none"));
    for (red, stuck, expected) in [
        (true, false, Some("high")),
        (false, true, Some("high")),
        (true, true, Some("high")),
        // The hop after: back to the seat's own effort.
        (false, false, None),
    ] {
        let (effort, _) = decide(&tools, &off, &task, &hop_with(red, stuck), None);
        assert_eq!(effort.as_deref(), expected, "red {red} stuck {stuck}");
    }
    // A seat already thinking keeps its rung (no cache re-key, never lowered).
    for configured in ["low", "high", "max"] {
        let seat = Seat::new(DEEPSEEK_V4, Some(configured));
        assert_eq!(
            decide(&tools, &seat, &task, &hop_with(true, true), None).0,
            None
        );
    }
    // A thinking-off seat with no `high` keeps native.
    let narrow = Seat::new(&["none", "low"], Some("none"));
    assert_eq!(decide(&tools, &narrow, &task, &hop_with(true, true), None).0, None);
    // A seat with no ladder is never consulted.
    let bare = Seat::new(&[], Some("none"));
    assert_eq!(
        decide(&tools, &bare, &task, &hop_with(true, true), None),
        (None, None)
    );
}

#[test]
fn the_shipped_leg_wakes_a_thinking_off_seat_after_a_red() {
    let tools = ToolRegistry::with_defaults();
    let club = Seat::new(DEEPSEEK_V4, Some("none"));
    let (effort, source) = decide(
        &tools,
        &club,
        &TaskHead::new("fix it"),
        &hop_with(true, false),
        None,
    );
    assert_eq!(effort.as_deref(), Some("high"));
    assert!(matches!(source, Some(LegSource::Shipped { .. })), "{source:?}");
}

#[test]
fn a_red_is_a_fact_for_the_one_hop_after_it() {
    let mut seen = RedSeen::default();
    assert!(!seen.take(0));
    assert!(seen.take(1));
    assert!(!seen.take(1));
    assert!(seen.take(3));
    assert!(!seen.take(3));
}

/// A thinking-off seat that plays a script of tool batches, then answers,
/// recording the effort of every request and the turn it was asked under.
struct Script {
    steps: Vec<Vec<ToolCall>>,
    asked: Mutex<Vec<(Option<String>, String)>>,
}

impl Script {
    fn new(steps: Vec<Vec<ToolCall>>) -> Self {
        Self {
            steps,
            asked: Mutex::default(),
        }
    }

    fn efforts(&self) -> Vec<Option<String>> {
        self.asked.lock().unwrap().iter().map(|(e, _)| e.clone()).collect()
    }
}

impl Club for Script {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Ok("done".into())
    }
    fn label(&self) -> &str {
        "deepseek"
    }
    fn model_identity(&self) -> Option<String> {
        Some("deepseek-v4-flash".into())
    }
    fn reasoning_levels(&self) -> &[String] {
        static LEVELS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        LEVELS.get_or_init(|| DEEPSEEK_V4.iter().map(|l| l.to_string()).collect())
    }
    fn reasoning_effort(&self) -> Option<String> {
        Some("none".into())
    }
    fn chat(&self, messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        Ok(self.play(messages, None))
    }
    fn chat_streaming_with_effort(
        &self,
        messages: &[ChatMsg],
        _tools: &[ToolDef],
        effort: Option<&str>,
        _cancel: &AtomicBool,
        _on_delta: &mut dyn FnMut(crate::agent::club::StreamDelta),
    ) -> Result<ClubReply, String> {
        Ok(self.play(messages, effort))
    }
}

impl Script {
    fn play(&self, messages: &[ChatMsg], effort: Option<&str>) -> ClubReply {
        let last = messages
            .last()
            .filter(|m| m.role == ChatRole::Harness)
            .map(|m| m.content.to_string())
            .unwrap_or_default();
        let mut asked = self.asked.lock().unwrap();
        asked.push((effort.map(str::to_string), last));
        match self.steps.get(asked.len() - 1) {
            Some(calls) => ClubReply::Calls(calls.clone()),
            None => ClubReply::Text("done".into()),
        }
    }
}

struct Fixed {
    name: &'static str,
    result: Result<&'static str, &'static str>,
}

impl Tool for Fixed {
    fn name(&self) -> &str {
        self.name
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: self.name.into(),
            description: "fixture".into(),
            params: serde_json::json!({"type": "object"}),
        }
    }
    fn call(&self, _: &Value) -> Result<String, String> {
        self.result.map(str::to_string).map_err(str::to_string)
    }
}

fn scripted_turn(club: &Script) -> Vec<ChatMsg> {
    let mut tools = ToolRegistry::new();
    tools.register(Box::new(Fixed {
        name: "probe",
        result: Ok("same as before"),
    }));
    tools.register(Box::new(Fixed {
        name: "run_tests",
        result: Err("tests: 1 failed: bowling::strike_bonus"),
    }));
    let (tx, _rx) = mpsc::channel();
    let mut history = vec![ChatMsg::user("make the bowling tests pass")];
    run_turn(
        club,
        &tools,
        &mut history,
        &AtomicBool::new(false),
        Some(12),
        &tx,
    )
    .unwrap();
    history
}

fn step(name: &str, n: usize) -> Vec<ToolCall> {
    vec![ToolCall {
        id: format!("call-{n}"),
        name: name.into(),
        args: serde_json::json!({"filter": "strike"}),
    }]
}

#[test]
fn a_silent_repeat_in_a_real_turn_gets_one_note_and_one_thinking_hop() {
    let _guard = crate::tests::env_lock();
    let club = Script::new((0..5).map(|n| step("probe", n)).collect());
    let history = scripted_turn(&club);
    let notes: Vec<_> = history
        .iter()
        .filter(|m| m.role == ChatRole::Harness && m.content.contains("times in a row"))
        .collect();
    assert_eq!(notes.len(), 1, "{history:#?}");
    assert!(notes[0].content.contains("`probe "), "{}", notes[0].content);
    // The note lands after the third identical call and the fourth request is
    // asked under it, with thinking on; the fifth is back to native.
    let asked = club.asked.lock().unwrap().clone();
    assert!(asked[3].1.contains("times in a row"), "{asked:#?}");
    let efforts = club.efforts();
    assert_eq!(
        efforts[..6],
        [None, None, None, Some("high".into()), None, None],
        "{efforts:?}"
    );
    assert_eq!(efforts.iter().flatten().count(), 1, "{efforts:?}");
}

#[test]
fn a_red_test_in_a_real_turn_lifts_thinking_for_the_next_request_only() {
    let _guard = crate::tests::env_lock();
    let club = Script::new(vec![step("probe", 0), step("run_tests", 1), step("probe", 2)]);
    scripted_turn(&club);
    let efforts = club.efforts();
    assert_eq!(efforts[..4], [None, None, Some("high".into()), None], "{efforts:?}");
    assert_eq!(efforts.iter().flatten().count(), 1, "{efforts:?}");
}
