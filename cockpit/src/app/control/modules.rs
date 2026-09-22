//! Module/layout commands and shell/focus/scroll control.
use super::*;

impl App {
    pub(crate) fn modules_text(&self) -> String {
        let mut out = String::from("modules\n");
        out.push_str(&self.module_host.layout_text());
        out.push_str("\n/open <id> activates · /close <id> suspends · /layout save|load <name>");
        out
    }

    pub(crate) fn open_module(&mut self, id: &str) -> String {
        let id = crate::platform::runtime::ModuleId::new(id);
        match self.module_host.activate(&id) {
            Ok(()) => {
                self.apply_module_activation(id.as_str());
                format!("module {id} active")
            }
            Err(e) => format!("/open: {e}"),
        }
    }

    pub(crate) fn close_module(&mut self, id: &str) -> String {
        let id = crate::platform::runtime::ModuleId::new(id);
        match self.module_host.suspend(&id) {
            Ok(()) => {
                self.apply_module_suspension(id.as_str());
                format!("module {id} suspended")
            }
            Err(e) => format!("/close: {e}"),
        }
    }

    pub(crate) fn layout_command(&mut self, action: &str, name: Option<&str>) -> String {
        match action {
            "show" | "list" => self.modules_text(),
            "save" => match name {
                Some(name) => match self.module_host.save_layout(name) {
                    Ok(path) => format!("layout saved: {}", path.display()),
                    Err(e) => format!("/layout save: {e}"),
                },
                None => "usage: /layout save <name>".to_string(),
            },
            "load" => match name {
                Some(name) => match self.module_host.load_layout(name) {
                    Ok(path) => {
                        self.apply_loaded_layout();
                        format!("layout loaded: {}", path.display())
                    }
                    Err(e) => format!("/layout load: {e}"),
                },
                None => "usage: /layout load <name>".to_string(),
            },
            _ => "usage: /layout [show|save <name>|load <name>]".to_string(),
        }
    }

    /// The graph module and the Raytrace stage route are one fact in two
    /// stores (module lifecycle vs scene routing). Every path that changes one
    /// side goes through here so they cannot drift.
    fn sync_graph_stage(&mut self, active: bool) {
        if active {
            self.scryglass
                .navigate(crate::ui::scryglass::StageRoute::Raytrace);
        } else {
            self.scryglass
                .controller
                .leave_route(crate::ui::scryglass::StageRoute::Raytrace);
        }
    }

    fn apply_module_activation(&mut self, id: &str) {
        match id {
            "shell" => {
                if self.shell.is_none() {
                    self.spawn_shell_from_last_area();
                }
                self.shell_focused = self.shell.is_some();
            }
            "image" => {}
            "graph" => self.sync_graph_stage(true),
            _ => {}
        }
    }

    fn apply_module_suspension(&mut self, id: &str) {
        match id {
            "shell" => self.shell_focused = false,
            "image" => self.viewer.clear(),
            "graph" => self.sync_graph_stage(false),
            _ => {}
        }
    }

    fn apply_loaded_layout(&mut self) {
        if !self.module_host.is_running("shell") {
            self.shell_focused = false;
        } else if self.shell.is_none() {
            self.spawn_shell_from_last_area();
        }
        let graph_running = self.module_host.is_running("graph");
        self.sync_graph_stage(graph_running);
    }

    /// Toggle the embedded shell pane: spawn it on first use, else flip focus.
    pub(crate) fn toggle_shell(&mut self) {
        if self.shell.is_none() {
            self.spawn_shell_from_last_area();
            self.shell_focused = self.shell.is_some();
        } else {
            self.shell_focused = !self.shell_focused;
        }
        let shell = crate::platform::runtime::ModuleId::new("shell");
        if self.shell_focused {
            let _ = self.module_host.activate(&shell);
        } else {
            let _ = self.module_host.suspend(&shell);
        }
    }

    /// Check if the embedded shell process has exited; if so, close the pane and return focus.
    pub(crate) fn poll_shell_status(&mut self) {
        if let Some(shell) = self.shell.as_ref()
            && !shell.is_alive()
        {
            self.shell = None;
            self.shell_focused = false;
            let _ = self
                .module_host
                .suspend(&crate::platform::runtime::ModuleId::new("shell"));
            self.system_msg("shell process exited".to_string());
        }
    }

