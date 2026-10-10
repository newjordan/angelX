use super::*;
use crate::drive::{
    chivalry::{Mount, Phase},
    together_realm::Realm,
    together_shooter::{RoomKind, TILE_UNITS, Tile},
};
use ratatui::{
    Terminal,
    backend::TestBackend,
    crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
};
fn app() -> App {
    App::preview(crate::ui::viewer::Viewer::static_preview())
}
fn paint(app: &mut App, w: u16, h: u16) -> ratatui::buffer::Buffer {
    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
    terminal
        .draw(|frame| crate::ui::draw::ui(frame, app))
        .unwrap();
    terminal.backend().buffer().clone()
}
fn labels(buffer: &ratatui::buffer::Buffer) -> String {
    buffer.content.iter().map(|c| c.symbol()).collect()
}
#[test]
fn chivalry_commands_preserve_live_floor_pause_and_link_projection_no_rewards() {
    let _guard = crate::tests::env_lock();
    let mut a = app();
    a.start_shooter(None);
    // Practice can pause a live non-home floor without swapping its map.
    a.dungeon.shooter = Some(crate::drive::together_shooter::Run::new(11, 7, None));
    a.sync_chivalry_projection();
    let floor = serde_json::to_value(&a.dungeon.shooter.as_ref().unwrap().dungeon).unwrap();
    let tick = a.dungeon.shooter.as_ref().unwrap().tick;
    paint(&mut a, 100, 35);
    a.shooter_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
    assert!(!a.dungeon.held.is_empty());
    let treasury = a.realm().treasury.clone();
    assert!(
        a.dungeon_command(Some("stable select cinder"))
            .contains("Cinder selected")
    );
    assert!(a.dungeon_command(Some("stable tend")).contains("brushed"));
    let state = a.realm().chivalry.clone();
    assert_eq!(a.world.chivalry, state);
    assert_eq!(
        a.dungeon
            .shooter
            .as_ref()
            .unwrap()
            .chivalry
            .as_ref()
            .unwrap()
            .selected,
        Mount::Cinder
    );
    assert!(a.dungeon_command(Some("stable")).contains("Stables"));
    assert!(a.dungeon.chivalry_visit.is_some());
    assert!(a.dungeon.held.is_empty());
    assert!(!a.dungeon_wants_fast_tick());
    a.advance_shooter_at(std::time::Instant::now() + std::time::Duration::from_secs(4));
    assert_eq!(a.dungeon.shooter.as_ref().unwrap().tick, tick);
    assert!(labels(&paint(&mut a, 100, 35)).contains("HOST-LOCAL PRACTICE"));
    assert!(
        a.dungeon_command(Some("tournament start"))
            .contains("pass 1/3")
    );
    assert!(
        a.dungeon_command(Some("tournament round 2 aim"))
            .contains("Expected pass 1")
    );
    a.dungeon_command(Some("tournament round 1 guard"));
    let one = a.world.chivalry.clone();
    assert!(
        a.dungeon_command(Some("tournament round 1 guard"))
            .contains("Expected pass 2")
    );
    assert_eq!(a.world.chivalry, one);
    // OS repeat cannot submit the next pass on a held G/A/C key.
    let mut repeat = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
    repeat.kind = KeyEventKind::Repeat;
    a.shooter_key(repeat);
    assert_eq!(a.world.chivalry, one);
    a.dungeon_command(Some("tournament round 2 aim"));
    a.dungeon_command(Some("tournament round 3 charge"));
    assert_eq!(a.realm().chivalry.tournament.phase, Phase::Finished);
    assert_eq!(a.realm().treasury, treasury);
    assert!(a.realm().home.trophies.is_empty());
    assert!(labels(&paint(&mut a, 100, 35)).contains("RESULT"));
    a.dungeon_command(Some("tournament leave"));
    assert!(a.dungeon.chivalry_visit.is_none());
    assert_eq!(
        serde_json::to_value(&a.dungeon.shooter.as_ref().unwrap().dungeon).unwrap(),
        floor
    );
    a.dungeon_command(Some("stable"));
    a.collapse_dungeon();
    assert!(a.dungeon.chivalry_visit.is_none());
    a.dungeon_command(Some("stable"));
    a.start_shooter(None);
    assert!(a.dungeon.chivalry_visit.is_none());
    a.dungeon_command(Some("stable"));
    a.dungeon_command(Some("off"));
    assert!(a.dungeon.chivalry_visit.is_none());
    assert!(a.world.chivalry_visit.is_none());
    assert!(a.thinking.is_none() && a.pending_turn.is_none());
}
#[test]
fn chivalry_invalid_actions_no_io_failed_save_rollback_and_owner_boundary() {
    let _guard = crate::tests::env_lock();
    let mut a = app();
    let before = a.realm().chivalry.clone();
    for command in [
        "stable select ghost",
        "stable tend twice",
        "tournament round -1 guard",
        "tournament round 1 unknown",
        "tournament start extra",
    ] {
        a.dungeon_command(Some(command));
        assert_eq!(a.realm().chivalry, before);
        assert!(a.world.chivalry_visit.is_none());
        assert!(a.dungeon.chivalry_visit.is_none());
    }
    let temp = std::env::temp_dir().join(format!("chivalry-owner-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();
    let rewards = temp.join("owner.json");
    a.dungeon.realm = Some(Realm::beside(Some(&rewards)));
    a.dungeon_command(Some("stable select mist"));
    a.dungeon_command(Some("stable tend"));
    a.dungeon_command(Some("tournament start"));
    a.dungeon_command(Some("tournament round 1 guard"));
    let saved = Realm::beside(Some(&rewards));
    assert_eq!(saved.chivalry, a.world.chivalry);
    let other = Realm::beside(Some(&temp.join("other.json")));
    assert_eq!(other.chivalry, Default::default());
    // Atomic existing writer failure must not publish a practice change.
    let bad = temp.join("not-a-directory");
    std::fs::write(&bad, b"x").unwrap();
    a.dungeon.realm = Some(Realm::beside(Some(&bad.join("owner.json"))));
    a.sync_chivalry_projection();
    let old = a.world.chivalry.clone();
    assert!(
        a.dungeon_command(Some("stable select cinder"))
            .contains("not committed")
    );
    assert_eq!(a.realm().chivalry, old);
    assert_eq!(a.world.chivalry, old);
    std::fs::remove_dir_all(temp).unwrap();
}
#[test]
fn chivalry_guest_protocol_restrictions_and_private_render_projection() {
    let _guard = crate::tests::env_lock();
    let mut host = app();
    host.start_shooter(None);
    host.dungeon_command(Some("stable select cinder"));
    host.dungeon_command(Some("stable tend"));
    assert!(
        host.dungeon_command(Some("host 127.0.0.1:0"))
            .contains("join")
    );
    assert!(host.dungeon.shooter.as_ref().unwrap().chivalry.is_none());
    assert!(!host.dungeon.notice.contains("Cinder") && !host.dungeon.notice.contains("tended"));
    let state = host.realm().chivalry.clone();
    assert!(
        host.dungeon_command(Some("tournament start"))
            .contains("Host-local practice is unavailable")
    );
    assert_eq!(host.realm().chivalry, state);
    let url = host.dungeon.guest.as_ref().unwrap().invitation_url();
    let mut guest = app();
    assert!(guest.join_delve(&url).contains("Joined"));
    assert_eq!(guest.world.chivalry, Default::default());
    assert!(
        guest
            .dungeon_command(Some("stable status"))
            .contains("host-only")
    );
    assert!(
        guest
            .dungeon_command(Some("tournament start"))
            .contains("host-only")
    );
    guest.input = "/world visit stables".into();
    guest.submit();
    assert!(guest.world.chivalry_visit.is_none());
    assert!(guest.leave_delve().contains("left"));
    host.dungeon_command(Some("off"));
}
#[test]
fn chivalry_restored_run_projection_is_rehydrated_without_restoring_side_visit() {
    let _guard = crate::tests::env_lock();
    let mut a = app();
    a.start_shooter(None);
    a.dungeon_command(Some("stable select mist"));
    a.dungeon_command(Some("stable tend"));
    a.dungeon_command(Some("stable"));
    let encoded = serde_json::to_vec(a.dungeon.shooter.as_ref().unwrap()).unwrap();
    let mut restored = app();
    restored.dungeon.realm = Some(a.realm().clone());
    restored.dungeon.shooter = Some(serde_json::from_slice(&encoded).unwrap());
    assert!(
        restored
            .dungeon
            .shooter
            .as_ref()
            .unwrap()
            .chivalry
            .is_none()
    );
    restored.advance_shooter_at(std::time::Instant::now());
    assert_eq!(
        restored
            .dungeon
            .shooter
            .as_ref()
            .unwrap()
            .chivalry
            .as_ref()
            .unwrap()
            .selected,
        Mount::Mist
    );
    assert!(restored.dungeon.chivalry_visit.is_none() && restored.world.chivalry_visit.is_none());
}

#[test]
fn chivalry_world_keyboard_travel_and_explicit_visit_beat_open_delve() {
    let _guard = crate::tests::env_lock();
    let mut a = app();
    a.input = "/world visit stables".into();
    a.submit();
    a.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(a.world.chivalry_visit.unwrap().inside);
    a.on_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    assert_eq!(a.world.chivalry_visit.unwrap().station, 1);
    // Unknown travel never closes or enters another practice scene.
    let visit = a.world.chivalry_visit;
    a.input = "/world visit impossible-building".into();
    a.submit();
    assert_eq!(a.world.chivalry_visit, visit);
    a.input = "/world visit school-study".into();
    a.submit();
    assert!(a.world.chivalry_visit.is_none());
    a.start_shooter(None);
    let floor = serde_json::to_value(&a.dungeon.shooter.as_ref().unwrap().dungeon).unwrap();
    a.dungeon_command(Some("stable"));
    assert!(a.dungeon.chivalry_visit.is_some());
    a.input = "/world visit tournament".into();
    a.submit();
    assert!(!a.dungeon.expanded && a.dungeon.chivalry_visit.is_none());
    assert_eq!(
        a.world.chivalry_visit.unwrap().place,
        crate::drive::chivalry::Place::Tournament
    );
    assert_eq!(
        serde_json::to_value(&a.dungeon.shooter.as_ref().unwrap().dungeon).unwrap(),
        floor
    );
    a.input = "/world follow".into();
    a.submit();
    assert!(a.world.chivalry_visit.is_none());
    // Same-owner command projection without a live Delve uses the mini-world.
    let mut b = app();
    b.dungeon_command(Some("stable select mist"));
    assert!(b.world.chivalry_visit.unwrap().inside);
    assert!(b.dungeon.shooter.is_none());
    b.dungeon_command(Some("stable leave"));
    assert!(b.world.chivalry_visit.is_none());
}
#[test]
fn chivalry_home_yard_entrances_are_connected_walkable_and_change_live_native_pixels() {
    let _guard = crate::tests::env_lock();
    let mut a = app();
    a.start_shooter(None);
    for kind in [RoomKind::Home, RoomKind::Yard] {
        let run = a.dungeon.shooter.as_mut().unwrap();
        let idx = run
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == kind)
            .unwrap();
        run.enter_for_test(idx);
        let portals = crate::drive::together_shooter::chivalry::portals(kind).unwrap();
        for (place, x, y) in portals {
            assert!(matches!(
                run.room().tile(x as i32, y as i32),
                Tile::Floor | Tile::Door
            ));
            // BFS over actual room collision tiles from the south entry to
            // both portals: no new disconnected signs or blocked approaches.
            let room = run.room();
            let start = (12, room.rows as i32 - 2);
            let mut queue = std::collections::VecDeque::from([start]);
            let mut seen = std::collections::BTreeSet::from([start]);
            while let Some((cx, cy)) = queue.pop_front() {
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let p = (cx + dx, cy + dy);
                    if matches!(room.tile(p.0, p.1), Tile::Floor | Tile::Door) && seen.insert(p) {
                        queue.push_back(p);
                    }
                }
            }
            assert!(seen.contains(&(x as i32, y as i32)));
            let hero = run.players.get_mut(&1).unwrap();
            hero.x = x * TILE_UNITS;
            hero.y = y * TILE_UNITS;
            assert_eq!(
                crate::drive::together_shooter::chivalry::near_portal(run, 1),
                Some(place)
            );
        }
        let before = crate::stage::world_viz::overworld::arena::frame_for(run, 384, 224, None);
        run.chivalry.as_mut().unwrap().selected = Mount::Mist;
        let after = crate::stage::world_viz::overworld::arena::frame_for(run, 384, 224, None);
        assert_ne!(before.rgba_bytes(), after.rgba_bytes());
        run.chivalry.as_mut().unwrap().selected = Mount::Bramble;
        let encoded = serde_json::to_string(run).unwrap();
        assert!(!encoded.contains("chivalry"));
        let restored: crate::drive::together_shooter::Run = serde_json::from_str(&encoded).unwrap();
        assert!(restored.chivalry.is_none());
    }
}
#[test]
fn chivalry_real_delve_and_mini_world_surface_captures_hidden_and_tiny_no_work() {
    let _guard = crate::tests::env_lock();
    let dir = std::env::var_os("ANGEL_CHIVALRY_REVIEW").map(std::path::PathBuf::from);
    if let Some(d) = &dir {
        std::fs::create_dir_all(d).unwrap();
    }
    let _backdrop = crate::tests::TestEnvGuard::set("ANGEL_BACKDROP", "in-process");
    let _map = crate::tests::TestEnvGuard::set("ANGEL_WORLD_MAP", "0");
    let mut a = app();
    a.visual_motion = crate::ui::viz::lifecycle_viz::MotionMode::Off;
    for name in [
        "mini-stables",
        "mini-stable-inside",
        "mini-lists",
        "delve-home",
        "delve-yard",
        "delve-stable",
        "delve-tournament-ready",
        "delve-tournament-running",
        "delve-tournament-result",
    ] {
        match name {
            "mini-stables" => {
                a.input = "/world visit stables".into();
                a.submit();
                assert!(a.world.chivalry_visit.is_some());
            }
            "mini-stable-inside" => {
                a.world.enter_interior();
            }
            "mini-lists" => {
                a.input = "/world visit tournament".into();
                a.submit();
            }
            "delve-home" => {
                a.start_shooter(None);
            }
            "delve-yard" => {
                let run = a.dungeon.shooter.as_mut().unwrap();
                let i = run
                    .dungeon
                    .rooms
                    .iter()
                    .position(|r| r.kind == RoomKind::Yard)
                    .unwrap();
                run.enter_for_test(i);
            }
            "delve-stable" => {
                a.dungeon_command(Some("stable select cinder"));
                a.dungeon_command(Some("stable tend"));
                a.dungeon_command(Some("stable"));
            }
            "delve-tournament-ready" => {
                a.dungeon_command(Some("tournament"));
            }
            "delve-tournament-running" => {
                a.dungeon_command(Some("tournament start"));
                a.dungeon_command(Some("tournament round 1 guard"));
            }
            _ => {
                a.dungeon_command(Some("tournament round 2 aim"));
                a.dungeon_command(Some("tournament round 3 charge"));
            }
        }
        if matches!(name, "delve-home" | "delve-yard") {
            let run = a.dungeon.shooter.as_ref().unwrap();
            for (w, h) in [(384, 224), (48, 28)] {
                let pixels = crate::stage::world_viz::overworld::arena::frame_for(run, w, h, None);
                let mut baseline = run.clone();
                baseline.chivalry = None;
                let before =
                    crate::stage::world_viz::overworld::arena::frame_for(&baseline, w, h, None);
                assert_ne!(pixels.rgba_bytes(), before.rgba_bytes());
                if let Some(d) = &dir {
                    save_pixels(&pixels, &d.join(format!("native-{name}-{w}x{h}.png")));
                    save_pixels(
                        &before,
                        &d.join(format!("native-{name}-before-{w}x{h}.png")),
                    );
                }
            }
        }
        for (w, h) in [(120, 40), (48, 16), (8, 4), (0, 0)] {
            let buffer = paint(&mut a, w, h);
            if name.starts_with("mini-") && w == 120 {
                assert!(labels(&buffer).contains("/dungeon"));
            }
            if name == "delve-stable" && w == 120 {
                let text = labels(&buffer);
                assert_eq!(text.matches("Cinder selected").count(), 1);
                assert!(
                    !text.contains("Stables · selected"),
                    "full duplicated status removed"
                );
                assert!(text.contains("E tend") && text.contains("Esc leave"));
                let image_rows = (0..h)
                    .filter(|y| {
                        (0..w).any(|x| {
                            let glyph = buffer[(x, *y)].symbol().chars().next().unwrap_or(' ');
                            (0x2801..=0x28ff).contains(&(glyph as u32))
                        })
                    })
                    .count();
                assert!(
                    image_rows >= 34,
                    "stable reclaimed the duplicated status rows"
                );
            }
            if let Some(d) = &dir
                && w > 0
                && h > 0
            {
                save_cells(&buffer, &d.join(format!("ui-{name}-{w}x{h}.png")));
            }
        }
    }
    a.dungeon_command(Some("off"));
    a.input = "/world off".into();
    a.submit();
    assert!(a.world.chivalry_visit.is_none());
    // The ordinary shared compose gate does not execute the renderer at all
    // for a hidden surface (including any optional native transport).
    let called = std::cell::Cell::new(false);
    assert!(
        crate::ui::draw::maybe_run_expensive_world_compose(false, || {
            called.set(true);
            a.world
                .scryglass_frame_with_motion(72, 26, false, 0.0, 0.0, 1.05, a.visual_motion)
        })
        .is_none()
    );
    assert!(!called.get());
    assert!(a.thinking.is_none() && a.pending_turn.is_none());
}
fn save_cells(buffer: &ratatui::buffer::Buffer, path: &std::path::Path) {
    use ratatui::style::Color;
    let rgb = |c: Color| match c {
        Color::Rgb(r, g, b) => [r, g, b, 255],
        _ => [10, 13, 20, 255],
    };
    let (w, h) = (buffer.area.width as u32, buffer.area.height as u32);
    let mut image = image::RgbaImage::from_pixel(w * 8, h * 16, image::Rgba([10, 13, 20, 255]));
    let mut metadata = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let c = buffer.cell((x as u16, y as u16)).unwrap();
            let glyph = c.symbol().chars().next().unwrap_or(' ');
            metadata.push(serde_json::json!([x, y, c.symbol(), rgb(c.fg), rgb(c.bg)]));
            for dy in 0..16 {
                for dx in 0..8 {
                    let mut ink = rgb(c.bg);
                    if (0x2800..=0x28ff).contains(&(glyph as u32)) {
                        let lx = (dx / 4) as usize;
                        let ly = (dy / 4) as usize;
                        if ((glyph as u32 - 0x2800) as u8)
                            & crate::ui::term::art::braille_dot_bit(lx, ly)
                            != 0
                            && dx % 4 > 0
                            && dy % 4 > 0
                        {
                            ink = rgb(c.fg);
                        }
                    } else if glyph == '▀' {
                        ink = rgb(if dy < 8 { c.fg } else { c.bg });
                    }
                    image.put_pixel(x * 8 + dx, y * 16 + dy, image::Rgba(ink));
                }
            }
        }
    }
    image.save(path).unwrap();
    // Text glyphs are preserved verbatim in a companion cell dump; a retained
    // font conversion after the gate makes text readable, not invented pixels.
    std::fs::write(
        path.with_extension("cells.json"),
        serde_json::to_vec(&metadata).unwrap(),
    )
    .unwrap();
}

fn save_pixels(pixels: &crate::stage::world_viz::overworld::Img, path: &std::path::Path) {
    image::RgbaImage::from_raw(pixels.w as u32, pixels.h as u32, pixels.rgba_bytes())
        .unwrap()
        .save(path)
        .unwrap();
}
