//! Authored presentation graphs and their active instances. Pure data shared
//! by native and browser Glass; no module execution or asset IO during paint.
use super::appearance::Animation;
use crate::rules::abilities::Origin;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

pub const MAX_ACTIVE: usize = 64;
pub const MAX_NODES: usize = 16;
pub const MAX_PIXELS: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Layer {
    Ground,
    #[default]
    Over,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Key {
    pub tick: u32,
    pub offset: [f32; 2],
    pub scale: f32,
    pub colour: [u8; 3],
    pub opacity: u8,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Shape {
    Ring {
        radius: f32,
        width: u8,
    },
    Disc {
        radius: f32,
    },
    Particles {
        count: u8,
        spread: f32,
        gravity: f32,
    },
    Sprite {
        animation: Arc<Animation>,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    #[serde(default)]
    pub layer: Layer,
    pub keys: Vec<Key>,
    pub shape: Shape,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Graph {
    pub ticks: u32,
    pub nodes: Vec<Node>,
}
impl Graph {
    pub fn pixel_count(&self) -> usize {
        self.nodes
            .iter()
            .map(|node| match &node.shape {
                Shape::Sprite { animation } => animation.frames.iter().map(Vec::len).sum(),
                _ => 0,
            })
            .sum()
    }
    pub fn validate(&self) -> Result<(), String> {
        let finite = |v: f32, max: f32| v.is_finite() && v.abs() <= max;
        if !(1..=600).contains(&self.ticks)
            || self.nodes.is_empty()
            || self.nodes.len() > MAX_NODES
            || self.pixel_count() > MAX_PIXELS
        {
            return Err("effect graph duration/node/pixel budget exceeded".into());
        }
        for node in &self.nodes {
            if node.keys.is_empty()
                || node.keys.len() > 16
                || node.keys[0].tick != 0
                || node.keys.windows(2).any(|w| w[0].tick >= w[1].tick)
                || node.keys.iter().any(|key| {
                    key.tick > self.ticks
                        || !key.offset.iter().all(|v| finite(*v, 32.0))
                        || !key.scale.is_finite()
                        || !(0.0..=4.0).contains(&key.scale)
                })
            {
                return Err("invalid effect keyframes".into());
            }
            let valid = match &node.shape {
                Shape::Ring { radius, width } => {
                    radius.is_finite() && (0.05..=8.0).contains(radius) && (1..=4).contains(width)
                }
                Shape::Disc { radius } => radius.is_finite() && (0.05..=4.0).contains(radius),
                Shape::Particles {
                    count,
                    spread,
                    gravity,
                } => {
                    (1..=64).contains(count)
                        && spread.is_finite()
                        && (0.0..=32.0).contains(spread)
                        && finite(*gravity, 16.0)
                }
                Shape::Sprite { animation } => {
                    (1..=8).contains(&animation.frames.len())
                        && (1..=300).contains(&animation.frame_ticks)
                        && animation.frames.iter().all(|frame| {
                            frame.len() == 1024 && frame.iter().any(|pixel| pixel[3] > 0)
                        })
                }
            };
            if !valid {
                return Err("invalid effect primitive".into());
            }
        }
        Ok(())
    }
}
impl Node {
    /// Linear keyframe evaluation is bounded and independent of module code.
    pub fn at(&self, tick: u32) -> Option<Key> {
        let mut previous = self.keys.first()?.clone();
        for next in self.keys.iter().skip(1).take(15) {
            if tick <= next.tick {
                let t = tick.saturating_sub(previous.tick) as f32
                    / next.tick.saturating_sub(previous.tick).max(1) as f32;
                let lerp = |a: f32, b: f32| a + (b - a) * t;
                return Some(Key {
                    tick,
                    offset: std::array::from_fn(|i| lerp(previous.offset[i], next.offset[i])),
                    scale: lerp(previous.scale, next.scale),
                    colour: std::array::from_fn(|i| {
                        lerp(f32::from(previous.colour[i]), f32::from(next.colour[i])).round() as u8
                    }),
                    opacity: lerp(f32::from(previous.opacity), f32::from(next.opacity)).round()
                        as u8,
                });
            }
            previous = next.clone();
        }
        Some(previous)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Playing {
    pub id: u64,
    pub player: u32,
    pub origin: Origin,
    pub graph: String,
    pub born: u64,
    pub position: [f32; 2],
    pub follow: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scene {
    /// Zero permits uncached/manual views. The owner changes a nonzero
    /// revision whenever its immutable catalogue or instance list changes.
    #[serde(default)]
    pub revision: u64,
    pub graphs: BTreeMap<String, Arc<Graph>>,
    pub effects: Vec<Playing>,
}
impl Scene {
    pub fn is_empty(&self) -> bool {
        self.graphs.is_empty() && self.effects.is_empty()
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.effects.len() > MAX_ACTIVE
            || self.graphs.len() > MAX_ACTIVE
            || self
                .graphs
                .values()
                .map(|graph| graph.pixel_count())
                .sum::<usize>()
                > MAX_PIXELS
        {
            return Err("effect scene capacity exceeded".into());
        }
        for (id, graph) in &self.graphs {
            if id.len() != 64
                || !id
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            {
                return Err("invalid effect graph identity".into());
            }
            graph.validate()?;
        }
        let mut ids = std::collections::BTreeSet::new();
        for playing in &self.effects {
            if playing.id == 0
                || !ids.insert(playing.id)
                || !(1..=4).contains(&playing.player)
                || !self.graphs.contains_key(&playing.graph)
                || !playing
                    .position
                    .iter()
                    .all(|v| v.is_finite() && v.abs() <= 10_000.0)
                || playing.origin.generation == 0
                || playing.origin.module.is_empty()
                || playing.origin.cast.is_empty()
                || playing.origin.module.len() > 96
                || playing.origin.cast.len() > 128
            {
                return Err("invalid effect instance/reference".into());
            }
        }
        Ok(())
    }
}
