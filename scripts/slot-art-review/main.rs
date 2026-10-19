#[allow(dead_code)]
mod painter {
    include!(concat!(env!("OUT_DIR"), "/painter.rs"));
}
use angel_delve::{
    render::overworld::arena::{self, slot_art},
    rules::{
        Input, Run,
        latch::{Beat, Duel, LENGTH},
    },
    settings::{self, Settings},
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Color};
use std::{collections::BTreeMap, path::Path};
fn capture(dir: &Path, name: &str, run: &Run, seat: u32) {
    let snapshot = serde_json::to_vec(run).unwrap();
    let im = arena::frame_for(run, 384, 224, Some(seat));
    image::save_buffer(
        dir.join(format!("{name}.png")),
        &im.rgb_bytes(),
        384,
        224,
        image::ColorType::Rgb8,
    )
    .unwrap();
    let (base, fx) = arena::art_v2::terminal_layers(run, Some(seat)).unwrap();
    let mut term = Terminal::new(TestBackend::new(96, 28)).unwrap();
    term.draw(|f| {
        let rect = Rect::new(0, 0, 96, 28);
        painter::paint_cells(f, rect, &base, &fx);
        painter::paint_latch_panels(f, rect, run, seat);
    })
    .unwrap();
    let buf = term.backend().buffer();
    let rgb = |c: Color| match c {
        Color::Rgb(r, g, b) => [r, g, b],
        _ => [0; 3],
    };
    let cells: Vec<_> = (0..28)
        .flat_map(|y| {
            (0..96).map(move |x| {
                let c = &buf[(x, y)];
                serde_json::json!({"glyph":c.symbol(),"fg":rgb(c.fg),"bg":rgb(c.bg)})
            })
        })
        .collect();
    std::fs::write(
        dir.join(format!("{name}-cells.json")),
        serde_json::to_vec(&cells).unwrap(),
    )
    .unwrap();
    assert_eq!(
        snapshot,
        serde_json::to_vec(run).unwrap(),
        "rendering mutated rules"
    );
}
fn game() -> Run {
    let mut r = Run::latch_for_test();
    for id in 2..=4 {
        r.join_seat(id, &format!("Seat {id}"));
    }
    r.step(&BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]));
    assert_eq!(r.latch.as_ref().unwrap().duels.len(), 4);
    r
}
fn slot_cells(dir: &Path) {
    use angel_delve::render::overworld::ink::Img;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/delve/assets/art-slots");
    let mut records = serde_json::Map::new();
    for entry in std::fs::read_dir(root).unwrap().flatten() {
        let folder = entry.path();
        if !folder.is_dir() {
            continue;
        }
        let slot = folder.file_name().unwrap().to_str().unwrap().to_owned();
        let metadata: serde_json::Value =
            serde_json::from_slice(&std::fs::read(folder.join("slot.json")).unwrap()).unwrap();
        let mut frames = serde_json::Map::new();
        for (name, _) in metadata["outputs"]
            .as_object()
            .unwrap()
            .iter()
            .filter(|(p, _)| p.starts_with("v1/"))
        {
            let source = image::open(folder.join(name)).unwrap().to_rgba8();
            let mut base = Img::black(384, 224);
            let x0 = (384 - source.width() as i32) / 2;
            let y0 = (224 - source.height() as i32) / 2;
            for (x, y, p) in source.enumerate_pixels() {
                if p[3] > 0 {
                    base.set(x0 + x as i32, y0 + y as i32, [p[0], p[1], p[2]]);
                }
            }
            let mut term = Terminal::new(TestBackend::new(96, 28)).unwrap();
            term.draw(|f| {
                painter::paint_cells(f, Rect::new(0, 0, 96, 28), &base, &Img::new(384, 224))
            })
            .unwrap();
            let buf = term.backend().buffer();
            let mut occupied = 0;
            let mut bright = 0;
            for y in 0..28 {
                for x in 0..96 {
                    let c = &buf[(x, y)];
                    for color in [c.fg, c.bg] {
                        if let Color::Rgb(r, g, b) = color {
                            occupied += u32::from(r.max(g).max(b) > 16);
                            bright += u32::from(r.max(g).max(b) > 80);
                        }
                    }
                }
            }
            frames.insert(name.clone(),serde_json::json!({"native_sha256":metadata["outputs"][name]["sha256"],"occupied_half_cells":occupied,"readable_half_cells":bright,"weak":bright<2}));
        }
        records.insert(slot, serde_json::Value::Object(frames));
    }
    std::fs::write(
        dir.join("terminal-slot-metrics.json"),
        serde_json::to_vec_pretty(&records).unwrap(),
    )
    .unwrap();
}
fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).expect("output directory"));
    std::fs::create_dir_all(&dir).unwrap();
    settings::set(Settings::default());
    slot_cells(&dir);
    let base = game();
    for original in [true, false] {
        slot_art::use_original(original);
        let phase = if original { "before" } else { "after" };
        let mut run = base.clone();
        for (name, beat, height, ticks, freeze) in [
            ("high", Beat::Tell, -1, 7, 0),
            ("low", Beat::Tell, 1, 7, 0),
            ("contact", Beat::Clean, -1, 18, 2),
            ("recover", Beat::Clean, -1, 5, 0),
        ] {
            for d in run.latch.as_mut().unwrap().duels.values_mut() {
                *d = Duel::new(41);
                d.beat = beat;
                d.height = height;
                d.guard = height;
                d.ticks = ticks;
                d.freeze = freeze;
                d.counters = u8::from(beat == Beat::Clean) as _;
                d.chain = u8::from(beat == Beat::Clean) as _;
            }
            capture(&dir, &format!("{phase}-{name}"), &run, 1);
        }
        for seat in 2..=4 {
            capture(&dir, &format!("{phase}-seat-{seat}"), &run, seat);
        }
        let started = std::time::Instant::now();
        for _ in 0..120 {
            std::hint::black_box(arena::frame_for(&run, 384, 224, Some(1)));
        }
        let mean_ms = started.elapsed().as_secs_f64() * 1000.0 / 120.0;
        std::fs::write(
            dir.join(format!("{phase}-frame-cost.json")),
            serde_json::to_vec_pretty(&serde_json::json!({"warm_native_ms":mean_ms,"samples":120}))
                .unwrap(),
        )
        .unwrap();
        let mut run = base.clone();
        let mut elapsed = 0;
        for _ in 0..LENGTH {
            if run.latch.as_ref().unwrap().left == 0 {
                break;
            }
            let input = run
                .latch
                .as_ref()
                .unwrap()
                .duels
                .iter()
                .map(|(&id, d)| (id, d.bot_input(1 + id % 3)))
                .collect();
            run.step(&input);
            elapsed += 1;
        }
        assert_eq!(
            run.latch.as_ref().unwrap().left,
            0,
            "four-seat bot round must finish"
        );
        capture(&dir, &format!("{phase}-podium"), &run, 1);
        let value = serde_json::json!({"elapsed_ticks":elapsed,"simulation":run});
        std::fs::write(
            dir.join(format!("{phase}-round.json")),
            serde_json::to_vec_pretty(&value).unwrap(),
        )
        .unwrap();
        println!("{phase}: four seats completed in {elapsed} ticks");
    }
    assert_eq!(
        std::fs::read(dir.join("before-round.json")).unwrap(),
        std::fs::read(dir.join("after-round.json")).unwrap(),
        "art changed round outcome"
    );
    slot_art::use_original(false);
}
