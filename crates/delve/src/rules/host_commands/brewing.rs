use super::*;
use crate::rules::brew_jobs::{self, Claim, Completion};

impl Journal {
    pub fn pending_brews(&self) -> Result<Vec<Claim>, String> {
        self.records
            .iter()
            .filter(|record| matches!(record.commands.as_slice(), [Command::ClaimBrew]))
            .map(
                |record| match record.receipt.result.as_ref().map(Vec::as_slice) {
                    Ok([Reply::Brew(work)]) => Ok(Claim {
                        session: self.session.clone(),
                        raid: self.raid,
                        request: record.receipt.id,
                        work: work.clone(),
                    }),
                    _ => Err("invalid retained brew claim".into()),
                },
            )
            .collect()
    }
    pub fn claim_brew(&mut self, run: &mut Run) -> Result<Option<Claim>, String> {
        if run.raid_id != self.raid {
            return Err("brew belongs to another raid".into());
        }
        if run.brewing.is_empty() {
            return Ok(None);
        }
        if !brew_jobs::Work::capturable(run, &run.brewing[0]) {
            // The player changed the slot or departed before model admission.
            // Retire that exact source fact through the ordinary command port;
            // it must not block later requests or attach to a later recipe.
            let ask = run.brewing[0].clone();
            let receipt = self.apply(
                run,
                Request {
                    session: self.session.clone(),
                    raid: self.raid,
                    id: self.next_id()?,
                    commands: vec![Command::DropQueuedBrew { ask }],
                },
            )?;
            self.acknowledge(self.raid, receipt.id)?;
            receipt.result?;
            return Ok(None);
        }
        let receipt = self.apply(
            run,
            Request {
                session: self.session.clone(),
                raid: self.raid,
                id: self.next_id()?,
                commands: vec![Command::ClaimBrew],
            },
        )?;
        if let Err(error) = receipt.result {
            self.acknowledge(self.raid, receipt.id)?;
            return Err(error);
        }
        self.pending_brews()?
            .into_iter()
            .find(|claim| claim.request == receipt.id)
            .map(Some)
            .ok_or("brew claim was not retained".into())
    }
    fn brew_record(&self, claim: &Claim) -> Result<usize, String> {
        if claim.session != self.session || claim.raid != self.raid {
            return Err("brew belongs to another game session".into());
        }
        let index = self
            .records
            .iter()
            .position(|record| record.receipt.id == claim.request)
            .ok_or("brew receipt retired; answer was not replayed")?;
        let record = &self.records[index];
        if !matches!(record.commands.as_slice(), [Command::ClaimBrew])
            || !matches!(record.receipt.result.as_ref().map(Vec::as_slice), Ok([Reply::Brew(work)]) if work == &claim.work)
        {
            return Err("brew identity reused with different captured inputs".into());
        }
        Ok(index)
    }
    pub fn brew_completion(&self, claim: &Claim) -> Result<Option<Completion>, String> {
        Ok(self.records[self.brew_record(claim)?].brew.clone())
    }
    pub fn validate_brew_world(&self, run: &Run) -> Result<(), String> {
        for record in &self.records {
            if let Ok([Reply::Brew(work)]) = record.receipt.result.as_ref().map(Vec::as_slice)
                && record.brew.is_none()
                && run.book.get(&work.placeholder.id) != Some(&work.placeholder)
            {
                return Err("game candidate erased or changed a pending brew binding".into());
            }
        }
        Ok(())
    }
    pub fn complete_brew(
        &mut self,
        run: &mut Run,
        claim: &Claim,
        input: cards::Card,
        accepted: cards::Card,
    ) -> Result<Completion, String> {
        let index = self.brew_record(claim)?;
        if let Some(done) = &self.records[index].brew {
            if done.input.as_ref() != Some(&input) || done.accepted.as_ref() != Some(&accepted) {
                return Err("brew completion reused with different content".into());
            }
            return Ok(done.clone());
        }
        let mut candidate = run.clone();
        let result =
            if candidate.book.get(&claim.work.placeholder.id) != Some(&claim.work.placeholder) {
                Err("the brew's captured stand-in changed".into())
            } else {
                brew_jobs::install(&mut candidate, &claim.work.ask, accepted.clone())
            };
        self.commit_brew(
            run,
            index,
            Some(candidate),
            Completion {
                input: Some(input),
                accepted: Some(accepted),
                result,
            },
        )
    }
    pub fn cancel_brew(
        &mut self,
        run: &mut Run,
        claim: &Claim,
        reason: String,
    ) -> Result<Completion, String> {
        let index = self.brew_record(claim)?;
        if let Some(done) = &self.records[index].brew {
            return Ok(done.clone());
        }
        self.commit_brew(
            run,
            index,
            None,
            Completion {
                input: None,
                accepted: None,
                result: Err(reason),
            },
        )
    }
    fn commit_brew(
        &mut self,
        run: &mut Run,
        index: usize,
        candidate: Option<Run>,
        done: Completion,
    ) -> Result<Completion, String> {
        if run.raid_id != self.raid {
            return Err("brew delivery belongs to another raid".into());
        }
        let work = match self.records[index]
            .receipt
            .result
            .as_ref()
            .map(Vec::as_slice)
        {
            Ok([Reply::Brew(work)]) => work,
            _ => return Err("invalid brew receipt".into()),
        };
        if !done.valid(work) {
            return Err("invalid brew completion".into());
        }
        let mut record = self.records[index].clone();
        record.brew = Some(done.clone());
        let bytes = record_bytes(&record)?;
        let held = self.held_bytes - self.records[index].held_bytes + bytes;
        if held > MAX_PENDING_BYTES {
            return Err("brew completion exceeds retained game byte capacity".into());
        }
        record.held_bytes = bytes;
        if done.result.is_ok() {
            *run = candidate.ok_or("missing prepared brew world")?;
        }
        self.records[index] = record;
        self.held_bytes = held;
        Ok(done)
    }
    pub fn acknowledge_brew(&mut self, claim: &Claim, done: &Completion) -> Result<(), String> {
        if claim.session != self.session || claim.raid != self.raid {
            return Err("brew acknowledgement belongs to another session".into());
        }
        if self
            .records
            .iter()
            .any(|record| record.receipt.id == claim.request)
            && self.brew_completion(claim)?.as_ref() != Some(done)
        {
            return Err("brew acknowledgement differs from committed delivery".into());
        }
        self.acknowledge(claim.raid, claim.request)
    }
}
