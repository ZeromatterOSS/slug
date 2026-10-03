//! Redacted facts travel with the actual selected failure, never a side trace.
use std::fmt;
use std::io;

use slug_bzlmod_v2::RegistryRefusalCause;
use slug_bzlmod_v2::RegistryRefusalDetail;
use slug_bzlmod_v2::RegistryRefusalIoKind;
use slug_bzlmod_v2::RegistryRefusalProtocolFailure;
use slug_bzlmod_v2::RegistryRefusalScopeCounter;
use slug_bzlmod_v2::RegistryRefusalScopeFailure;
use slug_bzlmod_v2::RegistryRefusalSite;
use slug_bzlmod_v2::RegistryTransportError;

use super::SourceCounter;
use super::SourceServiceError;

pub(in crate::runtime) struct SourceTransportFailure {
    message: String,
    cause: RegistryRefusalCause,
}
impl fmt::Debug for SourceTransportFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SourceTransportFailure")
            .field("cause", &self.cause)
            .finish()
    }
}
impl SourceTransportFailure {
    pub(super) fn new(message: impl Into<String>, cause: RegistryRefusalCause) -> Self {
        Self {
            message: message.into(),
            cause,
        }
    }
    pub(super) fn io(site: RegistryRefusalSite, error: &io::Error, message: &'static str) -> Self {
        Self::new(
            message,
            RegistryRefusalCause {
                site,
                detail: io_detail(error),
            },
        )
    }
    pub(super) fn scope(error: SourceServiceError) -> Self {
        Self::new(
            error.to_string(),
            RegistryRefusalCause {
                site: RegistryRefusalSite::Scope,
                detail: RegistryRefusalDetail::Scope(scope_failure(error)),
            },
        )
    }
    pub(super) fn opaque(site: RegistryRefusalSite, message: impl Into<String>) -> Self {
        Self::new(
            message,
            RegistryRefusalCause {
                site,
                detail: RegistryRefusalDetail::Opaque,
            },
        )
    }
    pub(in crate::runtime) fn into_message(self) -> String {
        self.message
    }
    pub(super) fn into_registry(self) -> RegistryTransportError {
        RegistryTransportError {
            message: self.message.into(),
            cause: Some(self.cause),
        }
    }
    pub(super) fn cause(&self) -> RegistryRefusalCause {
        self.cause
    }
}

pub(super) fn io_detail(error: &io::Error) -> RegistryRefusalDetail {
    use RegistryRefusalIoKind as R;
    use io::ErrorKind as K;
    let kind = match error.kind() {
        K::NotFound => R::NotFound,
        K::PermissionDenied => R::PermissionDenied,
        K::ConnectionRefused => R::ConnectionRefused,
        K::ConnectionReset => R::ConnectionReset,
        K::ConnectionAborted => R::ConnectionAborted,
        K::NotConnected => R::NotConnected,
        K::AddrInUse => R::AddrInUse,
        K::AddrNotAvailable => R::AddrNotAvailable,
        K::BrokenPipe => R::BrokenPipe,
        K::AlreadyExists => R::AlreadyExists,
        K::WouldBlock => R::WouldBlock,
        K::InvalidInput => R::InvalidInput,
        K::InvalidData => R::InvalidData,
        K::TimedOut => R::TimedOut,
        K::WriteZero => R::WriteZero,
        K::Interrupted => R::Interrupted,
        K::UnexpectedEof => R::UnexpectedEof,
        K::OutOfMemory => R::OutOfMemory,
        K::Unsupported => R::Unsupported,
        K::Other => R::Other,
        _ => R::Unknown,
    };
    RegistryRefusalDetail::Io {
        kind,
        errno: error.raw_os_error(),
    }
}

pub(super) fn hyper_detail(error: &hyper::Error) -> RegistryRefusalDetail {
    // Inspect only bounded borrowed sources; never render arbitrary errors.
    let mut source = std::error::Error::source(error);
    for _ in 0..8 {
        let Some(current) = source else { break };
        if let Some(io) = current.downcast_ref::<io::Error>() {
            return io_detail(io);
        }
        source = current.source();
    }
    use RegistryRefusalProtocolFailure as P;
    RegistryRefusalDetail::Protocol(if error.is_parse() {
        P::Parse
    } else if error.is_incomplete_message() {
        P::Incomplete
    } else if error.is_closed() {
        P::Closed
    } else if error.is_canceled() {
        P::Cancelled
    } else {
        P::Other
    })
}

fn counter(counter: SourceCounter) -> RegistryRefusalScopeCounter {
    use RegistryRefusalScopeCounter as R;
    match counter {
        SourceCounter::RegistryAttempts => R::RegistryAttempts,
        SourceCounter::RegistryBytes => R::RegistryBytes,
        SourceCounter::SourceAttempts => R::SourceAttempts,
        SourceCounter::TransferBytes => R::TransferBytes,
        SourceCounter::DecodedBytes => R::DecodedBytes,
        SourceCounter::WrittenBytes => R::WrittenBytes,
    }
}
fn scope_failure(error: SourceServiceError) -> RegistryRefusalScopeFailure {
    use RegistryRefusalScopeFailure as R;
    match error {
        SourceServiceError::InvalidLimits => R::InvalidLimits,
        SourceServiceError::ForeignScope => R::ForeignScope,
        SourceServiceError::AcquisitionClosed => R::AcquisitionClosed,
        SourceServiceError::Cancelled => R::Cancelled,
        SourceServiceError::WorkDeadline => R::WorkDeadline,
        SourceServiceError::CleanupDeadline => R::CleanupDeadline,
        SourceServiceError::Limit(c) => R::Limit(counter(c)),
        SourceServiceError::Overflow(c) => R::Overflow(counter(c)),
        SourceServiceError::BufferLimit => R::BufferLimit,
        SourceServiceError::Allocation => R::Allocation,
        SourceServiceError::UnsupportedSourceOwner => R::UnsupportedSourceOwner,
    }
}
#[cfg(test)]
#[path = "failure/tests.rs"]
mod tests;
