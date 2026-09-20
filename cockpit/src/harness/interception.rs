//! Interception — policy as context metadata, consulted when a tool is *used*.
//!
//! Transfer of arXiv 2608.25512 §3.2.3 (Def 26/27) and §6.3: a dependency can
//! carry policy metadata that the runtime consults at **invocation** rather than at
//! resolution. Two consequences the ledger asked for:
//!
//! * the same registered tool can be permitted in one context and denied in
//!   another (the root keeps write access; an attenuated seat does not), with the
//!   provider untouched — nothing is re-registered to change a policy;
//! * a grant can be tightened mid-session and take effect on the next call, with
//!   no reload, because the table is read per call.
//!
//! Rules are **nominal** (a tool name, or the network-facing name a seat names).
//! They are deliberately not inferred from a "mutates the workspace" flag, because
//! Angel has no such declaration on `ToolDef` — inventing one here would create a
//! second write list that could drift from the sandbox's and the audit trail's,
//! which is the drift this design exists to avoid. A seat that must be read-only
//! derives its deny list from the predicate it already trusts (code-mode does
//! exactly this with `is_code_mode_repo_read`), so its policy is stated once, in
//! the place that owns the distinction.
//!
//! Tables compose under a monoid: a seat's effective policy is
//! `ancestor ⋈ … ⋈ seat`. The merge is *narrowing-only* — there is no `Allow` rule
//! to offset a `Deny` — so a descendant cannot widen what an ancestor denied, and
//! the merge is total, associative, and has the empty table as its identity.

#![allow(dead_code)]

/// What a rule refuses.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Rule {
    /// Any invocation of this tool, by name.
    DenyTool(String),
    /// Any invocation of a tool a seat has named as network-facing.
    DenyNetworkRule(String),
}

impl Rule {
    /// The stable name recorded in a denial receipt.
    fn policy(&self) -> String {
        match self {
            Rule::DenyTool(name) => format!("deny:tool:{name}"),
            Rule::DenyNetworkRule(name) => format!("deny:network:{name}"),
        }
    }
}

/// A context's interception table.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Interception {
    rules: Vec<Rule>,
}

/// What the runtime decided for one invocation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    Permit,
    /// The policy that refused it — carried into the receipt so a denial names
    /// *why*, not just that it happened (Def 27's consulted-at-use metadata).
    Deny {
        policy: String,
    },
}

impl Decision {
    pub fn is_permit(&self) -> bool {
        matches!(self, Decision::Permit)
    }

    pub fn denial(&self) -> Option<&str> {
        match self {
            Decision::Permit => None,
            Decision::Deny { policy } => Some(policy.as_str()),
        }
    }
}

impl Interception {
    /// The identity of the merge: permits everything the context otherwise allows.
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn deny_tool(name: &str) -> Self {
        Self {
            rules: vec![Rule::DenyTool(name.to_string())],
        }
    }

    /// The deny list a seat derives from its own predicate, in one call.
    pub fn deny_tools<S: AsRef<str>>(names: impl IntoIterator<Item = S>) -> Self {
        Self {
            rules: names
                .into_iter()
                .map(|name| Rule::DenyTool(name.as_ref().to_string()))
                .collect(),
        }
    }

    pub fn deny_network_rule(name: &str) -> Self {
        Self {
            rules: vec![Rule::DenyNetworkRule(name.to_string())],
        }
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// `ancestor ⋈ child`: the child's rules are consulted first, and every
    /// ancestor rule survives the merge, so the result is never more permissive
    /// than either side.
    pub fn merge(&self, child: &Interception) -> Interception {
        let mut rules = child.rules.clone();
        rules.extend(self.rules.iter().cloned());
        Interception { rules }
    }

    /// Consult the table for one invocation.
    pub fn consult(&self, tool: &str) -> Decision {
        for rule in &self.rules {
            let hit = match rule {
                Rule::DenyTool(name) => name == tool,
                Rule::DenyNetworkRule(name) => name == tool,
            };
            if hit {
                return Decision::Deny {
                    policy: rule.policy(),
                };
            }
        }
        Decision::Permit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_empty_table_is_the_identity_of_the_merge() {
        let table = Interception::deny_tool("write_file");
        assert_eq!(Interception::empty().merge(&table), table);
        assert_eq!(table.merge(&Interception::empty()), table);
    }

    #[test]
    fn the_merge_is_associative() {
        let a = Interception::deny_tool("shell");
        let b = Interception::deny_tool("write_file");
        let c = Interception::deny_network_rule("web_fetch");
        assert_eq!(a.merge(&b).merge(&c), a.merge(&b.merge(&c)));
    }

    #[test]
    fn a_child_cannot_widen_what_the_parent_denied() {
        let parent = Interception::deny_tool("git_commit");
        let child = Interception::deny_tool("write_file");
        let effective = parent.merge(&child);
        assert_eq!(
            effective.consult("git_commit").denial(),
            Some("deny:tool:git_commit")
        );
        assert_eq!(
            effective.consult("write_file").denial(),
            Some("deny:tool:write_file")
        );
        assert!(effective.consult("read_file").is_permit());
    }

    #[test]
    fn a_denial_names_the_policy_that_refused_it() {
        let table = Interception::deny_tools(["write_file", "str_replace"]);
        assert_eq!(table.len(), 2);
        assert_eq!(
            table.consult("str_replace").denial(),
            Some("deny:tool:str_replace")
        );
        assert!(table.consult("read_file").is_permit());
        assert_eq!(
            Interception::deny_network_rule("web_fetch")
                .consult("web_fetch")
                .denial(),
            Some("deny:network:web_fetch")
        );
    }
}
