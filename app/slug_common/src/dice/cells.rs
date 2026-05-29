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
use slug_core::cells::alias::NonEmptyCellAlias;
use slug_core::cells::external::ExternalCellOrigin;
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

async fn bzlmod_extension_spoke_aliases(
    ctx: &mut DiceComputations<'_>,
    resolver: &CellResolver,
    current: CellName,
) -> slug_error::Result<Vec<(NonEmptyCellAlias, CellName)>> {
    let Ok(instance) = resolver.get(current) else {
        return Ok(Vec::new());
    };
    let Some(ExternalCellOrigin::ExtensionRepo(setup)) = instance.external() else {
        return Ok(Vec::new());
    };
    let extension_name = slug_bzlmod::extract_extension_name(setup.extension_id.as_ref());
    if setup.internal_name.as_ref() != extension_name
        && setup.internal_name.as_ref() != format!("{extension_name}s")
    {
        return Ok(Vec::new());
    }
    let workspace_id = slug_bzlmod::bzlmod_workspace_id_for_current_workspace(ctx).await?;
    let resolution_digest =
        slug_bzlmod::bzlmod_resolution_digest_for_workspace_id(ctx, workspace_id.clone()).await?;
    let aggregation = ctx
        .compute(
            &slug_bzlmod::BzlmodExtensionAggregationKey::for_workspace_id_with_resolution_digest(
                workspace_id.clone(),
                resolution_digest.clone(),
                setup.extension_id.as_ref(),
            ),
        )
        .await??;
    let Some(aggregation) = aggregation else {
        return Ok(Vec::new());
    };
    if slug_bzlmod::extract_owning_module(
        setup.extension_id.as_ref(),
        aggregation.root_module_name.as_ref(),
    ) == "_main"
    {
        return Ok(Vec::new());
    }
    let spokes = ctx
        .compute(
            &slug_bzlmod::ExtensionSpokesByExtensionIdKey::for_workspace_id_with_resolution_digest(
                workspace_id,
                resolution_digest,
                setup.extension_id.as_ref(),
            ),
        )
        .await??;
    let Some(spokes) = spokes else {
        return Ok(Vec::new());
    };

    let mut aliases = Vec::new();
    for spoke in spokes.iter() {
        if spoke.internal_name == spoke.canonical_name {
            continue;
        }
        if !spoke.internal_name.contains("__") {
            continue;
        }
        let Ok(alias) = NonEmptyCellAlias::new(spoke.internal_name.to_string()) else {
            continue;
        };
        let Ok(canonical) = CellName::unchecked_new(spoke.canonical_name.as_ref()) else {
            continue;
        };
        aliases.push((alias, canonical));
    }
    Ok(aliases)
}

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
            let current = resolver
                .get(self.0)
                .map(|cell| cell.name())
                .ok()
                .or_else(|| root_aliases.resolve_declared_or_runtime_alias(self.0.as_str()))
                .unwrap_or(self.0);
            let mut canonical_aliases: HashMap<_, _> = if matches!(
                current.as_str(),
                "bazel_tools" | "slug_builtins" | "local_config_platform" | "local_config_python"
            ) {
                root_aliases.mappings().collect()
            } else {
                root_aliases
                    .mappings()
                    .filter(|(alias, name)| alias.as_str() == name.as_str())
                    .collect()
            };
            canonical_aliases
                .extend(resolver.bzlmod_same_extension_internal_aliases(current.as_str()));
            canonical_aliases.extend(
                bzlmod_extension_spoke_aliases(ctx, &resolver, current)
                    .await?
                    .into_iter(),
            );
            return CellAliasResolver::new_bzlmod_for_non_root_cell(
                current,
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
    use slug_bzlmod::SetBzlmodDiceInputs;
    use slug_bzlmod::WorkspaceId;
    use slug_core::cells::BzlmodRuntimeCellInstallSnapshot;
    use slug_core::cells::BzlmodRuntimeDynamicAlias;
    use slug_core::cells::BzlmodRuntimeExtensionCell;
    use slug_core::cells::alias::NonEmptyCellAlias;
    use slug_core::cells::cell_root_path::CellRootPathBuf;
    use slug_core::cells::external::ExtensionRepoCellSetup;
    use slug_core::cells::instance::CellInstance;
    use slug_core::cells::name::CellName;
    use slug_core::cells::nested::NestedCells;
    use slug_core::cells::register_test_dynamic_extension_cell_alias;
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
        register_test_dynamic_extension_cell_alias(
            "stale_alias".to_owned(),
            stale_global.to_owned(),
        );
        let setup = ExtensionRepoCellSetup {
            canonical_name: Arc::from(canonical),
            extension_id: Arc::from("@owner//:ext.bzl%ext"),
            internal_name: Arc::from("generated"),
            spec_hash: Arc::from("sha256-test"),
            repo_spec_json: Arc::from("{}"),
            repo_env_json: Arc::from("{}"),
            extension_usages_digest: Arc::from(""),
            extension_replay_inputs_identity_digest: Arc::from(""),
            extension_repo_mappings_digest: Arc::from(""),
            extension_repo_mapping_overrides_digest: Arc::from(""),
            extension_bzl_transitive_digest: Arc::from(""),
            extension_recorded_inputs_json: Arc::from(""),
            materialized: false,
        };
        let snapshot = BzlmodRuntimeCellInstallSnapshot {
            root_module_name: None,
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
        let sibling_canonical = "owner++ext+sibling";
        let sibling_setup = ExtensionRepoCellSetup {
            canonical_name: Arc::from(sibling_canonical),
            extension_id: Arc::from("@owner//:ext.bzl%ext"),
            internal_name: Arc::from("sibling"),
            spec_hash: Arc::from("sha256-sibling"),
            repo_spec_json: Arc::from("{}"),
            repo_env_json: Arc::from("{}"),
            extension_usages_digest: Arc::from(""),
            extension_replay_inputs_identity_digest: Arc::from(""),
            extension_repo_mappings_digest: Arc::from(""),
            extension_repo_mapping_overrides_digest: Arc::from(""),
            extension_bzl_transitive_digest: Arc::from(""),
            extension_recorded_inputs_json: Arc::from(""),
            materialized: false,
        };
        resolver.register_bzlmod_runtime_extension_cell(
            sibling_canonical,
            &format!("bazel-external/{sibling_canonical}"),
            sibling_setup,
        )?;

        let dice = DiceBuilder::new()
            .build(UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        updater.set_cell_resolver(resolver)?;
        updater.set_is_bzlmod(true)?;
        updater.set_empty_bzlmod_dice_inputs_for_workspace(WorkspaceId::new(
            tmp.path().to_path_buf(),
            tmp.path().join("buck-out"),
        ))?;
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

        let apparent_aliases = dice
            .get_cell_alias_resolver(CellName::testing_new("runtime_alias"))
            .await?;
        assert_eq!(
            apparent_aliases.resolve_self(),
            CellName::testing_new(canonical)
        );
        assert_eq!(
            apparent_aliases.resolve("owner")?,
            CellName::testing_new("owner+")
        );
        assert_eq!(
            apparent_aliases.resolve("sibling")?,
            CellName::testing_new(sibling_canonical)
        );
        Ok(())
    }

    #[tokio::test]
    async fn bzlmod_bundled_tool_alias_resolver_can_see_root_repo_aliases() -> slug_error::Result<()>
    {
        let root = CellName::testing_new("root");
        let bazel_tools = CellName::testing_new("bazel_tools");
        let rules_cc = CellName::testing_new("rules_cc+");
        let root_path = CellRootPathBuf::testing_new("");
        let bazel_tools_path = CellRootPathBuf::testing_new("bazel_tools");
        let rules_cc_path = CellRootPathBuf::testing_new("bazel-external/rules_cc+");
        let cell_roots = [
            (root, root_path.as_path()),
            (bazel_tools, bazel_tools_path.as_path()),
            (rules_cc, rules_cc_path.as_path()),
        ];
        let root_instance = CellInstance::new(
            root,
            root_path.clone(),
            None,
            NestedCells::from_cell_roots(&cell_roots, &root_path),
        )?;
        let bazel_tools_instance = CellInstance::new(
            bazel_tools,
            bazel_tools_path.clone(),
            None,
            NestedCells::from_cell_roots(&cell_roots, &bazel_tools_path),
        )?;
        let rules_cc_instance = CellInstance::new(
            rules_cc,
            rules_cc_path.clone(),
            None,
            NestedCells::from_cell_roots(&cell_roots, &rules_cc_path),
        )?;
        let snapshot = BzlmodRuntimeCellInstallSnapshot::default();
        let mut root_aliases = HashMap::new();
        root_aliases.insert(NonEmptyCellAlias::new("rules_cc".to_owned())?, rules_cc);
        let root_alias_resolver = CellAliasResolver::new_bzlmod_with_runtime_cell_snapshot(
            root,
            root_aliases,
            &snapshot,
        )?;
        let resolver = CellResolver::new_bzlmod_with_runtime_cell_snapshot(
            vec![root_instance, bazel_tools_instance, rules_cc_instance],
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

        let aliases = dice.get_cell_alias_resolver(bazel_tools).await?;
        assert_eq!(aliases.resolve("rules_cc")?, rules_cc);

        Ok(())
    }
}
