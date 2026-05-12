/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_util::late_binding::LateBinding;
use starlark::values::FrozenStringValue;
use starlark::values::FrozenValue;
use starlark_map::small_map::SmallMap;

/// `rule()`, `anon_rule()`, `bxl.anon_rule()` value `impl` field.
pub static FROZEN_RULE_GET_IMPL: LateBinding<fn(FrozenValue) -> slug_error::Result<FrozenValue>> =
    LateBinding::new("FROZEN_RULE_GET_IMPL");

pub static FROZEN_PROMISE_ARTIFACT_MAPPINGS_GET_IMPL: LateBinding<
    fn(FrozenValue) -> slug_error::Result<SmallMap<FrozenStringValue, FrozenValue>>,
> = LateBinding::new("FROZEN_PROMISE_ARTIFACT_MAPPINGS_GET_IMPL");

/// Get `rule(outputs={...})` patterns from a frozen rule callable.
/// Returns a list of (name, pattern) pairs, e.g. [("output", "%{name}.binpb")].
pub static FROZEN_RULE_GET_OUTPUTS: LateBinding<
    fn(FrozenValue) -> slug_error::Result<Vec<(String, String)>>,
> = LateBinding::new("FROZEN_RULE_GET_OUTPUTS");
