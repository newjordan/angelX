use super::*;

#[test]
fn background_panic_marker_is_scoped_and_nested() {
    assert!(!contained_background_panic());
    let outcome = catch_background_unwind(|| {
        assert!(contained_background_panic());
        let nested = catch_background_unwind(contained_background_panic);
        assert!(matches!(nested, Ok(true)));
        panic!("contained fixture");
    });
    assert!(outcome.is_err());
    assert!(!contained_background_panic());
}
