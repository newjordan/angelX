//! The shipwrights: the host's model reads a whole set of salvage and
//! makes a vessel of it (`together_shooter::salvage`). One build at a time,
//! on a thread of its own, the way a wish goes to a grant runner
//! (`together_grants`): the brief, the game's checker on the answer, one
//! mend when it turns the answer away. Whatever happens, the build is never
//! lost: no answer, or no good one, and the yard's own fallback makes her.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::agent::club::Club;
use crate::drive::together_grants::{GRANT_CALL, GRANT_MOST};
use crate::drive::together_shooter::salvage::{self, Design, Plan, Vessel};

/// A build out with the model: its plan, the fleet as it stood, when it
/// went, and where the answer lands.
#[derive(Clone, Debug)]
pub(crate) struct Job {
    pub(crate) plan: Plan,
    pub(crate) fleet: Vec<Vessel>,
    pub(crate) started: Instant,
    answer: Arc<Mutex<Option<Result<Design, String>>>>,
}

impl Job {
    /// The pieces the build was asked for, by number.
    pub(crate) fn ids(&self) -> Vec<u32> {
        self.plan.pieces.iter().map(|p| p.id).collect()
    }
}

/// The host's shipwrights: the build in hand, if any.
#[derive(Clone, Debug, Default)]
pub(crate) struct Shipwright {
    pub(crate) job: Option<Job>,
}

impl Shipwright {
    /// Ask `club` for build `plan` on a thread of its own.
    #[cfg(test)]
    pub(crate) fn start(&mut self, club: Arc<dyn Club>, plan: Plan, fleet: Vec<Vessel>) {
        self.start_oracle(crate::drive::realm_services::oracle(club), plan, fleet);
    }

    pub(crate) fn start_oracle(&mut self, oracle: Arc<dyn angel_realm_host::services::Oracle>, plan: Plan, fleet: Vec<Vessel>) {
        let answer = Arc::new(Mutex::new(None));
        let slot = Arc::clone(&answer);
        let (p, f) = (plan.clone(), fleet.clone());
        let spawned = std::thread::Builder::new()
            .name("angel-shipwright".into())
            .spawn(move || {
                let made = design_with(&p, &f, |prompt| oracle.respond(prompt, GRANT_CALL));
                if let Ok(mut slot) = slot.lock() {
                    *slot = Some(made);
                }
            });
        if spawned.is_err() {
            // No thread to ask on: the fallback makes her at once.
            *answer.lock().unwrap_or_else(|e| e.into_inner()) = Some(Err("no shipwright thread".into()));
        }
        self.job = Some(Job { plan, fleet, started: Instant::now(), answer });
    }

    /// The build whose answer is in (or that has waited past
    /// `GRANT_MOST`): the job and the design to build, the model's or the
    /// fallback's, and whether the model made it.
    pub(crate) fn landed(&mut self) -> Option<(Job, Design, bool)> {
        let job = self.job.as_ref()?;
        let answer = job.answer.lock().ok().and_then(|mut a| a.take());
        let answer = match answer {
            Some(answer) => answer,
            None if job.started.elapsed() > GRANT_MOST + Duration::from_secs(10) => Err("the shipwrights took too long".into()),
            None => return None,
        };
        let job = self.job.take()?;
        Some(match answer {
            Ok(design) => (job, design, true),
            Err(_) => {
                let design = salvage::fallback(&job.plan, &job.fleet);
                (job, design, false)
            }
        })
    }
}

/// One build asked of `club`: the brief, the checker, one mend.
#[cfg(test)]
pub(crate) fn design(club: &dyn Club, plan: &Plan, fleet: &[Vessel]) -> Result<Design, String> {
    design_with(plan, fleet, |prompt| crate::agent::club::with_read_timeout_at_least(GRANT_CALL, || club.respond(prompt)))
}

fn design_with(plan: &Plan, fleet: &[Vessel], ask: impl Fn(&str) -> Result<String, String>) -> Result<Design, String> {
    let brief = salvage::brief(plan, fleet);
    let first = ask(&brief)?;
    match salvage::check(&first, fleet) {
        Ok(design) => Ok(design),
        Err(errors) => {
            let mend = format!(
                "{brief}\n\nYOUR FIRST ANSWER:\n{first}\nTHE GAME'S CHECKER TURNED IT AWAY:\n{}\n\nReply with the whole answer again, fixed, and nothing else.",
                errors.join("\n")
            );
            let second = ask(&mend)?;
            salvage::check(&second, fleet).map_err(|e| e.join("; "))
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/drive/together_salvage__tests.rs"]
mod tests;

#[cfg(test)]
impl Club for crate::drive::together_grants::Scripted {
    fn respond(&self, prompt: &str) -> Result<String, String> {
        angel_realm_host::services::Oracle::respond(self, prompt, GRANT_CALL)
    }
    fn label(&self) -> &str { "scripted" }
}
