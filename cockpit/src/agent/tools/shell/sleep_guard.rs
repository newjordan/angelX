//! Recognize literal synchronous sleep commands without treating quoted data as
//! executable shell syntax. Dynamic shell programs remain bounded by the runner.

use std::path::Path;

pub(super) fn contains_excessive_sleep(command: &str, max_secs: u64) -> bool {
    excessive_sleep(command, max_secs as f64, 0)
}

fn excessive_sleep(command: &str, max_secs: f64, depth: usize) -> bool {
    // Shell -c payloads can themselves invoke a shell. Keep static inspection
    // bounded even for an adversarially nested literal command.
    if depth >= 8 {
        return false;
    }
    let command = super::strip_shell_heredoc_bodies(command);
    let command = super::strip_unquoted_shell_comments(&command);
    commands(&command).iter().any(|words| {
        let mut at = 0;
        while let Some(word) = words.get(at) {
            match word.as_str() {
                "!" | "{" | "if" | "elif" | "then" | "else" | "while" | "until" | "do" => at += 1,
                "time" => {
                    at += 1;
                    if words.get(at).is_some_and(|word| word == "-p") {
                        at += 1;
                    }
                    if words.get(at).is_some_and(|word| word == "--") {
                        at += 1;
                    }
                }
                word if assignment(word) => at += 1,
                _ => break,
            }
        }
        while let Some(word) = words.get(at) {
            let name = Path::new(word).file_name().and_then(|name| name.to_str());
            match name {
                Some(wrapper @ ("command" | "exec" | "env" | "nohup")) => {
                    at += 1;
                    while let Some(word) = words.get(at) {
                        if wrapper == "env" {
                            let split = match word.as_str() {
                                "-S" | "--split-string" => {
                                    at += 1;
                                    words.get(at).map(String::as_str)
                                }
                                option => option
                                    .strip_prefix("--split-string=")
                                    .or_else(|| option.strip_prefix("-S")),
                            };
                            if let Some(payload) = split {
                                let mut payload = payload.to_string();
                                for argument in &words[at + 1..] {
                                    payload.push(' ');
                                    payload.push_str(argument);
                                }
                                return excessive_sleep(&payload, max_secs, depth + 1);
                            }
                        }
                        if word == "--" {
                            at += 1;
                            break;
                        }
                        if wrapper == "env" && assignment(word) {
                            at += 1;
                            continue;
                        }
                        let takes_value = match wrapper {
                            "exec" => word == "-a",
                            "env" => matches!(
                                word.as_str(),
                                "-u" | "--unset" | "-C" | "--chdir" | "-a" | "--argv0"
                            ),
                            _ => false,
                        };
                        let flag = match wrapper {
                            "command" => word == "-p",
                            "exec" => matches!(word.as_str(), "-c" | "-l"),
                            "env" => {
                                matches!(word.as_str(), "-i" | "--ignore-environment")
                                    || word.starts_with("--unset=")
                                    || word.starts_with("--chdir=")
                                    || word.starts_with("--argv0=")
                            }
                            _ => false,
                        };
                        if takes_value || flag {
                            at += 1 + usize::from(takes_value);
                        } else if word.starts_with('-') {
                            // Help, lookup (-v/-V), and unknown options do not
                            // establish that this wrapper executes a payload.
                            return false;
                        } else {
                            break;
                        }
                    }
                }
                Some("busybox") => at += 1,
                Some("timeout" | "gtimeout" | "nice" | "stdbuf") => {
                    let wrapper = name.unwrap();
                    at += 1;
                    while let Some(option) = words.get(at).filter(|word| word.starts_with('-')) {
                        if option == "--" {
                            at += 1;
                            break;
                        }
                        if matches!(option.as_str(), "--help" | "--version") {
                            return false;
                        }
                        let takes_value = match wrapper {
                            "timeout" | "gtimeout" => {
                                matches!(option.as_str(), "-k" | "--kill-after" | "-s" | "--signal")
                            }
                            "nice" => matches!(option.as_str(), "-n" | "--adjustment"),
                            "stdbuf" => matches!(
                                option.as_str(),
                                "-i" | "--input" | "-o" | "--output" | "-e" | "--error"
                            ),
                            _ => false,
                        };
                        at += 1 + usize::from(takes_value);
                    }
                    if matches!(wrapper, "timeout" | "gtimeout") {
                        if words
                            .get(at)
                            .is_none_or(|word| duration_seconds(word).is_none())
                        {
                            return false;
                        }
                        at += 1;
                    }
                }
                Some("eval") => {
                    return excessive_sleep(&words[at + 1..].join(" "), max_secs, depth + 1);
                }
                Some("ssh") => {
                    at += 1;
                    while let Some(option) = words.get(at).filter(|word| word.starts_with('-')) {
                        if option == "--" {
                            at += 1;
                            break;
                        }
                        if matches!(option.as_str(), "-N" | "-G" | "-V") {
                            return false;
                        }
                        // Attached values (-p22/-oBatchMode=yes) occupy one
                        // word; these exact options consume the following word.
                        let takes_value = matches!(
                            option.as_str(),
                            "-B" | "-b"
                                | "-c"
                                | "-D"
                                | "-E"
                                | "-e"
                                | "-F"
                                | "-I"
                                | "-i"
                                | "-J"
                                | "-L"
                                | "-l"
                                | "-m"
                                | "-O"
                                | "-o"
                                | "-p"
                                | "-R"
                                | "-S"
                                | "-W"
                                | "-w"
                        );
                        at += 1 + usize::from(takes_value);
                    }
                    // The first non-option is the destination. Remaining
                    // arguments form the literal remote shell command.
                    return words.get(at + 1..).is_some_and(|payload| {
                        excessive_sleep(&payload.join(" "), max_secs, depth + 1)
                    });
                }
                Some("sleep") => {
                    let mut seconds = 0.0;
                    for operand in &words[at + 1..] {
                        if matches!(operand.as_str(), "--help" | "--version") {
                            return false;
                        }
                        if operand == "--" {
                            continue;
                        }
                        if let Some(duration) = duration_seconds(operand) {
                            seconds += duration;
                        } else {
                            // Invalid or dynamic operands are not a proven
                            // literal wait; let the runner observe the result.
                            return false;
                        }
                    }
                    return seconds > max_secs;
                }
                Some("bash" | "sh" | "dash" | "zsh" | "ksh") => {
                    at += 1;
                    while let Some(option) = words.get(at) {
                        if matches!(option.as_str(), "--" | "--help" | "--version")
                            || !option.starts_with('-')
                        {
                            return false;
                        }
                        if !option.starts_with("--") && option[1..].contains('c') {
                            return words.get(at + 1).is_some_and(|payload| {
                                excessive_sleep(payload, max_secs, depth + 1)
                            });
                        }
                        at += 1 + usize::from(matches!(
                            option.as_str(),
                            "-o" | "-O" | "--rcfile" | "--init-file"
                        ));
                    }
                    return false;
                }
                _ => return false,
            }
        }
        false
    })
}

