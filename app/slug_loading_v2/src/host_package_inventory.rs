/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the above-listed
 * licenses.
 */

//! Repository-aware composition of the accepted loading package owners.

use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use dice::CancellationContext;
use dice::DiceComputations;
use dice::Key;
use dupe::Dupe;
use slug_bzlmod_v2::HostRepositorySourceRoute;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_identity_v2::PackageIdentifier;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathObservationEpoch;

use crate::HostCanonicalRepositoryLoadRoute;
use crate::HostCanonicalRepositoryLoadRouteError;
use crate::HostCanonicalRepositoryLoadRouteKey;
use crate::HostCanonicalRepositoryLoadRouteObservationError;
use crate::HostCanonicalRepositoryLoadRouteObservationKey;
use crate::LoadedPackage;
use crate::RepositoryPackageLoadError;
use crate::RootPackageLoadError;
use crate::RootPackageLoadKey;
use crate::RootPackageLoadObservationKey;
use crate::bzl_module::RepositoryPackageInventoryKey;
use crate::bzl_module::RepositoryPackageInventoryObservationKey;

type RootResult = Arc<Result<LoadedPackage, RootPackageLoadError>>;
type CanonicalRouteResult =
    Arc<Result<HostCanonicalRepositoryLoadRoute, HostCanonicalRepositoryLoadRouteError>>;
