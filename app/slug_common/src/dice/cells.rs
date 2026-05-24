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

use std::collections::HashMap;

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
use slug_core::cells::CellAliasResolver;
use slug_core::cells::CellResolver;
use slug_core::cells::name::CellName;
use slug_core::fs::project_rel_path::ProjectRelativePath;

use crate::legacy_configs::cells::BuckConfigBasedCells;
use crate::legacy_configs::dice::HasLegacyConfigs;

#[async_trait]
pub trait HasCellResolver {
    async fn get_cell_resolver(&mut self) -> slug_error::Result<CellResolver>;

    async fn is_cell_resolver_key_set(&mut self) -> slug_error::Result<bool>;

    async fn get_cell_alias_resolver(
        &mut self,
        cell: CellName,
    ) -> slug_error::Result<CellAliasResolver>;

    async fn get_cell_alias_resolver_for_dir(
        &mut self,
        dir: &ProjectRelativePath,
    ) -> slug_error::Result<CellAliasResolver>;

    /// Returns true if the project uses bzlmod (MODULE.bazel present).
    async fn is_bzlmod(&mut self) -> slug_error::Result<bool>;
}

pub trait SetCellResolver {
    fn set_cell_resolver(&mut self, cell_resolver: CellResolver) -> slug_error::Result<()>;

    fn set_none_cell_resolver(&mut self) -> slug_error::Result<()>;

    /// Set whether the project uses bzlmod (MODULE.bazel present).
    fn set_is_bzlmod(&mut self, is_bzlmod: bool) -> slug_error::Result<()>;
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
    async fn get_cell_resolver(&mut self) -> slug_error::Result<CellResolver> {
        self.compute(&CellResolverKey).await?.ok_or_else(|| {
            panic!("Tried to retrieve CellResolverKey from the graph, but key has None value")
        })
    }

    async fn is_cell_resolver_key_set(&mut self) -> slug_error::Result<bool> {
        Ok(self.compute(&CellResolverKey).await?.is_some())
    }

    async fn get_cell_alias_resolver(
        &mut self,
        cell: CellName,
    ) -> slug_error::Result<CellAliasResolver> {
        Ok(self.compute(&CellAliasResolverKey(cell)).await??)
    }

    async fn get_cell_alias_resolver_for_dir(
        &mut self,
        dir: &ProjectRelativePath,
    ) -> slug_error::Result<CellAliasResolver> {
        let cell = self.get_cell_resolver().await?.find(dir);
        self.get_cell_alias_resolver(cell).await
    }

    async fn is_bzlmod(&mut self) -> slug_error::Result<bool> {
        Ok(self.compute(&IsBzlmodKey).await?)
    }
}

/// Only used for cell alias resolvers parsed within dice, currently those for external cells
#[derive(Clone, Dupe, Display, Debug, Eq, Hash, PartialEq, Allocative)]
struct CellAliasResolverKey(CellName);

