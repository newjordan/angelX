use super::*;
use crate::rules::{HZ, Run};

fn ticks(secs: u16) -> u64 {
    u64::from(secs) * u64::from(HZ)
}

#[test]
fn every_track_is_a_file_of_the_game() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../cockpit/assets/dungeon/music");
    for track in [HOME, REST, DUNGEON, BOSS].into_iter().flatten() {
        let file = dir.join(format!("{}.mp3", track.name));
        let len = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
        assert!(len > 10_000, "{} is in the bundle's music", track.name);
        assert!(track.secs >= 20, "{} plays long enough to be a track", track.name);
        assert_eq!(known(track.name), Some(track.name));
    }
    assert_eq!(known("no-such-track"), None);
    // The whole score is in: over two hours of it.
    let all: u32 = [HOME, DUNGEON, BOSS].into_iter().flatten().map(|t| u32::from(t.secs)).sum();
    assert!(all > 2 * 3600, "{all} seconds");
    assert!(DUNGEON.len() >= 30 && BOSS.len() >= 6 && HOME.len() >= 60);
}

#[test]
fn a_list_moves_on_to_its_next_track_as_one_ends() {
    let first = play(DUNGEON, 0, 0);
    let at = DUNGEON.iter().position(|t| t.name == first).unwrap();
    let len = ticks(DUNGEON[at].secs);
    // The track still plays until its length is up, then the next does.
    let start = (0..len * 2).find(|&t| play(DUNGEON, 0, t) != first).unwrap();
    assert_eq!(play(DUNGEON, 0, start), DUNGEON[(at + 1) % DUNGEON.len()].name);
    let next_len = ticks(DUNGEON[(at + 1) % DUNGEON.len()].secs);
    assert_eq!(play(DUNGEON, 0, start + next_len - 1), DUNGEON[(at + 1) % DUNGEON.len()].name);
    assert_eq!(play(DUNGEON, 0, start + next_len), DUNGEON[(at + 2) % DUNGEON.len()].name);
}

#[test]
fn a_floor_plays_many_tracks_and_keeps_them_from_room_to_room() {
    let mut run = Run::new(7, 1, None);
    run.enemies.retain(|e| e.boss.is_none());
    let mut heard = std::collections::BTreeSet::new();
    let start = run.tick;
    for half in 0..120 {
        run.tick = start + ticks(30) * half;
        heard.extend(run.music());
    }
    assert!(heard.len() >= 20, "an hour on a floor plays {} tracks", heard.len());
    // Through a door: the floor's list plays on, the same track.
    run.tick = start + ticks(600);
    let here = run.music();
    let other = (0..run.dungeon.rooms.len()).find(|&i| i != run.at && run.dungeon.rooms[i].kind != crate::rules::RoomKind::Sanctuary).unwrap();
    run.at = other;
    assert_eq!(run.music(), here);
}

#[test]
fn a_floor_rests_between_its_tracks_and_never_plays_one_twice_running() {
    use crate::rules::music::{QUIET, play_sparse};
    let hz = u64::from(crate::rules::HZ);
    // Every second of two hours on one floor: what plays, and the quiets.
    let heard: Vec<Option<&str>> = (0..2 * 3600).map(|s| play_sparse(DUNGEON, 11, s * hz)).collect();
    let mut runs: Vec<(Option<&str>, u64)> = Vec::new();
    for &m in &heard {
        match runs.last_mut() {
            Some((last, n)) if *last == m => *n += 1,
            _ => runs.push((m, 1)),
        }
    }
    let tracks: Vec<&str> = runs.iter().filter_map(|&(m, _)| m).collect();
    assert!(tracks.len() >= 40, "{} tracks in two hours", tracks.len());
    // A quiet after every track, as long as `QUIET` says (the first and
    // last run of the window may be cut short).
    for pair in runs.windows(2) {
        assert!(pair[0].0.is_some() != pair[1].0.is_some(), "a track and a quiet by turns: {pair:?}");
    }
    for &(m, n) in &runs[1..runs.len() - 1] {
        if m.is_none() {
            assert!((QUIET.0..=QUIET.1).contains(&n), "a quiet of {n} s");
        }
    }
    for pair in tracks.windows(2) {
        assert_ne!(pair[0], pair[1], "never the same track twice running");
    }
    // Two delves play their floors' lists in orders of their own.
    let order = |seed: u64| -> Vec<&str> {
        let mut seen = Vec::new();
        for s in 0..3 * 3600 {
            if let Some(m) = play_sparse(DUNGEON, seed, s * hz)
                && seen.last() != Some(&m)
            {
                seen.push(m);
            }
        }
        seen.truncate(8);
        seen
    };
    assert_ne!(order(11), order(12));
}