fn assignment(word: &str) -> bool {
    let Some((name, _)) = word.split_once('=') else {
        return false;
    };
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn duration_seconds(operand: &str) -> Option<f64> {
    let (number, multiplier) = match operand.as_bytes().last()? {
        b's' => (&operand[..operand.len() - 1], 1.0),
        b'm' => (&operand[..operand.len() - 1], 60.0),
        b'h' => (&operand[..operand.len() - 1], 3600.0),
        b'd' => (&operand[..operand.len() - 1], 86400.0),
        _ => (operand, 1.0),
    };
    let seconds = number.parse::<f64>().ok()? * multiplier;
    (seconds >= 0.0).then_some(seconds)
}

/// Split only at unquoted shell control operators, retaining quoted/escaped
/// words and discarding redirection targets. This is a literal-command scanner,
/// not an interpreter: no substitutions or shell expansion are performed.
fn commands(command: &str) -> Vec<Vec<String>> {
    let mut commands = Vec::new();
    let mut words = Vec::new();
    let mut word = String::new();
    let mut started = false;
    let mut quote = None;
    let mut escaped = false;
    let mut redirect_target = false;
    let mut descriptor_literal = true;
    let flush = |word: &mut String,
                 started: &mut bool,
                 words: &mut Vec<String>,
                 redirect_target: &mut bool| {
        if *started {
            if *redirect_target {
                word.clear();
                *redirect_target = false;
            } else {
                words.push(std::mem::take(word));
            }
            *started = false;
        }
    };
    let mut chars = command.chars().peekable();
    while let Some(ch) = chars.next() {
        if escaped {
            if ch != '\n' {
                word.push(ch);
                started = true;
            }
            escaped = false;
            continue;
        }
        if ch == '\\' && quote != Some('\'') {
            if quote == Some('"')
                && chars
                    .peek()
                    .is_some_and(|next| !matches!(next, '$' | '`' | '"' | '\\' | '\n'))
            {
                word.push(ch);
                continue;
            }
            escaped = true;
            descriptor_literal = false;
            continue;
        }
        if quote == Some(ch) {
            quote = None;
            continue;
        }
        if quote.is_none() && matches!(ch, '\'' | '"') {
            quote = Some(ch);
            started = true;
            descriptor_literal = false;
            continue;
        }
        if quote.is_some() {
            word.push(ch);
            continue;
        }
        if matches!(ch, '<' | '>') || (ch == '&' && chars.peek() == Some(&'>')) {
            // A descriptor immediately adjoining the operator is syntax, not
            // the executable (e.g. `2>/dev/null sleep 30`).
            if started && descriptor_literal && word.bytes().all(|byte| byte.is_ascii_digit()) {
                word.clear();
                started = false;
            }
            flush(&mut word, &mut started, &mut words, &mut redirect_target);
            while chars.peek().is_some_and(|ch| matches!(ch, '<' | '>')) {
                chars.next();
            }
            if chars.peek() == Some(&'&') {
                chars.next();
            }
            redirect_target = true;
            descriptor_literal = true;
        } else if ch == '\n' || matches!(ch, ';' | '|' | '&' | '(' | ')') {
            flush(&mut word, &mut started, &mut words, &mut redirect_target);
            if !words.is_empty() {
                commands.push(std::mem::take(&mut words));
            }
            redirect_target = false;
            descriptor_literal = true;
        } else if ch.is_whitespace() {
            flush(&mut word, &mut started, &mut words, &mut redirect_target);
            descriptor_literal = true;
        } else {
            word.push(ch);
            started = true;
        }
    }
    flush(&mut word, &mut started, &mut words, &mut redirect_target);
    if !words.is_empty() {
        commands.push(words);
    }
    commands
}
