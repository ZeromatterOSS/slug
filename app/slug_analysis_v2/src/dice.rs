/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use compact_str::CompactString;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::AnalysisConfiguredTargetKey;
use slug_build_api_v2::AnalysisDepset;
use slug_build_api_v2::AnalysisTargetIdentity;
use slug_build_api_v2::AnalysisValue;
use slug_build_api_v2::ConfiguredTargetValue;
use slug_build_api_v2::DefaultInfo;
use slug_build_api_v2::DepsetOrder;
use slug_build_api_v2::ProviderCollection;
use slug_build_api_v2::ProviderIdentity;
use slug_build_api_v2::ProviderOccurrence;
use slug_build_api_v2::ProviderValue;
use slug_events_v2::CaptureEvaluationEvents;
use slug_events_v2::EvaluationEvent;
use slug_events_v2::EventBatch;
use slug_events_v2::StarlarkSourceLocation;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::PackageIdentifier;
use slug_identity_v2::TargetName;
use slug_loading_v2::AttributeKind;
use slug_loading_v2::CoercedAttributeValue;
use slug_loading_v2::CommandRegistrationExpansionKey;
use slug_loading_v2::CommandRegistrationExpansionObservationKey;
use slug_loading_v2::HostPackageInventory;
use slug_loading_v2::HostPackageInventoryErrorRef;
use slug_loading_v2::HostPackageInventoryKey;
use slug_loading_v2::HostPackageInventoryObservationKey;
use slug_loading_v2::LoadedPackage;
use slug_loading_v2::LoadingPreparationNeeds;
use slug_loading_v2::LoadingPreparationOutcome;
use slug_loading_v2::ModuleRegistrationExpansion;
use slug_loading_v2::ModuleRegistrationExpansionKey;
use slug_loading_v2::ModuleRegistrationExpansionObservationError;
use slug_loading_v2::ModuleRegistrationExpansionObservationKey;
use slug_loading_v2::PackageTargetKind;
use slug_loading_v2::attrs::AttributeDependencyConfiguration;
use slug_loading_v2::attrs::TransitionDefinition;
use slug_loading_v2::package::BuildSettingDeclaration;
use slug_loading_v2::package::NativeToolchainTarget;
use slug_loading_v2::package::StarlarkRuleImplementation;
use slug_loading_v2::package::ToolchainTypeRequirement;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathOutcome;
use slug_workspace_v2::ResolvedPathKey;
use slug_workspace_v2::ResolvedPathObservationKey;
use slug_workspace_v2::ResolvedPathState;
use starlark::PrintHandler;
use starlark::PrintLocation;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::values::Value;
use starlark::values::dict::DictRef;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::build_setting::matches_expected_text;
use crate::build_setting::resolve_candidate;
use crate::build_setting::unpack_transition_value;
use crate::configured_analysis_cycle_detector::ConfiguredAnalysisCycleGuard;
use crate::configured_attribute::ConfiguredAttributeCondition;
use crate::configured_attribute::ResolvedRuleAttribute;
use crate::configured_attribute::resolve_configured_attribute;
use crate::key::ConfigurationKey;
use crate::key::ConfigurationKind;
use crate::key::ConfiguredNodeKey;
use crate::key::ConfiguredTargetKey;
#[cfg(test)]
use crate::key::StarlarkOption;
use crate::result::ConfiguredActionAspectProvenance;
use crate::result::ConfiguredActionExecGroup;
use crate::result::ConfiguredActionOwnerContext;
use crate::result::ConfiguredActionPlatformConstraint;
use crate::result::ConfiguredActionToolchainContext;
use crate::result::ConfiguredNodeKind;
use crate::result::ConfiguredNodeResult;
use crate::result::ConfiguredPlatform;
use crate::result::ConfiguredToolchainContextRow;
use crate::result::ConfiguredToolchainResolution;
use crate::result::ConfiguredToolchainResolutionRow;
use crate::result::ConfiguredToolchainSelection;
use crate::result::PlatformSemanticFact;
use crate::result::ToolchainTopology;
use crate::starlark_rule::LoadedRuleError;
use crate::starlark_rule::PreparedConfiguredAttribute;
use crate::starlark_rule::PreparedDependency;
use crate::starlark_rule::PreparedToolchain;
use crate::starlark_rule::evaluate_loaded_rule;
use crate::subrule::DeclaredDependencyKey;
use crate::subrule::configured_dependency_rows;
use crate::subrule::validate_configured_dependency;

fn without_starlark_option(
    configuration: &ConfigurationKey,
    label: &CanonicalLabel,
) -> ConfigurationKey {
    if configuration.starlark_option(label).is_none() {
        return configuration.clone();
    }
    if let Some(configuration) = configuration.slug_configuration() {
        return ConfigurationKey::from_slug(configuration.without_starlark_option(label));
    }
    let mut result = ConfigurationKey::new(
        configuration.kind(),
        configuration
            .checksum()
            .expect("legacy configuration carries a checksum")
            .clone(),
    );
    for option in configuration
        .starlark_options()
        .iter()
        .filter(|option| option.label() != label)
    {
        result = result.with_starlark_option(option.clone());
    }
    result
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub enum AnalysisErrorKind {
    TargetNotFound {
        label: CanonicalLabel,
        build_file: PathBuf,
    },
    ExecutableRuleMissingExecutable {
        rule_class: CompactString,
    },
    UnsupportedConfiguredAttribute {
        target: CanonicalLabel,
        attribute: CompactString,
        exec_configuration: bool,
        executable: bool,
    },
    Message(String),
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct AnalysisError {
    kind: AnalysisErrorKind,
}

impl AnalysisError {
    pub fn message(message: impl Into<String>) -> Self {
        Self {
            kind: AnalysisErrorKind::Message(message.into()),
        }
    }

    fn new(message: impl Into<String>) -> Self {
        Self::message(message)
    }

    fn target_not_found(label: CanonicalLabel, build_file: PathBuf) -> Self {
        Self {
            kind: AnalysisErrorKind::TargetNotFound { label, build_file },
        }
    }

    fn from_loaded_rule_error(error: LoadedRuleError) -> Self {
        let kind = match error {
            LoadedRuleError::Message(message) => AnalysisErrorKind::Message(message),
            LoadedRuleError::ExecutableRuleMissingExecutable { rule_class } => {
                AnalysisErrorKind::ExecutableRuleMissingExecutable { rule_class }
            }
        };
        Self { kind }
    }

    fn unsupported_configured_attribute(
        target: CanonicalLabel,
        attribute: impl Into<CompactString>,
        exec_configuration: bool,
        executable: bool,
    ) -> Self {
        Self {
            kind: AnalysisErrorKind::UnsupportedConfiguredAttribute {
                target,
                attribute: attribute.into(),
                exec_configuration,
                executable,
            },
        }
    }

    pub fn kind(&self) -> &AnalysisErrorKind {
        &self.kind
    }
}

impl fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            AnalysisErrorKind::TargetNotFound { label, build_file } => {
                write!(
                    f,
                    "target `{label}` was not found in {}",
                    build_file.display()
                )
            }
            AnalysisErrorKind::ExecutableRuleMissingExecutable { rule_class } => write!(
                f,
                "The rule '{rule_class}' is executable. It needs to create an executable File and pass it as the 'executable' parameter to the DefaultInfo it returns."
            ),
            AnalysisErrorKind::UnsupportedConfiguredAttribute {
                target,
                attribute,
                exec_configuration,
                executable,
            } => {
                let declaration = match (*exec_configuration, *executable) {
                    (true, true) => "cfg=\"exec\" and executable=True",
                    (true, false) => "cfg=\"exec\"",
                    (false, true) => "executable=True",
                    (false, false) => unreachable!("unsupported attribute retains a reason"),
                };
                write!(
                    f,
                    "configured analysis of target `{target}` does not yet support attribute `{attribute}` declared with {declaration}"
                )
            }
            AnalysisErrorKind::Message(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for AnalysisError {}

/// The single Need-aware production DICE identity for configured-target analysis.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct ConfiguredNodeAnalysisKey {
    workspace: NormalizedAbsolutePath,
    node: ConfiguredNodeKey,
}

#[doc(hidden)]
#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct ConfiguredNodeAnalysisObservationKey(ConfiguredNodeAnalysisKey);

/// The sole configured `config_setting` match owner.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct ConfiguredConditionKey {
    workspace: NormalizedAbsolutePath,
    target: ConfiguredTargetKey,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct ConfiguredPlatformKey(NormalizedAbsolutePath, ConfiguredTargetKey);

#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct ConfiguredTargetPlatformKey(NormalizedAbsolutePath, ConfigurationKey);

/// Provider-free, structural identity for configured toolchain selection.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct ConfiguredToolchainResolutionKey {
    workspace: NormalizedAbsolutePath,
    configuration: ConfigurationKey,
    requirements: Arc<[ToolchainTypeRequirement]>,
}

#[doc(hidden)]
#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct ConfiguredToolchainResolutionObservationKey(ConfiguredToolchainResolutionKey);

#[derive(Debug, Clone, Copy, Eq, PartialEq, Allocative)]
pub enum ConfiguredConditionMatch {
    Match,
    NoMatch,
}

pub type ConfiguredConditionOutcome = LoadingPreparationOutcome<
    Result<Arc<Result<ConfiguredConditionMatch, AnalysisError>>, ObservedPathFrontierError>,
>;

pub type ConfiguredPlatformOutcome = LoadingPreparationOutcome<
    Result<Arc<Result<Arc<ConfiguredPlatform>, AnalysisError>>, ObservedPathFrontierError>,
>;

pub type ConfiguredToolchainResolutionOutcome =
    LoadingPreparationOutcome<Arc<Result<Arc<ConfiguredToolchainResolution>, AnalysisError>>>;
#[doc(hidden)]
pub type ObservedConfiguredToolchainResolutionOutcome =
    AnalysisDriverOutcome<Arc<Result<Arc<ConfiguredToolchainResolution>, AnalysisError>>>;

impl ConfiguredConditionKey {
    pub fn new(
        workspace: NormalizedAbsolutePath,
        target: ConfiguredTargetKey,
    ) -> Result<Self, AnalysisError> {
        if target.configuration().slug_configuration().is_none() {
            return Err(AnalysisError::message(
                "configured-condition matching requires a structural Slug configuration",
            ));
        }
        Ok(Self { workspace, target })
    }

    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        &self.workspace
    }

    pub fn target(&self) -> &ConfiguredTargetKey {
        &self.target
    }
}

impl ConfiguredPlatformKey {
    pub fn new(
        workspace: NormalizedAbsolutePath,
        target: ConfiguredTargetKey,
    ) -> Result<Self, AnalysisError> {
        if target.configuration().slug_configuration().is_none() {
            return Err(AnalysisError::message(
                "configured-platform analysis requires a structural Slug configuration",
            ));
        }
        Ok(Self(workspace, target))
    }
}

impl ConfiguredTargetPlatformKey {
    pub fn new(
        workspace: NormalizedAbsolutePath,
        configuration: ConfigurationKey,
    ) -> Result<Self, AnalysisError> {
        if configuration.slug_configuration().is_none() {
            return Err(AnalysisError::message(
                "target-platform analysis requires a structural Slug configuration",
            ));
        }
        Ok(Self(workspace, configuration))
    }
}

impl ConfiguredToolchainResolutionKey {
    pub fn new(
        workspace: NormalizedAbsolutePath,
        configuration: ConfigurationKey,
        requirements: Arc<[ToolchainTypeRequirement]>,
    ) -> Result<Self, AnalysisError> {
        if !matches!(
            configuration.kind(),
            ConfigurationKind::Target | ConfigurationKind::Exec
        ) || configuration.slug_configuration().is_none()
        {
            return Err(AnalysisError::message(
                "configured toolchain resolution requires a structural analysis configuration",
            ));
        }
        Ok(Self {
            workspace,
            configuration,
            requirements,
        })
    }

    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        &self.workspace
    }

    pub fn configuration(&self) -> &ConfigurationKey {
        &self.configuration
    }

    pub fn requirements(&self) -> &[ToolchainTypeRequirement] {
        &self.requirements
    }
}

impl ConfiguredToolchainResolutionObservationKey {
    #[doc(hidden)]
    pub fn new(
        workspace: NormalizedAbsolutePath,
        configuration: ConfigurationKey,
        requirements: Arc<[ToolchainTypeRequirement]>,
    ) -> Result<Self, AnalysisError> {
        ConfiguredToolchainResolutionKey::new(workspace, configuration, requirements).map(Self)
    }

    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        self.0.workspace()
    }

    pub fn configuration(&self) -> &ConfigurationKey {
        self.0.configuration()
    }

    pub fn requirements(&self) -> &[ToolchainTypeRequirement] {
        self.0.requirements()
    }
}

impl ConfiguredNodeAnalysisObservationKey {
    #[doc(hidden)]
    pub fn new(
        workspace: NormalizedAbsolutePath,
        node: impl Into<ConfiguredNodeKey>,
    ) -> Result<Self, AnalysisError> {
        ConfiguredNodeAnalysisKey::new(workspace, node).map(Self)
    }

    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        self.0.workspace()
    }

    pub fn node(&self) -> &ConfiguredNodeKey {
        self.0.node()
    }

    pub fn configured_target(&self) -> Option<&ConfiguredTargetKey> {
        self.0.configured_target()
    }
}

#[derive(Clone, Copy)]
enum ConfiguredAnalysisMode {
    Legacy,
    Observed,
}

type AnalysisDriverOutcome<T> = LoadingPreparationOutcome<Result<T, ObservedPathFrontierError>>;
type AnalysisSemanticOutcome<T> = AnalysisDriverOutcome<Result<T, AnalysisError>>;

macro_rules! analysis_value {
    ($outcome:expr) => {
        match $outcome {
            LoadingPreparationOutcome::Need(need) => return LoadingPreparationOutcome::Need(need),
            LoadingPreparationOutcome::Complete(Err(error)) => {
                return LoadingPreparationOutcome::Complete(Err(error))
            }
            LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
                return analysis_semantic_complete(Err(error))
            }
            LoadingPreparationOutcome::Complete(Ok(Ok(value))) => value,
        }
    };
}

#[doc(hidden)]
pub type ObservedConfiguredNodeAnalysisPreparationOutcome =
    AnalysisSemanticOutcome<ConfiguredNodeAnalysisObservationKey>;

impl ConfiguredNodeAnalysisKey {
    pub fn new(
        workspace: NormalizedAbsolutePath,
        node: impl Into<ConfiguredNodeKey>,
    ) -> Result<Self, AnalysisError> {
        let node = node.into();
        if let Some(configured_target) = node.configured_target()
            && configured_target
                .configuration()
                .slug_configuration()
                .is_none()
        {
            return Err(AnalysisError::message(
                "production configured-node analysis requires a structural Slug configuration",
            ));
        }
        Ok(Self { workspace, node })
    }

    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        &self.workspace
    }

    pub fn node(&self) -> &ConfiguredNodeKey {
        &self.node
    }

    pub fn configured_target(&self) -> Option<&ConfiguredTargetKey> {
        self.node.configured_target()
    }
}

#[async_trait]
impl Key for ConfiguredConditionKey {
    type Value = ConfiguredConditionOutcome;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        match compute_configured_condition(ctx, self).await {
            LoadingPreparationOutcome::Need(need) => LoadingPreparationOutcome::Need(need),
            LoadingPreparationOutcome::Complete(Err(error)) => {
                LoadingPreparationOutcome::Complete(Err(error))
            }
            LoadingPreparationOutcome::Complete(Ok(result)) => {
                LoadingPreparationOutcome::Complete(Ok(Arc::new(result)))
            }
        }
    }

    fn equality(left: &Self::Value, right: &Self::Value) -> bool {
        matches!(
            (left, right),
            (
                LoadingPreparationOutcome::Complete(Ok(left)),
                LoadingPreparationOutcome::Complete(Ok(right)),
            ) if matches!((left.as_ref(), right.as_ref()), (Ok(left), Ok(right)) if left == right)
        )
    }

    fn validity(value: &Self::Value) -> bool {
        matches!(
            value,
            LoadingPreparationOutcome::Complete(Ok(result)) if result.as_ref().is_ok()
        )
    }
}

fn configured_platform_equality(
    left: &ConfiguredPlatformOutcome,
    right: &ConfiguredPlatformOutcome,
) -> bool {
    matches!(
        (left, right),
        (
            LoadingPreparationOutcome::Complete(Ok(left)),
            LoadingPreparationOutcome::Complete(Ok(right)),
        ) if matches!((left.as_ref(), right.as_ref()), (Ok(left), Ok(right)) if left == right)
    )
}

fn configured_platform_validity(value: &ConfiguredPlatformOutcome) -> bool {
    matches!(
        value,
        LoadingPreparationOutcome::Complete(Ok(result)) if result.as_ref().is_ok()
    )
}

#[async_trait]
impl Key for ConfiguredPlatformKey {
    type Value = ConfiguredPlatformOutcome;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        match compute_configured_platform(ctx, self).await {
            LoadingPreparationOutcome::Need(need) => LoadingPreparationOutcome::Need(need),
            LoadingPreparationOutcome::Complete(Err(error)) => {
                LoadingPreparationOutcome::Complete(Err(error))
            }
            LoadingPreparationOutcome::Complete(Ok(result)) => {
                LoadingPreparationOutcome::Complete(Ok(Arc::new(result)))
            }
        }
    }

    fn equality(left: &Self::Value, right: &Self::Value) -> bool {
        configured_platform_equality(left, right)
    }

    fn validity(value: &Self::Value) -> bool {
        configured_platform_validity(value)
    }
}

#[async_trait]
impl Key for ConfiguredTargetPlatformKey {
    type Value = ConfiguredPlatformOutcome;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let structural = self
            .1
            .slug_configuration()
            .expect("ConfiguredTargetPlatformKey validates structural configuration");
        let label = match structural.target_platform_label() {
            Ok(label) => label,
            Err(error) => {
                return LoadingPreparationOutcome::Complete(Ok(Arc::new(Err(
                    AnalysisError::message(error.to_string()),
                ))));
            }
        };
        let key = ConfiguredPlatformKey::new(
            self.0.dupe(),
            ConfiguredTargetKey::new(label, self.1.clone()),
        )
        .expect("target platform inherits structural configuration");
        match ctx.compute(&key).await {
            Ok(value) => value,
            Err(error) => LoadingPreparationOutcome::Complete(Ok(Arc::new(Err(
                AnalysisError::message(format!("computing target platform through DICE: {error}")),
            )))),
        }
    }

    fn equality(left: &Self::Value, right: &Self::Value) -> bool {
        configured_platform_equality(left, right)
    }

    fn validity(value: &Self::Value) -> bool {
        configured_platform_validity(value)
    }
}

fn analysis_semantic_complete<T>(result: Result<T, AnalysisError>) -> AnalysisSemanticOutcome<T> {
    LoadingPreparationOutcome::Complete(Ok(result))
}

fn package_inventory_error(error: HostPackageInventoryErrorRef<'_>) -> AnalysisError {
    let message = match error {
        HostPackageInventoryErrorRef::Root(error) => error.to_string(),
        HostPackageInventoryErrorRef::CanonicalRoute(error) => error.to_string(),
        HostPackageInventoryErrorRef::Canonical(error) => error.to_string(),
    };
    AnalysisError::message(message)
}

async fn compute_configured_package_input(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: NormalizedAbsolutePath,
    package: PackageIdentifier,
    context: &str,
) -> AnalysisSemanticOutcome<ConfiguredPackageValue> {
    match mode {
        ConfiguredAnalysisMode::Legacy => {
            match ctx
                .compute(&HostPackageInventoryKey::new(workspace, package))
                .await
            {
                Ok(LoadingPreparationOutcome::Need(need)) => LoadingPreparationOutcome::Need(need),
                Ok(LoadingPreparationOutcome::Complete(value)) => {
                    analysis_semantic_complete(Ok(value))
                }
                Err(error) => analysis_semantic_complete(Err(AnalysisError::message(format!(
                    "{context}: {error}"
                )))),
            }
        }
        ConfiguredAnalysisMode::Observed => {
            match ctx
                .compute(&HostPackageInventoryObservationKey::new(workspace, package))
                .await
            {
                Ok(LoadingPreparationOutcome::Need(need)) => LoadingPreparationOutcome::Need(need),
                Ok(LoadingPreparationOutcome::Complete(Err(
                    slug_loading_v2::HostPackageInventoryObservationError::Frontier(error),
                ))) => LoadingPreparationOutcome::Complete(Err(error)),
                Ok(LoadingPreparationOutcome::Complete(Err(
                    slug_loading_v2::HostPackageInventoryObservationError::CanonicalRoute(error),
                ))) => {
                    analysis_semantic_complete(Err(AnalysisError::message(format!("{error:?}"))))
                }
                Ok(LoadingPreparationOutcome::Complete(Ok(observed))) => {
                    analysis_semantic_complete(Ok(observed.result().dupe()))
                }
                Err(error) => analysis_semantic_complete(Err(AnalysisError::message(format!(
                    "{context}: {error}"
                )))),
            }
        }
    }
}

