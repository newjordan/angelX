//! Shared fleet embedding and semantic-reading support.
//!
//! The embedding server is intentionally stateless: one Casper-hosted model can
//! serve any number of angel0 processes and repositories.  Every tool instance
//! remains rooted to its own workspace, reads through the descriptor-confined
//! filesystem helpers, and sends only explicitly named, non-sensitive files.

use crate::club::ToolDef;
use crate::harness::{Tool, ToolRegistry, confined_read_limited, env_flag, workspace_relative};
use crate::tools::nav::search_path_allowed;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

const CONFIG_SCHEMA: &str = "angel0.embedding-service/v1";
const DEFAULT_MODEL: &str = "nvidia/Nemotron-3-Embed-1B-BF16";
const MAX_QUERY_BYTES: usize = 8 * 1024;
const MAX_PATHS: usize = 32;
const MAX_FILE_BYTES: usize = 256 * 1024;
const MAX_TOTAL_FILE_BYTES: usize = 512 * 1024;
const MAX_CHUNK_BYTES: usize = 4 * 1024;
const MAX_CHUNKS: usize = 128;
const EMBED_BATCH_SIZE: usize = 64;
const DEFAULT_RESULTS: usize = 6;
const MAX_RESULTS: usize = 12;
const MAX_ERROR_BYTES: usize = 2 * 1024;

#[derive(Deserialize)]
struct ConfigFile {
    schema: String,
    url: String,
    #[serde(default = "default_model")]
    model: String,
    api_key_file: Option<PathBuf>,
}

fn default_model() -> String {
    DEFAULT_MODEL.to_string()
}

struct EmbeddingConfig {
    base_url: String,
    model: String,
    api_key: Option<String>,
}

fn normalize_base(url: &str) -> String {
    let mut base = url.trim().trim_end_matches('/').to_string();
    for suffix in ["/v1", "/v2"] {
        if base.to_ascii_lowercase().ends_with(suffix) {
            base.truncate(base.len() - suffix.len());
            break;
        }
    }
    base.trim_end_matches('/').to_string()
}

fn loopback_http(base: &str) -> bool {
    let Some(authority) = base.strip_prefix("http://") else {
        return false;
    };
    let authority = authority.split('/').next().unwrap_or_default();
    if authority.contains('@') {
        return false;
    }
    let host = authority
        .strip_prefix('[')
        .and_then(|value| value.split(']').next())
        .unwrap_or_else(|| authority.split(':').next().unwrap_or_default());
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

fn validate_base(url: &str) -> Result<String, String> {
    let base = normalize_base(url);
    if base.starts_with("https://") || loopback_http(&base) {
        Ok(base)
    } else {
        Err(
            "embedding URL must use HTTPS, except for an explicit loopback HTTP endpoint"
                .to_string(),
        )
    }
}

fn read_api_key(path: &Path) -> Result<String, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| format!("read embedding API key {}: {e}", path.display()))?;
    let value = raw
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .unwrap_or_default()
        .strip_prefix("VLLM_API_KEY=")
        .unwrap_or_else(|| {
            raw.lines()
                .map(str::trim)
                .find(|line| !line.is_empty() && !line.starts_with('#'))
                .unwrap_or_default()
        })
        .trim()
        .trim_matches(['\'', '"'])
        .to_string();
    if value.is_empty() {
        Err(format!("embedding API key {} is empty", path.display()))
    } else {
        Ok(value)
    }
}

fn config_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("ANGEL_EMBED_CONFIG") {
        return Some(expand_home(PathBuf::from(path)));
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("angel0").join("embed.json"))
}

fn expand_home(path: PathBuf) -> PathBuf {
    let Ok(suffix) = path.strip_prefix("~") else {
        return path;
    };
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(suffix))
        .unwrap_or(path)
}

