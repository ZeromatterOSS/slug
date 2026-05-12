/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_core::soft_error;
use starlark::eval::SoftErrorHandler;
pub struct SlugStarlarkSoftErrorHandler;

/// When starlark deprecates something, we propagate it to our `soft_error!` handler.
impl SoftErrorHandler for SlugStarlarkSoftErrorHandler {
    fn soft_error(&self, category: &str, error: starlark::Error) -> Result<(), starlark::Error> {
        let error = slug_error::Error::from(error);
        soft_error!(&format!("starlark_rust_{category}"), error, deprecation: true, quiet:true)?;
        Ok(())
    }
}
