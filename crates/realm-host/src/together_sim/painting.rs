use super::*;
use serde::Deserialize;
use angel_delve::rules::{
    card_paint::{self, Candidate, Decision, Request},
    host_commands::{Command, Reply},
};

impl Sim {
    /// Only invoked for completed source changes. The pure rules request carries
    /// the affected cards, not a Run/checkpoint or another job/resource registry.
    pub fn paint_cards(&mut self, cards: Vec<cards::Card>) -> Result<bool, String> {
        let live = self.live.as_mut().ok_or("no active game session")?;
        let shared = Arc::clone(live.shared());
        let mut core = lock(&shared.core);
        let request = Request {
            candidates: cards
                .into_iter()
                .filter_map(|authored| {
                    core.run
                        .book
                        .get(&authored.id)
                        .cloned()
                        .map(|current| Candidate { current, authored })
                })
                .collect(),
        };
        if request.candidates.is_empty() {
            return Ok(false);
        }
        request.validate()?;
        let decision = if let Some(engine) = core.engine.as_mut() {
            let parsed = std::cell::RefCell::new(None);
            engine.compute(
                card_paint::CONTRACT,
                card_paint::API,
                serde_json::to_value(&request).map_err(|e| e.to_string())?,
                |value| {
                    let decision = Decision::deserialize(value).map_err(|e| e.to_string())?;
                    request.validate_result(&decision)?;
                    parsed.replace(Some(decision));
                    Ok(())
                },
            )?;
            parsed
                .into_inner()
                .ok_or("paint decision was not validated")?
        } else {
            request.evaluate()?
        };
        let updates = request.updates(&decision)?;
        if updates.is_empty() {
            return Ok(false);
        }
        let Core {
            run,
            commands,
            edited,
            ..
        } = &mut *core;
        let request = angel_delve::rules::host_commands::Request {
            session: commands.session().to_owned(),
            raid: run.raid_id,
            id: commands.next_id()?,
            commands: vec![Command::PaintCards { updates }],
        };
        let receipt = commands.apply(run, request)?;
        commands.acknowledge(run.raid_id, receipt.id)?;
        let changed = match receipt.result?.as_slice() {
            [Reply::Changed(changed)] => *changed,
            _ => return Err("paint command returned an invalid reply".into()),
        };
        *edited |= changed;
        if changed {
            live.view.take();
        }
        Ok(changed)
    }
}
