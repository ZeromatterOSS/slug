/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fmt;

use slug_bzlmod_v2::BzlmodCommandPolicyKey;
use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
use slug_bzlmod_v2::LockfileMode;
use slug_configuration_v2::CommandConfigurationOccurrence;
use slug_identity_v2::ApparentLabel;
use slug_identity_v2::TargetPattern;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    Build,
    Run,
    Test,
    Query,
    Cquery,
    Aquery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlagDisposition {
    ParseOnly,
    IgnoredCompatible,
    Planned,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedFlag {
    pub raw: String,
    pub name: String,
    pub value: Option<String>,
    pub disposition: FlagDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryOutputFormat {
    Text,
    Label,
    Graph,
    StreamedJsonProto,
    LabelKind,
    Package,
    Build,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandParseError {
    MissingTarget { command: CommandKind },
    MissingQueryExpression { command: CommandKind },
    InvalidTargetPattern { value: String, message: String },
    InvalidQueryExpression { message: String },
    InvalidFlagValue { flag: String, message: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandPlaceholderError {
    pub command: CommandKind,
    pub owner_stage: &'static str,
    pub reason: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedArgv {
    pub flags: Vec<ParsedFlag>,
    pub positionals: Vec<String>,
    pub passthrough: Vec<String>,
}

impl CommandKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Run => "run",
            Self::Test => "test",
            Self::Query => "query",
            Self::Cquery => "cquery",
            Self::Aquery => "aquery",
        }
    }
}

impl CommandPlaceholderError {
    pub const fn planned(
        command: CommandKind,
        owner_stage: &'static str,
        reason: &'static str,
    ) -> Self {
        Self {
            command,
            owner_stage,
            reason,
        }
    }

    pub fn to_json_line(&self) -> String {
        format!(
            "{{\"error\":\"planned_placeholder\",\"command\":\"{}\",\"owner_stage\":\"{}\",\"reason\":\"{}\"}}",
            self.command,
            json_escape(self.owner_stage),
            json_escape(self.reason),
        )
    }
}

impl fmt::Display for CommandKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Display for FlagDisposition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ParseOnly => "parse_only",
            Self::IgnoredCompatible => "ignored_compatible",
            Self::Planned => "planned",
        })
    }
}

impl fmt::Display for QueryOutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text => f.write_str("text"),
            Self::Label => f.write_str("label"),
            Self::Graph => f.write_str("graph"),
            Self::StreamedJsonProto => f.write_str("streamed_jsonproto"),
            Self::LabelKind => f.write_str("label_kind"),
            Self::Package => f.write_str("package"),
            Self::Build => f.write_str("build"),
            Self::Other(value) => f.write_str(value),
        }
    }
}

impl fmt::Display for CommandParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingTarget { command } => write!(f, "{command} requires a target pattern"),
            Self::MissingQueryExpression { command } => {
                write!(f, "{command} requires a query expression")
            }
            Self::InvalidTargetPattern { value, message } => {
                write!(f, "invalid target pattern {value:?}: {message}")
            }
            Self::InvalidQueryExpression { message } => {
                write!(f, "invalid query expression: {message}")
            }
            Self::InvalidFlagValue { flag, message } => {
                write!(f, "invalid value for {flag}: {message}")
            }
        }
    }
}

impl std::error::Error for CommandParseError {}

impl fmt::Display for CommandPlaceholderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_json_line())
    }
}

pub(crate) fn split_args(args: &[impl AsRef<str>]) -> ParsedArgv {
    let mut flags = Vec::new();
    let mut positionals = Vec::new();
    let mut passthrough = Vec::new();
    let mut after_passthrough = false;
    for arg in args {
        let arg = arg.as_ref();
        if after_passthrough {
            passthrough.push(arg.to_owned());
        } else if arg == "--" {
            after_passthrough = true;
        } else if arg.starts_with("--") && arg.len() > 2 {
            flags.push(parse_flag(arg));
        } else {
            positionals.push(arg.to_owned());
        }
    }
    ParsedArgv {
        flags,
        positionals,
        passthrough,
    }
}

pub(crate) fn parse_target_patterns(
    command: CommandKind,
    values: &[String],
) -> Result<Vec<TargetPattern>, CommandParseError> {
    if values.is_empty() {
        return Err(CommandParseError::MissingTarget { command });
    }
    values
        .iter()
        .map(|value| {
            TargetPattern::parse(value).map_err(|message| CommandParseError::InvalidTargetPattern {
                value: value.clone(),
                message,
            })
        })
        .collect()
}

pub(crate) fn parse_single_target(
    command: CommandKind,
    value: Option<&String>,
) -> Result<TargetPattern, CommandParseError> {
    let Some(value) = value else {
        return Err(CommandParseError::MissingTarget { command });
    };
    TargetPattern::parse(value).map_err(|message| CommandParseError::InvalidTargetPattern {
        value: value.clone(),
        message,
    })
}

pub(crate) fn parse_query_expression_for(
    command: CommandKind,
    positionals: &[String],
) -> Result<String, CommandParseError> {
    if positionals.is_empty() {
        return Err(CommandParseError::MissingQueryExpression { command });
    }
    Ok(positionals.join(" "))
}

