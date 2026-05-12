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
use derive_more::Display;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use futures::future::FutureExt;
use itertools::Itertools;
use slug_build_api::actions::execute::dice_data::HasFallbackExecutorConfig;
use slug_build_api::analysis::calculation::RuleAnalysisCalculation;
use slug_common::dice::cells::HasCellResolver;
use slug_core::configuration::compatibility::MaybeCompatible;
use slug_core::configuration::data::ConfigurationData;
use slug_core::configuration::pair::ConfigurationNoExec;
use slug_core::execution_types::execution::ExecutionPlatform;
use slug_core::execution_types::execution::ExecutionPlatformError;
use slug_core::execution_types::execution::ExecutionPlatformIncompatibleReason;
use slug_core::execution_types::execution::ExecutionPlatformResolution;
use slug_core::execution_types::execution_platforms::ExecutionPlatformFallback;
use slug_core::execution_types::execution_platforms::ExecutionPlatforms;
use slug_core::execution_types::execution_platforms::ExecutionPlatformsData;
use slug_core::provider::label::ProvidersLabel;
use slug_core::target::label::label::TargetLabel;
use slug_core::target::target_configured_target_label::TargetConfiguredTargetLabel;
use slug_error::BuckErrorContext;
use slug_node::attrs::configuration_context::AttrConfigurationContext;
use slug_node::attrs::configuration_context::AttrConfigurationContextImpl;
use slug_node::attrs::inspect_options::AttrInspectOptions;
use slug_node::attrs::spec::internal::EXEC_COMPATIBLE_WITH_ATTRIBUTE;
use slug_node::configuration::calculation::CellNameForConfigurationResolution;
use slug_node::configuration::resolved::ConfigurationSettingKey;
use slug_node::configuration::resolved::MatchedConfigurationSettingKeysWithCfg;
use slug_node::execution::GET_EXECUTION_PLATFORMS;
use slug_node::execution::GetExecutionPlatforms;
use slug_node::execution::GetExecutionPlatformsImpl;
use slug_node::nodes::configured::ConfiguredTargetNode;
use slug_node::nodes::configured_frontend::ConfiguredTargetNodeCalculation;
use slug_node::nodes::frontend::TargetGraphCalculation;
use slug_node::nodes::unconfigured::TargetNodeRef;
use starlark_map::ordered_map::OrderedMap;

use crate::configuration::compute_platform_cfgs;
use crate::configuration::get_matched_cfg_keys;
use crate::configuration::get_matched_cfg_keys_for_node;
use crate::nodes::GatheredDeps;
use crate::nodes::LookingUpConfiguredNodeContext;
use crate::nodes::gather_deps;

#[derive(Debug, slug_error::Error)]
#[slug(input)]
enum ExecutionPlatformComputationError {
    #[error("Can't find toolchain_dep execution platform using configuration `{0}`")]
    ToolchainDepMissingPlatform(ConfigurationData),
    #[error("Target `{0}` has a transition_dep, which is not permitted on a toolchain rule")]
    ToolchainTransitionDep(TargetLabel),
}

/// DICE injected key for the `--extra_execution_platforms` flag.
///
/// Populated by `slug_server::ctx` from the BuildRequest's
/// `CommonBuildOptions` before analysis runs. Read by
/// `compute_execution_platforms` (Plan 24 Phase 1) to surface
/// user-supplied labels as the highest-priority candidate exec
/// platforms (matches Bazel's "last flag wins" semantics).
///
/// Modeled as an `InjectedKey` (not `UserComputationData`) so DICE
/// invalidates downstream computations when the flag changes between
/// builds in the same daemon — without this, a build that errors with
/// `--extra_execution_platforms=A` would serve the cached failure to a
/// follow-up build with `--extra_execution_platforms=B`.
#[derive(Display, Debug, Hash, Eq, Clone, Dupe, PartialEq, Allocative)]
#[display("ExtraExecutionPlatformsKey")]
pub struct ExtraExecutionPlatformsKey;

