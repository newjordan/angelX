//! Truthful attribution on competition submissions.
//!
//! Board CLIs (`yukon`, installed as `hilbert`) require `--model` and
//! `--harness` and print them on the public leaderboard and in the public
//! note. Models fill those flags by copying whatever they last read — a
//! leader's note head, a template, an older submission — so 2026-09-10 the
//! spark qwen38 loop published four submissions as "GPT 5.6 Sol / Codex" from
//! deepseek-v4-flash on angel0, and the operator reports this happens on every
//! board ("it's always just copying whatever it pulled"). The harness knows
//! its own identity; it stamps it and discards the model-supplied values.
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use regex::Regex;

pub(crate) const HARNESS_LABEL: &str = "angel0";

#[derive(Clone, Debug, Default)]
pub(crate) struct SubmitJournalEntry {
    pub workspace: Option<PathBuf>,
    pub turn: Option<String>,
    pub utc: String,
    pub tool: String,
    pub commit_or_patch_sha: String,
    pub note_file_sha256: String,
    pub model: String,
    pub harness: String,
    pub exit_code: i32,
    pub platform_response_excerpt: String,
    pub outcome: String,
}

fn journal() -> &'static Mutex<Vec<SubmitJournalEntry>> {
    static J: OnceLock<Mutex<Vec<SubmitJournalEntry>>> = OnceLock::new();
    J.get_or_init(|| Mutex::new(Vec::new()))
}

#[cfg(test)]
pub(crate) fn drain_journal() -> Vec<SubmitJournalEntry> {
    journal()
        .lock()
        .map(|mut g| g.drain(..).collect())
        .unwrap_or_default()
}

pub(crate) fn drain_workspace_journal(
    workspace: Option<&Path>,
    owner: &str,
) -> Vec<SubmitJournalEntry> {
    let workspace = workspace.and_then(|p| p.canonicalize().ok());
    let Ok(mut journal) = journal().lock() else {
        return Vec::new();
    };
    let mut owned = Vec::new();
    journal.retain(|entry| {
        if workspace.is_some()
            && entry.workspace == workspace
            && entry.turn.as_deref() == Some(owner)
        {
            owned.push(entry.clone());
            false
        } else {
            true
        }
    });
    owned
}

fn redact_excerpt(text: &str) -> String {
    let mut s: String = text.chars().take(400).collect();
    for key in ["ANGEL_", "API_KEY", "Bearer ", "sk-"] {
        if s.contains(key) {
            s = s.replace(key, "[redacted]");
        }
    }
    s
}

fn classify_outcome(exit: i32, output: &str) -> String {
    let lower = output.to_ascii_lowercase();
    if exit != 0 || lower.contains("refuse") {
        return "refused".into();
    }
    if lower.contains("rejected") || lower.contains("byte-gate") || lower.contains("byte gate") {
        return "rejected".into();
    }
    // Shell output is not an authoritative status channel. Enqueue is not acceptance.
    if output
        .lines()
        .any(|line| line.trim().eq_ignore_ascii_case("submission queued"))
        && crate::harness::extract_submission_id(output).is_some()
    {
        return "dispatched".into();
    }
    "unknown".into()
}

/// Record a stamped submit after the tool process exits (no network).
pub(crate) fn journal_execution(
    tool: &str,
    command: &str,
    workspace: Option<&Path>,
    exit: Option<i32>,
    output: &str,
) {
    if matches!(submission_words(command), Ok(words) if words.is_empty()) {
        return;
    }
    let exit_code = exit.unwrap_or(-1);
    let model = model_label();
    let words = submission_words(command)
        .ok()
        .and_then(|mut batches| (batches.len() == 1).then(|| batches.remove(0)));
    let note_sha = words
        .as_ref()
        .and_then(|words| {
            if !command[..words[0].start].trim().is_empty() {
                return None;
            }
            note_file_path(&command[words[0].start..words.last()?.end], workspace)
        })
        .and_then(|path| std::fs::read(path).ok())
        .map(|bytes| crate::cut::sha256_hex(&bytes))
        .unwrap_or_default();
    let entry = SubmitJournalEntry {
        workspace: workspace.and_then(|p| p.canonicalize().ok()),
        turn: crate::harness::run_identity::live_turn(),
        utc: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs().to_string())
            .unwrap_or_default(),
        tool: tool.to_string(),
        commit_or_patch_sha: String::new(),
        note_file_sha256: note_sha,
        model: model.as_ref().cloned().unwrap_or_default(),
        harness: HARNESS_LABEL.to_string(),
        exit_code,
        platform_response_excerpt: redact_excerpt(output),
        outcome: if model.is_err() {
            "refused".into()
        } else {
            classify_outcome(exit_code, output)
        },
    };
    if let Ok(mut g) = journal().lock() {
        g.push(entry);
    }
}

