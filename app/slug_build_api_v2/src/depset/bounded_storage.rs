//! Borrowed physical-store inspection. Accounting precedes traversal growth.
use super::*;

/// Incremental physical retention/work forecast, not a semantic depset order.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct DepsetStorageCharge {
    pub nodes: usize,
    pub edges: usize,
    pub visits: usize,
    pub bytes: usize,
}
/// Every physical leaf and node in the retained dense-store closure is visited,
/// including rows not reachable from the selected logical root. Metadata may
/// itself own storage; callers inspect it through `Node` after the store charge.
pub enum DepsetStorageEvent<'a, T, M> {
    Charge(DepsetStorageCharge),
    Node { metadata: &'a M, depth: usize },
    Value { value: &'a T, depth: usize },
}
struct Stored<T, M> {
    // An owning guard prevents address reuse across successive root inspections.
    _store: Arc<DenseDepsetStore<T, M>>,
    enclosing_depth: usize,
}
/// Scratch memo for ONE consistent inspection/budget policy. Successful visits
/// retain store guards; they do not contribute semantic value identity. A
/// callback failure poisons the session, preventing partial audits from reuse.
pub struct DepsetStorageVisitSet<T, M = ()> {
    stores: FxHashMap<usize, Stored<T, M>>,
    poisoned: bool,
}
impl<T, M> Default for DepsetStorageVisitSet<T, M> {
    fn default() -> Self {
        Self {
            stores: FxHashMap::default(),
            poisoned: false,
        }
    }
}
impl<T, M> fmt::Debug for DepsetStorageVisitSet<T, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DepsetStorageVisitSet")
            .field("stores", &self.stores.len())
            .field("poisoned", &self.poisoned)
            .finish()
    }
}
impl<T, M> DepsetStorageVisitSet<T, M> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn len(&self) -> usize {
        self.stores.len()
    }
    pub fn is_empty(&self) -> bool {
        self.stores.is_empty()
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum DepsetStorageVisitError<E> {
    Visitor(E),
    Poisoned,
}
impl<E: fmt::Display> fmt::Display for DepsetStorageVisitError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Visitor(e) => e.fmt(f),
            Self::Poisoned => f.write_str("depset storage inspection session is poisoned"),
        }
    }
}
impl<E: Error + 'static> Error for DepsetStorageVisitError<E> {}
impl<T, M> Depset<T, M> {
    /// Inspect retained storage without flattening, value hashing, or cloning.
    /// The callback enforces its shared budget on `Charge`, before either the
    /// visited map or pending stack grows. Events have no semantic order.
    pub fn visit_storage_bounded<'a, E>(
        &'a self,
        mut visitor: impl FnMut(DepsetStorageEvent<'a, T, M>) -> Result<(), E>,
    ) -> Result<(), E> {
        inspect(self, &mut DepsetStorageVisitSet::new(), 0, &mut visitor)
    }
    /// Reuse a physical audit session across roots. The enclosing depth is an
    /// audit context, not a semantic depset identity. At a greater enclosing
    /// depth ALL metadata/payload callbacks replay, charging bounded work but
    /// not duplicate retained storage. This also checks nested payload depth;
    /// a cached physical depth alone cannot establish that bound. Node/Value
    /// depth fields are absolute, including the supplied enclosing depth.
    pub fn visit_storage_bounded_with<'a, E>(
        &'a self,
        seen: &mut DepsetStorageVisitSet<T, M>,
        enclosing_depth: usize,
        mut visitor: impl FnMut(DepsetStorageEvent<'a, T, M>) -> Result<(), E>,
    ) -> Result<(), DepsetStorageVisitError<E>> {
        if seen.poisoned {
            return Err(DepsetStorageVisitError::Poisoned);
        }
        match inspect(self, seen, enclosing_depth, &mut visitor) {
            Ok(()) => Ok(()),
            Err(error) => {
                seen.poisoned = true;
                Err(DepsetStorageVisitError::Visitor(error))
            }
        }
    }
}
fn inspect<'a, T, M, E>(
    root: &'a Depset<T, M>,
    seen: &mut DepsetStorageVisitSet<T, M>,
    enclosing_depth: usize,
    visitor: &mut impl FnMut(DepsetStorageEvent<'a, T, M>) -> Result<(), E>,
) -> Result<(), E> {
    let mut pending = Vec::new();
    discover(&root.store, seen, enclosing_depth, &mut pending, visitor)?;
    while let Some(store) = pending.pop() {
        let mut maximum_depth = 0;
        for node in &store.nodes {
            maximum_depth = maximum_depth.max(node.depth);
            visitor(DepsetStorageEvent::Node {
                metadata: &node.metadata,
                depth: enclosing_depth.saturating_add(node.depth),
            })?;
        }
        for value in &store.leaves {
            visitor(DepsetStorageEvent::Value {
                value,
                depth: enclosing_depth.saturating_add(maximum_depth),
            })?;
        }
        for value in &store.external_depsets {
            discover(&value.store, seen, enclosing_depth, &mut pending, visitor)?;
        }
        for row in &store.external_rows {
            discover(&row.store, seen, enclosing_depth, &mut pending, visitor)?;
        }
    }
    Ok(())
}
fn discover<'a, T, M, E>(
    store: &'a Arc<DenseDepsetStore<T, M>>,
    seen: &mut DepsetStorageVisitSet<T, M>,
    enclosing_depth: usize,
    pending: &mut Vec<&'a DenseDepsetStore<T, M>>,
    visitor: &mut impl FnMut(DepsetStorageEvent<'a, T, M>) -> Result<(), E>,
) -> Result<(), E> {
    let identity = Arc::as_ptr(store) as usize;
    let previous = seen.stores.get(&identity).map(|s| s.enclosing_depth);
    if previous.is_some_and(|depth| depth >= enclosing_depth) {
        return visitor(DepsetStorageEvent::Charge(DepsetStorageCharge {
            visits: 1,
            ..DepsetStorageCharge::default()
        }));
    }
    let foreign = store
        .external_depsets
        .len()
        .saturating_add(store.external_rows.len());
    let mut charge = DepsetStorageCharge {
        nodes: 0,
        edges: 0,
        bytes: 2 * std::mem::size_of::<usize>(),
        visits: 1usize
            .saturating_add(store.nodes.len())
            .saturating_add(store.successors.len())
            .saturating_add(store.leaves.len())
            .saturating_add(foreign),
    };
    if previous.is_none() {
        charge.nodes = store.nodes.len();
        charge.edges = store.successors.len().saturating_add(foreign);
        charge.bytes = std::mem::size_of::<DenseDepsetStore<T, M>>()
            .saturating_add(std::mem::size_of_val(store.nodes.as_ref()))
            .saturating_add(std::mem::size_of_val(store.successors.as_ref()))
            .saturating_add(std::mem::size_of_val(store.leaves.as_ref()))
            .saturating_add(std::mem::size_of_val(store.external_depsets.as_ref()))
            .saturating_add(std::mem::size_of_val(store.external_rows.as_ref()))
            .saturating_add(8 * std::mem::size_of::<usize>());
    }
    visitor(DepsetStorageEvent::Charge(charge))?;
    if let Some(entry) = seen.stores.get_mut(&identity) {
        entry.enclosing_depth = enclosing_depth;
    } else {
        seen.stores.insert(
            identity,
            Stored {
                _store: store.clone(),
                enclosing_depth,
            },
        );
    }
    pending.push(store.as_ref());
    Ok(())
}

#[cfg(test)]
#[path = "bounded_storage_tests.rs"]
mod tests;
