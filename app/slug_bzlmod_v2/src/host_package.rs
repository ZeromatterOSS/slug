/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory or the Apache License, Version 2.0
 * found in the LICENSE-APACHE file in the root directory. You may select,
 * at your option, one of the above-listed licenses.
 */

#![allow(dead_code)] // Dormant until the later Host root-module packets.

use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::PackageIdentifier;
use slug_identity_v2::PackagePath;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathOutcome;
use slug_workspace_v2::PathResolutionError;
use slug_workspace_v2::ResolvedPathKey;
use slug_workspace_v2::ResolvedPathState;

use crate::RootPackageLookupInputsProjectionKey;
use crate::RootPackagePolicyProjectionError;
use crate::repository_ignore::HostRepositoryIgnoreError;
use crate::repository_ignore::HostRepositoryIgnoreKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
pub(crate) enum HostBuildFileName {
    BuildDotBazel,
    Build,
}

impl HostBuildFileName {
    fn as_str(self) -> &'static str {
        match self {
            Self::BuildDotBazel => "BUILD.bazel",
            Self::Build => "BUILD",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) struct HostPackage {
    /// The selected package-path entry. Consumers append package and target.
    package_root: NormalizedAbsolutePath,
    build_file_name: HostBuildFileName,
}

impl HostPackage {
    pub(crate) fn package_root(&self) -> &NormalizedAbsolutePath {
        &self.package_root
    }

    pub(crate) fn build_file_name(&self) -> HostBuildFileName {
        self.build_file_name
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) enum HostRootPackageLookup {
    Package(HostPackage),
    NoBuildFile,
    Deleted,
    InvalidPackageName { message: Arc<str> },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostRootPackageLookupError {
    PolicyInput(RootPackagePolicyProjectionError),
    RepositoryIgnore(HostRepositoryIgnoreError),
    Resolution {
        logical_path: NormalizedAbsolutePath,
        error: PathResolutionError,
    },
}

impl fmt::Display for HostRootPackageLookupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PolicyInput(error) => error.fmt(f),
            Self::RepositoryIgnore(error) => error.fmt(f),
            Self::Resolution {
                logical_path,
                error,
            } => write!(
                f,
                "failed to resolve package marker {}: {error:?}",
                logical_path.as_path().display()
            ),
        }
    }
}

impl std::error::Error for HostRootPackageLookupError {}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct HostRootPackageLookupKey {
    workspace: NormalizedAbsolutePath,
    package: PackagePath,
}

impl HostRootPackageLookupKey {
    pub(crate) fn new(workspace: NormalizedAbsolutePath, package: PackagePath) -> Self {
        Self { workspace, package }
    }
}

impl fmt::Display for HostRootPackageLookupKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-root-package-lookup:{}//{}",
            self.workspace, self.package
        )
    }
}

fn invalid_package_name(package: &PackagePath) -> Option<Arc<str>> {
    let value = package.as_str();
    if !value
        .bytes()
        .all(|byte| (b' '..=b'~').contains(&byte) && !matches!(byte, b':' | b'\\'))
    {
        let reason = r##"package names may contain A-Z, a-z, 0-9, or any of ' !"#$%&'()*+,-./;<=>?[]^_`{|}~' (any ASCII character except 0-31, 127, ':', or '\')"##;
        return Some(Arc::from(format!(
            "Invalid package name '{value}': {reason}"
        )));
    }
    if value
        .split('/')
        .any(|component| !component.is_empty() && component.bytes().all(|byte| byte == b'.'))
    {
        return Some(Arc::from(format!(
            "Invalid package name '{value}': package name component contains only '.' characters"
        )));
    }
    None
}

#[track_caller]
fn dice_invariant<T, E: fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| panic!("Host package-lookup DICE invariant failed: {error:?}"))
}

