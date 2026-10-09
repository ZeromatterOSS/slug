//! Borrowed test-only context at the original first-created source refusal.
use std::fmt;
use std::fmt::Write;

use slug_bzlmod_v2::RepositoryMaterializationInstanceLookupError as LookupError;
use slug_bzlmod_v2::RepositoryMaterializationResultEpoch;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;

use super::materialization_diagnostic::MaterializationDiagnostic;
use super::materialization_diagnostic::MaterializationWriter;

const NAMESPACE_ENTRIES: usize = 32;
const CONTEXT_LINE_BYTES: usize = 1024;

#[derive(Clone, Copy)]
pub(super) enum SourceAttemptPhase {
    CommandPreflight,
    NeedPaths,
    CertificatePrecheck,
    CertificateFinalize,
}
impl SourceAttemptPhase {
    fn name(self) -> &'static str {
        match self {
            Self::CommandPreflight => "command_preflight",
            Self::NeedPaths => "need_paths",
            Self::CertificatePrecheck => "certificate_precheck",
            Self::CertificateFinalize => "certificate_finalize",
        }
    }
    fn epoch_source(self) -> &'static str {
        match self {
            Self::CommandPreflight => "unavailable",
            Self::NeedPaths => "command",
            Self::CertificatePrecheck => "selected_completion",
            Self::CertificateFinalize => "selected_finalization",
        }
    }
}

pub(super) struct SourceAttemptContext<'a> {
    phase: SourceAttemptPhase,
    workspace: &'a NormalizedAbsolutePath,
    epoch: Option<&'a RepositoryMaterializationResultEpoch>,
}
impl<'a> SourceAttemptContext<'a> {
    pub(super) fn command_preflight(workspace: &'a NormalizedAbsolutePath) -> Self {
        Self {
            phase: SourceAttemptPhase::CommandPreflight,
            workspace,
            epoch: None,
        }
    }
    pub(super) fn need_paths(
        workspace: &'a NormalizedAbsolutePath,
        epoch: Option<&'a RepositoryMaterializationResultEpoch>,
    ) -> Self {
        Self {
            phase: SourceAttemptPhase::NeedPaths,
            workspace,
            epoch,
        }
    }
    pub(super) fn certificate_precheck(
        workspace: &'a NormalizedAbsolutePath,
        epoch: Option<&'a RepositoryMaterializationResultEpoch>,
    ) -> Self {
        Self {
            phase: SourceAttemptPhase::CertificatePrecheck,
            workspace,
            epoch,
        }
    }
    pub(super) fn certificate_finalize(
        workspace: &'a NormalizedAbsolutePath,
        epoch: Option<&'a RepositoryMaterializationResultEpoch>,
    ) -> Self {
        Self {
            phase: SourceAttemptPhase::CertificateFinalize,
            workspace,
            epoch,
        }
    }

    // Pure projection also represents zero-instance lookup refusal for tests;
    // report rejects that unobservable kernel domain before emitting anything.
    pub(super) fn diagnostic<'b>(
        &'b self,
        demand: &'b PathObservationDemand,
    ) -> SourceContextDiagnostic<'b> {
        let mut result = SourceContextDiagnostic {
            demand,
            phase: self.phase,
            epoch_source: if self.epoch.is_some() {
                self.phase.epoch_source()
            } else {
                "unavailable"
            },
            association: "unavailable",
            root_relation: "unavailable",
            owner: None,
        };
        let PathObservationNamespace::Materialization(instance) = demand.namespace() else {
            result.association = "host";
            result.root_relation = "not_applicable";
            return result;
        };
        let Some(epoch) = self.epoch else {
            return result;
        };
        let view = match epoch.borrowed_instance_for_namespace(
            self.workspace,
            instance,
            NAMESPACE_ENTRIES,
        ) {
            Ok(Some(view)) => view,
            Ok(None) => {
                result.association = "missing";
                return result;
            }
            Err(error) => {
                result.association = match error {
                    LookupError::WrongWorkspace => "wrong_workspace",
                    LookupError::InvalidInstance => "invalid_instance",
                    LookupError::TooManyEntries => "bounds_refused",
                    LookupError::AmbiguousInstance => "ambiguous",
                    LookupError::InvalidGenerationRoot => "invalid_generation_root",
                };
                return result;
            }
        };
        let path = demand.path().as_path();
        let root = view.generation_root();
        result.root_relation = if path == root {
            "root"
        } else if path.starts_with(root) {
            "descendant"
        } else if root.starts_with(path) {
            "ancestor"
        } else {
            "outside"
        };
        if matches!(result.root_relation, "root" | "descendant") {
            result.association = "unique";
            result.owner = Some(MaterializationDiagnostic::new(
                view.request(),
                "observation_instance",
            ));
        } else {
            result.association = "path_mismatch";
        }
        result
    }

    pub(super) fn report_charge(
        &self,
        scope: &super::dice::source_service::SourceServiceScope,
        demand: &PathObservationDemand,
        record: super::dice::source_service::SourceChargeRecord,
    ) {
        use super::dice::source_service::SourceChargeIdentity;
        scope.report_source_charge_for_test(
            record,
            self.charge_phase(),
            SourceChargeIdentity::Demand(demand),
        );
    }

    pub(super) fn charge_phase(&self) -> super::dice::source_service::SourceChargePhase {
        use super::dice::source_service::SourceChargePhase;
        match self.phase {
            SourceAttemptPhase::CommandPreflight => SourceChargePhase::CommandPreflight,
            SourceAttemptPhase::NeedPaths => SourceChargePhase::NeedPaths,
            SourceAttemptPhase::CertificatePrecheck => SourceChargePhase::CertificatePrecheck,
            SourceAttemptPhase::CertificateFinalize => SourceChargePhase::CertificateFinalize,
        }
    }

    pub(super) fn report(&self, demand: &PathObservationDemand) {
        if demand.operation() != PathObservationOperation::FileBytes
            || matches!(demand.namespace(), PathObservationNamespace::Materialization(id) if id.value() == 0)
        {
            return;
        }
        println!("{}", self.diagnostic(demand));
    }
}

pub(super) struct SourceContextDiagnostic<'a> {
    pub(super) demand: &'a PathObservationDemand,
    phase: SourceAttemptPhase,
    epoch_source: &'static str,
    pub(super) association: &'static str,
    pub(super) root_relation: &'static str,
    pub(super) owner: Option<MaterializationDiagnostic<'a>>,
}
impl fmt::Display for SourceContextDiagnostic<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut writer = MaterializationWriter {
            writer: formatter,
            bytes: 0,
            limit: CONTEXT_LINE_BYTES,
        };
        write!(
            writer,
            "WP990_SOURCE_CONTEXT phase={} epoch_source={} association={} namespace=",
            self.phase.name(),
            self.epoch_source,
            self.association
        )?;
        match self.demand.namespace() {
            PathObservationNamespace::Host => writer.write_str("host instance=none")?,
            PathObservationNamespace::Materialization(id) => {
                write!(writer, "materialization instance={}", id.value())?
            }
        }
        write!(
            writer,
            " operation=file_bytes root_relation={}",
            self.root_relation
        )?;
        if let Some(owner) = &self.owner {
            writeln!(formatter)?;
            owner.write_marker(formatter, "WP990_SOURCE_OWNER")?;
        }
        Ok(())
    }
}
