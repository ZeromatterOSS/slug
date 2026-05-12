/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use async_trait::async_trait;
use dice::DiceComputations;
use slug_core::package::PackageLabel;
use slug_interpreter::paths::package::PackageFilePath;
use slug_interpreter::paths::path::OwnedStarlarkPath;
use slug_node::super_package::SuperPackage;

use crate::interpreter::dice_calculation_delegate::HasCalculationDelegate;

#[async_trait]
pub trait EvalPackageFile {
    async fn eval_package_file(&mut self, path: PackageLabel) -> slug_error::Result<SuperPackage>;
}

#[async_trait]
impl EvalPackageFile for DiceComputations<'_> {
    async fn eval_package_file(&mut self, path: PackageLabel) -> slug_error::Result<SuperPackage> {
        self.get_interpreter_calculator(OwnedStarlarkPath::PackageFile(
            PackageFilePath::package_file_for_dir(path.as_cell_path()),
        ))
        .await?
        .eval_package_file(path)
        .await
    }
}
