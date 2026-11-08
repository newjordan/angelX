//! The loop card in the cockpit (`ui::loop_card`): the open launch drawn
//! as the window's card or the terminal's box, the start played after the
//! flag drops, and the race ticker embedded under the header that follows
//! the running loop. What the ticker counts comes from the loop's own
//! state (`loop_ctl` for `/loop`, `handoff_rl` for the campaign); nothing
//! here steers the loop.

use crate::app::App;
use crate::drive::loop_ctl::LoopStatus;
use crate::drive::loop_dialog::{LoopLaunchDialog, LoopLaunchPurpose};
use crate::ui::loop_card::{self, Race};
use ratatui::{Frame, layout::Rect};
use std::time::Instant;

/// The start: how long the flag and the launch play before the ticker docks.
const LAUNCH_HOLD: f32 = 1.15;
/// How long the ticker takes to settle into the subheader after the start.
const DOCKING: f32 = 0.42;
/// The ticker's knight comes in over this long as it docks.
const DOCK_IN: f32 = 0.3;
/// A finished loop holds its chequers in the ticker this long, then goes.
const COMPLETE_HOLD: f32 = 6.0;

/// The launch played after the flag drops, and the widget after it.
#[derive(Default)]
pub(crate) struct LoopCardState {
    launch: Option<(LoopLaunchDialog, Instant)>,
    dock: Option<Dock>,
    /// The cell size the ticker's own pictures were handed over at.
    pictures: Option<(u16, u16)>,
}

/// The widget's run: which loop it follows, the laps it has counted, and
/// the lap the knight is running now.
struct Dock {
    purpose: LoopLaunchPurpose,
    since: Instant,
    counted: usize,
    run: Option<Instant>,
    owed: usize,
    paused: bool,
    finished_at: Option<Instant>,
}

impl LoopCardState {
    /// The flag dropped on `dialog` and its loop is running: in the window,
    /// play the start and dock. A terminal docks immediately.
    pub(crate) fn launched(&mut self, dialog: LoopLaunchDialog, now: Instant) {
        let purpose = dialog.purpose;
        self.launch = crate::ui::glass::active().then_some((dialog, now));
        self.dock = Some(Dock {
            purpose,
            since: now,
            counted: 0,
            run: None,
            owed: 0,
            paused: false,
            finished_at: None,
        });
    }

    /// While the start plays or the knight runs, the cockpit draws fast:
    /// the ticker's road walks and its timer counts. A paused loop is
    /// still; a finished one draws until its chequers leave.
    pub(crate) fn animating(&self, now: Instant) -> bool {
        self.launch.is_some()
            || self.dock.as_ref().is_some_and(|d| {
                d.run.is_some()
                    || d.owed > 0
                    || !d.paused
                    || d.finished_at.is_some_and(|at| {
                        now.saturating_duration_since(at).as_secs_f32() < COMPLETE_HOLD
                    })
                    || now.saturating_duration_since(d.since).as_secs_f32()
                        < LAUNCH_HOLD + DOCKING + DOCK_IN
            })
    }

    /// Whether the widget is up, following a loop.
    #[cfg(test)]
    pub(crate) fn docked(&self) -> bool {
        self.dock.is_some()
    }
}

impl Dock {
    /// Queue every completed iteration, even during the launch or a modal.
    fn observe(&mut self, done: usize) {
        self.owed = self.owed.saturating_add(done.saturating_sub(self.counted));
        self.counted = done;
    }

    fn step(&mut self, now: Instant) -> Option<f32> {
        if self.run.is_some_and(|run| {
            loop_card::lap_run(now.saturating_duration_since(run).as_secs_f32()).is_none()
        }) {
            self.run = None;
        }
        if self.run.is_none() && self.owed > 0 {
            self.owed -= 1;
            self.run = Some(now);
        }
        self.run
            .and_then(|run| loop_card::lap_run(now.saturating_duration_since(run).as_secs_f32()))
    }
}

/// The rows the loop ticker takes in the layout this frame: the strip
/// under the header once a launched loop is running (in the window, once
/// the start has played), and none otherwise. The layout reserves them,
/// so the ticker never covers the transcript or the composer.
pub(crate) fn ticker_rows(app: &App, root: Rect) -> u16 {
    if app.loop_dialog.is_some() || root.width < loop_card::TICKER_MIN_COLS || root.height < 12 {
        return 0;
    }
    let Some(dock) = app.loop_card.dock.as_ref() else {
        return 0;
    };
    if race_of(app, dock.purpose).is_none() {
        return 0;
    }
    let now = Instant::now();
    if let Some((_, at)) = app.loop_card.launch.as_ref()
        && now.saturating_duration_since(*at).as_secs_f32() < LAUNCH_HOLD
    {
        return 0;
    }
    if let Some(at) = dock.finished_at
        && now.saturating_duration_since(at).as_secs_f32() >= COMPLETE_HOLD
    {
        return 0;
    }
    loop_card::TICKER_ROWS
}

