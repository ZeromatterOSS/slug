/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file.
 */

use std::convert::Infallible;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use fxhash::FxHashSet;
use slug_configuration_v2::HostPathFlavor;
use slug_configuration_v2::RetainedActionEnvironment;

use crate::ActionOutput;
use crate::ActionOutputKind;
use crate::AnalysisArtifact;
use crate::AnalysisValueKind;
use crate::RunfilesPackageDepset;
use crate::RunfilesSupport;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Allocative)]
pub enum RunfilesSymlinkMode {
    Create,
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub enum RunfilesSupportActionSpec {
    RepoMappingManifest {
        support: Arc<RunfilesSupport>,
        packages: RunfilesPackageDepset,
        workspace_name: CompactString,
        emit_compact_repo_mapping: bool,
        output: ActionOutput,
    },
    SourceSymlinkManifest {
        support: Arc<RunfilesSupport>,
        remotable: bool,
        output: ActionOutput,
    },
    SymlinkTree {
        support: Arc<RunfilesSupport>,
        environment: RetainedActionEnvironment,
        mode: RunfilesSymlinkMode,
        output: ActionOutput,
    },
    RunfilesTree {
        support: Arc<RunfilesSupport>,
        output: ActionOutput,
    },
}

impl RunfilesSupportActionSpec {
    pub fn default_actions(
        support: Arc<RunfilesSupport>,
        packages: RunfilesPackageDepset,
        path_flavor: HostPathFlavor,
        environment: RetainedActionEnvironment,
    ) -> Result<[Self; 4], &'static str> {
        if path_flavor == HostPathFlavor::Windows {
            return Err("runfiles support is unsupported on Windows");
        }
        if support.runfiles.is_empty() {
            return Err("runfiles support requires nonempty runfiles");
        }
        if packages.is_empty() {
            return Err("repository mapping requires nonempty package metadata");
        }
        let input_manifest = derived_output(
            &support.input_manifest,
            ActionOutputKind::File,
            "runfiles input manifest",
        )?;
        let manifest = derived_output(
            support
                .manifest
                .as_ref()
                .ok_or("default runfiles support requires a public manifest")?,
            ActionOutputKind::File,
            "runfiles public manifest",
        )?;
        let repo_mapping = derived_output(
            support
                .repo_mapping_manifest
                .as_ref()
                .ok_or("default runfiles support requires a repository mapping manifest")?,
            ActionOutputKind::File,
            "runfiles repository mapping manifest",
        )?;
        let tree = derived_output(
            &support.tree,
            ActionOutputKind::RunfilesTree,
            "runfiles tree",
        )?;
        let executable_path = tree
            .path()
            .strip_suffix(".runfiles")
            .ok_or("runfiles tree path must end in .runfiles")?;
        if input_manifest.path() != format!("{executable_path}.runfiles_manifest")
            || repo_mapping.path() != format!("{executable_path}.repo_mapping")
            || manifest.path() != format!("{}.runfiles/MANIFEST", executable_path)
        {
            return Err("runfiles support artifact paths are inconsistent");
        }
        let owner = derived_owner(&support.tree)?;
        for artifact in [
            &support.input_manifest,
            support.manifest.as_ref().expect("validated manifest"),
            support
                .repo_mapping_manifest
                .as_ref()
                .expect("validated repository mapping"),
        ] {
            if derived_owner(artifact)? != owner {
                return Err("runfiles support artifacts have different owners");
            }
        }
        let input_manifest = input_manifest.clone();
        let manifest = manifest.clone();
        let repo_mapping = repo_mapping.clone();
        let tree = tree.clone();

        Ok([
            Self::RepoMappingManifest {
                support: support.clone(),
                packages,
                workspace_name: support.runfiles.repository_prefix.clone(),
                emit_compact_repo_mapping: true,
                output: repo_mapping,
            },
            Self::SourceSymlinkManifest {
                support: support.clone(),
                remotable: false,
                output: input_manifest,
            },
            Self::SymlinkTree {
                support: support.clone(),
                environment,
                mode: RunfilesSymlinkMode::Create,
                output: manifest,
            },
            Self::RunfilesTree {
                support,
                output: tree,
            },
        ])
    }

