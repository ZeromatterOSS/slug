//! Checked native allocation bounds, folded into the one complete model walk.
use std::alloc::Layout;
use std::mem::size_of;

use compact_str::CompactString;

use super::*;
use crate::cc_features::*;

type Result = std::result::Result<(), PublishedValueError>;

pub(super) fn bytes(b: &mut PublishedValueArenaBuilder, bytes: Option<usize>) -> Result {
    b.charge_bytes(bytes.ok_or(PublishedValueError::Limit("feature storage bytes"))?)
}

fn arc_bytes<T>() -> Option<usize> {
    Layout::new::<[usize; 2]>()
        .extend(Layout::new::<T>())
        .ok()
        .map(|(layout, _)| layout.pad_to_align().size())
}

pub(super) fn array<T>(b: &mut PublishedValueArenaBuilder, values: &[T]) -> Result {
    bytes(b, values.len().checked_mul(size_of::<T>()))
}

pub(super) fn set<T>(b: &mut PublishedValueArenaBuilder, values: &CcSet<T>) -> Result {
    bytes(b, values.retained_storage_bytes_upper_bound())
}

pub(super) fn text(b: &mut PublishedValueArenaBuilder, value: &CompactString) -> Result {
    super::native::text(b, value)?;
    // Preserve the prior text allowance, including its inline representation.
    // capacity also covers spare heap storage; inline capacity is conservative.
    b.charge_bytes(value.capacity())
}

pub(super) fn model(b: &mut PublishedValueArenaBuilder, m: &CcToolchainFeatureModel) -> Result {
    bytes(b, arc_bytes::<CcToolchainFeatureModel>())?;
    array(b, &m.features)?;
    array(b, &m.action_configs)?;
    array(b, &m.artifact_name_patterns)?;
    array(b, &m.default_features_and_action_configs)
}

pub(super) fn feature(b: &mut PublishedValueArenaBuilder, f: &CcFeature) -> Result {
    array(b, &f.env_sets)?;
    array(b, &f.requires)?;
    array(b, &f.implies)?;
    array(b, &f.provides)
}

pub(super) fn action(b: &mut PublishedValueArenaBuilder, a: &CcActionConfig) -> Result {
    array(b, &a.tools)?;
    array(b, &a.implies)
}

pub(super) fn outer(b: &mut PublishedValueArenaBuilder, configuration: bool) -> Result {
    // Native wrapper allocation plus private occurrence allocation. These are
    // retained independently even when their inner model allocation is shared.
    b.charge_bytes(crate::analysis_value::published_projection_forecast())?;
    bytes(b, arc_bytes::<()>())?;
    if configuration {
        bytes(b, arc_bytes::<CcFeatureConfigurationModel>())?;
    }
    Ok(())
}

pub(super) fn configuration_sets(
    b: &mut PublishedValueArenaBuilder,
    c: &CcFeatureConfigurationModel,
) -> Result {
    set(b, &c.requested)?;
    set(b, &c.enabled_features)?;
    set(b, &c.enabled_action_configs)
}
