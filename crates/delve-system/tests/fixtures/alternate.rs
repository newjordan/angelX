//! A separately linked implementation: real Run::step, then a changed rule.
//! It is only built by the explicit implementation-swap regression suite.
fn main() {
    angel_delve_system::serve_with_policies(
        |run, inputs| {
            run.step(inputs);
            run.score += 7;
        },
        angel_delve_system::Policies {
            scroll: |request| {
                let mut result = request.evaluate()?;
                if request.said.contains("policy probe")
                    && let Ok(decision) = &mut result
                {
                    decision.offer.line = "Replacement scroll policy".into();
                    decision.blood_price = (decision.blood_price / 2).max(1);
                    for choice in &mut decision.offer.choices {
                        if choice.take == angel_delve::rules::genie::Take::Bolder {
                            choice.cost = format!("costs {} health", decision.blood_price);
                        }
                    }
                }
                Ok(result)
            },
            home: |request| {
                let mut result = request.evaluate()?;
                if request.transfer.marks.contains_key("native_policy_probe") {
                    result.treasury.add(angel_delve::realm::Spoil::Gold, 7);
                    result.changed = true;
                }
                Ok(result)
            },
            brew: |request| {
                request.validate()?;
                let mut card = request.card.clone();
                card.id = request.work.result_id.clone();
                card.name = "Worker Orchard".into();
                Ok(Ok(card))
            },
            wish: |request| {
                if request.work.ask.words == "reject wish policy" {
                    return Ok(Err(vec!["replacement declined this wish".into()]));
                }
                let mut result = request.evaluate()?;
                if let Ok(wish) = &mut result {
                    wish.name = "Worker Blessing".into();
                }
                Ok(result)
            },
            reforge: |request| {
                let mut result = request.evaluate()?;
                if let Ok(card) = &mut result {
                    card.name = "Worker Aegis".into();
                }
                Ok(result)
            },
            paint: |request| {
                request.validate()?;
                Ok(angel_delve::rules::card_paint::Decision {
                    accepted: request
                        .candidates
                        .iter()
                        .map(|c| c.current.id.clone())
                        .collect(),
                })
            },
        },
    )
    .unwrap();
}
