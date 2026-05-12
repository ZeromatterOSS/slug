/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_core::cells::CellAliasResolver;
use slug_core::cells::CellResolver;
use slug_core::cells::name::CellName;
use slug_core::package::PackageLabel;
use slug_core::pattern::pattern::ParsedPattern;
use slug_node::visibility::VisibilityPattern;
use slug_node::visibility::VisibilitySpecification;
use slug_node::visibility::VisibilityWithinViewBuilder;
use slug_node::visibility::WithinViewSpecification;
use starlark::environment::GlobalsBuilder;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::list_or_tuple::UnpackListOrTuple;
use starlark::values::none::NoneType;

use crate::interpreter::build_context::BuildContext;
use crate::super_package::eval_ctx::PackageFileVisibilityFields;

#[derive(Debug, slug_error::Error)]
#[slug(tag = Input)]
enum PackageFileError {
    #[error("`package()` function can be used at most once per `PACKAGE` file")]
    AtMostOnce,
}

/// Bazel-style visibility constants
const BAZEL_VISIBILITY_PUBLIC: &str = "//visibility:public";
const BAZEL_VISIBILITY_PRIVATE: &str = "//visibility:private";

fn parse_visibility(
    patterns: &[String],
    cell_name: CellName,
    cell_resolver: &CellResolver,
    cell_alias_resolver: &CellAliasResolver,
    current_package: Option<&PackageLabel>,
) -> slug_error::Result<VisibilitySpecification> {
    let mut builder = VisibilityWithinViewBuilder::with_capacity(patterns.len());
    for pattern in patterns {
        // Support both Slug-style ("PUBLIC") and Bazel-style ("//visibility:public")
        if pattern == VisibilityPattern::PUBLIC || pattern == BAZEL_VISIBILITY_PUBLIC {
            builder.add_public();
        } else if pattern == BAZEL_VISIBILITY_PRIVATE {
            // //visibility:private means no visibility - skip this entry
            continue;
        } else {
            // Normalize special Bazel visibility patterns before parsing:
            //   :__pkg__          -> //current/pkg: (matches all targets in current package)
            //   :__subpackages__  -> //current/pkg/... (current pkg + subpackages)
            //   //pkg:__pkg__     -> //pkg: (matches all targets in exact package)
            //   //pkg:__subpackages__ -> //pkg/... (recursive)
            //   :group_name       -> treat as public (package_group lookup not implemented)
            let normalized: Option<String> = if pattern == ":__pkg__" {
                // Relative pattern - requires current package context
                // :__pkg__ -> //pkg: (all targets in current package, same cell)
                current_package.map(|pkg| {
                    let cell_path = pkg.as_cell_path();
                    format!("//{}:", cell_path.path())
                })
            } else if pattern == ":__subpackages__" {
                // Relative pattern - requires current package context
                // :__subpackages__ -> //pkg/... (current pkg + subpackages, same cell)
                current_package.map(|pkg| {
                    let cell_path = pkg.as_cell_path();
                    let path = cell_path.path();
                    if path.is_empty() {
                        "//...".to_owned()
                    } else {
                        format!("//{}/...", path)
                    }
                })
            } else if pattern.starts_with(':') {
                // Relative package_group reference (":group_name") - treat as public
                // since we'd need to resolve the package_group target to check membership.
                builder.add_public();
                continue;
            } else if pattern.ends_with(":__pkg__") {
                // //some/pkg:__pkg__ -> //some/pkg: (all targets in that exact package)
                Some(pattern.trim_end_matches("__pkg__").to_owned())
            } else if pattern.ends_with(":__subpackages__") {
                // //some/pkg:__subpackages__ -> //some/pkg/... (recursive)
                let base = pattern.trim_end_matches(":__subpackages__");
                Some(format!("{}/...", base))
            } else {
                Some(pattern.clone())
            };

            let normalized = match normalized {
                Some(n) => n,
                None => {
                    // Relative pattern without package context - treat as public
                    builder.add_public();
                    continue;
                }
            };

            // Tolerate unresolvable visibility entries (e.g., unknown cell aliases,
            // package_group refs). Being more permissive is safe.
            match ParsedPattern::parse_precise(
                &normalized,
                cell_name,
                cell_resolver,
                cell_alias_resolver,
            ) {
                Ok(parsed) => {
                    builder.add(VisibilityPattern(parsed));
                }
                Err(_) => {
                    // Skip entries that can't be resolved
                    continue;
                }
            }
        }
    }
    Ok(builder.build_visibility())
}

