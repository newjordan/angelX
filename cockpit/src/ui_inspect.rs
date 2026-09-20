//! On-demand inspection of the exact logical Ratatui frame.
//!
//! The model worker never touches `App`. It requests a ticket through this
//! broker; the UI thread captures the next fully rendered frame after overlays
//! and border repair, then publishes owned cells plus matching semantic state.

use crate::App;
use crate::club::ToolDef;
use crate::harness::{Tool, ToolRegistry};
use ratatui::Frame;
use ratatui::buffer::{Cell, CellDiffOption};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const MAX_CAPTURE_CELLS: usize = 65_536;
const MAX_EXACT_RESPONSE_CELLS: usize = 2_048;
const MAX_PENDING_REQUESTS: usize = 16;
const MAX_CACHED_SNAPSHOTS: usize = 8;
const WAIT_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum InspectColor {
    Reset,
    Named { name: String },
    Indexed { index: u8 },
    Rgb { r: u8, g: u8, b: u8 },
}

impl From<Color> for InspectColor {
    fn from(color: Color) -> Self {
        match color {
            Color::Reset => Self::Reset,
            Color::Indexed(index) => Self::Indexed { index },
            Color::Rgb(r, g, b) => Self::Rgb { r, g, b },
            named => Self::Named {
                name: format!("{named:?}").to_ascii_lowercase(),
            },
        }
    }
}

#[derive(Clone)]
struct InspectCell {
    symbol: String,
    fg: InspectColor,
    bg: InspectColor,
    underline: InspectColor,
    modifier_bits: u16,
    modifiers: String,
    skip: bool,
}

impl From<&Cell> for InspectCell {
    fn from(cell: &Cell) -> Self {
        Self {
            symbol: cell.symbol().to_string(),
            fg: InspectColor::from(cell.fg),
            bg: InspectColor::from(cell.bg),
            underline: InspectColor::from(cell.underline_color),
            modifier_bits: cell.modifier.bits(),
            modifiers: format!("{:?}", cell.modifier),
            skip: cell.diff_option == CellDiffOption::Skip,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum UiOperation {
    InspectImage {
        action: crate::still_inspector::Action,
    },
    OpenControl {
        control: ControlKind,
    },
    NavigateControl {
        direction: NavigationDirection,
    },
    FocusSurface {
        surface: FocusSurface,
    },
    OpenObservatory {
        campaign: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum ControlKind {
    Model,
    Thinking,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum NavigationDirection {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum FocusSurface {
    Core,
    Agent,
    Scryglass,
}

#[derive(Clone)]
struct UiSnapshot {
    id: u64,
    captured_ms: u128,
    width: u16,
    height: u16,
    cells: Vec<InspectCell>,
    frame_omitted: Option<String>,
    panels: Vec<(String, Rect)>,
    semantics: Value,
    operation: Option<Value>,
    complete_visual: bool,
}

#[derive(Clone)]
struct UiRequest {
    ticket: u64,
    operation: Option<UiOperation>,
}

pub(crate) struct PreparedCapture {
    ticket: u64,
    operation: Option<Value>,
}

#[derive(Default)]
struct BrokerState {
    next_ticket: u64,
    pending: VecDeque<UiRequest>,
    waiting: HashSet<u64>,
    completed: HashMap<u64, Arc<UiSnapshot>>,
    cached: VecDeque<Arc<UiSnapshot>>,
    closed: bool,
}

pub(crate) struct UiSnapshotBroker {
    state: Mutex<BrokerState>,
    ready: Condvar,
}

impl UiSnapshotBroker {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(BrokerState::default()),
            ready: Condvar::new(),
        })
    }

    fn request(&self, operation: Option<UiOperation>) -> Result<Arc<UiSnapshot>, String> {
        let mut state = self.state.lock().map_err(|_| "UI broker poisoned")?;
        if state.closed {
            return Err("interactive UI is closed".to_string());
        }
        if state.pending.len() >= MAX_PENDING_REQUESTS {
            return Err(format!(
                "UI verifier queue is full ({MAX_PENDING_REQUESTS} pending requests)"
            ));
        }
        state.next_ticket = state.next_ticket.wrapping_add(1).max(1);
        let ticket = state.next_ticket;
        state.pending.push_back(UiRequest { ticket, operation });
        state.waiting.insert(ticket);
        let deadline = Instant::now() + WAIT_TIMEOUT;
        while !state.completed.contains_key(&ticket) && !state.closed {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                state.waiting.remove(&ticket);
                state.pending.retain(|request| request.ticket != ticket);
                return Err("UI did not produce a frame within 2s".to_string());
            }
            let (next, timeout) = self
                .ready
                .wait_timeout(state, remaining)
                .map_err(|_| "UI broker poisoned")?;
            state = next;
            if timeout.timed_out() && !state.completed.contains_key(&ticket) {
                state.waiting.remove(&ticket);
                state.pending.retain(|request| request.ticket != ticket);
                return Err("UI did not produce a frame within 2s".to_string());
            }
        }
        state.waiting.remove(&ticket);
        state
            .completed
            .remove(&ticket)
            .ok_or_else(|| "UI snapshot unavailable".to_string())
    }

    fn take_pending(&self) -> Option<UiRequest> {
        self.state
            .lock()
            .ok()
            .and_then(|mut state| state.pending.pop_front())
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.state
            .lock()
            .is_ok_and(|state| !state.pending.is_empty())
    }

    fn cached(&self, snapshot_id: u64) -> Result<Arc<UiSnapshot>, String> {
        self.state
            .lock()
            .map_err(|_| "UI broker poisoned")?
            .cached
            .iter()
            .find(|snapshot| snapshot.id == snapshot_id)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "snapshot {snapshot_id} is not cached; request a fresh frame and page it before the {}-snapshot cache advances",
                    MAX_CACHED_SNAPSHOTS
                )
            })
    }

