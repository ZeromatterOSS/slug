//! Credited regular-file capture; cumulative counters bound retained byte history.
use std::io;
use std::path::Path;
use std::sync::Arc;

use slug_workspace_v2::PathNodeKind;

use super::SourceBufferReservation;
use super::SourceBufferSite;
use super::SourceCounter;
use super::SourceServiceError;
use super::SourceServiceScope;

pub(in crate::runtime) const HOST_FILE_OBJECT_BYTES: usize = 1 << 20;
#[cfg(all(test, target_os = "linux"))]
pub(in crate::runtime) const KNOWN_SHAS_OBJECT_BYTES: usize = 4_749_878;
const CHUNK_BYTES: usize = 64 << 10;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum RegularFileStage {
    Pin,
    Stat,
    Reopen,
    Read,
    Identity,
}

#[derive(Debug)]
pub(in crate::runtime) enum RegularFileError {
    Scope(SourceServiceError),
    Io {
        stage: RegularFileStage,
        error: io::Error,
        message: &'static str,
    },
    NonRegular {
        kind: PathNodeKind,
    },
    Changed {
        before_read: bool,
    },
}
impl From<SourceServiceError> for RegularFileError {
    fn from(error: SourceServiceError) -> Self {
        Self::Scope(error)
    }
}

#[derive(Clone, Copy)]
pub(super) enum ReadPolicy {
    Registry,
    Host,
    #[cfg(all(test, target_os = "linux"))]
    KnownShas,
}
impl ReadPolicy {
    fn is_host(self) -> bool {
        match self {
            Self::Registry => false,
            Self::Host => true,
            #[cfg(all(test, target_os = "linux"))]
            Self::KnownShas => true,
        }
    }
    fn maximum(self, scope: &SourceServiceScope) -> usize {
        match self {
            Self::Registry => scope.limits().registry_object_bytes as usize,
            Self::Host => HOST_FILE_OBJECT_BYTES,
            #[cfg(all(test, target_os = "linux"))]
            Self::KnownShas => KNOWN_SHAS_OBJECT_BYTES,
        }
    }
    fn counter(self) -> SourceCounter {
        if self.is_host() {
            SourceCounter::DecodedBytes
        } else {
            SourceCounter::RegistryBytes
        }
    }
    fn capture_site(self) -> SourceBufferSite {
        if self.is_host() {
            SourceBufferSite::HostCapture
        } else {
            SourceBufferSite::RegistryCapture
        }
    }
    fn arc_site(self) -> SourceBufferSite {
        if self.is_host() {
            SourceBufferSite::HostArc
        } else {
            SourceBufferSite::RegistryArc
        }
    }
    fn read_site(self) -> SourceBufferSite {
        if self.is_host() {
            SourceBufferSite::HostFileRead
        } else {
            SourceBufferSite::RegistryFileRead
        }
    }
}

