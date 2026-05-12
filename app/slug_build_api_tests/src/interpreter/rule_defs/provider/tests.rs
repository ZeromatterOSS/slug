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
use slug_build_api::interpreter::rule_defs::provider::callable::register_provider;
use slug_build_api::interpreter::rule_defs::register_rule_defs;
use slug_core::bzl::ImportPath;
use slug_interpreter_for_build::interpreter::testing::Tester;

fn provider_tester() -> Tester {
    let mut tester = Tester::new().unwrap();
    tester.additional_globals(register_rule_defs);
    tester.additional_globals(register_provider);
    tester
}

#[test]
fn creates_providers() -> slug_error::Result<()> {
    // TODO(nmj): Starlark doesn't let you call 'new_invoker()' on is_mutable types.
    //                 Once that's fixed, make sure we can call 'FooInfo' before the module is
    //                 frozen.
    let mut tester = provider_tester();

    tester.run_starlark_test(indoc!(
        r#"
    FooInfo = provider(fields=["bar", "baz"])
    FooInfo2 = FooInfo
    #frozen_foo_1 = FooInfo(bar="bar_f1", baz="baz_f1")
    #frozen_foo_2 = FooInfo(bar="bar_f2")

    assert_eq('provider(fields={"f1": provider_field(typing.Any)})', repr(provider(fields=["f1"])))
    assert_eq('provider[FooInfo](fields={"bar": provider_field(typing.Any), "baz": provider_field(typing.Any)})', repr(FooInfo))
    assert_eq('provider[FooInfo](fields={"bar": provider_field(typing.Any), "baz": provider_field(typing.Any)})', repr(FooInfo2))

    def test():
        assert_eq('provider[FooInfo](fields={"bar": provider_field(typing.Any), "baz": provider_field(typing.Any)})', repr(FooInfo))
        assert_eq('provider[FooInfo](fields={"bar": provider_field(typing.Any), "baz": provider_field(typing.Any)})', repr(FooInfo2))

        #assert_eq("FooInfo(bar=\"bar_f1\", baz=\"baz_f1\")", repr(frozen_foo1))
        #assert_eq("bar_f1", frozen_foo1.bar)
        #assert_eq("baz_f1", frozen_foo1.baz)
        #assert_eq("FooInfo(bar=\"bar_f2\", baz=None)", repr(frozen_foo2))
        #assert_eq("bar_f2", frozen_foo2.bar)
        #assert_eq(None, frozen_foo2.baz)

        foo_1 = FooInfo(bar="bar_1", baz="baz_1")
        foo_2 = FooInfo(bar="bar_2")

        assert_eq('provider[FooInfo](fields={"bar": provider_field(typing.Any), "baz": provider_field(typing.Any)})', repr(FooInfo))
        assert_eq("FooInfo(bar=\"bar_1\", baz=\"baz_1\")", repr(foo_1))
        assert_eq("bar_1", foo_1.bar)
        assert_eq("baz_1", foo_1.baz)
        assert_eq("FooInfo(bar=\"bar_2\")", repr(foo_2))
        assert_eq("bar_2", foo_2.bar)
        assert_eq(False, hasattr(foo_2, "baz"))
        assert_eq("fallback", getattr(foo_2, "baz", "fallback"))

        assert_eq("{\"bar\":\"bar_1\",\"baz\":\"baz_1\"}", json.encode(foo_1))
    "#
    ))?;

    tester.run_starlark_test_expecting_error(
        indoc!(
            r#"
    FooInfo = provider(fields=["bar", "baz"])

    def test_compile_time():
        foo_1 = FooInfo(bar="bar1")
        foo_1.quz

    def test():
        pass
    "#
        ),
        "The attribute `quz` is not available on the type `FooInfo`",
    );

    tester.run_starlark_test_expecting_error(
        indoc!(
            r#"
    list = []
    list.append(provider(fields=["bar", "baz"]))
    "#
        ),
        "must be assigned to a variable",
    );

    // Make sure that frozen UserProvider instances work
    let mut tester = provider_tester();
    tester.add_import(
        &ImportPath::testing_new("root//provider:def1.bzl"),
        indoc!(
            r#"
            FooInfo = provider(fields=["foo"])
            "#
        ),
    )?;
    tester.add_import(
        &ImportPath::testing_new("root//provider:def2.bzl"),
        indoc!(
            r#"
            load("//provider:def1.bzl", "FooInfo")
            foo = FooInfo(foo="foo1")
            "#
        ),
    )?;
    tester.run_starlark_test(indoc!(
        r#"
        load("//provider:def2.bzl", "foo")
        def test():
            assert_eq('FooInfo(foo="foo1")', repr(foo))
        "#
    ))?;

    Ok(())
}

#[test]
fn test_schema_provider_missing_fields_are_absent() -> slug_error::Result<()> {
    let mut tester = provider_tester();
    tester.run_starlark_test(indoc!(
        r#"
    ListInfo = provider(fields=["present", "missing"])
    DictInfo = provider(fields={"present": "doc", "missing": "doc"})

    def test():
        list_info = ListInfo(present="yes")
        dict_info = DictInfo(present="yes")
        explicit_none = ListInfo(present="yes", missing=None)

        for info in [list_info, dict_info]:
            assert_eq(["present"], dir(info))
            assert_eq(True, hasattr(info, "present"))
            assert_eq(False, hasattr(info, "missing"))
            assert_eq("fallback", getattr(info, "missing", "fallback"))

        assert_eq(["missing", "present"], dir(explicit_none))
        assert_eq(True, hasattr(explicit_none, "missing"))
        assert_eq(None, explicit_none.missing)
    "#
    ))?;
    Ok(())
}

#[test]
fn test_builtin_provider_as_type() {
    let mut tester = provider_tester();
    tester
        .run_starlark_bzl_test(indoc!(
            r#"
        def test():
            configuration_info = ConfigurationInfo(constraints={}, values={})
            assert_true(isinstance(configuration_info, ConfigurationInfo))
            assert_true(isinstance(configuration_info, Provider))
            assert_true(not isinstance(configuration_info, DefaultInfo))
        "#
        ))
        .unwrap();
}

#[test]
fn test_user_defined_provider_as_type() {
    let mut tester = provider_tester();
    tester
        .run_starlark_bzl_test(indoc!(
            r#"
            FooInfo = provider(fields=["foo"])
            def test():
                foo = FooInfo(foo="foo1")
                assert_true(isinstance(foo, FooInfo))
                assert_true(isinstance(foo, Provider))
                assert_true(not isinstance(foo, DefaultInfo))
            "#
        ))
        .unwrap();
}

#[test]
fn test_user_defined_provider_creator_typecheck() {
    let mut tester = provider_tester();
    tester.run_starlark_bzl_test_expecting_error(
        indoc!(
            r#"
                Pkg = provider(fields = ["x"])

                def create_pkg():
                    return Pkg(y = 1)

                def test():
                    pass
    "#
        ),
        "Unexpected parameter named `y`",
    );
}

#[test]
fn test_provider_non_unique_fields() {
    let mut tester = provider_tester();
    tester.run_starlark_bzl_test_expecting_error(
        indoc!(
            r#"
                Pkg = provider(fields = ["x", "x"])

                def test():
                    pass
            "#
        ),
        "non-unique field names",
    )
}
