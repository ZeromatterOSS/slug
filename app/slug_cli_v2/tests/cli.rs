/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::process::Command;
use std::time::SystemTime;

fn slug() -> Command {
    Command::new(env!("CARGO_BIN_EXE_slug"))
}

fn scratch(name: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("slug-cli-{name}-{nanos}"));
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn write(path: impl AsRef<std::path::Path>, content: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, content).unwrap();
}

fn fixture_workspace(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/v2_oracle/fixtures")
        .join(name)
        .join("workspace")
}

#[derive(Clone, Copy)]
struct QueryOracleCase {
    args: &'static [&'static str],
    expected_labels: &'static [&'static str],
    error_fragments: &'static [&'static str],
}

struct NamedQueryOracleCase {
    name: &'static str,
    case: QueryOracleCase,
}

fn visibility_stage4_oracle_cases() -> [NamedQueryOracleCase; 12] {
    [
        NamedQueryOracleCase {
            name: "deps_package_group_include_cycle_retains_graph",
            case: QueryOracleCase {
                args: &["deps(//owner:cycle_visible)"],
                expected_labels: &[
                    "//owner:cycle_a",
                    "//owner:cycle_b",
                    "//owner:cycle_visible",
                ],
                error_fragments: &[],
            },
        },
        NamedQueryOracleCase {
            name: "misspelled_special_visibility_label_diagnostic",
            case: QueryOracleCase {
                args: &["//errors/misspelled:target"],
                expected_labels: &[],
                error_fragments: &[
                    "Invalid visibility label '//visibility:plubic'; did you mean //visibility:public or //visibility:private?",
                    "Evaluation of query",
                ],
            },
        },
        NamedQueryOracleCase {
            name: "malformed_package_specification_diagnostic",
            case: QueryOracleCase {
                args: &["//errors/malformed_spec:target"],
                expected_labels: &[],
                error_fragments: &[
                    "invalid package name 'not-a-package': must start with '//', '@', or be 'public' or 'private'",
                    "Evaluation of query",
                ],
            },
        },
        NamedQueryOracleCase {
            name: "labels_visibility_explicit_group_projects_raw_target",
            case: QueryOracleCase {
                args: &["labels(visibility, //owner:base_only)"],
                expected_labels: &["//owner:base"],
                error_fragments: &[],
            },
        },
        NamedQueryOracleCase {
            name: "labels_visibility_omitted_default_group_is_empty",
            case: QueryOracleCase {
                args: &["labels(visibility, //default_group:omitted)"],
                expected_labels: &[],
                error_fragments: &[],
            },
        },
        NamedQueryOracleCase {
            name: "labels_visibility_explicit_direct_specs_fail_non_loadable_lookup",
            case: QueryOracleCase {
                args: &["labels(visibility, //owner:direct_specs)"],
                expected_labels: &[],
                error_fragments: &[
                    "in 'visibility' of rule //owner:direct_specs: no such target '//viewer:__pkg__': target '__pkg__' not declared in package 'viewer'",
                ],
            },
        },
        NamedQueryOracleCase {
            name: "labels_visibility_omitted_default_direct_specs_is_empty",
            case: QueryOracleCase {
                args: &["labels(visibility, //default_direct:omitted)"],
                expected_labels: &[],
                error_fragments: &[],
            },
        },
        NamedQueryOracleCase {
            name: "deps_explicit_visibility_group_edge",
            case: QueryOracleCase {
                args: &["deps(//owner:base_only)"],
                expected_labels: &["//owner:base", "//owner:base_only"],
                error_fragments: &[],
            },
        },
        NamedQueryOracleCase {
            name: "deps_transitive_package_group_include_edges",
            case: QueryOracleCase {
                args: &["deps(//owner:top_reallow)"],
                expected_labels: &[
                    "//owner:base",
                    "//owner:middle",
                    "//owner:reallow",
                    "//owner:top",
                    "//owner:top_reallow",
                ],
                error_fragments: &[],
            },
        },
        NamedQueryOracleCase {
            name: "deps_inherited_group_edges_for_rule_and_source",
            case: QueryOracleCase {
                args: &["deps(set(//default_group:omitted //default_group:data.txt))"],
                expected_labels: &[
                    "//default_group:data.txt",
                    "//default_group:omitted",
                    "//owner:exact",
                ],
                error_fragments: &[],
            },
        },
        NamedQueryOracleCase {
            name: "deps_direct_package_specs_are_not_targets",
            case: QueryOracleCase {
                args: &["deps(//owner:direct_specs)"],
                expected_labels: &["//owner:direct_specs"],
                error_fragments: &[],
            },
        },
        NamedQueryOracleCase {
            name: "labels_visibility_explicit_subpackages_spec_fails_non_loadable_lookup",
            case: QueryOracleCase {
                args: &["labels(visibility, //owner:direct_subpackages_only)"],
                expected_labels: &[],
                error_fragments: &[
                    "in 'visibility' of rule //owner:direct_subpackages_only: no such target '//viewer:__subpackages__': target '__subpackages__' not declared in package 'viewer'",
                ],
            },
        },
    ]
}

fn visible_oracle_cases() -> [NamedQueryOracleCase; 25] {
    [
        NamedQueryOracleCase {
            name: "visible_explicit_default_and_direct_specs",
            case: QueryOracleCase {
                args: &[
                    "visible(//viewer:caller, set(//owner:explicit_public //owner:explicit_private //owner:default_private //defaults_public:default_public //owner:viewer_pkg //owner:viewer_subpackages //owner:other_subpackages))",
                ],
                expected_labels: &[
                    "//defaults_public:default_public",
                    "//owner:explicit_public",
                    "//owner:viewer_pkg",
                    "//owner:viewer_subpackages",
                ],
                error_fragments: &[],
            },
        },
        NamedQueryOracleCase {
            name: "visible_subpackage_distinguishes_pkg_from_subpackages",
            case: QueryOracleCase {
                args: &[
                    "visible(//viewer/sub:caller, set(//owner:viewer_pkg //owner:viewer_subpackages))",
                ],
                expected_labels: &["//owner:viewer_subpackages"],
                error_fragments: &[],
            },
        },
        NamedQueryOracleCase {
            name: "visible_same_package_overrides_private",
            case: QueryOracleCase {
                args: &["visible(//owner:ordinary, //owner:explicit_private)"],
                expected_labels: &["//owner:explicit_private"],
                error_fragments: &[],
            },
        },
        NamedQueryOracleCase {
            name: "visible_requires_all_callers",
            case: QueryOracleCase {
                args: &[
                    "visible(set(//viewer:caller //other:caller), set(//owner:explicit_public //owner:viewer_pkg))",
                ],
                expected_labels: &["//owner:explicit_public"],
                error_fragments: &[],
            },
        },
        NamedQueryOracleCase {
            name: "visible_empty_callers_is_vacuously_true",
            case: QueryOracleCase {
                args: &[
                    "visible(set(), set(//owner:explicit_private //owner:default_private //owner:viewer_pkg))",
                ],
                expected_labels: &[
                    "//owner:default_private",
                    "//owner:explicit_private",
                    "//owner:viewer_pkg",
                ],
                error_fragments: &[],
            },
        },
        NamedQueryOracleCase {
            name: "visible_package_group_exact_positive",
            case: QueryOracleCase {
                args: &["visible(//viewer:caller, //owner:exact_only)"],
                expected_labels: &["//owner:exact_only"],
                error_fragments: &[],
            },
        },
        NamedQueryOracleCase {
            name: "visible_package_group_exact_rejects_descendant",
            case: QueryOracleCase {
                args: &["visible(//viewer/sub:caller, //owner:exact_only)"],
                expected_labels: &[],
                error_fragments: &[],
            },
        },
        NamedQueryOracleCase {
            name: "visible_package_group_subtree_accepts_descendant",
            case: QueryOracleCase {
                args: &["visible(//viewer/sub:caller, //owner:subtree_only)"],
                expected_labels: &["//owner:subtree_only"],
                error_fragments: &[],
            },
        },
        NamedQueryOracleCase {
            name: "visible_local_negative_separate_include_and_direct_reallow",
            case: QueryOracleCase {
                args: &[
                    "visible(//viewer/blocked/reallowed:caller, set(//owner:base_only //owner:top_reallow //owner:direct_reallow))",
                ],
                expected_labels: &["//owner:direct_reallow", "//owner:top_reallow"],
                error_fragments: &[],
            },
        },
        NamedQueryOracleCase {
            name: "visible_local_negative_blocks_without_matching_reallow",
            case: QueryOracleCase {
                args: &[
                    "visible(//viewer/blocked:caller, set(//owner:base_only //owner:top_reallow //owner:direct_reallow))",
                ],
                expected_labels: &["//owner:direct_reallow"],
                error_fragments: &[],
            },
        },
        NamedQueryOracleCase {
            name: "visible_package_group_include_cycle_terminates",
            case: QueryOracleCase {
                args: &["visible(//viewer:caller, //owner:cycle_visible)"],
                expected_labels: &["//owner:cycle_visible"],
                error_fragments: &[],
            },
        },
        NamedQueryOracleCase {
            name: "visible_matching_javatests_caller_sees_private_java",
            case: QueryOracleCase {
                args: &["visible(//javatests/acme:caller, //java/acme:private)"],
                expected_labels: &["//java/acme:private"],
                error_fragments: &[],
            },
        },
        NamedQueryOracleCase {
            name: "visible_private_java_caller_cannot_see_javatests",
            case: QueryOracleCase {
                args: &["visible(//java/acme:caller, //javatests/acme:private)"],
                expected_labels: &[],
                error_fragments: &[],
            },
        },
        NamedQueryOracleCase {
            name: "visible_mismatching_javatests_suffix_is_rejected",
            case: QueryOracleCase {
                args: &["visible(//javatests/other:caller, //java/acme:private)"],
                expected_labels: &[],
                error_fragments: &[],
            },
        },
        NamedQueryOracleCase {
            name: "visible_generated_source_build_and_package_group_kinds",
            case: QueryOracleCase {
                args: &[
                    "visible(//viewer:caller, set(//artifacts:exported.txt //artifacts:implicit.txt //artifacts:public.out //artifacts:private.out //artifacts:BUILD.bazel //build_public:BUILD.bazel //artifacts:friends))",
                ],
                expected_labels: &[
                    "//artifacts:exported.txt",
                    "//artifacts:friends",
                    "//artifacts:public.out",
                    "//build_public:BUILD.bazel",
                ],
                error_fragments: &[],
            },
        },
        NamedQueryOracleCase {
            name: "visible_fake_load_target_is_public",
            case: QueryOracleCase {
                args: &["visible(//viewer:caller, loadfiles(//loads:consumer))"],
                expected_labels: &["//loads:defs.bzl"],
                error_fragments: &[],
            },
        },
        NamedQueryOracleCase {
            name: "visible_real_same_label_load_source_uses_real_visibility",
            case: QueryOracleCase {
                args: &["visible(//viewer:caller, //loads:defs.bzl)"],
                expected_labels: &[],
                error_fragments: &[],
            },
        },
        NamedQueryOracleCase {
            name: "visible_wrong_kind_group_label_and_include_are_ignored",
            case: QueryOracleCase {
                args: &[
                    "visible(//viewer:caller, set(//owner:wrong_direct //owner:wrong_include_target))",
                ],
                expected_labels: &[],
                error_fragments: &[],
            },
        },
        NamedQueryOracleCase {
            name: "visible_missing_top_level_group_diagnostic",
            case: QueryOracleCase {
                args: &["visible(//viewer:caller, //errors/missing_top:target)"],
                expected_labels: &[],
                error_fragments: &[
                    "Invalid visibility label '//errors/missing_top:does_not_exist': no such target '//errors/missing_top:does_not_exist'",
                    "target 'does_not_exist' not declared",
                ],
            },
        },
        NamedQueryOracleCase {
            name: "visible_missing_included_group_diagnostic",
            case: QueryOracleCase {
                args: &["visible(//viewer:caller, //errors/missing_include:target)"],
                expected_labels: &[],
                error_fragments: &[
                    "Invalid visibility label '//errors/missing_include:top': no such target '//errors/missing_include:does_not_exist'",
                    "target 'does_not_exist' not declared",
                ],
            },
        },
        NamedQueryOracleCase {
            name: "visible_config_setting_omitted_default_public_and_explicit_group_positive",
            case: QueryOracleCase {
                args: &[
                    "visible(//viewer:caller, set(//owner:config_omitted //owner:config_exact))",
                ],
                expected_labels: &["//owner:config_exact", "//owner:config_omitted"],
                error_fragments: &[],
            },
        },
        NamedQueryOracleCase {
            name: "visible_config_setting_explicit_group_restriction_rejects_other",
            case: QueryOracleCase {
                args: &[
                    "visible(//other:caller, set(//owner:config_omitted //owner:config_exact))",
                ],
                expected_labels: &["//owner:config_omitted"],
                error_fragments: &[],
            },
        },
        NamedQueryOracleCase {
            name: "visible_cross_package_top_and_included_groups",
            case: QueryOracleCase {
                args: &["visible(//viewer:caller, //cross_target:visible)"],
                expected_labels: &["//cross_target:visible"],
                error_fragments: &[],
            },
        },
        NamedQueryOracleCase {
            name: "visible_real_first_same_label_fake_input_survives",
            case: QueryOracleCase {
                args: &[
                    "visible(//viewer:caller, //loads:defs.bzl union loadfiles(//loads:consumer))",
                ],
                expected_labels: &["//loads:defs.bzl"],
                error_fragments: &[],
            },
        },
        NamedQueryOracleCase {
            name: "visible_same_label_fake_callers_materialize_by_label",
            case: QueryOracleCase {
                args: &[
                    "visible(loadfiles(//consumer_a:consumer) union loadfiles(//consumer_b:consumer), //consumer_a:restricted)",
                ],
                expected_labels: &["//consumer_a:restricted"],
                error_fragments: &[],
            },
        },
    ]
}

fn assert_visible_oracle_case(
    workspace: &std::path::Path,
    output_base_arg: Option<&str>,
    named: &NamedQueryOracleCase,
) {
    let mut command = slug();
    command.current_dir(workspace);
    if let Some(output_base_arg) = output_base_arg {
        command.arg(output_base_arg);
    }
    let output = command.arg("query").args(named.case.args).output().unwrap();
    if named.case.error_fragments.is_empty() {
        assert!(output.status.success(), "{}: {output:?}", named.name);
        assert!(output.stderr.is_empty(), "{}: {output:?}", named.name);
        let expected = named.case.expected_labels.join("\n")
            + if named.case.expected_labels.is_empty() {
                ""
            } else {
                "\n"
            };
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            expected,
            "{}",
            named.name
        );
    } else {
        assert_eq!(output.status.code(), Some(7), "{}: {output:?}", named.name);
        assert!(output.stdout.is_empty(), "{}: {output:?}", named.name);
        let stderr = String::from_utf8(output.stderr).unwrap();
        for fragment in named.case.error_fragments {
            assert!(stderr.contains(fragment), "{}: {stderr}", named.name);
        }
    }
}