// Bytes precede their credit on Drop. Returned Arc bytes do not carry a bank
// guard; cumulative RegistryBytes/DecodedBytes account retained history.
pub(super) struct Capture {
    bytes: Vec<u8>,
    _credit: SourceBufferReservation,
    policy: ReadPolicy,
}
impl Capture {
    pub(super) fn new(
        scope: &Arc<SourceServiceScope>,
        policy: ReadPolicy,
    ) -> Result<Self, SourceServiceError> {
        Self::with_capacity(scope, policy, policy.maximum(scope))
    }
    fn with_capacity(
        scope: &Arc<SourceServiceScope>,
        policy: ReadPolicy,
        maximum: usize,
    ) -> Result<Self, SourceServiceError> {
        let credit = scope.reserve_buffer_at(
            maximum + std::mem::size_of::<Vec<u8>>(),
            policy.capture_site(),
        )?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(maximum)
            .map_err(|_| scope.poison(SourceServiceError::Allocation))?;
        if bytes.capacity() > maximum {
            return Err(scope.poison(SourceServiceError::Allocation));
        }
        if policy.is_host() {
            scope.check()?;
        }
        Ok(Self {
            bytes,
            _credit: credit,
            policy,
        })
    }
    pub(super) fn append(
        &mut self,
        scope: &SourceServiceScope,
        bytes: &[u8],
    ) -> Result<(), SourceServiceError> {
        let counter = self.policy.counter();
        let len = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or_else(|| scope.poison(SourceServiceError::Overflow(counter)))?;
        if len > self.policy.maximum(scope) {
            return Err(scope.poison(SourceServiceError::Limit(counter)));
        }
        if len > self.bytes.capacity() {
            return Err(scope.poison(SourceServiceError::Allocation));
        }
        scope.charge(counter, bytes.len() as u64)?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    pub(super) fn finish(
        self,
        scope: &Arc<SourceServiceScope>,
    ) -> Result<Arc<[u8]>, SourceServiceError> {
        self.finish_with(scope, |_| {})
    }
    // One conversion path; production supplies an empty, zero-state closure.
    pub(super) fn finish_with(
        self,
        scope: &Arc<SourceServiceScope>,
        after_created: impl FnOnce(&Arc<[u8]>),
    ) -> Result<Arc<[u8]>, SourceServiceError> {
        scope.check()?;
        let _conversion = scope.reserve_buffer_at(self.bytes.len() + 64, self.policy.arc_site())?;
        let result = Arc::<[u8]>::from(self.bytes.as_slice());
        #[cfg(all(test, target_os = "linux"))]
        scope.copy_created_for_test(self.bytes.len(), self.bytes.capacity());
        after_created(&result);
        scope.check()?;
        #[cfg(all(test, target_os = "linux"))]
        scope.copy_returned_for_test();
        drop(self);
        Ok(result)
    }
}

pub(in crate::runtime) fn read_host_file(
    scope: &Arc<SourceServiceScope>,
    path: &Path,
) -> Result<Arc<[u8]>, RegularFileError> {
    #[cfg(all(test, target_os = "linux"))]
    {
        return read_host_with_charge_origin(scope, path, ReadPolicy::Host, || {});
    }
    #[cfg(not(all(test, target_os = "linux")))]
    {
        scope.charge(SourceCounter::SourceAttempts, 1)?;
        read_regular(scope, path, ReadPolicy::Host)
    }
}

#[cfg(all(test, target_os = "linux"))]
pub(in crate::runtime) fn read_host_file_with_charge_origin(
    scope: &Arc<SourceServiceScope>,
    path: &Path,
    on_first_refusal: impl FnOnce(),
) -> Result<Arc<[u8]>, RegularFileError> {
    read_host_with_charge_origin(scope, path, ReadPolicy::Host, on_first_refusal)
}

#[cfg(all(test, target_os = "linux"))]
pub(in crate::runtime) fn read_host_file_with_known_shas_allowance(
    scope: &Arc<SourceServiceScope>,
    demand: &slug_workspace_v2::PathObservationDemand,
    on_first_refusal: impl FnOnce(),
) -> Result<Arc<[u8]>, RegularFileError> {
    let policy = if scope.known_shas_object_bytes_for_test(demand).is_some() {
        ReadPolicy::KnownShas
    } else {
        ReadPolicy::Host
    };
    read_host_with_charge_origin(scope, demand.path().as_path(), policy, on_first_refusal)
}

#[cfg(all(test, target_os = "linux"))]
fn read_host_with_charge_origin(
    scope: &Arc<SourceServiceScope>,
    path: &Path,
    policy: ReadPolicy,
    on_first_refusal: impl FnOnce(),
) -> Result<Arc<[u8]>, RegularFileError> {
    read_host_with_census(scope, path, policy, on_first_refusal, |record| {
        scope.report_source_charge_for_test(
            record,
            super::SourceChargePhase::Unassociated,
            super::SourceChargeIdentity::HostPath(path),
        );
    })
}

#[cfg(all(test, target_os = "linux"))]
pub(in crate::runtime) fn read_host_file_with_known_shas_allowance_and_census(
    scope: &Arc<SourceServiceScope>,
    demand: &slug_workspace_v2::PathObservationDemand,
    on_first_refusal: impl FnOnce(),
    on_charge: impl FnMut(super::SourceChargeRecord),
) -> Result<Arc<[u8]>, RegularFileError> {
    let policy = if scope.known_shas_object_bytes_for_test(demand).is_some() {
        ReadPolicy::KnownShas
    } else {
        ReadPolicy::Host
    };
    read_host_with_census(
        scope,
        demand.path().as_path(),
        policy,
        on_first_refusal,
        on_charge,
    )
}

#[cfg(all(test, target_os = "linux"))]
fn read_host_with_census(
    scope: &Arc<SourceServiceScope>,
    path: &Path,
    policy: ReadPolicy,
    on_first_refusal: impl FnOnce(),
    mut on_charge: impl FnMut(super::SourceChargeRecord),
) -> Result<Arc<[u8]>, RegularFileError> {
    let (admission, created, record) =
        scope.charge_source_with_origin_record_for_test(1, super::SourceChargeDomain::Host);
    if let Some(record) = record {
        on_charge(record);
    }
    if let Err(error) = admission {
        if created {
            on_first_refusal();
        }
        return Err(RegularFileError::Scope(error));
    }
    read_regular(scope, path, policy)
}

#[cfg(target_os = "linux")]
pub(super) fn read_regular(
    scope: &Arc<SourceServiceScope>,
    path: &Path,
    policy: ReadPolicy,
) -> Result<Arc<[u8]>, RegularFileError> {
    use std::io::Read;
    read_regular_with(
        scope,
        path,
        policy,
        |_, _| {},
        |file, bytes| file.read(bytes),
    )
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy, Eq, PartialEq)]
enum ReadPoint {
    Pinned,
    BeforeRead,
    AfterRead,
    BeforeConversion,
}

#[cfg(target_os = "linux")]
fn checked_io<T>(
    scope: &SourceServiceScope,
    policy: ReadPolicy,
    stage: RegularFileStage,
    message: &'static str,
    operation: impl FnOnce() -> io::Result<T>,
) -> Result<T, RegularFileError> {
    if policy.is_host() {
        scope.check()?;
    }
    let result = operation();
    if policy.is_host() {
        scope.check()?;
    }
    result.map_err(|error| RegularFileError::Io {
        stage,
        error,
        message,
    })
}

