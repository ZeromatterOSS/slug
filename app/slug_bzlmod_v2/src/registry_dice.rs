/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fmt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use compact_str::CompactString;
use dice::DiceComputations;
use dice::DiceDataBuilder;
use dice::DiceTransactionUpdater;
use dice::InjectedKey;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use sha2::Digest;
use sha2::Sha256;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathObservationEpoch;

use crate::LockfileMode;
use crate::RegistryFileExpectation;
use crate::SourcePreparationOutcome;
use crate::VisibleLockfileRead;
use crate::host_registry::RegistryKnownFileHashesMode;
use crate::module_eval::RootModuleFilesKey;
use crate::module_eval::RootModuleFilesObservationKey;
use crate::module_eval::RootModuleLockfileMode;
use crate::module_eval::RootModuleLockfileModeKey;
use crate::registry::RegistryUrls;

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct RootModuleRegistryUrls(RegistryUrls);

impl From<RegistryUrls> for RootModuleRegistryUrls {
    fn from(urls: RegistryUrls) -> Self {
        Self(urls)
    }
}

impl RootModuleRegistryUrls {
    pub(crate) fn urls(&self) -> &RegistryUrls {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RootModuleRegistryUrlsKey {
    pub workspace: PathBuf,
}

impl fmt::Display for RootModuleRegistryUrlsKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "root-module-registry-urls:{}", self.workspace.display())
    }
}

impl InjectedKey for RootModuleRegistryUrlsKey {
    type Value = RootModuleRegistryUrls;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative, Dupe)]
pub struct RegistryRequestGeneration(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RegistryRequestGenerationKey {
    pub workspace: PathBuf,
}

impl fmt::Display for RegistryRequestGenerationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "registry-request-generation:{}",
            self.workspace.display()
        )
    }
}

impl InjectedKey for RegistryRequestGenerationKey {
    type Value = RegistryRequestGeneration;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RegistryPolicy {
    urls: RegistryUrls,
    mode: RootModuleLockfileMode,
    visible_lockfile: VisibleLockfileRead,
}

impl RegistryPolicy {
    pub fn urls(&self) -> &RegistryUrls {
        &self.urls
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RegistryPolicyKey {
    pub workspace: PathBuf,
}

impl fmt::Display for RegistryPolicyKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "registry-policy:{}", self.workspace.display())
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct RegistryPolicyObservationKey(RegistryPolicyKey);

#[allow(dead_code)]
impl RegistryPolicyObservationKey {
    pub(crate) fn new(workspace: PathBuf) -> Self {
        Self(RegistryPolicyKey { workspace })
    }
}

impl fmt::Display for RegistryPolicyObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

type RegistryPolicyResult = Arc<Result<RegistryPolicy, RegistryFileError>>;

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) struct ObservedRegistryPolicy {
    result: RegistryPolicyResult,
    observations: PathObservationEpoch,
}

#[allow(dead_code)]
impl ObservedRegistryPolicy {
    pub(crate) fn result(&self) -> &RegistryPolicyResult {
        &self.result
    }