    fn publish(&self, ticket: u64, snapshot: UiSnapshot) {
        if let Ok(mut state) = self.state.lock() {
            if state.waiting.contains(&ticket) {
                let snapshot = Arc::new(snapshot);
                state.completed.insert(ticket, Arc::clone(&snapshot));
                state.cached.push_back(snapshot);
                while state.cached.len() > MAX_CACHED_SNAPSHOTS {
                    state.cached.pop_front();
                }
            }
            self.ready.notify_all();
        }
    }
}

impl Drop for UiSnapshotBroker {
    fn drop(&mut self) {
        if let Ok(mut state) = self.state.lock() {
            state.closed = true;
            self.ready.notify_all();
        }
    }
}

static INTERACTIVE_BROKER: OnceLock<Arc<UiSnapshotBroker>> = OnceLock::new();

pub(crate) fn interactive_broker() -> Arc<UiSnapshotBroker> {
    Arc::clone(INTERACTIVE_BROKER.get_or_init(UiSnapshotBroker::new))
}

pub(crate) fn install(registry: &mut ToolRegistry, broker: Arc<UiSnapshotBroker>) {
    registry.register(Box::new(UiInspectTool {
        broker: Arc::clone(&broker),
    }));
    registry.register(Box::new(UiVerifyTool { broker }));
}

struct UiInspectTool {
    broker: Arc<UiSnapshotBroker>,
}

impl Tool for UiInspectTool {
    fn name(&self) -> &str {
        "ui_inspect"
    }

    fn def(&self) -> ToolDef {
        ToolDef {
            name: "ui_inspect".to_string(),
            description: "Inspect the next fully rendered interactive cockpit frame. Defaults to the braille Scryglass and returns matching panel, focus, media, world, and control state. Use format=cells for exact Ratatui symbols, color variants, underline color, modifiers, and skip flags.".to_string(),
            params: json!({
                "type": "object",
                "properties": {
                    "snapshot_id": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "Page an immutable cached snapshot without drawing a new frame."
                    },
                    "scope": {
                        "type": "string",
                        "enum": ["scryglass", "artifacts", "screen", "header", "transcript", "agent", "input", "shell", "image", "transport"],
                        "default": "scryglass"
                    },
                    "format": {
                        "type": "string",
                        "enum": ["text", "cells"],
                        "default": "text"
                    },
                    "row_offset": { "type": "integer", "minimum": 0, "default": 0 },
                    "row_count": { "type": "integer", "minimum": 1, "maximum": 120, "default": 24 }
                }
            }),
        }
    }

    fn call(&self, args: &Value) -> Result<String, String> {
        let snapshot = match args.get("snapshot_id") {
            Some(value) => self.broker.cached(
                value
                    .as_u64()
                    .ok_or_else(|| "snapshot_id must be a positive integer".to_string())?,
            )?,
            None => self.broker.request(None)?,
        };
        render_snapshot(&snapshot, args, "scryglass", 24)
    }
}

