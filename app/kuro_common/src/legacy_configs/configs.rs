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
use std::fmt::Display;
use std::sync::Arc;

use allocative::Allocative;
use dupe::Dupe;
use kuro_cli_proto::ConfigOverride;
use pagable::Pagable;
use starlark_map::sorted_map::SortedMap;

use crate::legacy_configs::key::BuckconfigKeyRef;

#[derive(Clone, Dupe, Debug, Allocative, Pagable)]
pub struct LegacyBuckConfig(pub(crate) Arc<ConfigData>);

#[derive(Debug, Allocative, Pagable)]
pub(crate) struct ConfigData {
    pub(crate) values: SortedMap<String, LegacyBuckConfigSection>,
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative, Pagable)]
pub(crate) enum ResolvedValue {
    // A placeholder used before we do resolution.
    Unknown,
    // Indicates that there's no resolution required, the resolved value and raw value are the same.
    Literal,
    // The resolved value for non-literals.
    Resolved(String),
}

#[derive(Debug, PartialEq, Eq, Allocative, Pagable)]
pub(crate) struct ConfigFileLocation {
    pub(crate) path: String,
    pub(crate) include_source: Option<Location>,
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative, Pagable)]
pub(crate) struct ConfigFileLocationWithLine {
    pub(crate) source_file: Arc<ConfigFileLocation>,
    pub(crate) line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative, Pagable)]
pub(crate) enum Location {
    File(ConfigFileLocationWithLine),
    CommandLineArgument,
}

impl Location {
    pub(crate) fn as_legacy_buck_config_location(&self) -> LegacyBuckConfigLocation<'_> {
        match self {
            Self::File(x) => LegacyBuckConfigLocation::File(&x.source_file.path, x.line),
            Self::CommandLineArgument => LegacyBuckConfigLocation::CommandLineArgument,
        }
    }
}

// Represents a config section and key only, for example, `cxx.compiler`.
#[derive(Clone, Debug)]
pub struct ConfigSectionAndKey {
    //  TODO(scottcao): Add cell_path
    pub section: String,
    pub key: String,
}

#[derive(kuro_error::Error, Debug)]
#[kuro(input)]
pub(crate) enum ConfigArgumentParseError {
    #[error("Could not find section separator (`.`) in pair `{0}`")]
    NoSectionDotSeparator(String),
    #[error("Could not find equals sign (`=`) in pair `{0}`")]
    NoEqualsSeparator(String),

    #[error("Expected key-value in format of `section.key=value` but only got `{0}`")]
    MissingData(String),

    #[error("Contains whitespace in key-value pair `{0}`")]
    WhitespaceInKeyOrValue(String),

    #[error("Specifying cells via cli config overrides is banned (`{0}.key=value`)")]
    CellOverrideViaCliConfig(&'static str),
}

// Parses config key in the format `section.key`
pub fn parse_config_section_and_key(
    raw_section_and_key: &str,
    raw_arg_in_err: Option<&str>, // Used in error strings to preserve the original config argument, not just section and key
) -> kuro_error::Result<ConfigSectionAndKey> {
    let raw_arg = raw_arg_in_err.unwrap_or(raw_section_and_key);
    let (raw_section, raw_key) = raw_section_and_key
        .split_once('.')
        .ok_or_else(|| ConfigArgumentParseError::NoSectionDotSeparator(raw_arg.to_owned()))?;

    // We only trim the section + key, whitespace in values needs to be preserved. For example,
    // Buck can be invoked with --config section.key="Some Value" that contains important whitespace.
    let trimmed_section = raw_section.trim_start();
    if trimmed_section.find(char::is_whitespace).is_some()
        || raw_key.find(char::is_whitespace).is_some()
    {
        return Err(ConfigArgumentParseError::WhitespaceInKeyOrValue(raw_arg.to_owned()).into());
    }

    if trimmed_section.is_empty() || raw_key.is_empty() {
        return Err(ConfigArgumentParseError::MissingData(raw_arg.to_owned()).into());
    }

    Ok(ConfigSectionAndKey {
        section: trimmed_section.to_owned(),
        key: raw_key.to_owned(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Pagable)]
pub(crate) struct ConfigValue {
    raw_value: String,
    pub(crate) resolved_value: ResolvedValue,
    pub(crate) source: Location,
}

#[derive(Debug, Default, Allocative, Pagable)]
pub struct LegacyBuckConfigSection {
    pub(crate) values: SortedMap<String, ConfigValue>,
}

impl ConfigValue {
    pub(crate) fn new_raw(source: ConfigFileLocationWithLine, value: String) -> Self {
        Self {
            raw_value: value,
            resolved_value: ResolvedValue::Unknown,
            source: Location::File(source),
        }
    }

    pub(crate) fn new_raw_arg(raw_value: String) -> Self {
        Self {
            raw_value,
            resolved_value: ResolvedValue::Unknown,
            source: Location::CommandLineArgument,
        }
    }

    pub(crate) fn raw_value(&self) -> &str {
        &self.raw_value
    }

    pub(crate) fn as_str(&self) -> &str {
        match &self.resolved_value {
            ResolvedValue::Literal => &self.raw_value,
            ResolvedValue::Resolved(v) => v,
            ResolvedValue::Unknown => {
                unreachable!("cannot call as_str() until all values are resolved")
            }
        }
    }
}

pub struct LegacyBuckConfigValue<'a> {
    pub(crate) value: &'a ConfigValue,
}

#[derive(PartialEq, Debug)]
pub enum LegacyBuckConfigLocation<'a> {
    File(&'a str, usize),
    CommandLineArgument,
}

impl Display for LegacyBuckConfigLocation<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::File(file, line) => {
                write!(f, "at {file}:{line}")
            }
            Self::CommandLineArgument => {
                write!(f, "on the command line")
            }
        }
    }
}

