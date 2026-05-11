/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Repository rule executor for built-in repository rules.
//!
//! This module implements the actual execution logic for common repository rules
//! like `http_archive`, `git_repository`, and `local_repository`. Rather than
//! invoking Starlark, we implement these natively for better performance and
//! integration with the existing download infrastructure.
//!
//! ## Supported Rules
//!
//! - `http_archive` - Download and extract archive from URL(s)
//! - `git_repository` - Clone a git repository at a specific commit
//! - `local_repository` - Symlink to a local directory
//! - `new_local_repository` - Create a repository from a local directory with custom BUILD

use std::io::Cursor;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use flate2::read::GzDecoder;
use sha2::Digest;
use sha2::Sha256;
use tar::Archive;
use zip::ZipArchive;

use crate::repository_execution::InvocationAttrs;
use crate::repository_execution::RepositoryExecutionError;
use crate::repository_execution::RepositoryRuleResult;
use crate::repository_invocations::RepositoryInvocation;

/// Execute a repository rule invocation.
///
/// This function dispatches to the appropriate handler based on the rule name.
pub fn execute_repository_rule(
    invocation: &RepositoryInvocation,
    project_root: &Path,
) -> kuro_error::Result<RepositoryRuleResult> {
    let attrs = InvocationAttrs::new(invocation);
    let working_dir = project_root.join("bazel-external").join(&invocation.name);

    tracing::info!(
        "Executing repository rule '{}' for '{}' at {:?}",
        invocation.rule_name,
        invocation.name,
        working_dir
    );

    // Check if already materialized
    if is_repo_complete(&working_dir) {
        tracing::debug!("Repository '{}' already materialized", invocation.name);
        return Ok(RepositoryRuleResult::success(
            invocation.name.clone(),
            working_dir,
        ));
    }

    // Clean and create working directory
    prepare_working_dir(&working_dir)?;

    // Dispatch based on rule name
    let result = match invocation.rule_name.as_str() {
        "http_archive" => execute_http_archive(invocation, &attrs, &working_dir),
        "http_file" => execute_http_file(invocation, &attrs, &working_dir),
        "http_jar" => execute_http_jar(invocation, &attrs, &working_dir),
        "git_repository" => execute_git_repository(invocation, &attrs, &working_dir),
        "local_repository" | "new_local_repository" => {
            execute_local_repository(invocation, &attrs, &working_dir)
        }
        rule_name => {
            // For unknown rules, create a minimal stub
            tracing::warn!(
                "Unknown repository rule '{}', creating stub repository",
                rule_name
            );
            create_stub_repository(invocation, &working_dir)
        }
    };

    match result {
        Ok(()) => {
            mark_repo_complete(&working_dir)?;
            Ok(RepositoryRuleResult::success(
                invocation.name.clone(),
                working_dir,
            ))
        }
        Err(e) => {
            // Clean up on failure
            let _ = std::fs::remove_dir_all(&working_dir);
            Err(e)
        }
    }
}

/// Check if a repository is already materialized.
fn is_repo_complete(working_dir: &Path) -> bool {
    working_dir.join(".kuro_repo_complete").exists()
}

/// Mark a repository as complete.
fn mark_repo_complete(working_dir: &Path) -> kuro_error::Result<()> {
    std::fs::write(working_dir.join(".kuro_repo_complete"), "complete").map_err(|e| {
        RepositoryExecutionError::WorkingDirFailed {
            reason: format!("Failed to write completion marker: {}", e),
        }
    })?;
    Ok(())
}

/// Prepare the working directory.
fn prepare_working_dir(working_dir: &Path) -> kuro_error::Result<()> {
    // Remove existing directory if present
    if working_dir.exists() {
        std::fs::remove_dir_all(working_dir).map_err(|e| {
            RepositoryExecutionError::WorkingDirFailed {
                reason: format!("Failed to clean existing directory: {}", e),
            }
        })?;
    }

    // Create fresh directory
    std::fs::create_dir_all(working_dir).map_err(|e| {
        RepositoryExecutionError::WorkingDirFailed {
            reason: format!("Failed to create directory: {}", e),
        }
    })?;

    Ok(())
}

