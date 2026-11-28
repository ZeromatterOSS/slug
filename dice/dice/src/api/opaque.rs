/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fmt;
use std::fmt::Debug;
use std::fmt::Formatter;

use crate::api::key::Key;
use crate::impls::opaque::OpaqueValueModern;

/// Computed value whose dependency is recorded when it is consumed.
///
/// A projection records its own dependency. Direct consumption records the underlying key.
///
/// Consume it only in computations that share the live context that produced it.
/// Separate transactions sharing that cache may use the same opaque value.
pub struct OpaqueValue<K: Key> {
    pub(crate) implementation: OpaqueValueModern<K>,
}

impl<K> Debug for OpaqueValue<K>
where
    K: Key,
    K::Value: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.implementation.fmt(f)
    }
}

impl<K: Key> OpaqueValue<K> {
    pub(crate) fn new(implementation: OpaqueValueModern<K>) -> Self {
        Self { implementation }
    }
}
