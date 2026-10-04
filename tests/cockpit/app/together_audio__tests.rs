use super::*;

#[test]
fn every_named_sound_and_track_has_a_file() {
    let dir = sound_dir().expect("the Delve's sound folder");
    for name in SFX {
        let file = dir.join("sfx").join(format!("{name}.mp3"));
        let size = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
        assert!(size > 1_000, "{name}: {size} bytes");
        assert!(
            (size as f32) / SFX_BYTES_PER_SECOND < 1.6,
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
