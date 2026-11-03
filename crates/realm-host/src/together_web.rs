//! The Delve's browser page, built and embedded with this binary
//! (`build.rs`): the angel-web shell and the Delve's wasm client, served by
//! the delve's own server at `/` and `/a/<hash>`, so a page and its host
//! always match.
//!
//! The music is not in the binary. It ships beside it (the package carries
//! `cockpit/assets/dungeon/music`), and the host reads it from there when it
//! opens its door: each track joins the page's sounds manifest and is served
//! like any other file. With no music folder found the page has no music,
//! a line says so in the log, and the game plays on.
//!
//! Where a shipped folder sits, and how a missing one is said, are the
//! cockpit's to know: it lends them as [`Places`].

use std::path::{Path, PathBuf};

use angel_serve::ClientBundle;

include!(concat!(env!("OUT_DIR"), "/delve_web.rs"));

/// What the host lends the page: where a shipped folder sits and how a
/// missing file is said.
pub struct Places {
    /// The shipped folder `place` (relative to the repository, as
    /// `cockpit/assets/dungeon/music`), under the first root that has it.
    pub resource: fn(place: &str) -> Option<PathBuf>,
    /// Say that a shipped file is missing: to the log, and to stderr when
    /// that is not the screen the cockpit draws on.
    pub missing: fn(line: &str),
}

/// The page for a hosted delve's server with the music found where the
/// package ships it (`together_audio`), or `None` in a build without one.
pub fn shipped() -> Option<ClientBundle> {
    bundle(&Places {
        resource: crate::together_audio::resource,
        missing: crate::together_audio::missing,
    })
}

/// The page for a hosted delve's server, or `None` in a build without one.
pub fn bundle(places: &Places) -> Option<ClientBundle> {
    let (index, files) = BUNDLE?;
    let mut files: Vec<(String, Vec<u8>, String)> = files
        .iter()
        .map(|(name, bytes)| {
            (
                name.to_string(),
                bytes.to_vec(),
                angel_serve::http::mime_for(name).to_string(),
            )
        })
        .collect();
    let mut sounds: Vec<String> = Some(SOUNDS)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .into_iter()
        .collect();
    for (prefix, place) in BESIDE {
        match (places.resource)(place) {
            Some(dir) => sounds.extend(add_folder(&dir, prefix, &mut files, places)),
            None => (places.missing)(&format!(
                "no {place} beside this binary: browser seats play without it"
            )),
        }
    }
    Some(ClientBundle {
        index_html: index.replace("@@SOUNDS@@", &format!("{{{}}}", sounds.join(","))),
        files,
    })
}

/// Every file in `dir` as `<prefix><file>`, added to `files`; their entries
/// in the sounds manifest (`"name":"{{asset:name.ext}}"`), in name order. A
/// file the page could not name, or that cannot be read, is left out.
fn add_folder(
    dir: &Path,
    prefix: &str,
    files: &mut Vec<(String, Vec<u8>, String)>,
    places: &Places,
) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        (places.missing)(&format!("could not read the sound folder {}", dir.display()));
        return Vec::new();
    };
    let mut paths: Vec<_> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    paths.sort();
    let mut sounds = Vec::new();
    for path in paths {
        let file = format!(
            "{prefix}{}",
            path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default()
        );
        if !file
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            (places.missing)(&format!("could not read the sound {}", path.display()));
            continue;
        };
        let stem = file.rsplit_once('.').map_or(file.as_str(), |(s, _)| s);
        sounds.push(format!("\"{stem}\":\"{{{{asset:{file}}}}}\""));
        let mime = angel_serve::http::mime_for(&file).to_string();
        files.push((file, bytes, mime));
    }
    sounds
}

#[cfg(test)]
mod tests;
