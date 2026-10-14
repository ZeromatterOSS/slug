//! Complete borrowed conversion admission before allocating source authorities.
use std::mem::size_of;

use super::*;

mod mapping_audit;
pub(super) use mapping_audit::MappingAudits;
use mapping_audit::MappingPlan;

fn add(total: &mut usize, bytes: usize) -> Result<(), String> {
    *total = total
        .checked_add(bytes)
        .ok_or("Args source conversion byte overflow")?;
    Ok(())
}

fn array<T>(count: usize) -> Result<usize, String> {
    // Explicit-capacity Vec and the new Arc slice coexist during transfer.
    count
        .checked_mul(size_of::<T>())
        .and_then(|bytes| bytes.checked_mul(2))
        .and_then(|bytes| bytes.checked_add(2 * size_of::<usize>()))
        .ok_or_else(|| "Args source conversion array overflow".into())
}

// Only the old Vec backing ends during transfer. Final Arc elements/headers
// and every dynamic identity payload remain in the complete retained bound.
fn output_transfer_bytes(direct: usize, reachable: usize) -> Result<usize, String> {
    direct
        .checked_mul(size_of::<CallableModuleIdentity>())
        .zip(reachable.checked_mul(size_of::<CallableModuleSource>()))
        .and_then(|(direct, reachable)| direct.checked_add(reachable))
        .ok_or_else(|| "Args source transfer byte overflow".into())
}

fn identity_bytes<'a>(
    identity: &'a BzlModuleIdentity,
    audits: &MappingAudits,
    plan: &mut MappingPlan<'a>,
    builder: &mut PublishedValueArenaBuilder,
    remaining: &mut usize,
) -> Result<usize, String> {
    step(builder, remaining)?;
    let label = &identity.label;
    let mut bytes = identity.workspace_path.as_os_str().len();
    for length in [
        label.package().repo().as_str().len(),
        label.package().package().as_str().len(),
        label.target().as_str().len(),
        label.mapping_id().map_or(0, |id| id.as_str().len()),
    ] {
        add(&mut bytes, length)?;
    }
    // The complete old bound remains in EVERY identity's output forecast.
    add(
        &mut bytes,
        plan.mapping_bytes(&identity.repository_mapping, audits, builder, remaining)?,
    )?;
    Ok(bytes)
}

fn forecast<'a>(
    manifest: &'a BzlLoadManifest,
    audits: &MappingAudits,
    builder: &mut PublishedValueArenaBuilder,
    remaining: &mut usize,
) -> Result<MappingPlan<'a>, String> {
    use slug_build_api_v2::callable_graph::CALLABLE_GRAPH_NODE_LIMIT;
    audits.check()?;
    let mut plan = MappingPlan::new(builder)?;
    if manifest.reachable.len() > CALLABLE_GRAPH_NODE_LIMIT
        || manifest.direct_children.len() > CALLABLE_GRAPH_NODE_LIMIT
    {
        return Err("Args source closure fact limit".into());
    }
    // Includes the inline root and both Arc handle fields, plus Arc counters.
    let mut bytes = size_of::<CallableSourceAuthorityData>()
        + CallableSourceAuthority::storage_overhead_bytes();
    add(
        &mut bytes,
        array::<CallableModuleIdentity>(manifest.direct_children.len())?,
    )?;
    add(
        &mut bytes,
        array::<CallableModuleSource>(manifest.reachable.len())?,
    )?;
    add(
        &mut bytes,
        identity_bytes(&manifest.root, audits, &mut plan, builder, remaining)?,
    )?;
    for child in &*manifest.direct_children {
        add(
            &mut bytes,
            identity_bytes(child, audits, &mut plan, builder, remaining)?,
        )?;
    }
    for source in &*manifest.reachable {
        add(
            &mut bytes,
            identity_bytes(&source.identity, audits, &mut plan, builder, remaining)?,
        )?;
    }
    plan.output_bytes = bytes;
    Ok(plan)
}

pub(super) fn convert(
    manifest: &BzlLoadManifest,
    audits: &mut MappingAudits,
    builder: &mut PublishedValueArenaBuilder,
    remaining: &mut usize,
) -> Result<CallableSourceAuthority, String> {
    let result = convert_inner(manifest, audits, builder, remaining);
    if result.is_err() {
        audits.fail();
        let _ = builder.charge_bytes(usize::MAX);
    }
    result
}

fn convert_inner(
    manifest: &BzlLoadManifest,
    audits: &mut MappingAudits,
    builder: &mut PublishedValueArenaBuilder,
    remaining: &mut usize,
) -> Result<CallableSourceAuthority, String> {
    audits.admit_header(builder)?;
    let plan = forecast(manifest, audits, builder, remaining)?;
    let transfer = output_transfer_bytes(manifest.direct_children.len(), manifest.reachable.len())?;
    let (credit, preparation) = plan.admit(audits, builder, remaining, transfer)?;
    plan.commit(audits, credit);
    let mut direct_children = Vec::with_capacity(manifest.direct_children.len());
    let mut reachable = Vec::with_capacity(manifest.reachable.len());
    for child in &*manifest.direct_children {
        direct_children.push(identity(child));
    }
    for source in &*manifest.reachable {
        reachable.push(CallableModuleSource {
            identity: identity(&source.identity),
            digest: source.source_digest,
        });
    }
    preparation
        .finish(CallableSourceAuthorityData {
            root: identity(&manifest.root),
            direct_children: direct_children.into(),
            reachable: reachable.into(),
            fingerprint: manifest.fingerprint,
        })
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests;
