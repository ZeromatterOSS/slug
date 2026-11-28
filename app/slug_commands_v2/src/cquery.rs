/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory.
 * You may select, at your option, one of the above-listed licenses.
 */

use std::path::Path;

use slug_bzlmod_v2::BzlmodCommandPolicyKey;
use slug_bzlmod_v2::LockfileMode;
use slug_configuration_v2::CommandConfigurationOverlay;
use slug_identity_v2::TargetPattern;
use slug_query_v2::QueryExpression;
use slug_query_v2::cquery_literals;
use slug_query_v2::validate_cquery_query;

use crate::common::CommandKind;
use crate::common::CommandParseError;
use crate::common::RepositoryEnvironmentOverride;
use crate::common::archive_distdir_value;
use crate::common::bzlmod_command_policy;
use crate::common::bzlmod_command_policy_for_workspace;
use crate::common::bzlmod_lockfile_mode;
use crate::common::bzlmod_registry_urls;
use crate::common::command_configuration_occurrence;
use crate::common::parse_bool_flag;
use crate::common::repository_environment_overrides;
use crate::common::split_args;
use crate::common::validate_bare_boolean_flag;

const LABEL_EXPRESSION: &str = "str(target.label)";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CqueryOutputMode {
    Label,
    LabelKind,
    StarlarkLabel,
    StarlarkFile(String),
    Graph,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CqueryRequest {
    pub expression: String,
    pub output_mode: CqueryOutputMode,
    pub include_implicit: bool,
    pub include_tool: bool,
    pub output_base: Option<String>,
    pub configuration_overlay: CommandConfigurationOverlay,
    pub bzlmod_policy: BzlmodCommandPolicyKey,
    pub lockfile_mode: LockfileMode,
    pub registry_urls: Vec<String>,
    pub distdirs: Vec<String>,
    pub repository_environment_overrides: Vec<RepositoryEnvironmentOverride>,
}

impl CqueryRequest {
    pub fn parse(args: &[impl AsRef<str>]) -> Result<Self, CommandParseError> {
        Self::parse_impl(args, None)
    }

    pub fn parse_at_workspace(
        args: &[impl AsRef<str>],
        workspace: &Path,
    ) -> Result<Self, CommandParseError> {
        Self::parse_impl(args, Some(workspace))
    }

    fn parse_impl(
        args: &[impl AsRef<str>],
        workspace: Option<&Path>,
    ) -> Result<Self, CommandParseError> {
        let parsed = split_args(args);
        if !parsed.passthrough.is_empty() {
            return Err(unsupported("-- is not supported by this cquery"));
        }
        if parsed.positionals.len() != 1 {
            return Err(if parsed.positionals.is_empty() {
                CommandParseError::MissingTarget {
                    command: CommandKind::Cquery,
                }
            } else {
                unsupported("cquery accepts exactly one literal target label")
            });
        }
        let cquery_expression = parsed.positionals[0].clone();
        let parsed_expression = QueryExpression::parse(&cquery_expression)
            .map_err(|error| unsupported(&error.to_string()))?;
        validate_cquery_query(&parsed_expression)
            .map_err(|error| unsupported(&error.to_string()))?;
        for literal in cquery_literals(&parsed_expression)
            .into_iter()
            .chain(parsed_expression.cquery_rdeps_seed())
        {
            let target = TargetPattern::parse(literal).map_err(|message| {
                CommandParseError::InvalidTargetPattern {
                    value: literal.to_owned(),
                    message,
                }
            })?;
            let TargetPattern::Single(label) = target else {
                return Err(unsupported(
                    "target patterns are not supported by this cquery",
                ));
            };
            if !label.repo().is_root() {
                return Err(unsupported(
                    "external repository labels are not supported by this cquery",
                ));
            }
        }

        let mut output = None;
        let mut starlark_expression = None;
        let mut starlark_file = None;
        let mut output_base = None;
        let mut distdirs = Vec::new();
        let mut configuration = Vec::new();
        let mut include_implicit = true;
        let mut include_tool = true;
        let mut saw_noimplicit_deps = false;
        let mut graph_unfactored = false;
        let mut saw_graph_unfactored = false;
        for flag in &parsed.flags {
            if let Some(occurrence) = command_configuration_occurrence(flag)? {
                configuration.push(occurrence);
                continue;
            }
            match flag.name.as_str() {
                "output" => set_once(&mut output, flag, "--output")?,
                "starlark:expr" => set_once(&mut starlark_expression, flag, "--starlark:expr")?,
                "starlark:file" => set_once(&mut starlark_file, flag, "--starlark:file")?,
                "output_base" => set_once(&mut output_base, flag, "--output_base")?,
                "distdir" => distdirs.push(archive_distdir_value(flag)?),
                "implicit_deps" => include_implicit = parse_bool_flag(flag, false)?,
                "noimplicit_deps" => {
                    include_implicit = parse_bool_flag(flag, true)?;
                    saw_noimplicit_deps |= !include_implicit;
                }
                "tool_deps" => include_tool = parse_bool_flag(flag, false)?,
                "notool_deps" => include_tool = parse_bool_flag(flag, true)?,
                "nograph:factored" => {
                    graph_unfactored = !parse_bool_flag(flag, true)?;
                    saw_graph_unfactored = true;
                }
                "noshow_progress" => validate_bare_boolean_flag(flag)?,
                "allow_yanked_versions"
                | "ignore_dev_dependency"
                | "noignore_dev_dependency"
                | "lockfile_mode"
                | "override_module"
                | "registry"
                | "repo_env" => {}
                _ => {
                    return Err(unsupported(&format!(
                        "{} is not supported by this cquery",
                        flag.raw
                    )));
                }
            }
        }
        if parsed_expression.cquery_preactivation_deps_spec().is_some() && include_implicit {
            return Err(unsupported(
                "deps() requires --noimplicit_deps in this cquery",
            ));
        }
        let output_mode = match (
            output.as_deref(),
            starlark_expression.as_deref(),
            starlark_file,
        ) {
            (_, Some(_), Some(_)) => {
                return Err(unsupported(
                    "You must not specify both --starlark:expr and --starlark:file",
                ));
            }
            (Some("starlark"), None, Some(path)) => CqueryOutputMode::StarlarkFile(path),
            (_, _, Some(_)) => {
                return Err(unsupported("--starlark:file requires --output=starlark"));
            }
            (None | Some("label"), None, None) => CqueryOutputMode::Label,
            (Some("label_kind"), None, None) => CqueryOutputMode::LabelKind,
            (Some("starlark"), Some(LABEL_EXPRESSION), None) => CqueryOutputMode::StarlarkLabel,
            (Some("graph"), None, None) => {
                let Some(_) = parsed_expression.cquery_preactivation_deps_spec() else {
                    return Err(unsupported(
                        "--output=graph requires a top-level deps() cquery expression",
                    ));
                };
                if !saw_noimplicit_deps || include_implicit {
                    return Err(unsupported("--output=graph requires --noimplicit_deps"));
                }
                if !saw_graph_unfactored || !graph_unfactored {
                    return Err(unsupported("--output=graph requires --nograph:factored"));
                }
                CqueryOutputMode::Graph
            }
            _ => {
                return Err(unsupported(
                    "expected default output, --output=label, --output=label_kind, or --output=starlark \
                     with --starlark:expr=str(target.label) or --starlark:file=<path>",
                ));
            }
        };
        if output_mode != CqueryOutputMode::Graph && saw_graph_unfactored {
            return Err(unsupported(
                "--nograph:factored is supported only with --output=graph",
            ));
        }
        let bzlmod_policy = match workspace {
            Some(workspace) => bzlmod_command_policy_for_workspace(&parsed.flags, workspace)?,
            None => bzlmod_command_policy(&parsed.flags)?,
        };
        let lockfile_mode = bzlmod_lockfile_mode(&parsed.flags)?;
        let registry_urls = bzlmod_registry_urls(&parsed.flags)?;
        let repository_environment_overrides = repository_environment_overrides(&parsed.flags)?;
        Ok(Self {
            expression: cquery_expression,
            output_mode,
            include_implicit,
            include_tool,
            output_base,
            configuration_overlay: configuration.into(),
            bzlmod_policy,
            lockfile_mode,
            registry_urls,
            distdirs,
            repository_environment_overrides,
        })
    }
}

fn set_once(
    slot: &mut Option<String>,
    flag: &crate::common::ParsedFlag,
    name: &str,
) -> Result<(), CommandParseError> {
    let value = flag
        .value
        .clone()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CommandParseError::InvalidFlagValue {
            flag: flag.raw.clone(),
            message: format!("{name} requires a non-empty value"),
        })?;
    if slot.replace(value).is_some() {
        return Err(unsupported(&format!("{name} may be specified only once")));
    }
    Ok(())
}

