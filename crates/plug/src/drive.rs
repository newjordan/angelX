use angel_live::kernel::{Binding, Candidate, Resources};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone, Copy)]
pub struct Landmark(char);

impl Landmark {
    pub fn glyph(self) -> char {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Place {
    #[default]
    Keep,
    Council,
    Smithy,
    Observatory,
    Library,
}

impl Place {
    pub const ALL: [Self; 5] = [
        Self::Keep,
        Self::Council,
        Self::Smithy,
        Self::Observatory,
        Self::Library,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Keep => "Keep",
            Self::Council => "Round Table",
            Self::Smithy => "Smithy",
            Self::Observatory => "Observatory",
            Self::Library => "Library",
        }
    }

    pub fn purpose(self) -> &'static str {
        match self {
            Self::Keep => "Campaign overview",
            Self::Council => "Agents & handoffs",
            Self::Smithy => "Changes & verification",
            Self::Observatory => "Research & measurements",
            Self::Library => "Findings & knowledge",
        }
    }

    pub fn building(self) -> Landmark {
        match self {
            Self::Keep => Landmark('⌂'),
            Self::Council => Landmark('◉'),
            Self::Smithy => Landmark('⚒'),
            Self::Observatory => Landmark('✦'),
            Self::Library => Landmark('▤'),
        }
    }

    pub fn step(self, delta: isize) -> Self {
        let index = Self::ALL
            .iter()
            .position(|place| *place == self)
            .unwrap_or(0);
        Self::ALL[(index as isize + delta).rem_euclid(5) as usize]
    }
}

pub type Neighbours<'a> = &'a mut dyn FnMut(usize) -> Vec<(usize, u32)>;

pub type Engine = fn(&str, &Value) -> Result<Value, String>;

pub trait ResearchMap {
    fn document(&self) -> &Value;

    fn nodes(&self) -> &[Value];

    fn frontier(&self, task: Option<&str>, limit: usize) -> Value;

    fn route(&self, from: &str, to: &str, explore: bool) -> Result<Value, String>;

    fn is_charted(&self, node: &Value) -> bool;

    fn has_negative_attempt(&self, node: &Value) -> bool;
}

pub trait Labyrinth {
    fn load(&self, root: &Path) -> Result<Option<Box<dyn ResearchMap>>, String>;
}

pub trait DriveWorld {
    type SettlementInput;

    fn settlement_input(
        _root: &Path,
        _task: &str,
        _labyrinth: &dyn Labyrinth,
    ) -> Result<Option<Self::SettlementInput>, String> {
        Ok(None)
    }

    fn settlement_contract(_site_id: &str, _loop_id: &str, _iteration: usize) -> String {
        String::new()
    }

    fn research_evidence(_advice: &Value, _active: bool) -> crate::feed::Evidence {
        crate::feed::Evidence::default()
    }

    fn tool_place(_name: &str, _args: &str) -> Place {
        Place::default()
    }

    fn shortest_path(
        _count: usize,
        _start: usize,
        _goal: usize,
        _neighbours: Neighbours<'_>,
    ) -> Option<Vec<usize>> {
        None
    }

    fn bind_resource(
        _contract: &str,
        _name: &str,
        _value: &Value,
        _assets: &BTreeMap<String, Arc<angel_live::Asset>>,
        _factory: &Candidate,
        _imports: &Resources,
    ) -> Result<Option<Binding>, String> {
        Ok(None)
    }

    fn validate_contract(_kind: &str, _value: &Value) -> Result<(), String> {
        Ok(())
    }

    fn install_scripted_rules(_engine: Engine) {}

    #[cfg(feature = "glass")]
    fn serving_knights(
        &self,
        _lead: crate::feed::AgentKey,
        _seats: &[crate::feed::AgentKey],
    ) -> Vec<(String, String)> {
        Vec::new()
    }
}