/// Resolve the root setting and structural configuration before admitting the
/// sole configured-analysis DICE key.
pub async fn prepare_configured_node_analysis(
    ctx: &mut DiceComputations<'_>,
    workspace: NormalizedAbsolutePath,
    requested: CanonicalLabel,
    configuration: ConfigurationKey,
) -> LoadingPreparationOutcome<Result<ConfiguredNodeAnalysisKey, AnalysisError>> {
    match prepare_configured_node_analysis_driver(
        ctx,
        ConfiguredAnalysisMode::Legacy,
        workspace,
        requested,
        configuration,
    )
    .await
    {
        LoadingPreparationOutcome::Need(need) => LoadingPreparationOutcome::Need(need),
        LoadingPreparationOutcome::Complete(Ok(result)) => {
            LoadingPreparationOutcome::Complete(result)
        }
        LoadingPreparationOutcome::Complete(Err(error)) => {
            panic!("legacy configured-analysis preparation produced frontier error: {error}")
        }
    }
}

#[doc(hidden)]
pub async fn prepare_configured_node_analysis_observed(
    ctx: &mut DiceComputations<'_>,
    workspace: NormalizedAbsolutePath,
    requested: CanonicalLabel,
    configuration: ConfigurationKey,
) -> ObservedConfiguredNodeAnalysisPreparationOutcome {
    prepare_configured_node_analysis_driver(
        ctx,
        ConfiguredAnalysisMode::Observed,
        workspace,
        requested,
        configuration,
    )
    .await
    .map(|result| result.map(|result| result.map(ConfiguredNodeAnalysisObservationKey)))
}

async fn prepare_configured_node_analysis_driver(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: NormalizedAbsolutePath,
    requested: CanonicalLabel,
    configuration: ConfigurationKey,
) -> AnalysisSemanticOutcome<ConfiguredNodeAnalysisKey> {
    prepare_configured_node_analysis_driver_with_source_admission(
        ctx,
        mode,
        workspace,
        requested,
        configuration,
        false,
    )
    .await
}

async fn prepare_configured_node_analysis_driver_with_source_admission(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: NormalizedAbsolutePath,
    requested: CanonicalLabel,
    configuration: ConfigurationKey,
    source_admitted: bool,
) -> AnalysisSemanticOutcome<ConfiguredNodeAnalysisKey> {
    if configuration.slug_configuration().is_none() {
        return analysis_semantic_complete(Err(AnalysisError::message(
            "production configured-node analysis requires a structural Slug configuration",
        )));
    }
    let package_outcome = compute_configured_package_input(
        ctx,
        mode,
        workspace.dupe(),
        requested.package().clone(),
        "loading configured target package through DICE",
    )
    .await;
    let mut all_need: Option<LoadingPreparationNeeds> = None;
    let mut first_outer = None;
    let mut first_error = None;
    let mut package_inventory = None;
    match package_outcome {
        LoadingPreparationOutcome::Need(need) => {
            all_need = Some(match all_need {
                Some(current) => current.try_union(&need).unwrap_or_else(|error| {
                    panic!(
                        "configured-node preparation Needs must be structurally compatible: \
                         {error:?}"
                    )
                }),
                None => need,
            });
        }
        LoadingPreparationOutcome::Complete(Err(error)) => {
            if first_outer.is_none() {
                first_outer = Some(error);
            }
        }
        LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
            if first_error.is_none() {
                first_error = Some(error);
            }
        }
        LoadingPreparationOutcome::Complete(Ok(Ok(value))) => package_inventory = Some(value),
    }
    if let Some(error) = first_outer {
        return LoadingPreparationOutcome::Complete(Err(error));
    }
    if let Some(need) = all_need {
        return LoadingPreparationOutcome::Need(need);
    }
    if let Some(error) = first_error {
        return analysis_semantic_complete(Err(error));
    }
    let package_inventory =
        package_inventory.expect("complete target package preparation stores its value");
    let package = match package_inventory.loaded() {
        Ok(package) => package,
        Err(error) => return analysis_semantic_complete(Err(package_inventory_error(error))),
    };
    let target = package
        .targets
        .iter()
        .find(|target| target.name == requested.target().as_str());
    let node = if (target.is_none() && source_admitted)
        || target.is_some_and(|target| matches!(target.kind, PackageTargetKind::ExportedFile))
    {
        ConfiguredNodeKey::null(requested)
    } else if target.is_none() {
        return analysis_semantic_complete(Err(AnalysisError::target_not_found(
            requested,
            package.build_file.clone(),
        )));
    } else {
        ConfiguredNodeKey::configured(ConfiguredTargetKey::new(requested, configuration))
    };
    analysis_semantic_complete(ConfiguredNodeAnalysisKey::new(workspace, node))
}

async fn build_setting_declaration(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: &NormalizedAbsolutePath,
    setting: &CanonicalLabel,
) -> AnalysisSemanticOutcome<BuildSettingDeclaration> {
    let package_inventory = match compute_configured_package_input(
        ctx,
        mode,
        workspace.dupe(),
        setting.package().clone(),
        "loading Starlark build-setting declaration through DICE",
    )
    .await
    {
        LoadingPreparationOutcome::Need(need) => return LoadingPreparationOutcome::Need(need),
        LoadingPreparationOutcome::Complete(Err(error)) => {
            return LoadingPreparationOutcome::Complete(Err(error));
        }
        LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
            return analysis_semantic_complete(Err(error));
        }
        LoadingPreparationOutcome::Complete(Ok(Ok(value))) => value,
    };
    let package = match package_inventory.loaded() {
        Ok(package) => package,
        Err(error) => return analysis_semantic_complete(Err(package_inventory_error(error))),
    };
    let Some(target) = package
        .targets
        .iter()
        .find(|target| target.name == setting.target().as_str())
    else {
        return analysis_semantic_complete(Err(AnalysisError::message(format!(
            "build setting {setting} is missing"
        ))));
    };
    let PackageTargetKind::StarlarkRule(rule) = &target.kind else {
        return analysis_semantic_complete(Err(AnalysisError::message(format!(
            "target {setting} is not a Starlark build setting"
        ))));
    };
    match rule.build_setting_declaration() {
        Ok(Some(declaration)) => analysis_semantic_complete(Ok(declaration)),
        Ok(None) => analysis_semantic_complete(Err(AnalysisError::message(format!(
            "target {setting} is not a Starlark build setting"
        )))),
        Err(error) => analysis_semantic_complete(Err(AnalysisError::message(format!(
            "invalid build-setting declaration for {setting}: {error}"
        )))),
    }
}

async fn compute_configured_condition(
    ctx: &mut DiceComputations<'_>,
    key: &ConfiguredConditionKey,
) -> AnalysisSemanticOutcome<ConfiguredConditionMatch> {
    let package_inventory = match compute_configured_package_input(
        ctx,
        ConfiguredAnalysisMode::Observed,
        key.workspace.dupe(),
        key.target.label().package().clone(),
        "loading config_setting target package through DICE",
    )
    .await
    {
        LoadingPreparationOutcome::Need(need) => return LoadingPreparationOutcome::Need(need),
        LoadingPreparationOutcome::Complete(Err(error)) => {
            return LoadingPreparationOutcome::Complete(Err(error));
        }
        LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
            return analysis_semantic_complete(Err(error));
        }
        LoadingPreparationOutcome::Complete(Ok(Ok(value))) => value,
    };
    let package = match package_inventory.loaded() {
        Ok(package) => package,
        Err(error) => return analysis_semantic_complete(Err(package_inventory_error(error))),
    };
    let Some(target) = package
        .targets
        .iter()
        .find(|target| target.name == key.target.label().target().as_str())
    else {
        return analysis_semantic_complete(Err(AnalysisError::target_not_found(
            key.target.label().clone(),
            package.build_file.clone(),
        )));
    };
    let PackageTargetKind::ConfigSetting { declaration } = &target.kind else {
        return analysis_semantic_complete(Err(AnalysisError::message(format!(
            "target {} is not a config_setting",
            key.target.label()
        ))));
    };
    let declaration = declaration.clone();
    if declaration.values().value().is_empty()
        && declaration.define_values().value().is_empty()
        && declaration.flag_values().value().is_empty()
        && declaration.constraint_values().value().is_empty()
    {
        return analysis_semantic_complete(Err(AnalysisError::message(format!(
            "config_setting {} must specify at least one non-empty predicate",
            key.target.label()
        ))));
    }
    let configuration = key
        .target
        .configuration()
        .slug_configuration()
        .expect("ConfiguredConditionKey validates structural configuration");
    let (native_match, mut first_error) = match configuration.matches_config_setting(
        declaration.values().value(),
        declaration.define_values().value(),
    ) {
        Ok(matches) => (matches, None),
        Err(error) => (
            false,
            Some(AnalysisError::message(format!(
                "matching native predicates for {}: {error}",
                key.target.label()
            ))),
        ),
    };

    let mut flag_match = true;
    let mut all_need: Option<LoadingPreparationNeeds> = None;
    let mut first_outer = None;
    for (flag, expected) in declaration.flag_values().value().iter() {
        match build_setting_declaration(ctx, ConfiguredAnalysisMode::Observed, &key.workspace, flag)
            .await
        {
            LoadingPreparationOutcome::Need(need) => {
                all_need = Some(match all_need {
                    Some(current) => current.try_union(&need).unwrap_or_else(|error| {
                        panic!("configured-condition Needs must agree: {error:?}")
                    }),
                    None => need,
                });
            }
            LoadingPreparationOutcome::Complete(Err(error)) => {
                if first_outer.is_none() {
                    first_outer = Some(error);
                }
            }
            LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
            LoadingPreparationOutcome::Complete(Ok(Ok(flag_declaration))) => {
                match matches_expected_text(
                    flag,
                    &flag_declaration,
                    key.target.configuration().starlark_option(flag),
                    expected,
                ) {
                    Ok(matches) => flag_match &= matches,
                    Err(error) if first_error.is_none() => {
                        first_error = Some(AnalysisError::message(error));
                    }
                    Err(_) => {}
                }
            }
        }
    }
    let mut constraint_match = true;
    if !declaration.constraint_values().value().is_empty() {
        let platform_key = ConfiguredTargetPlatformKey::new(
            key.workspace.dupe(),
            key.target.configuration().clone(),
        )
        .expect("configured condition has structural configuration");
        let platform = match ctx.compute(&platform_key).await {
            Ok(LoadingPreparationOutcome::Need(need)) => {
                all_need = Some(all_need.map_or(need.clone(), |current| {
                    current
                        .try_union(&need)
                        .expect("configured-condition Needs agree")
                }));
                None
            }
            Ok(LoadingPreparationOutcome::Complete(Err(error))) => {
                if first_outer.is_none() {
                    first_outer = Some(error);
                }
                None
            }
            Ok(LoadingPreparationOutcome::Complete(Ok(result))) => match result.as_ref() {
                Ok(platform) => Some(platform.dupe()),
                Err(error) => {
                    if first_error.is_none() {
                        first_error = Some(error.clone());
                    }
                    None
                }
            },
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(AnalysisError::message(format!(
                        "computing target platform for {}: {error}",
                        key.target.label()
                    )));
                }
                None
            }
        };
        for requested in declaration.constraint_values().value().iter() {
            let requested =
                ConfiguredTargetKey::new(requested.clone(), key.target.configuration().clone());
            match compute_configured_constraint(ctx, &key.workspace, &requested).await {
                LoadingPreparationOutcome::Need(need) => {
                    all_need = Some(all_need.map_or(need.clone(), |current| {
                        current
                            .try_union(&need)
                            .expect("configured-condition Needs agree")
                    }));
                }
                LoadingPreparationOutcome::Complete(Err(error)) => {
                    if first_outer.is_none() {
                        first_outer = Some(error);
                    }
                }
                LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
                LoadingPreparationOutcome::Complete(Ok(Ok(constraint))) => {
                    constraint_match &= platform.as_ref().is_some_and(|platform| {
                        platform.constraints().iter().any(|candidate| {
                            candidate.constraint_setting().label()
                                == constraint.constraint_setting().label()
                                && candidate.constraint_value().label()
                                    == constraint.constraint_value().label()
                        })
                    });
                }
            }
        }
    }
    if let Some(error) = first_outer {
        return LoadingPreparationOutcome::Complete(Err(error));
    }
    if let Some(need) = all_need {
        return LoadingPreparationOutcome::Need(need);
    }
    if let Some(error) = first_error {
        return analysis_semantic_complete(Err(error));
    }
    analysis_semantic_complete(Ok(if native_match && flag_match && constraint_match {
        ConfiguredConditionMatch::Match
    } else {
        ConfiguredConditionMatch::NoMatch
    }))
}

impl fmt::Display for ConfiguredNodeAnalysisKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "configured-node-analysis:{}", self.node)
    }
}

impl fmt::Display for ConfiguredNodeAnalysisObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

impl fmt::Display for ConfiguredConditionKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "configured-condition:{}", self.target)
    }
}

impl fmt::Display for ConfiguredPlatformKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "configured-platform:{}", self.1)
    }
}

impl fmt::Display for ConfiguredTargetPlatformKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "configured-target-platform:{}", self.1)
    }
}

type RootAnalysisKeyValue =
    LoadingPreparationOutcome<Arc<Result<Arc<ConfiguredNodeResult>, AnalysisError>>>;
type RootAnalysisDriverValue =
    AnalysisDriverOutcome<Arc<Result<Arc<ConfiguredNodeResult>, AnalysisError>>>;

macro_rules! root_value {
    ($outcome:expr) => {
        match $outcome {
            LoadingPreparationOutcome::Need(need) => return LoadingPreparationOutcome::Need(need),
            LoadingPreparationOutcome::Complete(Err(error)) => {
                return LoadingPreparationOutcome::Complete(Err(error))
            }
            LoadingPreparationOutcome::Complete(Ok(value)) => match value.as_ref() {
                Ok(result) => result.dupe(),
                Err(error) => return root_analysis_driver_complete(Err(error.clone())),
            },
        }
    };
}

#[derive(Default)]
struct AnalysisPrintCapture {
    events: RefCell<Vec<EvaluationEvent>>,
}

impl AnalysisPrintCapture {
    fn into_batch(self) -> EventBatch {
        EventBatch::from_events(self.events.into_inner())
    }
}

impl PrintHandler for AnalysisPrintCapture {
    fn println(&self, location: PrintLocation, text: &str) -> starlark::Result<()> {
        let (file, line, column) = location.into_parts();
        self.events
            .borrow_mut()
            .push(EvaluationEvent::StarlarkPrint {
                location: StarlarkSourceLocation::new(file, line, column),
                text: text.into(),
            });
        Ok(())
    }
}

fn starlark_rule_implementation<'a>(
    package: &'a LoadedPackage,
    configured_target: &ConfiguredTargetKey,
) -> Result<&'a StarlarkRuleImplementation, AnalysisError> {
    let label = configured_target.label();
    let target = package
        .targets
        .iter()
        .find(|target| target.name == label.target().as_str())
        .ok_or_else(|| {
            AnalysisError::target_not_found(label.clone(), package.build_file.clone())
        })?;
    let PackageTargetKind::StarlarkRule(implementation) = &target.kind else {
        return Err(AnalysisError::new(format!(
            "target `{label}` is not a Starlark rule"
        )));
    };
    Ok(implementation)
}

async fn prepare_configured_attribute_conditions(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: &NormalizedAbsolutePath,
    configuration: &ConfigurationKey,
    selector_labels: Vec<CanonicalLabel>,
) -> AnalysisSemanticOutcome<Vec<ConfiguredAttributeCondition>> {
    if selector_labels.is_empty() {
        return analysis_semantic_complete(Ok(Vec::new()));
    }

    let condition_configuration = configuration.clone();
    let condition_workspace = workspace.dupe();
    let condition_outcomes = ctx
        .compute_join(selector_labels.iter().cloned(), |ctx, label| {
            let key = ConfiguredConditionKey::new(
                condition_workspace.dupe(),
                ConfiguredTargetKey::new(label.clone(), condition_configuration.clone()),
            )
            .expect("configured rule carries a structural configuration");
            Box::pin(async move { (label, ctx.compute(&key).await) })
        })
        .await;
    let mut all_need: Option<LoadingPreparationNeeds> = None;
    let mut first_outer = None;
    let mut first_error = None;
    let mut truth = SmallMap::with_capacity(condition_outcomes.len());
    for (label, outcome) in condition_outcomes {
        match outcome {
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(AnalysisError::message(format!(
                        "computing configured selector condition {label}: {error}"
                    )));
                }
            }
            Ok(LoadingPreparationOutcome::Need(need)) => {
                all_need = Some(match all_need {
                    Some(current) => current.try_union(&need).unwrap_or_else(|error| {
                        panic!(
                            "configured selector Needs must be structurally compatible: {error:?}"
                        )
                    }),
                    None => need,
                });
            }
            Ok(LoadingPreparationOutcome::Complete(Err(error))) => {
                if first_outer.is_none() {
                    first_outer = Some(error);
                }
            }
            Ok(LoadingPreparationOutcome::Complete(Ok(result))) => match result.as_ref() {
                Ok(result) => {
                    truth.insert(label, *result == ConfiguredConditionMatch::Match);
                }
                Err(error) => {
                    if first_error.is_none() {
                        first_error = Some(error.clone());
                    }
                }
            },
        }
    }
    if let Some(error) = first_outer {
        return LoadingPreparationOutcome::Complete(Err(error));
    }
    if let Some(need) = all_need {
        return LoadingPreparationOutcome::Need(need);
    }
    if let Some(error) = first_error {
        return analysis_semantic_complete(Err(error));
    }

    load_configured_condition_declarations(ctx, mode, workspace, selector_labels, &truth).await
}

async fn load_configured_condition_declarations(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: &NormalizedAbsolutePath,
    labels: Vec<CanonicalLabel>,
    truth: &SmallMap<CanonicalLabel, bool>,
) -> AnalysisSemanticOutcome<Vec<ConfiguredAttributeCondition>> {
    let packages = labels
        .iter()
        .map(|label| label.package().clone())
        .collect::<SmallSet<_>>();
    let outcomes = ctx
        .compute_join(packages, |ctx, package| {
            Box::pin(async move {
                let outcome = compute_configured_package_input(
                    ctx,
                    mode,
                    workspace.dupe(),
                    package.clone(),
                    "loading configured selector declaration through DICE",
                )
                .await;
                (package, outcome)
            })
        })
        .await;
    let mut all_need: Option<LoadingPreparationNeeds> = None;
    let mut first_outer = None;
    let mut first_error = None;
    let mut loaded_packages = Vec::with_capacity(outcomes.len());
    for (package, outcome) in outcomes {
        match outcome {
            LoadingPreparationOutcome::Need(need) => {
                all_need = Some(match all_need {
                    Some(current) => current.try_union(&need).unwrap_or_else(|error| {
                        panic!(
                            "selector declaration Needs must be structurally compatible: {error:?}"
                        )
                    }),
                    None => need,
                });
            }
            LoadingPreparationOutcome::Complete(Err(error)) => {
                if first_outer.is_none() {
                    first_outer = Some(error);
                }
            }
            LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
            LoadingPreparationOutcome::Complete(Ok(Ok(value))) => {
                loaded_packages.push((package, value))
            }
        }
    }
    if let Some(error) = first_outer {
        return LoadingPreparationOutcome::Complete(Err(error));
    }
    if let Some(need) = all_need {
        return LoadingPreparationOutcome::Need(need);
    }
    if let Some(error) = first_error {
        return analysis_semantic_complete(Err(error));
    }

    let mut conditions = Vec::with_capacity(labels.len());
    for label in labels {
        let Some((_, inventory)) = loaded_packages
            .iter()
            .find(|(package, _)| package == label.package())
        else {
            unreachable!("every selector package completed")
        };
        let loaded = match inventory.loaded() {
            Ok(loaded) => loaded,
            Err(error) => return analysis_semantic_complete(Err(package_inventory_error(error))),
        };
        let Some(target) = loaded
            .targets
            .iter()
            .find(|target| target.name == label.target().as_str())
        else {
            return analysis_semantic_complete(Err(AnalysisError::target_not_found(
                label,
                loaded.build_file.clone(),
            )));
        };
        let PackageTargetKind::ConfigSetting { declaration } = &target.kind else {
            return analysis_semantic_complete(Err(AnalysisError::message(format!(
                "target {label} is not a config_setting"
            ))));
        };
        conditions.push(ConfiguredAttributeCondition {
            matches: *truth
                .get(&label)
                .expect("every completed condition stores truth"),
            label,
            declaration: declaration.clone(),
        });
    }

    analysis_semantic_complete(Ok(conditions))
}

