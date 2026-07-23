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
use std::sync::Arc;

use allocative::Allocative;
use dice::InjectedKey;
use dupe::Dupe;
use slug_identity_v2::ApparentLabel;
use slug_identity_v2::PackageIdentifier;
use starlark_map::sorted_map::SortedMap;

/// An observation supplied to the workspace DICE graph before loading begins.
///
/// The value deliberately distinguishes a file that does not exist from a
/// read failure.  A missing BUILD file is meaningful to package loading, while
/// a permission or I/O failure must be reported instead of being cached as an
/// absence.
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum WorkspaceFileValue {
    Present(Arc<String>),
    Absent,
    ReadError(Arc<String>),
}

/// Immutable, externally observed workspace state for one DICE revision.
///
/// A snapshot lets `WorkspaceFileKey` answer for any requested path: files not
/// represented in the observed state are explicitly absent rather than an
/// uninitialized injected key. `Arc` keeps the long-lived input cheap to clone.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct WorkspaceSnapshot {
    pub files: Arc<SortedMap<PathBuf, WorkspaceFileValue>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct WorkspaceSnapshotKey {
    pub workspace: PathBuf,
}

impl fmt::Display for WorkspaceSnapshotKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "workspace-snapshot:{}", self.workspace.display())
    }
}

impl InjectedKey for WorkspaceSnapshotKey {
    type Value = Arc<WorkspaceSnapshot>;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct WorkspaceFileKey {
    pub workspace: PathBuf,
    pub path: PathBuf,
}

impl fmt::Display for WorkspaceFileKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "workspace-file:{}", self.path.display())
    }
}

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
