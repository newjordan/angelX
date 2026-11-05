//! The Delve's browser page, built and embedded with this binary
//! (`angel-web-bundle`): the angel-web shell and the Delve's wasm client, served by
//! the delve's own server at `/` and `/a/<hash>`, so a page and its host
//! always match.
//!
//! The music is not in the binary. It ships beside it (the package carries
//! `cockpit/assets/dungeon/music`), and the host reads it from there when it
//! opens its door: each track joins the page's sounds manifest and is served
//! like any other file. With no music folder found the page has no music,
//! a line says so in the log, and the game plays on.

use std::path::Path;

use angel_serve::ClientBundle;

use angel_web_bundle::{self as web_bundle, unpack};

/// The page for a hosted delve's server, or `None` in a build without one.
pub(crate) fn bundle() -> Option<ClientBundle> {
    let (index, embedded) = web_bundle::embedded()?;
    let mut files = Vec::with_capacity(embedded.len());
    for (name, bytes) in embedded {
        let original_len = web_bundle::packed()
            .iter()
            .find_map(|(packed, len)| (*packed == *name).then_some(*len));
        let bytes = match unpack(bytes, original_len) {
            Ok(bytes) => bytes,
            Err(error) => {
                tracing::error!(asset = *name, %error, "embedded browser bundle is corrupt");
                return None;
            }
        };
        files.push((
            name.to_string(),
            bytes,
            angel_serve::http::mime_for(name).to_string(),
        ));
    }
    let mut sounds: Vec<String> = Some(web_bundle::sounds())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .into_iter()
        .collect();
    for (prefix, place) in web_bundle::beside() {
        match crate::drive::together_audio::resource(place) {
            Some(dir) => sounds.extend(add_folder(&dir, prefix, &mut files)),
            None => crate::drive::together_audio::missing(&format!(
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
fn add_folder(dir: &Path, prefix: &str, files: &mut Vec<(String, Vec<u8>, String)>) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        crate::drive::together_audio::missing(&format!(
            "could not read the sound folder {}",
            dir.display()
        ));
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
            path.file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_default()
        );
        if !file
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            crate::drive::together_audio::missing(&format!(
                "could not read the sound {}",
                path.display()
            ));
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
#[path = "../../../tests/cockpit/app/together_web__tests.rs"]
mod tests;
