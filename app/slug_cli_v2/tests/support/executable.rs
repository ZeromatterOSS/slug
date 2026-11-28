/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::path::PathBuf;
use std::process::Command;

pub(crate) const INSTALLED_DAEMON: bool = cfg!(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
));

pub(super) fn slug_executable() -> PathBuf {
    if let Some(executable) = std::env::var_os("SLUG_V2_INSTALLED_CLI_BIN") {
        let executable = PathBuf::from(executable);
        assert!(
            INSTALLED_DAEMON && executable.is_absolute(),
            "SLUG_V2_INSTALLED_CLI_BIN requires an absolute installed CLI on Linux x86-64"
        );
        return executable;
    }
    let executable = PathBuf::from(env!("CARGO_BIN_EXE_slug"));
    if executable.is_absolute() {
        executable
    } else {
        std::env::current_dir().unwrap().join(executable)
    }
}

pub(crate) fn slug() -> Command {
    Command::new(slug_executable())
}