impl dice::InjectedKey for ExtraExecutionPlatformsKey {
    type Value = Arc<[String]>;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

pub trait SetExtraExecutionPlatforms {
    fn set_extra_execution_platforms(&mut self, labels: Vec<String>) -> slug_error::Result<()>;
}

impl SetExtraExecutionPlatforms for dice::DiceTransactionUpdater {
    fn set_extra_execution_platforms(&mut self, labels: Vec<String>) -> slug_error::Result<()> {
        let value: Arc<[String]> = labels.into();
        Ok(self.changed_to(vec![(ExtraExecutionPlatformsKey, value)])?)
    }
}

/// Synthesizes a single-platform "legacy" `ExecutionPlatform` for
/// workspaces with **zero** registered exec platforms.
///
/// Plan 24 invariant: this function only fires from
/// `resolve_execution_platform` and `find_execution_platform_by_configuration`
/// when `compute_execution_platforms` returned `None` — i.e. the user
/// has set neither `--extra_execution_platforms`, nor
/// `register_execution_platforms()` in MODULE.bazel, nor
/// `build.execution_platforms` in buckconfig. With *any* of those, the
/// constraint-based path runs instead and unmatched constraints surface
/// `ExecutionPlatformError::NoCompatiblePlatform` (Phase 5: no silent
/// host fallback when registrations exist).
async fn legacy_execution_platform(
    ctx: &mut DiceComputations<'_>,
    target_cfg: &ConfigurationNoExec,
    exec_cfg: &ConfigurationNoExec,
) -> ExecutionPlatform {
    use futures::FutureExt;

    // Look up the *target* platform's exec_properties first — if the
    // user passed `--platforms=@…/platform_linux_x86_64` the target
    // cfg's PlatformInfo carries the RBE-shaped properties
    // (`OSFamily=…`, `container-image=…`) we want on every action's
    // RE Platform. Fall back to the exec cfg's platform (host) and
    // finally the daemon's fallback.
    //
    // `.boxed()` on the inner futures keeps the overall future size
    // bounded so this doesn't blow past
    // `compute_configured_target_node_no_transition`'s 700-byte cap
    // (rust async fns inline awaited futures into the parent's state
    // machine; chained option-fallbacks compound that quickly).
    let executor_config = match exec_platform_executor_config_from_cfg(ctx, target_cfg)
        .boxed()
        .await
    {
        Some(c) => c,
        None => match exec_platform_executor_config_from_cfg(ctx, exec_cfg)
            .boxed()
            .await
        {
            Some(c) => c,
            None => ctx.get_fallback_executor_config().clone(),
        },
    };
    ExecutionPlatform::legacy_execution_platform(executor_config, exec_cfg.dupe())
}

/// Bazel `--config=remote` flow: the user puts
/// `--platforms=@toolchains_buildbuddy//platforms:linux_x86_64` (or
/// equivalent) in their bazelrc, the platform's `PlatformInfo.exec_properties`
/// carries the right `OSFamily` / `Arch` / `container-image` keys that
/// drive RBE worker selection, and Bazel propagates those onto every
/// remote action's `Platform` message automatically. Slug used to drop
/// this signal: `legacy_execution_platform` returned the daemon's
/// fallback executor config (the `linux-remote-execution` placeholder
/// from `get_default_re_properties`), so RBE backends couldn't pick a
/// matching worker pool and compiles failed mid-execution with
/// `<optional>: No such file or directory` (the default container slug
/// targeted lacked a C++ toolchain).
///
/// Look up the cfg's platform label (e.g.
/// `buildbuddy_toolchain//:platform_linux_x86_64`), load its
/// `PlatformInfo`, and synthesize a `Hybrid` `CommandExecutorConfig`
/// whose `re_properties` are the platform's `exec_properties`. Returns
/// `None` for cfgs without an attached platform (`unbound`,
/// `unspecified`) or platforms without `exec_properties`, in which
/// case the caller falls back to the daemon's fallback config.
async fn exec_platform_executor_config_from_cfg(
    ctx: &mut DiceComputations<'_>,
    cfg: &ConfigurationNoExec,
) -> Option<Arc<slug_core::execution_types::executor_config::CommandExecutorConfig>> {
    use slug_build_api::interpreter::rule_defs::provider::builtin::platform_info::FrozenPlatformInfo;
    use slug_core::execution_types::executor_config::CacheUploadBehavior;
    use slug_core::execution_types::executor_config::CommandExecutorConfig;
    use slug_core::execution_types::executor_config::CommandGenerationOptions;
    use slug_core::execution_types::executor_config::Executor;
    use slug_core::execution_types::executor_config::HybridExecutionLevel;
    use slug_core::execution_types::executor_config::LocalExecutorOptions;
    use slug_core::execution_types::executor_config::MetaInternalExtraParams;
    use slug_core::execution_types::executor_config::PathSeparatorKind;
    use slug_core::execution_types::executor_config::RePlatformFields;
    use slug_core::execution_types::executor_config::RemoteEnabledExecutor;
    use slug_core::execution_types::executor_config::RemoteEnabledExecutorOptions;
    use slug_core::execution_types::executor_config::RemoteExecutorOptions;
    use slug_core::execution_types::executor_config::RemoteExecutorUseCase;
    use slug_core::pattern::pattern::ParsedPattern;
    use slug_core::pattern::pattern_type::TargetPatternExtra;
    use slug_core::provider::label::ProvidersLabel;

    let label_str = cfg.cfg().label().ok()?;
    // Parse `<cell>//<pkg>:<name>` into a TargetLabel using the root
    // cell as the parsing context.
    let cell_resolver = ctx.get_cell_resolver().await.ok()?;
    let root_cell = cell_resolver.root_cell();
    let alias_resolver = ctx.get_cell_alias_resolver(root_cell).await.ok()?;
    let parsed = ParsedPattern::<TargetPatternExtra>::parse_precise(
        label_str,
        root_cell,
        &cell_resolver,
        &alias_resolver,
    )
    .ok()?;
    let (target_label, TargetPatternExtra) = parsed.as_literal(label_str).ok()?;
    let providers_label = ProvidersLabel::default_for(target_label);
    let providers = ctx
        .get_configuration_analysis_result(&providers_label)
        .await
        .ok()?;
    let platform_info = providers
        .provider_collection()
        .builtin_provider::<FrozenPlatformInfo>()?;
    // RE platform messages take opaque string keys (`OSFamily`,
    // `container-image`, `Arch`, …). Filter out exec_properties whose
    // keys parse as Bazel labels — those are the build-setting-style
    // entries (e.g. `@bazel_tools//tools/cpp:compilation_mode = "opt"`)
    // that flow into ConfigurationData.build_settings, NOT to RE
    // worker selection.
    let entries: Vec<(String, String)> = platform_info
        .exec_properties_entries()
        .into_iter()
        .filter(|(k, _)| !k.starts_with('@') && !k.starts_with("//") && !k.contains("//"))
        .collect();
    if entries.is_empty() {
        return None;
    }
    let re_properties = RePlatformFields {
        properties: Arc::new(entries.into_iter().collect()),
    };
    Some(Arc::new(CommandExecutorConfig {
        executor: Executor::RemoteEnabled(RemoteEnabledExecutorOptions {
            executor: RemoteEnabledExecutor::Hybrid {
                local: LocalExecutorOptions::default(),
                remote: RemoteExecutorOptions::default(),
                level: HybridExecutionLevel::Limited,
            },
            re_properties,
            re_use_case: RemoteExecutorUseCase::slug_default(),
            re_action_key: None,
            cache_upload_behavior: CacheUploadBehavior::Disabled,
            remote_cache_enabled: true,
            remote_dep_file_cache_enabled: false,
            dependencies: Vec::new(),
            custom_image: None,
            meta_internal_extra_params: MetaInternalExtraParams::default(),
        }),
        options: CommandGenerationOptions {
            path_separator: PathSeparatorKind::system_default(),
            output_paths_behavior: Default::default(),
            use_bazel_protocol_remote_persistent_workers: false,
        },
    }))
}

/// Builds the cfg that slug uses as the legacy exec-configuration when
/// `build.execution_platforms` is not set.
///
/// Bazel's rule: omitting `register_execution_platforms()` means exec cfg
/// == target cfg (the target platform doubles as the exec platform). Slug
/// matches that today: pass `target_cfg` through when it's bound to a
/// real platform.
///
/// Fallback: when `target_cfg` is unbound (no `--target-platforms` /
/// `--host_platform` resolved to a real `platform()`), load
/// `@local_config_platform//:host`'s `PlatformInfo` and return its cfg.
/// This keeps Plan 19.3's `platform(exec_properties = {...})` defaults
/// (e.g. `compilation_mode = "opt"`) working for default builds while
/// no longer overriding a user-configured target/host platform whose
/// constraints (e.g. `@llvm//constraints/libc:gnu.2.28`) the build
/// actually depends on.
///
/// History: this used to *unconditionally* substitute lcp, which broke
/// zeromatter-style workspaces that set `--host_platform` to a custom
/// `platform()` carrying extra constraints. The substitution stripped
/// those constraints from the exec config and downstream `select()`
/// chains (rules_rs `RESOLVED_PLATFORMS`, rules_cc per-libc) fell
/// through to `@platforms//:incompatible`. See Plan 24 §5 (host-fallback
/// retirement).
async fn legacy_exec_cfg(
    ctx: &mut DiceComputations<'_>,
    target_cfg: &ConfigurationNoExec,
) -> ConfigurationNoExec {
    use slug_build_api::interpreter::rule_defs::provider::builtin::platform_info::FrozenPlatformInfo;
    use slug_core::cells::name::CellName;
    use slug_core::cells::paths::CellRelativePath;
    use slug_core::package::PackageLabel;
    use slug_core::target::name::TargetNameRef;

    // Bazel-shape: when target cfg is bound to a real platform, exec cfg
    // mirrors it (no `register_execution_platforms()` ⇒ exec == target).
    if target_cfg.cfg().is_bound() {
        return target_cfg.dupe();
    }

    // Unbound target cfg → use lcp/host as the legacy fallback.
    let lcp_cell = match CellName::unchecked_new("local_config_platform") {
        Ok(name) => name,
        Err(_) => return target_cfg.dupe(),
    };
    match ctx.get_cell_resolver().await {
        Ok(resolver) if resolver.get(lcp_cell).is_ok() => {}
        _ => return target_cfg.dupe(),
    };
    let pkg = match PackageLabel::new(lcp_cell, CellRelativePath::empty()) {
        Ok(p) => p,
        Err(_) => return target_cfg.dupe(),
    };
    let target_name = match TargetNameRef::new("host") {
        Ok(n) => n,
        Err(_) => return target_cfg.dupe(),
    };
    let host_label = TargetLabel::new(pkg, target_name);
    let providers_label = ProvidersLabel::default_for(host_label);
    let providers = match ctx
        .get_configuration_analysis_result(&providers_label)
        .await
    {
        Ok(p) => p,
        Err(_) => return target_cfg.dupe(),
    };
    let Some(platform_info) = providers
        .provider_collection()
        .builtin_provider::<FrozenPlatformInfo>()
    else {
        return target_cfg.dupe();
    };
    match platform_info.to_configuration() {
        Ok(cfg) => ConfigurationNoExec::new(cfg),
        Err(_) => target_cfg.dupe(),
    }
}

pub async fn find_execution_platform_by_configuration(
    ctx: &mut DiceComputations<'_>,
    exec_cfg: &ConfigurationData,
    cfg: &ConfigurationData,
) -> slug_error::Result<ExecutionPlatform> {
    match ctx.get_execution_platforms().await? {
        Some(platforms) if exec_cfg != &ConfigurationData::unbound_exec() => {
            for c in platforms.candidates() {
                if c.cfg() == exec_cfg {
                    return Ok(c.dupe());
                }
            }
            Err(slug_error::Error::from(
                ExecutionPlatformComputationError::ToolchainDepMissingPlatform(exec_cfg.dupe()),
            ))
        }
        _ => {
            let target_cfg = ConfigurationNoExec::new(cfg.dupe());
            let exec_cfg = legacy_exec_cfg(ctx, &target_cfg).await;
            Ok(legacy_execution_platform(ctx, &target_cfg, &exec_cfg).await)
        }
    }
}

struct ExecutionPlatformConstraints {
    exec_deps: Arc<[TargetLabel]>,
    toolchain_deps: Arc<[TargetConfiguredTargetLabel]>,
    exec_compatible_with: Arc<[ConfigurationSettingKey]>,
}

impl ExecutionPlatformConstraints {
    fn new_constraints(
        exec_deps: Arc<[TargetLabel]>,
        toolchain_deps: Arc<[TargetConfiguredTargetLabel]>,
        exec_compatible_with: Arc<[ConfigurationSettingKey]>,
    ) -> Self {
        Self {
            exec_deps,
            toolchain_deps,
            exec_compatible_with,
        }
    }

