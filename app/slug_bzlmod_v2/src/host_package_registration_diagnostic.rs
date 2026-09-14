//! Borrowed causal projection for registration diagnostics, not semantic identity.

use std::fmt;
use std::path::Path;

use super::*;

#[cfg(test)]
#[path = "host_package_registration_diagnostic_tests.rs"]
mod tests;

impl RepositoryPackageSourceError {
    /// Stream only bounded causal fields. The caller owns escaping and its budget.
    #[doc(hidden)]
    pub fn write_registration_diagnostic(&self, out: &mut dyn fmt::Write) -> fmt::Result {
        use RepositoryPackageSourceErrorInner as E;
        match &self.inner {
            E::ModuleEvaluation { .. } => out.write_str("ModuleEvaluation"),
            E::Unsupported { .. } => out.write_str("Unsupported"),
            E::InvalidPackageName { package, .. } => {
                write!(out, "InvalidPackageName package={package}")
            }
            E::Deleted { package } => write!(out, "Deleted package={package}"),
            E::NoBuildFile { package } => write!(out, "NoBuildFile package={package}"),
            E::Lookup { package, error } => {
                write!(out, "Lookup package={package} error=")?;
                lookup(out, error)
            }
            E::LookupCompute { package, .. } => {
                write!(out, "LookupCompute package={package}")
            }
            E::Source {
                logical_path,
                error,
            } => {
                out.write_str("Source path=")?;
                path(out, logical_path)?;
                out.write_str(" error=")?;
                error.write_registration_diagnostic(out)
            }
            E::SourceObservation {
                logical_path,
                error,
            } => {
                out.write_str("SourceObservation path=")?;
                path(out, logical_path)?;
                out.write_str(" error=")?;
                error.write_registration_diagnostic(out)
            }
            E::SourceCompute { logical_path, .. } => {
                out.write_str("SourceCompute path=")?;
                path(out, logical_path)
            }
            E::SelectedSourceAbsent { logical_path } => {
                out.write_str("SelectedSourceAbsent path=")?;
                path(out, logical_path)
            }
        }
    }
}

fn lookup(out: &mut dyn fmt::Write, error: &ExternalRepositoryPackageLookupError) -> fmt::Result {
    match error {
        ExternalRepositoryPackageLookupError::PolicyInput(_) => out.write_str("PolicyInput"),
        ExternalRepositoryPackageLookupError::RepositoryIgnore(error) => {
            out.write_str("RepositoryIgnore.")?;
            error.write_registration_diagnostic(out)
        }
        ExternalRepositoryPackageLookupError::RepositoryListing(_) => {
            out.write_str("RepositoryListing")
        }
        ExternalRepositoryPackageLookupError::Path(error) => {
            out.write_str("Path.")?;
            error.write_registration_diagnostic(out)
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
