//! ⠼ d3456 — the hop advisories: what the 0.1.6 harness said to a model at the
//! moments that decide a run, in the words it said them.
//!
//! Every page is the advisory's own sentence, second person and concrete, as
//! it stood in `nudges.rs` at 0.1.6 (the `[harness-telemetry]` tag aside). The
//! routes are voiced (`book::VOICED`): the legend introduces one with every
//! page, and each rides its own turn, never the tail of a tool result. On a
//! DeepSeek polyglot run the 0.1.6 build solved 26 of 26 where a build that had
//! compressed these into terse stamps lost most of its hard tasks to probe
//! loops, so the advice keeps its force and its concrete instructions.
//!
//! None of them stops a turn or denies a call. The 0.1.6 stops that some of
//! them once preceded are gone; only the advice remains.
//!
//! | route | 0.1.6 advisory | fires |
//! |-------|----------------|-------|
//! | `⠼⠁` | post-edit logic | the turn's first successful edit, once |
//! | `⠼⠃` | green verifier | a full verifier passed on changed code, once until the green goes stale |
//! | `⠼⠉` | final mile | a bounded turn near its horizon with a changed workspace |
//! | `⠼⠙` | first write | operator-armed inspection budget spent with no edit |
//! | `⠼⠑` | no edit | an answer that claims progress with no edit (stop checkpoint) |
//! | `⠼⠋` | weak verification | a green that ran only tests the model wrote |
//! | `⠼⠛` | mutation thrash | the same edit issued three times |
//! | `⠼⠓` | peripheral fan-out | four edits under docs or fixtures, none in the code |
//! | `⠼⠊` | error cascade | every call failed for six hops running |

use super::{Primary, Route, Sub};
use crate::agent::harness::*;
use std::collections::{HashMap, HashSet};

pub(crate) const CELL: char = '⠼';
pub(crate) const POST_EDIT: Route = Route::new(CELL, '⠁');
pub(crate) const GREEN: Route = Route::new(CELL, '⠃');
pub(crate) const FINAL_MILE: Route = Route::new(CELL, '⠉');
pub(crate) const FIRST_WRITE: Route = Route::new(CELL, '⠙');
pub(crate) const NO_EDIT: Route = Route::new(CELL, '⠑');
pub(crate) const WEAK: Route = Route::new(CELL, '⠋');
pub(crate) const THRASH: Route = Route::new(CELL, '⠛');
pub(crate) const FANOUT: Route = Route::new(CELL, '⠓');
pub(crate) const CASCADE: Route = Route::new(CELL, '⠊');

