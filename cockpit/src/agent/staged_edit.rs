//! Staged edit previews — propose, then accept/reject (oh-my-pi `ast_edit` +
//! `resolve` spirit, without tree-sitter).
//!
//! A hashline (or other) patch can be **staged** instead of written: the
//! preflight plan is retained in a process session queue. The agent calls
//! `resolve_edit` with `accept` (or `reject`) to commit or discard. Accept is
//! atomic across the batch (same rollback rules as live apply).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use crate::agent::harness::{
    confined_create_new, confined_edit, confined_read, confined_remove_file, confined_write,
};
use crate::agent::hashline::SectionPlan;

const MAX_STAGED: usize = 32;

/// One planned filesystem mutation held off disk until accept.
#[derive(Debug, Clone)]
pub(crate) enum StagedAction {
    Update {
        path: String,
        original: Vec<u8>,
        content: String,
    },
    Remove {
        path: String,
        original: Vec<u8>,
    },
    Move {
        from: String,
        to: String,
        original: Vec<u8>,
        content: String,
    },
}

impl StagedAction {
    fn summary_line(&self) -> String {
        match self {
            Self::Update {
                path,
                original,
                content,
            } => {
                let old_n = String::from_utf8_lossy(original).lines().count();
                let new_n = content.lines().count();
                format!("update {path} ({old_n} → {new_n} lines)")
            }
            Self::Remove { path, .. } => format!("remove {path}"),
            Self::Move {
                from, to, content, ..
            } => {
                format!("move {from} → {to} ({} lines)", content.lines().count())
            }
        }
    }
}

/// A multi-file proposed batch (one `apply_patch` / hashline call).
#[derive(Debug, Clone)]
pub(crate) struct StagedBatch {
    pub(crate) id: u32,
    pub(crate) workspace: PathBuf,
    pub(crate) actions: Vec<StagedAction>,
    pub(crate) notes: Vec<String>,
}

impl StagedBatch {
    pub(crate) fn render_card(&self) -> String {
        let mut out = format!(
            "✎ staged edit #{id} (proposed) · {n} file op(s)\n",
            id = self.id,
            n = self.actions.len()
        );
        for a in &self.actions {
            out.push_str(&format!("  · {}\n", a.summary_line()));
        }
        for n in &self.notes {
            out.push_str(&format!("  note: {n}\n"));
        }
        out.push_str(&format!(
            "Accept: resolve_edit id={} action=accept\nReject: resolve_edit id={} action=reject\n",
            self.id, self.id
        ));
        out
    }
}

#[derive(Default)]
struct StageStore {
    next_id: u32,
    batches: HashMap<u32, StagedBatch>,
}

impl StageStore {
    fn stage(
        &mut self,
        workspace: PathBuf,
        actions: Vec<StagedAction>,
        notes: Vec<String>,
    ) -> StagedBatch {
        if self.next_id == 0 {
            self.next_id = 1;
        }
        // Evict oldest if full.
        while self.batches.len() >= MAX_STAGED {
            if let Some(oldest) = self.batches.keys().copied().min() {
                self.batches.remove(&oldest);
            } else {
                break;
            }
        }
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        let batch = StagedBatch {
            id,
            workspace,
            actions,
            notes,
        };
        self.batches.insert(id, batch.clone());
        batch
    }

    fn take(&mut self, id: u32) -> Option<StagedBatch> {
        self.batches.remove(&id)
    }

    fn list(&self) -> Vec<&StagedBatch> {
        let mut v: Vec<_> = self.batches.values().collect();
        v.sort_by_key(|b| b.id);
        v
    }

    #[cfg(test)]
    fn clear(&mut self) {
        self.batches.clear();
        self.next_id = 1;
    }
}

fn store() -> std::sync::MutexGuard<'static, StageStore> {
    static S: OnceLock<Mutex<StageStore>> = OnceLock::new();
    let m = S.get_or_init(|| Mutex::new(StageStore::default()));
    match m.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    }
}

/// Build staged actions from hashline section plans + original bytes.
pub(crate) fn actions_from_hashline_plans(
    plans: Vec<(SectionPlan, Vec<u8>, Option<String>)>,
) -> (Vec<StagedAction>, Vec<String>) {
    let mut actions = Vec::new();
    let mut notes = Vec::new();
    for (plan, original, recovery) in plans {
        if let Some(r) = recovery {
            notes.push(r);
        }
        match plan {
            SectionPlan::Update { path, content } => {
                actions.push(StagedAction::Update {
                    path,
                    original,
                    content,
                });
            }
            SectionPlan::Remove { path } => {
                actions.push(StagedAction::Remove { path, original });
            }
            SectionPlan::Move { from, to, content } => {
                actions.push(StagedAction::Move {
                    from,
                    to,
                    original,
                    content,
                });
            }
        }
    }
    (actions, notes)
}

/// Stage a batch; returns the agent-facing card.
pub(crate) fn stage_batch(
    workspace: &Path,
    actions: Vec<StagedAction>,
    notes: Vec<String>,
) -> Result<String, String> {
    if actions.is_empty() {
        return Err("nothing to stage".into());
    }
    let batch = store().stage(workspace.to_path_buf(), actions, notes);
    Ok(batch.render_card())
}

pub(crate) fn list_staged() -> String {
    let s = store();
    let list = s.list();
    if list.is_empty() {
        return "no staged edits — apply_patch with stage=true to propose one\n".into();
    }
    let mut out = format!("{} staged edit(s):\n", list.len());
    for b in list {
        out.push_str(&b.render_card());
        out.push('\n');
    }
    out
}

