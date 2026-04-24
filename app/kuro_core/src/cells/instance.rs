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
use derivative::Derivative;
use dupe::Dupe;

use crate::cells::cell_root_path::CellRootPath;
use crate::cells::cell_root_path::CellRootPathBuf;
use crate::cells::external::ExternalCellOrigin;
use crate::cells::name::CellName;
use crate::cells::nested::NestedCells;

#[derive(Debug, kuro_error::Error)]
#[kuro(input)]
enum CellInstanceError {
    #[error(
        "Attempted to refer to cell `{0}`; however, this is an external cell which cannot be used from `{1}`"
    )]
    ExpectedNonExternalCell(CellName, &'static str),
    #[error("External cell `{0}` cannot have a nested cell `{1}`")]
    NestedInExternalCell(CellName, CellName),
}

/// A 'CellInstance', contains a 'CellName' and a path for that cell.
#[derive(Clone, Debug, derive_more::Display, Dupe, Allocative)]
#[display("{}", _0.name)]
pub struct CellInstance(Arc<CellData>);

/// Custom equality ignores the `external` field (see [`CellData`]). Two cell
/// instances with the same canonical name and materialized path are the same
/// as far as DICE caching is concerned; how we got the content doesn't affect
/// what's currently on disk.
impl PartialEq for CellInstance {
    fn eq(&self, other: &Self) -> bool {
        self.0.name == other.0.name
            && self.0.path == other.0.path
            && self.0.nested_cells == other.0.nested_cells
            && external_origin_eq(&self.0.external, &other.0.external)
    }
}
impl Eq for CellInstance {}

/// Semantic equality for `ExternalCellOrigin` that tolerates textual churn
/// in extension-repo metadata. When bzlmod re-resolves the module graph, the
/// canonical-name / extension-id / repo_spec_json fields can each pick up a
/// different textual form (apparent vs canonical module name, sorted vs
/// unsorted JSON attribute keys) without the repo content on disk changing.
/// Treating those as equal keeps the CellResolver stable across warm
/// invocations, which is load-bearing for DICE cache hits on ReadDirKey and
/// friends (see Plan 21).
fn external_origin_eq(a: &Option<ExternalCellOrigin>, b: &Option<ExternalCellOrigin>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => match (a, b) {
            (ExternalCellOrigin::ExtensionRepo(a), ExternalCellOrigin::ExtensionRepo(b)) => {
                a.internal_name == b.internal_name && a.materialized == b.materialized
            }
            // For deterministic origins (Bundled, Git, LocalPath, Bzlmod,
            // RepositoryRule) fall back to the derived equality.
            _ => a == b,
        },
        _ => false,
    }
}

#[derive(Derivative, PartialEq, Eq, Allocative)]
#[derivative(Debug)]
struct CellData {
    /// the fully canonicalized 'CellName'
    name: CellName,
    /// the project relative path to this 'CellInstance'
    path: CellRootPathBuf,
    /// How this cell was sourced (bundled / git / bzlmod / extension repo).
    /// Excluded from `PartialEq` for [`CellInstance`]; see its impl for the
    /// reason. Still part of the derived `CellData` equality for tests and
    /// internal consistency checks.
    external: Option<ExternalCellOrigin>,
    nested_cells: NestedCells,
}

impl CellInstance {
    pub fn new(
        name: CellName,
        path: CellRootPathBuf,
        external: Option<ExternalCellOrigin>,
        nested_cells: NestedCells,
    ) -> kuro_error::Result<CellInstance> {
        if external.is_some()
            && let Some(nested) = nested_cells.check_empty()
        {
            return Err(CellInstanceError::NestedInExternalCell(name, nested).into());
        }
        Ok(CellInstance(Arc::new(CellData {
            name,
            path,
            external,
            nested_cells,
        })))
    }

    /// Get the name of the cell, as supplied in `cell_name//foo:bar`.
    #[inline]
    pub fn name(&self) -> CellName {
        self.0.name.dupe()
    }

    /// Get the path of the cell, where it is routed.
    #[inline]
    pub fn path(&self) -> &CellRootPath {
        &self.0.path
    }

    #[inline]
    pub fn nested_cells(&self) -> &NestedCells {
        &self.0.nested_cells
    }

    #[inline]
    pub fn external(&self) -> Option<&ExternalCellOrigin> {
        self.0.external.as_ref()
    }

    #[inline]
    pub fn expect_non_external(&self, context: &'static str) -> kuro_error::Result<()> {
        match self.0.external {
            Some(_) => Err(CellInstanceError::ExpectedNonExternalCell(self.name(), context).into()),
            None => Ok(()),
        }
    }
}
