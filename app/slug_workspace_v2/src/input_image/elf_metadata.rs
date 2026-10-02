//! Normal DICE ELF facts derived from an exact regular member of an observed image.
//! This producer performs no IO and grants no native source authority.

use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;

use super::BudgetedImageBlock;
use super::ImageChargedPath;
use super::ImageDescriptorBudget;
use super::ImageDescriptorBudgetError;
use super::ImageObservationData;
use super::ImageObservationOperation;
use super::ImageObservationRequest;
use super::InputImageError;
use super::ObservedInputImage;
use super::ObservedInputImageKey;
use crate::NeedPathObservations;
use crate::PathObservationDemand;
use crate::PathObservationEpoch;
use crate::PathObservationKey;
use crate::PathObservationResult;
use crate::PathOperationResult;
use crate::PathOutcome;

mod types;
pub use types::*;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
pub enum ElfMetadataError {
    Budget(ImageDescriptorBudgetError),
    MissingBudget,
    Compute,
    InvalidScope,
    InvalidPolicy,
    Image(InputImageError),
    MissingMember,
    NonRegularMember,
    ObservationMismatch,
    StaleSource,
}
impl ElfMetadataError {
    pub fn is_transient(&self) -> bool {
        match self {
            Self::Budget(_)
            | Self::MissingBudget
            | Self::Compute
            | Self::ObservationMismatch
            | Self::StaleSource => true,
            Self::Image(error) => error.is_transient(),
            _ => false,
        }
    }
}
impl From<ImageDescriptorBudgetError> for ElfMetadataError {
    fn from(error: ImageDescriptorBudgetError) -> Self {
        Self::Budget(error)
    }
}
impl fmt::Display for ElfMetadataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed ELF metadata: {self:?}")
    }
}
impl std::error::Error for ElfMetadataError {}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub struct ObservedElfMetadataKey {
    image: ObservedInputImageKey,
    file: ImageChargedPath,
    policy: ElfMetadataPolicy,
}
impl ObservedElfMetadataKey {
    pub fn from_charged(
        image: ObservedInputImageKey,
        file: ImageChargedPath,
        policy: ElfMetadataPolicy,
    ) -> Result<Self, ElfMetadataError> {
        if !file.same_budget(image.anchor_root.budget()) {
            return Err(ImageDescriptorBudgetError::ForeignBudget.into());
        }
        if !file.as_path().starts_with(image.source_root().as_path()) {
            return Err(ElfMetadataError::InvalidScope);
        }
        Ok(Self {
            image,
            file,
            policy,
        })
    }
    pub fn image(&self) -> &ObservedInputImageKey {
        &self.image
    }
    pub fn file(&self) -> &ImageChargedPath {
        &self.file
    }
    pub fn policy(&self) -> ElfMetadataPolicy {
        self.policy
    }
}
impl fmt::Display for ObservedElfMetadataKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "observed-elf-metadata:{}:{}:{:?}",
            self.image,
            self.file.normalized_ref(),
            self.policy
        )
    }
}

#[derive(Debug, PartialEq, Eq, Allocative)]
struct ObservedInner {
    image: ObservedInputImage,
    file: ImageChargedPath,
    policy: ElfMetadataPolicy,
    verified: VerifiedElfMetadata,
    observations: PathObservationEpoch,
}
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedElfMetadata(BudgetedImageBlock<ObservedInner>);
impl ObservedElfMetadata {
    pub fn image(&self) -> &ObservedInputImage {
        &self.0.get().image
    }
    pub fn file(&self) -> &ImageChargedPath {
        &self.0.get().file
    }
    pub fn policy(&self) -> ElfMetadataPolicy {
        self.0.get().policy
    }
    pub fn verified(&self) -> &VerifiedElfMetadata {
        &self.0.get().verified
    }
    pub fn observations(&self) -> &PathObservationEpoch {
        &self.0.get().observations
    }
    pub fn same_budget(&self, budget: &ImageDescriptorBudget) -> bool {
        self.0.same_budget(budget)
            && self.image().same_budget(budget)
            && self.file().same_budget(budget)
            && self.verified().same_budget(budget)
            && self
                .observations()
                .observations()
                .iter()
                .all(|(demand, result)| {
                    demand
                        .image_path()
                        .is_some_and(|path| path.same_budget(budget))
                        && match result.as_ref() {
                            PathObservationResult::InputImage(value) => value.same_budget(budget),
                            _ => false,
                        }
                })
    }
    pub fn content(&self) -> ElfMetadataContent {
        ElfMetadataContent(self.dupe())
    }
}
#[derive(Debug, Clone, Allocative, Dupe)]
pub struct ElfMetadataContent(ObservedElfMetadata);
impl PartialEq for ElfMetadataContent {
    fn eq(&self, other: &Self) -> bool {
        self.0.image().content() == other.0.image().content()
            && self.0.file() == other.0.file()
            && self.0.policy() == other.0.policy()
            && self.0.verified() == other.0.verified()
    }
}
impl Eq for ElfMetadataContent {}

type ElfValue = PathOutcome<Result<ObservedElfMetadata, ElfMetadataError>>;
fn failed(error: impl Into<ElfMetadataError>) -> ElfValue {
    PathOutcome::Complete(Err(error.into()))
}

