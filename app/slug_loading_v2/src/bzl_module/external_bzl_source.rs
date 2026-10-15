//! Preserve the source reader's logical path with its exact bytes and observations.
use super::*;

#[derive(Debug)]
pub(super) struct ExternalBzlSourceValue {
    pub(super) bytes: Arc<[u8]>,
    pub(super) source_name: String,
    pub(super) presentation_path: PathBuf,
    pub(super) observations: PathObservationEpoch,
}

// Both request routes preserve the SAME native source reader contract. This
// never canonicalizes a path or reads the host; logical_path came with bytes.
fn present(
    bytes: Arc<[u8]>,
    logical_path: NormalizedAbsolutePath,
    label: CanonicalLabel,
    observations: PathObservationEpoch,
) -> Result<ExternalBzlSourceValue, ExternalBzlDriverOutcome> {
    match external_source_name(&logical_path) {
        Ok(source_name) => Ok(ExternalBzlSourceValue {
            bytes,
            source_name,
            presentation_path: logical_path.as_path().to_path_buf(),
            observations,
        }),
        Err(()) => Err(external_bzl_complete(
            Err(ExternalBzlModuleError::Encoding { label }),
            observations,
        )),
    }
}

pub(super) fn finish_external_bzl_source(
    child: ExternalBzlSourceChild,
    label: CanonicalLabel,
) -> Result<ExternalBzlSourceValue, ExternalBzlDriverOutcome> {
    match child {
        ExternalBzlSourceChild::Root(result, observations) => match result {
            Ok(HostRepositorySourceFileValue::Present {
                bytes,
                logical_path,
            }) => present(bytes, logical_path, label, observations),
            Ok(HostRepositorySourceFileValue::Absent) => Err(external_bzl_complete(
                Err(ExternalBzlModuleError::Absent { label }),
                observations,
            )),
            Err(error) => Err(external_bzl_complete(
                Err(ExternalBzlModuleError::Source { label, error }),
                observations,
            )),
        },
        ExternalBzlSourceChild::Observation(result, observations) => match result.as_ref() {
            Ok(HostRepositorySourceObservation::Builtin(value)) => {
                // Builtin has no Host reader logical_path. Its existing
                // canonical-label presentation and catalog identity are kept.
                let source_name = label.to_string();
                Ok(ExternalBzlSourceValue {
                    bytes: value.bytes_arc().dupe(),
                    presentation_path: PathBuf::from(&source_name),
                    source_name,
                    observations,
                })
            }
            Ok(HostRepositorySourceObservation::Request(
                HostRepositorySourceFileValue::Present {
                    bytes,
                    logical_path,
                },
            )) => present(bytes.dupe(), logical_path.dupe(), label, observations),
            Ok(HostRepositorySourceObservation::Request(HostRepositorySourceFileValue::Absent)) => {
                Err(external_bzl_complete(
                    Err(ExternalBzlModuleError::Absent { label }),
                    observations,
                ))
            }
            Err(error) => Err(external_bzl_complete(
                Err(ExternalBzlModuleError::SourceObservation {
                    label,
                    error: error.clone(),
                }),
                observations,
            )),
        },
        ExternalBzlSourceChild::DependencyCycle(cycle) => Err(external_bzl_complete(
            Err(ExternalBzlModuleError::DependencyCycle(cycle)),
            PathObservationEpoch::empty(),
        )),
        ExternalBzlSourceChild::Compute(message) => Err(external_bzl_complete(
            Err(ExternalBzlModuleError::SourceCompute { label, message }),
            PathObservationEpoch::empty(),
        )),
    }
}

#[cfg(test)]
mod tests;
