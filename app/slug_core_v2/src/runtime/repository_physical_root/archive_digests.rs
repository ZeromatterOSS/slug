//! Optional scalar receipts for exact entries of the committed frozen Archive.

use std::fmt;
use std::mem::size_of;

use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;

use super::*;
use crate::runtime::path_observation::rooted::ArchiveDigestReceipt;

// This bounds optional acceleration, not observation admission. Rows beyond
// this limit or unavailable native credit use the ordinary full read.
const MAX_ROWS: usize = 1024;

#[derive(Default)]
pub(super) struct ArchiveDigestReceipts {
    rows: Vec<(usize, ArchiveDigestReceipt)>,
    // The actual allocation retires before its bank reservation.
    credit: Option<ResourceCredit>,
}

impl fmt::Debug for ArchiveDigestReceipts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ArchiveDigestReceipts")
            .field("len", &self.rows.len())
            .finish()
    }
}

impl ArchiveDigestReceipts {
    fn get(&self, entry: usize) -> Option<ArchiveDigestReceipt> {
        self.index(entry).ok().map(|index| self.rows[index].1)
    }

    fn index(&self, entry: usize) -> Result<usize, usize> {
        self.rows.binary_search_by_key(&entry, |(index, _)| *index)
    }

    fn insert(
        &mut self,
        entry: usize,
        receipt: ArchiveDigestReceipt,
        resources: &Arc<RepositoryRootResources>,
    ) {
        let Err(index) = self.index(entry) else {
            return;
        };
        if self.rows.len() == self.rows.capacity() && !self.grow(resources) {
            return;
        }
        self.rows.insert(index, (entry, receipt));
    }

    fn grow(&mut self, resources: &Arc<RepositoryRootResources>) -> bool {
        let capacity = (self.rows.capacity() * 2).clamp(8, MAX_ROWS);
        if capacity <= self.rows.capacity() {
            return false;
        }
        // Hold the previous credit while the replacement allocates/moves.
        let bytes = capacity * size_of::<(usize, ArchiveDigestReceipt)>() + 64;
        let Ok(credit) = resources.reserve(bytes, 0, 0) else {
            return false;
        };
        // Allocate separately so allocator over-reservation can safely fall
        // back while preserving the already admitted rows and their credit.
        let mut rows = Vec::new();
        if rows.try_reserve_exact(capacity).is_err() || rows.capacity() != capacity {
            return false;
        }
        rows.append(&mut self.rows);
        self.rows = rows;
        self.credit = Some(credit);
        true
    }
}

/// This root borrow cannot escape a higher-level observation's ownership. Drop
/// unpublished rows on any failure; the mixed-batch caller decides when to adopt.
pub(in crate::runtime) struct PendingArchiveDigestReceipts<'root> {
    root: &'root RepositoryPhysicalRoot,
    receipts: ArchiveDigestReceipts,
}

impl<'root> PendingArchiveDigestReceipts<'root> {
    pub(super) fn new(root: &'root RepositoryPhysicalRoot) -> Self {
        Self {
            root,
            receipts: ArchiveDigestReceipts::default(),
        }
    }

    pub(super) fn belongs_to(&self, root: &RepositoryPhysicalRoot) -> bool {
        std::ptr::eq(self.root, root)
    }

    pub(in crate::runtime) fn matching(
        &self,
        entry: usize,
        demand: &PathObservationDemand,
        certificate: Option<&PathObservationEpoch>,
    ) -> Option<ArchiveDigestReceipt> {
        let PathObservationResult::FileDigest(PathOperationResult::Present(expected)) =
            certificate?.get(demand)?.as_ref()
        else {
            return None;
        };
        let receipt = self.root.archive_digests.lock().ok()?.get(entry)?;
        (receipt.digest() == *expected).then_some(receipt)
    }

    pub(in crate::runtime) fn record(&mut self, entry: usize, receipt: ArchiveDigestReceipt) {
        self.receipts.insert(entry, receipt, &self.root.resources);
    }

    pub(super) fn publish(self, root: &RepositoryPhysicalRoot) {
        if !self.belongs_to(root) || self.receipts.rows.is_empty() {
            return;
        }
        let Ok(mut committed) = root.archive_digests.lock() else {
            return;
        };
        if committed.rows.is_empty() {
            *committed = self.receipts;
        } else {
            for &(entry, receipt) in &self.receipts.rows {
                committed.insert(entry, receipt, &root.resources);
            }
        }
    }
}
