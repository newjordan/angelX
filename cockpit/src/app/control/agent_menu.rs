use super::*;
use crate::ui::agent_panel::controls::{AgentControlMenu, AgentMenuAction, AgentMenuKind};

const MAX_AGENT_MENU_QUERY_CHARS: usize = 64;

impl App {
    pub(crate) fn remember_brain_route(&self) {
        crate::platform::route_preferences::remember(&self.bag);
    }

    fn record_brain_route_receipt(&mut self) {
        let Some(choice) = self
            .bag
            .route_choices()
            .iter()
            .find(|choice| choice.selected)
            .cloned()
        else {
            return;
        };
        let mut route = choice.model;
        if let Some(effort) = choice.reasoning_effort {
            route.push('@');
            route.push_str(&effort);
        }
        self.brain_route_receipt = Some(crate::app::BrainRouteReceipt {
            label: format!("{} ▸ {route}", choice.agent),
            applied_at: std::time::Instant::now(),
        });
    }

    fn route_choice_for_target(
        &self,
        target: (usize, usize),
    ) -> Option<crate::agent::club::RouteChoice> {
        self.bag
            .route_choices()
            .iter()
            .find(|choice| choice.agent_index == target.0 && choice.slot_index == target.1)
            .cloned()
    }

    fn confirm_context_risk(&mut self, target: (usize, usize)) -> bool {
        let Some(choice) = self.route_choice_for_target(target) else {
            return false;
        };
        let context_used = self
            .tools
            .gauge
            .used_tokens
            .load(std::sync::atomic::Ordering::Relaxed);
        let requires_confirmation = !choice.selected
            && choice
                .metadata
                .context_usage_percent(context_used)
                .is_some_and(|percent| percent >= 95);
        if !requires_confirmation {
            return true;
        }
        if self
            .agent_menu
            .is_some_and(|menu| menu.confirm_target == Some(target))
        {
            return true;
        }

        let route_index = self
            .bag
            .route_choices()
            .iter()
            .position(|choice| choice.agent_index == target.0 && choice.slot_index == target.1);
        if let Some(menu) = self.agent_menu.as_mut() {
            menu.confirm_target = Some(target);
            if menu.kind == AgentMenuKind::Model
                && let Some(route_index) = route_index
            {
                menu.selected = route_index;
            }
        }
        false
    }

    fn open_thinking_menu_for_route(&mut self, target: (usize, usize)) -> bool {
        let Some(choice) = self.route_choice_for_target(target) else {
            return false;
        };
        if !choice.available || choice.reasoning_levels.is_empty() {
            return false;
        }
        let selected = choice
            .reasoning_effort
            .as_deref()
            .and_then(|effort| {
                choice
                    .reasoning_levels
                    .iter()
                    .position(|level| level.eq_ignore_ascii_case(effort))
            })
            .unwrap_or(0);
        self.agent_menu = Some(AgentControlMenu {
            kind: AgentMenuKind::Thinking,
            selected,
            route_target: Some(target),
            confirm_target: None,
        });
        self.agent_menu_search = None;
        self.selection = None;
        true
    }

    fn menu_effort_levels(&self, menu: AgentControlMenu) -> Vec<String> {
        menu.route_target
            .and_then(|target| self.route_choice_for_target(target))
            .map(|choice| choice.reasoning_levels.to_vec())
            .unwrap_or_default()
    }

    fn menu_route_available(&self, menu: AgentControlMenu) -> bool {
        menu.route_target
            .and_then(|target| self.route_choice_for_target(target))
            .is_some_and(|choice| choice.available)
    }

