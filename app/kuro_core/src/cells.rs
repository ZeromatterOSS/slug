/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//!
//! # Cell
//! A 'Cell' is sub-project within the main project for Buck. All files
//! reachable by Buck is belongs to a single Cell.
//! Cells can be sub-directories of other cells, but that makes that
//! sub-directory part of the sub-cell and no longer part of the parent cell.
//! For example, let's say there's cells 'parent-cell' and 'sub-cell' declared
//! in folders of the same names.
//! ```text
//!  parent-cell
//! +-- folder1
//! +-- folder2
//! +-- sub-cell
//! |   +-- folder3
//! ```
//! All files part of `folder1` and `folder2` will be part of 'parent-cell'.
//! Anything part of `sub-cell`, including `folder3`, are only part of the
//! 'sub-cell'.
//!
//! For users, each Cell is identified by 'CellAlias's. A 'CellAlias' is a
//! human-readable name that contains alphanumeric characters and underscores.
//! (i.e. shouldn't contain any special characters like `/`). Something like `1`
//! is a valid identifier, though not we do not suggest such naming as it's not
//! very descriptive.
//!
//! It's possible that in certain cell contexts, some Cells are not reachable by
//! any 'CellAlias'. However, in the global context, every Cell will be
//! reachable by at least one 'CellAlias'.
//!
//! ## Cell Alias
//! The cell alias appears within a fully qualified target with the syntax
//! `<cell alias>//<target label>`. For example, in `foo//some:target`, `foo` is
//! the cell alias. Examples like `foo/bar//some:target` has an invalid cell
//! alias of `foo/bar` since special characters are forbidden.
//!
//! The 'CellAlias' is specified via configuration files per cell. A
//! configuration specifies these with the syntax `<cell alias>=<relative path
//! to cell>`. We allow a many to one mapping from 'CellAlias' to Cell.
//!
//! Each Cell may give different aliases to the same cell. The 'CellAlias' will
//! be resolved based on the contextual cell that the alias appears in.
//! e.g. `mycell//foo:bar` build file will have any aliases that appears within
//! it be resolved using the aliases defined in `mycell` cell.
//!
//! Cells may omit declaring aliases for cells that exists globally. This means
//! that there will be no alias for those cells, and hence render those cells
//! inaccessible from the cell context that doesn't declare them.
//!
//! ### The Empty Cell Alias
//! The empty cell alias is a special alias injected by Buck to represent the
//! current contextual cell. That means, inside `mycell` cell, references to the
//! 'CellAlias' `""` will resolve to the `mycell` cell.
//!
//! ## Cell Name
//! Each Cell is uniquely identifier globally via a one to one mapping to a
//! 'CellName'. A 'CellName' is a canonicalized, human-readable name that
//! corresponds to a 'CellInstance'. The cell name is inferred from the global
//! list of 'CellAlias's available, by picking the first alias for each cell
//! path based on lexicogrpahic ordering of the aliases. The 'CellName' is
//! subject to the same character restrictions as 'CellAlias'.
//!
//! # Resolving Cells
//! Cells are represented by 'CellInstance'. The 'CellResolver' is able to
//! resolve 'CellNames' to 'CellInstance's. It is also able to find the
//! containing Cell given a path. 'CellAlias' can be resolved with an
//! 'CellAliasResolver'. Each 'CellInstance' contains a 'CellAliasResolver' for
//! the cell alias mapping for that particular cell.

pub mod alias;
pub mod build_file_cell;
pub mod cell_path;
pub mod cell_path_with_allowed_relative_dir;
pub mod cell_root_path;
pub mod external;
pub mod instance;
pub mod name;
pub mod nested;
pub mod paths;
pub(crate) mod sequence_trie_allocative;
pub mod unchecked_cell_rel_path;

use std::collections::HashMap;
use std::collections::hash_map;
use std::fmt::Debug;
use std::sync::Arc;
use std::sync::RwLock;

use allocative::Allocative;
use dupe::Dupe;
use dupe::OptionDupedExt;
use gazebo::prelude::*;
use instance::CellInstance;
use itertools::Itertools;
use kuro_fs::paths::abs_path::AbsPath;
use kuro_fs::paths::file_name::FileNameBuf;
use sequence_trie::SequenceTrie;

