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
use dupe::Dupe;

use crate::impls::core::versions::VersionEpoch;
use crate::versions::VersionNumber;

/// Opaque process-local identity of one live cache in a DICE engine.
///
/// Transactions sharing a live cache have equal identities. Reopening an idle
/// version creates a different identity. This token owns no cache or transaction
/// and is neither semantic equality nor a persistent content identity.
#[derive(Copy, Clone, Dupe, Debug, Eq, PartialEq, Hash, Allocative)]
pub struct DiceLiveContextId {
    engine: u64,
    version: VersionNumber,
    epoch: VersionEpoch,
}

impl DiceLiveContextId {
    pub(crate) fn new(engine: u64, version: VersionNumber, epoch: VersionEpoch) -> Self {
        Self {
            engine,
            version,
            epoch,
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod cancellation_tests;
