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
use std::sync::Arc;

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

    pub(crate) fn without_starlark_option(&self, label: &CanonicalLabel) -> Self {
        match &self.identity {
            ConfigurationIdentity::Slug(configuration) => {
                Self::from_slug(configuration.without_starlark_option(label))
            }
            ConfigurationIdentity::Legacy {
                kind,
                checksum,
                starlark_options,
            } => Self {
                identity: ConfigurationIdentity::Legacy {
                    kind: *kind,
                    checksum: checksum.clone(),
                    starlark_options: starlark_options.without(label),
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

    /// Complete collision-free configured-target value identity. This is not
    /// the display/checksum projection used by command and query rendering.
    #[rustfmt::skip]
    pub fn complete_identity_bytes(&self) -> Arc<[u8]> {
        let ConfigurationIdentity::Legacy { kind, checksum, starlark_options } = &self.identity else { return Arc::from(self.slug_configuration().unwrap().canonical_bytes()); };
        fn bytes(out: &mut Vec<u8>, value: &[u8]) { out.extend_from_slice(&(value.len() as u64).to_be_bytes()); out.extend_from_slice(value); }
        let mut out = b"slug-legacy-config\0\x01".to_vec();
        out.push(match kind { ConfigurationKind::Target => 0, ConfigurationKind::Exec => 1, ConfigurationKind::HostLike => 2 });
        bytes(&mut out, checksum.as_str().as_bytes());
        out.extend_from_slice(&(starlark_options.iter().len() as u64).to_be_bytes());
        for option in starlark_options.iter() {
            bytes(&mut out, option.label().stable_serialize().as_bytes());
            out.push(match option.scope() { StarlarkOptionScope::Default => 0, StarlarkOptionScope::Universal => 1, StarlarkOptionScope::Target => 2, StarlarkOptionScope::Project => 3 });
            match option.value() {
                StarlarkOptionValue::Integer(value) => { out.push(0); bytes(&mut out, &value.to_signed_bytes_be()); }
                StarlarkOptionValue::Boolean(value) => out.extend_from_slice(&[1, u8::from(*value)]),
                StarlarkOptionValue::String(value) => { out.push(2); bytes(&mut out, value.as_bytes()); }
                StarlarkOptionValue::StringList(values)
                | StarlarkOptionValue::StringSet(values) => {
                    out.push(if matches!(option.value(), StarlarkOptionValue::StringList(_)) { 3 } else { 4 });
                    out.extend_from_slice(&(values.len() as u64).to_be_bytes());
                    for value in values.iter() { bytes(&mut out, value.as_bytes()); }
                }
            }
        }
        out.into()
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
    should_apply_rule_transition: bool,
}

impl ConfiguredTargetKey {
    pub fn new(label: CanonicalLabel, configuration: ConfigurationKey) -> Self {
        Self {
            label,
            configuration,
            should_apply_rule_transition: true,
        }
    }

    pub(crate) fn without_rule_transition(
        label: CanonicalLabel,
        configuration: ConfigurationKey,
    ) -> Self {
        Self {
            label,
            configuration,
            should_apply_rule_transition: false,
        }
    }

    pub fn label(&self) -> &CanonicalLabel {
        &self.label
    }

    pub fn configuration(&self) -> &ConfigurationKey {
        &self.configuration
    }

    pub(crate) fn should_apply_rule_transition(&self) -> bool {
        self.should_apply_rule_transition
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

#[cfg(test)]
mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hash;
    use std::hash::Hasher;

    use super::*;

    #[test]
    fn rule_transition_control_is_identity_but_not_display() {
        let label = CanonicalLabel::parse("@@//pkg:target").unwrap();
        let configuration = ConfigurationKey::target("cfg").unwrap();
        let applying = ConfiguredTargetKey::new(label.clone(), configuration.clone());
        let skipped = ConfiguredTargetKey::without_rule_transition(label, configuration);
        assert!(applying.should_apply_rule_transition());
        assert!(!skipped.should_apply_rule_transition());
        assert_ne!(applying, skipped);
        assert_eq!(applying.stable_serialize(), skipped.stable_serialize());
        let hash = |key: &ConfiguredTargetKey| {
            let mut hasher = DefaultHasher::new();
            key.hash(&mut hasher);
            hasher.finish()
        };
        assert_ne!(hash(&applying), hash(&skipped));
        let retained_size = std::mem::size_of::<ConfiguredTargetKey>();
        let two_field_size = std::mem::size_of::<(CanonicalLabel, ConfigurationKey)>();
        assert!(
            retained_size <= two_field_size + std::mem::align_of::<ConfiguredTargetKey>(),
            "transition bit inflated key unexpectedly: {two_field_size} -> {retained_size}"
        );
    }
}
