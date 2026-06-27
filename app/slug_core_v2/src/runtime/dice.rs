/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use super::RuntimeMode;

pub trait IncrementalEngine {
    fn runtime_mode(&self) -> RuntimeMode;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct OneShotIncrementalEngine;

impl IncrementalEngine for OneShotIncrementalEngine {
    fn runtime_mode(&self) -> RuntimeMode {
        RuntimeMode::OneShot
    }
}
