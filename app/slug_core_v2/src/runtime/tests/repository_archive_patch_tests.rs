use super::*;

fn stream_scope(written: u64) -> Arc<SourceServiceScope> {
    use std::time::Duration;
    use std::time::Instant;

    use slug_workspace_v2::NormalizedAbsolutePath;

    use crate::runtime::dice::source_service::SourceServiceLimits;
    let work = Instant::now() + Duration::from_secs(2);
    SourceServiceScope::new_for_test(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        work,
        work + Duration::from_secs(1),
        SourceServiceLimits {
            written_bytes: written,
            ..SourceServiceLimits::default()
        },
    )
    .unwrap()
}

fn only_target_remains(root: &Path) {
    let names = std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    assert_eq!(names, [std::ffi::OsString::from("file")]);
}

#[test]
fn short_line_stream_preserves_shifted_hunks_suffix_mode_and_exact_quota() {
    let source = "a\n".repeat(16384);
    let root = root_file("file", source.as_bytes());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            root.path().join("file"),
            std::fs::Permissions::from_mode(0o640),
        )
        .unwrap();
    }
    let expected = format!(
        "a\na\nX\nY\n{}Z\n{}",
        "a\n".repeat(15996),
        "a\n".repeat(384)
    );
    let scope = stream_scope(expected.len() as u64);
    let patch = b"--- a/file\n+++ b/file\n@@ -2,2 +2,3 @@\n a\n-a\n+X\n+Y\n@@ -16000,1 +16001,1 @@\n-a\n+Z\n";
    apply_selected_bcr_patch(root.path(), patch, 1, &|| true, Some(&scope)).unwrap();
    assert_eq!(
        std::fs::read(root.path().join("file")).unwrap(),
        expected.as_bytes()
    );
    assert_eq!(scope.snapshot().written_bytes, expected.len() as u64);
    assert_eq!(scope.snapshot().live_buffer_bytes, 0);
    assert_eq!(scope.check(), Ok(()));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(root.path().join("file"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o640
        );
    }
    only_target_remains(root.path());
}

#[test]
fn invalid_final_hunk_never_stages_or_charges_earlier_output() {
    for (tail, message) in [
        ("@@ -3,1 +3,1 @@\n-wrong\n+C\n", "context does not match"),
        ("@@ -3,2 +3,1 @@\n-c\n+C\n", "line counts do not match"),
        ("@@ -9,1 +9,1 @@\n-c\n+C\n", "ranges are inconsistent"),
    ] {
        let root = root_file("file", b"a\nb\nc\nd\n");
        let scope = stream_scope(64);
        let patch = format!("--- a/file\n+++ b/file\n@@ -1,1 +1,1 @@\n-a\n+A\n{tail}");
        let error =
            apply_selected_bcr_patch(root.path(), patch.as_bytes(), 1, &|| true, Some(&scope))
                .unwrap_err();
        assert!(error.message.contains(message), "{error:?}");
        assert_eq!(
            std::fs::read(root.path().join("file")).unwrap(),
            b"a\nb\nc\nd\n"
        );
        assert_eq!(scope.snapshot().written_bytes, 0);
        assert_eq!(scope.snapshot().live_buffer_bytes, 0);
        only_target_remains(root.path());
    }
}

#[test]
fn streamed_write_quota_preserves_per_line_admission_and_original_on_failure() {
    use source_service::SourceCounter;
    use source_service::SourceServiceError;
    let patch = b"--- a/file\n+++ b/file\n@@ -2,1 +2,2 @@\n-b\n+B\n+extra\n";
    for quota in [11, 12] {
        let root = root_file("file", b"a\nb\nc\n");
        let scope = stream_scope(quota);
        let result = apply_selected_bcr_patch(root.path(), patch, 1, &|| true, Some(&scope));
        if quota == 12 {
            result.unwrap();
            assert_eq!(
                std::fs::read(root.path().join("file")).unwrap(),
                b"a\nB\nextra\nc\n"
            );
            assert_eq!(scope.snapshot().written_bytes, 12);
            assert_eq!(scope.check(), Ok(()));
        } else {
            let error = result.unwrap_err();
            let refusal = SourceServiceError::Limit(SourceCounter::WrittenBytes);
            assert!(error.message.contains(&refusal.to_string()), "{error:?}");
            assert_eq!(scope.check(), Err(refusal));
            assert_eq!(scope.snapshot().written_bytes, 10);
            assert_eq!(
                std::fs::read(root.path().join("file")).unwrap(),
                b"a\nb\nc\n"
            );
        }
        assert_eq!(scope.snapshot().live_buffer_bytes, 0);
        only_target_remains(root.path());
    }
}

#[test]
fn cancellation_in_final_validation_hunk_precedes_all_writes() {
    use std::cell::Cell;
    let root = root_file("file", b"a\nb\nc\n");
    let scope = stream_scope(64);
    let calls = Cell::new(0);
    let patch = b"--- a/file\n+++ b/file\n@@ -1,1 +1,1 @@\n-a\n+A\n@@ -3,1 +3,1 @@\n-c\n+C\n";
    // One parser check, two bounded-copy checks, then the two validation hunks.
    let error = apply_selected_bcr_patch(
        root.path(),
        patch,
        1,
        &|| {
            calls.set(calls.get() + 1);
            calls.get() < 5
        },
        Some(&scope),
    )
    .unwrap_err();
    assert!(error.message.contains("no longer active"), "{error:?}");
    assert_eq!(calls.get(), 5);
    assert_eq!(scope.snapshot().written_bytes, 0);
    assert_eq!(scope.snapshot().live_buffer_bytes, 0);
    assert_eq!(
        std::fs::read(root.path().join("file")).unwrap(),
        b"a\nb\nc\n"
    );
    only_target_remains(root.path());
}

