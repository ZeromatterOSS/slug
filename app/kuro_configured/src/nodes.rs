/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Calculations relating to 'TargetNode's that runs on Dice

use std::iter;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use derive_more::Display;
use dice::Demand;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use futures::FutureExt;
use itertools::Itertools;
use kuro_build_api::analysis::calculation::RuleAnalysisCalculation;
use kuro_build_api::interpreter::rule_defs::provider::builtin::dep_only_incompatible_info::DepOnlyIncompatibleCustomSoftErrors;
use kuro_build_api::interpreter::rule_defs::provider::builtin::dep_only_incompatible_info::FrozenDepOnlyIncompatibleInfo;
use kuro_build_api::transition::TRANSITION_ATTRS_PROVIDER;
use kuro_build_api::transition::TRANSITION_CALCULATION;
use kuro_build_signals::node_key::BuildSignalsNodeKey;
use kuro_build_signals::node_key::BuildSignalsNodeKeyImpl;
use kuro_common::dice::cells::HasCellResolver;
use kuro_common::dice::cycles::CycleGuard;
use kuro_common::legacy_configs::dice::HasLegacyConfigs;
use kuro_common::legacy_configs::key::BuckconfigKeyRef;
use kuro_common::legacy_configs::view::LegacyBuckConfigView;
use kuro_core::configuration::compatibility::IncompatiblePlatformReason;
use kuro_core::configuration::compatibility::IncompatiblePlatformReasonCause;
use kuro_core::configuration::compatibility::MaybeCompatible;
use kuro_core::configuration::data::ConfigurationData;
use kuro_core::configuration::pair::ConfigurationNoExec;
use kuro_core::configuration::pair::ConfigurationWithExec;
use kuro_core::configuration::transition::applied::TransitionApplied;
use kuro_core::configuration::transition::id::TransitionId;
use kuro_core::execution_types::execution::ExecutionPlatformResolution;
use kuro_core::pattern::pattern::ParsedPattern;
use kuro_core::pattern::pattern_type::TargetPatternExtra;
use kuro_core::plugins::PluginKind;
use kuro_core::plugins::PluginKindSet;
use kuro_core::plugins::PluginListElemKind;
use kuro_core::plugins::PluginLists;
use kuro_core::provider::label::ConfiguredProvidersLabel;
use kuro_core::provider::label::ProvidersLabel;
use kuro_core::soft_error;
use kuro_core::target::configured_or_unconfigured::ConfiguredOrUnconfiguredTargetLabel;
use kuro_core::target::configured_target_label::ConfiguredTargetLabel;
use kuro_core::target::label::label::TargetLabel;
use kuro_core::target::target_configured_target_label::TargetConfiguredTargetLabel;
use kuro_error::BuckErrorContext;
use kuro_error::internal_error;
use kuro_node::attrs::coerced_attr::CoercedAttr;
use kuro_node::attrs::configuration_context::AttrConfigurationContext;
use kuro_node::attrs::configuration_context::AttrConfigurationContextImpl;
use kuro_node::attrs::configuration_context::PlatformConfigurationError;
use kuro_node::attrs::configured_attr::ConfiguredAttr;
use kuro_node::attrs::configured_traversal::ConfiguredAttrTraversal;
use kuro_node::attrs::display::AttrDisplayWithContextExt;
use kuro_node::attrs::inspect_options::AttrInspectOptions;
use kuro_node::attrs::spec::AttributeId;
use kuro_node::attrs::spec::internal::INCOMING_TRANSITION_ATTRIBUTE;
use kuro_node::attrs::spec::internal::LEGACY_TARGET_COMPATIBLE_WITH_ATTRIBUTE;
use kuro_node::attrs::spec::internal::TARGET_COMPATIBLE_WITH_ATTRIBUTE;
use kuro_node::configuration::calculation::CellNameForConfigurationResolution;
use kuro_node::configuration::resolved::MatchedConfigurationSettingKeys;
use kuro_node::configuration::resolved::MatchedConfigurationSettingKeysWithCfg;
use kuro_node::nodes::configured::ConfiguredTargetNode;
use kuro_node::nodes::configured_frontend::CONFIGURED_TARGET_NODE_CALCULATION;
use kuro_node::nodes::configured_frontend::ConfiguredTargetNodeCalculation;
use kuro_node::nodes::configured_frontend::ConfiguredTargetNodeCalculationImpl;
use kuro_node::nodes::frontend::TargetGraphCalculation;
use kuro_node::nodes::unconfigured::TargetNode;
use kuro_node::nodes::unconfigured::TargetNodeRef;
use kuro_node::rule::RuleIncomingTransition;
use kuro_node::visibility::VisibilityError;
use kuro_util::arc_str::ArcStr;
use starlark_map::ordered_map::OrderedMap;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::configuration::compute_platform_cfgs;
use crate::configuration::get_matched_cfg_keys_for_node;
use crate::cycle::ConfiguredGraphCycleDescriptor;
use crate::execution::find_execution_platform_by_configuration;
use crate::execution::resolve_execution_platform;

#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Input)]
enum NodeCalculationError {
    #[error("expected `{0}` attribute to be a list but got `{1}`")]
    TargetCompatibleNotList(String, String),
    #[error(
        "`{0}` had both `{}` and `{}` attributes. It should only have one.",
        TARGET_COMPATIBLE_WITH_ATTRIBUTE.name,
        LEGACY_TARGET_COMPATIBLE_WITH_ATTRIBUTE.name
    )]
    BothTargetCompatibleWith(String),
    #[error(
        "Target {0} configuration transitioned\n\
        old: {1}\n\
        new: {2}\n\
        but attribute: {3}\n\
        resolved with old configuration to: {4}\n\
        resolved with new configuration to: {5}"
    )]
    TransitionAttrIncompatibleChange(
        TargetLabel,
        ConfigurationData,
        ConfigurationData,
        String,
        String,
        String,
    ),

    #[error(
        "Target {0} configuration transition is not idempotent
         in initial configuration  `{1}`
         first transitioned to cfg `{2}`
         then transitions to cfg   `{3}`
         Use `kuro audit configurations {1} {2} {3}` to see the configurations."
    )]
    TransitionNotIdempotent(
        TargetLabel,
        ConfigurationData,
        ConfigurationData,
        ConfigurationData,
    ),
}

enum CompatibilityConstraints {
    Any(ConfiguredAttr),
    All(ConfiguredAttr),
}