use crate::cells::alias::CellAlias;
use crate::cells::alias::NonEmptyCellAlias;
use crate::cells::cell_path::CellPath;
use crate::cells::cell_path::CellPathRef;
use crate::cells::cell_root_path::CellRootPathBuf;
use crate::cells::name::CellName;
use crate::cells::nested::NestedCells;
use crate::fs::project::ProjectRoot;
use crate::fs::project_rel_path::ProjectRelativePath;
use crate::fs::project_rel_path::ProjectRelativePathBuf;

/// Global storage for the root cell name, used for Bazel compatibility.
/// Set when CellResolver is created, read by workspace_root and artifact path logic.
static ROOT_CELL_NAME: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Global storage for non-root cell names (external repos).
static EXTERNAL_CELL_NAMES: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// Dynamic cell registry for extension repos created at runtime.
/// Maps canonical name → bazel-external path for repos not known at startup
/// (e.g., spoke repos created by the crate extension).
static DYNAMIC_EXTENSION_CELLS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<String, String>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

/// Global project root for dynamic cell filesystem operations.
static DYNAMIC_PROJECT_ROOT: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

/// Register a dynamically-discovered extension repo cell.
/// Called after extension execution materializes repos.
pub fn register_dynamic_extension_cell(canonical_name: String, path: String) {
    if let Ok(mut cells) = DYNAMIC_EXTENSION_CELLS.lock() {
        cells.insert(canonical_name.clone(), path.clone());
    }
    // Also create the external/ symlink for action execution.
    // Extract the short name (last component after '+') for spoke repos.
    let short_name = canonical_name
        .rfind('+')
        .map(|i| &canonical_name[i + 1..])
        .unwrap_or(&canonical_name);
    ensure_external_symlink(short_name, &path);
    // Also create a symlink for the full canonical name if different
    if short_name != canonical_name {
        ensure_external_symlink(&canonical_name, &path);
    }
}

/// Set the project root for dynamic cell filesystem scanning.
pub fn set_dynamic_project_root(root: std::path::PathBuf) {
    let _ = DYNAMIC_PROJECT_ROOT.set(root);
}

/// Get the project root (if set).
pub fn get_dynamic_project_root() -> Option<std::path::PathBuf> {
    DYNAMIC_PROJECT_ROOT.get().cloned()
}

/// Create an `external/<cell_name>` symlink pointing to the cell's actual directory.
/// This is needed because `artifact.path` returns `external/<cell>/...` for external
/// repo source files (matching Bazel convention), but kuro stores repos under
/// `bazel-external/`. The symlink bridges this gap for unsandboxed local execution.
///
/// If an existing symlink points to the wrong target (common when Bazel was run first
/// and left a `external/<cell>` symlink pointing to a different version in
/// `bazel-external/`), it is replaced. Non-symlink entries (directories or files) are
/// left alone — the user put them there.
pub fn ensure_external_symlink(cell_name: &str, cell_path: &str) {
    let project_root = match DYNAMIC_PROJECT_ROOT.get() {
        Some(root) => root.clone(),
        None => return,
    };
    let external_dir = project_root.join("external");
    let link_path = external_dir.join(cell_name);
    let desired_target = std::path::PathBuf::from("..").join(cell_path);
    match link_path.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            // Replace symlink only if it points to a different target.
            // Comparing the raw readlink output avoids stat'ing the target,
            // but can miss absolute-vs-relative flavor mismatches when the
            // same target was created from different call sites. Fall back
            // to canonicalize on mismatch so a semantically-equal link
            // doesn't get remove+create'd (→ invalidation event on every
            // build). See Plan 17.4 / memory/file_watcher_buck_out_alias.
            match std::fs::read_link(&link_path) {
                Ok(current) => {
                    if current == desired_target {
                        return;
                    }
                    match (
                        std::fs::canonicalize(&link_path),
                        std::fs::canonicalize(&desired_target),
                    ) {
                        (Ok(a), Ok(b)) if a == b => return,
                        _ => {
                            tracing::debug!(
                                "ensure_external_symlink: replacing stale link {} (was {} -> now {})",
                                link_path.display(),
                                current.display(),
                                desired_target.display(),
                            );
                            let _ = std::fs::remove_file(&link_path);
                        }
                    }
                }
                Err(_) => {
                    let _ = std::fs::remove_file(&link_path);
                }
            }
        }
        Ok(_) => {
            // Real file/dir at this path — don't clobber it.
            return;
        }
        Err(_) => {
            // No entry yet; fall through to create.
        }
    }
    let _ = std::fs::create_dir_all(&external_dir);
    #[cfg(unix)]
    {
        let _ = std::os::unix::fs::symlink(&desired_target, &link_path);
    }
    #[cfg(windows)]
    {
        let _ = std::os::windows::fs::symlink_dir(&desired_target, &link_path);
    }
}

