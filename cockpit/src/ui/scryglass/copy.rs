//! Copy actual displayed work, never an unrelated response or stale reveal.

use super::{Scryglass, StageOverlay};
use crate::ui::media::Media;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StageCopyTarget {
    Location,
    Text,
}

pub(crate) struct StageCopyPayload {
    pub(crate) text: String,
    pub(crate) fallback_file: &'static str,
    pub(crate) description: String,
}

impl Scryglass {
    pub(crate) fn copy_payload(
        &mut self,
        media: &[Media],
        target: StageCopyTarget,
    ) -> Result<StageCopyPayload, String> {
        let Some(StageOverlay::Media { index }) = self.controller.overlay() else {
            return Err("no Stage artifact is displayed; show a file or link first".into());
        };
        let card = media.get(*index).ok_or_else(|| {
            "the displayed Stage artifact is unavailable; reopen it to copy".to_string()
        })?;
        match target {
            StageCopyTarget::Location => {
                let text = if let Some(source) = card.source() {
                    // Copy the requested identity without reopening/canonicalizing it.
                    // Descriptor-bound source reads remain solely in the document loader.
                    if !source.path.is_absolute() {
                        return Err("Stage artifact has no absolute local path; reopen it".into());
                    }
                    source
                        .path
                        .to_str()
                        .ok_or_else(|| {
                            "Stage artifact path is not valid UTF-8; cannot copy it exactly"
                                .to_string()
                        })?
                        .to_string()
                } else {
                    let url = card.target();
                    if !url.starts_with("https://") && !url.starts_with("http://") {
                        return Err("Stage artifact has no copyable absolute path or URL".into());
                    }
                    url
                };
                Ok(StageCopyPayload {
                    text,
                    fallback_file: "stage-location.txt",
                    description: "Stage artifact location".into(),
                })
            }
            StageCopyTarget::Text => {
                if card.is_visual() {
                    return Err(
                        "Stage image/video has no document text; use /copy stage for its location"
                            .into(),
                    );
                }
                if card.source().is_none() {
                    return Err(
                        "remote page contents are not loaded; use /copy stage for the URL".into(),
                    );
                }
                // Reuse the exact bounded, sanitized preview and confined reader.
                // request() is asynchronous and discards stale reads on card switches.
                let document = self.document.request(card)?.ok_or_else(|| {
                    "Stage document is loading; retry /copy stage text when ready".to_string()
                })?;
                Ok(StageCopyPayload {
                    text: document.text.clone(),
                    fallback_file: "stage-document.txt",
                    description: format!("Stage document preview · {}", document.receipt),
                })
            }
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/scryglass__copy__tests.rs"]
mod tests;