#[derive(Debug, kuro_error::Error)]
#[kuro(input)]
enum ToolchainDepError {
    #[error("Target `{0}` was used as a toolchain_dep, but is not a toolchain rule")]
    NonToolchainRuleUsedAsToolchainDep(TargetLabel),
    #[error("Target `{0}` was used not as a toolchain_dep, but is a toolchain rule")]
    ToolchainRuleUsedAsNormalDep(TargetLabel),
}

#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Input)]
enum PluginDepError {
    #[error("Plugin dep `{0}` is a toolchain rule")]
    PluginDepIsToolchainRule(TargetLabel),
}

fn unpack_target_compatible_with_attr(
    target_node: TargetNodeRef,
    resolved_cfg: &MatchedConfigurationSettingKeysWithCfg,
    attr_id: AttributeId,
) -> kuro_error::Result<Option<ConfiguredAttr>> {
    let attr = target_node.known_attr_or_none(attr_id, AttrInspectOptions::All);
    let attr = match attr {
        Some(attr) => attr,
        None => return Ok(None),
    };

    struct AttrConfigurationContextToResolveCompatibleWith<'c> {
        resolved_cfg: &'c MatchedConfigurationSettingKeysWithCfg,
    }

    impl AttrConfigurationContext for AttrConfigurationContextToResolveCompatibleWith<'_> {
        fn matched_cfg_keys(&self) -> &MatchedConfigurationSettingKeys {
            self.resolved_cfg.settings()
        }

        fn cfg(&self) -> ConfigurationNoExec {
            self.resolved_cfg.cfg().dupe()
        }

        fn exec_cfg(&self) -> kuro_error::Result<ConfigurationNoExec> {
            Err(internal_error!(
                "exec_cfg() is not needed to resolve `{}` or `{}`",
                TARGET_COMPATIBLE_WITH_ATTRIBUTE.name,
                LEGACY_TARGET_COMPATIBLE_WITH_ATTRIBUTE.name
            ))
        }

        fn toolchain_cfg(&self) -> ConfigurationWithExec {
            unreachable!()
        }

        fn platform_cfg(&self, _label: &TargetLabel) -> kuro_error::Result<ConfigurationData> {
            unreachable!(
                "platform_cfg() is not needed to resolve `{}` or `{}`",
                TARGET_COMPATIBLE_WITH_ATTRIBUTE.name, LEGACY_TARGET_COMPATIBLE_WITH_ATTRIBUTE.name
            )
        }

        fn resolved_transitions(
            &self,
        ) -> kuro_error::Result<&OrderedMap<Arc<TransitionId>, Arc<TransitionApplied>>> {
            Err(internal_error!(
                "resolved_transitions() is not needed to resolve `{}` or `{}`",
                TARGET_COMPATIBLE_WITH_ATTRIBUTE.name,
                LEGACY_TARGET_COMPATIBLE_WITH_ATTRIBUTE.name
            ))
        }
    }

    let attr = attr
        .configure(&AttrConfigurationContextToResolveCompatibleWith { resolved_cfg })
        .with_buck_error_context(|| format!("Error configuring attribute `{}`", attr.name))?;

    match attr.value.unpack_list() {
        Some(values) => {
            if !values.is_empty() {
                Ok(Some(attr.value))
            } else {
                Ok(None)
            }
        }
        None => Err(NodeCalculationError::TargetCompatibleNotList(
            attr.name.to_owned(),
            attr.value.as_display_no_ctx().to_string(),
        )
        .into()),
    }
}

fn check_compatible(
    target_label: &ConfiguredTargetLabel,
    target_node: TargetNodeRef,
    resolved_cfg: &MatchedConfigurationSettingKeysWithCfg,
) -> kuro_error::Result<MaybeCompatible<()>> {
    let target_compatible_with = unpack_target_compatible_with_attr(
        target_node,
        resolved_cfg,
        TARGET_COMPATIBLE_WITH_ATTRIBUTE.id,
    )?;
    let legacy_compatible_with = unpack_target_compatible_with_attr(
        target_node,
        resolved_cfg,
        LEGACY_TARGET_COMPATIBLE_WITH_ATTRIBUTE.id,
    )?;

    let compatibility_constraints = match (target_compatible_with, legacy_compatible_with) {
        (None, None) => return Ok(MaybeCompatible::Compatible(())),
        (Some(..), Some(..)) => {
            return Err(
                NodeCalculationError::BothTargetCompatibleWith(target_label.to_string()).into(),
            );
        }
        (Some(target_compatible_with), None) => {
            CompatibilityConstraints::All(target_compatible_with)
        }
        (None, Some(legacy_compatible_with)) => {
            CompatibilityConstraints::Any(legacy_compatible_with)
        }
    };

    // We are compatible if the list of target expressions is empty,
    // OR if we match ANY expression in the list of attributes.
    let check_compatibility = |attr| -> kuro_error::Result<(Vec<_>, Vec<_>)> {
        let mut left = Vec::new();
        let mut right = Vec::new();
        for label in ConfiguredTargetNode::attr_as_target_compatible_with(attr) {
            let label = label?;
            match resolved_cfg.settings().setting_matches(&label) {
                Some(_) => left.push(label),
                None => right.push(label),
            }
        }

        Ok((left, right))
    };

    // We only record the first incompatibility, for either ANY or ALL.
    // TODO(cjhopman): Should we report _all_ the things that are incompatible?
    let incompatible_target = match compatibility_constraints {
        CompatibilityConstraints::Any(attr) => {
            let (compatible, incompatible) =
                check_compatibility(attr).with_buck_error_context(|| {
                    format!(
                        "attribute `{}`",
                        LEGACY_TARGET_COMPATIBLE_WITH_ATTRIBUTE.name
                    )
                })?;
            let incompatible = incompatible.into_iter().next();
            match (compatible.is_empty(), incompatible.into_iter().next()) {
                (false, _) | (true, None) => {
                    return Ok(MaybeCompatible::Compatible(()));
                }
                (true, Some(v)) => v,
            }
        }
        CompatibilityConstraints::All(attr) => {
            let (_compatible, incompatible) =
                check_compatibility(attr).with_buck_error_context(|| {
                    format!("attribute `{}`", TARGET_COMPATIBLE_WITH_ATTRIBUTE.name)
                })?;
            match incompatible.into_iter().next() {
                Some(label) => label,
                None => {
                    return Ok(MaybeCompatible::Compatible(()));
                }
            }
        }
    };
    Ok(MaybeCompatible::Incompatible(Arc::new(
        IncompatiblePlatformReason {
            target: target_label.dupe(),
            cause: IncompatiblePlatformReasonCause::UnsatisfiedConfig(incompatible_target.0.dupe()),
        },
    )))
}

