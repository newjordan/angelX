//! Machines with ratios: what each workshop makes per minute at the rate it
//! is really running, what it holds, and how it has spent its seconds. These
//! are read-outs; the economy never reads them back.
use crate::*;

#[derive(Clone, Debug, PartialEq)]
pub struct MachineRow {
    pub id: u32,
    pub plan: String,
    pub recipe: Option<String>,
    pub status: Status,
    /// Seconds a batch takes at the crew's efficiency and the power fitted.
    pub batch_seconds: f32,
    pub inputs_per_min: Vec<(String, f32)>,
    pub outputs_per_min: Vec<(String, f32)>,
    /// What the input buffer holds against what it takes.
    pub input_fill: Vec<(String, u32, u32)>,
    /// What waits in the output buffer against its limit.
    pub output_fill: Vec<(String, u32, u32)>,
    pub stats: Stats,
}

impl MachineRow {
    /// Share of its seconds spent starved of inputs or backed up on output,
    /// in percent: a machine high on either is where to look.
    pub fn starved_percent(&self) -> u32 {
        let total = self.stats.worked + self.stats.starved + self.stats.backed_up + self.stats.unstaffed + self.stats.stalled;
        if total == 0 { 0 } else { self.stats.starved * 100 / total }
    }
    pub fn backed_up_percent(&self) -> u32 {
        let total = self.stats.worked + self.stats.starved + self.stats.backed_up + self.stats.unstaffed + self.stats.stalled;
        if total == 0 { 0 } else { self.stats.backed_up * 100 / total }
    }
}

impl Kingdom {
    pub fn machine_report(&self, c: &Catalog) -> Vec<MachineRow> {
        let report = self.report(c);
        self.buildings
            .iter()
            .filter(|(id, b)| **id != 1 && self.job(c, b).is_some() && b.construction == 0)
            .map(|(&id, b)| {
                let plan = &c.plans[&b.plan];
                let (input, output, shifts) = self.job(c, b).unwrap();
                let efficiency = u32::from(report.population.efficiency.get(&id).copied().unwrap_or(100)).max(1);
                let multiplier = u32::from(self.energy_multiplier(c, id, &report.energy));
                let multiplier = if plan.power.as_ref().is_some_and(|p| p.is_consumer()) && b.power_upgrade {
                    multiplier.max(1)
                } else {
                    100
                };
                let batch = f32::from(shifts) * 10.0 * 10_000.0 / (efficiency * multiplier) as f32;
                let rate = |stock: &Stock| stock.iter().map(|(r, &n)| (r.clone(), n as f32 * 60.0 / batch)).collect();
                MachineRow {
                    id,
                    plan: b.plan.clone(),
                    recipe: b.recipe.clone(),
                    status: report.status.get(&id).cloned().unwrap_or(Status::Idle),
                    batch_seconds: batch,
                    inputs_per_min: rate(&input),
                    outputs_per_min: rate(&output),
                    input_fill: input
                        .iter()
                        .map(|(r, &n)| (r.clone(), b.input.get(r).copied().unwrap_or(0), (n * 8).max(8).min(plan.input_cap())))
                        .collect(),
                    output_fill: output
                        .keys()
                        .map(|r| (r.clone(), b.output.get(r).copied().unwrap_or(0), plan.output_cap()))
                        .collect(),
                    stats: b.stats,
                }
            })
            .collect()
    }
}
