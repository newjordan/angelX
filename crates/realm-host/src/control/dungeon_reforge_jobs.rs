//! Native model coordination only. The existing game journal owns all requests,
//! captured kit and terminal results; this queue holds model/file/IO handles.
use super::*;

impl Session<'_> {
    pub(super) fn discover_reforges(&mut self) {
        let sim = &self.dungeon.shooter;
        self.dungeon
            .forge
            .reforges
            .retain(|f| f.claim.as_ref().is_some_and(|c| sim.owns_reforge(c)));
        if self
            .dungeon
            .forge
            .forging
            .as_ref()
            .and_then(|f| f.claim.as_ref())
            .is_some_and(|c| !sim.owns_reforge(c))
        {
            // Do not interrupt the model. Its session-scoped file cannot enter a new game.
            self.dungeon.forge.forging = None;
        }
        let discovered = (|| -> Result<(), String> {
            for claim in self
                .dungeon
                .shooter
                .new_reforge_claims(&mut self.dungeon.forge.reforge_cursor)?
            {
                if self
                    .dungeon
                    .forge
                    .forging
                    .as_ref()
                    .and_then(|f| f.claim.as_ref())
                    == Some(&claim)
                    || self
                        .dungeon
                        .forge
                        .reforges
                        .iter()
                        .any(|f| f.claim.as_ref() == Some(&claim))
                {
                    continue;
                }
                let completed = self.dungeon.shooter.reforge_completion(&claim)?;
                let path = self
                    .reforged_dir()
                    .join(format!("{}.card", claim.work.result_id));
                let launched = completed.is_none() && path.is_file();
                self.dungeon.forge.reforges.push_back(Forging {
                    part: claim.work.part,
                    id: claim.work.result_id.clone(),
                    words: claim.work.words.clone(),
                    answer: completed.as_ref().and_then(|done| done.input.clone()),
                    completed,
                    claim: Some(claim),
                    cancellation: None,
                    notes: String::new(),
                    persisted: false,
                    acknowledged: false,
                    started: Instant::now(),
                    seen: launched,
                    launched,
                });
            }
            if self.dungeon.forge.reforges.iter().any(|f| !f.persisted) {
                if self.dungeon.save_path.is_none() {
                    return Err("reforge checkpoint path is unavailable".into());
                }
                self.dungeon.checkpoint(Instant::now(), true)?;
                for f in &mut self.dungeon.forge.reforges {
                    f.persisted = true;
                }
            }
            Ok(())
        })();
        if let Err(error) = discovered {
            self.dungeon.notice = format!("Reforge retained: {error}");
        }
    }
    pub(super) fn next_host_reforge(&mut self) {
        if self.dungeon.forge.forging.is_some() {
            return;
        }
        let idle = !self.facts().busy();
        // Terminal answers may drain while unrelated model work is running.
        let index = self.dungeon.forge.reforges.iter().position(|f| {
            f.persisted
                && (idle || f.answer.is_some() || f.completed.is_some() || f.cancellation.is_some())
        });
        let Some(index) = index else {
            return;
        };
        let mut forging = self.dungeon.forge.reforges.remove(index).unwrap();
        if !forging.launched
            && forging.answer.is_none()
            && forging.completed.is_none()
            && forging.cancellation.is_none()
        {
            let claim = forging.claim.as_ref().expect("host claim");
            let name = if claim.work.player_name == "You" && claim.work.player == 1 {
                self.host_name()
            } else {
                claim.work.player_name.clone()
            };
            let dir = self.reforged_dir();
            if let Err(error) = std::fs::create_dir_all(&dir) {
                self.dungeon.notice = format!("Reforge retained: {error}");
                self.dungeon.forge.reforges.push_front(forging);
                return;
            }
            let path = dir.join(format!("{}.card", forging.id));
            let sheets = crate::together_sheets::dir(&self.cx.workspace());
            let mut prompt = brief(&name, forging.part, &forging.words, &path, &sheets);
            if let Some(original) = &claim.work.original {
                prompt.push_str(&format!(
                    "\n\nThe kit captured when this request was accepted:\n{}",
                    original.definition_file()
                ));
            }
            forging.launched = true;
            forging.started = Instant::now();
            self.dungeon.notice = format!(
                "angelX is reforging {name}'s {}… keep playing.",
                forging.part.word()
            );
            self.dungeon.forge.forging = Some(forging);
            self.cx.make(Move::Submit { brief: prompt, keep_draft: true });
        } else {
            self.dungeon.forge.forging = Some(forging);
        }
    }
    pub(super) fn watch_host_reforge(&mut self) {
        let mut forging = self.dungeon.forge.forging.take().expect("host job");
        let busy = self.facts().busy();
        if forging.answer.is_none() && forging.completed.is_none() && forging.cancellation.is_none()
        {
            if busy {
                forging.seen = true;
                self.dungeon.forge.forging = Some(forging);
                return;
            }
            if !forging.seen && forging.started.elapsed() < Duration::from_secs(10) {
                self.dungeon.forge.forging = Some(forging);
                return;
            }
            let path = self.reforged_dir().join(format!("{}.card", forging.id));
            match self
                .read_forged(&path)
                .and_then(|raw| cards::check(&forging.id, &raw).map_err(|r| r.errors.join("; ")))
            {
                Ok(checked) => {
                    forging.notes = checked.notes.join("; ");
                    forging.answer = Some(checked.card);
                }
                Err(error) => forging.cancellation = Some(error.chars().take(2000).collect()),
            }
        }
        let claim = forging.claim.as_ref().expect("host claim");
        if forging.completed.is_none() {
            if let Some(answer) = forging.answer.clone() {
                match self.dungeon.shooter.answer_reforge(claim, answer) {
                    Ok(Ok(done)) => forging.completed = Some(done),
                    Ok(Err(errors)) => {
                        forging.answer = None;
                        forging.cancellation = Some(errors.join("; ").chars().take(2000).collect());
                    }
                    Err(error) => {
                        self.dungeon.notice = format!("Reforge answer retained: {error}");
                        self.dungeon.forge.forging = Some(forging);
                        return;
                    }
                }
            }
            if forging.completed.is_none()
                && let Some(reason) = forging.cancellation.clone()
            {
                match self.dungeon.shooter.cancel_reforge(claim, reason) {
                    Ok(done) => forging.completed = Some(done),
                    Err(error) => {
                        self.dungeon.notice = format!("Reforge cancellation retained: {error}");
                        self.dungeon.forge.forging = Some(forging);
                        return;
                    }
                }
            }
        }
        let done = forging.completed.as_ref().expect("terminal reforge");
        let source = self.reforged_dir().join(format!("{}.card", forging.id));
        let persist = (|| -> Result<(), String> {
            self.dungeon.checkpoint(Instant::now(), true)?;
            if !forging.acknowledged {
                // Painters reload this file later. Publish the accepted behavior,
                // not the model's earlier draft, before retiring its exact receipt.
                if done.result.is_ok()
                    && let Some(card) = &done.accepted
                {
                    angel_live::host::workspace_store::write_private_atomic_exact(
                        &source,
                        card.definition_file().as_bytes(),
                    )?;
                }
                self.dungeon.shooter.acknowledge_reforge(claim, done)?;
                forging.acknowledged = true;
            }
            self.dungeon.checkpoint(Instant::now(), true)
        })();
        if let Err(error) = persist {
            self.dungeon.notice = format!("Reforge delivery retained: {error}");
            self.dungeon.forge.forging = Some(forging);
            return;
        }
        let line = match &done.result {
            Ok(name) => {
                let card = done.accepted.as_ref().expect("successful card");
                let slots = painter::slots(card);
                let path = self.reforged_dir().join(format!("{}.card", forging.id));
                self.paint_card(painter::Target::Card(path), card, &forging.words, &slots);
                if forging.notes.is_empty() {
                    format!("Reforged: {name}. Back into the Sanctuary.")
                } else {
                    format!(
                        "Reforged: {name} ({}). Back into the Sanctuary.",
                        forging.notes
                    )
                }
            }
            Err(error) => format!("The reforge didn't take: {error}. T at the altar tries again."),
        };
        self.cx.say(line.clone().into());
        self.dungeon.notice = line;
        if self.dungeon.forge.reforges.is_empty() {
            self.expand_dungeon();
        }
        self.next_reforge();
    }
}
