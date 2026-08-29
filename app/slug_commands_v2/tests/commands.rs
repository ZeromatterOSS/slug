/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_bzlmod_v2::LockfileMode;
use slug_commands_v2::CommandConfigurationOccurrence;
use slug_commands_v2::FlagDisposition;
use slug_commands_v2::HELP_SUMMARY;
use slug_commands_v2::QueryOutputFormat;
use slug_commands_v2::RepositoryEnvironmentOverride;
use slug_commands_v2::aquery::AqueryRequest;
use slug_commands_v2::build::BuildRequest;
use slug_commands_v2::cquery::CqueryOutputMode;
use slug_commands_v2::cquery::CqueryRequest;
use slug_commands_v2::normalize_bzlmod_environment_value;
use slug_commands_v2::query::QueryRequest;
use slug_commands_v2::run::RunRequest;
use slug_commands_v2::test::TestRequest;
use slug_query_v2::QueryPolicy;

#[test]
fn build_request_parses_target_patterns_and_classifies_flags() {
    let request = BuildRequest::parse(&[
        "--remote_executor=grpc://127.0.0.1:50051",
        "--keep_going",
        "--allow_yanked_versions=zzz@2.0.0,yyy@1.0.0",
        "--ignore_dev_dependency",
        "--lockfile_mode=refresh",
        "//pkg:bin",
        "//pkg:all",
    ])
    .unwrap();

    assert_eq!(request.targets.len(), 2);
    assert_eq!(
        request
            .flags
            .iter()
            .find(|flag| flag.name == "remote_executor")
            .unwrap()
            .disposition,
        FlagDisposition::Planned
    );
    assert_eq!(
        request
            .flags
            .iter()
            .find(|flag| flag.name == "keep_going")
            .unwrap()
            .disposition,
        FlagDisposition::IgnoredCompatible
    );
    assert_eq!(
        request
            .flags
            .iter()
            .find(|flag| flag.name == "allow_yanked_versions")
            .unwrap()
            .disposition,
        FlagDisposition::ParseOnly
    );
    assert_eq!(
        request
            .flags
            .iter()
            .find(|flag| flag.name == "ignore_dev_dependency")
            .unwrap()
            .disposition,
        FlagDisposition::ParseOnly
    );
    assert_eq!(
        request
            .flags
            .iter()
            .find(|flag| flag.name == "lockfile_mode")
            .unwrap()
            .disposition,
        FlagDisposition::ParseOnly
    );
    assert_eq!(
        request.bzlmod_policy.stable_serialize(),
        "allow_yanked=[yyy@1.0.0,zzz@2.0.0];ignore_dev_dependency=true"
    );
    assert_eq!(request.lockfile_mode, LockfileMode::Refresh);
}

#[test]
fn independently_parsed_empty_build_and_cquery_overlays_share_allocation() {
    let build = BuildRequest::parse(&["//pkg:bin"]).unwrap();
    let other_build = BuildRequest::parse(&["//pkg:other"]).unwrap();
    let cquery = CqueryRequest::parse(&["//pkg:bin"]).unwrap();

    assert!(build.configuration_overlay.is_empty());
    assert!(cquery.configuration_overlay.is_empty());
    assert!(
        build
            .configuration_overlay
            .shares_allocation_with(&other_build.configuration_overlay)
    );
    assert!(
        build
            .configuration_overlay
            .shares_allocation_with(&cquery.configuration_overlay)
    );
}

#[test]
fn build_classifies_ordered_contextual_configuration_occurrences() {
    let request = BuildRequest::parse(&[
        "--//:setting=Gr\u{00fc}\u{00df}e",
        "--@dep//settings:enabled",
        "--no//settings:debug",
        "--extra_toolchains=//tc:a,//tc:b",
        "--extra_execution_platforms=//platform:x,",
        "--output_base=/tmp/slug",
        "--build_event_json_file=events.json",
        "--build_event_text_file=events.txt",
        "--bes_backend=grpcs://remote.buildbuddy.io",
        "--bes_results_url=https://app.buildbuddy.io/invocation/",
        "--remote_cache=grpcs://remote.buildbuddy.io",
        "--remote_executor=grpcs://remote.buildbuddy.io",
        "--remote_header=x-build=slug",
        "--remote_instance_name=main",
        "--remote_timeout=600",
        "--remote_retries=3",
        "--remote_default_exec_properties=cpu=x86_64",
        "--keep_going",
        "//pkg:bin",
    ])
    .unwrap();
    assert_eq!(request.configuration_overlay.len(), 5);
    assert!(matches!(
        request.configuration_overlay.iter().next(),
        Some(CommandConfigurationOccurrence::Starlark {
            apparent_label,
            raw_value: Some(value),
            negated: false,
        }) if apparent_label == "//:setting" && value == "Grüße"
    ));
    assert!(matches!(
        request.configuration_overlay.iter().nth(1),
        Some(CommandConfigurationOccurrence::Starlark {
            apparent_label,
            raw_value: None,
            negated: false,
        }) if apparent_label == "@dep//settings:enabled"
    ));
    assert!(matches!(
        request.configuration_overlay.iter().nth(2),
        Some(CommandConfigurationOccurrence::Starlark {
            apparent_label,
            raw_value: None,
            negated: true,
        }) if apparent_label == "//settings:debug"
    ));

    for flag in [
        "--config=dbg",
        "--unknown_configuration=value",
        "--@@dep+//settings:value=x",
        "--no//:setting=true",
        "--extra_toolchains",
        "--extra_execution_platforms",
    ] {
        let error = BuildRequest::parse(&[flag, "//pkg:bin"])
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("not supported by build")
                || error.contains("expected a direct root")
                || error.contains("does not accept")
                || error.contains("expected --extra")
        );
    }
}

