//! Content-addressed originals and append-only parking events, independent of history.
use super::*;
use crate::workspace_store::private_io::PrivateDirectory;
use std::io::{self, Read, Write};

const MARKER: &str = "trajectory-sha256:";

fn valid_digest(digest: &str) -> bool {
    digest.len() == 64
        && digest
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

fn receipt_digest(content: &str) -> Option<&str> {
    let start = content.rfind(MARKER)? + MARKER.len();
    let digest = content.get(start..start + 64)?;
    valid_digest(digest).then_some(digest)
}

/// Resolve and verify the original bytes; never treat a pathname or byte count as recovery.
fn resolve(root: &Path, digest: &str) -> io::Result<Vec<u8>> {
    if !valid_digest(digest) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid evidence digest",
        ));
    }
    let directory = PrivateDirectory::open_existing(&root.join("evidence"))?;
    let mut body = Vec::new();
    directory
        .existing(digest.as_ref())?
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "missing evidence body"))?
        .read_to_end(&mut body)?;
    if crate::cut::sha256_hex(&body) != digest {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "evidence digest mismatch",
        ));
    }
    Ok(body)
}

fn original_body(root: &Path, content: &str) -> io::Result<(String, Vec<u8>)> {
    let stored = crate::secrets::redact_bytes(content.as_bytes());
    let hash = crate::cut::sha256_hex(&stored);
    let directory = PrivateDirectory::open(&root.join("evidence"))?;
    if let Some(mut file) = directory.existing(format!("receipt-{hash}").as_ref())? {
        let mut digest = String::new();
        file.read_to_string(&mut digest)?;
        let body = resolve(root, &digest)?;
        return Ok((digest, body));
    }
    Ok((hash, stored))
}

/// Persist before replacement. On an I/O failure the caller keeps the body in history.
/// Retention is independent of trajectory logging: every replacement has a durable original.
pub(crate) fn park_tool_result(
    message: &ChatMsg,
    retained: &str,
    reason: &str,
) -> io::Result<String> {
    let root = trajectory_dir();
    let directory = PrivateDirectory::open(&root.join("evidence"))?;
    directory.secure_owner_only()?;
    // Text in tool output is untrusted. Only a store-authored association for
    // this exact receipt body can identify a previously parked original.
    let (digest, original) = original_body(&root, &message.content)?;
    let replacement = if retained.is_empty() {
        String::new()
    } else if receipt_digest(retained) == Some(digest.as_str()) {
        retained.to_owned()
    } else {
        format!("{retained} [{MARKER}{digest}]")
    };
    // The durable digest is part of the model-facing receipt. A nominally
    // shorter summary can become larger than the original once it is added.
    // Keep the original, and do not emit a parking event for a no-op.
    if !retained.is_empty() && replacement.len() >= message.content.len() {
        return Ok(message.content.to_string());
    }
    match directory.create_new(digest.as_ref()) {
        Ok(mut file) => {
            file.write_all(&original)?;
            file.sync_all()?;
            directory.sync()?;
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            resolve(&root, &digest)?;
        }
        Err(error) => return Err(error),
    }
    if !replacement.is_empty() {
        let name = format!("receipt-{}", crate::cut::sha256_hex(replacement.as_bytes()));
        match directory.create_new(name.as_ref()) {
            Ok(mut file) => {
                file.write_all(digest.as_bytes())?;
                file.sync_all()?;
                directory.sync()?;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                if original_body(&root, &replacement)?.0 != digest {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "receipt identity mismatch",
                    ));
                }
            }
            Err(error) => return Err(error),
        }
    }
    TURN_LEDGER.with(|cell| {
        let mut ledger = cell.borrow_mut();
        let event = serde_json::json!({"hop":ledger.turn_id.as_ref().map(|_| ledger.next_model_call),
            "schema":"angel-parked-tool/v1",
            "path":format!("evidence/{digest}"),
            "tool_call_id":message.tool_call_id, "digest_sha256":digest,
            "original_bytes":original.len(), "retained_bytes":replacement.len(), "reason":reason,
            "turn_id":ledger.turn_id, "ts_ms":now_ms()});
        // Independent journal survives compaction and absence of a final turn record.
        let mut journal = directory.append("parking-events.jsonl".as_ref())?;
        let _lock = TRAJECTORY_WRITE_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut line = serde_json::to_vec(&event)?;
        line.push(b'\n');
        journal.write_all(&line)?;
        journal.sync_all()?;
        directory.sync()?;
        ledger.parking_events.push(event);
        Ok::<_, io::Error>(())
    })?;
    Ok(replacement)
}

pub(crate) fn park_compaction_window(messages: &[ChatMsg], note: &str) -> io::Result<String> {
    let mut digests = std::collections::BTreeSet::new();
    for message in messages.iter().filter(|m| m.role == ChatRole::Tool) {
        park_tool_result(message, "", "compaction")?;
        let (digest, _) = original_body(&trajectory_dir(), &message.content)?;
        digests.insert(digest);
    }
    // Carry references from earlier compaction summaries through repeated compaction.
    for message in messages.iter().filter(|m| m.role == ChatRole::Harness) {
        for suffix in message.content.split(MARKER).skip(1) {
            if let Some(digest) = suffix.get(..64).filter(|digest| valid_digest(digest)) {
                digests.insert(digest.to_owned());
            }
        }
    }
    let mut note = note.to_owned();
    for digest in digests {
        note.push_str(&format!("\n[{MARKER}{digest}]"));
    }
    Ok(note)
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/trajectory__parking__tests.rs"]
mod tests;
