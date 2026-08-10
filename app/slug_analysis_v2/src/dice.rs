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
use slug_bzlmod_v2::RootModuleLoadingAnchorKey;
use slug_events_v2::CaptureEvaluationEvents;
use slug_events_v2::EvaluationEvent;
use slug_events_v2::EventBatch;
use slug_events_v2::StarlarkSourceLocation;
use slug_identity_v2::CanonicalLabel;
use slug_loading_v2::AttributeKind;
use slug_loading_v2::AttributeProvenance;
use slug_loading_v2::CoercedAttributeValue;
use slug_loading_v2::LoadedPackage;
use slug_loading_v2::LoadingPreparationNeeds;
use slug_loading_v2::LoadingPreparationOutcome;
use slug_loading_v2::PackageTargetKind;
use slug_loading_v2::RootPackageLoadKey;
use slug_loading_v2::package::NativeToolchainTarget;
use slug_loading_v2::package::StarlarkRuleImplementation;
use slug_workspace_v2::NormalizedAbsolutePath;
use starlark::PrintHandler;
use starlark::PrintLocation;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::values::Value;
use starlark::values::dict::DictRef;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::key::ConfigurationKey;
use crate::key::ConfiguredTargetKey;
use crate::key::RootStringSettingValue;
use crate::result::ConfiguredNodeResult;
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
    configured_target: ConfiguredTargetKey,
}