#[test]
fn command_requests_extract_bzlmod_policy_flags() {
    let query = QueryRequest::parse(&["--output=label", "deps(//pkg:bin)"]).unwrap();
    let cquery = CqueryRequest::parse(&[
        "--output=starlark",
        "--starlark:expr=str(target.label)",
        "--ignore_dev_dependency=false",
        "--lockfile_mode=error",
        "--allow_yanked_versions=all",
        "//pkg:bin",
    ])
    .unwrap();
    let aquery = AqueryRequest::parse(&["--ignore_dev_dependency", "//pkg:bin"]).unwrap();
    let run = RunRequest::parse(&["--ignore_dev_dependency", "//pkg:bin"]).unwrap();
    let test = TestRequest::parse(&["--noignore_dev_dependency", "//pkg:probe_test"]).unwrap();

    assert_eq!(query.output, QueryOutputFormat::Label);
    assert!(!query.bzlmod_policy.ignore_dev_dependency());
    assert_eq!(query.lockfile_mode, LockfileMode::Update);
    assert_eq!(
        cquery.bzlmod_policy.stable_serialize(),
        "allow_yanked=all;ignore_dev_dependency=false"
    );
    assert_eq!(cquery.lockfile_mode, LockfileMode::Error);
    assert!(aquery.bzlmod_policy.ignore_dev_dependency());
    assert!(run.bzlmod_policy.ignore_dev_dependency());
    assert!(!test.bzlmod_policy.ignore_dev_dependency());
    assert_eq!(test.lockfile_mode, LockfileMode::Update);
}

#[test]
fn bzlmod_policy_flags_report_structured_parse_errors() {
    let allow_error = BuildRequest::parse(&["--allow_yanked_versions=not-a-module", "//pkg:bin"])
        .unwrap_err()
        .to_string();
    let bool_error = CqueryRequest::parse(&[
        "--output=starlark",
        "--starlark:expr=str(target.label)",
        "--ignore_dev_dependency=maybe",
        "//pkg:bin",
    ])
    .unwrap_err()
    .to_string();
    let lockfile_error = BuildRequest::parse(&["--lockfile_mode=bad", "//pkg:bin"])
        .unwrap_err()
        .to_string();

    assert!(allow_error.contains("module@version"));
    assert!(bool_error.contains("expected a boolean value"));
    assert!(lockfile_error.contains("Not a valid Lockfile mode"));
}

#[test]
fn build_bzlmod_inputs_normalize_default_override_default_without_state() {
    let default_a = BuildRequest::parse(&["//pkg:bin"]).unwrap();
    let override_request = BuildRequest::parse(&[
        "--allow_yanked_versions=yyy@1.0.0",
        "--ignore_dev_dependency",
        "--lockfile_mode=error",
        "//pkg:bin",
    ])
    .unwrap();
    let default_b = BuildRequest::parse(&["//pkg:bin"]).unwrap();

    assert_eq!(
        default_a.bzlmod_policy.stable_serialize(),
        "allow_yanked=reject;ignore_dev_dependency=false"
    );
    assert_eq!(
        override_request.bzlmod_policy.stable_serialize(),
        "allow_yanked=[yyy@1.0.0];ignore_dev_dependency=true"
    );
    assert_eq!(override_request.lockfile_mode, LockfileMode::Error);
    assert_eq!(default_a.bzlmod_policy, default_b.bzlmod_policy);
    assert_eq!(default_a.lockfile_mode, default_b.lockfile_mode);
}

#[test]
fn registry_flags_are_repeatable_equality_only_and_query_is_narrowly_allowed() {
    let build = BuildRequest::parse(&[
        "--registry=https://a.example/",
        "--registry=https://a.example",
        "--registry=file:///tmp/registry",
        "//pkg:bin",
    ])
    .unwrap();
    assert_eq!(
        build.registry_urls,
        [
            "https://a.example/".to_owned(),
            "https://a.example".to_owned(),
            "file:///tmp/registry".to_owned(),
        ]
    );
    let default = BuildRequest::parse(&["//pkg:bin"]).unwrap();
    assert!(default.registry_urls.is_empty());

    let query = QueryRequest::parse(&[
        "--registry=https://a.example/",
        "--registry=file:///tmp/registry",
        "//pkg:bin",
    ])
    .unwrap();
    assert_eq!(
        query.registry_urls,
        ["https://a.example/", "file:///tmp/registry"]
    );
    for args in [
        vec!["--registry", "//pkg:bin"],
        vec!["--registry=", "//pkg:bin"],
        vec!["--registry", "https://a.example", "//pkg:bin"],
    ] {
        let error = BuildRequest::parse(&args).unwrap_err().to_string();
        assert!(error.contains("non-empty registry URL"), "{error}");
    }
    let query_error = QueryRequest::parse(&["--registry", "https://a.example", "//pkg:bin"])
        .unwrap_err()
        .to_string();
    assert!(
        query_error.contains("non-empty registry URL"),
        "{query_error}"
    );
    let unsupported = QueryRequest::parse(&["--lockfile_mode=off", "//pkg:bin"])
        .unwrap_err()
        .to_string();
    assert!(unsupported.contains("not supported by loading query"));
}

#[test]
fn environment_value_normalization_is_pure_and_source_specific() {
    let default_a = normalize_bzlmod_environment_value(None).unwrap();
    let allow = normalize_bzlmod_environment_value(Some("zzz@2.0.0, yyy@1.0.0")).unwrap();
    let default_b = normalize_bzlmod_environment_value(None).unwrap();
    assert_eq!(default_a, default_b);
    assert_eq!(
        allow.stable_serialize(),
        "allow_yanked=[yyy@1.0.0,zzz@2.0.0]"
    );

    let error = normalize_bzlmod_environment_value(Some("not-a-module"))
        .unwrap_err()
        .to_string();
    assert!(error.contains("BZLMOD_ALLOW_YANKED_VERSIONS"), "{error}");
    assert!(error.contains("module@version"), "{error}");
}