/// Execute http_archive repository rule.
fn execute_http_archive(
    invocation: &RepositoryInvocation,
    attrs: &InvocationAttrs,
    working_dir: &Path,
) -> kuro_error::Result<()> {
    // Get URLs - can be `url` (single) or `urls` (list)
    let urls = get_urls(attrs)?;
    if urls.is_empty() {
        return Err(RepositoryExecutionError::MissingAttribute {
            name: invocation.name.clone(),
            attr: "url or urls".to_owned(),
        }
        .into());
    }

    // Get integrity verification
    let sha256 = attrs.get_optional_string("sha256");
    let integrity = attrs.get_optional_string("integrity");
    let strip_prefix = attrs.get_optional_string("strip_prefix");

    // Try each URL until one succeeds
    let mut last_error = None;
    for url in &urls {
        match download_and_extract(url, working_dir, sha256, integrity, strip_prefix) {
            Ok(()) => {
                // Create BUILD.bazel if build_file or build_file_content is specified
                if let Some(content) = attrs.get_optional_string("build_file_content") {
                    std::fs::write(working_dir.join("BUILD.bazel"), content).map_err(|e| {
                        RepositoryExecutionError::ExecutionFailed {
                            name: invocation.name.clone(),
                            reason: format!("Failed to write BUILD.bazel: {}", e),
                        }
                    })?;
                } else if let Some(build_file) = attrs.get_optional_string("build_file") {
                    // build_file is a label like "@@repo//path:BUILD.foo" or a file path
                    let build_file_path = resolve_build_file_label(build_file, working_dir);
                    let content = std::fs::read_to_string(&build_file_path).map_err(|e| {
                        RepositoryExecutionError::ExecutionFailed {
                            name: invocation.name.clone(),
                            reason: format!(
                                "Could not read build_file '{}' for repository '{}' at '{}': {}",
                                build_file, invocation.name, build_file_path, e
                            ),
                        }
                    })?;
                    std::fs::write(working_dir.join("BUILD.bazel"), content).map_err(|e| {
                        RepositoryExecutionError::ExecutionFailed {
                            name: invocation.name.clone(),
                            reason: format!("Failed to write BUILD.bazel from build_file: {}", e),
                        }
                    })?;
                }

                // Apply patches if specified
                apply_patches(invocation, attrs, working_dir)?;

                // Create WORKSPACE if not present
                if !working_dir.join("WORKSPACE").exists()
                    && !working_dir.join("WORKSPACE.bazel").exists()
                {
                    std::fs::write(
                        working_dir.join("WORKSPACE.bazel"),
                        format!("workspace(name = \"{}\")\n", invocation.name),
                    )
                    .map_err(|e| {
                        RepositoryExecutionError::ExecutionFailed {
                            name: invocation.name.clone(),
                            reason: format!("Failed to write WORKSPACE.bazel: {}", e),
                        }
                    })?;
                }

                return Ok(());
            }
            Err(e) => {
                tracing::warn!("Failed to download from {}: {}", url, e);
                last_error = Some(e);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        RepositoryExecutionError::ExecutionFailed {
            name: invocation.name.clone(),
            reason: "All download URLs failed".to_owned(),
        }
        .into()
    }))
}

