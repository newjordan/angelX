use super::*;
#[test]
fn explicit_task_driver_does_not_fall_back_to_practice() {
    let mut bag = Bag::practice_for_test();
    assert!(matches!(
        bag.require_task_driver("missing-glm-fixture"),
        Err(TaskDriverSelectionError::Unconfigured)
    ));
    assert!(bag.require_task_driver("practice").is_ok());
}
#[test]
fn explicit_task_driver_does_not_fall_back_to_another_slot() {
    let mut bag = Bag::for_render_test(&[("box", &[("preferred", false), ("other", true)])]);
    assert!(matches!(
        bag.require_task_driver("preferred"),
        Err(TaskDriverSelectionError::Unavailable)
    ));
    assert!(bag.require_task_driver("other").is_ok());
}
#[test]
fn explicit_task_driver_uses_existing_alias_rules() {
    let mut bag = Bag::for_render_test(&[("box", &[("openrouter", true)])]);
    assert!(bag.require_task_driver("or").is_ok());
    assert_eq!(bag.selected_route_indices(), (0, 0));
}
