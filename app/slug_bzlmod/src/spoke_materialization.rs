/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory.
 * You may select, at your option, one of the above-listed licenses.
 */

//! Plan 36: Lazy materialization of extension spoke repos triggered from
//! sibling extensions' `module_ctx.path(Label)` / `module_ctx.read(Label)`
//! calls.
//!
//! When extension A (e.g. `rules_rs::toolchains`) declares spoke repos via
//! `cargo_repository(name = "cargo_linux_x86_64_1_95_0", ...)`, those specs
//! are captured but the repos are NOT materialized. When extension B (e.g.
//! `rules_rs::crate`) later runs and calls
//! `mctx.path(Label("@cargo_linux_x86_64_1_95_0//:bin/cargo"))`, the path
//! resolves to a directory that doesn't exist on disk yet — and the next
//! `mctx.execute([cargo_path, ...])` fails with "No such file or directory".
//!
//! This module provides:
//!
//! 1. A thread-local DICE pointer scoped to the duration of an extension's
//!    Starlark eval (`with_extension_dice`).
//! 2. `materialize_spoke_sync()` — synchronous bridge that takes a
//!    canonical name, finds the owning extension result through DICE, and
//!    drives DICE materialization via
//!    `tokio::task::block_in_place + Handle::block_on`.
//!
//! The synchronous bridge is the only place we use `unsafe`. It's safe
//! because the pointer's lifetime is strictly bounded by the
//! `with_extension_dice` scope, and the extension Starlark eval is the
//! only thing running on this thread during that scope.

use std::cell::Cell;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;

use dice::DiceComputations;

use crate::ExtensionRepoExecutionKey;
use crate::RepoSpec;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LazyRepoRuleMaterialization {
    pub extension_id: Arc<str>,
    pub repo_spec_json: Arc<str>,
}

// ============================================================================
// Thread-local DICE pointer for sync->async bridging during extension eval
// ============================================================================

thread_local! {
    /// Raw pointer to the `&mut DiceComputations<'_>` borrowed by the
    /// currently-executing extension. `None` outside `with_extension_dice`
    /// scopes.
    ///
    /// SAFETY contract: writers must clear before exiting the borrow's
    /// scope. Readers must only deref while a `with_extension_dice`
    /// activation is on the call stack of the same thread.
    static EXTENSION_DICE_PTR: Cell<Option<*mut DiceComputations<'static>>> = const { Cell::new(None) };

    /// Workspace identity paired with `EXTENSION_DICE_PTR`. Synchronous
    /// spoke materialization uses it to construct DICE keys without reading
    /// injected bzlmod projection data in the materialization bridge itself.
    static EXTENSION_WORKSPACE_ID: RefCell<Option<crate::WorkspaceId>> = RefCell::new(None);

    /// Runtime extension-repo setups for direct `use_repo_rule()` cells. These
    /// repos are not sibling spokes of a module extension, but `module_ctx`
    /// label dereferences still need to materialize them before returning a
    /// raw filesystem path.
    static EXTENSION_REPO_RULE_SETUPS: RefCell<Option<BTreeMap<String, LazyRepoRuleMaterialization>>> = const { RefCell::new(None) };
}

/// Run `f` with a thread-local pointer to the given DICE computations
/// available to nested sync code. Used by extension eval to allow
/// `mctx.path(Label)` etc. to drive lazy spoke materialization through
/// `materialize_spoke_sync`.
///
/// Nesting: the previous pointer (if any) is restored on exit.
pub fn with_extension_dice<R>(
    ctx: &mut DiceComputations<'_>,
    workspace_id: crate::WorkspaceId,
    f: impl FnOnce() -> R,
) -> R {
    with_extension_dice_and_repo_rules(ctx, workspace_id, BTreeMap::new(), f)
}

pub fn with_extension_dice_and_repo_rules<R>(
    ctx: &mut DiceComputations<'_>,
    workspace_id: crate::WorkspaceId,
    repo_rule_setups: BTreeMap<String, LazyRepoRuleMaterialization>,
    f: impl FnOnce() -> R,
) -> R {
    // Cast away the lifetime. SAFETY: `f` runs synchronously to completion
    // before this function returns; `ctx`'s borrow is live the entire time.
    // We restore the previous pointer on exit so nested scopes work.
    let raw = ctx as *mut DiceComputations<'_> as *mut DiceComputations<'static>;
    let prev = EXTENSION_DICE_PTR.with(|c| c.replace(Some(raw)));
    let prev_workspace = EXTENSION_WORKSPACE_ID.with(|c| c.replace(Some(workspace_id)));
    let prev_repo_rule_setups =
        EXTENSION_REPO_RULE_SETUPS.with(|c| c.replace(Some(repo_rule_setups)));
    // Use a guard so we restore on panic too.
    struct Guard {
        prev: Option<*mut DiceComputations<'static>>,
        prev_workspace: Option<crate::WorkspaceId>,
        prev_repo_rule_setups: Option<BTreeMap<String, LazyRepoRuleMaterialization>>,
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            let prev = self.prev.take();
            EXTENSION_DICE_PTR.with(|c| c.set(prev));
            let prev_workspace = self.prev_workspace.take();
            EXTENSION_WORKSPACE_ID.with(|c| {
                c.replace(prev_workspace);
            });
            let prev_repo_rule_setups = self.prev_repo_rule_setups.take();
            EXTENSION_REPO_RULE_SETUPS.with(|c| {
                c.replace(prev_repo_rule_setups);
            });
        }
    }
    let _guard = Guard {
        prev,
        prev_workspace,
        prev_repo_rule_setups,
    };
    f()
}