#[cfg(target_os = "linux")]
fn read_regular_with(
    scope: &Arc<SourceServiceScope>,
    path: &Path,
    policy: ReadPolicy,
    mut hook: impl FnMut(ReadPoint, &std::fs::File),
    mut read: impl FnMut(&mut std::fs::File, &mut [u8]) -> io::Result<usize>,
) -> Result<Arc<[u8]>, RegularFileError> {
    use std::fs::OpenOptions;
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::fs::OpenOptionsExt;
    scope.check()?;
    let pin = checked_io(
        scope,
        policy,
        RegularFileStage::Pin,
        "scoped registry inode pin failed",
        || {
            OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_PATH | libc::O_CLOEXEC)
                .open(path)
        },
    )?;
    let before = checked_io(
        scope,
        policy,
        RegularFileStage::Stat,
        "scoped registry pinned stat failed",
        || pin.metadata(),
    )?;
    if !before.is_file() {
        return Err(RegularFileError::NonRegular {
            kind: if before.is_dir() {
                PathNodeKind::Directory
            } else {
                PathNodeKind::SpecialFile
            },
        });
    }
    if before.len() > policy.maximum(scope) as u64 {
        return Err(scope
            .poison(SourceServiceError::Limit(policy.counter()))
            .into());
    }
    hook(ReadPoint::Pinned, &pin);
    scope.check()?;
    let mut input = checked_io(
        scope,
        policy,
        RegularFileStage::Reopen,
        "scoped registry data reopen failed",
        || {
            OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
                .open(format!("/proc/self/fd/{}", pin.as_raw_fd()))
        },
    )?;
    let identity = |m: &std::fs::Metadata| {
        (
            m.dev(),
            m.ino(),
            m.mode(),
            m.len(),
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec(),
        )
    };
    let reopened = checked_io(
        scope,
        policy,
        RegularFileStage::Stat,
        "scoped registry data stat failed",
        || input.metadata(),
    )?;
    if identity(&reopened) != identity(&before) {
        return Err(RegularFileError::Changed { before_read: true });
    }
    let capacity = if policy.is_host() {
        before.len() as usize
    } else {
        policy.maximum(scope)
    };
    let mut buffer = Capture::with_capacity(scope, policy, capacity)?;
    let _read_credit = scope.reserve_buffer_at(CHUNK_BYTES, policy.read_site())?;
    let mut chunk = [0u8; CHUNK_BYTES];
    loop {
        hook(ReadPoint::BeforeRead, &input);
        scope.check()?;
        // Registry keeps its original full chunk, including overflow rejection
        // before any charge. Host reads its pinned length plus one EOF probe.
        let remaining = capacity - buffer.bytes.len();
        let requested = if !policy.is_host() {
            CHUNK_BYTES
        } else if remaining == 0 {
            1
        } else {
            remaining.min(CHUNK_BYTES)
        };
        let result = read(&mut input, &mut chunk[..requested]);
        if policy.is_host() || result.is_ok() {
            scope.check()?;
        }
        let count = match result {
            Err(e) if policy.is_host() && e.kind() == io::ErrorKind::Interrupted => {
                continue;
            }
            other => other.map_err(|error| RegularFileError::Io {
                stage: RegularFileStage::Read,
                error,
                message: "scoped registry local read failed",
            })?,
        };
        hook(ReadPoint::AfterRead, &input);
        if policy.is_host() {
            scope.check()?;
        }
        if count == 0 {
            break;
        }
        if policy.is_host() && count > remaining {
            return Err(RegularFileError::Changed { before_read: false });
        }
        buffer.append(scope, &chunk[..count])?;
    }
    let after = checked_io(
        scope,
        policy,
        RegularFileStage::Stat,
        "scoped registry final stat failed",
        || input.metadata(),
    )?;
    if identity(&after) != identity(&before) || buffer.bytes.len() as u64 != before.len() {
        return Err(RegularFileError::Changed { before_read: false });
    }
    hook(ReadPoint::BeforeConversion, &input);
    if policy.is_host() {
        let named = match checked_io(
            scope,
            policy,
            RegularFileStage::Identity,
            "scoped Host current name stat failed",
            || std::fs::symlink_metadata(path),
        ) {
            Err(RegularFileError::Io { error, .. })
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
                ) =>
            {
                return Err(RegularFileError::Changed { before_read: false });
            }
            other => other?,
        };
        if identity(&named) != identity(&before) {
            return Err(RegularFileError::Changed { before_read: false });
        }
    }
    let result = buffer.finish(scope)?;
    if policy.is_host() {
        scope.check()?;
    }
    Ok(result)
}

#[cfg(not(target_os = "linux"))]
pub(super) fn read_regular(
    scope: &Arc<SourceServiceScope>,
    _path: &Path,
    _policy: ReadPolicy,
) -> Result<Arc<[u8]>, RegularFileError> {
    Err(scope
        .poison(SourceServiceError::UnsupportedSourceOwner)
        .into())
}

#[cfg(all(test, target_os = "linux"))]
#[path = "regular_file_tests.rs"]
mod tests;