#[async_trait]
impl Key for HostRootPackageLookupKey {
    type Value = PathOutcome<Arc<Result<HostRootPackageLookup, HostRootPackageLookupError>>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let inputs = match dice_invariant(
            ctx.compute(&RootPackageLookupInputsProjectionKey::new(
                self.workspace.dupe(),
            ))
            .await,
        ) {
            Ok(inputs) => inputs,
            Err(error) => {
                return PathOutcome::Complete(Arc::new(Err(
                    HostRootPackageLookupError::PolicyInput(error),
                )));
            }
        };

        if let Some(message) = invalid_package_name(&self.package) {
            return PathOutcome::Complete(Arc::new(Ok(
                HostRootPackageLookup::InvalidPackageName { message },
            )));
        }

        let package_id = PackageIdentifier::new(CanonicalRepoName::root(), self.package.clone());
        if inputs.deleted_packages().contains(&package_id) {
            return PathOutcome::Complete(Arc::new(Ok(HostRootPackageLookup::Deleted)));
        }
        if self.package.as_str() == "external" {
            return PathOutcome::Complete(Arc::new(Ok(HostRootPackageLookup::NoBuildFile)));
        }

        let repository_ignore = match dice_invariant(
            ctx.compute(&HostRepositoryIgnoreKey::new(self.workspace.dupe()))
                .await,
        ) {
            PathOutcome::Need(need) => return PathOutcome::Need(need),
            PathOutcome::Complete(value) => match value.as_ref() {
                Ok(value) => value.dupe(),
                Err(error) => {
                    return PathOutcome::Complete(Arc::new(Err(
                        HostRootPackageLookupError::RepositoryIgnore(error.clone()),
                    )));
                }
            },
        };
        if repository_ignore.matching_entry(&self.package).is_some() {
            return PathOutcome::Complete(Arc::new(Ok(HostRootPackageLookup::Deleted)));
        }

        for root in inputs.package_roots() {
            for build_file_name in [HostBuildFileName::BuildDotBazel, HostBuildFileName::Build] {
                let logical_path = NormalizedAbsolutePath::new(
                    root.as_path()
                        .join(self.package.as_str())
                        .join(build_file_name.as_str()),
                )
                .expect("joining package and marker to a normalized root remains absolute");
                let resolved = match dice_invariant(
                    ctx.compute(&ResolvedPathKey::new(
                        PathObservationNamespace::Host,
                        logical_path.dupe(),
                    ))
                    .await,
                ) {
                    PathOutcome::Need(need) => return PathOutcome::Need(need),
                    PathOutcome::Complete(Ok(resolved)) => resolved,
                    PathOutcome::Complete(Err(error)) => {
                        return PathOutcome::Complete(Arc::new(Err(
                            HostRootPackageLookupError::Resolution {
                                logical_path,
                                error,
                            },
                        )));
                    }
                };
                match resolved.state() {
                    ResolvedPathState::Present(lstat)
                        if matches!(
                            lstat.kind(),
                            PathNodeKind::RegularFile | PathNodeKind::SpecialFile
                        ) =>
                    {
                        return PathOutcome::Complete(Arc::new(Ok(
                            HostRootPackageLookup::Package(HostPackage {
                                package_root: root.dupe(),
                                build_file_name,
                            }),
                        )));
                    }
                    ResolvedPathState::Present(lstat) if lstat.kind() == PathNodeKind::Symlink => {
                        unreachable!("ResolvedPathKey returns the terminal symlink kind")
                    }
                    ResolvedPathState::Present(_) | ResolvedPathState::Missing => {}
                }
            }
        }