/// The rewritten command plus the notice the tool result carries.
pub(crate) struct Stamped {
    pub command: String,
    pub notice: String,
}

fn submit_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b(yukon|hilbert)(\s+)submit\b").expect("submit regex"))
}

#[derive(Debug)]
struct Word {
    start: usize,
    end: usize,
    value: String,
    dynamic: bool,
}

/// Recognize only the literal message-producing form `$(cat <<'DELIM' ... )`.
/// Its quoted body cannot execute shell expansions. Preserve byte offsets while
/// hiding that data from the flat attribution lexer; the original command is
/// still what runs. Pipelines, unquoted delimiters and executable tails stay on
/// the conservative path instead of being guessed safe by this recognizer.
fn literal_heredoc_substitution_end(command: &str, start: usize) -> Option<usize> {
    const MAX_LITERAL_BYTES: usize = 64 * 1024;
    let header_start = start.checked_add(2)?;
    let header_end = header_start + command.get(header_start..)?.find('\n')?;
    if header_end - start > MAX_LITERAL_BYTES {
        return None;
    }
    let header = command[header_start..header_end].trim_matches([' ', '\t']);
    let (program, rest) = header.split_once("<<")?;
    if !matches!(
        program.trim_matches([' ', '\t']),
        "cat" | "/bin/cat" | "/usr/bin/cat"
    ) {
        return None;
    }
    let strip_tabs = rest.starts_with('-');
    let rest = if strip_tabs { &rest[1..] } else { rest }.trim_matches([' ', '\t']);
    let quote = rest.as_bytes().first().copied()?;
    if !matches!(quote, b'\'' | b'"') || rest.as_bytes().last() != Some(&quote) {
        return None;
    }
    let delimiter = rest.get(1..rest.len().checked_sub(1)?)?;
    if delimiter.is_empty()
        || delimiter.len() > 128
        || !delimiter
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return None;
    }
    let mut next = header_end + 1;
    for line in command[next..].split_inclusive('\n') {
        next += line.len();
        if next - start > MAX_LITERAL_BYTES {
            return None;
        }
        let line = line.strip_suffix('\n').unwrap_or(line);
        let line = if strip_tabs {
            line.trim_start_matches('\t')
        } else {
            line
        };
        if line != delimiter {
            continue;
        }
        while command
            .as_bytes()
            .get(next)
            .is_some_and(u8::is_ascii_whitespace)
        {
            next += 1;
        }
        if next - start >= MAX_LITERAL_BYTES {
            return None;
        }
        return (command.as_bytes().get(next) == Some(&b')')).then_some(next + 1);
    }
    None
}

fn mask_literal_heredoc_substitutions(command: &str) -> Option<String> {
    let bytes = command.as_bytes();
    let mut masked: Option<Vec<u8>> = None;
    let mut quote = None;
    let mut at = 0;
    while at < bytes.len() {
        let byte = bytes[at];
        if byte == b'\\' && quote != Some(b'\'') {
            at = (at + 2).min(bytes.len());
            continue;
        }
        if quote != Some(b'\'') && byte == b'$' && bytes.get(at + 1) == Some(&b'(') {
            // Do not resume scanning inside an unsupported substitution using
            // the outer quote state: nested shell quoting has its own grammar.
            let end = literal_heredoc_substitution_end(command, at)?;
            let masked = masked.get_or_insert_with(|| bytes.to_vec());
            masked[at..end].fill(b'x');
            masked[at] = b'$'; // Still a dynamic argument, never a literal submit option.
            at = end;
            continue;
        }
        if quote == Some(byte) {
            quote = None;
        } else if quote.is_none() && matches!(byte, b'\'' | b'"') {
            quote = Some(byte);
        }
        at += 1;
    }
    masked.map(|bytes| String::from_utf8(bytes).expect("ASCII replacement at UTF-8 boundaries"))
}

