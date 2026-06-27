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

use slug_identity_v2::CanonicalLabel;

use crate::key::ConfigurationKey;
use crate::key::ConfiguredTargetKey;

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum TransitionKind {
    Target,
    Exec,
    HostLike,
    Custom { name: String },
}

impl TransitionKind {
    pub fn custom(name: impl Into<String>) -> Self {
        Self::Custom { name: name.into() }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Target => "target",
            Self::Exec => "exec",
            Self::HostLike => "host-like",
            Self::Custom { name } => name,
        }
    }
}

impl fmt::Display for TransitionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct TransitionEdge {
    kind: TransitionKind,
    output_configuration: ConfigurationKey,
}

impl TransitionEdge {
    pub fn new(kind: TransitionKind, output_configuration: ConfigurationKey) -> Self {
        Self {
            kind,
            output_configuration,
        }
    }

    pub fn kind(&self) -> &TransitionKind {
        &self.kind
    }

    pub fn output_configuration(&self) -> &ConfigurationKey {
        &self.output_configuration
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ConfiguredDependency {
    label: CanonicalLabel,
    transition: TransitionEdge,
}

impl ConfiguredDependency {
    pub fn new(label: CanonicalLabel, transition: TransitionEdge) -> Self {
        Self { label, transition }
    }

    pub fn label(&self) -> &CanonicalLabel {
        &self.label
    }

    pub fn transition(&self) -> &TransitionEdge {
        &self.transition
    }

    pub fn configured_key(&self) -> ConfiguredTargetKey {
        ConfiguredTargetKey::new(
            self.label.clone(),
            self.transition.output_configuration.clone(),
        )
    }
}
