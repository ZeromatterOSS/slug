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

use crate::LockfileMode;
use crate::RegistryFileExpectation;
use crate::VisibleLockfileRead;
use crate::host_registry::RegistryKnownFileHashesMode;
use crate::module_eval::RootModuleFilesKey;
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

#[async_trait]
impl Key for RegistryPolicyKey {
    type Value = Arc<Result<RegistryPolicy, RegistryFileError>>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let urls = match ctx
            .compute(&RootModuleRegistryUrlsKey {
                workspace: self.workspace.clone(),
            })
            .await
        {
            Ok(urls) => urls.urls().dupe(),
            Err(error) => {
                return Arc::new(Err(RegistryFileError::MissingRegistryUrls(
                    CompactString::new(error.to_string()),
                )));
            }
        };
        let mode = match ctx
            .compute(&RootModuleLockfileModeKey {
                workspace: self.workspace.clone(),
            })
            .await
        {
            Ok(mode) => mode,
            Err(error) => {
                return Arc::new(Err(RegistryFileError::MissingLockfileMode(
                    CompactString::new(error.to_string()),
                )));
            }
        };
        let visible_lockfile = match ctx
            .compute(&RootModuleFilesKey {
                workspace: self.workspace.clone(),
            })
            .await
        {
            Ok(files) => match files.as_ref() {
                Ok(files) => files.visible_lockfile.clone(),
                Err(error) => {
                    return Arc::new(Err(RegistryFileError::RootModuleFiles(error.clone())));
                }
            },
            Err(error) => {
                return Arc::new(Err(RegistryFileError::RootModuleFiles(CompactString::new(
                    error.to_string(),
                ))));
            }
        };
        Arc::new(Ok(RegistryPolicy {
            urls,
            mode,
            visible_lockfile,
        }))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
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