struct UiVerifyTool {
    broker: Arc<UiSnapshotBroker>,
}

impl Tool for UiVerifyTool {
    fn name(&self) -> &str {
        "ui_verify"
    }

    fn def(&self) -> ToolDef {
        ToolDef {
            name: "ui_verify".to_string(),
            description: "Apply one typed, display-only operation through the ordinary cockpit input path, then return its receipt with exact terminal cells and semantic state from that same completed frame. Page the immutable result later with ui_inspect(snapshot_id=...).".to_string(),
            params: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "oneOf": [
                            {
                                "type": "object",
                                "properties": {
                                    "op": { "const": "inspect_image" },
                                    "action": { "type": "string", "enum": ["zoom_in", "zoom_out", "left", "right", "up", "down", "fit"] }
                                },
                                "required": ["op", "action"],
                                "additionalProperties": false
                            },
                            {
                                "type": "object",
                                "properties": {
                                    "op": { "const": "open_control" },
                                    "control": { "type": "string", "enum": ["model", "thinking"] }
                                },
                                "required": ["op", "control"],
                                "additionalProperties": false
                            },
                            {
                                "type": "object",
                                "properties": {
                                    "op": { "const": "navigate_control" },
                                    "direction": { "type": "string", "enum": ["up", "down", "left", "right"] }
                                },
                                "required": ["op", "direction"],
                                "additionalProperties": false
                            },
                            {
                                "type": "object",
                                "properties": {
                                    "op": { "const": "focus_surface" },
                                    "surface": { "type": "string", "enum": ["core", "agent", "scryglass"] }
                                },
                                "required": ["op", "surface"],
                                "additionalProperties": false
                            },
                            {
                                "type": "object",
                                "properties": {
                                    "op": { "const": "open_observatory" },
                                    "campaign": {
                                        "type": "string",
                                        "minLength": 1,
                                        "description": "Optional exact campaign id from the loaded report catalog."
                                    }
                                },
                                "required": ["op"],
                                "additionalProperties": false
                            }
                        ]
                    },
                    "scope": {
                        "type": "string",
                        "enum": ["scryglass", "artifacts", "screen", "header", "transcript", "agent", "input", "shell", "image", "transport"],
                        "default": "screen"
                    },
                    "format": {
                        "type": "string",
                        "enum": ["text", "cells"],
                        "default": "text"
                    },
                    "row_offset": { "type": "integer", "minimum": 0, "default": 0 },
                    "row_count": { "type": "integer", "minimum": 1, "maximum": 120, "default": 17 }
                },
                "required": ["operation"],
                "additionalProperties": false
            }),
        }
    }

    fn call(&self, args: &Value) -> Result<String, String> {
        let operation = args
            .get("operation")
            .cloned()
            .ok_or_else(|| "operation is required".to_string())?;
        let operation = serde_json::from_value(operation)
            .map_err(|error| format!("invalid UI operation: {error}"))?;
        let snapshot = self.broker.request(Some(operation))?;
        render_snapshot(&snapshot, args, "screen", 17)
    }
}

fn scope_name(scope: &str) -> &str {
    match scope {
        "scryglass" => "artifacts",
        other => other,
    }
}

