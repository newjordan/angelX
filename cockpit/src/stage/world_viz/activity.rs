use super::Building;
use crate::agent::harness::{ToolEventId, ToolOutcome};

pub(super) fn pulse_operation(name: &str, args_summary: &str) -> String {
    let leaf = tool_leaf(name).to_ascii_lowercase();
    if matches!(
        leaf.as_str(),
        "shell" | "exec" | "exec_command" | "proc_run"
    ) {
        let first_arg = args_summary
            .split(", ")
            .next()
            .unwrap_or(args_summary)
            .trim();
        let command = ["cmd=", "command=", "script="]
            .iter()
            .find_map(|prefix| first_arg.strip_prefix(prefix))
            .unwrap_or(first_arg)
            .trim();
        if !command.is_empty() {
            return command.chars().take(120).collect();
        }
    }
    name.chars().take(120).collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RealmActivity {
    Study,
    Forge,
    Chronicle,
    Dispatch,
    Council,
    Memory,
    Research,
    Errand,
}

impl RealmActivity {
    pub(super) fn running_label(self) -> &'static str {
        match self {
            Self::Study => "study running",
            Self::Forge => "forge running",
            Self::Chronicle => "chronicle running",
            Self::Dispatch => "dispatch running",
            Self::Council => "council running",
            Self::Memory => "memory running",
            Self::Research => "research running",
            Self::Errand => "errand running",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ClassifiedActivity {
    pub(crate) building: Building,
    pub(crate) activity: RealmActivity,
}

pub(super) fn tool_leaf(name: &str) -> &str {
    let namespace_leaf = name.rsplit("__").next().unwrap_or(name);
    namespace_leaf
        .rsplit([':', '/', '.'])
        .next()
        .unwrap_or(namespace_leaf)
}

/// One bounded, centralized literal→realm classifier. Shell arguments are
/// inspected only for the known Git/build/test/network verbs below; arbitrary
/// prose never changes destinations or earns research attribution.
pub(crate) fn classify_tool_activity(name: &str, args_summary: &str) -> ClassifiedActivity {
    let leaf = tool_leaf(name).to_ascii_lowercase();
    let args = args_summary
        .chars()
        .take(512)
        .collect::<String>()
        .to_ascii_lowercase();
    let classified = |building, activity| ClassifiedActivity { building, activity };

    if leaf.contains("science")
        || leaf.contains("literature")
        || leaf.contains("research")
        || matches!(
            leaf.as_str(),
            "arxiv" | "scholar" | "openalex" | "crossref" | "pubmed" | "uniprot" | "europepmc"
        )
    {
        return classified(Building::Observatory, RealmActivity::Research);
    }
    if leaf.contains("delegate")
        || leaf.contains("swarm")
        || leaf.contains("formation")
        || matches!(leaf.as_str(), "present" | "moa")
    {
        return classified(Building::RoundTable, RealmActivity::Council);
    }
    if leaf.contains("memory")
        || leaf.contains("memor")
        || leaf.contains("recall")
        || leaf.contains("compact")
        || leaf.contains("deposit")
    {
        return classified(Building::Chapel, RealmActivity::Memory);
    }
    if leaf.contains("web")
        || leaf == "run"
        || leaf.contains("http")
        || leaf.contains("remote")
        || matches!(
            leaf.as_str(),
            "push" | "fetch" | "git_push" | "git_fetch" | "git_pull" | "git_clone"
        )
    {
        return classified(Building::Gatehouse, RealmActivity::Dispatch);
    }
    if leaf.starts_with("git_")
        || matches!(
            leaf.as_str(),
            "git" | "git_status" | "git_log" | "git_diff" | "todo" | "notes" | "plan"
        )
    {
        return classified(Building::Rookery, RealmActivity::Chronicle);
    }
    if leaf.contains("write")
        || leaf.contains("edit")
        || leaf.contains("patch")
        || leaf.contains("build")
        || leaf.contains("test")
        || matches!(
            leaf.as_str(),
            "cargo" | "check" | "lint" | "fmt" | "str_replace" | "tool_repair"
        )
    {
        return classified(Building::Smithy, RealmActivity::Forge);
    }
    if leaf.contains("read")
        || leaf.contains("search")
        || leaf.contains("grep")
        || leaf.contains("find")
        || leaf.contains("symbol")
        || leaf.contains("definition")
        || matches!(
            leaf.as_str(),
            "list_dir" | "outline" | "defs" | "self_map" | "hover" | "references"
        )
    {
        return classified(Building::Scriptorium, RealmActivity::Study);
    }

    if matches!(
        leaf.as_str(),
        "shell" | "exec" | "exec_command" | "proc_run"
    ) {
        if [
            "git push",
            "git fetch",
            "git pull",
            "git clone",
            "curl ",
            "wget ",
            "ssh ",
            "scp ",
        ]
        .iter()
        .any(|verb| args.contains(verb))
        {
            return classified(Building::Gatehouse, RealmActivity::Dispatch);
        }
        if ["git status", "git log", "git commit", "git add", "git diff"]
            .iter()
            .any(|verb| args.contains(verb))
        {
            return classified(Building::Rookery, RealmActivity::Chronicle);
        }
        if [
            "cargo ",
            "cargo",
            "make ",
            "ninja ",
            "cmake ",
            "go test",
            "npm test",
            "pnpm test",
            "yarn test",
            "pytest",
            "mvn test",
            "gradle test",
        ]
        .iter()
        .any(|verb| args.contains(verb))
        {
            return classified(Building::Smithy, RealmActivity::Forge);
        }
    }

    classified(Building::Keep, RealmActivity::Errand)
}

#[derive(Clone, Debug)]
pub(crate) struct ActiveWork {
    pub(crate) id: ToolEventId,
    pub(crate) operation: String,
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) pulse_operation: String,
    pub(crate) literal: String,
    pub(crate) landmark: Building,
    pub(crate) activity: RealmActivity,
    pub(crate) summary: String,
    pub(crate) outcome: Option<ToolOutcome>,
    pub(super) seq: u64,
}
