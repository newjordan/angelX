//! The numbers-only guard. A model that can check a change cheaply slides into moving one
//! numeric constant a step at a time against the checker and keeping the lowest value that
//! passes once. When the checker samples its inputs, that value passed by luck and fails on
//! another draw, so it is not a result.
//!
//! The guard reads each hop's calls in order. A change is numbers-only when its old and new
//! text are the same once every numeric literal is masked: an edit tool's `old` and `new`, a
//! patch's removed and added lines, an in-place `sed` or `perl` substitution, where a shell
//! variable written in a number's place counts as a number. A round is such a change followed
//! by a test or measurement run, in the same hop or after hops that edited nothing. An edit
//! that cannot be shown to be numbers-only starts the count again; reads, searches, notes and
//! other commands leave it alone. The count reaching [`FIRST_NOTICE`], and every
//! [`NOTICE_EVERY`] rounds after that, earns one harness turn in plain words.
use super::*;

pub(super) const FIRST_NOTICE: usize = 3;
pub(super) const NOTICE_EVERY: usize = 4;
const MASK: char = '\0';
/// Stands for the `$` of a variable the shell expands, so a sed script can tell it from a `$`
/// that reaches sed as written.
const EXPANSION: char = '\u{1}';

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Change {
    Numbers,
    Structural,
}

#[derive(Default)]
pub(super) struct NumberSweep {
    streak: usize,
    unchecked: bool,
}

impl NumberSweep {
    pub(super) fn observe<'a>(
        &mut self,
        hop: impl IntoIterator<Item = (&'a ToolCall, ExecutionOutcome)>,
    ) -> Option<String> {
        let mut due = false;
        for (call, execution) in hop {
            let landed = execution == ExecutionOutcome::Succeeded;
            // A red check still ran; a call that was denied or never started did nothing.
            if !landed && execution != ExecutionOutcome::Failed {
                continue;
            }
            // A change waits for its check through any hops that edit nothing.
            match change_of(call, landed) {
                Some(Change::Structural) => {
                    self.streak = 0;
                    self.unchecked = false;
                }
                Some(Change::Numbers) => self.unchecked = true,
                None => {}
            }
            if self.unchecked && is_check(call) {
                self.unchecked = false;
                self.streak += 1;
                due |= self.streak >= FIRST_NOTICE
                    && (self.streak - FIRST_NOTICE).is_multiple_of(NOTICE_EVERY);
            }
        }
        due.then(|| notice(self.streak))
    }
}

fn notice(rounds: usize) -> String {
    format!(
        "The last {rounds} changes altered only numbers, each followed by a check. If that check \
         samples its inputs, the lowest setting that passes once is not a result: it will fail on \
         another draw. Keep the last setting that passed with room to spare, record it, and make \
         the next change a structural one."
    )
}

fn shell_command(call: &ToolCall) -> Option<&str> {
    matches!(call.name.as_str(), "shell" | "bash" | "proc_run")
        .then(|| crate::agent::tools::shell::shell_command_arg(&call.args))
        .flatten()
}

fn is_check(call: &ToolCall) -> bool {
    is_verification_call(call) || shell_command(call).is_some_and(mentions_check_run)
}

fn change_of(call: &ToolCall, landed: bool) -> Option<Change> {
    match shell_command(call) {
        Some(command) => shell_change(command),
        // An edit tool that returned an error wrote nothing.
        None if landed => tool_change(call),
        None => None,
    }
}

pub(super) fn tool_change(call: &ToolCall) -> Option<Change> {
    let args = &call.args;
    let (mut targets, mut prose) = (0usize, 0usize);
    crate::knowledge::cut::for_each_mutation_target_path(&call.name, args, |path| {
        targets += 1;
        prose += usize::from(is_prose_only_path(path));
        false
    });
    // Notes and docs are not the thing under check: writing one is neither kind of change.
    if targets > 0 && prose == targets {
        return None;
    }
    match call.name.as_str() {
        "str_replace" => pairs_change([(args["old"].as_str()?, args["new"].as_str()?)]),
        "multi_edit" => pairs_change(
            args["edits"]
                .as_array()?
                .iter()
                .filter_map(|edit| Some((edit["old"].as_str()?, edit["new"].as_str()?))),
        ),
        "apply_patch" => match patch_pairs(args["diff"].as_str()?) {
            Some(pairs) => pairs_change(pairs),
            None => Some(Change::Structural),
        },
        // A whole-file write carries no old text to compare.
        "write_file" => Some(Change::Structural),
        _ => None,
    }
}

fn pairs_change<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str)>) -> Option<Change> {
    let mut numbers = false;
    for (old, new) in pairs {
        if old == new {
            continue;
        }
        if mask_numbers(old) != mask_numbers(new) {
            return Some(Change::Structural);
        }
        numbers = true;
    }
    numbers.then_some(Change::Numbers)
}