fn tests_function_oracle_cases() -> [QueryOracleCase; 21] {
    [
        QueryOracleCase {
            args: &["tests(set(//direct:direct_test //direct:plain))"],
            expected_labels: &["//direct:direct_test"],
            error_fragments: &[],
        },
        QueryOracleCase {
            args: &["tests(//implicit:empty)"],
            expected_labels: &["//implicit:alpha_test", "//implicit:large_test"],
            error_fragments: &[],
        },
        QueryOracleCase {
            args: &["tests(//explicit:root_suite)"],
            expected_labels: &[
                "//cross:cross_test",
                "//explicit:direct_test",
                "//explicit:nested_test",
            ],
            error_fragments: &[],
        },
        QueryOracleCase {
            args: &["tests(//explicit:only_direct)"],
            expected_labels: &["//explicit:direct_test"],
            error_fragments: &[],
        },
        QueryOracleCase {
            args: &["tests(//cycle_a:a)"],
            expected_labels: &[],
            error_fragments: &[],
        },
        QueryOracleCase {
            args: &["tests(//dedup:root)"],
            expected_labels: &["//dedup:shared_test"],
            error_fragments: &[],
        },
        QueryOracleCase {
            args: &["tests(//filters:bare)"],
            expected_labels: &["//filters:fast_test"],
            error_fragments: &[],
        },
        QueryOracleCase {
            args: &["tests(//filters:plus)"],
            expected_labels: &["//filters:fast_test"],
            error_fragments: &[],
        },
        QueryOracleCase {
            args: &["tests(//filters:exclude_slow)"],
            expected_labels: &["//filters:fast_test"],
            error_fragments: &[],
        },
        QueryOracleCase {
            args: &["tests(//filters:manual_suite)"],
            expected_labels: &["//filters:plain_test"],
            error_fragments: &[],
        },
        QueryOracleCase {
            args: &["tests(//filters:large)"],
            expected_labels: &["//filters:large_test"],
            error_fragments: &[],
        },
        QueryOracleCase {
            args: &["tests(//strict:non_test_member)"],
            expected_labels: &[],
            error_fragments: &[],
        },
        QueryOracleCase {
            args: &["--strict_test_suite", "tests(//strict:non_test_member)"],
            expected_labels: &[],
            error_fragments: &[
                "The label '//strict:plain' in the test_suite '//strict:non_test_member' does not refer to a test or test_suite rule!",
            ],
        },
        QueryOracleCase {
            args: &["tests(//missing:broken)"],
            expected_labels: &[],
            error_fragments: &[
                "couldn't expand 'tests' attribute of test_suite //missing:broken:",
                "no such target '//missing_target:missing_target'",
            ],
        },
        QueryOracleCase {
            args: &["tests(//explicit:root_suite)"],
            expected_labels: &[
                "//cross:cross_test",
                "//explicit:direct_test",
                "//explicit:nested_test",
            ],
            error_fragments: &[],
        },
        QueryOracleCase {
            args: &["--order_output=full", "tests(//explicit:root_suite)"],
            expected_labels: &[
                "//cross:cross_test",
                "//explicit:direct_test",
                "//explicit:nested_test",
            ],
            error_fragments: &[],
        },
        QueryOracleCase {
            args: &["tests(//provenance:omitted)"],
            expected_labels: &["//provenance:member_test"],
            error_fragments: &[],
        },
        QueryOracleCase {
            args: &["tests(//provenance:explicit_empty)"],
            expected_labels: &["//provenance:member_test"],
            error_fragments: &[],
        },
        QueryOracleCase {
            args: &["tests(//source_critical:parent_requires_parent_tag)"],
            expected_labels: &["//source_critical:nested_unfiltered_test"],
            error_fragments: &[],
        },
        QueryOracleCase {
            args: &["tests(//source_critical:filtered_direct_then_nested)"],
            expected_labels: &["//source_critical:shared_blocked_test"],
            error_fragments: &[],
        },
        QueryOracleCase {
            args: &["tests(//source_critical:exclude_literal_plus_tag)"],
            expected_labels: &["//source_critical:plain_tag_test"],
            error_fragments: &[],
        },
    ]
}

fn non_function_tests_oracle_cases() -> [QueryOracleCase; 6] {
    [
        QueryOracleCase {
            args: &["labels(tests, //bare_members:suite)"],
            expected_labels: &["//bare_members:a.txt", "//bare_members:dir/b.txt"],
            error_fragments: &[],
        },
        QueryOracleCase {
            args: &["deps(//bare_members:suite)"],
            expected_labels: &[
                "//bare_members:a.txt",
                "//bare_members:dir/b.txt",
                "//bare_members:suite",
            ],
            error_fragments: &[],
        },
        QueryOracleCase {
            args: &["labels($implicit_tests, //provenance:omitted)"],
            expected_labels: &["//provenance:member_test"],
            error_fragments: &[],
        },
        QueryOracleCase {
            args: &["labels($implicit_tests, //provenance:explicit_empty)"],
            expected_labels: &["//provenance:member_test"],
            error_fragments: &[],
        },
        QueryOracleCase {
            args: &["labels(tests, set(//provenance:omitted //provenance:explicit_empty))"],
            expected_labels: &[],
            error_fragments: &[],
        },
        QueryOracleCase {
            args: &["//duplicate_labels:duplicate"],
            expected_labels: &[],
            error_fragments: &[
                "Label '//duplicate_labels:member' is duplicated in the 'tests' attribute of rule 'duplicate'",
                "Evaluation of query",
            ],
        },
    ]
}

fn assert_query_oracle_case(
    workspace: &std::path::Path,
    output_base_arg: Option<&str>,
    case: QueryOracleCase,
) {
    let mut command = slug();
    command.current_dir(workspace);
    if let Some(output_base_arg) = output_base_arg {
        command.arg(output_base_arg);
    }
    let output = command.arg("query").args(case.args).output().unwrap();
    if case.error_fragments.is_empty() {
        assert!(output.status.success(), "{:?}: {output:?}", case.args);
        assert!(output.stderr.is_empty(), "{:?}: {output:?}", case.args);
        let stdout = String::from_utf8(output.stdout).unwrap();
        let mut actual = stdout.lines().collect::<Vec<_>>();
        actual.sort_unstable();
        let mut expected = case.expected_labels.to_vec();
        expected.sort_unstable();
        assert_eq!(actual, expected, "{:?}", case.args);
    } else {
        assert_eq!(output.status.code(), Some(7), "{:?}: {output:?}", case.args);
        assert!(output.stdout.is_empty(), "{:?}: {output:?}", case.args);
        let stderr = String::from_utf8(output.stderr).unwrap();
        for fragment in case.error_fragments {
            assert!(stderr.contains(fragment), "{:?}: {stderr}", case.args);
        }
    }
}

struct DaemonCleanup(std::path::PathBuf);

impl Drop for DaemonCleanup {
    fn drop(&mut self) {
        let socket = slug_server_v2::socket_path(&self.0);
        if socket.exists() {
            let _ = slug_server_v2::send_shutdown(&socket);
        }
    }
}

#[test]
fn version_reports_v2_bazel_floor() {
    let output = slug().arg("version").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Slug V2"));
    assert!(stdout.contains("Bazel compatibility: >=9.0.0"));
}

#[test]
fn help_is_bazel_v2_specific() {
    let output = slug().arg("help").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let lower = stdout.to_lowercase();
    assert!(stdout.contains("build"));
    assert!(stdout.contains("query"));
    assert!(stdout.contains("cquery"));
    assert!(stdout.contains("aquery"));
    assert!(!lower.contains(concat!("bu", "ck")));
    assert!(!stdout.contains(concat!("BU", "CK")));
    assert!(!stdout.contains(concat!("TAR", "GETS")));
    assert!(!lower.contains("cell"));
    assert!(!lower.contains(concat!(".", "bu", "ck", "config")));
}

#[test]
fn simple_rule_fixture_enters_the_dice_starlark_runtime_before_analysis() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/v2_oracle/fixtures/simple-rule-action/workspace");

    let output = slug()
        .current_dir(workspace)
        .args(["build", "//pkg:write_file", "--unknown_flag"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("\"error\":\"analysis_not_implemented\""));
    assert!(stderr.contains("\"command\":\"build\""));
    assert!(stderr.contains("\"target_count\":1"));
    assert!(stderr.contains("//pkg:write_file"));
    assert!(stderr.contains("--unknown_flag"));
    assert!(stderr.contains("\"loaded_package_count\":1"));
    assert!(stderr.contains("\"analyzed_target_count\":1"));
    assert!(stderr.contains("\"declared_action_count\":1"));
    assert!(stderr.contains("\"completed_boundary\":\"dice_starlark_rule_analysis\""));
}

#[test]
fn build_file_loading_fixture_reaches_package_loading_before_analysis() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/v2_oracle/fixtures/build-file-loading/workspace");

    let output = slug()
        .current_dir(workspace)
        .args(["build", "//pkg:fg"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("\"error\":\"analysis_not_implemented\""));
    assert!(stderr.contains("\"loaded_package_count\":1"));
    assert!(stderr.contains("\"completed_boundary\":\"dice_starlark_package_loading\""));
}

#[test]
fn typed_build_activation_publishes_cold_events_without_warm_daemon_replay() {
    let workspace = scratch("typed-build-activation");
    let package = workspace.join("pkg");
    write(
        workspace.join("MODULE.bazel"),
        "print(\"MODULE_EVENT\")\nmodule(name = \"typed_build\")\n",
    );
    write(
        package.join("defs.bzl"),
        "print(\"BZL_EVENT\")\ndef _impl(ctx):\n    print(\"ANALYSIS_EVENT\")\n    return [DefaultInfo(files = depset([]))]\nprobe = rule(implementation = _impl)\n",
    );
    write(
        package.join("BUILD.bazel"),
        "load(\":defs.bzl\", \"probe\")\nprint(\"BUILD_EVENT\")\nprobe(name = \"probe\")\n",
    );
    let workspace = workspace.canonicalize().unwrap();
    let event_stderr = format!(
        "DEBUG: {}:1:6: MODULE_EVENT\nDEBUG: {}:1:6: BZL_EVENT\nDEBUG: {}:2:6: BUILD_EVENT\nDEBUG: {}:3:10: ANALYSIS_EVENT\n",
        workspace.join("MODULE.bazel").display(),
        package.join("defs.bzl").display(),
        package.join("BUILD.bazel").display(),
        package.join("defs.bzl").display(),
    );

    let one_shot = slug()
        .current_dir(&workspace)
        .args(["build", "//pkg:probe"])
        .output()
        .unwrap();
    assert_eq!(one_shot.status.code(), Some(2), "{one_shot:?}");
    assert!(one_shot.stdout.is_empty());
    let stderr = String::from_utf8(one_shot.stderr).unwrap();
    assert!(stderr.starts_with(&event_stderr), "{stderr:?}");
    assert!(stderr.ends_with('\n'));
    assert!(stderr.contains("\"loaded_package_count\":1"));
    assert!(stderr.contains("\"analyzed_target_count\":1"));
    assert!(stderr.contains("\"runtime_mode\":\"one-shot\""));

    let output_base = scratch("typed-build-activation-output-base");
    let _cleanup = DaemonCleanup(output_base.clone());
    let output_base_arg = format!("--output_base={}", output_base.display());
    for index in 0..2 {
        let daemon = slug()
            .current_dir(&workspace)
            .args([output_base_arg.as_str(), "build", "//pkg:probe"])
            .output()
            .unwrap();
        assert_eq!(daemon.status.code(), Some(2), "{daemon:?}");
        assert!(daemon.stdout.is_empty());
        let stderr = String::from_utf8(daemon.stderr).unwrap();
        if index == 0 {
            assert!(stderr.starts_with(&event_stderr), "{stderr:?}");
        } else {
            assert!(!stderr.contains("DEBUG:"), "{stderr:?}");
        }
        assert!(stderr.ends_with('\n'));
        assert!(stderr.contains("\"analyzed_target_count\":1"));
        assert!(stderr.contains("\"runtime_mode\":\"daemon\""));
        assert!(stderr.contains("\"invalidated_files\":0"));
    }

    let missing = slug()
        .current_dir(&workspace)
        .args(["build", "//pkg:missing"])
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(2), "{missing:?}");
    assert!(missing.stdout.is_empty());
    let stderr = String::from_utf8(missing.stderr).unwrap();
    assert!(stderr.starts_with(&event_stderr[..event_stderr.rfind("DEBUG:").unwrap()]));
    assert!(stderr.contains("target `//pkg:missing` was not found in"));
    assert!(stderr.ends_with('\n'));
}

#[test]
fn typed_build_activation_preserves_native_reapi_projector_for_no_action_terminal() {
    let workspace = scratch("typed-build-reapi-projector");
    write(
        workspace.join("MODULE.bazel"),
        "module(name = \"typed_build\")\n",
    );
    write(
        workspace.join("pkg/BUILD.bazel"),
        "filegroup(name = \"probe\")\n",
    );
    let output_base = scratch("typed-build-reapi-projector-output-base");
    let _cleanup = DaemonCleanup(output_base.clone());
    let output_base_arg = format!("--output_base={}", output_base.display());

    for args in [
        vec![
            "build",
            "//pkg:probe",
            "--remote_executor=grpc://127.0.0.1:1",
        ],
        vec![
            output_base_arg.as_str(),
            "build",
            "//pkg:probe",
            "--remote_executor=grpc://127.0.0.1:1",
        ],
    ] {
        let output = slug().current_dir(&workspace).args(args).output().unwrap();
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains("\"message\":\"no executable actions were declared\""),
            "{stderr:?}"
        );
        assert!(stderr.ends_with('\n'));
        assert_eq!(stderr.lines().count(), 1, "{stderr:?}");
    }
}

#[test]
fn missing_build_target_is_structured_parse_error() {
    let output = slug().arg("build").output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("\"error\":\"command_parse_error\""));
    assert!(stderr.contains("\"command\":\"build\""));
    assert!(stderr.contains("build requires a target pattern"));
}

#[test]
fn planned_configured_and_action_queries_are_structured() {
    for command in ["cquery", "aquery"] {
        let output = slug().args([command, "//:x"]).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("\"error\":\"planned_placeholder\""));
        assert!(stderr.contains(&format!("\"command\":\"{command}\"")));
        assert!(stderr.contains("//:x"));
    }
}

#[test]
fn query_prints_text_labels_in_auto_and_full_order() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/v2_oracle/fixtures/query-parser-and-sets/workspace");
    let auto = slug()
        .current_dir(&workspace)
        .args(["query", "deps(//pkg:bin)"])
        .output()
        .unwrap();
    assert!(auto.status.success(), "{auto:?}");
    assert_eq!(
        String::from_utf8(auto.stdout).unwrap(),
        "//pkg:bin\n//pkg:data.txt\n//pkg:lib\n"
    );

    let full = slug()
        .current_dir(&workspace)
        .args(["query", "--order_output=full", "deps(//pkg:bin)"])
        .output()
        .unwrap();
    assert!(full.status.success(), "{full:?}");
    assert_eq!(
        String::from_utf8(full.stdout).unwrap(),
        "//pkg:bin\n//pkg:lib\n//pkg:data.txt\n"
    );
}

#[test]
fn typed_query_activation_publishes_cold_events_without_warm_daemon_replay() {
    let workspace = scratch("typed-query-activation");
    let package = workspace.join("pkg");
    write(
        workspace.join("MODULE.bazel"),
        "print(\"MODULE_EVENT\")\nmodule(name = \"typed_query\")\n",
    );
    write(
        package.join("defs.bzl"),
        "print(\"BZL_EVENT\")\nNAME = \"probe\"\n",
    );
    write(
        package.join("BUILD.bazel"),
        "load(\":defs.bzl\", \"NAME\")\nprint(\"BUILD_EVENT\")\nfilegroup(name = NAME)\n",
    );
    let workspace = workspace.canonicalize().unwrap();
    let event_stderr = format!(
        "DEBUG: {}:1:6: MODULE_EVENT\nDEBUG: {}:1:6: BZL_EVENT\nDEBUG: {}:2:6: BUILD_EVENT\n",
        workspace.join("MODULE.bazel").display(),
        package.join("defs.bzl").display(),
        package.join("BUILD.bazel").display(),
    );

    let one_shot = slug()
        .current_dir(&workspace)
        .args(["query", "//pkg:probe"])
        .output()
        .unwrap();
    assert!(one_shot.status.success(), "{one_shot:?}");
    assert_eq!(one_shot.stdout, b"//pkg:probe\n");
    assert_eq!(String::from_utf8(one_shot.stderr).unwrap(), event_stderr,);

    let output_base = scratch("typed-query-activation-output-base");
    let _cleanup = DaemonCleanup(output_base.clone());
    let output_base_arg = format!("--output_base={}", output_base.display());
    for index in 0..2 {
        let daemon = slug()
            .current_dir(&workspace)
            .args(["query", &output_base_arg, "deps(//pkg:probe)"])
            .output()
            .unwrap();
        assert!(daemon.status.success(), "{daemon:?}");
        assert_eq!(daemon.stdout, b"//pkg:probe\n");
        let stderr = String::from_utf8(daemon.stderr).unwrap();
        if index == 0 {
            assert_eq!(stderr, event_stderr);
        } else {
            assert!(stderr.is_empty(), "{stderr:?}");
        }
    }

    let missing = slug()
        .current_dir(&workspace)
        .args(["query", "//pkg:missing"])
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(7), "{missing:?}");
    assert!(missing.stdout.is_empty());
    let missing_stderr = String::from_utf8(missing.stderr).unwrap();
    assert!(
        missing_stderr.starts_with(&event_stderr),
        "{missing_stderr:?}"
    );
    assert_eq!(
        missing_stderr,
        format!(
            "{event_stderr}{{\"error\":\"query_error\",\"command\":\"query\",\"message\":\"no such target '//pkg:missing': target 'missing' not declared in package 'pkg'\",\"runtime_mode\":\"one-shot\"}}\n"
        )
    );

    let syntax = slug()
        .current_dir(&workspace)
        .args(["query", "deps("])
        .output()
        .unwrap();
    assert_eq!(syntax.status.code(), Some(2), "{syntax:?}");
    assert!(syntax.stdout.is_empty());
    let syntax_stderr = String::from_utf8(syntax.stderr).unwrap();
    assert!(!syntax_stderr.contains("DEBUG:"), "{syntax_stderr:?}");
    assert!(syntax_stderr.ends_with('\n'));
    assert_eq!(syntax_stderr.lines().count(), 1, "{syntax_stderr:?}");
}

