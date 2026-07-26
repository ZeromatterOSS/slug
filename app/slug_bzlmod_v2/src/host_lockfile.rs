/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory or the Apache License, Version 2.0
 * found in the LICENSE-APACHE file in the root directory.
 */

#![allow(dead_code)] // Dormant until the later Host registry packet.

use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use compact_str::CompactString;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathOutcome;

use crate::BazelLockfile;
use crate::host_file::HostFileBytes;
use crate::host_file::HostFileBytesKey;
use crate::host_file::HostFileError;
use crate::lockfile::parse_visible_lockfile_bytes_for_host;
use crate::module_eval::RootModuleLockfileModeKey;

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct HostVisibleLockfileValue {
    lockfile: Arc<BazelLockfile>,
}

impl HostVisibleLockfileValue {
    pub(crate) fn lockfile(&self) -> &Arc<BazelLockfile> {
        &self.lockfile
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostVisibleLockfileError {
    LockfileModeInput {
        workspace: NormalizedAbsolutePath,
        message: CompactString,
    },
    File {
        error: HostFileError,
    },
    BadLockfile {
        message: CompactString,
    },
    UncaughtParse {
        error: crate::LockfileParseError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub(crate) struct HostVisibleLockfileKey {
    workspace: NormalizedAbsolutePath,
}

impl HostVisibleLockfileKey {
    pub(crate) fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
}

impl fmt::Display for HostVisibleLockfileKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "host-visible-lockfile:{}", self.workspace)
    }
}

#[track_caller]
fn dice_invariant<T, E: fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| panic!("Host visible-lockfile DICE invariant failed: {error:?}"))
}

fn terminal_error(
    error: HostVisibleLockfileError,
) -> PathOutcome<Arc<Result<HostVisibleLockfileValue, HostVisibleLockfileError>>> {
    PathOutcome::Complete(Arc::new(Err(error)))
}

#[async_trait]
impl Key for HostVisibleLockfileKey {
    type Value = PathOutcome<Arc<Result<HostVisibleLockfileValue, HostVisibleLockfileError>>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let logical_path =
            NormalizedAbsolutePath::new(self.workspace.as_path().join("MODULE.bazel.lock"))
                .expect("joining the visible lockfile basename remains normalized absolute");
        let file = dice_invariant(ctx.compute(&HostFileBytesKey::new(logical_path)).await);
        let file = match file {
            PathOutcome::Need(need) => return PathOutcome::Need(need),
            PathOutcome::Complete(file) => file,
        };

        let mode = match ctx
            .compute(&RootModuleLockfileModeKey {
                workspace: self.workspace.as_path().to_path_buf(),
            })
            .await
        {
            Ok(mode) => mode.semantic_mode(),
            Err(error) => {
                return terminal_error(HostVisibleLockfileError::LockfileModeInput {
                    workspace: self.workspace.dupe(),
                    message: format!("missing injected root module lockfile mode: {error}").into(),
                });
            }
        };

