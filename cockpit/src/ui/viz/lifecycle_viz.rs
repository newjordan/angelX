//! Deterministic Angel-DMD tourney cut-ins for goal and loop lifecycle events.
//!
//! The renderer is deliberately pure at its public boundary: ceremony kind,
//! label, explicit elapsed time, dimensions, and motion mode fully determine the
//! frame. Generated plates are optional inputs baked into the application; when
//! they are missing or corrupt, procedural arena geometry and the bundled rider
//! dot mask preserve the same event meaning.

use crate::ui::retro_kit;
use crate::ui::term::art::{ColoredBrailleImage, DMD_PALETTE};
use ratatui::{
    style::{Color as TuiColor, Modifier, Style},
    text::{Line, Span, Text},
};
use std::path::{Path, PathBuf};

pub(crate) const FPS: f32 = 12.0;
pub(crate) const FRAME_COUNT: usize = 44;
pub(crate) const STATUS_ROWS: usize = 2;
const DURATION_SECS: f32 = 3.6;
const MAX_CELLS: u16 = 220;
const FULL_SCENE_MIN_WIDTH: usize = 34;
const FULL_SCENE_MIN_ART_HEIGHT: usize = 8;

type Rgb = [u8; 3];
const BLACK: Rgb = DMD_PALETTE[0];
const CYAN: Rgb = DMD_PALETTE[1];
const HUD_BLUE: Rgb = DMD_PALETTE[2];
const STEEL: Rgb = DMD_PALETTE[3];
const PALE: Rgb = DMD_PALETTE[4];
const GOLD: Rgb = DMD_PALETTE[5];
const FAILURE: Rgb = DMD_PALETTE[6];
const SUCCESS: Rgb = DMD_PALETTE[7];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum CeremonyKind {
    GoalSet,
    GoalDone,
    GoalCleared,
    LoopStart,
    LoopPaused,
    LoopDone,
    LoopStopped,
    LoopFailed,
    LoopEscalate,
}

