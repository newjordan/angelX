//! Website plate: the world renderer's own output, saved as PNG.
//!
//! `render_scene` is the production rasterizer — the same function the ride and
//! the region stages call — so this writes real frames, not stand-ins. Inert
//! unless `ANGELX_FRAMES_DIR` is set.
//!
//! ```sh
//! ANGELX_FRAMES_DIR=/tmp/angelx-frames cargo test -p angelX-cockpit \
//!   --no-default-features frames_world_rasters -- --nocapture
//! ```

use super::*;
use super::super::scene;

fn view(pos: V3, heading_rad: f32, pitch: f32, fov_rad: f32) -> View3 {
    View3 {
        pos,
        heading_rad,
        pitch,
        fov_rad,
    }
}

#[test]
fn frames_world_rasters() {
    let Some(dir) = std::env::var_os("ANGELX_FRAMES_DIR").map(std::path::PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let court = scene::scene_for(scene::SceneKey::COURT);

    let plates = [
        // The realm seen from the ride's proving vantage: eye at ground level,
        // looking straight down the court.
        (
            "fig03-world.png",
            view(v3(0.0, 0.0, 0.62), 0.0, 0.0, 1.05),
            1080,
            680,
        ),
        // The same mesh from a raised, gently tilted vantage — the ride camera
        // lifted above the ground plane, still looking down the court.
        (
            "figC-world-overhead.png",
            view(v3(0.0, -0.35, 1.9), 0.0, -0.34, 0.98),
            1080,
            680,
        ),
    ];

    for (name, view, w, h) in plates {
        let frame = render_scene(&court, &view, w, h);
        frame.save(dir.join(name)).unwrap();
    }
}
