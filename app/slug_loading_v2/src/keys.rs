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
use std::path::PathBuf;

use allocative::Allocative;
use slug_identity_v2::ApparentLabel;
pub use slug_workspace_v2::WorkspaceDirectoryEntry;
pub use slug_workspace_v2::WorkspaceDirectoryEntryKind;
pub use slug_workspace_v2::WorkspaceDirectoryKey;
pub use slug_workspace_v2::WorkspaceDirectorySnapshot;
pub use slug_workspace_v2::WorkspaceDirectorySnapshotKey;
pub use slug_workspace_v2::WorkspaceDirectoryValue;
pub use slug_workspace_v2::WorkspaceFileKey;
pub use slug_workspace_v2::WorkspaceFileValue;
pub use slug_workspace_v2::WorkspaceSnapshot;
pub use slug_workspace_v2::WorkspaceSnapshotKey;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct BzlParseKey {
    pub workspace: std::path::PathBuf,
    pub path: std::path::PathBuf,
}

impl fmt::Display for BzlParseKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bzl-parse:{}", self.path.display())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct LoadLabelResolutionKey {
    pub workspace: std::path::PathBuf,
    pub requesting_package: std::path::PathBuf,
    pub load: String,
}

impl fmt::Display for LoadLabelResolutionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "bzl-load-resolution:{}:{}",
            self.requesting_package.display(),
            self.load
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct BzlModuleEvalKey {
    pub workspace: std::path::PathBuf,
    pub path: std::path::PathBuf,
}

impl fmt::Display for BzlModuleEvalKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bzl-module-eval:{}", self.path.display())
    }
}

/// Pre-Host local package adapter. Its value is a `LegacyLoadedPackage`, not a
/// complete repository-mapping-aware `LoadedPackage`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct PackageLoadKey {
    pub workspace: std::path::PathBuf,
    pub package: std::path::PathBuf,
}

impl fmt::Display for PackageLoadKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "package-load:{}", self.package.display())
    }
}

/// DICE identity for the direct-directory-derived contents of one root package.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct PackageListingKey {
    pub workspace: PathBuf,
    pub package: PathBuf,
}

impl fmt::Display for PackageListingKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "package-listing:{}", self.package.display())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BuildFileTargetStub {
    pub label: ApparentLabel,
}
