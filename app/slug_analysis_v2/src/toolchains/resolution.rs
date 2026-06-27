/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::error::Error;
use std::fmt;

use slug_identity_v2::CanonicalLabel;

use crate::toolchains::platform_constraints::ConstraintSet;
use crate::toolchains::platform_constraints::ExecutionPlatform;
use crate::toolchains::registered::RegisteredToolchains;
use crate::toolchains::registered::ToolchainTarget;
use crate::toolchains::registered::ToolchainType;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ToolchainResolutionRequest {
    toolchain_type: ToolchainType,
    target_platform: ConstraintSet,
    registered: RegisteredToolchains,
}

impl ToolchainResolutionRequest {
    pub fn new(
        toolchain_type: ToolchainType,
        target_platform: ConstraintSet,
        registered: RegisteredToolchains,
    ) -> Self {
        Self {
            toolchain_type,
            target_platform,
            registered,
        }
    }

    pub fn resolve(&self) -> Result<ToolchainResolution, ToolchainResolutionError> {
        let mut events = Vec::new();
        for platform in self.registered.execution_platforms() {
            events.push(format!("consider execution platform {}", platform.label()));
            if let Some(toolchain) = self.first_matching_toolchain(platform, &mut events) {
                events.push(format!(
                    "selected toolchain {} on execution platform {}",
                    toolchain.label(),
                    platform.label()
                ));
                return Ok(ToolchainResolution {
                    selected_execution_platform: platform.label().clone(),
                    selected_toolchain: toolchain.label().clone(),
                    events,
                });
            }
        }

        Err(ToolchainResolutionError::MandatoryToolchainMissing {
            toolchain_type: self.toolchain_type.clone(),
            events,
        })
    }

    fn first_matching_toolchain<'a>(
        &'a self,
        platform: &ExecutionPlatform,
        events: &mut Vec<String>,
    ) -> Option<&'a ToolchainTarget> {
        self.registered.toolchains().iter().find(|toolchain| {
            if toolchain.toolchain_type() != &self.toolchain_type {
                return false;
            }
            if !toolchain
                .target_compatible_with()
                .is_satisfied_by(&self.target_platform)
            {
                events.push(format!(
                    "reject toolchain {}: target constraints do not match",
                    toolchain.label()
                ));
                return false;
            }
            if !toolchain
                .exec_compatible_with()
                .is_satisfied_by(platform.constraints())
            {
                events.push(format!(
                    "reject toolchain {}: execution constraints do not match {}",
                    toolchain.label(),
                    platform.label()
                ));
                return false;
            }
            true
        })
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ToolchainResolution {
    selected_execution_platform: CanonicalLabel,
    selected_toolchain: CanonicalLabel,
    events: Vec<String>,
}

impl ToolchainResolution {
    pub fn selected_execution_platform(&self) -> &CanonicalLabel {
        &self.selected_execution_platform
    }

    pub fn selected_toolchain(&self) -> &CanonicalLabel {
        &self.selected_toolchain
    }

    pub fn events(&self) -> &[String] {
        &self.events
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ToolchainResolutionError {
    MandatoryToolchainMissing {
        toolchain_type: ToolchainType,
        events: Vec<String>,
    },
}

impl ToolchainResolutionError {
    pub fn events(&self) -> &[String] {
        match self {
            Self::MandatoryToolchainMissing { events, .. } => events,
        }
    }
}

impl fmt::Display for ToolchainResolutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MandatoryToolchainMissing { toolchain_type, .. } => {
                write!(
                    f,
                    "mandatory toolchain type {toolchain_type} was not resolved"
                )
            }
        }
    }
}

impl Error for ToolchainResolutionError {}
