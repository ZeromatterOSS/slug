/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_commands_v2::FlagDisposition;
use slug_commands_v2::HELP_SUMMARY;
use slug_commands_v2::QueryOutputFormat;
use slug_commands_v2::aquery::AqueryRequest;
use slug_commands_v2::build::BuildRequest;
use slug_commands_v2::cquery::CqueryRequest;
use slug_commands_v2::query::QueryRequest;
use slug_commands_v2::run::RunRequest;
use slug_commands_v2::test::TestRequest;

#[test]
fn build_request_parses_target_patterns_and_classifies_flags() {
    let request = BuildRequest::parse(&[
        "--remote_executor=grpc://127.0.0.1:50051",
        "--keep_going",
        "--allow_yanked_versions=zzz@2.0.0,yyy@1.0.0",
        "--ignore_dev_dependency",
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
        request.bzlmod_policy.stable_serialize(),
        "allow_yanked=[yyy@1.0.0,zzz@2.0.0];ignore_dev_dependency=true"
    );
}

#[test]
fn command_requests_extract_bzlmod_policy_flags() {
    let query = QueryRequest::parse(&[
        "--ignore_dev_dependency=false",
        "--output=streamed_jsonproto",
        "deps(//pkg:bin)",
    ])
    .unwrap();
    let cquery = CqueryRequest::parse(&["--allow_yanked_versions=all", "//pkg:bin"]).unwrap();
    let aquery = AqueryRequest::parse(&["--ignore_dev_dependency", "deps(//pkg:bin)"]).unwrap();
    let run = RunRequest::parse(&["--ignore_dev_dependency", "//pkg:bin"]).unwrap();
    let test = TestRequest::parse(&["--noignore_dev_dependency", "//pkg:probe_test"]).unwrap();

    assert_eq!(query.output, QueryOutputFormat::StreamedJsonProto);
    assert!(!query.bzlmod_policy.ignore_dev_dependency());
    assert_eq!(
        cquery.query.bzlmod_policy.stable_serialize(),
        "allow_yanked=all;ignore_dev_dependency=false"
    );
    assert!(aquery.query.bzlmod_policy.ignore_dev_dependency());
    assert!(run.bzlmod_policy.ignore_dev_dependency());
    assert!(!test.bzlmod_policy.ignore_dev_dependency());
}

#[test]
fn bzlmod_policy_flags_report_structured_parse_errors() {
    let allow_error = BuildRequest::parse(&["--allow_yanked_versions=not-a-module", "//pkg:bin"])
        .unwrap_err()
        .to_string();
    let bool_error = QueryRequest::parse(&["--ignore_dev_dependency=maybe", "//pkg:bin"])
        .unwrap_err()
        .to_string();

    assert!(allow_error.contains("module@version"));
    assert!(bool_error.contains("expected a boolean value"));
}

#[test]
fn query_request_parses_expression_and_output_format() {
    let request = QueryRequest::parse(&["--output=streamed_jsonproto", "deps(//pkg:bin)"]).unwrap();

    assert_eq!(request.expression.to_string(), "deps(//pkg:bin)");
    assert_eq!(request.output, QueryOutputFormat::StreamedJsonProto);
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
