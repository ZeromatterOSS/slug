/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use crate::label::CanonicalLabel;
use crate::package::PackageIdentifier;
use crate::repo::CanonicalRepoName;

pub trait StableSerialize {
    fn stable_serialize(&self) -> String;
}

impl StableSerialize for CanonicalRepoName {
    fn stable_serialize(&self) -> String {
        self.to_string()
    }
}

impl StableSerialize for PackageIdentifier {
    fn stable_serialize(&self) -> String {
        self.to_string()
    }
}

impl StableSerialize for CanonicalLabel {
    fn stable_serialize(&self) -> String {
        match self.mapping_id() {
            Some(mapping_id) => format!("{}@mapping:{}", self, mapping_id.as_str()),
            None => self.to_string(),
        }
    }
}
