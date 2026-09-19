//! Call-scoped runfiles projection; retained artifacts remain the authority.

use std::fmt;

use compact_str::CompactString;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::ActionOutputKind;
use crate::AnalysisArtifact;
use crate::AnalysisValueKind;
use crate::RunfilesConflictPolicy;
use crate::RunfilesSupport;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum RunfilesLayoutTarget {
    Artifact(AnalysisArtifact),
    Empty,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RunfilesLayoutEntry {
    path: CompactString,
    target: RunfilesLayoutTarget,
}

impl RunfilesLayoutEntry {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn target(&self) -> &RunfilesLayoutTarget {
        &self.target
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum RunfilesLayoutDiagnostic {
    NestedRunfilesTree {
        path: CompactString,
        artifact: AnalysisArtifact,
    },
    Obscured {
        path: CompactString,
        artifact: AnalysisArtifact,
        ancestor_path: CompactString,
        ancestor: AnalysisArtifact,
        policy: RunfilesConflictPolicy,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum RunfilesLayoutError {
    InvalidPath(CompactString),
    NonArtifactFile,
}

impl fmt::Display for RunfilesLayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath(path) => {
                write!(f, "runfiles path `{path}` must be normalized and relative")
            }
            Self::NonArtifactFile => write!(f, "runfiles files must contain artifacts"),
        }
    }
}

impl std::error::Error for RunfilesLayoutError {}

/// Physical MANIFEST is distinct from entries in the source manifest.
#[derive(Debug, Clone, Copy)]
pub struct RunfilesManifestLink<'a> {
    output: &'a AnalysisArtifact,
    target: &'a AnalysisArtifact,
}

impl<'a> RunfilesManifestLink<'a> {
    pub fn output(&self) -> &'a AnalysisArtifact {
        self.output
    }

    pub fn target(&self) -> &'a AnalysisArtifact {
        self.target
    }
}

/// No absolute path, filesystem observation, execution result or new retained DAG.
/// Leaf clones are scratch: the existing depset visitor does not export borrows.
#[derive(Debug)]
pub struct RunfilesLayout<'a> {
    support: &'a RunfilesSupport,
    entries: Vec<RunfilesLayoutEntry>,
    constituents: Vec<AnalysisArtifact>,
    diagnostics: Vec<RunfilesLayoutDiagnostic>,
}

impl<'a> RunfilesLayout<'a> {
    pub fn support(&self) -> &'a RunfilesSupport {
        self.support
    }

    pub fn entries(&self) -> &[RunfilesLayoutEntry] {
        &self.entries
    }

    /// Includes obscured and overridden targets and all physical support manifests.
    /// Does not include this support's virtual tree artifact.
    pub fn constituents(&self) -> &[AnalysisArtifact] {
        &self.constituents
    }

    pub fn diagnostics(&self) -> &[RunfilesLayoutDiagnostic] {
        &self.diagnostics
    }

    pub fn manifest_link(&self) -> Option<RunfilesManifestLink<'a>> {
        self.support
            .manifest
            .as_ref()
            .map(|output| RunfilesManifestLink {
                output,
                target: &self.support.input_manifest,
            })
    }
}

impl RunfilesSupport {
    /// Pinned Bazel Runfiles.getRunfilesInputs ordering, with exact typed targets.
    pub fn layout(&self) -> Result<RunfilesLayout<'_>, RunfilesLayoutError> {
        let runfiles = &self.runfiles;
        validate_path(&runfiles.repository_prefix)?;
        let mut constituents = SmallSet::new();
        let mut diagnostics = Vec::new();
        let mut working = SmallMap::new();
        runfiles.symlinks.visit(|link| {
            validate_path(&link.path)?;
            constituents.insert(link.artifact.clone());
            if let Some(artifact) = checked_artifact(&mut diagnostics, &link.path, &link.artifact) {
                working.insert(link.path.clone(), artifact);
            }
            Ok::<_, RunfilesLayoutError>(())
        })?;
        runfiles.files.visit(|value| {
            let AnalysisValueKind::Artifact(artifact) = value.kind() else {
                return Err(RunfilesLayoutError::NonArtifactFile);
            };
            constituents.insert(artifact.clone());
            let path = artifact_path(artifact)?;
            if let Some(artifact) = checked_artifact(&mut diagnostics, &path, artifact) {
                working.insert(path, artifact);
            }
            Ok::<_, RunfilesLayoutError>(())
        })?;

        // Filter before adding empty paths or root symlinks. Same-path insertions
        // silently replace, matching checkAndPut; they are not prefix conflicts.
        let mut filtered = SmallMap::new();
        for (path, artifact) in &working {
            let obscuring = path.rmatch_indices('/').find_map(|(index, _)| {
                let prefix = &path[..index];
                working.get(prefix).map(|ancestor| (prefix, ancestor))
            });
            if let Some((ancestor_path, ancestor)) = obscuring {
                let suffix = &path[ancestor_path.len() + 1..];
                if !same_location_below(ancestor, suffix, artifact) {
                    diagnostics.push(RunfilesLayoutDiagnostic::Obscured {
                        path: path.clone(),
                        artifact: artifact.clone(),
                        ancestor_path: ancestor_path.into(),
                        ancestor: ancestor.clone(),
                        policy: runfiles.conflict_policy,
                    });
                }
            } else {
                filtered.insert(
                    path.clone(),
                    RunfilesLayoutTarget::Artifact(artifact.clone()),
                );
            }
        }
        runfiles.empty_filenames.visit(|path| {
            validate_path(path)?;
            filtered.insert(path.as_str().into(), RunfilesLayoutTarget::Empty);
            Ok::<_, RunfilesLayoutError>(())
        })?;