fn render_snapshot(
    snapshot: &UiSnapshot,
    args: &Value,
    default_scope: &str,
    default_row_count: u16,
) -> Result<String, String> {
    let scope = args
        .get("scope")
        .and_then(Value::as_str)
        .unwrap_or(default_scope);
    let format = args.get("format").and_then(Value::as_str).unwrap_or("text");
    let row_offset = args.get("row_offset").and_then(Value::as_u64).unwrap_or(0) as u16;
    let row_count = args
        .get("row_count")
        .and_then(Value::as_u64)
        .unwrap_or(u64::from(default_row_count))
        .clamp(1, 120) as u16;
    let rect = if scope == "screen" {
        Rect::new(0, 0, snapshot.width, snapshot.height)
    } else {
        snapshot
            .panels
            .iter()
            .find(|(name, _)| name == scope_name(scope))
            .map(|(_, rect)| *rect)
            .ok_or_else(|| format!("scope {scope:?} is not visible in this frame"))?
    };
    let start = row_offset.min(rect.height);
    let count = row_count.min(rect.height.saturating_sub(start));
    let selected = Rect::new(rect.x, rect.y + start, rect.width, count);
    let requested_cells = selected.width as usize * selected.height as usize;
    if format == "cells" && requested_cells > MAX_EXACT_RESPONSE_CELLS {
        return Ok(json!({
            "version": 2,
            "error": "exact cell page is too large",
            "max_cells": MAX_EXACT_RESPONSE_CELLS,
            "requested_cells": requested_cells,
            "hint": "request fewer rows or a narrower panel scope with this snapshot_id",
            "snapshot_id": snapshot.id,
            "captured_ms": snapshot.captured_ms,
            "terminal": { "width": snapshot.width, "height": snapshot.height },
            "scope": scope,
            "rect": { "x": rect.x, "y": rect.y, "width": rect.width, "height": rect.height },
            "operation": snapshot.operation,
            "state": snapshot.semantics
        })
        .to_string());
    }

    let frame = if let Some(reason) = snapshot.frame_omitted.as_ref() {
        json!({ "omitted": reason })
    } else if format == "cells" {
        let mut rows = Vec::with_capacity(selected.height as usize);
        for y in selected.y..selected.y + selected.height {
            let mut row = Vec::with_capacity(selected.width as usize);
            for x in selected.x..selected.x + selected.width {
                let cell = &snapshot.cells[y as usize * snapshot.width as usize + x as usize];
                row.push(json!({
                    "symbol": cell.symbol,
                    "fg": cell.fg,
                    "bg": cell.bg,
                    "underline": cell.underline,
                    "modifier_bits": cell.modifier_bits,
                    "modifiers": cell.modifiers,
                    "skip": cell.skip
                }));
            }
            rows.push(Value::Array(row));
        }
        Value::Array(rows)
    } else {
        let mut rows = Vec::with_capacity(selected.height as usize);
        for y in selected.y..selected.y + selected.height {
            let mut line = String::new();
            for x in selected.x..selected.x + selected.width {
                line.push_str(
                    &snapshot.cells[y as usize * snapshot.width as usize + x as usize].symbol,
                );
            }
            rows.push(Value::String(line));
        }
        Value::Array(rows)
    };
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    Ok(json!({
        "version": 2,
        "snapshot_id": snapshot.id,
        "captured_ms": snapshot.captured_ms,
        "age_ms": now_ms.saturating_sub(snapshot.captured_ms),
        "terminal": { "width": snapshot.width, "height": snapshot.height },
        "scope": scope,
        "rect": { "x": rect.x, "y": rect.y, "width": rect.width, "height": rect.height },
        "page": { "row_offset": start, "row_count": count, "total_rows": rect.height },
        "complete_visual": snapshot.complete_visual,
        "operation": snapshot.operation,
        "frame": frame,
        "state": snapshot.semantics
    })
    .to_string())
}

