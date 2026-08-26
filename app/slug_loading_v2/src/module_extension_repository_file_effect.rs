/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select either.
 */

use std::cell::RefCell;
use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use compact_str::CompactString;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan;
use slug_bzlmod_v2::GeneratedRepositoryFileEffectPlanBuilder;
use slug_bzlmod_v2::GeneratedRepositoryFileEffectPlanError;
use slug_bzlmod_v2::HostSelectedExtensionOwner;
use slug_bzlmod_v2::RootPackageBzlTarget;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_events_v2::CaptureEvaluationEvents;
use slug_events_v2::EvaluationEvent;
use slug_events_v2::EventBatch;
use slug_events_v2::StarlarkSourceLocation;
use slug_identity_v2::CanonicalLabel;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathObservationEpoch;
use starlark::PrintHandler;
use starlark::PrintLocation;
use starlark::any::ProvidesStaticType;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::none::NoneType;
use starlark::values::starlark_value;

use crate::bzl_module::HostBzlModuleError;
use crate::bzl_module::HostBzlModuleEvalKey;
use crate::bzl_module::HostBzlModuleObservationKey;
use crate::bzl_module::HostRootBzlLabel;
use crate::module_extension_repository_instantiation::HostInstantiatedModuleExtensionRepository;
use crate::module_extension_repository_rule::FrozenRepositoryRuleDefinition;
use crate::module_extension_repository_validation::HostSelectedExtensionOwnerCertificate;
use crate::module_extension_repository_validation::HostSelectedExtensionOwnerCertificateError;
use crate::module_extension_repository_validation::HostSelectedExtensionOwnerCertificateKey;
use crate::module_extension_repository_validation::HostSelectedExtensionOwnerCertificateObservationError;
use crate::module_extension_repository_validation::HostSelectedExtensionOwnerCertificateObservationKey;

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostSelectedRepositoryFileEffect {
    certificate: Arc<HostSelectedExtensionOwnerCertificate>,
    ordinal: usize,
    plan: GeneratedRepositoryFileEffectPlan,
}

impl HostSelectedRepositoryFileEffect {
    pub fn plan(&self) -> &GeneratedRepositoryFileEffectPlan {
        &self.plan
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostSelectedRepositoryFileEffectHostBzlError(HostBzlModuleError);

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum HostSelectedRepositoryFileEffectError {
    Certificate(HostSelectedExtensionOwnerCertificateError),
    Compute(CompactString),
    MissingOrdinal {
        certificate: Arc<HostSelectedExtensionOwnerCertificate>,
        ordinal: usize,
    },
    UnsupportedDefiningLabel {
        certificate: Arc<HostSelectedExtensionOwnerCertificate>,
        ordinal: usize,
        label: CanonicalLabel,
    },
    HostBzl {
        certificate: Arc<HostSelectedExtensionOwnerCertificate>,
        ordinal: usize,
        error: HostSelectedRepositoryFileEffectHostBzlError,
    },
    Projection {
        certificate: Arc<HostSelectedExtensionOwnerCertificate>,
        ordinal: usize,
        message: CompactString,
    },
    Path {
        certificate: Arc<HostSelectedExtensionOwnerCertificate>,
        ordinal: usize,
        error: GeneratedRepositoryFileEffectPlanError,
    },
    Invocation {
        certificate: Arc<HostSelectedExtensionOwnerCertificate>,
        ordinal: usize,
        message: CompactString,
    },
    Result {
        certificate: Arc<HostSelectedExtensionOwnerCertificate>,
        ordinal: usize,
        type_name: CompactString,
    },
}

impl fmt::Display for HostSelectedRepositoryFileEffectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for HostSelectedRepositoryFileEffectError {}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostSelectedRepositoryFileEffectKey {
    workspace: NormalizedAbsolutePath,
    owner: Arc<HostSelectedExtensionOwner>,
    ordinal: usize,
}

impl HostSelectedRepositoryFileEffectKey {
    pub fn new(
        workspace: NormalizedAbsolutePath,
        owner: Arc<HostSelectedExtensionOwner>,
        ordinal: usize,
    ) -> Self {
        Self {
            workspace,
            owner,
            ordinal,
        }
    }
}

impl fmt::Display for HostSelectedRepositoryFileEffectKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-selected-repository-file-effect:{}:{:?}:{}",
            self.workspace, self.owner, self.ordinal
        )
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostSelectedRepositoryFileEffectObservationKey(HostSelectedRepositoryFileEffectKey);

impl HostSelectedRepositoryFileEffectObservationKey {
    pub fn new(
        workspace: NormalizedAbsolutePath,
        owner: Arc<HostSelectedExtensionOwner>,
        ordinal: usize,
    ) -> Self {
        Self(HostSelectedRepositoryFileEffectKey::new(
            workspace, owner, ordinal,
        ))
    }
}

impl fmt::Display for HostSelectedRepositoryFileEffectObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

type EffectResult =
    Arc<Result<HostSelectedRepositoryFileEffect, HostSelectedRepositoryFileEffectError>>;

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedHostSelectedRepositoryFileEffect {
    result: EffectResult,
    observations: PathObservationEpoch,
}

impl ObservedHostSelectedRepositoryFileEffect {
    pub fn result(&self) -> &EffectResult {
        &self.result
    }

