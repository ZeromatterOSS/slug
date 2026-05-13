/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use indoc::indoc;
use slug_build_api::interpreter::rule_defs::register_rule_defs;
use slug_interpreter_for_build::interpreter::testing::Tester;

use crate::interpreter::rule_defs::artifact::testing::artifactory;

fn tester() -> slug_error::Result<Tester> {
    let mut tester = Tester::new()?;
    tester.additional_globals(register_rule_defs);
    tester.additional_globals(artifactory);
    Ok(tester)
}

#[test]
fn cc_internal_freeze_preserves_list_type() -> slug_error::Result<()> {
    let mut positive = tester()?;
    positive.run_starlark_bzl_test(indoc!(
        r#"
        P = provider(fields = ["x"])

        def test():
            cc_internal = cc_common.internal_DO_NOT_USE

            frozen_list = cc_internal.freeze(["a"])
            frozen_tuple = cc_internal.freeze(("a",))
            frozen_depset = cc_internal.freeze(depset(["a"]))
            compilation_outputs = cc_common.create_compilation_outputs(
                objects = depset(["a"]),
                pic_objects = depset(["pic"]),
            )

            assert_eq("list", type(frozen_list))
            assert_eq(["a", "b"], frozen_list + ["b"])
            assert_eq(["z", "a"], ["z"] + frozen_list)
            assert_eq("list", type(frozen_tuple))
            assert_eq(["a", "b"], frozen_tuple + ["b"])
            assert_eq("depset", type(frozen_depset))
            assert_eq("list", type(compilation_outputs.objects))
            assert_eq(["a", "b"], compilation_outputs.objects + ["b"])
            assert_eq(["a"], depset(compilation_outputs.objects).to_list())
            assert_eq("list", type(compilation_outputs.pic_objects))
            assert_eq(["pic", "more"], compilation_outputs.pic_objects + ["more"])
            assert_eq(["pic"], depset(compilation_outputs.pic_objects).to_list())
            depset([compilation_outputs])

            frozen_provider = P(x = cc_internal.freeze(["a"]))
            assert_eq("list", type(frozen_provider.x))
            assert_eq(["a", "b"], frozen_provider.x + ["b"])
            depset([frozen_provider])

            frozen_transitive = cc_internal.freeze([depset(["transitive"])])
            assert_eq(["transitive", "direct"], depset(["direct"], transitive = frozen_transitive).to_list())
        "#
    ))?;
    Ok(())
}

#[test]
fn cc_common_create_linker_input_is_depset_eligible() -> slug_error::Result<()> {
    let mut positive = tester()?;
    positive.run_starlark_bzl_test(indoc!(
        r#"
        def test():
            library_to_link = cc_common.create_library_to_link(
                actions = None,
                feature_configuration = None,
                cc_toolchain = None,
                static_library = "libexample.a",
                objects = depset(["example.o"]),
            )

            linker_input = cc_common.create_linker_input(
                owner = Label("//pkg:owner"),
                libraries = depset([library_to_link]),
                user_link_flags = ["-Wl,example", ["-Wl,nested"]],
                additional_inputs = depset(["input.script"]),
            )
            assert_eq("list", type(linker_input.user_link_flags))
            assert_eq(
                ["-Wl,example", "-Wl,nested", "-Wl,more"],
                linker_input.user_link_flags + ["-Wl,more"],
            )

            linker_inputs = depset([linker_input])
            linking_context = cc_common.create_linking_context(linker_inputs = linker_inputs)
            assert_eq([linker_input], linking_context.linker_inputs.to_list())
        "#
    ))?;
    Ok(())
}

#[test]
fn cc_common_compile_variables_carry_toolchain_target_identity() -> slug_error::Result<()> {
    let mut positive = tester()?;
    positive.run_starlark_bzl_test(indoc!(
        r#"
        def test():
            cc_toolchain = struct(
                target_gnu_system_name = "x86_64-linux-musl",
                libc = "musl",
            )
            variables = cc_common.create_compile_variables(
                feature_configuration = None,
                cc_toolchain = cc_toolchain,
            )
            assert_eq("x86_64-linux-musl", variables.target_system_name)
            assert_eq("musl", variables.target_libc)
        "#
    ))?;
    Ok(())
}