/// Create `external/` symlinks for all non-root cells.
/// Called once after cell resolver is set up.
pub fn ensure_external_symlinks_for_cells(cells: &[(impl AsRef<str>, impl AsRef<str>)]) {
    if DYNAMIC_PROJECT_ROOT.get().is_none() {
        return;
    }
    for (cell_name, cell_path) in cells {
        let name = cell_name.as_ref();
        let path = cell_path.as_ref();
        if !is_root_cell_name(name) && !path.is_empty() {
            ensure_external_symlink(name, path);
        }
    }
}

/// Look up a dynamically-registered extension repo cell path.
pub fn get_dynamic_extension_cell(name: &str) -> Option<String> {
    DYNAMIC_EXTENSION_CELLS
        .lock()
        .ok()
        .and_then(|cells| cells.get(name).cloned())
}

/// Check if a cell name is the root cell (main workspace).
pub fn is_root_cell_name(cell_name: &str) -> bool {
    cell_name.is_empty()
        || cell_name == "root"
        || ROOT_CELL_NAME.get().map_or(false, |root| root == cell_name)
}

/// Get all non-root cell names (external repos).
pub fn get_external_cell_names() -> Vec<String> {
    EXTERNAL_CELL_NAMES
        .lock()
        .map(|names| names.clone())
        .unwrap_or_default()
}

/// Errors from cell creation
#[derive(kuro_error::Error, Debug)]
#[kuro(input)]
enum CellError {
    #[error("Cell paths `{1}` and `{2}` had the same cell name `{0}`.")]
    DuplicateNames(CellName, CellRootPathBuf, CellRootPathBuf),
    #[error("Two cells, `{0}` and `{1}`, share the same path `{2}`")]
    DuplicatePaths(CellName, CellName, CellRootPathBuf),
    #[error("unknown cell alias: `{0}`. In cell `{1}`, known aliases are: `{}`", .2.iter().sorted().join(", "))]
    UnknownCellAlias(CellAlias, CellName, Vec<NonEmptyCellAlias>),
    #[error("unknown cell name: `{0}`. known cell names are `{}`", .1.iter().join(", "))]
    UnknownCellName(CellName, Vec<CellName>),
    #[error(
        "Cell name `{0}` should be an alias for an existing cell, but `{1}` isn't a known alias"
    )]
    AliasOnlyCell(NonEmptyCellAlias, NonEmptyCellAlias),
    #[error("Cell `{0}` alias `{0}` should point to itself, but it points to `{1}`")]
    WrongSelfAlias(CellName, CellName),
    #[error("No cell name for the root path, add an entry for `.`")]
    NoRootCell,
}

/// A 'CellAliasResolver' is unique to a 'CellInstance'.
/// It is responsible for resolving all 'CellAlias' encountered within the
/// 'CellInstance' into the global canonical 'CellName's
#[derive(Clone, Dupe, Debug, PartialEq, Eq, Allocative)]
pub struct CellAliasResolver {
    /// Current cell name.
    current: CellName,
    aliases: Arc<HashMap<NonEmptyCellAlias, CellName>>,
}

