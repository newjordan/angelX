use super::schema::CreditRow;
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::path::PathBuf;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(super) struct RoutingPolicy {
    pub version: u64,
    pub updated_ms: u64,
    #[serde(default)]
    pub applied_run_ids: Vec<String>,
    pub priors: Vec<RoutePrior>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct RoutePrior {
    pub task_type: String,
    pub role: String,
    pub club: String,
    pub post_action_trials: u64,
    pub verified_wins: u64,
    pub reward_sum: f64,
    pub total_tokens: u64,
    pub total_elapsed_ms: u64,
}

impl RoutePrior {
    fn score(&self) -> f64 {
        let trials = self.post_action_trials.max(1) as f64;
        let posterior = (self.reward_sum + 1.0) / (trials + 2.0);
        let token_penalty = (self.total_tokens as f64 / trials / 80_000.0).min(0.12);
        let latency_penalty = (self.total_elapsed_ms as f64 / trials / 1_800_000.0).min(0.1);
        posterior - token_penalty - latency_penalty
    }
}

#[derive(Clone, Debug)]
pub(super) struct PolicyStore {
    path: PathBuf,
}

struct PolicyLease {
    file: std::fs::File,
}

impl Drop for PolicyLease {
    fn drop(&mut self) {
        use std::os::fd::AsRawFd;
        // SAFETY: this object owns the descriptor until after the unlock call.
        unsafe {
            libc::flock(self.file.as_raw_fd(), libc::LOCK_UN);
        }
    }
}

impl PolicyStore {
    pub(super) fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub(super) fn load(&self) -> Result<RoutingPolicy, String> {
        if !self.path.exists() {
            return Ok(RoutingPolicy {
                version: 1,
                ..RoutingPolicy::default()
            });
        }
        let raw =
            std::fs::read(&self.path).map_err(|e| format!("read {}: {e}", self.path.display()))?;
        let policy: RoutingPolicy = serde_json::from_slice(&raw)
            .map_err(|e| format!("parse {}: {e}", self.path.display()))?;
        if policy.version != 1 {
            return Err(format!("unsupported routing policy v{}", policy.version));
        }
        Ok(policy)
    }

    pub(super) fn save(&self, policy: &RoutingPolicy) -> Result<(), String> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
            secure_dir(dir)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        let body = serde_json::to_vec_pretty(policy)
            .map_err(|e| format!("serialize routing policy: {e}"))?;
        std::fs::write(&tmp, body).map_err(|e| format!("write {}: {e}", tmp.display()))?;
        secure_file(&tmp)?;
        std::fs::rename(&tmp, &self.path)
            .map_err(|e| format!("replace {}: {e}", self.path.display()))
    }

    pub(super) fn select(
        &self,
        task_type: &str,
        role: &str,
        candidates: &[String],
    ) -> Result<String, String> {
        if candidates.is_empty() {
            return Err("no reachable swarm compiler routes".to_string());
        }
        let policy = self.load()?;
        let observed_trials = policy
            .priors
            .iter()
            .filter(|prior| prior.task_type == task_type && prior.role == role)
            .map(|prior| prior.post_action_trials)
            .sum::<u64>();
        let mut ranked = candidates.to_vec();
        ranked.sort();
        ranked.sort_by(|left, right| {
            let score = |club: &str| {
                policy
                    .priors
                    .iter()
                    .find(|prior| {
                        prior.task_type == task_type && prior.role == role && prior.club == club
                    })
                    .map(|prior| {
                        let uncertainty = 0.04 / (prior.post_action_trials + 1) as f64;
                        prior.score() + uncertainty
                    })
                    .unwrap_or(if observed_trials > 0 { 0.72 } else { 0.5 })
            };
            score(right)
                .partial_cmp(&score(left))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(ranked.remove(0))
    }

    pub(super) fn record(
        &self,
        run_id: &str,
        task_type: &str,
        credits: &[CreditRow],
    ) -> Result<RoutingPolicy, String> {
        // Different swarm runs may finish at the same time. Hold one
        // cross-process lease across the load/modify/atomic-save transaction so
        // their post-action evidence cannot overwrite one another.
        let _lease = self.acquire_lease()?;
        let mut policy = self.load()?;
        if policy.applied_run_ids.iter().any(|seen| seen == run_id) {
            return Ok(policy);
        }
        for credit in credits {
            let index = policy.priors.iter().position(|prior| {
                prior.task_type == task_type
                    && prior.role == credit.role
                    && prior.club == credit.club
            });
            let prior = match index {
                Some(index) => &mut policy.priors[index],
                None => {
                    policy.priors.push(RoutePrior {
                        task_type: task_type.to_string(),
                        role: credit.role.clone(),
                        club: credit.club.clone(),
                        post_action_trials: 0,
                        verified_wins: 0,
                        reward_sum: 0.0,
                        total_tokens: 0,
                        total_elapsed_ms: 0,
                    });
                    policy
                        .priors
                        .last_mut()
                        .ok_or_else(|| "routing prior insertion failed".to_string())?
                }
            };
            prior.post_action_trials += 1;
            prior.verified_wins += u64::from(credit.reward >= 0.999);
            prior.reward_sum += credit.reward.clamp(0.0, 1.0);
            prior.total_tokens = prior.total_tokens.saturating_add(credit.tokens);
            prior.total_elapsed_ms = prior
                .total_elapsed_ms
                .saturating_add(credit.elapsed_ms.min(u64::MAX as u128) as u64);
        }
        policy.updated_ms = super::runner::now_ms();
        policy.applied_run_ids.push(run_id.to_string());
        if policy.applied_run_ids.len() > 4_096 {
            let drop_count = policy.applied_run_ids.len() - 4_096;
            policy.applied_run_ids.drain(..drop_count);
        }
        policy.priors.sort_by(|a, b| {
            (&a.task_type, &a.role, &a.club).cmp(&(&b.task_type, &b.role, &b.club))
        });
        self.save(&policy)?;
        Ok(policy)
    }

    fn acquire_lease(&self) -> Result<PolicyLease, String> {
        use std::os::fd::AsRawFd;
        let parent = self
            .path
            .parent()
            .ok_or_else(|| "routing policy path has no parent".to_string())?;
        std::fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
        secure_dir(parent)?;
        let path = self.path.with_extension("lock");
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|e| format!("open {}: {e}", path.display()))?;
        secure_file(&path)?;
        // SAFETY: `file` owns a live descriptor for the lifetime of PolicyLease.
        let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
        if rc != 0 {
            return Err(format!(
                "lock {}: {}",
                path.display(),
                std::io::Error::last_os_error()
            ));
        }
        Ok(PolicyLease { file })
    }

    #[cfg(test)]
    pub(super) fn path(&self) -> &std::path::Path {
        &self.path
    }
}

#[cfg(unix)]
fn secure_dir(path: &std::path::Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .map_err(|e| format!("secure {}: {e}", path.display()))
}

#[cfg(not(unix))]
fn secure_dir(_path: &std::path::Path) -> Result<(), String> {
    Ok(())
}

#[cfg(unix)]
fn secure_file(path: &std::path::Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .map_err(|e| format!("secure {}: {e}", path.display()))
}

#[cfg(not(unix))]
fn secure_file(_path: &std::path::Path) -> Result<(), String> {
    Ok(())
}
