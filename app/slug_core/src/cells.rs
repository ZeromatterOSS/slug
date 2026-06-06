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

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::hash_map;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::sync::Arc;
use std::sync::RwLock;

use allocative::Allocative;
use dupe::Dupe;
use dupe::OptionDupedExt;
use gazebo::prelude::*;
use instance::CellInstance;
use itertools::Itertools;
use sequence_trie::SequenceTrie;
use slug_fs::paths::abs_path::AbsPath;
use slug_fs::paths::file_name::FileNameBuf;
use slug_fs::paths::forward_rel_path::ForwardRelativePathBuf;

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

#[derive(Clone, Debug, PartialEq, Eq)]
struct DynamicBzlmodScope {
    project_root: Option<std::path::PathBuf>,
    output_base: Option<std::path::PathBuf>,
}

impl DynamicBzlmodScope {
    fn for_project_root(project_root: std::path::PathBuf) -> Self {
        Self {
            project_root: Some(project_root),
            output_base: None,
        }
    }

    fn for_workspace(project_root: std::path::PathBuf, output_base: std::path::PathBuf) -> Self {
        Self {
            project_root: Some(project_root),
            output_base: Some(output_base),
        }
    }
}

#[derive(Clone, Debug)]
struct DynamicBzlmodEntry<T> {
    scope: DynamicBzlmodScope,
    value: T,
}

fn dynamic_bzlmod_entry<T>(value: T) -> DynamicBzlmodEntry<T> {
    DynamicBzlmodEntry {
        scope: dynamic_bzlmod_scope(),
        value,
    }
}

fn dynamic_bzlmod_entry_matches_current_scope<T>(entry: &DynamicBzlmodEntry<T>) -> bool {
    entry.scope == dynamic_bzlmod_scope()
}

fn dynamic_bzlmod_value_for_current_scope<T: Clone>(entry: &DynamicBzlmodEntry<T>) -> Option<T> {
    dynamic_bzlmod_entry_matches_current_scope(entry).then(|| entry.value.clone())
}

/// Dynamic cell registry for extension repos created at runtime.
/// Maps canonical name → bazel-external path for repos not known at startup
/// (e.g., spoke repos created by the crate extension).
static DYNAMIC_EXTENSION_CELLS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<String, DynamicBzlmodEntry<String>>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

static DYNAMIC_EXTENSION_CELL_ALIASES: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<String, DynamicBzlmodEntry<String>>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

static SCOPED_BZLMOD_REPO_ALIASES: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<(String, String), DynamicBzlmodEntry<String>>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

static BZLMOD_APPARENT_ALIAS_CACHE: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<String, DynamicBzlmodEntry<Option<String>>>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

static DYNAMIC_EXTENSION_SUFFIX_SCAN_CACHE: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<String, DynamicBzlmodEntry<Option<String>>>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

#[cfg(test)]
pub(crate) static BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK: std::sync::LazyLock<std::sync::Mutex<()>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(()));

/// Plan 36: dynamic-cell sibling registry that carries the
/// `ExtensionRepoCellSetup` alongside the path, so that
/// `get_or_create_dynamic_cell` can wire `ExternalCellOrigin::ExtensionRepo`
/// onto the synthesized `CellInstance`. With the origin set, the existing
/// file-ops layer routes accesses through
/// `slug_external_cells::extension_repo::get_file_ops_delegate`, which
/// drives lazy DICE materialization on first read — the same path
/// `mark_external_cell` produces for `use_repo`'d extension cells at
/// startup.
///
/// Stored separately from `DYNAMIC_EXTENSION_CELLS` so the older path-only
/// callers keep working unchanged.
static DYNAMIC_EXTENSION_CELL_SETUPS: std::sync::LazyLock<
    std::sync::Mutex<
        std::collections::HashMap<
            String,
            DynamicBzlmodEntry<crate::cells::external::ExtensionRepoCellSetup>,
        >,
    >,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

/// Current workspace for the temporary dynamic bzlmod cell adapter.
static DYNAMIC_BZLMOD_SCOPE: std::sync::LazyLock<std::sync::RwLock<DynamicBzlmodScope>> =
    std::sync::LazyLock::new(|| {
        std::sync::RwLock::new(DynamicBzlmodScope {
            project_root: None,
            output_base: None,
        })
    });

const MAX_UNKNOWN_CELL_ALIAS_SUGGESTIONS: usize = 50;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BzlmodRuntimeExtensionCell {
    pub canonical_name: String,
    pub internal_name: String,
    pub path: String,
    pub setup: crate::cells::external::ExtensionRepoCellSetup,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BzlmodRuntimeScopedRepoAlias {
    pub owner_module: String,
    pub apparent_name: String,
    pub target_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BzlmodRuntimeDynamicAlias {
    pub apparent_name: String,
    pub canonical_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct BzlmodRuntimeCellInstallSnapshot {
    pub root_module_name: Option<String>,
    pub extension_cells: Vec<BzlmodRuntimeExtensionCell>,
    pub scoped_aliases: Vec<BzlmodRuntimeScopedRepoAlias>,
    pub dynamic_aliases: Vec<BzlmodRuntimeDynamicAlias>,
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
struct BzlmodRuntimeAliasResolver {
    scoped_aliases: HashMap<(String, String), String>,
    dynamic_aliases: HashMap<String, String>,
    extension_cell_names: HashSet<String>,
    extension_cell_internal_names: HashMap<String, String>,
}

impl BzlmodRuntimeAliasResolver {
    fn from_snapshot(snapshot: &BzlmodRuntimeCellInstallSnapshot) -> Arc<Self> {
        let scoped_aliases = snapshot
            .scoped_aliases
            .iter()
            .map(|alias| {
                (
                    (alias.owner_module.clone(), alias.apparent_name.clone()),
                    alias.target_name.clone(),
                )
            })
            .collect();
        let dynamic_aliases = snapshot
            .dynamic_aliases
            .iter()
            .map(|alias| (alias.apparent_name.clone(), alias.canonical_name.clone()))
            .collect();
        let extension_cell_names = snapshot
            .extension_cells
            .iter()
            .map(|cell| cell.canonical_name.clone())
            .collect();
        let mut extension_cell_internal_names = HashMap::new();
        let mut duplicate_internal_names = HashSet::new();
        for cell in snapshot
            .extension_cells
            .iter()
            .filter(|cell| cell.internal_name != cell.canonical_name)
            .filter(|cell| {
                !extension_id_is_root_owned(
                    cell.setup.extension_id.as_ref(),
                    snapshot.root_module_name.as_deref(),
                )
            })
        {
            match extension_cell_internal_names.entry(cell.internal_name.clone()) {
                hash_map::Entry::Occupied(entry) => {
                    if entry.get() != &cell.canonical_name {
                        duplicate_internal_names.insert(cell.internal_name.clone());
                    }
                }
                hash_map::Entry::Vacant(entry) => {
                    entry.insert(cell.canonical_name.clone());
                }
            }
        }
        for duplicate in duplicate_internal_names {
            extension_cell_internal_names.remove(&duplicate);
        }
        Arc::new(Self {
            scoped_aliases,
            dynamic_aliases,
            extension_cell_names,
            extension_cell_internal_names,
        })
    }

    fn resolve_dynamic_alias(&self, apparent_name: &str) -> Option<String> {
        self.dynamic_aliases.get(apparent_name).cloned()
    }

    fn resolve_internal_extension_alias(&self, apparent_name: &str) -> Option<String> {
        self.extension_cell_internal_names
            .get(apparent_name)
            .cloned()
    }

    fn canonicalize_current_cell(&self, current_cell: &str) -> Option<String> {
        self.has_extension_cell(current_cell)
            .then(|| current_cell.to_owned())
            .or_else(|| self.resolve_dynamic_alias(current_cell))
            .or_else(|| {
                self.extension_cell_internal_names
                    .get(current_cell)
                    .cloned()
            })
    }

    fn has_extension_cell(&self, name: &str) -> bool {
        self.extension_cell_names.contains(name)
    }
}

fn extension_id_is_root_owned(extension_id: &str, root_module_name: Option<&str>) -> bool {
    let bzl_part = extension_id.split('%').next().unwrap_or(extension_id);
    if bzl_part.starts_with("//") {
        return true;
    }
    let stripped = bzl_part
        .strip_prefix("@@")
        .or_else(|| bzl_part.strip_prefix('@'))
        .unwrap_or(bzl_part);
    let Some(pos) = stripped.find("//") else {
        return true;
    };
    let module = &stripped[..pos];
    module == "_main" || root_module_name.is_some_and(|root| module == root)
}

#[derive(Debug)]
struct KnownCellAliasesForError {
    aliases: Vec<NonEmptyCellAlias>,
    total: usize,
}

impl KnownCellAliasesForError {
    fn new(aliases: &HashMap<NonEmptyCellAlias, CellName>) -> Self {
        let total = aliases.len();
        let aliases = aliases
            .keys()
            .sorted()
            .take(MAX_UNKNOWN_CELL_ALIAS_SUGGESTIONS)
            .cloned()
            .collect();
        Self { aliases, total }
    }
}

impl Display for KnownCellAliasesForError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.total == 0 {
            return f.write_str("<none>");
        }

        write!(f, "{}", self.aliases.iter().format(", "))?;

        let omitted = self.total.saturating_sub(self.aliases.len());
        if omitted != 0 {
            write!(
                f,
                " (showing {} of {}; {} omitted)",
                self.aliases.len(),
                self.total,
                omitted
            )?;
        } else {
            write!(f, " ({} total)", self.total)?;
        }

        Ok(())
    }
}

/// Register a dynamically-discovered extension repo cell.
/// Called after extension execution materializes repos.
pub fn register_dynamic_extension_cell(canonical_name: String, path: String) {
    let _ = (canonical_name, path);
}

#[doc(hidden)]
pub fn register_test_dynamic_extension_cell(canonical_name: String, path: String) {
    register_dynamic_extension_cell_for_test(canonical_name, path, true);
}

fn register_dynamic_extension_cell_for_test(
    canonical_name: String,
    path: String,
    create_symlink: bool,
) {
    if let Ok(mut cells) = DYNAMIC_EXTENSION_CELLS.lock() {
        cells.insert(canonical_name.clone(), dynamic_bzlmod_entry(path.clone()));
    }
    cache_bzlmod_apparent_alias_for_canonical_name(&canonical_name);
    cache_dynamic_extension_suffix_for_canonical_name(&canonical_name);
    if create_symlink {
        // Write only the canonical-name symlink (`<owner>+<ext>+<repo>`).
        // Apparent-name aliasing is handled elsewhere through proper alias
        // maps; writing the spoke's last `+`-segment as a symlink can collide
        // with a bzlmod module of the same apparent name.
        ensure_external_symlink(&canonical_name, &path);
    }
}

pub fn register_dynamic_extension_cell_alias(apparent_name: String, canonical_name: String) {
    let _ = (apparent_name, canonical_name);
}

#[doc(hidden)]
pub fn register_test_dynamic_extension_cell_alias(apparent_name: String, canonical_name: String) {
    if let Ok(mut aliases) = DYNAMIC_EXTENSION_CELL_ALIASES.lock() {
        aliases.insert(apparent_name, dynamic_bzlmod_entry(canonical_name));
    }
}

pub fn resolve_dynamic_extension_cell_alias(apparent_name: &str) -> Option<String> {
    let _ = apparent_name;
    None
}

#[doc(hidden)]
pub fn resolve_test_dynamic_extension_cell_alias(apparent_name: &str) -> Option<String> {
    // Exact canonical extension repo names are already Bazel execution
    // identities. `override_repo()` may map a generated repo to the selected
    // module for content/repo-mapping purposes, but Bazel aquery still renders
    // actions for `@@owner++extension+repo` under
    // `external/owner++extension+repo`. Do not collapse an exact generated
    // repo name to the override target while computing action paths.
    if apparent_name.contains('+') && get_test_dynamic_extension_cell(apparent_name).is_some() {
        return Some(apparent_name.to_owned());
    }

    DYNAMIC_EXTENSION_CELL_ALIASES
        .lock()
        .ok()
        .and_then(|aliases| {
            aliases
                .get(apparent_name)
                .and_then(dynamic_bzlmod_value_for_current_scope)
        })
}

pub fn register_scoped_bzlmod_repo_alias(
    owner_module: String,
    apparent_name: String,
    canonical_name: String,
) {
    let _ = (owner_module, apparent_name, canonical_name);
}

#[doc(hidden)]
pub fn register_test_scoped_bzlmod_repo_alias(
    owner_module: String,
    apparent_name: String,
    canonical_name: String,
) {
    if let Ok(mut aliases) = SCOPED_BZLMOD_REPO_ALIASES.lock() {
        aliases.insert(
            (owner_module, apparent_name),
            dynamic_bzlmod_entry(canonical_name),
        );
    }
}

pub fn resolve_scoped_bzlmod_repo_alias(owner_module: &str, apparent_name: &str) -> Option<String> {
    let _ = (owner_module, apparent_name);
    None
}

#[doc(hidden)]
pub fn resolve_test_scoped_bzlmod_repo_alias(
    owner_module: &str,
    apparent_name: &str,
) -> Option<String> {
    SCOPED_BZLMOD_REPO_ALIASES.lock().ok().and_then(|aliases| {
        aliases
            .get(&(owner_module.to_owned(), apparent_name.to_owned()))
            .and_then(dynamic_bzlmod_value_for_current_scope)
    })
}

/// Resolve an apparent repo name using the Bzlmod `use_repo` imports of the
/// module that owns the current extension-generated repo.
///
/// Bazel scopes apparent repo names. A generated repo like
/// `llvm+llvm_source+compiler-rt` resolving `@llvm-project` should use the
/// `llvm` module's imports, not a process-global apparent-name alias.
pub fn resolve_scoped_bzlmod_repo_alias_for_current_cell(
    current_cell: &str,
    apparent_name: &str,
) -> Option<String> {
    let _ = (current_cell, apparent_name);
    None
}

#[doc(hidden)]
pub fn resolve_test_scoped_bzlmod_repo_alias_for_current_cell(
    current_cell: &str,
    apparent_name: &str,
) -> Option<String> {
    if apparent_name.is_empty() {
        return None;
    }

    let aliases = SCOPED_BZLMOD_REPO_ALIASES.lock().ok()?;
    resolve_scoped_bzlmod_repo_alias_for_current_cell_with_lookup(
        current_cell,
        apparent_name,
        &|cell| canonical_test_dynamic_extension_cell_name(cell),
        &|owner_module| {
            aliases
                .get(&(owner_module.to_owned(), apparent_name.to_owned()))
                .and_then(dynamic_bzlmod_value_for_current_scope)
        },
        &|apparent_name| resolve_test_dynamic_extension_cell_alias(apparent_name),
    )
}

fn resolve_scoped_bzlmod_repo_alias_for_current_cell_with_lookup(
    current_cell: &str,
    apparent_name: &str,
    canonicalize_cell: &impl Fn(&str) -> Option<String>,
    lookup: &impl Fn(&str) -> Option<String>,
    resolve_dynamic_alias: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    let canonical_current_cell =
        canonicalize_cell(current_cell).filter(|cell| cell != current_cell);

    if let Some(owner_self_alias) =
        bzlmod_owner_self_alias(current_cell, apparent_name).or_else(|| {
            canonical_current_cell
                .as_deref()
                .and_then(|cell| bzlmod_owner_self_alias(cell, apparent_name))
        })
    {
        return Some(owner_self_alias);
    }

    lookup_scoped_bzlmod_repo_alias_for_cell(current_cell, lookup)
        .or_else(|| {
            canonical_current_cell
                .as_deref()
                .and_then(|canonical_cell| {
                    lookup_scoped_bzlmod_repo_alias_for_cell(canonical_cell, lookup)
                })
        })
        .or_else(|| {
            canonical_current_cell
                .as_deref()
                .or(Some(current_cell))
                .and_then(|cell| {
                    let prefix = bzlmod_extension_repo_prefix(cell)?;
                    resolve_dynamic_alias(apparent_name)
                        .or_else(|| canonicalize_cell(apparent_name))
                        .filter(|canonical| canonical.starts_with(&prefix))
                })
        })
}

fn bzlmod_extension_repo_prefix(cell: &str) -> Option<String> {
    let (owner, rest) = cell.split_once("++")?;
    let (extension_name, _) = rest.split_once('+')?;
    Some(format!("{owner}++{extension_name}+"))
}

fn bzlmod_owner_module_matches(left: &str, right: &str) -> bool {
    left == right || left.strip_suffix('+') == Some(right) || right.strip_suffix('+') == Some(left)
}

fn bzlmod_owner_self_alias(current_cell: &str, apparent_name: &str) -> Option<String> {
    if let Some(module_name) = current_cell.strip_suffix('+')
        && !module_name.contains('+')
        && apparent_name == module_name
    {
        return Some(current_cell.to_owned());
    }
    let (owner, _rest) = current_cell.split_once("++")?;
    (apparent_name == owner).then(|| format!("{owner}+"))
}

fn lookup_scoped_bzlmod_repo_alias_for_cell(
    current_cell: &str,
    lookup: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    if let Some(canonical) = lookup(current_cell).or_else(|| lookup(&format!("{current_cell}+"))) {
        return Some(canonical);
    }

    if let Some(module_name) = current_cell.strip_suffix("+override") {
        if let Some(canonical) = lookup(&format!("{module_name}+")).or_else(|| lookup(module_name))
        {
            return Some(canonical);
        }
    }

    if let Some(module_name) = current_cell.strip_suffix('+') {
        if let Some(canonical) = lookup(module_name) {
            return Some(canonical);
        }
    }

    if let Some((owner, _rest)) = current_cell.split_once("++") {
        let owner_module = &current_cell[..owner.len() + 1];
        return lookup(owner_module).or_else(|| owner_module.strip_suffix('+').and_then(lookup));
    }

    let mut parts = current_cell.splitn(3, '+');
    let owner_module = parts.next()?;
    parts.next()?;
    parts.next()?;

    lookup(owner_module).or_else(|| {
        if owner_module.ends_with('+') {
            None
        } else {
            lookup(&format!("{owner_module}+"))
        }
    })
}

/// Resolve an apparent or internal extension repo name to the canonical Bazel
/// repo name used under `external/`.
pub fn canonical_dynamic_extension_cell_name(name: &str) -> Option<String> {
    let _ = name;
    None
}

#[doc(hidden)]
pub fn canonical_test_dynamic_extension_cell_name(name: &str) -> Option<String> {
    if name.starts_with("crates__") {
        return Some(format!("rules_rs++crate+{name}"));
    }
    if let Some(canonical) = resolve_test_dynamic_extension_cell_alias(name) {
        return Some(canonical);
    }
    let cells = DYNAMIC_EXTENSION_CELLS.lock().ok()?;
    if cells
        .get(name)
        .is_some_and(dynamic_bzlmod_entry_matches_current_scope)
    {
        return Some(name.to_owned());
    }

    // Do not let the suffix fallback rewrite an already-resolved bzlmod module
    // cell. Kuro has both a direct `zstd` module and an extension repo named
    // `ape+ape_cosmos+zstd`; once a target label is in the `zstd` cell, action
    // source paths must stay under `external/zstd`, not the extension repo.
    if canonical_bzlmod_module_cell_name(name).is_some() {
        return None;
    }

    let suffix = format!("+{name}");
    if let Some(canonical) = cells
        .iter()
        .filter(|(_, entry)| dynamic_bzlmod_entry_matches_current_scope(entry))
        .map(|(canonical, _)| canonical)
        .filter(|canonical| canonical.ends_with(&suffix))
        .min()
        .cloned()
    {
        cache_dynamic_extension_suffix_for_canonical_name(&canonical);
        return Some(canonical);
    }

    scan_dynamic_extension_suffix_from_external_dir(name)
}

pub fn canonical_bzlmod_module_cell_name(name: &str) -> Option<String> {
    if name.contains('+') {
        return None;
    }

    let canonical_name = {
        let mut cache = BZLMOD_APPARENT_ALIAS_CACHE.lock().ok()?;
        match cache
            .get(name)
            .and_then(dynamic_bzlmod_value_for_current_scope)
        {
            Some(cached) => cached,
            None => {
                let discovered = scan_bzlmod_apparent_alias_from_external_dir(name);
                cache.insert(name.to_owned(), dynamic_bzlmod_entry(discovered.clone()));
                discovered
            }
        }
    }?;

    Some(canonical_name)
}

pub fn action_external_cell_name(
    project_root: &std::path::Path,
    cell_name: &str,
    cell_path: &str,
) -> String {
    if let Some(name) = cell_path
        .strip_prefix("bazel-external/")
        .and_then(|path| path.split('/').next())
        .filter(|name| name.contains('+'))
        .map(str::to_owned)
    {
        return name;
    }

    #[cfg(not(test))]
    {
        let _ = project_root;
        return cell_name.to_owned();
    }

    #[cfg(test)]
    {
        let filesystem_fallback_allowed = dynamic_bzlmod_directory_scan_allowed();
        filesystem_fallback_allowed
            .then(|| canonical_external_name_from_symlink(project_root, cell_name))
            .flatten()
            .or_else(|| canonical_test_dynamic_extension_cell_name(cell_name))
            .or_else(|| canonical_bzlmod_module_cell_name(cell_name))
            .or_else(|| {
                if !filesystem_fallback_allowed {
                    return None;
                }
                let suffix = format!("+{cell_name}");
                let bazel_external = project_root.join("bazel-external");
                let mut candidates = Vec::new();
                for entry in std::fs::read_dir(bazel_external).ok()?.flatten() {
                    if !entry.path().is_dir() {
                        continue;
                    }
                    let dir_name = entry.file_name();
                    let dir_name = dir_name.to_string_lossy();
                    if dir_name.ends_with(&suffix) {
                        candidates.push(dir_name.into_owned());
                    }
                }
                candidates.sort();
                candidates.into_iter().next()
            })
            .unwrap_or_else(|| cell_name.to_owned())
    }
}

#[cfg(test)]
fn canonical_external_name_from_symlink(
    project_root: &std::path::Path,
    cell_name: &str,
) -> Option<String> {
    if cell_name.contains('+') {
        return None;
    }
    let external = project_root.join("external").join(cell_name);
    let target = std::fs::read_link(external).ok()?;
    let name = target.file_name()?.to_str()?;
    name.contains('+').then(|| name.to_owned())
}

/// Plan 36: register a dynamic extension spoke cell with its
/// `ExtensionRepoCellSetup` so that `get_or_create_dynamic_cell`
/// wires `ExternalCellOrigin::ExtensionRepo` onto the synthesized
/// `CellInstance` — matching how use_repo'd extension cells are
/// registered at startup. With the origin set, the file-ops layer
/// routes accesses through the lazy materialization path.
pub fn register_dynamic_extension_cell_with_setup(
    canonical_name: String,
    path: String,
    setup: crate::cells::external::ExtensionRepoCellSetup,
) {
    let _ = (canonical_name, path, setup);
}

#[doc(hidden)]
pub fn register_test_dynamic_extension_cell_with_setup(
    canonical_name: String,
    path: String,
    setup: crate::cells::external::ExtensionRepoCellSetup,
) {
    if let Ok(mut setups) = DYNAMIC_EXTENSION_CELL_SETUPS.lock() {
        setups.insert(canonical_name.clone(), dynamic_bzlmod_entry(setup));
    }
    register_test_dynamic_extension_cell(canonical_name, path);
}

/// Register a dynamic extension cell without creating its `external/` symlink.
///
/// Lockfile replay can expose thousands of extension-internal spokes. Creating
/// every symlink during daemon startup regresses cold analysis; the resolver
/// installs the symlink when the cell is first requested.
pub fn register_dynamic_extension_cell_with_setup_lazy(
    canonical_name: String,
    path: String,
    setup: crate::cells::external::ExtensionRepoCellSetup,
) {
    let _ = (canonical_name, path, setup);
}

#[doc(hidden)]
pub fn register_test_dynamic_extension_cell_with_setup_lazy(
    canonical_name: String,
    path: String,
    setup: crate::cells::external::ExtensionRepoCellSetup,
) {
    if let Ok(mut setups) = DYNAMIC_EXTENSION_CELL_SETUPS.lock() {
        setups.insert(canonical_name.clone(), dynamic_bzlmod_entry(setup));
    }
    if let Ok(mut cells) = DYNAMIC_EXTENSION_CELLS.lock() {
        cells.insert(canonical_name.clone(), dynamic_bzlmod_entry(path));
    }
    cache_bzlmod_apparent_alias_for_canonical_name(&canonical_name);
}

/// Lookup the `ExtensionRepoCellSetup` for a dynamic extension cell, if
/// one was registered alongside the path mapping.
pub fn get_dynamic_extension_cell_setup(
    name: &str,
) -> Option<crate::cells::external::ExtensionRepoCellSetup> {
    let _ = name;
    None
}

#[doc(hidden)]
pub fn get_test_dynamic_extension_cell_setup(
    name: &str,
) -> Option<crate::cells::external::ExtensionRepoCellSetup> {
    DYNAMIC_EXTENSION_CELL_SETUPS
        .lock()
        .ok()
        .and_then(|m| m.get(name).and_then(dynamic_bzlmod_value_for_current_scope))
}

fn dynamic_project_root() -> Option<std::path::PathBuf> {
    dynamic_bzlmod_scope().project_root
}

fn dynamic_bzlmod_scope() -> DynamicBzlmodScope {
    DYNAMIC_BZLMOD_SCOPE
        .read()
        .ok()
        .map(|scope| scope.clone())
        .unwrap_or(DynamicBzlmodScope {
            project_root: None,
            output_base: None,
        })
}

fn dynamic_bzlmod_directory_scan_allowed() -> bool {
    #[cfg(not(test))]
    {
        // Production bzlmod resolution must come from the resolver/runtime graph
        // or explicit dynamic registrations, not best-effort filesystem scans.
        false
    }
    #[cfg(test)]
    {
        dynamic_bzlmod_scope().output_base.is_none()
    }
}

fn clear_dynamic_bzlmod_state_for_new_root() {
    if let Ok(mut cells) = DYNAMIC_EXTENSION_CELLS.lock() {
        cells.clear();
    }
    if let Ok(mut setups) = DYNAMIC_EXTENSION_CELL_SETUPS.lock() {
        setups.clear();
    }
    if let Ok(mut aliases) = DYNAMIC_EXTENSION_CELL_ALIASES.lock() {
        aliases.clear();
    }
    if let Ok(mut aliases) = SCOPED_BZLMOD_REPO_ALIASES.lock() {
        aliases.clear();
    }
    if let Ok(mut cache) = BZLMOD_APPARENT_ALIAS_CACHE.lock() {
        cache.clear();
    }
    if let Ok(mut cache) = DYNAMIC_EXTENSION_SUFFIX_SCAN_CACHE.lock() {
        cache.clear();
    }
}

fn set_dynamic_bzlmod_scope(scope: DynamicBzlmodScope, always_reset: bool) {
    let Some(root) = scope.project_root.as_ref() else {
        return;
    };
    ensure_execroot_layout(root);
    if scope.output_base.is_none() {
        repair_external_symlink_targets_impl(root);
    }
    let should_reset = DYNAMIC_BZLMOD_SCOPE
        .read()
        .ok()
        .is_none_or(|current_scope| always_reset || *current_scope != scope);
    if should_reset {
        clear_dynamic_bzlmod_state_for_new_root();
    }
    if let Ok(mut current_scope) = DYNAMIC_BZLMOD_SCOPE.write() {
        *current_scope = scope;
    }
}

/// Set the project root for dynamic cell filesystem scanning.
pub fn set_dynamic_project_root(root: std::path::PathBuf) {
    set_dynamic_bzlmod_scope(DynamicBzlmodScope::for_project_root(root), false);
}

/// Reset the temporary dynamic bzlmod cell adapter for a fresh resolution of
/// `root`, even when the daemon is already serving that root.
pub fn reset_dynamic_bzlmod_state_for_project_root(root: std::path::PathBuf) {
    set_dynamic_bzlmod_scope(DynamicBzlmodScope::for_project_root(root), true);
}

/// Reset the temporary dynamic bzlmod cell adapter for a fresh resolution of
/// `root` and output base. The output base is part of bzlmod workspace identity,
/// so transitional process-global entries must not be reused across it.
pub fn reset_dynamic_bzlmod_state_for_workspace(
    root: std::path::PathBuf,
    output_base: std::path::PathBuf,
) {
    set_dynamic_bzlmod_scope(DynamicBzlmodScope::for_workspace(root, output_base), true);
}

/// Path to the per-project execroot directory used as `cwd` for action
/// execution. Returns `<project_root>/execroot/<project_basename>` when
/// the project root has a usable basename, or `None` otherwise (in which
/// case actions fall back to running with `cwd = project_root`).
pub fn execroot_path(project_root: &std::path::Path) -> Option<std::path::PathBuf> {
    let basename = project_root.file_name().and_then(|s| s.to_str())?;
    if basename.is_empty() {
        return None;
    }
    Some(project_root.join("execroot").join(basename))
}

/// Build `<project_root>/execroot/<project_basename>/` as a real
/// directory containing **directory-only** symlinks to each top-level
/// workspace entry.
///
/// Plan 44 Phase 2.5: Bazel's rules_rust runner (and any tool that does
/// `read_dir(exec_root)`) expects exec_root's top level to look like
/// Bazel's exec_root — a synthesized staging dir with the action's
/// declared inputs as top-level entries — not the user's source tree
/// with `Cargo.toml` / `README.md` / etc. at top level. Without this,
/// `cargo_build_script_runner`'s `RULES_RUST_SYMLINK_EXEC_ROOT=1`
/// codepath wipes runfiles symlinks that share a name with a top-level
/// workspace file (`CHANGELOG.md`, `Cargo.toml`, `README.md`, …) and
/// `drain_runfiles_dir` then panics with `NotFound`.
///
/// Including only directories means `read_dir(execroot)` returns just
/// the workspace's directory tree (`external/`, `buck-out/`, plus the
/// user's first-party directories). Top-level leaf files are excluded;
/// well-formed Bazel actions reference them via `external/<repo>/...`
/// or similar paths, which still resolve through the directory
/// symlinks. The execroot is shared across all actions in the build —
/// safe because every action's view of the workspace top-level shape
/// is identical, and outputs flow through the `buck-out/` symlink.
///
/// This also subsumes the older self-symlink behavior: rules_rust's
/// `process_wrapper` resolves `${exec_root}` to the action's cwd, so
/// `${exec_root}/buck-out/...` resolves through the new `buck-out`
/// symlink to the actual `buck-out` tree.
fn ensure_execroot_layout(project_root: &std::path::Path) {
    let Some(execroot) = execroot_path(project_root) else {
        return;
    };

    // Replace any previous symlink-style execroot (older slug builds
    // installed `execroot/<basename> -> ..`).
    if let Ok(meta) = execroot.symlink_metadata() {
        if meta.file_type().is_symlink() {
            let _ = std::fs::remove_file(&execroot);
        }
    }
    if std::fs::create_dir_all(&execroot).is_err() {
        return;
    }

    // Symlink each top-level workspace directory into the execroot,
    // EXCEPT names that commonly appear as subdirectories of crate
    // runfiles trees. Including those at exec_root top-level causes
    // rules_rust's `cargo_build_script_runner` to push them into its
    // `exec_root_links` cleanup list while AlreadyExists at
    // `manifest_dir/<name>` (which create_runfiles_dir populated as a
    // real directory). The cleanup then panics on `remove_symlink`
    // because the path is a real dir, not a symlink.
    //
    // This is a coarse filter — names listed here cover the common
    // cases (Cargo crates' `ci/`, `docs/`, `examples/`, …). Phase 3
    // replaces this with per-action input narrowing (only the
    // inputs the action declares appear at top level).
    let entries = match std::fs::read_dir(project_root) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        // Skip the execroot dir itself to avoid recursive layout.
        if name == "execroot" {
            continue;
        }
        // Skip workspace dirs whose name commonly appears as a
        // top-level subdir of a Cargo crate's runfiles tree.
        if is_likely_runfiles_collision(&name) {
            continue;
        }
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        if !metadata.is_dir() {
            continue;
        }
        let link = execroot.join(&name);
        match link.symlink_metadata() {
            Ok(m) if m.file_type().is_symlink() => {
                // Refresh: remove and recreate. Cheap and avoids
                // stale targets after workspace layout changes.
                let _ = std::fs::remove_file(&link);
            }
            Ok(_) => continue,
            Err(_) => {}
        }
        let target = entry.path();
        #[cfg(unix)]
        {
            let _ = std::os::unix::fs::symlink(&target, &link);
        }
        #[cfg(windows)]
        {
            let _ = std::os::windows::fs::symlink_dir(&target, &link);
        }
    }
}

/// Names that frequently appear at the top level of a Cargo crate's
/// runfiles tree. Including a workspace top-level directory by these
/// names causes the rules_rust runner cleanup to fail (see
/// [`ensure_execroot_layout`]).
///
/// Conservative list — includes only well-known Cargo / Rust-project
/// conventions. False positives only mean those workspace dirs aren't
/// reachable as `cwd-relative/<name>/...` from inside an action; that
/// breaks any first-party action that reads `<name>/...` as a
/// workspace-relative path. Add new entries as collisions surface.
fn is_likely_runfiles_collision(name: &std::ffi::OsStr) -> bool {
    matches!(
        name.to_str(),
        Some(
            "ci" | "docs"
                | "examples"
                | "tests"
                | "src"
                | "benches"
                | "bench"
                | "doc"
                | "assets"
                | "data"
                | "fixtures"
        )
    )
}

/// Get the project root (if set).
pub fn get_dynamic_project_root() -> Option<std::path::PathBuf> {
    dynamic_project_root()
}

/// Create an `external/<cell_name>` symlink pointing to the cell's actual directory.
/// This is needed because `artifact.path` returns `external/<cell>/...` for external
/// repo source files (matching Bazel convention), but slug stores repos under
/// `bazel-external/`. The symlink bridges this gap for unsandboxed local execution.
///
/// If an existing symlink points to the wrong target (common when Bazel was run first
/// and left a `external/<cell>` symlink pointing to a different version in
/// `bazel-external/`), it is replaced. Non-symlink entries (directories or files) are
/// left alone — the user put them there.
/// Score a `bazel-external/<basename>` path for `external/<apparent>` symlink
/// precedence. Higher = more preferred.
///
/// When multiple cells share the same apparent name (e.g. `rules_python`
/// is both the bzlmod module `rules_python+` AND an extension spoke
/// `rules_foreign_cc+ext+rules_python`), the symlink should point at the
/// bzlmod module form because that's where bazel_dep'd consumers expect
/// templates and other source files to live. Extension spokes get their
/// own symlinks under their canonical names elsewhere.
///
///   `rules_python+` / `rules_python+1.9.0` → 3 (module form)
///   `rules_foreign_cc+ext+rules_python` → 1 (extension spoke, 3 segments)
///   `rules_python`                     → 0 (no version, ambiguous)
fn module_form_priority(cell_path: &str) -> u8 {
    let basename = std::path::Path::new(cell_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let plus_count = basename.matches('+').count();
    if plus_count == 1 {
        3
    } else if basename.contains("++") {
        2
    } else if plus_count >= 2 {
        1
    } else {
        0
    }
}

fn external_symlink_relative_target(cell_path: &str) -> std::path::PathBuf {
    let mut target = std::path::PathBuf::from("..");
    for component in cell_path
        .split(['/', '\\'])
        .filter(|component| !component.is_empty())
    {
        target.push(component);
    }
    target
}

fn external_symlink_target(project_root: &std::path::Path, cell_path: &str) -> std::path::PathBuf {
    let mut project_relative_target = std::path::PathBuf::new();
    for component in cell_path
        .split(['/', '\\'])
        .filter(|component| !component.is_empty())
    {
        project_relative_target.push(component);
    }
    let project_target = project_root.join(&project_relative_target);
    std::fs::canonicalize(project_target)
        .unwrap_or_else(|_| external_symlink_relative_target(cell_path))
}

fn resolve_external_symlink_target(
    external_dir: &std::path::Path,
    target: &std::path::Path,
) -> std::path::PathBuf {
    if target.is_absolute() {
        target.to_path_buf()
    } else {
        external_dir.join(target)
    }
}

fn remove_external_symlink(path: &std::path::Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        std::fs::remove_file(path).or_else(|_| std::fs::remove_dir(path))
    }
    #[cfg(not(windows))]
    {
        std::fs::remove_file(path)
    }
}

fn preferred_external_symlink_target_with_module_forms(
    project_root: &std::path::Path,
    apparent_name: &str,
    current_target: &std::path::Path,
    module_forms: Option<&std::collections::HashMap<String, std::path::PathBuf>>,
) -> std::path::PathBuf {
    let Some(current_name) = current_target.file_name().and_then(|s| s.to_str()) else {
        return current_target.to_path_buf();
    };
    let current_priority = module_form_priority(current_name);
    if current_priority < 3 {
        if let Some(module_forms) = module_forms {
            if let Some(module_target) = module_forms.get(apparent_name) {
                return module_target.clone();
            }
        } else if let Some(module_target) =
            preferred_module_form_target(project_root, apparent_name, current_priority)
        {
            return module_target;
        }
    }
    if current_priority >= 2 {
        return current_target.to_path_buf();
    }
    if !current_name.ends_with(&format!("+{apparent_name}")) {
        return current_target.to_path_buf();
    }

    let Some((owner, rest)) = current_name.split_once('+') else {
        return current_target.to_path_buf();
    };
    let canonical_name = format!("{owner}++{rest}");
    if module_form_priority(&canonical_name) <= current_priority {
        return current_target.to_path_buf();
    }
    let candidate = project_root.join("bazel-external").join(canonical_name);
    if candidate.is_dir() {
        return std::fs::canonicalize(&candidate).unwrap_or(candidate);
    }

    current_target.to_path_buf()
}

fn preferred_external_symlink_target(
    project_root: &std::path::Path,
    apparent_name: &str,
    current_target: &std::path::Path,
) -> std::path::PathBuf {
    preferred_external_symlink_target_with_module_forms(
        project_root,
        apparent_name,
        current_target,
        None,
    )
}

fn collect_preferred_module_form_targets(
    project_root: &std::path::Path,
) -> std::collections::HashMap<String, std::path::PathBuf> {
    let bazel_external = project_root.join("bazel-external");
    let Ok(entries) = std::fs::read_dir(&bazel_external) else {
        return std::collections::HashMap::new();
    };
    let mut module_forms = std::collections::HashMap::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|s| s.to_str()).map(str::to_owned) else {
            continue;
        };
        if module_form_priority(&name) != 3 {
            continue;
        }
        let Some((apparent, _version)) = name.split_once('+') else {
            continue;
        };
        if !path.is_dir() {
            continue;
        }
        let candidate = std::fs::canonicalize(&path).unwrap_or(path);
        module_forms
            .entry(apparent.to_owned())
            .and_modify(|current: &mut std::path::PathBuf| {
                if candidate < *current {
                    *current = candidate.clone();
                }
            })
            .or_insert(candidate);
    }
    module_forms
}

