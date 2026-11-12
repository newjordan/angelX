//! Native detailed-asset viewer. The procedural Resonator remains the default.
use angel_glass::{LAYER_PICTURES, Layer, LayerKind, Placement, PointerEvent, Zone, ZoneKind};
use crossterm::event::{Event, KeyCode, KeyEventKind, MouseEventKind};
use gauss_glass::{
    gpu::{Camera, Quality, Renderer, Stats},
    import::Cloud,
};
use ratatui_core::{
    buffer::Buffer,
    style::{Color, Modifier, Style},
    terminal::Terminal,
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
const PICTURE: u32 = LAYER_PICTURES + 751;
const GROUP: u32 = 751;
const INK: Color = Color::Rgb(226, 226, 218);
const MUTED: Color = Color::Rgb(128, 143, 151);
const GREEN: Color = Color::Rgb(155, 221, 190);

fn label(buf: &mut Buffer, x: u16, y: u16, s: &str, color: Color, bold: bool) {
    if y >= buf.area.height || x >= buf.area.width {
        return;
    }
    buf.set_stringn(
        x,
        y,
        s,
        usize::from(buf.area.width - x),
        Style::default().fg(color).add_modifier(if bold {
            Modifier::BOLD
        } else {
            Modifier::empty()
        }),
    );
}
fn layout(w: u32, h: u32) -> [f32; 4] {
    [
        18.,
        110.,
        (w as f32 - 36.).max(1.),
        (h as f32 - 204.).max(1.),
    ]
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Control {
    Orbit,
    Pan,
    Quality,
    Reset,
}

// Controls use the same cell coordinates for painting and hit testing. Hide the
// strip on small grids rather than leaving invisible or overlapping hit targets.
fn controls(grid: angel_glass::GridSize) -> Vec<(Control, [f32; 4])> {
    if grid.cols < 66 || grid.rows < 10 || grid.cell_width == 0 || grid.cell_height == 0 {
        return Vec::new();
    }
    let y = grid.rows.saturating_sub(7) as f32 * grid.cell_height as f32;
    [
        (Control::Orbit, 3, 15),
        (Control::Pan, 20, 15),
        (Control::Quality, 36, 14),
        (Control::Reset, 52, 12),
    ]
    .into_iter()
    .map(|(action, x, width)| {
        (
            action,
            [
                x as f32 * grid.cell_width as f32,
                y,
                width as f32 * grid.cell_width as f32,
                grid.cell_height as f32,
            ],
        )
    })
    .collect()
}

fn apply_control(action: Control, camera: &mut Camera, orbit: &mut bool, pan: &mut bool) {
    match action {
        Control::Orbit => *orbit = !*orbit,
        Control::Pan => *pan = !*pan,
        Control::Quality => camera.quality = camera.quality.next(),
        Control::Reset => {
            let (viewport, quality) = (camera.viewport, camera.quality);
            *camera = Camera::default();
            camera.viewport = viewport;
            camera.quality = quality;
            *orbit = false;
            *pan = false;
        }
    }
}

fn completed_control(
    pressed: Option<Control>,
    at: [f32; 2],
    click: bool,
    controls: &[(Control, [f32; 4])],
) -> Option<Control> {
    pressed.filter(|action| {
        click
            && controls.iter().any(|(candidate, rect)| {
                candidate == action && Zone::new(*rect, ZoneKind::Button).holds(at)
            })
    })
}

fn paint(buf: &mut Buffer, cloud: &Cloud, stats: &Stats, camera: Camera, orbit: bool, pan: bool) {
    let w = buf.area.width;
    let h = buf.area.height;
    label(
        buf,
        3,
        1,
        "G / G    GAUSSIAN STUDY  /  NATIVE GLASS",
        GREEN,
        true,
    );
    label(
        buf,
        3,
        3,
        &format!("{} / {}", cloud.source, camera.quality.label()),
        INK,
        true,
    );
    if w > 110 {
        label(
            buf,
            w.saturating_sub(32),
            1,
            &format!("SH {} / {}", cloud.degree, camera.quality.label()),
            MUTED,
            false,
        );
    }
    if w >= 66 && h >= 10 {
        let y = h - 7;
        label(
            buf,
            3,
            y,
            if orbit {
                "[ ORBIT: ON ]"
            } else {
                "[ ORBIT: OFF ]"
            },
            GREEN,
            true,
        );
        label(
            buf,
            20,
            y,
            if pan {
                "[ MODE: PAN ]"
            } else {
                "[ MODE: ORBIT ]"
            },
            GREEN,
            true,
        );
        label(buf, 36, y, "[ QUALITY ]", GREEN, true);
        label(buf, 52, y, "[ RESET ]", GREEN, true);
    }
    let y = h.saturating_sub(5);
    label(
        buf,
        3,
        y,
        if pan {
            "DRAG pan [P: orbit]   SCROLL zoom   SPACE auto-orbit   R reset   Q close"
        } else {
            "DRAG orbit [P: pan]   SCROLL zoom   SPACE auto-orbit   R reset   Q close"
        },
        INK,
        false,
    );
    label(
        buf,
        3,
        y + 1,
        "Arrows orbit   WASD pan   +/- zoom   [ / ] exposure   F quality   X/Y/Z rotate",
        MUTED,
        false,
    );
    let line = if let Some(e) = &stats.error {
        format!("GPU ERROR: {e}")
    } else {
        format!(
            "{} GAUSSIANS   {:.1} MiB buffers + {:.1} MiB target   {}x{} {}X   CPU sort {:.2} ms   EV {:+.1}{}",
            cloud.gaussians.len(),
            stats.resident_bytes as f64 / 1048576.,
            stats.target_bytes as f64 / 1048576.,
            stats.render_size[0],
            stats.render_size[1],
            stats.sample_scale,
            stats.sort_ms,
            camera.exposure,
            if orbit { "   ORBIT" } else { "" }
        )
    };
    label(buf, 3, h.saturating_sub(2), &line, GREEN, false);
}

pub fn run(args: &[String]) -> Result<(), String> {
    let flag = if args.iter().any(|a| a == "--inspect") {
        "--inspect"
    } else {
        "--import"
    };
    let at = args.iter().position(|a| a == flag).unwrap();
    let path = args
        .get(at + 1)
        .filter(|p| !p.starts_with("--"))
        .ok_or_else(|| format!("Usage: ./run.sh {flag} PATH [--smoke]"))?;
    let quality = if let Some(index) = args.iter().position(|arg| arg == "--quality") {
        args.get(index + 1)
            .and_then(|value| Quality::parse(value))
            .ok_or_else(|| "--quality must be reference, balanced, or ultra".to_string())?
    } else {
        Quality::default()
    };
    let start = Instant::now();
    let cloud = Cloud::load(path).map_err(|e| e.to_string())?;
    println!(
        "GAUSS_IMPORT_OK file={path:?} count={} sh_degree={} source_bytes={} radius={} load_ms={:.2}",
        cloud.gaussians.len(),
        cloud.degree,
        std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
        cloud.radius,
        start.elapsed().as_secs_f64() * 1000.
    );
    println!(
        "bounds centre={:?}; no Gaussian records discarded",
        cloud.centre
    );
    if flag == "--inspect" {
        return Ok(());
    }
    let smoke = args.iter().any(|a| a == "--smoke");
    let capture = std::env::var_os("GAUSS_CAPTURE_DIR").map(PathBuf::from);
    if let Some(dir) = &capture {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let cloud = Arc::new(cloud);
    let camera = Arc::new(Mutex::new(Camera {
        quality,
        ..Camera::default()
    }));
    let stats = Arc::new(Mutex::new(Stats::default()));
    let result_stats = stats.clone();
    let mut options = angel_glass::Options::default();
    options.title = format!("Gauss / Glass — {}", cloud.source);
    options.app_id = "gauss-import".into();
    options.columns = 132;
    options.rows = 48;
    options.font_size = 11.;
    options.palette.background = [8, 11, 17];
    options.palette.foreground = [226, 226, 218];
    angel_glass::run(options, move |host| {
        let mut terminal = Terminal::new(host.backend()).expect("Glass terminal");
        let _ = terminal.hide_cursor();
        let mut input = host.input_reader().expect("Glass input");
        host.set_gpu_picture(PICTURE, Some((Box::new(Renderer::new(cloud.clone(), camera.clone(), stats.clone())), (1200, 900))));
        let start = Instant::now();
        let mut last = start;
        let mut drag = None;
        let mut pressed = None;
        let mut orbit = false;
        let mut pan = false;
        let mut phase = 0;
        loop {
            let now = Instant::now();
            let dt = now.duration_since(last).as_secs_f32().min(0.1);
            last = now;
            let size = host.grid_size();
            let rect = layout(size.width_px, size.height_px);
            {
                let mut c = camera.lock().unwrap_or_else(|e| e.into_inner());
                c.viewport = [rect[2] as u32, rect[3] as u32];
                if orbit { c.yaw += dt * 0.13; }
            }
            let buttons = controls(size);
            // Put buttons first: zones are hit-tested in registration order.
            // Keep button indices stable even when controls are hidden at a
            // small size: already-queued presses still name their old zone.
            let mut zones: Vec<_> = (0..4).map(|i| Zone::new(
                buttons.get(i).map_or([0.0; 4], |(_, rect)| *rect), ZoneKind::Button
            )).collect();
            zones.push(Zone::new(rect, ZoneKind::Button));
            host.set_pointer_zones(GROUP, zones);
            for event in host.take_pointer_events(GROUP) {
                match event {
                    PointerEvent::Press { at, zone, .. } => {
                        pressed = [Control::Orbit, Control::Pan, Control::Quality, Control::Reset].get(zone).copied();
                        if zone == 4 { drag = Some(at); orbit = false; }
                    }
                    PointerEvent::Move { at } => if let Some(old) = drag {
                        let mut c = camera.lock().unwrap_or_else(|e| e.into_inner());
                        if pan { c.pan_by(at[0] - old[0], at[1] - old[1]); }
                        else { c.drag(at[0] - old[0], at[1] - old[1]); }
                        drag = Some(at);
                    },
                    PointerEvent::Release { at, click } => {
                        if let Some(action) = completed_control(pressed.take(), at, click, &buttons) {
                            let mut c = camera.lock().unwrap_or_else(|e| e.into_inner());
                            apply_control(action, &mut c, &mut orbit, &mut pan);
                        }
                        drag = None;
                    }
                }
            }
            host.set_layer_group(GROUP, vec![Layer::new(1, -1, Placement::Pixels(rect), LayerKind::Picture(PICTURE))]);
            let current = *camera.lock().unwrap_or_else(|e| e.into_inner());
            let status = stats.lock().unwrap_or_else(|e| e.into_inner()).clone();
            if terminal.draw(|f| paint(f.buffer_mut(), &cloud, &status, current, orbit, pan)).is_err() { break; }
            if smoke {
                let t = start.elapsed().as_secs_f32();
                if phase < 3 && t > 3. + phase as f32 * 2. {
                    if let Some(dir) = &capture {
                        std::thread::sleep(Duration::from_millis(100));
                        host.capture(dir.join(format!("import-{phase:02}.png")));
                        std::thread::sleep(Duration::from_millis(150));
                    }
                    let mut c = camera.lock().unwrap_or_else(|e| e.into_inner());
                    match phase {
                        0 => c.drag(65., -15.),
                        1 => { c.zoom_by(3.); c.pan_by(30., -12.); c.exposure = 0.3; }
                        _ => {}
                    }
                    phase += 1;
                }
                if t > 8.5 {
                    // Producer submissions are NOT presented frames or a GPU timestamp measurement.
                    println!("GAUSS_GPU_SMOKE submissions={} count={} resident_buffer_bytes={} last_cpu_sort_ms={:.3} cpu_submit_ms={:.3} elapsed={t:.2}s",
                        status.frames, status.count, status.resident_bytes, status.sort_ms, status.submit_ms);
                    println!("GAUSS_QUALITY mode={} render={}x{} sample_scale={} target_bytes={}", current.quality.label(),status.render_size[0],status.render_size[1],status.sample_scale,status.target_bytes);
                    break;
                }
            }
            match input(Duration::from_millis(16)) {
                Ok(Some(Event::Key(k))) if k.kind != KeyEventKind::Release => {
                    let mut c = camera.lock().unwrap_or_else(|e| e.into_inner());
                    match k.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Char(' ') => apply_control(Control::Orbit, &mut c, &mut orbit, &mut pan),
                        KeyCode::Char('p') => { apply_control(Control::Pan, &mut c, &mut orbit, &mut pan); drag = None; }
                        KeyCode::Char('f') => apply_control(Control::Quality, &mut c, &mut orbit, &mut pan),
                        KeyCode::Char('r') => { apply_control(Control::Reset, &mut c, &mut orbit, &mut pan); drag = None; }
                        KeyCode::Left => c.drag(-8., 0.),
                        KeyCode::Right => c.drag(8., 0.),
                        KeyCode::Up => c.drag(0., -8.),
                        KeyCode::Down => c.drag(0., 8.),
                        KeyCode::Char('w') => c.pan_by(0., 20.),
                        KeyCode::Char('s') => c.pan_by(0., -20.),
                        KeyCode::Char('a') => c.pan_by(20., 0.),
                        KeyCode::Char('d') => c.pan_by(-20., 0.),
                        KeyCode::Char('+') | KeyCode::Char('=') => c.zoom_by(1.),
                        KeyCode::Char('-') => c.zoom_by(-1.),
                        KeyCode::Char('[') => c.exposure = (c.exposure - 0.25).max(-3.),
                        KeyCode::Char(']') => c.exposure = (c.exposure + 0.25).min(3.),
                        KeyCode::Char('x') => c.pitch += std::f32::consts::FRAC_PI_2,
                        KeyCode::Char('y') => c.yaw += std::f32::consts::FRAC_PI_2,
                        KeyCode::Char('z') => c.roll += std::f32::consts::FRAC_PI_2,
                        _ => {}
                    }
                }
                Ok(Some(Event::Mouse(m))) => match m.kind {
                    MouseEventKind::ScrollUp => camera.lock().unwrap_or_else(|e| e.into_inner()).zoom_by(1.),
                    MouseEventKind::ScrollDown => camera.lock().unwrap_or_else(|e| e.into_inner()).zoom_by(-1.),
                    _ => {}
                },
                Ok(Some(Event::FocusLost)) => { drag = None; pressed = None; },
                Err(_) => break,
                _ => {}
            }
        }
        host.set_pointer_zones(GROUP, vec![]);
        host.set_layer_group(GROUP, vec![]);
        host.set_gpu_picture(PICTURE, None);
    }).map_err(|e| e.to_string())?;
    let stats = result_stats.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(e) = &stats.error {
        return Err(e.clone());
    }
    if stats.frames == 0 {
        return Err("No Gaussian GPU frames produced".into());
    }
    Ok(())
}

#[cfg(test)]
mod interaction_tests {
    use super::*;
    fn grid() -> angel_glass::GridSize {
        angel_glass::GridSize {
            cols: 100,
            rows: 40,
            width_px: 800,
            height_px: 640,
            cell_width: 8,
            cell_height: 16,
        }
    }
    #[test]
    fn buttons_match_visible_grid_and_do_not_overlap() {
        let buttons = controls(grid());
        assert_eq!(buttons.len(), 4);
        for (i, (_, r)) in buttons.iter().enumerate() {
            assert_eq!(r[1], 33. * 16.);
            for (_, other) in buttons.iter().skip(i + 1) {
                assert!(r[0] + r[2] <= other[0]);
            }
        }
        assert!(controls(angel_glass::GridSize { cols: 65, ..grid() }).is_empty());
        assert!(controls(angel_glass::GridSize { rows: 9, ..grid() }).is_empty());
    }
    #[test]
    fn buttons_cancel_drag_or_release_outside() {
        let buttons = controls(grid());
        let point = [buttons[0].1[0] + 2., buttons[0].1[1] + 2.];
        assert_eq!(
            completed_control(Some(Control::Orbit), point, true, &buttons),
            Some(Control::Orbit)
        );
        assert_eq!(
            completed_control(Some(Control::Orbit), point, false, &buttons),
            None
        );
        assert_eq!(
            completed_control(Some(Control::Orbit), [0., 0.], true, &buttons),
            None
        );
        assert_eq!(completed_control(None, point, true, &buttons), None);
        assert_eq!(
            completed_control(Some(Control::Orbit), point, true, &[]),
            None
        );
    }
    #[test]
    fn reset_clears_modes_and_camera_but_preserves_quality_and_viewport() {
        let mut c = Camera {
            yaw: 1.,
            pan: [1., 2.],
            quality: Quality::Ultra,
            viewport: [55, 66],
            ..Camera::default()
        };
        let (mut orbit, mut pan) = (true, true);
        apply_control(Control::Reset, &mut c, &mut orbit, &mut pan);
        assert_eq!(c.yaw, Camera::default().yaw);
        assert_eq!(c.pan, [0., 0.]);
        assert_eq!(c.viewport, [55, 66]);
        assert_eq!(c.quality, Quality::Ultra);
        assert!(!orbit && !pan);
        apply_control(Control::Orbit, &mut c, &mut orbit, &mut pan);
        apply_control(Control::Pan, &mut c, &mut orbit, &mut pan);
        apply_control(Control::Quality, &mut c, &mut orbit, &mut pan);
        assert!(orbit && pan);
        assert_eq!(c.quality, Quality::Reference);
    }
}