impl CeremonyKind {
    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::GoalSet => " tourney · objective challenged ",
            Self::GoalDone => " tourney · objective crowned ",
            Self::GoalCleared => " tourney · objective banners lowered ",
            Self::LoopStart => " tourney · challenge raised ",
            Self::LoopPaused => " tourney · riders held ",
            Self::LoopDone => " tourney · victory pass ",
            Self::LoopStopped => " tourney · field cleared ",
            Self::LoopFailed => " tourney · lance lost ",
            Self::LoopEscalate => " tourney · grand joust ",
        }
    }

    pub(crate) const fn duration_secs(self) -> f32 {
        DURATION_SECS
    }

    const fn outcome(self) -> Outcome {
        match self {
            Self::GoalSet | Self::LoopStart => Outcome::Challenge,
            Self::GoalDone | Self::LoopDone => Outcome::Victory,
            Self::GoalCleared | Self::LoopPaused | Self::LoopStopped => Outcome::Retreat,
            Self::LoopFailed => Outcome::Failure,
            Self::LoopEscalate => Outcome::Clash,
        }
    }

    /// Location plate staging this ceremony (the MANIFEST storyboard):
    /// arrival at the festival gate, the arena for the grand joust, the
    /// victory lap, and the quiet lists at dusk for failure and retreat.
    const fn backdrop(self) -> &'static str {
        match self.outcome() {
            Outcome::Challenge => "castle_gate.png",
            Outcome::Clash => "arena_dmd.png",
            Outcome::Victory => "victory_lap.png",
            Outcome::Failure | Outcome::Retreat => "lists_dusk.png",
        }
    }

    const fn outcome_word(self) -> &'static str {
        match self.outcome() {
            Outcome::Challenge => "challenge raised",
            Outcome::Clash => "lances crossed",
            Outcome::Victory => "opponent lance broken",
            Outcome::Failure => "player lance broken",
            Outcome::Retreat => "banners lowered",
        }
    }

    const fn work_state(self) -> &'static str {
        match self {
            Self::GoalSet => "Goal set",
            Self::GoalDone => "Goal completed",
            Self::GoalCleared => "Goal cleared",
            Self::LoopStart => "Loop started",
            Self::LoopPaused => "Loop paused",
            Self::LoopDone => "Loop completed",
            Self::LoopStopped => "Loop stopped",
            Self::LoopFailed => "Loop failed",
            Self::LoopEscalate => "Loop escalated",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    Challenge,
    Clash,
    Victory,
    Failure,
    Retreat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MotionMode {
    Full,
    Reduced,
    Off,
}

impl MotionMode {
    pub(crate) fn from_env() -> Self {
        std::env::var("ANGEL_TUI_MOTION")
            .ok()
            .as_deref()
            .map(Self::parse)
            .unwrap_or(Self::Full)
    }

    pub(crate) fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "reduced" => Self::Reduced,
            "off" => Self::Off,
            _ => Self::Full,
        }
    }

    pub(crate) const fn animates(self) -> bool {
        !matches!(self, Self::Off)
    }

    /// Wall time used for chrome clocks. `off` stays at `frozen`; `reduced`
    /// ticks at most once per second.
    pub(crate) fn chrome_elapsed(
        self,
        started: std::time::Instant,
        frozen: std::time::Instant,
    ) -> std::time::Duration {
        match self {
            Self::Off => frozen.saturating_duration_since(started),
            Self::Reduced => std::time::Duration::from_secs(started.elapsed().as_secs()),
            Self::Full => started.elapsed(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Entry,
    Approach,
    Impact,
    Outcome,
    Exit,
}

impl Phase {
    const fn word(self) -> &'static str {
        match self {
            Self::Entry => "dot curtain",
            Self::Approach => "charge",
            Self::Impact => "joust",
            Self::Outcome => "result",
            Self::Exit => "return",
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Timeline {
    frame: usize,
    phase: Phase,
    phase_t: f32,
}

impl Timeline {
    fn at(elapsed_secs: f32, motion: MotionMode) -> Self {
        let elapsed = if elapsed_secs.is_finite() {
            elapsed_secs.max(0.0)
        } else {
            0.0
        };
        let full_frame = ((elapsed * FPS).floor() as usize).min(FRAME_COUNT - 1);
        let frame = match motion {
            MotionMode::Full => full_frame,
            MotionMode::Reduced if elapsed < 0.45 => full_frame.min(5),
            MotionMode::Reduced if elapsed < 3.15 => 32,
            MotionMode::Reduced => full_frame.max(38),
            MotionMode::Off => 32,
        };
        let (phase, first, last) = match frame {
            0..=5 => (Phase::Entry, 0, 5),
            6..=19 => (Phase::Approach, 6, 19),
            20..=25 => (Phase::Impact, 20, 25),
            26..=37 => (Phase::Outcome, 26, 37),
            _ => (Phase::Exit, 38, FRAME_COUNT - 1),
        };
        let phase_t = if first == last {
            1.0
        } else {
            (frame.saturating_sub(first)) as f32 / (last - first) as f32
        };
        Self {
            frame,
            phase,
            phase_t: smoothstep(phase_t),
        }
    }
}

#[derive(Clone, Copy)]
enum Pose {
    Idle,
    Approach,
    Canter,
    Impact,
    Victory,
    Retreat,
}

impl Pose {
    const fn filename(self) -> &'static str {
        match self {
            Self::Idle => "idle.png",
            Self::Approach => "approach.png",
            Self::Canter => "canter.png",
            Self::Impact => "impact.png",
            Self::Victory => "victory.png",
            Self::Retreat => "retreat.png",
        }
    }
}

/// Theatrical loops from the tourney library intake
/// (`assets/tourney/library/sequences`). They are authored at the ceremony's
/// own 12 fps, so the timeline frame indexes sequence frames directly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sequence {
    FlagRaise,
    MaidenWave,
}

impl Sequence {
    const fn dir(self) -> &'static str {
        match self {
            Self::FlagRaise => "flag_raise",
            Self::MaidenWave => "maiden_wave",
        }
    }
    const fn frame_count(self) -> usize {
        match self {
            Self::FlagRaise => 14,
            Self::MaidenWave => 16,
        }
    }
    const fn aspect_x100(self) -> usize {
        match self {
            Self::FlagRaise => 54,
            Self::MaidenWave => 75,
        }
    }
    fn frame_path(self, asset_root: &Path, frame: usize) -> PathBuf {
        asset_root
            .join("library/sequences")
            .join(self.dir())
            .join(format!("{}_{frame:02}.png", self.dir()))
    }
}

#[derive(Clone, Copy)]
struct SequenceCue {
    sequence: Sequence,
    frame: usize,
}

/// Supporting flag/favor beats keep the same red/blue cast on stage. Mounted
/// identities and strike timing are rendered by the shared cast and effects.
fn sequence_cue(kind: CeremonyKind, timeline: Timeline) -> Option<SequenceCue> {
    let sequence = match (kind.outcome(), timeline.phase) {
        (Outcome::Challenge, _) => Sequence::FlagRaise,
        (Outcome::Clash, Phase::Approach) => Sequence::MaidenWave,
        (Outcome::Victory, Phase::Outcome | Phase::Exit) => Sequence::MaidenWave,
        (Outcome::Retreat, Phase::Entry | Phase::Approach) => Sequence::FlagRaise,
        _ => return None,
    };
    Some(SequenceCue {
        sequence,
        frame: timeline.frame % sequence.frame_count(),
    })
}

/// The herald's fanfare opens the ceremony (storyboard beat 4): a keyed
/// character still on the left rail during the entry curtain. The grand joust
/// skips it — the knight's charge already owns that entry — and retreat
/// ceremonies keep their quiet field.
fn draw_herald_fanfare(
    canvas: &mut DotCanvas,
    asset_root: &Path,
    kind: CeremonyKind,
    timeline: Timeline,
) {
    if timeline.phase != Phase::Entry || matches!(kind.outcome(), Outcome::Clash | Outcome::Retreat)
    {
        return;
    }
    let art_h = canvas.cell_height;
    let cell_h = (art_h * 3 / 5).max(4);
    let cell_w = (cell_h * 2).min(canvas.cell_width.saturating_sub(2).max(2));
    // A width clamp on narrow-tall panes re-derives height, keeping the
    // square still square instead of letting it swallow the pane.
    let cell_h = cell_h.min((cell_w / 2).max(1));
    let y = art_h.saturating_sub(cell_h) as i32;
    let path = asset_root.join("library/poses/herald.png");
    if let Some(image) = crate::ui::term::art::colored_image_braille(&path, cell_w, cell_h) {
        canvas.paste_image(&image, 1, y, cell_w, cell_h, false, None);
    }
}

/// Returns true only when the cue's frame actually landed on the canvas.
fn draw_sequence_cue(canvas: &mut DotCanvas, asset_root: &Path, cue: SequenceCue) -> bool {
    let art_w = canvas.cell_width;
    // Supporting cut-in stays in the upper rail; the blue knight owns the
    // right ground line and must remain visible through the whole ceremony.
    let cell_h = (canvas.cell_height / 3).max(2);
    let cell_w =
        (cell_h * cue.sequence.aspect_x100() * 2 / 100).clamp(2, art_w.saturating_sub(2).max(2));
    let cell_h = cell_h.min((cell_w * 100 / (cue.sequence.aspect_x100() * 2)).max(1));
    let path = cue.sequence.frame_path(asset_root, cue.frame);
    let Some(image) = crate::ui::term::art::colored_image_braille(&path, cell_w, cell_h) else {
        return false;
    };
    canvas.paste_image(
        &image,
        art_w.saturating_sub(cell_w + 1) as i32,
        0,
        cell_w,
        cell_h,
        false,
        None,
    );
    true
}

pub(crate) fn render(
    kind: CeremonyKind,
    label: &str,
    elapsed_secs: f32,
    width: u16,
    height: u16,
    motion: MotionMode,
) -> Text<'static> {
    let cell_width = width.min(MAX_CELLS) as usize;
    let cell_height = height.min(MAX_CELLS) as usize;
    if cell_width == 0 || cell_height == 0 {
        return Text::default();
    }
    let timeline = Timeline::at(elapsed_secs, motion);
    let art_height = cell_height.saturating_sub(STATUS_ROWS.min(cell_height));
    if let Some(art) = crate::stage::knight_journey::try_art(
        kind,
        label,
        elapsed_secs,
        cell_width,
        art_height,
        motion,
    ) {
        return finish_text(art, kind, label, timeline, cell_width, cell_height);
    }
    render_with_asset_root(
        kind,
        label,
        elapsed_secs,
        width,
        height,
        motion,
        &default_asset_root(),
    )
}

