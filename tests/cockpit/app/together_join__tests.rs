use super::*;

#[test]
fn invitations_parse_with_or_without_the_command_and_scheme() {
    let token = "0123456789abcdef0123456789abcdef";
    for link in [
        format!("http://100.94.1.2:8767/#{token}"),
        format!("/dungeon join http://100.94.1.2:8767/#{token}"),
        format!("100.94.1.2:8767/#{token}"),
    ] {
        let (base, got) = parse_link(&link).unwrap();
        assert_eq!(base, "http://100.94.1.2:8767");
        assert_eq!(got, token);
    }
    assert!(parse_link("http://100.94.1.2:8767/").is_err());
    assert!(parse_link("http://100.94.1.2:8767/#short").is_err());
}
