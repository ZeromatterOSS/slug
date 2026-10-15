//! Comparison work follows the complete descriptor, not its retained facade.
//! The caller admits every debit on its existing cumulative constructor ledger.
use std::mem::size_of;
use std::sync::Arc;

use compact_str::CompactString;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;

use super::super::BzlEnvironmentInputs;
use super::super::BzlEnvironmentMode;
use super::super::ExportSelection;
use super::limit;
use crate::BzlModuleIdentity;

type MappingPair = (ApparentRepoName, CanonicalRepoName);

pub(super) fn equal(
    left_payload: Option<usize>,
    left: &BzlEnvironmentInputs,
    right_payload: Option<usize>,
    right: &BzlEnvironmentInputs,
    mut pay: impl FnMut(usize, usize) -> Result<(), String>,
) -> Result<bool, String> {
    pay(2, 2 * size_of::<Option<usize>>())?;
    if left_payload != right_payload {
        return Ok(false);
    }
    pay(2, 2 * size_of::<BzlEnvironmentMode>())?;
    if left.mode != right.mode {
        return Ok(false);
    }
    pay(2, 2 * size_of::<u32>())?;
    if left.native_revision != right.native_revision {
        return Ok(false);
    }
    // Arc Eq for Eq payloads checks the actual guard before the slice. This
    // shortcut cannot apply to an independently equal foreign allocation.
    pay(2, 2 * size_of::<Arc<[ExportSelection]>>())?;
    if Arc::ptr_eq(&left.selections, &right.selections) {
        return Ok(true);
    }
    pay(2, 2 * size_of::<usize>())?;
    if left.selections.len() != right.selections.len() {
        return Ok(false);
    }
    for (a, b) in left.selections.iter().zip(right.selections.iter()) {
        if !text(&a.global, &b.global, &mut pay)?
            || !text(&a.export, &b.export, &mut pay)?
            || !identity(&a.module, &b.module, &mut pay)?
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn text(
    a: &CompactString,
    b: &CompactString,
    pay: &mut impl FnMut(usize, usize) -> Result<(), String>,
) -> Result<bool, String> {
    pay(2, 2 * size_of::<CompactString>())?;
    pay(2, bytes(a.len(), b.len())?)?;
    Ok(a == b)
}

fn identity(
    a: &BzlModuleIdentity,
    b: &BzlModuleIdentity,
    pay: &mut impl FnMut(usize, usize) -> Result<(), String>,
) -> Result<bool, String> {
    // Typed label equality includes optional mapping_id. All its text is
    // prepaid here; no formatted label or fingerprint replaces that equality.
    pay(2, 2 * size_of::<CanonicalLabel>())?;
    pay(2, bytes(label_bytes(&a.label)?, label_bytes(&b.label)?)?)?;
    if a.label != b.label {
        return Ok(false);
    }
    pay(2, 2 * size_of::<slug_build_api_v2::ModuleSourceOrigin>())?;
    pay(
        2,
        bytes(
            a.source.retained_text_bytes().ok_or_else(limit)?,
            b.source.retained_text_bytes().ok_or_else(limit)?,
        )?,
    )?;
    // In particular, Workspace PathBuf Eq compares components. Raw lengths
    // bound work only: unequal lengths do not imply unequal path identities.
    if a.source != b.source {
        return Ok(false);
    }
    pay(2, 2 * size_of::<Arc<[MappingPair]>>())?;
    if Arc::ptr_eq(&a.repository_mapping, &b.repository_mapping) {
        return Ok(true);
    }
    pay(2, 2 * size_of::<usize>())?;
    if a.repository_mapping.len() != b.repository_mapping.len() {
        return Ok(false);
    }
    for ((aa, ac), (ba, bc)) in a.repository_mapping.iter().zip(b.repository_mapping.iter()) {
        pay(2, 2 * size_of::<MappingPair>())?;
        pay(2, bytes(aa.as_str().len(), ba.as_str().len())?)?;
        if aa != ba {
            return Ok(false);
        }
        pay(2, bytes(ac.as_str().len(), bc.as_str().len())?)?;
        if ac != bc {
            return Ok(false);
        }
    }
    Ok(true)
}

fn label_bytes(label: &CanonicalLabel) -> Result<usize, String> {
    let package = label.package();
    let size = bytes(
        package.repo().as_str().len(),
        package.package().as_str().len(),
    )?;
    let size = bytes(size, label.target().as_str().len())?;
    bytes(size, label.mapping_id().map_or(0, |id| id.as_str().len()))
}

fn bytes(a: usize, b: usize) -> Result<usize, String> {
    a.checked_add(b).ok_or_else(limit)
}

#[cfg(test)]
mod tests;
