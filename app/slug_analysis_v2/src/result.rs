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
use slug_loading_v2::RuleCapability;

use crate::configured_target::ConfiguredEdge;
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
}
