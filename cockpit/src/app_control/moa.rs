use super::*;

impl App {
    pub(crate) fn close_moa_deck(&mut self) {
        self.moa_deck = None;
        self.scryglass
            .controller
            .leave_route(crate::scryglass::StageRoute::Formation);
    }

    pub(crate) fn open_moa_deck(&mut self, arg: Option<&str>) {
        match arg.map(str::trim).filter(|s| !s.is_empty()) {
            Some(a) if a.eq_ignore_ascii_case("clear") || a.eq_ignore_ascii_case("off") => {
                self.clear_moa_arm();
                self.system_msg(
                    "Formation cleared — normal coordinator turns restored".to_string(),
                );
            }
            Some(a) if a.eq_ignore_ascii_case("ledger") || a.eq_ignore_ascii_case("report") => {
                self.system_msg(crate::swarm::ledger::report_text(
                    None,
                    self.tools.current_workspace(),
                ));
            }
            Some(a) if a.eq_ignore_ascii_case("status") => {
                self.system_msg(self.moa_card_status_text());
            }
            Some(a) if formation_alias(a).is_some() => {
                let id = formation_alias(a).expect("checked above");
                self.open_moa_deck(None);
                self.select_moa_card(id);
                self.system_msg(format!(
                    "{} roster loaded — inspect or change each graph seat, then engage next turn or session",
                    crate::formations::formation(id).name
                ));
            }
            _ => {
                self.agent_menu = None;
                self.scryglass
                    .navigate(crate::scryglass::StageRoute::Formation);
                if self.moa_deck.is_none() {
                    let mut deck =
                        crate::formations::MoaDeckState::new(self.bag.moa_model_choices());
                    if let Some(active) = self.moa_one_shot.as_ref().or(self.moa_session.as_ref()) {
                        deck.load_engagement(active);
                    }
                    self.moa_deck = Some(deck);
                }
                let _ = self
                    .module_host
                    .activate(&crate::runtime::ModuleId::new("artifacts"));
                let live = if self.thinking.is_some() || self.bg_job.is_some() {
                    " · current run keeps its existing roster"
                } else {
                    ""
                };
                self.system_msg(format!(
                    "Formation roster open — Up/Down formation, Enter/N arm next turn, S session, Tab slots, T think{live}"
                ));
            }
        }
    }

    pub(crate) fn moa_deck_owns_input(&self) -> bool {
        self.moa_deck.is_some()
            && !self.shell_focused
            && self
                .module_host
                .focused()
                .is_some_and(|id| id.as_str() == "artifacts")
    }

