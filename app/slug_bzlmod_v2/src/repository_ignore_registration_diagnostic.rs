//! Borrowed causal projection for registration diagnostics, not semantic identity.

use std::fmt;
use std::path::Path;

use super::*;

#[cfg(test)]
#[path = "repository_ignore_registration_diagnostic_tests.rs"]
mod tests;

impl HostRepositoryIgnoreError {
    pub(crate) fn write_registration_diagnostic(&self, out: &mut dyn fmt::Write) -> fmt::Result {
        match self {
            Self::RepoFile(_) => out.write_str("RepoFile"),
            Self::RouteRepoFile(error) => {
                out.write_str("RouteRepoFile.")?;
                error.write_registration_diagnostic(out)
            }
            Self::NonregistryRepoFile(error) => {
                out.write_str("NonregistryRepoFile.")?;
                error.write_registration_diagnostic(out)
            }
            Self::PolicyProjection(_) => out.write_str("PolicyProjection"),
            Self::RepositoryListing(_) => out.write_str("RepositoryListing"),
            Self::BuiltinMetadata { actual } => {
                write!(out, "BuiltinMetadata actual={actual:?}")
            }
            Self::HostFile(_) => out.write_str("HostFile"),
            Self::RepositorySource(error) => {
                out.write_str("RepositorySource.")?;
                error.write_registration_diagnostic(out)
            }
            Self::RepositorySourceObservation(error) => {
                out.write_str("RepositorySourceObservation.")?;
                error.write_registration_diagnostic(out)
            }
            Self::InvalidAbsolute { logical_path, .. } => {
                out.write_str("InvalidAbsolute path=")?;
                path(out, logical_path.as_path())
            }
            Self::NativeInvalid { logical_path, .. } => {
                out.write_str("NativeInvalid path=")?;
                path(out, logical_path.as_path())
            }
        }
    }
}

fn path(out: &mut dyn fmt::Write, value: &Path) -> fmt::Result {
    out.write_str("hex:")?;
    for byte in value.as_os_str().as_encoded_bytes() {
        write!(out, "{byte:02x}")?;
    }
    Ok(())
}
