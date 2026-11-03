use super::*;
use angel_delve::rules::reforge_jobs::{self, Claim, Completion, PolicyRequest, PolicyResult};

impl Sim {
    /// The existing command sequence is the discovery cursor. Unchanged
    /// frames borrow metadata only; captured cards copy once for a new job.
    pub fn new_reforge_claims(
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
        let claims = core.commands.pending_reforges()?;
        *cursor = Some((core.commands.session().into(), next));
        Ok(claims)
    }
    pub fn owns_reforge(&self, claim: &Claim) -> bool {
        let Some(live) = self.live.as_ref() else {
            return false;
        };
        let core = lock(&live.shared().core);
        core.commands.session() == claim.session && core.run.raid_id == claim.raid
    }
    pub fn claim_reforge(
        &mut self,
        player: u32,
        part: shooter::knights::Part,
        words: String,
    ) -> Result<Claim, String> {
        let live = self.live.as_mut().ok_or("no active game session")?;
        live.view.take();
        let mut core = lock(&live.shared().core);
        let Core {
            run,
            commands,
            edited,
            ..
        } = &mut *core;
        let claim = commands.claim_reforge(run, player, part, words)?;
        *edited = true;
        Ok(claim)
    }
    #[cfg(any(test, feature = "testing"))]
    pub fn pending_reforges(&self) -> Result<Vec<Claim>, String> {
        let Some(live) = self.live.as_ref() else {
            return Ok(Vec::new());
        };
        lock(&live.shared().core).commands.pending_reforges()
    }
    pub fn reforge_completion(&self, claim: &Claim) -> Result<Option<Completion>, String> {
        let live = self.live.as_ref().ok_or("the game session ended")?;
        lock(&live.shared().core).commands.reforge_completion(claim)
    }
    /// Transport/capacity errors retain the exact answer. A terminal receipt
    /// never invokes newer policy again. Logical rejection lets the broker cancel.
    pub fn answer_reforge(
        &mut self,
        claim: &Claim,
        input: cards::Card,
    ) -> Result<Result<Completion, Vec<String>>, String> {
        let live = self.live.as_mut().ok_or("the game session ended")?;
        live.view.take();
        let mut core = lock(&live.shared().core);
        if let Some(done) = core.commands.reforge_completion(claim)? {
            if done.input.as_ref() != Some(&input) {
                return Err("reforge answer differs from committed result".into());
            }
            return Ok(Ok(done));
        }
        let request = PolicyRequest {
            work: claim.work.clone(),
            card: input.clone(),
        };
        request.validate()?;
        let result = if let Some(engine) = core.engine.as_mut() {
            let parsed = std::cell::RefCell::new(None);
            engine.compute(
                reforge_jobs::CONTRACT,
                reforge_jobs::API,
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
                .ok_or("reforge policy reply was not validated")?
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
        let done = commands.complete_reforge(run, claim, input, accepted)?;
        *edited = true;
        Ok(Ok(done))
    }
    pub fn cancel_reforge(
        &mut self,
        claim: &Claim,
        reason: String,
    ) -> Result<Completion, String> {
        let live = self.live.as_mut().ok_or("the game session ended")?;
        live.view.take();
        let mut core = lock(&live.shared().core);
        let Core {
            run,
            commands,
            edited,
            ..
        } = &mut *core;
        let done = commands.cancel_reforge(run, claim, reason)?;
        *edited = true;
        Ok(done)
    }
    pub fn acknowledge_reforge(
        &mut self,
        claim: &Claim,
        done: &Completion,
    ) -> Result<(), String> {
        let live = self.live.as_mut().ok_or("the game session ended")?;
        let mut core = lock(&live.shared().core);
        core.commands.acknowledge_reforge(claim, done)?;
        core.edited = true;
        Ok(())
    }
}