#[async_trait]
impl Key for CellAliasResolverKey {
    type Value = slug_error::Result<CellAliasResolver>;

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
            // sections, and root MODULE.bazel apparent names must not leak
            // into other modules. Keep canonical cell-name aliases available
            // for Slug's current cell model; module-scoped apparent names are
            // resolved by the temporary scoped bzlmod alias adapter.
            if self.0 == root_aliases.resolve_self() {
                return Ok(root_aliases.dupe());
            }
            let canonical_aliases: HashMap<_, _> = root_aliases
                .mappings()
                .filter(|(alias, name)| alias.as_str() == name.as_str())
                .collect();
            return CellAliasResolver::new_bzlmod_for_non_root_cell(
                self.0,
                root_aliases,
                canonical_aliases,
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
                    slug_core::cells::alias::NonEmptyCellAlias,
                    slug_core::cells::alias::NonEmptyCellAlias,
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
    fn set_cell_resolver(&mut self, cell_resolver: CellResolver) -> slug_error::Result<()> {
        Ok(self.changed_to(vec![(CellResolverKey, Some(cell_resolver))])?)
    }

    fn set_none_cell_resolver(&mut self) -> slug_error::Result<()> {
        Ok(self.changed_to(vec![(CellResolverKey, None)])?)
    }

    fn set_is_bzlmod(&mut self, is_bzlmod: bool) -> slug_error::Result<()> {
        Ok(self.changed_to(vec![(IsBzlmodKey, is_bzlmod)])?)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use dice::UserComputationData;
    use dice::testing::DiceBuilder;
    use slug_core::cells::BzlmodRuntimeCellInstallSnapshot;
    use slug_core::cells::BzlmodRuntimeDynamicAlias;
    use slug_core::cells::BzlmodRuntimeExtensionCell;
    use slug_core::cells::alias::NonEmptyCellAlias;
    use slug_core::cells::cell_root_path::CellRootPathBuf;
    use slug_core::cells::external::ExtensionRepoCellSetup;
    use slug_core::cells::instance::CellInstance;
    use slug_core::cells::name::CellName;
    use slug_core::cells::nested::NestedCells;
    use slug_core::cells::register_dynamic_extension_cell_alias;
    use slug_core::cells::reset_dynamic_bzlmod_state_for_project_root;

    use super::*;

    #[tokio::test]
    async fn bzlmod_non_root_alias_resolver_preserves_runtime_snapshot() -> slug_error::Result<()> {
        let tmp = tempfile::tempdir()?;
        reset_dynamic_bzlmod_state_for_project_root(tmp.path().to_path_buf());

        let root = CellName::testing_new("root");
        let dep = CellName::testing_new("dep+1.0");
        let root_path = CellRootPathBuf::testing_new("");
        let dep_path = CellRootPathBuf::testing_new("bazel-external/dep+1.0");
        let cell_roots = [(root, root_path.as_path()), (dep, dep_path.as_path())];
        let root_instance = CellInstance::new(
            root,
            root_path.clone(),
            None,
            NestedCells::from_cell_roots(&cell_roots, &root_path),
        )?;
        let dep_instance = CellInstance::new(
            dep,
            dep_path.clone(),
            None,
            NestedCells::from_cell_roots(&cell_roots, &dep_path),
        )?;

        let canonical = "owner++ext+generated";
        let stale_global = "stale_owner++ext+generated";
        register_dynamic_extension_cell_alias("stale_alias".to_owned(), stale_global.to_owned());
        let setup = ExtensionRepoCellSetup {
            canonical_name: Arc::from(canonical),
            extension_id: Arc::from("@owner//:ext.bzl%ext"),
            internal_name: Arc::from("generated"),
            spec_hash: Arc::from("sha256-test"),
            repo_spec_json: Arc::from("{}"),
            repo_env_json: Arc::from("{}"),
            materialized: false,
        };
        let snapshot = BzlmodRuntimeCellInstallSnapshot {
            extension_cells: vec![BzlmodRuntimeExtensionCell {
                canonical_name: canonical.to_owned(),
                internal_name: "generated".to_owned(),
                path: format!("bazel-external/{canonical}"),
                setup,
            }],
            scoped_aliases: Vec::new(),
            dynamic_aliases: vec![BzlmodRuntimeDynamicAlias {
                apparent_name: "runtime_alias".to_owned(),
                canonical_name: canonical.to_owned(),
            }],
        };
        let mut root_aliases = HashMap::new();
        root_aliases.insert(NonEmptyCellAlias::new("root_dep".to_owned())?, dep);
        root_aliases.insert(NonEmptyCellAlias::new(dep.as_str().to_owned())?, dep);
        let root_alias_resolver = CellAliasResolver::new_bzlmod_with_runtime_cell_snapshot(
            root,
            root_aliases,
            &snapshot,
        )?;
        let resolver = CellResolver::new_bzlmod_with_runtime_cell_snapshot(
            vec![root_instance, dep_instance],
            root_alias_resolver,
            snapshot,
        )?;

        let dice = DiceBuilder::new()
            .build(UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        updater.set_cell_resolver(resolver)?;
        updater.set_is_bzlmod(true)?;
        let mut dice = updater.commit().await;

        let aliases = dice.get_cell_alias_resolver(dep).await?;

        assert!(aliases.has_bzlmod_runtime_alias_snapshot());
        assert_eq!(
            aliases.resolve("runtime_alias")?,
            CellName::testing_new(canonical)
        );
        assert!(aliases.resolve("stale_alias").is_err());
        assert!(aliases.resolve("root_dep").is_err());
        assert_eq!(aliases.resolve(dep.as_str())?, dep);
        Ok(())
    }
}
