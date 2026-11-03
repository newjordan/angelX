//! The first session, run: `first_session_sim` prints a beginner's first
//! ten minutes for a set of seeds; the plain tests hold the floor of what
//! the first session must give.
//!
//! `cargo test -p angel-delve --profile fast first_session_sim -- --ignored --nocapture`
//! (env: FS_SEEDS=1,2,3 the seeds, FS_SECS=600 each session's length,
//! FS_OUT=<dir> a JSON of every number and line per seed, FS_FILM=<dir>
//! the first seed's screen as PNGs, FS_FILM_EVERY=2 ticks apart).

use super::*;

fn secs(s: &Session, what: &str) -> String {
    s.secs(what).map_or("-".into(), |t| format!("{t:.0}"))
}

/// Everything a session logged, as JSON.
pub fn report(s: &Session) -> serde_json::Value {
    let hz = HZ as f32;
    serde_json::json!({
        "seed": s.seed,
        "secs": s.ticks as f32 / hz,
        "firsts": s.firsts.iter().map(|(k, &t)| (k.to_string(), serde_json::json!(t as f32 / hz))).collect::<serde_json::Map<_, _>>(),
        "learned": s.learned.iter().map(|(k, (t, how))| serde_json::json!({"key": k.name(), "at": *t as f32 / hz, "how": how})).collect::<Vec<_>>(),
        "places": s.places.iter().map(|(t, p)| serde_json::json!({"at": *t as f32 / hz, "place": p})).collect::<Vec<_>>(),
        "chambers": s.chambers,
        "falls": s.falls.iter().map(|(t, at)| serde_json::json!({"at": *t as f32 / hz, "where": at})).collect::<Vec<_>>(),
        "falls_in_first_3": s.falls_in_first(3),
        "wipes": s.wipes,
        "kills": s.kills,
        "wishes": s.wishes.iter().map(|(t, w)| serde_json::json!({"at": *t as f32 / hz, "wish": w})).collect::<Vec<_>>(),
        "idle": s.idle.iter().map(|(k, &t)| (format!("{k:?}"), serde_json::json!(t as f32 / hz))).collect::<serde_json::Map<_, _>>(),
        "idle_secs": s.idle_secs(),
        "stuck_secs": s.stuck as f32 / hz,
        "fight_secs": s.boards_over_fights().1,
        "boards_over_fights_secs": s.boards_over_fights().0,
        "scroll_over_fights_secs": s.scroll_over_fight as f32 / hz,
        "shown": s.shown.iter().map(|(line, v)| serde_json::json!({
            "line": line, "layer": format!("{:?}", v.layer.unwrap_or(Layer::View)),
            "first": v.first as f32 / hz, "last": v.last as f32 / hz, "up": v.up as f32 / hz,
            "read": v.read.map(|t| t as f32 / hz), "over_fight": v.over_fight as f32 / hz,
        })).collect::<Vec<_>>(),
    })
}

fn row(s: &Session) -> String {
    format!(
        "seed {:>3} | move {:>3} fire {:>3} armed {:>3} hit {:>3} kill {:>3} wish {:>3} floor1 {:>3} exit {:>4} | falls {} (rooms 1-3: {}) wipes {} kills {:>2} | idle {:>4.0}s stuck {:>4.0}s boards over fights {:>3.0}s of {:>3.0}s",
        s.seed,
        secs(s, "moved"),
        secs(s, "fired"),
        secs(s, "armed"),
        secs(s, "first hit"),
        secs(s, "first kill"),
        secs(s, "first wish"),
        secs(s, "floor 1"),
        secs(s, "floor 1 left"),
        s.falls.len(),
        s.falls_in_first(3),
        s.wipes,
        s.kills,
        s.idle_secs(),
        s.stuck as f32 / HZ as f32,
        s.boards_over_fights().0,
        s.boards_over_fights().1,
    )
}

fn seeds() -> Vec<u64> {
    std::env::var("FS_SEEDS")
        .ok()
        .map(|s| s.split(',').filter_map(|n| n.trim().parse().ok()).collect())
        .unwrap_or_else(|| vec![1, 2, 3, 4, 5, 6])
}