fn semantic_state(app: &App) -> Value {
    let focused = app.module_host.focused().map(|id| id.as_str().to_string());
    let agent_menu = app.agent_menu.map(|menu| {
        json!({
            "kind": format!("{:?}", menu.kind).to_ascii_lowercase(),
            "selected": menu.selected,
            "route_target": menu.route_target.map(|(agent, slot)| json!([agent, slot])),
            "confirm_target": menu.confirm_target.map(|(agent, slot)| json!([agent, slot]))
        })
    });
    let active_media = app.scryglass.active_media().and_then(|index| {
        app.media.get(index).map(|media| {
            json!({
                "index": index + 1,
                "kind": media.sigil(),
                "label": media.label(),
                "target": media.target()
            })
        })
    });
    let (video_position, video_duration, video_paused, video_ended) = app.scryglass.video_status();
    let route_metadata = app.bag.in_hand().route_metadata();
    let output_budget = route_metadata
        .output_budget
        .label(route_metadata.output_budget_provenance.as_deref());
    let stage_route = app.scryglass.controller.route();
    let stage_overlay = app.scryglass.controller.overlay();
    let stage_owner = app.scryglass.controller.overlay_owner();
    let lesson_semantics = app.scryglass.lesson().map(|lesson| {
        json!({
            "topic": lesson.term(),
            "loading": lesson.is_loading(),
            "reference_status": format!("{:?}", lesson.reference_status()).to_ascii_lowercase(),
            "source_url": lesson.source_url()
        })
    });
    let catalog_semantics = app.scryglass.catalog_open().then(|| {
        let shelf = app.scryglass.selected_shelf();
        let tutor = crate::library::tutor_for(shelf.discipline);
        json!({
            "selection": app.scryglass.catalog_selection(),
            "count": crate::library::catalog_shelves().len(),
            "shelf_id": shelf.id,
            "shelf_title": shelf.title,
            "tutor_id": tutor.id,
            "tutor_name": tutor.name
        })
    });
    let latest_work = app.world.latest_active_work();
    let active_work = app
        .world
        .active_work()
        .map(|work| {
            json!({
                "id": work.id.0,
                "literal": work.literal,
                "landmark": format!("{:?}", work.landmark),
                "activity": format!("{:?}", work.activity),
                "summary": work.summary,
                "outcome": work.outcome.map(|outcome| json!({
                    "execution": outcome.execution.as_str(),
                    "verification": outcome.verification.as_str()
                }))
            })
        })
        .collect::<Vec<_>>();
    let next_unlock = app.world.next_unlock().map(|(threshold, label)| {
        json!({ "threshold": threshold, "label": label, "remaining": threshold.saturating_sub(app.world.renown()) })
    });
    let hearth = app.world.hearth_state();
    let formation = formation_semantics(app);
    let atlas_status = app.atlas.status();
    let atlas_items = app.atlas.list(
        app.atlas_view.lane,
        (!app.atlas_view.query.is_empty()).then_some(app.atlas_view.query.as_str()),
    );
    let atlas_selected = atlas_items.get(app.atlas_view.selected);
    let session_health = match app.session.save_status() {
        crate::session::SessionSaveStatus::Healthy => json!({ "state": "healthy" }),
        crate::session::SessionSaveStatus::Pending => json!({ "state": "pending" }),
        crate::session::SessionSaveStatus::Failed(error) => {
            json!({ "state": "degraded", "error": error.to_string() })
        }
    };
    let focused_agent = focused.as_deref() == Some("agent");
    let operational_rows = app
        .messages
        .iter()
        .filter(|message| matches!(message.role, crate::transcript::Role::Activity))
        .count();
    let tool_arg_repairs = crate::club::tool_arg_repair_records();
    let mut semantics = json!({
        "focus": {
            "module": focused,
            "approval": app.pending_approval.is_some(),
            "approval_scope": app.pending_approval.as_ref().and_then(|pending| pending.scope_label.clone()),
            "loop_dialog": app.loop_dialog.is_some(),
            "agent_menu": agent_menu,
            "shell": app.shell_focused
        },
        "agent": {
            "label": app.bag.in_hand_label(),
            "model": app.bag.in_hand_mode(),
            "reasoning_effort": app.bag.reasoning_effort(),
            "busy": app.thinking.is_some() || app.bg_job.is_some(),
            "detail_mode": if focused_agent { "focused" } else { "summary" },
            "reasoning_visible": focused_agent,
            "reasoning_state": if app.thinking.is_some() { "live" } else { "previous-turn" },
            "current_operation": latest_work.map(|work| work.literal.as_str()),
            "destination": latest_work.map(|work| format!("{:?}", work.landmark)),
            "output_budget": output_budget,
            "output_budget_policy": format!("{:?}", route_metadata.output_budget),
            "output_budget_provenance": route_metadata.output_budget_provenance,
            "workspace_boundary": {
                "lexical_root": app.tools.workspace_boundary().lexical_root,
                "canonical_root": app.tools.workspace_boundary().canonical_root,
                "repository_root": app.tools.workspace_boundary().repository.root,
                "repository_key": app.tools.workspace_boundary().repository.key
            }
        },
        "scryglass": {
            "image_inspector": app.viewer.inspector.semantic(),
            "surface": format!("{:?}", app.scryglass.surface),
            "visible": app.scryglass.visible,
            "renderable": app.scryglass.renderable,
            "world_mode": format!("{:?}", app.scryglass.world_mode()),
            "follow_agent": app.scryglass.follow_agent,
            "look_yaw": app.scryglass.look_yaw,
            "look_pitch": app.scryglass.look_pitch,
            "fov": app.scryglass.fov,
            "route": format!("{stage_route:?}"),
            "underlying_route": format!("{:?}", app.scryglass.controller.underlying_route()),
            "overlay": stage_overlay.map(|overlay| format!("{overlay:?}")),
            "overlay_owner": stage_owner.map(|owner| format!("{owner:?}")),
            "overlay_remaining_ms": app.scryglass.overlay_remaining_ms(Instant::now()),
            "pinned": app.scryglass.active_pinned(),
            "queued": app.scryglass.pending_count(),
            "media": active_media,
            "teaching": {
                "lesson": lesson_semantics,
                "catalog": catalog_semantics
            },
            "video": {
                "position_ms": video_position.as_millis(),
                "duration_ms": video_duration.map(|duration| duration.as_millis()),
                "paused": video_paused,
                "ended": video_ended
            }
        },
        "world": {
            "town": app.world.town_name(),
            "tier": app.world.level(),
            "renown": app.world.renown(),
            "next_unlock": next_unlock,
            "riding": app.world.riding(),
            "arrived": app.world.arrived_building().map(|building| format!("{:?}", building)),
            "inside_interior": app.world.inside_interior(),
            "interior": app.world.interior_building().map(|building| format!("{:?}", building)),
            "destination": format!("{:?}", app.world.destination()),
            "selected_building": match stage_route { crate::scryglass::StageRoute::Explore(building) => Some(format!("{building:?}")), _ => None },
            "camera_mode": app.world.camera_mode_label(),
            "active_events": active_work,
            "event_diagnostics": app.world.event_diagnostics(),
            "causal_ribbon": app.world.causal_ribbon(),
            "environment": {
                "weather": app.world.weather_label(),
                "hearth": { "light": hearth.light, "warmth": hearth.warmth, "beats": hearth.beats, "prosperity": hearth.prosperity },
                "day_phase": hearth.phase().label()
            },
            "memory_health": format!("{:?}", app.world.memory_health()).to_ascii_lowercase()
        },
        "formation": formation,
        "media": { "total": app.media.len(), "scroll": app.media_scroll },
        "observatory": app.observatory.semantic_state(
            stage_route == crate::scryglass::StageRoute::Observatory,
        ),
        "controls": {
            "transcript_mode": format!("{:?}", app.transcript_mode).to_ascii_lowercase(),
            "operational_row_count": operational_rows,
            "plan": app.plan_mode,
            "vim": app.vim_mode,
            "relentless": app.relentless_execution,
            "plain": app.plain_mode
        },
        "persistence": { "session": session_health, "memory": format!("{:?}", app.tools.store.health()).to_ascii_lowercase() },
        "tool_argument_repairs": tool_arg_repairs.into_iter().map(|record| json!({
            "call_id": record.call_id, "kind": record.kind, "digest": record.digest, "length": record.length
        })).collect::<Vec<_>>()
    });
    if app.atlas.enabled() {
        semantics["atlas"] = json!({
            "enabled": atlas_status.enabled,
            "health": atlas_status.health,
            "health_detail": atlas_status.detail,
            "open": stage_route == crate::scryglass::StageRoute::Vault,
            "lane": app.atlas_view.lane,
            "query": app.atlas_view.query,
            "review_count": atlas_status.review_count,
            "project_items": atlas_status.project_items,
            "shared_items": atlas_status.shared_items,
            "visible_items": atlas_items.len(),
            "selected_index": app.atlas_view.selected,
            "selected": atlas_selected.map(|item| json!({
                "id": item.id,
                "scope": item.scope,
                "kind": item.kind,
                "lifecycle": item.lifecycle,
                "epistemic": item.epistemic,
                "contested": item.contested,
                "stale": item.stale,
                "injection": item.injection,
                "source_ids": item.sources.iter().map(|source| &source.id).collect::<Vec<_>>(),
                "why_selected": if app.atlas_view.query.is_empty() {
                    format!("selected from {} lane", app.atlas_view.lane.label())
                } else {
                    format!("matched search {:?}", app.atlas_view.query)
                }
            }))
        });
    }
    let backplane = app.tools.backplane();
    let clerk = app.tools.clerk().status();
    semantics["backplane"] = json!({
        "mode": format!("{:?}", crate::backplane::mode()).to_ascii_lowercase(),
        "surfaces": backplane.surfaces().into_iter().take(64).collect::<Vec<_>>(),
        "routes": backplane.routes().into_iter().take(64).collect::<Vec<_>>(),
        "resource_leases": backplane.leases().into_iter().map(|lease| json!({
            "id": lease.id,
            "resource_group": lease.resource_group,
            "mode": lease.mode,
            "role": lease.role,
            "route_id": lease.route_id,
            "operator_confirmed": lease.operator_confirmed,
        })).collect::<Vec<_>>(),
        "clerk": clerk,
        "last_turn_outcome": &app.last_turn_outcome,
    });
    semantics
}