/// The removed and added lines of a patch, paired in order. `None` when a block removes and
/// adds different numbers of lines, or the patch shows no removed text at all.
fn patch_pairs(diff: &str) -> Option<Vec<(&str, &str)>> {
    fn pair_up<'a>(
        removed: &mut Vec<&'a str>,
        added: &mut Vec<&'a str>,
        pairs: &mut Vec<(&'a str, &'a str)>,
    ) -> bool {
        let even = removed.len() == added.len();
        pairs.extend(removed.drain(..).zip(added.drain(..)));
        even
    }
    let (mut removed, mut added, mut pairs) = (Vec::new(), Vec::new(), Vec::new());
    let mut lines = diff.lines().peekable();
    while let Some(line) = lines.next() {
        let file_header =
            line.starts_with("--- ") && lines.next_if(|next| next.starts_with("+++ ")).is_some();
        let old = line.strip_prefix('-').filter(|_| !file_header);
        let new = line.strip_prefix('+');
        if (old.is_none() && new.is_none() || old.is_some() && !added.is_empty())
            && !pair_up(&mut removed, &mut added, &mut pairs)
        {
            return None;
        }
        removed.extend(old);
        added.extend(new);
    }
    (pair_up(&mut removed, &mut added, &mut pairs) && !pairs.is_empty()).then_some(pairs)
}

pub(super) fn shell_change(command: &str) -> Option<Change> {
    let mut found = None;
    for words in shell_simple_commands_marking(command, Some(EXPANSION)) {
        match in_place_change(&words) {
            Some(Change::Structural) => return Some(Change::Structural),
            Some(Change::Numbers) => found = Some(Change::Numbers),
            None => {}
        }
    }
    found
}

/// What an in-place `sed` or `perl` among these words changes. `None` when there is none, or
/// its substitutions replace text with the same text.
fn in_place_change(words: &[String]) -> Option<Change> {
    let program = |word: &String| word.rsplit('/').next().unwrap_or(word).to_string();
    let at = words
        .iter()
        .position(|word| matches!(program(word).as_str(), "sed" | "gsed" | "perl"))?;
    let perl = program(&words[at]) == "perl";
    let (mut in_place, mut extended, mut unreadable) = (false, perl, false);
    let (mut scripts, mut operands) = (Vec::new(), Vec::new());
    let mut args = words[at + 1..].iter().map(String::as_str);
    while let Some(arg) = args.next() {
        if let Some(long) = arg.strip_prefix("--") {
            match long.split_once('=').unwrap_or((long, "")) {
                ("in-place", _) => in_place = true,
                ("regexp-extended", _) => extended = true,
                ("expression", "") => scripts.extend(args.next()),
                ("expression", script) => scripts.push(script),
                ("file", _) => unreadable = true,
                _ => {}
            }
        } else if let Some(flags) = arg.strip_prefix('-').filter(|flags| !flags.is_empty()) {
            if perl && flags.starts_with(['M', 'm', 'I']) {
                continue;
            }
            // `-i` takes the rest of its cluster as the backup suffix.
            let flags = match flags.split_once('i') {
                Some((before, _)) => {
                    in_place = true;
                    before
                }
                None => flags,
            };
            extended |= flags.contains(['E', 'r']);
            match flags.chars().last() {
                Some('e') => scripts.extend(args.next()),
                Some('f') => unreadable = true,
                _ => {}
            }
        } else {
            operands.push(arg);
        }
    }
    if !in_place {
        return None;
    }
    if scripts.is_empty() && !perl {
        scripts.extend(operands.first());
    }
    if unreadable || scripts.is_empty() {
        return Some(Change::Structural);
    }
    let mut numbers = false;
    for script in scripts {
        // A script with anything but substitutions in it deletes, inserts or rewrites lines.
        let Some(found) = substitutions(script) else {
            return Some(Change::Structural);
        };
        for (pattern, replacement) in found {
            let pattern = pattern_text(&pattern, extended);
            let (new, variable) = replacement_text(&replacement, &pattern.text, &pattern.groups);
            if !pattern.wild && !variable && pattern.text == new {
                continue;
            }
            let (old, new) = (mask_numbers(&pattern.text), mask_numbers(&new));
            let same = match new.strip_prefix(old.as_str()) {
                // What an open-ended pattern took from the line has to come back as a number
                // and its punctuation (`= 5;`), with no words in it.
                Some(rest) if pattern.open_end => {
                    rest.contains(MASK)
                        && !rest.contains(|ch: char| ch.is_alphanumeric() || ch == '_')
                }
                Some(rest) => rest.is_empty(),
                None => false,
            };
            if !same {
                return Some(Change::Structural);
            }
            numbers = true;
        }
    }
    numbers.then_some(Change::Numbers)
}