/// Apply patches to a repository after extraction.
///
/// Supports:
/// - `patches`: list of patch file paths to apply
/// - `patch_args`: arguments to pass to `patch` command (default: ["-p1"])
/// - `patch_cmds`: shell commands to run after patching
fn apply_patches(
    invocation: &RepositoryInvocation,
    attrs: &InvocationAttrs,
    working_dir: &Path,
) -> kuro_error::Result<()> {
    // Apply patch files
    if let Some(patches) = attrs.get_string_list("patches") {
        let default_patch_args = ["-p1".to_owned()];
        let patch_args = attrs
            .get_string_list("patch_args")
            .unwrap_or(&default_patch_args);

        for patch_path in patches {
            tracing::info!(
                "Applying patch '{}' to repository '{}'",
                patch_path,
                invocation.name
            );

            let resolved_patch_path = resolve_build_file_label(patch_path, working_dir);
            if let Err(e) =
                apply_patch_file(Path::new(&resolved_patch_path), patch_args, working_dir)
            {
                tracing::warn!("Patch '{}' failed (non-fatal): {}", patch_path, e);
            }
        }
    }

    // Run patch commands
    if let Some(patch_cmds) = attrs.get_string_list("patch_cmds") {
        for cmd_str in patch_cmds {
            tracing::info!("Running patch_cmd for '{}': {}", invocation.name, cmd_str);

            let shell = if cfg!(windows) { "cmd" } else { "sh" };
            let flag = if cfg!(windows) { "/c" } else { "-c" };

            let output = Command::new(shell)
                .arg(flag)
                .arg(cmd_str)
                .current_dir(working_dir)
                .output()
                .map_err(|e| RepositoryExecutionError::ExecutionFailed {
                    name: invocation.name.clone(),
                    reason: format!("Failed to run patch_cmd '{}': {}", cmd_str, e),
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                tracing::warn!("patch_cmd '{}' failed (non-fatal): {}", cmd_str, stderr);
            }
        }
    }

    Ok(())
}

fn apply_patch_file(
    patch_path: &Path,
    patch_args: &[String],
    working_dir: &Path,
) -> Result<(), String> {
    let mut cmd = Command::new("patch");
    for arg in patch_args {
        cmd.arg(arg);
    }
    match cmd
        .arg("-i")
        .arg(patch_path)
        .current_dir(working_dir)
        .output()
    {
        Ok(output) if output.status.success() => return Ok(()),
        Ok(output) => {
            return Err(String::from_utf8_lossy(&output.stderr).into_owned());
        }
        Err(e) if e.kind() != ErrorKind::NotFound => {
            return Err(format!("Failed to run patch command: {e}"));
        }
        Err(_) => {}
    }

    let strip_arg = git_apply_strip_arg(patch_args);
    let output = Command::new("git")
        .args(["apply", "--unsafe-paths", "--whitespace=nowarn", &strip_arg])
        .arg(patch_path)
        .current_dir(working_dir)
        .output()
        .map_err(|e| format!("Failed to run patch fallback via git apply: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{}{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        ))
    }
}

fn git_apply_strip_arg(patch_args: &[String]) -> String {
    let mut iter = patch_args.iter();
    while let Some(arg) = iter.next() {
        if let Some(strip) = arg.strip_prefix("-p") {
            if !strip.is_empty() {
                return format!("-p{strip}");
            }
            if let Some(strip) = iter.next() {
                return format!("-p{strip}");
            }
        }
    }
    "-p1".to_owned()
}

/// Execute http_file repository rule.
///
/// Downloads a single file and makes it available as a target.
/// Creates a BUILD.bazel that exposes the file via `filegroup`.
fn execute_http_file(
    invocation: &RepositoryInvocation,
    attrs: &InvocationAttrs,
    working_dir: &Path,
) -> kuro_error::Result<()> {
    let urls = get_urls(attrs)?;
    if urls.is_empty() {
        return Err(RepositoryExecutionError::MissingAttribute {
            name: invocation.name.clone(),
            attr: "url or urls".to_owned(),
        }
        .into());
    }

    let sha256 = attrs.get_optional_string("sha256");
    let integrity = attrs.get_optional_string("integrity");
    let downloaded_file_path = attrs
        .get_optional_string("downloaded_file_path")
        .unwrap_or("downloaded");

    // Download the file
    let mut last_error = None;
    let mut data = None;
    for url in &urls {
        match download_url(url) {
            Ok(d) => {
                data = Some(d);
                break;
            }
            Err(e) => {
                tracing::warn!("Failed to download from {}: {}", url, e);
                last_error = Some(e);
            }
        }
    }

    let file_data = data.ok_or_else(|| {
        last_error.unwrap_or_else(|| {
            RepositoryExecutionError::ExecutionFailed {
                name: invocation.name.clone(),
                reason: "All download URLs failed".to_owned(),
            }
            .into()
        })
    })?;

    // Verify integrity
    if let Some(expected) = sha256.as_deref() {
        verify_sha256(&file_data, expected)?;
    }
    if let Some(expected) = integrity.as_deref() {
        verify_integrity(&file_data, expected)?;
    }

    // Write the file. Bazel's http_file places the downloaded file in a
    // "file/" subdirectory so Label("@repo//file:downloaded") resolves correctly.
    let file_dir = working_dir.join("file");
    std::fs::create_dir_all(&file_dir).ok();
    let dest_path = file_dir.join(downloaded_file_path);
    std::fs::write(&dest_path, &file_data).map_err(|e| {
        RepositoryExecutionError::ExecutionFailed {
            name: invocation.name.clone(),
            reason: format!("Failed to write downloaded file: {}", e),
        }
    })?;

    // Set executable if requested
    #[cfg(unix)]
    {
        let executable = attrs.get_bool("executable", false);
        if executable {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&dest_path, std::fs::Permissions::from_mode(0o755));
        }
    }

    // Create root BUILD.bazel (empty package)
    std::fs::write(working_dir.join("BUILD.bazel"), "").ok();

    // Create file/BUILD.bazel (Bazel http_file convention)
    let file_build = format!(
        r#"package(default_visibility = ["//visibility:public"])

exports_files(["{}"])
"#,
        downloaded_file_path
    );
    std::fs::write(file_dir.join("BUILD.bazel"), file_build).ok();

    Ok(())
}

/// Execute http_jar repository rule.
///
/// Downloads a JAR file and makes it available as a java_import target.
/// Falls back to filegroup if java rules not available.
fn execute_http_jar(
    invocation: &RepositoryInvocation,
    attrs: &InvocationAttrs,
    working_dir: &Path,
) -> kuro_error::Result<()> {
    let urls = get_urls(attrs)?;
    if urls.is_empty() {
        return Err(RepositoryExecutionError::MissingAttribute {
            name: invocation.name.clone(),
            attr: "url or urls".to_owned(),
        }
        .into());
    }

    let sha256 = attrs.get_optional_string("sha256");
    let integrity = attrs.get_optional_string("integrity");

    // Download the jar
    let mut last_error = None;
    let mut data = None;
    for url in &urls {
        match download_url(url) {
            Ok(d) => {
                data = Some(d);
                break;
            }
            Err(e) => {
                tracing::warn!("Failed to download from {}: {}", url, e);
                last_error = Some(e);
            }
        }
    }

    let jar_data = data.ok_or_else(|| {
        last_error.unwrap_or_else(|| {
            RepositoryExecutionError::ExecutionFailed {
                name: invocation.name.clone(),
                reason: "All download URLs failed".to_owned(),
            }
            .into()
        })
    })?;

    // Verify integrity
    if let Some(expected) = sha256.as_deref() {
        verify_sha256(&jar_data, expected)?;
    }
    if let Some(expected) = integrity.as_deref() {
        verify_integrity(&jar_data, expected)?;
    }

    // Write the jar file
    let jar_filename = format!("{}.jar", invocation.name);
    std::fs::write(working_dir.join(&jar_filename), &jar_data).map_err(|e| {
        RepositoryExecutionError::ExecutionFailed {
            name: invocation.name.clone(),
            reason: format!("Failed to write jar file: {}", e),
        }
    })?;

    // Create BUILD.bazel with filegroup (since java_import requires rules_java)
    let build_content = format!(
        r#"package(default_visibility = ["//visibility:public"])

filegroup(
    name = "jar",
    srcs = ["{}"],
)

exports_files(["{}"])
"#,
        jar_filename, jar_filename
    );
    std::fs::write(working_dir.join("BUILD.bazel"), build_content).ok();

    Ok(())
}

/// Get URLs from attributes (handles both `url` and `urls`).
fn get_urls(attrs: &InvocationAttrs) -> kuro_error::Result<Vec<String>> {
    let mut urls = Vec::new();

    // Check `url` attribute first
    if let Some(url) = attrs.get_optional_string("url") {
        urls.push(url.to_owned());
    }

    // Check `urls` attribute
    if let Some(url_list) = attrs.get_string_list("urls") {
        urls.extend(url_list.iter().cloned());
    }

    Ok(urls)
}

/// Resolve a repository-rule file attribute that may be a Bazel label.
///
/// `build_file` and `patches` are executed while materializing
/// `{project_root}/bazel-external/{repo}`. Keep the bzlmod-side resolver small:
/// semantic parsing goes through `repo_mapping`, the normal path is exact, and
/// the old `bazel-external` directory scan remains only as an explicit fallback
/// for legacy module-version directories that have not been registered here.
fn resolve_build_file_label(label: &str, working_dir: &Path) -> String {
    let Some(parsed) =
        crate::repo_mapping::canonicalize_label_with_package_context(label, "", "", None)
    else {
        return label.to_owned();
    };

    let project_root = working_dir
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf);

    let Some(project_root) = project_root else {
        return label_to_relative_fragment(&parsed)
            .to_string_lossy()
            .to_string();
    };

    let repo = repository_executor_repo_dir_name(parsed.repo().as_str());
    let normal_path = if repo.is_empty() {
        label_path_under(&project_root, &parsed)
    } else {
        label_path_under(project_root.join("bazel-external").join(&repo), &parsed)
    };

    if normal_path.exists() {
        return normal_path.to_string_lossy().to_string();
    }

    let Some(fallback_path) =
        scan_bazel_external_for_repository_executor(&project_root, &repo, &parsed)
    else {
        return normal_path.to_string_lossy().to_string();
    };

    fallback_path.to_string_lossy().to_string()
}

