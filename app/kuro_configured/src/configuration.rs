/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use allocative::Allocative;
use async_trait::async_trait;
use derive_more::Display;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use futures::FutureExt;
use kuro_build_api::analysis::calculation::RuleAnalysisCalculation;
use kuro_build_api::interpreter::rule_defs::provider::builtin::configuration_info::FrozenConfigurationInfo;
use kuro_build_api::interpreter::rule_defs::provider::builtin::platform_info::FrozenPlatformInfo;
use kuro_build_api::interpreter::rule_defs::provider::collection::FrozenProviderCollectionValue;
use kuro_common::dice::cells::HasCellResolver;
use kuro_common::legacy_configs::configs::parse_config_section_and_key;
use kuro_common::legacy_configs::dice::HasLegacyConfigs;
use kuro_common::legacy_configs::key::BuckconfigKeyRef;
use kuro_core::configuration::config_setting::ConfigSettingData;
use kuro_core::configuration::data::ConfigurationData;
use kuro_core::configuration::pair::ConfigurationNoExec;
use kuro_core::provider::label::ProvidersLabel;
use kuro_core::target::label::label::TargetLabel;
use kuro_error::BuckErrorContext;
use kuro_node::attrs::attr_type::configuration_dep::ConfigurationDepKind;
use kuro_node::configuration::calculation::CONFIGURATION_CALCULATION;
use kuro_node::configuration::calculation::CellNameForConfigurationResolution;
use kuro_node::configuration::calculation::ConfigurationCalculationDyn;
use kuro_node::configuration::resolved::ConfigurationNode;
use kuro_node::configuration::resolved::ConfigurationSettingKey;
use kuro_node::configuration::resolved::MatchedConfigurationSettingKeys;
use kuro_node::configuration::resolved::MatchedConfigurationSettingKeysWithCfg;
use kuro_node::nodes::frontend::TargetGraphCalculation;
use kuro_node::nodes::unconfigured::TargetNodeRef;
use kuro_node::rule_type::NativeRuleKind;
use kuro_node::rule_type::RuleType;
use ref_cast::RefCast;
use starlark_map::ordered_map::OrderedMap;
use starlark_map::unordered_map::UnorderedMap;

#[derive(Debug, kuro_error::Error)]
#[kuro(input)]
pub enum ConfigurationError {
    #[error(
        "`{0}` target doesn't have a `ConfigurationInfo` provider so it can't be selected. Possible selectable rules are `config_setting` and `constraint_value`."
    )]
    MissingConfigurationInfoProvider(ProvidersLabel),
    #[error("Expected `{0}` to be a `platform()` target, but it had no `PlatformInfo` provider.")]
    MissingPlatformInfo(TargetLabel),
    #[error(
        "Platform target `{0}` evaluation returned `ProviderInfo` label `{1}` which resolved to an unequal configuration"
    )]
    PlatformEvalUnequalConfiguration(TargetLabel, TargetLabel),
    #[error(
        "Expected `{0}` to be a `constraint_setting()` target, but it had no `ConstraintSettingInfo` provider."
    )]
    MissingConstraintSettingInfo(TargetLabel),
}

async fn configuration_matches(
    ctx: &mut DiceComputations<'_>,
    cfg: &ConfigurationData,
    target_node_cell: CellNameForConfigurationResolution,
    constraints_and_configs: &ConfigSettingData,
) -> kuro_error::Result<bool> {
    for (key, value) in &constraints_and_configs.constraints {
        // For builtin/unspecified platforms, no constraints match.
        // This causes select() to use the default/fallback case.
        let constraint_val = match cfg.get_constraint_value(key) {
            Err(_) => return Ok(false),
            Ok(v) => v,
        };
        match constraint_val {
            Some(v) if v == value => {
                // Configuration explicitly sets this constraint and it matches
            }
            Some(_) => {
                // Configuration explicitly sets this constraint but it doesn't match
                return Ok(false);
            }
            None => {
                // Configuration doesn't set this constraint, check if there's a default
                match &key.default {
                    Some(default) if default == value => {
                        // Default value matches the required value
                    }
                    _ => {
                        // No default or default doesn't match
                        return Ok(false);
                    }
                }
            }
        }
    }

    // Cell used for buckconfigs is set to cell of target that applies select to match Buck v1 behavior.
    // Eventually, we want this to be the cell of the platform instead.
    for (raw_section_and_key, config_value) in &constraints_and_configs.buckconfigs {
        let config_section_and_key = parse_config_section_and_key(raw_section_and_key, None)?;
        let v = ctx
            .get_legacy_config_property(
                target_node_cell.0,
                BuckconfigKeyRef {
                    section: &config_section_and_key.section,
                    property: &config_section_and_key.key,
                },
            )
            .await?;
        match v {
            Some(v) if &*v == config_value => {}
            _ => return Ok(false),
        }
    }

    Ok(true)
}

#[derive(Clone, Display, Debug, Eq, Hash, PartialEq, Allocative)]
#[display("ConfigurationNode({}, {})", cfg_target, target_cfg)]
struct ConfigurationNodeKey {
    target_cfg: ConfigurationData,
    target_cell: CellNameForConfigurationResolution,
    cfg_target: ConfigurationSettingKey,
}