impl ConfiguredNodeAnalysisKey {
    pub fn new(
        workspace: NormalizedAbsolutePath,
        configured_target: ConfiguredTargetKey,
    ) -> Result<Self, AnalysisError> {
        if configured_target
            .configuration()
            .slug_configuration()
            .is_none()
        {
            return Err(AnalysisError::message(
                "production configured-node analysis requires a structural Slug configuration",
            ));
        }
        Ok(Self {
            workspace,
            configured_target,
        })
    }

    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        &self.workspace
    }

    pub fn configured_target(&self) -> &ConfiguredTargetKey {
        &self.configured_target
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
    if base_configuration.slug_configuration().is_none() {
        return LoadingPreparationOutcome::Complete(Err(AnalysisError::message(
            "production configured-node analysis requires a structural Slug configuration",
        )));
    }
    let explicit_validation = match explicit.as_ref() {
        Some(explicit) => Some(match CanonicalLabel::parse(explicit.label()) {
            Ok(setting) => root_string_build_setting_default(ctx, &workspace, &setting).await,
            Err(error) => LoadingPreparationOutcome::Complete(Err(AnalysisError::message(error))),
        }),
        None => None,
    };
    let package_outcome = match ctx
        .compute(&RootPackageLoadKey::new(
            workspace.dupe(),
            requested.package().package().clone(),
        ))
        .await
    {
        Ok(LoadingPreparationOutcome::Need(need)) => LoadingPreparationOutcome::Need(need),
        Ok(LoadingPreparationOutcome::Complete(value)) => match value.as_ref() {
            Ok(package) => LoadingPreparationOutcome::Complete(Ok(package.clone())),
            Err(error) => {
                LoadingPreparationOutcome::Complete(Err(AnalysisError::message(error.to_string())))
            }
        },
        Err(error) => LoadingPreparationOutcome::Complete(Err(AnalysisError::message(format!(
            "loading root setting target package through DICE: {error}"
        )))),
    };
    let mut all_need: Option<LoadingPreparationNeeds> = None;
    let mut first_error = None;
    if let Some(validation) = explicit_validation {
        match validation {
            LoadingPreparationOutcome::Need(need) => all_need = Some(need),
            LoadingPreparationOutcome::Complete(Err(error)) => first_error = Some(error),
            LoadingPreparationOutcome::Complete(Ok(_)) => {}
        }
    }
    let mut package = None;
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
            if first_error.is_none() {
                first_error = Some(error);
            }
        }
        LoadingPreparationOutcome::Complete(Ok(value)) => package = Some(value),
    }
    if let Some(need) = all_need {
        return LoadingPreparationOutcome::Need(need);
    }
    if let Some(error) = first_error {
        return LoadingPreparationOutcome::Complete(Err(error));
    }
    let package = package.expect("complete target package preparation stores its value");
    let Some(target) = package
        .targets
        .iter()
        .find(|target| target.name == requested.target().as_str())
    else {
        return LoadingPreparationOutcome::Complete(Err(AnalysisError::target_not_found(
            requested,
            package.build_file.clone(),
        )));
    };
    let PackageTargetKind::StarlarkRule(rule) = &target.kind else {
        return LoadingPreparationOutcome::Complete(ConfiguredNodeAnalysisKey::new(
            workspace,
            ConfiguredTargetKey::new(requested, base_configuration),
        ));
    };
    let required = match required_root_string_setting(rule, &requested) {
        Ok(required) => required,
        Err(error) => return LoadingPreparationOutcome::Complete(Err(error)),
    };
    if explicit.is_none()
        && let Err(error) =
            validate_carried_root_string_setting(&base_configuration, required.as_ref())
    {
        return LoadingPreparationOutcome::Complete(Err(error));
    }
    let configuration = match (required, explicit) {
        (Some(setting), Some(explicit)) if explicit.label() != setting.to_string() => {
            return LoadingPreparationOutcome::Complete(Err(AnalysisError::message(format!(
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
            match root_string_build_setting_default(ctx, &workspace, &setting).await {
                LoadingPreparationOutcome::Need(need) => {
                    return LoadingPreparationOutcome::Need(need);
                }
                LoadingPreparationOutcome::Complete(Err(error)) => {
                    return LoadingPreparationOutcome::Complete(Err(error));
                }
                LoadingPreparationOutcome::Complete(Ok(_)) => base_configuration,
            }
        }
        (Some(setting), None) => {
            let default = match root_string_build_setting_default(ctx, &workspace, &setting).await {
                LoadingPreparationOutcome::Need(need) => {
                    return LoadingPreparationOutcome::Need(need);
                }
                LoadingPreparationOutcome::Complete(Err(error)) => {
                    return LoadingPreparationOutcome::Complete(Err(error));
                }
                LoadingPreparationOutcome::Complete(Ok(default)) => default,
            };
            base_configuration.with_root_string_setting(RootStringSettingValue::new_for_label(
                setting.to_string(),
                default,
            ))
        }
    };
    LoadingPreparationOutcome::Complete(ConfiguredNodeAnalysisKey::new(
        workspace,
        ConfiguredTargetKey::new(requested, configuration),
    ))
}

async fn root_string_build_setting_default(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    setting: &CanonicalLabel,
) -> LoadingPreparationOutcome<Result<CompactString, AnalysisError>> {
    let package = match ctx
        .compute(&RootPackageLoadKey::new(
            workspace.dupe(),
            setting.package().package().clone(),
        ))
        .await
    {
        Ok(LoadingPreparationOutcome::Need(need)) => return LoadingPreparationOutcome::Need(need),
        Ok(LoadingPreparationOutcome::Complete(value)) => match value.as_ref() {
            Ok(package) => package.clone(),
            Err(error) => {
                return LoadingPreparationOutcome::Complete(Err(AnalysisError::message(
                    error.to_string(),
                )));
            }
        },
        Err(error) => {
            return LoadingPreparationOutcome::Complete(Err(AnalysisError::message(format!(
                "loading root string setting through DICE: {error}"
            ))));
        }
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
        return LoadingPreparationOutcome::Complete(Err(AnalysisError::message(format!(
            "root string build setting {setting} is missing"
        ))));
    };
    LoadingPreparationOutcome::Complete(Ok(default.into()))
}

impl fmt::Display for ConfiguredNodeAnalysisKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "configured-node-analysis:{}", self.configured_target)
    }
}

