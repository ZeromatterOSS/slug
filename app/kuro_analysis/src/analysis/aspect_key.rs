/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the above-listed licenses.
 */

//! DICE keys for aspect computation (Phase 8c).
//!
//! AspectKey is the cache key for aspect computation results:
//! - Key: (target, aspect_name)
//! - Value: AspectValue (provider collection returned by the aspect)

use std::sync::Arc;

use allocative::Allocative;
use derive_more::Display;
use kuro_core::target::configured_target_label::ConfiguredTargetLabel;
use kuro_util::arc_str::ArcStr;
use dupe::Dupe;

use kuro_build_api::interpreter::rule_defs::provider::collection::FrozenProviderCollectionValue;

/// DICE key for caching aspect computation results (Phase 8c).
///
/// Key = (target, aspect_name) → Value = AspectValue (providers)
///
/// This key identifies a unique aspect computation:
/// - `target`: The configured target this aspect is applied to
/// - `aspect_name`: The name of the aspect (e.g., "my_aspect")
#[derive(Clone, Dupe, Display, Debug, Eq, Hash, PartialEq, Allocative)]
#[display("AspectKey({}, {})", target, aspect_name)]
pub struct AspectKey {
    pub target: ConfiguredTargetLabel,
    pub aspect_name: ArcStr,
}

impl AspectKey {
    pub fn new(target: ConfiguredTargetLabel, aspect_name: ArcStr) -> Self {
        Self {
            target,
            aspect_name,
        }
    }
}

/// Result of aspect computation (cached in DICE).
///
/// This contains the frozen provider collection returned by the aspect's
/// implementation function.
#[derive(Clone, Dupe, Debug, Allocative)]
pub struct AspectValue {
    pub providers: FrozenProviderCollectionValue,
}

impl AspectValue {
    // TODO(Phase 8c): Implement empty() once we have a way to create empty FrozenProviderCollection
    // For now, this is not used since aspect computation is stubbed out
}
