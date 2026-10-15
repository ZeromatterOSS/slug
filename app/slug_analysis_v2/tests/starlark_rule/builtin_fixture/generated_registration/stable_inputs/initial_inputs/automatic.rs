//! Automatic authored-source views omit only complete admitted output roots.
use std::mem::size_of;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::WorkspaceDirectoryEntry;
use slug_workspace_v2::WorkspaceDirectoryEntryKind;
use slug_workspace_v2::WorkspaceDirectorySnapshot;
use slug_workspace_v2::WorkspaceDirectoryValue;
use slug_workspace_v2::WorkspaceFileValue;
use slug_workspace_v2::WorkspaceRawSnapshot;
use slug_workspace_v2::WorkspaceSnapshot;

use super::super::super::Budget;
// Paths are known complete native incarnation roots, never guessed by spelling.
pub(super) fn collect(
    workspace: &Path,
    roots: &[PathBuf],
    budget: &mut Budget,
) -> Result<
    (
        Arc<WorkspaceSnapshot>,
        Arc<WorkspaceRawSnapshot>,
        Arc<WorkspaceDirectorySnapshot>,
    ),
    String,
> {
    budget.charge(
        1,
        size_of::<Vec<PathBuf>>() * 4 + workspace.as_os_str().len() * 4,
    )?;
    let mut pending = vec![workspace.to_path_buf()];
    let mut files = Vec::new();
    let mut directories = Vec::new();
    while let Some(directory) = pending.pop() {
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(&directory).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            // Linux fixture filesystem discovery: NAME_MAX is255 bytes
            // (/usr/include/linux/limits.h). A4096-byte envelope conservatively
            // admits the owned OsString and allocator/header rounding BEFORE
            // file_name(), then actual full prefix+name credits the path copies.
            budget.charge(1, 4096 + 4 * size_of::<std::ffi::OsString>())?;
            let name = entry.file_name();
            if name.len() > 255 {
                return Err("fixture unsupported directory component width".into());
            }
            let width = directory
                .as_os_str()
                .len()
                .checked_add(name.len())
                .and_then(|n| n.checked_add(2))
                .ok_or("fixture discovery path width")?;
            budget.charge(
                roots.len() + 1,
                width.checked_mul(8).ok_or("fixture automatic path")?,
            )?;
            let path = directory.join(&name);
            if roots.iter().any(|root| root == &path) {
                continue;
            }
            let kind = entry.file_type().map_err(|e| e.to_string())?;
            budget.charge(
                1,
                16 * (size_of::<WorkspaceDirectoryEntry>() + size_of::<PathBuf>()) + name.len() * 4,
            )?;
            let kind = if kind.is_dir() {
                pending.push(path.clone());
                WorkspaceDirectoryEntryKind::Directory
            } else if kind.is_file() {
                let bytes = usize::try_from(entry.metadata().map_err(|e| e.to_string())?.len())
                    .map_err(|_| "fixture automatic file length")?;
                budget.charge(
                    1,
                    bytes
                        .checked_mul(16)
                        .and_then(|n| {
                            n.checked_add(16 * size_of::<(PathBuf, WorkspaceFileValue)>())
                        })
                        .ok_or("fixture automatic file")?,
                )?;
                files.push((
                    path.clone(),
                    WorkspaceFileValue::Present(Arc::new(
                        std::fs::read_to_string(&path).map_err(|e| e.to_string())?,
                    )),
                ));
                WorkspaceDirectoryEntryKind::RegularFile
            } else if kind.is_symlink() {
                WorkspaceDirectoryEntryKind::Symlink
            } else {
                WorkspaceDirectoryEntryKind::Other
            };
            entries.push(WorkspaceDirectoryEntry {
                name: name.to_str().ok_or("fixture non-Unicode name")?.into(),
                kind,
            });
        }
        budget.charge(1, 16 * size_of::<(PathBuf, WorkspaceDirectoryValue)>())?;
        budget.charge(
            entries
                .len()
                .checked_mul(entries.len().checked_ilog2().unwrap_or(0) as usize + 1)
                .ok_or("fixture directory sort work")?,
            0,
        )?;
        directories.push((directory, WorkspaceDirectoryValue::present(entries)));
    }
    // Complete map/sort/transfer and derived observation output are admitted
    // before their allocation. The existing deterministic source fields and
    // constant fixture metadata are reused, not rewritten by a new policy.
    let count = files
        .len()
        .checked_add(directories.len())
        .ok_or("fixture source count")?;
    budget.charge(
        count.checked_mul(48).ok_or("fixture source records")?,
        count
            .max(4)
            .checked_mul(32 * (size_of::<PathBuf>() + size_of::<WorkspaceFileValue>()))
            .ok_or("fixture source transfer")?,
    )?;
    let text = Arc::new(WorkspaceSnapshot {
        files: Arc::new(files.into_iter().collect()),
    });
    let raw = crate::raw_snapshot_from_text(&text);
    let directories = Arc::new(WorkspaceDirectorySnapshot {
        directories: Arc::new(directories.into_iter().collect()),
    });
    Ok((text, raw, directories))
}

