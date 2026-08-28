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

use crate::key::ConfigurationKey;
use crate::key::ConfigurationKind;
use crate::key::ConfiguredNodeKey;
use crate::key::ConfiguredTargetKey;
use crate::key::RootStringSettingValue;
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
    base_configuration: ConfigurationKey,
    explicit: Option<RootStringSettingValue>,
) -> LoadingPreparationOutcome<Result<ConfiguredNodeAnalysisKey, AnalysisError>> {
    match prepare_configured_node_analysis_driver(
        ctx,
        ConfiguredAnalysisMode::Legacy,
        workspace,
        requested,
        base_configuration,
        explicit,
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
    base_configuration: ConfigurationKey,
    explicit: Option<RootStringSettingValue>,
) -> ObservedConfiguredNodeAnalysisPreparationOutcome {
    prepare_configured_node_analysis_driver(
        ctx,
        ConfiguredAnalysisMode::Observed,
        workspace,
        requested,
        base_configuration,
        explicit,
    )
    .await
    .map(|result| result.map(|result| result.map(ConfiguredNodeAnalysisObservationKey)))
}

async fn prepare_configured_node_analysis_driver(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: NormalizedAbsolutePath,
    requested: CanonicalLabel,
    base_configuration: ConfigurationKey,
    explicit: Option<RootStringSettingValue>,
) -> AnalysisSemanticOutcome<ConfiguredNodeAnalysisKey> {
    if base_configuration.slug_configuration().is_none() {
        return analysis_semantic_complete(Err(AnalysisError::message(
            "production configured-node analysis requires a structural Slug configuration",
        )));
    }
    let explicit_validation = match explicit.as_ref() {
        Some(explicit) => Some(match CanonicalLabel::parse(explicit.label()) {
            Ok(setting) => root_string_build_setting_default(ctx, mode, &workspace, &setting).await,
            Err(error) => analysis_semantic_complete(Err(AnalysisError::message(error))),
        }),
        None => None,
    };
    let package_outcome = compute_configured_package_input(
        ctx,
        mode,
        workspace.dupe(),
        requested.package().clone(),
        "loading root setting target package through DICE",
    )
    .await;
    let mut all_need: Option<LoadingPreparationNeeds> = None;
    let mut first_outer = None;
    let mut first_error = None;
    if let Some(validation) = explicit_validation {
        match validation {
            LoadingPreparationOutcome::Need(need) => all_need = Some(need),
            LoadingPreparationOutcome::Complete(Err(error)) => first_outer = Some(error),
            LoadingPreparationOutcome::Complete(Ok(Err(error))) => first_error = Some(error),
            LoadingPreparationOutcome::Complete(Ok(Ok(_))) => {}
        }
    }
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
    let Some(target) = package
        .targets
        .iter()
        .find(|target| target.name == requested.target().as_str())
    else {
        return analysis_semantic_complete(Err(AnalysisError::target_not_found(
            requested,
            package.build_file.clone(),
        )));
    };
    let PackageTargetKind::StarlarkRule(rule) = &target.kind else {
        return analysis_semantic_complete(ConfiguredNodeAnalysisKey::new(
            workspace,
            ConfiguredTargetKey::new(requested, base_configuration),
        ));
    };
    let required = match required_root_string_setting(rule, &requested) {
        Ok(required) => required,
        Err(error) => return analysis_semantic_complete(Err(error)),
    };
    if explicit.is_none()
        && let Err(error) =
            validate_carried_root_string_setting(&base_configuration, required.as_ref())
    {
        return analysis_semantic_complete(Err(error));
    }
    let configuration = match (required, explicit) {
        (Some(setting), Some(explicit)) if explicit.label() != setting.to_string() => {
            return analysis_semantic_complete(Err(AnalysisError::message(format!(
                "root string setting request for {setting} carried {}",
                explicit.label()
            ))));
        }
        (Some(_), Some(explicit)) => base_configuration.with_root_string_setting(explicit),
        (None, Some(explicit)) => base_configuration.with_root_string_setting(explicit),
        (None, None) => base_configuration,
        (Some(setting), None)
            if base_configuration
                .root_string_setting()
                .is_some_and(|carried| carried.label() == setting.to_string()) =>
        {
            match root_string_build_setting_default(ctx, mode, &workspace, &setting).await {
                LoadingPreparationOutcome::Need(need) => {
                    return LoadingPreparationOutcome::Need(need);
                }
                LoadingPreparationOutcome::Complete(Err(error)) => {
                    return LoadingPreparationOutcome::Complete(Err(error));
                }
                LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
                    return analysis_semantic_complete(Err(error));
                }
                LoadingPreparationOutcome::Complete(Ok(Ok(_))) => base_configuration,
            }
        }
        (Some(setting), None) => {
            let default =
                match root_string_build_setting_default(ctx, mode, &workspace, &setting).await {
                    LoadingPreparationOutcome::Need(need) => {
                        return LoadingPreparationOutcome::Need(need);
                    }
                    LoadingPreparationOutcome::Complete(Err(error)) => {
                        return LoadingPreparationOutcome::Complete(Err(error));
                    }
                    LoadingPreparationOutcome::Complete(Ok(Err(error))) => {
                        return analysis_semantic_complete(Err(error));
                    }
                    LoadingPreparationOutcome::Complete(Ok(Ok(default))) => default,
                };
            base_configuration.with_root_string_setting(RootStringSettingValue::new_for_label(
                setting.to_string(),
                default,
            ))
        }
    };
    analysis_semantic_complete(ConfiguredNodeAnalysisKey::new(
        workspace,
        ConfiguredTargetKey::new(requested, configuration),
    ))
}

