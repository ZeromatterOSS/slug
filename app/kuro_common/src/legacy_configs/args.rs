/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use kuro_cli_proto::ConfigOverride;
use kuro_cli_proto::config_override::ConfigType;
use kuro_core::cells::cell_root_path::CellRootPathBuf;
use kuro_error::BuckErrorContext;

use crate::legacy_configs::configs::ConfigArgumentParseError;
use crate::legacy_configs::configs::ConfigSectionAndKey;
use crate::legacy_configs::configs::parse_config_section_and_key;
use crate::legacy_configs::file_ops::ConfigParserFileOps;

/// Representation of a processed config arg, namely after file path resolution has been performed.
///
/// Q1=B: only the `Flag` variant is used; `File` is kept as a tombstone so that
/// old clients sending `ConfigType::File` receive a clear error rather than a panic.
#[derive(Debug, Clone, PartialEq, Eq, allocative::Allocative)]
pub(crate) enum ResolvedLegacyConfigArg {
    /// A single config key-value pair (in `a.b=c` format).
    Flag(ResolvedConfigFlag),
}

#[derive(Clone, Debug, PartialEq, Eq, allocative::Allocative)]
pub(crate) struct ResolvedConfigFlag {
    pub(crate) section: String,
    pub(crate) key: String,
    // None value means this config is unset.
    pub(crate) value: Option<String>,
    // If this arg only applies to one cell, the root of that cell.
    pub(crate) cell: Option<CellRootPathBuf>,
}

fn resolve_config_flag_arg(
    cell: Option<CellRootPathBuf>,
    raw_arg: &str,
) -> kuro_error::Result<ResolvedConfigFlag> {
    let (raw_section_and_key, raw_value) = raw_arg
        .split_once('=')
        .ok_or_else(|| ConfigArgumentParseError::NoEqualsSeparator(raw_arg.to_owned()))?;
    let ConfigSectionAndKey { section, key } =
        parse_config_section_and_key(raw_section_and_key, Some(raw_arg))?;

    let value = match raw_value {
        "" => None, // An empty string unsets this config.
        v => Some(v.to_owned()),
    };

    Ok(ResolvedConfigFlag {
        cell,
        section,
        key,
        value,
    })
}

pub(crate) async fn resolve_config_args(
    args: &[ConfigOverride],
    _file_ops: &mut dyn ConfigParserFileOps,
) -> kuro_error::Result<Vec<ResolvedLegacyConfigArg>> {
    let mut resolved_args = Vec::new();

    for u in args {
        let config_type = ConfigType::try_from(u.config_type).with_buck_error_context(|| {
            format!(
                "Unknown ConfigType enum value `{}` when trying to deserialize",
                u.config_type
            )
        })?;
        let resolved = match config_type {
            ConfigType::Value => {
                let cell = u.get_cell()?.map(|p| p.to_buf());
                let resolved_flag = resolve_config_flag_arg(cell, &u.config_override)?;
                ResolvedLegacyConfigArg::Flag(resolved_flag)
            }
            ConfigType::File => {
                // Q1=B: --config-file is no longer supported; .buckconfig file parsing has
                // been removed. Clients sending ConfigType::File will have their file args
                // silently skipped. A future release may return an error here.
                tracing::warn!(
                    "--config-file (`{}`) is not supported and will be ignored",
                    u.config_override
                );
                continue;
            }
        };
        resolved_args.push(resolved);
    }

    Ok(resolved_args)
}

pub(crate) fn to_proto_config_args(
    args: &[ResolvedLegacyConfigArg],
) -> Vec<kuro_data::BuckconfigComponent> {
    use kuro_data::buckconfig_component::Data::ConfigValue;

    args.iter()
        .map(|arg| {
            let data = match arg {
                ResolvedLegacyConfigArg::Flag(resolved_config_flag) => {
                    ConfigValue(kuro_data::ConfigValue {
                        section: resolved_config_flag.section.to_owned(),
                        key: resolved_config_flag.key.to_owned(),
                        value: resolved_config_flag
                            .value
                            .clone()
                            .unwrap_or("not_set".to_owned()),
                        cell: resolved_config_flag
                            .cell
                            .clone()
                            .map(|flag| flag.to_string()),
                        is_cli: true,
                    })
                }
            };
            kuro_data::BuckconfigComponent { data: Some(data) }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::resolve_config_flag_arg;

    #[test]
    fn test_argument_pair() -> kuro_error::Result<()> {
        // Valid Formats

        let normal_pair = resolve_config_flag_arg(None, "apple.key=value")?;

        assert_eq!("apple", normal_pair.section);
        assert_eq!("key", normal_pair.key);
        assert_eq!(Some("value".to_owned()), normal_pair.value);

        let unset_pair = resolve_config_flag_arg(None, "apple.key=")?;

        assert_eq!("apple", unset_pair.section);
        assert_eq!("key", unset_pair.key);
        assert_eq!(None, unset_pair.value);

        // Whitespace

        let section_leading_whitespace = resolve_config_flag_arg(None, "  apple.key=value")?;
        assert_eq!("apple", section_leading_whitespace.section);
        assert_eq!("key", section_leading_whitespace.key);
        assert_eq!(Some("value".to_owned()), section_leading_whitespace.value);

        let pair_with_whitespace_in_key = resolve_config_flag_arg(None, "apple. key=value");
        assert!(pair_with_whitespace_in_key.is_err());

        let pair_with_whitespace_in_value =
            resolve_config_flag_arg(None, "apple.key= value with whitespace  ")?;
        assert_eq!("apple", pair_with_whitespace_in_value.section);
        assert_eq!("key", pair_with_whitespace_in_value.key);
        assert_eq!(
            Some(" value with whitespace  ".to_owned()),
            pair_with_whitespace_in_value.value
        );

        // Invalid Formats

        let pair_without_section = resolve_config_flag_arg(None, "key=value");
        assert!(pair_without_section.is_err());

        let pair_without_equals = resolve_config_flag_arg(None, "apple.keyvalue");
        assert!(pair_without_equals.is_err());

        let pair_without_section_or_equals = resolve_config_flag_arg(None, "applekeyvalue");
        assert!(pair_without_section_or_equals.is_err());

        Ok(())
    }
}
