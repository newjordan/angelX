//! The browser page's music comes from disk, not from the binary.

use super::*;

/// Places that find nothing and say nothing.
fn deaf() -> Places {
    Places {
        resource: |_| None,
        missing: |_| {},
    }
}

#[test]
fn a_folder_beside_the_binary_joins_the_page_by_name() {
    let dir = std::env::temp_dir().join(format!("angel-web-beside-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("inner")).unwrap();
    std::fs::write(dir.join("crypt.mp3"), b"ID3crypt").unwrap();
    std::fs::write(dir.join("a-boss.mp3"), b"ID3boss").unwrap();
    // A name the page could not carry is left out, and so is a folder.
    std::fs::write(dir.join("two words.mp3"), b"ID3").unwrap();
    let mut files = Vec::new();
    let sounds = add_folder(&dir, "music-", &mut files, &deaf());
    assert_eq!(
        sounds,
        [
            "\"music-a-boss\":\"{{asset:music-a-boss.mp3}}\"",
            "\"music-crypt\":\"{{asset:music-crypt.mp3}}\""
        ]
    );
    assert_eq!(files.len(), 2);
    assert_eq!(files[1].0, "music-crypt.mp3");
    assert_eq!(files[1].1, b"ID3crypt");
    assert_eq!(files[1].2, "audio/mpeg");
    // A folder that is not there adds nothing.
    assert!(add_folder(&dir.join("gone"), "music-", &mut files, &deaf()).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_page_takes_its_beside_folders_from_the_places_it_is_lent() {
    use std::sync::Mutex;
    static SAID: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let Some((_, embedded)) = BUNDLE else {
        return;
    };
    assert!(
        embedded.iter().all(|(name, _)| !name.starts_with("music-")),
        "no music is compiled in"
    );
    assert_eq!(BESIDE, &[("music-", "cockpit/assets/dungeon/music")]);
    // Nowhere to look: the page opens without music and a line says so.
    SAID.lock().unwrap().clear();
    let bare = bundle(&Places {
        resource: |_| None,
        missing: |line| SAID.lock().unwrap().push(line.to_string()),
    })
    .expect("the page");
    assert!(
        bare.files
            .iter()
            .all(|(name, _, _)| !name.starts_with("music-"))
    );
    assert_eq!(SAID.lock().unwrap().len(), 1);
    assert!(SAID.lock().unwrap()[0].contains("cockpit/assets/dungeon/music"));
    // This checkout has the music beside the host: every track in the folder
    // joins the page's sounds and is served as the file read off disk.
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = repo.join("cockpit/assets/dungeon/music");
    assert!(dir.is_dir(), "the checkout carries its music");
    let page = bundle(&Places {
        resource: |place| {
            let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(place);
            dir.is_dir().then_some(dir)
        },
        missing: |line| panic!("{line}"),
    })
    .expect("the page");
    let json = page
        .index_html
        .split("id=\"sounds\">")
        .nth(1)
        .and_then(|rest| rest.split("</script>").next())
        .expect("the page carries its sounds");
    let manifest: serde_json::Value = serde_json::from_str(json).expect("the sounds are JSON");
    let mut served = 0;
    for entry in std::fs::read_dir(&dir).unwrap().flatten() {
        let path = entry.path();
        let file = format!("music-{}", path.file_name().unwrap().to_string_lossy());
        if !path.is_file()
            || !file
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            continue;
        }
        let stem = file.rsplit_once('.').map_or(file.as_str(), |(s, _)| s);
        assert_eq!(
            manifest[stem].as_str(),
            Some(format!("{{{{asset:{file}}}}}").as_str()),
            "{file}"
        );
        let served_file = page
            .files
            .iter()
            .find(|(f, _, _)| *f == file)
            .unwrap_or_else(|| panic!("{file} is served"));
        assert_eq!(
            served_file.1,
            std::fs::read(&path).unwrap(),
            "{file} is the file on disk"
        );
        served += 1;
    }
    assert!(served > 0, "the music folder has tracks");
    // The effects stay compiled in.
    assert!(manifest["hit"].is_string());
}
