//! Bounded producer-owned refusal facts; these scalars confer no source authority.
use allocative::Allocative;

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Allocative)]
pub struct RegistryRefusalCause {
    pub site: RegistryRefusalSite,
    pub detail: RegistryRefusalDetail,
}
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Allocative)]
pub enum RegistryRefusalSite {
    AddressValidation,
    Dns,
    ConnectExhausted,
    TlsRootRejected,
    TlsRootsUnavailable,
    TlsProtocol,
    TlsName,
    TlsHandshake,
    HttpHandshake,
    HttpRequest,
    HttpHeaders,
    HttpBody,
    HttpConnection,
    HttpStatus,
    LocalPin,
    LocalStat,
    LocalReopen,
    LocalRead,
    LocalIdentity,
    Scope,
    InternalState,
}
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Allocative)]
pub enum RegistryRefusalDetail {
    Io {
        kind: RegistryRefusalIoKind,
        errno: Option<i32>,
    },
    Status(u16),
    Protocol(RegistryRefusalProtocolFailure),
    Scope(RegistryRefusalScopeFailure),
    Opaque,
}
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Allocative)]
pub enum RegistryRefusalIoKind {
    NotFound,
    PermissionDenied,
    ConnectionRefused,
    ConnectionReset,
    ConnectionAborted,
    NotConnected,
    AddrInUse,
    AddrNotAvailable,
    BrokenPipe,
    AlreadyExists,
    WouldBlock,
    InvalidInput,
    InvalidData,
    TimedOut,
    WriteZero,
    Interrupted,
    UnexpectedEof,
    OutOfMemory,
    Unsupported,
    Other,
    Unknown,
}
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Allocative)]
pub enum RegistryRefusalProtocolFailure {
    Parse,
    Incomplete,
    Closed,
    Cancelled,
    Other,
}
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Allocative)]
pub enum RegistryRefusalScopeFailure {
    InvalidLimits,
    ForeignScope,
    AcquisitionClosed,
    Cancelled,
    WorkDeadline,
    CleanupDeadline,
    Limit(RegistryRefusalScopeCounter),
    Overflow(RegistryRefusalScopeCounter),
    BufferLimit,
    Allocation,
    UnsupportedSourceOwner,
}
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Allocative)]
pub enum RegistryRefusalScopeCounter {
    RegistryAttempts,
    RegistryBytes,
    SourceAttempts,
    TransferBytes,
    DecodedBytes,
    WrittenBytes,
}

impl RegistryRefusalSite {
    /// Stable redacted receipt code; zero is reserved for absent data.
    #[doc(hidden)]
    pub fn code(self) -> u16 {
        match self {
            Self::AddressValidation => 1,
            Self::Dns => 2,
            Self::ConnectExhausted => 3,
            Self::TlsRootRejected => 4,
            Self::TlsRootsUnavailable => 5,
            Self::TlsProtocol => 6,
            Self::TlsName => 7,
            Self::TlsHandshake => 8,
            Self::HttpHandshake => 9,
            Self::HttpRequest => 10,
            Self::HttpHeaders => 11,
            Self::HttpBody => 12,
            Self::HttpConnection => 13,
            Self::HttpStatus => 14,
            Self::LocalPin => 15,
            Self::LocalStat => 16,
            Self::LocalReopen => 17,
            Self::LocalRead => 18,
            Self::LocalIdentity => 19,
            Self::Scope => 20,
            Self::InternalState => 21,
        }
    }
}

impl RegistryRefusalIoKind {
    /// Stable redacted receipt code; zero is reserved for absent data.
    #[doc(hidden)]
    pub fn code(self) -> u16 {
        match self {
            Self::NotFound => 1,
            Self::PermissionDenied => 2,
            Self::ConnectionRefused => 3,
            Self::ConnectionReset => 4,
            Self::ConnectionAborted => 5,
            Self::NotConnected => 6,
            Self::AddrInUse => 7,
            Self::AddrNotAvailable => 8,
            Self::BrokenPipe => 9,
            Self::AlreadyExists => 10,
            Self::WouldBlock => 11,
            Self::InvalidInput => 12,
            Self::InvalidData => 13,
            Self::TimedOut => 14,
            Self::WriteZero => 15,
            Self::Interrupted => 16,
            Self::UnexpectedEof => 17,
            Self::OutOfMemory => 18,
            Self::Unsupported => 19,
            Self::Other => 20,
            Self::Unknown => 21,
        }
    }
}

impl RegistryRefusalProtocolFailure {
    /// Stable redacted receipt code; zero is reserved for absent data.
    #[doc(hidden)]
    pub fn code(self) -> u16 {
        match self {
            Self::Parse => 1,
            Self::Incomplete => 2,
            Self::Closed => 3,
            Self::Cancelled => 4,
            Self::Other => 5,
        }
    }
}

impl RegistryRefusalScopeCounter {
    /// Stable redacted receipt code; zero is reserved for absent data.
    #[doc(hidden)]
    pub fn code(self) -> u16 {
        match self {
            Self::RegistryAttempts => 1,
            Self::RegistryBytes => 2,
            Self::SourceAttempts => 3,
            Self::TransferBytes => 4,
            Self::DecodedBytes => 5,
            Self::WrittenBytes => 6,
        }
    }
}

impl RegistryRefusalScopeFailure {
    /// Stable redacted receipt code; zero is reserved for absent data.
    #[doc(hidden)]
    pub fn code(self) -> u16 {
        match self {
            Self::InvalidLimits => 1,
            Self::ForeignScope => 2,
            Self::AcquisitionClosed => 3,
            Self::Cancelled => 4,
            Self::WorkDeadline => 5,
            Self::CleanupDeadline => 6,
            Self::Limit(..) => 7,
            Self::Overflow(..) => 8,
            Self::BufferLimit => 9,
            Self::Allocation => 10,
            Self::UnsupportedSourceOwner => 11,
        }
    }
}

impl RegistryRefusalScopeFailure {
    #[doc(hidden)]
    pub fn counter(self) -> Option<RegistryRefusalScopeCounter> {
        match self {
            Self::Limit(counter) | Self::Overflow(counter) => Some(counter),
            _ => None,
        }
    }
}