        let bytes = match file {
            Err(error) => return terminal_error(HostVisibleLockfileError::File { error }),
            Ok(HostFileBytes::Missing) => None,
            Ok(HostFileBytes::Present(bytes)) => Some(bytes),
        };
        match parse_visible_lockfile_bytes_for_host(&mode, bytes.as_deref()) {
            Ok(lockfile) => {
                PathOutcome::Complete(Arc::new(Ok(HostVisibleLockfileValue { lockfile })))
            }
            Err(error) => terminal_error(error),
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
    use std::fmt;
    use std::hash::Hash;
    use std::hash::Hasher;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use allocative::Allocative;
    use async_trait::async_trait;
    use dice::ActivationData;
    use dice::ActivationTracker;
    use dice::DetectCycles;
    use dice::Dice;
    use dice::DiceComputations;
    use dice::DiceProjectionComputations;
    use dice::DiceTransaction;
    use dice::DynKey;
    use dice::Key;
    use dice::ProjectionKey;
    use dice::UserComputationData;
    use dice_futures::cancellation::CancellationContext;
    use dupe::Dupe;
    use slug_workspace_v2::NormalizedAbsolutePath;
    use slug_workspace_v2::PathIoErrorKind;
    use slug_workspace_v2::PathLstat;
    use slug_workspace_v2::PathNodeKind;
    use slug_workspace_v2::PathObservationDemand;
    use slug_workspace_v2::PathObservationEpoch;
    use slug_workspace_v2::PathObservationEpochKey;
    use slug_workspace_v2::PathObservationError;
    use slug_workspace_v2::PathObservationNamespace;
    use slug_workspace_v2::PathObservationOperation;
    use slug_workspace_v2::PathObservationResult;
    use slug_workspace_v2::PathOperationResult;
    use slug_workspace_v2::PathOutcome;

    use super::*;
    use crate::LockfileMode;
    use crate::LockfileParseErrorSurface;
    use crate::RootModuleLockfileMode;

    type ScriptEntry = (PathObservationDemand, PathObservationResult);

    #[derive(Debug)]
    struct TrackedActivation {
        key: String,
        evaluated: bool,
        dependencies: Vec<String>,
    }

    #[derive(Default)]
    struct HostLockfileTracker {
        activations: Mutex<Vec<TrackedActivation>>,
    }

    impl HostLockfileTracker {
        fn take(&self) -> Vec<TrackedActivation> {
            std::mem::take(&mut *self.activations.lock().unwrap())
        }
    }

    impl ActivationTracker for HostLockfileTracker {
        fn key_activated(
            &self,
            key: &DynKey,
            dependencies: &mut dyn Iterator<Item = &DynKey>,
            activation: ActivationData,
        ) {
            if key.downcast_ref::<HostVisibleLockfileKey>().is_none()
                && key.downcast_ref::<HostFileBytesKey>().is_none()
            {
                return;
            }
            self.activations.lock().unwrap().push(TrackedActivation {
                key: key.to_string(),
                evaluated: matches!(activation, ActivationData::Evaluated(_)),
                dependencies: dependencies.map(ToString::to_string).collect(),
            });
        }
    }

    fn path(value: &str) -> NormalizedAbsolutePath {
        NormalizedAbsolutePath::new(value).unwrap()
    }

    fn workspace() -> NormalizedAbsolutePath {
        path("/workspace")
    }

    fn lockfile_path() -> NormalizedAbsolutePath {
        path("/workspace/MODULE.bazel.lock")
    }

    fn demand(value: &str, operation: PathObservationOperation) -> PathObservationDemand {
        PathObservationDemand::new(PathObservationNamespace::Host, path(value), operation)
    }

    fn lstat(value: &str, result: PathOperationResult<PathLstat>) -> ScriptEntry {
        (
            demand(value, PathObservationOperation::Lstat),
            PathObservationResult::Lstat(result),
        )
    }

    fn present(value: &str, kind: PathNodeKind, variant: i64) -> ScriptEntry {
        lstat(
            value,
            PathOperationResult::Present(PathLstat::new(
                kind,
                variant,
                variant + 1,
                variant + 2,
                variant + 3,
                0o755,
            )),
        )
    }

    fn missing(value: &str) -> ScriptEntry {
        lstat(value, PathOperationResult::Missing)
    }

    fn read_link(value: &str, target: &str) -> ScriptEntry {
        (
            demand(value, PathObservationOperation::ReadLink),
            PathObservationResult::ReadLink(PathOperationResult::Present(Arc::new(PathBuf::from(
                target,
            )))),
        )
    }

    fn file_bytes(value: &str, result: PathOperationResult<Arc<[u8]>>) -> ScriptEntry {
        (
            demand(value, PathObservationOperation::FileBytes),
            PathObservationResult::FileBytes(result),
        )
    }

    fn direct_script(
        kind: PathNodeKind,
        variant: i64,
        bytes: Option<PathOperationResult<Arc<[u8]>>>,
    ) -> Vec<ScriptEntry> {
        let mut script = vec![
            present("/", PathNodeKind::Directory, 0),
            present("/workspace", PathNodeKind::Directory, 1),
            present("/workspace/MODULE.bazel.lock", kind, variant),
        ];
        if let Some(bytes) = bytes {
            script.push(file_bytes("/workspace/MODULE.bazel.lock", bytes));
        }
        script
    }

    fn present_script(
        kind: PathNodeKind,
        variant: i64,
        bytes: impl AsRef<[u8]>,
    ) -> Vec<ScriptEntry> {
        direct_script(
            kind,
            variant,
            Some(PathOperationResult::Present(Arc::from(bytes.as_ref()))),
        )
    }

    fn missing_script(variant: i64) -> Vec<ScriptEntry> {
        vec![
            present("/", PathNodeKind::Directory, 0),
            present("/workspace", PathNodeKind::Directory, variant),
            missing("/workspace/MODULE.bazel.lock"),
        ]
    }

    fn linked_script(bytes: impl AsRef<[u8]>, variant: i64) -> Vec<ScriptEntry> {
        vec![
            present("/", PathNodeKind::Directory, 0),
            present("/workspace", PathNodeKind::Directory, 1),
            present(
                "/workspace/MODULE.bazel.lock",
                PathNodeKind::Symlink,
                variant,
            ),
            read_link("/workspace/MODULE.bazel.lock", "/physical.lock"),
            present("/physical.lock", PathNodeKind::SpecialFile, variant + 1),
            file_bytes(
                "/physical.lock",
                PathOperationResult::Present(Arc::from(bytes.as_ref())),
            ),
        ]
    }

    fn epoch(script: &[ScriptEntry]) -> PathObservationEpoch {
        PathObservationEpoch::new(
            script
                .iter()
                .map(|(demand, result)| (demand.dupe(), result.dupe())),
        )
        .unwrap()
    }

    async fn update(
        transaction: DiceTransaction,
        script: &[ScriptEntry],
        mode: Option<LockfileMode>,
    ) -> DiceTransaction {
        let mut updater = transaction.into_updater();
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch(script))])
            .unwrap();
        if let Some(mode) = mode {
            updater
                .changed_to(vec![(
                    RootModuleLockfileModeKey {
                        workspace: workspace().as_path().to_path_buf(),
                    },
                    RootModuleLockfileMode::from(mode),
                )])
                .unwrap();
        }
        updater.commit().await
    }

    async fn complete(
        transaction: &mut DiceTransaction,
        key: &HostVisibleLockfileKey,
    ) -> Arc<Result<HostVisibleLockfileValue, HostVisibleLockfileError>> {
        let PathOutcome::Complete(value) = transaction.compute(key).await.unwrap() else {
            panic!("complete Host visible-lockfile script still needs observations");
        };
        value
    }

    async fn compute_once(
        script: &[ScriptEntry],
        mode: Option<LockfileMode>,
    ) -> Arc<Result<HostVisibleLockfileValue, HostVisibleLockfileError>> {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let transaction = dice.updater().commit().await;
        let mut transaction = update(transaction, script, mode).await;
        complete(&mut transaction, &HostVisibleLockfileKey::new(workspace())).await
    }

    fn value(
        result: &Arc<Result<HostVisibleLockfileValue, HostVisibleLockfileError>>,
    ) -> &HostVisibleLockfileValue {
        result.as_ref().as_ref().unwrap()
    }

    fn empty(result: &Arc<Result<HostVisibleLockfileValue, HostVisibleLockfileError>>) -> bool {
        value(result)
            .lockfile()
            .semantically_eq(&crate::empty_bazel_lockfile())
    }

    #[test]
    fn key_identity_and_complete_only_equality_are_workspace_scoped() {
        assert_eq!(
            HostVisibleLockfileKey::new(workspace()),
            HostVisibleLockfileKey::new(workspace())
        );
        assert_ne!(
            HostVisibleLockfileKey::new(workspace()),
            HostVisibleLockfileKey::new(path("/other"))
        );

        let complete = PathOutcome::Complete(Arc::new(Ok(HostVisibleLockfileValue {
            lockfile: Arc::new(crate::empty_bazel_lockfile()),
        })));
        assert!(HostVisibleLockfileKey::validity(&complete));
        assert!(HostVisibleLockfileKey::equality(&complete, &complete));

        let demand = PathObservationDemand::new(
            PathObservationNamespace::Host,
            path("/need"),
            PathObservationOperation::Lstat,
        );
        let need = PathOutcome::Need(slug_workspace_v2::NeedPathObservations::singleton(demand));
        assert!(!HostVisibleLockfileKey::validity(&need));
        assert!(!HostVisibleLockfileKey::equality(&need, &need));
    }

    #[tokio::test]
    async fn file_observations_precede_mode_and_mode_precedes_complete_interpretation() {
        let source = br#"{"lockFileVersion":28}"#;
        let script = present_script(PathNodeKind::RegularFile, 10, source);
        let key = HostVisibleLockfileKey::new(workspace());

        let tracker = Arc::new(HostLockfileTracker::default());
        let user_data = UserComputationData {
            activation_tracker: Some(tracker.dupe() as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        let dependency_dice = Dice::builder().build(DetectCycles::Enabled);
        let mut updater = dependency_dice.updater_with_data(user_data);
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch(&[]))])
            .unwrap();
        let mut dependency_transaction = updater.commit().await;
        let PathOutcome::Need(_) = dependency_transaction.compute(&key).await.unwrap() else {
            panic!("an empty observation epoch must request the first Host file observation");
        };
        let activations = tracker.take();
        let host_activation = activations
            .iter()
            .find(|activation| activation.key.starts_with("host-visible-lockfile:"))
            .expect("the tracker must observe Host visible-lockfile evaluation");
        assert!(
            host_activation
                .dependencies
                .iter()
                .any(|dependency| dependency.starts_with("bzlmod-host-file-bytes:")),
            "Host file bytes must be the direct dependency while observations are needed"
        );
        assert!(
            host_activation
                .dependencies
                .iter()
                .all(|dependency| !dependency.starts_with("root-module-lockfile-mode:")),
            "lockfile mode must not be a dependency while Host file observations are needed"
        );

        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut transaction = dice.updater().commit().await;

        for prefix_len in 0..script.len() {
            transaction = update(transaction, &script[..prefix_len], None).await;
            let outcome = transaction.compute(&key).await.unwrap();
            let PathOutcome::Need(need) = &outcome else {
                panic!("mode was requested before file prefix {prefix_len} completed");
            };
            assert_eq!(need.demands(), &[script[prefix_len].0.dupe()]);
            assert!(!HostVisibleLockfileKey::validity(&outcome));
            assert!(!HostVisibleLockfileKey::equality(&outcome, &outcome));
        }
        transaction = update(transaction, &script, None).await;
        assert!(matches!(
            complete(&mut transaction, &key).await.as_ref(),
            Err(HostVisibleLockfileError::LockfileModeInput { workspace, .. })
                if workspace == &path("/workspace")
        ));

        assert!(matches!(
            compute_once(&missing_script(20), None).await.as_ref(),
            Err(HostVisibleLockfileError::LockfileModeInput { .. })
        ));
        let io = PathObservationError::Io {
            kind: PathIoErrorKind::PermissionDenied,
            raw_os_error: Some(13),
        };
        let error_script = direct_script(
            PathNodeKind::RegularFile,
            30,
            Some(PathOperationResult::Error(io)),
        );
        assert!(matches!(
            compute_once(&error_script, None).await.as_ref(),
            Err(HostVisibleLockfileError::LockfileModeInput { .. })
        ));
        assert!(matches!(
            compute_once(&error_script, Some(LockfileMode::Update))
                .await
                .as_ref(),
            Err(HostVisibleLockfileError::File {
                error: HostFileError::Observation {
                    logical_path,
                    operation: PathObservationOperation::FileBytes,
                    error,
                },
            }) if logical_path == &lockfile_path() && error == &io
        ));
    }

    #[tokio::test]
    async fn missing_regular_special_symlink_and_operational_errors_are_distinct() {
        assert!(empty(
            &compute_once(&missing_script(1), Some(LockfileMode::Error)).await
        ));
        for script in [
            present_script(PathNodeKind::RegularFile, 10, br#"{"lockFileVersion":28}"#),
            present_script(PathNodeKind::SpecialFile, 20, br#"{"lockFileVersion":28}"#),
            linked_script(br#"{"lockFileVersion":28}"#, 30),
        ] {
            assert_eq!(
                value(&compute_once(&script, Some(LockfileMode::Off)).await)
                    .lockfile()
                    .lock_file_version(),
                28
            );
        }

        assert!(matches!(
            compute_once(
                &direct_script(PathNodeKind::Directory, 40, None),
                Some(LockfileMode::Update),
            )
            .await
            .as_ref(),
            Err(HostVisibleLockfileError::File {
                error: HostFileError::WrongKind {
                    logical_path,
                    actual: PathNodeKind::Directory,
                },
            }) if logical_path == &lockfile_path()
        ));
        let io = PathObservationError::Io {
            kind: PathIoErrorKind::PermissionDenied,
            raw_os_error: Some(13),
        };
        assert!(matches!(
            compute_once(
                &direct_script(
                    PathNodeKind::SpecialFile,
                    50,
                    Some(PathOperationResult::Error(io)),
                ),
                Some(LockfileMode::Refresh),
            )
            .await
            .as_ref(),
            Err(HostVisibleLockfileError::File {
                error: HostFileError::Observation { error, .. },
            }) if error == &io
        ));
    }

    #[tokio::test]
    async fn retained_engine_covers_create_edit_delete_recreate_and_mode_a_b_a() {
        let source_a = br#"{"lockFileVersion":28,"factsVersions":{"//:ext.bzl%x":1}}"#;
        let source_b = br#"{"lockFileVersion":28,"factsVersions":{"//:ext.bzl%x":2}}"#;
        let key = HostVisibleLockfileKey::new(workspace());
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut transaction = dice.updater().commit().await;

        transaction = update(transaction, &missing_script(1), Some(LockfileMode::Update)).await;
        assert!(empty(&complete(&mut transaction, &key).await));

        let state_a = present_script(PathNodeKind::RegularFile, 10, source_a);
        transaction = update(transaction, &state_a, Some(LockfileMode::Off)).await;
        let created_a = complete(&mut transaction, &key).await;

        let state_b = present_script(PathNodeKind::RegularFile, 20, source_b);
        transaction = update(transaction, &state_b, Some(LockfileMode::Error)).await;
        let edited_b = complete(&mut transaction, &key).await;
        assert_ne!(created_a, edited_b);

        transaction = update(
            transaction,
            &missing_script(30),
            Some(LockfileMode::Refresh),
        )
        .await;
        assert!(empty(&complete(&mut transaction, &key).await));

        let recreated_a = present_script(PathNodeKind::SpecialFile, 40, source_a);
        transaction = update(transaction, &recreated_a, Some(LockfileMode::Off)).await;
        let restored_a = complete(&mut transaction, &key).await;
        assert_eq!(created_a, restored_a);

        transaction = update(transaction, &recreated_a, Some(LockfileMode::Error)).await;
        let mode_b = complete(&mut transaction, &key).await;
        transaction = update(transaction, &recreated_a, Some(LockfileMode::Off)).await;
        let mode_a = complete(&mut transaction, &key).await;
        assert_eq!(restored_a, mode_b);
        assert_eq!(restored_a, mode_a);
    }

    #[test]
    fn host_parser_covers_modes_markers_diagnostics_utf8_and_direct_surfaces() {
        for mode in [
            LockfileMode::Off,
            LockfileMode::Update,
            LockfileMode::Refresh,
            LockfileMode::Error,
        ] {
            assert!(
                parse_visible_lockfile_bytes_for_host(&mode, None)
                    .unwrap()
                    .semantically_eq(&crate::empty_bazel_lockfile())
            );
            assert_eq!(
                parse_visible_lockfile_bytes_for_host(&mode, Some(br#"{"lockFileVersion":28}"#),)
                    .unwrap()
                    .lock_file_version(),
                28
            );
        }

        let noncurrent =
            br#"{"decoy":{"lockFileVersion":27},"lockFileVersion":28,"facts":{"broken":"#;
        for mode in [
            LockfileMode::Off,
            LockfileMode::Update,
            LockfileMode::Refresh,
        ] {
            assert!(
                parse_visible_lockfile_bytes_for_host(&mode, Some(noncurrent))
                    .unwrap()
                    .semantically_eq(&crate::empty_bazel_lockfile())
            );
        }
        assert!(matches!(
            parse_visible_lockfile_bytes_for_host(&LockfileMode::Error, Some(noncurrent)),
            Err(HostVisibleLockfileError::BadLockfile { message })
                if message == "The version of MODULE.bazel.lock is not supported by this version of Bazel. Please run `bazel mod deps --lockfile_mode=update` to update your lockfile."
        ));

        let leading_zero =
            br#"{"decoy":{"lockFileVersion":028},"lockFileVersion":28,"factsVersions":{}}"#;
        assert_eq!(
            parse_visible_lockfile_bytes_for_host(&LockfileMode::Off, Some(leading_zero))
                .unwrap()
                .lock_file_version(),
            28
        );
        let overflow = br#"{"lockFileVersion":999999999999999999999}"#;
        assert!(matches!(
            parse_visible_lockfile_bytes_for_host(&LockfileMode::Off, Some(overflow)),
            Err(HostVisibleLockfileError::BadLockfile { message })
                if message.contains("overflows signed 32-bit integer")
                    && message.contains("Try deleting it and rerun the build.")
        ));

        let malformed = br#"{"lockFileVersion":28,"#;
        for mode in [
            LockfileMode::Off,
            LockfileMode::Update,
            LockfileMode::Refresh,
            LockfileMode::Error,
        ] {
            let HostVisibleLockfileError::BadLockfile { message } =
                parse_visible_lockfile_bytes_for_host(&mode, Some(malformed)).unwrap_err()
            else {
                panic!("malformed JSON must use the caught BAD_LOCKFILE surface");
            };
            assert_eq!(
                message.as_str(),
                "Failed to read and parse the MODULE.bazel.lock file with error: unterminated object at line 1 column 23. Try deleting it and rerun the build."
            );
        }
        let merge = br#"{"decoy":{"lockFileVersion":28},"lockFileVersion":"<<<<<<<"}"#;
        let HostVisibleLockfileError::BadLockfile { message } =
            parse_visible_lockfile_bytes_for_host(&LockfileMode::Off, Some(merge)).unwrap_err()
        else {
            panic!("merge-conflict JSON must use the caught BAD_LOCKFILE surface");
        };
        assert_eq!(
            message.as_str(),
            "Failed to read and parse the MODULE.bazel.lock file with error: java.lang.NumberFormatException: For input string: \"<<<<<<<\". This looks like a merge conflict. See https://bazel.build/external/lockfile#merge-conflicts for advice."
        );

        let ignored_marker =
            br#"{"decoy":{"lockFileVersion":28},"ignored":"<<<<<<<","lockFileVersion":"ordinary"}"#;
        let HostVisibleLockfileError::BadLockfile { message } =
            parse_visible_lockfile_bytes_for_host(&LockfileMode::Off, Some(ignored_marker))
                .unwrap_err()
        else {
            panic!("ordinary invalid integer must use the caught BAD_LOCKFILE surface");
        };
        assert_eq!(
            message.as_str(),
            "Failed to read and parse the MODULE.bazel.lock file with error: java.lang.NumberFormatException: For input string: \"ordinary\". Try deleting it and rerun the build."
        );
        assert!(!message.contains("This looks like a merge conflict."));

        let missing_metadata_enum = br#"{
          "lockFileVersion":28,
          "moduleExtensions":{
            "//:ext.bzl%x":{
              "general":{
                "bzlTransitiveDigest":"AQ==",
                "usagesDigest":"AgM=",
                "recordedInputs":[],
                "generatedRepoSpecs":{},
                "moduleExtensionMetadata":{}
              }
            }
          }
        }"#;
        let HostVisibleLockfileError::BadLockfile { message } =
            parse_visible_lockfile_bytes_for_host(&LockfileMode::Off, Some(missing_metadata_enum))
                .unwrap_err()
        else {
            panic!("missing metadata enum must use the caught BAD_LOCKFILE surface");
        };
        assert_eq!(
            message.as_str(),
            "Failed to read and parse the MODULE.bazel.lock file with error: missing useAllRepos. Try deleting it and rerun the build."
        );

        let malformed_utf8 = b"{\"lockFileVersion\":28,\"ignored\":\"\xed\xa0\x80\"}";
        assert_eq!(
            parse_visible_lockfile_bytes_for_host(&LockfileMode::Off, Some(malformed_utf8),)
                .unwrap()
                .lock_file_version(),
            28
        );

        let direct = br#"{"lockFileVersion":28,"registryFileHashes":{"u":"not-a-checksum"}}"#;
        assert!(matches!(
            parse_visible_lockfile_bytes_for_host(&LockfileMode::Off, Some(direct)),
            Err(HostVisibleLockfileError::UncaughtParse { error })
                if error.surface() == LockfileParseErrorSurface::DirectAdapterJsonParse
        ));
        let delimiter = br#"{"lockFileVersion":28,"moduleExtensions":{"//:ext.bzl":{}}}"#;
        assert!(matches!(
            parse_visible_lockfile_bytes_for_host(&LockfileMode::Off, Some(delimiter)),
            Err(HostVisibleLockfileError::UncaughtParse { error })
                if error.surface() == LockfileParseErrorSurface::DelimiterIndexOutOfBounds
        ));
    }

    #[test]
    fn full_value_fields_and_separately_allocated_semantic_equality_are_retained() {
        let source = format!(
            r#"{{
              "lockFileVersion": 28,
              "registryFileHashes": {{"u": "{}"}},
              "selectedYankedVersions": {{"subject@1.0.0": "reason"}},
              "moduleExtensions": {{
                "//:ext.bzl%x": {{
                  "general": {{
                    "bzlTransitiveDigest": "AQ==",
                    "usagesDigest": "AgM=",
                    "recordedInputs": [],
                    "generatedRepoSpecs": {{}}
                  }}
                }}
              }},
              "facts": {{"//:ext.bzl%x": {{"answer": 42}}}},
              "factsVersions": {{"//:ext.bzl%x": 7}}
            }}"#,
            "ab".repeat(32)
        );
        let first =
            parse_visible_lockfile_bytes_for_host(&LockfileMode::Update, Some(source.as_bytes()))
                .unwrap();
        let second =
            parse_visible_lockfile_bytes_for_host(&LockfileMode::Refresh, Some(source.as_bytes()))
                .unwrap();
        assert!(!Arc::ptr_eq(&first, &second));
        assert!(first.semantically_eq(&second));
        let first_outcome = PathOutcome::Complete(Arc::new(Ok(HostVisibleLockfileValue {
            lockfile: first.dupe(),
        })));
        let second_outcome = PathOutcome::Complete(Arc::new(Ok(HostVisibleLockfileValue {
            lockfile: second.dupe(),
        })));
        assert!(HostVisibleLockfileKey::equality(
            &first_outcome,
            &second_outcome
        ));
        assert_eq!(first.lock_file_version, 28);
        assert_eq!(first.registry_file_hashes.len(), 1);
        assert_eq!(first.selected_yanked_versions.len(), 1);
        assert_eq!(first.module_extensions.len(), 1);
        assert_eq!(first.facts.len(), 1);
        assert_eq!(first.facts_versions.len(), 1);
    }

    #[derive(Debug, Clone, Allocative, Dupe)]
    struct SemanticCounterKey {
        lockfile: HostVisibleLockfileKey,
        #[allocative(skip)]
        counter: Arc<AtomicUsize>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative, Dupe)]
    struct HostVisibleLockfileProjectionKey;

    impl fmt::Display for HostVisibleLockfileProjectionKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("host-visible-lockfile-value-projection")
        }
    }

    impl ProjectionKey for HostVisibleLockfileProjectionKey {
        type DeriveFromKey = HostVisibleLockfileKey;
        type Value = PathOutcome<Arc<Result<HostVisibleLockfileValue, HostVisibleLockfileError>>>;

        fn compute(
            &self,
            derive_from: &<Self::DeriveFromKey as Key>::Value,
            _ctx: &DiceProjectionComputations,
        ) -> Self::Value {
            derive_from.dupe()
        }

        fn equality(x: &Self::Value, y: &Self::Value) -> bool {
            HostVisibleLockfileKey::equality(x, y)
        }

        fn validity(value: &Self::Value) -> bool {
            HostVisibleLockfileKey::validity(value)
        }
    }

    impl PartialEq for SemanticCounterKey {
        fn eq(&self, other: &Self) -> bool {
            self.lockfile == other.lockfile && Arc::ptr_eq(&self.counter, &other.counter)
        }
    }

    impl Eq for SemanticCounterKey {}

    impl Hash for SemanticCounterKey {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.lockfile.hash(state);
            Arc::as_ptr(&self.counter).hash(state);
        }
    }

    impl fmt::Display for SemanticCounterKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                f,
                "host-visible-lockfile-semantic-counter:{}:{:p}",
                self.lockfile,
                Arc::as_ptr(&self.counter)
            )
        }
    }

    #[async_trait]
    impl Key for SemanticCounterKey {
        type Value = PathOutcome<usize>;

        async fn compute(
            &self,
            ctx: &mut DiceComputations,
            _cancellations: &CancellationContext,
        ) -> Self::Value {
            let opaque = dice_invariant(ctx.compute_opaque(&self.lockfile).await);
            dice_invariant(ctx.projection(&opaque, &HostVisibleLockfileProjectionKey))
                .map(|_| self.counter.fetch_add(1, Ordering::SeqCst) + 1)
        }

        fn equality(x: &Self::Value, y: &Self::Value) -> bool {
            x.complete_eq(y)
        }

        fn validity(value: &Self::Value) -> bool {
            value.is_complete()
        }
    }

    #[tokio::test]
    async fn formatting_recomputes_bytes_but_prunes_downstream_semantic_projection() {
        let semantic_a = br#"{"lockFileVersion":28,"facts":{},"factsVersions":{"//:ext.bzl%x":1}}"#;
        let formatting_only = br#"
            {
              "factsVersions": {"//:ext.bzl%x": 1},
              "lockFileVersion": 28,
              "facts": {}
            }
        "#;
        let semantic_b = br#"{"lockFileVersion":28,"facts":{},"factsVersions":{"//:ext.bzl%x":2}}"#;
        let tracker = Arc::new(HostLockfileTracker::default());
        let user_data = UserComputationData {
            activation_tracker: Some(tracker.dupe() as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut transaction = dice.updater_with_data(user_data).commit().await;
        let counter = Arc::new(AtomicUsize::new(0));
        let key = SemanticCounterKey {
            lockfile: HostVisibleLockfileKey::new(workspace()),
            counter: counter.dupe(),
        };

        transaction = update(
            transaction,
            &present_script(PathNodeKind::RegularFile, 10, semantic_a),
            Some(LockfileMode::Update),
        )
        .await;
        assert!(matches!(
            transaction.compute(&key).await.unwrap(),
            PathOutcome::Complete(1)
        ));
        tracker.take();

        transaction = update(
            transaction,
            &present_script(PathNodeKind::SpecialFile, 20, formatting_only),
            Some(LockfileMode::Update),
        )
        .await;
        assert!(matches!(
            transaction.compute(&key).await.unwrap(),
            PathOutcome::Complete(1)
        ));
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        let activations = tracker.take();
        assert!(
            activations.iter().any(|activation| {
                activation.evaluated && activation.key.starts_with("bzlmod-host-file-bytes:")
            }),
            "formatting and file-kind changes must reevaluate Host file bytes"
        );
        assert!(
            activations.iter().any(|activation| {
                activation.evaluated && activation.key.starts_with("host-visible-lockfile:")
            }),
            "formatting and file-kind changes must reevaluate the Host lockfile owner"
        );

        transaction = update(
            transaction,
            &present_script(PathNodeKind::RegularFile, 30, semantic_b),
            Some(LockfileMode::Update),
        )
        .await;
        assert!(matches!(
            transaction.compute(&key).await.unwrap(),
            PathOutcome::Complete(2)
        ));
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }
}
