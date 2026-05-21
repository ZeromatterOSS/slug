/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Plan 61 bzlmod graph identities and guardrail counters.
//!
//! These types are intentionally inert in 61.1. They name the DICE-owned values
//! the migration will switch to, while the current legacy startup path remains
//! the behavioral authority. Existing code can use the counters immediately to
//! prove when legacy paths compute, replay, or materialize bzlmod state.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use allocative::Allocative;
use sha2::Digest;
use sha2::Sha256;

use crate::resolution::ModuleKey;
use crate::resolution::ModuleSource;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct WorkspaceId {
    pub canonical_project_root: Arc<PathBuf>,
    pub output_base: Arc<PathBuf>,
    pub stable_hash: Arc<str>,
}

impl WorkspaceId {
    pub fn new(canonical_project_root: PathBuf, output_base: PathBuf) -> Self {
        let stable_hash = workspace_hash(&canonical_project_root, &output_base);
        Self {
            canonical_project_root: Arc::new(canonical_project_root),
            output_base: Arc::new(output_base),
            stable_hash: Arc::from(stable_hash.as_str()),
        }
    }

    pub fn stable_hash(&self) -> &str {
        &self.stable_hash
    }
}

fn workspace_hash(canonical_project_root: &Path, output_base: &Path) -> String {
    let mut hasher = Sha256::new();
    hasher.update(canonical_project_root.to_string_lossy().as_bytes());
    hasher.update([0]);
    hasher.update(output_base.to_string_lossy().as_bytes());
    hex::encode(hasher.finalize())
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct BzlmodWorkspaceKey {
    pub canonical_project_root: Arc<PathBuf>,
    pub output_base: Arc<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct BzlmodCommandPolicyKey {
    pub workspace_id: WorkspaceId,
    pub bazel_release_id: Arc<str>,
    pub starlark_semantics_digest: Arc<str>,
    pub bzlmod_flags_digest: Arc<str>,
    pub lockfile_mode: Arc<str>,
    pub registry_config_digest: Arc<str>,
    pub repository_cache_config_digest: Arc<str>,
    pub network_policy_digest: Arc<str>,
    pub repo_env_digest: Arc<str>,
    pub nonstrict_repo_env_digest: Arc<str>,
    pub ignore_dev_dependency: bool,
    pub allow_yanked_versions_digest: Arc<str>,
    pub bazel_compatibility_policy_digest: Arc<str>,
    pub isolated_extension_usages: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct RootModuleFileKey {
    pub workspace_id: WorkspaceId,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct ModuleSourceKey {
    pub workspace_id: WorkspaceId,
    pub command_policy_digest: Arc<str>,
    pub module_key: ModuleKey,
    pub source: ModuleSource,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct ModuleFileKey {
    pub workspace_id: WorkspaceId,
    pub source_digest: Arc<str>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct LocalOverrideSourceKey {
    pub workspace_id: WorkspaceId,
    pub declaring_module: ModuleKey,
    pub override_literal_digest: Arc<str>,
    pub resolved_abs_path: Arc<PathBuf>,
    pub module_file_digest: Arc<str>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub enum LockfileContentKind {
    Workspace,
    Hidden,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct LockfileContentKey {
    pub workspace_id: WorkspaceId,
    pub kind: LockfileContentKind,
    pub path: Arc<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct LockfileExtensionEntryKey {
    pub workspace_id: WorkspaceId,
    pub extension_instance_id: ModuleExtensionId,
    pub lockfile_digest: Arc<str>,
    pub lockfile_mode: Arc<str>,
    pub eval_factors_digest: Arc<str>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct BzlmodResolutionKey {
    pub workspace_id: WorkspaceId,
    pub command_policy_digest: Arc<str>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct ResolvedModuleIdentity {
    pub workspace_id: WorkspaceId,
    pub module_key: ModuleKey,
    pub canonical_repo_name: Arc<str>,
    pub apparent_repo_name: Arc<str>,
    pub module_name: Arc<str>,
    pub version: Arc<str>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct BzlmodCellGraphKey {
    pub workspace_id: WorkspaceId,
    pub resolution_digest: Arc<str>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub enum RepoMappingScope {
    Module(ModuleKey),
    ExtensionImplementation(ModuleExtensionId),
    GeneratedRepo { canonical_repo: Arc<str> },
    InnateRepoRule { invocation_id: Arc<str> },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct RepoMappingKey {
    pub workspace_id: WorkspaceId,
    pub resolution_digest: Arc<str>,
    pub scope: RepoMappingScope,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct RegisteredToolchainsKey {
    pub workspace_id: WorkspaceId,
    pub resolution_digest: Arc<str>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct RegisteredExecutionPlatformsKey {
    pub workspace_id: WorkspaceId,
    pub resolution_digest: Arc<str>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub enum RepoOriginKind {
    RootModule,
    Module(ModuleKey),
    ExtensionGenerated { extension: ModuleExtensionId },
    InnateRepoRule { owner_module: ModuleKey },
    Bundled,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct ModuleExtensionId {
    pub bzl_file_label: Arc<str>,
    pub extension_name: Arc<str>,
    pub isolation_key: Option<Arc<str>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct ExtensionUniqueName {
    pub workspace_id: WorkspaceId,
    pub extension_instance_id: ModuleExtensionId,
    pub unique_name: Arc<str>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct ModuleExtensionAggregationKey {
    pub workspace_id: WorkspaceId,
    pub resolution_digest: Arc<str>,
    pub extension_instance_id: ModuleExtensionId,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct ModuleExtensionReplayInputKey {
    pub workspace_id: WorkspaceId,
    pub extension_instance_id: ModuleExtensionId,
    pub lockfile_entry_digest: Arc<str>,
    pub bzl_transitive_digest: Arc<str>,
    pub usages_digest: Arc<str>,
    pub recorded_inputs_digest: Arc<str>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct ModuleExtensionExecutionIdentity {
    pub workspace_id: WorkspaceId,
    pub extension_instance_id: ModuleExtensionId,
    pub command_policy_digest: Arc<str>,
    pub replay_inputs_digest: Arc<str>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct InnateExtensionKey {
    pub workspace_id: WorkspaceId,
    pub owner_module_key: ModuleKey,
    pub bzl_label: Arc<str>,
    pub rule_name: Arc<str>,
    pub invocation_id: Arc<str>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct ExtensionSpokesKey {
    pub workspace_id: WorkspaceId,
    pub extension_instance_id: ModuleExtensionId,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct ExtensionRepoExecutionIdentity {
    pub workspace_id: WorkspaceId,
    pub canonical_repo: Arc<str>,
    pub repo_spec_digest: Arc<str>,
    pub repo_rule_impl_digest: Arc<str>,
    pub repo_replay_inputs_digest: Arc<str>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct RepoMaterializationManifestKey {
    pub workspace_id: WorkspaceId,
    pub output_base: Arc<PathBuf>,
    pub canonical_repo: Arc<str>,
    pub repo_spec_digest: Arc<str>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct ExternalSymlinkLayoutKey {
    pub workspace_id: WorkspaceId,
    pub output_base: Arc<PathBuf>,
    pub cell_graph_digest: Arc<str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BzlmodEventKind {
    BzlmodResolutionCompute,
    ModuleFileParse,
    ExtensionEval,
    ExtensionReplayHit,
    ExtensionReplayMissReason,
    RepoMaterializationHit,
    RepoMaterializationMissReason,
    LockfileRead,
    LockfileWriteAttempt,
}

impl BzlmodEventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BzlmodResolutionCompute => "bzlmod_resolution_compute",
            Self::ModuleFileParse => "module_file_parse",
            Self::ExtensionEval => "extension_eval",
            Self::ExtensionReplayHit => "extension_replay_hit",
            Self::ExtensionReplayMissReason => "extension_replay_miss_reason",
            Self::RepoMaterializationHit => "repo_materialization_hit",
            Self::RepoMaterializationMissReason => "repo_materialization_miss_reason",
            Self::LockfileRead => "lockfile_read",
            Self::LockfileWriteAttempt => "lockfile_write_attempt",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct BzlmodEventCounters {
    pub bzlmod_resolution_compute: u64,
    pub module_file_parse: u64,
    pub extension_eval: u64,
    pub extension_replay_hit: u64,
    pub extension_replay_miss_reason: u64,
    pub repo_materialization_hit: u64,
    pub repo_materialization_miss_reason: u64,
    pub lockfile_read: u64,
    pub lockfile_write_attempt: u64,
}

static BZLMOD_RESOLUTION_COMPUTE: AtomicU64 = AtomicU64::new(0);
static MODULE_FILE_PARSE: AtomicU64 = AtomicU64::new(0);
static EXTENSION_EVAL: AtomicU64 = AtomicU64::new(0);
static EXTENSION_REPLAY_HIT: AtomicU64 = AtomicU64::new(0);
static EXTENSION_REPLAY_MISS_REASON: AtomicU64 = AtomicU64::new(0);
static REPO_MATERIALIZATION_HIT: AtomicU64 = AtomicU64::new(0);
static REPO_MATERIALIZATION_MISS_REASON: AtomicU64 = AtomicU64::new(0);
static LOCKFILE_READ: AtomicU64 = AtomicU64::new(0);
static LOCKFILE_WRITE_ATTEMPT: AtomicU64 = AtomicU64::new(0);

fn counter(kind: BzlmodEventKind) -> &'static AtomicU64 {
    match kind {
        BzlmodEventKind::BzlmodResolutionCompute => &BZLMOD_RESOLUTION_COMPUTE,
        BzlmodEventKind::ModuleFileParse => &MODULE_FILE_PARSE,
        BzlmodEventKind::ExtensionEval => &EXTENSION_EVAL,
        BzlmodEventKind::ExtensionReplayHit => &EXTENSION_REPLAY_HIT,
        BzlmodEventKind::ExtensionReplayMissReason => &EXTENSION_REPLAY_MISS_REASON,
        BzlmodEventKind::RepoMaterializationHit => &REPO_MATERIALIZATION_HIT,
        BzlmodEventKind::RepoMaterializationMissReason => &REPO_MATERIALIZATION_MISS_REASON,
        BzlmodEventKind::LockfileRead => &LOCKFILE_READ,
        BzlmodEventKind::LockfileWriteAttempt => &LOCKFILE_WRITE_ATTEMPT,
    }
}

pub fn record_bzlmod_event(kind: BzlmodEventKind, detail: impl AsRef<str>) -> u64 {
    let count = counter(kind).fetch_add(1, Ordering::Relaxed) + 1;
    tracing::debug!(
        target: "slug_bzlmod::plan61",
        event_name = kind.as_str(),
        count,
        detail = detail.as_ref(),
        "bzlmod plan61 event"
    );
    count
}

pub fn bzlmod_event_counters() -> BzlmodEventCounters {
    BzlmodEventCounters {
        bzlmod_resolution_compute: BZLMOD_RESOLUTION_COMPUTE.load(Ordering::Relaxed),
        module_file_parse: MODULE_FILE_PARSE.load(Ordering::Relaxed),
        extension_eval: EXTENSION_EVAL.load(Ordering::Relaxed),
        extension_replay_hit: EXTENSION_REPLAY_HIT.load(Ordering::Relaxed),
        extension_replay_miss_reason: EXTENSION_REPLAY_MISS_REASON.load(Ordering::Relaxed),
        repo_materialization_hit: REPO_MATERIALIZATION_HIT.load(Ordering::Relaxed),
        repo_materialization_miss_reason: REPO_MATERIALIZATION_MISS_REASON.load(Ordering::Relaxed),
        lockfile_read: LOCKFILE_READ.load(Ordering::Relaxed),
        lockfile_write_attempt: LOCKFILE_WRITE_ATTEMPT.load(Ordering::Relaxed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_id_distinguishes_project_roots() {
        let first = WorkspaceId::new(PathBuf::from("/tmp/ws-a"), PathBuf::from("/tmp/out"));
        let second = WorkspaceId::new(PathBuf::from("/tmp/ws-b"), PathBuf::from("/tmp/out"));

        assert_ne!(first, second);
        assert_ne!(first.stable_hash(), second.stable_hash());
    }

    #[test]
    fn event_counters_are_addressable_by_plan61_names() {
        let before = bzlmod_event_counters();
        let count = record_bzlmod_event(BzlmodEventKind::ExtensionReplayMissReason, "unit-test");
        let after = bzlmod_event_counters();

        assert!(count > 0);
        assert!(
            after.extension_replay_miss_reason >= before.extension_replay_miss_reason + 1,
            "extension_replay_miss_reason before={} after={}",
            before.extension_replay_miss_reason,
            after.extension_replay_miss_reason
        );
        assert_eq!(
            BzlmodEventKind::ExtensionReplayMissReason.as_str(),
            "extension_replay_miss_reason"
        );
    }

    #[test]
    fn all_plan61_event_counters_are_observable_in_process() {
        type CounterReader = fn(&BzlmodEventCounters) -> u64;

        let cases: &[(BzlmodEventKind, CounterReader, &str)] = &[
            (
                BzlmodEventKind::BzlmodResolutionCompute,
                |c: &BzlmodEventCounters| c.bzlmod_resolution_compute,
                "bzlmod_resolution_compute",
            ),
            (
                BzlmodEventKind::ModuleFileParse,
                |c: &BzlmodEventCounters| c.module_file_parse,
                "module_file_parse",
            ),
            (
                BzlmodEventKind::ExtensionEval,
                |c: &BzlmodEventCounters| c.extension_eval,
                "extension_eval",
            ),
            (
                BzlmodEventKind::ExtensionReplayHit,
                |c: &BzlmodEventCounters| c.extension_replay_hit,
                "extension_replay_hit",
            ),
            (
                BzlmodEventKind::ExtensionReplayMissReason,
                |c: &BzlmodEventCounters| c.extension_replay_miss_reason,
                "extension_replay_miss_reason",
            ),
            (
                BzlmodEventKind::RepoMaterializationHit,
                |c: &BzlmodEventCounters| c.repo_materialization_hit,
                "repo_materialization_hit",
            ),
            (
                BzlmodEventKind::RepoMaterializationMissReason,
                |c: &BzlmodEventCounters| c.repo_materialization_miss_reason,
                "repo_materialization_miss_reason",
            ),
            (
                BzlmodEventKind::LockfileRead,
                |c: &BzlmodEventCounters| c.lockfile_read,
                "lockfile_read",
            ),
            (
                BzlmodEventKind::LockfileWriteAttempt,
                |c: &BzlmodEventCounters| c.lockfile_write_attempt,
                "lockfile_write_attempt",
            ),
        ];

        for &(kind, read_counter, name) in cases {
            let before = read_counter(&bzlmod_event_counters());
            assert_eq!(kind.as_str(), name);
            record_bzlmod_event(kind, "unit-test");
            let after = read_counter(&bzlmod_event_counters());
            assert!(after >= before + 1, "{name}: before={before} after={after}");
        }
    }
}
