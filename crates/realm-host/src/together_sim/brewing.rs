use super::*;
use angel_delve::rules::brew_jobs::{self, Claim, Completion, PolicyRequest, PolicyResult};

impl Sim {
    /// The existing command sequence is the discovery cursor. Unchanged
    /// frames borrow metadata only; captured cards copy once for a new job.
    pub fn new_brew_claims(
        &self,
        cursor: &mut Option<(String, u64)>,
    ) -> Result<Vec<Claim>, String> {
        let Some(live) = self.live.as_ref() else {
            return Ok(Vec::new());
        };
        let core = lock(&live.shared().core);
        let next = core.commands.next_id()?;
        if cursor
            .as_ref()
            .is_some_and(|(session, id)| session == core.commands.session() && *id == next)
        {
            return Ok(Vec::new());
        }
        let claims = core.commands.pending_brews()?;
        *cursor = Some((core.commands.session().into(), next));
        Ok(claims)
    }
    pub fn owns_brew(&self, claim: &Claim) -> bool {
        let Some(live) = self.live.as_ref() else {
            return false;
        };
        let core = lock(&live.shared().core);
        core.commands.session() == claim.session && core.run.raid_id == claim.raid
    }
    pub fn claim_brew(&mut self) -> Result<Option<Claim>, String> {
        let Some(live) = self.live.as_mut() else {
            return Ok(None);
        };
        live.view.take();
        let mut core = lock(&live.shared().core);
        let Core {
            run,
            commands,
            edited,
            ..
        } = &mut *core;
        let queued = run.brewing.len();
        let claim = commands.claim_brew(run)?;
        *edited |= claim.is_some() || queued != run.brewing.len();
        Ok(claim)
    }
    pub fn pending_brews(&self) -> Result<Vec<Claim>, String> {
        let Some(live) = self.live.as_ref() else {
            return Ok(Vec::new());
        };
        lock(&live.shared().core).commands.pending_brews()
    }
    pub fn brew_completion(&self, claim: &Claim) -> Result<Option<Completion>, String> {
        let live = self.live.as_ref().ok_or("the game session ended")?;
        lock(&live.shared().core).commands.brew_completion(claim)
    }
    /// Transport/capacity errors retain the job. Only a policy rejection asks
    /// the model to mend; a terminal receipt never invokes new policy again.
    pub fn answer_brew(
        &mut self,
        claim: &Claim,
        input: cards::Card,
    ) -> Result<Result<Completion, Vec<String>>, String> {
        let live = self.live.as_mut().ok_or("the game session ended")?;
        live.view.take();
        let mut core = lock(&live.shared().core);
        if let Some(done) = core.commands.brew_completion(claim)? {
            if done.input.as_ref() != Some(&input) {
                return Err("brew answer differs from committed result".into());
            }
            return Ok(Ok(done));
        }
        let request = PolicyRequest {
            work: claim.work.clone(),
            card: input.clone(),
            taken: shooter::cauldron::taken(&core.run.book),
        };
        request.validate()?;
        let result = if let Some(engine) = core.engine.as_mut() {
            let parsed = std::cell::RefCell::new(None);
            engine.compute(
                brew_jobs::CONTRACT,
                brew_jobs::API,
                serde_json::to_value(&request).map_err(|e| e.to_string())?,
                |value| {
                    let result = <PolicyResult as serde::Deserialize>::deserialize(value)
                        .map_err(|e| e.to_string())?;
                    request.validate_result(&result)?;
                    parsed.replace(Some(result));
                    Ok(())
                },
            )?;
            parsed
                .into_inner()
                .ok_or("brew policy reply was not validated")?
        } else {
            request.evaluate()?
        };
        let accepted = match result {
            Ok(card) => card,
            Err(errors) => return Ok(Err(errors)),
        };
        let Core {
            run,
            commands,
            edited,
            ..
        } = &mut *core;
        let done = commands.complete_brew(run, claim, input, accepted)?;
        *edited = true;
        Ok(Ok(done))
    }
    pub fn cancel_brew(
        &mut self,
        claim: &Claim,
        reason: String,
    ) -> Result<Completion, String> {
        let live = self.live.as_mut().ok_or("the game session ended")?;
        let mut core = lock(&live.shared().core);
        let Core {
            run,
            commands,
            edited,
            ..
        } = &mut *core;
        let done = commands.cancel_brew(run, claim, reason)?;
        *edited = true;
        Ok(done)
    }
    pub fn acknowledge_brew(
        &mut self,
        claim: &Claim,
        done: &Completion,
    ) -> Result<(), String> {
        let live = self.live.as_mut().ok_or("the game session ended")?;
        let mut core = lock(&live.shared().core);
        core.commands.acknowledge_brew(claim, done)?;
        core.edited = true;
        Ok(())
    }
}