fn preferred_module_form_target(
    project_root: &std::path::Path,
    apparent_name: &str,
    current_priority: u8,
) -> Option<std::path::PathBuf> {
    if current_priority >= 3 {
        return None;
    }
    let bazel_external = project_root.join("bazel-external");
    let entries = std::fs::read_dir(&bazel_external).ok()?;
    let prefix = format!("{apparent_name}+");
    let mut best: Option<std::path::PathBuf> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if !name.starts_with(&prefix) || module_form_priority(name) != 3 {
            continue;
        }
        if !path.is_dir() {
            continue;
        }
        let candidate = std::fs::canonicalize(&path).unwrap_or(path);
        if best.as_ref().is_none_or(|current| candidate < *current) {
            best = Some(candidate);
        }
    }
    best
}

fn desired_external_symlink_target(
    project_root: &std::path::Path,
    cell_name: &str,
    cell_path: &str,
) -> (std::path::PathBuf, u8) {
    let target = external_symlink_target(project_root, cell_path);
    let priority = module_form_priority(cell_path);
    if cell_name.contains('+') || !dynamic_bzlmod_directory_scan_allowed() {
        return (target, priority);
    }
    if let Some(module_target) = preferred_module_form_target(project_root, cell_name, priority) {
        return (module_target, 3);
    }
    (
        preferred_external_symlink_target(project_root, cell_name, &target),
        priority,
    )
}

pub fn repair_external_symlink_targets(project_root: &std::path::Path) {
    if !dynamic_bzlmod_directory_scan_allowed() {
        return;
    }
    repair_external_symlink_targets_impl(project_root);
}

fn repair_external_symlink_targets_impl(project_root: &std::path::Path) {
    let external_dir = project_root.join("external");
    let Ok(entries) = std::fs::read_dir(&external_dir) else {
        return;
    };
    let module_forms = collect_preferred_module_form_targets(project_root);

    for entry in entries.flatten() {
        let link_path = entry.path();
        let Ok(metadata) = std::fs::symlink_metadata(&link_path) else {
            continue;
        };
        if !metadata.file_type().is_symlink() {
            continue;
        }
        let Ok(current_target) = std::fs::read_link(&link_path) else {
            continue;
        };
        let current_abs = resolve_external_symlink_target(&external_dir, &current_target);
        let Some(canonical_target) = resolve_symlink_chain(&current_abs) else {
            continue;
        };
        let Some(apparent_name) = link_path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        let repaired_target = preferred_external_symlink_target_with_module_forms(
            project_root,
            apparent_name,
            &canonical_target,
            Some(&module_forms),
        );
        if current_abs == repaired_target {
            continue;
        }

        if let Err(e) = remove_external_symlink(&link_path) {
            tracing::debug!(
                ?e,
                link = %link_path.display(),
                "failed to remove external symlink while repairing target"
            );
            continue;
        }
        #[cfg(unix)]
        {
            if let Err(e) = std::os::unix::fs::symlink(&repaired_target, &link_path) {
                tracing::debug!(
                    ?e,
                    link = %link_path.display(),
                    target = %repaired_target.display(),
                    "failed to recreate external symlink with canonical target"
                );
            }
        }
        #[cfg(windows)]
        {
            if let Err(e) = std::os::windows::fs::symlink_dir(&repaired_target, &link_path) {
                tracing::debug!(
                    ?e,
                    link = %link_path.display(),
                    target = %repaired_target.display(),
                    "failed to recreate external symlink with canonical target"
                );
            }
        }
    }
}

fn resolve_symlink_chain(path: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut current = path.to_path_buf();
    for _ in 0..8 {
        let metadata = std::fs::symlink_metadata(&current).ok()?;
        if !metadata.file_type().is_symlink() {
            return std::fs::canonicalize(&current).ok().or(Some(current));
        }
        let target = std::fs::read_link(&current).ok()?;
        current = if target.is_absolute() {
            target
        } else {
            current.parent()?.join(target)
        };
    }
    None
}

pub fn ensure_external_symlink(cell_name: &str, cell_path: &str) {
    if let Some(project_root) = dynamic_project_root() {
        ensure_external_symlink_with_root(project_root, cell_name, cell_path);
    }
}

/// Like `ensure_external_symlink` but with an explicit owned project root, so
/// production callers (the bzlmod cell-graph install and resolver runtime-cell
/// creation) avoid the process-global `dynamic_project_root()` read (Plan 61 item 5).
pub fn ensure_external_symlink_with_root(
    project_root: std::path::PathBuf,
    cell_name: &str,
    cell_path: &str,
) {
    let filesystem_fallback_allowed = dynamic_bzlmod_directory_scan_allowed();
    let external_dir = project_root.join("external");
    let link_path = external_dir.join(cell_name);
    let (desired_target, desired_priority) =
        desired_external_symlink_target(&project_root, cell_name, cell_path);
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
                        match std::fs::canonicalize(&link_path) {
                            Ok(_) => return,
                            Err(_) => {
                                tracing::debug!(
                                    "ensure_external_symlink: replacing broken link {} (target {})",
                                    link_path.display(),
                                    current.display(),
                                );
                                let _ = remove_external_symlink(&link_path);
                            }
                        }
                    } else {
                        // Precedence: prefer bzlmod-module-form targets
                        // (`name+version`) over extension-spoke targets
                        // (`owner+ext+name`). Multiple extensions can produce
                        // sibling spokes that share the same apparent
                        // `cell_name`; whichever wins the race must be the
                        // module, not a spoke, so consumers of `external/<name>`
                        // (like template-expand actions reading
                        // `external/rules_python/python/private/...`) find
                        // the right files.
                        let current_str = current.to_string_lossy();
                        let current_priority = module_form_priority(&current_str);
                        if filesystem_fallback_allowed
                            && !cell_name.contains('+')
                            && current_priority > desired_priority
                        {
                            tracing::debug!(
                                "ensure_external_symlink: keeping {} (was {} pri={}, would be {} pri={})",
                                link_path.display(),
                                current.display(),
                                current_priority,
                                desired_target.display(),
                                desired_priority,
                            );
                            return;
                        }
                        match (
                            std::fs::canonicalize(&link_path),
                            std::fs::canonicalize(resolve_external_symlink_target(
                                &external_dir,
                                &desired_target,
                            )),
                        ) {
                            (Ok(a), Ok(b)) if a == b => return,
                            (Err(_), Err(_)) => {
                                // Both targets fail to canonicalize — we can't
                                // tell whether the existing link is really
                                // stale. In practice two different callers
                                // (bzlmod resolver and the dynamic extension
                                // cell registry) pick different canonical
                                // names for the same `apparent_name`, and
                                // when the `bazel-external/` target hasn't
                                // been materialized yet, both canonicalize
                                // calls fail. Replacing the link would touch
                                // its mtime on every invocation, the file
                                // watcher would pick that up, and DICE would
                                // invalidate package loads. Leave it.
                                if filesystem_fallback_allowed {
                                    return;
                                }
                                let _ = remove_external_symlink(&link_path);
                            }
                            _ => {
                                tracing::debug!(
                                    "ensure_external_symlink: replacing stale link {} (was {} -> now {})",
                                    link_path.display(),
                                    current.display(),
                                    desired_target.display(),
                                );
                                let _ = remove_external_symlink(&link_path);
                            }
                        }
                    }
                }
                Err(_) => {
                    let _ = remove_external_symlink(&link_path);
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
    ensure_external_symlinks_for_cells_with_root_cell(None, cells)
}

pub fn ensure_external_symlinks_for_cells_with_root_cell(
    root_cell_name: Option<&str>,
    cells: &[(impl AsRef<str>, impl AsRef<str>)],
) {
    let Some(project_root) = dynamic_project_root() else {
        return;
    };
    ensure_external_symlinks_for_cells_with_project_root(project_root, root_cell_name, cells);
}

/// Like `ensure_external_symlinks_for_cells_with_root_cell` but with an explicit
/// project root, so the production bzlmod cell-graph install avoids the process-global
/// `dynamic_project_root()` read (Plan 61 item 5).
pub fn ensure_external_symlinks_for_cells_with_project_root(
    project_root: std::path::PathBuf,
    root_cell_name: Option<&str>,
    cells: &[(impl AsRef<str>, impl AsRef<str>)],
) {
    for (cell_name, cell_path) in cells {
        let name = cell_name.as_ref();
        let path = cell_path.as_ref();
        if !is_root_cell_name_for_context(name, root_cell_name) && !path.is_empty() {
            ensure_external_symlink_with_root(project_root.clone(), name, path);
            let action_name = action_external_cell_name(&project_root, name, path);
            if action_name != name {
                ensure_external_symlink_with_root(project_root.clone(), &action_name, path);
            }
        }
    }
}

