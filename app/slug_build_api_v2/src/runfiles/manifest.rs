//! Pure byte encoders over retained runfiles owners and caller-owned target paths.

use std::fmt;

use compact_str::CompactString;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::ActionOutputKind;
use crate::AnalysisArtifact;
use crate::AnalysisValueKind;
use crate::RunfilesConflictPolicy;
use crate::RunfilesLayout;
use crate::RunfilesLayoutDiagnostic;
use crate::RunfilesLayoutTarget;
use crate::RunfilesRepositoryMapping;
use crate::RunfilesSupportActionSpec;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SourceManifestError<E> {
    Diagnostic(RunfilesLayoutDiagnostic),
    UnresolvedSymlink(AnalysisArtifact),
    InvalidTarget(AnalysisArtifact),
    Resolve(E),
}

impl<E: fmt::Display> fmt::Display for SourceManifestError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Diagnostic(diagnostic) => {
                write!(f, "source manifest has invalid runfiles: {diagnostic:?}")
            }
            Self::UnresolvedSymlink(artifact) => write!(
                f,
                "source manifest unresolved symlink is unsupported: {artifact:?}"
            ),
            Self::InvalidTarget(artifact) => write!(
                f,
                "source manifest target must be an absolute path without NUL: {artifact:?}"
            ),
            Self::Resolve(error) => error.fmt(f),
        }
    }
}

impl<E: fmt::Debug + fmt::Display> std::error::Error for SourceManifestError<E> {}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum RunfilesManifestError {
    NotRepositoryMappingAction,
    NonArtifactFile,
    InvalidRootSymlinkPath(CompactString),
}

impl fmt::Display for RunfilesManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotRepositoryMappingAction => write!(
                f,
                "repository mapping manifest requires its retained action"
            ),
            Self::NonArtifactFile => {
                write!(f, "repository mapping runfiles must contain artifacts")
            }
            Self::InvalidRootSymlinkPath(path) => {
                write!(f, "repository mapping root symlink path is invalid: {path}")
            }
        }
    }
}

impl std::error::Error for RunfilesManifestError {}

impl RunfilesLayout<'_> {
    /// The resolver supplies the artifact's absolute target path; this method
    /// performs no observation and grants no authority to read that path.
    /// Warn-policy diagnostics remain available through `diagnostics()`.
    pub fn source_manifest_bytes<E>(
        &self,
        mut resolve: impl FnMut(&AnalysisArtifact) -> Result<String, E>,
    ) -> Result<Vec<u8>, SourceManifestError<E>> {
        for diagnostic in self.diagnostics() {
            if matches!(
                diagnostic,
                RunfilesLayoutDiagnostic::NestedRunfilesTree { .. }
                    | RunfilesLayoutDiagnostic::Obscured {
                        policy: RunfilesConflictPolicy::Error,
                        ..
                    }
            ) {
                return Err(SourceManifestError::Diagnostic(diagnostic.clone()));
            }
        }
        // Unsupported target kinds fail before any resolver callback.
        for entry in self.entries() {
            if let RunfilesLayoutTarget::Artifact(
                artifact @ AnalysisArtifact::Derived { output, .. },
            ) = entry.target()
                && output.kind() == ActionOutputKind::Symlink
            {
                return Err(SourceManifestError::UnresolvedSymlink(artifact.clone()));
            }
        }
        let mut bytes = Vec::new();
        for entry in self.entries() {
            let target = match entry.target() {
                RunfilesLayoutTarget::Empty => None,
                RunfilesLayoutTarget::Artifact(artifact) => {
                    let path = resolve(artifact).map_err(SourceManifestError::Resolve)?;
                    if !path.starts_with('/') || path.contains('\0') {
                        return Err(SourceManifestError::InvalidTarget(artifact.clone()));
                    }
                    Some(path)
                }
            };
            let escaped = entry.path().contains([' ', '\n'])
                || target.as_ref().is_some_and(|path| path.contains('\n'));
            if escaped {
                bytes.push(b' ');
            }
            append_path(&mut bytes, entry.path(), escaped, true);
            bytes.push(b' ');
            if let Some(target) = target {
                append_path(&mut bytes, &target, escaped, false);
            }
            bytes.push(b'\n');
        }
        Ok(bytes)
    }
}

// Bazel's ISO-8859-1 writer preserves internal raw bytes. Slug valid Unicode
// therefore emits UTF-8 directly, changing only the specified ASCII characters.
fn append_path(bytes: &mut Vec<u8>, path: &str, escaped: bool, logical: bool) {
    if !escaped {
        bytes.extend_from_slice(path.as_bytes());
        return;
    }
    for byte in path.bytes() {
        match byte {
            b' ' if logical => bytes.extend_from_slice(b"\\s"),
            b'\n' => bytes.extend_from_slice(b"\\n"),
            b'\\' => bytes.extend_from_slice(b"\\b"),
            _ => bytes.push(byte),
        }
    }
}

