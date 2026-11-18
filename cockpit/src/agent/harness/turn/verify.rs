use super::*;
#[path = "verify_shell.rs"]
mod verify_shell;
const VERIFY_POLICY: &str = "verify-policy";

#[derive(serde::Serialize)]
struct SegmentFeature {
    verifier: bool,
    build: bool,
}

#[derive(serde::Serialize)]
struct VerifyFeatures {
    tool: &'static str,
    fmt_check: bool,
    verb: &'static str,
    check_flag: bool,
    mutation: bool,
    targets: u32,
    code_targets: u32,
    or_branch: bool,
    async_control: bool,
    status: Vec<SegmentFeature>,
    anywhere: Vec<SegmentFeature>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize)]
pub(crate) struct VerifyDecision {
    pub(crate) requires_verification: bool,
    pub(crate) is_verification: bool,
    pub(crate) is_progress_verifier: bool,
    pub(crate) runs_tests_anywhere: bool,
}

fn segment_feature(segment: &str) -> SegmentFeature {
    SegmentFeature {
        verifier: shell_segment_runs_verifier(segment),
        build: shell_segment_runs_build(segment),
    }
}

fn verify_features(call: &ToolCall) -> VerifyFeatures {
    let tool = match call.name.as_str() {
        "shell" => "shell",
        "cargo" => "cargo",
        "fmt" => "fmt",
        "check" | "lint" | "run_tests" => "check",
        _ => "other",
    };
    let verb = if tool == "cargo" {
        match call
            .args
            .get("args")
            .and_then(Value::as_str)
            .unwrap_or("")
            .split_whitespace()
            .next()
        {
            Some("test") => "test",
            Some("check") => "check",
            Some("clippy") => "clippy",
            Some("fmt") => "fmt",
            Some("build") => "build",
            _ => "none",
        }
    } else {
        "none"
    };
    let mutation = is_mutation_call(call);
    let (mut targets, mut code_targets) = (0u32, 0u32);
    if mutation {
        crate::knowledge::cut::for_each_mutation_target_path(&call.name, &call.args, |path| {
            targets += 1;
            if !is_prose_only_path(path) {
                code_targets += 1;
            }
            false
        });
    }
    let (or_branch, async_control, status, anywhere) = if tool == "shell" {
        let command = crate::agent::tools::shell::shell_command_arg(&call.args).unwrap_or("");
        (
            command.contains("||"),
            shell_has_async_control(command),
            shell_pipeline_segments(shell_status_bearing_segment(command))
                .map(segment_feature)
                .collect(),
            shell_segments_anywhere(command),
        )
    } else {
        (false, false, Vec::new(), Vec::new())
    };
    VerifyFeatures {
        tool,
        fmt_check: call.args.get("check").and_then(Value::as_bool) == Some(true),
        verb,
        check_flag: call
            .args
            .get("args")
            .and_then(Value::as_str)
            .is_some_and(|args| args.contains("--check")),
        mutation,
        targets,
        code_targets,
        or_branch,
        async_control,
        status,
        anywhere,
    }
}

fn heredoc_delimiter(command: &str, at: usize) -> Option<(String, bool, usize)> {
    let rest = command.get(at..)?;
    let (strip_tabs, rest) = match rest.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, rest),
    };
    let rest = rest.trim_start_matches([' ', '\t']);
    let mut delimiter = String::new();
    let mut chars = rest.char_indices();
    let mut end = rest.len();
    while let Some((index, ch)) = chars.next() {
        match ch {
            '\'' | '"' => loop {
                match chars.next()? {
                    (_, close) if close == ch => break,
                    (_, inner) => delimiter.push(inner),
                }
            },
            '\\' => delimiter.push(chars.next()?.1),
            ';' | '|' | '&' | '<' | '>' | '(' | ')' => {
                end = index;
                break;
            }
            ch if ch.is_whitespace() => {
                end = index;
                break;
            }
            ch => delimiter.push(ch),
        }
    }
    (!delimiter.is_empty()).then(|| (delimiter, strip_tabs, command.len() - rest.len() + end))
}

fn heredoc_body_end(command: &str, at: usize, delimiter: &str, strip_tabs: bool) -> Option<usize> {
    let mut start = at;
    loop {
        let rest = command.get(start..)?;
        let (line, next) = match rest.find('\n') {
            Some(newline) => (&rest[..newline], start + newline + 1),
            None => (rest, command.len()),
        };
        let line = line.strip_suffix('\r').unwrap_or(line);
        let line = if strip_tabs {
            line.trim_start_matches('\t')
        } else {
            line
        };
        if line == delimiter {
            return Some(next);
        }
        if next == command.len() {
            return None;
        }
        start = next;
    }
}