#[test]
fn tests_query_expansion_fixture_matches_exact_twenty_seven_non_build_rows_one_shot() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/v2_oracle/fixtures/tests-query-expansion/workspace");
    let function_cases = tests_function_oracle_cases();
    let non_function_cases = non_function_tests_oracle_cases();
    assert_eq!(function_cases.len() + non_function_cases.len(), 27);
    for case in function_cases.into_iter().chain(non_function_cases) {
        assert_query_oracle_case(&workspace, None, case);
    }
}

#[test]
fn visibility_stage4_fixture_matches_exact_twelve_non_visible_rows() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/v2_oracle/fixtures/query-visible-visibility/workspace");
    let cases = visibility_stage4_oracle_cases();
    assert_eq!(cases.len(), 12);
    for named in cases {
        assert!(!named.name.starts_with("visible_"), "{}", named.name);
        assert_query_oracle_case(&workspace, None, named.case);
    }
}

#[test]
fn visible_fixture_matches_exact_twenty_five_rows_one_shot() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/v2_oracle/fixtures/query-visible-visibility/workspace");
    let cases = visible_oracle_cases();
    assert_eq!(cases.len(), 25);
    for named in &cases {
        assert!(named.name.starts_with("visible_"), "{}", named.name);
        assert_visible_oracle_case(&workspace, None, named);
    }
}

#[test]
fn visible_fixture_matches_exact_twenty_five_rows_through_one_daemon() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/v2_oracle/fixtures/query-visible-visibility/workspace");
    let output_base = scratch("visible-query-output-base");
    let _cleanup = DaemonCleanup(output_base.clone());
    let output_base_arg = format!("--output_base={}", output_base.display());
    let cases = visible_oracle_cases();
    assert_eq!(cases.len(), 25);
    let mut daemon_pid = None;
    for named in &cases {
        assert_visible_oracle_case(&workspace, Some(&output_base_arg), named);
        let pid = std::fs::read_to_string(slug_server_v2::pid_path(&output_base)).unwrap();
        if let Some(previous) = &daemon_pid {
            assert_eq!(&pid, previous, "{}", named.name);
        } else {
            daemon_pid = Some(pid);
        }
    }
}

#[test]
fn tests_query_expansion_fixture_matches_all_twenty_one_function_rows_through_daemon() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/v2_oracle/fixtures/tests-query-expansion/workspace");
    let output_base = scratch("tests-query-output-base");
    let _cleanup = DaemonCleanup(output_base.clone());
    let output_base_arg = format!("--output_base={}", output_base.display());
    let cases = tests_function_oracle_cases();
    assert_eq!(cases.len(), 21);
    for case in cases {
        assert_query_oracle_case(&workspace, Some(&output_base_arg), case);
    }
    assert!(slug_server_v2::pid_path(&output_base).is_file());
}

#[test]
fn build_load_files_provenance_fixture_matches_all_fifty_seven_text_bazel_rows_through_cli() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/v2_oracle/fixtures/query-build-load-files-provenance/workspace");
    let successes: &[(&str, &[&str], &str)] = &[
        (
            "buildfiles_direct_primary",
            &["query", "buildfiles(//a:one)"],
            "//a:BUILD.bazel\n//shared:BUILD.bazel\n//shared:one.bzl\n//shared:two.bzl\n",
        ),
        (
            "loadfiles_direct_primary",
            &["query", "loadfiles(//a:one)"],
            "//shared:one.bzl\n//shared:two.bzl\n",
        ),
        (
            "siblings_fake_a",
            &["query", "siblings(loadfiles(//a:one))"],
            "//a:BUILD.bazel\n//a:one\n",
        ),
        (
            "siblings_fake_b",
            &["query", "siblings(loadfiles(//b:two))"],
            "//b:BUILD.bazel\n//b:two\n",
        ),
        (
            "siblings_two_consumers_ab",
            &[
                "query",
                "siblings(loadfiles(//a:one) union loadfiles(//b:two))",
            ],
            "//a:BUILD.bazel\n//a:one\n//b:BUILD.bazel\n//b:two\n",
        ),
        (
            "siblings_two_consumers_ba",
            &[
                "query",
                "siblings(loadfiles(//b:two) union loadfiles(//a:one))",
            ],
            "//a:BUILD.bazel\n//a:one\n//b:BUILD.bazel\n//b:two\n",
        ),
        (
            "siblings_real_fake_order",
            &["query", "siblings(//a:one union loadfiles(//b:two))"],
            "//a:BUILD.bazel\n//a:one\n//b:BUILD.bazel\n//b:two\n",
        ),
        (
            "siblings_fake_real_order",
            &["query", "siblings(loadfiles(//b:two) union //a:one)"],
            "//a:BUILD.bazel\n//a:one\n//b:BUILD.bazel\n//b:two\n",
        ),
        (
            "real_exported_bzl_siblings",
            &["query", "siblings(//shared:one.bzl)"],
            "//shared:BUILD.bazel\n//shared:one.bzl\n//shared:shared_rule\n//shared:two.bzl\n",
        ),
        (
            "siblings_real_fake_same_label",
            &[
                "query",
                "siblings(//shared:one.bzl union loadfiles(//a:one))",
            ],
            "//a:BUILD.bazel\n//a:one\n//shared:BUILD.bazel\n//shared:one.bzl\n//shared:shared_rule\n//shared:two.bzl\n",
        ),
        (
            "siblings_fake_real_same_label",
            &[
                "query",
                "siblings(loadfiles(//a:one) union //shared:one.bzl)",
            ],
            "//a:BUILD.bazel\n//a:one\n//shared:BUILD.bazel\n//shared:one.bzl\n//shared:shared_rule\n//shared:two.bzl\n",
        ),
        (
            "siblings_buildfiles_companions",
            &["query", "siblings(buildfiles(//a:one))"],
            "//a:BUILD.bazel\n//a:one\n",
        ),
        (
            "buildfiles_diamond",
            &["query", "buildfiles(//diamond:probe)"],
            "//diamond:BUILD.bazel\n//diamond:leaf.bzl\n//diamond:left.bzl\n//diamond:right.bzl\n",
        ),
        (
            "loadfiles_diamond",
            &["query", "loadfiles(//diamond:probe)"],
            "//diamond:leaf.bzl\n//diamond:left.bzl\n//diamond:right.bzl\n",
        ),
        (
            "buildfiles_fallback",
            &["query", "buildfiles(//fallback:only)"],
            "//fallback:BUILD\n//shared:BUILD.bazel\n//shared:one.bzl\n//shared:two.bzl\n",
        ),
        (
            "loadfiles_fallback",
            &["query", "loadfiles(//fallback:only)"],
            "//shared:one.bzl\n//shared:two.bzl\n",
        ),
        (
            "buildfiles_dual_primary",
            &["query", "buildfiles(//dual:preferred)"],
            "//dual:BUILD.bazel\n//shared:BUILD.bazel\n//shared:one.bzl\n//shared:two.bzl\n",
        ),
        (
            "buildfiles_root",
            &["query", "buildfiles(//:root)"],
            "//:BUILD.bazel\n",
        ),
        (
            "buildfiles_multiple_packages",
            &["query", "buildfiles(set(//a:one //fallback:only))"],
            "//a:BUILD.bazel\n//fallback:BUILD\n//shared:BUILD.bazel\n//shared:one.bzl\n//shared:two.bzl\n",
        ),
        (
            "loadfiles_multiple_packages",
            &["query", "loadfiles(set(//a:one //fallback:only))"],
            "//shared:one.bzl\n//shared:two.bzl\n",
        ),
        (
            "buildfiles_duplicate_operand",
            &["query", "buildfiles(set(//a:one //a:one))"],
            "//a:BUILD.bazel\n//shared:BUILD.bazel\n//shared:one.bzl\n//shared:two.bzl\n",
        ),
        ("loadfiles_empty", &["query", "loadfiles(set())"], ""),
        ("buildfiles_empty", &["query", "buildfiles(set())"], ""),
        (
            "buildfiles_idempotent",
            &["query", "buildfiles(buildfiles(//a:one))"],
            "//a:BUILD.bazel\n//shared:BUILD.bazel\n//shared:one.bzl\n//shared:two.bzl\n",
        ),
        (
            "buildfiles_union",
            &["query", "buildfiles(//a:one) union //fallback:only"],
            "//a:BUILD.bazel\n//fallback:only\n//shared:BUILD.bazel\n//shared:one.bzl\n//shared:two.bzl\n",
        ),
        (
            "buildfiles_intersect",
            &[
                "query",
                "buildfiles(//a:one) intersect set(//a:BUILD.bazel //shared:one.bzl //fallback:only)",
            ],
            "//a:BUILD.bazel\n//shared:one.bzl\n",
        ),
        (
            "loadfiles_except",
            &["query", "loadfiles(//a:one) except //shared:two.bzl"],
            "//shared:one.bzl\n",
        ),
        (
            "deps_function_fake_nodes",
            &["query", "deps(loadfiles(//a:one))"],
            "//shared:one.bzl\n//shared:two.bzl\n",
        ),
        (
            "deps_buildfiles_fake_nodes",
            &["query", "deps(buildfiles(//a:one))"],
            "//a:BUILD.bazel\n//shared:BUILD.bazel\n//shared:one.bzl\n//shared:two.bzl\n",
        ),
        (
            "default_direct",
            &["query", "buildfiles(//a:one)"],
            "//a:BUILD.bazel\n//shared:BUILD.bazel\n//shared:one.bzl\n//shared:two.bzl\n",
        ),
        (
            "auto_direct",
            &["query", "--order_output=auto", "buildfiles(//a:one)"],
            "//a:BUILD.bazel\n//shared:BUILD.bazel\n//shared:one.bzl\n//shared:two.bzl\n",
        ),
        (
            "full_direct",
            &["query", "--order_output=full", "buildfiles(//a:one)"],
            "//shared:two.bzl\n//shared:one.bzl\n//shared:BUILD.bazel\n",
        ),
        (
            "full_wrapped_union",
            &[
                "query",
                "--order_output=full",
                "buildfiles(//a:one) union set()",
            ],
            "//shared:two.bzl\n//shared:one.bzl\n//shared:BUILD.bazel\n",
        ),
        (
            "buildfiles_broken_companions",
            &["query", "buildfiles(//consumer_bad:probe)"],
            "//broken_load:BUILD.bazel\n//broken_load:good.bzl\n//broken_syntax:BUILD.bazel\n//broken_syntax:good.bzl\n//consumer_bad:BUILD.bazel\n",
        ),
        (
            "loadfiles_broken_companions",
            &["query", "loadfiles(//consumer_bad:probe)"],
            "//broken_load:good.bzl\n//broken_syntax:good.bzl\n",
        ),
        (
            "siblings_loadfiles_intersect_ab",
            &[
                "query",
                "siblings(loadfiles(//a:one) intersect loadfiles(//b:two))",
            ],
            "//a:BUILD.bazel\n//a:one\n",
        ),
        (
            "siblings_loadfiles_intersect_ba",
            &[
                "query",
                "siblings(loadfiles(//b:two) intersect loadfiles(//a:one))",
            ],
            "//b:BUILD.bazel\n//b:two\n",
        ),
        (
            "siblings_loadfiles_except_ab",
            &[
                "query",
                "siblings(loadfiles(//a:one) except loadfiles(//b:two))",
            ],
            "",
        ),
        (
            "siblings_loadfiles_except_ba",
            &[
                "query",
                "siblings(loadfiles(//b:two) except loadfiles(//a:one))",
            ],
            "",
        ),
        (
            "siblings_real_fake_intersect",
            &[
                "query",
                "siblings(//shared:one.bzl intersect loadfiles(//a:one))",
            ],
            "//shared:BUILD.bazel\n//shared:one.bzl\n//shared:shared_rule\n//shared:two.bzl\n",
        ),
        (
            "siblings_fake_real_intersect",
            &[
                "query",
                "siblings(loadfiles(//a:one) intersect //shared:one.bzl)",
            ],
            "//a:BUILD.bazel\n//a:one\n",
        ),
        (
            "siblings_real_fake_except",
            &[
                "query",
                "siblings(//shared:one.bzl except loadfiles(//a:one))",
            ],
            "",
        ),
        (
            "siblings_fake_real_except",
            &[
                "query",
                "siblings(loadfiles(//a:one) except //shared:one.bzl)",
            ],
            "//a:BUILD.bazel\n//a:one\n",
        ),
        (
            "siblings_single_fake_real_intersect",
            &[
                "query",
                "siblings(loadfiles(//single:only) intersect //shared:two.bzl)",
            ],
            "//single:BUILD.bazel\n//single:only\n",
        ),
        (
            "siblings_single_real_fake_intersect",
            &[
                "query",
                "siblings(//shared:two.bzl intersect loadfiles(//single:only))",
            ],
            "//shared:BUILD.bazel\n//shared:one.bzl\n//shared:shared_rule\n//shared:two.bzl\n",
        ),
        (
            "siblings_single_fake_real_except",
            &[
                "query",
                "siblings(loadfiles(//single:only) except //shared:two.bzl)",
            ],
            "",
        ),
        (
            "siblings_single_real_fake_except",
            &[
                "query",
                "siblings(//shared:two.bzl except loadfiles(//single:only))",
            ],
            "",
        ),
        (
            "siblings_single_fake_real_union",
            &[
                "query",
                "siblings(loadfiles(//single:only) union //shared:two.bzl)",
            ],
            "//shared:BUILD.bazel\n//shared:one.bzl\n//shared:shared_rule\n//shared:two.bzl\n//single:BUILD.bazel\n//single:only\n",
        ),
        (
            "siblings_single_real_fake_union",
            &[
                "query",
                "siblings(//shared:two.bzl union loadfiles(//single:only))",
            ],
            "//shared:BUILD.bazel\n//shared:one.bzl\n//shared:shared_rule\n//shared:two.bzl\n//single:BUILD.bazel\n//single:only\n",
        ),
    ];
    let failures: &[(&str, &[&str], i32, &str)] = &[
        (
            "missing_load_failure",
            &["query", "buildfiles(//missing_load:probe)"],
            7,
            "cannot load '//missing_load:missing.bzl': no such file",
        ),
        (
            "broken_bzl_failure",
            &["query", "loadfiles(//broken_bzl:probe)"],
            7,
            "compilation of module 'broken_bzl/bad.bzl' failed",
        ),
        (
            "bzl_cycle_failure",
            &["query", "loadfiles(//bzl_cycle:probe)"],
            7,
            "cycle detected in extension files",
        ),
        (
            "buildfiles_too_few_arguments",
            &["query", "buildfiles()"],
            2,
            "too few arguments to function 'buildfiles'",
        ),
        (
            "loadfiles_too_many_arguments",
            &["query", "loadfiles(//a:one, //b:two)"],
            2,
            "too many arguments to function 'loadfiles'",
        ),
        (
            "buildfiles_syntax_failure",
            &["query", "buildfiles("],
            2,
            "premature end of input",
        ),
        (
            "buildfiles_missing_target",
            &["query", "buildfiles(//a:missing)"],
            7,
            "no such target '//a:missing'",
        ),
        (
            "buildfiles_later_error",
            &["query", "buildfiles(//a:one union //missing:target)"],
            7,
            "no such package 'missing': BUILD file not found",
        ),
    ];
    assert_eq!(successes.len() + failures.len(), 57);

    for (name, args, expected_stdout) in successes {
        let output = slug().current_dir(&workspace).args(*args).output().unwrap();
        assert!(output.status.success(), "{name} {args:?}: {output:?}");
        assert!(output.stderr.is_empty(), "{name} {args:?}: {output:?}");
        assert_eq!(
            std::str::from_utf8(&output.stdout).unwrap(),
            *expected_stdout,
            "{name} {args:?}"
        );
    }
    for (name, args, exit, diagnostic) in failures {
        let output = slug().current_dir(&workspace).args(*args).output().unwrap();
        assert_eq!(
            output.status.code(),
            Some(*exit),
            "{name} {args:?}: {output:?}"
        );
        assert!(output.stdout.is_empty(), "{name} {args:?}: {output:?}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(diagnostic), "{name} {args:?}: {stderr}");
    }
}

