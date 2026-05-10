/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Core dice computations relating to cells

use allocative::Allocative;
use async_trait::async_trait;
use derive_more::Display;
use dice::CancellationContext;
use dice::DiceComputations;
use dice::DiceTransactionUpdater;
use dice::InjectedKey;
use dice::InvalidationSourcePriority;
use dice::Key;
use dupe::Dupe;
use kuro_core::cells::CellAliasResolver;
use kuro_core::cells::CellResolver;
use kuro_core::cells::name::CellName;
use kuro_core::fs::project_rel_path::ProjectRelativePath;

use crate::legacy_configs::cells::BuckConfigBasedCells;
use crate::legacy_configs::dice::HasLegacyConfigs;

#[async_trait]
pub trait HasCellResolver {
    async fn get_cell_resolver(&mut self) -> kuro_error::Result<CellResolver>;

    async fn is_cell_resolver_key_set(&mut self) -> kuro_error::Result<bool>;

    async fn get_cell_alias_resolver(
        &mut self,
        cell: CellName,
    ) -> kuro_error::Result<CellAliasResolver>;

    async fn get_cell_alias_resolver_for_dir(
        &mut self,
        dir: &ProjectRelativePath,
    ) -> kuro_error::Result<CellAliasResolver>;

    /// Returns true if the project uses bzlmod (MODULE.bazel present).
    async fn is_bzlmod(&mut self) -> kuro_error::Result<bool>;
}

pub trait SetCellResolver {
    fn set_cell_resolver(&mut self, cell_resolver: CellResolver) -> kuro_error::Result<()>;

    fn set_none_cell_resolver(&mut self) -> kuro_error::Result<()>;

    /// Set whether the project uses bzlmod (MODULE.bazel present).
    fn set_is_bzlmod(&mut self, is_bzlmod: bool) -> kuro_error::Result<()>;
}

#[derive(Clone, Dupe, Display, Debug, Eq, Hash, PartialEq, Allocative)]
#[display("{:?}", self)]
struct CellResolverKey;

impl InjectedKey for CellResolverKey {
    type Value = Option<CellResolver>;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Some(x), Some(y)) => x == y,
            (None, None) => true,
            (_, _) => false,
        }
    }

    fn invalidation_source_priority() -> InvalidationSourcePriority {
        InvalidationSourcePriority::Ignored
    }
}

#[derive(Clone, Dupe, Display, Debug, Eq, Hash, PartialEq, Allocative)]
#[display("{:?}", self)]
struct IsBzlmodKey;

impl InjectedKey for IsBzlmodKey {
    type Value = bool;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    fn invalidation_source_priority() -> InvalidationSourcePriority {
        InvalidationSourcePriority::Ignored
    }
}

#[async_trait]
impl HasCellResolver for DiceComputations<'_> {
    async fn get_cell_resolver(&mut self) -> kuro_error::Result<CellResolver> {
        self.compute(&CellResolverKey).await?.ok_or_else(|| {
            panic!("Tried to retrieve CellResolverKey from the graph, but key has None value")
        })
    }

    async fn is_cell_resolver_key_set(&mut self) -> kuro_error::Result<bool> {
        Ok(self.compute(&CellResolverKey).await?.is_some())
    }

    async fn get_cell_alias_resolver(
        &mut self,
        cell: CellName,
    ) -> kuro_error::Result<CellAliasResolver> {
        Ok(self.compute(&CellAliasResolverKey(cell)).await??)
    }

    async fn get_cell_alias_resolver_for_dir(
        &mut self,
        dir: &ProjectRelativePath,
    ) -> kuro_error::Result<CellAliasResolver> {
        let cell = self.get_cell_resolver().await?.find(dir);
        self.get_cell_alias_resolver(cell).await
    }

    async fn is_bzlmod(&mut self) -> kuro_error::Result<bool> {
        Ok(self.compute(&IsBzlmodKey).await?)
    }
}

/// Only used for cell alias resolvers parsed within dice, currently those for external cells
#[derive(Clone, Dupe, Display, Debug, Eq, Hash, PartialEq, Allocative)]
struct CellAliasResolverKey(CellName);

#[async_trait]
impl Key for CellAliasResolverKey {
    type Value = kuro_error::Result<CellAliasResolver>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let resolver = ctx.get_cell_resolver().await?;
        let root_aliases = resolver.root_cell_cell_alias_resolver();
        let is_bzlmod = ctx.is_bzlmod().await?;

        if is_bzlmod {
            // Bazel 9/Bzlmod does not read per-repository .buckconfig alias
            // sections. Keep the per-cell current-cell resolver, but do not
            // materialize one LegacyBuckConfigForCellKey per external repo.
            return CellAliasResolver::new_for_non_root_cell(
                self.0,
                root_aliases,
                std::iter::empty(),
            )
            .map_err(Into::into);
        }

        let config = ctx.get_legacy_config_for_cell(self.0).await?;

        // Cell alias resolvers that are parsed within dice differ from those outside of dice in
        // that they cannot create new cells, and so respect only their `cell_aliases` section, not
        // their `cells` section. This is the expected behavior for external cells, moving other
        // cell resolver parsing into dice would require this code to be adjusted.
        let cell_aliases: Box<
            dyn Iterator<
                Item = (
                    kuro_core::cells::alias::NonEmptyCellAlias,
                    kuro_core::cells::alias::NonEmptyCellAlias,
                ),
            >,
        > = Box::new(BuckConfigBasedCells::get_cell_aliases_from_config(&config)?);

        CellAliasResolver::new_for_non_root_cell(self.0, root_aliases, cell_aliases)
            .map_err(Into::into)
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            (_, _) => false,
        }
    }
}

impl SetCellResolver for DiceTransactionUpdater {
    fn set_cell_resolver(&mut self, cell_resolver: CellResolver) -> kuro_error::Result<()> {
        Ok(self.changed_to(vec![(CellResolverKey, Some(cell_resolver))])?)
    }

    fn set_none_cell_resolver(&mut self) -> kuro_error::Result<()> {
        Ok(self.changed_to(vec![(CellResolverKey, None)])?)
    }

    fn set_is_bzlmod(&mut self, is_bzlmod: bool) -> kuro_error::Result<()> {
        Ok(self.changed_to(vec![(IsBzlmodKey, is_bzlmod)])?)
    }
}
