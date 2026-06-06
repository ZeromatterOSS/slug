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
use derive_more::Display;
use dupe::Dupe;
use slug_util::arc_str::ArcS;

use crate::cells::cell_path::CellPath;
use crate::cells::name::CellName;
use crate::package::PackageLabel;
use crate::package::package_relative_path::PackageRelativePath;

/// Represents the path of a source artifact.
#[derive(
    Clone,
    Debug,
    derive_more::Display,
    Hash,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Allocative,
    strong_hash::StrongHash
)]
#[display("{}", self.as_ref())]
pub struct SourcePath {
    pkg: PackageLabel,
    path: ArcS<PackageRelativePath>,
    root_cell_name: Option<CellName>,
}

impl SourcePath {
    #[inline]
    pub fn new(pkg: PackageLabel, path: ArcS<PackageRelativePath>) -> Self {
        SourcePath {
            pkg,
            path,
            root_cell_name: None,
        }
    }

    #[inline]
    pub fn new_with_root_cell_name(
        pkg: PackageLabel,
        path: ArcS<PackageRelativePath>,
        root_cell_name: Option<CellName>,
    ) -> Self {
        SourcePath {
            pkg,
            path,
            root_cell_name,
        }
    }

    /// This is slow, but OK to use in tests.
    pub fn testing_new(pkg: &str, path: &str) -> Self {
        SourcePath::new(
            PackageLabel::testing_parse(pkg),
            ArcS::from(PackageRelativePath::new(path).unwrap()),
        )
    }

    #[inline]
    pub fn package(&self) -> PackageLabel {
        self.pkg.dupe()
    }

    #[inline]
    pub fn path(&self) -> &PackageRelativePath {
        &self.path
    }

    #[inline]
    pub fn to_cell_path(&self) -> CellPath {
        self.as_ref().to_cell_path()
    }

    #[inline]
    pub fn as_ref(&self) -> SourcePathRef<'_> {
        SourcePathRef {
            pkg: self.pkg.dupe(),
            path: &self.path,
            root_cell_name: self.root_cell_name,
        }
    }
}

#[derive(Display, Debug, Eq, Hash, PartialEq, Copy, Clone, Dupe)]
#[display("{}/{}", pkg, path.as_str())]
pub struct SourcePathRef<'a> {
    pkg: PackageLabel,
    path: &'a ArcS<PackageRelativePath>,
    root_cell_name: Option<CellName>,
}

impl<'a> SourcePathRef<'a> {
    #[inline]
    pub fn new(pkg: PackageLabel, path: &'a ArcS<PackageRelativePath>) -> SourcePathRef<'a> {
        SourcePathRef {
            pkg,
            path,
            root_cell_name: None,
        }
    }

    #[inline]
    pub fn new_with_root_cell_name(
        pkg: PackageLabel,
        path: &'a ArcS<PackageRelativePath>,
        root_cell_name: Option<CellName>,
    ) -> SourcePathRef<'a> {
        SourcePathRef {
            pkg,
            path,
            root_cell_name,
        }
    }

    #[inline]
    pub fn package(&self) -> PackageLabel {
        self.pkg.dupe()
    }

    #[inline]
    pub fn path(&self) -> &PackageRelativePath {
        self.path
    }

    #[inline]
    pub fn root_cell_name(&self) -> Option<CellName> {
        self.root_cell_name
    }

    #[inline]
    pub fn to_cell_path(&self) -> CellPath {
        self.pkg
            .as_cell_path()
            .join(self.path.as_forward_rel_path())
    }

    #[inline]
    pub fn to_owned(&self) -> SourcePath {
        SourcePath {
            pkg: self.pkg.dupe(),
            path: self.path.dupe(),
            root_cell_name: self.root_cell_name,
        }
    }
}