/// Display-only calibration scene names (`/tourney calibrate <scene>`).
pub(crate) fn calibration_kind(raw: &str) -> Option<CeremonyKind> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "start" => Some(CeremonyKind::LoopStart),
        "joust" => Some(CeremonyKind::LoopEscalate),
        "win" => Some(CeremonyKind::LoopDone),
        "fail" => Some(CeremonyKind::LoopFailed),
        "retreat" => Some(CeremonyKind::LoopStopped),
        // Display-only previews. service/guardian use the same completion
        // kinds as a real ceremony so success art can play; start_lifecycle_ceremony
        // does not mark the goal or loop complete.
        "study" | "dragon" => Some(CeremonyKind::LoopStart),
        "craft" => Some(CeremonyKind::LoopEscalate),
        "perseverance" => Some(CeremonyKind::LoopPaused),
        "service" => Some(CeremonyKind::GoalDone),
        "guardian" => Some(CeremonyKind::LoopDone),
        _ => None,
    }
}

pub(crate) fn warm_assets() {
    crate::stage::knight_journey::warm();
    let root = default_asset_root();
    crate::stage::knight_cast::warm();
    // Only supporting sequences are active now. Do not decode discarded
    // mounted packs, then evict useful frames before the first ceremony.
    for sequence in [Sequence::FlagRaise, Sequence::MaidenWave] {
        for frame in 0..sequence.frame_count() {
            let _ = crate::ui::term::art::preload_colored_image(&sequence.frame_path(&root, frame));
        }
    }
    for plate in [
        "castle_gate.png",
        "arena_dmd.png",
        "victory_lap.png",
        "lists_dusk.png",
    ] {
        let _ =
            crate::ui::term::art::preload_colored_image(&root.join("library/plates").join(plate));
    }
    let _ = crate::ui::term::art::preload_colored_image(&root.join("library/poses/herald.png"));
    for fx in ["pennants.png", "shield_crack.png"] {
        let _ = crate::ui::term::art::preload_colored_image(&root.join("library/fx").join(fx));
    }
    let _ = crate::ui::term::art::preload_colored_image(&root.join("arena-plate.png"));
    for pose in [
        Pose::Idle,
        Pose::Approach,
        Pose::Canter,
        Pose::Impact,
        Pose::Victory,
        Pose::Retreat,
    ] {
        let _ =
            crate::ui::term::art::preload_colored_image(&root.join("poses").join(pose.filename()));
    }
}

fn render_with_asset_root(
    kind: CeremonyKind,
    label: &str,
    elapsed_secs: f32,
    width: u16,
    height: u16,
    motion: MotionMode,
    asset_root: &Path,
) -> Text<'static> {
    let width = width.min(MAX_CELLS) as usize;
    let height = height.min(MAX_CELLS) as usize;
    if width == 0 || height == 0 {
        return Text::default();
    }
    let timeline = Timeline::at(elapsed_secs, motion);
    let art_height = height.saturating_sub(STATUS_ROWS.min(height));
    let mut lines = Vec::with_capacity(height);

    if art_height > 0 {
        let mut canvas = DotCanvas::new(width, art_height);
        if width < FULL_SCENE_MIN_WIDTH || art_height < FULL_SCENE_MIN_ART_HEIGHT {
            draw_compact_pose(&mut canvas, kind);
        } else {
            draw_arena_layer(&mut canvas, asset_root, kind);
            draw_ambient_layer(&mut canvas, timeline);
            if let Some(cue) = sequence_cue(kind, timeline) {
                let _ = draw_sequence_cue(&mut canvas, asset_root, cue);
            }
            // Keep the place legible, with the paired cast in the foreground.
            for color in canvas.dots.iter_mut().flatten() {
                *color = color.map(|channel| (u16::from(channel) * 28 / 100) as u8);
            }
            draw_rider_layer(&mut canvas, asset_root, kind, timeline);
            draw_herald_fanfare(&mut canvas, asset_root, kind, timeline);
            draw_fx_stills(&mut canvas, asset_root, kind, timeline);
            draw_effect_layer(&mut canvas, kind, timeline);
            apply_curtain(&mut canvas, timeline);
        }
        lines.extend(canvas.into_lines());
    }

    finish_text(lines, kind, label, timeline, width, height)
}

pub(crate) fn journey_status_rows(
    kind: CeremonyKind,
    label: &str,
    width: u16,
) -> Option<Text<'static>> {
    crate::stage::knight_journey::scene_for(kind, label)?;
    Some(finish_text(
        Vec::new(),
        kind,
        label,
        Timeline::at(0.0, MotionMode::Off),
        usize::from(width),
        2,
    ))
}