impl CellAliasResolver {
    /// Create an instance of `CellAliasResolver`. The special alias `""` must be present, or
    /// this will fail
    pub fn new(
        current: CellName,
        mut aliases: HashMap<NonEmptyCellAlias, CellName>,
    ) -> kuro_error::Result<CellAliasResolver> {
        let current_as_alias = NonEmptyCellAlias::new(current.as_str().to_owned())?;
        if let Some(alias_target) = aliases.insert(current_as_alias, current) {
            if alias_target != current {
                return Err(CellError::WrongSelfAlias(current, alias_target).into());
            }
        }

        let aliases = Arc::new(aliases);

        Ok(CellAliasResolver { current, aliases })
    }

    pub fn new_for_non_root_cell(
        current: CellName,
        root_aliases: &CellAliasResolver,
        alias_list: impl IntoIterator<Item = (NonEmptyCellAlias, NonEmptyCellAlias)>,
    ) -> kuro_error::Result<CellAliasResolver> {
        let mut aliases: HashMap<_, _> = root_aliases
            .mappings()
            .map(|(x, y)| (x.to_owned(), y))
            .collect();
        for (alias, destination) in alias_list {
            let Some(name) = aliases.get(&destination) else {
                return Err(CellError::AliasOnlyCell(alias, destination).into());
            };
            aliases.insert(alias, *name);
        }
        CellAliasResolver::new(current, aliases)
    }

    /// resolves a 'CellAlias' into its corresponding 'CellName'
    pub fn resolve(&self, alias: &str) -> kuro_error::Result<CellName> {
        if alias.is_empty() {
            return Ok(self.current);
        }
        if let Some(name) = self.aliases.get(alias).duped() {
            return Ok(name);
        }

        // Fallback: For extension repos, sibling repos in the same extension
        // can reference each other. If current cell is "X+Y+Z" and the alias is
        // "foo", try "X+Y+foo" as a canonical cell name.
        let current_str = self.current.as_str();
        if let Some(prefix_end) = current_str.rfind('+') {
            let prefix = &current_str[..=prefix_end]; // "X+Y+"
            let candidate = format!("{}{}", prefix, alias);
            // Check if this is a known alias (canonical names are their own aliases)
            if let Some(name) = self.aliases.get(candidate.as_str()).duped() {
                return Ok(name);
            }
            // Check the global dynamic registry
            if get_dynamic_extension_cell(&candidate).is_some() {
                if let Ok(cell_name) = CellName::unchecked_new(&candidate) {
                    // Register the apparent name as an alias for this cell too
                    register_dynamic_extension_cell(
                        candidate.clone(),
                        format!("bazel-external/{}", candidate),
                    );
                    return Ok(cell_name);
                }
            }
            // Check if a bazel-external directory exists for this candidate
            let candidate_path = format!("bazel-external/{}", candidate);
            if std::path::Path::new(&candidate_path).exists() {
                if let Ok(cell_name) = CellName::unchecked_new(&candidate) {
                    register_dynamic_extension_cell(candidate, candidate_path);
                    return Ok(cell_name);
                }
            }
        }

        Err(kuro_error::Error::from(CellError::UnknownCellAlias(
            CellAlias::new(alias.to_owned()),
            self.current,
            self.aliases.keys().cloned().collect(),
        )))
    }

    /// finds the 'CellName' for the current cell (with the alias `""`. See module docs)
    pub fn resolve_self(&self) -> CellName {
        self.current
    }

    pub fn mappings(&self) -> impl Iterator<Item = (&NonEmptyCellAlias, CellName)> {
        self.aliases.iter().map(|(alias, name)| (alias, *name))
    }
}

/// Resolves 'CellName's into 'CellInstance's.
// TODO(bobyf) we need to check if cells changed
#[derive(Clone, Dupe, Debug, Allocative)]
pub struct CellResolver(Arc<CellResolverInternals>);

impl PartialEq for CellResolver {
    fn eq(&self, other: &Self) -> bool {
        self.0.cells == other.0.cells
            && self.0.root_cell == other.0.root_cell
            && self.0.root_cell_alias_resolver == other.0.root_cell_alias_resolver
    }
}
impl Eq for CellResolver {}

#[derive(Debug, Allocative)]
struct CellResolverInternals {
    cells: HashMap<CellName, CellInstance>,
    /// Dynamically-added cells from extension execution (spoke repos, etc.)
    #[allocative(skip)]
    dynamic_cells: RwLock<HashMap<CellName, CellInstance>>,
    #[allocative(visit = crate::cells::sequence_trie_allocative::visit_sequence_trie)]
    path_mappings: SequenceTrie<FileNameBuf, CellName>,
    root_cell: CellName,
    root_cell_alias_resolver: CellAliasResolver,
}