/// The pattern and replacement of every `s` command in a sed script, with the delimiter
/// unescaped. `None` when the script holds any other command or does not parse.
fn substitutions(script: &str) -> Option<Vec<(String, String)>> {
    let chars: Vec<char> = script.chars().collect();
    let mut found = Vec::new();
    let mut at = 0;
    loop {
        while chars
            .get(at)
            .is_some_and(|ch| ch.is_whitespace() || *ch == ';')
        {
            at += 1;
        }
        if at == chars.len() {
            break;
        }
        at = address_end(&chars, at)?;
        let delimiter = *chars.get(at + 1)?;
        if chars[at] != 's'
            || delimiter.is_alphanumeric()
            || delimiter.is_whitespace()
            || delimiter == '\\'
        {
            return None;
        }
        at += 2;
        let pattern = field(&chars, &mut at, delimiter)?;
        let replacement = field(&chars, &mut at, delimiter)?;
        while chars.get(at).is_some_and(char::is_ascii_alphanumeric) {
            // `w` writes the changed lines to a file named by the rest of the script.
            if chars[at] == 'w' {
                return None;
            }
            at += 1;
        }
        found.push((pattern, replacement));
    }
    (!found.is_empty()).then_some(found)
}

/// Past the line address in front of a sed command: `12`, `$`, `/re/`, a `first,last` pair.
fn address_end(chars: &[char], mut at: usize) -> Option<usize> {
    for _ in 0..2 {
        match chars.get(at)? {
            '$' => at += 1,
            '/' => {
                at += 1;
                while *chars.get(at)? != '/' {
                    at += 1 + usize::from(chars[at] == '\\');
                }
                at += 1;
            }
            ch if ch.is_ascii_digit() => {
                while chars.get(at).is_some_and(char::is_ascii_digit) {
                    at += 1;
                }
            }
            _ => break,
        }
        if chars.get(at) != Some(&',') {
            break;
        }
        at += 1 + usize::from(matches!(chars.get(at + 1), Some('+' | '~')));
    }
    while chars.get(at).is_some_and(|ch| *ch == ' ' || *ch == '!') {
        at += 1;
    }
    Some(at)
}

fn field(chars: &[char], at: &mut usize, delimiter: char) -> Option<String> {
    let mut text = String::new();
    loop {
        let ch = *chars.get(*at)?;
        *at += 1;
        if ch == delimiter {
            return Some(text);
        }
        if ch == '\\' {
            let next = *chars.get(*at)?;
            *at += 1;
            if next != delimiter {
                text.push('\\');
            }
            text.push(next);
        } else {
            text.push(ch);
        }
    }
}

struct Pattern {
    text: String,
    groups: Vec<String>,
    /// A wildcard was read, so equal text does not mean that nothing changes.
    wild: bool,
    /// The pattern ends in `.*`, which takes the rest of the line whatever it holds.
    open_end: bool,
}

/// A substitution's pattern as the text it stands for: anchors and group marks dropped,
/// escapes resolved, and a `0` wherever a wildcard can match a number (`[0-9]*`, `\d+`, `.*`),
/// so `^limit = [0-9]*` reads as `limit = 0`. A closing `.*` is left off and reported instead.
fn pattern_text(pattern: &str, extended: bool) -> Pattern {
    let chars: Vec<char> = pattern.chars().collect();
    let mut text = String::new();
    let mut groups: Vec<String> = Vec::new();
    let mut open: Vec<(usize, usize)> = Vec::new();
    let (mut wild, mut open_end) = (false, false);
    let mut at = 0;
    while at < chars.len() {
        let (ch, next) = (chars[at], chars.get(at + 1).copied());
        let mark = match (ch, next) {
            ('\\', Some(mark @ ('(' | ')'))) if !extended => Some((mark, 2)),
            (mark @ ('(' | ')'), _) if extended => Some((mark, 1)),
            _ => None,
        };
        if let Some((mark, width)) = mark {
            if mark == '(' {
                open.push((groups.len(), text.len()));
                groups.push(String::new());
            } else if let Some((group, start)) = open.pop() {
                groups[group] = text[start..].to_string();
            }
            at += width;
            continue;
        }
        let wildcard = match (ch, next) {
            ('^', _) if at == 0 => {
                at += 1;
                continue;
            }
            ('$', None) => break,
            ('\\', Some('d')) => Some(at + 2),
            ('\\', Some(literal)) if literal.is_ascii_punctuation() => {
                text.push(literal);
                at += 2;
                continue;
            }
            ('[', _) => digit_class_end(&chars, at),
            // A bare dot is most often a decimal point; only a repeated one is a wildcard.
            ('.', _) => Some(at + 1).filter(|end| quantifier_end(&chars, *end) > *end),
            _ => None,
        };
        match wildcard {
            Some(end) => {
                wild = true;
                at = quantifier_end(&chars, end);
                if ch == '.' && matches!(&chars[at..], [] | ['$']) {
                    open_end = true;
                    break;
                }
                text.push('0');
            }
            None => {
                text.push(ch);
                at += 1;
            }
        }
    }
    Pattern {
        text,
        groups,
        wild,
        open_end,
    }
}

