/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

pub fn print_help() {
    println!(
        "Slug V2\n\nUSAGE:\n    slug <COMMAND> [ARGS...]\n\nCOMMANDS:\n    version    Print Slug V2 and Bazel compatibility identity\n    help       Print this help text\n    build      Planned Bazel-compatible build command\n    query      Planned Bazel-compatible query command\n    test       Planned Bazel-compatible test command\n    run        Planned Bazel-compatible run command"
    );
}
