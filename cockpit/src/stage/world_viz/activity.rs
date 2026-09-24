use super::Building;
use crate::agent::harness::{ToolEventId, ToolOutcome};

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
    pub(crate) literal: String,
    pub(crate) landmark: Building,
    pub(crate) activity: RealmActivity,
    pub(crate) summary: String,
    pub(crate) outcome: Option<ToolOutcome>,
    pub(super) seq: u64,
}

/// One tool call told the realm's way: the place's mark, a verb, and the thing
/// being worked on. The flavour lives in the verb; outcomes stay literal and
/// belong to the caller. Built from the same classifier that walks the knight,
/// so the strip and the world always name the same place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Herald {
    pub(crate) deed: Deed,
    pub(crate) running: &'static str,
    pub(crate) settled: &'static str,
    pub(crate) object: String,
}

/// The kind of deed a call is, as the realm acts it out. The strip draws its
/// mark; the overworld sends someone down the road for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Deed {
    Study,
    Seek,
    Forge,
    Trial,
    Chronicle,
    Seal,
    Dispatch,
    Council,
    Memory,
    Research,
    Errand,
}

impl Deed {
    pub(crate) fn glyph(self) -> char {
        match self {
            Deed::Study | Deed::Seek => '¶',
            Deed::Forge => '⚒',
            Deed::Trial => '⚔',
            Deed::Chronicle | Deed::Seal => '✎',
            Deed::Dispatch => '✉',
            Deed::Council => '⚜',
            Deed::Memory => '☩',
            Deed::Research => '☽',
            Deed::Errand => '♜',
        }
    }
}

impl Herald {
    /// `⚒ Forging toolstrip.rs` while running; the settled verb once the call
    /// came back clean. A failed call keeps the running verb — the attempt is
    /// what failed, and the caller says so.
    pub(crate) fn text(&self, settled: bool) -> String {
        let verb = if settled { self.settled } else { self.running };
        if self.object.is_empty() {
            format!("{} {verb}", self.deed.glyph())
        } else {
            format!("{} {verb} {}", self.deed.glyph(), self.object)
        }
    }

    /// Byte length of the mark and verb at the head of [`Self::text`], so the
    /// draw layer can set the verb apart from the dimmer object.
    pub(crate) fn lead_len(&self, settled: bool) -> usize {
        let verb = if settled { self.settled } else { self.running };
        self.deed.glyph().len_utf8() + 1 + verb.len()
    }
}

/// Longest object a herald carries; the literal call is one click away.
const HERALD_OBJECT_CHARS: usize = 40;

