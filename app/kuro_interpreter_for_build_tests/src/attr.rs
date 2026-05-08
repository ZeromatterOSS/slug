/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Tests for Bazel-compatible attr.* module.
//!
//! Note: The attr.* functions map to internal attrs.* types, so repr() shows
//! the underlying attrs.* format. This is an implementation detail.

use std::sync::Arc;

use dupe::Dupe;
use indoc::indoc;
use kuro_build_api::interpreter::rule_defs::provider::registration::register_builtin_providers;
use kuro_common::package_listing::listing::PackageListing;
use kuro_common::package_listing::listing::testing::PackageListingExt;
use kuro_core::build_file_path::BuildFilePath;
use kuro_core::bzl::ImportPath;
use kuro_core::cells::cell_path_with_allowed_relative_dir::CellPathWithAllowedRelativeDir;
use kuro_core::cells::name::CellName;
use kuro_core::cells::paths::CellRelativePath;
use kuro_core::package::PackageLabel;
use kuro_core::package::package_relative_path::PackageRelativePathBuf;
use kuro_core::plugins::PluginKindSet;
use kuro_core::target::label::interner::ConcurrentTargetLabelInterner;
use kuro_interpreter_for_build::attrs::coerce::attr_type::AttrTypeExt;
use kuro_interpreter_for_build::attrs::coerce::ctx::BuildAttrCoercionContext;
use kuro_interpreter_for_build::interpreter::testing::Tester;
use kuro_interpreter_for_build::interpreter::testing::cells;
use kuro_node::attrs::attr_type::AttrType;
use kuro_node::attrs::coerced_attr::CoercedAttr;
use kuro_node::attrs::coerced_path::CoercedPath;
use kuro_node::attrs::coercion_context::AttrCoercionContext;
use kuro_node::attrs::configurable::AttrIsConfigurable;
use kuro_node::attrs::hacks::value_to_string;
use kuro_node::attrs::inspect_options::AttrInspectOptions;
use kuro_node::provider_id_set::ProviderIdSet;
use starlark::values::Heap;

// =============================================================================
// Bazel-compatible attr.* module tests
// =============================================================================

#[test]
fn test_attr_module_registered() -> kuro_error::Result<()> {
    let mut tester = Tester::new().unwrap();
    tester.run_starlark_bzl_test(indoc!(
        r#"
        def test():
            # Bazel-style attr module should be available
            assert_eq(True, getattr(attr, "string") != None)
            assert_eq(True, getattr(attr, "int") != None)
            assert_eq(True, getattr(attr, "bool") != None)
            assert_eq(True, getattr(attr, "label") != None)
            assert_eq(True, getattr(attr, "label_list") != None)
            assert_eq(True, getattr(attr, "string_list") != None)
            assert_eq(True, getattr(attr, "int_list") != None)
            assert_eq(True, getattr(attr, "string_dict") != None)
            assert_eq(True, getattr(attr, "output") != None)
            assert_eq(True, getattr(attr, "output_list") != None)
        "#
    ))
}

#[test]
fn attr_string_works() -> kuro_error::Result<()> {
    let mut tester = Tester::new().unwrap();
    tester.run_starlark_bzl_test(indoc!(
        r#"
        frozen = attr.string(default="something", doc = "foo")
        def test():
            # attr.string() should work (maps to attrs.string internally)
            assert_eq('attrs.string(default="something")', repr(attr.string(default="something", doc = "foo")))
            assert_eq('attrs.string(default="something")', repr(frozen))
            # mandatory parameter should be accepted (even if not enforced at this level)
            attr.string(mandatory=True)
        "#
    ))
}

#[test]
fn attr_int_works() -> kuro_error::Result<()> {
    let mut tester = Tester::new().unwrap();
    tester.run_starlark_bzl_test(indoc!(
        r#"
        frozen = attr.int(default=42)
        def test():
            assert_eq('attrs.int(default=42)', repr(attr.int(default=42, doc = "foo")))
            assert_eq('attrs.int(default=42)', repr(frozen))
            # mandatory parameter should be accepted
            attr.int(mandatory=True)
        "#
    ))
}