impl CellResolver {
    pub fn new(
        cells: Vec<CellInstance>,
        root_cell_alias_resolver: CellAliasResolver,
    ) -> kuro_error::Result<CellResolver> {
        let mut path_mappings: SequenceTrie<FileNameBuf, CellName> = SequenceTrie::new();
        let mut root_cell = None;
        for cell in &cells {
            if cell.path().is_empty() {
                root_cell = Some(cell.name());
            }
            let prev = path_mappings.insert(cell.path().iter(), cell.name());
            if let Some(prev) = prev {
                return Err(
                    CellError::DuplicatePaths(cell.name(), prev, cell.path().to_buf()).into(),
                );
            }
        }

        let mut cells_map: HashMap<CellName, CellInstance> = HashMap::with_capacity(cells.len());
        for cell in cells {
            match cells_map.entry(cell.name()) {
                hash_map::Entry::Occupied(entry) => {
                    return Err(CellError::DuplicateNames(
                        cell.name(),
                        entry.get().path().to_buf(),
                        cell.path().to_buf(),
                    )
                    .into());
                }
                hash_map::Entry::Vacant(entry) => {
                    entry.insert(cell);
                }
            }
        }

        let root_cell = root_cell.ok_or(CellError::NoRootCell)?;
        // Store root cell name globally for Bazel compatibility checks
        let _ = ROOT_CELL_NAME.set(root_cell.as_str().to_owned());
        // Store non-root cell names for include path resolution
        if let Ok(mut ext_names) = EXTERNAL_CELL_NAMES.lock() {
            ext_names.clear();
            for cell_name in cells_map.keys() {
                if *cell_name != root_cell {
                    ext_names.push(cell_name.as_str().to_owned());
                }
            }
        }
        Ok(CellResolver(Arc::new(CellResolverInternals {
            cells: cells_map,
            dynamic_cells: RwLock::new(HashMap::new()),
            root_cell,
            path_mappings,
            root_cell_alias_resolver,
        })))
    }

