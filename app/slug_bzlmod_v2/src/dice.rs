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

use crate::ModuleKey;
use crate::YankedVersionPolicy;

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum LockfileMode {
    Off,
    Update,
    Refresh,
    Error,
}

impl LockfileMode {
    pub fn from_bazel_flag_value(value: &str) -> Result<Self, String> {
        match value {
            "off" => Ok(Self::Off),
            "update" => Ok(Self::Update),
            "refresh" => Ok(Self::Refresh),
            "error" => Ok(Self::Error),
            other => Err(format!(
                "Not a valid Lockfile mode: '{other}' (should be off, update, refresh or error)"
            )),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Update => "update",
            Self::Refresh => "refresh",
            Self::Error => "error",
        }
    }
}

impl fmt::Display for LockfileMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BzlmodCommandPolicyKey {
    yanked_versions_policy: YankedVersionPolicy,
}

impl BzlmodCommandPolicyKey {
    pub fn new(yanked_versions_policy: YankedVersionPolicy) -> Self {
        Self {
            yanked_versions_policy,
        }
    }

    pub fn from_allow_yanked_versions_flag(value: Option<&str>) -> Result<Self, String> {
        Ok(Self::new(YankedVersionPolicy::from_flag_value(value)?))
    }

    pub fn yanked_versions_policy(&self) -> &YankedVersionPolicy {
        &self.yanked_versions_policy
    }

    pub fn stable_serialize(&self) -> String {
        serialize_yanked_policy(&self.yanked_versions_policy)
    }
}

impl fmt::Display for BzlmodCommandPolicyKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.stable_serialize())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BzlmodEnvironmentPolicyKey {
    yanked_versions_policy: YankedVersionPolicy,
}

impl BzlmodEnvironmentPolicyKey {
    pub fn new(yanked_versions_policy: YankedVersionPolicy) -> Self {
        Self {
            yanked_versions_policy,
        }
    }

    pub fn from_bzlmod_allow_yanked_versions(value: Option<&str>) -> Result<Self, String> {
        Ok(Self::new(YankedVersionPolicy::from_env_value(value)?))
    }

    pub fn yanked_versions_policy(&self) -> &YankedVersionPolicy {
        &self.yanked_versions_policy
    }

    pub fn stable_serialize(&self) -> String {
        serialize_yanked_policy(&self.yanked_versions_policy)
    }
}

impl fmt::Display for BzlmodEnvironmentPolicyKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.stable_serialize())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BzlmodDiceInputs {
    root_module_digest: String,
    included_module_digest: String,
    registry_policy_digest: String,
    lockfile_digest: String,
    lockfile_mode: LockfileMode,
    command_policy: BzlmodCommandPolicyKey,
    environment_policy: BzlmodEnvironmentPolicyKey,
}

impl BzlmodDiceInputs {
    pub fn new(
        root_module_digest: impl Into<String>,
        included_module_digest: impl Into<String>,
        registry_policy_digest: impl Into<String>,
        lockfile_digest: impl Into<String>,
        lockfile_mode: LockfileMode,
        command_policy: BzlmodCommandPolicyKey,
        environment_policy: BzlmodEnvironmentPolicyKey,
    ) -> Result<Self, String> {
        let inputs = Self {
            root_module_digest: root_module_digest.into(),
            included_module_digest: included_module_digest.into(),
            registry_policy_digest: registry_policy_digest.into(),
            lockfile_digest: lockfile_digest.into(),
            lockfile_mode,
            command_policy,
            environment_policy,
        };
        inputs.validate()?;
        Ok(inputs)
    }

    pub fn lockfile_mode(&self) -> &LockfileMode {
        &self.lockfile_mode
    }

    pub fn command_policy(&self) -> &BzlmodCommandPolicyKey {
        &self.command_policy
    }

    pub fn environment_policy(&self) -> &BzlmodEnvironmentPolicyKey {
        &self.environment_policy
    }

    pub fn effective_yanked_versions_policy(&self) -> YankedVersionPolicy {
        self.command_policy
            .yanked_versions_policy()
            .union(self.environment_policy.yanked_versions_policy())
    }

    pub fn stable_serialize(&self) -> String {
        format!(
            "root={};includes={};registries={};lockfile={};mode={};command={};env={}",
            self.root_module_digest,
            self.included_module_digest,
            self.registry_policy_digest,
            self.lockfile_digest,
            self.lockfile_mode,
            self.command_policy,
            self.environment_policy
        )
    }

    fn validate(&self) -> Result<(), String> {
        for (name, value) in [
            ("root_module_digest", &self.root_module_digest),
            ("included_module_digest", &self.included_module_digest),
            ("registry_policy_digest", &self.registry_policy_digest),
            ("lockfile_digest", &self.lockfile_digest),
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

impl fmt::Display for BzlmodDiceInputs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.stable_serialize())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ResolvedBzlmodGraphDiceKey {
    root_module: ModuleKey,
    inputs: BzlmodDiceInputs,
}

impl ResolvedBzlmodGraphDiceKey {
    pub fn new(root_module: ModuleKey, inputs: BzlmodDiceInputs) -> Self {
        Self {
            root_module,
            inputs,
        }
    }

    pub fn root_module(&self) -> &ModuleKey {
        &self.root_module
    }

    pub fn inputs(&self) -> &BzlmodDiceInputs {
        &self.inputs
    }

    pub fn stable_serialize(&self) -> String {
        format!("{} {{{}}}", self.root_module, self.inputs)
    }
}

impl fmt::Display for ResolvedBzlmodGraphDiceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.stable_serialize())
    }
}

fn serialize_yanked_policy(policy: &YankedVersionPolicy) -> String {
    match policy {
        YankedVersionPolicy::Reject => "allow_yanked=reject".to_owned(),
        YankedVersionPolicy::AllowAll => "allow_yanked=all".to_owned(),
        YankedVersionPolicy::AllowList(allowed) => {
            let entries = allowed
                .iter()
                .map(ModuleKey::to_string)
                .collect::<Vec<_>>()
                .join(",");
            format!("allow_yanked=[{entries}]")
        }
    }
}