fn without_heredoc_bodies(command: &str) -> Option<std::borrow::Cow<'_, str>> {
    if !command.contains("<<") {
        return Some(std::borrow::Cow::Borrowed(command));
    }
    let bytes = command.as_bytes();
    let mut out = String::with_capacity(command.len());
    let mut pending: Vec<(String, bool)> = Vec::new();
    let (mut single, mut double, mut escaped) = (false, false, false);
    let (mut index, mut copied) = (0usize, 0usize);
    while index < bytes.len() {
        let byte = bytes[index];
        if escaped {
            escaped = false;
        } else if byte == b'\\' && !single {
            escaped = true;
        } else if byte == b'\'' && !double {
            single = !single;
        } else if byte == b'"' && !single {
            double = !double;
        } else if byte == b'<' && !single && !double && bytes.get(index + 1) == Some(&b'<') {
            if bytes.get(index + 2) == Some(&b'<') {
                index += 3;
                continue;
            }
            let (delimiter, strip_tabs, end) = heredoc_delimiter(command, index + 2)?;
            out.push_str(&command[copied..index]);
            out.push(' ');
            pending.push((delimiter, strip_tabs));
            index = end;
            copied = end;
            continue;
        } else if byte == b'\n' && !single && !double && !pending.is_empty() {
            out.push_str(&command[copied..=index]);
            let mut at = index + 1;
            for (delimiter, strip_tabs) in pending.drain(..) {
                at = heredoc_body_end(command, at, &delimiter, strip_tabs)?;
            }
            index = at;
            copied = at;
            continue;
        }
        index += 1;
    }
    if !pending.is_empty() {
        return None;
    }
    out.push_str(&command[copied..]);
    Some(std::borrow::Cow::Owned(out))
}

fn shell_segments_anywhere(command: &str) -> Vec<SegmentFeature> {
    let Some(command) = without_heredoc_bodies(command) else {
        return Vec::new();
    };
    let command = command.as_ref();
    let Some(positions) = shell_control_positions(command) else {
        return Vec::new();
    };
    let mut start = 0usize;
    positions
        .into_iter()
        .filter(|index| {
            matches!(
                command.as_bytes()[*index],
                b'|' | b'&' | b';' | b'\n' | b'\r'
            )
        })
        .chain(std::iter::once(command.len()))
        .map(|end| {
            let segment = command.get(start..end).unwrap_or("");
            start = end.saturating_add(1).min(command.len());
            segment_feature(segment)
        })
        .collect()
}

#[cfg(test)]
fn verify_policy_native(call: &ToolCall) -> VerifyDecision {
    VerifyDecision {
        requires_verification: mutation_requires_verification_native(call),
        is_verification: is_verification_call_native(call),
        is_progress_verifier: is_progress_verifier_call_native(&call.name, &call.args),
        runs_tests_anywhere: shell_runs_tests_anywhere_native(call),
    }
}

#[cfg(test)]
fn mutation_requires_verification_native(call: &ToolCall) -> bool {
    if !is_mutation_call(call) {
        return false;
    }
    let mut empty = true;
    let mut needs = false;
    crate::knowledge::cut::for_each_mutation_target_path(&call.name, &call.args, |path| {
        empty = false;
        if !is_prose_only_path(path) {
            needs = true;
            return true;
        }
        false
    });
    empty || needs
}

#[cfg(test)]
fn is_verification_call_native(call: &ToolCall) -> bool {
    match call.name.as_str() {
        "run_tests" | "check" | "lint" => true,
        "fmt" => call.args.get("check").and_then(Value::as_bool) == Some(true),
        "cargo" => {
            let first = call
                .args
                .get("args")
                .and_then(Value::as_str)
                .unwrap_or("")
                .split_whitespace()
                .next();
            matches!(first, Some("test" | "check" | "clippy"))
                || (first == Some("fmt")
                    && call
                        .args
                        .get("args")
                        .and_then(Value::as_str)
                        .is_some_and(|args| args.contains("--check")))
        }
        "shell" => {
            let command = crate::agent::tools::shell::shell_command_arg(&call.args).unwrap_or("");
            shell_command_runs_verifier(command)
        }
        _ => false,
    }
}

