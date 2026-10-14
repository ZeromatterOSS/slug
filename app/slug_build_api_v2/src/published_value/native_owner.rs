//! Complete borrowed owner inputs may import prior arenas; pure atom admission
//! remains closed and cannot retain these projected/current-arena wrappers.
use super::*;
use crate::ProviderIdentity;

pub(super) fn provider(
    b: &mut PublishedValueArenaBuilder,
    p: &crate::ProviderOccurrence,
    depth: usize,
) -> Result<(), PublishedValueError> {
    b.charge_visit()?;
    b.observe_depth(depth)?;
    b.charge_bytes(std::mem::size_of::<crate::ProviderOccurrence>())?;
    if let Some(origin) = p.original_origin() {
        b.import(origin.arena().clone())?;
        return Ok(());
    }
    match p.identity() {
        ProviderIdentity::Builtin(n) => text(b, n)?,
        ProviderIdentity::User(id) => {
            text(b, id.source_label())?;
            text(b, id.exported_name())?;
        }
    }
    for (n, v) in p.fields() {
        text(b, n)?;
        super::audit(b, v, depth + 1)?;
    }
    if let Some(f) = p.files_to_run() {
        files_to_run(b, f, depth + 1)?;
    }
    Ok(())
}
pub(super) fn audit(
    b: &mut PublishedValueArenaBuilder,
    v: &AnalysisValue,
    depth: usize,
) -> Result<(), PublishedValueError> {
    match v.kind() {
        AnalysisValueKind::List(values) | AnalysisValueKind::Tuple(values) => {
            for v in values {
                super::audit(b, v, depth + 1)?;
            }
        }
        AnalysisValueKind::Dictionary(values) => {
            for (k, v) in values {
                super::audit(b, k, depth + 1)?;
                super::audit(b, v, depth + 1)?;
            }
        }
        AnalysisValueKind::Struct(values) => {
            for (n, v) in values {
                text(b, n)?;
                super::audit(b, v, depth + 1)?;
            }
        }
        AnalysisValueKind::Provider(p) => provider(b, p, depth + 1)?,
        AnalysisValueKind::ConfiguredTarget(t) => configured_target(b, t, depth)?,
        AnalysisValueKind::CcToolchainVariables(v) => variables(b, v, depth + 1)?,
        AnalysisValueKind::PackageSpecificationInfo(v) => package_groups(b, v, depth + 1)?,
        _ => return Err(PublishedValueError::Unsupported("unmodeled owner input")),
    }
    Ok(())
}
pub(super) fn configured_target(
    b: &mut PublishedValueArenaBuilder,
    target: &crate::ConfiguredTargetValue,
    depth: usize,
) -> Result<(), PublishedValueError> {
    if let Some(key) = target.identity().configured() {
        configured_key(b, key)?;
    } else {
        label(b, target.identity().label())?;
    }
    for (_, provider) in target.providers().iter() {
        provider_value(b, provider, depth + 1)?;
    }
    Ok(())
}
fn variables(
    b: &mut PublishedValueArenaBuilder,
    v: &crate::cc_variables::CcToolchainVariables,
    depth: usize,
) -> Result<(), PublishedValueError> {
    b.charge_visit()?;
    b.observe_depth(depth)?;
    b.charge_bytes(std::mem::size_of::<crate::cc_variables::CcVariablesModel>())?;
    let m = v.model();
    if let Some(parent) = &m.parent {
        variables(b, parent, depth + 1)?;
    }
    for (n, v) in &m.locals {
        text(b, n)?;
        super::audit(b, v, depth + 1)?;
    }
    for (n, c) in &m.cache {
        text(b, n)?;
        for v in c.values() {
            super::audit(b, v, depth + 1)?;
        }
        if let crate::cc_variables::CcVariableCacheEntry::Error(e) = c {
            text(b, e)?;
        }
    }
    Ok(())
}
pub(super) fn package_groups(
    b: &mut PublishedValueArenaBuilder,
    v: &crate::PackageSpecificationInfo,
    depth: usize,
) -> Result<(), PublishedValueError> {
    depset::inspect(b, v.groups(), depth, |b, g, _| {
        charge_package_group_storage(b, package_group_storage_bytes(g))?;
        for p in g
            .exact_positive()
            .iter()
            .chain(g.subtree_positive())
            .chain(g.exact_negative().iter())
            .chain(g.subtree_negative())
        {
            text(b, p.repo().as_str())?;
            text(b, p.package().as_str())?;
        }
        Ok(())
    })
}

pub(super) fn charge_package_group_storage(
    b: &mut PublishedValueArenaBuilder,
    bytes: Option<usize>,
) -> Result<(), PublishedValueError> {
    // An unrepresentable forecast follows the same poisoning path as an
    // ordinary byte-budget refusal, including strict native admission.
    b.charge_storage(crate::DepsetStorageCharge {
        nodes: 0,
        edges: 4,
        visits: 1,
        bytes: bytes.unwrap_or(usize::MAX),
    })
}

pub(super) fn package_subtree_storage_bytes(len: usize) -> Option<usize> {
    (2 * std::mem::size_of::<usize>())
        .checked_add(len.checked_mul(std::mem::size_of::<slug_identity_v2::PackageIdentifier>())?)
}

/// Includes the group Arc, both actual set backings and both subtree Arc slices.
/// Inline package identifiers are paid here; their owned text is audited above.
fn package_group_storage_bytes(g: &crate::PackageGroupContents) -> Option<usize> {
    let arc_header = 2 * std::mem::size_of::<usize>();
    let mut bytes = std::mem::size_of::<crate::PackageGroupContents>().checked_add(arc_header)?;
    for set in [g.exact_positive(), g.exact_negative()] {
        bytes = bytes.checked_add(set.retained_storage_bytes_upper_bound()?)?;
    }
    for slice in [g.subtree_positive(), g.subtree_negative()] {
        bytes = bytes.checked_add(package_subtree_storage_bytes(slice.len())?)?;
    }
    Some(bytes)
}
