//! `/table`: the Round Table from a seat's chair. One vocabulary for
//! the arbiter and every knight — what a command does depends on
//! where this terminal sits (its own table as seat 0, or a guest
//! link to a friend's), never on which command was typed.
//!
//! Verbs: `host [name]` · `join <link> [name]` · `link` · `stop` · bare =
//! the board · `show <id>` · `new <title> | <body>` ·
//! `comment <id> <text>` · `vote <id> up|down` · `claim/release <id>` ·
//! `work <id>` (claim + a git worktree to work in) ·
//! `submit <id> <summary> | <evidence>` (the worktree's diff as the
//! bundle) · arbiter: `accept/reject/close <id>`,
//! `apply <id>` (bundle into a scratch worktree to inspect/test),
//! `merge <id> <commit>` (record the merge made by hand).

use super::*;
use angel_table::{ARBITER, Board, Issue, Op, State};
use std::process::Command as Proc;

const USAGE: &str = "Round Table: /table host [name] · join <link> [name] · link · stop · \
new <title> | <body> · show <id> · comment <id> <text> · vote <id> up|down · \
claim|release <id> · work <id> · submit <id> <summary> | <evidence> · \
accept|reject|close <id> · apply <id> · merge <id> <commit> · export [path]";

impl App {
    pub(crate) fn table_command(&mut self, arg: Option<&str>) -> String {
        let arg = arg.unwrap_or("").trim();
        let (verb, rest) = match arg.split_once(' ') {
            Some((v, r)) => (v, r.trim()),
            None => (arg, ""),
        };
        match verb {
            "" | "list" | "board" => self.table_board_text(),
            "host" => self.table_host(rest),
            "join" => self.table_join(rest),
            "link" | "links" => self.table_links(),
            "stop" | "leave" => self.table_stop(),
            "new" => {
                let (title, body) = split_bar(rest);
                self.table_cast(Op::NewIssue { title, body }, "proposed")
            }
            "show" => match parse_id(rest) {
                Some(id) => self.table_show(id),
                None => "usage: /table show <id>".into(),
            },
            "comment" => match split_id(rest) {
                Some((id, text)) => {
                    self.table_cast(Op::Comment { id, text: text.to_string() }, "spoken")
                }
                None => "usage: /table comment <id> <text>".into(),
            },
            "vote" => match split_id(rest) {
                Some((id, dir)) => self.table_cast(
                    Op::Vote { id, support: !matches!(dir, "down" | "no" | "-") },
                    "voted",
                ),
                None => "usage: /table vote <id> up|down".into(),
            },
            "claim" => self.table_id_op(rest, "claim", |id| Op::Claim { id }, "claimed"),
            "release" => self.table_id_op(rest, "release", |id| Op::Release { id }, "released"),
            "accept" => self.table_id_op(rest, "accept", |id| Op::Accept { id }, "accepted"),
            "reject" => self.table_id_op(rest, "reject", |id| Op::Reject { id }, "rejected"),
            "close" => self.table_id_op(rest, "close", |id| Op::Close { id }, "closed"),
            "merge" => match split_id(rest) {
                Some((id, commit)) => self
                    .table_cast(Op::Merge { id, commit: commit.to_string() }, "merge recorded"),
                None => "usage: /table merge <id> <commit>".into(),
            },
            "work" => match parse_id(rest) {
                Some(id) => self.table_work(id),
                None => "usage: /table work <id>".into(),
            },
            "submit" => match split_id(rest) {
                Some((id, tail)) => {
                    let (summary, evidence) = split_bar(tail);
                    self.table_submit(id, summary, evidence)
                }
                None => "usage: /table submit <id> <summary> | <evidence>".into(),
            },
            "apply" => match parse_id(rest) {
                Some(id) => self.table_apply(id),
                None => "usage: /table apply <id>".into(),
            },
            "export" => self.table_export(rest),
            _ => USAGE.into(),
        }
    }

    fn table_host(&mut self, name: &str) -> String {
        if self.table_host.is_some() {
            return "this table is already hosted here; /table stop first".into();
        }
        if self.table_guest.is_some() {
            return "you hold a guest seat elsewhere; /table stop first".into();
        }
        let name = if name.is_empty() { "Host" } else { name };
        match crate::drive::round_table::TableHost::start(self.tools.current_workspace(), name) {
            Ok((host, links)) => {
                self.table_host = Some(host);
                format_host_links("The Round Table is set. You are the arbiter (seat 0).", &links)
            }
            Err(e) => format!("the table did not rise: {e}"),
        }
    }

