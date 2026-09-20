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

/// One exclusive resource a child claims while it runs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Resource {
    /// The shared workspace root: a child that writes here claims the whole tree —
    /// today's `tools=code` seat, and any child without a checkout of its own.
    Workspace(PathBuf),
    /// A child's own checkout (a delegated git worktree). Worktrees live beside the
    /// repository, not inside it, so one of these still commutes with a workspace
    /// writer.
    Worktree(PathBuf),
    /// A single claimed path — a file a child was told to publish, a scratch dir.
    Path(PathBuf),
    /// A named git ref (a delegate's `angel/<run>` branch): two children may not
    /// create, move, or delete the same ref.
    Ref(String),
    /// A TCP port a child's service binds.
    Port(u16),
}

impl Resource {
    /// How a refusal names this resource.
    pub(crate) fn label(&self) -> String {
        match self {
            Resource::Workspace(path) => format!("the shared workspace {}", path.display()),
            Resource::Worktree(path) => format!("the worktree {}", path.display()),
            Resource::Path(path) => format!("the path {}", path.display()),
            Resource::Ref(name) => format!("the ref {name}"),
            Resource::Port(port) => format!("port {port}"),
        }
    }

    /// Do two resources exclude each other? Paths conflict when one contains the
    /// other — the same rule the tool-call gate uses, so there is one definition of
    /// overlapping paths in the harness, not two.
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
            // Distinct kinds with nothing in common (a port is not a path, and a
            // worktree beside the workspace is not inside it).
            _ => false,
        }
    }
}

impl std::fmt::Display for Resource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label())
    }
}

/// One child's declared footprint: the seat it is judged under, and what it claims.
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

    /// A child that writes in the one shared checkout.
    pub(crate) fn workspace_writer(seat: impl Into<String>, root: impl Into<PathBuf>) -> Self {
        Self::new(seat).claim(Resource::Workspace(root.into()))
    }

    /// A child with a checkout of its own, working on its own ref.
    pub(crate) fn worktree_child(
        seat: impl Into<String>,
        root: impl Into<PathBuf>,
        git_ref: impl Into<String>,
    ) -> Self {
        Self::new(seat)
            .claim(Resource::Worktree(root.into()))
            .claim(Resource::Ref(git_ref.into()))
    }

    /// The first resource both children claim — the overlap *witness*.
    pub(crate) fn overlap(&self, other: &Self) -> Option<Resource> {
        self.resources
            .iter()
            .find(|mine| other.resources.iter().any(|theirs| mine.conflicts(theirs)))
            .cloned()
    }

    /// May these two run at once (and be reverted in either order)?
    pub(crate) fn commutes_with(&self, other: &Self) -> bool {
        self.overlap(other).is_none()
    }
}

/// A wave decomposition: every pair inside a wave commutes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Plan {
    pub(crate) waves: Vec<Vec<usize>>,
}

impl Plan {
    /// One wave holding every child: they all commute, so they may run together and
    /// their inverses may be applied in any order.
    pub(crate) fn independent(&self) -> bool {
        self.waves.len() <= 1
    }

    /// The order the inverses are applied in: last wave first, and within a wave any
    /// order works (its members commute), so reverse insertion order is the
    /// deterministic choice.
    pub(crate) fn revert_order(&self) -> Vec<usize> {
        let mut order: Vec<usize> = self.waves.iter().flatten().copied().collect();
        order.reverse();
        order
    }
}

/// Greedy wave decomposition: each child joins the first wave it commutes with
/// everywhere, else opens a new one. Members of a wave therefore commute pairwise,
/// and a later wave only holds children that collided with an earlier wave.
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

/// The first pair that cannot run together, named with the resource they share.
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

/// The refusal receipt for a set of children that do not all commute, naming the
/// colliding pair and the resource — `None` when the whole set may run at once.
pub(crate) fn refusal(children: &[ChildFootprint]) -> Option<String> {
    let (left, right, resource) = first_conflict(children)?;
    // The way out depends on what collided: a shared *checkout* has a ready answer
    // (a delegated worktree is disjoint from it), a shared ref or port does not.
    let advice = match resource {
        Resource::Workspace(_) => {
            "run them one at a time, or use `delegate` (git-worktree isolated) for parallel \
             implementation — a delegated checkout is disjoint from the shared workspace"
        }
        _ => "run them one at a time, or give the second child a resource of its own",
    };
    Some(format!(
        "child '{}' and child '{}' both claim {resource}; they do not commute — {advice}",
        children[left].seat(),
        children[right].seat(),
    ))
}

#[cfg(test)]
#[path = "../../../tests/cockpit/harness/independence__tests.rs"]
mod tests;
