//! Offline six-seat measurements, shared by the before/after art review.
use super::*;
use crate::rules::{Run, TILE_UNITS};
use std::collections::BTreeMap;
use std::time::Instant;

#[test]
#[ignore = "writes PNGs and timing evidence; set ANGEL_TAVERN_REVIEW"]
fn write_tavern_quality_review() {
    let Some(dir) = std::env::var_os("ANGEL_TAVERN_REVIEW") else { return };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let label = std::env::var("ANGEL_TAVERN_LABEL").unwrap_or_else(|_| "review".into());
    let mut run = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    for id in 3..=6 { run.join_seat(id, &format!("Seat {id}")); }
    run.walk_alehouse();
    run.floor_plates.clear();
    for _ in 0..100 { run.step(&BTreeMap::new()); }
    let positions = [(10.5, 6.5), (5.2, 7.2), (23.1, 7.2), (9.3, 16.8), (18.0, 11.0), (30.0, 17.0)];
    for (id, (x, y)) in (1..=6).zip(positions) {
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y) = (x * TILE_UNITS, y * TILE_UNITS);
    }
    assert!(run.valid_snapshot());
    let (w, h) = seat_size();
    let (w, h) = (w as i32, h as i32);
    let save = |name: &str, im: Img| {
        image::RgbaImage::from_raw(im.w as u32 * 3, im.h as u32 * 3, im.rgba_scaled(3))
            .unwrap().save(dir.join(format!("{label}-{name}.png"))).unwrap();
    };
    for id in 1..=6 { save(&format!("seat-{id}"), seat_scene(&run, w, h, Some(id)).compose()); }
    let mut whole = (*scenery_cached(&run)).clone();
    whole.stamp(&room_layer(&run, None, true), 0, 0);
    save("hall", whole);
    let mut rack = run.clone();
    let (x, y) = crate::rules::alehouse::arm_stand(0);
    let hero = rack.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (x * TILE_UNITS, y * TILE_UNITS);
    save("wall-seat", seat_scene(&rack, w, h, Some(1)).compose());

    // Warm the exact path before measuring. No PNG work inside render timing.
    for id in 1..=6 { std::hint::black_box(seat_scene(&run, w, h, Some(id)).compose()); }
    let samples = 24;
    let mut render = 0;
    let mut png_time = 0;
    let mut png_bytes = 0;
    let mut state_time = 0;
    let mut snapshot_bytes = 0;
    let mut delta_bytes = 0;
    let mut writer = angel_net::doc::wire::Writer::default();
    let mut old = crate::wire::publish(&run).unwrap();
    let mut bytes = Vec::new();
    writer.value(&mut bytes, &old);
    let first_snapshot = bytes.len();
    for _ in 0..samples {
        run.step(&BTreeMap::new());
        let now = Instant::now();
        let new = crate::wire::publish(&run).unwrap();
        bytes.clear();
        writer.value(&mut bytes, &new);
        snapshot_bytes += bytes.len();
        bytes.clear();
        if let Some(patch) = angel_net::doc::delta::diff(&old, &new) { writer.patch(&mut bytes, &patch); }
        delta_bytes += bytes.len();
        state_time += now.elapsed().as_nanos();
        old = new;
        for id in 1..=6 {
            let now = Instant::now();
            let im = seat_scene(&run, w, h, Some(id)).compose();
            render += now.elapsed().as_nanos();
            let rgba = image::RgbaImage::from_raw(im.w as u32, im.h as u32, im.rgba_scaled(1)).unwrap();
            let now = Instant::now();
            let mut out = std::io::Cursor::new(Vec::new());
            rgba.write_to(&mut out, image::ImageFormat::Png).unwrap();
            png_time += now.elapsed().as_nanos();
            png_bytes += out.get_ref().len();
        }
    }
    let seats = samples * 6;
    let text = format!("label,width,height,samples,render_us_per_seat,png_encode_us_per_seat,png_bytes_per_seat,state_publish_snapshot_delta_us_per_tick,first_snapshot_bytes,warm_snapshot_bytes_per_tick,delta_bytes_per_tick\n{label},{w},{h},{samples},{:.2},{:.2},{:.2},{:.2},{first_snapshot},{:.2},{:.2}\n", render as f64 / seats as f64 / 1000.0, png_time as f64 / seats as f64 / 1000.0, png_bytes as f64 / seats as f64, state_time as f64 / samples as f64 / 1000.0, snapshot_bytes as f64 / samples as f64, delta_bytes as f64 / samples as f64);
    std::fs::write(dir.join(format!("{label}-metrics.csv")), &text).unwrap();
    println!("{text}");

    // Actual browser transport: one publication, six private seat trees,
    // warm connection dictionaries and swept, moving knight courses.
    let mut moving = run.clone();
    let mut publisher = crate::wire::Publisher::default();
    let mut writers: Vec<_> = (0..6).map(|_| angel_net::doc::wire::Writer::default()).collect();
    let first = publisher.publish(&moving).unwrap();
    let mut previous: Vec<_> = (0..6).map(|seat| crate::wire::seat_tree(&first, Some(seat)).into_owned()).collect();
    for (writer, tree) in writers.iter_mut().zip(&previous) { bytes.clear(); writer.value(&mut bytes, tree); }
    let mut sim_ns = 0;
    let mut wire_ns = 0;
    let mut moving_bytes = 0;
    let mut moved = [false; 6];
    for tick in 0..60 {
        let inputs = (1..=6).map(|id| (id, crate::rules::Input {
            move_x: if tick % 20 < 10 { 1 } else { -1 },
            move_y: if id % 2 == 0 { 1 } else { -1 },
            ..Default::default()
        })).collect();
        let before: Vec<_> = moving.players.values().map(|h| (h.x, h.y)).collect();
        let now = Instant::now();
        moving.step(&inputs);
        sim_ns += now.elapsed().as_nanos();
        for (i, (hero, old)) in moving.players.values().zip(before).enumerate() {
            moved[i] |= (hero.x, hero.y) != old;
        }
        let now = Instant::now();
        let tree = publisher.publish(&moving).unwrap();
        for seat in 0..6 {
            let current = crate::wire::seat_tree(&tree, Some(seat as u32)).into_owned();
            bytes.clear();
            if let Some(patch) = angel_net::doc::delta::diff(&previous[seat], &current) {
                writers[seat].patch(&mut bytes, &patch);
            }
            moving_bytes += bytes.len();
            previous[seat] = current;
        }
        wire_ns += now.elapsed().as_nanos();
    }
    assert!(moved.iter().all(|m| *m), "every knight must actually walk");
    assert!(moving.valid_snapshot());
    for id in 1..=6 { save(&format!("walking-seat-{id}"), seat_scene(&moving, w, h, Some(id)).compose()); }
    let wire = format!("label,ticks,sim_us_per_tick,publish_six_seat_delta_encode_us_per_tick,delta_bytes_per_seat_tick\n{label},60,{:.2},{:.2},{:.2}\n", sim_ns as f64 / 60.0 / 1000.0, wire_ns as f64 / 60.0 / 1000.0, moving_bytes as f64 / 360.0);
    std::fs::write(dir.join(format!("{label}-moving-wire.csv")), &wire).unwrap();
    println!("{wire}");
}

#[test]
fn tavern_keeps_static_scenery_across_dynamic_flame_steps() {
    let _pin = scenery::pin_scene_cache();
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    run.walk_alehouse();
    let before = scenery_cached(&run);
    run.tick += 6;
    let after = scenery_cached(&run);
    assert!(std::sync::Arc::ptr_eq(&before, &after), "static tavern timber, wall and lights must not rebake at 5Hz");
}