#[test]
fn attr_bool_works() -> kuro_error::Result<()> {
    let mut tester = Tester::new().unwrap();
    tester.run_starlark_bzl_test(indoc!(
        r#"
        frozen = attr.bool(default=False)
        def test():
            assert_eq('attrs.bool(default=True)', repr(attr.bool(default=True, doc = "foo")))
            assert_eq('attrs.bool(default=False)', repr(frozen))
            # mandatory parameter should be accepted
            attr.bool(mandatory=True)
        "#
    ))
}

#[test]
fn attr_label_works() -> kuro_error::Result<()> {
    let mut tester = Tester::new().unwrap();
    tester.run_starlark_bzl_test(indoc!(
        r#"
        frozen = attr.label(default="root//foo:bar")
        def test():
            # attr.label() creates a dependency attribute (maps to attrs.dep internally)
            assert_eq('attrs.dep(default="root//foo:bar")', repr(attr.label(default="//foo:bar")))
            assert_eq('attrs.dep(default="root//foo:bar")', repr(frozen))
            # Bazel-specific parameters should be accepted
            attr.label(mandatory=True)
            attr.label(executable=True)
            attr.label(allow_files=True)
            attr.label(allow_single_file=True)
        "#
    ))?;

    // In Bazel/Buck2, relative label defaults using ":" prefix are valid.
    // Bare names without ":" are NOT valid (requires explicit ":" or "//" prefix).
    let mut t = Tester::new().unwrap();
    t.run_starlark_bzl_test(indoc!(
        r#"
        def test():
            attr.label(default=":reltarget")
        "#
    ))
    .unwrap();
    // Bare names (no ":" prefix) should fail with a pattern parse error.
    t.run_starlark_bzl_test_expecting_error(
        indoc!(
            r#"
            def test():
                attr.label(default="notatarget")
            "#
        ),
        "Invalid target pattern",
    );
    Ok(())
}

#[test]
fn attr_label_list_works() -> kuro_error::Result<()> {
    let mut tester = Tester::new().unwrap();
    tester.run_starlark_bzl_test(indoc!(
        r#"
        frozen = attr.label_list(default=["root//foo:bar"])
        def test():
            # attr.label_list() creates a list of dependency attributes
            assert_eq('attrs.list(attrs.dep(), default=["root//foo:bar"])', repr(attr.label_list(default=["//foo:bar"])))
            assert_eq('attrs.list(attrs.dep(), default=["root//foo:bar"])', repr(frozen))
            # Empty default
            assert_eq('attrs.list(attrs.dep(), default=[])', repr(attr.label_list(default=[])))
            # Bazel-specific parameters should be accepted
            attr.label_list(mandatory=True)
            attr.label_list(allow_files=True)
        "#
    ))
}

