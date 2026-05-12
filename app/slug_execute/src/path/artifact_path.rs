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
use std::sync::OnceLock;
use std::sync::RwLock;

use either::Either;
use gazebo::cell::ARef;
use slug_core::cells::is_root_cell_name;
use slug_core::content_hash::ContentBasedPathHash;
use slug_core::deferred::base_deferred_key::BaseDeferredKey;
use slug_core::fs::artifact_path_resolver::ArtifactFs;
use slug_core::fs::buck_out_path::BuildArtifactPath;
use slug_core::fs::project_rel_path::ProjectRelativePathBuf;
use slug_core::package::source_path::SourcePathRef;
use slug_error::ErrorTag;
use slug_error::slug_error;
use slug_fs::paths::file_name::FileName;
use slug_fs::paths::forward_rel_path::ForwardRelativePath;
use slug_fs::paths::forward_rel_path::ForwardRelativePathBuf;

#[derive(Debug, Eq, PartialEq, Hash)]
pub struct ArtifactPath<'a> {
    pub base_path: Either<ARef<'a, BuildArtifactPath>, SourcePathRef<'a>>,
    pub projected_path: &'a ForwardRelativePath,
    /// The number of components at the prefix of that path that are internal details to the rule,
    /// not returned by `.short_path`.
    pub hidden_components_count: usize,
}

fn artifact_path_buck_out_root_cell() -> &'static RwLock<ProjectRelativePathBuf> {
    static BUCK_OUT_ROOT: OnceLock<RwLock<ProjectRelativePathBuf>> = OnceLock::new();
    BUCK_OUT_ROOT.get_or_init(|| {
        RwLock::new(ProjectRelativePathBuf::unchecked_new(
            "buck-out/v2".to_owned(),
        ))
    })
}

pub fn set_artifact_path_buck_out_root(root: ProjectRelativePathBuf) {
    *artifact_path_buck_out_root_cell()
        .write()
        .expect("artifact path buck-out root lock poisoned") = root;
}

pub fn get_artifact_path_buck_out_root() -> ProjectRelativePathBuf {
    artifact_path_buck_out_root_cell()
        .read()
        .expect("artifact path buck-out root lock poisoned")
        .to_buf()
}

fn canonical_external_cell_name(cell_name: &str) -> String {
    if cell_name.starts_with("crates__") {
        return format!("rules_rs++crate+{cell_name}");
    }
    slug_core::cells::canonical_dynamic_extension_cell_name(cell_name)
        .unwrap_or_else(|| cell_name.to_owned())
}

fn has_bazel_external_prefix(path: &ForwardRelativePath, cell_name: &str) -> bool {
    path.as_str()
        .strip_prefix("external/")
        .is_some_and(|rest| rest == cell_name || rest.starts_with(&format!("{cell_name}/")))
}