type RootAnalysisKeyValue =
    LoadingPreparationOutcome<Arc<Result<Arc<ConfiguredNodeResult>, AnalysisError>>>;

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
            dependencies.push(DeclaredDependencyKey {
                attribute: CompactString::from(value.declaration_name.as_str()),
                attribute_index: u32::try_from(attribute_index)
                    .expect("attribute dependency index fits u32"),
                sequence: schema.transition().is_some()
                    || matches!(schema.kind(), slug_loading_v2::AttributeKind::LabelList),
                key: ConfiguredTargetKey::new(label.clone(), configuration.clone()),
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
    key: ConfiguredTargetKey,
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

fn legacy_declared_dependency_keys(
    package: &LoadedPackage,
    configured_target: &ConfiguredTargetKey,
) -> Result<Vec<DeclaredDependencyKey>, AnalysisError> {
    Ok(starlark_rule_implementation(package, configured_target)?
        .dependencies()
        .iter()
        .cloned()
        .enumerate()
        .map(|(attribute_index, label)| DeclaredDependencyKey {
            attribute: CompactString::const_new("deps"),
            attribute_index: u32::try_from(attribute_index)
                .expect("attribute dependency index fits u32"),
            sequence: true,
            key: ConfiguredTargetKey::new(label, configured_target.configuration().clone()),
            transition_output: None,
        })
        .collect())
}

fn finish_analysis<T>(
    package: &LoadedPackage,
    configured_target: &ConfiguredTargetKey,
    declared_dependency_keys: &[DeclaredDependencyKey],
    computed: &SmallMap<ConfiguredTargetKey, T>,
    marker: Option<CompactString>,
    toolchain: Option<PreparedToolchain>,
    capture_events: bool,
    event_batch: &mut Option<EventBatch>,
) -> Result<ConfiguredNodeResult, AnalysisError>
where
    T: ComputedAnalysis,
{
    let _implementation = starlark_rule_implementation(package, configured_target)?;
    let resolved = declared_dependency_keys
        .iter()
        .map(|dependency| {
            let result = computed.get(&dependency.key).ok_or_else(|| {
                AnalysisError::new(format!(
                    "internal error: dependency result missing for `{}`",
                    dependency.key
                ))
            })?;
            let node_key = result.result().key().clone();
            let kind = match &dependency.transition_output {
                Some(output) => {
                    crate::configured_target::ConfiguredEdgeKind::TransitionedAttribute {
                        attribute: dependency.attribute.clone(),
                        index: dependency.attribute_index,
                        output: output.clone(),
                    }
                }
                None => crate::configured_target::ConfiguredEdgeKind::OrdinaryAttribute {
                    attribute: dependency.attribute.clone(),
                    index: dependency.attribute_index,
                },
            };
            Ok((
                PreparedDependency {
                    key: result
                        .result()
                        .configured_target_key()
                        .expect("current rule analysis only prepares configured nodes")
                        .clone(),
                    providers: result.result().providers().clone(),
                    attribute: dependency.attribute.clone(),
                    sequence: dependency.sequence,
                },
                crate::configured_target::ConfiguredEdge::new(node_key, kind),
            ))
        })
        .collect::<Result<Vec<_>, AnalysisError>>()?;
    let (dependencies, edges): (Vec<_>, Vec<_>) = resolved.into_iter().unzip();
    let print_capture = capture_events.then(AnalysisPrintCapture::default);
    let label = configured_target.label();
    let value = evaluate_loaded_rule(
        package,
        label.target().as_str(),
        configured_target.clone(),
        label.package().package().as_str(),
        dependencies,
        marker,
        toolchain,
        print_capture
            .as_ref()
            .map(|capture| capture as &dyn PrintHandler),
    );
    *event_batch = print_capture.map(AnalysisPrintCapture::into_batch);
    value
        .map_err(AnalysisError::from_loaded_rule_error)
        .map(|result| result.with_edges(edges))
}

fn root_analysis_complete(
    result: Result<ConfiguredNodeResult, AnalysisError>,
) -> RootAnalysisKeyValue {
    LoadingPreparationOutcome::Complete(Arc::new(result.map(Arc::new)))
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

type PreparedToolchainOutcome = LoadingPreparationOutcome<Result<PreparedToolchain, AnalysisError>>;
type RootPackageValue = Arc<Result<LoadedPackage, slug_loading_v2::RootPackageLoadError>>;
type RootPackages = Vec<(slug_identity_v2::PackagePath, RootPackageValue)>;

fn toolchain_outcome(result: Result<PreparedToolchain, AnalysisError>) -> PreparedToolchainOutcome {
    LoadingPreparationOutcome::Complete(result)
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
    packages: &'a RootPackages,
    label: &CanonicalLabel,
) -> Result<&'a slug_loading_v2::PackageTarget, AnalysisError> {
    let package = packages
        .iter()
        .find(|(path, _)| path == label.package().package())
        .ok_or_else(|| {
            AnalysisError::new(format!("toolchain label package was not loaded: {label}"))
        })?
        .1
        .as_ref()
        .as_ref()
        .map_err(|error| AnalysisError::new(error.to_string()))?;
    package
        .targets
        .iter()
        .find(|target| target.name == label.target().as_str())
        .ok_or_else(|| AnalysisError::new(format!("toolchain target was not found: {label}")))
}

fn constraint_value_setting(
    packages: &RootPackages,
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

fn require_root_native_reference(reference: &CanonicalLabel) -> Result<(), AnalysisError> {
    if reference.package().repo().is_root() {
        Ok(())
    } else {
        Err(AnalysisError::new(format!(
            "external native toolchain reference is not supported: {reference}"
        )))
    }
}

async fn resolve_root_toolchain(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    required: &CanonicalLabel,
    configuration: ConfigurationKey,
) -> PreparedToolchainOutcome {
    let required_type = match root_apparent_type(required) {
        Ok(required_type) => required_type,
        Err(error) => return toolchain_outcome(Err(error)),
    };
    let anchor = match ctx
        .compute(&RootModuleLoadingAnchorKey::new(workspace.dupe()))
        .await
    {
        Ok(LoadingPreparationOutcome::Need(need)) => return LoadingPreparationOutcome::Need(need),
        Ok(LoadingPreparationOutcome::Complete(value)) => match value.as_ref() {
            Ok(anchor) => anchor.dupe(),
            Err(error) => return toolchain_outcome(Err(AnalysisError::new(error.to_string()))),
        },
        Err(error) => {
            return toolchain_outcome(Err(AnalysisError::new(format!(
                "loading root module anchor through DICE: {error}"
            ))));
        }
    };
    let registrations = anchor.registrations();
    let mut labels = Vec::new();
    let mut registration_error = None;
    for apparent in registrations
        .execution_platforms()
        .iter()
        .chain(registrations.toolchains())
    {
        if !apparent.repo().is_root() {
            registration_error.get_or_insert_with(|| {
                AnalysisError::new(format!(
                    "external toolchain registration is not supported: {apparent}"
                ))
            });
            continue;
        }
        labels.push(
            CanonicalLabel::parse(&format!("@@{apparent}"))
                .expect("accepted direct root registration label canonicalizes"),
        );
    }
    labels.push(required.clone());
    let mut paths = SmallSet::with_capacity(labels.len());
    for label in &labels {
        paths.insert(label.package().package().clone());
    }
    let outcomes = ctx
        .compute_join(paths.into_iter(), |ctx, package| {
            Box::pin(async move {
                let value = ctx
                    .compute(&RootPackageLoadKey::new(workspace.dupe(), package.clone()))
                    .await;
                (package, value)
            })
        })
        .await;
    let mut needs: Option<LoadingPreparationNeeds> = None;
    let mut first_error = None;
    let mut packages = Vec::with_capacity(outcomes.len());
    for (path, outcome) in outcomes {
        match outcome {
            Ok(LoadingPreparationOutcome::Need(need)) => {
                needs = Some(match needs {
                    Some(current) => current.try_union(&need).expect("root package Needs agree"),
                    None => need,
                });
            }
            Ok(LoadingPreparationOutcome::Complete(value)) => packages.push((path, value)),
            Err(error) if first_error.is_none() => {
                first_error = Some(AnalysisError::new(format!(
                    "loading toolchain package through DICE: {error}"
                )));
            }
            Err(_) => {}
        }
    }
    if let Some(need) = needs {
        return LoadingPreparationOutcome::Need(need);
    }
    if let Some(error) = first_error {
        return toolchain_outcome(Err(error));
    }
    let mut seen = labels.iter().cloned().collect::<SmallSet<_>>();
    loop {
        let mut paths = SmallSet::new();
        let mut reference_error = None;
        for label in labels.clone() {
            if let Ok(target) = package_target(&packages, &label) {
                for reference in native_references(target) {
                    if let Err(error) = require_root_native_reference(&reference) {
                        reference_error.get_or_insert(error);
                        continue;
                    }
                    if seen.insert(reference.clone()) {
                        paths.insert(reference.package().package().clone());
                        labels.push(reference);
                    }
                }
            }
        }
        let paths = paths
            .into_iter()
            .filter(|path| !packages.iter().any(|(loaded, _)| loaded == path))
            .collect::<Vec<_>>();
        if paths.is_empty() {
            if let Some(error) = reference_error {
                return toolchain_outcome(Err(error));
            }
            break;
        }
        let outcomes = ctx
            .compute_join(paths, |ctx, package| {
                Box::pin(async move {
                    let value = ctx
                        .compute(&RootPackageLoadKey::new(workspace.dupe(), package.clone()))
                        .await;
                    (package, value)
                })
            })
            .await;
        let mut needs: Option<LoadingPreparationNeeds> = None;
        let mut round_error = None;
        for (path, outcome) in outcomes {
            match outcome {
                Ok(LoadingPreparationOutcome::Need(need)) => {
                    needs = Some(needs.map_or(need.clone(), |old| {
                        old.try_union(&need).expect("root package Needs agree")
                    }))
                }
                Ok(LoadingPreparationOutcome::Complete(value)) => packages.push((path, value)),
                Err(error) if round_error.is_none() => {
                    round_error = Some(AnalysisError::new(format!(
                        "loading toolchain package through DICE: {error}"
                    )));
                }
                Err(_) => {}
            }
        }
        if let Some(need) = needs {
            return LoadingPreparationOutcome::Need(need);
        }
        if let Some(error) = round_error {
            return toolchain_outcome(Err(error));
        }
        if let Some(error) = reference_error {
            return toolchain_outcome(Err(error));
        }
    }
    if let Some(error) = registration_error {
        return toolchain_outcome(Err(error));
    }
    match package_target(&packages, required) {
        Ok(target)
            if matches!(
                target.kind,
                PackageTargetKind::NativeToolchain(NativeToolchainTarget::ToolchainType)
            ) => {}
        Ok(_) => {
            return toolchain_outcome(Err(AnalysisError::new(format!(
                "required toolchain type is not toolchain_type: {required}"
            ))));
        }
        Err(error) => return toolchain_outcome(Err(error)),
    };
    let platform_labels = registrations
        .execution_platforms()
        .iter()
        .filter(|label| label.repo().is_root())
        .map(|label| {
            CanonicalLabel::parse(&format!("@@{label}")).expect("root registration canonicalizes")
        })
        .collect::<Vec<_>>();
    let toolchain_labels = registrations
        .toolchains()
        .iter()
        .filter(|label| label.repo().is_root())
        .map(|label| {
            CanonicalLabel::parse(&format!("@@{label}")).expect("root registration canonicalizes")
        })
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
        let mut settings = SmallSet::with_capacity(constraint_values.len());
        for value in constraint_values.iter() {
            match constraint_value_setting(&packages, value) {
                Ok(setting) if settings.insert(setting.clone()) => {}
                Ok(_) => {
                    return toolchain_outcome(Err(AnalysisError::new(format!(
                        "execution platform has duplicate constraint setting: {platform_label}"
                    ))));
                }
                Err(error) => return toolchain_outcome(Err(error)),
            }
        }
    }
    for toolchain_label in &toolchain_labels {
        let target = match package_target(&packages, toolchain_label) {
            Ok(target) => target,
            Err(error) => return toolchain_outcome(Err(error)),
        };
        let PackageTargetKind::NativeToolchain(NativeToolchainTarget::Toolchain {
            toolchain_type,
            implementation,
            exec_compatible_with,
        }) = &target.kind
        else {
            return toolchain_outcome(Err(AnalysisError::new(format!(
                "registered toolchain is not toolchain: {toolchain_label}"
            ))));
        };
        if !matches!(package_target(&packages, toolchain_type), Ok(target) if matches!(target.kind, PackageTargetKind::NativeToolchain(NativeToolchainTarget::ToolchainType)))
        {
            return toolchain_outcome(Err(AnalysisError::new(format!(
                "toolchain references a non-toolchain type: {toolchain_label}"
            ))));
        }
        let mut settings = SmallSet::with_capacity(exec_compatible_with.len());
        for value in exec_compatible_with.iter() {
            match constraint_value_setting(&packages, value) {
                Ok(setting) if settings.insert(setting.clone()) => {}
                Ok(_) => {
                    return toolchain_outcome(Err(AnalysisError::new(format!(
                        "toolchain has duplicate execution constraint setting: {toolchain_label}"
                    ))));
                }
                Err(error) => return toolchain_outcome(Err(error)),
            }
        }
        let implementation = match package_target(&packages, implementation) {
            Ok(target) => target,
            Err(error) => return toolchain_outcome(Err(error)),
        };
        let PackageTargetKind::StarlarkRule(rule) = &implementation.kind else {
            return toolchain_outcome(Err(AnalysisError::new(format!(
                "toolchain implementation is not a Starlark rule: {toolchain_label}"
            ))));
        };
        let marker = rule
            .values()
            .iter()
            .find(|value| {
                value.declaration_name == "marker"
                    && value.provenance == AttributeProvenance::Explicit
            })
            .and_then(|value| match value.value.as_ref() {
                CoercedAttributeValue::String(value) => Some(value),
                _ => None,
            });
        let capability = implementation
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
        let builtin_values_are_marker_defaults = rule.values().iter().all(|value| {
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
                    let intrinsic_empty = match value.value.as_ref() {
                        CoercedAttributeValue::None => true,
                        CoercedAttributeValue::String(value) => value.is_empty(),
                        CoercedAttributeValue::LabelList(values) => values.is_empty(),
                        CoercedAttributeValue::StringList(values) => values.is_empty(),
                        CoercedAttributeValue::StringDict(values) => values.is_empty(),
                        CoercedAttributeValue::LabelListDict(values) => values.is_empty(),
                        CoercedAttributeValue::Boolean(value) => !value,
                        CoercedAttributeValue::Integer(value) => *value == 0,
                        _ => false,
                    };
                    value.provenance != AttributeProvenance::Explicit && intrinsic_empty
                }
            }
        });
        if marker.is_none()
            || !rule.dependencies().is_empty()
            || !rule.required_toolchains().is_empty()
            || rule.is_root_string_build_setting()
            || user_schema.len() != 1
            || user_schema[0].declaration_name() != "marker"
            || !matches!(user_schema[0].kind(), AttributeKind::String)
            || user_schema[0].transition().is_some()
            || user_schema[0].dependency_reachable()
            || rule.values().len() != rule.schema().len()
            || !builtin_values_are_marker_defaults
            || !empty_tags
            || capability.executable
            || capability.test_kind.is_some()
        {
            return toolchain_outcome(Err(AnalysisError::new(format!(
                "toolchain implementation is not a marker leaf: {toolchain_label}"
            ))));
        }
    }
    let mut selected = None;
    for platform_label in platform_labels {
        let platform = package_target(&packages, &platform_label).expect("validated platform");
        let PackageTargetKind::NativeToolchain(NativeToolchainTarget::Platform {
            constraint_values,
        }) = &platform.kind
        else {
            unreachable!("registered platforms were prevalidated")
        };
        for toolchain_label in &toolchain_labels {
            let toolchain =
                package_target(&packages, toolchain_label).expect("validated toolchain");
            let PackageTargetKind::NativeToolchain(NativeToolchainTarget::Toolchain {
                toolchain_type,
                implementation,
                exec_compatible_with,
            }) = &toolchain.kind
            else {
                unreachable!("registered toolchains were prevalidated")
            };
            let compatible = exec_compatible_with
                .iter()
                .all(|value| constraint_values.contains(value));
            if toolchain_type == required && compatible {
                selected = Some(implementation.clone());
                break;
            }
        }
        if selected.is_some() {
            break;
        }
    }
    let Some(implementation) = selected else {
        return toolchain_outcome(Err(AnalysisError::new(format!(
            "no compatible toolchain was registered for {required}"
        ))));
    };
    let selected_result = match ctx
        .compute(
            &ConfiguredNodeAnalysisKey::new(
                workspace.dupe(),
                ConfiguredTargetKey::new(implementation.clone(), configuration),
            )
            .expect("toolchain analysis inherits a structural configuration"),
        )
        .await
    {
        Ok(LoadingPreparationOutcome::Need(need)) => return LoadingPreparationOutcome::Need(need),
        Ok(LoadingPreparationOutcome::Complete(value)) => match value.as_ref() {
            Ok(value) => value.clone(),
            Err(error) => return toolchain_outcome(Err(error.clone())),
        },
        Err(error) => {
            return toolchain_outcome(Err(AnalysisError::new(format!(
                "analyzing selected toolchain through DICE: {error}"
            ))));
        }
    };
    if selected_result.configured_dependencies().next().is_some()
        || !selected_result.actions().is_empty()
        || !selected_result.declared_outputs().is_empty()
        || !selected_result.diagnostics().is_empty()
        || selected_result.providers().len() != 2
        || selected_result.providers().default_info()
            != Some(&slug_build_api_v2::providers::DefaultInfo::empty())
        || selected_result.providers().toolchain_info().is_none()
    {
        return toolchain_outcome(Err(AnalysisError::new(
            "selected toolchain implementation must return only empty DefaultInfo and ToolchainInfo",
        )));
    }
    toolchain_outcome(Ok(PreparedToolchain {
        required_type,
        marker: selected_result
            .providers()
            .toolchain_info()
            .expect("checked ToolchainInfo")
            .marker
            .clone(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_native_reference_is_rejected_before_same_root_path_projection() {
        let root = CanonicalLabel::parse("@@//collision:value").unwrap();
        let external = CanonicalLabel::parse("@@external//collision:value").unwrap();
        assert_eq!(root.package().package(), external.package().package());
        assert!(require_root_native_reference(&root).is_ok());
        assert!(require_root_native_reference(&external).is_err());
    }

    #[test]
    fn completed_analysis_error_is_invalid_and_not_equal_to_itself() {
        let error = root_analysis_complete(Err(AnalysisError::new("analysis failed")));

        assert!(!<ConfiguredNodeAnalysisKey as Key>::validity(&error));
        assert!(!<ConfiguredNodeAnalysisKey as Key>::equality(
            &error, &error
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
            .compute_inner(ctx, capture_events, &mut event_batch)
            .await;
        if capture_events && value.is_complete() {
            ctx.store_evaluation_data(event_batch.unwrap_or_else(EventBatch::empty))
                .expect("ConfiguredNodeAnalysisKey stores exactly one local event batch");
        }
        value
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        root_analysis_success_eq(x, y)
    }

    fn validity(value: &Self::Value) -> bool {
        root_analysis_is_success(value)
    }
}

impl ConfiguredNodeAnalysisKey {
    async fn compute_inner(
        &self,
        ctx: &mut DiceComputations<'_>,
        capture_events: bool,
        event_batch: &mut Option<EventBatch>,
    ) -> RootAnalysisKeyValue {
        let configured_target = &self.configured_target;
        if !configured_target.label().package().repo().is_root() {
            return root_analysis_complete(Err(AnalysisError::new(format!(
                "external repository configured targets are not supported: {}",
                configured_target.label()
            ))));
        }
        let label = configured_target.label();
        let package_value = match ctx
            .compute(&RootPackageLoadKey::new(
                self.workspace.dupe(),
                label.package().package().clone(),
            ))
            .await
        {
            Ok(LoadingPreparationOutcome::Need(need)) => {
                return LoadingPreparationOutcome::Need(need);
            }
            Ok(LoadingPreparationOutcome::Complete(value)) => value,
            Err(error) => {
                return root_analysis_complete(Err(AnalysisError::new(format!(
                    "loading package through DICE: {error}"
                ))));
            }
        };
        let required_root_string_setting = {
            let package = match package_value.as_ref() {
                Ok(package) => package,
                Err(error) => {
                    return root_analysis_complete(Err(AnalysisError::new(error.to_string())));
                }
            };
            let rule = match starlark_rule_implementation(package, configured_target) {
                Ok(rule) => rule,
                Err(error) => return root_analysis_complete(Err(error)),
            };
            match required_root_string_setting(rule, label) {
                Ok(required) => required,
                Err(error) => return root_analysis_complete(Err(error)),
            }
        };
        if let Err(error) = validate_carried_root_string_setting(
            configured_target.configuration(),
            required_root_string_setting.as_ref(),
        ) {
            return root_analysis_complete(Err(error));
        }
        if configured_target
            .configuration()
            .root_string_setting()
            .is_none()
            && let Some(setting) = &required_root_string_setting
        {
            return root_analysis_complete(Err(AnalysisError::new(format!(
                "configured node was constructed before resolving root string setting {setting}"
            ))));
        }
        let (requirement, marker) = {
            let package = match package_value.as_ref() {
                Ok(package) => package,
                Err(error) => {
                    return root_analysis_complete(Err(AnalysisError::new(error.to_string())));
                }
            };
            let rule = match starlark_rule_implementation(package, configured_target) {
                Ok(rule) => rule,
                Err(error) => return root_analysis_complete(Err(error)),
            };
            if rule.required_toolchains().len() > 1 {
                return root_analysis_complete(Err(AnalysisError::new(
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
            (rule.required_toolchains().first().cloned(), marker)
        };
        let prepared_toolchain = if let Some(requirement) = requirement {
            match resolve_root_toolchain(
                ctx,
                &self.workspace,
                &requirement,
                configured_target.configuration().clone(),
            )
            .await
            {
                LoadingPreparationOutcome::Need(need) => {
                    return LoadingPreparationOutcome::Need(need);
                }
                LoadingPreparationOutcome::Complete(value) => match value {
                    Ok(value) => Some(value),
                    Err(error) => return root_analysis_complete(Err(error)),
                },
            }
        } else {
            None
        };
        let declared_dependency_keys = {
            let package = match package_value.as_ref() {
                Ok(package) => package,
                Err(error) => {
                    return root_analysis_complete(Err(AnalysisError::new(error.to_string())));
                }
            };
            let dependencies = if configured_target
                .configuration()
                .root_string_setting()
                .is_some()
                || required_root_string_setting.is_some()
            {
                root_declared_dependency_keys(package, configured_target)
            } else {
                legacy_declared_dependency_keys(package, configured_target)
            };
            match dependencies {
                Ok(keys) => keys,
                Err(error) => return root_analysis_complete(Err(error)),
            }
        };

        let mut unique = SmallSet::with_capacity(declared_dependency_keys.len());
        for dependency in &declared_dependency_keys {
            unique.insert(dependency.key.clone());
        }
        let workspace = &self.workspace;
        let preparations = ctx
            .compute_join(unique.into_iter(), |ctx, configured_target| {
                Box::pin(async move {
                    let prepared = prepare_configured_node_analysis(
                        ctx,
                        workspace.dupe(),
                        configured_target.label().clone(),
                        configured_target.configuration().clone(),
                        None,
                    )
                    .await;
                    (configured_target, prepared)
                })
            })
            .await;

        let mut all_need: Option<LoadingPreparationNeeds> = None;
        let mut first_error = None;
        let mut prepared = Vec::with_capacity(preparations.len());
        for (configured_target, outcome) in preparations {
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
                LoadingPreparationOutcome::Complete(value) => match value {
                    Ok(key) => prepared.push((configured_target, key)),
                    Err(error) => {
                        if first_error.is_none() {
                            first_error = Some(error);
                        }
                    }
                },
            }
        }
        if let Some(need) = all_need {
            return LoadingPreparationOutcome::Need(need);
        }
        if let Some(error) = first_error {
            return root_analysis_complete(Err(error));
        }

        let outcomes = ctx
            .compute_join(prepared, |ctx, (configured_target, key)| {
                Box::pin(async move {
                    let result = ctx.compute(&key).await;
                    (configured_target, result)
                })
            })
            .await;
        let mut all_need: Option<LoadingPreparationNeeds> = None;
        let mut first_error = None;
        let mut computed = SmallMap::with_capacity(outcomes.len());
        for (configured_target, outcome) in outcomes {
            match outcome {
                Err(error) => {
                    if first_error.is_none() {
                        first_error = Some(AnalysisError::new(format!(
                            "computing dependency `{configured_target}` through DICE: {error}"
                        )));
                    }
                }
                Ok(LoadingPreparationOutcome::Need(need)) => {
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
                Ok(LoadingPreparationOutcome::Complete(value)) => match value.as_ref() {
                    Ok(result) => {
                        computed.insert(configured_target, result.dupe());
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
        if let Some(error) = first_error {
            return root_analysis_complete(Err(error));
        }

        let package = package_value
            .as_ref()
            .as_ref()
            .expect("validated root package value remains immutable");
        root_analysis_complete(finish_analysis(
            package,
            configured_target,
            &declared_dependency_keys,
            &computed,
            marker,
            prepared_toolchain,
            capture_events,
            event_batch,
        ))
    }
}