#[test]
fn query_request_parses_expression_and_output_format() {
    let request =
        QueryRequest::parse(&["--output=label", "--order_output=full", "deps(//pkg:bin)"]).unwrap();

    assert_eq!(request.expression.to_string(), "deps(//pkg:bin)");
    assert_eq!(request.output, QueryOutputFormat::Label);
    assert_eq!(request.order, slug_query_v2::QueryOrder::Full);
    let compatible = QueryRequest::parse(&[
        "--noshow_progress",
        "--output=label",
        "//pkg:bin",
        "--noshow_progress",
    ])
    .unwrap();
    assert_eq!(compatible.expression, "//pkg:bin");
    assert_eq!(
        compatible
            .flags
            .iter()
            .map(|flag| flag.raw.as_str())
            .collect::<Vec<_>>(),
        ["--noshow_progress", "--output=label", "--noshow_progress"]
    );
    assert!(
        compatible
            .flags
            .iter()
            .filter(|flag| flag.name == "noshow_progress")
            .all(|flag| flag.disposition == FlagDisposition::IgnoredCompatible)
    );
}

#[test]
fn query_request_parses_graph_output_and_factoring_flags() {
    let default = QueryRequest::parse(&["--output=graph", "//pkg:bin"]).unwrap();
    assert_eq!(default.output, QueryOutputFormat::Graph);
    assert!(default.graph_factored);

    for (flag, expected) in [
        ("--graph:factored", true),
        ("--graph:factored=true", true),
        ("--graph:factored=false", false),
        ("--nograph:factored", false),
    ] {
        let request = QueryRequest::parse(&["--output=graph", flag, "//pkg:bin"]).unwrap();
        assert_eq!(request.graph_factored, expected, "{flag}");
    }

    assert!(
        QueryRequest::parse(&["--output=graph", "--graph:node_limit=512", "//pkg:bin",]).is_ok()
    );
    let error = QueryRequest::parse(&["--output=graph", "--graph:node_limit=513", "//pkg:bin"])
        .unwrap_err()
        .to_string();
    assert!(error.contains("other than 512 is deferred"), "{error}");
}

#[test]
fn query_request_parses_strict_test_suite_booleans_and_last_occurrence_wins() {
    let default = QueryRequest::parse(&["//pkg:bin"]).unwrap();
    assert_eq!(default.policy, QueryPolicy::default());

    for (flag, expected) in [
        ("--strict_test_suite", true),
        ("--strict_test_suite=true", true),
        ("--strict_test_suite=yes", true),
        ("--strict_test_suite=1", true),
        ("--strict_test_suite=false", false),
        ("--strict_test_suite=no", false),
        ("--strict_test_suite=0", false),
        ("--nostrict_test_suite", false),
        ("--nostrict_test_suite=true", false),
        ("--nostrict_test_suite=false", true),
    ] {
        let request = QueryRequest::parse(&[flag, "//pkg:bin"]).unwrap();
        assert_eq!(request.policy.strict_test_suite, expected, "{flag}");
        assert_eq!(
            request
                .flags
                .iter()
                .find(|parsed| parsed.raw == flag)
                .unwrap()
                .disposition,
            FlagDisposition::ParseOnly,
            "{flag}"
        );
    }

    let request = QueryRequest::parse(&[
        "--strict_test_suite",
        "--nostrict_test_suite",
        "--strict_test_suite=false",
        "--nostrict_test_suite=false",
        "//pkg:bin",
    ])
    .unwrap();
    assert!(request.policy.strict_test_suite);

    for flag in ["--strict_test_suite=maybe", "--nostrict_test_suite=maybe"] {
        let error = QueryRequest::parse(&[flag, "//pkg:bin"])
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("expected a boolean value"),
            "{flag}: {error}"
        );
    }
}

#[test]
fn query_request_accepts_label_label_kind_and_package_and_rejects_deferred_or_text_output() {
    let label = QueryRequest::parse(&["--output=label", "//pkg:bin"]).unwrap();
    assert_eq!(label.output, QueryOutputFormat::Label);
    let label_kind = QueryRequest::parse(&["--output=label_kind", "//pkg:bin"]).unwrap();
    assert_eq!(label_kind.output, QueryOutputFormat::LabelKind);
    let package = QueryRequest::parse(&["--output=package", "//pkg:bin"]).unwrap();
    assert_eq!(package.output, QueryOutputFormat::Package);

    let output = QueryRequest::parse(&["--output=build", "//pkg:bin"])
        .unwrap_err()
        .to_string();
    assert!(output.contains("only label, graph, label_kind, and package are implemented"));

    let text = QueryRequest::parse(&["--output=text", "//pkg:bin"])
        .unwrap_err()
        .to_string();
    assert!(text.contains("Invalid output format 'text'"), "{text}");
    assert!(
        text.contains(
            "label, label_kind, build, minrank, maxrank, package, location, graph, xml, proto, streamed_jsonproto, streamed_proto"
        ),
        "{text}"
    );

    for order in ["deps", "no"] {
        let error =
            QueryRequest::parse(&[format!("--order_output={order}"), "//pkg:bin".to_owned()])
                .unwrap_err()
                .to_string();
        assert!(error.contains("not supported"), "{error}");
    }
}

#[test]
fn query_request_rejects_missing_values_and_every_unsupported_flag_class() {
    let boolean_value = "Unexpected value after boolean option";
    for (flag, expected) in [
        ("--output", "expected label, graph, label_kind, or package"),
        ("--output=", "expected label, graph, label_kind, or package"),
        ("--order_output", "expected auto or full"),
        ("--order_output=", "expected auto or full"),
        ("--output_base", "expected a non-empty path"),
        ("--output_base=", "expected a non-empty path"),
        (
            "--allow_yanked_versions=all",
            "not supported by loading query",
        ),
        ("--ignore_dev_dependency", "not supported by loading query"),
        ("--lockfile_mode=off", "not supported by loading query"),
        ("--config=dbg", "not supported by loading query"),
        ("--keep_going", "not supported by loading query"),
        ("--noshow_progress=", boolean_value),
        ("--noshow_progress=true", boolean_value),
        ("--noshow_progress=false", boolean_value),
        ("--color=no", "not supported by loading query"),
        ("--show_progress", "not supported by loading query"),
        (
            "--build_event_json_file=events.json",
            "not supported by loading query",
        ),
        (
            "--remote_cache=grpc://cache",
            "not supported by loading query",
        ),
        ("--unknown_flag=value", "not supported by loading query"),
    ] {
        let error = QueryRequest::parse(&[flag, "//pkg:bin"])
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "{flag}: {error}");
    }
}

