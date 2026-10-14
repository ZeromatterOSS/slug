//! One shared preflight for every typed retained depset channel.
use super::*;
use crate::Depset;
use crate::DepsetStorageEvent;

pub(super) fn inspect<T, M>(
    b: &mut PublishedValueArenaBuilder,
    d: &Depset<T, M>,
    depth: usize,
    mut value: impl FnMut(&mut PublishedValueArenaBuilder, &T, usize) -> Result<(), PublishedValueError>,
) -> Result<(), PublishedValueError> {
    class_depth(d.depth())?;
    b.observe_depth(depth)?;
    d.visit_storage_bounded(|e| event(b, e, depth, &mut value))
}
pub(super) fn event<T, M>(
    b: &mut PublishedValueArenaBuilder,
    event: DepsetStorageEvent<'_, T, M>,
    depth: usize,
    value: &mut impl FnMut(
        &mut PublishedValueArenaBuilder,
        &T,
        usize,
    ) -> Result<(), PublishedValueError>,
) -> Result<(), PublishedValueError> {
    match event {
        DepsetStorageEvent::Charge(c) => b.charge_storage(c),
        DepsetStorageEvent::Node {
            depth: node_depth, ..
        } => {
            class_depth(node_depth)?;
            b.observe_depth(depth)
        }
        DepsetStorageEvent::Value {
            value: v,
            depth: node_depth,
        } => {
            class_depth(node_depth)?;
            let payload_depth = depth.saturating_add(1);
            b.observe_depth(payload_depth)?;
            value(b, v, payload_depth)
        }
    }
}

// A typed depset's cached DAG depth is separate from ordinary value containment.
pub(super) fn class_depth(depth: usize) -> Result<(), PublishedValueError> {
    if depth > crate::depset::MAX_DEPTH {
        Err(PublishedValueError::Limit("depset depth"))
    } else {
        Ok(())
    }
}
