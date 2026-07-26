/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use allocative::Allocative;
use allocative::Key;
use allocative::Visitor;

use crate::BzlmodHiddenLockfileDigest;
use crate::BzlmodVisibleLockfileDigest;
use crate::ModuleKey;
use crate::dice::LockfileMode;
use crate::host_lockfile::HostVisibleLockfileError;
pub use crate::lockfile_v28::AdapterDomain;
pub use crate::lockfile_v28::BazelLockfile;
use crate::lockfile_v28::LOCK_FILE_VERSION_28;
pub use crate::lockfile_v28::LockfileParseError;
pub use crate::lockfile_v28::LockfileParseErrorKind;
pub use crate::lockfile_v28::LockfileParseErrorSurface;
use crate::lockfile_v28::LockfileReadOutcome;
pub use crate::lockfile_v28::LockfileRenderError;
pub use crate::lockfile_v28::LockfileRenderErrorKind;
use crate::lockfile_v28::RegistryFileHash;
pub use crate::lockfile_v28::SourcePosition;
use crate::lockfile_v28::UnsupportedVersionPolicy;
use crate::lockfile_v28::read_lockfile_v28;
use crate::lockfile_v28::render_lockfile_v28;

pub const BAZEL_9_LOCK_FILE_VERSION: i32 = LOCK_FILE_VERSION_28;

impl Allocative for ModuleKey {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        visitor.visit_field(Key::new("name"), &self.name);
        visitor.visit_field(Key::new("version"), &self.version);
        visitor.exit();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative)]
pub enum RegistryFileExpectation {
    Unrecorded,
    RecordedAbsent,
    RecordedSha256([u8; 32]),
}

impl BazelLockfile {
    pub fn registry_file_expectation(&self, url: &str) -> Result<RegistryFileExpectation, String> {
        Ok(match self.registry_file_hashes.get(url) {
            None => RegistryFileExpectation::Unrecorded,
            Some(RegistryFileHash::NotFound) => RegistryFileExpectation::RecordedAbsent,
            Some(RegistryFileHash::Sha256(digest)) => {
                RegistryFileExpectation::RecordedSha256(*digest)
            }
        })
    }
}

pub fn empty_bazel_lockfile() -> BazelLockfile {
    BazelLockfile::default()
}

pub fn parse_bazel_lockfile(content: &str) -> Result<BazelLockfile, LockfileParseError> {
    parse_bazel_lockfile_bytes(content.as_bytes())
}

fn parse_bazel_lockfile_bytes(content: &[u8]) -> Result<BazelLockfile, LockfileParseError> {
    match read_lockfile_v28(content, UnsupportedVersionPolicy::Error)? {
        LockfileReadOutcome::Parsed(lockfile) => Ok(lockfile),
        LockfileReadOutcome::Empty => {
            unreachable!("Error policy never returns an empty lockfile outcome")
        }
    }
}

