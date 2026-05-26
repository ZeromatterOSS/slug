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
        Self::from_bazel_label_with_alias_resolver(raw, None)
    }

    pub fn from_bazel_label_with_alias_resolver(
        raw: &str,
        cell_alias_resolver: Option<&cells::CellAliasResolver>,
    ) -> slug_error::Result<Self> {
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
        {
            // Bazel's `@@` marks a canonical repo in label syntax; Slug's
            // internal cell name is the repo name without that sigil.
            let slug_prefix = if prefix == "@@" { "@" } else { prefix };
            let canonical = resolve_bzlmod_build_setting_repo(repo, cell_alias_resolver)
                .unwrap_or_else(|| repo.to_owned());
            if canonical != repo || prefix == "@@" {
                canon = format!("{slug_prefix}{canonical}//{package_and_target}");
            }
        }

        let target = TargetLabel::testing_parse(&canon);
        Ok(BuildSettingLabel(target))
    }
}

fn resolve_bzlmod_build_setting_repo(
    repo: &str,
    cell_alias_resolver: Option<&cells::CellAliasResolver>,
) -> Option<String> {
    cell_alias_resolver
        .and_then(|resolver| resolver.resolve_declared_or_runtime_alias(repo))
        .map(|cell| cell.as_str().to_owned())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use super::BuildSettingLabel;
    use crate::cells::BzlmodRuntimeCellInstallSnapshot;
    use crate::cells::BzlmodRuntimeDynamicAlias;
    use crate::cells::BzlmodRuntimeExtensionCell;
    use crate::cells::CellAliasResolver;
    use crate::cells::name::CellName;

    #[test]
    fn build_setting_labels_without_alias_owner_ignore_dynamic_extension_aliases() {
        let _guard = crate::cells::BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK
            .lock()
            .unwrap();
        let tmp = tempfile::tempdir().unwrap();
        crate::cells::reset_dynamic_bzlmod_state_for_project_root(tmp.path().to_path_buf());
        let apparent = "rules_rs++rules_rust+rules_rust";
        let wrong_global = "rules_rust+";
        crate::cells::register_dynamic_extension_cell_alias(
            apparent.to_owned(),
            wrong_global.to_owned(),
        );

        let label = BuildSettingLabel::from_bazel_label(
            "@@rules_rs++rules_rust+rules_rust//rust/private:bootstrap_setting",
        )
        .unwrap();

        assert_eq!(label.target().pkg().cell_name().as_str(), apparent);
        assert_eq!(label.target().name().as_str(), "bootstrap_setting");
        assert_eq!(
            crate::cells::resolve_dynamic_extension_cell_alias(apparent).as_deref(),
            Some(wrong_global)
        );
    }

    #[test]
    fn build_setting_labels_prefer_runtime_aliases_before_globals() -> slug_error::Result<()> {
        let _guard = crate::cells::BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK
            .lock()
            .unwrap();
        let tmp = tempfile::tempdir()?;
        crate::cells::reset_dynamic_bzlmod_state_for_project_root(tmp.path().to_path_buf());
        let apparent = "plan61_build_setting_runtime_alias";
        let canonical = "plan61_owner++settings+generated";
        let wrong_global = "plan61_wrong_owner++settings+generated";
        let snapshot = BzlmodRuntimeCellInstallSnapshot {
            extension_cells: Vec::new(),
            scoped_aliases: Vec::new(),
            dynamic_aliases: vec![BzlmodRuntimeDynamicAlias {
                apparent_name: apparent.to_owned(),
                canonical_name: canonical.to_owned(),
            }],
        };
        crate::cells::register_dynamic_extension_cell_alias(
            apparent.to_owned(),
            wrong_global.to_owned(),
        );
        let resolver = CellAliasResolver::new_bzlmod_with_runtime_cell_snapshot(
            CellName::testing_new("root"),
            HashMap::new(),
            &snapshot,
        )?;

        let label = BuildSettingLabel::from_bazel_label_with_alias_resolver(
            &format!("@@{apparent}//pkg:flag"),
            Some(&resolver),
        )?;

        assert_eq!(label.target().pkg().cell_name().as_str(), canonical);
        assert_eq!(
            crate::cells::resolve_dynamic_extension_cell_alias(apparent).as_deref(),
            Some(wrong_global)
        );
        Ok(())
    }

    #[test]
    fn build_setting_labels_runtime_miss_ignores_global_alias() -> slug_error::Result<()> {
        let _guard = crate::cells::BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK
            .lock()
            .unwrap();
        let tmp = tempfile::tempdir()?;
        crate::cells::reset_dynamic_bzlmod_state_for_project_root(tmp.path().to_path_buf());
        let apparent = "plan61_build_setting_runtime_miss";
        let wrong_global = "plan61_wrong_owner++settings+generated";
        let snapshot = BzlmodRuntimeCellInstallSnapshot::default();
        crate::cells::register_dynamic_extension_cell_alias(
            apparent.to_owned(),
            wrong_global.to_owned(),
        );
        let resolver = CellAliasResolver::new_bzlmod_with_runtime_cell_snapshot(
            CellName::testing_new("root"),
            HashMap::new(),
            &snapshot,
        )?;

        let label = BuildSettingLabel::from_bazel_label_with_alias_resolver(
            &format!("@@{apparent}//pkg:flag"),
            Some(&resolver),
        )?;

        assert_eq!(label.target().pkg().cell_name().as_str(), apparent);
        assert_eq!(
            crate::cells::resolve_dynamic_extension_cell_alias(apparent).as_deref(),
            Some(wrong_global)
        );
        Ok(())
    }

    #[test]
    fn build_setting_labels_no_snapshot_resolver_miss_ignores_global_alias()
    -> slug_error::Result<()> {
        let _guard = crate::cells::BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK
            .lock()
            .unwrap();
        let tmp = tempfile::tempdir()?;
        crate::cells::reset_dynamic_bzlmod_state_for_project_root(tmp.path().to_path_buf());
        let apparent = "plan61_build_setting_no_snapshot_miss";
        let wrong_global = "plan61_wrong_owner++settings+no_snapshot";
        crate::cells::register_dynamic_extension_cell_alias(
            apparent.to_owned(),
            wrong_global.to_owned(),
        );
        let resolver = CellAliasResolver::new(CellName::testing_new("root"), HashMap::new())?;

        let label = BuildSettingLabel::from_bazel_label_with_alias_resolver(
            &format!("@@{apparent}//pkg:flag"),
            Some(&resolver),
        )?;

        assert_eq!(label.target().pkg().cell_name().as_str(), apparent);
        assert_eq!(
            crate::cells::resolve_dynamic_extension_cell_alias(apparent).as_deref(),
            Some(wrong_global)
        );
        Ok(())
    }

    #[test]
    fn build_setting_labels_strip_canonical_sigil_for_runtime_owned_repo() -> slug_error::Result<()>
    {
        let _guard = crate::cells::BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK
            .lock()
            .unwrap();
        let tmp = tempfile::tempdir()?;
        crate::cells::reset_dynamic_bzlmod_state_for_project_root(tmp.path().to_path_buf());
        let canonical = "plan61_owner++settings+canonical";
        let wrong_global = "plan61_wrong_owner++settings+canonical";
        let setup = crate::cells::external::ExtensionRepoCellSetup {
            canonical_name: Arc::from(canonical),
            extension_id: Arc::from("@plan61_owner//:settings.bzl%settings"),
            internal_name: Arc::from("canonical"),
            spec_hash: Arc::from("sha256-plan61-build-setting-canonical"),
            repo_spec_json: Arc::from("{}"),
            repo_env_json: Arc::from("{}"),
            materialized: false,
        };
        let snapshot = BzlmodRuntimeCellInstallSnapshot {
            extension_cells: vec![BzlmodRuntimeExtensionCell {
                canonical_name: canonical.to_owned(),
                internal_name: "canonical".to_owned(),
                path: format!("bazel-external/{canonical}"),
                setup,
            }],
            scoped_aliases: Vec::new(),
            dynamic_aliases: Vec::new(),
        };
        crate::cells::register_dynamic_extension_cell_alias(
            canonical.to_owned(),
            wrong_global.to_owned(),
        );
        let resolver = CellAliasResolver::new_bzlmod_with_runtime_cell_snapshot(
            CellName::testing_new("root"),
            HashMap::new(),
            &snapshot,
        )?;

        let label = BuildSettingLabel::from_bazel_label_with_alias_resolver(
            &format!("@@{canonical}//pkg:flag"),
            Some(&resolver),
        )?;

        assert_eq!(label.target().pkg().cell_name().as_str(), canonical);
        assert_eq!(
            crate::cells::resolve_dynamic_extension_cell_alias(canonical).as_deref(),
            Some(wrong_global)
        );
        Ok(())
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