    fn new(
        node: TargetNodeRef,
        gathered_deps: &GatheredDeps,
        cfg_ctx: &(dyn AttrConfigurationContext + Sync),
    ) -> slug_error::Result<Self> {
        let exec_compatible_with: Arc<[_]> = if let Some(a) =
            node.known_attr_or_none(EXEC_COMPATIBLE_WITH_ATTRIBUTE.id, AttrInspectOptions::All)
        {
            let configured_attr = a.configure(cfg_ctx).with_buck_error_context(|| {
                format!(
                    "Error configuring attribute `{}` to resolve execution platform",
                    EXEC_COMPATIBLE_WITH_ATTRIBUTE.name
                )
            })?;
            ConfiguredTargetNode::attr_as_target_compatible_with(configured_attr.value)
                .map(|label| {
                    label.with_buck_error_context(|| {
                        format!("attribute `{}`", EXEC_COMPATIBLE_WITH_ATTRIBUTE.name)
                    })
                })
                .collect::<Result<_, _>>()?
        } else {
            Arc::new([])
        };

        Ok(Self::new_constraints(
            gathered_deps
                .exec_deps
                .iter()
                .map(|c| c.0.target().unconfigured().dupe())
                .collect(),
            gathered_deps
                .toolchain_deps
                .iter()
                .map(|c| c.dupe())
                .collect(),
            exec_compatible_with,
        ))
    }