    fn table_links(&self) -> String {
        if let Some(host) = &self.table_host {
            let links = host.links();
            return format_host_links("You are hosting the Round Table (arbiter, seat 0).", &links);
        }
        if let Some(guest) = &self.table_guest {
            return format!(
                "Joined Round Table at {}\nstatus: {}",
                guest.target(),
                guest.status_line()
            );
        }
        "not at a table: /table host, or /table join <link>".into()
    }

    fn table_join(&mut self, rest: &str) -> String {
        if self.table_host.is_some() || self.table_guest.is_some() {
            return "already at a table; /table stop first".into();
        }
        let mut words = rest.splitn(2, ' ');
        let link = words.next().unwrap_or("").trim();
        if link.is_empty() {
            return "usage: /table join <link> [your name]".into();
        }
        let name = words.next().map(str::trim).filter(|n| !n.is_empty()).unwrap_or("Knight");
        match crate::drive::round_table::TableGuest::connect(link, name) {
            Ok(mut guest) => {
                guest.poll();
                let status = guest.status_line();
                self.table_guest = Some(guest);
                format!("taking a seat at the Round Table as {name}… ({status})\n/table shows the board once it lands.")
            }
            Err(e) => format!("could not reach that table: {e}"),
        }
    }

    fn table_stop(&mut self) -> String {
        if let Some(host) = self.table_host.take() {
            host.stop();
            return "the table is cleared; its log stays in .angel/round-table.jsonl".into();
        }
        if let Some(mut guest) = self.table_guest.take() {
            guest.bye();
            return "you rise from the table".into();
        }
        "you are not at a table".into()
    }

    /// The board this terminal sees: the arbiter's own, or the
    /// server's latest published tree at a guest seat.
    fn table_board(&mut self) -> Option<Board> {
        if let Some(host) = &self.table_host {
            return Some(host.shared.board());
        }
        self.table_guest.as_mut().and_then(|g| g.board())
    }

    fn table_seat(&self) -> Option<u32> {
        if self.table_host.is_some() {
            return Some(ARBITER);
        }
        self.table_guest.as_ref().and_then(|g| g.seat())
    }

    fn table_cast(&mut self, op: Op, done: &str) -> String {
        if let Some(host) = &self.table_host {
            return match host.shared.apply(ARBITER, op) {
                Ok(_) => format!("{done}."),
                Err(e) => format!("refused: {e}"),
            };
        }
        if let Some(guest) = &mut self.table_guest {
            guest.cast(&op);
            return format!("{done} (cast; the board confirms it on the next snapshot).");
        }
        "not at a table: /table host, or /table join <link>".into()
    }

    fn table_id_op(
        &mut self,
        rest: &str,
        verb: &str,
        make: impl Fn(u64) -> Op,
        done: &str,
    ) -> String {
        match parse_id(rest) {
            Some(id) => self.table_cast(make(id), done),
            None => format!("usage: /table {verb} <id>"),
        }
    }

    fn table_board_text(&mut self) -> String {
        let Some(board) = self.table_board() else {
            return if let Some(guest) = &self.table_guest {
                // Say *why* there is no board: a rejected or dropped
                // link must not read as "wait longer".
                format!("the board has not landed yet — link: {}", guest.status_line())
            } else if self.table_host.is_some() {
                "the board has not landed yet — a breath, then /table again".into()
            } else {
                format!("not at a table.\n{USAGE}")
            };
        };
        let me = self.table_seat();
        let mut out = String::from("THE ROUND TABLE\nseated: ");
        if board.seats.is_empty() {
            out.push_str("(no one yet)");
        } else {
            let names: Vec<String> = board
                .seats
                .iter()
                .map(|(seat, name)| {
                    let crown = if *seat == ARBITER { " (arbiter)" } else { "" };
                    let you = if Some(*seat) == me { " ← you" } else { "" };
                    format!("{name}{crown}{you}")
                })
                .collect();
            out.push_str(&names.join(" · "));
        }
        if board.issues.is_empty() {
            out.push_str("\nthe board is empty — /table new <title> | <body>");
            return out;
        }
        for issue in board.issues.values() {
            out.push_str(&format!("\n{}", issue_line(&board, issue)));
        }
        out
    }

