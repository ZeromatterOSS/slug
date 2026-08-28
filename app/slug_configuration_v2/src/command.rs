//! Immutable command inputs which can affect structural configuration.

use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;
use serde::Deserialize;
use serde::Serialize;

/// One command occurrence retained until contextual configuration preparation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CommandConfigurationOccurrence {
    Starlark {
        apparent_label: CompactString,
        raw_value: Option<CompactString>,
        negated: bool,
    },
    ExtraToolchains {
        raw_value: CompactString,
    },
    ExtraExecutionPlatforms {
        raw_value: CompactString,
    },
}

impl CommandConfigurationOccurrence {
    pub fn starlark(
        apparent_label: impl Into<CompactString>,
        raw_value: Option<impl Into<CompactString>>,
        negated: bool,
    ) -> Self {
        Self::Starlark {
            apparent_label: apparent_label.into(),
            raw_value: raw_value.map(Into::into),
            negated,
        }
    }

    pub fn extra_toolchains(raw_value: impl Into<CompactString>) -> Self {
        Self::ExtraToolchains {
            raw_value: raw_value.into(),
        }
    }

    pub fn extra_execution_platforms(raw_value: impl Into<CompactString>) -> Self {
        Self::ExtraExecutionPlatforms {
            raw_value: raw_value.into(),
        }
    }
}

/// Compact immutable occurrence sequence shared by all command transports.
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    Hash,
    Allocative,
    Dupe,
    Serialize,
    Deserialize
)]
#[serde(transparent)]
pub struct CommandConfigurationOverlay(Arc<[CommandConfigurationOccurrence]>);

impl CommandConfigurationOverlay {
    pub fn new(occurrences: impl Into<Arc<[CommandConfigurationOccurrence]>>) -> Self {
        Self(occurrences.into())
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &CommandConfigurationOccurrence> {
        self.0.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether two carriers share the same immutable occurrence allocation.
    pub fn shares_allocation_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    #[cfg(test)]
    pub fn as_ptr(&self) -> *const CommandConfigurationOccurrence {
        self.0.as_ptr()
    }
}

impl From<Vec<CommandConfigurationOccurrence>> for CommandConfigurationOverlay {
    fn from(value: Vec<CommandConfigurationOccurrence>) -> Self {
        if value.is_empty() {
            Self::default()
        } else {
            Self(Arc::from(value))
        }
    }
}