type CanonicalResult = Arc<Result<LoadedPackage, RepositoryPackageLoadError>>;

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct HostPackageInventory(HostPackageInventoryTerminal);

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
enum HostPackageInventoryTerminal {
    Root(RootResult),
    CanonicalRoute(CanonicalRouteResult),
    Canonical(CanonicalResult),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostPackageInventoryErrorRef<'a> {
    Root(&'a RootPackageLoadError),
    CanonicalRoute(&'a HostCanonicalRepositoryLoadRouteError),
    Canonical(&'a RepositoryPackageLoadError),
}

impl HostPackageInventory {
    fn root(result: RootResult) -> Self {
        Self(HostPackageInventoryTerminal::Root(result))
    }

    fn canonical_route(result: CanonicalRouteResult) -> Self {
        debug_assert!(result.is_err());
        Self(HostPackageInventoryTerminal::CanonicalRoute(result))
    }

    fn canonical(result: CanonicalResult) -> Self {
        Self(HostPackageInventoryTerminal::Canonical(result))
    }

    pub fn loaded(&self) -> Result<&LoadedPackage, HostPackageInventoryErrorRef<'_>> {
        match &self.0 {
            HostPackageInventoryTerminal::Root(result) => result
                .as_ref()
                .as_ref()
                .map_err(HostPackageInventoryErrorRef::Root),
            HostPackageInventoryTerminal::CanonicalRoute(result) => {
                Err(HostPackageInventoryErrorRef::CanonicalRoute(
                    result
                        .as_ref()
                        .as_ref()
                        .expect_err("private constructor accepts only route errors"),
                ))
            }
            HostPackageInventoryTerminal::Canonical(result) => result
                .as_ref()
                .as_ref()
                .map_err(HostPackageInventoryErrorRef::Canonical),
        }
    }

    #[doc(hidden)]
    pub fn root_result(&self) -> Option<&Arc<Result<LoadedPackage, RootPackageLoadError>>> {
        match &self.0 {
            HostPackageInventoryTerminal::Root(result) => Some(result),
            _ => None,
        }
    }

    #[doc(hidden)]
    pub fn canonical_route_result(
        &self,
    ) -> Option<&Arc<Result<HostCanonicalRepositoryLoadRoute, HostCanonicalRepositoryLoadRouteError>>>
    {
        match &self.0 {
            HostPackageInventoryTerminal::CanonicalRoute(result) => Some(result),
            _ => None,
        }
    }

    #[doc(hidden)]
    pub fn canonical_result(
        &self,
    ) -> Option<&Arc<Result<LoadedPackage, RepositoryPackageLoadError>>> {
        match &self.0 {
            HostPackageInventoryTerminal::Canonical(result) => Some(result),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostPackageInventoryKey {
    workspace: NormalizedAbsolutePath,
    package: PackageIdentifier,
}

impl HostPackageInventoryKey {
    pub fn new(workspace: NormalizedAbsolutePath, package: PackageIdentifier) -> Self {
        Self { workspace, package }
    }

    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        &self.workspace
    }

    pub fn package(&self) -> &PackageIdentifier {
        &self.package
    }
}

impl fmt::Display for HostPackageInventoryKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-package-inventory:{}:{}",
            self.workspace, self.package
        )
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct HostPackageInventoryObservationKey(HostPackageInventoryKey);

impl HostPackageInventoryObservationKey {
    pub fn new(workspace: NormalizedAbsolutePath, package: PackageIdentifier) -> Self {
        Self(HostPackageInventoryKey::new(workspace, package))
    }

    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        self.0.workspace()
    }

    pub fn package(&self) -> &PackageIdentifier {
        self.0.package()
    }
}

impl fmt::Display for HostPackageInventoryObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedHostPackageInventory {
    result: Arc<HostPackageInventory>,
    observations: PathObservationEpoch,
}

impl ObservedHostPackageInventory {
    pub fn result(&self) -> &Arc<HostPackageInventory> {
        &self.result
    }

    pub fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum HostPackageInventoryObservationError {
    CanonicalRoute(HostCanonicalRepositoryLoadRouteObservationError),
    Frontier(ObservedPathFrontierError),
}

impl fmt::Display for HostPackageInventoryObservationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for HostPackageInventoryObservationError {}

#[derive(Debug, Clone, Copy)]
enum InventoryMode {
    Legacy,
    Observed,
}

type DriverOutcome = SourcePreparationOutcome<
    Result<(Arc<HostPackageInventory>, PathObservationEpoch), HostPackageInventoryObservationError>,
>;

fn merge_observations(
    route: &PathObservationEpoch,
    package: &PathObservationEpoch,
) -> Result<PathObservationEpoch, HostPackageInventoryObservationError> {
    PathObservationEpoch::from_shared(
        route
            .observations()
            .iter()
            .chain(package.observations())
            .map(|(demand, result)| (demand.dupe(), result.dupe())),
    )
    .map_err(|error| HostPackageInventoryObservationError::Frontier(error.into()))
}

async fn root_inventory(
    ctx: &mut DiceComputations<'_>,
    key: &HostPackageInventoryKey,
    mode: InventoryMode,
) -> DriverOutcome {
    match mode {
        InventoryMode::Legacy => match ctx
            .compute(&RootPackageLoadKey::new(
                key.workspace.dupe(),
                key.package.package().clone(),
            ))
            .await
            .expect("root package inventory DICE invariant")
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(result) => SourcePreparationOutcome::Complete(Ok((
                Arc::new(HostPackageInventory::root(result)),
                PathObservationEpoch::empty(),
            ))),
        },
        InventoryMode::Observed => match ctx
            .compute(&RootPackageLoadObservationKey::new(
                key.workspace.dupe(),
                key.package.package().clone(),
            ))
            .await
            .expect("observed root package inventory DICE invariant")
        {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => SourcePreparationOutcome::Complete(
                Err(HostPackageInventoryObservationError::Frontier(error)),
            ),
            SourcePreparationOutcome::Complete(Ok(observed)) => {
                SourcePreparationOutcome::Complete(Ok((
                    Arc::new(HostPackageInventory::root(observed.result().dupe())),
                    observed.observations().dupe(),
                )))
            }
        },
    }
}

async fn canonical_inventory(
    ctx: &mut DiceComputations<'_>,
    key: &HostPackageInventoryKey,
    mode: InventoryMode,
) -> DriverOutcome {
    match mode {
        InventoryMode::Legacy => canonical_legacy(ctx, key).await,
        InventoryMode::Observed => canonical_observed(ctx, key).await,
    }
}

async fn canonical_legacy(
    ctx: &mut DiceComputations<'_>,
    key: &HostPackageInventoryKey,
) -> DriverOutcome {
    let route = match ctx
        .compute(&HostCanonicalRepositoryLoadRouteKey::new(
            key.workspace.dupe(),
            key.package.repo().clone(),
        ))
        .await
        .expect("canonical package route DICE invariant")
    {
        SourcePreparationOutcome::Need(need) => return SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(route) => route,
    };
    let input = match route.as_ref() {
        Ok(route) => route.input().clone(),
        Err(_) => {
            return SourcePreparationOutcome::Complete(Ok((
                Arc::new(HostPackageInventory::canonical_route(route)),
                PathObservationEpoch::empty(),
            )));
        }
    };
    match ctx
        .compute(&RepositoryPackageInventoryKey::new(
            HostRepositorySourceRoute::canonical(input),
            key.package.package().clone(),
        ))
        .await
        .expect("canonical package inventory DICE invariant")
    {
        SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(result) => SourcePreparationOutcome::Complete(Ok((
            Arc::new(HostPackageInventory::canonical(result)),
            PathObservationEpoch::empty(),
        ))),
    }
}

async fn canonical_observed(
    ctx: &mut DiceComputations<'_>,
    key: &HostPackageInventoryKey,
) -> DriverOutcome {
    let observed_route = match ctx
        .compute(&HostCanonicalRepositoryLoadRouteObservationKey::new(
            key.workspace.dupe(),
            key.package.repo().clone(),
        ))
        .await
        .expect("observed canonical package route DICE invariant")
    {
        SourcePreparationOutcome::Need(need) => return SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Err(error)) => {
            return SourcePreparationOutcome::Complete(Err(
                HostPackageInventoryObservationError::CanonicalRoute(error),
            ));
        }
        SourcePreparationOutcome::Complete(Ok(observed)) => observed,
    };
    let input = match observed_route.result().as_ref() {
        Ok(route) => route.input().clone(),
        Err(_) => {
            return SourcePreparationOutcome::Complete(Ok((
                Arc::new(HostPackageInventory::canonical_route(
                    observed_route.result().dupe(),
                )),
                observed_route.observations().dupe(),
            )));
        }
    };
    match ctx
        .compute(&RepositoryPackageInventoryObservationKey::new(
            HostRepositorySourceRoute::canonical(input),
            key.package.package().clone(),
        ))
        .await
        .expect("observed canonical package inventory DICE invariant")
    {
        SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Err(error)) => SourcePreparationOutcome::Complete(Err(
            HostPackageInventoryObservationError::Frontier(error),
        )),
        SourcePreparationOutcome::Complete(Ok(observed_package)) => {
            match merge_observations(
                observed_route.observations(),
                observed_package.observations(),
            ) {
                Ok(observations) => SourcePreparationOutcome::Complete(Ok((
                    Arc::new(HostPackageInventory::canonical(
                        observed_package.result().dupe(),
                    )),
                    observations,
                ))),
                Err(error) => SourcePreparationOutcome::Complete(Err(error)),
            }
        }
    }
}