pub fn render_bazel_lockfile(lockfile: &BazelLockfile) -> Result<String, LockfileRenderError> {
    render_lockfile_v28(lockfile).map(Into::into)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VisibleLockfilePlan {
    Ignore,
    Keep,
    Write { content: String },
    Error { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VisibleLockfileApply {
    Ignored,
    Kept,
    Written { bytes: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleLockfileInput {
    digest: BzlmodVisibleLockfileDigest,
    content: Option<Arc<[u8]>>,
}

impl VisibleLockfileInput {
    pub fn absent() -> Self {
        Self {
            digest: BzlmodVisibleLockfileDigest::absent(),
            content: None,
        }
    }

    pub fn from_optional_bytes(content: Option<&[u8]>) -> Result<Self, String> {
        let Some(content) = content else {
            return Ok(Self::absent());
        };
        Ok(Self {
            digest: BzlmodVisibleLockfileDigest::from_content(content),
            content: Some(Arc::from(content)),
        })
    }

    pub fn digest(&self) -> &BzlmodVisibleLockfileDigest {
        &self.digest
    }

    pub fn existing_bytes(&self) -> Option<&[u8]> {
        self.content.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HiddenLockfileInput {
    digest: BzlmodHiddenLockfileDigest,
    content: Option<Arc<[u8]>>,
}

impl HiddenLockfileInput {
    pub fn absent() -> Self {
        Self {
            digest: BzlmodHiddenLockfileDigest::absent(),
            content: None,
        }
    }

    pub fn from_optional_bytes(content: Option<&[u8]>) -> Result<Self, String> {
        let Some(content) = content else {
            return Ok(Self::absent());
        };
        Ok(Self {
            digest: BzlmodHiddenLockfileDigest::from_content(content),
            content: Some(Arc::from(content)),
        })
    }

    pub fn digest(&self) -> &BzlmodHiddenLockfileDigest {
        &self.digest
    }

    pub fn existing_bytes(&self) -> Option<&[u8]> {
        self.content.as_deref()
    }

    pub fn parse_fail_open(&self) -> Result<BazelLockfile, LockfileParseError> {
        parse_hidden_lockfile_fail_open(self.existing_bytes())
    }
}

pub fn parse_hidden_lockfile_fail_open(
    existing_content: Option<&[u8]>,
) -> Result<BazelLockfile, LockfileParseError> {
    let Some(existing_content) = existing_content else {
        return Ok(empty_bazel_lockfile());
    };
    match read_lockfile_v28(existing_content, UnsupportedVersionPolicy::ReturnEmpty) {
        Ok(LockfileReadOutcome::Empty) => Ok(empty_bazel_lockfile()),
        Ok(LockfileReadOutcome::Parsed(lockfile)) => Ok(lockfile),
        Err(error) if is_caught_parse_surface(error.surface()) => Ok(empty_bazel_lockfile()),
        Err(error) => Err(error),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum VisibleLockfileRead {
    Ignored,
    Parsed(Arc<BazelLockfile>),
}

impl VisibleLockfileRead {
    pub fn parsed(&self) -> Option<&BazelLockfile> {
        match self {
            Self::Ignored => None,
            Self::Parsed(lockfile) => Some(lockfile),
        }
    }
}

pub fn parse_visible_lockfile_for_mode(
    mode: &LockfileMode,
    input: &VisibleLockfileInput,
) -> Result<VisibleLockfileRead, String> {
    parse_visible_lockfile_bytes_for_mode(mode, input.existing_bytes())
}

pub(crate) fn parse_visible_lockfile_content_for_mode(
    mode: &LockfileMode,
    existing_content: Option<&str>,
) -> Result<VisibleLockfileRead, String> {
    parse_visible_lockfile_bytes_for_mode(mode, existing_content.map(str::as_bytes))
}

pub(crate) fn parse_visible_lockfile_bytes_for_mode(
    mode: &LockfileMode,
    existing_content: Option<&[u8]>,
) -> Result<VisibleLockfileRead, String> {
    if matches!(mode, LockfileMode::Off) {
        return Ok(VisibleLockfileRead::Ignored);
    }
    let Some(existing_content) = existing_content else {
        return Ok(VisibleLockfileRead::Parsed(empty_bazel_lockfile().into()));
    };
    let policy = if matches!(mode, LockfileMode::Error) {
        UnsupportedVersionPolicy::Error
    } else {
        UnsupportedVersionPolicy::ReturnEmpty
    };
    match read_lockfile_v28(existing_content, policy) {
        Ok(LockfileReadOutcome::Empty) => {
            Ok(VisibleLockfileRead::Parsed(empty_bazel_lockfile().into()))
        }
        Ok(LockfileReadOutcome::Parsed(lockfile)) => {
            Ok(VisibleLockfileRead::Parsed(lockfile.into()))
        }
        Err(error)
            if error.surface() == LockfileParseErrorSurface::UnsupportedVersion
                && matches!(mode, LockfileMode::Error) =>
        {
            Err(unsupported_lockfile_version_message())
        }
        Err(error) if is_caught_parse_surface(error.surface()) => {
            Err(bad_visible_lockfile_message(error))
        }
        Err(error) => Err(error.to_string()),
    }
}

pub(crate) fn parse_visible_lockfile_bytes_for_host(
    mode: &LockfileMode,
    existing_content: Option<&[u8]>,
) -> Result<Arc<BazelLockfile>, HostVisibleLockfileError> {
    let Some(existing_content) = existing_content else {
        return Ok(Arc::new(empty_bazel_lockfile()));
    };
    let policy = if matches!(mode, LockfileMode::Error) {
        UnsupportedVersionPolicy::Error
    } else {
        UnsupportedVersionPolicy::ReturnEmpty
    };
    match read_lockfile_v28(existing_content, policy) {
        Ok(LockfileReadOutcome::Empty) => Ok(Arc::new(empty_bazel_lockfile())),
        Ok(LockfileReadOutcome::Parsed(lockfile)) => Ok(Arc::new(lockfile)),
        Err(error)
            if error.surface() == LockfileParseErrorSurface::UnsupportedVersion
                || is_caught_parse_surface(error.surface()) =>
        {
            let message = if error.surface() == LockfileParseErrorSurface::UnsupportedVersion {
                unsupported_lockfile_version_message().into()
            } else {
                let error_message = error.to_string();
                let suffix = if ["<<<<<<<", "=======", "|||||||", ">>>>>>>"]
                    .iter()
                    .any(|marker| error_message.contains(marker))
                {
                    " This looks like a merge conflict. See https://bazel.build/external/lockfile#merge-conflicts for advice."
                } else {
                    " Try deleting it and rerun the build."
                };
                format!(
                    "Failed to read and parse the MODULE.bazel.lock file with error: {error_message}.{suffix}"
                )
                .into()
            };
            Err(HostVisibleLockfileError::BadLockfile { message })
        }
        Err(error) => Err(HostVisibleLockfileError::UncaughtParse { error }),
    }
}

pub(crate) fn bad_visible_lockfile_message(error: impl std::fmt::Display) -> String {
    format!(
        "Failed to read and parse the MODULE.bazel.lock file with error: {error}. Try deleting it and rerun the build."
    )
}

fn unsupported_lockfile_version_message() -> String {
    "The version of MODULE.bazel.lock is not supported by this version of Bazel. Please run `bazel mod deps --lockfile_mode=update` to update your lockfile."
        .to_owned()
}

const fn is_caught_parse_surface(surface: LockfileParseErrorSurface) -> bool {
    matches!(
        surface,
        LockfileParseErrorSurface::CaughtJsonSyntax
            | LockfileParseErrorSurface::CaughtNullPointer
            | LockfileParseErrorSurface::CaughtIllegalArgument
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockfileReadInputs {
    pub mode: LockfileMode,
    pub visible: VisibleLockfileInput,
    pub hidden: HiddenLockfileInput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockfileReadSnapshot {
    pub visible: VisibleLockfileRead,
    pub hidden: Option<BazelLockfile>,
}

impl LockfileReadInputs {
    pub fn read(&self) -> Result<LockfileReadSnapshot, String> {
        if matches!(self.mode, LockfileMode::Off) {
            return Ok(LockfileReadSnapshot {
                visible: VisibleLockfileRead::Ignored,
                hidden: None,
            });
        }
        Ok(LockfileReadSnapshot {
            visible: parse_visible_lockfile_for_mode(&self.mode, &self.visible)?,
            hidden: Some(
                self.hidden
                    .parse_fail_open()
                    .map_err(|error| error.to_string())?,
            ),
        })
    }
}

pub fn plan_visible_lockfile(
    mode: &LockfileMode,
    existing_content: Option<&str>,
    desired: &BazelLockfile,
) -> Result<VisibleLockfilePlan, String> {
    match mode {
        LockfileMode::Off => Ok(VisibleLockfilePlan::Ignore),
        LockfileMode::Update | LockfileMode::Refresh => {
            let existing = parse_visible_lockfile_content_for_mode(mode, existing_content)?;
            let VisibleLockfileRead::Parsed(existing) = existing else {
                unreachable!("active lockfile mode returned Ignored")
            };
            if existing.semantically_eq(desired) {
                return Ok(VisibleLockfilePlan::Keep);
            }
            let content = render_bazel_lockfile(desired).map_err(|error| error.to_string())?;
            Ok(VisibleLockfilePlan::Write { content })
        }
        LockfileMode::Error => plan_error_mode_visible_lockfile(existing_content, desired),
    }
}

pub fn apply_visible_lockfile_plan(
    lockfile_path: impl AsRef<Path>,
    plan: &VisibleLockfilePlan,
) -> Result<VisibleLockfileApply, String> {
    match plan {
        VisibleLockfilePlan::Ignore => Ok(VisibleLockfileApply::Ignored),
        VisibleLockfilePlan::Keep => Ok(VisibleLockfileApply::Kept),
        VisibleLockfilePlan::Error { message } => Err(message.clone()),
        VisibleLockfilePlan::Write { content } => {
            write_visible_lockfile_atomically(lockfile_path.as_ref(), content)
        }
    }
}

fn write_visible_lockfile_atomically(
    lockfile_path: &Path,
    content: &str,
) -> Result<VisibleLockfileApply, String> {
    let parent = lockfile_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| {
            format!(
                "Unable to write MODULE.bazel.lock at {}: path has no parent directory",
                lockfile_path.display()
            )
        })?;
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|err| {
        format!(
            "Unable to create temporary MODULE.bazel.lock beside {}: {err}",
            lockfile_path.display()
        )
    })?;
    temp.write_all(content.as_bytes()).map_err(|err| {
        format!(
            "Unable to write temporary MODULE.bazel.lock for {}: {err}",
            lockfile_path.display()
        )
    })?;
    temp.as_file_mut().sync_all().map_err(|err| {
        format!(
            "Unable to flush temporary MODULE.bazel.lock for {}: {err}",
            lockfile_path.display()
        )
    })?;
    temp.persist(lockfile_path).map_err(|err| {
        format!(
            "Unable to publish MODULE.bazel.lock at {}: {}",
            lockfile_path.display(),
            err.error
        )
    })?;
    Ok(VisibleLockfileApply::Written {
        bytes: content.len(),
    })
}

fn plan_error_mode_visible_lockfile(
    existing_content: Option<&str>,
    desired: &BazelLockfile,
) -> Result<VisibleLockfilePlan, String> {
    let Some(existing_content) = existing_content else {
        return Ok(VisibleLockfilePlan::Error {
            message: "MODULE.bazel.lock is missing and --lockfile_mode=error does not permit updating it. Please run `bazel mod deps --lockfile_mode=update` to update your lockfile."
                .to_owned(),
        });
    };
    let existing =
        match parse_visible_lockfile_content_for_mode(&LockfileMode::Error, Some(existing_content))
        {
            Ok(VisibleLockfileRead::Parsed(lockfile)) => lockfile,
            Ok(VisibleLockfileRead::Ignored) => {
                unreachable!("error mode cannot produce an ignored lockfile")
            }
            Err(message) => return Ok(VisibleLockfilePlan::Error { message }),
        };
    if existing.semantically_eq(desired) {
        Ok(VisibleLockfilePlan::Keep)
    } else {
        Ok(VisibleLockfilePlan::Error {
            message: "MODULE.bazel.lock is no longer up-to-date. Please run `bazel mod deps --lockfile_mode=update` to update your lockfile."
                .to_owned(),
        })
    }
}
