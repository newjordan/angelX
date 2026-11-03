//! Typed existing Journal operations; native model/file lifetimes stay native.
use super::{Policies, host_commands};
use crate::rules::{
    Run, brew_jobs as brew, cards, cauldron, genie, home_settlement as home, knights, phrasebook,
    reforge_jobs as reforge, wish_jobs as wish,
};
use host_commands::{Journal, RealmClaim, playtest::VoteBatch};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum Event {
    Acknowledge {
        raid: u64,
        request: u64,
    },
    ClaimRealm,
    AcknowledgeRealm(RealmClaim),
    ClaimHome {
        context: Option<home::HomeContext>,
    },
    EvaluateHome(home::HomePolicyRequest),
    CompleteHome {
        claim: home::HomeClaim,
        result: home::HomeResult,
    },
    AcknowledgeHome {
        claim: home::HomeClaim,
        completed: home::HomeCompletion,
    },
    ClaimBrew,
    CompleteBrew {
        claim: brew::Claim,
        input: cards::Card,
    },
    CancelBrew {
        claim: brew::Claim,
        reason: String,
    },
    AcknowledgeBrew {
        claim: brew::Claim,
        completed: brew::Completion,
    },
    ClaimReforge {
        player: u32,
        part: knights::Part,
        words: String,
    },
    CompleteReforge {
        claim: reforge::Claim,
        input: cards::Card,
    },
    CancelReforge {
        claim: reforge::Claim,
        reason: String,
    },
    AcknowledgeReforge {
        claim: reforge::Claim,
        completed: reforge::Completion,
    },
    ClaimWish {
        ask: genie::Asked,
        maker: String,
    },
    CompleteWish {
        claim: wish::Claim,
        input: Option<phrasebook::Wish>,
        authored: bool,
        reason: Option<String>,
    },
    AcknowledgeWish {
        claim: wish::Claim,
        completed: wish::Completion,
    },
    PrepareVotes {
        records: crate::rules::votes::Records,
        seen: std::collections::BTreeSet<String>,
        couch: bool,
    },
    ClaimVotes,
    AcknowledgeVotes(VoteBatch),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum Query {
    PendingRealm,
    PendingHome,
    HomeCompletion(home::HomeClaim),
    PendingBrews,
    BrewCompletion(brew::Claim),
    PendingReforges,
    ReforgeCompletion(reforge::Claim),
    PendingWishes,
    WishCompletion(wish::Claim),
    PendingVotes,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum Reply {
    Done,
    Realm(Option<RealmClaim>),
    Home(Option<home::HomeClaim>),
    HomeResult(home::HomeResult),
    HomeCompleted(Option<home::HomeCompletion>),
    HomeDelivered(home::HomeDelivered),
    Brew(Option<brew::Claim>),
    Brews(Vec<brew::Claim>),
    BrewCompleted(Option<brew::Completion>),
    Reforge(reforge::Claim),
    Reforges(Vec<reforge::Claim>),
    ReforgeCompleted(Option<reforge::Completion>),
    Wish(wish::Claim),
    Wishes(Vec<wish::Claim>),
    WishCompleted(Option<wish::Completion>),
    Votes(Option<VoteBatch>),
    /// Existing model mend response; no completion has been committed.
    Rejected(Vec<String>),
}

pub(super) fn query(journal: &Journal, query: Query) -> Result<Reply, String> {
    Ok(match query {
        Query::PendingRealm => Reply::Realm(journal.pending_realm()?),
        Query::PendingHome => Reply::Home(journal.pending_home()?),
        Query::HomeCompletion(claim) => Reply::HomeCompleted(journal.home_completion(&claim)?),
        Query::PendingBrews => Reply::Brews(journal.pending_brews()?),
        Query::BrewCompletion(claim) => Reply::BrewCompleted(journal.brew_completion(&claim)?),
        Query::PendingReforges => Reply::Reforges(journal.pending_reforges()?),
        Query::ReforgeCompletion(claim) => {
            Reply::ReforgeCompleted(journal.reforge_completion(&claim)?)
        }
        Query::PendingWishes => Reply::Wishes(journal.pending_wishes()?),
        Query::WishCompletion(claim) => Reply::WishCompleted(journal.wish_completion(&claim)?),
        Query::PendingVotes => Reply::Votes(journal.pending_votes()?),
    })
}
pub(super) fn apply(
    run: &mut Run,
    journal: &mut Journal,
    event: Event,
    policies: &mut impl Policies,
) -> Result<Reply, String> {
    Ok(match event {
        Event::Acknowledge { raid, request } => {
            journal.acknowledge(raid, request)?;
            Reply::Done
        }
        Event::ClaimRealm => Reply::Realm(journal.claim_realm(run)?),
        Event::AcknowledgeRealm(claim) => {
            journal.acknowledge_realm(&claim)?;
            Reply::Done
        }
        Event::ClaimHome { context } => Reply::Home(match context {
            Some(context) => journal.claim_home_with_context(run, &context)?,
            None => journal.claim_home(run)?,
        }),
        Event::EvaluateHome(request) => {
            request.validate()?;
            let result = policies.home(&request)?;
            request.validate_result(&result)?;
            Reply::HomeResult(result)
        }
        Event::CompleteHome { claim, result } => {
            Reply::HomeDelivered(journal.complete_home(run, &claim, result)?)
        }
        Event::AcknowledgeHome { claim, completed } => {
            journal.acknowledge_home(&claim, &completed)?;
            Reply::Done
        }
        Event::ClaimBrew => Reply::Brew(journal.claim_brew(run)?),
        Event::CompleteBrew { claim, input } => {
            if let Some(done) = journal.brew_completion(&claim)? {
                if done.input.as_ref() != Some(&input) {
                    return Err("brew answer differs from committed result".into());
                }
                Reply::BrewCompleted(Some(done))
            } else {
                let request = brew::PolicyRequest {
                    work: claim.work.clone(),
                    card: input.clone(),
                    taken: cauldron::taken(&run.book),
                };
                request.validate()?;
                let result = policies.brew(&request)?;
                request.validate_result(&result)?;
                match result {
                    Ok(accepted) => Reply::BrewCompleted(Some(
                        journal.complete_brew(run, &claim, input, accepted)?,
                    )),
                    Err(errors) => Reply::Rejected(errors),
                }
            }
        }
        Event::CancelBrew { claim, reason } => {
            Reply::BrewCompleted(Some(journal.cancel_brew(run, &claim, reason)?))
        }
        Event::AcknowledgeBrew { claim, completed } => {
            journal.acknowledge_brew(&claim, &completed)?;
            Reply::Done
        }
        Event::ClaimReforge {
            player,
            part,
            words,
        } => Reply::Reforge(journal.claim_reforge(run, player, part, words)?),
        Event::CompleteReforge { claim, input } => {
            if let Some(done) = journal.reforge_completion(&claim)? {
                if done.input.as_ref() != Some(&input) {
                    return Err("reforge answer differs from committed result".into());
                }
                Reply::ReforgeCompleted(Some(done))
            } else {
                let request = reforge::PolicyRequest {
                    work: claim.work.clone(),
                    card: input.clone(),
                };
                request.validate()?;
                let result = policies.reforge(&request)?;
                request.validate_result(&result)?;
                match result {
                    Ok(accepted) => Reply::ReforgeCompleted(Some(
                        journal.complete_reforge(run, &claim, input, accepted)?,
                    )),
                    Err(errors) => Reply::Rejected(errors),
                }
            }
        }
        Event::CancelReforge { claim, reason } => {
            Reply::ReforgeCompleted(Some(journal.cancel_reforge(run, &claim, reason)?))
        }
        Event::AcknowledgeReforge { claim, completed } => {
            journal.acknowledge_reforge(&claim, &completed)?;
            Reply::Done
        }
        Event::ClaimWish { ask, maker } => Reply::Wish(journal.claim_wish(run, ask, maker)?),
        Event::CompleteWish {
            claim,
            input,
            authored,
            reason,
        } => {
            if let Some(done) = journal.wish_completion(&claim)? {
                if done.input != input
                    || done.authored != authored
                    || (input.is_none() && done.reason != reason)
                {
                    return Err("wish answer differs from committed result".into());
                }
                Reply::WishCompleted(Some(done))
            } else {
                let mut rejection = reason;
                let accepted = if let Some(input) = &input {
                    let request = wish::PolicyRequest {
                        work: claim.work.clone(),
                        wish: input.clone(),
                    };
                    request.validate()?;
                    let result = policies.wish(&request)?;
                    request.validate_result(&result)?;
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
                Reply::WishCompleted(Some(
                    journal.complete_wish(run, &claim, input, accepted, authored, rejection)?,
                ))
            }
        }
        Event::AcknowledgeWish { claim, completed } => {
            journal.acknowledge_wish(&claim, &completed)?;
            Reply::Done
        }
        Event::PrepareVotes {
            records,
            seen,
            couch,
        } => {
            journal.prepare_votes(run, records, seen, couch)?;
            Reply::Done
        }
        Event::ClaimVotes => Reply::Votes(journal.claim_votes(run)?),
        Event::AcknowledgeVotes(batch) => {
            journal.acknowledge_votes(run, &batch)?;
            Reply::Done
        }
    })
}
