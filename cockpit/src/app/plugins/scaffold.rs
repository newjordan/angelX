//! `angelX plugin new <id>`: a plugin that works as it stands, to grow from.
//!
//! It carries one command with one skill and one tool, a README and an MIT
//! license, and passes `angelX plugin check` before a line is changed.

use std::path::{Path, PathBuf};

/// Write a new plugin `id` into `dir` (by default `./<id>`), which must be
/// empty or absent. Returns the folder.
pub(crate) fn create(id: &str, dir: Option<&Path>, author: &str) -> Result<PathBuf, String> {
    if !super::valid_id(id) {
        return Err(format!(
            "`{id}`: a plugin id is 2-32 lowercase letters, digits and `-`, starting with a letter"
        ));
    }
    if crate::ui::input::builtin_command(id) {
        return Err(format!(
            "/{id} is one of angelX's own commands; choose another id"
        ));
    }
    let dir = dir.map_or_else(|| PathBuf::from(id), Path::to_path_buf);
    if std::fs::read_dir(&dir).is_ok_and(|mut listing| listing.next().is_some()) {
        return Err(format!("{} is not empty", dir.display()));
    }
    let author = if author.trim().is_empty() {
        "Your Name"
    } else {
        author.trim()
    };
    let label = id
        .split('-')
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_ascii_uppercase().to_string() + chars.as_str()
            })
        })
        .collect::<Vec<_>>()
        .join(" ");
    let tool = format!("{}_hello", id.replace('-', "_"));
    let year = 1970
        + std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs())
            / 31_556_952;
    let files: Vec<(String, String, bool)> = vec![
        (
            "cartridge.toml".into(),
            cartridge(id, &label, &tool, author),
            false,
        ),
        (format!("skills/{id}/SKILL.md"), skill(id, &tool), false),
        ("bin/hello".into(), HELLO.replace("{id}", id), true),
        ("README.md".into(), readme(id, &label, &tool), false),
        (
            "LICENSE".into(),
            MIT.replace("{year}", &year.to_string())
                .replace("{author}", author),
            false,
        ),
    ];
    for (path, text, executable) in files {
        let path = dir.join(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if executable {
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                    .map_err(|e| format!("{}: {e}", path.display()))?;
            }
        }
        #[cfg(not(unix))]
        let _ = executable;
    }
    Ok(dir)
}

fn cartridge(id: &str, label: &str, tool: &str, author: &str) -> String {
    format!(
        r#"# An angelX plugin (docs/PLUGINS.md). `angelX plugin check` says whether it
# loads and whether the registry would take it.
id = "{id}"
label = "{label}"

[plugin]
version = "0.1.0"
description = "One line on what /{id} does."
authors = ["{author}"]
license = "MIT"
# repository = "https://github.com/you/{id}"   # else submit reads the git remote
# angelx = "0.2.7"                               # the lowest angelX it works with
# setup = "setup.sh"                             # run after install and each update

# `/{id} <task>` sends one turn carrying skills/{id}.
[[commands]]
name = "{id}"
usage = "<what to do>"
skills = ["{id}"]

# A tool: bin/hello gets the call's arguments as JSON on stdin and its stdout
# is the result. It waits behind tool_search until a turn asks for it.
[[tools]]
name = "{tool}"
description = "Greet someone by name from the {label} plugin."
run = "bin/hello"
params = {{ type = "object", required = ["name"], properties = {{ name = {{ type = "string", description = "Who to greet" }} }} }}
"#
    )
}

fn skill(id: &str, tool: &str) -> String {
    format!(
        r#"---
name: {id}
description: What the {id} plugin knows and how its tool is used.
---

This turn came from `/{id}`. Say here what the agent should know and do: the
steps, the commands, what a good result looks like.

The plugin's `{tool}` tool greets someone; `tool_search` brings it in. Its
program is `${{ANGEL_CARTRIDGE_DIR}}/bin/hello`, which a shell can run too.
"#
    )
}

const HELLO: &str = r#"#!/bin/sh
# A plugin tool: the call's arguments arrive as JSON on stdin; stdout is the
# result; a non-zero exit is an error carrying the end of stderr. It runs in
# the session's workspace, with ANGEL_CARTRIDGE_DIR naming this plugin's folder.
args=$(cat)
name=$(printf '%s' "$args" | sed -n 's/.*"name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')
printf 'Hello, %s, from the {id} plugin.\n' "${name:-friend}"
"#;

fn readme(id: &str, label: &str, tool: &str) -> String {
    format!(
        r#"# {label}

An [angelX](https://github.com/newjordan/angelX) plugin.

## Use

```sh
angelX plugin install {id}
```

Then, in angelX, `/{id} <task>` sends a turn that carries the plugin's skill,
and the agent can call `{tool}`.

## Develop

```sh
angelX plugin link .      # angelX loads it from this folder
angelX plugin check .     # whether it loads and whether the registry takes it
angelX plugin submit .    # once pushed: opens the registry pull request
```

See [Plugins](https://github.com/newjordan/angelX/blob/main/docs/PLUGINS.md).
"#
    )
}

const MIT: &str = r#"MIT License

Copyright (c) {year} {author}

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
"#;