#[test]
fn cc_common_feature_env_sets_expand_for_matching_action() -> slug_error::Result<()> {
    let mut positive = tester()?;
    positive.run_starlark_bzl_test(indoc!(
        r#"
        def test():
            feature = struct(
                name = "musl_env",
                enabled = False,
                env_sets = [
                    struct(
                        actions = ["c++-compile"],
                        with_features = [
                            struct(features = ["musl_env"], not_features = ["blocked"]),
                        ],
                        env_entries = [
                            struct(
                                key = "CC",
                                value = "clang --target=%{target_system_name}",
                                expand_if_available = "target_system_name",
                            ),
                            struct(
                                key = "CFLAGS",
                                value = "--libc=%{target_libc}",
                                expand_if_available = "target_libc",
                            ),
                            struct(
                                key = "SKIP_ME",
                                value = "%{missing}",
                                expand_if_available = "missing",
                            ),
                        ],
                    ),
                    struct(
                        actions = ["c++-link-executable"],
                        with_features = [],
                        env_entries = [
                            struct(key = "LINK_ONLY", value = "1", expand_if_available = None),
                        ],
                    ),
                ],
            )
            features = cc_common.internal_DO_NOT_USE.cc_toolchain_features(
                toolchain_config_info = struct(
                    _features_DO_NOT_USE = [feature],
                    _action_configs_DO_NOT_USE = [],
                ),
                tools_directory = "",
            )
            fc = features.configure_features(requested_features = ["musl_env"])
            variables = cc_common.create_compile_variables(
                feature_configuration = fc,
                cc_toolchain = struct(
                    target_gnu_system_name = "x86_64-linux-musl",
                    libc = "musl",
                ),
            )
            env = cc_common.get_environment_variables(
                feature_configuration = fc,
                action_name = "c++-compile",
                variables = variables,
            )

            assert_eq("clang --target=x86_64-linux-musl", env["CC"])
            assert_eq("--libc=musl", env["CFLAGS"])
            assert_eq(False, "SKIP_ME" in env)
            assert_eq(False, "LINK_ONLY" in env)

            blocked = features.configure_features(requested_features = ["musl_env", "blocked"])
            blocked_env = cc_common.get_environment_variables(
                feature_configuration = blocked,
                action_name = "c++-compile",
                variables = variables,
            )
            assert_eq(False, "CC" in blocked_env)
        "#
    ))?;
    Ok(())
}

#[test]
fn cc_common_flag_sets_expand_action_configs_before_features() -> slug_error::Result<()> {
    let mut positive = tester()?;
    positive.run_starlark_bzl_test(indoc!(
        r#"
        def test():
            action_config = struct(
                action_name = "c++-link-executable",
                flag_sets = [
                    struct(
                        actions = [],
                        with_features = [],
                        flag_groups = [
                            struct(
                                flags = ["-resource-dir", "bazel-out/resource_directory"],
                                flag_groups = [],
                                iterate_over = None,
                                expand_if_available = None,
                                expand_if_not_available = None,
                                expand_if_true = None,
                                expand_if_false = None,
                                expand_if_equal = None,
                            ),
                        ],
                    ),
                ],
            )
            feature = struct(
                name = "runtime_paths",
                enabled = True,
                env_sets = [],
                flag_sets = [
                    struct(
                        actions = ["c++-link-executable"],
                        with_features = [
                            struct(features = ["runtime_paths"], not_features = ["blocked"]),
                        ],
                        flag_groups = [
                            struct(
                                flags = ["-target", "%{target_system_name}"],
                                flag_groups = [],
                                iterate_over = None,
                                expand_if_available = "target_system_name",
                                expand_if_not_available = None,
                                expand_if_true = None,
                                expand_if_false = None,
                                expand_if_equal = None,
                            ),
                            struct(
                                flags = ["-L%{library_search_directories}"],
                                flag_groups = [],
                                iterate_over = "library_search_directories",
                                expand_if_available = "library_search_directories",
                                expand_if_not_available = None,
                                expand_if_true = None,
                                expand_if_false = None,
                                expand_if_equal = None,
                            ),
                        ],
                    ),
                ],
            )
            features = cc_common.internal_DO_NOT_USE.cc_toolchain_features(
                toolchain_config_info = struct(
                    _features_DO_NOT_USE = [feature],
                    _action_configs_DO_NOT_USE = [action_config],
                ),
                tools_directory = "",
            )
            fc = features.configure_features(requested_features = ["runtime_paths"])
            variables = cc_common.create_link_variables(
                feature_configuration = fc,
                cc_toolchain = struct(
                    target_gnu_system_name = "x86_64-linux-gnu",
                    libc = "gnu",
                ),
                output_file = "out",
                library_search_directories = ["bazel-out/lib/a", "bazel-out/lib/b"],
            )
            cmd = cc_common.get_memory_inefficient_command_line(
                feature_configuration = fc,
                action_name = "c++-link-executable",
                variables = variables,
            )

            assert_eq([
                "-o",
                "out",
                "-resource-dir",
                "bazel-out/resource_directory",
                "-target",
                "x86_64-linux-gnu",
                "-Lbazel-out/lib/a",
                "-Lbazel-out/lib/b",
            ], cmd)

            blocked = features.configure_features(requested_features = ["runtime_paths", "blocked"])
            blocked_cmd = cc_common.get_memory_inefficient_command_line(
                feature_configuration = blocked,
                action_name = "c++-link-executable",
                variables = variables,
            )
            assert_eq([
                "-o",
                "out",
                "-resource-dir",
                "bazel-out/resource_directory",
            ], blocked_cmd)
        "#
    ))?;
    Ok(())
}

