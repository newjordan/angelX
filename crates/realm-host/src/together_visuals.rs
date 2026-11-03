//! Prepared authored VFX, staged emissions and generation-owned lifetimes.
use angel_delve::realm::{
    appearance::Animation,
    effects::{self, Graph, Key, Layer, Node, Playing, Scene, Shape},
};
use angel_delve::rules::{Run, abilities::Origin};
use angel_live::{Asset, kernel::Resource};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Weak};
use super::together_frames::Frame;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Specification {
    ticks: u32,
    nodes: Vec<NodeSpec>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NodeSpec {
    #[serde(default)]
    layer: Layer,
    keys: Vec<Key>,
    shape: Primitive,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
enum Primitive {
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
        frames: Vec<Frame>,
        frame_ticks: u32,
    },
}
#[derive(Debug)]
pub struct Prepared {
    pub id: String,
    pub graph: Arc<Graph>,
    _assets: Vec<Arc<Asset>>,
}
pub type Bank = BTreeMap<String, Arc<Prepared>>;
thread_local! {static GRAPHS:std::cell::RefCell<BTreeMap<String,Weak<Prepared>>>=const{std::cell::RefCell::new(BTreeMap::new())};}
pub fn prepare(
    specs: BTreeMap<String, Specification>,
    assets: &BTreeMap<String, Arc<Asset>>,
) -> Result<Bank, String> {
    if specs.len() > 16 {
        return Err("effect bank exceeds sixteen cues".into());
    }
    let mut bank = Bank::new();
    for (name, spec) in specs {
        if name.is_empty()
            || name.len() > 64
            || !name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
            || spec.nodes.len() > effects::MAX_NODES
        {
            return Err("invalid effect cue/node budget".into());
        }
        let mut held = Vec::new();
        let mut graph = Graph {
            ticks: spec.ticks,
            nodes: Vec::new(),
        };
        for node in spec.nodes {
            let shape = match node.shape {
                Primitive::Ring { radius, width } => Shape::Ring { radius, width },
                Primitive::Disc { radius } => Shape::Disc { radius },
                Primitive::Particles {
                    count,
                    spread,
                    gravity,
                } => Shape::Particles {
                    count,
                    spread,
                    gravity,
                },
                Primitive::Sprite {
                    frames,
                    frame_ticks,
                } => {
                    let (frames, assets) = super::together_frames::prepare(frames, assets)?;
                    held.extend(assets);
                    Shape::Sprite {
                        animation: Arc::new(Animation {
                            frames,
                            frame_ticks,
                            phase_offset: 0,
                        }),
                    }
                }
            };
            graph.nodes.push(Node {
                layer: node.layer,
                keys: node.keys,
                shape,
            });
        }
        graph.validate()?;
        let id = angel_live::host::sha256_hex(
            &serde_json::to_vec(&graph).map_err(|e| e.to_string())?,
        );
        let prepared = GRAPHS
            .with(|pool| pool.borrow().get(&id).and_then(Weak::upgrade))
            .unwrap_or_else(|| {
                Arc::new(Prepared {
                    id: id.clone(),
                    graph: Arc::new(graph),
                    _assets: held,
                })
            });
        GRAPHS.with(|pool| {
            let mut pool = pool.borrow_mut();
            pool.retain(|_, v| v.strong_count() > 0);
            pool.insert(id, Arc::downgrade(&prepared));
        });
        bank.insert(name, prepared);
        let pixels = bank
            .values()
            .map(|graph| (&graph.id, graph.graph.pixel_count()))
            .collect::<BTreeMap<_, _>>();
        if pixels.values().sum::<usize>() > effects::MAX_PIXELS {
            return Err("effect bank pixel budget exceeded".into());
        }
    }
    Ok(bank)
}
#[derive(Clone)]
pub struct Emission {
    pub player: u32,
    pub origin: Origin,
    pub prepared: Arc<Prepared>,
    pub offset: [f32; 2],
    pub follow: bool,
    pub resource: Option<Resource>,
}
struct Lease {
    _resource: Resource,
    prepared: Arc<Prepared>,
    playing: Playing,
}
pub struct Staged {
    next: u64,
    leases: Vec<Lease>,
}
#[derive(Default)]
pub struct Runtime {
    leases: BTreeMap<u64, Lease>,
    location: Option<(u64, usize)>,
    next: u64,
    revision: u64,
    pub emitted: u64,
}
impl Runtime {
    pub fn advance(&mut self, run: &mut Run) {
        let before = self.leases.len();
        let location = (run.raid_id, run.at);
        if self.location != Some(location) {
            self.leases.clear();
            self.location = Some(location);
        }
        self.leases.retain(|_, lease| {
            run.players.contains_key(&lease.playing.player)
                && run.tick >= lease.playing.born
                && run.tick - lease.playing.born < u64::from(lease.prepared.graph.ticks)
        });
        if self.leases.len() != before
            || !run
                .module_visuals
                .effects
                .iter()
                .map(|effect| effect.id)
                .eq(self.leases.keys().copied())
        {
            self.publish(run);
        }
    }
    pub fn cancel(&mut self, player: u32, run: &mut Run) {
        let before = self.leases.len();
        self.leases
            .retain(|_, lease| lease.playing.player != player);
        if before != self.leases.len() {
            self.publish(run);
        }
    }
    fn publish(&mut self, run: &mut Run) {
        self.revision = self.revision.wrapping_add(1);
        run.module_visuals = Scene {
            revision: self.revision,
            graphs: self
                .leases
                .values()
                .map(|lease| (lease.prepared.id.clone(), Arc::clone(&lease.prepared.graph)))
                .collect(),
            effects: self
                .leases
                .values()
                .map(|lease| lease.playing.clone())
                .collect(),
        };
    }
    pub fn stage(&self, run: &Run, emissions: Vec<Emission>) -> Result<Option<Staged>, String> {
        if emissions.is_empty() {
            return Ok(Some(Staged {
                next: self.next,
                leases: Vec::new(),
            }));
        }
        if self.leases.len() + emissions.len() > effects::MAX_ACTIVE {
            return Ok(None);
        }
        let mut graphs = self
            .leases
            .values()
            .map(|l| (l.prepared.id.clone(), Arc::clone(&l.prepared.graph)))
            .collect::<BTreeMap<_, _>>();
        let mut generations = self
            .leases
            .values()
            .map(|l| (l.playing.origin.module.clone(), l.playing.origin.generation))
            .collect::<BTreeSet<_>>();
        let mut next = self.next;
        let mut leases = Vec::new();
        for emission in emissions {
            let hero = run
                .players
                .get(&emission.player)
                .ok_or("effect player no longer exists")?;
            if !emission
                .offset
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 32.0)
            {
                return Err("invalid effect offset".into());
            }
            next = next.checked_add(1).ok_or("effect identity exhausted")?;
            graphs.insert(
                emission.prepared.id.clone(),
                Arc::clone(&emission.prepared.graph),
            );
            generations.insert((emission.origin.module.clone(), emission.origin.generation));
            let position = if emission.follow {
                emission.offset
            } else {
                [hero.x + emission.offset[0], hero.y + emission.offset[1]]
            };
            if !position
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 10_000.0)
            {
                return Err("invalid effect anchor".into());
            }
            leases.push(Lease {
                _resource: emission.resource.ok_or("effect has no source lease")?,
                prepared: Arc::clone(&emission.prepared),
                playing: Playing {
                    id: next,
                    player: emission.player,
                    origin: emission.origin,
                    graph: emission.prepared.id.clone(),
                    born: run.tick,
                    position,
                    follow: emission.follow,
                },
            });
        }
        if generations.len() > 8
            || graphs.values().map(|g| g.pixel_count()).sum::<usize>() > effects::MAX_PIXELS
        {
            return Ok(None);
        }
        Ok(Some(Staged { next, leases }))
    }
    pub fn commit(&mut self, staged: Staged, run: &mut Run) {
        if staged.leases.is_empty() {
            return;
        }
        self.next = staged.next;
        self.emitted += staged.leases.len() as u64;
        for lease in staged.leases {
            self.leases.insert(lease.playing.id, lease);
        }
        self.publish(run);
    }
    pub fn active(&self) -> usize {
        self.leases.len()
    }
    pub fn generations(&self) -> usize {
        self.leases
            .values()
            .map(|l| (&l.playing.origin.module, l.playing.origin.generation))
            .collect::<BTreeSet<_>>()
            .len()
    }
}
