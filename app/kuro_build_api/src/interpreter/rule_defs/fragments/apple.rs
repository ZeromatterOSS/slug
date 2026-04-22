/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Apple configuration fragment.

use std::fmt;
use std::fmt::Display;

use allocative::Allocative;
use starlark::starlark_simple_value;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::ProvidesStaticType;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::starlark_value;

// ============================================================================
// AppleFragment - Apple configuration fragment
// ============================================================================

/// Stub for Apple platform objects returned by `ctx.fragments.apple.single_arch_platform`.
/// Provides `platform_type` and other attributes that cc_toolchain_config.bzl accesses.
#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ApplePlatformStub;

impl Display for ApplePlatformStub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<apple_platform>")
    }
}

starlark_simple_value!(ApplePlatformStub);

#[starlark_value(type = "apple_platform")]
impl<'v> StarlarkValue<'v> for ApplePlatformStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "platform_type" | "name_in_plist" | "is_device")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "platform_type" => Some(heap.alloc("macos")),
            "name_in_plist" => Some(heap.alloc("MacOSX")),
            "is_device" => Some(heap.alloc(false)),
            _ => None,
        }
    }
}

/// Apple configuration fragment stub.
///
/// Accessed via `ctx.fragments.apple`. Returns safe defaults for Apple platform settings.
#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
pub struct AppleFragment;

impl Display for AppleFragment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<apple fragment>")
    }
}

starlark_simple_value!(AppleFragment);

#[starlark_value(type = "apple_fragment")]
impl<'v> StarlarkValue<'v> for AppleFragment {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "single_arch_platform"
                | "single_arch_cpu"
                | "bitcode_mode"
                | "mandatory_minimum_version"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "single_arch_platform" => Some(heap.alloc(ApplePlatformStub)),
            "bitcode_mode" => Some(heap.alloc("none")),
            "single_arch_cpu" => {
                if cfg!(target_arch = "aarch64") {
                    Some(heap.alloc("arm64"))
                } else {
                    Some(heap.alloc("x86_64"))
                }
            }
            "mandatory_minimum_version" => Some(heap.alloc(false)),
            _ => None,
        }
    }
}
