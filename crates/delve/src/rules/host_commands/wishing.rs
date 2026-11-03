use super::*;
use crate::rules::wish_jobs::{Claim, Completion, Outcome};

impl Journal {
    pub fn pending_wishes(&self) -> Result<Vec<Claim>, String> {
        self.records
            .iter()
            .filter(|r| matches!(r.commands.as_slice(), [Command::ClaimWish { .. }]))
            .map(
                |record| match record.receipt.result.as_ref().map(Vec::as_slice) {
                    Ok([Reply::Wish(work)]) => Ok(Claim {
                        session: self.session.clone(),
                        raid: self.raid,
                        request: record.receipt.id,
                        work: work.clone(),
                    }),
                    _ => Err("invalid retained wish claim".into()),
                },
            )
            .collect()
    }
    pub fn claim_wish(
        &mut self,
        run: &mut Run,
        ask: super::super::genie::Asked,
        maker: String,
    ) -> Result<Claim, String> {
        let receipt = self.apply(
            run,
            Request {
                session: self.session.clone(),
                raid: self.raid,
                id: self.next_id()?,
                commands: vec![Command::ClaimWish { ask, maker }],
            },
        )?;
        if let Err(error) = receipt.result {
            self.acknowledge(self.raid, receipt.id)?;
            return Err(error);
        }
        self.pending_wishes()?
            .into_iter()
            .find(|c| c.request == receipt.id)
            .ok_or("wish claim was not retained".into())
    }
    fn wish_record(&self, claim: &Claim) -> Result<usize, String> {
        if claim.session != self.session || claim.raid != self.raid {
            return Err("wish belongs to another game session".into());
        }
        let index = self
            .records
            .iter()
            .position(|r| r.receipt.id == claim.request)
            .ok_or("wish receipt retired; answer was not replayed")?;
        let record = &self.records[index];
        if !matches!(record.commands.as_slice(), [Command::ClaimWish { .. }])
            || !matches!(record.receipt.result.as_ref().map(Vec::as_slice), Ok([Reply::Wish(work)]) if work == &claim.work)
        {
            return Err("wish identity reused with different captured inputs".into());
        }
        Ok(index)
    }
    pub fn wish_completion(&self, claim: &Claim) -> Result<Option<Completion>, String> {
        Ok(self.records[self.wish_record(claim)?].wish.clone())
    }
    pub fn validate_wish_world(&self, run: &Run) -> Result<(), String> {
        for (player, hero) in &run.players {
            let mut seen = std::collections::BTreeSet::new();
            for binding in &hero.wish_jobs {
                if !seen.insert(binding) || !self.records.iter().any(|r| r.wish.is_none() && matches!(r.receipt.result.as_ref().map(Vec::as_slice), Ok([Reply::Wish(work)]) if work.ask.knight == *player && &work.binding == binding)) {
                    return Err("unowned wish reservation in game candidate".into());
                }
            }
        }
        Ok(())
    }
    pub fn validate_wish_handoff(&self, before: &Run, next: &Run) -> Result<(), String> {
        self.validate_wish_world(next)?;
        if before.seat_epochs != next.seat_epochs {
            return Err("rules candidate changed seat incarnations".into());
        }
        for (player, hero) in &before.players {
            if let Some(next) = next.players.get(player)
                && hero.wish_jobs != next.wish_jobs
            {
                return Err("rules candidate changed retained wish ownership".into());
            }
        }
        Ok(())
    }
    pub fn complete_wish(
        &mut self,
        run: &mut Run,
        claim: &Claim,
        input: Option<super::super::phrasebook::Wish>,
        accepted: Option<super::super::phrasebook::Wish>,
        authored: bool,
        reason: Option<String>,
    ) -> Result<Completion, String> {
        let index = self.wish_record(claim)?;
        if let Some(done) = &self.records[index].wish {
            if done.input != input
                || done.accepted != accepted
                || done.authored != authored
                || done.reason != reason
            {
                return Err("wish completion reused with different content".into());
            }
            return Ok(done.clone());
        }
        if run.raid_id != self.raid {
            return Err("wish delivery belongs to another raid".into());
        }
        let mut done = Completion {
            input,
            accepted,
            authored,
            reason,
            outcome: Outcome::Stale(String::new()),
        };
        if !done.valid(&claim.work) {
            return Err("invalid wish completion".into());
        }
        let mut candidate = run.clone();
        done.outcome = claim.work.deliver(
            &mut candidate,
            done.accepted.as_ref(),
            authored,
            done.reason.as_deref().unwrap_or("it could not be granted"),
        );
        let mut record = self.records[index].clone();
        record.wish = Some(done.clone());
        let bytes = record_bytes(&record)?;
        let held = self.held_bytes - self.records[index].held_bytes + bytes;
        if held > MAX_PENDING_BYTES {
            return Err("wish completion exceeds retained game byte capacity".into());
        }
        record.held_bytes = bytes;
        *run = candidate;
        self.records[index] = record;
        self.held_bytes = held;
        Ok(done)
    }
    pub fn acknowledge_wish(&mut self, claim: &Claim, done: &Completion) -> Result<(), String> {
        if claim.session != self.session || claim.raid != self.raid {
            return Err("wish acknowledgement belongs to another session".into());
        }
        if self.records.iter().any(|r| r.receipt.id == claim.request)
            && self.wish_completion(claim)?.as_ref() != Some(done)
        {
            return Err("wish acknowledgement differs from committed delivery".into());
        }
        self.acknowledge(claim.raid, claim.request)
    }
}
