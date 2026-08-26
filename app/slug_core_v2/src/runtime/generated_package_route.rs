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
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_bzlmod_v2::RootRepositoryRoute;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalRepoName;
use slug_loading_v2::HostSelectedRepositoryFileEffectError;
use slug_loading_v2::HostSelectedRepositoryFileEffectKey;
use slug_loading_v2::HostSelectedRepositoryFileEffectObservationKey;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathObservationEpoch;

use super::generated_repository_definition::HostCanonicalRepositoryApparentMappingErrorDisposition;
use super::generated_repository_definition::HostCanonicalRepositoryApparentMappingKey;
use super::generated_repository_definition::HostCanonicalRepositoryApparentMappingObservationKey;
use super::root_apparent_repository_definition::HostRootApparentRepositoryDefinitionKey;
use super::root_apparent_repository_definition::HostRootApparentRepositoryDefinitionKind;
use super::root_apparent_repository_definition::HostRootApparentRepositoryDefinitionObservationKey;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(super) struct GeneratedPackageRouteKey {
    workspace: NormalizedAbsolutePath,
    apparent_repo: ApparentRepoName,
}

impl GeneratedPackageRouteKey {
    pub(super) fn new(
        workspace: NormalizedAbsolutePath,
        apparent_repo: ApparentRepoName,
    ) -> Option<Self> {
        (!apparent_repo.is_root()).then_some(Self {
            workspace,
            apparent_repo,
        })
    }
}

impl fmt::Display for GeneratedPackageRouteKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "generated-package-route:{}:@{}",
            self.workspace,
            self.apparent_repo.as_str()
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(super) struct GeneratedPackageRouteObservationKey(GeneratedPackageRouteKey);

impl GeneratedPackageRouteObservationKey {
    pub(super) fn new(
        workspace: NormalizedAbsolutePath,
        apparent_repo: ApparentRepoName,
    ) -> Option<Self> {
        GeneratedPackageRouteKey::new(workspace, apparent_repo).map(Self)
    }
}

impl fmt::Display for GeneratedPackageRouteObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum GeneratedPackageRouteErrorKind {
    Missing,
    ContextMismatch,
    Definition(Arc<str>),
    Compute(Arc<str>),
    Effect(HostSelectedRepositoryFileEffectError),
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) struct GeneratedPackageRouteError {
    apparent_repo: ApparentRepoName,
    kind: GeneratedPackageRouteErrorKind,
}

impl GeneratedPackageRouteError {
    pub(super) fn compute(apparent_repo: ApparentRepoName, error: impl fmt::Display) -> Self {
        Self {
            apparent_repo,
            kind: GeneratedPackageRouteErrorKind::Compute(Arc::from(error.to_string())),
        }
    }

    pub(super) fn is_fallback_neutral(&self) -> bool {
        matches!(self.kind, GeneratedPackageRouteErrorKind::Missing)
    }
}

