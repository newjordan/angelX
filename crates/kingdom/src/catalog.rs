use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::OnceLock;

pub type Stock = BTreeMap<String, u32>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resource {
    pub name: String,
    pub colour: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub name: String,
    pub inputs: Stock,
    pub outputs: Stock,
    pub shifts: u16,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub name: String,
    pub glyph: String,
    pub colour: u32,
    pub cost: Stock,
    pub workers: u16,
    pub housing: u16,
    pub storage: u32,
    pub recipes: Vec<String>,
    #[serde(default)]
    pub extractor: bool,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub build_seconds: u16,
    #[serde(default)]
    pub carriers: u16,
    #[serde(default)]
    pub siting: Siting,
    #[serde(default)]
    pub passable: bool,
    #[serde(default)]
    pub power: Option<crate::power::MachinePower>,
    #[serde(default)]
    pub connector: Option<crate::PowerKind>,
}

/// Landscape requirements are content, rather than renderer-specific rules.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Siting {
    #[default]
    Any,
    Clay,
    Stone,
    Woodland,
    Riverbank,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Technology {
    pub name: String,
    pub description: String,
    pub cost: Stock,
    pub requires: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub schema: u32,
    pub id: String,
    pub resources: BTreeMap<String, Resource>,
    pub recipes: BTreeMap<String, Recipe>,
    pub plans: BTreeMap<String, Plan>,
    pub menu: Vec<String>,
    pub starter: Stock,
    #[serde(default)]
    pub technologies: BTreeMap<String, Technology>,
}

fn id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 48
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}
impl Catalog {
    pub fn parse(json: &str) -> Result<Self, String> {
        let catalog: Self = serde_json::from_str(json).map_err(|e| e.to_string())?;
        catalog.validate()?;
        Ok(catalog)
    }
    pub fn valid_stock(&self, stock: &Stock) -> bool {
        stock.len() <= self.resources.len()
            && stock
                .iter()
                .all(|(r, &n)| self.resources.contains_key(r) && n > 0 && n <= 1_000_000)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != 1
            || !id(&self.id)
            || self.resources.is_empty()
            || self.resources.len() > 64
            || self.recipes.len() > 128
            || self.plans.len() > 64
            || self.technologies.len() > 64
            || self.menu.is_empty()
            || self.menu.len() > 64
            || !self.valid_stock(&self.starter)
            || !self.plans.contains_key("camp")
            || !self.resources.contains_key("timber")
        {
            return Err("invalid kingdom catalogue bounds or foundation".into());
        }
        for (key, resource) in &self.resources {
            if !id(key)
                || resource.name.is_empty()
                || resource.name.len() > 40
                || resource.colour > 0xffffff
            {
                return Err(format!("invalid resource {key}"));
            }
        }
        for (key, recipe) in &self.recipes {
            if !id(key)
                || recipe.name.len() > 40
                || recipe.shifts == 0
                || recipe.shifts > 100
                || !self.valid_stock(&recipe.inputs)
                || !self.valid_stock(&recipe.outputs)
                || recipe.outputs.is_empty()
                || recipe
                    .inputs
                    .values()
                    .chain(recipe.outputs.values())
                    .any(|n| *n > 24)
            {
                return Err(format!("invalid recipe {key}"));
            }
        }
        for (key, plan) in &self.plans {
            if !id(key)
                || plan.name.len() > 40
                || plan.glyph.len() != 1
                || !plan.glyph.is_ascii()
                || plan.colour > 0xffffff
                || !self.valid_stock(&plan.cost)
                || plan.workers > 20
                || plan.housing > 40
                || plan.storage > 10_000
                || plan.recipes.len() > 16
                || plan.recipes.iter().any(|r| !self.recipes.contains_key(r))
                || (plan.extractor && !plan.recipes.is_empty())
                || plan.build_seconds > 600
                || plan.carriers > 20
                || plan.power.as_ref().is_some_and(|p| !p.valid(self))
                || plan
                    .power
                    .as_ref()
                    .is_some_and(|p| p.is_source() && (!plan.recipes.is_empty() || plan.extractor))
                || plan.connector.is_some_and(|kind| {
                    plan.power.is_some()
                        || plan.extractor
                        || !plan.recipes.is_empty()
                        || plan.workers > 0
                        || plan.housing > 0
                        || plan.storage > 0
                        || plan.carriers > 0
                        || plan.build_seconds > 0
                        || !plan.passable
                        || self
                            .plans
                            .values()
                            .filter(|p| p.connector == Some(kind))
                            .count()
                            != 1
                })
                || plan
                    .requires
                    .iter()
                    .any(|r| !self.technologies.contains_key(r))
            {
                return Err(format!("invalid building {key}"));
            }
        }
        for (key, tech) in &self.technologies {
            if !id(key)
                || tech.name.len() > 40
                || tech.description.len() > 160
                || !self.valid_stock(&tech.cost)
                || tech
                    .requires
                    .iter()
                    .any(|r| r == key || !self.technologies.contains_key(r))
            {
                return Err(format!("invalid technology {key}"));
            }
        }
        let mut resolved = std::collections::BTreeSet::new();
        loop {
            let before = resolved.len();
            for (key, tech) in &self.technologies {
                if tech.requires.iter().all(|r| resolved.contains(r)) {
                    resolved.insert(key.clone());
                }
            }
            if resolved.len() == before {
                break;
            }
        }
        if resolved.len() != self.technologies.len() {
            return Err("cyclic technology prerequisites".into());
        }
        let unique: std::collections::BTreeSet<_> = self.menu.iter().collect();
        if unique.len() != self.menu.len()
            || self
                .menu
                .iter()
                .any(|p| p == "camp" || !self.plans.contains_key(p))
        {
            return Err("invalid build menu".into());
        }
        Ok(())
    }
}

pub fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        Catalog::parse(include_str!("../../../delve/kingdom/frontier.json"))
            .expect("checked frontier catalogue")
    })
}