/// Ideally, we would check this much earlier. However, that turns out to be a bit tricky to
/// implement. Naively implementing this check on unconfigured nodes doesn't work because it results
/// in dice cycles when there are cycles in the unconfigured graph.
async fn check_plugin_deps(
    ctx: &mut DiceComputations<'_>,
    target_label: &ConfiguredTargetLabel,
    plugin_deps: &PluginLists,
) -> kuro_error::Result<()> {
    for (_, dep_label, elem_kind) in plugin_deps.iter() {
        if *elem_kind == PluginListElemKind::Direct {
            let dep_node = ctx
                .get_target_node(dep_label)
                .await
                .with_buck_error_context(|| {
                    format!("looking up unconfigured target node `{dep_label}`")
                })?;
            if dep_node.is_toolchain_rule() {
                return Err(PluginDepError::PluginDepIsToolchainRule(dep_label.dupe()).into());
            }
            if !dep_node.is_visible_to(target_label.unconfigured())? {
                return Err(VisibilityError::NotVisibleTo(
                    dep_label.dupe(),
                    target_label.unconfigured().dupe(),
                )
                .into());
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum CheckVisibility {
    Yes,
    No,
}

#[derive(Default)]
pub(crate) struct ErrorsAndIncompatibilities {
    errs: Vec<kuro_error::Error>,
    incompats: Vec<Arc<IncompatiblePlatformReason>>,
}

impl ErrorsAndIncompatibilities {
    fn unpack_dep_into(
        &mut self,
        target_label: &TargetConfiguredTargetLabel,
        result: kuro_error::Result<MaybeCompatible<ConfiguredTargetNode>>,
        check_visibility: CheckVisibility,
        list: &mut Vec<ConfiguredTargetNode>,
    ) {
        list.extend(self.unpack_dep(target_label, result, check_visibility));
    }

    fn unpack_dep(
        &mut self,
        target_label: &TargetConfiguredTargetLabel,
        result: kuro_error::Result<MaybeCompatible<ConfiguredTargetNode>>,
        check_visibility: CheckVisibility,
    ) -> Option<ConfiguredTargetNode> {
        match result {
            Err(e) => {
                self.errs.push(e);
            }
            Ok(MaybeCompatible::Incompatible(reason)) => {
                self.incompats.push(Arc::new(IncompatiblePlatformReason {
                    target: target_label.inner().dupe(),
                    cause: IncompatiblePlatformReasonCause::Dependency(reason.dupe()),
                }));
            }
            Ok(MaybeCompatible::Compatible(dep)) => {
                if CheckVisibility::No == check_visibility {
                    return Some(dep);
                }
                match dep.is_visible_to(target_label.unconfigured()) {
                    Ok(true) => {
                        return Some(dep);
                    }
                    Ok(false) => {
                        self.errs.push(
                            VisibilityError::NotVisibleTo(
                                dep.label().unconfigured().dupe(),
                                target_label.unconfigured().dupe(),
                            )
                            .into(),
                        );
                    }
                    Err(e) => {
                        self.errs.push(e.into());
                    }
                }
            }
        }
        None
    }

    /// Returns an error/incompatibility to return, if any, and `None` otherwise
    pub(crate) fn finalize<T>(mut self) -> Option<kuro_error::Result<MaybeCompatible<T>>> {
        // FIXME(JakobDegen): Report all incompatibilities
        if let Some(incompat) = self.incompats.pop() {
            return Some(Ok(MaybeCompatible::Incompatible(incompat)));
        }
        if let Some(err) = self.errs.pop() {
            return Some(Err(err.into()));
        }
        None
    }
}

#[derive(Default)]
pub(crate) struct GatheredDeps {
    pub(crate) deps: Vec<ConfiguredTargetNode>,
    pub(crate) exec_deps: SmallMap<ConfiguredProvidersLabel, CheckVisibility>,
    pub(crate) toolchain_deps: SmallSet<TargetConfiguredTargetLabel>,
    pub(crate) plugin_lists: PluginLists,
    /// Aspect results for dependencies with aspects attached (Phase 8c)
    pub(crate) aspect_results: std::collections::HashMap<
        (
            ConfiguredTargetLabel,
            std::sync::Arc<kuro_node::aspect_type::StarlarkAspectType>,
        ),
        kuro_analysis::analysis::aspect_key::AspectValue,
    >,
}

pub(crate) async fn gather_deps(
    target_label: &TargetConfiguredTargetLabel,
    target_node: TargetNodeRef<'_>,
    attr_cfg_ctx: &(dyn AttrConfigurationContext + Sync),
    ctx: &mut DiceComputations<'_>,
) -> kuro_error::Result<(GatheredDeps, ErrorsAndIncompatibilities)> {
    #[derive(Default)]
    struct Traversal {
        deps: OrderedMap<ConfiguredProvidersLabel, SmallSet<PluginKindSet>>,
        exec_deps: SmallMap<ConfiguredProvidersLabel, CheckVisibility>,
        toolchain_deps: SmallSet<TargetConfiguredTargetLabel>,
        plugin_lists: PluginLists,
    }

    impl ConfiguredAttrTraversal for Traversal {
        fn dep(&mut self, dep: &ConfiguredProvidersLabel) -> kuro_error::Result<()> {
            self.deps.entry(dep.dupe()).or_insert_with(SmallSet::new);
            Ok(())
        }

        fn dep_with_plugins(
            &mut self,
            dep: &ConfiguredProvidersLabel,
            plugin_kinds: &PluginKindSet,
        ) -> kuro_error::Result<()> {
            self.deps
                .entry(dep.dupe())
                .or_insert_with(SmallSet::new)
                .insert(plugin_kinds.dupe());
            Ok(())
        }

        fn exec_dep(&mut self, dep: &ConfiguredProvidersLabel) -> kuro_error::Result<()> {
            self.exec_deps.insert(dep.dupe(), CheckVisibility::Yes);
            Ok(())
        }

        fn toolchain_dep(&mut self, dep: &ConfiguredProvidersLabel) -> kuro_error::Result<()> {
            self.toolchain_deps
                .insert(TargetConfiguredTargetLabel::new_without_exec_cfg(
                    dep.target().dupe(),
                ));
            Ok(())
        }

        fn plugin_dep(&mut self, dep: &TargetLabel, kind: &PluginKind) -> kuro_error::Result<()> {
            self.plugin_lists
                .insert(kind.dupe(), dep.dupe(), PluginListElemKind::Direct);
            Ok(())
        }
    }

    let mut traversal = Traversal::default();
    for a in target_node.attrs(AttrInspectOptions::All) {
        let configured_attr = a.configure(attr_cfg_ctx)?;
        configured_attr.traverse(target_node.label().pkg(), &mut traversal)?;
    }

    // Phase 8c: Collect aspects that need to be applied to dependencies
    let mut aspect_keys = Vec::new();

    for a in target_node.attrs(AttrInspectOptions::All) {
        // Check if this attribute has aspects attached
        if !a.attr.aspects().is_empty() {
            let configured_attr = a.configure(attr_cfg_ctx)?;

            // Extract dependency labels from this configured attribute
            // Using the same ConfiguredAttrTraversal pattern
            struct AspectDepsCollector {
                deps: Vec<ConfiguredTargetLabel>,
            }

            impl ConfiguredAttrTraversal for AspectDepsCollector {
                fn dep(&mut self, dep: &ConfiguredProvidersLabel) -> kuro_error::Result<()> {
                    self.deps.push(dep.target().dupe());
                    Ok(())
                }

                fn dep_with_plugins(
                    &mut self,
                    dep: &ConfiguredProvidersLabel,
                    _plugin_kinds: &PluginKindSet,
                ) -> kuro_error::Result<()> {
                    self.deps.push(dep.target().dupe());
                    Ok(())
                }

                // Exec deps and toolchain deps don't propagate aspects in Phase 8c
                fn exec_dep(&mut self, _dep: &ConfiguredProvidersLabel) -> kuro_error::Result<()> {
                    Ok(())
                }

                fn toolchain_dep(
                    &mut self,
                    _dep: &ConfiguredProvidersLabel,
                ) -> kuro_error::Result<()> {
                    Ok(())
                }

                fn plugin_dep(
                    &mut self,
                    _dep: &TargetLabel,
                    _kind: &PluginKind,
                ) -> kuro_error::Result<()> {
                    Ok(())
                }
            }

            let mut collector = AspectDepsCollector { deps: Vec::new() };
            configured_attr.traverse(target_node.label().pkg(), &mut collector)?;

            // Schedule aspect computation for each dep (Phase 8c)
            for dep_label in collector.deps {
                for aspect_type in a.attr.aspects() {
                    aspect_keys.push(kuro_analysis::analysis::aspect_key::AspectKey::new(
                        dep_label.dupe(),
                        aspect_type.dupe(),
                    ));
                }
            }
        }
    }

    // Compute all aspects in parallel via DICE (following pattern from lines 499-503)
    let aspect_results = if !aspect_keys.is_empty() {
        ctx.compute_join(aspect_keys.iter(), |ctx, key| {
            async move {
                // Returns Result<AspectValue>
                ctx.compute(key).await
            }
            .boxed()
        })
        .await
    } else {
        Vec::new()
    };

    // Store aspect results temporarily, will process errors after errors_and_incompats is created
    let aspect_results_with_keys: Vec<_> = aspect_keys.into_iter().zip(aspect_results).collect();

    let dep_results = ctx
        .compute_join(traversal.deps.iter(), |ctx, v| {
            async move { ctx.get_internal_configured_target_node(v.0.target()).await }.boxed()
        })
        .await;

    let mut plugin_lists = traversal.plugin_lists;
    let mut deps = Vec::new();
    let mut errors_and_incompats = ErrorsAndIncompatibilities::default();
    for (res, (_, plugin_kind_sets)) in dep_results.into_iter().zip(traversal.deps) {
        let Some(dep) = errors_and_incompats.unpack_dep(target_label, res, CheckVisibility::Yes)
        else {
            continue;
        };

        if !plugin_kind_sets.is_empty() {
            for (kind, plugins) in dep.plugin_lists().iter_by_kind() {
                let Some(should_propagate) = plugin_kind_sets
                    .iter()
                    .filter_map(|set| set.get(kind))
                    .reduce(std::ops::BitOr::bitor)
                else {
                    continue;
                };
                let should_propagate = if should_propagate {
                    PluginListElemKind::Propagate
                } else {
                    PluginListElemKind::NoPropagate
                };
                for (target, elem_kind) in plugins {
                    if *elem_kind != PluginListElemKind::NoPropagate {
                        plugin_lists.insert(kind.dupe(), target.dupe(), should_propagate);
                    }
                }
            }
        }

        deps.push(dep);
    }

    let mut exec_deps = traversal.exec_deps;
    for kind in target_node.uses_plugins() {
        for plugin_label in plugin_lists.iter_for_kind(kind).map(|(target, _)| {
            attr_cfg_ctx.configure_exec_target(&ProvidersLabel::default_for(target.dupe()))
        }) {
            exec_deps
                .entry(plugin_label?)
                .or_insert(CheckVisibility::No);
        }
    }

    // Process aspect results and handle errors (Phase 8c)
    let mut aspect_results_map = std::collections::HashMap::new();
    for (key, result) in aspect_results_with_keys {
        match result {
            Ok(Ok(aspect_value)) => {
                aspect_results_map
                    .insert((key.target.dupe(), key.aspect_type.dupe()), aspect_value);
            }
            Ok(Err(e)) => {
                // Add to errors following existing error handling pattern
                errors_and_incompats.errs.push(e);
            }
            Err(e) => {
                // DICE error - convert to kuro_error::Error
                errors_and_incompats.errs.push(e.into());
            }
        }
    }

    Ok((
        GatheredDeps {
            deps,
            exec_deps,
            toolchain_deps: traversal.toolchain_deps,
            plugin_lists,
            aspect_results: aspect_results_map,
        },
        errors_and_incompats,
    ))
}

/// Resolves configured attributes of target node needed to compute transitions
async fn resolve_transition_attrs<'a>(
    transitions: impl Iterator<Item = &TransitionId>,
    target_node: &'a TargetNode,
    matched_cfg_keys: &MatchedConfigurationSettingKeysWithCfg,
    platform_cfgs: &OrderedMap<TargetLabel, ConfigurationData>,
    ctx: &mut DiceComputations<'_>,
) -> kuro_error::Result<OrderedMap<&'a str, Arc<ConfiguredAttr>>> {
    struct AttrConfigurationContextToResolveTransitionAttrs<'c> {
        matched_cfg_keys: &'c MatchedConfigurationSettingKeysWithCfg,
        toolchain_cfg: ConfigurationWithExec,
        platform_cfgs: &'c OrderedMap<TargetLabel, ConfigurationData>,
    }

    impl AttrConfigurationContext for AttrConfigurationContextToResolveTransitionAttrs<'_> {
        fn matched_cfg_keys(&self) -> &MatchedConfigurationSettingKeys {
            self.matched_cfg_keys.settings()
        }

        fn cfg(&self) -> ConfigurationNoExec {
            self.matched_cfg_keys.cfg().dupe()
        }

        fn exec_cfg(&self) -> kuro_error::Result<ConfigurationNoExec> {
            Err(internal_error!(
                "exec_cfg() is not needed in pre transition attribute resolution."
            ))
        }

        fn toolchain_cfg(&self) -> ConfigurationWithExec {
            self.toolchain_cfg.dupe()
        }

        fn platform_cfg(&self, label: &TargetLabel) -> kuro_error::Result<ConfigurationData> {
            match self.platform_cfgs.get(label) {
                Some(configuration) => Ok(configuration.dupe()),
                None => Err(PlatformConfigurationError::UnknownPlatformTarget(label.dupe()).into()),
            }
        }

        fn resolved_transitions(
            &self,
        ) -> kuro_error::Result<&OrderedMap<Arc<TransitionId>, Arc<TransitionApplied>>> {
            Err(internal_error!(
                "resolved_transitions() can't be used before transition execution."
            ))
        }
    }

    let cfg_ctx = AttrConfigurationContextToResolveTransitionAttrs {
        matched_cfg_keys,
        platform_cfgs,
        toolchain_cfg: matched_cfg_keys
            .cfg()
            .make_toolchain(&ConfigurationNoExec::unbound_exec()),
    };
    let mut result = OrderedMap::default();
    for tr in transitions {
        let attrs = TRANSITION_ATTRS_PROVIDER
            .get()?
            .transition_attrs(ctx, &tr)
            .await?;
        if let Some(attrs) = attrs {
            for attr in attrs.as_ref() {
                // Multiple outgoing transitions may refer the same attribute.
                if result.contains_key(attr.as_str()) {
                    continue;
                }

                if let Some(coerced_attr) = target_node.attr(&attr, AttrInspectOptions::All)? {
                    let configured_attr = coerced_attr.configure(&cfg_ctx)?;
                    if let Some(old_val) =
                        result.insert(configured_attr.name, Arc::new(configured_attr.value))
                    {
                        return Err(internal_error!(
                            "Found duplicated value `{}` for attr `{}` on target `{}`",
                            &old_val.as_display_no_ctx(),
                            attr,
                            target_node.label()
                        ));
                    }
                }
            }
        }
    }
    Ok(result)
}

