//! The party's play economy in the cockpit: banked spoils reach the island's
//! treasury, wishes are asked, drafted by angelX and granted.
use super::*;
use super::dungeon::DungeonView;
use crate::together_realm::{self as realm, Realm, Status};
use crate::together_shooter as shooter;

impl Session<'_> {
    pub(crate) fn realm(&mut self) -> &mut Realm {
        self.dungeon.collect_settled(true);
        economy(&mut self.dungeon.realm, self.world)
    }

    pub fn settle_off_frame(&mut self) -> bool {
        if self.dungeon.settling.is_some() {
            return true;
        }
        if self.dungeon.shooter.link().is_none() {
            return false;
        }
        self.flush_holdings_ops();
        self.realm();
        self.dungeon.settle_off_frame()
    }

    fn settle_in_place(
        &mut self,
        part: fn(&mut Realm, &mut super::dungeon_settle::InPlace<'_>, &mut super::dungeon_settle::Io) -> bool,
    ) -> bool {
        self.realm();
        let mut realm = self.dungeon.realm.take().expect("the realm was loaded above");
        let mut io = super::dungeon_settle::Io::of(self.dungeon);
        let ok = part(&mut realm, &mut super::dungeon_settle::InPlace(self.dungeon), &mut io);
        self.dungeon.realm = Some(realm);
        io.apply(self.dungeon);
        ok
    }

    pub fn host_name(&self) -> String {
        host_name()
    }

    pub fn settle_realm(&mut self) -> bool {
        self.settle_in_place(|realm, game, io| super::dungeon_settle::realm_cycle(realm, game, io))
    }

    pub fn settle_home(&mut self) -> bool {
        self.flush_holdings_ops();
        self.settle_in_place(|realm, game, io| super::dungeon_settle::home_cycle(realm, game, io))
    }

    pub(crate) fn flush_holdings_ops(&mut self) {
        if self.dungeon.ops.is_empty() {
            return;
        }
        let ops = std::mem::take(&mut self.dungeon.ops);
        let realm = self.realm();
        let said = shooter::holdings::feed(&mut realm.home.holdings, &ops);
        let _ = realm.save();
        let (home, treasury) = (self.realm().home.clone(), self.realm().treasury.clone());
        let _ = self.dungeon.shooter.sync_home(home, treasury);
        if let Some(line) = said.last() {
            self.dungeon.notice = line.clone();
        }
    }

    pub fn note_ops_end(&mut self, ok: bool) {
        if !ok {
            self.dungeon.ops.errors += 1;
        }
        self.dungeon.ops_calls.clear();
        self.flush_holdings_ops();
    }

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

    pub(super) fn wishes_dir(&self) -> std::path::PathBuf {
        self.cx.workspace().join(".angel/realm/wishes")
    }

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

pub fn economy<'a>(slot: &'a mut Option<Realm>, world: &crate::world_viz::World) -> &'a mut Realm {
    if slot.is_none() {
        *slot = Some(Realm::beside(world.rewards_path()));
    }
    slot.as_mut().expect("loaded above")
}

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
    pub fn note_ops_call(&mut self, id: &str, name: &str, args: &str) {
        match op_of_call(name, args) {
            Some((op, true)) => self.ops.note(op),
            Some((op, false)) if self.ops_calls.len() < 64 => {
                self.ops_calls.insert(id.to_string(), op);
            }
            _ => {}
        }
    }

    pub fn note_ops_result(&mut self, id: &str, clean: bool) {
        if let Some(op) = self.ops_calls.remove(id)
            && clean
        {
            self.ops.note(op);
        }
    }

    pub fn note_ops_turn(&mut self, cache: u64, fresh: u64, secs: u64) {
        let ops = &mut self.ops;
        ops.cache += cache;
        ops.tokens += fresh;
        if secs >= shooter::holdings::SLOW_TURN_SECS {
            ops.slow += 1;
        }
    }

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
