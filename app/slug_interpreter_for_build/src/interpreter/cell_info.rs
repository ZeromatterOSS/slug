/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::HashMap;
use std::sync::Arc;

use allocative::Allocative;
use slug_core::cells::CellAliasResolver;
use slug_core::cells::CellResolver;
use slug_core::cells::build_file_cell::BuildFileCell;

#[derive(Clone, Debug, Allocative)]
pub struct InterpreterCellInfo {
    cell_name: BuildFileCell,
    cell_resolver: CellResolver,
    cell_alias_resolver: CellAliasResolver,
    bzlmod_module_versions: Arc<HashMap<String, String>>,
}

impl InterpreterCellInfo {
    pub(crate) fn new(
        cell_name: BuildFileCell,
        cell_resolver: CellResolver,
        cell_alias_resolver: CellAliasResolver,
    ) -> slug_error::Result<Self> {
        Self::new_with_bzlmod_module_versions(
            cell_name,
            cell_resolver,
            cell_alias_resolver,
            Arc::new(HashMap::new()),
        )
    }

    pub(crate) fn new_with_bzlmod_module_versions(
        cell_name: BuildFileCell,
        cell_resolver: CellResolver,
        cell_alias_resolver: CellAliasResolver,
        bzlmod_module_versions: Arc<HashMap<String, String>>,
    ) -> slug_error::Result<Self> {
        Ok(Self {
            cell_name,
            cell_resolver,
            cell_alias_resolver,
            bzlmod_module_versions,
        })
    }

    pub(crate) fn name(&self) -> BuildFileCell {
        self.cell_name
    }

    pub fn cell_resolver(&self) -> &CellResolver {
        &self.cell_resolver
    }

    pub fn cell_alias_resolver(&self) -> &CellAliasResolver {
        &self.cell_alias_resolver
    }

    pub(crate) fn bzlmod_module_version(&self, cell_name: &str) -> Option<String> {
        self.bzlmod_module_versions.get(cell_name).cloned()
    }
}
