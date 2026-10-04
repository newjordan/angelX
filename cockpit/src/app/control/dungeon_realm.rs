//! The party's play economy in the cockpit: banked spoils reach the island's
//! treasury, wishes are asked, drafted by angelX and granted.
use super::*;
use crate::drive::together_realm::{self as realm, Realm, Status};
use crate::drive::together_shooter as shooter;

impl App {
    /// The island's realm, loaded beside the world's rewards on first use.
    pub(crate) fn realm(&mut self) -> &mut Realm {
        if self.dungeon.realm.is_none() {
            self.dungeon.realm = Some(Realm::beside(self.world.rewards_path()));
        }
        self.dungeon.realm.as_mut().expect("loaded above")
    }

    /// The host's name on plaques: the first word of git's user.name, else $USER.
    pub(crate) fn host_name(&self) -> String {
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

    /// Move what the run banked into the treasury, under each knight's name.
    pub(crate) fn settle_realm(&mut self) {
        let Some(run) = self.dungeon.shooter.as_mut() else {
            return;
        };
        if run.bank.is_empty() && run.reclaimed.is_empty() {
            return;
        }
        let hauls = std::mem::take(&mut run.bank);
        let reclaimed = std::mem::take(&mut run.reclaimed);
        let won = run.phase == shooter::Phase::Won;
        let names: std::collections::BTreeMap<u32, String> = run
            .players
            .iter()
            .map(|(&id, h)| (id, h.name.clone()))
            .collect();
        let host = self.host_name();
        let mut lines = Vec::new();
        let realm = self.realm();
        for haul in &hauls {
            let name = match names.get(&haul.hero).map(String::as_str) {
                Some("You") | None if haul.hero == 1 => host.clone(),
                Some(name) => name.to_string(),
                None => format!("Knight {}", haul.hero),
            };
            realm.bank(&name, &haul.spoils);
            lines.push(format!(
                "{name} banked {} ({})",
                haul.spoils.label(),
                haul.why
            ));
        }
        for pack in reclaimed {
            *realm.reclaimed.entry(pack.name().to_string()).or_default() += 1;
        }
        if won {
            realm.raids_won += 1;
            realm.offer_catalog();
        }
        let affordable: Vec<String> = realm
            .wishes
            .iter()
            .filter(|w| w.status == Status::Drafted && realm.treasury.covers(&w.price))
            .map(|w| w.name.clone())
            .collect();
        let saved = realm.save();
        let now = std::time::Instant::now();
        if !hauls.is_empty() {
            self.dungeon.chorus.cue("banked", now);
        }
        if affordable.len() > self.dungeon.ready_heard {
            self.dungeon.chorus.cue("treasury_ready", now);
        }
        self.dungeon.ready_heard = affordable.len();
        if !lines.is_empty() {
            let mut notice = lines.join(" · ");
            if !affordable.is_empty() {
                notice.push_str(&format!(
                    " · the treasury can raise: {} (Tab → realm)",
                    affordable.join(", ")
                ));
            }
            self.dungeon.notice = notice;
        }
        if let Err(error) = saved {
            self.dungeon.notice = format!("Could not save the realm: {error}");
        }
        // The run's bank is empty now; checkpoint so a crash cannot bank twice.
        let _ = self.dungeon.checkpoint(std::time::Instant::now(), true);
        self.publish_realm();
    }

    /// When angelX's drafting turn ends, read its draft in — no command needed.
    pub(crate) fn watch_drafting(&mut self) {
        let Some((id, started, seen)) = self.dungeon.drafting.clone() else {
            return;
        };
        let busy = self.thinking.is_some() || self.pending_turn.is_some();
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
        self.publish_realm();
    }

    /// The guest page shows the treasury and the wishes.
    pub(crate) fn publish_realm(&mut self) {
        if self.dungeon.guest.is_none() {
            return;
        }
        // Redraw the realm picture when a wish rises (and once per guest).
        let standing = self.realm().built().len();
        if self.dungeon.realm_drawn != Some(standing) {
            self.dungeon.realm_drawn = Some(standing);
            let img = crate::stage::world_viz::overworld::wish_view(&self.world.overworld_scene());
            if let Some(server) = &self.dungeon.guest {
                server.set_realm_picture(&img.rgb_bytes(), img.w as u32, img.h as u32);
            }
        }
        let drafting = self.dungeon.drafting.as_ref().map(|(id, _, _)| id.clone());
        let view = crate::drive::together_guest::RealmHud::of(self.realm(), drafting);
        if let Some(server) = &self.dungeon.guest {
            server.set_realm(view);
        }
    }

    /// Where angelX writes a wish's draft in this workspace.
    pub(super) fn wishes_dir(&self) -> std::path::PathBuf {
        self.tools.current_workspace().join(".angel/realm/wishes")
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
        self.publish_realm();
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
            self.input = brief;
            self.cursor = self.input.len();
            self.submit();
            self.dungeon.drafting = Some((id.clone(), std::time::Instant::now(), false));
            self.publish_realm();
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
                message
            }
            Err(error) => error,
        };
        self.dungeon.notice = message.clone();
        self.publish_realm();
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
            self.publish_realm();
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