    /// Get a `Cell` from the `CellMap`
    pub fn get(&self, cell: CellName) -> kuro_error::Result<&CellInstance> {
        if let Some(instance) = self.0.cells.get(&cell) {
            return Ok(instance);
        }

        // Check if this name is an alias for an existing cell.
        // This prevents creating duplicate dynamic cells when a pre-computed
        // extension repo cell exists under a canonical name (e.g.,
        // "rules_rs+crate+crates__typenum-1.19.0") but is referenced by its
        // apparent name ("crates__typenum-1.19.0").
        if let Ok(aliased) = self.0.root_cell_alias_resolver.resolve(cell.as_str()) {
            if aliased != cell {
                if let Some(instance) = self.0.cells.get(&aliased) {
                    return Ok(instance);
                }
            }
        }

        // Check dynamic cells from extension execution.
        // If found, promote to "static" by leaking the reference (safe: cells live for
        // the duration of the build). This avoids holding the RwLock across returns.
        if let Ok(dynamic) = self.0.dynamic_cells.read() {
            if dynamic.contains_key(&cell) {
                // Drop the read lock, get a write lock, and leak a reference
                drop(dynamic);
                return self.get_or_create_dynamic_cell(cell);
            }
        }

        // Check global dynamic registry (populated by extension execution).
        // First try exact match, then try finding canonical name by suffix match
        // (handles placeholder labels that use bare names like "crates__tempfile-3.26.0"
        // instead of canonical "rules_rs+crate+crates__tempfile-3.26.0").
        let dynamic_path = get_dynamic_extension_cell(cell.as_str()).or_else(|| {
            let suffix = format!("+{}", cell.as_str());
            if let Ok(cells) = DYNAMIC_EXTENSION_CELLS.lock() {
                for (canonical, path) in cells.iter() {
                    if canonical.ends_with(&suffix) {
                        return Some(path.clone());
                    }
                }
            }
            None
        });
        if let Some(path) = dynamic_path {
            // Auto-register this cell
            if let Ok(rel_path) = ProjectRelativePath::new(&path) {
                let cell_path = CellRootPathBuf::new(rel_path.to_owned());
                let nested = nested::NestedCells::from_cell_roots(&[], &*cell_path);
                if let Ok(instance) = CellInstance::new(cell, cell_path, None, nested) {
                    // Create external/ symlink for action execution
                    ensure_external_symlink(cell.as_str(), &path);
                    if let Ok(mut dynamic) = self.0.dynamic_cells.write() {
                        dynamic.insert(cell, instance);
                    }
                    return self.get_or_create_dynamic_cell(cell);
                }
            }
        }

        // Last resort: scan bazel-external/ for a directory matching *+{cell_name}
        // This handles spoke repos from extensions that may not be in the dynamic
        // registry yet (e.g., the first time an extension is triggered).
        // Use the root cell's path to determine the project root directory.
        {
            let cell_str = cell.as_str();
            let suffix = format!("+{}", cell_str);
            let bazel_ext_dir = DYNAMIC_PROJECT_ROOT
                .get()
                .map(|root| root.join("bazel-external"))
                .unwrap_or_else(|| std::path::PathBuf::from("bazel-external"));
            if let Ok(entries) = std::fs::read_dir(&bazel_ext_dir) {
                for entry in entries.flatten() {
                    let dir_name = entry.file_name();
                    let dir_name_str = dir_name.to_string_lossy();
                    if dir_name_str.ends_with(&suffix) && entry.path().is_dir() {
                        let path = format!("bazel-external/{}", dir_name_str);
                        if let Ok(rel_path) = ProjectRelativePath::new(&path) {
                            let cell_path = CellRootPathBuf::new(rel_path.to_owned());
                            let nested = nested::NestedCells::from_cell_roots(&[], &*cell_path);
                            if let Ok(instance) = CellInstance::new(cell, cell_path, None, nested) {
                                // Also register in dynamic registry for future lookups
                                register_dynamic_extension_cell(dir_name_str.to_string(), path);
                                if let Ok(mut dynamic) = self.0.dynamic_cells.write() {
                                    dynamic.insert(cell, instance);
                                }
                                return self.get_or_create_dynamic_cell(cell);
                            }
                        }
                    }
                }
            }
        }

        Err(kuro_error::Error::from(CellError::UnknownCellName(
            cell,
            self.0.cells.keys().copied().collect(),
        )))
    }

    /// Helper to get a reference to a dynamic cell, using unsafe to extend the lifetime.
    /// This is safe because dynamic cells are never removed and the CellResolver outlives
    /// all references to its cells within a build session.
    fn get_or_create_dynamic_cell(&self, cell: CellName) -> kuro_error::Result<&CellInstance> {
        let dynamic = self.0.dynamic_cells.read().map_err(|_| {
            CellError::UnknownCellName(cell, self.0.cells.keys().copied().collect())
        })?;
        if let Some(instance) = dynamic.get(&cell) {
            // SAFETY: The CellResolver (and its dynamic_cells) lives for the entire build.
            // Dynamic cells are append-only (never removed). The returned reference will
            // be valid as long as the CellResolver exists.
            let instance_ref: &CellInstance = unsafe { &*(instance as *const CellInstance) };
            Ok(instance_ref)
        } else {
            Err(kuro_error::Error::from(CellError::UnknownCellName(
                cell,
                self.0.cells.keys().copied().collect(),
            )))
        }
    }

    pub fn is_root_cell(&self, name: CellName) -> bool {
        name == self.0.root_cell
    }

    pub fn root_cell(&self) -> CellName {
        self.0.root_cell
    }

    pub fn root_cell_instance(&self) -> &CellInstance {
        self.get(self.root_cell())
            .expect("Should have had a root cell")
    }

    pub fn root_cell_cell_alias_resolver(&self) -> &CellAliasResolver {
        &self.0.root_cell_alias_resolver
    }

