//! Edit outcomes for the agent. A file tool that lands inside the live module
//! root returns, with its own result, what the loader made of the edit in the
//! `/reload status` format. A verdict that settles after the activation window
//! rides the next tool result, or the next turn's context.
use super::*;

const ENVELOPE: &str = "[live-module/v1]";

pub(super) struct Binding {
    /// The root as `/reload status` prints it.
    shown: PathBuf,
    /// The root edits are matched against.
    root: PathBuf,
    board: Arc<angel_live::Board>,
}

impl ToolRegistry {
    pub(crate) fn set_live_outcomes(&self, binding: Option<(PathBuf, Arc<angel_live::Board>)>) {
        *self.live_outcomes.lock().unwrap_or_else(|p| p.into_inner()) =
            binding.map(|(shown, board)| {
                Arc::new(Binding {
                    root: shown.canonicalize().unwrap_or_else(|_| shown.clone()),
                    shown,
                    board,
                })
            });
    }

    /// The module root this session's loader watches, when one is running.
    pub(crate) fn live_root(&self) -> Option<PathBuf> {
        self.live_binding().map(|binding| binding.shown.clone())
    }

    fn live_binding(&self) -> Option<Arc<Binding>> {
        self.live_outcomes
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// Open a watch, before the write, on every live module a file tool is
    /// about to change. Other tools and paths outside the root open nothing.
    pub(super) fn live_watch(
        &self,
        name: &str,
        args: &Value,
    ) -> Option<(Arc<angel_live::Board>, u64)> {
        let binding = self.live_binding()?;
        let mut ids = std::collections::BTreeSet::new();
        crate::knowledge::cut::for_each_mutation_target_path(name, args, |path| {
            if let Ok(resolved) = self.workspace_boundary().safe_path(path)
                && let Ok(inside) = resolved.strip_prefix(&binding.root)
            {
                let mut parts = inside.components();
                if let (Some(std::path::Component::Normal(id)), Some(_)) =
                    (parts.next(), parts.next())
                    && let Some(id) = id.to_str()
                    && id
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
                {
                    ids.insert(id.to_string());
                }
            }
            false
        });
        if ids.is_empty() {
            return None;
        }
        let ids: Vec<String> = ids.into_iter().collect();
        let watch = binding.board.watch(&ids);
        Some((Arc::clone(&binding.board), watch))
    }

    /// Append what the loader made of this call's edit, and any verdict that
    /// settled since an earlier one, to a successful result.
    pub(super) fn live_attach(
        &self,
        watch: Option<(Arc<angel_live::Board>, u64)>,
        mut result: Result<String, String>,
        cancel: Option<&AtomicBool>,
    ) -> Result<String, String> {
        let Ok(text) = &mut result else {
            if let Some((board, watch)) = watch {
                board.forget(watch);
            }
            return result;
        };
        let Some(binding) = self.live_binding() else {
            return result;
        };
        let mut outcomes = binding.board.take();
        if let Some((board, watch)) = watch
            && let Some(settled) = board.settle(watch, angel_live::WINDOW, &|| {
                cancel.is_some_and(|flag| flag.load(Ordering::Relaxed))
            })
        {
            outcomes.extend(settled);
        }
        if !outcomes.is_empty() {
            if !text.ends_with('\n') {
                text.push('\n');
            }
            text.push_str(&envelope(&binding.shown, &outcomes));
        }
        result
    }

    /// Verdicts no tool result has carried yet, for the next turn's context.
    pub(crate) fn take_live_outcomes(&self) -> Option<String> {
        let binding = self.live_binding()?;
        let outcomes = binding.board.take();
        (!outcomes.is_empty()).then(|| envelope(&binding.shown, &outcomes))
    }
}

fn envelope(shown: &Path, outcomes: &[angel_live::Outcome]) -> String {
    format!(
        "{ENVELOPE}\n{}\n",
        angel_live::Outcome::text(shown, outcomes)
    )
}
