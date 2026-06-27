/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::BTreeMap;

use crate::depset::Depset;
use crate::depset::DepsetOrder;
use crate::providers::Runfiles;

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct RunfilesBuilder {
    files: Vec<String>,
    symlinks: BTreeMap<String, String>,
    empty_filenames: Vec<String>,
}

impl RunfilesBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_file(mut self, file: impl Into<String>) -> Self {
        self.files.push(file.into());
        self
    }

    pub fn add_symlink(mut self, link: impl Into<String>, target: impl Into<String>) -> Self {
        self.symlinks.insert(link.into(), target.into());
        self
    }

    pub fn add_empty_filename(mut self, path: impl Into<String>) -> Self {
        self.empty_filenames.push(path.into());
        self
    }

    pub fn build(self) -> Runfiles {
        Runfiles {
            files: Depset::from_direct(DepsetOrder::Default, self.files)
                .expect("runfiles direct depset should be valid"),
            symlinks: self.symlinks,
            empty_filenames: Depset::from_direct(DepsetOrder::Default, self.empty_filenames)
                .expect("runfiles empty filenames depset should be valid"),
        }
    }
}
