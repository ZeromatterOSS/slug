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
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::starlark_module;
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
        // Bazel 9.0+ with bzlmod uses canonical label format with @@ prefix.
        // - "@@repo//pkg:target" for external repos
        // - "@@//pkg:target" for the root repo
        // This is critical for bazel_features is_bzlmod_enabled detection:
        //   str(Label("//:invalid")).startswith("@@") must be True
        if self.workspace_name.is_empty()
            || kuro_core::cells::is_root_cell_name(&self.workspace_name)
        {
            write!(f, "@@//{}:{}", self.package, self.name)
        } else {
            write!(
                f,
                "@@{}//{}:{}",
                self.workspace_name, self.package, self.name
            )
        }
    }
}

starlark::starlark_simple_value!(BazelLabel);

#[starlark_module]
fn bazel_label_methods(builder: &mut MethodsBuilder) {
    /// Resolve a label string relative to this label's repository and package.
    ///
    /// In Bazel: `Label("@repo//pkg:target").relative(":other")` → `@repo//pkg:other`
    fn relative<'v>(this: &BazelLabel, label: &str) -> starlark::Result<BazelLabel> {
        let resolved = resolve_relative_label(&this.workspace_name, &this.package, label);
        Ok(BazelLabel::parse(&resolved))
    }

    /// Returns a new Label in the same package with a different target name.
    ///
    /// In Bazel: `Label("//pkg:foo").same_package_label("bar")` → `Label("//pkg:bar")`
    fn same_package_label<'v>(
        this: &BazelLabel,
        target_name: &str,
    ) -> starlark::Result<BazelLabel> {
        let resolved = resolve_relative_label(&this.workspace_name, &this.package, target_name);
        Ok(BazelLabel::parse(&resolved))
    }
}

/// Resolve a label string relative to a given repository and package.
pub(crate) fn resolve_relative_label(workspace_name: &str, package: &str, label: &str) -> String {
    // Use @@ prefix for canonical labels (Bazel 9.0+ bzlmod format)
    if label.starts_with("@@") || label.starts_with('@') || label.starts_with("//") {
        // Absolute label - return as-is (possibly prepend repo if starts with //)
        if label.starts_with("//") && !workspace_name.is_empty() {
            // "//pkg:target" relative to "@repo" → "@@repo//pkg:target"
            format!("@@{}{}", workspace_name, label)
        } else {
            label.to_owned()
        }
    } else if let Some(target) = label.strip_prefix(':') {
        // ":target" → "@@repo//pkg:target"
        if workspace_name.is_empty() {
            format!("@@//{}:{}", package, target)
        } else {
            format!("@@{}//{}:{}", workspace_name, package, target)
        }
    } else {
        // Bare label - treat as target within same package
        if workspace_name.is_empty() {
            format!("@@//{}:{}", package, label)
        } else {
            format!("@@{}//{}:{}", workspace_name, package, label)
        }
    }
}

#[starlark_value(type = "Label")]
impl<'v> StarlarkValue<'v> for BazelLabel {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(bazel_label_methods)
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "name" | "package" | "workspace_name" | "workspace_root" | "repo_name"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "name" => Some(heap.alloc(self.name.as_str())),
            "package" => Some(heap.alloc(self.package.as_str())),
            "workspace_name" => Some(heap.alloc(self.workspace_name.as_str())),
            "workspace_root" => {
                // In Bazel, workspace_root is "" for the main repo and
                // "external/<repo_name>" for external repos.
                if self.workspace_name.is_empty()
                    || kuro_core::cells::is_root_cell_name(&self.workspace_name)
                {
                    Some(heap.alloc(""))
                } else {
                    Some(heap.alloc(format!("external/{}", self.workspace_name)))
                }
            }
            // repo_name is the modern Bazel equivalent of workspace_name
            "repo_name" => Some(heap.alloc(self.workspace_name.as_str())),
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
        // Parse "@@repo//pkg:target" or "@repo//pkg:target" format
        // Strip leading @@ or @ prefix to get the repo name
        let stripped = label_str
            .strip_prefix("@@")
            .or_else(|| label_str.strip_prefix('@'))
            .unwrap_or(label_str);

        let (workspace, rest) = if let Some(idx) = stripped.find("//") {
            (stripped[..idx].to_owned(), &stripped[idx + 2..])
        } else if stripped.starts_with("//") {
            (String::new(), &stripped[2..])
        } else if !stripped.is_empty() && !stripped.contains('/') && !stripped.contains(':') {
            (stripped.to_owned(), "")
        } else {
            (String::new(), stripped)
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

        // Store canonical form with @@ prefix (Bazel 9.0+ bzlmod format)
        let full = if workspace.is_empty() || kuro_core::cells::is_root_cell_name(&workspace) {
            format!("@@//{}:{}", package, name)
        } else {
            format!("@@{}//{}:{}", workspace, package, name)
        };

        BazelLabel {
            full,
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
