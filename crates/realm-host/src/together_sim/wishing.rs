use super::*;
use angel_delve::rules::wish_jobs::{self, Claim, Completion, PolicyRequest, PolicyResult};

impl Sim {
    pub fn wish_session(&self) -> Option<String> {
        self.live
            .as_ref()
            .map(|live| lock(&live.shared().core).commands.session().to_owned())
    }
    pub fn owns_wish(&self, claim: &Claim) -> bool {
        self.live.as_ref().is_some_and(|live| {
            let core = lock(&live.shared().core);
            core.commands.session() == claim.session && core.run.raid_id == claim.raid
        })
    }
    /// Claim only newly queued intentions. Unchanged frames copy no definitions.
    pub fn new_wish_claims(
        &mut self,
        cursor: &mut Option<(String, u64)>,
        maker: &str,
    ) -> Result<Vec<Claim>, String> {
        let Some(live) = self.live.as_mut() else {
            return Ok(Vec::new());
        };
        let shared = Arc::clone(live.shared());
        let mut core = lock(&shared.core);
        let mut admission_error = None;
        while let Some(ask) = core.run.asked.first().cloned() {
            let maker = if ask.knight == 1 {
                maker.to_string()
            } else {
                core.run
                    .players
                    .get(&ask.knight)
                    .map_or_else(String::new, |h| h.name.clone())
            };
            let Core {
                run,
                commands,
                edited,
                ..
            } = &mut *core;
            if let Err(error) = commands.claim_wish(run, ask, maker) {
                admission_error = Some(error);
                break;
            }
            *edited = true;
            live.view.take();
        }
        let next = core.commands.next_id()?;
        if cursor
            .as_ref()
            .is_some_and(|(session, id)| session == core.commands.session() && *id == next)
        {
            return admission_error.map_or(Ok(Vec::new()), Err);
        }
        let claims = core.commands.pending_wishes()?;
        *cursor = Some((core.commands.session().into(), next));
        Ok(claims)
    }
    pub fn wish_completion(&self, claim: &Claim) -> Result<Option<Completion>, String> {
        let live = self.live.as_ref().ok_or("the game session ended")?;
        lock(&live.shared().core).commands.wish_completion(claim)
    }
    pub fn answer_wish(
        &mut self,
        claim: &Claim,
        input: Option<shooter::phrasebook::Wish>,
        authored: bool,
        reason: Option<String>,
    ) -> Result<Completion, String> {
        let live = self.live.as_mut().ok_or("the game session ended")?;
        live.view.take();
        let mut core = lock(&live.shared().core);
        if let Some(done) = core.commands.wish_completion(claim)? {
            if done.input != input
                || done.authored != authored
                || (input.is_none() && done.reason != reason)
            {
                return Err("wish answer differs from committed result".into());
            }
            return Ok(done);
        }
        let mut rejection = reason;
        let accepted = if let Some(wish) = &input {
            let request = PolicyRequest {
                work: claim.work.clone(),
                wish: wish.clone(),
            };
            request.validate()?;
            let result = if let Some(engine) = core.engine.as_mut() {
                let parsed = std::cell::RefCell::new(None);
                engine.compute(
                    wish_jobs::CONTRACT,
                    wish_jobs::API,
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
                    .ok_or("wish policy reply was not validated")?
            } else {
                request.evaluate()?
            };
            match result {
                Ok(wish) => Some(wish),
                Err(errors) => {
                    rejection = Some(errors.join("; ").chars().take(2000).collect());
                    None
                }
            }
        } else {
            None
        };
        let Core {
            run,
            commands,
            edited,
            ..
        } = &mut *core;
        // Logical policy rejection is terminal and refunds exactly once.
        let done = commands.complete_wish(run, claim, input, accepted, authored, rejection)?;
        *edited = true;
        Ok(done)
    }
    pub fn acknowledge_wish(
        &mut self,
        claim: &Claim,
        done: &Completion,
    ) -> Result<(), String> {
        let live = self.live.as_mut().ok_or("the game session ended")?;
        let mut core = lock(&live.shared().core);
        core.commands.acknowledge_wish(claim, done)?;
        core.edited = true;
        Ok(())
    }
}
