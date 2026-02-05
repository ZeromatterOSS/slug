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

use std::io::Cursor;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use flate2::read::GzDecoder;
use kuro_error::BuckErrorContext;
use kuro_http::HttpClient;
use kuro_http::HttpClientBuilder;
use kuro_http::to_bytes;
use tar::Archive;
use zip::ZipArchive;

use crate::cache::ModuleCache;
use crate::integrity::verify_integrity;
use crate::registry::SourceInfo;

/// Errors that can occur during source fetching.
#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Input)]
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
    pub async fn new(cache: ModuleCache) -> kuro_error::Result<Self> {
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

    /// Fetch and extract source for a module.
    ///
    /// Returns the path to the extracted source directory.
    pub async fn fetch_source(
        &self,
        registry_url: &str,
        name: &str,
        version: &str,
        source_info: &SourceInfo,
    ) -> kuro_error::Result<PathBuf> {
        // Check if already fetched
        if self.cache.is_source_complete(registry_url, name, version) {
            tracing::debug!("Using cached source for {}@{}", name, version);
            return Ok(self.cache.source_dir(registry_url, name, version));
        }

        let dest_dir = self.cache.create_source_dir(registry_url, name, version)?;

        if source_info.is_git() {
            self.fetch_git(source_info, &dest_dir).await?;
        } else {
            self.fetch_archive(name, version, source_info, &dest_dir)
                .await?;
        }

        // Apply patches if any
        if !source_info.patches.is_empty() {
            self.apply_patches(&dest_dir, source_info).await?;
        }

        // Mark as complete
        self.cache
            .mark_source_complete(registry_url, name, version)?;

        Ok(dest_dir)
    }

    /// Fetch and extract an archive source.
    async fn fetch_archive(
        &self,
        name: &str,
        version: &str,
        source_info: &SourceInfo,
        dest_dir: &Path,
    ) -> kuro_error::Result<()> {
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
    async fn download_archive(&self, url: &str) -> kuro_error::Result<Vec<u8>> {
        tracing::info!("Downloading archive from {}", url);

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
    ) -> kuro_error::Result<()> {
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

        // Try zip
        if let Ok(()) = self.extract_zip(data, dest_dir, strip_prefix) {
            return Ok(());
        }

        Err(FetchError::ExtractionFailed {
            reason: "Unknown archive format".to_string(),
        }
        .into())
    }

    /// Extract a gzip-compressed tar archive.
    fn extract_tar_gz(
        &self,
        data: &[u8],
        dest_dir: &Path,
        strip_prefix: Option<&str>,
    ) -> kuro_error::Result<()> {
        extract_tar_gz_impl(data, dest_dir, strip_prefix)
    }

    /// Extract a plain tar archive (not implemented, placeholder).
    fn extract_tar(
        &self,
        _data: &[u8],
        _dest_dir: &Path,
        _strip_prefix: Option<&str>,
    ) -> kuro_error::Result<()> {
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
    ) -> kuro_error::Result<()> {
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
    async fn fetch_git(&self, source_info: &SourceInfo, dest_dir: &Path) -> kuro_error::Result<()> {
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
    ) -> kuro_error::Result<()> {
        if source_info.patches.is_empty() {
            return Ok(());
        }

        tracing::debug!(
            "Applying {} patches to {:?}",
            source_info.patches.len(),
            dest_dir
        );

        for (patch_file, _integrity) in &source_info.patches {
            // TODO: Download patch file from registry and apply
            // For now, just log a warning
            tracing::warn!(
                "Patch application not yet implemented: {} (strip={})",
                patch_file,
                source_info.patch_strip
            );
        }

        Ok(())
    }
}

/// Extract a gzip-compressed tar archive (standalone function for testing).
fn extract_tar_gz_impl(
    data: &[u8],
    dest_dir: &Path,
    strip_prefix: Option<&str>,
) -> kuro_error::Result<()> {
    let decoder = GzDecoder::new(data);
    let mut archive = Archive::new(decoder);

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
                }
            }
        }
    }

    Ok(())
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
}
