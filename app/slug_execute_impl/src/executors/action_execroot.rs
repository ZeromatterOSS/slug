/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Per-action execroot narrowing — Plan 44 Phase 2.6.
//!
//! Builds a small directory under `<project_root>/execroot/<digest>/`
//! containing only the top-level path components an action's declared
//! inputs and tools require. The action runs with that directory as
//! its `cwd`, so `read_dir(cwd)` returns exactly the prefixes the
//! action needs — matching Bazel's exec_root invariant without
//! sandbox staging.
//!
//! Replaces the global allowlist-filtered execroot from Phase 2.5
//! (`slug_core::cells::ensure_execroot_layout`).

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::hash::Hash;
use std::hash::Hasher;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;

use slug_core::content_hash::ContentBasedPathHash;
use slug_core::fs::artifact_path_resolver::ArtifactFs;
use slug_execute::execute::request::CommandExecutionInput;
use slug_execute::execute::request::CommandExecutionRequest;
use slug_fs::paths::abs_norm_path::AbsNormPath;
use slug_fs::paths::abs_norm_path::AbsNormPathBuf;

#[derive(Debug, Default, Eq, PartialEq)]
pub(crate) struct ActionExecrootPlan {
    top_level_prefixes: BTreeSet<String>,
    external_repos: BTreeSet<String>,
    external_paths: BTreeSet<String>,
    buck_out_inputs: BTreeSet<String>,
    buck_out_writable_dirs: BTreeSet<String>,
    buck_out_declared_outputs: BTreeSet<String>,
}

/// Compute the sorted set of top-level workspace path components
/// that an action's inputs and tools refer to.
///
/// Each component is the first segment of a project-relative path
/// (e.g. `buck-out/v2/gen/foo/bar` → `buck-out`,
/// `external/crates__zerocopy-0.8.42/src/lib.rs` → `external`,
/// `lib/units/build.rs` → `lib`).
pub(crate) fn collect_execroot_plan(
    request: &CommandExecutionRequest,
    artifact_fs: &ArtifactFs,
) -> ActionExecrootPlan {
    let mut plan = ActionExecrootPlan {
        top_level_prefixes: BTreeSet::new(),
        buck_out_inputs: BTreeSet::new(),
        buck_out_writable_dirs: BTreeSet::new(),
        buck_out_declared_outputs: BTreeSet::new(),
        external_repos: BTreeSet::new(),
        external_paths: BTreeSet::new(),
    };

    let inputs_iter = request.inputs().iter().chain(
        request
            .worker()
            .as_ref()
            .map(|w| w.inputs())
            .unwrap_or_default(),
    );

    for input in inputs_iter {
        match input {
            CommandExecutionInput::Artifact(group) => {
                for (artifact, _value) in group.iter() {
                    if let Ok(path) = artifact.resolve_configuration_hash_path(artifact_fs) {
                        add_execroot_path(&mut plan, path.as_str(), false);
                    }
                }
            }
            CommandExecutionInput::IncrementalRemoteOutput(path, _) => {
                add_execroot_path(&mut plan, path.as_str(), false);
            }
            CommandExecutionInput::ActionMetadata(metadata) => {
                if let Ok(path) = artifact_fs
                    .buck_out_path_resolver()
                    .resolve_gen(&metadata.path, Some(&metadata.content_hash))
                {
                    add_execroot_path(&mut plan, path.as_str(), false);
                }
            }
            CommandExecutionInput::ScratchPath(path) => {
                if let Ok(path) = artifact_fs.buck_out_path_resolver().resolve_scratch(path) {
                    add_execroot_path(&mut plan, path.as_str(), true);
                }
            }
        }
    }

    for arg in request.exe().iter().chain(request.args()) {
        add_execroot_paths_from_command_arg(&mut plan, arg);
    }

    for (_key, value) in request.env() {
        add_execroot_path_from_arg_segment(&mut plan, value);
    }

    for output in request.outputs() {
        if let Ok(resolved) = output.resolve(
            artifact_fs,
            Some(&ContentBasedPathHash::for_output_artifact()),
        ) {
            if let Some(buck_out_rel) = strip_buck_out_prefix(resolved.path().as_str()) {
                plan.buck_out_declared_outputs
                    .insert(buck_out_rel.to_owned());
            }
            if let Some(path) = resolved.path_to_create() {
                add_execroot_path(&mut plan, path.as_str(), true);
            }
        }
    }

    remove_buck_out_inputs_overlapping_writable_outputs(&mut plan);

    plan
}

fn add_execroot_paths_from_command_arg(plan: &mut ActionExecrootPlan, arg: &str) {
    let arg = trim_shell_quotes(arg);
    if let Some(value) = arg.strip_prefix('@') {
        add_execroot_path_from_arg_segment(plan, value);
        return;
    }

    if let Some((flag, value)) = arg.split_once('=') {
        if flag.starts_with("--") {
            add_execroot_path_from_arg_segment(plan, value);
        } else {
            add_execroot_path_from_arg_segment(plan, flag);
            add_execroot_path_from_arg_segment(plan, value);
        }
        return;
    }

    add_execroot_path_from_arg_segment(plan, arg);
}

fn add_execroot_path_from_arg_segment(plan: &mut ActionExecrootPlan, segment: &str) {
    let segment = trim_shell_quotes(segment);
    let segment = segment.strip_prefix('@').unwrap_or(segment);
    if is_known_execroot_relative_path(segment) {
        add_execroot_path(plan, segment, false);
    }
}

fn trim_shell_quotes(value: &str) -> &str {
    value
        .strip_prefix('\'')
        .and_then(|s| s.strip_suffix('\''))
        .or_else(|| value.strip_prefix('"').and_then(|s| s.strip_suffix('"')))
        .unwrap_or(value)
}

fn is_known_execroot_relative_path(path: &str) -> bool {
    matches!(path, "external" | "buck-out" | "bazel-out")
        || path.starts_with("external/")
        || path.starts_with("buck-out/")
        || path.starts_with("bazel-out/")
}