    fn table_show(&mut self, id: u64) -> String {
        let Some(board) = self.table_board() else {
            return "not at a table".into();
        };
        let Some(issue) = board.issues.get(&id) else {
            return format!("no issue #{id}");
        };
        let name = |seat: u32| board.seats.get(&seat).cloned().unwrap_or_else(|| format!("seat {seat}"));
        let mut out = format!("#{id} [{}] {}\nby {}\n{}", issue.state.label(), issue.title, name(issue.author), issue.body);
        if let Some(claim) = &issue.claim {
            out.push_str(&format!("\nclaimed by {} (until tick {})", name(claim.seat), claim.until));
        }
        out.push_str(&format!("\nvotes: {:+} ({} for, {} against)", issue.score(),
            issue.votes.values().filter(|v| **v).count(),
            issue.votes.values().filter(|v| !**v).count()));
        if let Some(sub) = &issue.submission {
            out.push_str(&format!(
                "\nsubmitted by {}: {} (bundle {}, {} bytes)\n  evidence: {}",
                name(sub.seat), sub.summary, sub.bundle_hash, sub.bundle_len, sub.evidence
            ));
        }
        if let Some(commit) = &issue.merged_commit {
            out.push_str(&format!("\nmerged as {commit}"));
        }
        for comment in &issue.comments {
            out.push_str(&format!("\n  {} · {}", name(comment.seat), comment.text));
        }
        out
    }
}

fn format_host_links(header: &str, links: &[String]) -> String {
    let mut out = String::from(header);
    out.push_str("\nSend a friend a link (in their Angel: /table join <link> <name>):");
    for link in links {
        out.push_str(&format!("\n  {link}"));
    }
    out
}

fn issue_line(board: &Board, issue: &Issue) -> String {
    let claim = issue
        .claim
        .map(|c| format!(" · claimed by {}", board.seats.get(&c.seat).map_or("?", String::as_str)))
        .unwrap_or_default();
    format!("#{:<3} {:<11} {:+ }  {}{}", issue.id, issue.state.label(), issue.score(), issue.title, claim)
}

fn parse_id(text: &str) -> Option<u64> {
    text.trim().trim_start_matches('#').parse().ok()
}

fn split_id(rest: &str) -> Option<(u64, &str)> {
    let (id, tail) = rest.split_once(' ')?;
    Some((parse_id(id)?, tail.trim()))
}

fn split_bar(text: &str) -> (String, String) {
    match text.split_once('|') {
        Some((a, b)) => (a.trim().to_string(), b.trim().to_string()),
        None => (text.trim().to_string(), String::new()),
    }
}

/// Run git in `dir`; stdout on success, a one-line refusal otherwise.
fn git(dir: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let out = Proc::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|e| format!("git did not run: {e}"))?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        Err(format!("git {}: {}", args.join(" "), err.lines().next().unwrap_or("failed")))
    }
}

