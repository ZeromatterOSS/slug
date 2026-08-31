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
use slug_build_api_v2::DepsetOrder;
use slug_build_api_v2::ProviderCollection;
use slug_build_api_v2::ProviderOccurrence;
use slug_build_api_v2::RunfilesPackageDepset;
use slug_build_api_v2::RunfilesPackageMetadata;
use slug_identity_v2::CanonicalLabel;
use slug_loading_v2::RuleCapability;

use crate::configured_target::ConfiguredEdge;
use crate::configured_target::ConfiguredEdgeKind;
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
    ToolchainDeclaration,
    ConfigSetting,
}

#[derive(Debug, Clone, Allocative)]
pub struct ConfiguredToolchainSelection {
    declaration: CanonicalLabel,
    implementation: ConfiguredTargetKey,
    actual_implementation: ConfiguredTargetKey,
    info: ProviderOccurrence,
}

impl ConfiguredToolchainSelection {
    pub fn new(
        declaration: CanonicalLabel,
        implementation: ConfiguredTargetKey,
        actual_implementation: ConfiguredTargetKey,
        info: ProviderOccurrence,
    ) -> Self {
        Self {
            declaration,
            implementation,
            actual_implementation,
            info,
        }
    }

    pub fn declaration(&self) -> &CanonicalLabel {
        &self.declaration
    }

    pub fn implementation(&self) -> &ConfiguredTargetKey {
        &self.implementation
    }

    pub fn actual_implementation(&self) -> &ConfiguredTargetKey {
        &self.actual_implementation
    }

    pub fn info(&self) -> &ProviderOccurrence {
        &self.info
    }
}

#[derive(Debug, Clone, Allocative)]
pub struct ConfiguredToolchainContextRow {
    requested: ConfiguredTargetKey,
    actual: ConfiguredTargetKey,
    mandatory: bool,
    selected: Option<ConfiguredToolchainSelection>,
}

impl ConfiguredToolchainContextRow {
    pub fn new(
        requested: ConfiguredTargetKey,
        actual: ConfiguredTargetKey,
        mandatory: bool,
        selected: Option<ConfiguredToolchainSelection>,
    ) -> Self {
        Self {
            requested,
            actual,
            mandatory,
            selected,
        }
    }

    pub fn requested(&self) -> &ConfiguredTargetKey {
        &self.requested
    }

    pub fn actual(&self) -> &ConfiguredTargetKey {
        &self.actual
    }

    pub fn mandatory(&self) -> bool {
        self.mandatory
    }

    pub fn selected(&self) -> Option<&ConfiguredToolchainSelection> {
        self.selected.as_ref()
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct ToolchainTopology {
    candidate_execution_platforms: Arc<[ConfiguredTargetKey]>,
    toolchain: Option<Arc<ConfiguredActionToolchainContext>>,
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
pub struct ConfiguredPlatform {
    requested: ConfiguredTargetKey,
    actual: ConfiguredTargetKey,
    fact: PlatformSemanticFact,
    constraints: Arc<[ConfiguredActionPlatformConstraint]>,
}

/// Provider-free selection result for one requested toolchain type.  Requested
/// aliases deliberately remain visible while selection groups use `actual`.
#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct ConfiguredToolchainResolutionRow {
    requested: ConfiguredTargetKey,
    actual: ConfiguredTargetKey,
    mandatory: bool,
    declaration: Option<CanonicalLabel>,
    implementation: Option<CanonicalLabel>,
}

impl ConfiguredToolchainResolutionRow {
    pub(crate) fn new(
        requested: ConfiguredTargetKey,
        actual: ConfiguredTargetKey,
        mandatory: bool,
        declaration: Option<CanonicalLabel>,
        implementation: Option<CanonicalLabel>,
    ) -> Self {
        assert_eq!(
            declaration.is_some(),
            implementation.is_some(),
            "toolchain declaration and implementation selection must agree"
        );
        Self {
            requested,
            actual,
            mandatory,
            declaration,
            implementation,
        }
    }

    pub fn requested(&self) -> &ConfiguredTargetKey {
        &self.requested
    }

    pub fn actual(&self) -> &ConfiguredTargetKey {
        &self.actual
    }

    pub fn mandatory(&self) -> bool {
        self.mandatory
    }

    pub fn declaration(&self) -> Option<&CanonicalLabel> {
        self.declaration.as_ref()
    }

    pub fn implementation(&self) -> Option<&CanonicalLabel> {
        self.implementation.as_ref()
    }
}

/// Immutable configured toolchain eligibility and selection facts.  This is
/// intentionally independent of implementation analysis and Starlark values.
#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct ConfiguredToolchainResolution {
    target_platform: Arc<ConfiguredPlatform>,
    execution_platform: Arc<ConfiguredPlatform>,
    rows: Arc<[ConfiguredToolchainResolutionRow]>,
}

impl ConfiguredToolchainResolution {
    pub(crate) fn new(
        target_platform: Arc<ConfiguredPlatform>,
        execution_platform: Arc<ConfiguredPlatform>,
        rows: Arc<[ConfiguredToolchainResolutionRow]>,
    ) -> Self {
        Self {
            target_platform,
            execution_platform,
            rows,
        }
    }

