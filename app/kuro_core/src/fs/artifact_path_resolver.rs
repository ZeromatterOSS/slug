/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use allocative::Allocative;
use dupe::Dupe;

use crate::cells::CellResolver;
use crate::cells::cell_path::CellPathRef;
use crate::content_hash::ContentBasedPathHash;
use crate::fs::buck_out_path::BuckOutPathResolver;
use crate::fs::buck_out_path::BuildArtifactPath;
use crate::fs::project::ProjectRoot;
use crate::fs::project_rel_path::ProjectRelativePathBuf;
use crate::package::source_path::SourcePathRef;

#[derive(Clone, Allocative)]
pub struct ArtifactFs {
    cell_resolver: CellResolver,
    buck_out_path_resolver: BuckOutPathResolver,
    project_filesystem: ProjectRoot,
}

impl ArtifactFs {
    pub fn new(
        buck_path_resolver: CellResolver,
        buck_out_path_resolver: BuckOutPathResolver,
        project_filesystem: ProjectRoot,
    ) -> Self {
        Self {
            cell_resolver: buck_path_resolver,
            buck_out_path_resolver,
            project_filesystem,
        }
    }

    pub fn retrieve_unhashed_location(
        &self,
        path: &BuildArtifactPath,
    ) -> Option<ProjectRelativePathBuf> {
        self.buck_out_path_resolver.unhashed_gen(path)
    }

    pub fn resolve_build(
        &self,
        path: &BuildArtifactPath,
        content_hash: Option<&ContentBasedPathHash>,
    ) -> kuro_error::Result<ProjectRelativePathBuf> {
        self.buck_out_path_resolver.resolve_gen(path, content_hash)
    }

    pub fn resolve_build_configuration_hash_path(
        &self,
        path: &BuildArtifactPath,
    ) -> kuro_error::Result<ProjectRelativePathBuf> {
        self.buck_out_path_resolver
            .resolve_gen_configuration_hash_path(path)
    }

    pub fn resolve_cell_path(
        &self,
        path: CellPathRef,
    ) -> kuro_error::Result<ProjectRelativePathBuf> {
        self.cell_resolver.resolve_path(path)
    }

    pub fn resolve_source(
        &self,
        source_artifact_path: SourcePathRef,
    ) -> kuro_error::Result<ProjectRelativePathBuf> {
        let cell_resolver = self.cell_resolver();
        let cell_name = source_artifact_path.package().cell_name();
        if cell_resolver.get(cell_name)?.external().is_some() {
            // Bazel convention: source files in non-root cells live at
            // `external/<cell>/<rel>` in the action sandbox. rules_cc's
            // `init_cc_compilation_context` emits `-Iexternal/<cell>/include`
            // for cc_libraries in external repos, so the action's input
            // tree must place the headers at that path or the compiler
            // can't resolve them. Local execution works through a
            // pre-existing `external/<cell> -> bazel-external/<cell>`
            // symlink at the project root; remote execution sees only
            // what we explicitly upload, so the project-relative path we
            // hand the input-tree builder must match the `-I` flag.
            //
            // Previously `resolve_source` returned the buck-out
            // materialization path (`buck-out/v2/external_cells/<origin>/<cell>/<rel>`)
            // — which works locally because all three of `external/<cell>`,
            // `buck-out/v2/external_cells/.../<cell>`, and `bazel-external/<cell>`
            // resolve to the same files via symlinks, but fails on RE
            // because the worker only has whatever path we uploaded.
            // Switch to the `external/<cell>/<rel>` form everywhere
            // (matching `ArtifactPath::with_path`).
            let cell_path = source_artifact_path.to_cell_path();
            let rel = cell_path.path().as_str();
            let combined = format!("external/{}/{}", cell_name.as_str(), rel);
            Ok(ProjectRelativePathBuf::unchecked_new(combined))
        } else {
            Ok(cell_resolver
                .resolve_path(source_artifact_path.package().as_cell_path())?
                .join(source_artifact_path.path()))
        }
    }

    pub fn resolve_offline_output_cache_path(
        &self,
        path: &BuildArtifactPath,
    ) -> kuro_error::Result<ProjectRelativePathBuf> {
        self.buck_out_path_resolver.resolve_offline_cache(path)
    }

    pub fn fs(&self) -> &ProjectRoot {
        &self.project_filesystem
    }

    pub fn buck_out_path_resolver(&self) -> &BuckOutPathResolver {
        &self.buck_out_path_resolver
    }

    pub fn cell_resolver(&self) -> &CellResolver {
        &self.cell_resolver
    }
}
