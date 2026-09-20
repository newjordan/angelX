//! Bounded lexical source acquisition for already discovered files. Each file
//! is read once through the workspace descriptor, independently of other files.
//! Hashes bind those read bytes; they do not assert an atomic repository snapshot.

use super::{RegexBuilder, Value, defs_regex_many, search_path_allowed};
use crate::agent::harness::{confined_read_limited_no_symlinks, workspace_relative};
use crate::knowledge::cut::sha256_hex;
use serde_json::json;
use std::path::{Path, PathBuf};

const FILE_LIMIT: usize = 2 * 1024 * 1024;
const HITS_PER_KIND: usize = 2;

pub(super) fn collect(root: &Path, args: &Value, names: &[String]) -> Result<String, String> {
    collect_with(root, args, names, |path| {
        confined_read_limited_no_symlinks(root, path, FILE_LIMIT)
    })
}

fn collect_with(
    root: &Path,
    args: &Value,
    names: &[String],
    mut read: impl FnMut(&Path) -> Result<Option<Vec<u8>>, String>,
) -> Result<String, String> {
    if names.is_empty() || names.len() > 8 || names.iter().any(|name| name.len() > 128) {
        return Err("source bundles require 1-8 names, each at most 128 bytes".into());
    }
    let raw_paths = args["paths"]
        .as_array()
        .filter(|paths| !paths.is_empty() && paths.len() <= 8)
        .ok_or("include_source requires 1-8 discovered file paths")?;
    // Validate the entire request before any filesystem access, including probes.
    let mut paths: Vec<PathBuf> = Vec::new();
    for raw in raw_paths {
        let raw = raw
            .as_str()
            .filter(|path| !path.is_empty() && path.len() <= 1024)
            .ok_or("source paths must be nonempty strings of at most 1024 bytes")?;
        let path = workspace_relative(root, Path::new(raw))?;
        if !search_path_allowed(&path) {
            return Err("source path is excluded by workspace search policy".into());
        }
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
    let budget = match args.get("max_source_bytes") {
        None => 16_384,
        Some(value) => value
            .as_u64()
            .filter(|value| (1024..=65_536).contains(value))
            .ok_or("max_source_bytes must be an integer from 1024 through 65536")?
            as usize,
    };
    let ignore_case = match args.get("ignore_case") {
        None => false,
        Some(value) => value.as_bool().ok_or("ignore_case must be boolean")?,
    };
    let patterns = names
        .iter()
        .map(|name| {
            let declaration = RegexBuilder::new(&defs_regex_many(&[name.as_str()]))
                .case_insensitive(ignore_case)
                .build()
                .map_err(|error| error.to_string())?;
            // Unlike ASCII byte indexing, these boundaries preserve Unicode
            // identifiers and names containing namespace separators or '$'.
            let reference = RegexBuilder::new(&format!(
                r"(?:^|[^\p{{L}}\p{{N}}_:$]){}(?:$|[^\p{{L}}\p{{N}}_:$])",
                regex::escape(name)
            ))
            .case_insensitive(ignore_case)
            .build()
            .map_err(|error| error.to_string())?;
            Ok((declaration, reference))
        })
        .collect::<Result<Vec<_>, String>>()?;

    let mut files = Vec::new();
    let mut source_bytes = 0;
    let mut truncated = false;
    let mut unread_files = 0;
    // Reserve an equal share for each file so an early large definition cannot
    // consume every later file's context. Unused shares are intentionally bounded.
    let per_file_budget = budget / paths.len();
    for path in paths {
        let bytes = match read(&path) {
            Ok(Some(bytes)) if bytes.len() <= FILE_LIMIT => bytes,
            Ok(_) => {
                unread_files += 1;
                files.push(json!({"path": path, "status": "file_limit_exceeded"}));
                continue;
            }
            Err(_) => {
                unread_files += 1;
                // Descriptor errors may contain unbounded platform text; the
                // explicit status avoids treating an unread file as no matches.
                files.push(json!({"path": path, "status": "unreadable"}));
                continue;
            }
        };
        let sha256 = sha256_hex(&bytes);
        let Ok(text) = std::str::from_utf8(&bytes) else {
            unread_files += 1;
            files.push(json!({"path": path, "sha256": sha256, "status": "invalid_utf8"}));
            continue;
        };
        let lines = text.split_inclusive('\n').collect::<Vec<_>>();
        let mut definitions = vec![Vec::new(); names.len()];
        let mut references = vec![Vec::new(); names.len()];
        let mut counts = vec![[0_usize; 2]; names.len()];
        for (index, line) in lines.iter().enumerate() {
            for (symbol, (declaration, reference)) in patterns.iter().enumerate() {
                let kind = if declaration.is_match(line) {
                    0
                } else if reference.is_match(line) {
                    1
                } else {
                    continue;
                };
                counts[symbol][kind] += 1;
                let hits = if kind == 0 {
                    &mut definitions[symbol]
                } else {
                    &mut references[symbol]
                };
                if hits.len() < HITS_PER_KIND {
                    hits.push(index);
                }
            }
        }
        // Definitions first; round-robin names before second hits. Merge all
        // overlapping windows, retaining the best priority of their anchors.
        let mut ranges = Vec::new();
        for group in [&definitions, &references] {
            for occurrence in 0..HITS_PER_KIND {
                for hits in group {
                    if let Some(&line) = hits.get(occurrence) {
                        ranges.push((
                            line.saturating_sub(4),
                            (line + 33).min(lines.len()),
                            ranges.len(),
                            line,
                        ));
                    }
                }
            }
        }
        ranges.sort_unstable_by_key(|range| range.0);
        let mut merged: Vec<(usize, usize, usize, usize)> = Vec::new();
        for (start, end, priority, anchor) in ranges {
            if let Some(last) = merged.last_mut()
                && start <= last.1
            {
                last.1 = last.1.max(end);
                if priority < last.2 {
                    last.2 = priority;
                    last.3 = anchor;
                }
            } else {
                merged.push((start, end, priority, anchor));
            }
        }
        merged.sort_unstable_by_key(|range| range.2);
        let mut remaining = per_file_budget;
        let mut windows = Vec::new();
        let mut file_truncated = counts.iter().flatten().any(|count| *count > HITS_PER_KIND);
        let window_count = merged.len();
        for (index, (original_start, end, _, anchor)) in merged.into_iter().enumerate() {
            let share = remaining / (window_count - index);
            let mut start = original_start;
            let mut prefix_bytes: usize = lines[start..anchor].iter().map(|line| line.len()).sum();
            // Preserve the matching line before spending a limited window on
            // preceding comments. Later disjoint anchors also retain a share.
            while start < anchor
                && (prefix_bytes > share / 4
                    || prefix_bytes + lines[anchor].len().min(share) > share)
            {
                prefix_bytes -= lines[start].len();
                start += 1;
            }
            let window = lines[start..end].concat();
            let mut keep = share.min(window.len());
            while !window.is_char_boundary(keep) {
                keep -= 1;
            }
            let clipped = keep < window.len() || start != original_start;
            file_truncated |= clipped;
            if keep == 0 {
                continue;
            }
            let source = &window[..keep];
            windows.push(json!({
                "start_line": start + 1,
                "end_line": start + source.split_inclusive('\n').count(),
                "anchor_line": anchor + 1,
                "leading_context_omitted": start != original_start,
                "window_truncated": clipped,
                "source": source
            }));
            remaining -= keep;
            source_bytes += keep;
        }
        let matches = names
            .iter()
            .enumerate()
            // A multi-file query otherwise repeats mostly empty rows. The
            // requested names live once in the envelope; omitted rows in a
            // successfully read file mean zero lexical matches, not unknown.
            .filter(|(i, _)| counts[*i] != [0, 0])
            .map(|(i, name)| {
                json!({
                "name": name, "definition_lines": counts[i][0], "reference_lines": counts[i][1]
                , "selected_definition_lines": definitions[i].iter().map(|line| line + 1).collect::<Vec<_>>()
                , "selected_reference_lines": references[i].iter().map(|line| line + 1).collect::<Vec<_>>()
                })
            })
            .collect::<Vec<_>>();
        truncated |= file_truncated;
        files.push(json!({
            "path": path, "status": "read", "sha256": sha256,
            "file_bytes": bytes.len(), "file_lines": lines.len(),
            "matches": matches, "windows": windows, "truncated": file_truncated
        }));
    }
    Ok(json!({
        "schema": "angel-definition-source/v1",
        "requested_names": names,
        "match_rows": "nonzero-counts-per-read-file",
        "scope": "requested-files-only", "matching": "lexical-heuristic",
        "window_lines_before": 4, "window_lines_after": 32,
        "max_hits_per_symbol_per_kind": HITS_PER_KIND,
        "max_file_bytes": FILE_LIMIT, "max_source_bytes": budget,
        "source_bytes": source_bytes, "unread_files": unread_files,
        "truncated": truncated, "files": files
    })
    .to_string())
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/tools/nav__definition_bundle__tests.rs"]
mod tests;