/// Verifies if configured node's attributes are equal to the same attributes configured with pre-transition configuration.
/// Only check attributes used in transition.
fn verify_transitioned_attrs(
    // Attributes resolved with pre-transition configuration
    pre_transition_attrs: &OrderedMap<&str, Arc<ConfiguredAttr>>,
    pre_transition_config: &ConfigurationData,
    node: &ConfiguredTargetNode,
) -> kuro_error::Result<()> {
    for (attr, attr_value) in pre_transition_attrs {
        let transition_configured_attr = node
            .get(attr, AttrInspectOptions::All)
            .with_internal_error(|| {
                format!(
                    "Attr {} was not found in transition for target {} ({})",
                    attr,
                    node.label(),
                    node.attrs(AttrInspectOptions::All)
                        .format_with(", ", |v, f| f(&format_args!("{v:?}")))
                )
            })?;
        if &transition_configured_attr.value != attr_value.as_ref() {
            return Err(NodeCalculationError::TransitionAttrIncompatibleChange(
                node.label().unconfigured().dupe(),
                pre_transition_config.dupe(),
                node.label().cfg().dupe(),
                attr.to_string(),
                attr_value.as_display_no_ctx().to_string(),
                transition_configured_attr
                    .value
                    .as_display_no_ctx()
                    .to_string(),
            )
            .into());
        }
    }
    Ok(())
}