async fn prepare_configured_rule_attributes(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: &NormalizedAbsolutePath,
    package: &LoadedPackage,
    configured_target: &ConfiguredTargetKey,
) -> AnalysisSemanticOutcome<Vec<ResolvedRuleAttribute>> {
    let implementation = match starlark_rule_implementation(package, configured_target) {
        Ok(implementation) => implementation,
        Err(error) => return analysis_semantic_complete(Err(error)),
    };
    let mut selector_labels = Vec::new();
    if let Some(config_dependencies) = implementation
        .values()
        .iter()
        .find(|value| value.declaration_name == "$config_dependencies")
    {
        config_dependencies.value.labels(&mut selector_labels);
    }
    let selector_labels = selector_labels
        .into_iter()
        .collect::<SmallSet<_>>()
        .into_iter()
        .collect();
    let conditions = match prepare_configured_attribute_conditions(
        ctx,
        mode,
        workspace,
        configured_target.configuration(),
        selector_labels,
    )
    .await
    {
        LoadingPreparationOutcome::Need(need) => return LoadingPreparationOutcome::Need(need),
        LoadingPreparationOutcome::Complete(Err(error)) => {
            return LoadingPreparationOutcome::Complete(Err(error));
        }
        LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
            return analysis_semantic_complete(Err(error));
        }
        LoadingPreparationOutcome::Complete(Ok(Ok(conditions))) => conditions,
    };

    let resolved = implementation
        .values()
        .iter()
        .zip(implementation.schema())
        .map(|(attribute, schema)| {
            resolve_configured_attribute(attribute.value.as_ref(), &conditions)
                .map(|value| ResolvedRuleAttribute {
                    declaration_name: attribute.declaration_name.clone(),
                    kind: schema.kind(),
                    sequence: schema.transition().is_some()
                        || matches!(schema.kind(), AttributeKind::LabelList),
                    value,
                })
                .map_err(|error| {
                    AnalysisError::message(format!(
                        "resolving configured attribute `{}`: {error}",
                        attribute.declaration_name
                    ))
                })
        })
        .collect::<Result<Vec<_>, _>>();
    analysis_semantic_complete(resolved)
}

async fn root_declared_dependency_keys(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: &NormalizedAbsolutePath,
    package: &LoadedPackage,
    configured_target: &ConfiguredTargetKey,
    resolved_attributes: &[ResolvedRuleAttribute],
) -> AnalysisSemanticOutcome<Vec<DeclaredDependencyKey>> {
    let implementation = match starlark_rule_implementation(package, configured_target) {
        Ok(implementation) => implementation,
        Err(error) => return analysis_semantic_complete(Err(error)),
    };
    let mut dependencies = Vec::new();
    for value in resolved_attributes {
        let Some(schema) = implementation
            .schema()
            .iter()
            .find(|schema| schema.declaration_name() == value.declaration_name)
        else {
            continue;
        };
        if !schema.ordinary_dependency() {
            continue;
        }
        if schema.declaration_name() == "$config_dependencies" {
            continue;
        }
        // Retain the Bazel tools allowlist in loading/query topology, but the
        // current Rust-native analysis subset has no permission-check action
        // and cannot configure external repositories yet.
        if schema.declaration_name() == "$allowlist_function_transition" {
            continue;
        }
        let mut labels = Vec::new();
        value.value.labels(&mut labels);
        let (configuration, transition_output) = match configured_dependency_configuration(
            ctx,
            mode,
            workspace,
            configured_target.configuration(),
            schema.transition(),
        )
        .await
        {
            LoadingPreparationOutcome::Need(need) => {
                return LoadingPreparationOutcome::Need(need);
            }
            LoadingPreparationOutcome::Complete(Err(error)) => {
                return LoadingPreparationOutcome::Complete(Err(error));
            }
            LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
                return analysis_semantic_complete(Err(error));
            }
            LoadingPreparationOutcome::Complete(Ok(Ok(configuration))) => configuration,
        };
        for (attribute_index, label) in labels.into_iter().enumerate() {
            let node = if label.package() == configured_target.label().package()
                && package
                    .targets
                    .iter()
                    .find(|target| target.name == label.target().as_str())
                    .is_none()
            {
                ConfiguredNodeKey::null(label)
            } else {
                ConfiguredNodeKey::configured(ConfiguredTargetKey::new(
                    label,
                    configuration.clone(),
                ))
            };
            dependencies.push(DeclaredDependencyKey {
                attribute: CompactString::from(value.declaration_name.as_str()),
                attribute_index: u32::try_from(attribute_index)
                    .expect("attribute dependency index fits u32"),
                node,
                transition_output: transition_output.clone(),
                hidden: false,
                exec_configuration: false,
                source_admitted: schema.allow_files()
                    || matches!(
                        schema.allow_single_file(),
                        Some(
                            slug_loading_v2::AllowSingleFile::True
                                | slug_loading_v2::AllowSingleFile::Extensions(_)
                        )
                    ),
                validation: None,
                configured_row: None,
            });
        }
    }
    analysis_semantic_complete(Ok(dependencies))
}

async fn configured_dependency_configuration(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: &NormalizedAbsolutePath,
    configuration: &ConfigurationKey,
    transition: Option<&TransitionDefinition>,
) -> AnalysisSemanticOutcome<(ConfigurationKey, Option<CanonicalLabel>)> {
    let Some(transition) = transition else {
        return analysis_semantic_complete(Ok((configuration.clone(), None)));
    };
    let output_label = match CanonicalLabel::parse(&format!("@@{}", transition.output())) {
        Ok(label) => label,
        Err(error) => return analysis_semantic_complete(Err(AnalysisError::new(error))),
    };
    let declaration = match build_setting_declaration(ctx, mode, workspace, &output_label).await {
        LoadingPreparationOutcome::Need(need) => return LoadingPreparationOutcome::Need(need),
        LoadingPreparationOutcome::Complete(Err(error)) => {
            return LoadingPreparationOutcome::Complete(Err(error));
        }
        LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
            return analysis_semantic_complete(Err(error));
        }
        LoadingPreparationOutcome::Complete(Ok(Ok(declaration))) => declaration,
    };
    let resolved = (|| -> Result<_, AnalysisError> {
        let module = Module::new();
        let returned = Evaluator::new(&module)
            .eval_function(
                transition.implementation().to_value(),
                &[Value::new_none(), Value::new_none()],
                &[],
            )
            .map_err(|error| AnalysisError::new(error.to_string()))?;
        let entries = DictRef::from_value(returned)
            .ok_or_else(|| AnalysisError::new("transition must return a dictionary"))?
            .iter()
            .collect::<Vec<_>>();
        let [(output, setting)] = entries.as_slice() else {
            return Err(AnalysisError::new(
                "transition must return exactly one declared output",
            ));
        };
        if output.unpack_str() != Some(transition.output()) {
            return Err(AnalysisError::new(format!(
                "transition output must be exactly {}",
                transition.output()
            )));
        }
        let candidate =
            unpack_transition_value(&output_label, &declaration, *setting, module.heap())
                .map_err(AnalysisError::new)?;
        let resolved = resolve_candidate(output_label.clone(), &declaration, candidate)
            .map_err(AnalysisError::new)?;
        Ok(match resolved {
            Some(resolved) => configuration.with_starlark_option(resolved),
            None => without_starlark_option(configuration, &output_label),
        })
    })();
    analysis_semantic_complete(resolved.map(|configuration| (configuration, Some(output_label))))
}

async fn prepare_declared_dependency_keys(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: &NormalizedAbsolutePath,
    dependencies: &mut [DeclaredDependencyKey],
) -> AnalysisSemanticOutcome<SmallMap<ConfiguredNodeKey, ConfiguredNodeAnalysisKey>> {
    let mut unique = SmallMap::with_capacity(dependencies.len());
    for dependency in dependencies.iter() {
        if let Some(source_admitted) = unique.get_mut(&dependency.node) {
            *source_admitted |= dependency.source_admitted;
        } else {
            unique.insert(dependency.node.clone(), dependency.source_admitted);
        }
    }
    let preparations = ctx
        .compute_join(unique, |ctx, (node, source_admitted)| {
            Box::pin(async move {
                let prepared = match &node {
                    ConfiguredNodeKey::Configured(configured_target) => {
                        prepare_configured_node_analysis_driver_with_source_admission(
                            ctx,
                            mode,
                            workspace.dupe(),
                            configured_target.label().clone(),
                            configured_target.configuration().clone(),
                            source_admitted,
                        )
                        .await
                    }
                    ConfiguredNodeKey::Null(_) => analysis_semantic_complete(
                        ConfiguredNodeAnalysisKey::new(workspace.dupe(), node.clone()),
                    ),
                };
                (node, prepared)
            })
        })
        .await;
    let mut all_need: Option<LoadingPreparationNeeds> = None;
    let mut first_outer = None;
    let mut first_error = None;
    let mut normalizations = SmallMap::with_capacity(preparations.len());
    let mut prepared = SmallMap::with_capacity(preparations.len());
    for (requested, outcome) in preparations {
        match outcome {
            LoadingPreparationOutcome::Need(need) => {
                all_need = Some(match all_need {
                    Some(current) => current.try_union(&need).unwrap_or_else(|error| {
                        panic!(
                            "root analysis dependency preparation Needs must be structurally compatible: \
                             {error:?}"
                        )
                    }),
                    None => need,
                });
            }
            LoadingPreparationOutcome::Complete(Err(error)) => {
                if first_outer.is_none() {
                    first_outer = Some(error);
                }
            }
            LoadingPreparationOutcome::Complete(Ok(value)) => match value {
                Ok(key) => {
                    let normalized = key.node.clone();
                    normalizations.insert(requested, normalized.clone());
                    prepared.insert(normalized, key);
                }
                Err(error) => {
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
            },
        }
    }
    if let Some(error) = first_outer {
        return LoadingPreparationOutcome::Complete(Err(error));
    }
    if let Some(need) = all_need {
        return LoadingPreparationOutcome::Need(need);
    }
    if let Some(error) = first_error {
        return analysis_semantic_complete(Err(error));
    }
    for dependency in dependencies {
        if let Some(normalized) = normalizations.get(&dependency.node) {
            dependency.node = normalized.clone();
        }
    }
    analysis_semantic_complete(Ok(prepared))
}

trait ComputedAnalysis {
    fn result(&self) -> &ConfiguredNodeResult;
}

impl ComputedAnalysis for ConfiguredNodeResult {
    fn result(&self) -> &ConfiguredNodeResult {
        self
    }
}

impl ComputedAnalysis for Arc<ConfiguredNodeResult> {
    fn result(&self) -> &ConfiguredNodeResult {
        self
    }
}

fn analysis_configured_key(key: &ConfiguredTargetKey) -> AnalysisConfiguredTargetKey {
    AnalysisConfiguredTargetKey::new(
        key.label().clone(),
        key.configuration().complete_identity_bytes(),
    )
}

fn configured_dependency_file_path(result: &ConfiguredNodeResult) -> String {
    let label = result.key().label();
    let package = label.package().package().as_str();
    if package.is_empty() {
        label.target().as_str().to_owned()
    } else {
        format!("{package}/{}", label.target())
    }
}

fn materialized_target_providers(
    result: &ConfiguredNodeResult,
) -> Result<ProviderCollection, AnalysisError> {
    if !matches!(
        result.kind(),
        ConfiguredNodeKind::SourceFile | ConfiguredNodeKind::GeneratedFile
    ) {
        return Ok(result.providers().clone());
    }
    let artifact = configured_dependency_artifact(result, None)?;
    let files = AnalysisDepset::new(
        DepsetOrder::Default,
        vec![AnalysisValue::artifact(artifact)],
        Vec::new(),
    )
    .map_err(|error| AnalysisError::message(error.to_string()))?;
    ProviderCollection::new(vec![ProviderValue::DefaultInfo(
        DefaultInfo::from_files(files)
            .map_err(|error| AnalysisError::message(error.to_string()))?,
    )])
    .map_err(|error| AnalysisError::message(error.to_string()))
}

fn configured_target_analysis_value(
    result: &ConfiguredNodeResult,
) -> Result<AnalysisValue, AnalysisError> {
    let identity = result.actual_configured_target().map_or_else(
        || AnalysisTargetIdentity::null(result.key().label().clone()),
        |key| AnalysisTargetIdentity::from(analysis_configured_key(key)),
    );
    Ok(AnalysisValue::configured_target(
        ConfiguredTargetValue::new(identity, materialized_target_providers(result)?),
    ))
}

fn derived_artifact(owner: &ConfiguredTargetKey, path: impl Into<String>) -> AnalysisArtifact {
    AnalysisArtifact::Derived {
        owner: analysis_configured_key(owner),
        output: ActionOutput::new(path, ActionOutputKind::File),
    }
}

fn configured_dependency_artifact(
    result: &ConfiguredNodeResult,
    explicit_path: Option<&str>,
) -> Result<AnalysisArtifact, AnalysisError> {
    match result.kind() {
        ConfiguredNodeKind::SourceFile => {
            Ok(AnalysisArtifact::Source(result.key().label().clone()))
        }
        ConfiguredNodeKind::GeneratedFile => {
            let mut producers = result
                .edges()
                .iter()
                .filter(|edge| {
                    matches!(
                        edge.kind(),
                        crate::configured_target::ConfiguredEdgeKind::GeneratedBy
                    )
                })
                .filter_map(|edge| edge.configured_target());
            let producer = producers.next().ok_or_else(|| {
                AnalysisError::message(format!(
                    "generated configured dependency {} has no generating target",
                    result.key().label()
                ))
            })?;
            if producers.next().is_some() {
                return Err(AnalysisError::message(format!(
                    "generated configured dependency {} has ambiguous generating targets",
                    result.key().label()
                )));
            }
            let path = configured_dependency_file_path(result);
            Ok(derived_artifact(producer, path))
        }
        _ => {
            let owner = result.actual_configured_target().ok_or_else(|| {
                AnalysisError::message(format!(
                    "configured dependency {} has no configured target identity",
                    result.key().label()
                ))
            })?;
            let path = explicit_path.ok_or_else(|| {
                AnalysisError::message(format!(
                    "configured dependency {} did not retain a file path",
                    result.key().label()
                ))
            })?;
            Ok(derived_artifact(owner, path))
        }
    }
}

fn configured_attribute_item(
    row: &crate::subrule::ConfiguredDependencyRow,
    result: &ConfiguredNodeResult,
) -> Result<AnalysisValue, AnalysisError> {
    if row.executable() {
        let executable = if matches!(
            result.kind(),
            ConfiguredNodeKind::SourceFile | ConfiguredNodeKind::GeneratedFile
        ) {
            configured_dependency_artifact(result, None)?
        } else {
            let path = result
                .providers()
                .default_info()
                .and_then(|info| info.files_to_run.executable.as_deref())
                .expect("configured executable validation ran before projection");
            configured_dependency_artifact(result, Some(path))?
        };
        return Ok(AnalysisValue::provider(ProviderOccurrence::new(
            ProviderIdentity::builtin("FilesToRunProvider"),
            [
                ("executable", AnalysisValue::artifact(executable)),
                ("runfiles_manifest", AnalysisValue::none()),
                ("repo_mapping_manifest", AnalysisValue::none()),
            ],
        )));
    }
    if row.allow_single_file() {
        let artifact = if matches!(
            result.kind(),
            ConfiguredNodeKind::SourceFile | ConfiguredNodeKind::GeneratedFile
        ) {
            configured_dependency_artifact(result, None)?
        } else {
            let artifacts = result
                .providers()
                .default_info()
                .map(DefaultInfo::file_artifacts)
                .unwrap_or_default();
            let [artifact] = artifacts.as_slice() else {
                return Err(AnalysisError::message(format!(
                    "configured dependency {} did not retain exactly one File",
                    result.key().label()
                )));
            };
            artifact.clone()
        };
        return Ok(AnalysisValue::artifact(artifact));
    }
    configured_target_analysis_value(result)
}

fn prepare_configured_attributes<T: ComputedAnalysis>(
    rows: &[crate::subrule::ConfiguredDependencyRow],
    keys: &[DeclaredDependencyKey],
    computed: &SmallMap<ConfiguredNodeKey, T>,
) -> Result<Vec<PreparedConfiguredAttribute>, AnalysisError> {
    rows.iter()
        .map(|row| {
            let values = keys
                .iter()
                .filter(|dependency| dependency.configured_row == Some(row.index))
                .map(|dependency| {
                    let result = computed.get(&dependency.node).ok_or_else(|| {
                        AnalysisError::new(format!(
                            "internal error: configured attribute result missing for `{}`",
                            dependency.node
                        ))
                    })?;
                    configured_attribute_item(row, result.result())
                })
                .collect::<Result<Vec<_>, _>>()?;
            let value = match row.kind {
                AttributeKind::Label => values
                    .into_iter()
                    .next()
                    .unwrap_or_else(AnalysisValue::none),
                AttributeKind::LabelList => AnalysisValue::list(values),
                _ => unreachable!("configured dependency row kinds were validated"),
            };
            Ok(PreparedConfiguredAttribute {
                owner: row.owner.clone(),
                user_name: row.user_name.clone(),
                value,
            })
        })
        .collect()
}

fn finish_analysis<T>(
    package: &LoadedPackage,
    configured_target: &ConfiguredTargetKey,
    resolved_attributes: Vec<ResolvedRuleAttribute>,
    declared_dependency_keys: &[DeclaredDependencyKey],
    configured_rows: &[crate::subrule::ConfiguredDependencyRow],
    computed: &SmallMap<ConfiguredNodeKey, T>,
    candidate_execution_platforms: Option<Vec<ConfiguredTargetKey>>,
    action_context: Arc<ConfiguredActionOwnerContext>,
    toolchain: Option<PreparedToolchain>,
    capture_events: bool,
    event_batch: &mut Option<EventBatch>,
) -> Result<ConfiguredNodeResult, AnalysisError>
where
    T: ComputedAnalysis,
{
    let target = package
        .targets
        .iter()
        .find(|target| target.name == configured_target.label().target().as_str())
        .expect("configured Starlark rule remains present in its loaded package");
    let visibility = package.effective_visibility(target);
    let visibility_labels = visibility
        .as_ref()
        .map_or(&[][..], |visibility| visibility.dependency_labels());
    for label in visibility_labels {
        require_root_delegating_reference(label, "declaring visibility")?;
    }
    let mut resolved_attributes = resolved_attributes;
    for row in configured_rows.iter().filter(|row| row.owner.is_none()) {
        let attribute = resolved_attributes
            .iter_mut()
            .find(|attribute| attribute.declaration_name == row.attribute)
            .ok_or_else(|| {
                AnalysisError::message(format!(
                    "configured rule attribute `{}` is missing from resolved attributes",
                    row.attribute
                ))
            })?;
        attribute.value = match row.kind {
            AttributeKind::Label => row
                .labels
                .first()
                .cloned()
                .map(CoercedAttributeValue::Label)
                .unwrap_or(CoercedAttributeValue::None),
            AttributeKind::LabelList => CoercedAttributeValue::LabelList(row.labels.clone().into()),
            _ => unreachable!("configured dependency row kinds were validated"),
        };
    }
    let mut dependencies = Vec::new();
    let mut edges = Vec::with_capacity(declared_dependency_keys.len() + visibility_labels.len());
    for dependency in declared_dependency_keys {
        let result = computed.get(&dependency.node).ok_or_else(|| {
            AnalysisError::new(format!(
                "internal error: dependency result missing for `{}`",
                dependency.node
            ))
        })?;
        validate_configured_dependency(dependency, result.result())?;
    }
    let configured_attributes =
        prepare_configured_attributes(configured_rows, declared_dependency_keys, computed)?;
    for dependency in declared_dependency_keys {
        let result = computed.get(&dependency.node).ok_or_else(|| {
            AnalysisError::new(format!(
                "internal error: dependency result missing for `{}`",
                dependency.node
            ))
        })?;
        let kind = if dependency.hidden {
            crate::configured_target::ConfiguredEdgeKind::ImplicitAttribute {
                attribute: dependency.attribute.clone(),
                index: dependency.attribute_index,
                tool: dependency.exec_configuration,
            }
        } else {
            match (&dependency.node, &dependency.transition_output) {
                (ConfiguredNodeKey::Null(_), _) => {
                    crate::configured_target::ConfiguredEdgeKind::Source
                }
                (ConfiguredNodeKey::Configured(_), Some(output)) => {
                    crate::configured_target::ConfiguredEdgeKind::TransitionedAttribute {
                        attribute: dependency.attribute.clone(),
                        index: dependency.attribute_index,
                        output: output.clone(),
                    }
                }
                (ConfiguredNodeKey::Configured(_), None) => {
                    crate::configured_target::ConfiguredEdgeKind::OrdinaryAttribute {
                        attribute: dependency.attribute.clone(),
                        index: dependency.attribute_index,
                    }
                }
            }
        };
        if !dependency.hidden {
            dependencies.push(PreparedDependency {
                key: result
                    .result()
                    .actual_configured_target()
                    .cloned()
                    .map(ConfiguredNodeKey::configured)
                    .unwrap_or_else(|| result.result().key().clone()),
                providers: result.result().providers().clone(),
                attribute: dependency.attribute.clone(),
                target_shape: dependency.configured_row.is_some(),
            });
        }
        edges.push(crate::configured_target::ConfiguredEdge::new(
            result.result().key().clone(),
            kind,
        ));
    }
    edges.extend(visibility_labels.iter().cloned().map(|label| {
        crate::configured_target::ConfiguredEdge::new(
            ConfiguredNodeKey::null(label),
            crate::configured_target::ConfiguredEdgeKind::DeclaringVisibility,
        )
    }));
    let toolchain_context = toolchain.as_ref().map(|toolchain| {
        toolchain
            .action_context
            .toolchain()
            .expect("prepared toolchain retains its context")
            .clone()
    });
    if let Some(context) = &toolchain_context {
        for row in context.rows() {
            edges.push(crate::configured_target::ConfiguredEdge::new(
                row.requested().clone().into(),
                crate::configured_target::ConfiguredEdgeKind::ToolchainRequirement,
            ));
            if let Some(selection) = row.selected() {
                edges.push(crate::configured_target::ConfiguredEdge::new(
                    selection.implementation().clone().into(),
                    crate::configured_target::ConfiguredEdgeKind::SelectedToolchainImplementation,
                ));
            }
        }
    }
    edges.extend(
        candidate_execution_platforms
            .iter()
            .flatten()
            .cloned()
            .enumerate()
            .map(|(index, platform)| {
                crate::configured_target::ConfiguredEdge::new(
                    platform.into(),
                    crate::configured_target::ConfiguredEdgeKind::CandidateExecutionPlatform {
                        index: u32::try_from(index)
                            .expect("execution-platform candidate index fits u32"),
                    },
                )
            }),
    );
    let print_capture = capture_events.then(AnalysisPrintCapture::default);
    let label = configured_target.label();
    let value = evaluate_loaded_rule(
        package,
        label.target().as_str(),
        configured_target.clone(),
        label.package().package().as_str(),
        dependencies,
        resolved_attributes,
        configured_attributes,
        action_context,
        toolchain,
        print_capture
            .as_ref()
            .map(|capture| capture as &dyn PrintHandler),
    );
    *event_batch = print_capture.map(AnalysisPrintCapture::into_batch);
    let toolchain_topology = candidate_execution_platforms.map(|candidates| {
        ToolchainTopology::new(candidates, toolchain_context)
            .expect("selected execution platform came from the candidate sequence")
    });
    value
        .map_err(AnalysisError::from_loaded_rule_error)
        .map(|result| {
            let result = result.with_edges(edges);
            match toolchain_topology {
                Some(topology) => result.with_toolchain_topology(topology),
                None => result,
            }
        })
}

#[cfg(test)]
fn root_analysis_complete(
    result: Result<ConfiguredNodeResult, AnalysisError>,
) -> RootAnalysisKeyValue {
    LoadingPreparationOutcome::Complete(Arc::new(result.map(Arc::new)))
}

fn root_analysis_driver_complete(
    result: Result<ConfiguredNodeResult, AnalysisError>,
) -> RootAnalysisDriverValue {
    LoadingPreparationOutcome::Complete(Ok(Arc::new(result.map(Arc::new))))
}

fn project_legacy_analysis(value: RootAnalysisDriverValue) -> RootAnalysisKeyValue {
    match value {
        LoadingPreparationOutcome::Need(need) => LoadingPreparationOutcome::Need(need),
        LoadingPreparationOutcome::Complete(Ok(value)) => {
            LoadingPreparationOutcome::Complete(value)
        }
        LoadingPreparationOutcome::Complete(Err(error)) => {
            panic!("legacy configured analysis produced frontier error: {error}")
        }
    }
}

fn native_empty_providers() -> slug_build_api_v2::ProviderCollection {
    slug_build_api_v2::ProviderCollection::from_values(Vec::new(), false)
        .expect("an explicitly non-required provider collection may be empty")
}

fn platform_semantic_fact(
    package: &LoadedPackage,
    target: &str,
    label: &CanonicalLabel,
) -> Result<PlatformSemanticFact, AnalysisError> {
    let attrs = package.native_attributes(target).ok_or_else(|| {
        AnalysisError::new(format!(
            "platform {label} has no retained native attributes"
        ))
    })?;
    for name in [
        "parents",
        "remote_execution_properties",
        "flags",
        "required_settings",
        "check_toolchain_types",
        "allowed_toolchain_types",
    ] {
        let value = &attrs
            .get(name)
            .ok_or_else(|| AnalysisError::new(format!("platform {label} has no {name} fact")))?
            .1
            .value;
        let is_default = match value {
            CoercedAttributeValue::LabelList(values) => values.is_empty(),
            CoercedAttributeValue::String(value) => value.is_empty(),
            CoercedAttributeValue::StringList(values) => values.is_empty(),
            CoercedAttributeValue::Boolean(value) => !value,
            _ => false,
        };
        if !is_default {
            return Err(AnalysisError::new(format!(
                "platform {label} has unsupported nondefault attribute {name}"
            )));
        }
    }
    match &attrs
        .get("exec_properties")
        .ok_or_else(|| AnalysisError::new(format!("platform {label} has no exec_properties fact")))?
        .1
        .value
    {
        CoercedAttributeValue::StringDict(values) => {
            let mut values = values.iter().cloned().collect::<Vec<_>>();
            values.sort_by(|left, right| left.0.cmp(&right.0));
            if values.windows(2).any(|pair| pair[0].0 == pair[1].0) {
                return Err(AnalysisError::new(format!(
                    "platform {label} has duplicate exec_properties keys"
                )));
            }
            Ok(PlatformSemanticFact {
                exec_properties: values.into(),
            })
        }
        _ => Err(AnalysisError::new(format!(
            "platform {label} has invalid exec_properties fact"
        ))),
    }
}

fn package_declares_source_label(package: &LoadedPackage, label: &CanonicalLabel) -> bool {
    package.targets.iter().any(|target| {
        matches!(
            &target.kind,
            PackageTargetKind::StarlarkRule(rule) if rule.dependencies().contains(label)
        )
    })
}

fn require_root_delegating_reference(
    reference: &CanonicalLabel,
    role: &str,
) -> Result<(), AnalysisError> {
    if reference.package().repo().is_root() {
        Ok(())
    } else {
        Err(AnalysisError::new(format!(
            "external {role} reference is not supported: {reference}"
        )))
    }
}

fn source_path(
    workspace: &NormalizedAbsolutePath,
    label: &CanonicalLabel,
) -> NormalizedAbsolutePath {
    let mut path = workspace.as_path().to_path_buf();
    let package = label.package().package().as_str();
    if !package.is_empty() {
        path.push(package);
    }
    path.push(label.target().as_str());
    NormalizedAbsolutePath::new(path)
        .expect("validated package and target names remain below the absolute workspace path")
}

async fn resolve_source_input(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    path: NormalizedAbsolutePath,
    label: &CanonicalLabel,
) -> AnalysisSemanticOutcome<slug_workspace_v2::ResolvedPath> {
    match mode {
        ConfiguredAnalysisMode::Legacy => {
            match ctx
                .compute(&ResolvedPathKey::new(PathObservationNamespace::Host, path))
                .await
            {
                Ok(PathOutcome::Need(need)) => {
                    LoadingPreparationOutcome::Need(LoadingPreparationNeeds::path(need))
                }
                Ok(PathOutcome::Complete(Ok(resolved))) => analysis_semantic_complete(Ok(resolved)),
                Ok(PathOutcome::Complete(Err(error))) => analysis_semantic_complete(Err(
                    AnalysisError::new(format!("resolving source file {label}: {error:?}")),
                )),
                Err(error) => analysis_semantic_complete(Err(AnalysisError::new(format!(
                    "resolving source file through DICE: {error}"
                )))),
            }
        }
        ConfiguredAnalysisMode::Observed => {
            match ctx
                .compute(&ResolvedPathObservationKey::new(
                    PathObservationNamespace::Host,
                    path,
                ))
                .await
            {
                Ok(PathOutcome::Need(need)) => {
                    LoadingPreparationOutcome::Need(LoadingPreparationNeeds::path(need))
                }
                Ok(PathOutcome::Complete(Err(error))) => {
                    LoadingPreparationOutcome::Complete(Err(error))
                }
                Ok(PathOutcome::Complete(Ok(observed))) => match observed.result() {
                    Ok(resolved) => analysis_semantic_complete(Ok(resolved.dupe())),
                    Err(error) => analysis_semantic_complete(Err(AnalysisError::new(format!(
                        "resolving source file {label}: {error:?}"
                    )))),
                },
                Err(error) => analysis_semantic_complete(Err(AnalysisError::new(format!(
                    "resolving source file through DICE: {error}"
                )))),
            }
        }
    }
}

async fn compute_configured_child(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: NormalizedAbsolutePath,
    label: CanonicalLabel,
    configuration: ConfigurationKey,
) -> RootAnalysisDriverValue {
    let key =
        match prepare_configured_node_analysis_driver(ctx, mode, workspace, label, configuration)
            .await
        {
            LoadingPreparationOutcome::Need(need) => return LoadingPreparationOutcome::Need(need),
            LoadingPreparationOutcome::Complete(Err(error)) => {
                return LoadingPreparationOutcome::Complete(Err(error));
            }
            LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
                return root_analysis_driver_complete(Err(error));
            }
            LoadingPreparationOutcome::Complete(Ok(Ok(key))) => key,
        };
    match mode {
        ConfiguredAnalysisMode::Legacy => match ctx.compute(&key).await {
            Ok(LoadingPreparationOutcome::Need(need)) => LoadingPreparationOutcome::Need(need),
            Ok(LoadingPreparationOutcome::Complete(value)) => {
                LoadingPreparationOutcome::Complete(Ok(value))
            }
            Err(error) => root_analysis_driver_complete(Err(AnalysisError::new(format!(
                "computing configured child through DICE: {error}"
            )))),
        },
        ConfiguredAnalysisMode::Observed => {
            match ctx
                .compute(&ConfiguredNodeAnalysisObservationKey(key))
                .await
            {
                Ok(value) => value,
                Err(error) => root_analysis_driver_complete(Err(AnalysisError::new(format!(
                    "computing configured child through DICE: {error}"
                )))),
            }
        }
    }
}

