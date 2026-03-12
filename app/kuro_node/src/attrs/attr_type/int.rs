/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use allocative::Allocative;
use pagable::Pagable;

#[derive(Debug, Eq, PartialEq, Hash, Pagable, Allocative, Clone)]
pub struct IntAttrType {
    /// If set, restricts the attribute to only accept these specific integer values.
    /// Used by Bazel's `attr.int(values=[...])` parameter.
    pub allowed_values: Option<Vec<i64>>,
}

impl IntAttrType {
    pub fn new() -> Self {
        Self {
            allowed_values: None,
        }
    }

    pub fn with_values(values: Vec<i64>) -> Self {
        if values.is_empty() {
            Self {
                allowed_values: None,
            }
        } else {
            Self {
                allowed_values: Some(values),
            }
        }
    }
}