#[derive(Clone, Display, Debug, Eq, Hash, PartialEq, Allocative)]
#[display(
    "ResolvedConfigurationKey(target_cfg: {}, cell: {}, configuration_deps size {})",
    target_cfg,
    target_cell,
    configuration_deps.len()
)]
struct MatchedConfigurationSettingKeysKey {
    target_cfg: ConfigurationData,
    target_cell: CellNameForConfigurationResolution,
    configuration_deps: Vec<ConfigurationSettingKey>,
}

async fn compute_platform_configuration_no_label_check(
    ctx: &mut DiceComputations<'_>,
    target: &TargetLabel,
) -> kuro_error::Result<ConfigurationData> {
    (&ctx
        // TODO(T198223238): Not supporting platforms being supplied via subtargets for now
        .get_configuration_analysis_result(&ProvidersLabel::default_for(target.dupe()))
        .await?)
        .provider_collection()
        .builtin_provider::<FrozenPlatformInfo>()
        .ok_or_else(|| ConfigurationError::MissingPlatformInfo(target.dupe()))?
        .to_configuration()
}

/// Basically, evaluate `platform()` rule.
async fn compute_platform_configuration(
    ctx: &mut DiceComputations<'_>,
    target: &TargetLabel,
) -> kuro_error::Result<ConfigurationData> {
    let configuration_data = compute_platform_configuration_no_label_check(ctx, target).await?;

    let cell_resolver = ctx.get_cell_resolver().await?;
    let cell_alias_resolver = ctx
        .get_cell_alias_resolver(cell_resolver.root_cell())
        .await?;
    let parsed_target = TargetLabel::parse(
        configuration_data.label()?,
        cell_resolver.root_cell(),
        &cell_resolver,
        &cell_alias_resolver,
    )
    .buck_error_context(
        "`PlatformInfo` label for `platform()` rule should be a valid target label",
    )?;

    if target != &parsed_target {
        // `target` may be an `alias` target. In this case we evaluate the label
        // from the configuration and check it resolves to the same configuration.

        let cfg_again = compute_platform_configuration_no_label_check(
            ctx,
            &parsed_target,
        )
        .await
        .buck_error_context(
            "Checking whether label of returned `PlatformInfo` resolves to the same configuration",
        )?;
        if cfg_again != configuration_data {
            return Err(ConfigurationError::PlatformEvalUnequalConfiguration(
                target.dupe(),
                parsed_target,
            )
            .into());
        }
    }

    Ok(configuration_data)
}

#[async_trait]
impl Key for MatchedConfigurationSettingKeysKey {
    type Value = kuro_error::Result<MatchedConfigurationSettingKeysWithCfg>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellation: &CancellationContext,
    ) -> Self::Value {
        let config_nodes = ctx
            .compute_join(self.configuration_deps.iter(), |ctx, d| {
                async move {
                    (
                        d.dupe(),
                        get_configuration_node(ctx, &self.target_cfg, self.target_cell, d).await,
                    )
                }
                .boxed()
            })
            .await;

        let mut resolved_settings = UnorderedMap::with_capacity(config_nodes.len());
        for (label, node) in config_nodes {
            let node = node?;
            resolved_settings.insert(label, node);
        }
        let resolved_settings = MatchedConfigurationSettingKeys::new(resolved_settings);
        Ok(MatchedConfigurationSettingKeysWithCfg::new(
            ConfigurationNoExec::new(self.target_cfg.dupe()),
            resolved_settings,
        ))
    }

    fn equality(_: &Self::Value, _: &Self::Value) -> bool {
        false
    }
}

async fn get_configuration_node(
    ctx: &mut DiceComputations<'_>,
    target_cfg: &ConfigurationData,
    target_cell: CellNameForConfigurationResolution,
    cfg_target: &ConfigurationSettingKey,
) -> kuro_error::Result<ConfigurationNode> {
    ctx.compute(&ConfigurationNodeKey {
        target_cfg: target_cfg.dupe(),
        target_cell,
        cfg_target: cfg_target.dupe(),
    })
    .await?
    .with_buck_error_context(|| {
        format!(
            "Error getting configuration node of `{cfg_target}` within the `{target_cfg}` configuration",
        )
    })
}

async fn get_configuration_key_providers(
    ctx: &mut DiceComputations<'_>,
    target_cfg: &ConfigurationData,
    cfg_target: &ProvidersLabel,
) -> kuro_error::Result<FrozenProviderCollectionValue> {
    let target_node = ctx.get_target_node(cfg_target.target()).await?;

    if matches!(
        target_node.rule_type(),
        RuleType::Native(NativeRuleKind::Alias)
    ) {
        // Bazel allows alias targets in select keys. For an alias-backed
        // config_setting_group, the alias `actual` attribute may itself be a
        // select(), so it must be resolved in the configuration currently being
        // matched, not in Kuro's unbound configuration-rule analysis cfg.
        let configured =
            cfg_target.configure_pair_no_exec(ConfigurationNoExec::new(target_cfg.dupe()));
        return Ok(ctx.get_providers(&configured).await?.require_compatible()?);
    }

    ctx.get_configuration_analysis_result(cfg_target).await
}

