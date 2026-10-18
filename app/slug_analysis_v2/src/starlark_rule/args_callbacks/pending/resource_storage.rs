//! Positive full private Box payloads; no new cost on Ready/ArgsWrite arrays.
use std::alloc::Layout;
use std::mem::align_of;
use std::mem::size_of;

use starlark::environment::Module;

use super::PendingSpawn;
use super::PreparedSpawn;

pub(super) const CONTROL_WORK: usize = 4;
pub(super) const CONTROL_BYTES: usize = 8 * size_of::<usize>();
fn admit<T>(charge: &mut impl FnMut(usize, usize) -> Result<(), String>) -> Result<(), String> {
    // Debit BEFORE layout, checked arithmetic, field copies or Box allocation.
    charge(CONTROL_WORK, CONTROL_BYTES)?;
    let bytes = Layout::new::<T>()
        .pad_to_align()
        .size()
        .checked_add(size_of::<Box<T>>())
        .and_then(|n| n.checked_add(align_of::<T>()))
        .and_then(|n| n.checked_add(8 * size_of::<usize>()))
        .ok_or("resource_set private Spawn layout overflow")?;
    let facts = bytes
        .checked_add(size_of::<usize>() - 1)
        .ok_or("resource_set private Spawn facts overflow")?
        / size_of::<usize>();
    charge(facts, bytes)
}
pub(super) fn pending(module: &Module) -> anyhow::Result<()> {
    admit::<PendingSpawn>(&mut |r, b| {
        slug_loading_v2::resource_set::prepay_action_storage(module, r, b)
            .map_err(|e| e.to_string())
    })
    .map_err(anyhow::Error::msg)
}
pub(super) fn prepared<R>(
    charge: &mut impl FnMut(usize, usize) -> Result<(), String>,
) -> Result<(), String> {
    admit::<PreparedSpawn<R>>(charge)
}

pub(super) fn build_info_pending(module: &Module) -> anyhow::Result<()> {
    admit::<super::build_info::PendingBuildInfo>(&mut |r, b| {
        slug_loading_v2::resource_set::prepay_action_storage(module, r, b)
            .map_err(|e| e.to_string())
    })
    .map_err(anyhow::Error::msg)
}
pub(super) fn build_info_prepared(
    charge: &mut impl FnMut(usize, usize) -> Result<(), String>,
) -> Result<(), String> {
    admit::<super::build_info::PreparedBuildInfo>(charge)
}
#[cfg(test)]
mod tests;

pub(in crate::starlark_rule) fn build_info_header(module: &Module) -> anyhow::Result<()> {
    admit::<slug_build_api_v2::BuildInfoTransformHeader>(&mut |r, b| {
        slug_loading_v2::resource_set::prepay_action_storage(module, r, b)
            .map_err(|e| e.to_string())
    })
    .map_err(anyhow::Error::msg)
}
