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