fn formation_semantics(app: &App) -> Value {
    let (engagement, phase) = if let Some(engagement) = app.moa_one_shot.as_ref() {
        (Some(engagement), "next-turn")
    } else if let Some(engagement) = app.moa_session.as_ref() {
        (Some(engagement), "session")
    } else {
        (
            None,
            if app.moa_deck.is_some() {
                "staging"
            } else {
                "idle"
            },
        )
    };
    let Some(engagement) = engagement else {
        return app.moa_deck.as_ref().map_or_else(
            || json!({ "phase": phase, "engagement_id": Value::Null, "seats": [] }),
            |deck| json!({
                "phase": phase,
                "engagement_id": Value::Null,
                "formation": deck.selected().name,
                "focus": format!("{:?}", deck.focus()).to_ascii_lowercase(),
                "seats": deck.selected().slots().iter().zip(deck.selected_roster().assignments()).map(|(slot, model)| json!({
                    "seat_id": slot.label(), "role": slot.role.label(), "model": model.as_ref().map(|model| model.display_label()),
                    "status": if model.is_some() { "staged" } else { "unassigned" }
                })).collect::<Vec<_>>()
            }),
        );
    };
    let formation = crate::formations::formation(engagement.formation);
    json!({
        "engagement_id": engagement.id,
        "formation": formation.name,
        "phase": phase,
        "sequence": engagement.id,
        "seats": formation.slots().iter().zip(engagement.roster.assignments()).map(|(slot, model)| json!({
            "seat_id": slot.label(),
            "role": slot.role.label(),
            "model": model.as_ref().map(|model| model.display_label()),
            "status": if model.is_some() { "mustered" } else { "unassigned" }
        })).collect::<Vec<_>>()
    })
}

