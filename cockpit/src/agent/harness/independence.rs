//! Independence gate for parallel children — arXiv 2608.25512 §3.1.1 (Def 42/44,
//! Thm 43/45), ledger T5.
//!
//! `footprint` answers "may these two *tool calls* run at once" inside one turn,
//! from the paths a call touches. This answers the same question one level up, for
//! *children* — delegated subagents, formation seats, worktree contributors — which
//! own exclusive resources (a checkout, a ref, a path, a port) rather than a single
//! path.
//!
//! Two children whose footprints do not intersect **commute**: they may run
//! concurrently and their inverses may then be applied in *any* order. Children
//! that share a resource do not commute: they are serialized, and their inverses
//! must be applied in reverse. The point of doing this from *declared* footprints
//! instead of a hardcoded rule is that the answer is auditable — a refusal names the
//! resource that collided, and a caller that gives a child its own checkout gets a
//! different answer without editing this file.
//!
//! (`swarm_compile`'s reviewer *identity* independence is a different property —
//! epistemic, about who reviewed what; this one is about what two children touch.)

#![allow(dead_code)]

use std::collections::BTreeSet;
use std::path::PathBuf;

use super::footprint::paths_overlap;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Resource {
    Workspace(PathBuf),
    Worktree(PathBuf),
    Path(PathBuf),
    Ref(String),
    Port(u16),
}

impl Resource {
    pub(crate) fn label(&self) -> String {
        match self {
            Resource::Workspace(path) => format!("the shared workspace {}", path.display()),
            Resource::Worktree(path) => format!("the worktree {}", path.display()),
            Resource::Path(path) => format!("the path {}", path.display()),
            Resource::Ref(name) => format!("the ref {name}"),
            Resource::Port(port) => format!("port {port}"),
        }
    }

    fn conflicts(&self, other: &Resource) -> bool {
        use Resource::*;
        match (self, other) {
            (Workspace(a), Workspace(b)) => paths_overlap(a, b),
            (Workspace(root), Worktree(path))
            | (Worktree(path), Workspace(root))
            | (Workspace(root), Path(path))
            | (Path(path), Workspace(root)) => paths_overlap(root, path),
            (Worktree(a), Worktree(b)) => paths_overlap(a, b),
            (Worktree(root), Path(path)) | (Path(path), Worktree(root)) => {
                paths_overlap(root, path)
            }
            (Path(a), Path(b)) => paths_overlap(a, b),
            (Ref(a), Ref(b)) => a == b,
            (Port(a), Port(b)) => a == b,
            _ => false,
        }
    }
}

impl std::fmt::Display for Resource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ChildFootprint {
    seat: String,
    resources: BTreeSet<Resource>,
}

impl ChildFootprint {
    pub(crate) fn new(seat: impl Into<String>) -> Self {
        Self {
            seat: seat.into(),
            resources: BTreeSet::new(),
        }
    }

    pub(crate) fn seat(&self) -> &str {
        &self.seat
    }

    pub(crate) fn resources(&self) -> &BTreeSet<Resource> {
        &self.resources
    }

    pub(crate) fn claim(mut self, resource: Resource) -> Self {
        self.resources.insert(resource);
        self
    }

    pub(crate) fn workspace_writer(seat: impl Into<String>, root: impl Into<PathBuf>) -> Self {
        Self::new(seat).claim(Resource::Workspace(root.into()))
    }

    pub(crate) fn worktree_child(
        seat: impl Into<String>,
        root: impl Into<PathBuf>,
        git_ref: impl Into<String>,
    ) -> Self {
        Self::new(seat)
            .claim(Resource::Worktree(root.into()))
            .claim(Resource::Ref(git_ref.into()))
    }

    pub(crate) fn overlap(&self, other: &Self) -> Option<Resource> {
        self.resources
            .iter()
            .find(|mine| other.resources.iter().any(|theirs| mine.conflicts(theirs)))
            .cloned()
    }

    pub(crate) fn commutes_with(&self, other: &Self) -> bool {
        self.overlap(other).is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Plan {
    pub(crate) waves: Vec<Vec<usize>>,
}

impl Plan {
    pub(crate) fn independent(&self) -> bool {
        self.waves.len() <= 1
    }

    pub(crate) fn revert_order(&self) -> Vec<usize> {
        let mut order: Vec<usize> = self.waves.iter().flatten().copied().collect();
        order.reverse();
        order
    }
}

pub(crate) fn plan(children: &[ChildFootprint]) -> Plan {
    let mut waves: Vec<Vec<usize>> = Vec::new();
    for (index, child) in children.iter().enumerate() {
        let wave = waves.iter_mut().find(|wave| {
            wave.iter()
                .all(|member| children[*member].commutes_with(child))
        });
        match wave {
            Some(wave) => wave.push(index),
            None => waves.push(vec![index]),
        }
    }
    Plan { waves }
}

pub(crate) fn first_conflict(children: &[ChildFootprint]) -> Option<(usize, usize, Resource)> {
    for (left, child) in children.iter().enumerate() {
        for (right, other) in children.iter().enumerate().skip(left + 1) {
            if let Some(resource) = child.overlap(other) {
                return Some((left, right, resource));
            }
        }
    }
    None
}

pub(crate) fn refusal(children: &[ChildFootprint]) -> Option<String> {
    let (left, right, resource) = first_conflict(children)?;
    let advice = match resource {
        Resource::Workspace(_) => crate::agent::harness::book::d56_replies::SHARED_WORKSPACE,
        _ => crate::agent::harness::book::d56_replies::SHARED_RESOURCE,
    };
    Some(format!(
        "child '{}' and child '{}' both claim {resource}; they do not commute\n{}",
        children[left].seat(),
        children[right].seat(),
        advice.cells(),
    ))
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/independence__tests.rs"]
mod tests;
