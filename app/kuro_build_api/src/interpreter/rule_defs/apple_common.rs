/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Bazel-compatible apple_common module.
//!
//! This provides an implementation of Bazel's apple_common built-in module
//! that rules_cc and other rulesets require for Apple platform support.
//!
//! Reference: https://bazel.build/rules/lib/apple_common

use std::fmt;
use std::fmt::Display;

use allocative::Allocative;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::ProvidesStaticType;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::starlark_value;

// ============================================================================
// AppleCommonModule - The main apple_common namespace
// ============================================================================

/// The apple_common module provides Apple platform utilities.
///
/// This module is used by rulesets for Apple platform support (macOS, iOS, etc.).
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct AppleCommonModule;

impl Display for AppleCommonModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "apple_common")
    }
}

starlark_simple_value!(AppleCommonModule);

#[starlark_value(type = "apple_common")]
impl<'v> StarlarkValue<'v> for AppleCommonModule {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(apple_common_module_methods)
    }
}

// ============================================================================
// AppleToolchain - Returned by apple_toolchain()
// ============================================================================

/// Apple toolchain information.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct AppleToolchain;

impl Display for AppleToolchain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AppleToolchain")
    }
}

starlark_simple_value!(AppleToolchain);

#[starlark_value(type = "AppleToolchain")]
impl<'v> StarlarkValue<'v> for AppleToolchain {}

// ============================================================================
// Objc Provider
// ============================================================================

/// Objc provider type.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ObjcProvider;

impl Display for ObjcProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider Objc>")
    }
}

starlark_simple_value!(ObjcProvider);

#[starlark_value(type = "Objc")]
impl<'v> StarlarkValue<'v> for ObjcProvider {}

/// Methods on the apple_common module.
#[starlark_module]
fn apple_common_module_methods(builder: &mut MethodsBuilder) {
    /// Returns Apple toolchain information.
    fn apple_toolchain(
        #[starlark(this)] _this: &AppleCommonModule,
    ) -> starlark::Result<AppleToolchain> {
        Ok(AppleToolchain)
    }

    /// The Objc provider.
    #[starlark(attribute)]
    fn Objc(this: &AppleCommonModule) -> starlark::Result<ObjcProvider> {
        let _ = this;
        Ok(ObjcProvider)
    }

    /// Apple platform type enum.
    #[starlark(attribute)]
    fn platform_type<'v>(this: &AppleCommonModule, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let _ = this;
        // Return a struct with platform type constants
        Ok(heap.alloc(PlatformType))
    }

    /// XcodeVersionConfig provider.
    #[starlark(attribute)]
    fn XcodeVersionConfig(
        this: &AppleCommonModule,
    ) -> starlark::Result<XcodeVersionConfigProvider> {
        let _ = this;
        Ok(XcodeVersionConfigProvider)
    }

    /// AppleDynamicFramework provider.
    #[starlark(attribute)]
    fn AppleDynamicFramework(
        this: &AppleCommonModule,
    ) -> starlark::Result<AppleDynamicFrameworkProvider> {
        let _ = this;
        Ok(AppleDynamicFrameworkProvider)
    }
}

// ============================================================================
// PlatformType - Apple platform type enum
// ============================================================================

/// Apple platform type enum (ios, macos, tvos, watchos, etc.).
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct PlatformType;

impl Display for PlatformType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "apple_common.platform_type")
    }
}

starlark_simple_value!(PlatformType);

#[starlark_value(type = "apple_platform_type")]
impl<'v> StarlarkValue<'v> for PlatformType {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(platform_type_attrs)
    }
}

#[starlark_module]
fn platform_type_attrs(builder: &mut MethodsBuilder) {
    #[starlark(attribute)]
    fn ios(this: &PlatformType) -> starlark::Result<&'static str> {
        let _ = this;
        Ok("ios")
    }

    #[starlark(attribute)]
    fn macos(this: &PlatformType) -> starlark::Result<&'static str> {
        let _ = this;
        Ok("macos")
    }

    #[starlark(attribute)]
    fn tvos(this: &PlatformType) -> starlark::Result<&'static str> {
        let _ = this;
        Ok("tvos")
    }

    #[starlark(attribute)]
    fn watchos(this: &PlatformType) -> starlark::Result<&'static str> {
        let _ = this;
        Ok("watchos")
    }

    #[starlark(attribute)]
    fn catalyst(this: &PlatformType) -> starlark::Result<&'static str> {
        let _ = this;
        Ok("catalyst")
    }

    #[starlark(attribute)]
    fn visionos(this: &PlatformType) -> starlark::Result<&'static str> {
        let _ = this;
        Ok("visionos")
    }
}

// ============================================================================
// XcodeVersionConfigProvider
// ============================================================================

/// XcodeVersionConfig provider type.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct XcodeVersionConfigProvider;

impl Display for XcodeVersionConfigProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider XcodeVersionConfig>")
    }
}

starlark_simple_value!(XcodeVersionConfigProvider);

#[starlark_value(type = "XcodeVersionConfig")]
impl<'v> StarlarkValue<'v> for XcodeVersionConfigProvider {}

// ============================================================================
// AppleDynamicFrameworkProvider
// ============================================================================

/// AppleDynamicFramework provider type.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct AppleDynamicFrameworkProvider;

impl Display for AppleDynamicFrameworkProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider AppleDynamicFramework>")
    }
}

starlark_simple_value!(AppleDynamicFrameworkProvider);

#[starlark_value(type = "AppleDynamicFramework")]
impl<'v> StarlarkValue<'v> for AppleDynamicFrameworkProvider {}

// ============================================================================
// Registration
// ============================================================================

/// Register the apple_common global.
#[starlark_module]
pub fn register_apple_common(globals: &mut GlobalsBuilder) {
    /// The apple_common module for Apple platform utilities.
    const apple_common: AppleCommonModule = AppleCommonModule;
}