async fn compute_actual_child(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: NormalizedAbsolutePath,
    requested: Arc<ConfiguredNodeResult>,
) -> RootAnalysisDriverValue {
    let actual = requested
        .actual_configured_target()
        .expect("configured child publishes actual identity");
    if requested.configured_target_key() == Some(actual) {
        return LoadingPreparationOutcome::Complete(Ok(Arc::new(Ok(requested))));
    }
    compute_configured_child(
        ctx,
        mode,
        workspace,
        actual.label().clone(),
        actual.configuration().clone(),
    )
    .await
}

async fn observed_configured_result(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    key: &ConfiguredTargetKey,
) -> AnalysisSemanticOutcome<Arc<ConfiguredNodeResult>> {
    let outcome = compute_configured_child(
        ctx,
        ConfiguredAnalysisMode::Observed,
        workspace.dupe(),
        key.label().clone(),
        key.configuration().clone(),
    )
    .await;
    match outcome {
        LoadingPreparationOutcome::Need(need) => LoadingPreparationOutcome::Need(need),
        LoadingPreparationOutcome::Complete(Err(error)) => {
            LoadingPreparationOutcome::Complete(Err(error))
        }
        LoadingPreparationOutcome::Complete(Ok(value)) => match value.as_ref() {
            Ok(result) => analysis_semantic_complete(Ok(result.dupe())),
            Err(error) => analysis_semantic_complete(Err(error.clone())),
        },
    }
}

async fn observed_actual_result(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    requested: Arc<ConfiguredNodeResult>,
) -> AnalysisSemanticOutcome<(ConfiguredTargetKey, Arc<ConfiguredNodeResult>)> {
    let Some(actual) = requested.actual_configured_target().cloned() else {
        return analysis_semantic_complete(Err(AnalysisError::message(
            "configured platform dependency has no actual configured target",
        )));
    };
    if requested.configured_target_key() == Some(&actual) {
        return analysis_semantic_complete(Ok((actual, requested)));
    }
    let result = analysis_value!(observed_configured_result(ctx, workspace, &actual).await);
    analysis_semantic_complete(Ok((actual, result)))
}

async fn compute_configured_constraint(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    requested: &ConfiguredTargetKey,
) -> AnalysisSemanticOutcome<ConfiguredActionPlatformConstraint> {
    let value = analysis_value!(observed_configured_result(ctx, workspace, requested).await);
    let (actual_value, value) =
        analysis_value!(observed_actual_result(ctx, workspace, value).await);
    if value.kind() != &ConfiguredNodeKind::ConstraintValue {
        return analysis_semantic_complete(Err(AnalysisError::message(format!(
            "expected configured constraint value at {}",
            requested.label()
        ))));
    }
    let Some(setting_key) = value
        .edges()
        .first()
        .and_then(|edge| edge.configured_target())
        .cloned()
    else {
        return analysis_semantic_complete(Err(AnalysisError::message(
            "constraint value has no configured setting edge",
        )));
    };
    let setting = analysis_value!(observed_configured_result(ctx, workspace, &setting_key).await);
    let (actual_setting, setting) =
        analysis_value!(observed_actual_result(ctx, workspace, setting).await);
    if setting.kind() != &ConfiguredNodeKind::ConstraintSetting {
        return analysis_semantic_complete(Err(AnalysisError::message(
            "constraint value references a non-constraint setting",
        )));
    }
    analysis_semantic_complete(Ok(ConfiguredActionPlatformConstraint::new(
        actual_value,
        actual_setting,
    )))
}

async fn compute_configured_platform(
    ctx: &mut DiceComputations<'_>,
    key: &ConfiguredPlatformKey,
) -> AnalysisSemanticOutcome<Arc<ConfiguredPlatform>> {
    let requested_result = analysis_value!(observed_configured_result(ctx, &key.0, &key.1).await);
    let (actual, platform) =
        analysis_value!(observed_actual_result(ctx, &key.0, requested_result).await);
    if key.1.configuration().kind() == ConfigurationKind::Exec {
        let structural = key
            .1
            .configuration()
            .slug_configuration()
            .expect("exec platform configuration remains structural");
        let normalized = match structural.to_exec_for_platform(actual.label()) {
            Ok(configuration) => ConfiguredTargetKey::new(
                actual.label().clone(),
                ConfigurationKey::from_slug(configuration),
            ),
            Err(error) => {
                return analysis_semantic_complete(Err(AnalysisError::message(error.to_string())));
            }
        };
        if normalized != actual {
            let platform =
                analysis_value!(resolution_platform(ctx, key.0.dupe(), normalized).await);
            return analysis_semantic_complete(Ok(Arc::new(ConfiguredPlatform::new(
                key.1.clone(),
                platform.actual().clone(),
                platform.fact().clone(),
                platform.constraints().to_vec().into(),
            ))));
        }
    }
    if platform.kind() != &ConfiguredNodeKind::Platform || !platform.diagnostics().is_empty() {
        return analysis_semantic_complete(Err(AnalysisError::message(format!(
            "configured platform {} has invalid semantic shape",
            key.1.label()
        ))));
    }
    let Some(fact) = platform.platform_semantic_fact().cloned() else {
        return analysis_semantic_complete(Err(AnalysisError::message(
            "configured platform has no platform fact",
        )));
    };
    let mut settings = SmallSet::with_capacity(platform.edges().len());
    let mut constraints = Vec::with_capacity(platform.edges().len());
    for edge in platform.edges() {
        if !matches!(
            edge.kind(),
            crate::configured_target::ConfiguredEdgeKind::PlatformConstraint { .. }
        ) {
            return analysis_semantic_complete(Err(AnalysisError::message(
                "configured platform has a non-constraint edge",
            )));
        }
        let Some(value_key) = edge.configured_target().cloned() else {
            return analysis_semantic_complete(Err(AnalysisError::message(
                "configured platform constraint is not configured",
            )));
        };
        let constraint =
            analysis_value!(compute_configured_constraint(ctx, &key.0, &value_key).await);
        if !settings.insert(constraint.constraint_setting().clone()) {
            return analysis_semantic_complete(Err(AnalysisError::message(
                "configured platform has a duplicate actual constraint setting",
            )));
        }
        constraints.push(constraint);
    }
    analysis_semantic_complete(Ok(Arc::new(ConfiguredPlatform::new(
        key.1.clone(),
        actual,
        fact,
        constraints.into(),
    ))))
}

fn root_analysis_success_eq(left: &RootAnalysisKeyValue, right: &RootAnalysisKeyValue) -> bool {
    match (left, right) {
        (LoadingPreparationOutcome::Complete(left), LoadingPreparationOutcome::Complete(right)) => {
            match (left.as_ref(), right.as_ref()) {
                (Ok(left), Ok(right)) => left == right,
                _ => false,
            }
        }
        _ => false,
    }
}

fn root_analysis_is_success(value: &RootAnalysisKeyValue) -> bool {
    matches!(
        value,
        LoadingPreparationOutcome::Complete(result) if result.as_ref().is_ok()
    )
}

fn require_supported_canonical_configured_target(
    node: &ConfiguredNodeKey,
    target: Option<&slug_loading_v2::PackageTarget>,
) -> Result<(), AnalysisError> {
    if node.label().package().repo().is_root() {
        return Ok(());
    }
    let supported = matches!(
        (node, target),
        (
            ConfiguredNodeKey::Configured(_),
            Some(slug_loading_v2::PackageTarget {
                kind: PackageTargetKind::Alias { .. }
                    | PackageTargetKind::ConfigSetting { .. }
                    | PackageTargetKind::NativeToolchain(_),
                ..
            })
        )
    ) || matches!(
        (node, target),
        (
            ConfiguredNodeKey::Configured(_),
            Some(slug_loading_v2::PackageTarget {
                kind: PackageTargetKind::StarlarkRule(_),
                ..
            })
        )
    );
    if supported {
        Ok(())
    } else {
        Err(AnalysisError::new(format!(
            "external repository configured target shape is not supported: {}",
            node.label()
        )))
    }
}

fn require_macro_namespace_compliance(
    package: &slug_loading_v2::LoadedPackage,
    label: &CanonicalLabel,
) -> Result<(), AnalysisError> {
    let Some(macro_name) = package
        .macro_origin(label.target().as_str())
        .and_then(|origin| origin.namespace_violation.as_deref())
    else {
        return Ok(());
    };
    Err(AnalysisError::message(format!(
        "Target {label} declared in symbolic macro '{macro_name}' violates macro naming rules and cannot be built. Name must be the same as the macro's name, or the macro's name followed by '_' (recommended), '-', or '.', and a non-empty string."
    )))
}

type PreparedToolchainOutcome = AnalysisSemanticOutcome<PreparedToolchain>;
type ConfiguredPackageValue = Arc<HostPackageInventory>;
type ConfiguredPackages = Vec<(PackageIdentifier, ConfiguredPackageValue)>;

#[derive(Debug, Clone)]
struct PreparedRegistrations {
    execution_platforms: Arc<[CanonicalLabel]>,
    toolchains: Arc<[CanonicalLabel]>,
}

fn toolchain_outcome(result: Result<PreparedToolchain, AnalysisError>) -> PreparedToolchainOutcome {
    analysis_semantic_complete(result)
}

enum RegistrationExpansionInput {
    Need(LoadingPreparationNeeds),
    OuterPath(ObservedPathFrontierError),
    OuterMessage(AnalysisError),
    Semantic(AnalysisError),
    Value(Arc<ModuleRegistrationExpansion>),
}

#[derive(Clone, Copy)]
enum RegistrationExpansionSource {
    Command,
    Module,
}

fn observed_registration_error(
    error: ModuleRegistrationExpansionObservationError,
) -> RegistrationExpansionInput {
    match error {
        ModuleRegistrationExpansionObservationError::Frontier(error) => {
            RegistrationExpansionInput::OuterPath(error)
        }
        error => RegistrationExpansionInput::OuterMessage(AnalysisError::new(error.to_string())),
    }
}