fn repository_executor_repo_dir_name(repo: &str) -> String {
    if repo.starts_with('+') {
        format!("_main{}", repo)
    } else {
        repo.to_owned()
    }
}

fn label_path_under(
    base: impl Into<PathBuf>,
    label: &crate::repo_mapping::CanonicalLabel,
) -> PathBuf {
    let mut path = base.into();
    if !label.package().is_empty() {
        path.push(label.package());
    }
    path.push(label.target());
    path
}

fn label_to_relative_fragment(label: &crate::repo_mapping::CanonicalLabel) -> PathBuf {
    label_path_under(label.repo().as_str(), label)
}

fn scan_bazel_external_for_repository_executor(
    project_root: &Path,
    repo: &str,
    label: &crate::repo_mapping::CanonicalLabel,
) -> Option<PathBuf> {
    if repo.is_empty() {
        return None;
    }

    let bazel_external = project_root.join("bazel-external");
    if !bazel_external.exists() {
        return None;
    }

    tracing::debug!(
        repo,
        "Falling back to bazel-external directory scanning for repository executor label"
    );

    let entries = std::fs::read_dir(&bazel_external).ok()?;
    let repo_prefix = format!("{}+", repo);
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.as_ref() != repo && !name.starts_with(&repo_prefix) {
            continue;
        }

        let path = label_path_under(entry.path(), label);
        if path.exists() {
            return Some(path);
        }
    }

    None
}

/// Download and extract an archive.
fn download_and_extract(
    url: &str,
    dest_dir: &Path,
    sha256: Option<&str>,
    integrity: Option<&str>,
    strip_prefix: Option<&str>,
) -> kuro_error::Result<()> {
    tracing::info!("Downloading from {}", url);

    // Download using curl or wget
    let data = download_url(url)?;

    // Verify integrity
    if let Some(expected) = sha256 {
        verify_sha256(&data, expected)?;
    }
    if let Some(expected) = integrity {
        verify_integrity(&data, expected)?;
    }

    // Extract
    extract_archive(&data, dest_dir, strip_prefix)?;

    Ok(())
}

