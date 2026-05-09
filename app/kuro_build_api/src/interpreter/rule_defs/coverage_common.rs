/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Bazel-compatible coverage_common module.
//!
//! This provides an implementation of Bazel's coverage_common built-in module
//! for code coverage instrumentation support.
//!
//! Reference: https://bazel.build/rules/lib/toplevel/coverage_common

use std::fmt;
use std::fmt::Display;
use std::sync::Arc;
use std::sync::OnceLock;

use allocative::Allocative;
use kuro_core::provider::id::ProviderId;
use kuro_interpreter::types::provider::callable::ProviderCallableLike;
use starlark::coerce::Coerce;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Evaluator;
use starlark::starlark_complex_value;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::values::Demand;
use starlark::values::Freeze;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::ProvidesStaticType;
use starlark::values::StarlarkValue;
use starlark::values::Trace;
use starlark::values::Value;
use starlark::values::ValueLifetimeless;
use starlark::values::ValueLike;
use starlark::values::starlark_value;

use crate::interpreter::rule_defs::depset::depset_to_list;
use crate::interpreter::rule_defs::depset::make_depset_from_lists;
use crate::interpreter::rule_defs::provider::ProviderLike;

// ============================================================================
// CoverageCommonModule - The main coverage_common namespace
// ============================================================================

/// The coverage_common module provides code coverage utilities.
///
/// This module is used by rulesets to configure instrumentation for code coverage.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CoverageCommonModule;

impl Display for CoverageCommonModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "coverage_common")
    }
}

starlark_simple_value!(CoverageCommonModule);

#[starlark_value(type = "coverage_common")]
impl<'v> StarlarkValue<'v> for CoverageCommonModule {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(coverage_common_module_methods)
    }
}

// ============================================================================
// InstrumentedFilesInfo - Provider for instrumented files
// ============================================================================

/// InstrumentedFilesInfo provider callable.
///
/// This is the callable provider type used in `providers = [InstrumentedFilesInfo]`.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct InstrumentedFilesInfoProvider;

impl InstrumentedFilesInfoProvider {
    /// Get the static provider ID for InstrumentedFilesInfo.
    pub fn provider_id() -> &'static Arc<ProviderId> {
        static PROVIDER_ID: OnceLock<Arc<ProviderId>> = OnceLock::new();
        PROVIDER_ID.get_or_init(|| {
            Arc::new(ProviderId {
                path: None,
                name: "InstrumentedFilesInfo".to_owned(),
            })
        })
    }
}

impl Display for InstrumentedFilesInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider InstrumentedFilesInfo>")
    }
}

starlark_simple_value!(InstrumentedFilesInfoProvider);

impl ProviderCallableLike for InstrumentedFilesInfoProvider {
    fn id(&self) -> kuro_error::Result<&Arc<ProviderId>> {
        Ok(Self::provider_id())
    }
}

#[starlark_value(type = "InstrumentedFilesInfo")]
impl<'v> StarlarkValue<'v> for InstrumentedFilesInfoProvider {
    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderCallableLike>(self);
    }
}

/// InstrumentedFilesInfo provider instance.
///
/// This provider carries information about which files are instrumented for coverage.
/// Fields:
/// - `instrumented_files`: depset of Files that should be measured for coverage
/// - `metadata_files`: depset of Files containing coverage metadata (e.g. .gcno)
#[derive(
    Debug,
    ProvidesStaticType,
    NoSerialize,
    Allocative,
    Trace,
    Coerce,
    Freeze
)]
#[repr(C)]
pub struct InstrumentedFilesInfoInstanceGen<V: ValueLifetimeless> {
    instrumented_files: V,
    metadata_files: V,
}

starlark_complex_value!(pub InstrumentedFilesInfoInstance);

impl<V: ValueLifetimeless> Display for InstrumentedFilesInfoInstanceGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "InstrumentedFilesInfo()")
    }
}

impl<'v, V: ValueLike<'v>> ProviderLike<'v> for InstrumentedFilesInfoInstanceGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn id(&self) -> &Arc<ProviderId> {
        InstrumentedFilesInfoProvider::provider_id()
    }

    fn items(&self) -> Vec<(&str, Value<'v>)> {
        vec![
            ("instrumented_files", self.instrumented_files.to_value()),
            ("metadata_files", self.metadata_files.to_value()),
        ]
    }
}

#[starlark_value(type = "InstrumentedFilesInfoInstance")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for InstrumentedFilesInfoInstanceGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "instrumented_files" => Some(self.instrumented_files.to_value()),
            "metadata_files" => Some(self.metadata_files.to_value()),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        vec!["instrumented_files".to_owned(), "metadata_files".to_owned()]
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderLike>(self);
    }
}

/// Collect File objects from a ctx.attr.<name> value, filtering by extensions.
fn collect_files_from_attr<'v>(
    attr_value: Value<'v>,
    extensions: &[String],
    heap: Heap<'v>,
) -> Vec<Value<'v>> {
    let mut files = Vec::new();

    // Try to iterate (could be a list of targets, files, labels, etc.)
    if let Ok(iter) = attr_value.iterate(heap) {
        for item in iter {
            // Try to get files from a Target with .files attribute
            if let Ok(Some(files_attr)) = item.get_attr("files", heap) {
                if let Ok(file_iter) = files_attr.iterate(heap) {
                    for f in file_iter {
                        if matches_extensions(f, extensions) {
                            files.push(f);
                        }
                    }
                }
            } else if item.get_attr("path", heap).ok().flatten().is_some()
                || item.get_attr("short_path", heap).ok().flatten().is_some()
            {
                // This looks like a File object itself
                if matches_extensions(item, extensions) {
                    files.push(item);
                }
            }
        }
    } else if attr_value.get_attr("path", heap).ok().flatten().is_some() {
        // Single File object
        if matches_extensions(attr_value, extensions) {
            files.push(attr_value);
        }
    }

    files
}

