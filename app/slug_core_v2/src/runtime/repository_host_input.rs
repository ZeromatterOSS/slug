/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use dice::DiceTransactionUpdater;
use dice::UserComputationData;
use slug_bzlmod_v2::RepositoryEnvironmentCell;
use slug_bzlmod_v2::RepositoryEnvironmentCellKey;
use slug_bzlmod_v2::RepositoryEnvironmentNameFrontier;
use slug_bzlmod_v2::RepositoryEnvironmentSnapshot;
use slug_bzlmod_v2::RepositoryHostInputTransaction;
use slug_bzlmod_v2::RepositoryPlatform;
use slug_bzlmod_v2::RepositoryPlatformKey;
use slug_workspace_v2::NormalizedAbsolutePath;

pub(super) fn initial_repository_environment_frontier(
    snapshot: &RepositoryEnvironmentSnapshot,
    prior: &RepositoryEnvironmentNameFrontier,
) -> RepositoryEnvironmentNameFrontier {
    snapshot.present_name_frontier().union(prior)
}

pub(super) fn install_repository_host_input_transaction(
    data: &mut UserComputationData,
    snapshot: RepositoryEnvironmentSnapshot,
    frontier: RepositoryEnvironmentNameFrontier,
) {
    data.data
        .set(RepositoryHostInputTransaction::new(snapshot, frontier));
}

pub(super) fn inject_repository_host_inputs(
    updater: &mut DiceTransactionUpdater,
    workspace: &NormalizedAbsolutePath,
    platform: RepositoryPlatform,
    snapshot: &RepositoryEnvironmentSnapshot,
    desired_frontier: &RepositoryEnvironmentNameFrontier,
    replaced_frontier: &RepositoryEnvironmentNameFrontier,
) -> Result<(), String> {
    updater
        .changed_to([(RepositoryPlatformKey::new(workspace.clone()), platform)])
        .map_err(|error| format!("injecting repository Host platform: {error}"))?;

    let observed = desired_frontier
        .iter()
        .map(|name| {
            (
                RepositoryEnvironmentCellKey::new(workspace.clone(), name.clone()),
                RepositoryEnvironmentCell::observed(snapshot.get(name).cloned()),
            )
        })
        .collect::<Vec<_>>();
    updater
        .changed_to(observed)
        .map_err(|error| format!("injecting observed repository environment cells: {error}"))?;

    let revoked = replaced_frontier
        .difference(desired_frontier)
        .map(|name| {
            (
                RepositoryEnvironmentCellKey::new(workspace.clone(), name.clone()),
                RepositoryEnvironmentCell::Unauthorized,
            )
        })
        .collect::<Vec<_>>();
    updater
        .changed_to(revoked)
        .map_err(|error| format!("revoking repository environment cells: {error}"))?;
    Ok(())
}
