//! Private scoped acquisition for consumed diagnostics and fresh general commands.
//! Ordinary unscoped services stay native; these limits are not a total RSS guarantee.

#[path = "source_service/buffer_provenance.rs"]
mod buffer_provenance;
#[path = "source_service/diagnostic.rs"]
mod diagnostic;
#[path = "source_service/failure.rs"]
mod failure;
#[path = "source_service/http.rs"]
mod http;
#[cfg(all(test, target_os = "linux"))]
#[path = "source_service/offline_archive.rs"]
mod offline_archive;
#[path = "source_service/registry.rs"]
mod registry;
#[cfg(all(test, target_os = "linux"))]
#[path = "source_service/registry_association.rs"]
mod registry_association;
#[path = "source_service/regular_file.rs"]
mod regular_file;
#[path = "source_service/scope.rs"]
mod scope;
#[cfg(all(test, target_os = "linux"))]
#[path = "source_service/source_charge_census.rs"]
mod source_charge_census;
#[cfg(test)]
#[path = "source_service/tests.rs"]
mod tests;
#[path = "source_service/timeout_provenance.rs"]
mod timeout_provenance;

#[cfg(all(test, target_os = "linux"))]
pub(in crate::runtime) use buffer_provenance::SourceBufferKind;
#[cfg(all(test, target_os = "linux"))]
pub(in crate::runtime) use buffer_provenance::SourceBufferOrigin;
pub(in crate::runtime) use buffer_provenance::SourceBufferSite;
#[cfg(all(test, target_os = "linux"))]
pub(in crate::runtime) use buffer_provenance::SourceBufferSnapshot;
pub(in crate::runtime) use buffer_provenance::SourceBufferUnit;
#[cfg(test)]
pub(in crate::runtime::dice) use diagnostic::DiagnosticObserver;
pub(in crate::runtime::dice) use diagnostic::ScopedPreparedSourceViewLease;
pub(in crate::runtime::dice) use diagnostic::SourcePreparationDiagnosticRuntime;
pub(in crate::runtime) use diagnostic::WorkspaceSourceServices;
pub(super) use failure::SourceTransportFailure;
pub(in crate::runtime) use http::SourceHttpAdmission;
pub(in crate::runtime) use http::request_scoped;
pub(super) use http::request_scoped_typed;
#[cfg(all(test, target_os = "linux"))]
pub(in crate::runtime) use offline_archive::OfflineArchiveBacking;
pub(in crate::runtime) use registry::ScopedRegistryIo;
#[cfg(all(test, target_os = "linux"))]
pub(in crate::runtime) use registry_association::RegistryAssociationSnapshot;
#[cfg(all(test, target_os = "linux"))]
pub(in crate::runtime) use registry_association::RegistryLimitRelation;
#[cfg(all(test, target_os = "linux"))]
pub(in crate::runtime) use registry_association::RegistryRequestAssociation;
pub(in crate::runtime) use regular_file::HOST_FILE_OBJECT_BYTES;
#[cfg(all(test, target_os = "linux"))]
pub(in crate::runtime) use regular_file::KNOWN_SHAS_OBJECT_BYTES;
pub(in crate::runtime) use regular_file::RegularFileError;
pub(in crate::runtime) use regular_file::RegularFileStage;
pub(in crate::runtime) use regular_file::read_host_file;
#[cfg(all(test, target_os = "linux"))]
pub(in crate::runtime) use regular_file::read_host_file_with_charge_origin;
#[cfg(all(test, target_os = "linux"))]
pub(in crate::runtime) use regular_file::read_host_file_with_known_shas_allowance;
#[cfg(all(test, target_os = "linux"))]
pub(in crate::runtime) use regular_file::read_host_file_with_known_shas_allowance_and_census;
#[cfg(target_os = "linux")]
pub(in crate::runtime) use regular_file::read_open_regular_file;
#[cfg(target_os = "linux")]
pub(in crate::runtime) use regular_file::read_open_regular_file_after_attempt;
pub(in crate::runtime) use scope::SourceBufferReservation;
pub(in crate::runtime) use scope::SourceCounter;
pub(in crate::runtime) use scope::SourceDecoderReservation;
pub(in crate::runtime) use scope::SourceServiceCounters;
pub(in crate::runtime) use scope::SourceServiceError;
pub(in crate::runtime) use scope::SourceServiceLimits;
pub(in crate::runtime) use scope::SourceServiceScope;
#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
))]
pub(in crate::runtime) use scope::SourceXzDecoder;
#[cfg(all(test, target_os = "linux"))]
pub(in crate::runtime) use source_charge_census::SourceChargeDomain;
#[cfg(all(test, target_os = "linux"))]
pub(in crate::runtime) use source_charge_census::SourceChargeIdentity;
#[cfg(all(test, target_os = "linux"))]
pub(in crate::runtime) use source_charge_census::SourceChargePhase;
#[cfg(all(test, target_os = "linux"))]
pub(in crate::runtime) use source_charge_census::SourceChargeRecord;
#[cfg(all(test, target_os = "linux"))]
pub(in crate::runtime) use timeout_provenance::SourceTimeoutDomain;
#[cfg(all(test, target_os = "linux"))]
pub(in crate::runtime) use timeout_provenance::SourceTimeoutOrigin;
#[cfg(all(test, target_os = "linux"))]
pub(in crate::runtime) use timeout_provenance::SourceTimeoutPhase;
#[cfg(all(test, target_os = "linux"))]
pub(in crate::runtime) use timeout_provenance::SourceTimeoutReturn;
#[cfg(all(test, target_os = "linux"))]
pub(in crate::runtime) use timeout_provenance::SourceTimeoutSite;
#[cfg(all(test, target_os = "linux"))]
pub(in crate::runtime) use timeout_provenance::SourceTimeoutSnapshot;
