//! `/browser` — the browser chooser popup.
//!
//! Detection and persistence live with the tool that consumes them
//! (`agent::tools::hands`); this file is only the cockpit ceremony: open the
//! popup, move the selection, commit the pick to `.angel/browser-bin`.

use super::*;

impl App {
    pub(crate) fn open_browser_picker(&mut self) {
        use crate::agent::tools::hands;
        let workspace = self.tools.current_workspace().to_path_buf();
        let effective = hands::browser_binary_in(&workspace);
        let saved = hands::read_browser_choice(&workspace);
        let mut rows: Vec<crate::app::BrowserPickRow> = hands::detected_browsers()
            .into_iter()
            .map(|(name, path, version)| {
                let pretty = match name.as_str() {
                    "google-chrome-stable" | "google-chrome" => "Google Chrome",
                    "chromium" | "chromium-browser" => "Chromium",
                    other => other,
                };
                let detail = if version.is_empty() {
                    path.display().to_string()
                } else {
                    format!("{version} · {}", path.display())
                };
                let current = effective.as_deref() == Some(path.as_path());
                crate::app::BrowserPickRow {
                    label: pretty.to_string(),
                    detail,
                    path: Some(path),
                    current,
                }
            })
            .collect();
        rows.push(crate::app::BrowserPickRow {
            label: "Auto".to_string(),
            detail: "Chrome first, then Chromium (clears the saved pick)".to_string(),
            path: None,
            current: saved.is_none() && std::env::var_os("ANGEL_BROWSER_BIN").is_none(),
        });
        if rows.len() == 1 {
            self.system_msg(
                "No Chrome or Chromium found on PATH. Install one, or set ANGEL_BROWSER_BIN."
                    .to_string(),
            );
            return;
        }
        let selected = rows.iter().position(|r| r.current).unwrap_or(0);
        self.browser_picker = Some(crate::app::BrowserPicker { rows, selected });
    }

    pub(crate) fn browser_picker_key(&mut self, code: KeyCode) -> bool {
        let Some(picker) = self.browser_picker.as_mut() else {
            return false;
        };
        match code {
            KeyCode::Esc => {
                self.browser_picker = None;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                picker.selected = picker.selected.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                picker.selected = (picker.selected + 1).min(picker.rows.len() - 1);
            }
            KeyCode::Enter => {
                let index = picker.selected;
                self.commit_browser_pick(index);
            }
            KeyCode::Char(c) if c.is_ascii_digit() => {
                if let Some(index) = c.to_digit(10).map(|d| d as usize).filter(|i| *i >= 1) {
                    let index = index - 1;
                    if index < picker.rows.len() {
                        self.commit_browser_pick(index);
                    }
                }
            }
            _ => {}
        }
        true
    }

    pub(crate) fn commit_browser_pick(&mut self, index: usize) {
        let Some(picker) = self.browser_picker.take() else {
            return;
        };
        let Some(row) = picker.rows.get(index) else {
            return;
        };
        let workspace = self.tools.current_workspace().to_path_buf();
        match crate::agent::tools::hands::write_browser_choice(&workspace, row.path.as_deref()) {
            Ok(()) => {
                let what = match &row.path {
                    Some(path) => format!("{} ({})", row.label, path.display()),
                    None => "Auto (Chrome first)".to_string(),
                };
                self.system_msg(format!(
                    "Browser: {what}. Applies the next time the browser tool opens; \
                     a session already running keeps its browser until closed."
                ));
            }
            Err(e) => self.system_msg(format!("Could not save browser pick: {e}")),
        }
    }
}
