/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

#![feature(assert_matches)]
#![feature(error_generic_member_access)]
#![feature(once_cell_try)]

use std::sync::Arc;

use async_trait::async_trait;
use kuro_common::dice::data::HasIoProvider;
use kuro_common::file_ops::delegate::FileOpsDelegate;
use kuro_common::file_ops::metadata::RawPathMetadata;
use kuro_core::cells::cell_root_path::CellRootPath;
use kuro_core::cells::external::ExternalCellOrigin;
use kuro_core::cells::name::CellName;
use kuro_core::fs::project_rel_path::ProjectRelativePathBuf;
use dice::DiceComputations;

mod bundled;
mod bzlmod;
mod git;
mod local;

struct ConcreteExternalCellsImpl;

#[derive(kuro_error::Error, Debug)]
#[kuro(tag = Tier0)]
enum ExternalCellsError {
    #[error("Tried to expand external cell to `{0}`, but that directory already contains data!")]
    ExpandDataAlreadyPresent(ProjectRelativePathBuf),
}

#[async_trait]
impl kuro_common::external_cells::ExternalCellsImpl for ConcreteExternalCellsImpl {
    async fn get_file_ops_delegate(
        &self,
        ctx: &mut DiceComputations<'_>,
        cell_name: CellName,
        origin: ExternalCellOrigin,
    ) -> kuro_error::Result<Arc<dyn FileOpsDelegate>> {
        match origin {
            ExternalCellOrigin::Bundled(cell_name) => {
                Ok(bundled::get_file_ops_delegate(ctx, cell_name).await? as _)
            }
            ExternalCellOrigin::Git(setup) => {
                Ok(git::get_file_ops_delegate(ctx, cell_name, setup).await? as _)
            }
            ExternalCellOrigin::LocalPath(setup) => {
                Ok(local::get_file_ops_delegate(ctx, cell_name, setup).await? as _)
            }
            ExternalCellOrigin::Bzlmod(setup) => {
                Ok(bzlmod::get_file_ops_delegate(ctx, cell_name, setup).await? as _)
            }
        }
    }

    fn check_bundled_cell_exists(&self, cell_name: CellName) -> kuro_error::Result<()> {
        bundled::find_bundled_data(cell_name).map(|_| ())
    }

    async fn expand(
        &self,
        ctx: &mut DiceComputations<'_>,
        cell: CellName,
        origin: ExternalCellOrigin,
        path: &CellRootPath,
    ) -> kuro_error::Result<()> {
        let dest_path = path.as_project_relative_path().to_buf();
        let io = ctx.global_data().get_io_provider();

        // Make sure we're not about to overwrite existing data
        match io.read_path_metadata_if_exists(dest_path.clone()).await? {
            None => (),
            Some(RawPathMetadata::Directory) => {
                let data = io.read_dir(dest_path.clone()).await?;
                if !data.is_empty() {
                    return Err(ExternalCellsError::ExpandDataAlreadyPresent(dest_path).into());
                }
            }
            Some(_) => {
                return Err(ExternalCellsError::ExpandDataAlreadyPresent(dest_path).into());
            }
        }

        // Materialize the whole cell, and then copy it into the repository.
        //
        // FIXME(JakobDegen): Ideally we'd be able to ask the materializer to just make a copy
        // without doing the actual materialization. However, that's not currently possible without
        // it resulting in the materializer tracking paths in the repo, so this will have to do for
        // now.
        match origin {
            ExternalCellOrigin::Bundled(cell) => {
                let materialized_path = bundled::materialize_all(ctx, cell).await?;
                io.project_root().copy(&materialized_path, &dest_path)?;
            }
            ExternalCellOrigin::Git(setup) => {
                let materialized_path = git::materialize_all(ctx, cell, setup).await?;
                io.project_root().copy(&materialized_path, &dest_path)?;
            }
            ExternalCellOrigin::LocalPath(setup) => {
                // Local path cells are already on the filesystem, no materialization needed
                let materialized_path = local::materialize_all(ctx, cell, setup).await?;
                io.project_root().copy(&materialized_path, &dest_path)?;
            }
            ExternalCellOrigin::Bzlmod(setup) => {
                // Bzlmod cells are at absolute cache paths, copy directly from there
                let abs_dest = io.project_root().resolve(&dest_path);
                bzlmod::copy_to_destination(&setup, abs_dest.as_path()).await?;
            }
        }

        Ok(())
    }
}

pub fn init_late_bindings() {
    kuro_common::external_cells::EXTERNAL_CELLS_IMPL.init(&ConcreteExternalCellsImpl);
}