impl RunfilesSupportActionSpec {
    /// Repo presence comes from the raw runfiles owner, never the filtered layout.
    pub fn repo_mapping_manifest_bytes(&self) -> Result<Vec<u8>, RunfilesManifestError> {
        let Self::RepoMappingManifest {
            support,
            packages,
            workspace_name,
            emit_compact_repo_mapping,
            ..
        } = self
        else {
            return Err(RunfilesManifestError::NotRepositoryMappingAction);
        };
        let mut present = SmallSet::<CompactString>::new();
        if !support.runfiles.symlinks.is_empty() {
            present.insert("".into());
        }
        support.runfiles.root_symlinks.visit(|link| {
            let first = link.path.split('/').next().unwrap_or_default();
            if first.is_empty() || matches!(first, "." | "..") {
                return Err(RunfilesManifestError::InvalidRootSymlinkPath(
                    link.path.clone(),
                ));
            }
            present.insert(first.into());
            Ok(())
        })?;
        support.runfiles.files.visit(|value| {
            let AnalysisValueKind::Artifact(artifact) = value.kind() else {
                return Err(RunfilesManifestError::NonArtifactFile);
            };
            let repo = match artifact {
                AnalysisArtifact::Source(label) => label.package().repo(),
                AnalysisArtifact::Derived { owner, .. } => owner.label().package().repo(),
            };
            present.insert(repo.as_str().into());
            Ok(())
        })?;
        let mut mappings = SmallMap::new();
        packages.visit(|package| {
            mappings
                .entry(package.package().repo().clone())
                .or_insert_with(|| package.mapping().clone());
            Ok::<_, RunfilesManifestError>(())
        })?;
        let mut mappings = mappings.into_iter().collect::<Vec<_>>();
        mappings.sort_by(|(left, _), (right, _)| left.as_str().cmp(right.as_str()));
        let mut bytes = Vec::new();
        // Several repositories can share one immutable mapping slice. Reuse its
        // filtered rows within this invocation, including noncompact encoding.
        let mut relevant_by_slice = SmallMap::new();
        let mut index = 0;
        while index < mappings.len() {
            let (repo, mapping) = &mappings[index];
            let mut next = index + 1;
            if *emit_compact_repo_mapping {
                while next < mappings.len()
                    && compact_pair(
                        repo.as_str(),
                        mapping,
                        mappings[next].0.as_str(),
                        &mappings[next].1,
                    )
                {
                    next += 1;
                }
            }
            let source = if next == index + 1 {
                repo.as_str().to_owned()
            } else {
                let (prefix, _) = repo.as_str().rsplit_once('+').expect("checked prefix");
                format!("{prefix}+*")
            };
            let entries = mapping.entries();
            let relevant = relevant_by_slice
                .entry((entries.as_ptr() as usize, entries.len()))
                .or_insert_with(|| {
                    let mut relevant = entries
                        .iter()
                        .filter(|(apparent, canonical)| {
                            !apparent.as_str().is_empty() && present.contains(canonical.as_str())
                        })
                        .collect::<Vec<_>>();
                    relevant.sort_by(|(left, _), (right, _)| left.as_str().cmp(right.as_str()));
                    relevant
                });
            for (apparent, canonical) in relevant.iter() {
                bytes.extend_from_slice(source.as_bytes());
                bytes.push(b',');
                bytes.extend_from_slice(apparent.as_str().as_bytes());
                bytes.push(b',');
                bytes.extend_from_slice(if canonical.is_root() {
                    workspace_name.as_bytes()
                } else {
                    canonical.as_str().as_bytes()
                });
                bytes.push(b'\n');
            }
            index = next;
        }
        Ok(bytes)
    }
}

fn compact_pair(
    left_name: &str,
    left: &RunfilesRepositoryMapping,
    right_name: &str,
    right: &RunfilesRepositoryMapping,
) -> bool {
    let Some(group) = left.compact_group() else {
        return false;
    };
    let Some((prefix, _)) = left_name.rsplit_once('+') else {
        return false;
    };
    right.compact_group() == Some(group)
        && right_name
            .rsplit_once('+')
            .is_some_and(|(right_prefix, _)| prefix == right_prefix)
        && (std::ptr::eq(left.entries(), right.entries()) || left.entries() == right.entries())
}

#[cfg(test)]
mod tests;
