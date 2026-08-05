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
use futures::FutureExt;
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
use slug_loading_v2::keys::PackageLoadKey;
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
use crate::result::AnalysisResult;
use crate::starlark_rule::PreparedDependency;
use crate::starlark_rule::PreparedToolchain;
use crate::starlark_rule::evaluate_loaded_rule;

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub enum AnalysisErrorKind {
    TargetNotFound {
        label: CanonicalLabel,
        build_file: PathBuf,
    },
    Message(String),
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct AnalysisError {
    kind: AnalysisErrorKind,
}

impl AnalysisError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            kind: AnalysisErrorKind::Message(message.into()),
        }
    }

    fn target_not_found(label: CanonicalLabel, build_file: PathBuf) -> Self {
        Self {
            kind: AnalysisErrorKind::TargetNotFound { label, build_file },
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
            AnalysisErrorKind::Message(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for AnalysisError {}

/// The single production DICE identity for configured-target analysis.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct ConfiguredTargetAnalysisKey {
    pub workspace: PathBuf,
    pub configured_target: ConfiguredTargetKey,
}

impl fmt::Display for ConfiguredTargetAnalysisKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "configured-target-analysis:{}", self.configured_target)
    }
}

type AnalysisKeyValue = Arc<Result<AnalysisResult, AnalysisError>>;

/// Dormant typed analysis identity for one root-repository configured target.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct RootConfiguredTargetAnalysisKey {
    workspace: NormalizedAbsolutePath,
    input: RootConfiguredTargetAnalysisInput,
}
#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
enum RootConfiguredTargetAnalysisInput {
    Resolved(ConfiguredTargetKey),
    RootStringSettingRequest {
        requested: CanonicalLabel,
        explicit: Option<RootStringSettingValue>,
    },
}

impl RootConfiguredTargetAnalysisKey {
    pub fn new(workspace: NormalizedAbsolutePath, configured_target: ConfiguredTargetKey) -> Self {
        Self {
            workspace,
            input: RootConfiguredTargetAnalysisInput::Resolved(configured_target),
        }
    }

    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        &self.workspace
    }

    pub fn configured_target(&self) -> &ConfiguredTargetKey {
        match &self.input {
            RootConfiguredTargetAnalysisInput::Resolved(key) => key,
            RootConfiguredTargetAnalysisInput::RootStringSettingRequest { requested, .. } => {
                panic!("unresolved root string setting request: {requested}")
            }
        }
    }

    pub fn resolved_configured_target(&self) -> Option<&ConfiguredTargetKey> {
        match &self.input {
            RootConfiguredTargetAnalysisInput::Resolved(key) => Some(key),
            RootConfiguredTargetAnalysisInput::RootStringSettingRequest { .. } => None,
        }
    }

    pub fn root_string_setting_request_parts(
        &self,
    ) -> Option<(&CanonicalLabel, Option<&RootStringSettingValue>)> {
        match &self.input {
            RootConfiguredTargetAnalysisInput::Resolved(_) => None,
            RootConfiguredTargetAnalysisInput::RootStringSettingRequest {
                requested,
                explicit,
            } => Some((requested, explicit.as_ref())),
        }
    }

    pub fn root_string_setting_request(
        workspace: NormalizedAbsolutePath,
        requested: CanonicalLabel,
        explicit: Option<RootStringSettingValue>,
    ) -> Self {
        Self {
            workspace,
            input: RootConfiguredTargetAnalysisInput::RootStringSettingRequest {
                requested,
                explicit,
            },
        }
    }
}

impl fmt::Display for RootConfiguredTargetAnalysisKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "root-configured-target-analysis:{}",
            match &self.input {
                RootConfiguredTargetAnalysisInput::Resolved(key) => key.to_string(),
                RootConfiguredTargetAnalysisInput::RootStringSettingRequest {
                    requested,
                    explicit,
                } => format!(
                    "request:{requested}={}",
                    explicit
                        .as_ref()
                        .map_or("<default>", RootStringSettingValue::as_str)
                ),
            }
        )
    }
}

