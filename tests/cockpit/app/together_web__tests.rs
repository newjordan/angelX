//! The browser page's music comes from disk, not from the binary.

use super::*;
use angel_delve::rules::music::{BOSS, DUNGEON, HOME, REST};

#[test]
fn the_music_is_not_in_the_binary_and_every_track_reaches_the_page() {
    let Some((_, embedded)) = web_bundle::embedded() else {
        return;
    };
    assert!(
        embedded.iter().all(|(name, _)| !name.starts_with("music-")),
        "no music is compiled in"
    );
    assert_eq!(web_bundle::beside(), &[("music-", "cockpit/assets/dungeon/music")]);
    let bundle = bundle().expect("the page");
    let json = bundle
        .index_html
        .split("id=\"sounds\">")
        .nth(1)
        .and_then(|rest| rest.split("</script>").next())
        .expect("the page carries its sounds");
    let manifest: serde_json::Value = serde_json::from_str(json).expect("the sounds are JSON");
    // This checkout has the music beside the binary, so every track the
    // game names is in the page and served from the files read off disk.
    let tracks: Vec<&str> = [HOME, REST, DUNGEON, BOSS]
        .into_iter()
        .flatten()
        .map(|t| t.name)
        .chain(crate::together_audio::TRACKS.iter().copied())
        .collect();
    for track in tracks {
        let name = angel_delve::wire::music_file(track);
        assert_eq!(
            manifest[&name].as_str(),
            Some(format!("{{{{asset:{name}.mp3}}}}").as_str()),
            "{track}"
        );
        let file = bundle
            .files
            .iter()
            .find(|(f, _, _)| *f == format!("{name}.mp3"))
            .unwrap_or_else(|| panic!("{track} is served"));
        assert!(file.1.len() > 10_000, "{track} is a whole track");
    }
    // The effects stay compiled in.
    assert!(manifest["hit"].is_string());
}
