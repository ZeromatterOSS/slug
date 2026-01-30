/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Aspect-related Starlark types for Bazel compatibility.
//!
//! This module provides the context and support types for aspect execution:
//!
//! - `AspectContext` - The context object (`ctx`) passed to aspect implementations
//! - `AspectRuleInfo` - Access to the underlying rule's information (`ctx.rule`)
//! - `AspectTargetProviders` - Wrapper for the target argument with `target[SomeInfo]` support

pub mod context;
pub mod rule_info;
pub mod target_providers;

pub use context::AspectContext;
pub use rule_info::AspectRuleInfo;
pub use target_providers::AspectTargetProviders;