type RootAnalysisKeyValue =
    LoadingPreparationOutcome<Arc<Result<Arc<AnalysisResult>, AnalysisError>>>;

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
                    "transition must return exactly one //:setting output",
                ));
            };
            if output.unpack_str() != Some(transition.output()) {
                return Err(AnalysisError::new(
                    "transition output must be exactly //:setting",
                ));
            }
            let setting = setting.unpack_str().ok_or_else(|| {
                AnalysisError::new("transition //:setting output must be a string")
            })?;
            configured_target
                .configuration()
                .with_root_string_setting(RootStringSettingValue::new(setting))
        } else {
            configured_target.configuration().clone()
        };
        for label in labels {
            dependencies.push(DeclaredDependencyKey {
                attribute: CompactString::from(value.declaration_name.as_str()),
                sequence: schema.transition().is_some()
                    || matches!(schema.kind(), slug_loading_v2::AttributeKind::LabelList),
                key: ConfiguredTargetKey::new(label.clone(), configuration.clone()),
            });
        }
    }
    Ok(dependencies)
}
#[derive(Debug, Clone)]
struct DeclaredDependencyKey {
    attribute: CompactString,
    sequence: bool,
    key: ConfiguredTargetKey,
}

trait ComputedAnalysis {
    fn result(&self) -> &AnalysisResult;
}

impl ComputedAnalysis for AnalysisResult {
    fn result(&self) -> &AnalysisResult {
        self
    }
}