    /// Gets the compatible execution platforms for a give list of compatible_with constraints and execution deps.
    ///
    /// We do this as a sort of monolithic computation (rather than checking things one-by-one or separating
    /// compatible_with and exec deps) because we expect those values to be common across many nodes (for example,
    /// all c++ targets targeting a specific platform are likely to share compatible_with and exec_deps except
    /// for some rare exceptions). By having a monolithic key like `(Vec<TargetLabel>, Vec<TargetLabel>)` allows all
    /// those nodes to just have a single dice dep. This approach has the downside that it is less incremental, but
    /// we expect these things to change rarely.
    async fn one_for_cell(
        self,
        ctx: &mut DiceComputations<'_>,
        cell: CellNameForConfigurationResolution,
    ) -> slug_error::Result<ExecutionPlatformResolution> {
        ctx.compute(&ExecutionPlatformResolutionKey {
            target_node_cell: cell,
            exec_compatible_with: self.exec_compatible_with,
            exec_deps: self.exec_deps,
            toolchain_deps: self.toolchain_deps,
        })
        .await?
    }
}

#[derive(Clone, Display, Debug, Dupe, Eq, Hash, PartialEq, Allocative)]
#[display(
        "ToolchainExecutionPlatformCompatibilityKey({}, {})",
        target,
        exec_platform.id()
    )]
pub(crate) struct ToolchainExecutionPlatformCompatibilityKey {
    target: TargetConfiguredTargetLabel,
    exec_platform: ExecutionPlatform,
}

