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
            Ok(urls) => urls.0,
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

enum RemotePolicy {
    FetchUnrecorded,
    ReuseRecordedAbsent,
    EnforceMissing,
    EnforceSha256([u8; 32]),
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

        let io = ctx
            .global_data()
            .get::<RegistryIoHandle>()
            .map_err(|_| RegistryFileError::MissingIoCapability)?
            .0
            .clone();
        match io.read_exact(&self.url).await {
            Ok(RegistryIoOutcome::Found(bytes)) => Ok(RegistryFileValue::Found {
                sha256: sha256(&bytes),
                bytes,
                recordable_remote_expectation: None,
            }),
            Ok(RegistryIoOutcome::NotFound) => {
                self.request_generation(ctx).await?;
                Ok(RegistryFileValue::NotFound {
                    source: RegistryNotFoundSource::LocalAbsence,
                    recordable_remote_expectation: None,
                })
            }
            Err(error) => {
                self.request_generation(ctx).await?;
                Err(RegistryFileError::LocalRead {
                    path,
                    message: error.message,
                })
            }
        }
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
        let remote_policy = self.remote_policy(policy)?;
        match remote_policy {
            RemotePolicy::ReuseRecordedAbsent => {
                return Ok(RegistryFileValue::NotFound {
                    source: RegistryNotFoundSource::RecordedAbsence,
                    recordable_remote_expectation: Some(RegistryFileExpectation::RecordedAbsent),
                });
            }
            RemotePolicy::EnforceMissing => {
                return Err(RegistryFileError::MissingChecksumInError {
                    url: self.url.clone(),
                });
            }
            RemotePolicy::FetchUnrecorded => {
                self.request_generation(ctx).await?;
                return self.fetch_remote(ctx, None, true).await;
            }
            RemotePolicy::EnforceSha256(expected) => {
                return self.fetch_remote(ctx, Some(expected), false).await;
            }
        }
    }

    fn remote_policy(&self, policy: &RegistryPolicy) -> Result<RemotePolicy, RegistryFileError> {
        let mode = policy.mode.semantic_mode();
        if matches!(mode, LockfileMode::Off) {
            return Ok(RemotePolicy::FetchUnrecorded);
        }
        let lockfile = match &policy.visible_lockfile {
            VisibleLockfileRead::Parsed(lockfile) => lockfile,
            VisibleLockfileRead::Ignored => {
                return Err(RegistryFileError::InvalidLockfileExpectation {
                    url: self.url.clone(),
                    message: CompactString::new(
                        "lockfile-reading mode received ignored visible lockfile",
                    ),
                });
            }
        };
        let expectation = lockfile
            .registry_file_expectation(self.url.as_str())
            .map_err(|error| RegistryFileError::InvalidLockfileExpectation {
                url: self.url.clone(),
                message: CompactString::new(error),
            })?;
        Ok(match (mode, expectation) {
            (LockfileMode::Update | LockfileMode::Refresh, RegistryFileExpectation::Unrecorded) => {
                RemotePolicy::FetchUnrecorded
            }
            (LockfileMode::Error, RegistryFileExpectation::Unrecorded) => {
                RemotePolicy::EnforceMissing
            }
            (
                LockfileMode::Update | LockfileMode::Error,
                RegistryFileExpectation::RecordedAbsent,
            ) => RemotePolicy::ReuseRecordedAbsent,
            (LockfileMode::Refresh, RegistryFileExpectation::RecordedAbsent) => {
                RemotePolicy::FetchUnrecorded
            }
            (
                LockfileMode::Update | LockfileMode::Refresh | LockfileMode::Error,
                RegistryFileExpectation::RecordedSha256(digest),
            ) => RemotePolicy::EnforceSha256(digest),
            (LockfileMode::Off, _) => unreachable!("off returned before lockfile access"),
        })
    }

    async fn fetch_remote(
        &self,
        ctx: &mut DiceComputations<'_>,
        expected: Option<[u8; 32]>,
        all_outcomes_retryable: bool,
    ) -> Result<RegistryFileValue, RegistryFileError> {
        let io = ctx
            .global_data()
            .get::<RegistryIoHandle>()
            .map_err(|_| RegistryFileError::MissingIoCapability)?
            .0
            .clone();
        match io.read_exact(&self.url).await {
            Ok(RegistryIoOutcome::Found(bytes)) => {
                let actual = sha256(&bytes);
                if let Some(expected) = expected
                    && actual != expected
                {
                    return Err(RegistryFileError::ChecksumMismatch {
                        url: self.url.clone(),
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
                    self.request_generation(ctx).await?;
                }
                Ok(RegistryFileValue::NotFound {
                    source: RegistryNotFoundSource::Io404,
                    recordable_remote_expectation: Some(RegistryFileExpectation::RecordedAbsent),
                })
            }
            Err(error) => {
                if !all_outcomes_retryable {
                    self.request_generation(ctx).await?;
                }
                Err(RegistryFileError::Transport {
                    url: self.url.clone(),
                    message: error.message,
                })
            }
        }
    }

    async fn request_generation(
        &self,
        ctx: &mut DiceComputations<'_>,
    ) -> Result<RegistryRequestGeneration, RegistryFileError> {
        ctx.compute(&RegistryRequestGenerationKey {
            workspace: self.workspace.clone(),
        })
        .await
        .map_err(|error| {
            RegistryFileError::MissingRequestGeneration(CompactString::new(error.to_string()))
        })
    }
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
