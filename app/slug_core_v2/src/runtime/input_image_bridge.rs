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
    SelectedLabel(slug_bzlmod_v2::SelectedLabelSourceKey),
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
    repository: Option<DeclaredImageRepository>,
}

#[derive(Clone)]
enum DeclaredImageRepository {
    Explicit(
        Arc<RepositoryMaterializationRequest>,
        slug_bzlmod_v2::RepositoryMaterialization,
    ),
    Selected(slug_bzlmod_v2::ObservedSelectedLabelSource),
}

impl DeclaredImageRepository {
    fn selection(
        &self,
    ) -> (
        &Arc<RepositoryMaterializationRequest>,
        &slug_bzlmod_v2::RepositoryMaterialization,
    ) {
        match self {
            Self::Explicit(request, result) => (request, result),
            Self::Selected(selected) => selected.repository().expect("external selected source"),
        }
    }
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
                    .map(DeclaredImageRepository::selection),
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

/// The baseline driver supplies its one current declaration unchanged.
pub(super) fn observe_command_paths(
    command: &NativeDemandCommand<'_>,
    demands: Vec<PathObservationDemand>,
) -> Result<PathObservationEpoch, NativeDemandSessionError> {
    let declarations = command.image_request.as_slice();
    let budget = command.runtime.image_resources.descriptor_budget();
    // Preserve the legacy admission order and error: even an expired source
    // declaration cannot hide a foreign later row in the same incoming batch.
    for demand in &demands {
        let PathObservationOperation::InputImage(scope) = demand.operation() else {
            continue;
        };
        let request = command
            .image_request
            .as_ref()
            .ok_or_else(|| image_error("undeclared image observation"))?;
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
    let credit = image_batch_credit(command, &demands)?;
    // The legacy source() deadline check follows whole-batch admission, before
    // the observer's cancellation callback. Open one retained source per batch.
    let sources = retain_image_sources(command, declarations, &demands, None)?;
    observe_authorized_image_paths(command, declarations, &sources, demands, credit, &|| {
        declarations
            .iter()
            .all(|request| std::time::Instant::now() < request.deadline)
            && command
                .runtime
                .repository_materializer
                .session_is_active(command.repository_session)
    })
}

/// Only private, normally selected declarations authorize image reads. Validate
/// every row before any source factory opens an FD, including mixed legacy rows.
pub(super) fn observe_declared_image_paths(
    command: &NativeDemandCommand<'_>,
    declarations: &[DeclaredImageRequest],
    demands: Vec<PathObservationDemand>,
    active: &dyn Fn() -> bool,
) -> Result<PathObservationEpoch, NativeDemandSessionError> {
    preflight_declared_image_paths(command, declarations, &demands)?;
    if !active() {
        return Err(image_error("image preparation cancelled"));
    }
    let credit = image_batch_credit(command, &demands)?;
    let sources = retain_image_sources(command, declarations, &demands, Some(active))?;
    observe_authorized_image_paths(command, declarations, &sources, demands, credit, active)
}

pub(in crate::runtime::dice) fn image_batch_credit(
    command: &NativeDemandCommand<'_>,
    demands: &[PathObservationDemand],
) -> Result<slug_workspace_v2::ImageDescriptorReservation, NativeDemandSessionError> {
    let budget = command.runtime.image_resources.descriptor_budget();
    let row = std::mem::size_of::<(PathObservationDemand, Arc<PathObservationResult>)>();
    let charge = demands.iter().try_fold(
        512 + std::mem::size_of::<[Option<RetainedImageSource>; 3]>(),
        |total, demand| {
            demand
                .path()
                .as_path()
                .as_os_str()
                .len()
                .checked_add(row)
                .and_then(|n| n.checked_mul(16))
                .and_then(|n| total.checked_add(n))
                .ok_or_else(|| image_error("image batch descriptor overflow"))
        },
    )?;
    budget
        .try_reserve(charge)
        .map_err(|error| NativeDemandSessionError::InputImage(error.into()))
}

enum RetainedImageSource {
    Plain(input_image::ScopedImageSource),
    #[cfg(target_os = "linux")]
    Elf(input_image::elf_metadata::AuthorizedElfObservation),
}

impl RetainedImageSource {
    fn scoped(&self) -> &input_image::ScopedImageSource {
        match self {
            Self::Plain(source) => source,
            #[cfg(target_os = "linux")]
            Self::Elf(authorized) => authorized.source(),
        }
    }
}

fn retain_image_sources(
    command: &NativeDemandCommand<'_>,
    declarations: &[DeclaredImageRequest],
    demands: &[PathObservationDemand],
    active: Option<&dyn Fn() -> bool>,
) -> Result<[Option<RetainedImageSource>; 3], NativeDemandSessionError> {
    let mut sources = std::array::from_fn(|_| None);
    for (index, request) in declarations.iter().enumerate() {
        let selected = |demand: &&PathObservationDemand| {
            declarations
                .iter()
                .position(|candidate| candidate.allows(demand))
                == Some(index)
        };
        if demands.iter().find(selected).is_none() {
            continue;
        }
        if active.is_some_and(|active| !active()) {
            return Err(image_error("image preparation cancelled"));
        }
        let source = request.source(command)?;
        let has_elf = demands.iter().filter(selected).any(|demand| {
            matches!(demand.operation(), PathObservationOperation::InputImage(scope)
                if matches!(scope.operation(), ImageObservationOperation::BoundedElfMetadata { .. }))
        });
        sources[index] = Some(if has_elf {
            let elf = request.elf.as_ref().expect("authorized ELF declaration");
            #[cfg(target_os = "linux")]
            {
                RetainedImageSource::Elf(
                    input_image::elf_metadata::AuthorizedElfObservation::new(
                        source,
                        elf.certificate.clone(),
                        elf.file.clone(),
                        elf.policy,
                        &command.runtime.image_resources,
                    )
                    .map_err(NativeDemandSessionError::InputImage)?,
                )
            }
            #[cfg(not(target_os = "linux"))]
            {
                let _ = (source, elf);
                return Err(NativeDemandSessionError::InputImage(
                    input_image::InputImageCaptureError::UnsupportedPlatform,
                ));
            }
        } else {
            RetainedImageSource::Plain(source)
        });
    }
    Ok(sources)
}

fn observe_authorized_image_paths(
    command: &NativeDemandCommand<'_>,
    declarations: &[DeclaredImageRequest],
    sources: &[Option<RetainedImageSource>; 3],
    demands: Vec<PathObservationDemand>,
    _credit: slug_workspace_v2::ImageDescriptorReservation,
    active: &dyn Fn() -> bool,
) -> Result<PathObservationEpoch, NativeDemandSessionError> {
    let budget = command.runtime.image_resources.descriptor_budget();
    observe_image_rows(command, demands, _credit, active, |demand| {
        let PathObservationOperation::InputImage(scope) = demand.operation() else {
            unreachable!()
        };
        let index = declarations
            .iter()
            .position(|request| request.allows(demand))
            .expect("whole batch authorized");
        let source = sources[index].as_ref().expect("retained authorized source");
        let result = if matches!(
            scope.operation(),
            ImageObservationOperation::BoundedElfMetadata { .. }
        ) {
            observe_declared_elf(command, source, demand, active)?
        } else {
            source
                .scoped()
                .observe(demand, budget, active)
                .map_err(NativeDemandSessionError::InputImage)?
        };
        Ok(result)
    })
}

pub(in crate::runtime::dice) fn observe_image_rows(
    command: &NativeDemandCommand<'_>,
    demands: Vec<PathObservationDemand>,
    _credit: slug_workspace_v2::ImageDescriptorReservation,
    active: &dyn Fn() -> bool,
    mut observe_image: impl FnMut(
        &PathObservationDemand,
    ) -> Result<PathObservationResult, NativeDemandSessionError>,
) -> Result<PathObservationEpoch, NativeDemandSessionError> {
    let mut observations = Vec::with_capacity(demands.len());
    let mut legacy = Vec::new();
    for demand in demands {
        if !active() {
            return Err(image_error("image preparation cancelled"));
        }
        if matches!(demand.operation(), PathObservationOperation::InputImage(_)) {
            let result = observe_image(&demand)?;
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
    if !active() {
        return Err(image_error("image preparation cancelled"));
    }
    PathObservationEpoch::from_shared(observations).map_err(NativeDemandSessionError::PathEpoch)
}

pub(super) fn preflight_declared_image_paths(
    command: &NativeDemandCommand<'_>,
    declarations: &[DeclaredImageRequest],
    demands: &[PathObservationDemand],
) -> Result<(), NativeDemandSessionError> {
    if declarations.len() > 3 {
        return Err(image_error("invalid image declaration batch"));
    }
    let budget = command.runtime.image_resources.descriptor_budget();
    for declaration in declarations {
        if declaration
            .elf
            .as_ref()
            .is_some_and(|elf| !elf.same_budget(budget))
        {
            return Err(image_error("foreign image declaration"));
        }
        if let Some(DeclaredImageRepository::Selected(selected)) = &declaration.repository {
            label_source::check_declaration(command.runtime, declaration, selected)?;
        }
    }
    for demand in demands {
        let PathObservationOperation::InputImage(scope) = demand.operation() else {
            continue;
        };
        if !scope.same_budget(budget)
            || !demand
                .image_path()
                .is_some_and(|path| path.same_budget(budget))
        {
            return Err(image_error("foreign image observation"));
        }
        let mut matching = declarations.iter().filter(|request| request.allows(demand));
        let first = matching
            .next()
            .ok_or_else(|| image_error("undeclared image observation"))?;
        if matching.any(|other| {
            other.key != first.key
                || other
                    .repository
                    .as_ref()
                    .map(DeclaredImageRepository::selection)
                    != first
                        .repository
                        .as_ref()
                        .map(DeclaredImageRepository::selection)
        }) {
            return Err(image_error("ambiguous image observation"));
        }
    }
    Ok(())
}

pub(super) fn declared_selected_image(
    runtime: &WorkspaceRuntime,
    selected: slug_bzlmod_v2::ObservedSelectedLabelSource,
    deadline: std::time::Instant,
) -> Result<DeclaredImageRequest, NativeDemandSessionError> {
    label_source::check_selection(runtime, &selected)?;
    let repository = selected
        .repository()
        .map(|_| DeclaredImageRepository::Selected(selected.clone()));
    Ok(DeclaredImageRequest {
        key: selected.image_key().clone(),
        elf: None,
        deadline,
        repository,
    })
}

pub(super) fn declare_selected_elf(
    runtime: &WorkspaceRuntime,
    declaration: &mut DeclaredImageRequest,
    selected: &slug_bzlmod_v2::ObservedSelectedLabelSource,
    certificate: ObservedInputImage,
    policy: ElfMetadataPolicy,
) -> Result<(), NativeDemandSessionError> {
    label_source::check_declaration(runtime, declaration, selected)?;
    label_source::check_member(runtime, selected, &certificate)?;
    declaration.elf = Some(DeclaredElfRequest {
        certificate,
        file: selected.selected_file().clone(),
        policy,
    });
    Ok(())
}

pub(super) fn capture_selected_image(
    command: &NativeDemandCommand<'_>,
    declaration: &DeclaredImageRequest,
    certificate: ObservedInputImage,
    stage: input_image::PreparedInputImageStage,
    active: &dyn Fn() -> bool,
) -> Result<CapturedInputImageLease, NativeDemandSessionError> {
    if certificate.namespace() != declaration.key.namespace()
        || certificate.anchor_root() != declaration.key.anchor_root()
        || certificate.source_root() != declaration.key.source_root()
        || certificate.policy() != declaration.key.policy()
    {
        return Err(image_error("selected capture certificate changed"));
    }
    input_image::capture_prepared_input_image(
        input_image::AuthorizedObservedInputImage::new(declaration.source(command)?, certificate),
        &command.runtime.image_resources,
        active,
        stage,
    )
    .map_err(NativeDemandSessionError::InputImage)
}

pub(super) fn capture_selected_image_retaining(
    command: &NativeDemandCommand<'_>,
    declaration: &DeclaredImageRequest,
    certificate: ObservedInputImage,
    stage: &mut input_image::PreparedInputImageStage,
    active: &dyn Fn() -> bool,
) -> Result<CapturedInputImageLease, NativeDemandSessionError> {
    if certificate.namespace() != declaration.key.namespace()
        || certificate.anchor_root() != declaration.key.anchor_root()
        || certificate.source_root() != declaration.key.source_root()
        || certificate.policy() != declaration.key.policy()
    {
        return Err(image_error("selected capture certificate changed"));
    }
    input_image::capture_prepared_input_image_retaining(
        input_image::AuthorizedObservedInputImage::new(declaration.source(command)?, certificate),
        &command.runtime.image_resources,
        active,
        stage,
    )
    .map_err(NativeDemandSessionError::InputImage)
}

fn image_error(message: &'static str) -> NativeDemandSessionError {
    NativeDemandSessionError::Computation(anyhow::anyhow!(message))
}

fn observe_declared_elf(
    command: &NativeDemandCommand<'_>,
    source: &RetainedImageSource,
    demand: &PathObservationDemand,
    active: &dyn Fn() -> bool,
) -> Result<PathObservationResult, NativeDemandSessionError> {
    #[cfg(target_os = "linux")]
    {
        let RetainedImageSource::Elf(authorized) = source else {
            return Err(image_error("missing ELF declaration"));
        };
        input_image::elf_metadata::observe_elf_metadata(
            authorized,
            demand,
            &command.runtime.image_resources,
            active,
        )
        .map_err(NativeDemandSessionError::InputImage)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (command, source, demand, active);
        Err(NativeDemandSessionError::InputImage(
            input_image::InputImageCaptureError::UnsupportedPlatform,
        ))
    }
}

#[path = "input_image_bridge/prepare.rs"]
mod prepare;

#[path = "input_image_bridge/label_source.rs"]
pub(in crate::runtime::dice) mod label_source;

#[cfg(all(test, target_os = "linux"))]
#[path = "input_image_bridge/label_tests.rs"]
mod label_tests;

#[cfg(all(test, target_os = "linux"))]
#[path = "input_image_bridge/tests.rs"]
mod tests;

#[cfg(all(test, target_os = "linux"))]
#[path = "input_image_bridge/elf_tests.rs"]
mod elf_tests;
