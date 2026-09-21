#[test]
fn repo_features_are_typed_owned_and_separate_from_ignore_projection() {
    let (first, _) = evaluate(
        b"repo(features=['a', '-a', 'no_layering_check', ''])\nignore_directories(['ignored'])",
        RootRepoFileUtf8Mode::Warning,
    );
    let first = first.unwrap();
    assert_eq!(
        first.features().as_ref(),
        ["a", "-a", "no_layering_check", ""]
    );
    assert_eq!(first.ignored_directories(), ["ignored"]);
    let (other, _) = evaluate(
        b"repo(features=('b',))\nignore_directories(['ignored'])",
        RootRepoFileUtf8Mode::Warning,
    );
    let other = other.unwrap();
    assert_eq!(first.ignored_directories(), other.ignored_directories());
    assert_ne!(first, other);
    let (empty, _) = evaluate(b"repo(features=range(0))", RootRepoFileUtf8Mode::Warning);
    assert!(empty.unwrap().features().is_empty());
    // Existing unrelated keyword projection remains discard-only.
    let (unknown, _) = evaluate(
        b"repo(value=['not-a-feature'])",
        RootRepoFileUtf8Mode::Warning,
    );
    assert!(unknown.unwrap().features().is_empty());
    let (dict, _) = evaluate(
        b"repo(features={'key':False})",
        RootRepoFileUtf8Mode::Warning,
    );
    assert_eq!(dict.unwrap().features().as_ref(), ["key"]);
}

#[test]
fn repo_features_reject_invalid_container_and_elements_eagerly() {
    for source in [
        "repo(features='a')",
        "repo(features=None)",
        "repo(features={1:True})",
        "repo(features=[1])",
        "repo(features=range(1))",
    ] {
        let (result, _) = evaluate(source.as_bytes(), RootRepoFileUtf8Mode::Warning);
        assert!(
            matches!(result, Err(HostRepoFileError::Evaluation { .. })),
            "{source}: {result:?}"
        );
    }
}
