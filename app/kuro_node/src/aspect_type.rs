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
use derive_more::Display;
use pagable::Pagable;
use strong_hash::StrongHash;

use crate::bzl_or_bxl_path::BzlOrBxlPath;

/// Identifies an aspect by its defining module and exported name.
/// Analogous to StarlarkRuleType for rules.
///
/// This enables DICE-based module loading for aspects, following the same
/// pattern as rule loading. The module path allows AspectKey::compute() to
/// load the aspect callable from its defining .bzl file.
#[derive(
    Clone,
    Debug,
    Display,
    Eq,
    Hash,
    PartialEq,
    StrongHash,
    Pagable,
    Allocative
)]
#[display("{}:{}", path, name)]
pub struct StarlarkAspectType {
    /// The .bzl or .bxl file that defines this aspect
    pub path: BzlOrBxlPath,
    /// The exported symbol name (e.g., "my_aspect")
    pub name: String,
}

impl StarlarkAspectType {
    pub fn new(path: BzlOrBxlPath, name: String) -> Self {
        Self { path, name }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::sync::Arc;

    use kuro_core::bzl::ImportPath;

    use crate::aspect_type::StarlarkAspectType;
    use crate::bzl_or_bxl_path::BzlOrBxlPath;

    #[test]
    fn aspect_type_has_useful_string() {
        let import_path = ImportPath::testing_new("root//some/subdir:aspects.bzl");
        let name = "my_aspect".to_owned();

        assert_eq!(
            "root//some/subdir/aspects.bzl:my_aspect",
            &StarlarkAspectType {
                path: BzlOrBxlPath::Bzl(import_path),
                name
            }
            .to_string()
        );
    }

    #[test]
    fn aspect_type_equality() {
        let path1 = ImportPath::testing_new("root//pkg:aspects.bzl");
        let path2 = ImportPath::testing_new("root//pkg:aspects.bzl");
        let path3 = ImportPath::testing_new("root//other:aspects.bzl");

        let type1 = StarlarkAspectType::new(BzlOrBxlPath::Bzl(path1), "my_aspect".to_owned());
        let type2 = StarlarkAspectType::new(BzlOrBxlPath::Bzl(path2), "my_aspect".to_owned());
        let type3 = StarlarkAspectType::new(BzlOrBxlPath::Bzl(path3), "my_aspect".to_owned());

        assert_eq!(type1, type2); // Same path and name
        assert_ne!(type1, type3); // Different path
    }

    #[test]
    fn aspect_type_hash_consistency() {
        let path = ImportPath::testing_new("root//pkg:aspects.bzl");
        let type1 =
            StarlarkAspectType::new(BzlOrBxlPath::Bzl(path.clone()), "my_aspect".to_owned());
        let type2 = StarlarkAspectType::new(BzlOrBxlPath::Bzl(path), "my_aspect".to_owned());

        let mut hasher1 = DefaultHasher::new();
        let mut hasher2 = DefaultHasher::new();
        type1.hash(&mut hasher1);
        type2.hash(&mut hasher2);

        assert_eq!(hasher1.finish(), hasher2.finish()); // Equal types have equal hashes
    }

    #[test]
    fn aspect_type_arc_wrapped() {
        // Verify Arc wrapping works correctly for DICE key usage
        let path = ImportPath::testing_new("root//pkg:aspects.bzl");
        let aspect_type = Arc::new(StarlarkAspectType::new(
            BzlOrBxlPath::Bzl(path),
            "my_aspect".to_owned(),
        ));

        let cloned = aspect_type.clone();
        assert_eq!(aspect_type.name, cloned.name);
        assert_eq!(Arc::strong_count(&aspect_type), 2);
    }

    #[test]
    fn aspect_type_different_names_not_equal() {
        let path = ImportPath::testing_new("root//pkg:aspects.bzl");
        let type1 = StarlarkAspectType::new(BzlOrBxlPath::Bzl(path.clone()), "aspect1".to_owned());
        let type2 = StarlarkAspectType::new(BzlOrBxlPath::Bzl(path), "aspect2".to_owned());

        assert_ne!(type1, type2); // Different names
    }
}
