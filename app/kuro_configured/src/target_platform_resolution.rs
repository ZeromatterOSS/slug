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

use async_trait::async_trait;
use dice::DiceComputations;
use dupe::Dupe;
use kuro_common::dice::cells::HasCellResolver;
use kuro_core::cells::name::CellName;
use kuro_core::cells::paths::CellRelativePath;
use kuro_core::configuration::build_setting::BuildSettingLabel;
use kuro_core::configuration::build_setting::BuildSettingValue;
use kuro_core::configuration::data::ConfigurationData;
use kuro_core::global_cfg_options::GlobalCfgOptions;
use kuro_core::package::PackageLabel;
use kuro_core::target::configured_target_label::ConfiguredTargetLabel;
use kuro_core::target::label::label::TargetLabel;
use kuro_core::target::name::TargetNameRef;
use kuro_core::target::target_configured_target_label::TargetConfiguredTargetLabel;
use kuro_error::BuckErrorContext;
use kuro_node::cfg_constructor::CFG_CONSTRUCTOR_CALCULATION_IMPL;
use kuro_node::configuration::target_platform_detector::TargetPlatformDetector;
use kuro_node::nodes::frontend::TargetGraphCalculation;
use kuro_node::nodes::unconfigured::RuleKind;
use kuro_node::nodes::unconfigured::TargetNode;
use kuro_node::super_package::SuperPackage;
use kuro_node::target_calculation::CONFIGURED_TARGET_CALCULATION;
use kuro_node::target_calculation::ConfiguredTargetCalculationImpl;

use crate::configuration::get_platform_configuration;
use crate::execution::get_execution_platform_toolchain_dep;

async fn get_target_platform_detector(
    _ctx: &mut DiceComputations<'_>,
) -> kuro_error::Result<Arc<TargetPlatformDetector>> {
    // `[parser] target_platform_detector_spec` is no longer parsed.
    // The Bazel-equivalent workspace default is `--platforms=X`
    // (already plumbed via `GlobalCfgOptions.target_platform`), which
    // takes precedence over this fallback in `get_configured_target`
    // and propagates to deps via configuration inheritance. The empty
    // detector lets the host-platform fallback below run when the
    // user supplies neither `--platforms` nor a per-target
    // `default_target_platform`.
    Ok(Arc::new(TargetPlatformDetector::empty()))
}

async fn get_default_platform(
    ctx: &mut DiceComputations<'_>,
    target: &TargetLabel,
) -> kuro_error::Result<ConfigurationData> {
    let detector = get_target_platform_detector(ctx).await?;
    if let Some(target) = detector.detect(target) {
        return get_platform_configuration(ctx, target)
            .await
            .map_err(kuro_error::Error::from);
    }

    // In Bazel/bzlmod mode, @local_config_platform//:host is auto-registered as the
    // host platform with detected OS/CPU constraints. Use it as the default target platform
    // so that select() expressions with @platforms// constraints work correctly.
    let resolver = ctx.get_cell_resolver().await?;
    let lcp_cell = CellName::unchecked_new("local_config_platform")?;
    if resolver.get(lcp_cell).is_ok() {
        let pkg = PackageLabel::new(lcp_cell, CellRelativePath::empty())?;
        let lcp_label = TargetLabel::new(pkg, TargetNameRef::new("host")?);
        match get_platform_configuration(ctx, &lcp_label).await {
            Ok(cfg) => return Ok(cfg),
            Err(e) => {
                tracing::debug!(
                    "Could not load @local_config_platform//:host as default platform: {e}"
                );
            }
        }
    }

    Ok(ConfigurationData::unspecified())
}

/// Canonical label of Bazel's `--compilation_mode` CLI flag. Matching Bazel's
/// internal pseudo-label so `select({"@bazel_tools//tools/cpp:opt": ...})` and
/// `ctx.fragments.cpp.compilation_mode` can read it uniformly.
const COMPILATION_MODE_LABEL: &str = "@bazel_tools//tools/cpp:compilation_mode";

