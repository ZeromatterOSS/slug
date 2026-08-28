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
use slug_events_v2::CaptureEvaluationEvents;
use slug_events_v2::EvaluationEvent;
use slug_events_v2::EventBatch;
use slug_events_v2::StarlarkSourceLocation;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::PackageIdentifier;
use slug_identity_v2::TargetName;
use slug_loading_v2::AttributeKind;
use slug_loading_v2::AttributeProvenance;
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
use slug_loading_v2::attrs::TransitionDefinition;
use slug_loading_v2::package::BuildSettingDeclaration;
use slug_loading_v2::package::NativeToolchainTarget;
use slug_loading_v2::package::StarlarkRuleImplementation;
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
use crate::result::PlatformSemanticFact;
use crate::result::ToolchainSelection;
use crate::result::ToolchainTopology;
use crate::starlark_rule::LoadedRuleError;
use crate::starlark_rule::PreparedDependency;
use crate::starlark_rule::PreparedToolchain;
use crate::starlark_rule::evaluate_loaded_rule;

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

#[derive(Debug, Clone, Copy, Eq, PartialEq, Allocative)]
pub enum ConfiguredConditionMatch {
    Match,
    NoMatch,
}

pub type ConfiguredConditionOutcome = LoadingPreparationOutcome<
    Result<Arc<Result<ConfiguredConditionMatch, AnalysisError>>, ObservedPathFrontierError>,