    /// Get a `CellName` from a path by finding the best matching cell path that
    /// is a prefix of the current path relative to the project root. e.g. `fbcode/foo/bar` matches
    /// cell path `fbcode`.
    pub fn find<P: AsRef<ProjectRelativePath> + ?Sized>(&self, path: &P) -> CellName {
        *self
            .0
            .path_mappings
            .get_ancestor(path.as_ref().iter())
            // Note: Must have a root cell
            .unwrap()
    }

    pub fn get_cell_path<P: AsRef<ProjectRelativePath> + ?Sized>(&self, path: &P) -> CellPath {
        let path = path.as_ref();
        let cell = self.find(path);
        // Both of these unwraps are ok by construction of the `CellResolver`
        let instance = self.get(cell).unwrap();
        let relative = path
            .strip_prefix(instance.path().as_project_relative_path())
            .unwrap();
        CellPath::new(cell, relative.to_owned().into())
    }

    pub fn get_cell_path_from_abs_path(
        &self,
        path: &AbsPath,
        fs: &ProjectRoot,
    ) -> kuro_error::Result<CellPath> {
        Ok(self.get_cell_path(&fs.relativize_any(path)?))
    }

    pub fn cells(&self) -> impl Iterator<Item = (CellName, &CellInstance)> {
        self.0
            .cells
            .iter()
            .map(|(name, instance)| (*name, instance))
    }

    /// Resolves a given 'Package' to the 'ProjectRelativePath' that points to
    /// the 'Package'
    ///
    /// ```
    /// use std::convert::TryFrom;
    ///
    /// use kuro_core::cells::CellResolver;
    /// use kuro_core::cells::cell_path::CellPath;
    /// use kuro_core::cells::cell_root_path::CellRootPathBuf;
    /// use kuro_core::cells::name::CellName;
    /// use kuro_core::cells::paths::CellRelativePathBuf;
    /// use kuro_core::fs::project_rel_path::ProjectRelativePath;
    /// use kuro_core::fs::project_rel_path::ProjectRelativePathBuf;
    ///
    /// let cell_path = ProjectRelativePath::new("my/cell")?;
    /// let cells = CellResolver::testing_with_name_and_path(
    ///     CellName::testing_new("mycell"),
    ///     CellRootPathBuf::new(cell_path.to_buf()),
    /// );
    ///
    /// let cell_path = CellPath::new(
    ///     CellName::testing_new("mycell"),
    ///     CellRelativePathBuf::unchecked_new("some/path".to_owned()),
    /// );
    ///
    /// assert_eq!(
    ///     cells.resolve_path(cell_path.as_ref())?,
    ///     ProjectRelativePathBuf::unchecked_new("my/cell/some/path".into()),
    /// );
    ///
    /// # kuro_error::Ok(())
    /// ```
    pub fn resolve_path(
        &self,
        cell_path: CellPathRef,
    ) -> kuro_error::Result<ProjectRelativePathBuf> {
        Ok(self.get(cell_path.cell())?.path().join(cell_path.path()))
    }

    // These are constructors for tests.

    pub fn testing_with_name_and_path(
        other_name: CellName,
        other_path: CellRootPathBuf,
    ) -> CellResolver {
        // It is an error to build a CellResolver that doesn't cover the root.
        // Therefore, if it isn't needed for the test, just make one up.
        if other_path.is_empty() {
            Self::testing_with_names_and_paths_with_alias(
                &[(other_name, other_path)],
                HashMap::new(),
            )
        } else {
            Self::testing_with_names_and_paths_with_alias(
                &[
                    (other_name, other_path),
                    (
                        CellName::testing_new("root"),
                        CellRootPathBuf::testing_new(""),
                    ),
                ],
                HashMap::new(),
            )
        }
    }

    pub fn testing_with_names_and_paths(cells: &[(CellName, CellRootPathBuf)]) -> CellResolver {
        Self::testing_with_names_and_paths_with_alias(
            &cells.map(|(name, path)| (*name, path.clone())),
            HashMap::new(),
        )
    }