impl ToolchainExecutionPlatformCompatibilityKey {
    async fn compute_impl(
        &self,
        ctx: &mut DiceComputations<'_>,
    ) -> slug_error::Result<Result<(), ExecutionPlatformIncompatibleReason>> {
        let node = ctx.get_target_node(self.target.unconfigured()).await?;
        if node.transition_deps().next().is_some() {
            // We could actually check this when defining the rule, but a bit of a corner
            // case, and much simpler to do so here.
            return Err(slug_error::Error::from(
                ExecutionPlatformComputationError::ToolchainTransitionDep(
                    self.target.unconfigured().dupe(),
                ),
            ));
        }
        let cell_name = CellNameForConfigurationResolution(self.target.pkg().cell_name());
        let matched_cfg_keys =
            get_matched_cfg_keys_for_node(ctx, self.target.cfg(), cell_name, node.as_ref()).await?;
        let platform_cfgs = compute_platform_cfgs(ctx, node.as_ref()).await?;
        // We don't really need `resolved_transitions` here:
        // `Traversal` declared above ignores transitioned dependencies.
        // But we pass `resolved_transitions` here to prevent breakages in the future
        // if something here changes.
        let resolved_transitions = OrderedMap::new();
        let cfg_ctx = AttrConfigurationContextImpl::new(
            &matched_cfg_keys,
            ConfigurationNoExec::unbound_exec(),
            &resolved_transitions,
            &platform_cfgs,
        );
        let (gathered_deps, errors_and_incompats) =
            gather_deps(&self.target, node.as_ref(), &cfg_ctx, ctx).await?;
        if let Some(ret) = errors_and_incompats.finalize() {
            // Statically assert that we hit one of the `?`s
            enum Void {}
            let _: Void = ret?.require_compatible()?;
        }
        let constraints =
            ExecutionPlatformConstraints::new(node.as_ref(), &gathered_deps, &cfg_ctx)?;

        check_execution_platform(
            ctx,
            cell_name,
            &constraints.exec_compatible_with,
            &constraints.exec_deps,
            &self.exec_platform,
            &constraints.toolchain_deps,
        )
        .await
    }
}

#[async_trait]
impl Key for ToolchainExecutionPlatformCompatibilityKey {
    type Value = slug_error::Result<Result<(), ExecutionPlatformIncompatibleReason>>;
    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellation: &CancellationContext,
    ) -> Self::Value {
        Ok(LookingUpConfiguredNodeContext::add_context(
            self.compute_impl(ctx).await,
            self.target.inner().dupe(),
        )?)
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }
}

async fn check_toolchain_execution_platform_compatibility(
    ctx: &mut DiceComputations<'_>,
    target: TargetConfiguredTargetLabel,
    exec_platform: ExecutionPlatform,
) -> slug_error::Result<Result<(), ExecutionPlatformIncompatibleReason>> {
    ctx.compute(&ToolchainExecutionPlatformCompatibilityKey {
        target,
        exec_platform,
    })
    .await?
}

pub(crate) async fn get_execution_platform_toolchain_dep(
    ctx: &mut DiceComputations<'_>,
    target_label: &TargetConfiguredTargetLabel,
    target_node: TargetNodeRef<'_>,
) -> slug_error::Result<MaybeCompatible<ExecutionPlatformResolution>> {
    assert!(target_node.is_toolchain_rule());
    let target_cfg = target_label.cfg();
    let target_cell = target_node.label().pkg().cell_name();
    let matched_cfg_keys = get_matched_cfg_keys_for_node(
        ctx,
        target_cfg,
        CellNameForConfigurationResolution(target_cell),
        target_node,
    )
    .await?;
    if target_node.transition_deps().next().is_some() {
        Err(slug_error::Error::from(
            ExecutionPlatformComputationError::ToolchainTransitionDep(
                target_label.unconfigured().dupe(),
            ),
        ))
    } else {
        let platform_cfgs = compute_platform_cfgs(ctx, target_node).await?;
        let resolved_transitions = OrderedMap::new();
        let cfg_ctx = AttrConfigurationContextImpl::new(
            &matched_cfg_keys,
            ConfigurationNoExec::unbound_exec(),
            &resolved_transitions,
            &platform_cfgs,
        );
        let (gathered_deps, errors_and_incompats) =
            gather_deps(target_label, target_node, &cfg_ctx, ctx).await?;
        if let Some(ret) = errors_and_incompats.finalize() {
            return ret.map_err(Into::into);
        }
        Ok(MaybeCompatible::Compatible(
            resolve_execution_platform(
                ctx,
                target_node,
                &matched_cfg_keys,
                &gathered_deps,
                &cfg_ctx,
            )
            .await?,
        ))
    }
}

