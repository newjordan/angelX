//! The Delve's browser page, built and embedded with this binary
//! (`build.rs`): the angel-web shell and the Delve's wasm client, served by
//! the delve's own server at `/` and `/a/<hash>`, so a page and its host
//! always match.

use angel_serve::ClientBundle;

include!(concat!(env!("OUT_DIR"), "/delve_web.rs"));

/// The page for a hosted delve's server, or `None` in a build without one.
pub(crate) fn bundle() -> Option<ClientBundle> {
    let (index, files) = BUNDLE?;
    Some(ClientBundle {
        index_html: index.to_string(),
        files: files
            .iter()
            .map(|(name, bytes)| {
                (
                    name.to_string(),
                    bytes.to_vec(),
                    angel_serve::http::mime_for(name).to_string(),
                )
            })
            .collect(),
    })
}