#[test]
fn cc_toolchain_features_configure_honors_pre_filtered_requested_features() -> slug_error::Result<()>
{
    let mut positive = tester()?;
    positive.run_starlark_bzl_test(indoc!(
        r#"
        def test():
            unsupported_default = struct(
                name = "module_maps",
                enabled = True,
                env_sets = [],
                flag_sets = [],
            )
            supported_default = struct(
                name = "supports_pic",
                enabled = True,
                env_sets = [],
                flag_sets = [],
            )
            features = cc_common.internal_DO_NOT_USE.cc_toolchain_features(
                toolchain_config_info = struct(
                    _features_DO_NOT_USE = [unsupported_default, supported_default],
                    _action_configs_DO_NOT_USE = [],
                ),
                tools_directory = "",
            )

            assert_eq(["module_maps", "supports_pic"], features.default_features_and_action_configs())

            fc = features.configure_features(requested_features = ["supports_pic"])
            assert_eq(False, fc.is_enabled("module_maps"))
            assert_eq(True, fc.is_enabled("supports_pic"))

            public_fc = cc_common.configure_features(
                ctx = struct(features = [], disabled_features = []),
                cc_toolchain = struct(_toolchain_features = features),
                requested_features = [],
                unsupported_features = ["module_maps"],
            )
            assert_eq(False, public_fc.is_enabled("module_maps"))
            assert_eq(True, public_fc.is_enabled("supports_pic"))
        "#
    ))?;
    Ok(())
}

#[test]
fn cc_common_modern_toolchain_args_expand_for_link_action() -> slug_error::Result<()> {
    let mut positive = tester()?;
    positive.run_starlark_bzl_test(indoc!(
        r#"
        def test():
            action = struct(name = "c++-link-executable")
            nested = struct(
                legacy_flag_group = struct(
                    flags = ["-resource-dir", "bazel-out/resource_directory"],
                    flag_groups = [],
                    iterate_over = None,
                    expand_if_available = None,
                    expand_if_not_available = None,
                    expand_if_true = None,
                    expand_if_false = None,
                    expand_if_equal = None,
                ),
            )
            args_info = struct(
                actions = depset([action]),
                requires_any_of = [],
                nested = nested,
            )
            features = cc_common.internal_DO_NOT_USE.cc_toolchain_features(
                toolchain_config_info = struct(
                    features = [],
                    enabled_features = [],
                    args = struct(
                        by_action = [
                            struct(action = action, args = [args_info]),
                        ],
                    ),
                ),
                tools_directory = "",
            )
            fc = features.configure_features(requested_features = [])
            variables = cc_common.create_link_variables(
                feature_configuration = fc,
                cc_toolchain = struct(
                    target_gnu_system_name = "x86_64-linux-gnu",
                    libc = "gnu",
                ),
                output_file = "out",
            )
            cmd = cc_common.get_memory_inefficient_command_line(
                feature_configuration = fc,
                action_name = "c++-link-executable",
                variables = variables,
            )
            assert_eq(["-o", "out", "-resource-dir", "bazel-out/resource_directory"], cmd)
        "#
    ))?;
    Ok(())
}

