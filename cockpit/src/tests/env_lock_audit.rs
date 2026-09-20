//! Source lint for direct process-env mutation in synchronous Rust unit tests.
//! This is lexical, not call-graph analysis. Fixture-owned locks require an
//! inline `env-lock-exempt: <reason>` comment in each affected test. Review those
//! annotations whenever changing the fixture; never add a second, nested lock.
use std::path::Path;

#[derive(Debug)]
struct Token<'a> {
    text: &'a str,
    offset: usize,
}

// Skip comments and literals before balancing braces: test fixtures often
// contain Rust source, JSON, shell scripts, and braces in raw strings.
fn tokens(source: &str) -> Vec<Token<'_>> {
    let literal = regex::Regex::new(
        r#"(?s)^(?:b|c)?"(?:\\.|[^"\\])*"|^b?'(?:\\(?:u\{[0-9a-fA-F_]+\}|x[0-9a-fA-F]{2}|.)|[^'\\])'"#,
    )
    .unwrap();
    let raw = regex::Regex::new(r##"^(?:b|c)?r(\#*)""##).unwrap();
    let word = regex::Regex::new(r"^[A-Za-z_][A-Za-z_0-9]*").unwrap();
    let mut out = Vec::new();
    let mut i = 0;
    while i < source.len() {
        let rest = &source[i..];
        if rest.starts_with("//") {
            i += rest.find('\n').unwrap_or(rest.len());
        } else if rest.starts_with("/*") {
            let mut depth = 1;
            i += 2;
            while i < source.len() && depth > 0 {
                if source[i..].starts_with("/*") {
                    depth += 1;
                    i += 2;
                } else if source[i..].starts_with("*/") {
                    depth -= 1;
                    i += 2;
                } else {
                    i += source[i..].chars().next().unwrap().len_utf8();
                }
            }
        } else if let Some(c) = raw.captures(rest) {
            let start = c.get(0).unwrap().end();
            let end = format!("\"{}", &c[1]);
            i += start + rest[start..].find(&end).expect("closed raw string") + end.len();
        } else if let Some(m) = literal.find(rest) {
            i += m.end();
        } else if let Some(m) = word.find(rest) {
            out.push(Token {
                text: m.as_str(),
                offset: i,
            });
            i += m.end();
        } else {
            let n = rest.chars().next().unwrap().len_utf8();
            if !rest[..n].chars().all(char::is_whitespace) {
                out.push(Token {
                    text: &rest[..n],
                    offset: i,
                });
            }
            i += n;
        }
    }
    out
}

fn violations(source: &str) -> Vec<usize> {
    let ts = tokens(source);
    let mut failures = Vec::new();
    for i in 0..ts.len().saturating_sub(3) {
        if ts[i..i + 4]
            .iter()
            .map(|t| t.text)
            .ne(["#", "[", "test", "]"])
        {
            continue;
        }
        let start = (i + 4..ts.len()).find(|&j| ts[j].text == "{").unwrap();
        let mut depth = 1;
        let mut end = start + 1;
        while depth > 0 {
            assert!(
                end < ts.len(),
                "unbalanced test body at byte {}",
                ts[start].offset
            );
            match ts[end].text {
                "{" => depth += 1,
                "}" => depth -= 1,
                _ => {}
            }
            end += 1;
        }
        let body = &ts[start + 1..end - 1];
        let mutation = (0..body.len()).find(|&j| {
            let rest = &body[j..];
            (rest.len() >= 2
                && matches!(rest[0].text, "set_var" | "remove_var")
                && rest[1].text == "(")
                || (rest.len() >= 5
                    && rest[0].text.ends_with("EnvGuard")
                    && rest[1].text == ":"
                    && rest[2].text == ":"
                    && matches!(rest[3].text, "set" | "unset")
                    && rest[4].text == "(")
        });
        let Some(mutation) = mutation else { continue };
        // A named guard in the test's outer scope, acquired before mutation,
        // remains alive until all subsequently declared restoration guards drop.
        let mut nesting = 0;
        let mut statement = 0;
        let mut locked = false;
        for j in 0..mutation {
            match body[j].text {
                "{" => nesting += 1,
                "}" => {
                    nesting -= 1;
                    if nesting == 0 {
                        statement = j + 1;
                    }
                }
                ";" if nesting == 0 => statement = j + 1,
                "env_lock" if nesting == 0 && body.get(j + 1).is_some_and(|t| t.text == "(") => {
                    locked = body[statement].text == "let"
                        && body.get(statement + 1).is_some_and(|t| t.text != "_");
                }
                _ => {}
            }
        }
        let original = &source[ts[start].offset..ts[end - 1].offset];
        let exempt = original.lines().any(|line| {
            line.trim()
                .strip_prefix("// env-lock-exempt:")
                .is_some_and(|reason| !reason.trim().is_empty())
        });
        if !locked && !exempt {
            failures.push(
                source[..body[mutation].offset]
                    .bytes()
                    .filter(|&b| b == b'\n')
                    .count()
                    + 1,
            );
        }
    }
    failures
}

