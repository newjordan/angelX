//! Skills slash command (`/skills`) — a thin presentation layer over the ONE
//! shared skill loader in `harness` (`load_skills_for`). `/skills` lists what is
//! available; `/skills <name>[,<name>...] [task]` loads up to four bundles in
//! explicit order and hands them to the agent as bounded context.
//!
//! Discovery + parsing is the harness loader's job, so this command and the
//! `skill(...)` agent tool see the *same* catalog and use the *same* parser:
//! both the folder-per-skill `SKILL.md` (+ `---` frontmatter) convention and the
//! flat `<name>.md` layout are recognised identically. Project skills may live
//! under `.claude/skills`, `.opencode/skills`, or `.agents/skills`; compatible
//! global roots are also discovered. User skills live in `ANGEL_SKILLS_DIR`
//! (default `~/.angel0/skills`) and win over every compatible source.

use std::path::{Path, PathBuf};

/// The user skills dir (for help text only). The actual catalog is bundled +
/// user, merged by the shared loader; this is just where a user drops their own.
fn user_dir() -> PathBuf {
    if let Ok(d) = std::env::var("ANGEL_SKILLS_DIR")
        && !d.trim().is_empty()
    {
        return PathBuf::from(d);
    }
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".angel0").join("skills")
}

/// Where user skills are read from (for the "no skills" help text).
pub fn dir_display() -> String {
    user_dir().display().to_string()
}

/// Names of available skills (bundled + user), sorted — via the shared loader.
pub fn list_for(workspace: &Path) -> Vec<String> {
    crate::harness::load_skills_for(workspace)
        .into_iter()
        .map(|s| s.name)
        .collect()
}

/// Bounded, model-free keyword search over the already-admitted catalog.
/// Matching is case-insensitive across names and one-line descriptions; skill
/// bodies stay unloaded and unrendered.
pub fn search_for(workspace: &Path, query: &str) -> String {
    const MAX_QUERY_CHARS: usize = 128;
    const MAX_RESULTS: usize = 24;

    let query = query.trim();
    if query.is_empty() {
        return "usage: /skills search <query>".to_string();
    }
    if query.chars().count() > MAX_QUERY_CHARS {
        return format!("/skills search: query too long (max {MAX_QUERY_CHARS} characters)");
    }
    let needles = query
        .split_whitespace()
        .map(str::to_lowercase)
        .collect::<Vec<_>>();
    let query_lower = query.to_lowercase();
    let mut matches = crate::harness::load_skills_for(workspace)
        .into_iter()
        .filter_map(|skill| {
            let name = skill.name.to_lowercase();
            let description = skill.description.to_lowercase();
            if !needles
                .iter()
                .all(|needle| name.contains(needle) || description.contains(needle))
            {
                return None;
            }
            let rank = if name == query_lower {
                0
            } else if needles.iter().all(|needle| name.contains(needle)) {
                1
            } else {
                2
            };
            Some((rank, skill))
        })
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.name.cmp(&right.1.name))
    });

    let label = crate::harness::safe_audit_entry_label(std::ffi::OsStr::new(query));
    let mut out = format!("skills search {label:?} · {} match(es)", matches.len());
    for (_, skill) in matches.iter().take(MAX_RESULTS) {
        out.push_str("\n- ");
        out.push_str(&skill.name);
        if !skill.description.is_empty() {
            out.push_str(" · ");
            out.push_str(&crate::harness::safe_audit_entry_label(
                std::ffi::OsStr::new(&skill.description),
            ));
        }
    }
    if matches.len() > MAX_RESULTS {
        out.push_str(&format!(
            "\n- {} additional match(es) omitted",
            matches.len() - MAX_RESULTS
        ));
    }
    if matches.is_empty() {
        out.push_str("\nno installed skill matched · /skills lists the admitted catalog");
    } else {
        out.push_str("\n/skills <name> [task] to apply one");
    }
    out
}

