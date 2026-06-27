/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::BTreeMap;

use slug_identity_v2::CanonicalLabel;

use crate::toolchains::registered::ToolchainType;
use crate::toolchains::resolution::ToolchainResolution;

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct ResolvedToolchainContext {
    selected_execution_platform: Option<CanonicalLabel>,
    toolchains: BTreeMap<ToolchainType, CanonicalLabel>,
    events: Vec<String>,
}

impl ResolvedToolchainContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, toolchain_type: ToolchainType, resolution: ToolchainResolution) {
        self.selected_execution_platform = Some(resolution.selected_execution_platform().clone());
        self.toolchains
            .insert(toolchain_type, resolution.selected_toolchain().clone());
        self.events.extend_from_slice(resolution.events());
    }

    pub fn selected_execution_platform(&self) -> Option<&CanonicalLabel> {
        self.selected_execution_platform.as_ref()
    }

    pub fn toolchain(&self, toolchain_type: &ToolchainType) -> Option<&CanonicalLabel> {
        self.toolchains.get(toolchain_type)
    }

    pub fn events(&self) -> &[String] {
        &self.events
    }
}
