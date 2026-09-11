//! Borrowed causal projection for registration diagnostics, not semantic identity.

use std::fmt;
use std::path::Path;

use super::*;

#[cfg(test)]
#[path = "registration_diagnostic_tests.rs"]
mod tests;

impl HostRepositorySourceObservationError {
    /// Stream only bounded causal fields. The caller owns escaping and its budget.
    #[doc(hidden)]
    pub fn write_registration_diagnostic(&self, out: &mut dyn fmt::Write) -> fmt::Result {
        source_class(out, &self.input)?;
        out.write_str(": ")?;
        match &self.kind {
            HostRepositorySourceObservationErrorKind::BuiltinPath => {
                out.write_str("BuiltinPath path=")?;
                path(out, self.relative_path.as_path())
            }
            HostRepositorySourceObservationErrorKind::Builtin(error) => builtin(out, error),
            HostRepositorySourceObservationErrorKind::BuiltinCompute(message) => {
                out.write_str("BuiltinCompute path=")?;
                path(out, self.relative_path.as_path())?;
                write!(out, " message={message}")
            }
            HostRepositorySourceObservationErrorKind::Request(error) => request(out, error),
            HostRepositorySourceObservationErrorKind::RequestCompute(message) => {
                out.write_str("RequestCompute path=")?;
                path(out, self.relative_path.as_path())?;
                write!(out, " message={message}")
            }
        }
    }
}

fn source_class(
    out: &mut dyn fmt::Write,
    input: &HostRepositorySourceObservationInput,
) -> fmt::Result {
    out.write_str(match input {
        HostRepositorySourceObservationInput::Root(input) => match input.view().disposition() {
            HostRepositorySourceInputDispositionView::Builtin(_) => "RootBuiltin",
            HostRepositorySourceInputDispositionView::Request(_) => "RootRequest",
        },
        HostRepositorySourceObservationInput::Canonical(input) => {
            match input.view().disposition() {
                HostRepositorySourceInputDispositionView::Builtin(_) => "CanonicalBuiltin",
                HostRepositorySourceInputDispositionView::Request(_) => "CanonicalRequest",
            }
        }
    })
}

fn path(out: &mut dyn fmt::Write, value: &Path) -> fmt::Result {
    out.write_str("hex:")?;
    for byte in value.as_os_str().as_encoded_bytes() {
        write!(out, "{byte:02x}")?;
    }
    Ok(())
}

fn builtin(out: &mut dyn fmt::Write, error: &BuiltinBazelToolsSourceFileError) -> fmt::Result {
    use BuiltinBazelToolsSourceFileError as E;
    match error {
        E::InvalidPath { path } => write!(out, "Builtin.InvalidPath path={path}"),
        E::WrongKind { path, actual } => write!(
            out,
            "Builtin.WrongKind path={path} actual={}",
            builtin_source_kind(*actual)
        ),
        E::UnsupportedCatalog { path } => {
            write!(out, "Builtin.UnsupportedCatalog path={path}")
        }
        E::Integrity {
            path,
            expected_sha256,
            actual_sha256,
        } => write!(
            out,
            "Builtin.Integrity path={path} actual_sha256={actual_sha256} expected_sha256={expected_sha256}"
        ),
    }
}

fn builtin_source_kind(kind: crate::BuiltinBazelToolsSourceKind) -> &'static str {
    match kind {
        crate::BuiltinBazelToolsSourceKind::File => "File",
        crate::BuiltinBazelToolsSourceKind::Directory => "Directory",
    }
}

fn request(out: &mut dyn fmt::Write, error: &RepositorySourceFileError) -> fmt::Result {
    use RepositorySourceFileError as E;
    match error {
        E::InvalidRepoRelativePath { requested_path } => {
            out.write_str("Request.InvalidRepoRelativePath path=")?;
            path(out, requested_path)
        }
        E::MaterializationCompute {
            repo_relative_path,
            message,
        } => {
            out.write_str("Request.MaterializationCompute path=")?;
            path(out, repo_relative_path)?;
            write!(out, " message={message}")
        }
        E::Materialization {
            repo_relative_path,
            error,
        } => {
            out.write_str("Request.Materialization path=")?;
            path(out, repo_relative_path)?;
            let (kind, message) = materialization(error);
            write!(out, " kind={kind} message={message}")
        }
        E::InvalidMaterializedPath { repo_relative_path } => {
            out.write_str("Request.InvalidMaterializedPath path=")?;
            path(out, repo_relative_path)
        }
        E::Observation {
            repo_relative_path,
            operation,
            error,
        } => {
            out.write_str("Request.Observation path=")?;
            path(out, repo_relative_path)?;
            write!(out, " operation={} error=", operation_name(*operation))?;
            observation_error(out, error)
        }
        E::InconsistentState {
            repo_relative_path,
            operation,
            before,
            after,
        } => {
            out.write_str("Request.InconsistentState path=")?;
            path(out, repo_relative_path)?;
            write!(out, " operation={} before=", operation_name(*operation))?;
            state(out, *before)?;
            out.write_str(" after=")?;
            state(out, *after)
        }
        E::WrongKind {
            repo_relative_path,
            actual,
        } => {
            out.write_str("Request.WrongKind path=")?;
            path(out, repo_relative_path)?;
            write!(out, " actual={}", node_kind(*actual))
        }
        E::Cycle { repo_relative_path } => {
            out.write_str("Request.Cycle path=")?;
            path(out, repo_relative_path)
        }
        E::InfiniteExpansion { repo_relative_path } => {
            out.write_str("Request.InfiniteExpansion path=")?;
            path(out, repo_relative_path)
        }
        E::ResolutionCompute {
            repo_relative_path,
            message,
        } => {
            out.write_str("Request.ResolutionCompute path=")?;
            path(out, repo_relative_path)?;
            write!(out, " message={message}")
        }
        E::FileCompute {
            repo_relative_path,
            message,
        } => {
            out.write_str("Request.FileCompute path=")?;
            path(out, repo_relative_path)?;
            write!(out, " message={message}")
        }
    }
}