#[async_trait]
impl Key for ObservedElfMetadataKey {
    type Value = ElfValue;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match compute(self, ctx).await {
            Ok(value) => value,
            Err(error) => failed(error),
        }
    }
    fn equality(a: &Self::Value, b: &Self::Value) -> bool {
        Self::validity(a) && Self::validity(b) && a.complete_eq(b)
    }
    fn validity(value: &Self::Value) -> bool {
        match value {
            PathOutcome::Complete(Ok(_)) => true,
            PathOutcome::Complete(Err(error)) => !error.is_transient(),
            PathOutcome::Need(_) => false,
        }
    }
}

async fn compute(
    key: &ObservedElfMetadataKey,
    ctx: &mut DiceComputations<'_>,
) -> Result<ElfValue, ElfMetadataError> {
    let budget = ctx
        .per_transaction_data()
        .data
        .get::<ImageDescriptorBudget>()
        .map_err(|_| ElfMetadataError::MissingBudget)?
        .dupe();
    if !key.file.same_budget(&budget)
        || !key.image.anchor_root.same_budget(&budget)
        || !key.image.source_root.same_budget(&budget)
    {
        return Err(ImageDescriptorBudgetError::ForeignBudget.into());
    }
    let image = match ctx
        .compute(&key.image)
        .await
        .map_err(|_| ElfMetadataError::Compute)?
    {
        PathOutcome::Need(need) => return Ok(PathOutcome::Need(need)),
        PathOutcome::Complete(Err(error)) => return Err(ElfMetadataError::Image(error)),
        PathOutcome::Complete(Ok(image)) => image,
    };
    if !image.same_budget(&budget) {
        return Err(ImageDescriptorBudgetError::ForeignBudget.into());
    }
    let entry = image
        .entries()
        .iter()
        .find(|entry| entry.path() == &key.file)
        .ok_or(ElfMetadataError::MissingMember)?;
    let digest = ElfSourceDigestBinding::from_digest(
        entry
            .file_digest()
            .ok_or(ElfMetadataError::NonRegularMember)?,
    );
    let request = ImageObservationRequest::new(
        key.image.anchor_root.dupe(),
        ImageObservationOperation::BoundedElfMetadata {
            expected_digest: digest,
            policy: key.policy,
        },
    );
    // Own new singleton/conversion storage before entering the raw key. Equal
    // interned raw keys may retain an older token, so never forward their Need.
    let need_bytes = std::mem::size_of::<PathObservationDemand>()
        .checked_mul(4)
        .and_then(|n| n.checked_add(128))
        .ok_or(ImageDescriptorBudgetError::ChargeOverflow)?;
    let path = key.file.retain_charge(budget.try_reserve(need_bytes)?)?;
    let demand = PathObservationDemand::input_image(key.image.namespace(), path, request.dupe())
        .map_err(|_| ElfMetadataError::ObservationMismatch)?;
    let raw = match ctx
        .compute(&PathObservationKey::new(demand.dupe()))
        .await
        .map_err(|_| ElfMetadataError::Compute)?
    {
        PathOutcome::Need(_) => {
            return Ok(PathOutcome::Need(NeedPathObservations::singleton(demand)));
        }
        PathOutcome::Complete(raw) => raw,
    };
    let PathObservationResult::InputImage(result) = raw.as_ref() else {
        return Err(ElfMetadataError::ObservationMismatch);
    };
    if !result.same_budget(&budget) {
        return Err(ImageDescriptorBudgetError::ForeignBudget.into());
    }
    if result.request() != &request || result.issued_path() != &key.file {
        return Err(ElfMetadataError::ObservationMismatch);
    }
    let verified = match result.data() {
        ImageObservationData::ElfMetadata(PathOperationResult::Present(value)) => value.dupe(),
        ImageObservationData::ElfMetadata(
            PathOperationResult::Missing | PathOperationResult::Error(_),
        ) => return Err(ElfMetadataError::StaleSource),
        _ => return Err(ElfMetadataError::ObservationMismatch),
    };
    if verified.digest() != digest || verified.policy() != key.policy {
        return Err(ElfMetadataError::ObservationMismatch);
    }
    if !verified.same_budget(&budget) {
        return Err(ImageDescriptorBudgetError::ForeignBudget.into());
    }
    let rows = image
        .observations()
        .observations()
        .len()
        .checked_add(1)
        .ok_or(ImageDescriptorBudgetError::ChargeOverflow)?;
    let bytes = rows
        .checked_mul(std::mem::size_of::<(
            PathObservationDemand,
            Arc<PathObservationResult>,
        )>())
        .and_then(|n| n.checked_mul(4))
        .and_then(|n| n.checked_add(128))
        .ok_or(ImageDescriptorBudgetError::ChargeOverflow)?;
    let reservation = budget.try_reserve(bytes)?;
    let mut frontier = Vec::with_capacity(rows);
    frontier.extend(
        image
            .observations()
            .observations()
            .iter()
            .map(|(demand, result)| (demand.dupe(), result.dupe())),
    );
    frontier.push((demand, raw));
    let observations = PathObservationEpoch::from_shared(frontier)
        .map_err(|_| ElfMetadataError::ObservationMismatch)?;
    let block = reservation.freeze(ObservedInner {
        image,
        file: key.file.dupe(),
        policy: key.policy,
        verified,
        observations,
    })?;
    Ok(PathOutcome::Complete(Ok(ObservedElfMetadata(block))))
}
