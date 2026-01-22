/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use kuro_interpreter_for_build::interpreter::testing::Tester;

#[test]
fn test_attr_display() -> kuro_error::Result<()> {
    let mut tester = Tester::new().unwrap();
    tester.run_starlark_bzl_test(r#"
def assert_eq(a, b):
    if a != b:
        fail(a + " != " + b)

assert_eq(repr(attrs.bool(default = True)), "attrs.bool(default=True)")
assert_eq(repr(attrs.string()), "attrs.string()")
assert_eq(repr(attrs.list(attrs.string())), "attrs.list(attrs.string())")
assert_eq(repr(attrs.dict(attrs.string(), attrs.string())), "attrs.dict(attrs.string(), attrs.string(), sorted=False)")
assert_eq(repr(attrs.one_of(attrs.string())), "attrs.one_of(attrs.string())")
assert_eq(repr(attrs.tuple(attrs.string())), "attrs.tuple(attrs.string())")
assert_eq(repr(attrs.option(attrs.string())), "attrs.option(attrs.string())")

def test(): pass
"#)?;
    Ok(())
}

/// Test Bazel-compatible attr module (singular) is registered and functions work
#[test]
fn test_bazel_attr_module_registered() -> kuro_error::Result<()> {
    let mut tester = Tester::new().unwrap();
    tester.run_starlark_bzl_test(r#"
def test():
    # Verify the attr module (Bazel-style, singular) exists
    assert_eq(True, getattr(attr, "string") != None)
    assert_eq(True, getattr(attr, "int") != None)
    assert_eq(True, getattr(attr, "bool") != None)
    assert_eq(True, getattr(attr, "label") != None)
    assert_eq(True, getattr(attr, "label_list") != None)
    assert_eq(True, getattr(attr, "string_list") != None)
    assert_eq(True, getattr(attr, "int_list") != None)
    assert_eq(True, getattr(attr, "string_dict") != None)
    assert_eq(True, getattr(attr, "string_list_dict") != None)
    assert_eq(True, getattr(attr, "label_keyed_string_dict") != None)
    assert_eq(True, getattr(attr, "output") != None)
    assert_eq(True, getattr(attr, "output_list") != None)
"#)?;
    Ok(())
}

/// Test Bazel-compatible attr.string() function
#[test]
fn test_bazel_attr_string() -> kuro_error::Result<()> {
    let mut tester = Tester::new().unwrap();
    tester.run_starlark_bzl_test(r#"
def test():
    # attr.string() should create a string attribute
    s = attr.string()
    assert_eq("attrs.string()", repr(s))

    # With default value
    s2 = attr.string(default = "hello")
    assert_eq('attrs.string(default="hello")', repr(s2))

    # mandatory parameter is accepted but unused (Bazel compat)
    s3 = attr.string(mandatory = True)
    assert_eq("attrs.string()", repr(s3))
"#)?;
    Ok(())
}

/// Test Bazel-compatible attr.int() function
#[test]
fn test_bazel_attr_int() -> kuro_error::Result<()> {
    let mut tester = Tester::new().unwrap();
    tester.run_starlark_bzl_test(r#"
def test():
    i = attr.int()
    assert_eq("attrs.int()", repr(i))

    i2 = attr.int(default = 42)
    assert_eq("attrs.int(default=42)", repr(i2))
"#)?;
    Ok(())
}

/// Test Bazel-compatible attr.bool() function
#[test]
fn test_bazel_attr_bool() -> kuro_error::Result<()> {
    let mut tester = Tester::new().unwrap();
    tester.run_starlark_bzl_test(r#"
def test():
    b = attr.bool()
    assert_eq("attrs.bool()", repr(b))

    b2 = attr.bool(default = True)
    assert_eq("attrs.bool(default=True)", repr(b2))

    b3 = attr.bool(default = False)
    assert_eq("attrs.bool(default=False)", repr(b3))
"#)?;
    Ok(())
}

/// Test Bazel-compatible attr.label() function (maps to attrs.dep())
#[test]
fn test_bazel_attr_label() -> kuro_error::Result<()> {
    let mut tester = Tester::new().unwrap();
    tester.run_starlark_bzl_test(r#"
def test():
    # attr.label() creates a dependency attribute
    l = attr.label()
    assert_eq("attrs.dep()", repr(l))

    # With default (absolute target required)
    l2 = attr.label(default = "//foo:bar")
    assert_eq('attrs.dep(default="root//foo:bar")', repr(l2))
"#)?;
    Ok(())
}

/// Test Bazel-compatible attr.label_list() function
#[test]
fn test_bazel_attr_label_list() -> kuro_error::Result<()> {
    let mut tester = Tester::new().unwrap();
    tester.run_starlark_bzl_test(r#"
def test():
    ll = attr.label_list()
    assert_eq("attrs.list(attrs.dep())", repr(ll))

    ll2 = attr.label_list(default = [])
    assert_eq("attrs.list(attrs.dep(), default=[])", repr(ll2))
"#)?;
    Ok(())
}

/// Test Bazel-compatible attr.string_list() function
#[test]
fn test_bazel_attr_string_list() -> kuro_error::Result<()> {
    let mut tester = Tester::new().unwrap();
    tester.run_starlark_bzl_test(r#"
def test():
    sl = attr.string_list()
    assert_eq("attrs.list(attrs.string())", repr(sl))

    sl2 = attr.string_list(default = ["a", "b"])
    assert_eq('attrs.list(attrs.string(), default=["a", "b"])', repr(sl2))
"#)?;
    Ok(())
}

/// Test Bazel-compatible attr.int_list() function
#[test]
fn test_bazel_attr_int_list() -> kuro_error::Result<()> {
    let mut tester = Tester::new().unwrap();
    tester.run_starlark_bzl_test(r#"
def test():
    il = attr.int_list()
    assert_eq("attrs.list(attrs.int())", repr(il))

    il2 = attr.int_list(default = [1, 2, 3])
    assert_eq("attrs.list(attrs.int(), default=[1, 2, 3])", repr(il2))
"#)?;
    Ok(())
}

/// Test Bazel-compatible attr.string_dict() function
#[test]
fn test_bazel_attr_string_dict() -> kuro_error::Result<()> {
    let mut tester = Tester::new().unwrap();
    tester.run_starlark_bzl_test(r#"
def test():
    sd = attr.string_dict()
    assert_eq("attrs.dict(attrs.string(), attrs.string(), sorted=False)", repr(sd))

    sd2 = attr.string_dict(default = {"key": "value"})
    assert_eq('attrs.dict(attrs.string(), attrs.string(), sorted=False, default={"key": "value"})', repr(sd2))
"#)?;
    Ok(())
}

/// Test Bazel-compatible attr.string_list_dict() function
#[test]
fn test_bazel_attr_string_list_dict() -> kuro_error::Result<()> {
    let mut tester = Tester::new().unwrap();
    tester.run_starlark_bzl_test(r#"
def test():
    sld = attr.string_list_dict()
    assert_eq("attrs.dict(attrs.string(), attrs.list(attrs.string()), sorted=False)", repr(sld))
"#)?;
    Ok(())
}

/// Test Bazel-compatible attr.output() function
#[test]
fn test_bazel_attr_output() -> kuro_error::Result<()> {
    let mut tester = Tester::new().unwrap();
    tester.run_starlark_bzl_test(r#"
def test():
    o = attr.output()
    # Output is implemented as string attr in Kuro
    assert_eq("attrs.string()", repr(o))
"#)?;
    Ok(())
}

/// Test Bazel-compatible attr.output_list() function
#[test]
fn test_bazel_attr_output_list() -> kuro_error::Result<()> {
    let mut tester = Tester::new().unwrap();
    tester.run_starlark_bzl_test(r#"
def test():
    ol = attr.output_list()
    # Output list is implemented as list of strings
    assert_eq("attrs.list(attrs.string())", repr(ol))
"#)?;
    Ok(())
}
