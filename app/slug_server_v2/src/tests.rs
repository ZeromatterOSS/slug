/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file or the Apache-License, Version 2.0 found in the
 * LICENSE-APACHE file in the root directory of this source tree. You may
 * select, at your option, one of the above-listed licenses.
 */

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

use slug_bzlmod_v2::BzlmodCommandPolicyKey;
use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
use slug_bzlmod_v2::LockfileMode;
use slug_identity_v2::TargetPattern;
use slug_query_v2::QueryOrder;
use slug_query_v2::QueryPolicy;
use slug_reapi_v2::RemoteConfig;

use crate::BuildRequest;
use crate::BzlmodRequestInputs;
use crate::CqueryRequest;
use crate::Daemon;
use crate::DaemonRequest;
use crate::DaemonResponse;
use crate::QueryRequest;
use crate::server::handle_request;

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("slug-server-{name}-{nanos}"));
    fs::create_dir_all(&root).unwrap();
    root
}

fn write(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

fn remote_disabled() -> RemoteConfig {
    RemoteConfig {
        executor: None,
        cache: None,
        instance_name: None,
        headers: BTreeMap::new(),
        timeout_seconds: None,
        retry_attempts: None,
        default_exec_properties: BTreeMap::new(),
    }
}

fn target(label: &str) -> TargetPattern {
    TargetPattern::parse(label).unwrap()
}

#[test]
fn daemon_new_retains_only_its_runtime_and_test_process_host_arcs() {
    let workspace = scratch("process-host-owner");
    let daemon = Daemon::new(&workspace).unwrap();
    assert_eq!(
        std::sync::Arc::strong_count(&daemon.process_host_for_test),
        2
    );
}

#[test]
fn daemon_bzlmod_inputs_are_request_local_default_override_default() {
    let workspace = scratch("bzlmod-request-local");
    write(
        &workspace.join("MODULE.bazel"),
        "module(name = \"demo\")\nbazel_dep(name = \"dev_dep\", version = \"1.0\", dev_dependency = True)\n",
    );
    write(&workspace.join("BUILD.bazel"), "");
    write(
        &workspace.join("pkg/BUILD.bazel"),
        "filegroup(name = \"probe\")\n",
    );
    let mut daemon = Daemon::new(&workspace).unwrap();
    let defaults = || {
        (
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
            Vec::new(),
        )
    };
    let overrides = || {
        (
            BzlmodCommandPolicyKey::from_flags(Some("all"), true).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(Some("all")).unwrap(),
            LockfileMode::Off,
            vec!["https://registry.example/override/".to_owned()],
        )
    };
    let expected = vec![
        BzlmodRequestInputs::default(),
        BzlmodRequestInputs {
            command_allow_yanked_versions: Some("all".to_owned()),
            ignore_dev_dependency: true,
            environment_allow_yanked_versions: Some("all".to_owned()),
            lockfile_mode: "off".to_owned(),
            registry_urls: vec!["https://registry.example/override/".to_owned()],
        },
        BzlmodRequestInputs::default(),
    ];

    for inputs in [defaults(), overrides(), defaults()] {
        let result = daemon.build_with_bzlmod_inputs(
            &[target("//pkg:probe")],
            &remote_disabled(),
            &[],
            inputs.0,
            inputs.1,
            inputs.2,
            inputs.3,
        );
        assert!(!result.stderr.contains("build_runtime_error"), "{result:?}");
    }
    assert_eq!(daemon.take_forwarded_bzlmod_inputs_for_test(), expected);

    for inputs in [defaults(), overrides(), defaults()] {
        let result = daemon.query_with_output_policy_and_bzlmod_inputs(
            "//pkg:probe",
            QueryOrder::Auto,
            "text",
            true,
            QueryPolicy::default(),
            inputs.0,
            inputs.1,
            inputs.2,
            inputs.3,
        );
        assert_eq!(result.exit_code, 0, "{result:?}");
        assert_eq!(result.stdout, "//pkg:probe\n");
    }
    assert_eq!(daemon.take_forwarded_bzlmod_inputs_for_test(), expected);
}

const DEFS_BZL: &str = "\
load(\":message.bzl\", \"MESSAGE\")
def _impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + \".txt\")
    ctx.actions.write(out, MESSAGE + \"\\n\")
    return [DefaultInfo(files = depset([out]))]
message_rule = rule(implementation = _impl)
";

const BUILD_BAZEL: &str =
    "load(\":defs.bzl\", \"message_rule\")\nmessage_rule(name = \"message\")\n";

/// The first build populates the digest cache; zero files are invalidated
/// because nothing was previously cached.
#[test]
fn first_build_invalidates_zero_files() {
    let workspace = scratch("first-build");
    write(&workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    write(&workspace.join("BUILD.bazel"), "");
    write(
        &workspace.join("pkg/BUILD.bazel"),
        "filegroup(name = \"probe\", srcs = [])\n",
    );

    let mut daemon = Daemon::new(&workspace).unwrap();
    let result = daemon.build(&[target("//pkg:probe")], &remote_disabled(), &[]);
    assert_eq!(result.invalidated_files, 0);
    assert!(result.stderr.contains("\"invalidated_files\":0"));
}

/// Editing a loaded `.bzl` file between builds causes the daemon to invalidate
/// exactly one path and recompute the dependent package.
#[test]
fn bzl_edit_invalidates_one_file_on_second_build() {
    let workspace = scratch("bzl-edit");
    let package = workspace.join("pkg");
    write(&workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    write(&workspace.join("BUILD.bazel"), "");
    write(&package.join("message.bzl"), "MESSAGE = \"one\"\n");
    write(&package.join("defs.bzl"), DEFS_BZL);
    write(&package.join("BUILD.bazel"), BUILD_BAZEL);

    let mut daemon = Daemon::new(&workspace).unwrap();
    // First build: populates cache, 0 invalidated.
    let first = daemon.build(&[target("//pkg:message")], &remote_disabled(), &[]);
    assert_eq!(first.invalidated_files, 0);

    // Edit message.bzl: "one" -> "two".
    write(&package.join("message.bzl"), "MESSAGE = \"two\"\n");

    // Second build: exactly 1 file invalidated (message.bzl).
    let second = daemon.build(&[target("//pkg:message")], &remote_disabled(), &[]);
    assert_eq!(second.invalidated_files, 1);
}

/// A third build with no further edits invalidates zero files (the digest
/// cache matches the current state).
#[test]
fn third_build_after_no_edit_invalidates_zero() {
    let workspace = scratch("third-build");
    let package = workspace.join("pkg");
    write(&workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    write(&workspace.join("BUILD.bazel"), "");
    write(&package.join("message.bzl"), "MESSAGE = \"one\"\n");
    write(&package.join("defs.bzl"), DEFS_BZL);
    write(&package.join("BUILD.bazel"), BUILD_BAZEL);

    let mut daemon = Daemon::new(&workspace).unwrap();
    daemon.build(&[target("//pkg:message")], &remote_disabled(), &[]);
    write(&package.join("message.bzl"), "MESSAGE = \"two\"\n");
    let second = daemon.build(&[target("//pkg:message")], &remote_disabled(), &[]);
    assert_eq!(second.invalidated_files, 1);
    let third = daemon.build(&[target("//pkg:message")], &remote_disabled(), &[]);
    assert_eq!(third.invalidated_files, 0);
}

#[test]
fn retained_daemon_build_publishes_cold_and_changed_events_without_warm_replay() {
    let workspace = scratch("build-selected-events");
    let package = workspace.join("pkg");
    write(
        &workspace.join("MODULE.bazel"),
        "print(\"MODULE_EVENT\")\nmodule(name = \"demo\")\n",
    );
    write(
        &package.join("defs.bzl"),
        "print(\"BZL_EVENT\")\ndef _impl(ctx):\n    print(\"ANALYSIS_EVENT\")\n    return [DefaultInfo(files = depset([]))]\nprobe = rule(implementation = _impl)\n",
    );
    write(
        &package.join("BUILD.bazel"),
        "load(\":defs.bzl\", \"probe\")\nprint(\"BUILD_EVENT\")\nprobe(name = \"probe\")\n",
    );
    let workspace = workspace.canonicalize().unwrap();
    let cold_events = format!(
        "DEBUG: {}:1:6: MODULE_EVENT\nDEBUG: {}:1:6: BZL_EVENT\nDEBUG: {}:2:6: BUILD_EVENT\nDEBUG: {}:3:10: ANALYSIS_EVENT\n",
        workspace.join("MODULE.bazel").display(),
        workspace.join("pkg/defs.bzl").display(),
        workspace.join("pkg/BUILD.bazel").display(),
        workspace.join("pkg/defs.bzl").display(),
    );
    let mut daemon = Daemon::new(&workspace).unwrap();
    let targets = [target("//pkg:probe")];

    let cold = daemon.build(&targets, &remote_disabled(), &[]);
    assert_eq!(cold.exit_code, 2, "{cold:?}");
    assert!(cold.stderr.starts_with(&cold_events), "{cold:?}");
    assert_eq!(cold.invalidated_files, 0);

    let warm = daemon.build(&targets, &remote_disabled(), &[]);
    assert_eq!(warm.exit_code, 2, "{warm:?}");
    assert!(!warm.stderr.contains("DEBUG:"), "{warm:?}");
    assert_eq!(warm.invalidated_files, 0);

    write(
        &package.join("BUILD.bazel"),
        "load(\":defs.bzl\", \"probe\")\nprint(\"BUILD_CHANGED\")\nprobe(name = \"probe\")\n",
    );
    let changed = daemon.build(&targets, &remote_disabled(), &[]);
    assert_eq!(changed.exit_code, 2, "{changed:?}");
    assert!(
        changed.stderr.starts_with(&format!(
            "DEBUG: {}:2:6: BUILD_CHANGED\n",
            workspace.join("pkg/BUILD.bazel").display()
        )),
        "{changed:?}"
    );
    assert_eq!(changed.invalidated_files, 1);

    let warm_after_change = daemon.build(&targets, &remote_disabled(), &[]);
    assert_eq!(warm_after_change.exit_code, 2, "{warm_after_change:?}");
    assert!(
        !warm_after_change.stderr.contains("DEBUG:"),
        "{warm_after_change:?}"
    );
    assert_eq!(warm_after_change.invalidated_files, 0);
}

#[test]
fn missing_loaded_bzl_is_absent_then_create_is_observed_without_a_key_panic() {
    let workspace = scratch("missing-then-create-bzl");
    let package = workspace.join("pkg");
    write(&workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    write(&workspace.join("BUILD.bazel"), "");
    write(
        &package.join("BUILD.bazel"),
        "load(\":defs.bzl\", \"declare\")\ndeclare()\n",
    );

    let mut daemon = Daemon::new(&workspace).unwrap();
    let missing = daemon.build(&[target("//pkg:probe")], &remote_disabled(), &[]);
    assert!(
        missing
            .stderr
            .contains("cannot load '//pkg:defs.bzl': no such file"),
        "{missing:?}"
    );

    write(
        &package.join("defs.bzl"),
        "def declare():\n    native.filegroup(name = \"probe\", srcs = [])\n",
    );
    let created = daemon.build(&[target("//pkg:probe")], &remote_disabled(), &[]);
    assert_eq!(created.invalidated_files, 1);
    assert!(
        !created.stderr.contains("build_runtime_error"),
        "{created:?}"
    );
    assert!(created.stderr.contains("dice_starlark_package_loading"));
}

#[test]
fn transitive_missing_bzl_reports_the_deepest_label_and_recovers() {
    let workspace = scratch("transitive-missing-then-create-bzl");
    let package = workspace.join("pkg");
    write(&workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    write(
        &package.join("BUILD.bazel"),
        "load(\":defs.bzl\", \"declare\")\ndeclare()\n",
    );
    write(
        &package.join("defs.bzl"),
        "load(\":missing.bzl\", \"NAME\")\ndef declare():\n    native.filegroup(name = NAME)\n",
    );

    let mut daemon = Daemon::new(&workspace).unwrap();
    let missing = daemon.build(&[target("//pkg:probe")], &remote_disabled(), &[]);
    assert!(
        missing
            .stderr
            .contains("cannot load '//pkg:missing.bzl': no such file"),
        "{missing:?}"
    );
    assert!(!missing.stderr.contains("cannot load '//pkg:defs.bzl'"));

    write(&package.join("missing.bzl"), "NAME = \"probe\"\n");
    let created = daemon.build(&[target("//pkg:probe")], &remote_disabled(), &[]);
    assert_eq!(created.invalidated_files, 1);
    assert!(
        !created.stderr.contains("build_runtime_error"),
        "{created:?}"
    );
}

#[test]
fn retained_runtime_switches_from_build_bazel_to_build_fallback() {
    let workspace = scratch("build-fallback-transition");
    let package = workspace.join("pkg");
    write(&workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    write(&workspace.join("BUILD.bazel"), "");
    let primary = package.join("BUILD.bazel");
    write(&primary, "filegroup(name = \"primary\", srcs = [])\n");

    let mut daemon = Daemon::new(&workspace).unwrap();
    let first = daemon.build(&[target("//pkg:primary")], &remote_disabled(), &[]);
    assert!(!first.stderr.contains("build_runtime_error"), "{first:?}");

    fs::remove_file(&primary).unwrap();
    write(
        &package.join("BUILD"),
        "filegroup(name = \"fallback\", srcs = [])\n",
    );
    let fallback = daemon.build(&[target("//pkg:fallback")], &remote_disabled(), &[]);
    assert_eq!(fallback.invalidated_files, 2);
    assert!(
        !fallback.stderr.contains("build_runtime_error"),
        "{fallback:?}"
    );
    assert!(fallback.stderr.contains("dice_starlark_package_loading"));
}

#[test]
fn tagged_query_protocol_carries_output_and_preserves_old_request_defaults() {
    let request = DaemonRequest::Query(QueryRequest {
        expression: "deps(//pkg:bin)".to_owned(),
        order_output: "full".to_owned(),
        output: "graph".to_owned(),
        graph_factored: false,
        strict_test_suite: true,
        bzlmod: BzlmodRequestInputs::default(),
    });
    let json = serde_json::to_value(request).unwrap();
    assert_eq!(json["kind"], "query");
    assert_eq!(json["request"]["expression"], "deps(//pkg:bin)");
    assert_eq!(json["request"]["order_output"], "full");
    assert_eq!(json["request"]["output"], "graph");
    assert_eq!(json["request"]["graph_factored"], false);
    assert_eq!(json["request"]["strict_test_suite"], true);
    assert_eq!(json["request"].as_object().unwrap().len(), 6);

    let old: DaemonRequest = serde_json::from_str(
        r#"{"kind":"query","request":{"expression":"//pkg:bin","order_output":"auto"}}"#,
    )
    .unwrap();
    let DaemonRequest::Query(old) = old else {
        panic!("expected query request");
    };
    assert_eq!(old.output, "text");
    assert!(old.graph_factored);
    assert!(!old.strict_test_suite);
    assert_eq!(old.bzlmod, BzlmodRequestInputs::default());
}

#[test]
fn retained_daemon_strict_test_suite_toggle_is_request_local_and_invalidates_no_files() {
    let workspace = scratch("strict-test-suite-policy");
    write(&workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    write(
        &workspace.join("pkg/BUILD.bazel"),
        "filegroup(name = \"plain\")\ntest_suite(name = \"suite\", tests = [\":plain\"])\n",
    );
    let mut daemon = Daemon::new(&workspace).unwrap();
    let expression = "tests(//pkg:suite)";

    let default = daemon.query(expression, QueryOrder::Auto);
    assert_eq!(default.exit_code, 0, "{default:?}");
    assert!(default.stdout.is_empty(), "{default:?}");
    assert_eq!(default.invalidated_files, 0);

    let strict = daemon.query_with_policy(
        expression,
        QueryOrder::Auto,
        QueryPolicy {
            strict_test_suite: true,
        },
    );
    assert_eq!(strict.exit_code, 7, "{strict:?}");
    assert!(strict.stdout.is_empty(), "{strict:?}");
    assert_eq!(strict.invalidated_files, 0);
    assert!(
        strict.stderr.contains(
            "The label '//pkg:plain' in the test_suite '//pkg:suite' does not refer to a test or test_suite rule!"
        ),
        "{strict:?}"
    );

    let default_again = daemon.query(expression, QueryOrder::Auto);
    assert_eq!(default_again.exit_code, 0, "{default_again:?}");
    assert!(default_again.stdout.is_empty(), "{default_again:?}");
    assert_eq!(default_again.invalidated_files, 0);
}

#[test]
fn retained_daemon_formats_graph_from_the_same_query_result_path() {
    let workspace = scratch("query-graph-output");
    write(&workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    write(&workspace.join("shared/BUILD.bazel"), "");
    write(&workspace.join("shared/defs.bzl"), "VALUE = 1\n");
    write(
        &workspace.join("pkg/BUILD.bazel"),
        "load(\"//shared:defs.bzl\", \"VALUE\")\nfilegroup(name = \"probe\", srcs = [])\n",
    );

    let mut daemon = Daemon::new(&workspace).unwrap();
    let graph = daemon.query_with_output("loadfiles(//pkg:probe)", QueryOrder::Full, "graph", true);
    assert_eq!(graph.exit_code, 0, "{graph:?}");
    assert!(graph.stderr.is_empty(), "{graph:?}");
    assert_eq!(
        graph.stdout,
        concat!(
            "digraph mygraph {\n",
            "  node [shape=box];\n",
            "  \"//shared:defs.bzl\"\n",
            "}\n",
        )
    );

    let label_kind = daemon.query_with_output("//pkg:probe", QueryOrder::Full, "label_kind", true);
    assert_eq!(label_kind.exit_code, 0, "{label_kind:?}");
    assert!(label_kind.stderr.is_empty(), "{label_kind:?}");
    assert_eq!(label_kind.stdout, "filegroup rule //pkg:probe\n");
}

#[test]
fn retained_daemon_matches_the_three_accepted_package_rows() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/v2_oracle/fixtures/query-loading-thin-vertical/workspace");
    let mut daemon = Daemon::new(&workspace).unwrap();
    for (expression, expected) in [
        (
            "set(//nested/child:child //:root //app:app //nested:branch //app:via_alias)",
            "\napp\nnested\nnested/child\n",
        ),
        ("deps(//app:app)", "app\nlib\nnested\nnested/child\n"),
        ("loadfiles(//app:app)", "rules\n"),
    ] {
        for order in [QueryOrder::Auto, QueryOrder::Full] {
            let result = daemon.query_with_output(expression, order, "package", true);
            assert_eq!(result.exit_code, 0, "{expression}, {order}: {result:?}");
            assert!(
                result.stderr.is_empty(),
                "{expression}, {order}: {result:?}"
            );
            assert_eq!(result.stdout, expected, "{expression}, {order}");
        }
    }
}

#[test]
fn retained_daemon_accepts_explicit_label_output() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/v2_oracle/fixtures/query-path-topology/workspace");
    let mut daemon = Daemon::new(&workspace).unwrap();
    let expression = "allpaths(//:linear_start, //:linear_end)";
    for (order, expected) in [
        (
            QueryOrder::Auto,
            "//:linear_end\n//:linear_mid\n//:linear_start\n",
        ),
        (
            QueryOrder::Full,
            "//:linear_start\n//:linear_mid\n//:linear_end\n",
        ),
    ] {
        let result = daemon.query_with_output(expression, order, "label", true);
        assert_eq!(result.exit_code, 0, "{order}: {result:?}");
        assert!(result.stderr.is_empty(), "{order}: {result:?}");
        assert_eq!(result.stdout, expected, "{order}");
    }
}

#[test]
fn retained_daemon_matches_the_ten_accepted_label_kind_rows() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/v2_oracle/fixtures");
    let rule_workspace = fixtures.join("query-executables-rule-capability/workspace");
    let mut rules = Daemon::new(&rule_workspace).unwrap();
    for (expression, expected) in [
        (
            "//pkg:arbitrary_target",
            "exec_arbitrary rule //pkg:arbitrary_target\n",
        ),
        (
            "//pkg:ordinary_target",
            "implicit_test_test rule //pkg:ordinary_target\n",
        ),
        (
            "//pkg:explicit_test_target",
            "explicit_test_test rule //pkg:explicit_test_target\n",
        ),
        ("//pkg:plain", "plain_rule rule //pkg:plain\n"),
        (
            "//pkg:generated_owner",
            "output_rule rule //pkg:generated_owner\n",
        ),
        ("//pkg:files", "filegroup rule //pkg:files\n"),
        ("//pkg:alias_exec", "alias rule //pkg:alias_exec\n"),
        ("//pkg:setting", "config_setting rule //pkg:setting\n"),
    ] {
        let result = rules.query_with_output(expression, QueryOrder::Auto, "label_kind", true);
        assert_eq!(result.exit_code, 0, "{expression}: {result:?}");
        assert!(result.stderr.is_empty(), "{expression}: {result:?}");
        assert_eq!(result.stdout, expected, "{expression}");
    }

    let generated_workspace = fixtures.join("query-labels-attribute-metadata/workspace");
    let mut generated = Daemon::new(&generated_workspace).unwrap();
    for (expression, expected) in [
        (
            "labels(out, //pkg:outputs)",
            "generated file //pkg:one.out\n",
        ),
        (
            "labels(outs, //pkg:outputs)",
            "generated file //pkg:three.out\ngenerated file //pkg:two.out\n",
        ),
    ] {
        let result = generated.query_with_output(expression, QueryOrder::Auto, "label_kind", true);
        assert_eq!(result.exit_code, 0, "{expression}: {result:?}");
        assert!(result.stderr.is_empty(), "{expression}: {result:?}");
        assert_eq!(result.stdout, expected, "{expression}");
    }
}

#[test]
fn label_kind_completes_cross_package_depth_boundaries_without_changing_standard_queries() {
    let workspace = scratch("query-label-kind-depth-boundary");
    write(&workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    let producer_defs = workspace.join("producer/defs.bzl");
    let producer_build = workspace.join("producer/BUILD.bazel");
    write(
        &producer_defs,
        "def _impl(ctx):\n    return [DefaultInfo()]\nfirst_rule = rule(implementation = _impl)\n",
    );
    write(
        &producer_build,
        "load(\":defs.bzl\", \"first_rule\")\nfirst_rule(name = \"rule\")\n",
    );
    write(
        &workspace.join("consumer/BUILD.bazel"),
        "filegroup(name = \"consumer\", srcs = [\"//producer:rule\"])\n",
    );
    let expression = "deps(//consumer:consumer, 1)";
    let mut daemon = Daemon::new(&workspace).unwrap();

    let standard = daemon.query(expression, QueryOrder::Auto);
    assert_eq!(standard.exit_code, 0, "{standard:?}");
    assert_eq!(standard.stdout, "//consumer:consumer\n//producer:rule\n");
    let graph = daemon.query_with_output(expression, QueryOrder::Full, "graph", true);
    assert_eq!(graph.exit_code, 0, "{graph:?}");
    assert_eq!(
        graph.stdout,
        concat!(
            "digraph mygraph {\n",
            "  node [shape=box];\n",
            "  \"//consumer:consumer\"\n",
            "  \"//consumer:consumer\" -> \"//producer:rule\"\n",
            "  \"//producer:rule\"\n",
            "}\n",
        )
    );

    let first = daemon.query_with_output(expression, QueryOrder::Auto, "label_kind", true);
    assert_eq!(first.exit_code, 0, "{first:?}");
    assert_eq!(
        first.stdout,
        "filegroup rule //consumer:consumer\nfirst_rule rule //producer:rule\n"
    );

    write(&producer_build, "this is not valid Starlark\n");
    let standard_after_producer_break = daemon.query(expression, QueryOrder::Auto);
    assert_eq!(
        standard_after_producer_break.exit_code, 0,
        "{standard_after_producer_break:?}"
    );
    assert_eq!(
        standard_after_producer_break.stdout,
        "//consumer:consumer\n//producer:rule\n"
    );
    let broken = daemon.query_with_output(expression, QueryOrder::Auto, "label_kind", true);
    assert_eq!(broken.exit_code, 7, "{broken:?}");
    assert!(broken.stdout.is_empty(), "{broken:?}");
    assert!(
        broken.stderr.contains("\"error\":\"query_error\"")
            && broken.stderr.contains("this is not valid Starlark")
            && broken.stderr.contains("Evaluation of query"),
        "{broken:?}"
    );

    write(
        &producer_defs,
        "def _impl(ctx):\n    return [DefaultInfo()]\nsecond_rule = rule(implementation = _impl)\n",
    );
    write(
        &producer_build,
        "load(\":defs.bzl\", \"second_rule\")\nsecond_rule(name = \"rule\")\n",
    );
    let second = daemon.query_with_output(expression, QueryOrder::Auto, "label_kind", true);
    assert_eq!(second.exit_code, 0, "{second:?}");
    assert_eq!(
        second.stdout,
        "filegroup rule //consumer:consumer\nsecond_rule rule //producer:rule\n"
    );
}

#[test]
fn tagged_build_protocol_preserves_existing_fields_and_common_response() {
    let request = DaemonRequest::Build(BuildRequest {
        targets: vec!["//pkg:one".to_owned(), "//pkg:two".to_owned()],
        executor: Some("grpc://executor".to_owned()),
        default_exec_properties: vec![
            ("cpu".to_owned(), "x86_64".to_owned()),
            ("os".to_owned(), "linux".to_owned()),
        ],
        bzlmod: BzlmodRequestInputs::default(),
    });
    let json = serde_json::to_string(&request).unwrap();
    let round_trip: DaemonRequest = serde_json::from_str(&json).unwrap();
    let DaemonRequest::Build(build) = round_trip else {
        panic!("expected tagged build request");
    };
    assert_eq!(build.targets, ["//pkg:one", "//pkg:two"]);
    assert_eq!(build.executor.as_deref(), Some("grpc://executor"));
    assert_eq!(
        build.default_exec_properties,
        [
            ("cpu".to_owned(), "x86_64".to_owned()),
            ("os".to_owned(), "linux".to_owned())
        ]
    );

    let response = DaemonResponse {
        exit_code: 2,
        stdout: String::new(),
        stderr: "{\"error\":\"analysis_not_implemented\"}".to_owned(),
        invalidated_files: 3,
    };
    let response: DaemonResponse =
        serde_json::from_str(&serde_json::to_string(&response).unwrap()).unwrap();
    assert_eq!(response.exit_code, 2);
    assert!(response.stdout.is_empty());
    assert_eq!(response.stderr, "{\"error\":\"analysis_not_implemented\"}");
    assert_eq!(response.invalidated_files, 3);
}

#[test]
fn tagged_cquery_protocol_is_narrow_and_round_trips() {
    let request = DaemonRequest::Cquery(CqueryRequest {
        target: "//pkg:probe".to_owned(),
        bzlmod: BzlmodRequestInputs::default(),
    });
    let json = serde_json::to_string(&request).unwrap();
    assert!(!json.contains("order_output"));
    assert!(!json.contains("expression"));
    let round_trip: DaemonRequest = serde_json::from_str(&json).unwrap();
    let DaemonRequest::Cquery(request) = round_trip else {
        panic!("expected tagged cquery request");
    };
    assert_eq!(request.target, "//pkg:probe");
    assert_eq!(request.bzlmod, BzlmodRequestInputs::default());
}

#[test]
fn retained_cquery_missing_recovers_without_new_invalidations() {
    let workspace = scratch("cquery-recovery");
    write(&workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    write(
        &workspace.join("pkg/defs.bzl"),
        "def _impl(ctx):\n    return [DefaultInfo(files = depset([]))]\nprobe = rule(implementation = _impl)\n",
    );
    write(
        &workspace.join("pkg/BUILD.bazel"),
        "load(\":defs.bzl\", \"probe\")\nprobe(name = \"probe\")\n",
    );
    let mut daemon = Daemon::new(&workspace).unwrap();
    let run = |daemon: &mut Daemon, value: &str| {
        daemon.cquery_starlark_label_with_bzlmod_inputs(
            &target(value),
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
            Vec::new(),
        )
    };
    let success = run(&mut daemon, "//pkg:probe");
    assert_eq!(success.exit_code, 0, "{success:?}");
    assert_eq!(success.stdout, "@@//pkg:probe\n");
    let missing = run(&mut daemon, "//pkg:missing");
    assert_eq!(missing.exit_code, 1, "{missing:?}");
    assert!(missing.stdout.is_empty());
    assert_eq!(missing.invalidated_files, 0);
    assert!(missing.stderr.contains("ERROR: Skipping '//pkg:missing'"));
    let recovery = run(&mut daemon, "//pkg:probe");
    assert_eq!(recovery.exit_code, 0, "{recovery:?}");
    assert_eq!(recovery.stdout, "@@//pkg:probe\n");
    assert_eq!(recovery.invalidated_files, 0);
}

#[test]
fn bzlmod_protocol_is_primitive_canonical_and_backward_compatible() {
    let default_command = BzlmodCommandPolicyKey::from_flags(None, false).unwrap();
    let override_command =
        BzlmodCommandPolicyKey::from_flags(Some("zzz@2.0.0,yyy@1.0.0"), true).unwrap();
    let default_environment =
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap();
    let override_environment =
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(Some("all")).unwrap();
    let default_a = BzlmodRequestInputs::from_normalized(
        &default_command,
        &default_environment,
        &LockfileMode::Update,
    );
    let override_inputs = BzlmodRequestInputs::from_normalized(
        &override_command,
        &override_environment,
        &LockfileMode::Error,
    );
    let default_b = BzlmodRequestInputs::from_normalized(
        &default_command,
        &default_environment,
        &LockfileMode::Update,
    );
    assert_eq!(default_a, default_b);
    assert_eq!(
        override_inputs.command_allow_yanked_versions.as_deref(),
        Some("yyy@1.0.0,zzz@2.0.0")
    );
    assert_eq!(
        override_inputs.environment_allow_yanked_versions.as_deref(),
        Some("all")
    );
    assert!(override_inputs.ignore_dev_dependency);
    assert_eq!(override_inputs.lockfile_mode, "error");

    let old: DaemonRequest = serde_json::from_str(
        r#"{"kind":"build","request":{"targets":["//pkg:probe"],"executor":null,"default_exec_properties":[]}}"#,
    )
    .unwrap();
    let DaemonRequest::Build(old) = old else {
        panic!("expected old build request");
    };
    assert_eq!(old.bzlmod, BzlmodRequestInputs::default());
}

#[test]
fn malformed_bzlmod_protocol_input_is_request_local() {
    let workspace = scratch("bzlmod-malformed-request");
    write(&workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    write(
        &workspace.join("pkg/BUILD.bazel"),
        "filegroup(name = \"probe\")\n",
    );
    let mut daemon = Daemon::new(&workspace).unwrap();
    for (request, expected_exit, expected_error) in [
        (
            r#"{"kind":"query","request":{"expression":"//pkg:probe","order_output":"auto","bzlmod":{"command_allow_yanked_versions":"not-a-module"}}}"#,
            2,
            "module@version",
        ),
        (
            r#"{"kind":"query","request":{"expression":"//pkg:probe","order_output":"auto","bzlmod":{"environment_allow_yanked_versions":"not-a-module"}}}"#,
            2,
            "BZLMOD_ALLOW_YANKED_VERSIONS",
        ),
        (
            r#"{"kind":"query","request":{"expression":"//pkg:probe","order_output":"auto","bzlmod":{"lockfile_mode":"invalid"}}}"#,
            2,
            "Not a valid Lockfile mode",
        ),
        (
            r#"{"kind":"query","request":{"expression":"//pkg:probe","order_output":"auto","bzlmod":{"registry_urls":["file://bad"]}}}"#,
            7,
            "Unsupported non-local file URL",
        ),
    ] {
        let malformed = handle_request(&mut daemon, request);
        assert_eq!(malformed.exit_code, expected_exit);
        assert!(malformed.stderr.contains(expected_error), "{malformed:?}");

        let recovered = handle_request(
            &mut daemon,
            r#"{"kind":"query","request":{"expression":"//pkg:probe","order_output":"auto"}}"#,
        );
        assert_eq!(recovered.exit_code, 0, "{recovered:?}");
        assert_eq!(recovered.stdout, "//pkg:probe\n");
    }
}

#[test]
fn bzlmod_registry_wire_field_is_primitive_and_omitted_field_defaults() {
    let inputs: BzlmodRequestInputs = serde_json::from_str("{}").unwrap();
    assert!(inputs.registry_urls.is_empty());
    let inputs = BzlmodRequestInputs {
        registry_urls: vec![
            "https://a.example/".to_owned(),
            "file:///tmp/registry".to_owned(),
        ],
        ..BzlmodRequestInputs::default()
    };
    let json = serde_json::to_value(&inputs).unwrap();
    assert_eq!(
        json["registry_urls"],
        serde_json::json!(["https://a.example/", "file:///tmp/registry"])
    );
}

#[test]
fn retained_daemon_query_observes_build_dependency_edits() {
    let workspace = scratch("query-build-edit");
    let package = workspace.join("pkg");
    write(&workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    write(
        &package.join("BUILD.bazel"),
        "filegroup(name = \"bin\", srcs = [\"one.txt\"])\n",
    );
    write(&package.join("one.txt"), "one\n");

    let mut daemon = Daemon::new(&workspace).unwrap();
    let first = daemon.query("deps(//pkg:bin)", QueryOrder::Auto);
    assert_eq!(first.exit_code, 0, "{first:?}");
    assert_eq!(first.stdout, "//pkg:bin\n//pkg:one.txt\n");
    assert_eq!(first.invalidated_files, 0);

    write(
        &package.join("BUILD.bazel"),
        "filegroup(name = \"bin\", srcs = [\"two.txt\"])\n",
    );
    write(&package.join("two.txt"), "two\n");
    let second = daemon.query("deps(//pkg:bin)", QueryOrder::Auto);
    assert_eq!(second.exit_code, 0, "{second:?}");
    assert_eq!(second.stdout, "//pkg:bin\n//pkg:two.txt\n");
    assert_eq!(second.invalidated_files, 2);

    let third = daemon.query("deps(//pkg:bin)", QueryOrder::Auto);
    assert_eq!(third.exit_code, 0, "{third:?}");
    assert_eq!(third.invalidated_files, 0);
}

#[test]
fn retained_daemon_query_publishes_cold_events_without_warm_replay() {
    let workspace = scratch("query-selected-events");
    let package = workspace.join("pkg");
    write(
        &workspace.join("MODULE.bazel"),
        "print(\"MODULE_EVENT\")\nmodule(name = \"demo\")\n",
    );
    write(
        &package.join("defs.bzl"),
        "print(\"BZL_EVENT\")\nNAME = \"probe\"\n",
    );
    write(
        &package.join("BUILD.bazel"),
        "load(\":defs.bzl\", \"NAME\")\nprint(\"BUILD_EVENT\")\nfilegroup(name = NAME)\n",
    );
    let workspace = workspace.canonicalize().unwrap();
    let expected = format!(
        "DEBUG: {}:1:6: MODULE_EVENT\nDEBUG: {}:1:6: BZL_EVENT\nDEBUG: {}:2:6: BUILD_EVENT\n",
        workspace.join("MODULE.bazel").display(),
        workspace.join("pkg/defs.bzl").display(),
        workspace.join("pkg/BUILD.bazel").display(),
    );
    let mut daemon = Daemon::new(&workspace).unwrap();
    for index in 0..2 {
        let result = daemon.query("deps(//pkg:probe)", QueryOrder::Auto);
        assert_eq!(result.exit_code, 0, "{result:?}");
        assert_eq!(result.stdout, "//pkg:probe\n");
        if index == 0 {
            assert_eq!(result.stderr, expected);
        } else {
            assert!(result.stderr.is_empty(), "{result:?}");
        }
        assert_eq!(result.invalidated_files, 0);
    }

    write(
        &package.join("BUILD.bazel"),
        "load(\":defs.bzl\", \"NAME\")\nprint(\"BUILD_CHANGED\")\nfilegroup(name = NAME)\n",
    );
    let changed = daemon.query("deps(//pkg:probe)", QueryOrder::Auto);
    assert_eq!(changed.exit_code, 0, "{changed:?}");
    assert_eq!(changed.stdout, "//pkg:probe\n");
    assert_eq!(
        changed.stderr,
        format!(
            "DEBUG: {}:2:6: BUILD_CHANGED\n",
            workspace.join("pkg/BUILD.bazel").display()
        )
    );
    assert_eq!(changed.invalidated_files, 1);

    let warm_after_change = daemon.query("deps(//pkg:probe)", QueryOrder::Auto);
    assert_eq!(warm_after_change.exit_code, 0, "{warm_after_change:?}");
    assert!(warm_after_change.stderr.is_empty(), "{warm_after_change:?}");
    assert_eq!(warm_after_change.invalidated_files, 0);
}

#[test]
fn retained_daemon_direct_external_query_replays_only_changed_external_build() {
    let workspace = scratch("external-query-selected-events");
    write(
        &workspace.join("MODULE.bazel"),
        "print(\"MODULE_EVENT\")\nmodule(name = \"demo\")\nbazel_dep(name = \"dep\", version = \"1.0.0\")\nlocal_path_override(module_name = \"dep\", path = \"dep\")\n",
    );
    write(
        &workspace.join("dep/MODULE.bazel"),
        "module(name = \"dep\", version = \"1.0.0\")\n",
    );
    write(
        &workspace.join("dep/BUILD.bazel"),
        "print(\"EXTERNAL_BUILD_EVENT\")\nexports_files([\"target.txt\"])\n",
    );
    write(&workspace.join("dep/target.txt"), "target\n");
    let mut daemon = Daemon::new(&workspace).unwrap();

    let first = daemon.query("@dep//:target.txt", QueryOrder::Auto);
    assert_eq!(first.exit_code, 0, "{first:?}");
    assert_eq!(first.stdout, "@dep//:target.txt\n");
    let module_index = first.stderr.find("MODULE_EVENT").unwrap();
    let build_index = first.stderr.find("EXTERNAL_BUILD_EVENT").unwrap();
    assert!(module_index < build_index, "{first:?}");
    assert_eq!(first.invalidated_files, 0);

    let warm = daemon.query("@dep//:target.txt", QueryOrder::Auto);
    assert_eq!(warm.exit_code, 0, "{warm:?}");
    assert_eq!(warm.stdout, "@dep//:target.txt\n");
    assert!(warm.stderr.is_empty(), "{warm:?}");
    assert_eq!(warm.invalidated_files, 0);

    write(
        &workspace.join("dep/BUILD.bazel"),
        "print(\"EXTERNAL_BUILD_EDITED\")\nexports_files([\"edited.txt\"])\n",
    );
    write(&workspace.join("dep/edited.txt"), "edited\n");
    let edited = daemon.query("@dep//:edited.txt", QueryOrder::Auto);
    assert_eq!(edited.exit_code, 0, "{edited:?}");
    assert_eq!(edited.stdout, "@dep//:edited.txt\n");
    assert!(
        edited.stderr.contains("EXTERNAL_BUILD_EDITED"),
        "{edited:?}"
    );
    assert!(!edited.stderr.contains("MODULE_EVENT"), "{edited:?}");
    assert_eq!(edited.invalidated_files, 2);
}

#[test]
fn retained_daemon_build_observes_direct_external_exported_sources() {
    let workspace = scratch("external-build-source");
    write(
        &workspace.join("MODULE.bazel"),
        "print(\"ROOT_EVENT\")\nmodule(name = \"demo\")\nbazel_dep(name = \"dep\", version = \"1.0.0\")\nlocal_path_override(module_name = \"dep\", path = \"dep\")\n",
    );
    write(
        &workspace.join("BUILD.bazel"),
        "exports_files([\"root.txt\"])\n",
    );
    write(&workspace.join("root.txt"), "root\n");
    write(
        &workspace.join("dep/MODULE.bazel"),
        "module(name = \"dep\", version = \"1.0.0\")\n",
    );
    write(
        &workspace.join("dep/BUILD.bazel"),
        "print(\"DEP_BUILD_EVENT\")\nexports_files([\"target.txt\"])\nfilegroup(name = \"files\")\n",
    );
    write(
        &workspace.join("dep/rulepkg/defs.bzl"),
        "def _impl(ctx):\n    fail(\"ANALYSIS_MUST_NOT_RUN\")\nprobe = rule(implementation = _impl)\n",
    );
    write(
        &workspace.join("dep/rulepkg/BUILD.bazel"),
        "load(\":defs.bzl\", \"probe\")\nprobe(name = \"rule\", visibility = [\"//visibility:public\"])\n",
    );
    let source = workspace.join("dep/target.txt");
    write(&source, "one\n");
    let mut daemon = Daemon::new(&workspace).unwrap();
    let terminal = |invalidated_files| {
        format!(
            "{{\"success\":true,\"command\":\"build\",\"target_count\":1,\"loaded_package_count\":1,\"analyzed_target_count\":0,\"declared_action_count\":0,\"runtime_mode\":\"daemon\",\"invalidated_files\":{invalidated_files},\"completed_boundary\":\"dice_exported_source_file\"}}\n"
        )
    };

    let cold = daemon.build(&[target("@dep//:target.txt")], &remote_disabled(), &[]);
    assert_eq!(cold.exit_code, 0, "{cold:?}");
    assert!(cold.stdout.is_empty(), "{cold:?}");
    assert_eq!(cold.stderr.matches("ROOT_EVENT").count(), 1, "{cold:?}");
    assert_eq!(
        cold.stderr.matches("DEP_BUILD_EVENT").count(),
        1,
        "{cold:?}"
    );
    let root_event = cold.stderr.find("ROOT_EVENT").unwrap();
    let build_event = cold.stderr.find("DEP_BUILD_EVENT").unwrap();
    let terminal_start = cold.stderr.find("{\"success\":true").unwrap();
    assert!(
        root_event < build_event && build_event < terminal_start,
        "{cold:?}"
    );
    assert!(cold.stderr.ends_with(&terminal(0)), "{cold:?}");
    assert!(!cold.stderr.contains("reapi"), "{cold:?}");

    let warm = daemon.build(&[target("@dep//:target.txt")], &remote_disabled(), &[]);
    assert_eq!(warm.exit_code, 0, "{warm:?}");
    assert!(warm.stdout.is_empty(), "{warm:?}");
    assert_eq!(warm.stderr, terminal(0));

    write(&source, "two\n");
    let edited = daemon.build(&[target("@dep//:target.txt")], &remote_disabled(), &[]);
    assert_eq!(edited.exit_code, 0, "{edited:?}");
    assert!(edited.stdout.is_empty(), "{edited:?}");
    assert_eq!(edited.stderr, terminal(1));

    fs::remove_file(&source).unwrap();
    let absent = daemon.build(&[target("@dep//:target.txt")], &remote_disabled(), &[]);
    assert_eq!(absent.exit_code, 2, "{absent:?}");
    assert!(absent.stdout.is_empty(), "{absent:?}");
    assert_eq!(
        absent.stderr,
        "{\"error\":\"build_runtime_error\",\"command\":\"build\",\"message\":\"@@dep+//:target.txt: missing input file '@@dep+//:target.txt'\",\"runtime_mode\":\"daemon\",\"invalidated_files\":1}\n"
    );

    fs::create_dir(&source).unwrap();
    let directory = daemon.build(&[target("@dep//:target.txt")], &remote_disabled(), &[]);
    assert_eq!(directory.exit_code, 0, "{directory:?}");
    assert!(directory.stdout.is_empty(), "{directory:?}");
    assert_eq!(directory.stderr, terminal(0));

    fs::remove_dir(&source).unwrap();
    write(&source, "three\n");
    let recreated = daemon.build(&[target("@dep//:target.txt")], &remote_disabled(), &[]);
    assert_eq!(recreated.exit_code, 0, "{recreated:?}");
    assert!(recreated.stdout.is_empty(), "{recreated:?}");
    assert_eq!(recreated.stderr, terminal(1));

    let root = daemon.build(&[target("//:root.txt")], &remote_disabled(), &[]);
    assert_eq!(root.exit_code, 0, "{root:?}");
    assert!(root.stdout.is_empty(), "{root:?}");
    assert_eq!(root.stderr, terminal(0));

    let unchanged = daemon.build(&[target("@dep//:files")], &remote_disabled(), &[]);
    assert_eq!(unchanged.exit_code, 2, "{unchanged:?}");
    assert!(unchanged.stderr.contains("not an exported source file"));

    let rule = daemon.build(&[target("@dep//rulepkg:rule")], &remote_disabled(), &[]);
    assert_eq!(rule.exit_code, 2, "{rule:?}");
    assert!(rule.stderr.contains("not an exported source file"));
    assert!(!rule.stderr.contains("ANALYSIS_MUST_NOT_RUN"), "{rule:?}");

    let mut remote_execute = remote_disabled();
    remote_execute.executor = Some("grpc://must-not-run".to_owned());
    let remote_source = daemon.build(&[target("@dep//:target.txt")], &remote_execute, &[]);
    assert_eq!(remote_source.exit_code, 0, "{remote_source:?}");
    assert_eq!(remote_source.stderr, terminal(0));
    assert!(!remote_source.stderr.contains("reapi"), "{remote_source:?}");
}

#[test]
fn retained_daemon_external_module_cycle_recovers_without_stale_events() {
    let workspace = scratch("external-module-cycle");
    write(
        &workspace.join("MODULE.bazel"),
        "print(\"ROOT_EVENT\")\nmodule(name = \"demo\")\nbazel_dep(name = \"dep\", version = \"1.0.0\")\nlocal_path_override(module_name = \"dep\", path = \"dep\")\n",
    );
    write(
        &workspace.join("dep/MODULE.bazel"),
        "print(\"DEP_EVENT\")\nmodule(name = \"dep\", version = \"1.0.0\")\ninclude(\"//cycle:a.MODULE.bazel\")\n",
    );
    write(
        &workspace.join("dep/cycle/a.MODULE.bazel"),
        "print(\"A_EVENT\")\ninclude(\"//cycle:b.MODULE.bazel\")\n",
    );
    let repeated = "print(\"B_EVENT\")\ninclude(\"//cycle:a.MODULE.bazel\")\n";
    write(&workspace.join("dep/cycle/b.MODULE.bazel"), repeated);
    write(&workspace.join("dep/cycle/BUILD.bazel"), "");
    write(
        &workspace.join("dep/BUILD.bazel"),
        "print(\"BUILD_EVENT\")\nexports_files([\"target.txt\"])\n",
    );
    write(&workspace.join("dep/target.txt"), "target\n");
    let mut daemon = Daemon::new(&workspace).unwrap();
    let message = format!(
        "Slug does not support MODULE.bazel include cycles in direct local_path_override repository '@dep' for module 'dep': include \"//cycle:a.MODULE.bazel\" at {}:2:1 repeats ancestor include \"//cycle:a.MODULE.bazel\" at {}:3:1",
        workspace.join("dep/cycle/b.MODULE.bazel").display(),
        workspace.join("dep/MODULE.bazel").display(),
    );
    let terminal = |invalidated_files: usize| {
        format!(
            "{{\"error\":\"unsupported_feature\",\"command\":\"query\",\"message\":\"{}\",\"runtime_mode\":\"daemon\",\"invalidated_files\":{invalidated_files}}}\n",
            slug_core_v2::error::json_escape(&message),
        )
    };

    let cold = daemon.query("@dep//:target.txt", QueryOrder::Auto);
    assert_eq!(cold.exit_code, 7, "{cold:?}");
    assert!(cold.stdout.is_empty(), "{cold:?}");
    assert!(
        cold.stderr.contains("DEBUG:") && cold.stderr.contains("ROOT_EVENT"),
        "{cold:?}"
    );
    assert_eq!(cold.stderr.matches("ROOT_EVENT").count(), 1, "{cold:?}");
    assert!(cold.stderr.contains("\"error\":\"unsupported_feature\""));
    assert!(cold.stderr.contains("/dep/cycle/b.MODULE.bazel:2:1"));
    assert!(cold.stderr.contains("/dep/MODULE.bazel:3:1"));
    assert!(cold.stderr.ends_with(&terminal(0)), "{cold:?}");
    assert!(!cold.stderr.contains("DEP_EVENT"), "{cold:?}");
    assert!(!cold.stderr.contains("A_EVENT"), "{cold:?}");
    assert!(!cold.stderr.contains("B_EVENT"), "{cold:?}");
    assert!(!cold.stderr.contains("BUILD_EVENT"), "{cold:?}");
    assert!(!cold.stderr.contains("Evaluation of query"), "{cold:?}");
    assert_eq!(cold.invalidated_files, 0);

    let build = daemon.build(&[target("@dep//:target.txt")], &remote_disabled(), &[]);
    assert_eq!(build.exit_code, 7, "{build:?}");
    assert!(build.stdout.is_empty(), "{build:?}");
    assert_eq!(
        build.stderr,
        format!(
            "{{\"error\":\"unsupported_feature\",\"command\":\"build\",\"message\":\"{}\",\"runtime_mode\":\"daemon\",\"invalidated_files\":0}}\n",
            slug_core_v2::error::json_escape(&message),
        )
    );

    let warm = daemon.query("buildfiles(@dep//:target.txt)", QueryOrder::Auto);
    assert_eq!(warm.exit_code, 7, "{warm:?}");
    assert!(warm.stdout.is_empty(), "{warm:?}");
    assert!(
        warm.stderr
            .starts_with("{\"error\":\"unsupported_feature\"")
    );
    assert_eq!(warm.stderr, terminal(0));
    assert!(!warm.stderr.contains("DEBUG:"), "{warm:?}");
    assert!(!warm.stderr.contains("Evaluation of query"), "{warm:?}");
    assert_eq!(warm.invalidated_files, 0);

    write(
        &workspace.join("dep/cycle/b.MODULE.bazel"),
        "print(\"B_EVENT\")\n",
    );
    let recovered = daemon.query("@dep//:target.txt", QueryOrder::Auto);
    assert_eq!(recovered.exit_code, 0, "{recovered:?}");
    assert_eq!(recovered.stdout, "@dep//:target.txt\n");
    for event in ["DEP_EVENT", "A_EVENT", "B_EVENT", "BUILD_EVENT"] {
        assert_eq!(recovered.stderr.matches(event).count(), 1, "{recovered:?}");
    }
    assert_eq!(recovered.invalidated_files, 1);

    let supported_warm = daemon.query("@dep//:target.txt", QueryOrder::Auto);
    assert_eq!(supported_warm.exit_code, 0, "{supported_warm:?}");
    assert!(supported_warm.stderr.is_empty(), "{supported_warm:?}");
    assert_eq!(supported_warm.invalidated_files, 0);

    write(&workspace.join("dep/cycle/b.MODULE.bazel"), repeated);
    let reintroduced = daemon.query("loadfiles(@dep//:target.txt)", QueryOrder::Auto);
    assert_eq!(reintroduced.exit_code, 7, "{reintroduced:?}");
    assert!(reintroduced.stdout.is_empty(), "{reintroduced:?}");
    assert!(
        reintroduced
            .stderr
            .contains("\"error\":\"unsupported_feature\"")
    );
    assert_eq!(reintroduced.stderr, terminal(1));
    assert!(!reintroduced.stderr.contains("DEBUG:"), "{reintroduced:?}");
    assert!(
        !reintroduced.stderr.contains("Evaluation of query"),
        "{reintroduced:?}"
    );
    assert_eq!(reintroduced.invalidated_files, 1);
}

#[test]
fn daemon_query_preflight_error_has_one_terminal_newline() {
    let workspace = scratch("query-preflight-error");
    write(&workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    let mut daemon = Daemon::new(&workspace).unwrap();
    let result = daemon.query_with_output("set()", QueryOrder::Auto, "unsupported", true);
    assert_eq!(result.exit_code, 2, "{result:?}");
    assert!(result.stdout.is_empty());
    assert_eq!(
        result.stderr,
        "{\"error\":\"query_runtime_error\",\"command\":\"query\",\"message\":\"output format 'unsupported' is not supported by loading query\",\"runtime_mode\":\"daemon\",\"invalidated_files\":0}\n"
    );
    assert_eq!(result.stderr.lines().count(), 1);
    assert_eq!(result.invalidated_files, 0);
}

#[test]
fn retained_daemon_some_observes_candidate_create_rename_delete_recreate() {
    let workspace = scratch("some-candidate-transitions");
    let build = workspace.join("cand/BUILD.bazel");
    write(&workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    write(&build, "filegroup(name = \"one\")\n");
    let expression = "some(//cand:all, 10)";
    let mut daemon = Daemon::new(&workspace).unwrap();

    let initial = daemon.query(expression, QueryOrder::Auto);
    assert_eq!(initial.exit_code, 0, "{initial:?}");
    assert_eq!(initial.stdout, "//cand:one\n");

    write(
        &build,
        "filegroup(name = \"one\")\nfilegroup(name = \"two\")\n",
    );
    let created = daemon.query(expression, QueryOrder::Auto);
    assert_eq!(created.exit_code, 0, "{created:?}");
    assert_eq!(created.stdout, "//cand:one\n//cand:two\n");

    write(
        &build,
        "filegroup(name = \"one\")\nfilegroup(name = \"middle\")\n",
    );
    let renamed = daemon.query(expression, QueryOrder::Auto);
    assert_eq!(renamed.exit_code, 0, "{renamed:?}");
    assert_eq!(renamed.stdout, "//cand:middle\n//cand:one\n");

    write(&build, "filegroup(name = \"one\")\n");
    let deleted = daemon.query(expression, QueryOrder::Auto);
    assert_eq!(deleted.exit_code, 0, "{deleted:?}");
    assert_eq!(deleted.stdout, "//cand:one\n");

    write(
        &build,
        "filegroup(name = \"one\")\nfilegroup(name = \"zeta\")\n",
    );
    let recreated = daemon.query(expression, QueryOrder::Auto);
    assert_eq!(recreated.exit_code, 0, "{recreated:?}");
    assert_eq!(recreated.stdout, "//cand:one\n//cand:zeta\n");
}

#[test]
fn retained_daemon_reverse_query_observes_edge_and_subtree_transitions() {
    let workspace = scratch("reverse-query-transitions");
    write(&workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    write(
        &workspace.join("app/BUILD.bazel"),
        "filegroup(name = \"top\", srcs = [\"//leaf:item\"])\n",
    );
    write(
        &workspace.join("leaf/BUILD.bazel"),
        "filegroup(name = \"item\", srcs = [])\n",
    );

    let mut daemon = Daemon::new(&workspace).unwrap();
    let first = daemon.query("rdeps(//app:top, //leaf:item)", QueryOrder::Auto);
    assert_eq!(first.exit_code, 0, "{first:?}");
    assert_eq!(first.stdout, "//app:top\n//leaf:item\n");

    write(
        &workspace.join("app/BUILD.bazel"),
        "filegroup(name = \"top\", srcs = [])\n",
    );
    let lost = daemon.query("rdeps(//app:top, //leaf:item)", QueryOrder::Auto);
    assert_eq!(lost.exit_code, 0, "{lost:?}");
    assert!(lost.stdout.is_empty(), "{lost:?}");

    write(
        &workspace.join("tree/base/BUILD.bazel"),
        "filegroup(name = \"base\", srcs = [])\n",
    );
    let subtree = daemon.query("//tree/...", QueryOrder::Auto);
    assert_eq!(subtree.exit_code, 0, "{subtree:?}");
    assert_eq!(subtree.stdout, "//tree/base:base\n");

    write(
        &workspace.join("tree/dynamic/BUILD.bazel"),
        "filegroup(name = \"dynamic\", srcs = [])\n",
    );
    let created = daemon.query("//tree/...", QueryOrder::Auto);
    assert_eq!(created.exit_code, 0, "{created:?}");
    assert_eq!(created.stdout, "//tree/base:base\n//tree/dynamic:dynamic\n");
}

#[test]
fn retained_daemon_siblings_observes_build_file_and_priority_transitions() {
    let workspace = scratch("siblings-build-file-transitions");
    let package = workspace.join("pkg");
    let modern = package.join("BUILD.bazel");
    let fallback = package.join("BUILD");
    write(&workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    write(&modern, "filegroup(name = \"one\")\n");

    let mut daemon = Daemon::new(&workspace).unwrap();
    let initial = daemon.query("siblings(//pkg:BUILD.bazel)", QueryOrder::Auto);
    assert_eq!(initial.exit_code, 0, "{initial:?}");
    assert_eq!(initial.stdout, "//pkg:BUILD.bazel\n//pkg:one\n");
    assert_eq!(initial.invalidated_files, 0);

    write(
        &modern,
        "filegroup(name = \"one\")\nfilegroup(name = \"two\")\n",
    );
    let edited = daemon.query("siblings(//pkg:BUILD.bazel)", QueryOrder::Auto);
    assert_eq!(edited.exit_code, 0, "{edited:?}");
    assert_eq!(edited.stdout, "//pkg:BUILD.bazel\n//pkg:one\n//pkg:two\n");
    assert_eq!(edited.invalidated_files, 1);

    fs::rename(&modern, &fallback).unwrap();
    let fallback_only = daemon.query("siblings(//pkg:BUILD)", QueryOrder::Auto);
    assert_eq!(fallback_only.exit_code, 0, "{fallback_only:?}");
    assert_eq!(fallback_only.stdout, "//pkg:BUILD\n//pkg:one\n//pkg:two\n");
    assert_eq!(fallback_only.invalidated_files, 2);

    write(&modern, "filegroup(name = \"preferred\")\n");
    let dual = daemon.query("siblings(//pkg:BUILD.bazel)", QueryOrder::Auto);
    assert_eq!(dual.exit_code, 0, "{dual:?}");
    assert_eq!(dual.stdout, "//pkg:BUILD.bazel\n//pkg:preferred\n");
    assert_eq!(dual.invalidated_files, 1);

    write(&fallback, "filegroup(name = \"ignored\")\n");
    let ignored_edit = daemon.query("siblings(//pkg:BUILD.bazel)", QueryOrder::Auto);
    assert_eq!(ignored_edit.exit_code, 0, "{ignored_edit:?}");
    assert_eq!(ignored_edit.stdout, "//pkg:BUILD.bazel\n//pkg:preferred\n");
    assert_eq!(ignored_edit.invalidated_files, 1);

    fs::remove_file(&modern).unwrap();
    fs::remove_file(&fallback).unwrap();
    let missing = daemon.query("siblings(//pkg:BUILD.bazel)", QueryOrder::Auto);
    assert_eq!(missing.exit_code, 7, "{missing:?}");
    assert!(missing.stdout.is_empty());
    assert_eq!(missing.invalidated_files, 2);

    write(&fallback, "filegroup(name = \"recreated\")\n");
    let recreated = daemon.query("siblings(//pkg:BUILD)", QueryOrder::Auto);
    assert_eq!(recreated.exit_code, 0, "{recreated:?}");
    assert_eq!(recreated.stdout, "//pkg:BUILD\n//pkg:recreated\n");
    assert_eq!(recreated.invalidated_files, 1);
}

#[test]
fn retained_daemon_path_query_observes_edge_and_reachable_package_transitions() {
    let workspace = scratch("path-query-transitions");
    write(&workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    write(
        &workspace.join("origin/BUILD.bazel"),
        "filegroup(name = \"top\", srcs = [\"//mid:item\"])\n",
    );
    write(
        &workspace.join("mid/BUILD.bazel"),
        "filegroup(name = \"item\", srcs = [\"//dest:end\"])\n",
    );
    write(
        &workspace.join("dest/BUILD.bazel"),
        "filegroup(name = \"end\", srcs = [])\n",
    );
    let expression = "somepath(//origin:top, //dest:end)";

    let mut daemon = Daemon::new(&workspace).unwrap();
    let initial = daemon.query(expression, QueryOrder::Auto);
    assert_eq!(initial.exit_code, 0, "{initial:?}");
    assert_eq!(initial.stdout, "//origin:top\n//mid:item\n//dest:end\n");

    write(
        &workspace.join("mid/BUILD.bazel"),
        "filegroup(name = \"item\", srcs = [])\n",
    );
    let lost = daemon.query(expression, QueryOrder::Auto);
    assert_eq!(lost.exit_code, 0, "{lost:?}");
    assert!(lost.stdout.is_empty(), "{lost:?}");

    write(
        &workspace.join("mid/BUILD.bazel"),
        "filegroup(name = \"item\", srcs = [\"//dest:end\"])\n",
    );
    let restored = daemon.query(expression, QueryOrder::Auto);
    assert_eq!(restored.exit_code, 0, "{restored:?}");
    assert_eq!(restored.stdout, "//origin:top\n//mid:item\n//dest:end\n");

    write(
        &workspace.join("branch/BUILD.bazel"),
        "filegroup(name = \"item\", srcs = [\"//dest:end\"])\n",
    );
    write(
        &workspace.join("origin/BUILD.bazel"),
        "filegroup(name = \"top\", srcs = [\"//branch:item\"])\n",
    );
    let gained = daemon.query(expression, QueryOrder::Auto);
    assert_eq!(gained.exit_code, 0, "{gained:?}");
    assert_eq!(gained.stdout, "//origin:top\n//branch:item\n//dest:end\n");

    write(
        &workspace.join("origin/BUILD.bazel"),
        "filegroup(name = \"top\", srcs = [\"//mid:item\"])\n",
    );
    fs::remove_file(workspace.join("branch/BUILD.bazel")).unwrap();
    let removed = daemon.query(expression, QueryOrder::Auto);
    assert_eq!(removed.exit_code, 0, "{removed:?}");
    assert_eq!(removed.stdout, "//origin:top\n//mid:item\n//dest:end\n");

    write(
        &workspace.join("branch/BUILD.bazel"),
        "filegroup(name = \"item\", srcs = [\"//dest:end\"])\n",
    );
    write(
        &workspace.join("origin/BUILD.bazel"),
        "filegroup(name = \"top\", srcs = [\"//branch:item\"])\n",
    );
    let recreated = daemon.query(expression, QueryOrder::Auto);
    assert_eq!(recreated.exit_code, 0, "{recreated:?}");
    assert_eq!(
        recreated.stdout,
        "//origin:top\n//branch:item\n//dest:end\n"
    );
}

#[test]
fn retained_daemon_loadfiles_observes_leaf_and_load_edge_transitions() {
    let workspace = scratch("loadfiles-transitions");
    let app_build = workspace.join("app/BUILD.bazel");
    let root_bzl = workspace.join("root/root.bzl");
    write(&workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    write(&workspace.join("root/BUILD.bazel"), "");
    write(&workspace.join("leaf/BUILD.bazel"), "");
    write(&workspace.join("alternate/BUILD.bazel"), "");
    write(
        &app_build,
        "load(\"//root:root.bzl\", \"ROOT\")\nfilegroup(name = \"app\")\n",
    );
    write(
        &root_bzl,
        "load(\"//leaf:one.bzl\", \"VALUE\")\nROOT = VALUE\n",
    );
    write(&workspace.join("leaf/one.bzl"), "VALUE = 1\n");
    write(&workspace.join("leaf/two.bzl"), "VALUE = 2\n");
    write(
        &workspace.join("alternate/alternate.bzl"),
        "load(\"//leaf:two.bzl\", \"VALUE\")\nROOT = VALUE\n",
    );

    let mut daemon = Daemon::new(&workspace).unwrap();
    let expression = "loadfiles(//app:app)";

    let initial = daemon.query(expression, QueryOrder::Auto);
    assert_eq!(initial.exit_code, 0, "{initial:?}");
    assert_eq!(initial.stdout, "//leaf:one.bzl\n//root:root.bzl\n");
    assert_eq!(initial.invalidated_files, 0);

    write(&workspace.join("leaf/one.bzl"), "VALUE = 11\n");
    let leaf_edit = daemon.query(expression, QueryOrder::Auto);
    assert_eq!(leaf_edit.exit_code, 0, "{leaf_edit:?}");
    assert_eq!(leaf_edit.stdout, initial.stdout);
    assert_eq!(leaf_edit.invalidated_files, 1);

    write(
        &root_bzl,
        "load(\"//leaf:two.bzl\", \"VALUE\")\nROOT = VALUE\n",
    );
    let transitive_switch = daemon.query(expression, QueryOrder::Auto);
    assert_eq!(transitive_switch.exit_code, 0, "{transitive_switch:?}");
    assert_eq!(
        transitive_switch.stdout,
        "//leaf:two.bzl\n//root:root.bzl\n"
    );
    assert_eq!(transitive_switch.invalidated_files, 1);

    write(&root_bzl, "ROOT = 0\n");
    let transitive_deleted = daemon.query(expression, QueryOrder::Auto);
    assert_eq!(transitive_deleted.exit_code, 0, "{transitive_deleted:?}");
    assert_eq!(transitive_deleted.stdout, "//root:root.bzl\n");
    assert_eq!(transitive_deleted.invalidated_files, 1);

    write(
        &root_bzl,
        "load(\"//leaf:one.bzl\", \"VALUE\")\nROOT = VALUE\n",
    );
    let transitive_recreated = daemon.query(expression, QueryOrder::Auto);
    assert_eq!(
        transitive_recreated.exit_code, 0,
        "{transitive_recreated:?}"
    );
    assert_eq!(
        transitive_recreated.stdout,
        "//leaf:one.bzl\n//root:root.bzl\n"
    );
    assert_eq!(transitive_recreated.invalidated_files, 1);

    write(
        &app_build,
        "load(\"//alternate:alternate.bzl\", \"ROOT\")\nfilegroup(name = \"app\")\n",
    );
    let direct_switch = daemon.query(expression, QueryOrder::Auto);
    assert_eq!(direct_switch.exit_code, 0, "{direct_switch:?}");
    assert_eq!(
        direct_switch.stdout,
        "//alternate:alternate.bzl\n//leaf:two.bzl\n"
    );
    assert_eq!(direct_switch.invalidated_files, 1);

    write(&app_build, "filegroup(name = \"app\")\n");
    let direct_deleted = daemon.query(expression, QueryOrder::Auto);
    assert_eq!(direct_deleted.exit_code, 0, "{direct_deleted:?}");
    assert!(direct_deleted.stdout.is_empty(), "{direct_deleted:?}");
    assert_eq!(direct_deleted.invalidated_files, 1);

    write(
        &app_build,
        "load(\"//root:root.bzl\", \"ROOT\")\nfilegroup(name = \"app\")\n",
    );
    let direct_recreated = daemon.query(expression, QueryOrder::Auto);
    assert_eq!(direct_recreated.exit_code, 0, "{direct_recreated:?}");
    assert_eq!(direct_recreated.stdout, "//leaf:one.bzl\n//root:root.bzl\n");
    assert_eq!(direct_recreated.invalidated_files, 1);
}

#[test]
fn retained_daemon_buildfiles_tracks_loaded_companion_priority_only() {
    let workspace = scratch("buildfiles-companion-transitions");
    let loaded = workspace.join("loaded");
    let primary = loaded.join("BUILD.bazel");
    let fallback = loaded.join("BUILD");
    write(&workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    write(
        &workspace.join("app/BUILD.bazel"),
        "load(\"//loaded:defs.bzl\", \"DEFS\")\nfilegroup(name = \"app\")\n",
    );
    write(&loaded.join("defs.bzl"), "DEFS = 1\n");
    write(&primary, "this is deliberately not valid(\n");

    let mut daemon = Daemon::new(&workspace).unwrap();
    let initial_buildfiles = daemon.query("buildfiles(//app:app)", QueryOrder::Auto);
    assert_eq!(initial_buildfiles.exit_code, 0, "{initial_buildfiles:?}");
    assert_eq!(
        initial_buildfiles.stdout,
        "//app:BUILD.bazel\n//loaded:BUILD.bazel\n//loaded:defs.bzl\n"
    );
    assert_eq!(initial_buildfiles.invalidated_files, 0);

    let initial_loadfiles = daemon.query("loadfiles(//app:app)", QueryOrder::Auto);
    assert_eq!(initial_loadfiles.exit_code, 0, "{initial_loadfiles:?}");
    assert_eq!(initial_loadfiles.stdout, "//loaded:defs.bzl\n");
    assert_eq!(initial_loadfiles.invalidated_files, 0);

    fs::rename(&primary, &fallback).unwrap();
    let fallback_buildfiles = daemon.query("buildfiles(//app:app)", QueryOrder::Auto);
    assert_eq!(fallback_buildfiles.exit_code, 0, "{fallback_buildfiles:?}");
    assert_eq!(
        fallback_buildfiles.stdout,
        "//app:BUILD.bazel\n//loaded:BUILD\n//loaded:defs.bzl\n"
    );
    assert_eq!(fallback_buildfiles.invalidated_files, 2);

    let fallback_loadfiles = daemon.query("loadfiles(//app:app)", QueryOrder::Auto);
    assert_eq!(fallback_loadfiles.exit_code, 0, "{fallback_loadfiles:?}");
    assert_eq!(fallback_loadfiles.stdout, initial_loadfiles.stdout);
    assert_eq!(fallback_loadfiles.invalidated_files, 0);

    write(&primary, "this primary is also deliberately not valid(\n");
    let preferred_buildfiles = daemon.query("buildfiles(//app:app)", QueryOrder::Auto);
    assert_eq!(
        preferred_buildfiles.exit_code, 0,
        "{preferred_buildfiles:?}"
    );
    assert_eq!(
        preferred_buildfiles.stdout,
        "//app:BUILD.bazel\n//loaded:BUILD.bazel\n//loaded:defs.bzl\n"
    );
    assert_eq!(preferred_buildfiles.invalidated_files, 1);

    let preferred_loadfiles = daemon.query("loadfiles(//app:app)", QueryOrder::Auto);
    assert_eq!(preferred_loadfiles.exit_code, 0, "{preferred_loadfiles:?}");
    assert_eq!(preferred_loadfiles.stdout, initial_loadfiles.stdout);
    assert_eq!(preferred_loadfiles.invalidated_files, 0);

    fs::remove_file(&primary).unwrap();
    let restored_fallback = daemon.query("buildfiles(//app:app)", QueryOrder::Auto);
    assert_eq!(restored_fallback.exit_code, 0, "{restored_fallback:?}");
    assert_eq!(
        restored_fallback.stdout,
        "//app:BUILD.bazel\n//loaded:BUILD\n//loaded:defs.bzl\n"
    );
    assert_eq!(restored_fallback.invalidated_files, 1);
}