/// Check if a file matches the given extensions filter.
fn matches_extensions<'v>(file: Value<'v>, extensions: &[String]) -> bool {
    if extensions.is_empty() {
        return true; // No filter means all files match
    }
    let name = file.to_str();
    for ext in extensions {
        if name.ends_with(&format!(".{}", ext)) {
            return true;
        }
    }
    false
}

/// Collect InstrumentedFilesInfo from dependency targets transitively.
fn collect_dep_coverage<'v>(
    attr_value: Value<'v>,
    heap: Heap<'v>,
) -> starlark::Result<(Vec<Value<'v>>, Vec<Value<'v>>)> {
    let mut instrumented = Vec::new();
    let mut metadata = Vec::new();

    if let Ok(iter) = attr_value.iterate(heap) {
        for dep in iter {
            // Try to get InstrumentedFilesInfo from the dep's providers
            if let Ok(Some(provider)) = dep.get_attr("InstrumentedFilesInfo", heap) {
                if let Ok(Some(inst_files)) = provider.get_attr("instrumented_files", heap) {
                    instrumented.extend(depset_to_list(inst_files, heap)?);
                }
                if let Ok(Some(meta_files)) = provider.get_attr("metadata_files", heap) {
                    metadata.extend(depset_to_list(meta_files, heap)?);
                }
            }
        }
    }

    Ok((instrumented, metadata))
}

/// Methods on the coverage_common module.
#[starlark_module]
fn coverage_common_module_methods(builder: &mut MethodsBuilder) {
    /// Creates an InstrumentedFilesInfo provider.
    ///
    /// Collects source files from the named source_attributes on ctx.attr,
    /// optionally filtering by file extensions. Transitively merges
    /// InstrumentedFilesInfo from dependency_attributes.
    #[allow(unused_variables)]
    fn instrumented_files_info<'v>(
        #[starlark(this)] _this: &CoverageCommonModule,
        ctx: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        source_attributes: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        dependency_attributes: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] extensions: Value<
            'v,
        >,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        metadata_files: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        coverage_support_files: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        coverage_environment: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        reported_to_actual_sources: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let heap = eval.heap();

        // Parse extensions filter
        let ext_filter: Vec<String> = if !extensions.is_none() {
            let mut exts = Vec::new();
            if let Ok(iter) = extensions.iterate(heap) {
                for ext in iter {
                    if let Some(s) = ext.unpack_str() {
                        exts.push(s.to_owned());
                    }
                }
            }
            exts
        } else {
            Vec::new() // No filter = all files
        };

        let mut all_instrumented_files: Vec<Value<'v>> = Vec::new();
        let mut all_metadata_files: Vec<Value<'v>> = Vec::new();

        // Collect source files from source_attributes on ctx.attr
        if !source_attributes.is_none() {
            if let Ok(Some(ctx_attr)) = ctx.get_attr("attr", heap) {
                if let Ok(iter) = source_attributes.iterate(heap) {
                    for attr_name in iter {
                        if let Some(name) = attr_name.unpack_str() {
                            if let Ok(Some(attr_val)) = ctx_attr.get_attr(name, heap) {
                                let files = collect_files_from_attr(attr_val, &ext_filter, heap);
                                all_instrumented_files.extend(files);
                            }
                        }
                    }
                }
            }
        }

        // Collect transitive InstrumentedFilesInfo from dependency_attributes
        if !dependency_attributes.is_none() {
            if let Ok(Some(ctx_attr)) = ctx.get_attr("attr", heap) {
                if let Ok(iter) = dependency_attributes.iterate(heap) {
                    for attr_name in iter {
                        if let Some(name) = attr_name.unpack_str() {
                            if let Ok(Some(attr_val)) = ctx_attr.get_attr(name, heap) {
                                let (inst, meta) = collect_dep_coverage(attr_val, heap)?;
                                all_instrumented_files.extend(inst);
                                all_metadata_files.extend(meta);
                            }
                        }
                    }
                }
            }
        }

        // Add explicit metadata_files
        if !metadata_files.is_none() {
            if let Ok(iter) = metadata_files.iterate(heap) {
                for f in iter {
                    all_metadata_files.push(f);
                }
            }
        }

        // Create depsets for the collected files
        let instrumented_depset =
            make_depset_from_lists(heap, all_instrumented_files, Vec::new(), "default")?;
        let metadata_depset =
            make_depset_from_lists(heap, all_metadata_files, Vec::new(), "default")?;

        Ok(heap.alloc(InstrumentedFilesInfoInstance {
            instrumented_files: instrumented_depset,
            metadata_files: metadata_depset,
        }))
    }
}

// ============================================================================
// Registration
// ============================================================================

/// Register the coverage_common global and InstrumentedFilesInfo provider.
#[starlark_module]
pub fn register_coverage_common(globals: &mut GlobalsBuilder) {
    /// The coverage_common module for code coverage utilities.
    const coverage_common: CoverageCommonModule = CoverageCommonModule;

    /// InstrumentedFilesInfo provider for code coverage instrumentation.
    const InstrumentedFilesInfo: InstrumentedFilesInfoProvider = InstrumentedFilesInfoProvider;
}