fn add_execroot_path(plan: &mut ActionExecrootPlan, path: &str, writable_dir: bool) {
    if let Some(buck_out_rel) = strip_buck_out_prefix(path) {
        if writable_dir {
            plan.buck_out_writable_dirs.insert(buck_out_rel.to_owned());
        } else {
            plan.buck_out_inputs.insert(buck_out_rel.to_owned());
        }
    } else if let Some(external_rel) = strip_external_prefix(path) {
        if let Some((repo, rest)) = external_rel.split_once('/') {
            if !repo.is_empty() && !rest.is_empty() {
                plan.external_paths.insert(external_rel.to_owned());
            }
        } else if let Some(repo) = top_level_component(external_rel) {
            plan.external_repos.insert(repo.to_owned());
        } else {
            plan.top_level_prefixes.insert("external".to_owned());
        }
    } else if let Some(prefix) = top_level_component(path) {
        plan.top_level_prefixes.insert(prefix.to_owned());
    }
}

fn remove_buck_out_inputs_overlapping_writable_outputs(plan: &mut ActionExecrootPlan) {
    if plan.buck_out_inputs.is_empty() || plan.buck_out_declared_outputs.is_empty() {
        return;
    }

    let output_paths = plan.buck_out_declared_outputs.clone();
    plan.buck_out_inputs.retain(|input| {
        !output_paths
            .iter()
            .any(|output| paths_overlap(input, output))
    });
}

fn paths_overlap(a: &str, b: &str) -> bool {
    a == b || path_is_prefix(a, b) || path_is_prefix(b, a)
}

fn path_is_prefix(prefix: &str, path: &str) -> bool {
    if prefix.is_empty() {
        return true;
    }
    path.strip_prefix(prefix)
        .is_some_and(|rest| rest.starts_with('/'))
}

fn strip_buck_out_prefix(path: &str) -> Option<&str> {
    let rest = path.trim_start_matches('/').strip_prefix("buck-out")?;
    if rest.is_empty() {
        return Some("");
    }
    rest.strip_prefix('/')
}

fn strip_external_prefix(path: &str) -> Option<&str> {
    let rest = path.trim_start_matches('/').strip_prefix("external")?;
    if rest.is_empty() {
        return Some("");
    }
    rest.strip_prefix('/')
}

/// Extract the first path component of a project-relative path.
///
/// Returns `None` for empty paths or paths that escape the workspace
/// (defensive — `ProjectRelativePath` should never have those, but
/// the helper is paranoid).
fn top_level_component(path: &str) -> Option<&str> {
    let trimmed = path.trim_start_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    let first = trimmed.split('/').next()?;
    if first.is_empty() || first == "." || first == ".." {
        return None;
    }
    Some(first)
}

/// Stable digest for a sorted prefix set. Used as the per-action
/// execroot directory name. Not security-sensitive — just needs to
/// dedupe identical input shapes — so the standard hasher is fine.
fn digest_plan(plan: &ActionExecrootPlan) -> String {
    // Use SipHasher with fixed keys for stability across processes
    // (the std DefaultHasher uses a randomized key).
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    "prefixes".hash(&mut hasher);
    for prefix in &plan.top_level_prefixes {
        prefix.hash(&mut hasher);
        0u8.hash(&mut hasher); // separator
    }
    "external-repos".hash(&mut hasher);
    for repo in &plan.external_repos {
        repo.hash(&mut hasher);
        0u8.hash(&mut hasher);
    }
    "external-paths".hash(&mut hasher);
    for path in &plan.external_paths {
        path.hash(&mut hasher);
        0u8.hash(&mut hasher);
    }
    "buck-out-inputs".hash(&mut hasher);
    for path in &plan.buck_out_inputs {
        path.hash(&mut hasher);
        0u8.hash(&mut hasher);
    }
    "buck-out-writable-dirs".hash(&mut hasher);
    for path in &plan.buck_out_writable_dirs {
        path.hash(&mut hasher);
        0u8.hash(&mut hasher);
    }
    format!("{:016x}", hasher.finish())
}

/// Process-global cache of execroot directories that have already been
/// materialised this build. Keyed by digest so concurrent actions with
/// the same input shape share the same directory without re-doing
/// `mkdir`/`symlinkat` work.
static MATERIALIZED_EXECROOTS: Mutex<Option<MaterializedSet>> = Mutex::new(None);

struct MaterializedSet {
    project_root: PathBuf,
    active: HashMap<String, usize>,
}

#[derive(Debug)]
pub(crate) struct ActionExecrootLease {
    project_root: PathBuf,
    digest: String,
    path: AbsNormPathBuf,
}

impl ActionExecrootLease {
    #[cfg(test)]
    pub(crate) fn as_path(&self) -> &Path {
        self.path.as_path()
    }

    pub(crate) fn as_abs_norm_path(&self) -> &AbsNormPath {
        self.path.as_ref()
    }
}

impl PartialEq for ActionExecrootLease {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}

impl Eq for ActionExecrootLease {}

impl Drop for ActionExecrootLease {
    fn drop(&mut self) {
        let should_remove = {
            let Ok(mut guard) = MATERIALIZED_EXECROOTS.lock() else {
                return;
            };
            let Some(entry) = guard.as_mut() else {
                return;
            };
            if entry.project_root != self.project_root {
                return;
            }
            let Some(count) = entry.active.get_mut(&self.digest) else {
                return;
            };
            if *count > 1 {
                *count -= 1;
                false
            } else {
                entry.active.remove(&self.digest);
                true
            }
        };

        if should_remove {
            let _ = std::fs::remove_dir_all(self.path.as_path());
        }
    }
}

