//! The floating Heartglass: its modules' designed layout over the whole
//! window, the saved layout and its defaults, the hit-test, and what a
//! drag, a resize and a hide do. All pure: no window, no font.

use super::Grid;
use super::hud::{self, Anchored, Hud, MARGIN, Module, Part, Place, Saved};

const CELL: (f32, f32) = (12.0, 25.0);

fn grid(width: u32, height: u32) -> Grid {
    Grid { width, height }
}

fn hud(w: u32, h: u32) -> Hud {
    hud::arrange(grid(w, h), CELL, 5, &Saved::default()).expect("a HUD")
}

/// The sizes people use: a laptop, the nested seat, a tall and a narrow
/// window, and a 4K screen.
const SIZES: [(u32, u32); 6] = [
    (1600, 1000),
    (1576, 880),
    (1280, 720),
    (941, 880),
    (941, 640),
    (3840, 2160),
];

fn overlap(a: super::Panel, b: super::Panel) -> bool {
    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
}

#[test]
fn the_designed_hud_spreads_over_the_window_inside_its_header_and_composer() {
    for (w, h) in SIZES {
        let hud = hud(w, h);
        assert_eq!(hud.modules.len(), 6, "{w}x{h}");
        let top = super::TOP_ROWS * CELL.1;
        let bottom = h as f32 - super::BOTTOM_ROWS * CELL.1;
        for m in &hud.modules {
            // Its content is in the window; only its margin's shade may
            // hang over an edge.
            let c = m.content();
            assert!(c.x >= 0.0 && c.y >= 0.0, "{w}x{h} {:?} {c:?}", m.module);
            assert!(c.x + c.w <= w as f32 + 0.01, "{w}x{h} {:?} {c:?}", m.module);
            assert!(c.y + c.h <= h as f32 + 0.01, "{w}x{h} {:?} {c:?}", m.module);
            // Only the heartbeat strip may ride in the header's middle.
            assert!(
                c.y >= top - 0.01 || (m.module == Module::Title && w as f32 >= hud::WIDE),
                "{w}x{h} {:?} under the header",
                m.module
            );
            // Only the ticker may ride the rule over the composer, and
            // never its input row.
            let floor = if m.module == Module::Ticker {
                h as f32 - 3.8 * CELL.1
            } else {
                bottom
            };
            assert!(
                c.y + c.h <= floor + 0.01,
                "{w}x{h} {:?} over the composer",
                m.module
            );
            assert_eq!(m.scale, 1.0);
        }
        // No two modules' contents cover each other.
        for (i, a) in hud.modules.iter().enumerate() {
            for b in &hud.modules[i + 1..] {
                assert!(
                    !overlap(a.content(), b.content()),
                    "{w}x{h}: {:?} {:?} over {:?} {:?}",
                    a.module,
                    a.content(),
                    b.module,
                    b.content()
                );
            }
        }
        // The heartbeat strip runs across the top; the heart stands at the
        // right with its rail under it; the boards keep low, the legend
        // under them; the ticker is at the bottom.
        let get = |m: Module| hud.get(m).unwrap().content();
        let (title, heart, stages, boards, legend, ticker) = (
            get(Module::Title),
            get(Module::Heart),
            get(Module::Stages),
            get(Module::Boards),
            get(Module::Legend),
            get(Module::Ticker),
        );
        if w as f32 >= hud::WIDE {
            // In the header's middle: right of its menus, left of the
            // workspace's name, inside the header's rows.
            assert!(title.y + title.h <= top, "{w}x{h} {title:?}");
            assert!(title.x >= w as f32 * 0.40 - 0.01);
            assert!(title.x + title.w <= w as f32 - 9.0 * CELL.0 + 0.01);
        } else {
            assert!((title.y - top).abs() < 0.01);
            assert!(((title.x + title.w / 2.0) - w as f32 / 2.0).abs() < 0.01);
        }
        assert!(heart.y >= title.y + title.h);
        let right = w as f32 - 2.0 * CELL.0;
        assert!(
            (heart.x + heart.w - right).abs() < 0.01 || heart.x + heart.w <= boards.x,
            "{w}x{h}: the heart at the right, or left of the boards"
        );
        assert!(stages.y >= heart.y + heart.h && stages.x == heart.x);
        assert!((boards.x + boards.w - right).abs() < 0.01);
        assert!(legend.y >= boards.y + boards.h);
        assert!(
            ticker.y + ticker.h > bottom,
            "{w}x{h}: the ticker leaves the transcript's last row"
        );
        assert!(heart.h >= 140.0, "{w}x{h} heart {heart:?}");
    }
}

