//! Operator-triggered, one-flight campaign orchestration.

use super::*;
use crate::harness::{
    AuthorizedCampaignBase, CampaignAlignmentReceipt, PreparedSwarmRun, SwarmCompilerEngine,
    SwarmRunReceipt,
};
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) enum CampaignPending {
    Inspect {
        engine: Arc<SwarmCompilerEngine>,
        cancelled: Arc<AtomicBool>,
        rx: mpsc::Receiver<Result<crate::harness::CampaignBase, String>>,
    },
    Prepare {
        engine: Arc<SwarmCompilerEngine>,
        cancelled: Arc<AtomicBool>,
        rx: mpsc::Receiver<Result<(PreparedSwarmRun, AuthorizedCampaignBase), String>>,
    },
    Execute {
        cancelled: Arc<AtomicBool>,
        rx: mpsc::Receiver<Result<SwarmRunReceipt, String>>,
    },
    Review {
        cancelled: Arc<AtomicBool>,
        rx: mpsc::Receiver<Result<CampaignAlignmentReceipt, String>>,
    },
}

impl CampaignPending {
    fn cancel(&self) {
        let cancelled = match self {
            Self::Inspect { cancelled, .. }
            | Self::Prepare { cancelled, .. }
            | Self::Execute { cancelled, .. }
            | Self::Review { cancelled, .. } => cancelled,
        };
        cancelled.store(true, Ordering::Release);
    }

    fn is_cancelled(&self) -> bool {
        match self {
            Self::Inspect { cancelled, .. }
            | Self::Prepare { cancelled, .. }
            | Self::Execute { cancelled, .. }
            | Self::Review { cancelled, .. } => cancelled.load(Ordering::Acquire),
        }
    }
}

impl App {
    pub(crate) fn campaign_command(&mut self, arg: Option<&str>) -> String {
        let command = match crate::campaign::CampaignCommand::parse(arg) {
            Ok(command) => command,
            Err(error) => return format!("campaign error: {error}"),
        };
        if !matches!(
            &command,
            crate::campaign::CampaignCommand::Advance | crate::campaign::CampaignCommand::Review
        ) {
            if matches!(
                &command,
                crate::campaign::CampaignCommand::Pause | crate::campaign::CampaignCommand::Abandon
            ) && let Some(pending) = self.campaign_pending.as_ref()
            {
                pending.cancel();
            }
            let workspace = self.tools.current_workspace().to_path_buf();
            let goal = self.goal.clone();
            return self.campaign.command(arg, &workspace, goal.as_ref());
        }
        if self.thinking.is_some()
            || self.bg_job.is_some()
            || self.campaign_pending.is_some()
            || self.loop_pending.is_some()
            || self.pending_approval.is_some()
            || self.loop_active()
        {
            return "campaign error: the flight slot is busy; no campaign budget was spent"
                .to_string();
        }
        let Some(engine) = self.tools.swarm_compiler() else {
            return "campaign error: this cockpit has no internal swarm compiler service"
                .to_string();
        };
        let cancelled = Arc::new(AtomicBool::new(false));
        match command {
            crate::campaign::CampaignCommand::Advance => {
                let (tx, rx) = mpsc::channel();
                let worker = Arc::clone(&engine);
                std::thread::spawn(move || {
                    let _ = tx.send(worker.inspect_campaign_base());
                });
                self.campaign_pending = Some(CampaignPending::Inspect {
                    engine,
                    cancelled,
                    rx,
                });
                "campaign advance authorized · inspecting and freezing the clean base".to_string()
            }
            crate::campaign::CampaignCommand::Review => {
                let request = match self.campaign.begin_alignment_review() {
                    Ok(request) => request,
                    Err(error) => return format!("campaign review stopped: {error}"),
                };
                let (tx, rx) = mpsc::channel();
                let worker_cancelled = Arc::clone(&cancelled);
                std::thread::spawn(move || {
                    let result =
                        engine.campaign_alignment_review(request, worker_cancelled.as_ref());
                    let _ = tx.send(result);
                });
                self.campaign_pending = Some(CampaignPending::Review { cancelled, rx });
                "campaign review authorized · running one exact read-only alignment gate"
                    .to_string()
            }
            _ => unreachable!("non-flight campaign command returned above"),
        }
    }

