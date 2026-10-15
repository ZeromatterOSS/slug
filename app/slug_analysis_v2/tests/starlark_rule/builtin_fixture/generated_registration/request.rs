//! Prepay the existing materialization owner's potential Need construction.
use std::alloc::Layout;
use std::sync::Arc;

use slug_bzlmod_v2::GeneratedRepositoryOrderedEffectRef;
use slug_bzlmod_v2::HostSelectedExtensionOwner;
use slug_bzlmod_v2::RepositoryExecuteCallId;
use slug_bzlmod_v2::RepositoryExecuteRequest;
use slug_bzlmod_v2::RepositoryFilesystemCallId;
use slug_bzlmod_v2::RepositoryFilesystemRequest;
use slug_bzlmod_v2::RepositoryMaterializationKind;
use slug_bzlmod_v2::RepositoryMaterializationRequest;
use slug_bzlmod_v2::RepositoryMaterializationRequestId;
use slug_bzlmod_v2::SelectedExtensionDownloadRequest;
use slug_bzlmod_v2::SourcePreparationNeeds;
use starlark_map::StarlarkHashValue;
use starlark_map::small_map::SmallMap;

fn arc_bytes<T>() -> Option<usize> {
    Some(
        Layout::array::<usize>(2)
            .ok()?
            .extend(Layout::new::<T>())
            .ok()?
            .0
            .pad_to_align()
            .size(),
    )
}

pub(super) fn admit(
    request: &Arc<RepositoryMaterializationRequest>,
    budget: &mut super::Budget,
) -> Result<bool, String> {
    let RepositoryMaterializationKind::GeneratedFileEffects(plan) = &request.kind else {
        return Ok(false);
    };
    for effect in plan.ordered_effects() {
        budget.charge(1, 0)?;
        if !matches!(effect, GeneratedRepositoryOrderedEffectRef::File(_)) {
            return Ok(false);
        }
    }
    // Request::clone shares attributes and both plan arrays. Its owned scalar
    // strings, including the label's mapping identity, must still be prepaid.
    let label = &request.repo_spec.rule_id.bzl_file;
    let strings = [
        request.id.workspace.as_path().as_os_str().len(),
        request.id.canonical_repo.as_str().len(),
        label.package().repo().as_str().len(),
        label.package().package().as_str().len(),
        label.target().as_str().len(),
        label.mapping_id().map_or(0, |id| id.as_str().len()),
        request.repo_spec.rule_id.rule_name.len(),
    ]
    .into_iter()
    .try_fold(0usize, usize::checked_add)
    .and_then(|n| n.checked_mul(8));
    type Requests =
        SmallMap<RepositoryMaterializationRequestId, Arc<RepositoryMaterializationRequest>>;
    type Downloads =
        SmallMap<Arc<HostSelectedExtensionOwner>, Arc<SelectedExtensionDownloadRequest>>;
    type Executes = SmallMap<RepositoryExecuteCallId, Arc<RepositoryExecuteRequest>>;
    type Filesystem = SmallMap<RepositoryFilesystemCallId, Arc<RepositoryFilesystemRequest>>;
    // Needs::repository allocates four Arc map headers and one Request Arc.
    // Its single map insertion has no Index and Vec2 minimum capacity <=8.
    let pair = Layout::array::<(
        RepositoryMaterializationRequestId,
        Arc<RepositoryMaterializationRequest>,
    )>(8)
    .ok()
    .and_then(|values| {
        values
            .extend(Layout::array::<StarlarkHashValue>(8).ok()?)
            .ok()
            .map(|x| x.0.size())
    });
    let headers = arc_bytes::<Requests>()
        .and_then(|n| n.checked_add(arc_bytes::<Downloads>()?))
        .and_then(|n| n.checked_add(arc_bytes::<Executes>()?))
        .and_then(|n| n.checked_add(arc_bytes::<Filesystem>()?))
        .and_then(|n| n.checked_add(arc_bytes::<RepositoryMaterializationRequest>()?))
        .and_then(|n| n.checked_add(pair?))
        .and_then(|n| n.checked_add(std::mem::size_of::<SourcePreparationNeeds>()))
        .and_then(|n| n.checked_add(std::mem::size_of::<Arc<RepositoryMaterializationRequest>>()))
        .and_then(|n| n.checked_mul(4)); // Conservative simultaneous move/return peak.
    let bytes = strings
        .and_then(|n| n.checked_add(headers?))
        .ok_or("fixture materialization request storage overflow")?;
    // The Arc clone and actual materialization-owner lookup follow admission.
    budget.charge(2, bytes)?;
    Ok(true)
}
