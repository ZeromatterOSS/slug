/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::borrow::Cow;
use std::fmt;
use std::hash::Hash;

use either::Either;
use gazebo::cell::ARef;
use kuro_core::cells::is_root_cell_name;
use kuro_core::content_hash::ContentBasedPathHash;
use kuro_core::deferred::base_deferred_key::BaseDeferredKey;
use kuro_core::fs::artifact_path_resolver::ArtifactFs;
use kuro_core::fs::buck_out_path::BuildArtifactPath;
use kuro_core::fs::project_rel_path::ProjectRelativePathBuf;
use kuro_core::package::source_path::SourcePathRef;
use kuro_error::ErrorTag;
use kuro_error::kuro_error;
use kuro_fs::paths::file_name::FileName;
use kuro_fs::paths::forward_rel_path::ForwardRelativePath;
use kuro_fs::paths::forward_rel_path::ForwardRelativePathBuf;

#[derive(Debug, Eq, PartialEq, Hash)]
pub struct ArtifactPath<'a> {
    pub base_path: Either<ARef<'a, BuildArtifactPath>, SourcePathRef<'a>>,
    pub projected_path: &'a ForwardRelativePath,
    /// The number of components at the prefix of that path that are internal details to the rule,
    /// not returned by `.short_path`.
    pub hidden_components_count: usize,
}

