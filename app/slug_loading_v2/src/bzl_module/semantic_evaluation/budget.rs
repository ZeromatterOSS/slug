//! Deterministic semantic charges and separately bounded preparation scratch.
use super::*;

pub(crate) mod peak_context;
use peak_context::PeakContext;

mod refusal;

pub(crate) const RECORD_LIMIT: usize = 65_536;
pub(crate) const BYTE_LIMIT: usize = 64 * 1024 * 1024;
pub(crate) const DEPTH_LIMIT: usize = 256;
const HEADER: usize = 2 * std::mem::size_of::<usize>();

#[derive(Debug, Clone, Copy, Default, Allocative)]
pub(crate) struct Budget {
    pub(crate) records: usize,
    pub(crate) bytes: usize,
    pub(crate) depth: usize,
}
impl Budget {
    pub(crate) fn charge(&mut self, records: usize, bytes: usize) -> Result<(), String> {
        self.records = self.records.checked_add(records).ok_or_else(limit)?;
        self.bytes = self.bytes.checked_add(bytes).ok_or_else(limit)?;
        if self.records > RECORD_LIMIT || self.bytes > BYTE_LIMIT {
            return Err(limit());
        }
        Ok(())
    }
    pub(crate) fn charge_with_context(
        &mut self,
        context: PeakContext,
        records: usize,
        bytes: usize,
    ) -> Result<(), String> {
        let before = *self;
        self.charge(records, bytes)
            .map_err(|error| refusal::charge(context, before, *self, records, bytes, error))
    }
    pub(crate) fn context(&self, phase: &'static str, error: String) -> String {
        format!(
            "{phase}: {error} (records={} bytes={} depth={})",
            self.records, self.bytes, self.depth
        )
    }
    pub(crate) fn depth(&mut self, depth: usize) -> Result<(), String> {
        self.depth = self.depth.max(depth);
        if self.depth > DEPTH_LIMIT {
            return Err(limit());
        }
        Ok(())
    }
    pub(crate) fn depth_with_context(
        &mut self,
        context: PeakContext,
        depth: usize,
    ) -> Result<(), String> {
        let before = *self;
        self.depth(depth)
            .map_err(|error| refusal::depth(context, before, *self, depth, error))
    }
    pub(crate) fn identity(&mut self, identity: &BzlModuleIdentity) -> Result<(), String> {
        identity_charges(identity, |records, bytes| self.charge(records, bytes))
    }
    pub(crate) fn identity_with_context(
        &mut self,
        context: PeakContext,
        identity: &BzlModuleIdentity,
    ) -> Result<(), String> {
        identity_charges(identity, |records, bytes| {
            self.charge_with_context(context, records, bytes)
        })
    }
    pub(crate) fn array<T>(&mut self, count: usize) -> Result<(), String> {
        self.charge(0, array_bytes::<T>(count))
    }
    pub(crate) fn array_with_context<T>(
        &mut self,
        context: PeakContext,
        count: usize,
    ) -> Result<(), String> {
        self.charge_with_context(context, 0, array_bytes::<T>(count))
    }
    pub(crate) fn scratch(&mut self, slots: usize, item_size: usize) -> Result<(), String> {
        self.charge(1, scratch_bytes(slots, item_size))
    }
    pub(crate) fn scratch_with_context(
        &mut self,
        context: PeakContext,
        slots: usize,
        item_size: usize,
    ) -> Result<(), String> {
        self.charge_with_context(context, 1, scratch_bytes(slots, item_size))
    }
}

fn array_bytes<T>(count: usize) -> usize {
    HEADER.saturating_add(count.saturating_mul(std::mem::size_of::<T>()))
}
fn scratch_bytes(slots: usize, item_size: usize) -> usize {
    // Conservative fixed map-node credit and complete next Vec allocation.
    1024usize.saturating_add(slots.saturating_mul(item_size))
}
fn identity_charges(
    identity: &BzlModuleIdentity,
    mut charge: impl FnMut(usize, usize) -> Result<(), String>,
) -> Result<(), String> {
    let label = &identity.label;
    let raw = std::mem::size_of::<BzlModuleIdentity>()
        .saturating_add(label.package().repo().as_str().len())
        .saturating_add(label.package().package().as_str().len())
        .saturating_add(label.target().as_str().len())
        .saturating_add(label.mapping_id().map_or(0, |id| id.as_str().len()))
        .saturating_add(identity.source.retained_text_bytes().unwrap_or(usize::MAX))
        .saturating_add(HEADER);
    charge(1, raw)?;
    for (apparent, canonical) in &*identity.repository_mapping {
        charge(
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

pub(crate) fn limit() -> String {
    "semantic .bzl evaluation input resource limit".into()
}

pub(crate) fn limit_with_context(context: PeakContext) -> String {
    refusal::arithmetic(context)
}

/// Physical peak admission is separate from deterministic logical/event
/// credits. Check the aggregate before any covered allocation can grow.
pub(crate) fn admit_peak(
    context: PeakContext,
    logical: Budget,
    scratch: Budget,
    extra: usize,
) -> Result<(), String> {
    let base = logical
        .bytes
        .checked_add(scratch.bytes)
        .ok_or_else(|| refusal::peak_overflow(context, logical, scratch, extra, None))?;
    let bytes = base
        .checked_add(extra)
        .ok_or_else(|| refusal::peak_overflow(context, logical, scratch, extra, Some(base)))?;
    if bytes > BYTE_LIMIT {
        return Err(format!(
            "semantic preparation joint peak: {} (logical_records={} logical_bytes={} scratch_records={} scratch_bytes={} extra_bytes={} owner={} phase={} first_ledger={})",
            limit(),
            logical.records,
            logical.bytes,
            scratch.records,
            scratch.bytes,
            extra,
            context.owner(),
            context.phase(),
            context.first_ledger()
        ));
    }
    Ok(())
}

/// Forecast both fixed map/candidate storage and complete next borrowed Vec
/// allocation before entry/or_default/push can mutate either collection.
pub(crate) fn admit_index_growth<T>(
    context: PeakContext,
    logical: Budget,
    scratch: &mut Budget,
    borrowed: usize,
    candidates: usize,
) -> Result<(), String> {
    let mut proposed = *scratch;
    let work_context = context.with_ledger(peak_context::FirstLedger::Work);
    proposed.scratch_with_context(
        work_context,
        borrowed.saturating_add(1).saturating_mul(2),
        std::mem::size_of::<T>(),
    )?;
    proposed.scratch_with_context(
        work_context,
        candidates.saturating_add(1).saturating_mul(2),
        std::mem::size_of::<usize>(),
    )?;
    admit_peak(context, logical, proposed, 0)?;
    *scratch = proposed;
    Ok(())
}

#[cfg(test)]
mod tests;