fn is_root_cell_name_for_context(cell_name: &str, explicit_root_cell_name: Option<&str>) -> bool {
    cell_name.is_empty()
        || cell_name == "root"
        || explicit_root_cell_name.is_some_and(|root| root == cell_name)
        || {
            #[cfg(test)]
            {
                explicit_root_cell_name.is_none() && is_root_cell_name(cell_name)
            }

            #[cfg(not(test))]
            {
                false
            }
        }
}

/// Look up a dynamically-registered extension repo cell path.
pub fn get_dynamic_extension_cell(name: &str) -> Option<String> {
    let _ = name;
    None
}

#[doc(hidden)]
pub fn get_test_dynamic_extension_cell(name: &str) -> Option<String> {
    DYNAMIC_EXTENSION_CELLS.lock().ok().and_then(|cells| {
        cells
            .get(name)
            .and_then(dynamic_bzlmod_value_for_current_scope)
    })
}

/// Check if a cell name is the root cell (main workspace).
pub fn is_root_cell_name(cell_name: &str) -> bool {
    cell_name.is_empty() || cell_name == "root"
}

/// Errors from cell creation
#[derive(slug_error::Error, Debug)]
#[slug(input)]
enum CellError {
    #[error("Cell paths `{1}` and `{2}` had the same cell name `{0}`.")]
    DuplicateNames(CellName, CellRootPathBuf, CellRootPathBuf),
    #[error("Two cells, `{0}` and `{1}`, share the same path `{2}`")]
    DuplicatePaths(CellName, CellName, CellRootPathBuf),
    #[error("unknown cell alias: `{0}`. In cell `{1}`, known aliases are: `{2}`")]
    UnknownCellAlias(CellAlias, CellName, KnownCellAliasesForError),
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
    bzlmod_runtime_aliases: Option<Arc<BzlmodRuntimeAliasResolver>>,
}

impl CellAliasResolver {
    fn lookup_alias(&self, alias: &str) -> Option<CellName> {
        let alias = NonEmptyCellAlias::new(alias.to_owned()).ok()?;
        self.aliases.get(&alias).duped()
    }

    fn current_as_alias(current: CellName) -> slug_error::Result<NonEmptyCellAlias> {
        NonEmptyCellAlias::new(current.as_str().to_owned())
    }

    fn new_with_shared_aliases(
        current: CellName,
        aliases: Arc<HashMap<NonEmptyCellAlias, CellName>>,
        bzlmod_runtime_aliases: Option<Arc<BzlmodRuntimeAliasResolver>>,
    ) -> slug_error::Result<CellAliasResolver> {
        let current_as_alias = Self::current_as_alias(current)?;
        if let Some(alias_target) = aliases.get(&current_as_alias) {
            if *alias_target != current {
                return Err(CellError::WrongSelfAlias(current, *alias_target).into());
            }
        }

        slug_util::memory_checkpoint::checkpoint(
            "cell_alias_resolver_shared",
            [("aliases", aliases.len())],
        );

        Ok(CellAliasResolver {
            current,
            aliases,
            bzlmod_runtime_aliases,
        })
    }

    /// Create an instance of `CellAliasResolver`. The special alias `""` must be present, or
    /// this will fail
    pub fn new(
        current: CellName,
        aliases: HashMap<NonEmptyCellAlias, CellName>,
    ) -> slug_error::Result<CellAliasResolver> {
        Self::new_with_bzlmod_runtime_aliases(current, aliases, None)
    }

    pub fn new_bzlmod_with_runtime_cell_snapshot(
        current: CellName,
        aliases: HashMap<NonEmptyCellAlias, CellName>,
        runtime_cell_snapshot: &BzlmodRuntimeCellInstallSnapshot,
    ) -> slug_error::Result<CellAliasResolver> {
        Self::new_with_bzlmod_runtime_aliases(
            current,
            aliases,
            Some(BzlmodRuntimeAliasResolver::from_snapshot(
                runtime_cell_snapshot,
            )),
        )
    }

    fn new_with_bzlmod_runtime_aliases(
        current: CellName,
        mut aliases: HashMap<NonEmptyCellAlias, CellName>,
        bzlmod_runtime_aliases: Option<Arc<BzlmodRuntimeAliasResolver>>,
    ) -> slug_error::Result<CellAliasResolver> {
        let input_aliases = aliases.len();
        let current_as_alias = Self::current_as_alias(current)?;
        if let Some(alias_target) = aliases.insert(current_as_alias, current) {
            if alias_target != current {
                return Err(CellError::WrongSelfAlias(current, alias_target).into());
            }
        }

        let aliases = Arc::new(aliases);

        slug_util::memory_checkpoint::checkpoint(
            "cell_alias_resolver_new",
            [("input_aliases", input_aliases), ("aliases", aliases.len())],
        );

        Ok(CellAliasResolver {
            current,
            aliases,
            bzlmod_runtime_aliases,
        })
    }

    pub fn new_for_non_root_cell(
        current: CellName,
        root_aliases: &CellAliasResolver,
        alias_list: impl IntoIterator<Item = (NonEmptyCellAlias, NonEmptyCellAlias)>,
    ) -> slug_error::Result<CellAliasResolver> {
        let mut alias_list = alias_list.into_iter();
        let Some((first_alias, first_destination)) = alias_list.next() else {
            return CellAliasResolver::new_with_shared_aliases(
                current,
                root_aliases.aliases.dupe(),
                root_aliases.bzlmod_runtime_aliases.clone(),
            );
        };

        let mut aliases: HashMap<_, _> = root_aliases.mappings().collect();
        let Some(name) = aliases.get(&first_destination) else {
            return Err(CellError::AliasOnlyCell(first_alias, first_destination).into());
        };
        aliases.insert(first_alias, *name);
        for (alias, destination) in alias_list {
            let Some(name) = aliases.get(&destination) else {
                return Err(CellError::AliasOnlyCell(alias, destination).into());
            };
            aliases.insert(alias, *name);
        }
        CellAliasResolver::new_with_bzlmod_runtime_aliases(
            current,
            aliases,
            root_aliases.bzlmod_runtime_aliases.clone(),
        )
    }

    pub fn new_bzlmod_for_non_root_cell(
        current: CellName,
        root_aliases: &CellAliasResolver,
        aliases: HashMap<NonEmptyCellAlias, CellName>,
    ) -> slug_error::Result<CellAliasResolver> {
        CellAliasResolver::new_with_bzlmod_runtime_aliases(
            current,
            aliases,
            root_aliases.bzlmod_runtime_aliases.clone(),
        )
    }

    /// Resolve an alias using only aliases carried by this resolver: explicit
    /// alias mappings and bzlmod runtime snapshot aliases/cells. This omits
    /// transitional process-global dynamic maps and directory scans.
    pub fn resolve_declared_or_runtime_alias(&self, alias: &str) -> Option<CellName> {
        if alias.is_empty() || alias == self.current.as_str() {
            return Some(self.current);
        }
        let canonical_current_cell = self
            .bzlmod_runtime_aliases
            .as_ref()
            .and_then(|runtime_aliases| {
                runtime_aliases.canonicalize_current_cell(self.current.as_str())
            })
            .filter(|cell| cell != self.current.as_str());
        if let Some(canonical_name) =
            bzlmod_owner_self_alias(self.current.as_str(), alias).or_else(|| {
                canonical_current_cell
                    .as_deref()
                    .and_then(|cell| bzlmod_owner_self_alias(cell, alias))
            })
            && let Ok(cell_name) = CellName::unchecked_new(&canonical_name)
        {
            return Some(cell_name);
        }
        if let Some(canonical_name) = self.resolve_scoped_bzlmod_repo_alias_from_runtime(alias) {
            if let Ok(cell_name) = CellName::unchecked_new(&canonical_name) {
                return Some(cell_name);
            }
        }
        if let Some(name) = self.lookup_alias(alias) {
            return Some(name);
        }

        if let Some(canonical_name) = self.resolve_dynamic_extension_cell_alias_from_runtime(alias)
        {
            if let Ok(cell_name) = CellName::unchecked_new(&canonical_name) {
                return Some(cell_name);
            }
        }

        if let Some(canonical_name) = self.resolve_internal_extension_cell_alias_from_runtime(alias)
        {
            if let Ok(cell_name) = CellName::unchecked_new(&canonical_name) {
                return Some(cell_name);
            }
        }

        if self.has_bzlmod_runtime_extension_cell(alias) {
            if let Ok(cell_name) = CellName::unchecked_new(alias) {
                return Some(cell_name);
            }
        }

        let current_str = self.current.as_str();
        if let Some(prefix_end) = current_str.rfind('+') {
            let prefix = &current_str[..=prefix_end];
            let candidate = format!("{prefix}{alias}");
            if let Some(name) = self.lookup_alias(&candidate) {
                return Some(name);
            }
            if self.has_bzlmod_runtime_extension_cell(&candidate) {
                if let Ok(cell_name) = CellName::unchecked_new(&candidate) {
                    return Some(cell_name);
                }
            }
        }

        None
    }

    pub fn has_bzlmod_runtime_alias_snapshot(&self) -> bool {
        self.bzlmod_runtime_aliases.is_some()
    }

    /// resolves a 'CellAlias' into its corresponding 'CellName'
    pub fn resolve(&self, alias: &str) -> slug_error::Result<CellName> {
        if alias.is_empty() {
            return Ok(self.current);
        }
        if alias == self.current.as_str() {
            return Ok(self.current);
        }
        let canonical_current_cell = self
            .bzlmod_runtime_aliases
            .as_ref()
            .and_then(|runtime_aliases| {
                runtime_aliases.canonicalize_current_cell(self.current.as_str())
            })
            .filter(|cell| cell != self.current.as_str());
        if let Some(canonical_name) =
            bzlmod_owner_self_alias(self.current.as_str(), alias).or_else(|| {
                canonical_current_cell
                    .as_deref()
                    .and_then(|cell| bzlmod_owner_self_alias(cell, alias))
            })
            && let Ok(cell_name) = CellName::unchecked_new(&canonical_name)
        {
            return Ok(cell_name);
        }
        let has_runtime_snapshot = self.has_bzlmod_runtime_alias_snapshot();
        if let Some(canonical_name) = self
            .resolve_scoped_bzlmod_repo_alias_from_runtime(alias)
            .or_else(|| {
                (!has_runtime_snapshot)
                    .then(|| process_global_scoped_bzlmod_alias_fallback(self.current, alias))
                    .flatten()
            })
        {
            if let Ok(cell_name) = CellName::unchecked_new(&canonical_name) {
                return Ok(cell_name);
            }
        }
        if let Some(name) = self.lookup_alias(alias) {
            return Ok(name);
        }

        if let Some(canonical_name) = self
            .resolve_dynamic_extension_cell_alias_from_runtime(alias)
            .or_else(|| {
                (!has_runtime_snapshot)
                    .then(|| process_global_dynamic_extension_alias_fallback(alias))
                    .flatten()
            })
        {
            if let Ok(cell_name) = CellName::unchecked_new(&canonical_name) {
                return Ok(cell_name);
            }
        }

        if let Some(canonical_name) = self.resolve_internal_extension_cell_alias_from_runtime(alias)
        {
            if let Ok(cell_name) = CellName::unchecked_new(&canonical_name) {
                return Ok(cell_name);
            }
        }

        if self.has_bzlmod_runtime_extension_cell(alias)
            || (!has_runtime_snapshot && process_global_dynamic_extension_cell_exists(alias))
        {
            if let Ok(cell_name) = CellName::unchecked_new(alias) {
                return Ok(cell_name);
            }
        }

        if !has_runtime_snapshot {
            if let Some(cell_name) = process_global_apparent_alias_dir_fallback(alias) {
                return Ok(cell_name);
            }
        }

        if matches!(
            alias,
            "bazel_tools" | "slug_builtins" | "local_config_platform" | "local_config_python"
        ) {
            if let Ok(cell_name) = CellName::unchecked_new(alias) {
                return Ok(cell_name);
            }
        }

        // Fallback: For extension repos, sibling repos in the same extension
        // can reference each other. If current cell is "X+Y+Z" and the alias is
        // "foo", try "X+Y+foo" as a canonical cell name.
        let current_str = self.current.as_str();
        if let Some(prefix_end) = current_str.rfind('+') {
            let prefix = &current_str[..=prefix_end]; // "X+Y+"
            let candidate = format!("{}{}", prefix, alias);
            // Check if this is a known alias (canonical names are their own aliases)
            if let Some(name) = self.lookup_alias(&candidate) {
                return Ok(name);
            }
            if self.has_bzlmod_runtime_extension_cell(&candidate) {
                if let Ok(cell_name) = CellName::unchecked_new(&candidate) {
                    return Ok(cell_name);
                }
            }
            if !has_runtime_snapshot {
                if process_global_dynamic_extension_cell_exists(&candidate) {
                    if let Ok(cell_name) = CellName::unchecked_new(&candidate) {
                        process_global_register_dynamic_extension_cell_fallback(
                            candidate.clone(),
                            format!("bazel-external/{}", candidate),
                        );
                        return Ok(cell_name);
                    }
                }
                if let Some(candidate_path) =
                    process_global_dynamic_extension_dir_fallback(&candidate)
                {
                    if let Ok(cell_name) = CellName::unchecked_new(&candidate) {
                        process_global_register_dynamic_extension_cell_fallback(
                            candidate,
                            candidate_path,
                        );
                        return Ok(cell_name);
                    }
                }
            }
        }

        Err(slug_error::Error::from(CellError::UnknownCellAlias(
            CellAlias::new(alias.to_owned()),
            self.current,
            KnownCellAliasesForError::new(&self.aliases),
        )))
    }

    fn resolve_scoped_bzlmod_repo_alias_from_runtime(&self, alias: &str) -> Option<String> {
        let runtime_aliases = self.bzlmod_runtime_aliases.as_ref()?;
        resolve_scoped_bzlmod_repo_alias_for_current_cell_with_lookup(
            self.current.as_str(),
            alias,
            &|cell| runtime_aliases.canonicalize_current_cell(cell),
            &|owner_module| {
                runtime_aliases
                    .scoped_aliases
                    .get(&(owner_module.to_owned(), alias.to_owned()))
                    .cloned()
            },
            &|apparent_name| runtime_aliases.resolve_dynamic_alias(apparent_name),
        )
    }

    fn resolve_dynamic_extension_cell_alias_from_runtime(&self, alias: &str) -> Option<String> {
        self.bzlmod_runtime_aliases
            .as_ref()
            .and_then(|runtime_aliases| runtime_aliases.resolve_dynamic_alias(alias))
    }

    fn resolve_internal_extension_cell_alias_from_runtime(&self, alias: &str) -> Option<String> {
        self.bzlmod_runtime_aliases
            .as_ref()
            .and_then(|runtime_aliases| runtime_aliases.resolve_internal_extension_alias(alias))
    }

    fn has_bzlmod_runtime_extension_cell(&self, name: &str) -> bool {
        self.bzlmod_runtime_aliases
            .as_ref()
            .is_some_and(|runtime_aliases| runtime_aliases.has_extension_cell(name))
    }

    /// finds the 'CellName' for the current cell (with the alias `""`. See module docs)
    pub fn resolve_self(&self) -> CellName {
        self.current
    }

    pub fn mappings(&self) -> Box<dyn Iterator<Item = (NonEmptyCellAlias, CellName)> + '_> {
        let self_alias = Self::current_as_alias(self.current)
            .expect("CellName must be a valid non-empty cell alias");
        if self.aliases.contains_key(&self_alias) {
            Box::new(self.aliases.iter().map(|(alias, name)| (*alias, *name)))
        } else {
            Box::new(
                self.aliases
                    .iter()
                    .map(|(alias, name)| (*alias, *name))
                    .chain(std::iter::once((self_alias, self.current))),
            )
        }
    }
}

#[cfg(test)]
fn process_global_scoped_bzlmod_alias_fallback(current: CellName, alias: &str) -> Option<String> {
    resolve_test_scoped_bzlmod_repo_alias_for_current_cell(current.as_str(), alias)
}

#[cfg(not(test))]
fn process_global_scoped_bzlmod_alias_fallback(_current: CellName, _alias: &str) -> Option<String> {
    None
}

#[cfg(test)]
fn process_global_dynamic_extension_alias_fallback(alias: &str) -> Option<String> {
    resolve_test_dynamic_extension_cell_alias(alias)
}

#[cfg(not(test))]
fn process_global_dynamic_extension_alias_fallback(_alias: &str) -> Option<String> {
    None
}

#[cfg(test)]
fn process_global_dynamic_extension_cell_exists(name: &str) -> bool {
    get_test_dynamic_extension_cell(name).is_some()
}

#[cfg(not(test))]
fn process_global_dynamic_extension_cell_exists(_name: &str) -> bool {
    false
}

#[cfg(test)]
fn process_global_apparent_alias_dir_fallback(alias: &str) -> Option<CellName> {
    resolve_bzlmod_apparent_alias_from_external_dir(alias)
}

#[cfg(not(test))]
fn process_global_apparent_alias_dir_fallback(_alias: &str) -> Option<CellName> {
    None
}

#[cfg(test)]
fn process_global_dynamic_extension_dir_fallback(candidate: &str) -> Option<String> {
    if dynamic_bzlmod_directory_scan_allowed() {
        let candidate_path = format!("bazel-external/{}", candidate);
        std::path::Path::new(&candidate_path)
            .exists()
            .then_some(candidate_path)
    } else {
        None
    }
}

#[cfg(not(test))]
fn process_global_dynamic_extension_dir_fallback(_candidate: &str) -> Option<String> {
    None
}

#[cfg(test)]
fn process_global_register_dynamic_extension_cell_fallback(name: String, path: String) {
    register_test_dynamic_extension_cell(name, path);
}

#[cfg(not(test))]
fn process_global_register_dynamic_extension_cell_fallback(_name: String, _path: String) {}

#[cfg(test)]
fn resolve_bzlmod_apparent_alias_from_external_dir(alias: &str) -> Option<CellName> {
    if alias.contains('+') {
        return None;
    }
    if !dynamic_bzlmod_directory_scan_allowed() {
        return None;
    }

    let canonical_name = {
        let mut cache = BZLMOD_APPARENT_ALIAS_CACHE.lock().ok()?;
        match cache
            .get(alias)
            .and_then(dynamic_bzlmod_value_for_current_scope)
        {
            Some(cached) => cached,
            None => {
                let discovered = scan_bzlmod_apparent_alias_from_external_dir(alias);
                cache.insert(alias.to_owned(), dynamic_bzlmod_entry(discovered.clone()));
                discovered
            }
        }
    }?;

    let path = format!("bazel-external/{}", canonical_name);
    register_test_dynamic_extension_cell(canonical_name.clone(), path);
    register_test_dynamic_extension_cell_alias(alias.to_owned(), canonical_name.clone());
    CellName::unchecked_new(&canonical_name).ok()
}

fn scan_bzlmod_apparent_alias_from_external_dir(alias: &str) -> Option<String> {
    if !dynamic_bzlmod_directory_scan_allowed() {
        return None;
    }
    let bazel_ext_dir = dynamic_project_root()
        .map(|root| root.join("bazel-external"))
        .unwrap_or_else(|| std::path::PathBuf::from("bazel-external"));
    let prefix = format!("{}+", alias);
    let mut best: Option<String> = None;
    for entry in std::fs::read_dir(&bazel_ext_dir).ok()?.flatten() {
        let file_type = entry.file_type().ok()?;
        if !file_type.is_dir() && !file_type.is_symlink() {
            continue;
        }
        let dir_name = entry.file_name();
        let Some(dir_name) = dir_name.to_str() else {
            continue;
        };
        if !dir_name.starts_with(&prefix) {
            continue;
        }
        let suffix = &dir_name[prefix.len()..];
        if suffix.contains('+') {
            continue;
        }
        match &best {
            Some(candidate) if candidate.as_str() <= dir_name => {}
            _ => best = Some(dir_name.to_owned()),
        }
    }
    best
}

fn cache_bzlmod_apparent_alias_for_canonical_name(canonical_name: &str) {
    let Some((alias, suffix)) = canonical_name.split_once('+') else {
        return;
    };
    if alias.is_empty() || suffix.contains('+') {
        return;
    }
    if let Ok(mut cache) = BZLMOD_APPARENT_ALIAS_CACHE.lock() {
        let replace = match cache.get(alias) {
            Some(entry) if dynamic_bzlmod_entry_matches_current_scope(entry) => entry
                .value
                .as_deref()
                .is_none_or(|existing| canonical_name < existing),
            _ => true,
        };
        if replace {
            cache.insert(
                alias.to_owned(),
                dynamic_bzlmod_entry(Some(canonical_name.to_owned())),
            );
        }
    }
}

fn scan_dynamic_extension_suffix_from_external_dir(name: &str) -> Option<String> {
    if !dynamic_bzlmod_directory_scan_allowed() {
        return None;
    }
    if name.contains('+') {
        return scan_exact_dynamic_extension_from_external_dir(name);
    }

    let cached = DYNAMIC_EXTENSION_SUFFIX_SCAN_CACHE
        .lock()
        .ok()
        .and_then(|cache| {
            cache
                .get(name)
                .and_then(dynamic_bzlmod_value_for_current_scope)
        });
    if let Some(cached) = cached {
        return cached;
    }

    let discovered = scan_dynamic_extension_suffix_from_external_dir_uncached(name);
    if let Ok(mut cache) = DYNAMIC_EXTENSION_SUFFIX_SCAN_CACHE.lock() {
        cache.insert(name.to_owned(), dynamic_bzlmod_entry(discovered.clone()));
    }
    discovered
}

fn scan_exact_dynamic_extension_from_external_dir(name: &str) -> Option<String> {
    let cached = DYNAMIC_EXTENSION_SUFFIX_SCAN_CACHE
        .lock()
        .ok()
        .and_then(|cache| {
            cache
                .get(name)
                .and_then(dynamic_bzlmod_value_for_current_scope)
        });
    if let Some(cached) = cached {
        return cached;
    }

    let discovered = dynamic_project_root()
        .map(|root| root.join("bazel-external").join(name))
        .or_else(|| Some(std::path::PathBuf::from("bazel-external").join(name)))
        .filter(|path| path.is_dir())
        .map(|_| name.to_owned());
    if let Ok(mut cache) = DYNAMIC_EXTENSION_SUFFIX_SCAN_CACHE.lock() {
        cache.insert(name.to_owned(), dynamic_bzlmod_entry(discovered.clone()));
    }
    discovered
}

fn scan_dynamic_extension_suffix_from_external_dir_uncached(name: &str) -> Option<String> {
    if !dynamic_bzlmod_directory_scan_allowed() {
        return None;
    }
    let bazel_ext_dir = dynamic_project_root()
        .map(|root| root.join("bazel-external"))
        .unwrap_or_else(|| std::path::PathBuf::from("bazel-external"));
    let suffix = format!("+{name}");
    let mut candidates = Vec::new();
    for entry in std::fs::read_dir(bazel_ext_dir).ok()?.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() && !file_type.is_symlink() {
            continue;
        }
        let dir_name = entry.file_name();
        let dir_name = dir_name.to_string_lossy();
        if dir_name.ends_with(&suffix) {
            candidates.push(dir_name.into_owned());
        }
    }
    candidates.sort();
    candidates.into_iter().next()
}