/// Compute configured target node ignoring transition for this node.
async fn compute_configured_target_node_no_transition(
    target_label: &ConfiguredTargetLabel,
    target_node: TargetNode,
    ctx: &mut DiceComputations<'_>,
) -> kuro_error::Result<MaybeCompatible<ConfiguredTargetNode>> {
    let partial_target_label =
        &TargetConfiguredTargetLabel::new_without_exec_cfg(target_label.dupe());
    let target_cfg = target_label.cfg();
    let target_cell = target_node.label().pkg().cell_name();
    let resolved_configuration = get_matched_cfg_keys_for_node(
        ctx,
        target_cfg,
        CellNameForConfigurationResolution(target_cell),
        target_node.as_ref(),
    )
    .await
    .with_buck_error_context(|| {
        format!("Error resolving configuration deps of `{target_label}`")
    })?;

    // Must check for compatibility before evaluating non-compatibility attributes.
    if let MaybeCompatible::Incompatible(reason) =
        check_compatible(target_label, target_node.as_ref(), &resolved_configuration)?
    {
        return Ok(MaybeCompatible::Incompatible(reason));
    }

    let platform_cfgs = compute_platform_cfgs(ctx, target_node.as_ref()).await?;

    let mut resolved_transitions = OrderedMap::new();
    let attrs = resolve_transition_attrs(
        target_node.transition_deps().map(|(_, tr)| tr.as_ref()),
        &target_node,
        &resolved_configuration,
        &platform_cfgs,
        ctx,
    )
    .boxed()
    .await?;
    for (_dep, tr) in target_node.transition_deps() {
        let resolved_cfg = TRANSITION_CALCULATION
            .get()?
            .apply_transition(ctx, &attrs, target_cfg, tr)
            .await?;
        resolved_transitions.insert(tr.dupe(), resolved_cfg);
    }

    // We need to collect deps and to ensure that all attrs can be successfully
    // configured so that we don't need to support propagate configuration errors on attr access.
    let attr_cfg_ctx = AttrConfigurationContextImpl::new(
        &resolved_configuration,
        // We have not yet done exec platform resolution so for now we just use `unbound_exec`
        // here. We only use this when collecting exec deps and toolchain deps. In both of those
        // cases, we replace the exec cfg later on in this function with the "proper" exec cfg.
        ConfigurationNoExec::unbound_exec(),
        &resolved_transitions,
        &platform_cfgs,
    );
    let (gathered_deps, mut errors_and_incompats) = gather_deps(
        partial_target_label,
        target_node.as_ref(),
        &attr_cfg_ctx,
        ctx,
    )
    .boxed()
    .await?;

    check_plugin_deps(ctx, target_label, &gathered_deps.plugin_lists)
        .boxed()
        .await?;

    let execution_platform_resolution = if target_cfg.is_unbound() {
        // The unbound configuration is used when evaluation configuration nodes.
        // That evaluation is
        // (1) part of execution platform resolution and
        // (2) isn't allowed to do execution
        // And so we use an "unspecified" execution platform to avoid cycles and cause any attempts at execution to fail.
        ExecutionPlatformResolution::unspecified()
    } else if let Some(exec_cfg) = target_label.exec_cfg() {
        // The label was produced by a toolchain_dep, so we use the execution platform of our parent
        // We need to convert that to an execution platform, so just find the one with the same configuration.
        ExecutionPlatformResolution::new(
            Some(
                find_execution_platform_by_configuration(
                    ctx,
                    exec_cfg,
                    resolved_configuration.cfg().cfg(),
                )
                .await?,
            ),
            Vec::new(),
        )
    } else {
        resolve_execution_platform(
            ctx,
            target_node.as_ref(),
            &resolved_configuration,
            &gathered_deps,
            &attr_cfg_ctx,
        )
        .boxed()
        .await?
    };
    let execution_platform = execution_platform_resolution.cfg();

    // We now need to replace the dummy exec config we used above with the real one

    let execution_platform = &execution_platform;
    let toolchain_deps = &gathered_deps.toolchain_deps;
    let exec_deps = &gathered_deps.exec_deps;

    let get_toolchain_deps = DiceComputations::declare_closure(move |ctx| {
        async move {
            ctx.compute_join(
                toolchain_deps,
                |ctx, target: &TargetConfiguredTargetLabel| {
                    async move {
                        ctx.get_internal_configured_target_node(
                            &target.with_exec_cfg(execution_platform.cfg().dupe()),
                        )
                        .await
                    }
                    .boxed()
                },
            )
            .await
        }
        .boxed()
    });

    let get_exec_deps = DiceComputations::declare_closure(|ctx| {
        async move {
            ctx.compute_join(exec_deps, |ctx, (target, check_visibility)| {
                async move {
                    (
                        ctx.get_internal_configured_target_node(
                            &target
                                .target()
                                .unconfigured()
                                .configure_pair(execution_platform.cfg_pair().dupe()),
                        )
                        .await,
                        *check_visibility,
                    )
                }
                .boxed()
            })
            .await
        }
        .boxed()
    });

    let (toolchain_dep_results, exec_dep_results): (Vec<_>, Vec<_>) =
        ctx.compute2(get_toolchain_deps, get_exec_deps).await;

    let mut deps = gathered_deps.deps;
    let mut exec_deps = Vec::with_capacity(gathered_deps.exec_deps.len());

    for dep in toolchain_dep_results {
        errors_and_incompats.unpack_dep_into(
            partial_target_label,
            dep,
            CheckVisibility::Yes,
            &mut deps,
        );
    }
    for (dep, check_visibility) in exec_dep_results {
        errors_and_incompats.unpack_dep_into(
            partial_target_label,
            dep,
            check_visibility,
            &mut exec_deps,
        );
    }

    if let Some(ret) = errors_and_incompats.finalize() {
        return ret;
    }

    Ok(MaybeCompatible::Compatible(ConfiguredTargetNode::new(
        target_label.dupe(),
        target_node.dupe(),
        resolved_configuration,
        resolved_transitions,
        execution_platform_resolution,
        deps,
        exec_deps,
        platform_cfgs,
        gathered_deps.plugin_lists,
    )))
}

