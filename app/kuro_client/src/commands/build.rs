/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::io::Write;
use std::path::PathBuf;

use async_trait::async_trait;
use dupe::Dupe;
use kuro_cli_proto::BuildRequest;
use kuro_cli_proto::BuildTarget;
use kuro_cli_proto::TargetCfg;
use kuro_cli_proto::build_request::BuildProviders;
use kuro_cli_proto::build_request::ResponseOptions;
use kuro_cli_proto::build_request::build_providers;
use kuro_client_ctx::client_ctx::ClientCommandContext;
use kuro_client_ctx::command_outcome::CommandOutcome;
use kuro_client_ctx::common::BuckArgMatches;
use kuro_client_ctx::common::CommonBuildConfigurationOptions;
use kuro_client_ctx::common::CommonCommandOptions;
use kuro_client_ctx::common::CommonEventLogOptions;
use kuro_client_ctx::common::CommonStarlarkOptions;
use kuro_client_ctx::common::PrintOutputsFormat;
use kuro_client_ctx::common::build::CommonBuildOptions;
use kuro_client_ctx::common::build::CommonOutputOptions;
use kuro_client_ctx::common::target_cfg::TargetCfgWithUniverseOptions;
use kuro_client_ctx::common::timeout::CommonTimeoutOptions;
use kuro_client_ctx::common::ui::CommonConsoleOptions;
use kuro_client_ctx::daemon::client::BuckdClientConnector;
use kuro_client_ctx::daemon::client::NoPartialResultHandler;
use kuro_client_ctx::events_ctx::EventsCtx;
use kuro_client_ctx::exit_result::ClientIoError;
use kuro_client_ctx::exit_result::ExitResult;
use kuro_client_ctx::final_console::FinalConsole;
use kuro_client_ctx::output_destination_arg::OutputDestinationArg;
use kuro_client_ctx::streaming::StreamingCommand;
use kuro_core::kuro_env;
use kuro_error::BuckErrorContext;
use kuro_error::kuro_error;

use crate::commands::build::out::copy_to_out;
use crate::print::PrintOutputs;

mod out;

#[derive(Debug, clap::Parser)]
#[clap(name = "build", about = "Build the specified targets")]
pub struct BuildCommand {
    #[clap(flatten)]
    show_output: CommonOutputOptions,

    #[clap(
        long = "materializations",
        short = 'M',
        help = "Materialize (or skip) the final artifacts, bypassing buckconfig.",
        ignore_case = true,
        value_enum
    )]
    materializations: Option<FinalArtifactMaterializations>,

    #[clap(
        long = "upload-final-artifacts",
        help = "Upload (or skip) the final artifacts.",
        ignore_case = true,
        value_enum
    )]
    upload_final_artifacts: Option<FinalArtifactUploads>,

    #[allow(unused)]
    #[clap(
        long,
        group = "default-info",
        help = "Build default info (this is the default)"
    )]
    build_default_info: bool,

    #[clap(
        long,
        group = "default-info",
        help = "Do not build default info (this is not the default)"
    )]
    skip_default_info: bool,

    #[allow(unused)]
    #[clap(
        long,
        group = "run-info",
        help = "Build runtime dependencies (this is the default)"
    )]
    build_run_info: bool,

    #[clap(
        long,
        group = "run-info",
        help = "Do not build runtime dependencies (this is not the default)"
    )]
    skip_run_info: bool,

    #[clap(
        long,
        group = "test-info",
        help = "Build tests (this is not the default)"
    )]
    build_test_info: bool,

    #[allow(unused)]
    #[clap(
        long,
        group = "test-info",
        help = "Do not build tests (this is the default)"
    )]
    skip_test_info: bool,

    #[clap(
        long = "out",
        help = "Copy the output of the built target to this path (`-` to stdout)"
    )]
    output_path: Option<OutputDestinationArg>,

    #[clap(name = "TARGET_PATTERNS", help = "Patterns to build", value_hint = clap::ValueHint::Other)]
    patterns: Vec<String>,

    /// This option does nothing. It is here to keep compatibility with Buck1 and ci
    #[clap(long = "deep", hide = true)]
    _deep: bool,

    #[clap(flatten)]
    build_opts: CommonBuildOptions,

    #[clap(flatten)]
    target_cfg: TargetCfgWithUniverseOptions,

    #[clap(flatten)]
    timeout_options: CommonTimeoutOptions,

    #[clap(flatten)]
    common_opts: CommonCommandOptions,
}

impl BuildCommand {
    fn default_info(&self) -> build_providers::Action {
        if self.skip_default_info {
            return build_providers::Action::Skip;
        }
        build_providers::Action::Build
    }

