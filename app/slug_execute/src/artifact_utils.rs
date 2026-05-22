/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::sync::Arc;

use dupe::Dupe;
use slug_core::fs::project::ProjectRoot;
use slug_core::fs::project_rel_path::ProjectRelativePath;
use slug_core::fs::project_rel_path::ProjectRelativePathBuf;
use slug_directory::directory::entry::DirectoryEntry;
use slug_error::BuckErrorContext;
use slug_fs::fs_util;

use crate::artifact_value::ArtifactValue;
use crate::digest_config::DigestConfig;
use crate::directory::ActionDirectoryBuilder;
use crate::directory::ActionDirectoryEntry;
use crate::directory::ActionDirectoryMember;
use crate::directory::ActionSharedDirectory;
use crate::directory::INTERNER;
use crate::directory::extract_artifact_value;
use crate::directory::insert_artifact;
use crate::directory::insert_entry;
use crate::directory::new_symlink;
use crate::directory::override_executable_bit;
use crate::directory::relativize_directory;

pub struct ArtifactValueBuilder<'a> {
    /// Only used to relativize paths; no disk operations performed!
    project_fs: &'a ProjectRoot,
    builder: ActionDirectoryBuilder,
    digest_config: DigestConfig,
}

impl<'a> ArtifactValueBuilder<'a> {
    pub fn new(project_fs: &'a ProjectRoot, digest_config: DigestConfig) -> Self {
        Self {
            project_fs,
            builder: ActionDirectoryBuilder::empty(),
            digest_config,
        }
    }

    pub fn add_entry(
        &mut self,
        path: ProjectRelativePathBuf,
        entry: ActionDirectoryEntry<ActionDirectoryBuilder>,
    ) -> slug_error::Result<()> {
        insert_entry(&mut self.builder, path, entry)
    }

    /// Inserts an input to the tree, which will be required when following
    /// symlinks to calculate the `deps` of the `ArtifactValue`.
    pub fn add_input_value(
        &mut self,
        path: ProjectRelativePathBuf,
        value: &ArtifactValue,
    ) -> slug_error::Result<()> {
        insert_artifact(&mut self.builder, path, value)
    }

    /// Takes an input `src_value`, adds it to the builder at `src`. Then
    /// creates a symlink to `src`, adds it to the builder at `dest` and
    /// returns it.
    pub fn add_symlinked(
        &mut self,
        src_value: &ArtifactValue,
        src: ProjectRelativePathBuf,
        dest: &ProjectRelativePath,
    ) -> slug_error::Result<()> {
        let symlink = new_symlink(self.project_fs.relative_path(&src, dest))?;
        insert_artifact(&mut self.builder, src, src_value)?;
        let entry = DirectoryEntry::Leaf(symlink);
        self.builder.insert(dest, entry)?;
        Ok(())
    }

    /// Like `add_symlinked`, but the symlink target is absolute. Use this when
    /// the resulting artifact may be declared at a different path than `dest`.
    pub fn add_external_symlinked(
        &mut self,
        src_value: &ArtifactValue,
        src: ProjectRelativePathBuf,
        dest: &ProjectRelativePath,
    ) -> slug_error::Result<()> {
        let target = match src_value.entry() {
            DirectoryEntry::Leaf(ActionDirectoryMember::ExternalSymlink(s)) => s.to_path_buf(),
            DirectoryEntry::Leaf(ActionDirectoryMember::Symlink(s)) => {
                let parent = src
                    .parent()
                    .buck_error_context("Symlink has no dir parent")?;
                self.project_fs
                    .resolve(parent)
                    .as_path()
                    .join(s.target().as_str())
            }
            _ => self.project_fs.resolve(&src).as_path().to_path_buf(),
        };
        let symlink = new_symlink(target)?;
        insert_artifact(&mut self.builder, src, src_value)?;
        let entry = DirectoryEntry::Leaf(symlink);
        self.builder.insert(dest, entry)?;
        Ok(())
    }

