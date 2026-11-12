use super::Delve;
use crate::cart::drive::EvidenceStats;
use crate::houses;
use crate::loop_summary::{DurableEvents, EscalationTier, LoopStatus, LoopSummary};
use crate::together_shooter::holdings::Op;
use crate::village::{self, VillagePulse};
use crate::world_viz::{self, LoopBudgetSnapshot, LoopMirror, SchoolSnapshot};
use angel_live::telemetry::{GraphEvent, Measurement, ToolEventId};
use angel_plug::feed::{
    Competition, Evidence, Feed, FeedRules, FeedWorld, Health, LoopFacts, LoopPhase, LoopTier,
    Route, Serving, StageAct,
};
use ratatui::crossterm::event::KeyCode;
use std::sync::mpsc;

/// Display-side sensing can arrive in a reconnect burst. Preserve FIFO while
/// preventing that optional telemetry from monopolizing input/draw latency.
pub const VILLAGE_PULSES_PER_FRAME: usize = 64;

#[derive(Default)]
pub struct State {
    /// Last-seen loop facts diffed each frame into `AdventureEvent`s for the
    /// world quest (`adventure.rs`). Drains whether or not the world pane is
    /// visible — quest state must track the loop like the village pulses.
    pub(crate) loop_mirror: LoopMirror,
    /// Pulses from the village's single background sensing thread (forge +
    /// head health, rare chatter), drained in `advance`. `None` when the
    /// village is off (`ANGEL_VILLAGE=0`) or under test.
    pub village_rx: Option<mpsc::Receiver<VillagePulse>>,
}

impl From<EvidenceStats> for Evidence {
    fn from(stats: EvidenceStats) -> Self {
        Self {
            active: stats.active,
            observations: stats.observations,
            checks: stats.checks,
            contrasts: stats.contrasts,
            inconclusive: stats.inconclusive,
        }
    }
}

pub(crate) fn loop_status(phase: LoopPhase) -> LoopStatus {
    phase.into()
}

pub(crate) fn escalation_tier(tier: LoopTier) -> EscalationTier {
    tier.into()
}

pub fn summary_of(facts: LoopFacts) -> LoopSummary {
    LoopSummary {
        status: loop_status(facts.phase),
        iteration: facts.iteration,
        tier: escalation_tier(facts.tier),
        submissions: facts.submissions,
        podrace: facts.podrace,
        pivot: facts.pivot,
        measured_candidates: facts.measured_candidates,
        verifier_blocked: facts.verifier_blocked,
        task: facts.task,
        stale_count: facts.stale_count,
        durable_settlement_events: facts.checkpoint.map(|checkpoint| DurableEvents {
            loop_id: checkpoint.loop_id,
            workspace: checkpoint.workspace,
            rows: checkpoint.rows,
            plans: checkpoint
                .plans
                .into_iter()
                .filter_map(|(iteration, plan)| {
                    Some((iteration, serde_json::from_value(plan).ok()?))
                })
                .collect(),
        }),
        last_log_all_errors: facts.last_log_all_errors,
        idle_log_tail: facts.idle_log_tail,
        promotions: facts.promotions,
        last_promotion_score: facts.last_promotion_score,
        promotion_failures: facts.promotion_failures,
        last_promotion_failure_score: facts.last_promotion_failure_score,
    }
}

pub fn serving_of(facts: &Serving) -> houses::Serving {
    let lead = house_of(&facts.lead);
    let mut seats: Vec<houses::HouseId> = Vec::new();
    for seat in &facts.seats {
        if let Some(house) = house_of(seat)
            && !seats.contains(&house)
        {
            seats.push(house);
        }
    }
    let lead = lead.or_else(|| seats.first().copied());
    seats.retain(|&house| Some(house) != lead);
    houses::Serving {
        lead,
        seated: seats,
        turn: facts.turn,
    }
}

fn house_of(route: &Route) -> Option<houses::HouseId> {
    houses::of_agent(route.key)
}