fn parse_within_view(
    patterns: &[String],
    cell_name: CellName,
    cell_resolver: &CellResolver,
    cell_alias_resolver: &CellAliasResolver,
) -> slug_error::Result<WithinViewSpecification> {
    let mut builder = VisibilityWithinViewBuilder::with_capacity(patterns.len());
    for pattern in patterns {
        if pattern == VisibilityPattern::PUBLIC {
            builder.add_public();
        } else {
            builder.add(VisibilityPattern(ParsedPattern::parse_precise(
                pattern,
                cell_name,
                cell_resolver,
                cell_alias_resolver,
            )?));
        }
    }
    Ok(builder.build_within_view())
}

/// Globals for `PACKAGE` files and `bzl` files included from `PACKAGE` files.
#[starlark_module]
pub(crate) fn register_package_function(globals: &mut GlobalsBuilder) {
    /// Deprecated. Use `package(default_visibility=...)` instead.
    ///
    /// Sets the default visibility for all targets in the package. When called,
    /// all targets in the BUILD file will use this visibility unless they specify
    /// an explicit visibility attribute.
    ///
    /// Example:
    /// ```python
    /// package_default_visibility(["//visibility:public"])
    /// # or
    /// package_default_visibility(["//my_package:__subpackages__"])
    /// ```
    ///
    /// See: https://bazel.build/reference/be/functions#package_default_visibility
    fn package_default_visibility(
        #[starlark(require = pos)] visibility: UnpackListOrTuple<String>,
        eval: &mut Evaluator,
    ) -> starlark::Result<NoneType> {
        let build_context = BuildContext::from_context(eval)?;
        // Only valid in BUILD files, not PACKAGE files
        if build_context
            .additional
            .require_package_file("package_default_visibility")
            .is_ok()
        {
            // In PACKAGE files, this is a no-op (package() should be used instead)
            return Ok(NoneType);
        }
        if !visibility.items.is_empty() {
            let current_package = build_context
                .base_path()
                .ok()
                .and_then(|p| PackageLabel::from_cell_path(p.as_ref()).ok());
            let vis = parse_visibility(
                &visibility.items,
                build_context.cell_info().name().name(),
                build_context.cell_info().cell_resolver(),
                build_context.cell_info().cell_alias_resolver(),
                current_package.as_ref(),
            )?;
            if let Ok(internals) =
                crate::interpreter::module_internals::ModuleInternals::from_context(
                    eval,
                    "package_default_visibility",
                )
            {
                internals.set_build_file_default_visibility(vis);
            }
        }
        Ok(NoneType)
    }

    /// DO NOT USE THIS FUNCTION!
    ///
    /// It controls which test config to use in downstream systems. Mostly likely you don't want to specify it by yourself.
    fn test_config_unification_rollout(
        enabled: bool,
        eval: &mut Evaluator,
    ) -> starlark::Result<NoneType> {
        let build_context = BuildContext::from_context(eval)?;
        let package_file_eval_ctx = build_context.additional.require_package_file("package")?;
        *package_file_eval_ctx
            .test_config_unification_rollout
            .borrow_mut() = Some(enabled);
        Ok(NoneType)
    }

    /// Sets package-level attributes.
    ///
    /// In PACKAGE files (Buck2-style), this sets visibility and within_view for the package.
    /// In BUILD files (Bazel-style), this sets default_visibility and other package defaults.
    ///
    /// The function auto-detects the context:
    /// - If in a PACKAGE file: uses Buck2 semantics (inherit, visibility, within_view)
    /// - If in a BUILD file: uses Bazel semantics (default_visibility, etc.) as a no-op stub
    fn package(
        #[starlark(require=named, default=false)] inherit: bool,
        #[starlark(require=named, default=UnpackListOrTuple::default())]
        visibility: UnpackListOrTuple<String>,
        #[starlark(require=named, default=UnpackListOrTuple::default())]
        within_view: UnpackListOrTuple<String>,
        // Bazel-compatible parameters (used in BUILD files)
        #[starlark(require=named, default=UnpackListOrTuple::default())]
        default_visibility: UnpackListOrTuple<String>,
        #[starlark(require=named)] default_testonly: Option<bool>,
        #[starlark(require=named, default=UnpackListOrTuple::default())]
        default_deprecation: UnpackListOrTuple<String>,
        #[starlark(require=named, default=UnpackListOrTuple::default())]
        features: UnpackListOrTuple<String>,
        #[starlark(require=named, default=UnpackListOrTuple::default())]
        default_applicable_licenses: UnpackListOrTuple<String>,
        #[starlark(require=named, default=UnpackListOrTuple::default())]
        default_package_metadata: UnpackListOrTuple<String>,
        eval: &mut Evaluator,
    ) -> starlark::Result<NoneType> {
        let build_context = BuildContext::from_context(eval)?;

        // Try to get PACKAGE file context - if it fails, we're in a BUILD file
        match build_context.additional.require_package_file("package") {
            Ok(package_file_eval_ctx) => {
                // PACKAGE file context - use Buck2 semantics.
                // Support both Buck2-style `visibility=` and Bazel-style `default_visibility=`
                // as aliases for each other in PACKAGE files.
                let vis_items: Vec<String> = if !visibility.items.is_empty() {
                    visibility.items.clone()
                } else {
                    default_visibility.items.clone()
                };
                let current_package = build_context
                    .base_path()
                    .ok()
                    .and_then(|p| PackageLabel::from_cell_path(p.as_ref()).ok());
                let visibility = parse_visibility(
                    &vis_items,
                    build_context.cell_info().name().name(),
                    build_context.cell_info().cell_resolver(),
                    build_context.cell_info().cell_alias_resolver(),
                    current_package.as_ref(),
                )?;
                let within_view = parse_within_view(
                    &within_view.items,
                    build_context.cell_info().name().name(),
                    build_context.cell_info().cell_resolver(),
                    build_context.cell_info().cell_alias_resolver(),
                )?;

                match &mut *package_file_eval_ctx.visibility.borrow_mut() {
                    Some(_) => {
                        return Err(slug_error::Error::from(PackageFileError::AtMostOnce).into());
                    }
                    x => {
                        *x = Some(PackageFileVisibilityFields {
                            visibility,
                            within_view,
                            inherit,
                        })
                    }
                };
            }
            Err(_) => {
                // BUILD file context - use Bazel semantics
                // Set the default visibility if specified
                if !default_visibility.items.is_empty() {
                    let current_package = build_context
                        .base_path()
                        .ok()
                        .and_then(|p| PackageLabel::from_cell_path(p.as_ref()).ok());
                    let visibility = parse_visibility(
                        &default_visibility.items,
                        build_context.cell_info().name().name(),
                        build_context.cell_info().cell_resolver(),
                        build_context.cell_info().cell_alias_resolver(),
                        current_package.as_ref(),
                    )?;
                    // Get the ModuleInternals to set the BUILD file's default visibility
                    if let Ok(internals) =
                        crate::interpreter::module_internals::ModuleInternals::from_context(
                            eval, "package",
                        )
                    {
                        internals.set_build_file_default_visibility(visibility);
                    }
                }
                // Other parameters are currently no-ops (could be implemented later)
                let _ = (
                    default_testonly,
                    default_deprecation,
                    features,
                    default_applicable_licenses,
                    default_package_metadata,
                );
            }
        }

        Ok(NoneType)
    }
}