#[test]
fn a_window_too_small_floats_no_hud() {
    for (w, h) in [(600, 900), (1600, 400), (0, 0)] {
        assert!(hud::arrange(grid(w, h), CELL, 5, &Saved::default()).is_none());
    }
    // Cells the window does not know fall back to the panel's.
    assert_eq!(
        hud::arrange(grid(1600, 1000), (0.0, 0.0), 5, &Saved::default()),
        Some(hud(1600, 1000))
    );
}

#[test]
fn a_module_kept_to_an_edge_stays_by_it_when_the_window_changes() {
    let a = Anchored::at(1000.0, 200.0, 1600.0, 1.0);
    assert_eq!(a.start(200.0, 1600.0), 1000.0);
    assert_eq!(a.start(200.0, 1200.0), 600.0);
    let a = Anchored::of(700.0, 200.0, 1600.0);
    assert_eq!(a.anchor, 0.5);
    assert_eq!(a.start(200.0, 1600.0), 700.0);
    assert_eq!(Anchored::of(10.0, 200.0, 1600.0).anchor, 0.0);
    assert_eq!(Anchored::of(1390.0, 200.0, 1600.0).anchor, 1.0);
}

#[test]
fn no_file_an_old_file_or_a_broken_one_falls_back_to_the_design() {
    let designed = hud(1600, 1000);
    for bytes in [
        &b""[..],
        b"not json",
        b"[1, 2, 3]",
        b"{}",
        b"{\"version\": 0}",
        b"{\"modules\": {}}",
        b"{\"modules\": {\"nonsense\": {\"x\": {\"anchor\": 1, \"offset\": 5}}}}",
        b"{\"modules\": {\"heart\": {}}}",
        b"{\"modules\": {\"heart\": {\"x\": null, \"y\": null, \"scale\": null}}, \"extra\": true}",
    ] {
        let saved = Saved::parse(bytes);
        assert_eq!(
            hud::arrange(grid(1600, 1000), CELL, 5, &saved).unwrap(),
            designed,
            "{}",
            String::from_utf8_lossy(bytes)
        );
    }
    // A file from before a field existed keeps the rest of its design.
    let saved = Saved::parse(br#"{"modules": {"boards": {"x": {"anchor": 0, "offset": 40}}}}"#);
    let moved = hud::arrange(grid(1600, 1000), CELL, 5, &saved).unwrap();
    let b = moved.get(Module::Boards).unwrap();
    assert_eq!(b.rect.x, 40.0);
    assert_eq!(b.rect.y, designed.get(Module::Boards).unwrap().rect.y);
    assert_eq!(b.scale, 1.0);
    for m in [Module::Title, Module::Heart, Module::Ticker] {
        assert_eq!(moved.get(m), designed.get(m));
    }
}

#[test]
fn values_out_of_bounds_fall_back_or_are_held_in() {
    let saved = Saved::parse(
        br#"{"modules": {
            "heart": {"x": {"anchor": 7, "offset": 3}, "y": {"anchor": 0, "offset": 1e30}, "scale": 99},
            "title": {"scale": -4},
            "boards": {"x": {"anchor": 1, "offset": 5000}, "y": {"anchor": 1, "offset": -9000}}
        }}"#,
    );
    let designed = hud(1600, 1000);
    let h = hud::arrange(grid(1600, 1000), CELL, 5, &saved).unwrap();
    let heart = h.get(Module::Heart).unwrap();
    // A bad anchor or offset is the design; the scale is clamped, and the
    // module still fits the window.
    assert_eq!(
        heart.scale,
        hud::MAX_SCALE
            .min(1000.0 / heart.natural.1)
            .min(1600.0 / heart.natural.0)
    );
    let hc = heart.content();
    assert!(hc.x + hc.w <= 1600.01 && hc.y + hc.h <= 1000.01 && hc.y >= 0.0);
    assert_eq!(h.get(Module::Title).unwrap().scale, hud::MIN_SCALE);
    // Far off the window is pulled back until its content is inside it.
    let b = h.get(Module::Boards).unwrap();
    assert_eq!(b.rect.x, 1600.0 - b.rect.w + MARGIN);
    assert_eq!(b.rect.y, -MARGIN);
    assert_eq!(b.content().x + b.content().w, 1600.0);
    assert_eq!(b.content().y, 0.0);
    assert_eq!(h.get(Module::Ticker), designed.get(Module::Ticker));
}

