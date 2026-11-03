//! The Heartglass's printed plate, drawn without a font: text comes from a
//! stand-in typesetter (a block of ink per character), so these check
//! where things are drawn and in what colour, not the glyphs. The board
//! is made in code.

use super::campaign::{self, Campaign, parse_board};
use super::plate::{self, CloudView, Ink, Style, Typeset, View};
use super::{Grid, Layout};

/// A character is a solid block of ink `size / 2` wide.
struct Blocks;

impl Typeset for Blocks {
    fn text(&mut self, line: &str, size: f32, ink: [u8; 3], _bold: bool) -> Option<Ink> {
        let w = (line.chars().count() as f32 * size * 0.5).round() as u32;
        let h = size.round() as u32;
        if w == 0 || h == 0 {
            return None;
        }
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for _ in 0..w * h {
            rgba.extend_from_slice(&[ink[0], ink[1], ink[2], 255]);
        }
        Some((w, h, rgba))
    }
}

fn layout() -> Layout {
    Layout::for_grid(
        Grid {
            width: 1576,
            height: 880,
        },
        None,
        5,
    )
    .unwrap()
}

fn campaign() -> Campaign {
    let json = r#"{"schema": "vec.board/1", "updated": "2026-10-20T21:05:00Z",
        "phase": {"name": "P2", "collab_ends": "2026-10-27T00:00:00Z"},
        "boards": [
          {"key": "T1:val", "short": "T1", "label": "Single-cell E10.5", "heart": false, "ve_leader": 74.0, "ve_top10": 70.0},
          {"key": "T2:heart:val_interp", "short": "HI", "label": "Heart interp E8.5", "heart": true},
          {"key": "T2:heart:val_extrap", "short": "HX", "label": "Heart extrap E10.5", "heart": true,
           "ve_leader": 72.0, "ve_top10": 66.0, "ve_partner": 58.0, "yukon_frontier": 55.0,
           "ours_best": {"score": 62.0}, "proxy_best": {"score": 64.0},
           "budget": {"used": 1, "limit": 2, "resets": "2026-10-21T00:00:00Z"},
           "history": [{"kind": "proxy", "score": 64.0}, {"kind": "official", "score": 62.0}]},
          {"key": "T2:embryo:val_interp", "short": "EI", "label": "Embryo interp E7.5", "heart": false},
          {"key": "T3:gata4", "short": "G4", "label": "Gata4 KO E8.75", "heart": true}],
        "events": [{"at": "2026-10-20T21:00:00Z", "kind": "scored", "text": "HX 62.0"}]}"#;
    parse_board(json.as_bytes()).unwrap()
}

/// The plate's pixel at shown point `p` of `layout`, at the scale drawn.
fn at(ink: &Ink, layout: &Layout, p: [f32; 2], scale: f32) -> [u8; 4] {
    let x = ((p[0] - layout.panel.x) * scale) as u32;
    let y = ((p[1] - layout.panel.y) * scale) as u32;
    let i = ((y * ink.0 + x) * 4) as usize;
    [ink.2[i], ink.2[i + 1], ink.2[i + 2], ink.2[i + 3]]
}

/// The demo plate's frame, to look at (text is the stand-in blocks):
/// `ANGEL_ARENA_SHOTS=<dir> cargo test -p angelX-cockpit --bin angel write_demo_plate_shot -- --ignored`
#[test]
#[ignore]
fn write_demo_plate_shot() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("heartglass-demo");
    std::fs::create_dir_all(&dir).unwrap();
    let l = layout();
    let c = super::demo::campaign(1_793_000_000);
    let view = View {
        campaign: Some(&c),
        cloud: None,
        dir: "synthetic demo",
    };
    let ink = plate::draw(&l, &view, 2.0, &mut Blocks);
    image::RgbaImage::from_raw(ink.0, ink.1, ink.2)
        .unwrap()
        .save(dir.join("demo-plate.png"))
        .unwrap();
}

#[test]
fn the_plate_is_its_panel_at_twice_the_size() {
    let l = layout();
    for c in [None, Some(campaign())] {
        let view = View {
            campaign: c.as_ref(),
            cloud: None,
            dir: "~/.angelX/vec",
        };
        let ink = plate::draw(&l, &view, 2.0, &mut Blocks);
        assert_eq!(
            (ink.0, ink.1),
            (
                (l.panel.w * 2.0).ceil() as u32,
                (l.panel.h * 2.0).ceil() as u32
            )
        );
        assert_eq!(ink.2.len(), (ink.0 * ink.1 * 4) as usize);
        // Clear glass between the marks: most of the plate is see-through.
        let clear = ink.2.chunks(4).filter(|p| p[3] == 0).count();
        assert!(clear * 2 > (ink.0 * ink.1) as usize, "{clear} clear pixels");
    }
}

