#[path = "../../../slug_build_api_v2/tests/cpp_compile_support/mod.rs"]
mod fixture;

#[test]
fn cpp_compile_command_execution_and_input_tree_projection_refuse() {
    let action = fixture::action(super::root_support::root());
    assert!(action.cpp_compile_spec().is_some());
    assert_eq!(
        slug_reapi_v2::ReapiCommand::from_action(&action).unwrap_err(),
        "CppCompile REAPI projection is not admitted"
    );
    assert_eq!(
        slug_reapi_v2::ReapiCommand::for_execution(&action).unwrap_err(),
        "CppCompile REAPI projection is not admitted"
    );
    assert_eq!(
        slug_reapi_v2::ReapiInputTree::from_action(&action)
            .unwrap_err()
            .to_string(),
        "CppCompile discovery-aware REAPI input trees are not admitted"
    );
    assert!(action.render_argv().is_err());
    let forged = slug_build_api_v2::ActionSpec::new(
        slug_build_api_v2::ActionKind::CppCompile,
        "CppCompile",
        action.outputs().to_vec(),
    );
    assert_eq!(
        slug_reapi_v2::ReapiInputTree::from_action(&forged)
            .unwrap_err()
            .to_string(),
        "CppCompile discovery-aware REAPI input trees are not admitted"
    );
}