fn finish_text(
    mut lines: Vec<Line<'static>>,
    kind: CeremonyKind,
    label: &str,
    timeline: Timeline,
    width: usize,
    height: usize,
) -> Text<'static> {
    let journey_status = crate::stage::knight_journey::scene_for(kind, label).map(|_| {
        if label.starts_with("calibration · ") {
            "Preview only"
        } else {
            kind.work_state()
        }
    });
    if height >= 2 {
        let status = journey_status
            .map(|status| format!(" {status}"))
            .unwrap_or_else(|| {
                format!(
                    " {} · {} · frame {:02}/{:02}",
                    timeline.phase.word(),
                    kind.outcome_word(),
                    timeline.frame,
                    FRAME_COUNT - 1
                )
            });
        lines.push(fixed_line(
            &status,
            width,
            Style::new()
                .fg(rgb(PALE))
                .bg(rgb(BLACK))
                .add_modifier(Modifier::BOLD),
        ));
        lines.push(fixed_line(
            &format!(" target · {}", one_line(label)),
            width,
            Style::new().fg(rgb(STEEL)).bg(rgb(BLACK)),
        ));
    } else {
        lines.push(fixed_line(
            &journey_status
                .map(|status| format!(" {status}"))
                .unwrap_or_else(|| format!(" {} · {}", timeline.phase.word(), kind.outcome_word())),
            width,
            Style::new().fg(rgb(PALE)).bg(rgb(BLACK)),
        ));
    }
    lines.truncate(height);
    while lines.len() < height {
        lines.push(fixed_line("", width, Style::new().bg(rgb(BLACK))));
    }
    Text::from(lines)
}

fn default_asset_root() -> PathBuf {
    crate::platform::runtime_paths::cockpit_dir().join("assets/tourney")
}

fn draw_arena_layer(canvas: &mut DotCanvas, asset_root: &Path, kind: CeremonyKind) {
    // The ceremony's location plate stages the scene; the legacy arena plate
    // backs any missing location, and geometry backs a missing pack entirely.
    for path in [
        asset_root.join("library/plates").join(kind.backdrop()),
        asset_root.join("arena-plate.png"),
    ] {
        if let Some(arena) = crate::ui::term::art::colored_image_braille(
            &path,
            canvas.cell_width,
            canvas.cell_height,
        ) {
            canvas.paste_image(
                &arena,
                0,
                0,
                canvas.cell_width,
                canvas.cell_height,
                false,
                None,
            );
            return;
        }
    }

    // Missing/corrupt source fallback: sparse rail, stands, and horizon.
    let w = canvas.dot_width as i32;
    let h = canvas.dot_height as i32;
    let rail_y = (h * 3 / 4).max(1);
    canvas.line(0, rail_y, w - 1, rail_y, CYAN);
    canvas.line(0, rail_y + 2, w - 1, rail_y + 2, STEEL);
    for x in (2..w.saturating_sub(2)).step_by(12) {
        canvas.line(x, rail_y - 2, x, rail_y + 4, PALE);
    }
    canvas.line(0, h - 2, w - 1, h - 2, STEEL);
    canvas.line(0, h / 3, w / 5, h / 3, HUD_BLUE);
    canvas.line(w * 4 / 5, h / 3, w - 1, h / 3, HUD_BLUE);
}

fn draw_ambient_layer(canvas: &mut DotCanvas, timeline: Timeline) {
    let w = canvas.dot_width as i32;
    let h = canvas.dot_height as i32;
    let drift = match timeline.phase {
        Phase::Approach | Phase::Impact => (timeline.frame % 3) as i32 - 1,
        _ => 0,
    };
    // A low, ordered-dither haze and a few drifting pinpricks bind the newer
    // DMD ceremony renderer to the cockpit's shared retro art direction.
    // These stay behind the authored arena plate and never obscure the lane.
    for y in 0..h.max(0) {
        let sink = y as f32 / h.max(1) as f32;
        for x in 0..w.max(0) {
            let haze = ((sink - 0.64) / 0.36).clamp(0.0, 1.0).powi(2) * 0.12;
            if retro_kit::dither(x as usize + timeline.frame / 6, y as usize, haze) {
                let ink = retro_kit::MOONLIT.banded(0.18 + sink * 0.22, 5);
                canvas.set(x, y, [ink.r, ink.g, ink.b]);
            } else if sink < 0.58
                && retro_kit::hash01(x + (timeline.frame / 2) as i32, y, 0x5109) > 0.997
            {
                canvas.set(x, y, PALE);
            }
        }
    }
    // Subdued parallax pennants live high and outside the jousting lane.
    for (index, anchor) in [w / 6, w / 3, w * 2 / 3, w * 5 / 6].into_iter().enumerate() {
        let top = 2 + (index % 2) as i32;
        canvas.line(anchor, top, anchor, (h / 4).max(top + 2), STEEL);
        let tip = anchor
            + if index % 2 == 0 {
                4 + drift
            } else {
                -4 - drift
            };
        canvas.line(
            anchor,
            top + 1,
            tip,
            top + 2,
            if index % 2 == 0 { GOLD } else { HUD_BLUE },
        );
        canvas.line(tip, top + 2, anchor, top + 3, STEEL);
    }
    // Deterministic, low-density crowd glints; never a generic particle field.
    let crowd_y = (h * 2 / 3).max(1);
    for x in (3..w.saturating_sub(2)).step_by(9) {
        if ((x as usize + timeline.frame) / 3).is_multiple_of(2) {
            canvas.set(x, crowd_y + (x % 3), STEEL);
        }
    }
}