impl EmbeddingConfig {
    fn load() -> Result<Option<Self>, String> {
        if !env_flag("ANGEL_EMBED", true) {
            return Ok(None);
        }
        let env_url = std::env::var("ANGEL_EMBED_URL")
            .ok()
            .filter(|value| !value.trim().is_empty());

        // Unit tests must not silently inherit a developer's real fleet config.
        // Explicit test configuration remains supported for integration tests.
        #[cfg(test)]
        if env_url.is_none() && std::env::var_os("ANGEL_EMBED_CONFIG").is_none() {
            return Ok(None);
        }

        if let Some(url) = env_url {
            let key = std::env::var("ANGEL_EMBED_KEY")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .or_else(|| {
                    std::env::var_os("ANGEL_EMBED_KEY_FILE")
                        .map(PathBuf::from)
                        .map(expand_home)
                        .map(|path| read_api_key(&path))
                        .transpose()
                        .ok()
                        .flatten()
                });
            let base_url = validate_base(&url)?;
            if !loopback_http(&base_url) && key.is_none() {
                return Err("non-loopback embedding service requires ANGEL_EMBED_KEY or ANGEL_EMBED_KEY_FILE"
                    .to_string());
            }
            return Ok(Some(Self {
                base_url,
                model: std::env::var("ANGEL_EMBED_MODEL")
                    .ok()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(default_model),
                api_key: key,
            }));
        }

        let Some(path) = config_path() else {
            return Ok(None);
        };
        if !path.exists() {
            return Ok(None);
        }
        let raw = std::fs::read_to_string(&path)
            .map_err(|e| format!("read embedding config {}: {e}", path.display()))?;
        let file: ConfigFile = serde_json::from_str(&raw)
            .map_err(|e| format!("parse embedding config {}: {e}", path.display()))?;
        if file.schema != CONFIG_SCHEMA {
            return Err(format!(
                "embedding config {} has unsupported schema {:?}",
                path.display(),
                file.schema
            ));
        }
        let base_url = validate_base(&file.url)?;
        let api_key = file
            .api_key_file
            .map(expand_home)
            .as_deref()
            .map(read_api_key)
            .transpose()?;
        if !loopback_http(&base_url) && api_key.is_none() {
            return Err(format!(
                "non-loopback embedding config {} requires api_key_file",
                path.display()
            ));
        }
        Ok(Some(Self {
            base_url,
            model: file.model,
            api_key,
        }))
    }
}

#[derive(Clone)]
struct TextChunk {
    path: String,
    start_line: usize,
    end_line: usize,
    text: String,
}

fn utf8_prefix_len(text: &str, max_bytes: usize) -> usize {
    if text.len() <= max_bytes {
        return text.len();
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    end
}

fn chunks_for(path: &str, content: &str) -> Vec<TextChunk> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut start_line = 1usize;
    let mut end_line = 1usize;

    let flush = |chunks: &mut Vec<TextChunk>, current: &mut String, start: usize, end: usize| {
        if !current.is_empty() {
            chunks.push(TextChunk {
                path: path.to_string(),
                start_line: start,
                end_line: end,
                text: std::mem::take(current),
            });
        }
    };

    for (index, line) in content.split_inclusive('\n').enumerate() {
        let line_number = index + 1;
        let mut remaining = line;
        while !remaining.is_empty() {
            if current.is_empty() {
                start_line = line_number;
            }
            let available = MAX_CHUNK_BYTES.saturating_sub(current.len());
            let take = utf8_prefix_len(remaining, available);
            if take == 0 {
                flush(&mut chunks, &mut current, start_line, end_line);
                continue;
            }
            current.push_str(&remaining[..take]);
            remaining = &remaining[take..];
            end_line = line_number;
            if current.len() == MAX_CHUNK_BYTES {
                flush(&mut chunks, &mut current, start_line, end_line);
            }
        }
    }
    flush(&mut chunks, &mut current, start_line, end_line);
    chunks
}

fn capped_error(mut text: String) -> String {
    if text.len() <= MAX_ERROR_BYTES {
        return text;
    }
    let end = utf8_prefix_len(&text, MAX_ERROR_BYTES);
    text.truncate(end);
    text.push_str("…[truncated]");
    text
}