    pub(crate) fn moa_deck_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        if !self.moa_deck_owns_input() {
            return false;
        }
        let Some(deck) = self.moa_deck.as_mut() else {
            return false;
        };
        // The live-turn interrupt contract outranks this display-only board.
        // Ctrl+G remains the global shell escape hatch; Ctrl+C joins Esc as a
        // live-turn interrupt. Every other modified chord stays captured so it
        // cannot edit the hidden composer beneath Formation.
        if matches!(code, KeyCode::Esc) && (self.thinking.is_some() || self.bg_job.is_some()) {
            return false;
        }
        if modifiers.contains(KeyModifiers::CONTROL) && matches!(code, KeyCode::Char('c' | 'g')) {
            return false;
        }
        if modifiers != KeyModifiers::NONE {
            return true;
        }
        if deck.focus() == crate::formations::MoaDeckFocus::Models {
            return match code {
                KeyCode::Esc => {
                    deck.close_model_picker();
                    true
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    deck.move_model(-1);
                    true
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    deck.move_model(1);
                    true
                }
                KeyCode::Enter => {
                    deck.assign_selected_model();
                    true
                }
                _ => matches!(code, KeyCode::Char(_)),
            };
        }
        if deck.focus() == crate::formations::MoaDeckFocus::Efforts {
            return match code {
                KeyCode::Esc => {
                    deck.close_effort_picker();
                    true
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    deck.move_effort(-1);
                    true
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    deck.move_effort(1);
                    true
                }
                KeyCode::Enter => {
                    self.stage_moa_effort_selected();
                    true
                }
                _ => matches!(code, KeyCode::Char(_)),
            };
        }
        match code {
            KeyCode::Esc => {
                self.close_moa_deck();
                true
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if deck.focus() == crate::formations::MoaDeckFocus::Slots {
                    deck.move_slot(-1);
                } else {
                    deck.move_selection(-1);
                }
                true
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if deck.focus() == crate::formations::MoaDeckFocus::Slots {
                    deck.move_slot(1);
                } else {
                    deck.move_selection(1);
                }
                true
            }
            KeyCode::Left | KeyCode::BackTab => {
                deck.focus_formations();
                true
            }
            KeyCode::Right | KeyCode::Tab => {
                deck.focus_slots();
                true
            }
            KeyCode::Enter => {
                if deck.focus() == crate::formations::MoaDeckFocus::Formations {
                    // Card-battle lock-in: Enter arms the selected formation for
                    // the next turn. Tab still drills into the roster graph for
                    // seat reassignment; S/A arms for the whole session.
                    self.play_selected_moa_card(false);
                } else {
                    deck.open_model_picker();
                }
                true
            }
            KeyCode::Char('m') => {
                deck.open_model_picker();
                true
            }
            KeyCode::Char('t') => {
                self.open_moa_effort_picker();
                true
            }
            KeyCode::Char('n') | KeyCode::Char(' ') => {
                self.play_selected_moa_card(false);
                true
            }
            KeyCode::Char('s') | KeyCode::Char('a') => {
                self.play_selected_moa_card(true);
                true
            }
            KeyCode::Char('c') => {
                self.clear_moa_arm();
                self.system_msg(
                    "Formation cleared — normal coordinator turns restored".to_string(),
                );
                true
            }
            _ => matches!(code, KeyCode::Char(_)),
        }
    }

    pub(crate) fn moa_arm_chip(&self) -> Option<String> {
        if let Some(engagement) = &self.moa_one_shot {
            let formation = crate::formations::formation(engagement.formation);
            return Some(format!(
                " · {} {}/{} armed ",
                formation.name,
                engagement.roster.assigned_count(),
                engagement.roster.slot_count()
            ));
        }
        self.moa_session.as_ref().map(|engagement| {
            let formation = crate::formations::formation(engagement.formation);
            format!(
                " · {} {}/{} session ",
                formation.name,
                engagement.roster.assigned_count(),
                engagement.roster.slot_count()
            )
        })
    }

    pub(crate) fn moa_arm_status(&self) -> String {
        match (&self.moa_one_shot, &self.moa_session) {
            (Some(next), Some(session)) => format!(
                "{} next · {} session",
                crate::formations::formation(next.formation).name,
                crate::formations::formation(session.formation).name
            ),
            (Some(next), None) => {
                format!(
                    "{} · next",
                    crate::formations::formation(next.formation).name
                )
            }
            (None, Some(session)) => {
                format!(
                    "{} · session",
                    crate::formations::formation(session.formation).name
                )
            }
            (None, None) => {
                // While the deck is open the world rail used to keep saying
                // "Solo" even with Grok War highlighted — selection felt dead.
                // Surface the draft pick until Enter/N/S (or re-click) arms it.
                if let Some(deck) = self.moa_deck.as_ref() {
                    let formation = deck.selected();
                    if formation.is_resting() {
                        "Solo · normal turns".to_string()
                    } else if !deck.selected_roster().is_ready() {
                        format!(
                            "{} · needs {}",
                            formation.name,
                            deck.selected_roster().missing_labels().join(",")
                        )
                    } else if !deck.selected_roster_online() {
                        format!("{} · route offline", formation.name)
                    } else {
                        format!("{} · draft · Enter arms", formation.name)
                    }
                } else {
                    "Solo · normal turns".to_string()
                }
            }
        }
    }

    /// Short label for the agent-panel FORMATION chip (armed > draft > plain).
    pub(crate) fn moa_formation_chip_label(&self) -> String {
        if let Some(engagement) = self.moa_one_shot.as_ref().or(self.moa_session.as_ref()) {
            let name = crate::formations::formation(engagement.formation).name;
            let scope = if self.moa_one_shot.is_some() {
                "next"
            } else {
                "sess"
            };
            return format!("{name}/{scope}");
        }
        if let Some(deck) = self.moa_deck.as_ref() {
            let formation = deck.selected();
            if !formation.is_resting() {
                return format!("{}/draft", formation.name);
            }
        }
        "FORMATION".to_string()
    }

    pub(crate) fn select_moa_card(&mut self, id: crate::formations::FormationId) {
        if let Some(deck) = self.moa_deck.as_mut() {
            deck.select(id);
        }
    }

    /// Mouse hit on a formation row. First click previews; clicking the already
    /// selected ready card arms it for the next turn so the world rail leaves
    /// Solo once the operator has actually locked the pick in.
    pub(crate) fn select_or_arm_moa_card(&mut self, id: crate::formations::FormationId) {
        let already_selected = self
            .moa_deck
            .as_ref()
            .is_some_and(|deck| deck.selected().id == id);
        self.select_moa_card(id);
        if !already_selected {
            return;
        }
        let ready = self.moa_deck.as_ref().is_some_and(|deck| {
            !deck.selected().is_resting()
                && deck.selected_roster().is_ready()
                && deck.selected_roster_online()
        });
        if ready {
            self.play_selected_moa_card(false);
        }
    }

    pub(crate) fn select_moa_slot(&mut self, index: usize) {
        if let Some(deck) = self.moa_deck.as_mut()
            && deck.select_slot(index)
        {
            deck.open_model_picker();
        }
    }

    pub(crate) fn assign_moa_model(&mut self, index: usize) {
        if let Some(deck) = self.moa_deck.as_mut() {
            deck.assign_model(index);
        }
    }

    /// Open the THINK picker for the focused Formation seat. The ladder is the
    /// assigned route's own declared reasoning levels — capability truth: a
    /// route that declares none gets the explicit n/a line, never an invented
    /// ladder — and every refusal to open says why.
    pub(crate) fn open_moa_effort_picker(&mut self) {
        let Some(deck) = self.moa_deck.as_ref() else {
            return;
        };
        let slots = deck.selected().slots();
        if slots.is_empty() {
            self.system_msg(
                "Solo Strike has no formation seats — the coordinator keeps its own THINK setting"
                    .to_string(),
            );
            return;
        }
        let slot_index = deck.selected_slot_index();
        let Some(slot) = slots.get(slot_index) else {
            return;
        };
        if slot.role == crate::formations::FormationRole::Scout {
            self.system_msg(format!(
                "{}: effort n/a — the swarm carries no scout seat effort; the scout rides its route's own THINK",
                slot.label()
            ));
            return;
        }
        let Some(route) = deck.selected_roster().assignment(slot_index) else {
            self.system_msg(format!(
                "{} has no model — assign one (Enter/M) before staging THINK effort",
                slot.label()
            ));
            return;
        };
        let (agent_index, route_slot_index) = (route.agent_index, route.slot_index);
        let levels = self
            .bag
            .route_choices()
            .iter()
            .find(|choice| {
                choice.agent_index == agent_index && choice.slot_index == route_slot_index
            })
            .map(|choice| choice.reasoning_levels.to_vec())
            .unwrap_or_default();
        if let Some(deck) = self.moa_deck.as_mut() {
            deck.open_effort_picker(levels);
        }
    }

    /// Stage THINK picker row `index` onto the focused seat's role (keyboard
    /// Enter today; the picker's mouse rows land with the Formation WorldButton
    /// wave). The staged value engages with the next armed turn/session, and
    /// the result is voiced either way.
    pub(crate) fn stage_moa_effort(&mut self, index: usize) {
        let Some(deck) = self.moa_deck.as_mut() else {
            return;
        };
        let Some((role, staged)) = deck.stage_effort(index) else {
            return;
        };
        match staged {
            Some(effort) => self.system_msg(format!(
                "{} seats staged at THINK {effort} — rides each seat call when this roster engages",
                role.label()
            )),
            None => self.system_msg(format!(
                "{} seats cleared — back to the env seat effort policy",
                role.label()
            )),
        }
    }

    pub(crate) fn stage_moa_effort_selected(&mut self) {
        let Some(index) = self
            .moa_deck
            .as_ref()
            .map(|deck| deck.selected_effort_index())
        else {
            return;
        };
        self.stage_moa_effort(index);
    }

    pub(crate) fn play_selected_moa_card(&mut self, session: bool) {
        let Some(deck) = self.moa_deck.as_ref() else {
            return;
        };
        let unavailable = deck.unavailable_slot_labels();
        let engagement = deck.engagement();
        if !unavailable.is_empty() {
            self.system_msg(format!(
                "Formation roster has offline or changed route(s) at {} — choose replacements before engaging",
                unavailable.join(", ")
            ));
            return;
        }
        let Some(engagement) = engagement else {
            return;
        };
        self.arm_moa_engagement(engagement, session);
    }

    pub(crate) fn clear_moa_cards(&mut self) {
        self.clear_moa_arm();
        self.system_msg("Formation cleared — normal coordinator turns restored".to_string());
    }

    /// Apply the staged formation at the turn boundary. `false` means the
    /// roster changed or went offline and the caller must keep the draft rather
    /// than silently sending it through another model. Successful engagement
    /// is carried by the rebuilt route itself, never by rewriting User text.
    pub(crate) fn apply_armed_moa_to_turn(&mut self) -> bool {
        let one_shot = self.moa_one_shot.clone();
        let Some(engagement) = one_shot.clone().or_else(|| self.moa_session.clone()) else {
            return true;
        };
        let formation = *crate::formations::formation(engagement.formation);
        if formation.is_resting() {
            return true;
        }
        if self.moa_restore_route.is_none() {
            self.moa_restore_route = Some(self.bag.selected_route_indices());
        }
        formation.apply_sota_env();
        match self.bag.activate_sota_moa_with_roster(&engagement.roster) {
            Ok(_) => {
                if one_shot.is_some() {
                    self.moa_one_shot = None;
                    self.moa_restore_after_turn = true;
                }
                true
            }
            Err(e) => {
                if one_shot.is_some() {
                    if let Some(session) = self.moa_session.as_ref() {
                        crate::formations::formation(session.formation).apply_sota_env();
                    } else {
                        crate::formations::formation(crate::formations::FormationId::SoloStrike)
                            .apply_sota_env();
                    }
                } else {
                    crate::formations::formation(crate::formations::FormationId::SoloStrike)
                        .apply_sota_env();
                }
                self.system_msg(format!(
                    "Formation could not engage: {e} — draft kept for repair; no turn was sent"
                ));
                self.open_moa_deck(None);
                false
            }
        }
    }

    /// Hand control back after a one-shot worker completes. If a session roster
    /// was staged during the run it becomes active now; otherwise restore the
    /// concrete route that was selected before engagement.
    pub(crate) fn restore_moa_after_turn(&mut self) {
        if !std::mem::take(&mut self.moa_restore_after_turn) {
            return;
        }
        if let Some(session) = self.moa_session.clone() {
            crate::formations::formation(session.formation).apply_sota_env();
            match self.bag.activate_sota_moa_with_roster(&session.roster) {
                Ok(_) => return,
                Err(error) => self.system_msg(format!(
                    "Session formation could not resume: {error} — restored the prior route"
                )),
            }
        }
        crate::formations::formation(crate::formations::FormationId::SoloStrike).apply_sota_env();
        if let Some((agent_index, slot_index)) = self.moa_restore_route.take()
            && !self.bag.restore_route(agent_index, slot_index)
        {
            self.bag.settle_in_hand();
        }
    }

    fn arm_moa_engagement(&mut self, engagement: crate::formations::MoaEngagement, session: bool) {
        let formation = *crate::formations::formation(engagement.formation);
        if formation.is_resting() {
            self.clear_moa_arm();
            self.close_moa_deck();
            self.system_msg("Solo Strike selected — normal coordinator turns restored".to_string());
            return;
        }
        if !engagement.roster.is_ready() {
            self.system_msg(format!(
                "Formation roster incomplete — assign model(s) to {} before engaging",
                engagement.roster.missing_labels().join(", ")
            ));
            return;
        }
        if session {
            self.moa_session = Some(engagement.clone());
            self.moa_one_shot = None;
        } else {
            self.moa_one_shot = Some(engagement.clone());
        }
        self.close_moa_deck();
        let scope = if session {
            "armed for session"
        } else {
            "armed for one turn"
        };
        let live = if self.thinking.is_some() || self.bg_job.is_some() {
            " · current run unchanged"
        } else {
            ""
        };
        // The staged THINK column rides the engagement; the receipt names each
        // staged seat so an effort override is never armed silently.
        let efforts = engagement.roster.seat_efforts();
        let staged = [
            (crate::formations::FormationRole::Propose, &efforts.propose),
            (crate::formations::FormationRole::Judge, &efforts.judge),
            (crate::formations::FormationRole::Verify, &efforts.verify),
            (
                crate::formations::FormationRole::Aggregate,
                &efforts.aggregate,
            ),
        ]
        .into_iter()
        .filter_map(|(role, effort)| {
            effort
                .as_deref()
                .map(|effort| format!("{} {effort}", role.label()))
        })
        .collect::<Vec<_>>();
        let think = if staged.is_empty() {
            String::new()
        } else {
            format!(" · THINK {}", staged.join(" / "))
        };
        self.system_msg(format!(
            "{} {scope} — {} roster seats ({} metered/remote · {} local), adaptive input spend · provider-native generation · final output shaped, est {}{}{}",
            formation.name,
            engagement.roster.slot_count(),
            engagement.roster.metered_count(),
            engagement.roster.local_count(),
            formation.est_cost_label(),
            think,
            live
        ));
    }

    pub(crate) fn clear_moa_arm(&mut self) {
        self.moa_one_shot = None;
        self.moa_session = None;
        if self.thinking.is_some() || self.bg_job.is_some() {
            self.moa_restore_after_turn |= self.moa_restore_route.is_some();
        } else {
            self.moa_restore_after_turn = false;
            crate::formations::formation(crate::formations::FormationId::SoloStrike)
                .apply_sota_env();
            if let Some((agent_index, slot_index)) = self.moa_restore_route.take()
                && !self.bag.restore_route(agent_index, slot_index)
            {
                self.bag.settle_in_hand();
            }
        }
    }

    fn moa_card_status_text(&self) -> String {
        let one = self
            .moa_one_shot
            .as_ref()
            .map(|engagement| crate::formations::formation(engagement.formation).name)
            .unwrap_or("(none)");
        let session = self
            .moa_session
            .as_ref()
            .map(|engagement| crate::formations::formation(engagement.formation).name)
            .unwrap_or("(none)");
        format!(
            "Agent formation\n  next turn {one}\n  session   {session}\n{}",
            self.bag.sota_moa_status()
        )
    }
}

