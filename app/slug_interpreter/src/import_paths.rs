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
use dupe::Dupe;
use pagable::Pagable;
use slug_common::dice::cells::HasCellResolver;
use slug_common::legacy_configs::configs::LegacyBuckConfig;
use slug_common::legacy_configs::dice::HasLegacyConfigs;
use slug_common::legacy_configs::view::LegacyBuckConfigView;
use slug_core::bzl::ImportPath;
use slug_core::cells::CellAliasResolver;
use slug_core::cells::build_file_cell::BuildFileCell;

use crate::package_imports::PackageImplicitImports;

#[derive(PartialEq, Allocative, Pagable)]
pub struct ImplicitImportPaths {
    pub root_import: Option<ImportPath>,
    pub package_imports: PackageImplicitImports,
}

impl ImplicitImportPaths {
    pub fn parse(
        config: impl LegacyBuckConfigView,
        cell_name: BuildFileCell,
        cell_alias_resolver: &CellAliasResolver,
    ) -> slug_error::Result<ImplicitImportPaths> {
        let _ = config;
        let package_imports =
            PackageImplicitImports::new(cell_name, cell_alias_resolver.dupe(), None)?;

        Ok(ImplicitImportPaths {
            root_import: None,
            package_imports,
        })
    }

    pub fn root_import(&self) -> Option<&ImportPath> {
        self.root_import.as_ref()
    }
}

#[async_trait]
pub trait HasImportPaths {
    async fn import_paths_for_cell(
        &mut self,
        cell_name: BuildFileCell,
    ) -> slug_error::Result<Arc<ImplicitImportPaths>>;
}

#[async_trait]
impl HasImportPaths for DiceComputations<'_> {
    async fn import_paths_for_cell(
        &mut self,
        cell_name: BuildFileCell,
    ) -> slug_error::Result<Arc<ImplicitImportPaths>> {
        #[derive(Debug, Eq, PartialEq, Hash, Clone, derive_more::Display, Allocative)]
        #[display("{}", cell_name)]
        struct ImportPathsKey {
            cell_name: BuildFileCell,
        }

        #[async_trait]
        impl Key for ImportPathsKey {
            type Value = slug_error::Result<Arc<ImplicitImportPaths>>;

            async fn compute(
                &self,
                ctx: &mut DiceComputations,
                _cancellation: &CancellationContext,
            ) -> Self::Value {
                let cell_alias_resolver =
                    ctx.get_cell_alias_resolver(self.cell_name.name()).await?;

                if ctx.is_bzlmod().await? {
                    // Bazel 9/Bzlmod has no legacy package implicit import
                    // configuration. Avoid retaining one buckconfig DICE node
                    // per external repo solely to pass an ignored config view.
                    let config = LegacyBuckConfig::empty();
                    Ok(Arc::new(ImplicitImportPaths::parse(
                        &config,
                        self.cell_name,
                        &cell_alias_resolver,
                    )?))
                } else {
                    let config = ctx.get_legacy_config_on_dice(self.cell_name.name()).await?;
                    Ok(Arc::new(ImplicitImportPaths::parse(
                        config.view(ctx),
                        self.cell_name,
                        &cell_alias_resolver,
                    )?))
                }
            }

            fn equality(x: &Self::Value, y: &Self::Value) -> bool {
                match (x, y) {
                    (Ok(x), Ok(y)) => x == y,
                    _ => false,
                }
            }
        }

        self.compute(&ImportPathsKey { cell_name }).await?
    }
}
