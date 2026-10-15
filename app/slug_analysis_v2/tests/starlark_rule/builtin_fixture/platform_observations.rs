//! Initial direct listings; unsupported/oversized input leaves the actual Need.
use std::fs;
use std::mem::size_of;
use std::path::Path;

use slug_bzlmod_v2::Directive;
use slug_bzlmod_v2::ModuleFile;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathDirectoryEntries;
use slug_workspace_v2::PathDirectoryEntry;
use slug_workspace_v2::PathDirectoryEntryKind;
use slug_workspace_v2::PathDirectoryName;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;
type Observation = (PathObservationDemand, PathObservationResult);
struct Budget {
    records: usize,
    bytes: usize,
}
impl Budget {
    fn pay(&mut self, records: usize, bytes: usize) -> Option<()> {
        let records = self.records.checked_sub(records)?;
        let bytes = self.bytes.checked_sub(bytes)?;
        self.records = records;
        self.bytes = bytes;
        Some(())
    }
}
pub(in super::super) fn observations(workspace: &Path) -> [Option<Observation>; 2] {
    collect(workspace).unwrap_or([None, None])
}
fn collect(workspace: &Path) -> Option<[Option<Observation>; 2]> {
    // One cumulative allowance for both directories, never reset/refunded.
    let mut budget = Budget {
        records: 65_536,
        bytes: 64 * 1024 * 1024,
    };
    budget.pay(
        1,
        size_of::<Budget>().checked_add(size_of::<[Option<Observation>; 2]>())?,
    )?;
    budget.pay(
        0,
        workspace
            .as_os_str()
            .len()
            .checked_add(13)?
            .checked_mul(2)?
            .checked_add(size_of::<std::path::PathBuf>())?,
    )?;
    let module = workspace.join("MODULE.bazel");
    let len = usize::try_from(fs::metadata(&module).ok()?.len()).ok()?;
    budget.pay(1, len.checked_mul(2)?)?;
    let source = fs::read_to_string(module).ok()?;
    let parsed = ModuleFile::parse(&source).ok()?;
    let mut local = None;
    for directive in &parsed.directives {
        budget.pay(1, 0)?;
        match directive {
            Directive::Include(_) => return None,
            Directive::LocalPathOverride(v) if v.module_name == "platforms" => {
                if local.replace(v.path.as_str()).is_some() {
                    return None;
                }
            }
            Directive::SingleVersionOverride(v) if v.module_name == "platforms" => return None,
            Directive::MultipleVersionOverride(v) if v.module_name == "platforms" => return None,
            Directive::ArchiveOverride(v) if v.module_name == "platforms" => return None,
            Directive::GitOverride(v) if v.module_name == "platforms" => return None,
            _ => {}
        }
    }
    let local = local?;
    let mut result = [None, None];
    for (slot, leaf) in result.iter_mut().zip(["cpu", "os"]) {
        let path_bytes = workspace
            .as_os_str()
            .len()
            .checked_add(local.len())?
            .checked_add(leaf.len() + 2)?;
        // Complete request/output headers, path copies and move peak, before
        // owning paths. The existing initial epoch map owner is unchanged.
        budget.pay(
            1,
            path_bytes
                .checked_mul(4)?
                .checked_add(size_of::<Observation>().checked_mul(8)?)?
                .checked_add(128)?,
        )?;
        let path = workspace.join(local).join(leaf);
        if !fs::symlink_metadata(&path).ok()?.file_type().is_dir() {
            return None;
        }
        let entries = read_directory(&path, &mut budget)?;
        *slot = Some((
            PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new(path).ok()?,
                PathObservationOperation::DirectoryEntries,
            ),
            PathObservationResult::DirectoryEntries(PathOperationResult::Present(entries)),
        ));
    }
    Some(result)
}
fn read_directory(path: &Path, budget: &mut Budget) -> Option<PathDirectoryEntries> {
    let mut entries = Vec::new();
    budget.pay(0, size_of::<Vec<PathDirectoryEntry>>())?;
    for entry in fs::read_dir(path).ok()? {
        budget.pay(1, 0)?;
        let entry = entry.ok()?;
        let kind = entry.file_type().ok()?; // no-follow, as root_epoch_with_listings
        let kind = if kind.is_file() {
            PathDirectoryEntryKind::File
        } else if kind.is_dir() {
            PathDirectoryEntryKind::Directory
        } else if kind.is_symlink() {
            PathDirectoryEntryKind::Symlink
        } else {
            PathDirectoryEntryKind::Unknown
        };
        let name = entry.file_name();
        budget.pay(0, name.capacity().checked_mul(2)?)?;
        if entries.len() == entries.capacity() {
            let next = entries.capacity().checked_mul(2)?.max(4);
            // Actual old/new coexistence paid before explicit Vec growth.
            budget.pay(
                0,
                next.checked_add(entries.capacity())?
                    .checked_mul(size_of::<PathDirectoryEntry>())?,
            )?;
            entries
                .try_reserve_exact(next.checked_sub(entries.len())?)
                .ok()?;
        }
        entries.push(PathDirectoryEntry::new(
            PathDirectoryName::new(name).ok()?,
            kind,
        ));
    }
    // Sorting scratch, Vec-to-Arc backing and transfer peak. Names move, never
    // clone; their actual backing was charged before retained insertion.
    budget.pay(
        0,
        entries
            .len()
            .checked_mul(size_of::<PathDirectoryEntry>())?
            .checked_mul(4)?
            .checked_add(2 * size_of::<usize>())?,
    )?;
    Some(PathDirectoryEntries::new(entries))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_custom_local_listings_preserve_members_and_input_policy() {
        let root = super::super::super::scratch();
        let source =
            "module(name='root')\nlocal_path_override(module_name='platforms',path='custom')\n";
        fs::write(root.join("MODULE.bazel"), source).unwrap();
        for leaf in ["cpu", "os"] {
            fs::create_dir_all(root.join("custom").join(leaf)).unwrap();
        }
        fs::write(root.join("custom/os/authored.txt"), "actual").unwrap();
        fs::create_dir(root.join("custom/os/child")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("authored.txt", root.join("custom/os/link")).unwrap();
        let rows = observations(&root);
        assert!(rows.iter().all(Option::is_some));
        assert_eq!(
            rows,
            observations(&root),
            "repeat observation does not alter the input"
        );
        let (_, PathObservationResult::DirectoryEntries(PathOperationResult::Present(os))) =
            rows[1].as_ref().unwrap()
        else {
            panic!("actual listing");
        };
        assert!(
            os.entries()
                .iter()
                .any(|e| e.name().as_os_str() == "authored.txt"
                    && e.kind() == PathDirectoryEntryKind::File)
        );
        assert!(
            os.entries().iter().any(|e| e.name().as_os_str() == "child"
                && e.kind() == PathDirectoryEntryKind::Directory)
        );
        #[cfg(unix)]
        assert!(
            os.entries()
                .iter()
                .any(|e| e.name().as_os_str() == "link"
                    && e.kind() == PathDirectoryEntryKind::Symlink)
        );
        assert_eq!(
            fs::read_to_string(root.join("MODULE.bazel")).unwrap(),
            source
        );
        for source in [
            "module(",
            "module(name='root')\ninclude('//:part')\n",
            "module(name='root')\narchive_override(module_name='platforms',urls=['https://example.test/source'])\n",
        ] {
            fs::write(root.join("MODULE.bazel"), source).unwrap();
            assert!(observations(&root).iter().all(Option::is_none));
            assert_eq!(
                fs::read_to_string(root.join("MODULE.bazel")).unwrap(),
                source
            );
        }
        fs::remove_file(root.join("MODULE.bazel")).unwrap();
        assert!(observations(&root).iter().all(Option::is_none));
    }
    #[test]
    fn listing_refuses_before_first_retained_growth_and_never_truncates() {
        let root = super::super::super::scratch();
        fs::write(root.join("entry"), "actual").unwrap();
        let mut budget = Budget {
            records: 1,
            bytes: size_of::<Vec<PathDirectoryEntry>>(),
        };
        assert!(read_directory(&root, &mut budget).is_none());
        let mut budget = Budget {
            records: 0,
            bytes: 64 * 1024 * 1024,
        };
        assert!(read_directory(&root, &mut budget).is_none());
    }
}