#[test]
fn siblings_build_file_node_fixture_matches_all_oracle_rows() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/v2_oracle/fixtures/query-siblings-build-file-node/workspace");
    const MODERN: &str = "//modern:BUILD.bazel\n//modern:alias\n//modern:custom\n//modern:cycle_a\n//modern:cycle_b\n//modern:explicit.txt\n//modern:implicit.txt\n//modern:leaf\n//modern:rule\n";
    const MODERN_FULL: &str = "//modern:rule\n//modern:leaf\n//modern:implicit.txt\n//modern:explicit.txt\n//modern:cycle_b\n//modern:cycle_a\n//modern:custom\n//modern:alias\n//modern:BUILD.bazel\n";
    let successful = [
        (
            vec!["query", "//modern:BUILD.bazel"],
            "//modern:BUILD.bazel\n",
        ),
        (vec!["query", "//fallback:BUILD"], "//fallback:BUILD\n"),
        (
            vec!["query", "siblings(//fallback:BUILD)"],
            "//fallback:BUILD\n//fallback:only\n",
        ),
        (vec!["query", "//:BUILD.bazel"], "//:BUILD.bazel\n"),
        (
            vec!["query", "siblings(//:BUILD.bazel)"],
            "//:BUILD.bazel\n//:root_rule\n",
        ),
        (vec!["query", "//dual:preferred"], "//dual:preferred\n"),
        (vec!["query", "//dual:BUILD.bazel"], "//dual:BUILD.bazel\n"),
        (
            vec!["query", "siblings(//dual:BUILD.bazel)"],
            "//dual:BUILD.bazel\n//dual:preferred\n",
        ),
        (vec!["query", "siblings(//modern:BUILD.bazel)"], MODERN),
        (vec!["query", "siblings(//modern:rule)"], MODERN),
        (vec!["query", "siblings(//modern:explicit.txt)"], MODERN),
        (vec!["query", "siblings(//modern:implicit.txt)"], MODERN),
        (vec!["query", "siblings(//modern:alias)"], MODERN),
        (vec!["query", "siblings(//modern:custom)"], MODERN),
        (
            vec!["query", "siblings(//namedall:BUILD.bazel)"],
            "//namedall:BUILD.bazel\n//namedall:all\n//namedall:other\n",
        ),
        (
            vec![
                "query",
                "siblings(set(//modern:rule //modern:alias //modern:rule))",
            ],
            MODERN,
        ),
        (
            vec!["query", "siblings(set(//modern:rule //fallback:only))"],
            "//fallback:BUILD\n//fallback:only\n//modern:BUILD.bazel\n//modern:alias\n//modern:custom\n//modern:cycle_a\n//modern:cycle_b\n//modern:explicit.txt\n//modern:implicit.txt\n//modern:leaf\n//modern:rule\n",
        ),
        (vec!["query", "siblings(set())"], ""),
        (
            vec!["query", "siblings(//modern:rule) union //fallback:only"],
            "//fallback:only\n//modern:BUILD.bazel\n//modern:alias\n//modern:custom\n//modern:cycle_a\n//modern:cycle_b\n//modern:explicit.txt\n//modern:implicit.txt\n//modern:leaf\n//modern:rule\n",
        ),
        (
            vec!["query", "siblings(set(//modern:rule //fallback:only))"],
            "//fallback:BUILD\n//fallback:only\n//modern:BUILD.bazel\n//modern:alias\n//modern:custom\n//modern:cycle_a\n//modern:cycle_b\n//modern:explicit.txt\n//modern:implicit.txt\n//modern:leaf\n//modern:rule\n",
        ),
        (
            vec![
                "query",
                "siblings(//modern:rule) intersect set(//modern:BUILD.bazel //modern:rule //fallback:only)",
            ],
            "//modern:BUILD.bazel\n//modern:rule\n",
        ),
        (
            vec![
                "query",
                "siblings(//modern:rule) except set(//modern:BUILD.bazel //modern:implicit.txt)",
            ],
            "//modern:alias\n//modern:custom\n//modern:cycle_a\n//modern:cycle_b\n//modern:explicit.txt\n//modern:leaf\n//modern:rule\n",
        ),
        (
            vec!["query", "deps(//modern:BUILD.bazel)"],
            "//modern:BUILD.bazel\n",
        ),
        (
            vec![
                "query",
                "rdeps(siblings(//modern:rule), //modern:BUILD.bazel)",
            ],
            "//modern:BUILD.bazel\n",
        ),
        (
            vec!["query", "same_pkg_direct_rdeps(//modern:BUILD.bazel)"],
            "",
        ),
        (
            vec!["query", "--order_output=auto", "siblings(//modern:rule)"],
            MODERN,
        ),
        (
            vec!["query", "--order_output=full", "siblings(//modern:rule)"],
            MODERN_FULL,
        ),
        (
            vec!["query", "--order_output=full", "siblings(//modern:cycle_a)"],
            MODERN_FULL,
        ),
        (
            vec!["query", "--order_output=full", "siblings(//provenance:a)"],
            "//provenance:zz\n//provenance:z\n//provenance:y\n//provenance:a\n//provenance:BUILD.bazel\n",
        ),
        (
            vec![
                "query",
                "--order_output=full",
                "siblings(//provenance:a) union set()",
            ],
            "//provenance:zz\n//provenance:z\n//provenance:y\n//provenance:a\n//provenance:BUILD.bazel\n",
        ),
        (
            vec![
                "query",
                "--order_output=full",
                "siblings(deps(//provenance:a))",
            ],
            "//provenance:z\n//provenance:y\n//provenance:a\n//provenance:zz\n//provenance:BUILD.bazel\n",
        ),
    ];
    let failures = [
        (
            vec!["query", "//modern:BUILD"],
            7,
            "no such target '//modern:BUILD'",
        ),
        (
            vec!["query", "//fallback:BUILD.bazel"],
            7,
            "no such target '//fallback:BUILD.bazel'",
        ),
        (vec!["query", "//:BUILD"], 7, "no such target '//:BUILD'"),
        (
            vec!["query", "//dual:BUILD"],
            7,
            "no such target '//dual:BUILD'",
        ),
        (
            vec!["query", "//dual:ignored"],
            7,
            "no such target '//dual:ignored'",
        ),
        (
            vec!["query", "siblings()"],
            2,
            "too few arguments to function 'siblings'",
        ),
        (
            vec!["query", "siblings(//modern:rule, //fallback:only)"],
            2,
            "too many arguments to function 'siblings'",
        ),
        (vec!["query", "siblings("], 2, "premature end of input"),
        (
            vec!["query", "siblings(//modern:missing)"],
            7,
            "no such target '//modern:missing'",
        ),
        (
            vec!["query", "siblings(//missing:target)"],
            7,
            "no such package 'missing': BUILD file not found",
        ),
        (
            vec!["query", "siblings(//modern:rule union //modern:missing)"],
            7,
            "no such target '//modern:missing'",
        ),
        (
            vec!["query", "siblings(//modern:rule union //missing:target)"],
            7,
            "no such package 'missing': BUILD file not found",
        ),
    ];
    assert_eq!(successful.len() + failures.len(), 43);

    for (argv, expected_stdout) in successful {
        let output = slug().current_dir(&workspace).args(&argv).output().unwrap();
        assert!(output.status.success(), "{argv:?}: {output:?}");
        assert!(output.stderr.is_empty(), "{argv:?}: {output:?}");
        assert_eq!(
            std::str::from_utf8(&output.stdout).unwrap(),
            expected_stdout,
            "{argv:?}"
        );
    }
    for (argv, expected_exit, expected_message) in failures {
        let output = slug().current_dir(&workspace).args(&argv).output().unwrap();
        assert_eq!(output.status.code(), Some(expected_exit), "{argv:?}");
        assert!(output.stdout.is_empty(), "{argv:?}: {output:?}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(expected_message), "{argv:?}: {stderr}");
    }
}

#[test]
fn loading_query_fixture_matches_full_bazel_semantic_slice() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/v2_oracle/fixtures/query-loading-thin-vertical/workspace");
    let successful = [
        ("//app:app", "//app:app\n"),
        ("//lib:implicit.txt", "//lib:implicit.txt\n"),
        ("//lib:explicit.txt", "//lib:explicit.txt\n"),
        ("//lib:missing_input.txt", "//lib:missing_input.txt\n"),
        ("//lib:all", "//lib:leaf\n"),
        (
            "//...",
            "//:root\n//app:app\n//app:via_alias\n//cycle:a\n//cycle:b\n//lib:leaf\n//nested:branch\n//nested/child:child\n",
        ),
        (
            "deps(//app:via_alias)",
            "//app:via_alias\n//lib:explicit.txt\n//lib:implicit.txt\n//lib:leaf\n//lib:missing_input.txt\n",
        ),
        (
            "deps(//app:app)",
            "//app:app\n//app:via_alias\n//lib:explicit.txt\n//lib:implicit.txt\n//lib:leaf\n//lib:missing_input.txt\n//nested:branch\n//nested/child:child.txt\n",
        ),
        ("deps(//cycle:a)", "//cycle:a\n//cycle:b\n"),
        (
            "deps(//app:app) except //lib:explicit.txt intersect //...",
            "//app:app\n//app:via_alias\n//lib:leaf\n//nested:branch\n",
        ),
    ];
    for (expression, expected) in successful {
        let output = slug()
            .current_dir(&workspace)
            .args(["query", expression])
            .output()
            .unwrap();
        assert!(output.status.success(), "{expression}: {output:?}");
        assert_eq!(
            std::str::from_utf8(&output.stdout).unwrap(),
            expected,
            "{expression}"
        );
        assert!(output.stderr.is_empty(), "{expression}: {output:?}");
    }

    for (expression, expected) in [
        (
            "//lib:missing",
            "no such target '//lib:missing': target 'missing' not declared in package 'lib'",
        ),
        (
            "//missing:target",
            "no such package 'missing': BUILD file not found",
        ),
    ] {
        let output = slug()
            .current_dir(&workspace)
            .args(["query", expression])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(7), "{expression}: {output:?}");
        assert!(output.stdout.is_empty(), "{expression}: {output:?}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(expected), "{expression}: {stderr}");
    }
}

#[test]
fn labels_metadata_fixture_matches_twenty_nine_non_label_kind_bazel_rows_through_cli() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/v2_oracle/fixtures/query-labels-attribute-metadata/workspace");
    let successes: &[(&[&str], &str)] = &[
        (
            &["query", "labels(one, //pkg:explicit)"],
            "//pkg:source.txt\n",
        ),
        (
            &["query", "labels(many, //pkg:explicit)"],
            "//other:cross.txt\n//pkg:source.txt\n",
        ),
        (
            &["query", "labels(with_default, //pkg:omitted)"],
            "//pkg:default.txt\n",
        ),
        (&["query", "labels(many, //pkg:empty)"], ""),
        (&["query", "labels(note, //pkg:explicit)"], ""),
        (&["query", "labels(no_such_attr, //pkg:explicit)"], ""),
        (
            &["query", "labels($implicit, //pkg:implicit)"],
            "//pkg:implicit.txt\n",
        ),
        (&["query", "labels(_implicit, //pkg:implicit)"], ""),
        (
            &["query", "labels(chosen, //pkg:selecting)"],
            "//pkg:branch_arm.txt\n//pkg:branch_default.txt\n//pkg:branch_linux.txt\n",
        ),
        (
            &["query", "labels(combined, //pkg:selecting)"],
            "//pkg:branch_arm.txt\n//pkg:branch_default.txt\n//pkg:branch_linux.txt\n//pkg:shared.txt\n",
        ),
        (&["query", "labels(out, //pkg:outputs)"], "//pkg:one.out\n"),
        (
            &["query", "labels(outs, //pkg:outputs)"],
            "//pkg:three.out\n//pkg:two.out\n",
        ),
        (
            &["query", "labels(outs, //pkg:outputs_two)"],
            "//pkg:five.out\n//pkg:six.out\n",
        ),
        (
            &["query", "deps(labels(outs, //pkg:outputs))"],
            "//pkg:outputs\n//pkg:three.out\n//pkg:two.out\n",
        ),
        (
            &[
                "query",
                "deps(labels(outs, //pkg:outputs) union //pkg:outputs)",
            ],
            "//pkg:outputs\n//pkg:three.out\n//pkg:two.out\n",
        ),
        (
            &[
                "query",
                "deps(labels(outs, //pkg:outputs) union labels(outs, //pkg:outputs_two) union //pkg:outputs union //pkg:outputs_two)",
            ],
            "//pkg:five.out\n//pkg:outputs\n//pkg:outputs_two\n//pkg:six.out\n//pkg:three.out\n//pkg:two.out\n",
        ),
        (
            &["query", "labels(string_labels, //pkg:dicts)"],
            "//other:cross.txt\n//pkg:source.txt\n",
        ),
        (
            &["query", "labels(label_strings, //pkg:dicts)"],
            "//other:cross.txt\n//pkg:default.txt\n",
        ),
        (
            &["query", "labels(label_lists, //pkg:dicts)"],
            "//other:cross.txt\n//pkg:implicit.txt\n//pkg:source.txt\n",
        ),
        (
            &[
                "query",
                "labels(many, set(//pkg:source.txt //pkg:alias //pkg:BUILD.bazel))",
            ],
            "",
        ),
        (
            &[
                "query",
                "labels(many, set(//pkg:explicit //pkg:explicit //other:consumer))",
            ],
            "//other:cross.txt\n//pkg:source.txt\n",
        ),
        (
            &[
                "query",
                "labels(one, //pkg:explicit) union labels(with_default, //pkg:omitted)",
            ],
            "//pkg:default.txt\n//pkg:source.txt\n",
        ),
        (
            &[
                "query",
                "--order_output=auto",
                "labels(many, //pkg:explicit)",
            ],
            "//other:cross.txt\n//pkg:source.txt\n",
        ),
        (
            &[
                "query",
                "--order_output=full",
                "labels(many, //pkg:explicit)",
            ],
            "//pkg:source.txt\n//other:cross.txt\n",
        ),
        // The plain default-order invocation is intentionally a distinct
        // oracle row from explicit --order_output=auto.
        (
            &["query", "labels(many, //pkg:explicit)"],
            "//other:cross.txt\n//pkg:source.txt\n",
        ),
    ];
    let graph_rows: &[(&[&str], &str)] = &[
        (
            &[
                "query",
                "--output=graph",
                "--graph:factored",
                "deps(labels(outs, //pkg:outputs) union //pkg:outputs)",
            ],
            concat!(
                "digraph mygraph {\n",
                "  node [shape=box];\n",
                "  \"//pkg:three.out\\n//pkg:two.out\"\n",
                "  \"//pkg:three.out\\n//pkg:two.out\" -> \"//pkg:outputs\"\n",
                "  \"//pkg:outputs\"\n",
                "}\n",
            ),
        ),
        (
            &[
                "query",
                "--output=graph",
                "--graph:factored",
                "deps(labels(outs, //pkg:outputs) union labels(outs, //pkg:outputs_two) union //pkg:outputs union //pkg:outputs_two)",
            ],
            concat!(
                "digraph mygraph {\n",
                "  node [shape=box];\n",
                "  \"//pkg:six.out\\n//pkg:five.out\"\n",
                "  \"//pkg:six.out\\n//pkg:five.out\" -> \"//pkg:outputs_two\"\n",
                "  \"//pkg:outputs_two\"\n",
                "  \"//pkg:three.out\\n//pkg:two.out\"\n",
                "  \"//pkg:three.out\\n//pkg:two.out\" -> \"//pkg:outputs\"\n",
                "  \"//pkg:outputs\"\n",
                "}\n",
            ),
        ),
    ];
    let failures: &[(&[&str], &[&str])] = &[
        (
            &["query", "labels(many, //pkg:missing_ref)"],
            &[
                "in 'many' of rule //pkg:missing_ref: no such package 'does_not_exist'",
                "Evaluation of query",
            ],
        ),
        (
            &["query", "//broken:must"],
            &[
                "missing value for mandatory attribute 'required'",
                "Evaluation of query",
            ],
        ),
    ];
    assert_eq!(successes.len() + graph_rows.len() + failures.len(), 29);

    for (argv, expected_stdout) in successes {
        let output = slug().current_dir(&workspace).args(*argv).output().unwrap();
        assert!(output.status.success(), "{argv:?}: {output:?}");
        assert!(output.stderr.is_empty(), "{argv:?}: {output:?}");
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            *expected_stdout,
            "{argv:?}"
        );
    }
    for (argv, expected_stdout) in graph_rows {
        let output = slug().current_dir(&workspace).args(*argv).output().unwrap();
        assert!(output.status.success(), "{argv:?}: {output:?}");
        assert!(output.stderr.is_empty(), "{argv:?}: {output:?}");
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            *expected_stdout,
            "{argv:?}"
        );
    }
    for (argv, expected_fragments) in failures {
        let output = slug().current_dir(&workspace).args(*argv).output().unwrap();
        assert_eq!(output.status.code(), Some(7), "{argv:?}: {output:?}");
        assert!(output.stdout.is_empty(), "{argv:?}: {output:?}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        for fragment in *expected_fragments {
            assert!(stderr.contains(fragment), "{argv:?}: {stderr}");
        }
    }
}