impl FeedWorld for Delve {
    fn feed(&mut self, event: Feed<'_>) {
        let world = &mut self.host.world;
        match event {
            Feed::Ticks(ticks) => {
                for _ in 0..ticks {
                    world.tick();
                }
            }
            Feed::ToolCall {
                id,
                name,
                args,
                painted,
            } => {
                if painted {
                    world.note_tool_call_event(id.clone(), name, args);
                }
            }
            Feed::ToolResult {
                id,
                name,
                summary,
                outcome,
                painted,
            } => {
                if name == "graph" {
                    world.settle_graph_call(id, outcome);
                }
                if painted {
                    world.note_tool_result_event(id, name, summary, outcome);
                }
            }
            Feed::Notice { text, painted } => {
                // The scriptorium copies a scroll for every compaction.
                if text.starts_with("compacting context") {
                    self.host.dungeon.ops.note(Op::Compactions);
                }
                if painted {
                    world.note_notice(text);
                }
            }
            Feed::Compacted => self.host.dungeon.ops.note(Op::Compactions),
            Feed::Evidence(evidence) => world.note_school(SchoolSnapshot {
                active: evidence.active,
                observations: evidence.observations,
                checks: evidence.checks,
                contrasts: evidence.contrasts,
                inconclusive: evidence.inconclusive,
            }),
            Feed::Lesson(term) => world.begin_teaching_lesson(term),
            Feed::LoopIdle => self.host.dungeon.ops_loop = 0,
            Feed::TurnEnded { ok, painted } => {
                if painted {
                    world.turn_ended(ok);
                } else {
                    world.turn_ended_hidden_stage(ok);
                }
            }
            Feed::Workspace(root) => {
                self.host.world = world_viz::World::for_workspace(root);
                self.host.together = crate::together::Together::default();
                self.host.dungeon.reset_retaining_wish_jobs();
                if let Err(error) = self.host.dungeon.restore(root) {
                    self.host.dungeon.notice = format!("Could not restore delve: {error}");
                }
                self.host
                    .world
                    .enable_districts(super::app::scan_workspace_districts(root));
            }
        }
    }

    fn graph_event(&mut self, id: &ToolEventId, event: &GraphEvent) {
        self.host.world.note_graph_event(id, event);
    }

    fn competition(&mut self, event: Competition<'_>) {
        let world = &mut self.host.world;
        match event {
            Competition::Started(label) => world.competition_test_started(label),
            Competition::Pending(label) => world.competition_test_pending(label),
            Competition::Reported { label, score } => {
                world.competition_test_reported_score(label, score)
            }
            Competition::Complete(label) => world.competition_test_complete(label),
            Competition::Error { label, reason } => world.competition_test_error(label, reason),
            Competition::TimedOut(label) => world.competition_test_timeout(label),
        }
    }

    fn health(&mut self, read: &dyn Fn() -> Health) {
        let health = read();
        let world = &mut self.host.world;
        world.note_memory_health(health.memory);
        world.note_atlas_status(health.atlas, health.atlas_reviews);
        world.note_backplane_status(health.clerk, health.clerk_queue, health.clerk_conflicts);
    }

    fn serving(&mut self, read: &dyn Fn() -> Serving) {
        let serving = serving_of(&read());
        houses::note_serving(&serving);
        self.host.world.note_serving(&serving);
    }

    fn horse_service(&mut self, read: &dyn Fn() -> (Route, Serving), calls: u32) {
        let (lead, facts) = read();
        let lead = house_of(&lead);
        let seated: Vec<houses::HouseId> = serving_of(&facts)
            .seated
            .into_iter()
            .filter(|&h| Some(h) != lead)
            .collect();
        if lead.is_none() && seated.is_empty() {
            return;
        }
        let realm = self.host.economy();
        if let Some(house) = lead {
            realm
                .home
                .stable
                .note_service(houses::get(house).key, calls);
        }
        for house in seated {
            realm.home.stable.note_service(houses::get(house).key, 1);
        }
        let _ = realm.save();
    }

    fn measured(&mut self, read: &dyn Fn() -> Vec<(String, Measurement)>) {
        for (title, measurement) in read() {
            let _ = self
                .host
                .world
                .competition_test_measurement(&title, &measurement);
        }
    }

    fn adventure(&mut self, read: &dyn Fn() -> LoopFacts, party: u8) {
        let facts = read();
        let has_id = facts.has_id;
        let summary = summary_of(facts);
        // The loop log is persisted by loop_ctl. Iteration boundaries alone
        // are not receipts (resume may first observe iteration 200).
        if has_id {
            self.host.sync_loop(&summary);
        }
        let events = self
            .feed
            .loop_mirror
            .drain(&summary, self.host.world.tool_mix(), party);
        for event in events {
            self.host.world.note_adventure(event);
        }
    }

