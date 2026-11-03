//! The page's rules modules (`rules.js`), as every bundle build writes it:
//! the Bend modules a game's client runs in the browser, named by the game's
//! `rules.list`, each file wrapped as a function. The page's CSP allows no
//! eval, so the files reach it as served script, not source text. The page's
//! `angel.rules` import (`shell/boot.js`) loads a module by id and calls its
//! `render(input)`, as a native host runs the same files in V8
//! (`angel_live::bundle`). A game with no `rules.list` gets a page with none.
use std::fs;
use std::path::{Path, PathBuf};

/// `rules.js` for the game whose crate folder is `game`, and every path it
/// was made from (the list, each module folder and file).
pub fn rules_js(game: &Path) -> (String, Vec<PathBuf>) {
    let list = game.join("rules.list");
    let mut out = String::from(
        "// The game's rules modules (crates/web/rules_js.rs): its rules.list's Bend modules, a function per file.\n\
         \"use strict\";\n\
         globalThis.angelRules = Object.create(null);\n",
    );
    let Ok(text) = fs::read_to_string(&list) else {
        return (out, Vec::new());
    };
    let mut read = vec![list.clone()];
    for line in text.lines() {
        let folder = line.trim();
        if folder.is_empty() || folder.starts_with('#') {
            continue;
        }
        let dir = game.join(folder);
        let id = dir
            .file_name()
            .and_then(|n| n.to_str())
            .filter(|id| plain(id))
            .unwrap_or_else(|| panic!("{}: {folder} names no module", list.display()));
        let manifest = fs::read_to_string(dir.join("module.json"))
            .unwrap_or_else(|e| panic!("{}: {folder}/module.json: {e}", list.display()));
        let entry = string_field(&manifest, "entry")
            .filter(|entry| plain(entry))
            .unwrap_or_else(|| panic!("{}: {folder}/module.json names no entry", list.display()));
        let mut files = Vec::new();
        js_files(&dir, &dir, &mut files);
        files.sort();
        out.push_str(&format!("angelRules[\"{id}\"] = {{entry: \"{entry}\", files: {{\n"));
        for (name, path) in files {
            let source = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            out.push_str(&format!(
                "\"{name}\": function (module, exports, require, asset, resource) {{\n{source}\n}},\n"
            ));
            read.push(path);
        }
        out.push_str("}};\n");
        read.push(dir.join("module.json"));
    }
    (out, read)
}

/// A name that is its own JavaScript string literal.
fn plain(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b"._/-".contains(&b))
}

/// The string value of `"field"` in a small JSON manifest.
fn string_field(json: &str, field: &str) -> Option<String> {
    let rest = &json[json.find(&format!("\"{field}\""))? + field.len() + 2..];
    let rest = rest.trim_start().strip_prefix(':')?.trim_start().strip_prefix('"')?;
    Some(rest[..rest.find('"')?].to_string())
}

/// Every `*.js` under `dir`, named from `root` with `/`; dotfiles skipped, as
/// the live loader captures a module.
fn js_files(root: &Path, dir: &Path, files: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            js_files(root, &path, files);
        } else if name.ends_with(".js") {
            let rel = path.strip_prefix(root).expect("under the module").to_string_lossy().replace('\\', "/");
            if plain(&rel) {
                files.push((rel, path));
            }
        }
    }
}