pub(crate) async fn resolve_execution_platform(
    ctx: &mut DiceComputations<'_>,
    node: TargetNodeRef<'_>,
    matched_cfg_keys: &MatchedConfigurationSettingKeysWithCfg,
    gathered_deps: &GatheredDeps,
    cfg_ctx: &(dyn AttrConfigurationContext + Sync),
) -> slug_error::Result<ExecutionPlatformResolution> {
    // If no execution platforms are configured, we fall back to the legacy execution
    // platform behavior: a single executor config (the fallback config) with an exec
    // cfg derived from `@local_config_platform//:host` (Plan 20.1). Using the host
    // cfg rather than the target cfg activates the exec_properties defaults folded
    // in by Plan 19.3 (`compilation_mode = "opt"` for tool builds).
    // The non-none case will be handled when we invoke the resolve_execution_platform() on ctx below, the none
    // case can't be handled there because we don't pass the full configuration into it.
    if ctx.get_execution_platforms().await?.is_none() {
        let target_cfg = matched_cfg_keys.cfg();
        let exec_cfg = legacy_exec_cfg(ctx, target_cfg).await;
        return Ok(ExecutionPlatformResolution::new(
            Some(legacy_execution_platform(ctx, target_cfg, &exec_cfg).await),
            Vec::new(),
        ));
    };

    let constraints = ExecutionPlatformConstraints::new(node, gathered_deps, cfg_ctx)?;
    constraints
        .one_for_cell(
            ctx,
            CellNameForConfigurationResolution(node.label().pkg().cell_name()),
        )
        .await
}

/// Load a single execution-platform candidate from a label string.
///
/// Bazel's `register_execution_platforms()` and `--extra_execution_platforms`
/// flag both accept labels of `platform()` or `execution_platform()` rules.
/// This helper accepts either:
///
/// - `execution_platform()` → produces `ExecutionPlatformInfo`, used directly.
/// - `platform()` → produces `PlatformInfo`. Synthesized into an
///   `ExecutionPlatform` whose `re_properties` come from the platform's
///   `exec_properties` (filtered to the opaque-key RE entries — label-shaped
///   keys land in `build_settings`, not `re_properties`).
///
/// Returns `Ok(None)` when the label parses but doesn't expose either
/// provider. Returns `Err` for parse failures or analysis failures so
/// misspelled labels surface loudly rather than being silently dropped.
async fn load_platform_candidate(
    ctx: &mut DiceComputations<'_>,
    label_str: &str,
) -> slug_error::Result<Option<ExecutionPlatform>> {
    use slug_build_api::interpreter::rule_defs::provider::builtin::execution_platform_info::FrozenExecutionPlatformInfo;
    use slug_build_api::interpreter::rule_defs::provider::builtin::platform_info::FrozenPlatformInfo;

    let cells = ctx.get_cell_resolver().await?;
    let root_cell = cells.root_cell();
    let alias_resolver = ctx.get_cell_alias_resolver(root_cell).await?;
    let target_label = TargetLabel::parse(label_str, root_cell, &cells, &alias_resolver)?;
    let providers_label = ProvidersLabel::default_for(target_label.dupe());
    let providers = ctx
        .get_configuration_analysis_result(&providers_label)
        .await?;
    let collection = providers.provider_collection();

    if let Some(info) = collection.builtin_provider::<FrozenExecutionPlatformInfo>() {
        return Ok(Some(info.to_execution_platform()?));
    }

    if let Some(platform_info) = collection.builtin_provider::<FrozenPlatformInfo>() {
        let cfg = platform_info.to_configuration()?;
        let cfg_no_exec = ConfigurationNoExec::new(cfg.dupe());
        let executor_config = match exec_platform_executor_config_from_cfg(ctx, &cfg_no_exec)
            .boxed()
            .await
        {
            Some(c) => c,
            None => ctx.get_fallback_executor_config().clone(),
        };
        return Ok(Some(ExecutionPlatform::platform(
            target_label,
            cfg,
            executor_config,
        )));
    }

    Ok(None)
}