#[cfg(test)]
fn is_progress_verifier_call_native(name: &str, args: &Value) -> bool {
    let call = ToolCall {
        id: String::new(),
        name: name.into(),
        args: args.clone(),
    };
    if is_verification_call_native(&call) {
        return true;
    }
    if name == "cargo" {
        return args
            .get("args")
            .and_then(Value::as_str)
            .is_some_and(|s| s.split_whitespace().next() == Some("build"));
    }
    if name != "shell" {
        return false;
    }
    let command = crate::agent::tools::shell::shell_command_arg(args).unwrap_or("");
    if command.contains("||") || shell_has_async_control(command) {
        return false;
    }
    shell_pipeline_segments(shell_status_bearing_segment(command)).any(shell_segment_runs_build)
}

fn verify_policy_check(
    features: &VerifyFeatures,
) -> impl Fn(&VerifyDecision) -> Result<(), String> + '_ {
    move |decision| {
        if decision.requires_verification && !features.mutation {
            return Err("verify policy owes verification for a call that changed nothing".into());
        }
        if decision.requires_verification && features.targets > 0 && features.code_targets == 0 {
            return Err("verify policy owes verification for prose-only targets".into());
        }
        if decision.runs_tests_anywhere && features.tool != "shell" {
            return Err("verify policy answered the shell-only field for another tool".into());
        }
        if decision.is_verification && !decision.is_progress_verifier {
            return Err("verify policy dropped progress credit for a verifier".into());
        }
        let build = match features.tool {
            "cargo" => features.verb == "build",
            "shell" => features.status.iter().any(|segment| segment.build),
            _ => false,
        };
        if decision.is_progress_verifier && !decision.is_verification && !build {
            return Err("verify policy claimed progress without verifier or build evidence".into());
        }
        Ok(())
    }
}

#[cfg(test)]
fn verify_policy_with(
    call: &ToolCall,
    program: Option<angel_live::Program>,
) -> (VerifyDecision, crate::agent::harness::live_policy::LegSource) {
    let features = verify_features(call);
    let input = serde_json::to_value(&features).unwrap_or(Value::Null);
    crate::agent::harness::live_policy::decide_with(
        program,
        VERIFY_POLICY,
        &input,
        verify_policy_check(&features),
        || verify_policy_native(call),
    )
}

fn verify_policy(call: &ToolCall) -> VerifyDecision {
    let features = verify_features(call);
    let input = serde_json::to_value(&features).unwrap_or(Value::Null);
    crate::agent::harness::live_policy::decide_shipped(
        VERIFY_POLICY,
        &input,
        verify_policy_check(&features),
    )
    .0
}

pub(crate) fn mutation_requires_verification(call: &ToolCall) -> bool {
    verify_policy(call).requires_verification
}

pub(crate) fn is_verification_call(call: &ToolCall) -> bool {
    verify_policy(call).is_verification
}

pub(crate) fn is_progress_verifier_call(name: &str, args: &Value) -> bool {
    verify_policy(&ToolCall {
        id: String::new(),
        name: name.into(),
        args: args.clone(),
    })
    .is_progress_verifier
}

pub(crate) fn shell_runs_tests_anywhere(call: &ToolCall) -> bool {
    verify_policy(call).runs_tests_anywhere
}

#[cfg(test)]
pub(crate) fn verification_decision_with(
    call: &ToolCall,
    program: Option<angel_live::Program>,
) -> (VerifyDecision, crate::agent::harness::live_policy::LegSource) {
    verify_policy_with(call, program)
}

#[cfg(test)]
pub(crate) fn verification_decision_native(call: &ToolCall) -> VerifyDecision {
    verify_policy_native(call)
}
fn shell_segment_runs_build(segment: &str) -> bool {
    let words: Vec<_> = segment
        .split_whitespace()
        .skip_while(|s| shell_env_assignment(s))
        .collect();
    let Some(program) = words.first().map(|s| s.rsplit('/').next().unwrap_or(s)) else {
        return false;
    };
    match program {
        "cargo" => words.get(1) == Some(&"build"),
        "npm" | "pnpm" | "yarn" | "bun" => {
            words.get(1) == Some(&"build")
                || (words.get(1) == Some(&"run") && words.get(2) == Some(&"build"))
        }
        "cmake" => words.get(1) == Some(&"--build"),
        "make" | "ninja" => {
            words.len() == 1 || words.get(1).is_some_and(|s| matches!(*s, "all" | "build"))
        }
        _ => false,
    }
}

