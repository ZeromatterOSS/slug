//! Workspace-global status descriptors, not status contents or execution authority.
use allocative::Allocative;
use dupe::Dupe;
use slug_workspace_v2::NormalizedAbsolutePath;

/// Configuration-independent build-data root. Normalization remains owned by
/// the workspace primitive; OS-native identity is never a displayed string.
#[derive(Debug, Clone, Dupe, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub struct WorkspaceStatusRoot(NormalizedAbsolutePath);

impl WorkspaceStatusRoot {
    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        &self.0
    }
    pub fn path(&self) -> &'static str {
        "bazel-out"
    }
}

#[derive(
    Debug, Clone, Copy, Dupe, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative
)]
pub enum WorkspaceStatusArtifactKind {
    Stable,
    Volatile,
}

#[derive(Debug, Clone, Dupe, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub struct WorkspaceStatusArtifact {
    root: WorkspaceStatusRoot,
    kind: WorkspaceStatusArtifactKind,
}

impl WorkspaceStatusArtifact {
    pub fn root(&self) -> &WorkspaceStatusRoot {
        &self.root
    }
    pub fn kind(&self) -> WorkspaceStatusArtifactKind {
        self.kind
    }
    /// Artifact.ConstantMetadataArtifact policy only: this is not a digest and
    /// conveys no authority to refresh, cache, or materialize volatile bytes.
    pub fn is_constant_metadata(&self) -> bool {
        self.kind == WorkspaceStatusArtifactKind::Volatile
    }
    pub fn path(&self) -> &'static str {
        match self.kind {
            WorkspaceStatusArtifactKind::Stable => "bazel-out/stable-status.txt",
            WorkspaceStatusArtifactKind::Volatile => "bazel-out/volatile-status.txt",
        }
    }
    pub fn short_path(&self) -> &'static str {
        match self.kind {
            WorkspaceStatusArtifactKind::Stable => "stable-status.txt",
            WorkspaceStatusArtifactKind::Volatile => "volatile-status.txt",
        }
    }
}

#[derive(Debug, Clone, Dupe, Eq, PartialEq, Allocative)]
pub struct WorkspaceStatusArtifacts {
    stable: WorkspaceStatusArtifact,
    volatile: WorkspaceStatusArtifact,
}

impl WorkspaceStatusArtifacts {
    pub fn new(workspace: NormalizedAbsolutePath) -> Self {
        let root = WorkspaceStatusRoot(workspace);
        Self {
            stable: WorkspaceStatusArtifact {
                root: root.clone(),
                kind: WorkspaceStatusArtifactKind::Stable,
            },
            volatile: WorkspaceStatusArtifact {
                root,
                kind: WorkspaceStatusArtifactKind::Volatile,
            },
        }
    }
    pub fn stable(&self) -> &WorkspaceStatusArtifact {
        &self.stable
    }
    pub fn volatile(&self) -> &WorkspaceStatusArtifact {
        &self.volatile
    }
}

#[cfg(test)]
mod tests;
