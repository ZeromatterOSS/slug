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

use crate::toolchains::context::ResolvedToolchainContext;
use crate::toolchains::registered::ToolchainType;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ExecGroup {
    name: String,
    toolchain_types: Vec<ToolchainType>,
    exec_properties: BTreeMap<String, String>,
}

impl ExecGroup {
    pub fn new(name: impl Into<String>, toolchain_types: Vec<ToolchainType>) -> Self {
        Self {
            name: name.into(),
            toolchain_types,
            exec_properties: BTreeMap::new(),
        }
    }

    pub fn with_exec_properties(mut self, exec_properties: BTreeMap<String, String>) -> Self {
        self.exec_properties = exec_properties;
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn toolchain_types(&self) -> &[ToolchainType] {
        &self.toolchain_types
    }

    pub fn exec_properties(&self) -> &BTreeMap<String, String> {
        &self.exec_properties
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct ExecGroupCollection {
    groups: BTreeMap<String, ExecGroup>,
    resolved_contexts: BTreeMap<String, ResolvedToolchainContext>,
}

impl ExecGroupCollection {
    pub fn new(groups: Vec<ExecGroup>) -> Self {
        let groups = groups
            .into_iter()
            .map(|group| (group.name.clone(), group))
            .collect();
        Self {
            groups,
            resolved_contexts: BTreeMap::new(),
        }
    }

    pub fn group(&self, name: &str) -> Option<&ExecGroup> {
        self.groups.get(name)
    }

    pub fn set_resolved_context(
        &mut self,
        name: impl Into<String>,
        context: ResolvedToolchainContext,
    ) {
        self.resolved_contexts.insert(name.into(), context);
    }

    pub fn resolved_context(&self, name: &str) -> Option<&ResolvedToolchainContext> {
        self.resolved_contexts.get(name)
    }
}
