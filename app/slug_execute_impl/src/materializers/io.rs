/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::HashMap;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use slug_core::fs::project::ProjectRoot;
use slug_core::fs::project_rel_path::ProjectRelativePathBuf;
use slug_directory::directory::directory::Directory;
use slug_directory::directory::entry::DirectoryEntry;
use slug_execute::directory::ActionDirectory;
use slug_execute::directory::ActionDirectoryEntry;
use slug_execute::directory::ActionDirectoryMember;
use slug_execute::directory::ActionDirectoryRef;
use slug_execute::directory::ActionSharedDirectory;
use slug_execute::execute::blocking::IoRequest;
use slug_fs::fs_util;
use slug_fs::paths::abs_norm_path::AbsNormPath;
use slug_fs::paths::abs_norm_path::AbsNormPathBuf;

pub struct MaterializeTreeStructure {
    pub path: ProjectRelativePathBuf,
    pub entry: ActionDirectoryEntry<ActionSharedDirectory>,
}

impl IoRequest for MaterializeTreeStructure {
    fn execute(self: Box<Self>, project_fs: &ProjectRoot) -> slug_error::Result<()> {
        materialize_dirs_and_syms(
            self.entry.as_ref(),
            project_fs.root().join(&self.path),
            project_fs,
        )?;

        Ok(())
    }
}

/// Materializes the entry at `dest`.
///
/// - `materialize_dirs_and_syms`: if `true`, materializes directories and
///   symlinks.
/// - `file_src`: takes the destination path of a file, and returns its
///   source path (where it should be copied from). If it returns [`None`],
///   the file is not materialized.
fn materialize<F, D>(
    entry: DirectoryEntry<&D, &ActionDirectoryMember>,
    dest: &AbsNormPath,
    materialize_dirs_and_syms: bool,
    mut file_src: F,
    executable_bit_override: Option<bool>,
    project_fs: Option<&ProjectRoot>,
) -> slug_error::Result<()>
where
    F: FnMut(&AbsNormPath) -> Option<AbsNormPathBuf>,
    D: ActionDirectory,
{
    let mut dest = dest.to_owned();
    if materialize_dirs_and_syms {
        // create the directory where we'll materialize the entry
        if let Some(parent) = dest.parent() {
            fs_util::create_dir_all(parent)?;
        }
    }
    materialize_recursively(
        entry.map_dir(|d| Directory::as_ref(d)),
        &mut dest,
        materialize_dirs_and_syms,
        &mut file_src,
        executable_bit_override,
        project_fs,
    )
}

/// Materializes the directories and symlinks of an entry at `dest`. Files
/// are not materialized.
pub(crate) fn materialize_dirs_and_syms<P, D>(
    entry: DirectoryEntry<&D, &ActionDirectoryMember>,
    dest: P,
    project_fs: &ProjectRoot,
) -> slug_error::Result<()>
where
    P: AsRef<AbsNormPath>,
    D: ActionDirectory,
{
    materialize(
        entry,
        dest.as_ref(),
        true,
        |_: &AbsNormPath| None,
        None,
        Some(project_fs),
    )
}

/// Materializes the files of an the entry rooted at `dest`.
///
/// Files are copied from `src`. In other words, if a file would be
/// materialized at `dest/p`, then it's copied from `src/p`.
pub(crate) fn materialize_files<P, D>(
    entry: DirectoryEntry<&D, &ActionDirectoryMember>,
    src: P,
    dest: P,
    executable_bit_override: Option<bool>,
) -> slug_error::Result<()>
where
    P: AsRef<AbsNormPath>,
    D: ActionDirectory,
{
    let src = src.as_ref();
    let dest = dest.as_ref();
    let file_src = |d: &AbsNormPath| {
        // It's safe to unwrap because `materialize_impl` always gives us a
        // path inside `dest`.
        let subpath = d.strip_prefix(dest).unwrap();
        if subpath.as_str().is_empty() {
            // `dest` itself is a file
            Some(src.to_buf())
        } else {
            Some(src.join(subpath))
        }
    };
    materialize(entry, dest, false, file_src, executable_bit_override, None)
}

/// Materializes the files of an entry rooted at `dest`.
///
/// For a file at path `file_dest` in the entry, if `file_dest` exists in
/// `srcs` with value `file_src`, the file is copied from `file_src` to
/// `file_dest`. It's then removed from `srcs`.
fn _materialize_files_from_map<P, D>(
    entry: DirectoryEntry<&D, &ActionDirectoryMember>,
    srcs: &mut HashMap<AbsNormPathBuf, AbsNormPathBuf>,
    dest: P,
) -> slug_error::Result<()>
where
    P: AsRef<AbsNormPath>,
    D: ActionDirectory,
{
    let file_src = |d: &AbsNormPath| srcs.remove(d);
    materialize(entry, dest.as_ref(), false, file_src, None, None)
}

