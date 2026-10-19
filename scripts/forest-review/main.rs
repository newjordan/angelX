//! Real shared-stage replay, native views and the cockpit's actual cell buffer.
#[path = "../../cockpit/src/stage/world_viz/art/cells.rs"]
mod cells;
#[path = "../../cockpit/src/stage/world_viz/art/forest_road.rs"]
mod panels;
use angel_delve::{
    render::overworld::arena,
    rules::{HZ, Run, classes},
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Color};
use serde_json::json;
use std::{collections::BTreeMap, path::Path, time::Instant};
fn rgb(c: Color) -> [u8; 3] {
    if let Color::Rgb(r, g, b) = c {
        [r, g, b]
    } else {
        [0, 0, 0]
    }
}
fn hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
    })
}
fn main() {
    let root = std::env::args().nth(1).expect("output directory");
    let root = Path::new(&root);
    std::fs::create_dir_all(root).unwrap();
    angel_delve::settings::set(angel_delve::settings::Settings {
        art_v2: true,
        feel_ledger: false,
    });
    let mut r = Run::forest_road_for_test();
    for id in 2..=4 {
        r.join_seat(id, &format!("Knight {id}"));
    }
    for (n, c) in classes::CLASSES[..4].iter().enumerate() {
        r.become_class(n as u32 + 1, c);
    }
    r.start_forest_road();
    let mut found = std::collections::BTreeSet::new();
    let mut snapshots = vec![];
    let mut snapshots_ms = vec![];
    let mut milestones = vec![];
    for _ in 0..HZ * 300 {
        let s = r.forest_road.as_ref().unwrap();
        let inputs: BTreeMap<_, _> = r
            .players
            .iter()
            .map(|(&id, h)| (id, s.bot_input(id, h)))
            .collect();
        r.step(&inputs);
        let s = r.forest_road.as_ref().unwrap();
        if milestones
            .last()
            .is_none_or(|m: &serde_json::Value| m["wave"] != s.wave)
        {
            milestones.push(json!({"wave":s.wave,"ticks":s.elapsed}));
        }
        let name = if s.elapsed == 45 {
            Some("start")
        } else if s.wave == 3
            && s.started
            && s.foes[0].x - s.knights[&1].camera < 330
            && s.foes[0].beat == angel_delve::rules::forest_road::Beat::Tell
        {
            Some("midboss")
        } else if s.wave == 7
            && s.started
            && s.foes[0].x - s.knights[&1].camera < 330
            && s.foes[0].beat == angel_delve::rules::forest_road::Beat::Attack
        {
            Some("feller")
        } else if s.freeing == Some(40) {
            Some("freeing")
        } else {
            None
        };
        if let Some(name) = name.filter(|n| !found.contains(*n)) {
            found.insert(name.to_string());
            let (base, fx) = arena::forest_road_layers(&r, Some(1));
            image::save_buffer(
                root.join(format!("{name}-native.png")),
                &base.rgb_bytes(),
                384,
                224,
                image::ColorType::Rgb8,
            )
            .unwrap();
            let mut terminal = Terminal::new(TestBackend::new(96, 28)).unwrap();
            terminal
                .draw(|f| {
                    cells::paint_cells(f, Rect::new(0, 0, 96, 28), &base, &fx);
                    panels::paint_forest_road_panels(f, Rect::new(0, 0, 96, 28), &r, Some(1));
                })
                .unwrap();
            let buffer: Vec<_> = terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .map(|c| json!({"glyph":c.symbol(),"fg":rgb(c.fg),"bg":rgb(c.bg)}))
                .collect();
            std::fs::write(
                root.join(format!("{name}-cells.json")),
                serde_json::to_vec(&buffer).unwrap(),
            )
            .unwrap();
            std::fs::write(
                root.join(format!("{name}-state.json")),
                serde_json::to_vec(&r).unwrap(),
            )
            .unwrap();
            let mut frames = vec![];
            let mut cell_times = vec![];
            for _ in 0..60 {
                let t = Instant::now();
                for seat in 1..=4 {
                    std::hint::black_box(arena::frame_for(&r, 384, 224, Some(seat)));
                }
                frames.push(t.elapsed().as_secs_f64() * 1000.0);
                let t = Instant::now();
                terminal
                    .draw(|f| {
                        cells::paint_cells(f, Rect::new(0, 0, 96, 28), &base, &fx);
                        panels::paint_forest_road_panels(f, Rect::new(0, 0, 96, 28), &r, Some(1));
                    })
                    .unwrap();
                cell_times.push(t.elapsed().as_secs_f64() * 1000.0);
            }
            frames.sort_by(f64::total_cmp);
            cell_times.sort_by(f64::total_cmp);
            snapshots_ms.push(json!({"moment":name,"four_native_views_median_ms":frames[30],"four_native_views_p95_ms":frames[57],"cells_median_ms":cell_times[30],"cells_p95_ms":cell_times[57]}));
            snapshots.push(json!({"moment":name,"ticks":s.elapsed,"seats":s.knights.len(),"camera":s.knights[&1].camera}));
        }
        if s.completed.is_some() {
            break;
        }
    }
    let s = r.forest_road.as_ref().unwrap();
    assert!(s.completed.is_some(), "four-bot replay did not finish");
    assert_eq!(found.len(), 4);
    let replay = serde_json::to_vec(&r).unwrap();
    let report = json!({"completion_ticks":s.elapsed,"completion_seconds":s.elapsed as f64/HZ as f64,"falls":s.knights.iter().map(|(&id,k)|(id,k.falls)).collect::<Vec<_>>(),"replay_fnv64":format!("{:016x}",hash(&replay)),"milestones":milestones,"captures":snapshots,"frame_cost":snapshots_ms});
    std::fs::write(
        root.join("metrics.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    println!("{}", report);
    // Verify the released character's actual surface placement and dialogue.
    r.walk_kingdom(Some(angel_delve::rules::forest_road::ENT_TILE));
    r.step(&BTreeMap::from([(1, angel_delve::rules::Input {
        interact: true, ..Default::default()
    })]));
    assert!(r.found_line().unwrap().contains("Old Oak"));
    let npc = arena::frame_for(&r, 384, 224, Some(1));
    image::save_buffer(root.join("kingdom-ent-native.png"), &npc.rgb_bytes(),
        384, 224, image::ColorType::Rgb8).unwrap();
}
