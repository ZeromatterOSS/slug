//! Bounded inspection of the exclusive command at Source bootstrap refusal.
use super::super::super::mount_lease::quota::archive_usage_for_probe;
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
    fn record(self, inventory: &mut BackingInventory, debit_bytes: u64) {
        inventory.root_counts[self as usize] += 1;
        inventory.root_debit_bytes += debit_bytes;
    }
}

#[derive(Clone, Copy)]
enum ArchiveFamily {
    General,
    Source,
}

fn record_sealed_archive(
    inventory: &mut BackingInventory,
    family: ArchiveFamily,
    slot: usize,
    file: &File,
    class: FrozenArchiveQuota,
) {
    let bytes = class.bytes();
    if bytes < ARCHIVE_BYTES {
        SlotClass::SealedCompactArchive.record(inventory, bytes);
        return;
    }
    match family {
        ArchiveFamily::General => SlotClass::SealedFullGeneralArchive,
        ArchiveFamily::Source => SlotClass::SealedFullSourceArchive,
    }
    .record(inventory, bytes);
    let Some(row) = inventory.sealed_full.iter_mut().find(|row| row.is_none()) else {
        inventory.complete = false;
        return;
    };
    let usage = archive_usage_for_probe(file, class.quota()).ok();
    *row = Some(SealedArchiveBacking {
        family: family as u64,
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
                    .record(&mut inventory, quota(builder.mode).0);
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
                            SlotClass::SealedProcess.record(&mut inventory, PROCESS_BYTES);
                        }
                        (RepositoryWorkspaceMode::Archive, Some(class)) => {
                            record_sealed_archive(
                                &mut inventory,
                                ArchiveFamily::General,
                                index,
                                &root.file,
                                class,
                            );
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
                Some(class) => {
                    record_sealed_archive(
                        &mut inventory,
                        ArchiveFamily::Source,
                        index,
                        &root.file,
                        class,
                    );
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