async fn compute_configured_target_node(
    key: &ConfiguredTargetNodeKey,
    ctx: &mut DiceComputations<'_>,
) -> kuro_error::Result<MaybeCompatible<ConfiguredTargetNode>> {
    let target_node = ctx
        .get_target_node(key.0.unconfigured())
        .await
        .with_buck_error_context(|| {
            format!(
                "looking up unconfigured target node `{}`",
                key.0.unconfigured()
            )
        })?;

    match key.0.exec_cfg() {
        None if target_node.is_toolchain_rule() => {
            return Err(ToolchainDepError::ToolchainRuleUsedAsNormalDep(
                key.0.unconfigured().dupe(),
            )
            .into());
        }
        Some(_) if !target_node.is_toolchain_rule() => {
            return Err(ToolchainDepError::NonToolchainRuleUsedAsToolchainDep(
                key.0.unconfigured().dupe(),
            )
            .into());
        }
        _ => {}
    }

    let transition_id = match &target_node.rule.cfg {
        RuleIncomingTransition::None => None,
        RuleIncomingTransition::Fixed(transition_id) => Some(transition_id.dupe()),
        RuleIncomingTransition::FromAttribute => target_node
            .attr_or_none(INCOMING_TRANSITION_ATTRIBUTE.name, AttrInspectOptions::All)
            .and_then(|v| match v.value {
                CoercedAttr::None => None,
                CoercedAttr::ConfigurationDep(l) => Some(Arc::new(TransitionId::Target(l.dupe()))),
                _ => unreachable!("Verified by attr coercer"),
            }),
    };

    if let Some(transition_id) = transition_id {
        compute_configured_forward_target_node(key, &target_node, &transition_id, ctx).await
    } else {
        // We are not caching `ConfiguredTransitionedNodeKey` because this is cheap,
        // and no need to fetch `target_node` again.
        compute_configured_target_node_no_transition(&key.0.dupe(), target_node, ctx).await
    }
}