impl fmt::Display for GeneratedPackageRouteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "generated repository route @{}: ",
            self.apparent_repo.as_str()
        )?;
        match &self.kind {
            GeneratedPackageRouteErrorKind::Missing => {
                f.write_str("apparent name is missing from the canonical mapping")
            }
            GeneratedPackageRouteErrorKind::ContextMismatch => {
                f.write_str("retained mapping context mismatch")
            }
            GeneratedPackageRouteErrorKind::Definition(message)
            | GeneratedPackageRouteErrorKind::Compute(message) => f.write_str(message),
            GeneratedPackageRouteErrorKind::Effect(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for GeneratedPackageRouteError {}

type GeneratedPackageRouteResult = Arc<Result<RootRepositoryRoute, GeneratedPackageRouteError>>;
type GeneratedPackageRouteOutcome = SourcePreparationOutcome<GeneratedPackageRouteResult>;

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(super) struct ObservedGeneratedPackageRoute {
    result: GeneratedPackageRouteResult,
    observations: PathObservationEpoch,
}

impl ObservedGeneratedPackageRoute {
    pub(super) fn result(&self) -> &GeneratedPackageRouteResult {
        &self.result
    }

    pub(super) fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(super) struct GeneratedPackageRouteObservationError;

impl fmt::Display for GeneratedPackageRouteObservationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("generated package route observation failed")
    }
}

#[derive(Clone, Copy)]
enum GeneratedPackageRouteMode {
    Legacy,
    Observed,
}

type GeneratedPackageRouteDriverOutcome = SourcePreparationOutcome<
    Result<
        (GeneratedPackageRouteResult, PathObservationEpoch),
        GeneratedPackageRouteObservationError,
    >,
>;

fn complete_generated_package_route(
    key: &GeneratedPackageRouteKey,
    result: Result<RootRepositoryRoute, GeneratedPackageRouteErrorKind>,
    observations: PathObservationEpoch,
) -> GeneratedPackageRouteDriverOutcome {
    SourcePreparationOutcome::Complete(Ok((
        Arc::new(result.map_err(|kind| GeneratedPackageRouteError {
            apparent_repo: key.apparent_repo.clone(),
            kind,
        })),
        observations,
    )))
}

fn merge_generated_package_route_observations(
    mapping: &PathObservationEpoch,
    definition: &PathObservationEpoch,
) -> Result<PathObservationEpoch, ObservedPathFrontierError> {
    PathObservationEpoch::from_shared(
        mapping
            .observations()
            .iter()
            .chain(definition.observations().iter())
            .map(|(demand, result)| (demand.dupe(), result.dupe())),
    )
    .map_err(ObservedPathFrontierError::from)
}

fn mapping_key(key: &GeneratedPackageRouteKey) -> HostCanonicalRepositoryApparentMappingKey {
    HostCanonicalRepositoryApparentMappingKey::new(
        key.workspace.dupe(),
        CanonicalRepoName::root(),
        key.apparent_repo.clone(),
    )
}

fn definition_key(key: &GeneratedPackageRouteKey) -> HostRootApparentRepositoryDefinitionKey {
    HostRootApparentRepositoryDefinitionKey::new(key.workspace.dupe(), key.apparent_repo.clone())
        .expect("generated package route rejects root apparent repositories")
}

fn effect_key(
    key: &GeneratedPackageRouteKey,
    seed: super::generated_repository_definition::HostGeneratedRepositoryEffectSeed<'_>,
) -> HostSelectedRepositoryFileEffectKey {
    HostSelectedRepositoryFileEffectKey::new(
        key.workspace.dupe(),
        seed.owner().clone(),
        seed.ordinal(),
    )
}

fn compute_kind(error: impl fmt::Display) -> GeneratedPackageRouteErrorKind {
    GeneratedPackageRouteErrorKind::Compute(Arc::from(error.to_string()))
}

fn mapping_error_kind(
    disposition: HostCanonicalRepositoryApparentMappingErrorDisposition,
    error: impl fmt::Display,
) -> GeneratedPackageRouteErrorKind {
    match disposition {
        HostCanonicalRepositoryApparentMappingErrorDisposition::Missing => {
            GeneratedPackageRouteErrorKind::Missing
        }
        HostCanonicalRepositoryApparentMappingErrorDisposition::ContextMismatch => {
            GeneratedPackageRouteErrorKind::ContextMismatch
        }
        HostCanonicalRepositoryApparentMappingErrorDisposition::Other => {
            GeneratedPackageRouteErrorKind::Definition(Arc::from(error.to_string()))
        }
    }
}

async fn drive_generated_package_route(
    ctx: &mut DiceComputations<'_>,
    key: &GeneratedPackageRouteKey,
    mode: GeneratedPackageRouteMode,
) -> GeneratedPackageRouteDriverOutcome {
    let (mapping_result, mapping_observations) = match mode {
        GeneratedPackageRouteMode::Legacy => match ctx.compute(&mapping_key(key)).await {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(result)) => {
                (result, PathObservationEpoch::empty())
            }
            Err(error) => {
                return complete_generated_package_route(
                    key,
                    Err(compute_kind(error)),
                    PathObservationEpoch::empty(),
                );
            }
        },
        GeneratedPackageRouteMode::Observed => match ctx
            .compute(&HostCanonicalRepositoryApparentMappingObservationKey::new(
                key.workspace.dupe(),
                CanonicalRepoName::root(),
                key.apparent_repo.clone(),
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(Err(_))) => {
                return SourcePreparationOutcome::Complete(Err(
                    GeneratedPackageRouteObservationError,
                ));
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => {
                (observed.result().dupe(), observed.observations().dupe())
            }
            Err(error) => {
                return complete_generated_package_route(
                    key,
                    Err(compute_kind(error)),
                    PathObservationEpoch::empty(),
                );
            }
        },
    };
    if let Err(error) = mapping_result.as_ref() {
        return complete_generated_package_route(
            key,
            Err(mapping_error_kind(error.disposition(), error)),
            mapping_observations,
        );
    }

    let (definition_result, definition_observations) = match mode {
        GeneratedPackageRouteMode::Legacy => match ctx.compute(&definition_key(key)).await {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(result)) => {
                (result, PathObservationEpoch::empty())
            }
            Err(error) => {
                return complete_generated_package_route(
                    key,
                    Err(compute_kind(error)),
                    mapping_observations,
                );
            }
        },
        GeneratedPackageRouteMode::Observed => match ctx
            .compute(
                &HostRootApparentRepositoryDefinitionObservationKey::new(
                    key.workspace.dupe(),
                    key.apparent_repo.clone(),
                )
                .expect("generated package route rejects root apparent repositories"),
            )
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(Err(_))) => {
                return SourcePreparationOutcome::Complete(Err(
                    GeneratedPackageRouteObservationError,
                ));
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => {
                (observed.result().dupe(), observed.observations().dupe())
            }
            Err(error) => {
                return complete_generated_package_route(
                    key,
                    Err(compute_kind(error)),
                    mapping_observations,
                );
            }
        },
    };
    let observations = match merge_generated_package_route_observations(
        &mapping_observations,
        &definition_observations,
    ) {
        Ok(observations) => observations,
        Err(error) => {
            let _ = error;
            return SourcePreparationOutcome::Complete(Err(GeneratedPackageRouteObservationError));
        }
    };
    let view_and_seed = match definition_result.as_ref() {
        Err(error) => Err(GeneratedPackageRouteErrorKind::Definition(Arc::from(
            error.to_string(),
        ))),
        Ok(definition) => match definition.view() {
            Some(view) if view.kind() == HostRootApparentRepositoryDefinitionKind::Generated => {
                view.generated_effect_seed()
                    .map(|seed| (view, seed))
                    .ok_or(GeneratedPackageRouteErrorKind::Missing)
            }
            _ => Err(GeneratedPackageRouteErrorKind::Missing),
        },
    };
    let (view, seed) = match view_and_seed {
        Ok(value) => value,
        Err(error) => return complete_generated_package_route(key, Err(error), observations),
    };
    let (effect_result, effect_observations) = match mode {
        GeneratedPackageRouteMode::Legacy => match ctx.compute(&effect_key(key, seed)).await {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(result)) => {
                (result, PathObservationEpoch::empty())
            }
            Err(error) => {
                return complete_generated_package_route(
                    key,
                    Err(compute_kind(error)),
                    observations,
                );
            }
        },
        GeneratedPackageRouteMode::Observed => match ctx
            .compute(&HostSelectedRepositoryFileEffectObservationKey::new(
                key.workspace.dupe(),
                seed.owner().clone(),
                seed.ordinal(),
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                let _ = error;
                return SourcePreparationOutcome::Complete(Err(
                    GeneratedPackageRouteObservationError,
                ));
            }
            Ok(SourcePreparationOutcome::Complete(Ok(observed))) => {
                (observed.result().dupe(), observed.observations().dupe())
            }
            Err(error) => {
                return complete_generated_package_route(
                    key,
                    Err(compute_kind(error)),
                    observations,
                );
            }
        },
    };
    let observations =
        match merge_generated_package_route_observations(&observations, &effect_observations) {
            Ok(observations) => observations,
            Err(error) => {
                let _ = error;
                return SourcePreparationOutcome::Complete(Err(
                    GeneratedPackageRouteObservationError,
                ));
            }
        };
    let result = match effect_result.as_ref() {
        Err(error) => Err(GeneratedPackageRouteErrorKind::Effect(error.clone())),
        Ok(effect) => view
            .repo_spec()
            .cloned()
            .and_then(|repo_spec| {
                RootRepositoryRoute::for_generated_repo_spec(
                    key.workspace.dupe(),
                    key.apparent_repo.clone(),
                    view.canonical_repo().clone(),
                    repo_spec,
                    view.local_path_policy(),
                    effect.plan().clone(),
                )
            })
            .ok_or(GeneratedPackageRouteErrorKind::Missing),
    };
    complete_generated_package_route(key, result, observations)
}

