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
}
