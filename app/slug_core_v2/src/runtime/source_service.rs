//! Private, consumed diagnostic acquisition service. Ordinary native services
//! do not install this scope; none of these limits are a total RSS guarantee.

#[path = "source_service/diagnostic.rs"]
mod diagnostic;
#[path = "source_service/http.rs"]
mod http;
#[path = "source_service/registry.rs"]
mod registry;
#[path = "source_service/scope.rs"]
mod scope;
#[cfg(test)]
#[path = "source_service/tests.rs"]
mod tests;

pub(in crate::runtime::dice) use diagnostic::ScopedPreparedSourceViewLease;
pub(in crate::runtime::dice) use diagnostic::SourcePreparationDiagnosticRuntime;
pub(in crate::runtime) use diagnostic::WorkspaceSourceServices;
pub(in crate::runtime) use http::SourceHttpAdmission;
pub(in crate::runtime) use http::request_scoped;
pub(in crate::runtime) use registry::ScopedRegistryIo;
pub(in crate::runtime) use scope::SourceBufferReservation;
pub(in crate::runtime) use scope::SourceCounter;
pub(in crate::runtime) use scope::SourceServiceCounters;
pub(in crate::runtime) use scope::SourceServiceError;
pub(in crate::runtime) use scope::SourceServiceLimits;
pub(in crate::runtime) use scope::SourceServiceScope;
