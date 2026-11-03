use super::*;

fn belfry() -> Run {
    let mut run = Run::at_home(
        17,
        1,
        None,
        crate::rules::home::Home::default(),
        Default::default(),
    );
    let room = super::world::room_of(&run.dungeon, RoomKind::Belfry).unwrap();
    run.arrive(room, Game::Volley.start());
    run.step(&BTreeMap::new());
    run.step(&BTreeMap::new());
    assert!(run.pastimes.round.is_some());
    assert!(
        run.pastimes
            .round
            .as_ref()
            .unwrap()
            .volley
            .as_ref()
            .unwrap()
            .ball
            .is_some()
    );
    assert!(
        run.pastimes.heat.is_some(),
        "a solo rally still uses the shared heat path"
    );
    run
}

/// An ordinary input bot: predict the crossing, move to a contact point,
/// aim with arrows, and tap F. It never changes the ball, score, or clock.
fn bounce_wall(ball: &mut volley::Ball) {
    if ball.x < COURT_LEFT {
        ball.x = 2 * COURT_LEFT - ball.x;
        ball.dx = ball.dx.abs();
    } else if ball.x > COURT_RIGHT {
        ball.x = 2 * COURT_RIGHT - ball.x;
        ball.dx = -ball.dx.abs();
    }
}

/// Return the exact paddle crossing and the post-step ball used by the rules.
fn paddle_crossing(ball: &volley::Ball) -> (i32, i32, volley::Ball) {
    let mut ball = ball.clone();
    for ticks in 1..=64 {
        let (old_x, old_y) = (ball.x, ball.y);
        ball.x += ball.dx;
        ball.y += ball.dy;
        bounce_wall(&mut ball);
        if ball.y >= PADDLE_Y {
            let at = old_x + (PADDLE_Y - old_y) * ball.dx / ball.dy;
            return (ticks, at, ball);
        }
    }
    panic!("a descending ball must cross the paddle");
}

/// Play the projected perfect return through the same wall/top intersections.
fn projected_bell(
    ball: &volley::Ball,
    at: i32,
    paddle: i32,
    aim: i8,
    bells: u8,
    combo: u8,
) -> (usize, i32) {
    let mut ball = volley::Ball {
        x: at.clamp(COURT_LEFT, COURT_RIGHT),
        y: 2 * PADDLE_Y - ball.y,
        dx: Volley::return_dx(at, paddle, aim.saturating_mul(2), ball.dx),
        dy: -(Volley::speed(bells, combo + 1) + 16),
        served: false,
    };
    for _ in 0..64 {
        let (old_x, old_y) = (ball.x, ball.y);
        ball.x += ball.dx;
        ball.y += ball.dy;
        bounce_wall(&mut ball);
        if ball.y <= COURT_TOP {
            let at_top = old_x + (old_y - COURT_TOP) * ball.dx / -ball.dy;
            let bell = ((at_top - COURT_LEFT) / (3 * 256)).clamp(0, 3) as usize;
            return (bell, at_top);
        }
    }
    panic!("a returned ball must reach the bells");
}