    fn loop_noted(&mut self, read: &dyn Fn() -> LoopFacts) -> bool {
        let facts = read();
        let active = facts.active;
        let iteration = facts.iteration;
        let done = facts.phase == LoopPhase::Done;
        let budget = active.then(|| LoopBudgetSnapshot {
            iteration,
            max_iters: facts.budget.max_iters,
            tokens_spent: facts.budget.tokens_spent,
            token_budget: facts.budget.token_budget,
            elapsed_secs: facts.budget.elapsed_secs,
            deadline_secs: facts.budget.deadline_secs,
        });
        // Share the visual inactivity/blocker interpretation with the quest.
        // Objective-comparison absence is not evidence of an idle party.
        let agitated = active && LoopMirror::stall_level(&summary_of(facts)) >= 2;
        self.host
            .world
            .note_loop(active, iteration, agitated, done, budget);
        self.host.world.visiting_quintain()
    }

    fn loop_snapshot(&mut self, read: &dyn Fn() -> LoopFacts) {
        self.host.set_loop(summary_of(read()));
    }

    fn wake_village(&mut self) {
        if cfg!(test) || !village::enabled() {
            return;
        }
        let path = village::state_path();
        let state = village::load_state(path.as_deref());
        let heads = village::discover_heads();
        let head_ids: Vec<String> = heads.iter().map(|(id, _)| id.clone()).collect();
        let seed = village::PollerSeed {
            forge_url: village::forge_url(),
            voice_url: village::voice_url(),
            flavor: village::flavor_enabled(),
            heads,
            last_adapter: state.last_adapter.clone(),
            need_name: state.apprentice_name.is_empty(),
            need_boot_chatter: state.chatter.is_empty(),
            town: self.host.world.town_name().to_string(),
        };
        self.host.world.enable_village(state, path, &head_ids);
        self.feed.village_rx = Some(village::spawn_poller(seed));
    }

    fn drain_village(&mut self) -> usize {
        let Some(rx) = &self.feed.village_rx else {
            return 0;
        };
        let mut drained = 0;
        for _ in 0..VILLAGE_PULSES_PER_FRAME {
            let Ok(pulse) = rx.try_recv() else {
                break;
            };
            self.host.world.note_village(pulse);
            drained += 1;
        }
        drained
    }
}

impl FeedRules for Delve {
    fn first_person_key(&mut self, code: KeyCode) -> StageAct {
        let world = &mut self.host.world;
        if world.interior_building() == Some(world_viz::Building::Observatory)
            && matches!(code, KeyCode::Enter | KeyCode::Char('a'))
        {
            return StageAct {
                handled: true,
                open_assets: true,
                ..StageAct::default()
            };
        }
        if world.interior_building() == Some(world_viz::Building::Scriptorium)
            && matches!(code, KeyCode::Enter | KeyCode::Char('c'))
        {
            return StageAct {
                handled: true,
                open_catalog: true,
                ..StageAct::default()
            };
        }
        // Keep the familiar key as a status shortcut. Dotmax is the
        // sole outdoor renderer; room entry owns plate transitions.
        if matches!(code, KeyCode::Char('v')) {
            world_viz::world3d::cycle();
            return StageAct {
                handled: true,
                say: Some(world_viz::world3d::status_line()),
                ..StageAct::default()
            };
        }
        if !world.inside_interior() && !world.riding() && matches!(code, KeyCode::Enter) {
            return StageAct {
                handled: true,
                enter_interior: true,
                ..StageAct::default()
            };
        }
        StageAct::default()
    }

    fn map_key(&mut self, code: KeyCode) -> StageAct {
        match code {
            KeyCode::Left | KeyCode::Char('h') => {
                self.host.world.cycle_landmark(-1);
                StageAct {
                    handled: true,
                    ..StageAct::default()
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.host.world.cycle_landmark(1);
                StageAct {
                    handled: true,
                    ..StageAct::default()
                }
            }
            KeyCode::Enter => StageAct {
                handled: true,
                toggle_camera: true,
                ..StageAct::default()
            },
            _ => StageAct::default(),
        }
    }

    fn leave_interior(&mut self) -> Option<bool> {
        if self.host.world.inside_interior() {
            Some(self.host.world.leave_interior())
        } else {
            None
        }
    }
}
