//! A separate64-scope owner; ordinary tool admission remains limited to3.
use slug_workspace_v2::ImageDescriptorReservation;
use slug_workspace_v2::ImageObservationOperation;

use super::*;

pub(in crate::runtime::dice) struct Catalogue {
    key: RuntimeSourceProfileKey,
    plan: ObservedRuntimeSourceRequests,
    reachable: SelectedWorkspaceDemands,
    deadline: std::time::Instant,
    _credit: ImageDescriptorReservation,
}
impl Catalogue {
    pub(in crate::runtime::dice) fn new(
        runtime: &WorkspaceRuntime,
        key: &RuntimeSourceProfileKey,
        plan: &ObservedRuntimeSourceRequests,
        reachable: &SelectedWorkspaceDemands,
        deadline: std::time::Instant,
    ) -> Result<Self, NativeDemandSessionError> {
        let credit = runtime
            .image_resources
            .descriptor_budget()
            .try_reserve(std::mem::size_of::<Self>() + 256)
            .map_err(image_error)?;
        Ok(Self {
            key: key.clone(),
            plan: plan.dupe(),
            reachable: reachable.clone(),
            deadline,
            _credit: credit,
        })
    }
    fn choose(
        &self,
        demand: &PathObservationDemand,
    ) -> Result<Option<usize>, NativeDemandSessionError> {
        let mut matches = self
            .plan
            .requests()
            .enumerate()
            .filter(|(_, row)| allows(row.image_key(), demand));
        let Some((index, first)) = matches.next() else {
            return Ok(None);
        };
        if matches.any(|(_, other)| {
            other.image_key().namespace() != first.image_key().namespace()
                || other.image_key().anchor_root() != first.image_key().anchor_root()
                || other.selected_label_source().and_then(|s| s.repository())
                    != first.selected_label_source().and_then(|s| s.repository())
        }) {
            return Err(refusal("ambiguous runtime image authority"));
        }
        Ok(Some(index))
    }
    pub(in crate::runtime::dice) fn preflight(
        &self,
        command: &NativeDemandCommand<'_>,
        demands: &[PathObservationDemand],
    ) -> Result<(), NativeDemandSessionError> {
        if self.plan.requests().len() > 64 {
            return Err(refusal("too many runtime declarations"));
        }
        for row in self.plan.requests() {
            factory::preflight(command, &self.plan, row)?;
        }
        let budget = command.runtime.image_resources.descriptor_budget();
        for demand in demands {
            if !matches!(demand.operation(), PathObservationOperation::InputImage(_)) {
                continue;
            }
            let PathObservationOperation::InputImage(scope) = demand.operation() else {
                unreachable!()
            };
            if !scope.same_budget(budget)
                || !demand.image_path().is_some_and(|p| p.same_budget(budget))
            {
                return Err(refusal("foreign runtime image observation"));
            }
            if self.choose(demand)?.is_none() {
                return Err(refusal("undeclared runtime image observation"));
            }
        }
        Ok(())
    }
    pub(in crate::runtime::dice) fn is_runtime(
        &self,
        demand: &PathObservationDemand,
    ) -> Result<bool, NativeDemandSessionError> {
        Ok(self.choose(demand)?.is_some())
    }
    fn source(
        &self,
        command: &NativeDemandCommand<'_>,
        index: usize,
        active: &dyn Fn() -> bool,
    ) -> Result<input_image::ScopedImageSource, NativeDemandSessionError> {
        validate_plan(
            command,
            &self.key,
            &self.plan,
            &self.reachable,
            self.deadline,
        )?;
        let row = self
            .plan
            .requests()
            .nth(index)
            .expect("selected runtime declaration");
        let source = factory::source(command, &self.plan, row, active)?;
        validate_plan(
            command,
            &self.key,
            &self.plan,
            &self.reachable,
            self.deadline,
        )?;
        Ok(source)
    }
    pub(in crate::runtime::dice) fn observe(
        &self,
        command: &NativeDemandCommand<'_>,
        demands: Vec<PathObservationDemand>,
    ) -> Result<PathObservationEpoch, NativeDemandSessionError> {
        self.preflight(command, &demands)?;
        let active = || selected_scratch_source::active(command, self.deadline).is_ok();
        if !active() {
            return Err(refusal("runtime source preparation cancelled"));
        }
        let slot_credit = command
            .runtime
            .image_resources
            .descriptor_budget()
            .try_reserve(std::mem::size_of::<[Option<input_image::ScopedImageSource>; 64]>() + 256)
            .map_err(image_error)?;
        let row_credit = input_image_bridge::image_batch_credit(command, &demands)?;
        let mut sources: [Option<input_image::ScopedImageSource>; 64] =
            std::array::from_fn(|_| None);
        for demand in &demands {
            let index = self.choose(demand)?.expect("preflighted runtime row");
            if sources[index].is_none() {
                sources[index] = Some(self.source(command, index, &active)?);
            }
        }
        let epoch = input_image_bridge::observe_image_rows(
            command,
            demands,
            row_credit,
            &active,
            |demand| {
                let index = self.choose(demand)?.expect("preflighted runtime row");
                sources[index]
                    .as_ref()
                    .expect("retained current source")
                    .observe(
                        demand,
                        command.runtime.image_resources.descriptor_budget(),
                        &active,
                    )
                    .map_err(NativeDemandSessionError::InputImage)
            },
        );
        drop(sources);
        drop(slot_credit);
        epoch
    }
    pub(in crate::runtime::dice) fn capture(
        &self,
        command: &NativeDemandCommand<'_>,
        index: usize,
        image: ObservedInputImage,
        stage: input_image::PreparedInputImageStage,
    ) -> Result<input_image::CapturedInputImageLease, NativeDemandSessionError> {
        let row = self
            .plan
            .requests()
            .nth(index)
            .ok_or_else(|| refusal("unknown runtime capture"))?;
        if image.namespace() != row.image_key().namespace()
            || image.anchor_root() != row.image_key().anchor_root()
            || image.source_root() != row.image_key().source_root()
            || image.policy() != row.image_key().policy()
        {
            return Err(refusal("foreign runtime capture certificate"));
        }
        let active = || selected_scratch_source::active(command, self.deadline).is_ok();
        input_image::capture_prepared_input_image(
            input_image::AuthorizedObservedInputImage::new(
                self.source(command, index, &active)?,
                image,
            ),
            &command.runtime.image_resources,
            &active,
            stage,
        )
        .map_err(NativeDemandSessionError::InputImage)
    }
}
fn allows(key: &slug_workspace_v2::ObservedInputImageKey, demand: &PathObservationDemand) -> bool {
    let PathObservationOperation::InputImage(scope) = demand.operation() else {
        return false;
    };
    let path = demand.path().as_path();
    if demand.namespace() != key.namespace()
        || scope.scope_anchor_root() != key.anchor_root()
        || !path.starts_with(key.anchor_root().as_path())
        || !(path.starts_with(key.source_root().as_path())
            || key.source_root().as_path().starts_with(path))
    {
        return false;
    }
    // Traversal observes ancestor identity only. Contents outside the selected
    // subtree never become readable through its declared anchor.
    if !path.starts_with(key.source_root().as_path()) {
        return matches!(scope.operation(), ImageObservationOperation::Lstat);
    }
    let limits = key.policy().limits();
    match scope.operation() {
        ImageObservationOperation::Lstat => true,
        ImageObservationOperation::BoundedDigest { max_bytes } => {
            max_bytes == limits.max_file_bytes
        }
        ImageObservationOperation::BoundedDirectory(bounds) => {
            bounds.max_entries <= limits.max_entries
                && bounds.max_name_bytes_total <= limits.max_descriptor_bytes
                && bounds.max_component_bytes <= limits.max_path_bytes
        }
        ImageObservationOperation::BoundedReadLink { .. }
        | ImageObservationOperation::BoundedElfMetadata { .. } => false,
    }
}