fn scan(dir: &Path, failures: &mut Vec<String>, files: &mut usize) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(Result::unwrap)
        .collect();
    entries.sort_by_key(|e| e.path());
    for entry in entries {
        let kind = entry.file_type().unwrap();
        let path = entry.path();
        // Never follow symlinks out of the source tree.
        if kind.is_dir() {
            scan(&path, failures, files);
        } else if kind.is_file() && path.extension().is_some_and(|ext| ext == "rs") {
            *files += 1;
            let source = std::fs::read_to_string(&path).unwrap();
            let found = std::panic::catch_unwind(|| violations(&source))
                .unwrap_or_else(|_| panic!("source lint could not parse {}", path.display()));
            for line in found {
                failures.push(format!("{}:{line}", path.display()));
            }
        }
    }
}

#[test]
fn env_mutating_tests_hold_shared_lock() {
    let mut failures = Vec::new();
    let mut files = 0;
    scan(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut failures,
        &mut files,
    );
    assert!(files > 0, "source scan must not pass vacuously");
    assert!(
        failures.is_empty(),
        "env-mutating tests need a named outer-scope env_lock() guard before mutation:\n{}",
        failures.join("\n")
    );
}

#[test]
fn env_lock_lint_recognizes_mutations_and_ignores_literals() {
    for mutation in [
        "std::env::set_var(\"X\", \"1\")",
        "std::env::remove_var(\"X\")",
        "EnvGuard::set(\"X\", \"1\")",
        "EnvGuard::unset(\"X\")",
        "crate::tests::TestEnvGuard::set(\"X\", \"1\")",
    ] {
        let unguarded = format!("#[test]\nfn example() {{\n {mutation};\n}}");
        assert_eq!(violations(&unguarded), [3]);
        let guarded = format!(
            "#[test] fn example() {{ let _guard = crate::tests::env_lock(); {mutation}; }}"
        );
        assert!(violations(&guarded).is_empty());
        for lock in [
            "{ let _guard = env_lock(); }",
            "let _ = env_lock();",
            "// env_lock()\n",
        ] {
            assert_eq!(
                violations(&format!("#[test] fn example() {{ {lock} {mutation}; }}")).len(),
                1
            );
        }
        assert_eq!(
            violations(&format!(
                "#[test] fn example() {{ {mutation}; let _guard = env_lock(); }}"
            ))
            .len(),
            1
        );
    }
    assert!(violations("#[test] fn example() { if false { return; } struct Local {} let _guard = env_lock(); EnvGuard::unset(\"X\"); }").is_empty());
    assert!(
        violations(
            r###"#[test] fn fixture() {
        let _s = r##"#[test] fn fake() { std::env::set_var("X", "1"); }"##;
        /* nested /* std::env::set_var("X", "1"); */ comment */
        let _continued = "brace { \
            still literal";
        let _c = '}';
        let _s = "EnvGuard::set(\"X\", \"1\")";
    }"###
        )
        .is_empty()
    );
    assert!(violations("#[test] fn fixture() {\n// env-lock-exempt: fixture owns shared lock for closure.\nfixture(|| { EnvGuard::set(\"X\", \"1\"); }); }").is_empty());
}
