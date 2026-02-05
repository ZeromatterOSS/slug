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
use starlark::values::NoSerialize;
use starlark::values::ProvidesStaticType;
use starlark::values::StarlarkValue;
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
    fn exec(
        #[starlark(this)] _this: &ConfigModule,
        #[starlark(require = named, default = "")] exec_group: &str,
    ) -> starlark::Result<ConfigTransition> {
        Ok(ConfigTransition {
            kind: "exec".to_owned(),
            exec_group: if exec_group.is_empty() {
                None
            } else {
                Some(exec_group.to_owned())
            },
        })
    }

    /// Returns a transition to the target configuration.
    fn target(#[starlark(this)] _this: &ConfigModule) -> starlark::Result<ConfigTransition> {
        Ok(ConfigTransition {
            kind: "target".to_owned(),
            exec_group: None,
        })
    }
}

/// Register the config global.
#[starlark_module]
pub fn register_config(globals: &mut GlobalsBuilder) {
    /// The config module for configuration transitions.
    const config: ConfigModule = ConfigModule;
}
