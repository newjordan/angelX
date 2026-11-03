//! Evidence a finished reply links to, opened in the glass showcase.
//!
//! When the agent ends a turn with a line like `Evidence: [iteration
//! report](reports/iteration-7.md)`, the window brings that file in
//! itself, carried by the squire (the wizard for chart data), the way a
//! `present` call does. One showing per reply: the last link on a line
//! that names evidence, else the last picture, video, model or chart
//! data linked anywhere. Web pages, source files and anything outside
//! the workspace stay links. Outside the glass window nothing changes.

use super::markdown;
use std::path::Path;
use std::sync::Arc;

/// The width the reply is laid out at to find its links: wide enough that
/// a paragraph is one line, so "the line a link sits on" is its paragraph
/// or list item.
const LAYOUT_WIDTH: usize = 4096;

/// Words that mark a line's links as evidence.
const EVIDENCE_WORDS: [&str; 4] = ["evidence", "proof", "report", "receipt"];

/// A linked local file and whether it sits on a line naming evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Candidate {
    pub(crate) target: Arc<str>,
    pub(crate) evidence: bool,
}

/// The local link targets in `text`, most worth showing first: those on
/// lines naming evidence (the last of them first), then the rest (last
/// first). Web URLs, other schemes and in-page anchors are left out; so
/// is anything inside code.
pub(crate) fn candidates(text: &str) -> Vec<Candidate> {
    if !text.contains(['[', '<']) {
        return Vec::new();
    }
    let links = markdown::links(text, LAYOUT_WIDTH);
    if links.is_empty() {
        return Vec::new();
    }
    let lines = markdown::render(text, LAYOUT_WIDTH);
    let mut found: Vec<Candidate> = Vec::new();
    for link in links.iter() {
        let target = link.target.trim();
        if !local(target) || found.iter().any(|c| c.target.as_ref() == target) {
            continue;
        }
        let evidence = lines.get(link.line).is_some_and(|line| {
            let words: String = line.spans.iter().map(|s| s.content.as_ref()).collect::<String>().to_lowercase();
            EVIDENCE_WORDS.iter().any(|word| words.contains(word))
        });
        found.push(Candidate { target: Arc::from(target), evidence });
    }
    found.reverse();
    // Stable: evidence lines first, each group last link first.
    found.sort_by_key(|c| !c.evidence);
    found
}

/// Whether `target` names a local file rather than a page or an anchor.
fn local(target: &str) -> bool {
    if target.is_empty() || target.starts_with('#') || target.starts_with("mailto:") {
        return false;
    }
    !target.contains("://") || target.starts_with("file://")
}

/// Whether the showcase should bring in `path` unasked: pictures, video,
/// models and chart data always; written reports and logs only when the
/// reply called them evidence. Source code and markup stay links.
pub(crate) fn worth_showing(path: &Path, evidence: bool) -> bool {
    let Some(extension) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    let extension = extension.to_ascii_lowercase();
    let media = matches!(
        extension.as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "tif" | "tiff" | "qoi"
            | "mp4" | "mkv" | "mov" | "webm" | "avi"
            | "obj" | "glb"
            | "csv" | "tsv"
    );
    let report = matches!(
        extension.as_str(),
        "md" | "markdown" | "txt" | "text" | "log" | "json" | "jsonl" | "ndjson" | "yaml" | "yml"
            | "toml" | "diff" | "patch"
    );
    media || (evidence && report)
}

impl crate::app::App {
    /// Open the evidence `reply` links to in the glass showcase. Does
    /// nothing outside the glass window, or when no link is worth it.
    pub(crate) fn show_reply_evidence(&mut self, reply: &str) {
        if !crate::ui::glass::active() {
            return;
        }
        let root = self.tools.current_workspace().to_path_buf();
        if let Some(media) = evidence_media(reply, &root) {
            crate::ui::showcase::open_evidence(&media);
        }
    }
}

/// The media for the first of `reply`'s candidates that resolves to an
/// existing file inside `root` worth showing: the same confinement a
/// click on the link gets.
pub(crate) fn evidence_media(reply: &str, root: &Path) -> Option<super::media::Media> {
    for candidate in candidates(reply) {
        let Ok(path) = super::media::model_presentation_target_in(&candidate.target, root) else {
            continue;
        };
        if path.starts_with("http://") || path.starts_with("https://") {
            continue;
        }
        if !worth_showing(Path::new(&path), candidate.evidence) {
            continue;
        }
        let Ok(card) = super::media::artifact_from_path_in(&path, root) else {
            continue;
        };
        let root = root.canonicalize().ok()?;
        return Some(super::media::Media::Confined { card: Box::new(card), root });
    }
    None
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/ui_evidence__tests.rs"]
mod tests;