fn rally_input(run: &Run) -> BTreeMap<u32, Input> {
    let v = run
        .pastimes
        .round
        .as_ref()
        .unwrap()
        .volley
        .as_ref()
        .unwrap();
    let (mut target, mut aim_x) = (v.paddle, 0);
    let mut crossing = None;
    if let Some(b) = v.ball.as_ref().filter(|b| b.dy > 0 && b.y < PADDLE_Y) {
        let (ticks, at, projected) = paddle_crossing(b);
        crossing = Some((ticks, at));
        let next = v.next_bell().unwrap_or(0);
        let goal = BELL_X[next];
        let mut best: Option<((bool, i32, i32), i32, i8)> = None;
        // All candidates stay on the paddle's actual 40-unit movement grid.
        for k in -40..=40 {
            let candidate = 12 * 256 + 40 * k;
            if !(COURT_LEFT..=COURT_RIGHT).contains(&candidate) {
                continue;
            }
            let paddle = v.paddle + (candidate - v.paddle).clamp(-40 * ticks, 40 * ticks);
            if (at - paddle).abs() > PADDLE_REACH {
                continue;
            }
            for aim in [-1i8, 0, 1] {
                let (bell, at_top) = projected_bell(&projected, at, paddle, aim, v.bells, v.combo);
                let rank = (
                    bell != next,
                    (candidate - v.paddle).abs(),
                    (at_top - goal).abs(),
                );
                if best.as_ref().is_none_or(|(old, _, _)| rank < *old) {
                    best = Some((rank, candidate, aim));
                }
            }
        }
        if let Some((_, paddle, aim)) = best {
            (target, aim_x) = (paddle, aim);
        } else {
            // No shaped return is reachable yet: at least chase the ball.
            let k = (at - 12 * 256).div_euclid(40);
            target = (12 * 256 + 40 * k).clamp(COURT_LEFT, COURT_RIGHT);
            aim_x = (goal - at).signum() as i8;
        }
    }
    let move_x = (target - v.paddle).signum() as i8;
    let next_paddle = v.paddle + i32::from(move_x) * 40;
    let fire = v.swing == 0
        && crossing
            .is_some_and(|(ticks, at)| ticks == 1 && (at - next_paddle).abs() <= PADDLE_REACH);
    BTreeMap::from([(
        1,
        Input {
            move_x,
            aim_x,
            fire,
            ..Input::default()
        },
    )])
}

fn until(run: &mut Run, stop: impl Fn(&Run) -> bool, play: bool) {
    for _ in 0..300 {
        if stop(run) {
            return;
        }
        let keys = if play {
            rally_input(run)
        } else {
            BTreeMap::new()
        };
        run.step(&keys);
        assert!(run.valid_snapshot());
        assert_eq!(run.wire_copy().pastimes, run.pastimes);
    }
    panic!("no next beat: {:?}", run.pastimes.round);
}

#[test]
fn bell_rally_requires_timing_and_survives_a_miss() {
    let start = belfry();
    let mut idle = start.clone();
    until(
        &mut idle,
        |r| {
            r.pastimes
                .round
                .as_ref()
                .unwrap()
                .volley
                .as_ref()
                .unwrap()
                .ball
                .is_none()
        },
        false,
    );
    assert_eq!(
        idle.pastimes.round.as_ref().unwrap().progress,
        0,
        "a serve cannot score itself"
    );
    assert_eq!(
        idle.pastimes
            .round
            .as_ref()
            .unwrap()
            .volley
            .as_ref()
            .unwrap()
            .lives,
        2
    );

    let mut won = start.clone();
    let mut perfects = 0;
    for _ in 0..Game::Volley.time() {
        if won.pastimes.round.is_none() {
            break;
        }
        won.step(&rally_input(&won));
        if won
            .pastimes
            .round
            .as_ref()
            .is_some_and(|r| r.volley.as_ref().unwrap().perfect_flash == PERFECT_FLASH_TICKS)
        {
            perfects += 1;
        }
        assert!(won.valid_snapshot());
        assert_eq!(won.wire_copy().pastimes, won.pastimes);
    }
    assert_eq!(
        won.home.countryside.games.volley.wins, 1,
        "normal inputs must win: {:?}",
        won.pastimes.round
    );
    assert_eq!(won.home.countryside.games.volley.plays, 1);
    assert!(won.home.countryside.games.volley.best > 0);
    assert!(won.score > 0);
    assert!(won.pastimes.round.is_none());
    assert!(perfects > 0, "the input bot must exercise a perfect return");
    let paid = won.score;
    won.step(&BTreeMap::new());
    assert_eq!(won.score, paid, "a completed rally pays once");

    let mut recovery = start;
    until(
        &mut recovery,
        |r| r.pastimes.round.as_ref().is_none_or(|r| r.progress > 0),
        true,
    );
    let bells = recovery.pastimes.round.as_ref().unwrap().progress;
    assert!(bells > 0, "a bell must be won by a return before this test");
    until(
        &mut recovery,
        |r| {
            r.pastimes
                .round
                .as_ref()
                .unwrap()
                .volley
                .as_ref()
                .unwrap()
                .ball
                .is_none()
        },
        false,
    );
    let v = recovery
        .pastimes
        .round
        .as_ref()
        .unwrap()
        .volley
        .as_ref()
        .unwrap();
    assert_eq!(
        recovery.pastimes.round.as_ref().unwrap().progress,
        bells,
        "a miss keeps earned bells"
    );
    assert_eq!((v.lives, v.combo), (2, 0));
    assert!(v.best_combo > 0);
    until(
        &mut recovery,
        |r| {
            r.pastimes
                .round
                .as_ref()
                .unwrap()
                .volley
                .as_ref()
                .unwrap()
                .ball
                .is_some()
        },
        false,
    );

    let wire = serde_json::to_vec(&recovery).unwrap();
    let mut restored: Run = serde_json::from_slice(&wire).unwrap();
    assert!(restored.valid_snapshot());
    for _ in 0..30 {
        let a = rally_input(&recovery);
        recovery.step(&a);
        restored.step(&a);
        assert_eq!(recovery.pastimes, restored.pastimes);
        assert!(restored.valid_snapshot());
    }
}

