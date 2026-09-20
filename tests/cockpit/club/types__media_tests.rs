use super::*;

fn temp_path(label: &str) -> std::path::PathBuf {
    static ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "angel-media-{label}-{}-{}",
        std::process::id(),
        ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ))
}

#[test]
fn image_admission_uses_magic_not_extension_and_rejects_malformed_bytes() {
    let disguised = temp_path("pixel.jpg");
    let mut png = Vec::new();
    image::DynamicImage::new_rgba8(1, 1)
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    std::fs::write(&disguised, &png).unwrap();
    let media = Media::image_from_path(&disguised).unwrap();
    assert!(matches!(media, Media::Image { ref mime, .. } if mime == "image/png"));
    std::fs::write(&disguised, b"not really an image").unwrap();
    assert!(Media::image_from_path(&disguised).is_err());
    std::fs::remove_file(disguised).unwrap();
}

#[test]
fn attachment_reader_rejects_oversized_and_empty_inputs_before_encoding() {
    let path = temp_path("bounded.bin");
    std::fs::write(&path, []).unwrap();
    assert!(read_attachment_bounded(&path, 4, "test").is_err());
    std::fs::write(&path, b"12345").unwrap();
    let error = read_attachment_bounded(&path, 4, "test").unwrap_err();
    assert!(error.contains("safety limit"), "{error}");
    std::fs::remove_file(path).unwrap();
}
