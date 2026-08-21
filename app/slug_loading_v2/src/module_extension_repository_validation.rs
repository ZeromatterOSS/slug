/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file.
 */

use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use compact_str::CompactString;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_bzlmod_v2::HostSelectedExtensionDefinitionImport;
use slug_bzlmod_v2::HostSelectedExtensionDefinitionOverride;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathObservationEpoch;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::module_extension_repository_instantiation::HostInstantiatedModuleExtensionRepositories;
use crate::module_extension_repository_instantiation::HostInstantiatedModuleExtensionRepositoriesError;
use crate::module_extension_repository_instantiation::HostInstantiatedModuleExtensionRepositoriesForRequest;
use crate::module_extension_repository_instantiation::HostInstantiatedModuleExtensionRepositoriesKey;
use crate::module_extension_repository_instantiation::HostInstantiatedModuleExtensionRepositoriesObservationError;
use crate::module_extension_repository_instantiation::HostInstantiatedModuleExtensionRepositoriesObservationKey;

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostValidatedGeneratedRepositorySpecs {
    predecessor: Arc<HostInstantiatedModuleExtensionRepositories>,
}
struct GeneratedSpecIter<'a> {
    extensions: &'a [HostInstantiatedModuleExtensionRepositoriesForRequest],
    extension: usize,
    repository: usize,
    remaining: usize,
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct HostGeneratedRepositoryMapping<'a> {
    context_repo: &'a CanonicalRepoName,
    entries: &'a SmallMap<ApparentRepoName, CanonicalRepoName>,
}

impl<'a> HostGeneratedRepositoryMapping<'a> {
    pub fn context_repo(&self) -> &'a CanonicalRepoName {
        self.context_repo
    }

    pub fn entries(&self) -> &'a SmallMap<ApparentRepoName, CanonicalRepoName> {
        self.entries
    }
}

impl HostValidatedGeneratedRepositorySpecs {
    pub fn iter(
        &self,
    ) -> impl ExactSizeIterator<
        Item = (
            &slug_identity_v2::CanonicalRepoName,
            &slug_bzlmod_v2::RepoSpec,
            &str,
            HostGeneratedRepositoryMapping<'_>,
        ),
    > {
        let extensions = self.predecessor.parts().1;
        GeneratedSpecIter {
            extensions,
            extension: 0,
            repository: 0,
            remaining: extensions
                .iter()
                .map(|extension| extension.parts().1.len())
                .sum(),
        }
    }
}

impl<'a> Iterator for GeneratedSpecIter<'a> {
    type Item = (
        &'a slug_identity_v2::CanonicalRepoName,
        &'a slug_bzlmod_v2::RepoSpec,
        &'a str,
        HostGeneratedRepositoryMapping<'a>,
    );

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(extension) = self.extensions.get(self.extension) {
            if let Some(repository) = extension.parts().1.get(self.repository) {
                self.repository += 1;
                self.remaining -= 1;
                let (canonical_name, repo_spec) = repository.spec_parts();
                return Some((
                    canonical_name,
                    repo_spec,
                    repository.generated_name(),
                    HostGeneratedRepositoryMapping {
                        context_repo: canonical_name,
                        entries: extension.mapping_entries().as_ref(),
                    },
                ));
            }
            self.extension += 1;
            self.repository = 0;
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl ExactSizeIterator for GeneratedSpecIter<'_> {}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum PrivateValidationError {
    Instantiation(HostInstantiatedModuleExtensionRepositoriesError),
    InstantiationCompute(CompactString),
    Join {
        predecessor: Arc<HostInstantiatedModuleExtensionRepositories>,
        message: CompactString,
    },
    Validation {
        predecessor: Arc<HostInstantiatedModuleExtensionRepositories>,
        validated: usize,
        current: HostInstantiatedModuleExtensionRepositoriesForRequest,
        offender: HostModuleExtensionValidationOffender,
        error: HostModuleExtensionValidationError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostModuleExtensionValidationOffender {
    Import(HostSelectedExtensionDefinitionImport),
    Override(HostSelectedExtensionDefinitionOverride),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative)]
pub(crate) enum HostModuleExtensionValidationError {
    MissingImport,
    MissingOverride,
    InjectCollision,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostValidatedGeneratedRepositorySpecsError {
    inner: PrivateValidationError,
}

impl fmt::Display for HostValidatedGeneratedRepositorySpecsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.inner)
    }
}

impl std::error::Error for HostValidatedGeneratedRepositorySpecsError {}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostValidatedModuleExtensionRepositoriesKey {
    workspace: NormalizedAbsolutePath,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
#[allow(dead_code)] // Private observed sibling; a later packet owns consumer activation.
struct HostValidatedModuleExtensionRepositoriesObservationKey(
    HostValidatedModuleExtensionRepositoriesKey,
);

#[allow(dead_code)]
impl HostValidatedModuleExtensionRepositoriesObservationKey {
    fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self(HostValidatedModuleExtensionRepositoriesKey::new(workspace))
    }
}

#[rustfmt::skip]
impl fmt::Display for HostValidatedModuleExtensionRepositoriesObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "observed-{}", self.0) }
}

impl HostValidatedModuleExtensionRepositoriesKey {
    pub fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
}

impl fmt::Display for HostValidatedModuleExtensionRepositoriesKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-validated-module-extension-repositories:{}",
            self.workspace
        )
    }
}

#[doc(hidden)]
pub type HostValidatedGeneratedRepositorySpecsOutcome = SourcePreparationOutcome<
    Arc<Result<HostValidatedGeneratedRepositorySpecs, HostValidatedGeneratedRepositorySpecsError>>,
>;

type ValidatedRepositoriesResult =
    Arc<Result<HostValidatedGeneratedRepositorySpecs, HostValidatedGeneratedRepositorySpecsError>>;

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
#[allow(dead_code)] // Retained only by the callerless observed sibling.
struct ObservedHostValidatedGeneratedRepositorySpecs {
    result: ValidatedRepositoriesResult,
    observations: PathObservationEpoch,
}

#[allow(dead_code)]
impl ObservedHostValidatedGeneratedRepositorySpecs {
    fn result(&self) -> &ValidatedRepositoriesResult {
        &self.result
    }

    fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
enum HostValidatedModuleExtensionRepositoriesObservationError {
    Instantiation(HostInstantiatedModuleExtensionRepositoriesObservationError),
}

#[derive(Clone, Copy)]
enum ValidatedRepositoriesMode {
    Legacy,
    Observed,
}

type ValidatedRepositoriesDriverOutcome = SourcePreparationOutcome<
    Result<
        (ValidatedRepositoriesResult, PathObservationEpoch),
        HostValidatedModuleExtensionRepositoriesObservationError,
    >,
>;

fn complete_driver(
    value: Result<
        HostValidatedGeneratedRepositorySpecs,
        HostValidatedGeneratedRepositorySpecsError,
    >,
    observations: PathObservationEpoch,
) -> ValidatedRepositoriesDriverOutcome {
    SourcePreparationOutcome::Complete(Ok((Arc::new(value), observations)))
}

#[rustfmt::skip]
async fn compute_validated_repositories(ctx: &mut DiceComputations<'_>, key: &HostValidatedModuleExtensionRepositoriesKey, mode: ValidatedRepositoriesMode) -> ValidatedRepositoriesDriverOutcome {
    let child = match mode {
        ValidatedRepositoriesMode::Legacy => match ctx.compute(&HostInstantiatedModuleExtensionRepositoriesKey::new(key.workspace.clone())).await {
            Ok(SourcePreparationOutcome::Need(need)) => return SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(value)) => (value, PathObservationEpoch::empty()),
            Err(error) => return complete_driver(Err(HostValidatedGeneratedRepositorySpecsError { inner: PrivateValidationError::InstantiationCompute(error.to_string().into()) }), PathObservationEpoch::empty()),
        },
        ValidatedRepositoriesMode::Observed => match ctx.compute(&HostInstantiatedModuleExtensionRepositoriesObservationKey::new(key.workspace.clone())).await {
            Ok(SourcePreparationOutcome::Need(need)) => return SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(Err(error))) => return SourcePreparationOutcome::Complete(Err(HostValidatedModuleExtensionRepositoriesObservationError::Instantiation(error))),
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => (observed.result().dupe(), observed.observations().dupe()),
            Err(error) => return complete_driver(Err(HostValidatedGeneratedRepositorySpecsError { inner: PrivateValidationError::InstantiationCompute(error.to_string().into()) }), PathObservationEpoch::empty()),
        },
    };
    let (result, observations) = child;
    let predecessor = match result.as_ref() {
        Ok(value) => Arc::new(value.clone()),
        Err(error) => return complete_driver(Err(HostValidatedGeneratedRepositorySpecsError { inner: PrivateValidationError::Instantiation(error.clone()) }), observations),
    };
    complete_driver(validate_repositories(predecessor).map_err(|inner| HostValidatedGeneratedRepositorySpecsError { inner }), observations)
}