    pub fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum HostSelectedRepositoryFileEffectObservationError {
    Certificate(HostSelectedExtensionOwnerCertificateObservationError),
    HostBzl {
        certificate: Arc<HostSelectedExtensionOwnerCertificate>,
        ordinal: usize,
        error: ObservedPathFrontierError,
    },
    Merge {
        certificate: Arc<HostSelectedExtensionOwnerCertificate>,
        ordinal: usize,
        error: ObservedPathFrontierError,
    },
}

#[derive(Clone, Copy)]
enum EffectMode {
    Legacy,
    Observed,
}

type EffectDriver = SourcePreparationOutcome<
    Result<(EffectResult, PathObservationEpoch), HostSelectedRepositoryFileEffectObservationError>,
>;

#[derive(Default)]
struct InvocationPrintCapture {
    events: RefCell<Vec<EvaluationEvent>>,
}

impl InvocationPrintCapture {
    fn into_batch(self) -> EventBatch {
        EventBatch::from_events(self.events.into_inner())
    }
}

impl PrintHandler for InvocationPrintCapture {
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum RepositoryFileContextError {
    PathArgument,
    Plan(GeneratedRepositoryFileEffectPlanError),
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct RepositoryFileContext {}

#[derive(Debug, ProvidesStaticType)]
struct RepositoryFileInvocationState {
    effects: RefCell<Option<GeneratedRepositoryFileEffectPlanBuilder>>,
    error: RefCell<Option<RepositoryFileContextError>>,
}

impl RepositoryFileInvocationState {
    fn new() -> Self {
        Self {
            effects: RefCell::new(Some(GeneratedRepositoryFileEffectPlan::builder())),
            error: RefCell::new(None),
        }
    }

    fn from_evaluator<'a>(
        eval: &'a Evaluator<'_, '_, '_>,
    ) -> anyhow::Result<&'a RepositoryFileInvocationState> {
        eval.extra
            .and_then(|extra| extra.downcast_ref::<Self>())
            .ok_or_else(|| anyhow::anyhow!("repository_ctx is outside repository-rule execution"))
    }

    fn fail(&self, error: RepositoryFileContextError) -> anyhow::Error {
        *self.error.borrow_mut() = Some(error);
        anyhow::anyhow!("unsupported repository_ctx.file argument")
    }

    fn take_error(&self) -> Option<RepositoryFileContextError> {
        self.error.borrow_mut().take()
    }

    fn finish(&self) -> GeneratedRepositoryFileEffectPlan {
        self.effects
            .borrow_mut()
            .take()
            .expect("repository file context completes at most once")
            .finish()
    }
}

impl fmt::Display for RepositoryFileContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<repository_ctx>")
    }
}

starlark::starlark_simple_value!(RepositoryFileContext);

#[starlark_value(type = "repository_ctx")]
impl<'v> StarlarkValue<'v> for RepositoryFileContext {
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(repository_file_context_methods)
    }
}

#[starlark_module]
fn repository_file_context_methods(builder: &mut MethodsBuilder) {
    fn file<'v>(
        this: Value<'v>,
        #[starlark(require = pos)] path: Value<'v>,
        #[starlark(default = "")] content: &str,
        #[starlark(default = true)] executable: bool,
        #[starlark(default = false)] legacy_utf8: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        RepositoryFileContext::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("invalid repository_ctx receiver"))?;
        let _ = legacy_utf8;
        let state = RepositoryFileInvocationState::from_evaluator(eval)?;
        let Some(path) = path.unpack_str() else {
            return Err(state.fail(RepositoryFileContextError::PathArgument));
        };
        let result = state
            .effects
            .borrow_mut()
            .as_mut()
            .expect("repository context has not completed")
            .push(
                CompactString::new(path),
                Arc::from(content.as_bytes()),
                executable,
            );
        if let Err(error) = result {
            return Err(state.fail(RepositoryFileContextError::Plan(error)));
        }
        Ok(NoneType)
    }
}

fn merge_observations(
    left: &PathObservationEpoch,
    right: &PathObservationEpoch,
) -> Result<PathObservationEpoch, ObservedPathFrontierError> {
    PathObservationEpoch::from_shared(
        left.observations()
            .iter()
            .map(|(demand, result)| (demand.dupe(), result.dupe()))
            .chain(
                right
                    .observations()
                    .iter()
                    .map(|(demand, result)| (demand.dupe(), result.dupe())),
            ),
    )
    .map_err(ObservedPathFrontierError::from)
}

fn root_label(
    certificate: &Arc<HostSelectedExtensionOwnerCertificate>,
    ordinal: usize,
    repository: &HostInstantiatedModuleExtensionRepository,
) -> Result<HostRootBzlLabel, HostSelectedRepositoryFileEffectError> {
    let label = &repository.call().definition.defining_label;
    if !label.package().repo().is_root() {
        return Err(
            HostSelectedRepositoryFileEffectError::UnsupportedDefiningLabel {
                certificate: certificate.clone(),
                ordinal,
                label: label.clone(),
            },
        );
    }
    let target = RootPackageBzlTarget::parse(label.target().as_str()).map_err(|_| {
        HostSelectedRepositoryFileEffectError::UnsupportedDefiningLabel {
            certificate: certificate.clone(),
            ordinal,
            label: label.clone(),
        }
    })?;
    Ok(HostRootBzlLabel::new(
        label.package().package().clone(),
        target,
    ))
}