/// The same edit issued this many times: `ANGEL_MUTATION_THRASH_NUDGE=3`, the
/// value the 0.1.6 task defaults set.
pub(crate) const THRASH_REPEATS: usize = 3;
/// Edits under docs or fixtures with none in the code: `ANGEL_PERIPHERAL_MUTATION_NUDGE=4`.
pub(crate) const FANOUT_EDITS: usize = 4;
/// Hops a bounded turn reserves for its final mile once it has edited. The 0.1.6
/// task defaults: six for a rapid bounded turn, four otherwise.
pub(crate) const FINAL_MILE_HOPS: usize = 4;
pub(crate) const RAPID_FINAL_MILE_HOPS: usize = 6;
/// The page of `⠼⠉` an inspection-only hop meets again: the last sentence.
pub(crate) const FINAL_MILE_AGAIN: usize = 4;

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "advisories",
    surface: "the hop advisories: review an edit, a green verifier, the final mile, the first write, weak checks, thrash, fan-out, an error cascade",
    subs: &[
        Sub {
            route: POST_EDIT,
            name: "post-edit",
            signal: "the first edit landed; review it before testing",
            action: "",
            ideas: "",
            pages: &[
                "Review the edit logically before testing: trace the state transitions, invariants, cleanup/empty cases, and error paths implied by the task.",
                "Prefer the code that *implements or emits* the behavior (library/pkg/src machinery) over generated testdata, docs, or example trees.",
                "If the bug names multiple surfaces (code + config/markdown/model file), check whether each still needs a change.",
                "Respect explicit no-build and externally delegated verification instructions; in those cases finish with the candidate and a truthful pending-verification note.",
                "Otherwise, if the edit already covers them, run one smallest relevant verifier from a *pre-existing* project test entry point and finish.",
                "Do not invent new tests as proof, stack broader checks without a concrete diagnostic, or thrash the same edit.",
            ],
        },
        Sub {
            route: GREEN,
            name: "green",
            signal: "a full project verifier passed on this workspace",
            action: "",
            ideas: "",
            pages: &[
                "GREEN VERIFIER. A full project verifier just passed on this workspace.",
                "Use this result as evidence.",
                "Complete every remaining requested deliverable, including additional edits or distinct checks when needed, then give the final answer summarizing the work.",
            ],
        },
        Sub {
            route: FINAL_MILE,
            name: "final-mile",
            signal: "the bounded turn is near its horizon and the workspace has changed",
            action: "",
            ideas: "",
            pages: &[
                "FINAL-MILE BUDGET ACTIVE. The workspace has changed and the bounded turn is near its horizon.",
                "Stop broad inspection.",
                "Follow the operator's verification arrangement: if local builds/checks are prohibited or verification is delegated to another host or evaluator, preserve the candidate and report local verification as pending; do not create build manifests or run setup to bypass that arrangement.",
                "Otherwise run the smallest relevant verifier now; if it fails, make only the concrete fix supported by its diagnostics, verify again, then return an honest final answer.",
                "Do not spend the remaining calls re-reading known context.",
            ],
        },
        Sub {
            route: FIRST_WRITE,
            name: "first-write",
            signal: "the armed inspection budget is spent and nothing has been edited",
            action: "",
            ideas: "",
            pages: &[
                "ACTIONABLE CANDIDATE PROGRESS. Inspection has not yet produced candidate progress.",
                "The next tool must mutate the candidate, run the narrow validation implied by current evidence, or report the concrete blocker.",
                "Do not wrap another source read in a build/check.",
                "Submission is never implied by this hop guard.",
            ],
        },
        Sub {
            route: NO_EDIT,
            name: "no-edit",
            signal: "the answer claims progress but no source file was changed this turn",
            action: "",
            ideas: "",
            pages: &[
                "NO WORKSPACE MUTATION. You claimed progress or completion but no source file was changed this turn.",
                "If the bug is real, make the smallest edit (or a dependency bump in go.mod / package.json / Cargo.toml when the fix is an upstream library version).",
                "If you truly cannot edit, name the concrete blocker.",
                "A bare claim that it is already fixed is not enough.",
            ],
        },
        Sub {
            route: WEAK,
            name: "weak-verification",
            signal: "the green check only ran tests the model wrote this turn",
            action: "",
            ideas: "",
            pages: &[
                "WEAK VERIFICATION. The green check only ran tests or files you created or heavily rewrote this turn.",
                "That is not evidence the task's real acceptance tests pass.",
                "Prefer pre-existing project test entry points (package test suites, named cases already in the tree).",
                "Keep verifying against those, or disclose the residual risk honestly.",
            ],
        },
        Sub {
            route: THRASH,
            name: "mutation-thrash",
            signal: "the same edit was issued several times",
            action: "",
            ideas: "",
            pages: &[
                "MUTATION THRASH. You re-issued the same edit signature multiple times (same path and old/new payload, or the same non-unique short snippet).",
                "Stop replaying it.",
                "If the tool said 'old is not unique', include the enclosing function or more unique context in `old`.",
                "If the edit already applied, run a verifier or change approach.",
                "Do not spend the remaining horizon re-applying an identical patch.",
            ],
        },
        Sub {
            route: FANOUT,
            name: "peripheral-fanout",
            signal: "edits landed under docs or fixtures with none in the implementing code",
            action: "",
            ideas: "",
            pages: &[
                "PERIPHERAL FAN-OUT. Several edits landed under docs/, testdata/, fixtures, or generated examples without a corresponding change in the implementing library (src/, pkg/, lib/, core/).",
                "Find the code that *produces* those artifacts and fix it at the source instead of hand-patching every generated copy.",
            ],
        },
        Sub {
            route: CASCADE,
            name: "error-cascade",
            signal: "every call failed for six hops in a row",
            action: "",
            ideas: "",
            pages: &[
                "ERROR CASCADE REDIRECTION: Every tool call in the last 6 hops failed.",
                "Stop repeating failing commands.",
                "Read the compiler diagnostics above and rewrite the file cleanly using `write_file` instead of accumulating micro-patches.",
            ],
        },
    ],
};