/// Keep spans and command boundaries, unlike shell::shell_words which flattens
/// both. Quoted data is never searched for executable names.
fn submission_words(command: &str) -> Result<Vec<Vec<Word>>, String> {
    // Here-doc bodies are data, not command lines. Do not lex them as code.
    // The existing shell reader has the same stripping rule, but its flattened
    // token API cannot retain edit spans. Unsupported execution syntax fails
    // explicitly rather than being rewritten as though fully parsed.

    let masked = mask_literal_heredoc_substitutions(command);
    let command = masked.as_deref().unwrap_or(command);
    let mut commands = Vec::new();
    let mut words = Vec::new();
    let mut chars = command.char_indices().peekable();
    while let Some(&(start, ch)) = chars.peek() {
        if ch == '#' {
            for (_, c) in chars.by_ref() {
                if c == '\n' {
                    break;
                }
            }
            if !words.is_empty() {
                commands.push(std::mem::take(&mut words));
            }
            continue;
        }
        if ch.is_whitespace() || ";|&()".contains(ch) {
            chars.next();
            if (ch == '\n' || ";|&()".contains(ch)) && !words.is_empty() {
                commands.push(std::mem::take(&mut words));
            }
            continue;
        }
        let mut value = String::new();
        let mut quote = None;
        let mut dynamic = false;
        let mut end = start;
        while let Some(&(at, c)) = chars.peek() {
            if quote.is_none() && (c.is_whitespace() || ";|&()".contains(c)) {
                break;
            }
            chars.next();
            end = at + c.len_utf8();
            if c == '\\' && quote != Some('\'') {
                if let Some((at, c)) = chars.next() {
                    end = at + c.len_utf8();
                    if c != '\n' {
                        value.push(c);
                    }
                } else {
                    dynamic = true;
                }
            } else if Some(c) == quote {
                quote = None;
            } else if quote.is_none() && matches!(c, '\'' | '"') {
                quote = Some(c);
            } else {
                dynamic |= quote != Some('\'') && matches!(c, '$' | '`' | '<' | '>');
                value.push(c);
            }
        }
        dynamic |= quote.is_some();
        if !dynamic {
            let decoded = crate::tools::build::parse_direct_argv(&command[start..end])?;
            if decoded.len() != 1 {
                return Err("submission refused: ambiguous shell word".into());
            }
            value = decoded[0].clone();
        }
        words.push(Word {
            start,
            end,
            value,
            dynamic,
        });
    }
    if !words.is_empty() {
        commands.push(words);
    }
    let mut submissions = Vec::new();
    for mut words in commands {
        let mut at = 0;
        while at < words.len()
            && (words[at]
                .value
                .split_once('=')
                .is_some_and(|(key, _)| crate::harness::shell_verifier::valid_env_key(key))
                || matches!(words[at].value.as_str(), "env" | "command" | "exec"))
        {
            at += 1;
        }
        let Some(executable) = words.get(at) else {
            continue;
        };
        let name = Path::new(&executable.value)
            .file_name()
            .and_then(|p| p.to_str())
            .unwrap_or("");
        if matches!(name, "sudo" | "timeout" | "nice" | "nohup" | "xargs")
            && words
                .iter()
                .any(|w| matches!(w.value.as_str(), "yukon" | "hilbert"))
            && words.iter().any(|w| w.value == "submit")
        {
            return Err(
                "submission refused: executable wrapper arguments are not statically attributable"
                    .into(),
            );
        }
        if !matches!(name, "yukon" | "hilbert") {
            if words.iter().any(|word| {
                word.dynamic
                    && (word.value.contains("$(") || word.value.contains('`'))
                    && (submit_re().is_match(&word.value)
                        || word.value.split_whitespace().any(|part| part == "submit"))
            }) {
                return Err(
                    "submission refused: executable command substitution is not statically attributable; run the submission as a separate literal command".into(),
                );
            }
            if words.iter().any(|w| w.dynamic && w.value.contains("<<")) {
                // A heredoc's remaining lines are not ordinary shell commands.
                // Reject only if executable submission follows its delimiter;
                // the payload itself is never searched or modified.
                let header_end = command[words[0].start..]
                    .find('\n')
                    .map(|n| words[0].start + n)
                    .unwrap_or(command.len());
                let marker_at = words
                    .iter()
                    .position(|word| word.dynamic && word.value.starts_with("<<"))
                    .ok_or("submission refused: ambiguous heredoc command structure")?;
                let marker = &words[marker_at];
                let marker_raw = &command[marker.start..marker.end];
                let raw_start = marker.start + if marker_raw.starts_with("<<-") { 3 } else { 2 };
                let (delimiter_start, delimiter_end) = if raw_start == marker.end {
                    let next = words
                        .get(marker_at + 1)
                        .ok_or("submission refused: missing heredoc delimiter")?;
                    (next.start, next.end)
                } else {
                    (raw_start, marker.end)
                };
                if delimiter_end > header_end || delimiter_start >= delimiter_end {
                    return Err("submission refused: ambiguous heredoc command structure".into());
                }
                let delimiter = crate::tools::build::parse_direct_argv(
                    &command[delimiter_start..delimiter_end],
                )
                .ok()
                .and_then(|w| (w.len() == 1).then(|| w[0].clone()));
                let Some(delimiter) = delimiter else {
                    return Err("submission refused: ambiguous heredoc command structure".into());
                };
                let quoted_delimiter =
                    command[delimiter_start..delimiter_end].contains(['\'', '"', '\\']);
                // A pipeline or output redirection belongs to the header, not
                // the delimiter. Live proof commands use `<<'EOF' 2>&1 | head`.
                // Still inspect that executable tail: a submit in the pipeline
                // must retain the same attribution checks as a later command.
                let header_tail = &command[delimiter_end..header_end];
                if header_tail.contains("<<") {
                    return Err(
                        "submission refused: multiple heredocs are not statically attributable"
                            .into(),
                    );
                }
                let mut header_submissions = submission_words(header_tail)?;
                for words in &mut header_submissions {
                    for word in words {
                        word.start += delimiter_end;
                        word.end += delimiter_end;
                    }
                }
                submissions.extend(header_submissions);
                let body_start = (header_end + 1).min(command.len());
                let mut suffix_start = body_start;
                let mut found = false;
                for line in command[body_start..].split_inclusive('\n') {
                    suffix_start += line.len();
                    if line.trim_end_matches('\n').trim_start_matches('\t') == delimiter {
                        found = true;
                        break;
                    }
                }
                if !found {
                    return Ok(submissions);
                }
                // Only quoted heredocs make every body byte literal. An
                // unquoted body can execute substitutions despite looking like
                // a document; refuse potential nested submissions rather than
                // silently skipping their attribution along with the text.
                let body = &command[body_start..suffix_start];
                if !quoted_delimiter
                    && (body.contains("$(") || body.contains('`'))
                    && submit_re().is_match(body)
                {
                    return Err("submission refused: executable heredoc expansion is not statically attributable".into());
                }
                let mut suffix = submission_words(&command[suffix_start..])?;
                for words in &mut suffix {
                    for word in words {
                        word.start += suffix_start;
                        word.end += suffix_start;
                    }
                }
                submissions.extend(suffix);
                return Ok(submissions);
            }
            if (executable.dynamic || matches!(name, "eval" | "sh" | "bash" | "zsh"))
                && words
                    .iter()
                    .any(|w| w.value == "submit" || submit_re().is_match(&w.value))
            {
                return Err(
                    "submission refused: dynamic shell submission is not statically attributable"
                        .into(),
                );
            }
            continue;
        }
        if words.get(at + 1).map(|w| w.value.as_str()) != Some("submit") {
            continue;
        }
        if words.iter().any(|w| w.dynamic) {
            return Err(
                "submission refused: dynamic submission arguments are not statically attributable"
                    .into(),
            );
        }
        words.drain(..at);
        submissions.push(words);
    }
    Ok(submissions)
}

