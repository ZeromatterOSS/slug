/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Source fetching and extraction for bzlmod.
//!
//! This module handles downloading source archives and git repositories,
//! verifying integrity, and extracting to the cache.

use std::borrow::Cow;
use std::io::Cursor;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use allocative::Allocative;
use bzip2::read::BzDecoder;
use flate2::read::GzDecoder;
use sha2::Digest;
use sha2::Sha256;
use slug_error::BuckErrorContext;
use slug_http::HttpClient;
use slug_http::HttpClientBuilder;
use slug_http::to_bytes;
use tar::Archive;
use xz2::read::XzDecoder;
use zip::ZipArchive;
use zstd::stream::read::Decoder as ZstdDecoder;

use crate::cache::ModuleCache;
use crate::integrity::verify_integrity;
use crate::registry::SourceInfo;

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct OverridePatchInput {
    pub label: String,
    pub path: PathBuf,
    pub digest: String,
    pub content: Vec<u8>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Allocative)]
pub struct OverridePatchInputs {
    pub digest: String,
    pub inputs: Vec<OverridePatchInput>,
    pub has_untracked_inputs: bool,
}

impl OverridePatchInputs {
    pub fn content_for_label(&self, label: &str) -> Option<&[u8]> {
        self.inputs
            .iter()
            .find(|input| input.label == label)
            .map(|input| input.content.as_slice())
    }
}

#[derive(Debug)]
enum PatchToolError {
    Spawn(std::io::Error),
    Write(std::io::Error),
    Wait(std::io::Error),
    Failed(String),
}

impl std::fmt::Display for PatchToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawn(e) => write!(f, "failed to spawn patch command: {}", e),
            Self::Write(e) => write!(f, "failed to write patch: {}", e),
            Self::Wait(e) => write!(f, "failed to wait for patch command: {}", e),
            Self::Failed(stderr) => write!(f, "{}", stderr),
        }
    }
}

