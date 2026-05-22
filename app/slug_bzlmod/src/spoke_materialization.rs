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

use dice::DiceComputations;

use crate::ExtensionRepoExecutionKey;

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

    // Bridge sync -> async. block_in_place releases the current tokio
    // worker so other tasks can make progress while we wait. The nested
    // block_on then drives the DICE compute on this thread.
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            // SAFETY: `with_extension_dice` is active on this call stack;
            // the pointer is valid for the duration of `f` (the eval
            // closure) which encloses this call.
            let ctx: &mut DiceComputations<'_> = unsafe { &mut *raw };
            let Some(key) = spoke_execution_key(ctx, canonical_name).await? else {
                return Ok(false);
            };
            match ctx.compute(&key).await {
                Ok(Ok(_)) => Ok(true),
                Ok(Err(e)) => Err(e),
                Err(e) => Err(slug_error::slug_error!(
                    slug_error::ErrorTag::Tier0,
                    "DICE compute failed for spoke '{}': {}",
                    canonical_name,
                    e
                )),
            }
        })
    })
}

async fn spoke_execution_key(
    ctx: &mut DiceComputations<'_>,
    canonical_name: &str,
) -> slug_error::Result<Option<ExtensionRepoExecutionKey>> {
    let Some(_) = crate::parse_canonical_name(canonical_name) else {
        return Ok(None);
    };
    let session_data = ctx
        .compute(&crate::BzlmodSessionDataKey)
        .await
        .map_err(|e| {
            slug_error::slug_error!(
                slug_error::ErrorTag::Tier0,
                "DICE compute failed while looking up spoke '{}': {}",
                canonical_name,
                e
            )
        })?;
    let Some(spokes_key) =
        crate::extension_spokes_key_for_canonical_repo(&session_data, canonical_name)
    else {
        return Ok(None);
    };
    let spokes = match ctx.compute(&spokes_key).await {
        Ok(Ok(spokes)) => spokes,
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
        return Ok(Some(ExtensionRepoExecutionKey::from_arcs(
            spoke.canonical_name.clone(),
            spokes.extension_id.clone(),
            spoke.repo_spec.clone(),
            spokes.project_root.clone(),
        )));
    }

    Ok(None)
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