impl App {
    /// Present the active `/table` session's seats and board state on the
    /// painted Round Table card when the MoA council is idle.
    pub(crate) fn table_council_presentation(
        &mut self,
    ) -> Option<crate::ui::viz::council::CouncilPresentation> {
        use crate::ui::viz::agentviz::SeatState;
        use crate::ui::viz::council::{CouncilPresentation, CouncilSeat};
        use crate::ui::viz::council_table::Deed;

        if self.table_host.is_none() && self.table_guest.is_none() {
            return None;
        }
        let me = self.table_seat();
        let Some(board) = self.table_board() else {
            let stage = self
                .table_guest
                .as_ref()
                .map_or_else(|| "gathering".into(), |g| g.status_line());
            return Some(CouncilPresentation {
                stage,
                omitted_seats: 0,
                seats: Vec::new(),
                deed: Deed::Council,
            });
        };

        let omitted_seats = board.seats.len().saturating_sub(16);
        let seats: Vec<CouncilSeat> = board
            .seats
            .iter()
            .take(16)
            .map(|(seat, name)| {
                let answered = board.issues.values().any(|issue| {
                    issue.submission.as_ref().is_some_and(|s| s.seat == *seat)
                        || (issue.state == State::Merged
                            && issue.claim.as_ref().is_some_and(|c| c.seat == *seat))
                });
                let role = if *seat == ARBITER { " ★" } else { "" };
                let you = if Some(*seat) == me { " · you" } else { "" };
                CouncilSeat {
                    label: format!("{name}{role}{you}"),
                    state: if answered {
                        SeatState::Returned
                    } else {
                        SeatState::Running
                    },
                }
            })
            .collect();

        let has_submitted = board.issues.values().any(|i| i.state == State::Submitted);
        let has_in_progress = board.issues.values().any(|i| i.state == State::InProgress);
        let has_accepted = board.issues.values().any(|i| i.state == State::Accepted);
        let open_count = board
            .issues
            .values()
            .filter(|i| {
                matches!(
                    i.state,
                    State::Proposed | State::Accepted | State::InProgress | State::Submitted
                )
            })
            .count();
        let deed = if has_submitted {
            Deed::Judge
        } else if has_in_progress {
            Deed::Synthesis
        } else if has_accepted {
            Deed::Verify
        } else {
            Deed::Council
        };
        let stage = format!("{} seated · {open_count} open", board.seats.len());

        Some(CouncilPresentation {
            stage,
            omitted_seats,
            seats,
            deed,
        })
    }

    /// `/table work <id>`: claim the issue and pitch a worktree for
    /// this seat's own Angel to work in. The work itself is done by
    /// the seat's agent with its ordinary tools, in that folder.
    fn table_work(&mut self, id: u64) -> String {
        let seat = self.table_seat();
        let claimed = self.table_cast(Op::Claim { id }, "claimed");
        if claimed.starts_with("refused") || claimed.starts_with("not at") {
            return claimed;
        }
        let workspace = self.tools.current_workspace().to_path_buf();
        let dir = workspace.join(".angel").join("table-work").join(format!("issue-{id}"));
        if !dir.exists() {
            let branch = format!("table/issue-{id}-seat{}", seat.unwrap_or(0));
            if let Some(parent) = dir.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let dir_arg = dir.to_string_lossy().to_string();
            // A leftover branch from a re-claim is fine to reuse.
            let made = git(&workspace, &["worktree", "add", "-b", &branch, &dir_arg, "HEAD"])
                .or_else(|_| git(&workspace, &["worktree", "add", &dir_arg, &branch]));
            if let Err(e) = made {
                return format!("claimed #{id}, but no worktree: {e}");
            }
        }
        self.table_work.insert(id, dir.clone());
        format!(
            "claimed #{id}. Your worktree stands at {}\n\
             Work there with your Angel as usual, then:\n\
             /table submit {id} <summary> | <test evidence>",
            dir.display()
        )
    }

    /// `/table submit`: the worktree's whole diff against its base,
    /// staged and bundled, cast to the arbiter with its evidence.
    fn table_submit(&mut self, id: u64, summary: String, evidence: String) -> String {
        let Some(dir) = self.table_work.get(&id).cloned() else {
            return format!("no worktree for #{id} on this machine; /table work {id} first");
        };
        if summary.trim().is_empty() || evidence.trim().is_empty() {
            return "a submission needs a summary and its test evidence:\n/table submit <id> <summary> | <evidence>".into();
        }
        let bundle = git(&dir, &["add", "-A"])
            .and_then(|_| git(&dir, &["diff", "--cached", "--binary"]));
        match bundle {
            Ok(bytes) if bytes.is_empty() => format!("the worktree for #{id} has no changes to submit"),
            Ok(bytes) => {
                let kb = bytes.len() / 1024;
                let said = self.table_cast(
                    Op::Submit { id, summary, evidence, bundle: bytes },
                    &format!("submitted ({} KiB patch)", kb.max(1)),
                );
                said
            }
            Err(e) => format!("could not bundle the work: {e}"),
        }
    }

