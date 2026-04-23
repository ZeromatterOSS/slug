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
