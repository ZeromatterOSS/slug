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
use slug_commands_v2::FlagDisposition;
use slug_commands_v2::HELP_SUMMARY;
use slug_commands_v2::QueryOutputFormat;
use slug_commands_v2::aquery::AqueryRequest;
use slug_commands_v2::build::BuildRequest;
use slug_commands_v2::cquery::CqueryRequest;
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
fn command_requests_extract_bzlmod_policy_flags() {
    let query = QueryRequest::parse(&["--output=text", "deps(//pkg:bin)"]).unwrap();
    let cquery = CqueryRequest::parse(&[
        "--ignore_dev_dependency=false",
        "--lockfile_mode=error",
        "--allow_yanked_versions=all",
        "//pkg:bin",
    ])
    .unwrap();
    let aquery = AqueryRequest::parse(&["--ignore_dev_dependency", "deps(//pkg:bin)"]).unwrap();
    let run = RunRequest::parse(&["--ignore_dev_dependency", "//pkg:bin"]).unwrap();
    let test = TestRequest::parse(&["--noignore_dev_dependency", "//pkg:probe_test"]).unwrap();

    assert_eq!(query.output, QueryOutputFormat::Text);
    assert!(!query.bzlmod_policy.ignore_dev_dependency());
    assert_eq!(query.lockfile_mode, LockfileMode::Update);
    assert_eq!(
        cquery.query.bzlmod_policy.stable_serialize(),
        "allow_yanked=all;ignore_dev_dependency=false"
    );
    assert_eq!(cquery.query.lockfile_mode, LockfileMode::Error);
    assert!(aquery.query.bzlmod_policy.ignore_dev_dependency());
    assert!(run.bzlmod_policy.ignore_dev_dependency());
    assert!(!test.bzlmod_policy.ignore_dev_dependency());
    assert_eq!(test.lockfile_mode, LockfileMode::Update);
}

#[test]
fn bzlmod_policy_flags_report_structured_parse_errors() {
    let allow_error = BuildRequest::parse(&["--allow_yanked_versions=not-a-module", "//pkg:bin"])
        .unwrap_err()
        .to_string();
    let bool_error = CqueryRequest::parse(&["--ignore_dev_dependency=maybe", "//pkg:bin"])
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
fn query_request_parses_expression_and_output_format() {
    let request =
        QueryRequest::parse(&["--output=text", "--order_output=full", "deps(//pkg:bin)"]).unwrap();

    assert_eq!(request.expression.to_string(), "deps(//pkg:bin)");
    assert_eq!(request.output, QueryOutputFormat::Text);
    assert_eq!(request.order, slug_query_v2::QueryOrder::Full);
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
fn query_request_rejects_deferred_output_and_order_modes_before_runtime() {
    let output = QueryRequest::parse(&["--output=label_kind", "//pkg:bin"])
        .unwrap_err()
        .to_string();
    assert!(output.contains("only text and graph are implemented"));

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
    for (flag, expected) in [
        ("--output", "expected text or graph"),
        ("--output=", "expected text or graph"),
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
fn cquery_and_aquery_retain_placeholder_flag_parsing_without_query_restrictions() {
    let cquery =
        CqueryRequest::parse(&["--output=label_kind", "--order_output=deps", "//pkg:bin"]).unwrap();
    assert_eq!(cquery.query.output, QueryOutputFormat::LabelKind);
    assert_eq!(cquery.query.order, slug_query_v2::QueryOrder::Auto);

    let aquery = AqueryRequest::parse(&[
        "--output=streamed_jsonproto",
        "--order_output=no",
        "//pkg:bin",
    ])
    .unwrap();
    assert_eq!(aquery.query.output, QueryOutputFormat::StreamedJsonProto);
    assert_eq!(aquery.query.order, slug_query_v2::QueryOrder::Auto);

    let cquery = CqueryRequest::parse(&["--strict_test_suite", "//pkg:bin"]).unwrap();
    let aquery = AqueryRequest::parse(&["--nostrict_test_suite=false", "//pkg:bin"]).unwrap();
    assert_eq!(cquery.query.policy, QueryPolicy::default());
    assert_eq!(aquery.query.policy, QueryPolicy::default());
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
fn test_cquery_and_aquery_have_stage_owned_placeholders() {
    let test = TestRequest::parse(&["//pkg:probe_test"]).unwrap();
    let cquery = CqueryRequest::parse(&["//pkg:probe"]).unwrap();
    let aquery = AqueryRequest::parse(&["deps(//pkg:probe)"]).unwrap();

    assert!(
        test.placeholder_error()
            .to_json_line()
            .contains("Stage 7/8")
    );
    assert!(
        cquery
            .placeholder_error()
            .to_json_line()
            .contains("Stage 6/8")
    );
    assert!(
        aquery
            .placeholder_error()
            .to_json_line()
            .contains("Stage 6/8")
    );
}

#[test]
fn help_summary_is_bazel_v2_command_surface_only() {
    assert!(HELP_SUMMARY.contains("build <target-pattern>"));
    assert!(!HELP_SUMMARY.contains("legacy"));
    assert!(!HELP_SUMMARY.contains("cell"));
    assert!(!HELP_SUMMARY.contains("out"));
}