/// Returns the configured [ExecutionPlatforms] or `None` if no source supplies any.
///
/// Sources, in priority order (matches Bazel: last flag wins, root module beats deps):
///
/// 1. **CLI** (`--extra_execution_platforms`) — top of the candidate list,
///    so a user-supplied platform overrides any module/buckconfig
///    registration when constraints match.
/// 2. **MODULE.bazel** (`register_execution_platforms()`) — collected by
///    `slug_bzlmod` in BFS module order (root first), then transitive deps.
/// 3. **`build.execution_platforms` buckconfig** — slug's legacy single
///    registration target (an `execution_platforms()` rule that exposes
///    `FrozenExecutionPlatformRegistrationInfo` containing multiple
///    inner platforms). Preserved for backward compat.
///
/// Returns `None` only when every source is empty, so the caller falls
/// back to `legacy_execution_platform`. Otherwise returns a candidate
/// list with `Fallback::Error` (matching Bazel's "no compatible
/// execution platform" error semantics) — except when source 3 is the
/// only source, in which case its `fallback()` is honored.
async fn compute_execution_platforms(
    ctx: &mut DiceComputations<'_>,
) -> slug_error::Result<Option<ExecutionPlatforms>> {
    let cells = ctx.get_cell_resolver().await?;
    let root_cell = cells.root_cell();
    let alias_resolver = ctx.get_cell_alias_resolver(root_cell).await?;

    let cli_extras: Arc<[String]> = ctx.compute(&ExtraExecutionPlatformsKey).await?;
    let cli_extras: Vec<String> = cli_extras.iter().cloned().collect();
    let module_registrations = slug_bzlmod::get_registered_execution_platforms();

    if cli_extras.is_empty() && module_registrations.is_empty() {
        return Ok(None);
    }

    let mut platforms: Vec<ExecutionPlatform> = Vec::new();
    let mut display_target: Option<TargetLabel> = None;

    for label_str in cli_extras.iter().chain(module_registrations.iter()) {
        if let Some(p) = load_platform_candidate(ctx, label_str).boxed().await? {
            if display_target.is_none() {
                display_target = Some(TargetLabel::parse(
                    label_str,
                    root_cell,
                    &cells,
                    &alias_resolver,
                )?);
            }
            platforms.push(p);
        }
    }

    if platforms.is_empty() {
        return Ok(None);
    }

    let display_target = display_target
        .internal_error("compute_execution_platforms produced platforms with no display target")?;
    // CLI/MODULE registrations are the only source — use Bazel's "no
    // compatible platform → error" semantics so misconfigurations
    // surface loudly instead of silently routing to the host.
    let fallback = ExecutionPlatformFallback::Error;

    Ok(Some(Arc::new(ExecutionPlatformsData::new(
        display_target,
        platforms,
        fallback,
    ))))
}

/// Check if a particular execution platform is compatible with the constraints or not.
/// Return either Ok/Ok if it is, or a reason if not.
async fn check_execution_platform(
    ctx: &mut DiceComputations<'_>,
    target_node_cell: CellNameForConfigurationResolution,
    exec_compatible_with: &[ConfigurationSettingKey],
    exec_deps: &[TargetLabel],
    exec_platform: &ExecutionPlatform,
    toolchain_deps: &[TargetConfiguredTargetLabel],
) -> slug_error::Result<Result<(), ExecutionPlatformIncompatibleReason>> {
    let matched_cfg_keys = get_matched_cfg_keys(
        ctx,
        exec_platform.cfg(),
        target_node_cell,
        exec_compatible_with,
    )
    .await?;

    // Then check if the platform satisfies compatible_with
    for constraint in exec_compatible_with {
        if matched_cfg_keys
            .settings()
            .setting_matches(constraint)
            .is_none()
        {
            return Ok(Err(
                ExecutionPlatformIncompatibleReason::ConstraintNotSatisfied(constraint.dupe().0),
            ));
        }
    }

    // Then check that all exec_deps are compatible with the platform. We collect errors separately,
    // so that we do not report an error if we would later find an incompatibility.
    let dep_results = ctx
        .compute_join(exec_deps.iter(), |ctx, dep| {
            Box::pin(async move {
                let cfg_pair = exec_platform.cfg_pair_no_exec().dupe();
                let cfg = exec_platform.cfg().dupe();
                let result = ctx
                    .get_internal_configured_target_node(&dep.configure_pair_no_exec(cfg_pair))
                    .await;
                match result {
                    Ok(MaybeCompatible::Compatible(_)) => Ok(None),
                    Ok(MaybeCompatible::Incompatible(reason)) => Ok(Some(reason)),
                    Err(e) => Err(e.context(format!(
                        "Error checking compatibility of `{}` with `{}`",
                        dep, cfg
                    ))),
                }
            })
        })
        .await;

    let mut errs = Vec::new();
    for result in dep_results {
        match result {
            Ok(None) => (),
            Ok(Some(reason)) => {
                return Ok(Err(
                    ExecutionPlatformIncompatibleReason::ExecutionDependencyIncompatible(
                        reason.dupe(),
                    ),
                ));
            }
            Err(e) => errs.push(e),
        };
    }

    for result in ctx
        .compute_join(toolchain_deps.iter(), |ctx, dep| {
            let dep = dep.dupe();
            let exec_platform = exec_platform.dupe();
            async move {
                check_toolchain_execution_platform_compatibility(ctx, dep, exec_platform).await
            }
            .boxed()
        })
        .await
    {
        match result {
            Ok(Ok(())) => {}
            Ok(Err(reason)) => {
                return Ok(Err(reason));
            }
            Err(e) => errs.push(e),
        }
    }
    if let Some(e) = errs.pop() {
        return Err(e.into());
    }

    Ok(Ok(()))
}

async fn get_execution_platforms_enabled(
    ctx: &mut DiceComputations<'_>,
) -> slug_error::Result<ExecutionPlatforms> {
    ctx.get_execution_platforms()
        .await?
        .buck_error_context("Execution platforms are not enabled")
}