    /// Like `add_symlinked`, but the symlink target is the source artifact
    /// path itself. This matches Bazel `ctx.actions.symlink(target_file = ...)`.
    pub fn add_artifact_path_symlinked(
        &mut self,
        src_value: &ArtifactValue,
        src: ProjectRelativePathBuf,
        dest: &ProjectRelativePath,
    ) -> slug_error::Result<()> {
        let symlink = new_symlink(self.project_fs.resolve(&src).as_path())?;
        insert_artifact(&mut self.builder, src, src_value)?;
        let entry = DirectoryEntry::Leaf(symlink);
        self.builder.insert(dest, entry)?;
        Ok(())
    }

    /// Takes an input `src_value`, adds it to the builder at `src`. Then
    /// creates a copy of `src_value`'s entry relativized as if it had been
    /// copied from `src` to `dest`, adds it to the builder at `dest` and
    /// returns it.
    pub fn add_copied(
        &mut self,
        src_value: &ArtifactValue,
        src: &ProjectRelativePath,
        dest: &ProjectRelativePath,
        executable_bit_override: Option<bool>,
    ) -> slug_error::Result<ActionDirectoryEntry<ActionSharedDirectory>> {
        insert_artifact(&mut self.builder, src.to_buf(), src_value)?;

        let entry = match src_value.entry() {
            DirectoryEntry::Dir(directory) => {
                let mut builder = directory.dupe().into_builder();
                relativize_directory(&mut builder, src, dest)?;
                if let Some(executable_bit_override) = executable_bit_override {
                    override_executable_bit(&mut builder, executable_bit_override)?;
                }
                DirectoryEntry::Dir(
                    builder.fingerprint(self.digest_config.as_directory_serializer()),
                )
            }
            DirectoryEntry::Leaf(ActionDirectoryMember::Symlink(s)) => {
                // TODO: This seems like it normally shouldn't need to be normalizing anything.
                let reldest = self.project_fs.relative_path(
                    src.parent()
                        .buck_error_context("Symlink has no dir parent")?,
                    dest,
                );
                // RelativePathBuf converts platform specific path separators.
                let reldest = fs_util::relative_path_from_system(&reldest)?;
                let s = s.relativized(reldest);
                DirectoryEntry::Leaf(ActionDirectoryMember::Symlink(Arc::new(s)))
            }
            DirectoryEntry::Leaf(ActionDirectoryMember::ExternalSymlink(s)) => {
                DirectoryEntry::Leaf(ActionDirectoryMember::ExternalSymlink(
                    s.with_full_target()?,
                ))
            }
            DirectoryEntry::Leaf(ActionDirectoryMember::File(f)) => {
                let file_metadata = if let Some(executable_bit_override) = executable_bit_override {
                    f.dupe().with_executable(executable_bit_override)
                } else {
                    f.dupe()
                };
                DirectoryEntry::Leaf(ActionDirectoryMember::File(file_metadata))
            }
        };

        let entry = entry.map_dir(|d| d.shared(&*INTERNER));

        self.builder
            .insert(dest, entry.dupe().map_dir(|d| d.into_builder()))?;

        Ok(entry)
    }