fn cache_dynamic_extension_suffix_for_canonical_name(canonical_name: &str) {
    let Some((_, suffix)) = canonical_name.rsplit_once('+') else {
        return;
    };
    if suffix.is_empty() {
        return;
    }
    if let Ok(mut cache) = DYNAMIC_EXTENSION_SUFFIX_SCAN_CACHE.lock() {
        let replace_suffix = match cache.get(suffix) {
            Some(entry) if dynamic_bzlmod_entry_matches_current_scope(entry) => entry
                .value
                .as_deref()
                .is_none_or(|existing| canonical_name < existing),
            _ => true,
        };
        if replace_suffix {
            cache.insert(
                suffix.to_owned(),
                dynamic_bzlmod_entry(Some(canonical_name.to_owned())),
            );
        }
        cache.insert(
            canonical_name.to_owned(),
            dynamic_bzlmod_entry(Some(canonical_name.to_owned())),
        );
    }
}

#[cfg(test)]
fn clear_bzlmod_apparent_alias_cache_for_tests() {
    if let Ok(mut cache) = BZLMOD_APPARENT_ALIAS_CACHE.lock() {
        cache.clear();
    }
}

#[cfg(test)]
fn clear_dynamic_extension_suffix_scan_cache_for_tests() {
    if let Ok(mut cache) = DYNAMIC_EXTENSION_SUFFIX_SCAN_CACHE.lock() {
        cache.clear();
    }
}

#[cfg(test)]
fn cached_bzlmod_apparent_alias_for_tests(alias: &str) -> Option<Option<String>> {
    BZLMOD_APPARENT_ALIAS_CACHE.lock().ok().and_then(|cache| {
        cache
            .get(alias)
            .and_then(dynamic_bzlmod_value_for_current_scope)
    })
}

#[cfg(test)]
fn cached_dynamic_extension_suffix_for_tests(alias: &str) -> Option<Option<String>> {
    DYNAMIC_EXTENSION_SUFFIX_SCAN_CACHE
        .lock()
        .ok()
        .and_then(|cache| {
            cache
                .get(alias)
                .and_then(dynamic_bzlmod_value_for_current_scope)
        })
}

#[cfg(test)]
mod bzlmod_apparent_alias_cache_tests {
    use super::*;

    #[test]
    fn cache_tracks_lexicographically_first_module_form_alias() {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        clear_bzlmod_apparent_alias_cache_for_tests();

        cache_bzlmod_apparent_alias_for_canonical_name("rules_python+1.9.0");
        cache_bzlmod_apparent_alias_for_canonical_name("rules_python+");
        cache_bzlmod_apparent_alias_for_canonical_name("rules_python+1.8.0");
        cache_bzlmod_apparent_alias_for_canonical_name("rules_python+ext+spoke");
        cache_bzlmod_apparent_alias_for_canonical_name("plain_repo");

        assert_eq!(
            cached_bzlmod_apparent_alias_for_tests("rules_python"),
            Some(Some("rules_python+".to_owned()))
        );
        assert_eq!(cached_bzlmod_apparent_alias_for_tests("plain_repo"), None);
    }

    #[test]
    fn cache_updates_negative_lookup_when_module_form_registers_later() {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        clear_bzlmod_apparent_alias_cache_for_tests();

        BZLMOD_APPARENT_ALIAS_CACHE
            .lock()
            .unwrap()
            .insert("rules_cc".to_owned(), dynamic_bzlmod_entry(None));
        cache_bzlmod_apparent_alias_for_canonical_name("rules_cc+0.2.9");

        assert_eq!(
            cached_bzlmod_apparent_alias_for_tests("rules_cc"),
            Some(Some("rules_cc+0.2.9".to_owned()))
        );
    }

    #[test]
    fn dynamic_extension_suffix_cache_updates_negative_lookup_when_repo_registers_later() {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        clear_dynamic_extension_suffix_scan_cache_for_tests();

        DYNAMIC_EXTENSION_SUFFIX_SCAN_CACHE
            .lock()
            .unwrap()
            .insert("generated".to_owned(), dynamic_bzlmod_entry(None));
        register_test_dynamic_extension_cell(
            "owner++extension+generated".to_owned(),
            "bazel-external/owner++extension+generated".to_owned(),
        );

        assert_eq!(
            cached_dynamic_extension_suffix_for_tests("generated"),
            Some(Some("owner++extension+generated".to_owned()))
        );
        assert_eq!(
            cached_dynamic_extension_suffix_for_tests("owner++extension+generated"),
            Some(Some("owner++extension+generated".to_owned()))
        );
    }

    #[test]
    fn dynamic_extension_suffix_cache_clears_on_bzlmod_root_reset() {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        clear_dynamic_extension_suffix_scan_cache_for_tests();

        DYNAMIC_EXTENSION_SUFFIX_SCAN_CACHE.lock().unwrap().insert(
            "generated".to_owned(),
            dynamic_bzlmod_entry(Some("old++ext+generated".to_owned())),
        );

        let tmp = tempfile::tempdir().unwrap();
        reset_dynamic_bzlmod_state_for_project_root(tmp.path().to_path_buf());

        assert_eq!(cached_dynamic_extension_suffix_for_tests("generated"), None);
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
            && self.0.bzlmod_runtime_cell_snapshot == other.0.bzlmod_runtime_cell_snapshot
    }
}
impl Eq for CellResolver {}

#[derive(Debug, Allocative)]
struct CellResolverInternals {
    cells: HashMap<CellName, CellInstance>,
    /// Bzlmod extension cells published by the resolver's cell graph.
    ///
    /// This lets exact generated-repo lookups create lazy cells from resolver
    /// state before falling back to the transitional process-global registry.
    #[allocative(skip)]
    bzlmod_runtime_cell_snapshot: Option<Arc<BzlmodRuntimeCellInstallSnapshot>>,
    /// Dynamically-added cells from extension execution (spoke repos, etc.)
    #[allocative(skip)]
    dynamic_cells: RwLock<HashMap<CellName, DynamicCellInstance>>,
    #[allocative(visit = crate::cells::sequence_trie_allocative::visit_sequence_trie)]
    path_mappings: SequenceTrie<FileNameBuf, CellName>,
    root_cell: CellName,
    root_cell_alias_resolver: CellAliasResolver,
    resolve_root_alias_cell_names: bool,
    /// Absolute project root, when this resolver was built for a bzlmod workspace.
    /// Lets resolver-owned runtime-cell creation install `external/` symlinks without
    /// reading the process-global `dynamic_project_root()` (Plan 61 item 5). `None`
    /// for non-bzlmod / test resolvers, which fall back to the global helper.
    project_root: Option<std::path::PathBuf>,
}

#[derive(Debug)]
enum DynamicCellInstance {
    /// Cells discovered from the transitional process-global registries must
    /// stay scoped to the root that published those registries.
    #[cfg(test)]
    RootScoped(DynamicBzlmodEntry<&'static CellInstance>),
    /// Cells created directly from this resolver's bzlmod graph snapshot are
    /// owned by the resolver and should not depend on process-global root
    /// adapters after promotion.
    GraphOwned(&'static CellInstance),
}

#[derive(Clone, Copy)]
enum DynamicCellPathKind {
    GraphOwned,
    RootScoped,
}

impl DynamicCellInstance {
    #[cfg(test)]
    fn root_scoped(instance: CellInstance) -> Self {
        Self::RootScoped(dynamic_bzlmod_entry(Box::leak(Box::new(instance))))
    }

    fn graph_owned(instance: CellInstance) -> Self {
        Self::GraphOwned(Box::leak(Box::new(instance)))
    }

    fn graph_owned_instance(&self) -> Option<&'static CellInstance> {
        match self {
            Self::GraphOwned(instance) => Some(*instance),
            #[cfg(test)]
            Self::RootScoped(_) => None,
        }
    }

    fn root_scoped_instance_for_current_context(&self) -> Option<&'static CellInstance> {
        match self {
            #[cfg(test)]
            Self::RootScoped(entry) => dynamic_bzlmod_value_for_current_scope(entry),
            Self::GraphOwned(_) => None,
        }
    }

    fn instance_for_current_context(&self) -> Option<&'static CellInstance> {
        self.graph_owned_instance()
            .or_else(|| self.root_scoped_instance_for_current_context())
    }
}

impl CellResolver {
    pub fn new(
        cells: Vec<CellInstance>,
        root_cell_alias_resolver: CellAliasResolver,
    ) -> slug_error::Result<CellResolver> {
        Self::new_with_root_alias_cell_lookup(cells, root_cell_alias_resolver, true, None, None)
    }

    pub fn new_without_root_alias_cell_lookup(
        cells: Vec<CellInstance>,
        root_cell_alias_resolver: CellAliasResolver,
    ) -> slug_error::Result<CellResolver> {
        Self::new_with_root_alias_cell_lookup(cells, root_cell_alias_resolver, false, None, None)
    }

    pub fn new_bzlmod_with_runtime_cell_snapshot(
        cells: Vec<CellInstance>,
        root_cell_alias_resolver: CellAliasResolver,
        runtime_cell_snapshot: BzlmodRuntimeCellInstallSnapshot,
    ) -> slug_error::Result<CellResolver> {
        Self::new_with_root_alias_cell_lookup(
            cells,
            root_cell_alias_resolver,
            false,
            Some(Arc::new(runtime_cell_snapshot)),
            None,
        )
    }

    /// Production bzlmod constructor that records the absolute project root so
    /// resolver-owned runtime-cell creation installs `external/` symlinks without
    /// reading the process-global `dynamic_project_root()` (Plan 61 item 5).
    pub fn new_bzlmod_with_runtime_cell_snapshot_and_project_root(
        cells: Vec<CellInstance>,
        root_cell_alias_resolver: CellAliasResolver,
        runtime_cell_snapshot: BzlmodRuntimeCellInstallSnapshot,
        project_root: Option<std::path::PathBuf>,
    ) -> slug_error::Result<CellResolver> {
        Self::new_with_root_alias_cell_lookup(
            cells,
            root_cell_alias_resolver,
            false,
            Some(Arc::new(runtime_cell_snapshot)),
            project_root,
        )
    }

    fn new_with_root_alias_cell_lookup(
        cells: Vec<CellInstance>,
        root_cell_alias_resolver: CellAliasResolver,
        resolve_root_alias_cell_names: bool,
        bzlmod_runtime_cell_snapshot: Option<Arc<BzlmodRuntimeCellInstallSnapshot>>,
        project_root: Option<std::path::PathBuf>,
    ) -> slug_error::Result<CellResolver> {
        let input_cell_count = cells.len();
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
        slug_util::memory_checkpoint::checkpoint(
            "cell_resolver_new",
            [
                ("input_cells", input_cell_count),
                ("cells", cells_map.len()),
                ("root_aliases", root_cell_alias_resolver.aliases.len()),
            ],
        );
        Ok(CellResolver(Arc::new(CellResolverInternals {
            cells: cells_map,
            bzlmod_runtime_cell_snapshot,
            dynamic_cells: RwLock::new(HashMap::new()),
            root_cell,
            path_mappings,
            root_cell_alias_resolver,
            resolve_root_alias_cell_names,
            project_root,
        })))
    }

    /// Get a `Cell` from the `CellMap`
    pub fn get(&self, cell: CellName) -> slug_error::Result<&CellInstance> {
        if let Some(instance) = self.0.cells.get(&cell) {
            if let Some(crate::cells::external::ExternalCellOrigin::ExtensionRepo(static_setup)) =
                instance.external()
                && let Ok(dynamic) = self.0.dynamic_cells.read()
                && let Some(dynamic_instance) = dynamic
                    .get(&cell)
                    .and_then(|entry| self.dynamic_cell_instance_for_lookup(entry))
                && matches!(
                    dynamic_instance.external(),
                    Some(crate::cells::external::ExternalCellOrigin::ExtensionRepo(
                        dynamic_setup
                    )) if dynamic_setup != static_setup
                )
            {
                drop(dynamic);
                return self.get_or_create_dynamic_cell(cell);
            }
            return Ok(instance);
        }

        // Check if this name is an alias for an existing cell.
        // This prevents creating duplicate dynamic cells when a pre-computed
        // extension repo cell exists under a canonical name (e.g.,
        // "rules_rs+crate+crates__typenum-1.19.0") but is referenced by its
        // apparent name ("crates__typenum-1.19.0").
        if self.0.resolve_root_alias_cell_names {
            if let Some(aliased) = self
                .0
                .root_cell_alias_resolver
                .resolve_declared_or_runtime_alias(cell.as_str())
            {
                if aliased != cell {
                    if let Some(instance) = self.0.cells.get(&aliased) {
                        return Ok(instance);
                    }
                }
            }
        }

        // Check dynamic cells from extension execution.
        // If found, promote to "static" by leaking the reference (safe: cells live for
        // the duration of the build). This avoids holding the RwLock across returns.
        if let Ok(dynamic) = self.0.dynamic_cells.read() {
            if dynamic
                .get(&cell)
                .and_then(|entry| self.dynamic_cell_instance_for_lookup(entry))
                .is_some()
            {
                // Drop the read lock, get a write lock, and leak a reference
                drop(dynamic);
                return self.get_or_create_dynamic_cell(cell);
            }
        }

        if let Some(runtime_cell) = self.bzlmod_runtime_extension_cell_for_name(cell.as_str()) {
            return self.get_or_create_bzlmod_runtime_cell(cell, runtime_cell);
        }

        if self.0.bzlmod_runtime_cell_snapshot.is_some() {
            if !cell.as_str().ends_with('+')
                && !cell.as_str().contains('+')
                && let Ok(canonical_module_cell) =
                    CellName::unchecked_new(&format!("{}+", cell.as_str()))
                && let Some(instance) = self.0.cells.get(&canonical_module_cell)
            {
                return Ok(instance);
            }
            if let Some(instance) = self.bzlmod_static_module_cell_for_canonical_name(cell) {
                return Ok(instance);
            }
            return Err(slug_error::Error::from(CellError::UnknownCellName(
                cell,
                self.0.cells.keys().copied().collect(),
            )));
        }

        #[cfg(test)]
        {
            // Check global dynamic registry (populated by extension execution).
            // Resolve through the canonical helper so apparent-name/suffix fallback is
            // deterministic and respects known module-cell collisions.
            let dynamic_lookup = canonical_test_dynamic_extension_cell_name(cell.as_str())
                .and_then(|canonical| {
                    get_test_dynamic_extension_cell(&canonical).map(|path| (canonical, path))
                });
            if let Some((canonical, path)) = dynamic_lookup {
                // Auto-register this cell
                if let Ok(rel_path) = ProjectRelativePath::new(&path) {
                    let cell_path = CellRootPathBuf::new(rel_path.to_owned());
                    let nested = nested::NestedCells::from_cell_roots(&[], &*cell_path);
                    // Plan 36: if the canonical name has a registered
                    // ExtensionRepoCellSetup, attach it as the external origin
                    // so file ops route through the lazy-materialization path.
                    let external = get_test_dynamic_extension_cell_setup(&canonical)
                        .map(crate::cells::external::ExternalCellOrigin::ExtensionRepo);
                    if let Ok(instance) = CellInstance::new(cell, cell_path, external, nested) {
                        // Create external/ symlink for action execution
                        ensure_external_symlink(cell.as_str(), &path);
                        if let Ok(mut dynamic) = self.0.dynamic_cells.write() {
                            dynamic.insert(cell, DynamicCellInstance::root_scoped(instance));
                        }
                        return self.get_or_create_dynamic_cell(cell);
                    }
                }
            }

            // Last resort: scan bazel-external/ for a directory matching *+{cell_name}
            // This handles spoke repos from extensions that may not be in the dynamic
            // registry yet (e.g., the first time an extension is triggered).
            // Use the root cell's path to determine the project root directory.
            if dynamic_bzlmod_directory_scan_allowed() {
                let cell_str = cell.as_str();
                let bazel_ext_dir = dynamic_project_root()
                    .map(|root| root.join("bazel-external"))
                    .unwrap_or_else(|| std::path::PathBuf::from("bazel-external"));
                let exact_path = bazel_ext_dir.join(cell_str);
                if exact_path.is_dir() {
                    let path = format!("bazel-external/{cell_str}");
                    if let Ok(rel_path) = ProjectRelativePath::new(&path) {
                        let cell_path = CellRootPathBuf::new(rel_path.to_owned());
                        let nested = nested::NestedCells::from_cell_roots(&[], &*cell_path);
                        if let Ok(instance) = CellInstance::new(cell, cell_path, None, nested) {
                            register_test_dynamic_extension_cell(cell_str.to_owned(), path);
                            if let Ok(mut dynamic) = self.0.dynamic_cells.write() {
                                dynamic.insert(cell, DynamicCellInstance::root_scoped(instance));
                            }
                            return self.get_or_create_dynamic_cell(cell);
                        }
                    }
                }

                if let Some(canonical) =
                    scan_dynamic_extension_suffix_from_external_dir_uncached(cell_str)
                {
                    let path = format!("bazel-external/{canonical}");
                    if let Ok(rel_path) = ProjectRelativePath::new(&path) {
                        let cell_path = CellRootPathBuf::new(rel_path.to_owned());
                        let nested = nested::NestedCells::from_cell_roots(&[], &*cell_path);
                        if let Ok(instance) = CellInstance::new(cell, cell_path, None, nested) {
                            // Also register in dynamic registry for future lookups
                            register_test_dynamic_extension_cell(canonical, path);
                            if let Ok(mut dynamic) = self.0.dynamic_cells.write() {
                                dynamic.insert(cell, DynamicCellInstance::root_scoped(instance));
                            }
                            return self.get_or_create_dynamic_cell(cell);
                        }
                    }
                }
            }
        }

        Err(slug_error::Error::from(CellError::UnknownCellName(
            cell,
            self.0.cells.keys().copied().collect(),
        )))
    }

    /// Helper to get a reference to a dynamic cell.
    ///
    /// Dynamic cells are discovered lazily while other async computations may
    /// still hold references to earlier dynamic cells. Store leaked
    /// `CellInstance`s in the dynamic map so HashMap reallocation cannot
    /// invalidate returned references.
    fn get_or_create_dynamic_cell(&self, cell: CellName) -> slug_error::Result<&CellInstance> {
        let dynamic = self.0.dynamic_cells.read().map_err(|_| {
            CellError::UnknownCellName(cell, self.0.cells.keys().copied().collect())
        })?;
        if let Some(instance) = dynamic
            .get(&cell)
            .and_then(|entry| self.dynamic_cell_instance_for_lookup(entry))
        {
            Ok(instance)
        } else {
            Err(slug_error::Error::from(CellError::UnknownCellName(
                cell,
                self.0.cells.keys().copied().collect(),
            )))
        }
    }

