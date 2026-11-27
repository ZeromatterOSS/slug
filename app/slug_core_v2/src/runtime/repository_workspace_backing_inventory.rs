//! Bounded inspection of the exclusive command at Source bootstrap refusal.
use super::super::super::mount_lease::quota::full_archive_usage_for_probe;
use super::super::super::source::SourceRootInput;
use super::*;
use crate::runtime::probe_observer::workspace_progress::BackingInventory;
use crate::runtime::probe_observer::workspace_progress::SealedArchiveBacking;

#[derive(Clone, Copy)]
enum SlotClass {
    MutableArchive,
    MutableProcess,
    SealedProcess,
    SealedCompactArchive,
    SealedFullGeneralArchive,
    SealedFullSourceArchive,
}

impl SlotClass {
    fn record(self, inventory: &mut BackingInventory) {
        inventory.root_counts[self as usize] += 1;
        inventory.root_debit_bytes += match self {
            Self::MutableArchive
            | Self::SealedFullGeneralArchive
            | Self::SealedFullSourceArchive => ARCHIVE_BYTES,
            Self::MutableProcess | Self::SealedProcess | Self::SealedCompactArchive => {
                PROCESS_BYTES
            }
        };
    }
}

fn sample_sealed_full(inventory: &mut BackingInventory, family: u64, slot: usize, file: &File) {
    let Some(row) = inventory.sealed_full.iter_mut().find(|row| row.is_none()) else {
        inventory.complete = false;
        return;
    };
    let usage = full_archive_usage_for_probe(file).ok();
    *row = Some(SealedArchiveBacking {
        family,
        slot: slot as u64,
        allocated_bytes: usage.map(|(allocated, _)| allocated),
        used_inodes: usage.map(|(_, used_inodes)| used_inodes),
    });
    inventory.complete &= usage.is_some();
}

impl RepositoryWorkspaceCommand {
    pub(super) fn backing_inventory_for_probe(&self, input: &SourceRootInput) -> BackingInventory {
        let mut inventory = BackingInventory {
            root_counts: [0; 6],
            root_debit_bytes: 0,
            bank_live_bytes: self.resources.snapshot().backing_bytes,
            held_source_capture_bytes: input.backing_bytes_for_probe(),
            unassigned_backing_bytes: None,
            sealed_full: [None; 4],
            complete: true,
        };

        // Each slot owns either mutable progress or one finalized view. Actor
        // rows and historical read-view dependencies can alias these backings.
        for (index, slot) in self.roots.iter().enumerate() {
            match (&slot.progress, &slot.process_view) {
                (Some(progress), None) => {
                    let builder = &progress.builder;
                    if builder
                        .backing
                        .matches(&self.resources, builder.mode)
                        .is_err()
                    {
                        inventory.complete = false;
                        continue;
                    }
                    match builder.mode {
                        RepositoryWorkspaceMode::Archive => SlotClass::MutableArchive,
                        RepositoryWorkspaceMode::Process => SlotClass::MutableProcess,
                    }
                    .record(&mut inventory);
                }
                (None, Some(view)) => {
                    let Some(root) = view.data.root() else {
                        inventory.complete = false;
                        continue;
                    };
                    let frozen = root.frozen_archive_quota();
                    if root
                        .backing
                        .matches_data(&self.resources, root.mode, frozen)
                        .is_err()
                    {
                        inventory.complete = false;
                        continue;
                    }
                    match (root.mode, frozen) {
                        (RepositoryWorkspaceMode::Process, None) => {
                            SlotClass::SealedProcess.record(&mut inventory);
                        }
                        (
                            RepositoryWorkspaceMode::Archive,
                            Some(FrozenArchiveQuota::CompactArchive),
                        ) => {
                            SlotClass::SealedCompactArchive.record(&mut inventory);
                        }
                        (
                            RepositoryWorkspaceMode::Archive,
                            Some(FrozenArchiveQuota::FullArchive),
                        ) => {
                            SlotClass::SealedFullGeneralArchive.record(&mut inventory);
                            sample_sealed_full(&mut inventory, 0, index, &root.file);
                        }
                        _ => inventory.complete = false,
                    }
                }
                _ => inventory.complete = false,
            }
        }
        for (index, slot) in self.source_slots.iter().enumerate() {
            let Some(root) = slot
                .process_view
                .as_ref()
                .and_then(|view| view.data.source_root())
            else {
                inventory.complete = false;
                continue;
            };
            let frozen = root.frozen_archive_quota();
            if root
                .backing
                .matches_data(&self.resources, RepositoryWorkspaceMode::Archive, frozen)
                .is_err()
            {
                inventory.complete = false;
                continue;
            }
            match frozen {
                Some(FrozenArchiveQuota::CompactArchive) => {
                    SlotClass::SealedCompactArchive.record(&mut inventory);
                }
                Some(FrozenArchiveQuota::FullArchive) => {
                    SlotClass::SealedFullSourceArchive.record(&mut inventory);
                    sample_sealed_full(&mut inventory, 1, index, &root.file);
                }
                None => inventory.complete = false,
            }
        }

        inventory.unassigned_backing_bytes = inventory
            .bank_live_bytes
            .checked_sub(inventory.root_debit_bytes)
            .and_then(|remaining| remaining.checked_sub(inventory.held_source_capture_bytes));
        inventory.complete &= inventory.unassigned_backing_bytes == Some(0);
        inventory
    }
}