/// The final-mile reserve for a turn: hops kept back once it has edited.
pub(crate) fn final_mile_reserve(pace: TaskPace, bounded: bool) -> usize {
    match pace {
        TaskPace::Rapid if bounded => RAPID_FINAL_MILE_HOPS,
        _ => FINAL_MILE_HOPS,
    }
}

/// Whether the final mile opens now: a bounded turn, edited, within `reserve`
/// hops of its cap.
pub(crate) fn should_activate_final_mile(
    max_hops: Option<usize>,
    completed_hops: usize,
    reserve_hops: usize,
    mutation_seen: bool,
    already_active: bool,
) -> bool {
    reserve_hops > 0
        && mutation_seen
        && !already_active
        && max_hops.is_some_and(|max| max.saturating_sub(completed_hops) <= reserve_hops)
}

/// A batch that is inspection alone while the final mile is open and a
/// verifier is still owed: the advice is given again. The calls still run.
pub(crate) fn rejects_inspection(
    active: bool,
    verification_outstanding: bool,
    calls: &[ToolCall],
) -> bool {
    active
        && verification_outstanding
        && !calls
            .iter()
            .any(|call| is_mutation_call(call) || is_verification_call(call))
}

/// `ANGEL_FIRST_WRITE_CALLS`: inspection calls before the first edit at which
/// `⠼⠙` is given. Off (0) unless the operator arms it: an evidence audit
/// legitimately reads for a long time.
pub(crate) fn first_write_limit() -> usize {
    env_usize("ANGEL_FIRST_WRITE_CALLS", 0)
}

/// `ANGEL_NO_EDIT_ANSWER_GUARD`: off unless the operator arms it, and only
/// with a first-write limit, as at 0.1.6.
pub(crate) fn no_edit_guard_armed() -> bool {
    env_flag("ANGEL_NO_EDIT_ANSWER_GUARD", false) && first_write_limit() > 0
}

/// `ANGEL_POST_EDIT_LOGIC_REVIEW`: on unless the operator turns it off.
pub(crate) fn post_edit_review_enabled() -> bool {
    env_flag("ANGEL_POST_EDIT_LOGIC_REVIEW", true)
}

/// The same edit issued again and again, by its identity: the path and the
/// old/new payload of each edit (a `multi_edit` batch is its edits), so a
/// snippet that only differs by how a batch packed it still collides.
#[derive(Default)]
pub(crate) struct MutationThrash {
    seen: HashMap<String, usize>,
}

impl MutationThrash {
    /// The edit calls of a batch, counted; the route when one edit reaches
    /// [`THRASH_REPEATS`], once per edit.
    pub(crate) fn observe(&mut self, calls: &[ToolCall]) -> Option<super::Raise> {
        let mut thrashed = None;
        for call in calls {
            for signature in mutation_edit_signatures(call) {
                let count = self.seen.entry(signature.clone()).or_default();
                *count += 1;
                if *count == THRASH_REPEATS && thrashed.is_none() {
                    thrashed = Some(super::Raise::new(
                        THRASH,
                        format!(
                            "×{THRASH_REPEATS}: {}",
                            signature.chars().take(240).collect::<String>()
                        ),
                    ));
                }
            }
        }
        thrashed
    }
}

/// Edits that landed only under docs, fixtures and generated examples.
#[derive(Default)]
pub(crate) struct PeripheralFanout {
    peripheral: HashSet<String>,
    core_hit: bool,
    raised: bool,
}

impl PeripheralFanout {
    /// A successful edit's target paths; the route once [`FANOUT_EDITS`]
    /// distinct peripheral paths were edited and no other path was.
    pub(crate) fn observe(&mut self, call: &ToolCall) -> Option<super::Raise> {
        crate::knowledge::cut::for_each_mutation_target_path(&call.name, &call.args, |path| {
            if is_peripheral_mutation_path(path) {
                self.peripheral.insert(path.to_string());
            } else if !path.trim().is_empty() {
                self.core_hit = true;
            }
            false
        });
        (!self.raised && !self.core_hit && self.peripheral.len() >= FANOUT_EDITS).then(|| {
            self.raised = true;
            let mut paths = self.peripheral.iter().cloned().collect::<Vec<_>>();
            paths.sort();
            super::Raise::new(FANOUT, paths.join("\n"))
        })
    }
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/harness/book__advisories_tests.rs"]
mod tests;