#[async_trait]
impl Key for HostValidatedModuleExtensionRepositoriesKey {
    type Value = HostValidatedGeneratedRepositorySpecsOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match compute_validated_repositories(ctx, self, ValidatedRepositoriesMode::Legacy).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                debug_assert!(observations.observations().is_empty());
                SourcePreparationOutcome::Complete(result)
            }
            SourcePreparationOutcome::Complete(Err(_)) => {
                unreachable!("legacy validation has no observed outer")
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
impl Key for HostValidatedModuleExtensionRepositoriesObservationKey {
    type Value = SourcePreparationOutcome<
        Result<
            ObservedHostValidatedGeneratedRepositorySpecs,
            HostValidatedModuleExtensionRepositoriesObservationError,
        >,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match compute_validated_repositories(ctx, &self.0, ValidatedRepositoriesMode::Observed)
            .await
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                SourcePreparationOutcome::Complete(Err(error))
            }
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                SourcePreparationOutcome::Complete(Ok(
                    ObservedHostValidatedGeneratedRepositorySpecs {
                        result,
                        observations,
                    },
                ))
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

fn validate_repositories(
    predecessor: Arc<HostInstantiatedModuleExtensionRepositories>,
) -> Result<HostValidatedGeneratedRepositorySpecs, PrivateValidationError> {
    let (invocations, extensions) = predecessor.parts();
    if invocations.invoked.len() != extensions.len() {
        return Err(PrivateValidationError::Join {
            predecessor,
            message: "invoked and instantiated extension counts differ".into(),
        });
    }

    for (validated, (receipt, current)) in invocations
        .invoked
        .iter()
        .zip(extensions.iter())
        .enumerate()
    {
        let (request, repositories) = current.parts();
        if receipt.request != *request {
            return Err(PrivateValidationError::Join {
                predecessor,
                message: "invoked and instantiated extension requests differ".into(),
            });
        }
        let generated = repositories
            .iter()
            .map(|repository| CompactString::from(repository.generated_name()))
            .collect::<SmallSet<_>>();
        let (imports, overrides) = request.validation_parts();

        for import in imports {
            let (_, exported, _) = import.parts();
            if !generated.contains(exported)
                && !overrides
                    .iter()
                    .any(|override_value| override_value.parts().0 == exported)
            {
                return Err(validation_error(
                    predecessor.clone(),
                    validated,
                    current,
                    HostModuleExtensionValidationOffender::Import(import.clone()),
                    HostModuleExtensionValidationError::MissingImport,
                ));
            }
        }
        for override_value in overrides {
            let (name, _, must_exist) = override_value.parts();
            let exists = generated.contains(name);
            let error = if must_exist && !exists {
                Some(HostModuleExtensionValidationError::MissingOverride)
            } else if !must_exist && exists {
                Some(HostModuleExtensionValidationError::InjectCollision)
            } else {
                None
            };
            if let Some(error) = error {
                return Err(validation_error(
                    predecessor.clone(),
                    validated,
                    current,
                    HostModuleExtensionValidationOffender::Override(override_value.clone()),
                    error,
                ));
            }
        }
    }

    Ok(HostValidatedGeneratedRepositorySpecs { predecessor })
}

fn validation_error(
    predecessor: Arc<HostInstantiatedModuleExtensionRepositories>,
    validated: usize,
    current: &HostInstantiatedModuleExtensionRepositoriesForRequest,
    offender: HostModuleExtensionValidationOffender,
    error: HostModuleExtensionValidationError,
) -> PrivateValidationError {
    PrivateValidationError::Validation {
        predecessor,
        validated,
        current: current.clone(),
        offender,
        error,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hash;
    use std::hash::Hasher;
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
    use dice::UserComputationData;
    use slug_bzlmod_v2::OverrideAttributeValue;
    use slug_bzlmod_v2::RegistryFileKey;
    use slug_bzlmod_v2::RepositoryMaterializationKey;
    use slug_bzlmod_v2::SourcePreparationOutcome;
    use slug_events_v2::EvaluationEvent;
    use slug_events_v2::EventBatch;
    use slug_workspace_v2::NormalizedAbsolutePath;
    use slug_workspace_v2::PathObservationEpoch;
    use slug_workspace_v2::PathObservationEpochKey;

    use super::*;
    use crate::module_extension_repository_instantiation::HostInstantiatedModuleExtensionRepositoriesObservationError;
    use crate::module_extension_repository_instantiation::HostInstantiatedModuleExtensionRepositoriesObservationKey;
    use crate::module_extension_repository_instantiation::ObservedHostInstantiatedModuleExtensionRepositories;
    use crate::module_extension_repository_instantiation::tests::WORKSPACE;
    use crate::module_extension_repository_instantiation::tests::transaction_untracked;
    use crate::module_extension_repository_instantiation::tests::transaction_with_tracker;
    const EXTENSION: &str = r#"
repo=repository_rule(lambda ctx: None)
def impl(ctx):
    repo(name='first')
    repo(name='second')
ext=module_extension(implementation=impl)
"#;

    #[test]
    #[rustfmt::skip]
    fn instantiation_observation_surface_is_validation_sibling_usable() {
        let key = HostInstantiatedModuleExtensionRepositoriesObservationKey::new(NormalizedAbsolutePath::new("/workspace").unwrap());
        assert_eq!(key.to_string(), "observed-host-instantiated-module-extension-repositories:\"/workspace\"");

        fn inspect(_value: &<HostInstantiatedModuleExtensionRepositoriesObservationKey as Key>::Value, observed: &ObservedHostInstantiatedModuleExtensionRepositories, _error: &HostInstantiatedModuleExtensionRepositoriesObservationError) {
            let _: &Arc<Result<HostInstantiatedModuleExtensionRepositories, HostInstantiatedModuleExtensionRepositoriesError>> = observed.result();
            let _: &PathObservationEpoch = observed.observations();
        }

        let _ = inspect as fn(&SourcePreparationOutcome<Result<ObservedHostInstantiatedModuleExtensionRepositories, HostInstantiatedModuleExtensionRepositoriesObservationError>>, &ObservedHostInstantiatedModuleExtensionRepositories, &HostInstantiatedModuleExtensionRepositoriesObservationError);
    }
    #[derive(Default)]
    struct ValidationTracker {
        validation: Mutex<Vec<(ActivationKind, bool)>>,
        forbidden: Mutex<Vec<&'static str>>,
        activations: Mutex<Vec<(String, ActivationKind, Option<EventBatch>)>>,
        dependencies: Mutex<Vec<(String, Vec<String>)>>,
    }

    impl ActivationTracker for ValidationTracker {
        fn key_activated(
            &self,
            key: &DynKey,
            dependencies: &mut dyn Iterator<Item = &DynKey>,
            _: ActivationData,
        ) {
            self.dependencies.lock().unwrap().push((
                key.to_string(),
                dependencies.map(ToString::to_string).collect(),
            ));
        }

        fn tracks_rich_activations(&self) -> bool {
            true
        }

        fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
            let batch = activation
                .evaluation_data()
                .and_then(|data| data.downcast_ref::<EventBatch>())
                .map(Dupe::dupe);
            self.activations.lock().unwrap().push((
                key.to_string(),
                activation.kind(),
                batch.clone(),
            ));
            if key
                .downcast_ref::<HostValidatedModuleExtensionRepositoriesKey>()
                .is_some()
            {
                self.validation
                    .lock()
                    .unwrap()
                    .push((activation.kind(), batch.is_some()));
            } else if key.downcast_ref::<RegistryFileKey>().is_some() {
                self.forbidden.lock().unwrap().push("registry");
            } else if key.downcast_ref::<RepositoryMaterializationKey>().is_some() {
                self.forbidden.lock().unwrap().push("materialization");
            }
        }
    }

    async fn compute(
        dice: &Arc<Dice>,
        module: &str,
    ) -> HostValidatedGeneratedRepositorySpecsOutcome {
        transaction_untracked(dice, module, EXTENSION, true)
            .await
            .compute(&HostValidatedModuleExtensionRepositoriesKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            ))
            .await
            .unwrap()
    }
    async fn compute_with_extension(
        dice: &Arc<Dice>,
        module: &str,
        extension: &str,
    ) -> HostValidatedGeneratedRepositorySpecsOutcome {
        transaction_untracked(dice, module, extension, true)
            .await
            .compute(&HostValidatedModuleExtensionRepositoriesKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            ))
            .await
            .unwrap()
    }

    async fn compute_observed(
        dice: &Arc<Dice>,
        module: &str,
        extension: &str,
        present: bool,
        tracker: Option<Arc<ValidationTracker>>,
    ) -> <HostValidatedModuleExtensionRepositoriesObservationKey as Key>::Value {
        let mut transaction = match tracker {
            Some(tracker) => {
                transaction_with_tracker(dice, module, extension, present, tracker).await
            }
            None => transaction_untracked(dice, module, extension, present).await,
        };
        transaction
            .compute(
                &HostValidatedModuleExtensionRepositoriesObservationKey::new(
                    NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                ),
            )
            .await
            .unwrap()
    }

    fn observed_carrier(
        value: &<HostValidatedModuleExtensionRepositoriesObservationKey as Key>::Value,
    ) -> &ObservedHostValidatedGeneratedRepositorySpecs {
        match value {
            SourcePreparationOutcome::Complete(Ok(value)) => value,
            value => panic!("expected observed validation carrier: {value:?}"),
        }
    }

    fn module(imports: &str, directive: &str) -> String {
        format!(
            "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\n{imports}\n{directive}\n"
        )
    }
    fn external_style_view(
        value: &crate::HostValidatedGeneratedRepositorySpecs,
    ) -> Vec<(String, String)> {
        let rows = value.iter();
        assert_eq!(rows.len(), rows.size_hint().0);
        rows.map(|(name, spec, _, _)| {
            (name.as_str().to_owned(), spec.rule_id.rule_name.to_string())
        })
        .collect()
    }

    #[tokio::test]
    async fn real_validation_orders_imports_before_polarity_and_restores() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let valid = module("use_repo(e, one='first', two='second')", "");
        let a = compute(&dice, &valid).await;
        let warm = compute(&dice, &valid).await;
        assert!(HostValidatedModuleExtensionRepositoriesKey::equality(
            &a, &warm
        ));
        assert!(HostValidatedModuleExtensionRepositoriesKey::validity(&a));
        assert!(matches!(a, SourcePreparationOutcome::Complete(ref value) if value.is_ok()));
        let SourcePreparationOutcome::Complete(value) = &a else {
            panic!("validation must complete")
        };
        let rows = external_style_view(value.as_ref().as_ref().unwrap());
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|(_, rule)| rule == "repo"));
        assert!(rows[0].0.ends_with("+first"));
        assert!(rows[1].0.ends_with("+second"));

        let missing = compute(
            &dice,
            &module(
                "use_repo(e, bad='missing')",
                "override_repo(e, absent='bazel_tools')",
            ),
        )
        .await;
        assert!(matches!(
            missing,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostValidatedGeneratedRepositorySpecsError { inner: PrivateValidationError::Validation {
                        error: HostModuleExtensionValidationError::MissingImport,
                        ..
                    } })
                )
        ));

        let override_backed = compute(
            &dice,
            &module(
                "use_repo(e, virtual_alias='virtual')",
                "override_repo(e, virtual='bazel_tools')",
            ),
        )
        .await;
        assert!(matches!(
            override_backed,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostValidatedGeneratedRepositorySpecsError { inner: PrivateValidationError::Validation {
                        error: HostModuleExtensionValidationError::MissingOverride,
                        ..
                    } })
                )
        ));

        let missing_override =
            compute(&dice, &module("", "override_repo(e, absent='bazel_tools')")).await;
        assert!(matches!(
            missing_override,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostValidatedGeneratedRepositorySpecsError { inner: PrivateValidationError::Validation {
                        error: HostModuleExtensionValidationError::MissingOverride,
                        ..
                    } })
                )
        ));

        let inject_collision =
            compute(&dice, &module("", "inject_repo(e, first='bazel_tools')")).await;
        assert!(matches!(
            inject_collision,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostValidatedGeneratedRepositorySpecsError { inner: PrivateValidationError::Validation {
                        error: HostModuleExtensionValidationError::InjectCollision,
                        ..
                    } })
                )
        ));

        let restored = compute(&dice, &valid).await;
        assert!(HostValidatedModuleExtensionRepositoriesKey::equality(
            &a, &restored
        ));
    }

    #[tokio::test]
    async fn public_view_retains_overridden_rows_full_specs_and_request_order() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let module = |swap: bool, generated: &str, injected: &str, target: &str, swap_ops: bool| {
            let alpha = format!(
                "a=use_extension('//:ext.bzl','alpha')\nuse_repo(a, first_alias='first')\noverride_repo(a, {generated}='root_alias')\n{}{}",
                if swap_ops {
                    "inject_repo(a, other='root_alias')\n".to_owned()
                } else {
                    format!("inject_repo(a, {injected}='{target}')\n")
                },
                if swap_ops {
                    format!("inject_repo(a, {injected}='{target}')\n")
                } else {
                    "inject_repo(a, other='root_alias')\n".to_owned()
                },
            );
            let beta = "b=use_extension('//:ext.bzl','beta')\n";
            format!(
                "module(name='bazel_tools', repo_name='root_alias')\n{}{}",
                if swap { beta } else { &alpha },
                if swap { &alpha } else { beta },
            )
        };
        let source = |rule: &str, attr: &str, value: &str, target: &str, target_first: bool| {
            let kwargs = if target_first {
                format!("target='{target}', {attr}='{value}'")
            } else {
                format!("{attr}='{value}', target='{target}'")
            };
            format!(
                r#"alpha_rule=repository_rule(lambda ctx: None, attrs={{'{attr}':attr.string(),'target':attr.label()}})
beta_rule=repository_rule(lambda ctx: None, attrs={{'count':attr.int(),'target':attr.label()}})
def alpha_impl(ctx):
    {rule}(name='first', {kwargs})
    {rule}(name='second', {attr}='two', target='@first//:item')
def beta_impl(ctx):
    beta_rule(name='third', count=3, target='@third//:item')
alpha=module_extension(implementation=alpha_impl)
beta=module_extension(implementation=beta_impl)
"#
            )
        };
        let baseline_module = module(false, "first", "injected", "root_alias", false);
        let baseline_source = source("alpha_rule", "text", "one", "@second//:item", false);
        let a = compute_with_extension(&dice, &baseline_module, &baseline_source).await;
        let SourcePreparationOutcome::Complete(value) = &a else {
            panic!("publication must complete")
        };
        let rows = value.as_ref().as_ref().unwrap().iter().collect::<Vec<_>>();
        assert_eq!(rows.len(), 3);
        assert!(rows[0].0.as_str().ends_with("+first"));
        assert!(rows[1].0.as_str().ends_with("+second"));
        assert!(rows[2].0.as_str().ends_with("+third"));
        assert_eq!(rows[0].2, "first");
        assert_eq!(rows[1].2, "second");
        assert_eq!(rows[2].2, "third");
        assert_eq!(rows[0].3.context_repo(), rows[0].0);
        assert_eq!(rows[1].3.context_repo(), rows[1].0);
        assert_eq!(rows[2].3.context_repo(), rows[2].0);
        assert!(std::ptr::eq(rows[0].3.entries(), rows[1].3.entries()));
        assert!(!std::ptr::eq(rows[1].3.entries(), rows[2].3.entries()));
        let first = ApparentRepoName::new("first").unwrap();
        let second = ApparentRepoName::new("second").unwrap();
        let third = ApparentRepoName::new("third").unwrap();
        let root_alias = ApparentRepoName::new("root_alias").unwrap();
        let injected = ApparentRepoName::new("injected").unwrap();
        let other = ApparentRepoName::new("other").unwrap();
        assert_eq!(
            rows[0].3.entries().get(&root_alias),
            Some(&CanonicalRepoName::root())
        );
        assert_eq!(
            rows[0].3.entries().get(&first),
            Some(&CanonicalRepoName::root())
        );
        assert_eq!(rows[0].3.entries().get(&second), Some(rows[1].0));
        assert_eq!(rows[2].3.entries().get(&third), Some(rows[2].0));
        assert_eq!(
            rows[0].3.entries().get(&injected),
            Some(&CanonicalRepoName::root())
        );
        assert_eq!(
            rows[0].3.entries().get(&other),
            Some(&CanonicalRepoName::root())
        );
        let alpha_names = rows[0]
            .3
            .entries()
            .keys()
            .filter_map(|name| {
                matches!(
                    name.as_str(),
                    "root_alias" | "first" | "second" | "injected" | "other"
                )
                .then_some(name.as_str())
            })
            .collect::<Vec<_>>();
        assert_eq!(
            alpha_names,
            ["root_alias", "first", "second", "injected", "other"]
        );
        let swapped_mapping = compute_with_extension(
            &dice,
            &module(false, "first", "injected", "root_alias", true),
            &baseline_source,
        )
        .await;
        let SourcePreparationOutcome::Complete(swapped_value) = &swapped_mapping else {
            panic!("swapped mapping must complete")
        };
        let swapped_rows = swapped_value
            .as_ref()
            .as_ref()
            .unwrap()
            .iter()
            .collect::<Vec<_>>();
        assert_eq!(
            swapped_rows[0]
                .3
                .entries()
                .keys()
                .filter_map(|name| {
                    matches!(name.as_str(), "injected" | "other").then_some(name.as_str())
                })
                .collect::<Vec<_>>(),
            ["other", "injected"]
        );
        assert!(!HostValidatedModuleExtensionRepositoriesKey::equality(
            &a,
            &swapped_mapping
        ));
        let restored_mapping =
            compute_with_extension(&dice, &baseline_module, &baseline_source).await;
        assert!(HostValidatedModuleExtensionRepositoriesKey::equality(
            &a,
            &restored_mapping
        ));
        assert_eq!(rows[0].1.rule_id.rule_name, "alpha_rule");
        assert_eq!(rows[2].1.rule_id.rule_name, "beta_rule");
        assert!(
            rows[0]
                .1
                .rule_id
                .bzl_file
                .to_string()
                .ends_with("//:ext.bzl")
        );
        assert_eq!(
            rows[0]
                .1
                .attributes
                .keys()
                .map(|name| name.as_str())
                .collect::<Vec<_>>(),
            ["text", "target"]
        );
        assert_eq!(
            rows[0].1.attributes.get("text"),
            Some(&OverrideAttributeValue::String("one".into()))
        );
        assert!(matches!(
            rows[0].1.attributes.get("target"),
            Some(OverrideAttributeValue::Label(label))
                if label.to_string().contains("+second//:item")
        ));

        let variants = [
            (
                module(false, "renamed", "injected", "root_alias", false),
                source("alpha_rule", "text", "one", "@second//:item", false)
                    .replace("name='first'", "name='renamed'"),
            ),
            (
                baseline_module.clone(),
                source("renamed_rule", "text", "one", "@second//:item", false)
                    .replace("alpha_rule=", "renamed_rule="),
            ),
            (
                baseline_module.clone(),
                source("alpha_rule", "message", "one", "@second//:item", false),
            ),
            (
                baseline_module.clone(),
                source("alpha_rule", "text", "changed", "@second//:item", false),
            ),
            (
                baseline_module.clone(),
                source("alpha_rule", "text", "one", "@second//:item", true),
            ),
            (
                baseline_module.clone(),
                source("alpha_rule", "text", "one", "@first//:item", false),
            ),
            (
                module(true, "first", "injected", "root_alias", false),
                baseline_source.clone(),
            ),
            (
                module(false, "first", "other", "root_alias", false),
                baseline_source.clone(),
            ),
            (
                module(false, "first", "injected", "first_alias", false),
                baseline_source.clone(),
            ),
        ];
        for (variant_module, variant_source) in variants {
            let changed = compute_with_extension(&dice, &variant_module, &variant_source).await;
            assert!(!HostValidatedModuleExtensionRepositoriesKey::equality(
                &a, &changed
            ));
            let restored = compute_with_extension(&dice, &baseline_module, &baseline_source).await;
            assert!(HostValidatedModuleExtensionRepositoriesKey::equality(
                &a, &restored
            ));
        }
    }
    #[tokio::test]
    async fn success_boundaries_and_structural_identity_restore() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));

        let alias_a = compute(&dice, &module("use_repo(e, local='first')", "")).await;
        let alias_b = compute(&dice, &module("use_repo(e, renamed='first')", "")).await;
        assert!(matches!(
            alias_a,
            SourcePreparationOutcome::Complete(ref value) if value.is_ok()
        ));
        assert!(!HostValidatedModuleExtensionRepositoriesKey::equality(
            &alias_a, &alias_b
        ));
        let alias_restored = compute(&dice, &module("use_repo(e, local='first')", "")).await;
        assert!(HostValidatedModuleExtensionRepositoriesKey::equality(
            &alias_a,
            &alias_restored
        ));

        let override_present =
            compute(&dice, &module("", "override_repo(e, first='bazel_tools')")).await;
        let inject_absent =
            compute(&dice, &module("", "inject_repo(e, absent='bazel_tools')")).await;
        assert!(matches!(
            override_present,
            SourcePreparationOutcome::Complete(ref value) if value.is_ok()
        ));
        assert!(matches!(
            inject_absent,
            SourcePreparationOutcome::Complete(ref value) if value.is_ok()
        ));

        let missing_a = compute(
            &dice,
            &module("use_repo(e, a='missing_a', b='missing_b')", ""),
        )
        .await;
        let missing_b = compute(
            &dice,
            &module("use_repo(e, b='missing_b', a='missing_a')", ""),
        )
        .await;
        for (value, expected) in [(&missing_a, "a"), (&missing_b, "b")] {
            assert!(matches!(
                value,
                SourcePreparationOutcome::Complete(value)
                    if matches!(
                        value.as_ref(),
                        Err(HostValidatedGeneratedRepositorySpecsError { inner: PrivateValidationError::Validation {
                            offender: HostModuleExtensionValidationOffender::Import(import),
                            error: HostModuleExtensionValidationError::MissingImport,
                            ..
                        } }) if import.parts().0 == expected
                    )
            ));
        }
        assert!(!HostValidatedModuleExtensionRepositoriesKey::equality(
            &missing_a, &missing_b
        ));
        let missing_restored = compute(
            &dice,
            &module("use_repo(e, a='missing_a', b='missing_b')", ""),
        )
        .await;
        assert!(HostValidatedModuleExtensionRepositoriesKey::equality(
            &missing_a,
            &missing_restored
        ));

        let target_a = compute(&dice, &module("", "override_repo(e, absent='bazel_tools')")).await;
        let target_b = compute(&dice, &module("", "override_repo(e, absent='platforms')")).await;
        assert!(!HostValidatedModuleExtensionRepositoriesKey::equality(
            &target_a, &target_b
        ));
        let target_restored =
            compute(&dice, &module("", "override_repo(e, absent='bazel_tools')")).await;
        assert!(HostValidatedModuleExtensionRepositoriesKey::equality(
            &target_a,
            &target_restored
        ));

        let empty_extension =
            "def impl(ctx):\n    pass\next=module_extension(implementation=impl)\n";
        let empty = compute_with_extension(&dice, &module("", ""), empty_extension).await;
        assert!(matches!(
            empty,
            SourcePreparationOutcome::Complete(ref value) if value.is_ok()
        ));
        let one_call_extension = "repo=repository_rule(lambda ctx: None)\ndef impl(ctx):\n    repo(name='first')\next=module_extension(implementation=impl)\n";
        let one_call = compute_with_extension(
            &dice,
            &module("use_repo(e, local='first')", ""),
            one_call_extension,
        )
        .await;
        assert!(!HostValidatedModuleExtensionRepositoriesKey::equality(
            &alias_a, &one_call
        ));
        let calls_restored = compute(&dice, &module("use_repo(e, local='first')", "")).await;
        assert!(HostValidatedModuleExtensionRepositoriesKey::equality(
            &alias_a,
            &calls_restored
        ));
    }
    #[tokio::test]
    async fn offender_fields_locations_order_and_polarity_restore() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));

        let import_a = compute(&dice, &module("use_repo(e, local='missing_a')", "")).await;
        let import_b = compute(&dice, &module("use_repo(e, local='missing_b')", "")).await;
        let import_location = compute(
            &dice,
            "module(name='bazel_tools')\ne = use_extension('//:ext.bzl','ext')\nuse_repo(e, local='missing_a')\n",
        )
        .await;
        for (value, exported, column) in [
            (&import_a, "missing_a", 16),
            (&import_b, "missing_b", 16),
            (&import_location, "missing_a", 18),
        ] {
            assert!(matches!(
                value,
                SourcePreparationOutcome::Complete(value)
                    if matches!(
                        value.as_ref(),
                        Err(HostValidatedGeneratedRepositorySpecsError { inner: PrivateValidationError::Validation {
                            validated: 0,
                            offender: HostModuleExtensionValidationOffender::Import(import),
                            error: HostModuleExtensionValidationError::MissingImport,
                            ..
                        } }) if import.parts().0 == "local"
                            && import.parts().1 == exported
                            && import.parts().2.start_column == column
                    )
            ));
        }
        assert!(!HostValidatedModuleExtensionRepositoriesKey::equality(
            &import_a, &import_b
        ));
        assert!(!HostValidatedModuleExtensionRepositoriesKey::equality(
            &import_a,
            &import_location
        ));
        let import_restored = compute(&dice, &module("use_repo(e, local='missing_a')", "")).await;
        assert!(HostValidatedModuleExtensionRepositoriesKey::equality(
            &import_a,
            &import_restored
        ));

        let override_a =
            compute(&dice, &module("", "override_repo(e, absent='bazel_tools')")).await;
        let override_b = compute(&dice, &module("", "override_repo(e, other='bazel_tools')")).await;
        let order_a = compute(
            &dice,
            &module(
                "",
                "override_repo(e, other='bazel_tools')\noverride_repo(e, absent='bazel_tools')",
            ),
        )
        .await;
        let order_b = compute(
            &dice,
            &module(
                "",
                "override_repo(e, absent='bazel_tools')\noverride_repo(e, other='bazel_tools')",
            ),
        )
        .await;
        let override_location = compute(
            &dice,
            &module("", "\noverride_repo(e, absent='bazel_tools')"),
        )
        .await;
        for (value, name, line) in [
            (&override_a, "absent", 4),
            (&override_b, "other", 4),
            (&order_a, "other", 4),
            (&order_b, "absent", 4),
            (&override_location, "absent", 5),
        ] {
            assert!(matches!(
                value,
                SourcePreparationOutcome::Complete(value)
                    if matches!(
                        value.as_ref(),
                        Err(HostValidatedGeneratedRepositorySpecsError { inner: PrivateValidationError::Validation {
                            validated: 0,
                            offender: HostModuleExtensionValidationOffender::Override(row),
                            error: HostModuleExtensionValidationError::MissingOverride,
                            ..
                        } }) if row.parts().0 == name
                            && row.parts().2
                            && row.location().start_line == line
                    )
            ));
        }
        assert!(!HostValidatedModuleExtensionRepositoriesKey::equality(
            &override_a,
            &override_b
        ));
        assert!(!HostValidatedModuleExtensionRepositoriesKey::equality(
            &order_a, &order_b
        ));
        assert!(!HostValidatedModuleExtensionRepositoriesKey::equality(
            &override_a,
            &override_location
        ));

        let polarity_a = compute(&dice, &module("", "override_repo(e, first='bazel_tools')")).await;
        let polarity_b = compute(&dice, &module("", "inject_repo(e, first='bazel_tools')")).await;
        assert!(matches!(
            polarity_a,
            SourcePreparationOutcome::Complete(ref value) if value.is_ok()
        ));
        assert!(matches!(
            polarity_b,
            SourcePreparationOutcome::Complete(ref value)
                if matches!(
                    value.as_ref(),
                    Err(HostValidatedGeneratedRepositorySpecsError { inner: PrivateValidationError::Validation {
                        offender: HostModuleExtensionValidationOffender::Override(row),
                        error: HostModuleExtensionValidationError::InjectCollision,
                        ..
                    } }) if row.parts().0 == "first" && !row.parts().2
                )
        ));
        assert!(!HostValidatedModuleExtensionRepositoriesKey::equality(
            &polarity_a,
            &polarity_b
        ));
        let polarity_restored =
            compute(&dice, &module("", "override_repo(e, first='bazel_tools')")).await;
        assert!(HostValidatedModuleExtensionRepositoriesKey::equality(
            &polarity_a,
            &polarity_restored
        ));
        let override_restored =
            compute(&dice, &module("", "override_repo(e, absent='bazel_tools')")).await;
        assert!(HostValidatedModuleExtensionRepositoriesKey::equality(
            &override_a,
            &override_restored
        ));
        let order_restored = compute(
            &dice,
            &module(
                "",
                "override_repo(e, other='bazel_tools')\noverride_repo(e, absent='bazel_tools')",
            ),
        )
        .await;
        assert!(HostValidatedModuleExtensionRepositoriesKey::equality(
            &order_a,
            &order_restored
        ));
    }
    #[tokio::test]
    async fn real_validation_reuses_without_events_and_retains_prefix_context() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(ValidationTracker::default());
        let extension = r#"
