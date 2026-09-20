use super::*;

#[test]
fn document_is_actual_bounded_content_and_escapes_terminal_commands() {
    let root = std::env::temp_dir().join(format!("angel-stage-document-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("report.md");
    std::fs::write(&path, "# Real result\nmeasured: 17\n\x1b[2J").unwrap();
    let document = load_document(&MediaSource::operator(path.clone())).unwrap();
    assert!(document.text.contains("measured: 17"));
    assert!(!document.text.contains('\x1b'));
    assert!(document.receipt.contains("SHA256"));
    std::fs::write(&path, b"%PDF-1.7\n").unwrap();
    assert!(
        load_document(&MediaSource::operator(path.clone()))
            .err()
            .unwrap()
            .contains("Unsupported document format")
    );
    std::fs::write(&path, "é".repeat(MAX_DOCUMENT_BYTES)).unwrap();
    let document = load_document(&MediaSource::operator(path.clone())).unwrap();
    assert!(document.receipt.starts_with("TRUNCATED"));
    assert!(document.text.ends_with("later content.]"));
    assert!(
        load_document(&MediaSource::operator(root.join("missing.md")))
            .err()
            .unwrap()
            .contains("Cannot read requested file")
    );
    assert!(
        load_document(&MediaSource::operator(root.clone()))
            .err()
            .unwrap()
            .contains("regular file")
    );
    std::fs::remove_dir_all(root).unwrap();
}