async fn compute_configured_forward_target_node(
    key: &ConfiguredTargetNodeKey,
    target_node: &TargetNode,
    transition_id: &TransitionId,
    ctx: &mut DiceComputations<'_>,
) -> kuro_error::Result<MaybeCompatible<ConfiguredTargetNode>> {
    let target_label_before_transition = &key.0;
    let platform_cfgs = compute_platform_cfgs(ctx, target_node.as_ref())
        .boxed()
        .await?;
    let matched_cfg_keys = get_matched_cfg_keys_for_node(
        ctx,
        target_label_before_transition.cfg(),
        CellNameForConfigurationResolution(target_node.label().pkg().cell_name()),
        target_node.as_ref(),
    )
    .await
    .with_buck_error_context(|| {
        format!("Error resolving configuration deps of `{target_label_before_transition}`")
    })?;

    let attrs = resolve_transition_attrs(
        iter::once(transition_id),
        target_node,
        &matched_cfg_keys,
        &platform_cfgs,
        ctx,
    )
    .boxed()
    .await?;

    let cfg = TRANSITION_CALCULATION
        .get()?
        .apply_transition(
            ctx,
            &attrs,
            target_label_before_transition.cfg(),
            transition_id,
        )
        .await?;
    let target_label_after_transition = target_label_before_transition
        .unconfigured()
        .configure(cfg.single()?.dupe());

    if &target_label_after_transition == target_label_before_transition {
        // Transitioned to identical configured target, no need to create a forward node.
        compute_configured_target_node_no_transition(
            target_label_before_transition,
            target_node.dupe(),
            ctx,
        )
        .boxed()
        .await
    } else {
        // This must call through dice to get the configured target node so that it is the correct
        // instance (because ConfiguredTargetNode uses reference equality on its deps).
        // This also helps further verify idempotence (as we will get the real result with the any transition applied again).
        let transitioned_node = ctx
            .get_internal_configured_target_node(&target_label_after_transition)
            .await?;

        // In apply_transition() above we've checked that the transition is idempotent when applied again with the same attrs (but the
        // transitioned cfg) we don't know if it causes an attr change (and then a subsequent change in the transition
        // result). We verify that here. If we're in a case where it is changing the attr in a way that causes the transition
        // to introduce a cycle, we depend on the dice cycle detection to identify it. Alternatively we could directly recompute
        // the node and check the attrs, but we'd still need to request the real node from dice and it doesn't seem worth
        // that extra cost just for a slightly improved error message.
        if let MaybeCompatible::Compatible(node) = &transitioned_node {
            // check that the attrs weren't changed first. This should be the only way that we can hit non-idempotence
            // here and gives a better error than if we just give the general idempotence error.
            verify_transitioned_attrs(&attrs, matched_cfg_keys.cfg().cfg(), node)?;

            if let Some(forward) = node.forward_target() {
                return Err(NodeCalculationError::TransitionNotIdempotent(
                    target_label_before_transition.unconfigured().dupe(),
                    target_label_before_transition.cfg().dupe(),
                    target_label_after_transition.cfg().dupe(),
                    forward.label().cfg().dupe(),
                ))
                .internal_error("idempotence should have been enforced by transition idempotence and attr change checks");
            }
        }

        let configured_target_node = transitioned_node.try_map(|transitioned_node| {
            ConfiguredTargetNode::new_forward(
                target_label_before_transition.dupe(),
                transitioned_node,
            )
        })?;

        Ok(configured_target_node)
    }
}

#[derive(Clone, Dupe, Display, Debug, Eq, Hash, PartialEq, Allocative)]
pub struct ConfiguredTargetNodeKey(pub ConfiguredTargetLabel);

struct ConfiguredTargetNodeCalculationInstance;

pub(crate) fn init_configured_target_node_calculation() {
    CONFIGURED_TARGET_NODE_CALCULATION.init(&ConfiguredTargetNodeCalculationInstance);
}

#[derive(Debug, Allocative, Eq, PartialEq)]
pub(crate) struct LookingUpConfiguredNodeContext {
    target: ConfiguredTargetLabel,
    len: usize,
    rest: Option<Arc<Self>>,
}

impl kuro_error::TypedContext for LookingUpConfiguredNodeContext {
    fn eq(&self, other: &dyn kuro_error::TypedContext) -> bool {
        match (other as &dyn std::any::Any).downcast_ref::<Self>() {
            Some(v) => self == v,
            None => false,
        }
    }

    fn display(&self) -> Option<String> {
        Some(format!("{}", self))
    }
}

impl LookingUpConfiguredNodeContext {
    pub(crate) fn new(target: ConfiguredTargetLabel, parent: Option<Arc<Self>>) -> Self {
        let (len, rest) = match parent {
            Some(v) => (v.len + 1, Some(v.clone())),
            None => (1, None),
        };
        Self { target, len, rest }
    }

    pub(crate) fn add_context<T>(
        res: kuro_error::Result<T>,
        target: ConfiguredTargetLabel,
    ) -> kuro_error::Result<T> {
        res.compute_context(
            |parent_ctx: Arc<Self>| Self::new(target.dupe(), Some(parent_ctx)),
            || Self::new(target.dupe(), None),
        )
    }
}

impl std::fmt::Display for LookingUpConfiguredNodeContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.len == 1 {
            write!(f, "Error looking up configured node {}", &self.target)?;
        } else {
            writeln!(
                f,
                "Error in configured node dependency, dependency chain follows (-> indicates depends on, ^ indicates same configuration as previous):"
            )?;

            let mut curr = self;
            let mut prev_cfg = None;
            let mut is_first = true;

            loop {
                f.write_str("    ")?;
                if is_first {
                    f.write_str("   ")?;
                } else {
                    f.write_str("-> ")?;
                }

                write!(f, "{}", curr.target.unconfigured())?;
                let cfg = Some(curr.target.cfg());
                f.write_str(" (")?;
                if cfg == prev_cfg {
                    f.write_str("^")?;
                } else {
                    std::fmt::Display::fmt(curr.target.cfg(), f)?;
                }
                f.write_str(")\n")?;
                is_first = false;
                prev_cfg = Some(curr.target.cfg());
                match &curr.rest {
                    Some(v) => curr = &**v,
                    None => break,
                }
            }
        }
        Ok(())
    }
}

#[async_trait]
impl Key for ConfiguredTargetNodeKey {
    type Value = kuro_error::Result<MaybeCompatible<ConfiguredTargetNode>>;
    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellation: &CancellationContext,
    ) -> Self::Value {
        let res = CycleGuard::<ConfiguredGraphCycleDescriptor>::new(ctx)?
            .guard_this(compute_configured_target_node(self, ctx))
            .await
            .into_result(ctx)
            .await??;
        Ok(LookingUpConfiguredNodeContext::add_context(
            res,
            self.0.dupe(),
        )?)
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }

    fn provide<'a>(&'a self, demand: &mut Demand<'a>) {
        demand.provide_value_with(|| BuildSignalsNodeKey::new(self.dupe()))
    }
}

impl BuildSignalsNodeKeyImpl for ConfiguredTargetNodeKey {
    fn kind(&self) -> &'static str {
        "configure_target"
    }
}

#[async_trait]
impl ConfiguredTargetNodeCalculationImpl for ConfiguredTargetNodeCalculationInstance {
    async fn get_configured_target_node(
        &self,
        ctx: &mut DiceComputations<'_>,
        target: &ConfiguredTargetLabel,
        check_dependency_incompatibility: bool,
    ) -> kuro_error::Result<MaybeCompatible<ConfiguredTargetNode>> {
        let maybe_compatible_node = ctx
            .compute(&ConfiguredTargetNodeKey(target.dupe()))
            .await??;
        if check_dependency_incompatibility {
            if let MaybeCompatible::Incompatible(reason) = &maybe_compatible_node {
                if matches!(
                    &reason.cause,
                    &IncompatiblePlatformReasonCause::Dependency(_)
                ) {
                    if check_error_on_incompatible_dep(ctx, target.unconfigured_label()).await? {
                        return Err(reason.to_err().into());
                    }
                    soft_error!(
                        "dep_only_incompatible_version_two", reason.to_soft_err().into(),
                        quiet: false,
                        // Log at least one sample per unique package.
                        low_cardinality_key_for_additional_logview_samples: Some(Box::new(target.unconfigured().pkg())),
                    )?;
                    if let Some(custom_soft_errors) = get_dep_only_incompatible_custom_soft_error(
                        ctx,
                        target.unconfigured_label(),
                    )
                    .await?
                    {
                        for custom_soft_error in custom_soft_errors {
                            soft_error!(
                                &custom_soft_error,
                                reason.to_soft_err().into(),
                                quiet: true,
                                task: false,
                            )?;
                        }
                    }
                }
            }
        }
        Ok(maybe_compatible_node)
    }
}

