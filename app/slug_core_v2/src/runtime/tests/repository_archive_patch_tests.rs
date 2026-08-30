use super::*;

fn root_file(path: &str, bytes: &[u8]) -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join(path);
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    std::fs::write(target, bytes).unwrap();
    root
}

#[test]
fn applies_rules_shell_shape_multiple_files_and_hunks() {
    let root = root_file(
        "MODULE.bazel",
        b"module(\n    name = \"rules_shell\",\n    version = \"0.0.0\",\n)\n\nbazel_dep(name = \"bazel_features\", version = \"1.18.0\")\nbazel_dep(name = \"bazel_skylib\", version = \"1.6.1\")\n",
    );
    std::fs::write(root.path().join("other.txt"), b"one\ntwo\nthree\n").unwrap();
    let patch = b"===================================================================\n--- a/MODULE.bazel\n+++ b/MODULE.bazel\n@@ -1,7 +1,7 @@\n module(\n     name = \"rules_shell\",\n-    version = \"0.0.0\",\n+    version = \"0.6.1\",\n )\n \n bazel_dep(name = \"bazel_features\", version = \"1.18.0\")\n bazel_dep(name = \"bazel_skylib\", version = \"1.6.1\")\n--- a/other.txt\n+++ b/other.txt\n@@ -1,1 +1,1 @@\n-one\n+ONE\n@@ -3,1 +3,1 @@\n-three\n+THREE\n";
    apply_selected_bcr_patch(root.path(), patch, 1, &|| true).unwrap();
    assert_eq!(
        std::fs::read(root.path().join("MODULE.bazel")).unwrap(),
        b"module(\n    name = \"rules_shell\",\n    version = \"0.6.1\",\n)\n\nbazel_dep(name = \"bazel_features\", version = \"1.18.0\")\nbazel_dep(name = \"bazel_skylib\", version = \"1.6.1\")\n"
    );
    assert_eq!(
        std::fs::read(root.path().join("other.txt")).unwrap(),
        b"ONE\ntwo\nTHREE\n"
    );
}

#[test]
fn rejects_unsupported_paths_shapes_context_and_cancellation() {
    let cases: &[(&[u8], &str)] = &[
        (
            b"--- /dev/null\n+++ b/file\n@@ -0,0 +1,1 @@\n+x\n",
            "unsupported file path",
        ),
        (
            b"--- a/file\n+++ b/other\n@@ -1,1 +1,1 @@\n-x\n+y\n",
            "must be equal",
        ),
        (b"diff --git a/file b/file\n", "malformed file header"),
        (b"--- a/file\r\n+++ b/file\r\n", "UTF-8 LF"),
        (
            b"--- a/file\n+++ b/file\n@@ -1,1 +1,1 @@\n\\ No newline at end of file\n",
            "unsupported hunk line",
        ),
    ];
    for (patch, expected) in cases {
        let root = root_file("file", b"x\n");
        let error = apply_selected_bcr_patch(root.path(), patch, 1, &|| true).unwrap_err();
        assert!(error.message.contains(expected), "{error:?}");
    }

    let root = root_file("file", b"x\n");
    let mismatch = b"--- a/file\n+++ b/file\n@@ -1,1 +1,1 @@\n-z\n+y\n";
    assert!(
        apply_selected_bcr_patch(root.path(), mismatch, 1, &|| true)
            .unwrap_err()
            .message
            .contains("context does not match")
    );
    let patch = b"--- a/file\n+++ b/file\n@@ -1,1 +1,1 @@\n-x\n+y\n";
    assert!(
        apply_selected_bcr_patch(root.path(), patch, 1, &|| false)
            .unwrap_err()
            .message
            .contains("no longer active")
    );
    assert_eq!(std::fs::read(root.path().join("file")).unwrap(), b"x\n");
}