async fn resolve_execution_platform_from_constraints(
    ctx: &mut DiceComputations<'_>,
    target_node_cell: CellNameForConfigurationResolution,
    exec_compatible_with: &[ConfigurationSettingKey],
    exec_deps: &[TargetLabel],
    toolchain_deps: &[TargetConfiguredTargetLabel],
) -> slug_error::Result<ExecutionPlatformResolution> {
    let mut skipped = Vec::new();
    let execution_platforms = get_execution_platforms_enabled(ctx).await?;
    for exec_platform in execution_platforms.candidates() {
        match check_execution_platform(
            ctx,
            target_node_cell,
            exec_compatible_with,
            exec_deps,
            exec_platform,
            toolchain_deps,
        )
        .await?
        {
            Ok(()) => {
                return Ok(ExecutionPlatformResolution::new(
                    Some(exec_platform.dupe()),
                    skipped,
                ));
            }
            Err(reason) => {
                skipped.push((exec_platform.id(), reason));
            }
        }
    }

    match execution_platforms.fallback() {
        ExecutionPlatformFallback::UseUnspecifiedExec => {
            Ok(ExecutionPlatformResolution::new(None, skipped))
        }
        ExecutionPlatformFallback::Error => {
            Err(ExecutionPlatformError::NoCompatiblePlatform(Arc::new(skipped).into()).into())
        }
        ExecutionPlatformFallback::Platform(platform) => Ok(ExecutionPlatformResolution::new(
            Some(platform.dupe()),
            skipped,
        )),
    }
}

#[derive(Clone, Dupe, Debug, Eq, Hash, PartialEq, Allocative)]
pub(crate) struct ExecutionPlatformResolutionKey {
    /// Determining a compatible execution platform requires checking the target and toolchain's
    /// exec_compatible_with. This in turn requires a ResolvedConfiguration, which resolves the
    /// buckconfig-related config_setting values based on the cell of the target the configuration
    /// is being resolved for.
    target_node_cell: CellNameForConfigurationResolution,
    exec_compatible_with: Arc<[ConfigurationSettingKey]>,
    exec_deps: Arc<[TargetLabel]>,
    toolchain_deps: Arc<[TargetConfiguredTargetLabel]>,
}

impl Display for ExecutionPlatformResolutionKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Resolving execution platform: cell:{}",
            self.target_node_cell
        )?;

        if !self.exec_compatible_with.is_empty() {
            write!(
                f,
                ", exec_compatible_with=[{}]",
                self.exec_compatible_with.iter().join(", ")
            )?
        }

        if !self.exec_deps.is_empty() {
            write!(f, ", exec_deps=[{}]", self.exec_deps.iter().join(", "))?
        }

        if !self.toolchain_deps.is_empty() {
            write!(
                f,
                ", toolchain_deps=[{}]",
                self.toolchain_deps.iter().join(", ")
            )?;
        }

        Ok(())
    }
}

#[async_trait]
impl Key for ExecutionPlatformResolutionKey {
    type Value = slug_error::Result<ExecutionPlatformResolution>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellation: &CancellationContext,
    ) -> Self::Value {
        resolve_execution_platform_from_constraints(
            ctx,
            self.target_node_cell,
            &self.exec_compatible_with,
            &self.exec_deps,
            &self.toolchain_deps,
        )
        .await
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }
}

#[derive(Clone, Dupe, Display, Debug, Eq, Hash, PartialEq, Allocative)]
#[display("ExecutionPlatforms")]
pub struct ExecutionPlatformsKey;

#[async_trait]
impl Key for ExecutionPlatformsKey {
    type Value = slug_error::Result<Option<ExecutionPlatforms>>;
    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellation: &CancellationContext,
    ) -> Self::Value {
        compute_execution_platforms(ctx).await
    }

    fn equality(_: &Self::Value, _: &Self::Value) -> bool {
        // TODO(cjhopman) should these be comparable for caching
        false
    }
}

struct GetExecutionPlatformsInstance;

#[async_trait]
impl GetExecutionPlatformsImpl for GetExecutionPlatformsInstance {
    async fn get_execution_platforms_impl(
        &self,
        ctx: &mut DiceComputations<'_>,
    ) -> slug_error::Result<Option<ExecutionPlatforms>> {
        ctx.compute(&ExecutionPlatformsKey).await?
    }

    async fn execution_platform_resolution_one_for_cell(
        &self,
        dice: &mut DiceComputations<'_>,
        exec_deps: Arc<[TargetLabel]>,
        toolchain_deps: Arc<[TargetConfiguredTargetLabel]>,
        exec_compatible_with: Arc<[ConfigurationSettingKey]>,
        cell: CellNameForConfigurationResolution,
    ) -> slug_error::Result<ExecutionPlatformResolution> {
        ExecutionPlatformConstraints::new_constraints(
            exec_deps,
            toolchain_deps,
            exec_compatible_with,
        )
        .one_for_cell(dice, cell)
        .await
    }
}

pub(crate) fn init_get_execution_platforms() {
    GET_EXECUTION_PLATFORMS.init(&GetExecutionPlatformsInstance);
}
