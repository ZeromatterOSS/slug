//! The private, declared normal-compute bridge for input-image preparation.
//! Image observations are transient here; process owners are not activated.
use slug_workspace_v2::ElfMetadataPolicy;
use slug_workspace_v2::ElfSourceDigestBinding;
use slug_workspace_v2::ImageChargedPath;
use slug_workspace_v2::ImageObservationOperation;
use slug_workspace_v2::InputImagePolicy;
use slug_workspace_v2::ObservedElfMetadata;
use slug_workspace_v2::ObservedElfMetadataKey;
use slug_workspace_v2::ObservedInputImage;
use slug_workspace_v2::ObservedInputImageKey;
use slug_workspace_v2::PathDirectoryName;
use slug_workspace_v2::PathOutcome;

use super::*;
use crate::runtime::input_image::CapturedInputImageLease;
use crate::runtime::input_image::{self};

#[allow(dead_code)]
#[derive(Clone)]
pub(super) enum DeclaredImageSource {
    WorkspaceSubtree(PathBuf),
    ImmutableRepository {
        request: Arc<RepositoryMaterializationRequest>,
        subtree: PathBuf,
    },
}

#[derive(Clone)]
pub(super) struct DeclaredImageRequest {
    key: ObservedInputImageKey,
    elf: Option<DeclaredElfRequest>,
    deadline: std::time::Instant,
    repository: Option<(
        Arc<RepositoryMaterializationRequest>,
        slug_bzlmod_v2::RepositoryMaterialization,
    )>,
}

#[derive(Clone)]
struct DeclaredElfRequest {
    certificate: ObservedInputImage,
    file: ImageChargedPath,
    policy: ElfMetadataPolicy,
}

impl DeclaredElfRequest {
    fn allows(&self, demand: &PathObservationDemand) -> bool {
        let PathObservationOperation::InputImage(request) = demand.operation() else {
            return false;
        };
        let ImageObservationOperation::BoundedElfMetadata {
            expected_digest,
            policy,
        } = request.operation()
        else {
            return false;
        };
        self.file.as_path() == demand.path().as_path()
            && self.certificate.namespace() == demand.namespace()
            && self.certificate.anchor_root() == request.scope_anchor_root()
            && policy == self.policy
            && self
                .certificate
                .entries()
                .iter()
                .find(|entry| entry.path().as_path() == self.file.as_path())
                .and_then(|entry| entry.file_digest())
                .map(ElfSourceDigestBinding::from_digest)
                == Some(expected_digest)
    }

    fn same_budget(&self, budget: &slug_workspace_v2::ImageDescriptorBudget) -> bool {
        self.certificate.same_budget(budget) && self.file.same_budget(budget)
    }
}

impl DeclaredImageRequest {
    fn source(
        &self,
        command: &NativeDemandCommand<'_>,
    ) -> Result<input_image::ScopedImageSource, NativeDemandSessionError> {
        if std::time::Instant::now() >= self.deadline {
            return Err(image_error("image preparation deadline"));
        }
        command
            .runtime
            .repository_materializer
            .image_source(
                command.repository_session,
                self.key.namespace(),
                self.key.anchor_root(),
                self.repository
                    .as_ref()
                    .map(|(request, result)| (request, result)),
            )
            .map_err(NativeDemandSessionError::Repository)
    }

    fn allows(&self, demand: &PathObservationDemand) -> bool {
        let PathObservationOperation::InputImage(request) = demand.operation() else {
            return false;
        };
        let limits = self.key.policy().limits();
        if demand.namespace() != self.key.namespace()
            || request.scope_anchor_root() != self.key.anchor_root()
        {
            return false;
        }
        let path = demand.path().as_path();
        if !path.starts_with(self.key.anchor_root().as_path())
            || !(path.starts_with(self.key.source_root().as_path())
                || self.key.source_root().as_path().starts_with(path))
        {
            return false;
        }
        match request.operation() {
            ImageObservationOperation::BoundedElfMetadata { .. } => {
                self.elf.as_ref().is_some_and(|elf| elf.allows(demand))
            }
            ImageObservationOperation::Lstat => true,
            ImageObservationOperation::BoundedDigest { max_bytes } => {
                max_bytes == limits.max_file_bytes
            }
            ImageObservationOperation::BoundedDirectory(bounds) => {
                bounds.max_entries <= limits.max_entries
                    && bounds.max_name_bytes_total <= limits.max_descriptor_bytes
                    && bounds.max_component_bytes <= limits.max_path_bytes
            }
            ImageObservationOperation::BoundedReadLink { max_target_bytes } => {
                max_target_bytes <= limits.max_path_bytes
            }
        }
    }
}

