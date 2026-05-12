/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use slug_core::cells::cell_path::CellPath;
use slug_core::cells::cell_path_with_allowed_relative_dir::CellPathWithAllowedRelativeDir;

#[async_trait]
pub trait HasAllowRelativePaths {
    async fn dirs_allowing_relative_paths(
        &mut self,
        cell_path: CellPath,
    ) -> slug_error::Result<Arc<CellPathWithAllowedRelativeDir>>;
}

#[async_trait]
impl HasAllowRelativePaths for DiceComputations<'_> {
    async fn dirs_allowing_relative_paths(
        &mut self,
        cell_path: CellPath,
    ) -> slug_error::Result<Arc<CellPathWithAllowedRelativeDir>> {
        #[derive(Debug, Eq, PartialEq, Hash, Clone, derive_more::Display, Allocative)]
        #[display("{}", cell_path)]
        struct AllowRelativePathsKey {
            cell_path: CellPath,
        }

        #[async_trait]
        impl Key for AllowRelativePathsKey {
            type Value = slug_error::Result<Arc<CellPathWithAllowedRelativeDir>>;

            async fn compute(
                &self,
                _ctx: &mut DiceComputations,
                _cancellation: &CancellationContext,
            ) -> Self::Value {
                Ok(Arc::new(CellPathWithAllowedRelativeDir::new(
                    self.cell_path.clone(),
                    None,
                )))
            }

            fn equality(x: &Self::Value, y: &Self::Value) -> bool {
                match (x, y) {
                    (Ok(x), Ok(y)) => x == y,
                    _ => false,
                }
            }
        }

        self.compute(&AllowRelativePathsKey { cell_path }).await?
    }
}
