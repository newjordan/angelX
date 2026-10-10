use super::*;
use crate::drive::labyrinth::{Map, Passage, context, initialize, load};
use std::path::PathBuf;

struct Workspace(PathBuf);
impl Workspace {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "angel-failed-routes-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn attempt(verdict: &str, index: usize) -> Value {
    let digest = |file: &str| json!({"path":format!("labyrinth/angel/campaigns/research/agents/{file}"),"sha256":"a".repeat(64)});
    json!({"id":format!("round-{index}"),"claim_id":format!("claim-{index}"),"round":index,
        "verdict":verdict,"route":format!("universal positive-case route {index}"),
        "lesson":"The zero case defeats this perspective; the question remains unanswered",
        "perspective":"inspect tiny cases","next_test":"inspect the changed domain",
        "author_report":digest("attack/report.md"),"referee_report":digest("referee/report.md"),
        "check":digest("referee/checks/referee-0.json"),"counterexample":digest("referee/artifacts/zero.json")})
}

fn question(attempts: Vec<Value>) -> Value {
    json!({"id":"q.open","kind":"question","status":"open","title":"An unanswered question",
        "statement":"Does the conjecture hold in the remaining domain?","links":[{"to":"m.definition","rel":"uses"}],
        "failed_routes":attempts})
}

fn document(question: Value) -> Value {
    json!({"schema":1,"nodes":[question,{"id":"m.definition","kind":"method","links":[]}]})
}

fn research_door(map: &Map) -> &Value {
    &map.nodes[map.index["q.open"]]
}

#[test]
fn checked_negative_routes_reach_fresh_plan_context_without_closing_the_question() {
    let workspace = Workspace::new();
    initialize(&workspace.0).unwrap();
    let negative = attempt("FALSE", 1);
    let original = document(question(vec![negative.clone()]));
    let bytes = serde_json::to_vec(&original).unwrap();
    std::fs::write(workspace.0.join("labyrinth/knowledge.json"), &bytes).unwrap();
    let map = load(&workspace.0).unwrap().unwrap();
    assert!(has_negative_attempt(research_door(&map)));
    assert_eq!(map.node_passage(map.index["q.open"]), Passage::Door);
    let frontier = map.frontier(Some("research"), 3);
    assert_eq!(frontier[0]["id"], "q.open");
    assert_eq!(frontier[0]["failed_routes"][0]["check"], negative["check"]);
    let plan = map.plan(Some("research"), 3);
    assert_eq!(plan["closed_routes"][0]["door_id"], "q.open");
    assert_eq!(plan["closed_routes"][0]["route"], negative["route"]);
    assert!(plan["authority"].is_null() && plan["reward"].is_null());
    let path = map.route("m.definition", "q.open", true).unwrap();
    assert_eq!(path["path"][1]["passage"], "door");
    assert!(map.route("m.definition", "q.open", false).unwrap()["path"].is_null());
    let cue = context(&workspace.0, "research");
    assert!(cue.contains("universal positive-case route 1"));
    assert!(cue.contains("referee-0.json"));
    assert!(cue.contains(&crate::agent::harness::book::labyrinth_campaign::MAP.cells()));
    assert!(
        !cue.contains("avoid repeating failed perspectives"),
        "policy must remain encoded in Legend"
    );
    assert_eq!(
        std::fs::read(workspace.0.join("labyrinth/knowledge.json")).unwrap(),
        bytes
    );
    assert_eq!(
        research_door(&load(&workspace.0).unwrap().unwrap())["failed_routes"],
        original["nodes"][0]["failed_routes"]
    );
}

