//! One caller ledger pays comparison, filtered guard and transfer peaks.
use std::mem::size_of;

use slug_bzlmod_v2::OverrideAttributeKey;
use slug_bzlmod_v2::OverrideAttributeValue;
use slug_bzlmod_v2::RepositoryMaterializationRequest;
use slug_identity_v2::CanonicalLabel;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathObservationInstanceId;
use slug_workspace_v2::PathObservationNamespace;

use super::super::Budget;

fn label(label: &CanonicalLabel, budget: &mut Budget) -> Result<(), String> {
    for text in [
        label.package().repo().as_str(),
        label.package().package().as_str(),
        label.target().as_str(),
        label.mapping_id().map_or("", |id| id.as_str()),
    ] {
        budget.charge(
            4,
            text.len().checked_mul(8).ok_or("fixture label overflow")?,
        )?;
    }
    Ok(())
}
fn key(key: &OverrideAttributeKey, budget: &mut Budget) -> Result<(), String> {
    match key {
        OverrideAttributeKey::String(text) => budget.charge(
            4,
            text.capacity()
                .checked_mul(8)
                .ok_or("fixture attribute key overflow")?,
        ),
        OverrideAttributeKey::Label(value) => label(value, budget),
    }
}
fn values<'a>(
    values: impl Iterator<Item = &'a OverrideAttributeValue>,
    budget: &mut Budget,
) -> Result<(), String> {
    // Iterative borrowed frames; every extension is prepaid, including first
    // allocation and old/new coexistence. No recursive Rust stack walk.
    let mut frames = Vec::new();
    for value in values {
        budget.charge(1, size_of::<&OverrideAttributeValue>() * 16)?;
        frames.push(value);
    }
    while let Some(value) = frames.pop() {
        budget.charge(4, size_of::<OverrideAttributeValue>() * 4)?;
        match value {
            OverrideAttributeValue::None
            | OverrideAttributeValue::Bool(_)
            | OverrideAttributeValue::Int(_) => {}
            OverrideAttributeValue::String(text) => budget.charge(
                4,
                text.capacity()
                    .checked_mul(8)
                    .ok_or("fixture attribute text overflow")?,
            )?,
            OverrideAttributeValue::Label(value) => label(value, budget)?,
            OverrideAttributeValue::Iterable(items) => {
                budget.charge(
                    items.len(),
                    items
                        .len()
                        .checked_mul(size_of::<&OverrideAttributeValue>() * 16)
                        .ok_or("fixture attribute frames overflow")?,
                )?;
                frames.extend(items.iter());
            }
            OverrideAttributeValue::Map(items) => {
                // SmallMap foreign Eq may inspect every colliding key for each
                // entry. Ordered publication Eq is also included in factor4.
                budget.charge(
                    items
                        .len()
                        .checked_mul(items.len())
                        .and_then(|n| n.checked_mul(4))
                        .ok_or("fixture map comparison overflow")?,
                    items
                        .len()
                        .checked_mul(size_of::<&OverrideAttributeValue>() * 16)
                        .ok_or("fixture map frames overflow")?,
                )?;
                for (k, v) in items.iter() {
                    key(k, budget)?;
                    frames.push(v);
                }
            }
        }
    }
    Ok(())
}
pub(in super::super) fn request(
    request: &std::sync::Arc<RepositoryMaterializationRequest>,
    budget: &mut Budget,
) -> Result<(), String> {
    let n = request.repo_spec.attributes.len();
    budget.charge(
        n.checked_mul(n)
            .and_then(|n| n.checked_mul(4))
            .ok_or("fixture attribute comparison overflow")?,
        size_of::<Vec<&OverrideAttributeValue>>(),
    )?;
    for name in request.repo_spec.attributes.keys() {
        budget.charge(
            4,
            name.capacity()
                .checked_mul(8)
                .ok_or("fixture attribute name overflow")?,
        )?;
    }
    values(request.repo_spec.attributes.values(), budget)?;
    label(&request.repo_spec.rule_id.bzl_file, budget)?;
    budget.charge(
        8,
        request
            .repo_spec
            .rule_id
            .rule_name
            .capacity()
            .checked_mul(8)
            .and_then(|n| {
                n.checked_add(
                    request
                        .id
                        .workspace
                        .as_path()
                        .as_os_str()
                        .len()
                        .checked_mul(8)?,
                )
            })
            .and_then(|n| n.checked_add(request.id.canonical_repo.as_str().len().checked_mul(8)?))
            .ok_or("fixture request comparison overflow")?,
    )?;
    // Existing request preflight includes actual plan arrays' file record walk
    // and the real owner return/Need allocation before its Arc clone.
    if let slug_bzlmod_v2::RepositoryMaterializationKind::Local { logical_root } = &request.kind {
        budget.charge(
            4,
            logical_root
                .as_path()
                .as_os_str()
                .len()
                .checked_mul(8)
                .ok_or("fixture local root overflow")?,
        )?;
    }
    super::super::request::admit(request, budget)?;
    Ok(())
}
pub(super) fn observations(
    prior: &PathObservationEpoch,
    current: &PathObservationEpoch,
    instance: PathObservationInstanceId,
    budget: &mut Budget,
) -> Result<(), String> {
    budget.charge(
        1,
        size_of::<
            Vec<(
                slug_workspace_v2::PathObservationDemand,
                std::sync::Arc<slug_workspace_v2::PathObservationResult>,
            )>,
        >(),
    )?;
    for (demand, _) in prior.observations() {
        budget.charge(1, 0)?;
        if demand.namespace() == PathObservationNamespace::Materialization(instance) {
            budget.charge(
                1 + current.observations().len().checked_ilog2().unwrap_or(0) as usize,
                demand
                    .path()
                    .as_path()
                    .as_os_str()
                    .len()
                    .checked_mul(4)
                    .and_then(|n| {
                        n.checked_add(
                            size_of::<(
                                slug_workspace_v2::PathObservationDemand,
                                std::sync::Arc<slug_workspace_v2::PathObservationResult>,
                            )>() * 4,
                        )
                    })
                    .ok_or("fixture observation guard overflow")?,
            )?;
        }
    }
    Ok(())
}

// Existing header/array transfer credit stays paid by materialize(). These
// dynamic scalar fields also copy; Arc payloads and request bodies stay shared.
pub(super) fn transfer(
    observations: &PathObservationEpoch,
    entries: &[slug_bzlmod_v2::RepositoryMaterializationEpochEntry],
    budget: &mut Budget,
) -> Result<(), String> {
    for (demand, _) in observations.observations() {
        budget.charge(
            1,
            demand
                .path()
                .as_path()
                .as_os_str()
                .len()
                .checked_mul(4)
                .ok_or("fixture observation transfer overflow")?,
        )?;
    }
    for entry in entries {
        use slug_bzlmod_v2::RepositoryMaterializationResult as Result;
        use slug_bzlmod_v2::RepositoryMaterializationSuccess as Success;
        let bytes = match &entry.result {
            Result::Success(Success::Local) => 0,
            Result::Success(Success::Immutable {
                generation_root, ..
            }) => generation_root.as_os_str().len(),
            Result::SpecError(text)
            | Result::TransportError { message: text, .. }
            | Result::MaterializationError { message: text, .. } => text.capacity(),
        };
        budget.charge(
            1,
            bytes
                .checked_mul(4)
                .ok_or("fixture entry transfer overflow")?,
        )?;
    }
    Ok(())
}