async fn compute_registration_expansion_input(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: NormalizedAbsolutePath,
    configuration: &slug_configuration_v2::SlugConfiguration,
    source: RegistrationExpansionSource,
    execution_platforms: bool,
    context: &str,
) -> RegistrationExpansionInput {
    match (mode, source) {
        (ConfiguredAnalysisMode::Legacy, RegistrationExpansionSource::Module) => {
            let key = if execution_platforms {
                ModuleRegistrationExpansionKey::execution_platforms(workspace)
            } else {
                ModuleRegistrationExpansionKey::toolchains(workspace)
            };
            match ctx.compute(&key).await {
                Ok(LoadingPreparationOutcome::Need(need)) => RegistrationExpansionInput::Need(need),
                Ok(LoadingPreparationOutcome::Complete(value)) => {
                    RegistrationExpansionInput::Value(value)
                }
                Err(error) => RegistrationExpansionInput::Semantic(AnalysisError::new(format!(
                    "{context}: {error}"
                ))),
            }
        }
        (ConfiguredAnalysisMode::Observed, RegistrationExpansionSource::Module) => {
            let key = if execution_platforms {
                ModuleRegistrationExpansionObservationKey::execution_platforms(workspace)
            } else {
                ModuleRegistrationExpansionObservationKey::toolchains(workspace)
            };
            match ctx.compute(&key).await {
                Ok(LoadingPreparationOutcome::Need(need)) => RegistrationExpansionInput::Need(need),
                Ok(LoadingPreparationOutcome::Complete(Err(error))) => {
                    observed_registration_error(error)
                }
                Ok(LoadingPreparationOutcome::Complete(Ok(observed))) => {
                    RegistrationExpansionInput::Value(observed.result().dupe())
                }
                Err(error) => RegistrationExpansionInput::Semantic(AnalysisError::new(format!(
                    "{context}: {error}"
                ))),
            }
        }
        (ConfiguredAnalysisMode::Legacy, RegistrationExpansionSource::Command) => {
            let key = if execution_platforms {
                CommandRegistrationExpansionKey::execution_platforms(
                    workspace,
                    configuration.dupe(),
                )
            } else {
                CommandRegistrationExpansionKey::toolchains(workspace, configuration.dupe())
            };
            match ctx.compute(&key).await {
                Ok(LoadingPreparationOutcome::Need(need)) => RegistrationExpansionInput::Need(need),
                Ok(LoadingPreparationOutcome::Complete(value)) => {
                    RegistrationExpansionInput::Value(value)
                }
                Err(error) => RegistrationExpansionInput::Semantic(AnalysisError::new(format!(
                    "{context}: {error}"
                ))),
            }
        }
        (ConfiguredAnalysisMode::Observed, RegistrationExpansionSource::Command) => {
            let key = if execution_platforms {
                CommandRegistrationExpansionObservationKey::execution_platforms(
                    workspace,
                    configuration.dupe(),
                )
            } else {
                CommandRegistrationExpansionObservationKey::toolchains(
                    workspace,
                    configuration.dupe(),
                )
            };
            match ctx.compute(&key).await {
                Ok(LoadingPreparationOutcome::Need(need)) => RegistrationExpansionInput::Need(need),
                Ok(LoadingPreparationOutcome::Complete(Err(error))) => {
                    observed_registration_error(error)
                }
                Ok(LoadingPreparationOutcome::Complete(Ok(observed))) => {
                    RegistrationExpansionInput::Value(observed.result().dupe())
                }
                Err(error) => RegistrationExpansionInput::Semantic(AnalysisError::new(format!(
                    "{context}: {error}"
                ))),
            }
        }
    }
}

fn merge_registration_labels(
    command: &ModuleRegistrationExpansion,
    module: &ModuleRegistrationExpansion,
) -> Result<Arc<[CanonicalLabel]>, AnalysisError> {
    let command = command
        .labels()
        .map_err(|error| AnalysisError::new(error.to_string()))?;
    let module = module
        .labels()
        .map_err(|error| AnalysisError::new(error.to_string()))?;
    let mut seen = SmallSet::with_capacity(command.len() + module.len());
    let mut labels = Vec::with_capacity(command.len() + module.len());
    for label in command.iter().chain(module.iter()) {
        if seen.insert(label.clone()) {
            labels.push(label.clone());
        }
    }
    Ok(labels.into())
}

async fn prepare_registrations(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: &NormalizedAbsolutePath,
    configuration: &ConfigurationKey,
    has_toolchain_requirement: bool,
    has_local_declarations: bool,
) -> AnalysisSemanticOutcome<Option<PreparedRegistrations>> {
    if !has_toolchain_requirement && !has_local_declarations {
        return analysis_semantic_complete(Ok(None));
    }
    let Some(structural) = configuration.slug_configuration() else {
        return analysis_semantic_complete(Err(AnalysisError::new(
            "registration preparation requires a structural Slug configuration",
        )));
    };
    let command_execution_platforms = compute_registration_expansion_input(
        ctx,
        mode,
        workspace.dupe(),
        structural,
        RegistrationExpansionSource::Command,
        true,
        "loading command execution-platform registrations through DICE",
    )
    .await;
    let module_execution_platforms = compute_registration_expansion_input(
        ctx,
        mode,
        workspace.dupe(),
        structural,
        RegistrationExpansionSource::Module,
        true,
        "loading MODULE execution-platform registrations through DICE",
    )
    .await;
    let command_toolchains = compute_registration_expansion_input(
        ctx,
        mode,
        workspace.dupe(),
        structural,
        RegistrationExpansionSource::Command,
        false,
        "loading command toolchain registrations through DICE",
    )
    .await;
    let module_toolchains = compute_registration_expansion_input(
        ctx,
        mode,
        workspace.dupe(),
        structural,
        RegistrationExpansionSource::Module,
        false,
        "loading MODULE toolchain registrations through DICE",
    )
    .await;
    let mut values = [None, None, None, None];
    let mut needs: Option<LoadingPreparationNeeds> = None;
    let mut first_outer_path = None;
    let mut first_outer_message = None;
    let mut first_error = None;
    for (index, outcome) in [
        command_execution_platforms,
        module_execution_platforms,
        command_toolchains,
        module_toolchains,
    ]
    .into_iter()
    .enumerate()
    {
        match outcome {
            RegistrationExpansionInput::Need(need) => {
                needs = Some(needs.map_or(need.clone(), |current| {
                    current
                        .try_union(&need)
                        .expect("registration family Needs agree")
                }));
            }
            RegistrationExpansionInput::OuterPath(error)
                if first_outer_path.is_none() && first_outer_message.is_none() =>
            {
                first_outer_path = Some(error);
            }
            RegistrationExpansionInput::OuterMessage(error)
                if first_outer_path.is_none() && first_outer_message.is_none() =>
            {
                first_outer_message = Some(error);
            }
            RegistrationExpansionInput::OuterPath(_)
            | RegistrationExpansionInput::OuterMessage(_) => {}
            RegistrationExpansionInput::Semantic(error) if first_error.is_none() => {
                first_error = Some(error);
            }
            RegistrationExpansionInput::Semantic(_) => {}
            RegistrationExpansionInput::Value(value) => match value.labels() {
                Ok(_) => values[index] = Some(value),
                Err(error) if first_error.is_none() => {
                    first_error = Some(AnalysisError::new(error.to_string()));
                }
                Err(_) => {}
            },
        }
    }
    if let Some(error) = first_outer_path {
        return LoadingPreparationOutcome::Complete(Err(error));
    }
    if let Some(error) = first_outer_message {
        return analysis_semantic_complete(Err(error));
    }
    if let Some(need) = needs {
        return LoadingPreparationOutcome::Need(need);
    }
    if let Some(error) = first_error {
        return analysis_semantic_complete(Err(error));
    }
    let [
        Some(command_execution_platforms),
        Some(module_execution_platforms),
        Some(command_toolchains),
        Some(module_toolchains),
    ] = values
    else {
        unreachable!("complete registration preparation retains all source/family values")
    };
    let execution_platforms = match merge_registration_labels(
        &command_execution_platforms,
        &module_execution_platforms,
    ) {
        Ok(labels) => labels,
        Err(error) => return analysis_semantic_complete(Err(error)),
    };
    let toolchains = match merge_registration_labels(&command_toolchains, &module_toolchains) {
        Ok(labels) => labels,
        Err(error) => return analysis_semantic_complete(Err(error)),
    };
    analysis_semantic_complete(Ok(Some(PreparedRegistrations {
        execution_platforms,
        toolchains,
    })))
}

fn local_toolchain_declarations(
    package: &LoadedPackage,
    rule_label: &CanonicalLabel,
) -> SmallSet<CanonicalLabel> {
    package
        .targets
        .iter()
        .filter_map(|target| match &target.kind {
            PackageTargetKind::NativeToolchain(NativeToolchainTarget::Toolchain {
                implementation,
                ..
            }) if implementation == rule_label => Some(
                rule_label.with_target(
                    TargetName::parse(&target.name)
                        .expect("loaded native toolchain name remains a target name"),
                ),
            ),
            _ => None,
        })
        .collect()
}

fn package_target<'a>(
    packages: &'a ConfiguredPackages,
    label: &CanonicalLabel,
) -> Result<&'a slug_loading_v2::PackageTarget, AnalysisError> {
    let package = packages
        .iter()
        .find(|(package, _)| configured_package_identity_matches(package, label))
        .ok_or_else(|| {
            AnalysisError::new(format!("toolchain label package was not loaded: {label}"))
        })?
        .1
        .loaded()
        .map_err(package_inventory_error)?;
    package
        .targets
        .iter()
        .find(|target| target.name == label.target().as_str())
        .ok_or_else(|| AnalysisError::new(format!("toolchain target was not found: {label}")))
}

fn configured_package_identity_matches(
    package: &PackageIdentifier,
    label: &CanonicalLabel,
) -> bool {
    package == label.package()
}

fn native_references(target: &slug_loading_v2::PackageTarget) -> Vec<CanonicalLabel> {
    match &target.kind {
        PackageTargetKind::NativeToolchain(NativeToolchainTarget::Toolchain {
            toolchain_type,
            implementation,
            exec_compatible_with,
            target_compatible_with,
            target_settings,
            ..
        }) => {
            let mut references = vec![toolchain_type.clone(), implementation.clone()];
            references.extend(exec_compatible_with.value().iter().cloned());
            references.extend(target_compatible_with.value().iter().cloned());
            references.extend(target_settings.value().selector_key_labels());
            references
        }
        PackageTargetKind::NativeToolchain(native) => native.semantic_references(),
        _ => Vec::new(),
    }
}

async fn compute_toolchain_analysis_input(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    key: ConfiguredNodeAnalysisKey,
    context: &str,
) -> RootAnalysisDriverValue {
    match mode {
        ConfiguredAnalysisMode::Legacy => match ctx.compute(&key).await {
            Ok(LoadingPreparationOutcome::Need(need)) => LoadingPreparationOutcome::Need(need),
            Ok(LoadingPreparationOutcome::Complete(value)) => {
                LoadingPreparationOutcome::Complete(Ok(value))
            }
            Err(error) => root_analysis_driver_complete(Err(AnalysisError::new(format!(
                "{context}: {error}"
            )))),
        },
        ConfiguredAnalysisMode::Observed => {
            match ctx
                .compute(&ConfiguredNodeAnalysisObservationKey(key))
                .await
            {
                Ok(value) => value,
                Err(error) => root_analysis_driver_complete(Err(AnalysisError::new(format!(
                    "{context}: {error}"
                )))),
            }
        }
    }
}

async fn load_configured_native_packages(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: &NormalizedAbsolutePath,
    labels: &mut Vec<CanonicalLabel>,
) -> AnalysisSemanticOutcome<ConfiguredPackages> {
    let mut packages = Vec::new();
    let mut seen = labels.iter().cloned().collect::<SmallSet<_>>();
    loop {
        let packages_to_load = labels
            .iter()
            .map(|label| label.package().clone())
            .filter(|package| !packages.iter().any(|(loaded, _)| loaded == package))
            .collect::<SmallSet<_>>();
        if packages_to_load.is_empty() {
            let mut next = SmallSet::new();
            for label in labels.clone() {
                if let Ok(target) = package_target(&packages, &label) {
                    for reference in native_references(target) {
                        if seen.insert(reference.clone()) {
                            next.insert(reference.package().clone());
                            labels.push(reference);
                        }
                    }
                }
            }
            if next
                .iter()
                .all(|package| packages.iter().any(|(loaded, _)| loaded == package))
            {
                return analysis_semantic_complete(Ok(packages));
            }
        }
        let outcomes = ctx
            .compute_join(packages_to_load.into_iter(), |ctx, package| {
                Box::pin(async move {
                    let value = compute_configured_package_input(
                        ctx,
                        mode,
                        workspace.dupe(),
                        package.clone(),
                        "loading toolchain package through DICE",
                    )
                    .await;
                    (package, value)
                })
            })
            .await;
        let mut needs: Option<LoadingPreparationNeeds> = None;
        let mut first_outer = None;
        let mut first_error = None;
        for (package, outcome) in outcomes {
            match outcome {
                LoadingPreparationOutcome::Need(need) => {
                    needs = Some(needs.map_or(need.clone(), |current| {
                        current.try_union(&need).expect("root package Needs agree")
                    }));
                }
                LoadingPreparationOutcome::Complete(Err(error)) if first_outer.is_none() => {
                    first_outer = Some(error);
                }
                LoadingPreparationOutcome::Complete(Err(_)) => {}
                LoadingPreparationOutcome::Complete(Ok(Err(error))) if first_error.is_none() => {
                    first_error = Some(error);
                }
                LoadingPreparationOutcome::Complete(Ok(Err(_))) => {}
                LoadingPreparationOutcome::Complete(Ok(Ok(value))) => {
                    packages.push((package, value));
                }
            }
        }
        if let Some(error) = first_outer {
            return LoadingPreparationOutcome::Complete(Err(error));
        }
        if let Some(need) = needs {
            return LoadingPreparationOutcome::Need(need);
        }
        if let Some(error) = first_error {
            return analysis_semantic_complete(Err(error));
        }
    }
}

async fn prepare_toolchain_target_settings(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: &NormalizedAbsolutePath,
    configuration: &ConfigurationKey,
    packages: &ConfiguredPackages,
    toolchains: &[CanonicalLabel],
) -> AnalysisSemanticOutcome<SmallMap<CanonicalLabel, bool>> {
    let selector_labels = toolchains
        .iter()
        .filter_map(|label| package_target(packages, label).ok())
        .filter_map(|target| match &target.kind {
            PackageTargetKind::NativeToolchain(NativeToolchainTarget::Toolchain {
                target_settings,
                ..
            }) => Some(target_settings.value()),
            _ => None,
        })
        .flat_map(CoercedAttributeValue::selector_key_labels)
        .collect::<SmallSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let selector_conditions = match prepare_configured_attribute_conditions(
        ctx,
        mode,
        workspace,
        configuration,
        selector_labels,
    )
    .await
    {
        LoadingPreparationOutcome::Need(need) => return LoadingPreparationOutcome::Need(need),
        LoadingPreparationOutcome::Complete(Err(error)) => {
            return LoadingPreparationOutcome::Complete(Err(error));
        }
        LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
            return analysis_semantic_complete(Err(error));
        }
        LoadingPreparationOutcome::Complete(Ok(Ok(conditions))) => conditions,
    };

    let mut resolved = Vec::with_capacity(toolchains.len());
    let mut selected_labels = SmallSet::new();
    for label in toolchains {
        let target = match package_target(packages, label) {
            Ok(target) => target,
            Err(error) => return analysis_semantic_complete(Err(error)),
        };
        let PackageTargetKind::NativeToolchain(NativeToolchainTarget::Toolchain {
            target_settings,
            ..
        }) = &target.kind
        else {
            return analysis_semantic_complete(Err(AnalysisError::message(format!(
                "registered toolchain is not toolchain: {label}"
            ))));
        };
        let value =
            match resolve_configured_attribute(target_settings.value(), &selector_conditions) {
                Ok(value) => value,
                Err(error) => {
                    return analysis_semantic_complete(Err(AnalysisError::message(format!(
                        "resolving target_settings for {label}: {error}"
                    ))));
                }
            };
        let CoercedAttributeValue::LabelList(settings) = value else {
            return analysis_semantic_complete(Err(AnalysisError::message(format!(
                "toolchain target_settings did not resolve to a label list: {label}"
            ))));
        };
        selected_labels.extend(settings.iter().cloned());
        resolved.push((label.clone(), settings));
    }

    let selected_conditions = match prepare_configured_attribute_conditions(
        ctx,
        mode,
        workspace,
        configuration,
        selected_labels
            .into_iter()
            .filter(|label| {
                !selector_conditions
                    .iter()
                    .any(|condition| &condition.label == label)
            })
            .collect(),
    )
    .await
    {
        LoadingPreparationOutcome::Need(need) => return LoadingPreparationOutcome::Need(need),
        LoadingPreparationOutcome::Complete(Err(error)) => {
            return LoadingPreparationOutcome::Complete(Err(error));
        }
        LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
            return analysis_semantic_complete(Err(error));
        }
        LoadingPreparationOutcome::Complete(Ok(Ok(conditions))) => conditions,
    };
    analysis_semantic_complete(Ok(resolved
        .into_iter()
        .map(|(label, settings)| {
            let eligible = settings.iter().all(|setting| {
                selector_conditions
                    .iter()
                    .chain(&selected_conditions)
                    .find(|condition| &condition.label == setting)
                    .is_some_and(|condition| condition.matches)
            });
            (label, eligible)
        })
        .collect()))
}