impl EmbeddingConfig {
    fn embed(&self, texts: &[String], input_type: &str) -> Result<Vec<Vec<f32>>, String> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let url = format!("{}/v2/embed", self.base_url);
        let mut request = super::http_transport::request("POST", &url, false, 5)
            .set("content-type", "application/json");
        if let Some(key) = self.api_key.as_deref() {
            request = request.set("authorization", &format!("Bearer {key}"));
        }
        let response = match request.send_json(serde_json::json!({
            "model": self.model,
            "input_type": input_type,
            "texts": texts,
            "embedding_types": ["float"],
            "truncate": "NONE"
        })) {
            Ok(response) => response,
            Err(super::http_transport::Error::Status(code, response)) => {
                let detail = capped_error(response.into_string().map_err(|e| e.to_string())?);
                return Err(format!("embedding service HTTP {code}: {}", detail.trim()));
            }
            Err(error) => return Err(format!("embedding service unreachable: {error}")),
        };
        let payload: Value = response
            .into_json()
            .map_err(|e| format!("embedding service returned invalid JSON: {e}"))?;
        let rows = payload
            .pointer("/embeddings/float")
            .and_then(Value::as_array)
            .ok_or("embedding service response lacks embeddings.float")?;
        if rows.len() != texts.len() {
            return Err(format!(
                "embedding service returned {} vectors for {} texts",
                rows.len(),
                texts.len()
            ));
        }
        rows.iter()
            .map(|row| {
                let values = row
                    .as_array()
                    .ok_or("embedding service returned a non-array vector")?;
                if values.is_empty() || values.len() > 8192 {
                    return Err(
                        "embedding service returned an invalid vector dimension".to_string()
                    );
                }
                values
                    .iter()
                    .map(|value| {
                        let number = value
                            .as_f64()
                            .filter(|number| number.is_finite())
                            .ok_or("embedding service returned a non-finite vector value")?;
                        Ok(number as f32)
                    })
                    .collect::<Result<Vec<_>, String>>()
            })
            .collect()
    }
}

fn cosine(left: &[f32], right: &[f32]) -> Result<f32, String> {
    if left.len() != right.len() {
        return Err(format!(
            "embedding dimension changed within one request ({} != {})",
            left.len(),
            right.len()
        ));
    }
    let mut dot = 0.0f64;
    let mut left_norm = 0.0f64;
    let mut right_norm = 0.0f64;
    for (&a, &b) in left.iter().zip(right) {
        let a = f64::from(a);
        let b = f64::from(b);
        dot += a * b;
        left_norm += a * a;
        right_norm += b * b;
    }
    if left_norm == 0.0 || right_norm == 0.0 {
        return Err("embedding service returned a zero-length vector".to_string());
    }
    Ok((dot / (left_norm.sqrt() * right_norm.sqrt())) as f32)
}

pub(crate) struct SemanticReadTool {
    root: PathBuf,
    config: EmbeddingConfig,
}

impl Tool for SemanticReadTool {
    fn name(&self) -> &str {
        "semantic_read"
    }