fn authenticate_rule(
    certificate: &Arc<HostSelectedExtensionOwnerCertificate>,
    ordinal: usize,
    repository: &HostInstantiatedModuleExtensionRepository,
    module: &crate::bzl_module::FrozenBzlModule,
) -> Result<starlark::values::FrozenValue, HostSelectedRepositoryFileEffectError> {
    let call = repository.call();
    let value = module
        .module
        .get(&call.definition.exported_name)
        .map_err(|error| HostSelectedRepositoryFileEffectError::Projection {
            certificate: certificate.clone(),
            ordinal,
            message: error.to_string().into(),
        })?;
    let rule = value
        .downcast::<FrozenRepositoryRuleDefinition>()
        .map_err(|_| HostSelectedRepositoryFileEffectError::Projection {
            certificate: certificate.clone(),
            ordinal,
            message: "selected export is not repository_rule".into(),
        })?;
    let Some(projection) = rule.projection() else {
        return Err(HostSelectedRepositoryFileEffectError::Projection {
            certificate: certificate.clone(),
            ordinal,
            message: "selected repository_rule is not exported".into(),
        });
    };
    if projection != call.definition {
        return Err(HostSelectedRepositoryFileEffectError::Projection {
            certificate: certificate.clone(),
            ordinal,
            message: "reacquired repository_rule projection differs".into(),
        });
    }
    Ok(rule.implementation())
}

fn finish_result(
    certificate: Arc<HostSelectedExtensionOwnerCertificate>,
    ordinal: usize,
    implementation: starlark::values::FrozenValue,
    capture: Option<InvocationPrintCapture>,
    ctx: &mut DiceComputations<'_>,
) -> Result<HostSelectedRepositoryFileEffect, HostSelectedRepositoryFileEffectError> {
    let invocation_module = Module::new();
    let context = invocation_module
        .heap()
        .alloc_simple(RepositoryFileContext {});
    let state = RepositoryFileInvocationState::new();
    let returned = {
        let mut evaluator = Evaluator::new(&invocation_module);
        if let Some(capture) = capture.as_ref() {
            evaluator.set_print_handler(capture);
        }
        evaluator.extra = Some(&state);
        evaluator.eval_function(implementation.to_value(), &[context], &[])
    };
    let context_error = state.take_error();
    let result = match returned {
        Err(error) => match context_error {
            Some(RepositoryFileContextError::PathArgument) => {
                Err(HostSelectedRepositoryFileEffectError::Invocation {
                    certificate,
                    ordinal,
                    message: "repository_ctx.file path must be a string".into(),
                })
            }
            Some(RepositoryFileContextError::Plan(error)) => {
                Err(HostSelectedRepositoryFileEffectError::Path {
                    certificate,
                    ordinal,
                    error,
                })
            }
            None => Err(HostSelectedRepositoryFileEffectError::Invocation {
                certificate,
                ordinal,
                message: error.to_string().into(),
            }),
        },
        Ok(value) if !value.is_none() => Err(HostSelectedRepositoryFileEffectError::Result {
            certificate,
            ordinal,
            type_name: value.get_type().into(),
        }),
        Ok(_) => Ok(HostSelectedRepositoryFileEffect {
            certificate,
            ordinal,
            plan: state.finish(),
        }),
    };
    if let Some(capture) = capture {
        ctx.store_evaluation_data(capture.into_batch())
            .expect("repository-file invocation stores one local Complete event batch");
    }
    result
}

