//! The competition-mode leg: engagement and shell-inventory verdicts checked
//! against their native bodies, plus the fallback and the cost of a decision.
//!
//! `cockpit/live/competition-mode` decides. `turn/competition.rs` keeps the
//! native bodies as the fallback and the parity oracle.

use super::*;
use crate::agent::harness::live_policy::{LegSource, shipped_leg};
use crate::agent::harness::turn::{
    competition_mode_active_native, competition_mode_trigger, git_show_decided,
    git_show_is_inventory_thrash_native, goal_phrases, launch_phrases, ls_cmd_is_recursive_listing,
    ls_decided, shell_cmd_is_inventory_thrash_native, shell_decided, trigger_decided,
};
use crate::drive::goal::GOAL_BLOCK_HEADER;

const LEG: &str = "competition-mode";

fn goal_message(objective: &str) -> ChatMsg {
    ChatMsg::harness(format!(
        "{GOAL_BLOCK_HEADER}\n⠗⠃⠃ \"{objective}\"\n{GOAL_BLOCK_HEADER}"
    ))
}

/// Every engagement shape the trigger has to answer: each launch phrase, each
/// competitive goal word, and the near misses that must stay disarmed.
fn engagement_table() -> Vec<Vec<ChatMsg>> {
    let mut table: Vec<Vec<ChatMsg>> = Vec::new();
    for phrase in launch_phrases() {
        table.push(vec![ChatMsg::user(format!("please {phrase} now"))]);
    }
    for word in goal_phrases() {
        table.push(vec![
            ChatMsg::user("how goes?"),
            goal_message(&format!("keep the {word} moving")),
        ]);
    }
    table.push(Vec::new());
    table.push(vec![ChatMsg::user("")]);
    table.push(vec![ChatMsg::user("   ")]);
    // The launch phrase in an assistant message is not the operator arming it.
    table.push(vec![
        ChatMsg::user("keep going"),
        ChatMsg::assistant("competition mode engaged"),
    ]);
    // Anti-sticky: the previous turn's phrase does not survive a new steer, but a
    // competitive standing goal does.
    table.push(vec![
        ChatMsg::user("run the competition-loop"),
        goal_message("place a winning submission on the board"),
        ChatMsg::assistant("working"),
        ChatMsg::user("you are wasting time on the track"),
    ]);
    table.push(vec![
        ChatMsg::user("run the competition-loop"),
        goal_message("refactor the parser for clarity"),
        ChatMsg::assistant("working"),
        ChatMsg::user("hey pro, how goes?"),
    ]);
    table.push(vec![
        ChatMsg::user("run the competition-loop"),
        ChatMsg::assistant("done"),
        ChatMsg::user("hey pro, how goes?"),
    ]);
    // Uppercase and path-like near misses: only the launch vocabulary counts.
    table.push(vec![ChatMsg::user("FIRE NOW")]);
    table.push(vec![ChatMsg::user("run PodRace please")]);
    table.push(vec![ChatMsg::user(
        "look at src/competition_loop/parser.rs",
    )]);
    table.push(vec![ChatMsg::user("the leaderboard page looks nice")]);
    // A goal block with no competitive word is not intent to compete.
    table.push(vec![
        ChatMsg::user("carry on"),
        goal_message("ship the parser refactor"),
    ]);
    table.push(vec![
        goal_message("hold the leaderboard lead"),
        ChatMsg::assistant("steady"),
    ]);
    table
}

/// The shell commands the inventory scan has to answer: recursive listings, the
/// legal and illegal `git show` forms, the inventory command table, and cheap
/// board reads that must stay legal.
fn shell_table() -> Vec<&'static str> {
    vec![
        "",
        "ls",
        "ls -la",
        "ls -laR",
        "ls -lR",
        "ls -R",
        "ls -Ra",
        "ls -1r",
        "ls --recursive",
        "ls --recurse=2",
        "ls --recursive=always",
        "ls --color",
        "/bin/ls -1r",
        "-laR ls",
        "FOO=1 ls -laR",
        "ls board.md handoff.md",
        "cat board.md",
        "head -n 20 board.md",
        "cat board.md | head",
        "printf done",
        "rg needle",
        "FOO=1 rg needle",
        "ripgrep needle",
        "grep -r needle .",
        "egrep needle",
        "fgrep needle",
        "ugrep needle",
        "ag needle",
        "ack needle",
        "fd needle",
        "tree src",
        "locate board.md",
        "find . -name x",
        "find\tsrc",
        "git log --oneline",
        "git log",
        "git status",
        "git diff",
        "git ls-files",
        "git ls-tree HEAD",
        "git blame board.md",
        "git shortlog -sn",
        "git rev-list HEAD",
        "git reflog",
        "git whatchanged",
        "git grep submission",
        "git show HEAD:board.md",
        "git show HEAD:board.md | head",
        "git show --stat HEAD",
        "git show --name-only HEAD",
        "git show HEAD -- board.md",
        "git show HEAD",
        "git show origin/main:board.md",
        "cd /tmp && ls -laR",
        "sudo find / -name x",
    ]
}