#[test]
fn executables_rule_capability_fixture_matches_all_thirty_two_non_label_kind_bazel_rows() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/v2_oracle/fixtures/query-executables-rule-capability/workspace");
    let successes: &[(&[&str], &str)] = &[
        (
            &["query", "executables(//pkg:all)"],
            "//pkg:arbitrary_target\n//pkg:edge_exec\n//pkg:target_test\n",
        ),
        (&["query", "executables(//pkg:plain)"], ""),
        (&["query", "executables(//pkg:ordinary_target)"], ""),
        (&["query", "executables(//pkg:explicit_test_target)"], ""),
        (
            &["query", "executables(//pkg:target_test)"],
            "//pkg:target_test\n",
        ),
        (&["query", "executables(//pkg:data.txt)"], ""),
        (
            &[
                "query",
                "executables(set(//pkg:data.txt //pkg:generated.txt //pkg:BUILD.bazel //pkg:files //pkg:alias_exec //pkg:setting))",
            ],
            "",
        ),
        (&["query", "executables(//pkg:alias_exec)"], ""),
        (&["query", "executables(//pkg:BUILD.bazel)"], ""),
        (&["query", "executables(//pkg:generated.txt)"], ""),
        (&["query", "executables(//pkg:files)"], ""),
        (&["query", "executables(//pkg:setting)"], ""),
        (&["query", "executables(set())"], ""),
        (
            &[
                "query",
                "executables(set(//pkg:arbitrary_target //pkg:arbitrary_target //pkg:target_test))",
            ],
            "//pkg:arbitrary_target\n//pkg:target_test\n",
        ),
        (
            &[
                "query",
                "executables(//pkg:arbitrary_target) union //pkg:plain",
            ],
            "//pkg:arbitrary_target\n//pkg:plain\n",
        ),
        (
            &[
                "query",
                "executables(//pkg:all) intersect set(//pkg:target_test //pkg:plain)",
            ],
            "//pkg:target_test\n",
        ),
        (
            &["query", "executables(//pkg:all) except //pkg:target_test"],
            "//pkg:arbitrary_target\n//pkg:edge_exec\n",
        ),
        (
            &["query", "let x = //pkg:all in executables($x)"],
            "//pkg:arbitrary_target\n//pkg:edge_exec\n//pkg:target_test\n",
        ),
        (
            &["query", "executables(executables(//pkg:all))"],
            "//pkg:arbitrary_target\n//pkg:edge_exec\n//pkg:target_test\n",
        ),
        (
            &["query", "executables(//pkg:all)"],
            "//pkg:arbitrary_target\n//pkg:edge_exec\n//pkg:target_test\n",
        ),
        (
            &["query", "--order_output=auto", "executables(//pkg:all)"],
            "//pkg:arbitrary_target\n//pkg:edge_exec\n//pkg:target_test\n",
        ),
        (
            &["query", "--order_output=full", "executables(//pkg:all)"],
            "//pkg:target_test\n//pkg:edge_exec\n//pkg:arbitrary_target\n",
        ),
        (
            &["query", "deps(executables(//pkg:edge_exec))"],
            "//pkg:edge_dep\n//pkg:edge_exec\n",
        ),
        (&["query", "executables(//test_false:probe)"], ""),
    ];
    let graph_rows: &[(&[&str], &str)] = &[
        (
            &[
                "query",
                "--output=graph",
                "--graph:factored",
                "executables(//pkg:edge_exec)",
            ],
            concat!(
                "digraph mygraph {\n",
                "  node [shape=box];\n",
                "  \"//pkg:edge_exec\"\n",
                "}\n",
            ),
        ),
        (
            &[
                "query",
                "--output=graph",
                "--graph:factored",
                "deps(executables(//pkg:edge_exec))",
            ],
            concat!(
                "digraph mygraph {\n",
                "  node [shape=box];\n",
                "  \"//pkg:edge_exec\"\n",
                "  \"//pkg:edge_exec\" -> \"//pkg:edge_dep\"\n",
                "  \"//pkg:edge_dep\"\n",
                "}\n",
            ),
        ),
    ];
    let failures: &[(&[&str], i32, &str)] = &[
        (
            &["query", "executables(//missing:nope)"],
            7,
            "no such package 'missing'",
        ),
        (
            &["query", "executables()"],
            2,
            "too few arguments to function 'executables'",
        ),
        (
            &["query", "executables(//pkg:plain, //pkg:arbitrary_target)"],
            2,
            "too many arguments to function 'executables'",
        ),
        (
            &[
                "query",
                "executables(//pkg:arbitrary_target union //missing:nope)",
            ],
            7,
            "no such package 'missing'",
        ),
        (
            &["query", "//invalid_not_test:probe"],
            7,
            "Invalid rule class name 'not_test_suffix', test rule class names must end with '_test' and other rule classes must not",
        ),
        (
            &["query", "//invalid_suffix_test:probe"],
            7,
            "Invalid rule class name 'suffix_test', test rule class names must end with '_test' and other rule classes must not",
        ),
    ];
    assert_eq!(successes.len() + graph_rows.len() + failures.len(), 32);

    for (argv, expected_stdout) in successes.iter().chain(graph_rows) {
        let output = slug().current_dir(&workspace).args(*argv).output().unwrap();
        assert!(output.status.success(), "{argv:?}: {output:?}");
        assert!(output.stderr.is_empty(), "{argv:?}: {output:?}");
        assert_eq!(
            std::str::from_utf8(&output.stdout).unwrap(),
            *expected_stdout,
            "{argv:?}"
        );
    }
    for (argv, expected_exit, diagnostic) in failures {
        let output = slug().current_dir(&workspace).args(*argv).output().unwrap();
        assert_eq!(
            output.status.code(),
            Some(*expected_exit),
            "{argv:?}: {output:?}"
        );
        assert!(output.stdout.is_empty(), "{argv:?}: {output:?}");
        assert!(
            std::str::from_utf8(&output.stderr)
                .unwrap()
                .contains(diagnostic),
            "{argv:?}: {output:?}"
        );
    }
}

