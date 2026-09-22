use super::*;

#[test]
fn export_records_every_cell_with_rgb_styles() {
    let value = export("win", 2_600, 18, 6).unwrap();
    let rows = value["cells"].as_array().unwrap();
    assert_eq!(rows.len(), 6);
    assert!(rows.iter().all(|row| row.as_array().unwrap().len() == 18));
    let cell = &rows[0][0];
    assert!(cell["glyph"].is_string());
    assert_eq!(cell["style"]["fg"].as_array().unwrap().len(), 3);
    assert_eq!(cell["style"]["bg"].as_array().unwrap().len(), 3);
}

#[test]
fn aliases_and_invalid_scenes_are_deterministic() {
    assert_eq!(export("loop-done", 0, 1, 1).unwrap()["scene"], "loop-done");
    assert!(export("unknown", 0, 1, 1).is_err());
    assert_eq!(
        export("moa-tag-team", 0, 1, 1).unwrap()["scene"],
        "moa-tag-team"
    );
    assert_eq!(
        export("moa-auto-moa", 0, 1, 1).unwrap()["scene"],
        "moa-auto-moa"
    );
}
