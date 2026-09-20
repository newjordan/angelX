//! The swarm: a Mixture-of-Agents deep-think club over a single fast MoE.
//!
//! When handed an open-ended problem with no specific mandate, the swarm fans
//! out a first wave of `width` agents, measures local novelty/dissent, and only
//! buys more proposer width when the pool is still unstable and non-duplicative.
//! Large pools are coordinated by clustering representatives, pod judging, and
//! map-reduce synthesis before the final answer. A tight, well-specified
//! directive still short-circuits to one direct pass: that adaptive gate is the
//! *smart* in "smart harness".
//!
//! **Tool-capable driver (not a text-only oracle).** As the top-level brain the
//! swarm is a real agent: when the harness offers tools, [`SwarmClub::chat`]
//! routes through [`SwarmClub::drive_with_tools`], which runs a tool-offered pass
//! on the driver (`aggregate`) club and hands any tool call **straight back** to
//! the harness — the agent reads files, runs commands, delegates, gathers
//! evidence, loops. An armed or explicitly summoned formation runs one planning
//! preflight before the first tool action by default (with an explicit opt-out);
//! its explicitly unverified advisory is replayed into every coordinator hop so
//! the provider prefix remains stable.
//! The *final answer* is also optionally amplified by the MoA fan-out, and only
//! for reasoning-dominant / research questions
//! ([`wants_synthesis`]); a tool-grounded action answer is already backed by real
//! output, so it is normally returned as-is rather than re-synthesized text-only. The MoA
//! *stages* (proposer/judge/verify/aggregate) remain text-only by design — they
//! reason over evidence the tool loop already surfaced. With no tools offered
//! (`respond`, benches, control-bench) the swarm is the pure MoA think it always
//! was. `ANGEL_SOTA_MOA_ALWAYS` amplifies every turn.
//!
//! **Why gemma4-on-Spark exclusively.** A 26B / ~4B-active NVFP4 MoE on the
//! GB10's unified memory makes a dozen concurrent generations cost about one
//! dense reply, so the fan-out is practically free here and nowhere else in the
//! fleet. The swarm wraps any [`Club`] (so it's testable over a mock), but in the
//! [`Bag`](crate::agent::club::Bag) the inner club is the gemma4 vLLM endpoint.
//!
//! **Concurrency.** The club layer is blocking (`ureq`), so a layer fans out via
//! [`std::thread::scope`]: `width` threads each make one blocking chat call, and
//! vLLM continuous-batches the simultaneous requests server-side. No async
//! runtime is involved — this slots straight into the harness's worker thread.
//!
//! **Autoresearch knobs (optional, composable).** Each stage below switches on
//! independently and they stack into one pipeline; `ANGEL_SWARM_MAX` turns the
//! whole stack on:
//! - `RESEARCH` — web-ground the proposers from a live search (best-effort).
//! - `REFLECT`  — a critic critiques the drafts between layers; the critique is
//!   fed back so the next layer evolves to fix it (GEPA-style reflection).
//! - `JUDGE`    — score the drafts and keep only the top-k before synthesis. A
//!   `JUDGE_PANEL` of independent reviewers can vote: each scores blind, the
//!   per-draft median ranks (peer-review-style, anchor-resistant). `JUDGE_DIMS`
//!   scores weighted dimensions (correctness/insight/rigor/novelty → one composite)
//!   instead of a single 0–10.
//! - `SAMPLES`  — self-consistency: N final syntheses, a chooser picks the best.
//! - `VERIFY`   — adversarial check → revise loop on the final answer. `VERIFY_GUARD`
//!   keeps the best-scored answer across rounds so a revision can never regress.
//! - `CITE`     — when research-grounded, hold the final answer's claims to the
//!   gathered sources: attribute what they support, hedge or drop what they don't.
//! - `HEDGE`    — fold a calibrated-claims "hedge ladder" into the base voice so
//!   every stage ties claim strength to evidence strength.

pub(crate) use crate::agent::club::{
    ChatMsg, ChatRole, Club, ClubReply, StreamDelta, TokenUsage, ToolDef,
};
pub(crate) use crate::agent::swarm_delegate::{
    Router, dedupe_requests, evidence_block, parse_requests, strip_blocks,
};
pub(crate) use std::sync::Arc;
pub(crate) use std::sync::atomic::{AtomicBool, Ordering};

mod angles;
mod club;
mod dissent;
pub(crate) mod ledger;
mod parse;
mod pipeline;
mod prompts;
mod routing;
mod scale;
mod waves;

pub(crate) use angles::*;
pub(crate) use club::*;
pub(crate) use dissent::*;
pub(crate) use parse::*;
pub(crate) use prompts::*;
pub(crate) use routing::*;
pub(crate) use scale::*;
pub(crate) use waves::*;

// ---------------------------------------------------------------------------
// Tests — offline, over a counting mock club (no network)
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/swarm__tests.rs"]
mod tests;