fn apply_operation(app: &mut App, operation: &UiOperation) -> Result<String, String> {
    if let UiOperation::InspectImage { action } = operation {
        if app.pending_approval.is_some() || app.agent_menu.is_some() || app.loop_dialog.is_some() {
            return Err("a modal owns input".into());
        }
        return if app.inspect_still(*action) {
            Ok(format!("image display {action:?}"))
        } else {
            Err("no ready visible still image to inspect".into())
        };
    }
    if let UiOperation::OpenObservatory { campaign } = operation {
        if campaign.as_ref().is_some_and(|id| id.trim().is_empty()) {
            return Err("campaign id must not be blank".to_string());
        }
        app.input = campaign.as_ref().map_or_else(
            || "/observatory".to_string(),
            |id| format!("/observatory campaign {}", id.trim()),
        );
        app.cursor = app.input.len();
        app.submit();
        if app.scryglass.controller.route() != crate::scryglass::StageRoute::Observatory {
            return Err("Observatory surface could not be opened".to_string());
        }
        let state = app.observatory.semantic_state(true);
        if let Some(expected) = campaign.as_ref().map(|id| id.trim()) {
            let selected = state["selected_campaign"]["id"].as_str();
            if selected.is_none_or(|id| !id.eq_ignore_ascii_case(expected)) {
                let error = state["error"]
                    .as_str()
                    .map_or_else(|| format!("unknown campaign {expected}"), str::to_string);
                return Err(error);
            }
            return Ok(format!("opened Observatory campaign {expected}"));
        }
        return Ok("opened Observatory report catalog".to_string());
    }

    let code = match operation {
        UiOperation::OpenControl { control } => {
            if app.thinking.is_some() || app.bg_job.is_some() {
                return Err("brain controls are locked while a turn owns the flight slot".into());
            }
            match control {
                ControlKind::Model => KeyCode::F(9),
                ControlKind::Thinking => KeyCode::F(10),
            }
        }
        UiOperation::NavigateControl { direction } => {
            if app.agent_menu.is_none() {
                return Err("no model or thinking control is open".into());
            }
            match direction {
                NavigationDirection::Up => KeyCode::Up,
                NavigationDirection::Down => KeyCode::Down,
                NavigationDirection::Left => KeyCode::Left,
                NavigationDirection::Right => KeyCode::Right,
            }
        }
        UiOperation::FocusSurface { surface } => match surface {
            FocusSurface::Core => KeyCode::F(2),
            FocusSurface::Agent => KeyCode::F(3),
            FocusSurface::Scryglass => KeyCode::F(4),
        },
        UiOperation::OpenObservatory { .. } | UiOperation::InspectImage { .. } => {
            unreachable!("handled above")
        }
    };
    app.on_key(KeyEvent::new(code, KeyModifiers::NONE));

    match operation {
        UiOperation::OpenControl { control } => {
            let expected = match control {
                ControlKind::Model => crate::agent::controls::AgentMenuKind::Model,
                ControlKind::Thinking => crate::agent::controls::AgentMenuKind::Thinking,
            };
            if app.agent_menu.is_some_and(|menu| menu.kind == expected) {
                Ok(format!(
                    "opened {} control",
                    format!("{control:?}").to_ascii_lowercase()
                ))
            } else {
                Err(format!(
                    "{} control is unavailable for the selected route",
                    format!("{control:?}").to_ascii_lowercase()
                ))
            }
        }
        UiOperation::NavigateControl { direction } => Ok(format!(
            "navigated {} control",
            format!("{direction:?}").to_ascii_lowercase()
        )),
        UiOperation::FocusSurface { surface } => {
            let expected = match surface {
                FocusSurface::Core => "core",
                FocusSurface::Agent => "agent",
                FocusSurface::Scryglass => "artifacts",
            };
            if app
                .module_host
                .focused()
                .is_some_and(|id| id.as_str() == expected)
            {
                Ok(format!("focused {expected}"))
            } else {
                Err(format!("surface {expected} could not be focused"))
            }
        }
        UiOperation::OpenObservatory { .. } | UiOperation::InspectImage { .. } => {
            unreachable!("handled above")
        }
    }
}