/// What keeps out from under the loop card in a terminal; the window draws
/// its card over its pictures. The docked ticker occludes nothing: the
/// layout reserves its rows.
pub(crate) fn occluder(root: Rect, app: &App) -> Option<Rect> {
    if app.loop_dialog.is_none() {
        return None;
    }
    #[cfg(feature = "glass")]
    if crate::ui::glass::active() && loop_card::card_plan(root).is_some() {
        return None;
    }
    Some(loop_card::terminal_area(root))
}

/// This frame's loop card: the open launch, or the start playing out, or
/// the widget in the corner.
pub(crate) fn render(frame: &mut Frame, app: &mut App) {
    let root = frame.area();
    if let Some(dialog) = app.loop_dialog.as_ref() {
        // A new launch outranks the last one's start.
        app.loop_card.launch = None;
        #[cfg(feature = "glass")]
        if crate::ui::glass::active()
            && let Some(plan) = loop_card::card_plan(root)
        {
            let art = crate::ui::glass::cell_pixels()
                .and_then(|cell| loop_card::art(loop_card::art_key(plan, cell)));
            if let Some((card, hits)) =
                loop_card::card(root, dialog, art.as_deref(), loop_card::Phase::Open)
            {
                app.loop_dialog_hits = hits;
                crate::ui::glass::set_card(Some(card));
                return;
            }
        }
        app.loop_dialog_hits = loop_card::render_terminal(frame.buffer_mut(), root, dialog);
        return;
    }
    render_running(frame, app, root);
}

/// The start and the ticker, following the same live counters on both surfaces.
fn render_running(frame: &mut Frame, app: &mut App, root: Rect) {
    let now = Instant::now();
    let race = app
        .loop_card
        .dock
        .as_ref()
        .and_then(|d| race_of(app, d.purpose));
    if race.is_none() {
        // The loop ended or was stopped: the ticker goes with it, and its
        // pictures are handed over again for the next one.
        app.loop_card.dock = None;
        app.loop_card.launch = None;
        app.loop_card.pictures = None;
        return;
    }
    let race = race.expect("checked above");
    let phase = app.loop_card.dock.as_ref().map_or(0.0, |d| {
        now.saturating_duration_since(d.since).as_secs_f32()
    });
    let expired = app.loop_card.dock.as_mut().is_some_and(|dock| {
        dock.paused = race.paused;
        let expired = if race.complete {
            let at = *dock.finished_at.get_or_insert(now);
            now.saturating_duration_since(at).as_secs_f32() >= COMPLETE_HOLD
        } else {
            dock.finished_at = None;
            false
        };
        dock.observe(race.laps_done);
        expired
    });
    if expired {
        app.loop_card.dock = None;
        app.loop_card.launch = None;
        app.loop_card.pictures = None;
        return;
    }
    let area = app
        .panel_frames
        .get(crate::ui::panels::PanelKind::LoopTicker);
    #[cfg(feature = "glass")]
    if let Some(host) = crate::ui::glass::host() {
        render_window(frame, app, root, now, host, race, area, phase);
        return;
    }
    if let Some(area) = area
        && let Some(dock) = app.loop_card.dock.as_mut()
    {
        let s = dock.step(now);
        loop_card::render_terminal_ticker(frame.buffer_mut(), area, &race, s, phase);
    }
}

#[cfg(feature = "glass")]
fn render_window(
    frame: &mut Frame,
    app: &mut App,
    root: Rect,
    now: Instant,
    host: &angel_glass::Host,
    race: Race,
    area: Option<Rect>,
    phase: f32,
) {
    // The start: the flag and the launch on the card; then the card goes
    // and the ticker is in the subheader the layout reserved for it.
    if let Some((dialog, at)) = app.loop_card.launch.as_ref() {
        let t = now.saturating_duration_since(*at).as_secs_f32();
        if t < LAUNCH_HOLD {
            let art = crate::ui::glass::cell_pixels().and_then(|cell| {
                loop_card::card_plan(root)
                    .and_then(|plan| loop_card::art(loop_card::art_key(plan, cell)))
            });
            if let Some((card, _)) =
                loop_card::card(root, dialog, art.as_deref(), loop_card::Phase::Launch)
            {
                crate::ui::glass::set_card(Some(card));
            }
            return;
        }
        app.loop_card.launch = None;
    }

    let Some(area) = area else {
        return;
    };
    let Some(dock) = app.loop_card.dock.as_mut() else {
        return;
    };
    let s = dock.step(now);
    let fade = ((phase - LAUNCH_HOLD) / (DOCKING + DOCK_IN)).clamp(0.0, 1.0);

    // The strip's words and road in cells; the knight as a picture over
    // his slot.
    loop_card::render_ticker(frame.buffer_mut(), area, &race, s, phase, false);
    let Some(cell) = crate::ui::glass::cell_pixels() else {
        return;
    };
    if app.loop_card.pictures != Some(cell) {
        for picture in loop_card::ticker_pictures(cell) {
            host.upload_picture(picture);
        }
        app.loop_card.pictures = Some(cell);
    }
    let layers = loop_card::ticker_layers(&race, area, phase, fade);
    crate::ui::glass::show_group(host, DOCK_GROUP, layers);
}

