/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Typed, bounded acquisition state for one selected module-extension owner.
//! The bytes are a DICE input, not a physical extension working directory.

use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use compact_str::CompactString;
use dice::Demand;
use dice::DiceComputations;
use dice::InjectedKey;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use sha2::Digest;
use sha2::Sha256;
use slug_workspace_v2::NormalizedAbsolutePath;

use crate::HostRepositoryRelativePath;
use crate::HostSelectedExtensionOwner;

pub const SELECTED_EXTENSION_DOWNLOAD_LIMIT: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum SelectedExtensionDownloadError {
    InvalidUrl,
    UnsupportedUrl,
    UrlAuthentication,
    NonUnicodeOutput,
    Oversize,
    ChecksumMismatch,
    ForeignWorkspace,
    DuplicateOwner,
    UnauthorizedEntry,
}

impl fmt::Display for SelectedExtensionDownloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidUrl => "selected extension download URL is invalid",
            Self::UnsupportedUrl => "selected extension download URL scheme is unsupported",
            Self::UrlAuthentication => {
                "selected extension download URL authentication is unsupported"
            }
            Self::NonUnicodeOutput => "selected extension download output is not Unicode",
            Self::Oversize => "selected extension download exceeds the 32 MiB limit",
            Self::ChecksumMismatch => "selected extension download SHA-256 mismatch",
            Self::ForeignWorkspace => "selected extension download belongs to another workspace",
            Self::DuplicateOwner => "selected extension download contains duplicate owners",
            Self::UnauthorizedEntry => "selected extension download has an unauthorized entry",
        };
        f.write_str(message)
    }
}

impl std::error::Error for SelectedExtensionDownloadError {}

#[derive(Clone, PartialEq, Eq, Hash, Allocative)]
pub struct SelectedExtensionDownloadRequest {
    workspace: NormalizedAbsolutePath,
    owner: Arc<HostSelectedExtensionOwner>,
    output: HostRepositoryRelativePath,
    url: CompactString,
    sha256: [u8; 32],
    executable: bool,
}

impl SelectedExtensionDownloadRequest {
    pub fn new(
        workspace: NormalizedAbsolutePath,
        owner: Arc<HostSelectedExtensionOwner>,
        output: HostRepositoryRelativePath,
        url: impl Into<CompactString>,
        sha256: [u8; 32],
        executable: bool,
    ) -> Result<Self, SelectedExtensionDownloadError> {
        if output.as_path().to_str().is_none() {
            return Err(SelectedExtensionDownloadError::NonUnicodeOutput);
        }
        let url = url.into();
        let parsed =
            url::Url::parse(&url).map_err(|_| SelectedExtensionDownloadError::InvalidUrl)?;
        if !matches!(parsed.scheme(), "https" | "file") {
            return Err(SelectedExtensionDownloadError::UnsupportedUrl);
        }
        if !parsed.username().is_empty() || parsed.password().is_some() {
            return Err(SelectedExtensionDownloadError::UrlAuthentication);
        }
        Ok(Self {
            workspace,
            owner,
            output,
            url,
            sha256,
            executable,
        })
    }

    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        &self.workspace
    }

    pub fn owner(&self) -> &Arc<HostSelectedExtensionOwner> {
        &self.owner
    }

    pub fn output(&self) -> &HostRepositoryRelativePath {
        &self.output
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }

    pub fn executable(&self) -> bool {
        self.executable
    }

    pub fn sha256_hex(&self) -> String {
        hex::encode(self.sha256)
    }

    pub fn sha256_sri(&self) -> String {
        format!("sha256-{}", STANDARD.encode(self.sha256))
    }
}

impl fmt::Debug for SelectedExtensionDownloadRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SelectedExtensionDownloadRequest")
            .field("workspace", &self.workspace)
            .field("owner", &self.owner)
            .field("output", &self.output)
            .field("url", &"<redacted>")
            .field("sha256", &self.sha256_hex())
            .field("executable", &self.executable)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct SelectedExtensionDownloadVerified {
    request: Arc<SelectedExtensionDownloadRequest>,
    bytes: Arc<[u8]>,
}