#[test]
fn cquery_accepts_only_the_label_kind_and_bounded_graph_output_matrix() {
    let default = CqueryRequest::parse(&["//pkg:bin"]).unwrap();
    assert_eq!(default.output_mode, CqueryOutputMode::Label);
    assert!(default.include_implicit);
    assert!(default.include_tool);

    let label = CqueryRequest::parse(&["--output=label", "//pkg:bin"]).unwrap();
    assert_eq!(label.output_mode, CqueryOutputMode::Label);

    let label_kind = CqueryRequest::parse(&["--output=label_kind", "//pkg:bin"]).unwrap();
    assert_eq!(label_kind.output_mode, CqueryOutputMode::LabelKind);

    let starlark = CqueryRequest::parse(&[
        "--output=starlark",
        "--starlark:expr=str(target.label)",
        "--output_base=/tmp/slug-cquery",
        "//pkg:bin",
    ])
    .unwrap();
    assert_eq!(starlark.output_mode, CqueryOutputMode::StarlarkLabel);
    assert_eq!(starlark.expression, "//pkg:bin");
    assert_eq!(starlark.output_base.as_deref(), Some("/tmp/slug-cquery"));

    let graph = CqueryRequest::parse(&[
        "--output=graph",
        "--nograph:factored",
        "--noimplicit_deps",
        "deps(//pkg:bin)",
    ])
    .unwrap();
    assert_eq!(graph.output_mode, CqueryOutputMode::Graph);
    assert!(!graph.include_implicit);
    for depth in [0, 1, 2, 3, i32::MAX] {
        let graph = CqueryRequest::parse(&[
            "--output=graph",
            "--nograph:factored",
            "--noimplicit_deps",
            &format!("deps(//pkg:bin, {depth})"),
        ])
        .unwrap();
        assert_eq!(graph.output_mode, CqueryOutputMode::Graph, "depth {depth}");
    }
    for args in [
        vec!["--noimplicit_deps", "executables(deps(//pkg:bin))"],
        vec![
            "--output=label_kind",
            "--noimplicit_deps",
            "executables(deps(//pkg:bin, 2))",
        ],
        vec![
            "--output=starlark",
            "--starlark:expr=str(target.label)",
            "--noimplicit_deps",
            "executables(deps(//pkg:bin, 2147483647))",
        ],
        vec![
            "--output=graph",
            "--nograph:factored",
            "--noimplicit_deps",
            "executables(deps(//pkg:bin))",
        ],
        vec![
            "--output=graph",
            "--nograph:factored",
            "--noimplicit_deps",
            "filter('^//pkg:', deps(//pkg:bin))",
        ],
        vec![
            "--output=graph",
            "--nograph:factored",
            "--noimplicit_deps",
            "kind('rule$', deps(//pkg:bin, 2147483647))",
        ],
        vec![
            "--output=graph",
            "--nograph:factored",
            "--noimplicit_deps",
            "filter('bin$', executables(deps(//pkg:bin, 2147483647)))",
        ],
        vec![
            "--output=graph",
            "--nograph:factored",
            "--noimplicit_deps",
            "filter('bin$', kind('rule$', deps(//pkg:bin, 2147483647)))",
        ],
        vec![
            "--output=graph",
            "--nograph:factored",
            "--noimplicit_deps",
            "rdeps(deps(//pkg:bin), //pkg:child)",
        ],
        vec![
            "--output=graph",
            "--nograph:factored",
            "--noimplicit_deps",
            "rdeps(//pkg:bin, //pkg:child)",
        ],
    ] {
        assert!(CqueryRequest::parse(&args).is_ok(), "{args:?}");
    }
    for expression in ["deps(//pkg:bin, -1)", "deps(//pkg:bin, 2147483648)"] {
        assert!(
            CqueryRequest::parse(&[
                "--output=graph",
                "--nograph:factored",
                "--noimplicit_deps",
                expression,
            ])
            .is_err(),
            "{expression}"
        );
    }

    let set = CqueryRequest::parse(&["set(//pkg:bin //pkg:lib) union //pkg:bin"]).unwrap();
    assert_eq!(set.output_mode, CqueryOutputMode::Label);
    assert_eq!(set.expression, "set(//pkg:bin //pkg:lib) union //pkg:bin");
    assert!(CqueryRequest::parse(&["let x = set() in $x"]).is_ok());
    assert!(CqueryRequest::parse(&["filter('^//pkg:', set(//pkg:bin //pkg:lib))"]).is_ok());
    assert!(CqueryRequest::parse(&["filter('(', //pkg:bin)"]).is_ok());
    assert!(CqueryRequest::parse(&["some(//pkg:bin)"]).is_ok());
    assert!(CqueryRequest::parse(&["executables(//pkg:bin)"]).is_ok());
    assert!(CqueryRequest::parse(&["kind('rule', //pkg:bin)"]).is_ok());
    assert!(CqueryRequest::parse(&["siblings(//pkg:bin)"]).is_ok());
    for expression in [
        "deps(//pkg:bin)",
        "filter()",
        "filter(set(//pkg:bin), //pkg:bin)",
    ] {
        assert!(CqueryRequest::parse(&[expression]).is_err(), "{expression}");
    }

    for expression in [
        "set(//pkg:bin //pkg:lib //pkg:bin)",
        "let x = set(//pkg:bin //pkg:lib //pkg:bin) in ($x except //pkg:lib) union //pkg:lib",
        "set()",
    ] {
        let starlark = CqueryRequest::parse(&[
            "--output=starlark",
            "--starlark:expr=str(target.label)",
            expression,
        ])
        .unwrap();
        assert_eq!(starlark.output_mode, CqueryOutputMode::StarlarkLabel);
        assert_eq!(starlark.expression, expression);
    }

    for args in [
        vec![
            "--output=label",
            "--starlark:expr=str(target.label)",
            "//pkg:bin",
        ],
        vec![
            "--output=starlark",
            "--starlark:expr=target.label",
            "//pkg:bin",
        ],
        vec!["--output=starlark", "//pkg:bin"],
        vec!["--starlark:expr=str(target.label)", "//pkg:bin"],
        vec!["--output=text", "//pkg:bin"],
        vec!["--nograph:factored", "--output=label_kind", "//pkg:bin"],
        vec![
            "--output=label_kind",
            "--starlark:expr=str(target.label)",
            "//pkg:bin",
        ],
        vec!["--output", "--starlark:expr=str(target.label)", "//pkg:bin"],
        vec!["--output=starlark", "--starlark:expr", "//pkg:bin"],
        vec![
            "--output=",
            "--starlark:expr=str(target.label)",
            "//pkg:bin",
        ],
        vec!["--output=starlark", "--starlark:expr=", "//pkg:bin"],
        vec!["--output=starlark", "--starlark:file=fmt.bzl", "//pkg:bin"],
        vec!["--output=graph", "--noimplicit_deps", "deps(//pkg:bin)"],
        vec![
            "--output=graph",
            "--nograph:factored",
            "--implicit_deps=false",
            "deps(//pkg:bin)",
        ],
        vec!["--output=graph", "--nograph:factored", "deps(//pkg:bin)"],
        vec![
            "--output=graph",
            "--nograph:factored=false",
            "--noimplicit_deps",
            "deps(//pkg:bin)",
        ],
        vec![
            "--output=graph",
            "--nograph:factored",
            "--noimplicit_deps",
            "filter('^//pkg:', //pkg:bin)",
        ],
        vec![
            "--output=graph:nofactored",
            "--noimplicit_deps",
            "deps(//pkg:bin)",
        ],
        vec!["--nograph:factored", "//pkg:bin"],
        vec![
            "--output=starlark",
            "--starlark:expr=str(target.label)",
            "--",
            "//pkg:bin",
        ],
        vec![
            "--output=starlark",
            "--starlark:expr=str(target.label)",
            "//pkg:bin",
            "//pkg:other",
        ],
        vec![
            "--output=starlark",
            "--starlark:expr=str(target.label)",
            "//pkg:all",
        ],
        vec![
            "--output=starlark",
            "--starlark:expr=str(target.label)",
            "//pkg/...",
        ],
        vec![
            "--output=starlark",
            "--starlark:expr=str(target.label)",
            "@dep//pkg:bin",
        ],
        vec![
            "--output=starlark",
            "--starlark:expr=str(target.label)",
            "--order_output=full",
            "//pkg:bin",
        ],
        vec!["deps(//pkg:bin)"],
        vec!["1"],
        vec!["$x"],
        vec!["let x = $x in //pkg:bin"],
        vec!["//pkg:all"],
        vec!["@dep//pkg:bin"],
    ] {
        assert!(CqueryRequest::parse(&args).is_err(), "{args:?}");
    }

    assert!(CqueryRequest::parse(&["--output=label", "--output=label", "//pkg:bin",]).is_err());
    assert!(
        CqueryRequest::parse(&[
            "--output=starlark",
            "--starlark:expr=str(target.label)",
            "--starlark:expr=str(target.label)",
            "//pkg:bin",
        ])
        .is_err()
    );
}