fn draw_rider_layer(
    canvas: &mut DotCanvas,
    asset_root: &Path,
    kind: CeremonyKind,
    timeline: Timeline,
) {
    let cell_w = canvas.cell_width;
    let art_h = canvas.cell_height;
    let rider_w = (cell_w / 4)
        .clamp(9, 24)
        .min(cell_w.saturating_sub(2).max(1));
    let rider_h = (art_h * 3 / 4).clamp(5, 14).min(art_h.max(1));
    let contact = cell_w as f32 * 0.5;
    let impact_player = contact - rider_w as f32 + 1.0;
    let impact_opponent = contact - 1.0;
    let (mut player_x, mut opponent_x) = match timeline.phase {
        Phase::Entry => {
            let t = timeline.phase_t;
            (
                lerp(-(rider_w as f32) * 0.75, cell_w as f32 * 0.12, t),
                lerp(
                    cell_w as f32 - rider_w as f32 * 0.25,
                    cell_w as f32 * 0.68,
                    t,
                ),
            )
        }
        Phase::Approach => (
            lerp(cell_w as f32 * 0.10, impact_player, timeline.phase_t),
            lerp(cell_w as f32 * 0.70, impact_opponent, timeline.phase_t),
        ),
        Phase::Impact => (
            impact_player - (1.0 - timeline.phase_t) * 0.7,
            impact_opponent + (1.0 - timeline.phase_t) * 0.7,
        ),
        Phase::Outcome => outcome_positions(
            kind.outcome(),
            impact_player,
            impact_opponent,
            rider_w,
            timeline.phase_t,
        ),
        Phase::Exit => (
            lerp(impact_player, -(rider_w as f32), timeline.phase_t),
            lerp(
                impact_opponent,
                cell_w as f32 + rider_w as f32 * 0.2,
                timeline.phase_t,
            ),
        ),
    };
    if matches!(kind.outcome(), Outcome::Challenge) && timeline.phase == Phase::Outcome {
        player_x = cell_w as f32 * 0.18;
        opponent_x = cell_w as f32 * 0.62;
    }
    if matches!(kind.outcome(), Outcome::Retreat) && timeline.phase == Phase::Outcome {
        player_x -= timeline.phase_t * rider_w as f32 * 0.4;
        opponent_x += timeline.phase_t * rider_w as f32 * 0.4;
    }

    let bob = if timeline.phase == Phase::Approach && timeline.frame.is_multiple_of(2) {
        -1
    } else {
        0
    };
    let base_y = art_h.saturating_sub(rider_h + 1) as i32 + bob;
    let (player_pose, opponent_pose) = poses_for(kind.outcome(), timeline);
    draw_rider(
        canvas,
        asset_root,
        player_pose,
        player_x.round() as i32,
        base_y,
        rider_w,
        rider_h,
        true,
        [172, 54, 48],
        GOLD,
    );
    draw_rider(
        canvas,
        asset_root,
        opponent_pose,
        opponent_x.round() as i32,
        base_y,
        rider_w,
        rider_h,
        false,
        HUD_BLUE,
        if kind.outcome() == Outcome::Failure {
            SUCCESS
        } else {
            STEEL
        },
    );

    draw_lances(
        canvas,
        kind.outcome(),
        timeline,
        player_x,
        opponent_x,
        base_y as f32,
        rider_w as f32,
        rider_h as f32,
    );
}

fn outcome_positions(
    outcome: Outcome,
    player: f32,
    opponent: f32,
    rider_w: usize,
    t: f32,
) -> (f32, f32) {
    match outcome {
        Outcome::Victory => (player - t, opponent + t * rider_w as f32 * 0.55),
        Outcome::Failure => (player - t * rider_w as f32 * 0.55, opponent + t),
        Outcome::Retreat => (player - t * 2.0, opponent + t * 2.0),
        Outcome::Clash => (player - t * 1.5, opponent + t * 1.5),
        Outcome::Challenge => (player, opponent),
    }
}