#[test]
fn attr_label_list_allow_files_accepts_directory_sources() -> kuro_error::Result<()> {
    let mut tester = Tester::new().unwrap();
    tester.additional_globals(register_builtin_providers);
    tester.add_import(
        &ImportPath::testing_new("root//:rules.bzl"),
        indoc!(
            r#"
            def _impl(ctx):
                return DefaultInfo()

            dir_headers = rule(
                impl = _impl,
                attrs = {
                    "hdrs": attr.label_list(allow_files = True),
                },
            )
            "#
        ),
    )?;

    let build_path = BuildFilePath::testing_new("root//some/package:BUILD.bazel");
    let result = tester.eval_build_file(
        &build_path,
        indoc!(
            r#"
            load("//:rules.bzl", "dir_headers")

            dir_headers(
                name = "headers",
                hdrs = ["include"],
            )
            "#
        ),
        PackageListing::testing_files(&["include/a.h", "include/bits/b.h"]),
    )?;

    let target = result
        .get_target(kuro_core::target::name::TargetNameRef::new("headers")?)
        .expect("target should be recorded");
    let hdrs = target
        .attr_or_none("hdrs", AttrInspectOptions::All)
        .expect("hdrs attr should be present");

    let CoercedAttr::List(items) = hdrs.value else {
        panic!("expected list attr, got {:?}", hdrs.value);
    };
    assert_eq!(items.len(), 1);

    let CoercedAttr::OneOf(inner, _) = &items[0] else {
        panic!("expected one_of attr, got {:?}", items[0]);
    };
    let CoercedAttr::SourceFile(CoercedPath::Directory(dir)) = &**inner else {
        panic!("expected directory source, got {:?}", inner);
    };

    assert_eq!("include", dir.dir.as_str());
    let files = dir
        .files
        .iter()
        .map(|path| path.as_str().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(vec!["include/a.h", "include/bits/b.h"], files);

    Ok(())
}

#[test]
fn attr_string_list_works() -> kuro_error::Result<()> {
    let mut tester = Tester::new().unwrap();
    tester.run_starlark_bzl_test(indoc!(
        r#"
        frozen = attr.string_list(default=["a", "b"])
        def test():
            assert_eq('attrs.list(attrs.string(), default=["a", "b"])', repr(attr.string_list(default=["a", "b"])))
            assert_eq('attrs.list(attrs.string(), default=["a", "b"])', repr(frozen))
            # Empty default
            assert_eq('attrs.list(attrs.string(), default=[])', repr(attr.string_list(default=[])))
            # mandatory parameter should be accepted
            attr.string_list(mandatory=True)
        "#
    ))
}

#[test]
fn attr_int_list_works() -> kuro_error::Result<()> {
    let mut tester = Tester::new().unwrap();
    tester.run_starlark_bzl_test(indoc!(
        r#"
        frozen = attr.int_list(default=[1, 2, 3])
        def test():
            assert_eq('attrs.list(attrs.int(), default=[1, 2, 3])', repr(attr.int_list(default=[1, 2, 3])))
            assert_eq('attrs.list(attrs.int(), default=[1, 2, 3])', repr(frozen))
            # mandatory parameter should be accepted
            attr.int_list(mandatory=True)
        "#
    ))
}

#[test]
fn attr_string_dict_works() -> kuro_error::Result<()> {
    let mut tester = Tester::new().unwrap();
    tester.run_starlark_bzl_test(indoc!(
        r#"
        frozen = attr.string_dict(default={"key": "value"})
        def test():
            assert_eq('attrs.dict(attrs.string(), attrs.string(), sorted=False, default={"key": "value"})', repr(attr.string_dict(default={"key": "value"})))
            assert_eq('attrs.dict(attrs.string(), attrs.string(), sorted=False, default={"key": "value"})', repr(frozen))
            # mandatory parameter should be accepted
            attr.string_dict(mandatory=True)
        "#
    ))
}

#[test]
fn attr_string_list_dict_works() -> kuro_error::Result<()> {
    let mut tester = Tester::new().unwrap();
    tester.run_starlark_bzl_test(indoc!(
        r#"
        frozen = attr.string_list_dict(default={"key": ["a", "b"]})
        def test():
            assert_eq('attrs.dict(attrs.string(), attrs.list(attrs.string()), sorted=False, default={"key": ["a", "b"]})', repr(attr.string_list_dict(default={"key": ["a", "b"]})))
            assert_eq('attrs.dict(attrs.string(), attrs.list(attrs.string()), sorted=False, default={"key": ["a", "b"]})', repr(frozen))
            # mandatory parameter should be accepted
            attr.string_list_dict(mandatory=True)
        "#
    ))
}

#[test]
fn attr_output_works() -> kuro_error::Result<()> {
    let mut tester = Tester::new().unwrap();
    tester.run_starlark_bzl_test(indoc!(
        r#"
        frozen = attr.output(default="out.txt")
        def test():
            # attr.output() for declaring output files (maps to attrs.string internally)
            assert_eq('attrs.string(default="out.txt")', repr(attr.output(default="out.txt")))
            assert_eq('attrs.string(default="out.txt")', repr(frozen))
            # mandatory parameter should be accepted
            attr.output(mandatory=True)
        "#
    ))
}

#[test]
fn attr_output_list_works() -> kuro_error::Result<()> {
    let mut tester = Tester::new().unwrap();
    tester.run_starlark_bzl_test(indoc!(
        r#"
        frozen = attr.output_list(default=["a.txt", "b.txt"])
        def test():
            assert_eq('attrs.list(attrs.string(), default=["a.txt", "b.txt"])', repr(attr.output_list(default=["a.txt", "b.txt"])))
            assert_eq('attrs.list(attrs.string(), default=["a.txt", "b.txt"])', repr(frozen))
            # mandatory parameter should be accepted
            attr.output_list(mandatory=True)
        "#
    ))
}

// =============================================================================
// Internal coercion tests (implementation details, not API tests)
// =============================================================================

#[test]
fn attr_coercer_coerces() -> kuro_error::Result<()> {
    Heap::temp(|heap| {
        let some_cells = cells(None)?;
        let cell_resolver = some_cells.1;
        let cell_alias_resolver = some_cells.0;
        let package = PackageLabel::new(
            CellName::testing_new("root"),
            CellRelativePath::unchecked_new("foo"),
        )?;
        let enclosing_package = (package.dupe(), PackageListing::testing_empty());
        let coercer_ctx = BuildAttrCoercionContext::new_with_package(
            cell_resolver,
            cell_alias_resolver,
            enclosing_package,
            false,
            Arc::new(ConcurrentTargetLabelInterner::default()),
            CellPathWithAllowedRelativeDir::backwards_relative_not_supported(
                package.as_cell_path().to_owned(),
            ),
        );

        // Test label coercion (used by attr.label())
        let label_coercer = AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY);
        let label_value1 = label_coercer.coerce(
            AttrIsConfigurable::Yes,
            &coercer_ctx,
            heap.alloc("root//foo:bar"),
        )?;
        let label_value2 = label_coercer.coerce(
            AttrIsConfigurable::Yes,
            &coercer_ctx,
            heap.alloc("root//foo:bar[baz]"),
        )?;
        let label_value3 =
            label_coercer.coerce(AttrIsConfigurable::Yes, &coercer_ctx, heap.alloc(":bar"))?;
        let invalid_label_value1 = label_coercer.coerce(
            AttrIsConfigurable::Yes,
            &coercer_ctx,
            heap.alloc("root//foo/..."),
        );
        let invalid_label_value2 = label_coercer.coerce(
            AttrIsConfigurable::Yes,
            &coercer_ctx,
            heap.alloc("root//foo:"),
        );

        assert_eq!(
            "root//foo:bar",
            value_to_string(&label_value1, package.dupe())?
        );
        assert_eq!(
            "root//foo:bar[baz]",
            value_to_string(&label_value2, package.dupe())?
        );
        assert_eq!(
            "root//foo:bar",
            value_to_string(&label_value3, package.dupe())?
        );
        assert!(invalid_label_value1.is_err());
        assert!(invalid_label_value2.is_err());

        // Test string coercion (used by attr.string())
        let string_coercer = AttrType::string();
        let string_value1 =
            string_coercer.coerce(AttrIsConfigurable::Yes, &coercer_ctx, heap.alloc("str"))?;
        assert_eq!("str", value_to_string(&string_value1, package.dupe())?);

        Ok(())
    })
}