#[async_trait]
impl Key for ConfigurationNodeKey {
    type Value = kuro_error::Result<ConfigurationNode>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellation: &CancellationContext,
    ) -> Self::Value {
        let providers =
            get_configuration_key_providers(ctx, &self.target_cfg, &self.cfg_target.0).await?;

        // capture the result so the temporaries get dropped before providers
        let result = match providers
            .provider_collection()
            .builtin_provider::<FrozenConfigurationInfo>()
        {
            Some(configuration_info) => configuration_info,
            None => {
                return Err::<_, kuro_error::Error>(
                    ConfigurationError::MissingConfigurationInfoProvider(self.cfg_target.0.dupe())
                        .into(),
                );
            }
        }
        .to_config_setting_data();

        let matches =
            configuration_matches(ctx, &self.target_cfg, self.target_cell, &result).await?;

        Ok(ConfigurationNode::new(Some(result).filter(|_| matches)))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }
}

pub(crate) async fn get_platform_configuration(
    ctx: &mut DiceComputations<'_>,
    target: &TargetLabel,
) -> kuro_error::Result<ConfigurationData> {
    #[derive(derive_more::Display, Debug, Eq, Hash, PartialEq, Clone, Allocative)]
    struct PlatformConfigurationKey(TargetLabel);

    #[async_trait]
    impl Key for PlatformConfigurationKey {
        type Value = kuro_error::Result<ConfigurationData>;

        async fn compute(
            &self,
            ctx: &mut DiceComputations,
            _cancellation: &CancellationContext,
        ) -> Self::Value {
            compute_platform_configuration(ctx, &self.0)
                .await
                .map_err(kuro_error::Error::from)
        }

        fn equality(x: &Self::Value, y: &Self::Value) -> bool {
            match (x, y) {
                (Ok(x), Ok(y)) => x == y,
                _ => false,
            }
        }
    }

    ctx.compute(&PlatformConfigurationKey(target.dupe()))
        .await?
        .map_err(kuro_error::Error::from)
}

pub(crate) async fn compute_platform_cfgs(
    ctx: &mut DiceComputations<'_>,
    node: TargetNodeRef<'_>,
) -> kuro_error::Result<OrderedMap<TargetLabel, ConfigurationData>> {
    let mut platform_map = OrderedMap::new();
    for (platform_target, kind) in node.get_configuration_deps_with_kind() {
        if kind == ConfigurationDepKind::ConfiguredDepPlatform {
            let platform_target = platform_target.target();
            let config = get_platform_configuration(ctx, platform_target).await?;
            platform_map.insert(platform_target.dupe(), config);
        }
    }

    Ok(platform_map)
}

pub(crate) async fn get_matched_cfg_keys<
    'a,
    T: IntoIterator<Item = &'a ConfigurationSettingKey> + Send,
>(
    ctx: &mut DiceComputations<'_>,
    target_cfg: &ConfigurationData,
    target_cell: CellNameForConfigurationResolution,
    configuration_deps: T,
) -> kuro_error::Result<MatchedConfigurationSettingKeysWithCfg> {
    let configuration_deps: Vec<ConfigurationSettingKey> =
        configuration_deps.into_iter().map(|t| t.dupe()).collect();
    ctx.compute(&MatchedConfigurationSettingKeysKey {
        target_cfg: target_cfg.dupe(),
        target_cell,
        configuration_deps,
    })
    .await?
}

pub(crate) async fn get_matched_cfg_keys_for_node(
    ctx: &mut DiceComputations<'_>,
    target_cfg: &ConfigurationData,
    target_cell: CellNameForConfigurationResolution,
    node: TargetNodeRef<'_>,
) -> kuro_error::Result<MatchedConfigurationSettingKeysWithCfg> {
    let d = node
        .get_configuration_deps_with_kind()
        .filter_map(|(d, k)| {
            match k {
                ConfigurationDepKind::CompatibilityAttribute => true,
                ConfigurationDepKind::SelectKey => true,
                ConfigurationDepKind::ConfiguredDepPlatform => false,
                ConfigurationDepKind::Transition => false,
            }
            .then_some(d)
        })
        .map(ConfigurationSettingKey::ref_cast);
    get_matched_cfg_keys(ctx, &target_cfg, target_cell, d).await
}

struct ConfigurationCalculationDynImpl;

#[async_trait]
impl ConfigurationCalculationDyn for ConfigurationCalculationDynImpl {
    async fn get_platform_configuration(
        &self,
        ctx: &mut DiceComputations<'_>,
        target: &TargetLabel,
    ) -> kuro_error::Result<ConfigurationData> {
        Ok(get_platform_configuration(ctx, target).await?)
    }
}

pub(crate) fn init_configuration_calculation() {
    CONFIGURATION_CALCULATION.init(&ConfigurationCalculationDynImpl);
}