#[test]
fn output_base_executables_reuses_one_daemon_across_capability_transitions() {
    let workspace = scratch("executables-capability-workspace");
    let output_base = scratch("executables-capability-output-base");
    let _cleanup = DaemonCleanup(output_base.clone());
    write(workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    let defs = workspace.join("pkg/defs.bzl");
    let build = workspace.join("pkg/BUILD.bazel");
    let definition = |name: &str, arguments: &str| {
        format!(
            "def _impl(ctx):\n    return [DefaultInfo()]\n\n{name} = rule(implementation = _impl{arguments})\n"
        )
    };
    let build_file = |rule: &str, target: &str| {
        format!("load(\":defs.bzl\", \"{rule}\")\n{rule}(name = \"{target}\")\n")
    };
    let output_base_arg = format!("--output_base={}", output_base.display());
    let query = |expression: &str| {
        slug()
            .current_dir(&workspace)
            .args([output_base_arg.as_str(), "query", expression])
            .output()
            .unwrap()
    };
    let assert_query = |expression: &str, expected: &str| {
        let output = query(expression);
        assert!(output.status.success(), "{expression}: {output:?}");
        assert!(output.stderr.is_empty(), "{expression}: {output:?}");
        assert_eq!(
            std::str::from_utf8(&output.stdout).unwrap(),
            expected,
            "{expression}"
        );
    };

    write(&defs, &definition("probe", ", executable = False"));
    write(&build, &build_file("probe", "item"));
    assert_query("executables(//pkg:item)", "");
    let pid = std::fs::read_to_string(slug_server_v2::pid_path(&output_base)).unwrap();

    write(&defs, &definition("probe", ", executable = True"));
    assert_query("executables(//pkg:item)", "//pkg:item\n");

    write(&defs, &definition("renamed_exec", ", executable = True"));
    write(&build, &build_file("renamed_exec", "item"));
    assert_query("executables(//pkg:item)", "//pkg:item\n");

    write(&defs, &definition("renamed_exec", ", executable = False"));
    assert_query("executables(//pkg:item)", "");

    write(&defs, &definition("probe_test", ", test = True"));
    write(&build, &build_file("probe_test", "item"));
    assert_query("executables(//pkg:item)", "");

    write(&defs, &definition("renamed_exec", ", executable = True"));
    write(&build, &build_file("renamed_exec", "item"));
    assert_query("executables(//pkg:item)", "//pkg:item\n");

    write(&build, &build_file("renamed_exec", "item_test"));
    assert_query("executables(//pkg:item_test)", "//pkg:item_test\n");

    write(
        &build,
        "# formatting only\nload( \":defs.bzl\", \"renamed_exec\" )\nrenamed_exec( name = \"item_test\" )\n",
    );
    assert_query("executables(//pkg:item_test)", "//pkg:item_test\n");

    std::fs::remove_file(&build).unwrap();
    let deleted = query("executables(//pkg:item_test)");
    assert_eq!(deleted.status.code(), Some(7), "{deleted:?}");
    assert!(deleted.stdout.is_empty(), "{deleted:?}");
    assert!(
        std::str::from_utf8(&deleted.stderr)
            .unwrap()
            .contains("no such package 'pkg'"),
        "{deleted:?}"
    );

    write(&build, &build_file("renamed_exec", "item_test"));
    assert_query("executables(//pkg:item_test)", "//pkg:item_test\n");
    assert_eq!(
        std::fs::read_to_string(slug_server_v2::pid_path(&output_base)).unwrap(),
        pid
    );
}

#[test]
fn reverse_query_fixture_matches_all_twenty_six_bazel_rows_through_cli() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/v2_oracle/fixtures/query-rdeps-and-subtree-patterns/workspace");
    let successes: &[(&[&str], &str)] = &[
        (
            &["query", "//tree/left/..."],
            "//tree/left:cross_only\n//tree/left:custom_parent\n//tree/left:cycle_a\n//tree/left:cycle_b\n//tree/left:leaf\n//tree/left:parent_one\n//tree/left:parent_two\n//tree/left:via_alias\n//tree/left/nested:nested\n",
        ),
        (&["query", "//nonpackage/..."], "//nonpackage/desc:desc\n"),
        (
            &["query", "rdeps(//..., //tree/left:source.txt)"],
            "//tree/left:custom_parent\n//tree/left:leaf\n//tree/left:parent_one\n//tree/left:parent_two\n//tree/left:source.txt\n//tree/left:via_alias\n//tree/right:right_both\n//tree/right:right_cross_only\n",
        ),
        (
            &["query", "rdeps(//..., //tree/left:source.txt, 0)"],
            "//tree/left:source.txt\n",
        ),
        (
            &["query", "rdeps(//..., //tree/left:source.txt, 1)"],
            "//tree/left:leaf\n//tree/left:parent_one\n//tree/left:parent_two\n//tree/left:source.txt\n//tree/right:right_both\n//tree/right:right_cross_only\n",
        ),
        (
            &["query", "rdeps(//..., //tree/left:source.txt, 2)"],
            "//tree/left:leaf\n//tree/left:parent_one\n//tree/left:parent_two\n//tree/left:source.txt\n//tree/left:via_alias\n//tree/right:right_both\n//tree/right:right_cross_only\n",
        ),
        (
            &[
                "query",
                "rdeps(//tree/right:right_parent, //tree/left:source.txt)",
            ],
            "",
        ),
        (
            &[
                "query",
                "rdeps(set(//tree/left:parent_one //tree/right:right_parent), //tree/left:source.txt)",
            ],
            "//tree/left:parent_one\n//tree/left:source.txt\n",
        ),
        (
            &[
                "query",
                "rdeps(//..., set(//tree/left:source.txt //tree/left:source.txt))",
            ],
            "//tree/left:custom_parent\n//tree/left:leaf\n//tree/left:parent_one\n//tree/left:parent_two\n//tree/left:source.txt\n//tree/left:via_alias\n//tree/right:right_both\n//tree/right:right_cross_only\n",
        ),
        (
            &["query", "rdeps(//tree/left:cycle_a, //tree/left:cycle_b)"],
            "//tree/left:cycle_a\n//tree/left:cycle_b\n",
        ),
        (
            &["query", "rdeps(//..., //tree/left:leaf)"],
            "//tree/left:custom_parent\n//tree/left:leaf\n//tree/left:via_alias\n",
        ),
        (
            &[
                "query",
                "rdeps(//tree/right:right_parent, //tree/left:leaf)",
            ],
            "",
        ),
        (
            &["query", "rdeps(//tree/..., //tree/left:leaf)"],
            "//tree/left:custom_parent\n//tree/left:leaf\n//tree/left:via_alias\n",
        ),
        (
            &[
                "query",
                "--order_output=auto",
                "rdeps(//tree/..., //tree/left:leaf)",
            ],
            "//tree/left:custom_parent\n//tree/left:leaf\n//tree/left:via_alias\n",
        ),
        (
            &[
                "query",
                "--order_output=full",
                "rdeps(//tree/..., //tree/left:leaf)",
            ],
            "//tree/left:custom_parent\n//tree/left:via_alias\n//tree/left:leaf\n",
        ),
        (
            &["query", "same_pkg_direct_rdeps(//tree/left:source.txt)"],
            "//tree/left:leaf\n//tree/left:parent_one\n//tree/left:parent_two\n",
        ),
        (
            &[
                "query",
                "same_pkg_direct_rdeps(set(//tree/left:source.txt //tree/left:source.txt))",
            ],
            "//tree/left:leaf\n//tree/left:parent_one\n//tree/left:parent_two\n",
        ),
        (
            &["query", "same_pkg_direct_rdeps(//tree/left:leaf)"],
            "//tree/left:via_alias\n",
        ),
        (
            &["query", "same_pkg_direct_rdeps(//tree/left:via_alias)"],
            "//tree/left:custom_parent\n",
        ),
        (
            &[
                "query",
                "same_pkg_direct_rdeps(set(//tree/left:source.txt //tree/right:right_source.txt))",
            ],
            "//tree/left:leaf\n//tree/left:parent_one\n//tree/left:parent_two\n//tree/right:right_both\n//tree/right:right_parent\n",
        ),
    ];
    for (args, expected) in successes {
        let output = slug().current_dir(&workspace).args(*args).output().unwrap();
        assert!(output.status.success(), "{args:?}: {output:?}");
        assert_eq!(
            std::str::from_utf8(&output.stdout).unwrap(),
            *expected,
            "{args:?}"
        );
    }

    let failures: &[(&[&str], i32, &str)] = &[
        (
            &["query", "//empty/..."],
            7,
            "no targets found beneath 'empty'",
        ),
        (
            &["query", "//missing/..."],
            7,
            "no targets found beneath 'missing'",
        ),
        (
            &["query", "rdeps(//..., //tree/left:leaf, 1, 2)"],
            2,
            "too many arguments to function 'rdeps'",
        ),
        (
            &["query", "rdeps(//..., 1)"],
            7,
            "no such target '//:1': target '1' not declared in package ''",
        ),
        (
            &["query", "same_pkg_direct_rdeps()"],
            2,
            "too few arguments to function 'same_pkg_direct_rdeps'",
        ),
        (
            &["query", "same_pkg_direct_rdeps(1)"],
            7,
            "no such target '//:1': target '1' not declared in package ''",
        ),
    ];
    for (args, exit, message) in failures {
        let output = slug().current_dir(&workspace).args(*args).output().unwrap();
        assert_eq!(output.status.code(), Some(*exit), "{args:?}: {output:?}");
        assert!(output.stdout.is_empty(), "{args:?}: {output:?}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(message), "{args:?}: {stderr}");
    }
}

#[test]
fn path_topology_fixture_covers_all_forty_three_bazel_rows_through_cli() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/v2_oracle/fixtures/query-path-topology/workspace");
    const LINEAR_ALL_AUTO: &str = "//:linear_end\n//:linear_mid\n//:linear_start\n";
    const LINEAR_FORWARD: &str = "//:linear_start\n//:linear_mid\n//:linear_end\n";
    const EMPTY: &str = "";
    let successes: &[(&[&str], &[&str])] = &[
        (
            &["query", "allpaths(//:linear_start, //:linear_end)"],
            &[LINEAR_ALL_AUTO],
        ),
        (
            &[
                "query",
                "--order_output=auto",
                "allpaths(//:linear_start, //:linear_end)",
            ],
            &[LINEAR_ALL_AUTO],
        ),
        (
            &[
                "query",
                "--order_output=full",
                "allpaths(//:linear_start, //:linear_end)",
            ],
            &[LINEAR_FORWARD],
        ),
        (
            &["query", "somepath(//:linear_start, //:linear_end)"],
            &[LINEAR_FORWARD],
        ),
        (
            &[
                "query",
                "--order_output=auto",
                "somepath(//:linear_start, //:linear_end)",
            ],
            &[LINEAR_FORWARD],
        ),
        (
            &[
                "query",
                "somepath(//:linear_start, //:linear_end) union //:disconnected",
            ],
            &["//:disconnected\n//:linear_end\n//:linear_mid\n//:linear_start\n"],
        ),
        (
            &[
                "query",
                "--order_output=auto",
                "somepath(//:linear_start, //:linear_end) union //:disconnected",
            ],
            &["//:disconnected\n//:linear_end\n//:linear_mid\n//:linear_start\n"],
        ),
        (
            &[
                "query",
                "--order_output=full",
                "somepath(//:linear_start, //:linear_end)",
            ],
            &[LINEAR_FORWARD],
        ),
        (
            &["query", "allpaths(//:diamond_start, //:diamond_end)"],
            &[
                "//:diamond_end\n//:diamond_left\n//:diamond_right\n//:diamond_split\n//:diamond_start\n",
            ],
        ),
        (
            &[
                "query",
                "--order_output=full",
                "somepath(//:diamond_start, //:diamond_end)",
            ],
            &[
                "//:diamond_start\n//:diamond_split\n//:diamond_left\n//:diamond_end\n",
                "//:diamond_start\n//:diamond_split\n//:diamond_right\n//:diamond_end\n",
            ],
        ),
        (
            &["query", "allpaths(//:cycle_a, //:cycle_end)"],
            &["//:cycle_a\n//:cycle_b\n//:cycle_end\n"],
        ),
        (
            &[
                "query",
                "--order_output=full",
                "somepath(//:cycle_a, //:cycle_end)",
            ],
            &["//:cycle_a\n//:cycle_b\n//:cycle_end\n"],
        ),
        (
            &["query", "allpaths(//:linear_mid, //:linear_mid)"],
            &["//:linear_mid\n"],
        ),
        (
            &["query", "somepath(//:linear_mid, //:linear_mid)"],
            &["//:linear_mid\n"],
        ),
        (
            &["query", "allpaths(//:linear_start, //:disconnected)"],
            &[EMPTY],
        ),
        (
            &["query", "somepath(//:linear_start, //:disconnected)"],
            &[EMPTY],
        ),
        (&["query", "allpaths(set(), //:linear_end)"], &[EMPTY]),
        (&["query", "allpaths(//:linear_start, set())"], &[EMPTY]),
        (&["query", "somepath(set(), //:linear_end)"], &[EMPTY]),
        (&["query", "somepath(//:linear_start, set())"], &[EMPTY]),
        (
            &[
                "query",
                "allpaths(set(//:multi_a //:multi_b), set(//:multi_end_a //:multi_end_b))",
            ],
            &["//:multi_a\n//:multi_b\n//:multi_end_a\n//:multi_end_b\n"],
        ),
        (
            &[
                "query",
                "--order_output=full",
                "somepath(set(//:one_pair_origin //:one_pair_other_origin), set(//:one_pair_destination //:one_pair_other_destination))",
            ],
            &["//:one_pair_origin\n//:one_pair_destination\n"],
        ),
        (
            &[
                "query",
                "--order_output=full",
                "somepath(set(//:ambiguous_a //:ambiguous_b), set(//:ambiguous_a_end //:ambiguous_b_end))",
            ],
            &[
                "//:ambiguous_a\n//:ambiguous_a_left\n//:ambiguous_a_end\n",
                "//:ambiguous_a\n//:ambiguous_a_right\n//:ambiguous_a_end\n",
                "//:ambiguous_b\n//:ambiguous_b_left\n//:ambiguous_b_end\n",
                "//:ambiguous_b\n//:ambiguous_b_right\n//:ambiguous_b_end\n",
            ],
        ),
        (
            &[
                "query",
                "allpaths(//:linear_start, set(//:linear_end //:disconnected))",
            ],
            &[LINEAR_ALL_AUTO],
        ),
        (
            &[
                "query",
                "somepath(//:linear_start, set(//:linear_end //:disconnected))",
            ],
            &[LINEAR_FORWARD],
        ),
        (
            &[
                "query",
                "allpaths(set(//:linear_start //:linear_start), set(//:linear_end //:linear_end))",
            ],
            &[LINEAR_ALL_AUTO],
        ),
        (
            &[
                "query",
                "somepath(set(//:linear_start //:linear_start), set(//:linear_end //:linear_end))",
            ],
            &[LINEAR_FORWARD],
        ),
        (
            &["query", "allpaths(//:source_parent, //:source.txt)"],
            &["//:source.txt\n//:source_parent\n"],
        ),
        (
            &["query", "somepath(//:source_parent, //:source.txt)"],
            &["//:source_parent\n//:source.txt\n"],
        ),
        (
            &["query", "allpaths(//:source.txt, //:source_parent)"],
            &[EMPTY],
        ),
        (
            &["query", "somepath(//:source.txt, //:source_parent)"],
            &[EMPTY],
        ),
        (
            &["query", "allpaths(//:alias_start, //:linear_end)"],
            &["//:alias_start\n//:linear_end\n//:linear_mid\n//:linear_start\n"],
        ),
        (
            &["query", "somepath(//:alias_start, //:linear_end)"],
            &["//:alias_start\n//:linear_start\n//:linear_mid\n//:linear_end\n"],
        ),
        (
            &["query", "allpaths(//:custom_start, //:custom_end)"],
            &["//:custom_end\n//:custom_mid\n//:custom_start\n"],
        ),
        (
            &["query", "somepath(//:custom_start, //:custom_end)"],
            &["//:custom_start\n//:custom_mid\n//:custom_end\n"],
        ),
    ];
    for (args, alternatives) in successes {
        let output = slug().current_dir(&workspace).args(*args).output().unwrap();
        assert!(output.status.success(), "{args:?}: {output:?}");
        let stdout = std::str::from_utf8(&output.stdout).unwrap();
        assert!(
            alternatives.contains(&stdout),
            "{args:?}: unexpected complete-path alternative {stdout:?}"
        );
    }

    let failures: &[(&[&str], i32, &str)] = &[
        (
            &["query", "allpaths()"],
            2,
            "too few arguments to function 'allpaths'",
        ),
        (
            &[
                "query",
                "allpaths(//:linear_start, //:linear_end, //:linear_mid)",
            ],
            2,
            "too many arguments to function 'allpaths'",
        ),
        (
            &["query", "somepath()"],
            2,
            "too few arguments to function 'somepath'",
        ),
        (
            &[
                "query",
                "somepath(//:linear_start, //:linear_end, //:linear_mid)",
            ],
            2,
            "too many arguments to function 'somepath'",
        ),
        (
            &["query", "allpaths(1, //:linear_end)"],
            7,
            "no such target '//:1': target '1' not declared in package ''",
        ),
        (
            &["query", "allpaths(//:linear_start, 1)"],
            7,
            "no such target '//:1': target '1' not declared in package ''",
        ),
        (
            &["query", "somepath(1, //:linear_end)"],
            7,
            "no such target '//:1': target '1' not declared in package ''",
        ),
        (
            &["query", "somepath(//:linear_start, 1)"],
            7,
            "no such target '//:1': target '1' not declared in package ''",
        ),
    ];
    for (args, exit, message) in failures {
        let output = slug().current_dir(&workspace).args(*args).output().unwrap();
        assert_eq!(output.status.code(), Some(*exit), "{args:?}: {output:?}");
        assert!(output.stdout.is_empty(), "{args:?}: {output:?}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(message), "{args:?}: {stderr}");
    }
}

#[test]
fn some_selection_fixture_covers_all_forty_two_bazel_rows_through_cli() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/v2_oracle/fixtures/query-some-selection/workspace");
    const ONE_OF_THREE: &[&str] = &["//:zeta\n", "//:alpha\n", "//:middle\n"];
    const TWO_AUTO: &[&str] = &[
        "//:alpha\n//:middle\n",
        "//:alpha\n//:zeta\n",
        "//:middle\n//:zeta\n",
    ];
    const TWO_FULL: &[&str] = &[
        "//:alpha\n//:middle\n",
        "//:alpha\n//:zeta\n",
        "//:middle\n//:alpha\n",
        "//:middle\n//:zeta\n",
        "//:zeta\n//:alpha\n",
        "//:zeta\n//:middle\n",
    ];
    let successes: &[(&str, Option<&str>, &[&str])] = &[
        ("some(//:single)", None, &["//:single\n"]),
        ("some(//:single)", Some("auto"), &["//:single\n"]),
        ("some(//:single)", Some("full"), &["//:single\n"]),
        ("some(set(//:zeta //:alpha //:middle))", None, ONE_OF_THREE),
        (
            "some(set(//:zeta //:alpha //:middle), 2)",
            Some("auto"),
            TWO_AUTO,
        ),
        (
            "some(set(//:zeta //:alpha //:middle), 2)",
            Some("full"),
            TWO_FULL,
        ),
        (
            "some(set(//:zeta //:alpha //:middle), 3)",
            Some("auto"),
            &["//:alpha\n//:middle\n//:zeta\n"],
        ),
        (
            "some(set(//:zeta //:alpha //:middle), 3)",
            Some("full"),
            &["//:zeta\n//:middle\n//:alpha\n"],
        ),
        (
            "some(set(//:zeta //:alpha //:middle), 5)",
            None,
            &["//:alpha\n//:middle\n//:zeta\n"],
        ),
        (
            "some(set(//:zeta //:zeta //:alpha), 5)",
            None,
            &["//:alpha\n//:zeta\n"],
        ),
        (
            "some(some(set(//:zeta //:alpha), 2) union some(set(//:alpha //:middle), 2), 5)",
            None,
            &["//:alpha\n//:middle\n//:zeta\n"],
        ),
        ("some(//:single, 2147483647)", None, &["//:single\n"]),
        (
            "some(deps(//:cycle_a), 5)",
            None,
            &["//:cycle_a\n//:cycle_b\n"],
        ),
        (
            "some(set(//:early //other:later), 1)",
            None,
            &["//:early\n", "//other:later\n"],
        ),
        (
            "some(set(//:early //other:later), 5)",
            None,
            &["//:early\n//other:later\n"],
        ),
        (
            "some(//recursive/..., 5)",
            None,
            &["//recursive:rec_zeta\n//recursive/nested:rec_alpha\n"],
        ),
        (
            "deps(//:depth_root, 2147483647)",
            None,
            &["//:depth_child\n//:depth_root\n"],
        ),
        ("deps(//:depth_root, '-1')", None, &[""]),
        ("deps(//:depth_root, '-2147483648')", None, &[""]),
        (
            "rdeps(set(//:depth_root //:depth_child), //:depth_child, '-1')",
            None,
            &[""],
        ),
        (
            "rdeps(set(//:depth_root //:depth_child), //:depth_child, '-2147483648')",
            None,
            &[""],
        ),
        (
            "rdeps(set(//:depth_root //:depth_child), //:depth_child, 2147483647)",
            None,
            &["//:depth_child\n//:depth_root\n"],
        ),
    ];
    let failures: &[(&str, i32, &str)] = &[
        (
            "some(//:early union //:missing_target, 1)",
            7,
            "no such target '//:missing_target'",
        ),
        (
            "some(set(//:early //:missing_target), 1)",
            7,
            "no such target '//:missing_target'",
        ),
        (
            "some(//:early union //missing:target, 1)",
            7,
            "no such package 'missing'",
        ),
        (
            "some(set(//:early //missing:target), 1)",
            7,
            "no such package 'missing'",
        ),
        ("some(set())", 7, "argument set is empty"),
        ("some(//:single, 0)", 7, "argument set is empty"),
        ("some(set(), 0)", 7, "argument set is empty"),
        ("some(//:single, '-1')", 7, "argument set is empty"),
        ("some(set(), '-1')", 7, "argument set is empty"),
        ("some(//:single, '-2147483648')", 7, "argument set is empty"),
        ("some(//:single, -1)", 2, "syntax error at '- 1 )'"),
        (
            "some(//missing:target, 2147483648)",
            2,
            "expected an integer literal: '2147483648'",
        ),
        (
            "some(//missing:target, '-2147483649')",
            2,
            "expected an integer literal: '-2147483649'",
        ),
        (
            "some(//missing:target, 2_147_483_647)",
            2,
            "expected an integer literal: '2_147_483_647'",
        ),
        (
            "some(//:single, nope)",
            2,
            "expected an integer literal: 'nope'",
        ),
        ("some()", 2, "too few arguments to function 'some'"),
        (
            "some(//:single, 1, 2)",
            2,
            "too many arguments to function 'some'",
        ),
        ("some(2147483648)", 7, "no such target '//:2147483648'"),
        (
            "deps(//:depth_root, 2147483648)",
            2,
            "expected an integer literal: '2147483648'",
        ),
        (
            "rdeps(set(//:depth_root //:depth_child), //:depth_child, 2147483648)",
            2,
            "expected an integer literal: '2147483648'",
        ),
    ];
    assert_eq!(successes.len() + failures.len(), 42);

    for (expression, order, alternatives) in successes {
        let mut command = slug();
        command.current_dir(&workspace).arg("query");
        if let Some(order) = order {
            command.arg(format!("--order_output={order}"));
        }
        let output = command.arg(expression).output().unwrap();
        assert!(output.status.success(), "{expression}: {output:?}");
        let stdout = std::str::from_utf8(&output.stdout).unwrap();
        assert!(alternatives.contains(&stdout), "{expression}: {stdout:?}");
    }
    for (expression, exit, message) in failures {
        let output = slug()
            .current_dir(&workspace)
            .args(["query", expression])
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(*exit),
            "{expression}: {output:?}"
        );
        assert!(output.stdout.is_empty(), "{expression}: {output:?}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(message), "{expression}: {stderr}");
    }
}