#[test]
fn coercing_src_to_path_works() -> kuro_error::Result<()> {
    let cell_resolver = cells(None).unwrap().1;
    let cell_alias_resolver = cells(None).unwrap().0;
    let package = PackageLabel::new(
        CellName::testing_new("root"),
        CellRelativePath::unchecked_new("foo/bar"),
    )?;
    let package_ctx = BuildAttrCoercionContext::new_with_package(
        cell_resolver.dupe(),
        cell_alias_resolver.dupe(),
        (
            package.dupe(),
            PackageListing::testing_files(&["baz/quz.cpp"]),
        ),
        false,
        Arc::new(ConcurrentTargetLabelInterner::default()),
        CellPathWithAllowedRelativeDir::backwards_relative_not_supported(
            package.as_cell_path().to_owned(),
        ),
    );
    let no_package_ctx = BuildAttrCoercionContext::new_no_package(
        cell_resolver,
        CellName::testing_new("root"),
        cell_alias_resolver,
        Arc::new(ConcurrentTargetLabelInterner::default()),
    );

    let err = no_package_ctx
        .coerce_path("baz/quz.cpp", false)
        .unwrap_err();
    assert!(err.to_string().contains("Expected a package"));

    let err = package_ctx
        .coerce_path("/invalid/absolute/path", false)
        .unwrap_err();
    assert!(format!("{err:#}").contains("absolute path"), "{err:?}");

    let err = package_ctx
        .coerce_path("../upward/traversal", false)
        .unwrap_err();
    assert!(err.to_string().contains("normalized path"));

    let expected = PackageRelativePathBuf::unchecked_new("baz/quz.cpp".to_owned());
    assert_eq!(
        expected.as_path(),
        &**package_ctx
            .coerce_path("baz/quz.cpp", false)
            .unwrap()
            .path()
    );
    Ok(())
}
