/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Bazel-compatible config module stub.
//!
//! In Bazel, the `config` module provides configuration transitions like
//! config.exec() and config.target(). This is a stub to allow rules to load.

use std::fmt;
use std::fmt::Display;

use allocative::Allocative;
use starlark::environment::GlobalsBuilder;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::ProvidesStaticType;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::starlark_value;

/// A configuration transition reference.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ConfigTransition {
    kind: String,
    exec_group: Option<String>,
}

impl Display for ConfigTransition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.exec_group {
            Some(group) => write!(f, "config.{}(exec_group={})", self.kind, group),
            None => write!(f, "config.{}()", self.kind),
        }
    }
}

starlark_simple_value!(ConfigTransition);

#[starlark_value(type = "config_transition")]
impl<'v> StarlarkValue<'v> for ConfigTransition {}

/// A build setting descriptor returned by config.bool(), config.int(), config.string().
/// Used with `rule(build_setting = config.bool(flag = True))`.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ConfigBuildSetting {
    pub setting_type: String,
    pub flag: bool,
    pub allow_multiple: bool,
}

impl Display for ConfigBuildSetting {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "config.{}(flag={})", self.setting_type, self.flag)
    }
}

starlark_simple_value!(ConfigBuildSetting);

#[starlark_value(type = "config_build_setting")]
impl<'v> StarlarkValue<'v> for ConfigBuildSetting {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "allow_multiple" | "flag" | "setting_type")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "allow_multiple" => Some(Value::new_bool(self.allow_multiple)),
            "flag" => Some(Value::new_bool(self.flag)),
            "setting_type" => Some(heap.alloc(self.setting_type.as_str())),
            _ => None,
        }
    }
}

/// The config module for configuration transitions.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ConfigModule;

impl Display for ConfigModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "config")
    }
}

starlark_simple_value!(ConfigModule);

#[starlark_value(type = "config_module")]
impl<'v> StarlarkValue<'v> for ConfigModule {
    fn get_methods() -> Option<&'static starlark::environment::Methods> {
        static RES: starlark::environment::MethodsStatic =
            starlark::environment::MethodsStatic::new();
        RES.methods(config_module_methods)
    }
}

#[starlark_module]
fn config_module_methods(builder: &mut starlark::environment::MethodsBuilder) {
    /// Returns a transition to the exec configuration.
    /// Accepts None or a string for exec_group (rules_python passes None).
    fn exec<'v>(
        #[starlark(this)] _this: &ConfigModule,
        #[starlark(default = starlark::values::none::NoneType)] exec_group: Value<'v>,
    ) -> starlark::Result<ConfigTransition> {
        let group = exec_group
            .unpack_str()
            .map(|s| s.to_owned())
            .filter(|s| !s.is_empty());
        Ok(ConfigTransition {
            kind: "exec".to_owned(),
            exec_group: group,
        })
    }

    /// Returns a no-transition marker (identity transition).
    fn none(#[starlark(this)] _this: &ConfigModule) -> starlark::Result<ConfigTransition> {
        Ok(ConfigTransition {
            kind: "none".to_owned(),
            exec_group: None,
        })
    }

    /// Returns a transition to the target configuration.
    fn target(#[starlark(this)] _this: &ConfigModule) -> starlark::Result<ConfigTransition> {
        Ok(ConfigTransition {
            kind: "target".to_owned(),
            exec_group: None,
        })
    }

    /// Returns a boolean build setting descriptor.
    /// Used with `rule(build_setting = config.bool(flag = True))`.
    fn bool(
        #[starlark(this)] _this: &ConfigModule,
        #[starlark(require = named, default = false)] flag: bool,
        #[starlark(require = named, default = false)] allow_multiple: bool,
    ) -> starlark::Result<ConfigBuildSetting> {
        Ok(ConfigBuildSetting {
            setting_type: "bool".to_owned(),
            flag,
            allow_multiple,
        })
    }

    /// Returns an integer build setting descriptor.
    fn int(
        #[starlark(this)] _this: &ConfigModule,
        #[starlark(require = named, default = false)] flag: bool,
        #[starlark(require = named, default = false)] allow_multiple: bool,
    ) -> starlark::Result<ConfigBuildSetting> {
        Ok(ConfigBuildSetting {
            setting_type: "int".to_owned(),
            flag,
            allow_multiple,
        })
    }

    /// Returns a string build setting descriptor.
    fn string(
        #[starlark(this)] _this: &ConfigModule,
        #[starlark(require = named, default = false)] flag: bool,
        #[starlark(require = named, default = false)] allow_multiple: bool,
    ) -> starlark::Result<ConfigBuildSetting> {
        Ok(ConfigBuildSetting {
            setting_type: "string".to_owned(),
            flag,
            allow_multiple,
        })
    }

    /// Returns a string_list build setting descriptor.
    fn string_list(
        #[starlark(this)] _this: &ConfigModule,
        #[starlark(require = named, default = false)] flag: bool,
        #[starlark(require = named, default = false)] repeatable: bool,
        #[starlark(require = named, default = false)] allow_multiple: bool,
    ) -> starlark::Result<ConfigBuildSetting> {
        let _ = repeatable;
        Ok(ConfigBuildSetting {
            setting_type: "string_list".to_owned(),
            flag,
            allow_multiple: allow_multiple || repeatable,
        })
    }

    /// Creates a string set build setting (like string_list but with deduplication).
    fn string_set(
        #[starlark(this)] _this: &ConfigModule,
        #[starlark(require = named, default = false)] flag: bool,
        #[starlark(require = named, default = false)] repeatable: bool,
        #[starlark(require = named, default = false)] allow_multiple: bool,
    ) -> starlark::Result<ConfigBuildSetting> {
        let _ = repeatable;
        Ok(ConfigBuildSetting {
            setting_type: "string_set".to_owned(),
            flag,
            allow_multiple: allow_multiple || repeatable,
        })
    }
}

/// Register the config global.
#[starlark_module]
pub fn register_config(globals: &mut GlobalsBuilder) {
    /// The config module for configuration transitions.
    const config: ConfigModule = ConfigModule;
}