    fn run_info(&self) -> build_providers::Action {
        if self.skip_run_info {
            return build_providers::Action::Skip;
        }
        build_providers::Action::BuildIfAvailable
    }

    fn test_info(&self) -> build_providers::Action {
        if self.build_test_info {
            return build_providers::Action::BuildIfAvailable;
        }
        build_providers::Action::Skip
    }

    pub(crate) fn patterns(&self) -> &Vec<String> {
        &self.patterns
    }

    pub(crate) fn target_universe(&self) -> &Vec<String> {
        &self.target_cfg.target_universe
    }

    pub(crate) fn target_cfg(&self) -> TargetCfg {
        self.target_cfg.target_cfg.target_cfg().clone()
    }
}

#[derive(Debug, Clone, Dupe, clap::ValueEnum)]
#[clap(rename_all = "snake_case")]
pub enum FinalArtifactMaterializations {
    All,
    None,
}
pub trait MaterializationsToProto {
    fn to_proto(&self) -> kuro_cli_proto::build_request::Materializations;
}
impl MaterializationsToProto for Option<FinalArtifactMaterializations> {
    fn to_proto(&self) -> kuro_cli_proto::build_request::Materializations {
        match self {
            Some(FinalArtifactMaterializations::All) => {
                kuro_cli_proto::build_request::Materializations::Materialize
            }
            Some(FinalArtifactMaterializations::None) => {
                kuro_cli_proto::build_request::Materializations::Skip
            }
            None => kuro_cli_proto::build_request::Materializations::Default,
        }
    }
}

#[derive(Debug, Clone, Dupe, clap::ValueEnum)]
#[clap(rename_all = "snake_case")]
pub enum FinalArtifactUploads {
    Always,
    Never,
}
pub trait UploadsToProto {
    fn to_proto(&self) -> kuro_cli_proto::build_request::Uploads;
}
impl UploadsToProto for Option<FinalArtifactUploads> {
    fn to_proto(&self) -> kuro_cli_proto::build_request::Uploads {
        match self {
            Some(FinalArtifactUploads::Always) => kuro_cli_proto::build_request::Uploads::Always,
            Some(FinalArtifactUploads::Never) => kuro_cli_proto::build_request::Uploads::Never,
            None => kuro_cli_proto::build_request::Uploads::Never,
        }
    }
}

pub fn print_build_result(
    console: &FinalConsole,
    errors: &[kuro_data::ErrorReport],
) -> kuro_error::Result<()> {
    for error in errors {
        console.print_error(&error.message)?;
    }
    Ok(())
}

#[async_trait(?Send)]
impl StreamingCommand for BuildCommand {
    const COMMAND_NAME: &'static str = "build";

    async fn exec_impl(
        self,
        buckd: &mut BuckdClientConnector,
        matches: BuckArgMatches<'_>,
        ctx: &mut ClientCommandContext<'_>,
        events_ctx: &mut EventsCtx,
    ) -> ExitResult {
        let context = ctx.client_context(matches, &self)?;

        let result = buckd
            .with_flushing()
            .build(
                BuildRequest {
                    context: Some(context),
                    target_patterns: self.patterns.clone(),
                    target_cfg: Some(self.target_cfg.target_cfg.target_cfg()),
                    build_providers: Some(BuildProviders {
                        default_info: self.default_info() as i32,
                        run_info: self.run_info() as i32,
                        test_info: self.test_info() as i32,
                    }),
                    response_options: Some(ResponseOptions {
                        // Always request outputs so we can create bazel-bin symlinks.
                        return_outputs: true,
                    }),
                    build_opts: Some(self.build_opts.to_proto()),
                    final_artifact_materializations: self.materializations.to_proto() as i32,
                    final_artifact_uploads: self.upload_final_artifacts.to_proto() as i32,
                    target_universe: self.target_cfg.target_universe,
                    timeout: self.timeout_options.overall_timeout()?,
                },
                events_ctx,
                ctx.console_interaction_stream(&self.common_opts.console_opts),
                &mut NoPartialResultHandler,
            )
            .await;
        let success = match &result {
            Ok(CommandOutcome::Success(response)) => response.errors.is_empty(),
            Ok(CommandOutcome::Failure(_)) => false,
            Err(_) => false,
        };

        let console = self.common_opts.console_opts.final_console();

        if success {
            if self.patterns.is_empty() {
                console.print_warning("NO BUILD TARGET PATTERNS SPECIFIED")?;
            } else {
                print_build_succeeded(&console, ctx, None)?;
            }
        } else {
            print_build_failed(&console)?;
        }

        if kuro_env!("BUCK2_TEST_BUILD_ERROR", bool, applicability = testing)? {
            return kuro_error!(
                kuro_error::ErrorTag::TestOnly,
                "Injected Build Response Error"
            )
            .into();
        }

        // Most build errors are returned in the `result.errors` field, but some are not and printed
        // here.
        let response = result??;

        print_build_result(&console, &response.errors)?;

        let mut stdout = Vec::new();

        if let Some(build_report) = response.serialized_build_report {
            stdout.extend(build_report.as_bytes());
            writeln!(&mut stdout)?;
        }

        if let Some(format) = self.show_output.format() {
            print_outputs(
                &mut stdout,
                &response.build_targets,
                self.show_output.is_full().then_some(response.project_root),
                format,
            )?;
        }

        let res = if success {
            if let Some(stdout) = &self.output_path {
                copy_to_out(
                    &response.build_targets,
                    ctx.paths()?.project_root(),
                    &ctx.working_dir,
                    stdout,
                )
                .await
                .buck_error_context("Error requesting specific output path for --out")?;
            }

            // Create bazel-bin convenience symlinks for Bazel compatibility.
            // Mirrors Bazel's behavior: bazel-bin/<pkg>/<artifact> → buck-out/v2/gen/...
            if let Err(e) = create_bazel_bin_symlinks(
                ctx.paths()?.project_root().root().as_path(),
                &response.build_targets,
            ) {
                // Non-fatal: don't fail build if symlinks can't be created (e.g., on Windows)
                tracing::debug!("Failed to create bazel-bin symlinks: {}", e);
            }

            ExitResult::success()
        } else {
            ExitResult::from_command_result_errors(response.errors)
        };

        res.with_stdout(stdout)
    }