#[test]
fn the_pointer_finds_a_modules_body_grip_and_hide_button() {
    let h = hud(1600, 1000);
    for m in &h.modules {
        let c = m.content();
        let centre = [c.x + c.w / 2.0, c.y + c.h / 2.0];
        assert_eq!(
            h.hit(centre, None),
            Some((m.module, Part::Body)),
            "{:?}",
            m.module
        );
        // The hide button and the tab are there only once the module is
        // under the pointer, its chrome showing.
        let close = m.close();
        let on_close = [close.x + close.w / 2.0, close.y + close.h / 2.0];
        assert_eq!(
            h.hit(on_close, Some(m.module)),
            Some((m.module, Part::Close))
        );
        assert_ne!(h.hit(on_close, None), Some((m.module, Part::Close)));
        let tab = m.tab();
        assert_eq!(
            h.hit([tab.x + 30.0, tab.y + tab.h / 2.0], Some(m.module)),
            Some((m.module, Part::Body))
        );
        let grip = m.grip();
        assert_eq!(
            h.hit([grip.x + grip.w / 2.0, grip.y + grip.h / 2.0], None),
            Some((m.module, Part::Resize)),
            "{:?}",
            m.module
        );
    }
    // The middle of the transcript is nobody's.
    assert_eq!(h.hit([500.0, 500.0], None), None);
    assert_eq!(h.hit([-5.0, -5.0], None), None);
    // The zones handed to the window: the hovered module's hide button
    // and tab, then every grip, then every body.
    let zones = h.zones(Some(Module::Heart));
    assert_eq!(zones.len(), 2 + 2 * h.modules.len());
    assert_eq!((zones[0].0, zones[0].2), (Module::Heart, Part::Close));
    assert_eq!(zones[2].2, Part::Resize);
    assert_eq!(zones.last().unwrap().2, Part::Body);
    assert_eq!(h.zones(None).len(), 2 * h.modules.len());
}

#[test]
fn dragging_a_module_moves_it_and_keeps_it_inside() {
    let h = hud(1600, 1000);
    let mut saved = Saved::default();
    let boards = *h.get(Module::Boards).unwrap();
    let c = boards.content();
    let from = [c.x + 40.0, c.y + 40.0];
    let grab = hud::press(&h, from, false, None).unwrap();
    assert_eq!((grab.module, grab.part), (Module::Boards, Part::Body));
    hud::drag(&mut saved, &h, &grab, [from[0] - 700.0, from[1] - 300.0]);
    let moved = hud::arrange(grid(1600, 1000), CELL, 5, &saved).unwrap();
    let b = moved.get(Module::Boards).unwrap();
    assert_eq!(
        (b.rect.x, b.rect.y),
        (boards.rect.x - 700.0, boards.rect.y - 300.0)
    );
    // Released, it stays and is to be kept.
    assert!(hud::release(
        &mut saved,
        &h,
        &grab,
        [from[0] - 700.0, from[1] - 300.0],
        false
    ));
    assert_eq!(
        hud::arrange(grid(1600, 1000), CELL, 5, &saved).unwrap(),
        moved
    );
    // Off the window, it stops at the edge.
    hud::drag(&mut saved, &h, &grab, [from[0] + 5000.0, from[1] + 5000.0]);
    let b = *hud::arrange(grid(1600, 1000), CELL, 5, &saved)
        .unwrap()
        .get(Module::Boards)
        .unwrap();
    let c = b.content();
    assert_eq!((c.x + c.w, c.y + c.h), (1600.0, 1000.0));
    // Near the window's margin, it settles on it.
    let target = 2.0 * CELL.0 - MARGIN + 6.0;
    hud::drag(
        &mut saved,
        &h,
        &grab,
        [from[0] + target - boards.rect.x, from[1] - 300.0],
    );
    let b = *hud::arrange(grid(1600, 1000), CELL, 5, &saved)
        .unwrap()
        .get(Module::Boards)
        .unwrap();
    assert_eq!(b.content().x, 2.0 * CELL.0);
    // Only the module dragged moved.
    let after = hud::arrange(grid(1600, 1000), CELL, 5, &saved).unwrap();
    for m in [
        Module::Title,
        Module::Heart,
        Module::Stages,
        Module::Legend,
        Module::Ticker,
    ] {
        assert_eq!(after.get(m), h.get(m));
    }
}