/// Bounded health report for every configured skill source. This shares the
/// loader's exact admission path, so a rejection shown here is the reason the
/// same entry is absent from `/skills` and `skill(name)`.
pub fn check_for(workspace: &Path) -> String {
    let audit = crate::harness::audit_skills_for(workspace);
    let mut out = format!(
        "skills check · {} active · {} manual-only · {} usable entries · {}/{} sources available · {} rejected",
        audit.active_skills,
        audit.manual_only_skills,
        audit.usable_entries,
        audit.available_sources,
        audit.configured_sources,
        audit.rejected_entries
    );
    if audit.capped_entries > 0 {
        out.push_str(&format!(
            " · {} entries beyond the per-source catalog cap",
            audit.capped_entries
        ));
    }
    if audit.rejected_entries == 0 && audit.capped_entries == 0 {
        out.push_str("\nhealthy · every examined skill candidate is usable");
        return out;
    }
    for issue in &audit.issues {
        let source = crate::harness::safe_audit_entry_label(issue.source.as_os_str());
        out.push_str("\n- ");
        out.push_str(&source);
        out.push('/');
        out.push_str(&issue.entry);
        out.push_str(": ");
        out.push_str(issue.reason);
    }
    let omitted = audit.rejected_entries.saturating_sub(audit.issues.len());
    if omitted > 0 {
        out.push_str(&format!(
            "\n- {omitted} additional rejection(s) omitted from this bounded report"
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_and_load_from_a_skills_dir() {
        // Mutates ANGEL_SKILLS_DIR (process-global env) — serialize with every
        // other env-touching test so the parallel runner can't race the environ
        // table (harness/tests/skills.rs sets the same var under this lock).
        let _guard = crate::tests::env_lock();
        let tmp = std::env::temp_dir().join(format!("angel_skills_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&tmp);
        std::fs::write(
            tmp.join("unique-test-review-skill.md"),
            "Be a strict reviewer.",
        )
        .unwrap();
        std::fs::write(
            tmp.join("unique-test-rust-debug.md"),
            "Diagnose zqxcompilerneedle failures without guessing.\u{1b}[31m",
        )
        .unwrap();
        for index in 0..30 {
            std::fs::write(
                tmp.join(format!("unique-test-search-cap-{index:02}.md")),
                "Apply zqxcompilerneedle verification.",
            )
            .unwrap();
        }
        std::fs::write(tmp.join("notes.txt"), "ignored").unwrap();
        let old_user = std::env::var_os("ANGEL_SKILLS_DIR");
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_SKILLS_DIR", &tmp) };
        let workspace = std::env::current_dir().unwrap();
        let names = list_for(&workspace);
        let by_name = search_for(&workspace, "rust debug");
        let by_description = search_for(&workspace, "zqxcompilerneedle");
        let no_match = search_for(&workspace, "definitely-absent-query");
        restore_env("ANGEL_SKILLS_DIR", old_user);
        let _ = std::fs::remove_dir_all(&tmp);
        assert!(
            names.contains(&"unique-test-review-skill".to_string()),
            "test skill missing from catalog: {names:?}"
        );
        assert!(by_name.contains("unique-test-rust-debug"), "{by_name}");
        assert!(
            by_description.contains("unique-test-rust-debug"),
            "{by_description}"
        );
        assert!(!by_description.contains('\u{1b}'), "{by_description:?}");
        assert!(by_description.contains("31 match(es)"), "{by_description}");
        assert!(
            by_description.contains("7 additional match(es) omitted"),
            "{by_description}"
        );
        assert_eq!(by_description.lines().count(), 27, "{by_description}");
        assert!(
            no_match.contains("no installed skill matched"),
            "{no_match}"
        );
        assert_eq!(search_for(&workspace, ""), "usage: /skills search <query>");
    }

    fn restore_env(key: &str, old: Option<std::ffi::OsString>) {
        match old {
            // TODO: Audit that the environment access only happens in single-threaded code.
            Some(v) => unsafe { std::env::set_var(key, v) },
            // TODO: Audit that the environment access only happens in single-threaded code.
            None => unsafe { std::env::remove_var(key) },
        }
    }
}
