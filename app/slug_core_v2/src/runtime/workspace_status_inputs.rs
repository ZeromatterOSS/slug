//! Fresh, bounded Rust Host observations. These are raw inputs, never status bytes.
#![allow(dead_code)]

use std::sync::Arc;

use allocative::Allocative;
use dice::DiceTransactionUpdater;
use dice::InjectedKey;
use dice::UserComputationData;
use dupe::Dupe;
use slug_workspace_v2::NormalizedAbsolutePath;

const MAX_TEXT_BYTES: usize = 4096;
pub(super) const MAX_MILLIS: u64 = 253_402_300_799_999;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
pub(super) enum StatusError {
    UnsupportedPlatform,
    UnsupportedPolicy,
    SourceDateEpochPresent,
    MissingIdentity,
    NonUnicodeIdentity,
    OversizeIdentity,
    InvalidIdentity,
    HostRead,
    TimeRead,
    TimeRange,
    Calendar,
    Unauthorized,
    ForeignCapture,
}

impl std::fmt::Display for StatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "workspace status: {self:?}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) enum ObservedText {
    Absent,
    Text(Arc<str>),
    NonUnicode,
    TooLong,
    ReadError(StatusError),
}

impl ObservedText {
    pub(super) fn from_os(value: Option<std::ffi::OsString>) -> Self {
        match value {
            None => Self::Absent,
            Some(value) => match value.into_string() {
                Ok(value) if value.len() <= MAX_TEXT_BYTES => Self::Text(value.into()),
                Ok(_) => Self::TooLong,
                Err(_) => Self::NonUnicode,
            },
        }
    }

    pub(super) fn identity(&self) -> Result<&str, StatusError> {
        match self {
            Self::Text(text) if text.len() > MAX_TEXT_BYTES => Err(StatusError::OversizeIdentity),
            Self::Text(text) if text.is_empty() || text.contains(['\0', '\r', '\n']) => {
                Err(StatusError::InvalidIdentity)
            }
            Self::Text(text) => Ok(text),
            Self::Absent => Err(StatusError::MissingIdentity),
            Self::NonUnicode => Err(StatusError::NonUnicodeIdentity),
            Self::TooLong => Err(StatusError::OversizeIdentity),
            Self::ReadError(error) => Err(*error),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative)]
pub(super) enum StatusPolicy {
    DefaultEnglishUtc,
    UnsupportedEmbed,
    UnsupportedCommand,
    UnsupportedLocale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative)]
pub(super) enum StatusSource {
    UnixReportedUserKernelHost,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) struct RawWorkspaceStatusInputs {
    pub(super) user: ObservedText,
    pub(super) hostname: ObservedText,
    pub(super) uid: Result<(u32, u32), StatusError>,
    pub(super) source_date_epoch: ObservedText,
    pub(super) millis: Result<u64, StatusError>,
    pub(super) policy: StatusPolicy,
    pub(super) source: StatusSource,
}

pub(super) fn observe_native() -> Result<RawWorkspaceStatusInputs, StatusError> {
    #[cfg(unix)]
    {
        let hostname = match nix::unistd::gethostname() {
            Ok(value) => ObservedText::from_os(Some(value)),
            Err(_) => ObservedText::ReadError(StatusError::HostRead),
        };
        let user = ObservedText::from_os(std::env::var_os("USER"));
        let source_date_epoch = ObservedText::from_os(std::env::var_os("SOURCE_DATE_EPOCH"));
        let uid = Ok((
            nix::unistd::getuid().as_raw(),
            nix::unistd::geteuid().as_raw(),
        ));
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| StatusError::TimeRead)
            .and_then(|time| u64::try_from(time.as_millis()).map_err(|_| StatusError::TimeRange));
        Ok(RawWorkspaceStatusInputs {
            user,
            hostname,
            uid,
            source_date_epoch,
            millis,
            policy: StatusPolicy::DefaultEnglishUtc,
            source: StatusSource::UnixReportedUserKernelHost,
        })
    }
    #[cfg(not(unix))]
    {
        Err(StatusError::UnsupportedPlatform)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) struct WorkspaceStatusCapture {
    pub(super) workspace: NormalizedAbsolutePath,
    pub(super) request: u64,
    pub(super) raw: RawWorkspaceStatusInputs,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(super) enum WorkspaceStatusCaptureValue {
    Unauthorized,
    Present(Arc<WorkspaceStatusCapture>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(super) struct WorkspaceStatusCaptureKey(pub(super) NormalizedAbsolutePath);

impl std::fmt::Display for WorkspaceStatusCaptureKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "workspace-status-capture:{}", self.0)
    }
}

#[async_trait::async_trait]
impl InjectedKey for WorkspaceStatusCaptureKey {
    type Value = WorkspaceStatusCaptureValue;
    fn equality(left: &Self::Value, right: &Self::Value) -> bool {
        left == right
    }
}

/// Authenticity carrier only; the injected key owns the semantic dependency.
pub(super) struct WorkspaceStatusTransaction(pub(super) Arc<WorkspaceStatusCapture>);

pub(super) fn install(data: &mut UserComputationData, capture: &Arc<WorkspaceStatusCapture>) {
    data.data.set(WorkspaceStatusTransaction(capture.dupe()));
}

pub(super) fn inject(
    updater: &mut DiceTransactionUpdater,
    workspace: &NormalizedAbsolutePath,
    capture: Option<&Arc<WorkspaceStatusCapture>>,
) -> anyhow::Result<()> {
    if capture.is_some_and(|value| &value.workspace != workspace) {
        anyhow::bail!("{}", StatusError::ForeignCapture);
    }
    updater.changed_to([(
        WorkspaceStatusCaptureKey(workspace.dupe()),
        capture.map_or(WorkspaceStatusCaptureValue::Unauthorized, |value| {
            WorkspaceStatusCaptureValue::Present(value.dupe())
        }),
    )])?;
    Ok(())
}
