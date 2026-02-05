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
use starlark::environment::GlobalsBuilder;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::values::Demand;
use starlark::values::NoSerialize;
use starlark::values::ProvidesStaticType;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::starlark_value;

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
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct InstrumentedFilesInfoInstance;

impl Display for InstrumentedFilesInfoInstance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "InstrumentedFilesInfo()")
    }
}

starlark_simple_value!(InstrumentedFilesInfoInstance);

impl<'v> ProviderLike<'v> for InstrumentedFilesInfoInstance {
    fn id(&self) -> &Arc<ProviderId> {
        InstrumentedFilesInfoProvider::provider_id()
    }

    fn items(&self) -> Vec<(&str, Value<'v>)> {
        // InstrumentedFilesInfo is a stub with no fields currently
        vec![]
    }
}

#[starlark_value(type = "InstrumentedFilesInfo")]
impl<'v> StarlarkValue<'v> for InstrumentedFilesInfoInstance {
    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderLike>(self);
    }
}

/// Methods on the coverage_common module.
#[starlark_module]
fn coverage_common_module_methods(builder: &mut MethodsBuilder) {
    /// Creates an InstrumentedFilesInfo provider.
    ///
    /// This is a stub - actual coverage support is not yet implemented.
    #[allow(unused_variables)]
    fn instrumented_files_info<'v>(
        #[starlark(this)] _this: &CoverageCommonModule,
        #[starlark(require = named)] ctx: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] source_attributes: Value<'v>,
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
    ) -> starlark::Result<InstrumentedFilesInfoInstance> {
        // TODO: Implement actual coverage instrumentation support
        Ok(InstrumentedFilesInfoInstance)
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