#[test]
fn compact_negative_history_keeps_the_latest_attempts_and_full_source_history() {
    let workspace = Workspace::new();
    initialize(&workspace.0).unwrap();
    let mut attempts: Vec<_> = (0..128).map(|index| attempt("GAP", index)).collect();
    for attempt in &mut attempts {
        attempt["route"] = json!(format!(
            "{} {}",
            attempt["claim_id"].as_str().unwrap(),
            "long route ".repeat(100)
        ));
        attempt["lesson"] = json!("detailed negative reasoning ".repeat(100));
    }
    let original = document(question(attempts));
    std::fs::write(
        workspace.0.join("labyrinth/knowledge.json"),
        serde_json::to_vec(&original).unwrap(),
    )
    .unwrap();
    let map = load(&workspace.0).unwrap().unwrap();
    assert_eq!(
        map.frontier(None, 3)[0]["failed_routes"]
            .as_array()
            .unwrap()
            .len(),
        8
    );
    let cue = context(&workspace.0, "research");
    assert!(
        cue.len() < 6000,
        "fresh prompts must not expand into full campaign history"
    );
    assert!(cue.contains("claim-127") && cue.contains("claim-126"));
    assert!(!cue.contains("claim-120"));
    let loaded_history = &research_door(&map)["failed_routes"];
    let source_history = &original["nodes"][0]["failed_routes"];
    assert_eq!(loaded_history.as_array().unwrap().len(), 128);
    assert_eq!(
        crate::knowledge::cut::sha256_hex(&serde_json::to_vec(loaded_history).unwrap()),
        crate::knowledge::cut::sha256_hex(&serde_json::to_vec(source_history).unwrap()),
        "the full stored history must survive compact projection unchanged"
    );
}

#[test]
fn structured_negative_check_provenance_retains_actual_checker_and_output_identities() {
    let mut negative = attempt("GAP", 2);
    let fields = negative.as_object_mut().unwrap();
    fields.remove("lesson");
    fields.remove("check");
    negative["reason"] = json!("The independent implementation found an unsupported assumption");
    negative["perspective"] = Value::Null;
    negative["counterexample"] = Value::Null;
    negative["referee_check"] = json!({"id":"referee-0",
        "code_path":"labyrinth/angel/campaigns/research/agents/referee/artifacts/check.py",
        "code_sha256":"b".repeat(64),
        "output_path":"labyrinth/angel/campaigns/research/agents/referee/checks/referee-0.out",
        "output_sha256":"c".repeat(64),"source_sha256":"d".repeat(64),"command_sha256":"e".repeat(64)});
    let map = Map::parse(document(question(vec![negative.clone()]))).unwrap();
    assert!(has_negative_attempt(research_door(&map)));
    assert_eq!(map.node_passage(map.index["q.open"]), Passage::Door);
    let frontier = map.frontier(None, 1);
    assert_eq!(
        frontier[0]["failed_routes"][0]["referee_check"],
        negative["referee_check"]
    );
    assert_eq!(
        frontier[0]["failed_routes"][0]["lesson"],
        negative["reason"]
    );
    negative["check"] = json!({"path":"labyrinth/angel/campaigns/research/agents/referee/checks/referee-0.out",
        "sha256":"c".repeat(64)});
    let map = Map::parse(document(question(vec![negative.clone()]))).unwrap();
    let frontier = map.frontier(None, 1);
    assert_eq!(frontier[0]["failed_routes"][0]["check"], negative["check"]);
    assert_eq!(
        frontier[0]["failed_routes"][0]["referee_check"], negative["referee_check"],
        "the output digest must not hide the actual executed checker identity"
    );
    negative["check"] = Value::Null;
    negative["referee_check"]["code_path"] = json!("labyrinth/./check.py");
    assert!(Map::parse(document(question(vec![negative]))).is_err());
}

#[test]
fn malformed_negative_provenance_fails_closed_without_promoting_or_refuting_a_node() {
    for change in 0..4 {
        let mut negative = attempt("GAP", 0);
        match change {
            0 => negative["verdict"] = json!("ESTABLISHED"),
            1 => negative["referee_report"]["sha256"] = json!("missing"),
            2 => negative["check"]["path"] = json!("../outside.json"),
            _ => negative["route"] = json!(""),
        }
        let node = question(vec![negative]);
        assert!(!has_negative_attempt(&node));
        assert!(
            Map::parse(document(node))
                .err()
                .unwrap()
                .contains("failed routes")
        );
    }
    assert!(
        Map::parse(document(question(
            (0..129).map(|index| attempt("GAP", index)).collect()
        )))
        .is_err()
    );
    let map = Map::parse(document(question(Vec::new()))).unwrap();
    assert!(!has_negative_attempt(research_door(&map)));
    assert_eq!(map.node_passage(map.index["q.open"]), Passage::Door);
}
