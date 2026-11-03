//! Native pending projection bound to the journal that observed its source.
use super::*;
use angel_delve::rules::{
    host_commands::{Command, Request},
    speculative::Event,
};

#[derive(Clone, Debug, PartialEq)]
pub struct Pending {
    session: String,
    event: Event,
}
impl Sim {
    pub fn capture_speculative(&self, event: Event) -> Option<Pending> {
        if event.changes.is_empty() {
            return None;
        }
        let live = self.live.as_ref()?;
        let core = lock(&live.shared().core);
        (core.run.raid_id == event.raid).then(|| Pending {
            session: core.commands.session().into(),
            event,
        })
    }
    /// Absence is retryable. A different journal retires the old projection;
    /// matching raid/seat numbers alone never authorize adoption by a new game.
    pub fn deliver_speculative(&mut self, pending: &Pending) -> Result<(), String> {
        if pending.event.changes.is_empty() {
            return Ok(());
        }
        let live = self.live.as_mut().ok_or("no active game session")?;
        let shared = Arc::clone(live.shared());
        let mut core = lock(&shared.core);
        if core.commands.session() != pending.session {
            return Ok(());
        }
        let Core {
            run,
            commands,
            edited,
            ..
        } = &mut *core;
        let request = Request {
            session: pending.session.clone(),
            raid: run.raid_id,
            id: commands.next_id()?,
            commands: vec![Command::Speculative {
                event: pending.event.clone(),
            }],
        };
        let receipt = commands.apply(run, request)?;
        commands.acknowledge(run.raid_id, receipt.id)?;
        receipt.result?;
        *edited = true;
        live.view.take();
        Ok(())
    }
}