impl SelectedExtensionDownloadVerified {
    pub fn new(
        request: Arc<SelectedExtensionDownloadRequest>,
        bytes: Arc<[u8]>,
    ) -> Result<Self, SelectedExtensionDownloadError> {
        if bytes.len() > SELECTED_EXTENSION_DOWNLOAD_LIMIT {
            return Err(SelectedExtensionDownloadError::Oversize);
        }
        if Sha256::digest(&*bytes).as_slice() != request.sha256() {
            return Err(SelectedExtensionDownloadError::ChecksumMismatch);
        }
        Ok(Self { request, bytes })
    }

    pub fn request(&self) -> &Arc<SelectedExtensionDownloadRequest> {
        &self.request
    }

    pub fn bytes(&self) -> &Arc<[u8]> {
        &self.bytes
    }
}

impl fmt::Debug for SelectedExtensionDownloadVerified {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SelectedExtensionDownloadVerified")
            .field("request", &self.request)
            .field("byte_count", &self.bytes.len())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum SelectedExtensionDownloadEpoch {
    Unauthorized,
    AuthorizedEmpty,
    Ready(Arc<SelectedExtensionDownloadVerified>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct SelectedExtensionDownloadEpochKey {
    workspace: NormalizedAbsolutePath,
    owner: Arc<HostSelectedExtensionOwner>,
}

impl SelectedExtensionDownloadEpochKey {
    pub fn new(workspace: NormalizedAbsolutePath, owner: Arc<HostSelectedExtensionOwner>) -> Self {
        Self { workspace, owner }
    }

    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        &self.workspace
    }

    pub fn owner(&self) -> &Arc<HostSelectedExtensionOwner> {
        &self.owner
    }
}

impl fmt::Display for SelectedExtensionDownloadEpochKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "selected-extension-download:{}:{:?}",
            self.workspace,
            self.owner.unique_name()
        )
    }
}

impl InjectedKey for SelectedExtensionDownloadEpochKey {
    type Value = SelectedExtensionDownloadEpoch;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

/// Immutable command snapshot. Core owns construction, injection, pruning, and restoration.
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct SelectedExtensionDownloadTransaction {
    workspace: NormalizedAbsolutePath,
    authorized: Arc<[Arc<HostSelectedExtensionOwner>]>,
    entries: Arc<[Arc<SelectedExtensionDownloadVerified>]>,
}

impl SelectedExtensionDownloadTransaction {
    pub fn new(
        workspace: NormalizedAbsolutePath,
        authorized: impl IntoIterator<Item = Arc<HostSelectedExtensionOwner>>,
        entries: impl IntoIterator<Item = Arc<SelectedExtensionDownloadVerified>>,
    ) -> Result<Self, SelectedExtensionDownloadError> {
        let authorized = authorized.into_iter().collect::<Vec<_>>();
        for (index, owner) in authorized.iter().enumerate() {
            if authorized[..index].contains(owner) {
                return Err(SelectedExtensionDownloadError::DuplicateOwner);
            }
        }
        let entries = entries.into_iter().collect::<Vec<_>>();
        for (index, entry) in entries.iter().enumerate() {
            if entry.request.workspace != workspace {
                return Err(SelectedExtensionDownloadError::ForeignWorkspace);
            }
            if !authorized.contains(&entry.request.owner) {
                return Err(SelectedExtensionDownloadError::UnauthorizedEntry);
            }
            if entries[..index]
                .iter()
                .any(|previous| previous.request.owner == entry.request.owner)
            {
                return Err(SelectedExtensionDownloadError::DuplicateOwner);
            }
        }
        Ok(Self {
            workspace,
            authorized: authorized.into(),
            entries: entries.into(),
        })
    }

    pub fn empty(workspace: NormalizedAbsolutePath) -> Self {
        Self {
            workspace,
            authorized: Arc::from([]),
            entries: Arc::from([]),
        }
    }

    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        &self.workspace
    }

    pub fn authorized(&self) -> &[Arc<HostSelectedExtensionOwner>] {
        &self.authorized
    }

    pub fn entries(&self) -> &[Arc<SelectedExtensionDownloadVerified>] {
        &self.entries
    }

    pub fn is_authorized(&self, owner: &HostSelectedExtensionOwner) -> bool {
        self.authorized
            .iter()
            .any(|candidate| candidate.as_ref() == owner)
    }