/// Optional real-renderer review, using the same live-input rally.
/// `ANGEL_BELFRY_SHOTS=/tmp/belfry cargo test belfry_rally_shots -- --ignored --nocapture`
#[test]
#[ignore]
fn belfry_rally_shots() {
    let Some(dir) = std::env::var_os("ANGEL_BELFRY_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut run = belfry();
    let mut saved = 0;
    for _ in 0..Game::Volley.time() {
        if run.pastimes.round.is_none() {
            break;
        }
        let before = run
            .pastimes
            .round
            .as_ref()
            .unwrap()
            .volley
            .as_ref()
            .unwrap()
            .perfect_flash;
        run.step(&rally_input(&run));
        let Some(v) = run.pastimes.round.as_ref().and_then(|r| r.volley.as_ref()) else {
            break;
        };
        if saved == 0 || (v.perfect_flash == PERFECT_FLASH_TICKS && before == 0) || v.combo == 3 {
            // Native room pixels make the actual ball readable; the full
            // screenshot still includes the same live HUD and room layers.
            let img = crate::render::overworld::arena::frame_for(
                &run,
                crate::render::overworld::arena::NATIVE_W,
                crate::render::overworld::arena::NATIVE_H,
                Some(1),
            );
            let scale = if saved == 0 { 2 } else { 1 };
            image::RgbaImage::from_raw(
                img.w as u32 * scale,
                img.h as u32 * scale,
                img.rgba_scaled(scale),
            )
            .unwrap()
            .save(dir.join(format!("rally-{saved}.png")))
            .unwrap();
            if let Some(b) = &v.ball {
                let (x, y) = crate::render::overworld::arena::frame_px(
                    &run,
                    img.w,
                    img.h,
                    Some(1),
                    b.x as f32 / 128.0,
                    b.y as f32 / 128.0,
                );
                let origin = crate::render::overworld::arena::view_origin(&run, Some(1));
                let overlay = crate::render::overworld::arena::overlay_for_test(&run, Some(1));
                eprintln!(
                    "rally-{saved}: ball={:?} view=({}, {}) origin={:?} overlay={:?}",
                    b,
                    x,
                    y,
                    origin,
                    overlay.get(x - origin.0, y - origin.1)
                );
                let bright = crate::render::overworld::ink::ink('w').unwrap();
                let visible = (-4..=4).any(|dy| {
                    (-4..=4)
                        .any(|dx| overlay.get(x - origin.0 + dx, y - origin.1 + dy) == Some(bright))
                });
                assert!(visible, "scenery must not cover the live ball");
            }
            saved += 1;
            if saved == 3 {
                break;
            }
        }
    }
    assert_eq!(saved, 3, "three real rally frames");
}