/// The layer group the widget travels in.
#[cfg(feature = "glass")]
const DOCK_GROUP: u32 = 0x4C4F_4F50;

/// What the widget shows of the loop it follows, read from the loop's own
/// state; none once that loop has ended or been stopped.
fn race_of(app: &App, purpose: LoopLaunchPurpose) -> Option<Race> {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);
    let clock = |deadline: u64, started: u64| {
        (deadline > 0).then(|| {
            (
                deadline.saturating_sub(now_ms.saturating_sub(started) / 1000),
                deadline,
            )
        })
    };
    let fuel =
        |budget: usize, spent: usize| (budget > 0).then(|| (budget.saturating_sub(spent), budget));
    match purpose {
        LoopLaunchPurpose::AgentLoop => {
            let st = &app.loop_ctl;
            if matches!(
                st.status,
                LoopStatus::Idle | LoopStatus::Stopped | LoopStatus::Failed
            ) {
                return None;
            }
            Some(Race {
                laps_done: st.iteration,
                laps: st.max_iters,
                clock: clock(st.deadline_secs, st.started_ms),
                fuel: fuel(st.token_budget, st.tokens_spent),
                rolls: false,
                paused: st.status == LoopStatus::Paused,
                complete: st.status == LoopStatus::Done,
                task: app.loop_task_text(),
            })
        }
        LoopLaunchPurpose::HandoffRl => {
            let h = &app.handoff_rl;
            h.active.then(|| Race {
                laps_done: h.handoff_count,
                laps: h.max_iters,
                clock: clock(h.deadline_secs, h.started_ms),
                fuel: fuel(h.token_budget, h.tokens_spent),
                rolls: true,
                paused: false,
                complete: false,
                task: h.task.clone(),
            })
        }
    }
}