/// Synchronously materialize the spoke repo named `canonical_name` by
/// driving its `ExtensionRepoExecutionKey` through DICE.
///
/// Returns `Ok(())` if the spoke is already on disk, was successfully
/// materialized, or no registration exists (caller decides whether
/// missing-registration is an error). Returns the underlying error if
/// materialization fails.
///
/// Must be called from inside a `with_extension_dice` scope on a tokio
/// runtime worker thread.
pub fn materialize_spoke_sync(canonical_name: &str) -> slug_error::Result<bool> {
    let raw = match EXTENSION_DICE_PTR.with(|c| c.get()) {
        Some(p) => p,
        None => {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Tier0,
                "materialize_spoke_sync called for '{}' outside with_extension_dice scope",
                canonical_name
            ));
        }
    };

    // Check if the spoke is already on disk. If so, skip DICE entirely —
    // no need to materialize what's already there. This avoids DICE
    // dependency cycles that cause deadlocks when a spoke's DICE key
    // transitively depends on the currently-running computation.
    let spoke_dir = EXTENSION_WORKSPACE_ID.with(|c| {
        c.borrow().as_ref().map(|ws| {
            ws.canonical_project_root
                .join("bazel-external")
                .join(canonical_name)
        })
    });
    if let Some(ref dir) = spoke_dir {
        if dir.is_dir() {
            return Ok(true);
        }
    }

    // Spoke is not on disk — attempt DICE materialization with a timeout
    // to avoid deadlocking the runtime.
    tokio::task::block_in_place(|| {
        let result = tokio::runtime::Handle::current().block_on(async {
            let ctx: &mut DiceComputations<'_> = unsafe { &mut *raw };
            let Some(key) = spoke_execution_key(ctx, canonical_name).await? else {
                return Ok(false);
            };
            // Timeout to prevent deadlock: if the spoke's DICE computation
            // depends on a key held by the current computation, it will
            // never complete. Return false after the timeout so the build
            // can continue; the spoke will be materialized on retry.
            let compute_future = ctx.compute(&key);
            match tokio::time::timeout(std::time::Duration::from_secs(30), compute_future).await {
                Ok(Ok(Ok(_))) => Ok(true),
                Ok(Ok(Err(e))) => Err(e),
                Ok(Err(e)) => Err(slug_error::slug_error!(
                    slug_error::ErrorTag::Tier0,
                    "DICE compute failed for spoke '{}': {}",
                    canonical_name,
                    e
                )),
                Err(_) => {
                    tracing::warn!(
                        "materialize_spoke_sync: timed out waiting for DICE \
                         computation of '{}', skipping (potential deadlock avoided)",
                        canonical_name
                    );
                    Ok(false)
                }
            }
        });
        // Handle::block_on returns the future's Output directly.
        result
    })
}

async fn spoke_execution_key(
    ctx: &mut DiceComputations<'_>,
    canonical_name: &str,
) -> slug_error::Result<Option<ExtensionRepoExecutionKey>> {
    let Some(_) = crate::parse_canonical_name(canonical_name) else {
        return Ok(None);
    };
    let workspace_id = EXTENSION_WORKSPACE_ID
        .with(|c| c.borrow().clone())
        .ok_or_else(|| {
            slug_error::slug_error!(
                slug_error::ErrorTag::Tier0,
                "materialize_spoke_sync called for '{}' without workspace identity",
                canonical_name
            )
        })?;
    let resolution_digest =
        crate::bzlmod_resolution_digest_for_workspace_id(ctx, workspace_id.clone()).await?;
    let lookup_key =
        crate::ExtensionSpokesByCanonicalRepoKey::for_workspace_id_with_resolution_digest(
            workspace_id.clone(),
            resolution_digest,
            canonical_name,
        );
    let spokes = match ctx.compute(&lookup_key).await {
        Ok(Ok(Some(spokes))) => spokes,
        Ok(Ok(None)) => return repo_rule_execution_key(ctx, workspace_id, canonical_name).await,
        Ok(Err(e)) => return Err(e),
        Err(e) => {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Tier0,
                "DICE compute failed for extension spokes while looking up '{}': {}",
                canonical_name,
                e
            ));
        }
    };
    if let Some(spoke) = spokes.by_canonical_or_internal_name(canonical_name) {
        let repo_mappings =
            crate::bzlmod_repo_mappings_for_workspace_id(ctx, workspace_id.clone()).await?;
        let repo_mappings = merged_repo_mappings(
            repo_mappings.repo_mappings.as_ref(),
            spokes.recorded_input_repo_mappings.as_ref(),
        );
        return Ok(Some(
            ExtensionRepoExecutionKey::from_arcs_with_workspace_id_repo_env_and_repo_mappings(
                spoke.canonical_name.clone(),
                spokes.extension_id.clone(),
                spoke.repo_spec.clone(),
                spokes.workspace_id.clone(),
                spokes.repo_env.clone(),
                std::sync::Arc::new(repo_mappings),
            ),
        ));
    }

    Ok(None)
}