/// Construct Host authority only for Automatic mode. Explicit mode never
/// allocates or pays for a Host Epoch that it would immediately discard.
pub(in super::super) fn host_epoch(
    workspace: &Path,
    text: &WorkspaceSnapshot,
    directories: &WorkspaceDirectorySnapshot,
    budget: &mut Budget,
) -> Result<PathObservationEpoch, String> {
    let ancestors = workspace.components().count();
    // The assembler stops at an already-known ancestor; every directory Lstat
    // is emitted once. This filesystem directory superset includes empty dirs,
    // so2F+3D+ancestors+8 bounds all final rows, fixed missing rows and the two
    // optional platform listings without repeating an ancestor per file.
    let observation_bound = text
        .files
        .len()
        .checked_mul(2)
        .and_then(|n| n.checked_add(directories.directories.len().checked_mul(3)?))
        .and_then(|n| n.checked_add(ancestors))
        .and_then(|n| n.checked_add(8))
        .ok_or("fixture observation count")?;
    let pair = size_of::<(
        slug_workspace_v2::PathObservationDemand,
        Arc<slug_workspace_v2::PathObservationResult>,
    )>();
    for (path, value) in text.files.iter() {
        budget.charge(
            2,
            path.as_os_str()
                .len()
                .checked_add(32)
                .and_then(|n| n.checked_mul(32))
                .and_then(|n| n.checked_add(pair * 32))
                .ok_or("fixture file observation storage")?,
        )?;
        if let WorkspaceFileValue::Present(value) = value {
            budget.charge(
                1,
                value
                    .len()
                    .checked_mul(8)
                    .ok_or("fixture source observation bytes")?,
            )?;
        }
    }
    for path in directories
        .directories
        .keys()
        .map(PathBuf::as_path)
        .chain(workspace.ancestors())
    {
        budget.charge(
            3,
            path.as_os_str()
                .len()
                .checked_add(32)
                .and_then(|n| n.checked_mul(48))
                .and_then(|n| n.checked_add(pair * 48))
                .ok_or("fixture directory observation storage")?,
        )?;
    }
    let comparisons = observation_bound.checked_ilog2().unwrap_or(0) as usize + 1;
    // Set discovery includes one paid failed encounter per file and at most D
    // successful parent insertions; each directory also checks two BUILD names.
    let discovery = text
        .files
        .len()
        .checked_add(
            directories
                .directories
                .len()
                .checked_mul(4)
                .ok_or("fixture parent discovery")?,
        )
        .and_then(|n| n.checked_add(3))
        .and_then(|n| n.checked_mul(comparisons))
        .ok_or("fixture parent comparison work")?;
    budget.charge(discovery, 0)?;
    // Ordered row construction, Epoch sorting and ordered final transfer are
    // bounded by the unique-row upper bound, not repeated per-file ancestors.
    budget.charge(
        observation_bound
            .checked_mul(comparisons)
            .and_then(|n| n.checked_mul(3))
            .ok_or("fixture observation sort")?,
        observation_bound
            .max(4)
            .checked_mul(pair * 16)
            .and_then(|n| {
                n.checked_add(
                    workspace
                        .as_os_str()
                        .len()
                        .checked_add(32)?
                        .checked_mul(comparisons)?
                        .checked_mul(16)?,
                )
            })
            .ok_or("fixture observation peak")?,
    )?;
    Ok(crate::observation_epoch_from_snapshot(
        workspace,
        text,
        std::iter::empty::<PathBuf>(),
    ))
}