async fn prepare_selected_toolchain_context(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: &NormalizedAbsolutePath,
    owner: &ConfiguredTargetKey,
    resolution: &ConfiguredToolchainResolution,
) -> PreparedToolchainOutcome {
    let execution_platform = resolution.execution_platform().actual().clone();
    if execution_platform.configuration().kind() != ConfigurationKind::Exec {
        return toolchain_outcome(Err(AnalysisError::message(
            "selected toolchain implementation requires exec configuration",
        )));
    }
    let inputs = resolution
        .rows()
        .iter()
        .enumerate()
        .filter_map(|(index, row)| {
            row.implementation().map(|implementation| {
                let key = ConfiguredNodeAnalysisKey::new(
                    workspace.dupe(),
                    ConfiguredTargetKey::new(
                        implementation.clone(),
                        execution_platform.configuration().clone(),
                    ),
                )
                .expect("selected implementation inherits structural exec configuration");
                (index, key)
            })
        })
        .collect::<Vec<_>>();

    let outcomes = if inputs.is_empty() {
        Vec::new()
    } else {
        ctx.compute_join(inputs, |ctx, (index, key)| {
            Box::pin(async move {
                (
                    index,
                    compute_toolchain_analysis_input(
                        ctx,
                        mode,
                        key,
                        "analyzing selected toolchain through DICE",
                    )
                    .await,
                )
            })
        })
        .await
    };

    let mut all_need: Option<LoadingPreparationNeeds> = None;
    let mut first_outer = None;
    let mut first_error = None;
    let mut computed = SmallMap::with_capacity(outcomes.len());
    for (index, outcome) in outcomes {
        match outcome {
            LoadingPreparationOutcome::Need(need) => {
                all_need = Some(match all_need {
                    Some(current) => current.try_union(&need).unwrap_or_else(|error| {
                        panic!(
                            "selected toolchain Needs must be structurally compatible: {error:?}"
                        )
                    }),
                    None => need,
                });
            }
            LoadingPreparationOutcome::Complete(Err(error)) => {
                if first_outer.is_none() {
                    first_outer = Some(error);
                }
            }
            LoadingPreparationOutcome::Complete(Ok(value)) => match value.as_ref() {
                Ok(result) => {
                    computed.insert(index, result.dupe());
                }
                Err(error) => {
                    if first_error.is_none() {
                        first_error = Some(error.clone());
                    }
                }
            },
        }
    }
    if let Some(need) = all_need {
        return LoadingPreparationOutcome::Need(need);
    }
    if let Some(error) = first_outer {
        return LoadingPreparationOutcome::Complete(Err(error));
    }
    if let Some(error) = first_error {
        return toolchain_outcome(Err(error));
    }

    let mut rows = Vec::with_capacity(resolution.rows().len());
    for (index, row) in resolution.rows().iter().enumerate() {
        let selected = match (row.declaration(), row.implementation()) {
            (None, None) => None,
            (Some(declaration), Some(implementation)) => {
                let result = computed
                    .get(&index)
                    .expect("selected toolchain result remains ordered");
                if !matches!(
                    result.kind(),
                    ConfiguredNodeKind::Rule | ConfiguredNodeKind::Alias
                ) {
                    return toolchain_outcome(Err(AnalysisError::message(format!(
                        "selected toolchain implementation is not a Starlark rule: {implementation}"
                    ))));
                }
                let Some(info) = result.providers().toolchain_info().cloned() else {
                    return toolchain_outcome(Err(AnalysisError::message(format!(
                        "selected toolchain implementation does not provide ToolchainInfo: {implementation}"
                    ))));
                };
                let requested_implementation = ConfiguredTargetKey::new(
                    implementation.clone(),
                    execution_platform.configuration().clone(),
                );
                let actual_implementation = result
                    .actual_configured_target()
                    .cloned()
                    .ok_or_else(|| {
                        AnalysisError::message(format!(
                            "selected toolchain implementation has no configured identity: {implementation}"
                        ))
                    });
                let actual_implementation = match actual_implementation {
                    Ok(actual) => actual,
                    Err(error) => return toolchain_outcome(Err(error)),
                };
                Some(ConfiguredToolchainSelection::new(
                    declaration.clone(),
                    requested_implementation,
                    actual_implementation,
                    info,
                ))
            }
            _ => unreachable!("resolution validates declaration and implementation selection"),
        };
        rows.push(ConfiguredToolchainContextRow::new(
            row.requested().clone(),
            row.actual().clone(),
            row.mandatory(),
            selected,
        ));
    }
    let toolchain = match ConfiguredActionToolchainContext::new(execution_platform.clone(), rows) {
        Ok(toolchain) => Arc::new(toolchain),
        Err(error) => return toolchain_outcome(Err(AnalysisError::message(error))),
    };
    let action_context = ConfiguredActionOwnerContext::new(
        owner.clone(),
        ConfiguredActionExecGroup::Default,
        execution_platform,
        resolution.execution_platform().fact().clone(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        resolution.execution_platform().constraints().to_vec(),
        Some(toolchain),
        ConfiguredActionAspectProvenance::Absent,
    )
    .map(Arc::new)
    .map_err(AnalysisError::message);
    toolchain_outcome(action_context.map(|action_context| PreparedToolchain { action_context }))
}
fn resolution_success_eq(
    left: &ConfiguredToolchainResolutionOutcome,
    right: &ConfiguredToolchainResolutionOutcome,
) -> bool {
    matches!(
        (left, right),
        (
            LoadingPreparationOutcome::Complete(left),
            LoadingPreparationOutcome::Complete(right),
        ) if matches!((left.as_ref(), right.as_ref()), (Ok(left), Ok(right)) if left == right)
    )
}

fn resolution_is_success(value: &ConfiguredToolchainResolutionOutcome) -> bool {
    matches!(value, LoadingPreparationOutcome::Complete(value) if value.as_ref().is_ok())
}

async fn resolution_platform(
    ctx: &mut DiceComputations<'_>,
    workspace: NormalizedAbsolutePath,
    key: ConfiguredTargetKey,
) -> AnalysisSemanticOutcome<Arc<ConfiguredPlatform>> {
    let key = match ConfiguredPlatformKey::new(workspace, key) {
        Ok(key) => key,
        Err(error) => return analysis_semantic_complete(Err(error)),
    };
    match ctx.compute(&key).await {
        Ok(LoadingPreparationOutcome::Need(need)) => LoadingPreparationOutcome::Need(need),
        Ok(LoadingPreparationOutcome::Complete(Err(error))) => {
            LoadingPreparationOutcome::Complete(Err(error))
        }
        Ok(LoadingPreparationOutcome::Complete(Ok(value))) => analysis_semantic_complete(
            value
                .as_ref()
                .as_ref()
                .map(|value| value.dupe())
                .map_err(|error| error.clone()),
        ),
        Err(error) => analysis_semantic_complete(Err(AnalysisError::message(format!(
            "computing configured execution platform through DICE: {error}"
        )))),
    }
}

async fn prepare_execution_platform_registrations(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: &NormalizedAbsolutePath,
    configuration: &ConfigurationKey,
) -> AnalysisSemanticOutcome<Arc<[CanonicalLabel]>> {
    let Some(structural) = configuration.slug_configuration() else {
        return analysis_semantic_complete(Err(AnalysisError::message(
            "execution-platform registration requires a structural Slug configuration",
        )));
    };
    let command = compute_registration_expansion_input(
        ctx,
        mode,
        workspace.dupe(),
        structural,
        RegistrationExpansionSource::Command,
        true,
        "loading command execution-platform registrations through DICE",
    )
    .await;
    let module = compute_registration_expansion_input(
        ctx,
        mode,
        workspace.dupe(),
        structural,
        RegistrationExpansionSource::Module,
        true,
        "loading MODULE execution-platform registrations through DICE",
    )
    .await;
    let mut need = None;
    let mut outer = None;
    let mut outer_message = None;
    let mut semantic = None;
    let mut values = [None, None];
    for (index, value) in [command, module].into_iter().enumerate() {
        match value {
            RegistrationExpansionInput::Need(value) => {
                need = Some(
                    need.map_or(value.clone(), |current: LoadingPreparationNeeds| {
                        current.try_union(&value).expect("registration Needs agree")
                    }),
                );
            }
            RegistrationExpansionInput::OuterPath(error) if outer.is_none() => outer = Some(error),
            RegistrationExpansionInput::OuterMessage(error) if outer_message.is_none() => {
                outer_message = Some(error)
            }
            RegistrationExpansionInput::Semantic(error) if semantic.is_none() => {
                semantic = Some(error)
            }
            RegistrationExpansionInput::Value(value) => match value.labels() {
                Ok(_) => values[index] = Some(value),
                Err(error) if semantic.is_none() => {
                    semantic = Some(AnalysisError::message(error.to_string()))
                }
                Err(_) => {}
            },
            _ => {}
        }
    }
    if let Some(error) = outer {
        return LoadingPreparationOutcome::Complete(Err(error));
    }
    if let Some(error) = outer_message {
        return analysis_semantic_complete(Err(error));
    }
    if let Some(need) = need {
        return LoadingPreparationOutcome::Need(need);
    }
    if let Some(error) = semantic {
        return analysis_semantic_complete(Err(error));
    }
    let [Some(command), Some(module)] = values else {
        unreachable!("complete execution-platform registration retains both sources")
    };
    analysis_semantic_complete(merge_registration_labels(&command, &module))
}

async fn configured_candidate_execution_platforms(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: &NormalizedAbsolutePath,
    configuration: &ConfigurationKey,
    has_requirements: bool,
    has_local_declarations: bool,
) -> AnalysisSemanticOutcome<Vec<ConfiguredTargetKey>> {
    let labels = if has_requirements || has_local_declarations {
        match prepare_registrations(
            ctx,
            mode,
            workspace,
            configuration,
            has_requirements,
            has_local_declarations,
        )
        .await
        {
            LoadingPreparationOutcome::Need(need) => return LoadingPreparationOutcome::Need(need),
            LoadingPreparationOutcome::Complete(Err(error)) => {
                return LoadingPreparationOutcome::Complete(Err(error));
            }
            LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
                return analysis_semantic_complete(Err(error));
            }
            LoadingPreparationOutcome::Complete(Ok(Ok(Some(value)))) => value.execution_platforms,
            LoadingPreparationOutcome::Complete(Ok(Ok(None))) => Arc::from([]),
        }
    } else {
        match prepare_execution_platform_registrations(ctx, mode, workspace, configuration).await {
            LoadingPreparationOutcome::Need(need) => return LoadingPreparationOutcome::Need(need),
            LoadingPreparationOutcome::Complete(Err(error)) => {
                return LoadingPreparationOutcome::Complete(Err(error));
            }
            LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
                return analysis_semantic_complete(Err(error));
            }
            LoadingPreparationOutcome::Complete(Ok(Ok(value))) => value,
        }
    };
    let structural = configuration
        .slug_configuration()
        .expect("candidate preparation validates structural configuration");
    let mut labels = labels.to_vec();
    labels.push(match structural.host_platform_label() {
        Some(label) => label,
        None => {
            return analysis_semantic_complete(Err(AnalysisError::message(
                "configuration has no canonical host platform",
            )));
        }
    });
    let mut candidates = Vec::with_capacity(labels.len());
    let mut seen = SmallSet::with_capacity(labels.len());
    for (index, label) in labels.iter().enumerate() {
        let exec = match structural.to_exec_for_platform(label) {
            Ok(value) => ConfigurationKey::from_slug(value),
            Err(error) => {
                return analysis_semantic_complete(Err(AnalysisError::message(error.to_string())));
            }
        };
        let platform = analysis_value!(
            resolution_platform(
                ctx,
                workspace.dupe(),
                ConfiguredTargetKey::new(label.clone(), exec)
            )
            .await
        );
        if !seen.insert(platform.actual().clone()) {
            if index + 1 == labels.len() {
                continue;
            }
            return analysis_semantic_complete(Err(AnalysisError::message(format!(
                "registered execution platforms converge on actual platform {}",
                platform.actual()
            ))));
        }
        candidates.push(platform.actual().clone());
    }
    analysis_semantic_complete(Ok(candidates))
}

async fn actual_toolchain_target(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    requested: ConfiguredTargetKey,
) -> AnalysisSemanticOutcome<(ConfiguredTargetKey, Arc<ConfiguredNodeResult>)> {
    let requested_result =
        analysis_value!(observed_configured_result(ctx, workspace, &requested).await);
    observed_actual_result(ctx, workspace, requested_result).await
}

fn platform_has_actual_constraint(
    platform: &ConfiguredPlatform,
    constraint: &(CanonicalLabel, CanonicalLabel),
) -> bool {
    platform.constraints().iter().any(|candidate| {
        candidate.constraint_setting().label() == &constraint.0
            && candidate.constraint_value().label() == &constraint.1
    })
}

fn platform_satisfies_platform_constraints(
    platform: &ConfiguredPlatform,
    required: &ConfiguredPlatform,
) -> bool {
    required.constraints().iter().all(|constraint| {
        platform.constraints().iter().any(|candidate| {
            candidate.constraint_setting().label() == constraint.constraint_setting().label()
                && candidate.constraint_value().label() == constraint.constraint_value().label()
        })
    })
}

async fn prepared_toolchain_constraints(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    configuration: &ConfigurationKey,
    labels: SmallSet<CanonicalLabel>,
) -> AnalysisSemanticOutcome<SmallMap<CanonicalLabel, (CanonicalLabel, CanonicalLabel)>> {
    let mut resolved = SmallMap::with_capacity(labels.len());
    for label in labels {
        let requested = ConfiguredTargetKey::new(label.clone(), configuration.clone());
        let value =
            analysis_value!(compute_configured_constraint(ctx, workspace, &requested).await);
        resolved.insert(
            label,
            (
                value.constraint_setting().label().clone(),
                value.constraint_value().label().clone(),
            ),
        );
    }
    analysis_semantic_complete(Ok(resolved))
}

async fn compute_configured_toolchain_resolution(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    key: &ConfiguredToolchainResolutionKey,
) -> AnalysisSemanticOutcome<Arc<ConfiguredToolchainResolution>> {
    let target_platform_key =
        ConfiguredTargetPlatformKey::new(key.workspace.dupe(), key.configuration.clone())
            .expect("resolution key validates a structural analysis configuration");
    let target_platform = match ctx.compute(&target_platform_key).await {
        Ok(LoadingPreparationOutcome::Need(need)) => return LoadingPreparationOutcome::Need(need),
        Ok(LoadingPreparationOutcome::Complete(Err(error))) => {
            return LoadingPreparationOutcome::Complete(Err(error));
        }
        Ok(LoadingPreparationOutcome::Complete(Ok(value))) => match value.as_ref() {
            Ok(value) => value.dupe(),
            Err(error) => return analysis_semantic_complete(Err(error.clone())),
        },
        Err(error) => {
            return analysis_semantic_complete(Err(AnalysisError::message(format!(
                "computing configured target platform through DICE: {error}"
            ))));
        }
    };

    let registrations = if key.requirements.is_empty() {
        match prepare_execution_platform_registrations(
            ctx,
            mode,
            &key.workspace,
            &key.configuration,
        )
        .await
        {
            LoadingPreparationOutcome::Need(need) => return LoadingPreparationOutcome::Need(need),
            LoadingPreparationOutcome::Complete(Err(error)) => {
                return LoadingPreparationOutcome::Complete(Err(error));
            }
            LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
                return analysis_semantic_complete(Err(error));
            }
            LoadingPreparationOutcome::Complete(Ok(Ok(labels))) => (labels, Arc::from([])),
        }
    } else {
        match prepare_registrations(ctx, mode, &key.workspace, &key.configuration, true, false)
            .await
        {
            LoadingPreparationOutcome::Need(need) => return LoadingPreparationOutcome::Need(need),
            LoadingPreparationOutcome::Complete(Err(error)) => {
                return LoadingPreparationOutcome::Complete(Err(error));
            }
            LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
                return analysis_semantic_complete(Err(error));
            }
            LoadingPreparationOutcome::Complete(Ok(Ok(Some(value)))) => {
                (value.execution_platforms, value.toolchains)
            }
            LoadingPreparationOutcome::Complete(Ok(Ok(None))) => {
                unreachable!("required resolution prepares registrations")
            }
        }
    };
    let structural = key
        .configuration
        .slug_configuration()
        .expect("resolution key validates structural configuration");
    let host = match structural.host_platform_label() {
        Some(label) => label,
        None => {
            return analysis_semantic_complete(Err(AnalysisError::message(
                "configuration has no canonical host platform",
            )));
        }
    };
    let mut candidate_labels = registrations.0.to_vec();
    candidate_labels.push(host);
    let candidate_outcomes = ctx
        .compute_join(candidate_labels.iter().cloned(), |ctx, label| {
            let configuration = structural
                .to_exec_for_platform(&label)
                .map(ConfigurationKey::from_slug)
                .map_err(|error| AnalysisError::message(error.to_string()));
            let workspace = key.workspace.dupe();
            Box::pin(async move {
                let value = match configuration {
                    Ok(configuration) => {
                        resolution_platform(
                            ctx,
                            workspace,
                            ConfiguredTargetKey::new(label.clone(), configuration),
                        )
                        .await
                    }
                    Err(error) => analysis_semantic_complete(Err(error)),
                };
                (label, value)
            })
        })
        .await;
    let mut platforms = Vec::with_capacity(candidate_outcomes.len());
    let mut seen = SmallMap::with_capacity(candidate_outcomes.len());
    let mut need = None;
    let mut outer = None;
    let mut semantic = None;
    for (index, (label, value)) in candidate_outcomes.into_iter().enumerate() {
        match value {
            LoadingPreparationOutcome::Need(value) => {
                need = Some(
                    need.map_or(value.clone(), |current: LoadingPreparationNeeds| {
                        current.try_union(&value).expect("platform Needs agree")
                    }),
                )
            }
            LoadingPreparationOutcome::Complete(Err(error)) if outer.is_none() => {
                outer = Some(error)
            }
            LoadingPreparationOutcome::Complete(Ok(Err(error))) if semantic.is_none() => {
                semantic = Some(error)
            }
            LoadingPreparationOutcome::Complete(Ok(Ok(platform))) => {
                if let Some(previous) = seen.get(platform.actual()) {
                    if index + 1 == candidate_labels.len() {
                        continue;
                    }
                    if semantic.is_none() {
                        semantic = Some(AnalysisError::message(format!(
                            "registered execution platforms {previous} and {label} resolve to the same actual platform {}",
                            platform.actual()
                        )));
                    }
                    continue;
                }
                seen.insert(platform.actual().clone(), label);
                platforms.push(platform);
            }
            _ => {}
        }
    }
    if let Some(error) = outer {
        return LoadingPreparationOutcome::Complete(Err(error));
    }
    if let Some(need) = need {
        return LoadingPreparationOutcome::Need(need);
    }
    if let Some(error) = semantic {
        return analysis_semantic_complete(Err(error));
    }
    let selected_empty = platforms
        .first()
        .cloned()
        .expect("host platform is always a candidate");
    if key.requirements.is_empty() {
        return analysis_semantic_complete(Ok(Arc::new(ConfiguredToolchainResolution::new(
            target_platform,
            selected_empty,
            Arc::from([]),
        ))));
    }

    let mut labels = registrations.1.to_vec();
    labels.extend(
        key.requirements
            .iter()
            .map(|requirement| requirement.label().clone()),
    );
    let _packages =
        match load_configured_native_packages(ctx, mode, &key.workspace, &mut labels).await {
            LoadingPreparationOutcome::Need(need) => return LoadingPreparationOutcome::Need(need),
            LoadingPreparationOutcome::Complete(Err(error)) => {
                return LoadingPreparationOutcome::Complete(Err(error));
            }
            LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
                return analysis_semantic_complete(Err(error));
            }
            LoadingPreparationOutcome::Complete(Ok(Ok(value))) => value,
        };
    let mut actual_requirements = Vec::with_capacity(key.requirements.len());
    for requirement in key.requirements.iter() {
        let requested =
            ConfiguredTargetKey::new(requirement.label().clone(), key.configuration.clone());
        let (actual, value) =
            analysis_value!(actual_toolchain_target(ctx, &key.workspace, requested.clone()).await);
        if value.kind() != &ConfiguredNodeKind::ToolchainType {
            return analysis_semantic_complete(Err(AnalysisError::message(format!(
                "required toolchain type is not toolchain_type: {}",
                requirement.label()
            ))));
        }
        actual_requirements.push((requested, actual, requirement.mandatory()));
    }
    let mut declarations = Vec::with_capacity(registrations.1.len());
    for declaration in registrations.1.iter() {
        let requested = ConfiguredTargetKey::new(declaration.clone(), key.configuration.clone());
        let (actual, value) =
            analysis_value!(actual_toolchain_target(ctx, &key.workspace, requested).await);
        if value.kind() != &ConfiguredNodeKind::ToolchainDeclaration {
            return analysis_semantic_complete(Err(AnalysisError::message(format!(
                "registered toolchain is not toolchain: {declaration}"
            ))));
        }
        declarations.push(actual.label().clone());
    }
    let mut declaration_labels = declarations.clone();
    let packages =
        match load_configured_native_packages(ctx, mode, &key.workspace, &mut declaration_labels)
            .await
        {
            LoadingPreparationOutcome::Need(need) => return LoadingPreparationOutcome::Need(need),
            LoadingPreparationOutcome::Complete(Err(error)) => {
                return LoadingPreparationOutcome::Complete(Err(error));
            }
            LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
                return analysis_semantic_complete(Err(error));
            }
            LoadingPreparationOutcome::Complete(Ok(Ok(value))) => value,
        };
    let mut declaration_types = SmallMap::with_capacity(declarations.len());
    for declaration in &declarations {
        let target = match package_target(&packages, declaration) {
            Ok(target) => target,
            Err(error) => return analysis_semantic_complete(Err(error)),
        };
        let PackageTargetKind::NativeToolchain(NativeToolchainTarget::Toolchain {
            toolchain_type,
            ..
        }) = &target.kind
        else {
            unreachable!("checked declaration remains native toolchain");
        };
        let type_key = ConfiguredTargetKey::new(toolchain_type.clone(), key.configuration.clone());
        let (actual_type, value) =
            analysis_value!(actual_toolchain_target(ctx, &key.workspace, type_key).await);
        if value.kind() != &ConfiguredNodeKind::ToolchainType {
            return analysis_semantic_complete(Err(AnalysisError::message(format!(
                "registered toolchain has non-toolchain type: {declaration}"
            ))));
        }
        declaration_types.insert(declaration.clone(), actual_type);
    }
    let target_settings = match prepare_toolchain_target_settings(
        ctx,
        mode,
        &key.workspace,
        &key.configuration,
        &packages,
        &declarations,
    )
    .await
    {
        LoadingPreparationOutcome::Need(need) => return LoadingPreparationOutcome::Need(need),
        LoadingPreparationOutcome::Complete(Err(error)) => {
            return LoadingPreparationOutcome::Complete(Err(error));
        }
        LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
            return analysis_semantic_complete(Err(error));
        }
        LoadingPreparationOutcome::Complete(Ok(Ok(value))) => value,
    };
    let mut constraint_labels = SmallSet::new();
    for declaration in &declarations {
        let target = match package_target(&packages, declaration) {
            Ok(target) => target,
            Err(error) => return analysis_semantic_complete(Err(error)),
        };
        let PackageTargetKind::NativeToolchain(NativeToolchainTarget::Toolchain {
            exec_compatible_with,
            target_compatible_with,
            ..
        }) = &target.kind
        else {
            return analysis_semantic_complete(Err(AnalysisError::message(format!(
                "registered toolchain is not toolchain: {declaration}"
            ))));
        };
        constraint_labels.extend(exec_compatible_with.value().iter().cloned());
        constraint_labels.extend(target_compatible_with.value().iter().cloned());
    }
    let constraints = match prepared_toolchain_constraints(
        ctx,
        &key.workspace,
        &key.configuration,
        constraint_labels,
    )
    .await
    {
        LoadingPreparationOutcome::Need(need) => return LoadingPreparationOutcome::Need(need),
        LoadingPreparationOutcome::Complete(Err(error)) => {
            return LoadingPreparationOutcome::Complete(Err(error));
        }
        LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
            return analysis_semantic_complete(Err(error));
        }
        LoadingPreparationOutcome::Complete(Ok(Ok(value))) => value,
    };
    for declaration in &declarations {
        let target = match package_target(&packages, declaration) {
            Ok(target) => target,
            Err(error) => return analysis_semantic_complete(Err(error)),
        };
        let PackageTargetKind::NativeToolchain(NativeToolchainTarget::Toolchain {
            exec_compatible_with,
            target_compatible_with,
            use_target_platform_constraints,
            ..
        }) = &target.kind
        else {
            unreachable!("checked declaration remains native toolchain");
        };
        if *use_target_platform_constraints.value()
            && (!exec_compatible_with.value().is_empty()
                || !target_compatible_with.value().is_empty())
        {
            return analysis_semantic_complete(Err(AnalysisError::message(format!(
                "toolchain cannot combine use_target_platform_constraints with explicit execution or target constraints: {declaration}"
            ))));
        }
        for (name, values) in [
            ("execution", exec_compatible_with.value()),
            ("target", target_compatible_with.value()),
        ] {
            let mut settings = SmallSet::with_capacity(values.len());
            for value in values.iter() {
                let setting = constraints
                    .get(value)
                    .expect("all declaration constraints were prepared")
                    .0
                    .clone();
                if !settings.insert(setting) {
                    return analysis_semantic_complete(Err(AnalysisError::message(format!(
                        "toolchain has duplicate {name} constraint setting: {declaration}"
                    ))));
                }
            }
        }
    }
    let mut groups: SmallMap<ConfiguredTargetKey, (bool, Option<CanonicalLabel>)> = SmallMap::new();
    for (_, actual, mandatory) in &actual_requirements {
        let group = groups.entry(actual.clone()).or_insert((false, None));
        group.0 |= *mandatory;
    }
    let mut eligible = groups
        .keys()
        .cloned()
        .map(|actual| (actual, false))
        .collect::<SmallMap<_, _>>();
    let mut selected = None;
    for platform in &platforms {
        let mut selected_groups = groups.clone();
        let mut coverage = 0usize;
        for (actual_type, group) in selected_groups.iter_mut() {
            for declaration in &declarations {
                let target = match package_target(&packages, declaration) {
                    Ok(target) => target,
                    Err(error) => return analysis_semantic_complete(Err(error)),
                };
                let PackageTargetKind::NativeToolchain(NativeToolchainTarget::Toolchain {
                    exec_compatible_with,
                    target_compatible_with,
                    use_target_platform_constraints,
                    ..
                }) = &target.kind
                else {
                    continue;
                };
                if declaration_types.get(declaration) != Some(actual_type)
                    || target_settings.get(declaration) != Some(&true)
                {
                    continue;
                }
                let execution_matches = if *use_target_platform_constraints.value() {
                    platform_satisfies_platform_constraints(platform, &target_platform)
                } else {
                    exec_compatible_with.value().iter().all(|value| {
                        platform_has_actual_constraint(
                            platform,
                            constraints
                                .get(value)
                                .expect("all declaration constraints were prepared"),
                        )
                    })
                };
                if !execution_matches {
                    continue;
                }
                if !*use_target_platform_constraints.value()
                    && !target_compatible_with.value().iter().all(|value| {
                        platform_has_actual_constraint(
                            target_platform.as_ref(),
                            constraints
                                .get(value)
                                .expect("all declaration constraints were prepared"),
                        )
                    })
                {
                    continue;
                }
                *eligible
                    .get_mut(actual_type)
                    .expect("selected group remains eligible map key") = true;
                group.1 = Some(declaration.clone());
                coverage += 1;
                break;
            }
        }
        if selected_groups
            .values()
            .any(|(mandatory, declaration)| *mandatory && declaration.is_none())
        {
            continue;
        }
        if selected
            .as_ref()
            .is_none_or(|(_, best, _)| coverage > *best)
        {
            selected = Some((platform.clone(), coverage, selected_groups));
        }
    }
    for (actual, (mandatory, _)) in groups.iter() {
        if *mandatory && !eligible.get(actual).copied().unwrap_or(false) {
            return analysis_semantic_complete(Err(AnalysisError::message(format!(
                "no compatible toolchain was registered for {actual}"
            ))));
        }
    }
    let Some((execution_platform, _, selections)) = selected else {
        return analysis_semantic_complete(Err(AnalysisError::message(
            "no common execution platform satisfies mandatory toolchain types",
        )));
    };
    let rows = actual_requirements
        .into_iter()
        .map(|(requested, actual, mandatory)| {
            let declaration = selections
                .get(&actual)
                .expect("every requirement has a group")
                .1
                .clone();
            let implementation = declaration.as_ref().map(|declaration| {
                let target = package_target(&packages, declaration)
                    .expect("selected declaration remains in loaded packages");
                let PackageTargetKind::NativeToolchain(NativeToolchainTarget::Toolchain {
                    implementation,
                    ..
                }) = &target.kind
                else {
                    unreachable!("selected declaration remains a native toolchain")
                };
                implementation.clone()
            });
            ConfiguredToolchainResolutionRow::new(
                requested,
                actual,
                mandatory,
                declaration,
                implementation,
            )
        })
        .collect::<Vec<_>>()
        .into();
    analysis_semantic_complete(Ok(Arc::new(ConfiguredToolchainResolution::new(
        target_platform,
        execution_platform,
        rows,
    ))))
}

