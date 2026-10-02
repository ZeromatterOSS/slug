//! Credited source recipes. Existing normal route allocations transfer ownership.
use std::fmt;
use std::mem::size_of;
use std::sync::Arc;

use allocative::Allocative;
use dupe::Dupe;
use slug_identity_v2::CanonicalLabel;
use slug_workspace_v2::BudgetedImageBlock;
use slug_workspace_v2::ImageChargedPath;
use slug_workspace_v2::ImageDescriptorBudget;
use slug_workspace_v2::ImageDescriptorBudgetError;
use slug_workspace_v2::ImageDescriptorReservation;
use slug_workspace_v2::InputImagePolicy;
use slug_workspace_v2::ObservedInputImageKey;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathObservationEpoch;

use crate::HostRepositoryLabelPathError;
use crate::RepositoryMaterialization;
use crate::RepositoryMaterializationError;
use crate::RepositoryMaterializationRequest;
use crate::repository_label_path::HostRepositoryLabelPathSource;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative, Dupe)]
pub struct SelectedLabelSourceLimits {
    pub max_transforms: usize,
    pub max_path_bytes: usize,
    pub max_descriptor_bytes: usize,
}
impl Default for SelectedLabelSourceLimits {
    fn default() -> Self {
        Self {
            max_transforms: 64,
            max_path_bytes: 4096,
            max_descriptor_bytes: 4 * 1024 * 1024,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative, Dupe)]
pub struct SelectedLabelSourcePolicy {
    limits: SelectedLabelSourceLimits,
    image: InputImagePolicy,
}
impl SelectedLabelSourcePolicy {
    pub fn new(
        limits: SelectedLabelSourceLimits,
        image: InputImagePolicy,
    ) -> Result<Self, SelectedLabelSourceError> {
        let ceiling = SelectedLabelSourceLimits::default();
        if limits.max_transforms > ceiling.max_transforms
            || limits.max_path_bytes == 0
            || limits.max_path_bytes > ceiling.max_path_bytes
            || limits.max_descriptor_bytes > ceiling.max_descriptor_bytes
        {
            return Err(SelectedLabelSourceError::InvalidPolicy);
        }
        Ok(Self { limits, image })
    }
    pub fn plain() -> Self {
        Self {
            limits: SelectedLabelSourceLimits::default(),
            image: InputImagePolicy::plain(),
        }
    }
    pub fn limits(&self) -> &SelectedLabelSourceLimits {
        &self.limits
    }
    pub fn image_policy(&self) -> InputImagePolicy {
        self.image
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
pub enum SelectedLabelSourceLimit {
    Transforms,
    PathBytes,
    DescriptorBytes,
}
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum SelectedLabelSourceError {
    Budget(ImageDescriptorBudgetError),
    MissingBudget,
    InvalidPolicy,
    OriginMismatch,
    InvalidChild,
    InvalidPath,
    ScopeEscape,
    UnsupportedLocal,
    UnsupportedBuiltin,
    LimitExceeded(SelectedLabelSourceLimit),
    Label(BudgetedImageBlock<HostRepositoryLabelPathError>),
    Frontier(ObservedPathFrontierError),
    Compute,
    Materialization,
    ObservationMismatch,
}
impl SelectedLabelSourceError {
    pub fn is_transient(&self) -> bool {
        match self {
            Self::Budget(_)
            | Self::MissingBudget
            | Self::Frontier(_)
            | Self::Compute
            | Self::Materialization
            | Self::ObservationMismatch => true,
            Self::Label(error) => matches!(
                error.get(),
                HostRepositoryLabelPathError::Materialization(_)
                    | HostRepositoryLabelPathError::MaterializationCompute(_)
                    | HostRepositoryLabelPathError::RootPackageLookup(_)
                    | HostRepositoryLabelPathError::ExternalPackageLookup(_)
            ),
            _ => false,
        }
    }
}
impl From<ImageDescriptorBudgetError> for SelectedLabelSourceError {
    fn from(error: ImageDescriptorBudgetError) -> Self {
        Self::Budget(error)
    }
}
impl fmt::Display for SelectedLabelSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "selected Label source: {self:?}")
    }
}
impl std::error::Error for SelectedLabelSourceError {}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub struct SelectedLabelSourceChild(BudgetedImageBlock<String>);
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub enum SelectedLabelSourceTransform {
    Parent,
    Child(SelectedLabelSourceChild),
}
impl SelectedLabelSourceTransform {
    pub fn child(&self) -> Option<&str> {
        match self {
            Self::Parent => None,
            Self::Child(child) => Some(child.0.get().as_str()),
        }
    }
}
#[derive(Debug, PartialEq, Eq, Hash, Allocative)]
struct Origin {
    workspace: ImageChargedPath,
    label: CanonicalLabel,
    source: HostRepositoryLabelPathSource,
    policy: SelectedLabelSourcePolicy,
    descriptor_bytes: usize,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub struct SelectedLabelSourceRecipe {
    origin: BudgetedImageBlock<Origin>,
    transforms: BudgetedImageBlock<Vec<SelectedLabelSourceTransform>>,
}
impl SelectedLabelSourceRecipe {
    /// Source transfers its existing normal owner. No route clone occurs here.
    pub fn new(
        workspace: ImageChargedPath,
        label: &CanonicalLabel,
        source: HostRepositoryLabelPathSource,
        policy: SelectedLabelSourcePolicy,
        budget: &ImageDescriptorBudget,
    ) -> Result<Self, SelectedLabelSourceError> {
        if !workspace.same_budget(budget) {
            return Err(ImageDescriptorBudgetError::ForeignBudget.into());
        }
        let workspace_text = workspace
            .as_path()
            .to_str()
            .ok_or(SelectedLabelSourceError::InvalidPath)?;
        match &source {
            HostRepositoryLabelPathSource::Root(root)
                if root == workspace.normalized_ref() && label.package().repo().is_root() => {}
            HostRepositoryLabelPathSource::External(route)
                if route.workspace() == workspace.normalized_ref()
                    && route.canonical_repo() == label.package().repo() => {}
            _ => return Err(SelectedLabelSourceError::OriginMismatch),
        }
        check_path(workspace_text.len(), &policy)?;
        let label_bytes = label_bytes(label)?;
        let descriptor_bytes = add(workspace_text.len(), add(label_bytes, 32)?)?;
        check_descriptors(descriptor_bytes, &policy)?;
        // String::clone copies live length; source spare capacity is not copied.
        // Mapping-id is an owned String too. Reserve before any owned clone.
        let reservation = budget.try_reserve(add(label_bytes, 4 * 64)?)?;
        let origin = reservation.freeze(Origin {
            workspace,
            label: label.clone(),
            source,
            policy,
            descriptor_bytes,
        })?;
        let transforms = budget.try_reserve(0)?.freeze(Vec::new())?;
        Ok(Self { origin, transforms })
    }
    /// Select the original Label's parent, sharing its credited origin rather
    /// than cloning Label/source ownership or retaining prior derived transforms.
    pub fn origin_parent(
        &self,
        budget: &ImageDescriptorBudget,
    ) -> Result<Self, SelectedLabelSourceError> {
        if !self.same_budget(budget) {
            return Err(ImageDescriptorBudgetError::ForeignBudget.into());
        }
        if self.policy().limits.max_transforms == 0 {
            return Err(SelectedLabelSourceError::LimitExceeded(
                SelectedLabelSourceLimit::Transforms,
            ));
        }
        check_descriptors(add(self.origin.get().descriptor_bytes, 16)?, &self.policy())?;
        let credit = budget.try_reserve(size_of::<SelectedLabelSourceTransform>())?;
        let mut transforms = Vec::new();
        transforms
            .try_reserve_exact(1)
            .map_err(|_| ImageDescriptorBudgetError::ResourceLimit)?;
        transforms.push(SelectedLabelSourceTransform::Parent);
        Ok(Self {
            origin: self.origin.dupe(),
            transforms: credit.freeze(transforms)?,
        })
    }
    pub fn parent(&self, budget: &ImageDescriptorBudget) -> Result<Self, SelectedLabelSourceError> {
        self.append(None, budget)
    }
    pub fn child(
        &self,
        literal: &str,
        budget: &ImageDescriptorBudget,
    ) -> Result<Self, SelectedLabelSourceError> {
        if literal.is_empty()
            || literal.contains(['\\', '\0'])
            || literal
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
            || literal
                .split('/')
                .next()
                .is_some_and(|part| part.ends_with(':'))
        {
            return Err(SelectedLabelSourceError::InvalidChild);
        }
        check_path(literal.len(), &self.policy())?;
        self.append(Some(literal), budget)
    }
    fn append(
        &self,
        child: Option<&str>,
        budget: &ImageDescriptorBudget,
    ) -> Result<Self, SelectedLabelSourceError> {
        if !self.same_budget(budget) {
            return Err(ImageDescriptorBudgetError::ForeignBudget.into());
        }
        let count = add(self.transforms().len(), 1)?;
        if count > self.policy().limits.max_transforms {
            return Err(SelectedLabelSourceError::LimitExceeded(
                SelectedLabelSourceLimit::Transforms,
            ));
        }
        let descriptor_bytes = add(
            self.descriptor_bytes()?,
            add(16, child.map_or(0, str::len))?,
        )?;
        check_descriptors(descriptor_bytes, &self.policy())?;
        let bytes = count
            .checked_mul(size_of::<SelectedLabelSourceTransform>())
            .ok_or(ImageDescriptorBudgetError::ChargeOverflow)?;
        let reservation = budget.try_reserve(bytes)?;
        let next = if let Some(child) = child {
            let reservation = budget.try_reserve(child.len())?;
            let mut literal = String::new();
            literal
                .try_reserve_exact(child.len())
                .map_err(|_| ImageDescriptorBudgetError::ResourceLimit)?;
            literal.push_str(child);
            SelectedLabelSourceTransform::Child(SelectedLabelSourceChild(
                reservation.freeze(literal)?,
            ))
        } else {
            SelectedLabelSourceTransform::Parent
        };
        let mut transforms = Vec::new();
        transforms
            .try_reserve_exact(count)
            .map_err(|_| ImageDescriptorBudgetError::ResourceLimit)?;
        transforms.extend(self.transforms().iter().map(Dupe::dupe));
        transforms.push(next);
        Ok(Self {
            origin: self.origin.dupe(),
            transforms: reservation.freeze(transforms)?,
        })
    }
    pub fn workspace(&self) -> &ImageChargedPath {
        &self.origin.get().workspace
    }
    pub fn label(&self) -> &CanonicalLabel {
        &self.origin.get().label
    }
    pub fn source(&self) -> &HostRepositoryLabelPathSource {
        &self.origin.get().source
    }
    pub fn transforms(&self) -> &[SelectedLabelSourceTransform] {
        self.transforms.get()
    }
    pub fn policy(&self) -> SelectedLabelSourcePolicy {
        self.origin.get().policy
    }
    pub fn same_budget(&self, budget: &ImageDescriptorBudget) -> bool {
        self.origin.same_budget(budget)
            && self.workspace().same_budget(budget)
            && self.transforms.same_budget(budget)
            && self.transforms().iter().all(|transform| match transform {
                SelectedLabelSourceTransform::Parent => true,
                SelectedLabelSourceTransform::Child(child) => child.0.same_budget(budget),
            })
    }
    pub(super) fn descriptor_bytes(&self) -> Result<usize, SelectedLabelSourceError> {
        self.transforms()
            .iter()
            .try_fold(self.origin.get().descriptor_bytes, |bytes, transform| {
                add(bytes, add(16, transform.child().map_or(0, str::len))?)
            })
    }
}
pub(super) fn add(left: usize, right: usize) -> Result<usize, SelectedLabelSourceError> {
    left.checked_add(right)
        .ok_or_else(|| ImageDescriptorBudgetError::ChargeOverflow.into())
}
pub(super) fn label_bytes(label: &CanonicalLabel) -> Result<usize, SelectedLabelSourceError> {
    [
        label.package().repo().as_str().len(),
        label.package().package().as_str().len(),
        label.target().as_str().len(),
        label.mapping_id().map_or(0, |id| id.as_str().len()),
    ]
    .into_iter()
    .try_fold(0, add)
}
pub(super) fn check_path(
    bytes: usize,
    policy: &SelectedLabelSourcePolicy,
) -> Result<(), SelectedLabelSourceError> {
    if bytes > policy.limits.max_path_bytes {
        return Err(SelectedLabelSourceError::LimitExceeded(
            SelectedLabelSourceLimit::PathBytes,
        ));
    }
    Ok(())
}
pub(super) fn check_descriptors(
    bytes: usize,
    policy: &SelectedLabelSourcePolicy,
) -> Result<(), SelectedLabelSourceError> {
    if bytes > policy.limits.max_descriptor_bytes {
        return Err(SelectedLabelSourceError::LimitExceeded(
            SelectedLabelSourceLimit::DescriptorBytes,
        ));
    }
    Ok(())
}

pub(super) type MaterializationValue =
    Arc<Result<RepositoryMaterialization, RepositoryMaterializationError>>;
#[derive(Debug, PartialEq, Eq, Allocative)]
pub(super) struct SelectionInner {
    pub recipe: SelectedLabelSourceRecipe,
    pub image: ObservedInputImageKey,
    pub file: ImageChargedPath,
    pub observations: PathObservationEpoch,
    pub repository: Option<(Arc<RepositoryMaterializationRequest>, MaterializationValue)>,
}
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedSelectedLabelSource(pub(super) BudgetedImageBlock<SelectionInner>);
impl ObservedSelectedLabelSource {
    pub fn recipe(&self) -> &SelectedLabelSourceRecipe {
        &self.0.get().recipe
    }
    pub fn image_key(&self) -> &ObservedInputImageKey {
        &self.0.get().image
    }
    pub fn selected_file(&self) -> &ImageChargedPath {
        &self.0.get().file
    }
    pub fn observations(&self) -> &PathObservationEpoch {
        &self.0.get().observations
    }
    pub fn repository(
        &self,
    ) -> Option<(
        &Arc<RepositoryMaterializationRequest>,
        &RepositoryMaterialization,
    )> {
        self.0.get().repository.as_ref().map(|(request, value)| {
            (
                request,
                value.as_ref().as_ref().expect("selection retains success"),
            )
        })
    }
    pub fn same_budget(&self, budget: &ImageDescriptorBudget) -> bool {
        self.0.same_budget(budget)
            && self.recipe().same_budget(budget)
            && self.selected_file().same_budget(budget)
        // Image key's anchor/tree are produced with the same budget and retained
        // inside this opaque selection; no external constructor can replace them.
    }
}
