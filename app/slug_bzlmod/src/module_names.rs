/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the root directory of this source tree.
 * You may select, at your option, one of the above-listed licenses.
 */

pub(crate) fn is_valid_bazel_module_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_lowercase() {
        return false;
    }
    let last = name.chars().last().unwrap();
    if !(last.is_ascii_lowercase() || last.is_ascii_digit()) {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '-' | '_'))
}

pub(crate) fn invalid_bazel_module_name_message(name: &str) -> String {
    format!(
        "invalid module name '{name}': valid names must 1) only contain lowercase letters (a-z), digits (0-9), dots (.), hyphens (-), and underscores (_); 2) begin with a lowercase letter; 3) end with a lowercase letter or digit."
    )
}
