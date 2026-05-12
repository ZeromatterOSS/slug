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
use dice::DiceTransaction;
use dupe::Dupe;
use futures::future::FutureExt;
use gazebo::prelude::VecExt;
use slug_artifact::artifact::artifact_type::Artifact;
use slug_build_api::actions::artifact::get_artifact_fs::GetArtifactFs;
use slug_build_api::analysis::calculation::RuleAnalysisCalculation;
use slug_cli_proto::TargetsRequest;
use slug_cli_proto::TargetsShowOutputsResponse;
use slug_cli_proto::targets_show_outputs_response::TargetPaths;
use slug_common::pattern::parse_from_cli::parse_patterns_from_cli_args;
use slug_common::pattern::resolve::ResolveTargetPatterns;
use slug_common::pattern::resolve::ResolvedPattern;
use slug_core::global_cfg_options::GlobalCfgOptions;
use slug_core::package::PackageLabel;
use slug_core::pattern::pattern::PackageSpec;
use slug_core::pattern::pattern::ParsedPattern;
use slug_core::pattern::pattern_type::ProvidersPatternExtra;
use slug_core::provider::label::ConfiguredProvidersLabel;
use slug_core::provider::label::ProvidersLabel;
use slug_core::target::label::label::TargetLabel;
use slug_error::BuckErrorContext;
use slug_execute::artifact::artifact_dyn::ArtifactDyn;
use slug_node::nodes::eval_result::EvaluationResult;
use slug_node::nodes::frontend::TargetGraphCalculation;
use slug_node::target_calculation::ConfiguredTargetCalculation;
use slug_server_ctx::ctx::ServerCommandContextTrait;
use slug_server_ctx::global_cfg_options::global_cfg_options_from_client_context;
use slug_server_ctx::partial_result_dispatcher::NoPartialResult;
use slug_server_ctx::partial_result_dispatcher::PartialResultDispatcher;
use slug_server_ctx::template::ServerCommandTemplate;
use slug_server_ctx::template::run_server_command;

struct TargetsArtifacts {
    providers_label: ConfiguredProvidersLabel,
    artifacts: Vec<Artifact>,
}

pub async fn targets_show_outputs_command(
    ctx: &dyn ServerCommandContextTrait,
    partial_result_dispatcher: PartialResultDispatcher<NoPartialResult>,
    req: TargetsRequest,
) -> slug_error::Result<TargetsShowOutputsResponse> {
    run_server_command(
        TargetsShowOutputsServerCommand { req },
        ctx,
        partial_result_dispatcher,
    )
    .await
}

struct TargetsShowOutputsServerCommand {
    req: TargetsRequest,
}

#[async_trait]
impl ServerCommandTemplate for TargetsShowOutputsServerCommand {
    type StartEvent = slug_data::TargetsCommandStart;
    type EndEvent = slug_data::TargetsCommandEnd;
    type Response = slug_cli_proto::TargetsShowOutputsResponse;
    type PartialResult = NoPartialResult;

    async fn command(
        &self,
        server_ctx: &dyn ServerCommandContextTrait,
        _partial_result_dispatcher: PartialResultDispatcher<Self::PartialResult>,
        ctx: DiceTransaction,
    ) -> slug_error::Result<Self::Response> {
        targets_show_outputs(server_ctx, ctx, &self.req).await
    }
}

async fn targets_show_outputs(
    server_ctx: &dyn ServerCommandContextTrait,
    mut ctx: DiceTransaction,
    request: &TargetsRequest,
) -> slug_error::Result<TargetsShowOutputsResponse> {
    let cwd = server_ctx.working_dir();

    let global_cfg_options = global_cfg_options_from_client_context(
        request
            .target_cfg
            .as_ref()
            .internal_error("target_cfg must be set")?,
        server_ctx,
        &mut ctx,
    )
    .await?;

    let parsed_patterns = parse_patterns_from_cli_args::<ProvidersPatternExtra>(
        &mut ctx,
        &request.target_patterns,
        cwd,
    )
    .await?;

    let artifact_fs = ctx.get_artifact_fs().await?;

    let mut targets_paths = Vec::new();

    for targets_artifacts in
        retrieve_targets_artifacts_from_patterns(&mut ctx, &global_cfg_options, &parsed_patterns)
            .await?
    {
        let mut paths = Vec::new();
        for artifact in targets_artifacts.artifacts {
            let path = artifact.resolve_configuration_hash_path(&artifact_fs)?;
            paths.push(path.to_string());
        }
        targets_paths.push(TargetPaths {
            target: targets_artifacts.providers_label.unconfigured().to_string(),
            paths,
        })
    }

    Ok(TargetsShowOutputsResponse { targets_paths })
}