#[test]
fn a_click_without_a_drag_changes_nothing() {
    let h = hud(1600, 1000);
    let mut saved = Saved::default();
    let c = h.get(Module::Heart).unwrap().content();
    let at = [c.x + 30.0, c.y + 30.0];
    let grab = hud::press(&h, at, false, None).unwrap();
    assert!(!hud::release(&mut saved, &h, &grab, at, true));
    assert_eq!(saved, Saved::default());
}

#[test]
fn the_grip_or_a_modifier_resizes_from_the_top_left_corner() {
    let h = hud(1600, 1000);
    let mut saved = Saved::default();
    let heart = *h.get(Module::Heart).unwrap();
    let g = heart.grip();
    let from = [g.x + g.w / 2.0, g.y + g.h / 2.0];
    let grab = hud::press(&h, from, false, None).unwrap();
    assert_eq!(grab.part, Part::Resize);
    // Dragged in by a quarter of its width: three quarters the size.
    hud::drag(
        &mut saved,
        &h,
        &grab,
        [from[0] - heart.rect.w * 0.25, from[1] - heart.rect.h * 0.25],
    );
    let after = *hud::arrange(grid(1600, 1000), CELL, 5, &saved)
        .unwrap()
        .get(Module::Heart)
        .unwrap();
    assert!((after.scale - 0.75).abs() < 1e-4, "{}", after.scale);
    assert!(
        (after.rect.x - heart.rect.x).abs() < 0.01 && (after.rect.y - heart.rect.y).abs() < 0.01
    );
    assert!((after.rect.w - heart.rect.w * 0.75).abs() < 0.01);
    // Shrunk to nothing, it stops at the least scale.
    hud::drag(&mut saved, &h, &grab, [0.0, 0.0]);
    let after = *hud::arrange(grid(1600, 1000), CELL, 5, &saved)
        .unwrap()
        .get(Module::Heart)
        .unwrap();
    assert_eq!(after.scale, hud::MIN_SCALE);
    // Shift (or Control) on its body resizes too.
    let c = h.get(Module::Ticker).unwrap().content();
    let grab = hud::press(&h, [c.x + 20.0, c.y + 10.0], true, None).unwrap();
    assert_eq!((grab.module, grab.part), (Module::Ticker, Part::Resize));
    hud::drag(&mut saved, &h, &grab, [c.x + 20.0 + 2000.0, c.y + 10.0]);
    let t = *hud::arrange(grid(1600, 1000), CELL, 5, &saved)
        .unwrap()
        .get(Module::Ticker)
        .unwrap();
    // Grown as far as the window lets it.
    let tc = t.content();
    assert!(tc.x + tc.w <= 1600.01 && t.scale > 1.0 && t.scale <= hud::MAX_SCALE);
}

#[test]
fn the_hide_button_hides_a_module_and_reset_brings_it_back() {
    let h = hud(1600, 1000);
    let mut saved = Saved::default();
    let close = h.get(Module::Legend).unwrap().close();
    let at = [close.x + close.w / 2.0, close.y + close.h / 2.0];
    let grab = hud::press(&h, at, true, Some(Module::Legend)).unwrap();
    assert_eq!(
        grab.part,
        Part::Close,
        "a modifier never turns the hide button into a grip"
    );
    assert!(hud::release(&mut saved, &h, &grab, at, true));
    let hidden = hud::arrange(grid(1600, 1000), CELL, 5, &saved).unwrap();
    assert!(hidden.get(Module::Legend).is_none());
    assert_eq!(hidden.modules.len(), 5);
    assert_eq!(
        saved.modules.get("legend"),
        Some(&Place {
            hidden: true,
            ..Place::default()
        })
    );
    // A press on the hide button let go elsewhere hides nothing.
    let mut other = Saved::default();
    let grab = hud::press(&h, at, false, Some(Module::Legend)).unwrap();
    assert!(!hud::release(&mut other, &h, &grab, [10.0, 10.0], false));
    assert_eq!(other, Saved::default());
    // The reset is the design.
    assert_eq!(
        hud::arrange(grid(1600, 1000), CELL, 5, &Saved::default()).unwrap(),
        h
    );
}

