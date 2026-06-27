use std::path::PathBuf;

use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::layout::BazelLayout;

#[test]
fn bazel_layout_uses_execroot_and_bazel_out() {
    let layout = BazelLayout::new("/workspace", "/output-base", "main");
    assert_eq!(
        layout.execroot(),
        PathBuf::from("/output-base").join("execroot").join("main")
    );
    assert_eq!(
        layout.bin_dir("k8-fastbuild"),
        PathBuf::from("/output-base")
            .join("execroot")
            .join("main")
            .join("bazel-out")
            .join("k8-fastbuild")
            .join("bin")
    );
    assert_eq!(
        layout.testlogs_dir("k8-fastbuild"),
        PathBuf::from("/output-base")
            .join("execroot")
            .join("main")
            .join("bazel-out")
            .join("k8-fastbuild")
            .join("testlogs")
    );
}

#[test]
fn external_repo_paths_are_bazel_shaped() {
    let layout = BazelLayout::new("/workspace", "/output-base", "main");
    let repo = CanonicalRepoName::new("rules_cc~0.1.0").unwrap();
    let path = layout.external_repo_dir(&repo);
    let text = path.to_string_lossy();
    assert!(text.contains("external"));
    assert!(text.contains("rules_cc~0.1.0"));
}
