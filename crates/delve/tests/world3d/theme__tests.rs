use super::*;

#[test]
fn every_theme_has_a_name_and_a_number_of_its_own() {
    for (i, theme) in Theme::ALL.iter().enumerate() {
        assert_eq!(theme.slot(), i as u32);
        assert_eq!(Theme::from_name(theme.name()), Some(*theme));
    }
}

#[test]
fn each_delve_and_the_kingdoms_places_have_patches_of_their_painted_items() {
    let themes = Themes::scan(&super::super::assets::plate_pack());
    for theme in Theme::ALL {
        let Some(patches) = themes.get(theme) else {
            panic!("{}: no patches", theme.name());
        };
        assert!(!patches.swatches.is_empty(), "{}", theme.name());
        // The kinds every room stands on: its floor and its walls.
        for (kind, from) in &patches.from {
            let item = pilot::item(from).expect("an item");
            let own = Theme::of(item) == theme || theme.borrows().contains(&Theme::of(item));
            assert!(own, "{}: {kind} of {from}, another theme's", theme.name());
        }
        // Each laid at the theme's density, a cell at most; and a page sent
        // the PNG reads the same patches back.
        assert!(patches.swatches.iter().all(|s| s.width <= SWATCH_CELL && s.height <= SWATCH_CELL));
        let back = Patches::of_png(theme, Arc::clone(&patches.png)).expect("the PNG reads");
        assert_eq!(back.key, patches.key);
        let mut a: Vec<_> = back.swatches.iter().map(|s| (s.kind as u32, s.width, s.height, s.rgba.clone())).collect();
        let mut b: Vec<_> = patches.swatches.iter().map(|s| (s.kind as u32, s.width, s.height, s.rgba.clone())).collect();
        a.sort();
        b.sort();
        assert!(a == b, "{}: the PNG holds the patches", theme.name());
        println!("{}: {}", theme.name(), patches.from.iter().map(|(k, f)| format!("{k}<{f}")).collect::<Vec<_>>().join(" "));
    }
    // A page has no folder: only what it is sent.
    let page = Themes::scan(std::path::Path::new("/nowhere/at/all"));
    assert!(page.get(Theme::Delve(Pack::Crypt)).is_none());
    let crypt = themes.get(Theme::Delve(Pack::Crypt)).unwrap();
    assert!(page.receive("crypt", crypt.key, &crypt.png));
    assert_eq!(page.get(Theme::Delve(Pack::Crypt)).map(|p| p.key), Some(crypt.key));
    assert!(!page.receive("nowhere", 1, &crypt.png));
}