pub(crate) fn reject(id: u32, reason: Option<&str>) -> Result<String, String> {
    let batch = store()
        .take(id)
        .ok_or_else(|| format!("unknown staged edit #{id}; resolve_edit action=list"))?;
    Ok(format!(
        "rejected staged edit #{id} ({} op(s)){}\n",
        batch.actions.len(),
        reason
            .filter(|r| !r.is_empty())
            .map(|r| format!(" · reason: {r}"))
            .unwrap_or_default()
    ))
}

/// Accept a staged batch: apply all actions transactionally to `workspace`
/// (must match the batch workspace root).
pub(crate) fn accept(id: u32, reason: Option<&str>) -> Result<String, String> {
    let batch = store()
        .take(id)
        .ok_or_else(|| format!("unknown staged edit #{id}; resolve_edit action=list"))?;
    let root = &batch.workspace;
    commit_actions(root, batch.actions).map(|receipts| {
        format!(
            "accepted staged edit #{id}{} · {}\n",
            reason
                .filter(|r| !r.is_empty())
                .map(|r| format!(" ({r})"))
                .unwrap_or_default(),
            receipts.join("; ")
        )
    })
}

/// Apply staged actions to disk (shared with live hashline commit path).
pub(crate) fn commit_actions(
    root: &Path,
    actions: Vec<StagedAction>,
) -> Result<Vec<String>, String> {
    enum Undo {
        Restore { path: String, content: Vec<u8> },
        RemoveCreated { path: String },
    }
    let mut undo: Vec<Undo> = Vec::new();
    let mut receipts: Vec<String> = Vec::new();

    let rollback = |undo: Vec<Undo>| {
        for step in undo.into_iter().rev() {
            match step {
                Undo::Restore { path, content } => {
                    let _ = confined_write(root, Path::new(&path), &content);
                }
                Undo::RemoveCreated { path } => {
                    let _ = confined_remove_file(root, Path::new(&path));
                }
            }
        }
    };

    for action in actions {
        match action {
            StagedAction::Update {
                path,
                original,
                content,
            } => {
                let path_msg = path.clone();
                let original_for_check = original.clone();
                let updated = content.into_bytes();
                let new_tag = crate::agent::hashline::record_snapshot(
                    &path,
                    &String::from_utf8_lossy(&updated),
                );
                let result = confined_edit(root, Path::new(&path), move |bytes| {
                    if bytes != original_for_check {
                        return Err(format!(
                            "staged {path_msg}: file changed since proposal; re-stage"
                        ));
                    }
                    Ok((updated, ()))
                });
                match result {
                    Ok(()) => {
                        undo.push(Undo::Restore {
                            path: path.clone(),
                            content: original,
                        });
                        receipts.push(format!("{path} · new tag #{new_tag}"));
                    }
                    Err(e) => {
                        rollback(undo);
                        return Err(format!("{e}; earlier staged ops rolled back"));
                    }
                }
            }
            StagedAction::Remove { path, original } => {
                match confined_read(root, Path::new(&path)) {
                    Ok(live) if live == original => {}
                    Ok(_) => {
                        rollback(undo);
                        return Err(format!(
                            "staged {path}: file changed since proposal; re-stage; earlier ops rolled back"
                        ));
                    }
                    Err(e) => {
                        rollback(undo);
                        return Err(format!("staged {path}: {e}; earlier ops rolled back"));
                    }
                }
                if let Err(e) = confined_remove_file(root, Path::new(&path)) {
                    rollback(undo);
                    return Err(format!("{e}; earlier staged ops rolled back"));
                }
                crate::agent::hashline::invalidate_snapshot(&path);
                undo.push(Undo::Restore {
                    path: path.clone(),
                    content: original,
                });
                receipts.push(format!("{path} · removed"));
            }
            StagedAction::Move {
                from,
                to,
                original,
                content,
            } => {
                match confined_read(root, Path::new(&from)) {
                    Ok(live) if live == original => {}
                    Ok(_) => {
                        rollback(undo);
                        return Err(format!(
                            "staged {from}: file changed since proposal; re-stage; earlier ops rolled back"
                        ));
                    }
                    Err(e) => {
                        rollback(undo);
                        return Err(format!("staged {from}: {e}; earlier ops rolled back"));
                    }
                }
                let bytes = content.into_bytes();
                let text = String::from_utf8_lossy(&bytes).into_owned();
                if let Err(e) = confined_create_new(root, Path::new(&to), &bytes) {
                    rollback(undo);
                    return Err(format!("{e}; earlier staged ops rolled back"));
                }
                undo.push(Undo::RemoveCreated { path: to.clone() });
                if let Err(e) = confined_remove_file(root, Path::new(&from)) {
                    let _ = confined_remove_file(root, Path::new(&to));
                    undo.pop();
                    rollback(undo);
                    return Err(format!("{e}; earlier staged ops rolled back"));
                }
                crate::agent::hashline::relocate_snapshot(&from, &to);
                let new_tag = crate::agent::hashline::record_snapshot(&to, &text);
                undo.push(Undo::Restore {
                    path: from.clone(),
                    content: original,
                });
                receipts.push(format!("{from} → {to} · new tag #{new_tag}"));
            }
        }
    }
    Ok(receipts)
}

#[cfg(test)]
pub(crate) fn clear_staged_for_test() {
    store().clear();
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/staged_edit__tests.rs"]
mod tests;