    pub fn target_platform(&self) -> &Arc<ConfiguredPlatform> {
        &self.target_platform
    }

    pub fn execution_platform(&self) -> &Arc<ConfiguredPlatform> {
        &self.execution_platform
    }

    pub fn rows(&self) -> &[ConfiguredToolchainResolutionRow] {
        &self.rows
    }
}

impl ConfiguredPlatform {
    pub(crate) fn new(
        requested: ConfiguredTargetKey,
        actual: ConfiguredTargetKey,
        fact: PlatformSemanticFact,
        constraints: Arc<[ConfiguredActionPlatformConstraint]>,
    ) -> Self {
        Self {
            requested,
            actual,
            fact,
            constraints,
        }
    }

    pub fn requested(&self) -> &ConfiguredTargetKey {
        &self.requested
    }

    pub fn actual(&self) -> &ConfiguredTargetKey {
        &self.actual
    }

    pub fn fact(&self) -> &PlatformSemanticFact {
        &self.fact
    }

    pub fn constraints(&self) -> &[ConfiguredActionPlatformConstraint] {
        &self.constraints
    }
}

#[derive(Debug, Clone, Allocative)]
pub struct ConfiguredActionToolchainContext {
    execution_platform: ConfiguredTargetKey,
    rows: Arc<[ConfiguredToolchainContextRow]>,
}

impl ConfiguredActionToolchainContext {
    pub fn new(
        execution_platform: ConfiguredTargetKey,
        rows: Vec<ConfiguredToolchainContextRow>,
    ) -> Result<Self, String> {
        ensure_action(
            execution_platform.configuration().kind() == ConfigurationKind::Exec
                && execution_platform
                    .configuration()
                    .slug_configuration()
                    .is_some(),
            "configured toolchain context requires structural exec platform",
        )?;
        ensure_action(
            !rows.is_empty(),
            "configured toolchain context requires rows",
        )?;
        let mut requested = BTreeSet::new();
        ensure_action(
            rows.iter()
                .all(|row| requested.insert(row.requested.clone())),
            "configured toolchain context has duplicate requested type",
        )?;
        ensure_action(
            rows.iter().all(|row| {
                is_analysis_configured(&row.requested)
                    && row.actual.configuration() == row.requested.configuration()
                    && is_analysis_configured(&row.actual)
                    && (!row.mandatory || row.selected.is_some())
                    && row.selected.as_ref().is_none_or(|selected| {
                        selected.implementation.configuration()
                            == execution_platform.configuration()
                            && selected.actual_implementation.configuration()
                                == execution_platform.configuration()
                            && selected.info.identity().is_builtin("ToolchainInfo")
                    })
            }),
            "configured toolchain context has invalid row identity",
        )?;
        Ok(Self {
            execution_platform,
            rows: rows.into(),
        })
    }

    pub fn execution_platform(&self) -> &ConfiguredTargetKey {
        &self.execution_platform
    }

    pub fn rows(&self) -> &[ConfiguredToolchainContextRow] {
        &self.rows
    }