/// Folds CLI-derived build settings (`--compilation_mode`,
/// `--//foo:bar=value`) into the top-level target `ConfigurationData`.
///
/// Called exactly once per top-level target configuration, at the end of
/// `get_configured_target`. Every downstream analysis that depends on this
/// cfg then sees the settings in `ConfigurationData.build_settings`.
/// Transitions and exec-platform construction preserve these entries since
/// `ConfigurationData::with_build_setting` only ever adds or overrides.
fn apply_cli_build_settings(cfg: ConfigurationData) -> kuro_error::Result<ConfigurationData> {
    let compilation_mode =
        kuro_build_api::interpreter::rule_defs::build_config::get_compilation_mode();
    let starlark_flags: Vec<(String, String)> =
        kuro_build_api::interpreter::rule_defs::build_config::get_all_starlark_flags()
            .into_iter()
            .collect();
    apply_cli_build_settings_with(cfg, &compilation_mode, &starlark_flags)
}

/// Pure helper that folds the given compilation_mode and starlark_flags into
/// `cfg.build_settings`. Separated from the globals-reading entry point so
/// unit tests can exercise it deterministically.
fn apply_cli_build_settings_with(
    cfg: ConfigurationData,
    compilation_mode: &str,
    starlark_flags: &[(String, String)],
) -> kuro_error::Result<ConfigurationData> {
    if !cfg.is_bound() {
        return Ok(cfg);
    }
    let mut out = cfg;
    let compile_label = BuildSettingLabel::from_bazel_label(COMPILATION_MODE_LABEL)?;
    out = out.with_build_setting(
        compile_label,
        BuildSettingValue::String(compilation_mode.to_owned()),
    )?;

    for (raw_label, raw_value) in starlark_flags {
        let label = match BuildSettingLabel::from_bazel_label(raw_label) {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!("skipping CLI flag `{raw_label}`: {e}");
                continue;
            }
        };
        // CLI flags are strings at parse time; user build-setting rules
        // that expect bool/int/list will coerce at consumption time
        // (e.g. in `ctx.build_setting_value`). Keep the raw string here so
        // the stored value round-trips the `--//foo:bar=…` CLI syntax.
        out = out.with_build_setting(label, BuildSettingValue::String(raw_value.clone()))?;
    }
    Ok(out)
}

struct ConfiguredTargetCalculationInstance;

pub(crate) fn init_configured_target_calculation() {
    CONFIGURED_TARGET_CALCULATION.init(&ConfiguredTargetCalculationInstance);
}

