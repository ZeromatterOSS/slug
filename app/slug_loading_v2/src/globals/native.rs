/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

pub fn removed_language_rule(name: &str) -> bool {
    matches!(
        name,
        "cc_binary"
            | "cc_library"
            | "cc_test"
            | "java_library"
            | "py_binary"
            | "py_library"
            | "py_test"
    )
}
