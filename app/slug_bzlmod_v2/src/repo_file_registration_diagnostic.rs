//! Borrowed causal projection for registration diagnostics, not semantic identity.

use std::fmt;
use std::path::Path;

use super::*;

#[cfg(test)]
#[path = "repo_file_registration_diagnostic_tests.rs"]
mod tests;

impl HostRouteRepoFileError {
    pub(crate) fn write_registration_diagnostic(&self, out: &mut dyn fmt::Write) -> fmt::Result {
        match self {
            Self::PolicyProjection(_) => out.write_str("PolicyProjection"),
            Self::BuiltinListing(_) => out.write_str("BuiltinListing"),
            Self::BuiltinMetadata { actual } => {
                write!(out, "BuiltinMetadata actual={actual:?}")
            }
            Self::Source(error) => {
                out.write_str("Source.")?;
                error.write_registration_diagnostic(out)
            }
            Self::SourceObservation(error) => {
                out.write_str("SourceObservation.")?;
                error.write_registration_diagnostic(out)
            }
            Self::Evaluation(error) => evaluation(out, error),
        }
    }
}

fn evaluation(out: &mut dyn fmt::Write, error: &HostRepoFileError) -> fmt::Result {
    let (kind, logical_path) = match error {
        HostRepoFileError::PolicyProjection(_) => {
            return out.write_str("Evaluation.PolicyProjection");
        }
        HostRepoFileError::HostFile(_) => return out.write_str("Evaluation.HostFile"),
        HostRepoFileError::InvalidUtf8 { logical_path } => ("InvalidUtf8", logical_path),
        HostRepoFileError::Syntax { logical_path, .. } => ("Syntax", logical_path),
        HostRepoFileError::RestrictedSyntax { logical_path, .. } => {
            ("RestrictedSyntax", logical_path)
        }
        HostRepoFileError::Compile { logical_path, .. } => ("Compile", logical_path),
        HostRepoFileError::Evaluation { logical_path, .. } => ("Evaluation", logical_path),
    };
    write!(out, "Evaluation.{kind} path=")?;
    path(out, logical_path.as_path())
}

fn path(out: &mut dyn fmt::Write, value: &Path) -> fmt::Result {
    out.write_str("hex:")?;
    for byte in value.as_os_str().as_encoded_bytes() {
        write!(out, "{byte:02x}")?;
    }
    Ok(())
}
