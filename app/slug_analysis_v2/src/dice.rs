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

use crate::key::ConfiguredTargetKey;

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct AnalysisDiceInputs {
    command_line_digest: String,
    build_settings_digest: String,
    repository_mapping_digest: String,
    toolchain_policy_digest: String,
}

impl AnalysisDiceInputs {
    pub fn new(
        command_line_digest: impl Into<String>,
        build_settings_digest: impl Into<String>,
        repository_mapping_digest: impl Into<String>,
        toolchain_policy_digest: impl Into<String>,
    ) -> Result<Self, String> {
        let inputs = Self {
            command_line_digest: command_line_digest.into(),
            build_settings_digest: build_settings_digest.into(),
            repository_mapping_digest: repository_mapping_digest.into(),
            toolchain_policy_digest: toolchain_policy_digest.into(),
        };
        inputs.validate()?;
        Ok(inputs)
    }

    pub fn stable_serialize(&self) -> String {
        format!(
            "cmd={};settings={};repos={};toolchains={}",
            self.command_line_digest,
            self.build_settings_digest,
            self.repository_mapping_digest,
            self.toolchain_policy_digest
        )
    }

    fn validate(&self) -> Result<(), String> {
        for (name, value) in [
            ("command_line_digest", &self.command_line_digest),
            ("build_settings_digest", &self.build_settings_digest),
            ("repository_mapping_digest", &self.repository_mapping_digest),
            ("toolchain_policy_digest", &self.toolchain_policy_digest),
        ] {
            if value.is_empty() {
                return Err(format!("{name} must not be empty"));
            }
            if !value
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
            {
                return Err(format!("invalid {name}: {value}"));
            }
        }
        Ok(())
    }
}

impl fmt::Display for AnalysisDiceInputs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.stable_serialize())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ConfiguredTargetDiceKey {
    configured_target: ConfiguredTargetKey,
    inputs: AnalysisDiceInputs,
}

impl ConfiguredTargetDiceKey {
    pub fn new(configured_target: ConfiguredTargetKey, inputs: AnalysisDiceInputs) -> Self {
        Self {
            configured_target,
            inputs,
        }
    }

    pub fn configured_target(&self) -> &ConfiguredTargetKey {
        &self.configured_target
    }

    pub fn inputs(&self) -> &AnalysisDiceInputs {
        &self.inputs
    }

    pub fn stable_serialize(&self) -> String {
        format!(
            "{} {{{}}}",
            self.configured_target.stable_serialize(),
            self.inputs.stable_serialize()
        )
    }
}

impl fmt::Display for ConfiguredTargetDiceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.stable_serialize())
    }
}