#[test]
fn aquery_accepts_one_main_repo_literal_or_deps_and_text_output() {
    let default = AqueryRequest::parse(&["//pkg:bin"]).unwrap();
    assert_eq!(default.expression, "//pkg:bin");
    assert_eq!(default.target.to_string(), "//pkg:bin");
    assert_eq!(default.scope, slug_query_v2::AqueryScope::Literal);
    assert_eq!(default.output_base, None);

    let deps = AqueryRequest::parse(&["deps(//pkg:bin)"]).unwrap();
    assert_eq!(deps.target.to_string(), "//pkg:bin");
    assert_eq!(deps.scope, slug_query_v2::AqueryScope::Deps);

    let explicit = AqueryRequest::parse(&[
        "--output=text",
        "--output_base=out",
        "--registry=https://registry.example/",
        "(//pkg:bin)",
    ])
    .unwrap();
    assert_eq!(explicit.target.to_string(), "//pkg:bin");
    assert_eq!(explicit.output_base.as_deref(), Some("out"));
    assert_eq!(
        explicit.registry_urls,
        vec!["https://registry.example/".to_owned()]
    );

    for args in [
        vec!["deps(//pkg:bin, 1)"],
        vec!["deps(deps(//pkg:bin))"],
        vec!["kind('rule', //pkg:bin)"],
        vec!["//pkg:all"],
        vec!["@dep//pkg:bin"],
        vec!["//pkg:bin", "//pkg:other"],
        vec!["--output=jsonproto", "//pkg:bin"],
        vec!["--output=text", "--output=text", "//pkg:bin"],
        vec!["--noshow_progress", "//pkg:bin"],
        vec!["//pkg:bin", "--", "passthrough"],
    ] {
        assert!(AqueryRequest::parse(&args).is_err(), "{args:?}");
    }
}

