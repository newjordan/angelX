//! Grant runners are native resources. The game owns identified intentions,
//! captured definitions and once-only grant/refund delivery.
use super::*;
use angel_delve::rules::wish_jobs::Outcome;

impl Session<'_> {
    /// Admission refusal is visible on the same pick, after durable capture.
    /// A failed checkpoint retains the identified lesson for the normal retry.
    pub(super) fn retire_cancelled_wishes(&mut self) {
        let mut still = Vec::new();
        for mut lesson in std::mem::take(&mut self.dungeon.forge.learning) {
            let cancelled = !lesson.detached && lesson.pick == Pick::Chosen
                && lesson.claim.is_some()
                && lesson.persisted
                && lesson.returned
                && lesson.failed.is_some()
                && lesson.completed.is_none();
            if !cancelled || !self.deliver_wish_lesson(&mut lesson) {
                still.push(lesson);
            }
        }
        self.dungeon.forge.learning = still;
    }

    pub(super) fn publish_wish(
        &self,
        wish: &crate::together_shooter::phrasebook::Wish,
    ) -> Result<(), String> {
        Self::publish_wish_at(&self.cx.workspace(), wish, true)
    }

    pub(super) fn publish_wish_at(
        workspace: &std::path::Path,
        wish: &crate::together_shooter::phrasebook::Wish,
        replace: bool,
    ) -> Result<(), String> {
        let path = workspace.join(".angel/dungeon/phrasebook.txt");
        let _owner = angel_live::host::workspace_store::try_private_lock(&path)?;
        let old = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e.to_string()),
        };
        if !replace {
            let entries = crate::together_shooter::phrasebook::read(&old);
            let existing_id = old.lines().filter_map(|line| line.trim_start().strip_prefix("wish ")).any(|head| head.split('|').next().is_some_and(|id| id.trim() == wish.id));
            if existing_id {
                if entries.iter().find(|entry| entry.id == wish.id).is_some_and(|existing| existing.definition_file() == wish.definition_file()) {
                    return Ok(());
                }
                return Err(format!("{}: original library source changed before publication", wish.id));
            }
            if entries.iter().any(|entry| entry.name.trim().eq_ignore_ascii_case(wish.name.trim())) {
                return Err(format!("{}: original library name changed before publication", wish.name));
            }
        }
        // Replace only this ID; other concurrent runner results and operator
        // entries survive. Publication is serialized on the native owner.
        let mut text = String::new();
        let mut omit = false;
        for line in old.lines() {
            if let Some(head) = line.trim_start().strip_prefix("wish ") {
                omit = head
                    .split('|')
                    .next()
                    .is_some_and(|id| id.trim() == wish.id);
            }
            if !omit {
                text.push_str(line);
                text.push('\n');
            }
        }
        text.push('\n');
        text.push_str(&wish.definition_file());
        angel_live::host::workspace_store::write_private_atomic_exact(&path, text.as_bytes())
    }

    pub fn deliver_wish_lesson(&mut self, lesson: &mut Lesson) -> bool {
        let Some(claim) = &lesson.claim else {
            return true;
        };
        if !self.dungeon.shooter.owns_wish(claim) {
            return true;
        }
        if lesson.completed.is_none() {
            let over = lesson.returned || lesson.asked.elapsed() > LEARN_TIME;
            if lesson.answer.is_none() && lesson.failed.is_none() && over {
                let deck = self.dungeon.wish_deck(lesson.player);
                // Compatibility: externally authored entries may satisfy a
                // waiting request. Capture the selected definition once.
                let book = crate::together_shooter::phrasebook::book();
                let learned = lesson
                    .learned_id
                    .as_ref()
                    .and_then(|id| book.iter().find(|w| &w.id == id))
                    .cloned()
                    .or_else(|| {
                        lesson.before.as_ref().filter(|_| over).and_then(|before| {
                            let added: Vec<_> = book
                                .iter()
                                .filter(|w| !before.contains(&w.id))
                                .cloned()
                                .collect();
                            crate::together_shooter::phrasebook::find_in(
                                &added,
                                &lesson.words,
                                &deck,
                            )
                        })
                    });
                lesson.authored = learned.is_some();
                lesson.answer = learned.or_else(|| {
                    crate::together_shooter::phrasebook::find_for(&lesson.words, &deck)
                });
            }
            if lesson.answer.is_none() && lesson.failed.is_none() && !over {
                return false;
            }
            let reason = if lesson.answer.is_none() {
                Some(lesson.failed.clone().unwrap_or_else(|| {
                    if lesson.returned {
                        "it could not be granted".into()
                    } else {
                        "no answer in time".into()
                    }
                }))
            } else {
                None
            };
            match self.dungeon.shooter.answer_wish(
                claim,
                lesson.answer.clone(),
                lesson.authored,
                reason,
            ) {
                Ok(done) => lesson.completed = Some(done),
                Err(error) => {
                    self.dungeon.notice = format!("Wish answer retained: {error}");
                    return false;
                }
            }
        }
        let done = lesson.completed.as_ref().expect("terminal wish");
        let persisted = (|| -> Result<(), String> {
            if self.dungeon.save_path.is_none() {
                return Err("wish checkpoint path is unavailable".into());
            }
            self.dungeon.checkpoint(Instant::now(), true)?;
            if !lesson.acknowledged {
                if done.authored
                    && !matches!(done.outcome, Outcome::Conflict(_))
                    && let Some(wish) = &done.accepted
                {
                    self.publish_wish(wish)?;
                }
                self.dungeon.shooter.acknowledge_wish(claim, done)?;
                lesson.acknowledged = true;
            }
            self.dungeon.checkpoint(Instant::now(), true)
        })();
        if let Err(error) = persisted {
            self.dungeon.notice = format!("Wish delivery retained: {error}");
            return false;
        }
        let (name, failure, line) = match &done.outcome {
            Outcome::Granted(name) => (
                Some(name.clone()),
                None,
                format!("The genie granted “{}”: {name}.", lesson.words),
            ),
            Outcome::Plain(name) => (
                Some(name.clone()),
                None,
                format!(
                    "The genie grants his plain answer to “{}”: {name}.",
                    lesson.words
                ),
            ),
            Outcome::Refunded(why) | Outcome::Conflict(why) => (
                None,
                Some(why.clone()),
                format!(
                    "The genie couldn't grant “{}” ({why}). Your wish is given back.",
                    lesson.words
                ),
            ),
            Outcome::Stale(why) => (
                None,
                Some(why.clone()),
                format!("The old wish “{}” ended: {why}.", lesson.words),
            ),
        };
        if self.dungeon.forge.landings.len() >= KEPT_LANDINGS {
            self.dungeon.forge.landings.remove(0);
        }
        self.dungeon.forge.landings.push(Landing {
            player: lesson.player,
            words: lesson.words.clone(),
            took: lesson.asked.elapsed(),
            timing: lesson.timing,
            granted: name,
            failed: failure,
        });
        self.dungeon.notice = line;
        if done.authored
            && let (Outcome::Granted(_), Some(wish)) = (&done.outcome, &done.accepted)
        {
            self.paint_wish(wish, &lesson.words);
        }
        true
    }
}
