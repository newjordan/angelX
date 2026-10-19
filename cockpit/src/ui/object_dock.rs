//! Display-only mini-viz object dock. One layout owns text, hit targets and
//! Glass layers; routes remain the existing WorldButton actions.
use std::time::{Duration, Instant};

use super::viz::lifecycle_viz::MotionMode;
use crate::app::{App, WorldButton};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::Paragraph,
};

const REVEAL: Duration = Duration::from_millis(640);
const TILE: u16 = 9;
/// The narrowest pane the dock floats its solids from (three to a page); a
/// narrower one keeps its row of bracketed controls, so Back is never pages
/// out of reach.
pub(crate) const MIN_WIDTH: u16 = 40;

#[derive(Debug)]
pub(crate) struct ObjectDock {
    pub(crate) open: bool,
    pub(crate) visible: bool,
    pub(crate) page: usize,
    from: f32,
    changed: Instant,
    epoch: Instant,
    exploring: bool,
    /// The actions of the row the dock stands in for, in the dock's order
    /// (an arrival floats Enter, Explore and Back; the world all seven).
    offered: Vec<(&'static str, WorldButton)>,
    objects: Vec<FloatingObject>,
    pub(crate) area: Rect,
    pub(crate) tiles: Vec<(Rect, WorldButton)>,
}

impl Default for ObjectDock {
    fn default() -> Self {
        Self {
            open: false,
            visible: false,
            page: 0,
            from: 0.0,
            changed: Instant::now(),
            epoch: Instant::now(),
            exploring: false,
            offered: ACTIONS.to_vec(),
            objects: Vec::new(),
            area: Rect::default(),
            tiles: Vec::new(),
        }
    }
}

impl ObjectDock {
    pub(crate) fn progress(&self, now: Instant, motion: MotionMode) -> f32 {
        let target = f32::from(self.open);
        if motion != MotionMode::Full {
            return target;
        }
        let t = (now.saturating_duration_since(self.changed).as_secs_f32() / REVEAL.as_secs_f32())
            .clamp(0.0, 1.0);
        // Ease-out cubic; reversal starts at the current displayed extent.
        let ease = 1.0 - (1.0 - t).powi(3);
        self.from + (target - self.from) * ease
    }

    pub(crate) fn toggle(&mut self, now: Instant, motion: MotionMode) {
        self.from = self.progress(now, motion);
        self.open = !self.open;
        self.changed = now;
    }

    /// Open and already settled, as if the reveal had run: for tests that
    /// look at the floated actions without waiting out the motion.
    #[cfg(test)]
    pub(crate) fn open_settled(&mut self) {
        self.open = true;
        self.from = 1.0;
        self.changed = Instant::now() - REVEAL;
    }

    /// Whether the dock is up and carries `action` on any of its pages.
    #[cfg(test)]
    pub(crate) fn offers(&self, action: WorldButton) -> bool {
        self.visible && self.offered.iter().any(|(_, offered)| *offered == action)
    }

    pub(crate) fn animating(&self, now: Instant, motion: MotionMode) -> bool {
        motion == MotionMode::Full
            && (self.progress(now, motion) - f32::from(self.open)).abs() > 0.001
    }

    /// A visible object, not the empty space between objects, owns the pointer.
    pub(crate) fn hit(&self, x: u16, y: u16) -> Option<WorldButton> {
        if !self.visible { return None; }
        self.tiles.iter().find(|(rect, _)| {
            x >= rect.x && x < rect.right() && y >= rect.y && y < rect.bottom()
        }).map(|(_, button)| *button)
    }

