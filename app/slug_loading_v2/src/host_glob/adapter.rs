/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory or the Apache License, Version 2.0
 * found in the LICENSE-APACHE file in the root directory. You may select,
 * at your option, one of the above-listed licenses.
 */

use std::sync::Arc;

use allocative::Allocative;
use dice::DiceComputations;
use dupe::Dupe;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_identity_v2::PackagePath;
use slug_workspace_v2::NormalizedAbsolutePath;

use super::dice_invariant;
use super::traversal::HostGlobPattern;
use super::traversal::HostGlobPatternError;
use super::traversal::HostGlobTraversalError;
use super::traversal::HostGlobTraversalKey;
use super::traversal::HostGlobTraversalKeyError;
use super::traversal::HostGlobTraversalOperation;
use super::traversal::HostGlobTraversalOutcome;

/// Callable-facing operation retained in one prepared-request identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative, Dupe)]
pub(crate) enum HostGlobLoadingOperation {
    Files,
    FilesAndDirs,
}

/// Exact raw pattern and operation requested by one BUILD `glob()` attempt.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub(crate) struct HostGlobLoadingRequest {
    pattern: Arc<[u8]>,
    operation: HostGlobLoadingOperation,
}

impl HostGlobLoadingRequest {
    pub(crate) fn new(pattern: impl Into<Arc<[u8]>>, operation: HostGlobLoadingOperation) -> Self {
        Self {
            pattern: pattern.into(),
            operation,
        }
    }
}

/// Input rejected before the adapter creates or computes a traversal key.
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(super) enum HostGlobLoadingInputError {
    Pattern(HostGlobPatternError),
    Key(HostGlobTraversalKeyError),
}

/// Ordered package-relative raw paths prepared for a future loading consumer.
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(super) struct HostGlobLoadingMatches {
    paths: Arc<[Arc<[u8]>]>,
}

impl HostGlobLoadingMatches {
    pub(super) fn paths(&self) -> &[Arc<[u8]>] {
        &self.paths
    }
}

pub(super) type HostGlobLoadingOutcome =
    SourcePreparationOutcome<Arc<Result<HostGlobLoadingMatches, HostGlobTraversalError>>>;

/// Opaque crate-local preservation of request-construction diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) struct HostGlobRequestInputError(HostGlobLoadingInputError);

/// Opaque crate-local preservation of a complete traversal diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct HostGlobRequestTraversalError(HostGlobTraversalError);

/// Raw, sorted package-relative matches prepared for one request.
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) struct HostGlobRequestMatches(HostGlobLoadingMatches);

impl HostGlobRequestMatches {
    pub(crate) fn paths(&self) -> &[Arc<[u8]>] {
        self.0.paths()
    }
}

pub(crate) type HostGlobPrepared =
    Arc<Result<HostGlobRequestMatches, HostGlobRequestTraversalError>>;
pub(crate) type HostGlobRequestOutcome = SourcePreparationOutcome<HostGlobPrepared>;

fn project_traversal_outcome(outcome: HostGlobTraversalOutcome) -> HostGlobLoadingOutcome {
    outcome.map(|value| {
        Arc::new(match value.as_ref() {
            Ok(traversal) => Ok(HostGlobLoadingMatches {
                paths: traversal
                    .matches()
                    .iter()
                    .map(|entry| entry.relative_path.dupe())
                    .collect::<Vec<_>>()
                    .into(),
            }),
            Err(error) => Err(error.clone()),
        })
    })
}

/// Compute one checked Host glob through the caller-owned DICE transaction.
///
/// This is deliberately below callable semantics: it has one pattern, one
/// operation, and no include/exclude or `allow_empty` policy.
pub(super) async fn compute_host_glob_for_loading(
    ctx: &mut DiceComputations<'_>,
    workspace: NormalizedAbsolutePath,
    logical_package_root: NormalizedAbsolutePath,
    package: PackagePath,
    pattern: Arc<[u8]>,
    operation: HostGlobTraversalOperation,
) -> Result<HostGlobLoadingOutcome, HostGlobLoadingInputError> {
    let pattern = HostGlobPattern::new(pattern).map_err(HostGlobLoadingInputError::Pattern)?;
    let key =
        HostGlobTraversalKey::new(workspace, logical_package_root, package, pattern, operation)
            .map_err(HostGlobLoadingInputError::Key)?;
    Ok(project_traversal_outcome(dice_invariant(
        ctx.compute(&key).await,
    )))
}

/// Compute one shared callable request through the caller's DICE transaction.
pub(crate) async fn compute_host_glob_request(
    ctx: &mut DiceComputations<'_>,
    workspace: NormalizedAbsolutePath,
    logical_package_root: NormalizedAbsolutePath,
    package: PackagePath,
    request: HostGlobLoadingRequest,
) -> Result<HostGlobRequestOutcome, HostGlobRequestInputError> {
    let operation = match request.operation {
        HostGlobLoadingOperation::Files => HostGlobTraversalOperation::Files,
        HostGlobLoadingOperation::FilesAndDirs => HostGlobTraversalOperation::FilesAndDirs,
    };
    compute_host_glob_for_loading(
        ctx,
        workspace,
        logical_package_root,
        package,
        request.pattern,
        operation,
    )
    .await
    .map(|outcome| {
        outcome.map(|value| {
            Arc::new(match value.as_ref() {
                Ok(matches) => Ok(HostGlobRequestMatches(matches.dupe())),
                Err(error) => Err(HostGlobRequestTraversalError(error.clone())),
            })
        })
    })
    .map_err(HostGlobRequestInputError)
}

#[cfg(all(test, unix))]
#[path = "adapter_tests.rs"]
mod adapter_tests;
