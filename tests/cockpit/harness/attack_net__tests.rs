use super::*;

const REPLY: &str = r#"Sure, here is the plan:
```json
{"leaders":[
  {"role":"Research Lead","mandate":"Find the facts {not braces}","subtasks":[
    {"prompt":"Survey prior art","tools":"research"},
    {"prompt":"List constraints","tools":"bogus"}]},
  {"role":"Build Lead","mandate":"Ship code","subtasks":[{"prompt":"Write the module","tools":"code"}]}
]}
```
Done."#;

fn plan() -> AttackPlan {
    parse_plan("Make it fast", REPLY, "anthropic", "anthropic-fast").expect("plan parses")
}

#[test]
fn parses_fenced_reply_into_tiered_tree() {
    let p = plan();
    assert_eq!(p.leaders.len(), 2);
    assert_eq!(p.leaders[0].id, "L1");
    assert_eq!(p.leaders[0].club, "anthropic");
    assert_eq!(p.leaders[0].subtasks[0].id, "L1.1");
    assert_eq!(p.leaders[0].subtasks[0].club, "anthropic-fast");
    assert_eq!(p.leaders[0].subtasks[0].tools, "research");
    assert_eq!(
        p.leaders[0].subtasks[1].tools, "none",
        "unknown grant falls back"
    );
    assert_eq!(p.node_count(), 1 + 2 + 3);
}

#[test]
fn revision_stales_subtree_and_blocks_launch_until_regenerated_and_approved() {
    let mut p = plan();
    p.approve().unwrap();
    assert!(p.launch_blocker().is_none());

    assert!(
        !p.revise_leader(0, "Research Lead", None).unwrap(),
        "no-op edit"
    );
    assert!(p.launch_blocker().is_none());

    assert!(
        p.revise_leader(0, "Security Lead", Some("Threat model it"))
            .unwrap()
    );
    assert_eq!(p.stale_leaders(), vec![0]);
    assert!(p.launch_blocker().unwrap().contains("L1"));
    assert!(p.approve().is_err());
    assert!(p.to_graph_spec().is_err());

    let drafts = parse_subtasks(r#"{"subtasks":[{"prompt":"Enumerate attack surface"}]}"#).unwrap();
    p.apply_subtasks(0, drafts, "anthropic-fast").unwrap();
    assert_eq!(p.leaders[0].state, LeaderState::Regenerated);
    assert_eq!(p.leaders[0].subtasks.len(), 1);
    assert!(p.launch_blocker().unwrap().contains("approve"));
    p.approve().unwrap();
    assert!(p.launch_blocker().is_none());
}

#[test]
fn regenerate_prompt_names_revised_role_and_siblings() {
    let mut p = plan();
    p.revise_leader(1, "Perf Lead", Some("Make it fast"))
        .unwrap();
    let prompt = regenerate_prompt(&p, 1).unwrap();
    assert!(prompt.contains("Revised leader: Perf Lead"));
    assert!(
        prompt.contains("Research Lead"),
        "siblings listed to avoid overlap"
    );
    assert!(regenerate_prompt(&p, 9).is_none());
}

#[test]
fn compiles_bottom_up_graph_with_per_node_clubs() {
    let mut p = plan();
    p.set_club(NodeRef::Subtask(1, 0), "glm-5.3-flash").unwrap();
    p.approve().unwrap();
    let spec = p.to_graph_spec().unwrap();
    let ids: Vec<&str> = spec.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, ["L1.1", "L1.2", "L1", "L2.1", "L2", "objective"]);
    let l1 = &spec.nodes[2];
    assert_eq!(l1.depends_on, ["L1.1", "L1.2"]);
    assert_eq!(l1.club.as_deref(), Some("anthropic"));
    assert_eq!(spec.nodes[3].club.as_deref(), Some("glm-5.3-flash"));
    assert_eq!(spec.nodes[5].depends_on, ["L1", "L2"]);
    assert!(spec.nodes[0].prompt.contains("{task}"));
    assert!(spec.nodes[0].prompt.contains("Survey prior art"));
}

#[test]
fn club_swap_withdraws_approval() {
    let mut p = plan();
    p.approve().unwrap();
    p.set_club(NodeRef::Leader(0), "anthropic-fast").unwrap();
    assert!(p.launch_blocker().unwrap().contains("approve"));
    assert!(p.set_club(NodeRef::Leader(7), "x").is_err());
}

#[test]
fn rejects_replies_without_usable_json() {
    assert!(parse_plan("o", "no json here", "a", "b").is_err());
    assert!(parse_plan("o", r#"{"leaders":[]}"#, "a", "b").is_err());
    assert!(parse_subtasks("{ broken").is_err());
}

#[test]
fn caps_leader_and_subtask_counts() {
    let subs: Vec<String> = (0..20).map(|i| format!(r#"{{"prompt":"s{i}"}}"#)).collect();
    let leaders: Vec<String> = (0..20)
        .map(|i| format!(r#"{{"role":"R{i}","subtasks":[{}]}}"#, subs.join(",")))
        .collect();
    let reply = format!(r#"{{"leaders":[{}]}}"#, leaders.join(","));
    let p = parse_plan("o", &reply, "a", "b").unwrap();
    assert_eq!(p.leaders.len(), MAX_LEADERS);
    assert!(
        p.leaders
            .iter()
            .all(|l| l.subtasks.len() == MAX_SUBTASKS_PER_LEADER)
    );
}

#[test]
fn layout_centers_parents_over_children() {
    let p = plan();
    let (nodes, edges) = layout(&p);
    assert_eq!(nodes.len(), p.node_count());
    assert_eq!(edges.len(), p.node_count() - 1);
    let x = |r: NodeRef| nodes.iter().find(|n| n.node == r).unwrap().x;
    assert_eq!(x(NodeRef::Subtask(0, 0)), 0.0);
    assert_eq!(x(NodeRef::Subtask(1, 0)), 1.0);
    assert!((x(NodeRef::Leader(0)) - 0.25).abs() < 1e-9);
    assert!((x(NodeRef::Leader(1)) - 1.0).abs() < 1e-9);
    assert!((x(NodeRef::Root) - 0.625).abs() < 1e-9);
}

#[test]
fn spline_hits_endpoints_and_leaves_vertically() {
    let pts = spline((0.0, 0.0), (1.0, 1.0), 9);
    assert_eq!(pts.len(), 9);
    assert_eq!(pts[0], (0.0, 0.0));
    let last = pts[8];
    assert!((last.0 - 1.0).abs() < 1e-12 && (last.1 - 1.0).abs() < 1e-12);
    assert!(
        pts[1].0 < pts[1].1,
        "near the parent the curve is mostly vertical"
    );
    assert!((pts[4].0 - 0.5).abs() < 1e-12, "symmetric midpoint");
}
