use super::*;

#[test]
fn confined_path_rejects_escape() {
    let ws = PathBuf::from("/tmp/ws");
    assert!(confined_path(&ws, "a/b.mp4").is_ok());
    assert!(confined_path(&ws, "../evil.mp4").is_err());
    assert!(confined_path(&ws, "/etc/passwd").is_err());
    assert!(confined_path(&ws, "a/../../evil.mp4").is_err());
    assert!(confined_path(&ws, "").is_err());
}

#[test]
fn timeline_rejects_bad_geometry() {
    let clips = vec![("a.mp4".to_string(), 5.0, 2.0)];
    assert!(build_timeline_filter(&clips, 0.0, 640, 360, 24, None).is_err());
    let clips = vec![];
    assert!(build_timeline_filter(&clips, 0.0, 640, 360, 24, None).is_err());
}

#[test]
fn hard_cut_concat_chain() {
    let clips = vec![
        ("a.mp4".to_string(), 0.0, 2.0),
        ("b.mp4".to_string(), 1.0, 3.0),
    ];
    let g = build_timeline_filter(&clips, 0.0, 640, 360, 24, None).unwrap();
    assert!(g.contains("concat=n=2:v=1:a=0[vout]"));
    assert!(g.contains("trim=0:2"));
    assert!(g.contains("trim=1:3"));
    assert!(g.ends_with("format=yuv420p[vfinal]"));
}

#[test]
fn xfade_offsets_accumulate() {
    // Three 4s clips, 0.5s fades: first join at 3.5, second at 7.0.
    let clips = vec![
        ("a.mp4".to_string(), 0.0, 4.0),
        ("b.mp4".to_string(), 0.0, 4.0),
        ("c.mp4".to_string(), 0.0, 4.0),
    ];
    let g = build_timeline_filter(&clips, 0.5, 640, 360, 24, None).unwrap();
    assert!(g.contains("offset=3.5000"), "{g}");
    assert!(g.contains("offset=7.0000"), "{g}");
    assert!(g.contains("[vout]"));
}

#[test]
fn fade_longer_than_segment_errors() {
    let clips = vec![
        ("a.mp4".to_string(), 0.0, 0.4),
        ("b.mp4".to_string(), 0.0, 4.0),
    ];
    assert!(build_timeline_filter(&clips, 0.5, 640, 360, 24, None).is_err());
}

#[test]
fn music_chain_tracks_timeline_length() {
    let clips = vec![
        ("a.mp4".to_string(), 0.0, 4.0),
        ("b.mp4".to_string(), 0.0, 4.0),
    ];
    // 8s total, 0.5 fade → 7.5s timeline; tail 1.0 → fade starts 6.5.
    let g = build_timeline_filter(&clips, 0.5, 640, 360, 24, Some(("m", 1.0))).unwrap();
    assert!(g.contains("atrim=0:7.500"), "{g}");
    assert!(g.contains("afade=t=out:st=6.500:d=1.000"), "{g}");
    assert!(g.contains("[2:a]"), "{g}");
}
#[test]
fn even_frame_times_spans_inset_and_centers() {
    let ts = even_frame_times(6.0, 4).unwrap();
    assert_eq!(ts.len(), 4);
    assert!((ts[0] - 0.9375).abs() < 1e-9); // 0.25 + 5.5*0.5/4
    assert!((ts[3] - (0.25 + 5.5 * 3.5 / 4.0)).abs() < 1e-9);
    assert!(ts.windows(2).all(|w| (w[1] - w[0] - 1.375).abs() < 1e-9));
}

#[test]
fn even_frame_times_rejects_unknown_or_short_duration() {
    assert!(even_frame_times(0.0, 4).is_none());
    assert!(even_frame_times(0.5, 4).is_none());
    assert!(even_frame_times(6.0, 0).is_none());
    // single frame lands mid-clip
    let one = even_frame_times(10.0, 1).unwrap();
    assert!((one[0] - 5.0).abs() < 1e-9);
}
