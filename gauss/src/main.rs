mod viewer;
use angel_glass::{
    Host, LAYER_PICTURES, Layer, LayerKind, Placement, PointerEvent, Zone, ZoneKind,
    graphics::Picture,
};
use crossterm::event::{Event, KeyCode, KeyEventKind, MouseEventKind};
use gauss_glass::{State, instrument, palette, project};
use ratatui_core::{
    buffer::Buffer,
    style::{Color, Modifier, Style},
    terminal::Terminal,
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

const ATLAS: u32 = LAYER_PICTURES + 740;
const GROUP: u32 = 740;
const TILE: u32 = 40;
const INK: Color = Color::Rgb(225, 222, 213);
const MUTED: Color = Color::Rgb(122, 135, 147);
const GOLD: Color = Color::Rgb(208, 167, 116);
const GREEN: Color = Color::Rgb(139, 222, 190);

#[derive(Clone, Copy)]
struct Layout {
    scene: [f32; 4],
    side: [f32; 4],
    slider: [f32; 4],
    power: [f32; 4],
    light: [f32; 4],
    orbit: [f32; 4],
    reset: [f32; 4],
}
impl Layout {
    fn new(w: f32, h: f32) -> Self {
        let wide = w >= 1000.0;
        let (scene, side) = if wide {
            (
                [24., 115., w - 360., (h - 205.).max(180.)],
                [w - 302., 170., 270., h - 250.],
            )
        } else {
            (
                [16., 98., w - 32., (h - 410.).max(180.)],
                [32., (h - 283.).max(300.), w - 64., 220.],
            )
        };
        let [x, y, width, _] = side;
        Self {
            scene,
            side,
            slider: [x, y + 99., width, 26.],
            power: [x, y + 146., width * 0.46, 35.],
            light: [x + width * 0.53, y + 146., width * 0.47, 35.],
            orbit: [x, y + 192., width * 0.46, 35.],
            reset: [x + width * 0.53, y + 192., width * 0.47, 35.],
        }
    }
    fn zones(&self, s: &State) -> Vec<Zone> {
        let p = s.power_position(self.scene);
        vec![
            Zone::new([p[0] - 29., p[1] - 24., 58., 48.], ZoneKind::Button),
            Zone::new(self.slider, ZoneKind::Button),
            Zone::new(self.power, ZoneKind::Button),
            Zone::new(self.light, ZoneKind::Button),
            Zone::new(self.orbit, ZoneKind::Button),
            Zone::new(self.reset, ZoneKind::Button),
            Zone::new(self.scene, ZoneKind::Button),
        ]
    }
}
fn atlas(host: &Host) {
    let width = TILE * 16;
    let height = width * 2;
    let mut rgba = vec![0; (width * height * 4) as usize];
    for warm in [true, false] {
        for i in 0..256 {
            let rgb = palette(i, warm);
            for y in 0..TILE {
                for x in 0..TILE {
                    let u = ((x as f32 + 0.5) / TILE as f32 - 0.5) * 6.;
                    let v = ((y as f32 + 0.5) / TILE as f32 - 0.5) * 6.;
                    let alpha = if u * u + v * v > 9. {
                        0.
                    } else {
                        (-0.5 * (u * u + v * v)).exp()
                    };
                    let row = (i as u32 / 16 + u32::from(!warm) * 16) * TILE + y;
                    let column = (i as u32 % 16) * TILE + x;
                    let at = ((row * width + column) * 4) as usize;
                    rgba[at..at + 4].copy_from_slice(&[
                        rgb[0],
                        rgb[1],
                        rgb[2],
                        (alpha * 255.) as u8,
                    ]);
                }
            }
        }
    }
    host.upload_picture(Picture {
        id: ATLAS,
        width,
        height,
        rgba,
        cols: 0,
        rows: 0,
    });
}
fn solid(id: u64, rect: [f32; 4], rgb: [f32; 4]) -> Layer {
    Layer::new(id, -3, Placement::Pixels(rect), LayerKind::Solid(rgb))
}
fn layers(splats: &[gauss_glass::Splat], s: &State, l: Layout) -> Vec<Layer> {
    let mut out = Vec::with_capacity(splats.len() + 50);
    let [x, y, w, h] = l.scene;
    // Grounding shadow: a Gaussian, not a hard ellipse or mesh impostor.
    let shadow = Layer::new(
        1,
        -4,
        Placement::Pixels([x + w * 0.22, y + h * 0.63, w * 0.56, h * 0.20]),
        LayerKind::Glow([0., 0., 0., 0.70]),
    );
    out.push(shadow);
    // Sparse spatial ruler, behind the object.
    for i in 0..=12 {
        let px = x + w * 0.12 + w * 0.76 * i as f32 / 12.;
        out.push(solid(
            10 + i,
            [px, y + h * 0.86, 1., if i % 3 == 0 { 9. } else { 4. }],
            [0.22, 0.28, 0.31, 0.45],
        ));
    }
    out.push(solid(
        30,
        [x + w * 0.12, y + h * 0.86, w * 0.76, 1.],
        [0.20, 0.25, 0.29, 0.35],
    ));
    for (i, p) in project(splats, s, l.scene).into_iter().enumerate() {
        let mut layer = Layer::new(
            100 + i as u64,
            -2,
            Placement::Pixels([
                p.centre[0] - p.sigma[0] * 3.,
                p.centre[1] - p.sigma[1] * 3.,
                p.sigma[0] * 6.,
                p.sigma[1] * 6.,
            ]),
            LayerKind::PicturePart(
                ATLAS,
                [
                    (p.color % 16) as f32 * TILE as f32,
                    ((p.color / 16) as f32 + if s.warm { 0. } else { 16. }) * TILE as f32,
                    TILE as f32,
                    TILE as f32,
                ],
            ),
        );
        layer.angle = p.angle;
        layer.opacity = p.alpha;
        out.push(layer);
    }
    for (i, rect) in [l.power, l.light, l.orbit, l.reset].into_iter().enumerate() {
        out.push(solid(50000 + i as u64, rect, [0.08, 0.105, 0.13, 0.75]));
    }
    let [sx, sy, sw, _] = l.slider;
    out.push(solid(50010, [sx, sy + 12., sw, 2.], [0.18, 0.23, 0.27, 1.]));
    out.push(solid(
        50011,
        [sx, sy + 12., sw * s.resonance, 2.],
        [0.58, 0.79, 0.70, 1.],
    ));
    out.push(Layer::new(
        50012,
        -1,
        Placement::Pixels([sx + sw * s.resonance - 5., sy + 7., 10., 12.]),
        LayerKind::Solid([0.72, 0.94, 0.83, 1.]),
    ));
    out
}
fn text(buf: &mut Buffer, cell: [f32; 2], x: f32, y: f32, content: &str, color: Color, bold: bool) {
    let col = (x / cell[0]).round().max(0.) as u16;
    let row = (y / cell[1]).round().max(0.) as u16;
    if col < buf.area.width && row < buf.area.height {
        let mut style = Style::default().fg(color);
        if bold {
            style = style.add_modifier(Modifier::BOLD);
        }
        buf.set_stringn(col, row, content, (buf.area.width - col) as usize, style);
    }
}
fn ui(buf: &mut Buffer, cell: [f32; 2], size: [f32; 2], s: &State, l: Layout, count: usize) {
    let [w, h] = size;
    text(buf, cell, 32., 24., "G / G     GAUSS GLASS", INK, true);
    if w > 650. {
        text(
            buf,
            cell,
            w - 290.,
            24.,
            "EXPERIMENT 001  /  NATIVE",
            MUTED,
            false,
        );
    }
    text(buf, cell, 32., 69., "Matter, made interactive.", INK, true);
    text(
        buf,
        cell,
        32.,
        96.,
        "A small instrument. A different kind of interface.",
        MUTED,
        false,
    );
    let [x, y, _, _] = l.side;
    text(buf, cell, x, y - 33., "01 / RESONANCE", GOLD, true);
    text(
        buf,
        cell,
        x,
        y + 2.,
        &format!("{:05.1} Hz", s.frequency()),
        INK,
        true,
    );
    text(buf, cell, x, y + 32., "Shape the signal.", INK, false);
    text(
        buf,
        cell,
        x,
        y + 57.,
        "Drag the line to tune.",
        MUTED,
        false,
    );
    for (rect, label, color) in [
        (
            l.power,
            if s.powered {
                "[ ON ] POWER"
            } else {
                "[ -- ] POWER"
            },
            if s.powered { GREEN } else { MUTED },
        ),
        (
            l.light,
            if s.warm { "WARM LIGHT" } else { "COOL LIGHT" },
            GOLD,
        ),
        (
            l.orbit,
            if s.orbit { "ORBIT: ON" } else { "ORBIT: OFF" },
            MUTED,
        ),
        (l.reset, "RESET VIEW", MUTED),
    ] {
        text(buf, cell, rect[0] + 8., rect[1] + 8., label, color, false);
    }
    if w >= 1000. {
        text(
            buf,
            cell,
            x,
            y + 270.,
            "NOT A VIDEO. NOT A MESH.",
            GOLD,
            false,
        );
        text(
            buf,
            cell,
            x,
            y + 301.,
            "3D Gaussian surfaces.",
            MUTED,
            false,
        );
        text(
            buf,
            cell,
            x,
            y + 326.,
            "Depth-sorted. Relightable.",
            MUTED,
            false,
        );
        text(
            buf,
            cell,
            x,
            y + 351.,
            "Yours to turn and touch.",
            MUTED,
            false,
        );
    }
    let [sx, sy, sw, sh] = l.scene;
    text(buf, cell, sx + 12., sy + sh * 0.82, "X", MUTED, false);
    if sh > 300. {
        text(buf, cell, sx + 12., sy + sh * 0.08, "S / 001", GOLD, false);
        text(
            buf,
            cell,
            sx + 12.,
            sy + sh * 0.13,
            "THE RESONATOR",
            MUTED,
            false,
        );
        text(
            buf,
            cell,
            sx + sw * 0.57,
            sy + sh * 0.76,
            if s.powered {
                "SIGNAL ACTIVE"
            } else {
                "SIGNAL STANDBY"
            },
            if s.powered { GREEN } else { MUTED },
            false,
        );
    }
    text(
        buf,
        cell,
        32.,
        h - 60.,
        "DRAG to orbit   SCROLL to zoom   CLICK the core",
        MUTED,
        false,
    );
    text(
        buf,
        cell,
        32.,
        h - 33.,
        "SPACE power   L light   O orbit   R reset   Q close",
        MUTED,
        false,
    );
    if w > 1050. {
        text(
            buf,
            cell,
            w - 290.,
            h - 33.,
            &format!("{count} GAUSSIANS / GLASS"),
            GOLD,
            false,
        );
    }
}
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--import" || a == "--inspect") {
        if let Err(error) = viewer::run(&args) {
            eprintln!("gauss: {error}");
            std::process::exit(1);
        }
        return;
    }

    let smoke = std::env::args().any(|a| a == "--smoke");
    let capture = std::env::var_os("GAUSS_CAPTURE_DIR").map(PathBuf::from);
    if let Some(dir) = &capture {
        std::fs::create_dir_all(dir).expect("capture directory");
    }
    let mut options = angel_glass::Options::default();
    options.title = "Gauss / Glass — the resonator".into();
    options.app_id = "gauss-glass".into();
    options.columns = 118;
    options.rows = 40;
    options.font_size = 12.;
    options.palette.background = [9, 12, 18];
    options.palette.foreground = [225, 222, 213];
    let result = angel_glass::run(options, move |host| {
        let mut terminal = Terminal::new(host.backend()).expect("Glass terminal");
        terminal.hide_cursor().ok();
        let mut input = host.input_reader().expect("Glass input");
        let splats = instrument();
        let mut state = State::default();
        atlas(&host);
        let start = Instant::now();
        let mut previous = start;
        let mut drag: Option<(usize, [f32; 2])> = None;

        let mut phase = 0;
        let mut frames = 0u64;
        loop {
            let now = Instant::now();
            state.tick(now.duration_since(previous).as_secs_f32());
            previous = now;
            let grid = host.grid_size();
            let size = [grid.width_px as f32, grid.height_px as f32];
            let cell = [
                grid.cell_width.max(1) as f32,
                grid.cell_height.max(1) as f32,
            ];
            let layout = Layout::new(size[0], size[1]);
            host.set_pointer_zones(GROUP, layout.zones(&state));
            for event in host.take_pointer_events(GROUP) {
                match event {
                    PointerEvent::Press { at, zone, .. } => {
                        drag = Some((zone, at));
                        if zone == 1 {
                            state.tune((at[0] - layout.slider[0]) / layout.slider[2]);
                        }
                    }
                    PointerEvent::Move { at } => {
                        if let Some((zone, last)) = drag {
                            if zone == 1 {
                                state.tune((at[0] - layout.slider[0]) / layout.slider[2]);
                            } else if zone == 0 || zone == 6 {
                                state.drag(at[0] - last[0], at[1] - last[1]);
                            }
                            drag = Some((zone, at));
                        }
                    }
                    PointerEvent::Release { click, .. } => {
                        if let Some((zone, _)) = drag.take() {
                            if click {
                                match zone {
                                    0 | 2 => state.powered = !state.powered,
                                    3 => state.warm = !state.warm,
                                    4 => state.orbit = !state.orbit,
                                    5 => state = State::default(),
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }

            host.set_layer_group(GROUP, layers(&splats, &state, layout));
            if let Err(e) =
                terminal.draw(|f| ui(f.buffer_mut(), cell, size, &state, layout, splats.len()))
            {
                eprintln!("draw: {e}");
                break;
            }
            frames += 1;
            // Deterministic action sequence for GPU captures, no services or automation daemon.
            if smoke {
                let t = start.elapsed().as_secs_f32();
                let threshold = [3.0, 4.4, 5.8, 7.2, 8.6];
                if phase < threshold.len() && t >= threshold[phase] {
                    if let Some(dir) = &capture {
                        std::thread::sleep(Duration::from_millis(80));
                        host.capture(dir.join(format!(
                            "{:02}-{}.png",
                            phase,
                            ["warm", "cool", "standby", "orbit", "tuned"][phase]
                        )));
                        std::thread::sleep(Duration::from_millis(120));
                    }
                    match phase {
                        0 => state.warm = false,
                        1 => state.powered = false,
                        2 => {
                            state.powered = true;
                            state.orbit = true;
                        }
                        3 => state.tune(0.88),
                        _ => {}
                    }
                    phase += 1;
                }
                if t > 9.8 {
                    println!(
                        "GLASS_SMOKE_OK frames={frames} gaussians={} elapsed={t:.2}s captures={phase}",
                        splats.len()
                    );
                    break;
                }
            }
            match input(Duration::from_millis(16)) {
                Ok(Some(Event::Key(k))) if k.kind != KeyEventKind::Release => match k.code {
                    KeyCode::Esc | KeyCode::Char('q') => break,
                    KeyCode::Char(' ') => state.powered = !state.powered,
                    KeyCode::Char('l') => state.warm = !state.warm,
                    KeyCode::Char('o') => state.orbit = !state.orbit,
                    KeyCode::Char('r') => state = State::default(),
                    KeyCode::Left => state.tune(state.resonance - 0.025),
                    KeyCode::Right => state.tune(state.resonance + 0.025),
                    KeyCode::Up => state.drag(0., -6.),
                    KeyCode::Down => state.drag(0., 6.),
                    KeyCode::Char('+') | KeyCode::Char('=') => state.zoom_by(1.),
                    KeyCode::Char('-') => state.zoom_by(-1.),
                    KeyCode::Char('s') => {
                        if let Some(dir) = &capture {
                            host.capture(dir.join("manual.png"));
                        }
                    }
                    _ => {}
                },
                Ok(Some(Event::Mouse(m))) => match m.kind {
                    MouseEventKind::ScrollUp => state.zoom_by(1.),
                    MouseEventKind::ScrollDown => state.zoom_by(-1.),
                    _ => {}
                },
                Ok(Some(Event::FocusLost)) => drag = None,
                Err(_) => break,
                _ => {}
            }
        }
        host.set_pointer_zones(GROUP, vec![]);
        host.set_layer_group(GROUP, vec![]);
        host.free_pictures([ATLAS]);
    });
    if let Err(error) = result {
        eprintln!("gauss-glass: {error}");
        std::process::exit(1);
    }
}