    fn console_opts(&self) -> &CommonConsoleOptions {
        &self.common_opts.console_opts
    }

    fn event_log_opts(&self) -> &CommonEventLogOptions {
        &self.common_opts.event_log_opts
    }

    fn build_config_opts(&self) -> &CommonBuildConfigurationOptions {
        &self.common_opts.config_opts
    }

    fn starlark_opts(&self) -> &CommonStarlarkOptions {
        &self.common_opts.starlark_opts
    }
}

pub(crate) fn print_build_succeeded(
    console: &FinalConsole,
    ctx: &ClientCommandContext<'_>,
    extra: Option<&str>,
) -> kuro_error::Result<()> {
    if ctx.verbosity.print_success_message() {
        console.print_success_no_newline("BUILD SUCCEEDED")?;
        console.print_stderr(extra.unwrap_or_default())?;
    }
    Ok(())
}

pub(crate) fn print_build_failed(console: &FinalConsole) -> kuro_error::Result<()> {
    console.print_error("BUILD FAILED")
}

pub(crate) fn print_outputs(
    out: impl Write,
    targets: &[BuildTarget],
    root_path: Option<String>,
    format: PrintOutputsFormat,
) -> Result<(), ClientIoError> {
    let root_path = root_path.map(PathBuf::from);
    let mut print = PrintOutputs::new(out, root_path, format)?;

    for build_target in targets {
        // just print the default info for build command
        let outputs = build_target.outputs.iter().filter(|output| {
            output
                .providers
                .as_ref()
                .is_none_or(|p| p.default_info && !p.other)
        });

        // only print the unconfigured target for now until we migrate everything to support
        // also printing configurations
        if outputs.clone().count() > 1 {
            // FIXME(JakobDegen): Why exactly do we not show the path?
            print.output(&build_target.target, None)?;
            continue;
        }
        for output in outputs {
            print.output(&build_target.target, Some(&output.path))?;
        }
    }

    print.finish()
}