/// Download a URL using curl or wget.
fn download_url(url: &str) -> kuro_error::Result<Vec<u8>> {
    // Timeouts: split connect and total. Stuck TCP connects get ~30s before
    // skipping to the next URL in the caller's fallback list. Stalled
    // in-flight transfers get up to 60s wall-time total. Previously this
    // function allowed `--max-time 300` per URL with no `--connect-timeout`
    // and then tried wget as a fallback on the same URL — a single
    // unreachable mirror (e.g. gmplib.org intermittent outage) blocked the
    // daemon thread for 5+5 minutes before the next URL in the caller's
    // urls[] list was tried. On slow-but-live mirrors, the caller's next
    // URL is typically faster; favour falling through quickly.
    // See Plan 10 Phase 7 diagnostic findings.
    const CONNECT_TIMEOUT_SECS: &str = "30";
    const TOTAL_TIMEOUT_SECS: &str = "60";

    // Try curl first. On Windows, use curl.exe to avoid PowerShell alias.
    let curl_cmd = if cfg!(windows) { "curl.exe" } else { "curl" };
    let output = Command::new(curl_cmd)
        .args([
            "-fsSL",
            "--connect-timeout",
            CONNECT_TIMEOUT_SECS,
            "--max-time",
            TOTAL_TIMEOUT_SECS,
            url,
        ])
        .output();

    match output {
        Ok(output) if output.status.success() => return Ok(output.stdout),
        Ok(output) => {
            // curl ran but the URL failed. Common causes at this point are
            // HTTP errors (4xx/5xx) or timeouts. wget is unlikely to
            // recover from HTTP errors, and if the failure was a timeout,
            // wget will time out on the same URL for the same duration.
            // Skip wget; surface the error and let the caller try the next
            // URL in its fallback list.
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::debug!("curl failed for {}: {}", url, stderr);
            return Err(RepositoryExecutionError::ExecutionFailed {
                name: url.to_owned(),
                reason: format!("Download failed: {}", stderr),
            }
            .into());
        }
        Err(e) => {
            tracing::debug!("curl not available: {}", e);
        }
    }

    // curl not found - try wget as the primary tool.
    let output = Command::new("wget")
        .args([
            "-q",
            "-O",
            "-",
            "--connect-timeout",
            CONNECT_TIMEOUT_SECS,
            "--timeout",
            TOTAL_TIMEOUT_SECS,
            url,
        ])
        .output()
        .map_err(|e| RepositoryExecutionError::ExecutionFailed {
            name: url.to_owned(),
            reason: format!("Neither curl nor wget available: {}", e),
        })?;

    if output.status.success() {
        Ok(output.stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(RepositoryExecutionError::ExecutionFailed {
            name: url.to_owned(),
            reason: format!("Download failed: {}", stderr),
        }
        .into())
    }
}

/// Verify SHA256 hash.
fn verify_sha256(data: &[u8], expected: &str) -> kuro_error::Result<()> {
    let hash = Sha256::digest(data);
    let computed = hex::encode(hash);

    if computed.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(RepositoryExecutionError::ExecutionFailed {
            name: "integrity".to_owned(),
            reason: format!("SHA256 mismatch: expected {}, got {}", expected, computed),
        }
        .into())
    }
}

/// Verify SRI integrity hash.
fn verify_integrity(data: &[u8], expected: &str) -> kuro_error::Result<()> {
    use base64::Engine;

    let (algo, hash) =
        expected
            .split_once('-')
            .ok_or_else(|| RepositoryExecutionError::ExecutionFailed {
                name: "integrity".to_owned(),
                reason: format!("Invalid integrity format: {}", expected),
            })?;

    if algo != "sha256" {
        return Err(RepositoryExecutionError::ExecutionFailed {
            name: "integrity".to_owned(),
            reason: format!("Unsupported hash algorithm: {}", algo),
        }
        .into());
    }

    let expected_bytes = base64::engine::general_purpose::STANDARD
        .decode(hash)
        .map_err(|e| RepositoryExecutionError::ExecutionFailed {
            name: "integrity".to_owned(),
            reason: format!("Invalid base64: {}", e),
        })?;

    let computed = Sha256::digest(data);
    if computed.as_slice() == expected_bytes.as_slice() {
        Ok(())
    } else {
        Err(RepositoryExecutionError::ExecutionFailed {
            name: "integrity".to_owned(),
            reason: format!("Integrity mismatch"),
        }
        .into())
    }
}

/// Extract an archive, auto-detecting format.
fn extract_archive(
    data: &[u8],
    dest_dir: &Path,
    strip_prefix: Option<&str>,
) -> kuro_error::Result<()> {
    // Try tar.gz first
    if extract_tar_gz(data, dest_dir, strip_prefix).is_ok() {
        return Ok(());
    }

    // Try tar.xz
    if extract_tar_xz(data, dest_dir, strip_prefix).is_ok() {
        return Ok(());
    }

    // Try tar.zst
    if extract_tar_zst(data, dest_dir, strip_prefix).is_ok() {
        return Ok(());
    }

    // Try zip
    if extract_zip(data, dest_dir, strip_prefix).is_ok() {
        return Ok(());
    }

    Err(RepositoryExecutionError::ExecutionFailed {
        name: "extract".to_owned(),
        reason: format!(
            "Unknown archive format ({} bytes, starts with {:02x?})",
            data.len(),
            &data[..data.len().min(8)]
        ),
    }
    .into())
}

/// Extract a tar archive from any reader.
fn extract_tar_from_reader<R: std::io::Read>(
    reader: R,
    dest_dir: &Path,
    strip_prefix: Option<&str>,
) -> kuro_error::Result<()> {
    let mut archive = Archive::new(reader);

    for entry_result in
        archive
            .entries()
            .map_err(|e| RepositoryExecutionError::ExecutionFailed {
                name: "extract".to_owned(),
                reason: e.to_string(),
            })?
    {
        let mut entry = entry_result.map_err(|e| RepositoryExecutionError::ExecutionFailed {
            name: "extract".to_owned(),
            reason: e.to_string(),
        })?;

        let path = entry
            .path()
            .map_err(|e| RepositoryExecutionError::ExecutionFailed {
                name: "extract".to_owned(),
                reason: e.to_string(),
            })?;

        // Apply strip_prefix
        let dest_path = if let Some(prefix) = strip_prefix {
            let path_str = path.to_string_lossy();
            if let Some(stripped) = path_str.strip_prefix(prefix) {
                let stripped = stripped.trim_start_matches('/');
                if stripped.is_empty() {
                    continue;
                }
                dest_dir.join(stripped)
            } else if path_str.starts_with(prefix.trim_end_matches('/')) {
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
            std::fs::create_dir_all(parent).ok();
        }

        // Extract based on entry type
        let entry_type = entry.header().entry_type();
        if entry_type.is_dir() {
            std::fs::create_dir_all(&dest_path).ok();
        } else if entry_type.is_file() {
            let mut file = std::fs::File::create(&dest_path).map_err(|e| {
                RepositoryExecutionError::ExecutionFailed {
                    name: "extract".to_owned(),
                    reason: format!("Failed to create file: {}", e),
                }
            })?;
            std::io::copy(&mut entry, &mut file).ok();

            // Set permissions
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(mode) = entry.header().mode() {
                    let _ =
                        std::fs::set_permissions(&dest_path, std::fs::Permissions::from_mode(mode));
                }
            }
        } else if entry_type.is_symlink() {
            #[cfg(unix)]
            if let Ok(link_name) = entry.link_name() {
                if let Some(link_target) = link_name {
                    let _ = std::os::unix::fs::symlink(&*link_target, &dest_path);
                }
            }
        }
    }

    Ok(())
}

