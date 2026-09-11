//! Borrowed causal projection for registration diagnostics, not semantic identity.
use super::*;

#[cfg(test)]
#[path = "registration_diagnostic_tests.rs"]
mod tests;

impl HostSelectedExtensionDemandError {
    /// Stream only causal fields. The caller owns escaping and the output budget.
    #[doc(hidden)]
    pub fn write_registration_diagnostic(&self, out: &mut dyn fmt::Write) -> fmt::Result {
        match &self.0 {
            DemandError::Mappings(error) => mappings(out, error),
            DemandError::Missing { requested } => write!(out, "Missing {}", requested.as_str()),
            DemandError::Ambiguous {
                requested,
                first,
                conflicting,
            } => write!(
                out,
                "Ambiguous {}: {} / {}",
                requested.as_str(),
                first.unique_name().as_str(),
                conflicting.unique_name().as_str(),
            ),
            DemandError::Inconsistent { requested, owner } => write!(
                out,
                "Inconsistent {}: {}",
                requested.as_str(),
                owner.unique_name().as_str(),
            ),
        }
    }
}

impl HostSelectedExtensionOwnerInputsError {
    /// Stream only causal fields; never render retained owner state.
    #[doc(hidden)]
    pub fn write_registration_diagnostic(&self, out: &mut dyn fmt::Write) -> fmt::Result {
        inputs(out, &self.0)
    }
}

impl HostSelectedInnateRepositoryOwnerInputsError {
    /// Stream only causal fields; never render retained owner state.
    #[doc(hidden)]
    pub fn write_registration_diagnostic(&self, out: &mut dyn fmt::Write) -> fmt::Result {
        inputs(out, &self.0)
    }
}

fn inputs(out: &mut dyn fmt::Write, error: &OwnerInputsError) -> fmt::Result {
    match error {
        OwnerInputsError::Mappings(error) => mappings(out, error),
        OwnerInputsError::Missing { owner } => {
            write!(out, "Missing {}", owner.unique_name().as_str())
        }
        OwnerInputsError::Inconsistent { owner } => {
            write!(out, "Inconsistent {}", owner.unique_name().as_str())
        }
        OwnerInputsError::Unsupported { owner } => {
            write!(out, "Unsupported {}", owner.unique_name().as_str())
        }
        OwnerInputsError::Invalid { owner, message } => {
            write!(out, "Invalid {}: {message}", owner.unique_name().as_str())
        }
    }
}

fn module(out: &mut dyn fmt::Write, key: &HostGraphModuleKey) -> fmt::Result {
    match key {
        HostGraphModuleKey::Root => out.write_str("<root>"),
        HostGraphModuleKey::Module { name, version } => {
            write!(out, "{name}@{}", version.normalized())
        }
    }
}

fn mappings(out: &mut dyn fmt::Write, error: &HostSelectedExtensionMappingsError) -> fmt::Result {
    use HostSelectedExtensionMappingsError as E;
    out.write_str("Mappings: ")?;
    match error {
        E::Routes(error) => routes(out, error),
        E::RoutesCompute(message) => write!(out, "RoutesCompute: {message}"),
        E::RootFiles(message) => write!(out, "RootFiles: {message}"),
        E::RootFilesCompute(message) => write!(out, "RootFilesCompute: {message}"),
        E::Invalid { owner, message } => {
            out.write_str("Invalid ")?;
            module(out, owner)?;
            write!(out, ": {message}")
        }
    }
}

fn routes(out: &mut dyn fmt::Write, error: &HostSelectedModuleRoutesError) -> fmt::Result {
    use HostSelectedModuleRoutesError as E;
    out.write_str("Routes: ")?;
    match error {
        E::Graph(_) => out.write_str("[diagnostic incomplete: Graph]"),
        E::RepoSpecs(_) => out.write_str("[diagnostic incomplete: RepoSpecs]"),
        E::GraphCompute(message) => write!(out, "GraphCompute: {message}"),
        E::RepoSpecsCompute(message) => write!(out, "RepoSpecsCompute: {message}"),
        E::Invalid {
            module: key,
            message,
        }
        | E::RegistryMismatch {
            module: key,
            message,
        } => {
            out.write_str(if matches!(error, E::Invalid { .. }) {
                "Invalid "
            } else {
                "RegistryMismatch "
            })?;
            module(out, key)?;
            write!(out, ": {message}")
        }
        E::CanonicalCollision {
            canonical_repo,
            first,
            second,
        } => {
            write!(out, "CanonicalCollision {}: ", canonical_repo.as_str())?;
            module(out, first)?;
            out.write_str(" / ")?;
            module(out, second)
        }
    }
}