        PathOutcome::Complete(Arc::new(Ok(HostRootPackageLookup::NoBuildFile)))
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
    #[cfg(unix)]
    use std::fmt;
    #[cfg(unix)]
    use std::hash::Hash;
    #[cfg(unix)]
    use std::hash::Hasher;
    #[cfg(unix)]
    use std::path::PathBuf;
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
    use slug_identity_v2::PackagePath;
    #[cfg(unix)]
    use slug_workspace_v2::NormalizedAbsolutePath;
    #[cfg(unix)]
    use slug_workspace_v2::PathIoErrorKind;
    #[cfg(unix)]
    use slug_workspace_v2::PathLstat;
    #[cfg(unix)]
    use slug_workspace_v2::PathNodeKind;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationDemand;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationEpoch;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationEpochKey;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationError;
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
    use super::HostBuildFileName;
    #[cfg(unix)]
    use super::HostRootPackageLookup;
    #[cfg(unix)]
    use super::HostRootPackageLookupKey;
    #[cfg(unix)]
    use crate::RootPackagePolicyInputs;
    #[cfg(unix)]
    use crate::inject_root_package_policy_inputs;

    #[cfg(unix)]
    type ScriptEntry = (PathObservationDemand, PathObservationResult);

    #[cfg(unix)]
    fn path(value: &str) -> NormalizedAbsolutePath {
        NormalizedAbsolutePath::new(value).unwrap()
    }

    #[cfg(unix)]
    fn lstat(kind: PathNodeKind, variant: i64) -> PathLstat {
        PathLstat::new(kind, variant, variant, variant, variant, 0o755)
    }

    #[cfg(unix)]
    fn demand(value: &str, operation: PathObservationOperation) -> PathObservationDemand {
        PathObservationDemand::new(PathObservationNamespace::Host, path(value), operation)
    }

    #[cfg(unix)]
    fn observed_lstat(value: &str, result: PathOperationResult<PathLstat>) -> ScriptEntry {
        (
            demand(value, PathObservationOperation::Lstat),
            PathObservationResult::Lstat(result),
        )
    }

    #[cfg(unix)]
    fn present(value: &str, kind: PathNodeKind, variant: i64) -> ScriptEntry {
        observed_lstat(value, PathOperationResult::Present(lstat(kind, variant)))
    }

    #[cfg(unix)]
    fn missing(value: &str) -> ScriptEntry {
        observed_lstat(value, PathOperationResult::Missing)
    }

    #[cfg(unix)]
    fn lstat_error(value: &str) -> ScriptEntry {
        observed_lstat(
            value,
            PathOperationResult::Error(PathObservationError::Io {
                kind: PathIoErrorKind::PermissionDenied,
                raw_os_error: Some(13),
            }),
        )
    }