/// The shipped leg and the native scans agree on every engagement shape, and the
/// leg is the one that answered.
#[test]
fn live_competition_mode_leg_matches_native_engagement() {
    let _guard = crate::tests::env_lock();
    let _env = EnvGuard::unset("ANGEL_COMPETITION_MODE");
    let leg = shipped_leg(LEG);
    let mut checked = 0;
    let (mut engaged, mut disarmed) = (0, 0);
    for history in engagement_table() {
        let native = competition_mode_trigger(&history);
        assert_eq!(
            competition_mode_active_native(&history),
            native.is_some(),
            "{history:?}"
        );
        let (decided, source) = trigger_decided(Some(leg.clone()), &history);
        assert_eq!(decided, native, "{history:?}");
        assert!(
            matches!(source, LegSource::Live { .. }),
            "{} kept the native trigger for {history:?}",
            source.as_str()
        );
        if native.is_some() {
            engaged += 1;
        } else {
            disarmed += 1;
        }
        checked += 1;
    }
    assert!(checked >= 30, "only {checked} turns were compared");
    assert!(
        engaged > 0 && disarmed > 0,
        "the table missed a branch: {engaged} engaged / {disarmed} disarmed"
    );
    eprintln!("competition-mode leg engaged {engaged} of {checked} turns, refused {disarmed}");
}

/// Every shell verdict — recursive listing, `git show` form and whole-command
/// thrash — is the same decision on both legs.
#[test]
fn live_competition_mode_leg_matches_native_shell_inventory() {
    let _guard = crate::tests::env_lock();
    let leg = shipped_leg(LEG);
    let mut checked = 0;
    let (mut thrash, mut legal) = (0, 0);
    for cmd in shell_table() {
        let native = shell_cmd_is_inventory_thrash_native(cmd);
        if native {
            thrash += 1;
        } else {
            legal += 1;
        }
        let (decided, source) = shell_decided(Some(leg.clone()), cmd);
        assert_eq!(decided, native, "shell thrash for {cmd:?}");
        assert!(
            matches!(source, LegSource::Live { .. }),
            "{} kept the native shell verdict for {cmd:?}",
            source.as_str()
        );
        let ls_native = ls_cmd_is_recursive_listing(cmd);
        let (ls_live, ls_source) = ls_decided(Some(leg.clone()), cmd);
        assert_eq!(ls_live, ls_native, "recursive listing for {cmd:?}");
        assert!(
            matches!(ls_source, LegSource::Live { .. }),
            "{} kept the native listing verdict for {cmd:?}",
            ls_source.as_str()
        );
        let show_native = git_show_is_inventory_thrash_native(cmd);
        let (show_live, show_source) = git_show_decided(Some(leg.clone()), cmd);
        assert_eq!(show_live, show_native, "git show verdict for {cmd:?}");
        assert!(
            matches!(show_source, LegSource::Live { .. }),
            "{} kept the native show verdict for {cmd:?}",
            show_source.as_str()
        );
        checked += 1;
    }
    assert!(checked >= 30, "only {checked} commands were compared");
    assert!(
        thrash > 0 && legal > 0,
        "the table missed a branch: {thrash} thrash / {legal} legal"
    );
    eprintln!("competition-mode leg classified {checked} commands: {thrash} thrash, {legal} legal");
}