    pub fn get(
        &self,
        owner: &HostSelectedExtensionOwner,
    ) -> Option<&Arc<SelectedExtensionDownloadVerified>> {
        self.entries
            .iter()
            .find(|entry| entry.request.owner.as_ref() == owner)
    }
}

/// A completed call contributes its exact request to the terminal activation closure.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct UsedDownloadRequestKey {
    request: Arc<SelectedExtensionDownloadRequest>,
}

impl UsedDownloadRequestKey {
    pub fn new(request: Arc<SelectedExtensionDownloadRequest>) -> Self {
        Self { request }
    }
}

impl fmt::Display for UsedDownloadRequestKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "used-selected-extension-download:{}:{:?}",
            self.request.workspace,
            self.request.owner.unique_name()
        )
    }
}

#[async_trait]
impl Key for UsedDownloadRequestKey {
    type Value = ();

    async fn compute(&self, _: &mut DiceComputations, _: &CancellationContext) -> Self::Value {}

    fn equality(_: &Self::Value, _: &Self::Value) -> bool {
        true
    }

    fn provide<'a>(&'a self, demand: &mut Demand<'a>) {
        demand.provide_value_with(|| self.request.dupe());
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use dice::DynKey;

    use super::*;
    use crate::SourcePreparationNeeds;
    use crate::SourcePreparationNeedsError;
    use crate::host_repository_relative_path;

    fn digest(bytes: &[u8]) -> [u8; 32] {
        let mut result = [0; 32];
        result.copy_from_slice(Sha256::digest(bytes).as_slice());
        result
    }

    fn request(
        owner: &str,
        output: &str,
        url: &str,
        bytes: &[u8],
    ) -> Arc<SelectedExtensionDownloadRequest> {
        Arc::new(
            SelectedExtensionDownloadRequest::new(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
                Arc::new(HostSelectedExtensionOwner::testing(owner)),
                host_repository_relative_path(PathBuf::from(output)).unwrap(),
                url,
                digest(bytes),
                true,
            )
            .unwrap(),
        )
    }

