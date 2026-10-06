//! Original-producer association over physically issued readonly status files.
use std::io;

use slug_build_api_v2::WorkspaceStatusArtifact;
use slug_build_api_v2::WorkspaceStatusArtifactKind;

use super::workspace_status_content::WorkspaceStatusContent;
use super::*;
use crate::runtime::workspace_status_publication_file::PublishedStatusFiles;

#[derive(Debug)]
pub(super) struct WorkspaceStatusFileInputs {
    status: Arc<WorkspaceStatusContent>,
    files: PublishedStatusFiles,
}
impl WorkspaceStatusFileInputs {
    pub(super) fn issued(status: Arc<WorkspaceStatusContent>, files: PublishedStatusFiles) -> Self {
        Self { status, files }
    }
    pub(super) fn read<'a>(
        &self,
        expected: &Arc<WorkspaceStatusContent>,
        descriptor: &WorkspaceStatusArtifact,
        scratch: &'a mut [u8],
    ) -> io::Result<&'a [u8]> {
        if !Arc::ptr_eq(expected, &self.status) {
            return Err(io::Error::other("status input original producer differs"));
        }
        let (index, original) = match descriptor.kind() {
            WorkspaceStatusArtifactKind::Stable => (0, self.status.artifacts.stable()),
            WorkspaceStatusArtifactKind::Volatile => (1, self.status.artifacts.volatile()),
        };
        if descriptor != original || descriptor.root().workspace() != &self.status.capture.workspace
        {
            return Err(io::Error::other(
                "status input descriptor association differs",
            ));
        }
        self.files.read(
            index,
            self.status.capture.workspace.as_path(),
            self.status.bytes(descriptor.kind()),
            scratch,
        )
    }
    #[cfg(all(test, target_os = "linux", target_env = "gnu"))]
    pub(super) fn historical_bytes(
        &self,
        kind: WorkspaceStatusArtifactKind,
    ) -> io::Result<Vec<u8>> {
        self.files.historical_bytes(match kind {
            WorkspaceStatusArtifactKind::Stable => 0,
            WorkspaceStatusArtifactKind::Volatile => 1,
        })
    }
    #[cfg(all(test, target_os = "linux", target_env = "gnu"))]
    pub(super) fn raw_fds(&self) -> [i32; 4] {
        self.files.raw_fds()
    }
    #[cfg(all(test, target_os = "linux", target_env = "gnu"))]
    pub(super) fn readonly_flags(&self) -> io::Result<[i32; 2]> {
        self.files.readonly_flags()
    }
    #[cfg(all(test, target_os = "linux", target_env = "gnu"))]
    pub(super) fn identities(&self) -> io::Result<[(u64, u64); 2]> {
        self.files.identities()
    }
}