    pub(crate) fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

type RegistryPolicyDriverOutcome = SourcePreparationOutcome<
    Result<(RegistryPolicyResult, PathObservationEpoch), ObservedPathFrontierError>,
>;

#[derive(Clone, Copy)]
enum RegistryPolicyMode {
    Legacy,
    Observed,
}

fn registry_policy_complete(
    result: Result<RegistryPolicy, RegistryFileError>,
    observations: PathObservationEpoch,
) -> RegistryPolicyDriverOutcome {
    SourcePreparationOutcome::Complete(Ok((Arc::new(result), observations)))
}

fn registry_policy_error(
    error: RegistryFileError,
    observations: PathObservationEpoch,
) -> RegistryPolicyDriverOutcome {
    registry_policy_complete(Err(error), observations)
}

fn finish_observed_registry_policy_root(
    outcome: <RootModuleFilesObservationKey as Key>::Value,
) -> Result<
    (
        Arc<Result<crate::module_eval::RootModuleFiles, CompactString>>,
        PathObservationEpoch,
    ),
    RegistryPolicyDriverOutcome,
> {
    match outcome {
        SourcePreparationOutcome::Need(need) => Err(SourcePreparationOutcome::Need(need)),
        SourcePreparationOutcome::Complete(Err(error)) => {
            Err(SourcePreparationOutcome::Complete(Err(error)))
        }
        SourcePreparationOutcome::Complete(Ok(observed)) => {
            Ok((observed.result().dupe(), observed.observations().dupe()))
        }
    }
}
fn registry_policy_root_compute_error(message: CompactString) -> RegistryPolicyDriverOutcome {
    registry_policy_error(
        RegistryFileError::RootModuleFiles(message),
        PathObservationEpoch::empty(),
    )
}
fn project_legacy_registry_policy(outcome: RegistryPolicyDriverOutcome) -> RegistryPolicyResult {
    match outcome {
        SourcePreparationOutcome::Complete(Ok((result, _))) => result,
        _ => panic!("legacy registry policy driver returned a nonsemantic outcome"),
    }
}

async fn drive_registry_policy(
    ctx: &mut DiceComputations<'_>,
    key: &RegistryPolicyKey,
    mode: RegistryPolicyMode,
) -> RegistryPolicyDriverOutcome {
    let urls = match ctx
        .compute(&RootModuleRegistryUrlsKey {
            workspace: key.workspace.clone(),
        })
        .await
    {
        Ok(urls) => urls.urls().dupe(),
        Err(error) => {
            return registry_policy_error(
                RegistryFileError::MissingRegistryUrls(error.to_string().into()),
                PathObservationEpoch::empty(),
            );
        }
    };
    let mode_value = match ctx
        .compute(&RootModuleLockfileModeKey {
            workspace: key.workspace.clone(),
        })
        .await
    {
        Ok(mode) => mode,
        Err(error) => {
            return registry_policy_error(
                RegistryFileError::MissingLockfileMode(error.to_string().into()),
                PathObservationEpoch::empty(),
            );
        }
    };
    let (files, observations) = match mode {
        RegistryPolicyMode::Legacy => {
            let files = match ctx
                .compute(&RootModuleFilesKey {
                    workspace: key.workspace.clone(),
                })
                .await
            {
                Ok(files) => files,
                Err(error) => {
                    return registry_policy_root_compute_error(error.to_string().into());
                }
            };
            (files, PathObservationEpoch::empty())
        }
        RegistryPolicyMode::Observed => {
            let workspace = match NormalizedAbsolutePath::new(key.workspace.clone()) {
                Ok(workspace) => workspace,
                Err(error) => {
                    return registry_policy_root_compute_error(error.to_string().into());
                }
            };
            let observed = match ctx
                .compute(&RootModuleFilesObservationKey::new(workspace))
                .await
            {
                Ok(observed) => observed,
                Err(error) => {
                    return registry_policy_root_compute_error(error.to_string().into());
                }
            };
            match finish_observed_registry_policy_root(observed) {
                Ok(root) => root,
                Err(outcome) => return outcome,
            }
        }
    };
    let visible_lockfile = match files.as_ref() {
        Ok(files) => files.visible_lockfile.clone(),
        Err(error) => {
            return registry_policy_error(
                RegistryFileError::RootModuleFiles(error.clone()),
                observations,
            );
        }
    };
    registry_policy_complete(
        Ok(RegistryPolicy {
            urls,
            mode: mode_value,
            visible_lockfile,
        }),
        observations,
    )
}

#[async_trait]
impl Key for RegistryPolicyKey {
    type Value = RegistryPolicyResult;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        project_legacy_registry_policy(
            drive_registry_policy(ctx, self, RegistryPolicyMode::Legacy).await,
        )
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[async_trait]
impl Key for RegistryPolicyObservationKey {
    type Value =
        SourcePreparationOutcome<Result<ObservedRegistryPolicy, ObservedPathFrontierError>>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        drive_registry_policy(ctx, &self.0, RegistryPolicyMode::Observed)
            .await
            .map(|outcome| {
                outcome.map(|(result, observations)| ObservedRegistryPolicy {
                    result,
                    observations,
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub struct RegistryFileUrl(Arc<str>);

impl RegistryFileUrl {
    pub fn new(url: impl AsRef<str>) -> Self {
        Self(Arc::from(url.as_ref()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum RegistryIoOutcome {
    Found(Arc<[u8]>),
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RegistryTransportError {
    pub message: CompactString,
}

#[async_trait]
pub trait RegistryIo: Send + Sync + 'static {
    async fn read_exact(
        &self,
        url: &RegistryFileUrl,
    ) -> Result<RegistryIoOutcome, RegistryTransportError>;

    async fn read_local_exact(
        &self,
        url: &RegistryFileUrl,
        path: &Path,
    ) -> Result<RegistryIoOutcome, RegistryTransportError> {
        let _ = path;
        self.read_exact(url).await
    }
}

struct RegistryIoHandle(Arc<dyn RegistryIo>);

pub fn install_registry_io(builder: &mut DiceDataBuilder, io: Arc<dyn RegistryIo>) {
    builder.set(RegistryIoHandle(io));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative)]
pub enum RegistryNotFoundSource {
    Io404,
    RecordedAbsence,
    LocalAbsence,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum RegistryFileValue {
    Found {
        bytes: Arc<[u8]>,
        sha256: [u8; 32],
        recordable_remote_expectation: Option<RegistryFileExpectation>,
    },
    NotFound {
        source: RegistryNotFoundSource,
        recordable_remote_expectation: Option<RegistryFileExpectation>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum RegistryFileError {
    InvalidFileUrl(RegistryFileUrl),
    UnsupportedUrl(RegistryFileUrl),
    MissingRegistryUrls(CompactString),
    MissingLockfileMode(CompactString),
    MissingRequestGeneration(CompactString),
    RootModuleFiles(CompactString),
    MissingIoCapability,
    MissingChecksumInError {
        url: RegistryFileUrl,
    },
    Transport {
        url: RegistryFileUrl,
        message: CompactString,
    },
    ChecksumMismatch {
        url: RegistryFileUrl,
        expected: [u8; 32],
        actual: [u8; 32],
    },
    LocalRead {
        path: PathBuf,
        message: CompactString,
    },
    InvalidLockfileExpectation {
        url: RegistryFileUrl,
        message: CompactString,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum RegistryRemoteError {
    InvalidRemoteHashesMode {
        url: RegistryFileUrl,
        mode: RegistryKnownFileHashesMode,
    },
    MissingRequestGeneration {
        workspace: PathBuf,
        url: RegistryFileUrl,
        message: CompactString,
    },
    MissingIoCapability {
        url: RegistryFileUrl,
    },
    MissingChecksumInError {
        url: RegistryFileUrl,
    },
    InvalidLockfileExpectation {
        url: RegistryFileUrl,
        message: CompactString,
    },
    Transport {
        url: RegistryFileUrl,
        message: CompactString,
    },
    ChecksumMismatch {
        url: RegistryFileUrl,
        expected: [u8; 32],
        actual: [u8; 32],
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum RegistryLocalError {
    MissingRequestGeneration {
        workspace: PathBuf,
        url: RegistryFileUrl,
        message: CompactString,
    },
    MissingIoCapability {
        url: RegistryFileUrl,
    },
    Read {
        url: RegistryFileUrl,
        path: PathBuf,
        message: CompactString,
    },
}

impl From<RegistryRemoteError> for RegistryFileError {
    fn from(error: RegistryRemoteError) -> Self {
        match error {
            RegistryRemoteError::InvalidRemoteHashesMode { url, .. } => Self::UnsupportedUrl(url),
            RegistryRemoteError::MissingRequestGeneration { message, .. } => {
                Self::MissingRequestGeneration(message)
            }
            RegistryRemoteError::MissingIoCapability { .. } => Self::MissingIoCapability,
            RegistryRemoteError::MissingChecksumInError { url } => {
                Self::MissingChecksumInError { url }
            }
            RegistryRemoteError::InvalidLockfileExpectation { url, message } => {
                Self::InvalidLockfileExpectation { url, message }
            }
            RegistryRemoteError::Transport { url, message } => Self::Transport { url, message },
            RegistryRemoteError::ChecksumMismatch {
                url,
                expected,
                actual,
            } => Self::ChecksumMismatch {
                url,
                expected,
                actual,
            },
        }
    }
}

impl From<RegistryLocalError> for RegistryFileError {
    fn from(error: RegistryLocalError) -> Self {
        match error {
            RegistryLocalError::MissingRequestGeneration { message, .. } => {
                Self::MissingRequestGeneration(message)
            }
            RegistryLocalError::MissingIoCapability { .. } => Self::MissingIoCapability,
            RegistryLocalError::Read { path, message, .. } => Self::LocalRead { path, message },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RegistryFileKey {
    pub workspace: PathBuf,
    pub url: RegistryFileUrl,
}

impl fmt::Display for RegistryFileKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "registry-file:{}", self.url.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative)]
enum RegistryIoPlan {
    FetchUnverified,
    ReplayRecordedAbsent,
    RejectUnrecorded,
    VerifySha256([u8; 32]),
}

#[async_trait]
impl Key for RegistryFileKey {
    type Value = Arc<Result<RegistryFileValue, RegistryFileError>>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        if self.url.as_str().starts_with("file:") {
            return Arc::new(self.compute_local(ctx).await);
        }
        if !self.url.as_str().starts_with("http://") && !self.url.as_str().starts_with("https://") {
            return Arc::new(Err(RegistryFileError::UnsupportedUrl(self.url.clone())));
        }
        Arc::new(self.compute_remote(ctx).await)
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

impl RegistryFileKey {
    async fn compute_local(
        &self,
        ctx: &mut DiceComputations<'_>,
    ) -> Result<RegistryFileValue, RegistryFileError> {
        let Some(path) = self.url.as_str().strip_prefix("file://") else {
            return Err(RegistryFileError::InvalidFileUrl(self.url.clone()));
        };
        if !path.starts_with('/') {
            return Err(RegistryFileError::InvalidFileUrl(self.url.clone()));
        }
        let path = PathBuf::from(path);
        let policy = ctx
            .compute(&RegistryPolicyKey {
                workspace: self.workspace.clone(),
            })
            .await
            .map_err(|error| {
                RegistryFileError::RootModuleFiles(CompactString::new(error.to_string()))
            })?;
        policy.as_ref().as_ref().map_err(|error| error.clone())?;
        // Keep the direct edge even though RegistryPolicyKey also reads root files: the policy
        // value deliberately projects them down to lockfile visibility, whereas a local registry
        // success must replay when the root module's own semantics change.
        let root_files = ctx
            .compute(&RootModuleFilesKey {
                workspace: self.workspace.clone(),
            })
            .await
            .map_err(|error| {
                RegistryFileError::RootModuleFiles(CompactString::new(error.to_string()))
            })?;
        root_files
            .as_ref()
            .as_ref()
            .map_err(|error| RegistryFileError::RootModuleFiles(error.clone()))?;
        read_local_registry_file(ctx, &self.workspace, &self.url, &path)
            .await
            .map_err(RegistryFileError::from)
    }

    async fn compute_remote(
        &self,
        ctx: &mut DiceComputations<'_>,
    ) -> Result<RegistryFileValue, RegistryFileError> {
        let policy = ctx
            .compute(&RegistryPolicyKey {
                workspace: self.workspace.clone(),
            })
            .await
            .map_err(|error| {
                RegistryFileError::RootModuleFiles(CompactString::new(error.to_string()))
            })?;
        let policy = policy.as_ref().as_ref().map_err(|error| error.clone())?;
        let mode = policy.mode.semantic_mode();
        read_remote_registry_file(
            ctx,
            &self.workspace,
            &self.url,
            &mode,
            &policy.visible_lockfile,
        )
        .await
        .map_err(RegistryFileError::from)
    }
}

pub(crate) async fn read_remote_registry_file(
    ctx: &mut DiceComputations<'_>,
    workspace: &Path,
    url: &RegistryFileUrl,
    mode: &LockfileMode,
    visible_lockfile: &VisibleLockfileRead,
) -> Result<RegistryFileValue, RegistryRemoteError> {
    let plan = if matches!(mode, LockfileMode::Off) {
        RegistryIoPlan::FetchUnverified
    } else {
        let lockfile = match visible_lockfile {
            VisibleLockfileRead::Parsed(lockfile) => lockfile,
            VisibleLockfileRead::Ignored => {
                return Err(RegistryRemoteError::InvalidLockfileExpectation {
                    url: url.dupe(),
                    message: CompactString::new(
                        "lockfile-reading mode received ignored visible lockfile",
                    ),
                });
            }
        };
        let expectation = lockfile
            .registry_file_expectation(url.as_str())
            .map_err(|error| RegistryRemoteError::InvalidLockfileExpectation {
                url: url.dupe(),
                message: CompactString::new(error),
            })?;
        let hashes_mode = match mode {
            LockfileMode::Update => RegistryKnownFileHashesMode::UseAndUpdate,
            LockfileMode::Refresh => RegistryKnownFileHashesMode::UseImmutableAndUpdate,
            LockfileMode::Error => RegistryKnownFileHashesMode::Enforce,
            LockfileMode::Off => {
                unreachable!("legacy Off selected its plan before lockfile access")
            }
        };
        host_remote_plan(url, hashes_mode, expectation)?
    };
    execute_remote_registry_plan(ctx, workspace, url, plan).await
}

#[allow(dead_code)] // Dormant until the Host registry-file owner consumes the bridge.
pub(crate) async fn read_host_remote_registry_file(
    ctx: &mut DiceComputations<'_>,
    workspace: &Path,
    url: &RegistryFileUrl,
    hashes_mode: RegistryKnownFileHashesMode,
    expectation: RegistryFileExpectation,
) -> Result<RegistryFileValue, RegistryRemoteError> {
    let plan = host_remote_plan(url, hashes_mode, expectation)?;
    execute_remote_registry_plan(ctx, workspace, url, plan).await
}

fn host_remote_plan(
    url: &RegistryFileUrl,
    hashes_mode: RegistryKnownFileHashesMode,
    expectation: RegistryFileExpectation,
) -> Result<RegistryIoPlan, RegistryRemoteError> {
    if hashes_mode == RegistryKnownFileHashesMode::Ignore {
        return Err(RegistryRemoteError::InvalidRemoteHashesMode {
            url: url.dupe(),
            mode: hashes_mode,
        });
    }
    Ok(match (hashes_mode, expectation) {
        (
            RegistryKnownFileHashesMode::UseAndUpdate
            | RegistryKnownFileHashesMode::UseImmutableAndUpdate,
            RegistryFileExpectation::Unrecorded,
        ) => RegistryIoPlan::FetchUnverified,
        (
            RegistryKnownFileHashesMode::UseAndUpdate | RegistryKnownFileHashesMode::Enforce,
            RegistryFileExpectation::RecordedAbsent,
        ) => RegistryIoPlan::ReplayRecordedAbsent,
        (
            RegistryKnownFileHashesMode::UseImmutableAndUpdate,
            RegistryFileExpectation::RecordedAbsent,
        ) => RegistryIoPlan::FetchUnverified,
        (RegistryKnownFileHashesMode::Enforce, RegistryFileExpectation::Unrecorded) => {
            RegistryIoPlan::RejectUnrecorded
        }
        (
            RegistryKnownFileHashesMode::UseAndUpdate
            | RegistryKnownFileHashesMode::UseImmutableAndUpdate
            | RegistryKnownFileHashesMode::Enforce,
            RegistryFileExpectation::RecordedSha256(digest),
        ) => RegistryIoPlan::VerifySha256(digest),
        (RegistryKnownFileHashesMode::Ignore, _) => {
            unreachable!("Ignore returned a typed routing error")
        }
    })
}

async fn execute_remote_registry_plan(
    ctx: &mut DiceComputations<'_>,
    workspace: &Path,
    url: &RegistryFileUrl,
    plan: RegistryIoPlan,
) -> Result<RegistryFileValue, RegistryRemoteError> {
    match plan {
        RegistryIoPlan::ReplayRecordedAbsent => Ok(RegistryFileValue::NotFound {
            source: RegistryNotFoundSource::RecordedAbsence,
            recordable_remote_expectation: Some(RegistryFileExpectation::RecordedAbsent),
        }),
        RegistryIoPlan::RejectUnrecorded => {
            Err(RegistryRemoteError::MissingChecksumInError { url: url.dupe() })
        }
        RegistryIoPlan::FetchUnverified => {
            request_remote_generation(ctx, workspace, url).await?;
            fetch_remote_registry_file(ctx, workspace, url, None, true).await
        }
        RegistryIoPlan::VerifySha256(expected) => {
            fetch_remote_registry_file(ctx, workspace, url, Some(expected), false).await
        }
    }
}

async fn fetch_remote_registry_file(
    ctx: &mut DiceComputations<'_>,
    workspace: &Path,
    url: &RegistryFileUrl,
    expected: Option<[u8; 32]>,
    all_outcomes_retryable: bool,
) -> Result<RegistryFileValue, RegistryRemoteError> {
    let io = ctx
        .global_data()
        .get::<RegistryIoHandle>()
        .map_err(|_| RegistryRemoteError::MissingIoCapability { url: url.dupe() })?
        .0
        .clone();
    match io.read_exact(url).await {
        Ok(RegistryIoOutcome::Found(bytes)) => {
            let actual = sha256(&bytes);
            if let Some(expected) = expected
                && actual != expected
            {
                return Err(RegistryRemoteError::ChecksumMismatch {
                    url: url.dupe(),
                    expected,
                    actual,
                });
            }
            Ok(RegistryFileValue::Found {
                bytes,
                sha256: actual,
                recordable_remote_expectation: Some(RegistryFileExpectation::RecordedSha256(
                    actual,
                )),
            })
        }
        Ok(RegistryIoOutcome::NotFound) => {
            if !all_outcomes_retryable {
                request_remote_generation(ctx, workspace, url).await?;
            }
            Ok(RegistryFileValue::NotFound {
                source: RegistryNotFoundSource::Io404,
                recordable_remote_expectation: Some(RegistryFileExpectation::RecordedAbsent),
            })
        }
        Err(error) => {
            if !all_outcomes_retryable {
                request_remote_generation(ctx, workspace, url).await?;
            }
            Err(RegistryRemoteError::Transport {
                url: url.dupe(),
                message: error.message,
            })
        }
    }
}

async fn request_remote_generation(
    ctx: &mut DiceComputations<'_>,
    workspace: &Path,
    url: &RegistryFileUrl,
) -> Result<RegistryRequestGeneration, RegistryRemoteError> {
    ctx.compute(&RegistryRequestGenerationKey {
        workspace: workspace.to_path_buf(),
    })
    .await
    .map_err(|error| RegistryRemoteError::MissingRequestGeneration {
        workspace: workspace.to_path_buf(),
        url: url.dupe(),
        message: CompactString::new(error.to_string()),
    })
}

pub(crate) async fn read_local_registry_file(
    ctx: &mut DiceComputations<'_>,
    workspace: &Path,
    url: &RegistryFileUrl,
    path: &Path,
) -> Result<RegistryFileValue, RegistryLocalError> {
    let io = ctx
        .global_data()
        .get::<RegistryIoHandle>()
        .map_err(|_| RegistryLocalError::MissingIoCapability { url: url.dupe() })?
        .0
        .clone();
    match io.read_local_exact(url, path).await {
        Ok(RegistryIoOutcome::Found(bytes)) => Ok(RegistryFileValue::Found {
            sha256: sha256(&bytes),
            bytes,
            recordable_remote_expectation: None,
        }),
        Ok(RegistryIoOutcome::NotFound) => {
            request_local_generation(ctx, workspace, url).await?;
            Ok(RegistryFileValue::NotFound {
                source: RegistryNotFoundSource::LocalAbsence,
                recordable_remote_expectation: None,
            })
        }
        Err(error) => {
            request_local_generation(ctx, workspace, url).await?;
            Err(RegistryLocalError::Read {
                url: url.dupe(),
                path: path.to_path_buf(),
                message: error.message,
            })
        }
    }
}

async fn request_local_generation(
    ctx: &mut DiceComputations<'_>,
    workspace: &Path,
    url: &RegistryFileUrl,
) -> Result<RegistryRequestGeneration, RegistryLocalError> {
    ctx.compute(&RegistryRequestGenerationKey {
        workspace: workspace.to_path_buf(),
    })
    .await
    .map_err(|error| RegistryLocalError::MissingRequestGeneration {
        workspace: workspace.to_path_buf(),
        url: url.dupe(),
        message: CompactString::new(error.to_string()),
    })
}

pub fn inject_registry_request_inputs(
    updater: &mut DiceTransactionUpdater,
    workspace: &std::path::Path,
    urls: RegistryUrls,
    generation: RegistryRequestGeneration,
) -> anyhow::Result<()> {
    updater.changed_to(vec![(
        RootModuleRegistryUrlsKey {
            workspace: workspace.to_path_buf(),
        },
        RootModuleRegistryUrls::from(urls),
    )])?;
    updater.changed_to(vec![(
        RegistryRequestGenerationKey {
            workspace: workspace.to_path_buf(),
        },
        generation,
    )])?;
    Ok(())
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

#[cfg(test)]
mod bridge_tests {
    use std::sync::Mutex;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use dice::ActivationData;
    use dice::ActivationTracker;
    use dice::DetectCycles;
    use dice::Dice;
    use dice::DiceTransaction;
    use dice::DynKey;
    use dice::UserComputationData;

    use super::*;

    const WORKSPACE: &str = "/registry-bridge";
    const REMOTE_URL: &str = "https://registry.example/file";
    const LOCAL_URL: &str = "file:///registry-bridge/absent-decoy";
    const LOCAL_PATH: &str = "/registry-bridge/file";

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Response {
        Found(Arc<[u8]>),
        NotFound,
        Transport(CompactString),
    }

    #[derive(Debug)]
    struct ScriptedIo {
        response: Mutex<Response>,
        calls: AtomicUsize,
        last_local_call: Mutex<Option<(RegistryFileUrl, PathBuf)>>,
    }

    impl ScriptedIo {
        fn new(response: Response) -> Self {
            Self {
                response: Mutex::new(response),
                calls: AtomicUsize::new(0),
                last_local_call: Mutex::new(None),
            }
        }
        fn set(&self, response: Response) {
            *self.response.lock().unwrap() = response;
        }
        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
        fn last_local_call(&self) -> Option<(RegistryFileUrl, PathBuf)> {
            self.last_local_call.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl RegistryIo for ScriptedIo {
        async fn read_exact(
            &self,
            _url: &RegistryFileUrl,
        ) -> Result<RegistryIoOutcome, RegistryTransportError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            match self.response.lock().unwrap().clone() {
                Response::Found(bytes) => Ok(RegistryIoOutcome::Found(bytes)),
                Response::NotFound => Ok(RegistryIoOutcome::NotFound),
                Response::Transport(message) => Err(RegistryTransportError { message }),
            }
        }

        async fn read_local_exact(
            &self,
            url: &RegistryFileUrl,
            path: &Path,
        ) -> Result<RegistryIoOutcome, RegistryTransportError> {
            *self.last_local_call.lock().unwrap() = Some((url.dupe(), path.to_path_buf()));
            self.read_exact(url).await
        }
    }

    fn found(bytes: &'static [u8]) -> Response {
        Response::Found(Arc::from(bytes))
    }
    fn remote_found(bytes: &'static [u8]) -> RegistryFileValue {
        let actual = sha256(bytes);
        RegistryFileValue::Found {
            bytes: Arc::from(bytes),
            sha256: actual,
            recordable_remote_expectation: Some(RegistryFileExpectation::RecordedSha256(actual)),
        }
    }
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative)]
    enum RemoteRequest {
        Host {
            mode: RegistryKnownFileHashesMode,
            expectation: RegistryFileExpectation,
        },
        LegacyOff,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
    struct RemoteTestKey {
        workspace: PathBuf,
        url: RegistryFileUrl,
        request: RemoteRequest,
    }
    impl fmt::Display for RemoteTestKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "registry-remote-bridge-test:{}", self.url.as_str())
        }
    }

    #[async_trait]
    impl Key for RemoteTestKey {
        type Value = Arc<Result<RegistryFileValue, RegistryRemoteError>>;

        async fn compute(
            &self,
            ctx: &mut DiceComputations,
            _cancellations: &CancellationContext,
        ) -> Self::Value {
            let value = match self.request {
                RemoteRequest::Host { mode, expectation } => {
                    read_host_remote_registry_file(
                        ctx,
                        &self.workspace,
                        &self.url,
                        mode,
                        expectation,
                    )
                    .await
                }
                RemoteRequest::LegacyOff => {
                    read_remote_registry_file(
                        ctx,
                        &self.workspace,
                        &self.url,
                        &LockfileMode::Off,
                        &VisibleLockfileRead::Ignored,
                    )
                    .await
                }
            };
            Arc::new(value)
        }

        fn equality(x: &Self::Value, y: &Self::Value) -> bool {
            x == y
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
    struct LocalTestKey {
        workspace: PathBuf,
        url: RegistryFileUrl,
        path: PathBuf,
    }
    impl fmt::Display for LocalTestKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "registry-local-bridge-test:{}", self.url.as_str())
        }
    }

    #[async_trait]
    impl Key for LocalTestKey {
        type Value = Arc<Result<RegistryFileValue, RegistryLocalError>>;

        async fn compute(
            &self,
            ctx: &mut DiceComputations,
            _cancellations: &CancellationContext,
        ) -> Self::Value {
            Arc::new(read_local_registry_file(ctx, &self.workspace, &self.url, &self.path).await)
        }

        fn equality(x: &Self::Value, y: &Self::Value) -> bool {
            x == y
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum DirectDependency {
        Generation,
        Forbidden,
    }
    #[derive(Default)]
    struct DirectTracker(Mutex<Vec<Vec<DirectDependency>>>);
    impl DirectTracker {
        fn last(&self) -> Vec<DirectDependency> {
            self.0.lock().unwrap().last().unwrap().clone()
        }
    }

    impl ActivationTracker for DirectTracker {
        fn key_activated(
            &self,
            key: &DynKey,
            dependencies: &mut dyn Iterator<Item = &DynKey>,
            _activation: ActivationData,
        ) {
            if key.downcast_ref::<RemoteTestKey>().is_none()
                && key.downcast_ref::<LocalTestKey>().is_none()
            {
                return;
            }
            self.0.lock().unwrap().push(
                dependencies
                    .map(|dependency| {
                        if dependency
                            .downcast_ref::<RegistryRequestGenerationKey>()
                            .is_some()
                        {
                            DirectDependency::Generation
                        } else {
                            DirectDependency::Forbidden
                        }
                    })
                    .collect(),
            );
        }
    }

    fn dice(io: Option<Arc<ScriptedIo>>) -> Arc<Dice> {
        let mut builder = Dice::builder();
        if let Some(io) = io {
            install_registry_io(&mut builder, io);
        }
        builder.build(DetectCycles::Enabled)
    }
    async fn transaction(
        dice: &Arc<Dice>,
        generation: Option<u64>,
        tracker: Option<Arc<dyn ActivationTracker>>,
    ) -> DiceTransaction {
        let data = UserComputationData {
            activation_tracker: tracker,
            ..Default::default()
        };
        let mut updater = dice.updater_with_data(data);
        if let Some(generation) = generation {
            updater
                .changed_to(vec![(
                    RegistryRequestGenerationKey {
                        workspace: PathBuf::from(WORKSPACE),
                    },
                    RegistryRequestGeneration(generation),
                )])
                .unwrap();
        }
        updater.commit().await
    }

    fn remote_key(request: RemoteRequest) -> RemoteTestKey {
        RemoteTestKey {
            workspace: PathBuf::from(WORKSPACE),
            url: RegistryFileUrl::new(REMOTE_URL),
            request,
        }
    }
    fn local_key(url: &str, path: &str) -> LocalTestKey {
        LocalTestKey {
            workspace: PathBuf::from(WORKSPACE),
            url: RegistryFileUrl::new(url),
            path: PathBuf::from(path),
        }
    }
    async fn fresh_remote(
        request: RemoteRequest,
        response: Option<Response>,
        generation: Option<u64>,
    ) -> (
        Arc<Result<RegistryFileValue, RegistryRemoteError>>,
        usize,
        Vec<DirectDependency>,
    ) {
        let io = response.map(|response| Arc::new(ScriptedIo::new(response)));
        let tracker = Arc::new(DirectTracker::default());
        let dice = dice(io.clone());
        let mut tx = transaction(
            &dice,
            generation,
            Some(tracker.dupe() as Arc<dyn ActivationTracker>),
        )
        .await;
        let value = tx.compute(&remote_key(request)).await.unwrap();
        (value, io.map_or(0, |io| io.calls()), tracker.last())
    }

    #[test]
    fn remote_hash_mode_expectation_matrix_and_error_equality_are_exact() {
        let digest = [7; 32];
        for mode in [
            RegistryKnownFileHashesMode::Ignore,
            RegistryKnownFileHashesMode::UseAndUpdate,
            RegistryKnownFileHashesMode::UseImmutableAndUpdate,
            RegistryKnownFileHashesMode::Enforce,
        ] {
            for expectation in [
                RegistryFileExpectation::Unrecorded,
                RegistryFileExpectation::RecordedAbsent,
                RegistryFileExpectation::RecordedSha256(digest),
            ] {
                let actual = host_remote_plan(&RegistryFileUrl::new(REMOTE_URL), mode, expectation);
                if mode == RegistryKnownFileHashesMode::Ignore {
                    assert_eq!(
                        actual,
                        Err(RegistryRemoteError::InvalidRemoteHashesMode {
                            url: RegistryFileUrl::new(REMOTE_URL),
                            mode,
                        })
                    );
                    continue;
                }
                let expected = match (mode, expectation) {
                    (
                        RegistryKnownFileHashesMode::UseAndUpdate
                        | RegistryKnownFileHashesMode::UseImmutableAndUpdate,
                        RegistryFileExpectation::Unrecorded,
                    ) => RegistryIoPlan::FetchUnverified,
                    (
                        RegistryKnownFileHashesMode::UseAndUpdate
                        | RegistryKnownFileHashesMode::Enforce,
                        RegistryFileExpectation::RecordedAbsent,
                    ) => RegistryIoPlan::ReplayRecordedAbsent,
                    (
                        RegistryKnownFileHashesMode::UseImmutableAndUpdate,
                        RegistryFileExpectation::RecordedAbsent,
                    ) => RegistryIoPlan::FetchUnverified,
                    (RegistryKnownFileHashesMode::Enforce, RegistryFileExpectation::Unrecorded) => {
                        RegistryIoPlan::RejectUnrecorded
                    }
                    (_, RegistryFileExpectation::RecordedSha256(digest)) => {
                        RegistryIoPlan::VerifySha256(digest)
                    }
                    (RegistryKnownFileHashesMode::Ignore, _) => unreachable!(),
                };
                assert_eq!(actual, Ok(expected));
            }
        }
    }

    #[tokio::test]
    async fn bridge_order_missing_precedence_values_and_dependencies_are_exact() {
        use DirectDependency::Generation;
        let unverified = RemoteRequest::Host {
            mode: RegistryKnownFileHashesMode::UseAndUpdate,
            expectation: RegistryFileExpectation::Unrecorded,
        };
        let (value, calls, dependencies) =
            fresh_remote(unverified, Some(found(b"bytes")), None).await;
        assert!(matches!(
            value.as_ref(),
            Err(RegistryRemoteError::MissingRequestGeneration {
                workspace,
                url,
                ..
            }) if workspace == Path::new(WORKSPACE) && url.as_str() == REMOTE_URL
        ));
        assert_eq!((calls, dependencies), (0, vec![]));
        let (value, calls, dependencies) = fresh_remote(unverified, None, Some(1)).await;
        assert_eq!(
            value.as_ref(),
            &Err(RegistryRemoteError::MissingIoCapability {
                url: RegistryFileUrl::new(REMOTE_URL),
            })
        );
        assert_eq!((calls, dependencies), (0, vec![Generation]));
        let (value, calls, dependencies) =
            fresh_remote(unverified, Some(Response::NotFound), Some(1)).await;
        assert_eq!(
            value.as_ref(),
            &Ok(RegistryFileValue::NotFound {
                source: RegistryNotFoundSource::Io404,
                recordable_remote_expectation: Some(RegistryFileExpectation::RecordedAbsent),
            })
        );
        assert_eq!((calls, dependencies), (1, vec![Generation]));
        let digest = sha256(b"expected");
        let verified = RemoteRequest::Host {
            mode: RegistryKnownFileHashesMode::Enforce,
            expectation: RegistryFileExpectation::RecordedSha256(digest),
        };
        let (value, calls, dependencies) =
            fresh_remote(verified, Some(found(b"expected")), None).await;
        assert_eq!(value.as_ref(), &Ok(remote_found(b"expected")));
        assert_eq!((calls, dependencies), (1, vec![]));
        let actual = sha256(b"wrong");
        let (value, calls, dependencies) =
            fresh_remote(verified, Some(found(b"wrong")), None).await;
        assert_eq!(
            value.as_ref(),
            &Err(RegistryRemoteError::ChecksumMismatch {
                url: RegistryFileUrl::new(REMOTE_URL),
                expected: digest,
                actual,
            })
        );
        assert_eq!((calls, dependencies), (1, vec![]));
        let (value, calls, dependencies) =
            fresh_remote(verified, Some(Response::NotFound), None).await;
        assert!(matches!(
            value.as_ref(),
            Err(RegistryRemoteError::MissingRequestGeneration { .. })
        ));
        assert_eq!((calls, dependencies), (1, vec![]));
        for (request, expected) in [
            (
                RemoteRequest::Host {
                    mode: RegistryKnownFileHashesMode::UseAndUpdate,
                    expectation: RegistryFileExpectation::RecordedAbsent,
                },
                Ok(RegistryFileValue::NotFound {
                    source: RegistryNotFoundSource::RecordedAbsence,
                    recordable_remote_expectation: Some(RegistryFileExpectation::RecordedAbsent),
                }),
            ),
            (
                RemoteRequest::Host {
                    mode: RegistryKnownFileHashesMode::Enforce,
                    expectation: RegistryFileExpectation::Unrecorded,
                },
                Err(RegistryRemoteError::MissingChecksumInError {
                    url: RegistryFileUrl::new(REMOTE_URL),
                }),
            ),
            (
                RemoteRequest::Host {
                    mode: RegistryKnownFileHashesMode::Ignore,
                    expectation: RegistryFileExpectation::RecordedSha256(digest),
                },
                Err(RegistryRemoteError::InvalidRemoteHashesMode {
                    url: RegistryFileUrl::new(REMOTE_URL),
                    mode: RegistryKnownFileHashesMode::Ignore,
                }),
            ),
        ] {
            let (value, calls, dependencies) = fresh_remote(request, None, None).await;
            assert_eq!((calls, dependencies), (0, vec![]));
            assert_eq!(value.as_ref(), &expected);
        }
        let (value, calls, dependencies) =
            fresh_remote(RemoteRequest::LegacyOff, Some(found(b"legacy")), None).await;
        assert!(matches!(
            value.as_ref(),
            Err(RegistryRemoteError::MissingRequestGeneration { .. })
        ));
        assert_eq!((calls, dependencies), (0, vec![]));
        let (value, calls, dependencies) = fresh_remote(verified, None, None).await;
        assert!(matches!(
            value.as_ref(),
            Err(RegistryRemoteError::MissingIoCapability { url })
                if url.as_str() == REMOTE_URL
        ));
        assert_eq!((calls, dependencies), (0, vec![]));
        let (value, calls, dependencies) =
            fresh_remote(verified, Some(Response::Transport("offline".into())), None).await;
        assert!(matches!(
            value.as_ref(),
            Err(RegistryRemoteError::MissingRequestGeneration { .. })
        ));
        assert_eq!((calls, dependencies), (1, vec![]));
    }

    #[tokio::test]
    async fn remote_bridge_retained_retry_and_stickiness_are_exact() {
        let io = Arc::new(ScriptedIo::new(Response::Transport("offline".into())));
        let dice = dice(Some(io.dupe()));
        let key = remote_key(RemoteRequest::LegacyOff);
        let mut tx = transaction(&dice, Some(1), None).await;
        let transport = RegistryRemoteError::Transport {
            url: RegistryFileUrl::new(REMOTE_URL),
            message: "offline".into(),
        };
        assert_eq!(
            tx.compute(&key).await.unwrap().as_ref(),
            &Err(transport.clone())
        );
        assert_eq!(
            transport,
            RegistryRemoteError::Transport {
                url: RegistryFileUrl::new(REMOTE_URL),
                message: CompactString::new("offline"),
            }
        );
        io.set(found(b"recovered"));
        let mut same = transaction(&dice, Some(1), None).await;
        assert!(matches!(
            same.compute(&key).await.unwrap().as_ref(),
            Err(RegistryRemoteError::Transport { .. })
        ));
        assert_eq!(io.calls(), 1);
        let mut next = transaction(&dice, Some(2), None).await;
        assert_eq!(
            next.compute(&key).await.unwrap().as_ref(),
            &Ok(remote_found(b"recovered"))
        );
        assert_eq!(io.calls(), 2);
        io.set(found(b"new generation"));
        let mut third = transaction(&dice, Some(3), None).await;
        assert_eq!(
            third.compute(&key).await.unwrap().as_ref(),
            &Ok(remote_found(b"new generation"))
        );
        assert_eq!(io.calls(), 3);
        let expected = b"verified";
        let verified = remote_key(RemoteRequest::Host {
            mode: RegistryKnownFileHashesMode::UseAndUpdate,
            expectation: RegistryFileExpectation::RecordedSha256(sha256(expected)),
        });
        io.set(Response::NotFound);
        let mut missing = transaction(&dice, Some(3), None).await;
        assert!(matches!(
            missing.compute(&verified).await.unwrap().as_ref(),
            Ok(RegistryFileValue::NotFound {
                source: RegistryNotFoundSource::Io404,
                ..
            })
        ));
        io.set(found(expected));
        let mut retry = transaction(&dice, Some(4), None).await;
        assert!(matches!(
            retry.compute(&verified).await.unwrap().as_ref(),
            Ok(RegistryFileValue::Found { bytes, .. }) if bytes.as_ref() == expected
        ));
        let calls = io.calls();
        io.set(Response::Transport("must stay sticky".into()));
        let mut later = transaction(&dice, Some(5), None).await;
        assert!(matches!(
            later.compute(&verified).await.unwrap().as_ref(),
            Ok(RegistryFileValue::Found { bytes, .. }) if bytes.as_ref() == expected
        ));
        assert_eq!(io.calls(), calls);
        let wanted = sha256(b"wanted");
        let mismatch = remote_key(RemoteRequest::Host {
            mode: RegistryKnownFileHashesMode::Enforce,
            expectation: RegistryFileExpectation::RecordedSha256(wanted),
        });
        io.set(found(b"wrong"));
        let mut first = transaction(&dice, Some(6), None).await;
        assert!(matches!(
            first.compute(&mismatch).await.unwrap().as_ref(),
            Err(RegistryRemoteError::ChecksumMismatch { .. })
        ));
        let calls = io.calls();
        io.set(found(b"wanted"));
        let mut changed_generation = transaction(&dice, Some(7), None).await;
        assert!(matches!(
            changed_generation
                .compute(&mismatch)
                .await
                .unwrap()
                .as_ref(),
            Err(RegistryRemoteError::ChecksumMismatch { .. })
        ));
        assert_eq!(io.calls(), calls);
    }

    #[tokio::test]
    async fn local_bridge_typed_retry_stickiness_and_dependencies_are_exact() {
        use DirectDependency::Generation;
        let key = local_key(LOCAL_URL, LOCAL_PATH);
        let io = Arc::new(ScriptedIo::new(Response::NotFound));
        let engine = dice(Some(io.dupe()));
        let tracker = Arc::new(DirectTracker::default());
        let mut first = transaction(
            &engine,
            Some(1),
            Some(tracker.dupe() as Arc<dyn ActivationTracker>),
        )
        .await;
        assert_eq!(
            first.compute(&key).await.unwrap().as_ref(),
            &Ok(RegistryFileValue::NotFound {
                source: RegistryNotFoundSource::LocalAbsence,
                recordable_remote_expectation: None,
            })
        );
        assert_eq!(
            io.last_local_call(),
            Some((RegistryFileUrl::new(LOCAL_URL), PathBuf::from(LOCAL_PATH)))
        );
        assert_eq!(tracker.last(), vec![Generation]);
        io.set(found(b"created"));
        let mut same = transaction(&engine, Some(1), None).await;
        assert!(matches!(
            same.compute(&key).await.unwrap().as_ref(),
            Ok(RegistryFileValue::NotFound { .. })
        ));
        assert_eq!(io.calls(), 1);
        let mut created = transaction(&engine, Some(2), None).await;
        let created_sha = sha256(b"created");
        assert_eq!(
            created.compute(&key).await.unwrap().as_ref(),
            &Ok(RegistryFileValue::Found {
                bytes: Arc::from(&b"created"[..]),
                sha256: created_sha,
                recordable_remote_expectation: None,
            })
        );
        io.set(Response::Transport("must stay sticky".into()));
        let mut later = transaction(&engine, Some(3), None).await;
        assert!(matches!(
            later.compute(&key).await.unwrap().as_ref(),
            Ok(RegistryFileValue::Found { bytes, .. }) if bytes.as_ref() == b"created"
        ));
        assert_eq!(io.calls(), 2);
        let error_key = local_key("file:///registry-bridge/error", "/registry-bridge/error");
        let mut failed = transaction(&engine, Some(4), None).await;
        let error = failed.compute(&error_key).await.unwrap();
        assert_eq!(
            error.as_ref(),
            &Err(RegistryLocalError::Read {
                url: RegistryFileUrl::new("file:///registry-bridge/error"),
                path: PathBuf::from("/registry-bridge/error"),
                message: "must stay sticky".into(),
            })
        );
        let separately_allocated = RegistryLocalError::Read {
            url: RegistryFileUrl::new("file:///registry-bridge/error"),
            path: PathBuf::from("/registry-bridge/error"),
            message: CompactString::new("must stay sticky"),
        };
        assert_eq!(error.as_ref().as_ref().unwrap_err(), &separately_allocated);
        io.set(found(b"repaired"));
        let mut same = transaction(&engine, Some(4), None).await;
        assert!(same.compute(&error_key).await.unwrap().is_err());
        let mut repaired = transaction(&engine, Some(5), None).await;
        assert!(matches!(
            repaired.compute(&error_key).await.unwrap().as_ref(),
            Ok(RegistryFileValue::Found { bytes, .. }) if bytes.as_ref() == b"repaired"
        ));
        let missing_io_dice = dice(None);
        let tracker = Arc::new(DirectTracker::default());
        let mut missing_io = transaction(
            &missing_io_dice,
            None,
            Some(tracker.dupe() as Arc<dyn ActivationTracker>),
        )
        .await;
        assert_eq!(
            missing_io.compute(&key).await.unwrap().as_ref(),
            &Err(RegistryLocalError::MissingIoCapability {
                url: RegistryFileUrl::new(LOCAL_URL)
            })
        );
        assert_eq!(tracker.last(), vec![]);
        let io = Arc::new(ScriptedIo::new(Response::NotFound));
        let missing_generation_dice = dice(Some(io.dupe()));
        let mut missing_generation = transaction(&missing_generation_dice, None, None).await;
        assert!(matches!(
            missing_generation.compute(&key).await.unwrap().as_ref(),
            Err(RegistryLocalError::MissingRequestGeneration {
                workspace,
                url,
                ..
            }) if workspace == Path::new(WORKSPACE) && url.as_str() == LOCAL_URL
        ));
        assert_eq!(io.calls(), 1);

        let io = Arc::new(ScriptedIo::new(Response::Transport("offline".into())));
        let missing_generation_dice = dice(Some(io.dupe()));
        let mut missing_generation = transaction(&missing_generation_dice, None, None).await;
        assert!(matches!(
            missing_generation.compute(&key).await.unwrap().as_ref(),
            Err(RegistryLocalError::MissingRequestGeneration {
                workspace,
                url,
                ..
            }) if workspace == Path::new(WORKSPACE) && url.as_str() == LOCAL_URL
        ));
        assert_eq!(io.calls(), 1);
        assert_eq!(
            io.last_local_call(),
            Some((RegistryFileUrl::new(LOCAL_URL), PathBuf::from(LOCAL_PATH)))
        );
    }
}

#[cfg(all(test, unix))]
mod policy_observation_tests {
    use std::collections::HashSet;
    use std::sync::Mutex;

    use dice::ActivationData;
    use dice::ActivationKind;
    use dice::ActivationTracker;
    use dice::DetectCycles;
    use dice::Dice;
    use dice::DynKey;
    use dice::RichActivation;
    use dice::UserComputationData;
    use slug_events_v2::CaptureEvaluationEvents;
    use slug_events_v2::EvaluationEvent;
    use slug_events_v2::EventBatch;
    use slug_workspace_v2::NeedPathObservations;
    use slug_workspace_v2::PathLstat;
    use slug_workspace_v2::PathNodeKind;
    use slug_workspace_v2::PathObservationDemand;
    use slug_workspace_v2::PathObservationEpochError;
    use slug_workspace_v2::PathObservationEpochKey;
    use slug_workspace_v2::PathObservationNamespace;
    use slug_workspace_v2::PathObservationOperation;
    use slug_workspace_v2::PathObservationResult;
    use slug_workspace_v2::PathOperationResult;
    use slug_workspace_v2::WorkspaceFileValue;
    use slug_workspace_v2::WorkspaceRawFileValue;
    use slug_workspace_v2::WorkspaceRawSnapshot;
    use slug_workspace_v2::WorkspaceRawSnapshotKey;
    use slug_workspace_v2::WorkspaceSnapshot;
    use slug_workspace_v2::WorkspaceSnapshotKey;
    use starlark_map::small_map::SmallMap;
    use starlark_map::sorted_map::SortedMap;

    use super::*;
    use crate::BzlmodCommandPolicyKey;
    use crate::BzlmodEnvironmentPolicyKey;
    use crate::RootPackagePolicyInputs;
    use crate::SourcePreparationNeeds;
    use crate::inject_root_module_request_inputs;
    use crate::inject_root_package_policy_inputs;

    const WORKSPACE: &str = "/policy-observation";
    const MODULE: &str = "module(name='root')\nprint('policy')";
    const MODULE_B: &str = "module(name='other')\nprint('policy')";
    const LOCKFILE_A: &str = r#"{"lockFileVersion":28}"#;
    const LOCKFILE_B: &str = r#"{"lockFileVersion":28,"moduleExtensions":{}}"#;
    const REGISTRY_A: &str = "https://registry-a";
    const REGISTRY_B: &str = "https://registry-b";
    fn workspace() -> NormalizedAbsolutePath {
        NormalizedAbsolutePath::new(WORKSPACE).unwrap()
    }

    fn present(kind: PathNodeKind, variant: i64, permissions: i32) -> PathObservationResult {
        PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
            kind,
            variant,
            variant,
            variant,
            variant,
            permissions,
        )))
    }
    fn epoch(source: &str, lockfile: &str, variant: i64) -> PathObservationEpoch {
        let mut entries = SmallMap::new();
        for path in ["/", WORKSPACE] {
            entries.insert(
                PathObservationDemand::new(
                    PathObservationNamespace::Host,
                    NormalizedAbsolutePath::new(path).unwrap(),
                    PathObservationOperation::Lstat,
                ),
                present(PathNodeKind::Directory, variant, 0o755),
            );
        }
        for (path, bytes) in [
            (format!("{WORKSPACE}/MODULE.bazel"), source),
            (format!("{WORKSPACE}/MODULE.bazel.lock"), lockfile),
        ] {
            let path = NormalizedAbsolutePath::new(path).unwrap();
            entries.insert(
                PathObservationDemand::new(
                    PathObservationNamespace::Host,
                    path.dupe(),
                    PathObservationOperation::Lstat,
                ),
                present(PathNodeKind::RegularFile, variant, 0o644),
            );
            entries.insert(
                PathObservationDemand::new(
                    PathObservationNamespace::Host,
                    path,
                    PathObservationOperation::FileBytes,
                ),
                PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                    bytes.as_bytes(),
                ))),
            );
        }
        PathObservationEpoch::new(entries).unwrap()
    }

    fn snapshot(source: &str, lockfile: &str) -> Arc<WorkspaceSnapshot> {
        Arc::new(WorkspaceSnapshot {
            files: Arc::new(SortedMap::from_iter([
                (
                    PathBuf::from(format!("{WORKSPACE}/MODULE.bazel")),
                    WorkspaceFileValue::Present(Arc::new(source.to_owned())),
                ),
                (
                    PathBuf::from(format!("{WORKSPACE}/MODULE.bazel.lock")),
                    WorkspaceFileValue::Present(Arc::new(lockfile.to_owned())),
                ),
            ])),
        })
    }

    fn inject(
        updater: &mut DiceTransactionUpdater,
        source: &str,
        lockfile: &str,
        mode: LockfileMode,
        registry: &str,
        variant: i64,
        observe: bool,
    ) -> PathObservationEpoch {
        let epoch = epoch(source, lockfile, variant);
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                if observe {
                    epoch.dupe()
                } else {
                    PathObservationEpoch::empty()
                },
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                WorkspaceSnapshotKey {
                    workspace: PathBuf::from(WORKSPACE),
                },
                snapshot(source, lockfile),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                WorkspaceRawSnapshotKey {
                    workspace: PathBuf::from(WORKSPACE),
                },
                Arc::new(WorkspaceRawSnapshot {
                    files: Arc::new(SortedMap::from_iter([(
                        PathBuf::from(format!("{WORKSPACE}/MODULE.bazel.lock")),
                        WorkspaceRawFileValue::Present(Arc::from(lockfile.as_bytes())),
                    )])),
                }),
            )])
            .unwrap();
        inject_root_package_policy_inputs(
            updater,
            RootPackagePolicyInputs::new(
                workspace(),
                [workspace()],
                std::iter::empty::<&str>(),
                None,
                Some("warning"),
            )
            .unwrap(),
        )
        .unwrap();
        inject_root_module_request_inputs(
            updater,
            Path::new(WORKSPACE),
            BzlmodCommandPolicyKey::from_flags_with_module_overrides(
                None,
                false,
                Path::new(WORKSPACE),
                std::iter::empty::<&str>(),
            )
            .unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            mode,
        )
        .unwrap();
        inject_registry_request_inputs(
            updater,
            Path::new(WORKSPACE),
            RegistryUrls::new([registry]),
            RegistryRequestGeneration(variant as u64),
        )
        .unwrap();
        epoch
    }
    fn inject_a(
        updater: &mut DiceTransactionUpdater,
        variant: i64,
        observe: bool,
    ) -> PathObservationEpoch {
        inject(
            updater,
            MODULE,
            LOCKFILE_A,
            LockfileMode::Update,
            REGISTRY_A,
            variant,
            observe,
        )
    }

    #[derive(Default)]
    struct Tracker {
        rows: Mutex<Vec<(String, Vec<String>)>>,
        batches: Mutex<Vec<(String, ActivationKind, Option<EventBatch>)>>,
    }

    impl Tracker {
        fn take(
            &self,
        ) -> (
            Vec<(String, Vec<String>)>,
            Vec<(String, ActivationKind, Option<EventBatch>)>,
        ) {
            (
                std::mem::take(&mut *self.rows.lock().unwrap()),
                std::mem::take(&mut *self.batches.lock().unwrap()),
            )
        }
    }

    impl ActivationTracker for Tracker {
        fn key_activated(
            &self,
            key: &DynKey,
            deps: &mut dyn Iterator<Item = &DynKey>,
            _: ActivationData,
        ) {
            self.rows
                .lock()
                .unwrap()
                .push((key.to_string(), deps.map(ToString::to_string).collect()));
        }

        fn tracks_rich_activations(&self) -> bool {
            true
        }

        fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
            self.batches.lock().unwrap().push((
                key.to_string(),
                activation.kind(),
                activation
                    .evaluation_data()
                    .and_then(|data| data.downcast_ref::<EventBatch>())
                    .map(Dupe::dupe),
            ));
        }
    }
    fn assert_no_root_files_activation(tracker: &Tracker) {
        let (rows, _) = tracker.take();
        assert!(rows.iter().all(|(owner, deps)| {
            [
                "root-module-files:",
                "observed-root-module-files:",
                "registry-file:",
                "module-source-preparation:",
                "host-discovered-module:",
                "host-selected-module-graph:",
                "host-registry:",
                "host-pure-module-extension:",
                "host-instantiated-module-extension:",
                "host-validated-module-extension:",
            ]
            .iter()
            .all(|prefix| {
                !owner.starts_with(prefix)
                    && deps
                        .iter()
                        .all(|dependency| !dependency.starts_with(prefix))
            })
        }));
    }

    fn updater(dice: &Arc<Dice>, tracker: Arc<Tracker>) -> DiceTransactionUpdater {
        let mut data = UserComputationData {
            activation_tracker: Some(tracker as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        data.data.set(CaptureEvaluationEvents);
        dice.updater_with_data(data)
    }

    fn row<'a>(rows: &'a [(String, Vec<String>)], owner: &str) -> &'a [String] {
        rows.iter()
            .find(|(candidate, _)| candidate == owner)
            .unwrap()
            .1
            .as_slice()
    }
    fn events<'a>(
        batches: &'a [(String, ActivationKind, Option<EventBatch>)],
        prefix: &str,
    ) -> Vec<&'a EventBatch> {
        batches
            .iter()
            .filter_map(|(owner, kind, batch)| {
                (owner.starts_with(prefix) && *kind == ActivationKind::Evaluated)
                    .then(|| batch.as_ref())
                    .flatten()
            })
            .collect()
    }

    fn complete(outcome: &<RegistryPolicyObservationKey as Key>::Value) -> &ObservedRegistryPolicy {
        let SourcePreparationOutcome::Complete(Ok(observed)) = outcome else {
            panic!("observed registry policy did not complete: {outcome:?}");
        };
        observed
    }

    #[test]
    fn observed_registry_policy_identity_and_root_control_flow_are_exact() {
        let a = RegistryPolicyObservationKey::new(PathBuf::from(WORKSPACE));
        let b = RegistryPolicyObservationKey::new(PathBuf::from("/other"));
        assert_eq!(
            a.to_string(),
            format!("observed-registry-policy:{WORKSPACE}")
        );
        assert_eq!(HashSet::from([a, b]).len(), 2);
        let result: RegistryPolicyResult =
            Arc::new(Err(RegistryFileError::MissingLockfileMode("arc".into())));
        let projected = project_legacy_registry_policy(SourcePreparationOutcome::Complete(Ok((
            result.dupe(),
            PathObservationEpoch::empty(),
        ))));
        assert!(Arc::ptr_eq(&result, &projected));
        let demand = PathObservationDemand::new(
            PathObservationNamespace::Host,
            workspace(),
            PathObservationOperation::Lstat,
        );
        let root_need = SourcePreparationOutcome::Need(SourcePreparationNeeds::path(
            NeedPathObservations::singleton(demand.dupe()),
        ));
        assert!(matches!(
            finish_observed_registry_policy_root(root_need),
            Err(SourcePreparationOutcome::Need(_))
        ));
        let need: <RegistryPolicyObservationKey as Key>::Value = SourcePreparationOutcome::Need(
            SourcePreparationNeeds::path(NeedPathObservations::singleton(demand.dupe())),
        );
        assert!(!RegistryPolicyObservationKey::validity(&need));
        assert!(!RegistryPolicyObservationKey::equality(&need, &need));
        let root_outer = SourcePreparationOutcome::Complete(Err(ObservedPathFrontierError::from(
            PathObservationEpochError::DuplicateDemand(demand.dupe()),
        )));
        assert!(matches!(
            finish_observed_registry_policy_root(root_outer),
            Err(SourcePreparationOutcome::Complete(Err(_)))
        ));
        let outer: <RegistryPolicyObservationKey as Key>::Value =
            SourcePreparationOutcome::Complete(Err(ObservedPathFrontierError::from(
                PathObservationEpochError::DuplicateDemand(demand),
            )));
        assert!(RegistryPolicyObservationKey::validity(&outer));
        assert!(RegistryPolicyObservationKey::equality(&outer, &outer));
    }

    #[tokio::test]
    async fn observed_registry_policy_preserves_family_epoch_events_and_lifecycle() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(Tracker::default());
        let mut held: Option<(
            <RegistryPolicyObservationKey as Key>::Value,
            RegistryPolicyResult,
            PathObservationEpoch,
        )> = None;
        for index in 0..9 {
            let source = if index == 5 { MODULE_B } else { MODULE };
            let lockfile = if index == 7 { LOCKFILE_B } else { LOCKFILE_A };
            let mode = if index == 3 {
                LockfileMode::Refresh
            } else {
                LockfileMode::Update
            };
            let registry = if index == 1 { REGISTRY_B } else { REGISTRY_A };
            let variant = if index == 5 || index == 7 { 2 } else { 1 };
            let same_as_a = index % 2 == 0;
            let mut update = updater(&dice, tracker.dupe());
            let injected = inject(&mut update, source, lockfile, mode, registry, variant, true);
            let mut tx = update.commit().await;
            let key = RegistryPolicyObservationKey::new(PathBuf::from(WORKSPACE));
            let root = tx
                .compute(&RootModuleFilesObservationKey::new(workspace()))
                .await
                .unwrap();
            let SourcePreparationOutcome::Complete(Ok(root)) = root else {
                panic!("root observation did not complete");
            };
            let (_, root_batches) = tracker.take();
            let outcome = tx.compute(&key).await.unwrap();
            let observed = complete(&outcome);
            assert_eq!(observed.observations(), &injected);
            let (observed_rows, observed_batches) = tracker.take();
            for (demand, result) in observed.observations().observations() {
                assert!(Arc::ptr_eq(
                    result,
                    root.observations().observations().get(demand).unwrap()
                ));
            }
            assert_eq!(
                row(&observed_rows, &key.to_string()),
                vec![
                    format!("root-module-registry-urls:{WORKSPACE}"),
                    format!("root-module-lockfile-mode:{WORKSPACE}"),
                    format!("observed-root-module-files:\"{WORKSPACE}\""),
                ]
            );
            let parent = |owner: &String, _: &ActivationKind, _: &Option<EventBatch>| {
                owner == &key.to_string()
            };
            assert!(
                observed_batches
                    .iter()
                    .filter(|(o, k, b)| parent(o, k, b))
                    .all(|(_, _, batch)| batch.is_none())
            );
            assert!(
                index != 0
                    || observed_batches
                        .iter()
                        .any(|(o, k, b)| parent(o, k, b) && *k == ActivationKind::Evaluated)
            );
            assert!(observed_rows.iter().all(|(owner, deps)| {
                std::iter::once(owner).chain(deps).all(|name| {
                    !name.starts_with("root-module-files:")
                        && !name.starts_with("registry-file:")
                        && !name.starts_with("module-source-preparation:")
                        && !name.starts_with("host-discovered-module:")
                        && !name.starts_with("host-selected-module-graph:")
                        && !name.starts_with("host-registry:")
                        && !name.starts_with("host-pure-module-extension:")
                        && !name.starts_with("host-instantiated-module-extension:")
                        && !name.starts_with("host-validated-module-extension:")
                })
            }));

            let legacy_key = RegistryPolicyKey {
                workspace: PathBuf::from(WORKSPACE),
            };
            tx.compute(&RootModuleFilesKey {
                workspace: PathBuf::from(WORKSPACE),
            })
            .await
            .unwrap();
            let (_, legacy_child_batches) = tracker.take();
            let legacy = tx.compute(&legacy_key).await.unwrap();
            assert_eq!(observed.result().as_ref(), legacy.as_ref());
            let (legacy_rows, legacy_parent_batches) = tracker.take();
            assert_eq!(
                row(&legacy_rows, &legacy_key.to_string()),
                vec![
                    format!("root-module-registry-urls:{WORKSPACE}"),
                    format!("root-module-lockfile-mode:{WORKSPACE}"),
                    format!("root-module-files:{WORKSPACE}"),
                ]
            );
            tx.compute(&key).await.unwrap();
            let (_, warm_batches) = tracker.take();
            assert!(
                legacy_parent_batches
                    .iter()
                    .filter(|(owner, _, _)| owner == &legacy_key.to_string())
                    .all(|(_, _, batch)| batch.is_none())
            );
            assert!(
                index != 0
                    || legacy_parent_batches
                        .iter()
                        .any(|(owner, kind, _)| owner == &legacy_key.to_string()
                            && *kind == ActivationKind::Evaluated)
            );
            if index == 0 {
                let observed_batches =
                    events(&root_batches, "bzlmod-observed-host-root-module-file:");
                let legacy_batches = events(&legacy_child_batches, "root-module-evaluation:");
                assert_eq!(observed_batches, legacy_batches);
                assert!(matches!(observed_batches.as_slice(), [batch]
                    if matches!(batch.events(),
                        [EvaluationEvent::StarlarkPrint { text, .. }] if text == "policy")));
            }
            assert!(warm_batches.iter().all(|(_, _, batch)| batch.is_none()));

            assert!(legacy_rows.iter().all(|(owner, deps)| {
                std::iter::once(owner).chain(deps).all(|name| {
                    !name.starts_with("observed-root-module-files:")
                        && !name.starts_with("bzlmod-observed-host-root-module-file:")
                })
            }));
            if let Some((held_outcome, held_result, held_epoch)) = &held {
                assert_eq!(
                    RegistryPolicyObservationKey::equality(held_outcome, &outcome),
                    same_as_a
                );
                if same_as_a {
                    assert_eq!(held_result.as_ref(), observed.result().as_ref());
                    assert_eq!(held_epoch, observed.observations());
                }
            } else {
                held = Some((
                    outcome.clone(),
                    observed.result().dupe(),
                    observed.observations().dupe(),
                ));
            }
        }
    }

    #[tokio::test]
    async fn observed_registry_policy_stops_on_inputs_need_and_cancellation_then_recovers() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let key = RegistryPolicyObservationKey::new(PathBuf::from(WORKSPACE));
        let tracker = Arc::new(Tracker::default());
        let mut tx = updater(&dice, tracker.dupe()).commit().await;
        let missing_urls = tx.compute(&key).await.unwrap();
        let missing_urls = complete(&missing_urls);
        assert!(matches!(
            missing_urls.result().as_ref(),
            Err(RegistryFileError::MissingRegistryUrls(_))
        ));
        assert!(missing_urls.observations().observations().is_empty());
        assert_no_root_files_activation(&tracker);

        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut update = updater(&dice, tracker.dupe());
        update
            .changed_to(vec![(
                RootModuleRegistryUrlsKey {
                    workspace: PathBuf::from(WORKSPACE),
                },
                RootModuleRegistryUrls::from(RegistryUrls::new(["https://registry"])),
            )])
            .unwrap();
        let mut tx = update.commit().await;
        let missing_mode = complete(&tx.compute(&key).await.unwrap()).dupe();
        assert!(matches!(
            missing_mode.result().as_ref(),
            Err(RegistryFileError::MissingLockfileMode(_))
        ));
        assert!(missing_mode.observations().observations().is_empty());
        assert_no_root_files_activation(&tracker);

        let SourcePreparationOutcome::Complete(Ok((result, observations))) =
            registry_policy_root_compute_error("root compute".into())
        else {
            panic!("root compute projection was not semantic");
        };
        assert_eq!(
            result.as_ref(),
            &Err(RegistryFileError::RootModuleFiles("root compute".into()))
        );
        assert!(observations.observations().is_empty());

        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut update = updater(&dice, tracker.dupe());
        inject_a(&mut update, 7, false);
        let mut tx = update.commit().await;
        let mut future = Box::pin(tx.compute(&key));
        std::future::poll_fn(|context| {
            assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        drop(future);
        drop(tx);
        let (_, cancelled_batches) = tracker.take();
        assert!(
            cancelled_batches
                .iter()
                .all(|(owner, _, batch)| owner != &key.to_string() || batch.is_none())
        );
        let mut update = updater(&dice, tracker.dupe());
        inject_a(&mut update, 7, false);
        let mut tx = update.commit().await;
        let need = tx.compute(&key).await.unwrap();
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        assert!(!RegistryPolicyObservationKey::validity(&need));
        let (need_rows, need_batches) = tracker.take();
        assert_eq!(
            row(&need_rows, &key.to_string()),
            [
                format!("root-module-registry-urls:{WORKSPACE}"),
                format!("root-module-lockfile-mode:{WORKSPACE}"),
                format!("observed-root-module-files:\"{WORKSPACE}\""),
            ]
        );
        assert!(
            need_batches
                .iter()
                .all(|(owner, _, batch)| owner != &key.to_string() || batch.is_none())
        );

        let mut update = updater(&dice, tracker.dupe());
        let expected = inject_a(&mut update, 7, true);
        let mut tx = update.commit().await;
        let recovered = tx.compute(&key).await.unwrap();
        assert_eq!(complete(&recovered).observations(), &expected);

        let mut update = updater(&dice, tracker);
        let expected = inject(
            &mut update,
            "module(name=",
            LOCKFILE_A,
            LockfileMode::Update,
            "https://registry",
            8,
            true,
        );
        let mut tx = update.commit().await;
        let semantic = tx.compute(&key).await.unwrap();
        let semantic = complete(&semantic);
        assert!(matches!(
            semantic.result().as_ref(),
            Err(RegistryFileError::RootModuleFiles(_))
        ));
        let actual = semantic.observations().observations();
        assert_eq!(actual.len(), 4);
        assert!(actual.iter().all(|(demand, result)| Arc::ptr_eq(
            result,
            expected.observations().get(demand).unwrap()
        )));
    }
}
