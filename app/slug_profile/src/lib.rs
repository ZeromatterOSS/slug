/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

#![feature(error_generic_member_access)]

use std::sync::Arc;

use slug_cli_proto::HasClientContext;
use slug_cli_proto::profile_request::ProfileOpts;
use slug_core::fs::project::ProjectRoot;
use slug_core::pattern::unparsed::UnparsedPatternPredicate;
use slug_core::pattern::unparsed::UnparsedPatterns;
use slug_error::BuckErrorContext;
use slug_error::conversion::from_any_with_tag;
use slug_error::slug_error;
use slug_fs::fs_util;
use slug_fs::paths::abs_norm_path::AbsNormPath;
use slug_fs::paths::abs_path::AbsPath;
use slug_interpreter::starlark_profiler::config::StarlarkProfilerConfiguration;
use slug_interpreter::starlark_profiler::data::StarlarkProfileDataAndStats;
use starlark::eval::ProfileMode;

pub fn proto_to_profile_mode(proto: slug_cli_proto::ProfileMode) -> ProfileMode {
    match proto {
        slug_cli_proto::ProfileMode::HeapAllocated => ProfileMode::HeapAllocated,
        slug_cli_proto::ProfileMode::HeapRetained => ProfileMode::HeapRetained,
        slug_cli_proto::ProfileMode::HeapFlameAllocated => ProfileMode::HeapFlameAllocated,
        slug_cli_proto::ProfileMode::HeapFlameRetained => ProfileMode::HeapFlameRetained,
        slug_cli_proto::ProfileMode::HeapSummaryAllocated => ProfileMode::HeapSummaryAllocated,
        slug_cli_proto::ProfileMode::HeapSummaryRetained => ProfileMode::HeapSummaryRetained,
        slug_cli_proto::ProfileMode::TimeFlame => ProfileMode::TimeFlame,
        slug_cli_proto::ProfileMode::Statement => ProfileMode::Statement,
        slug_cli_proto::ProfileMode::Bytecode => ProfileMode::Bytecode,
        slug_cli_proto::ProfileMode::BytecodePairs => ProfileMode::BytecodePairs,
        slug_cli_proto::ProfileMode::Typecheck => ProfileMode::Typecheck,
        slug_cli_proto::ProfileMode::Coverage => ProfileMode::Coverage,
        slug_cli_proto::ProfileMode::None => ProfileMode::None,
    }
}

pub fn starlark_profiler_configuration_from_request(
    req: &slug_cli_proto::ProfileRequest,
    project_root: &ProjectRoot,
) -> slug_error::Result<StarlarkProfilerConfiguration> {
    let profiler_proto = slug_cli_proto::ProfileMode::try_from(req.profile_mode)
        .buck_error_context("Invalid profiler")?;

    let profile_mode = proto_to_profile_mode(profiler_proto);

    match req.profile_opts.as_ref().expect("Missing profile opts") {
        ProfileOpts::TargetProfile(opts) => {
            let action = slug_cli_proto::target_profile::Action::try_from(opts.action)
                .buck_error_context("Invalid action")?;
            Ok(match (action, opts.recursive) {
                (slug_cli_proto::target_profile::Action::Loading, false) => {
                    let working_dir = AbsNormPath::new(&req.client_context()?.working_dir)?;
                    let working_dir = project_root.relativize(working_dir)?;
                    StarlarkProfilerConfiguration::ProfileLoading(
                        profile_mode,
                        UnparsedPatternPredicate::AnyOf(UnparsedPatterns::new(
                            opts.target_patterns.clone(),
                            working_dir.to_buf(),
                        )),
                    )
                }
                (slug_cli_proto::target_profile::Action::Loading, true) => {
                    return Err(slug_error!(
                        slug_error::ErrorTag::Input,
                        "Recursive profiling is not supported for loading profiling, but you can pass multiple target patterns."
                    ));
                }
                (slug_cli_proto::target_profile::Action::Analysis, false) => {
                    let working_dir = AbsNormPath::new(&req.client_context()?.working_dir)?;
                    let working_dir = project_root.relativize(working_dir)?;
                    StarlarkProfilerConfiguration::ProfileAnalysis(
                        profile_mode,
                        UnparsedPatternPredicate::AnyOf(UnparsedPatterns::new(
                            opts.target_patterns.clone(),
                            working_dir.to_buf(),
                        )),
                    )
                }
                (slug_cli_proto::target_profile::Action::Analysis, true) => {
                    StarlarkProfilerConfiguration::ProfileAnalysis(
                        profile_mode,
                        UnparsedPatternPredicate::Any,
                    )
                }
            })
        }
        ProfileOpts::BxlProfile(_) => Ok(StarlarkProfilerConfiguration::ProfileBxl(profile_mode)),
    }
}

#[allow(clippy::format_collect)]
pub fn write_starlark_profile(
    profile_data: &StarlarkProfileDataAndStats,
    targets: &[String],
    output: &AbsPath,
) -> slug_error::Result<()> {
    fs_util::create_dir_if_not_exists(output)?;

    fs_util::write(
        output.join("targets.txt"),
        profile_data
            .targets
            .iter()
            .map(|t| format!("{t}\n"))
            .collect::<String>(),
    )
    .buck_error_context("Failed to write targets")?;

    match profile_data.profile_data.profile_mode() {
        ProfileMode::HeapAllocated
        | ProfileMode::HeapRetained
        | ProfileMode::HeapFlameAllocated
        | ProfileMode::HeapFlameRetained
        | ProfileMode::TimeFlame => {
            let mut profile = profile_data.profile_data.gen_flame_data()?;
            if profile.is_empty() {
                // inferno does not like empty flamegraphs.
                profile = "empty 1\n".to_owned();
            }
            let mut svg = Vec::new();
            let mut options = inferno::flamegraph::Options::default();
            let title = format!(
                "Flame Graph - {}",
                &profile_data.profile_data.profile_mode().to_string()
            );
            options.title = if targets.len() == 1 {
                format!("{} on {}", title, targets[0])
            } else if targets.len() > 1 {
                format!("{} on {} and {} more", title, targets[0], targets.len() - 1)
            } else {
                title
            };

            inferno::flamegraph::from_reader(&mut options, profile.as_bytes(), &mut svg)
                .map_err(|e| from_any_with_tag(e, slug_error::ErrorTag::Profile))
                .buck_error_context("writing SVG from profile data")?;

            fs_util::write(output.join("flame.src"), &profile)
                .buck_error_context("Failed to write flame.src")?;
            fs_util::write(output.join("flame.svg"), &svg)
                .buck_error_context("Failed to write flame.svg")?;
        }
        _ => {}
    };

    match profile_data.profile_data.profile_mode() {
        ProfileMode::HeapFlameAllocated | ProfileMode::HeapFlameRetained => {}
        _ => {
            let profile = profile_data.profile_data.gen_csv()?;
            fs_util::write(output.join("profile.csv"), profile)
                .buck_error_context("Failed to write profile")?;
        }
    };
    Ok(())
}

pub fn get_profile_response(
    profile_data: Arc<StarlarkProfileDataAndStats>,
    targets: &[String],
    output: &AbsPath,
) -> slug_error::Result<slug_cli_proto::ProfileResponse> {
    write_starlark_profile(profile_data.as_ref(), targets, output)?;

    Ok(slug_cli_proto::ProfileResponse {
        elapsed: Some(profile_data.duration().try_into()?),
        total_retained_bytes: profile_data.total_retained_bytes() as u64,
    })
}