    pub(crate) fn agent_menu_indices(
        &self,
        menu: AgentControlMenu,
        enabled_only: bool,
    ) -> Vec<usize> {
        let query = self.agent_menu_search.as_deref().unwrap_or_default();
        match menu.kind {
            AgentMenuKind::Model => self
                .bag
                .route_choices()
                .iter()
                .enumerate()
                .filter(|(_, choice)| {
                    (!enabled_only || choice.available)
                        && (choice.available
                            || choice.selected
                            || self.agent_menu_show_unavailable
                            || !query.is_empty())
                        && crate::ui::agent_panel::controls::query_matches(
                            query,
                            &[
                                &choice.agent,
                                &choice.driver,
                                &choice.model,
                                &crate::ui::agent_panel::controls::connection_label(choice),
                            ],
                        )
                })
                .map(|(index, _)| index)
                .collect(),
            AgentMenuKind::Thinking => {
                let route_available = self.menu_route_available(menu);
                self.menu_effort_levels(menu)
                    .iter()
                    .enumerate()
                    .filter(|(_, effort)| {
                        (!enabled_only || route_available)
                            && crate::ui::agent_panel::controls::query_matches(
                                query,
                                &[effort.as_str()],
                            )
                    })
                    .map(|(index, _)| index)
                    .collect()
            }
        }
    }

    fn reconcile_agent_menu_search(&mut self) {
        let Some(menu) = self.agent_menu else {
            return;
        };
        if let Some(open) = self.agent_menu.as_mut() {
            open.confirm_target = None;
        }
        let matching = self.agent_menu_indices(menu, false);
        if matching.contains(&menu.selected) {
            return;
        }
        let next = self
            .agent_menu_indices(menu, true)
            .into_iter()
            .next()
            .or_else(|| matching.into_iter().next());
        if let (Some(open), Some(next)) = (self.agent_menu.as_mut(), next) {
            open.selected = next;
        }
    }

    pub(crate) fn open_agent_menu(&mut self, kind: AgentMenuKind) {
        if self.thinking.is_some() || self.bg_job.is_some() {
            return;
        }
        self.agent_menu_search = None;
        self.agent_menu_details = false;
        self.agent_menu_show_unavailable = false;
        self.close_moa_deck();
        if self
            .route_evidence_loaded_at
            .is_none_or(|loaded| loaded.elapsed() >= std::time::Duration::from_secs(30))
        {
            self.route_evidence = crate::knowledge::route_intelligence::load_default();
            self.route_evidence_loaded_at = Some(std::time::Instant::now());
        }
        match kind {
            AgentMenuKind::Model => {
                let choices = self.bag.route_choices();
                if choices.is_empty() {
                    return;
                }
                let selected = choices
                    .iter()
                    .position(|choice| choice.selected)
                    .or_else(|| choices.iter().position(|choice| choice.available))
                    .unwrap_or(0);
                self.agent_menu = Some(AgentControlMenu {
                    kind,
                    selected,
                    route_target: None,
                    confirm_target: None,
                });
                self.selection = None;
            }
            AgentMenuKind::Thinking => {
                let Some(target) = self
                    .bag
                    .route_choices()
                    .iter()
                    .find(|choice| choice.selected)
                    .map(|choice| (choice.agent_index, choice.slot_index))
                else {
                    return;
                };
                self.open_thinking_menu_for_route(target);
            }
        }
    }

    pub(crate) fn open_agent_menu_filtered(&mut self, kind: AgentMenuKind, query: &str) -> bool {
        if self.thinking.is_some() || self.bg_job.is_some() {
            return false;
        }
        self.open_agent_menu(kind);
        if self.agent_menu.is_none() {
            return false;
        }
        self.agent_menu_search = Some(query.chars().take(MAX_AGENT_MENU_QUERY_CHARS).collect());
        self.reconcile_agent_menu_search();
        true
    }

