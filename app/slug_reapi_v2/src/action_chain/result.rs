//! Typed attempt results: local metadata never impersonates a REAPI response.

use super::*;

#[derive(Debug, Clone)]
pub struct LocalManifestResult {
    output: ActionOutput,
    pub(super) bytes: Arc<[u8]>,
    digest: ReapiDigest,
}
impl LocalManifestResult {
    fn new(output: ActionOutput, bytes: Arc<[u8]>) -> Self {
        let digest = ReapiDigest::of_bytes(&bytes);
        Self {
            output,
            bytes,
            digest,
        }
    }
    pub fn output(&self) -> &ActionOutput {
        &self.output
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn digest(&self) -> &ReapiDigest {
        &self.digest
    }
}

/// One entry per planned action, including metadata-only virtual tree completion.
#[derive(Debug, Clone)]
pub enum ActionChainStepResult {
    Remote(RemoteExecutionResult),
    ArtifactSymlink(ArtifactSymlinkResult),
    Manifest(LocalManifestResult),
    SymlinkTree(LocalManifestResult),
    RunfilesTree { output: ActionOutput },
}
impl ActionChainStepResult {
    pub fn remote(&self) -> Option<&RemoteExecutionResult> {
        match self {
            Self::Remote(result) => Some(result),
            _ => None,
        }
    }
    pub fn local_file(&self) -> Option<&LocalManifestResult> {
        match self {
            Self::Manifest(file) | Self::SymlinkTree(file) => Some(file),
            _ => None,
        }
    }
    pub(crate) fn from_runfiles(action: PreparedRunfilesAction) -> Self {
        match action {
            PreparedRunfilesAction::Manifest { output, bytes } => {
                Self::Manifest(LocalManifestResult::new(output, bytes))
            }
            PreparedRunfilesAction::SymlinkTree { output, bytes } => {
                Self::SymlinkTree(LocalManifestResult::new(output, bytes))
            }
            PreparedRunfilesAction::RunfilesTree { output } => Self::RunfilesTree { output },
        }
    }
}