#[test]
fn a_boards_marks_land_where_its_scores_are() {
    let l = layout();
    let c = campaign();
    let view = View {
        campaign: Some(&c),
        cloud: None,
        dir: "",
    };
    let ink = plate::draw(&l, &view, 2.0, &mut Blocks);
    let domain = plate::domain(Some(&c));
    let hx = plate::track(l.rows[2]);
    let x = |v: f64| plate::score_x(v, domain, hx.x0, hx.x1);
    // Our official bar, solid rose, between the floor and our score.
    let bar = at(&ink, &l, [(x(50.0) + x(62.0)) / 2.0, hx.y], 2.0);
    assert!(
        bar[3] > 200 && bar[0] > 200 && bar[1] < 120,
        "the bar is solid rose: {bar:?}"
    );
    // Past our score, inside the proxy's ghost, it is faint.
    let ghost = at(&ink, &l, [(x(62.0) + x(64.0)) / 2.0 + 2.0, hx.y - 3.0], 2.0);
    assert!(
        ghost[3] > 0 && ghost[3] < 120,
        "the proxy is a ghost: {ghost:?}"
    );
    // The leader's mark is gold above the rail, the team's violet below.
    let gold = at(&ink, &l, [x(72.0), hx.y - 11.0], 2.0);
    assert!(
        gold[0] > 200 && gold[1] > 150 && gold[2] < 120,
        "leader gold: {gold:?}"
    );
    let violet = at(&ink, &l, [x(58.0), hx.y + 9.8], 2.0);
    assert!(
        violet[2] > 200 && violet[0] > 120 && violet[1] < 160,
        "team violet: {violet:?}"
    );
    // A board with no scores has a bare rail at the same place.
    let ei = plate::track(l.rows[3]);
    let bare = at(&ink, &l, [(x(50.0) + x(62.0)) / 2.0, ei.y], 2.0);
    assert!(bare[0] < 100, "no bar on a board with no score: {bar:?}");
    // One budget pip lit (one of two left), one hollow.
    let lit = at(&ink, &l, [hx.right + 6.0, hx.y], 2.0);
    let used = at(&ink, &l, [hx.right + 16.0, hx.y], 2.0);
    assert!(lit[3] > 200 && lit[0] > 200, "a lit pip: {lit:?}");
    assert!(
        used[3] < 120,
        "the used pip is hollow in the middle: {used:?}"
    );
}

#[test]
fn the_score_range_holds_everything_known_rounded_to_fives() {
    assert_eq!(plate::domain(None), (45.0, 80.0));
    let mut c = campaign();
    assert_eq!(plate::domain(Some(&c)), (45.0, 80.0));
    c.boards[0].ve_leader = Some(91.3);
    c.boards[1].ve_partner = Some(38.2);
    assert_eq!(plate::domain(Some(&c)), (35.0, 95.0));
    // A rail clamps what is off its ends.
    assert_eq!(plate::score_x(10.0, (45.0, 80.0), 100.0, 200.0), 100.0);
    assert_eq!(plate::score_x(99.0, (45.0, 80.0), 100.0, 200.0), 200.0);
    assert_eq!(plate::score_x(62.5, (45.0, 80.0), 100.0, 200.0), 150.0);
}

#[test]
fn the_clock_counts_down_and_says_how_old_the_ticker_is() {
    let c = campaign();
    let ends = c.phase.collab_ends.unwrap();
    let (countdown, reset, age) =
        plate::clock_text(&c, ends - (3 * 86_400 + 4 * 3_600 + 5 * 60 + 6));
    assert_eq!(countdown, "3d 04:05:06");
    assert!(reset.starts_with("budget resets"));
    assert!(age.ends_with("d ago"));
    assert_eq!(plate::clock_text(&c, ends + 1).0, "ended");
    let resets = c.next_reset().unwrap();
    assert_eq!(
        plate::clock_text(&c, resets - 2 * 3_600 - 7 * 60).1,
        "budget resets 2h07m"
    );
    assert_eq!(plate::ago(3), "now");
    assert_eq!(plate::ago(42), "42s ago");
    assert_eq!(plate::ago(7 * 60 + 5), "7m ago");
    assert_eq!(plate::ago(-5), "now");
    // Nothing to count: empty strings, no picture.
    let mut quiet = c.clone();
    quiet.phase = campaign::Phase::default();
    quiet.events.clear();
    for b in &mut quiet.boards {
        b.budget.resets = None;
    }
    assert_eq!(
        plate::clock_text(&quiet, 0),
        (String::new(), String::new(), String::new())
    );
}