    #[test]
    fn selected_download_request_retains_full_identity_and_formats_sha() {
        let bytes = b"abc";
        let first = request("crate+", "cargo-bazel", "file:///tmp/generator-a", bytes);
        let same = request("crate+", "cargo-bazel", "file:///tmp/generator-a", bytes);
        let changed_url = request("crate+", "cargo-bazel", "file:///tmp/generator-b", bytes);
        let changed_output = request("crate+", "other", "file:///tmp/generator-a", bytes);
        let changed_owner = request("another+", "cargo-bazel", "file:///tmp/generator-a", bytes);
        let changed_sha = request("crate+", "cargo-bazel", "file:///tmp/generator-a", b"abd");
        let changed_executable = SelectedExtensionDownloadRequest::new(
            first.workspace().clone(),
            first.owner().clone(),
            first.output().clone(),
            first.url(),
            *first.sha256(),
            false,
        )
        .unwrap();
        assert_eq!(first, same);
        assert_ne!(first, changed_url);
        assert_ne!(first, changed_output);
        assert_ne!(first, changed_owner);
        assert_ne!(first.as_ref(), changed_sha.as_ref());
        assert_ne!(first.as_ref(), &changed_executable);
        assert_eq!(
            first.sha256_hex(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            first.sha256_sri(),
            "sha256-ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0="
        );
        assert!(!format!("{first:?}").contains("file:///tmp/generator-a"));
    }

    #[test]
    fn selected_download_request_rejects_unsupported_sources() {
        let owner = Arc::new(HostSelectedExtensionOwner::testing("crate+"));
        let output = host_repository_relative_path(PathBuf::from("cargo-bazel")).unwrap();
        let create = |url: &str| {
            SelectedExtensionDownloadRequest::new(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
                owner.clone(),
                output.clone(),
                url,
                digest(b"abc"),
                true,
            )
        };
        assert_eq!(
            create("http://example.test/file").unwrap_err(),
            SelectedExtensionDownloadError::UnsupportedUrl
        );
        assert_eq!(
            create("https://user:secret@example.test/file").unwrap_err(),
            SelectedExtensionDownloadError::UrlAuthentication
        );
        assert_eq!(
            create("not a URL").unwrap_err(),
            SelectedExtensionDownloadError::InvalidUrl
        );
        assert!(host_repository_relative_path(PathBuf::from("../escape")).is_err());
    }

    #[test]
    fn selected_download_verified_bytes_are_bounded_authenticated_and_shared() {
        let request = request("crate+", "cargo-bazel", "file:///tmp/generator", b"abc");
        let source: Arc<[u8]> = Arc::from(&b"abc"[..]);
        let verified =
            SelectedExtensionDownloadVerified::new(request.clone(), source.clone()).unwrap();
        assert!(Arc::ptr_eq(verified.bytes(), &source));
        let copied = verified.dupe();
        assert!(Arc::ptr_eq(verified.bytes(), copied.bytes()));
        assert_eq!(
            SelectedExtensionDownloadVerified::new(request, Arc::from(&b"abd"[..])).unwrap_err(),
            SelectedExtensionDownloadError::ChecksumMismatch
        );
        assert!(!format!("{verified:?}").contains("abc"));
    }

    #[test]
    fn selected_download_epoch_and_transaction_distinguish_authorized_and_revoked() {
        let request = request("crate+", "cargo-bazel", "file:///tmp/generator", b"abc");
        let verified = Arc::new(
            SelectedExtensionDownloadVerified::new(request.clone(), Arc::from(&b"abc"[..]))
                .unwrap(),
        );
        let workspace = request.workspace().clone();
        let cold = SelectedExtensionDownloadTransaction::empty(workspace.clone());
        assert!(!cold.is_authorized(request.owner()));
        assert!(cold.get(request.owner()).is_none());
        let ready = SelectedExtensionDownloadTransaction::new(
            workspace.clone(),
            [request.owner().clone()],
            [verified.clone()],
        )
        .unwrap();
        assert!(ready.is_authorized(request.owner()));
        assert_eq!(ready.get(request.owner()), Some(&verified));
        assert!(
            !<SelectedExtensionDownloadEpochKey as InjectedKey>::equality(
                &SelectedExtensionDownloadEpoch::Unauthorized,
                &SelectedExtensionDownloadEpoch::Ready(verified.clone()),
            )
        );
        assert_eq!(
            SelectedExtensionDownloadTransaction::new(workspace.clone(), [], [verified.clone()])
                .unwrap_err(),
            SelectedExtensionDownloadError::UnauthorizedEntry
        );
        assert_eq!(
            SelectedExtensionDownloadTransaction::new(
                workspace.clone(),
                [request.owner().clone(), request.owner().clone()],
                []
            )
            .unwrap_err(),
            SelectedExtensionDownloadError::DuplicateOwner
        );
        assert_eq!(
            SelectedExtensionDownloadTransaction::new(
                workspace,
                [request.owner().clone()],
                [verified.clone(), verified]
            )
            .unwrap_err(),
            SelectedExtensionDownloadError::DuplicateOwner
        );
    }

    #[test]
    fn selected_download_need_union_conflicts_by_owner_and_used_key_provides_request() {
        let first = request("crate+", "cargo-bazel", "file:///tmp/generator-a", b"abc");
        let same = request("crate+", "cargo-bazel", "file:///tmp/generator-a", b"abc");
        let changed = request("crate+", "cargo-bazel", "file:///tmp/generator-b", b"abc");
        let other = request("another+", "cargo-bazel", "file:///tmp/generator-a", b"abc");
        let first_need = SourcePreparationNeeds::selected_extension_download((*first).clone());
        let same_need = SourcePreparationNeeds::selected_extension_download((*same).clone());
        let changed_need = SourcePreparationNeeds::selected_extension_download((*changed).clone());
        let other_need = SourcePreparationNeeds::selected_extension_download((*other).clone());
        assert_eq!(
            first_need
                .try_union(&same_need)
                .unwrap()
                .selected_extension_downloads()
                .len(),
            1
        );
        assert_eq!(
            first_need
                .try_union(&other_need)
                .unwrap()
                .selected_extension_downloads()
                .len(),
            2
        );
        assert!(matches!(
            first_need.try_union(&changed_need),
            Err(SourcePreparationNeedsError::ConflictingSelectedExtensionDownload { .. })
        ));
        let marker = DynKey::from_key(UsedDownloadRequestKey::new(first.clone()));
        let provided = marker
            .request_value::<Arc<SelectedExtensionDownloadRequest>>()
            .unwrap();
        assert!(Arc::ptr_eq(&first, &provided));
    }
}
