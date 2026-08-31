/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use allocative::Allocative;
use compact_str::CompactString;
use slug_identity_v2::CanonicalLabel;

use crate::key::ConfiguredNodeKey;
use crate::key::ConfiguredTargetKey;

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct ConfiguredEdge {
    target: ConfiguredNodeKey,
    kind: ConfiguredEdgeKind,
}

impl ConfiguredEdge {
    pub fn new(target: ConfiguredNodeKey, kind: ConfiguredEdgeKind) -> Self {
        Self { target, kind }
    }
    pub fn target(&self) -> &ConfiguredNodeKey {
        &self.target
    }
    pub fn kind(&self) -> &ConfiguredEdgeKind {
        &self.kind
    }
    pub fn configured_target(&self) -> Option<&ConfiguredTargetKey> {
        self.target.configured_target()
    }
    pub fn implicit(&self) -> bool {
        self.kind.implicit()
    }
    pub fn tool(&self) -> bool {
        self.kind.tool()
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub enum ConfiguredEdgeKind {
    OrdinaryAttribute {
        attribute: CompactString,
        index: u32,
    },
    TransitionedAttribute {
        attribute: CompactString,
        index: u32,
        output: CanonicalLabel,
    },
    ImplicitAttribute {
        attribute: CompactString,
        index: u32,
        tool: bool,
    },
    AliasActual,
    GeneratedBy,
    Source,
    DeclaringVisibility,
    PackageGroupInclude {
        index: u32,
    },
    ToolchainRequirement,
    SelectedToolchainImplementation,
    CandidateExecutionPlatform {
        index: u32,
    },
    HostPlatform,
    PlatformConstraint {
        index: u32,
    },
    ConstraintSetting,
    FunctionTransitionAllowlist,
}

impl ConfiguredEdgeKind {
    pub fn implicit(&self) -> bool {
        matches!(
            self,
            Self::ImplicitAttribute { .. }
                | Self::PackageGroupInclude { .. }
                | Self::ToolchainRequirement
                | Self::SelectedToolchainImplementation
                | Self::CandidateExecutionPlatform { .. }
                | Self::HostPlatform
                | Self::PlatformConstraint { .. }
                | Self::ConstraintSetting
                | Self::FunctionTransitionAllowlist
        )
    }
    pub const fn tool(&self) -> bool {
        matches!(self, Self::ImplicitAttribute { tool: true, .. })
    }
}