#[test]
fn output_base_query_reuses_one_daemon_across_build_edits() {
    let workspace = scratch("query-workspace");
    let output_base = scratch("query-output-base");
    let _cleanup = DaemonCleanup(output_base.clone());
    write(workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    write(
        workspace.join("pkg/BUILD.bazel"),
        "filegroup(name = \"bin\", srcs = [\"one.txt\"])\n",
    );
    write(workspace.join("pkg/one.txt"), "one\n");

    let output_base_arg = format!("--output_base={}", output_base.display());
    let first = slug()
        .current_dir(&workspace)
        .args([output_base_arg.as_str(), "query", "deps(//pkg:bin)"])
        .output()
        .unwrap();
    assert!(first.status.success(), "{first:?}");
    assert_eq!(
        String::from_utf8(first.stdout).unwrap(),
        "//pkg:bin\n//pkg:one.txt\n"
    );
    let pid = std::fs::read_to_string(slug_server_v2::pid_path(&output_base)).unwrap();

    write(
        workspace.join("pkg/BUILD.bazel"),
        "filegroup(name = \"bin\", srcs = [\"two.txt\"])\n",
    );
    write(workspace.join("pkg/two.txt"), "two\n");
    let second = slug()
        .current_dir(&workspace)
        .args([output_base_arg.as_str(), "query", "deps(//pkg:bin)"])
        .output()
        .unwrap();
    assert!(second.status.success(), "{second:?}");
    assert_eq!(
        String::from_utf8(second.stdout).unwrap(),
        "//pkg:bin\n//pkg:two.txt\n"
    );
    assert_eq!(
        std::fs::read_to_string(slug_server_v2::pid_path(&output_base)).unwrap(),
        pid
    );
}

#[test]
fn direct_external_module_cycle_is_public_only_for_query() {
    let workspace = scratch("external-module-cycle");
    write(
        workspace.join("MODULE.bazel"),
        "print(\"ROOT_EVENT\")\nmodule(name = \"demo\")\nbazel_dep(name = \"dep\", version = \"1.0.0\")\nlocal_path_override(module_name = \"dep\", path = \"dep\")\n",
    );
    write(
        workspace.join("dep/MODULE.bazel"),
        "print(\"DEP_EVENT\")\nmodule(name = \"dep\", version = \"1.0.0\")\ninclude(\"//cycle:a.MODULE.bazel\")\n",
    );
    write(
        workspace.join("dep/cycle/a.MODULE.bazel"),
        "include(\"//cycle:b.MODULE.bazel\")\n",
    );
    write(
        workspace.join("dep/cycle/b.MODULE.bazel"),
        "include(\"//cycle:a.MODULE.bazel\")\n",
    );
    write(workspace.join("dep/cycle/BUILD.bazel"), "");
    write(
        workspace.join("dep/BUILD.bazel"),
        "print(\"BUILD_EVENT\")\nexports_files([\"target.txt\"])\n",
    );
    write(workspace.join("dep/target.txt"), "target\n");

    let query = slug()
        .current_dir(&workspace)
        .args(["query", "@dep//:target.txt"])
        .output()
        .unwrap();
    assert_eq!(query.status.code(), Some(7), "{query:?}");
    assert!(query.stdout.is_empty(), "{query:?}");
    let stderr = String::from_utf8(query.stderr).unwrap();
    let event = stderr.find("ROOT_EVENT").unwrap();
    let terminal = stderr.find("{\"error\":\"unsupported_feature\"").unwrap();
    let message = format!(
        "Slug does not support MODULE.bazel include cycles in direct local_path_override repository '@dep' for module 'dep': include \"//cycle:a.MODULE.bazel\" at {}:1:1 repeats ancestor include \"//cycle:a.MODULE.bazel\" at {}:3:1",
        workspace.join("dep/cycle/b.MODULE.bazel").display(),
        workspace.join("dep/MODULE.bazel").display(),
    );
    let expected_terminal = format!(
        "{{\"error\":\"unsupported_feature\",\"command\":\"query\",\"message\":\"{}\",\"runtime_mode\":\"one-shot\"}}\n",
        slug_core_v2::error::json_escape(&message),
    );
    assert!(event < terminal, "{stderr}");
    assert_eq!(&stderr[terminal..], expected_terminal);
    assert!(stderr.contains("/dep/cycle/b.MODULE.bazel:1:1"), "{stderr}");
    assert!(stderr.contains("/dep/MODULE.bazel:3:1"), "{stderr}");
    assert!(stderr.contains("\"runtime_mode\":\"one-shot\""), "{stderr}");
    assert!(!stderr.contains("DEP_EVENT"), "{stderr}");
    assert!(!stderr.contains("BUILD_EVENT"), "{stderr}");
    assert!(!stderr.contains("Evaluation of query"), "{stderr}");

    let build = slug()
        .current_dir(&workspace)
        .args(["build", "@dep//:target.txt"])
        .output()
        .unwrap();
    let build_stderr = String::from_utf8(build.stderr).unwrap();
    assert!(
        !build_stderr.contains("unsupported_feature"),
        "{build_stderr}"
    );
    assert!(!build_stderr.contains("DEP_EVENT"), "{build_stderr}");
    assert!(!build_stderr.contains("BUILD_EVENT"), "{build_stderr}");

    write(
        workspace.join("dep/MODULE.bazel"),
        "module(name = \"dep\", version = \"1.0.0\")\nunknown_symbol()\n",
    );
    let ordinary = slug()
        .current_dir(&workspace)
        .args(["query", "@dep//:target.txt"])
        .output()
        .unwrap();
    assert_eq!(ordinary.status.code(), Some(7), "{ordinary:?}");
    let ordinary_stderr = String::from_utf8(ordinary.stderr).unwrap();
    assert!(ordinary_stderr.contains("\"error\":\"query_error\""));
    assert!(!ordinary_stderr.contains("unsupported_feature"));
    assert!(ordinary_stderr.contains("Evaluation of query"));
}

#[test]
fn direct_external_query_matches_one_shot_and_retained_daemon_output_and_events() {
    let workspace = scratch("external-query-workspace");
    let output_base = scratch("external-query-output-base");
    let _cleanup = DaemonCleanup(output_base.clone());
    write(
        workspace.join("MODULE.bazel"),
        "print(\"MODULE_EVENT\")\nmodule(name = \"demo\")\nbazel_dep(name = \"dep\", version = \"1.0.0\")\nlocal_path_override(module_name = \"dep\", path = \"dep\")\n",
    );
    write(
        workspace.join("dep/MODULE.bazel"),
        "module(name = \"dep\", version = \"1.0.0\")\n",
    );
    write(
        workspace.join("dep/BUILD.bazel"),
        "print(\"EXTERNAL_BUILD_EVENT\")\nexports_files([\"target.txt\"])\n",
    );
    write(workspace.join("dep/target.txt"), "target\n");
    write(
        workspace.join("dep/macro/BUILD.bazel"),
        "load(\":defs.bzl\", \"make_filegroup\")\nprint(\"EXTERNAL_MACRO_BUILD\")\nmake_filegroup(name = \"macro_files\")\n",
    );
    write(
        workspace.join("dep/macro/defs.bzl"),
        "print(\"EXTERNAL_DEFS_EVENT\")\ndef make_filegroup(name):\n    print(\"EXTERNAL_MACRO_BODY\")\n    native.filegroup(name = name)\n",
    );
    write(
        workspace.join("macro/BUILD.bazel"),
        "filegroup(name = \"root_sentinel\")\n",
    );
    write(
        workspace.join("dep/rule/BUILD.bazel"),
        "load(\":defs.bzl\", \"probe\")\nprobe(name = \"probe\", visibility = [\"//visibility:public\"])\n",
    );
    write(
        workspace.join("dep/rule/defs.bzl"),
        "def _impl(ctx):\n    return [DefaultInfo()]\nprobe = rule(implementation = _impl)\n",
    );

    let one_shot = slug()
        .current_dir(&workspace)
        .args(["query", "@dep//:target.txt"])
        .output()
        .unwrap();
    assert!(one_shot.status.success(), "{one_shot:?}");
    assert_eq!(
        String::from_utf8(one_shot.stdout).unwrap(),
        "@dep//:target.txt\n"
    );
    let one_shot_stderr = String::from_utf8(one_shot.stderr).unwrap();
    let module_index = one_shot_stderr.find("MODULE_EVENT").unwrap();
    let build_index = one_shot_stderr.find("EXTERNAL_BUILD_EVENT").unwrap();
    assert!(module_index < build_index, "{one_shot_stderr}");

    let one_shot_macro = slug()
        .current_dir(&workspace)
        .args(["query", "--output=label_kind", "@dep//macro:macro_files"])
        .output()
        .unwrap();
    assert!(one_shot_macro.status.success(), "{one_shot_macro:?}");
    assert_eq!(
        String::from_utf8(one_shot_macro.stdout).unwrap(),
        "filegroup rule @dep//macro:macro_files\n"
    );
    let one_shot_macro_stderr = String::from_utf8(one_shot_macro.stderr).unwrap();
    let defs_index = one_shot_macro_stderr.find("EXTERNAL_DEFS_EVENT").unwrap();
    let macro_build_index = one_shot_macro_stderr.find("EXTERNAL_MACRO_BUILD").unwrap();
    let macro_body_index = one_shot_macro_stderr.find("EXTERNAL_MACRO_BODY").unwrap();
    assert!(defs_index < macro_build_index && macro_build_index < macro_body_index);
    let one_shot_rule = slug()
        .current_dir(&workspace)
        .args(["query", "--output=label_kind", "@dep//rule:probe"])
        .output()
        .unwrap();
    assert!(one_shot_rule.status.success(), "{one_shot_rule:?}");
    assert_eq!(
        String::from_utf8(one_shot_rule.stdout).unwrap(),
        "probe rule @dep//rule:probe\n"
    );

    let output_base_arg = format!("--output_base={}", output_base.display());
    let daemon_query = |args: &[&str]| {
        slug()
            .current_dir(&workspace)
            .arg(output_base_arg.as_str())
            .args(args)
            .output()
            .unwrap()
    };
    let first = slug()
        .current_dir(&workspace)
        .args([output_base_arg.as_str(), "query", "@dep//:target.txt"])
        .output()
        .unwrap();
    assert!(first.status.success(), "{first:?}");
    assert_eq!(
        String::from_utf8(first.stdout).unwrap(),
        "@dep//:target.txt\n"
    );
    let first_stderr = String::from_utf8(first.stderr).unwrap();
    assert!(first_stderr.contains("MODULE_EVENT"), "{first_stderr}");
    assert!(
        first_stderr.contains("EXTERNAL_BUILD_EVENT"),
        "{first_stderr}"
    );
    let pid = std::fs::read_to_string(slug_server_v2::pid_path(&output_base)).unwrap();

    let warm = slug()
        .current_dir(&workspace)
        .args([output_base_arg.as_str(), "query", "@dep//:target.txt"])
        .output()
        .unwrap();
    assert!(warm.status.success(), "{warm:?}");
    assert_eq!(String::from_utf8_lossy(&warm.stdout), "@dep//:target.txt\n");
    assert!(warm.stderr.is_empty(), "{warm:?}");
    assert_eq!(
        std::fs::read_to_string(slug_server_v2::pid_path(&output_base)).unwrap(),
        pid
    );

    let macro_cold = slug()
        .current_dir(&workspace)
        .args([
            output_base_arg.as_str(),
            "query",
            "--output=label_kind",
            "@dep//macro:macro_files",
        ])
        .output()
        .unwrap();
    assert!(macro_cold.status.success(), "{macro_cold:?}");
    assert_eq!(
        String::from_utf8(macro_cold.stdout).unwrap(),
        "filegroup rule @dep//macro:macro_files\n"
    );
    let macro_cold_stderr = String::from_utf8(macro_cold.stderr).unwrap();
    assert!(macro_cold_stderr.contains("EXTERNAL_DEFS_EVENT"));
    assert!(macro_cold_stderr.contains("EXTERNAL_MACRO_BUILD"));
    assert!(macro_cold_stderr.contains("EXTERNAL_MACRO_BODY"));

    let rule_cold = daemon_query(&["query", "--output=label_kind", "@dep//rule:probe"]);
    assert!(rule_cold.status.success(), "{rule_cold:?}");
    assert_eq!(
        String::from_utf8(rule_cold.stdout).unwrap(),
        "probe rule @dep//rule:probe\n"
    );

    for (args, expected) in [
        (
            vec!["query", "siblings(@dep//:target.txt)"],
            "@dep//:BUILD.bazel\n@dep//:target.txt\n",
        ),
        (
            vec!["query", "same_pkg_direct_rdeps(@dep//:target.txt)"],
            "",
        ),
        (
            vec!["query", "buildfiles(@dep//:target.txt)"],
            "@dep//:BUILD.bazel\n",
        ),
        (vec!["query", "loadfiles(@dep//:target.txt)"], ""),
        (
            vec!["query", "--output=label", "@dep//:target.txt"],
            "@dep//:target.txt\n",
        ),
        (
            vec!["query", "--output=label_kind", "@dep//:target.txt"],
            "source file @dep//:target.txt\n",
        ),
        (
            vec!["query", "--output=package", "@dep//:target.txt"],
            "@dep//\n",
        ),
        (
            vec!["query", "--output=graph", "@dep//:target.txt"],
            concat!(
                "digraph mygraph {\n",
                "  node [shape=box];\n",
                "  \"@dep//:target.txt\"\n",
                "}\n",
            ),
        ),
        (
            vec!["query", "--output=label_kind", "@dep//macro:macro_files"],
            "filegroup rule @dep//macro:macro_files\n",
        ),
        (
            vec!["query", "loadfiles(@dep//macro:macro_files)"],
            "@dep//macro:defs.bzl\n",
        ),
        (
            vec!["query", "buildfiles(@dep//macro:macro_files)"],
            "@dep//macro:BUILD.bazel\n@dep//macro:defs.bzl\n",
        ),
    ] {
        let output = slug()
            .current_dir(&workspace)
            .arg(output_base_arg.as_str())
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    }

    write(
        workspace.join("dep/macro/defs.bzl"),
        "print(\"EXTERNAL_DEFS_EDITED\")\ndef make_filegroup(name):\n    native.filegroup(name = name)\n",
    );
    let macro_edited = slug()
        .current_dir(&workspace)
        .args([
            output_base_arg.as_str(),
            "query",
            "loadfiles(@dep//macro:macro_files)",
        ])
        .output()
        .unwrap();
    assert!(macro_edited.status.success(), "{macro_edited:?}");
    assert_eq!(
        String::from_utf8(macro_edited.stdout).unwrap(),
        "@dep//macro:defs.bzl\n"
    );
    assert!(String::from_utf8_lossy(&macro_edited.stderr).contains("EXTERNAL_DEFS_EDITED"));

    std::fs::remove_file(workspace.join("dep/macro/defs.bzl")).unwrap();
    let macro_deleted = slug()
        .current_dir(&workspace)
        .args([output_base_arg.as_str(), "query", "@dep//macro:macro_files"])
        .output()
        .unwrap();
    assert_eq!(macro_deleted.status.code(), Some(7), "{macro_deleted:?}");
    assert!(
        String::from_utf8_lossy(&macro_deleted.stderr)
            .contains("cannot load '@@dep+//macro:defs.bzl': no such file")
    );

    write(
        workspace.join("dep/macro/defs.bzl"),
        "def make_filegroup(name):\n    native.filegroup(name = name)\n",
    );
    let macro_recreated = slug()
        .current_dir(&workspace)
        .args([output_base_arg.as_str(), "query", "@dep//macro:macro_files"])
        .output()
        .unwrap();
    assert!(macro_recreated.status.success(), "{macro_recreated:?}");
    assert_eq!(
        String::from_utf8(macro_recreated.stdout).unwrap(),
        "@dep//macro:macro_files\n"
    );
    assert_eq!(
        std::fs::read_to_string(slug_server_v2::pid_path(&output_base)).unwrap(),
        pid
    );

    write(
        workspace.join("dep/rule/defs.bzl"),
        "print(\"RULE_EDITED\")\ndef _impl(ctx):\n    return [DefaultInfo()]\nprobe = rule(implementation = _impl)\n",
    );
    let rule_edited = daemon_query(&["query", "@dep//rule:probe"]);
    assert!(rule_edited.status.success(), "{rule_edited:?}");
    assert!(String::from_utf8_lossy(&rule_edited.stderr).contains("RULE_EDITED"));
    std::fs::remove_file(workspace.join("dep/rule/defs.bzl")).unwrap();
    let rule_deleted = daemon_query(&["query", "@dep//rule:probe"]);
    assert_eq!(rule_deleted.status.code(), Some(7), "{rule_deleted:?}");
    write(
        workspace.join("dep/rule/defs.bzl"),
        "def _impl(ctx):\n    return [DefaultInfo()]\nprobe = rule(implementation = _impl)\n",
    );
    let rule_recreated = daemon_query(&["query", "@dep//rule:probe"]);
    assert!(rule_recreated.status.success(), "{rule_recreated:?}");
    assert_eq!(
        String::from_utf8(rule_recreated.stdout).unwrap(),
        "@dep//rule:probe\n"
    );

    let external_pattern = slug()
        .current_dir(&workspace)
        .args(["query", "@dep//:*"])
        .output()
        .unwrap();
    assert_eq!(
        external_pattern.status.code(),
        Some(7),
        "{external_pattern:?}"
    );
    assert!(external_pattern.stdout.is_empty(), "{external_pattern:?}");
    assert!(
        String::from_utf8_lossy(&external_pattern.stderr)
            .contains("external repository query patterns are deferred"),
        "{external_pattern:?}"
    );

    write(
        workspace.join("dep/BUILD.bazel"),
        "print(\"EXTERNAL_BUILD_EDITED\")\nexports_files([\"edited.txt\"])\n",
    );
    write(workspace.join("dep/edited.txt"), "edited\n");
    let edited = slug()
        .current_dir(&workspace)
        .args([output_base_arg.as_str(), "query", "@dep//:edited.txt"])
        .output()
        .unwrap();
    assert!(edited.status.success(), "{edited:?}");
    assert_eq!(
        String::from_utf8(edited.stdout).unwrap(),
        "@dep//:edited.txt\n"
    );
    let edited_stderr = String::from_utf8(edited.stderr).unwrap();
    assert!(
        edited_stderr.contains("EXTERNAL_BUILD_EDITED"),
        "{edited_stderr}"
    );
    assert!(!edited_stderr.contains("MODULE_EVENT"), "{edited_stderr}");

    write(
        workspace.join("dep2/MODULE.bazel"),
        "module(name = \"dep\", version = \"1.0.0\")\n",
    );
    write(
        workspace.join("dep2/BUILD.bazel"),
        "exports_files([\"remapped.txt\"])\n",
    );
    write(workspace.join("dep2/remapped.txt"), "remapped\n");
    write(
        workspace.join("MODULE.bazel"),
        "module(name = \"demo\")\nbazel_dep(name = \"dep\", version = \"1.0.0\")\nlocal_path_override(module_name = \"dep\", path = \"dep2\")\n",
    );
    let remapped = slug()
        .current_dir(&workspace)
        .args([
            output_base_arg.as_str(),
            "query",
            "buildfiles(@dep//:remapped.txt)",
        ])
        .output()
        .unwrap();
    assert!(remapped.status.success(), "{remapped:?}");
    assert_eq!(
        String::from_utf8(remapped.stdout).unwrap(),
        "@dep//:BUILD.bazel\n"
    );

    std::fs::remove_file(workspace.join("dep2/BUILD.bazel")).unwrap();
    let deleted = slug()
        .current_dir(&workspace)
        .args([output_base_arg.as_str(), "query", "@dep//:remapped.txt"])
        .output()
        .unwrap();
    assert_eq!(deleted.status.code(), Some(7), "{deleted:?}");
    assert!(deleted.stdout.is_empty(), "{deleted:?}");

    write(
        workspace.join("dep2/BUILD.bazel"),
        "exports_files([\"remapped.txt\"])\n",
    );
    let recovered = slug()
        .current_dir(&workspace)
        .args([output_base_arg.as_str(), "query", "@dep//:remapped.txt"])
        .output()
        .unwrap();
    assert!(recovered.status.success(), "{recovered:?}");
    assert_eq!(
        String::from_utf8(recovered.stdout).unwrap(),
        "@dep//:remapped.txt\n"
    );
    assert_eq!(
        std::fs::read_to_string(slug_server_v2::pid_path(&output_base)).unwrap(),
        pid
    );
}

#[test]
fn bzlmod_environment_is_captured_per_one_shot_and_daemon_query_child() {
    let workspace = scratch("bzlmod-env-workspace");
    let output_base = scratch("bzlmod-env-output-base");
    let _cleanup = DaemonCleanup(output_base.clone());
    write(workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    write(
        workspace.join("pkg/BUILD.bazel"),
        "filegroup(name = \"probe\")\n",
    );

    let one_shot = slug()
        .current_dir(&workspace)
        .env("BZLMOD_ALLOW_YANKED_VERSIONS", "yyy@1.0.0")
        .args(["query", "//pkg:probe"])
        .output()
        .unwrap();
    assert!(one_shot.status.success(), "{one_shot:?}");
    assert_eq!(String::from_utf8(one_shot.stdout).unwrap(), "//pkg:probe\n");

    let invalid = slug()
        .current_dir(&workspace)
        .env("BZLMOD_ALLOW_YANKED_VERSIONS", "not-a-module")
        .args(["query", "//pkg:probe"])
        .output()
        .unwrap();
    assert_eq!(invalid.status.code(), Some(2), "{invalid:?}");
    assert!(
        String::from_utf8(invalid.stderr)
            .unwrap()
            .contains("BZLMOD_ALLOW_YANKED_VERSIONS")
    );

    let output_base_arg = format!("--output_base={}", output_base.display());
    let daemon_a = slug()
        .current_dir(&workspace)
        .env_remove("BZLMOD_ALLOW_YANKED_VERSIONS")
        .args([output_base_arg.as_str(), "query", "//pkg:probe"])
        .output()
        .unwrap();
    assert!(daemon_a.status.success(), "{daemon_a:?}");
    let pid = std::fs::read_to_string(slug_server_v2::pid_path(&output_base)).unwrap();
    let daemon_b = slug()
        .current_dir(&workspace)
        .env("BZLMOD_ALLOW_YANKED_VERSIONS", "all")
        .args([output_base_arg.as_str(), "query", "//pkg:probe"])
        .output()
        .unwrap();
    assert!(daemon_b.status.success(), "{daemon_b:?}");
    let daemon_a_again = slug()
        .current_dir(&workspace)
        .env_remove("BZLMOD_ALLOW_YANKED_VERSIONS")
        .args([output_base_arg.as_str(), "query", "//pkg:probe"])
        .output()
        .unwrap();
    assert!(daemon_a_again.status.success(), "{daemon_a_again:?}");
    assert_eq!(
        std::fs::read_to_string(slug_server_v2::pid_path(&output_base)).unwrap(),
        pid
    );
}

#[test]
fn equality_form_registry_reaches_one_shot_and_daemon_build_and_query() {
    let workspace = scratch("registry-command-transport");
    let output_base = scratch("registry-command-transport-output-base");
    let _cleanup = DaemonCleanup(output_base.clone());
    write(workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    write(workspace.join("BUILD.bazel"), "");
    write(
        workspace.join("pkg/BUILD.bazel"),
        "filegroup(name = \"probe\")\n",
    );
    let registry_a = "--registry=https://registry-a.example/";
    let registry_b = "--registry=https://registry-b.example";
    let output_base_arg = format!("--output_base={}", output_base.display());

    for args in [
        vec!["query", registry_a, registry_b, "//pkg:probe"],
        vec![
            output_base_arg.as_str(),
            "query",
            registry_a,
            registry_b,
            "//pkg:probe",
        ],
    ] {
        let output = slug().current_dir(&workspace).args(args).output().unwrap();
        assert!(output.status.success(), "{output:?}");
        assert_eq!(String::from_utf8(output.stdout).unwrap(), "//pkg:probe\n");
    }
    for args in [
        vec!["build", registry_a, registry_b, "//pkg:probe"],
        vec![
            output_base_arg.as_str(),
            "build",
            registry_a,
            registry_b,
            "//pkg:probe",
        ],
    ] {
        let output = slug().current_dir(&workspace).args(args).output().unwrap();
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("analysis_not_implemented")
        );
    }
    for args in [
        vec!["query", "--registry=file://bad", "//pkg:probe"],
        vec![
            output_base_arg.as_str(),
            "query",
            "--registry=file://bad",
            "//pkg:probe",
        ],
        vec!["build", "--registry=file://bad", "//pkg:probe"],
        vec![
            output_base_arg.as_str(),
            "build",
            "--registry=file://bad",
            "//pkg:probe",
        ],
    ] {
        let output = slug().current_dir(&workspace).args(args).output().unwrap();
        assert!(!output.status.success(), "{output:?}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains("Invalid registry URL: file://bad: Unsupported non-local file URL"),
            "{stderr}"
        );
        assert!(!stderr.contains("analysis_not_implemented"), "{stderr}");
    }
}

#[test]
fn bzlmod_environment_is_captured_per_one_shot_and_daemon_build_child() {
    let workspace = scratch("bzlmod-build-env-workspace");
    let output_base = scratch("bzlmod-build-env-output-base");
    let _cleanup = DaemonCleanup(output_base.clone());
    write(workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    write(workspace.join("BUILD.bazel"), "");
    write(
        workspace.join("pkg/BUILD.bazel"),
        "filegroup(name = \"probe\")\n",
    );

    let one_shot = slug()
        .current_dir(&workspace)
        .env("BZLMOD_ALLOW_YANKED_VERSIONS", "all")
        .args(["build", "//pkg:probe"])
        .output()
        .unwrap();
    assert_eq!(one_shot.status.code(), Some(2), "{one_shot:?}");
    assert!(
        String::from_utf8(one_shot.stderr)
            .unwrap()
            .contains("analysis_not_implemented")
    );

    let output_base_arg = format!("--output_base={}", output_base.display());
    let daemon_b = slug()
        .current_dir(&workspace)
        .env("BZLMOD_ALLOW_YANKED_VERSIONS", "yyy@1.0.0")
        .args([output_base_arg.as_str(), "build", "//pkg:probe"])
        .output()
        .unwrap();
    assert_eq!(daemon_b.status.code(), Some(2), "{daemon_b:?}");
    assert!(
        String::from_utf8(daemon_b.stderr)
            .unwrap()
            .contains("analysis_not_implemented")
    );
    let pid = std::fs::read_to_string(slug_server_v2::pid_path(&output_base)).unwrap();
    let daemon_a = slug()
        .current_dir(&workspace)
        .env_remove("BZLMOD_ALLOW_YANKED_VERSIONS")
        .args([output_base_arg.as_str(), "build", "//pkg:probe"])
        .output()
        .unwrap();
    assert_eq!(daemon_a.status.code(), Some(2), "{daemon_a:?}");
    assert_eq!(
        std::fs::read_to_string(slug_server_v2::pid_path(&output_base)).unwrap(),
        pid
    );
}

#[cfg(unix)]
#[test]
fn non_unicode_bzlmod_environment_is_rejected_before_one_shot_evaluation() {
    use std::os::unix::ffi::OsStringExt;

    let workspace = scratch("bzlmod-env-non-unicode");
    write(workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    write(
        workspace.join("pkg/BUILD.bazel"),
        "filegroup(name = \"probe\")\n",
    );
    let output = slug()
        .current_dir(&workspace)
        .env(
            "BZLMOD_ALLOW_YANKED_VERSIONS",
            std::ffi::OsString::from_vec(vec![0xff]),
        )
        .args(["query", "//pkg:probe"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("environment value is not valid Unicode"),
        "{stderr}"
    );
}

#[test]
fn only_package_loading_query_errors_receive_evaluation_context() {
    let workspace = scratch("query-error-context");
    let output_base = scratch("query-error-context-output-base");
    let _cleanup = DaemonCleanup(output_base.clone());
    write(workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    write(workspace.join("BUILD.bazel"), "filegroup(name = \"one\")\n");
    let output_base_arg = format!("--output_base={}", output_base.display());

    for (expression, exit_code, diagnostic) in [
        ("some(//:one, -1)", 2, "syntax error at '- 1 )'"),
        ("some(set())", 7, "argument set is empty"),
    ] {
        let output = slug()
            .current_dir(&workspace)
            .args(["query", expression])
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(exit_code),
            "{expression}: {output:?}"
        );
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains(diagnostic),
            "one-shot {expression}: {stderr}"
        );
        assert!(
            !stderr.contains("Evaluation of query"),
            "one-shot {expression}: {stderr}"
        );
    }

    for (expression, exit_code, diagnostic) in [
        ("some(//:one, -1)", 2, "syntax error at '- 1 )'"),
        ("some(set())", 7, "argument set is empty"),
    ] {
        let output = slug()
            .current_dir(&workspace)
            .args([output_base_arg.as_str(), "query", expression])
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(exit_code),
            "{expression}: {output:?}"
        );
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(diagnostic), "daemon {expression}: {stderr}");
        assert!(
            !stderr.contains("Evaluation of query"),
            "daemon {expression}: {stderr}"
        );
    }
}

#[test]
fn output_base_labels_reuses_one_daemon_across_metadata_semantic_transitions() {
    let workspace = scratch("labels-metadata-workspace");
    let output_base = scratch("labels-metadata-output-base");
    let _cleanup = DaemonCleanup(output_base.clone());
    write(workspace.join("MODULE.bazel"), "module(name = \"demo\")\n");
    let defs = workspace.join("pkg/defs.bzl");
    let build = workspace.join("pkg/BUILD.bazel");
    let schema = |default| {
        format!(
            "def _impl(ctx):\n    return [DefaultInfo()]\nprobe = rule(implementation = _impl, attrs = {{\"dep\": attr.label(default = \":{default}\"), \"out\": attr.output(mandatory = True)}})\n"
        )
    };
    write(&defs, &schema("one.txt"));
    write(
        &build,
        "config_setting(name = \"linux\", values = {\"cpu\": \"k8\"})\nexports_files([\"one.txt\", \"two.txt\", \"three.txt\"])\nload(\":defs.bzl\", \"probe\")\nprobe(name = \"rule\", out = \"one.out\")\n",
    );
    let output_base_arg = format!("--output_base={}", output_base.display());
    let query = |expression: &str| {
        slug()
            .current_dir(&workspace)
            .args([output_base_arg.as_str(), "query", expression])
            .output()
            .unwrap()
    };
    let assert_query = |expression: &str, expected: &str| {
        let output = query(expression);
        assert!(output.status.success(), "{expression}: {output:?}");
        assert_eq!(
            std::str::from_utf8(&output.stdout).unwrap(),
            expected,
            "{expression}"
        );
        assert!(output.stderr.is_empty(), "{expression}: {output:?}");
    };

    assert_query("labels(dep, //pkg:rule)", "//pkg:one.txt\n");
    let pid = std::fs::read_to_string(slug_server_v2::pid_path(&output_base)).unwrap();
    write(
        &build,
        "# semantic no-op\nconfig_setting( name = \"linux\", values = {\"cpu\": \"k8\"} )\nexports_files([\"one.txt\", \"two.txt\", \"three.txt\"])\nload(\":defs.bzl\", \"probe\")\nprobe( name = \"rule\", out = \"one.out\" )\n",
    );
    assert_query("labels(dep, //pkg:rule)", "//pkg:one.txt\n");

    write(&defs, &schema("two.txt"));
    assert_query("labels(dep, //pkg:rule)", "//pkg:two.txt\n");

    write(
        &build,
        "config_setting(name = \"linux\", values = {\"cpu\": \"k8\"})\nexports_files([\"one.txt\", \"two.txt\", \"three.txt\"])\nload(\":defs.bzl\", \"probe\")\nprobe(name = \"rule\", dep = \":three.txt\", out = \"one.out\")\n",
    );
    assert_query("labels(dep, //pkg:rule)", "//pkg:three.txt\n");

    write(
        &build,
        "config_setting(name = \"linux\", values = {\"cpu\": \"k8\"})\nexports_files([\"one.txt\", \"two.txt\", \"three.txt\"])\nload(\":defs.bzl\", \"probe\")\nprobe(name = \"rule\", dep = select({\":linux\": \":one.txt\", \"//conditions:default\": \":two.txt\"}), out = \"one.out\")\n",
    );
    assert_query("labels(dep, //pkg:rule)", "//pkg:one.txt\n//pkg:two.txt\n");

    write(
        &build,
        "config_setting(name = \"linux\", values = {\"cpu\": \"k8\"})\nexports_files([\"one.txt\", \"two.txt\", \"three.txt\"])\nload(\":defs.bzl\", \"probe\")\nprobe(name = \"rule\", dep = select({\":linux\": \":one.txt\", \"//conditions:default\": \":two.txt\"}), out = \"two.out\")\n",
    );
    assert_query("labels(out, //pkg:rule)", "//pkg:two.out\n");
    assert_eq!(
        std::fs::read_to_string(slug_server_v2::pid_path(&output_base)).unwrap(),
        pid
    );
}

#[test]
fn label_kind_matches_the_accepted_rule_capability_and_generated_file_rows() {
    let cases = [
        (
            "query-executables-rule-capability",
            "//pkg:arbitrary_target",
            "exec_arbitrary rule //pkg:arbitrary_target\n",
        ),
        (
            "query-executables-rule-capability",
            "//pkg:ordinary_target",
            "implicit_test_test rule //pkg:ordinary_target\n",
        ),
        (
            "query-executables-rule-capability",
            "//pkg:explicit_test_target",
            "explicit_test_test rule //pkg:explicit_test_target\n",
        ),
        (
            "query-executables-rule-capability",
            "//pkg:plain",
            "plain_rule rule //pkg:plain\n",
        ),
        (
            "query-executables-rule-capability",
            "//pkg:generated_owner",
            "output_rule rule //pkg:generated_owner\n",
        ),
        (
            "query-executables-rule-capability",
            "//pkg:files",
            "filegroup rule //pkg:files\n",
        ),
        (
            "query-executables-rule-capability",
            "//pkg:alias_exec",
            "alias rule //pkg:alias_exec\n",
        ),
        (
            "query-executables-rule-capability",
            "//pkg:setting",
            "config_setting rule //pkg:setting\n",
        ),
        (
            "query-labels-attribute-metadata",
            "labels(out, //pkg:outputs)",
            "generated file //pkg:one.out\n",
        ),
        (
            "query-labels-attribute-metadata",
            "labels(outs, //pkg:outputs)",
            "generated file //pkg:three.out\ngenerated file //pkg:two.out\n",
        ),
    ];

    for (fixture, expression, expected) in cases {
        let output = slug()
            .current_dir(fixture_workspace(fixture))
            .args(["query", "--output=label_kind", expression])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{fixture} {expression}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            output.stderr.is_empty(),
            "{fixture} {expression}: {output:?}"
        );
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            expected,
            "{fixture} {expression}"
        );
    }
}

