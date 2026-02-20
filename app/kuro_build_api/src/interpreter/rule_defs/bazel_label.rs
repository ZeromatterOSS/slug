/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Bazel-compatible Label type.
//!
//! Implements the `Label` type returned by `Label()` and `ctx.package_relative_label()`.
//! In Bazel, Label objects have `.name`, `.package`, `.workspace_name`, `.workspace_root`,
//! and `.relative()` attributes.

use std::fmt;

use allocative::Allocative;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::ProvidesStaticType;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark::values::starlark_value;

/// A Bazel-compatible Label value returned by `Label()` and `ctx.package_relative_label()`.
///
/// Has `.name`, `.package`, `.workspace_name`, `.workspace_root` attributes.
/// Display returns the full label string (no quotes), usable directly in format strings.
#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
pub struct BazelLabel {
    /// The full resolved label string (e.g., "@repo//pkg:target")
    full: String,
    /// The target name (e.g., "target")
    name: String,
    /// The package path (e.g., "pkg" or "pkg/sub")
    package: String,
    /// The workspace/repo name (e.g., "repo" or "")
    workspace_name: String,
}

impl fmt::Display for BazelLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.full)
    }
}

starlark::starlark_simple_value!(BazelLabel);

#[starlark_value(type = "Label")]
impl<'v> StarlarkValue<'v> for BazelLabel {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "name" | "package" | "workspace_name" | "workspace_root" | "relative"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "name" => Some(heap.alloc(self.name.as_str())),
            "package" => Some(heap.alloc(self.package.as_str())),
            "workspace_name" => Some(heap.alloc(self.workspace_name.as_str())),
            "workspace_root" => Some(heap.alloc("")),
            "relative" => {
                // Return the full label string as a fallback
                Some(heap.alloc(self.full.as_str()))
            }
            _ => None,
        }
    }

    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        if let Some(other_label) = other.downcast_ref::<BazelLabel>() {
            Ok(self.full == other_label.full)
        } else if let Some(s) = other.unpack_str() {
            Ok(self.full == s)
        } else {
            Ok(false)
        }
    }

    fn write_hash(
        &self,
        hasher: &mut starlark::collections::StarlarkHasher,
    ) -> starlark::Result<()> {
        use std::hash::Hash;
        self.full.hash(hasher);
        Ok(())
    }

    fn to_bool(&self) -> bool {
        true
    }
}

impl BazelLabel {
    /// Parse a fully-resolved label string like "@repo//pkg:target" into a BazelLabel.
    ///
    /// The label_str is assumed to be already resolved (absolute, not relative).
    pub fn parse(label_str: &str) -> Self {
        // Parse "@repo//pkg:target" format
        let (workspace, rest) = if let Some(stripped) = label_str.strip_prefix('@') {
            if let Some(idx) = stripped.find("//") {
                (stripped[..idx].to_owned(), &stripped[idx + 2..])
            } else {
                (stripped.to_owned(), "")
            }
        } else if let Some(stripped) = label_str.strip_prefix("//") {
            (String::new(), stripped)
        } else {
            (String::new(), label_str)
        };

        let (package, name) = if let Some(colon_idx) = rest.find(':') {
            (
                rest[..colon_idx].to_owned(),
                rest[colon_idx + 1..].to_owned(),
            )
        } else if rest.is_empty() {
            (String::new(), String::new())
        } else {
            // No colon - infer target name from last path component
            let last = rest.rsplit('/').next().unwrap_or(rest);
            (rest.to_owned(), last.to_owned())
        };

        BazelLabel {
            full: label_str.to_owned(),
            name,
            package,
            workspace_name: workspace,
        }
    }

    /// Get the full label string.
    pub fn full(&self) -> &str {
        &self.full
    }

    /// Get the target name component.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the package path component.
    pub fn package(&self) -> &str {
        &self.package
    }

    /// Get the workspace/repo name.
    pub fn workspace_name(&self) -> &str {
        &self.workspace_name
    }
}