    pub fn support(&self) -> &Arc<RunfilesSupport> {
        match self {
            Self::RepoMappingManifest { support, .. }
            | Self::SourceSymlinkManifest { support, .. }
            | Self::SymlinkTree { support, .. }
            | Self::RunfilesTree { support, .. } => support,
        }
    }

    pub fn mnemonic(&self) -> &'static str {
        match self {
            Self::RepoMappingManifest { .. } => "RepoMappingManifest",
            Self::SourceSymlinkManifest { .. } => "SourceSymlinkManifest",
            Self::SymlinkTree { .. } => "SymlinkTree",
            Self::RunfilesTree { .. } => "RunfilesTree",
        }
    }

    pub fn output(&self) -> &ActionOutput {
        match self {
            Self::RepoMappingManifest { output, .. }
            | Self::SourceSymlinkManifest { output, .. }
            | Self::SymlinkTree { output, .. }
            | Self::RunfilesTree { output, .. } => output,
        }
    }

    pub fn visit_declared_inputs(&self, mut visitor: impl FnMut(&AnalysisArtifact)) {
        match self {
            Self::RepoMappingManifest { .. } => {}
            Self::SourceSymlinkManifest { support, .. } => {
                support
                    .runfiles
                    .files
                    .visit(|value| {
                        if let AnalysisValueKind::Artifact(artifact) = value.kind()
                            && matches!(
                                artifact,
                                AnalysisArtifact::Derived { output, .. }
                                    if output.kind() == ActionOutputKind::Symlink
                            )
                        {
                            visitor(artifact);
                        }
                        Ok::<_, Infallible>(())
                    })
                    .unwrap_or_else(|never| match never {});
            }
            Self::SymlinkTree { support, .. } => visitor(&support.input_manifest),
            Self::RunfilesTree { support, .. } => {
                let mut seen = FxHashSet::default();
                let mut visit = |artifact: &AnalysisArtifact| {
                    if seen.insert(artifact.clone()) {
                        visitor(artifact);
                    }
                };
                support
                    .runfiles
                    .files
                    .visit(|value| {
                        if let AnalysisValueKind::Artifact(artifact) = value.kind() {
                            visit(artifact);
                        }
                        Ok::<_, Infallible>(())
                    })
                    .unwrap_or_else(|never| match never {});
                support
                    .runfiles
                    .symlinks
                    .visit(|symlink| {
                        visit(&symlink.artifact);
                        Ok::<_, Infallible>(())
                    })
                    .unwrap_or_else(|never| match never {});
                support
                    .runfiles
                    .root_symlinks
                    .visit(|symlink| {
                        visit(&symlink.artifact);
                        Ok::<_, Infallible>(())
                    })
                    .unwrap_or_else(|never| match never {});
                visit(
                    support
                        .manifest
                        .as_ref()
                        .expect("default support has a public manifest"),
                );
                visit(
                    support
                        .repo_mapping_manifest
                        .as_ref()
                        .expect("default support has a repository mapping manifest"),
                );
            }
        }
    }
}

fn derived_output<'a>(
    artifact: &'a AnalysisArtifact,
    kind: ActionOutputKind,
    role: &'static str,
) -> Result<&'a ActionOutput, &'static str> {
    match artifact {
        AnalysisArtifact::Derived { output, .. } if output.kind() == kind => Ok(output),
        AnalysisArtifact::Derived { .. } => Err(match role {
            "runfiles tree" => "runfiles tree has the wrong output kind",
            _ => "runfiles manifest has the wrong output kind",
        }),
        AnalysisArtifact::Source(_) => Err("runfiles support artifacts must be derived"),
    }
}

fn derived_owner(
    artifact: &AnalysisArtifact,
) -> Result<&crate::AnalysisConfiguredTargetKey, &'static str> {
    match artifact {
        AnalysisArtifact::Derived { owner, .. } => Ok(owner),
        AnalysisArtifact::Source(_) => Err("runfiles support artifacts must be derived"),
    }
}