/// Extract tar.gz archive.
fn extract_tar_gz(
    data: &[u8],
    dest_dir: &Path,
    strip_prefix: Option<&str>,
) -> kuro_error::Result<()> {
    extract_tar_from_reader(GzDecoder::new(data), dest_dir, strip_prefix)
}

/// Extract tar.xz archive.
fn extract_tar_xz(
    data: &[u8],
    dest_dir: &Path,
    strip_prefix: Option<&str>,
) -> kuro_error::Result<()> {
    extract_tar_from_reader(xz2::read::XzDecoder::new(data), dest_dir, strip_prefix)
}

/// Extract tar.zst archive.
fn extract_tar_zst(
    data: &[u8],
    dest_dir: &Path,
    strip_prefix: Option<&str>,
) -> kuro_error::Result<()> {
    let decoder = zstd::stream::read::Decoder::new(data).map_err(|e| {
        RepositoryExecutionError::ExecutionFailed {
            name: "extract".to_owned(),
            reason: e.to_string(),
        }
    })?;
    extract_tar_from_reader(decoder, dest_dir, strip_prefix)
}

/// Extract zip archive.
fn extract_zip(data: &[u8], dest_dir: &Path, strip_prefix: Option<&str>) -> kuro_error::Result<()> {
    let cursor = Cursor::new(data);
    let mut archive =
        ZipArchive::new(cursor).map_err(|e| RepositoryExecutionError::ExecutionFailed {
            name: "extract".to_owned(),
            reason: e.to_string(),
        })?;

    for i in 0..archive.len() {
        let mut file =
            archive
                .by_index(i)
                .map_err(|e| RepositoryExecutionError::ExecutionFailed {
                    name: "extract".to_owned(),
                    reason: e.to_string(),
                })?;

        let file_path = match file.enclosed_name() {
            Some(path) => path.to_owned(),
            None => continue,
        };

        // Apply strip_prefix
        let dest_path = if let Some(prefix) = strip_prefix {
            let stripped = file_path.strip_prefix(prefix).unwrap_or(&file_path);
            dest_dir.join(stripped)
        } else {
            dest_dir.join(&file_path)
        };

        if dest_path == dest_dir {
            continue;
        }

        if file.is_dir() {
            std::fs::create_dir_all(&dest_path).ok();
        } else {
            if let Some(parent) = dest_path.parent() {
                std::fs::create_dir_all(parent).ok();
            }

            let mut outfile = std::fs::File::create(&dest_path).map_err(|e| {
                RepositoryExecutionError::ExecutionFailed {
                    name: "extract".to_owned(),
                    reason: format!("Failed to create file: {}", e),
                }
            })?;
            std::io::copy(&mut file, &mut outfile).ok();

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = file.unix_mode() {
                    let _ =
                        std::fs::set_permissions(&dest_path, std::fs::Permissions::from_mode(mode));
                }
            }
        }
    }

    Ok(())
}

/// Execute git_repository rule.
fn execute_git_repository(
    invocation: &RepositoryInvocation,
    attrs: &InvocationAttrs,
    working_dir: &Path,
) -> kuro_error::Result<()> {
    let remote = attrs.require_string("remote")?;
    let commit = attrs.get_optional_string("commit");
    let tag = attrs.get_optional_string("tag");
    let branch = attrs.get_optional_string("branch");

    // Determine what to checkout
    let checkout_ref = commit.or(tag).or(branch).unwrap_or("HEAD");

    tracing::info!("Cloning git repository {} at {}", remote, checkout_ref);

    // Initialize git repo
    run_git(working_dir, |c| {
        c.arg("init");
    })?;

    // Add remote
    run_git(working_dir, |c| {
        c.arg("remote").arg("add").arg("origin").arg(remote);
    })?;

    // Fetch
    run_git(working_dir, |c| {
        c.arg("fetch").arg("origin").arg(checkout_ref);
    })?;

    // Checkout
    run_git(working_dir, |c| {
        c.arg("reset").arg("--hard").arg("FETCH_HEAD");
    })?;

    // Plan 39: keep `.git`. Downstream rules — most prominently
    // rules_rs's `crate_git_repository` — use `git --git-dir=<>/.git
    // worktree add` to fan one master clone out into per-crate spokes,
    // and that fails if we strip the directory here.

    // Create WORKSPACE if not present
    if !working_dir.join("WORKSPACE").exists() && !working_dir.join("WORKSPACE.bazel").exists() {
        std::fs::write(
            working_dir.join("WORKSPACE.bazel"),
            format!("workspace(name = \"{}\")\n", invocation.name),
        )
        .ok();
    }

    Ok(())
}