    pub(crate) fn open_model_menu_command(&mut self, raw: &str) -> bool {
        let raw = raw.trim();
        let combined = raw.rsplit_once('@').and_then(|(model, effort)| {
            let (model, effort) = (model.trim(), effort.trim());
            (!model.is_empty() && !effort.is_empty()).then_some((model, effort))
        });
        let model_query = combined.map_or(raw, |(model, _)| model);
        if !self.open_agent_menu_filtered(AgentMenuKind::Model, model_query) {
            return false;
        }
        let Some((_, effort_query)) = combined else {
            return true;
        };

        let exact = self
            .bag
            .route_choices()
            .iter()
            .filter(|choice| {
                choice.available
                    && [&choice.agent, &choice.driver, &choice.model]
                        .iter()
                        .any(|field| field.eq_ignore_ascii_case(model_query))
            })
            .cloned()
            .collect::<Vec<_>>();
        if exact.len() != 1 || exact[0].reasoning_levels.is_empty() {
            return true;
        }
        let target = (exact[0].agent_index, exact[0].slot_index);
        if self.open_thinking_menu_for_route(target) {
            self.agent_menu_search = Some(
                effort_query
                    .chars()
                    .take(MAX_AGENT_MENU_QUERY_CHARS)
                    .collect(),
            );
            self.reconcile_agent_menu_search();
        }
        true
    }

    pub(crate) fn move_agent_menu(&mut self, delta: i32) {
        let Some(menu) = self.agent_menu else {
            return;
        };
        let indices = self.agent_menu_indices(menu, true);
        if indices.is_empty() {
            return;
        }
        let position = indices.iter().position(|index| *index == menu.selected);
        let next = match (position, delta >= 0) {
            (Some(position), true) => indices[(position + 1) % indices.len()],
            (Some(position), false) => indices[(position + indices.len() - 1) % indices.len()],
            (None, true) => indices[0],
            (None, false) => *indices.last().expect("non-empty menu indices"),
        };
        if let Some(open) = self.agent_menu.as_mut() {
            open.selected = next;
            open.confirm_target = None;
        }
    }

    /// Cycle the in-hand route's thinking effort across its reasoning levels,
    /// returning the applied effort when one exists.
    ///
    /// Test-only. This was written for a deliberate `[`/`]` keybind, but those
    /// keys now drive the Scryglass gallery (`turn_io.rs`) and the agent menu's
    /// own movement (`agent_menu.rs`), so no production path calls it: the live
    /// way to choose an effort is `open_thinking_menu_for_route`. Give it a key
    /// it owns, or delete it together with its tests.
    #[cfg(test)]
    fn cycle_thinking(&mut self, forward: bool) -> Option<String> {
        let levels = self.bag.reasoning_levels();
        if levels.is_empty() {
            return None;
        }
        let current = self.bag.reasoning_effort();
        let position = current.as_deref().and_then(|effort| {
            levels
                .iter()
                .position(|level| level.eq_ignore_ascii_case(effort))
        });
        let next = match (position, forward) {
            (Some(i), true) => &levels[(i + 1) % levels.len()],
            (Some(i), false) => &levels[(i + levels.len() - 1) % levels.len()],
            (None, true) => &levels[0],
            (None, false) => levels.last().expect("non-empty levels"),
        };
        let applied = self.bag.set_reasoning_effort(next)?;
        self.remember_brain_route();
        Some(applied)
    }

    fn toggle_agent_menu_kind(&mut self) {
        let Some(menu) = self.agent_menu else {
            return;
        };
        match menu.kind {
            AgentMenuKind::Model => {
                let target = self
                    .bag
                    .route_choices()
                    .get(menu.selected)
                    .map(|choice| (choice.agent_index, choice.slot_index));
                if let Some(target) = target {
                    self.open_thinking_menu_for_route(target);
                }
            }
            AgentMenuKind::Thinking => {
                let choices = self.bag.route_choices();
                let selected = menu
                    .route_target
                    .and_then(|target| {
                        choices.iter().position(|choice| {
                            choice.agent_index == target.0 && choice.slot_index == target.1
                        })
                    })
                    .or_else(|| choices.iter().position(|choice| choice.selected))
                    .unwrap_or(0);
                if !choices.is_empty() {
                    self.agent_menu_search = None;
                    self.agent_menu = Some(AgentControlMenu {
                        kind: AgentMenuKind::Model,
                        selected,
                        route_target: None,
                        confirm_target: None,
                    });
                }
            }
        }
    }

