/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Build settings carried by a `ConfigurationData`.
//!
//! A build setting is a target declared via `rule(build_setting=config.*)`.
//! Unlike constraints, build settings are mutable across transitions and can
//! be read at analysis time through `ctx.attr`, `ctx.var`, `ctx.fragments`,
//! and `select()`.

use allocative::Allocative;
use derive_more::Display;
use dupe::Dupe;
use pagable::Pagable;
use strong_hash::StrongHash;

use crate::cells;
use crate::target::label::label::TargetLabel;

/// Label that identifies a build-setting target.
#[derive(
    Clone, Dupe, Debug, Display, Hash, Eq, PartialEq, Ord, PartialOrd, Allocative, StrongHash,
    Pagable
)]
#[display("{}", _0)]
pub struct BuildSettingLabel(pub TargetLabel);

impl BuildSettingLabel {
    pub fn new(target: TargetLabel) -> Self {
        Self(target)
    }

    pub fn target(&self) -> &TargetLabel {
        &self.0
    }

    pub fn command_line_option_name(&self) -> Option<&str> {
        let pkg = self.0.pkg();
        if pkg.cell_name().as_str() == "slug_settings"
            && pkg.cell_relative_path().as_str() == "command_line_option"
        {
            Some(self.0.name().as_str())
        } else {
            None
        }
    }

    pub fn is_command_line_option(&self) -> bool {
        self.command_line_option_name().is_some()
    }

    /// Canonicalises a Bazel-style label string into a `BuildSettingLabel`.
    ///
    /// Transitions declare inputs/outputs as raw strings (`"//:my_flag"`,
    /// `"//command_line_option:compilation_mode"`, `"@bazel_tools//..."`).
    /// Slug's `TargetLabel` parser needs an explicit cell prefix, so
    /// unprefixed labels are routed through a synthetic `@slug_settings`
    /// cell. The synthetic cell is only a storage key — it is not resolved
    /// or analysed as a real target. Cell-aware parsing is a follow-up;
    /// see Plan 19.4.
    pub fn from_bazel_label(raw: &str) -> slug_error::Result<Self> {
        const SYNTHETIC_CELL: &str = "@slug_settings";

        let mut canon = if raw.starts_with('@') {
            raw.to_owned()
        } else if let Some(rest) = raw.strip_prefix("//") {
            format!("{SYNTHETIC_CELL}//{rest}")
        } else {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Input,
                "build-setting label must start with `@` or `//`: `{}`",
                raw
            ));
        };

        if let Some((prefix, rest)) = canon
            .strip_prefix("@@")
            .map(|rest| ("@@", rest))
            .or_else(|| canon.strip_prefix('@').map(|rest| ("@", rest)))
            && let Some((repo, package_and_target)) = rest.split_once("//")
            && let Some(canonical) = cells::resolve_dynamic_extension_cell_alias(repo)
        {
            // Bazel's `@@` marks a canonical repo in label syntax; Slug's
            // internal cell name is the repo name without that sigil.
            let slug_prefix = if prefix == "@@" { "@" } else { prefix };
            canon = format!("{slug_prefix}{canonical}//{package_and_target}");
        }

        let target = TargetLabel::testing_parse(&canon);
        Ok(BuildSettingLabel(target))
    }
}

#[cfg(test)]
mod tests {
    use super::BuildSettingLabel;

    #[test]
    fn build_setting_labels_resolve_dynamic_extension_aliases() {
        crate::cells::register_dynamic_extension_cell_alias(
            "rules_rs++rules_rust+rules_rust".to_owned(),
            "rules_rust+".to_owned(),
        );

        let label = BuildSettingLabel::from_bazel_label(
            "@@rules_rs++rules_rust+rules_rust//rust/private:bootstrap_setting",
        )
        .unwrap();

        assert_eq!(label.target().pkg().cell_name().as_str(), "rules_rust+");
        assert_eq!(label.target().name().as_str(), "bootstrap_setting");
    }
}

/// Typed value of a build setting.
///
/// `StringSet` stores its elements as a sorted, deduplicated `Vec<String>` so
/// the enum can derive serialization traits the `pagable` crate requires.
/// Construct via [`BuildSettingValue::string_set`] to enforce the invariant.
#[derive(
    Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Allocative, StrongHash, Pagable
)]
pub enum BuildSettingValue {
    Bool(bool),
    Int(i64),
    String(String),
    StringList(Vec<String>),
    StringSet(Vec<String>),
}

impl BuildSettingValue {
    /// Constructs a `StringSet` with the invariant enforced (sorted, deduped).
    pub fn string_set<I: IntoIterator<Item = String>>(items: I) -> Self {
        let mut v: Vec<String> = items.into_iter().collect();
        v.sort();
        v.dedup();
        BuildSettingValue::StringSet(v)
    }

    /// Returns the type name. Matches the `build_setting_type` string stored on
    /// `Rule` (produced by `config.bool()`, `config.int()`, etc.).
    pub fn type_name(&self) -> &'static str {
        match self {
            BuildSettingValue::Bool(_) => "bool",
            BuildSettingValue::Int(_) => "int",
            BuildSettingValue::String(_) => "string",
            BuildSettingValue::StringList(_) => "string_list",
            BuildSettingValue::StringSet(_) => "string_set",
        }
    }
}

impl std::fmt::Display for BuildSettingValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildSettingValue::Bool(v) => write!(f, "{v}"),
            BuildSettingValue::Int(v) => write!(f, "{v}"),
            BuildSettingValue::String(v) => write!(f, "{v}"),
            BuildSettingValue::StringList(xs) => {
                f.write_str("[")?;
                for (i, x) in xs.iter().enumerate() {
                    if i > 0 {
                        f.write_str(",")?;
                    }
                    f.write_str(x)?;
                }
                f.write_str("]")
            }
            BuildSettingValue::StringSet(xs) => {
                f.write_str("{")?;
                for (i, x) in xs.iter().enumerate() {
                    if i > 0 {
                        f.write_str(",")?;
                    }
                    f.write_str(x)?;
                }
                f.write_str("}")
            }
        }
    }
}
