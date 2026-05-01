/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::HashSet;

use dice::DiceTransaction;
use kuro_core::bzl::ImportPath;
use kuro_core::cells::build_file_cell::BuildFileCell;
use kuro_core::cells::name::CellName;
use kuro_interpreter::file_type::StarlarkFileType;
use kuro_interpreter::import_paths::HasImportPaths;
use kuro_interpreter::load_module::INTERPRETER_CALCULATION_IMPL;
use kuro_interpreter::load_module::InterpreterCalculation;
use starlark::environment::Globals;

/// The environment in which a Starlark file is evaluated.
pub(crate) struct Environment {
    /// The globals that are driven from Rust.
    pub(crate) globals: Globals,
    /// A path that is implicitly loaded as additional globals.
    preload: Option<ImportPath>,
}

impl Environment {
    pub(crate) async fn new(
        cell: CellName,
        // Retained for binary-API stability with callers that still
        // pass `StarlarkFileType::Buck` etc. — used to gate the
        // prelude scrape, which is gone.
        _path_type: StarlarkFileType,
        dice: &mut DiceTransaction,
    ) -> kuro_error::Result<Environment> {
        let globals = INTERPRETER_CALCULATION_IMPL.get()?.global_env(dice).await?;

        let preload = dice
            .import_paths_for_cell(BuildFileCell::new(cell))
            .await?
            .root_import()
            .cloned();

        Ok(Environment { globals, preload })
    }

    pub(crate) async fn get_names(
        &self,
        // Retained for binary-API stability after the prelude scrape
        // was removed.
        _path_type: StarlarkFileType,
        dice: &DiceTransaction,
    ) -> kuro_error::Result<HashSet<String>> {
        let mut dice = dice.clone();
        let mut names = HashSet::new();

        for x in self.globals.names() {
            names.insert(x.as_str().to_owned());
        }

        if let Some(preload) = &self.preload {
            let m = dice.get_loaded_module_from_import_path(preload).await?;
            for x in m.env().names() {
                names.insert(x.as_str().to_owned());
            }
        }

        Ok(names)
    }
}