async fn compute_effect(
    ctx: &mut DiceComputations<'_>,
    key: &HostSelectedRepositoryFileEffectKey,
    mode: EffectMode,
) -> EffectDriver {
    let (certificate, observations) = match mode {
        EffectMode::Legacy => match ctx
            .compute(&HostSelectedExtensionOwnerCertificateKey::new(
                key.workspace.dupe(),
                key.owner.clone(),
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(value)) => (value, PathObservationEpoch::empty()),
            Err(error) => {
                return SourcePreparationOutcome::Complete(Ok((
                    Arc::new(Err(HostSelectedRepositoryFileEffectError::Compute(
                        error.to_string().into(),
                    ))),
                    PathObservationEpoch::empty(),
                )));
            }
        },
        EffectMode::Observed => match ctx
            .compute(&HostSelectedExtensionOwnerCertificateObservationKey::new(
                key.workspace.dupe(),
                key.owner.clone(),
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                return SourcePreparationOutcome::Complete(Err(
                    HostSelectedRepositoryFileEffectObservationError::Certificate(error),
                ));
            }
            Ok(SourcePreparationOutcome::Complete(Ok(value))) => {
                (value.result().dupe(), value.observations().dupe())
            }
            Err(error) => {
                return SourcePreparationOutcome::Complete(Ok((
                    Arc::new(Err(HostSelectedRepositoryFileEffectError::Compute(
                        error.to_string().into(),
                    ))),
                    PathObservationEpoch::empty(),
                )));
            }
        },
    };
    let certificate = match certificate.as_ref() {
        Ok(value) => Arc::new(value.clone()),
        Err(error) => {
            return SourcePreparationOutcome::Complete(Ok((
                Arc::new(Err(HostSelectedRepositoryFileEffectError::Certificate(
                    error.clone(),
                ))),
                observations,
            )));
        }
    };
    let Some(repository) = certificate.repository(key.ordinal) else {
        return SourcePreparationOutcome::Complete(Ok((
            Arc::new(Err(HostSelectedRepositoryFileEffectError::MissingOrdinal {
                certificate,
                ordinal: key.ordinal,
            })),
            observations,
        )));
    };
    let repository = repository.clone();
    let label = match root_label(&certificate, key.ordinal, &repository) {
        Ok(label) => label,
        Err(error) => {
            return SourcePreparationOutcome::Complete(Ok((Arc::new(Err(error)), observations)));
        }
    };
    let (module, child_observations) = match mode {
        EffectMode::Legacy => match ctx
            .compute(&HostBzlModuleEvalKey::new(key.workspace.dupe(), label))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(value)) => {
                (value.as_ref().clone(), PathObservationEpoch::empty())
            }
            Err(error) => {
                return SourcePreparationOutcome::Complete(Ok((
                    Arc::new(Err(HostSelectedRepositoryFileEffectError::Compute(
                        error.to_string().into(),
                    ))),
                    observations,
                )));
            }
        },
        EffectMode::Observed => match ctx
            .compute(&HostBzlModuleObservationKey::new(
                key.workspace.dupe(),
                label,
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                return SourcePreparationOutcome::Complete(Err(
                    HostSelectedRepositoryFileEffectObservationError::HostBzl {
                        certificate,
                        ordinal: key.ordinal,
                        error,
                    },
                ));
            }
            Ok(SourcePreparationOutcome::Complete(Ok(value))) => {
                (value.result().clone(), value.observations().dupe())
            }
            Err(error) => {
                return SourcePreparationOutcome::Complete(Ok((
                    Arc::new(Err(HostSelectedRepositoryFileEffectError::Compute(
                        error.to_string().into(),
                    ))),
                    observations,
                )));
            }
        },
    };
    let observations = match merge_observations(&observations, &child_observations) {
        Ok(value) => value,
        Err(error) => {
            return SourcePreparationOutcome::Complete(Err(
                HostSelectedRepositoryFileEffectObservationError::Merge {
                    certificate,
                    ordinal: key.ordinal,
                    error,
                },
            ));
        }
    };
    let module = match module {
        Ok(module) => module,
        Err(error) => {
            return SourcePreparationOutcome::Complete(Ok((
                Arc::new(Err(HostSelectedRepositoryFileEffectError::HostBzl {
                    certificate,
                    ordinal: key.ordinal,
                    error: HostSelectedRepositoryFileEffectHostBzlError(error),
                })),
                observations,
            )));
        }
    };
    let implementation = match authenticate_rule(&certificate, key.ordinal, &repository, &module) {
        Ok(value) => value,
        Err(error) => {
            return SourcePreparationOutcome::Complete(Ok((Arc::new(Err(error)), observations)));
        }
    };
    let capture = ctx
        .per_transaction_data()
        .data
        .get::<CaptureEvaluationEvents>()
        .is_ok()
        .then(InvocationPrintCapture::default);
    let result = finish_result(certificate, key.ordinal, implementation, capture, ctx);
    SourcePreparationOutcome::Complete(Ok((Arc::new(result), observations)))
}