pub(crate) fn herald(name: &str, args_summary: &str) -> Herald {
    let leaf = tool_leaf(name).to_ascii_lowercase();
    let args = parse_args(args_summary);
    let shell = matches!(
        leaf.as_str(),
        "shell" | "exec" | "exec_command" | "proc_run"
    );
    let command = if shell {
        command_head(args.command())
    } else {
        String::new()
    };
    let heralded = |deed, running, settled, object: String| Herald {
        deed,
        running,
        settled,
        object: clip(&object),
    };

    if matches!(
        leaf.as_str(),
        "spawn" | "delegate" | "wait" | "wait_agent" | "wait-agent" | "join_agents"
    ) {
        return heralded(Deed::Council, "Awaiting", "Heard", "the council".into());
    }
    if let Some(object) = trial_object(&leaf, &args) {
        return heralded(Deed::Trial, "Trial by", "Trial by", object);
    }

    let classified = classify_tool_activity(name, args_summary);
    let named = || leaf.replace('_', " ");
    match classified.activity {
        RealmActivity::Study => {
            if [
                "search",
                "grep",
                "find",
                "symbol",
                "definition",
                "references",
            ]
            .iter()
            .any(|verb| leaf.contains(verb))
            {
                let object = args
                    .quoted(&["pattern", "query", "q", "regex", "symbol", "name"])
                    .or_else(|| args.file())
                    .unwrap_or_else(named);
                heralded(Deed::Seek, "Seeking", "Sought", object)
            } else {
                heralded(
                    Deed::Study,
                    "Studying",
                    "Studied",
                    args.file().unwrap_or_else(named),
                )
            }
        }
        RealmActivity::Forge if shell => heralded(Deed::Trial, "Trial by", "Trial by", command),
        RealmActivity::Forge if leaf == "tool_repair" => {
            heralded(Deed::Forge, "Mending", "Mended", "a tool call".into())
        }
        RealmActivity::Forge => heralded(
            Deed::Forge,
            "Forging",
            "Forged",
            args.file().unwrap_or_else(named),
        ),
        // The Rookery seals a commit's scroll before a raven carries it off.
        RealmActivity::Chronicle
            if command == "git commit" || matches!(leaf.as_str(), "git_commit" | "commit") =>
        {
            heralded(Deed::Seal, "Sealing", "Sealed", "a commit".into())
        }
        RealmActivity::Chronicle if shell => {
            heralded(Deed::Chronicle, "Chronicling", "Chronicled", command)
        }
        RealmActivity::Chronicle => {
            let object = if leaf.starts_with("git") {
                named()
            } else {
                format!("the {}", named())
            };
            heralded(Deed::Chronicle, "Chronicling", "Chronicled", object)
        }
        RealmActivity::Dispatch if shell => {
            heralded(Deed::Dispatch, "Dispatching", "Dispatched", command)
        }
        RealmActivity::Dispatch if leaf.contains("search") => heralded(
            Deed::Dispatch,
            "Seeking",
            "Sought",
            args.quoted(&["query", "q"]).unwrap_or_else(named),
        ),
        RealmActivity::Dispatch => match args.host() {
            Some(host) => heralded(Deed::Dispatch, "Riding to", "Back from", host),
            None => heralded(Deed::Dispatch, "Dispatching", "Dispatched", named()),
        },
        RealmActivity::Council => {
            heralded(Deed::Council, "Convening", "Convened", "the council".into())
        }
        RealmActivity::Memory if leaf.contains("compact") => {
            heralded(Deed::Memory, "Condensing", "Condensed", "the record".into())
        }
        RealmActivity::Memory if leaf.contains("deposit") => heralded(
            Deed::Memory,
            "Enshrining",
            "Enshrined",
            args.quoted(&["key", "title", "name"])
                .unwrap_or_else(|| "a memory".into()),
        ),
        RealmActivity::Memory => heralded(
            Deed::Memory,
            "Recalling",
            "Recalled",
            args.quoted(&["query", "q", "key", "topic"])
                .unwrap_or_else(|| "a memory".into()),
        ),
        RealmActivity::Research => heralded(
            Deed::Research,
            "Consulting",
            "Consulted",
            args.quoted(&["query", "q", "term", "search"])
                .map(|query| format!("{} on {query}", named()))
                .unwrap_or_else(named),
        ),
        RealmActivity::Errand if shell => heralded(Deed::Errand, "Running", "Ran", command),
        RealmActivity::Errand => heralded(Deed::Errand, "Wielding", "Wielded", named()),
    }
}

/// Verification calls are trials: the tool-level verifiers the strip already
/// labels, and the cargo/test/build tools the classifier sends to the Smithy.
fn trial_object(leaf: &str, args: &ParsedArgs) -> Option<String> {
    match leaf {
        "run_tests" => Some("the test suite".into()),
        "check" => Some("the checker".into()),
        "lint" => Some("the linter".into()),
        "clippy" => Some("clippy".into()),
        "fmt" => Some("the formatter".into()),
        "cargo" => {
            let sub = args
                .raw
                .split_whitespace()
                .next()
                .filter(|word| !word.contains('='))
                .unwrap_or("");
            Some(format!("cargo {sub}").trim_end().to_string())
        }
        _ if leaf.contains("test") => Some("the tests".into()),
        _ if leaf.contains("build") => Some("the build".into()),
        _ => None,
    }
}

