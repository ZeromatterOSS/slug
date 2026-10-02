//! Normal Label-source selection. This derives selectors, never IO authority or bytes.
use std::fmt;
use std::path::Path;
use std::path::PathBuf;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_workspace_v2::ImageChargedPath;
use slug_workspace_v2::ImageDescriptorBudget;
use slug_workspace_v2::ImageDescriptorReservation;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedInputImageKey;
use slug_workspace_v2::PathObservationNamespace;

use crate::HostRepositoryMaterializationDisposition;
use crate::RepositoryMaterialization;
use crate::SourcePreparationOutcome;
use crate::repository_label_path::HostRepositoryLabelPathError;
use crate::repository_label_path::HostRepositoryLabelPathObservationKey;
use crate::repository_label_path::HostRepositoryLabelPathSource;
use crate::repository_label_path::RepositoryLabelPathAddress;
use crate::source_preparation::RepositoryMaterializationResultKey;

mod types;
pub use types::*;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub struct SelectedLabelSourceKey(SelectedLabelSourceRecipe);
impl SelectedLabelSourceKey {
    pub fn new(recipe: SelectedLabelSourceRecipe) -> Self {
        Self(recipe)
    }
    pub fn recipe(&self) -> &SelectedLabelSourceRecipe {
        &self.0
    }
}
impl fmt::Display for SelectedLabelSourceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "selected-label-source:{}", self.0.label())
    }
}
type Value =
    SourcePreparationOutcome<Result<ObservedSelectedLabelSource, SelectedLabelSourceError>>;
#[async_trait]
impl Key for SelectedLabelSourceKey {
    type Value = Value;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match compute(self, ctx).await {
            Ok(value) => value,
            Err(error) => SourcePreparationOutcome::Complete(Err(error)),
        }
    }
    fn equality(left: &Self::Value, right: &Self::Value) -> bool {
        Self::validity(left) && Self::validity(right) && left.complete_eq(right)
    }
    fn validity(value: &Self::Value) -> bool {
        match value {
            SourcePreparationOutcome::Need(_) => false,
            SourcePreparationOutcome::Complete(Ok(_)) => true,
            SourcePreparationOutcome::Complete(Err(error)) => !error.is_transient(),
        }
    }
}

async fn compute(
    key: &SelectedLabelSourceKey,
    ctx: &mut DiceComputations<'_>,
) -> Result<Value, SelectedLabelSourceError> {
    let budget = ctx
        .per_transaction_data()
        .data
        .get::<ImageDescriptorBudget>()
        .map_err(|_| SelectedLabelSourceError::MissingBudget)?
        .dupe();
    let recipe = key.recipe();
    if !recipe.same_budget(&budget) {
        return Err(slug_workspace_v2::ImageDescriptorBudgetError::ForeignBudget.into());
    }
    let address = RepositoryLabelPathAddress::from_label(recipe.label());
    let label_key = match recipe.source() {
        HostRepositoryLabelPathSource::Root(root) => {
            HostRepositoryLabelPathObservationKey::new_root(root.dupe(), address)
        }
        HostRepositoryLabelPathSource::External(route) => {
            // This is the existing ordinary Label-key owner, not a recipe clone.
            HostRepositoryLabelPathObservationKey::new_external(route.clone(), address)
        }
    }
    .map_err(|_| SelectedLabelSourceError::OriginMismatch)?;
    let label = match ctx
        .compute(&label_key)
        .await
        .map_err(|_| SelectedLabelSourceError::Compute)?
    {
        SourcePreparationOutcome::Need(need) => return Ok(SourcePreparationOutcome::Need(need)),
        SourcePreparationOutcome::Complete(Err(error)) => {
            return Err(SelectedLabelSourceError::Frontier(error));
        }
        SourcePreparationOutcome::Complete(Ok(label)) => label,
    };
    let value = match label.result().as_ref() {
        Ok(value) => value,
        Err(error) => return Err(label_error(error, &budget)?),
    };
    let mut reservation = budget.try_reserve(0)?;
    let mut descriptors = recipe.descriptor_bytes()?;
    let mut repository = None;
    let (anchor, namespace) = match recipe.source() {
        HostRepositoryLabelPathSource::Root(_) => {
            (recipe.workspace().dupe(), PathObservationNamespace::Host)
        }
        HostRepositoryLabelPathSource::External(route) => {
            let request = match route
                .materialization_disposition()
                .map_err(|_| SelectedLabelSourceError::Materialization)?
            {
                HostRepositoryMaterializationDisposition::Builtin(_) => {
                    return Err(SelectedLabelSourceError::UnsupportedBuiltin);
                }
                HostRepositoryMaterializationDisposition::Request(request) => request,
            };
            let result = match ctx
                .compute(&RepositoryMaterializationResultKey {
                    request: request.dupe(),
                })
                .await
                .map_err(|_| SelectedLabelSourceError::Compute)?
            {
                SourcePreparationOutcome::Need(need) => {
                    return Ok(SourcePreparationOutcome::Need(need));
                }
                SourcePreparationOutcome::Complete(result) => result,
            };
            let (root, namespace) = match result.as_ref() {
                Err(_) => return Err(SelectedLabelSourceError::Materialization),
                Ok(RepositoryMaterialization::Local { source_root, .. }) => {
                    if !source_root.starts_with(recipe.workspace().as_path()) {
                        return Err(SelectedLabelSourceError::UnsupportedLocal);
                    }
                    (source_root.as_path(), PathObservationNamespace::Host)
                }
                Ok(RepositoryMaterialization::Immutable {
                    generation_root,
                    observation_instance,
                    ..
                }) => (
                    generation_root.as_path(),
                    PathObservationNamespace::Materialization(*observation_instance),
                ),
            };
            let anchor = charged_path(
                root,
                None,
                recipe,
                &budget,
                &mut reservation,
                &mut descriptors,
            )?;
            // Ordinary Label path must agree with the actual current materialization.
            // Compare lexical relative components without another path allocation.
            let expected = Path::new(recipe.label().package().package().as_str())
                .components()
                .chain(Path::new(recipe.label().target().as_str()).components())
                .filter(|part| !matches!(part, std::path::Component::CurDir));
            if value.namespace() != namespace
                || !value
                    .path()
                    .as_path()
                    .strip_prefix(anchor.as_path())
                    .is_ok_and(|relative| relative.components().eq(expected))
            {
                return Err(SelectedLabelSourceError::ObservationMismatch);
            }
            repository = Some((request, result));
            (anchor, namespace)
        }
    };
    if value.namespace() != namespace {
        return Err(SelectedLabelSourceError::ObservationMismatch);
    }
    let original = value.path().as_path();
    if !original.starts_with(anchor.as_path()) {
        return Err(SelectedLabelSourceError::ScopeEscape);
    }
    types::check_path(original.as_os_str().len(), &recipe.policy())?;
    descriptors = types::add(descriptors, original.as_os_str().len())?;
    types::check_descriptors(descriptors, &recipe.policy())?;
    let mut file = ImageChargedPath::pin_existing(value.path().dupe(), &budget)?;
    let parent = original
        .parent()
        .ok_or(SelectedLabelSourceError::InvalidPath)?;
    if !parent.starts_with(anchor.as_path()) {
        return Err(SelectedLabelSourceError::ScopeEscape);
    }
    let tree = charged_path(
        parent,
        None,
        recipe,
        &budget,
        &mut reservation,
        &mut descriptors,
    )?;
    for transform in recipe.transforms() {
        let (base, child) = match transform {
            SelectedLabelSourceTransform::Parent => (
                file.as_path()
                    .parent()
                    .ok_or(SelectedLabelSourceError::InvalidPath)?,
                None,
            ),
            SelectedLabelSourceTransform::Child(_) => (file.as_path(), transform.child()),
        };
        if !base.starts_with(tree.as_path()) {
            return Err(SelectedLabelSourceError::ScopeEscape);
        }
        file = charged_path(
            base,
            child,
            recipe,
            &budget,
            &mut reservation,
            &mut descriptors,
        )?;
        if !file.as_path().starts_with(tree.as_path())
            || !file.as_path().starts_with(anchor.as_path())
        {
            return Err(SelectedLabelSourceError::ScopeEscape);
        }
    }
    let image = ObservedInputImageKey::from_charged(
        namespace,
        anchor,
        tree,
        recipe.policy().image_policy(),
    )
    .map_err(|_| SelectedLabelSourceError::ObservationMismatch)?;
    Ok(SourcePreparationOutcome::Complete(Ok(
        ObservedSelectedLabelSource(reservation.freeze(types::SelectionInner {
            recipe: recipe.dupe(),
            image,
            file,
            observations: label.observations().dupe(),
            repository,
        })?),
    )))
}