    fn spawn_shell_from_last_area(&mut self) {
        let area = self.shell_area.unwrap_or(ratatui::layout::Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        });
        match ShellPane::spawn(area.height.max(1), area.width.max(1)) {
            Ok(s) => {
                self.shell = Some(s);
            }
            Err(e) => self.messages.push(Message {
                role: Role::System,
                text: (if ShellPane::can_spawn() {
                    format!("shell spawn failed: {e}")
                } else {
                    "shell unavailable: this environment has no PTY access \
                     (devpts mounted without a usable /dev/ptmx)."
                        .to_string()
                })
                .into(),
            }),
        }
    }

    pub(crate) fn handle_module_focus_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::F(2) => {
                self.focus_module("core");
                true
            }
            _ => false,
        }
    }

    pub(crate) fn focus_module(&mut self, id: &str) {
        if id != "artifacts" {
            self.scryglass_drag = None;
        }
        let module_id = crate::platform::runtime::ModuleId::new(id);
        if !self.module_host.is_running(id) {
            let _ = self.module_host.activate(&module_id);
            self.apply_module_activation(id);
        }
        let _ = self.module_host.focus(&module_id);
        self.tool_strip.pause_for_reading(id == "core");
    }

    fn focused_scroll_pane(&self) -> crate::ui::mouse::PaneId {
        match self.module_host.focused().map(|id| id.as_str()) {
            Some("agent") if self.module_host.is_running("agent") => {
                crate::ui::mouse::PaneId::AgentBay
            }
            Some("artifacts") if self.module_host.is_running("artifacts") => {
                crate::ui::mouse::PaneId::Artifacts
            }
            _ => crate::ui::mouse::PaneId::Transcript,
        }
    }

    pub(crate) fn scroll_focused_view_up(&mut self, rows: u16) {
        self.scroll_pane_up(self.focused_scroll_pane(), rows);
    }

    pub(crate) fn scroll_focused_view_down(&mut self, rows: u16) {
        self.scroll_pane_down(self.focused_scroll_pane(), rows);
    }

    pub(crate) fn scroll_focused_view_top(&mut self) {
        match self.focused_scroll_pane() {
            crate::ui::mouse::PaneId::AgentBay => self.reasoning_scroll = usize::MAX,
            crate::ui::mouse::PaneId::Artifacts => self.scryglass.scroll_teaching_top(),
            _ => self.scroll = u16::MAX,
        }
    }

    pub(crate) fn scroll_focused_view_bottom(&mut self) {
        match self.focused_scroll_pane() {
            crate::ui::mouse::PaneId::AgentBay => self.reasoning_scroll = 0,
            crate::ui::mouse::PaneId::Artifacts => self.scryglass.scroll_teaching_bottom(),
            _ => self.scroll = 0,
        }
    }

    /// Clipboard geometry is intentionally text-only. The rest of a visible
    /// agent bay still owns focus and scrolling, but never starts a selection.
    pub(crate) fn interaction_pane_at(&self, x: u16, y: u16) -> Option<crate::ui::mouse::PaneId> {
        // The full visible mini-viz owns its border, footer, pending and fault
        // states too. Clipboard geometry remains text-only and separate.
        if self
            .panel_frames
            .get(crate::ui::panels::PanelKind::Artifacts)
            .is_some_and(|rect| crate::ui::mouse::point_in(rect, x, y))
        {
            return Some(crate::ui::mouse::PaneId::Artifacts);
        }
        self.panes.pane_at(x, y).map(|(pane, _)| pane).or_else(|| {
            self.panes.rect_of(crate::ui::mouse::PaneId::AgentBay)?;
            let frame = self
                .panel_frames
                .get(crate::ui::panels::PanelKind::AgentBay)?;
            crate::ui::mouse::point_in(frame, x, y).then_some(crate::ui::mouse::PaneId::AgentBay)
        })
    }

    pub(crate) fn scroll_pane_at(&mut self, x: u16, y: u16, rows: u16, up: bool) {
        let hit = self.interaction_pane_at(x, y);
        if let Some(pane) = hit {
            self.focus_pane_module(pane);
        }
        let pane = hit.unwrap_or(crate::ui::mouse::PaneId::Transcript);
        if up {
            self.scroll_pane_up(pane, rows);
        } else {
            self.scroll_pane_down(pane, rows);
        }
    }

    pub(crate) fn scroll_pane_up(&mut self, pane: crate::ui::mouse::PaneId, rows: u16) {
        match pane {
            crate::ui::mouse::PaneId::AgentBay => {
                self.reasoning_scroll = self.reasoning_scroll.saturating_add(usize::from(rows))
            }
            crate::ui::mouse::PaneId::Artifacts if self.scryglass.teaching_surface_open() => {
                self.scryglass.scroll_teaching_up(rows)
            }
            crate::ui::mouse::PaneId::Artifacts => {}
            _ => self.scroll = self.scroll.saturating_add(rows),
        }
    }

    pub(crate) fn scroll_pane_down(&mut self, pane: crate::ui::mouse::PaneId, rows: u16) {
        match pane {
            crate::ui::mouse::PaneId::AgentBay => {
                self.reasoning_scroll = self.reasoning_scroll.saturating_sub(usize::from(rows))
            }
            crate::ui::mouse::PaneId::Artifacts if self.scryglass.teaching_surface_open() => {
                self.scryglass.scroll_teaching_down(rows)
            }
            crate::ui::mouse::PaneId::Artifacts => {}
            _ => self.scroll = self.scroll.saturating_sub(rows),
        }
    }

    pub(crate) fn focus_pane_module(&mut self, pane: crate::ui::mouse::PaneId) {
        match pane {
            crate::ui::mouse::PaneId::Transcript => self.focus_module("core"),
            crate::ui::mouse::PaneId::Input => {
                self.focus_module("core");
                self.tool_strip.pause_for_reading(false);
            }
            crate::ui::mouse::PaneId::AgentBay => self.focus_module("agent"),
            crate::ui::mouse::PaneId::Artifacts => self.focus_module("artifacts"),
            crate::ui::mouse::PaneId::Shell => self.focus_module("shell"),
        }
    }

    pub(crate) fn toggle_module(&mut self, id: &str) {
        if self.module_host.is_running(id) {
            let text = self.close_module(id);
            self.system_msg(text);
        } else {
            let text = self.open_module(id);
            self.system_msg(text);
        }
    }
}