#[test]
#[ignore]
fn first_session_sim() {
    let secs: u32 = std::env::var("FS_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(600);
    let out = std::env::var_os("FS_OUT").map(std::path::PathBuf::from);
    let film_dir = std::env::var_os("FS_FILM").map(std::path::PathBuf::from);
    let every: u64 = std::env::var("FS_FILM_EVERY").ok().and_then(|s| s.parse().ok()).unwrap_or(2);
    let seeds = seeds();
    // One after another: the renderer's caches are shared, and threads
    // drawing at once wait on each other far longer than they save.
    let sessions: Vec<Session> = seeds
        .iter()
        .enumerate()
        .map(|(i, &seed)| {
            let film = (i == 0).then(|| film_dir.clone()).flatten().map(|dir| {
                std::fs::create_dir_all(&dir).unwrap();
                Film::new(dir, every)
            });
            play_tuned(seed, Beginner::default(), secs, film.as_ref(), true)
        })
        .collect();
    println!("beginner, live start, {secs} s each (seconds from the link):");
    for s in &sessions {
        println!("{}", row(s));
    }
    let n = sessions.len() as f32;
    let mean = |what: &str| {
        let got: Vec<f32> = sessions.iter().filter_map(|s| s.secs(what)).collect();
        if got.is_empty() { "-".to_string() } else { format!("{:.0} ({}/{})", got.iter().sum::<f32>() / got.len() as f32, got.len(), sessions.len()) }
    };
    println!(
        "mean | move {} fire {} armed {} hit {} kill {} wish {} floor1 {} exit {} | falls {:.1} (rooms 1-3: {:.1}) wipes {:.1} | idle {:.0}s boards over fights {:.0}s of {:.0}s",
        mean("moved"),
        mean("fired"),
        mean("armed"),
        mean("first hit"),
        mean("first kill"),
        mean("first wish"),
        mean("floor 1"),
        mean("floor 1 left"),
        sessions.iter().map(|s| s.falls.len()).sum::<usize>() as f32 / n,
        sessions.iter().map(|s| s.falls_in_first(3)).sum::<usize>() as f32 / n,
        sessions.iter().map(|s| s.wipes).sum::<u32>() as f32 / n,
        sessions.iter().map(|s| s.idle_secs()).sum::<f32>() / n,
        sessions.iter().map(|s| s.boards_over_fights().0).sum::<f32>() / n,
        sessions.iter().map(|s| s.boards_over_fights().1).sum::<f32>() / n,
    );
    let first = &sessions[0];
    println!("\nseed {}: keys learned", first.seed);
    for (k, (t, how)) in &first.learned {
        println!("  {:>5.1}s {:<18} {how}", *t as f32 / HZ as f32, k.name());
    }
    println!("seed {}: places", first.seed);
    for (t, p) in &first.places {
        println!("  {:>5.1}s {p}", *t as f32 / HZ as f32);
    }
    println!("seed {}: falls {:?} wishes {:?} idle {:?}", first.seed, first.falls, first.wishes, first.idle);
    println!("seed {}: lines on the screen, by when first up", first.seed);
    let mut shown: Vec<(&String, &Shown)> = first.shown.iter().collect();
    shown.sort_by_key(|(_, v)| v.first);
    for (line, v) in shown {
        println!(
            "  {:>5.1}s {:<6} up {:>5.1}s read {:>6} over-fight {:>4.1}s  {line}",
            v.first as f32 / HZ as f32,
            format!("{:?}", v.layer.unwrap_or(Layer::View)),
            v.up as f32 / HZ as f32,
            v.read.map_or("-".into(), |t| format!("{:.1}s", t as f32 / HZ as f32)),
            v.over_fight as f32 / HZ as f32,
        );
    }
    if let Some(dir) = out {
        std::fs::create_dir_all(&dir).unwrap();
        for s in &sessions {
            std::fs::write(dir.join(format!("seed-{}.json", s.seed)), serde_json::to_string_pretty(&report(s)).unwrap()).unwrap();
        }
    }
}

#[test]
fn a_line_teaches_the_keys_it_names() {
    assert_eq!(teaches("T TO MAKE THEM"), vec![Key::Wish]);
    assert_eq!(teaches("E: BE THIS KNIGHT"), vec![Key::Use]);
    assert_eq!(teaches("WISH 3 T"), vec![Key::Wish]);
    assert_eq!(teaches("ARROWS AIM  F SHOOT"), vec![Key::Fire]);
    assert!(teaches("T").is_empty(), "a key alone names nothing");
    assert!(teaches("DON'T PANIC").is_empty());
    assert_eq!(keycap("F"), Some(Key::Fire), "the tray's cap under the weapon");
    assert_eq!(keycap("R"), None);
}

/// Seconds from the session's first arrival on floor 1 to when `line` was
/// first up and first read.
fn after_arrival(s: &Session, line: &str) -> (Option<f32>, Option<f32>) {
    let hz = HZ as f32;
    let Some(arrival) = s.places.iter().find(|(_, p)| p.contains("floor 1")).map(|(t, _)| *t as f32 / hz) else {
        return (None, None);
    };
    let shown = s.shown.get(line);
    (shown.map(|v| v.first as f32 / hz - arrival), shown.and_then(|v| v.read).map(|t| t as f32 / hz - arrival))
}

/// A beginner's first floor, as the first session measured it on
/// 2026-10-29 and as it was fixed: the floor's wish sign is up long enough
/// to read as the party arrives (it waited under the title card and the
/// dare), Fortune's dare comes after it, and a beginner who has not found
/// the wish key is told it in the first calm moment.
#[test]
fn a_beginner_reads_the_first_floors_wish_sign_and_is_told_the_wish_key() {
    let s = play(1, Beginner::default(), 45, None);
    let (up, read) = after_arrival(&s, "T TO MAKE THEM");
    assert!(up.is_some_and(|t| t < 3.0), "the wish sign came up {up:?} s into the floor");
    assert!(read.is_some_and(|t| t < 6.0), "the wish sign was read {read:?} s into the floor");
    let (dare, _) = after_arrival(&s, "FORTUNE DARES YOU");
    assert!(dare.is_some_and(|t| t >= 4.5), "the dare came down {dare:?} s into the floor, over the wish sign");
    let (hint, hint_read) = after_arrival(&s, "T MAKE A WISH");
    assert!(hint.is_some_and(|t| t < 10.0) && hint_read.is_some(), "the wish key's hint: up {hint:?}, read {hint_read:?}");
    assert!(s.knows_by(Key::Wish, 30.0), "the wish key was learned at {:?}", s.learned.get(&Key::Wish));
}