/// Allocate a normalized lexical selector only after semantic and physical checks.
fn charged_path(
    base: &Path,
    child: Option<&str>,
    recipe: &SelectedLabelSourceRecipe,
    budget: &ImageDescriptorBudget,
    reservation: &mut ImageDescriptorReservation,
    descriptors: &mut usize,
) -> Result<ImageChargedPath, SelectedLabelSourceError> {
    let bytes = if let Some(child) = child {
        types::add(types::add(base.as_os_str().len(), child.len())?, 1)?
    } else {
        base.as_os_str().len()
    };
    types::check_path(bytes, &recipe.policy())?;
    *descriptors = types::add(*descriptors, bytes)?;
    types::check_descriptors(*descriptors, &recipe.policy())?;
    // Input PathBuf + geometric normalization replacement peaks and Arc headers.
    reservation.try_grow(types::add(
        bytes
            .checked_mul(5)
            .ok_or(slug_workspace_v2::ImageDescriptorBudgetError::ChargeOverflow)?,
        128,
    )?)?;
    let mut path = PathBuf::new();
    path.try_reserve_exact(bytes)
        .map_err(|_| slug_workspace_v2::ImageDescriptorBudgetError::ResourceLimit)?;
    path.push(base);
    if let Some(child) = child {
        path.push(child);
    }
    let path =
        NormalizedAbsolutePath::new(path).map_err(|_| SelectedLabelSourceError::InvalidPath)?;
    if path.as_path().to_str().is_none() {
        return Err(SelectedLabelSourceError::InvalidPath);
    }
    Ok(ImageChargedPath::pin_existing(path, budget)?)
}
fn label_error(
    error: &HostRepositoryLabelPathError,
    budget: &ImageDescriptorBudget,
) -> Result<SelectedLabelSourceError, SelectedLabelSourceError> {
    let owned = match error {
        HostRepositoryLabelPathError::RootRepositoryMismatch(repo) => repo.as_str().len(),
        HostRepositoryLabelPathError::ExternalRepositoryMismatch(left, right) => {
            types::add(left.as_str().len(), right.as_str().len())?
        }
        _ => 0, // Remaining payloads are existing shared Arc strings or fixed enums.
    };
    Ok(SelectedLabelSourceError::Label(
        budget
            .try_reserve(types::add(owned, 128)?)?
            .freeze(error.clone())?,
    ))
}
