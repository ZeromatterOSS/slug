//! Flat exact source inputs: equality never recursively expands a load diamond.
use std::hash::Hash;
use std::hash::Hasher;

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(super) struct StableKey {
    pub(super) identity: BzlModuleIdentity,
    pub(super) source_name: CompactString,
    pub(super) context: BzlModuleContext,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(super) struct Binding {
    pub(super) raw: Arc<str>,
    pub(super) child: Arc<StableKey>,
}
#[derive(Debug, Allocative)]
pub(super) struct LocalFacts {
    pub(super) key: Arc<StableKey>,
    pub(super) source: Arc<[u8]>,
    pub(super) bindings: Arc<[Binding]>,
    pub(super) budget: budget::Budget,
}
impl PartialEq for LocalFacts {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key && self.source == other.source && self.bindings == other.bindings
    }
}
impl Eq for LocalFacts {}
impl Hash for LocalFacts {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.key.hash(h);
        self.source.hash(h);
        self.bindings.hash(h);
    }
}

pub(super) fn row_budget<'a>(
    identity: &BzlModuleIdentity,
    source_name: &str,
    source: &[u8],
    children: impl Iterator<Item = (&'a str, &'a StableKey)>,
) -> Result<budget::Budget, String> {
    let mut b = budget::Budget::default();
    b.charge(
        1,
        std::mem::size_of::<LocalFacts>()
            + std::mem::size_of::<StableKey>()
            + 4 * std::mem::size_of::<usize>(),
    )?;
    b.identity(identity)?;
    b.charge(
        1,
        source_name
            .len()
            .saturating_add(source.len())
            .saturating_add(2 * std::mem::size_of::<usize>()),
    )?;
    // Reserve HEAD's parser-owned source even when the inherited live parser
    // shares bytes. Equal complete inputs keep identical event admission; the
    // owned projection copies this String only on an actual semantic miss.
    reserve_parser_source_copy(&mut b, source.len())?;
    let mut count = 0usize;
    for (raw, child) in children {
        b.charge(
            1,
            raw.len().saturating_add(2 * std::mem::size_of::<usize>()),
        )?;
        // Derived Binding Eq/Hash visits the COMPLETE stable key at every
        // ordered occurrence, even when the immutable child Arc is shared.
        // Logical admission must forecast that repeated work before any copy;
        // actual payload sharing is only a separate physical-retention fact.
        b.charge(
            1,
            std::mem::size_of::<StableKey>().saturating_add(child.source_name.len()),
        )?;
        b.identity(&child.identity)?;
        count = count.checked_add(1).ok_or_else(budget::limit)?;
    }
    b.array::<Binding>(count)?;
    Ok(b)
}

pub(super) fn reserve_parser_source_copy(
    budget: &mut budget::Budget,
    source_len: usize,
) -> Result<(), String> {
    budget.charge(0, source_len.saturating_add(std::mem::size_of::<String>()))
}
