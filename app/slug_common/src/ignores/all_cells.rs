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

use async_trait::async_trait;
use dice::DiceComputations;
use slug_core::cells::cell_path::CellPathRef;
use slug_core::cells::name::CellName;
use slug_core::cells::paths::CellRelativePath;

use crate::dice::cells::HasCellResolver;
use crate::file_ops::dice::DiceFileComputations;
use crate::ignores::file_ignores::CellFileIgnores;

/// Read `<cell_root>/.bazelignore` (via DICE) and convert it to the
/// comma-separated ignore-spec string that `IgnoreSet::from_ignore_spec`
/// expects. See [`crate::ignores::bazelignore::parse_bazelignore`] for the
/// format.
async fn read_bazelignore_spec(
    ctx: &mut DiceComputations<'_>,
    cell_name: CellName,
) -> slug_error::Result<Option<String>> {
    let rel = CellRelativePath::unchecked_new(".bazelignore");
    let path = CellPathRef::new(cell_name, rel);
    let Some(content) = DiceFileComputations::read_file_if_exists(ctx, path).await? else {
        return Ok(None);
    };
    Ok(Some(crate::ignores::bazelignore::parse_bazelignore(
        &content,
    )))
}

#[async_trait]
pub(crate) trait HasCellFileIgnores {
    async fn new_cell_ignores(
        &mut self,
        cell_name: CellName,
    ) -> slug_error::Result<Arc<CellFileIgnores>>;
}

#[async_trait]
impl HasCellFileIgnores for DiceComputations<'_> {
    async fn new_cell_ignores(
        &mut self,
        cell_name: CellName,
    ) -> slug_error::Result<Arc<CellFileIgnores>> {
        let cells = self.get_cell_resolver().await?;
        let instance = cells.get(cell_name)?;

        let ignore_spec_owned = read_bazelignore_spec(self, cell_name).await?;
        let ignore_spec_str = ignore_spec_owned.as_deref().unwrap_or("");

        Ok(Arc::new(CellFileIgnores::new_for_interpreter(
            ignore_spec_str,
            instance.nested_cells().clone(),
            cells.is_root_cell(cell_name),
        )?))
    }
}