#[test]
fn cquery_deps_requires_noimplicit_and_preserves_bazel_boolean_spellings() {
    assert!(CqueryRequest::parse(&["deps(//pkg:bin)"]).is_err());
    assert!(CqueryRequest::parse(&["rdeps(deps(//pkg:bin), //pkg:child)"]).is_err());
    assert!(CqueryRequest::parse(&["rdeps(//pkg:bin, //pkg:child)"]).is_err());
    for expression in [
        "executables(deps(//pkg:bin))",
        "filter('bin$', executables(deps(//pkg:bin)))",
        "filter('bin$', kind('rule$', deps(//pkg:bin)))",
    ] {
        assert!(CqueryRequest::parse(&[expression]).is_err());
    }
    assert!(CqueryRequest::parse(&["filter('bin', deps(//pkg:bin))"]).is_err());
    assert!(CqueryRequest::parse(&["kind('rule$', deps(//pkg:bin))"]).is_err());
    assert!(CqueryRequest::parse(&["--notool_deps", "deps(//pkg:bin)"]).is_err());

    let noimplicit = CqueryRequest::parse(&["--noimplicit_deps", "deps(//pkg:bin)"]).unwrap();
    assert!(!noimplicit.include_implicit);
    assert!(noimplicit.include_tool);
    assert!(
        CqueryRequest::parse(&["--noimplicit_deps", "rdeps(deps(//pkg:bin), //pkg:child)",])
            .is_ok()
    );
    assert!(
        CqueryRequest::parse(&[
            "--noimplicit_deps",
            "filter('(', rdeps(//pkg:bin, //pkg:child))",
        ])
        .is_ok()
    );
    assert!(
        CqueryRequest::parse(&[
            "--noimplicit_deps",
            "executables(rdeps(//pkg:bin, //pkg:child))",
        ])
        .is_ok()
    );
    assert!(
        CqueryRequest::parse(&[
            "--noimplicit_deps",
            "kind('(', rdeps(//pkg:bin, //pkg:child))",
        ])
        .is_ok()
    );
    for depth in ["0", "1", "'-1'", "'-2147483648'", "2147483647"] {
        let expression = format!("rdeps(deps(//pkg:bin), //pkg:child, {depth})");
        assert!(CqueryRequest::parse(&["--noimplicit_deps", &expression]).is_ok());
        let direct = format!("rdeps(//pkg:bin, //pkg:child, {depth})");
        assert!(CqueryRequest::parse(&["--noimplicit_deps", &direct]).is_ok());
        let filtered = format!("filter('child$', rdeps(//pkg:bin, //pkg:child, {depth}))");
        assert!(CqueryRequest::parse(&["--noimplicit_deps", &filtered]).is_ok());
        let executable = format!("executables(rdeps(//pkg:bin, //pkg:child, {depth}))");
        assert!(CqueryRequest::parse(&["--noimplicit_deps", &executable]).is_ok());
        let kind = format!("kind('rule', rdeps(//pkg:bin, //pkg:child, {depth}))");
        assert!(CqueryRequest::parse(&["--noimplicit_deps", &kind]).is_ok());
    }
    for depth in ["2147483648", "'-2147483649'"] {
        let expression = format!("rdeps(deps(//pkg:bin), //pkg:child, {depth})");
        assert!(CqueryRequest::parse(&["--noimplicit_deps", &expression]).is_err());
    }
    for depth in ["0", "1", "2", "2147483647"] {
        let expression = format!("rdeps(deps(//pkg:bin, {depth}), //pkg:child, 1)");
        assert!(CqueryRequest::parse(&["--noimplicit_deps", &expression]).is_ok());
    }
    for depth in ["'-1'", "2147483648"] {
        let expression = format!("rdeps(deps(//pkg:bin, {depth}), //pkg:child)");
        assert!(CqueryRequest::parse(&["--noimplicit_deps", &expression]).is_err());
    }
    for expression in [
        "rdeps(set(//pkg:bin), //pkg:child)",
        "rdeps(executables(//pkg:bin), //pkg:child)",
        "rdeps(//pkg:bin union //pkg:other, //pkg:child)",
        "rdeps(//pkg:all, //pkg:child)",
        "rdeps(//pkg/..., //pkg:child)",
        "rdeps(@dep//pkg:bin, //pkg:child)",
        "rdeps(//pkg:bin, @dep//pkg:child)",
        "filter('child$', rdeps(deps(//pkg:bin), //pkg:child))",
        "filter('child$', rdeps(set(//pkg:bin), //pkg:child))",
        "filter('child$', rdeps(//pkg:bin, set(//pkg:child)))",
        "filter('child$', rdeps(@dep//pkg:bin, //pkg:child))",
        "filter('child$', rdeps(//pkg:bin, @dep//pkg:child))",
        "filter('child$', rdeps(//pkg/..., //pkg:child))",
        "filter('child$', filter('child$', rdeps(//pkg:bin, //pkg:child)))",
        "executables(rdeps(deps(//pkg:bin), //pkg:child))",
        "executables(rdeps(set(//pkg:bin), //pkg:child))",
        "executables(rdeps(//pkg:bin, set(//pkg:child)))",
        "executables(rdeps(@dep//pkg:bin, //pkg:child))",
        "executables(rdeps(//pkg:bin, @dep//pkg:child))",
        "executables(rdeps(//pkg/..., //pkg:child))",
        "executables(executables(rdeps(//pkg:bin, //pkg:child)))",
        "filter('child$', executables(rdeps(//pkg:bin, //pkg:child)))",
        "kind('rule', rdeps(deps(//pkg:bin), //pkg:child))",
        "kind('rule', rdeps(set(//pkg:bin), //pkg:child))",
        "kind('rule', rdeps(//pkg:bin, set(//pkg:child)))",
        "kind('rule', rdeps(@dep//pkg:bin, //pkg:child))",
        "kind('rule', rdeps(//pkg:bin, @dep//pkg:child))",
        "kind('rule', rdeps(//pkg/..., //pkg:child))",
        "kind('rule', kind('rule', rdeps(//pkg:bin, //pkg:child)))",
        "executables(kind('rule', rdeps(//pkg:bin, //pkg:child)))",
    ] {
        assert!(
            CqueryRequest::parse(&["--noimplicit_deps", expression]).is_err(),
            "{expression}"
        );
    }

    let filtered = CqueryRequest::parse(&[
        "--implicit_deps=false",
        "--tool_deps=false",
        "deps(//pkg:bin, 2)",
    ])
    .unwrap();
    assert!(!filtered.include_implicit);
    assert!(!filtered.include_tool);

    let filtered_wrapper =
        CqueryRequest::parse(&["--noimplicit_deps", "filter('bin', deps(//pkg:bin, 2))"]).unwrap();
    assert!(!filtered_wrapper.include_implicit);

    let kind_wrapper =
        CqueryRequest::parse(&["--noimplicit_deps", "kind('rule$', deps(//pkg:bin, 2))"]).unwrap();
    assert!(!kind_wrapper.include_implicit);

    let named_kind = CqueryRequest::parse(&[
        "--noimplicit_deps",
        "filter('bin$', kind('rule$', deps(//pkg:bin, 2)))",
    ])
    .unwrap();
    assert!(!named_kind.include_implicit);

    let negated_values = CqueryRequest::parse(&[
        "--noimplicit_deps=true",
        "--notool_deps=true",
        "deps(//pkg:bin)",
    ])
    .unwrap();
    assert!(!negated_values.include_implicit);
    assert!(!negated_values.include_tool);

    let non_deps =
        CqueryRequest::parse(&["--noimplicit_deps", "--notool_deps", "//pkg:bin"]).unwrap();
    assert!(!non_deps.include_implicit);
    assert!(!non_deps.include_tool);

    for flag in ["--implicit_deps=maybe", "--notool_deps=maybe"] {
        let error = CqueryRequest::parse(&[flag, "//pkg:bin"])
            .unwrap_err()
            .to_string();
        assert!(error.contains("expected a boolean value"), "{error}");
    }
}