async fn retrieve_targets_artifacts_from_patterns(
    ctx: &mut DiceComputations<'_>,
    global_cfg_options: &GlobalCfgOptions,
    parsed_patterns: &[ParsedPattern<ProvidersPatternExtra>],
) -> slug_error::Result<Vec<TargetsArtifacts>> {
    let resolved_pattern = ResolveTargetPatterns::resolve(ctx, parsed_patterns).await?;

    retrieve_artifacts_for_targets(ctx, resolved_pattern, global_cfg_options).await
}

async fn retrieve_artifacts_for_targets(
    ctx: &mut DiceComputations<'_>,
    spec: ResolvedPattern<ProvidersPatternExtra>,
    global_cfg_options: &GlobalCfgOptions,
) -> slug_error::Result<Vec<TargetsArtifacts>> {
    let artifacts_for_specs = ctx
        .try_compute_join(spec.specs, |ctx, (package_with_modifiers, spec)| {
            async move {
                {
                    let res = ctx
                        .get_interpreter_results(package_with_modifiers.package.dupe())
                        .await?;
                    retrieve_artifacts_for_spec(
                        ctx,
                        package_with_modifiers.package.dupe(),
                        spec,
                        global_cfg_options,
                        res,
                    )
                    .await
                }
            }
            .boxed()
        })
        .await?;

    let mut results = Vec::new();
    for artifacts in artifacts_for_specs {
        results.extend(artifacts);
    }

    Ok(results)
}

async fn retrieve_artifacts_for_spec(
    ctx: &mut DiceComputations<'_>,
    package: PackageLabel,
    spec: PackageSpec<ProvidersPatternExtra>,
    global_cfg_options: &GlobalCfgOptions,
    res: Arc<EvaluationResult>,
) -> slug_error::Result<Vec<TargetsArtifacts>> {
    let available_targets = res.targets();

    let todo_targets: Vec<(ProvidersLabel, &GlobalCfgOptions)> = match spec {
        PackageSpec::All() => available_targets
            .keys()
            .map(|t| {
                (
                    ProvidersLabel::default_for(TargetLabel::new(package.dupe(), t)),
                    global_cfg_options,
                )
            })
            .collect(),
        PackageSpec::Targets(targets) => {
            for (target_name, _) in &targets {
                res.resolve_target(target_name)?;
            }
            targets.into_map(|(target_name, providers)| {
                (
                    providers.into_providers_label(package.dupe(), target_name.as_ref()),
                    global_cfg_options,
                )
            })
        }
    };

    let outputs = ctx.try_compute_join(todo_targets, |ctx, (providers_label, cfg_flags)| {
        async move { retrieve_artifacts_for_provider_label(ctx, providers_label, cfg_flags).await }
            .boxed()
    }).await?;
    Ok(outputs)
}

async fn retrieve_artifacts_for_provider_label(
    ctx: &mut DiceComputations<'_>,
    providers_label: ProvidersLabel,
    global_cfg_options: &GlobalCfgOptions,
) -> slug_error::Result<TargetsArtifacts> {
    let providers_label = ctx
        .get_configured_provider_label(&providers_label, global_cfg_options)
        .await?;

    let providers = ctx
        .get_providers(&providers_label)
        .await?
        .require_compatible()?;

    let collection = providers.provider_collection();

    let mut artifacts = Vec::new();
    collection
        .default_info()?
        .for_each_default_output_artifact_only(&mut |o| artifacts.push(o))?;

    Ok(TargetsArtifacts {
        providers_label,
        artifacts,
    })
}
