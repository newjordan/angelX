//! Captured speculative-work metadata. Native runners retain their jobs; the
//! journal admits only bounded, fenced changes to their game projection.
use super::{Run, altar, dead_wishes, phrasebook};
use serde::{Deserialize, Serialize};

type Ladder = (String, Vec<String>);
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub raid: u64,
    pub changes: Vec<Change>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Change {
    Lamp {
        player: u32,
        epoch: u64,
        words: String,
        observed: Option<altar::Pending>,
        offer: Option<(u64, String, bool)>,
        sent: bool,
    },
    Maker {
        player: u32,
        epoch: u64,
        wish: String,
        observed: Option<(u32, String)>,
        name: String,
    },
    Ladder {
        wish: String,
        observed: Option<Ladder>,
        value: Ladder,
        holders: Vec<(u32, u64, Vec<String>)>,
    },
}
impl Event {
    pub fn new(run: &Run) -> Self {
        Self {
            raid: run.raid_id,
            changes: Vec::new(),
        }
    }
    pub fn lamp(&mut self, run: &Run, player: u32, epoch: u64, words: &str, sent: bool) {
        self.changes.push(Change::Lamp {
            player,
            epoch,
            words: words.into(),
            offer: run
                .players
                .get(&player)
                .and_then(|h| h.offer.as_ref())
                .map(|o| {
                    (
                        o.opened,
                        o.said.clone(),
                        o.target.unwrap_or_default().theirs(),
                    )
                }),
            observed: run
                .lamp
                .wishing
                .iter()
                .find(|p| p.knight == player && p.words == words)
                .cloned(),
            sent,
        });
    }
    pub fn maker(&mut self, run: &Run, player: u32, epoch: u64, wish: &str, name: &str) {
        let name = if name.trim().is_empty() {
            run.players
                .get(&player)
                .map(|h| h.name.clone())
                .unwrap_or_default()
        } else {
            name.trim().into()
        };
        if !name.is_empty() {
            self.changes.push(Change::Maker {
                player,
                epoch,
                wish: wish.into(),
                observed: run.makers.get(wish).cloned(),
                name,
            });
        }
    }
    pub fn ladders(run: &Run, book: &[phrasebook::Wish]) -> Self {
        let mut event = Self::new(run);
        for wish in book.iter().filter(|w| w.learned) {
            let holders: Vec<_> = run
                .players
                .iter()
                .filter(|(_, h)| {
                    h.deck
                        .iter()
                        .any(|c| dead_wishes::wish_of(c) == Some(wish.id.as_str()))
                })
                .map(|(&id, h)| (id, run.seat_epoch(id), h.deck.clone()))
                .collect();
            let value = (
                wish.name.clone(),
                wish.tiers.iter().map(|(_, says)| says.clone()).collect(),
            );
            if !holders.is_empty() && run.ladders.get(&wish.id) != Some(&value) {
                event.changes.push(Change::Ladder {
                    wish: wish.id.clone(),
                    observed: run.ladders.get(&wish.id).cloned(),
                    value,
                    holders,
                });
            }
        }
        event
    }
    pub fn valid(&self) -> bool {
        self.changes.len() <= 256
            && serde_json::to_vec(self).is_ok_and(|v| v.len() <= 512 * 1024)
            && self.changes.iter().all(|change| match change {
                Change::Lamp { player, words, .. } => {
                    (1..=super::MAX_PLAYERS).contains(player)
                        && !words.is_empty()
                        && words.len() <= 4096
                }
                Change::Maker {
                    player, wish, name, ..
                } => {
                    (1..=super::MAX_PLAYERS).contains(player)
                        && !wish.is_empty()
                        && wish.len() <= 128
                        && name.len() <= 1024
                }
                Change::Ladder {
                    wish,
                    value,
                    holders,
                    ..
                } => {
                    !wish.is_empty()
                        && wish.len() <= 128
                        && value.1.len() <= 16
                        && !holders.is_empty()
                        && holders.len() <= super::MAX_PLAYERS as usize
                        && holders.iter().all(|(id, _, deck)| {
                            (1..=super::MAX_PLAYERS).contains(id)
                                && deck
                                    .iter()
                                    .any(|c| dead_wishes::wish_of(c) == Some(wish.as_str()))
                        })
                }
            })
    }
    pub fn deliver(&self, run: &mut Run) -> Result<bool, String> {
        if !self.valid() {
            return Err("invalid speculative metadata event".into());
        }
        if run.raid_id != self.raid {
            return Ok(false);
        }
        // Check the complete batch before writing any metadata. A stale event
        // retires harmlessly; it cannot alter a replacement seat or source.
        for change in &self.changes {
            let current = match change {
                Change::Lamp {
                    player,
                    epoch,
                    words,
                    observed,
                    offer,
                    ..
                } => {
                    run.players.contains_key(player)
                        && run.seat_epoch(*player) == *epoch
                        && run
                            .players
                            .get(player)
                            .and_then(|h| h.offer.as_ref())
                            .map(|o| {
                                (
                                    o.opened,
                                    o.said.clone(),
                                    o.target.unwrap_or_default().theirs(),
                                )
                            })
                            == *offer
                        && run
                            .lamp
                            .wishing
                            .iter()
                            .find(|p| p.knight == *player && p.words == *words)
                            == observed.as_ref()
                }
                Change::Maker {
                    player,
                    epoch,
                    wish,
                    observed,
                    ..
                } => {
                    run.players.contains_key(player)
                        && run.seat_epoch(*player) == *epoch
                        && run.makers.get(wish) == observed.as_ref()
                }
                Change::Ladder {
                    wish,
                    observed,
                    holders,
                    ..
                } => {
                    run.ladders.get(wish) == observed.as_ref()
                        && run
                            .players
                            .iter()
                            .filter(|(_, h)| {
                                h.deck
                                    .iter()
                                    .any(|c| dead_wishes::wish_of(c) == Some(wish.as_str()))
                            })
                            .map(|(&id, _)| id)
                            .eq(holders.iter().map(|(id, _, _)| *id))
                        && holders.iter().all(|(id, epoch, deck)| {
                            run.seat_epoch(*id) == *epoch
                                && run.players.get(id).is_some_and(|h| h.deck == *deck)
                        })
                }
            };
            if !current {
                return Ok(false);
            }
        }
        for change in &self.changes {
            match change {
                Change::Lamp {
                    player,
                    words,
                    sent,
                    ..
                } => {
                    if *sent {
                        run.lamp_sent(*player, words);
                    } else {
                        run.lamp_dropped(*player, words);
                    }
                }
                Change::Maker {
                    player, wish, name, ..
                } => {
                    run.makers.insert(wish.clone(), (*player, name.clone()));
                }
                Change::Ladder { wish, value, .. } => {
                    run.ladders.insert(wish.clone(), value.clone());
                }
            }
        }
        Ok(!self.changes.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::host_commands::{Command, Journal, Reply, Request};
    fn game() -> Run {
        Run::new(0, 9, Some("Guest"))
    }
    fn request(event: Event, id: u64) -> Request {
        Request {
            session: "speculative".into(),
            raid: 9,
            id,
            commands: vec![Command::Speculative { event }],
        }
    }
    #[test]
    fn journal_replay_keeps_exact_metadata_and_issued_effects() {
        let mut run = game();
        run.players.get_mut(&1).unwrap().spell_cooldowns[0] = 71;
        run.cues.push("issued".into());
        let issued = run.cues.clone();
        let mut event = Event::new(&run);
        event.lamp(&run, 1, run.seat_epoch(1), "original words", true);
        event.maker(&run, 1, run.seat_epoch(1), "captured", "Captured maker");
        let request = request(event, 1);
        let mut journal = Journal::new("speculative".into(), 9);
        let receipt = journal.apply(&mut run, request.clone()).unwrap();
        assert_eq!(receipt.result, Ok(vec![Reply::Changed(true)]));
        run.tick += 10;
        let mut journal = Journal::restore(journal.capture().unwrap(), "speculative", 9).unwrap();
        assert_eq!(journal.apply(&mut run, request).unwrap(), receipt);
        assert_eq!(run.lamp.wishing.len(), 1);
        assert_eq!(run.lamp.wishing[0].words, "original words");
        assert_eq!(run.makers["captured"].1, "Captured maker");
        assert_eq!(run.players[&1].spell_cooldowns[0], 71);
        assert_eq!(run.cues, issued);
    }
    #[test]
    fn stale_seat_raid_or_source_rejects_whole_batch() {
        for stale in 0..3 {
            let mut run = game();
            let mut event = Event::new(&run);
            event.lamp(&run, 1, run.seat_epoch(1), "words", true);
            event.maker(&run, 1, run.seat_epoch(1), "captured", "Old");
            match stale {
                0 => event.raid += 1,
                1 => {
                    if let Change::Lamp { epoch, .. } = &mut event.changes[0] {
                        *epoch += 1;
                    }
                }
                _ => {
                    run.makers.insert("captured".into(), (1, "New".into()));
                }
            }
            assert!(!event.deliver(&mut run).unwrap());
            assert!(run.lamp.wishing.is_empty());
            assert_ne!(
                run.makers.get("captured").map(|m| m.1.as_str()),
                Some("Old")
            );
        }
    }
    #[test]
    fn ladder_capture_uses_exact_wish_and_never_reloads_input() {
        let mut run = game();
        run.players.get_mut(&1).unwrap().deck = vec!["wish-grace-more-1".into()];
        let mut book = phrasebook::read(
            "wish grace | Grace\nsay grace\ntier damage 10 :: Original\n\nwish grace-more | Grace More\nsay grace more\ntier damage 20 :: More\n",
        );
        for wish in &mut book {
            wish.learned = true;
        }
        let event = Event::ladders(&run, &book);
        assert_eq!(event.changes.len(), 1);
        book[1].name = "Edited source".into();
        assert!(event.deliver(&mut run).unwrap());
        assert!(!run.ladders.contains_key("grace"));
        assert_eq!(run.ladders["grace-more"].0, "Grace More");
        let mut stale = Event::ladders(&run, &book);
        assert_eq!(stale.changes.len(), 1);
        run.players.get_mut(&1).unwrap().deck.clear();
        assert!(!stale.deliver(&mut run).unwrap());
        stale.changes.clear();
    }
    #[test]
    fn bounded_admission_retries_original_event_after_receipt_cleanup() {
        let mut run = game();
        let mut journal = Journal::new("speculative".into(), 9);
        for id in 1..=64 {
            let empty = Event::new(&run);
            journal.apply(&mut run, request(empty, id)).unwrap();
        }
        let mut event = Event::new(&run);
        event.lamp(&run, 1, run.seat_epoch(1), "pending answer", true);
        let pending = request(event, 65);
        assert!(journal.apply(&mut run, pending.clone()).is_err());
        assert!(run.lamp.wishing.is_empty());
        journal.acknowledge(9, 1).unwrap();
        journal.apply(&mut run, pending.clone()).unwrap();
        journal.apply(&mut run, pending).unwrap();
        assert_eq!(run.lamp.wishing.len(), 1);
        assert_eq!(run.lamp.wishing[0].words, "pending answer");
        journal.acknowledge(9, 65).unwrap();
    }
}