impl ComputedAnalysis for Arc<AnalysisResult> {
    fn result(&self) -> &AnalysisResult {
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
        .map(|label| DeclaredDependencyKey {
            attribute: CompactString::const_new("deps"),
            sequence: true,
            key: ConfiguredTargetKey::new(label, configured_target.configuration().clone()),
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
) -> Result<AnalysisResult, AnalysisError>
where
    T: ComputedAnalysis,
{
    let _implementation = starlark_rule_implementation(package, configured_target)?;
    let dependencies = declared_dependency_keys
        .iter()
        .map(|dependency| {
            let result = computed.get(&dependency.key).ok_or_else(|| {
                AnalysisError::new(format!(
                    "internal error: dependency result missing for `{}`",
                    dependency.key
                ))
            })?;
            Ok(PreparedDependency {
                key: dependency.key.clone(),
                providers: result.result().providers().clone(),
                attribute: dependency.attribute.clone(),
                sequence: dependency.sequence,
            })
        })
        .collect::<Result<Vec<_>, AnalysisError>>()?;
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
    value.map_err(AnalysisError::new)
}

#[async_trait]
impl Key for ConfiguredTargetAnalysisKey {
    type Value = AnalysisKeyValue;

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
        let value = Arc::new(
            self.compute_inner(ctx, capture_events, &mut event_batch)
                .await,
        );
        if capture_events {
            ctx.store_evaluation_data(event_batch.unwrap_or_else(EventBatch::empty))
                .expect("ConfiguredTargetAnalysisKey stores exactly one local event batch");
        }
        value
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        matches!((x.as_ref(), y.as_ref()), (Ok(x), Ok(y)) if x == y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_ok()
    }
}

impl ConfiguredTargetAnalysisKey {
    async fn compute_inner(
        &self,
        ctx: &mut DiceComputations<'_>,
        capture_events: bool,
        event_batch: &mut Option<EventBatch>,
    ) -> Result<AnalysisResult, AnalysisError> {
        let label = self.configured_target.label();
        if !label.package().repo().is_root() {
            return Err(AnalysisError::new(format!(
                "external repository configured targets are not supported: {label}"
            )));
        }
        let package_path = self.workspace.join(label.package().package().as_str());
        let package_value = ctx
            .compute(&PackageLoadKey {
                workspace: self.workspace.clone(),
                package: package_path,
            })
            .await
            .map_err(|error| {
                AnalysisError::new(format!("loading package through DICE: {error}"))
            })?;
        let declared_dependency_keys = {
            let package = package_value
                .as_ref()
                .as_ref()
                .map_err(|error| AnalysisError::new(error.to_string()))?;
            legacy_declared_dependency_keys(package, &self.configured_target)?
        };
        let mut unique = SmallSet::with_capacity(declared_dependency_keys.len());
        for dependency in &declared_dependency_keys {
            unique.insert(dependency.key.clone());
        }
        let workspace = &self.workspace;
        let computed = ctx
            .try_compute_join(unique.into_iter(), |ctx, configured_target| {
                async move {
                    let value = ctx
                        .compute(&ConfiguredTargetAnalysisKey {
                            workspace: workspace.clone(),
                            configured_target: configured_target.clone(),
                        })
                        .await
                        .map_err(|error| {
                            AnalysisError::new(format!(
                                "computing dependency `{configured_target}` through DICE: {error}"
                            ))
                        })?;
                    let result = value.as_ref().as_ref().map_err(Clone::clone)?.clone();
                    Ok((configured_target, result))
                }
                .boxed()
            })
            .await?;
        let computed = computed
            .into_iter()
            .collect::<SmallMap<ConfiguredTargetKey, AnalysisResult>>();
        let package = package_value
            .as_ref()
            .as_ref()
            .expect("validated legacy package value remains immutable");
        finish_analysis(
            package,
            &self.configured_target,
            &declared_dependency_keys,
            &computed,
            None,
            None,
            capture_events,
            event_batch,
        )
    }
}

fn root_analysis_complete(result: Result<AnalysisResult, AnalysisError>) -> RootAnalysisKeyValue {
    LoadingPreparationOutcome::Complete(Arc::new(result.map(Arc::new)))
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
        if marker.is_none()
            || !rule.dependencies().is_empty()
            || !rule.required_toolchains().is_empty()
            || rule.is_root_string_build_setting()
            || rule.schema().len() != 2
            || rule.schema()[0].declaration_name() != "marker"
            || !matches!(rule.schema()[0].kind(), AttributeKind::String)
            || rule.schema()[0].transition().is_some()
            || rule.schema()[0].dependency_reachable()
            || rule.schema()[1].declaration_name() != "tags"
            || rule.schema()[1].transition().is_some()
            || rule.schema()[1].dependency_reachable()
            || rule.values().len() != 2
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
        .compute(&RootConfiguredTargetAnalysisKey::new(
            workspace.dupe(),
            ConfiguredTargetKey::new(implementation.clone(), configuration),
        ))
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
    if !selected_result.direct_dependencies().is_empty()
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
}

#[async_trait]
impl Key for RootConfiguredTargetAnalysisKey {
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
                .expect("RootConfiguredTargetAnalysisKey stores exactly one local event batch");
        }
        value
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

impl RootConfiguredTargetAnalysisKey {
    async fn compute_inner(
        &self,
        ctx: &mut DiceComputations<'_>,
        capture_events: bool,
        event_batch: &mut Option<EventBatch>,
    ) -> RootAnalysisKeyValue {
        if let RootConfiguredTargetAnalysisInput::RootStringSettingRequest {
            requested,
            explicit,
        } = &self.input
        {
            let setting =
                CanonicalLabel::parse("@@//:setting").expect("packet-fixed setting label is valid");
            let package = match ctx
                .compute(&RootPackageLoadKey::new(
                    self.workspace.dupe(),
                    setting.package().package().clone(),
                ))
                .await
            {
                Ok(LoadingPreparationOutcome::Need(need)) => {
                    return LoadingPreparationOutcome::Need(need);
                }
                Ok(LoadingPreparationOutcome::Complete(value)) => value,
                Err(error) => {
                    return root_analysis_complete(Err(AnalysisError::new(format!(
                        "loading setting package through DICE: {error}"
                    ))));
                }
            };
            let default = match package.as_ref() {
                Ok(package) => package
                    .targets
                    .iter()
                    .find(|target| target.name == "setting")
                    .and_then(|target| match &target.kind {
                        PackageTargetKind::StarlarkRule(rule)
                            if rule.is_root_string_build_setting() =>
                        {
                            rule.root_string_build_setting_default()
                        }
                        _ => None,
                    }),
                Err(error) => {
                    return root_analysis_complete(Err(AnalysisError::new(error.to_string())));
                }
            };
            let Some(default) = default else {
                return root_analysis_complete(Err(AnalysisError::new(
                    "root string build setting @@//:setting is missing",
                )));
            };
            let value = explicit
                .clone()
                .unwrap_or_else(|| RootStringSettingValue::new(default));
            let configuration = ConfigurationKey::target("first-build")
                .expect("existing opaque base config is valid")
                .with_root_string_setting(value);
            return ctx
                .compute(&Self::new(
                    self.workspace.dupe(),
                    ConfiguredTargetKey::new(requested.clone(), configuration),
                ))
                .await
                .unwrap_or_else(|error| {
                    root_analysis_complete(Err(AnalysisError::new(format!(
                        "resolving root string setting through DICE: {error}"
                    ))))
                });
        }
        let configured_target = match &self.input {
            RootConfiguredTargetAnalysisInput::Resolved(key) => key,
            RootConfiguredTargetAnalysisInput::RootStringSettingRequest { .. } => unreachable!(),
        };
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
        let outcomes = ctx
            .compute_join(unique.into_iter(), |ctx, configured_target| {
                Box::pin(async move {
                    let result = ctx
                        .compute(&RootConfiguredTargetAnalysisKey::new(
                            workspace.dupe(),
                            configured_target.clone(),
                        ))
                        .await;
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
