//! The party's play economy in the cockpit: banked spoils reach the island's
//! treasury, wishes are asked, drafted by angelX and granted.
use super::*;
use super::dungeon::DungeonView;
use crate::together_realm::{self as realm, Realm, Status};
use crate::together_shooter as shooter;

impl Session<'_> {
    /// The island's realm, loaded beside the world's rewards on first use.
    pub(crate) fn realm(&mut self) -> &mut Realm {
        economy(&mut self.dungeon.realm, self.world)
    }

    /// The host's name on plaques: the first word of git's user.name, else $USER.
    pub fn host_name(&self) -> String {
        host_name()
    }

    /// Move what the run banked into the treasury, under each knight's name.
    pub fn settle_realm(&mut self) -> bool {
        // A claim remains in the game journal until both stores agree on its
        // identity. New spoils can accumulate while this exact claim retries.
        for _ in 0..16 {
            let settled = (|| {
                let Some(claim) = self.dungeon.shooter.claim_realm()? else {
                    return Ok(None);
                };
                if self.realm().persistent() && self.dungeon.save_path.is_none() {
                    return Err("the game checkpoint path is unavailable".into());
                }
                // Save the emptied queues AND retained receipt in one core
                // capture before the destination is allowed to spend it.
                self.dungeon.checkpoint(std::time::Instant::now(), true)?;
                let host = self.host_name();
                let result = self.realm().settle_claim(&claim, &host)?;
                self.dungeon.shooter.acknowledge_realm(&claim)?;
                self.dungeon.checkpoint(std::time::Instant::now(), true)?;
                Ok::<_, String>(Some((claim, result)))
            })();
            let (claim, result) = match settled {
                Ok(Some(settled)) => settled,
                Ok(None) => return true,
                Err(error) => {
                    self.dungeon.notice = format!("Could not save the realm: {error}");
                    return false;
                }
            };
            let now = std::time::Instant::now();
            if result.applied && !claim.transfer.hauls.is_empty() {
                self.dungeon.chorus.cue("banked", now);
            }
            if result.tribute_paid {
                self.dungeon.chorus.cue("tribute", now);
            }
            if result.applied && result.affordable.len() > self.dungeon.ready_heard {
                self.dungeon.chorus.cue("treasury_ready", now);
            }
            self.dungeon.ready_heard = result.affordable.len();
            if !result.lines.is_empty() {
                let mut notice = result.lines.join(" · ");
                if !result.affordable.is_empty() {
                    notice.push_str(&format!(
                        " · the treasury can raise: {} (Tab → realm)",
                        result.affordable.join(", ")
                    ));
                }
                self.dungeon.notice = notice;
            }
        }
        // Bound UI work if a producer keeps banking; a restart/close may not
        // discard a backlog simply because one bounded transfer completed.
        self.dungeon.notice = "The realm is still banking the party's spoils".into();
        false
    }

    /// Pay for what the party asked for at the Undercroft's plates out of
    /// the treasury, remember the deepest floor reached and the landing
    /// chosen, and tell the run what now stands and what it can spend.
    pub fn settle_home(&mut self) -> bool {
        self.flush_holdings_ops();
        for _ in 0..16 {
            let settled = (|| {
                let context = self.realm().home_context();
                self.dungeon
                    .shooter
                    .sync_home(context.home.clone(), context.treasury.clone())?;
                let Some(claim) = self.dungeon.shooter.claim_home_with_context(&context)? else {
                    return Ok(None);
                };
                if self.realm().persistent() && self.dungeon.save_path.is_none() {
                    return Err("the game checkpoint path is unavailable".into());
                }
                // Persist reservations/facts before the destination is allowed
                // to pay. A replay always uses this same request and payload.
                self.dungeon.checkpoint(std::time::Instant::now(), true)?;
                if self.dungeon.shooter.home_completion(&claim)?.is_none() {
                    let host = self.host_name();
                    let unsaved = self.dungeon.bounties_unsaved;
                    let (realm, sim) = (&mut self.dungeon.realm, &mut self.dungeon.shooter);
                    let result = realm
                        .as_mut()
                        .ok_or("realm was not loaded")?
                        .prepare_home_claim(&claim, &host, unsaved, |request| {
                            sim.evaluate_home(request)
                        })?;
                    self.dungeon.shooter.complete_home(&claim, result)?;
                }
                let completed = self
                    .dungeon
                    .shooter
                    .home_completion(&claim)?
                    .ok_or("home delivery did not commit")?;
                // Only after world+delivery is durable may a refund commit and
                // its destination discard the large prepared result.
                self.dungeon.checkpoint(std::time::Instant::now(), true)?;
                self.realm().finish_home_claim(&claim, &completed)?;
                let realm = self.realm();
                let (home, treasury) = (realm.home.clone(), realm.treasury.clone());
                self.dungeon.shooter.sync_home(home, treasury)?;
                self.dungeon.shooter.acknowledge_home(&claim, &completed)?;
                self.dungeon.checkpoint(std::time::Instant::now(), true)?;
                Ok::<_, String>(Some(completed))
            })();
            let completed = match settled {
                Ok(Some(completed)) => completed,
                Ok(None) => return true,
                Err(error) => {
                    self.dungeon.notice = format!("Could not save the realm: {error}");
                    return false;
                }
            };
            self.dungeon.bounties_unsaved = false;
            let now = std::time::Instant::now();
            let built_or_opened = !completed.result.said.is_empty();
            for (cue, notice) in completed.result.said {
                self.dungeon.chorus.cue(&cue, now);
                self.dungeon.notice = notice;
            }
            if let Some(feat) = completed
                .result
                .announced
                .last()
                .and_then(|id| shooter::feats::feat(id))
            {
                self.dungeon.notice = if built_or_opened {
                    format!(
                        "{} · Deed: {} ({})",
                        self.dungeon.notice,
                        feat.name,
                        feat.tier.name()
                    )
                } else {
                    format!(
                        "Deed recorded: {}. {} ({} in the Herald's coffer, Fortune's hall)",
                        feat.name,
                        feat.says,
                        feat.tier.name()
                    )
                };
            }
            if !completed.delivered.refund.is_empty() {
                self.dungeon.notice = format!(
                    "Purchase could not be delivered; {} returned to the treasury",
                    completed.delivered.refund.label()
                );
            }
        }
        self.dungeon.notice = "The realm is still settling the party's purchases".into();
        false
    }

    /// angelX's own work since the last settle, folded into the holdings:
    /// stores, walls, labour, scrolls, retainers, patrols and unrest (never
    /// gold), saved with the realm and shown in the open run.
    pub(crate) fn flush_holdings_ops(&mut self) {
        if self.dungeon.ops.is_empty() {
            return;
        }
        let ops = std::mem::take(&mut self.dungeon.ops);
        let realm = self.realm();
        let said = shooter::holdings::feed(&mut realm.home.holdings, &ops);
        // A failed save keeps the change in memory; the next save carries it.
        let _ = realm.save();
        let (home, treasury) = (self.realm().home.clone(), self.realm().treasury.clone());
        let _ = self.dungeon.shooter.sync_home(home, treasury);
        if let Some(line) = said.last() {
            self.dungeon.notice = line.clone();
        }
    }

    /// A turn ends: a failed one stirs unrest; calls that never reported
    /// are forgotten; the holdings hear it all.
    pub fn note_ops_end(&mut self, ok: bool) {
        if !ok {
            self.dungeon.ops.errors += 1;
        }
        self.dungeon.ops_calls.clear();
        self.flush_holdings_ops();
    }

    /// When angelX's drafting turn ends, read its draft in — no command needed.
    pub fn watch_drafting(&mut self) {
        let Some((id, started, seen)) = self.dungeon.drafting.clone() else {
            return;
        };
        let busy = self.facts().busy();
        if busy {
            if !seen {
                self.dungeon.drafting = Some((id, started, true));
            }
            return;
        }
        if !seen && started.elapsed() < std::time::Duration::from_secs(10) {
            return;
        }
        self.dungeon.drafting = None;
        let path = self.wishes_dir().join(format!("{id}.wish"));
        self.dungeon.notice = if path.is_file() {
            let report = self.dungeon_wishes("reload");
            let line = report
                .lines()
                .find(|l| l.starts_with(&format!("{id}:")))
                .unwrap_or(&report)
                .to_string();
            format!("angelX drafted the wish — {line}. Tab → realm to see its price.")
        } else {
            format!(
                "angelX's turn ended without {}; /dungeon grant it again to retry.",
                path.display()
            )
        };
    }

    /// Where angelX writes a wish's draft in this workspace.
    pub(super) fn wishes_dir(&self) -> std::path::PathBuf {
        self.cx.workspace().join(".angel/realm/wishes")
    }

    /// A wish in someone's own words.
    pub(crate) fn ask_wish(&mut self, by: &str, words: &str) -> String {
        let result = self.realm().ask(by, words);
        let message = match result {
            Ok(id) => {
                let _ = self.realm().save();
                format!(
                    "{by} wished for “{}”. /dungeon grant {id} asks angelX to draft it.",
                    words.trim()
                )
            }
            Err(error) => error,
        };
        message
    }

    /// Grant a wish by id or list number: draft it with angelX when it is
    /// only words, raise it when it is drafted and paid for.
    pub(crate) fn grant_wish(&mut self, which: &str) -> String {
        let which = which.trim();
        let id = {
            let realm = self.realm();
            which
                .parse::<usize>()
                .ok()
                .and_then(|n| realm.wishes.get(n.wrapping_sub(1)))
                .or_else(|| realm.wishes.iter().find(|w| w.id == which))
                .map(|w| w.id.clone())
        };
        let Some(id) = id else {
            return format!("No wish `{which}`. /dungeon wishes lists them.");
        };
        let status = self
            .realm()
            .wishes
            .iter()
            .find(|w| w.id == id)
            .map(|w| w.status);
        if status == Some(Status::Asked) {
            let dir = self.wishes_dir();
            let path = dir.join(format!("{id}.wish"));
            let realm = self.realm();
            let wish = realm
                .wishes
                .iter()
                .find(|w| w.id == id)
                .cloned()
                .expect("found");
            let brief = realm::brief(&wish, &path, &realm.treasury);
            let _ = std::fs::create_dir_all(&dir);
            // Hand the wish to angelX in the open: the party watches it draft.
            self.collapse_dungeon();
            self.cx.make(Move::Submit { brief, keep_draft: false });
            self.dungeon.drafting = Some((id.clone(), std::time::Instant::now(), false));
            return format!(
                "angelX is drafting “{}”. Its draft is read in when the turn ends; then /dungeon grant {id} raises it once the treasury can pay.",
                wish.words
            );
        }
        let result = self.realm().grant(&id);
        let message = match result {
            Ok(message) => {
                let _ = self.realm().save();
                self.dungeon
                    .chorus
                    .cue(&format!("wish_raised:{id}"), std::time::Instant::now());
                // Say so when the realm has no ground left to stand it on,
                // rather than raise it out of sight.
                let homeless =
                    crate::world_viz::overworld::wishes::unplaced(&self.realm().built());
                if homeless.is_empty() {
                    message
                } else {
                    format!(
                        "{message}, but the realm has no free ground to show {} yet",
                        homeless.join(", ")
                    )
                }
            }
            Err(error) => error,
        };
        self.dungeon.notice = message.clone();
        message
    }

    /// `/dungeon wishes [reload]`: the wishing stone, or angelX's drafts read in.
    pub(crate) fn dungeon_wishes(&mut self, tail: &str) -> String {
        if tail == "reload" {
            let dir = self.wishes_dir();
            let mut report = Vec::new();
            let mut paths: Vec<_> = std::fs::read_dir(&dir)
                .map(|entries| entries.filter_map(Result::ok).map(|e| e.path()).collect())
                .unwrap_or_default();
            paths.retain(|p: &std::path::PathBuf| {
                p.extension().is_some_and(|e| e == "wish") && p.is_file()
            });
            paths.sort();
            for path in paths {
                let stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                let raw = match std::fs::metadata(&path) {
                    Ok(m) if m.len() as usize <= realm::MAX_BYTES => {
                        std::fs::read_to_string(&path).unwrap_or_default()
                    }
                    _ => {
                        report.push(format!("{stem}: too large"));
                        continue;
                    }
                };
                // Pictures named by `sheet art <file>` are cut into rows.
                let sheets = self.cx.workspace().join(".angel/realm/sheets");
                let paint = crate::together_sheets::paint_dir(&self.cx.workspace());
                let raw = match crate::together_sheets::inline(&raw, &sheets, &paint, realm::ART as u32) {
                    Ok(raw) => raw,
                    Err(errors) => {
                        report.push(format!("{stem}: {}", errors.join("; ")));
                        continue;
                    }
                };
                match realm::check(&stem, &raw) {
                    Ok((draft, notes)) => {
                        let name = draft.name.clone();
                        match self.realm().draft(draft) {
                            Ok(()) if notes.is_empty() => {
                                report.push(format!("{stem}: {name} drafted"))
                            }
                            Ok(()) => report
                                .push(format!("{stem}: {name} drafted ({})", notes.join("; "))),
                            Err(error) => report.push(format!("{stem}: {error}")),
                        }
                    }
                    Err(errors) => report.push(format!("{stem}: {}", errors.join("; "))),
                }
            }
            let _ = self.realm().save();
            return if report.is_empty() {
                format!("No drafts in {}.", dir.display())
            } else {
                report.join("\n")
            };
        }
        let realm = self.realm();
        let mut out = vec![format!("Treasury: {}", realm.treasury.label())];
        for (i, wish) in realm.wishes.iter().enumerate() {
            let line = match wish.status {
                Status::Asked => format!(
                    "{}. “{}” — {} wished it; /dungeon grant {} has angelX draft it",
                    i + 1,
                    wish.words,
                    wish.by,
                    i + 1
                ),
                Status::Drafted => {
                    let short = realm.treasury.shortfall(&wish.price);
                    format!(
                        "{}. {} — costs {}{}",
                        i + 1,
                        wish.name,
                        wish.price.label(),
                        if short.is_empty() {
                            format!(" · ready: /dungeon grant {}", i + 1)
                        } else {
                            format!(" · {short}")
                        }
                    )
                }
                Status::Built => format!("{}. {} — stands in the realm", i + 1, wish.name),
            };
            out.push(line);
        }
        let sources: Vec<String> = realm::Spoil::ALL
            .iter()
            .map(|s| format!("{} from {}", s.word(), s.source()))
            .collect();
        out.push(format!(
            "/dungeon wish <your words> makes a new wish · spoils: {}",
            sources.join(", ")
        ));
        out.join("\n")
    }
}

