use super::*;

#[test]
fn d02_regular_redirection_keeps_numeric_and_dash_file_targets() {
    assert_eq!(
        literal_output_redirection_targets("printf ok > 123; printf ok > -").unwrap(),
        vec!["123", "-"]
    );
    assert!(
        literal_output_redirection_targets("printf ok 2>&1; printf ok 2>&-")
            .unwrap()
            .is_empty()
    );
}