    fn apply_agent_menu_selection(&mut self) {
        let Some(menu) = self.agent_menu else {
            return;
        };
        if !self.agent_menu_indices(menu, true).contains(&menu.selected) {
            return;
        }
        let applied = match menu.kind {
            AgentMenuKind::Model => self
                .bag
                .route_choices()
                .get(menu.selected)
                .cloned()
                .is_some_and(|choice| {
                    let target = (choice.agent_index, choice.slot_index);
                    self.confirm_context_risk(target)
                        && self.bag.select_route(choice.agent_index, choice.slot_index)
                }),
            AgentMenuKind::Thinking => match menu.route_target {
                Some((agent_index, slot_index)) => self
                    .menu_effort_levels(menu)
                    .get(menu.selected)
                    .cloned()
                    .is_some_and(|effort| {
                        self.confirm_context_risk((agent_index, slot_index))
                            && self
                                .bag
                                .select_route_with_effort(agent_index, slot_index, &effort)
                    }),
                None => false,
            },
        };
        if applied {
            self.remember_brain_route();
            self.record_brain_route_receipt();
            self.agent_menu = None;
            self.agent_menu_search = None;
        }
    }

    fn focus_operational_pick(&mut self) {
        let Some(menu) = self.agent_menu else {
            return;
        };
        let choices = self.bag.route_choices();
        let selected = match menu.kind {
            AgentMenuKind::Model => {
                let context_used = self
                    .tools
                    .gauge
                    .used_tokens
                    .load(std::sync::atomic::Ordering::Relaxed);
                crate::knowledge::route_intelligence::operational_pick_for_context(
                    &self.route_evidence,
                    &choices,
                    context_used,
                )
            }
            AgentMenuKind::Thinking => menu.route_target.and_then(|target| {
                let choice = choices.iter().find(|choice| {
                    choice.agent_index == target.0 && choice.slot_index == target.1
                })?;
                if !choice.available {
                    return None;
                }
                crate::knowledge::route_intelligence::operational_effort_pick(
                    &self.route_evidence,
                    choice,
                    &choice.reasoning_levels,
                )
            }),
        };
        self.agent_menu_search = None;
        if let Some(open) = self.agent_menu.as_mut() {
            open.confirm_target = None;
            if let Some(selected) = selected {
                open.selected = selected;
            }
        }
    }

    fn focus_quality_pick(&mut self) {
        let Some(menu) = self.agent_menu else {
            return;
        };
        let choices = self.bag.route_choices();
        let selected = match menu.kind {
            AgentMenuKind::Model => {
                let context_used = self
                    .tools
                    .gauge
                    .used_tokens
                    .load(std::sync::atomic::Ordering::Relaxed);
                crate::knowledge::route_intelligence::quality_pick_for_context(
                    &self.route_evidence,
                    &choices,
                    context_used,
                )
            }
            AgentMenuKind::Thinking => menu.route_target.and_then(|target| {
                let choice = choices.iter().find(|choice| {
                    choice.agent_index == target.0 && choice.slot_index == target.1
                })?;
                if !choice.available {
                    return None;
                }
                crate::knowledge::route_intelligence::quality_effort_pick(
                    &self.route_evidence,
                    choice,
                    &choice.reasoning_levels,
                )
            }),
        };
        self.agent_menu_search = None;
        if let Some(open) = self.agent_menu.as_mut() {
            open.confirm_target = None;
            if let Some(selected) = selected {
                open.selected = selected;
            }
        }
    }

