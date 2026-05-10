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
use dupe::Dupe;
use kuro_core::package::package_relative_path::PackageRelativePath;
use kuro_fs::paths::file_name::FileName;
use kuro_fs::paths::file_name::FileNameBuf;
use kuro_util::arc_str::ArcS;
use starlark_map::sorted_set::SortedSet;
use starlark_map::sorted_vec::SortedVec;

use crate::package_listing::file_listing::PackageFileListing;

#[derive(Clone, Dupe, Eq, PartialEq, Debug, Allocative)]
pub struct PackageListing {
    listing: Arc<PackageListingData>,
}

#[derive(Eq, PartialEq, Debug, Allocative)]
struct PackageListingData {
    files: PackageFileListing,
    directories: SortedSet<ArcS<PackageRelativePath>>,
    subpackages: SortedVec<ArcS<PackageRelativePath>>,
    buildfile: FileNameBuf,
}

impl PackageListing {
    pub(crate) fn new(
        files: SortedSet<ArcS<PackageRelativePath>>,
        directories: SortedSet<ArcS<PackageRelativePath>>,
        subpackages: SortedVec<ArcS<PackageRelativePath>>,
        buildfile: FileNameBuf,
    ) -> Self {
        Self {
            listing: Arc::new(PackageListingData {
                files: PackageFileListing { files },
                directories,
                subpackages,
                buildfile,
            }),
        }
    }

    pub fn empty(buildfile: FileNameBuf) -> Self {
        Self::new(
            SortedSet::new(),
            SortedSet::new(),
            SortedVec::new(),
            buildfile,
        )
    }

    pub fn files(&self) -> &PackageFileListing {
        &self.listing.files
    }

    pub fn get_file(&self, file: &PackageRelativePath) -> Option<ArcS<PackageRelativePath>> {
        self.listing.files.get_file(file)
    }

    pub fn get_dir(&self, dir: &PackageRelativePath) -> Option<ArcS<PackageRelativePath>> {
        // Empty paths must refer to a directory, since the whole thing is rooted
        // at a directory. But empty paths are not explicitly added to the `directories` variable,
        // so handle them specially.
        if dir.is_empty() {
            Some(ArcS::from(PackageRelativePath::empty()))
        } else {
            self.listing.directories.get(dir).map(|x| x.dupe())
        }
    }

    pub fn files_within<'a>(
        &'a self,
        dir: &PackageRelativePath,
    ) -> impl Iterator<Item = &'a ArcS<PackageRelativePath>> + use<'a> {
        self.listing.files.files_within(dir)
    }

    pub fn subpackages_within<'a>(
        &'a self,
        dir: &'a PackageRelativePath,
    ) -> impl Iterator<Item = &'a PackageRelativePath> + 'a {
        self.listing
            .subpackages
            .iter()
            .map(|x| x.as_ref())
            .filter(move |x: &&PackageRelativePath| x.starts_with(dir))
    }

    pub fn buildfile(&self) -> &FileName {
        &self.listing.buildfile
    }

    pub(crate) fn file_count(&self) -> usize {
        self.listing.files.files.iter().count()
    }

    pub(crate) fn directory_count(&self) -> usize {
        self.listing.directories.iter().count()
    }

    pub(crate) fn subpackage_count(&self) -> usize {
        self.listing.subpackages.iter().count()
    }

    pub(crate) fn approximate_path_bytes(&self) -> usize {
        self.listing
            .files
            .files
            .iter()
            .map(|path| path.as_ref().as_str().len())
            .sum::<usize>()
            + self
                .listing
                .directories
                .iter()
                .map(|path| path.as_ref().as_str().len())
                .sum::<usize>()
            + self
                .listing
                .subpackages
                .iter()
                .map(|path| path.as_ref().as_str().len())
                .sum::<usize>()
    }
}

pub mod testing {
    use kuro_core::package::package_relative_path::PackageRelativePathBuf;
    use kuro_fs::paths::file_name::FileNameBuf;
    use starlark_map::sorted_set::SortedSet;
    use starlark_map::sorted_vec::SortedVec;

    use crate::package_listing::listing::PackageListing;

    pub trait PackageListingExt {
        fn testing_empty() -> Self;
        fn testing_files(files: &[&str]) -> Self;
        fn testing_new(files: &[&str], buildfile: &str) -> Self;
    }

    impl PackageListingExt for PackageListing {
        fn testing_empty() -> Self {
            Self::testing_files(&[])
        }

        fn testing_files(files: &[&str]) -> Self {
            Self::testing_new(files, "BUILD.bazel")
        }

        fn testing_new(files: &[&str], buildfile: &str) -> Self {
            let files = files
                .iter()
                .map(|f| {
                    PackageRelativePathBuf::try_from((*f).to_owned())
                        .unwrap()
                        .to_arc()
                })
                .collect::<Vec<_>>();
            let directories = files
                .iter()
                .flat_map(|file| {
                    let mut directories = Vec::new();
                    let mut current = file.as_ref();
                    while let Some(parent) = current.parent() {
                        if parent.is_empty() {
                            break;
                        }
                        directories.push(parent.to_arc());
                        current = parent;
                    }
                    directories
                })
                .collect::<Vec<_>>();
            PackageListing::new(
                SortedSet::from_iter(files),
                SortedSet::from_iter(directories),
                SortedVec::new(),
                FileNameBuf::unchecked_new(buildfile),
            )
        }
    }
}
