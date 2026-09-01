/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use allocative::Allocative;
use compact_str::CompactString;
use starlark_map::small_set::SmallSet;

pub use crate::lockfile_v28::FactNumber;
pub use crate::lockfile_v28::FactValue;
pub use crate::lockfile_v28::Facts;

/// The root module repositories declared direct by an extension implementation.
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Default)]
pub enum ModuleExtensionRepositorySelection {
    #[default]
    Unspecified,
    All,
    Explicit(SmallSet<CompactString>),
}

impl ModuleExtensionRepositorySelection {
    pub fn explicit(values: impl IntoIterator<Item = CompactString>) -> Self {
        Self::Explicit(values.into_iter().collect())
    }

    pub fn contains(&self, value: &str) -> bool {
        matches!(self, Self::Explicit(values) if values.contains(value))
    }

    pub fn explicit_values(&self) -> Option<&SmallSet<CompactString>> {
        match self {
            Self::Explicit(values) => Some(values),
            Self::Unspecified | Self::All => None,
        }
    }
}

/// Heap-independent result of `module_ctx.extension_metadata`.
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Default)]
pub struct ModuleExtensionMetadata {
    root_module_direct_deps: ModuleExtensionRepositorySelection,
    root_module_direct_dev_deps: ModuleExtensionRepositorySelection,
    reproducible: bool,
    facts: Facts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative)]
pub struct RootModuleExtensionUsage {
    has_non_dev_dependency: bool,
    has_dev_dependency: bool,
}

impl RootModuleExtensionUsage {
    pub fn new(has_non_dev_dependency: bool, has_dev_dependency: bool) -> Self {
        Self {
            has_non_dev_dependency,
            has_dev_dependency,
        }
    }

    pub fn has_non_dev_dependency(self) -> bool {
        self.has_non_dev_dependency
    }

    pub fn has_dev_dependency(self) -> bool {
        self.has_dev_dependency
    }
}

impl ModuleExtensionMetadata {
    pub fn new(
        root_module_direct_deps: ModuleExtensionRepositorySelection,
        root_module_direct_dev_deps: ModuleExtensionRepositorySelection,
        reproducible: bool,
        facts: Facts,
    ) -> Self {
        Self {
            root_module_direct_deps,
            root_module_direct_dev_deps,
            reproducible,
            facts,
        }
    }

    pub fn root_module_direct_deps(&self) -> &ModuleExtensionRepositorySelection {
        &self.root_module_direct_deps
    }

    pub fn root_module_direct_dev_deps(&self) -> &ModuleExtensionRepositorySelection {
        &self.root_module_direct_dev_deps
    }

    pub fn reproducible(&self) -> bool {
        self.reproducible
    }

    pub fn facts(&self) -> &Facts {
        &self.facts
    }
}

#[cfg(test)]
mod tests {
    use starlark_map::sorted_map::SortedMap;

    use super::*;

    #[test]
    fn metadata_equality_is_set_like_and_retains_every_semantic_field() {
        let left = ModuleExtensionMetadata::new(
            ModuleExtensionRepositorySelection::explicit(["a".into(), "b".into()]),
            ModuleExtensionRepositorySelection::explicit([]),
            false,
            Facts::default(),
        );
        let reordered = ModuleExtensionMetadata::new(
            ModuleExtensionRepositorySelection::explicit(["b".into(), "a".into()]),
            ModuleExtensionRepositorySelection::explicit([]),
            false,
            Facts::default(),
        );
        assert_eq!(left, reordered);
        assert_ne!(
            left,
            ModuleExtensionMetadata::new(
                ModuleExtensionRepositorySelection::explicit(["a".into()]),
                ModuleExtensionRepositorySelection::explicit([]),
                false,
                Facts::default(),
            )
        );
        assert_ne!(
            left,
            ModuleExtensionMetadata::new(
                left.root_module_direct_deps.clone(),
                ModuleExtensionRepositorySelection::explicit(["dev".into()]),
                false,
                Facts::default(),
            )
        );
        assert_ne!(
            left,
            ModuleExtensionMetadata::new(
                left.root_module_direct_deps.clone(),
                left.root_module_direct_dev_deps.clone(),
                true,
                Facts::default(),
            )
        );
        assert_ne!(
            left,
            ModuleExtensionMetadata::new(
                left.root_module_direct_deps.clone(),
                left.root_module_direct_dev_deps.clone(),
                false,
                Facts::new(SortedMap::from_iter([(
                    "key".into(),
                    FactValue::String("value".into()),
                )])),
            )
        );
    }
}
