/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory or the Apache License, Version 2.0
 * found in the LICENSE-APACHE file in the root directory. You may select,
 * at your option, one of the above-listed licenses.
 */

#![allow(dead_code)] // Dormant until the later Host root-module packet.

use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dice::DiceComputations;
use dupe::Dupe;
use slug_identity_v2::PackagePath;
use slug_workspace_v2::NeedPathObservations;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathOutcome;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::LogicalSpan;
use crate::NonrootIncludeRequest;
use crate::host_package::HostRootPackageLookup;
use crate::host_package::HostRootPackageLookupError;
use crate::host_package::HostRootPackageLookupKey;
use crate::host_package::HostRootPackageLookupObservationKey;
use crate::host_package::ObservedHostRootPackageLookup;
use crate::module_eval::ParsedRootInclude;
use crate::module_eval::parse_root_include;

type ObservedPreflight = PathOutcome<
    Result<
        (
            Arc<Result<HostRootIncludeHorizon, HostRootIncludeError>>,
            PathObservationEpoch,
        ),
        ObservedPathFrontierError,
    >,
>;

#[derive(Clone, Copy)]
enum RootIncludeFrontierMode {
    Legacy,
    Observed,
}

enum PackageLookupValue {
    Legacy(Arc<Result<HostRootPackageLookup, HostRootPackageLookupError>>),
    Observed(ObservedHostRootPackageLookup),
}

impl PackageLookupValue {
    fn result(&self) -> &Result<HostRootPackageLookup, HostRootPackageLookupError> {
        match self {
            Self::Legacy(value) => value,
            Self::Observed(value) => value.result(),
        }
    }

    fn observations(&self) -> Option<&PathObservationEpoch> {
        match self {
            Self::Legacy(_) => None,
            Self::Observed(value) => Some(value.observations()),
        }
    }
}

type PackageOutcome = PathOutcome<Result<PackageLookupValue, ObservedPathFrontierError>>;

