//! Only current normal requests can consume raw compact scratch proof cells.
use allocative::Allocative;
use slug_bzlmod_v2::SelectedScratchElfMetadataEpoch;
use slug_bzlmod_v2::SelectedScratchElfMetadataEpochKey;
use slug_bzlmod_v2::SelectedScratchElfMetadataNeeds;
use slug_bzlmod_v2::SelectedScratchElfMetadataRequest;
use slug_bzlmod_v2::SelectedScratchElfMetadataResult;
use slug_bzlmod_v2::SelectedScratchElfMetadataTransaction;
use slug_bzlmod_v2::SelectedScratchResources;
use slug_workspace_v2::BudgetedImageBlock;

use super::*;

#[derive(Allocative)]
struct StateInner {
    selector: slug_bzlmod_v2::SelectedScratchElfMetadataSelector,
    allowed: Option<SelectedScratchElfMetadataNeeds>,
    ready: [Option<SelectedScratchElfMetadataResult>; 3],
    issued: [Option<SelectedScratchElfMetadataEpochKey>; 3],
    resources: SelectedScratchResources,
}
#[derive(Clone, Dupe)]
pub(in crate::runtime::dice) struct ScratchMetadataState(BudgetedImageBlock<StateInner>);
impl ScratchMetadataState {
    pub(in crate::runtime::dice) fn new(
        plan: &ObservedSelectedExtensionToolSourceRequests,
        resources: &SelectedScratchResources,
    ) -> Result<Self, NativeDemandSessionError> {
        Self::for_plan(plan, resources, None)
    }
    pub(in crate::runtime::dice) fn for_plan(
        plan: &ObservedSelectedExtensionToolSourceRequests,
        resources: &SelectedScratchResources,
        previous: Option<&Self>,
    ) -> Result<Self, NativeDemandSessionError> {
        if !plan.same_resources(resources) {
            return Err(refusal("foreign tool request plan"));
        }
        let credit = resources
            .descriptor_budget()
            .try_reserve(std::mem::size_of::<StateInner>() + 256)
            .map_err(image_failure)?;
        let mut requests: [Option<SelectedScratchElfMetadataRequest>; 3] =
            std::array::from_fn(|_| None);
        let mut count = 0;
        for request in plan
            .requests()
            .filter_map(|request| request.scratch_request())
        {
            requests[count] = Some(request.dupe());
            count += 1;
        }
        let allowed = if count == 0 {
            None
        } else {
            // Inline handles only; the constructor owns the credited fixed batch.
            let compact: [&SelectedScratchElfMetadataRequest; 3] =
                std::array::from_fn(|i| requests[i.min(count - 1)].as_ref().unwrap());
            // Needs accepts owned cheap handles. No executable bytes are copied.
            let owned: [SelectedScratchElfMetadataRequest; 3] =
                std::array::from_fn(|i| compact[i].dupe());
            Some(
                SelectedScratchElfMetadataNeeds::new(&owned[..count], resources)
                    .map_err(shared_failure)?,
            )
        };
        let mut inner = StateInner {
            selector: plan.selector().dupe(),
            allowed,
            ready: std::array::from_fn(|_| None),
            issued: std::array::from_fn(|_| None),
            resources: resources.dupe(),
        };
        if let Some(previous) = previous {
            if !previous.0.get().resources.same_instance(resources)
                || previous.0.get().selector != inner.selector
            {
                return Err(refusal("foreign scratch proof state"));
            }
            inner.issued = previous.0.get().issued.clone();
        }
        for (index, request) in requests[..count].iter().flatten().enumerate() {
            let key = SelectedScratchElfMetadataEpochKey::new(request);
            if !inner.issued.iter().any(|cell| cell.as_ref() == Some(&key)) {
                let slot = inner
                    .issued
                    .iter_mut()
                    .find(|cell| cell.is_none())
                    .ok_or_else(|| refusal("too many scratch proof cells"))?;
                *slot = Some(key);
            }
            inner.ready[index] = previous
                .and_then(|state| {
                    state
                        .0
                        .get()
                        .ready
                        .iter()
                        .flatten()
                        .find(|result| result.request() == request)
                })
                .map(Dupe::dupe);
        }
        Ok(Self(credit.freeze(inner).map_err(image_failure)?))
    }
    pub(in crate::runtime::dice) fn transaction(
        &self,
    ) -> Result<SelectedScratchElfMetadataTransaction, NativeDemandSessionError> {
        let inner = self.0.get();
        let first = inner.ready.iter().flatten().next();
        let Some(first) = first else {
            return SelectedScratchElfMetadataTransaction::new(
                &inner.selector,
                &inner.resources,
                inner.allowed.as_ref(),
                &[],
            )
            .map_err(shared_failure);
        };
        let mut ready: [SelectedScratchElfMetadataResult; 3] =
            std::array::from_fn(|_| first.dupe());
        let mut length = 0;
        for result in inner.ready.iter().flatten() {
            ready[length] = result.dupe();
            length += 1;
        }
        SelectedScratchElfMetadataTransaction::new(
            &inner.selector,
            &inner.resources,
            inner.allowed.as_ref(),
            &ready[..length],
        )
        .map_err(shared_failure)
    }
    pub(in crate::runtime::dice) fn install(
        &self,
        data: &mut UserComputationData,
    ) -> Result<(), NativeDemandSessionError> {
        data.data.set(self.transaction()?);
        Ok(())
    }
    pub(in crate::runtime::dice) fn inject(
        &self,
        updater: &mut DiceTransactionUpdater,
        revoked: bool,
    ) -> Result<(), NativeDemandSessionError> {
        let transaction = self.transaction()?;
        for key in self.0.get().issued.iter().flatten() {
            let value = if revoked {
                SelectedScratchElfMetadataEpoch::Unauthorized
            } else {
                transaction.epoch(key)
            };
            updater
                .changed_to(std::iter::once((key.dupe(), value)))
                .map_err(|error| NativeDemandSessionError::Injection(error.into()))?;
        }
        Ok(())
    }
    pub(in crate::runtime::dice) fn preflight(
        &self,
        plan: &ObservedSelectedExtensionToolSourceRequests,
        need: &SelectedScratchElfMetadataNeeds,
    ) -> Result<(), NativeDemandSessionError> {
        let resources = &self.0.get().resources;
        let transaction = self.transaction()?;
        if !need.same_resources(resources) || !transaction.authorizes_selector(plan.selector()) {
            return Err(refusal("foreign scratch ELF batch"));
        }
        // Every full request is checked before the first parser read.
        for request in need.requests() {
            if !transaction.is_authorized(request)
                || !plan
                    .requests()
                    .any(|current| current.scratch_request() == Some(request))
                || !request.same_resources(resources)
            {
                return Err(refusal("undeclared scratch ELF request"));
            }
        }
        Ok(())
    }
    pub(in crate::runtime::dice) fn fulfill(
        &self,
        plan: &ObservedSelectedExtensionToolSourceRequests,
        need: &SelectedScratchElfMetadataNeeds,
        active: &dyn Fn() -> bool,
    ) -> Result<Self, NativeDemandSessionError> {
        let resources = &self.0.get().resources;
        self.preflight(plan, need)?;
        let transaction = self.transaction()?;
        let credit = resources
            .descriptor_budget()
            .try_reserve(std::mem::size_of::<StateInner>() + 256)
            .map_err(image_failure)?;
        let mut inner = StateInner {
            selector: self.0.get().selector.dupe(),
            allowed: self.0.get().allowed.dupe(),
            ready: self.0.get().ready.clone(),
            issued: self.0.get().issued.clone(),
            resources: resources.dupe(),
        };
        let mut progressed = false;
        for request in need.requests() {
            if transaction.get(request).is_some() {
                continue;
            }
            #[cfg(target_os = "linux")]
            let verified = crate::runtime::input_image::elf_metadata::authenticate_elf_bytes(
                request.blob().bytes(),
                request.expected_digest(),
                request.policy(),
                resources.descriptor_budget(),
                active,
            )
            .map_err(NativeDemandSessionError::InputImage)?;
            #[cfg(not(target_os = "linux"))]
            let verified: slug_workspace_v2::VerifiedElfMetadata =
                return Err(NativeDemandSessionError::InputImage(
                    input_image::InputImageCaptureError::UnsupportedPlatform,
                ));
            let result = SelectedScratchElfMetadataResult::new(request, verified, resources)
                .map_err(shared_failure)?;
            let slot = inner
                .ready
                .iter_mut()
                .find(|entry| {
                    entry.is_none()
                        || entry
                            .as_ref()
                            .is_some_and(|old| old.request().operand() == request.operand())
                })
                .ok_or_else(|| refusal("too many scratch results"))?;
            *slot = Some(result);
            progressed = true;
        }
        if !progressed {
            return Err(refusal("scratch ELF made no progress"));
        }
        if !active() {
            return Err(NativeDemandSessionError::InputImage(
                input_image::InputImageCaptureError::Cancelled,
            ));
        }
        Ok(Self(credit.freeze(inner).map_err(image_failure)?))
    }
}