#[async_trait]
impl Key for GeneratedPackageRouteKey {
    type Value = GeneratedPackageRouteOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match drive_generated_package_route(ctx, self, GeneratedPackageRouteMode::Legacy).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                debug_assert!(observations.observations().is_empty());
                SourcePreparationOutcome::Complete(result)
            }
            SourcePreparationOutcome::Complete(Err(_)) => {
                unreachable!("legacy generated package route has no observed outer")
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
impl Key for GeneratedPackageRouteObservationKey {
    type Value = SourcePreparationOutcome<
        Result<ObservedGeneratedPackageRoute, GeneratedPackageRouteObservationError>,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match drive_generated_package_route(ctx, &self.0, GeneratedPackageRouteMode::Observed).await
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                SourcePreparationOutcome::Complete(Err(error))
            }
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                SourcePreparationOutcome::Complete(Ok(ObservedGeneratedPackageRoute {
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
    use dupe::Dupe;
    use slug_bzlmod_v2::RepositoryMaterializationSuccess;
    use slug_bzlmod_v2::SourcePreparationOutcome;
    use slug_events_v2::CaptureEvaluationEvents;
    use slug_events_v2::EventBatch;
    use slug_identity_v2::ApparentRepoName;
    use slug_identity_v2::CanonicalRepoName;
    use slug_loading_v2::HostSelectedRepositoryFileEffectObservationKey;
    use slug_workspace_v2::NormalizedAbsolutePath;
    use slug_workspace_v2::PathObservationEpoch;
    use slug_workspace_v2::PathObservationEpochKey;

    use super::super::generated_repository_definition::HostCanonicalRepositoryApparentMappingErrorDisposition;
    use super::super::generated_repository_definition::HostCanonicalRepositoryApparentMappingObservationKey;
    use super::super::generated_repository_definition::tests::EXTENSION_A;
    use super::super::generated_repository_definition::tests::MODULE;
    use super::super::generated_repository_definition::tests::WORKSPACE;
    use super::super::generated_repository_definition::tests::transaction;
    use super::super::generated_repository_definition::tests::transaction_with_command_override;
    use super::super::root_apparent_repository_definition::HostRootApparentRepositoryDefinitionObservationKey;
    use super::super::root_apparent_repository_definition::tests::CompositionTracker;
    use super::super::root_apparent_repository_definition::tests::local_materialized_transaction;
    use super::GeneratedPackageRouteError;
    use super::GeneratedPackageRouteErrorKind;
    use super::GeneratedPackageRouteKey;
    use super::GeneratedPackageRouteObservationKey;
    use super::mapping_error_kind;

    const FILE_EFFECT_EXTENSION: &str = r#"
def write(ctx):
    print('generated package route child event')
    ctx.file('BUILD.bazel', 'exports_files([\"generated.txt\"])\\n')
repo=repository_rule(implementation=write, attrs={'value':attr.string(), 'target':attr.label()})
def impl(ctx):
    repo(name='first', value='one', target=':local')
    repo(name='second', value='two', target='@first//:item')
ext=module_extension(implementation=impl)
"#;

    #[derive(Default)]
    struct RouteTracker(Mutex<Vec<(String, ActivationKind, Option<EventBatch>)>>);

    impl RouteTracker {
        fn take(&self) -> Vec<(String, ActivationKind, Option<EventBatch>)> {
            std::mem::take(&mut *self.0.lock().unwrap())
        }
    }

    impl ActivationTracker for RouteTracker {
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

    #[test]
    fn generated_package_route_keys_reject_root_and_preserve_identity_and_display() {
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let apparent = ApparentRepoName::new("generated").unwrap();
        let key = GeneratedPackageRouteKey::new(workspace.clone(), apparent.clone()).unwrap();
        let same = GeneratedPackageRouteKey::new(workspace.clone(), apparent.clone()).unwrap();
        let observed =
            GeneratedPackageRouteObservationKey::new(workspace.clone(), apparent).unwrap();
        let hash = |value: &GeneratedPackageRouteKey| {
            let mut state = DefaultHasher::new();
            value.hash(&mut state);
            state.finish()
        };

        assert_eq!(key, same);
        assert_eq!(hash(&key), hash(&same));
        assert_eq!(
            key.to_string(),
            "generated-package-route:\"/workspace\":@generated"
        );
        assert_eq!(
            observed.to_string(),
            "observed-generated-package-route:\"/workspace\":@generated"
        );
        assert!(
            GeneratedPackageRouteKey::new(workspace.clone(), ApparentRepoName::root()).is_none()
        );
        assert!(
            GeneratedPackageRouteObservationKey::new(workspace, ApparentRepoName::root()).is_none()
        );
        let _: fn(&<GeneratedPackageRouteKey as Key>::Value) -> bool =
            GeneratedPackageRouteKey::validity;
    }

    #[tokio::test]
    async fn generated_package_route_constructs_legacy_and_observed_generated_routes() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let apparent = ApparentRepoName::new("first").unwrap();
        let key = GeneratedPackageRouteKey::new(workspace.clone(), apparent.clone()).unwrap();
        let observed_key =
            GeneratedPackageRouteObservationKey::new(workspace.clone(), apparent.clone()).unwrap();
        let mut transaction = transaction(&dice, MODULE, EXTENSION_A, true, None).await;

        let legacy = transaction.compute(&key).await.unwrap();
        let SourcePreparationOutcome::Complete(route) = legacy else {
            panic!("generated route must complete")
        };
        let Ok(route) = route.as_ref() else {
            panic!("generated route must resolve")
        };
        assert_eq!(route.module_name(), "first");

        let observed = transaction.compute(&observed_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(observed)) = observed else {
            panic!("observed generated route must complete")
        };
        assert!(observed.result().as_ref().is_ok());
        assert!(!observed.observations().observations().is_empty());

        let mapping_key = HostCanonicalRepositoryApparentMappingObservationKey::new(
            workspace.clone(),
            CanonicalRepoName::root(),
            apparent.clone(),
        );
        let definition_key =
            HostRootApparentRepositoryDefinitionObservationKey::new(workspace.clone(), apparent)
                .unwrap();
        let SourcePreparationOutcome::Complete(Ok(mapping)) =
            transaction.compute(&mapping_key).await.unwrap()
        else {
            panic!("mapping carrier must complete")
        };
        let SourcePreparationOutcome::Complete(Ok(definition)) =
            transaction.compute(&definition_key).await.unwrap()
        else {
            panic!("definition carrier must complete")
        };
        assert_eq!(
            observed.observations(),
            &super::merge_generated_package_route_observations(
                mapping.observations(),
                definition.observations(),
            )
            .unwrap()
        );

        let missing =
            GeneratedPackageRouteKey::new(workspace, ApparentRepoName::new("missing").unwrap())
                .unwrap();
        let SourcePreparationOutcome::Complete(missing) =
            transaction.compute(&missing).await.unwrap()
        else {
            panic!("missing mapping must complete")
        };
        assert!(matches!(missing.as_ref(), Err(error) if error.is_fallback_neutral()));
    }

    #[tokio::test]
    async fn generated_package_route_associates_real_effect_observations_and_child_events() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let apparent = ApparentRepoName::new("first").unwrap();
        let tracker = Arc::new(RouteTracker::default());
        let mut transaction = transaction(
            &dice,
            MODULE,
            FILE_EFFECT_EXTENSION,
            true,
            Some(tracker.clone() as Arc<dyn ActivationTracker>),
        )
        .await;
        let key = GeneratedPackageRouteKey::new(workspace.clone(), apparent.clone()).unwrap();
        let observed_key =
            GeneratedPackageRouteObservationKey::new(workspace.clone(), apparent.clone()).unwrap();
        let observed = transaction.compute(&observed_key).await.unwrap();
        let SourcePreparationOutcome::Complete(Ok(observed)) = observed else {
            panic!("observed generated route must complete")
        };
        let SourcePreparationOutcome::Complete(Ok(definition)) = transaction
            .compute(
                &HostRootApparentRepositoryDefinitionObservationKey::new(
                    workspace.clone(),
                    apparent.clone(),
                )
                .unwrap(),
            )
            .await
            .unwrap()
        else {
            panic!("generated definition must complete")
        };
        let seed = definition
            .result()
            .as_ref()
            .as_ref()
            .unwrap()
            .view()
            .unwrap()
            .generated_effect_seed()
            .unwrap();
        let effect = transaction
            .compute(&HostSelectedRepositoryFileEffectObservationKey::new(
                workspace.clone(),
                seed.owner().clone(),
                seed.ordinal(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(effect)) = effect else {
            panic!("generated effect must complete")
        };
        let mapping = transaction
            .compute(&HostCanonicalRepositoryApparentMappingObservationKey::new(
                workspace.clone(),
                CanonicalRepoName::root(),
                apparent,
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(Ok(mapping)) = mapping else {
            panic!("mapping must complete")
        };
        let expected = super::merge_generated_package_route_observations(
            mapping.observations(),
            definition.observations(),
        )
        .unwrap();
        assert_eq!(
            observed.observations(),
            &super::merge_generated_package_route_observations(&expected, effect.observations())
                .unwrap()
        );
        let legacy = transaction.compute(&key).await.unwrap();
        assert!(
            matches!(legacy, SourcePreparationOutcome::Complete(route) if route.as_ref().is_ok())
        );
        let events = tracker.take();
        assert!(
            events
                .iter()
                .filter(|(name, _, _)| name == &observed_key.to_string())
                .all(|(_, _, batch)| batch.is_none())
        );
        assert_eq!(
            events
                .iter()
                .filter(
                    |(name, _, _)| name == &format!("observed-{}", super::effect_key(&key, seed))
                )
                .filter_map(|(_, _, batch)| batch.as_ref())
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn generated_package_route_forwards_real_effect_semantic_terminal() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let apparent = ApparentRepoName::new("first").unwrap();
        let extension = FILE_EFFECT_EXTENSION.replace(
            "ctx.file('BUILD.bazel', 'exports_files([\\\"generated.txt\\\"])\\\\n')",
            "fail('generated package route semantic failure')",
        );
        let mut transaction = transaction(&dice, MODULE, &extension, true, None).await;
        let legacy_key =
            GeneratedPackageRouteKey::new(workspace.clone(), apparent.clone()).unwrap();
        let observed_key = GeneratedPackageRouteObservationKey::new(workspace, apparent).unwrap();
        let legacy = transaction.compute(&legacy_key).await.unwrap();
        let observed = transaction.compute(&observed_key).await.unwrap();
        let SourcePreparationOutcome::Complete(legacy) = legacy else {
            panic!("semantic route must complete")
        };
        let SourcePreparationOutcome::Complete(Ok(observed)) = observed else {
            panic!("observed semantic route must complete")
        };
        assert_eq!(&legacy, observed.result());
        assert!(matches!(
            legacy.as_ref(),
            Err(GeneratedPackageRouteError {
                kind: GeneratedPackageRouteErrorKind::Effect(_),
                ..
            })
        ));
    }

    #[tokio::test]
    async fn generated_package_route_forwards_lawful_pre_effect_need_without_activation() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let tracker = Arc::new(RouteTracker::default());
        let mut transaction = transaction(
            &dice,
            MODULE,
            FILE_EFFECT_EXTENSION,
            true,
            Some(tracker.clone() as Arc<dyn ActivationTracker>),
        )
        .await;
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
        let legacy = GeneratedPackageRouteKey::new(
            workspace.clone(),
            ApparentRepoName::new("first").unwrap(),
        )
        .unwrap();
        let observed = GeneratedPackageRouteObservationKey::new(
            workspace,
            ApparentRepoName::new("first").unwrap(),
        )
        .unwrap();
        assert!(matches!(
            transaction.compute(&legacy).await.unwrap(),
            SourcePreparationOutcome::Need(_)
        ));
        assert!(matches!(
            transaction.compute(&observed).await.unwrap(),
            SourcePreparationOutcome::Need(_)
        ));
        assert!(
            tracker
                .take()
                .iter()
                .all(|(name, _, _)| !name.contains("HostSelectedRepositoryFileEffect"))
        );
    }

    #[tokio::test]
    async fn generated_package_route_does_not_activate_effect_for_selected_nonregistry() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let module = "module(name='bazel_tools')\nlocal_path_override(module_name='local', path='local')\nbazel_dep(name='local', version='1', repo_name='local_alias')\n";
        let mut transaction =
            transaction_with_command_override(&dice, module, EXTENSION_A, "local").await;
        let key = GeneratedPackageRouteKey::new(
            workspace.clone(),
            ApparentRepoName::new("local_alias").unwrap(),
        )
        .unwrap();
        let route = transaction.compute(&key).await.unwrap();
        let request = match route {
            SourcePreparationOutcome::Need(need) => need
                .repository_materializations()
                .values()
                .next()
                .unwrap()
                .clone(),
            SourcePreparationOutcome::Complete(_) => {
                panic!("local override must materialize first")
            }
        };
        let tracker = Arc::new(RouteTracker::default());
        let transaction = local_materialized_transaction(
            &dice,
            &workspace,
            request,
            Arc::new(CompositionTracker::default()),
            RepositoryMaterializationSuccess::Local,
        )
        .await;
        let mut user_data = UserComputationData {
            cycle_detector: Some(slug_loading_v2::bzl_load_cycle_detector()),
            activation_tracker: Some(tracker.clone() as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        user_data.data.set(CaptureEvaluationEvents);
        let mut transaction = transaction.into_updater().commit_with_data(user_data).await;
        let route = transaction.compute(&key).await.unwrap();
        assert!(matches!(
            route,
            SourcePreparationOutcome::Complete(result) if matches!(result.as_ref(), Err(error) if error.is_fallback_neutral())
        ));
        assert!(
            tracker
                .take()
                .iter()
                .all(|(name, _, _)| !name.contains("HostSelectedRepositoryFileEffect"))
        );
    }

    #[test]
    fn generated_package_route_terminal_algebra_preserves_fallback_polarity() {
        let apparent = ApparentRepoName::new("generated").unwrap();
        let missing = GeneratedPackageRouteError {
            apparent_repo: apparent.clone(),
            kind: mapping_error_kind(
                HostCanonicalRepositoryApparentMappingErrorDisposition::Missing,
                "missing",
            ),
        };
        let context = mapping_error_kind(
            HostCanonicalRepositoryApparentMappingErrorDisposition::ContextMismatch,
            "context",
        );
        let other = mapping_error_kind(
            HostCanonicalRepositoryApparentMappingErrorDisposition::Other,
            "other",
        );
        let complete: <GeneratedPackageRouteKey as Key>::Value =
            SourcePreparationOutcome::Complete(Arc::new(Err(missing.clone())));
        let changed: <GeneratedPackageRouteKey as Key>::Value =
            SourcePreparationOutcome::Complete(Arc::new(Err(GeneratedPackageRouteError {
                apparent_repo: apparent,
                kind: GeneratedPackageRouteErrorKind::ContextMismatch,
            })));

        assert!(missing.is_fallback_neutral());
        assert!(matches!(
            context,
            GeneratedPackageRouteErrorKind::ContextMismatch
        ));
        assert!(matches!(
            other,
            GeneratedPackageRouteErrorKind::Definition(_)
        ));
        assert!(GeneratedPackageRouteKey::validity(&complete));
        assert!(GeneratedPackageRouteKey::equality(&complete, &complete));
        assert!(!GeneratedPackageRouteKey::equality(&complete, &changed));
    }

    #[test]
    fn generated_package_route_forwards_effect_terminals_without_parent_carriers_or_events() {
        let source = include_str!("generated_package_route.rs");
        let production = source.split_once("\n#[cfg(test)]\nmod tests {").unwrap().0;
        let driver = &production[production
            .find("async fn drive_generated_package_route")
            .unwrap()..];
        let mapping = driver
            .find("HostCanonicalRepositoryApparentMappingObservationKey::new")
            .unwrap();
        let definition = driver
            .find("HostRootApparentRepositoryDefinitionObservationKey::new")
            .unwrap();
        let effect = driver
            .find("HostSelectedRepositoryFileEffectObservationKey::new")
            .unwrap();
        assert!(mapping < definition && definition < effect);
        assert!(driver.contains("SourcePreparationOutcome::Need(need)"));
        assert!(driver.contains("GeneratedPackageRouteErrorKind::Effect(error.clone())"));
        assert!(!driver.contains("EventBatch"));
        assert!(!driver.contains("CaptureEvaluationEvents"));
    }
}