fn first_seen_packages(parsed: &[ParsedRootInclude]) -> SmallSet<PackagePath> {
    let mut unique = SmallSet::with_capacity(parsed.len());
    for include in parsed {
        unique.insert(include.package().package().clone());
    }
    unique
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostRootIncludePackageFailure {
    NoBuildFile,
    Deleted,
    InvalidPackageName { message: Arc<str> },
    Operational(HostRootPackageLookupError),
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostRootIncludeError {
    BadLabel {
        raw_label: CompactString,
        location: LogicalSpan,
        message: CompactString,
    },
    Package {
        raw_label: CompactString,
        location: LogicalSpan,
        failure: HostRootIncludePackageFailure,
    },
}

impl fmt::Display for HostRootIncludeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for HostRootIncludeError {}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct HostRootInclude {
    include: ParsedRootInclude,
    logical_path: NormalizedAbsolutePath,
}

impl HostRootInclude {
    pub(crate) fn include(&self) -> &ParsedRootInclude {
        &self.include
    }

    pub(crate) fn logical_path(&self) -> &NormalizedAbsolutePath {
        &self.logical_path
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct HostRootIncludeHorizon {
    includes: Arc<[HostRootInclude]>,
}

impl HostRootIncludeHorizon {
    pub(crate) fn includes(&self) -> &[HostRootInclude] {
        &self.includes
    }
}

#[track_caller]
fn dice_invariant<T, E: fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| panic!("Host include-horizon DICE invariant failed: {error:?}"))
}

async fn compute_package_lookup(
    ctx: &mut DiceComputations<'_>,
    mode: RootIncludeFrontierMode,
    workspace: &NormalizedAbsolutePath,
    package: &PackagePath,
) -> PackageOutcome {
    match mode {
        RootIncludeFrontierMode::Legacy => {
            match dice_invariant(
                ctx.compute(&HostRootPackageLookupKey::new(
                    workspace.dupe(),
                    package.clone(),
                ))
                .await,
            ) {
                PathOutcome::Need(need) => PathOutcome::Need(need),
                PathOutcome::Complete(value) => {
                    PathOutcome::Complete(Ok(PackageLookupValue::Legacy(value)))
                }
            }
        }
        RootIncludeFrontierMode::Observed => {
            match dice_invariant(
                ctx.compute(&HostRootPackageLookupObservationKey::new(
                    workspace.dupe(),
                    package.clone(),
                ))
                .await,
            ) {
                PathOutcome::Need(need) => PathOutcome::Need(need),
                PathOutcome::Complete(Err(error)) => PathOutcome::Complete(Err(error)),
                PathOutcome::Complete(Ok(value)) => {
                    PathOutcome::Complete(Ok(PackageLookupValue::Observed(value)))
                }
            }
        }
    }
}

fn complete_preflight(
    result: Result<HostRootIncludeHorizon, HostRootIncludeError>,
    observations: PathObservationEpoch,
) -> ObservedPreflight {
    PathOutcome::Complete(Ok((Arc::new(result), observations)))
}

fn complete_package_error(
    include: ParsedRootInclude,
    failure: HostRootIncludePackageFailure,
    observations: PathObservationEpoch,
) -> ObservedPreflight {
    complete_preflight(
        Err(HostRootIncludeError::Package {
            raw_label: CompactString::new(include.raw_label()),
            location: include.location().clone(),
            failure,
        }),
        observations,
    )
}

fn union_observations(
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

async fn drive_root_include_preflight(
    ctx: &mut DiceComputations<'_>,
    mode: RootIncludeFrontierMode,
    workspace: &NormalizedAbsolutePath,
    requests: &[NonrootIncludeRequest],
) -> ObservedPreflight {
    let mut parsed = Vec::with_capacity(requests.len());
    for request in requests {
        match parse_root_include(request) {
            Ok(include) => parsed.push(include),
            Err(message) => {
                return complete_preflight(
                    Err(HostRootIncludeError::BadLabel {
                        raw_label: request.path.clone(),
                        location: request.location.clone(),
                        message,
                    }),
                    PathObservationEpoch::empty(),
                );
            }
        }
    }

    let unique = first_seen_packages(&parsed);
    let computed = ctx
        .compute_join(unique, |ctx, package| {
            Box::pin(async move {
                let outcome = compute_package_lookup(ctx, mode, workspace, &package).await;
                (package, outcome)
            })
        })
        .await;
    let outcomes = computed
        .into_iter()
        .collect::<SmallMap<_, PackageOutcome>>();
    let all_need: Option<NeedPathObservations> =
        outcomes.values().fold(None, |need, outcome| match outcome {
            PathOutcome::Need(incoming) => Some(match need {
                Some(current) => current.union(incoming),
                None => incoming.dupe(),
            }),
            PathOutcome::Complete(_) => need,
        });

    let mut observations = PathObservationEpoch::empty();
    let mut resolved = Vec::with_capacity(parsed.len());
    for include in parsed {
        let package_path = include.package().package();
        let value = match outcomes
            .get(package_path)
            .expect("every parsed include package was computed")
        {
            PathOutcome::Need(_) => {
                return PathOutcome::Need(
                    all_need.expect("the current package contributed a nonempty Need"),
                );
            }
            PathOutcome::Complete(Err(error)) => {
                return PathOutcome::Complete(Err(error.dupe()));
            }
            PathOutcome::Complete(Ok(value)) => value,
        };
        if let Some(incoming) = value.observations() {
            observations = match union_observations(&observations, incoming) {
                Ok(observations) => observations,
                Err(error) => return PathOutcome::Complete(Err(error)),
            };
        }
        let package_root = match value.result() {
            Ok(HostRootPackageLookup::Package(package)) => package.package_root(),
            Ok(HostRootPackageLookup::NoBuildFile) => {
                return complete_package_error(
                    include,
                    HostRootIncludePackageFailure::NoBuildFile,
                    observations,
                );
            }
            Ok(HostRootPackageLookup::Deleted) => {
                return complete_package_error(
                    include,
                    HostRootIncludePackageFailure::Deleted,
                    observations,
                );
            }
            Ok(HostRootPackageLookup::InvalidPackageName { message }) => {
                return complete_package_error(
                    include,
                    HostRootIncludePackageFailure::InvalidPackageName {
                        message: message.dupe(),
                    },
                    observations,
                );
            }
            Err(error) => {
                return complete_package_error(
                    include,
                    HostRootIncludePackageFailure::Operational(error.clone()),
                    observations,
                );
            }
        };
        let logical_path = NormalizedAbsolutePath::new(
            package_root
                .as_path()
                .join(package_path.as_str())
                .join(include.target().as_str()),
        )
        .expect("validated root include path remains normalized absolute");
        resolved.push(HostRootInclude {
            include,
            logical_path,
        });
    }
    complete_preflight(
        Ok(HostRootIncludeHorizon {
            includes: resolved.into(),
        }),
        observations,
    )
}

pub(crate) async fn preflight_root_include_horizon(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    requests: &[NonrootIncludeRequest],
) -> PathOutcome<Arc<Result<HostRootIncludeHorizon, HostRootIncludeError>>> {
    match drive_root_include_preflight(ctx, RootIncludeFrontierMode::Legacy, workspace, requests)
        .await
    {
        PathOutcome::Need(need) => PathOutcome::Need(need),
        PathOutcome::Complete(Ok((result, _observations))) => PathOutcome::Complete(result),
        PathOutcome::Complete(Err(error)) => {
            panic!("legacy include preflight received frontier error: {error}")
        }
    }
}

pub(crate) async fn preflight_root_include_horizon_observed(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    requests: &[NonrootIncludeRequest],
) -> ObservedPreflight {
    drive_root_include_preflight(ctx, RootIncludeFrontierMode::Observed, workspace, requests).await
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::fmt;
    #[cfg(unix)]
    use std::hash::Hash;
    #[cfg(unix)]
    use std::hash::Hasher;
    #[cfg(unix)]
    use std::sync::Arc;
    #[cfg(unix)]
    use std::sync::atomic::AtomicUsize;
    #[cfg(unix)]
    use std::sync::atomic::Ordering;

    #[cfg(unix)]
    use allocative::Allocative;
    #[cfg(unix)]
    use async_trait::async_trait;
    use compact_str::CompactString;
    #[cfg(unix)]
    use dice::DetectCycles;
    #[cfg(unix)]
    use dice::Dice;
    #[cfg(unix)]
    use dice::DiceComputations;
    #[cfg(unix)]
    use dice::DiceTransaction;
    #[cfg(unix)]
    use dice::Key;
    #[cfg(unix)]
    use dice_futures::cancellation::CancellationContext;
    #[cfg(unix)]
    use dupe::Dupe;
    #[cfg(unix)]
    use slug_workspace_v2::NormalizedAbsolutePath;
    #[cfg(unix)]
    use slug_workspace_v2::PathLstat;
    #[cfg(unix)]
    use slug_workspace_v2::PathNodeKind;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationDemand;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationEpoch;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationEpochError;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationEpochKey;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationNamespace;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationOperation;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationResult;
    #[cfg(unix)]
    use slug_workspace_v2::PathOperationResult;
    #[cfg(unix)]
    use slug_workspace_v2::PathOutcome;

    #[cfg(unix)]
    use super::HostRootIncludeError;
    #[cfg(unix)]
    use super::HostRootIncludeHorizon;
    #[cfg(unix)]
    use super::HostRootIncludePackageFailure;
    use super::first_seen_packages;
    #[cfg(unix)]
    use super::preflight_root_include_horizon;
    #[cfg(unix)]
    use super::preflight_root_include_horizon_observed;
    #[cfg(unix)]
    use super::union_observations;
    use crate::LogicalModuleFileId;
    use crate::LogicalSpan;
    use crate::NonrootIncludeRequest;
    #[cfg(unix)]
    use crate::RootPackagePolicyInputs;
    #[cfg(unix)]
    use crate::inject_root_package_policy_inputs;
    use crate::module_eval::parse_root_include;

    fn request(label: &str, line: u32) -> NonrootIncludeRequest {
        NonrootIncludeRequest {
            path: CompactString::new(label),
            location: LogicalSpan {
                file: LogicalModuleFileId::new("MODULE.bazel"),
                start_line: line,
                start_column: 1,
                end_line: line,
                end_column: 20,
            },
        }
    }

    #[test]
    fn parsed_root_include_preserves_canonical_identity_and_exact_boundaries() {
        let nested = request("//pkg:dir/child.MODULE.bazel/.", 7);
        let parsed = parse_root_include(&nested).unwrap();
        assert!(parsed.package().repo().is_root());
        assert_eq!(parsed.package().package().as_str(), "pkg");
        assert_eq!(parsed.target().as_str(), "dir/child.MODULE.bazel");
        assert_eq!(parsed.raw_label(), nested.path.as_str());
        assert_eq!(parsed.location(), &nested.location);

        let colonless = parse_root_include(&request("//pkg/file.MODULE.bazel", 8)).unwrap();
        assert_eq!(
            colonless.package().package().as_str(),
            "pkg/file.MODULE.bazel"
        );
        assert_eq!(colonless.target().as_str(), "file.MODULE.bazel");

        let grouped = [
            parse_root_include(&request("//a:first.MODULE.bazel", 1)).unwrap(),
            parse_root_include(&request("//a:second.MODULE.bazel", 2)).unwrap(),
            parse_root_include(&request("//b:third.MODULE.bazel", 3)).unwrap(),
        ];
        assert_eq!(
            first_seen_packages(&grouped)
                .iter()
                .map(|package| package.as_str())
                .collect::<Vec<_>>(),
            ["a", "b"]
        );

        for label in [
            "//pkg:",
            "@//pkg:file.MODULE.bazel",
            "@@//pkg:file.MODULE.bazel",
            "@repo//pkg:file.MODULE.bazel",
            ":relative.MODULE.bazel",
            r"//pkg:bad\name.MODULE.bazel",
            "//pkg:../bad.MODULE.bazel",
            "//pkg:dir/.child.MODULE.bazel",
            "//pkg:bad.txt",
            "//...:ok.MODULE.bazel",
        ] {
            assert!(parse_root_include(&request(label, 1)).is_err(), "{label}");
        }
    }

    #[cfg(unix)]
    type ScriptEntry = (PathObservationDemand, PathObservationResult);

    #[cfg(unix)]
    fn path(value: &str) -> NormalizedAbsolutePath {
        NormalizedAbsolutePath::new(value).unwrap()
    }

    #[cfg(unix)]
    fn demand(value: &str) -> PathObservationDemand {
        PathObservationDemand::new(
            PathObservationNamespace::Host,
            path(value),
            PathObservationOperation::Lstat,
        )
    }

    #[cfg(unix)]
    fn present(value: &str, kind: PathNodeKind, variant: i64) -> ScriptEntry {
        (
            demand(value),
            PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                kind, variant, variant, variant, variant, 0o755,
            ))),
        )
    }

    #[cfg(unix)]
    fn missing(value: &str) -> ScriptEntry {
        (
            demand(value),
            PathObservationResult::Lstat(PathOperationResult::Missing),
        )
    }

    #[cfg(unix)]
    fn prelude(roots: &[&str]) -> Vec<ScriptEntry> {
        let mut entries = vec![
            present("/", PathNodeKind::Directory, 1),
            present("/workspace", PathNodeKind::Directory, 1),
            missing("/workspace/REPO.bazel"),
        ];
        for root in roots {
            entries.push(present(root, PathNodeKind::Directory, 1));
            entries.push(missing(&format!("{root}/.bazelignore")));
        }
        entries
    }

    #[cfg(unix)]
    fn policy(roots: &[&str], deleted: &[&str]) -> RootPackagePolicyInputs {
        RootPackagePolicyInputs::new(
            path("/workspace"),
            roots.iter().map(|root| path(root)).collect::<Vec<_>>(),
            deleted,
            None,
            Some("warning"),
        )
        .unwrap()
    }

    #[cfg(unix)]
    fn epoch(entries: &[ScriptEntry]) -> PathObservationEpoch {
        PathObservationEpoch::new(
            entries
                .iter()
                .map(|(demand, result)| (demand.dupe(), result.dupe())),
        )
        .unwrap()
    }

    #[cfg(unix)]
    async fn preflight(
        inputs: Option<RootPackagePolicyInputs>,
        entries: Option<&[ScriptEntry]>,
        requests: &[NonrootIncludeRequest],
    ) -> PathOutcome<Arc<Result<HostRootIncludeHorizon, HostRootIncludeError>>> {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut updater = dice.updater();
        if let Some(inputs) = inputs {
            inject_root_package_policy_inputs(&mut updater, inputs).unwrap();
        }
        if let Some(entries) = entries {
            updater
                .changed_to(vec![(PathObservationEpochKey, epoch(entries))])
                .unwrap();
        }
        let mut transaction = updater.commit().await;
        preflight_root_include_horizon(&mut transaction, &path("/workspace"), requests).await
    }

    #[cfg(unix)]
    async fn observed_preflight(
        inputs: RootPackagePolicyInputs,
        observations: PathObservationEpoch,
        requests: &[NonrootIncludeRequest],
    ) -> PathOutcome<
        Result<
            (
                Arc<Result<HostRootIncludeHorizon, HostRootIncludeError>>,
                PathObservationEpoch,
            ),
            slug_workspace_v2::ObservedPathFrontierError,
        >,
    > {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut updater = dice.updater();
        inject_root_package_policy_inputs(&mut updater, inputs).unwrap();
        updater
            .changed_to(vec![(PathObservationEpochKey, observations)])
            .unwrap();
        let mut transaction = updater.commit().await;
        preflight_root_include_horizon_observed(&mut transaction, &path("/workspace"), requests)
            .await
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn parses_the_whole_horizon_before_requesting_any_package_key() {
        let requests = [
            request("//valid:first.MODULE.bazel", 1),
            request("//pkg:", 2),
        ];
        let outcome = preflight(None, None, &requests).await;
        assert!(matches!(
            outcome,
            PathOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostRootIncludeError::BadLabel {
                        raw_label,
                        location,
                        ..
                    }) if raw_label == "//pkg:" && location.start_line == 2
                )
        ));

        let operational = preflight(None, None, &[request("//valid:file.MODULE.bazel", 9)]).await;
        assert!(matches!(
            operational,
            PathOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostRootIncludeError::Package {
                        raw_label,
                        location,
                        failure: HostRootIncludePackageFailure::Operational(
                            crate::host_package::HostRootPackageLookupError::PolicyInput(_)
                        ),
                    }) if raw_label == "//valid:file.MODULE.bazel"
                        && location.start_line == 9
                )
        ));
    }

    #[cfg(unix)]
    #[derive(Debug, Clone, Allocative)]
    struct HorizonCounterKey {
        workspace: NormalizedAbsolutePath,
        requests: Arc<[NonrootIncludeRequest]>,
        #[allocative(skip)]
        counter: Arc<AtomicUsize>,
    }

    #[cfg(unix)]
    impl PartialEq for HorizonCounterKey {
        fn eq(&self, other: &Self) -> bool {
            self.workspace == other.workspace
                && self.requests == other.requests
                && Arc::ptr_eq(&self.counter, &other.counter)
        }
    }

    #[cfg(unix)]
    impl Eq for HorizonCounterKey {}

    #[cfg(unix)]
    impl Hash for HorizonCounterKey {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.workspace.hash(state);
            for request in self.requests.iter() {
                request.path.hash(state);
                request.location.hash(state);
            }
            Arc::as_ptr(&self.counter).hash(state);
        }
    }

    #[cfg(unix)]
    impl fmt::Display for HorizonCounterKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                f,
                "host-include-horizon-counter:{:p}",
                Arc::as_ptr(&self.counter)
            )
        }
    }

    #[cfg(unix)]
    #[async_trait]
    impl Key for HorizonCounterKey {
        type Value = PathOutcome<Arc<Result<usize, HostRootIncludeError>>>;

        async fn compute(
            &self,
            ctx: &mut DiceComputations,
            _cancellations: &CancellationContext,
        ) -> Self::Value {
            preflight_root_include_horizon(ctx, &self.workspace, &self.requests)
                .await
                .map(|result| {
                    Arc::new(match result.as_ref() {
                        Ok(_) => Ok(self.counter.fetch_add(1, Ordering::SeqCst) + 1),
                        Err(error) => Err(error.clone()),
                    })
                })
        }

        fn equality(x: &Self::Value, y: &Self::Value) -> bool {
            x.complete_eq(y)
        }

        fn validity(value: &Self::Value) -> bool {
            value.is_complete()
        }
    }

    #[cfg(unix)]
    async fn update_epoch(
        transaction: DiceTransaction,
        entries: &[ScriptEntry],
    ) -> DiceTransaction {
        let mut updater = transaction.into_updater();
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch(entries))])
            .unwrap();
        updater.commit().await
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn grouped_dedupe_unions_all_needs_and_blocks_downstream_until_success() {
        let roots = ["/root"];
        let requests: Arc<[NonrootIncludeRequest]> = Arc::from([
            request("//a:first.MODULE.bazel", 1),
            request("//a:second.MODULE.bazel", 2),
            request("//b:third.MODULE.bazel", 3),
        ]);
        let counter = Arc::new(AtomicUsize::new(0));
        let key = HorizonCounterKey {
            workspace: path("/workspace"),
            requests,
            counter: counter.dupe(),
        };
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut updater = dice.updater();
        inject_root_package_policy_inputs(&mut updater, policy(&roots, &[])).unwrap();
        let mut transaction = updater.commit().await;

        let mut incomplete = prelude(&roots);
        incomplete.extend([
            present("/root/a", PathNodeKind::Directory, 2),
            present("/root/b", PathNodeKind::Directory, 2),
        ]);
        transaction = update_epoch(transaction, &incomplete).await;
        let outcome = transaction.compute(&key).await.unwrap();
        let PathOutcome::Need(need) = &outcome else {
            panic!("incomplete package horizon must return Need");
        };
        assert_eq!(
            need.demands()
                .iter()
                .map(|demand| demand.path().as_path())
                .collect::<Vec<_>>(),
            [
                std::path::Path::new("/root/a/BUILD.bazel"),
                std::path::Path::new("/root/b/BUILD.bazel")
            ]
        );
        assert!(!HorizonCounterKey::validity(&outcome));
        assert!(!HorizonCounterKey::equality(&outcome, &outcome));
        assert_eq!(counter.load(Ordering::SeqCst), 0);
        assert!(need.demands().iter().all(|demand| {
            demand.operation() == PathObservationOperation::Lstat
                && !demand
                    .path()
                    .as_path()
                    .to_string_lossy()
                    .contains(".MODULE.bazel")
        }));

        let mut complete = incomplete;
        complete.extend([
            present("/root/a/BUILD.bazel", PathNodeKind::RegularFile, 3),
            present("/root/b/BUILD.bazel", PathNodeKind::RegularFile, 3),
        ]);
        transaction = update_epoch(transaction, &complete).await;
        assert!(matches!(
            transaction.compute(&key).await.unwrap(),
            PathOutcome::Complete(value) if matches!(value.as_ref(), Ok(1))
        ));
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn mixed_results_follow_original_source_order_after_group_completion() {
        let roots = ["/root"];
        let mut entries = prelude(&roots);
        entries.extend([
            present("/root/a", PathNodeKind::Directory, 2),
            present("/root/b", PathNodeKind::Directory, 2),
        ]);

        let terminal_first = [
            request("//deleted:first.MODULE.bazel", 1),
            request("//a:later.MODULE.bazel", 2),
        ];
        let outcome = preflight(
            Some(policy(&roots, &["//deleted"])),
            Some(&entries),
            &terminal_first,
        )
        .await;
        assert!(matches!(
            outcome,
            PathOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostRootIncludeError::Package {
                        raw_label,
                        failure: HostRootIncludePackageFailure::Deleted,
                        ..
                    }) if raw_label == "//deleted:first.MODULE.bazel"
                )
        ));

        let need_first = [
            request("//a:first.MODULE.bazel", 1),
            request("//deleted:middle.MODULE.bazel", 2),
            request("//b:last.MODULE.bazel", 3),
        ];
        let outcome = preflight(
            Some(policy(&roots, &["//deleted"])),
            Some(&entries),
            &need_first,
        )
        .await;
        let PathOutcome::Need(need) = outcome else {
            panic!("source-first unresolved package must return grouped Need");
        };
        assert_eq!(
            need.demands()
                .iter()
                .map(|demand| demand.path().as_path())
                .collect::<Vec<_>>(),
            [
                std::path::Path::new("/root/a/BUILD.bazel"),
                std::path::Path::new("/root/b/BUILD.bazel")
            ]
        );

        let first_terminal = [
            request("//external:first.MODULE.bazel", 10),
            request("//deleted:second.MODULE.bazel", 20),
        ];
        let outcome = preflight(
            Some(policy(&[], &["//deleted"])),
            Some(&prelude(&[])),
            &first_terminal,
        )
        .await;
        assert!(matches!(
            outcome,
            PathOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostRootIncludeError::Package {
                        raw_label,
                        location,
                        failure: HostRootIncludePackageFailure::NoBuildFile,
                    }) if raw_label == "//external:first.MODULE.bazel"
                        && location.start_line == 10
                )
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn alternate_root_paths_use_canonical_targets_and_preserve_order() {
        let roots = ["/root-a", "/root-b"];
        let mut entries = prelude(&roots);
        entries.extend([
            present("/root-a/pkg", PathNodeKind::Directory, 2),
            missing("/root-a/pkg/BUILD.bazel"),
            missing("/root-a/pkg/BUILD"),
            present("/root-b/pkg", PathNodeKind::Directory, 2),
            present("/root-b/pkg/BUILD.bazel", PathNodeKind::RegularFile, 2),
        ]);
        let requests = [
            request("//pkg:dir/first.MODULE.bazel/.", 1),
            request("//pkg:second.MODULE.bazel", 2),
        ];
        let outcome = preflight(Some(policy(&roots, &[])), Some(&entries), &requests).await;
        let PathOutcome::Complete(value) = outcome else {
            panic!("complete alternate-root script returned Need");
        };
        let horizon = value.as_ref().as_ref().unwrap();
        assert_eq!(
            horizon
                .includes()
                .iter()
                .map(|include| include.include().raw_label())
                .collect::<Vec<_>>(),
            [
                "//pkg:dir/first.MODULE.bazel/.",
                "//pkg:second.MODULE.bazel"
            ]
        );
        assert_eq!(
            horizon
                .includes()
                .iter()
                .map(|include| include.logical_path().as_path())
                .collect::<Vec<_>>(),
            [
                std::path::Path::new("/root-b/pkg/dir/first.MODULE.bazel"),
                std::path::Path::new("/root-b/pkg/second.MODULE.bazel")
            ]
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn colonless_label_derives_the_repeated_default_target_path() {
        let roots = ["/root"];
        let mut entries = prelude(&roots);
        entries.extend([
            present("/root/pkg", PathNodeKind::Directory, 2),
            present("/root/pkg/file.MODULE.bazel", PathNodeKind::Directory, 2),
            present(
                "/root/pkg/file.MODULE.bazel/BUILD.bazel",
                PathNodeKind::RegularFile,
                2,
            ),
        ]);
        let outcome = preflight(
            Some(policy(&roots, &[])),
            Some(&entries),
            &[request("//pkg/file.MODULE.bazel", 1)],
        )
        .await;
        let PathOutcome::Complete(value) = outcome else {
            panic!("complete colonless-label script returned Need");
        };
        let horizon = value.as_ref().as_ref().unwrap();
        assert_eq!(
            horizon.includes()[0].logical_path().as_path(),
            std::path::Path::new("/root/pkg/file.MODULE.bazel/file.MODULE.bazel")
        );
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn observed_preflight_retains_exact_arcs_and_decisive_source_prefix() {
        let roots = ["/root"];
        let mut entries = prelude(&roots);
        entries.extend([
            present("/root/a", PathNodeKind::Directory, 2),
            present("/root/a/BUILD.bazel", PathNodeKind::RegularFile, 2),
            present("/root/b", PathNodeKind::Directory, 3),
            present("/root/b/BUILD.bazel", PathNodeKind::RegularFile, 3),
        ]);
        let injected = epoch(&entries);
        let requests = [
            request("//a:first.MODULE.bazel", 1),
            request("//a:second.MODULE.bazel", 2),
            request("//b:third.MODULE.bazel", 3),
        ];
        let outcome = observed_preflight(policy(&roots, &[]), injected.dupe(), &requests).await;
        let PathOutcome::Complete(Ok((horizon, retained))) = outcome else {
            panic!("complete observed preflight did not retain a frontier");
        };
        assert_eq!(horizon.as_ref().as_ref().unwrap().includes().len(), 3);
        assert_eq!(retained.observations().len(), injected.observations().len());
        for (demand, result) in retained.observations() {
            assert!(Arc::ptr_eq(
                result,
                injected.get(demand).expect("retained demand was injected")
            ));
        }

        let terminal = observed_preflight(
            policy(&roots, &["//deleted"]),
            injected,
            &[
                request("//deleted:first.MODULE.bazel", 10),
                request("//b:later.MODULE.bazel", 20),
            ],
        )
        .await;
        let PathOutcome::Complete(Ok((terminal, retained))) = terminal else {
            panic!("source-first semantic terminal must complete");
        };
        assert!(matches!(
            terminal.as_ref(),
            Err(HostRootIncludeError::Package {
                raw_label,
                failure: HostRootIncludePackageFailure::Deleted,
                ..
            }) if raw_label == "//deleted:first.MODULE.bazel"
        ));
        assert!(retained.observations().is_empty());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn observed_preflight_groups_need_and_union_errors_are_outer() {
        let roots = ["/root"];
        let requests = [
            request("//a:first.MODULE.bazel", 1),
            request("//b:second.MODULE.bazel", 2),
        ];
        let mut incomplete = prelude(&roots);
        incomplete.extend([
            present("/root/a", PathNodeKind::Directory, 2),
            present("/root/b", PathNodeKind::Directory, 2),
        ]);
        let need = observed_preflight(policy(&roots, &[]), epoch(&incomplete), &requests).await;
        let PathOutcome::Need(need) = need else {
            panic!("incomplete observed packages must return grouped Need");
        };
        assert_eq!(need.demands().len(), 2);
        assert!(need.demands().iter().any(|demand| {
            demand.path().as_path() == std::path::Path::new("/root/a/BUILD.bazel")
        }));
        assert!(need.demands().iter().any(|demand| {
            demand.path().as_path() == std::path::Path::new("/root/b/BUILD.bazel")
        }));

        let demand = demand("/conflict");
        let left = PathObservationEpoch::new([(
            demand.dupe(),
            PathObservationResult::Lstat(PathOperationResult::Missing),
        )])
        .unwrap();
        let right = PathObservationEpoch::new([(
            demand,
            PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                PathNodeKind::RegularFile,
                1,
                1,
                1,
                1,
                0o644,
            ))),
        )])
        .unwrap();
        assert!(matches!(
            union_observations(&left, &right),
            Err(slug_workspace_v2::ObservedPathFrontierError::Epoch(
                PathObservationEpochError::ConflictingDemand(_)
            ))
        ));
    }
}