async fn check_error_on_incompatible_dep(
    ctx: &mut DiceComputations<'_>,
    target_label: &TargetLabel,
) -> kuro_error::Result<bool> {
    if check_target_enabled_for_config(
        ctx,
        target_label,
        "kuro",
        "error_on_dep_only_incompatible_excluded",
    )
    .await?
    {
        return Ok(false);
    }
    check_target_enabled_for_config(ctx, target_label, "kuro", "error_on_dep_only_incompatible")
        .await
}

async fn check_target_enabled_for_config(
    ctx: &mut DiceComputations<'_>,
    target_label: &TargetLabel,
    section: &'static str,
    property: &'static str,
) -> kuro_error::Result<bool> {
    #[derive(Clone, Dupe, Display, Debug, Eq, Hash, PartialEq, Allocative)]
    #[display("ConfigPatternCalculation({section}, {property})")]
    struct ConfigPatternCalculation {
        section: &'static str,
        property: &'static str,
    }

    #[async_trait]
    impl Key for ConfigPatternCalculation {
        type Value = kuro_error::Result<Arc<Vec<ParsedPattern<TargetPatternExtra>>>>;

        async fn compute(
            &self,
            mut ctx: &mut DiceComputations,
            _cancellation: &CancellationContext,
        ) -> Self::Value {
            let cell_resolver = ctx.get_cell_resolver().await?;
            let root_cell = cell_resolver.root_cell();
            let alias_resolver = ctx.get_cell_alias_resolver(root_cell).await?;
            let root_conf = ctx.get_legacy_root_config_on_dice().await?;
            let patterns: Vec<String> = root_conf
                .view(&mut ctx)
                .parse_list(BuckconfigKeyRef {
                    section: self.section,
                    property: &self.property,
                })?
                .unwrap_or_default();

            let mut result = Vec::new();
            for pattern in patterns {
                result.push(ParsedPattern::parse_precise(
                    pattern.trim(),
                    root_cell,
                    &cell_resolver,
                    &alias_resolver,
                )?);
            }
            Ok(result.into())
        }

        fn equality(x: &Self::Value, y: &Self::Value) -> bool {
            match (x, y) {
                (Ok(x), Ok(y)) => x == y,
                _ => false,
            }
        }
    }

    let patterns = ctx
        .compute(&ConfigPatternCalculation { section, property })
        .await??;
    for pattern in patterns.iter() {
        if pattern.matches(target_label) {
            return Ok(true);
        }
    }

    Ok(false)
}

async fn get_dep_only_incompatible_custom_soft_error(
    ctx: &mut DiceComputations<'_>,
    target_label: &TargetLabel,
) -> kuro_error::Result<Option<Vec<ArcStr>>> {
    #[derive(Clone, Dupe, Display, Debug, Eq, Hash, PartialEq, Allocative)]
    struct GetDepOnlyIncompatibleInfo;

    #[async_trait]
    impl Key for GetDepOnlyIncompatibleInfo {
        type Value = kuro_error::Result<Option<Arc<DepOnlyIncompatibleCustomSoftErrors>>>;

        async fn compute(
            &self,
            mut ctx: &mut DiceComputations,
            _cancellation: &CancellationContext,
        ) -> Self::Value {
            let cell_resolver = ctx.get_cell_resolver().await?;
            let root_cell = cell_resolver.root_cell();
            let alias_resolver = ctx.get_cell_alias_resolver(root_cell).await?;
            let root_conf = ctx.get_legacy_root_config_on_dice().await?;
            let Some(target) = root_conf.view(&mut ctx).parse::<String>(BuckconfigKeyRef {
                section: "kuro",
                property: "dep_only_incompatible_info",
            })?
            else {
                return Ok(None);
            };
            let target =
                ProvidersLabel::parse(&target, root_cell.dupe(), &cell_resolver, &alias_resolver)?;
            let providers = ctx.get_configuration_analysis_result(&target).await?;
            let dep_only_incompatible_info = providers
                .provider_collection()
                .builtin_provider::<FrozenDepOnlyIncompatibleInfo>()
                .unwrap();
            let result = dep_only_incompatible_info.custom_soft_errors(
                root_cell,
                &cell_resolver,
                &alias_resolver,
            )?;
            Ok(Some(Arc::new(result)))
        }

        fn equality(x: &Self::Value, y: &Self::Value) -> bool {
            match (x, y) {
                (Ok(x), Ok(y)) => x == y,
                _ => false,
            }
        }
    }

    let Some(custom_soft_errors) = ctx.compute(&GetDepOnlyIncompatibleInfo).await?? else {
        return Ok(None);
    };
    let soft_error_categories: Vec<_> = custom_soft_errors
        .iter()
        .filter_map(|(soft_error_category, rollout_patterns)| {
            if rollout_patterns.matches(target_label) {
                Some(soft_error_category.dupe())
            } else {
                None
            }
        })
        .collect();
    Ok(Some(soft_error_categories))
}

#[allow(unused)]
fn _assert_compute_configured_target_node_no_transition_size() {
    const fn sz<F, T1, T2, T3, R>(_: &F) -> usize
    where
        F: FnOnce(T1, T2, T3) -> R,
    {
        std::mem::size_of::<R>()
    }

    const _: () = assert!(
        sz(&compute_configured_target_node_no_transition) <= 700,
        "compute_configured_target_node_no_transition size is larger than 700 bytes",
    );
}

#[allow(unused)]
fn _assert_compute_configured_forward_target_node_size() {
    const fn sz<F, T1, T2, T3, T4, R>(_: &F) -> usize
    where
        F: FnOnce(T1, T2, T3, T4) -> R,
    {
        std::mem::size_of::<R>()
    }

    const _: () = assert!(
        sz(&compute_configured_forward_target_node) <= 700,
        "compute_configured_forward_target_node size is larger than 700 bytes",
    );
}