impl ArtifactPath<'_> {
    pub fn with_filename<F, T>(&self, f: F) -> kuro_error::Result<T>
    where
        for<'b> F: FnOnce(&'b FileName) -> T,
    {
        let file_name = match self.projected_path.is_empty() {
            false => self.projected_path,
            true => match self.base_path.as_ref() {
                Either::Left(buck_out) => buck_out.path(),
                Either::Right(buck) => buck.path().as_ref(),
            },
        }
        .file_name()
        .ok_or_else(|| {
            kuro_error!(
                ErrorTag::ArtifactMissingFilename,
                "Artifact has no file name: `{}`",
                self
            )
        })?;

        Ok(f(file_name))
    }

    pub fn with_short_path<F, T>(&self, f: F) -> T
    where
        for<'b> F: FnOnce(&'b ForwardRelativePath) -> T,
    {
        match self.base_path.as_ref() {
            Either::Left(buck_out) => {
                let base = buck_out.path();
                let path = base.join_cow(self.projected_path);
                let path = match path.strip_prefix_components(self.hidden_components_count) {
                    Some(p) => p,
                    None => ForwardRelativePath::empty(),
                };
                // Buck2 semantics: short_path for generated files is just the artifact's
                // own relative path within the rule's artifact namespace (no package prefix).
                // e.g., ctx.actions.write("my_output", "") -> short_path = "my_output"
                f(path)
            }
            Either::Right(source) => {
                // Buck2 semantics: short_path for source files is the path relative to the
                // package directory (NOT the full cell-relative path).
                // e.g., source file at pkg/dir1/file.txt -> short_path = "dir1/file.txt"
                // This matches how Buck2 uses short_path as a dictionary key for symlinked_dir etc.
                let file_rel: &ForwardRelativePath = source.path().as_ref();
                let cell_name = source.package().cell_name();
                let full = file_rel.join(self.projected_path);
                let path = match full.strip_prefix_components(self.hidden_components_count) {
                    Some(p) => p,
                    None => ForwardRelativePath::empty(),
                };
                if is_root_cell_name(cell_name.as_str()) {
                    f(path)
                } else {
                    // External repo: prepend "../repo_name/pkg_rel/"
                    let pkg_path: &ForwardRelativePath =
                        source.package().cell_relative_path().as_ref();
                    let cell_rel_path = if pkg_path.is_empty() {
                        path.as_str().to_owned()
                    } else {
                        format!("{}/{}", pkg_path.as_str(), path.as_str())
                    };
                    let with_prefix = ForwardRelativePathBuf::unchecked_new(format!(
                        "../{}/{}",
                        cell_name.as_str(),
                        cell_rel_path
                    ));
                    f(with_prefix.as_ref())
                }
            }
        }
    }

    /// Returns the display path of the artifact (for str() / repr() in Starlark).
    /// For build artifacts: same as short_path.
    /// For source files: the cell-relative path (e.g., "artifacts/DATA" for pkg artifacts, file DATA).
    /// This matches Buck2 Display semantics: `<source artifacts/DATA>`.
    pub fn with_display_path<F, T>(&self, f: F) -> T
    where
        for<'b> F: FnOnce(&'b ForwardRelativePath) -> T,
    {
        match self.base_path.as_ref() {
            Either::Left(_) => {
                // For build artifacts, display is same as short_path
                self.with_short_path(f)
            }
            Either::Right(source) => {
                // For source files, display uses the cell-relative path
                // e.g., file "artifacts/DATA" in package "artifacts" -> "artifacts/DATA"
                let pkg_path: &ForwardRelativePath =
                    source.package().cell_relative_path().as_ref();
                let file_rel: &ForwardRelativePath = source.path().as_ref();
                let cell_name = source.package().cell_name();
                if pkg_path.is_empty() {
                    // Root package
                    let full = file_rel.join(self.projected_path);
                    let path = match full.strip_prefix_components(self.hidden_components_count) {
                        Some(p) => p,
                        None => ForwardRelativePath::empty(),
                    };
                    f(path)
                } else if is_root_cell_name(cell_name.as_str()) {
                    // Main repo: use cell-relative path (package/file)
                    let base = pkg_path.join(file_rel);
                    let full = base.join(self.projected_path);
                    let path = match full.strip_prefix_components(self.hidden_components_count) {
                        Some(p) => p,
                        None => ForwardRelativePath::empty(),
                    };
                    f(path)
                } else {
                    // External repo: "../repo_name/pkg/file"
                    let base = pkg_path.join(file_rel);
                    let full = base.join(self.projected_path);
                    let path = match full.strip_prefix_components(self.hidden_components_count) {
                        Some(p) => p,
                        None => ForwardRelativePath::empty(),
                    };
                    let with_prefix = ForwardRelativePathBuf::unchecked_new(format!(
                        "../{}/{}",
                        cell_name.as_str(),
                        path.as_str()
                    ));
                    f(with_prefix.as_ref())
                }
            }
        }
    }

    /// Returns the full execution path of the artifact.
    /// For build artifacts, returns the complete buck-out path that can be used in commands.
    /// For source files, returns the cell-relative path.
    pub fn with_full_path<F, T>(&self, f: F) -> T
    where
        for<'b> F: FnOnce(&'b ForwardRelativePath) -> T,
    {
        match self.base_path.as_ref() {
            Either::Left(buck_out) => {
                // For build artifacts, construct the full buck-out path
                // Path structure: buck-out/v2/gen/<cell_name>/<cfg_hash>/<cell_relative_path>/__<target_name>__/<artifact_path>
                let owner = buck_out.owner().owner();
                // Extract the target label from the owner key.
                // For aspects, use the underlying target label.
                let target_opt = match owner {
                    BaseDeferredKey::TargetLabel(target) => Some(target.clone()),
                    BaseDeferredKey::Aspect(aspect_key) => aspect_key.configured_label(),
                    _ => None,
                };
                if let Some(target) = target_opt {
                    let cfg_hash = target.cfg().output_hash().as_str();
                    let cell_name = target.pkg().cell_name().as_str();
                    let cell_relative_path = target.pkg().cell_relative_path().as_str();
                    let target_name = target.name().as_str();
                    let artifact_path = buck_out.path();

                    // Escape target name (replace = with special sequence)
                    let escaped_target_name = target_name.replace('=', "__EQ__");

                    // Build the full path
                    // Path format: buck-out/v2/gen/<cell>/<cfg_hash>[/<pkg_path>]/__<target>__/<artifact_path>
                    let full_path = if cell_relative_path.is_empty() {
                        format!(
                            "buck-out/v2/gen/{}/{}/__{}__/{}",
                            cell_name,
                            cfg_hash,
                            escaped_target_name,
                            artifact_path.join(self.projected_path)
                        )
                    } else {
                        format!(
                            "buck-out/v2/gen/{}/{}/{}/__{}__/{}",
                            cell_name,
                            cfg_hash,
                            cell_relative_path,
                            escaped_target_name,
                            artifact_path.join(self.projected_path)
                        )
                    };

                    // Convert to ForwardRelativePathBuf and call f
                    let full_path_buf = ForwardRelativePathBuf::unchecked_new(full_path);
                    f(&full_path_buf)
                } else {
                    // Fallback for non-target owners (anon targets, BXL)
                    let base_path = Cow::Borrowed(buck_out.path());
                    let path = base_path.join_cow(self.projected_path);
                    f(&path)
                }
            }
            Either::Right(buck) => {
                // For source files, return cell-relative path + source path.
                // For external repos (non-root cells), prefix with "external/<cell>"
                // to match Bazel's execution-time path convention.
                let cell_name = buck.package().cell_name().as_str();
                let cell_relative = buck
                    .package()
                    .cell_relative_path()
                    .as_forward_relative_path()
                    .join(buck.path());
                let path = cell_relative.join_cow(self.projected_path);

                if !kuro_core::cells::is_root_cell_name(cell_name) {
                    let full_path = format!("external/{}/{}", cell_name, path);
                    let full_path_buf = ForwardRelativePathBuf::unchecked_new(full_path);
                    f(&full_path_buf)
                } else {
                    f(&path)
                }
            }
        }
    }

    /// Returns the project relative path of the artifact.
    /// A build artifact that is declared to be content-based must have a content hash
    /// provided, otherwise an error is returned.
    pub fn resolve(
        &self,
        artifact_fs: &ArtifactFs,
        content_hash: Option<&ContentBasedPathHash>,
    ) -> kuro_error::Result<ProjectRelativePathBuf> {
        let ArtifactPath {
            base_path,
            projected_path,
            hidden_components_count: _,
        } = self;

        let base_path = match base_path {
            Either::Left(build) => artifact_fs
                .buck_out_path_resolver()
                .resolve_gen(build, content_hash)?,
            Either::Right(source) => artifact_fs.resolve_source(*source)?,
        };

        Ok(base_path.join(projected_path))
    }

    /// This function will return the same project relative path as `resolve_path` except
    /// for content-based artifacts, where it will return a path that uses the configuration
    /// hash instead of the content hash.
    pub fn resolve_configuration_hash_path(
        &self,
        artifact_fs: &ArtifactFs,
    ) -> kuro_error::Result<ProjectRelativePathBuf> {
        let ArtifactPath {
            base_path,
            projected_path,
            hidden_components_count: _,
        } = self;

        let base_path = match base_path {
            Either::Left(build) => artifact_fs
                .buck_out_path_resolver()
                .resolve_gen_configuration_hash_path(build)?,
            Either::Right(source) => artifact_fs.resolve_source(*source)?,
        };

        Ok(base_path.join(projected_path))
    }

    pub fn is_content_based_path(&self) -> bool {
        match self.base_path.as_ref() {
            Either::Left(build_artifact_path) => build_artifact_path.is_content_based_path(),
            Either::Right(_) => false,
        }
    }
}

impl fmt::Display for ArtifactPath<'_> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        // NOTE: This produces a representation we tend to use in Starlark for those, which isn't
        // really consistent with what we use when *not* in Starlark.
        self.with_short_path(|p| write!(fmt, "{p}"))
    }
}