impl<'a> LegacyBuckConfigValue<'a> {
    pub fn as_str(&self) -> &'a str {
        self.value.as_str()
    }

    pub fn raw_value(&self) -> &str {
        self.value.raw_value()
    }

    pub fn location(&self) -> LegacyBuckConfigLocation<'_> {
        match &self.value.source {
            Location::File(file) => {
                LegacyBuckConfigLocation::File(&file.source_file.path, file.line)
            }
            Location::CommandLineArgument => LegacyBuckConfigLocation::CommandLineArgument,
        }
    }

    pub fn location_stack(&self) -> Vec<LegacyBuckConfigLocation<'_>> {
        let mut res = Vec::new();
        let mut location = Some(&self.value.source);

        while let Some(loc) = location.take() {
            match &loc {
                Location::File(loc) => {
                    res.push(LegacyBuckConfigLocation::File(
                        &loc.source_file.path,
                        loc.line,
                    ));
                    location = loc.source_file.include_source.as_ref();
                }
                Location::CommandLineArgument => {
                    // No stack
                }
            }
        }
        res
    }
}

impl LegacyBuckConfig {
    pub fn empty() -> Self {
        Self(Arc::new(ConfigData {
            values: SortedMap::new(),
        }))
    }

    /// Build a config from CLI `-c section.key=value` overrides only.
    ///
    /// This is the Q1=B constructor: no `.buckconfig` file parsing is performed.
    /// All values are stored with `ResolvedValue::Literal` so `as_str()` works
    /// immediately (no `$(config ...)` interpolation, which does not apply to
    /// CLI flags).
    pub fn from_overrides_only(
        args: &[kuro_cli_proto::ConfigOverride],
    ) -> kuro_error::Result<Self> {
        use std::collections::BTreeMap;

        use kuro_cli_proto::config_override::ConfigType;

        let mut sections: BTreeMap<String, std::collections::BTreeMap<String, ConfigValue>> =
            BTreeMap::new();

        for arg in args {
            let config_type = ConfigType::try_from(arg.config_type).map_err(|_| {
                kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Input,
                    "Unknown ConfigType enum value `{}` in config override",
                    arg.config_type
                )
            })?;
            if config_type != ConfigType::Value {
                // Skip --config-file args; they are not read from disk in this path.
                continue;
            }
            let raw = &arg.config_override;
            let (sk, v) = raw.split_once('=').ok_or_else(|| {
                kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Input,
                    "Could not find equals sign (`=`) in config override `{}`",
                    raw
                )
            })?;
            let (s, k) = sk.split_once('.').ok_or_else(|| {
                kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Input,
                    "Could not find section separator (`.`) in config override `{}`",
                    raw
                )
            })?;
            let s = s.trim_start();
            if s.is_empty() || k.is_empty() {
                continue;
            }
            if v.is_empty() {
                // Empty value means unset: remove the key if it exists.
                if let Some(section) = sections.get_mut(s) {
                    section.remove(k);
                }
            } else {
                let value = ConfigValue {
                    raw_value: v.to_owned(),
                    resolved_value: ResolvedValue::Literal,
                    source: Location::CommandLineArgument,
                };
                sections
                    .entry(s.to_owned())
                    .or_default()
                    .insert(k.to_owned(), value);
            }
        }

        let values = sections
            .into_iter()
            .map(|(s, v)| {
                (
                    s,
                    LegacyBuckConfigSection {
                        values: v.into_iter().collect(),
                    },
                )
            })
            .collect();

        Ok(Self(Arc::new(ConfigData { values })))
    }

    /// Build a config from already-resolved CLI flag overrides only.
    ///
    /// Like `from_overrides_only` but accepts `ResolvedConfigFlag` values that
    /// have already been parsed from the raw `section.key=value` string.
    /// File-type `ResolvedLegacyConfigArg` variants are silently skipped.
    pub(crate) fn from_resolved_flags(
        args: &[crate::legacy_configs::args::ResolvedLegacyConfigArg],
    ) -> Self {
        use std::collections::BTreeMap;

        let mut sections: BTreeMap<String, std::collections::BTreeMap<String, ConfigValue>> =
            BTreeMap::new();

        for arg in args {
            let flag = match arg {
                crate::legacy_configs::args::ResolvedLegacyConfigArg::Flag(f) => f,
                _ => continue, // skip File variants
            };
            let s = &flag.section;
            let k = &flag.key;
            if let Some(v) = &flag.value {
                let value = ConfigValue {
                    raw_value: v.clone(),
                    resolved_value: ResolvedValue::Literal,
                    source: Location::CommandLineArgument,
                };
                sections
                    .entry(s.clone())
                    .or_default()
                    .insert(k.clone(), value);
            } else {
                // None value means this config is unset.
                if let Some(section) = sections.get_mut(s.as_str()) {
                    section.remove(k.as_str());
                }
            }
        }

        let values = sections
            .into_iter()
            .map(|(s, v)| {
                (
                    s,
                    LegacyBuckConfigSection {
                        values: v.into_iter().collect(),
                    },
                )
            })
            .collect();

        Self(Arc::new(ConfigData { values }))
    }

    pub fn filter_values<F>(&self, filter: F) -> Self
    where
        F: Fn(&BuckconfigKeyRef) -> bool,
    {
        let values = self
            .0
            .values
            .iter()
            .filter_map(|(section, section_data)| {
                let values: SortedMap<_, _> = section_data
                    .values
                    .iter()
                    .filter(|(property, _)| filter(&BuckconfigKeyRef { section, property }))
                    .map(|(property, value)| (property.clone(), value.clone()))
                    .collect();
                if values.is_empty() {
                    None
                } else {
                    Some((section.clone(), LegacyBuckConfigSection { values }))
                }
            })
            .collect();
        Self(Arc::new(ConfigData { values }))
    }
}

