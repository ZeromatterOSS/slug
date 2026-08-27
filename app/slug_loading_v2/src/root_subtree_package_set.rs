/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Loading-owned recursive package discovery for the root repository.

use std::fmt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use compact_str::CompactString;
use dice::CancellationContext;
use dice::DiceComputations;
use dice::Key;
use dupe::Dupe;
use slug_bzlmod_v2::HostRootPackageBoundaryKey;
use slug_bzlmod_v2::HostRootPackageBoundaryKind;
use slug_bzlmod_v2::HostRootPackageBoundaryObservationKey;
use slug_bzlmod_v2::RootPackageLookupInputsProjectionKey;
use slug_bzlmod_v2::SourcePreparationNeeds;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_identity_v2::PackagePath;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathDirectoryEntryKind;
use slug_workspace_v2::PathDirectoryListing;
use slug_workspace_v2::PathDirectoryListingKey;
use slug_workspace_v2::PathDirectoryListingObservationKey;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathOutcome;
use slug_workspace_v2::ResolvedPathKey;
use slug_workspace_v2::ResolvedPathObservationKey;
use slug_workspace_v2::ResolvedPathState;

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct RootSubtreePackageSet {
    packages: Arc<[CompactString]>,
}

impl RootSubtreePackageSet {
    pub fn packages(&self) -> &Arc<[CompactString]> {
        &self.packages
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative, Dupe)]
pub struct RootSubtreePackageSetError(Arc<str>);

impl RootSubtreePackageSetError {
    fn evaluation(error: impl fmt::Display) -> Self {
        Self(Arc::from(error.to_string()))
    }
}

impl fmt::Display for RootSubtreePackageSetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

type PackageSetValue = Arc<Result<RootSubtreePackageSet, RootSubtreePackageSetError>>;

#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct RootSubtreePackageSetKey {
    workspace: NormalizedAbsolutePath,
    prefix: PackagePath,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct RootSubtreePackageSetObservationKey(RootSubtreePackageSetKey);

#[derive(Debug, Clone, Eq, PartialEq, Allocative, Dupe)]
pub struct ObservedRootSubtreePackageSet {
    result: PackageSetValue,
    observations: PathObservationEpoch,
}

impl ObservedRootSubtreePackageSet {
    pub fn result(&self) -> &PackageSetValue {
        &self.result
    }

    pub fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

impl RootSubtreePackageSetKey {
    pub fn new(workspace: NormalizedAbsolutePath, prefix: PackagePath) -> Self {
        Self { workspace, prefix }
    }
}

impl RootSubtreePackageSetObservationKey {
    pub fn new(workspace: NormalizedAbsolutePath, prefix: PackagePath) -> Self {
        Self(RootSubtreePackageSetKey::new(workspace, prefix))
    }
}

impl fmt::Display for RootSubtreePackageSetKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "root-subtree-package-set://{}", self.prefix.as_str())
    }
}

impl fmt::Display for RootSubtreePackageSetObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ObservationMode {
    Legacy,
    Observed,
}

