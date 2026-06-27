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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleFile {
    pub module: Option<ModuleHeader>,
    pub directives: Vec<Directive>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleHeader {
    pub name: String,
    pub version: Option<String>,
    pub compatibility_level: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Directive {
    Include(String),
    BazelDep(BazelDep),
    LocalPathOverride(LocalPathOverride),
    RegisterToolchains(Vec<String>),
    RegisterExecutionPlatforms(Vec<String>),
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

impl ModuleFile {
    pub fn parse(source: &str) -> Result<Self, String> {
        let mut module = None;
        let mut directives = Vec::new();
        for (line_number, raw_line) in source.lines().enumerate() {
            let line = raw_line.split('#').next().unwrap_or_default().trim();
            if line.is_empty() {
                continue;
            }
            let Some((name, args)) = parse_call(line) else {
                return Err(format!(
                    "line {} is not a supported MODULE.bazel directive",
                    line_number + 1
                ));
            };
            match name {
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
                "register_toolchains" => {
                    directives.push(Directive::RegisterToolchains(parse_label_args(args)?));
                }
                "register_execution_platforms" => {
                    directives.push(Directive::RegisterExecutionPlatforms(parse_label_args(
                        args,
                    )?));
                }
                other => return Err(format!("unsupported MODULE.bazel directive: {other}")),
            }
        }
        Ok(Self { module, directives })
    }
}

fn parse_call(line: &str) -> Option<(&str, &str)> {
    let (name, rest) = line.split_once('(')?;
    let args = rest.strip_suffix(')')?;
    Some((name.trim(), args.trim()))
}

fn parse_module(args: &str) -> Result<ModuleHeader, String> {
    let kwargs = parse_kwargs(args)?;
    let name = required(&kwargs, "name")?.to_owned();
    Ok(ModuleHeader {
        name,
        version: kwargs.get("version").cloned(),
        compatibility_level: kwargs
            .get("compatibility_level")
            .map(|value| {
                value
                    .parse::<u64>()
                    .map_err(|_| "compatibility_level must be an integer".to_owned())
            })
            .transpose()?,
    })
}

fn parse_bazel_dep(args: &str) -> Result<BazelDep, String> {
    let kwargs = parse_kwargs(args)?;
    Ok(BazelDep {
        name: required(&kwargs, "name")?.to_owned(),
        version: required(&kwargs, "version")?.to_owned(),
        repo_name: kwargs.get("repo_name").cloned(),
        dev_dependency: kwargs
            .get("dev_dependency")
            .is_some_and(|value| matches!(value.as_str(), "True" | "true")),
    })
}

fn parse_local_path_override(args: &str) -> Result<LocalPathOverride, String> {
    let kwargs = parse_kwargs(args)?;
    Ok(LocalPathOverride {
        module_name: required(&kwargs, "module_name")?.to_owned(),
        path: required(&kwargs, "path")?.to_owned(),
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
            parse_value(arg)
                .ok_or_else(|| format!("registration argument must be a string label: {arg}"))
        })
        .collect()
}

fn parse_kwargs(args: &str) -> Result<BTreeMap<String, String>, String> {
    let mut kwargs = BTreeMap::new();
    for arg in split_args(args) {
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

fn required<'a>(kwargs: &'a BTreeMap<String, String>, key: &str) -> Result<&'a str, String> {
    kwargs
        .get(key)
        .map(String::as_str)
        .ok_or_else(|| format!("missing required argument: {key}"))
}

fn split_args(args: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut start = 0;
    let mut in_string = false;
    for (index, ch) in args.char_indices() {
        match ch {
            '"' => in_string = !in_string,
            ',' if !in_string => {
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

fn parse_value(value: &str) -> Option<String> {
    if let Some(value) = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    {
        return Some(value.to_owned());
    }
    if value.chars().all(|ch| ch.is_ascii_digit()) {
        return Some(value.to_owned());
    }
    if matches!(value, "True" | "False" | "true" | "false") {
        return Some(value.to_owned());
    }
    None
}
