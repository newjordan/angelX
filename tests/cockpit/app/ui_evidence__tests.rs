use super::*;

fn targets(text: &str) -> Vec<(String, bool)> {
    candidates(text)
        .into_iter()
        .map(|c| (c.target.to_string(), c.evidence))
        .collect()
}

struct TempDir(std::path::PathBuf);
impl TempDir {
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn tempdir(tag: &str) -> TempDir {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("angel-evidence-{tag}-{}-{unique}", std::process::id()));
    std::fs::create_dir_all(&path).unwrap();
    TempDir(path)
}

#[test]
fn evidence_line_link_comes_first_even_when_earlier() {
    let text = "Changed [the loop](cockpit/src/drive/loop_ctl.rs).\n\nEvidence: [iteration report](reports/iteration-7.md).\n\nSee also [the chart](out/run.csv).";
    assert_eq!(
        targets(text),
        vec![
            ("reports/iteration-7.md".into(), true),
            ("out/run.csv".into(), false),
            ("cockpit/src/drive/loop_ctl.rs".into(), false),
        ]
    );
}

#[test]
fn the_last_evidence_link_wins_and_list_items_count_as_lines() {
    let text = "- Evidence: [first](a/one.md), [second](a/two.png)\n- Proof: [third](a/three.log)\n";
    let found = targets(text);
    assert_eq!(found[0], ("a/three.log".into(), true));
    assert_eq!(found[1], ("a/two.png".into(), true));
    assert_eq!(found[2], ("a/one.md".into(), true));
}

#[test]
fn web_urls_anchors_mail_and_code_are_not_candidates() {
    let text = "Evidence: [site](https://example.com/r.md), <http://example.com/x.png>, [top](#top), [mail](mailto:a@b.c), [ssh](ssh://h/f.png)\n\n```text\n[fake](fake.png)\n```\n\n`[also fake](fake2.png)`";
    assert!(targets(text).is_empty(), "{:?}", targets(text));
}

#[test]
fn file_urls_count_as_local_and_duplicates_fold() {
    let text = "Evidence: [shot](file:///w/shot.png) and [again](file:///w/shot.png)";
    assert_eq!(targets(text), vec![("file:///w/shot.png".into(), true)]);
}

#[test]
fn plain_prose_has_no_candidates() {
    assert!(candidates("All done, no links here.").is_empty());
    assert!(candidates("").is_empty());
}

#[test]
fn media_always_shows_reports_only_as_evidence_and_code_never() {
    for media in ["a.png", "b.JPG", "c.mp4", "d.glb", "e.obj", "f.csv", "g.tsv", "h.webp"] {
        assert!(worth_showing(Path::new(media), false), "{media}");
        assert!(worth_showing(Path::new(media), true), "{media}");
    }
    for report in ["r.md", "r.txt", "r.log", "r.json", "r.jsonl", "r.yaml", "r.diff"] {
        assert!(!worth_showing(Path::new(report), false), "{report}");
        assert!(worth_showing(Path::new(report), true), "{report}");
    }
    for never in ["main.rs", "app.py", "page.html", "archive.zip", "binary", "doc.pdf"] {
        assert!(!worth_showing(Path::new(never), true), "{never}");
    }
}

#[test]
fn evidence_media_resolves_inside_the_workspace_only() {
    let dir = tempdir("root");
    let root = dir.path();
    std::fs::create_dir_all(root.join("reports")).unwrap();
    std::fs::write(root.join("reports/iteration-7.md"), "# Iteration 7\n").unwrap();
    std::fs::write(root.join("main.rs"), "fn main() {}\n").unwrap();
    let outside = tempdir("outside");
    let stray = outside.path().join("stray.png");
    std::fs::write(&stray, b"not really a png").unwrap();

    let reply = format!(
        "Evidence: [iteration report](reports/iteration-7.md).\n\nAlso [outside]({}) and [code](main.rs) and [gone](missing.png).",
        stray.display()
    );
    let media = evidence_media(&reply, root).expect("the report resolves");
    assert_eq!(media.label(), "iteration-7.md");
    assert!(matches!(media, crate::ui::media::Media::Confined { .. }));

    // Not called evidence: a report alone stays a link.
    assert!(evidence_media("Wrote [notes](reports/iteration-7.md).", root).is_none());
    // Outside the workspace, source code and missing files never open.
    let rest = format!("Evidence: [outside]({}) [code](main.rs) [gone](missing.png)", stray.display());
    assert!(evidence_media(&rest, root).is_none());
    // An absolute path inside the workspace resolves like a relative one.
    let absolute = format!("Evidence: [report]({})", root.join("reports/iteration-7.md").display());
    assert!(evidence_media(&absolute, root).is_some());
}

#[test]
fn outside_the_window_a_reply_with_evidence_changes_nothing() {
    let _guard = crate::tests::env_lock();
    let mut app = crate::app::App::preview(crate::ui::viewer::Viewer::static_preview());
    let (media, messages) = (app.media.len(), app.messages.len());
    app.show_reply_evidence("Evidence: [shot](Cargo.toml) and [chart](data.csv)");
    assert_eq!(app.media.len(), media);
    assert_eq!(app.messages.len(), messages);
}

#[test]
fn a_recent_close_holds_unasked_showings_back() {
    use crate::ui::showcase::{EVIDENCE_HOLD, held};
    let now = std::time::Instant::now();
    assert!(!held(None, now));
    assert!(held(Some(now), now));
    assert!(held(Some(now), now + EVIDENCE_HOLD / 2));
    assert!(!held(Some(now), now + EVIDENCE_HOLD));
}
