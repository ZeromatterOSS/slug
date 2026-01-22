/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

#![feature(error_generic_member_access)]
#![cfg(test)]
#![allow(clippy::bool_assert_comparison)]

mod actions;
mod analysis;
mod artifact_groups;
mod attrs;
mod build;
mod interpreter;
mod nodes;

#[test]
fn init_late_bindings_for_test() {
    #[ctor::ctor]
    fn init() {
        kuro_action_impl::init_late_bindings();
        kuro_analysis::init_late_bindings();
        kuro_anon_target::init_late_bindings();
        kuro_configured::init_late_bindings();
        kuro_events::init_late_bindings();
        kuro_interpreter_for_build::init_late_bindings();
        kuro_build_api::init_late_bindings();
        kuro_transition::init_late_bindings();
    }
}
