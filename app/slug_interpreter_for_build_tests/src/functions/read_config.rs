/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_interpreter_for_build::interpreter::testing::Tester;

#[test]
fn test_read_config_not_supported() -> slug_error::Result<()> {
    let mut tester = Tester::new().unwrap();
    tester.run_starlark_test_expecting_error(
        r#"
def test():
    read_config("section", "key")
"#,
        "Buck2-specific function not available in Bazel-compatible mode",
    );
    Ok(())
}

#[test]
fn test_read_root_config_not_supported() -> slug_error::Result<()> {
    let mut tester = Tester::new().unwrap();
    tester.run_starlark_test_expecting_error(
        r#"
def test():
    read_root_config("section", "key")
"#,
        "Buck2-specific function not available in Bazel-compatible mode",
    );
    Ok(())
}