pub(crate) fn bzlmod_command_policy(
    flags: &[ParsedFlag],
) -> Result<BzlmodCommandPolicyKey, CommandParseError> {
    let (allow_yanked_versions, ignore_dev_dependency) = bzlmod_command_parts(flags)?;
    if let Some(flag) = flags.iter().find(|flag| flag.name == "override_module") {
        return Err(CommandParseError::InvalidFlagValue {
            flag: flag.raw.clone(),
            message: "--override_module requires a workspace-owned command path".to_owned(),
        });
    }
    normalized_bzlmod_command_policy(allow_yanked_versions, ignore_dev_dependency)
}

pub(crate) fn bzlmod_command_policy_for_workspace(
    flags: &[ParsedFlag],
    workspace: &std::path::Path,
) -> Result<BzlmodCommandPolicyKey, CommandParseError> {
    let (allow_yanked_versions, ignore_dev_dependency) = bzlmod_command_parts(flags)?;
    normalized_bzlmod_command_policy(allow_yanked_versions, ignore_dev_dependency)?;
    let overrides = flags
        .iter()
        .filter(|flag| flag.name == "override_module")
        .map(|flag| {
            flag.value
                .as_deref()
                .ok_or_else(|| CommandParseError::InvalidFlagValue {
                    flag: flag.raw.clone(),
                    message: "expected module-name=path".to_owned(),
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    BzlmodCommandPolicyKey::from_flags_with_module_overrides(
        allow_yanked_versions,
        ignore_dev_dependency,
        workspace,
        overrides,
    )
    .map_err(|message| CommandParseError::InvalidFlagValue {
        flag: "--override_module".to_owned(),
        message,
    })
}

fn bzlmod_command_parts(flags: &[ParsedFlag]) -> Result<(Option<&str>, bool), CommandParseError> {
    let allow_yanked_versions = flags
        .iter()
        .rev()
        .find(|flag| flag.name == "allow_yanked_versions")
        .map(|flag| {
            flag.value
                .as_deref()
                .ok_or_else(|| CommandParseError::InvalidFlagValue {
                    flag: flag.raw.clone(),
                    message: "expected 'all' or a comma-separated module@version allowlist"
                        .to_owned(),
                })
        })
        .transpose()?;
    let ignore_dev_dependency = flags
        .iter()
        .rev()
        .find_map(|flag| match flag.name.as_str() {
            "ignore_dev_dependency" => Some(parse_bool_flag(flag, false)),
            "noignore_dev_dependency" => Some(parse_bool_flag(flag, true)),
            _ => None,
        })
        .transpose()?
        .unwrap_or(false);
    Ok((allow_yanked_versions, ignore_dev_dependency))
}

fn normalized_bzlmod_command_policy(
    allow_yanked_versions: Option<&str>,
    ignore_dev_dependency: bool,
) -> Result<BzlmodCommandPolicyKey, CommandParseError> {
    BzlmodCommandPolicyKey::from_flags(allow_yanked_versions, ignore_dev_dependency).map_err(
        |message| CommandParseError::InvalidFlagValue {
            flag: "--allow_yanked_versions".to_owned(),
            message,
        },
    )
}

pub(crate) fn bzlmod_lockfile_mode(
    flags: &[ParsedFlag],
) -> Result<LockfileMode, CommandParseError> {
    let Some(flag) = flags.iter().rev().find(|flag| flag.name == "lockfile_mode") else {
        return Ok(LockfileMode::Update);
    };
    let value = flag
        .value
        .as_deref()
        .ok_or_else(|| CommandParseError::InvalidFlagValue {
            flag: flag.raw.clone(),
            message: "expected one of off, update, refresh, or error".to_owned(),
        })?;
    LockfileMode::from_bazel_flag_value(value).map_err(|message| {
        CommandParseError::InvalidFlagValue {
            flag: flag.raw.clone(),
            message,
        }
    })
}

pub(crate) fn bzlmod_registry_urls(flags: &[ParsedFlag]) -> Result<Vec<String>, CommandParseError> {
    flags
        .iter()
        .filter(|flag| flag.name == "registry")
        .map(|flag| {
            flag.value
                .as_deref()
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| CommandParseError::InvalidFlagValue {
                    flag: flag.raw.clone(),
                    message: "expected a non-empty registry URL".to_owned(),
                })
        })
        .collect()
}

/// Normalize the value captured from `BZLMOD_ALLOW_YANKED_VERSIONS`.
///
/// Environment access belongs to the CLI request boundary; this function is
/// deliberately pure so one-shot and daemon requests share identical parsing.
pub fn normalize_bzlmod_environment_value(
    value: Option<&str>,
) -> Result<BzlmodEnvironmentPolicyKey, CommandParseError> {
    BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(value).map_err(|message| {
        CommandParseError::InvalidFlagValue {
            flag: "BZLMOD_ALLOW_YANKED_VERSIONS".to_owned(),
            message,
        }
    })
}

pub(crate) fn output_format(flags: &[ParsedFlag]) -> QueryOutputFormat {
    let Some(value) = flags
        .iter()
        .rev()
        .find(|flag| flag.name == "output")
        .and_then(|flag| flag.value.as_deref())
    else {
        return QueryOutputFormat::Text;
    };
    match value {
        "text" => QueryOutputFormat::Text,
        "label" => QueryOutputFormat::Label,
        "graph" => QueryOutputFormat::Graph,
        "streamed_jsonproto" => QueryOutputFormat::StreamedJsonProto,
        "label_kind" => QueryOutputFormat::LabelKind,
        "package" => QueryOutputFormat::Package,
        "build" => QueryOutputFormat::Build,
        other => QueryOutputFormat::Other(other.to_owned()),
    }
}

pub(crate) fn parse_bool_flag(flag: &ParsedFlag, negated: bool) -> Result<bool, CommandParseError> {
    let parsed = match flag.value.as_deref() {
        Some(value) => parse_bool_value(&flag.raw, value)?,
        None => true,
    };
    Ok(if negated { !parsed } else { parsed })
}

/// Classify the only command occurrences admitted to structural
/// configuration. Contextual declaration/type checks remain DICE-owned.
pub(crate) fn command_configuration_occurrence(
    flag: &ParsedFlag,
) -> Result<Option<CommandConfigurationOccurrence>, CommandParseError> {
    let occurrence = match flag.name.as_str() {
        "extra_toolchains" => CommandConfigurationOccurrence::extra_toolchains(
            required_joined_value(flag, "--extra_toolchains=<patterns>")?,
        ),
        "extra_execution_platforms" => CommandConfigurationOccurrence::extra_execution_platforms(
            required_joined_value(flag, "--extra_execution_platforms=<patterns>")?,
        ),
        name => {
            let (label, negated) = if name.starts_with("//") || name.starts_with('@') {
                (name, false)
            } else if let Some(label) = name
                .strip_prefix("no")
                .filter(|label| label.starts_with("//") || label.starts_with('@'))
            {
                (label, true)
            } else {
                return Ok(None);
            };
            ApparentLabel::parse(label).map_err(|message| CommandParseError::InvalidFlagValue {
                flag: flag.raw.clone(),
                message: format!(
                    "expected a direct root or apparent-external build-setting label: {message}"
                ),
            })?;
            if negated && flag.value.is_some() {
                return Err(CommandParseError::InvalidFlagValue {
                    flag: flag.raw.clone(),
                    message: "Boolean no-form does not accept a joined value".to_owned(),
                });
            }
            CommandConfigurationOccurrence::starlark(label, flag.value.as_deref(), negated)
        }
    };
    Ok(Some(occurrence))
}

fn required_joined_value<'a>(
    flag: &'a ParsedFlag,
    expected: &str,
) -> Result<&'a str, CommandParseError> {
    flag.value
        .as_deref()
        .ok_or_else(|| CommandParseError::InvalidFlagValue {
            flag: flag.raw.clone(),
            message: format!("expected {expected}"),
        })
}

fn parse_bool_value(flag: &str, value: &str) -> Result<bool, CommandParseError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" => Ok(true),
        "0" | "false" | "no" => Ok(false),
        _ => Err(CommandParseError::InvalidFlagValue {
            flag: flag.to_owned(),
            message: "expected a boolean value".to_owned(),
        }),
    }
}