#[test]
fn later_file_refusal_preserves_earlier_file_commit() {
    let root = root_file("file", b"a\n");
    std::fs::write(root.path().join("other"), b"b\n").unwrap();
    let scope = stream_scope(64);
    let patch = b"--- a/file\n+++ b/file\n@@ -1,1 +1,1 @@\n-a\n+A\n--- a/other\n+++ b/other\n@@ -1,1 +1,1 @@\n-wrong\n+B\n";
    assert!(
        apply_selected_bcr_patch(root.path(), patch, 1, &|| true, Some(&scope))
            .unwrap_err()
            .message
            .contains("context does not match")
    );
    assert_eq!(std::fs::read(root.path().join("file")).unwrap(), b"A\n");
    assert_eq!(std::fs::read(root.path().join("other")).unwrap(), b"b\n");
    assert_eq!(scope.snapshot().written_bytes, 2);
    assert_eq!(scope.snapshot().live_buffer_bytes, 0);
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 2);
}

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
    apply_selected_bcr_patch(root.path(), patch, 1, &|| true, None).unwrap();
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
        let error = apply_selected_bcr_patch(root.path(), patch, 1, &|| true, None).unwrap_err();
        assert!(error.message.contains(expected), "{error:?}");
    }

    let root = root_file("file", b"x\n");
    let mismatch = b"--- a/file\n+++ b/file\n@@ -1,1 +1,1 @@\n-z\n+y\n";
    assert!(
        apply_selected_bcr_patch(root.path(), mismatch, 1, &|| true, None)
            .unwrap_err()
            .message
            .contains("context does not match")
    );
    let patch = b"--- a/file\n+++ b/file\n@@ -1,1 +1,1 @@\n-x\n+y\n";
    assert!(
        apply_selected_bcr_patch(root.path(), patch, 1, &|| false, None)
            .unwrap_err()
            .message
            .contains("no longer active")
    );
    assert_eq!(std::fs::read(root.path().join("file")).unwrap(), b"x\n");
}

#[test]
fn scoped_patch_fallible_reservation_preserves_target_poison_and_cleanup() {
    use std::cell::Cell;
    use std::time::Duration;
    use std::time::Instant;

    use slug_workspace_v2::NormalizedAbsolutePath;
    use source_service::SourceServiceError;

    use crate::runtime::dice::source_service::SourceServiceLimits;

    let new_scope = || {
        let work = Instant::now() + Duration::from_secs(2);
        SourceServiceScope::new_for_test(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            work,
            work + Duration::from_secs(1),
            SourceServiceLimits::default(),
        )
        .unwrap()
    };
    let payload = b"old\n";
    let scope = new_scope();
    let root = root_file("file", payload);
    apply_selected_bcr_patch(
        root.path(),
        b"--- a/file\n+++ b/file\n@@ -1,1 +1,1 @@\n-old\n+new\n",
        1,
        &|| true,
        Some(&scope),
    )
    .unwrap();
    assert_eq!(std::fs::read(root.path().join("file")).unwrap(), b"new\n");
    assert_eq!(scope.check(), Ok(()));
    assert_eq!(scope.snapshot().live_buffer_bytes, 0);
    assert_eq!(scope.snapshot().written_bytes, 4);

    for mode in 0..4 {
        let scope = new_scope();
        let root = root_file("file", payload);
        let lines = ["-old\n", "+new\n"];
        let hunks = [(
            HunkHeader {
                old_start: 1,
                old_count: 1,
                new_start: 1,
                new_count: 1,
            },
            lines.as_slice(),
        )];
        let calls = Cell::new(0);
        let invoked = Cell::new(false);
        if mode == 3 {
            scope.cancel();
        }
        let error = apply_file_with_reserve(
            root.path(),
            "file",
            &hunks,
            &|| {
                calls.set(calls.get() + 1);
                true
            },
            Some(&scope),
            |bytes, capacity| {
                invoked.set(true);
                assert_eq!(capacity, payload.len());
                assert!(bytes.is_empty());
                assert_eq!(bytes.capacity(), 0);
                assert_eq!(
                    scope.snapshot().live_buffer_bytes,
                    payload.len() + 64 * 1024
                );
                if mode == 1 {
                    scope.cancel();
                }
                bytes.try_reserve_exact(if mode == 2 { capacity + 1 } else { usize::MAX })
            },
        )
        .unwrap_err();
        let expected = if mode == 1 || mode == 3 {
            SourceServiceError::Cancelled
        } else {
            SourceServiceError::Allocation
        };
        assert!(error.message.contains(&expected.to_string()), "{error:?}");
        assert_eq!(scope.check(), Err(expected));
        assert_eq!(invoked.get(), mode != 3);
        assert_eq!(calls.get(), 0, "copy never began");
        assert_eq!(std::fs::read(root.path().join("file")).unwrap(), payload);
        let entries = std::fs::read_dir(root.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        assert_eq!(
            entries,
            [std::ffi::OsString::from("file")],
            "no staged output"
        );
        assert_eq!(scope.snapshot().written_bytes, 0);
        assert_eq!(scope.snapshot().live_buffer_bytes, 0);
    }
}