        let mut final_entries = SmallMap::new();
        let mut saw_workspace = false;
        for (path, target) in filtered {
            let path: CompactString = if let Some(external) = path.strip_prefix("../") {
                saw_workspace |= below_prefix(external, &runfiles.repository_prefix);
                external.into()
            } else {
                saw_workspace = true;
                format!("{}/{path}", runfiles.repository_prefix).into()
            };
            final_entries.insert(path, target);
        }
        runfiles.root_symlinks.visit(|link| {
            validate_path(&link.path)?;
            constituents.insert(link.artifact.clone());
            saw_workspace |= below_prefix(&link.path, &runfiles.repository_prefix);
            if let Some(artifact) = checked_artifact(&mut diagnostics, &link.path, &link.artifact) {
                final_entries.insert(link.path.clone(), RunfilesLayoutTarget::Artifact(artifact));
            }
            Ok::<_, RunfilesLayoutError>(())
        })?;
        if let Some(mapping) = &self.repo_mapping_manifest {
            if let Some(artifact) = checked_artifact(&mut diagnostics, "_repo_mapping", mapping) {
                final_entries.insert(
                    "_repo_mapping".into(),
                    RunfilesLayoutTarget::Artifact(artifact),
                );
            }
        }
        if !saw_workspace {
            final_entries.insert(
                format!("{}/.runfile", runfiles.repository_prefix).into(),
                RunfilesLayoutTarget::Empty,
            );
        }
        constituents.insert(self.input_manifest.clone());
        if let Some(manifest) = &self.manifest {
            constituents.insert(manifest.clone());
        }
        if let Some(mapping) = &self.repo_mapping_manifest {
            constituents.insert(mapping.clone());
        }
        let mut entries = final_entries
            .into_iter()
            .map(|(path, target)| RunfilesLayoutEntry { path, target })
            .collect::<Vec<_>>();
        // PathFragment.compareTo delegates to Java String.compareTo.
        entries.sort_by(|left, right| left.path.encode_utf16().cmp(right.path.encode_utf16()));
        Ok(RunfilesLayout {
            support: self,
            entries,
            constituents: constituents
                .into_iter()
                .filter(|artifact| artifact != &self.tree)
                .collect(),
            diagnostics,
        })
    }
}

fn validate_path(path: &str) -> Result<(), RunfilesLayoutError> {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('\0')
        || path
            .split('/')
            .any(|component| matches!(component, "" | "." | ".."))
    {
        return Err(RunfilesLayoutError::InvalidPath(path.into()));
    }
    Ok(())
}

fn checked_artifact(
    diagnostics: &mut Vec<RunfilesLayoutDiagnostic>,
    path: &str,
    artifact: &AnalysisArtifact,
) -> Option<AnalysisArtifact> {
    if matches!(artifact, AnalysisArtifact::Derived { output, .. } if output.kind() == ActionOutputKind::RunfilesTree)
    {
        diagnostics.push(RunfilesLayoutDiagnostic::NestedRunfilesTree {
            path: path.into(),
            artifact: artifact.clone(),
        });
        None
    } else {
        Some(artifact.clone())
    }
}

fn artifact_path(artifact: &AnalysisArtifact) -> Result<CompactString, RunfilesLayoutError> {
    let (repo, relative) = match artifact {
        AnalysisArtifact::Source(label) => {
            // Source short_path is the retained canonical label's runfiles projection.
            let path = artifact.short_path();
            let relative = if label.package().repo().is_root() {
                path.as_ref()
            } else {
                path.strip_prefix("../").expect("external source prefix")
            };
            validate_path(relative)?;
            return Ok(path.as_ref().into());
        }
        AnalysisArtifact::Derived { owner, output } => {
            (owner.label().package().repo(), output.path())
        }
    };
    validate_path(relative)?;
    Ok(if repo.is_root() {
        relative.into()
    } else {
        format!("../{}/{relative}", repo.as_str()).into()
    })
}

fn below_prefix(path: &str, prefix: &str) -> bool {
    path == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('/'))
}

fn same_location_below(
    ancestor: &AnalysisArtifact,
    suffix: &str,
    child: &AnalysisArtifact,
) -> bool {
    let same_namespace = match (ancestor, child) {
        (AnalysisArtifact::Source(left), AnalysisArtifact::Source(right)) => {
            left.package().repo() == right.package().repo()
        }
        (
            AnalysisArtifact::Derived { owner: left, .. },
            AnalysisArtifact::Derived { owner: right, .. },
        ) => {
            left.label().package().repo() == right.label().package().repo()
                && left.configuration() == right.configuration()
        }
        _ => false,
    };
    same_namespace && format!("{}/{suffix}", ancestor.path()) == child.path()
}

#[cfg(test)]
mod tests;