/// A throwaway policy module, so a deliberately out-of-contract leg can be
/// pinned without touching the shipped one.
fn temporary_policy_leg(id: &str, render: &str) -> angel_live::Program {
    let root = std::env::temp_dir().join(format!(
        "angel-competition-mode-leg-{}-{id}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let module = root.join(id);
    std::fs::create_dir_all(&module).unwrap();
    std::fs::write(
        module.join("module.json"),
        format!(
            r#"{{"api_version":1,"id":"{id}","kind":"policy","entry":"index.js","probe":{{}}}}"#
        ),
    )
    .unwrap();
    std::fs::write(module.join("index.js"), render).unwrap();
    let mut registry =
        angel_live::Registry::validated(root, crate::agent::hotload::evaluate, |_, _| Ok(()));
    registry.refresh(true);
    registry
        .snapshot()
        .policies
        .get(id)
        .cloned()
        .unwrap_or_else(|| panic!("fixture leg {id} did not publish"))
}

/// A leg that reports a phrase the turn never matched, and a leg whose verdict
/// does not decode, are both refused: the native body answers and the caller can
/// still tell where the answer came from.
#[test]
fn live_competition_mode_leg_falls_back_when_a_leg_breaks_contract() {
    let _guard = crate::tests::env_lock();
    let _env = EnvGuard::unset("ANGEL_COMPETITION_MODE");

    let wild = temporary_policy_leg(
        "competition-mode-wild",
        "module.exports={apiVersion:1,render: () => ({trigger:'phrase',index:99})}",
    );
    let history = vec![ChatMsg::user("hello there")];
    let (decided, source) = trigger_decided(Some(wild), &history);
    assert_eq!(decided, competition_mode_trigger(&history));
    assert_eq!(decided, None);
    assert!(
        matches!(source, LegSource::LiveFallback { .. }),
        "{} answered an impossible phrase index",
        source.as_str()
    );

    let broken = temporary_policy_leg(
        "competition-mode-broken",
        "module.exports={apiVersion:1,render: () => ({hit:'maybe'})}",
    );
    let (decided, source) = shell_decided(Some(broken), "git log --oneline");
    assert!(decided, "the native scan still calls git log thrash");
    assert!(
        matches!(source, LegSource::LiveFallback { .. }),
        "{} answered a non-boolean verdict",
        source.as_str()
    );
}

/// One decision costs microseconds, so routing through the leg cannot move a
/// turn's budget.
#[test]
fn competition_mode_leg_decisions_stay_microseconds() {
    let _guard = crate::tests::env_lock();
    let _env = EnvGuard::unset("ANGEL_COMPETITION_MODE");
    let leg = shipped_leg(LEG);
    let history = vec![
        ChatMsg::user("run the competition-loop"),
        goal_message("place a winning submission on the board"),
        ChatMsg::assistant("working"),
        ChatMsg::user("you are wasting time on the track"),
    ];
    let commands = [
        "git show HEAD:board.md",
        "git log --oneline",
        "ls -laR",
        "cat board.md",
    ];
    let median = |mut samples: Vec<u128>| {
        samples.sort_unstable();
        samples[samples.len() / 2]
    };
    let mut trigger_samples = Vec::with_capacity(200);
    for _ in 0..200 {
        let started = std::time::Instant::now();
        let _ = trigger_decided(Some(leg.clone()), &history);
        trigger_samples.push(started.elapsed().as_micros());
    }
    let mut shell_samples = Vec::with_capacity(200);
    for index in 0..200 {
        let started = std::time::Instant::now();
        let _ = shell_decided(Some(leg.clone()), commands[index % commands.len()]);
        shell_samples.push(started.elapsed().as_micros());
    }
    let trigger_median = median(trigger_samples);
    let shell_median = median(shell_samples);
    eprintln!(
        "competition-mode leg: median {trigger_median} µs per engagement decision, \
         {shell_median} µs per shell decision (200 calls each)"
    );
    assert!(
        trigger_median < 5_000 && shell_median < 5_000,
        "median {trigger_median}/{shell_median} µs per decision is not a turn-budget cost"
    );
}

/// With no live generation offered, the shipped module still answers the shell
/// and `ls` questions and the receipt says where the answer came from.
#[test]
fn a_turn_with_no_live_generation_uses_the_shipped_competition_leg() {
    let (hit, source) = shell_decided(None, "git log --oneline");
    assert!(hit, "the shipped scan still calls git log thrash");
    assert!(
        matches!(source, LegSource::Shipped { .. }),
        "{source:?} did not name the shipped leg"
    );
    // Callers hand the scan a lowercased command, as the shell scan does.
    let (hit, source) = ls_decided(None, &"ls -laR".to_ascii_lowercase());
    assert!(
        hit,
        "the shipped scan still calls a recursive listing thrash"
    );
    assert!(
        matches!(source, LegSource::Shipped { .. }),
        "{source:?} did not name the shipped leg"
    );
}
