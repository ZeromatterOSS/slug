//! Deterministic semantic charges and separately bounded preparation scratch.
use super::*;

pub(super) const RECORD_LIMIT: usize = 65_536;
pub(super) const BYTE_LIMIT: usize = 64 * 1024 * 1024;
pub(super) const DEPTH_LIMIT: usize = 256;
const HEADER: usize = 2 * std::mem::size_of::<usize>();

#[derive(Debug, Clone, Copy, Default, Allocative)]
pub(super) struct Budget {
    pub(super) records: usize,
    pub(super) bytes: usize,
    pub(super) depth: usize,
}
impl Budget {
    pub(super) fn charge(&mut self, records: usize, bytes: usize) -> Result<(), String> {
        self.records = self.records.checked_add(records).ok_or_else(limit)?;
        self.bytes = self.bytes.checked_add(bytes).ok_or_else(limit)?;
        if self.records > RECORD_LIMIT || self.bytes > BYTE_LIMIT {
            return Err(limit());
        }
        Ok(())
    }
    pub(super) fn depth(&mut self, depth: usize) -> Result<(), String> {
        self.depth = self.depth.max(depth);
        if self.depth > DEPTH_LIMIT {
            return Err(limit());
        }
        Ok(())
    }
    pub(super) fn identity(&mut self, identity: &BzlModuleIdentity) -> Result<(), String> {
        let label = &identity.label;
        let raw = std::mem::size_of::<BzlModuleIdentity>()
            .saturating_add(label.package().repo().as_str().len())
            .saturating_add(label.package().package().as_str().len())
            .saturating_add(label.target().as_str().len())
            .saturating_add(label.mapping_id().map_or(0, |id| id.as_str().len()))
            .saturating_add(identity.workspace_path.as_os_str().len())
            .saturating_add(HEADER);
        self.charge(1, raw)?;
        for (apparent, canonical) in &*identity.repository_mapping {
            self.charge(
                1,
                std::mem::size_of::<(
                    slug_identity_v2::ApparentRepoName,
                    slug_identity_v2::CanonicalRepoName,
                )>()
                .saturating_add(apparent.as_str().len())
                .saturating_add(canonical.as_str().len()),
            )?;
        }
        Ok(())
    }
    pub(super) fn array<T>(&mut self, count: usize) -> Result<(), String> {
        self.charge(
            0,
            HEADER.saturating_add(count.saturating_mul(std::mem::size_of::<T>())),
        )
    }
    pub(super) fn scratch(&mut self, slots: usize, item_size: usize) -> Result<(), String> {
        // Conservative fixed map-node credit and complete next Vec allocation.
        self.charge(1, 1024usize.saturating_add(slots.saturating_mul(item_size)))
    }
}
pub(super) fn limit() -> String {
    "semantic .bzl evaluation input resource limit".into()
}

/// Physical peak admission is separate from deterministic logical/event
/// credits. Check the aggregate before any covered allocation can grow.
pub(super) fn admit_peak(logical: Budget, scratch: Budget, extra: usize) -> Result<(), String> {
    let bytes = logical
        .bytes
        .checked_add(scratch.bytes)
        .and_then(|bytes| bytes.checked_add(extra))
        .ok_or_else(limit)?;
    if bytes > BYTE_LIMIT {
        return Err(limit());
    }
    Ok(())
}

/// Forecast both fixed map/candidate storage and complete next borrowed Vec
/// allocation before entry/or_default/push can mutate either collection.
pub(super) fn admit_index_growth<T>(
    logical: Budget,
    scratch: &mut Budget,
    borrowed: usize,
    candidates: usize,
) -> Result<(), String> {
    let mut proposed = *scratch;
    proposed.scratch(
        borrowed.saturating_add(1).saturating_mul(2),
        std::mem::size_of::<T>(),
    )?;
    proposed.scratch(
        candidates.saturating_add(1).saturating_mul(2),
        std::mem::size_of::<usize>(),
    )?;
    admit_peak(logical, proposed, 0)?;
    *scratch = proposed;
    Ok(())
}

#[cfg(test)]
mod tests;
