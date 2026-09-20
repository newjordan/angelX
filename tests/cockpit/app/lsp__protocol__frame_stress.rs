use super::read_frame;

#[test]
fn oversized_content_length_is_rejected_not_allocated() {
    // A buggy/hostile server declaring a gigantic Content-Length must be
    // rejected before `vec![0u8; len]` aborts the whole process.
    let framed = format!("Content-Length: {}\r\n\r\n", 9_000_000_000u64);
    let mut cursor = std::io::Cursor::new(framed.into_bytes());
    assert!(read_frame(&mut cursor).is_err());
}

#[test]
fn well_formed_small_frame_still_reads() {
    let body = "{\"jsonrpc\":\"2.0\"}";
    let framed = format!("Content-Length: {}\r\n\r\n{body}", body.len());
    let mut cursor = std::io::Cursor::new(framed.into_bytes());
    let v = read_frame(&mut cursor).unwrap().unwrap();
    assert_eq!(v["jsonrpc"], "2.0");
}
