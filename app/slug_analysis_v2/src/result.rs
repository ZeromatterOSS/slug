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
use std::collections::BTreeSet;
use std::ops::Deref;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use slug_build_api_v2::ActionKind;
use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
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

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct PlatformSemanticFact {
    pub exec_properties: Arc<[(CompactString, CompactString)]>,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub enum ConfiguredActionExecGroup {
    Default,
    Named(CompactString),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub enum ConfiguredActionAspectProvenance {
    Absent,
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct ConfiguredActionPlatformConstraint(ConfiguredTargetKey, ConfiguredTargetKey);

impl ConfiguredActionPlatformConstraint {
    pub fn new(value: ConfiguredTargetKey, setting: ConfiguredTargetKey) -> Self {
        Self(value, setting)
    }

    pub fn constraint_value(&self) -> &ConfiguredTargetKey {
        &self.0
    }

    pub fn constraint_setting(&self) -> &ConfiguredTargetKey {
        &self.1
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct ConfiguredActionToolchainContext(ToolchainSelection, CompactString);

impl ConfiguredActionToolchainContext {
    pub fn new(selection: ToolchainSelection, marker: CompactString) -> Self {
        Self(selection, marker)
    }

    pub fn selection(&self) -> &ToolchainSelection {
        &self.0
    }

    pub fn marker(&self) -> &str {
        &self.1
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Allocative)]
pub enum ConfiguredActionExecutionState {
    SelectedToolchain,
    SelectedPlatformOnly,
    UnresolvedDefault,
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct ConfiguredActionOwnerContext {
    owner: ConfiguredTargetKey,
    exec_group: ConfiguredActionExecGroup,
    execution_platform: Option<ConfiguredTargetKey>,
    platform_fact: Option<PlatformSemanticFact>,
    platform_constraints: Arc<[ConfiguredActionPlatformConstraint]>,
    toolchain: Option<Arc<ConfiguredActionToolchainContext>>,
    aspect: ConfiguredActionAspectProvenance,
}

impl ConfiguredActionOwnerContext {
    pub fn unresolved_default(owner: ConfiguredTargetKey) -> Result<Self, String> {
        ensure_action(
            is_target_configured(&owner),
            "configured action owner requires target configuration",
        )?;
        Ok(Self {
            owner,
            exec_group: ConfiguredActionExecGroup::Default,
            execution_platform: None,
            platform_fact: None,
            platform_constraints: Arc::new([]),
            toolchain: None,
            aspect: ConfiguredActionAspectProvenance::Absent,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        owner: ConfiguredTargetKey,
        exec_group: ConfiguredActionExecGroup,
        execution_platform: ConfiguredTargetKey,
        platform_fact: PlatformSemanticFact,
        target_exec_properties: &BTreeMap<String, String>,
        group_exec_properties: &BTreeMap<String, String>,
        platform_constraints: Vec<ConfiguredActionPlatformConstraint>,
        toolchain: Option<Arc<ConfiguredActionToolchainContext>>,
        aspect: ConfiguredActionAspectProvenance,
    ) -> Result<Self, String> {
        ensure_action(
            is_target_configured(&owner),
            "configured action owner requires target configuration",
        )?;
        ensure_action(
            execution_platform.configuration().kind() == ConfigurationKind::Exec
                && execution_platform
                    .configuration()
                    .slug_configuration()
                    .is_some(),
            "configured action platform requires structural exec configuration",
        )?;
        if let Some(toolchain) = &toolchain {
            ensure_action(
                toolchain.selection().execution_platform() == &execution_platform,
                "configured action toolchain has mismatched platform",
            )?;
            ensure_action(
                [
                    toolchain.selection().toolchain_type(),
                    toolchain.selection().implementation(),
                ]
                .into_iter()
                .all(|key| {
                    key.configuration() == owner.configuration() && is_target_configured(key)
                }),
                "configured action toolchain requires owner target configuration",
            )?;
        }
        ensure_action(
            platform_fact
                .exec_properties
                .windows(2)
                .all(|pair| pair[0].0 < pair[1].0),
            "configured action platform properties require unique key order",
        )?;
        ensure_action(
            platform_constraints.iter().all(|constraint| {
                [
                    constraint.constraint_value(),
                    constraint.constraint_setting(),
                ]
                .into_iter()
                .all(|key| key.configuration() == execution_platform.configuration())
            }),
            "configured action constraint requires selected exec configuration",
        )?;
        let mut seen_settings = BTreeSet::new();
        ensure_action(
            platform_constraints
                .iter()
                .all(|constraint| seen_settings.insert(constraint.constraint_setting().clone())),
            "configured action platform has duplicate constraint setting",
        )?;
        let exec_properties = merge_exec_properties(
            &platform_fact.exec_properties,
            target_exec_properties,
            group_exec_properties,
        );
        Ok(Self {
            owner,
            exec_group,
            execution_platform: Some(execution_platform),
            platform_fact: Some(PlatformSemanticFact { exec_properties }),
            platform_constraints: platform_constraints.into(),
            toolchain,
            aspect,
        })
    }

    pub fn owner(&self) -> &ConfiguredTargetKey {
        &self.owner
    }

    pub fn exec_group(&self) -> &ConfiguredActionExecGroup {
        &self.exec_group
    }

    pub fn execution_state(&self) -> ConfiguredActionExecutionState {
        match (self.execution_platform.is_some(), self.toolchain.is_some()) {
            (false, _) => ConfiguredActionExecutionState::UnresolvedDefault,
            (true, true) => ConfiguredActionExecutionState::SelectedToolchain,
            (true, false) => ConfiguredActionExecutionState::SelectedPlatformOnly,
        }
    }

    pub fn execution_platform(&self) -> Option<&ConfiguredTargetKey> {
        self.execution_platform.as_ref()
    }

    pub fn platform_fact(&self) -> Option<&PlatformSemanticFact> {
        self.platform_fact.as_ref()
    }

    pub fn platform_constraints(&self) -> &[ConfiguredActionPlatformConstraint] {
        &self.platform_constraints
    }

    pub fn toolchain(&self) -> Option<&Arc<ConfiguredActionToolchainContext>> {
        self.toolchain.as_ref()
    }

    pub fn aspect(&self) -> ConfiguredActionAspectProvenance {
        self.aspect
    }
}

fn is_target_configured(key: &ConfiguredTargetKey) -> bool {
    key.configuration().kind() == ConfigurationKind::Target
}

fn ensure_action(condition: bool, message: &'static str) -> Result<(), String> {
    condition.then_some(()).ok_or_else(|| message.to_owned())
}

fn merge_exec_properties(
    platform: &Arc<[(CompactString, CompactString)]>,
    target: &BTreeMap<String, String>,
    group: &BTreeMap<String, String>,
) -> Arc<[(CompactString, CompactString)]> {
    if target.is_empty() && group.is_empty() {
        return platform.clone();
    }
    let mut merged = platform.iter().cloned().collect::<BTreeMap<_, _>>();
    merged.extend(
        target
            .iter()
            .chain(group)
            .map(|(key, value)| (CompactString::from(key), CompactString::from(value))),
    );
    merged.into_iter().collect::<Vec<_>>().into()
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct ConfiguredAction {
    spec: ActionSpec,
    context: Arc<ConfiguredActionOwnerContext>,
}

impl ConfiguredAction {
    pub fn context(&self) -> &Arc<ConfiguredActionOwnerContext> {
        &self.context
    }
}

impl Deref for ConfiguredAction {
    type Target = ActionSpec;

    fn deref(&self) -> &Self::Target {
        &self.spec
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ConfiguredActionView<'a>(&'a ConfiguredAction);

impl<'a> ConfiguredActionView<'a> {
    pub fn owner(&self) -> &'a ConfiguredTargetKey {
        self.0.context.owner()
    }

    pub fn spec(&self) -> &'a ActionSpec {
        &self.0.spec
    }

    pub fn output(&self) -> &'a ActionOutput {
        &self.0.spec.outputs()[0]
    }

    pub fn execution_platform(&self) -> &'a ConfiguredTargetKey {
        self.0
            .context
            .execution_platform()
            .expect("configured action view validates a selected platform")
    }

    pub fn exec_group(&self) -> &'a ConfiguredActionExecGroup {
        self.0.context.exec_group()
    }

    pub fn platform_fact(&self) -> &'a PlatformSemanticFact {
        self.0
            .context
            .platform_fact()
            .expect("configured action view validates a selected platform")
    }

    pub fn platform_constraints(&self) -> &'a [ConfiguredActionPlatformConstraint] {
        self.0.context.platform_constraints()
    }

    pub fn toolchain(&self) -> Option<&'a ConfiguredActionToolchainContext> {
        self.0.context.toolchain().map(Arc::as_ref)
    }

    pub fn context(&self) -> &'a Arc<ConfiguredActionOwnerContext> {
        self.0.context()
    }

    pub fn execution_state(&self) -> ConfiguredActionExecutionState {
        self.0.context.execution_state()
    }

    pub fn aspect(&self) -> ConfiguredActionAspectProvenance {
        self.0.context.aspect()
    }
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
    actions: Arc<[ConfiguredAction]>,
    declared_outputs: Arc<[CompactString]>,
    edges: Arc<[ConfiguredEdge]>,
    diagnostics: Arc<[AnalysisDiagnostic]>,
    rule_capability: Option<RuleCapability>,
    toolchain_topology: Option<ToolchainTopology>,
    platform_semantic_fact: Option<PlatformSemanticFact>,
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
            platform_semantic_fact: None,
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
            platform_semantic_fact: None,
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

    pub fn actions(&self) -> &[ConfiguredAction] {
        &self.actions
    }

    pub fn configured_file_write_actions(
        &self,
    ) -> Result<impl ExactSizeIterator<Item = ConfiguredActionView<'_>>, &'static str> {
        for action in self.actions.iter() {
            let spec: &ActionSpec = action;
            if action.context.execution_platform().is_none() {
                return Err("configured FileWrite action requires a selected execution platform");
            }
            if !matches!(spec.kind(), ActionKind::Write { .. }) {
                return Err("configured action view supports only FileWrite actions");
            }
            if !matches!(spec.outputs(), [output] if output.kind() == ActionOutputKind::File) {
                return Err("configured FileWrite action requires exactly one file output");
            }
            if !spec.argv().is_empty()
                || !spec.inputs().is_empty()
                || !spec.tools().is_empty()
                || !spec.param_files().is_empty()
                || !spec.env().is_empty()
                || !spec.execution_requirements().is_empty()
                || !spec.exec_properties().is_empty()
                || spec.progress_message().is_some()
            {
                return Err("configured FileWrite action has unsupported execution fields");
            }
        }
        Ok(self.actions.iter().map(ConfiguredActionView))
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

    pub fn platform_semantic_fact(&self) -> Option<&PlatformSemanticFact> {
        self.platform_semantic_fact.as_ref()
    }

    pub fn with_action_specs(
        mut self,
        specs: Vec<ActionSpec>,
        contexts: Vec<Arc<ConfiguredActionOwnerContext>>,
    ) -> Result<Self, String> {
        if specs.is_empty() {
            return Ok(self);
        }
        let owner = self
            .configured_target_key()
            .ok_or_else(|| "configured action owner is not a configured target".to_owned())?;
        if contexts.iter().any(|context| context.owner() != owner) {
            return Err("configured action context has mismatched owner".to_owned());
        }
        let context_count = contexts.len();
        let by_group = contexts
            .into_iter()
            .map(|context| (context.exec_group().clone(), context))
            .collect::<BTreeMap<_, _>>();
        if by_group.len() != context_count {
            return Err("configured action contexts contain duplicate group".to_owned());
        }
        self.actions = specs
            .into_iter()
            .map(|spec| {
                let group = spec
                    .exec_group()
                    .map_or(ConfiguredActionExecGroup::Default, |name| {
                        ConfiguredActionExecGroup::Named(CompactString::from(name))
                    });
                let context = by_group.get(&group).cloned().ok_or_else(|| {
                    "configured action has no matching exec-group context".to_owned()
                })?;
                Ok(ConfiguredAction { spec, context })
            })
            .collect::<Result<Vec<_>, String>>()?
            .into();
        Ok(self)
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

    pub(crate) fn with_platform_semantic_fact(mut self, fact: PlatformSemanticFact) -> Self {
        self.platform_semantic_fact = Some(fact);
        self
    }
}