#[test]
fn cc_common_static_archive_uses_feature_args_instead_of_fallback_prefix() -> slug_error::Result<()>
{
    let mut positive = tester()?;
    positive.run_starlark_bzl_test(indoc!(
        r#"
        def test():
            action = struct(name = "c++-link-static-library")
            nested = struct(
                legacy_flag_group = struct(
                    flags = ["rcsD", "%{output_execpath}"],
                    flag_groups = [],
                    iterate_over = None,
                    expand_if_available = None,
                    expand_if_not_available = None,
                    expand_if_true = None,
                    expand_if_false = None,
                    expand_if_equal = None,
                ),
            )
            archive_args = struct(
                actions = depset([action]),
                requires_any_of = [],
                nested = nested,
            )
            features = cc_common.internal_DO_NOT_USE.cc_toolchain_features(
                toolchain_config_info = struct(
                    features = [],
                    enabled_features = [],
                    args = struct(
                        by_action = [
                            struct(action = action, args = [archive_args]),
                        ],
                    ),
                ),
                tools_directory = "",
            )
            fc = features.configure_features(requested_features = [])
            variables = cc_common.create_link_variables(
                feature_configuration = fc,
                cc_toolchain = struct(
                    target_gnu_system_name = "x86_64-linux-gnu",
                    libc = "gnu",
                ),
                output_file = "libout.a",
            )
            cmd = cc_common.get_memory_inefficient_command_line(
                feature_configuration = fc,
                action_name = "c++-link-static-library",
                variables = variables,
            )
            assert_eq(["rcsD", "libout.a"], cmd)
        "#
    ))?;
    Ok(())
}

#[test]
fn cc_common_link_expands_linker_input_locations_and_inputs() -> slug_error::Result<()> {
    let mut positive = tester()?;
    positive.run_starlark_bzl_test(indoc!(
        r#"
        def test():
            captured = []

            def declare_file(name):
                return struct(
                    path = "buck-out/gen/pkg/" + name,
                    as_output = lambda: "out:" + name,
                )

            def run(args, outputs = [], inputs = [], category = None, identifier = None, progress_message = None):
                captured.append(struct(args = args, inputs = inputs))

            actions = struct(declare_file = declare_file, run = run)
            features = cc_common.internal_DO_NOT_USE.cc_toolchain_features(
                toolchain_config_info = struct(features = [], enabled_features = [], args = struct(by_action = [])),
                tools_directory = "",
            )
            fc = features.configure_features(requested_features = [])
            map_file = bound_artifact("//pkg:all_map", "buck-out/gen/external/llvm/runtimes/glibc/build/all.map")
            linker_input = cc_common.create_linker_input(
                owner = Label("//pkg:owner"),
                user_link_flags = ["-Wl,--version-script=$(location :all.map)"],
                additional_inputs = depset([map_file]),
            )
            linking_context = cc_common.create_linking_context(
                linker_inputs = depset([linker_input]),
            )

            cc_common.link(
                actions = actions,
                name = "libexample",
                cc_toolchain = struct(
                    target_gnu_system_name = "x86_64-linux-gnu",
                    libc = "gnu",
                ),
                feature_configuration = fc,
                output_type = "dynamic_library",
                linking_contexts = [linking_context],
            )

            assert_eq(1, len(captured))
            version_script_flags = [
                arg
                for arg in captured[0].args
                if type(arg) == "string" and arg.startswith("-Wl,--version-script=")
            ]
            assert_eq(1, len(version_script_flags))
            assert_eq(True, version_script_flags[0].endswith("/all.map"))
            assert_eq(False, "$(" in version_script_flags[0])
            assert_eq([map_file], captured[0].inputs)
        "#
    ))?;
    Ok(())
}

#[test]
fn cc_common_link_expands_direct_user_link_flags_before_feature_expansion() -> slug_error::Result<()>
{
    let mut positive = tester()?;
    positive.run_starlark_bzl_test(indoc!(
        r#"
        def test():
            captured = []

            def declare_file(name):
                return struct(
                    path = "buck-out/gen/pkg/" + name,
                    as_output = lambda: "out:" + name,
                )

            def run(args, outputs = [], inputs = [], category = None, identifier = None, progress_message = None):
                captured.append(struct(args = args, inputs = inputs))

            action = struct(name = "c++-link-dynamic-library")
            nested = struct(
                legacy_flag_group = struct(
                    flags = ["%{user_link_flags}"],
                    flag_groups = [],
                    iterate_over = "user_link_flags",
                    expand_if_available = "user_link_flags",
                    expand_if_not_available = None,
                    expand_if_true = None,
                    expand_if_false = None,
                    expand_if_equal = None,
                ),
            )
            args_info = struct(
                actions = depset([action]),
                requires_any_of = [],
                nested = nested,
            )
            features = cc_common.internal_DO_NOT_USE.cc_toolchain_features(
                toolchain_config_info = struct(
                    features = [],
                    enabled_features = [],
                    args = struct(
                        by_action = [
                            struct(action = action, args = [args_info]),
                        ],
                    ),
                ),
                tools_directory = "",
            )
            fc = features.configure_features(requested_features = [])
            map_file = bound_artifact("//pkg:all_map", "buck-out/gen/external/llvm/runtimes/glibc/build/all.map")

            cc_common.link(
                actions = struct(declare_file = declare_file, run = run),
                name = "libexample",
                cc_toolchain = struct(
                    target_gnu_system_name = "x86_64-linux-gnu",
                    libc = "gnu",
                ),
                feature_configuration = fc,
                output_type = "dynamic_library",
                user_link_flags = ["-Wl,--version-script=$(location :all.map)"],
                additional_inputs = [map_file],
            )

            assert_eq(1, len(captured))
            version_script_flags = [
                arg
                for arg in captured[0].args
                if type(arg) == "string" and arg.startswith("-Wl,--version-script=")
            ]
            assert_eq(1, len(version_script_flags))
            assert_eq(True, version_script_flags[0].endswith("/all.map"))
            assert_eq(False, "$(" in version_script_flags[0])
            assert_eq([map_file], captured[0].inputs)
        "#
    ))?;
    Ok(())
}