    fn dynamic_cell_instance_for_lookup(
        &self,
        entry: &DynamicCellInstance,
    ) -> Option<&'static CellInstance> {
        if self.0.bzlmod_runtime_cell_snapshot.is_some() {
            entry.graph_owned_instance()
        } else {
            entry.instance_for_current_context()
        }
    }

    fn bzlmod_runtime_extension_cell_for_name(
        &self,
        name: &str,
    ) -> Option<BzlmodRuntimeExtensionCell> {
        self.0
            .bzlmod_runtime_cell_snapshot
            .as_ref()?
            .extension_cells
            .iter()
            .find(|cell| cell.canonical_name == name)
            .cloned()
    }

    fn bzlmod_static_module_cell_for_canonical_name(
        &self,
        cell: CellName,
    ) -> Option<&CellInstance> {
        let canonical_name = cell.as_str();
        let module_name = canonical_name.strip_suffix('+')?;
        if module_name.is_empty() {
            return None;
        }

        let module_cell = CellName::unchecked_new(module_name).ok()?;
        let instance = self.0.cells.get(&module_cell)?;
        let cell_root = instance.path().as_project_relative_path().as_str();
        let external_repo = cell_root
            .strip_prefix("bazel-external/")?
            .split('/')
            .next()?;
        (external_repo == canonical_name).then_some(instance)
    }

    /// Install the `external/<cell>` symlink for a resolver-owned runtime cell.
    /// Uses the resolver's recorded project root (Plan 61 item 5) so production
    /// runtime-cell creation does not read the process-global `dynamic_project_root()`;
    /// resolvers without a recorded root (non-bzlmod / test) fall back to the global.
    fn install_runtime_cell_symlink(&self, cell_name: &str, cell_path: &str) {
        match &self.0.project_root {
            Some(project_root) => {
                ensure_external_symlink_with_root(project_root.clone(), cell_name, cell_path)
            }
            None => ensure_external_symlink(cell_name, cell_path),
        }
    }

    fn get_or_create_bzlmod_runtime_cell(
        &self,
        cell: CellName,
        runtime_cell: BzlmodRuntimeExtensionCell,
    ) -> slug_error::Result<&CellInstance> {
        let rel_path = ProjectRelativePath::new(&runtime_cell.path)?;
        let cell_path = CellRootPathBuf::new(rel_path.to_owned());
        let nested = nested::NestedCells::from_cell_roots(&[], &*cell_path);
        let external = Some(crate::cells::external::ExternalCellOrigin::ExtensionRepo(
            runtime_cell.setup.dupe(),
        ));
        let instance = CellInstance::new(cell, cell_path, external, nested)?;
        self.install_runtime_cell_symlink(cell.as_str(), &runtime_cell.path);
        if let Ok(mut dynamic) = self.0.dynamic_cells.write() {
            dynamic.insert(cell, DynamicCellInstance::graph_owned(instance));
        }
        self.get_or_create_dynamic_cell(cell)
    }

    /// Register a bzlmod extension repo cell on this resolver only.
    ///
    /// This is used for sibling spokes discovered from the current DICE
    /// extension-spoke value. Unlike the transitional process-global dynamic
    /// registry, the promoted cell is owned by this resolver and survives
    /// process-global root changes without leaking into other resolvers.
    pub fn register_bzlmod_runtime_extension_cell(
        &self,
        name: &str,
        path: &str,
        setup: crate::cells::external::ExtensionRepoCellSetup,
    ) -> slug_error::Result<()> {
        let cell = CellName::unchecked_new(name)?;
        let internal_alias = CellName::unchecked_new(setup.internal_name.as_ref()).ok();
        if let Some(existing) = self.0.cells.get(&cell) {
            if let Some(crate::cells::external::ExternalCellOrigin::ExtensionRepo(existing_setup)) =
                existing.external()
            {
                if existing_setup == &setup {
                    return Ok(());
                }
            } else {
                return Ok(());
            }
        }
        if let Some(existing) = self.0.dynamic_cells.read().ok().and_then(|dynamic| {
            dynamic
                .get(&cell)
                .and_then(|entry| self.dynamic_cell_instance_for_lookup(entry))
                .cloned()
        }) {
            if let Some(crate::cells::external::ExternalCellOrigin::ExtensionRepo(existing_setup)) =
                existing.external()
            {
                if existing_setup == &setup {
                    return Ok(());
                }
            } else {
                return Ok(());
            }
        }

        let rel_path = ProjectRelativePath::new(path)?;
        let cell_path = CellRootPathBuf::new(rel_path.to_owned());
        let nested = nested::NestedCells::from_cell_roots(&[], &*cell_path);
        let external = Some(crate::cells::external::ExternalCellOrigin::ExtensionRepo(
            setup.clone(),
        ));
        let instance = CellInstance::new(cell, cell_path, external, nested)?;
        self.install_runtime_cell_symlink(cell.as_str(), path);
        let mut dynamic = self.0.dynamic_cells.write().map_err(|_| {
            slug_error::slug_error!(
                slug_error::ErrorTag::Tier0,
                "failed to register bzlmod runtime extension cell '{}': dynamic cell lock poisoned",
                cell.as_str()
            )
        })?;
        dynamic.insert(cell, DynamicCellInstance::graph_owned(instance.dupe()));
        if let Some(internal_alias) = internal_alias
            && internal_alias != cell
            && !self.0.cells.contains_key(&internal_alias)
        {
            let should_insert_alias = match dynamic
                .get(&internal_alias)
                .and_then(|entry| self.dynamic_cell_instance_for_lookup(entry))
            {
                Some(existing) => matches!(
                    existing.external(),
                    Some(crate::cells::external::ExternalCellOrigin::ExtensionRepo(
                        existing_setup
                    )) if existing_setup != &setup
                ),
                None => true,
            };
            if should_insert_alias {
                dynamic.insert(internal_alias, DynamicCellInstance::graph_owned(instance));
            }
        }
        Ok(())
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

    pub fn project_root(&self) -> Option<&std::path::Path> {
        self.0.project_root.as_deref()
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
        if let Some(cell_path) = self.best_dynamic_cell_path(path, DynamicCellPathKind::GraphOwned)
        {
            return cell_path;
        }
        if let Some(cell_path) = self.get_bzlmod_runtime_cell_path(path) {
            return cell_path;
        }
        if self.0.bzlmod_runtime_cell_snapshot.is_none() {
            if let Some(cell_path) =
                self.best_dynamic_cell_path(path, DynamicCellPathKind::RootScoped)
            {
                return cell_path;
            }
        }
        let cell = self.find(path);
        // Both of these unwraps are ok by construction of the `CellResolver`
        let instance = self.get(cell).unwrap();
        let relative = path
            .strip_prefix(instance.path().as_project_relative_path())
            .unwrap();
        CellPath::new(cell, relative.to_owned().into())
    }

    fn best_dynamic_cell_path(
        &self,
        path: &ProjectRelativePath,
        kind: DynamicCellPathKind,
    ) -> Option<CellPath> {
        let dynamic_cells = self.0.dynamic_cells.read().ok()?;
        let mut best_dynamic: Option<(usize, CellPath)> = None;
        for (cell, instance) in dynamic_cells.iter().filter_map(|(cell, entry)| {
            let instance = match kind {
                DynamicCellPathKind::GraphOwned => entry.graph_owned_instance(),
                DynamicCellPathKind::RootScoped => entry.root_scoped_instance_for_current_context(),
            }?;
            Some((cell, instance))
        }) {
            let cell_root = instance.path().as_project_relative_path();
            let Some(relative) = path.strip_prefix_opt(cell_root) else {
                continue;
            };
            let depth = cell_root.iter().count();
            if best_dynamic
                .as_ref()
                .is_none_or(|(best_depth, _)| depth > *best_depth)
            {
                best_dynamic = Some((depth, CellPath::new(*cell, relative.to_owned().into())));
            }
        }
        best_dynamic.map(|(_, cell_path)| cell_path)
    }

    pub fn get_cell_path_from_abs_path(
        &self,
        path: &AbsPath,
        fs: &ProjectRoot,
    ) -> slug_error::Result<CellPath> {
        Ok(self.get_cell_path(&fs.relativize_any(path)?))
    }

    fn get_bzlmod_runtime_cell_path(&self, path: &ProjectRelativePath) -> Option<CellPath> {
        let snapshot = self.0.bzlmod_runtime_cell_snapshot.as_ref()?;
        let mut best_runtime: Option<(usize, CellName, ForwardRelativePathBuf)> = None;
        for runtime_cell in snapshot.extension_cells.iter() {
            let Ok(cell_root) = ProjectRelativePath::new(&runtime_cell.path) else {
                continue;
            };
            let Some(relative) = path.strip_prefix_opt(cell_root) else {
                continue;
            };
            let Ok(cell) = CellName::unchecked_new(&runtime_cell.canonical_name) else {
                continue;
            };
            let depth = cell_root.iter().count();
            if best_runtime
                .as_ref()
                .is_none_or(|(best_depth, _, _)| depth > *best_depth)
            {
                best_runtime = Some((depth, cell, relative.to_owned()));
            }
        }
        let (_, cell, relative) = best_runtime?;
        self.get(cell).ok()?;
        Some(CellPath::new(cell, relative.into()))
    }

    pub fn cells(&self) -> impl Iterator<Item = (CellName, &CellInstance)> {
        self.0
            .cells
            .iter()
            .map(|(name, instance)| (*name, instance))
    }

    /// Project-relative repo paths that bzlmod label resolution can learn from
    /// this resolver's graph snapshot without consulting process-global dynamic
    /// cell maps.
    pub fn bzlmod_label_cell_paths(&self) -> Vec<(String, String)> {
        self.bzlmod_label_cell_paths_for_owner(None)
    }

    /// Project-relative repo paths visible from a bzlmod owner module. This
    /// includes module-scoped aliases such as non-root `use_repo_rule()`
    /// repositories that module extensions can reference through Label().
    pub fn bzlmod_label_cell_paths_for_owner(
        &self,
        owner_module: Option<&str>,
    ) -> Vec<(String, String)> {
        let mut paths = BTreeMap::new();
        if let Some(snapshot) = self.0.bzlmod_runtime_cell_snapshot.as_ref() {
            for runtime_cell in snapshot.extension_cells.iter() {
                paths
                    .entry(runtime_cell.canonical_name.clone())
                    .or_insert_with(|| runtime_cell.path.clone());
            }
            for alias in snapshot.dynamic_aliases.iter() {
                if let Some(path) = self.bzlmod_label_path_for_cell(&alias.canonical_name) {
                    paths.entry(alias.apparent_name.clone()).or_insert(path);
                }
            }
            if let Some(owner_module) = owner_module {
                for alias in snapshot
                    .scoped_aliases
                    .iter()
                    .filter(|alias| bzlmod_owner_module_matches(&alias.owner_module, owner_module))
                {
                    if let Some(path) = self.bzlmod_label_path_for_cell(&alias.target_name) {
                        paths.entry(alias.apparent_name.clone()).or_insert(path);
                    }
                }
            }
        }
        if let Ok(dynamic_cells) = self.0.dynamic_cells.read() {
            for (cell, entry) in dynamic_cells.iter() {
                if let Some(instance) = entry.graph_owned_instance() {
                    paths
                        .entry(cell.as_str().to_owned())
                        .or_insert_with(|| instance.path().as_str().to_owned());
                }
            }
        }
        for (alias, target) in self.0.root_cell_alias_resolver.mappings() {
            if let Some(path) = self.bzlmod_label_path_for_cell(target.as_str()) {
                paths.entry(alias.as_str().to_owned()).or_insert(path);
            }
        }
        paths.into_iter().collect()
    }

    pub fn bzlmod_same_extension_internal_aliases(
        &self,
        current_cell: &str,
    ) -> Vec<(NonEmptyCellAlias, CellName)> {
        let Some(prefix) = bzlmod_extension_repo_prefix(current_cell) else {
            return Vec::new();
        };
        let Ok(dynamic_cells) = self.0.dynamic_cells.read() else {
            return Vec::new();
        };
        dynamic_cells
            .iter()
            .filter_map(|(alias, entry)| {
                let instance = entry.graph_owned_instance()?;
                let canonical = instance.name();
                if alias.as_str() == canonical.as_str() || !canonical.as_str().starts_with(&prefix)
                {
                    return None;
                }
                NonEmptyCellAlias::new(alias.as_str().to_owned())
                    .ok()
                    .map(|alias| (alias, canonical))
            })
            .collect()
    }

    fn bzlmod_label_path_for_cell(&self, cell_name: &str) -> Option<String> {
        let cell = CellName::unchecked_new(cell_name).ok()?;
        self.0
            .cells
            .get(&cell)
            .map(|instance| instance.path().as_str().to_owned())
            .or_else(|| {
                self.bzlmod_runtime_extension_cell_for_name(cell_name)
                    .map(|cell| cell.path)
            })
    }

    /// Resolves a given 'Package' to the 'ProjectRelativePath' that points to
    /// the 'Package'
    ///
    /// ```
    /// use std::convert::TryFrom;
    ///
    /// use slug_core::cells::CellResolver;
    /// use slug_core::cells::cell_path::CellPath;
    /// use slug_core::cells::cell_root_path::CellRootPathBuf;
    /// use slug_core::cells::name::CellName;
    /// use slug_core::cells::paths::CellRelativePathBuf;
    /// use slug_core::fs::project_rel_path::ProjectRelativePath;
    /// use slug_core::fs::project_rel_path::ProjectRelativePathBuf;
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
    /// # slug_error::Ok(())
    /// ```
    pub fn resolve_path(
        &self,
        cell_path: CellPathRef,
    ) -> slug_error::Result<ProjectRelativePathBuf> {
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
    use slug_fs::paths::forward_rel_path::ForwardRelativePath;
    use slug_fs::paths::forward_rel_path::ForwardRelativePathBuf;

    use super::*;
    use crate::cells::cell_root_path::CellRootPath;

    #[test]
    fn test_of_names_and_paths() -> slug_error::Result<()> {
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
    fn test_cells() -> slug_error::Result<()> {
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

    #[test]
    fn get_cell_path_prefers_dynamic_extension_cell_over_root() -> slug_error::Result<()> {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir()?;
        reset_dynamic_bzlmod_state_for_project_root(tmp.path().to_path_buf());
        let canonical = "dynamic_owner++ext+generated_repo";
        let dynamic_path = format!("bazel-external/{canonical}");
        register_test_dynamic_extension_cell(canonical.to_owned(), dynamic_path.clone());

        let cells = CellResolver::testing_with_names_and_paths(&[(
            CellName::testing_new("root"),
            CellRootPathBuf::testing_new(""),
        )]);
        let dynamic_cell = CellName::testing_new(canonical);
        cells.get(dynamic_cell)?;

        assert_eq!(
            cells.get_cell_path(ProjectRelativePath::new(&format!(
                "{dynamic_path}/defs.bzl"
            ))?),
            CellPath::new(
                dynamic_cell,
                ForwardRelativePathBuf::unchecked_new("defs.bzl".to_owned()).into()
            )
        );

        Ok(())
    }

    #[test]
    fn cell_resolver_discovers_exact_dynamic_extension_repo_dir() -> slug_error::Result<()> {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir()?;
        std::fs::create_dir_all(tmp.path().join("bazel-external/exact_owner++ext+generated"))?;
        set_dynamic_project_root(tmp.path().to_path_buf());

        let cells = CellResolver::testing_with_names_and_paths(&[(
            CellName::testing_new("root"),
            CellRootPathBuf::testing_new(""),
        )]);
        let dynamic_cell = CellName::testing_new("exact_owner++ext+generated");

        assert_eq!(cells.get(dynamic_cell)?.name(), dynamic_cell);

        Ok(())
    }

    #[test]
    fn bzlmod_resolver_runtime_cell_symlink_uses_recorded_project_root_not_global()
    -> slug_error::Result<()> {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let resolver_root = tempfile::tempdir()?;
        let global_root = tempfile::tempdir()?;
        // Point the process-global project root somewhere ELSE to prove the resolver
        // does not consult it when installing the runtime-cell symlink (Plan 61 item 5).
        reset_dynamic_bzlmod_state_for_project_root(global_root.path().to_path_buf());

        let root = CellName::testing_new("root");
        let root_path = CellRootPathBuf::testing_new("");
        let root_instance = CellInstance::new(
            root,
            root_path.clone(),
            None,
            NestedCells::from_cell_roots(&[(root, root_path.as_path())], &root_path),
        )?;
        let canonical = "owner++ext+generated";
        let setup = crate::cells::external::ExtensionRepoCellSetup {
            canonical_name: Arc::from(canonical),
            extension_id: Arc::from("@owner//:ext.bzl%ext"),
            internal_name: Arc::from("generated"),
            spec_hash: Arc::from("sha256-test"),
            repo_spec_json: Arc::from("{}"),
            repo_env_json: Arc::from("{}"),
            extension_usages_digest: Arc::from(""),
            extension_replay_inputs_identity_digest: Arc::from(""),
            extension_repo_mappings_digest: Arc::from(""),
            extension_repo_mapping_overrides_digest: Arc::from(""),
            extension_bzl_transitive_digest: Arc::from(""),
            extension_recorded_inputs_json: Arc::from(""),
            materialized: false,
        };
        let snapshot = BzlmodRuntimeCellInstallSnapshot {
            root_module_name: None,
            extension_cells: vec![BzlmodRuntimeExtensionCell {
                canonical_name: canonical.to_owned(),
                internal_name: "generated".to_owned(),
                path: format!("bazel-external/{canonical}"),
                setup,
            }],
            scoped_aliases: Vec::new(),
            dynamic_aliases: Vec::new(),
        };
        let root_aliases = CellAliasResolver::new_bzlmod_with_runtime_cell_snapshot(
            root,
            HashMap::new(),
            &snapshot,
        )?;
        let resolver = CellResolver::new_bzlmod_with_runtime_cell_snapshot_and_project_root(
            vec![root_instance],
            root_aliases,
            snapshot,
            Some(resolver_root.path().to_path_buf()),
        )?;

        // Looking up the runtime extension cell triggers symlink installation.
        let _ = resolver.get(CellName::testing_new(canonical))?;

        assert!(
            resolver_root
                .path()
                .join("external")
                .join(canonical)
                .symlink_metadata()
                .is_ok(),
            "external/ symlink should be installed under the resolver's recorded project root"
        );
        assert!(
            global_root
                .path()
                .join("external")
                .join(canonical)
                .symlink_metadata()
                .is_err(),
            "external/ symlink must NOT be installed under the process-global project root"
        );
        Ok(())
    }

    #[test]
    fn bzlmod_resolver_uses_runtime_snapshot_for_lazy_extension_cell() -> slug_error::Result<()> {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir()?;
        reset_dynamic_bzlmod_state_for_project_root(tmp.path().to_path_buf());

        let root = CellName::testing_new("root");
        let root_path = CellRootPathBuf::testing_new("");
        let root_instance = CellInstance::new(
            root,
            root_path.clone(),
            None,
            NestedCells::from_cell_roots(&[(root, root_path.as_path())], &root_path),
        )?;
        let canonical = "owner++ext+generated";
        let sibling = "owner++ext+sibling";
        let setup = crate::cells::external::ExtensionRepoCellSetup {
            canonical_name: Arc::from(canonical),
            extension_id: Arc::from("@owner//:ext.bzl%ext"),
            internal_name: Arc::from("generated"),
            spec_hash: Arc::from("sha256-test"),
            repo_spec_json: Arc::from("{}"),
            repo_env_json: Arc::from("{}"),
            extension_usages_digest: Arc::from(""),
            extension_replay_inputs_identity_digest: Arc::from(""),
            extension_repo_mappings_digest: Arc::from(""),
            extension_repo_mapping_overrides_digest: Arc::from(""),
            extension_bzl_transitive_digest: Arc::from(""),
            extension_recorded_inputs_json: Arc::from(""),
            materialized: false,
        };
        let sibling_setup = crate::cells::external::ExtensionRepoCellSetup {
            canonical_name: Arc::from(sibling),
            extension_id: Arc::from("@owner//:ext.bzl%ext"),
            internal_name: Arc::from("sibling"),
            spec_hash: Arc::from("sha256-sibling"),
            repo_spec_json: Arc::from("{}"),
            repo_env_json: Arc::from("{}"),
            extension_usages_digest: Arc::from(""),
            extension_replay_inputs_identity_digest: Arc::from(""),
            extension_repo_mappings_digest: Arc::from(""),
            extension_repo_mapping_overrides_digest: Arc::from(""),
            extension_bzl_transitive_digest: Arc::from(""),
            extension_recorded_inputs_json: Arc::from(""),
            materialized: false,
        };
        let snapshot = BzlmodRuntimeCellInstallSnapshot {
            root_module_name: None,
            extension_cells: vec![
                BzlmodRuntimeExtensionCell {
                    canonical_name: canonical.to_owned(),
                    internal_name: "generated".to_owned(),
                    path: format!("bazel-external/{canonical}"),
                    setup: setup.clone(),
                },
                BzlmodRuntimeExtensionCell {
                    canonical_name: sibling.to_owned(),
                    internal_name: "sibling".to_owned(),
                    path: format!("bazel-external/{sibling}"),
                    setup: sibling_setup,
                },
            ],
            scoped_aliases: vec![BzlmodRuntimeScopedRepoAlias {
                owner_module: "owner+".to_owned(),
                apparent_name: "helper".to_owned(),
                target_name: "dep+1.0".to_owned(),
            }],
            dynamic_aliases: vec![BzlmodRuntimeDynamicAlias {
                apparent_name: "generated_alias".to_owned(),
                canonical_name: canonical.to_owned(),
            }],
        };
        assert_eq!(get_test_dynamic_extension_cell(canonical), None);
        assert_eq!(
            resolve_test_dynamic_extension_cell_alias("generated_alias"),
            None
        );
        assert_eq!(
            resolve_test_scoped_bzlmod_repo_alias_for_current_cell("owner+", "helper"),
            None
        );

        let root_aliases = CellAliasResolver::new_bzlmod_with_runtime_cell_snapshot(
            root,
            HashMap::new(),
            &snapshot,
        )?;
        let owner_aliases = CellAliasResolver::new_bzlmod_with_runtime_cell_snapshot(
            CellName::testing_new("owner+"),
            HashMap::new(),
            &snapshot,
        )?;
        assert_eq!(
            root_aliases.resolve("generated_alias")?,
            CellName::testing_new(canonical)
        );
        assert_eq!(
            root_aliases.resolve("generated")?,
            CellName::testing_new(canonical)
        );
        assert_eq!(
            root_aliases.resolve("sibling")?,
            CellName::testing_new(sibling)
        );
        assert_eq!(
            owner_aliases.resolve("helper")?,
            CellName::testing_new("dep+1.0")
        );
        let generated_aliases = CellAliasResolver::new_bzlmod_with_runtime_cell_snapshot(
            CellName::testing_new(canonical),
            HashMap::new(),
            &snapshot,
        )?;
        assert_eq!(
            generated_aliases.resolve("sibling")?,
            CellName::testing_new(sibling)
        );
        assert_eq!(get_test_dynamic_extension_cell(sibling), None);

        let resolver = CellResolver::new_bzlmod_with_runtime_cell_snapshot(
            vec![root_instance],
            root_aliases,
            snapshot,
        )?;
        let cell_name = CellName::testing_new(canonical);
        assert_eq!(
            resolver.get_cell_path(ProjectRelativePath::new(&format!(
                "bazel-external/{canonical}/defs.bzl"
            ))?),
            CellPath::new(
                cell_name,
                ForwardRelativePathBuf::unchecked_new("defs.bzl".to_owned()).into()
            )
        );
        assert_eq!(get_test_dynamic_extension_cell(canonical), None);
        let cell = resolver.get(cell_name)?;

        assert_eq!(cell.name(), cell_name);
        assert_eq!(cell.path().as_str(), format!("bazel-external/{canonical}"));
        assert!(matches!(
            cell.external(),
            Some(crate::cells::external::ExternalCellOrigin::ExtensionRepo(origin))
                if origin == &setup
        ));
        assert_eq!(get_test_dynamic_extension_cell(canonical), None);
        assert!(resolver.get(CellName::testing_new("generated")).is_err());
        assert!(resolver.get(CellName::testing_new("sibling")).is_err());
        reset_dynamic_bzlmod_state_for_project_root(tmp.path().join("after"));
        assert_eq!(get_test_dynamic_extension_cell(canonical), None);
        assert_eq!(resolver.get(cell_name)?.name(), cell_name);
        assert_eq!(
            resolver.get_cell_path(ProjectRelativePath::new(&format!(
                "bazel-external/{canonical}/defs.bzl"
            ))?),
            CellPath::new(
                cell_name,
                ForwardRelativePathBuf::unchecked_new("defs.bzl".to_owned()).into()
            )
        );

        Ok(())
    }

    #[test]
    fn get_cell_path_prefers_runtime_snapshot_over_root_scoped_dynamic_cell()
    -> slug_error::Result<()> {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir()?;
        reset_dynamic_bzlmod_state_for_project_root(tmp.path().to_path_buf());

        let root = CellName::testing_new("root");
        let root_path = CellRootPathBuf::testing_new("");
        let root_instance = CellInstance::new(
            root,
            root_path.clone(),
            None,
            NestedCells::from_cell_roots(&[(root, root_path.as_path())], &root_path),
        )?;
        let canonical = "runtime_owner++ext+generated";
        let stale_global = "stale_owner++ext+generated";
        let runtime_path = format!("bazel-external/{canonical}");
        let setup = crate::cells::external::ExtensionRepoCellSetup {
            canonical_name: Arc::from(canonical),
            extension_id: Arc::from("@runtime_owner//:ext.bzl%ext"),
            internal_name: Arc::from("generated"),
            spec_hash: Arc::from("sha256-test"),
            repo_spec_json: Arc::from("{}"),
            repo_env_json: Arc::from("{}"),
            extension_usages_digest: Arc::from(""),
            extension_replay_inputs_identity_digest: Arc::from(""),
            extension_repo_mappings_digest: Arc::from(""),
            extension_repo_mapping_overrides_digest: Arc::from(""),
            extension_bzl_transitive_digest: Arc::from(""),
            extension_recorded_inputs_json: Arc::from(""),
            materialized: false,
        };
        let snapshot = BzlmodRuntimeCellInstallSnapshot {
            root_module_name: None,
            extension_cells: vec![BzlmodRuntimeExtensionCell {
                canonical_name: canonical.to_owned(),
                internal_name: "generated".to_owned(),
                path: runtime_path.clone(),
                setup,
            }],
            scoped_aliases: Vec::new(),
            dynamic_aliases: Vec::new(),
        };
        let root_aliases = CellAliasResolver::new_bzlmod_with_runtime_cell_snapshot(
            root,
            HashMap::new(),
            &snapshot,
        )?;
        let resolver = CellResolver::new_bzlmod_with_runtime_cell_snapshot(
            vec![root_instance],
            root_aliases,
            snapshot,
        )?;

        register_test_dynamic_extension_cell(stale_global.to_owned(), runtime_path.clone());
        let stale_cell = CellName::testing_new(stale_global);
        assert!(resolver.get(stale_cell).is_err());

        let runtime_cell = CellName::testing_new(canonical);
        assert_eq!(
            resolver.get_cell_path(ProjectRelativePath::new(&format!(
                "{runtime_path}/defs.bzl"
            ))?),
            CellPath::new(
                runtime_cell,
                ForwardRelativePathBuf::unchecked_new("defs.bzl".to_owned()).into()
            )
        );

        Ok(())
    }

    #[test]
    fn get_cell_path_with_runtime_snapshot_rejects_root_scoped_dynamic_cell_miss()
    -> slug_error::Result<()> {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir()?;
        reset_dynamic_bzlmod_state_for_project_root(tmp.path().to_path_buf());

        let root = CellName::testing_new("root");
        let root_path = CellRootPathBuf::testing_new("");
        let root_instance = CellInstance::new(
            root,
            root_path.clone(),
            None,
            NestedCells::from_cell_roots(&[(root, root_path.as_path())], &root_path),
        )?;
        let snapshot = BzlmodRuntimeCellInstallSnapshot::default();
        let root_aliases = CellAliasResolver::new_bzlmod_with_runtime_cell_snapshot(
            root,
            HashMap::new(),
            &snapshot,
        )?;
        let resolver = CellResolver::new_bzlmod_with_runtime_cell_snapshot(
            vec![root_instance],
            root_aliases,
            snapshot,
        )?;

        let stale_global = "stale_path_owner++ext+generated";
        let stale_path = format!("bazel-external/{stale_global}");
        let stale_cell = CellName::testing_new(stale_global);
        let stale_path_buf = CellRootPathBuf::testing_new(&stale_path);
        let stale_instance = CellInstance::new(
            stale_cell,
            stale_path_buf.clone(),
            None,
            NestedCells::from_cell_roots(
                &[(stale_cell, stale_path_buf.as_path())],
                &stale_path_buf,
            ),
        )?;
        resolver
            .0
            .dynamic_cells
            .write()
            .unwrap()
            .insert(stale_cell, DynamicCellInstance::root_scoped(stale_instance));

        assert_eq!(
            resolver.get_cell_path(ProjectRelativePath::new(&format!("{stale_path}/defs.bzl"))?),
            CellPath::new(
                root,
                ForwardRelativePathBuf::unchecked_new(format!("{stale_path}/defs.bzl")).into()
            )
        );
        assert!(resolver.get(stale_cell).is_err());

        Ok(())
    }

    #[test]
    fn bzlmod_runtime_snapshot_rejects_process_global_alias_misses() -> slug_error::Result<()> {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir()?;
        reset_dynamic_bzlmod_state_for_project_root(tmp.path().to_path_buf());

        let apparent = "runtime_missing_alias";
        let wrong_global = "wrong_owner++ext+generated";
        register_test_dynamic_extension_cell_alias(apparent.to_owned(), wrong_global.to_owned());
        register_test_dynamic_extension_cell(
            wrong_global.to_owned(),
            format!("bazel-external/{wrong_global}"),
        );
        register_test_scoped_bzlmod_repo_alias(
            "root".to_owned(),
            apparent.to_owned(),
            wrong_global.to_owned(),
        );
        let snapshot = BzlmodRuntimeCellInstallSnapshot::default();
        let root_aliases = CellAliasResolver::new_bzlmod_with_runtime_cell_snapshot(
            CellName::testing_new("root"),
            HashMap::new(),
            &snapshot,
        )?;

        assert_eq!(
            resolve_test_dynamic_extension_cell_alias(apparent).as_deref(),
            Some(wrong_global)
        );
        assert_eq!(
            resolve_test_scoped_bzlmod_repo_alias_for_current_cell("root", apparent).as_deref(),
            Some(wrong_global)
        );
        assert!(root_aliases.resolve(apparent).is_err());

        let root = CellName::testing_new("root");
        let root_path = CellRootPathBuf::testing_new("");
        let root_instance = CellInstance::new(
            root,
            root_path.clone(),
            None,
            NestedCells::from_cell_roots(&[(root, root_path.as_path())], &root_path),
        )?;
        let resolver = CellResolver::new_bzlmod_with_runtime_cell_snapshot(
            vec![root_instance],
            root_aliases,
            snapshot,
        )?;
        assert!(resolver.get(CellName::testing_new(wrong_global)).is_err());

        Ok(())
    }

    #[test]
    fn cell_resolver_get_no_snapshot_alias_miss_ignores_process_global_alias()
    -> slug_error::Result<()> {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir()?;
        reset_dynamic_bzlmod_state_for_project_root(tmp.path().to_path_buf());

        let root = CellName::testing_new("root");
        let canonical = CellName::testing_new("plan61_get_canonical_cell");
        let apparent = CellName::testing_new("plan61_get_apparent_alias");
        let wrong_global = CellName::testing_new("plan61_wrong_owner++get+alias");
        register_test_dynamic_extension_cell_alias(
            apparent.as_str().to_owned(),
            wrong_global.as_str().to_owned(),
        );
        let root_path = CellRootPathBuf::testing_new("");
        let canonical_path =
            CellRootPathBuf::testing_new("bazel-external/plan61_get_canonical_cell");
        let cell_roots = [
            (root, root_path.as_path()),
            (canonical, canonical_path.as_path()),
        ];
        let root_instance = CellInstance::new(
            root,
            root_path.clone(),
            None,
            NestedCells::from_cell_roots(&cell_roots, &root_path),
        )?;
        let canonical_instance = CellInstance::new(
            canonical,
            canonical_path.clone(),
            None,
            NestedCells::from_cell_roots(&cell_roots, cell_roots[1].1),
        )?;
        let mut aliases = HashMap::new();
        aliases.insert(
            NonEmptyCellAlias::new("declared_alias".to_owned())?,
            canonical,
        );
        let root_aliases = CellAliasResolver::new(root, aliases)?;
        let resolver = CellResolver::new(vec![root_instance, canonical_instance], root_aliases)?;

        assert_eq!(
            resolver
                .get(CellName::testing_new("declared_alias"))?
                .name(),
            canonical
        );
        assert!(resolver.get(apparent).is_err());
        assert_eq!(
            resolve_test_dynamic_extension_cell_alias(apparent.as_str()).as_deref(),
            Some(wrong_global.as_str())
        );

        Ok(())
    }

    #[test]
    fn bzlmod_runtime_snapshot_resolves_extension_owner_self_alias() -> slug_error::Result<()> {
        let snapshot = BzlmodRuntimeCellInstallSnapshot::default();
        let aliases = CellAliasResolver::new_bzlmod_with_runtime_cell_snapshot(
            CellName::testing_new("rules_cc++compatibility_proxy+cc_compatibility_proxy"),
            HashMap::new(),
            &snapshot,
        )?;

        assert_eq!(
            aliases.resolve("rules_cc")?,
            CellName::testing_new("rules_cc+")
        );

        Ok(())
    }

    #[test]
    fn bzlmod_resolver_resolves_owner_self_alias_without_runtime_snapshot() -> slug_error::Result<()>
    {
        let aliases = CellAliasResolver::new(
            CellName::testing_new("rules_rs++crate+crates__clap-4.5.60"),
            HashMap::new(),
        )?;

        assert_eq!(
            aliases.resolve_declared_or_runtime_alias("rules_rs"),
            Some(CellName::testing_new("rules_rs+"))
        );

        Ok(())
    }

    #[test]
    fn bzlmod_resolver_resolves_canonical_module_self_alias() -> slug_error::Result<()> {
        let aliases =
            CellAliasResolver::new(CellName::testing_new("rules_license+"), HashMap::new())?;

        assert_eq!(
            aliases.resolve_declared_or_runtime_alias("rules_license"),
            Some(CellName::testing_new("rules_license+"))
        );

        Ok(())
    }

    #[test]
    fn bzlmod_runtime_snapshot_resolves_owner_self_alias_from_apparent_generated_repo()
    -> slug_error::Result<()> {
        let setup = crate::cells::external::ExtensionRepoCellSetup {
            canonical_name: Arc::from("rules_rs++crate+crates__clap-4.5.60"),
            extension_id: Arc::from("@rules_rs//rs:extensions.bzl%crate"),
            internal_name: Arc::from("crates__clap-4.5.60"),
            spec_hash: Arc::from("sha256-test"),
            repo_spec_json: Arc::from("{}"),
            repo_env_json: Arc::from("{}"),
            extension_usages_digest: Arc::from(""),
            extension_replay_inputs_identity_digest: Arc::from(""),
            extension_repo_mappings_digest: Arc::from(""),
            extension_repo_mapping_overrides_digest: Arc::from(""),
            extension_bzl_transitive_digest: Arc::from(""),
            extension_recorded_inputs_json: Arc::from(""),
            materialized: false,
        };
        let snapshot = BzlmodRuntimeCellInstallSnapshot {
            dynamic_aliases: vec![BzlmodRuntimeDynamicAlias {
                apparent_name: "crates__clap-4.5.60".to_owned(),
                canonical_name: "rules_rs++crate+crates__clap-4.5.60".to_owned(),
            }],
            extension_cells: vec![BzlmodRuntimeExtensionCell {
                canonical_name: "rules_rs++crate+crates__clap-4.5.60".to_owned(),
                internal_name: "crates__clap-4.5.60".to_owned(),
                path: "bazel-external/rules_rs++crate+crates__clap-4.5.60".to_owned(),
                setup,
            }],
            ..Default::default()
        };
        let aliases = CellAliasResolver::new_bzlmod_with_runtime_cell_snapshot(
            CellName::testing_new("crates__clap-4.5.60"),
            HashMap::new(),
            &snapshot,
        )?;

        assert_eq!(
            aliases.resolve("rules_rs")?,
            CellName::testing_new("rules_rs+")
        );

        Ok(())
    }

    #[test]
    fn bzlmod_runtime_snapshot_resolves_owner_alias_from_internal_extension_repo_name()
    -> slug_error::Result<()> {
        let setup = crate::cells::external::ExtensionRepoCellSetup {
            canonical_name: Arc::from("rules_rs++crate+crates"),
            extension_id: Arc::from("@rules_rs//rs:extensions.bzl%crate"),
            internal_name: Arc::from("crates"),
            spec_hash: Arc::from("sha256-test"),
            repo_spec_json: Arc::from("{}"),
            repo_env_json: Arc::from("{}"),
            extension_usages_digest: Arc::from(""),
            extension_replay_inputs_identity_digest: Arc::from(""),
            extension_repo_mappings_digest: Arc::from(""),
            extension_repo_mapping_overrides_digest: Arc::from(""),
            extension_bzl_transitive_digest: Arc::from(""),
            extension_recorded_inputs_json: Arc::from(""),
            materialized: false,
        };
        let snapshot = BzlmodRuntimeCellInstallSnapshot {
            root_module_name: None,
            extension_cells: vec![BzlmodRuntimeExtensionCell {
                canonical_name: "rules_rs++crate+crates".to_owned(),
                internal_name: "crates".to_owned(),
                path: "bazel-external/rules_rs++crate+crates".to_owned(),
                setup,
            }],
            ..Default::default()
        };
        let aliases = CellAliasResolver::new_bzlmod_with_runtime_cell_snapshot(
            CellName::testing_new("crates"),
            HashMap::new(),
            &snapshot,
        )?;

        assert_eq!(
            aliases.resolve_declared_or_runtime_alias("rules_rs"),
            Some(CellName::testing_new("rules_rs+"))
        );

        Ok(())
    }

    #[test]
    fn bzlmod_runtime_snapshot_resolves_unique_internal_extension_repo_name()
    -> slug_error::Result<()> {
        let setup = crate::cells::external::ExtensionRepoCellSetup {
            canonical_name: Arc::from("rules_rs++crate+crates__serde-1.0.228"),
            extension_id: Arc::from("@rules_rs//rs:extensions.bzl%crate"),
            internal_name: Arc::from("crates__serde-1.0.228"),
            spec_hash: Arc::from("sha256-serde"),
            repo_spec_json: Arc::from("{}"),
            repo_env_json: Arc::from("{}"),
            extension_usages_digest: Arc::from(""),
            extension_replay_inputs_identity_digest: Arc::from(""),
            extension_repo_mappings_digest: Arc::from(""),
            extension_repo_mapping_overrides_digest: Arc::from(""),
            extension_bzl_transitive_digest: Arc::from(""),
            extension_recorded_inputs_json: Arc::from(""),
            materialized: false,
        };
        let snapshot = BzlmodRuntimeCellInstallSnapshot {
            root_module_name: None,
            extension_cells: vec![BzlmodRuntimeExtensionCell {
                canonical_name: "rules_rs++crate+crates__serde-1.0.228".to_owned(),
                internal_name: "crates__serde-1.0.228".to_owned(),
                path: "bazel-external/rules_rs++crate+crates__serde-1.0.228".to_owned(),
                setup,
            }],
            ..Default::default()
        };
        let aliases = CellAliasResolver::new_bzlmod_with_runtime_cell_snapshot(
            CellName::testing_new("root"),
            HashMap::new(),
            &snapshot,
        )?;

        assert_eq!(
            aliases.resolve_declared_or_runtime_alias("crates__serde-1.0.228"),
            Some(CellName::testing_new(
                "rules_rs++crate+crates__serde-1.0.228"
            ))
        );
        assert_eq!(
            aliases.resolve("crates__serde-1.0.228")?,
            CellName::testing_new("rules_rs++crate+crates__serde-1.0.228")
        );

        Ok(())
    }

    #[test]
    fn bzlmod_runtime_snapshot_ignores_ambiguous_internal_extension_repo_name()
    -> slug_error::Result<()> {
        let first_setup = crate::cells::external::ExtensionRepoCellSetup {
            canonical_name: Arc::from("first_owner++crate+generated"),
            extension_id: Arc::from("@first_owner//:extensions.bzl%crate"),
            internal_name: Arc::from("generated"),
            spec_hash: Arc::from("sha256-first"),
            repo_spec_json: Arc::from("{}"),
            repo_env_json: Arc::from("{}"),
            extension_usages_digest: Arc::from(""),
            extension_replay_inputs_identity_digest: Arc::from(""),
            extension_repo_mappings_digest: Arc::from(""),
            extension_repo_mapping_overrides_digest: Arc::from(""),
            extension_bzl_transitive_digest: Arc::from(""),
            extension_recorded_inputs_json: Arc::from(""),
            materialized: false,
        };
        let second_setup = crate::cells::external::ExtensionRepoCellSetup {
            canonical_name: Arc::from("second_owner++crate+generated"),
            extension_id: Arc::from("@second_owner//:extensions.bzl%crate"),
            internal_name: Arc::from("generated"),
            spec_hash: Arc::from("sha256-second"),
            repo_spec_json: Arc::from("{}"),
            repo_env_json: Arc::from("{}"),
            extension_usages_digest: Arc::from(""),
            extension_replay_inputs_identity_digest: Arc::from(""),
            extension_repo_mappings_digest: Arc::from(""),
            extension_repo_mapping_overrides_digest: Arc::from(""),
            extension_bzl_transitive_digest: Arc::from(""),
            extension_recorded_inputs_json: Arc::from(""),
            materialized: false,
        };
        let snapshot = BzlmodRuntimeCellInstallSnapshot {
            extension_cells: vec![
                BzlmodRuntimeExtensionCell {
                    canonical_name: "first_owner++crate+generated".to_owned(),
                    internal_name: "generated".to_owned(),
                    path: "bazel-external/first_owner++crate+generated".to_owned(),
                    setup: first_setup,
                },
                BzlmodRuntimeExtensionCell {
                    canonical_name: "second_owner++crate+generated".to_owned(),
                    internal_name: "generated".to_owned(),
                    path: "bazel-external/second_owner++crate+generated".to_owned(),
                    setup: second_setup,
                },
            ],
            ..Default::default()
        };
        let aliases = CellAliasResolver::new_bzlmod_with_runtime_cell_snapshot(
            CellName::testing_new("root"),
            HashMap::new(),
            &snapshot,
        )?;

        assert_eq!(aliases.resolve_declared_or_runtime_alias("generated"), None);

        Ok(())
    }

    #[test]
    fn bzlmod_runtime_snapshot_ignores_root_internal_extension_repo_name() -> slug_error::Result<()>
    {
        let setup = crate::cells::external::ExtensionRepoCellSetup {
            canonical_name: Arc::from("plan61_root++replay_ext+replayed_repo"),
            extension_id: Arc::from("@plan61_root//:replay_ext.bzl%replay_ext"),
            internal_name: Arc::from("replayed_repo"),
            spec_hash: Arc::from("sha256-replayed"),
            repo_spec_json: Arc::from("{}"),
            repo_env_json: Arc::from("{}"),
            extension_usages_digest: Arc::from(""),
            extension_replay_inputs_identity_digest: Arc::from(""),
            extension_repo_mappings_digest: Arc::from(""),
            extension_repo_mapping_overrides_digest: Arc::from(""),
            extension_bzl_transitive_digest: Arc::from(""),
            extension_recorded_inputs_json: Arc::from(""),
            materialized: false,
        };
        let snapshot = BzlmodRuntimeCellInstallSnapshot {
            root_module_name: Some("plan61_root".to_owned()),
            extension_cells: vec![BzlmodRuntimeExtensionCell {
                canonical_name: "plan61_root++replay_ext+replayed_repo".to_owned(),
                internal_name: "replayed_repo".to_owned(),
                path: "bazel-external/plan61_root++replay_ext+replayed_repo".to_owned(),
                setup,
            }],
            ..Default::default()
        };
        let aliases = CellAliasResolver::new_bzlmod_with_runtime_cell_snapshot(
            CellName::testing_new("root"),
            HashMap::new(),
            &snapshot,
        )?;

        assert_eq!(
            aliases.resolve_declared_or_runtime_alias("replayed_repo"),
            None
        );

        Ok(())
    }

    #[test]
    fn bzlmod_runtime_snapshot_resolves_same_extension_internal_sibling_alias()
    -> slug_error::Result<()> {
        let generated_setup = crate::cells::external::ExtensionRepoCellSetup {
            canonical_name: Arc::from("rules_rs++crate+crates__git_repo"),
            extension_id: Arc::from("@rules_rs//rs:extensions.bzl%crate"),
            internal_name: Arc::from("crates__git_repo"),
            spec_hash: Arc::from("sha256-generated"),
            repo_spec_json: Arc::from("{}"),
            repo_env_json: Arc::from("{}"),
            extension_usages_digest: Arc::from(""),
            extension_replay_inputs_identity_digest: Arc::from(""),
            extension_repo_mappings_digest: Arc::from(""),
            extension_repo_mapping_overrides_digest: Arc::from(""),
            extension_bzl_transitive_digest: Arc::from(""),
            extension_recorded_inputs_json: Arc::from(""),
            materialized: false,
        };
        let sibling_setup = crate::cells::external::ExtensionRepoCellSetup {
            canonical_name: Arc::from("rules_rs++crate+crates__ts-rs-12.0.1"),
            extension_id: Arc::from("@rules_rs//rs:extensions.bzl%crate"),
            internal_name: Arc::from("crates__ts-rs-12.0.1"),
            spec_hash: Arc::from("sha256-sibling"),
            repo_spec_json: Arc::from("{}"),
            repo_env_json: Arc::from("{}"),
            extension_usages_digest: Arc::from(""),
            extension_replay_inputs_identity_digest: Arc::from(""),
            extension_repo_mappings_digest: Arc::from(""),
            extension_repo_mapping_overrides_digest: Arc::from(""),
            extension_bzl_transitive_digest: Arc::from(""),
            extension_recorded_inputs_json: Arc::from(""),
            materialized: false,
        };
        let snapshot = BzlmodRuntimeCellInstallSnapshot {
            extension_cells: vec![
                BzlmodRuntimeExtensionCell {
                    canonical_name: "rules_rs++crate+crates__git_repo".to_owned(),
                    internal_name: "crates__git_repo".to_owned(),
                    path: "bazel-external/rules_rs++crate+crates__git_repo".to_owned(),
                    setup: generated_setup,
                },
                BzlmodRuntimeExtensionCell {
                    canonical_name: "rules_rs++crate+crates__ts-rs-12.0.1".to_owned(),
                    internal_name: "crates__ts-rs-12.0.1".to_owned(),
                    path: "bazel-external/rules_rs++crate+crates__ts-rs-12.0.1".to_owned(),
                    setup: sibling_setup,
                },
            ],
            ..Default::default()
        };
        let aliases = CellAliasResolver::new_bzlmod_with_runtime_cell_snapshot(
            CellName::testing_new("rules_rs++crate+crates__git_repo"),
            HashMap::new(),
            &snapshot,
        )?;

        assert_eq!(
            aliases.resolve_declared_or_runtime_alias("crates__ts-rs-12.0.1"),
            Some(CellName::testing_new(
                "rules_rs++crate+crates__ts-rs-12.0.1"
            ))
        );

        Ok(())
    }

    #[test]
    fn bzlmod_runtime_snapshot_resolves_static_canonical_module_cell() -> slug_error::Result<()> {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir()?;
        reset_dynamic_bzlmod_state_for_project_root(tmp.path().to_path_buf());

        let root = CellName::testing_new("root");
        let owner = CellName::testing_new("owner");
        let owner_canonical = CellName::testing_new("owner+");
        let root_path = CellRootPathBuf::testing_new("");
        let owner_path = CellRootPathBuf::testing_new("bazel-external/owner+");
        let cell_roots = [(root, root_path.as_path()), (owner, owner_path.as_path())];
        let root_instance = CellInstance::new(
            root,
            root_path.clone(),
            None,
            NestedCells::from_cell_roots(&cell_roots, &root_path),
        )?;
        let owner_instance = CellInstance::new(
            owner,
            owner_path.clone(),
            None,
            NestedCells::from_cell_roots(&cell_roots, &owner_path),
        )?;
        let snapshot = BzlmodRuntimeCellInstallSnapshot::default();
        let root_aliases = CellAliasResolver::new_bzlmod_with_runtime_cell_snapshot(
            root,
            HashMap::new(),
            &snapshot,
        )?;
        let resolver = CellResolver::new_bzlmod_with_runtime_cell_snapshot(
            vec![root_instance, owner_instance],
            root_aliases,
            snapshot,
        )?;

        assert_eq!(resolver.get(owner_canonical)?.name(), owner);
        let load_path = CellPath::new(
            owner_canonical,
            ForwardRelativePathBuf::unchecked_new("replay_ext.bzl".to_owned()).into(),
        );
        assert_eq!(
            resolver.resolve_path(load_path.as_ref())?,
            ProjectRelativePathBuf::unchecked_new("bazel-external/owner+/replay_ext.bzl".into())
        );

        Ok(())
    }

    #[test]
    fn bzlmod_resolver_get_resolves_apparent_module_name_to_canonical_module_cell()
    -> slug_error::Result<()> {
        let root = CellName::testing_new("root");
        let zstd = CellName::testing_new("zstd+");
        let root_path = CellRootPathBuf::testing_new("");
        let zstd_path = CellRootPathBuf::testing_new("bazel-external/zstd+");
        let cell_roots = [(root, root_path.as_path()), (zstd, zstd_path.as_path())];
        let root_instance = CellInstance::new(
            root,
            root_path.clone(),
            None,
            NestedCells::from_cell_roots(&cell_roots, &root_path),
        )?;
        let zstd_instance = CellInstance::new(
            zstd,
            zstd_path.clone(),
            None,
            NestedCells::from_cell_roots(&cell_roots, &zstd_path),
        )?;
        let snapshot = BzlmodRuntimeCellInstallSnapshot::default();
        let root_aliases = CellAliasResolver::new_bzlmod_with_runtime_cell_snapshot(
            root,
            HashMap::new(),
            &snapshot,
        )?;
        let resolver = CellResolver::new_bzlmod_with_runtime_cell_snapshot(
            vec![root_instance, zstd_instance],
            root_aliases,
            snapshot,
        )?;

        assert_eq!(resolver.get(CellName::testing_new("zstd"))?.name(), zstd);

        Ok(())
    }

    #[test]
    fn bzlmod_label_cell_paths_project_runtime_snapshot_without_globals() -> slug_error::Result<()>
    {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir()?;
        reset_dynamic_bzlmod_state_for_project_root(tmp.path().to_path_buf());

        let root = CellName::testing_new("root");
        let dep = CellName::testing_new("dep");
        let root_path = CellRootPathBuf::testing_new("");
        let dep_path = CellRootPathBuf::testing_new("bazel-external/dep+1.0");
        let cell_roots = [(root, root_path.as_path()), (dep, dep_path.as_path())];
        let root_instance = CellInstance::new(
            root,
            root_path.clone(),
            None,
            NestedCells::from_cell_roots(&cell_roots, &root_path),
        )?;
        let dep_instance = CellInstance::new(
            dep,
            dep_path.clone(),
            None,
            NestedCells::from_cell_roots(&cell_roots, &dep_path),
        )?;
        let canonical = "owner++ext+generated";
        let runtime_path = format!("bazel-external/{canonical}");
        let setup = crate::cells::external::ExtensionRepoCellSetup {
            canonical_name: Arc::from(canonical),
            extension_id: Arc::from("@owner//:ext.bzl%ext"),
            internal_name: Arc::from("generated"),
            spec_hash: Arc::from("sha256-test"),
            repo_spec_json: Arc::from("{}"),
            repo_env_json: Arc::from("{}"),
            extension_usages_digest: Arc::from(""),
            extension_replay_inputs_identity_digest: Arc::from(""),
            extension_repo_mappings_digest: Arc::from(""),
            extension_repo_mapping_overrides_digest: Arc::from(""),
            extension_bzl_transitive_digest: Arc::from(""),
            extension_recorded_inputs_json: Arc::from(""),
            materialized: false,
        };
        let snapshot = BzlmodRuntimeCellInstallSnapshot {
            root_module_name: None,
            extension_cells: vec![BzlmodRuntimeExtensionCell {
                canonical_name: canonical.to_owned(),
                internal_name: "generated".to_owned(),
                path: runtime_path.clone(),
                setup,
            }],
            scoped_aliases: vec![BzlmodRuntimeScopedRepoAlias {
                owner_module: "owner+".to_owned(),
                apparent_name: "owner_tool".to_owned(),
                target_name: canonical.to_owned(),
            }],
            dynamic_aliases: vec![BzlmodRuntimeDynamicAlias {
                apparent_name: "runtime_alias".to_owned(),
                canonical_name: canonical.to_owned(),
            }],
        };
        let mut aliases = HashMap::new();
        aliases.insert(
            NonEmptyCellAlias::new("root_generated".to_owned())?,
            CellName::testing_new(canonical),
        );
        aliases.insert(NonEmptyCellAlias::new("dep_alias".to_owned())?, dep);
        let root_aliases =
            CellAliasResolver::new_bzlmod_with_runtime_cell_snapshot(root, aliases, &snapshot)?;
        let resolver = CellResolver::new_bzlmod_with_runtime_cell_snapshot(
            vec![root_instance, dep_instance],
            root_aliases,
            snapshot,
        )?;

        let paths: BTreeMap<_, _> = resolver.bzlmod_label_cell_paths().into_iter().collect();

        assert_eq!(paths.get(canonical), Some(&runtime_path));
        assert_eq!(paths.get("runtime_alias"), Some(&runtime_path));
        assert_eq!(paths.get("root_generated"), Some(&runtime_path));
        assert!(!paths.contains_key("owner_tool"));
        let owner_paths: BTreeMap<_, _> = resolver
            .bzlmod_label_cell_paths_for_owner(Some("owner"))
            .into_iter()
            .collect();
        assert_eq!(owner_paths.get("owner_tool"), Some(&runtime_path));
        assert_eq!(
            paths.get("dep_alias").map(String::as_str),
            Some("bazel-external/dep+1.0")
        );
        assert!(!paths.contains_key("generated"));
        assert_eq!(get_test_dynamic_extension_cell(canonical), None);

        reset_dynamic_bzlmod_state_for_project_root(tmp.path().join("after"));
        assert_eq!(paths.get(canonical), Some(&runtime_path));
        assert_eq!(get_test_dynamic_extension_cell(canonical), None);

        let late_canonical = "owner++ext+late";
        let late_path = format!("bazel-external/{late_canonical}");
        let late_setup = crate::cells::external::ExtensionRepoCellSetup {
            canonical_name: Arc::from(late_canonical),
            extension_id: Arc::from("@owner//:ext.bzl%ext"),
            internal_name: Arc::from("late"),
            spec_hash: Arc::from("sha256-late"),
            repo_spec_json: Arc::from("{}"),
            repo_env_json: Arc::from("{}"),
            extension_usages_digest: Arc::from(""),
            extension_replay_inputs_identity_digest: Arc::from(""),
            extension_repo_mappings_digest: Arc::from(""),
            extension_repo_mapping_overrides_digest: Arc::from(""),
            extension_bzl_transitive_digest: Arc::from(""),
            extension_recorded_inputs_json: Arc::from(""),
            materialized: false,
        };
        resolver.register_bzlmod_runtime_extension_cell(late_canonical, &late_path, late_setup)?;
        let paths: BTreeMap<_, _> = resolver.bzlmod_label_cell_paths().into_iter().collect();
        assert_eq!(paths.get(late_canonical), Some(&late_path));
        assert_eq!(
            resolver.get(CellName::testing_new("late"))?.name(),
            CellName::testing_new(late_canonical)
        );
        assert_eq!(get_test_dynamic_extension_cell(late_canonical), None);

        Ok(())
    }

    #[test]
    fn bzlmod_resolver_registers_runtime_spoke_without_global_registry() -> slug_error::Result<()> {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir()?;
        reset_dynamic_bzlmod_state_for_project_root(tmp.path().to_path_buf());
        let resolver = CellResolver::testing_with_names_and_paths(&[(
            CellName::testing_new("root"),
            CellRootPathBuf::testing_new(""),
        )]);
        let canonical = "owner++ext+sibling";
        let setup = crate::cells::external::ExtensionRepoCellSetup {
            canonical_name: Arc::from(canonical),
            extension_id: Arc::from("@owner//:ext.bzl%ext"),
            internal_name: Arc::from("sibling"),
            spec_hash: Arc::from("sha256-sibling"),
            repo_spec_json: Arc::from("{}"),
            repo_env_json: Arc::from("{}"),
            extension_usages_digest: Arc::from(""),
            extension_replay_inputs_identity_digest: Arc::from(""),
            extension_repo_mappings_digest: Arc::from(""),
            extension_repo_mapping_overrides_digest: Arc::from(""),
            extension_bzl_transitive_digest: Arc::from(""),
            extension_recorded_inputs_json: Arc::from(""),
            materialized: false,
        };

        resolver.register_bzlmod_runtime_extension_cell(
            canonical,
            &format!("bazel-external/{canonical}"),
            setup.clone(),
        )?;

        assert_eq!(get_test_dynamic_extension_cell(canonical), None);
        let cell = resolver.get(CellName::testing_new(canonical))?;
        assert_eq!(cell.path().as_str(), format!("bazel-external/{canonical}"));
        assert!(matches!(
            cell.external(),
            Some(crate::cells::external::ExternalCellOrigin::ExtensionRepo(origin))
                if origin == &setup
        ));

        reset_dynamic_bzlmod_state_for_project_root(tmp.path().join("after"));
        assert_eq!(get_test_dynamic_extension_cell(canonical), None);
        assert_eq!(
            resolver
                .get(CellName::testing_new(canonical))?
                .name()
                .as_str(),
            canonical
        );

        Ok(())
    }

    #[test]
    fn bzlmod_resolver_runtime_spoke_overlays_static_placeholder_extension_cell()
    -> slug_error::Result<()> {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir()?;
        reset_dynamic_bzlmod_state_for_project_root(tmp.path().to_path_buf());

        let root = CellName::testing_new("root");
        let root_path = CellRootPathBuf::testing_new("");
        let root_instance = CellInstance::new(
            root,
            root_path.clone(),
            None,
            NestedCells::from_cell_roots(&[(root, root_path.as_path())], &root_path),
        )?;
        let canonical = "owner++ext+generated";
        let placeholder_setup = crate::cells::external::ExtensionRepoCellSetup {
            canonical_name: Arc::from(canonical),
            extension_id: Arc::from("@owner//:ext.bzl%ext"),
            internal_name: Arc::from("generated"),
            spec_hash: Arc::from(""),
            repo_spec_json: Arc::from(""),
            repo_env_json: Arc::from("{}"),
            extension_usages_digest: Arc::from(""),
            extension_replay_inputs_identity_digest: Arc::from(""),
            extension_repo_mappings_digest: Arc::from(""),
            extension_repo_mapping_overrides_digest: Arc::from(""),
            extension_bzl_transitive_digest: Arc::from(""),
            extension_recorded_inputs_json: Arc::from(""),
            materialized: false,
        };
        let real_setup = crate::cells::external::ExtensionRepoCellSetup {
            canonical_name: Arc::from(canonical),
            extension_id: Arc::from("@owner//:ext.bzl%ext"),
            internal_name: Arc::from("generated"),
            spec_hash: Arc::from("sha256-generated"),
            repo_spec_json: Arc::from(r#"{"rule":"repo"}"#),
            repo_env_json: Arc::from("{}"),
            extension_usages_digest: Arc::from(""),
            extension_replay_inputs_identity_digest: Arc::from(""),
            extension_repo_mappings_digest: Arc::from(""),
            extension_repo_mapping_overrides_digest: Arc::from(""),
            extension_bzl_transitive_digest: Arc::from("bzl-digest"),
            extension_recorded_inputs_json: Arc::from("[]"),
            materialized: false,
        };
        let cell_name = CellName::testing_new(canonical);
        let cell_path = CellRootPathBuf::testing_new(&format!("bazel-external/{canonical}"));
        let placeholder_instance = CellInstance::new(
            cell_name,
            cell_path.clone(),
            Some(crate::cells::external::ExternalCellOrigin::ExtensionRepo(
                placeholder_setup,
            )),
            NestedCells::from_cell_roots(&[], &cell_path),
        )?;
        let snapshot = BzlmodRuntimeCellInstallSnapshot::default();
        let root_aliases = CellAliasResolver::new_bzlmod_with_runtime_cell_snapshot(
            root,
            HashMap::new(),
            &snapshot,
        )?;
        let resolver = CellResolver::new_bzlmod_with_runtime_cell_snapshot(
            vec![root_instance, placeholder_instance],
            root_aliases,
            snapshot,
        )?;

        resolver.register_bzlmod_runtime_extension_cell(
            canonical,
            &format!("bazel-external/{canonical}"),
            real_setup.clone(),
        )?;

        let cell = resolver.get(cell_name)?;
        assert!(matches!(
            cell.external(),
            Some(crate::cells::external::ExternalCellOrigin::ExtensionRepo(origin))
                if origin == &real_setup
        ));
        reset_dynamic_bzlmod_state_for_project_root(tmp.path().join("after"));

        Ok(())
    }

    #[test]
    fn cell_resolver_dynamic_suffix_lookup_is_deterministic() -> slug_error::Result<()> {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir()?;
        reset_dynamic_bzlmod_state_for_project_root(tmp.path().to_path_buf());
        register_test_dynamic_extension_cell(
            "z_owner++ext+generated".to_owned(),
            "bazel-external/z_owner++ext+generated".to_owned(),
        );
        register_test_dynamic_extension_cell(
            "a_owner++ext+generated".to_owned(),
            "bazel-external/a_owner++ext+generated".to_owned(),
        );
        let cells = CellResolver::testing_with_names_and_paths(&[(
            CellName::testing_new("root"),
            CellRootPathBuf::testing_new(""),
        )]);
        let apparent = CellName::testing_new("generated");

        let cell = cells.get(apparent)?;

        assert_eq!(apparent, cell.name());
        assert_eq!(
            "bazel-external/a_owner++ext+generated",
            cell.path().as_str()
        );
        reset_dynamic_bzlmod_state_for_project_root(tmp.path().join("after"));
        Ok(())
    }

    #[test]
    fn dynamic_bzlmod_entries_are_scoped_to_current_project_root() -> slug_error::Result<()> {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir()?;
        let root_a = tmp.path().join("a");
        let root_b = tmp.path().join("b");
        reset_dynamic_bzlmod_state_for_project_root(root_a.clone());
        std::fs::create_dir_all(root_a.join("bazel-external/dep+1.0"))?;
        let canonical = "owner++ext+repo";
        let setup = crate::cells::external::ExtensionRepoCellSetup {
            canonical_name: Arc::from(canonical),
            extension_id: Arc::from("@owner//:ext.bzl%ext"),
            internal_name: Arc::from("repo"),
            spec_hash: Arc::from("sha256-test"),
            repo_spec_json: Arc::from("{}"),
            repo_env_json: Arc::from("{}"),
            extension_usages_digest: Arc::from(""),
            extension_replay_inputs_identity_digest: Arc::from(""),
            extension_repo_mappings_digest: Arc::from(""),
            extension_repo_mapping_overrides_digest: Arc::from(""),
            extension_bzl_transitive_digest: Arc::from(""),
            extension_recorded_inputs_json: Arc::from(""),
            materialized: false,
        };
        let canonical_path = format!("bazel-external/{canonical}");

        register_test_dynamic_extension_cell_with_setup(
            canonical.to_owned(),
            canonical_path.clone(),
            setup.clone(),
        );
        register_test_dynamic_extension_cell_alias("repo_alias".to_owned(), canonical.to_owned());
        register_test_scoped_bzlmod_repo_alias(
            "owner+".to_owned(),
            "dep".to_owned(),
            "dep+1.0".to_owned(),
        );

        assert_eq!(
            get_test_dynamic_extension_cell(canonical).as_deref(),
            Some(canonical_path.as_str())
        );
        assert_eq!(
            resolve_test_dynamic_extension_cell_alias("repo_alias").as_deref(),
            Some(canonical)
        );
        assert_eq!(
            get_test_dynamic_extension_cell_setup(canonical),
            Some(setup)
        );
        assert_eq!(
            resolve_test_scoped_bzlmod_repo_alias("owner+", "dep").as_deref(),
            Some("dep+1.0")
        );
        assert_eq!(
            canonical_test_dynamic_extension_cell_name("repo").as_deref(),
            Some(canonical)
        );
        assert_eq!(
            canonical_bzlmod_module_cell_name("dep").as_deref(),
            Some("dep+1.0")
        );
        let root_cell = CellName::testing_new("scoped_root");
        let external_cell = CellName::testing_new("external_dep");
        let cells = CellResolver::testing_with_names_and_paths(&[
            (root_cell, CellRootPathBuf::testing_new("")),
            (external_cell, CellRootPathBuf::testing_new("external_dep")),
        ]);
        let dynamic_cell = CellName::testing_new(canonical);
        assert!(!is_root_cell_name(root_cell.as_str()));
        assert_eq!(cells.get(dynamic_cell)?.name(), dynamic_cell);
        assert_eq!(
            cells.get_cell_path(ProjectRelativePath::new(&format!(
                "{canonical_path}/defs.bzl"
            ))?),
            CellPath::new(
                dynamic_cell,
                ForwardRelativePathBuf::unchecked_new("defs.bzl".to_owned()).into()
            )
        );

        *DYNAMIC_BZLMOD_SCOPE.write().unwrap() =
            DynamicBzlmodScope::for_project_root(root_b.clone());

        assert_eq!(get_test_dynamic_extension_cell(canonical), None);
        assert_eq!(
            resolve_test_dynamic_extension_cell_alias("repo_alias"),
            None
        );
        assert_eq!(get_test_dynamic_extension_cell_setup(canonical), None);
        assert_eq!(resolve_test_scoped_bzlmod_repo_alias("owner+", "dep"), None);
        assert_eq!(canonical_test_dynamic_extension_cell_name("repo"), None);
        assert_eq!(canonical_bzlmod_module_cell_name("dep"), None);
        assert!(!is_root_cell_name(root_cell.as_str()));
        assert!(cells.get(dynamic_cell).is_err());
        assert_eq!(
            cells.get_cell_path(ProjectRelativePath::new(&format!(
                "{canonical_path}/defs.bzl"
            ))?),
            CellPath::new(
                root_cell,
                ForwardRelativePathBuf::unchecked_new(format!("{canonical_path}/defs.bzl")).into()
            )
        );

        reset_dynamic_bzlmod_state_for_project_root(root_b);
        Ok(())
    }

    #[test]
    fn dynamic_bzlmod_entries_are_scoped_to_current_output_base() -> slug_error::Result<()> {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir()?;
        let root = tmp.path().join("workspace");
        let output_a = tmp.path().join("out-a");
        let output_b = tmp.path().join("out-b");
        reset_dynamic_bzlmod_state_for_workspace(root.clone(), output_a.clone());

        let canonical = "owner++ext+repo";
        let canonical_path = format!("bazel-external/{canonical}");
        let setup = crate::cells::external::ExtensionRepoCellSetup {
            canonical_name: Arc::from(canonical),
            extension_id: Arc::from("@owner//:ext.bzl%ext"),
            internal_name: Arc::from("repo"),
            spec_hash: Arc::from("sha256-a"),
            repo_spec_json: Arc::from("{}"),
            repo_env_json: Arc::from(r#"{"REPO_ENV":"A"}"#),
            extension_usages_digest: Arc::from(""),
            extension_replay_inputs_identity_digest: Arc::from(""),
            extension_repo_mappings_digest: Arc::from(""),
            extension_repo_mapping_overrides_digest: Arc::from(""),
            extension_bzl_transitive_digest: Arc::from(""),
            extension_recorded_inputs_json: Arc::from(""),
            materialized: false,
        };

        register_test_dynamic_extension_cell_with_setup(
            canonical.to_owned(),
            canonical_path.clone(),
            setup.clone(),
        );
        register_test_dynamic_extension_cell_alias("repo_alias".to_owned(), canonical.to_owned());
        register_test_scoped_bzlmod_repo_alias(
            "owner+".to_owned(),
            "repo".to_owned(),
            canonical.to_owned(),
        );

        assert_eq!(
            get_test_dynamic_extension_cell(canonical).as_deref(),
            Some(canonical_path.as_str())
        );
        assert_eq!(
            get_test_dynamic_extension_cell_setup(canonical),
            Some(setup)
        );
        assert_eq!(
            resolve_test_dynamic_extension_cell_alias("repo_alias").as_deref(),
            Some(canonical)
        );
        assert_eq!(
            resolve_test_scoped_bzlmod_repo_alias("owner+", "repo").as_deref(),
            Some(canonical)
        );

        *DYNAMIC_BZLMOD_SCOPE.write().unwrap() =
            DynamicBzlmodScope::for_workspace(root.clone(), output_b.clone());

        assert_eq!(get_test_dynamic_extension_cell(canonical), None);
        assert_eq!(get_test_dynamic_extension_cell_setup(canonical), None);
        assert_eq!(
            resolve_test_dynamic_extension_cell_alias("repo_alias"),
            None
        );
        assert_eq!(
            resolve_test_scoped_bzlmod_repo_alias("owner+", "repo"),
            None
        );

        *DYNAMIC_BZLMOD_SCOPE.write().unwrap() =
            DynamicBzlmodScope::for_workspace(root.clone(), output_a);

        assert_eq!(
            get_test_dynamic_extension_cell(canonical).as_deref(),
            Some(canonical_path.as_str())
        );
        assert_eq!(
            resolve_test_dynamic_extension_cell_alias("repo_alias").as_deref(),
            Some(canonical)
        );

        reset_dynamic_bzlmod_state_for_workspace(root, output_b);
        assert_eq!(get_test_dynamic_extension_cell(canonical), None);
        Ok(())
    }

    #[test]
    fn workspace_scoped_bzlmod_entries_do_not_scan_bazel_external() -> slug_error::Result<()> {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir()?;
        let root = tmp.path().join("workspace");
        let output = tmp.path().join("out");
        let canonical = "owner++ext+repo";
        std::fs::create_dir_all(root.join("bazel-external").join(canonical))?;
        std::fs::create_dir_all(root.join("bazel-external/dep+1.0"))?;
        reset_dynamic_bzlmod_state_for_workspace(root, output);

        assert_eq!(canonical_test_dynamic_extension_cell_name("repo"), None);
        assert_eq!(canonical_test_dynamic_extension_cell_name(canonical), None);
        assert_eq!(canonical_bzlmod_module_cell_name("dep"), None);

        let cells = CellResolver::testing_with_names_and_paths(&[(
            CellName::testing_new("root"),
            CellRootPathBuf::testing_new(""),
        )]);
        assert!(cells.get(CellName::testing_new(canonical)).is_err());

        Ok(())
    }

    #[test]
    fn execroot_path_returns_basename_subdir() {
        let path = std::path::Path::new("/tmp/some/workspace");
        assert_eq!(
            super::execroot_path(path),
            Some(std::path::PathBuf::from(
                "/tmp/some/workspace/execroot/workspace"
            ))
        );
    }

    #[test]
    fn execroot_path_returns_none_for_empty_basename() {
        assert_eq!(super::execroot_path(std::path::Path::new("/")), None);
    }

    #[test]
    fn unknown_cell_alias_diagnostic_caps_known_aliases() -> slug_error::Result<()> {
        let mut aliases = HashMap::new();
        for i in 0..100 {
            aliases.insert(
                NonEmptyCellAlias::new(format!("alias_{i:03}"))?,
                CellName::testing_new("root"),
            );
        }
        let resolver = CellAliasResolver::new(CellName::testing_new("root"), aliases)?;

        let err = resolver.resolve("missing").unwrap_err().to_string();

        assert!(err.contains("unknown cell alias: `missing`"));
        assert!(err.contains("known aliases are: `alias_000"));
        assert!(err.contains("showing 50 of 101; 51 omitted"));
        assert!(!err.contains("alias_050"));
        assert!(!err.contains("alias_099"));

        Ok(())
    }

    #[test]
    fn cell_alias_resolver_resolves_non_empty_alias_key() -> slug_error::Result<()> {
        let mut aliases = HashMap::new();
        aliases.insert(
            NonEmptyCellAlias::new("bazel_lib".to_owned())?,
            CellName::testing_new("bazel_lib"),
        );
        let resolver = CellAliasResolver::new(CellName::testing_new("root"), aliases)?;

        assert_eq!(
            CellName::testing_new("bazel_lib"),
            resolver.resolve("bazel_lib")?
        );

        Ok(())
    }

    #[test]
    fn canonical_dynamic_extension_cell_name_preserves_bzlmod_module_cell() {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        reset_dynamic_bzlmod_state_for_project_root(tmp.path().to_path_buf());

        let apparent = "exact_module_cell_test_unique";
        let canonical = "owner+extension+exact_module_cell_test_unique";
        std::fs::create_dir_all(tmp.path().join(format!("bazel-external/{apparent}+"))).unwrap();
        register_test_dynamic_extension_cell(
            canonical.to_owned(),
            format!("bazel-external/{canonical}"),
        );

        let resolved = canonical_test_dynamic_extension_cell_name(apparent);

        assert_eq!(None, resolved);
    }

    #[test]
    fn canonical_bzlmod_module_cell_name_uses_empty_version_module_suffix() {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("bazel-external/llvm+")).unwrap();
        reset_dynamic_bzlmod_state_for_project_root(tmp.path().to_path_buf());

        assert_eq!(
            Some("llvm+"),
            canonical_bzlmod_module_cell_name("llvm").as_deref()
        );
    }

    #[test]
    fn action_external_cell_name_uses_canonical_generated_repo_suffix() {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let apparent = "rustc_linux_x86_64_1_95_0";
        let canonical = "rules_rs++toolchains+rustc_linux_x86_64_1_95_0";
        std::fs::create_dir_all(tmp.path().join("bazel-external").join(canonical)).unwrap();
        reset_dynamic_bzlmod_state_for_project_root(tmp.path().to_path_buf());

        assert_eq!(
            canonical,
            action_external_cell_name(tmp.path(), apparent, &format!("bazel-external/{apparent}"))
        );
    }

    #[test]
    #[cfg(unix)]
    fn action_external_cell_name_uses_external_symlink_target() {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let apparent = "rustc_linux_x86_64_1_95_0";
        let canonical = "rules_rs++toolchains+rustc_linux_x86_64_1_95_0";
        let canonical_path = tmp.path().join("bazel-external").join(canonical);
        std::fs::create_dir_all(&canonical_path).unwrap();
        std::fs::create_dir(tmp.path().join("external")).unwrap();
        std::os::unix::fs::symlink(&canonical_path, tmp.path().join("external").join(apparent))
            .unwrap();
        reset_dynamic_bzlmod_state_for_project_root(tmp.path().to_path_buf());

        assert_eq!(
            canonical,
            action_external_cell_name(tmp.path(), apparent, &format!("bazel-external/{apparent}"))
        );
    }

    #[test]
    #[cfg(unix)]
    fn workspace_scoped_action_external_cell_name_ignores_physical_fallbacks() {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let output = tmp.path().join("out");
        let apparent = "rustc_linux_x86_64_1_95_0";
        let canonical = "rules_rs++toolchains+rustc_linux_x86_64_1_95_0";
        let canonical_path = tmp.path().join("bazel-external").join(canonical);
        std::fs::create_dir_all(&canonical_path).unwrap();
        std::fs::create_dir(tmp.path().join("external")).unwrap();
        std::os::unix::fs::symlink(&canonical_path, tmp.path().join("external").join(apparent))
            .unwrap();
        reset_dynamic_bzlmod_state_for_workspace(tmp.path().to_path_buf(), output);

        assert_eq!(
            apparent,
            action_external_cell_name(tmp.path(), apparent, &format!("bazel-external/{apparent}"))
        );

        register_test_dynamic_extension_cell(
            canonical.to_owned(),
            format!("bazel-external/{canonical}"),
        );
        register_test_dynamic_extension_cell_alias(apparent.to_owned(), canonical.to_owned());
        assert_eq!(
            canonical,
            action_external_cell_name(tmp.path(), apparent, &format!("bazel-external/{apparent}"))
        );
    }

    #[test]
    #[cfg(unix)]
    fn workspace_scoped_external_symlink_replaces_stale_physical_fallback() {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let output = tmp.path().join("out");
        let apparent = "rustc_linux_x86_64_1_95_0";
        let stale = "rules_rs++toolchains+rustc_linux_x86_64_1_95_0";
        let apparent_path = tmp.path().join("bazel-external").join(apparent);
        let stale_path = tmp.path().join("bazel-external").join(stale);
        std::fs::create_dir_all(&apparent_path).unwrap();
        std::fs::create_dir_all(&stale_path).unwrap();
        std::fs::create_dir(tmp.path().join("external")).unwrap();
        std::os::unix::fs::symlink(&stale_path, tmp.path().join("external").join(apparent))
            .unwrap();
        reset_dynamic_bzlmod_state_for_workspace(tmp.path().to_path_buf(), output);

        let apparent_cell_path = format!("bazel-external/{apparent}");
        ensure_external_symlinks_for_cells(&[(apparent, apparent_cell_path.as_str())]);

        let repaired = std::fs::read_link(tmp.path().join("external").join(apparent)).unwrap();
        assert_eq!(repaired, apparent_path);
    }

    #[test]
    #[cfg(unix)]
    fn workspace_scoped_external_symlink_replaces_unmaterialized_stale_link() {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let output = tmp.path().join("out");
        let apparent = "rustc_linux_x86_64_1_95_0";
        let stale = "rules_rs++toolchains+rustc_linux_x86_64_1_95_0";
        std::fs::create_dir(tmp.path().join("external")).unwrap();
        let stale_target = external_symlink_relative_target(&format!("bazel-external/{stale}"));
        std::os::unix::fs::symlink(&stale_target, tmp.path().join("external").join(apparent))
            .unwrap();
        reset_dynamic_bzlmod_state_for_workspace(tmp.path().to_path_buf(), output);

        let apparent_cell_path = format!("bazel-external/{apparent}");
        ensure_external_symlinks_for_cells(&[(apparent, apparent_cell_path.as_str())]);

        let repaired = std::fs::read_link(tmp.path().join("external").join(apparent)).unwrap();
        assert_eq!(
            repaired,
            external_symlink_relative_target(&apparent_cell_path)
        );
    }

    #[test]
    #[cfg(unix)]
    fn workspace_scope_reset_does_not_run_legacy_external_symlink_repair() {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let output = root.join("out");
        let apparent = "rules_rust";
        let extension_repo = root
            .join("bazel-external")
            .join("rules_rs++rules_rust+rules_rust");
        let module_repo = root.join("bazel-external").join("rules_rust+0.69.0");
        reset_dynamic_bzlmod_state_for_project_root(root.to_path_buf());
        std::fs::create_dir_all(&extension_repo).unwrap();
        std::fs::create_dir_all(&module_repo).unwrap();
        std::fs::create_dir(root.join("external")).unwrap();
        std::os::unix::fs::symlink(&extension_repo, root.join("external").join(apparent)).unwrap();

        reset_dynamic_bzlmod_state_for_workspace(root.to_path_buf(), output);

        assert_eq!(
            std::fs::canonicalize(root.join("external").join(apparent)).unwrap(),
            std::fs::canonicalize(extension_repo).unwrap()
        );
    }

    #[test]
    #[cfg(unix)]
    fn project_root_scope_reset_runs_legacy_external_symlink_repair_after_workspace_scope() {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let output = root.join("out");
        let apparent = "rules_rust";
        let extension_repo = root
            .join("bazel-external")
            .join("rules_rs++rules_rust+rules_rust");
        let module_repo = root.join("bazel-external").join("rules_rust+0.69.0");
        reset_dynamic_bzlmod_state_for_workspace(root.to_path_buf(), output);
        std::fs::create_dir_all(&extension_repo).unwrap();
        std::fs::create_dir_all(&module_repo).unwrap();
        std::fs::create_dir(root.join("external")).unwrap();
        std::os::unix::fs::symlink(&extension_repo, root.join("external").join(apparent)).unwrap();

        reset_dynamic_bzlmod_state_for_project_root(root.to_path_buf());

        assert_eq!(
            std::fs::canonicalize(root.join("external").join(apparent)).unwrap(),
            std::fs::canonicalize(module_repo).unwrap()
        );
    }

    #[test]
    fn cell_alias_resolver_prefers_scoped_bzlmod_repo_alias() -> slug_error::Result<()> {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir()?;
        reset_dynamic_bzlmod_state_for_project_root(tmp.path().to_path_buf());

        let apparent = "scoped_alias_test_project";
        let wanted = "owner++toolchain+scoped_alias_test_project";
        let global_wrong = "owner+other_extension+scoped_alias_test_project";

        register_test_scoped_bzlmod_repo_alias(
            "owner+".to_owned(),
            apparent.to_owned(),
            wanted.to_owned(),
        );

        let mut aliases = HashMap::new();
        aliases.insert(
            NonEmptyCellAlias::new(apparent.to_owned())?,
            CellName::testing_new(global_wrong),
        );
        let resolver =
            CellAliasResolver::new(CellName::testing_new("owner+source+generated"), aliases)?;

        assert_eq!(CellName::testing_new(wanted), resolver.resolve(apparent)?);
        Ok(())
    }

    #[test]
    fn scoped_bzlmod_repo_alias_resolves_double_plus_owner() {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        reset_dynamic_bzlmod_state_for_project_root(tmp.path().to_path_buf());

        let apparent = "scoped_alias_test_double_plus_project";
        let wanted = "double_owner++toolchain+scoped_alias_test_double_plus_project";
        register_test_scoped_bzlmod_repo_alias(
            "double_owner+".to_owned(),
            apparent.to_owned(),
            wanted.to_owned(),
        );

        assert_eq!(
            Some(wanted.to_owned()),
            resolve_test_scoped_bzlmod_repo_alias_for_current_cell(
                "double_owner++source+generated",
                apparent
            )
        );
    }

    #[test]
    fn scoped_bzlmod_repo_alias_resolves_double_plus_owner_without_separator() {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        reset_dynamic_bzlmod_state_for_project_root(tmp.path().to_path_buf());

        let apparent = "scoped_alias_test_double_plus_project_no_separator";
        let wanted = "double_owner_no_separator++toolchain+scoped_alias_test_double_plus_project";
        register_test_scoped_bzlmod_repo_alias(
            "double_owner_no_separator".to_owned(),
            apparent.to_owned(),
            wanted.to_owned(),
        );

        assert_eq!(
            Some(wanted.to_owned()),
            resolve_test_scoped_bzlmod_repo_alias_for_current_cell(
                "double_owner_no_separator++source+generated",
                apparent
            )
        );
    }

    #[test]
    fn scoped_bzlmod_repo_alias_resolves_apparent_generated_repo_cell() {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        reset_dynamic_bzlmod_state_for_project_root(tmp.path().to_path_buf());

        let apparent = "crates";
        let wanted = "rules_rs++crate+crates";
        register_test_scoped_bzlmod_repo_alias(
            "rules_rs+".to_owned(),
            apparent.to_owned(),
            wanted.to_owned(),
        );

        assert_eq!(
            Some(wanted.to_owned()),
            resolve_test_scoped_bzlmod_repo_alias_for_current_cell("crates__clap-4.5.60", apparent)
        );
    }

    #[test]
    fn scoped_bzlmod_repo_alias_resolves_archive_override_module_cell() {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        reset_dynamic_bzlmod_state_for_project_root(tmp.path().to_path_buf());

        let apparent = "rules_rust";
        let wanted = "rules_rust+";
        register_test_scoped_bzlmod_repo_alias(
            "rules_rs".to_owned(),
            apparent.to_owned(),
            wanted.to_owned(),
        );

        assert_eq!(
            Some(wanted.to_owned()),
            resolve_test_scoped_bzlmod_repo_alias_for_current_cell("rules_rs+override", apparent)
        );
    }

    #[test]
    fn scoped_bzlmod_repo_alias_last_registration_wins() {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        reset_dynamic_bzlmod_state_for_project_root(tmp.path().to_path_buf());

        let apparent = "rules_rust";
        register_test_scoped_bzlmod_repo_alias(
            "rules_rs".to_owned(),
            apparent.to_owned(),
            "rules_rs++rules_rust+rules_rust".to_owned(),
        );
        register_test_scoped_bzlmod_repo_alias(
            "rules_rs".to_owned(),
            apparent.to_owned(),
            "rules_rust+".to_owned(),
        );

        assert_eq!(
            Some("rules_rust+".to_owned()),
            resolve_test_scoped_bzlmod_repo_alias_for_current_cell("rules_rs", apparent)
        );
    }

    #[test]
    fn scoped_bzlmod_repo_alias_resolves_same_extension_dynamic_alias() {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        reset_dynamic_bzlmod_state_for_project_root(tmp.path().to_path_buf());

        register_test_dynamic_extension_cell_alias(
            "crates".to_owned(),
            "rules_rs++crate+crates".to_owned(),
        );
        register_test_dynamic_extension_cell_alias(
            "other".to_owned(),
            "rules_rs++other_ext+other".to_owned(),
        );

        assert_eq!(
            Some("rules_rs++crate+crates".to_owned()),
            resolve_test_scoped_bzlmod_repo_alias_for_current_cell(
                "rules_rs++crate+crates__clap-4.5.60",
                "crates",
            )
        );
        assert_eq!(
            None,
            resolve_test_scoped_bzlmod_repo_alias_for_current_cell(
                "rules_rs++crate+crates__clap-4.5.60",
                "other",
            )
        );
    }

    #[test]
    fn scoped_bzlmod_repo_alias_resolves_ordinary_module_owner() {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        reset_dynamic_bzlmod_state_for_project_root(tmp.path().to_path_buf());

        let apparent = "scoped_alias_test_module_project";
        let wanted = "tar.bzl++toolchains+scoped_alias_test_module_project";
        register_test_scoped_bzlmod_repo_alias(
            "ordinary_owner".to_owned(),
            apparent.to_owned(),
            wanted.to_owned(),
        );

        assert_eq!(
            Some(wanted.to_owned()),
            resolve_test_scoped_bzlmod_repo_alias_for_current_cell("ordinary_owner", apparent)
        );
    }

    #[test]
    fn dynamic_alias_does_not_override_exact_generated_extension_cell() -> slug_error::Result<()> {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir()?;
        reset_dynamic_bzlmod_state_for_project_root(tmp.path().to_path_buf());
        let generated = "override_owner++ext+generated_repo";
        let selected = "selected_repo+1.0.0";
        register_test_dynamic_extension_cell(
            generated.to_owned(),
            format!("bazel-external/{generated}"),
        );
        register_test_dynamic_extension_cell_alias(generated.to_owned(), selected.to_owned());
        register_test_dynamic_extension_cell_alias("repo_alias".to_owned(), selected.to_owned());

        let aliases = HashMap::new();
        let resolver = CellAliasResolver::new(CellName::testing_new("root"), aliases)?;

        assert_eq!(
            CellName::testing_new(generated),
            resolver.resolve(generated)?
        );
        assert_eq!(
            CellName::testing_new(selected),
            resolver.resolve("repo_alias")?
        );
        reset_dynamic_bzlmod_state_for_project_root(tmp.path().join("after"));
        Ok(())
    }

    #[test]
    fn external_symlink_relative_target_splits_bazel_style_paths() {
        assert_eq!(
            external_symlink_relative_target("bazel-external/rules_rust+0.69.0"),
            std::path::PathBuf::from("..")
                .join("bazel-external")
                .join("rules_rust+0.69.0")
        );
    }

    #[test]
    fn external_symlink_target_uses_canonical_repo_dir_when_available() {
        let tmp = tempfile::tempdir().unwrap();
        let project_root = tmp.path();
        let repo = project_root
            .join("bazel-external")
            .join("rules_rust+0.69.0");
        std::fs::create_dir_all(&repo).unwrap();

        assert_eq!(
            external_symlink_target(project_root, "bazel-external/rules_rust+0.69.0"),
            std::fs::canonicalize(repo).unwrap()
        );
    }

    #[test]
    fn desired_external_symlink_target_prefers_module_form_for_apparent_alias() {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let project_root = tmp.path();
        let bazel_external = project_root.join("bazel-external");
        let module_repo = bazel_external.join("rules_python+1.9.0");
        let extension_repo = bazel_external.join("rules_foreign_cc++ext+rules_python");
        std::fs::create_dir_all(&module_repo).unwrap();
        std::fs::create_dir_all(&extension_repo).unwrap();
        reset_dynamic_bzlmod_state_for_project_root(project_root.to_path_buf());

        let (target, priority) = desired_external_symlink_target(
            project_root,
            "rules_python",
            "bazel-external/rules_foreign_cc++ext+rules_python",
        );

        assert_eq!(priority, 3);
        assert_eq!(target, std::fs::canonicalize(module_repo).unwrap());
    }

    #[test]
    fn desired_external_symlink_target_keeps_canonical_extension_cell() {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let project_root = tmp.path();
        let bazel_external = project_root.join("bazel-external");
        let module_repo = bazel_external.join("rules_python+1.9.0");
        let extension_repo = bazel_external.join("rules_foreign_cc++ext+rules_python");
        std::fs::create_dir_all(&module_repo).unwrap();
        std::fs::create_dir_all(&extension_repo).unwrap();
        reset_dynamic_bzlmod_state_for_project_root(project_root.to_path_buf());

        let (target, priority) = desired_external_symlink_target(
            project_root,
            "rules_foreign_cc++ext+rules_python",
            "bazel-external/rules_foreign_cc++ext+rules_python",
        );

        assert_eq!(priority, 2);
        assert_eq!(target, std::fs::canonicalize(extension_repo).unwrap());
    }

    #[test]
    fn ensure_external_symlinks_for_cells_creates_canonical_module_link() {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let project_root = tmp.path();
        let module_repo = project_root.join("bazel-external").join("rules_python+");
        std::fs::create_dir_all(&module_repo).unwrap();
        reset_dynamic_bzlmod_state_for_project_root(project_root.to_path_buf());

        ensure_external_symlinks_for_cells(&[("rules_python", "bazel-external/rules_python+")]);

        let external = project_root.join("external");
        assert_eq!(
            std::fs::canonicalize(external.join("rules_python")).unwrap(),
            std::fs::canonicalize(&module_repo).unwrap()
        );
        assert_eq!(
            std::fs::canonicalize(external.join("rules_python+")).unwrap(),
            std::fs::canonicalize(&module_repo).unwrap()
        );
    }

    #[test]
    fn ensure_external_symlinks_with_explicit_root_ignores_stale_global_root() {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let project_root = tmp.path();
        let current_root_dir = project_root.join("current_root");
        let dep_dir = project_root.join("dep");
        std::fs::create_dir_all(&current_root_dir).unwrap();
        std::fs::create_dir_all(&dep_dir).unwrap();
        reset_dynamic_bzlmod_state_for_project_root(project_root.to_path_buf());
        ensure_external_symlinks_for_cells_with_root_cell(
            Some("current_root"),
            &[("current_root", "current_root"), ("dep", "dep")],
        );

        let external = project_root.join("external");
        assert!(!external.join("current_root").exists());
        assert_eq!(
            std::fs::canonicalize(external.join("dep")).unwrap(),
            std::fs::canonicalize(dep_dir).unwrap()
        );
    }

    #[test]
    fn module_form_priority_prefers_double_plus_canonical_over_collapsed_extension_repo() {
        assert!(
            module_form_priority("bazel-external/rules_python+1.9.0")
                > module_form_priority("bazel-external/rules_rs++crate+crates__diplomat-tool")
        );
        assert!(
            module_form_priority("bazel-external/rules_rs++crate+crates__diplomat-tool")
                > module_form_priority("bazel-external/rules_rs+crate+crates__diplomat-tool")
        );
    }

    #[test]
    fn resolve_external_symlink_target_resolves_relative_to_external_dir() {
        let external_dir = std::path::Path::new("workspace").join("external");
        assert_eq!(
            resolve_external_symlink_target(
                &external_dir,
                &std::path::PathBuf::from("..")
                    .join("bazel-external")
                    .join("rules_rust+0.69.0"),
            ),
            std::path::Path::new("workspace")
                .join("external")
                .join("..")
                .join("bazel-external")
                .join("rules_rust+0.69.0")
        );
    }

    #[test]
    fn repair_external_symlink_targets_collapses_symlink_chain() {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let external = root.join("external");
        let bazel_external = root.join("bazel-external");
        let cache_repo = root.join("cache").join("rules_rust");
        std::fs::create_dir_all(&external).unwrap();
        std::fs::create_dir_all(&bazel_external).unwrap();
        std::fs::create_dir_all(&cache_repo).unwrap();

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&cache_repo, bazel_external.join("rules_rust+0.69.0"))
                .unwrap();
            std::os::unix::fs::symlink(
                std::path::PathBuf::from("..")
                    .join("bazel-external")
                    .join("rules_rust+0.69.0"),
                external.join("rules_rust"),
            )
            .unwrap();
        }
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_dir(
                &cache_repo,
                bazel_external.join("rules_rust+0.69.0"),
            )
            .unwrap();
            std::os::windows::fs::symlink_dir(
                std::path::PathBuf::from("..")
                    .join("bazel-external")
                    .join("rules_rust+0.69.0"),
                external.join("rules_rust"),
            )
            .unwrap();
        }

        reset_dynamic_bzlmod_state_for_project_root(root.to_path_buf());
        repair_external_symlink_targets(root);

        assert_eq!(
            std::fs::canonicalize(external.join("rules_rust")).unwrap(),
            std::fs::canonicalize(cache_repo).unwrap()
        );
    }

    #[test]
    fn repair_external_symlink_targets_prefers_double_plus_canonical_repo() {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let external = root.join("external");
        let bazel_external = root.join("bazel-external");
        let collapsed = bazel_external.join("rules_rs+crate+crates__foo-1.0.0");
        let canonical = bazel_external.join("rules_rs++crate+crates__foo-1.0.0");
        std::fs::create_dir_all(&external).unwrap();
        std::fs::create_dir_all(&collapsed).unwrap();
        std::fs::create_dir_all(&canonical).unwrap();

        #[cfg(unix)]
        std::os::unix::fs::symlink(
            std::path::PathBuf::from("..")
                .join("bazel-external")
                .join("rules_rs+crate+crates__foo-1.0.0"),
            external.join("crates__foo-1.0.0"),
        )
        .unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(
            std::path::PathBuf::from("..")
                .join("bazel-external")
                .join("rules_rs+crate+crates__foo-1.0.0"),
            external.join("crates__foo-1.0.0"),
        )
        .unwrap();

        reset_dynamic_bzlmod_state_for_project_root(root.to_path_buf());
        repair_external_symlink_targets(root);

        assert_eq!(
            std::fs::canonicalize(external.join("crates__foo-1.0.0")).unwrap(),
            std::fs::canonicalize(canonical).unwrap()
        );
    }

    #[test]
    fn repair_external_symlink_targets_prefers_module_form_over_extension_repo() {
        let _guard = BZLMOD_APPARENT_ALIAS_CACHE_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let external = root.join("external");
        let bazel_external = root.join("bazel-external");
        let extension_repo = bazel_external.join("rules_rs++rules_rust+rules_rust");
        let module_repo = bazel_external.join("rules_rust+0.69.0");
        std::fs::create_dir_all(&external).unwrap();
        std::fs::create_dir_all(&extension_repo).unwrap();
        std::fs::create_dir_all(&module_repo).unwrap();

        #[cfg(unix)]
        std::os::unix::fs::symlink(
            std::path::PathBuf::from("..")
                .join("bazel-external")
                .join("rules_rs++rules_rust+rules_rust"),
            external.join("rules_rust"),
        )
        .unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(
            std::path::PathBuf::from("..")
                .join("bazel-external")
                .join("rules_rs++rules_rust+rules_rust"),
            external.join("rules_rust"),
        )
        .unwrap();

        reset_dynamic_bzlmod_state_for_project_root(root.to_path_buf());
        repair_external_symlink_targets(root);

        assert_eq!(
            std::fs::canonicalize(external.join("rules_rust")).unwrap(),
            std::fs::canonicalize(module_repo).unwrap()
        );
    }

    #[test]
    fn ensure_execroot_layout_creates_dir_only_symlinks() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        // Mix of files and directories at workspace root, including
        // names that frequently appear in Cargo crate runfiles trees.
        std::fs::create_dir(root.join("external")).unwrap();
        std::fs::create_dir(root.join("buck-out")).unwrap();
        std::fs::create_dir(root.join("lib")).unwrap();
        std::fs::create_dir(root.join("ci")).unwrap();
        std::fs::create_dir(root.join("docs")).unwrap();
        std::fs::create_dir(root.join("tests")).unwrap();
        std::fs::write(root.join("Cargo.toml"), "[package]\n").unwrap();
        std::fs::write(root.join("CHANGELOG.md"), "").unwrap();

        super::ensure_execroot_layout(root);

        let exec = root.join("execroot").join(root.file_name().unwrap());
        assert!(exec.is_dir(), "execroot should be a real directory");

        // Top-level dirs that don't collide with crate runfiles trees
        // are symlinked through.
        assert!(exec.join("external").is_dir());
        assert!(exec.join("buck-out").is_dir());
        assert!(exec.join("lib").is_dir());

        // Top-level dirs whose names collide with common crate
        // runfiles subdirs are skipped.
        assert!(!exec.join("ci").exists());
        assert!(!exec.join("docs").exists());
        assert!(!exec.join("tests").exists());

        // Leaf files are skipped — their presence at exec_root top
        // level would collide with the runfiles tree's leaf-file
        // entries (CHANGELOG.md, Cargo.toml, …).
        assert!(!exec.join("Cargo.toml").exists());
        assert!(!exec.join("CHANGELOG.md").exists());
    }

    #[test]
    fn ensure_execroot_layout_replaces_legacy_self_symlink() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let basename = root.file_name().unwrap();

        // Older slug builds installed `execroot/<basename> -> ..` —
        // simulate that and ensure the layout helper replaces it
        // with a real directory.
        let exec_dir = root.join("execroot");
        std::fs::create_dir_all(&exec_dir).unwrap();
        let legacy = exec_dir.join(basename);
        #[cfg(unix)]
        std::os::unix::fs::symlink("..", &legacy).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir("..", &legacy).unwrap();

        std::fs::create_dir(root.join("external")).unwrap();

        super::ensure_execroot_layout(root);

        assert!(legacy.is_dir(), "legacy symlink should be replaced");
        assert!(
            !legacy.symlink_metadata().unwrap().file_type().is_symlink(),
            "legacy symlink should be removed"
        );
        assert!(legacy.join("external").is_dir());
    }
}