#[async_trait]
impl Key for ConfiguredToolchainResolutionKey {
    type Value = ConfiguredToolchainResolutionOutcome;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        match compute_configured_toolchain_resolution(ctx, ConfiguredAnalysisMode::Legacy, self)
            .await
        {
            LoadingPreparationOutcome::Need(need) => LoadingPreparationOutcome::Need(need),
            LoadingPreparationOutcome::Complete(Err(error)) => {
                panic!("legacy configured toolchain resolution produced frontier error: {error}")
            }
            LoadingPreparationOutcome::Complete(Ok(value)) => {
                LoadingPreparationOutcome::Complete(Arc::new(value))
            }
        }
    }

    fn equality(left: &Self::Value, right: &Self::Value) -> bool {
        resolution_success_eq(left, right)
    }
    fn validity(value: &Self::Value) -> bool {
        resolution_is_success(value)
    }
}

#[async_trait]
impl Key for ConfiguredToolchainResolutionObservationKey {
    type Value = ObservedConfiguredToolchainResolutionOutcome;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        match compute_configured_toolchain_resolution(
            ctx,
            ConfiguredAnalysisMode::Observed,
            &self.0,
        )
        .await
        {
            LoadingPreparationOutcome::Need(need) => LoadingPreparationOutcome::Need(need),
            LoadingPreparationOutcome::Complete(Err(error)) => {
                LoadingPreparationOutcome::Complete(Err(error))
            }
            LoadingPreparationOutcome::Complete(Ok(value)) => {
                LoadingPreparationOutcome::Complete(Ok(Arc::new(value)))
            }
        }
    }

    fn equality(left: &Self::Value, right: &Self::Value) -> bool {
        match (left, right) {
            (
                LoadingPreparationOutcome::Complete(Err(left)),
                LoadingPreparationOutcome::Complete(Err(right)),
            ) => left == right,
            (
                LoadingPreparationOutcome::Complete(Ok(left)),
                LoadingPreparationOutcome::Complete(Ok(right)),
            ) => matches!((left.as_ref(), right.as_ref()), (Ok(left), Ok(right)) if left == right),
            _ => false,
        }
    }

    fn validity(value: &Self::Value) -> bool {
        match value {
            LoadingPreparationOutcome::Complete(Err(_)) => true,
            LoadingPreparationOutcome::Complete(Ok(value)) => value.as_ref().is_ok(),
            LoadingPreparationOutcome::Need(_) => false,
        }
    }
}

impl fmt::Display for ConfiguredToolchainResolutionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "configured-toolchain-resolution:{}", self.configuration)
    }
}

impl fmt::Display for ConfiguredToolchainResolutionObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_legacy_starlark_option_removal_preserves_configuration() {
        let retained = CanonicalLabel::parse("@@//:retained").unwrap();
        let configuration = ConfigurationKey::target("legacy-checksum-with-owned-storage")
            .unwrap()
            .with_starlark_option(StarlarkOption::string(
                retained,
                "value",
                crate::key::StarlarkOptionScope::Default,
            ));
        let missing = CanonicalLabel::parse("@@//:missing").unwrap();
        let retained_before = configuration
            .starlark_options()
            .iter()
            .next()
            .expect("fixture carries one retained option")
            as *const StarlarkOption;

        let unchanged = without_starlark_option(&configuration, &missing);

        assert_eq!(unchanged, configuration);
        assert!(unchanged.starlark_option(&missing).is_none());
        let retained_after = unchanged
            .starlark_options()
            .iter()
            .next()
            .expect("unchanged configuration retains the fixture option")
            as *const StarlarkOption;
        assert_eq!(retained_after, retained_before);
    }

    #[test]
    fn configured_package_identity_distinguishes_same_paths_across_repositories() {
        let root = CanonicalLabel::parse("@@//collision:value").unwrap();
        let external = CanonicalLabel::parse("@@external//collision:value").unwrap();
        assert_eq!(root.package().package(), external.package().package());
        assert!(configured_package_identity_matches(root.package(), &root));
        assert!(!configured_package_identity_matches(
            root.package(),
            &external
        ));
        assert!(!configured_package_identity_matches(
            external.package(),
            &root
        ));
    }

    #[test]
    fn external_delegating_references_are_rejected_before_null_node_projection() {
        let root = CanonicalLabel::parse("@@//visibility:group").unwrap();
        let external = CanonicalLabel::parse("@@external//visibility:group").unwrap();

        assert!(require_root_delegating_reference(&root, "declaring visibility").is_ok());
        for role in ["declaring visibility", "package-group include"] {
            let error = require_root_delegating_reference(&external, role).unwrap_err();
            assert_eq!(
                error.to_string(),
                format!("external {role} reference is not supported: {external}")
            );
        }
    }

    #[test]
    fn completed_analysis_error_is_invalid_and_not_equal_to_itself() {
        let error = root_analysis_complete(Err(AnalysisError::new("analysis failed")));

        assert!(!<ConfiguredNodeAnalysisKey as Key>::validity(&error));
        assert!(!<ConfiguredNodeAnalysisKey as Key>::equality(
            &error, &error
        ));
    }

    #[test]
    fn observed_outer_is_complete_while_semantic_error_stays_invalid() {
        let semantic_arc = Arc::new(Err(AnalysisError::new("analysis failed")));
        let semantic: RootAnalysisDriverValue =
            LoadingPreparationOutcome::Complete(Ok(semantic_arc.dupe()));
        assert!(!ConfiguredNodeAnalysisObservationKey::validity(&semantic));
        assert!(!ConfiguredNodeAnalysisObservationKey::equality(
            &semantic, &semantic
        ));
        let LoadingPreparationOutcome::Complete(projected) =
            project_legacy_analysis(semantic.clone())
        else {
            unreachable!()
        };
        assert!(Arc::ptr_eq(&semantic_arc, &projected));
        let demand = slug_workspace_v2::PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new("/outer").unwrap(),
            slug_workspace_v2::PathObservationOperation::Lstat,
        );
        let outer: RootAnalysisDriverValue =
            LoadingPreparationOutcome::Complete(Err(ObservedPathFrontierError::from(
                slug_workspace_v2::PathObservationEpochError::DuplicateDemand(demand),
            )));
        assert!(ConfiguredNodeAnalysisObservationKey::validity(&outer));
        assert!(ConfiguredNodeAnalysisObservationKey::equality(
            &outer, &outer
        ));
    }
}

#[async_trait]
impl Key for ConfiguredNodeAnalysisKey {
    type Value = RootAnalysisKeyValue;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let capture_events = ctx
            .per_transaction_data()
            .data
            .get::<CaptureEvaluationEvents>()
            .is_ok();
        let mut event_batch = None;
        let value = self
            .compute_inner(
                ctx,
                ConfiguredAnalysisMode::Legacy,
                capture_events,
                &mut event_batch,
            )
            .await;
        if capture_events && matches!(value, LoadingPreparationOutcome::Complete(Ok(_))) {
            ctx.store_evaluation_data(event_batch.unwrap_or_else(EventBatch::empty))
                .expect("ConfiguredNodeAnalysisKey stores exactly one local event batch");
        }
        project_legacy_analysis(value)
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        root_analysis_success_eq(x, y)
    }

    fn validity(value: &Self::Value) -> bool {
        root_analysis_is_success(value)
    }
}

#[async_trait]
impl Key for ConfiguredNodeAnalysisObservationKey {
    type Value = RootAnalysisDriverValue;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let capture_events = ctx
            .per_transaction_data()
            .data
            .get::<CaptureEvaluationEvents>()
            .is_ok();
        let mut event_batch = None;
        let value = self
            .0
            .compute_inner(
                ctx,
                ConfiguredAnalysisMode::Observed,
                capture_events,
                &mut event_batch,
            )
            .await;
        if capture_events && matches!(value, LoadingPreparationOutcome::Complete(Ok(_))) {
            ctx.store_evaluation_data(event_batch.unwrap_or_else(EventBatch::empty))
                .expect(
                    "ConfiguredNodeAnalysisObservationKey stores exactly one local event batch",
                );
        }
        value
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (
                LoadingPreparationOutcome::Complete(Err(left)),
                LoadingPreparationOutcome::Complete(Err(right)),
            ) => left == right,
            (
                LoadingPreparationOutcome::Complete(Ok(left)),
                LoadingPreparationOutcome::Complete(Ok(right)),
            ) => match (left.as_ref(), right.as_ref()) {
                (Ok(left), Ok(right)) => left == right,
                _ => false,
            },
            _ => false,
        }
    }

    fn validity(value: &Self::Value) -> bool {
        match value {
            LoadingPreparationOutcome::Complete(Err(_)) => true,
            LoadingPreparationOutcome::Complete(Ok(result)) => result.as_ref().is_ok(),
            LoadingPreparationOutcome::Need(_) => false,
        }
    }
}