/// Run a git command.
fn run_git(cwd: &Path, configure: impl FnOnce(&mut Command)) -> kuro_error::Result<()> {
    let mut cmd = Command::new("git");
    configure(&mut cmd);
    cmd.current_dir(cwd);

    let output = cmd
        .output()
        .map_err(|e| RepositoryExecutionError::ExecutionFailed {
            name: "git".to_owned(),
            reason: format!("Failed to run git: {}", e),
        })?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(RepositoryExecutionError::ExecutionFailed {
            name: "git".to_owned(),
            reason: format!("Git command failed: {}", stderr),
        }
        .into())
    }
}

/// Execute local_repository or new_local_repository rule.
fn execute_local_repository(
    invocation: &RepositoryInvocation,
    attrs: &InvocationAttrs,
    working_dir: &Path,
) -> kuro_error::Result<()> {
    let path = attrs.require_string("path")?;

    // Resolve relative paths against the project root (parent of bazel-external/).
    // In Bazel, relative paths in new_local_repository are resolved relative to
    // the workspace root (where MODULE.bazel lives).
    let resolved_path = if Path::new(path).is_relative() {
        if let Some(bazel_external) = working_dir.parent() {
            if let Some(project_root) = bazel_external.parent() {
                project_root.join(path)
            } else {
                PathBuf::from(path)
            }
        } else {
            PathBuf::from(path)
        }
    } else {
        PathBuf::from(path)
    };

    let resolved_path = resolved_path
        .canonicalize()
        .unwrap_or_else(|_| resolved_path.clone());

    if invocation.rule_name == "new_local_repository" {
        // For new_local_repository: create working dir with symlinks to individual
        // entries from the target, plus a custom BUILD.bazel. Don't symlink the
        // directory itself (that would write BUILD.bazel into the source tree).
        std::fs::create_dir_all(working_dir).ok();

        // Symlink all entries from the target directory.
        // When path points to an ancestor of the working dir (e.g. "../.."),
        // symlinking everything would recurse into ourselves. We don't have
        // a generic solution, but excluding known kuro output dirs covers the
        // common case (Bazel's llvm-raw pattern).
        if let Ok(entries) = std::fs::read_dir(&resolved_path) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                // Skip kuro/bazel output dirs to avoid recursive self-reference
                if matches!(
                    name_str.as_ref(),
                    "bazel-external"
                        | "bazel-out"
                        | "bazel-bin"
                        | "bazel-testlogs"
                        | "buck-out"
                        | ".kuro_repo_complete"
                ) {
                    continue;
                }
                let target = working_dir.join(&name);
                if !target.exists() {
                    #[cfg(unix)]
                    std::os::unix::fs::symlink(entry.path(), &target).ok();
                    #[cfg(not(unix))]
                    {
                        if entry.path().is_dir() {
                            copy_dir_recursive(&entry.path(), &target).ok();
                        } else {
                            std::fs::copy(entry.path(), &target).ok();
                        }
                    }
                }
            }
        }

        // Write custom BUILD file
        if let Some(content) = attrs.get_optional_string("build_file_content") {
            // Remove any symlinked BUILD files first
            std::fs::remove_file(working_dir.join("BUILD.bazel")).ok();
            std::fs::remove_file(working_dir.join("BUILD")).ok();
            std::fs::write(working_dir.join("BUILD.bazel"), content).ok();
        }
    } else {
        // For local_repository: symlink the entire directory (it has its own BUILD)
        #[cfg(unix)]
        {
            std::fs::remove_dir(working_dir).ok();
            std::os::unix::fs::symlink(&resolved_path, working_dir).map_err(|e| {
                RepositoryExecutionError::ExecutionFailed {
                    name: invocation.name.clone(),
                    reason: format!(
                        "Failed to create symlink {} -> {}: {}",
                        working_dir.display(),
                        resolved_path.display(),
                        e
                    ),
                }
            })?;
        }
        #[cfg(not(unix))]
        {
            copy_dir_recursive(&resolved_path, working_dir)?;
        }
    }

    Ok(())
}

#[cfg(not(unix))]
fn copy_dir_recursive(src: &Path, dst: &Path) -> kuro_error::Result<()> {
    std::fs::create_dir_all(dst).ok();

    for entry in std::fs::read_dir(src).map_err(|e| RepositoryExecutionError::ExecutionFailed {
        name: "copy".to_owned(),
        reason: e.to_string(),
    })? {
        let entry = entry.map_err(|e| RepositoryExecutionError::ExecutionFailed {
            name: "copy".to_owned(),
            reason: e.to_string(),
        })?;
        let path = entry.path();
        let dest_path = dst.join(entry.file_name());

        if path.is_dir() {
            copy_dir_recursive(&path, &dest_path)?;
        } else {
            std::fs::copy(&path, &dest_path).ok();
        }
    }

    Ok(())
}