fn materialize_recursively<'a, F, D>(
    entry: DirectoryEntry<D, &ActionDirectoryMember>,
    dest: &mut AbsNormPathBuf,
    materialize_dirs_and_syms: bool,
    file_src: &mut F,
    executable_bit_override: Option<bool>,
    project_fs: Option<&ProjectRoot>,
) -> slug_error::Result<()>
where
    F: FnMut(&AbsNormPath) -> Option<AbsNormPathBuf>,
    D: ActionDirectoryRef<'a>,
{
    match entry {
        DirectoryEntry::Dir(d) => {
            if materialize_dirs_and_syms {
                fs_util::create_dir_all(&dest)?;
            }
            for (name, entry) in d.entries() {
                dest.push(name);
                materialize_recursively(
                    entry,
                    dest,
                    materialize_dirs_and_syms,
                    file_src,
                    executable_bit_override,
                    project_fs,
                )?;
                dest.pop();
            }
            Ok(())
        }
        DirectoryEntry::Leaf(ActionDirectoryMember::File(_)) => {
            if let Some(src) = file_src(dest) {
                fs_util::copy(src, &dest)?;
                if let Some(executable_bit_override) = executable_bit_override {
                    fs_util::set_executable(&dest, executable_bit_override)?;
                }
            }
            Ok(())
        }
        DirectoryEntry::Leaf(ActionDirectoryMember::Symlink(s)) => {
            if materialize_dirs_and_syms && fs_util::symlink_metadata(&dest).is_err() {
                let target = materializer_symlink_target(
                    project_fs,
                    Path::new(s.target().as_str()),
                    dest.as_ref(),
                );
                fs_util::symlink(target, dest)?;
            }
            Ok(())
        }
        DirectoryEntry::Leaf(ActionDirectoryMember::ExternalSymlink(s)) => {
            if materialize_dirs_and_syms && fs_util::symlink_metadata(&dest).is_err() {
                let target = materializer_symlink_target(project_fs, s.target(), dest.as_ref());
                fs_util::symlink(target, dest)?;
            }
            Ok(())
        }
    }
}

fn materializer_symlink_target(
    project_fs: Option<&ProjectRoot>,
    target: &Path,
    _dest: &AbsNormPath,
) -> PathBuf {
    if target.is_absolute() {
        return target.to_path_buf();
    }

    project_fs
        .and_then(|project_fs| project_external_target(project_fs, target))
        .unwrap_or_else(|| target.to_path_buf())
}

fn project_external_target(project_fs: &ProjectRoot, target: &Path) -> Option<PathBuf> {
    let mut components = target.components();
    while let Some(component) = components.next() {
        if component != Component::Normal("external".as_ref()) {
            continue;
        }

        let external_dir = project_fs.root().as_path().join("external");
        let repo = components.next()?;
        let repo = match repo {
            Component::Normal(repo) => repo,
            _ => return None,
        };
        let rest = components.as_path();

        let alias = external_dir.join(repo);
        if let Some(candidate) = external_alias_target(&external_dir, &alias, rest) {
            if candidate.exists() {
                return Some(candidate);
            }
        }

        let candidate = alias.join(rest);
        if candidate.exists() {
            return Some(candidate);
        }

        let candidate = project_fs
            .root()
            .as_path()
            .join("bazel-external")
            .join(repo)
            .join(rest);
        if candidate.exists() {
            return Some(candidate);
        }
        return None;
    }

    None
}

fn external_alias_target(external_dir: &Path, alias: &Path, rest: &Path) -> Option<PathBuf> {
    let target = std::fs::read_link(alias).ok()?;
    let target = if target.is_absolute() {
        target
    } else {
        external_dir.join(target)
    };
    Some(target.join(rest))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use slug_core::fs::project::ProjectRootTemp;
    use slug_fs::fs_util;

    use super::project_external_target;

    #[test]
    fn project_external_target_maps_generated_external_symlink() -> slug_error::Result<()> {
        let project = ProjectRootTemp::new()?;
        let rustc =
            project.path().root().as_path().join(
                "bazel-external/rules_rs+toolchains+rustc_windows_x86_64_1_91_1/bin/rustc.exe",
            );
        fs_util::create_dir_all(slug_fs::paths::abs_path::AbsPath::new(
            rustc.parent().unwrap(),
        )?)?;
        fs_util::write(slug_fs::paths::abs_path::AbsPath::new(&rustc)?, b"rustc")?;
        fs_util::create_dir_all(slug_fs::paths::abs_path::AbsPath::new(
            &project.path().root().as_path().join("external"),
        )?)?;
        fs_util::symlink(
            Path::new("../bazel-external/rules_rs+toolchains+rustc_windows_x86_64_1_91_1"),
            slug_fs::paths::abs_path::AbsPath::new(
                &project
                    .path()
                    .root()
                    .as_path()
                    .join("external/rustc_windows_x86_64_1_91_1"),
            )?,
        )?;

        let target = Path::new(
            "../../../../../../../../../external/rustc_windows_x86_64_1_91_1/bin/rustc.exe",
        );

        assert_eq!(
            project_external_target(project.path(), target).as_deref(),
            Some(rustc.as_path())
        );

        Ok(())
    }
}