fn note_line_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?im)^([ \t>*_-]*(model|harness)[ \t*_]*:[ \t*_]*)(.*)$")
            .expect("note line regex")
    })
}

/// Never substitute configured defaults or first-request provenance for a seat.
pub(crate) fn model_label() -> Result<String, String> {
    crate::harness::run_identity::live_model()
        .ok_or_else(|| "submission refused: no live executing model identity".into())
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// Rewrite a shell command that submits to a board so its attribution flags
/// carry the live identity. `None` when the command is not a submission.
pub(crate) fn stamp(command: &str, workspace: Option<&Path>) -> Result<Option<Stamped>, String> {
    let submissions = submission_words(command)?;
    if submissions.is_empty() {
        return Ok(None);
    }
    let model = match model_label() {
        Ok(model) => model,
        Err(error) => {
            journal_execution("identity_guard", command, workspace, None, &error);
            return Err(error);
        }
    };
    let mut edits = Vec::new();
    for words in &submissions {
        edits.push((
            words[1].end,
            words[1].end,
            format!(
                " --model {} --harness {}",
                shell_quote(&model),
                shell_quote(HARNESS_LABEL)
            ),
        ));
        let mut at = 2;
        while at < words.len() {
            let word = &words[at];
            if word.value == "--" {
                break;
            }
            if matches!(
                word.value.as_str(),
                "--note"
                    | "--note-file"
                    | "--name"
                    | "--file"
                    | "--problem"
                    | "--track"
                    | "--commit"
            ) {
                if words.get(at + 1).is_none() {
                    return Err("submission refused: missing submission option value".into());
                }
                at += 2;
                continue;
            }
            if matches!(word.value.as_str(), "--model" | "--harness") {
                let Some(value) = words.get(at + 1) else {
                    return Err("submission refused: missing attribution flag value".into());
                };
                if value.value.starts_with("--") {
                    return Err("submission refused: missing attribution flag value".into());
                }
                edits.push((word.start, value.end, String::new()));
                at += 2;
            } else {
                if word.value.starts_with("--model=") || word.value.starts_with("--harness=") {
                    edits.push((word.start, word.end, String::new()));
                }
                at += 1;
            }
        }
    }
    edits.sort_by_key(|edit| edit.0);
    let mut stamped = command.to_string();
    for (start, end, replacement) in edits.into_iter().rev() {
        stamped.replace_range(start..end, &replacement);
    }
    let mut notice = format!(
        "[submission identity] model={model} harness={HARNESS_LABEL} — --model/--harness are stamped by the harness from the live run identity; model-supplied values are discarded"
    );
    for words in &submissions {
        let segment = &command[words[0].start..words.last().unwrap().end];
        let prefix = command[..words[0].start].trim();
        let anchor = prefix
            .strip_suffix("&&")
            .and_then(|prefix| crate::tools::build::parse_direct_argv(prefix.trim()).ok())
            .filter(|argv| argv.len() == 2 && argv[0] == "cd")
            .map(|argv| expand_home(&argv[1]))
            .and_then(|dir| {
                if dir.is_absolute() {
                    Some(dir)
                } else {
                    workspace.map(|w| w.join(dir))
                }
            });
        // More complex directory transitions cannot authenticate a note path.
        if !prefix.is_empty() && anchor.is_none() {
            continue;
        }
        if let Some(path) = note_file_path(segment, anchor.as_deref().or(workspace)) {
            match correct_note_file(&path, &model) {
                Ok(0) => {}
                Ok(n) => notice.push_str(&format!(
                    "; note file {}: {n} Model/Harness line(s) corrected",
                    path.display()
                )),
                Err(error) => notice.push_str(&format!(
                    "; note file {}: not corrected ({error})",
                    path.display()
                )),
            }
        }
    }
    Ok(Some(Stamped {
        command: stamped,
        notice,
    }))
}

fn expand_home(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = std::env::var_os("HOME")
    {
        return PathBuf::from(home).join(rest);
    }
    PathBuf::from(path)
}

fn note_file_path(command: &str, workspace: Option<&Path>) -> Option<PathBuf> {
    let words = crate::tools::build::parse_direct_argv(command).ok()?;
    let mut raw = None;
    let mut at = 2;
    while at < words.len() {
        if words[at] == "--" {
            break;
        }
        if matches!(
            words[at].as_str(),
            "--note"
                | "--model"
                | "--harness"
                | "--name"
                | "--file"
                | "--problem"
                | "--track"
                | "--commit"
        ) {
            at += 2;
            continue;
        }
        if words[at] == "--note-file" {
            raw = words.get(at + 1).map(String::as_str);
            break;
        }
        if let Some(value) = words[at].strip_prefix("--note-file=") {
            raw = Some(value);
            break;
        }
        // A quoted prose argument containing --note-file is not a flag.
        at += 1;
    }
    let raw = raw?;
    let path = expand_home(raw);
    if path.is_absolute() {
        return Some(path);
    }
    // The caller supplies a certified anchor; never infer cd from quoted prose.
    if raw.contains('$') {
        return None;
    }
    Some(workspace?.join(path))
}

/// Replace the values of `Model:` / `Harness:` lines (any markdown emphasis)
/// with the live identity. Returns the number of lines corrected.
fn correct_note_file(path: &Path, model: &str) -> std::io::Result<usize> {
    let text = std::fs::read_to_string(path)?;
    let mut corrected = 0usize;
    let rewritten = note_line_re().replace_all(&text, |caps: &regex::Captures| {
        let value = if caps[2].eq_ignore_ascii_case("model") {
            model
        } else {
            HARNESS_LABEL
        };
        let current = caps[3].trim().trim_end_matches("**").trim();
        if current == value {
            return caps[0].to_string();
        }
        corrected += 1;
        let closing = if caps[3].trim_end().ends_with("**") {
            "**"
        } else {
            ""
        };
        format!("{}{value}{closing}", &caps[1])
    });
    if corrected > 0 {
        std::fs::write(path, rewritten.as_ref())?;
    }
    Ok(corrected)
}

#[cfg(test)]
#[path = "../../../tests/cockpit/tools/submit_identity__tests.rs"]
mod tests;