    #[cfg(unix)]
    fn bytes(value: &str, contents: &'static [u8]) -> ScriptEntry {
        (
            demand(value, PathObservationOperation::FileBytes),
            PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(contents))),
        )
    }

    #[cfg(unix)]
    fn read_link(value: &str, target: &str) -> ScriptEntry {
        (
            demand(value, PathObservationOperation::ReadLink),
            PathObservationResult::ReadLink(PathOperationResult::Present(Arc::new(PathBuf::from(
                target,
            )))),
        )
    }

    #[cfg(unix)]
    fn missing_read_link(value: &str) -> ScriptEntry {
        (
            demand(value, PathObservationOperation::ReadLink),
            PathObservationResult::ReadLink(PathOperationResult::Missing),
        )
    }

    #[cfg(unix)]
    fn inputs(roots: &[&str], deleted: &[&str], vendor: Option<&str>) -> RootPackagePolicyInputs {
        RootPackagePolicyInputs::new(
            path("/workspace"),
            roots.iter().map(|root| path(root)).collect::<Vec<_>>(),
            deleted,
            vendor.map(path),
            Some("warning"),
        )
        .unwrap()
    }

    #[cfg(unix)]
    fn repository_prelude(roots: &[&str], variant: i64) -> Vec<ScriptEntry> {
        let mut entries = vec![
            present("/", PathNodeKind::Directory, variant),
            present("/workspace", PathNodeKind::Directory, variant),
            missing("/workspace/REPO.bazel"),
        ];
        for root in roots {
            entries.push(present(root, PathNodeKind::Directory, variant));
            entries.push(missing(&format!("{root}/.bazelignore")));
        }
        entries
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
    async fn lookup(
        policy: RootPackagePolicyInputs,
        entries: Vec<ScriptEntry>,
        package: &str,
    ) -> PathOutcome<Arc<Result<HostRootPackageLookup, super::HostRootPackageLookupError>>> {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut updater = dice.updater();
        inject_root_package_policy_inputs(&mut updater, policy).unwrap();
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                PathObservationEpoch::new(entries).unwrap(),
            )])
            .unwrap();
        let mut transaction = updater.commit().await;
        transaction
            .compute(&HostRootPackageLookupKey::new(
                path("/workspace"),
                PackagePath::parse(package).unwrap(),
            ))
            .await
            .unwrap()
    }

    #[cfg(unix)]
    async fn lookup_without_observations(
        policy: Option<RootPackagePolicyInputs>,
        package: &str,
    ) -> PathOutcome<Arc<Result<HostRootPackageLookup, super::HostRootPackageLookupError>>> {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut updater = dice.updater();
        if let Some(policy) = policy {
            inject_root_package_policy_inputs(&mut updater, policy).unwrap();
        }
        let mut transaction = updater.commit().await;
        transaction
            .compute(&HostRootPackageLookupKey::new(
                path("/workspace"),
                PackagePath::parse(package).unwrap(),
            ))
            .await
            .unwrap()
    }

    #[cfg(unix)]
    fn package(
        outcome: &PathOutcome<
            Arc<Result<HostRootPackageLookup, super::HostRootPackageLookupError>>,
        >,
    ) -> &super::HostPackage {
        let PathOutcome::Complete(value) = outcome else {
            panic!("complete script returned an observation Need");
        };
        let Ok(HostRootPackageLookup::Package(package)) = value.as_ref() else {
            panic!("expected package, got {value:?}");
        };
        package
    }

    #[cfg(unix)]
    #[derive(Debug, Clone, Allocative)]
    struct LookupCounterKey {
        lookup: HostRootPackageLookupKey,
        #[allocative(skip)]
        counter: Arc<AtomicUsize>,
    }

    #[cfg(unix)]
    impl PartialEq for LookupCounterKey {
        fn eq(&self, other: &Self) -> bool {
            self.lookup == other.lookup && Arc::ptr_eq(&self.counter, &other.counter)
        }
    }

    #[cfg(unix)]
    impl Eq for LookupCounterKey {}

    #[cfg(unix)]
    impl Hash for LookupCounterKey {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.lookup.hash(state);
            Arc::as_ptr(&self.counter).hash(state);
        }
    }

    #[cfg(unix)]
    impl fmt::Display for LookupCounterKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                f,
                "host-package-lookup-counter:{}:{:p}",
                self.lookup,
                Arc::as_ptr(&self.counter)
            )
        }
    }

    #[cfg(unix)]
    #[async_trait]
    impl Key for LookupCounterKey {
        type Value = PathOutcome<usize>;

        async fn compute(
            &self,
            ctx: &mut DiceComputations,
            _cancellations: &CancellationContext,
        ) -> Self::Value {
            ctx.compute(&self.lookup)
                .await
                .unwrap()
                .map(|_| self.counter.fetch_add(1, Ordering::SeqCst) + 1)
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
    async fn ordered_roots_are_outer_to_build_file_name_priority() {
        let roots = ["/root-a", "/root-b"];
        let mut entries = repository_prelude(&roots, 1);
        entries.extend([
            present("/root-a/pkg", PathNodeKind::Directory, 1),
            missing("/root-a/pkg/BUILD.bazel"),
            present("/root-a/pkg/BUILD", PathNodeKind::RegularFile, 1),
            present("/root-b/pkg", PathNodeKind::Directory, 1),
            present("/root-b/pkg/BUILD.bazel", PathNodeKind::RegularFile, 1),
        ]);
        let outcome = lookup(inputs(&roots, &[], None), entries, "pkg").await;
        let selected = package(&outcome);
        assert_eq!(selected.package_root(), &path("/root-a"));
        assert_eq!(selected.build_file_name(), HostBuildFileName::Build);

        let roots = ["/root-a"];
        let mut entries = repository_prelude(&roots, 2);
        entries.extend([
            present("/root-a/pkg", PathNodeKind::Directory, 2),
            present("/root-a/pkg/BUILD.bazel", PathNodeKind::RegularFile, 2),
            present("/root-a/pkg/BUILD", PathNodeKind::RegularFile, 2),
        ]);
        let outcome = lookup(inputs(&roots, &[], None), entries, "pkg").await;
        assert_eq!(
            package(&outcome).build_file_name(),
            HostBuildFileName::BuildDotBazel
        );

        let roots = ["/root-a", "/root-b"];
        let mut entries = repository_prelude(&roots, 3);
        entries.extend([
            present("/root-a/pkg", PathNodeKind::Directory, 3),
            present("/root-a/pkg/BUILD.bazel", PathNodeKind::Directory, 3),
            missing("/root-a/pkg/BUILD"),
            present("/root-b/pkg", PathNodeKind::Directory, 3),
            present("/root-b/pkg/BUILD.bazel", PathNodeKind::RegularFile, 3),
        ]);
        let outcome = lookup(inputs(&roots, &[], None), entries, "pkg").await;
        let selected = package(&outcome);
        assert_eq!(selected.package_root(), &path("/root-b"));
        assert_eq!(selected.build_file_name(), HostBuildFileName::BuildDotBazel);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn empty_roots_and_missing_markers_are_no_build_file() {
        let outcome = lookup(inputs(&[], &[], None), repository_prelude(&[], 1), "pkg").await;
        assert!(matches!(
            outcome,
            PathOutcome::Complete(value)
                if matches!(value.as_ref(), Ok(HostRootPackageLookup::NoBuildFile))
        ));

        let roots = ["/root-a"];
        let mut entries = repository_prelude(&roots, 2);
        entries.extend([
            present("/root-a/pkg", PathNodeKind::Directory, 2),
            missing("/root-a/pkg/BUILD.bazel"),
            missing("/root-a/pkg/BUILD"),
        ]);
        let outcome = lookup(inputs(&roots, &[], None), entries, "pkg").await;
        assert!(matches!(
            outcome,
            PathOutcome::Complete(value)
                if matches!(value.as_ref(), Ok(HostRootPackageLookup::NoBuildFile))
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn deletion_and_external_precede_every_observation() {
        let deleted =
            lookup_without_observations(Some(inputs(&["/root-a"], &["//pkg"], None)), "pkg").await;
        assert!(matches!(
            deleted,
            PathOutcome::Complete(value)
                if matches!(value.as_ref(), Ok(HostRootPackageLookup::Deleted))
        ));

        let external =
            lookup_without_observations(Some(inputs(&["/root-a"], &[], None)), "external").await;
        assert!(matches!(
            external,
            PathOutcome::Complete(value)
                if matches!(value.as_ref(), Ok(HostRootPackageLookup::NoBuildFile))
        ));

        let deleted_external = lookup_without_observations(
            Some(inputs(&["/root-a"], &["//external"], None)),
            "external",
        )
        .await;
        assert!(matches!(
            deleted_external,
            PathOutcome::Complete(value)
                if matches!(value.as_ref(), Ok(HostRootPackageLookup::Deleted))
        ));

        let nonmain_only = lookup(
            inputs(&[], &["@other//pkg"], None),
            repository_prelude(&[], 1),
            "pkg",
        )
        .await;
        assert!(matches!(
            nonmain_only,
            PathOutcome::Complete(value)
                if matches!(value.as_ref(), Ok(HostRootPackageLookup::NoBuildFile))
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn invalid_package_name_precedes_every_observation() {
        for (package, expected) in [
            (
                "bad:name",
                r##"Invalid package name 'bad:name': package names may contain A-Z, a-z, 0-9, or any of ' !"#$%&'()*+,-./;<=>?[]^_`{|}~' (any ASCII character except 0-31, 127, ':', or '\')"##,
            ),
            (
                "...",
                "Invalid package name '...': package name component contains only '.' characters",
            ),
            (
                ".../bad:name",
                r##"Invalid package name '.../bad:name': package names may contain A-Z, a-z, 0-9, or any of ' !"#$%&'()*+,-./;<=>?[]^_`{|}~' (any ASCII character except 0-31, 127, ':', or '\')"##,
            ),
        ] {
            let invalid =
                lookup_without_observations(Some(inputs(&["/root-a"], &[], None)), package).await;
            let PathOutcome::Complete(value) = invalid else {
                panic!("invalid package name must not request observations");
            };
            let Ok(HostRootPackageLookup::InvalidPackageName { message }) = value.as_ref() else {
                panic!("expected invalid package name, got {value:?}");
            };
            assert_eq!(message.as_ref(), expected);
            assert!(HostRootPackageLookupKey::equality(
                &PathOutcome::Complete(value.dupe()),
                &PathOutcome::Complete(value)
            ));
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn repo_bazelignore_and_contained_vendor_all_delete_packages() {
        let roots = ["/root-a"];

        let repo = vec![
            present("/", PathNodeKind::Directory, 1),
            present("/workspace", PathNodeKind::Directory, 1),
            present("/workspace/REPO.bazel", PathNodeKind::RegularFile, 1),
            bytes(
                "/workspace/REPO.bazel",
                b"ignore_directories(['repo/**'])\n",
            ),
            present("/root-a", PathNodeKind::Directory, 1),
            missing("/root-a/.bazelignore"),
        ];
        let outcome = lookup(inputs(&roots, &[], None), repo, "repo/child").await;
        assert!(matches!(
            outcome,
            PathOutcome::Complete(value)
                if matches!(value.as_ref(), Ok(HostRootPackageLookup::Deleted))
        ));

        let bazelignore = vec![
            present("/", PathNodeKind::Directory, 2),
            present("/workspace", PathNodeKind::Directory, 2),
            missing("/workspace/REPO.bazel"),
            present("/root-a", PathNodeKind::Directory, 2),
            present("/root-a/.bazelignore", PathNodeKind::RegularFile, 2),
            bytes("/root-a/.bazelignore", b"ignored\n"),
        ];
        let outcome = lookup(inputs(&roots, &[], None), bazelignore, "ignored/child").await;
        assert!(matches!(
            outcome,
            PathOutcome::Complete(value)
                if matches!(value.as_ref(), Ok(HostRootPackageLookup::Deleted))
        ));

        let vendor = repository_prelude(&roots, 3);
        let outcome = lookup(
            inputs(&roots, &[], Some("/root-a/vendor")),
            vendor,
            "vendor/child",
        )
        .await;
        assert!(matches!(
            outcome,
            PathOutcome::Complete(value)
                if matches!(value.as_ref(), Ok(HostRootPackageLookup::Deleted))
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn special_and_symlink_terminal_markers_are_files() {
        let roots = ["/root-a"];
        let mut special = repository_prelude(&roots, 1);
        special.extend([
            present("/root-a/pkg", PathNodeKind::Directory, 1),
            present("/root-a/pkg/BUILD.bazel", PathNodeKind::SpecialFile, 1),
        ]);
        let outcome = lookup(inputs(&roots, &[], None), special, "pkg").await;
        assert_eq!(
            package(&outcome).build_file_name(),
            HostBuildFileName::BuildDotBazel
        );

        let mut symlink = repository_prelude(&roots, 2);
        symlink.extend([
            present("/root-a/pkg", PathNodeKind::Directory, 2),
            present("/root-a/pkg/BUILD.bazel", PathNodeKind::Symlink, 2),
            read_link("/root-a/pkg/BUILD.bazel", "/outside/marker"),
            present("/outside", PathNodeKind::Directory, 2),
            present("/outside/marker", PathNodeKind::SpecialFile, 2),
        ]);
        let outcome = lookup(inputs(&roots, &[], None), symlink, "pkg").await;
        assert_eq!(
            package(&outcome).build_file_name(),
            HostBuildFileName::BuildDotBazel
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn marker_metadata_is_pruned_from_successful_equality() {
        async fn with_variant(
            variant: i64,
        ) -> PathOutcome<Arc<Result<HostRootPackageLookup, super::HostRootPackageLookupError>>>
        {
            let roots = ["/root-a"];
            let mut entries = repository_prelude(&roots, variant);
            entries.extend([
                present("/root-a/pkg", PathNodeKind::Directory, variant),
                present(
                    "/root-a/pkg/BUILD.bazel",
                    PathNodeKind::RegularFile,
                    variant,
                ),
            ]);
            lookup(inputs(&roots, &[], None), entries, "pkg").await
        }

        let first = with_variant(1).await;
        let changed_metadata = with_variant(99).await;
        assert!(HostRootPackageLookupKey::equality(
            &first,
            &changed_metadata
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn retained_lifecycle_prunes_metadata_and_replays_create_delete_restore() {
        fn script(marker: Option<i64>, variant: i64) -> Vec<ScriptEntry> {
            let roots = ["/root-a"];
            let mut entries = repository_prelude(&roots, variant);
            entries.push(present("/root-a/pkg", PathNodeKind::Directory, variant));
            match marker {
                Some(marker_variant) => entries.push(present(
                    "/root-a/pkg/BUILD.bazel",
                    PathNodeKind::RegularFile,
                    marker_variant,
                )),
                None => {
                    entries.push(missing("/root-a/pkg/BUILD.bazel"));
                    entries.push(missing("/root-a/pkg/BUILD"));
                }
            }
            entries
        }

        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut updater = dice.updater();
        inject_root_package_policy_inputs(&mut updater, inputs(&["/root-a"], &[], None)).unwrap();
        let mut transaction = updater.commit().await;
        let lookup =
            HostRootPackageLookupKey::new(path("/workspace"), PackagePath::parse("pkg").unwrap());
        let count = Arc::new(AtomicUsize::new(0));
        let counter = LookupCounterKey {
            lookup: lookup.clone(),
            counter: count.dupe(),
        };

        transaction = update_epoch(transaction, &script(None, 1)).await;
        let missing_value = transaction.compute(&lookup).await.unwrap();
        assert!(matches!(
            &missing_value,
            PathOutcome::Complete(value)
                if matches!(value.as_ref(), Ok(HostRootPackageLookup::NoBuildFile))
        ));
        assert!(matches!(
            transaction.compute(&counter).await.unwrap(),
            PathOutcome::Complete(1)
        ));

        transaction = update_epoch(transaction, &script(Some(2), 2)).await;
        let created = transaction.compute(&lookup).await.unwrap();
        assert_eq!(package(&created).package_root(), &path("/root-a"));
        assert!(matches!(
            transaction.compute(&counter).await.unwrap(),
            PathOutcome::Complete(2)
        ));

        transaction = update_epoch(transaction, &script(Some(99), 99)).await;
        let metadata_changed = transaction.compute(&lookup).await.unwrap();
        assert!(HostRootPackageLookupKey::equality(
            &created,
            &metadata_changed
        ));
        assert!(matches!(
            transaction.compute(&counter).await.unwrap(),
            PathOutcome::Complete(2)
        ));
        assert_eq!(count.load(Ordering::SeqCst), 2);

        transaction = update_epoch(transaction, &script(None, 3)).await;
        let deleted = transaction.compute(&lookup).await.unwrap();
        assert!(HostRootPackageLookupKey::equality(&missing_value, &deleted));
        assert!(matches!(
            transaction.compute(&counter).await.unwrap(),
            PathOutcome::Complete(3)
        ));

        transaction = update_epoch(transaction, &script(Some(4), 4)).await;
        let restored = transaction.compute(&lookup).await.unwrap();
        assert!(HostRootPackageLookupKey::equality(&created, &restored));
        assert!(matches!(
            transaction.compute(&counter).await.unwrap(),
            PathOutcome::Complete(4)
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn policy_ignore_and_resolution_failures_remain_typed() {
        let missing_input = lookup_without_observations(None, "pkg").await;
        assert!(matches!(
            missing_input,
            PathOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(super::HostRootPackageLookupError::PolicyInput(_))
                )
        ));

        let roots = ["/root-a"];
        let ignore_failure = vec![
            present("/", PathNodeKind::Directory, 1),
            present("/workspace", PathNodeKind::Directory, 1),
            lstat_error("/workspace/REPO.bazel"),
        ];
        let outcome = lookup(inputs(&roots, &[], None), ignore_failure, "pkg").await;
        assert!(matches!(
            outcome,
            PathOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(super::HostRootPackageLookupError::RepositoryIgnore(_))
                )
        ));

        let mut resolution_failure = repository_prelude(&roots, 2);
        resolution_failure.extend([
            present("/root-a/pkg", PathNodeKind::Directory, 2),
            lstat_error("/root-a/pkg/BUILD.bazel"),
        ]);
        let outcome = lookup(inputs(&roots, &[], None), resolution_failure, "pkg").await;
        assert!(matches!(
            outcome,
            PathOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(super::HostRootPackageLookupError::Resolution {
                        error: slug_workspace_v2::PathResolutionError::Observation { .. },
                        ..
                    })
                )
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cycle_and_infinite_expansion_failures_remain_discriminating() {
        let roots = ["/root-a"];
        let mut inconsistent = repository_prelude(&roots, 0);
        inconsistent.extend([
            present("/root-a/pkg", PathNodeKind::Directory, 0),
            present("/root-a/pkg/BUILD.bazel", PathNodeKind::Symlink, 0),
            missing_read_link("/root-a/pkg/BUILD.bazel"),
        ]);
        let outcome = lookup(inputs(&roots, &[], None), inconsistent, "pkg").await;
        assert!(matches!(
            outcome,
            PathOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(super::HostRootPackageLookupError::Resolution {
                        error: slug_workspace_v2::PathResolutionError::InconsistentState { .. },
                        ..
                    })
                )
        ));

        let mut cycle = repository_prelude(&roots, 1);
        cycle.extend([
            present("/root-a/pkg", PathNodeKind::Directory, 1),
            present("/root-a/pkg/BUILD.bazel", PathNodeKind::Symlink, 1),
            read_link("/root-a/pkg/BUILD.bazel", "BUILD.bazel"),
        ]);
        let outcome = lookup(inputs(&roots, &[], None), cycle, "pkg").await;
        assert!(matches!(
            outcome,
            PathOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(super::HostRootPackageLookupError::Resolution {
                        error: slug_workspace_v2::PathResolutionError::Cycle { .. },
                        ..
                    })
                )
        ));

        let mut expansion = repository_prelude(&roots, 2);
        expansion.extend([
            present("/root-a/pkg", PathNodeKind::Directory, 2),
            present("/root-a/pkg/BUILD.bazel", PathNodeKind::Symlink, 2),
            read_link("/root-a/pkg/BUILD.bazel", "/a"),
            present("/a", PathNodeKind::Symlink, 2),
            read_link("/a", "/a/child"),
        ]);
        let outcome = lookup(inputs(&roots, &[], None), expansion, "pkg").await;
        assert!(matches!(
            outcome,
            PathOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(super::HostRootPackageLookupError::Resolution {
                        error: slug_workspace_v2::PathResolutionError::InfiniteExpansion { .. },
                        ..
                    })
                )
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn incomplete_observation_is_transient_invalid_and_self_unequal() {
        let outcome =
            lookup_without_observations(Some(inputs(&["/root-a"], &[], None)), "pkg").await;
        assert!(matches!(outcome, PathOutcome::Need(_)));
        assert!(!HostRootPackageLookupKey::validity(&outcome));
        assert!(!HostRootPackageLookupKey::equality(&outcome, &outcome));
    }
}
