/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory or the Apache License found in the
 * LICENSE-APACHE file. You may select the license that applies to you.
 */

use std::fmt;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::PackageIdentifier;
use slug_identity_v2::PackagePath;
use slug_identity_v2::TargetName;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathOutcome;

use crate::HostRepositoryMaterializationDisposition;
use crate::HostRepositorySourceRoute;
use crate::RepositoryMaterialization;
use crate::SourcePreparationNeeds;
use crate::SourcePreparationOutcome;
use crate::host_package::ExternalRepositoryPackageLookup;
use crate::host_package::ExternalRepositoryPackageLookupError;
use crate::host_package::ExternalRepositoryPackageLookupKey;
use crate::host_package::ExternalRepositoryPackageLookupObservationKey;
use crate::host_package::HostRootPackageLookup;
use crate::host_package::HostRootPackageLookupError;
use crate::host_package::HostRootPackageLookupKey;
use crate::host_package::HostRootPackageLookupObservationKey;
use crate::source_preparation::RepositoryMaterializationResultKey;
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositoryLabelPathAddress {
    package: PackageIdentifier,
    target: TargetName,
}
impl RepositoryLabelPathAddress {
    pub fn from_label(label: &CanonicalLabel) -> Self {
        Self {
            package: label.package().clone(),
            target: label.target().clone(),
        }
    }
    pub fn repo(&self) -> &CanonicalRepoName {
        self.package.repo()
    }
    pub fn package(&self) -> &PackagePath {
        self.package.package()
    }
    pub fn target(&self) -> &TargetName {
        &self.target
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub enum HostRepositoryLabelPathSource {
    Root(NormalizedAbsolutePath),
    External(HostRepositorySourceRoute),
}
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum HostRepositoryLabelPathError {
    RootRepositoryMismatch(CanonicalRepoName),
    ExternalRepositoryMismatch(CanonicalRepoName, CanonicalRepoName),
    PackageAbsent,
    PackageDeleted,
    PackageIgnored,
    InvalidPackageName(Arc<str>),
    RootPackageLookup(Arc<str>),
    ExternalPackageLookup(Arc<str>),
    BuiltinCatalog,
    Materialization(Arc<str>),
    MaterializationCompute(Arc<str>),
    InvalidMaterializedPath,
    NonUnicodePhysicalPath,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub struct HostRepositoryLabelPathValue {
    path: NormalizedAbsolutePath,
    namespace: PathObservationNamespace,
}

impl HostRepositoryLabelPathValue {
    pub fn new(
        path: NormalizedAbsolutePath,
        namespace: PathObservationNamespace,
    ) -> Result<Self, HostRepositoryLabelPathError> {
        path.as_path()
            .to_str()
            .is_some()
            .then_some(Self { path, namespace })
            .ok_or(HostRepositoryLabelPathError::NonUnicodePhysicalPath)
    }

    pub fn path(&self) -> &NormalizedAbsolutePath {
        &self.path
    }

    pub fn path_str(&self) -> &str {
        self.path
            .as_path()
            .to_str()
            .expect("HostRepositoryLabelPathValue validates Unicode at construction")
    }

    pub fn namespace(&self) -> PathObservationNamespace {
        self.namespace
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostRepositoryLabelPathKey {
    source: HostRepositoryLabelPathSource,
    address: RepositoryLabelPathAddress,
}

impl HostRepositoryLabelPathKey {
    pub fn new_root(
        workspace: NormalizedAbsolutePath,
        address: RepositoryLabelPathAddress,
    ) -> Result<Self, HostRepositoryLabelPathError> {
        if !address.repo().is_root() {
            return Err(HostRepositoryLabelPathError::RootRepositoryMismatch(
                address.repo().clone(),
            ));
        }
        Ok(Self {
            source: HostRepositoryLabelPathSource::Root(workspace),
            address,
        })
    }

    pub fn new_external(
        route: HostRepositorySourceRoute,
        address: RepositoryLabelPathAddress,
    ) -> Result<Self, HostRepositoryLabelPathError> {
        if address.repo() != route.canonical_repo() {
            return Err(HostRepositoryLabelPathError::ExternalRepositoryMismatch(
                route.canonical_repo().clone(),
                address.repo().clone(),
            ));
        }
        Ok(Self {
            source: HostRepositoryLabelPathSource::External(route),
            address,
        })
    }
}

impl Hash for HostRepositoryLabelPathKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.source.hash(state);
        self.address.hash(state);
    }
}

impl fmt::Display for HostRepositoryLabelPathKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "repository-label-path:{}//{}:{}",
            self.address.repo(),
            self.address.package(),
            self.address.target()
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostRepositoryLabelPathObservationKey(HostRepositoryLabelPathKey);

impl HostRepositoryLabelPathObservationKey {
    pub fn new_root(
        workspace: NormalizedAbsolutePath,
        address: RepositoryLabelPathAddress,
    ) -> Result<Self, HostRepositoryLabelPathError> {
        HostRepositoryLabelPathKey::new_root(workspace, address).map(Self)
    }

    pub fn new_external(
        route: HostRepositorySourceRoute,
        address: RepositoryLabelPathAddress,
    ) -> Result<Self, HostRepositoryLabelPathError> {
        HostRepositoryLabelPathKey::new_external(route, address).map(Self)
    }
}

impl fmt::Display for HostRepositoryLabelPathObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedHostRepositoryLabelPath {
    result: Arc<Result<HostRepositoryLabelPathValue, HostRepositoryLabelPathError>>,
    observations: PathObservationEpoch,
}

impl ObservedHostRepositoryLabelPath {
    pub fn result(
        &self,
    ) -> &Arc<Result<HostRepositoryLabelPathValue, HostRepositoryLabelPathError>> {
        &self.result
    }

    pub fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

type LabelPathResult = Arc<Result<HostRepositoryLabelPathValue, HostRepositoryLabelPathError>>;
type ObservedLabelPathResult =
    SourcePreparationOutcome<Result<ObservedHostRepositoryLabelPath, ObservedPathFrontierError>>;

fn complete(
    result: Result<HostRepositoryLabelPathValue, HostRepositoryLabelPathError>,
) -> LabelPathResult {
    Arc::new(result)
}

fn observed_complete(
    result: Result<HostRepositoryLabelPathValue, HostRepositoryLabelPathError>,
    observations: PathObservationEpoch,
) -> ObservedLabelPathResult {
    SourcePreparationOutcome::Complete(Ok(ObservedHostRepositoryLabelPath {
        result: complete(result),
        observations,
    }))
}

fn root_lookup_result(
    lookup: Result<HostRootPackageLookup, HostRootPackageLookupError>,
    address: &RepositoryLabelPathAddress,
) -> Result<NormalizedAbsolutePath, HostRepositoryLabelPathError> {
    match lookup {
        Ok(HostRootPackageLookup::Package(package)) => {
            lexical_join(package.package_root(), address.package(), address.target())
        }
        Ok(HostRootPackageLookup::NoBuildFile) => Err(HostRepositoryLabelPathError::PackageAbsent),
        Ok(HostRootPackageLookup::Deleted) => Err(HostRepositoryLabelPathError::PackageDeleted),
        Ok(HostRootPackageLookup::InvalidPackageName { message }) => {
            Err(HostRepositoryLabelPathError::InvalidPackageName(message))
        }
        Err(error) => Err(HostRepositoryLabelPathError::RootPackageLookup(Arc::from(
            format!("{error:?}"),
        ))),
    }
}

fn external_lookup_result(
    lookup: Result<ExternalRepositoryPackageLookup, ExternalRepositoryPackageLookupError>,
) -> Result<(), HostRepositoryLabelPathError> {
    match lookup {
        Ok(ExternalRepositoryPackageLookup::Package(_)) => Ok(()),
        Ok(ExternalRepositoryPackageLookup::NoBuildFile) => {
            Err(HostRepositoryLabelPathError::PackageAbsent)
        }
        Ok(ExternalRepositoryPackageLookup::Deleted) => {
            Err(HostRepositoryLabelPathError::PackageDeleted)
        }
        Ok(ExternalRepositoryPackageLookup::IgnoredDirectory) => {
            Err(HostRepositoryLabelPathError::PackageIgnored)
        }
        Ok(ExternalRepositoryPackageLookup::InvalidPackageName { message }) => {
            Err(HostRepositoryLabelPathError::InvalidPackageName(message))
        }
        Err(error) => Err(HostRepositoryLabelPathError::ExternalPackageLookup(
            Arc::from(format!("{error:?}")),
        )),
    }
}

fn lexical_join(
    root: &NormalizedAbsolutePath,
    package: &PackagePath,
    target: &TargetName,
) -> Result<NormalizedAbsolutePath, HostRepositoryLabelPathError> {
    NormalizedAbsolutePath::new(root.as_path().join(package.as_str()).join(target.as_str()))
        .map_err(|_| HostRepositoryLabelPathError::InvalidMaterializedPath)
}

async fn external_value(
    ctx: &mut DiceComputations<'_>,
    route: &HostRepositorySourceRoute,
    address: &RepositoryLabelPathAddress,
) -> SourcePreparationOutcome<LabelPathResult> {
    if route.is_builtin_bazel_tools() {
        return SourcePreparationOutcome::Complete(complete(Err(
            HostRepositoryLabelPathError::BuiltinCatalog,
        )));
    }
    let request = match route.materialization_disposition() {
        Ok(HostRepositoryMaterializationDisposition::Request(request)) => request,
        Ok(HostRepositoryMaterializationDisposition::Builtin(_)) => {
            return SourcePreparationOutcome::Complete(complete(Err(
                HostRepositoryLabelPathError::BuiltinCatalog,
            )));
        }
        Err(error) => {
            return SourcePreparationOutcome::Complete(complete(Err(
                HostRepositoryLabelPathError::Materialization(Arc::from(format!("{error:?}"))),
            )));
        }
    };
    let materialization = match ctx
        .compute(&RepositoryMaterializationResultKey { request })
        .await
    {
        Ok(SourcePreparationOutcome::Need(need)) => return SourcePreparationOutcome::Need(need),
        Ok(SourcePreparationOutcome::Complete(value)) => value,
        Err(error) => {
            return SourcePreparationOutcome::Complete(complete(Err(
                HostRepositoryLabelPathError::MaterializationCompute(Arc::from(error.to_string())),
            )));
        }
    };
    let materialization = match materialization.as_ref() {
        Ok(value) => value,
        Err(error) => {
            return SourcePreparationOutcome::Complete(complete(Err(
                HostRepositoryLabelPathError::Materialization(Arc::from(format!("{error:?}"))),
            )));
        }
    };
    let (root, namespace) = match materialization {
        RepositoryMaterialization::Local { source_root, .. } => {
            (source_root, PathObservationNamespace::Host)
        }
        RepositoryMaterialization::Immutable {
            generation_root,
            observation_instance,
            ..
        } => (
            generation_root,
            PathObservationNamespace::Materialization(*observation_instance),
        ),
    };
    let value = NormalizedAbsolutePath::new(
        root.join(address.package().as_str())
            .join(address.target().as_str()),
    )
    .map_err(|_| HostRepositoryLabelPathError::InvalidMaterializedPath)
    .and_then(|path| HostRepositoryLabelPathValue::new(path, namespace));
    SourcePreparationOutcome::Complete(complete(value))
}

async fn drive_legacy(
    ctx: &mut DiceComputations<'_>,
    key: &HostRepositoryLabelPathKey,
) -> SourcePreparationOutcome<LabelPathResult> {
    match &key.source {
        HostRepositoryLabelPathSource::Root(workspace) => match ctx
            .compute(&HostRootPackageLookupKey::new(
                workspace.dupe(),
                key.address.package().clone(),
            ))
            .await
        {
            Ok(PathOutcome::Need(need)) => {
                SourcePreparationOutcome::Need(SourcePreparationNeeds::path(need))
            }
            Ok(PathOutcome::Complete(value)) => SourcePreparationOutcome::Complete(complete(
                root_lookup_result(value.as_ref().clone(), &key.address).and_then(|path| {
                    HostRepositoryLabelPathValue::new(path, PathObservationNamespace::Host)
                }),
            )),
            Err(error) => SourcePreparationOutcome::Complete(complete(Err(
                HostRepositoryLabelPathError::RootPackageLookup(Arc::from(error.to_string())),
            ))),
        },
        HostRepositoryLabelPathSource::External(route) => {
            let lookup = ExternalRepositoryPackageLookupKey::from_source_route(
                route.clone(),
                key.address.package.clone(),
            )
            .expect("label-path key validates its external repository");
            match ctx.compute(&lookup).await {
                Ok(SourcePreparationOutcome::Need(need)) => SourcePreparationOutcome::Need(need),
                Ok(SourcePreparationOutcome::Complete(result)) => {
                    match external_lookup_result(result.as_ref().clone()) {
                        Ok(()) => external_value(ctx, route, &key.address).await,
                        Err(error) => SourcePreparationOutcome::Complete(complete(Err(error))),
                    }
                }
                Err(error) => SourcePreparationOutcome::Complete(complete(Err(
                    HostRepositoryLabelPathError::ExternalPackageLookup(Arc::from(
                        error.to_string(),
                    )),
                ))),
            }
        }
    }
}

#[async_trait]
impl Key for HostRepositoryLabelPathKey {
    type Value = SourcePreparationOutcome<LabelPathResult>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        drive_legacy(ctx, self).await
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }
    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[async_trait]
impl Key for HostRepositoryLabelPathObservationKey {
    type Value = ObservedLabelPathResult;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match &self.0.source {
            HostRepositoryLabelPathSource::Root(workspace) => match ctx
                .compute(&HostRootPackageLookupObservationKey::new(
                    workspace.dupe(),
                    self.0.address.package().clone(),
                ))
                .await
            {
                Ok(PathOutcome::Need(need)) => {
                    SourcePreparationOutcome::Need(SourcePreparationNeeds::path(need))
                }
                Ok(PathOutcome::Complete(Err(error))) => {
                    SourcePreparationOutcome::Complete(Err(error))
                }
                Ok(PathOutcome::Complete(Ok(lookup))) => observed_complete(
                    root_lookup_result(lookup.result().clone(), &self.0.address).and_then(|path| {
                        HostRepositoryLabelPathValue::new(path, PathObservationNamespace::Host)
                    }),
                    lookup.observations().dupe(),
                ),
                Err(error) => observed_complete(
                    Err(HostRepositoryLabelPathError::RootPackageLookup(Arc::from(
                        error.to_string(),
                    ))),
                    PathObservationEpoch::empty(),
                ),
            },
            HostRepositoryLabelPathSource::External(route) => {
                let lookup = ExternalRepositoryPackageLookupObservationKey::from_source_route(
                    route.clone(),
                    self.0.address.package.clone(),
                )
                .expect("label-path key validates its external repository");
                match ctx.compute(&lookup).await {
                    Ok(SourcePreparationOutcome::Need(need)) => {
                        SourcePreparationOutcome::Need(need)
                    }
                    Ok(SourcePreparationOutcome::Complete(Err(error))) => {
                        SourcePreparationOutcome::Complete(Err(error))
                    }
                    Ok(SourcePreparationOutcome::Complete(Ok(lookup))) => {
                        match external_lookup_result(lookup.result().as_ref().clone()) {
                            Err(error) => {
                                observed_complete(Err(error), lookup.observations().dupe())
                            }
                            Ok(()) => match external_value(ctx, route, &self.0.address).await {
                                SourcePreparationOutcome::Need(need) => {
                                    SourcePreparationOutcome::Need(need)
                                }
                                SourcePreparationOutcome::Complete(result) => observed_complete(
                                    result.as_ref().clone(),
                                    lookup.observations().dupe(),
                                ),
                            },
                        }
                    }
                    Err(error) => observed_complete(
                        Err(HostRepositoryLabelPathError::ExternalPackageLookup(
                            Arc::from(error.to_string()),
                        )),
                        PathObservationEpoch::empty(),
                    ),
                }
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
    use std::path::PathBuf;
    use std::sync::Arc;

    use compact_str::CompactString;
    use dice::DetectCycles;
    use dice::Dice;
    use dice::Key;
    use slug_identity_v2::ApparentRepoName;
    use slug_identity_v2::CanonicalLabel;
    use slug_identity_v2::CanonicalRepoName;
    use slug_workspace_v2::NormalizedAbsolutePath;
    use slug_workspace_v2::PathLstat;
    use slug_workspace_v2::PathNodeKind;
    use slug_workspace_v2::PathObservationDemand;
    use slug_workspace_v2::PathObservationEpoch;
    use slug_workspace_v2::PathObservationEpochKey;
    use slug_workspace_v2::PathObservationInstanceId;
    use slug_workspace_v2::PathObservationNamespace;
    use slug_workspace_v2::PathObservationOperation;
    use slug_workspace_v2::PathObservationResult;
    use slug_workspace_v2::PathOperationResult;
    use starlark_map::small_map::SmallMap;

    use super::HostRepositoryLabelPathKey;
    use super::HostRepositoryLabelPathObservationKey;
    use super::HostRepositoryLabelPathValue;
    use super::RepositoryLabelPathAddress;
    use crate::GeneratedRepositoryFileEffectPlan;
    use crate::HostCanonicalRepositoryRoute;
    use crate::HostRepositoryMaterializationDisposition;
    use crate::HostRepositorySourceRoute;
    use crate::HostSelectedExtensionOwner;
    use crate::OverrideAttributeValue;
    use crate::RepoRuleId;
    use crate::RepoSpec;
    use crate::RepositoryMaterialization;
    use crate::RootPackagePolicyInputs;
    use crate::RootRepositoryRoute;
    use crate::SourcePreparationOutcome;
    use crate::host_canonical_repository_source_input;
    use crate::host_package::ExternalRepositoryPackageLookup;
    use crate::host_package::ExternalRepositoryPackageLookupKey;
    use crate::host_package::ExternalRepositoryPackageLookupObservationKey;
    use crate::host_package::HostBuildFileName;
    use crate::host_package::ObservedExternalRepositoryPackageLookup;
    use crate::inject_root_package_policy_inputs;
    use crate::source_preparation::RepositoryMaterializationResultKey;

    fn path(value: &str) -> NormalizedAbsolutePath {
        NormalizedAbsolutePath::new(value).unwrap()
    }

    fn lstat(
        value: &str,
        result: PathOperationResult<PathLstat>,
    ) -> (PathObservationDemand, PathObservationResult) {
        (
            PathObservationDemand::new(
                PathObservationNamespace::Host,
                path(value),
                PathObservationOperation::Lstat,
            ),
            PathObservationResult::Lstat(result),
        )
    }

    fn present(value: &str, kind: PathNodeKind) -> (PathObservationDemand, PathObservationResult) {
        lstat(
            value,
            PathOperationResult::Present(PathLstat::new(kind, 1, 1, 1, 1, 0o755)),
        )
    }

    fn missing(value: &str) -> (PathObservationDemand, PathObservationResult) {
        lstat(value, PathOperationResult::Missing)
    }

    fn repo_spec() -> RepoSpec {
        RepoSpec {
            rule_id: RepoRuleId {
                bzl_file: CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:local.bzl")
                    .unwrap(),
                rule_name: "local_repository".into(),
            },
            attributes: Arc::new(SmallMap::from_iter([(
                CompactString::new("path"),
                OverrideAttributeValue::String("dep".into()),
            )])),
        }
    }

    fn local_route() -> HostRepositorySourceRoute {
        HostRepositorySourceRoute::root(RootRepositoryRoute::for_test(
            path("/workspace"),
            ApparentRepoName::new("dep").unwrap(),
            "dep".into(),
            CanonicalRepoName::new("dep+").unwrap(),
            repo_spec(),
        ))
    }

    fn immutable_route() -> HostRepositorySourceRoute {
        let canonical_repo = CanonicalRepoName::new("generated+").unwrap();
        let route = HostCanonicalRepositoryRoute::generated(
            path("/workspace"),
            canonical_repo.clone(),
            Arc::new(HostSelectedExtensionOwner::testing("+extension")),
            0,
            "generated",
            repo_spec(),
            canonical_repo,
            SmallMap::new(),
        )
        .unwrap();
        HostRepositorySourceRoute::canonical(
            host_canonical_repository_source_input(
                Arc::new(route),
                Some(GeneratedRepositoryFileEffectPlan::build([]).unwrap()),
            )
            .unwrap(),
        )
    }

    fn external_address(route: &HostRepositorySourceRoute) -> RepositoryLabelPathAddress {
        RepositoryLabelPathAddress::from_label(
            &CanonicalLabel::parse(&format!(
                "@@{}//pkg:missing",
                route.canonical_repo().as_str()
            ))
            .unwrap(),
        )
    }

    fn materialization_key(
        route: &HostRepositorySourceRoute,
    ) -> RepositoryMaterializationResultKey {
        let HostRepositoryMaterializationDisposition::Request(request) =
            route.materialization_disposition().unwrap()
        else {
            panic!("test route must materialize")
        };
        RepositoryMaterializationResultKey { request }
    }

    async fn external_transaction(
        dice: &Arc<Dice>,
        route: &HostRepositorySourceRoute,
        address: &RepositoryLabelPathAddress,
        materialization: RepositoryMaterialization,
    ) -> dice::DiceTransaction {
        let lookup = ExternalRepositoryPackageLookupKey::from_source_route(
            route.clone(),
            address.package.clone(),
        )
        .unwrap();
        let observed_lookup = ExternalRepositoryPackageLookupObservationKey::from_source_route(
            route.clone(),
            address.package.clone(),
        )
        .unwrap();
        let marker = PathObservationEpoch::new([present(
            "/materialized/pkg/BUILD.bazel",
            PathNodeKind::RegularFile,
        )])
        .unwrap();
        let mut updater = dice.updater();
        updater
            .changed_to(vec![(
                lookup,
                SourcePreparationOutcome::Complete(Arc::new(Ok(
                    ExternalRepositoryPackageLookup::Package(HostBuildFileName::BuildDotBazel),
                ))),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                observed_lookup,
                SourcePreparationOutcome::Complete(Ok(
                    ObservedExternalRepositoryPackageLookup::for_test(
                        Ok(ExternalRepositoryPackageLookup::Package(
                            HostBuildFileName::BuildDotBazel,
                        )),
                        marker,
                    ),
                )),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                materialization_key(route),
                SourcePreparationOutcome::Complete(Arc::new(Ok(materialization))),
            )])
            .unwrap();
        updater.commit().await
    }

    #[test]
    fn address_is_mapping_free_and_value_identity_includes_namespace() {
        let label = CanonicalLabel::parse("@@repo+//pkg:sub/missing").unwrap();
        let address = RepositoryLabelPathAddress::from_label(&label);
        assert_eq!(address.repo().as_str(), "repo+");
        assert_eq!(address.package().as_str(), "pkg");
        assert_eq!(address.target().as_str(), "sub/missing");
        assert!(
            HostRepositoryLabelPathKey::new_root(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
                address
            )
            .is_err()
        );

        let path = NormalizedAbsolutePath::new("/workspace/pkg/sub/missing").unwrap();
        let host = HostRepositoryLabelPathValue::new(path.clone(), PathObservationNamespace::Host)
            .unwrap();
        let materialized = HostRepositoryLabelPathValue::new(
            path,
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(1)),
        )
        .unwrap();
        assert_ne!(host, materialized);
        assert!(std::mem::size_of::<RepositoryLabelPathAddress>() <= 384);
        assert!(std::mem::size_of::<HostRepositoryLabelPathKey>() <= 384);
        assert!(std::mem::size_of::<HostRepositoryLabelPathValue>() <= 128);
        let mut hasher = DefaultHasher::new();
        host.hash(&mut hasher);
    }

    #[test]
    fn constructors_and_package_dispositions_fail_closed() {
        let production = include_str!("repository_label_path.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        for forbidden in [
            "HostRepositoryPathKey",
            "ResolvedPathKey",
            "PathObservationOperation",
            "std::fs",
        ] {
            assert!(
                !production.contains(forbidden),
                "forbidden target owner: {forbidden}"
            );
        }
        let root = RepositoryLabelPathAddress::from_label(
            &CanonicalLabel::parse("@@//pkg:target").unwrap(),
        );
        let external = RepositoryLabelPathAddress::from_label(
            &CanonicalLabel::parse("@@dep+//pkg:target").unwrap(),
        );
        assert!(matches!(
            HostRepositoryLabelPathKey::new_root(path("/workspace"), external.clone()),
            Err(super::HostRepositoryLabelPathError::RootRepositoryMismatch(
                _
            ))
        ));
        assert!(matches!(
            HostRepositoryLabelPathKey::new_external(local_route(), root),
            Err(super::HostRepositoryLabelPathError::ExternalRepositoryMismatch(_, _))
        ));
        for (lookup, expected) in [
            (
                ExternalRepositoryPackageLookup::NoBuildFile,
                super::HostRepositoryLabelPathError::PackageAbsent,
            ),
            (
                ExternalRepositoryPackageLookup::Deleted,
                super::HostRepositoryLabelPathError::PackageDeleted,
            ),
            (
                ExternalRepositoryPackageLookup::IgnoredDirectory,
                super::HostRepositoryLabelPathError::PackageIgnored,
            ),
            (
                ExternalRepositoryPackageLookup::InvalidPackageName {
                    message: Arc::from("invalid"),
                },
                super::HostRepositoryLabelPathError::InvalidPackageName(Arc::from("invalid")),
            ),
        ] {
            assert_eq!(super::external_lookup_result(Ok(lookup)), Err(expected));
        }

        #[cfg(unix)]
        {
            use std::ffi::OsString;
            use std::os::unix::ffi::OsStringExt;
            let path =
                NormalizedAbsolutePath::new(PathBuf::from(OsString::from_vec(vec![b'/', 0xff])))
                    .unwrap();
            assert!(matches!(
                HostRepositoryLabelPathValue::new(path, PathObservationNamespace::Host),
                Err(super::HostRepositoryLabelPathError::NonUnicodePhysicalPath)
            ));
        }
    }

    #[tokio::test]
    async fn external_roots_namespaces_needs_builtin_and_a_b_a_are_owned() {
        let local_route = local_route();
        let local_address = external_address(&local_route);
        let local_materialization = RepositoryMaterialization::Local {
            canonical_repo: local_address.repo().clone(),
            repo_spec: repo_spec(),
            source_root: PathBuf::from("/local-root"),
        };
        let local_dice = Dice::builder().build(DetectCycles::Enabled);
        let mut local = external_transaction(
            &local_dice,
            &local_route,
            &local_address,
            local_materialization,
        )
        .await;
        let local_key =
            HostRepositoryLabelPathKey::new_external(local_route, local_address).unwrap();
        let SourcePreparationOutcome::Complete(local) = local.compute(&local_key).await.unwrap()
        else {
            panic!("injected direct-local path must complete")
        };
        let local = local.as_ref().as_ref().unwrap();
        assert_eq!(local.path_str(), "/local-root/pkg/missing");
        assert_eq!(local.namespace(), PathObservationNamespace::Host);

        let route = immutable_route();
        let address = external_address(&route);
        let instance = PathObservationInstanceId::new(41);
        let immutable = |root: &str| RepositoryMaterialization::Immutable {
            canonical_repo: address.repo().clone(),
            repo_spec: repo_spec(),
            source_identity: Arc::from(root),
            generation_root: PathBuf::from(root),
            observation_instance: instance,
        };
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut transaction = external_transaction(&dice, &route, &address, immutable("/a")).await;
        let key = HostRepositoryLabelPathKey::new_external(route.clone(), address.clone()).unwrap();
        let observed_key =
            HostRepositoryLabelPathObservationKey::new_external(route.clone(), address.clone())
                .unwrap();
        let mut values = Vec::new();
        for root in ["/a", "/b", "/a"] {
            if root != "/a" || !values.is_empty() {
                let mut updater = transaction.into_updater();
                updater
                    .changed_to(vec![(
                        materialization_key(&route),
                        SourcePreparationOutcome::Complete(Arc::new(Ok(immutable(root)))),
                    )])
                    .unwrap();
                transaction = updater.commit().await;
            }
            let SourcePreparationOutcome::Complete(value) =
                transaction.compute(&key).await.unwrap()
            else {
                panic!("injected immutable path must complete")
            };
            values.push(value);
        }
        assert_eq!(values[0], values[2]);
        assert_ne!(values[0], values[1]);
        let immutable = values[2].as_ref().as_ref().unwrap();
        assert_eq!(immutable.path_str(), "/a/pkg/missing");
        assert_eq!(
            immutable.namespace(),
            PathObservationNamespace::Materialization(instance)
        );
        let SourcePreparationOutcome::Complete(Ok(observed)) =
            transaction.compute(&observed_key).await.unwrap()
        else {
            panic!("observed immutable path must complete")
        };
        assert_eq!(observed.result(), &values[2]);
        assert_eq!(observed.observations().observations().len(), 1);

        let need_dice = Dice::builder().build(DetectCycles::Enabled);
        let lookup = ExternalRepositoryPackageLookupKey::from_source_route(
            route.clone(),
            address.package.clone(),
        )
        .unwrap();
        let mut updater = need_dice.updater();
        updater
            .changed_to(vec![(
                lookup,
                SourcePreparationOutcome::Complete(Arc::new(Ok(
                    ExternalRepositoryPackageLookup::Package(HostBuildFileName::BuildDotBazel),
                ))),
            )])
            .unwrap();
        let mut need_transaction = updater.commit().await;
        let need = need_transaction.compute(&key).await.unwrap();
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        assert!(!HostRepositoryLabelPathKey::validity(&need));

        let builtin_route = HostRepositorySourceRoute::root(RootRepositoryRoute::builtin_for_test(
            path("/workspace"),
        ));
        let builtin_address = external_address(&builtin_route);
        let builtin_lookup = ExternalRepositoryPackageLookupKey::from_source_route(
            builtin_route.clone(),
            builtin_address.package.clone(),
        )
        .unwrap();
        let builtin_key =
            HostRepositoryLabelPathKey::new_external(builtin_route, builtin_address).unwrap();
        let builtin_dice = Dice::builder().build(DetectCycles::Enabled);
        let mut updater = builtin_dice.updater();
        updater
            .changed_to(vec![(
                builtin_lookup,
                SourcePreparationOutcome::Complete(Arc::new(Ok(
                    ExternalRepositoryPackageLookup::Package(HostBuildFileName::BuildDotBazel),
                ))),
            )])
            .unwrap();
        let mut builtin = updater.commit().await;
        let SourcePreparationOutcome::Complete(result) =
            builtin.compute(&builtin_key).await.unwrap()
        else {
            panic!("built-in path must fail terminally")
        };
        assert!(matches!(
            result.as_ref(),
            Err(super::HostRepositoryLabelPathError::BuiltinCatalog)
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn root_uses_selected_package_root_without_observing_target() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut updater = dice.updater();
        inject_root_package_policy_inputs(
            &mut updater,
            RootPackagePolicyInputs::new(
                path("/workspace"),
                vec![path("/first"), path("/second")],
                std::iter::empty::<&str>(),
                None,
                Some("warning"),
            )
            .unwrap(),
        )
        .unwrap();
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                PathObservationEpoch::new([
                    present("/", PathNodeKind::Directory),
                    present("/workspace", PathNodeKind::Directory),
                    missing("/workspace/REPO.bazel"),
                    present("/first", PathNodeKind::Directory),
                    missing("/first/.bazelignore"),
                    present("/first/pkg", PathNodeKind::Directory),
                    missing("/first/pkg/BUILD.bazel"),
                    missing("/first/pkg/BUILD"),
                    present("/second", PathNodeKind::Directory),
                    missing("/second/.bazelignore"),
                    present("/second/pkg", PathNodeKind::Directory),
                    present("/second/pkg/BUILD.bazel", PathNodeKind::RegularFile),
                ])
                .unwrap(),
            )])
            .unwrap();
        let mut transaction = updater.commit().await;
        let address = RepositoryLabelPathAddress::from_label(
            &CanonicalLabel::parse("@@//pkg:sub/missing").unwrap(),
        );
        let key =
            HostRepositoryLabelPathKey::new_root(path("/workspace"), address.clone()).unwrap();
        let result = match transaction.compute(&key).await.unwrap() {
            SourcePreparationOutcome::Complete(result) => result,
            SourcePreparationOutcome::Need(need) => panic!("unexpected path need: {need:?}"),
        };
        let result = result.as_ref().as_ref().unwrap();
        assert_eq!(result.path_str(), "/second/pkg/sub/missing");
        assert_eq!(result.namespace(), PathObservationNamespace::Host);

        let observed =
            HostRepositoryLabelPathObservationKey::new_root(path("/workspace"), address).unwrap();
        let SourcePreparationOutcome::Complete(Ok(observed)) =
            transaction.compute(&observed).await.unwrap()
        else {
            panic!("unexpected observed path need")
        };
        assert_eq!(observed.result().as_ref().as_ref().unwrap(), result);
        assert!(
            observed
                .observations()
                .observations()
                .keys()
                .all(|demand| !demand.path().as_path().ends_with("sub/missing"))
        );
    }
}