/// Build (or return the cached path of) a per-action execroot
/// containing top-level symlinks for every prefix in `prefixes`.
///
/// Path: `<project_root>/execroot/<digest>/`.
///
/// Mirrors Bazel's
/// `<output_base>/execroot/<workspace_name>/` layout so that
/// rules_rust's `process_wrapper` (which derives `${exec_root}` as
/// `<output_base>/execroot/<basename of cwd>`) resolves to the
/// directory that actually exists on disk. The directory is created
/// lazily; symlinks point to the workspace's `<project_root>/<prefix>/`
/// directory. If a prefix doesn't exist as a workspace directory it's
/// silently skipped — actions reference real paths only.
pub(crate) fn ensure_execroot(
    project_root: &AbsNormPath,
    plan: &ActionExecrootPlan,
) -> Option<ActionExecrootLease> {
    if plan.top_level_prefixes.is_empty()
        && plan.external_repos.is_empty()
        && plan.external_paths.is_empty()
        && plan.buck_out_inputs.is_empty()
        && plan.buck_out_writable_dirs.is_empty()
    {
        return None;
    }
    if !plan.external_repos.is_empty()
        || !plan.external_paths.is_empty()
        || plan.top_level_prefixes.contains("external")
    {
        slug_core::cells::repair_external_symlink_targets(project_root.as_path());
    }

    let digest = digest_plan(plan);
    let execroot_abs: PathBuf = project_root.as_path().join("execroot").join(&digest);

    let mut guard = MATERIALIZED_EXECROOTS.lock().ok()?;
    let entry = guard.get_or_insert_with(|| MaterializedSet {
        project_root: project_root.as_path().to_path_buf(),
        active: Default::default(),
    });
    // Reset cache if the project root changed (e.g. test isolation).
    if entry.project_root != project_root.as_path() {
        entry.project_root = project_root.as_path().to_path_buf();
        entry.active.clear();
    }

    if let Some(count) = entry.active.get_mut(&digest) {
        *count += 1;
        let path = AbsNormPathBuf::new(execroot_abs).ok()?;
        return Some(ActionExecrootLease {
            project_root: project_root.as_path().to_path_buf(),
            digest,
            path,
        });
    }

    if let Err(e) = std::fs::create_dir_all(&execroot_abs) {
        tracing::debug!(?e, "failed to create per-action execroot dir; falling back");
        return None;
    }

    for prefix in &plan.top_level_prefixes {
        let target = project_root.as_path().join(prefix);
        if !target.exists() {
            continue;
        }
        let link = execroot_abs.join(prefix);
        if prefix == "external" {
            let repos = project_root
                .as_path()
                .join("external")
                .read_dir()
                .ok()
                .into_iter()
                .flatten()
                .filter_map(|entry| entry.ok())
                .filter_map(|entry| entry.file_name().into_string().ok())
                .collect();
            if !materialize_external_prefix(
                project_root.as_path(),
                &target,
                &link,
                &repos,
                &BTreeSet::new(),
            ) {
                return None;
            }
            continue;
        }
        match link.symlink_metadata() {
            Ok(meta) if meta.file_type().is_symlink() => {
                // Stale or wrong target — refresh.
                if !remove_symlink_path(&link) {
                    return None;
                }
            }
            Ok(_) => continue,
            Err(_) => {}
        }
        #[cfg(unix)]
        let r = std::os::unix::fs::symlink(&target, &link);
        #[cfg(windows)]
        let r = std::os::windows::fs::symlink_dir(&target, &link);
        if let Err(e) = r {
            // EEXIST race with another action populating the same
            // dir is fine — the symlink is content-equivalent
            // because `prefixes` derives from the same digest.
            if e.kind() != std::io::ErrorKind::AlreadyExists {
                tracing::debug!(
                    ?e,
                    prefix = %prefix,
                    "failed to populate execroot symlink; falling back"
                );
                return None;
            }
        }
    }

    if !plan.external_repos.is_empty() || !plan.external_paths.is_empty() {
        let project_external = project_root.as_path().join("external");
        let execroot_external = execroot_abs.join("external");
        if !materialize_external_prefix(
            project_root.as_path(),
            &project_external,
            &execroot_external,
            &plan.external_repos,
            &plan.external_paths,
        ) {
            return None;
        }
    }

    if !plan.buck_out_inputs.is_empty() || !plan.buck_out_writable_dirs.is_empty() {
        let buck_out_root = execroot_abs.join("buck-out");
        if !ensure_directory_path(&buck_out_root) {
            return None;
        }

        for rel in &plan.buck_out_writable_dirs {
            let dir = buck_out_root.join(rel);
            if !ensure_directory_path(&dir) {
                return None;
            }
        }

        for rel in buck_out_inputs_to_link(project_root.as_path(), &plan.buck_out_inputs) {
            let target = project_root.as_path().join("buck-out").join(&rel);
            if !target.exists() {
                continue;
            }
            if !link_buck_out_path(&buck_out_root, &rel, &target, target.is_dir()) {
                return None;
            }
        }
    }

    entry.active.insert(digest.clone(), 1);
    let path = AbsNormPathBuf::new(execroot_abs).ok()?;
    Some(ActionExecrootLease {
        project_root: project_root.as_path().to_path_buf(),
        digest,
        path,
    })
}

fn buck_out_inputs_to_link(project_root: &Path, inputs: &BTreeSet<String>) -> Vec<String> {
    let mut linked = Vec::new();
    for input in inputs {
        if !input.is_empty()
            && linked.iter().any(|ancestor: &String| {
                !ancestor.is_empty()
                    && path_is_prefix(ancestor, input)
                    && project_root.join("buck-out").join(ancestor).is_dir()
            })
        {
            continue;
        }
        linked.push(input.clone());
    }
    linked
}

fn link_buck_out_path(buck_out_root: &Path, rel: &str, target: &Path, target_is_dir: bool) -> bool {
    let link = buck_out_root.join(rel);
    let Some(parent) = link.parent() else {
        return false;
    };
    if !ensure_directory_path(parent) {
        return false;
    }
    match link.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            let current = std::fs::read_link(&link).ok();
            if current.as_deref() == Some(target) {
                return true;
            }
            if !remove_symlink_path(&link) {
                return false;
            }
        }
        Ok(_) => return true,
        Err(_) => {}
    }

    #[cfg(unix)]
    let r = std::os::unix::fs::symlink(target, &link);
    #[cfg(windows)]
    let r = if target_is_dir {
        std::os::windows::fs::symlink_dir(target, &link)
    } else {
        std::os::windows::fs::symlink_file(target, &link)
    };
    if let Err(e) = r {
        if e.kind() != std::io::ErrorKind::AlreadyExists {
            tracing::debug!(
                ?e,
                link = %link.display(),
                target = %target.display(),
                "failed to populate filtered buck-out execroot path"
            );
            return false;
        }
    }
    let _ = target_is_dir;
    true
}

