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
use starlark::environment::GlobalsBuilder;

/// Globals defined in `slug_build_api`.
pub static REGISTER_BUCK2_BUILD_API_GLOBALS: LateBinding<fn(&mut GlobalsBuilder)> =
    LateBinding::new("REGISTER_BUCK2_BUILD_API_GLOBALS");

/// `__internal__`s defined in `slug_build_api`.
pub static REGISTER_BUCK2_BUILD_API_INTERNALS: LateBinding<fn(&mut GlobalsBuilder)> =
    LateBinding::new("REGISTER_BUCK2_BUILD_API_INTERNALS");

/// Globals defined in `slug_transitions` crate.
pub static REGISTER_BUCK2_TRANSITION_GLOBALS: LateBinding<fn(&mut GlobalsBuilder)> =
    LateBinding::new("REGISTER_BUCK2_TRANSITION_GLOBALS");

/// Globals defined in `slug_action_impl` crate.
pub static REGISTER_BUCK2_ACTION_IMPL_GLOBALS: LateBinding<fn(&mut GlobalsBuilder)> =
    LateBinding::new("REGISTER_BUCK2_ACTION_IMPL_GLOBALS");

/// Globals defined in `slug_anon_targets` crate.
pub static REGISTER_BUCK2_ANON_TARGETS_GLOBALS: LateBinding<fn(&mut GlobalsBuilder)> =
    LateBinding::new("REGISTER_BUCK2_ANON_TARGETS_GLOBALS");

/// Globals defined in `slug_bxl` crate,
/// which are used to create the context for `.bxl` evaluation.
pub static REGISTER_BUCK2_BXL_GLOBALS: LateBinding<fn(&mut GlobalsBuilder)> =
    LateBinding::new("REGISTER_BUCK2_BXL_GLOBALS");

/// Globals defined in `slug_cfg_constructor` crate.
pub static REGISTER_BUCK2_CFG_CONSTRUCTOR_GLOBALS: LateBinding<fn(&mut GlobalsBuilder)> =
    LateBinding::new("REGISTER_BUCK2_CFG_CONSTRUCTOR_GLOBALS");
