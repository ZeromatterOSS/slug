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
use slug_configuration_v2::SlugConfiguration;
pub use slug_configuration_v2::SlugConfigurationKind as ConfigurationKind;
pub use slug_configuration_v2::StarlarkOption;
pub use slug_configuration_v2::StarlarkOptionScope;
pub use slug_configuration_v2::StarlarkOptionValue;
use slug_configuration_v2::StarlarkOptions;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::serialization::StableSerialize;

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
    identity: ConfigurationIdentity,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
enum ConfigurationIdentity {
    /// Production identity: the complete typed structure participates in Eq
    /// and Hash. The projection is derived spelling only.
    Slug(SlugConfiguration),
    /// Bounded compatibility for existing unit fixtures. Production command
    /// roots must never construct this variant.
    Legacy {
        kind: ConfigurationKind,
        checksum: ConfigurationChecksum,
        starlark_options: StarlarkOptions,
    },
}

impl ConfigurationKey {
    pub fn new(kind: ConfigurationKind, checksum: ConfigurationChecksum) -> Self {
        Self {
            identity: ConfigurationIdentity::Legacy {
                kind,
                checksum,
                starlark_options: StarlarkOptions::default(),
            },
        }
    }

    pub fn from_slug(configuration: SlugConfiguration) -> Self {
        Self {
            identity: ConfigurationIdentity::Slug(configuration),
        }
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
        match &self.identity {
            ConfigurationIdentity::Slug(configuration) => configuration.kind(),
            ConfigurationIdentity::Legacy { kind, .. } => *kind,
        }
    }

    pub fn checksum(&self) -> Option<&ConfigurationChecksum> {
        match &self.identity {
            ConfigurationIdentity::Slug(_) => None,
            ConfigurationIdentity::Legacy { checksum, .. } => Some(checksum),
        }
    }

    pub fn slug_configuration(&self) -> Option<&SlugConfiguration> {
        match &self.identity {
            ConfigurationIdentity::Slug(configuration) => Some(configuration),
            ConfigurationIdentity::Legacy { .. } => None,
        }
    }

    pub fn with_starlark_option(&self, value: StarlarkOption) -> Self {
        match &self.identity {
            ConfigurationIdentity::Slug(configuration) => {
                Self::from_slug(configuration.with_starlark_option(value))
            }
            ConfigurationIdentity::Legacy {
                kind,
                checksum,
                starlark_options,
            } => Self {
                identity: ConfigurationIdentity::Legacy {
                    kind: *kind,
                    checksum: checksum.clone(),
                    starlark_options: starlark_options.with(value),
                },
            },
        }
    }

    pub fn starlark_option(&self, label: &CanonicalLabel) -> Option<&StarlarkOption> {
        match &self.identity {
            ConfigurationIdentity::Slug(configuration) => {
                configuration.starlark_options().get(label)
            }
            ConfigurationIdentity::Legacy {
                starlark_options, ..
            } => starlark_options.get(label),
        }
    }

    pub fn starlark_options(&self) -> &StarlarkOptions {
        match &self.identity {
            ConfigurationIdentity::Slug(configuration) => configuration.starlark_options(),
            ConfigurationIdentity::Legacy {
                starlark_options, ..
            } => starlark_options,
        }
    }

    pub fn stable_serialize(&self) -> String {
        match &self.identity {
            ConfigurationIdentity::Slug(configuration) => {
                format!("{}:{}", configuration.kind(), configuration.projection())
            }
            ConfigurationIdentity::Legacy { kind, checksum, .. } => {
                format!("{kind}:{checksum}")
            }
        }
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

/// Canonical configured-query identity. Configured targets carry structural
/// configuration; null nodes retain the root source and package-group forms
/// admitted by the delegating topology packet.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub enum ConfiguredNodeKey {
    Configured(ConfiguredTargetKey),
    Null(CanonicalLabel),
}

impl ConfiguredNodeKey {
    pub fn configured(target: ConfiguredTargetKey) -> Self {
        Self::Configured(target)
    }

    pub fn null(label: CanonicalLabel) -> Self {
        Self::Null(label)
    }

    pub fn configured_target(&self) -> Option<&ConfiguredTargetKey> {
        match self {
            Self::Configured(target) => Some(target),
            Self::Null(_) => None,
        }
    }

    pub fn label(&self) -> &CanonicalLabel {
        match self {
            Self::Configured(target) => target.label(),
            Self::Null(label) => label,
        }
    }
}

impl From<ConfiguredTargetKey> for ConfiguredNodeKey {
    fn from(value: ConfiguredTargetKey) -> Self {
        Self::configured(value)
    }
}

impl fmt::Display for ConfiguredNodeKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configured(target) => target.fmt(f),
            Self::Null(label) => write!(f, "{label} [null]"),
        }
    }
}