fn ensure_directory_path(path: &Path) -> bool {
    match path.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            if !remove_symlink_path(path) {
                return false;
            }
        }
        Ok(meta) if meta.is_dir() => return true,
        Ok(_) => return false,
        Err(_) => {}
    }
    if let Err(e) = std::fs::create_dir_all(path) {
        tracing::debug!(
            ?e,
            path = %path.display(),
            "failed to create execroot directory"
        );
        return false;
    }
    true
}

fn materialize_external_prefix(
    project_root: &std::path::Path,
    project_external: &std::path::Path,
    execroot_external: &std::path::Path,
    repos: &BTreeSet<String>,
    paths: &BTreeSet<String>,
) -> bool {
    let mut alias_cache: BTreeMap<std::path::PathBuf, Vec<String>> = BTreeMap::new();

    match execroot_external.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            if !remove_symlink_path(execroot_external) {
                tracing::debug!(
                    path = %execroot_external.display(),
                    "failed to replace external prefix symlink with directory"
                );
                return false;
            }
        }
        Ok(meta) if !meta.is_dir() => return true,
        Ok(_) => {}
        Err(_) => {}
    }
    if let Err(e) = std::fs::create_dir_all(execroot_external) {
        tracing::debug!(
            ?e,
            path = %execroot_external.display(),
            "failed to create execroot external directory"
        );
        return false;
    }

    for repo in repos {
        let Some(target) = external_repo_target(project_root, project_external, repo) else {
            continue;
        };
        if !link_external_entry(execroot_external, repo.into(), &target) {
            return false;
        }
        let Some(aliases) =
            external_alias_names_for_target(project_external, &target, &mut alias_cache)
        else {
            return false;
        };
        for alias in aliases {
            if !link_external_entry(execroot_external, alias.into(), &target) {
                return false;
            }
        }
    }

    for path in paths {
        let Some((repo, rel)) = path.split_once('/') else {
            continue;
        };
        let Some(target) = external_repo_target(project_root, project_external, repo) else {
            continue;
        };
        if !link_external_path(execroot_external, repo, rel, &target) {
            return false;
        }
        let Some(aliases) =
            external_alias_names_for_target(project_external, &target, &mut alias_cache)
        else {
            return false;
        };
        for alias in aliases {
            if !link_external_path(execroot_external, &alias, rel, &target) {
                return false;
            }
        }
    }

    true
}

fn external_repo_target(
    project_root: &std::path::Path,
    project_external: &std::path::Path,
    repo: &str,
) -> Option<std::path::PathBuf> {
    let apparent = project_external.join(repo);
    if apparent.symlink_metadata().is_ok() {
        return Some(
            canonical_external_entry_target(project_external, &apparent).unwrap_or(apparent),
        );
    }

    let canonical = project_root.join("bazel-external").join(repo);
    if canonical.symlink_metadata().is_ok() {
        return Some(std::fs::canonicalize(&canonical).unwrap_or(canonical));
    }

    None
}

fn external_alias_names_for_target(
    project_external: &std::path::Path,
    selected_target: &std::path::Path,
    alias_cache: &mut BTreeMap<std::path::PathBuf, Vec<String>>,
) -> Option<Vec<String>> {
    if let Some(aliases) = alias_cache.get(selected_target) {
        return Some(aliases.clone());
    }

    let entries = match std::fs::read_dir(project_external) {
        Ok(entries) => entries,
        Err(e) => {
            tracing::debug!(
                ?e,
                path = %project_external.display(),
                "failed to read project external aliases"
            );
            return None;
        }
    };

    let mut aliases = Vec::new();
    for entry in entries.flatten() {
        let target = canonical_external_entry_target(project_external, &entry.path())
            .unwrap_or_else(|| entry.path());
        if target != selected_target {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(ToOwned::to_owned) else {
            continue;
        };
        aliases.push(name);
    }

    alias_cache.insert(selected_target.to_path_buf(), aliases.clone());
    Some(aliases)
}

fn link_external_path(
    execroot_external: &std::path::Path,
    repo: &str,
    rel: &str,
    repo_target: &std::path::Path,
) -> bool {
    let target = repo_target.join(rel);
    if !target.exists() {
        return true;
    }

    let repo_root = execroot_external.join(repo);
    match repo_root.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            if !remove_symlink_path(&repo_root) {
                tracing::debug!(
                    link = %repo_root.display(),
                    "failed to replace execroot external repo symlink for nested path"
                );
                return false;
            }
        }
        Ok(meta) if !meta.is_dir() => return true,
        Ok(_) | Err(_) => {}
    }

    let link = repo_root.join(rel);
    let Some(parent) = link.parent() else {
        return false;
    };
    if !ensure_directory_path(parent) {
        return false;
    }
    match link.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            let current = std::fs::read_link(&link).ok();
            if current.as_deref() == Some(&target) {
                return true;
            }
            if !remove_symlink_path(&link) {
                tracing::debug!(
                    link = %link.display(),
                    target = %target.display(),
                    "failed to remove stale execroot external file symlink"
                );
                return false;
            }
        }
        Ok(_) => return true,
        Err(_) => {}
    }

    #[cfg(unix)]
    let r = std::os::unix::fs::symlink(&target, &link);
    #[cfg(windows)]
    let r = if target.is_dir() {
        std::os::windows::fs::symlink_dir(&target, &link)
    } else {
        std::os::windows::fs::symlink_file(&target, &link)
    };
    if let Err(e) = r {
        if e.kind() != std::io::ErrorKind::AlreadyExists {
            tracing::debug!(
                ?e,
                link = %link.display(),
                target = %target.display(),
                "failed to populate execroot external file symlink"
            );
            return false;
        }
    }
    true
}

