use super::*;

#[test]
fn every_named_sound_and_track_has_a_file() {
    let dir = sound_dir().expect("the Delve's sound folder");
    for name in SFX {
        let file = dir.join("sfx").join(format!("{name}.mp3"));
        let size = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
        assert!(size > 1_000, "{name}: {size} bytes");
        // The wheel's clicks last as long as its spin, and a few great moments
        // (a guardian's fall, the hand's drag, the gate, the circle's bell)
        // ring a little longer; everything else is short.
        let most = match *name {
            "wheel_spin" => 4.6,
            "boss_death" | "hell_gate_rise" => 3.2,
            "hand_drag" | "circle_toll" => 2.6,
            // The audition's long takes (2026-10-24) and the Bard's lullaby.
            "boss_fall" | "boss_roar" | "crack_open" | "hell_gate_enter" | "wish_grant" => 2.6,
            _ => 1.6,
        };
        assert!(
            (size as f32) / SFX_BYTES_PER_SECOND < most,
            "{name} is short"
        );
    }
    for track in TRACKS {
        let file = dir.join("music").join(format!("{track}.mp3"));
        assert!(
            std::fs::metadata(&file).is_ok_and(|m| m.len() > 200_000),
            "{track}"
        );
    }
}

#[test]
fn every_bard_slice_and_lullaby_has_a_file_the_mixer_can_name() {
    use angel_delve::rules::bard;
    let dir = sound_dir().expect("the Delve's sound folder");
    let all: Vec<&str> = names().collect();
    for name in bard::names() {
        assert!(all.contains(name), "{name} is not in the mixer's names");
        let size = std::fs::metadata(dir.join("sfx").join(format!("{name}.mp3"))).map(|m| m.len()).unwrap_or(0);
        // A slice is a beat and its 16 ms tail (0.416 s, a little over 5 kB at 96 kbit/s); a Lullaby, a bar and a half.
        let seconds = size as f32 / SFX_BYTES_PER_SECOND;
        let (lo, hi) = if bard::is_slice(name) { (0.3, 0.6) } else { (1.5, 3.2) };
        assert!((lo..hi).contains(&seconds), "{name}: {seconds} s");
        assert_eq!(gap(name), Duration::from_millis(if bard::is_slice(name) { 300 } else { 800 }), "{name}");
    }
    for gone in ["sax_a2", "bard_kick", "bard_wub", "bard_overload", "bard_glitch_0", "bard_lullaby"] {
        assert!(!all.contains(&gone), "{gone} is replaced by the songs");
    }
}

#[test]
fn the_bards_song_is_let_through_when_other_effects_fill_the_mixer() {
    use angel_delve::rules::bard;
    let mut audio = Audio::default();
    let now = Instant::now();
    for name in SFX.iter().take(MAX_AT_ONCE + 4) {
        audio.sfx(name, now);
    }
    assert_eq!(audio.played, MAX_AT_ONCE, "the mixer is full");
    // A missed slice is a hole in the music: the song is never the effect that is let go.
    audio.sfx(bard::slice(0, 12), now);
    assert_eq!(audio.played, MAX_AT_ONCE + 1);
    // A tapped fire restarting the opening over itself is let go; the next beat is its own name.
    audio.sfx(bard::slice(0, 12), now + Duration::from_millis(120));
    assert_eq!(audio.played, MAX_AT_ONCE + 1);
    audio.sfx(bard::slice(0, 13), now + Duration::from_millis(400));
    assert_eq!(audio.played, MAX_AT_ONCE + 2);
}

#[test]
fn effects_are_throttled_per_name_and_unknown_names_ignored() {
    let mut audio = Audio::default();
    let now = Instant::now();
    audio.sfx("shot_bow", now);
    audio.sfx("shot_bow", now + Duration::from_millis(20));
    assert_eq!(audio.played, 1, "a second arrow within the gap is let go");
    audio.sfx("shot_bow", now + Duration::from_millis(100));
    assert_eq!(audio.played, 2);
    audio.sfx("kazoo", now);
    assert_eq!(audio.played, 2, "unknown names make no sound");
}

#[test]
fn only_a_few_effects_sound_at_once() {
    let mut audio = Audio::default();
    let now = Instant::now();
    for name in SFX.iter().take(9) {
        audio.sfx(name, now);
    }
    assert_eq!(audio.played, MAX_AT_ONCE);
    // Once the first ones end, more may play.
    audio.sfx("bomb", now + Duration::from_secs(3));
    assert_eq!(audio.played, MAX_AT_ONCE + 1);
}

#[test]
fn muted_audio_is_silent_and_remembers_its_track() {
    let mut audio = Audio::default();
    audio.set_muted(true);
    audio.sfx("hit", Instant::now());
    assert_eq!(audio.played, 0);
    audio.music(Some("crypt"));
    assert_eq!(audio.track(), Some("crypt"));
    assert!(
        audio.player.is_none(),
        "no player while muted (and never in tests)"
    );
    audio.set_muted(false);
    assert_eq!(audio.track(), Some("crypt"));
}

#[test]
fn tracks_switch_stop_and_ignore_unknown_names() {
    let mut audio = Audio::default();
    audio.music(Some("mines"));
    assert_eq!(audio.track(), Some("mines"));
    audio.music(Some("mines"));
    assert_eq!(audio.track(), Some("mines"), "the same track is a no-op");
    audio.music(Some("boss"));
    assert_eq!(audio.track(), Some("boss"));
    audio.music(Some("polka"));
    assert_eq!(
        audio.track(),
        Some("boss"),
        "an unknown track keeps what plays"
    );
    audio.music(None);
    assert_eq!(audio.track(), None);
    audio.music(Some("menu"));
    audio.stop();
    assert_eq!(audio.track(), None);
    assert!(audio.player.is_none());
}

#[test]
fn the_knights_voices_play_and_mute_on_their_own() {
    let mut audio = Audio::default();
    let now = Instant::now();
    let grunt = crate::together_shooter::chatter::clip(1, "oof", 0);
    assert!(sfx_file(grunt).is_some(), "{grunt} has a file");
    audio.sfx(grunt, now);
    assert_eq!(audio.played, 1);
    audio.voices.muted = true;
    audio.sfx(crate::together_shooter::chatter::clip(2, "cheer", 0), now);
    assert_eq!(audio.played, 1, "the knights' voices are off");
    audio.sfx("bomb", now);
    assert_eq!(audio.played, 2, "effects carry on without them");
}