#[test]
fn the_layout_is_kept_and_read_back_from_its_file() {
    let dir = std::env::temp_dir().join(format!("heartglass-hud-{}", std::process::id()));
    let path = dir.join("heartglass-hud.json");
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(
        Saved::load(&path),
        Saved::default(),
        "no file is the design"
    );
    let h = hud(1600, 1000);
    let mut saved = Saved::default();
    let c = h.get(Module::Title).unwrap().content();
    let grab = hud::press(&h, [c.x + 5.0, c.y + 5.0], false, None).unwrap();
    assert!(hud::release(
        &mut saved,
        &h,
        &grab,
        [c.x - 95.0, c.y + 205.0],
        false
    ));
    saved.save(&path).unwrap();
    let back = Saved::load(&path);
    assert_eq!(back.version, hud::VERSION);
    assert_eq!(
        hud::arrange(grid(1600, 1000), CELL, 5, &back),
        hud::arrange(grid(1600, 1000), CELL, 5, &saved)
    );
    // Only what was moved is written: the rest keeps its design.
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        text.contains("\"title\"") && !text.contains("\"heart\""),
        "{text}"
    );
    // A file grown past any layout's size is not read.
    std::fs::write(&path, vec![b' '; 70_000]).unwrap();
    assert_eq!(Saved::load(&path), Saved::default());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_modules_own_layout_holds_its_parts_on_its_content() {
    let h = hud(1600, 1000);
    for m in &h.modules {
        let l = hud::local_layout(m, 5);
        let c = m.local_content();
        assert_eq!(l.panel.w, m.natural.0);
        assert_eq!((c.x, c.y), (MARGIN, MARGIN));
        assert_eq!(l.rows.len(), 5);
        if m.module == Module::Boards {
            let last = l.rows.last().unwrap();
            assert!((last.y + last.h - (c.y + c.h)).abs() < 0.01);
        }
        // Mapped to the window, its content is where the HUD says.
        let [x, y, w, hh] = m.rect_at([c.x, c.y, c.w, c.h]);
        assert_eq!(super::Panel { x, y, w, h: hh }, m.content());
    }
}

#[cfg(feature = "glass")]
#[test]
fn a_modules_halo_follows_its_print_and_never_covers_the_text() {
    use super::layers::halo_for_test;
    use super::plate::{self, View};
    // A print with one block of ink in the middle-left of its content.
    let (w, h) = (200u32, 100u32);
    let mut rgba = vec![0u8; (w * 2 * h * 2 * 4) as usize];
    for y in 80..120u32 {
        for x in 60..140u32 {
            let i = ((y * w * 2 + x) * 4) as usize;
            rgba[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
        }
    }
    let content = super::Panel {
        x: MARGIN,
        y: MARGIN,
        w: w as f32 - 2.0 * MARGIN,
        h: h as f32 - 2.0 * MARGIN,
    };
    let halo = halo_for_test(&(w * 2, h * 2, rgba), (w, h), 1.0, content);
    assert_eq!((halo.0, halo.1), (w, h));
    let alpha = |x: u32, y: u32| halo.2[((y * w + x) * 4 + 3) as usize];
    let max = halo.2.chunks(4).map(|p| p[3]).max().unwrap();
    assert!(max as f32 <= 0.84 * 255.0 + 1.0, "never opaque: {max}");
    // Darkest behind the ink, a faint wash elsewhere on the content, and
    // clear in the corners of the margin.
    assert!(
        alpha(50, 50) > alpha(160, 50) + 40,
        "{} {}",
        alpha(50, 50),
        alpha(160, 50)
    );
    assert!(alpha(160, 50) > 20);
    assert_eq!(alpha(0, 0), 0);
    assert_eq!(alpha(w - 1, h - 1), 0);
    // A module's print is its natural size at its scale, at twice the
    // shown pixels.
    let hud = hud(1600, 1000);
    struct Blocks;
    impl plate::Typeset for Blocks {
        fn text(&mut self, line: &str, size: f32, ink: [u8; 3], _bold: bool) -> Option<plate::Ink> {
            let w = (line.chars().count() as f32 * size * 0.5).round() as u32;
            let h = size.round() as u32;
            (w > 0 && h > 0).then(|| (w, h, [ink[0], ink[1], ink[2], 255].repeat((w * h) as usize)))
        }
    }
    let view = View {
        campaign: None,
        cloud: None,
        dir: "~/.angelX/vec",
    };
    for m in &hud.modules {
        let l = hud::local_layout(m, 5);
        let ink = plate::module(m.module, &l, &view, 2.0 * 1.5, &mut Blocks);
        assert_eq!(
            (ink.0, ink.1),
            (
                (m.natural.0 * 3.0).ceil() as u32,
                (m.natural.1 * 3.0).ceil() as u32
            ),
            "{:?}",
            m.module
        );
    }
}
