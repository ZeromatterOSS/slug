/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::future::Future;
use std::io::Write;
use std::pin::Pin;

use dice::DiceTransaction;
use slug_core::category::Category;
use slug_core::target::configured_target_label::ConfiguredTargetLabel;
use slug_util::late_binding::LateBinding;

/// Implementation of `audit dep-files`.
pub static AUDIT_DEP_FILES: LateBinding<
    for<'a> fn(
        ctx: &'a DiceTransaction,
        ConfiguredTargetLabel,
        Category,
        Option<String>,
        &'a mut (dyn Write + Send),
    ) -> Pin<Box<dyn Future<Output = slug_error::Result<()>> + Send + 'a>>,
> = LateBinding::new("AUDIT_DEP_FILES");