    pub fn testing_with_names_and_paths_with_alias(
        cells: &[(CellName, CellRootPathBuf)],
        mut root_cell_aliases: HashMap<NonEmptyCellAlias, CellName>,
    ) -> CellResolver {
        assert_eq!(
            cells.len(),
            cells.iter().map(|(cell, _)| *cell).unique().count(),
            "duplicate cell name"
        );
        assert_eq!(
            cells.len(),
            cells
                .iter()
                .map(|(_, path)| path.as_path())
                .unique()
                .count(),
            "duplicate cell paths"
        );

        let all_roots = cells
            .iter()
            .map(|(cell, path)| (*cell, path.as_path()))
            .collect::<Vec<_>>();
        let instances: Vec<CellInstance> = cells
            .iter()
            .map(|(name, path)| {
                CellInstance::new(
                    *name,
                    path.clone(),
                    None,
                    NestedCells::from_cell_roots(&all_roots, path),
                )
                .unwrap()
            })
            .collect();

        let mut root = None;
        for (cell, p) in cells {
            root_cell_aliases.insert(
                NonEmptyCellAlias::new(cell.as_str().to_owned()).unwrap(),
                *cell,
            );
            if p.is_repo_root() {
                root = Some(*cell);
            }
        }

        let root_aliases = CellAliasResolver::new(root.unwrap(), root_cell_aliases).unwrap();

        CellResolver::new(instances, root_aliases).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use kuro_fs::paths::forward_rel_path::ForwardRelativePath;
    use kuro_fs::paths::forward_rel_path::ForwardRelativePathBuf;

    use super::*;
    use crate::cells::cell_root_path::CellRootPath;

    #[test]
    fn test_of_names_and_paths() -> kuro_error::Result<()> {
        use crate::fs::project_rel_path::ProjectRelativePathBuf;

        let cell_resolver = CellResolver::testing_with_name_and_path(
            CellName::testing_new("foo"),
            CellRootPathBuf::new(ProjectRelativePathBuf::unchecked_new("bar".into())),
        );

        let cell = cell_resolver.get(CellName::testing_new("foo"))?;
        assert_eq!(CellName::testing_new("foo"), cell.name());
        assert_eq!("bar", cell.path().as_str());

        Ok(())
    }

    #[test]
    fn test_cells() -> kuro_error::Result<()> {
        let cell1_path = CellRootPath::new(ProjectRelativePath::new("my/cell1")?);
        let cell2_path = CellRootPath::new(ProjectRelativePath::new("cell2")?);
        let cell3_path = CellRootPath::new(ProjectRelativePath::new("my/cell3")?);

        let cells = CellResolver::testing_with_names_and_paths(&[
            (
                CellName::testing_new("root"),
                CellRootPathBuf::testing_new(""),
            ),
            (CellName::testing_new("cell1"), cell1_path.to_buf()),
            (CellName::testing_new("cell2"), cell2_path.to_buf()),
            (CellName::testing_new("cell3"), cell3_path.to_buf()),
        ]);

        assert_eq!(cells.find(cell1_path), CellName::testing_new("cell1"));
        assert_eq!(cells.find(cell2_path), CellName::testing_new("cell2"));
        assert_eq!(cells.find(cell3_path), CellName::testing_new("cell3"));
        assert_eq!(
            cells.find(
                &cell2_path
                    .as_project_relative_path()
                    .join(ForwardRelativePath::new("fake/cell3")?)
            ),
            CellName::testing_new("cell2")
        );
        assert_eq!(
            cells.find(
                &cell3_path
                    .as_project_relative_path()
                    .join(ForwardRelativePath::new("more/foo")?)
            ),
            CellName::testing_new("cell3")
        );

        assert_eq!(
            cells.get_cell_path(cell1_path),
            CellPath::new(
                CellName::testing_new("cell1"),
                ForwardRelativePathBuf::unchecked_new("".to_owned()).into()
            )
        );

        assert_eq!(
            cells.get_cell_path(cell2_path),
            CellPath::new(
                CellName::testing_new("cell2"),
                ForwardRelativePathBuf::unchecked_new("".to_owned()).into()
            )
        );

        assert_eq!(
            cells.get_cell_path(
                &cell2_path
                    .as_project_relative_path()
                    .join(ForwardRelativePath::new("fake/cell3")?)
            ),
            CellPath::new(
                CellName::testing_new("cell2"),
                ForwardRelativePathBuf::unchecked_new("fake/cell3".to_owned()).into()
            )
        );

        Ok(())
    }
}
