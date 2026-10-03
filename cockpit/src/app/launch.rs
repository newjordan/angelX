//! Process-local initial input. Never serialized or reconstructed from sessions.
use super::*;
use crate::agent::club::{Club, RouteIdentity};
use crate::interactive_launch::InteractiveLaunch;

#[derive(Clone)]
pub(crate) struct BoundRoute {
    pub club: Arc<dyn Club>,
    pub route: RouteIdentity,
    pub indices: (usize, usize),
    pub workspace: PathBuf,
    pub cli_effort: bool,
}
impl BoundRoute {
    pub fn current(&self, app: &App) -> bool {
        self.indices == app.bag.selected_route_indices()
            && self.route == app.bag.in_hand_route_identity()
            && self.workspace == app.tools.current_workspace()
            && self.workspace.is_dir()
            && std::env::current_dir().ok().as_deref() == Some(self.workspace.as_path())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LaunchState {
    Pending,
    Enqueued,
    Cancelled,
    Failed,
}
pub(crate) struct LaunchInput {
    pub state: LaunchState,
    pub text: Option<String>,
    pub binding: BoundRoute,
}
impl LaunchInput {
    fn transition(&mut self, state: LaunchState) -> Option<String> {
        if self.state != LaunchState::Pending || state == LaunchState::Pending {
            return None;
        }
        self.state = state;
        self.text.take()
    }
}

/// Restore once, then apply CLI selection. No auth/catalog rereads or env writes.
pub(crate) fn prepare(
    launch: &InteractiveLaunch,
    workspace: &std::path::Path,
) -> std::io::Result<(Bag, BoundRoute)> {
    let prepare = || -> Result<_, String> {
        let startup = crate::agent::codex_startup::CodexStartup::load_for_launch(launch)?;
        let mut bag = Bag::standard_for_launch(startup, launch);
        if let Some(driver) = &launch.driver {
            bag.select_launch_route(driver, launch.model.as_deref())?;
        }
        if launch.driver.is_some() || launch.effort.is_some() {
            let _ = crate::platform::route_preferences::restore_for_launch(
                &mut bag,
                launch.driver.is_some(),
                launch.effort.is_some(),
            );
        } else {
            let _ = crate::platform::route_preferences::restore(&mut bag);
        }
        if launch.prompt.is_some() && bag.pending_brain {
            return Err("initial turn requires a settled operational route".into());
        }
        let mut club = bag.in_hand();
        // CLI effort may correct an invalid ambient preference on either
        // subscription or API Responses routes. The checked override still
        // rejects model, catalog and transport errors before capture.
        if let Some(requested) = &launch.effort {
            let wire = club.checked_launch_effort(requested)?;
            if bag.set_reasoning_effort(&wire).is_none() {
                return Err("requested effort cannot be applied to this route".into());
            }
            club = crate::agent::club::launch_effort(club, requested, "cli")?;
        }
        if (launch.driver.is_some() || launch.effort.is_some() || launch.prompt.is_some())
            && club.resolved_model_defaults()["selection_error"].is_string()
        {
            return Err("selected route has an invalid startup decision".into());
        }
        let binding = BoundRoute {
            club,
            route: bag.in_hand_route_identity(),
            indices: bag.selected_route_indices(),
            workspace: workspace.to_path_buf(),
            cli_effort: launch.effort.is_some(),
        };
        Ok((bag, binding))
    };
    prepare().map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "requested launch route/model/effort is unavailable or unsupported",
        )
    })
}

impl App {
    pub(crate) fn install_launch_prompt(&mut self, text: String, binding: BoundRoute) {
        if self.launch_input.is_some() {
            return;
        }
        self.startup_intro
            .dismiss(Instant::now(), self.visual_motion);
        self.launch_input = Some(LaunchInput {
            state: LaunchState::Pending,
            text: Some(text),
            binding,
        });
    }
    pub(crate) fn launch_is_pending(&self) -> bool {
        self.launch_input
            .as_ref()
            .is_some_and(|input| input.state == LaunchState::Pending)
    }
    pub(crate) fn cancel_launch(&mut self, draft: bool) -> bool {
        let text = self
            .launch_input
            .as_mut()
            .and_then(|input| input.transition(LaunchState::Cancelled));
        if let Some(text) = text {
            if draft {
                if !self.input.is_empty() {
                    self.launch_typed_ahead = Some(std::mem::take(&mut self.input));
                }
                self.install_launch_draft(text);
            }
            self.launch_route = None;
            return true;
        }
        false
    }
    /// Call before an interaction that may change the binding. The cancellation
    /// draft and typed-ahead input are separate; never a steer or concatenation.
    pub(crate) fn launch_interaction(&mut self) {
        self.cancel_launch(true);
        self.launch_route = None;
    }
    pub(crate) fn advance_launch_input(&mut self) {
        if !self.launch_is_pending() {
            return;
        }
        let input = self.launch_input.as_ref().unwrap();
        if !input.binding.current(self) {
            self.cancel_launch(true);
            return;
        }
        if self.should_quit || self.exit_request.is_some() {
            self.cancel_launch(false);
            return;
        }
        if !input.binding.club.is_available() {
            self.launch_input
                .as_mut()
                .unwrap()
                .transition(LaunchState::Failed);
            self.launch_route = None;
            self.system_msg("initial turn not queued: bound route became unavailable".to_string());
            return;
        }
        if self.thinking.is_some()
            || self.pending_turn.is_some()
            || self.bg_job.is_some()
            || self.loop_pending.is_some()
            || self.campaign_pending.is_some()
            || self.pending_approval.is_some()
        {
            return;
        }
        let mut binding = input.binding.clone();
        if !binding.cli_effort {
            let text = self
                .launch_input
                .as_ref()
                .unwrap()
                .text
                .as_ref()
                .unwrap()
                .clone();
            self.apply_ultrathink_effort(&text);
            binding.club = self.bag.in_hand();
            if let Some(effort) = self.bag.reasoning_effort() {
                // Capture the session choice, not a later worker-thread getter.
                match crate::agent::club::launch_effort(
                    Arc::clone(&binding.club),
                    &effort,
                    "initial-capture",
                ) {
                    Ok(club) => binding.club = club,
                    Err(_) => {
                        self.launch_input
                            .as_mut()
                            .unwrap()
                            .transition(LaunchState::Failed);
                        self.launch_route = None;
                        self.system_msg(
                            "initial turn not queued: effort capture failed".to_string(),
                        );
                        return;
                    }
                }
            }
        }
        binding.route = binding.club.route_identity();
        let text = self
            .launch_input
            .as_mut()
            .unwrap()
            .transition(LaunchState::Enqueued)
            .unwrap();
        let raw: Arc<str> = text.into();
        self.messages
            .push(Message::new(Role::User, Arc::clone(&raw)));
        self.scroll = 0;
        self.pending_turn = Some(PendingTurn {
            raw: Arc::clone(&raw),
            user_msg: ChatMsg::user(raw.as_ref()),
            turn_evidence: None,
            echo_drawn: false,
            retry_draft: raw,
            clipboard_images: 0,
            launch: Some(binding),
        });
        self.launch_route = None;
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/launch_lifecycle__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../tests/cockpit/gate1_launch__tests.rs"]
mod gate1_tests;
