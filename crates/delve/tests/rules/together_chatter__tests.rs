use super::super::*;
use super::*;

/// A calm, empty room with the host's knight in it, the chatter awake.
fn quiet() -> Run {
    let mut run = Run::new(4, 1, None);
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    run.enter_for_test(room);
    run.calm_for_test();
    run.enemies.clear();
    // The way in has its own moments; start from a quiet party.
    run.cues.clear();
    run.chatter(0);
    run.sounds.clear();
    run
}

/// A hit as a monster lands it, on a knight free to take it.
fn strike(run: &mut Run, id: u32, damage: u32) {
    run.players.get_mut(&id).unwrap().invulnerable = 0;
    run.hurt_for_test(id, damage);
}

/// The knights' noises since the last look.
fn heard(run: &mut Run) -> Vec<&'static str> {
    let said = run
        .sounds
        .iter()
        .copied()
        .filter(|s| s.starts_with("knights/"))
        .collect();
    run.sounds.clear();
    said
}

/// A moment happens `ticks` after the last one; the knights react.
fn moment(run: &mut Run, ticks: u64, cue: &str) -> Vec<&'static str> {
    run.tick += ticks;
    let from = run.cues.len();
    run.cues.push(cue.into());
    run.chatter(from);
    heard(run)
}

#[test]
fn a_knight_keeps_one_voice() {
    assert!(clip(1, "oof", 0).starts_with("knights/a_oof_"));
    assert!(clip(2, "cheer", 1).starts_with("knights/b_cheer_"));
    assert!(clip(4, "waah", 0).starts_with("knights/d_"));
    assert!(
        clip(5, "hup", 0).starts_with("knights/a_"),
        "seats wrap round"
    );
}

#[test]
fn big_moments_need_a_breath_and_small_ones_a_long_quiet() {
    let mut run = quiet();
    let first = moment(&mut run, 0, "card_relic");
    assert_eq!(first.len(), 1);
    assert!(first[0].contains("_ooh_"));
    assert!(
        moment(&mut run, 1, "card_rare").is_empty(),
        "small moments wait"
    );
    let big = moment(&mut run, BREATH, "boss_fall:cinderjaw");
    assert!(big.len() == 1 && big[0].contains("_cheer_"), "{big:?}");
    assert!(
        moment(&mut run, 1, "victory").is_empty(),
        "a breath between lines"
    );
    assert_eq!(moment(&mut run, QUIET, "card_rare").len(), 1);
    assert!(
        moment(&mut run, BREATH, "room_clear").is_empty(),
        "not every moment"
    );
}

#[test]
fn the_biggest_moment_of_a_tick_is_the_one_remarked_on() {
    let mut run = quiet();
    run.cues.push("card_rare".into());
    run.cues.push("boss_rise:cinderjaw".into());
    run.chatter(0);
    let said = heard(&mut run);
    assert!(said.len() == 1 && said[0].contains("_uhoh_"), "{said:?}");
}

#[test]
fn drained_cues_are_read_afresh() {
    let mut run = quiet();
    moment(&mut run, 0, "run_start");
    run.cues.clear();
    let said = moment(&mut run, BREATH, "revive");
    assert!(said.len() == 1 && said[0].contains("_thanks_"), "{said:?}");
}

#[test]
fn a_party_takes_turns() {
    let mut run = quiet();
    run.join_seat(2, "Friend");
    run.chatter(run.cues.len());
    heard(&mut run);
    let one = moment(&mut run, BREATH, "boss_rise");
    let two = moment(&mut run, BREATH, "boss_fall");
    assert!(one[0].starts_with("knights/a_"), "{one:?}");
    assert!(two[0].starts_with("knights/b_"), "{two:?}");
}

#[test]
fn hits_grunt_now_and_then_and_a_fall_always_sounds() {
    let mut run = quiet();
    strike(&mut run, 1, 4);
    let oof = heard(&mut run);
    assert!(oof.len() == 1 && oof[0].contains("_oof_"), "{oof:?}");
    run.tick += 1;
    strike(&mut run, 1, 4);
    assert!(heard(&mut run).is_empty(), "not every hit");
    run.tick += 6 * HZ as u64;
    strike(&mut run, 1, 4);
    assert_eq!(heard(&mut run).len(), 1);
    run.tick += 1;
    let hp = run.players[&1].hp;
    strike(&mut run, 1, hp * 10);
    let fall = heard(&mut run);
    assert!(fall.iter().any(|s| s.contains("_waah_")), "{fall:?}");
}

/// How often the knights make a noise in real fights: three knights fire at
/// the nearest foe, strafe, and roll now and then, room after room.
/// `cargo test --bin angel knights_noise_rate -- --ignored --nocapture`
#[test]
#[ignore]
fn knights_noise_rate() {
    let mut run = Run::new(9, 1, None);
    run.join_seat(2, "Friend");
    run.join_seat(3, "Other");
    let rooms: Vec<usize> = (0..run.dungeon.rooms.len())
        .filter(|&r| run.dungeon.rooms[r].kind == RoomKind::Fight)
        .collect();
    let mut counts: BTreeMap<&str, u32> = BTreeMap::new();
    let mut ticks = 0u64;
    for room in rooms.into_iter().take(6) {
        run.enter_for_test(room);
        for t in 0..90 * HZ {
            let inputs: BTreeMap<u32, Input> = run
                .players
                .iter()
                .map(|(&id, h)| {
                    let foe = run.enemies.iter().filter(|e| e.hp > 0).min_by(|a, b| {
                        (a.x - h.x)
                            .hypot(a.y - h.y)
                            .total_cmp(&(b.x - h.x).hypot(b.y - h.y))
                    });
                    let (ax, ay) = foe.map_or((1.0, 0.0), |e| (e.x - h.x, e.y - h.y));
                    let sign = |v: f32| if v.abs() < 0.5 { 0 } else { v.signum() as i8 };
                    let input = Input {
                        aim_x: sign(ax),
                        aim_y: sign(ay),
                        move_x: if (t / 40 + id) % 2 == 0 { 1 } else { -1 },
                        move_y: sign(-ay),
                        fire: true,
                        dash: (t + id * 17) % 70 == 0,
                        ..Default::default()
                    };
                    (id, input)
                })
                .collect();
            run.step(&inputs);
            ticks += 1;
            for sound in std::mem::take(&mut run.sounds) {
                if let Some(rest) = sound.strip_prefix("knights/") {
                    let clip = rest.split('_').nth(1).unwrap_or(rest);
                    *counts.entry(clip).or_default() += 1;
                }
            }
            run.cues.clear();
            if run.phase != Phase::Fighting {
                break;
            }
        }
        for hero in run.players.values_mut() {
            hero.hp = hero.max_hp;
        }
    }
    let minutes = ticks as f32 / HZ as f32 / 60.0;
    println!("{minutes:.1} minutes of fighting, three knights");
    for (clip, n) in &counts {
        println!("  {clip:<7} {n:>4}  {:.1} a minute", *n as f32 / minutes);
    }
}