fn shell_env_assignment(word: &str) -> bool {
    let Some((name, _)) = word.split_once('=') else {
        return false;
    };
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn ascii_choice(word: &str, options: &[&str]) -> bool {
    options
        .iter()
        .any(|option| word.eq_ignore_ascii_case(option))
}

const JS_TEST_RUNNERS: &[&str] = &["jest", "vitest", "mocha", "ava", "jasmine", "tap"];

fn runs_js_test_runner(program: &str, words: &[&str]) -> bool {
    let subcommand = words.get(1).copied().unwrap_or("");
    let rest = if ascii_choice(program, &["npx", "bunx", "pnpx"]) {
        &words[1..]
    } else if ascii_choice(program, &["npm", "pnpm", "yarn", "bun"])
        && ascii_choice(subcommand, &["exec", "dlx", "x"])
    {
        &words[2..]
    } else if ascii_choice(program, &["pnpm", "yarn"]) {
        &words[1..]
    } else {
        words
    };
    rest.iter()
        .find(|word| !word.starts_with('-'))
        .map(|word| word.rsplit('/').next().unwrap_or(word))
        .is_some_and(|runner| ascii_choice(runner, JS_TEST_RUNNERS))
}

fn shell_segment_runs_verifier(segment: &str) -> bool {
    let words = segment.split_whitespace().collect::<Vec<_>>();
    let mut start = 0;
    while words
        .get(start)
        .is_some_and(|word| shell_env_assignment(word))
    {
        start += 1;
    }
    let words = &words[start..];
    let Some(program) = words
        .first()
        .map(|word| word.rsplit('/').next().unwrap_or(word))
    else {
        return false;
    };
    if runs_js_test_runner(program, words) {
        return true;
    }
    if program.eq_ignore_ascii_case("cargo") {
        words
            .get(1)
            .is_some_and(|word| ascii_choice(word, &["test", "check", "clippy"]))
            || (words
                .get(1)
                .is_some_and(|word| word.eq_ignore_ascii_case("fmt"))
                && words
                    .iter()
                    .any(|word| word.eq_ignore_ascii_case("--check")))
    } else if program.eq_ignore_ascii_case("node") {
        words
            .iter()
            .skip(1)
            .any(|word| word.eq_ignore_ascii_case("--test"))
    } else if ascii_choice(program, &["npm", "pnpm", "yarn", "bun"]) {
        words
            .get(1)
            .is_some_and(|word| word.eq_ignore_ascii_case("test"))
            || (words
                .get(1)
                .is_some_and(|word| word.eq_ignore_ascii_case("run"))
                && words
                    .get(2)
                    .is_some_and(|word| word.eq_ignore_ascii_case("test")))
    } else if ascii_choice(program, &["pytest", "pytest-3"]) {
        true
    } else if ascii_choice(program, &["python", "python3"]) {
        words.windows(2).any(|pair| {
            pair[0].eq_ignore_ascii_case("-m") && ascii_choice(pair[1], &["pytest", "unittest"])
        })
    } else if program.eq_ignore_ascii_case("uv") {
        words
            .get(1)
            .is_some_and(|word| word.eq_ignore_ascii_case("run"))
            && (words
                .get(2)
                .is_some_and(|word| ascii_choice(word, &["pytest", "pytest-3"]))
                || words.windows(3).any(|triple| {
                    ascii_choice(triple[0], &["python", "python3"])
                        && triple[1].eq_ignore_ascii_case("-m")
                        && triple[2].eq_ignore_ascii_case("pytest")
                }))
    } else if program.eq_ignore_ascii_case("go") {
        words
            .get(1)
            .is_some_and(|word| ascii_choice(word, &["test", "build", "vet"]))
    } else if ascii_choice(program, &["make", "just"]) {
        words
            .get(1)
            .is_some_and(|word| ascii_choice(word, &["test", "check"]))
    } else if ascii_choice(program, &["mvn", "mvnw"]) {
        words
            .iter()
            .skip(1)
            .any(|word| ascii_choice(word, &["test", "verify", "package"]))
    } else if ascii_choice(program, &["gradle", "gradlew"]) {
        words.iter().skip(1).any(|word| {
            let task = word.rsplit(':').next().unwrap_or(word);
            ascii_choice(task, &["test", "check", "build"])
        })
    } else if program.eq_ignore_ascii_case("dotnet") {
        words
            .get(1)
            .is_some_and(|word| ascii_choice(word, &["test", "build"]))
    } else if program.eq_ignore_ascii_case("ssh") {
        let mut i = 1;
        while i < words.len() {
            let arg = words[i];
            if arg.starts_with('-') {
                if matches!(
                    arg,
                    "-o" | "-p" | "-i" | "-l" | "-F" | "-c" | "-b" | "-E" | "-e" | "-J"
                ) {
                    i += 2;
                } else {
                    i += 1;
                }
            } else {
                i += 1;
                break;
            }
        }
        if i < words.len() {
            let remote_cmd = words[i..].join(" ");
            let trimmed = remote_cmd
                .strip_prefix('\'')
                .and_then(|s| s.strip_suffix('\''))
                .or_else(|| {
                    remote_cmd
                        .strip_prefix('"')
                        .and_then(|s| s.strip_suffix('"'))
                })
                .unwrap_or(&remote_cmd);
            shell_command_runs_verifier(trimmed)
        } else {
            false
        }
    } else {
        program.eq_ignore_ascii_case("ctest")
    }
}

fn shell_has_async_control(command: &str) -> bool {
    let Some(positions) = shell_control_positions(command) else {
        return true;
    };
    positions.into_iter().any(|index| {
        let bytes = command.as_bytes();
        if bytes[index] != b'&' {
            return false;
        }
        let prev = index.checked_sub(1).and_then(|i| bytes.get(i)).copied();
        let next = bytes.get(index + 1).copied();
        prev != Some(b'&') && next != Some(b'&') && prev != Some(b'>') && next != Some(b'>')
    })
}

fn shell_control_positions(command: &str) -> Option<Vec<usize>> {
    let bytes = command.as_bytes();
    let mut positions = Vec::new();
    let mut single = false;
    let mut double = false;
    let mut escaped = false;
    for (index, byte) in bytes.iter().copied().enumerate() {
        if escaped {
            escaped = false;
            continue;
        }
        if byte == b'\\' && !single {
            escaped = true;
            continue;
        }
        match byte {
            b'\'' if !double => single = !single,
            b'"' if !single => double = !double,
            b'`' if !single => return None,
            b'$' if !single && bytes.get(index + 1) == Some(&b'(') => return None,
            b'<' | b'>' if !single && !double && bytes.get(index + 1) == Some(&b'(') => {
                return None;
            }
            b'(' | b')' | b'{' | b'}' if !single && !double => return None,
            b'<' if !single && !double && bytes.get(index + 1) == Some(&b'<') => return None,
            b'#' if !single && !double => return None,
            b';' | b'\n' | b'\r' | b'|' | b'&' if !single && !double => positions.push(index),
            _ => {}
        }
    }
    if single || double || escaped {
        None
    } else {
        Some(positions)
    }
}

fn shell_status_bearing_segment(command: &str) -> &str {
    let Some(positions) = shell_control_positions(command) else {
        return "";
    };
    let mut end = command.len();
    for index in positions
        .into_iter()
        .rev()
        .filter(|index| matches!(command.as_bytes()[*index], b';' | b'\n' | b'\r'))
    {
        let candidate = command.get(index + 1..end).unwrap_or("");
        if !candidate.trim().is_empty() {
            return candidate;
        }
        end = index;
    }
    command.get(..end).unwrap_or("")
}

#[cfg(test)]
fn shell_runs_tests_anywhere_native(call: &ToolCall) -> bool {
    if call.name != "shell" {
        return false;
    }
    let command = crate::agent::tools::shell::shell_command_arg(&call.args).unwrap_or("");
    without_heredoc_bodies(command).is_some_and(|command| verify_shell::classify(&command, 1))
}

fn shell_command_runs_verifier(command: &str) -> bool {
    verify_shell::classify(command, 0)
}

/// Whether a shell command invokes a test runner anywhere, whatever wraps it: `timeout`, an
/// environment prefix, a subshell, a loop, a pipeline, a memory-cap wrapper. Deliberately
/// looser than the status-bearing classifier: it never decides that verification passed,
/// only whether a non-zero exit was a red test or a call that failed to run.
pub(crate) fn mentions_test_run(command: &str) -> bool {
    let Some(command) = without_heredoc_bodies(command) else {
        return false;
    };
    let words: Vec<&str> = command
        .split(|c: char| c.is_whitespace() || matches!(c, ';' | '|' | '&' | '(' | ')' | '{' | '}'))
        .filter(|word| !word.is_empty())
        .collect();
    let program = |word: &'_ str| -> String {
        word.rsplit('/').next().unwrap_or(word).to_ascii_lowercase()
    };
    words.windows(2).any(|pair| {
        let name = program(pair[0]);
        if name == "cargo" || name.ends_with("-cargo") {
            matches!(pair[1], "test" | "nextest" | "check" | "clippy")
        } else if matches!(
            name.as_str(),
            "go" | "npm" | "pnpm" | "yarn" | "bun" | "deno" | "dotnet" | "swift" | "zig" | "mix" | "gradle" | "mvn"
        ) {
            pair[1] == "test"
        } else if matches!(name.as_str(), "make" | "ninja") {
            matches!(pair[1], "test" | "check")
        } else {
            false
        }
    }) || words.iter().any(|word| {
        matches!(
            program(word).as_str(),
            "pytest" | "ctest" | "tox" | "nox" | "rspec" | "jest" | "vitest" | "phpunit"
        )
    })
}