fn run_patch_tool(
    program: &str,
    args: &[String],
    current_dir: Option<&Path>,
    patch_content: &[u8],
) -> Result<(), PatchToolError> {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if let Some(current_dir) = current_dir {
        cmd.current_dir(current_dir);
    }

    let mut child = cmd.spawn().map_err(PatchToolError::Spawn)?;
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin
            .write_all(patch_content)
            .map_err(PatchToolError::Write)?;
    }

    let output = child.wait_with_output().map_err(PatchToolError::Wait)?;
    if !output.status.success() {
        return Err(PatchToolError::Failed(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }
    Ok(())
}

fn patch_files_in_apply_order(
    patches: &crate::registry::RegistryFileMap,
) -> Vec<(&String, &String)> {
    patches.iter().collect()
}

fn patch_already_applied(dest_dir: &Path, strip: u32, patch_content: &[u8]) -> bool {
    let patch = String::from_utf8_lossy(patch_content).replace("\r\n", "\n");
    let mut current_path: Option<String> = None;
    let mut saw_hunk = false;
    let mut files = Vec::<(String, Vec<String>, Vec<String>)>::new();
    let mut added = Vec::<String>::new();
    let mut removed = Vec::<String>::new();

    for line in patch.lines() {
        if let Some(path) = line.strip_prefix("+++ ") {
            if let Some(previous_path) = current_path.take() {
                files.push((
                    previous_path,
                    std::mem::take(&mut added),
                    std::mem::take(&mut removed),
                ));
            }
            current_path = patch_file_path(path, strip);
            saw_hunk = false;
            continue;
        }
        if line.starts_with("@@ ") || line == "@@" {
            saw_hunk = true;
            continue;
        }
        if current_path.is_none() || !saw_hunk || line.starts_with("\\ ") {
            continue;
        }
        if line.starts_with("+++") || line.starts_with("---") {
            continue;
        }
        if let Some(content) = line.strip_prefix('+') {
            if !content.is_empty() {
                added.push(content.to_owned());
            }
        } else if let Some(content) = line.strip_prefix('-') {
            if !content.is_empty() {
                removed.push(content.to_owned());
            }
        }
    }

    if let Some(previous_path) = current_path {
        files.push((previous_path, added, removed));
    }

    if files.is_empty() {
        return false;
    }

    files.into_iter().all(|(path, added, removed)| {
        let file_path = dest_dir.join(path);
        let Ok(content) = std::fs::read_to_string(file_path) else {
            return false;
        };
        let content = content.replace("\r\n", "\n");
        added.iter().all(|line| content.contains(line))
            && removed.iter().all(|line| !content.contains(line))
    })
}

fn patch_file_path(path: &str, strip: u32) -> Option<String> {
    let path = path.split_whitespace().next()?;
    if path == "/dev/null" {
        return None;
    }
    let mut components = path.split('/').skip(strip as usize).peekable();
    if components.peek().is_none() {
        return None;
    }
    Some(components.collect::<Vec<_>>().join("/"))
}

struct TempPatchDir {
    path: PathBuf,
}

impl TempPatchDir {
    fn new() -> slug_error::Result<Self> {
        let base = std::env::temp_dir();
        for attempt in 0..100_u32 {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default();
            let path = base.join(format!(
                "slug-svo-module-patch-{}-{}-{}",
                std::process::id(),
                nanos,
                attempt
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => {
                    return Err(FetchError::PatchFailed {
                        patch: format!("failed to create temporary patch directory: {}", e),
                    }
                    .into());
                }
            }
        }
        Err(FetchError::PatchFailed {
            patch: "failed to create unique temporary patch directory".to_owned(),
        }
        .into())
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempPatchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn filter_patch_content_for_single_file(
    patch_content: &[u8],
    strip: u32,
    single_file: &str,
) -> slug_error::Result<Vec<u8>> {
    let patch = std::str::from_utf8(patch_content).map_err(|e| FetchError::PatchFailed {
        patch: format!("override patch is not valid UTF-8: {}", e),
    })?;
    let mut filtered = String::new();
    let mut section = Vec::<&str>::new();
    let mut section_is_git_diff = false;

    for line in patch.lines() {
        if line.starts_with("diff --git ") {
            append_single_file_patch_section(&section, strip, single_file, &mut filtered);
            section.clear();
            section_is_git_diff = true;
        } else if line.starts_with("--- ") && !section.is_empty() && !section_is_git_diff {
            append_single_file_patch_section(&section, strip, single_file, &mut filtered);
            section.clear();
        }

        section.push(line);
        if section.len() == 1 && !line.starts_with("diff --git ") {
            section_is_git_diff = false;
        }
    }
    append_single_file_patch_section(&section, strip, single_file, &mut filtered);

    Ok(filtered.into_bytes())
}

fn append_single_file_patch_section(
    section: &[&str],
    strip: u32,
    single_file: &str,
    filtered: &mut String,
) {
    if !patch_section_targets_single_file(section, strip, single_file) {
        return;
    }
    for line in section {
        filtered.push_str(line);
        filtered.push('\n');
    }
}

fn patch_section_targets_single_file(section: &[&str], strip: u32, single_file: &str) -> bool {
    let mut old_path = None;
    let mut new_path = None;

    for line in section {
        if let Some(path) = line.strip_prefix("--- ") {
            old_path = patch_file_path(path, strip);
        } else if let Some(path) = line.strip_prefix("+++ ") {
            new_path = patch_file_path(path, strip);
        } else if let Some(rest) = line.strip_prefix("diff --git ") {
            let mut parts = rest.split_whitespace();
            old_path.get_or_insert_with(|| {
                parts
                    .next()
                    .and_then(|path| patch_file_path(path, strip))
                    .unwrap_or_default()
            });
            new_path.get_or_insert_with(|| {
                parts
                    .next()
                    .and_then(|path| patch_file_path(path, strip))
                    .unwrap_or_default()
            });
        }
    }

    old_path.as_deref() == Some(single_file) && new_path.as_deref() == Some(single_file)
}

/// Errors that can occur during source fetching.
#[derive(Debug, slug_error::Error)]
#[slug(tag = Input)]
pub enum FetchError {
    #[error("Failed to download from URL: {url}")]
    DownloadFailed { url: String },

    #[error("All download URLs failed for module: {name}@{version}")]
    AllUrlsFailed { name: String, version: String },

    #[error("Failed to extract archive: {reason}")]
    ExtractionFailed { reason: String },

    #[error("Git clone failed: {reason}")]
    GitCloneFailed { reason: String },

    #[error("Unsupported archive format: {filename}")]
    UnsupportedFormat { filename: String },

    #[error("No source URL or git remote specified")]
    NoSourceSpecified,

    #[error("Failed to apply patch: {patch}")]
    PatchFailed { patch: String },
}

/// Source fetcher for downloading and extracting module sources.
#[derive(Clone)]
pub struct SourceFetcher {
    /// HTTP client for downloading archives.
    http_client: Arc<HttpClient>,

    /// Module cache.
    cache: Arc<ModuleCache>,
}

impl SourceFetcher {
    /// Create a new source fetcher.
    pub async fn new(cache: ModuleCache) -> slug_error::Result<Self> {
        let http_client = HttpClientBuilder::https_with_system_roots()
            .await?
            .with_max_redirects(10)
            .build();

        Ok(Self {
            http_client: Arc::new(http_client),
            cache: Arc::new(cache),
        })
    }

    /// Create a fetcher with an existing HTTP client.
    pub fn with_http_client(http_client: Arc<HttpClient>, cache: ModuleCache) -> Self {
        Self {
            http_client,
            cache: Arc::new(cache),
        }
    }

    /// Fetch a git repository directly to a destination directory.
    /// Used for git_override during module resolution.
    pub async fn fetch_git_direct(
        &self,
        source_info: &SourceInfo,
        dest_dir: &Path,
    ) -> slug_error::Result<()> {
        self.fetch_git(source_info, dest_dir).await
    }

    /// Fetch and extract an archive directly to a destination directory.
    /// Used for archive_override during module resolution.
    pub async fn fetch_archive_direct(
        &self,
        urls: &[String],
        integrity: Option<&str>,
        strip_prefix: Option<&str>,
        dest_dir: &Path,
    ) -> slug_error::Result<()> {
        if urls.is_empty() {
            return Err(FetchError::NoSourceSpecified.into());
        }

        // Try each URL until one succeeds
        let mut last_error = None;
        for url in urls {
            match self.download_archive(url).await {
                Ok(data) => {
                    // Verify integrity if specified
                    if let Some(integrity) = integrity {
                        verify_integrity(&data, integrity)?;
                    }
                    return self.extract_archive(&data, dest_dir, strip_prefix);
                }
                Err(e) => {
                    tracing::warn!("Failed to download from {}: {}", url, e);
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            FetchError::AllUrlsFailed {
                name: "archive_override".to_string(),
                version: "unknown".to_string(),
            }
            .into()
        }))
    }

    /// Apply root-main-repository override patches to a fetched override source.
    pub fn apply_local_override_patches(
        dest_dir: &Path,
        workspace_root: &Path,
        main_repo_name: Option<&str>,
        patches: &[String],
        patch_strip: u32,
    ) -> slug_error::Result<()> {
        Self::apply_local_override_patches_with_inputs(
            dest_dir,
            workspace_root,
            main_repo_name,
            patches,
            patch_strip,
            None,
        )
    }

    pub fn apply_local_override_patches_with_inputs(
        dest_dir: &Path,
        workspace_root: &Path,
        main_repo_name: Option<&str>,
        patches: &[String],
        patch_strip: u32,
        patch_inputs: Option<&OverridePatchInputs>,
    ) -> slug_error::Result<()> {
        for patch_label in patches {
            let patch_content = local_override_patch_content(
                patch_inputs,
                workspace_root,
                main_repo_name,
                patch_label,
            )?;
            Self::apply_patch_content(dest_dir, patch_label, patch_strip, patch_content.as_ref())?;
        }
        Ok(())
    }

    /// Digest the exact local patch files that affect a non-registry override
    /// cache directory. This keeps warm override fetches from reusing a source
    /// tree patched with older bytes when the root patch file changes.
    pub fn local_override_patch_digest(
        workspace_root: &Path,
        main_repo_name: Option<&str>,
        patches: &[String],
        patch_strip: u32,
    ) -> slug_error::Result<Option<String>> {
        Self::local_override_patch_digest_with_inputs(
            workspace_root,
            main_repo_name,
            patches,
            patch_strip,
            None,
        )
    }

    pub fn local_override_patch_digest_with_inputs(
        workspace_root: &Path,
        main_repo_name: Option<&str>,
        patches: &[String],
        patch_strip: u32,
        patch_inputs: Option<&OverridePatchInputs>,
    ) -> slug_error::Result<Option<String>> {
        if patches.is_empty() {
            return Ok(None);
        }

        let mut hasher = Sha256::new();
        hasher.update(b"slug-local-override-patches-v1");
        hasher.update((patches.len() as u64).to_le_bytes());
        hasher.update(patch_strip.to_le_bytes());
        for patch_label in patches {
            let patch_content = local_override_patch_content(
                patch_inputs,
                workspace_root,
                main_repo_name,
                patch_label,
            )?;
            hasher.update((patch_label.len() as u64).to_le_bytes());
            hasher.update(patch_label.as_bytes());
            hasher.update((patch_content.len() as u64).to_le_bytes());
            hasher.update(patch_content.as_ref());
        }
        Ok(Some(hex::encode(hasher.finalize())))
    }

    /// Fingerprint root-local override patch inputs plus patch commands for
    /// repository materialization cache identity.
    pub fn local_override_patch_effect_digest(
        workspace_root: &Path,
        main_repo_name: Option<&str>,
        patches: &[String],
        patch_strip: u32,
        patch_cmds: &[String],
    ) -> slug_error::Result<Option<String>> {
        Self::local_override_patch_effect_digest_with_inputs(
            workspace_root,
            main_repo_name,
            patches,
            patch_strip,
            patch_cmds,
            None,
        )
    }

    pub fn local_override_patch_effect_digest_with_inputs(
        workspace_root: &Path,
        main_repo_name: Option<&str>,
        patches: &[String],
        patch_strip: u32,
        patch_cmds: &[String],
        patch_inputs: Option<&OverridePatchInputs>,
    ) -> slug_error::Result<Option<String>> {
        if patches.is_empty() && patch_cmds.is_empty() && patch_strip == 0 {
            return Ok(None);
        }

        let mut hasher = Sha256::new();
        hasher.update(b"slug-local-override-patch-effect-v1");
        hasher.update((patches.len() as u64).to_le_bytes());
        hasher.update(patch_strip.to_le_bytes());
        for patch_label in patches {
            let patch_content = local_override_patch_content(
                patch_inputs,
                workspace_root,
                main_repo_name,
                patch_label,
            )?;
            hasher.update((patch_label.len() as u64).to_le_bytes());
            hasher.update(patch_label.as_bytes());
            hasher.update((patch_content.len() as u64).to_le_bytes());
            hasher.update(patch_content.as_ref());
        }
        hasher.update((patch_cmds.len() as u64).to_le_bytes());
        for cmd in patch_cmds {
            hasher.update((cmd.len() as u64).to_le_bytes());
            hasher.update(cmd.as_bytes());
        }
        Ok(Some(hex::encode(hasher.finalize())))
    }

    /// Apply root-local `single_version_override` patches to the registry
    /// `MODULE.bazel` contents only, matching Bazel's discovery-time behavior.
    pub fn apply_single_version_module_patches(
        module_content: &str,
        workspace_root: &Path,
        main_repo_name: Option<&str>,
        patches: &[String],
        patch_strip: u32,
    ) -> slug_error::Result<String> {
        Self::apply_single_version_module_patches_with_inputs(
            module_content,
            workspace_root,
            main_repo_name,
            patches,
            patch_strip,
            None,
        )
    }

    pub fn apply_single_version_module_patches_with_inputs(
        module_content: &str,
        workspace_root: &Path,
        main_repo_name: Option<&str>,
        patches: &[String],
        patch_strip: u32,
        patch_inputs: Option<&OverridePatchInputs>,
    ) -> slug_error::Result<String> {
        if patches.is_empty() {
            return Ok(module_content.to_owned());
        }

        let temp = TempPatchDir::new()?;
        let module_path = temp.path().join("MODULE.bazel");
        std::fs::write(&module_path, module_content)
            .buck_error_context("Failed to write temporary MODULE.bazel for override patches")?;

        for patch_label in patches {
            let patch_content = local_override_patch_content(
                patch_inputs,
                workspace_root,
                main_repo_name,
                patch_label,
            )?;
            let module_patch = filter_patch_content_for_single_file(
                patch_content.as_ref(),
                patch_strip,
                "MODULE.bazel",
            )?;
            if module_patch.is_empty() {
                continue;
            }
            Self::apply_patch_content(temp.path(), patch_label, patch_strip, &module_patch)?;
        }

        std::fs::read_to_string(&module_path)
            .buck_error_context("Failed to read patched temporary MODULE.bazel")
    }

    /// Run root-local override patch commands after source patching.
    pub fn apply_local_override_patch_cmds(
        dest_dir: &Path,
        module_name: &str,
        patch_cmds: &[String],
    ) -> slug_error::Result<()> {
        for cmd_str in patch_cmds {
            let shell = if cfg!(windows) { "cmd" } else { "sh" };
            let flag = if cfg!(windows) { "/c" } else { "-c" };
            let output = Command::new(shell)
                .arg(flag)
                .arg(cmd_str)
                .current_dir(dest_dir)
                .output()
                .map_err(|e| FetchError::PatchFailed {
                    patch: format!(
                        "failed to run patch_cmd '{}' for '{}': {}",
                        cmd_str, module_name, e
                    ),
                })?;

            if !output.status.success() {
                return Err(FetchError::PatchFailed {
                    patch: format!(
                        "patch_cmd '{}' for '{}' failed: {}{}",
                        cmd_str,
                        module_name,
                        String::from_utf8_lossy(&output.stderr),
                        String::from_utf8_lossy(&output.stdout)
                    ),
                }
                .into());
            }
        }
        Ok(())
    }

    /// Fetch and extract source for a module.
    ///
    /// Returns the path to the extracted source directory.
    pub async fn fetch_source(
        &self,
        registry_url: &str,
        name: &str,
        version: &str,
        source_info: &SourceInfo,
    ) -> slug_error::Result<PathBuf> {
        self.fetch_source_with_identity(registry_url, name, version, source_info, None)
            .await
    }

    /// Fetch and extract source for a module with additional materialization
    /// identity, such as root override patches.
    pub async fn fetch_source_with_identity(
        &self,
        registry_url: &str,
        name: &str,
        version: &str,
        source_info: &SourceInfo,
        source_identity: Option<&str>,
    ) -> slug_error::Result<PathBuf> {
        // Check if already fetched
        if self
            .cache
            .is_source_complete_with_identity(registry_url, name, version, source_identity)
        {
            tracing::debug!("Using cached source for {}@{}", name, version);
            return Ok(self.cache.source_dir_with_identity(
                registry_url,
                name,
                version,
                source_identity,
            ));
        }

        let dest_dir =
            self.cache
                .source_dir_with_identity(registry_url, name, version, source_identity);
        if dest_dir.exists() {
            tracing::debug!(
                "Removing incomplete cached source for {}@{} at {:?}",
                name,
                version,
                dest_dir
            );
            std::fs::remove_dir_all(&dest_dir).with_buck_error_context(|| {
                format!(
                    "Failed to remove incomplete cached source for {}@{} at {}",
                    name,
                    version,
                    dest_dir.display()
                )
            })?;
        }

        let dest_dir = self.cache.create_source_dir_with_identity(
            registry_url,
            name,
            version,
            source_identity,
        )?;

        if source_info.is_git() {
            self.fetch_git(source_info, &dest_dir).await?;
        } else {
            self.fetch_archive(name, version, source_info, &dest_dir)
                .await?;
        }

        // Apply overlay files (before patches, per Bazel convention)
        if !source_info.overlay.is_empty() {
            self.apply_overlays(&dest_dir, source_info, registry_url, name, version)
                .await?;
        }

        // Apply patches if any
        if !source_info.patches.is_empty() {
            self.apply_patches(&dest_dir, source_info, registry_url, name, version)
                .await?;
        }

        // Mark as complete
        self.cache.mark_source_complete_with_identity(
            registry_url,
            name,
            version,
            source_identity,
        )?;

        Ok(dest_dir)
    }

    /// Fetch and extract an archive source.
    async fn fetch_archive(
        &self,
        name: &str,
        version: &str,
        source_info: &SourceInfo,
        dest_dir: &Path,
    ) -> slug_error::Result<()> {
        let urls = source_info.get_urls();
        if urls.is_empty() {
            return Err(FetchError::NoSourceSpecified.into());
        }

        // Try to fetch from cached download first (by integrity hash)
        if let Some(integrity) = &source_info.integrity {
            if let Some(data) = self.cache.read_download(integrity)? {
                tracing::debug!("Using cached download for {}@{}", name, version);
                // Verify integrity
                verify_integrity(&data, integrity)?;
                // Extract
                return self.extract_archive(&data, dest_dir, source_info.strip_prefix.as_deref());
            }
        }

        // Try each URL until one succeeds
        let mut last_error = None;
        for url in &urls {
            match self.download_archive(url).await {
                Ok(data) => {
                    // Verify integrity if specified
                    if let Some(integrity) = &source_info.integrity {
                        verify_integrity(&data, integrity)?;
                        // Cache the download
                        self.cache.write_download(integrity, &data)?;
                    }

                    // Extract
                    return self.extract_archive(
                        &data,
                        dest_dir,
                        source_info.strip_prefix.as_deref(),
                    );
                }
                Err(e) => {
                    tracing::warn!("Failed to download from {}: {}", url, e);
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            FetchError::AllUrlsFailed {
                name: name.to_string(),
                version: version.to_string(),
            }
            .into()
        }))
    }

    /// Download an archive from a URL.
    async fn download_archive(&self, url: &str) -> slug_error::Result<Vec<u8>> {
        tracing::info!("Downloading archive from {}", url);

        if let Some(path) = url.strip_prefix("file://") {
            return std::fs::read(path)
                .with_buck_error_context(|| format!("Failed to read local archive from {}", url));
        }

        let response = self
            .http_client
            .get(url)
            .await
            .buck_error_context("Failed to download archive")?;

        let body = to_bytes(response.into_body()).await?;
        Ok(body.to_vec())
    }

    /// Extract an archive to a destination directory.
    fn extract_archive(
        &self,
        data: &[u8],
        dest_dir: &Path,
        strip_prefix: Option<&str>,
    ) -> slug_error::Result<()> {
        tracing::debug!(
            "Extracting archive to {:?} (strip_prefix: {:?})",
            dest_dir,
            strip_prefix
        );

        // Try gzip-compressed tar first
        if let Ok(()) = self.extract_tar_gz(data, dest_dir, strip_prefix) {
            return Ok(());
        }

        // Try plain tar
        if let Ok(()) = self.extract_tar(data, dest_dir, strip_prefix) {
            return Ok(());
        }

        // Try XZ-compressed tar
        if let Ok(()) = self.extract_tar_xz(data, dest_dir, strip_prefix) {
            return Ok(());
        }

        // Try zstd-compressed tar
        if let Ok(()) = self.extract_tar_zst(data, dest_dir, strip_prefix) {
            return Ok(());
        }

        // Try bzip2-compressed tar
        if let Ok(()) = self.extract_tar_bz2(data, dest_dir, strip_prefix) {
            return Ok(());
        }

        // Try zip
        if let Ok(()) = self.extract_zip(data, dest_dir, strip_prefix) {
            return Ok(());
        }

        // Log some bytes for debugging
        let preview = if data.len() > 100 {
            String::from_utf8_lossy(&data[..100]).to_string()
        } else {
            String::from_utf8_lossy(data).to_string()
        };
        tracing::warn!(
            "Archive extraction failed for {} bytes at {:?}. First bytes: {:?}",
            data.len(),
            dest_dir,
            preview
        );

        Err(FetchError::ExtractionFailed {
            reason: format!(
                "Unknown archive format ({} bytes, starts with {:02x?})",
                data.len(),
                &data[..data.len().min(8)]
            ),
        }
        .into())
    }

    /// Extract a gzip-compressed tar archive.
    fn extract_tar_gz(
        &self,
        data: &[u8],
        dest_dir: &Path,
        strip_prefix: Option<&str>,
    ) -> slug_error::Result<()> {
        extract_tar_gz_impl(data, dest_dir, strip_prefix)
    }

    /// Extract an XZ-compressed tar archive (.tar.xz).
    fn extract_tar_xz(
        &self,
        data: &[u8],
        dest_dir: &Path,
        strip_prefix: Option<&str>,
    ) -> slug_error::Result<()> {
        let decoder = XzDecoder::new(data);
        extract_tar_from_reader(decoder, dest_dir, strip_prefix)
    }

    /// Extract a zstd-compressed tar archive (.tar.zst).
    fn extract_tar_zst(
        &self,
        data: &[u8],
        dest_dir: &Path,
        strip_prefix: Option<&str>,
    ) -> slug_error::Result<()> {
        let decoder = ZstdDecoder::new(data).buck_error_context("Failed to create zstd decoder")?;
        extract_tar_from_reader(decoder, dest_dir, strip_prefix)
    }

    /// Extract a bzip2-compressed tar archive (.tar.bz2).
    fn extract_tar_bz2(
        &self,
        data: &[u8],
        dest_dir: &Path,
        strip_prefix: Option<&str>,
    ) -> slug_error::Result<()> {
        let decoder = BzDecoder::new(data);
        extract_tar_from_reader(decoder, dest_dir, strip_prefix)
    }

    /// Extract a plain tar archive (not implemented, placeholder).
    fn extract_tar(
        &self,
        _data: &[u8],
        _dest_dir: &Path,
        _strip_prefix: Option<&str>,
    ) -> slug_error::Result<()> {
        Err(FetchError::ExtractionFailed {
            reason: "Plain tar not yet supported".to_string(),
        }
        .into())
    }

    /// Extract a zip archive.
    fn extract_zip(
        &self,
        data: &[u8],
        dest_dir: &Path,
        strip_prefix: Option<&str>,
    ) -> slug_error::Result<()> {
        let cursor = Cursor::new(data);
        let mut archive = ZipArchive::new(cursor).map_err(|e| FetchError::ExtractionFailed {
            reason: format!("Failed to open zip archive: {}", e),
        })?;

        for i in 0..archive.len() {
            let mut file = archive
                .by_index(i)
                .map_err(|e| FetchError::ExtractionFailed {
                    reason: format!("Failed to read zip entry: {}", e),
                })?;

            let file_path = match file.enclosed_name() {
                Some(path) => path.to_owned(),
                None => continue, // Skip invalid paths
            };

            // Apply strip_prefix if specified
            let dest_path = if let Some(prefix) = strip_prefix {
                let stripped = file_path.strip_prefix(prefix).unwrap_or(&file_path);
                dest_dir.join(stripped)
            } else {
                dest_dir.join(&file_path)
            };

            // Skip if path is empty after stripping
            if dest_path == dest_dir {
                continue;
            }

            if file.is_dir() {
                std::fs::create_dir_all(&dest_path).map_err(|e| FetchError::ExtractionFailed {
                    reason: format!("Failed to create directory {:?}: {}", dest_path, e),
                })?;
            } else {
                // Create parent directories
                if let Some(parent) = dest_path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| FetchError::ExtractionFailed {
                        reason: format!("Failed to create parent directory {:?}: {}", parent, e),
                    })?;
                }

                // Extract file
                let mut outfile = std::fs::File::create(&dest_path).map_err(|e| {
                    FetchError::ExtractionFailed {
                        reason: format!("Failed to create file {:?}: {}", dest_path, e),
                    }
                })?;

                std::io::copy(&mut file, &mut outfile).map_err(|e| {
                    FetchError::ExtractionFailed {
                        reason: format!("Failed to write file {:?}: {}", dest_path, e),
                    }
                })?;

                // Set permissions on Unix
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Some(mode) = file.unix_mode() {
                        let _ = std::fs::set_permissions(
                            &dest_path,
                            std::fs::Permissions::from_mode(mode),
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// Fetch a git repository.
    async fn fetch_git(&self, source_info: &SourceInfo, dest_dir: &Path) -> slug_error::Result<()> {
        let remote = source_info
            .remote
            .as_ref()
            .ok_or(FetchError::NoSourceSpecified)?;

        let commit = source_info
            .commit
            .as_ref()
            .ok_or_else(|| FetchError::GitCloneFailed {
                reason: "No commit specified for git_repository".to_string(),
            })?;

        tracing::info!("Cloning git repository {} at {}", remote, commit);

        // Build git clone command
        let mut cmd = Command::new("git");
        cmd.arg("clone");

        // Use shallow clone if shallow_since is specified
        if let Some(shallow_since) = &source_info.shallow_since {
            cmd.arg("--shallow-since").arg(shallow_since);
        }

        cmd.arg("--single-branch").arg(remote).arg(dest_dir);

        let output = cmd.output().map_err(|e| FetchError::GitCloneFailed {
            reason: format!("Failed to execute git: {}", e),
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(FetchError::GitCloneFailed {
                reason: format!("git clone failed: {}", stderr),
            }
            .into());
        }

        // Checkout the specific commit
        let output = Command::new("git")
            .current_dir(dest_dir)
            .arg("checkout")
            .arg(commit)
            .output()
            .map_err(|e| FetchError::GitCloneFailed {
                reason: format!("Failed to execute git checkout: {}", e),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(FetchError::GitCloneFailed {
                reason: format!("git checkout failed: {}", stderr),
            }
            .into());
        }

        Ok(())
    }

    /// Apply patches to the source directory.
    async fn apply_patches(
        &self,
        dest_dir: &Path,
        source_info: &SourceInfo,
        registry_url: &str,
        name: &str,
        version: &str,
    ) -> slug_error::Result<()> {
        if source_info.patches.is_empty() {
            return Ok(());
        }

        tracing::info!(
            "Applying {} patches to {}@{}",
            source_info.patches.len(),
            name,
            version
        );

        for (patch_file, _integrity) in patch_files_in_apply_order(&source_info.patches) {
            // Download patch from registry: {base_url}/modules/{name}/{version}/patches/{patch_file}
            let patch_url = format!(
                "{}/modules/{}/{}/patches/{}",
                registry_url, name, version, patch_file
            );
            tracing::debug!("Fetching patch from {}", patch_url);

            let response = self
                .http_client
                .get(&patch_url)
                .await
                .with_buck_error_context(|| format!("Failed to fetch patch: {}", patch_file))?;

            let body = to_bytes(response.into_body()).await?;
            let patch_content = body.to_vec();

            Self::apply_patch_content(
                dest_dir,
                patch_file,
                source_info.patch_strip,
                &patch_content,
            )?;

            tracing::debug!("Applied patch: {}", patch_file);
        }

        Ok(())
    }

    fn apply_patch_content(
        dest_dir: &Path,
        patch_file: &str,
        strip: u32,
        patch_content: &[u8],
    ) -> slug_error::Result<()> {
        let patch_args = [
            format!("-p{}", strip),
            "--no-backup-if-mismatch".to_owned(),
            "-d".to_owned(),
            dest_dir.to_string_lossy().into_owned(),
        ];
        let patch_result = run_patch_tool("patch", &patch_args, None, patch_content);
        match &patch_result {
            Ok(()) => return Ok(()),
            Err(PatchToolError::Spawn(e)) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => {}
        }

        let git_args = [
            "apply".to_owned(),
            format!("-p{}", strip),
            "--unsafe-paths".to_owned(),
            "--whitespace=nowarn".to_owned(),
        ];
        match run_patch_tool("git", &git_args, Some(dest_dir), patch_content) {
            Ok(()) => return Ok(()),
            Err(git_err) => {
                let reverse_check_args = [
                    "apply".to_owned(),
                    format!("-p{}", strip),
                    "--reverse".to_owned(),
                    "--check".to_owned(),
                    "--unsafe-paths".to_owned(),
                    "--whitespace=nowarn".to_owned(),
                ];
                if run_patch_tool("git", &reverse_check_args, Some(dest_dir), patch_content).is_ok()
                {
                    tracing::debug!(
                        patch = patch_file,
                        "Registry patch is already applied; treating warm cache as complete"
                    );
                    return Ok(());
                }
                if patch_already_applied(dest_dir, strip, patch_content) {
                    tracing::debug!(
                        patch = patch_file,
                        "Registry patch content is already present; treating warm cache as complete"
                    );
                    return Ok(());
                }
                let patch_context = match patch_result {
                    Ok(()) => String::new(),
                    Err(err) => format!("patch: {}; ", err),
                };
                return Err(FetchError::PatchFailed {
                    patch: format!(
                        "{}: failed to apply with `patch` or `git apply`: {}git apply: {}",
                        patch_file, patch_context, git_err
                    ),
                }
                .into());
            }
        }
    }

    /// Apply overlay files on top of the extracted source directory.
    ///
    /// Overlay files are fetched from the BCR at
    /// `{registry_url}/modules/{name}/{version}/overlay/{filename}`
    /// and written into the destination directory, overwriting any existing files.
    /// This happens BEFORE patches are applied, matching Bazel's behavior.
    async fn apply_overlays(
        &self,
        dest_dir: &Path,
        source_info: &SourceInfo,
        registry_url: &str,
        name: &str,
        version: &str,
    ) -> slug_error::Result<()> {
        if source_info.overlay.is_empty() {
            return Ok(());
        }

        tracing::info!(
            "Applying {} overlay file(s) to {}@{}",
            source_info.overlay.len(),
            name,
            version
        );

        for (overlay_path, _integrity) in &source_info.overlay {
            let overlay_url = format!(
                "{}/modules/{}/{}/overlay/{}",
                registry_url, name, version, overlay_path
            );
            tracing::debug!("Fetching overlay from {}", overlay_url);

            let response = self
                .http_client
                .get(&overlay_url)
                .await
                .with_buck_error_context(|| {
                    format!("Failed to fetch overlay file: {}", overlay_path)
                })?;

            let body = to_bytes(response.into_body()).await?;
            let overlay_content = body.to_vec();

            // Write overlay file to destination, creating parent dirs as needed
            let dest_file = dest_dir.join(overlay_path);
            if let Some(parent) = dest_file.parent() {
                std::fs::create_dir_all(parent).map_err(|e| FetchError::ExtractionFailed {
                    reason: format!(
                        "Failed to create overlay directory for '{}': {}",
                        overlay_path, e
                    ),
                })?;
            }
            std::fs::write(&dest_file, &overlay_content).map_err(|e| {
                FetchError::ExtractionFailed {
                    reason: format!("Failed to write overlay file '{}': {}", overlay_path, e),
                }
            })?;

            tracing::debug!("Applied overlay: {}", overlay_path);
        }

        Ok(())
    }
}

fn local_override_patch_content<'a>(
    patch_inputs: Option<&'a OverridePatchInputs>,
    workspace_root: &Path,
    main_repo_name: Option<&str>,
    patch_label: &str,
) -> slug_error::Result<Cow<'a, [u8]>> {
    if let Some(patch_inputs) = patch_inputs {
        return patch_inputs
            .content_for_label(patch_label)
            .map(Cow::Borrowed)
            .ok_or_else(|| {
                FetchError::PatchFailed {
                    patch: format!(
                        "tracked override patch input '{}' was not provided",
                        patch_label
                    ),
                }
                .into()
            });
    }

    let patch_path = override_patch_label_path(workspace_root, main_repo_name, patch_label)?;
    let patch_content = std::fs::read(&patch_path).with_buck_error_context(|| {
        format!(
            "Failed to read override patch '{}' at {}",
            patch_label,
            patch_path.display()
        )
    })?;
    Ok(Cow::Owned(patch_content))
}

pub fn override_patch_label_path(
    workspace_root: &Path,
    main_repo_name: Option<&str>,
    raw_label: &str,
) -> slug_error::Result<PathBuf> {
    let label =
        crate::repo_mapping::canonicalize_label_with_package_context(raw_label, "", "", None)
            .ok_or_else(|| FetchError::PatchFailed {
                patch: format!("invalid override patch label '{}'", raw_label),
            })?;
    let repo = label.repo().as_str();
    if !repo.is_empty() && Some(repo) != main_repo_name {
        return Err(FetchError::PatchFailed {
            patch: format!(
                "invalid override patch label '{}': only patches from the main repository are supported",
                raw_label
            ),
        }
        .into());
    }

    let mut path = workspace_root.to_path_buf();
    if !label.package().is_empty() {
        path.push(label.package());
    }
    path.push(label.target());
    Ok(path)
}

/// Extract a gzip-compressed tar archive (standalone function for testing).
fn extract_tar_gz_impl(
    data: &[u8],
    dest_dir: &Path,
    strip_prefix: Option<&str>,
) -> slug_error::Result<()> {
    let decoder = GzDecoder::new(data);
    extract_tar_from_reader(decoder, dest_dir, strip_prefix)
}

/// Extract a tar archive from any reader (generic over decompression).
fn extract_tar_from_reader<R: std::io::Read>(
    reader: R,
    dest_dir: &Path,
    strip_prefix: Option<&str>,
) -> slug_error::Result<()> {
    let mut archive = Archive::new(reader);

    for entry_result in archive
        .entries()
        .map_err(|e| FetchError::ExtractionFailed {
            reason: e.to_string(),
        })?
    {
        let mut entry = entry_result.map_err(|e| FetchError::ExtractionFailed {
            reason: e.to_string(),
        })?;

        let path = entry.path().map_err(|e| FetchError::ExtractionFailed {
            reason: e.to_string(),
        })?;

        // Apply strip_prefix if specified
        let dest_path = if let Some(prefix) = strip_prefix {
            let path_str = path.to_string_lossy();
            if let Some(stripped) = path_str.strip_prefix(prefix) {
                let stripped = stripped.trim_start_matches('/');
                if stripped.is_empty() {
                    continue;
                }
                dest_dir.join(stripped)
            } else if path_str.starts_with(prefix.trim_end_matches('/')) {
                // Handle case where prefix doesn't have trailing slash
                let prefix_with_slash = format!("{}/", prefix.trim_end_matches('/'));
                if let Some(stripped) = path_str.strip_prefix(&prefix_with_slash) {
                    if stripped.is_empty() {
                        continue;
                    }
                    dest_dir.join(stripped)
                } else {
                    continue;
                }
            } else {
                continue;
            }
        } else {
            dest_dir.join(&*path)
        };

        // Create parent directories
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| FetchError::ExtractionFailed {
                reason: format!("Failed to create directory {:?}: {}", parent, e),
            })?;
        }

        // Extract based on entry type
        let entry_type = entry.header().entry_type();
        if entry_type.is_dir() {
            std::fs::create_dir_all(&dest_path).map_err(|e| FetchError::ExtractionFailed {
                reason: format!("Failed to create directory {:?}: {}", dest_path, e),
            })?;
        } else if entry_type.is_file() {
            let mut file =
                std::fs::File::create(&dest_path).map_err(|e| FetchError::ExtractionFailed {
                    reason: format!("Failed to create file {:?}: {}", dest_path, e),
                })?;
            std::io::copy(&mut entry, &mut file).map_err(|e| FetchError::ExtractionFailed {
                reason: format!("Failed to write file {:?}: {}", dest_path, e),
            })?;

            // Set file permissions on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(mode) = entry.header().mode() {
                    let permissions = std::fs::Permissions::from_mode(mode);
                    let _ = std::fs::set_permissions(&dest_path, permissions);
                }
            }
        } else if entry_type.is_symlink() {
            if let Ok(link_name) = entry.link_name() {
                if let Some(link_target) = link_name {
                    #[cfg(unix)]
                    {
                        let _ = std::os::unix::fs::symlink(&*link_target, &dest_path);
                    }
                    #[cfg(not(unix))]
                    {
                        let source_path =
                            resolve_tar_link_target(&link_target, dest_dir, strip_prefix);
                        let _ = std::fs::copy(source_path, &dest_path);
                    }
                }
            }
        } else if entry_type.is_hard_link() {
            let link_name = entry
                .link_name()
                .map_err(|e| FetchError::ExtractionFailed {
                    reason: e.to_string(),
                })?;
            if let Some(link_target) = link_name {
                let source_path = resolve_tar_link_target(&link_target, dest_dir, strip_prefix);
                std::fs::hard_link(&source_path, &dest_path)
                    .or_else(|_| std::fs::copy(&source_path, &dest_path).map(|_| ()))
                    .map_err(|e| FetchError::ExtractionFailed {
                        reason: format!(
                            "Failed to materialize hard link {:?} -> {:?}: {}",
                            dest_path, source_path, e
                        ),
                    })?;
            }
        }
    }

    Ok(())
}

fn resolve_tar_link_target(
    link_target: &Path,
    dest_dir: &Path,
    strip_prefix: Option<&str>,
) -> PathBuf {
    let target_str = link_target.to_string_lossy();
    if let Some(prefix) = strip_prefix {
        if let Some(stripped) = target_str.strip_prefix(prefix) {
            return dest_dir.join(stripped.trim_start_matches('/'));
        }
        let prefix_with_slash = format!("{}/", prefix.trim_end_matches('/'));
        if let Some(stripped) = target_str.strip_prefix(&prefix_with_slash) {
            return dest_dir.join(stripped);
        }
    }
    dest_dir.join(link_target)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::TempDir;

    use super::*;

    fn create_test_tar_gz(strip_prefix: Option<&str>) -> Vec<u8> {
        use flate2::Compression;
        use flate2::write::GzEncoder;

        let mut builder = tar::Builder::new(Vec::new());

        let prefix = strip_prefix.unwrap_or("");
        let prefix_path = if prefix.is_empty() {
            String::new()
        } else {
            format!("{}/", prefix)
        };

        // Add a file
        let content = b"Hello, World!";
        let mut header = tar::Header::new_gnu();
        header.set_path(format!("{}test.txt", prefix_path)).unwrap();
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append(&header, &content[..]).unwrap();

        // Add a directory
        let mut header = tar::Header::new_gnu();
        header.set_path(format!("{}subdir/", prefix_path)).unwrap();
        header.set_entry_type(tar::EntryType::Directory);
        header.set_size(0);
        header.set_mode(0o755);
        header.set_cksum();
        builder.append(&header, &[][..]).unwrap();

        // Add a file in the subdirectory
        let content = b"Nested content";
        let mut header = tar::Header::new_gnu();
        header
            .set_path(format!("{}subdir/nested.txt", prefix_path))
            .unwrap();
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append(&header, &content[..]).unwrap();

        let tar_data = builder.into_inner().unwrap();

        // Compress with gzip
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&tar_data).unwrap();
        encoder.finish().unwrap()
    }

    fn create_hard_link_tar_gz(strip_prefix: Option<&str>) -> Vec<u8> {
        use flate2::Compression;
        use flate2::write::GzEncoder;

        let mut builder = tar::Builder::new(Vec::new());
        let prefix = strip_prefix.unwrap_or("");
        let prefix = if prefix.is_empty() {
            String::new()
        } else {
            format!("{prefix}/")
        };

        let content = b"multicall";
        let original = format!("{prefix}bin/tool.exe");
        let link = format!("{prefix}bin/tool-alias.exe");
        let mut header = tar::Header::new_gnu();
        header.set_path(&original).unwrap();
        header.set_size(content.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        builder.append(&header, &content[..]).unwrap();

        let mut link_header = tar::Header::new_gnu();
        link_header.set_entry_type(tar::EntryType::Link);
        link_header.set_path(&link).unwrap();
        link_header.set_link_name(&original).unwrap();
        link_header.set_size(0);
        link_header.set_mode(0o755);
        link_header.set_cksum();
        builder.append(&link_header, std::io::empty()).unwrap();

        let tar_data = builder.into_inner().unwrap();
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&tar_data).unwrap();
        encoder.finish().unwrap()
    }

    #[test]
    fn test_extract_tar_gz_no_strip() {
        let temp_dir = TempDir::new().unwrap();

        let data = create_test_tar_gz(None);
        let dest = temp_dir.path().join("extracted");
        std::fs::create_dir(&dest).unwrap();

        // Use the standalone extraction function directly
        extract_tar_gz_impl(&data, &dest, None).unwrap();

        assert!(dest.join("test.txt").exists());
        assert!(dest.join("subdir/nested.txt").exists());

        let content = std::fs::read_to_string(dest.join("test.txt")).unwrap();
        assert_eq!(content, "Hello, World!");
    }

    #[test]
    fn test_extract_tar_gz_with_strip_prefix() {
        let temp_dir = TempDir::new().unwrap();

        let data = create_test_tar_gz(Some("rules_cc-0.0.9"));
        let dest = temp_dir.path().join("extracted");
        std::fs::create_dir(&dest).unwrap();

        // Use the standalone extraction function directly
        extract_tar_gz_impl(&data, &dest, Some("rules_cc-0.0.9")).unwrap();

        assert!(dest.join("test.txt").exists());
        assert!(dest.join("subdir/nested.txt").exists());

        let content = std::fs::read_to_string(dest.join("test.txt")).unwrap();
        assert_eq!(content, "Hello, World!");
    }

    #[test]
    fn test_extract_tar_gz_materializes_hard_links() {
        let temp_dir = TempDir::new().unwrap();

        let data = create_hard_link_tar_gz(Some("toolchain"));
        let dest = temp_dir.path().join("extracted");
        std::fs::create_dir(&dest).unwrap();

        extract_tar_gz_impl(&data, &dest, Some("toolchain")).unwrap();

        assert_eq!(
            std::fs::read(dest.join("bin/tool.exe")).unwrap(),
            b"multicall"
        );
        assert_eq!(
            std::fs::read(dest.join("bin/tool-alias.exe")).unwrap(),
            b"multicall"
        );
    }

    #[test]
    fn git_apply_patch_tool_applies_registry_patch_shape() {
        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("a.txt");
        std::fs::write(&file, "old\n").unwrap();

        let patch = b"diff --git a/a.txt b/a.txt\n\
--- a/a.txt\n\
+++ b/a.txt\n\
@@ -1 +1 @@\n\
-old\n\
+new\n";
        let args = [
            "apply".to_owned(),
            "-p1".to_owned(),
            "--unsafe-paths".to_owned(),
            "--whitespace=nowarn".to_owned(),
        ];

        run_patch_tool("git", &args, Some(temp_dir.path()), patch).unwrap();

        assert_eq!(
            std::fs::read_to_string(file).unwrap().replace("\r\n", "\n"),
            "new\n"
        );
    }

    #[test]
    fn patch_files_in_apply_order_preserves_source_json_order() {
        let mut patches = crate::registry::RegistryFileMap::new();
        patches.insert("module_dot_bazel_version.patch".to_owned(), String::new());
        patches.insert("MODULE.bazel.patch".to_owned(), String::new());
        patches.insert("0001-Add-MODULE.bazel.patch".to_owned(), String::new());
        patches.insert(
            "0002-Add-utf8_range-dependency.patch".to_owned(),
            String::new(),
        );

        let ordered = patch_files_in_apply_order(&patches)
            .into_iter()
            .map(|(patch, _)| patch.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            ordered,
            [
                "module_dot_bazel_version.patch",
                "MODULE.bazel.patch",
                "0001-Add-MODULE.bazel.patch",
                "0002-Add-utf8_range-dependency.patch"
            ]
        );
    }

    #[test]
    fn apply_patch_content_accepts_already_applied_patch_with_changed_context() {
        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("MODULE.bazel");
        std::fs::write(
            &file,
            concat!(
                "module(\n",
                "    name = \"rules_proto\",\n",
                "    version = \"7.1.0\",\n",
                ")\n",
                "\n",
                "bazel_dep(name = \"protobuf\", version = \"29.1\", repo_name = \"com_google_protobuf\")\n",
            ),
        )
        .unwrap();

        let patch = b"diff --git a/MODULE.bazel b/MODULE.bazel\n\
--- a/MODULE.bazel\n\
+++ b/MODULE.bazel\n\
@@ -1 +1 @@\n\
-    version = \"0.0.0\",\n\
+    version = \"7.1.0\",\n\
 bazel_dep(name = \"protobuf\", version = \"27.1\", repo_name = \"com_google_protobuf\")\n";

        assert_eq!(
            patch_file_path("b/MODULE.bazel", 1).as_deref(),
            Some("MODULE.bazel")
        );
        let content = std::fs::read_to_string(&file)
            .unwrap()
            .replace("\r\n", "\n");
        assert!(content.contains("    version = \"7.1.0\","));
        assert!(!content.contains("    version = \"0.0.0\","));
        assert!(patch_already_applied(temp_dir.path(), 1, patch));
        SourceFetcher::apply_patch_content(temp_dir.path(), "already.patch", 1, patch).unwrap();

        let content = std::fs::read_to_string(file).unwrap().replace("\r\n", "\n");
        assert!(content.contains("version = \"7.1.0\""));
        assert!(content.contains("protobuf\", version = \"29.1\""));
    }

    #[test]
    fn single_version_module_patch_skips_non_module_hunks() {
        let temp_dir = TempDir::new().unwrap();
        let patch = temp_dir.path().join("fix.patch");
        std::fs::write(
            &patch,
            concat!(
                "diff --git a/MODULE.bazel b/MODULE.bazel\n",
                "--- a/MODULE.bazel\n",
                "+++ b/MODULE.bazel\n",
                "@@ -1 +1,2 @@\n",
                " module(name = \"dep\", version = \"1.0.0\")\n",
                "+bazel_dep(name = \"extra\", version = \"1.0.0\")\n",
                "diff --git a/BUILD.bazel b/BUILD.bazel\n",
                "--- a/BUILD.bazel\n",
                "+++ b/BUILD.bazel\n",
                "@@ -1 +1,2 @@\n",
                " filegroup(name = \"ok\", srcs = [])\n",
                "+filegroup(name = \"patched\", srcs = [])\n",
            ),
        )
        .unwrap();

        let patched = SourceFetcher::apply_single_version_module_patches(
            "module(name = \"dep\", version = \"1.0.0\")\n",
            temp_dir.path(),
            None,
            &["//:fix.patch".to_owned()],
            1,
        )
        .unwrap();

        assert!(patched.contains("bazel_dep(name = \"extra\""));
        assert!(!patched.contains("filegroup"));
    }
}