fn poses_for(outcome: Outcome, timeline: Timeline) -> (Pose, Pose) {
    match timeline.phase {
        Phase::Entry => (Pose::Idle, Pose::Idle),
        Phase::Approach if timeline.frame.is_multiple_of(3) => (Pose::Approach, Pose::Approach),
        Phase::Approach => (Pose::Canter, Pose::Canter),
        Phase::Impact => (Pose::Impact, Pose::Impact),
        Phase::Exit => (Pose::Retreat, Pose::Retreat),
        Phase::Outcome => match outcome {
            Outcome::Victory => (Pose::Victory, Pose::Impact),
            Outcome::Failure => (Pose::Impact, Pose::Victory),
            Outcome::Retreat => (Pose::Retreat, Pose::Retreat),
            Outcome::Challenge => (Pose::Idle, Pose::Idle),
            Outcome::Clash => (Pose::Impact, Pose::Impact),
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_rider(
    canvas: &mut DotCanvas,
    asset_root: &Path,
    pose: Pose,
    x: i32,
    y: i32,
    width: usize,
    height: usize,
    mirror: bool,
    base: Rgb,
    accent: Rgb,
) {
    use crate::stage::knight_cast::{Pose as CastPose, Side};
    let side = if mirror {
        Side::RedLeft
    } else {
        Side::BlueRight
    };
    let cast_pose = match pose {
        Pose::Approach => CastPose::StepA,
        Pose::Canter | Pose::Retreat => CastPose::StepB,
        _ => CastPose::Mounted,
    };
    if let Some(sprite) = crate::stage::knight_cast::sprite(side, cast_pose) {
        let region = crate::ui::term::art::ImageRegion {
            x: 0,
            y: 0,
            width: sprite.width(),
            height: sprite.height(),
        };
        let stem = 0x4341_5354_0000 + (side as u64) * 16 + cast_pose as u64;
        // Fit both axes together instead of stretching a square horse across
        // a wide action lane. The ground remains shared by both companions.
        let fit = (width as f32 * 2.0 / sprite.width() as f32)
            .min(height as f32 * 4.0 / sprite.height() as f32);
        let w = ((sprite.width() as f32 * fit / 2.0).floor() as usize).max(1);
        let h = ((sprite.height() as f32 * fit / 4.0).floor() as usize).max(1);
        if let Some(image) =
            crate::ui::term::art::colored_rgba_region_braille(sprite, stem, region, w, h, false)
        {
            let cell_x = x + (width.saturating_sub(w) / 2) as i32;
            let cell_y = y + height.saturating_sub(h) as i32;
            let retreat = matches!(pose, Pose::Retreat);
            canvas.erase_silhouette(sprite, cell_x, cell_y, w, h, retreat);
            canvas.paste_image(&image, cell_x, cell_y, w, h, retreat, None);
            return;
        }
    }
    let path = asset_root.join("poses").join(pose.filename());
    if let Some(image) = crate::ui::term::art::colored_image_braille(&path, width, height) {
        canvas.paste_image(&image, x, y, width, height, mirror, Some((base, accent)));
    } else {
        canvas.draw_mask(
            RIDER_DOT_MASK,
            x * 2,
            y * 4,
            width * 2,
            height * 4,
            mirror,
            base,
            accent,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_lances(
    canvas: &mut DotCanvas,
    outcome: Outcome,
    timeline: Timeline,
    player_x: f32,
    opponent_x: f32,
    rider_y: f32,
    rider_w: f32,
    rider_h: f32,
) {
    let contact_x = canvas.dot_width as i32 / 2;
    let contact_y = ((rider_y + rider_h * 0.38) * 4.0) as i32;
    let player_grip = (
        ((player_x + rider_w * 0.58) * 2.0) as i32,
        ((rider_y + rider_h * 0.38) * 4.0) as i32,
    );
    let opponent_grip = (
        ((opponent_x + rider_w * 0.42) * 2.0) as i32,
        ((rider_y + rider_h * 0.38) * 4.0) as i32,
    );

    if timeline.phase == Phase::Outcome {
        match outcome {
            Outcome::Challenge => {
                let pole_x = player_grip.0;
                canvas.line(
                    pole_x,
                    player_grip.1,
                    pole_x,
                    (rider_y * 4.0) as i32 - 5,
                    GOLD,
                );
                let top = (rider_y * 4.0) as i32 - 5;
                canvas.line(pole_x, top, pole_x + 9, top + 3, CYAN);
                canvas.line(pole_x + 9, top + 3, pole_x, top + 6, HUD_BLUE);
            }
            Outcome::Victory => {
                canvas.line(
                    player_grip.0,
                    player_grip.1,
                    contact_x + 2,
                    contact_y - 5,
                    GOLD,
                );
                canvas.line(
                    opponent_grip.0,
                    opponent_grip.1,
                    contact_x + 4,
                    contact_y + 2,
                    STEEL,
                );
                canvas.line(
                    contact_x + 1,
                    contact_y + 1,
                    contact_x - 5,
                    contact_y + 6,
                    FAILURE,
                );
            }
            Outcome::Failure => {
                canvas.line(
                    opponent_grip.0,
                    opponent_grip.1,
                    contact_x - 2,
                    contact_y - 5,
                    STEEL,
                );
                canvas.line(
                    player_grip.0,
                    player_grip.1,
                    contact_x - 4,
                    contact_y + 2,
                    GOLD,
                );
                canvas.line(
                    contact_x - 1,
                    contact_y + 1,
                    contact_x + 5,
                    contact_y + 6,
                    FAILURE,
                );
            }
            Outcome::Retreat => {
                canvas.line(
                    player_grip.0,
                    player_grip.1,
                    player_grip.0 - 8,
                    player_grip.1 + 7,
                    STEEL,
                );
                canvas.line(
                    opponent_grip.0,
                    opponent_grip.1,
                    opponent_grip.0 + 8,
                    opponent_grip.1 + 7,
                    STEEL,
                );
            }
            Outcome::Clash => {
                canvas.line(player_grip.0, player_grip.1, contact_x + 3, contact_y, GOLD);
                canvas.line(
                    opponent_grip.0,
                    opponent_grip.1,
                    contact_x - 3,
                    contact_y,
                    PALE,
                );
            }
        }
        return;
    }

    if matches!(timeline.phase, Phase::Approach | Phase::Impact) {
        canvas.line(player_grip.0, player_grip.1, contact_x + 2, contact_y, GOLD);
        canvas.line(
            opponent_grip.0,
            opponent_grip.1,
            contact_x - 2,
            contact_y,
            PALE,
        );
    } else if timeline.phase == Phase::Exit {
        canvas.line(
            player_grip.0,
            player_grip.1,
            player_grip.0 - 7,
            player_grip.1 + 6,
            STEEL,
        );
        canvas.line(
            opponent_grip.0,
            opponent_grip.1,
            opponent_grip.0 + 7,
            opponent_grip.1 + 6,
            STEEL,
        );
    }
}

/// Authored FX stills from `library/fx` — pennants, shield crack, etc.
/// Drawn as a small right/left inset so they read as gift-bag animation bits
/// without covering the knight sequence.
fn draw_fx_stills(
    canvas: &mut DotCanvas,
    asset_root: &Path,
    kind: CeremonyKind,
    timeline: Timeline,
) {
    let name = match (kind.outcome(), timeline.phase) {
        (Outcome::Challenge, Phase::Entry | Phase::Approach) => Some("pennants.png"),
        (Outcome::Clash, Phase::Impact) => Some("shield_crack.png"),
        (Outcome::Failure, Phase::Outcome | Phase::Exit) => Some("shield_crack.png"),
        (Outcome::Victory, Phase::Outcome | Phase::Exit) => Some("pennants.png"),
        _ => None,
    };
    let Some(name) = name else {
        return;
    };
    let path = asset_root.join("library/fx").join(name);
    let art_w = canvas.cell_width;
    let art_h = canvas.cell_height;
    let cell_h = (art_h / 3).max(3).min(art_h.saturating_sub(1).max(1));
    let cell_w = cell_h.clamp(3, art_w.saturating_sub(2).max(2));
    let x = match kind.outcome() {
        Outcome::Victory | Outcome::Challenge => 1i32,
        _ => art_w.saturating_sub(cell_w + 1) as i32,
    };
    let y = 1i32;
    if let Some(image) = crate::ui::term::art::colored_image_braille(&path, cell_w, cell_h) {
        canvas.paste_image(&image, x, y, cell_w, cell_h, false, None);
    }
}

fn draw_effect_layer(canvas: &mut DotCanvas, kind: CeremonyKind, timeline: Timeline) {
    let cx = canvas.dot_width as i32 / 2;
    let cy = canvas.dot_height as i32 * 5 / 9;
    if timeline.phase == Phase::Impact {
        let radius = 2 + (timeline.phase_t * 8.0).round() as i32;
        let color = match kind.outcome() {
            Outcome::Failure => FAILURE,
            Outcome::Victory => SUCCESS,
            _ => GOLD,
        };
        for (dx, dy) in [
            (1, 0),
            (-1, 0),
            (0, 1),
            (0, -1),
            (1, 1),
            (-1, 1),
            (1, -1),
            (-1, -1),
        ] {
            canvas.line(
                cx + dx * 2,
                cy + dy,
                cx + dx * radius,
                cy + dy * radius,
                color,
            );
        }
    }
    if timeline.phase == Phase::Outcome
        && matches!(
            kind.outcome(),
            Outcome::Victory | Outcome::Failure | Outcome::Clash
        )
    {
        let color = if kind.outcome() == Outcome::Failure {
            FAILURE
        } else {
            GOLD
        };
        for i in 0..9 {
            let hash = timeline.frame.wrapping_mul(31).wrapping_add(i * 17);
            let x = cx + hash as i32 % 25 - 12;
            let y = cy + (hash / 7) as i32 % 11 - 5;
            canvas.set(x, y, color);
        }
    }
}

fn apply_curtain(canvas: &mut DotCanvas, timeline: Timeline) {
    match timeline.phase {
        Phase::Entry => {
            let edge = timeline.phase_t * canvas.dot_width as f32;
            canvas.retain(|x, y| x as f32 <= edge + f32::from(BAYER4[y % 4][x % 4]) * 0.35);
        }
        Phase::Exit => {
            let edge = timeline.phase_t * canvas.dot_width as f32;
            canvas.retain(|x, y| x as f32 + f32::from(BAYER4[y % 4][x % 4]) * 0.3 >= edge);
        }
        _ => {}
    }
}

fn draw_compact_pose(canvas: &mut DotCanvas, kind: CeremonyKind) {
    let w = canvas.dot_width as i32;
    let h = canvas.dot_height as i32;
    let cx = w / 2;
    let cy = h / 2;
    let color = match kind.outcome() {
        Outcome::Victory => SUCCESS,
        Outcome::Failure => FAILURE,
        Outcome::Challenge => GOLD,
        Outcome::Clash => PALE,
        Outcome::Retreat => STEEL,
    };
    if kind.outcome() == Outcome::Challenge {
        canvas.line(cx - 3, h - 2, cx - 3, 1, GOLD);
        canvas.line(cx - 3, 1, cx + 5, cy, CYAN);
        canvas.line(cx + 5, cy, cx - 3, cy + 2, HUD_BLUE);
    } else {
        canvas.line(1, h.saturating_sub(2), w.saturating_sub(2), 1, color);
        canvas.line(1, 1, w.saturating_sub(2), h.saturating_sub(2), color);
        canvas.set(cx, cy, PALE);
    }
}

const BAYER4: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

// Hand-corrected fallback dot silhouette: helmet/rider over a four-legged horse.
// It is bundled into the binary and remains readable even when every PNG is gone.
const RIDER_DOT_MASK: &[&str] = &[
    "...........cc........",
    "..........cccc.......",
    "...........cc........",
    ".........ccccgg......",
    "....ccccccccgggg.....",
    "..ccccccccccccggcc...",
    ".cccccccccccccccccc..",
    "cccccccccccccccccccc.",
    "..cccccccccccccccc...",
    "....cc..cc....cc.....",
    "...cc...cc....cc.....",
    "..cc....cc.....cc....",
];

struct DotCanvas {
    cell_width: usize,
    cell_height: usize,
    dot_width: usize,
    dot_height: usize,
    dots: Vec<Option<Rgb>>,
}

impl DotCanvas {
    fn new(cell_width: usize, cell_height: usize) -> Self {
        let dot_width = cell_width * 2;
        let dot_height = cell_height * 4;
        Self {
            cell_width,
            cell_height,
            dot_width,
            dot_height,
            dots: vec![None; dot_width * dot_height],
        }
    }

    fn set(&mut self, x: i32, y: i32, color: Rgb) {
        if x < 0 || y < 0 {
            return;
        }
        let (x, y) = (x as usize, y as usize);
        if x < self.dot_width && y < self.dot_height {
            self.dots[y * self.dot_width + x] = Some(color);
        }
    }

    fn line(&mut self, mut x0: i32, mut y0: i32, x1: i32, y1: i32, color: Rgb) {
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut error = dx + dy;
        loop {
            self.set(x0, y0, color);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let twice = error * 2;
            if twice >= dy {
                error += dy;
                x0 += sx;
            }
            if twice <= dx {
                error += dx;
                y0 += sy;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn erase_silhouette(
        &mut self,
        source: &image::RgbaImage,
        cell_x: i32,
        cell_y: i32,
        width: usize,
        height: usize,
        mirror: bool,
    ) {
        // Match the proportional nearest-neighbor fit used by terminal_art.
        // Opaque black armor must occlude the backdrop even when it emits no dot.
        let (dw, dh) = (width * 2, height * 4);
        let fit = (dw as f32 / source.width() as f32).min(dh as f32 / source.height() as f32);
        let sw = ((source.width() as f32 * fit).round() as usize).clamp(1, dw);
        let sh = ((source.height() as f32 * fit).round() as usize).clamp(1, dh);
        let (ox, oy) = ((dw - sw) / 2, (dh - sh) / 2);
        for dy in 0..sh {
            for dx in 0..sw {
                let sx = ((dx as f32 + 0.5) * source.width() as f32 / sw as f32) as u32;
                let sy = ((dy as f32 + 0.5) * source.height() as f32 / sh as f32) as u32;
                if source.get_pixel(sx.min(source.width() - 1), sy.min(source.height() - 1))[3] < 96
                {
                    continue;
                }
                let px = if mirror { dw - 1 - (ox + dx) } else { ox + dx };
                let x = cell_x * 2 + px as i32;
                let y = cell_y * 4 + (oy + dy) as i32;
                if x >= 0 && y >= 0 && x < self.dot_width as i32 && y < self.dot_height as i32 {
                    self.dots[y as usize * self.dot_width + x as usize] = None;
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn paste_image(
        &mut self,
        image: &ColoredBrailleImage,
        cell_x: i32,
        cell_y: i32,
        target_width: usize,
        target_height: usize,
        mirror: bool,
        tint: Option<(Rgb, Rgb)>,
    ) {
        if target_width == 0 || target_height == 0 {
            return;
        }
        let src_dot_w = image.width * 2;
        let src_dot_h = image.height * 4;
        let target_dot_w = target_width * 2;
        let target_dot_h = target_height * 4;
        for dy in 0..target_dot_h {
            let sy = (dy * src_dot_h / target_dot_h).min(src_dot_h.saturating_sub(1));
            for dx in 0..target_dot_w {
                let mut sx = (dx * src_dot_w / target_dot_w).min(src_dot_w.saturating_sub(1));
                if mirror {
                    sx = src_dot_w.saturating_sub(1).saturating_sub(sx);
                }
                let Some(cell) = image.cell(sx / 2, sy / 4) else {
                    continue;
                };
                let bit = crate::ui::term::art::braille_dot_bit(sx % 2, sy % 4);
                if crate::ui::term::art::braille_bits(cell.glyph) & bit == 0 {
                    continue;
                }
                let color =
                    tint.map_or(cell.fg, |(base, accent)| tint_color(cell.fg, base, accent));
                self.set(cell_x * 2 + dx as i32, cell_y * 4 + dy as i32, color);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_mask(
        &mut self,
        mask: &[&str],
        x: i32,
        y: i32,
        width: usize,
        height: usize,
        mirror: bool,
        base: Rgb,
        accent: Rgb,
    ) {
        let source_h = mask.len().max(1);
        let source_w = mask.iter().map(|row| row.len()).max().unwrap_or(1).max(1);
        for dy in 0..height {
            let sy = (dy * source_h / height.max(1)).min(source_h - 1);
            let row = mask[sy].as_bytes();
            for dx in 0..width {
                let sample_x = dx * source_w / width.max(1);
                let sx = if mirror {
                    source_w - 1 - sample_x
                } else {
                    sample_x
                };
                let mark = row.get(sx).copied().unwrap_or(b'.');
                let color = match mark {
                    b'c' => Some(base),
                    b'g' => Some(accent),
                    _ => None,
                };
                if let Some(color) = color {
                    self.set(x + dx as i32, y + dy as i32, color);
                }
            }
        }
    }

    fn retain(&mut self, mut keep: impl FnMut(usize, usize) -> bool) {
        for y in 0..self.dot_height {
            for x in 0..self.dot_width {
                if !keep(x, y) {
                    self.dots[y * self.dot_width + x] = None;
                }
            }
        }
    }

    fn into_lines(self) -> Vec<Line<'static>> {
        let mut lines = Vec::with_capacity(self.cell_height);
        for cell_y in 0..self.cell_height {
            let mut spans = Vec::with_capacity(self.cell_width);
            for cell_x in 0..self.cell_width {
                let mut bits = 0u8;
                let mut votes = [(BLACK, 0u8); 8];
                let mut used = 0usize;
                for local_y in 0..4 {
                    for local_x in 0..2 {
                        let x = cell_x * 2 + local_x;
                        let y = cell_y * 4 + local_y;
                        if let Some(color) = self.dots[y * self.dot_width + x] {
                            bits |= crate::ui::term::art::braille_dot_bit(local_x, local_y);
                            let index = votes[..used]
                                .iter()
                                .position(|(c, _)| *c == color)
                                .unwrap_or_else(|| {
                                    let index = used;
                                    votes[index].0 = color;
                                    used += 1;
                                    index
                                });
                            votes[index].1 += 1;
                        }
                    }
                }
                let color = votes[..used]
                    .iter()
                    .max_by_key(|(color, count)| {
                        (
                            *count,
                            DMD_PALETTE.iter().position(|c| c == color).unwrap_or(0),
                            *color,
                        )
                    })
                    .map_or(BLACK, |(color, _)| *color);
                spans.push(Span::styled(
                    crate::ui::term::art::braille_char(bits).to_string(),
                    Style::new().fg(rgb(color)).bg(rgb(BLACK)),
                ));
            }
            lines.push(Line::from(spans));
        }
        lines
    }
}

fn tint_color(source: Rgb, base: Rgb, accent: Rgb) -> Rgb {
    match source {
        GOLD => accent,
        PALE | CYAN => base,
        HUD_BLUE | STEEL => STEEL,
        SUCCESS => SUCCESS,
        FAILURE => FAILURE,
        _ => base,
    }
}

fn rgb([r, g, b]: Rgb) -> TuiColor {
    TuiColor::Rgb(r, g, b)
}

fn fixed_line(raw: &str, width: usize, style: Style) -> Line<'static> {
    let mut text = raw.chars().take(width).collect::<String>();
    let count = text.chars().count();
    if count < width {
        text.push_str(&" ".repeat(width - count));
    }
    Line::from(Span::styled(text, style))
}

fn one_line(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/lifecycle_viz__tests.rs"]
mod tests;