fn shell_pipeline_segments(segment: &str) -> impl Iterator<Item = &str> {
    let mut start = 0usize;
    shell_control_positions(segment)
        .unwrap_or_default()
        .into_iter()
        .filter(|index| matches!(segment.as_bytes()[*index], b'|' | b'&'))
        .chain(std::iter::once(segment.len()))
        .map(move |end| {
            let item = &segment[start..end];
            start = end.saturating_add(1).min(segment.len());
            item
        })
}

fn cargo_test_is_full(words: &[&str]) -> bool {
    const TARGETING_FLAGS: &[&str] = &[
        "--ignored",
        "--test",
        "--tests",
        "--bench",
        "--benches",
        "--example",
        "--examples",
        "--package",
        "-p",
        "--exclude",
        "--lib",
        "--bin",
        "--bins",
        "--doc",
        "--no-run",
    ];
    words.iter().all(|word| {
        *word == "--"
            || (word.starts_with('-')
                && !TARGETING_FLAGS
                    .iter()
                    .any(|flag| word.eq_ignore_ascii_case(flag)))
    })
}

pub(crate) fn verification_result_has_known_gap(result: &str) -> bool {
    let header = result.lines().next().unwrap_or("");
    header.split("; ").any(|part| part == "coverage=incomplete")
}