    /// `/table apply` (arbiter): lay the submitted bundle onto a
    /// scratch worktree of HEAD and report what it touches. Nothing
    /// is run from the patch; testing it there is the arbiter's next
    /// ordinary step, then merging by hand and `/table merge`.
    fn table_apply(&mut self, id: u64) -> String {
        let Some(host) = &self.table_host else {
            return "only the arbiter applies bundles, and this terminal is not hosting".into();
        };
        let Some(bundle) = host.shared.bundle(id) else {
            return format!("no bundle on file for #{id} (nothing submitted, or the table restarted from a guest's view)");
        };
        let workspace = self.tools.current_workspace().to_path_buf();
        let dir = workspace.join(".angel").join("table-scratch").join(format!("issue-{id}"));
        if !dir.exists() {
            if let Some(parent) = dir.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let dir_arg = dir.to_string_lossy().to_string();
            if let Err(e) = git(&workspace, &["worktree", "add", "--detach", &dir_arg, "HEAD"]) {
                return format!("no scratch worktree: {e}");
            }
        }
        let patch_path = dir.with_extension("patch");
        if let Err(e) = std::fs::write(&patch_path, &bundle) {
            return format!("could not stage the bundle: {e}");
        }
        let patch_arg = patch_path.to_string_lossy().to_string();
        match git(&dir, &["apply", "--check", &patch_arg])
            .and_then(|_| git(&dir, &["apply", &patch_arg]))
            .and_then(|_| git(&dir, &["diff", "--stat"]))
        {
            Ok(stat) => format!(
                "bundle for #{id} applied clean at {}\n{}\n\
                 Inspect and test it there; when merged by hand, record it:\n\
                 /table merge {id} <commit>",
                dir.display(),
                String::from_utf8_lossy(&stat).trim_end()
            ),
            Err(e) => format!("the bundle does not apply on HEAD: {e}"),
        }
    }

    fn table_export(&mut self, rest: &str) -> String {
        let Some(host) = &self.table_host else {
            return "the log lives with the arbiter; only the host exports it".into();
        };
        let workspace = self.tools.current_workspace();
        let path = if rest.is_empty() {
            workspace.join(".angel").join("round-table.jsonl")
        } else {
            workspace.join(rest)
        };
        match std::fs::write(&path, host.shared.board().export_jsonl()) {
            Ok(()) => format!("the table's log is at {}", path.display()),
            Err(e) => format!("could not write the log: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::viewer::Viewer;
    use crate::ui::viz::agentviz::SeatState;
    use crate::ui::viz::council_table::Deed;

    #[test]
    fn round_table_link_and_council_presentation_track_host_and_board() {
        let mut app = App::preview(Viewer::static_preview());
        // Hosting restores the board from the workspace's log: give the
        // table a fresh workspace, so no earlier run's issues come back.
        let workspace = std::env::temp_dir().join(format!(
            "angel-round-table-test-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&workspace).unwrap();
        std::sync::Arc::get_mut(&mut app.tools)
            .expect("preview registry should be uniquely owned")
            .set_workspace(workspace.clone());
        assert!(app.table_council_presentation().is_none());
        assert_eq!(
            app.table_command(Some("link")),
            "not at a table: /table host, or /table join <link>"
        );

        let hosted = app.table_command(Some("host Arthur"));
        assert!(
            hosted.contains("You are the arbiter (seat 0)."),
            "unexpected host reply: {hosted}"
        );

        let reprinted = app.table_command(Some("link"));
        assert!(
            reprinted.contains("You are hosting the Round Table (arbiter, seat 0).")
                && reprinted.contains("/table join"),
            "unexpected /table link output: {reprinted}"
        );

        let pres = app
            .table_council_presentation()
            .expect("hosted table presents a council card");
        assert_eq!(pres.stage, "1 seated · 0 open");
        assert_eq!(pres.deed, Deed::Council);
        assert_eq!(pres.seats.len(), 1);
        assert_eq!(pres.seats[0].label, "Arthur ★ · you");
        assert_eq!(pres.seats[0].state, SeatState::Running);

        app.table_command(Some("new Forge Excalibur | Temper the blade."));
        app.table_command(Some("accept 1"));
        let pres = app.table_council_presentation().unwrap();
        assert_eq!(pres.deed, Deed::Verify);
        assert_eq!(pres.stage, "1 seated · 1 open");

        let stopped = app.table_command(Some("stop"));
        assert!(stopped.contains("the table is cleared"));
        assert!(app.table_council_presentation().is_none());
        let _ = std::fs::remove_dir_all(&workspace);
    }
}