fn parse_flag(raw: &str) -> ParsedFlag {
    let without_prefix = &raw[2..];
    let (name, value) = match without_prefix.split_once('=') {
        Some((name, value)) => (name.to_owned(), Some(value.to_owned())),
        None => (without_prefix.to_owned(), None),
    };
    let disposition = classify_flag(&name);
    ParsedFlag {
        raw: raw.to_owned(),
        name,
        value,
        disposition,
    }
}

fn classify_flag(name: &str) -> FlagDisposition {
    match name {
        "output"
        | "order_output"
        | "output_base"
        | "graph:factored"
        | "nograph:factored"
        | "graph:node_limit"
        | "strict_test_suite"
        | "nostrict_test_suite"
        | "config"
        | "allow_yanked_versions"
        | "ignore_dev_dependency"
        | "noignore_dev_dependency"
        | "lockfile_mode"
        | "override_module"
        | "registry" => FlagDisposition::ParseOnly,
        "color" | "show_progress" | "noshow_progress" | "keep_going" => {
            FlagDisposition::IgnoredCompatible
        }
        "build_event_json_file"
        | "build_event_text_file"
        | "remote_cache"
        | "remote_executor"
        | "remote_header"
        | "remote_instance_name"
        | "remote_timeout"
        | "remote_retries"
        | "test_output"
        | "test_env"
        | "runs_per_test"
        | "remote_default_exec_properties"
        | "bes_backend"
        | "bes_results_url" => FlagDisposition::Planned,
        _ => FlagDisposition::Planned,
    }
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::new();
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch if ch.is_control() => escaped.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => escaped.push(ch),
        }
    }
    escaped
}