async fn root_string_build_setting_default(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: &NormalizedAbsolutePath,
    setting: &CanonicalLabel,
) -> AnalysisSemanticOutcome<CompactString> {
    let package_inventory = match compute_configured_package_input(
        ctx,
        mode,
        workspace.dupe(),
        setting.package().clone(),
        "loading root string setting through DICE",
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
    let Some(default) = package
        .targets
        .iter()
        .find(|target| target.name == setting.target().as_str())
        .and_then(|target| match &target.kind {
            PackageTargetKind::StarlarkRule(rule) if rule.is_root_string_build_setting() => {
                rule.root_string_build_setting_default()
            }
            _ => None,
        })
    else {
        return analysis_semantic_complete(Err(AnalysisError::message(format!(
            "root string build setting {setting} is missing"
        ))));
    };
    analysis_semantic_complete(Ok(default.into()))
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

fn required_root_string_setting(
    implementation: &StarlarkRuleImplementation,
    target: &CanonicalLabel,
) -> Result<Option<CanonicalLabel>, AnalysisError> {
    let mut required = implementation
        .is_root_string_build_setting()
        .then(|| target.clone());
    let mut insert = |candidate: CanonicalLabel| -> Result<(), AnalysisError> {
        if let Some(existing) = &required
            && existing != &candidate
        {
            return Err(AnalysisError::new(format!(
                "multiple string build settings are not supported: {existing} and {candidate}"
            )));
        }
        required = Some(candidate);
        Ok(())
    };
    for schema in implementation.schema() {
        if let Some(transition) = schema.transition() {
            insert(
                CanonicalLabel::parse(&format!("@@{}", transition.output()))
                    .map_err(AnalysisError::new)?,
            )?;
        }
    }
    let fixed = CanonicalLabel::parse("@@//:setting").expect("fixed setting label is valid");
    if implementation
        .dependencies()
        .iter()
        .any(|dependency| dependency == &fixed)
    {
        insert(fixed)?;
    }
    Ok(required)
}

fn validate_carried_root_string_setting(
    configuration: &ConfigurationKey,
    required: Option<&CanonicalLabel>,
) -> Result<(), AnalysisError> {
    let (Some(carried), Some(required)) = (configuration.root_string_setting(), required) else {
        return Ok(());
    };
    if carried.label() != required.to_string() {
        return Err(AnalysisError::new(format!(
            "multiple string build settings are not supported: {} and {required}",
            carried.label()
        )));
    }
    Ok(())
}

fn root_declared_dependency_keys(
    package: &LoadedPackage,
    configured_target: &ConfiguredTargetKey,
) -> Result<Vec<DeclaredDependencyKey>, AnalysisError> {
    let implementation = starlark_rule_implementation(package, configured_target)?;
    let mut dependencies = Vec::new();
    for value in implementation.values() {
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
        // Retain the Bazel tools allowlist in loading/query topology, but the
        // current Rust-native analysis subset has no permission-check action
        // and cannot configure external repositories yet.
        if schema.declaration_name() == "$allowlist_function_transition" {
            continue;
        }
        let labels: Vec<&CanonicalLabel> = match value.value.as_ref() {
            CoercedAttributeValue::Label(label) => vec![label],
            CoercedAttributeValue::LabelList(labels) => labels.iter().collect(),
            _ => continue,
        };
        let configuration = if let Some(transition) = schema.transition() {
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
            let setting = setting.unpack_str().ok_or_else(|| {
                AnalysisError::new(format!(
                    "transition {} output must be a string",
                    transition.output()
                ))
            })?;
            let output_label = CanonicalLabel::parse(&format!("@@{}", transition.output()))
                .map_err(AnalysisError::new)?;
            configured_target.configuration().with_root_string_setting(
                RootStringSettingValue::new_for_label(output_label.to_string(), setting),
            )
        } else {
            configured_target.configuration().clone()
        };
        let transition_output = schema
            .transition()
            .map(|transition| CanonicalLabel::parse(&format!("@@{}", transition.output())))
            .transpose()
            .map_err(AnalysisError::new)?;
        for (attribute_index, label) in labels.into_iter().enumerate() {
            let node = if label.package() == configured_target.label().package()
                && package
                    .targets
                    .iter()
                    .find(|target| target.name == label.target().as_str())
                    .is_none()
            {
                ConfiguredNodeKey::null(label.clone())
            } else {
                ConfiguredNodeKey::configured(ConfiguredTargetKey::new(
                    label.clone(),
                    configuration.clone(),
                ))
            };
            dependencies.push(DeclaredDependencyKey {
                attribute: CompactString::from(value.declaration_name.as_str()),
                attribute_index: u32::try_from(attribute_index)
                    .expect("attribute dependency index fits u32"),
                sequence: schema.transition().is_some()
                    || matches!(schema.kind(), slug_loading_v2::AttributeKind::LabelList),
                node,
                transition_output: transition_output.clone(),
            });
        }
    }
    Ok(dependencies)
}
#[derive(Debug, Clone)]
struct DeclaredDependencyKey {
    attribute: CompactString,
    attribute_index: u32,
    sequence: bool,
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
    declared_dependency_keys: &[DeclaredDependencyKey],
    computed: &SmallMap<ConfiguredNodeKey, T>,
    marker: Option<CompactString>,
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
            sequence: dependency.sequence,
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
        marker,
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
    let key = match prepare_configured_node_analysis_driver(
        ctx,
        mode,
        workspace,
        label,
        configuration,
        None,
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
struct PreparedModuleRegistrations {
    execution_platforms: Arc<ModuleRegistrationExpansion>,
    toolchains: Arc<ModuleRegistrationExpansion>,
}

impl PreparedModuleRegistrations {
    fn execution_platforms(&self) -> Result<&[CanonicalLabel], AnalysisError> {
        self.execution_platforms
            .labels()
            .map(|labels| labels.as_ref())
            .map_err(|error| AnalysisError::new(error.to_string()))
    }

    fn toolchains(&self) -> Result<&[CanonicalLabel], AnalysisError> {
        self.toolchains
            .labels()
            .map(|labels| labels.as_ref())
            .map_err(|error| AnalysisError::new(error.to_string()))
    }
}

fn toolchain_outcome(result: Result<PreparedToolchain, AnalysisError>) -> PreparedToolchainOutcome {
    analysis_semantic_complete(result)
}

async fn compute_registration_expansion_input(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: NormalizedAbsolutePath,
    execution_platforms: bool,
    context: &str,
) -> AnalysisSemanticOutcome<Arc<ModuleRegistrationExpansion>> {
    match mode {
        ConfiguredAnalysisMode::Legacy => {
            let key = if execution_platforms {
                ModuleRegistrationExpansionKey::execution_platforms(workspace)
            } else {
                ModuleRegistrationExpansionKey::toolchains(workspace)
            };
            match ctx.compute(&key).await {
                Ok(LoadingPreparationOutcome::Need(need)) => LoadingPreparationOutcome::Need(need),
                Ok(LoadingPreparationOutcome::Complete(value)) => {
                    analysis_semantic_complete(Ok(value))
                }
                Err(error) => analysis_semantic_complete(Err(AnalysisError::new(format!(
                    "{context}: {error}"
                )))),
            }
        }
        ConfiguredAnalysisMode::Observed => {
            let key = if execution_platforms {
                ModuleRegistrationExpansionObservationKey::execution_platforms(workspace)
            } else {
                ModuleRegistrationExpansionObservationKey::toolchains(workspace)
            };
            match ctx.compute(&key).await {
                Ok(LoadingPreparationOutcome::Need(need)) => LoadingPreparationOutcome::Need(need),
                Ok(LoadingPreparationOutcome::Complete(Err(
                    ModuleRegistrationExpansionObservationError::Frontier(error),
                ))) => LoadingPreparationOutcome::Complete(Err(error)),
                Ok(LoadingPreparationOutcome::Complete(Err(error))) => {
                    analysis_semantic_complete(Err(AnalysisError::new(error.to_string())))
                }
                Ok(LoadingPreparationOutcome::Complete(Ok(observed))) => {
                    analysis_semantic_complete(Ok(observed.result().dupe()))
                }
                Err(error) => analysis_semantic_complete(Err(AnalysisError::new(format!(
                    "{context}: {error}"
                )))),
            }
        }
    }
}

async fn prepare_module_registrations(
    ctx: &mut DiceComputations<'_>,
    mode: ConfiguredAnalysisMode,
    workspace: &NormalizedAbsolutePath,
    has_toolchain_requirement: bool,
    has_local_declarations: bool,
) -> AnalysisSemanticOutcome<Option<PreparedModuleRegistrations>> {
    if !has_toolchain_requirement && !has_local_declarations {
        return analysis_semantic_complete(Ok(None));
    }
    let execution_platforms = compute_registration_expansion_input(
        ctx,
        mode,
        workspace.dupe(),
        true,
        "loading execution-platform registrations through DICE",
    )
    .await;
    let toolchains = compute_registration_expansion_input(
        ctx,
        mode,
        workspace.dupe(),
        false,
        "loading toolchain registrations through DICE",
    )
    .await;
    let mut values = [None, None];
    let mut needs: Option<LoadingPreparationNeeds> = None;
    let mut first_outer = None;
    let mut first_error = None;
    for (index, outcome) in [execution_platforms, toolchains].into_iter().enumerate() {
        match outcome {
            LoadingPreparationOutcome::Need(need) => {
                needs = Some(needs.map_or(need.clone(), |current| {
                    current
                        .try_union(&need)
                        .expect("registration family Needs agree")
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
            LoadingPreparationOutcome::Complete(Ok(Ok(value))) => match value.labels() {
                Ok(_) => values[index] = Some(value),
                Err(error) if first_error.is_none() => {
                    first_error = Some(AnalysisError::new(error.to_string()));
                }
                Err(_) => {}
            },
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
    let [Some(execution_platforms), Some(toolchains)] = values else {
        unreachable!("complete registration preparation retains both family values")
    };
    analysis_semantic_complete(Ok(Some(PreparedModuleRegistrations {
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
    registrations: Option<&PreparedModuleRegistrations>,
    local_declarations: &SmallSet<CanonicalLabel>,
    has_toolchain_requirement: bool,
) -> Result<Option<Vec<ConfiguredTargetKey>>, AnalysisError> {
    let Some(registrations) = registrations else {
        debug_assert!(!has_toolchain_requirement && local_declarations.is_empty());
        return Ok(None);
    };
    let toolchains = registrations.toolchains()?;
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
        .execution_platforms()?
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
        PackageTargetKind::NativeToolchain(NativeToolchainTarget::ConstraintValue {
            constraint_setting,
        }) => vec![constraint_setting.clone()],
        PackageTargetKind::NativeToolchain(NativeToolchainTarget::Platform {
            constraint_values,
        }) => constraint_values.to_vec(),
        PackageTargetKind::NativeToolchain(NativeToolchainTarget::Toolchain {
            toolchain_type,
            implementation,
            exec_compatible_with,
        }) => std::iter::once(toolchain_type)
            .chain(std::iter::once(implementation))
            .chain(exec_compatible_with.iter())
            .cloned()
            .collect(),
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
    }) = &target.kind
    else {
        return Err(AnalysisError::new(format!(
            "registered toolchain is not toolchain: {label}"
        )));
    };
    if !matches!(package_target(packages, toolchain_type), Ok(target) if matches!(target.kind, PackageTargetKind::NativeToolchain(NativeToolchainTarget::ToolchainType)))
    {
        return Err(AnalysisError::new(format!(
            "toolchain references a non-toolchain type: {label}"
        )));
    }
    validate_constraint_settings(packages, exec_compatible_with, || {
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

fn select_root_toolchain(
    packages: &ConfiguredPackages,
    required: &CanonicalLabel,
    platforms: &[CanonicalLabel],
    toolchains: &[CanonicalLabel],
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
            }) = &toolchain.kind
            else {
                unreachable!("registered toolchains were prevalidated")
            };
            if toolchain_type == required
                && exec_compatible_with
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
    registrations: &PreparedModuleRegistrations,
) -> PreparedToolchainOutcome {
    let required_type = match root_apparent_type(required) {
        Ok(required_type) => required_type,
        Err(error) => return toolchain_outcome(Err(error)),
    };
    let toolchain_labels = match registrations.toolchains() {
        Ok(labels) => labels,
        Err(error) => return toolchain_outcome(Err(error)),
    };
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
    let (selected_platform, declaration, implementation) =
        match select_root_toolchain(&packages, required, &platform_labels, &toolchain_labels) {
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
        let required_root_string_setting = {
            let rule = match starlark_rule_implementation(package, configured_target) {
                Ok(rule) => rule,
                Err(error) => return root_analysis_driver_complete(Err(error)),
            };
            match required_root_string_setting(rule, label) {
                Ok(required) => required,
                Err(error) => return root_analysis_driver_complete(Err(error)),
            }
        };
        if let Err(error) = validate_carried_root_string_setting(
            configured_target.configuration(),
            required_root_string_setting.as_ref(),
        ) {
            return root_analysis_driver_complete(Err(error));
        }
        if configured_target
            .configuration()
            .root_string_setting()
            .is_none()
            && let Some(setting) = &required_root_string_setting
        {
            return root_analysis_driver_complete(Err(AnalysisError::new(format!(
                "configured node was constructed before resolving root string setting {setting}"
            ))));
        }
        let (requirement, marker) = {
            let rule = match starlark_rule_implementation(package, configured_target) {
                Ok(rule) => rule,
                Err(error) => return root_analysis_driver_complete(Err(error)),
            };
            if rule.required_toolchains().len() > 1 {
                return root_analysis_driver_complete(Err(AnalysisError::new(
                    "toolchain resolution supports exactly zero or one required type",
                )));
            }
            let marker = rule
                .values()
                .iter()
                .find(|value| value.declaration_name == "marker")
                .and_then(|value| match value.value.as_ref() {
                    CoercedAttributeValue::String(value) => Some(value.clone()),
                    _ => None,
                });
            (
                rule.required_toolchains()
                    .first()
                    .map(|requirement| requirement.label().clone()),
                marker,
            )
        };
        let local_declarations = local_toolchain_declarations(package, configured_target.label());
        let registrations = match prepare_module_registrations(
            ctx,
            mode,
            &self.workspace,
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
        let declared_dependency_keys = {
            match root_declared_dependency_keys(package, configured_target) {
                Ok(keys) => keys,
                Err(error) => return root_analysis_driver_complete(Err(error)),
            }
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
                                None,
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
            &declared_dependency_keys,
            &computed,
            marker,
            candidate_execution_platforms,
            action_context,
            prepared_toolchain,
            capture_events,
            event_batch,
        ))
    }
}