    pub(crate) fn campaign_drain_pending(&mut self) {
        let Some(pending) = self.campaign_pending.take() else {
            return;
        };
        if pending.is_cancelled() {
            return;
        }
        match pending {
            CampaignPending::Inspect {
                engine,
                cancelled,
                rx,
            } => match rx.try_recv() {
                Ok(Ok(base)) => match self.campaign.freeze_round(base) {
                    Ok(launch) => {
                        let (tx, next_rx) = mpsc::channel();
                        let worker = Arc::clone(&engine);
                        let worker_cancelled = Arc::clone(&cancelled);
                        std::thread::spawn(move || {
                            if worker_cancelled.load(Ordering::Acquire) {
                                let _ = tx.send(Err(
                                    "campaign preparation cancelled by operator".to_string()
                                ));
                                return;
                            }
                            let authorization = launch.authorization;
                            let result = worker
                                .campaign_prepare(launch.request, authorization.clone())
                                .map(|prepared| (prepared, authorization));
                            let _ = tx.send(result);
                        });
                        self.campaign_pending = Some(CampaignPending::Prepare {
                            engine,
                            cancelled,
                            rx: next_rx,
                        });
                        self.system_msg(
                            "campaign round contract frozen · preparing its durable swarm graph"
                                .to_string(),
                        );
                    }
                    Err(error) => self.system_msg(format!("campaign advance stopped: {error}")),
                },
                Ok(Err(error)) => {
                    self.system_msg(format!("campaign base inspection failed: {error}"))
                }
                Err(mpsc::TryRecvError::Empty) => {
                    self.campaign_pending = Some(CampaignPending::Inspect {
                        engine,
                        cancelled,
                        rx,
                    })
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.system_msg("campaign base worker disconnected".to_string())
                }
            },
            CampaignPending::Prepare {
                engine,
                cancelled,
                rx,
            } => match rx.try_recv() {
                Ok(Ok((prepared, authorization))) => {
                    match self.campaign.attach_prepared(&prepared, &authorization) {
                        Ok(()) => {
                            let run_id = prepared.run_id.clone();
                            let (tx, next_rx) = mpsc::channel();
                            let worker_cancelled = Arc::clone(&cancelled);
                            std::thread::spawn(move || {
                                let result = engine.campaign_execute(
                                    run_id,
                                    authorization,
                                    worker_cancelled.as_ref(),
                                );
                                let _ = tx.send(result);
                            });
                            self.campaign_pending = Some(CampaignPending::Execute {
                                cancelled,
                                rx: next_rx,
                            });
                            self.system_msg(format!(
                                "campaign attached swarm run {} · executing one proof round",
                                prepared.run_id
                            ));
                        }
                        Err(error) => self.campaign_pause_after_error(&error),
                    }
                }
                Ok(Err(error)) => self.campaign_pause_after_error(&error),
                Err(mpsc::TryRecvError::Empty) => {
                    self.campaign_pending = Some(CampaignPending::Prepare {
                        engine,
                        cancelled,
                        rx,
                    })
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.campaign_pause_after_error("campaign prepare worker disconnected")
                }
            },
            CampaignPending::Execute { cancelled, rx } => match rx.try_recv() {
                Ok(Ok(receipt)) => match self.campaign.finish_round(receipt) {
                    Ok(message) => self.system_msg(message),
                    Err(error) => self.campaign_pause_after_error(&error),
                },
                Ok(Err(error)) => self.campaign_pause_after_error(&error),
                Err(mpsc::TryRecvError::Empty) => {
                    self.campaign_pending = Some(CampaignPending::Execute { cancelled, rx })
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.campaign_pause_after_error("campaign execution worker disconnected")
                }
            },
            CampaignPending::Review { cancelled, rx } => match rx.try_recv() {
                Ok(Ok(receipt)) => match self.campaign.finish_alignment_review(receipt) {
                    Ok(message) => self.system_msg(message),
                    Err(error) => self.campaign_block_alignment(&error),
                },
                Ok(Err(error)) => self.campaign_block_alignment(&error),
                Err(mpsc::TryRecvError::Empty) => {
                    self.campaign_pending = Some(CampaignPending::Review { cancelled, rx })
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.campaign_block_alignment("campaign alignment worker disconnected")
                }
            },
        }
    }

    fn campaign_pause_after_error(&mut self, error: &str) {
        let message = match self.campaign.pause_execution(error) {
            Ok(paused) => paused,
            Err(pause_error) => {
                format!("campaign execution error: {error}; durable pause failed: {pause_error}")
            }
        };
        self.system_msg(message);
    }

    fn campaign_block_alignment(&mut self, error: &str) {
        let message = self
            .campaign
            .fail_alignment_review(error)
            .unwrap_or_else(|block_error| {
                format!("campaign alignment error: {error}; durable block failed: {block_error}")
            });
        self.system_msg(message);
    }
}