    pub fn has_selected(&self) -> bool {
        self.rows.iter().any(|row| row.selected.is_some())
    }
}

impl PartialEq for ConfiguredActionToolchainContext {
    fn eq(&self, other: &Self) -> bool {
        self.execution_platform == other.execution_platform
            && self.rows.len() == other.rows.len()
            && self
                .rows
                .iter()
                .zip(other.rows.iter())
                .all(|(left, right)| {
                    left.requested == right.requested
                        && left.actual == right.actual
                        && left.mandatory == right.mandatory
                        && match (&left.selected, &right.selected) {
                            (None, None) => true,
                            (Some(left), Some(right)) => {
                                left.declaration == right.declaration
                                    && left.implementation == right.implementation
                                    && left.actual_implementation == right.actual_implementation
                            }
                            _ => false,
                        }
                })
            && ProviderOccurrence::publication_eq_pairs(
                self.rows
                    .iter()
                    .zip(other.rows.iter())
                    .filter_map(|(left, right)| {
                        Some((
                            left.selected.as_ref()?.info(),
                            right.selected.as_ref()?.info(),
                        ))
                    }),
            )
    }
}

impl Eq for ConfiguredActionToolchainContext {}

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
            is_analysis_configured(&owner),
            "configured action owner requires structural analysis configuration",
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
            is_analysis_configured(&owner),
            "configured action owner requires structural analysis configuration",
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
                toolchain.execution_platform() == &execution_platform,
                "configured action toolchain has mismatched platform",
            )?;
            ensure_action(
                toolchain.rows().iter().all(|row| {
                    row.requested().configuration() == owner.configuration()
                        && row.actual().configuration() == owner.configuration()
                }),
                "configured action toolchain types require owner analysis configuration",
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
        match (
            self.execution_platform.is_some(),
            self.toolchain
                .as_ref()
                .is_some_and(|toolchain| toolchain.has_selected()),
        ) {
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

fn is_analysis_configured(key: &ConfiguredTargetKey) -> bool {
    matches!(
        key.configuration().kind(),
        ConfigurationKind::Target | ConfigurationKind::Exec
    ) && key.configuration().slug_configuration().is_some()
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
        toolchain: Option<Arc<ConfiguredActionToolchainContext>>,
    ) -> Result<Self, String> {
        if candidate_execution_platforms
            .iter()
            .any(|candidate| candidate.configuration().kind() != ConfigurationKind::Exec)
        {
            return Err("candidate execution platforms require exec configuration".to_owned());
        }
        if let Some(toolchain) = &toolchain
            && !candidate_execution_platforms.contains(toolchain.execution_platform())
        {
            return Err("selected execution platform is not a candidate".to_owned());
        }
        Ok(Self {
            candidate_execution_platforms: candidate_execution_platforms.into(),
            toolchain,
        })
    }

    pub fn candidate_execution_platforms(&self) -> &[ConfiguredTargetKey] {
        &self.candidate_execution_platforms
    }

    pub fn toolchain(&self) -> Option<&Arc<ConfiguredActionToolchainContext>> {
        self.toolchain.as_ref()
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct ConfiguredNodeResult {
    key: ConfiguredNodeKey,
    actual_configured_target: Option<ConfiguredTargetKey>,
    kind: ConfiguredNodeKind,
    providers: ProviderCollection,
    actions: Arc<[ConfiguredAction]>,
    declared_outputs: Arc<[CompactString]>,
    edges: Arc<[ConfiguredEdge]>,
    diagnostics: Arc<[AnalysisDiagnostic]>,
    rule_capability: Option<RuleCapability>,
    toolchain_topology: Option<ToolchainTopology>,
    platform_semantic_fact: Option<PlatformSemanticFact>,
    runfiles_packages: RunfilesPackageDepset,
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub(crate) struct RunfilesPackageClosureRow {
    key: ConfiguredNodeKey,
    packages: RunfilesPackageDepset,
}

impl RunfilesPackageClosureRow {
    pub(crate) fn new(key: ConfiguredNodeKey, packages: RunfilesPackageDepset) -> Self {
        Self { key, packages }
    }

    pub(crate) fn from_result(result: &ConfiguredNodeResult) -> Self {
        Self::new(result.key.clone(), result.runfiles_packages.clone())
    }
}

#[derive(Debug, Default)]
pub(crate) struct RunfilesPackageCollector {
    direct: Vec<Arc<RunfilesPackageMetadata>>,
    configured: Vec<RunfilesPackageClosureRow>,
}

impl RunfilesPackageCollector {
    pub(crate) fn add_direct(&mut self, package: Arc<RunfilesPackageMetadata>) {
        self.direct.push(package);
    }

    pub(crate) fn add_configured(&mut self, row: RunfilesPackageClosureRow) {
        self.configured.push(row);
    }

    pub(crate) fn finish(
        mut self,
        edges: &[ConfiguredEdge],
    ) -> Result<RunfilesPackageDepset, String> {
        self.direct
            .sort_by(|left, right| left.package().cmp(right.package()));
        for pair in self.direct.windows(2) {
            if pair[0].package() == pair[1].package() && pair[0] != pair[1] {
                return Err(format!(
                    "runfiles package {} has inconsistent metadata",
                    pair[1].package()
                ));
            }
        }
        self.direct
            .dedup_by(|left, right| left.package() == right.package());

        self.configured
            .sort_by(|left, right| left.key.cmp(&right.key));
        for pair in self.configured.windows(2) {
            if pair[0].key == pair[1].key && pair[0].packages != pair[1].packages {
                return Err(format!(
                    "configured dependency {} has inconsistent package closures",
                    pair[1].key
                ));
            }
        }
        self.configured
            .dedup_by(|left, right| left.key == right.key);

        for edge in edges
            .iter()
            .filter(|edge| runfiles_package_contributing_edge(edge.kind()))
        {
            if self
                .configured
                .binary_search_by(|row| row.key.cmp(edge.target()))
                .is_err()
            {
                return Err(format!(
                    "package-contributing edge {} has no runfiles package closure",
                    edge.target()
                ));
            }
        }

        RunfilesPackageDepset::new(
            DepsetOrder::Default,
            self.direct,
            self.configured
                .into_iter()
                .map(|row| row.packages)
                .collect(),
        )
        .map_err(|error| format!("building runfiles package closure: {error}"))
    }
}

fn runfiles_package_contributing_edge(kind: &ConfiguredEdgeKind) -> bool {
    !matches!(
        kind,
        ConfiguredEdgeKind::ToolchainRequirement
            | ConfiguredEdgeKind::CandidateExecutionPlatform { .. }
            | ConfiguredEdgeKind::HostPlatform
    )
}

impl ConfiguredNodeResult {
    pub fn new_rule(
        key: ConfiguredTargetKey,
        providers: ProviderCollection,
        rule_capability: Option<RuleCapability>,
        runfiles_packages: RunfilesPackageDepset,
    ) -> Self {
        Self {
            actual_configured_target: Some(key.clone()),
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
            runfiles_packages,
        }
    }

    pub(crate) fn new_native(
        key: ConfiguredNodeKey,
        kind: ConfiguredNodeKind,
        providers: ProviderCollection,
        rule_capability: Option<RuleCapability>,
        runfiles_packages: RunfilesPackageDepset,
    ) -> Self {
        assert_ne!(
            kind,
            ConfiguredNodeKind::Rule,
            "native nodes cannot be rules"
        );
        let actual_configured_target = key.configured_target().cloned();
        Self {
            key,
            actual_configured_target,
            kind,
            providers,
            actions: Arc::from([]),
            declared_outputs: Arc::from([]),
            edges: Arc::from([]),
            diagnostics: Arc::from([]),
            rule_capability,
            toolchain_topology: None,
            platform_semantic_fact: None,
            runfiles_packages,
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

    pub fn actual_configured_target(&self) -> Option<&ConfiguredTargetKey> {
        self.actual_configured_target.as_ref()
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

    pub fn runfiles_packages(&self) -> &RunfilesPackageDepset {
        &self.runfiles_packages
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

    pub(crate) fn with_actual_configured_target(mut self, actual: ConfiguredTargetKey) -> Self {
        assert!(self.configured_target_key().is_some());
        self.actual_configured_target = Some(actual);
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
