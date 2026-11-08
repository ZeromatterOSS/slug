/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select the license that applies to you.
 */

use slug_workspace_v2::AnchoredPathResolutionError;
use slug_workspace_v2::AnchoredResolvedPathObservationKey;
use slug_workspace_v2::MaterializationPathAnchor;

use super::*;

// Generic repository routes must preserve the anchor produced by materialization.
// In particular, resolving the root must never demand its Host-side ancestors.
pub(super) async fn resolve(
    ctx: &mut DiceComputations<'_>,
    mode: HostRepositoryObservationMode,
    namespace: PathObservationNamespace,
    requested_path: NormalizedAbsolutePath,
    repo_relative_path: Arc<PathBuf>,
    anchor: &MaterializationPathAnchor,
) -> HostRepositoryPathDriverOutcome {
    let checked = if anchor.namespace() == namespace {
        AnchoredResolvedPathObservationKey::new(anchor.dupe(), requested_path.as_path()).ok()
    } else {
        None
    };
    let Some(checked) = checked else {
        return host_repository_complete(
            Err(RepositorySourceFileError::InvalidMaterializedPath { repo_relative_path }),
            PathObservationEpoch::empty(),
        );
    };
    match ctx.compute(&checked).await {
        Ok(PathOutcome::Need(need)) => SourcePreparationOutcome::path_need(need),
        Ok(PathOutcome::Complete(Err(error))) => match mode {
            HostRepositoryObservationMode::Legacy => host_repository_complete(
                Err(RepositorySourceFileError::ResolutionCompute {
                    repo_relative_path,
                    message: Arc::from(error.to_string()),
                }),
                PathObservationEpoch::empty(),
            ),
            HostRepositoryObservationMode::Observed => {
                SourcePreparationOutcome::Complete(Err(error))
            }
        },
        Ok(PathOutcome::Complete(Ok(observed))) => {
            let result = match observed.result() {
                Ok(resolved) => Ok(HostRepositoryPathValue(resolved.dupe())),
                Err(AnchoredPathResolutionError::Resolution(error)) => {
                    Err(project_resolution_error(repo_relative_path, error.clone()))
                }
                Err(AnchoredPathResolutionError::EscapesAnchor { .. }) => {
                    Err(RepositorySourceFileError::InvalidMaterializedPath { repo_relative_path })
                }
            };
            let observations = match mode {
                HostRepositoryObservationMode::Legacy => PathObservationEpoch::empty(),
                HostRepositoryObservationMode::Observed => observed.observations().dupe(),
            };
            host_repository_complete(result, observations)
        }
        Err(error) => host_repository_complete(
            Err(RepositorySourceFileError::ResolutionCompute {
                repo_relative_path,
                message: Arc::from(error.to_string()),
            }),
            PathObservationEpoch::empty(),
        ),
    }
}