/// `key=value, key=value` as `summarize_args` writes it, or the bare text it
/// passes through for a `command`. Values may themselves hold `, `; a piece
/// only starts a new pair when it opens with `word=`.
struct ParsedArgs<'a> {
    raw: &'a str,
    pairs: Vec<(&'a str, String)>,
}

fn parse_args(raw: &str) -> ParsedArgs<'_> {
    let raw = raw.trim();
    let mut pairs: Vec<(&str, String)> = Vec::new();
    for piece in raw.split(", ") {
        let key = piece.split_once('=').map(|(key, _)| key).filter(|key| {
            !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        });
        match (key, pairs.last_mut()) {
            (Some(key), _) => pairs.push((key, piece[key.len() + 1..].to_string())),
            (None, Some((_, value))) => {
                value.push_str(", ");
                value.push_str(piece);
            }
            (None, None) => {}
        }
    }
    ParsedArgs { raw, pairs }
}

impl ParsedArgs<'_> {
    fn get(&self, keys: &[&str]) -> Option<&str> {
        keys.iter().find_map(|want| {
            self.pairs
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(want))
                .map(|(_, value)| value.trim())
                .filter(|value| !value.is_empty())
        })
    }

    fn command(&self) -> &str {
        self.get(&["cmd", "command", "script"]).unwrap_or(self.raw)
    }

    /// The file a call works on, by its last path segment. Bare text with no
    /// pairs is taken as the path itself (`read_file 実装.md`).
    fn file(&self) -> Option<String> {
        let path = self
            .get(&["path", "file", "file_path", "filename", "target", "dir"])
            .or_else(|| (self.pairs.is_empty() && !self.raw.is_empty()).then_some(self.raw))?;
        let path = path.trim_end_matches('/');
        if matches!(path, "" | ".") {
            return Some("the workspace".into());
        }
        let leaf = path.rsplit('/').next().unwrap_or(path);
        Some(if leaf.is_empty() { path } else { leaf }.to_string())
    }

    fn quoted(&self, keys: &[&str]) -> Option<String> {
        self.get(keys).map(|value| format!("\"{value}\""))
    }

    fn host(&self) -> Option<String> {
        let url = self.get(&["url", "uri", "href"]).unwrap_or(self.raw);
        let rest = url.split_once("://")?.1;
        let host = rest.split(['/', '?', '#']).next()?;
        (!host.is_empty()).then(|| host.to_string())
    }
}

/// The part of a shell command worth naming: the first segment that is not a
/// `cd`/`export`, without env assignments, as the program plus at most one
/// word that is not a flag. Paths shrink to their last segment.
fn command_head(command: &str) -> String {
    let segment = command
        .split(['&', ';', '|'])
        .map(str::trim)
        .find(|segment| {
            !segment.is_empty()
                && !["cd ", "export ", "source ", "set "]
                    .iter()
                    .any(|skip| segment.starts_with(skip))
        })
        .unwrap_or("");
    let short = |word: &str| {
        let word = word.trim_matches(['"', '\'']);
        word.rsplit('/').next().unwrap_or(word).to_string()
    };
    let mut words = segment
        .split_whitespace()
        .skip_while(|word| word.contains('=') && !word.starts_with('-'));
    let Some(program) = words.next() else {
        return String::new();
    };
    let mut head = short(program);
    if let Some(next) = words.next().filter(|word| !word.starts_with('-')) {
        head.push(' ');
        head.push_str(&short(next));
    }
    head
}

fn clip(object: &str) -> String {
    if object.chars().count() <= HERALD_OBJECT_CHARS {
        return object.to_string();
    }
    let mut clipped: String = object.chars().take(HERALD_OBJECT_CHARS - 1).collect();
    clipped.push('…');
    clipped
}
