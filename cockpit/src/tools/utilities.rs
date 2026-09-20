//! Utility tools: `present` (rich-media carousel card), `reverse`, and
//! `word_count`. `reverse` and `word_count` are tiny text utilities kept as
//! always-available smoke tools; `present` delivers a card to the cockpit's
//! right-panel carousel.

use crate::club::ToolDef;
use crate::harness::Tool;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub(crate) struct PresentTool {
    workspace: PathBuf,
}
impl PresentTool {
    pub(crate) fn new(workspace: &Path) -> Self {
        Self {
            workspace: workspace.to_path_buf(),
        }
    }
}
impl Tool for PresentTool {
    fn name(&self) -> &str {
        "present"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "present".to_string(),
            description: "Show the requested actual work in the ordinary terminal Stage. \
                Use this when asked to show work, images, videos, or reports: image/video \
                displays a local artifact; resource/report displays a local UTF-8 document \
                (including Markdown, source, CSV, or JSON). Keep the artifact's actual path \
                and a descriptive label; do not substitute example work. The display stays \
                open until dismissed. Remote links show their URL, not fetched page contents. \
                The result acknowledges a queued display request; decode/read failures are \
                shown honestly in Stage."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "kind": {
                        "type": "string",
                        "enum": ["image", "video", "link", "graph", "resource", "report"],
                        "description": "image/video = local terminal-native preview; resource/report = local UTF-8 document; link/graph = exact URL or local report"
                    },
                    "label": { "type": "string", "description": "Short label shown on the card." },
                    "url": { "type": "string", "description": "Actual local artifact path (relative to the active workspace or absolute), or an exact http(s) URL for a link." }
                },
                "required": ["kind", "label", "url"]
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let kind = args["kind"].as_str().ok_or("missing 'kind'")?;
        let label = args["label"].as_str().ok_or("missing 'label'")?;
        let url = args["url"].as_str().ok_or("missing 'url'")?;
        if !["image", "video", "link", "graph", "resource", "report"].contains(&kind) {
            return Err(format!(
                "unknown kind '{kind}' (image|video|link|graph|resource|report)"
            ));
        }
        let kind = if kind == "report" { "resource" } else { kind };
        let url = crate::media::model_presentation_target_in(url, &self.workspace)?;
        if !crate::media::presentation_text_valid(label, 512) {
            return Err(
                "label must contain 1–512 bytes without control or direction characters".into(),
            );
        }
        Ok(serde_json::json!({
            "status": "queued", "kind": kind, "label": label, "url": url
        })
        .to_string())
    }
}

fn string_param(name: &str, desc: &str) -> ToolDef {
    ToolDef {
        name: name.to_string(),
        description: desc.to_string(),
        params: serde_json::json!({
            "type": "object",
            "properties": { "text": { "type": "string", "description": "input text" } },
            "required": ["text"],
        }),
    }
}

pub(crate) struct ReverseTool;
impl Tool for ReverseTool {
    fn name(&self) -> &str {
        "reverse"
    }
    fn def(&self) -> ToolDef {
        string_param("reverse", "Reverse the characters of the given text.")
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let text = args["text"].as_str().ok_or("missing 'text'")?;
        Ok(text.chars().rev().collect())
    }
}

pub(crate) struct WordCountTool;
impl Tool for WordCountTool {
    fn name(&self) -> &str {
        "word_count"
    }
    fn def(&self) -> ToolDef {
        string_param(
            "word_count",
            "Count the whitespace-separated words in text.",
        )
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let text = args["text"].as_str().ok_or("missing 'text'")?;
        Ok(text.split_whitespace().count().to_string())
    }
}

#[cfg(test)]
mod presentation_tests {
    use super::*;

    #[test]
    fn show_work_present_preserves_identity_and_only_acknowledges_queueing() {
        let workspace = std::env::temp_dir().join("angel-present-project");
        std::fs::create_dir_all(&workspace).unwrap();
        let tool = PresentTool::new(&workspace);
        let result = tool
            .call(&serde_json::json!({
                "kind": "report", "label": "Measured result", "url": "reports/result.md"
            }))
            .unwrap();
        let (kind, label, url) = crate::media::presentation_from_result(&result).unwrap();
        assert_eq!(kind, "resource");
        assert_eq!(label, "Measured result");
        assert_eq!(url, workspace.join("reports/result.md").to_string_lossy());
        assert!(!result.contains("presented"));
        assert!(
            tool.call(&serde_json::json!({ "kind": "invalid", "label": "x", "url": "x" }))
                .is_err()
        );
        assert!(
            tool.call(&serde_json::json!({ "kind": "resource", "label": "x", "url": "" }))
                .is_err()
        );
        assert!(crate::media::presentation_from_result("error: missing target").is_none());
        assert!(
            crate::media::presentation_from_result(
                r#"{"status":"queued","kind":"invalid","label":"x","url":"x"}"#
            )
            .is_none()
        );
    }
}
