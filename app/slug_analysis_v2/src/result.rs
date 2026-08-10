/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use slug_build_api_v2::ActionSpec;
use slug_build_api_v2::ProviderCollection;
use slug_identity_v2::CanonicalLabel;
use slug_loading_v2::RuleCapability;

use crate::configured_target::ConfiguredEdge;
use crate::key::ConfigurationKind;
use crate::key::ConfiguredNodeKey;
use crate::key::ConfiguredTargetKey;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct AnalysisDiagnostic {
    severity: DiagnosticSeverity,
    message: String,
}

impl AnalysisDiagnostic {
    pub fn new(severity: DiagnosticSeverity, message: impl Into<String>) -> Self {
        Self {
            severity,
            message: message.into(),
        }
    }

    pub fn severity(&self) -> DiagnosticSeverity {
        self.severity
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub enum ConfiguredNodeKind {
    Rule,
    Alias,
    SourceFile,
    GeneratedFile,
    PackageGroup,
    Platform,
    ConstraintValue,
    ConstraintSetting,
    ToolchainType,
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct ToolchainSelection {
    execution_platform: ConfiguredTargetKey,
    declaration: CanonicalLabel,
    toolchain_type: ConfiguredTargetKey,
    implementation: ConfiguredTargetKey,
}

impl ToolchainSelection {
    pub fn new(
        execution_platform: ConfiguredTargetKey,
        declaration: CanonicalLabel,
        toolchain_type: ConfiguredTargetKey,
        implementation: ConfiguredTargetKey,
    ) -> Self {
        Self {
            execution_platform,
            declaration,
            toolchain_type,
            implementation,
        }
    }

    pub fn execution_platform(&self) -> &ConfiguredTargetKey {
        &self.execution_platform
    }

    pub fn declaration(&self) -> &CanonicalLabel {
        &self.declaration
    }

    pub fn toolchain_type(&self) -> &ConfiguredTargetKey {
        &self.toolchain_type
    }

    pub fn implementation(&self) -> &ConfiguredTargetKey {
        &self.implementation
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct ToolchainTopology {
    candidate_execution_platforms: Arc<[ConfiguredTargetKey]>,
    selection: Option<ToolchainSelection>,
}

impl ToolchainTopology {
    pub fn new(
        candidate_execution_platforms: Vec<ConfiguredTargetKey>,
        selection: Option<ToolchainSelection>,
    ) -> Result<Self, String> {
        if candidate_execution_platforms
            .iter()
            .any(|candidate| candidate.configuration().kind() != ConfigurationKind::Exec)
        {
            return Err("candidate execution platforms require exec configuration".to_owned());
        }
        if let Some(selection) = &selection
            && !candidate_execution_platforms.contains(selection.execution_platform())
        {
            return Err("selected execution platform is not a candidate".to_owned());
        }
        if selection.as_ref().is_some_and(|selection| {
            selection.toolchain_type().configuration().kind() != ConfigurationKind::Target
                || selection.implementation().configuration().kind() != ConfigurationKind::Target
        }) {
            return Err(
                "selected toolchain type and implementation require target configuration"
                    .to_owned(),
            );
        }
        Ok(Self {
            candidate_execution_platforms: candidate_execution_platforms.into(),
            selection,
        })
    }

    pub fn candidate_execution_platforms(&self) -> &[ConfiguredTargetKey] {
        &self.candidate_execution_platforms
    }

    pub fn selection(&self) -> Option<&ToolchainSelection> {
        self.selection.as_ref()
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct ConfiguredNodeResult {
    key: ConfiguredNodeKey,
    kind: ConfiguredNodeKind,
    providers: ProviderCollection,
    actions: Arc<[ActionSpec]>,
    declared_outputs: Arc<[CompactString]>,
    edges: Arc<[ConfiguredEdge]>,
    diagnostics: Arc<[AnalysisDiagnostic]>,
    rule_capability: Option<RuleCapability>,
    toolchain_topology: Option<ToolchainTopology>,
}

impl ConfiguredNodeResult {
    pub fn new_rule(
        key: ConfiguredTargetKey,
        providers: ProviderCollection,
        rule_capability: Option<RuleCapability>,
    ) -> Self {
        Self {
            key: key.into(),
            kind: ConfiguredNodeKind::Rule,
            providers,
            actions: Arc::from([]),
            declared_outputs: Arc::from([]),
            edges: Arc::from([]),
            diagnostics: Arc::from([]),
            rule_capability,
            toolchain_topology: None,
        }
    }

    pub(crate) fn new_native(
        key: ConfiguredNodeKey,
        kind: ConfiguredNodeKind,
        providers: ProviderCollection,
        rule_capability: Option<RuleCapability>,
    ) -> Self {
        assert_ne!(
            kind,
            ConfiguredNodeKind::Rule,
            "native nodes cannot be rules"
        );
        Self {
            key,
            kind,
            providers,
            actions: Arc::from([]),
            declared_outputs: Arc::from([]),
            edges: Arc::from([]),
            diagnostics: Arc::from([]),
            rule_capability,
            toolchain_topology: None,
        }
    }

    pub fn key(&self) -> &ConfiguredNodeKey {
        &self.key
    }
    pub fn kind(&self) -> &ConfiguredNodeKind {
        &self.kind
    }
    pub fn configured_target_key(&self) -> Option<&ConfiguredTargetKey> {
        self.key.configured_target()
    }

    pub fn providers(&self) -> &ProviderCollection {
        &self.providers
    }

    pub fn actions(&self) -> &[ActionSpec] {
        &self.actions
    }

    pub fn declared_outputs(&self) -> &[CompactString] {
        &self.declared_outputs
    }

    pub fn edges(&self) -> &[ConfiguredEdge] {
        &self.edges
    }
    pub fn configured_dependencies(&self) -> impl Iterator<Item = &ConfiguredTargetKey> {
        self.edges
            .iter()
            .filter_map(ConfiguredEdge::configured_target)
    }

    pub fn diagnostics(&self) -> &[AnalysisDiagnostic] {
        &self.diagnostics
    }

    pub fn rule_capability(&self) -> Option<&RuleCapability> {
        self.rule_capability.as_ref()
    }

    pub fn toolchain_topology(&self) -> Option<&ToolchainTopology> {
        self.toolchain_topology.as_ref()
    }

    pub fn with_actions(mut self, actions: Vec<ActionSpec>) -> Self {
        self.actions = actions.into();
        self
    }

    pub fn with_declared_outputs(mut self, declared_outputs: Vec<String>) -> Self {
        self.declared_outputs = declared_outputs
            .into_iter()
            .map(CompactString::from)
            .collect::<Vec<_>>()
            .into();
        self
    }

    pub fn with_edges(mut self, edges: Vec<ConfiguredEdge>) -> Self {
        self.edges = edges.into();
        self
    }

    pub fn with_diagnostics(mut self, diagnostics: Vec<AnalysisDiagnostic>) -> Self {
        self.diagnostics = diagnostics.into();
        self
    }

    pub fn with_toolchain_topology(mut self, topology: ToolchainTopology) -> Self {
        self.toolchain_topology = Some(topology);
        self
    }
}