>;

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
    let Some(_target) = package
        .targets
        .iter()
        .find(|target| target.name == requested.target().as_str())
    else {
        return analysis_semantic_complete(Err(AnalysisError::target_not_found(
            requested,
            package.build_file.clone(),
        )));
    };
    analysis_semantic_complete(ConfiguredNodeAnalysisKey::new(
        workspace,
        ConfiguredTargetKey::new(requested, configuration),
    ))
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
    if !declaration.constraint_values().value().is_empty() {
        return analysis_semantic_complete(Err(AnalysisError::message(format!(
            "config_setting {} uses constraint_values before the configured target-platform fact is available",
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
    if let Some(error) = first_outer {
        return LoadingPreparationOutcome::Complete(Err(error));
    }
    if let Some(need) = all_need {
        return LoadingPreparationOutcome::Need(need);
    }
    if let Some(error) = first_error {
        return analysis_semantic_complete(Err(error));
    }
    analysis_semantic_complete(Ok(if native_match && flag_match {
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

type RootAnalysisKeyValue =
    LoadingPreparationOutcome<Arc<Result<Arc<ConfiguredNodeResult>, AnalysisError>>>;
type RootAnalysisDriverValue =
    AnalysisDriverOutcome<Arc<Result<Arc<ConfiguredNodeResult>, AnalysisError>>>;

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
#[derive(Debug, Clone)]
struct DeclaredDependencyKey {
    attribute: CompactString,
    attribute_index: u32,
    node: ConfiguredNodeKey,
    transition_output: Option<CanonicalLabel>,
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

fn finish_analysis<T>(
    package: &LoadedPackage,
    configured_target: &ConfiguredTargetKey,
    resolved_attributes: Vec<ResolvedRuleAttribute>,
    declared_dependency_keys: &[DeclaredDependencyKey],
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
    let mut dependencies = Vec::new();
    let mut edges = Vec::with_capacity(declared_dependency_keys.len() + visibility_labels.len());
    for dependency in declared_dependency_keys {
        let result = computed.get(&dependency.node).ok_or_else(|| {
            AnalysisError::new(format!(
                "internal error: dependency result missing for `{}`",
                dependency.node
            ))
        })?;
        let kind = match (&dependency.node, &dependency.transition_output) {
            (ConfiguredNodeKey::Null(_), _) => crate::configured_target::ConfiguredEdgeKind::Source,
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
        };
        dependencies.push(PreparedDependency {
            key: result.result().key().clone(),
            providers: result.result().providers().clone(),
            attribute: dependency.attribute.clone(),
        });
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
    let selection = toolchain.as_ref().map(|toolchain| {
        toolchain
            .action_context
            .toolchain()
            .expect("prepared toolchain context retains a toolchain")
            .selection()
            .clone()
    });
    if let Some(toolchain) = &toolchain {
        let selection = toolchain
            .action_context
            .toolchain()
            .expect("prepared toolchain context retains a toolchain")
            .selection();
        edges.push(crate::configured_target::ConfiguredEdge::new(
            selection.toolchain_type().clone().into(),
            crate::configured_target::ConfiguredEdgeKind::ToolchainRequirement,
        ));
        edges.push(crate::configured_target::ConfiguredEdge::new(
            selection.implementation().clone().into(),
            crate::configured_target::ConfiguredEdgeKind::SelectedToolchainImplementation,
        ));
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
        action_context,
        toolchain,
        print_capture
            .as_ref()
            .map(|capture| capture as &dyn PrintHandler),
    );
    *event_batch = print_capture.map(AnalysisPrintCapture::into_batch);
    let toolchain_topology = candidate_execution_platforms.map(|candidates| {
        ToolchainTopology::new(candidates, selection)
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
                kind: PackageTargetKind::NativeToolchain(_),
                ..
            })
        )
    ) || matches!((node, target), (ConfiguredNodeKey::Configured(_), Some(target)) if is_marker_leaf_target(target));
    if supported {
        Ok(())
    } else {
        Err(AnalysisError::new(format!(
            "external repository configured target shape is not supported: {}",
            node.label()
        )))
    }
}

type PreparedToolchainOutcome = AnalysisSemanticOutcome<PreparedToolchain>;
type ConfiguredPackageValue = Arc<HostPackageInventory>;
type ConfiguredPackages = Vec<(PackageIdentifier, ConfiguredPackageValue)>;

#[derive(Debug, Clone)]
struct PreparedRegistrations {
    execution_platforms: Arc<[CanonicalLabel]>,
    toolchains: Arc<[CanonicalLabel]>,
}

impl PreparedRegistrations {
    fn execution_platforms(&self) -> &[CanonicalLabel] {
        &self.execution_platforms
    }

    fn toolchains(&self) -> &[CanonicalLabel] {
        &self.toolchains
    }
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

fn rule_execution_platforms(
    configuration: &ConfigurationKey,
    registrations: Option<&PreparedRegistrations>,
    local_declarations: &SmallSet<CanonicalLabel>,
    has_toolchain_requirement: bool,
) -> Result<Option<Vec<ConfiguredTargetKey>>, AnalysisError> {
    let Some(registrations) = registrations else {
        debug_assert!(!has_toolchain_requirement && local_declarations.is_empty());
        return Ok(None);
    };
    let toolchains = registrations.toolchains();
    if !has_toolchain_requirement {
        let registered = toolchains
            .iter()
            .any(|declaration| local_declarations.contains(declaration));
        if !registered {
            return Ok(None);
        }
    }
    let Some(structural) = configuration.slug_configuration() else {
        return Err(AnalysisError::new(
            "execution-platform topology requires a structural Slug configuration",
        ));
    };
    let execution_configuration = match structural.to_exec() {
        Ok(configuration) => ConfigurationKey::from_slug(configuration),
        Err(error) => return Err(AnalysisError::new(error.to_string())),
    };
    let platforms = registrations
        .execution_platforms()
        .iter()
        .cloned()
        .map(|label| ConfiguredTargetKey::new(label, execution_configuration.clone()))
        .collect();
    Ok(Some(platforms))
}

fn root_apparent_type(label: &CanonicalLabel) -> Result<CompactString, AnalysisError> {
    if !label.package().repo().is_root() {
        return Err(AnalysisError::new(format!(
            "toolchain requirements must be root labels: {label}"
        )));
    }
    let text = label.to_string();
    Ok(text
        .strip_prefix("@@")
        .expect("root canonical labels have the @@ spelling")
        .into())
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

fn constraint_value_setting(
    packages: &ConfiguredPackages,
    label: &CanonicalLabel,
) -> Result<CanonicalLabel, AnalysisError> {
    let target = package_target(packages, label)?;
    let PackageTargetKind::NativeToolchain(NativeToolchainTarget::ConstraintValue {
        constraint_setting,
    }) = &target.kind
    else {
        return Err(AnalysisError::new(format!(
            "expected constraint_value at {label}"
        )));
    };
    let setting = package_target(packages, constraint_setting)?;
    if !matches!(
        setting.kind,
        PackageTargetKind::NativeToolchain(NativeToolchainTarget::ConstraintSetting)
    ) {
        return Err(AnalysisError::new(format!(
            "constraint value {label} references a non-constraint setting {constraint_setting}"
        )));
    }
    Ok(constraint_setting.clone())
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

#[derive(Debug, Clone)]
struct PreparedExecutionPlatform {
    key: ConfiguredTargetKey,
    fact: PlatformSemanticFact,
    constraints: Vec<ConfiguredActionPlatformConstraint>,
}

type PreparedExecutionPlatformOutcome = AnalysisSemanticOutcome<PreparedExecutionPlatform>;

fn finish_execution_platform_analysis(
    analysis: RootAnalysisDriverValue,
) -> Result<Arc<ConfiguredNodeResult>, PreparedExecutionPlatformOutcome> {
    match analysis {
        LoadingPreparationOutcome::Need(need) => Err(LoadingPreparationOutcome::Need(need)),
        LoadingPreparationOutcome::Complete(Err(error)) => {
            Err(LoadingPreparationOutcome::Complete(Err(error)))
        }
        LoadingPreparationOutcome::Complete(Ok(value)) => match value.as_ref() {
            Ok(value) => Ok(value.dupe()),
            Err(error) => Err(analysis_semantic_complete(Err(error.clone()))),
        },
    }
}

async fn prepare_execution_platform(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: &NormalizedAbsolutePath,
    packages: &ConfiguredPackages,
    key: ConfiguredTargetKey,
) -> PreparedExecutionPlatformOutcome {
    let analysis_key = ConfiguredNodeAnalysisKey::new(workspace.dupe(), key.clone())
        .expect("execution platform inherits structural exec configuration");
    let analysis = compute_toolchain_analysis_input(
        ctx,
        mode,
        analysis_key,
        "analyzing selected execution platform through DICE",
    )
    .await;
    let result = match finish_execution_platform_analysis(analysis) {
        Ok(result) => result,
        Err(terminal) => return terminal,
    };
    if result.configured_target_key() != Some(&key)
        || result.kind() != &ConfiguredNodeKind::Platform
        || !result.diagnostics().is_empty()
        || result.platform_semantic_fact().is_none()
    {
        return analysis_semantic_complete(Err(AnalysisError::new(
            "selected execution platform analysis has invalid semantic shape",
        )));
    }
    let target = match package_target(packages, key.label()) {
        Ok(target) => target,
        Err(error) => return analysis_semantic_complete(Err(error)),
    };
    let PackageTargetKind::NativeToolchain(NativeToolchainTarget::Platform { constraint_values }) =
        &target.kind
    else {
        return analysis_semantic_complete(Err(AnalysisError::new(
            "selected execution platform is not platform",
        )));
    };
    if result.edges().len() != constraint_values.len() {
        return analysis_semantic_complete(Err(AnalysisError::new(
            "selected execution platform has mismatched constraint edges",
        )));
    }
    let mut constraints = Vec::with_capacity(constraint_values.len());
    for (index, (edge, value)) in result
        .edges()
        .iter()
        .zip(constraint_values.iter())
        .enumerate()
    {
        let value_key = ConfiguredTargetKey::new(value.clone(), key.configuration().clone());
        if edge.target() != &ConfiguredNodeKey::configured(value_key.clone())
            || !matches!(edge.kind(), crate::configured_target::ConfiguredEdgeKind::PlatformConstraint { index: edge_index } if edge_index == &u32::try_from(index).expect("constraint index fits u32"))
        {
            return analysis_semantic_complete(Err(AnalysisError::new(
                "selected execution platform has unordered constraint edges",
            )));
        }
        let setting = match constraint_value_setting(packages, value) {
            Ok(setting) => setting,
            Err(error) => return analysis_semantic_complete(Err(error)),
        };
        constraints.push(ConfiguredActionPlatformConstraint::new(
            value_key,
            ConfiguredTargetKey::new(setting, key.configuration().clone()),
        ));
    }
    analysis_semantic_complete(Ok(PreparedExecutionPlatform {
        key,
        fact: result
            .platform_semantic_fact()
            .expect("validated platform fact")
            .clone(),
        constraints,
    }))
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

async fn prepare_default_action_context(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: &NormalizedAbsolutePath,
    owner: &ConfiguredTargetKey,
    candidates: Option<&[ConfiguredTargetKey]>,
) -> AnalysisSemanticOutcome<Arc<ConfiguredActionOwnerContext>> {
    let Some([platform_key]) = candidates else {
        return analysis_semantic_complete(
            ConfiguredActionOwnerContext::unresolved_default(owner.clone())
                .map(Arc::new)
                .map_err(AnalysisError::new),
        );
    };
    let mut labels = vec![platform_key.label().clone()];
    let packages = match load_configured_native_packages(ctx, mode, workspace, &mut labels).await {
        LoadingPreparationOutcome::Need(need) => return LoadingPreparationOutcome::Need(need),
        LoadingPreparationOutcome::Complete(Err(error)) => {
            return LoadingPreparationOutcome::Complete(Err(error));
        }
        LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
            return analysis_semantic_complete(Err(error));
        }
        LoadingPreparationOutcome::Complete(Ok(Ok(packages))) => packages,
    };
    let platform =
        match prepare_execution_platform(ctx, mode, workspace, &packages, platform_key.clone())
            .await
        {
            LoadingPreparationOutcome::Need(need) => return LoadingPreparationOutcome::Need(need),
            LoadingPreparationOutcome::Complete(Err(error)) => {
                return LoadingPreparationOutcome::Complete(Err(error));
            }
            LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
                return analysis_semantic_complete(Err(error));
            }
            LoadingPreparationOutcome::Complete(Ok(Ok(platform))) => platform,
        };
    analysis_semantic_complete(
        ConfiguredActionOwnerContext::new(
            owner.clone(),
            ConfiguredActionExecGroup::Default,
            platform.key,
            platform.fact,
            &BTreeMap::new(),
            &BTreeMap::new(),
            platform.constraints,
            None,
            ConfiguredActionAspectProvenance::Absent,
        )
        .map(Arc::new)
        .map_err(AnalysisError::new),
    )
}

fn validate_constraint_settings(
    packages: &ConfiguredPackages,
    values: &[CanonicalLabel],
    duplicate: impl Fn() -> AnalysisError,
) -> Result<(), AnalysisError> {
    let mut settings = SmallSet::with_capacity(values.len());
    for value in values {
        let setting = constraint_value_setting(packages, value)?;
        if !settings.insert(setting) {
            return Err(duplicate());
        }
    }
    Ok(())
}

fn is_marker_leaf_target(target: &slug_loading_v2::PackageTarget) -> bool {
    let PackageTargetKind::StarlarkRule(rule) = &target.kind else {
        return false;
    };
    let marker = rule.values().iter().any(|value| {
        value.declaration_name == "marker"
            && value.provenance == AttributeProvenance::Explicit
            && matches!(value.value.as_ref(), CoercedAttributeValue::String(_))
    });
    let capability = target
        .rule_capability()
        .expect("Starlark rule has a capability");
    let empty_tags = rule.values().iter().any(|value| {
        value.declaration_name == "tags"
            && value.provenance == AttributeProvenance::Default
            && matches!(value.value.as_ref(), CoercedAttributeValue::StringList(tags) if tags.is_empty())
    });
    let user_schema = rule
        .schema()
        .iter()
        .filter(|schema| !schema.is_builtin())
        .collect::<Vec<_>>();
    let builtin_defaults = rule.values().iter().all(|value| {
        let Some(schema) = rule
            .schema()
            .iter()
            .find(|schema| schema.declaration_name() == value.declaration_name)
        else {
            return false;
        };
        if !schema.is_builtin() {
            return true;
        }
        match schema.declaration_name() {
            "name" => {
                value.provenance == AttributeProvenance::Explicit
                    && matches!(value.value.as_ref(), CoercedAttributeValue::String(_))
            }
            "visibility" => value.provenance == AttributeProvenance::Default,
            "generator_name" | "generator_function" | "generator_location" => {
                value.provenance == AttributeProvenance::Implicit
                    && matches!(value.value.as_ref(), CoercedAttributeValue::String(_))
            }
            "deprecation" => {
                value.provenance == AttributeProvenance::Default
                    && matches!(value.value.as_ref(), CoercedAttributeValue::None)
            }
            _ => {
                value.provenance != AttributeProvenance::Explicit
                    && match value.value.as_ref() {
                        CoercedAttributeValue::None => true,
                        CoercedAttributeValue::String(value) => value.is_empty(),
                        CoercedAttributeValue::LabelList(values) => values.is_empty(),
                        CoercedAttributeValue::StringList(values) => values.is_empty(),
                        CoercedAttributeValue::StringDict(values) => values.is_empty(),
                        CoercedAttributeValue::LabelListDict(values) => values.is_empty(),
                        CoercedAttributeValue::Boolean(value) => !value,
                        CoercedAttributeValue::Integer(value) => *value == 0,
                        _ => false,
                    }
            }
        }
    });
    marker
        && rule.dependencies().is_empty()
        && rule.required_toolchains().is_empty()
        && !rule.is_root_string_build_setting()
        && user_schema.len() == 1
        && user_schema[0].declaration_name() == "marker"
        && matches!(user_schema[0].kind(), AttributeKind::String)
        && user_schema[0].transition().is_none()
        && !user_schema[0].dependency_reachable()
        && rule.values().len() == rule.schema().len()
        && builtin_defaults
        && empty_tags
        && !capability.executable
        && capability.test_kind.is_none()
}

fn validate_marker_toolchain(
    packages: &ConfiguredPackages,
    label: &CanonicalLabel,
) -> Result<(), AnalysisError> {
    let target = package_target(packages, label)?;
    let PackageTargetKind::NativeToolchain(NativeToolchainTarget::Toolchain {
        toolchain_type,
        implementation,
        exec_compatible_with,
        target_compatible_with,
        use_target_platform_constraints,
        target_settings: _,
    }) = &target.kind
    else {
        return Err(AnalysisError::new(format!(
            "registered toolchain is not toolchain: {label}"
        )));
    };
    if !target_compatible_with.value().is_empty() || *use_target_platform_constraints.value() {
        return Err(AnalysisError::new(format!(
            "registered toolchain uses unsupported target compatibility: {label}"
        )));
    }
    if !matches!(package_target(packages, toolchain_type), Ok(target) if matches!(target.kind, PackageTargetKind::NativeToolchain(NativeToolchainTarget::ToolchainType)))
    {
        return Err(AnalysisError::new(format!(
            "toolchain references a non-toolchain type: {label}"
        )));
    }
    validate_constraint_settings(packages, exec_compatible_with.value(), || {
        AnalysisError::new(format!(
            "toolchain has duplicate execution constraint setting: {label}"
        ))
    })?;
    let implementation = package_target(packages, implementation)?;
    let PackageTargetKind::StarlarkRule(_) = &implementation.kind else {
        return Err(AnalysisError::new(format!(
            "toolchain implementation is not a Starlark rule: {label}"
        )));
    };
    if !is_marker_leaf_target(implementation) {
        return Err(AnalysisError::new(format!(
            "toolchain implementation is not a marker leaf: {label}"
        )));
    }
    Ok(())
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

fn select_root_toolchain(
    packages: &ConfiguredPackages,
    required: &CanonicalLabel,
    platforms: &[CanonicalLabel],
    toolchains: &[CanonicalLabel],
    target_settings: &SmallMap<CanonicalLabel, bool>,
) -> Result<(CanonicalLabel, CanonicalLabel, CanonicalLabel), AnalysisError> {
    for platform_label in platforms {
        let platform = package_target(packages, platform_label)?;
        let PackageTargetKind::NativeToolchain(NativeToolchainTarget::Platform {
            constraint_values,
        }) = &platform.kind
        else {
            unreachable!("registered platforms were prevalidated")
        };
        for declaration in toolchains {
            let toolchain = package_target(packages, declaration)?;
            let PackageTargetKind::NativeToolchain(NativeToolchainTarget::Toolchain {
                toolchain_type,
                implementation,
                exec_compatible_with,
                ..
            }) = &toolchain.kind
            else {
                unreachable!("registered toolchains were prevalidated")
            };
            if target_settings.get(declaration) == Some(&true)
                && toolchain_type == required
                && exec_compatible_with
                    .value()
                    .iter()
                    .all(|value| constraint_values.contains(value))
            {
                return Ok((
                    platform_label.clone(),
                    declaration.clone(),
                    implementation.clone(),
                ));
            }
        }
    }
    Err(AnalysisError::new(format!(
        "no compatible toolchain was registered for {required}"
    )))
}

#[allow(clippy::too_many_arguments)]
async fn prepare_selected_toolchain(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: &NormalizedAbsolutePath,
    owner: &ConfiguredTargetKey,
    required: &CanonicalLabel,
    required_type: CompactString,
    candidates: &[ConfiguredTargetKey],
    declaration: CanonicalLabel,
    implementation: CanonicalLabel,
    platform: PreparedExecutionPlatform,
) -> PreparedToolchainOutcome {
    let configuration = owner.configuration().clone();
    let key = ConfiguredNodeAnalysisKey::new(
        workspace.dupe(),
        ConfiguredTargetKey::new(implementation.clone(), configuration.clone()),
    )
    .expect("toolchain analysis inherits a structural configuration");
    let selected = compute_toolchain_analysis_input(
        ctx,
        mode,
        key,
        "analyzing selected toolchain through DICE",
    )
    .await;
    let result = match selected {
        LoadingPreparationOutcome::Need(need) => return LoadingPreparationOutcome::Need(need),
        LoadingPreparationOutcome::Complete(Err(error)) => {
            return LoadingPreparationOutcome::Complete(Err(error));
        }
        LoadingPreparationOutcome::Complete(Ok(value)) => match value.as_ref() {
            Ok(value) => value.clone(),
            Err(error) => return toolchain_outcome(Err(error.clone())),
        },
    };
    let topology_is_exact = if declaration.package() == implementation.package() {
        result.edges().len() == candidates.len()
            && result
                .edges()
                .iter()
                .zip(candidates)
                .enumerate()
                .all(|(index, (edge, platform))| {
                    edge.target() == &ConfiguredNodeKey::configured(platform.clone())
                        && matches!(edge.kind(), crate::configured_target::ConfiguredEdgeKind::CandidateExecutionPlatform { index: edge_index } if *edge_index == u32::try_from(index).expect("candidate index fits u32"))
                })
            && result.toolchain_topology().is_some_and(|topology| {
                topology.candidate_execution_platforms() == candidates
                    && topology.selection().is_none()
            })
    } else {
        result.edges().is_empty() && result.toolchain_topology().is_none()
    };
    if !topology_is_exact
        || !result.diagnostics().is_empty()
        || result.providers().len() != 2
        || result.providers().default_info().is_none()
        || result.providers().toolchain_info().is_none()
    {
        return toolchain_outcome(Err(AnalysisError::new(
            "selected toolchain implementation must return only DefaultInfo and ToolchainInfo with exact topology",
        )));
    }
    let toolchain = Arc::new(ConfiguredActionToolchainContext::new(
        ToolchainSelection::new(
            platform.key,
            declaration,
            ConfiguredTargetKey::new(required.clone(), configuration.clone()),
            ConfiguredTargetKey::new(implementation, configuration),
        ),
        result
            .providers()
            .toolchain_info()
            .expect("checked ToolchainInfo")
            .marker
            .clone(),
    ));
    let context = ConfiguredActionOwnerContext::new(
        owner.clone(),
        ConfiguredActionExecGroup::Default,
        toolchain.selection().execution_platform().clone(),
        platform.fact,
        &BTreeMap::new(),
        &BTreeMap::new(),
        platform.constraints,
        Some(toolchain),
        ConfiguredActionAspectProvenance::Absent,
    )
    .map(Arc::new)
    .map_err(AnalysisError::new);
    toolchain_outcome(context.map(|action_context| PreparedToolchain {
        required_type,
        action_context,
    }))
}

async fn resolve_root_toolchain(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: &NormalizedAbsolutePath,
    required: &CanonicalLabel,
    owner: &ConfiguredTargetKey,
    candidate_execution_platforms: &[ConfiguredTargetKey],
    registrations: &PreparedRegistrations,
) -> PreparedToolchainOutcome {
    let required_type = match root_apparent_type(required) {
        Ok(required_type) => required_type,
        Err(error) => return toolchain_outcome(Err(error)),
    };
    let toolchain_labels = registrations.toolchains();
    let mut labels = candidate_execution_platforms
        .iter()
        .map(|candidate| candidate.label().clone())
        .collect::<Vec<_>>();
    labels.extend(toolchain_labels.iter().cloned());
    labels.push(required.clone());
    let packages = match load_configured_native_packages(ctx, mode, workspace, &mut labels).await {
        LoadingPreparationOutcome::Need(need) => return LoadingPreparationOutcome::Need(need),
        LoadingPreparationOutcome::Complete(Err(error)) => {
            return LoadingPreparationOutcome::Complete(Err(error));
        }
        LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
            return toolchain_outcome(Err(error));
        }
        LoadingPreparationOutcome::Complete(Ok(Ok(packages))) => packages,
    };
    if !matches!(package_target(&packages, required), Ok(target) if matches!(target.kind, PackageTargetKind::NativeToolchain(NativeToolchainTarget::ToolchainType)))
    {
        return toolchain_outcome(Err(AnalysisError::new(format!(
            "required toolchain type is not toolchain_type: {required}"
        ))));
    }
    let platform_labels = candidate_execution_platforms
        .iter()
        .map(|platform| platform.label().clone())
        .collect::<Vec<_>>();
    for platform_label in &platform_labels {
        let target = match package_target(&packages, platform_label) {
            Ok(target) => target,
            Err(error) => return toolchain_outcome(Err(error)),
        };
        let PackageTargetKind::NativeToolchain(NativeToolchainTarget::Platform {
            constraint_values,
        }) = &target.kind
        else {
            return toolchain_outcome(Err(AnalysisError::new(format!(
                "registered execution platform is not platform: {platform_label}"
            ))));
        };
        if let Err(error) = validate_constraint_settings(&packages, constraint_values, || {
            AnalysisError::new(format!(
                "execution platform has duplicate constraint setting: {platform_label}"
            ))
        }) {
            return toolchain_outcome(Err(error));
        }
    }
    for toolchain_label in toolchain_labels {
        if let Err(error) = validate_marker_toolchain(&packages, toolchain_label) {
            return toolchain_outcome(Err(error));
        }
    }
    let target_settings = match prepare_toolchain_target_settings(
        ctx,
        mode,
        workspace,
        owner.configuration(),
        &packages,
        toolchain_labels,
    )
    .await
    {
        LoadingPreparationOutcome::Need(need) => return LoadingPreparationOutcome::Need(need),
        LoadingPreparationOutcome::Complete(Err(error)) => {
            return LoadingPreparationOutcome::Complete(Err(error));
        }
        LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
            return toolchain_outcome(Err(error));
        }
        LoadingPreparationOutcome::Complete(Ok(Ok(settings))) => settings,
    };
    let (selected_platform, declaration, implementation) = match select_root_toolchain(
        &packages,
        required,
        &platform_labels,
        &toolchain_labels,
        &target_settings,
    ) {
        Ok(selected) => selected,
        Err(error) => return toolchain_outcome(Err(error)),
    };
    let selected_platform_key = candidate_execution_platforms
        .iter()
        .find(|platform| platform.label() == &selected_platform)
        .expect("selected root platform retains its configured candidate identity")
        .clone();
    let platform =
        prepare_execution_platform(ctx, mode, workspace, &packages, selected_platform_key).await;
    let platform = match platform {
        LoadingPreparationOutcome::Need(need) => return LoadingPreparationOutcome::Need(need),
        LoadingPreparationOutcome::Complete(Err(error)) => {
            return LoadingPreparationOutcome::Complete(Err(error));
        }
        LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
            return toolchain_outcome(Err(error));
        }
        LoadingPreparationOutcome::Complete(Ok(Ok(value))) => value,
    };

    prepare_selected_toolchain(
        ctx,
        mode,
        workspace,
        owner,
        required,
        required_type,
        candidate_execution_platforms,
        declaration,
        implementation,
        platform,
    )
    .await
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
        assert!(matches!(
            finish_execution_platform_analysis(semantic.clone()),
            Err(LoadingPreparationOutcome::Complete(Ok(result))) if result.is_err()
        ));

        let demand = slug_workspace_v2::PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new("/outer").unwrap(),
            slug_workspace_v2::PathObservationOperation::Lstat,
        );
        let need = LoadingPreparationNeeds::path(
            slug_workspace_v2::NeedPathObservations::singleton(demand.dupe()),
        );
        assert!(matches!(
            finish_execution_platform_analysis(LoadingPreparationOutcome::Need(need)),
            Err(LoadingPreparationOutcome::Need(_))
        ));
        let outer: RootAnalysisDriverValue =
            LoadingPreparationOutcome::Complete(Err(ObservedPathFrontierError::from(
                slug_workspace_v2::PathObservationEpochError::DuplicateDemand(demand),
            )));
        assert!(ConfiguredNodeAnalysisObservationKey::validity(&outer));
        assert!(ConfiguredNodeAnalysisObservationKey::equality(
            &outer, &outer
        ));
        assert!(matches!(
            finish_execution_platform_analysis(outer),
            Err(LoadingPreparationOutcome::Complete(Err(_)))
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
        if let Err(error) = require_supported_canonical_configured_target(node, target) {
            return root_analysis_driver_complete(Err(error));
        }
        match (&self.node, target.map(|target| &target.kind)) {
            (ConfiguredNodeKey::Null(_), None) if package_declares_source_label(package, label) => {
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
                Some(PackageTargetKind::NativeToolchain(NativeToolchainTarget::ConstraintSetting)),
            ) if configured_target.configuration().kind() == ConfigurationKind::Exec => {
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
            ) if configured_target.configuration().kind() == ConfigurationKind::Exec => {
                let child = compute_configured_child(
                    ctx,
                    mode,
                    self.workspace.dupe(),
                    constraint_setting.clone(),
                    configured_target.configuration().clone(),
                )
                .await;
                let child = match child {
                    LoadingPreparationOutcome::Need(need) => {
                        return LoadingPreparationOutcome::Need(need);
                    }
                    LoadingPreparationOutcome::Complete(Err(error)) => {
                        return LoadingPreparationOutcome::Complete(Err(error));
                    }
                    LoadingPreparationOutcome::Complete(Ok(value)) => match value.as_ref() {
                        Ok(result) => result.dupe(),
                        Err(error) => return root_analysis_driver_complete(Err(error.clone())),
                    },
                };
                if child.kind() != &ConfiguredNodeKind::ConstraintSetting {
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
            ) if configured_target.configuration().kind() == ConfigurationKind::Exec => {
                let fact = match platform_semantic_fact(package, label.target().as_str(), label) {
                    Ok(fact) => fact,
                    Err(error) => return root_analysis_driver_complete(Err(error)),
                };
                let mut seen_settings = SmallSet::with_capacity(constraint_values.len());
                let mut edges = Vec::with_capacity(constraint_values.len());
                for (index, constraint_value) in constraint_values.iter().enumerate() {
                    let child = compute_configured_child(
                        ctx,
                        mode,
                        self.workspace.dupe(),
                        constraint_value.clone(),
                        configured_target.configuration().clone(),
                    )
                    .await;
                    let child = match child {
                        LoadingPreparationOutcome::Need(need) => {
                            return LoadingPreparationOutcome::Need(need);
                        }
                        LoadingPreparationOutcome::Complete(Err(error)) => {
                            return LoadingPreparationOutcome::Complete(Err(error));
                        }
                        LoadingPreparationOutcome::Complete(Ok(value)) => match value.as_ref() {
                            Ok(result) => result.dupe(),
                            Err(error) => return root_analysis_driver_complete(Err(error.clone())),
                        },
                    };
                    if child.kind() != &ConfiguredNodeKind::ConstraintValue {
                        return root_analysis_driver_complete(Err(AnalysisError::new(format!(
                            "platform {label} references a non-constraint value {constraint_value}"
                        ))));
                    }
                    let Some(setting) = child.edges().first().map(|edge| edge.target().clone())
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
            ) if configured_target.configuration().kind() == ConfigurationKind::Target => {
                return root_analysis_driver_complete(Ok(ConfiguredNodeResult::new_native(
                    self.node.clone(),
                    ConfiguredNodeKind::ToolchainType,
                    native_empty_providers(),
                    target.and_then(|target| target.rule_capability()).cloned(),
                )));
            }
            (
                ConfiguredNodeKey::Configured(_),
                Some(PackageTargetKind::NativeToolchain(NativeToolchainTarget::Toolchain {
                    ..
                })),
            ) => {
                return root_analysis_driver_complete(Err(AnalysisError::new(format!(
                    "toolchain declaration nodes are not supported: {label}"
                ))));
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
                let child = compute_configured_child(
                    ctx,
                    mode,
                    self.workspace.dupe(),
                    actual.clone(),
                    configured_target.configuration().clone(),
                )
                .await;
                let child = match child {
                    LoadingPreparationOutcome::Need(need) => {
                        return LoadingPreparationOutcome::Need(need);
                    }
                    LoadingPreparationOutcome::Complete(Err(error)) => {
                        return LoadingPreparationOutcome::Complete(Err(error));
                    }
                    LoadingPreparationOutcome::Complete(Ok(value)) => match value.as_ref() {
                        Ok(result) => result.dupe(),
                        Err(error) => return root_analysis_driver_complete(Err(error.clone())),
                    },
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
                )])));
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
                let child = compute_configured_child(
                    ctx,
                    mode,
                    self.workspace.dupe(),
                    producer,
                    configured_target.configuration().clone(),
                )
                .await;
                let child = match child {
                    LoadingPreparationOutcome::Need(need) => {
                        return LoadingPreparationOutcome::Need(need);
                    }
                    LoadingPreparationOutcome::Complete(Err(error)) => {
                        return LoadingPreparationOutcome::Complete(Err(error));
                    }
                    LoadingPreparationOutcome::Complete(Ok(value)) => match value.as_ref() {
                        Ok(result) => result.dupe(),
                        Err(error) => return root_analysis_driver_complete(Err(error.clone())),
                    },
                };
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
        let requirement = {
            let rule = match starlark_rule_implementation(package, configured_target) {
                Ok(rule) => rule,
                Err(error) => return root_analysis_driver_complete(Err(error)),
            };
            if rule.required_toolchains().len() > 1 {
                return root_analysis_driver_complete(Err(AnalysisError::new(
                    "toolchain resolution supports exactly zero or one required type",
                )));
            }
            rule.required_toolchains()
                .first()
                .map(|requirement| requirement.label().clone())
        };
        let local_declarations = local_toolchain_declarations(package, configured_target.label());
        let registrations = match prepare_registrations(
            ctx,
            mode,
            &self.workspace,
            configured_target.configuration(),
            requirement.is_some(),
            !local_declarations.is_empty(),
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
            LoadingPreparationOutcome::Complete(Ok(Ok(registrations))) => registrations,
        };
        let candidate_execution_platforms = match rule_execution_platforms(
            configured_target.configuration(),
            registrations.as_ref(),
            &local_declarations,
            requirement.is_some(),
        ) {
            Ok(platforms) => platforms,
            Err(error) => return root_analysis_driver_complete(Err(error)),
        };
        let prepared_toolchain = if let Some(requirement) = requirement {
            let candidates = candidate_execution_platforms
                .as_ref()
                .expect("a toolchain requirement prepares candidate execution platforms");
            let registrations = registrations
                .as_ref()
                .expect("a toolchain requirement prepares module registrations");
            match resolve_root_toolchain(
                ctx,
                mode,
                &self.workspace,
                &requirement,
                configured_target,
                candidates,
                registrations,
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
                LoadingPreparationOutcome::Complete(Ok(Ok(value))) => Some(value),
            }
        } else {
            None
        };
        let action_context = if let Some(toolchain) = &prepared_toolchain {
            toolchain.action_context.clone()
        } else {
            match prepare_default_action_context(
                ctx,
                mode,
                &self.workspace,
                configured_target,
                candidate_execution_platforms.as_deref(),
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
                LoadingPreparationOutcome::Complete(Ok(Ok(context))) => context,
            }
        };
        let declared_dependency_keys = match root_declared_dependency_keys(
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

        let mut unique = SmallSet::with_capacity(declared_dependency_keys.len());
        for dependency in &declared_dependency_keys {
            unique.insert(dependency.node.clone());
        }
        let workspace = &self.workspace;
        let preparations = ctx
            .compute_join(unique.into_iter(), |ctx, node| {
                Box::pin(async move {
                    let prepared = match &node {
                        ConfiguredNodeKey::Configured(configured_target) => {
                            prepare_configured_node_analysis_driver(
                                ctx,
                                mode,
                                workspace.dupe(),
                                configured_target.label().clone(),
                                configured_target.configuration().clone(),
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
        let mut prepared = Vec::with_capacity(preparations.len());
        for (node, outcome) in preparations {
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
                    Ok(key) => prepared.push((node, key)),
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
            return root_analysis_driver_complete(Err(error));
        }

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
                            Err(error) => root_analysis_driver_complete(Err(AnalysisError::new(
                                format!("computing dependency `{node}` through DICE: {error}"),
                            ))),
                        },
                        ConfiguredAnalysisMode::Observed => match ctx
                            .compute(&ConfiguredNodeAnalysisObservationKey(key))
                            .await
                        {
                            Ok(value) => value,
                            Err(error) => root_analysis_driver_complete(Err(AnalysisError::new(
                                format!("computing dependency `{node}` through DICE: {error}"),
                            ))),
                        },
                    };
                    (node, result)
                })
            })
            .await;
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
            &computed,
            candidate_execution_platforms,
            action_context,
            prepared_toolchain,
            capture_events,
            event_batch,
        ))
    }
}
