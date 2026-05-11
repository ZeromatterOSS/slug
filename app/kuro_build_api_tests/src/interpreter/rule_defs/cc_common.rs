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
use kuro_build_api::interpreter::rule_defs::register_rule_defs;
use kuro_interpreter_for_build::interpreter::testing::Tester;

fn tester() -> kuro_error::Result<Tester> {
    let mut tester = Tester::new()?;
    tester.additional_globals(register_rule_defs);
    Ok(tester)
}

#[test]
fn cc_internal_freeze_preserves_list_type() -> kuro_error::Result<()> {
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
fn cc_common_create_linker_input_is_depset_eligible() -> kuro_error::Result<()> {
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
fn cc_common_compile_variables_carry_toolchain_target_identity() -> kuro_error::Result<()> {
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
fn cc_info_values_are_depset_eligible_inside_rust_dep_variant_shape() -> kuro_error::Result<()> {
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
