/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use dice::DiceTransaction;
use dice_futures::spawn::spawn_dropcancel;
use dupe::Dupe;
use futures::future::FutureExt;
use slug_analysis::analysis::calculation::profile_analysis;
use slug_cli_proto::TargetCfg;
use slug_cli_proto::profile_request::ProfileOpts;
use slug_cli_proto::target_profile::Action;
use slug_common::pattern::parse_from_cli::parse_and_resolve_patterns_from_cli_args;
use slug_core::package::PackageLabel;
use slug_core::pattern::pattern_type::ConfiguredProvidersPatternExtra;
use slug_core::pattern::pattern_type::TargetPatternExtra;
use slug_core::target::configured_target_label::ConfiguredTargetLabel;
use slug_error::BuckErrorContext;
use slug_error::internal_error;
use slug_fs::paths::abs_path::AbsPath;
use slug_interpreter::dice::starlark_provider::StarlarkEvalKind;
use slug_interpreter::starlark_profiler::config::GetStarlarkProfilerInstrumentation;
use slug_interpreter::starlark_profiler::config::StarlarkProfilerConfiguration;
use slug_interpreter::starlark_profiler::data::StarlarkProfileDataAndStats;
use slug_interpreter::starlark_profiler::mode::StarlarkProfileMode;
use slug_node::nodes::frontend::TargetGraphCalculation;
use slug_profile::get_profile_response;
use slug_profile::starlark_profiler_configuration_from_request;
use slug_server_ctx::ctx::ServerCommandContextTrait;
use slug_server_ctx::partial_result_dispatcher::NoPartialResult;
use slug_server_ctx::partial_result_dispatcher::PartialResultDispatcher;
use slug_server_ctx::pattern_parse_and_resolve::parse_and_resolve_patterns_to_targets_from_cli_args;
use slug_server_ctx::target_resolution_config::TargetResolutionConfig;
use slug_server_ctx::template::ServerCommandTemplate;
use slug_server_ctx::template::run_server_command;

async fn generate_profile_analysis(
    mut ctx: DiceTransaction,
    server_ctx: &dyn ServerCommandContextTrait,
    target_patterns: &[String],
    target_resolution_config: TargetResolutionConfig,
    profile_mode: &StarlarkProfilerConfiguration,
) -> slug_error::Result<Arc<StarlarkProfileDataAndStats>> {
    let targets = parse_and_resolve_patterns_to_targets_from_cli_args::<
        ConfiguredProvidersPatternExtra,
    >(&mut ctx, target_patterns, server_ctx.working_dir())
    .await?;

    let target_resolution_config = &target_resolution_config;
    let configured_targetss = ctx
        .try_compute_join(targets, |ctx, label| {
            async move {
                target_resolution_config
                    .get_configured_target(ctx, &label.target_label, None)
                    .await
            }
            .boxed()
        })
        .await?;

    let configured_targets: Vec<ConfiguredTargetLabel> =
        configured_targetss.into_iter().flatten().collect();

    match profile_mode {
        StarlarkProfilerConfiguration::ProfileAnalysis(..) => {
            profile_analysis(&mut ctx, &configured_targets)
                .await
                .buck_error_context("Recursive profile analysis failed")
                .map(Arc::new)
        }
        _ => Err(internal_error!("Incorrect profile mode")),
    }
}

async fn generate_profile_loading(
    ctx: &DiceTransaction,
    package: PackageLabel,
) -> slug_error::Result<StarlarkProfileDataAndStats> {
    // Self-check.
    let profile_mode = ctx
        .clone()
        .get_starlark_profiler_mode(&StarlarkEvalKind::LoadBuildFile(package.dupe()))
        .await?;
    match profile_mode {
        StarlarkProfileMode::None => {
            return Err(internal_error!("profile mode must be set in DICE"));
        }
        StarlarkProfileMode::Profile(_) => {}
    }

    let eval_result = ctx.clone().get_interpreter_results(package).await?;

    let starlark_profile = &eval_result
        .starlark_profile
        .as_ref()
        .internal_error("profile result must be set")?;
    Ok(StarlarkProfileDataAndStats::downcast(&***starlark_profile)?.clone())
}

