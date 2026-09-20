use super::*;

fn cells(text: &Text<'_>) -> usize {
    text.lines.iter().map(|l| l.spans.len()).sum()
}

#[test]
fn render_has_requested_dimensions() {
    let t = render(0.0, 20, 8);
    assert_eq!(t.lines.len(), 8);
    assert!(t.lines.iter().all(|l| l.spans.len() == 20));
    // Two pixels per cell are encoded as fg/bg of one half-block glyph.
    assert!(t.lines[0].spans.iter().all(|s| s.content == HALF_BLOCK));
}

#[test]
fn zero_area_is_empty() {
    assert_eq!(cells(&render(1.0, 0, 8)), 0);
    assert_eq!(cells(&render(1.0, 20, 0)), 0);
}

#[test]
fn cube_is_actually_hit_on_screen() {
    // A centered ray (down -Z) must strike the cube and be lit, not background.
    let spin = Spin::at(0.0);
    let local_origin = spin.to_local(V3::new(0.0, 0.0, CAM_DIST));
    let dir = spin.to_local(V3::new(0.0, 0.0, -1.0)).normalize();
    assert!(
        intersect_cube(local_origin, dir).is_some(),
        "the eye ray through the center should hit the cube"
    );
}

#[test]
fn rays_that_miss_return_none() {
    // Straight up from the eye never meets the box.
    let dir = V3::new(0.0, 1.0, 0.0);
    assert!(intersect_cube(V3::new(0.0, 0.0, CAM_DIST), dir).is_none());
}

#[test]
fn frame_contains_both_cube_and_background() {
    // Over a full frame some pixels hit the saturated cube and some land on the
    // dim backdrop — i.e. the cube neither fills nor misses the whole window.
    let t = render(0.7, 40, 16);
    let mut saw_bright = false;
    let mut saw_dim = false;
    for line in &t.lines {
        for span in &line.spans {
            if let Some(Color::Rgb(r, g, b)) = span.style.fg {
                let lum = r as u32 + g as u32 + b as u32;
                saw_bright |= lum > 360;
                saw_dim |= lum < 60;
            }
        }
    }
    assert!(saw_bright, "expected lit cube pixels");
    assert!(saw_dim, "expected dim background pixels");
}

#[test]
fn extremes_do_not_panic() {
    let _ = render(-3.0, 1, 1);
    let _ = render(9999.0, 80, 40);
    let _ = render(f32::NAN, 10, 10);
}

#[test]
fn hsv_primaries_are_correct() {
    assert_eq!(hsv_to_rgb(0.0, 1.0, 1.0), (1.0, 0.0, 0.0)); // red
    let (r, g, b) = hsv_to_rgb(1.0 / 3.0, 1.0, 1.0); // green
    assert!(g > 0.99 && r < 0.01 && b < 0.01);
}