/// What a tool call is to the holdings, by the herald's reading of it: a
/// commit, a test run, a compaction (each counted when it comes back
/// clean), or a subagent sent out (counted as it goes).
pub fn op_of_call(name: &str, args: &str) -> Option<(shooter::holdings::Op, bool)> {
    use crate::world_viz::{Deed as Work, herald};
    use shooter::holdings::Op;
    let leaf = name.rsplit(['.', ':', '/']).next().unwrap_or(name).to_ascii_lowercase();
    if matches!(leaf.as_str(), "spawn" | "delegate") {
        return Some((Op::Subagents, true));
    }
    if leaf.contains("compact") {
        return Some((Op::Compactions, false));
    }
    let told = herald(name, args);
    match told.deed {
        Work::Seal => Some((Op::Commits, false)),
        Work::Trial if told.object.contains("test") => Some((Op::Tests, false)),
        _ => None,
    }
}

/// The island's realm in `slot`, loaded beside the world's rewards on first use.
pub fn economy<'a>(slot: &'a mut Option<Realm>, world: &crate::world_viz::World) -> &'a mut Realm {
    if slot.is_none() {
        *slot = Some(Realm::beside(world.rewards_path()));
    }
    slot.as_mut().expect("loaded above")
}

/// The host's name on plaques: the first word of git's user.name, else $USER.
pub fn host_name() -> String {
    static NAME: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    NAME.get_or_init(|| {
        std::process::Command::new("git")
            .args(["config", "user.name"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| s.split_whitespace().next().map(str::to_owned))
            .or_else(|| std::env::var("USER").ok())
            .unwrap_or_else(|| "Host".into())
    })
    .clone()
}

impl DungeonView {
    /// A tool call starts: a subagent is a retainer at once; a commit, a
    /// test run or a compaction waits for its result.
    pub fn note_ops_call(&mut self, id: &str, name: &str, args: &str) {
        match op_of_call(name, args) {
            Some((op, true)) => self.ops.note(op),
            Some((op, false)) if self.ops_calls.len() < 64 => {
                self.ops_calls.insert(id.to_string(), op);
            }
            _ => {}
        }
    }


    /// A tool call's result: counted for the holdings when it came back clean.
    pub fn note_ops_result(&mut self, id: &str, clean: bool) {
        if let Some(op) = self.ops_calls.remove(id)
            && clean
        {
            self.ops.note(op);
        }
    }


    /// A turn's spend: its fresh tokens and cache hits, and whether it was
    /// slow.
    pub fn note_ops_turn(&mut self, cache: u64, fresh: u64, secs: u64) {
        let ops = &mut self.ops;
        ops.cache += cache;
        ops.tokens += fresh;
        if secs >= shooter::holdings::SLOW_TURN_SECS {
            ops.slow += 1;
        }
    }


    /// The loop's iterations so far: each new one is a patrol.
    pub fn note_ops_loop(&mut self, iteration: usize) {
        if iteration > self.ops_loop {
            let new = (iteration - self.ops_loop).min(64) as u32;
            self.ops.loops += new;
        }
        self.ops_loop = iteration;
    }

}

#[cfg(test)]
#[path = "../../tests/together_holdings__tests.rs"]
mod holdings_tests;