#[test]
fn cquery_configuration_overlay_preserves_unicode_empty_and_every_occurrence() {
    let default = CqueryRequest::parse(&["//pkg:bin"]).unwrap();
    assert!(default.configuration_overlay.is_empty());

    let unicode = CqueryRequest::parse(&["--//:setting=Grüße", "//pkg:bin"]).unwrap();
    assert!(matches!(
        unicode.configuration_overlay.iter().next(),
        Some(CommandConfigurationOccurrence::Starlark {
            raw_value: Some(value),
            ..
        }) if value == "Grüße"
    ));

    let empty = CqueryRequest::parse(&["--//:setting=", "//pkg:bin"]).unwrap();
    assert!(matches!(
        empty.configuration_overlay.iter().next(),
        Some(CommandConfigurationOccurrence::Starlark {
            raw_value: Some(value),
            ..
        }) if value.is_empty()
    ));

    let repeated =
        CqueryRequest::parse(&["--//:setting=first", "--//:setting=最後", "//pkg:bin"]).unwrap();
    assert_eq!(repeated.configuration_overlay.len(), 2);
    assert!(matches!(
        repeated.configuration_overlay.iter().nth(1),
        Some(CommandConfigurationOccurrence::Starlark {
            raw_value: Some(value),
            ..
        }) if value == "最後"
    ));

    let boolean = CqueryRequest::parse(&["--//:setting", "--no//:other", "//pkg:bin"]).unwrap();
    assert_eq!(boolean.configuration_overlay.len(), 2);
}

#[test]
fn query_request_preserves_invalid_syntax_for_authoritative_runtime_parse() {
    let request = QueryRequest::parse(&["deps(//pkg:bin"]).unwrap();
    assert_eq!(request.expression, "deps(//pkg:bin");
}

#[test]
fn run_request_preserves_program_args_after_target() {
    let request = RunRequest::parse(&["//pkg:bin", "--", "--name", "slug"]).unwrap();

    assert_eq!(request.target.to_string(), "//pkg:bin");
    assert_eq!(request.program_args, vec!["--name", "slug"]);
}

#[test]
fn test_keeps_its_placeholder_while_aquery_owns_a_literal_request() {
    let test = TestRequest::parse(&["//pkg:probe_test"]).unwrap();
    let aquery = AqueryRequest::parse(&["//pkg:probe"]).unwrap();

    assert!(
        test.placeholder_error()
            .to_json_line()
            .contains("Stage 7/8")
    );
    assert_eq!(aquery.target.to_string(), "//pkg:probe");
}

#[test]
fn help_summary_is_bazel_v2_command_surface_only() {
    assert!(HELP_SUMMARY.contains("build <target-pattern>"));
    assert!(!HELP_SUMMARY.contains("legacy"));
    assert!(!HELP_SUMMARY.contains("cell"));
    assert!(!HELP_SUMMARY.contains("out"));
}