/// Past the bracket expression at `at`, when it is a class that matches digits.
fn digit_class_end(chars: &[char], at: usize) -> Option<usize> {
    let mut end = at + 1;
    if chars.get(end) == Some(&'^') {
        return None;
    }
    end += usize::from(chars.get(end) == Some(&']'));
    while *chars.get(end)? != ']' {
        end += match &chars[end..] {
            ['[', ':', ..] => {
                chars[end..]
                    .windows(2)
                    .position(|pair| pair == [':', ']'])?
                    + 2
            }
            _ => 1,
        };
    }
    let class: String = chars[at + 1..end].iter().collect();
    (class.contains("0-9") || class.contains(":digit:")).then_some(end + 1)
}

fn quantifier_end(chars: &[char], mut at: usize) -> usize {
    loop {
        match chars.get(at..).unwrap_or_default() {
            ['*' | '+' | '?', ..] => at += 1,
            ['\\', '+' | '?', ..] => at += 2,
            ['{', rest @ ..] | ['\\', '{', rest @ ..] => {
                match rest.iter().position(|ch| *ch == '}') {
                    Some(close) => at = chars.len() - rest.len() + close + 1,
                    None => return at,
                }
            }
            _ => return at,
        }
    }
}

/// A substitution's replacement as the text it writes: escapes resolved, `&` and group
/// references filled in from the pattern, and a `0` for each shell variable, whose value is
/// taken to be a number when it stands where the pattern has one. With it, whether any
/// variable was read.
fn replacement_text(replacement: &str, whole: &str, groups: &[String]) -> (String, bool) {
    let mut text = String::new();
    let mut variable = false;
    let mut chars = replacement.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == EXPANSION {
            if chars.next_if_eq(&'{').is_some() {
                while chars.next().is_some_and(|ch| ch != '}') {}
            } else if chars.next_if(char::is_ascii_digit).is_none() {
                while chars
                    .next_if(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
                    .is_some()
                {}
            }
            text.push('0');
            variable = true;
            continue;
        }
        let group = matches!(ch, '\\' | '$')
            .then(|| chars.peek()?.to_digit(10)?.checked_sub(1))
            .flatten()
            .and_then(|index| groups.get(index as usize));
        match (ch, group) {
            (_, Some(group)) => {
                text.push_str(group);
                chars.next();
            }
            ('&', _) => text.push_str(whole),
            ('\\', _) => match chars.next() {
                Some(literal) if literal.is_ascii_punctuation() => text.push(literal),
                Some(other) => text.extend(['\\', other]),
                None => text.push('\\'),
            },
            _ => text.push(ch),
        }
    }
    (text, variable)
}

/// `text` with every numeric literal replaced by one mask character. A literal is a run of
/// digits with `_` separators, a fraction and an exponent, or a `0x`, `0o` or `0b` run. Digits
/// inside a name (`int32`, `sha256`, `step_2`) are part of the name and stay as written.
pub(super) fn mask_numbers(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let digit = |at: usize| chars.get(at).is_some_and(char::is_ascii_digit);
    let digits = |mut at: usize| {
        while digit(at) || (chars.get(at) == Some(&'_') && digit(at + 1)) {
            at += 1;
        }
        at
    };
    let mut masked = String::with_capacity(text.len());
    let mut at = 0;
    while at < chars.len() {
        let in_name = at > 0 && (chars[at - 1].is_alphanumeric() || chars[at - 1] == '_');
        if in_name || !digit(at) {
            masked.push(chars[at]);
            at += 1;
            continue;
        }
        let radix = chars[at] == '0'
            && matches!(chars.get(at + 1), Some('x' | 'X' | 'o' | 'O' | 'b' | 'B'))
            && chars.get(at + 2).is_some_and(char::is_ascii_hexdigit);
        if radix {
            at += 2;
            while chars
                .get(at)
                .is_some_and(|ch| ch.is_ascii_hexdigit() || *ch == '_')
            {
                at += 1;
            }
        } else {
            at = digits(at);
            if chars.get(at) == Some(&'.') && digit(at + 1) {
                at = digits(at + 1);
            }
            let sign = usize::from(matches!(chars.get(at + 1), Some('+' | '-')));
            if matches!(chars.get(at), Some('e' | 'E')) && digit(at + 1 + sign) {
                at = digits(at + 1 + sign);
            }
        }
        masked.push(MASK);
    }
    masked
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/harness/number_sweep__tests.rs"]
mod tests;
