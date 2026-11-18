//! The directive-mandate decisions of a swarm turn: what the classifier's
//! verdict means, whether the advisor reviews the final answer, whether that
//! review annotates it, whether the turn takes the single direct pass, and
//! whether the live-research route engages.
//!
//! The decisions themselves live in
//! `cockpit/live/swarm-mandate/swarm-mandate.bend`: Rust keeps every text test
//! (case-folding a verdict, the substring tests, the answer trim, the advisor's
//! `BLOCK:`/`NOTE:` parse) and hands the leg booleans. The decisions they
//! replaced answer only as the parity oracle in the test module.

use super::Mandate;
use crate::agent::harness::live_policy;

const LEG: &str = "swarm-mandate";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct VerdictFacts {
    pub(crate) call_ok: bool,
    pub(crate) saw_tight: bool,
    pub(crate) saw_open: bool,
}

impl VerdictFacts {
    pub(crate) fn of(reply: Option<&str>) -> Self {
        let Some(reply) = reply else {
            return Self {
                call_ok: false,
                saw_tight: false,
                saw_open: false,
            };
        };
        let verdict = reply.to_ascii_uppercase();
        Self {
            call_ok: true,
            saw_tight: verdict.contains("TIGHT"),
            saw_open: verdict.contains("OPEN"),
        }
    }

    pub(crate) fn check(self) -> impl Fn(&MandateOut) -> Result<(), String> {
        move |out: &MandateOut| match out.mandate {
            Mandate::Tight if self.call_ok && self.saw_tight && !self.saw_open => Ok(()),
            Mandate::Tight => Err("only an unambiguous TIGHT verdict takes the single pass".into()),
            Mandate::Open => Ok(()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
pub(crate) struct MandateOut {
    pub(crate) mandate: Mandate,
}

pub(crate) fn mandate_input(f: VerdictFacts) -> serde_json::Value {
    serde_json::json!({
        "kind": "classify",
        "call_ok": f.call_ok,
        "saw_tight": f.saw_tight,
        "saw_open": f.saw_open,
    })
}

pub(crate) fn mandate_with(f: VerdictFacts) -> (Mandate, live_policy::LegSource) {
    let (out, source) = live_policy::decide_shipped(LEG, &mandate_input(f), f.check());
    (out.mandate, source)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AdviseFacts {
    pub(crate) enabled: bool,
    pub(crate) blank: bool,
    pub(crate) annotated: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
pub(crate) struct AdviseOut {
    pub(crate) advise: bool,
}

pub(crate) fn advise_input(f: AdviseFacts) -> serde_json::Value {
    serde_json::json!({
        "kind": "advise",
        "enabled": f.enabled,
        "blank": f.blank,
        "annotated": f.annotated,
    })
}

pub(crate) fn advise_with(f: AdviseFacts) -> (bool, live_policy::LegSource) {
    let check = move |out: &AdviseOut| match (f.enabled, out.advise) {
        (false, false) | (true, _) => Ok(()),
        (false, true) => Err("the advisor is switched off".into()),
    };
    let (out, source) = live_policy::decide_shipped(LEG, &advise_input(f), check);
    (out.advise, source)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AnnotateFacts {
    pub(crate) clear: bool,
    pub(crate) has_annotation: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
pub(crate) struct AnnotateOut {
    pub(crate) annotate: bool,
}

pub(crate) fn annotate_input(f: AnnotateFacts) -> serde_json::Value {
    serde_json::json!({
        "kind": "annotate",
        "clear": f.clear,
        "has_annotation": f.has_annotation,
    })
}

pub(crate) fn annotate_with(f: AnnotateFacts) -> (bool, live_policy::LegSource) {
    let check = move |out: &AnnotateOut| match (f.clear, out.annotate) {
        (true, false) | (false, _) => Ok(()),
        (true, true) => Err("a clear review must not annotate".into()),
    };
    let (out, source) = live_policy::decide_shipped(LEG, &annotate_input(f), check);
    (out.annotate, source)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PassFacts {
    pub(crate) smart_route: bool,
    pub(crate) research_route: bool,
    pub(crate) fastpath: bool,
    pub(crate) tight: bool,
    pub(crate) classify_open: bool,
    pub(crate) deliberation: bool,
    pub(crate) always: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
pub(crate) struct PassOut {
    pub(crate) direct: bool,
}

pub(crate) fn pass_input(f: PassFacts) -> serde_json::Value {
    serde_json::json!({
        "kind": "pass",
        "smart_route": f.smart_route,
        "research_route": f.research_route,
        "fastpath": f.fastpath,
        "tight": f.tight,
        "classify_open": f.classify_open,
        "deliberation": f.deliberation,
        "always": f.always,
    })
}

impl PassOut {
    pub(crate) fn check(f: PassFacts) -> impl Fn(&PassOut) -> Result<(), String> {
        move |out: &PassOut| {
            if out.direct && f.always {
                return Err("an always-on swarm never takes the direct pass".into());
            }
            if out.direct && f.research_route {
                return Err("a research-routed turn always deliberates".into());
            }
            Ok(())
        }
    }
}

pub(crate) fn pass_with(f: PassFacts) -> (bool, live_policy::LegSource) {
    let (out, source) = live_policy::decide_shipped(LEG, &pass_input(f), PassOut::check(f));
    (out.direct, source)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ResearchFacts {
    pub(crate) wants_live: bool,
    pub(crate) research_enabled: bool,
    pub(crate) researcher_present: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
pub(crate) struct ResearchOut {
    pub(crate) research_route: bool,
}

pub(crate) fn research_route_input(f: ResearchFacts) -> serde_json::Value {
    serde_json::json!({
        "kind": "research_route",
        "wants_live": f.wants_live,
        "research_enabled": f.research_enabled,
        "researcher_present": f.researcher_present,
    })
}

pub(crate) fn research_route_with(f: ResearchFacts) -> (bool, live_policy::LegSource) {
    let check = move |out: &ResearchOut| match (f.wants_live, out.research_route) {
        (false, false) | (true, _) => Ok(()),
        (false, true) => Err("a turn without a live ask must not route research".into()),
    };
    let (out, source) = live_policy::decide_shipped(LEG, &research_route_input(f), check);
    (out.research_route, source)
}
