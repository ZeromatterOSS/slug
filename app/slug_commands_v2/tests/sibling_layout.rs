//! The fixed nonsibling layout query must not silently admit its unmodeled flag.
use slug_commands_v2::build::BuildRequest;

#[test]
fn build_rejects_unadmitted_sibling_repository_layout_before_dispatch() {
    for flag in [
        "--experimental_sibling_repository_layout",
        "--experimental_sibling_repository_layout=true",
        "--experimental_sibling_repository_layout=false",
        "--noexperimental_sibling_repository_layout",
    ] {
        let error = BuildRequest::parse(&[flag, "//pkg:target"])
            .unwrap_err()
            .to_string();
        assert!(error.contains(flag), "{error}");
        assert!(error.contains("not supported by build"), "{error}");
    }
}