pub(crate) fn prepare_next_capture(
    app: &mut App,
    broker: &UiSnapshotBroker,
) -> Option<PreparedCapture> {
    let request = broker.take_pending()?;
    let operation = request.operation.map(|operation| {
        let requested = serde_json::to_value(&operation).unwrap_or(Value::Null);
        let before_state = semantic_state(app);
        match apply_operation(app, &operation) {
            Ok(detail) => json!({
                "requested": requested,
                "status": "applied",
                "detail": detail,
                "before_state": before_state
            }),
            Err(error) => json!({
                "requested": requested,
                "status": "rejected",
                "error": error,
                "before_state": before_state
            }),
        }
    });
    Some(PreparedCapture {
        ticket: request.ticket,
        operation,
    })
}

pub(crate) fn capture_after_draw(
    app: &App,
    frame: &mut Frame,
    broker: &UiSnapshotBroker,
    prepared: PreparedCapture,
) {
    let area = frame.area();
    let content = frame.buffer_mut().content();
    let (cells, frame_omitted) = if content.len() <= MAX_CAPTURE_CELLS {
        (content.iter().map(InspectCell::from).collect(), None)
    } else {
        (
            Vec::new(),
            Some(format!(
                "{} cells exceed capture limit {}",
                content.len(),
                MAX_CAPTURE_CELLS
            )),
        )
    };
    let panels = app
        .panel_frames
        .iter()
        .map(|(kind, rect)| (kind.name().to_string(), rect))
        .collect::<Vec<_>>();
    let semantics = semantic_state(app);
    let complete_visual = frame_omitted.is_none() && cells.iter().all(|cell| !cell.skip);
    let captured_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    broker.publish(
        prepared.ticket,
        UiSnapshot {
            id: prepared.ticket,
            captured_ms,
            width: area.width,
            height: area.height,
            cells,
            frame_omitted,
            panels,
            semantics,
            operation: prepared.operation,
            complete_visual,
        },
    );
}

#[cfg(test)]
#[path = "../../tests/cockpit/app/ui_inspect__tests.rs"]
mod tests;