/// Create a stub repository for unknown rules.
fn create_stub_repository(
    invocation: &RepositoryInvocation,
    working_dir: &Path,
) -> kuro_error::Result<()> {
    // Create a minimal BUILD file
    std::fs::write(
        working_dir.join("BUILD.bazel"),
        format!(
            "# Stub repository for '{}'\n# Rule '{}' not yet implemented\n",
            invocation.name, invocation.rule_name
        ),
    )
    .map_err(|e| RepositoryExecutionError::ExecutionFailed {
        name: invocation.name.clone(),
        reason: format!("Failed to write BUILD.bazel: {}", e),
    })?;

    // Create WORKSPACE
    std::fs::write(
        working_dir.join("WORKSPACE.bazel"),
        format!("workspace(name = \"{}\")\n", invocation.name),
    )
    .map_err(|e| RepositoryExecutionError::ExecutionFailed {
        name: invocation.name.clone(),
        reason: format!("Failed to write WORKSPACE.bazel: {}", e),
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_prepare_working_dir() {
        let temp = TempDir::new().unwrap();
        let working_dir = temp.path().join("test_repo");

        prepare_working_dir(&working_dir).unwrap();
        assert!(working_dir.exists());
        assert!(working_dir.is_dir());
    }

    #[test]
    fn test_is_repo_complete() {
        let temp = TempDir::new().unwrap();
        let working_dir = temp.path().join("test_repo");
        std::fs::create_dir_all(&working_dir).unwrap();

        assert!(!is_repo_complete(&working_dir));

        mark_repo_complete(&working_dir).unwrap();
        assert!(is_repo_complete(&working_dir));
    }

    #[test]
    fn test_get_urls() {
        let mut inv = RepositoryInvocation::new("test".to_owned(), "http_archive".to_owned());

        // Single url
        inv.attrs.insert(
            "url".to_owned(),
            crate::repository_invocations::AttrValue::String(
                "https://example.com/a.tar.gz".to_owned(),
            ),
        );

        let attrs = InvocationAttrs::new(&inv);
        let urls = get_urls(&attrs).unwrap();
        assert_eq!(urls, vec!["https://example.com/a.tar.gz"]);

        // Multiple urls
        inv.attrs.insert(
            "urls".to_owned(),
            crate::repository_invocations::AttrValue::StringList(vec![
                "https://example.com/b.tar.gz".to_owned(),
                "https://example.com/c.tar.gz".to_owned(),
            ]),
        );

        let attrs = InvocationAttrs::new(&inv);
        let urls = get_urls(&attrs).unwrap();
        assert_eq!(
            urls,
            vec![
                "https://example.com/a.tar.gz",
                "https://example.com/b.tar.gz",
                "https://example.com/c.tar.gz"
            ]
        );
    }

    #[test]
    fn test_verify_sha256() {
        let data = b"Hello, World!";
        let hash = Sha256::digest(data);
        let expected = hex::encode(hash);

        assert!(verify_sha256(data, &expected).is_ok());
        assert!(verify_sha256(data, "wrong_hash").is_err());
    }

    #[test]
    fn resolve_build_file_label_uses_canonical_label_parser() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let repo_root = project_root.join("bazel-external").join("rules_cc");
        let working_dir = project_root.join("bazel-external").join("current_repo");
        std::fs::create_dir_all(repo_root.join("cc")).unwrap();
        std::fs::create_dir_all(&working_dir).unwrap();
        std::fs::write(repo_root.join("cc").join("BUILD.rules"), "").unwrap();

        let resolved = resolve_build_file_label("@@rules_cc//cc:BUILD.rules", &working_dir);

        assert_eq!(
            PathBuf::from(resolved),
            repo_root.join("cc").join("BUILD.rules")
        );
    }

    #[test]
    fn resolve_build_file_label_keeps_plain_paths_plain() {
        let temp = TempDir::new().unwrap();
        let working_dir = temp.path().join("bazel-external").join("current_repo");

        assert_eq!(
            resolve_build_file_label("third_party/BUILD.foo", &working_dir),
            "third_party/BUILD.foo"
        );
    }

    #[test]
    fn resolve_build_file_label_supports_main_repo_labels() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let working_dir = project_root.join("bazel-external").join("current_repo");
        std::fs::create_dir_all(project_root.join("tools")).unwrap();
        std::fs::create_dir_all(&working_dir).unwrap();
        std::fs::write(project_root.join("tools").join("BUILD.repo"), "").unwrap();

        let resolved = resolve_build_file_label("//tools:BUILD.repo", &working_dir);

        assert_eq!(
            PathBuf::from(resolved),
            project_root.join("tools").join("BUILD.repo")
        );
    }

    #[test]
    fn resolve_build_file_label_quarantines_bazel_external_scan_fallback() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let legacy_repo = project_root.join("bazel-external").join("rules_cc+0.1.0");
        let working_dir = project_root.join("bazel-external").join("current_repo");
        std::fs::create_dir_all(legacy_repo.join("cc")).unwrap();
        std::fs::create_dir_all(&working_dir).unwrap();
        std::fs::write(legacy_repo.join("cc").join("BUILD.rules"), "").unwrap();

        let resolved = resolve_build_file_label("@rules_cc//cc:BUILD.rules", &working_dir);

        assert_eq!(
            PathBuf::from(resolved),
            legacy_repo.join("cc").join("BUILD.rules")
        );
    }
}
