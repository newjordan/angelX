use crate::loop_summary::{EscalationTier, LoopStatus};
use crate::world_viz::Building;
use angel_plug::commands::Building as PlugBuilding;
use angel_plug::feed::{LoopPhase, LoopTier};

impl From<Building> for PlugBuilding {
    fn from(building: Building) -> Self {
        match building {
            Building::Keep => Self::Keep,
            Building::Gatehouse => Self::Gatehouse,
            Building::Rookery => Self::Rookery,
            Building::Scriptorium => Self::Scriptorium,
            Building::Smithy => Self::Smithy,
            Building::Chapel => Self::Chapel,
            Building::RoundTable => Self::RoundTable,
            Building::Observatory => Self::Observatory,
        }
    }
}

impl From<PlugBuilding> for Building {
    fn from(building: PlugBuilding) -> Self {
        match building {
            PlugBuilding::Keep => Self::Keep,
            PlugBuilding::Gatehouse => Self::Gatehouse,
            PlugBuilding::Rookery => Self::Rookery,
            PlugBuilding::Scriptorium => Self::Scriptorium,
            PlugBuilding::Smithy => Self::Smithy,
            PlugBuilding::Chapel => Self::Chapel,
            PlugBuilding::RoundTable => Self::RoundTable,
            PlugBuilding::Observatory => Self::Observatory,
        }
    }
}

impl From<LoopPhase> for LoopStatus {
    fn from(phase: LoopPhase) -> Self {
        match phase {
            LoopPhase::Idle => Self::Idle,
            LoopPhase::Baselining => Self::Baselining,
            LoopPhase::Running => Self::Running,
            LoopPhase::Verifying => Self::Verifying,
            LoopPhase::AwaitingApproval => Self::AwaitingApproval,
            LoopPhase::Paused => Self::Paused,
            LoopPhase::Done => Self::Done,
            LoopPhase::Stopped => Self::Stopped,
            LoopPhase::Failed => Self::Failed,
        }
    }
}

impl From<LoopTier> for EscalationTier {
    fn from(tier: LoopTier) -> Self {
        match tier {
            LoopTier::Local => Self::Local,
            LoopTier::Swarm => Self::Swarm,
            LoopTier::Sota => Self::Sota,
        }
    }
}
