use super::Shelf;
use angel_plug::drive::DriveWorld;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SettlementInput {}

impl SettlementInput {
    pub(crate) fn valid(&self) -> bool {
        false
    }
}

impl DriveWorld for Shelf {
    type SettlementInput = SettlementInput;
}
