//! Advisory metadata reporting must never decide semantic freshness.

use std::fs::File;
use std::fs::FileTimes;
use std::hint::black_box;

use super::*;

fn configured_labels(daemon: &mut Daemon, expression: &str) -> crate::QueryResult {
    daemon.cquery_with_bzlmod_inputs(
        expression,
        true,
        true,
        CqueryOutput::StarlarkLabel,
        BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        LockfileMode::Update,
        Vec::new(),
        root_setting_overlay(None),
    )
}

#[test]
fn advisory_miss_does_not_hide_configured_content_edit_or_restoration() {
    let guard = tempfile::tempdir().unwrap();
    let workspace = guard.path();
    write_configured_module(workspace, "module(name = \"demo\")\n");
    let definitions = workspace.join("pkg/defs.bzl");
    let definition = |name: &str| {
        format!(
            "def _impl(ctx):\n    return [DefaultInfo(files = depset([]))]\n{name} = rule(implementation = _impl)\nprobe = {name}\n"
        )
    };
    let original = definition("alpha_rule");
    let changed = definition("bravo_rule");
    assert_eq!(original.len(), changed.len());
    write(&definitions, &original);
    write(
        &workspace.join("pkg/BUILD.bazel"),
        "load(\":defs.bzl\", \"probe\")\nprobe(name = \"probe\")\n",
    );
    let modified = definitions.metadata().unwrap().modified().unwrap();
    let mut daemon = Daemon::new(workspace).unwrap();
    let alpha = "kind('^alpha_rule rule$', //pkg:probe)";
    let bravo = "kind('^bravo_rule rule$', //pkg:probe)";
    let first = configured_labels(&mut daemon, alpha);
    assert_eq!(first.exit_code, 0, "{first:?}");
    assert_eq!(first.stdout, "@@//pkg:probe\n");
    assert_eq!(first.invalidated_files, 0);

    for (source, selected, absent) in [(&changed, bravo, alpha), (&original, alpha, bravo)] {
        write(&definitions, source);
        File::options()
            .write(true)
            .open(&definitions)
            .unwrap()
            .set_times(FileTimes::new().set_modified(modified))
            .unwrap();
        // Make the private advisory state current without supplying any DICE
        // input. The genuine command must observe changed content itself.
        daemon.observations.observe(workspace).unwrap();
        let selected_result = configured_labels(&mut daemon, selected);
        assert_eq!(selected_result.invalidated_files, 0, "{selected_result:?}");
        assert_eq!(selected_result.exit_code, 0, "{selected_result:?}");
        assert_eq!(selected_result.stdout, "@@//pkg:probe\n");
        let absent_result = configured_labels(&mut daemon, absent);
        assert_eq!(absent_result.exit_code, 0, "{absent_result:?}");
        assert!(absent_result.stdout.is_empty(), "{absent_result:?}");
    }
}

#[test]
fn configured_command_does_not_load_unrelated_huge_binary_into_metrics() {
    let guard = tempfile::tempdir().unwrap();
    let workspace = guard.path();
    write_configured_module(workspace, "module(name = \"demo\")\n");
    write(
        &workspace.join("pkg/defs.bzl"),
        "def _impl(ctx):\n    return [DefaultInfo(files = depset([]))]\nprobe = rule(implementation = _impl)\n",
    );
    write(
        &workspace.join("pkg/BUILD.bazel"),
        "load(\":defs.bzl\", \"probe\")\nprobe(name = \"probe\")\n",
    );
    std::fs::create_dir(workspace.join("target")).unwrap();
    let binary = workspace.join("target/unrelated.bin");
    File::create(&binary).unwrap().set_len(128 << 30).unwrap();
    let mut daemon = Daemon::new(workspace).unwrap();
    let cold = configured_labels(&mut daemon, "//pkg:probe");
    assert_eq!(cold.exit_code, 0, "{cold:?}");
    assert_eq!(cold.stdout, "@@//pkg:probe\n");
    assert_eq!(cold.invalidated_files, 0);
    let warm = configured_labels(&mut daemon, "//pkg:probe");
    assert_eq!(warm.exit_code, 0, "{warm:?}");
    assert_eq!(warm.stdout, cold.stdout);
    assert_eq!(warm.invalidated_files, 0);
    File::options()
        .write(true)
        .open(binary)
        .unwrap()
        .set_len(256 << 30)
        .unwrap();
    let binary_changed = configured_labels(&mut daemon, "//pkg:probe");
    assert_eq!(binary_changed.exit_code, 0, "{binary_changed:?}");
    assert_eq!(binary_changed.stdout, cold.stdout);
    assert_eq!(binary_changed.invalidated_files, 1);
}

/// Run explicitly in an isolated process under /usr/bin/time. The legacy mode
/// is only permitted on a small control, never on the live checkout.
#[test]
#[ignore = "supervised scanner memory measurement; root and mode required"]
fn workspace_metrics_memory_probe() {
    let root = std::env::var_os("SLUG_WP880_METRICS_ROOT").expect("explicit probe root");
    let root = std::path::PathBuf::from(root);
    match std::env::var("SLUG_WP880_METRICS_MODE").as_deref() {
        Ok("metadata") => {
            let mut adapter = crate::workspace_metrics::FilesystemObservationAdapter::default();
            assert_eq!(adapter.observe(&root).unwrap(), 0);
            assert_eq!(adapter.observe(&root).unwrap(), 0);
            black_box(adapter);
        }
        Ok("legacy-small-control") => {
            // The supervisor supplies a fresh fixture containing only one
            // bounded sparse control; reject the real checkout explicitly.
            let entries = std::fs::read_dir(&root).unwrap().collect::<Vec<_>>();
            assert_eq!(entries.len(), 1);
            let entry = entries[0].as_ref().unwrap();
            assert!(entry.file_type().unwrap().is_file());
            assert!(entry.metadata().unwrap().len() <= 64 << 20);
            let first = slug_core_v2::runtime::observe_workspace(&root).unwrap();
            let previous = first.files.clone();
            drop(first);
            let current = slug_core_v2::runtime::observe_workspace(&root).unwrap();
            black_box((previous, current));
        }
        mode => panic!("explicit valid probe mode required: {mode:?}"),
    }
}
