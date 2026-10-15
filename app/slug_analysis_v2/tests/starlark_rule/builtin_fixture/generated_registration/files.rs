//! File-only cfgtest IO. No production materializer or execution is replaced.
use std::collections::BTreeSet;
use std::fs;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use slug_bzlmod_v2::GeneratedRepositoryOrderedEffectRef as Effect;
use slug_bzlmod_v2::RepositoryMaterializationEpochEntry;
use slug_bzlmod_v2::RepositoryMaterializationKind;
use slug_bzlmod_v2::RepositoryMaterializationRequest;
use slug_bzlmod_v2::RepositoryMaterializationResult;
use slug_bzlmod_v2::RepositoryMaterializationSuccess;
use slug_bzlmod_v2::SourcePreparationNeeds;
use slug_bzlmod_v2::digest_module_file_content;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathDirectoryEntries;
use slug_workspace_v2::PathDirectoryEntry;
use slug_workspace_v2::PathDirectoryEntryKind;
use slug_workspace_v2::PathDirectoryName;
use slug_workspace_v2::PathLstat;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationInstanceId;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;

type Observations = Vec<(PathObservationDemand, Arc<PathObservationResult>)>;

pub(super) fn preflight(
    workspace: &Path,
    need: &SourcePreparationNeeds,
    budget: &mut super::Budget,
) -> Result<(), String> {
    for request in need.repository_materializations().values() {
        // Even an empty plan retains a root and ancestor/absence observations.
        let ancestors = workspace
            .components()
            .count()
            .checked_add(8)
            .ok_or("fixture root depth overflow")?;
        let root_bytes = workspace
            .as_os_str()
            .len()
            .checked_add(128 + 1024)
            .and_then(|n| n.checked_mul(ancestors))
            .and_then(|n| n.checked_mul(32))
            .and_then(|n| n.checked_add(std::mem::size_of::<RepositoryMaterializationEpochEntry>()))
            .ok_or("fixture root storage overflow")?;
        budget.charge(
            ancestors
                .checked_mul(32)
                .and_then(|n| n.checked_add(64))
                .ok_or("fixture root records overflow")?,
            root_bytes,
        )?;
        let RepositoryMaterializationKind::GeneratedFileEffects(plan) = &request.kind else {
            unreachable!()
        };
        for effect in plan.ordered_effects() {
            let Effect::File(file) = effect else {
                unreachable!()
            };
            let path = file.path();
            if path.is_empty()
                || (path.contains('\\') || path.contains('\0'))
                || !path
                    .split('/')
                    .all(|part| !part.is_empty() && part != "." && part != "..")
                || !Path::new(path)
                    .components()
                    .all(|c| matches!(c, Component::Normal(_)))
            {
                return Err("generated registration fixture invalid path".into());
            }
            let depth = Path::new(path)
                .components()
                .count()
                .checked_add(workspace.components().count())
                .and_then(|n| n.checked_add(8))
                .ok_or("fixture depth overflow")?;
            // Conservative complete tree/ancestor/absence/listing+move storage.
            let width = workspace
                .as_os_str()
                .len()
                .checked_add(128)
                .and_then(|n| n.checked_add(path.len()))
                .ok_or("fixture path storage overflow")?;
            let bytes = width
                .checked_add(1024)
                .and_then(|n| n.checked_mul(depth))
                .and_then(|n| n.checked_mul(32))
                .and_then(|n| n.checked_add(file.content().len().checked_mul(3)?))
                .ok_or("fixture tree storage overflow")?;
            budget.charge(
                (depth + 8)
                    .checked_mul(32)
                    .ok_or("fixture record overflow")?,
                bytes,
            )?;
            for other in plan.ordered_effects() {
                budget.charge(1, 0)?;
                let Effect::File(other) = other else {
                    unreachable!()
                };
                if path != other.path()
                    && (Path::new(path).starts_with(other.path())
                        || Path::new(other.path()).starts_with(path))
                {
                    return Err("generated registration fixture file/directory conflict".into());
                }
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
pub(super) fn write(
    workspace: &Path,
    request: &Arc<RepositoryMaterializationRequest>,
    next: &mut u64,
) -> Result<(RepositoryMaterializationEpochEntry, Observations), String> {
    use std::os::unix::fs::PermissionsExt;
    let RepositoryMaterializationKind::GeneratedFileEffects(plan) = &request.kind else {
        unreachable!()
    };
    let mut fresh = None;
    for _ in 0..64 {
        let id = *next;
        *next = next.checked_add(1).ok_or("fixture instance overflow")?;
        let root = workspace.join(format!(".slug_generated_registration_{id}"));
        match fs::create_dir(&root) {
            Ok(()) => {
                fresh = Some((root, PathObservationInstanceId::new(id)));
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    let (root, instance) = fresh.ok_or("no fresh generated fixture directory")?;
    let mut files = BTreeSet::new();
    for effect in plan.ordered_effects() {
        let Effect::File(file) = effect else {
            unreachable!()
        };
        let path = root.join(file.path());
        fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        fs::write(&path, file.content()).map_err(|e| e.to_string())?;
        fs::set_permissions(
            &path,
            fs::Permissions::from_mode(if file.executable() { 0o755 } else { 0o644 }),
        )
        .map_err(|e| e.to_string())?;
        files.insert(path);
    }
    // Identity follows the actual final tree, including overwritten files/mode.
    let mut tree = files.clone();
    tree.insert(root.clone());
    for path in &files {
        let mut parent = path.parent();
        while let Some(path) = parent.filter(|p| p.starts_with(&root)) {
            tree.insert(path.to_path_buf());
            parent = path.parent();
        }
    }
    let mut identity = Vec::new();
    for path in &tree {
        let relative = path
            .strip_prefix(&root)
            .unwrap()
            .to_str()
            .ok_or("non-Unicode generated fixture path")?;
        identity.extend_from_slice(&(relative.len() as u64).to_le_bytes());
        identity.extend_from_slice(relative.as_bytes());
        let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        identity.push(if metadata.is_dir() {
            0
        } else if metadata.is_file() {
            1
        } else {
            return Err("unexpected generated fixture node".into());
        });
        identity.extend_from_slice(&(metadata.permissions().mode() & 0o777).to_le_bytes());
        if metadata.is_file() {
            identity.extend_from_slice(
                digest_module_file_content(fs::read(path).map_err(|e| e.to_string())?).as_bytes(),
            );
        }
    }
    let observed = observe(&root, instance, &files)?;
    let entry = RepositoryMaterializationEpochEntry {
        request: request.clone(),
        result: RepositoryMaterializationResult::Success(
            RepositoryMaterializationSuccess::Immutable {
                source_identity: digest_module_file_content(identity).into(),
                generation_root: root,
                observation_instance: instance,
            },
        ),
    };
    Ok((entry, observed))
}

#[cfg(not(unix))]
pub(super) fn write(
    _: &Path,
    _: &Arc<RepositoryMaterializationRequest>,
    _: &mut u64,
) -> Result<(RepositoryMaterializationEpochEntry, Observations), String> {
    Err("generated registration fixture requires Unix executable modes".into())
}

#[cfg(unix)]
fn lstat(path: &Path) -> Result<PathObservationResult, String> {
    use std::os::unix::fs::MetadataExt;
    match fs::symlink_metadata(path) {
        Ok(m) => {
            let kind = if m.is_dir() {
                PathNodeKind::Directory
            } else if m.is_file() {
                PathNodeKind::RegularFile
            } else {
                return Err("unexpected non-file generated fixture node".into());
            };
            Ok(PathObservationResult::Lstat(PathOperationResult::Present(
                PathLstat::new(
                    kind,
                    m.size() as i64,
                    m.mtime()
                        .wrapping_mul(1000)
                        .wrapping_add(m.mtime_nsec() / 1_000_000),
                    m.ctime()
                        .wrapping_mul(1000)
                        .wrapping_add(m.ctime_nsec() / 1_000_000),
                    m.ino() as i64,
                    (m.mode() & 0o777) as i32,
                ),
            )))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Ok(PathObservationResult::Lstat(PathOperationResult::Missing))
        }
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(unix)]
fn observe(
    root: &Path,
    instance: PathObservationInstanceId,
    files: &BTreeSet<PathBuf>,
) -> Result<Observations, String> {
    let namespace = PathObservationNamespace::Materialization(instance);
    let mut out = Vec::new();
    let mut put = |path: &Path, value: PathObservationResult| -> Result<(), String> {
        out.push((
            PathObservationDemand::new(
                namespace,
                NormalizedAbsolutePath::new(path.to_path_buf()).map_err(|e| e.to_string())?,
                value.operation(),
            ),
            Arc::new(value),
        ));
        Ok(())
    };
    let mut directories = BTreeSet::from([root.to_path_buf()]);
    for path in files {
        put(path, lstat(path)?)?;
        put(
            path,
            PathObservationResult::FileBytes(PathOperationResult::Present(
                fs::read(path).map_err(|e| e.to_string())?.into(),
            )),
        )?;
        let mut parent = path.parent();
        while let Some(path) = parent.filter(|p| p.starts_with(root)) {
            directories.insert(path.to_path_buf());
            parent = path.parent();
        }
    }
    for directory in &directories {
        put(directory, lstat(directory)?)?;
        let entries = fs::read_dir(directory)
            .map_err(|e| e.to_string())?
            .map(|entry| {
                let entry = entry.map_err(|e| e.to_string())?;
                let kind = if entry.file_type().map_err(|e| e.to_string())?.is_dir()
                    && directories.contains(&entry.path())
                {
                    PathDirectoryEntryKind::Directory
                } else if entry.file_type().map_err(|e| e.to_string())?.is_file()
                    && files.contains(&entry.path())
                {
                    PathDirectoryEntryKind::File
                } else {
                    return Err("unexpected generated fixture directory entry".into());
                };
                Ok(PathDirectoryEntry::new(
                    PathDirectoryName::new(entry.file_name()).map_err(|e| e.to_string())?,
                    kind,
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        put(
            directory,
            PathObservationResult::DirectoryEntries(PathOperationResult::Present(
                PathDirectoryEntries::new(entries),
            )),
        )?;
        for name in [
            "MODULE.bazel",
            "REPO.bazel",
            ".bazelignore",
            "BUILD",
            "BUILD.bazel",
        ] {
            let path = directory.join(name);
            if !files.contains(&path) {
                put(&path, lstat(&path)?)?;
            }
        }
    }
    let mut ancestor = root.parent();
    while let Some(path) = ancestor {
        put(path, lstat(path)?)?;
        ancestor = path.parent();
    }
    Ok(out)
}