#[test]
fn cc_common_public_configure_features_preserves_toolchain_flag_sets() -> slug_error::Result<()> {
    let mut positive = tester()?;
    positive.run_starlark_bzl_test(indoc!(
        r#"
        def test():
            action_config = struct(
                action_name = "c++-link-executable",
                flag_sets = [
                    struct(
                        actions = [],
                        with_features = [],
                        flag_groups = [
                            struct(
                                flags = ["-resource-dir", "bazel-out/resource_directory"],
                                flag_groups = [],
                                iterate_over = None,
                                expand_if_available = None,
                                expand_if_not_available = None,
                                expand_if_true = None,
                                expand_if_false = None,
                                expand_if_equal = None,
                            ),
                        ],
                    ),
                ],
            )
            features = cc_common.internal_DO_NOT_USE.cc_toolchain_features(
                toolchain_config_info = struct(
                    features = [],
                    enabled_features = [],
                    action_configs = [action_config],
                ),
                tools_directory = "",
            )
            fc = cc_common.configure_features(
                ctx = struct(features = [], disabled_features = []),
                cc_toolchain = struct(_toolchain_features = features),
                requested_features = [],
                unsupported_features = [],
            )
            variables = cc_common.create_link_variables(
                feature_configuration = fc,
                cc_toolchain = struct(
                    target_gnu_system_name = "x86_64-linux-gnu",
                    libc = "gnu",
                ),
                output_file = "out",
            )
            cmd = cc_common.get_memory_inefficient_command_line(
                feature_configuration = fc,
                action_name = "c++-link-executable",
                variables = variables,
            )
            assert_eq(["-o", "out", "-resource-dir", "bazel-out/resource_directory"], cmd)
        "#
    ))?;
    Ok(())
}

#[test]
fn cc_info_values_are_depset_eligible_inside_rust_dep_variant_shape() -> slug_error::Result<()> {
    let mut positive_tester = tester()?;
    positive_tester.run_starlark_bzl_test(indoc!(
        r#"
        DepVariantInfo = provider(fields = [
            "crate_info",
            "dep_info",
            "build_info",
            "cc_info",
            "crate_group_info",
        ])
        CcCompilationContextInfo = provider(fields = ["_header_info"])
        RulesCcInfo = provider(fields = ["compilation_context"])

        def test():
            cc = CcInfo()
            depset([cc])

            dep = DepVariantInfo(
                crate_info = None,
                dep_info = None,
                build_info = None,
                cc_info = cc,
                crate_group_info = None,
            )
            assert_eq([dep], depset([dep]).to_list())

            header_info = cc_common.internal_DO_NOT_USE.create_header_info()
            compilation_context = CcCompilationContextInfo(_header_info = header_info)
            rules_cc_info = RulesCcInfo(compilation_context = compilation_context)
            rules_cc_dep = DepVariantInfo(
                crate_info = None,
                dep_info = None,
                build_info = None,
                cc_info = rules_cc_info,
                crate_group_info = None,
            )
            assert_eq([rules_cc_dep], depset([rules_cc_dep]).to_list())
        "#
    ))?;

    let mut negative = tester()?;
    negative.run_starlark_bzl_test_expecting_error(
        indoc!(
            r#"
            PlainInfo = provider(fields = ["items"])

            def test():
                depset([PlainInfo(items = [])])
            "#
        ),
        "depset elements must not be mutable values",
    );

    Ok(())
}
