//! Check pieces of Delve content the way the game and its quality bar do,
//! and say what is wrong in plain words (`rules::content`, docs/TEMPLATES.md).
//!
//!     cargo run -p angel-delve --example content_check -- <file>...
//!     cargo run -p angel-delve --example content_check -- --builtin
//!     cargo run -p angel-delve --example content_check -- --templates
//!     cargo run -p angel-delve --example content_check -- --template wish-worn.wish
//!
//! A file's kind is its extension: `.wish` (a phrasebook entry), `.card`,
//! `.boss` (a foe), `.room` (a quadrant), `.vfx` (an effect recipe) or
//! `.pastime` (a pastime's spec). A phrasebook (`phrasebook.txt`) is checked
//! entry by entry. `--builtin` checks every piece the game carries and prints
//! those under the bar; `--all` prints every report; `--template <name>`
//! prints a template to start from. The exit code is 1 when anything named
//! is turned away or under the bar.

use angel_delve::rules::content::{self, Kind, Report};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: content_check <file>... | --builtin [--all] | --templates | --template <name>");
        std::process::exit(2);
    }
    let all = args.iter().any(|a| a == "--all");
    let mut reports: Vec<Report> = Vec::new();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--all" => {}
            "--builtin" => reports.extend(content::builtin()),
            "--templates" => reports.extend(content::templates()),
            "--template" => {
                let name = it.next().map(String::as_str).unwrap_or("");
                match content::template(name) {
                    Some(text) => print!("{text}"),
                    None => {
                        let names: Vec<&str> = content::TEMPLATES.iter().map(|(n, _)| *n).collect();
                        eprintln!("no template `{name}`: {}", names.join(" "));
                        std::process::exit(2);
                    }
                }
                return;
            }
            path => {
                let text = match std::fs::read_to_string(path) {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("{path}: {e}");
                        std::process::exit(2);
                    }
                };
                if path.ends_with(".txt") {
                    for (id, entry) in content::entries(&text) {
                        reports.push(content::check(Kind::Wish, &id, &entry));
                    }
                    continue;
                }
                match content::check_file(path, &text) {
                    Some(r) => reports.push(r),
                    None => {
                        eprintln!("{path}: a file's kind is its extension (.wish .card .boss .room .vfx .pastime)");
                        std::process::exit(2);
                    }
                }
            }
        }
    }
    let failing = reports.iter().filter(|r| !r.passes()).count();
    for r in &reports {
        if all || !r.passes() || reports.len() < 8 {
            println!("{}", r.explain());
        }
    }
    eprintln!("{} checked, {} at the bar, {failing} not", reports.len(), reports.len() - failing);
    if failing > 0 {
        std::process::exit(1);
    }
}