#[async_trait]
impl ConfiguredTargetCalculationImpl for ConfiguredTargetCalculationInstance {
    async fn get_configured_target(
        &self,
        ctx: &mut DiceComputations<'_>,
        target: &TargetLabel,
        global_cfg_options: &GlobalCfgOptions,
    ) -> kuro_error::Result<ConfiguredTargetLabel> {
        let (node, super_package) = ctx.get_target_node_with_super_package(target).await?;

        async fn get_platform_configuration_from_options(
            ctx: &mut DiceComputations<'_>,
            global_cfg_options: &GlobalCfgOptions,
            target: &TargetLabel,
            node: &TargetNode,
            super_package: &SuperPackage,
        ) -> kuro_error::Result<ConfigurationData> {
            let current_cfg = match global_cfg_options.target_platform.as_ref() {
                Some(global_target_platform) => {
                    get_platform_configuration(ctx, global_target_platform).await?
                }
                None => match node.get_default_target_platform() {
                    Some(target) => get_platform_configuration(ctx, target).await?,
                    None => get_default_platform(ctx, target).await?,
                },
            };

            let resolved = CFG_CONSTRUCTOR_CALCULATION_IMPL
                .get()?
                .eval_cfg_constructor(
                    ctx,
                    node.as_ref(),
                    super_package,
                    current_cfg,
                    &global_cfg_options.cli_modifiers,
                    node.rule_type(),
                )
                .await
                .with_buck_error_context(|| format!("Resolving modifiers for target `{target}`"))?;
            apply_cli_build_settings(resolved)
        }

        match node.rule_kind() {
            RuleKind::Configuration => Ok(target.configure(ConfigurationData::unbound())),
            RuleKind::Normal => Ok(target.configure(
                get_platform_configuration_from_options(
                    ctx,
                    global_cfg_options,
                    target,
                    &node,
                    &super_package,
                )
                .await?,
            )),
            RuleKind::Toolchain => {
                let cfg = get_platform_configuration_from_options(
                    ctx,
                    global_cfg_options,
                    target,
                    &node,
                    &super_package,
                )
                .await?;
                let exec_cfg = get_execution_platform_toolchain_dep(
                    ctx,
                    &TargetConfiguredTargetLabel::new_configure(target, cfg.dupe()),
                    node.as_ref(),
                )
                .await?
                // FIXME(JakobDegen): This is busted. Callers of this function expect to need to
                // subsequently actually configure the target, and handle any possible
                // incompatibilities at that time. Doing this here prevents them from handling those
                // as they would for non-toolchain targets.
                //
                // FIXME(JakobDegen): Write a test for the above.
                .require_compatible()?
                .cfg();
                Ok(target.configure_with_exec(cfg, exec_cfg.cfg().dupe()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use kuro_core::configuration::data::ConfigurationDataData;

    use super::*;

    fn make_cfg() -> ConfigurationData {
        ConfigurationData::from_platform(
            "cfg_for//:testing".to_owned(),
            ConfigurationDataData::empty(),
        )
        .unwrap()
    }

    #[test]
    fn cli_compilation_mode_lands_in_build_settings() -> kuro_error::Result<()> {
        let cfg = apply_cli_build_settings_with(make_cfg(), "opt", &[])?;
        let label = BuildSettingLabel::from_bazel_label(COMPILATION_MODE_LABEL)?;
        assert_eq!(
            cfg.get_build_setting(&label)?,
            Some(&BuildSettingValue::String("opt".to_owned()))
        );
        Ok(())
    }

    #[test]
    fn starlark_flags_land_in_build_settings() -> kuro_error::Result<()> {
        let flags = vec![
            ("//:my_flag".to_owned(), "baz".to_owned()),
            ("@foo//:feature".to_owned(), "on".to_owned()),
        ];
        let cfg = apply_cli_build_settings_with(make_cfg(), "fastbuild", &flags)?;
        let my_flag = BuildSettingLabel::from_bazel_label("//:my_flag")?;
        assert_eq!(
            cfg.get_build_setting(&my_flag)?,
            Some(&BuildSettingValue::String("baz".to_owned()))
        );
        let feature = BuildSettingLabel::from_bazel_label("@foo//:feature")?;
        assert_eq!(
            cfg.get_build_setting(&feature)?,
            Some(&BuildSettingValue::String("on".to_owned()))
        );
        Ok(())
    }

    /// Different `--compilation_mode` values produce configurations with
    /// distinct output hashes — this is what lets dbg-configured and
    /// opt-configured analyses coexist in the DICE cache without colliding.
    #[test]
    fn differing_compilation_modes_produce_distinct_cfgs() -> kuro_error::Result<()> {
        let opt = apply_cli_build_settings_with(make_cfg(), "opt", &[])?;
        let dbg = apply_cli_build_settings_with(make_cfg(), "dbg", &[])?;
        let fastbuild = apply_cli_build_settings_with(make_cfg(), "fastbuild", &[])?;
        assert_ne!(opt.output_hash(), dbg.output_hash());
        assert_ne!(opt.output_hash(), fastbuild.output_hash());
        assert_ne!(dbg.output_hash(), fastbuild.output_hash());
        Ok(())
    }

    /// Unparseable CLI flag labels are logged and skipped rather than
    /// failing the whole build.
    #[test]
    fn invalid_flag_label_is_skipped() -> kuro_error::Result<()> {
        let flags = vec![
            ("bogus".to_owned(), "x".to_owned()),
            ("//:good".to_owned(), "y".to_owned()),
        ];
        let cfg = apply_cli_build_settings_with(make_cfg(), "fastbuild", &flags)?;
        let good = BuildSettingLabel::from_bazel_label("//:good")?;
        assert_eq!(
            cfg.get_build_setting(&good)?,
            Some(&BuildSettingValue::String("y".to_owned()))
        );
        Ok(())
    }
}
