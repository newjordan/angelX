use super::*;
use crate::rules::reforge_jobs::{Claim, Completion};

impl Journal {
    pub fn pending_reforges(&self) -> Result<Vec<Claim>, String> {
        self.records
            .iter()
            .filter(|record| matches!(record.commands.as_slice(), [Command::ClaimReforge { .. }]))
            .map(
                |record| match record.receipt.result.as_ref().map(Vec::as_slice) {
                    Ok([Reply::Reforge(work)]) => Ok(Claim {
                        session: self.session.clone(),
                        raid: self.raid,
                        request: record.receipt.id,
                        work: work.clone(),
                    }),
                    _ => Err("invalid retained reforge claim".into()),
                },
            )
            .collect()
    }
    pub fn claim_reforge(
        &mut self,
        run: &mut Run,
        player: u32,
        part: knights::Part,
        words: String,
    ) -> Result<Claim, String> {
        let receipt = self.apply(
            run,
            Request {
                session: self.session.clone(),
                raid: self.raid,
                id: self.next_id()?,
                commands: vec![Command::ClaimReforge {
                    player,
                    part,
                    words,
                }],
            },
        )?;
        if let Err(error) = receipt.result {
            self.acknowledge(self.raid, receipt.id)?;
            return Err(error);
        }
        self.pending_reforges()?
            .into_iter()
            .find(|c| c.request == receipt.id)
            .ok_or("reforge claim was not retained".into())
    }
    fn reforge_record(&self, claim: &Claim) -> Result<usize, String> {
        if claim.session != self.session || claim.raid != self.raid {
            return Err("reforge belongs to another game session".into());
        }
        let index = self
            .records
            .iter()
            .position(|record| record.receipt.id == claim.request)
            .ok_or("reforge receipt retired; answer was not replayed")?;
        let record = &self.records[index];
        if !matches!(record.commands.as_slice(), [Command::ClaimReforge { .. }])
            || !matches!(record.receipt.result.as_ref().map(Vec::as_slice), Ok([Reply::Reforge(work)]) if work == &claim.work)
        {
            return Err("reforge identity reused with different captured inputs".into());
        }
        Ok(index)
    }
    pub fn reforge_completion(&self, claim: &Claim) -> Result<Option<Completion>, String> {
        Ok(self.records[self.reforge_record(claim)?].reforge.clone())
    }
    pub fn validate_reforge_world(&self, run: &Run) -> Result<(), String> {
        for (player, hero) in &run.players {
            if let Some(binding) = &hero.reforge_job {
                let owned = self.records.iter().any(|record|record.reforge.is_none() && matches!(record.receipt.result.as_ref().map(Vec::as_slice),Ok([Reply::Reforge(work)]) if work.player==*player && &work.result_id==binding));
                if !owned {
                    return Err("unowned reforge reservation in game candidate".into());
                }
            }
        }
        Ok(())
    }
    pub fn validate_reforge_handoff(&self, before: &Run, next: &Run) -> Result<(), String> {
        self.validate_reforge_world(next)?;
        for (player, hero) in &before.players {
            if let Some(next) = next.players.get(player)
                && hero.reforge_job != next.reforge_job
            {
                return Err("rules candidate changed retained reforge ownership".into());
            }
        }
        Ok(())
    }
    pub fn complete_reforge(
        &mut self,
        run: &mut Run,
        claim: &Claim,
        input: cards::Card,
        accepted: cards::Card,
    ) -> Result<Completion, String> {
        let index = self.reforge_record(claim)?;
        if let Some(done) = &self.records[index].reforge {
            if done.input.as_ref() != Some(&input) || done.accepted.as_ref() != Some(&accepted) {
                return Err("reforge completion reused with different content".into());
            }
            return Ok(done.clone());
        }
        let mut candidate = run.clone();
        let result = claim.work.install(&mut candidate, accepted.clone());
        self.commit_reforge(
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
    pub fn cancel_reforge(
        &mut self,
        run: &mut Run,
        claim: &Claim,
        reason: String,
    ) -> Result<Completion, String> {
        let index = self.reforge_record(claim)?;
        if let Some(done) = &self.records[index].reforge {
            return Ok(done.clone());
        }
        self.commit_reforge(
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
    fn commit_reforge(
        &mut self,
        run: &mut Run,
        index: usize,
        candidate: Option<Run>,
        done: Completion,
    ) -> Result<Completion, String> {
        if run.raid_id != self.raid {
            return Err("reforge delivery belongs to another raid".into());
        }
        let work = match self.records[index]
            .receipt
            .result
            .as_ref()
            .map(Vec::as_slice)
        {
            Ok([Reply::Reforge(work)]) => work,
            _ => return Err("invalid reforge receipt".into()),
        };
        if !done.valid(work) {
            return Err("invalid reforge completion".into());
        }
        let mut record = self.records[index].clone();
        record.reforge = Some(done.clone());
        let bytes = record_bytes(&record)?;
        let held = self.held_bytes - self.records[index].held_bytes + bytes;
        if held > MAX_PENDING_BYTES {
            return Err("reforge completion exceeds retained game byte capacity".into());
        }
        record.held_bytes = bytes;
        let mut candidate = if done.result.is_ok() {
            candidate.ok_or("missing prepared reforge world")?
        } else {
            run.clone()
        };
        if let Some(hero) = candidate.players.get_mut(&work.player)
            && hero.reforge_job.as_deref() == Some(&work.result_id)
        {
            hero.reforge_job = None;
        }
        *run = candidate;
        self.records[index] = record;
        self.held_bytes = held;
        Ok(done)
    }
    pub fn acknowledge_reforge(&mut self, claim: &Claim, done: &Completion) -> Result<(), String> {
        if claim.session != self.session || claim.raid != self.raid {
            return Err("reforge acknowledgement belongs to another session".into());
        }
        if self
            .records
            .iter()
            .any(|record| record.receipt.id == claim.request)
            && self.reforge_completion(claim)?.as_ref() != Some(done)
        {
            return Err("reforge acknowledgement differs from committed delivery".into());
        }
        self.acknowledge(claim.raid, claim.request)
    }
}