    fn poses(&self, now: Instant, motion: MotionMode, screen: Rect) -> Vec<FloatingObject> {
        let progress = self.progress(now, motion);
        if progress <= 0.001 || self.area.width < MIN_WIDTH || screen.height < 3 {
            return Vec::new();
        }
        let (page, slots, rects) = layout(self.area, self.offered.len(), self.page);
        let time = if motion == MotionMode::Full {
            now.saturating_duration_since(self.epoch).as_secs_f32()
        } else { 0.0 };
        let count = rects.len();
        // Fan outward only into available terminal space. No pane is resized.
        let left_room = f32::from(self.area.x.saturating_sub(screen.x)).min(5.0);
        let right_room = f32::from(screen.right().saturating_sub(self.area.right())).min(5.0);
        rects.into_iter().enumerate().map(|(i, rect)| {
            let (mut label, button) = self.offered[page * slots + i];
            if button == WorldButton::ScryglassMap && self.exploring { label = "Map"; }
            let shape = primitive_index(button).unwrap();
            let phase = shape as f32 * 1.71;
            // Slightly different arrival curves, heights and slow periods avoid
            // a rigid row or a single drawer-like moving edge. A pure function
            // of progress keeps mid-flight reversals position-continuous.
            let p = progress.powf(1.0 + (shape % 3) as f32 * 0.22);
            let moving = motion == MotionMode::Full;
            let sway = if moving { (time * (0.31 + shape as f32 * 0.017) + phase).sin() * 0.35 } else { 0.0 };
            let bob = if moving { (time * (0.47 + shape as f32 * 0.013) + phase).sin() * 0.32 } else { 0.0 };
            let w = (f32::from(rect.width) - 1.6).max(1.0) * (0.65 + 0.35 * p);
            let h = (f32::from(self.area.height).min(5.0) - 0.8).max(1.0)
                * (0.55 + 0.45 * p);
            let start_x = f32::from(rect.x) + (f32::from(rect.width) - w) * 0.5;
            let fraction = if count == 1 { 0.5 } else { i as f32 / (count - 1) as f32 };
            let destination_x = f32::from(self.area.x) - left_room
                + (f32::from(self.area.width) + left_room + right_room - w) * fraction;
            let x = (start_x + (destination_x - start_x + sway) * p)
                .clamp(f32::from(screen.x), (f32::from(screen.right()) - w).max(f32::from(screen.x)));
            let lift = 2.4 + (phase.cos() + 1.0) * 1.25;
            let y = (f32::from(self.area.bottom()) - 1.0 - h - (lift + bob) * p)
                .clamp(f32::from(screen.y), (f32::from(screen.bottom()) - h - 1.0).max(f32::from(screen.y)));
            let bounds = [x, y, w, h];
            let object = cell_envelope(bounds).intersection(screen);
            let label_width = (label.len() as u16).min(screen.width);
            let label_x = ((x + w * 0.5).round() as u16).saturating_sub(label_width / 2)
                .clamp(screen.x, screen.right().saturating_sub(label_width));
            let label_rect = Rect::new(label_x, ((y + h).ceil() as u16).min(screen.bottom() - 1), label_width, 1);
            FloatingObject { button, label, bounds, label_rect,
                hit: object.union(label_rect).intersection(screen), opacity: p }
        }).collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
struct FloatingObject {
    button: WorldButton,
    label: &'static str,
    bounds: [f32; 4],
    label_rect: Rect,
    hit: Rect,
    opacity: f32,
}

fn cell_envelope([x, y, w, h]: [f32; 4]) -> Rect {
    let left = x.floor() as u16;
    let top = y.floor() as u16;
    Rect::new(left, top, (x + w).ceil() as u16 - left, (y + h).ceil() as u16 - top)
}

const ACTIONS: [(&str, WorldButton); 7] = [
    ("Enter", WorldButton::ScryglassEnter),
    ("Explore", WorldButton::ScryglassMap),
    ("Tower", WorldButton::Tower),
    ("Assets", WorldButton::Assets),
    ("Library", WorldButton::ScryglassLibrary),
    ("Vault", WorldButton::ScryglassVault),
    ("Back", WorldButton::Back),
];

// Ice, teal, amethyst, amber, periwinkle, jade, rose.
fn accent(button: WorldButton) -> [u8; 3] {
    match button {
        WorldButton::ScryglassEnter => [111, 220, 255],
        WorldButton::ScryglassMap => [67, 230, 208],
        WorldButton::Tower => [190, 153, 255],
        WorldButton::Assets => [255, 197, 111],
        WorldButton::ScryglassLibrary => [146, 178, 255],
        WorldButton::ScryglassVault => [112, 231, 170],
        WorldButton::Back => [241, 151, 180],
        _ => [111, 220, 255],
    }
}

/// Constant-size slots and explicit paging keep *all seven* actions reachable,
/// rather than silently dropping the last actions at narrow breakpoints.
fn layout(area: Rect, count: usize, page: usize) -> (usize, usize, Vec<Rect>) {
    let slots = ((area.width.saturating_sub(8) / TILE) as usize)
        .max(1)
        .min(count);
    let pages = count.div_ceil(slots);
    let page = page.min(pages - 1);
    let n = slots.min(count - page * slots);
    let tile_width = TILE.min(area.width.saturating_sub(8));
    let width = tile_width * n as u16;
    let x = area.x + 4 + area.width.saturating_sub(8 + width) / 2;
    let tiles = (0..n)
        .map(|i| Rect::new(x + i as u16 * tile_width, area.y, tile_width, area.height))
        .collect();
    (page, slots, tiles)
}

/// Seven distinct solids, ray-marched once and reused by both display paths.
/// The light and camera are fixed: reduced/off motion has identical geometry.
fn primitive_index(button: WorldButton) -> Option<usize> {
    Some(match button {
        WorldButton::ScryglassEnter => 0, // cube
        WorldButton::ScryglassMap => 1,   // sphere
        WorldButton::Tower => 2,          // square pyramid
        WorldButton::Assets => 3,         // cylinder
        WorldButton::ScryglassLibrary => 4, // cone
        WorldButton::ScryglassVault => 5,  // torus
        WorldButton::Back => 6,           // octahedron
        _ => return None,
    })
}

const PRIMITIVE_W: usize = 32;
const PRIMITIVE_H: usize = 24;

fn primitive(button: WorldButton) -> Option<&'static [f32]> {
    static IMAGES: std::sync::OnceLock<Vec<Vec<f32>>> = std::sync::OnceLock::new();
    let index = primitive_index(button)?;
    Some(&IMAGES.get_or_init(|| (0..7).map(primitive_image).collect())[index])
}

fn primitive_image(shape: usize) -> Vec<f32> {
    // Rotate the solid under an orthographic camera so planar faces have depth.
    let rotate = |[x, y, z]: [f32; 3]| {
        let (s, c) = 0.55_f32.sin_cos();
        let (x, z) = (c * x + s * z, -s * x + c * z);
        let (s, c) = (-0.28_f32).sin_cos();
        [x, c * y - s * z, s * y + c * z]
    };
    let distance = |p: [f32; 3]| {
        let [x, y, z] = rotate(p);
        let radius = (x * x + z * z).sqrt();
        match shape {
            0 => x.abs().max(y.abs()).max(z.abs()) - 0.64,
            1 => (x * x + y * y + z * z).sqrt() - 0.82,
            2 => ((x.abs().max(z.abs()) + 0.48 * y - 0.39) / 1.11)
                .max(-y - 0.72).max(y - 0.82),
            3 => (radius - 0.62).max(y.abs() - 0.73),
            4 => ((radius + 0.45 * y - 0.37) / 1.10)
                .max(-y - 0.74).max(y - 0.82),
            5 => ((radius - 0.57).powi(2) + y * y).sqrt() - 0.25,
            _ => (x.abs() + y.abs() + z.abs() - 1.12) / 3.0_f32.sqrt(),
        }
    };
    let mut image = vec![0.0; PRIMITIVE_W * PRIMITIVE_H];
    for row in 0..PRIMITIVE_H {
        for col in 0..PRIMITIVE_W {
            let x = (col as f32 + 0.5) / PRIMITIVE_W as f32 * 2.5 - 1.25;
            let y = 1.1 - (row as f32 + 0.5) / PRIMITIVE_H as f32 * 2.2;
            let mut z = 2.5;
            for _ in 0..72 {
                let p = [x, y, z];
                let d = distance(p);
                if d < 0.003 {
                    let e = 0.004;
                    let n = [distance([x + e, y, z]) - distance([x - e, y, z]),
                        distance([x, y + e, z]) - distance([x, y - e, z]),
                        distance([x, y, z + e]) - distance([x, y, z - e])];
                    let len = (n[0]*n[0] + n[1]*n[1] + n[2]*n[2]).sqrt().max(0.0001);
                    let diffuse = ((-0.45*n[0] + 0.65*n[1] + 0.61*n[2]) / len).max(0.0);
                    image[row * PRIMITIVE_W + col] = 0.24 + 0.76 * diffuse;
                    break;
                }
                z -= d.max(0.002);
                if z < -1.4 { break; }
            }
        }
    }
    image
}

fn render_primitive(frame: &mut Frame, rect: Rect, button: WorldButton) {
    let Some(image) = primitive(button) else { return; };
    if rect.width == 0 || rect.height == 0 { return; }
    let color = accent(button);
    // Braille gives every terminal cell a 2x4 pixel projection, not a flat icon.
    let dots = [[0, 3], [1, 4], [2, 5], [6, 7]];
    for row in 0..rect.height {
        for col in 0..rect.width {
            let mut bits = 0u32;
            let mut light = 0.0;
            let mut count = 0.0;
            for dy in 0..4 {
                for dx in 0..2 {
                    let sx = (usize::from(col) * 2 + dx) * PRIMITIVE_W / (usize::from(rect.width) * 2);
                    let sy = (usize::from(row) * 4 + dy) * PRIMITIVE_H / (usize::from(rect.height) * 4);
                    let shade = image[sy * PRIMITIVE_W + sx];
                    if shade > 0.0 {
                        bits |= 1 << dots[dy][dx];
                        light += shade;
                        count += 1.0;
                    }
                }
            }
            if bits != 0 {
                let rgb = color.map(|c| (f32::from(c) * light / count).round() as u8);
                frame.buffer_mut()[(rect.x + col, rect.y + row)]
                    .set_char(char::from_u32(0x2800 + bits).unwrap())
                    .set_fg(Color::Rgb(rgb[0], rgb[1], rgb[2]));
            }
        }
    }
}

/// The pane only publishes the launch anchor. Painting happens after all panes
/// and border repair, so neither neighboring windows nor their frames clip us.
pub(crate) fn prepare(app: &mut App, area: Rect, exploring: bool, offered: Vec<WorldButton>) {
    let dock = &mut app.object_dock;
    dock.offered = ACTIONS.iter().copied().filter(|(_, action)| offered.contains(action)).collect();
    dock.visible = !area.is_empty() && !dock.offered.is_empty();
    dock.area = area;
    dock.exploring = exploring;
    dock.tiles.clear();
    dock.objects.clear();
}

pub(crate) fn render_overlay(frame: &mut Frame<'_>, app: &mut App) {
    if !app.object_dock.visible { return; }
    // Modal controls still own the screen. Glass must disappear with the text.
    if app.pending_approval.is_some() || app.loop_dialog.is_some()
        || app.agent_menu.is_some() || app.setup.open() || app.moa_deck_owns_input()
    {
        app.object_dock.visible = false;
        app.object_dock.tiles.clear();
        app.object_dock.objects.clear();
        return;
    }
    let now = Instant::now();
    let motion = app.visual_motion;
    let dock = &mut app.object_dock;
    let area = dock.area.intersection(frame.area());
    dock.tiles.clear();
    dock.objects = dock.poses(now, motion, frame.area());
    if area.is_empty() { return; }
    let settled = !dock.animating(now, motion);
    let handle = Rect::new(area.x, area.bottom() - 1, area.width.min(3), 1);
    frame.render_widget(
        Paragraph::new(if dock.open { " ◂ " } else { " ▸ " })
            .style(Style::new().fg(Color::Rgb(111, 220, 255))), handle);
    app.world_buttons.push((handle, WorldButton::DockToggle));
    dock.tiles.push((handle, WorldButton::DockToggle));
    // No Clear, backplate, shared outline or catch-all surface. The terminal
    // underneath survives everywhere except the actual object/label glyphs.
    for object in &dock.objects {
        if !super::glass::active() {
            render_primitive(frame, cell_envelope(object.bounds).intersection(frame.area()), object.button);
        }
        let [r, g, b] = accent(object.button).map(|v| (f32::from(v) * object.opacity) as u8);
        frame.render_widget(Paragraph::new(object.label)
            .style(Style::new().fg(Color::Rgb(r, g, b)).bold()), object.label_rect);
        if dock.open && settled {
            dock.tiles.push((object.hit, object.button));
            app.world_buttons.push((object.hit, object.button));
        }
    }
    if area.width >= MIN_WIDTH && dock.open && settled {
        let (page, slots, _) = layout(area, dock.offered.len(), dock.page);
        dock.page = page;
        let pages = dock.offered.len().div_ceil(slots);
        if pages > 1 {
            for (x, enabled, text, action) in [
                (area.right() - 3, page > 0, "‹", WorldButton::DockPrevious),
                (area.right() - 1, page + 1 < pages, "›", WorldButton::DockNext),
            ] {
                if enabled {
                    let rect = Rect::new(x, area.bottom() - 1, 1, 1);
                    frame.render_widget(Paragraph::new(text)
                        .style(Style::new().fg(Color::Rgb(111, 220, 255))), rect);
                    dock.tiles.push((rect, action));
                    app.world_buttons.push((rect, action));
                }
            }
        }
    }
    super::glass::object_dock(dock, motion);
}

/// Exactly two surfaces per object: its soft local light and its solid. Both
/// float above the text grid; there is deliberately no shared drawer geometry.
#[cfg(feature = "glass")]
pub(crate) fn layers(dock: &ObjectDock, motion: MotionMode) -> Vec<angel_glass::Layer> {
    use angel_glass::{Layer, LayerKind, Placement, Stand};
    if !dock.visible { return Vec::new(); }
    let mut out = Vec::with_capacity(dock.objects.len() * 2);
    for object in &dock.objects {
        let shape = primitive_index(object.button).unwrap();
        let [r, g, b] = accent(object.button).map(|v| f32::from(v) / 255.0);
        for (part, z, kind, additive) in [
            (0, 20, LayerKind::Glow([r, g, b, 0.12]), true),
            (1, 21, LayerKind::Primitive {
                shape: shape as u32, color: [r, g, b, 0.88],
                phase: (0.55 + shape as f32 * 0.71) % std::f32::consts::TAU,
                speed: if motion == MotionMode::Full { 0.20 + shape as f32 * 0.012 } else { 0.0 },
            }, false),
        ] {
            let mut layer = Layer::new(0xD0C0_0000 + shape as u64 * 2 + part,
                z, Placement::Cells(object.bounds), kind);
            layer.opacity = object.opacity;
            layer.additive = additive;
            layer.stand = Stand::Screen;
            out.push(layer);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_button_has_a_unique_lit_3d_primitive() {
        let mut seen = Vec::new();
        for (_, button) in ACTIONS {
            let image = primitive(button).expect("every action needs a solid");
            assert_eq!(image.len(), PRIMITIVE_W * PRIMITIVE_H);
            assert!(image.iter().all(|v| v.is_finite() && (0.0..=1.01).contains(v)));
            assert!(image.iter().filter(|&&v| v > 0.0).count() > 60);
            assert!(image.iter().any(|&v| v > 0.65), "solid needs a lit face");
            assert!(image.iter().any(|&v| v > 0.0 && v < 0.4), "solid needs a shaded face");
            assert!(!seen.contains(&image), "two actions share a shape");
            seen.push(image);
        }
    }

    #[test]
    fn terminal_draws_each_solid_inside_its_button_without_losing_labels() {
        use ratatui::{Terminal, backend::TestBackend};
        let mut terminal = Terminal::new(TestBackend::new(77, 5)).unwrap();
        terminal.draw(|frame| {
            for (i, (label, button)) in ACTIONS.iter().enumerate() {
                render_primitive(frame, Rect::new(i as u16 * 11, 0, 10, 3), *button);
                frame.render_widget(Paragraph::new(*label), Rect::new(i as u16 * 11, 4, 10, 1));
            }
        }).unwrap();
        let buffer = terminal.backend().buffer();
        let mut projections = Vec::new();
        for (i, (label, _)) in ACTIONS.iter().enumerate() {
            let x = i as u16 * 11;
            let mut projection = String::new();
            let mut colors = std::collections::HashSet::new();
            for y in 0..3 {
                for col in x..x+10 {
                    let cell = &buffer[(col, y)];
                    projection.push_str(cell.symbol());
                    if cell.symbol() != " " { colors.insert(format!("{:?}", cell.fg)); }
                }
            }
            assert!(projection.chars().any(|c| ('\u{2801}'..='\u{28ff}').contains(&c)));
            assert!(colors.len() > 1, "{label} must show 3D light and shade");
            assert!(!projections.contains(&projection), "{label} repeats another silhouette");
            projections.push(projection);
            assert_eq!(buffer[(x + 10, 1)].symbol(), " ", "solid escaped its button");
            let text: String = (x..x+label.len() as u16).map(|col| buffer[(col, 4)].symbol()).collect();
            assert_eq!(text, *label);
        }
    }

    #[test]
    fn reversible_bounded_reveal_and_reduced_motion() {
        let now = Instant::now();
        let mut dock = ObjectDock::default();
        dock.toggle(now, MotionMode::Full);
        assert_eq!(dock.progress(now, MotionMode::Full), 0.0);
        let mid = now + Duration::from_millis(100);
        let p = dock.progress(mid, MotionMode::Full);
        assert!(p > 0.0 && p < 1.0);
        dock.toggle(mid, MotionMode::Full);
        assert_eq!(dock.progress(mid, MotionMode::Full), p);
        assert_eq!(dock.progress(mid + REVEAL, MotionMode::Full), 0.0);
        for mode in [MotionMode::Reduced, MotionMode::Off] {
            dock.toggle(mid + REVEAL, mode);
            assert_eq!(dock.progress(mid + REVEAL, mode), f32::from(dock.open));
            assert!(!dock.animating(mid + REVEAL, mode));
        }
    }

    #[test]
    fn every_action_reachable_in_bounds_without_overlaps_at_all_widths() {
        for width in 16..180 {
            let area = Rect::new(7, 11, width, 5);
            let mut seen = Vec::new();
            let mut page = 0;
            loop {
                let (actual, slots, rects) = layout(area, 7, page);
                assert_eq!(actual, page);
                for (i, rect) in rects.iter().enumerate() {
                    assert_eq!(rect.intersection(area), *rect);
                    for other in rects.iter().skip(i + 1) {
                        assert!(rect.intersection(*other).is_empty());
                    }
                    seen.push(ACTIONS[page * slots + i].1);
                }
                page += 1;
                if page * slots >= 7 {
                    break;
                }
            }
            assert_eq!(seen, ACTIONS.map(|(_, action)| action));
        }
    }

    #[cfg(feature = "glass")]
    #[test]
    fn object_dock_expansion_has_no_shared_panel_or_border() {
        let mut dock = ObjectDock::default();
        dock.visible = true;
        dock.open = true;
        dock.area = Rect::new(7, 11, 80, 5);
        dock.objects = dock.poses(Instant::now(), MotionMode::Off, Rect::new(0, 0, 100, 30));
        assert_eq!(dock.objects.len(), 7);
        assert_eq!(layers(&dock, MotionMode::Off).len(), 14);
        assert!(layers(&dock, MotionMode::Off).iter().all(|layer| matches!(
            layer.kind,
            angel_glass::LayerKind::Primitive { .. } | angel_glass::LayerKind::Glow(_)
        )), "expanding objects must never create a shared panel, well, or rim");
    }

    #[cfg(feature = "glass")]
    #[test]
    fn glass_geometry_is_valid_bounded_and_cleared_when_hidden() {
        let mut dock = ObjectDock::default();
        dock.visible = true;
        dock.open = true;
        dock.area = Rect::new(7, 11, 80, 5);
        dock.objects = dock.poses(Instant::now(), MotionMode::Off, Rect::new(0, 0, 100, 30));
        let geometry = layers(&dock, MotionMode::Off);
        assert!(geometry.len() < 32, "one procedural surface per object, not a pixel mosaic");
        let solids: Vec<_> = geometry.iter().filter(|l| matches!(l.kind,
            angel_glass::LayerKind::Primitive { .. })).collect();
        assert_eq!(solids.len(), 7);
        for (shape, layer) in solids.iter().enumerate() {
            assert!(matches!(layer.kind, angel_glass::LayerKind::Primitive {
                shape: s, speed: 0.0, .. } if s == shape as u32));
            assert!(layer.z > 0, "object must break above the text-grid plane");
        }
        for layer in geometry {
            assert!(layer.clone().sanitized().is_some(), "invalid {layer:?}");
            let angel_glass::Placement::Cells([x, y, w, h]) = layer.placement else {
                panic!("not cell relative")
            };
            assert!(x >= 0.0 && y >= 0.0 && x + w <= 100.0 && y + h <= 30.0);
        }
        dock.visible = false;
        assert!(layers(&dock, MotionMode::Off).is_empty());
    }

    #[cfg(feature = "glass")]
    #[test]
    fn glass_objects_rotate_above_the_pane_and_freeze_for_reduced_motion() {
        use angel_glass::{LayerKind, Placement};
        let mut dock = ObjectDock::default();
        dock.visible = true;
        dock.open = true;
        dock.area = Rect::new(7, 11, 80, 5);
        dock.objects = dock.poses(Instant::now(), MotionMode::Off, Rect::new(0, 0, 100, 30));
        for mode in [MotionMode::Full, MotionMode::Reduced, MotionMode::Off] {
            let geometry = layers(&dock, mode);
            let solids: Vec<_> = geometry.iter().filter(|l| matches!(l.kind, LayerKind::Primitive { .. })).collect();
            assert_eq!(solids.len(), 7);
            for (layer, object) in solids.iter().zip(&dock.objects) {
                assert_eq!(layer.animating(), mode == MotionMode::Full);
                let Placement::Cells([_, y, _, h]) = layer.placement else { panic!("cell placement") };
                assert!(y < f32::from(dock.area.y), "solid must escape the drawer edge");
                let hit = object.hit;
                assert!(f32::from(hit.y) <= y && f32::from(hit.bottom()) >= y + h);
                assert!(y + h <= f32::from(object.label_rect.y), "solid must not cover labels");
            }
        }
        dock.open = false;
        dock.objects = dock.poses(Instant::now(), MotionMode::Off, Rect::new(0, 0, 100, 30));
        assert!(!layers(&dock, MotionMode::Off).iter().any(|l| matches!(l.kind, LayerKind::Primitive { .. })));
    }

    #[test]
    fn object_dock_independent_lift_sway_reversal_and_motion_off() {
        let now = Instant::now();
        let mut dock = ObjectDock::default();
        dock.visible = true;
        dock.area = Rect::new(30, 11, 80, 5);
        dock.epoch = now;
        let screen = Rect::new(0, 0, 120, 30);
        dock.toggle(now, MotionMode::Full);
        assert!(dock.poses(now, MotionMode::Full, screen).is_empty());
        let mid = now + REVEAL / 2;
        let before = dock.poses(mid, MotionMode::Full, screen);
        assert_eq!(before.len(), 7);
        assert!(before.windows(2).any(|o| o[0].bounds[1] != o[1].bounds[1]));
        assert!(before.windows(2).any(|o| o[0].opacity != o[1].opacity));
        dock.toggle(mid, MotionMode::Full);
        assert_eq!(dock.poses(mid, MotionMode::Full, screen), before);
        assert!(dock.poses(mid + REVEAL, MotionMode::Full, screen).is_empty());
        dock.toggle(mid + REVEAL, MotionMode::Off);
        let still = dock.poses(mid, MotionMode::Off, screen);
        assert_eq!(still, dock.poses(mid + Duration::from_secs(10), MotionMode::Reduced, screen));
        assert!(still.iter().any(|o| o.bounds[0] < f32::from(dock.area.x)));
        assert!(still.iter().all(|o| o.bounds[1] < f32::from(dock.area.y)));
        assert_ne!(dock.poses(mid, MotionMode::Full, screen),
            dock.poses(mid + Duration::from_secs(10), MotionMode::Full, screen));
    }

    #[test]
    fn object_dock_floating_hits_stay_bounded_nonoverlapping_and_paged() {
        let now = Instant::now();
        for width in MIN_WIDTH..180 {
            let mut dock = ObjectDock::default();
            dock.open = true;
            dock.area = Rect::new(7, 11, width, 5);
            let screen = Rect::new(0, 0, width + 14, 30);
            let mut seen = Vec::new();
            loop {
                let objects = dock.poses(now, MotionMode::Off, screen);
                for (i, object) in objects.iter().enumerate() {
                    assert_eq!(object.hit.intersection(screen), object.hit);
                    for other in &objects[i + 1..] {
                        assert!(object.hit.intersection(other.hit).is_empty(),
                            "overlapping actions at width {width}: {object:?}, {other:?}");
                    }
                    seen.push(object.button);
                }
                let (_, slots, _) = layout(dock.area, 7, dock.page);
                dock.page += 1;
                if dock.page * slots >= 7 { break; }
            }
            assert_eq!(seen, ACTIONS.map(|(_, button)| button));
        }
    }

    #[test]
    fn object_dock_overlay_preserves_underlying_cells_and_click_through_gaps() {
        use crate::tests::{env_lock, TestEnvGuard};
        use ratatui::{Terminal, backend::TestBackend};
        let _guard = env_lock();
        let _comp = TestEnvGuard::unset("ANGEL_COMP_MODE");
        let mut app = App::preview(crate::ui::viewer::Viewer::new());
        app.visual_motion = MotionMode::Off;
        app.object_dock.open = true;
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        terminal.draw(|frame| {
            for y in 0..30 {
                frame.render_widget(Paragraph::new("x".repeat(100))
                    .style(Style::new().bg(Color::Rgb(9, 13, 20))), Rect::new(0, y, 100, 1));
            }
            prepare(&mut app, Rect::new(30, 20, 60, 5), false, ACTIONS.map(|(_, action)| action).to_vec());
            render_overlay(frame, &mut app);
        }).unwrap();
        assert_eq!(app.object_dock.objects.len(), 5, "compact viewport pages, never overlaps");
        let buffer = terminal.backend().buffer();
        let mut untouched = 0;
        for y in 0..30 {
            for x in 0..100 {
                if app.object_dock.hit(x, y).is_none() {
                    assert_eq!(buffer[(x, y)].symbol(), "x", "cleared empty terminal at {x},{y}");
                    assert_eq!(buffer[(x, y)].bg, Color::Rgb(9, 13, 20));
                    untouched += 1;
                }
            }
        }
        assert!(untouched > 2500);
        assert!(app.object_dock.hit(50, 24).is_none(), "no catch-all panel hit");
        assert!(!app.world_buttons.iter().any(|(_, b)| *b == WorldButton::DockSurface));
    }
}