#[async_trait]
impl Key for HostSelectedRepositoryFileEffectKey {
    type Value = SourcePreparationOutcome<EffectResult>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match compute_effect(ctx, self, EffectMode::Legacy).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                debug_assert!(observations.observations().is_empty());
                SourcePreparationOutcome::Complete(result)
            }
            SourcePreparationOutcome::Complete(Err(_)) => {
                unreachable!("legacy repository-file effect has no observed outer")
            }
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[async_trait]
impl Key for HostSelectedRepositoryFileEffectObservationKey {
    type Value = SourcePreparationOutcome<
        Result<
            ObservedHostSelectedRepositoryFileEffect,
            HostSelectedRepositoryFileEffectObservationError,
        >,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match compute_effect(ctx, &self.0, EffectMode::Observed).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                SourcePreparationOutcome::Complete(Err(error))
            }
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                SourcePreparationOutcome::Complete(Ok(ObservedHostSelectedRepositoryFileEffect {
                    result,
                    observations,
                }))
            }
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::Mutex;

    use dice::ActivationData;
    use dice::ActivationKind;
    use dice::ActivationTracker;
    use dice::DetectCycles;
    use dice::Dice;
    use dice::DynKey;
    use dice::Key;
    use dice::RichActivation;
    use dupe::Dupe;
    use slug_bzlmod_v2::HostSelectedExtensionDemandKey;
    use slug_bzlmod_v2::SourcePreparationNeeds;
    use slug_identity_v2::CanonicalRepoName;
    use slug_workspace_v2::NeedPathObservations;
    use slug_workspace_v2::NormalizedAbsolutePath;
    use slug_workspace_v2::PathObservationDemand;
    use slug_workspace_v2::PathObservationEpochKey;
    use slug_workspace_v2::PathObservationNamespace;
    use slug_workspace_v2::PathObservationOperation;
    use starlark::environment::FrozenModule;
    use starlark::environment::Globals;
    use starlark::syntax::AstModule;
    use starlark::syntax::Dialect;

    use super::*;
    use crate::module_extension_repository_instantiation::tests::WORKSPACE;
    use crate::module_extension_repository_instantiation::tests::transaction_untracked;
    use crate::module_extension_repository_instantiation::tests::transaction_with_tracker;

    enum InvocationOutcome {
        Complete(GeneratedRepositoryFileEffectPlan),
        Context(RepositoryFileContextError),
        Binding,
    }

    fn load(source: &str) -> FrozenModule {
        let ast = AstModule::parse("//:repo.bzl", source.to_owned(), &Dialect::Standard).unwrap();
        let module = Module::new();
        Evaluator::new(&module)
            .eval_module(ast, &Globals::standard())
            .unwrap();
        module.freeze().unwrap()
    }

    fn invoke_outcome(source: &str) -> InvocationOutcome {
        let frozen = load(source);
        let module = Module::new();
        let function = frozen.get("run").unwrap().owned_value(module.frozen_heap());
        let context = module.heap().alloc_simple(RepositoryFileContext {});
        let state = RepositoryFileInvocationState::new();
        let result = {
            let mut evaluator = Evaluator::new(&module);
            evaluator.extra = Some(&state);
            evaluator.eval_function(function, &[context], &[])
        };
        match result {
            Ok(value) => {
                assert!(value.is_none());
                assert!(state.take_error().is_none());
                InvocationOutcome::Complete(state.finish())
            }
            Err(_) => match state.take_error() {
                Some(error) => InvocationOutcome::Context(error),
                None => InvocationOutcome::Binding,
            },
        }
    }

    fn invoke(
        source: &str,
    ) -> Result<GeneratedRepositoryFileEffectPlan, RepositoryFileContextError> {
        match invoke_outcome(source) {
            InvocationOutcome::Complete(plan) => Ok(plan),
            InvocationOutcome::Context(error) => Err(error),
            InvocationOutcome::Binding => panic!("expected repository context failure"),
        }
    }

    #[test]
    fn repository_ctx_file_preserves_defaults_order_bytes_and_trailing_forms() {
        let plan = invoke(
            r#"
def run(ctx):
    ctx.file("BUILD.bazel", "exports_files([\"generated.txt\"])\n")
    ctx.file("generated.txt", "hello from extension\n", False, True)
"#,
        )
        .unwrap();
        assert_eq!(plan.effects().len(), 2);
        assert_eq!(plan.effects()[0].path(), "BUILD.bazel");
        assert_eq!(
            plan.effects()[0].content(),
            b"exports_files([\"generated.txt\"])\n"
        );
        assert!(plan.effects()[0].executable());
        assert_eq!(plan.effects()[1].path(), "generated.txt");
        assert_eq!(plan.effects()[1].content(), b"hello from extension\n");
        assert!(!plan.effects()[1].executable());
    }

    #[test]
    fn repository_ctx_file_records_first_typed_path_failure() {
        let repeated = invoke(
            r#"
def run(ctx):
    ctx.file("first", "one")
    ctx.file("first", "two")
"#,
        );
        assert!(matches!(
            repeated,
            Err(RepositoryFileContextError::Plan(
                GeneratedRepositoryFileEffectPlanError::RepeatedPath(path)
            )) if path == "first"
        ));
        for path in ["", "/absolute", "a/../b", "a\\b", "a/"] {
            let source = format!("def run(ctx):\n    ctx.file({path:?})\n");
            assert!(matches!(
                invoke(&source),
                Err(RepositoryFileContextError::Plan(
                    GeneratedRepositoryFileEffectPlanError::InvalidPath(_)
                ))
            ));
        }
        assert!(matches!(
            invoke("def run(ctx):\n    ctx.file(1)\n"),
            Err(RepositoryFileContextError::PathArgument)
        ));
    }

    #[test]
    fn repository_ctx_file_uses_normal_starlark_binding_for_path_and_named_arguments() {
        let plan = invoke(
            r#"
def run(ctx):
    ctx.file("named", content="bytes", executable=False, legacy_utf8=True)
"#,
        )
        .unwrap();
        assert_eq!(plan.effects()[0].path(), "named");
        assert_eq!(plan.effects()[0].content(), b"bytes");
        assert!(!plan.effects()[0].executable());

        for source in [
            "def run(ctx):\n    ctx.file(path='named')\n",
            "def run(ctx):\n    ctx.file('named', 'one', content='two')\n",
            "def run(ctx):\n    ctx.file('named', missing=True)\n",
            "def run(ctx):\n    ctx.unknown()\n",
        ] {
            assert!(matches!(invoke_outcome(source), InvocationOutcome::Binding));
        }
        for source in [
            "def run(ctx):\n    ctx.file(True)\n",
            "def run(ctx):\n    ctx.file([])\n",
        ] {
            assert!(matches!(
                invoke_outcome(source),
                InvocationOutcome::Context(RepositoryFileContextError::PathArgument)
            ));
        }
    }

    const MODULE: &str = "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\nuse_repo(e, generated='first')\n";
    const EXTENSION: &str = r#"
def write(ctx):
    print('repository-file-effect')
    ctx.file('BUILD.bazel', content='exports_files([\"generated.txt\"])\\n')
    ctx.file('generated.txt', 'from-rule', executable=False)
repo=repository_rule(implementation=write)
def impl(ctx):
    repo(name='first')