#[async_trait]
impl Key for RootSubtreePackageSetKey {
    type Value = SourcePreparationOutcome<PackageSetValue>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        match compute_root_subtree_packages(
            ctx,
            &self.workspace,
            &self.prefix,
            ObservationMode::Legacy,
        )
        .await
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok(value)) => {
                SourcePreparationOutcome::Complete(value.result)
            }
            SourcePreparationOutcome::Complete(Err(error)) => {
                panic!("legacy root subtree produced observed outer error: {error}")
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

type ObservedPackageSetValue =
    SourcePreparationOutcome<Result<ObservedRootSubtreePackageSet, ObservedPathFrontierError>>;

#[async_trait]
impl Key for RootSubtreePackageSetObservationKey {
    type Value = ObservedPackageSetValue;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        compute_root_subtree_packages(
            ctx,
            &self.0.workspace,
            &self.0.prefix,
            ObservationMode::Observed,
        )
        .await
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

fn union_source_need(
    accumulated: &mut Option<SourcePreparationNeeds>,
    next: SourcePreparationNeeds,
) {
    *accumulated = Some(match accumulated.take() {
        Some(existing) => existing
            .try_union(&next)
            .expect("root loading source Needs must be compatible"),
        None => next,
    });
}

fn merge_observations(
    current: &PathObservationEpoch,
    incoming: &PathObservationEpoch,
) -> Result<PathObservationEpoch, ObservedPathFrontierError> {
    PathObservationEpoch::from_shared(
        current
            .observations()
            .iter()
            .chain(incoming.observations())
            .map(|(demand, result)| (demand.dupe(), result.dupe())),
    )
    .map_err(ObservedPathFrontierError::from)
}

fn complete(
    result: Result<RootSubtreePackageSet, RootSubtreePackageSetError>,
    observations: PathObservationEpoch,
) -> ObservedPackageSetValue {
    SourcePreparationOutcome::Complete(Ok(ObservedRootSubtreePackageSet {
        result: Arc::new(result),
        observations,
    }))
}

enum PathBatchValue<T> {
    Need(SourcePreparationNeeds),
    Outer(ObservedPathFrontierError),
    Complete(Result<T, RootSubtreePackageSetError>, PathObservationEpoch),
}

async fn compute_root_subtree_packages(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    prefix: &PackagePath,
    mode: ObservationMode,
) -> ObservedPackageSetValue {
    let roots = match ctx
        .compute(&RootPackageLookupInputsProjectionKey::new(
            workspace.clone(),
        ))
        .await
    {
        Err(error) => {
            return complete(
                Err(RootSubtreePackageSetError::evaluation(error)),
                PathObservationEpoch::empty(),
            );
        }
        Ok(Err(error)) => {
            return complete(
                Err(RootSubtreePackageSetError::evaluation(error)),
                PathObservationEpoch::empty(),
            );
        }
        Ok(Ok(inputs)) => inputs.package_roots().to_vec(),
    };

    let mut observations = PathObservationEpoch::empty();
    let mut pending = vec![PathBuf::from(prefix.as_str())];
    let mut packages = Vec::new();
    while let Some(relative) = pending.pop() {
        let package_text = relative.to_str().map(|value| value.replace('\\', "/"));
        if let Some(package_text) = package_text.as_deref() {
            let package = match PackagePath::parse(package_text) {
                Ok(package) => package,
                Err(error) => {
                    return complete(
                        Err(RootSubtreePackageSetError::evaluation(error)),
                        observations,
                    );
                }
            };
            match mode {
                ObservationMode::Legacy => {
                    match ctx
                        .compute(&HostRootPackageBoundaryKey::new(workspace.clone(), package))
                        .await
                        .expect("Host package-boundary DICE invariant")
                    {
                        PathOutcome::Need(need) => {
                            return SourcePreparationOutcome::Need(SourcePreparationNeeds::path(
                                need,
                            ));
                        }
                        PathOutcome::Complete(value) => match value.as_ref() {
                            Err(error) => {
                                return complete(
                                    Err(RootSubtreePackageSetError::evaluation(error)),
                                    observations,
                                );
                            }
                            Ok(boundary)
                                if boundary.kind()
                                    == HostRootPackageBoundaryKind::IgnoredDirectory =>
                            {
                                continue;
                            }
                            Ok(boundary)
                                if boundary.kind() == HostRootPackageBoundaryKind::Package =>
                            {
                                packages.push(CompactString::new(package_text));
                            }
                            Ok(_) => {}
                        },
                    }
                }
                ObservationMode::Observed => {
                    match ctx
                        .compute(&HostRootPackageBoundaryObservationKey::new(
                            workspace.clone(),
                            package,
                        ))
                        .await
                        .expect("observed Host package-boundary DICE invariant")
                    {
                        PathOutcome::Need(need) => {
                            return SourcePreparationOutcome::Need(SourcePreparationNeeds::path(
                                need,
                            ));
                        }
                        PathOutcome::Complete(Err(error)) => {
                            return SourcePreparationOutcome::Complete(Err(error));
                        }
                        PathOutcome::Complete(Ok(value)) => {
                            observations =
                                match merge_observations(&observations, value.observations()) {
                                    Ok(observations) => observations,
                                    Err(error) => {
                                        return SourcePreparationOutcome::Complete(Err(error));
                                    }
                                };
                            match value.result() {
                                Err(error) => {
                                    return complete(
                                        Err(RootSubtreePackageSetError::evaluation(error)),
                                        observations,
                                    );
                                }
                                Ok(boundary)
                                    if boundary.kind()
                                        == HostRootPackageBoundaryKind::IgnoredDirectory =>
                                {
                                    continue;
                                }
                                Ok(boundary)
                                    if boundary.kind() == HostRootPackageBoundaryKind::Package =>
                                {
                                    packages.push(CompactString::new(package_text));
                                }
                                Ok(_) => {}
                            }
                        }
                    }
                }
            }
        } else {
            match probe_native_package_marker(ctx, &roots, &relative, mode, observations).await {
                SourcePreparationOutcome::Need(need) => {
                    return SourcePreparationOutcome::Need(need);
                }
                SourcePreparationOutcome::Complete(Err(error)) => {
                    return SourcePreparationOutcome::Complete(Err(error));
                }
                SourcePreparationOutcome::Complete(Ok((Err(error), reached))) => {
                    return complete(Err(error), reached);
                }
                SourcePreparationOutcome::Complete(Ok((Ok(true), reached))) => {
                    return complete(
                        Err(RootSubtreePackageSetError::evaluation(format!(
                            "package path is not UTF-8: {}",
                            relative.display()
                        ))),
                        reached,
                    );
                }
                SourcePreparationOutcome::Complete(Ok((Ok(false), reached))) => {
                    observations = reached;
                }
            }
        }

        let relative_display = relative.display().to_string();
        let listings = ctx
            .compute_join(roots.iter().cloned(), |ctx, root| {
                let logical = NormalizedAbsolutePath::new(root.as_path().join(&relative))
                    .expect("package-root child remains absolute");
                let relative_display = &relative_display;
                Box::pin(async move {
                    match mode {
                        ObservationMode::Legacy => match ctx
                            .compute(&PathDirectoryListingKey::new(
                                PathObservationNamespace::Host,
                                logical,
                            ))
                            .await
                        {
                            Err(error) => PathBatchValue::Complete(
                                Err(RootSubtreePackageSetError::evaluation(error)),
                                PathObservationEpoch::empty(),
                            ),
                            Ok(PathOutcome::Need(need)) => {
                                PathBatchValue::Need(SourcePreparationNeeds::path(need))
                            }
                            Ok(PathOutcome::Complete(value)) => PathBatchValue::Complete(
                                match value.as_ref() {
                                    Ok(value) => Ok(value.clone()),
                                    Err(error) => {
                                        Err(RootSubtreePackageSetError::evaluation(format!(
                                            "reading workspace directory {}: {error:?}",
                                            relative_display
                                        )))
                                    }
                                },
                                PathObservationEpoch::empty(),
                            ),
                        },
                        ObservationMode::Observed => match ctx
                            .compute(&PathDirectoryListingObservationKey::new(
                                PathObservationNamespace::Host,
                                logical,
                            ))
                            .await
                        {
                            Err(error) => PathBatchValue::Complete(
                                Err(RootSubtreePackageSetError::evaluation(error)),
                                PathObservationEpoch::empty(),
                            ),
                            Ok(PathOutcome::Need(need)) => {
                                PathBatchValue::Need(SourcePreparationNeeds::path(need))
                            }
                            Ok(PathOutcome::Complete(Err(error))) => PathBatchValue::Outer(error),
                            Ok(PathOutcome::Complete(Ok(value))) => PathBatchValue::Complete(
                                value.result().clone().map_err(|error| {
                                    RootSubtreePackageSetError::evaluation(format!(
                                        "reading workspace directory {}: {error:?}",
                                        relative_display
                                    ))
                                }),
                                value.observations().dupe(),
                            ),
                        },
                    }
                })
            })
            .await;
        let mut needs = None;
        let mut first_outer = None;
        let mut first_error = None;
        let mut children = Vec::new();
        for listing in listings {
            match listing {
                PathBatchValue::Need(need) => union_source_need(&mut needs, need),
                PathBatchValue::Outer(error) => {
                    first_outer.get_or_insert(error);
                }
                PathBatchValue::Complete(result, epoch) => {
                    match merge_observations(&observations, &epoch) {
                        Ok(reached) => observations = reached,
                        Err(error) => {
                            first_outer.get_or_insert(error);
                        }
                    }
                    match result {
                        Err(error) => {
                            first_error.get_or_insert(error);
                        }
                        Ok(PathDirectoryListing::Missing) => {}
                        Ok(PathDirectoryListing::Present(entries)) => {
                            children.extend(
                                entries
                                    .entries()
                                    .iter()
                                    .filter(|entry| {
                                        entry.kind() == PathDirectoryEntryKind::Directory
                                    })
                                    .map(|entry| relative.join(entry.name().as_os_str())),
                            );
                        }
                    }
                }
            }
        }
        if let Some(error) = first_outer {
            return SourcePreparationOutcome::Complete(Err(error));
        }
        if let Some(need) = needs {
            return SourcePreparationOutcome::Need(need);
        }
        if let Some(error) = first_error {
            return complete(Err(error), observations);
        }
        children.sort_unstable();
        children.dedup();
        pending.extend(children.into_iter().rev());
    }
    packages.sort_unstable();
    packages.dedup();
    complete(
        Ok(RootSubtreePackageSet {
            packages: packages.into(),
        }),
        observations,
    )
}

type MarkerProbeOutcome = SourcePreparationOutcome<
    Result<
        (
            Result<bool, RootSubtreePackageSetError>,
            PathObservationEpoch,
        ),
        ObservedPathFrontierError,
    >,
>;

async fn probe_native_package_marker(
    ctx: &mut DiceComputations<'_>,
    roots: &[NormalizedAbsolutePath],
    relative: &Path,
    mode: ObservationMode,
    mut observations: PathObservationEpoch,
) -> MarkerProbeOutcome {
    let probes = roots
        .iter()
        .flat_map(|root| {
            ["BUILD.bazel", "BUILD"].map(|basename| {
                NormalizedAbsolutePath::new(root.as_path().join(relative).join(basename))
                    .expect("package marker remains absolute")
            })
        })
        .collect::<Vec<_>>();
    let results = ctx
        .compute_join(probes, |ctx, marker| {
            Box::pin(async move {
                match mode {
                    ObservationMode::Legacy => match ctx
                        .compute(&ResolvedPathKey::new(
                            PathObservationNamespace::Host,
                            marker,
                        ))
                        .await
                    {
                        Err(error) => PathBatchValue::Complete(
                            Err(RootSubtreePackageSetError::evaluation(error)),
                            PathObservationEpoch::empty(),
                        ),
                        Ok(PathOutcome::Need(need)) => {
                            PathBatchValue::Need(SourcePreparationNeeds::path(need))
                        }
                        Ok(PathOutcome::Complete(result)) => PathBatchValue::Complete(
                            result
                                .as_ref()
                                .clone()
                                .map(|resolved| {
                                    matches!(
                                        resolved.state(),
                                        ResolvedPathState::Present(lstat)
                                            if matches!(
                                                lstat.kind(),
                                                PathNodeKind::RegularFile
                                                    | PathNodeKind::SpecialFile
                                            )
                                    )
                                })
                                .map_err(|error| {
                                    RootSubtreePackageSetError::evaluation(format!("{error:?}"))
                                }),
                            PathObservationEpoch::empty(),
                        ),
                    },
                    ObservationMode::Observed => match ctx
                        .compute(&ResolvedPathObservationKey::new(
                            PathObservationNamespace::Host,
                            marker,
                        ))
                        .await
                    {
                        Err(error) => PathBatchValue::Complete(
                            Err(RootSubtreePackageSetError::evaluation(error)),
                            PathObservationEpoch::empty(),
                        ),
                        Ok(PathOutcome::Need(need)) => {
                            PathBatchValue::Need(SourcePreparationNeeds::path(need))
                        }
                        Ok(PathOutcome::Complete(Err(error))) => PathBatchValue::Outer(error),
                        Ok(PathOutcome::Complete(Ok(value))) => PathBatchValue::Complete(
                            value
                                .result()
                                .clone()
                                .map(|resolved| {
                                    matches!(
                                        resolved.state(),
                                        ResolvedPathState::Present(lstat)
                                            if matches!(
                                                lstat.kind(),
                                                PathNodeKind::RegularFile
                                                    | PathNodeKind::SpecialFile
                                            )
                                    )
                                })
                                .map_err(|error| {
                                    RootSubtreePackageSetError::evaluation(format!("{error:?}"))
                                }),
                            value.observations().dupe(),
                        ),
                    },
                }
            })
        })
        .await;
    let mut needs = None;
    let mut first_outer = None;
    let mut first_terminal = None;
    for result in results {
        match result {
            PathBatchValue::Need(need) => union_source_need(&mut needs, need),
            PathBatchValue::Outer(error) => {
                first_outer.get_or_insert(error);
            }
            PathBatchValue::Complete(result, epoch) => {
                match merge_observations(&observations, &epoch) {
                    Ok(reached) => observations = reached,
                    Err(error) => {
                        first_outer.get_or_insert(error);
                    }
                }
                match result {
                    Err(error) => {
                        first_terminal.get_or_insert(Err(error));
                    }
                    Ok(true) => {
                        first_terminal.get_or_insert(Ok(true));
                    }
                    Ok(false) => {}
                }
            }
        }
    }
    if let Some(error) = first_outer {
        return SourcePreparationOutcome::Complete(Err(error));
    }
    if let Some(need) = needs {
        return SourcePreparationOutcome::Need(need);
    }
    if let Some(result) = first_terminal {
        return SourcePreparationOutcome::Complete(Ok((result, observations)));
    }
    SourcePreparationOutcome::Complete(Ok((Ok(false), observations)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_retain_prefix_and_display_without_query_policy() {
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let prefix = PackagePath::parse("tree/sub").unwrap();
        assert_eq!(
            RootSubtreePackageSetKey::new(workspace.clone(), prefix.clone()).to_string(),
            "root-subtree-package-set://tree/sub"
        );
        assert_eq!(
            RootSubtreePackageSetObservationKey::new(workspace, prefix).to_string(),
            "observed-root-subtree-package-set://tree/sub"
        );

        let packages = RootSubtreePackageSet {
            packages: Arc::from([CompactString::new("tree/sub")]),
        };
        assert_eq!(packages.packages().as_ref(), ["tree/sub"]);
        assert_eq!(
            RootSubtreePackageSetError::evaluation("terminal").to_string(),
            "terminal"
        );
    }
}