repo=repository_rule(lambda ctx: None)
def impl(ctx):
    repo(name='generated')
first=module_extension(implementation=impl)
second=module_extension(implementation=impl)
"#;
        let module = "module(name='bazel_tools')\na=use_extension('//:ext.bzl','first')\nuse_repo(a, ok='generated')\nb=use_extension('//:ext.bzl','second')\nuse_repo(b, bad='missing')\n";
        let mut transaction =
            transaction_with_tracker(&dice, module, extension, true, tracker.clone()).await;
        let key = HostValidatedModuleExtensionRepositoriesKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
        );
        let error = transaction.compute(&key).await.unwrap();
        let SourcePreparationOutcome::Complete(value) = &error else {
            panic!("validation must complete")
        };
        assert!(matches!(
            value.as_ref(),
            Err(HostValidatedGeneratedRepositorySpecsError { inner: PrivateValidationError::Validation {
                validated: 1,
                offender: HostModuleExtensionValidationOffender::Import(import),
                error: HostModuleExtensionValidationError::MissingImport,
                ..
            } }) if import.parts().0 == "bad" && import.parts().1 == "missing"
        ));
        let reused = transaction.compute(&key).await.unwrap();
        assert!(HostValidatedModuleExtensionRepositoriesKey::equality(
            &error, &reused
        ));
        assert_eq!(
            *tracker.validation.lock().unwrap(),
            [
                (ActivationKind::Evaluated, false),
                (ActivationKind::Reused, false),
            ]
        );
        assert!(tracker.forbidden.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn corrupted_count_and_full_request_joins_fail_closed() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let extension = r#"
repo=repository_rule(lambda ctx: None)
def impl(ctx):
    repo(name='generated')
first=module_extension(implementation=impl)
second=module_extension(implementation=impl)
"#;
        let module = "module(name='bazel_tools')\na=use_extension('//:ext.bzl','first')\nuse_repo(a, one='generated')\nb=use_extension('//:ext.bzl','second')\nuse_repo(b, two='generated')\n";
        let mut transaction = transaction_untracked(&dice, module, extension, true).await;
        let value = transaction
            .compute(&HostValidatedModuleExtensionRepositoriesKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(value) = value else {
            panic!("validation must complete")
        };
        let predecessor = value.as_ref().as_ref().unwrap().predecessor.clone();

        let count =
            validate_repositories(Arc::new(predecessor.with_truncated_extensions_for_test()));
        assert!(matches!(
            count,
            Err(PrivateValidationError::Join { ref message, .. })
                if message.contains("counts differ")
        ));

        let request = validate_repositories(Arc::new(predecessor.with_swapped_requests_for_test()));
        assert!(matches!(
            request,
            Err(PrivateValidationError::Join { ref message, .. })
                if message.contains("requests differ")
        ));
    }
    #[tokio::test]
    async fn predecessor_need_is_invalid_and_self_unequal() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let _initial = transaction_untracked(
            &dice,
            "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\n",
            EXTENSION,
            true,
        )
        .await;
        let mut updater = dice.updater_with_data(UserComputationData {
            cycle_detector: Some(crate::cycle_detector::bzl_load_cycle_detector()),
            ..Default::default()
        });
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                PathObservationEpoch::new([]).unwrap(),
            )])
            .unwrap();
        let mut transaction = updater.commit().await;
        let value = transaction
            .compute(&HostValidatedModuleExtensionRepositoriesKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            ))
            .await
            .unwrap();
        assert!(!HostValidatedModuleExtensionRepositoriesKey::validity(
            &value
        ));
        assert!(!HostValidatedModuleExtensionRepositoriesKey::equality(
            &value, &value
        ));
        assert!(matches!(value, SourcePreparationOutcome::Need(_)));
    }

    #[tokio::test]
    async fn predecessor_error_is_terminal() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut transaction = transaction_untracked(
            &dice,
            "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\n",
            EXTENSION,
            false,
        )
        .await;
        let value = transaction
            .compute(&HostValidatedModuleExtensionRepositoriesKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            ))
            .await
            .unwrap();
        assert!(HostValidatedModuleExtensionRepositoriesKey::validity(
            &value
        ));
        assert!(matches!(
            value,
            SourcePreparationOutcome::Complete(result)
                if matches!(
                    result.as_ref(),
                    Err(HostValidatedGeneratedRepositorySpecsError { inner: PrivateValidationError::Instantiation(_) })
                )
        ));
    }

    #[tokio::test]
    async fn observed_validation_identity_finisher_and_terminal_algebra() {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let key = HostValidatedModuleExtensionRepositoriesObservationKey::new(workspace.clone());
        let same = HostValidatedModuleExtensionRepositoriesObservationKey::new(workspace.clone());
        let other = HostValidatedModuleExtensionRepositoriesObservationKey::new(
            NormalizedAbsolutePath::new("/other").unwrap(),
        );
        let hash = |key: &HostValidatedModuleExtensionRepositoriesObservationKey| {
            let mut hasher = DefaultHasher::new();
            key.hash(&mut hasher);
            hasher.finish()
        };
        assert_eq!(
            key.to_string(),
            "observed-host-validated-module-extension-repositories:\"/module-extension-repository-instantiation\""
        );
        assert_eq!(key, same);
        assert_ne!(key, other);
        assert_eq!(hash(&key), hash(&same));
        assert_ne!(hash(&key), hash(&other));

        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let success_module = module("use_repo(e, one='first', two='second')", "");
        let success = compute_observed(&dice, &success_module, EXTENSION, true, None).await;
        let carrier = observed_carrier(&success);
        assert!(carrier.result().is_ok());
        assert!(!carrier.observations().observations().is_empty());
        assert!(HostValidatedModuleExtensionRepositoriesObservationKey::validity(&success));
        assert!(
            HostValidatedModuleExtensionRepositoriesObservationKey::equality(&success, &success)
        );

        for (module, extension, present) in [
            (success_module.clone(), EXTENSION, true),
            (module("use_repo(e, bad='missing')", ""), EXTENSION, true),
            (
                module("", "override_repo(e, absent='bazel_tools')"),
                EXTENSION,
                true,
            ),
            (
                module("", "inject_repo(e, first='bazel_tools')"),
                EXTENSION,
                true,
            ),
            (success_module.clone(), EXTENSION, false),
        ] {
            let case_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let mut legacy_tx =
                transaction_untracked(&case_dice, &module, extension, present).await;
            let legacy = legacy_tx
                .compute(&HostValidatedModuleExtensionRepositoriesKey::new(
                    workspace.clone(),
                ))
                .await
                .unwrap();
            let observed = compute_observed(&case_dice, &module, extension, present, None).await;
            let SourcePreparationOutcome::Complete(legacy) = legacy else {
                panic!("legacy validation must complete")
            };
            assert_eq!(
                legacy.as_ref(),
                observed_carrier(&observed).result().as_ref()
            );
        }
        assert!(matches!(
            observed_carrier(
                &compute_observed(
                    &dice,
                    &module("use_repo(e, bad='missing')", ""),
                    EXTENSION,
                    true,
                    None,
                )
                .await
            )
            .result()
            .as_ref(),
            Err(HostValidatedGeneratedRepositorySpecsError {
                inner: PrivateValidationError::Validation {
                    validated: 0,
                    offender: HostModuleExtensionValidationOffender::Import(_),
                    error: HostModuleExtensionValidationError::MissingImport,
                    ..
                }
            })
        ));

        let join_module = "module(name='bazel_tools')\na=use_extension('//:ext.bzl','first')\nb=use_extension('//:ext.bzl','second')\n";
        let join_source = r#"repo=repository_rule(lambda ctx: None)
def impl(ctx):
    repo(name='generated')
first=module_extension(implementation=impl)
second=module_extension(implementation=impl)
"#;
        let join = compute_observed(&dice, join_module, join_source, true, None).await;
        let predecessor = observed_carrier(&join)
            .result()
            .as_ref()
            .as_ref()
            .unwrap()
            .predecessor
            .clone();
        assert!(matches!(
            validate_repositories(Arc::new(predecessor.with_truncated_extensions_for_test())),
            Err(PrivateValidationError::Join { predecessor, ref message })
                if predecessor.parts().0.invoked.len() == 2 && message.contains("counts differ")
        ));
        assert!(matches!(
            validate_repositories(Arc::new(predecessor.with_swapped_requests_for_test())),
            Err(PrivateValidationError::Join { predecessor, ref message })
                if predecessor.parts().0.invoked.len() == 2 && message.contains("requests differ")
        ));

        let tracker = Arc::new(ValidationTracker::default());
        let need_module = "module(name='bazel_tools')\nbazel_dep(name='dep',version='1.0')\nlocal_path_override(module_name='dep',path='dep')\ne=use_extension('//:ext.bzl','ext')\n";
        let need = compute_observed(
            &Dice::builder().build(DetectCycles::Enabled),
            need_module,
            EXTENSION,
            true,
            Some(tracker.clone()),
        )
        .await;
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        assert!(!HostValidatedModuleExtensionRepositoriesObservationKey::validity(&need));
        assert!(!HostValidatedModuleExtensionRepositoriesObservationKey::equality(&need, &need));
        assert_eq!(
            tracker
                .dependencies
                .lock()
                .unwrap()
                .iter()
                .find(|(name, _)| name == &key.to_string())
                .unwrap()
                .1,
            [
                HostInstantiatedModuleExtensionRepositoriesObservationKey::new(workspace)
                    .to_string()
            ]
        );
        assert!(
            tracker
                .activations
                .lock()
                .unwrap()
                .iter()
                .filter(|(name, _, _)| name == &key.to_string())
                .all(|(_, _, batch)| batch.is_none())
        );

        let source = include_str!("module_extension_repository_validation.rs");
        let producer = &source[source.find("type ValidatedRepositoriesResult").unwrap()
            ..source.find("fn validate_repositories(").unwrap()];
        assert_eq!(
            producer
                .matches("HostInstantiatedModuleExtensionRepositoriesObservationKey::new")
                .count(),
            1
        );
        assert!(producer.contains(
            "HostValidatedModuleExtensionRepositoriesObservationError::Instantiation(error)"
        ));
        assert!(!producer.contains("union_"));
        assert!(!producer.contains("store_evaluation_data"));
    }

    #[tokio::test]
    async fn observed_validation_real_order_events_and_parity() {
        let extension = r#"print('load')
repo=repository_rule(lambda ctx: None)
def first_impl(ctx):
    print('invoke-first')
    repo(name='generated')
def second_impl(ctx):
    print('invoke-second')
    repo(name='generated')
first=module_extension(implementation=first_impl)
second=module_extension(implementation=second_impl)
"#;
        let module = "module(name='bazel_tools')\na=use_extension('//:ext.bzl','first')\nuse_repo(a, ok='generated')\nb=use_extension('//:ext.bzl','second')\nuse_repo(b, bad='missing')\noverride_repo(b, absent='bazel_tools')\n";
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(ValidationTracker::default());
        let observed =
            compute_observed(&dice, module, extension, true, Some(tracker.clone())).await;
        let validation_carrier = observed_carrier(&observed);
        assert!(matches!(
            validation_carrier.result().as_ref(),
            Err(HostValidatedGeneratedRepositorySpecsError {
                inner: PrivateValidationError::Validation {
                    predecessor,
                    validated: 1,
                    current,
                    offender: HostModuleExtensionValidationOffender::Import(import),
                    error: HostModuleExtensionValidationError::MissingImport,
                }
            }) if predecessor.parts().0.invoked.len() == 2
                && current.parts().0 == &predecessor.parts().0.invoked[1].request
                && import.parts().0 == "bad"
                && import.parts().1 == "missing"
        ));

        let mut legacy_tx =
            transaction_with_tracker(&dice, module, extension, true, tracker.clone()).await;
        let legacy_key = HostValidatedModuleExtensionRepositoriesKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
        );
        let legacy = legacy_tx.compute(&legacy_key).await.unwrap();
        let SourcePreparationOutcome::Complete(legacy) = legacy else {
            panic!("legacy validation must complete")
        };
        assert_eq!(legacy.as_ref(), validation_carrier.result().as_ref());

        let observed_key = HostValidatedModuleExtensionRepositoriesObservationKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
        );
        let dependencies = tracker.dependencies.lock().unwrap();
        assert_eq!(
            dependencies
                .iter()
                .find(|(name, _)| name == &observed_key.to_string())
                .unwrap()
                .1,
            [
                HostInstantiatedModuleExtensionRepositoriesObservationKey::new(
                    NormalizedAbsolutePath::new(WORKSPACE).unwrap()
                )
                .to_string()
            ]
        );
        assert_eq!(
            dependencies
                .iter()
                .find(|(name, _)| name == &legacy_key.to_string())
                .unwrap()
                .1,
            [HostInstantiatedModuleExtensionRepositoriesKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap()
            )
            .to_string()]
        );
        drop(dependencies);
        let activations = tracker.activations.lock().unwrap();
        let parent_rows = activations
            .iter()
            .filter(|(name, _, _)| name == &observed_key.to_string())
            .collect::<Vec<_>>();
        assert_eq!(parent_rows.len(), 1);
        assert_eq!(parent_rows[0].1, ActivationKind::Evaluated);
        assert!(parent_rows[0].2.is_none());
        assert!(
            activations
                .iter()
                .filter(|(name, _, _)| {
                    name.contains("instantiated-module-extension-repositories:")
                        || name.contains("validated-module-extension-repositories:")
                })
                .all(|(_, _, batch)| batch.is_none())
        );
        let prints = activations
            .iter()
            .filter(|(name, _, _)| name.starts_with("observed-"))
            .filter_map(|(_, _, batch)| batch.as_ref())
            .flat_map(EventBatch::events)
            .filter_map(|event| match event {
                EvaluationEvent::StarlarkPrint { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(prints, ["load", "invoke-first", "invoke-second"]);
        drop(activations);

        let warm_tracker = Arc::new(ValidationTracker::default());
        let mut warm_tx =
            transaction_with_tracker(&dice, module, extension, true, warm_tracker.clone()).await;
        let warm = warm_tx.compute(&observed_key).await.unwrap();
        assert!(HostValidatedModuleExtensionRepositoriesObservationKey::equality(&observed, &warm));
        assert!(Arc::ptr_eq(
            validation_carrier.result(),
            observed_carrier(&warm).result()
        ));
        assert!(
            warm_tracker
                .activations
                .lock()
                .unwrap()
                .iter()
                .any(|(name, kind, batch)| name == &observed_key.to_string()
                    && *kind == ActivationKind::Reused
                    && batch.is_none())
        );
        assert!(
            warm_tracker
                .activations
                .lock()
                .unwrap()
                .iter()
                .all(|(_, _, batch)| batch.is_none())
        );
    }

    #[tokio::test]
    async fn observed_validation_lifecycle_cancellation_and_nonactivation() {
        let base_module = module("use_repo(e, local='first')", "");
        let import_b = module("use_repo(e, local='missing')", "");
        let override_b = module("", "inject_repo(e, first='bazel_tools')");
        let generated_b = EXTENSION.replace("name='first'", "name='renamed'");
        let same_semantic = format!("{base_module}\n");
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let key = HostValidatedModuleExtensionRepositoriesObservationKey::new(workspace.clone());
        let mut held = Vec::new();
        for (module, extension) in [
            (base_module.as_str(), EXTENSION),
            (import_b.as_str(), EXTENSION),
            (base_module.as_str(), EXTENSION),
            (override_b.as_str(), EXTENSION),
            (base_module.as_str(), EXTENSION),
            (base_module.as_str(), generated_b.as_str()),
            (base_module.as_str(), EXTENSION),
            (same_semantic.as_str(), EXTENSION),
        ] {
            let mut transaction = transaction_untracked(&dice, module, extension, true).await;
            let global = transaction.compute(&PathObservationEpochKey).await.unwrap();
            let value = transaction.compute(&key).await.unwrap();
            let carrier = observed_carrier(&value).dupe();
            let child = transaction
                .compute(
                    &HostInstantiatedModuleExtensionRepositoriesObservationKey::new(
                        workspace.clone(),
                    ),
                )
                .await
                .unwrap();
            let SourcePreparationOutcome::Complete(Ok(child)) = child else {
                panic!("observed instantiation child must complete")
            };
            assert_eq!(carrier.observations(), child.observations());
            for (demand, result) in carrier.observations().observations() {
                assert_eq!(result.as_ref(), global.get(demand).unwrap().as_ref());
            }
            held.push(carrier);
        }
        for (a, b, restored) in [(0, 1, 2), (2, 3, 4), (4, 5, 6)] {
            assert_ne!(held[a].result(), held[b].result());
            assert_eq!(held[a].result(), held[restored].result());
            assert_eq!(held[a], held[restored]);
        }
        assert!(matches!(
            held[1].result().as_ref(),
            Err(HostValidatedGeneratedRepositorySpecsError {
                inner: PrivateValidationError::Validation {
                    validated: 0,
                    current,
                    offender: HostModuleExtensionValidationOffender::Import(import),
                    error: HostModuleExtensionValidationError::MissingImport,
                    ..
                }
            }) if current.parts().0.validation_parts().0[0] == *import
                && import.parts().1 == "missing"
        ));
        assert!(matches!(
            held[3].result().as_ref(),
            Err(HostValidatedGeneratedRepositorySpecsError {
                inner: PrivateValidationError::Validation {
                    predecessor,
                    validated: 0,
                    current,
                    offender: HostModuleExtensionValidationOffender::Override(row),
                    error: HostModuleExtensionValidationError::InjectCollision,
                }
            }) if current.parts().0 == &predecessor.parts().0.invoked[0].request
                && current.parts().0.validation_parts().1[0] == *row
                && row.parts().0 == "first"
        ));
        assert!(matches!(
            held[5].result().as_ref(),
            Err(HostValidatedGeneratedRepositorySpecsError {
                inner: PrivateValidationError::Validation {
                    offender: HostModuleExtensionValidationOffender::Import(import),
                    error: HostModuleExtensionValidationError::MissingImport,
                    ..
                }
            }) if import.parts().1 == "first"
        ));
        assert_eq!(held[0].result(), held[7].result());
        assert_ne!(held[0].observations(), held[7].observations());

        let tracker = Arc::new(ValidationTracker::default());
        let mut warm_tx =
            transaction_with_tracker(&dice, &base_module, EXTENSION, true, tracker.clone()).await;
        let first = observed_carrier(&warm_tx.compute(&key).await.unwrap()).dupe();
        tracker.activations.lock().unwrap().clear();
        tracker.dependencies.lock().unwrap().clear();
        let repeated = observed_carrier(&warm_tx.compute(&key).await.unwrap()).dupe();
        assert!(Arc::ptr_eq(first.result(), repeated.result()));
        assert!(
            tracker
                .activations
                .lock()
                .unwrap()
                .iter()
                .any(|(name, kind, batch)| name == &key.to_string()
                    && *kind == ActivationKind::Reused
                    && batch.is_none())
        );

        let cancel_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let cancel_tracker = Arc::new(ValidationTracker::default());
        let mut cancelled = transaction_with_tracker(
            &cancel_dice,
            &base_module,
            EXTENSION,
            true,
            cancel_tracker.clone(),
        )
        .await;
        let mut future = Box::pin(cancelled.compute(&key));
        std::future::poll_fn(|context| {
            assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        drop(future);
        assert!(
            cancel_tracker
                .activations
                .lock()
                .unwrap()
                .iter()
                .all(|(name, _, _)| name != &key.to_string())
        );
        assert!(
            cancel_tracker
                .dependencies
                .lock()
                .unwrap()
                .iter()
                .all(|(name, _)| name != &key.to_string())
        );

        let mut recovery = transaction_with_tracker(
            &cancel_dice,
            &base_module,
            EXTENSION,
            true,
            cancel_tracker.clone(),
        )
        .await;
        let global = recovery.compute(&PathObservationEpochKey).await.unwrap();
        let recovered = observed_carrier(&recovery.compute(&key).await.unwrap()).dupe();
        assert_eq!(recovered.result(), held[0].result());
        for (demand, result) in recovered.observations().observations() {
            assert_eq!(result.as_ref(), global.get(demand).unwrap().as_ref());
        }
        let legacy = HostValidatedModuleExtensionRepositoriesKey::new(workspace).to_string();
        let activations = cancel_tracker.activations.lock().unwrap();
        let dependencies = cancel_tracker.dependencies.lock().unwrap();
        assert!(activations.iter().all(|(name, _, _)| name != &legacy));
        assert!(dependencies.iter().all(|(name, children)| {
            name != &legacy && children.iter().all(|child| child != &legacy)
        }));
        for forbidden in [
            "host-root-repository-mapping:",
            "host-generated-repository-definition:",
            "host-canonical-selected-module-definition:",
            "host-canonical-repository",
        ] {
            assert!(
                activations
                    .iter()
                    .all(|(name, _, _)| !name.contains(forbidden))
            );
            assert!(dependencies.iter().all(|(name, children)| {
                !name.contains(forbidden) && children.iter().all(|child| !child.contains(forbidden))
            }));
        }
    }
}
