use super::*;

#[test]
fn worker_proposal_approval_skill_loading_and_feedback_form_one_loop() {
    use std::io::Write;
    let _lock = crate::tests::env_lock();
    let fixture = crate::tests::TestGitWorkspace::new("habit-worker-roundtrip");
    let workspace = fixture.path();
    let identity = crate::workspace_store::repo_identity(workspace);
    let proposed = workspace.join("proposed");
    let live = workspace.join("live");
    let state = workspace.join("habit-state");
    let graph = workspace.join("graph.json");
    let ledger = workspace.join("ledger.jsonl");
    let mut rows = Vec::new();
    for session in 1..=12 {
        for (seq, command) in ["cargo build", "cargo test"].iter().enumerate() {
            rows.push(serde_json::json!({
                    "kind":"event", "event":"cmd", "v":3, "ts":now_secs(), "session":session, "seq":seq,
                    "repo":{"key":identity.key,"root":identity.root,"slug":"worker-fixture"},
                    "cmd":{"text":command,"exit":0,"timed_out":false,"verdict":"pass",
                        "pipefail":true,"independent":true,"source":"agent"}
                }).to_string());
        }
    }
    std::fs::write(&ledger, rows.join("\n") + "\n").unwrap();
    let tick = || {
        let result = std::process::Command::new("node")
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/habitsmith-tick.mjs"))
            .arg("--force")
            .env("ANGEL_CAUSAL_GRAPH", &graph)
            .env("ANGEL_EXPERIENCE_LOG", &ledger)
            .env("ANGEL_HABITS_DIR", &state)
            .env("ANGEL_HABIT_PROPOSED_DIR", &proposed)
            .env("ANGEL_SKILLS_DIR", &live)
            .env(
                "ANGEL_BUNDLED_SKILLS_DIR",
                workspace.join("no-bundled-skills"),
            )
            .env("ANGEL_HABITS", "1")
            .env_remove("ANGEL_HABIT_MIN_BELIEF")
            .env_remove("ANGEL_HABIT_MAX_PROPOSALS_PER_DAY")
            .env_remove("ANGEL_GRAPH_LEASE_TOKEN")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    };
    tick();
    let drafts = list_proposals_for_in(&proposed, workspace);
    assert_eq!(drafts.len(), 1);
    let name = drafts[0].name.clone();
    let fact = drafts[0].fact.clone().unwrap();
    assert!(status_text_in(&proposed, &state.join("status.json"), workspace).contains(&name));
    let approved = approve_in(&proposed, &live, &state, &name, workspace);
    assert!(approved.contains("approved"), "{approved}");
    let skills = crate::harness::load_skills_from(&live);
    assert!(skills.iter().any(|skill| skill.name == name));
    let mut events = std::fs::OpenOptions::new()
        .append(true)
        .open(&ledger)
        .unwrap();
    for key in [&identity.key, &"different-workspace".to_string()] {
        writeln!(
            events,
            "{}",
            serde_json::json!({
                "kind":"event", "event":"skill", "v":3, "ts":now_secs(), "session":20,
                "repo":{"key":key,"root":identity.root}, "skill":{"name":name,"ok":true}
            })
        )
        .unwrap();
    }
    drop(events);
    tick();
    let status: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(state.join("status.json")).unwrap()).unwrap();
    let learned = status["facts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == fact)
        .unwrap();
    assert_eq!(
        learned["skill"]["uses"], 1,
        "usage must stay bound to this repository"
    );
    assert_eq!(learned["skill"]["name"], name);
    assert!(
        learned["belief"].as_f64().unwrap() > 0.85,
        "the approval must fold back into belief"
    );
    assert!(
        std::fs::read_to_string(state.join("verdicts.jsonl"))
            .unwrap()
            .is_empty()
    );
    assert!(
        list_proposals_for_in(&proposed, workspace).is_empty(),
        "installed skills must not be re-proposed"
    );
    assert!(status_text_in(&proposed, &state.join("status.json"), workspace).contains("1 use(s)"));
}