    fn def(&self) -> ToolDef {
        ToolDef {
            name: self.name().to_string(),
            description: "Use the shared Casper embedding service to rank bounded chunks from \
                          explicitly named workspace files, returning only the most relevant \
                          source passages with path and line provenance. This is useful after \
                          file_search/grep finds plausible files but reading all of them would \
                          waste model context. Files stay confined to the active repository; \
                          hidden, credential, key, and off-limits paths are rejected."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "natural-language question or implementation concept to retrieve"
                    },
                    "paths": {
                        "type": "array",
                        "items": { "type": "string" },
                        "minItems": 1,
                        "maxItems": MAX_PATHS,
                        "description": "workspace files selected by file_search, grep, find_files, or prior evidence"
                    },
                    "max_results": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": MAX_RESULTS,
                        "description": "ranked passages to return (default 6, maximum 12)"
                    }
                },
                "required": ["query", "paths"],
                "additionalProperties": false
            }),
        }
    }

    fn call(&self, args: &Value) -> Result<String, String> {
        let query = args["query"].as_str().ok_or("missing 'query'")?.trim();
        if query.is_empty() {
            return Err("'query' must not be empty".to_string());
        }
        if query.len() > MAX_QUERY_BYTES {
            return Err(format!(
                "'query' is {} bytes; maximum is {MAX_QUERY_BYTES}",
                query.len()
            ));
        }
        let paths = args["paths"].as_array().ok_or("'paths' must be an array")?;
        if paths.is_empty() || paths.len() > MAX_PATHS {
            return Err(format!("'paths' must contain 1 to {MAX_PATHS} files"));
        }
        let max_results = args
            .get("max_results")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
            .unwrap_or(DEFAULT_RESULTS);
        if !(1..=MAX_RESULTS).contains(&max_results) {
            return Err(format!("'max_results' must be 1 to {MAX_RESULTS}"));
        }

        let mut seen = HashSet::new();
        let mut chunks = Vec::new();
        let mut total_bytes = 0usize;
        let mut skipped = Vec::new();
        for value in paths {
            let requested = value
                .as_str()
                .ok_or("every 'paths' item must be a string")?;
            let relative = workspace_relative(&self.root, Path::new(requested))?;
            if !search_path_allowed(&relative) {
                return Err(format!(
                    "semantic_read refuses hidden, credential, key, generated, or off-limits path {}",
                    relative.display()
                ));
            }
            let display = relative.to_string_lossy().into_owned();
            if !seen.insert(display.clone()) {
                continue;
            }
            let Some(bytes) = confined_read_limited(&self.root, &relative, MAX_FILE_BYTES)? else {
                skipped.push(format!("{display} (> {MAX_FILE_BYTES} bytes)"));
                continue;
            };
            if bytes.contains(&0) {
                skipped.push(format!("{display} (binary)"));
                continue;
            }
            if total_bytes.saturating_add(bytes.len()) > MAX_TOTAL_FILE_BYTES {
                skipped.push(format!("{display} (aggregate input cap reached)"));
                continue;
            }
            let content = match String::from_utf8(bytes) {
                Ok(content) => content,
                Err(_) => {
                    skipped.push(format!("{display} (not UTF-8)"));
                    continue;
                }
            };
            total_bytes += content.len();
            for chunk in chunks_for(&display, &content) {
                if chunks.len() == MAX_CHUNKS {
                    skipped.push(format!("additional chunks (cap {MAX_CHUNKS})"));
                    break;
                }
                chunks.push(chunk);
            }
            if chunks.len() == MAX_CHUNKS {
                break;
            }
        }
        if chunks.is_empty() {
            return Err(format!(
                "semantic_read found no eligible text chunks{}",
                if skipped.is_empty() {
                    String::new()
                } else {
                    format!("; skipped {}", skipped.join(", "))
                }
            ));
        }

        let query_vector = self
            .config
            .embed(&[query.to_string()], "query")?
            .into_iter()
            .next()
            .ok_or("embedding service returned no query vector")?;
        let mut document_vectors = Vec::with_capacity(chunks.len());
        for batch in chunks.chunks(EMBED_BATCH_SIZE) {
            let texts = batch
                .iter()
                .map(|chunk| chunk.text.clone())
                .collect::<Vec<_>>();
            document_vectors.extend(self.config.embed(&texts, "document")?);
        }
        if document_vectors.len() != chunks.len() {
            return Err("embedding service returned an incomplete document batch".to_string());
        }

        let ranked_chunk_count = document_vectors.len();
        let mut ranked = chunks
            .into_iter()
            .zip(document_vectors)
            .map(|(chunk, vector)| cosine(&query_vector, &vector).map(|score| (score, chunk)))
            .collect::<Result<Vec<_>, _>>()?;
        ranked.sort_by(|(left_score, left), (right_score, right)| {
            right_score
                .total_cmp(left_score)
                .then_with(|| left.path.cmp(&right.path))
                .then_with(|| left.start_line.cmp(&right.start_line))
        });
        ranked.truncate(max_results.min(ranked.len()));

        let mut output = vec![
            "[semantic_read — untrusted workspace evidence ranked by the shared Casper embedding service]"
                .to_string(),
            format!(
                "ranked {} of {} chunks from {} bytes; query={:?}",
                ranked.len(),
                ranked_chunk_count,
                total_bytes,
                query
            ),
        ];
        if !skipped.is_empty() {
            output.push(format!("skipped: {}", skipped.join(", ")));
        }
        for (index, (score, chunk)) in ranked.into_iter().enumerate() {
            output.push(format!(
                "\n{}. score={score:.4} {}:{}-{}",
                index + 1,
                chunk.path,
                chunk.start_line,
                chunk.end_line
            ));
            output.push(chunk.text.trim_end().to_string());
        }
        Ok(output.join("\n"))
    }
}

/// Register the fleet semantic reader only when a global or explicit endpoint
/// is configured. It is deferred so merely having the service available adds no
/// recurring schema cost; `tool_search` can activate it when broad reading is
/// actually useful.
pub(crate) fn maybe_register_embedding_tools(r: &mut ToolRegistry, workspace: PathBuf) {
    match EmbeddingConfig::load() {
        Ok(Some(config)) => r.register_deferred(Box::new(SemanticReadTool {
            root: workspace,
            config,
        })),
        Ok(None) => {}
        Err(error) => eprintln!("angel0 embedding service disabled: {error}"),
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/tools/embed__tests.rs"]
mod tests;