fn link_external_entry(
    execroot_external: &std::path::Path,
    name: std::ffi::OsString,
    target: &std::path::Path,
) -> bool {
    let link = execroot_external.join(name);
    match link.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            let current = std::fs::read_link(&link).ok();
            if current.as_deref() == Some(target) {
                return true;
            }
            if !remove_symlink_path(&link) {
                tracing::debug!(
                    link = %link.display(),
                    target = %target.display(),
                    "failed to remove stale execroot external repo symlink"
                );
                return false;
            }
        }
        Ok(_) => return true,
        Err(_) => {}
    }

    #[cfg(unix)]
    let r = std::os::unix::fs::symlink(target, &link);
    #[cfg(windows)]
    let r = std::os::windows::fs::symlink_dir(target, &link);
    if let Err(e) = r {
        if e.kind() != std::io::ErrorKind::AlreadyExists {
            tracing::debug!(
                ?e,
                link = %link.display(),
                target = %target.display(),
                "failed to populate execroot external repo symlink"
            );
            return false;
        }
    }
    true
}

fn remove_symlink_path(path: &std::path::Path) -> bool {
    match std::fs::remove_file(path) {
        Ok(()) => true,
        Err(file_err) => match std::fs::remove_dir(path) {
            Ok(()) => true,
            Err(dir_err) => {
                tracing::debug!(
                    ?file_err,
                    ?dir_err,
                    path = %path.display(),
                    "failed to remove symlink"
                );
                false
            }
        },
    }
}

fn canonical_external_entry_target(
    project_external: &std::path::Path,
    entry_path: &std::path::Path,
) -> Option<std::path::PathBuf> {
    let metadata = std::fs::symlink_metadata(entry_path).ok()?;
    if !metadata.file_type().is_symlink() {
        return std::fs::canonicalize(entry_path).ok();
    }
    let target = std::fs::read_link(entry_path).ok()?;
    let target = if target.is_absolute() {
        target
    } else {
        project_external.join(target)
    };
    std::fs::canonicalize(target).ok()
}

