// Isolated visual verifier: imports production coin material and motion.
// Does not start an agent, change session state, or join a game.
use std::time::Duration;

#[path = "../src/ui/viz/spend_viz.rs"]
pub(crate) mod spend_source;

mod ui {
    pub mod viz {
        pub mod lifecycle_viz {
            #[derive(Clone, Copy, Debug, PartialEq, Eq)]
            pub(crate) enum MotionMode { Full, Reduced, Off }
        }
        pub(crate) use crate::spend_source as spend_viz;
    }
}

use ui::viz::{lifecycle_viz::MotionMode, spend_viz};
use angel_glass::{Layer, LayerKind, Placement};

fn main() {
    let out = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../artifacts/spend-coin");
    std::fs::create_dir_all(&out).unwrap();
    let mut options = angel_glass::Options::from_environment();
    options.columns = 80;
    options.rows = 22;
    options.title = "Gold coin milestone preview".into();
    angel_glass::run(options, move |host| {
        use ratatui::{Terminal, layout::Rect, style::{Color, Style}, widgets::Paragraph};
        let mut terminal = Terminal::new(host.backend()).unwrap();
        // The sixteen native flip frames use the exact production material.
        for frame in 0..16u32 {
            let mut rgba = Vec::with_capacity(96 * 96 * 4);
            for y in 0..96 {
                for x in 0..96 {
                    let color = spend_viz::gold_pixel((x as f32 + 0.5)/48.0 - 1.0,
                        (y as f32 + 0.5)/48.0 - 1.0, 0.08 + frame as f32 / 15.0 * 0.92);
                    rgba.extend_from_slice(&color.map_or([0;4], |[r,g,b]| [r,g,b,255]));
                }
            }
            host.upload_picture(angel_glass::graphics::Picture {
                id: angel_glass::LAYER_PICTURES + 0x400 + frame,
                width: 96, height: 96, rgba, cols: 0, rows: 0,
            });
        }
        for (name, elapsed) in [("face", 0.0), ("edge", 0.084375), ("burst", 0.72), ("landing", 1.2)] {
            terminal.draw(|f| {
                let area = f.area();
                f.render_widget(Paragraph::new("GOLD COIN / MILLION INPUT MILESTONES")
                    .style(Style::new().fg(Color::Rgb(255,220,120))), Rect::new(2,1,area.width-4,1));
                f.render_widget(Paragraph::new("Native 96px coin material | production toss geometry | isolated preview")
                    .style(Style::new().fg(Color::Rgb(145,165,190))), Rect::new(2,2,area.width-4,1));
                for (column, count) in [1u64, 2, 5].into_iter().enumerate() {
                    let x = 2 + column as u16 * 25;
                    f.render_widget(Paragraph::new(format!("{count}M : {count} coin{}", if count == 1 {""} else {"s"}))
                        .style(Style::new().fg(Color::Rgb(235,210,145))), Rect::new(x,4,24,1));
                }
                f.render_widget(Paragraph::new("Terminal fallback / same gold material")
                    .style(Style::new().fg(Color::Rgb(145,165,190))), Rect::new(2,18,55,1));
                let p = spend_viz::pose(Rect::new(3,19,8,3), elapsed, MotionMode::Full).unwrap();
                spend_viz::render_coin(f,p);
                f.render_widget(Paragraph::new("Reduced motion: face stays visible. Off: static coin fan + tally.")
                    .style(Style::new().fg(Color::Rgb(145,165,190))), Rect::new(13,20,65,1));
            }).unwrap();
            let mut layers = Vec::new();
            for (column, count) in [1u64, 2, 5].into_iter().enumerate() {
                let stage = Rect::new(2 + column as u16 * 25, 6, 24, 11);
                for p in spend_viz::poses(stage, f64::from(elapsed), count, MotionMode::Full) {
                    let phase = ((p.face_scale - 0.08)/0.92*15.0).round().clamp(0.0,15.0) as u32;
                    let a = p.area;
                    layers.push(Layer::new(layers.len() as u64,8,
                        Placement::Cells([a.x as f32,a.y as f32,a.width as f32,a.height as f32]),
                        LayerKind::Picture(angel_glass::LAYER_PICTURES + 0x400 + phase)));
                }
            }
            host.set_layer_group(0x434f494e,layers);
            std::thread::sleep(Duration::from_millis(180));
            host.capture(out.join(format!("{name}.png")));
            std::thread::sleep(Duration::from_millis(220));
        }
    }).unwrap();
}