fn draft(workspace: &Path) -> String {
    let identity = crate::workspace_store::repo_identity(workspace);
    format!(
        "---\nname: proj-build-test\ndescription: Run the observed build → test workflow.\nfact: hyp_habit_k_build-test\nrisky: true\nscope: project\nrepo_key: \"{}\"\nrepo_root: \"{}\"\n---\n\n# proj-build-test\n\nSteps:\n1. `cargo build` (build, passes 100%)\n\n---\nEvidence: 6 run(s) across 6 session(s), 6 clean end-to-end · belief 0.78\n",
        identity.key,
        identity.root.display()
    )
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("angel-habits-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write_draft(base: &Path, folder: &str, text: &str) {
    let dir = base.join(folder);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("SKILL.md"), text).unwrap();
}

#[test]
fn lists_proposals_with_frontmatter_evidence_and_risk() {
    let base = scratch("list");
    write_draft(&base, "proj-build-test", &draft(&base));
    let ps = list_proposals_in(&base);
    assert_eq!(ps.len(), 1);
    assert_eq!(ps[0].name, "proj-build-test");
    assert_eq!(ps[0].fact.as_deref(), Some("hyp_habit_k_build-test"));
    assert!(ps[0].risky);
    assert!(ps[0].evidence.as_deref().unwrap().contains("6 run(s)"));

    let text = status_text_in(&base, &base.join("no-status.json"), &base);
    assert!(text.contains("proj-build-test"));
    assert!(text.contains("⚠ risky"));
    assert!(text.contains("approve <name>"));
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn empty_dir_says_how_drafts_appear() {
    let base = scratch("empty");
    let text = status_text_in(&base, &base.join("no-status.json"), &base);
    assert!(text.contains("no habitsmith proposals"));
    assert!(text.contains("habitsmith-tick.mjs"));
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn installed_section_renders_usage_and_drift_from_the_tick_artifact() {
    let base = scratch("drift");
    let identity = crate::workspace_store::repo_identity(&base);
    let status = serde_json::json!({
        "v": 1,
        "facts": [
            { "name": "proj-build-test-run", "repoKey": identity.key, "repoRoot": identity.root, "belief": 0.42, "drifting": true,
              "skill": { "uses": 7, "ok": 5, "lastUsed": 1_783_300_000u64 } },
            { "name": "proj-lint-test", "repoKey": identity.key, "repoRoot": identity.root, "belief": 0.81, "drifting": false,
              "skill": { "uses": 3, "ok": 3, "lastUsed": 1_783_300_000u64 } },
            { "name": "never-installed", "repoKey": identity.key, "repoRoot": identity.root, "belief": 0.77, "drifting": false, "skill": null },
        ],
    });
    std::fs::write(
        base.join("status.json"),
        serde_json::to_string(&status).unwrap(),
    )
    .unwrap();

    let text = status_text_in(
        &base.join("nothing-proposed"),
        &base.join("status.json"),
        &base,
    );
    assert!(
        text.contains("proj-build-test-run [belief 0.42] ⚠ drifting"),
        "{text}"
    );
    assert!(text.contains("proj-lint-test [belief 0.81] · 3 use(s)"));
    assert!(!text.contains("never-installed"), "no usage → not listed");
    // Garbage status.json never breaks the command.
    std::fs::write(base.join("status.json"), "{nope").unwrap();
    let ok = status_text_in(
        &base.join("nothing-proposed"),
        &base.join("status.json"),
        &base,
    );
    assert!(ok.contains("no habitsmith proposals"));
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn approve_moves_the_folder_and_spools_supports() {
    let base = scratch("approve");
    let (proposed, live, state) = (base.join("p"), base.join("l"), base.join("s"));
    write_draft(&proposed, "proj-build-test", &draft(&base));

    let msg = approve_in(&proposed, &live, &state, "proj-build-test", &base);
    assert!(msg.contains("approved"), "{msg}");
    assert!(
        live.join("proj-build-test/SKILL.md").exists(),
        "installed into live dir"
    );
    assert!(
        !proposed.join("proj-build-test").exists(),
        "draft gone from proposed"
    );
    let spool = std::fs::read_to_string(state.join("verdicts.jsonl")).unwrap();
    assert!(spool.contains("\"action\":\"approve\""));
    assert!(spool.contains("hyp_habit_k_build-test"));

    // Approving again: the draft no longer exists.
    let again = approve_in(&proposed, &live, &state, "proj-build-test", &base);
    assert!(again.contains("no proposal named"), "{again}");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn approve_refuses_to_clobber_an_existing_live_skill() {
    let base = scratch("clobber");
    let (proposed, live, state) = (base.join("p"), base.join("l"), base.join("s"));
    write_draft(&proposed, "proj-build-test", &draft(&base));
    write_draft(&live, "proj-build-test", "already here");

    let msg = approve_in(&proposed, &live, &state, "proj-build-test", &base);
    assert!(msg.contains("already exists"), "{msg}");
    assert!(proposed.join("proj-build-test").exists(), "draft untouched");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn reject_deletes_and_spools_contradicts() {
    let base = scratch("reject");
    let (proposed, state) = (base.join("p"), base.join("s"));
    write_draft(&proposed, "proj-build-test", &draft(&base));

    let msg = reject_in(&proposed, &state, "proj-build-test", &base);
    assert!(msg.contains("rejected"), "{msg}");
    assert!(!proposed.join("proj-build-test").exists());
    let spool = std::fs::read_to_string(state.join("verdicts.jsonl")).unwrap();
    assert!(spool.contains("\"action\":\"reject\""));

    let unknown = reject_in(&proposed, &state, "nope", &base);
    assert!(unknown.contains("no proposal named 'nope'"));
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn startup_notice_fires_once_per_new_batch() {
    let base = scratch("notice");
    let (proposed, stamp) = (base.join("p"), base.join("s/last-launch"));
    write_draft(&proposed, "proj-build-test", &draft(&base));

    // First launch after the draft appeared: notice (stamp was 0/missing).
    let first = startup_notice_in(&proposed, &stamp, now_secs(), &base);
    assert!(first.is_some_and(|n| n.contains("1 skill proposal(s)")));
    // Next launch, nothing new: silent (stamp advanced past the mtime).
    let second = startup_notice_in(&proposed, &stamp, now_secs() + 10, &base);
    assert!(second.is_none());
    // Empty dir: silent.
    let none = startup_notice_in(&base.join("nowhere"), &stamp, now_secs() + 20, &base);
    assert!(none.is_none());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn proposal_lifecycle_is_project_bound_and_legacy_drafts_are_inert() {
    let base = scratch("project-bound");
    let alpha = base.join("alpha");
    let beta = base.join("beta");
    let proposed = base.join("proposed");
    let live = base.join("live");
    let state = base.join("state");
    std::fs::create_dir_all(&alpha).unwrap();
    std::fs::create_dir_all(&beta).unwrap();
    write_draft(&proposed, "alpha-draft", &draft(&alpha));
    write_draft(&proposed, "beta-draft", &draft(&beta));
    write_draft(
        &proposed,
        "legacy-draft",
        "---\nname: legacy-habit\nfact: hyp_habit_legacy\n---\nlegacy",
    );

    assert_eq!(list_proposals_for_in(&proposed, &alpha).len(), 1);
    assert_eq!(list_proposals_for_in(&proposed, &beta).len(), 1);
    assert!(
        approve_in(&proposed, &live, &state, "legacy-habit", &alpha).contains("no proposal named")
    );

    let alpha_msg = approve_in(&proposed, &live, &state, "proj-build-test", &alpha);
    assert!(alpha_msg.contains("approved"), "{alpha_msg}");
    assert!(proposed.join("beta-draft").exists());
    assert!(!proposed.join("alpha-draft").exists());

    let beta_msg = approve_in(&proposed, &live, &state, "proj-build-test", &beta);
    assert!(beta_msg.contains("approved"), "{beta_msg}");
    assert!(live.join("alpha-draft/SKILL.md").exists());
    assert!(live.join("beta-draft/SKILL.md").exists());
    let _ = std::fs::remove_dir_all(&base);
}
