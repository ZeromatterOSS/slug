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
use slug_build_api_v2::ActionSpec;
use slug_build_api_v2::ProviderCollection;
use slug_loading_v2::RuleCapability;

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

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct AnalysisResult {
    key: ConfiguredTargetKey,
    providers: ProviderCollection,
    actions: Vec<ActionSpec>,
    declared_outputs: Vec<String>,
    direct_dependencies: Arc<[ConfiguredTargetKey]>,
    diagnostics: Vec<AnalysisDiagnostic>,
    rule_capability: Option<RuleCapability>,
}

impl AnalysisResult {
    pub fn new(
        key: ConfiguredTargetKey,
        providers: ProviderCollection,
        rule_capability: Option<RuleCapability>,
    ) -> Self {
        Self {
            key,
            providers,
            actions: Vec::new(),
            declared_outputs: Vec::new(),
            direct_dependencies: Arc::from([]),
            diagnostics: Vec::new(),
            rule_capability,
        }
    }

    pub fn key(&self) -> &ConfiguredTargetKey {
        &self.key
    }

    pub fn providers(&self) -> &ProviderCollection {
        &self.providers
    }

    pub fn actions(&self) -> &[ActionSpec] {
        &self.actions
    }

    pub fn declared_outputs(&self) -> &[String] {
        &self.declared_outputs
    }

    pub fn direct_dependencies(&self) -> &[ConfiguredTargetKey] {
        &self.direct_dependencies
    }

    pub fn diagnostics(&self) -> &[AnalysisDiagnostic] {
        &self.diagnostics
    }

    pub fn rule_capability(&self) -> Option<&RuleCapability> {
        self.rule_capability.as_ref()
    }

    pub fn with_actions(mut self, actions: Vec<ActionSpec>) -> Self {
        self.actions = actions;
        self
    }

    pub fn with_declared_outputs(mut self, declared_outputs: Vec<String>) -> Self {
        self.declared_outputs = declared_outputs;
        self
    }

    pub fn with_direct_dependencies(
        mut self,
        direct_dependencies: Vec<ConfiguredTargetKey>,
    ) -> Self {
        self.direct_dependencies = direct_dependencies.into();
        self
    }

    pub fn with_diagnostics(mut self, diagnostics: Vec<AnalysisDiagnostic>) -> Self {
        self.diagnostics = diagnostics;
        self
    }
}
