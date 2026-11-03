use super::*;

#[test]
fn parses_pr_forms() {
    let p = parse_github_uri("pr://").unwrap();
    assert!(matches!(p.kind, Kind::Pr));
    assert!(p.number.is_none() && p.repo.is_none());
    assert_eq!(parse_github_uri("pr://42").unwrap().number, Some(42));
    let p = parse_github_uri("pr://42/diff").unwrap();
    assert_eq!(p.number, Some(42));
    assert!(matches!(p.view, View::Diff));
    let p = parse_github_uri("pr://acme/widgets/7/files").unwrap();
    assert_eq!(p.repo.as_deref(), Some("acme/widgets"));
    assert_eq!(p.number, Some(7));
    assert!(matches!(p.view, View::Files));
}

#[test]
fn parses_issue_forms() {
    let p = parse_github_uri("issue://3/comments").unwrap();
    assert_eq!(p.kind, Kind::Issue);
    assert_eq!(p.number, Some(3));
    assert_eq!(p.view, View::Comments);
    let p = parse_github_uri("issue://o/r/9").unwrap();
    assert_eq!(p.repo.as_deref(), Some("o/r"));
    assert_eq!(p.number, Some(9));
}

#[test]
fn rejects_garbage() {
    assert!(parse_github_uri("pr://not-a-number").is_err());
    assert!(parse_github_uri("file.rs").is_err());
}

#[test]
fn gh_output_cap_preserves_utf8_at_and_across_the_limit() {
    const NOTICE: &str = "\n…[truncated pr:// / issue:// body]\n";

    for (prefix, character) in [(1, 'é'), (2, '€'), (3, '😀')] {
        let mut text = "x".repeat(MAX_GH_OUT - prefix);
        text.push(character);
        text.push_str("remaining");
        cap_gh_output(&mut text);
        assert_eq!(text, format!("{}{NOTICE}", "x".repeat(MAX_GH_OUT - prefix)));
    }

    let mut exact = "x".repeat(MAX_GH_OUT - 4);
    exact.push('😀');
    let original = exact.clone();
    cap_gh_output(&mut exact);
    assert_eq!(exact, original);

    let mut aligned = original;
    aligned.push('x');
    cap_gh_output(&mut aligned);
    assert_eq!(
        aligned,
        format!("{}{NOTICE}", "x".repeat(MAX_GH_OUT - 4) + "😀")
    );
}