    /// Builds the `ArtifactValue`. Since `self.builder` is rooted at the
    /// project root, `output` must be passed to specify the path of the value
    /// being built.
    pub fn build(&self, output: &ProjectRelativePath) -> slug_error::Result<ArtifactValue> {
        match extract_artifact_value(&self.builder, output, self.digest_config)? {
            Some(v) => Ok(v),
            None => {
                tracing::debug!("Extracting {} produces empty directory!", output);
                Ok(ArtifactValue::dir(self.digest_config.empty_directory()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use slug_common::external_symlink::ExternalSymlink;
    use slug_common::file_ops::metadata::FileMetadata;
    use slug_common::file_ops::metadata::Symlink;
    use slug_core::fs::project::ProjectRootTemp;

    use super::*;
    use crate::directory::insert_file;

    fn path(s: &str) -> &ProjectRelativePath {
        ProjectRelativePath::new(s).unwrap()
    }

    fn get_symlink(s: &str) -> Arc<Symlink> {
        Arc::new(Symlink::new(s.into()))
    }

    fn get_symlink_artifact_value(s: &str) -> ArtifactValue {
        let symlink = DirectoryEntry::Leaf(ActionDirectoryMember::Symlink(get_symlink(s)));
        ArtifactValue::new(symlink, None)
    }

    #[test]
    fn symlinked_external_artifact_points_to_source_repo() -> slug_error::Result<()> {
        let fs = ProjectRootTemp::new().unwrap();
        let digest_config = DigestConfig::testing_default();
        let src =
            path("external/rules_rs++toolchains+rustc_linux_x86_64_1_95_0/bin/rustc").to_buf();
        let dest = path(
            "buck-out/plan61/gen/rules_rs++toolchains+default_rust_toolchains/ef6520194e777f31/external/rules_rs++toolchains+default_rust_toolchains/linux_x86_64_1_95_0_rust_toolchain_bootstrap/bin/rustc",
        );

        let src_value = {
            let mut builder = ActionDirectoryBuilder::empty();
            insert_file(
                &mut builder,
                src.clone(),
                FileMetadata::empty(digest_config.cas_digest_config()),
            )?;
            extract_artifact_value(&builder, src.as_ref(), digest_config)?
                .buck_error_context("missing source value")?
        };

        let mut builder = ArtifactValueBuilder::new(fs.path(), digest_config);
        builder.add_symlinked(&src_value, src, dest)?;
        let value = builder.build(dest)?;

        let symlink = match value.entry() {
            DirectoryEntry::Leaf(ActionDirectoryMember::Symlink(symlink)) => symlink,
            entry => panic!("expected symlink entry, got {entry:?}"),
        };

        assert_eq!(
            symlink.target().as_str(),
            "../../../../../../../../../external/rules_rs++toolchains+rustc_linux_x86_64_1_95_0/bin/rustc",
        );

        Ok(())
    }

    #[test]
    fn external_symlinked_artifact_is_destination_independent() -> slug_error::Result<()> {
        let fs = ProjectRootTemp::new().unwrap();
        let digest_config = DigestConfig::testing_default();
        let src =
            path("external/rules_rs++toolchains+rustc_linux_x86_64_1_95_0/bin/rustc").to_buf();
        let tmp_dest = path(
            "buck-out/plan61/gen/rules_rs++toolchains+default_rust_toolchains/__content_based_path__/external/rules_rs++toolchains+default_rust_toolchains/linux_x86_64_1_95_0_rust_toolchain_bootstrap/bin/rustc",
        );
        let final_dest = path(
            "buck-out/plan61/gen/rules_rs++toolchains+default_rust_toolchains/ef6520194e777f31/external/rules_rs++toolchains+default_rust_toolchains/linux_x86_64_1_95_0_rust_toolchain_bootstrap/bin/rustc",
        );

        let src_value = {
            let mut builder = ActionDirectoryBuilder::empty();
            insert_file(
                &mut builder,
                src.clone(),
                FileMetadata::empty(digest_config.cas_digest_config()),
            )?;
            extract_artifact_value(&builder, src.as_ref(), digest_config)?
                .buck_error_context("missing source value")?
        };

        let mut builder = ArtifactValueBuilder::new(fs.path(), digest_config);
        builder.add_external_symlinked(&src_value, src.clone(), tmp_dest)?;
        let value = builder.build(tmp_dest)?;

        let symlink = match value.entry() {
            DirectoryEntry::Leaf(ActionDirectoryMember::ExternalSymlink(symlink)) => symlink,
            entry => panic!("expected external symlink entry, got {entry:?}"),
        };

        assert_eq!(symlink.target(), fs.path().resolve(src).as_path(),);
        assert_ne!(symlink.target(), fs.path().resolve(final_dest).as_path());

        Ok(())
    }

    #[test]
    fn external_symlinked_artifact_preserves_source_symlink_target() -> slug_error::Result<()> {
        let fs = ProjectRootTemp::new().unwrap();
        let digest_config = DigestConfig::testing_default();
        let source_path = path(
            "buck-out/plan61/gen/rules_rs++toolchains+default_rust_toolchains/ef6520194e777f31/external/rules_rs++toolchains+default_rust_toolchains/linux_x86_64_1_95_0_rust_toolchain_bootstrap/bin/rustc",
        )
        .to_buf();
        let actual_rustc = fs
            .path()
            .root()
            .as_path()
            .join("bazel-external/rules_rs++toolchains+rustc_linux_x86_64_1_95_0/bin/rustc");
        let src_value = ArtifactValue::external_symlink(Arc::new(ExternalSymlink::new(
            actual_rustc.clone(),
            Default::default(),
        )?));
        let dest = path(
            "buck-out/plan61/gen/rules_rs++toolchains+default_rust_toolchains/__content_based_path__/external/rules_rs++toolchains+default_rust_toolchains/linux_x86_64_1_95_0_rust_toolchain_bootstrap/bin/rustc",
        );

        let mut builder = ArtifactValueBuilder::new(fs.path(), digest_config);
        builder.add_external_symlinked(&src_value, source_path.clone(), dest)?;
        let value = builder.build(dest)?;

        let symlink = match value.entry() {
            DirectoryEntry::Leaf(ActionDirectoryMember::ExternalSymlink(symlink)) => symlink,
            entry => panic!("expected external symlink entry, got {entry:?}"),
        };

        assert_eq!(symlink.target(), actual_rustc.as_path());
        assert_ne!(symlink.target(), fs.path().resolve(source_path).as_path());

        Ok(())
    }

    #[test]
    fn artifact_path_symlinked_ignores_source_symlink_target() -> slug_error::Result<()> {
        let fs = ProjectRootTemp::new().unwrap();
        let digest_config = DigestConfig::testing_default();
        let src =
            path("external/rules_rs++toolchains+rustc_linux_x86_64_1_95_0/bin/rustc").to_buf();
        let dest = path(
            "buck-out/plan61/gen/rules_rs++toolchains+default_rust_toolchains/ef6520194e777f31/external/rules_rs++toolchains+default_rust_toolchains/linux_x86_64_1_95_0_rust_toolchain_bootstrap/bin/rustc",
        );
        let src_value = get_symlink_artifact_value(
            "/var/mnt/dev/zeromatter-kuro/buck-out/plan61/gen/rules_rs++toolchains+default_rust_toolchains/ef6520194e777f31/external/rules_rs++toolchains+default_rust_toolchains/linux_x86_64_1_95_0_rust_toolchain_bootstrap/bin/rustc",
        );

        let mut builder = ArtifactValueBuilder::new(fs.path(), digest_config);
        builder.add_artifact_path_symlinked(&src_value, src.clone(), dest)?;
        let value = builder.build(dest)?;

        let symlink = match value.entry() {
            DirectoryEntry::Leaf(ActionDirectoryMember::ExternalSymlink(symlink)) => symlink,
            entry => panic!("expected external symlink entry, got {entry:?}"),
        };

        assert_eq!(symlink.target(), fs.path().resolve(&src).as_path());

        Ok(())
    }

    #[test]
    fn copy_relativized_symlink() -> slug_error::Result<()> {
        // /
        // |-d1/
        // | |-d2/
        // | | |-d3/
        // | | |  |-d4/
        // | | |  | |-link -> ../../../d6/target
        // | |-d5/
        // | | |-new_link
        // |-d6/
        // | |-target

        let entry = {
            let fs = ProjectRootTemp::new().unwrap();
            let mut builder = ArtifactValueBuilder::new(fs.path(), DigestConfig::testing_default());
            builder.add_copied(
                &get_symlink_artifact_value("../../../d6/target"),
                path("d1/d2/d3/d4/link"),
                path("d1/d5/new_link"),
                None,
            )?
        };

        let new_symlink = match entry.as_ref() {
            DirectoryEntry::Leaf(ActionDirectoryMember::Symlink(s)) => s,
            _ => panic!("Symlink type is expected!"),
        };

        assert_eq!(
            new_symlink,
            &get_symlink("../d6/target"),
            "Symlinks are different"
        );

        Ok(())
    }
}