pub(super) fn observe_command_paths(
    command: &NativeDemandCommand<'_>,
    demands: Vec<PathObservationDemand>,
) -> Result<PathObservationEpoch, NativeDemandSessionError> {
    let mut observations = Vec::with_capacity(demands.len());
    let mut legacy = Vec::new();
    let budget = command.runtime.image_resources.descriptor_budget();
    // Validate the entire image batch before doing even the first read.
    for demand in &demands {
        if matches!(demand.operation(), PathObservationOperation::InputImage(_)) {
            let request = command
                .image_request
                .as_ref()
                .ok_or_else(|| image_error("undeclared image observation"))?;
            let PathObservationOperation::InputImage(scope) = demand.operation() else {
                unreachable!()
            };
            if !request.allows(demand)
                || request
                    .elf
                    .as_ref()
                    .is_some_and(|elf| !elf.same_budget(budget))
                || !scope.same_budget(budget)
                || !demand
                    .image_path()
                    .is_some_and(|path| path.same_budget(budget))
            {
                return Err(image_error("foreign image observation"));
            }
        }
    }
    let source = demands
        .iter()
        .any(|demand| demand.image_path().is_some())
        .then(|| {
            command
                .image_request
                .as_ref()
                .expect("validated image request")
                .source(command)
        })
        .transpose()?;
    let active = || {
        command
            .image_request
            .as_ref()
            .is_none_or(|request| std::time::Instant::now() < request.deadline)
            && command
                .runtime
                .repository_materializer
                .session_is_active(command.repository_session)
    };
    for demand in demands {
        if matches!(demand.operation(), PathObservationOperation::InputImage(_)) {
            let PathObservationOperation::InputImage(scope) = demand.operation() else {
                unreachable!()
            };
            let result = if matches!(
                scope.operation(),
                ImageObservationOperation::BoundedElfMetadata { .. }
            ) {
                observe_declared_elf(command, &demand, &active)?
            } else {
                source
                    .as_ref()
                    .expect("authorized image source")
                    .observe(&demand, budget, &active)
                    .map_err(NativeDemandSessionError::InputImage)?
            };
            observations.push((demand, Arc::new(result)));
        } else {
            legacy.push(demand);
        }
    }
    if !legacy.is_empty() {
        let observed = command
            .runtime
            .repository_materializer
            .observe_native(command.repository_session, legacy)
            .map_err(NativeDemandSessionError::Repository)?;
        observations.extend(
            observed
                .observations()
                .iter()
                .map(|(demand, result)| (demand.clone(), result.dupe())),
        );
    }
    PathObservationEpoch::from_shared(observations).map_err(NativeDemandSessionError::PathEpoch)
}

fn image_error(message: &'static str) -> NativeDemandSessionError {
    NativeDemandSessionError::Computation(anyhow::anyhow!(message))
}

fn observe_declared_elf(
    command: &NativeDemandCommand<'_>,
    demand: &PathObservationDemand,
    active: &dyn Fn() -> bool,
) -> Result<PathObservationResult, NativeDemandSessionError> {
    let request = command
        .image_request
        .as_ref()
        .ok_or_else(|| image_error("missing ELF declaration"))?;
    let elf = request
        .elf
        .as_ref()
        .ok_or_else(|| image_error("missing ELF declaration"))?;
    #[cfg(target_os = "linux")]
    {
        let authorized = input_image::elf_metadata::AuthorizedElfObservation::new(
            request.source(command)?,
            elf.certificate.clone(),
            elf.file.clone(),
            elf.policy,
            &command.runtime.image_resources,
        )
        .map_err(NativeDemandSessionError::InputImage)?;
        input_image::elf_metadata::observe_elf_metadata(
            &authorized,
            demand,
            &command.runtime.image_resources,
            active,
        )
        .map_err(NativeDemandSessionError::InputImage)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (elf, demand, active);
        Err(NativeDemandSessionError::InputImage(
            input_image::InputImageCaptureError::UnsupportedPlatform,
        ))
    }
}

#[path = "input_image_bridge/prepare.rs"]
mod prepare;

#[cfg(all(test, target_os = "linux"))]
#[path = "input_image_bridge/tests.rs"]
mod tests;

#[cfg(all(test, target_os = "linux"))]
#[path = "input_image_bridge/elf_tests.rs"]
mod elf_tests;
