//! Bounded, off-thread views of actual local work. No page fetches or examples.

use crate::media::{Media, MediaSource};
use std::io::Read;
use std::sync::{Arc, mpsc};

const MAX_DOCUMENT_BYTES: usize = 256 * 1024;
const MAX_DOCUMENT_LINES: usize = 8192;

pub(crate) struct Document {
    pub(crate) text: String,
    pub(crate) receipt: String,
}

#[derive(Default)]
pub(super) struct DocumentSurface {
    target: Option<String>,
    pending: Option<mpsc::Receiver<Result<Arc<Document>, String>>>,
    ready: Option<Result<Arc<Document>, String>>,
    pub(super) scroll: u16,
    pub(super) wrapped_rows: Option<(u16, u16)>,
}

impl DocumentSurface {
    pub(super) fn clear(&mut self) {
        // Keep at most one outstanding reader, even if the operator switches
        // faster than storage can respond. Its stale result is discarded.
        self.target = None;
        self.ready = None;
        self.scroll = 0;
        self.wrapped_rows = None;
    }

    pub(super) fn request(&mut self, media: &Media) -> Result<Option<Arc<Document>>, String> {
        let target = media.target();
        if let Some(rx) = self.pending.take() {
            match rx.try_recv() {
                Ok(result) => {
                    if self.target.as_ref() == Some(&target) {
                        self.ready = Some(result);
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {
                    self.pending = Some(rx);
                    return Ok(None);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    if self.target.as_ref() == Some(&target) {
                        self.ready =
                            Some(Err("document reader disconnected; reopen to retry".into()));
                    }
                }
            }
        }
        if self.target.as_ref() == Some(&target) {
            if let Some(ready) = &self.ready {
                return ready.clone().map(Some);
            }
        } else {
            self.target = Some(target.clone());
            self.ready = None;
            self.scroll = 0;
            self.wrapped_rows = None;
        }
        let source = media.source();
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("angel-stage-document".into())
            .spawn(move || {
                let result = match source {
                    Some(source) => load_document(&source),
                    None => Ok(Document {
                        text: format!("Remote source\n\n{target}\n\nPage contents are not loaded in the terminal. Show a saved local report to inspect its contents here."),
                        receipt: "URL reference · no page contents fetched".into(),
                    }),
                };
                let _ = tx.send(result.map(Arc::new));
            })
            .map_err(|error| format!("could not start document reader: {error}"))?;
        self.pending = Some(rx);
        Ok(None)
    }
}

fn load_document(source: &MediaSource) -> Result<Document, String> {
    let file = source.open()?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("Cannot identify requested file: {error}"))?;
    if !metadata.is_file() {
        return Err("Unsupported source: the requested path is not a regular file".into());
    }
    let mut bytes = Vec::with_capacity((metadata.len() as usize).min(MAX_DOCUMENT_BYTES + 1));
    file.take((MAX_DOCUMENT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Cannot read requested file: {error}"))?;
    let truncated = bytes.len() > MAX_DOCUMENT_BYTES;
    bytes.truncate(MAX_DOCUMENT_BYTES);
    if bytes.contains(&0) || bytes.starts_with(b"%PDF-") {
        return Err("Unsupported document format: Stage reads UTF-8 text/Markdown; binary files and PDF need a text export".into());
    }
    let text = match std::str::from_utf8(&bytes) {
        Ok(text) => text,
        Err(error) if truncated && error.error_len().is_none() => {
            std::str::from_utf8(&bytes[..error.valid_up_to()]).expect("validated UTF-8 prefix")
        }
        Err(_) => return Err("Unsupported document encoding: Stage needs UTF-8 text".into()),
    };
    let line_end = text
        .match_indices('\n')
        .nth(MAX_DOCUMENT_LINES - 1)
        .map(|(index, _)| index + 1)
        .unwrap_or(text.len());
    let lines_truncated = line_end < text.len();
    let text = &text[..line_end];
    // Escape control bytes instead of interpreting them as terminal commands;
    // line breaks and tabs keep the source readable. Content remains evidence.
    let mut displayed = String::with_capacity(text.len());
    for character in text.chars() {
        if (character.is_control() && !matches!(character, '\n' | '\t'))
            || matches!(character, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        {
            displayed.extend(character.escape_default());
        } else {
            displayed.push(character);
        }
    }
    if displayed.is_empty() {
        displayed.push_str("[The requested file is empty.]");
    }
    if lines_truncated {
        displayed.push_str("\n[Preview truncated at 8192 lines. Reopen a smaller excerpt to inspect later content.]\n");
    }
    let digest = ring::digest::digest(&ring::digest::SHA256, &bytes);
    let sha = digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let receipt = if truncated {
        displayed.push_str("\n\n[Preview truncated at 256 KiB. Reopen a smaller excerpt to inspect later content.]");
        format!(
            "TRUNCATED · first {} of {} bytes · prefix SHA256 {sha}",
            bytes.len(),
            metadata.len()
        )
    } else if lines_truncated {
        format!(
            "TRUNCATED · first 8192 lines · {} source bytes · SHA256 {sha}",
            bytes.len()
        )
    } else {
        format!(
            "UTF-8 source snapshot · {} bytes · SHA256 {sha}",
            bytes.len()
        )
    };
    Ok(Document {
        text: displayed,
        receipt,
    })
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/scryglass__document__tests.rs"]
mod tests;
