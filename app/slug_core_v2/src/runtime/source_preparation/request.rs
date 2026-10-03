//! Admission for the two actual retained caller copies, before either clone.
use slug_bzlmod_v2::ModuleKey;
use slug_bzlmod_v2::YankedVersionPolicy;

use super::*;

fn policy_bytes(policy: &YankedVersionPolicy) -> Result<usize, NativeDemandSessionError> {
    let YankedVersionPolicy::AllowList(rows) = policy else {
        return Ok(0);
    };
    rows.iter()
        .try_fold(32 * std::mem::size_of::<ModuleKey>() + 512, |total, row| {
            // Conservative node capacity/header allowance, including clone peak.
            // Shared override/registry/environment backing is not cloned here.
            total
                .checked_add(32 * std::mem::size_of::<ModuleKey>() + 512)
                .and_then(|n| n.checked_add(row.name.capacity()))
                .and_then(|n| n.checked_add(row.version.capacity()))
                .ok_or_else(|| refusal("source caller policy capacity overflow"))
        })
}
pub(super) fn copy_credit(
    runtime: &WorkspaceRuntime,
    inputs: &NativeDemandRequestInputBundle,
) -> Result<ImageDescriptorReservation, NativeDemandSessionError> {
    let bytes = policy_bytes(inputs.command_policy.yanked_versions_policy())?
        .checked_add(policy_bytes(
            inputs.environment_policy.yanked_versions_policy(),
        )?)
        .and_then(|n| n.checked_add(std::mem::size_of::<NativeDemandRequestInputBundle>()))
        .and_then(|n| n.checked_mul(2))
        .and_then(|n| n.checked_add(std::mem::size_of::<PreparedSourceOwner>() + 512))
        .ok_or_else(|| refusal("source caller copy capacity overflow"))?;
    runtime
        .image_resources
        .descriptor_budget()
        .try_reserve(bytes)
        .map_err(|e| NativeDemandSessionError::InputImage(e.into()))
}
