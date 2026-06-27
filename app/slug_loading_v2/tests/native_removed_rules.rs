use slug_loading_v2::globals::native::removed_language_rule;

#[test]
fn language_rules_are_marked_removed_from_native() {
    assert!(removed_language_rule("cc_library"));
    assert!(removed_language_rule("py_test"));
    assert!(!removed_language_rule("filegroup"));
    assert!(!removed_language_rule("alias"));
}
