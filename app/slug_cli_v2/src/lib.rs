/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

pub mod commands;

#[cfg(all(feature = "native-probe-observer", not(test)))]
compile_error!("native-probe-observer is available only to the slug_cli_v2 library test");

#[cfg(test)]
mod payload_demand_probe;

pub fn main() -> i32 {
    commands::dispatch(std::env::args())
}