#[test]
fn long_text_is_cut_to_fit_with_an_ellipsis() {
    let l = layout();
    let mut blocks = Blocks;
    let mut print = plate::Print::new(l.panel, 2.0, &mut blocks);
    let style = Style::new(plate::TINY, plate::INK);
    let at = [l.panel.x + 10.0, l.panel.y + 10.0];
    // 40 characters at 6.25 px each would be 250 px; it gets at most 100.
    let w = print.text_fit(&"x".repeat(40), at, style, 100.0, 0.0);
    assert!(w <= 100.0 && w > 80.0, "{w}");
    // Text that fits is set whole (the blocks round to whole pixels).
    assert!(
        (print.text_fit("short", at, style, 100.0, 0.0) - 5.0 * plate::TINY * 0.5).abs() <= 0.5
    );
}

#[test]
fn a_predicted_stage_names_its_boards_tag() {
    let c = campaign();
    assert_eq!(plate::board_tag("T2:heart:val_extrap", Some(&c)), "HX");
    assert_eq!(plate::board_tag("T3:gata4", None), "G4");
    assert_eq!(plate::board_tag("T9:new:thing", None), "thing");
    let cloud = campaign::Cloud {
        groups: vec![
            campaign::Group {
                name: "myocardium".into(),
                rgb: [255, 64, 88],
            },
            campaign::Group {
                name: "neural".into(),
                rgb: [80, 140, 255],
            },
        ],
        stages: vec![campaign::Stage {
            label: "E10.5*".into(),
            predicted: true,
            board: "T2:heart:val_extrap".into(),
            points: vec![
                campaign::Point {
                    pos: [0.0; 3],
                    group: 0,
                    cardiac: true,
                },
                campaign::Point {
                    pos: [0.1; 3],
                    group: 1,
                    cardiac: false,
                },
            ],
        }],
    };
    // The key lists only the lineages that are cardiac somewhere.
    let view = CloudView::of(&cloud);
    assert_eq!(
        view.lineages,
        vec![("myocardium".to_string(), [255, 64, 88])]
    );
    assert_eq!(view.stages, vec![("E10.5*".to_string(), true)]);
    let (built, _) = super::heart::stage_meshes(&cloud, 0).unwrap();
    let title = plate::stage_title(&built, "HX", 300.0, 2.0, &mut Blocks);
    assert!(title.0 > 0 && title.1 > 0);
}

/// The plate's pictures as they were before the floating HUD split its
/// drawing into modules: the plate is the operator's chosen look, so any
/// change to a pixel of it is a change to that look, and must be meant.
#[test]
fn the_plate_draws_exactly_as_it_did() {
    fn fnv(ink: &Ink) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in ink.0.to_le_bytes().iter().chain(&ink.1.to_le_bytes()).chain(&ink.2) {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        h
    }
    let l = layout();
    let c = campaign();
    let cloud = CloudView {
        stages: vec![
            ("E8.25".into(), false),
            ("E8.75".into(), false),
            ("E9.5".into(), false),
            ("E10.5*".into(), true),
        ],
        lineages: vec![
            ("myocardium".into(), [255, 64, 88]),
            ("endocardium".into(), [60, 209, 237]),
            ("epicardium".into(), [240, 185, 60]),
        ],
    };
    let hashes: Vec<u64> = [
        View { campaign: None, cloud: None, dir: "~/.angelX/vec" },
        View { campaign: Some(&c), cloud: None, dir: "~/.angelX/vec" },
        View { campaign: Some(&c), cloud: Some(&cloud), dir: "~/.angelX/vec" },
    ]
    .iter()
    .map(|view| fnv(&plate::draw(&l, view, 2.0, &mut Blocks)))
    .collect();
    assert_eq!(
        hashes,
        vec![0xf1dd_920c_858d_7868, 0xf59e_79d3_6d1a_20a0, 0xdbd4_b837_bbbc_7fe6],
        "{hashes:#x?}"
    );
}
