/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
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
//! 1. A global `SPOKE_REGISTRY` populated when extensions register their
//!    captured `RepoSpec`s. Maps `canonical_name -> (extension_id, RepoSpec,
//!    project_root)`.
//! 2. A thread-local DICE pointer scoped to the duration of an extension's
//!    Starlark eval (`with_extension_dice`).
//! 3. `materialize_spoke_sync()` — synchronous bridge that takes a
//!    canonical name, looks up the spec, and drives DICE materialization
//!    via `tokio::task::block_in_place + Handle::block_on`.
//!
//! The synchronous bridge is the only place we use `unsafe`. It's safe
//! because the pointer's lifetime is strictly bounded by the
//! `with_extension_dice` scope, and the extension Starlark eval is the
//! only thing running on this thread during that scope.

use std::cell::Cell;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;

use dice::DiceComputations;
use fxhash::FxHashMap;

use crate::ExtensionRepoExecutionKey;
use crate::RepoSpec;

/// Information needed to materialize a single spoke repo on demand.
#[derive(Clone)]
pub struct SpokeRegistration {
    pub extension_id: Arc<str>,
    pub repo_spec: Arc<RepoSpec>,
    pub project_root: Arc<PathBuf>,
}

/// Global registry of spoke repos that may need lazy materialization.
///
/// Populated by `kuro_external_cells::extension_repo` after each
/// extension's spec capture loop. Read by `materialize_spoke_sync` when
/// a sibling extension dereferences a spoke via `mctx.path(Label)`.
static SPOKE_REGISTRY: RwLock<Option<FxHashMap<String, SpokeRegistration>>> = RwLock::new(None);

/// Register a spoke for potential lazy materialization.
///
/// Idempotent: repeated registration of the same canonical_name overwrites
/// the previous entry (the latest spec wins, which matches DICE's
/// last-write-wins semantics for the underlying RepoSpec).
pub fn register_spoke(canonical_name: String, registration: SpokeRegistration) {
    let mut guard = SPOKE_REGISTRY.write().unwrap();
    let map = guard.get_or_insert_with(FxHashMap::default);
    map.insert(canonical_name, registration);
}

/// Look up a spoke's materialization spec by canonical name.
pub fn lookup_spoke(canonical_name: &str) -> Option<SpokeRegistration> {
    SPOKE_REGISTRY
        .read()
        .unwrap()
        .as_ref()
        .and_then(|m| m.get(canonical_name).cloned())
}

/// Set of extension IDs whose sibling spokes have already been registered as
/// dynamic cells (or seeded statically from `MODULE.bazel.lock`).
///
/// `kuro_external_cells::extension_repo::get_file_ops_delegate` consults this
/// to decide whether it needs to evaluate the extension via DICE just to
/// discover spoke names. With the lockfile present, startup-time spoke
/// seeding marks every extension as already-seeded so the DICE compute is
/// skipped on warm builds. Without the lockfile, the first file-ops call
/// triggers extension eval (DICE-cached), registers all sibling spokes, and
/// marks the extension here so subsequent calls short-circuit.
static SEEDED_EXTENSIONS: RwLock<Option<std::collections::HashSet<String>>> = RwLock::new(None);

/// Mark `extension_id`'s sibling spokes as already registered. Idempotent.
pub fn mark_extension_spokes_seeded(extension_id: &str) {
    let mut guard = SEEDED_EXTENSIONS.write().unwrap();
    let set = guard.get_or_insert_with(std::collections::HashSet::new);
    set.insert(extension_id.to_owned());
}

/// Returns true if `extension_id`'s sibling spokes are known to be registered.
pub fn extension_spokes_seeded(extension_id: &str) -> bool {
    SEEDED_EXTENSIONS
        .read()
        .unwrap()
        .as_ref()
        .is_some_and(|s| s.contains(extension_id))
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
}

/// Run `f` with a thread-local pointer to the given DICE computations
/// available to nested sync code. Used by extension eval to allow
/// `mctx.path(Label)` etc. to drive lazy spoke materialization through
/// `materialize_spoke_sync`.
///
/// Nesting: the previous pointer (if any) is restored on exit.
pub fn with_extension_dice<R>(ctx: &mut DiceComputations<'_>, f: impl FnOnce() -> R) -> R {
    // Cast away the lifetime. SAFETY: `f` runs synchronously to completion
    // before this function returns; `ctx`'s borrow is live the entire time.
    // We restore the previous pointer on exit so nested scopes work.
    let raw = ctx as *mut DiceComputations<'_> as *mut DiceComputations<'static>;
    let prev = EXTENSION_DICE_PTR.with(|c| c.replace(Some(raw)));
    // Use a guard so we restore on panic too.
    struct Guard(Option<*mut DiceComputations<'static>>);
    impl Drop for Guard {
        fn drop(&mut self) {
            let prev = self.0.take();
            EXTENSION_DICE_PTR.with(|c| c.set(prev));
        }
    }
    let _guard = Guard(prev);
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
pub fn materialize_spoke_sync(canonical_name: &str) -> kuro_error::Result<bool> {
    // Check disk first — common case where sibling already triggered
    // materialization, or eager path materialized at registration time.
    let registration = match lookup_spoke(canonical_name) {
        Some(r) => r,
        None => {
            // Not a known extension spoke. Caller will fall back to its
            // existing Label resolution.
            return Ok(false);
        }
    };
    let marker = registration
        .project_root
        .join("bazel-external")
        .join(canonical_name)
        .join(".kuro_repo_complete");
    if marker.exists() {
        return Ok(true);
    }

    let raw = match EXTENSION_DICE_PTR.with(|c| c.get()) {
        Some(p) => p,
        None => {
            return Err(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Tier0,
                "materialize_spoke_sync called for '{}' outside with_extension_dice scope",
                canonical_name
            ));
        }
    };

    let key = ExtensionRepoExecutionKey::from_arcs(
        Arc::from(canonical_name),
        registration.extension_id.clone(),
        registration.repo_spec.clone(),
        registration.project_root.clone(),
    );

    // Bridge sync -> async. block_in_place releases the current tokio
    // worker so other tasks can make progress while we wait. The nested
    // block_on then drives the DICE compute on this thread.
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            // SAFETY: `with_extension_dice` is active on this call stack;
            // the pointer is valid for the duration of `f` (the eval
            // closure) which encloses this call.
            let ctx: &mut DiceComputations<'_> = unsafe { &mut *raw };
            match ctx.compute(&key).await {
                Ok(Ok(_)) => Ok(true),
                Ok(Err(e)) => Err(e),
                Err(e) => Err(kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Tier0,
                    "DICE compute failed for spoke '{}': {}",
                    canonical_name,
                    e
                )),
            }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_returns_none_for_unknown() {
        assert!(lookup_spoke("definitely_not_registered_xyz_123").is_none());
    }

    #[test]
    fn register_and_lookup_roundtrip() {
        let canonical = "test+ext+roundtrip_spoke".to_owned();
        let spec = RepoSpec::new("@@ext//pkg:file.bzl%test_rule".to_owned());
        register_spoke(
            canonical.clone(),
            SpokeRegistration {
                extension_id: Arc::from("@@ext//pkg:file.bzl%test"),
                repo_spec: Arc::new(spec),
                project_root: Arc::new(PathBuf::from("/tmp")),
            },
        );
        let found = lookup_spoke(&canonical).expect("spoke registered");
        assert_eq!(&*found.extension_id, "@@ext//pkg:file.bzl%test");
    }
}