#[test]
fn package_output_matches_the_three_accepted_bazel_rows() {
    let workspace = fixture_workspace("query-loading-thin-vertical");
    for (expression, expected) in [
        (
            "set(//nested/child:child //:root //app:app //nested:branch //app:via_alias)",
            "\napp\nnested\nnested/child\n",
        ),
        ("deps(//app:app)", "app\nlib\nnested\nnested/child\n"),
        ("loadfiles(//app:app)", "rules\n"),
    ] {
        let output = slug()
            .current_dir(&workspace)
            .args(["query", "--output=package", expression])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{expression}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty(), "{expression}: {output:?}");
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    }
}

#[test]
fn explicit_label_output_matches_bazel_and_text_is_rejected() {
    let workspace = fixture_workspace("query-path-topology");
    let expression = "allpaths(//:linear_start, //:linear_end)";
    let cases = [
        (
            "--order_output=auto",
            "//:linear_end\n//:linear_mid\n//:linear_start\n",
        ),
        (
            "--order_output=full",
            "//:linear_start\n//:linear_mid\n//:linear_end\n",
        ),
    ];

    for (order, expected) in cases {
        let output = slug()
            .current_dir(&workspace)
            .args(["query", "--output=label", order, expression])
            .output()
            .unwrap();
        assert!(output.status.success(), "{order}: {output:?}");
        assert!(output.stderr.is_empty(), "{order}: {output:?}");
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            expected,
            "{order}"
        );
    }

    let output_base = scratch("explicit-label-output-base");
    let _cleanup = DaemonCleanup(output_base.clone());
    let output_base_arg = format!("--output_base={}", output_base.display());
    for (order, expected) in cases {
        let output = slug()
            .current_dir(&workspace)
            .args([
                output_base_arg.as_str(),
                "query",
                "--output=label",
                order,
                expression,
            ])
            .output()
            .unwrap();
        assert!(output.status.success(), "{order}: {output:?}");
        assert!(output.stderr.is_empty(), "{order}: {output:?}");
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            expected,
            "{order}"
        );
    }

    let rejected = slug()
        .current_dir(&workspace)
        .args(["query", "--output=text", "//:linear_start"])
        .output()
        .unwrap();
    assert_eq!(rejected.status.code(), Some(2), "{rejected:?}");
    assert!(rejected.stdout.is_empty(), "{rejected:?}");
    let rejected_stderr = String::from_utf8_lossy(&rejected.stderr);
    assert!(
        rejected_stderr.contains("Invalid output format 'text'. Valid values are: label, label_kind, build, minrank, maxrank, package, location, graph, xml, proto, streamed_jsonproto, streamed_proto"),
        "{rejected:?}"
    );
}
