//! Complete native source validation with one constructor-local mapping scan.
//! Full fact/byte volumes remain per occurrence; only actual row reads reuse a
//! borrowed immutable allocation guard. The caller prepays this entire memo.
use std::mem::align_of;
use std::mem::size_of;

use super::*;
use crate::published_value::PublishedValueError;

type Mapping = Arc<[(ApparentRepoName, CanonicalRepoName)]>;
struct Volume<'a> {
    guard: &'a Mapping,
    bytes: usize,
}
struct Memo<'a> {
    volumes: fxhash::FxHashMap<usize, Volume<'a>>,
    bound: usize,
}

pub(super) fn scratch_bytes(bound: usize) -> Result<usize, PublishedValueError> {
    if bound == 0 || bound > CALLABLE_GRAPH_NODE_LIMIT {
        return Err(PublishedValueError::Limit("source audit entries"));
    }
    // Full header, actual borrowed entry layout, table minimum/control and
    // alignment, conservative old/new coexistence, plus move temporary. This
    // maximum is paid at begin, before the first output Data clone.
    let cell = size_of::<(usize, Volume<'_>)>()
        .checked_add(16)
        .and_then(|n| n.checked_add(align_of::<(usize, Volume<'_>)>()));
    bound
        .max(4)
        .checked_mul(8)
        .and_then(|n| cell.and_then(|cell| n.checked_mul(cell)))
        .and_then(|n| n.checked_add(size_of::<Memo<'_>>()))
        .and_then(|n| n.checked_add(size_of::<(usize, Volume<'_>)>()))
        .ok_or(PublishedValueError::Limit("source audit bytes"))
}
fn identities(data: &CallableSourceAuthorityData) -> impl Iterator<Item = &CallableModuleIdentity> {
    std::iter::once(&data.root)
        .chain(data.direct_children.iter())
        .chain(data.environment_bindings.iter().map(|b| &b.module))
        .chain(data.reachable.iter().map(|s| &s.identity))
}
fn add(total: &mut usize, value: usize) -> Result<(), PublishedValueError> {
    *total = total
        .checked_add(value)
        .ok_or(PublishedValueError::Limit("source bytes"))?;
    Ok(())
}
impl<'a> Memo<'a> {
    fn module(
        &mut self,
        module: &'a CallableModuleIdentity,
        charge: &mut impl FnMut() -> Result<(), PublishedValueError>,
    ) -> Result<usize, PublishedValueError> {
        charge()?;
        let mut bytes = CallableSourceAuthority::module_prefix_bytes(module);
        // Both the probe and conditional actual-Arc guard comparison are paid
        // before the lookup. A miss keeps the conservative comparison charge.
        charge()?;
        charge()?;
        let mapping = &module.repository_mapping;
        let key = Arc::as_ptr(mapping) as *const () as usize;
        let volume = if let Some(volume) = self.volumes.get(&key) {
            if !Arc::ptr_eq(volume.guard, mapping) {
                return Err(PublishedValueError::Invalid("source mapping guard"));
            }
            volume.bytes
        } else {
            let count = self
                .volumes
                .len()
                .checked_add(1)
                .filter(|n| *n <= self.bound)
                .ok_or(PublishedValueError::Invalid("source audit capacity"))?;
            let mut volume = 2 * size_of::<usize>();
            for (apparent, canonical) in &**mapping {
                charge()?;
                add(
                    &mut volume,
                    size_of::<(ApparentRepoName, CanonicalRepoName)>(),
                )?;
                add(&mut volume, apparent.as_str().len())?;
                add(&mut volume, canonical.as_str().len())?;
            }
            charge()?;
            // with_capacity(bound) guarantees room for every admitted unique
            // mapping. No resize, owning guard clone or later byte admission.
            debug_assert!(count <= self.volumes.capacity());
            self.volumes.insert(
                key,
                Volume {
                    guard: mapping,
                    bytes: volume,
                },
            );
            volume
        };
        add(&mut bytes, volume)?;
        Ok(bytes)
    }
}

pub(super) fn prepare(
    data: CallableSourceAuthorityData,
    bound: usize,
    mut charge: impl FnMut() -> Result<(), PublishedValueError>,
) -> Result<PreparedCallableSourceAuthority, PublishedValueError> {
    charge()?;
    if data.direct_children.len() > CALLABLE_GRAPH_NODE_LIMIT
        || data.environment_bindings.len() > CALLABLE_GRAPH_NODE_LIMIT
        || data.reachable.len() > CALLABLE_GRAPH_NODE_LIMIT
    {
        return Err(PublishedValueError::Limit("source facts"));
    }
    let count = 1usize
        .checked_add(data.direct_children.len())
        .and_then(|n| n.checked_add(data.environment_bindings.len()))
        .and_then(|n| n.checked_add(data.reachable.len()))
        .filter(|n| bound != 0 && bound <= CALLABLE_GRAPH_NODE_LIMIT && *n <= bound)
        .ok_or(PublishedValueError::Invalid(
            "source authority identity bound",
        ))?;
    let mut facts = data.environment_bindings.len() * 2;
    for module in identities(&data) {
        charge()?;
        facts = facts
            .checked_add(1)
            .and_then(|n| n.checked_add(module.repository_mapping.len()))
            .ok_or(PublishedValueError::Limit("source facts"))?;
    }
    charge()?;
    if facts > CALLABLE_GRAPH_NODE_LIMIT {
        return Err(PublishedValueError::Limit("source facts"));
    }
    debug_assert!(count <= bound);
    let bytes = {
        // The length-only full-F pass must precede allocation/row scans, just
        // as ordinary new rejects source facts before source bytes.
        let mut memo = Memo {
            volumes: fxhash::FxHashMap::with_capacity_and_hasher(bound, Default::default()),
            bound,
        };
        let mut bytes = memo.module(&data.root, &mut charge)?;
        for module in &*data.direct_children {
            add(&mut bytes, memo.module(module, &mut charge)?)?;
        }
        for binding in &*data.environment_bindings {
            charge()?;
            let global = binding.global.capacity();
            charge()?;
            let export = binding.export.capacity();
            add(&mut bytes, memo.module(&binding.module, &mut charge)?)?;
            add(
                &mut bytes,
                size_of::<CallableEnvironmentBinding>() - size_of::<CallableModuleIdentity>(),
            )?;
            add(&mut bytes, global)?;
            add(&mut bytes, export)?;
        }
        for source in &*data.reachable {
            add(&mut bytes, memo.module(&source.identity, &mut charge)?)?;
            add(
                &mut bytes,
                size_of::<CallableModuleSource>() - size_of::<CallableModuleIdentity>(),
            )?;
        }
        add(
            &mut bytes,
            size_of::<CallableSourceAuthorityData>() - size_of::<CallableModuleIdentity>(),
        )?;
        add(
            &mut bytes,
            CallableSourceAuthority::storage_overhead_bytes(),
        )?;
        add(&mut bytes, 6 * size_of::<usize>())?;
        bytes
        // Every borrowed mapping guard/table is dropped before moving Data.
    };
    charge()?;
    if bytes > CALLABLE_GRAPH_BYTE_LIMIT {
        return Err(PublishedValueError::Limit("source bytes"));
    }
    Ok(PreparedCallableSourceAuthority {
        data,
        fact_count: facts,
        bytes,
    })
}

#[cfg(test)]
#[path = "source_audit/tests.rs"]
mod tests;