ext=module_extension(implementation=impl)
"#;

    #[derive(Default)]
    struct EffectTracker(Mutex<Vec<(String, ActivationKind, Option<EventBatch>)>>);

    impl EffectTracker {
        fn take(&self) -> Vec<(String, ActivationKind, Option<EventBatch>)> {
            std::mem::take(&mut *self.0.lock().unwrap())
        }
    }

    impl ActivationTracker for EffectTracker {
        fn key_activated(
            &self,
            _: &DynKey,
            _: &mut dyn Iterator<Item = &DynKey>,
            _: ActivationData,
        ) {
        }

        fn tracks_rich_activations(&self) -> bool {
            true
        }

        fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
            self.0.lock().unwrap().push((
                key.to_string(),
                activation.kind(),
                activation
                    .evaluation_data()
                    .and_then(|data| data.downcast_ref::<EventBatch>())
                    .map(Dupe::dupe),
            ));
        }
    }

    async fn owner(transaction: &mut dice::DiceTransaction) -> Arc<HostSelectedExtensionOwner> {
        let requested = CanonicalRepoName::new("+ext+first").unwrap();
        let demand = transaction
            .compute(&HostSelectedExtensionDemandKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                requested,
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(demand) = demand else {
            panic!("selected demand must complete")
        };
        demand.as_ref().as_ref().unwrap().owner().clone()
    }

    fn observed_carrier(
        value: &<HostSelectedRepositoryFileEffectObservationKey as Key>::Value,
    ) -> &ObservedHostSelectedRepositoryFileEffect {
        match value {
            SourcePreparationOutcome::Complete(Ok(value)) => value,
            value => panic!("expected observed repository-file carrier: {value:?}"),
        }
    }

    #[tokio::test]
    async fn selected_repository_file_effect_is_owner_ordinal_scoped_and_observed() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let mut transaction = transaction_untracked(&dice, MODULE, EXTENSION, true).await;
        let owner = owner(&mut transaction).await;
        let legacy_key =
            HostSelectedRepositoryFileEffectKey::new(workspace.dupe(), owner.clone(), 0);
        let observed_key =
            HostSelectedRepositoryFileEffectObservationKey::new(workspace.dupe(), owner.clone(), 0);
        let legacy = transaction.compute(&legacy_key).await.unwrap();
        let observed = transaction.compute(&observed_key).await.unwrap();
        let SourcePreparationOutcome::Complete(legacy_result) = &legacy else {
            panic!("legacy file effect must complete")
        };
        let carrier = observed_carrier(&observed);
        assert_eq!(legacy_result, carrier.result());
        assert!(HostSelectedRepositoryFileEffectKey::equality(
            &legacy, &legacy
        ));
        assert!(HostSelectedRepositoryFileEffectKey::validity(&legacy));
        assert!(HostSelectedRepositoryFileEffectObservationKey::equality(
            &observed, &observed
        ));
        assert!(HostSelectedRepositoryFileEffectObservationKey::validity(
            &observed
        ));
        let effect = legacy_result.as_ref().as_ref().unwrap();
        assert_eq!(effect.plan().effects().len(), 2);
        assert_eq!(effect.plan().effects()[0].path(), "BUILD.bazel");
        assert_eq!(effect.plan().effects()[1].path(), "generated.txt");
        assert!(!effect.plan().effects()[1].executable());

        let certificate = transaction
            .compute(&HostSelectedExtensionOwnerCertificateObservationKey::new(
                workspace.dupe(),
                owner.clone(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(certificate)) = certificate else {
            panic!("observed certificate must complete")
        };
        let certificate = certificate.dupe();
        let certificate_value = Arc::new(certificate.result().as_ref().as_ref().unwrap().clone());
        let repository = certificate_value.repository(0).unwrap();
        let label = root_label(&certificate_value, 0, repository).unwrap();
        let child = transaction
            .compute(&HostBzlModuleObservationKey::new(workspace.dupe(), label))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(child)) = child else {
            panic!("observed HostBzl must complete")
        };
        assert_eq!(
            carrier.observations(),
            &merge_observations(certificate.observations(), child.observations()).unwrap()
        );
        let global = transaction.compute(&PathObservationEpochKey).await.unwrap();
        for (demand, result) in carrier.observations().observations() {
            assert_eq!(result.as_ref(), global.get(demand).unwrap().as_ref());
        }

        let missing = transaction
            .compute(&HostSelectedRepositoryFileEffectKey::new(
                workspace, owner, 1,
            ))
            .await
            .unwrap();
        assert!(matches!(
            missing,
            SourcePreparationOutcome::Complete(value)
                if matches!(value.as_ref(), Err(HostSelectedRepositoryFileEffectError::MissingOrdinal { ordinal: 1, .. }))
        ));
    }

    #[tokio::test]
    async fn selected_repository_file_effect_owns_cold_warm_events_and_recovers_after_cancel() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let tracker = Arc::new(EffectTracker::default());
        let mut transaction =
            transaction_with_tracker(&dice, MODULE, EXTENSION, true, tracker.clone()).await;
        let selected_owner = owner(&mut transaction).await;
        tracker.take();
        let key = HostSelectedRepositoryFileEffectObservationKey::new(
            workspace.dupe(),
            selected_owner,
            0,
        );
        let first = observed_carrier(&transaction.compute(&key).await.unwrap()).dupe();
        let cold = tracker.take();
        let rows = cold
            .iter()
            .filter(|(name, _, _)| name == &key.to_string())
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].1, ActivationKind::Evaluated);
        assert_eq!(
            rows[0]
                .2
                .as_ref()
                .unwrap()
                .events()
                .iter()
                .filter_map(|event| match event {
                    EvaluationEvent::StarlarkPrint { text, .. } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            ["repository-file-effect"]
        );
        let second = observed_carrier(&transaction.compute(&key).await.unwrap()).dupe();
        assert!(Arc::ptr_eq(first.result(), second.result()));
        assert!(tracker.take().iter().any(|(name, kind, batch)| {
            name == &key.to_string() && *kind == ActivationKind::Reused && batch.is_none()
        }));

        let cancelled_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let cancelled_tracker = Arc::new(EffectTracker::default());
        let mut cancelled = transaction_with_tracker(
            &cancelled_dice,
            MODULE,
            EXTENSION,
            true,
            cancelled_tracker.clone(),
        )
        .await;
        let cancelled_owner = owner(&mut cancelled).await;
        cancelled_tracker.take();
        let cancelled_key = HostSelectedRepositoryFileEffectObservationKey::new(
            workspace.dupe(),
            cancelled_owner,
            0,
        );
        let mut future = Box::pin(cancelled.compute(&cancelled_key));
        std::future::poll_fn(|context| {
            assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        drop(future);
        assert!(
            cancelled_tracker
                .take()
                .iter()
                .all(|(name, _, _)| name != &cancelled_key.to_string())
        );
        let mut recovery = transaction_untracked(&cancelled_dice, MODULE, EXTENSION, true).await;
        let recovered_owner = owner(&mut recovery).await;
        let recovered = observed_carrier(
            &recovery
                .compute(&HostSelectedRepositoryFileEffectObservationKey::new(
                    workspace,
                    recovered_owner,
                    0,
                ))
                .await
                .unwrap(),
        )
        .dupe();
        assert_eq!(recovered.result(), first.result());
    }

    #[tokio::test]
    async fn selected_repository_file_effect_reloads_implementation_and_is_complete_only() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let replacement = EXTENSION.replace("from-rule", "from-reloaded-rule");
        let mut values = Vec::new();
        for extension in [EXTENSION, replacement.as_str(), EXTENSION] {
            let mut transaction = transaction_untracked(&dice, MODULE, extension, true).await;
            let selected_owner = owner(&mut transaction).await;
            let value = transaction
                .compute(&HostSelectedRepositoryFileEffectKey::new(
                    workspace.dupe(),
                    selected_owner,
                    0,
                ))
                .await
                .unwrap();
            let SourcePreparationOutcome::Complete(value) = value else {
                panic!("reloaded file effect must complete")
            };
            values.push(value);
        }
        assert_eq!(
            values[0].as_ref().as_ref().unwrap().plan().effects()[1].content(),
            b"from-rule"
        );
        assert_eq!(
            values[1].as_ref().as_ref().unwrap().plan().effects()[1].content(),
            b"from-reloaded-rule"
        );
        assert_eq!(values[0], values[2]);
        assert_ne!(values[0], values[1]);

        let need: <HostSelectedRepositoryFileEffectKey as Key>::Value =
            SourcePreparationOutcome::Need(SourcePreparationNeeds::path(
                NeedPathObservations::singleton(PathObservationDemand::new(
                    PathObservationNamespace::Host,
                    NormalizedAbsolutePath::new("/repository-file-effect-need").unwrap(),
                    PathObservationOperation::Lstat,
                )),
            ));
        assert!(!HostSelectedRepositoryFileEffectKey::validity(&need));
        assert!(!HostSelectedRepositoryFileEffectObservationKey::validity(
            &SourcePreparationOutcome::Need(match need {
                SourcePreparationOutcome::Need(need) => need,
                SourcePreparationOutcome::Complete(_) => unreachable!(),
            }),
        ));
    }

    #[tokio::test]
    async fn selected_repository_file_effect_forwards_real_observed_need_without_batch() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let tracker = Arc::new(EffectTracker::default());
        let mut transaction =
            transaction_with_tracker(&dice, MODULE, EXTENSION, true, tracker.clone()).await;
        let selected_owner = owner(&mut transaction).await;
        let epoch = transaction.compute(&PathObservationEpochKey).await.unwrap();
        let ext = NormalizedAbsolutePath::new(format!("{WORKSPACE}/ext.bzl")).unwrap();
        let epoch = PathObservationEpoch::new(
            epoch
                .observations()
                .iter()
                .filter(|(demand, _)| demand.path() != &ext)
                .map(|(demand, result)| (demand.dupe(), result.as_ref().clone())),
        )
        .unwrap();
        let mut updater = transaction.into_updater();
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch)])
            .unwrap();
        let mut transaction = updater.commit().await;
        tracker.take();
        let legacy =
            HostSelectedRepositoryFileEffectKey::new(workspace.dupe(), selected_owner.clone(), 0);
        let key = HostSelectedRepositoryFileEffectObservationKey::new(workspace, selected_owner, 0);
        assert!(matches!(
            transaction.compute(&legacy).await.unwrap(),
            SourcePreparationOutcome::Need(_)
        ));
        assert!(matches!(
            transaction.compute(&key).await.unwrap(),
            SourcePreparationOutcome::Need(_)
        ));
        assert!(
            tracker
                .take()
                .iter()
                .filter(|(name, _, _)| name == &legacy.to_string() || name == &key.to_string())
                .all(|(_, _, batch)| batch.is_none())
        );
    }

    #[tokio::test]
    async fn selected_repository_file_effect_executes_selected_ordinal_not_failing_sibling() {
        let extension = r#"
def selected(ctx):
    ctx.file('selected', 'ok')
def sibling(ctx):
    fail('sibling repository rule must not execute')
first=repository_rule(implementation=selected)
second=repository_rule(implementation=sibling)
def impl(ctx):
    first(name='first')
    second(name='second')
ext=module_extension(implementation=impl)
"#;
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let mut transaction = transaction_untracked(&dice, MODULE, extension, true).await;
        let selected_owner = owner(&mut transaction).await;
        let value = transaction
            .compute(&HostSelectedRepositoryFileEffectKey::new(
                workspace,
                selected_owner,
                0,
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(value) = value else {
            panic!("selected ordinal must complete")
        };
        assert_eq!(
            value.as_ref().as_ref().unwrap().plan().effects()[0].path(),
            "selected"
        );
    }

    #[tokio::test]
    async fn selected_repository_file_effect_preserves_semantic_failures_in_observed_carrier() {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let semantic_extension = EXTENSION.replace(
            "ctx.file('BUILD.bazel', content='exports_files([\\\"generated.txt\\\"])\\\\n')",
            "fail('repository-rule semantic failure')",
        );
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut semantic = transaction_untracked(&dice, MODULE, &semantic_extension, true).await;
        let semantic_owner = owner(&mut semantic).await;
        let legacy = semantic
            .compute(&HostSelectedRepositoryFileEffectKey::new(
                workspace.dupe(),
                semantic_owner.clone(),
                0,
            ))
            .await
            .unwrap();
        let observed = semantic
            .compute(&HostSelectedRepositoryFileEffectObservationKey::new(
                workspace.dupe(),
                semantic_owner,
                0,
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(legacy) = legacy else {
            panic!("semantic legacy failure must complete")
        };
        let observed = observed_carrier(&observed);
        assert_eq!(legacy, *observed.result());
        assert!(matches!(
            legacy.as_ref(),
            Err(HostSelectedRepositoryFileEffectError::Invocation { .. })
        ));

        let tracker = Arc::new(EffectTracker::default());
        let event_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut observed_transaction = transaction_with_tracker(
            &event_dice,
            MODULE,
            &semantic_extension,
            true,
            tracker.clone(),
        )
        .await;
        let observed_owner = owner(&mut observed_transaction).await;
        tracker.take();
        let key = HostSelectedRepositoryFileEffectObservationKey::new(workspace, observed_owner, 0);
        let _ = observed_transaction.compute(&key).await.unwrap();
        let batches = tracker
            .take()
            .into_iter()
            .filter(|(name, _, _)| name == &key.to_string())
            .filter_map(|(_, _, batch)| batch)
            .collect::<Vec<_>>();
        assert_eq!(batches.len(), 1);
        assert_eq!(
            batches[0]
                .events()
                .iter()
                .filter_map(|event| match event {
                    EvaluationEvent::StarlarkPrint { text, .. } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            ["repository-file-effect"]
        );
    }

    #[tokio::test]
    async fn selected_repository_file_effect_keys_distinguish_owner_and_ordinal() {
        let module = "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\nf=use_extension('//:ext.bzl','other')\nuse_repo(e, first='first')\nuse_repo(f, second='first')\n";
        let extension = format!("{EXTENSION}\nother=module_extension(implementation=impl)\n");
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let mut transaction = transaction_untracked(&dice, module, &extension, true).await;
        let first = owner(&mut transaction).await;
        let demand = transaction
            .compute(&HostSelectedExtensionDemandKey::new(
                workspace.dupe(),
                CanonicalRepoName::new("+other+first").unwrap(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(demand) = demand else {
            panic!("second selected demand must complete")
        };
        let second = demand.as_ref().as_ref().unwrap().owner().clone();
        assert_ne!(
            HostSelectedRepositoryFileEffectKey::new(workspace.dupe(), first.clone(), 0),
            HostSelectedRepositoryFileEffectKey::new(workspace.dupe(), first.clone(), 1)
        );
        assert_ne!(
            HostSelectedRepositoryFileEffectObservationKey::new(workspace.dupe(), second, 0),
            HostSelectedRepositoryFileEffectObservationKey::new(workspace, first, 0)
        );
    }

    #[test]
    fn selected_repository_file_effect_has_exact_authentication_and_retained_shape() {
        let source = include_str!("module_extension_repository_file_effect.rs");
        let production = &source[..source.find("\n#[cfg(test)]").unwrap()];
        for shape in [
            "let Some(path) = path.unpack_str()",
            "get(&call.definition.exported_name)",
            "downcast::<FrozenRepositoryRuleDefinition>()",
            "if projection != call.definition",
            "Ok(rule.implementation())",
            "SourcePreparationOutcome::Complete(Err(error))",
            "merge_observations(&observations, &child_observations)",
            "HostSelectedRepositoryFileEffectObservationError::Certificate(error)",
            "HostSelectedRepositoryFileEffectObservationError::HostBzl {",
            "HostSelectedRepositoryFileEffectObservationError::Merge {",
        ] {
            assert!(
                production.contains(shape),
                "missing producer shape: {shape}"
            );
        }
        let start = production
            .find("pub struct HostSelectedRepositoryFileEffect {")
            .unwrap();
        let end = production
            .find("\n#[derive(Clone, Copy)]\nenum EffectMode")
            .unwrap();
        let retained = &production[start..end];
        for absent in [
            "FrozenModule",
            "FrozenValue",
            "Heap",
            "Evaluator",
            "RepositoryFileContext",
            "std::fs",
            "I/O",
        ] {
            assert!(
                !retained.contains(absent),
                "retained shape contains {absent}"
            );
        }
    }
}