async fn repo_rule_execution_key(
    ctx: &mut DiceComputations<'_>,
    workspace_id: crate::WorkspaceId,
    canonical_name: &str,
) -> slug_error::Result<Option<ExtensionRepoExecutionKey>> {
    let setup = EXTENSION_REPO_RULE_SETUPS.with(|c| {
        c.borrow()
            .as_ref()
            .and_then(|setups| setups.get(canonical_name).cloned())
    });
    let Some(setup) = setup else {
        return Ok(None);
    };
    if setup.repo_spec_json.is_empty() {
        return Ok(None);
    }
    let repo_spec = serde_json::from_str::<RepoSpec>(&setup.repo_spec_json).map_err(|e| {
        slug_error::slug_error!(
            slug_error::ErrorTag::Input,
            "Failed to deserialize RepoSpec for direct repo-rule repo '{}': {}",
            canonical_name,
            e
        )
    })?;
    let repo_env = crate::bzlmod_repo_env_for_workspace_id(ctx, workspace_id.clone()).await?;
    let repo_mappings = crate::bzlmod_repo_mappings_for_workspace_id(ctx, workspace_id.clone())
        .await?
        .repo_mappings
        .clone();
    Ok(Some(
        ExtensionRepoExecutionKey::new_with_workspace_id_repo_env_and_repo_mappings(
            canonical_name.to_owned(),
            setup.extension_id.to_string(),
            repo_spec,
            workspace_id,
            repo_env,
            repo_mappings,
        ),
    ))
}

fn merged_repo_mappings(
    graph_mappings: &crate::RepoMappingSnapshot,
    recorded_mappings: &crate::RepoMappingSnapshot,
) -> crate::RepoMappingSnapshot {
    let mut merged = graph_mappings.clone();
    for (source_repo, mapping) in recorded_mappings {
        merged
            .entry(source_repo.clone())
            .or_default()
            .extend(mapping.clone());
    }
    merged
}

#[cfg(test)]
mod tests {
    fn complete_marker(spec_hash: &str, output_digest: Option<&str>) -> String {
        match (spec_hash.is_empty(), output_digest) {
            (true, None) => "complete".to_owned(),
            (true, Some(output_digest)) => format!("complete:output:{output_digest}"),
            (false, Some(output_digest)) => {
                format!("complete:{spec_hash}:output:{output_digest}")
            }
            (false, None) => format!("complete:{spec_hash}"),
        }
    }

    #[test]
    fn complete_marker_includes_output_digest_when_available() {
        assert_eq!(complete_marker("", None), "complete");
        assert_eq!(
            complete_marker("", Some("sha256-out")),
            "complete:output:sha256-out"
        );
        assert_eq!(
            complete_marker("sha256-spec", Some("sha256-out")),
            "complete:sha256-spec:output:sha256-out"
        );
        assert_eq!(complete_marker("sha256-spec", None), "complete:sha256-spec");
    }

    #[test]
    fn spoke_marker_changes_when_output_changes() {
        let temp = tempfile::TempDir::new().unwrap();
        let repo_dir = temp.path().join("bazel-external/repo");
        std::fs::create_dir_all(&repo_dir).unwrap();
        std::fs::write(repo_dir.join("data.txt"), "fresh").unwrap();
        let digest = crate::repository_executor::repository_output_digest(&repo_dir).unwrap();
        let marker = complete_marker("sha256-spec", Some(&digest));

        std::fs::write(repo_dir.join(".slug_repo_complete"), &marker).unwrap();
        std::fs::write(repo_dir.join("data.txt"), "corrupt").unwrap();
        let changed_digest =
            crate::repository_executor::repository_output_digest(&repo_dir).unwrap();

        assert_ne!(digest, changed_digest);
        assert_ne!(
            marker,
            complete_marker("sha256-spec", Some(&changed_digest))
        );
    }
}