pub(crate) fn verification_is_completion_sufficient(call: &ToolCall) -> bool {
    match call.name.as_str() {
        "run_tests" => call
            .args
            .get("args")
            .and_then(Value::as_str)
            .map(|args| args.split_whitespace().collect::<Vec<_>>())
            .is_none_or(|words| cargo_test_is_full(&words)),
        "check" => true,
        "lint" | "fmt" => false,
        "cargo" => call
            .args
            .get("args")
            .and_then(Value::as_str)
            .map(|args| args.split_whitespace().collect::<Vec<_>>())
            .is_some_and(|words| match words.first() {
                Some(&"test") => cargo_test_is_full(&words[1..]),
                _ => false,
            }),
        "shell" => false,
        _ => false,
    }
}

pub(crate) fn with_turn_deadline_cancel<R>(
    operator_cancel: &AtomicBool,
    deadline: Option<Instant>,
    operation: impl FnOnce(&AtomicBool) -> R,
) -> R {
    crate::agent::tools::http_transport::with_deadline(deadline, || {
        let Some(deadline) = deadline else {
            return operation(operator_cancel);
        };
        let effective_cancel =
            AtomicBool::new(operator_cancel.load(Ordering::Acquire) || Instant::now() >= deadline);
        let _activity_owner =
            super::super::exec::link_child_owner(&effective_cancel, operator_cancel);
        struct ProcessOwnerTransfer(usize, usize);
        impl Drop for ProcessOwnerTransfer {
            fn drop(&mut self) {
                crate::agent::tools::proc::transfer_owned(self.0, self.1);
            }
        }
        let _process_owner = ProcessOwnerTransfer(
            &effective_cancel as *const _ as usize,
            operator_cancel as *const _ as usize,
        );
        if effective_cancel.load(Ordering::Acquire) {
            return operation(&effective_cancel);
        }

        let done = AtomicBool::new(false);
        std::thread::scope(|scope| {
            struct WatcherGuard<'a> {
                done: &'a AtomicBool,
                thread: std::thread::Thread,
            }
            impl Drop for WatcherGuard<'_> {
                fn drop(&mut self) {
                    self.done.store(true, Ordering::Release);
                    self.thread.unpark();
                }
            }

            let watcher = scope.spawn(|| {
                while !done.load(Ordering::Acquire) {
                    if operator_cancel.load(Ordering::Acquire) || Instant::now() >= deadline {
                        effective_cancel.store(true, Ordering::Release);
                        return;
                    }
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    std::thread::park_timeout(remaining.min(Duration::from_millis(25)));
                }
            });
            let _watcher_guard = WatcherGuard {
                done: &done,
                thread: watcher.thread().clone(),
            };
            operation(&effective_cancel)
        })
    })
}