#[test]
fn command_module_overrides_normalize_fold_and_forward_for_active_commands() {
    let workspace = std::path::Path::new("/workspace");
    let flags = [
        "--override_module=zed=relative/../zed",
        "--override_module=a_mod=%workspace%/deps/a=with=equals",
        "--override_module=zed=",
        "--override_module=zed=/absolute/zed",
        "--override_module=a=/a",
        "--override_module=a.b-c_d9=relative/path",
        "--override_module=replace=/one",
        "--override_module=replace=/two",
        "--override_module=gone=/gone",
        "--override_module=gone=",
    ];
    let build_args = flags
        .iter()
        .copied()
        .chain(["//pkg:bin"])
        .collect::<Vec<_>>();
    let build = BuildRequest::parse_at_workspace(&build_args, workspace).unwrap();
    assert_eq!(
        build
            .bzlmod_policy
            .module_overrides()
            .map(|(name, path)| (name.to_owned(), path.display().to_string()))
            .collect::<Vec<_>>(),
        [
            ("a".to_owned(), "/a".to_owned()),
            ("a.b-c_d9".to_owned(), "/workspace/relative/path".to_owned()),
            (
                "a_mod".to_owned(),
                "/workspace/deps/a=with=equals".to_owned()
            ),
            ("replace".to_owned(), "/two".to_owned()),
            ("zed".to_owned(), "/absolute/zed".to_owned()),
        ]
    );
    assert!(BuildRequest::parse(&build_args).is_err());

    let reordered = BuildRequest::parse_at_workspace(
        &[
            "--override_module=zed=/absolute/zed",
            "--override_module=a=/a",
            "--override_module=replace=/two",
            "--override_module=a_mod=/workspace/deps/a=with=equals",
            "--override_module=a.b-c_d9=/workspace/relative/path",
            "//pkg:bin",
        ],
        workspace,
    )
    .unwrap();
    assert_eq!(build.bzlmod_policy, reordered.bzlmod_policy);
    assert_eq!(
        build.bzlmod_policy.stable_serialize(),
        "allow_yanked=reject;ignore_dev_dependency=false;module_overrides=a=/a,a.b-c_d9=/workspace/relative/path,a_mod=/workspace/deps/a=with=equals,replace=/two,zed=/absolute/zed"
    );
    assert!(
        QueryRequest::parse_at_workspace(
            &["--override_module=dep=deps/dep", "//pkg:bin"],
            workspace,
        )
        .unwrap()
        .bzlmod_policy
        .module_overrides()
        .any(|(name, _)| name == "dep")
    );
    assert!(
        AqueryRequest::parse_at_workspace(
            &["--override_module=dep=deps/dep", "//pkg:bin"],
            workspace,
        )
        .unwrap()
        .bzlmod_policy
        .module_overrides()
        .any(|(name, _)| name == "dep")
    );
    assert!(
        CqueryRequest::parse_at_workspace(
            &["--override_module=dep=deps/dep", "//pkg:bin"],
            workspace,
        )
        .unwrap()
        .bzlmod_policy
        .module_overrides()
        .any(|(name, _)| name == "dep")
    );
}

#[test]
fn command_module_override_errors_follow_raw_argv_order() {
    let workspace = std::path::Path::new("/workspace");
    for (value, message) in [
        ("bad", "module-name=path"),
        ("=/dep", "invalid module name"),
        ("Bad=/dep", "invalid module name"),
        ("9bad=/dep", "invalid module name"),
        ("a/b=/dep", "invalid module name"),
        ("a.=/dep", "invalid module name"),
        ("a_=/dep", "invalid module name"),
    ] {
        let flag = format!("--override_module={value}");
        let error = BuildRequest::parse_at_workspace(&[flag.as_str(), "//pkg:bin"], workspace)
            .unwrap_err()
            .to_string();
        assert!(error.contains(message), "{error}");
    }

    let missing_value =
        BuildRequest::parse_at_workspace(&["--override_module", "//pkg:bin"], workspace)
            .unwrap_err()
            .to_string();
    assert!(
        missing_value.contains("expected module-name=path"),
        "{missing_value}"
    );

    let nul_path = BuildRequest::parse_at_workspace(
        &["--override_module=dep=/bad path", "//pkg:bin"],
        workspace,
    )
    .unwrap_err()
    .to_string();
    assert!(nul_path.contains("must not contain NUL"), "{nul_path}");

    let first = BuildRequest::parse_at_workspace(
        &[
            "--override_module=Bad=/dep",
            "--override_module=also_bad",
            "//pkg:bin",
        ],
        workspace,
    )
    .unwrap_err()
    .to_string();
    assert!(first.contains("Bad"), "{first}");
    assert!(
        TestRequest::parse(&["--override_module=dep=/dep", "//pkg:probe_test"])
            .unwrap_err()
            .to_string()
            .contains("workspace-owned command path")
    );
}

#[test]
fn repository_environment_overrides_are_ordered_category_wide_and_redacted() {
    let args = [
        "--repo_env=B=first=tail",
        "--repo_env=A",
        "--repo_env==B",
        "--repo_env=B=",
        "//pkg:bin",
    ];
    let expected = vec![
        RepositoryEnvironmentOverride::Set {
            name: "B".to_owned(),
            value: "first=tail".to_owned(),
        },
        RepositoryEnvironmentOverride::Inherit {
            name: "A".to_owned(),
        },
        RepositoryEnvironmentOverride::Unset {
            name: "B".to_owned(),
        },
        RepositoryEnvironmentOverride::Set {
            name: "B".to_owned(),
            value: String::new(),
        },
    ];
    let build = BuildRequest::parse(&args).unwrap();
    assert_eq!(build.repository_environment_overrides, expected);
    assert!(!format!("{:?}", build.repository_environment_overrides).contains("first=tail"));
    let debug = format!("{build:?}");
    assert!(!debug.contains("first=tail"), "{debug}");
    assert!(debug.contains("--repo_env=<redacted>"), "{debug}");

    let category_debug = [
        format!(
            "{:?}",
            QueryRequest::parse(&["--repo_env=A=category-secret", "//pkg:bin"]).unwrap()
        ),
        format!(
            "{:?}",
            AqueryRequest::parse(&["--repo_env=A=category-secret", "//pkg:bin"]).unwrap()
        ),
        format!(
            "{:?}",
            CqueryRequest::parse(&["--repo_env=A=category-secret", "//pkg:bin"]).unwrap()
        ),
        format!(
            "{:?}",
            RunRequest::parse(&["--repo_env=A=category-secret", "//pkg:bin"]).unwrap()
        ),
        format!(
            "{:?}",
            TestRequest::parse(&["--repo_env=A=category-secret", "//pkg:test"]).unwrap()
        ),
    ];
    for debug in category_debug {
        assert!(!debug.contains("category-secret"), "{debug}");
        assert!(debug.contains("<redacted>"), "{debug}");
    }

    for malformed in ["--repo_env", "--repo_env=", "--repo_env=="] {
        let error = BuildRequest::parse(&[malformed, "//pkg:bin"])
            .unwrap_err()
            .to_string();
        assert!(error.contains("--repo_env=<redacted>"), "{error}");
        assert!(!error.contains("sentinel-secret"), "{error}");
    }
}