fn formation_alias(raw: &str) -> Option<crate::formations::FormationId> {
    let alias = raw.trim().to_ascii_lowercase().replace(['_', '-'], " ");
    let alias = alias.split_whitespace().collect::<Vec<_>>().join(" ");
    match alias.as_str() {
        "gpu" | "gpu comp" | "gpu competition" | "gpu comp moa" | "comp" | "competition"
        | "overnight" | "sleep" | "night loop" | "overnight loop" => {
            Some(crate::formations::FormationId::GpuComp)
        }
        "solo" | "solo strike" | "rest" | "resting" => {
            Some(crate::formations::FormationId::SoloStrike)
        }
        "recon" | "scout" => Some(crate::formations::FormationId::Recon),
        "duel" => Some(crate::formations::FormationId::Duel),
        "council" => Some(crate::formations::FormationId::Council),
        "all in" | "allin" | "all" => Some(crate::formations::FormationId::AllIn),
        "grok" | "grok war" | "grokwar" | "war" | "war trio" | "trio" | "sota war"
        | "frontier war" | "last stand" | "battle" => Some(crate::formations::FormationId::GrokWar),
        "tag" | "tag team" | "tagteam" | "tag out" | "local" | "local pair" | "local tag"
        | "local tag team" | "home team" => Some(crate::formations::FormationId::TagTeam),
        "math" | "math god" | "mathgod" | "proximity" | "soundness" | "lean" => {
            Some(crate::formations::FormationId::MathGod)
        }
        "auto" | "auto moa" | "automoa" | "standing" | "standing moa" | "sota" | "sota moa" => {
            Some(crate::formations::FormationId::AutoMoa)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};

    #[test]
    fn t_key_opens_the_think_picker_and_enter_stages_the_level() {
        let _lock = crate::tests::env_lock();
        let mut app = crate::seed_preview_app();
        app.bag = crate::club::Bag::for_reasoning_render_test();
        app.open_moa_deck(None);
        app.select_moa_card(crate::formations::FormationId::Duel);
        {
            let deck = app.moa_deck.as_mut().unwrap();
            assert!(deck.select_slot(2)); // J1
            let sol = deck
                .models()
                .iter()
                .position(|choice| choice.route.model == "gpt-5.6-sol")
                .expect("ladder-declaring route");
            assert!(deck.assign_model(sol));
        }
        assert!(app.moa_deck_key(KeyCode::Char('t'), KeyModifiers::NONE));
        let deck = app.moa_deck.as_ref().unwrap();
        assert_eq!(deck.focus(), crate::formations::MoaDeckFocus::Efforts);
        assert_eq!(deck.effort_options(), ["low", "medium", "high"]);
        // Nothing staged: cursor on env default; Up wraps onto "high".
        assert!(app.moa_deck_key(KeyCode::Up, KeyModifiers::NONE));
        assert!(app.moa_deck_key(KeyCode::Enter, KeyModifiers::NONE));
        let deck = app.moa_deck.as_ref().unwrap();
        assert_eq!(deck.focus(), crate::formations::MoaDeckFocus::Slots);
        assert_eq!(
            deck.selected_roster()
                .role_effort(crate::formations::FormationRole::Judge),
            Some("high")
        );
        assert!(
            app.messages
                .iter()
                .any(|message| message.text.contains("staged at THINK high"))
        );
    }

    /// Every refusal to open the picker must say why — a silent `t` reads as
    /// nonexistence.
    #[test]
    fn think_gates_speak_for_resting_scout_and_unassigned_seats() {
        let _lock = crate::tests::env_lock();
        let mut app = crate::seed_preview_app();
        app.bag = crate::club::Bag::for_render_test(&[("alpha", &[("model-a", true)])]);
        app.open_moa_deck(None);

        // Solo Strike has no seats at all.
        assert!(app.moa_deck_key(KeyCode::Char('t'), KeyModifiers::NONE));
        assert!(
            app.messages
                .iter()
                .any(|message| message.text.contains("no formation seats"))
        );

        // Recon's scout seat: the swarm carries no scout effort policy.
        app.select_moa_card(crate::formations::FormationId::Recon);
        assert!(app.moa_deck.as_mut().unwrap().select_slot(0));
        assert!(app.moa_deck_key(KeyCode::Char('t'), KeyModifiers::NONE));
        assert!(
            app.messages
                .iter()
                .any(|message| message.text.contains("no scout seat effort"))
        );
        assert_ne!(
            app.moa_deck.as_ref().unwrap().focus(),
            crate::formations::MoaDeckFocus::Efforts
        );

        // Tag Team P1 finds no fleet box here: effort staging needs a route.
        app.select_moa_card(crate::formations::FormationId::TagTeam);
        assert!(app.moa_deck.as_mut().unwrap().select_slot(0));
        assert!(app.moa_deck_key(KeyCode::Char('t'), KeyModifiers::NONE));
        assert!(
            app.messages
                .iter()
                .any(|message| message.text.contains("has no model"))
        );
        assert_ne!(
            app.moa_deck.as_ref().unwrap().focus(),
            crate::formations::MoaDeckFocus::Efforts
        );
    }

    /// Arming a roster with a staged THINK column names it in the receipt.
    #[test]
    fn arm_receipt_names_the_staged_think_column() {
        let _lock = crate::tests::env_lock();
        let mut app = crate::seed_preview_app();
        app.bag = crate::club::Bag::for_reasoning_render_test();
        app.open_moa_deck(None);
        app.select_moa_card(crate::formations::FormationId::Duel);
        {
            let deck = app.moa_deck.as_mut().unwrap();
            assert!(deck.select_slot(2)); // J1
            assert!(deck.open_effort_picker(vec![
                "low".to_string(),
                "medium".to_string(),
                "high".to_string()
            ]));
            deck.move_effort(-1);
            assert!(deck.stage_selected_effort().is_some());
        }
        app.play_selected_moa_card(false);
        assert!(app.moa_one_shot.is_some());
        assert!(
            app.messages
                .iter()
                .any(|message| message.text.contains("THINK JUDGE high"))
        );
    }
}
