//! Complete identity fields and actual SmallMap/index peak, without owning copies.
use std::alloc::Layout;
use std::mem::size_of;

use super::*;
use crate::bzl_module::semantic_evaluation::budget::limit;

pub(in crate::bzl_environment::source_context) fn identity(
    id: &BzlModuleIdentity,
    mut charge: impl FnMut(usize, usize) -> Result<(), String>,
) -> Result<(), String> {
    identity_with_mapping(id, |r, b, _| charge(r, b))
}
pub(in crate::bzl_environment::source_context) fn identity_with_mapping(
    id: &BzlModuleIdentity,
    mut charge: impl FnMut(
        usize,
        usize,
        Option<(&ApparentRepoName, &CanonicalRepoName)>,
    ) -> Result<(), String>,
) -> Result<(), String> {
    identity_prefix(id, |r, b| charge(r, b, None))?;
    mapping_entries(id, |r, b, a, c| charge(r, b, Some((a, c))))
}
pub(in crate::bzl_environment::source_context) fn identity_prefix(
    id: &BzlModuleIdentity,
    mut charge: impl FnMut(usize, usize) -> Result<(), String>,
) -> Result<(), String> {
    let label = &id.label;
    charge(
        1,
        size_of::<BzlModuleIdentity>()
            .saturating_add(label.package().repo().as_str().len())
            .saturating_add(label.package().package().as_str().len())
            .saturating_add(label.target().as_str().len())
            .saturating_add(label.mapping_id().map_or(0, |v| v.as_str().len()))
            .saturating_add(id.source.retained_text_bytes().ok_or_else(limit)?)
            .saturating_add(2 * size_of::<usize>()),
    )
}
pub(in crate::bzl_environment::source_context) fn mapping_entries(
    id: &BzlModuleIdentity,
    mut charge: impl FnMut(usize, usize, &ApparentRepoName, &CanonicalRepoName) -> Result<(), String>,
) -> Result<(), String> {
    for (a, c) in &*id.repository_mapping {
        charge(
            1,
            size_of::<(ApparentRepoName, CanonicalRepoName)>()
                .saturating_add(a.as_str().len())
                .saturating_add(c.as_str().len()),
            a,
            c,
        )?;
    }
    Ok(())
}

fn label_bytes(label: &CanonicalLabel) -> usize {
    size_of::<CanonicalLabel>()
        .saturating_add(label.package().repo().as_str().len())
        .saturating_add(label.package().package().as_str().len())
        .saturating_add(label.target().as_str().len())
        .saturating_add(label.mapping_id().map_or(0, |v| v.as_str().len()))
}
pub(super) fn hash_label(label: &CanonicalLabel, work: &mut Budget) -> Result<u64, String> {
    use std::hash::Hasher;
    work.charge(1, label_bytes(label))?;
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    label.hash(&mut hash);
    Ok(hash.finish())
}
pub(super) fn same_label(
    a: &CanonicalLabel,
    b: &CanonicalLabel,
    work: &mut Budget,
) -> Result<bool, String> {
    work.charge(2, label_bytes(a).saturating_add(label_bytes(b)))?;
    Ok(a == b)
}
// Called only AFTER complete label equality, which includes mapping_id.
// Arc identity skips the actual immutable slice traversal; foreign slices are
// compared completely (or to the first unequal pair), never by digest.
pub(super) fn difference_tail(
    a: &BzlModuleSourceProvenance,
    b: &BzlModuleSourceProvenance,
    work: &mut Budget,
    mut inspected_pair: impl FnMut(),
) -> Result<Option<conflict::TailDifference>, String> {
    work.charge(
        2,
        64usize
            .saturating_add(2 * size_of::<slug_build_api_v2::ModuleSourceOrigin>())
            .saturating_add(a.identity.source.retained_text_bytes().ok_or_else(limit)?)
            .saturating_add(b.identity.source.retained_text_bytes().ok_or_else(limit)?),
    )?;
    if a.identity.source != b.identity.source {
        return Ok(Some(conflict::TailDifference::Origin));
    }
    if a.source_digest != b.source_digest {
        return Ok(Some(conflict::TailDifference::Digest));
    }
    work.charge(1, 4 * size_of::<usize>())?;
    let x = &a.identity.repository_mapping;
    let y = &b.identity.repository_mapping;
    if Arc::ptr_eq(x, y) {
        return Ok(None);
    }
    work.charge(1, 2 * size_of::<usize>())?;
    if x.len() != y.len() {
        return Ok(Some(conflict::TailDifference::MappingLength));
    }
    for (ordinal, ((aa, ac), (ba, bc))) in x.iter().zip(y.iter()).enumerate() {
        work.charge(
            2,
            (2 * size_of::<(ApparentRepoName, CanonicalRepoName)>())
                .saturating_add(aa.as_str().len())
                .saturating_add(ac.as_str().len())
                .saturating_add(ba.as_str().len())
                .saturating_add(bc.as_str().len()),
        )?;
        inspected_pair();
        if aa != ba || ac != bc {
            return Ok(Some(conflict::TailDifference::MappingEntry(ordinal)));
        }
    }
    Ok(None)
}
#[cfg(test)]
pub(super) fn same_tail(
    a: &BzlModuleSourceProvenance,
    b: &BzlModuleSourceProvenance,
    work: &mut Budget,
    inspected_pair: impl FnMut(),
) -> Result<bool, String> {
    Ok(difference_tail(a, b, work, inspected_pair)?.is_none())
}
fn map_peak<K, V>(count: usize) -> Result<usize, String> {
    let c = count.checked_mul(2).ok_or_else(limit)?.max(4);
    let entries = Layout::array::<(K, V)>(c)
        .and_then(|pairs| pairs.extend(Layout::array::<starlark_map::StarlarkHashValue>(c)?))
        .map_err(|_| limit())?
        .0
        .pad_to_align()
        .size();
    let index = c
        .checked_mul(4)
        .and_then(|b| b.checked_mul(size_of::<usize>() + 1))
        .and_then(|b| b.checked_add(1024))
        .and_then(|b| b.checked_add(std::mem::align_of::<usize>().max(16) - 1))
        .and_then(|b| b.checked_add(16))
        .ok_or_else(limit)?;
    entries
        .checked_mul(2)
        .and_then(|b| b.checked_add(index.checked_mul(2)?))
        .and_then(|b| b.checked_add(size_of::<SmallMap<K, V>>()))
        .ok_or_else(limit)
}
pub(super) fn scratch_bytes(count: usize) -> Result<usize, String> {
    // Dedicated pair/hash arrays, min capacity, old/new coexistence, boxed
    // Index/header, load/control/rehash. Candidate Vec element peaks sum to
    // <=16*M usize slots, including a min4 allocation for every nonempty bucket.
    map_peak::<u64, Vec<usize>>(count)?
        .checked_add(map_peak::<usize, (usize, &BzlModuleSourceProvenance)>(
            count,
        )?)
        .and_then(|b| b.checked_add(count.checked_mul(16 * size_of::<usize>())?))
        .and_then(|b| {
            b.checked_add(count.checked_mul(2 * size_of::<&BzlModuleSourceProvenance>())?)
        })
        .and_then(|b| b.checked_add(size_of::<Plan<'_>>()))
        .ok_or_else(limit)
}
