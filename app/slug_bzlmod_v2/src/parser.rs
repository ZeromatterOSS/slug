/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::BTreeMap;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleFile {
    pub module: Option<ModuleHeader>,
    pub directives: Vec<Directive>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleHeader {
    pub name: String,
    pub version: Option<String>,
    pub repo_name: Option<String>,
    pub compatibility_level: Option<u64>,
    pub bazel_compatibility: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Directive {
    Include(String),
    BazelDep(BazelDep),
    LocalPathOverride(LocalPathOverride),
    SingleVersionOverride(SingleVersionOverride),
    MultipleVersionOverride(MultipleVersionOverride),
    ArchiveOverride(ArchiveOverride),
    GitOverride(GitOverride),
    UseExtension(UseExtension),
    ExtensionTag(ExtensionTag),
    UseRepo(UseRepo),
    OverrideRepo(OverrideRepo),
    InjectRepo(InjectRepo),
    UseRepoRule(UseRepoRule),
    RepoRuleInvocation(RepoRuleInvocation),
    RegisterToolchains(Registration),
    RegisterExecutionPlatforms(Registration),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registration {
    pub labels: Vec<String>,
    pub dev_dependency: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BazelDep {
    pub name: String,
    pub version: String,
    pub repo_name: Option<String>,
    pub dev_dependency: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalPathOverride {
    pub module_name: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SingleVersionOverride {
    pub module_name: String,
    pub version: String,
    pub registry: Option<String>,
    pub patches: Vec<String>,
    pub patch_cmds: Vec<String>,
    pub patch_strip: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultipleVersionOverride {
    pub module_name: String,
    pub versions: Vec<String>,
    pub registry: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveOverride {
    pub module_name: String,
    pub urls: Vec<String>,
    pub integrity: Option<String>,
    pub strip_prefix: Option<String>,
    pub patches: Vec<String>,
    pub patch_strip: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitOverride {
    pub module_name: String,
    pub remote: String,
    pub commit: String,
    pub shallow_since: Option<String>,
    pub patches: Vec<String>,
    pub patch_strip: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UseExtension {
    pub proxy_name: String,
    pub bzl_label: String,
    pub extension_name: String,
    pub dev_dependency: bool,
    pub isolate: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionTag {
    pub extension_proxy: String,
    pub tag_class: String,
    pub attrs: BTreeMap<String, ModuleAttributeValue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UseRepo {
    pub extension_proxy: String,
    pub repos: Vec<RepoImport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverrideRepo {
    pub extension_proxy: String,
    pub repos: Vec<RepoImport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InjectRepo {
    pub extension_proxy: String,
    pub repos: Vec<RepoImport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoImport {
    pub apparent_name: String,
    pub repo_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UseRepoRule {
    pub proxy_name: String,
    pub bzl_label: String,
    pub rule_name: String,
    pub dev_dependency: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoRuleInvocation {
    pub rule_proxy: String,
    pub repo_name: String,
    pub attrs: BTreeMap<String, ModuleAttributeValue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleAttributeValue {
    String(String),
    StringList(Vec<String>),
    Integer(u64),
    Bool(bool),
}

impl ModuleFile {
    pub fn parse(source: &str) -> Result<Self, String> {
        let mut module = None;
        let mut directives = Vec::new();
        let mut extension_proxies = BTreeSet::new();
        let mut repo_rule_proxies = BTreeSet::new();
        for (line_number, raw_line) in source.lines().enumerate() {
            let line = raw_line.split('#').next().unwrap_or_default().trim();
            if line.is_empty() {
                continue;
            }
            let Some((assignment, name, args)) = parse_statement(line) else {
                return Err(format!(
                    "line {} is not a supported MODULE.bazel directive",
                    line_number + 1
                ));
            };
            match name {
                "use_extension" => {
                    let Some(proxy_name) = assignment else {
                        return Err("use_extension requires assignment to a proxy".to_owned());
                    };
                    directives.push(Directive::UseExtension(parse_use_extension(
                        proxy_name, args,
                    )?));
                    extension_proxies.insert(proxy_name.to_owned());
                }
                "use_repo_rule" => {
                    let Some(proxy_name) = assignment else {
                        return Err("use_repo_rule requires assignment to a proxy".to_owned());
                    };
                    directives.push(Directive::UseRepoRule(parse_use_repo_rule(
                        proxy_name, args,
                    )?));
                    repo_rule_proxies.insert(proxy_name.to_owned());
                }
                other if assignment.is_some() => {
                    return Err(format!("{other} does not support assignment"));
                }
                "module" => {
                    module = Some(parse_module(args)?);
                }
                "include" => directives.push(Directive::Include(parse_single_label_arg(args)?)),
                "bazel_dep" => directives.push(Directive::BazelDep(parse_bazel_dep(args)?)),
                "local_path_override" => {
                    directives.push(Directive::LocalPathOverride(parse_local_path_override(
                        args,
                    )?));
                }
                "single_version_override" => {
                    directives.push(Directive::SingleVersionOverride(
                        parse_single_version_override(args)?,
                    ));
                }
                "multiple_version_override" => {
                    directives.push(Directive::MultipleVersionOverride(
                        parse_multiple_version_override(args)?,
                    ));
                }
                "archive_override" => {
                    directives.push(Directive::ArchiveOverride(parse_archive_override(args)?));
                }
                "git_override" => {
                    directives.push(Directive::GitOverride(parse_git_override(args)?));
                }
                "use_repo" => {
                    directives.push(Directive::UseRepo(parse_use_repo(args)?));
                }
                "override_repo" => {
                    directives.push(Directive::OverrideRepo(parse_override_repo(args)?));
                }
                "inject_repo" => {
                    directives.push(Directive::InjectRepo(parse_inject_repo(args)?));
                }
                "register_toolchains" => {
                    directives.push(Directive::RegisterToolchains(parse_registration(args)?));
                }
                "register_execution_platforms" => {
                    directives.push(Directive::RegisterExecutionPlatforms(parse_registration(
                        args,
                    )?));
                }
                other if is_extension_tag_call(other, &extension_proxies) => {
                    directives.push(Directive::ExtensionTag(parse_extension_tag(other, args)?));
                }
                other if repo_rule_proxies.contains(other) => {
                    directives.push(Directive::RepoRuleInvocation(parse_repo_rule_invocation(
                        other, args,
                    )?));
                }
                other => return Err(format!("unsupported MODULE.bazel directive: {other}")),
            }
        }
        Ok(Self { module, directives })
    }
}

fn parse_statement(line: &str) -> Option<(Option<&str>, &str, &str)> {
    let (name, args) = parse_call(line)?;
    let Some((assignment, name)) = name.split_once('=') else {
        return Some((None, name.trim(), args));
    };
    Some((Some(assignment.trim()), name.trim(), args))
}

fn parse_call(line: &str) -> Option<(&str, &str)> {
    let (name, rest) = line.split_once('(')?;
    let args = rest.strip_suffix(')')?;
    Some((name.trim(), args.trim()))
}

fn parse_module(args: &str) -> Result<ModuleHeader, String> {
    let kwargs = parse_kwargs(args)?;
    let name = required_string(&kwargs, "name")?.to_owned();
    Ok(ModuleHeader {
        name,
        version: optional_string(&kwargs, "version")?.map(str::to_owned),
        repo_name: optional_string(&kwargs, "repo_name")?.map(str::to_owned),
        compatibility_level: kwargs
            .get("compatibility_level")
            .map(|value| {
                value
                    .as_u64()
                    .ok_or_else(|| "compatibility_level must be an integer".to_owned())
            })
            .transpose()?,
        bazel_compatibility: optional_string_list(&kwargs, "bazel_compatibility")?
            .unwrap_or_default(),
    })
}

fn parse_bazel_dep(args: &str) -> Result<BazelDep, String> {
    let kwargs = parse_kwargs(args)?;
    Ok(BazelDep {
        name: required_string(&kwargs, "name")?.to_owned(),
        version: required_string(&kwargs, "version")?.to_owned(),
        repo_name: optional_string(&kwargs, "repo_name")?.map(str::to_owned),
        dev_dependency: optional_bool(&kwargs, "dev_dependency")?.unwrap_or(false),
    })
}

fn parse_local_path_override(args: &str) -> Result<LocalPathOverride, String> {
    let kwargs = parse_kwargs(args)?;
    Ok(LocalPathOverride {
        module_name: required_string(&kwargs, "module_name")?.to_owned(),
        path: required_string(&kwargs, "path")?.to_owned(),
    })
}

fn parse_single_version_override(args: &str) -> Result<SingleVersionOverride, String> {
    let kwargs = parse_kwargs(args)?;
    Ok(SingleVersionOverride {
        module_name: required_string(&kwargs, "module_name")?.to_owned(),
        version: required_string(&kwargs, "version")?.to_owned(),
        registry: optional_string(&kwargs, "registry")?.map(str::to_owned),
        patches: optional_string_list(&kwargs, "patches")?.unwrap_or_default(),
        patch_cmds: optional_string_list(&kwargs, "patch_cmds")?.unwrap_or_default(),
        patch_strip: optional_u64(&kwargs, "patch_strip")?.unwrap_or(0),
    })
}

fn parse_multiple_version_override(args: &str) -> Result<MultipleVersionOverride, String> {
    let kwargs = parse_kwargs(args)?;
    Ok(MultipleVersionOverride {
        module_name: required_string(&kwargs, "module_name")?.to_owned(),
        versions: required_string_list(&kwargs, "versions")?,
        registry: optional_string(&kwargs, "registry")?.map(str::to_owned),
    })
}

fn parse_archive_override(args: &str) -> Result<ArchiveOverride, String> {
    let kwargs = parse_kwargs(args)?;
    Ok(ArchiveOverride {
        module_name: required_string(&kwargs, "module_name")?.to_owned(),
        urls: required_string_list(&kwargs, "urls")?,
        integrity: optional_string(&kwargs, "integrity")?.map(str::to_owned),
        strip_prefix: optional_string(&kwargs, "strip_prefix")?.map(str::to_owned),
        patches: optional_string_list(&kwargs, "patches")?.unwrap_or_default(),
        patch_strip: optional_u64(&kwargs, "patch_strip")?.unwrap_or(0),
    })
}

fn parse_git_override(args: &str) -> Result<GitOverride, String> {
    let kwargs = parse_kwargs(args)?;
    Ok(GitOverride {
        module_name: required_string(&kwargs, "module_name")?.to_owned(),
        remote: required_string(&kwargs, "remote")?.to_owned(),
        commit: required_string(&kwargs, "commit")?.to_owned(),
        shallow_since: optional_string(&kwargs, "shallow_since")?.map(str::to_owned),
        patches: optional_string_list(&kwargs, "patches")?.unwrap_or_default(),
        patch_strip: optional_u64(&kwargs, "patch_strip")?.unwrap_or(0),
    })
}

fn parse_use_extension(proxy_name: &str, args: &str) -> Result<UseExtension, String> {
    if proxy_name.is_empty() {
        return Err("use_extension proxy name must not be empty".to_owned());
    }
    let parts = split_args(args);
    if parts.len() < 2 {
        return Err("use_extension requires a .bzl label and extension name".to_owned());
    }
    let bzl_label = parse_string_literal(parts[0])
        .ok_or_else(|| "use_extension first argument must be a string label".to_owned())?;
    let extension_name = parse_string_literal(parts[1])
        .ok_or_else(|| "use_extension second argument must be a string name".to_owned())?;
    let kwargs = parse_kwargs_from_parts(&parts[2..])?;
    Ok(UseExtension {
        proxy_name: proxy_name.to_owned(),
        bzl_label,
        extension_name,
        dev_dependency: optional_bool(&kwargs, "dev_dependency")?.unwrap_or(false),
        isolate: optional_bool(&kwargs, "isolate")?.unwrap_or(false),
    })
}

fn is_extension_tag_call(name: &str, extension_proxies: &BTreeSet<String>) -> bool {
    let Some((extension_proxy, tag_class)) = name.split_once('.') else {
        return false;
    };
    extension_proxies.contains(extension_proxy) && parse_symbol_literal(tag_class).is_some()
}

fn parse_extension_tag(call_name: &str, args: &str) -> Result<ExtensionTag, String> {
    let Some((extension_proxy, tag_class)) = call_name.split_once('.') else {
        return Err(format!(
            "extension tag call must use proxy.tag syntax: {call_name}"
        ));
    };
    let attrs = parse_kwargs(args)?
        .into_iter()
        .map(|(key, value)| (key, value.into()))
        .collect();
    Ok(ExtensionTag {
        extension_proxy: extension_proxy.to_owned(),
        tag_class: tag_class.to_owned(),
        attrs,
    })
}
fn parse_use_repo(args: &str) -> Result<UseRepo, String> {
    let (extension_proxy, repos) = parse_extension_repo_imports("use_repo", args)?;
    Ok(UseRepo {
        extension_proxy,
        repos,
    })
}

fn parse_override_repo(args: &str) -> Result<OverrideRepo, String> {
    let (extension_proxy, repos) = parse_extension_repo_imports("override_repo", args)?;
    Ok(OverrideRepo {
        extension_proxy,
        repos,
    })
}

fn parse_inject_repo(args: &str) -> Result<InjectRepo, String> {
    let (extension_proxy, repos) = parse_extension_repo_imports("inject_repo", args)?;
    Ok(InjectRepo {
        extension_proxy,
        repos,
    })
}

fn parse_extension_repo_imports(
    kind: &str,
    args: &str,
) -> Result<(String, Vec<RepoImport>), String> {
    let parts = split_args(args);
    let Some((first, repos)) = parts.split_first() else {
        return Err(format!("{kind} requires an extension proxy"));
    };
    let extension_proxy = parse_symbol_literal(first)
        .ok_or_else(|| format!("{kind} first argument must be an extension proxy"))?;
    let repos = parse_repo_imports(kind, repos)?;
    Ok((extension_proxy, repos))
}

fn parse_repo_imports(kind: &str, repos: &[&str]) -> Result<Vec<RepoImport>, String> {
    repos
        .iter()
        .map(|repo| {
            if let Some((apparent_name, repo_name)) = repo.split_once('=') {
                let apparent_name = apparent_name.trim();
                let repo_name = parse_string_literal(repo_name.trim()).ok_or_else(|| {
                    format!("{kind} repository mapping must point at a string: {repo}")
                })?;
                return Ok(RepoImport {
                    apparent_name: apparent_name.to_owned(),
                    repo_name,
                });
            }
            let repo_name = parse_string_literal(repo)
                .ok_or_else(|| format!("{kind} repository argument must be a string: {repo}"))?;
            Ok(RepoImport {
                apparent_name: repo_name.clone(),
                repo_name,
            })
        })
        .collect::<Result<Vec<_>, _>>()
}

fn parse_use_repo_rule(proxy_name: &str, args: &str) -> Result<UseRepoRule, String> {
    if proxy_name.is_empty() {
        return Err("use_repo_rule proxy name must not be empty".to_owned());
    }
    let parts = split_args(args);
    if parts.len() < 2 {
        return Err("use_repo_rule requires a .bzl label and repository rule name".to_owned());
    }
    let bzl_label = parse_string_literal(parts[0])
        .ok_or_else(|| "use_repo_rule first argument must be a string label".to_owned())?;
    let rule_name = parse_string_literal(parts[1])
        .ok_or_else(|| "use_repo_rule second argument must be a string name".to_owned())?;
    let kwargs = parse_kwargs_from_parts(&parts[2..])?;
    Ok(UseRepoRule {
        proxy_name: proxy_name.to_owned(),
        bzl_label,
        rule_name,
        dev_dependency: optional_bool(&kwargs, "dev_dependency")?.unwrap_or(false),
    })
}

fn parse_repo_rule_invocation(rule_proxy: &str, args: &str) -> Result<RepoRuleInvocation, String> {
    let kwargs = parse_kwargs(args)?;
    let repo_name = required_string(&kwargs, "name")?.to_owned();
    let attrs = kwargs
        .into_iter()
        .filter(|(key, _)| key != "name")
        .map(|(key, value)| (key, value.into()))
        .collect();
    Ok(RepoRuleInvocation {
        rule_proxy: rule_proxy.to_owned(),
        repo_name,
        attrs,
    })
}

fn parse_registration(args: &str) -> Result<Registration, String> {
    let parts = split_args(args);
    let mut labels = Vec::new();
    let mut kwargs = BTreeMap::new();
    for part in parts {
        if part.contains('=') {
            for (key, value) in parse_kwargs_from_parts(&[part])? {
                kwargs.insert(key, value);
            }
        } else {
            labels.push(
                parse_string_literal(part).ok_or_else(|| {
                    format!("registration argument must be a string label: {part}")
                })?,
            );
        }
    }
    Ok(Registration {
        labels,
        dev_dependency: optional_bool(&kwargs, "dev_dependency")?.unwrap_or(false),
    })
}
fn parse_single_label_arg(args: &str) -> Result<String, String> {
    let labels = parse_label_args(args)?;
    if labels.len() != 1 {
        return Err("include requires exactly one label".to_owned());
    }
    Ok(labels.into_iter().next().unwrap())
}

fn parse_label_args(args: &str) -> Result<Vec<String>, String> {
    split_args(args)
        .into_iter()
        .map(|arg| {
            parse_string_literal(arg)
                .ok_or_else(|| format!("registration argument must be a string label: {arg}"))
        })
        .collect()
}

fn parse_kwargs(args: &str) -> Result<BTreeMap<String, Value>, String> {
    let parts = split_args(args);
    parse_kwargs_from_parts(&parts)
}

fn parse_kwargs_from_parts(parts: &[&str]) -> Result<BTreeMap<String, Value>, String> {
    let mut kwargs = BTreeMap::new();
    for arg in parts {
        let Some((key, value)) = arg.split_once('=') else {
            return Err(format!("expected keyword argument, got {arg}"));
        };
        let Some(value) = parse_value(value.trim()) else {
            return Err(format!("unsupported value for {key}"));
        };
        kwargs.insert(key.trim().to_owned(), value);
    }
    Ok(kwargs)
}

fn required_string<'a>(kwargs: &'a BTreeMap<String, Value>, key: &str) -> Result<&'a str, String> {
    kwargs
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing required argument: {key}"))
}

fn optional_string<'a>(
    kwargs: &'a BTreeMap<String, Value>,
    key: &str,
) -> Result<Option<&'a str>, String> {
    kwargs
        .get(key)
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| format!("{key} must be a string"))
        })
        .transpose()
}

fn required_string_list(
    kwargs: &BTreeMap<String, Value>,
    key: &str,
) -> Result<Vec<String>, String> {
    optional_string_list(kwargs, key)?.ok_or_else(|| format!("missing required argument: {key}"))
}

fn optional_string_list(
    kwargs: &BTreeMap<String, Value>,
    key: &str,
) -> Result<Option<Vec<String>>, String> {
    kwargs
        .get(key)
        .map(|value| {
            value
                .as_string_list()
                .map(|value| value.to_vec())
                .ok_or_else(|| format!("{key} must be a list of strings"))
        })
        .transpose()
}

fn optional_bool(kwargs: &BTreeMap<String, Value>, key: &str) -> Result<Option<bool>, String> {
    kwargs
        .get(key)
        .map(|value| {
            value
                .as_bool()
                .ok_or_else(|| format!("{key} must be a boolean"))
        })
        .transpose()
}

fn optional_u64(kwargs: &BTreeMap<String, Value>, key: &str) -> Result<Option<u64>, String> {
    kwargs
        .get(key)
        .map(|value| {
            value
                .as_u64()
                .ok_or_else(|| format!("{key} must be an integer"))
        })
        .transpose()
}

fn split_args(args: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut start = 0;
    let mut in_string = false;
    let mut list_depth = 0u32;
    for (index, ch) in args.char_indices() {
        match ch {
            '"' => in_string = !in_string,
            '[' if !in_string => list_depth += 1,
            ']' if !in_string && list_depth > 0 => list_depth -= 1,
            ',' if !in_string && list_depth == 0 => {
                let part = args[start..index].trim();
                if !part.is_empty() {
                    result.push(part);
                }
                start = index + 1;
            }
            _ => {}
        }
    }
    let part = args[start..].trim();
    if !part.is_empty() {
        result.push(part);
    }
    result
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Value {
    String(String),
    StringList(Vec<String>),
    Integer(u64),
    Bool(bool),
}

impl From<Value> for ModuleAttributeValue {
    fn from(value: Value) -> Self {
        match value {
            Value::String(value) => Self::String(value),
            Value::StringList(value) => Self::StringList(value),
            Value::Integer(value) => Self::Integer(value),
            Value::Bool(value) => Self::Bool(value),
        }
    }
}

impl Value {
    fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    fn as_string_list(&self) -> Option<&[String]> {
        match self {
            Self::StringList(value) => Some(value),
            _ => None,
        }
    }

    fn as_u64(&self) -> Option<u64> {
        match self {
            Self::Integer(value) => Some(*value),
            _ => None,
        }
    }

    fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }
}

fn parse_symbol_literal(value: &str) -> Option<String> {
    let mut chars = value.chars();
    let first = chars.next()?;
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return None;
    }
    if !chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric()) {
        return None;
    }
    Some(value.to_owned())
}

fn parse_string_literal(value: &str) -> Option<String> {
    if let Some(value) = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    {
        return Some(value.to_owned());
    }
    None
}

fn parse_value(value: &str) -> Option<Value> {
    if let Some(value) = parse_string_literal(value) {
        return Some(Value::String(value));
    }
    if let Some(value) = value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
    {
        let mut values = Vec::new();
        for item in split_args(value) {
            values.push(parse_string_literal(item)?);
        }
        return Some(Value::StringList(values));
    }
    if value.chars().all(|ch| ch.is_ascii_digit()) {
        return Some(Value::Integer(value.parse().ok()?));
    }
    match value {
        "True" | "true" => Some(Value::Bool(true)),
        "False" | "false" => Some(Value::Bool(false)),
        _ => None,
    }
}