    /// Captures all keyboard input while the deck is open, so ordinary typing
    /// cannot leak into the composer beneath it.
    pub(crate) fn agent_menu_key(&mut self, code: KeyCode) -> bool {
        if self.agent_menu.is_none() {
            return false;
        }
        if self.thinking.is_some() || self.bg_job.is_some() {
            self.agent_menu = None;
            self.agent_menu_search = None;
            return true;
        }
        if code == KeyCode::Esc
            && self
                .agent_menu
                .is_some_and(|menu| menu.confirm_target.is_some())
        {
            self.agent_menu = None;
            self.agent_menu_search = None;
            return true;
        }
        if self.agent_menu_search.is_some() {
            match code {
                KeyCode::Esc => {
                    self.agent_menu_search = None;
                    self.reconcile_agent_menu_search();
                    return true;
                }
                KeyCode::Backspace => {
                    if let Some(query) = self.agent_menu_search.as_mut() {
                        query.pop();
                    }
                    self.reconcile_agent_menu_search();
                    return true;
                }
                KeyCode::Char(character) => {
                    if self
                        .agent_menu_search
                        .as_ref()
                        .is_some_and(|query| query.chars().count() < MAX_AGENT_MENU_QUERY_CHARS)
                    {
                        if let Some(query) = self.agent_menu_search.as_mut() {
                            query.push(character);
                        }
                        self.reconcile_agent_menu_search();
                    }
                    return true;
                }
                _ => {}
            }
        }
        match code {
            KeyCode::Esc => {
                self.agent_menu = None;
                self.agent_menu_search = None;
            }
            KeyCode::Up | KeyCode::Char('k') => self.move_agent_menu(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_agent_menu(1),
            KeyCode::Tab | KeyCode::Left | KeyCode::Right => self.toggle_agent_menu_kind(),
            KeyCode::Char('/') => {
                self.agent_menu_search = Some(String::new());
                self.reconcile_agent_menu_search();
            }
            KeyCode::Char('o') => self.focus_operational_pick(),
            KeyCode::Char('q') => self.focus_quality_pick(),
            KeyCode::Char('d') => {
                self.agent_menu_details = !self.agent_menu_details;
            }
            KeyCode::Char('a') => {
                self.agent_menu_show_unavailable = !self.agent_menu_show_unavailable;
                self.reconcile_agent_menu_search();
            }
            KeyCode::Char('+') => {
                let message = self.rate_last_turn(Some("useful"));
                self.system_msg(message);
            }
            KeyCode::Char('-') => {
                let message = self.rate_last_turn(Some("miss"));
                self.system_msg(message);
            }
            KeyCode::Char('[') | KeyCode::Char(']') => {
                // A preview must never change the in-hand route. Inspect the
                // highlighted model's own ladder; Enter commits both choices.
                if self
                    .agent_menu
                    .is_some_and(|menu| menu.kind == AgentMenuKind::Model)
                {
                    self.toggle_agent_menu_kind();
                }
                if self
                    .agent_menu
                    .is_some_and(|menu| menu.kind == AgentMenuKind::Thinking)
                {
                    self.move_agent_menu(if code == KeyCode::Char(']') { 1 } else { -1 });
                }
            }
            KeyCode::Enter => self.apply_agent_menu_selection(),
            _ => {}
        }
        true
    }

    pub(crate) fn apply_agent_menu_action(&mut self, action: AgentMenuAction) {
        if self.thinking.is_some() || self.bg_job.is_some() {
            self.agent_menu = None;
            return;
        }
        let applied = match action {
            AgentMenuAction::ToggleDetails => {
                self.agent_menu_details = !self.agent_menu_details;
                return;
            }
            AgentMenuAction::ToggleUnavailable => {
                self.agent_menu_show_unavailable = !self.agent_menu_show_unavailable;
                self.reconcile_agent_menu_search();
                return;
            }
            AgentMenuAction::SelectRoute {
                agent_index,
                slot_index,
            } => {
                self.confirm_context_risk((agent_index, slot_index))
                    && self.bag.select_route(agent_index, slot_index)
            }
            AgentMenuAction::InspectEffort {
                agent_index,
                slot_index,
            } => {
                self.open_thinking_menu_for_route((agent_index, slot_index));
                return;
            }
            AgentMenuAction::SelectEffort {
                agent_index,
                slot_index,
                effort,
            } => {
                self.confirm_context_risk((agent_index, slot_index))
                    && self
                        .bag
                        .select_route_with_effort(agent_index, slot_index, &effort)
            }
        };
        if applied {
            self.remember_brain_route();
            self.record_brain_route_receipt();
            self.agent_menu = None;
            self.agent_menu_search = None;
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/app_control__agent_menu__route_deck_tests.rs"]
mod route_deck_tests;
