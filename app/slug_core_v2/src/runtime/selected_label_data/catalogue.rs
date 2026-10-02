//! Exact-file admission. An anchor alone never grants its other contents.
use slug_workspace_v2::ImageDescriptorReservation;
use slug_workspace_v2::ImageObservationOperation;
use slug_workspace_v2::InputImageEntryKind;
use slug_workspace_v2::ObservedInputImageKey;

use super::*;

pub(in crate::runtime::dice) struct Catalogue {
    key: RuntimeSourceProfileKey,
    plan: ObservedSelectedDataLabelRequests,
    reachable: SelectedWorkspaceDemands,
    deadline: std::time::Instant,
    _credit: ImageDescriptorReservation,
}
impl Catalogue {
    pub(in crate::runtime::dice) fn new(
        runtime: &WorkspaceRuntime,
        key: &RuntimeSourceProfileKey,
        plan: &ObservedSelectedDataLabelRequests,
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
        let mut matching = self
            .plan
            .requests()
            .filter(|row| allows(row.leaf_image_key(), demand));
        let Some(first) = matching.next() else {
            return Ok(None);
        };
        if matching.any(|row| {
            row.selection().repository() != first.selection().repository()
                || row.leaf_image_key().namespace() != first.leaf_image_key().namespace()
                || row.leaf_image_key().anchor_root() != first.leaf_image_key().anchor_root()
        }) {
            return Err(refusal("ambiguous data Label authority"));
        }
        // Data-first routing is permitted only for the SAME actual authority.
        // Common ancestor rows can also belong to an existing full tree.
        for row in self.plan.runtime_requests().requests() {
            if same_scope(row.image_key(), first.leaf_image_key())
                && row.selected_label_source().and_then(|s| s.repository())
                    != first.selection().repository()
            {
                return Err(refusal("conflicting runtime/data source authority"));
            }
        }
        for row in self.plan.runtime_requests().tool_requests().requests() {
            if let Some(selected) = row.label_selection() {
                if same_scope(selected.image_key(), first.leaf_image_key())
                    && selected.repository() != first.selection().repository()
                {
                    return Err(refusal("conflicting tool/data source authority"));
                }
            }
        }
        Ok(Some(first.occurrence_index()))
    }
    pub(in crate::runtime::dice) fn observes(
        &self,
        demand: &PathObservationDemand,
    ) -> Result<bool, NativeDemandSessionError> {
        Ok(self.choose(demand)?.is_some())
    }
    pub(in crate::runtime::dice) fn preflight(
        &self,
        command: &NativeDemandCommand<'_>,
        demands: &[PathObservationDemand],
    ) -> Result<(), NativeDemandSessionError> {
        validate_requests(
            command,
            &self.key,
            &self.plan,
            &self.reachable,
            self.deadline,
        )?;
        let budget = command.runtime.image_resources.descriptor_budget();
        for demand in demands {
            let PathObservationOperation::InputImage(scope) = demand.operation() else {
                return Err(refusal("non-image data observation"));
            };
            if !scope.same_budget(budget)
                || !demand.image_path().is_some_and(|p| p.same_budget(budget))
                || self.choose(demand)?.is_none()
            {
                return Err(refusal("undeclared data Label observation"));
            }
        }
        Ok(())
    }
    fn source(
        &self,
        command: &NativeDemandCommand<'_>,
        index: usize,
    ) -> Result<input_image::ScopedImageSource, NativeDemandSessionError> {
        validate_requests(
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
            .ok_or_else(|| refusal("unknown data Label occurrence"))?;
        let source = command
            .runtime
            .repository_materializer
            .image_source(
                command.repository_session,
                row.leaf_image_key().namespace(),
                row.leaf_image_key().anchor_root(),
                row.selection().repository(),
            )
            .map_err(NativeDemandSessionError::Repository)?;
        validate_requests(
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
        let budget = command.runtime.image_resources.descriptor_budget();
        let slots = budget
            .try_reserve(std::mem::size_of::<[Option<input_image::ScopedImageSource>; 64]>() + 256)
            .map_err(image_error)?;
        // Rows/conversion peaks are reserved before the first source FD, too.
        let row_credit = input_image_bridge::image_batch_credit(command, &demands)?;
        let mut sources: [Option<input_image::ScopedImageSource>; 64] =
            std::array::from_fn(|_| None);
        let active = || selected_scratch_source::active(command, self.deadline).is_ok();
        for demand in &demands {
            let index = self.choose(demand)?.expect("preflighted data row");
            if sources[index].is_none() {
                sources[index] = Some(self.source(command, index)?);
            }
        }
        let epoch = input_image_bridge::observe_image_rows(
            command,
            demands,
            row_credit,
            &active,
            |demand| {
                let index = self.choose(demand)?.expect("preflighted data row");
                sources[index]
                    .as_ref()
                    .expect("retained data source")
                    .observe(demand, budget, &active)
                    .map_err(NativeDemandSessionError::InputImage)
            },
        );
        drop(sources);
        drop(slots);
        epoch
    }
    pub(in crate::runtime::dice) fn capture(
        &self,
        command: &NativeDemandCommand<'_>,
        index: usize,
        image: ObservedInputImage,
        stage: input_image::PreparedInputImageStage,
    ) -> Result<input_image::CapturedInputFileLease, NativeDemandSessionError> {
        let row = self
            .plan
            .requests()
            .nth(index)
            .ok_or_else(|| refusal("unknown data Label capture"))?;
        check_image(row.leaf_image_key(), &image)?;
        let active = || selected_scratch_source::active(command, self.deadline).is_ok();
        input_image::capture_prepared_input_file(
            input_image::AuthorizedObservedInputImage::new(self.source(command, index)?, image),
            &command.runtime.image_resources,
            &active,
            stage,
        )
        .map_err(NativeDemandSessionError::InputImage)
    }
}
pub(in crate::runtime::dice) fn check_image(
    key: &ObservedInputImageKey,
    image: &ObservedInputImage,
) -> Result<(), NativeDemandSessionError> {
    if !key.is_regular_file_root()
        || !image.is_regular_file_root()
        || image.namespace() != key.namespace()
        || image.anchor_root() != key.anchor_root()
        || image.source_root() != key.source_root()
        || image.policy() != key.policy()
        || image.entries().len() != 1
        || image.entries()[0].path().as_path() != key.source_root().as_path()
        || !matches!(
            image.entries()[0].kind(),
            InputImageEntryKind::RegularFile(_)
        )
    {
        return Err(refusal("foreign data Label file certificate"));
    }
    Ok(())
}
fn allows(key: &ObservedInputImageKey, demand: &PathObservationDemand) -> bool {
    let PathObservationOperation::InputImage(scope) = demand.operation() else {
        return false;
    };
    let path = demand.path().as_path();
    if demand.namespace() != key.namespace()
        || scope.scope_anchor_root() != key.anchor_root()
        || !path.starts_with(key.anchor_root().as_path())
    {
        return false;
    }
    match scope.operation() {
        ImageObservationOperation::Lstat => key.source_root().as_path().starts_with(path),
        ImageObservationOperation::BoundedDigest { max_bytes } => {
            path == key.source_root().as_path() && max_bytes == key.policy().limits().max_file_bytes
        }
        _ => false,
    }
}

fn same_scope(a: &ObservedInputImageKey, b: &ObservedInputImageKey) -> bool {
    a.namespace() == b.namespace() && a.anchor_root() == b.anchor_root()
}
