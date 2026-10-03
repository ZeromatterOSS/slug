//! Preserve the established diagnostic bytes while retaining typed facts in equality.

use std::fmt;

use super::*;

impl fmt::Debug for RegistryTransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegistryTransportError")
            .field("message", &self.message)
            .finish()
    }
}

impl fmt::Debug for RegistryFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFileUrl(value) => f.debug_tuple("InvalidFileUrl").field(value).finish(),
            Self::UnsupportedUrl(value) => f.debug_tuple("UnsupportedUrl").field(value).finish(),
            Self::MissingRegistryUrls(value) => {
                f.debug_tuple("MissingRegistryUrls").field(value).finish()
            }
            Self::MissingLockfileMode(value) => {
                f.debug_tuple("MissingLockfileMode").field(value).finish()
            }
            Self::MissingRequestGeneration(value) => f
                .debug_tuple("MissingRequestGeneration")
                .field(value)
                .finish(),
            Self::RootModuleFiles(value) => f.debug_tuple("RootModuleFiles").field(value).finish(),
            Self::MissingIoCapability => f.write_str("MissingIoCapability"),
            Self::MissingChecksumInError { url, .. } => f
                .debug_struct("MissingChecksumInError")
                .field("url", url)
                .finish(),
            Self::Transport { url, message, .. } => f
                .debug_struct("Transport")
                .field("url", url)
                .field("message", message)
                .finish(),
            Self::ChecksumMismatch {
                url,
                expected,
                actual,
                ..
            } => f
                .debug_struct("ChecksumMismatch")
                .field("url", url)
                .field("expected", expected)
                .field("actual", actual)
                .finish(),
            Self::LocalRead { path, message, .. } => f
                .debug_struct("LocalRead")
                .field("path", path)
                .field("message", message)
                .finish(),
            Self::InvalidLockfileExpectation { url, message, .. } => f
                .debug_struct("InvalidLockfileExpectation")
                .field("url", url)
                .field("message", message)
                .finish(),
        }
    }
}

impl fmt::Debug for RegistryRemoteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRemoteHashesMode { url, mode, .. } => f
                .debug_struct("InvalidRemoteHashesMode")
                .field("url", url)
                .field("mode", mode)
                .finish(),
            Self::MissingRequestGeneration {
                workspace,
                url,
                message,
                ..
            } => f
                .debug_struct("MissingRequestGeneration")
                .field("workspace", workspace)
                .field("url", url)
                .field("message", message)
                .finish(),
            Self::MissingIoCapability { url, .. } => f
                .debug_struct("MissingIoCapability")
                .field("url", url)
                .finish(),
            Self::MissingChecksumInError { url, .. } => f
                .debug_struct("MissingChecksumInError")
                .field("url", url)
                .finish(),
            Self::InvalidLockfileExpectation { url, message, .. } => f
                .debug_struct("InvalidLockfileExpectation")
                .field("url", url)
                .field("message", message)
                .finish(),
            Self::Transport { url, message, .. } => f
                .debug_struct("Transport")
                .field("url", url)
                .field("message", message)
                .finish(),
            Self::ChecksumMismatch {
                url,
                expected,
                actual,
                ..
            } => f
                .debug_struct("ChecksumMismatch")
                .field("url", url)
                .field("expected", expected)
                .field("actual", actual)
                .finish(),
        }
    }
}

impl fmt::Debug for RegistryLocalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRequestGeneration {
                workspace,
                url,
                message,
                ..
            } => f
                .debug_struct("MissingRequestGeneration")
                .field("workspace", workspace)
                .field("url", url)
                .field("message", message)
                .finish(),
            Self::MissingIoCapability { url, .. } => f
                .debug_struct("MissingIoCapability")
                .field("url", url)
                .finish(),
            Self::Read {
                url, path, message, ..
            } => f
                .debug_struct("Read")
                .field("url", url)
                .field("path", path)
                .field("message", message)
                .finish(),
        }
    }
}