impl ArtifactPath<'_> {
    pub fn with_filename<F, T>(&self, f: F) -> slug_error::Result<T>
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
            slug_error!(
                ErrorTag::ArtifactMissingFilename,
                "Artifact has no file name: `{}`",
                self
            )
        })?;

        Ok(f(file_name))
    }

    /// Internal helper: the artifact's path *without* the Bazel
    /// repo-relative prefix (no package, no `../<cell>/`). Used by
    /// `Artifact::project()` to compute `hidden_components_count` deltas, and
    /// generally wherever a caller needs the rule-local fragment that
    /// `BuildArtifactPath::path` describes (post-projection,
    /// post-`hidden_components_count`).
    pub fn with_rule_local_short_path<F, T>(&self, f: F) -> T
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
                f(path)
            }
            Either::Right(source) => {
                let file_rel: &ForwardRelativePath = source.path().as_ref();
                let full = file_rel.join(self.projected_path);
                let path = match full.strip_prefix_components(self.hidden_components_count) {
                    Some(p) => p,
                    None => ForwardRelativePath::empty(),
                };
                f(path)
            }
        }
    }

    pub fn with_short_path<F, T>(&self, f: F) -> T
    where
        for<'b> F: FnOnce(&'b ForwardRelativePath) -> T,
    {
        // Bazel `File.short_path`: the file's path relative to its repository
        // root (workspace root for the main repo, repo root for external
        // repos). Excludes configuration-specific bin_dir prefixes. For
        // external repos the result has a `../<repo_name>/` prefix so it lands
        // outside `<exe>.runfiles/<workspace>/` when laid down in a runfiles
        // tree.
        //
        // - root-cell build artifact in package `lib/glue`, artifact path
        //   `glue-build-script_` → short_path = `lib/glue/glue-build-script_`
        // - external-cell build artifact in package `` of repo
        //   `crates__libm-0.2.16`, artifact path `build_script_build` →
        //   short_path = `../crates__libm-0.2.16/build_script_build`
        // - root-cell source `lib/glue/build.rs` → `lib/glue/build.rs`
        // - external-cell source `crates__libm-0.2.16//src/lib.rs` →
        //   `../crates__libm-0.2.16/src/lib.rs`
        match self.base_path.as_ref() {
            Either::Left(buck_out) => {
                let rule_local = buck_out.path();
                let rule_local = rule_local.join_cow(self.projected_path);
                let rule_local =
                    match rule_local.strip_prefix_components(self.hidden_components_count) {
                        Some(p) => p,
                        None => ForwardRelativePath::empty(),
                    };
                // Reach the configured target label through the owner so we
                // can prepend the package (and external `../<cell>/`).
                let cfg_label = buck_out.owner().owner().configured_label();
                match cfg_label {
                    Some(label) => {
                        let cell_name = label.pkg().cell_name();
                        let external_cell_name = canonical_external_cell_name(cell_name.as_str());
                        let pkg_rel = label.pkg().cell_relative_path().as_str();
                        let rule_local_str = rule_local.as_str();
                        let prefixed = match (
                            is_root_cell_name(cell_name.as_str()),
                            pkg_rel.is_empty(),
                            rule_local_str.is_empty(),
                        ) {
                            (true, true, _) => rule_local_str.to_owned(),
                            (true, false, true) => pkg_rel.to_owned(),
                            (true, false, false) => format!("{}/{}", pkg_rel, rule_local_str),
                            (false, true, true) => format!("../{}", external_cell_name),
                            (false, true, false) => {
                                format!("../{}/{}", external_cell_name, rule_local_str)
                            }
                            (false, false, true) => {
                                format!("../{}/{}", external_cell_name, pkg_rel)
                            }
                            (false, false, false) => {
                                format!("../{}/{}/{}", external_cell_name, pkg_rel, rule_local_str)
                            }
                        };
                        let buf = ForwardRelativePathBuf::unchecked_new(prefixed);
                        f(buf.as_ref())
                    }
                    None => {
                        // No configured target owner (anon target / dynamic
                        // action / etc.) — fall back to the rule-local path.
                        f(rule_local)
                    }
                }
            }
            Either::Right(source) => {
                let file_rel: &ForwardRelativePath = source.path().as_ref();
                let full = file_rel.join(self.projected_path);
                let in_pkg = match full.strip_prefix_components(self.hidden_components_count) {
                    Some(p) => p,
                    None => ForwardRelativePath::empty(),
                };
                let cell_name = source.package().cell_name();
                let external_cell_name = canonical_external_cell_name(cell_name.as_str());
                let pkg_path: &ForwardRelativePath = source.package().cell_relative_path().as_ref();
                let cell_rel = if pkg_path.is_empty() {
                    in_pkg.as_str().to_owned()
                } else {
                    format!("{}/{}", pkg_path.as_str(), in_pkg.as_str())
                };
                if is_root_cell_name(cell_name.as_str()) {
                    let buf = ForwardRelativePathBuf::unchecked_new(cell_rel);
                    f(buf.as_ref())
                } else {
                    let with_prefix = ForwardRelativePathBuf::unchecked_new(format!(
                        "../{}/{}",
                        external_cell_name, cell_rel
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
                let pkg_path: &ForwardRelativePath = source.package().cell_relative_path().as_ref();
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
                    let buck_out_root = get_artifact_path_buck_out_root();
                    let buck_out_root = buck_out_root.as_str();
                    let cfg_hash = target.cfg().output_hash().as_str();
                    let cell_name = target.pkg().cell_name().as_str();
                    let external_cell_name = canonical_external_cell_name(cell_name);
                    let cell_relative_path = target.pkg().cell_relative_path().as_str();
                    let target_name = target.name().as_str();
                    let artifact_path = buck_out.path();

                    // Escape target name (replace = with special sequence)
                    let escaped_target_name = target_name.replace('=', "__EQ__");

                    // Shareable-artifact path: `<bin_dir>/<filename>` with no
                    // package or `__<target>__/` segment. Used by
                    // `actions.declare_shareable_artifact` (rules_cc's virtual
                    // includes, LTO, etc.) where the filename is already
                    // fully-qualified from bin_dir root.
                    // Kept in sync with `BaseDeferredKey::make_hashed_path`.
                    let resolution = buck_out.path_resolution_method();
                    let full_path = if matches!(
                        resolution,
                        slug_core::fs::buck_out_path::BuckOutPathKind::Shareable
                    ) {
                        let joined = artifact_path.join(self.projected_path);
                        format!(
                            "{}/gen/{}/{}/{}",
                            buck_out_root, external_cell_name, cfg_hash, joined
                        )
                    } else if matches!(
                        resolution,
                        slug_core::fs::buck_out_path::BuckOutPathKind::BazelOutput
                    ) {
                        let joined = artifact_path.join(self.projected_path);
                        let is_root = slug_core::cells::is_root_cell_name(cell_name);
                        let has_external_prefix =
                            has_bazel_external_prefix(&joined, &external_cell_name);
                        if has_external_prefix {
                            format!(
                                "{}/gen/{}/{}/{}",
                                buck_out_root, external_cell_name, cfg_hash, joined
                            )
                        } else {
                            match (is_root, cell_relative_path.is_empty()) {
                                (true, true) => {
                                    format!(
                                        "{}/gen/{}/{}/{}",
                                        buck_out_root, external_cell_name, cfg_hash, joined
                                    )
                                }
                                (true, false) => {
                                    format!(
                                        "{}/gen/{}/{}/{}/{}",
                                        buck_out_root,
                                        external_cell_name,
                                        cfg_hash,
                                        cell_relative_path,
                                        joined
                                    )
                                }
                                (false, true) => {
                                    format!(
                                        "{}/gen/{}/{}/external/{}/{}",
                                        buck_out_root,
                                        external_cell_name,
                                        cfg_hash,
                                        external_cell_name,
                                        joined
                                    )
                                }
                                (false, false) => {
                                    format!(
                                        "{}/gen/{}/{}/external/{}/{}/{}",
                                        buck_out_root,
                                        external_cell_name,
                                        cfg_hash,
                                        external_cell_name,
                                        cell_relative_path,
                                        joined
                                    )
                                }
                            }
                        }
                    } else if cell_relative_path.is_empty() {
                        // Path format: <buck-out>/gen/<cell>/<cfg_hash>/__<target>__/<artifact_path>
                        format!(
                            "{}/gen/{}/{}/__{}__/{}",
                            buck_out_root,
                            external_cell_name,
                            cfg_hash,
                            escaped_target_name,
                            artifact_path.join(self.projected_path)
                        )
                    } else {
                        format!(
                            "{}/gen/{}/{}/{}/__{}__/{}",
                            buck_out_root,
                            external_cell_name,
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
                let external_cell_name = canonical_external_cell_name(cell_name);
                let cell_relative = buck
                    .package()
                    .cell_relative_path()
                    .as_forward_relative_path()
                    .join(buck.path());
                let path = cell_relative.join_cow(self.projected_path);

                if !slug_core::cells::is_root_cell_name(cell_name) {
                    let full_path = format!("external/{}/{}", external_cell_name, path);
                    let full_path_buf = ForwardRelativePathBuf::unchecked_new(full_path);
                    f(&full_path_buf)
                } else {
                    f(&path)
                }
            }
        }
    }

    /// Bazel-style root: the bin_dir prefix of `full_path` *excluding*
    /// the package and filename. Empty string for source artifacts.
    ///
    /// Used by `artifact.root.path` to satisfy rules_cc's
    /// `cc_compilation_helper.bzl::_repo_relative_path`, which relies on
    /// `paths.relativize(full_path, root_path)` returning a path that
    /// starts with the target's package directory.
    pub fn with_root_path<F, T>(&self, f: F) -> T
    where
        for<'b> F: FnOnce(&'b ForwardRelativePath) -> T,
    {
        match self.base_path.as_ref() {
            Either::Left(buck_out) => {
                let owner = buck_out.owner().owner();
                let target_opt = match owner {
                    BaseDeferredKey::TargetLabel(target) => Some(target.clone()),
                    BaseDeferredKey::Aspect(aspect_key) => aspect_key.configured_label(),
                    _ => None,
                };
                if let Some(target) = target_opt {
                    let buck_out_root = get_artifact_path_buck_out_root();
                    let buck_out_root = buck_out_root.as_str();
                    let cfg_hash = target.cfg().output_hash().as_str();
                    let cell_name = target.pkg().cell_name().as_str();
                    let external_cell_name = canonical_external_cell_name(cell_name);
                    let is_root = slug_core::cells::is_root_cell_name(cell_name);
                    let root_path = if is_root {
                        format!("{}/gen/{}/{}", buck_out_root, external_cell_name, cfg_hash)
                    } else {
                        format!(
                            "{}/gen/{}/{}/external/{}",
                            buck_out_root, external_cell_name, cfg_hash, external_cell_name
                        )
                    };
                    let root_path_buf = ForwardRelativePathBuf::unchecked_new(root_path);
                    f(&root_path_buf)
                } else {
                    // Fallback: empty root for non-target owners.
                    f(ForwardRelativePath::empty())
                }
            }
            Either::Right(_) => {
                // Source artifacts: Bazel convention is empty string.
                f(ForwardRelativePath::empty())
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
    ) -> slug_error::Result<ProjectRelativePathBuf> {
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
    ) -> slug_error::Result<ProjectRelativePathBuf> {
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