pub mod testing {
    use super::*;

    /// Build a `LegacyBuckConfig` from CLI config overrides only (no file data).
    ///
    /// Q1=B replacement for the old `parse(file_data, path)` helper. The `data`
    /// and `path` parameters are accepted but ignored so that call sites that
    /// haven't been migrated still compile; they should be removed over time.
    #[allow(unused_variables)]
    pub fn parse(_data: &[(&str, &str)], _path: &str) -> kuro_error::Result<LegacyBuckConfig> {
        Ok(LegacyBuckConfig::empty())
    }

    /// Build a `LegacyBuckConfig` from CLI config overrides only.
    ///
    /// The `data` and `cell_path` parameters are ignored; only `config_args` is used.
    #[allow(unused_variables)]
    pub fn parse_with_config_args(
        _data: &[(&str, &str)],
        _cell_path: &str,
        config_args: &[ConfigOverride],
    ) -> kuro_error::Result<LegacyBuckConfig> {
        LegacyBuckConfig::from_overrides_only(config_args)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use itertools::Itertools;
    use kuro_core::cells::cell_root_path::CellRootPathBuf;

    use super::*;
    use crate::legacy_configs::key::BuckconfigKeyRef;

    pub(crate) fn assert_config_value(
        config: &LegacyBuckConfig,
        section: &str,
        key: &str,
        expected: &str,
    ) {
        match config.get_section(section) {
            None => {
                panic!(
                    "Expected config to have section `{}`, but had sections `<{}>`",
                    section,
                    config.sections().join(", ")
                );
            }
            Some(values) => match values.get(key) {
                None => panic!(
                    "Expected section `{}` to have key `{}`, but had keys `<{}>`",
                    section,
                    key,
                    values.keys().join(", ")
                ),
                Some(v) if v.as_str() != expected => {
                    panic!(
                        "Expected `{}.{}` to have value `{}`. Got `{}`.",
                        section,
                        key,
                        expected,
                        v.as_str()
                    );
                }
                _ => {}
            },
        }
    }

    pub(crate) fn assert_config_value_is_empty(
        config: &LegacyBuckConfig,
        section: &str,
        key: &str,
    ) {
        match config.get_section(section) {
            Some(values) => match values.get(key) {
                Some(v) => {
                    panic!(
                        "Expected `{}.{}` to not exist. Got `{}` for value.",
                        section,
                        key,
                        v.as_str()
                    );
                }
                _ => {}
            },
            _ => {}
        };
    }

    #[test]
    fn test_config_args_ordering() -> kuro_error::Result<()> {
        let config_args = vec![
            ConfigOverride::flag_no_cell("apple.key=value1"),
            ConfigOverride::flag_no_cell("apple.key=value2"),
        ];
        let config = LegacyBuckConfig::from_overrides_only(&config_args)?;
        assert_config_value(&config, "apple", "key", "value2");
        Ok(())
    }

    #[test]
    fn test_config_args_empty() -> kuro_error::Result<()> {
        let config_args = vec![ConfigOverride::flag_no_cell("apple.key=")];
        let config = LegacyBuckConfig::from_overrides_only(&config_args)?;
        assert_config_value_is_empty(&config, "apple", "key");
        Ok(())
    }

    #[test]
    fn test_config_args_cli_flag_wins() -> kuro_error::Result<()> {
        // Q1=B: only CLI args contribute; this tests last-wins ordering.
        let config_args = vec![ConfigOverride::flag_no_cell("apple.key=value2")];
        let config = LegacyBuckConfig::from_overrides_only(&config_args)?;
        assert_config_value(&config, "apple", "key", "value2");
        assert_eq!(
            config
                .get_section("apple")
                .unwrap()
                .get("key")
                .unwrap()
                .location(),
            LegacyBuckConfigLocation::CommandLineArgument
        );
        Ok(())
    }

    #[test]
    fn test_section_and_key() -> kuro_error::Result<()> {
        // Valid Formats
        let normal_section_and_key = parse_config_section_and_key("apple.key", None)?;
        assert_eq!("apple", normal_section_and_key.section);
        assert_eq!("key", normal_section_and_key.key);

        // Whitespace
        let section_leading_whitespace = parse_config_section_and_key("  apple.key", None)?;
        assert_eq!("apple", section_leading_whitespace.section);
        assert_eq!("key", section_leading_whitespace.key);

        let pair_with_whitespace_in_key = parse_config_section_and_key("apple. key", None);
        assert!(pair_with_whitespace_in_key.is_err());

        // Invalid Formats
        let pair_without_dot = parse_config_section_and_key("applekey", None);
        assert!(pair_without_dot.is_err());

        Ok(())
    }

    #[test]
    fn test_config_args_cell_in_value() -> kuro_error::Result<()> {
        let config_args = vec![ConfigOverride::flag_no_cell("apple.key=foo//value1")];
        let config = LegacyBuckConfig::from_overrides_only(&config_args)?;
        assert_config_value(&config, "apple", "key", "foo//value1");
        Ok(())
    }

    #[test]
    fn test_from_overrides_only_basic() -> kuro_error::Result<()> {
        let args = vec![
            ConfigOverride::flag_no_cell("section.key=value"),
            ConfigOverride::flag_no_cell("section.other=hello"),
            ConfigOverride::flag_no_cell("other.x=42"),
        ];
        let config = LegacyBuckConfig::from_overrides_only(&args)?;
        assert_config_value(&config, "section", "key", "value");
        assert_config_value(&config, "section", "other", "hello");
        assert_config_value(&config, "other", "x", "42");
        // Location must be CommandLineArgument
        let v = config.get_section("section").unwrap().get("key").unwrap();
        assert_eq!(v.location(), LegacyBuckConfigLocation::CommandLineArgument);
        Ok(())
    }

    #[test]
    fn test_from_overrides_only_last_wins() -> kuro_error::Result<()> {
        let args = vec![
            ConfigOverride::flag_no_cell("section.key=first"),
            ConfigOverride::flag_no_cell("section.key=second"),
        ];
        let config = LegacyBuckConfig::from_overrides_only(&args)?;
        assert_config_value(&config, "section", "key", "second");
        Ok(())
    }

    #[test]
    fn test_from_overrides_only_empty_value_unsets() -> kuro_error::Result<()> {
        let args = vec![
            ConfigOverride::flag_no_cell("section.key=value"),
            ConfigOverride::flag_no_cell("section.key="),
        ];
        let config = LegacyBuckConfig::from_overrides_only(&args)?;
        assert_config_value_is_empty(&config, "section", "key");
        Ok(())
    }

    #[test]
    fn test_from_overrides_only_skips_file_args() -> kuro_error::Result<()> {
        let args = vec![
            ConfigOverride::flag_no_cell("section.key=value"),
            ConfigOverride::file("some-config-file", Some(CellRootPathBuf::testing_new(""))),
        ];
        // File args are skipped; no panic/error
        let config = LegacyBuckConfig::from_overrides_only(&args)?;
        assert_config_value(&config, "section", "key", "value");
        Ok(())
    }
}