fn unsupported(message: &str) -> CommandParseError {
    CommandParseError::InvalidFlagValue {
        flag: "cquery".to_owned(),
        message: message.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noshow_progress_is_compatible_with_existing_cquery_output() {
        for mut args in [
            vec!["//pkg:probe", "--output=label"],
            vec![
                "//pkg:probe",
                "--output=starlark",
                "--starlark:expr=str(target.label)",
            ],
        ] {
            let baseline = CqueryRequest::parse(&args).unwrap();
            args.push("--noshow_progress");
            assert_eq!(CqueryRequest::parse(&args).unwrap(), baseline);
        }
        assert!(matches!(
            CqueryRequest::parse(&["//pkg:probe", "--noshow_progress=false"]),
            Err(CommandParseError::InvalidFlagValue { flag, .. })
                if flag == "--noshow_progress=false"
        ));
    }

    #[test]
    fn starlark_file_preserves_path_and_pinned_query_inputs() {
        let baseline = CqueryRequest::parse(&[
            "//pkg:probe",
            "--@rules_rust//rust/toolchain/channel=nightly",
            "--lockfile_mode=off",
        ])
        .unwrap();
        let request = CqueryRequest::parse(&[
            "//pkg:probe",
            "--output=starlark",
            "--starlark:file=cquery_format.bzl",
            "--noshow_progress",
            "--@rules_rust//rust/toolchain/channel=nightly",
            "--lockfile_mode=off",
        ])
        .unwrap();
        assert_eq!(
            request,
            CqueryRequest {
                output_mode: CqueryOutputMode::StarlarkFile("cquery_format.bzl".into()),
                ..baseline
            }
        );
        let request = CqueryRequest::parse(&[
            "//pkg:probe",
            "--starlark:file=./format dir/cquery=output.bzl",
            "--output=starlark",
        ])
        .unwrap();
        assert_eq!(
            request.output_mode,
            CqueryOutputMode::StarlarkFile("./format dir/cquery=output.bzl".into()),
        );
    }

    #[test]
    fn starlark_file_rejects_conflicting_or_incomplete_options() {
        let cases: &[(&[&str], &str)] = &[
            (
                &[
                    "--output=starlark",
                    "--starlark:file=first.bzl",
                    "--starlark:expr=str(target.label)",
                ],
                "must not specify both --starlark:expr and --starlark:file",
            ),
            (
                &["--starlark:file=first.bzl"],
                "--starlark:file requires --output=starlark",
            ),
            (
                &["--output=label", "--starlark:file=first.bzl"],
                "--starlark:file requires --output=starlark",
            ),
            (
                &["--output=starlark", "--starlark:file"],
                "--starlark:file requires a non-empty value",
            ),
            (
                &["--output=starlark", "--starlark:file="],
                "--starlark:file requires a non-empty value",
            ),
            (
                &[
                    "--output=starlark",
                    "--starlark:file=first.bzl",
                    "--starlark:file=second.bzl",
                ],
                "--starlark:file may be specified only once",
            ),
            (
                &["--noshow_progress=false"],
                "Unexpected value after boolean option",
            ),
        ];
        for &(options, message) in cases {
            let args = ["//pkg:probe"].into_iter().chain(options.iter().copied());
            let error = CqueryRequest::parse(&args.collect::<Vec<_>>()).unwrap_err();
            assert!(error.to_string().contains(message), "{error}");
        }
        assert_eq!(
            CqueryRequest::parse(&[
                "//pkg:probe",
                "--output=starlark",
                "--starlark:expr=str(target.label)",
            ])
            .unwrap()
            .output_mode,
            CqueryOutputMode::StarlarkLabel,
        );
    }

    #[test]
    fn distdirs_parse_in_order_and_stay_out_of_configuration() {
        let baseline = CqueryRequest::parse(&["//pkg:probe"]).unwrap();
        let request = CqueryRequest::parse(&[
            "//pkg:probe",
            "--distdir=relative",
            "--distdir=/absolute",
            "--distdir=relative",
        ])
        .unwrap();
        assert_eq!(request.distdirs, ["relative", "/absolute", "relative"]);
        assert!(baseline.distdirs.is_empty());
        assert_eq!(
            request.configuration_overlay,
            baseline.configuration_overlay
        );
        assert_eq!(request.bzlmod_policy, baseline.bzlmod_policy);
        let flags = split_args(&["--distdir=relative"]);
        assert_eq!(
            flags.flags[0].disposition,
            crate::common::FlagDisposition::ParseOnly
        );
        for flag in ["--distdir", "--distdir="] {
            assert!(
                CqueryRequest::parse(&["//pkg:probe", flag])
                    .unwrap_err()
                    .to_string()
                    .contains("non-empty directory")
            );
        }
        assert!(CqueryRequest::parse(&["//pkg:probe", "--distdir", "relative"]).is_err());
    }
}