#[cfg(test)]
pub(crate) fn race_for_test(app: &App, purpose: LoopLaunchPurpose) -> Option<Race> {
    race_of(app, purpose)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn loop_widget_runs_one_lap_for_every_completion_including_launch_and_bursts() {
        let now = Instant::now();
        let mut dock = Dock {
            purpose: LoopLaunchPurpose::AgentLoop,
            since: now,
            counted: 0,
            run: None,
            owed: 0,
            paused: false,
            finished_at: None,
        };
        dock.observe(2); // two completions while the flag is still falling
        dock.observe(7); // and a burst while another modal owns the screen
        dock.observe(7); // a redraw cannot add a lap
        assert_eq!(dock.owed, 7);
        for lap in 0..7 {
            let at =
                now + Duration::from_secs_f32(loop_card::LAP_RUN * lap as f32 + 0.01 * lap as f32);
            assert_eq!(dock.step(at), Some(0.0));
            assert_eq!(dock.owed, 6 - lap);
        }
        assert!(dock.step(now + Duration::from_secs(20)).is_none());
        assert_eq!(dock.owed, 0);
    }

    #[test]
    fn loop_widget_is_idle_between_completions() {
        let now = Instant::now();
        let mut dock = Dock {
            purpose: LoopLaunchPurpose::AgentLoop,
            since: now,
            counted: 0,
            run: None,
            owed: 0,
            paused: false,
            finished_at: None,
        };
        for seconds in 0..10 {
            dock.observe(0);
            assert!(dock.step(now + Duration::from_secs(seconds)).is_none());
        }
        dock.observe(1);
        assert_eq!(dock.step(now), Some(0.0));
    }

    #[test]
    fn running_loop_is_a_top_subheader_not_a_bottom_card() {
        let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 30)).unwrap();
        terminal.draw(|f| crate::ui::draw::ui(f, &mut app)).unwrap();
        let idle_transcript = app
            .panel_frames
            .get(crate::ui::panels::PanelKind::Transcript)
            .expect("idle transcript");

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        app.loop_ctl.status = LoopStatus::Running;
        app.loop_ctl.task = "ship the parser".into();
        app.loop_ctl.iteration = 2; // two laps done: the knight is on L03
        app.loop_ctl.max_iters = 0;
        app.loop_ctl.deadline_secs = 263; // 04:23 left
        app.loop_ctl.started_ms = now_ms + 1_000;
        app.loop_card
            .launched(LoopLaunchDialog::new("ship", 0, false), Instant::now());
        terminal.draw(|f| crate::ui::draw::ui(f, &mut app)).unwrap();

        let buf = terminal.backend().buffer();
        let lines: Vec<String> = (0..buf.area.height)
            .map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect())
            .collect();
        let ticker_y = lines
            .iter()
            .position(|line| line.contains("L03"))
            .unwrap_or_else(|| panic!("no L03 ticker row\n{}", lines.join("\n")));
        let transcript = app
            .panel_frames
            .get(crate::ui::panels::PanelKind::Transcript)
            .expect("loop transcript");
        assert!(
            transcript.y > idle_transcript.y,
            "the subheader reserves rows instead of floating: idle {idle_transcript:?}, loop {transcript:?}"
        );
        assert!(
            ticker_y < usize::from(transcript.y),
            "ticker row {ticker_y} must sit above the transcript {transcript:?}\n{}",
            lines.join("\n")
        );
        let ticker = &lines[ticker_y];
        assert!(ticker.contains("04:23"), "{ticker}");
        assert!(ticker.contains('∞'), "endless fuel: {ticker}");
        assert!(
            lines[..=ticker_y].iter().any(|line| line.contains("LOOP")),
            "the tab rides the subheader's top rule\n{}",
            lines.join("\n")
        );
        assert!(
            !lines[15..].iter().any(|line| line.contains("CLOCK")),
            "no bottom loop card remains\n{}",
            lines.join("\n")
        );

        // Flat chrome on a short terminal: the strip still docks under the
        // header and the body still starts below it.
        let mut small = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 18)).unwrap();
        small.draw(|f| crate::ui::draw::ui(f, &mut app)).unwrap();
        let buf = small.backend().buffer();
        let lines: Vec<String> = (0..buf.area.height)
            .map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect())
            .collect();
        let ticker_y = lines
            .iter()
            .position(|line| line.contains("L03"))
            .unwrap_or_else(|| panic!("no L03 ticker row at 80x18\n{}", lines.join("\n")));
        let transcript = app
            .panel_frames
            .get(crate::ui::panels::PanelKind::Transcript)
            .expect("compact transcript");
        assert!(
            ticker_y < usize::from(transcript.y),
            "compact ticker row {ticker_y} above transcript {transcript:?}\n{}",
            lines.join("\n")
        );
    }

    #[test]
    fn loop_widget_leaves_with_the_run_on_the_terminal_too() {
        let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
        app.loop_ctl.status = LoopStatus::Running;
        app.loop_card
            .launched(LoopLaunchDialog::new("ship", 0, false), Instant::now());
        assert!(app.loop_card.docked());
        // The ticker's rows are reserved by the layout: it occludes nothing.
        assert!(occluder(Rect::new(0, 0, 100, 30), &app).is_none());
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 30)).unwrap();
        terminal.draw(|f| render(f, &mut app)).unwrap();
        assert!(app.loop_card.docked());
        app.loop_ctl.status = LoopStatus::Stopped;
        assert!(occluder(Rect::new(0, 0, 100, 30), &app).is_none());
        terminal.draw(|f| render(f, &mut app)).unwrap();
        assert!(!app.loop_card.docked());
        assert!(!app.loop_card.animating(Instant::now()));
    }

    #[test]
    fn a_completed_loop_holds_its_chequers_then_leaves() {
        let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
        app.loop_ctl.status = LoopStatus::Running;
        app.loop_card
            .launched(LoopLaunchDialog::new("ship", 0, false), Instant::now());
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 30)).unwrap();
        terminal.draw(|f| render(f, &mut app)).unwrap();
        app.loop_ctl.status = LoopStatus::Done;
        terminal.draw(|f| render(f, &mut app)).unwrap();
        assert!(app.loop_card.docked(), "complete holds briefly");
        assert_eq!(
            race_for_test(&app, LoopLaunchPurpose::AgentLoop).map(|race| race.complete),
            Some(true)
        );
        if let Some(dock) = app.loop_card.dock.as_mut() {
            dock.finished_at = Some(Instant::now() - Duration::from_secs(7));
        }
        assert_eq!(ticker_rows(&app, Rect::new(0, 0, 100, 30)), 0);
        terminal.draw(|f| render(f, &mut app)).unwrap();
        assert!(!app.loop_card.docked());
    }
}