fn materialization(error: &RepositoryMaterializationError) -> (&'static str, &str) {
    use RepositoryMaterializationError as E;
    match error {
        E::RootModuleFiles(message) => ("RootModuleFiles", message.as_str()),
        E::MissingOverride(message) => ("MissingOverride", message.as_str()),
        E::UnsupportedOverride(message) => ("UnsupportedOverride", message.as_str()),
        E::InvalidCanonicalRepository(message) => ("InvalidCanonicalRepository", message.as_str()),
        E::InvalidWorkspace(message) => ("InvalidWorkspace", message.as_str()),
        E::ResultCompute(message) => ("ResultCompute", message.as_str()),
        E::MissingGeneration(message) => ("MissingGeneration", message.as_str()),
        E::Spec(message) => ("Spec", message.as_str()),
        E::Transport(message) => ("Transport", message.as_str()),
        E::Materialization(message) => ("Materialization", message.as_str()),
    }
}

fn operation_name(operation: PathObservationOperation) -> &'static str {
    match operation {
        PathObservationOperation::Lstat => "Lstat",
        PathObservationOperation::ReadLink => "ReadLink",
        PathObservationOperation::FileBytes => "FileBytes",
        PathObservationOperation::DirectoryEntries => "DirectoryEntries",
        PathObservationOperation::WindowsLongPath => "WindowsLongPath",
        PathObservationOperation::WindowsOptionPathLongName => "WindowsOptionPathLongName",
    }
}

fn node_kind(kind: PathNodeKind) -> &'static str {
    match kind {
        PathNodeKind::RegularFile => "RegularFile",
        PathNodeKind::Directory => "Directory",
        PathNodeKind::Symlink => "Symlink",
        PathNodeKind::SpecialFile => "SpecialFile",
    }
}

fn state(out: &mut dyn fmt::Write, value: Option<PathLstat>) -> fmt::Result {
    match value {
        Some(value) => out.write_str(node_kind(value.kind())),
        None => out.write_str("none"),
    }
}

fn observation_error(out: &mut dyn fmt::Write, error: &PathObservationError) -> fmt::Result {
    match error {
        PathObservationError::Io { kind, raw_os_error } => {
            write!(out, "Io kind={} raw_os_error=", io_kind(*kind))?;
            match raw_os_error {
                Some(code) => write!(out, "{code}"),
                None => out.write_str("none"),
            }
        }
        PathObservationError::NotALink => out.write_str("NotALink"),
        PathObservationError::WrongKind { expected, actual } => write!(
            out,
            "WrongKind expected={} actual={}",
            node_kind(*expected),
            node_kind(*actual)
        ),
        PathObservationError::InconsistentState { before, after } => {
            out.write_str("InconsistentState before=")?;
            state(out, *before)?;
            out.write_str(" after=")?;
            state(out, *after)
        }
    }
}

fn io_kind(kind: slug_workspace_v2::PathIoErrorKind) -> &'static str {
    use slug_workspace_v2::PathIoErrorKind as E;
    match kind {
        E::NotFound => "NotFound",
        E::PermissionDenied => "PermissionDenied",
        E::ConnectionRefused => "ConnectionRefused",
        E::ConnectionReset => "ConnectionReset",
        E::HostUnreachable => "HostUnreachable",
        E::NetworkUnreachable => "NetworkUnreachable",
        E::ConnectionAborted => "ConnectionAborted",
        E::NotConnected => "NotConnected",
        E::AddrInUse => "AddrInUse",
        E::AddrNotAvailable => "AddrNotAvailable",
        E::NetworkDown => "NetworkDown",
        E::BrokenPipe => "BrokenPipe",
        E::AlreadyExists => "AlreadyExists",
        E::WouldBlock => "WouldBlock",
        E::NotADirectory => "NotADirectory",
        E::IsADirectory => "IsADirectory",
        E::DirectoryNotEmpty => "DirectoryNotEmpty",
        E::ReadOnlyFilesystem => "ReadOnlyFilesystem",
        E::StaleNetworkFileHandle => "StaleNetworkFileHandle",
        E::InvalidInput => "InvalidInput",
        E::InvalidData => "InvalidData",
        E::TimedOut => "TimedOut",
        E::WriteZero => "WriteZero",
        E::StorageFull => "StorageFull",
        E::NotSeekable => "NotSeekable",
        E::QuotaExceeded => "QuotaExceeded",
        E::FileTooLarge => "FileTooLarge",
        E::ResourceBusy => "ResourceBusy",
        E::ExecutableFileBusy => "ExecutableFileBusy",
        E::Deadlock => "Deadlock",
        E::CrossesDevices => "CrossesDevices",
        E::TooManyLinks => "TooManyLinks",
        E::InvalidFilename => "InvalidFilename",
        E::ArgumentListTooLong => "ArgumentListTooLong",
        E::Interrupted => "Interrupted",
        E::Unsupported => "Unsupported",
        E::UnexpectedEof => "UnexpectedEof",
        E::OutOfMemory => "OutOfMemory",
        E::Other => "Other",
    }
}