impl ConfiguredNodeAnalysisKey {
    async fn compute_inner(
        &self,
        ctx: &mut DiceComputations<'_>,
        mode: ConfiguredAnalysisMode,
        capture_events: bool,
        event_batch: &mut Option<EventBatch>,
    ) -> RootAnalysisDriverValue {
        let node = &self.node;
        let label = node.label();
        let package_inventory = match compute_configured_package_input(
            ctx,
            mode,
            self.workspace.dupe(),
            label.package().clone(),
            "loading package through DICE",
        )
        .await
        {
            LoadingPreparationOutcome::Need(need) => {
                return LoadingPreparationOutcome::Need(need);
            }
            LoadingPreparationOutcome::Complete(Err(error)) => {
                return LoadingPreparationOutcome::Complete(Err(error));
            }
            LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
                return root_analysis_driver_complete(Err(error));
            }
            LoadingPreparationOutcome::Complete(Ok(Ok(value))) => value,
        };
        let package = match package_inventory.loaded() {
            Ok(package) => package,
            Err(error) => {
                return root_analysis_driver_complete(Err(package_inventory_error(error)));
            }
        };
        let target = package
            .targets
            .iter()
            .find(|target| target.name == label.target().as_str());
        if let Err(error) = require_macro_namespace_compliance(package, label) {
            return root_analysis_driver_complete(Err(error));
        }
        if let Err(error) = require_supported_canonical_configured_target(node, target) {
            return root_analysis_driver_complete(Err(error));
        }
        match (&self.node, target.map(|target| &target.kind)) {
            (ConfiguredNodeKey::Null(_), source_kind)
                if matches!(source_kind, Some(PackageTargetKind::ExportedFile))
                    || (source_kind.is_none() && package_declares_source_label(package, label)) =>
            {
                let source_path = source_path(&self.workspace, label);
                let resolved = match resolve_source_input(ctx, mode, source_path, label).await {
                    LoadingPreparationOutcome::Need(need) => {
                        return LoadingPreparationOutcome::Need(need);
                    }
                    LoadingPreparationOutcome::Complete(Err(error)) => {
                        return LoadingPreparationOutcome::Complete(Err(error));
                    }
                    LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
                        return root_analysis_driver_complete(Err(error));
                    }
                    LoadingPreparationOutcome::Complete(Ok(Ok(resolved))) => resolved,
                };
                match resolved.state() {
                    ResolvedPathState::Present(metadata)
                        if metadata.kind() == PathNodeKind::RegularFile =>
                    {
                        return root_analysis_driver_complete(Ok(
                            ConfiguredNodeResult::new_native(
                                self.node.clone(),
                                ConfiguredNodeKind::SourceFile,
                                native_empty_providers(),
                                None,
                            ),
                        ));
                    }
                    ResolvedPathState::Missing => {
                        return root_analysis_driver_complete(Err(
                            AnalysisError::target_not_found(
                                label.clone(),
                                package.build_file.clone(),
                            ),
                        ));
                    }
                    ResolvedPathState::Present(metadata) => {
                        return root_analysis_driver_complete(Err(AnalysisError::new(format!(
                            "source file {label} has unsupported filesystem kind {:?}",
                            metadata.kind()
                        ))));
                    }
                }
            }
            (ConfiguredNodeKey::Null(_), None) => {
                return root_analysis_driver_complete(Err(AnalysisError::target_not_found(
                    label.clone(),
                    package.build_file.clone(),
                )));
            }
            (
                ConfiguredNodeKey::Null(_),
                Some(PackageTargetKind::PackageGroup { includes, .. }),
            ) => {
                for include in includes.iter() {
                    if let Err(error) =
                        require_root_delegating_reference(include, "package-group include")
                    {
                        return root_analysis_driver_complete(Err(error));
                    }
                }
                let edges = includes
                    .iter()
                    .enumerate()
                    .map(|(index, include)| {
                        crate::configured_target::ConfiguredEdge::new(
                            ConfiguredNodeKey::null(include.clone()),
                            crate::configured_target::ConfiguredEdgeKind::PackageGroupInclude {
                                index: u32::try_from(index)
                                    .expect("package-group include index fits u32"),
                            },
                        )
                    })
                    .collect();
                return root_analysis_driver_complete(Ok(ConfiguredNodeResult::new_native(
                    self.node.clone(),
                    ConfiguredNodeKind::PackageGroup,
                    native_empty_providers(),
                    None,
                )
                .with_edges(edges)));
            }
            (
                ConfiguredNodeKey::Configured(configured_target),
                Some(PackageTargetKind::NativeToolchain(
                    NativeToolchainTarget::ConstraintSetting {
                        default_constraint_value,
                    },
                )),
            ) if matches!(
                configured_target.configuration().kind(),
                ConfigurationKind::Target | ConfigurationKind::Exec
            ) =>
            {
                if default_constraint_value.is_some() {
                    return root_analysis_driver_complete(Err(AnalysisError::new(format!(
                        "constraint setting defaults are unsupported: {label}"
                    ))));
                }
                return root_analysis_driver_complete(Ok(ConfiguredNodeResult::new_native(
                    self.node.clone(),
                    ConfiguredNodeKind::ConstraintSetting,
                    native_empty_providers(),
                    target.and_then(|target| target.rule_capability()).cloned(),
                )));
            }
            (
                ConfiguredNodeKey::Configured(configured_target),
                Some(PackageTargetKind::NativeToolchain(NativeToolchainTarget::ConstraintValue {
                    constraint_setting,
                })),
            ) if matches!(
                configured_target.configuration().kind(),
                ConfigurationKind::Target | ConfigurationKind::Exec
            ) =>
            {
                let child = root_value!(
                    compute_configured_child(
                        ctx,
                        mode,
                        self.workspace.dupe(),
                        constraint_setting.clone(),
                        configured_target.configuration().clone(),
                    )
                    .await
                );
                let actual = root_value!(
                    compute_actual_child(ctx, mode, self.workspace.dupe(), child.dupe(),).await
                );
                if actual.kind() != &ConfiguredNodeKind::ConstraintSetting {
                    return root_analysis_driver_complete(Err(AnalysisError::new(format!(
                        "constraint value {label} references a non-constraint setting {constraint_setting}"
                    ))));
                }
                return root_analysis_driver_complete(Ok(ConfiguredNodeResult::new_native(
                    self.node.clone(),
                    ConfiguredNodeKind::ConstraintValue,
                    native_empty_providers(),
                    target.and_then(|target| target.rule_capability()).cloned(),
                )
                .with_edges(vec![crate::configured_target::ConfiguredEdge::new(
                    child.key().clone(),
                    crate::configured_target::ConfiguredEdgeKind::ConstraintSetting,
                )])));
            }
            (
                ConfiguredNodeKey::Configured(configured_target),
                Some(PackageTargetKind::NativeToolchain(NativeToolchainTarget::Platform {
                    constraint_values,
                })),
            ) if matches!(
                configured_target.configuration().kind(),
                ConfigurationKind::Target | ConfigurationKind::Exec
            ) =>
            {
                let fact = match platform_semantic_fact(package, label.target().as_str(), label) {
                    Ok(fact) => fact,
                    Err(error) => return root_analysis_driver_complete(Err(error)),
                };
                let mut seen_settings = SmallSet::with_capacity(constraint_values.len());
                let mut edges = Vec::with_capacity(constraint_values.len());
                for (index, constraint_value) in constraint_values.iter().enumerate() {
                    let child = root_value!(
                        compute_configured_child(
                            ctx,
                            mode,
                            self.workspace.dupe(),
                            constraint_value.clone(),
                            configured_target.configuration().clone(),
                        )
                        .await
                    );
                    let actual = root_value!(
                        compute_actual_child(ctx, mode, self.workspace.dupe(), child.dupe(),).await
                    );
                    if actual.kind() != &ConfiguredNodeKind::ConstraintValue {
                        return root_analysis_driver_complete(Err(AnalysisError::new(format!(
                            "platform {label} references a non-constraint value {constraint_value}"
                        ))));
                    }
                    let Some(setting) = actual.edges().first().map(|edge| edge.target().clone())
                    else {
                        return root_analysis_driver_complete(Err(AnalysisError::new(format!(
                            "constraint value {constraint_value} has no setting edge"
                        ))));
                    };
                    if !seen_settings.insert(setting) {
                        return root_analysis_driver_complete(Err(AnalysisError::new(format!(
                            "execution platform has duplicate constraint setting: {label}"
                        ))));
                    }
                    edges.push(crate::configured_target::ConfiguredEdge::new(
                        child.key().clone(),
                        crate::configured_target::ConfiguredEdgeKind::PlatformConstraint {
                            index: u32::try_from(index)
                                .expect("platform constraint index fits u32"),
                        },
                    ));
                }
                return root_analysis_driver_complete(Ok(ConfiguredNodeResult::new_native(
                    self.node.clone(),
                    ConfiguredNodeKind::Platform,
                    native_empty_providers(),
                    target.and_then(|target| target.rule_capability()).cloned(),
                )
                .with_edges(edges)
                .with_platform_semantic_fact(fact)));
            }
            (
                ConfiguredNodeKey::Configured(configured_target),
                Some(PackageTargetKind::NativeToolchain(NativeToolchainTarget::ToolchainType)),
            ) if matches!(
                configured_target.configuration().kind(),
                ConfigurationKind::Target | ConfigurationKind::Exec
            ) =>
            {
                return root_analysis_driver_complete(Ok(ConfiguredNodeResult::new_native(
                    self.node.clone(),
                    ConfiguredNodeKind::ToolchainType,
                    native_empty_providers(),
                    target.and_then(|target| target.rule_capability()).cloned(),
                )));
            }
            (
                ConfiguredNodeKey::Configured(configured_target),
                Some(PackageTargetKind::NativeToolchain(NativeToolchainTarget::Toolchain {
                    ..
                })),
            ) if matches!(
                configured_target.configuration().kind(),
                ConfigurationKind::Target | ConfigurationKind::Exec
            ) =>
            {
                return root_analysis_driver_complete(Ok(ConfiguredNodeResult::new_native(
                    self.node.clone(),
                    ConfiguredNodeKind::ToolchainDeclaration,
                    native_empty_providers(),
                    target.and_then(|target| target.rule_capability()).cloned(),
                )));
            }
            (
                ConfiguredNodeKey::Configured(configured_target),
                Some(PackageTargetKind::NativeToolchain(native)),
            ) => {
                return root_analysis_driver_complete(Err(AnalysisError::new(format!(
                    "native {} target {label} is incompatible with {} configuration",
                    native.rule_class(),
                    configured_target.configuration().kind(),
                ))));
            }
            (
                ConfiguredNodeKey::Configured(configured_target),
                Some(PackageTargetKind::Alias { actual }),
            ) => {
                let cycle_guard = ctx.cycle_guard::<ConfiguredAnalysisCycleGuard>();
                let child_future = compute_configured_child(
                    ctx,
                    mode,
                    self.workspace.dupe(),
                    actual.clone(),
                    configured_target.configuration().clone(),
                );
                let child = match cycle_guard {
                    Ok(Some(guard)) => match guard.guard_this(child_future).await {
                        Ok(child) => root_value!(child),
                        Err(cycle) => {
                            return root_analysis_driver_complete(Err(AnalysisError::message(
                                cycle.to_string(),
                            )));
                        }
                    },
                    Ok(None) => root_value!(child_future.await),
                    Err(error) => {
                        return root_analysis_driver_complete(Err(AnalysisError::message(
                            format!("reading configured-analysis cycle guard: {error}"),
                        )));
                    }
                };
                return root_analysis_driver_complete(Ok(ConfiguredNodeResult::new_native(
                    self.node.clone(),
                    ConfiguredNodeKind::Alias,
                    child.providers().clone(),
                    target.and_then(|target| target.rule_capability()).cloned(),
                )
                .with_edges(vec![crate::configured_target::ConfiguredEdge::new(
                    child.key().clone(),
                    crate::configured_target::ConfiguredEdgeKind::AliasActual,
                )])
                .with_actual_configured_target(
                    child
                        .actual_configured_target()
                        .expect("configured alias child publishes actual identity")
                        .clone(),
                )));
            }
            (
                ConfiguredNodeKey::Configured(configured_target),
                Some(PackageTargetKind::GeneratedFile {
                    generating_rule, ..
                }),
            ) => {
                let producer = label.with_target(
                    TargetName::parse(generating_rule.as_str())
                        .expect("loaded generated-file producer name remains a target name"),
                );
                let child = root_value!(
                    compute_configured_child(
                        ctx,
                        mode,
                        self.workspace.dupe(),
                        producer,
                        configured_target.configuration().clone(),
                    )
                    .await
                );
                return root_analysis_driver_complete(Ok(ConfiguredNodeResult::new_native(
                    self.node.clone(),
                    ConfiguredNodeKind::GeneratedFile,
                    native_empty_providers(),
                    None,
                )
                .with_edges(vec![crate::configured_target::ConfiguredEdge::new(
                    child.key().clone(),
                    crate::configured_target::ConfiguredEdgeKind::GeneratedBy,
                )])));
            }
            (ConfiguredNodeKey::Configured(_), Some(PackageTargetKind::StarlarkRule(_))) => {}
            (ConfiguredNodeKey::Configured(_), None) => {
                return root_analysis_driver_complete(Err(AnalysisError::target_not_found(
                    label.clone(),
                    package.build_file.clone(),
                )));
            }
            (ConfiguredNodeKey::Configured(configured_target), Some(_)) => {
                let error = starlark_rule_implementation(package, configured_target)
                    .expect_err("non-Starlark configured nodes retain the existing error");
                return root_analysis_driver_complete(Err(error));
            }
            _ => {
                return root_analysis_driver_complete(Err(AnalysisError::new(format!(
                    "configured-node identity `{}` is incompatible with loaded target kind",
                    self.node
                ))));
            }
        }
        let configured_target = self
            .node
            .configured_target()
            .expect("Starlark rule nodes retain structural configuration");
        let implementation = match starlark_rule_implementation(package, configured_target) {
            Ok(implementation) => implementation,
            Err(error) => return root_analysis_driver_complete(Err(error)),
        };
        let configured_dependency_names = implementation
            .configured_dependency_attributes()
            .filter(|attribute| !attribute.is_hidden())
            .map(|attribute| attribute.name())
            .collect::<SmallSet<_>>();
        if let Some(schema) = implementation.schema().iter().find(|schema| {
            (schema.executable()
                || matches!(
                    schema.dependency_configuration(),
                    AttributeDependencyConfiguration::Exec
                ))
                && !configured_dependency_names.contains(schema.declaration_name())
        }) {
            return root_analysis_driver_complete(Err(
                AnalysisError::unsupported_configured_attribute(
                    configured_target.label().clone(),
                    schema.declaration_name(),
                    matches!(
                        schema.dependency_configuration(),
                        AttributeDependencyConfiguration::Exec
                    ),
                    schema.executable(),
                ),
            ));
        }
        let resolved_attributes = match prepare_configured_rule_attributes(
            ctx,
            mode,
            &self.workspace,
            package,
            configured_target,
        )
        .await
        {
            LoadingPreparationOutcome::Need(need) => {
                return LoadingPreparationOutcome::Need(need);
            }
            LoadingPreparationOutcome::Complete(Err(error)) => {
                return LoadingPreparationOutcome::Complete(Err(error));
            }
            LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
                return root_analysis_driver_complete(Err(error));
            }
            LoadingPreparationOutcome::Complete(Ok(Ok(values))) => values,
        };
        let mut declared_dependency_keys = match root_declared_dependency_keys(
            ctx,
            mode,
            &self.workspace,
            package,
            configured_target,
            &resolved_attributes,
        )
        .await
        {
            LoadingPreparationOutcome::Need(need) => {
                return LoadingPreparationOutcome::Need(need);
            }
            LoadingPreparationOutcome::Complete(Err(error)) => {
                return LoadingPreparationOutcome::Complete(Err(error));
            }
            LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
                return root_analysis_driver_complete(Err(error));
            }
            LoadingPreparationOutcome::Complete(Ok(Ok(keys))) => keys,
        };

        let structural_configuration = configured_target
            .configuration()
            .slug_configuration()
            .expect("production analysis retains structural configuration");
        let configured_rows =
            match configured_dependency_rows(implementation, structural_configuration) {
                Ok(rows) => rows,
                Err(error) => return root_analysis_driver_complete(Err(error)),
            };
        let has_exec_dependency = configured_rows.iter().any(|row| row.exec_configuration);
        let target_only_prepared = if has_exec_dependency {
            None
        } else {
            for row in &configured_rows {
                declared_dependency_keys.extend(row.into_keys(|label, _| {
                    ConfiguredNodeKey::configured(ConfiguredTargetKey::new(
                        label,
                        configured_target.configuration().clone(),
                    ))
                }));
            }
            Some(
                match prepare_declared_dependency_keys(
                    ctx,
                    mode,
                    &self.workspace,
                    &mut declared_dependency_keys,
                )
                .await
                {
                    LoadingPreparationOutcome::Need(need) => {
                        return LoadingPreparationOutcome::Need(need);
                    }
                    LoadingPreparationOutcome::Complete(Err(error)) => {
                        return LoadingPreparationOutcome::Complete(Err(error));
                    }
                    LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
                        return root_analysis_driver_complete(Err(error));
                    }
                    LoadingPreparationOutcome::Complete(Ok(Ok(prepared))) => prepared,
                },
            )
        };
        let requirements = match starlark_rule_implementation(package, configured_target) {
            Ok(rule) => Arc::from(rule.required_toolchains()),
            Err(error) => return root_analysis_driver_complete(Err(error)),
        };
        let resolution_key = match ConfiguredToolchainResolutionKey::new(
            self.workspace.dupe(),
            configured_target.configuration().clone(),
            requirements,
        ) {
            Ok(key) => key,
            Err(error) => return root_analysis_driver_complete(Err(error)),
        };
        let resolution = match mode {
            ConfiguredAnalysisMode::Legacy => match ctx.compute(&resolution_key).await {
                Ok(LoadingPreparationOutcome::Need(need)) => {
                    return LoadingPreparationOutcome::Need(need);
                }
                Ok(LoadingPreparationOutcome::Complete(value)) => match value.as_ref() {
                    Ok(value) => value.dupe(),
                    Err(error) => return root_analysis_driver_complete(Err(error.clone())),
                },
                Err(error) => {
                    return root_analysis_driver_complete(Err(AnalysisError::message(format!(
                        "computing configured toolchain resolution through DICE: {error}"
                    ))));
                }
            },
            ConfiguredAnalysisMode::Observed => match ctx
                .compute(&ConfiguredToolchainResolutionObservationKey(resolution_key))
                .await
            {
                Ok(LoadingPreparationOutcome::Need(need)) => {
                    return LoadingPreparationOutcome::Need(need);
                }
                Ok(LoadingPreparationOutcome::Complete(Err(error))) => {
                    return LoadingPreparationOutcome::Complete(Err(error));
                }
                Ok(LoadingPreparationOutcome::Complete(Ok(value))) => match value.as_ref() {
                    Ok(value) => value.dupe(),
                    Err(error) => return root_analysis_driver_complete(Err(error.clone())),
                },
                Err(error) => {
                    return root_analysis_driver_complete(Err(AnalysisError::message(format!(
                        "computing observed configured toolchain resolution through DICE: {error}"
                    ))));
                }
            },
        };
        let local_declarations = local_toolchain_declarations(package, configured_target.label());
        let candidate_execution_platforms = match configured_candidate_execution_platforms(
            ctx,
            mode,
            &self.workspace,
            configured_target.configuration(),
            !resolution.rows().is_empty(),
            !local_declarations.is_empty(),
        )
        .await
        {
            LoadingPreparationOutcome::Need(need) => return LoadingPreparationOutcome::Need(need),
            LoadingPreparationOutcome::Complete(Err(error)) => {
                return LoadingPreparationOutcome::Complete(Err(error));
            }
            LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
                return root_analysis_driver_complete(Err(error));
            }
            LoadingPreparationOutcome::Complete(Ok(Ok(value))) => value,
        };
        let selected_platform = resolution.execution_platform();
        let prepared = if has_exec_dependency {
            let exec_configuration = match structural_configuration
                .to_exec_for_platform(selected_platform.actual().label())
            {
                Ok(configuration) => ConfigurationKey::from_slug(configuration),
                Err(error) => {
                    return root_analysis_driver_complete(Err(AnalysisError::message(format!(
                        "projecting configured dependency Exec configuration: {error}"
                    ))));
                }
            };
            for row in &configured_rows {
                declared_dependency_keys.extend(row.into_keys(|label, exec| {
                    ConfiguredNodeKey::configured(ConfiguredTargetKey::new(
                        label,
                        if exec {
                            exec_configuration.clone()
                        } else {
                            configured_target.configuration().clone()
                        },
                    ))
                }));
            }
            match prepare_declared_dependency_keys(
                ctx,
                mode,
                &self.workspace,
                &mut declared_dependency_keys,
            )
            .await
            {
                LoadingPreparationOutcome::Need(need) => {
                    return LoadingPreparationOutcome::Need(need);
                }
                LoadingPreparationOutcome::Complete(Err(error)) => {
                    return LoadingPreparationOutcome::Complete(Err(error));
                }
                LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
                    return root_analysis_driver_complete(Err(error));
                }
                LoadingPreparationOutcome::Complete(Ok(Ok(prepared))) => prepared,
            }
        } else {
            target_only_prepared.expect("target-only dependencies were prepared before resolution")
        };
        let cycle_guard = ctx.cycle_guard::<ConfiguredAnalysisCycleGuard>();
        let child_future = async {
            let prepared_toolchain = if resolution.rows().is_empty() {
                None
            } else {
                match prepare_selected_toolchain_context(
                    ctx,
                    mode,
                    &self.workspace,
                    configured_target,
                    &resolution,
                )
                .await
                {
                    LoadingPreparationOutcome::Need(need) => {
                        return LoadingPreparationOutcome::Need(need);
                    }
                    LoadingPreparationOutcome::Complete(Err(error)) => {
                        return LoadingPreparationOutcome::Complete(Err(error));
                    }
                    LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
                        return analysis_semantic_complete(Err(error));
                    }
                    LoadingPreparationOutcome::Complete(Ok(Ok(value))) => Some(value),
                }
            };
            let outcomes = ctx
                .compute_join(prepared, |ctx, (node, key)| {
                    Box::pin(async move {
                        let result = match mode {
                            ConfiguredAnalysisMode::Legacy => match ctx.compute(&key).await {
                                Ok(LoadingPreparationOutcome::Need(need)) => {
                                    LoadingPreparationOutcome::Need(need)
                                }
                                Ok(LoadingPreparationOutcome::Complete(value)) => {
                                    LoadingPreparationOutcome::Complete(Ok(value))
                                }
                                Err(error) => {
                                    root_analysis_driver_complete(Err(AnalysisError::new(format!(
                                        "computing dependency `{node}` through DICE: {error}"
                                    ))))
                                }
                            },
                            ConfiguredAnalysisMode::Observed => match ctx
                                .compute(&ConfiguredNodeAnalysisObservationKey(key))
                                .await
                            {
                                Ok(value) => value,
                                Err(error) => {
                                    root_analysis_driver_complete(Err(AnalysisError::new(format!(
                                        "computing dependency `{node}` through DICE: {error}"
                                    ))))
                                }
                            },
                        };
                        (node, result)
                    })
                })
                .await;
            analysis_semantic_complete(Ok((prepared_toolchain, outcomes)))
        };
        let children = match cycle_guard {
            Ok(Some(guard)) => match guard.guard_this(child_future).await {
                Ok(children) => children,
                Err(cycle) => {
                    return root_analysis_driver_complete(Err(AnalysisError::message(
                        cycle.to_string(),
                    )));
                }
            },
            Ok(None) => child_future.await,
            Err(error) => {
                return root_analysis_driver_complete(Err(AnalysisError::message(format!(
                    "reading configured-analysis cycle guard: {error}"
                ))));
            }
        };
        let (prepared_toolchain, outcomes) = match children {
            LoadingPreparationOutcome::Need(need) => {
                return LoadingPreparationOutcome::Need(need);
            }
            LoadingPreparationOutcome::Complete(Err(error)) => {
                return LoadingPreparationOutcome::Complete(Err(error));
            }
            LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
                return root_analysis_driver_complete(Err(error));
            }
            LoadingPreparationOutcome::Complete(Ok(Ok(children))) => children,
        };
        let action_context = if let Some(toolchain) = &prepared_toolchain {
            toolchain.action_context.clone()
        } else {
            match ConfiguredActionOwnerContext::new(
                configured_target.clone(),
                ConfiguredActionExecGroup::Default,
                selected_platform.actual().clone(),
                selected_platform.fact().clone(),
                &BTreeMap::new(),
                &BTreeMap::new(),
                selected_platform.constraints().to_vec(),
                None,
                ConfiguredActionAspectProvenance::Absent,
            ) {
                Ok(context) => Arc::new(context),
                Err(error) => {
                    return root_analysis_driver_complete(Err(AnalysisError::message(error)));
                }
            }
        };
        let candidate_execution_platforms = Some(candidate_execution_platforms);
        let mut all_need: Option<LoadingPreparationNeeds> = None;
        let mut first_outer = None;
        let mut first_error = None;
        let mut computed = SmallMap::with_capacity(outcomes.len());
        for (node, outcome) in outcomes {
            match outcome {
                LoadingPreparationOutcome::Need(need) => {
                    all_need = Some(match all_need {
                        Some(current) => current.try_union(&need).unwrap_or_else(|error| {
                            panic!(
                                "root analysis dependency Needs must be structurally compatible: \
                                 {error:?}"
                            )
                        }),
                        None => need,
                    });
                }
                LoadingPreparationOutcome::Complete(Err(error)) => {
                    if first_outer.is_none() {
                        first_outer = Some(error);
                    }
                }
                LoadingPreparationOutcome::Complete(Ok(value)) => match value.as_ref() {
                    Ok(result) => {
                        computed.insert(node, result.dupe());
                    }
                    Err(error) => {
                        if first_error.is_none() {
                            first_error = Some(error.clone());
                        }
                    }
                },
            }
        }
        if let Some(error) = first_outer {
            return LoadingPreparationOutcome::Complete(Err(error));
        }
        if let Some(need) = all_need {
            return LoadingPreparationOutcome::Need(need);
        }
        if let Some(error) = first_error {
            return root_analysis_driver_complete(Err(error));
        }

        root_analysis_driver_complete(finish_analysis(
            package,
            configured_target,
            resolved_attributes,
            &declared_dependency_keys,
            &configured_rows,
            &computed,
            candidate_execution_platforms,
            action_context,
            prepared_toolchain,
            capture_events,
            event_batch,
        ))
    }
}