pub(crate) fn is_completion_green(outcome: VerificationOutcome, mutation_seen: bool) -> bool {
    mutation_seen && outcome == VerificationOutcome::Passed
}

pub(super) fn verification_attempt_releases_gate(
    call: &ToolCall,
    outcome: VerificationOutcome,
) -> bool {
    outcome != VerificationOutcome::NotApplicable
        && (call.name != "shell" || outcome != VerificationOutcome::Inconclusive)
}

fn labeled_count(result: &str, label: &str) -> Option<usize> {
    let words: Vec<&str> = result
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .filter(|word| !word.is_empty())
        .collect();
    words.windows(2).find_map(|pair| {
        (pair[1] == label)
            .then(|| pair[0].parse::<usize>().ok())
            .flatten()
    })
}

pub(crate) fn verification_outcome(call: &ToolCall, result: &str) -> Option<VerificationOutcome> {
    if !is_verification_call(call)
        || result.starts_with("action capsule denied")
        || is_hook_blocked_result(result)
    {
        return None;
    }
    if is_error_result(result) {
        return Some(VerificationOutcome::Failed);
    }
    if result.starts_with("verification inconclusive:") {
        return Some(VerificationOutcome::Inconclusive);
    }
    let typed_summary = matches!(call.name.as_str(), "run_tests" | "check" | "lint");
    let verdict_text = if typed_summary {
        result.lines().next().unwrap_or("")
    } else {
        result
    };
    if typed_summary {
        if result.starts_with("verification inconclusive:") {
            return Some(VerificationOutcome::Inconclusive);
        }
        if result.starts_with("no tests ran") {
            return Some(VerificationOutcome::Failed);
        }
        let expected = match call.name.as_str() {
            "run_tests" => "tests:",
            "check" => "check:",
            _ => "lint:",
        };
        if !verdict_text.starts_with(expected) {
            return Some(VerificationOutcome::Inconclusive);
        }
    }
    if ascii_contains_ignore_case(verdict_text, "timed out") {
        return Some(VerificationOutcome::Failed);
    }
    if typed_summary && ascii_contains_ignore_case(verdict_text, "unlabeled") {
        return Some(VerificationOutcome::Inconclusive);
    }
    let outcome = match call.name.as_str() {
        "run_tests" => match (
            labeled_count(verdict_text, "passed"),
            labeled_count(verdict_text, "failed"),
        ) {
            (Some(passed), Some(0)) if passed > 0 => VerificationOutcome::Passed,
            (_, Some(failed)) if failed > 0 => VerificationOutcome::Failed,
            _ if ascii_contains_ignore_case(result, "no tests ran") => VerificationOutcome::Failed,
            _ => VerificationOutcome::Inconclusive,
        },
        "check" | "lint" => match labeled_count(verdict_text, "errors") {
            Some(0) => VerificationOutcome::Passed,
            Some(_) => VerificationOutcome::Failed,
            None => VerificationOutcome::Inconclusive,
        },
        "fmt" => {
            if ascii_contains_ignore_case(result, "already rustfmt-clean") {
                VerificationOutcome::Passed
            } else if ascii_contains_ignore_case(result, "needs formatting") {
                VerificationOutcome::Failed
            } else {
                VerificationOutcome::Inconclusive
            }
        }
        "shell" => {
            VerificationOutcome::Inconclusive
        }
        "cargo" => {
            let args = call.args.get("args").and_then(Value::as_str).unwrap_or("");
            let first = args.split_whitespace().next();
            match first {
                Some("test") => {
                    let parsed = crate::agent::harness::parse_test_result(result);
                    if parsed.all_passed() {
                        VerificationOutcome::Passed
                    } else if parsed.ran() > 0
                        || ascii_contains_ignore_case(result, "could not compile")
                    {
                        VerificationOutcome::Failed
                    } else {
                        VerificationOutcome::Inconclusive
                    }
                }
                Some("check" | "clippy") => {
                    if ascii_contains_ignore_case(result, "error: could not compile") {
                        VerificationOutcome::Failed
                    } else if ascii_contains_ignore_case(result, "[cargo verdict: pass]")
                        || ascii_contains_ignore_case(result, "finished ")
                    {
                        VerificationOutcome::Passed
                    } else {
                        VerificationOutcome::Inconclusive
                    }
                }
                Some("fmt") if args.contains("--check") => {
                    if ascii_contains_ignore_case(result, "[cargo verdict: pass]")
                        || result.trim().is_empty()
                    {
                        VerificationOutcome::Passed
                    } else if ascii_contains_ignore_case(result, "diff in") {
                        VerificationOutcome::Failed
                    } else {
                        VerificationOutcome::Inconclusive
                    }
                }
                _ => VerificationOutcome::Inconclusive,
            }
        }
        _ => VerificationOutcome::Inconclusive,
    };
    Some(outcome)
}