/// Create `bazel-bin/<pkg>/<artifact>` symlinks after a successful build, mirroring Bazel's
/// convenience symlink behavior.  Only creates symlinks for `default_info` outputs from the
/// main repo (not external cells), and skips non-Unix platforms.
///
/// Output path format: `buck-out/v2/gen/<cell>/<cfg_hash>[/<pkg>/...]/__<target>__/<artifact>`
/// Symlink target:     `bazel-bin[/<pkg>]/<artifact>` → relative path to actual output
#[cfg(unix)]
fn create_bazel_bin_symlinks(
    project_root: &std::path::Path,
    build_targets: &[BuildTarget],
) -> kuro_error::Result<()> {
    let bazel_bin = project_root.join("bazel-bin");

    for target in build_targets {
        for output in &target.outputs {
            // Only create symlinks for default_info outputs
            if !output.providers.as_ref().map_or(false, |p| p.default_info) {
                continue;
            }

            // Parse: buck-out/v2/gen/<cell>/<cfg_hash>[/<pkg>/...]/__<target>__/<artifact...>
            let components: Vec<&str> = output.path.split('/').collect();

            // Minimum: ["buck-out","v2","gen","<cell>","<cfg_hash>","__target__","<artifact>"]
            if components.len() < 7
                || components[0] != "buck-out"
                || components[1] != "v2"
                || components[2] != "gen"
            {
                continue;
            }

            // Skip external cell outputs (e.g. buck-out/v2/gen/rules_cc/...)
            // Only process main repo outputs where outputs are most useful.
            // Heuristic: the cell name matches the last component of the project root dir name.
            // More precisely: skip paths with "external" or those under bzlmod external cells.
            let cell = components[3];
            if cell.contains("__") || cell == "external" {
                continue;
            }

            // Find the __<target>__ component (starts at index 5 after cell + cfg_hash)
            let target_dir_offset = components[5..]
                .iter()
                .position(|c| c.starts_with("__") && c.ends_with("__") && c.len() > 4);
            let target_dir_idx = match target_dir_offset {
                Some(i) => i + 5,
                None => continue,
            };

            // Package path: components between cfg_hash (index 4) and __target__ (target_dir_idx)
            let pkg_components = &components[5..target_dir_idx];
            // Artifact path: components after __target__
            let artifact_components = &components[target_dir_idx + 1..];

            if artifact_components.is_empty() {
                continue;
            }

            // Construct bazel-bin/<pkg>/<artifact> path
            let mut bazel_path = bazel_bin.clone();
            for comp in pkg_components {
                bazel_path = bazel_path.join(comp);
            }
            for comp in artifact_components {
                bazel_path = bazel_path.join(comp);
            }

            // Create parent directories
            if let Some(parent) = bazel_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            // Compute relative symlink target from the symlink's location to the project root,
            // then append the buck-out relative path.
            // Depth = 1 (bazel-bin/) + pkg_components.len() + artifact dir components
            let dir_depth = 1 + pkg_components.len() + artifact_components.len().saturating_sub(1);
            let rel_target = format!("{}{}", "../".repeat(dir_depth), output.path);

            // Remove existing symlink/file before creating new one
            if bazel_path.symlink_metadata().is_ok() {
                std::fs::remove_file(&bazel_path).ok();
            }

            std::os::unix::fs::symlink(&rel_target, &bazel_path).with_buck_error_context(|| {
                format!(
                    "Creating bazel-bin symlink {} -> {}",
                    bazel_path.display(),
                    rel_target
                )
            })?;
        }
    }

    Ok(())
}

/// No-op on non-Unix platforms.
#[cfg(not(unix))]
fn create_bazel_bin_symlinks(
    _project_root: &std::path::Path,
    _build_targets: &[BuildTarget],
) -> kuro_error::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use assert_matches::assert_matches;
    use build_providers::Action;
    use clap::Parser;

    use super::*;

    fn parse(args: &[&str]) -> kuro_error::Result<BuildCommand> {
        Ok(BuildCommand::try_parse_from(
            std::iter::once("program").chain(args.iter().copied()),
        )?)
    }

    #[test]
    fn infos_default() -> kuro_error::Result<()> {
        let opts = parse(&[])?;

        assert_eq!(opts.default_info(), Action::Build);
        assert_eq!(opts.run_info(), Action::BuildIfAvailable);
        assert_eq!(opts.test_info(), Action::Skip);

        Ok(())
    }

    #[test]
    fn infos_noop() -> kuro_error::Result<()> {
        let opts = parse(&[
            "--skip-test-info",
            "--build-default-info",
            "--build-run-info",
        ])?;

        assert_eq!(opts.default_info(), Action::Build);
        assert_eq!(opts.run_info(), Action::BuildIfAvailable);
        assert_eq!(opts.test_info(), Action::Skip);

        Ok(())
    }

    #[test]
    fn infos_configure() -> kuro_error::Result<()> {
        let opts = parse(&["--skip-default-info"])?;
        assert_eq!(opts.default_info(), Action::Skip);

        let opts = parse(&["--skip-run-info"])?;
        assert_eq!(opts.run_info(), Action::Skip);

        let opts = parse(&["--build-test-info"])?;
        assert_eq!(opts.test_info(), Action::BuildIfAvailable);

        Ok(())
    }

    #[test]
    fn infos_validation() -> kuro_error::Result<()> {
        // Test duplicate args
        assert_matches!(
            parse(&["--build-default-info", "--skip-default-info"]),
            Err(..)
        );
        assert_matches!(parse(&["--build-run-info", "--skip-run-info"]), Err(..));
        assert_matches!(parse(&["--build-test-info", "--skip-test-info"]), Err(..));

        // Test args across all groups.
        assert_matches!(
            parse(&[
                "--skip-default-info",
                "--skip-run-info",
                "--build-test-info"
            ]),
            Ok(..)
        );

        Ok(())
    }
}
