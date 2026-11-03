//! The salvage yard's makings, on the host (`together_salvage`): a whole
//! set at home goes to the shipwrights, a design that lands is built, and a
//! vessel built is shown to the party.

use super::*;

impl Session<'_> {
    /// Each look: build what has landed, show the party what was built,
    /// and send the next whole set out. Only the host makes vessels.
    pub(crate) fn watch_salvage(&mut self) {
        if self.dungeon.joined.is_some() {
            return;
        }
        use angel_delve::rules::host_commands::{Command, Reply};
        if let Some((job, design, by_model)) = self.dungeon.forge.shipwright.landed()
            && !self.dungeon.shooter.is_none()
        {
            // A yard that moved on (the build made, a piece gone) is planned
            // again on the next look.
            let built = self.dungeon.shooter.command(Command::BuildVessel {
                n: job.plan.n,
                ids: job.ids(),
                design,
                by_model,
            });
            if let Ok(Reply::Chosen(name)) = built {
                self.dungeon.notice = format!("The shipwrights have made something of the salvage: {name}.");
            }
        }
        if self
            .dungeon
            .shooter
            .as_ref()
            .is_some_and(|run| run.at_home_now() && run.home.salvage.vessels.iter().any(|v| !v.revealed))
        {
            let _ = self.dungeon.shooter.command(Command::RevealVessel);
        }
        if self.dungeon.forge.shipwright.job.is_some() {
            return;
        }
        let Some((plan, fleet)) = self
            .dungeon
            .shooter
            .as_ref()
            .and_then(|run| run.salvage_plan().map(|plan| (plan, run.home.salvage.vessels.clone())))
        else {
            return;
        };
        let oracle = self
            .dungeon
            .forge
            .grants
            .selected(|| self.cx.oracle());
        self.dungeon.forge.shipwright.start_oracle(oracle, plan, fleet);
    }
}
