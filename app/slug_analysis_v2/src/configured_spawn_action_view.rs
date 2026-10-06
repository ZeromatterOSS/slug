/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_build_api_v2::SpawnSpec;

use crate::ConfiguredAction;
use crate::ConfiguredActionAspectProvenance;
use crate::ConfiguredActionOwnerContext;
use crate::key::ConfiguredTargetKey;

/// Borrowed ordinary Spawn metadata for query; no execution or staging authority.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ConfiguredSpawnActionView<'a>(&'a ConfiguredAction);

impl<'a> ConfiguredSpawnActionView<'a> {
    pub fn from_configured_action(action: &'a ConfiguredAction) -> Result<Self, &'static str> {
        let spawn = action
            .spawn_spec()
            .ok_or("configured Spawn view requires a typed ordinary Spawn")?;
        let context = action.context();
        if context.execution_platform().is_none() {
            return Err("configured Spawn action requires a selected execution platform");
        }
        if context.raw_platform_fact().is_none() || context.platform_fact().is_none() {
            return Err("configured Spawn action requires raw and effective platform facts");
        }
        if spawn.outputs().is_empty() {
            return Err("configured Spawn action requires at least one declared output");
        }
        if context.aspect() != ConfiguredActionAspectProvenance::Absent {
            return Err("configured Spawn query does not support aspect provenance");
        }
        Ok(Self(action))
    }

    pub fn action(&self) -> &'a ConfiguredAction {
        self.0
    }

    pub fn spec(&self) -> &'a SpawnSpec {
        self.0
            .spawn_spec()
            .expect("configured Spawn view validates the typed spec")
    }

    pub fn context(&self) -> &'a ConfiguredActionOwnerContext {
        self.0.context().as_ref()
    }

    pub fn owner(&self) -> &'a ConfiguredTargetKey {
        self.context().owner()
    }

    pub fn execution_platform(&self) -> &'a ConfiguredTargetKey {
        self.context()
            .execution_platform()
            .expect("configured Spawn view validates a selected platform")
    }
}
