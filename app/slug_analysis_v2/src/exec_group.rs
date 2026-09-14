/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use slug_identity_v2::CanonicalLabel;

/// Semantic identity of an execution group shared by dependency transitions
/// and action ownership.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub enum ConfiguredExecGroup {
    Default,
    Named(CompactString),
    Automatic(Arc<CanonicalLabel>),
}

impl ConfiguredExecGroup {
    pub fn automatic(label: CanonicalLabel) -> Self {
        Self::Automatic(Arc::new(label))
    }
}