pub async fn profile_command(
    ctx: &dyn ServerCommandContextTrait,
    partial_result_dispatcher: PartialResultDispatcher<NoPartialResult>,
    req: slug_cli_proto::ProfileRequest,
) -> slug_error::Result<slug_cli_proto::ProfileResponse> {
    run_server_command(ProfileServerCommand { req }, ctx, partial_result_dispatcher).await
}

struct ProfileServerCommand {
    req: slug_cli_proto::ProfileRequest,
}

#[async_trait]
impl ServerCommandTemplate for ProfileServerCommand {
    type StartEvent = slug_data::ProfileCommandStart;
    type EndEvent = slug_data::ProfileCommandEnd;
    type Response = slug_cli_proto::ProfileResponse;
    type PartialResult = NoPartialResult;

    async fn command(
        &self,
        server_ctx: &dyn ServerCommandContextTrait,
        _partial_result_dispatcher: PartialResultDispatcher<Self::PartialResult>,
        ctx: DiceTransaction,
    ) -> slug_error::Result<Self::Response> {
        let output = AbsPath::new(Path::new(&self.req.destination_path))?;

        let profile_mode =
            starlark_profiler_configuration_from_request(&self.req, server_ctx.project_root())?;

        match self
            .req
            .profile_opts
            .as_ref()
            .expect("Target profile not populated")
        {
            ProfileOpts::TargetProfile(opts) => {
                let action = slug_cli_proto::target_profile::Action::try_from(opts.action)
                    .buck_error_context("Invalid action")?;

                let profile_data = generate_profile(
                    server_ctx,
                    ctx,
                    &opts.target_patterns,
                    opts.target_cfg
                        .as_ref()
                        .internal_error("target_cfg not set")?,
                    &opts.target_universe,
                    action,
                    &profile_mode,
                )
                .await?;

                Ok(get_profile_response(
                    profile_data,
                    &opts.target_patterns,
                    output,
                )?)
            }
            _ => {
                return Err(slug_error::slug_error!(
                    slug_error::ErrorTag::Input,
                    "{}",
                    "Expected target profile opts, not BXL profile opts"
                ));
            }
        }
    }
}

async fn generate_profile(
    server_ctx: &dyn ServerCommandContextTrait,
    mut ctx: DiceTransaction,
    target_patterns: &[String],
    target_cfg: &TargetCfg,
    target_universe: &[String],
    action: Action,
    profile_mode: &StarlarkProfilerConfiguration,
) -> slug_error::Result<Arc<StarlarkProfileDataAndStats>> {
    let target_resolution_config =
        TargetResolutionConfig::from_args(&mut ctx, target_cfg, server_ctx, target_universe)
            .await?;

    match action {
        Action::Analysis => {
            generate_profile_analysis(
                ctx,
                server_ctx,
                target_patterns,
                target_resolution_config,
                profile_mode,
            )
            .await
        }
        Action::Loading => {
            let resolved = parse_and_resolve_patterns_from_cli_args::<TargetPatternExtra>(
                &mut ctx,
                &target_patterns,
                server_ctx.working_dir(),
            )
            .await?;

            let ctx = &ctx;
            let ctx_data = ctx.per_transaction_data();

            let profiles = slug_util::future::try_join_all(resolved.specs.into_iter().map(
                |(package_with_modifiers, _spec)| {
                    let ctx = ctx.dupe();
                    spawn_dropcancel(
                        move |_cancel| {
                            async move {
                                generate_profile_loading(&ctx, package_with_modifiers.package).await
                            }
                            .boxed()
                        },
                        &*ctx_data.spawner,
                        ctx_data,
                    )
                },
            ))
            .await?;

            Ok(StarlarkProfileDataAndStats::merge(profiles.iter()).map(Arc::new)?)
        }
    }
}
