/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_identity_v2::ApparentLabel;
use slug_identity_v2::PackageIdentifier;
use slug_identity_v2::RepositoryMappingId;

use crate::load_label::LoadLabel;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BzlParseKey {
    pub label: LoadLabel,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LoadLabelResolutionKey {
    pub requesting_package: PackageIdentifier,
    pub mapping_id: RepositoryMappingId,
    pub load: LoadLabel,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BzlModuleEvalKey {
    pub label: LoadLabel,
    pub mapping_id: RepositoryMappingId,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PackageLoadKey {
    pub package: PackageIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GlobExpansionKey {
    pub package: PackageIdentifier,
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BuildFileTargetStub {
    pub label: ApparentLabel,
}
