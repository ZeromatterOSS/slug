/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fmt;

use slug_identity_v2::CanonicalLabel;

use crate::toolchains::platform_constraints::ConstraintSet;
use crate::toolchains::platform_constraints::ExecutionPlatform;

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ToolchainType {
    label: CanonicalLabel,
}

impl ToolchainType {
    pub fn new(label: CanonicalLabel) -> Self {
        Self { label }
    }

    pub fn label(&self) -> &CanonicalLabel {
        &self.label
    }
}

impl fmt::Display for ToolchainType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ToolchainTarget {
    label: CanonicalLabel,
    toolchain_type: ToolchainType,
    target_compatible_with: ConstraintSet,
    exec_compatible_with: ConstraintSet,
}

impl ToolchainTarget {
    pub fn new(
        label: CanonicalLabel,
        toolchain_type: ToolchainType,
        target_compatible_with: ConstraintSet,
        exec_compatible_with: ConstraintSet,
    ) -> Self {
        Self {
            label,
            toolchain_type,
            target_compatible_with,
            exec_compatible_with,
        }
    }

    pub fn label(&self) -> &CanonicalLabel {
        &self.label
    }

    pub fn toolchain_type(&self) -> &ToolchainType {
        &self.toolchain_type
    }

    pub fn target_compatible_with(&self) -> &ConstraintSet {
        &self.target_compatible_with
    }

    pub fn exec_compatible_with(&self) -> &ConstraintSet {
        &self.exec_compatible_with
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct RegisteredToolchains {
    toolchains: Vec<ToolchainTarget>,
    execution_platforms: Vec<ExecutionPlatform>,
}

impl RegisteredToolchains {
    pub fn new(
        toolchains: Vec<ToolchainTarget>,
        execution_platforms: Vec<ExecutionPlatform>,
    ) -> Self {
        Self {
            toolchains,
            execution_platforms,
        }
    }

    pub fn toolchains(&self) -> &[ToolchainTarget] {
        &self.toolchains
    }

    pub fn execution_platforms(&self) -> &[ExecutionPlatform] {
        &self.execution_platforms
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct RegisteredToolchainsKey {
    bzlmod_resolution_digest: String,
    command_line_registration_digest: String,
}

impl RegisteredToolchainsKey {
    pub fn new(
        bzlmod_resolution_digest: impl Into<String>,
        command_line_registration_digest: impl Into<String>,
    ) -> Result<Self, String> {
        let key = Self {
            bzlmod_resolution_digest: bzlmod_resolution_digest.into(),
            command_line_registration_digest: command_line_registration_digest.into(),
        };
        key.validate()?;
        Ok(key)
    }

    pub fn stable_serialize(&self) -> String {
        format!(
            "bzlmod={};flags={}",
            self.bzlmod_resolution_digest, self.command_line_registration_digest
        )
    }

    fn validate(&self) -> Result<(), String> {
        for (name, value) in [
            ("bzlmod_resolution_digest", &self.bzlmod_resolution_digest),
            (
                "command_line_registration_digest",
                &self.command_line_registration_digest,
            ),
        ] {
            if value.is_empty() {
                return Err(format!("{name} must not be empty"));
            }
        }
        Ok(())
    }
}