pub(crate) fn turn_event_outcome(call: &ToolCall, result: &str, denied: bool) -> ToolOutcome {
    let header = result.lines().next().unwrap_or("").trim();
    let header = header.trim_start_matches("tool error: ");
    let starts = |prefix: &str| {
        header
            .get(..prefix.len())
            .is_some_and(|text| text.eq_ignore_ascii_case(prefix))
    };
    let execution = if denied
        || result.starts_with("action capsule denied")
        || is_hook_blocked_result(result)
    {
        ExecutionOutcome::Denied
    } else if starts("worker panicked")
        || starts("tool panicked")
        || (is_error_result(result) && starts(&format!("{} panicked:", call.name)))
    {
        ExecutionOutcome::Panicked
    } else if (is_error_result(result)
        && crate::agent::sandbox::process_owner::KillReceipt::from_error(result)
            .is_some_and(|kill| kill.reason == "cancelled"))
        || [
            "cancelled by ",
            "canceled by ",
            "interrupted by ",
            "execution cancelled before spawn",
            "shell command cancelled",
            "cargo command cancelled",
            "cargo test cancelled",
            "cargo clippy cancelled",
            "cargo check cancelled",
            "cargo fmt cancelled",
            "code_mode cancelled",
            "process wait cancelled",
            "process launch cancelled",
        ]
        .into_iter()
        .any(starts)
    {
        ExecutionOutcome::Cancelled
    } else if ascii_contains_ignore_case(result, "[timed out after ") || is_error_result(result) {
        ExecutionOutcome::Failed
    } else {
        ExecutionOutcome::Succeeded
    };
    let verification = verification_outcome(call, result).unwrap_or_else(|| {
        if (is_verification_call(call) && execution != ExecutionOutcome::Denied)
            || (matches!(
                call.name.as_str(),
                "shell" | "proc_run" | "proc_status" | "proc_wait" | "proc_stop"
            ) && matches!(
                execution,
                ExecutionOutcome::Failed | ExecutionOutcome::Cancelled | ExecutionOutcome::Panicked
            ))
        {
            VerificationOutcome::Inconclusive
        } else {
            VerificationOutcome::NotApplicable
        }
    });
    ToolOutcome {
        execution,
        verification,
    }
}

#[cfg(test)]
pub(crate) fn looks_like_deferred_action_only(answer: &str) -> bool {
    let text = answer.trim();
    if text.is_empty() || text.len() > 4_000 {
        return false;
    }
    const INTENT: &[&str] = &[
        "let me ",
        "i'll ",
        "i will ",
        "i need to ",
        "i’m ",
        "i am ",
        "i'm going to ",
        "i plan to ",
        "i should ",
        "we should ",
        "next i ",
        "starting with ",
        "starting ",
        "beginning ",
        "start by ",
        "first, i'll ",
        "first i’ll ",
        "first i ",
        "i’ll ",
        "before i can ",
        "my approach",
        "step 1",
        "plan:",
        "here's the plan",
        "here is the plan",
    ];
    if !INTENT
        .iter()
        .any(|needle| ascii_contains_ignore_case(text, needle))
    {
        return false;
    }
    const WORK: &[&str] = &[
        "inspect",
        "check",
        "look at",
        "read",
        "recon",
        "pull the lay",
        "locat",
        "brief",
        "council",
        "delegate",
        "discuss",
        "figure out",
        "analyz",
        "investigat",
        "research",
        "explore",
        "understand",
        "review the",
        "map the",
        "think through",
        "consider",
    ];
    if !WORK
        .iter()
        .any(|needle| ascii_contains_ignore_case(text, needle))
    {
        return false;
    }
    const CLOSURE: &[&str] = &[
        "done",
        "completed",
        "fixed",
        "changed",
        "verified",
        "the answer is",
        "i found",
        "i checked",
        "submitted",
        "submission id",
        "zero submissions",
    ];
    !CLOSURE
        .iter()
        .any(|needle| ascii_contains_ignore_case(text, needle))
}
