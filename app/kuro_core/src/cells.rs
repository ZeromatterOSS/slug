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

static DYNAMIC_EXTENSION_CELL_ALIASES: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<String, String>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

/// Plan 36: dynamic-cell sibling registry that carries the
/// `ExtensionRepoCellSetup` alongside the path, so that
/// `get_or_create_dynamic_cell` can wire `ExternalCellOrigin::ExtensionRepo`
/// onto the synthesized `CellInstance`. With the origin set, the existing
/// file-ops layer routes accesses through
/// `kuro_external_cells::extension_repo::get_file_ops_delegate`, which
/// drives lazy DICE materialization on first read — the same path
/// `mark_external_cell` produces for `use_repo`'d extension cells at
/// startup.
///
/// Stored separately from `DYNAMIC_EXTENSION_CELLS` so the older path-only
/// callers keep working unchanged.
static DYNAMIC_EXTENSION_CELL_SETUPS: std::sync::LazyLock<
    std::sync::Mutex<
        std::collections::HashMap<String, crate::cells::external::ExtensionRepoCellSetup>,
    >,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

/// Global project root for dynamic cell filesystem operations.
static DYNAMIC_PROJECT_ROOT: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

/// Register a dynamically-discovered extension repo cell.
/// Called after extension execution materializes repos.
pub fn register_dynamic_extension_cell(canonical_name: String, path: String) {
    if let Ok(mut cells) = DYNAMIC_EXTENSION_CELLS.lock() {
        cells.insert(canonical_name.clone(), path.clone());
    }
    // Always write the canonical-name symlink (`<owner>+<ext>+<repo>`).
    // Action commands, file-watch paths, and `bazel-external/` consumers
    // all use the canonical form as the source of truth.
    ensure_external_symlink(&canonical_name, &path);

    // Apparent-name aliasing (e.g. consuming modules' `use_repo`) is
    // handled elsewhere through proper alias maps. Do NOT also write a
    // symlink under the spoke's last `+`-segment as the apparent name:
    // for `rules_foreign_cc+ext+rules_python` that segment is
    // `rules_python`, which collides with the bzlmod module
    // `rules_python+1.9.0`. The collision used to silently overwrite the
    // module's symlink so action paths resolved to the wrong directory
    // (e.g. rules_python's bootstrap templates went missing).
}

pub fn register_dynamic_extension_cell_alias(apparent_name: String, canonical_name: String) {
    if let Ok(mut aliases) = DYNAMIC_EXTENSION_CELL_ALIASES.lock() {
        aliases.insert(apparent_name, canonical_name);
    }
}

pub fn resolve_dynamic_extension_cell_alias(apparent_name: &str) -> Option<String> {
    DYNAMIC_EXTENSION_CELL_ALIASES
        .lock()
        .ok()
        .and_then(|aliases| aliases.get(apparent_name).cloned())
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
    if let Ok(mut setups) = DYNAMIC_EXTENSION_CELL_SETUPS.lock() {
        setups.insert(canonical_name.clone(), setup);
    }
    register_dynamic_extension_cell(canonical_name, path);
}

/// Lookup the `ExtensionRepoCellSetup` for a dynamic extension cell, if
/// one was registered alongside the path mapping.
pub fn get_dynamic_extension_cell_setup(
    name: &str,
) -> Option<crate::cells::external::ExtensionRepoCellSetup> {
    DYNAMIC_EXTENSION_CELL_SETUPS
        .lock()
        .ok()
        .and_then(|m| m.get(name).cloned())
}

/// Set the project root for dynamic cell filesystem scanning.
pub fn set_dynamic_project_root(root: std::path::PathBuf) {
    ensure_execroot_layout(&root);
    let _ = DYNAMIC_PROJECT_ROOT.set(root);
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

    // Replace any previous symlink-style execroot (older kuro builds
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
/// Score a `bazel-external/<basename>` path for `external/<apparent>` symlink
/// precedence. Higher = more preferred.
///
/// When multiple cells share the same apparent name (e.g. `rules_python`
/// is both the bzlmod module `rules_python+1.9.0` AND an extension spoke
/// `rules_foreign_cc+ext+rules_python`), the symlink should point at the
/// bzlmod module form because that's where bazel_dep'd consumers expect
/// templates and other source files to live. Extension spokes get their
/// own symlinks under their canonical names elsewhere.
///
///   `rules_python+1.9.0`               → 2 (module form, name+version)
///   `rules_foreign_cc+ext+rules_python` → 1 (extension spoke, 3 segments)
///   `rules_python`                     → 0 (no version, ambiguous)
fn module_form_priority(cell_path: &str) -> u8 {
    let basename = std::path::Path::new(cell_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let plus_count = basename.matches('+').count();
    match plus_count {
        1 => 2,
        n if n >= 2 => 1,
        _ => 0,
    }
}

pub fn ensure_external_symlink(cell_name: &str, cell_path: &str) {
    let project_root = match DYNAMIC_PROJECT_ROOT.get() {
        Some(root) => root.clone(),
        None => return,
    };
    let external_dir = project_root.join("external");
    let link_path = external_dir.join(cell_name);
    let desired_target = std::path::PathBuf::from("..").join(cell_path);
    let desired_priority = module_form_priority(cell_path);
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
                    if current_priority > desired_priority {
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
                        std::fs::canonicalize(&desired_target),
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
                            return;
                        }
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
        let dynamic_lookup = {
            let exact =
                get_dynamic_extension_cell(cell.as_str()).map(|p| (cell.as_str().to_owned(), p));
            exact.or_else(|| {
                let suffix = format!("+{}", cell.as_str());
                if let Ok(cells) = DYNAMIC_EXTENSION_CELLS.lock() {
                    for (canonical, path) in cells.iter() {
                        if canonical.ends_with(&suffix) {
                            return Some((canonical.clone(), path.clone()));
                        }
                    }
                }
                None
            })
        };
        if let Some((canonical, path)) = dynamic_lookup {
            // Auto-register this cell
            if let Ok(rel_path) = ProjectRelativePath::new(&path) {
                let cell_path = CellRootPathBuf::new(rel_path.to_owned());
                let nested = nested::NestedCells::from_cell_roots(&[], &*cell_path);
                // Plan 36: if the canonical name has a registered
                // ExtensionRepoCellSetup, attach it as the external origin
                // so file ops route through the lazy-materialization path.
                let external = get_dynamic_extension_cell_setup(&canonical)
                    .map(crate::cells::external::ExternalCellOrigin::ExtensionRepo);
                if let Ok(instance) = CellInstance::new(cell, cell_path, external, nested) {
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

        // Older kuro builds installed `execroot/<basename> -> ..` —
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