/// Reset the materialised-execroot cache. Tests use this between
/// independent project roots to avoid cross-talk.
#[cfg(test)]
pub(crate) fn reset_cache_for_test() {
    if let Ok(mut guard) = MATERIALIZED_EXECROOTS.lock() {
        *guard = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_plan(prefixes: BTreeSet<String>) -> ActionExecrootPlan {
        ActionExecrootPlan {
            top_level_prefixes: prefixes,
            ..Default::default()
        }
    }

    fn digest_prefixes(prefixes: &BTreeSet<String>) -> String {
        digest_plan(&ActionExecrootPlan {
            top_level_prefixes: prefixes.clone(),
            ..Default::default()
        })
    }

    #[test]
    fn top_level_component_strips_slashes() {
        assert_eq!(top_level_component(""), None);
        assert_eq!(top_level_component("/"), None);
        assert_eq!(top_level_component("/foo"), Some("foo"));
        assert_eq!(top_level_component("foo"), Some("foo"));
        assert_eq!(top_level_component("foo/bar"), Some("foo"));
        assert_eq!(top_level_component("foo/bar/baz"), Some("foo"));
        assert_eq!(top_level_component("./foo"), None);
    }

    #[test]
    fn digest_is_stable_and_set_independent_order() {
        let mut a = BTreeSet::new();
        a.insert("buck-out".to_owned());
        a.insert("external".to_owned());
        a.insert("lib".to_owned());

        let mut b = BTreeSet::new();
        b.insert("lib".to_owned());
        b.insert("external".to_owned());
        b.insert("buck-out".to_owned());

        assert_eq!(digest_prefixes(&a), digest_prefixes(&b));
    }

    #[test]
    fn digest_changes_with_prefix_set() {
        let mut a = BTreeSet::new();
        a.insert("buck-out".to_owned());

        let mut b = BTreeSet::new();
        b.insert("buck-out".to_owned());
        b.insert("external".to_owned());

        assert_ne!(digest_prefixes(&a), digest_prefixes(&b));
    }

    #[test]
    fn digest_changes_with_external_paths() {
        let mut a = ActionExecrootPlan::default();
        a.external_paths
            .insert("llvm++musl+musl_libc/include/float.h".to_owned());

        let mut b = ActionExecrootPlan::default();
        b.external_paths
            .insert("llvm++musl+musl_libc/include/float.h".to_owned());
        b.external_paths
            .insert("llvm++musl+musl_libc/arch/x86_64/bits/float.h".to_owned());

        assert_ne!(digest_plan(&a), digest_plan(&b));
    }

    #[test]
    fn execroot_uses_workspace_layout() {
        reset_cache_for_test();
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path().to_path_buf();
        std::fs::create_dir_all(project.join("buck-out")).unwrap();
        let project_norm = AbsNormPathBuf::new(project.clone()).unwrap();

        let mut prefixes = BTreeSet::new();
        prefixes.insert("buck-out".to_owned());

        let execroot = ensure_execroot(&project_norm, &test_plan(prefixes)).unwrap();

        assert!(execroot.as_path().starts_with(project.join("execroot")));
    }

    #[test]
    fn ensure_execroot_creates_dir_with_symlinks() {
        reset_cache_for_test();
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path();
        std::fs::create_dir(project.join("buck-out")).unwrap();
        std::fs::create_dir(project.join("lib")).unwrap();
        let project_norm = AbsNormPathBuf::new(project.to_path_buf()).unwrap();

        let mut prefixes = BTreeSet::new();
        prefixes.insert("buck-out".to_owned());
        prefixes.insert("lib".to_owned());

        let exec = ensure_execroot(&project_norm, &test_plan(prefixes)).unwrap();

        assert!(exec.as_path().is_dir());
        assert!(exec.as_path().join("buck-out").is_dir());
        assert!(exec.as_path().join("lib").is_dir());

        // Workspace dirs not in the prefix set are absent.
        assert!(!exec.as_path().join("ci").exists());
    }

    #[test]
    fn ensure_execroot_skips_missing_workspace_dirs() {
        reset_cache_for_test();
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path();
        std::fs::create_dir(project.join("buck-out")).unwrap();
        let project_norm = AbsNormPathBuf::new(project.to_path_buf()).unwrap();

        let mut prefixes = BTreeSet::new();
        prefixes.insert("buck-out".to_owned());
        prefixes.insert("does-not-exist".to_owned());

        let exec = ensure_execroot(&project_norm, &test_plan(prefixes)).unwrap();

        assert!(exec.as_path().join("buck-out").is_dir());
        assert!(!exec.as_path().join("does-not-exist").exists());
    }

    #[test]
    fn ensure_execroot_repairs_external_symlink_chains() {
        reset_cache_for_test();
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path();
        let external = project.join("external");
        let bazel_external = project.join("bazel-external");
        let cache_repo = project.join("cache").join("rules_rust");
        std::fs::create_dir_all(&external).unwrap();
        std::fs::create_dir_all(&bazel_external).unwrap();
        std::fs::create_dir_all(&cache_repo).unwrap();

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&cache_repo, bazel_external.join("rules_rust+0.69.0"))
                .unwrap();
            std::os::unix::fs::symlink(
                std::path::PathBuf::from("..")
                    .join("bazel-external")
                    .join("rules_rust+0.69.0"),
                external.join("rules_rust"),
            )
            .unwrap();
        }
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_dir(
                &cache_repo,
                bazel_external.join("rules_rust+0.69.0"),
            )
            .unwrap();
            std::os::windows::fs::symlink_dir(
                std::path::PathBuf::from("..")
                    .join("bazel-external")
                    .join("rules_rust+0.69.0"),
                external.join("rules_rust"),
            )
            .unwrap();
        }

        let project_norm = AbsNormPathBuf::new(project.to_path_buf()).unwrap();
        let mut plan = ActionExecrootPlan::default();
        plan.external_repos.insert("rules_rust".to_owned());
        plan.external_repos.insert("rules_rust+0.69.0".to_owned());

        let exec = ensure_execroot(&project_norm, &plan).unwrap();

        assert!(exec.as_path().join("external").is_dir());
        assert_eq!(
            std::fs::canonicalize(exec.as_path().join("external").join("rules_rust")).unwrap(),
            std::fs::canonicalize(&cache_repo).unwrap()
        );
        assert_eq!(
            std::fs::canonicalize(exec.as_path().join("external").join("rules_rust+0.69.0"))
                .unwrap(),
            std::fs::canonicalize(&cache_repo).unwrap()
        );
        assert_eq!(
            std::fs::canonicalize(external.join("rules_rust")).unwrap(),
            std::fs::canonicalize(&cache_repo).unwrap()
        );
    }

    #[test]
    fn identical_prefix_sets_share_execroot() {
        reset_cache_for_test();
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path();
        std::fs::create_dir(project.join("buck-out")).unwrap();
        let project_norm = AbsNormPathBuf::new(project.to_path_buf()).unwrap();

        let mut prefixes = BTreeSet::new();
        prefixes.insert("buck-out".to_owned());

        let a = ensure_execroot(&project_norm, &test_plan(prefixes.clone())).unwrap();
        let b = ensure_execroot(&project_norm, &test_plan(prefixes)).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn execroot_lease_removes_directory_after_last_user() {
        reset_cache_for_test();
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path();
        std::fs::create_dir(project.join("src")).unwrap();
        let project_norm = AbsNormPathBuf::new(project.to_path_buf()).unwrap();
        let prefixes = BTreeSet::from(["src".to_owned()]);

        let first = ensure_execroot(&project_norm, &test_plan(prefixes.clone())).unwrap();
        let execroot_path = first.as_path().to_path_buf();
        assert!(execroot_path.exists());

        let second = ensure_execroot(&project_norm, &test_plan(prefixes)).unwrap();
        assert_eq!(first, second);
        drop(first);
        assert!(
            execroot_path.exists(),
            "shared execroot should stay until the last lease drops"
        );
        drop(second);
        assert!(
            !execroot_path.exists(),
            "execroot should be removed after the last live action lease"
        );
    }

    #[test]
    fn external_paths_track_only_referenced_files() {
        let mut plan = ActionExecrootPlan::default();

        add_execroot_path(
            &mut plan,
            "external/rules_rs++crate+crates__serde-1.0.228/src/lib.rs",
            false,
        );
        add_execroot_path(
            &mut plan,
            "external/crates__proc-macro2-1.0.106/src/lib.rs",
            false,
        );
        add_execroot_path(&mut plan, "lib/units/src/lib.rs", false);

        assert!(plan.top_level_prefixes.contains("lib"));
        assert!(!plan.top_level_prefixes.contains("external"));
        assert!(
            plan.external_paths
                .contains("rules_rs++crate+crates__serde-1.0.228/src/lib.rs")
        );
        assert!(
            plan.external_paths
                .contains("crates__proc-macro2-1.0.106/src/lib.rs")
        );
    }

    #[test]
    fn command_args_contribute_execroot_relative_paths() {
        let mut plan = ActionExecrootPlan::default();

        add_execroot_paths_from_command_arg(
            &mut plan,
            "--input_dep_env_path=external/rules_rs++crate+crates__serde_core-1.0.228/cargo_toml_env_vars.env",
        );
        add_execroot_paths_from_command_arg(&mut plan, "buck-out/plan61/gen/repo/out=m/out");
        add_execroot_paths_from_command_arg(&mut plan, "'@buck-out/plan61/tmp/params'");

        assert!(
            plan.external_paths
                .contains("rules_rs++crate+crates__serde_core-1.0.228/cargo_toml_env_vars.env")
        );
        assert!(plan.buck_out_inputs.contains("plan61/gen/repo/out"));
        assert!(plan.buck_out_inputs.contains("plan61/tmp/params"));
        assert!(!plan.top_level_prefixes.contains("m"));
    }

    #[test]
    fn output_tree_args_do_not_prelink_declared_outputs() {
        let mut plan = ActionExecrootPlan::default();

        add_execroot_paths_from_command_arg(
            &mut plan,
            "--sysroot=buck-out/plan61/gen/rust_toolchain/linux_x86_64_bootstrap",
        );
        add_execroot_path(
            &mut plan,
            "buck-out/plan61/gen/rust_toolchain/linux_x86_64_bootstrap/bin/rustc",
            true,
        );
        plan.buck_out_declared_outputs
            .insert("plan61/gen/rust_toolchain/linux_x86_64_bootstrap/bin/rustc".to_owned());
        remove_buck_out_inputs_overlapping_writable_outputs(&mut plan);

        assert!(
            !plan
                .buck_out_inputs
                .contains("plan61/gen/rust_toolchain/linux_x86_64_bootstrap")
        );
        assert!(
            plan.buck_out_writable_dirs
                .contains("plan61/gen/rust_toolchain/linux_x86_64_bootstrap/bin/rustc")
        );
    }

    #[test]
    fn output_tree_args_keep_non_overlapping_inputs() {
        let mut plan = ActionExecrootPlan::default();

        add_execroot_paths_from_command_arg(
            &mut plan,
            "buck-out/plan61/gen/pkg/output_config.json",
        );
        add_execroot_path(&mut plan, "buck-out/plan61/gen/pkg", true);
        plan.buck_out_declared_outputs
            .insert("plan61/gen/pkg/output".to_owned());
        remove_buck_out_inputs_overlapping_writable_outputs(&mut plan);

        assert!(
            plan.buck_out_inputs
                .contains("plan61/gen/pkg/output_config.json")
        );
        assert!(plan.buck_out_writable_dirs.contains("plan61/gen/pkg"));
    }

    #[test]
    fn ensure_execroot_materializes_sparse_external_repos() {
        reset_cache_for_test();
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path();
        let external = project.join("external");
        let bazel_external = project.join("bazel-external");
        let serde_repo = project.join("repos").join("serde");
        let quote_repo = project.join("repos").join("quote");
        std::fs::create_dir_all(&external).unwrap();
        std::fs::create_dir_all(&bazel_external).unwrap();
        std::fs::create_dir_all(&serde_repo).unwrap();
        std::fs::create_dir_all(&quote_repo).unwrap();

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&serde_repo, external.join("crates__serde-1.0.228"))
                .unwrap();
            std::os::unix::fs::symlink(
                &quote_repo,
                bazel_external.join("rules_rs++crate+crates__quote-1.0.42"),
            )
            .unwrap();
            std::os::unix::fs::symlink(&quote_repo, external.join("crates__quote-1.0.42")).unwrap();
        }
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_dir(&serde_repo, external.join("crates__serde-1.0.228"))
                .unwrap();
            std::os::windows::fs::symlink_dir(
                &quote_repo,
                bazel_external.join("rules_rs++crate+crates__quote-1.0.42"),
            )
            .unwrap();
            std::os::windows::fs::symlink_dir(&quote_repo, external.join("crates__quote-1.0.42"))
                .unwrap();
        }

        let mut plan = ActionExecrootPlan::default();
        plan.external_repos
            .insert("crates__serde-1.0.228".to_owned());
        plan.external_repos
            .insert("rules_rs++crate+crates__quote-1.0.42".to_owned());

        let project_norm = AbsNormPathBuf::new(project.to_path_buf()).unwrap();
        let exec = ensure_execroot(&project_norm, &plan).unwrap();

        assert_eq!(
            std::fs::canonicalize(exec.as_path().join("external/crates__serde-1.0.228")).unwrap(),
            std::fs::canonicalize(&serde_repo).unwrap()
        );
        assert_eq!(
            std::fs::canonicalize(
                exec.as_path()
                    .join("external/rules_rs++crate+crates__quote-1.0.42")
            )
            .unwrap(),
            std::fs::canonicalize(&quote_repo).unwrap()
        );
        assert_eq!(
            std::fs::canonicalize(exec.as_path().join("external/crates__quote-1.0.42")).unwrap(),
            std::fs::canonicalize(&quote_repo).unwrap()
        );
        assert!(
            !exec
                .as_path()
                .join("external/crates__unreferenced-1.0.0")
                .exists()
        );
    }

    #[test]
    fn ensure_execroot_materializes_external_file_paths_with_real_directories() {
        reset_cache_for_test();
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path();
        let external = project.join("external");
        let repo = project.join("repos").join("diplomat");
        std::fs::create_dir_all(&external).unwrap();
        std::fs::create_dir_all(repo.join("tool/src/js")).unwrap();
        std::fs::create_dir_all(repo.join("tool/templates/js")).unwrap();
        std::fs::write(repo.join("tool/src/js/gen.rs"), b"source").unwrap();
        std::fs::write(repo.join("tool/templates/js/base.js.jinja"), b"template").unwrap();

        #[cfg(unix)]
        std::os::unix::fs::symlink(&repo, external.join("rules_rs++crate+crates__diplomat.git"))
            .unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(
            &repo,
            external.join("rules_rs++crate+crates__diplomat.git"),
        )
        .unwrap();

        let project_norm = AbsNormPathBuf::new(project.to_path_buf()).unwrap();
        let mut plan = ActionExecrootPlan::default();
        plan.top_level_prefixes.insert("external".to_owned());
        plan.external_paths
            .insert("rules_rs++crate+crates__diplomat.git/tool/src/js/gen.rs".to_owned());
        plan.external_paths.insert(
            "rules_rs++crate+crates__diplomat.git/tool/templates/js/base.js.jinja".to_owned(),
        );

        let exec = ensure_execroot(&project_norm, &plan).unwrap();
        let exec_repo = exec
            .as_path()
            .join("external/rules_rs++crate+crates__diplomat.git");
        assert!(
            exec_repo.is_dir(),
            "external repo parent should be a real execroot directory"
        );
        assert!(
            !exec_repo
                .symlink_metadata()
                .unwrap()
                .file_type()
                .is_symlink(),
            "external repo parent must not be a symlink when nested paths are declared"
        );
        assert!(exec_repo.join("tool/src/js/gen.rs").exists());
        assert!(exec_repo.join("tool/templates/js/base.js.jinja").exists());
        assert!(
            exec
                .as_path()
                .join(
                    "external/rules_rs++crate+crates__diplomat.git/tool/src/js/../../templates/js/base.js.jinja"
                )
                .exists(),
            "`..` through declared external source paths should stay inside the execroot"
        );
    }

    #[test]
    fn ensure_execroot_cache_distinguishes_external_path_sets() {
        reset_cache_for_test();
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path();
        let external = project.join("external");
        let repo = project.join("repos").join("musl");
        std::fs::create_dir_all(&external).unwrap();
        std::fs::create_dir_all(repo.join("include")).unwrap();
        std::fs::create_dir_all(repo.join("arch/x86_64/bits")).unwrap();
        std::fs::write(repo.join("include/float.h"), b"#include <bits/float.h>\n").unwrap();
        std::fs::write(repo.join("arch/x86_64/bits/float.h"), b"bits").unwrap();

        #[cfg(unix)]
        std::os::unix::fs::symlink(&repo, external.join("llvm++musl+musl_libc")).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&repo, external.join("llvm++musl+musl_libc")).unwrap();

        let project_norm = AbsNormPathBuf::new(project.to_path_buf()).unwrap();
        let mut include_only = ActionExecrootPlan::default();
        include_only
            .external_paths
            .insert("llvm++musl+musl_libc/include/float.h".to_owned());

        let include_exec = ensure_execroot(&project_norm, &include_only).unwrap();
        assert!(
            include_exec
                .as_path()
                .join("external/llvm++musl+musl_libc/include/float.h")
                .exists()
        );
        assert!(
            !include_exec
                .as_path()
                .join("external/llvm++musl+musl_libc/arch/x86_64/bits/float.h")
                .exists()
        );

        let mut include_and_arch = ActionExecrootPlan::default();
        include_and_arch
            .external_paths
            .insert("llvm++musl+musl_libc/include/float.h".to_owned());
        include_and_arch
            .external_paths
            .insert("llvm++musl+musl_libc/arch/x86_64/bits/float.h".to_owned());

        let arch_exec = ensure_execroot(&project_norm, &include_and_arch).unwrap();
        assert_ne!(include_exec, arch_exec);
        assert!(
            arch_exec
                .as_path()
                .join("external/llvm++musl+musl_libc/include/float.h")
                .exists()
        );
        assert!(
            arch_exec
                .as_path()
                .join("external/llvm++musl+musl_libc/arch/x86_64/bits/float.h")
                .exists()
        );
    }

    #[test]
    fn ensure_execroot_filters_buck_out_inputs() {
        reset_cache_for_test();
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path();
        std::fs::create_dir_all(project.join("buck-out/root/gen/pkg")).unwrap();
        std::fs::write(project.join("buck-out/root/gen/pkg/declared.rlib"), b"ok").unwrap();
        std::fs::write(
            project.join("buck-out/root/gen/pkg/undeclared_meta.rlib"),
            b"bad",
        )
        .unwrap();
        std::fs::create_dir_all(project.join("buck-out/root/gen/out")).unwrap();
        let project_norm = AbsNormPathBuf::new(project.to_path_buf()).unwrap();

        let mut plan = ActionExecrootPlan::default();
        plan.buck_out_inputs
            .insert("root/gen/pkg/declared.rlib".to_owned());
        plan.buck_out_writable_dirs
            .insert("root/gen/out".to_owned());

        let exec = ensure_execroot(&project_norm, &plan).unwrap();

        assert!(
            exec.as_path()
                .join("buck-out/root/gen/pkg/declared.rlib")
                .exists()
        );
        assert!(
            !exec
                .as_path()
                .join("buck-out/root/gen/pkg/undeclared_meta.rlib")
                .exists()
        );
        assert!(exec.as_path().join("buck-out/root/gen/out").is_dir());
        assert!(
            !exec
                .as_path()
                .join("buck-out/root/gen/out")
                .symlink_metadata()
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }

    #[test]
    fn ensure_execroot_does_not_link_nested_buck_out_inputs_through_directory_alias() {
        reset_cache_for_test();
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path();
        let sysroot = project.join("buck-out/root/gen/toolchain/sysroot");
        std::fs::create_dir_all(sysroot.join("bin")).unwrap();
        std::fs::write(sysroot.join("bin/rustc"), b"rustc").unwrap();
        let project_norm = AbsNormPathBuf::new(project.to_path_buf()).unwrap();

        let mut plan = ActionExecrootPlan::default();
        plan.buck_out_inputs
            .insert("root/gen/toolchain/sysroot".to_owned());
        plan.buck_out_inputs
            .insert("root/gen/toolchain/sysroot/bin/rustc".to_owned());

        let exec = ensure_execroot(&project_norm, &plan).unwrap();

        let exec_sysroot = exec.as_path().join("buck-out/root/gen/toolchain/sysroot");
        assert!(
            exec_sysroot
                .symlink_metadata()
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(std::fs::read_link(&exec_sysroot).unwrap(), sysroot);
        assert!(
            !std::fs::read_link(sysroot.join("bin/rustc"))
                .map(|target| target == sysroot.join("bin/rustc"))
                .unwrap_or(false)
        );
    }

    #[test]
    fn buck_out_paths_preserve_isolation_dir_component() {
        let mut plan = ActionExecrootPlan::default();

        add_execroot_path(&mut plan, "buck-out/plan61-smoke/gen/pkg/out.txt", true);
        add_execroot_path(&mut plan, "buck-out/plan61-smoke/gen/pkg/input.rlib", false);

        assert!(
            plan.buck_out_writable_dirs
                .contains("plan61-smoke/gen/pkg/out.txt")
        );
        assert!(
            plan.buck_out_inputs
                .contains("plan61-smoke/gen/pkg/input.rlib")
        );
    }

    #[test]
    fn writable_buck_out_parent_does_not_expose_undeclared_siblings() {
        reset_cache_for_test();
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path();
        std::fs::create_dir_all(project.join("buck-out/plan61/gen/pkg")).unwrap();
        std::fs::write(
            project.join("buck-out/plan61/gen/pkg/undeclared.txt"),
            b"bad",
        )
        .unwrap();
        let project_norm = AbsNormPathBuf::new(project.to_path_buf()).unwrap();

        let mut plan = ActionExecrootPlan::default();
        plan.buck_out_writable_dirs
            .insert("plan61/gen/pkg".to_owned());

        let exec = ensure_execroot(&project_norm, &plan).unwrap();

        assert!(exec.as_path().join("buck-out/plan61/gen/pkg").is_dir());
        assert!(
            !exec
                .as_path()
                .join("buck-out/plan61/gen/pkg/undeclared.txt")
                .exists()
        );
    }
}
