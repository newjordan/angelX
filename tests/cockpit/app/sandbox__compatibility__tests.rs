use super::*;
#[test]
fn sandbox_doctor_classes_and_exact_remedies() {
    let cases = [
        (
            false,
            true,
            false,
            false,
            "",
            Cause::MissingBwrap,
            "sudo apt install bubblewrap",
        ),
        (
            true,
            false,
            false,
            false,
            "",
            Cause::MissingUserns,
            "CONFIG_USER_NS=y",
        ),
        (
            true,
            true,
            false,
            true,
            "",
            Cause::UsernsDisabled,
            "sudo sysctl -w kernel.unprivileged_userns_clone=1",
        ),
        (
            true,
            true,
            true,
            false,
            "bwrap: setting up uid map: Permission denied",
            Cause::AppArmor,
            "sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0",
        ),
    ];
    for (b, u, a, c, stderr, expected, remedy) in cases {
        let cause = classify(b, u, a, c, stderr).unwrap();
        assert_eq!(cause, expected);
        assert!(cause.remedy().contains(remedy));
        assert!(cause.notice().contains("Landlock-only"));
    }
    assert_eq!(classify(true, true, true, false, ""), None);
}
