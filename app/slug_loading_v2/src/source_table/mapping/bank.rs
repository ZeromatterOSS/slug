//! Borrowed producer-local lookup. Actual guards outlive every address key.
use starlark_map::small_map::SmallMap;

use super::*;
use crate::bzl_environment::publication::declaration_storage;
#[path = "bank_finish.rs"]
mod finish;
enum Entry<'a> {
    Inherited(&'a MappingPair),
    New { raw: &'a Raw, prepared: Prepared },
}
pub(crate) struct Bank<'a> {
    index: SmallMap<usize, usize>,
    entries: Vec<(Entry<'a>, bool)>,
    uses: Vec<usize>,
    active: Option<(usize, bool)>,
    capacity: usize,
    rows: usize,
    used: usize,
}
impl<'a> Bank<'a> {
    pub(crate) fn new(capacity: usize, rows: usize, work: &mut Budget) -> Result<Self, String> {
        work.charge(4, std::mem::size_of::<Self>())?;
        let map = crate::bzl_environment::references::descriptor_memo_scratch_bytes(capacity)?;
        let slots = std::alloc::Layout::array::<(Entry<'a>, bool)>(capacity)
            .map_err(|_| "mapping entry scratch")?
            .size();
        let row_bytes = std::alloc::Layout::array::<usize>(rows)
            .map_err(|_| "mapping ordinal scratch")?
            .size();
        let pairs = std::alloc::Layout::array::<Option<MappingPair>>(capacity)
            .map_err(|_| "mapping pair scratch")?
            .size();
        let arrays = slots
            .checked_mul(2)
            .and_then(|b| row_bytes.checked_mul(2).and_then(|r| b.checked_add(r)))
            .and_then(|b| pairs.checked_mul(5).and_then(|p| b.checked_add(p)))
            .and_then(|b| b.checked_add(12 * std::mem::size_of::<usize>()))
            .and_then(|b| b.checked_add(map))
            .ok_or("mapping scratch overflow")?;
        work.charge(4, arrays)?;
        Ok(Self {
            index: SmallMap::with_capacity(capacity),
            entries: Vec::with_capacity(capacity),
            uses: Vec::with_capacity(rows),
            active: None,
            capacity,
            rows,
            used: 0,
        })
    }
    pub(crate) fn inherit(
        &mut self,
        pair: &'a MappingPair,
        work: &mut Budget,
    ) -> Result<(), String> {
        work.charge(4, 0)?;
        let key = pair.pointer();
        if self.index.contains_key(&key) {
            return Ok(());
        }
        self.insert(key, Entry::Inherited(pair))
    }
    fn insert(&mut self, key: usize, entry: Entry<'a>) -> Result<(), String> {
        if self.entries.len() == self.capacity {
            return Err("mapping bank capacity".into());
        }
        self.index.insert(key, self.entries.len());
        self.entries.push((entry, false));
        Ok(())
    }
    /// A copy forecast may borrow only an already-sealed inherited proof.
    /// Taking this borrowed bank does not promote even a completed New entry.
    pub(crate) fn copy_volume(
        &self,
        raw: &Raw,
        work: &mut Budget,
    ) -> Result<Option<(usize, usize)>, String> {
        // Raw borrow/address, index/slot/tag, two scalar reads and result copy.
        // Admit the whole control envelope before reading any bank state.
        work.charge(
            8,
            std::mem::size_of::<(
                &Raw,
                usize,
                Option<&usize>,
                &(Entry<'_>, bool),
                Option<(usize, usize)>,
            )>(),
        )?;
        let key = Arc::as_ptr(raw) as *const () as usize;
        Ok(match self.index.get(&key) {
            Some(position) => match &self.entries[*position].0 {
                Entry::Inherited(pair) => Some((pair.copy.records, pair.copy.bytes)),
                Entry::New { .. } => None,
            },
            None => None,
        })
    }
    pub(in crate::source_table) fn select(
        &mut self,
        raw: &'a Raw,
        work: &mut Budget,
    ) -> Result<(), String> {
        work.charge(6, 0)?;
        let key = Arc::as_ptr(raw) as *const () as usize;
        let position = match self.index.get(&key) {
            Some(p) => *p,
            None => {
                let p = self.entries.len();
                let prepared = Prepared::new(raw.len(), work)?;
                self.insert(key, Entry::New { raw, prepared })?;
                p
            }
        };
        if self.uses.len() == self.rows {
            return Err("mapping row capacity".into());
        }
        let first = !self.entries[position].1;
        if first {
            self.used = self.used.checked_add(1).ok_or("mapping used count")?;
            self.entries[position].1 = true;
        }
        self.uses.push(position);
        self.active = Some((position, first));
        Ok(())
    }
    pub(in crate::source_table) fn needs_preparation(
        &self,
        work: &mut Budget,
    ) -> Result<bool, String> {
        // Active-view copy/check, selected entry/tag borrow, result/dispatch.
        // All scalar temporaries are admitted before any bank field is read.
        work.charge(
            3,
            std::mem::size_of::<(Option<(usize, bool)>, &Entry<'_>, bool)>(),
        )?;
        let (position, first) = self.active.ok_or("mapping row missing")?;
        let entry = &self.entries[position].0;
        Ok(first && matches!(entry, Entry::New { .. }))
    }
    pub(in crate::source_table) fn entry(
        &mut self,
        a: &ApparentRepoName,
        c: &CanonicalRepoName,
        work: &mut Budget,
    ) -> Result<(), String> {
        work.charge(1, 0)?;
        let (position, first) = self.active.ok_or("mapping row missing")?;
        if first {
            if let Entry::New { prepared, .. } = &mut self.entries[position].0 {
                prepared.entry(a, c, work)?;
            }
        }
        Ok(())
    }
    fn shape_at(&self, position: usize) -> Result<PublicationShape, String> {
        match &self.entries[position].0 {
            Entry::Inherited(pair) => Ok(pair.shape()),
            Entry::New { prepared, .. } => prepared.shape(),
        }
    }
    pub(in crate::source_table) fn shapes(&self) -> impl Iterator<Item = PublicationShape> + '_ {
        self.uses
            .iter()
            .map(|p| self.shape_at(*p).expect("mapping completion checked"))
    }
    pub(in crate::source_table) fn storage(&self) -> Result<PublicationShape, String> {
        let mut shape = declaration_storage(self.used)?;
        shape.bytes = self
            .used
            .checked_mul(std::mem::size_of::<MappingPair>())
            .and_then(|n| n.checked_add(2 * std::mem::size_of::<usize>()))
            .ok_or("mapping bank storage")?;
        shape.visits = self
            .used
            .checked_mul(4)
            .and_then(|n| n.checked_add(1))
            .ok_or("mapping bank facts")?;
        shape.edges = self.used.checked_mul(2).ok_or("mapping bank edges")?;
        Ok(shape)
    }
    pub(in crate::source_table) fn metadata_storage(&self) -> Result<PublicationShape, String> {
        let mut shape = self.storage()?;
        let child = declaration_storage(self.rows)?;
        shape.edges = shape
            .edges
            .checked_add(child.edges)
            .and_then(|e| self.used.checked_mul(2).and_then(|n| e.checked_add(n)))
            .ok_or("mapping metadata edges")?;
        shape.visits = shape
            .visits
            .checked_add(child.visits)
            .and_then(|e| self.used.checked_mul(4).and_then(|n| e.checked_add(n)))
            .ok_or("mapping metadata visits")?;
        shape.bytes = shape
            .bytes
            .checked_add(child.bytes)
            .and_then(|e| {
                self.used
                    .checked_mul(header_bytes())
                    .and_then(|n| e.checked_add(n))
            })
            .ok_or("mapping metadata bytes")?;
        shape.depth = 5;
        Ok(shape)
    }
}

#[cfg(test)]
#[path = "bank_tests.rs"]
mod tests;
