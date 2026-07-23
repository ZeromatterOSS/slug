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

use allocative::Allocative;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::serialization::StableSerialize;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub enum ConfigurationKind {
    Target,
    Exec,
    HostLike,
}

impl ConfigurationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Target => "target",
            Self::Exec => "exec",
            Self::HostLike => "host-like",
        }
    }
}

impl fmt::Display for ConfigurationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub struct ConfigurationChecksum(String);

impl ConfigurationChecksum {
    pub fn new(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        if value.is_empty() {
            return Err("configuration checksum must not be empty".to_owned());
        }
        if !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
        {
            return Err(format!("invalid configuration checksum: {value}"));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ConfigurationChecksum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub struct ConfigurationKey {
    kind: ConfigurationKind,
    checksum: ConfigurationChecksum,
}

impl ConfigurationKey {
    pub fn new(kind: ConfigurationKind, checksum: ConfigurationChecksum) -> Self {
        Self { kind, checksum }
    }

    pub fn target(checksum: impl Into<String>) -> Result<Self, String> {
        Ok(Self::new(
            ConfigurationKind::Target,
            ConfigurationChecksum::new(checksum)?,
        ))
    }

    pub fn exec(checksum: impl Into<String>) -> Result<Self, String> {
        Ok(Self::new(
            ConfigurationKind::Exec,
            ConfigurationChecksum::new(checksum)?,
        ))
    }

    pub fn host_like(checksum: impl Into<String>) -> Result<Self, String> {
        Ok(Self::new(
            ConfigurationKind::HostLike,
            ConfigurationChecksum::new(checksum)?,
        ))
    }

    pub fn kind(&self) -> ConfigurationKind {
        self.kind
    }

    pub fn checksum(&self) -> &ConfigurationChecksum {
        &self.checksum
    }

    pub fn stable_serialize(&self) -> String {
        format!("{}:{}", self.kind, self.checksum)
    }
}

impl fmt::Display for ConfigurationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.stable_serialize())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub struct ConfiguredTargetKey {
    label: CanonicalLabel,
    configuration: ConfigurationKey,
}

impl ConfiguredTargetKey {
    pub fn new(label: CanonicalLabel, configuration: ConfigurationKey) -> Self {
        Self {
            label,
            configuration,
        }
    }

    pub fn label(&self) -> &CanonicalLabel {
        &self.label
    }

    pub fn configuration(&self) -> &ConfigurationKey {
        &self.configuration
    }

    pub fn stable_serialize(&self) -> String {
        format!(
            "{} [{}]",
            self.label.stable_serialize(),
            self.configuration.stable_serialize()
        )
    }
}

impl fmt::Display for ConfiguredTargetKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.stable_serialize())
    }
}