async fn drive_inventory(
    ctx: &mut DiceComputations<'_>,
    key: &HostPackageInventoryKey,
    mode: InventoryMode,
) -> DriverOutcome {
    if key.package.repo().is_root() {
        root_inventory(ctx, key, mode).await
    } else {
        canonical_inventory(ctx, key, mode).await
    }
}

#[async_trait]
impl Key for HostPackageInventoryKey {
    type Value = SourcePreparationOutcome<Arc<HostPackageInventory>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        match drive_inventory(ctx, self, InventoryMode::Legacy).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                debug_assert!(observations.observations().is_empty());
                SourcePreparationOutcome::Complete(result)
            }
            SourcePreparationOutcome::Complete(Err(error)) => {
                panic!("legacy package inventory produced observed outer error: {error}")
            }
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[async_trait]
impl Key for HostPackageInventoryObservationKey {
    type Value = SourcePreparationOutcome<
        Result<ObservedHostPackageInventory, HostPackageInventoryObservationError>,
    >;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        match drive_inventory(ctx, &self.0, InventoryMode::Observed).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Err(error)) => {
                SourcePreparationOutcome::Complete(Err(error))
            }
            SourcePreparationOutcome::Complete(Ok((result, observations))) => {
                SourcePreparationOutcome::Complete(Ok(ObservedHostPackageInventory {
                    result,
                    observations,
                }))
            }
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}
