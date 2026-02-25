/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::fmt::Debug;
use std::sync::Arc;

use bumpalo::Bump;
use dupe::Dupe;
use dupe::IterDupedExt;
use hashbrown::HashTable;
use hashbrown::hash_table;
use kuro_common::package_listing::listing::PackageListing;
use kuro_core::cells::CellAliasResolver;
use kuro_core::cells::CellResolver;
use kuro_core::cells::cell_path::CellPath;
use kuro_core::cells::cell_path_with_allowed_relative_dir::CellPathWithAllowedRelativeDir;
use kuro_core::cells::name::CellName;
use kuro_core::cells::paths::CellRelativePath;
use kuro_core::cells::paths::CellRelativePathBuf;
use kuro_core::package::PackageLabel;
use kuro_core::package::package_relative_path::PackageRelativePath;
use kuro_core::package::package_relative_path::PackageRelativePathBuf;
use kuro_core::pattern::pattern::ParsedPattern;
use kuro_core::pattern::pattern::TargetParsingRel;
use kuro_core::pattern::pattern_type::PatternType;
use kuro_core::pattern::pattern_type::ProvidersPatternExtra;
use kuro_core::pattern::pattern_type::TargetPatternExtra;
use kuro_core::provider::label::ProvidersLabel;
use kuro_core::soft_error;
use kuro_core::target::label::interner::ConcurrentTargetLabelInterner;
use kuro_core::target::label::label::TargetLabel;
use kuro_core::target::name::TargetNameRef;
use kuro_node::attrs::coerced_attr::CoercedAttr;
use kuro_node::attrs::coerced_path::CoercedDirectory;
use kuro_node::attrs::coerced_path::CoercedPath;
use kuro_node::attrs::coercion_context::AttrCoercionContext;
use kuro_node::configuration::resolved::ConfigurationSettingKey;
use kuro_node::query::query_functions::CONFIGURED_GRAPH_QUERY_FUNCTIONS;
use kuro_query::query::syntax::simple::eval::error::QueryError;
use kuro_query::query::syntax::simple::functions::QueryLiteralVisitor;
use kuro_query_parser::Expr;
use kuro_query_parser::spanned::Spanned;
use kuro_util::arc_str::ArcSlice;
use kuro_util::arc_str::ArcStr;
use tracing::info;

use super::interner::AttrCoercionInterner;
use crate::attrs::coerce::arc_str_interner::ArcStrInterner;
use crate::attrs::coerce::str_hash::str_hash;

#[derive(Debug, kuro_error::Error)]
#[kuro(input)]
enum BuildAttrCoercionContextError {
    #[error("Expected a label, got the pattern `{0}`.")]
    RequiredLabel(String),
    #[error("Expected a package: `{0}` can only be specified in a build file.")]
    NotBuildFileContext(String),
    #[error("Expected file, but got a directory for path `{1}` in package `{0}`.")]
    SourceFileIsDirectory(PackageLabel, String),
    #[error("Source file `{1}` does not exist as a member of package `{0}`.")]
    SourceFileMissing(PackageLabel, String),
    #[error(
        "Directory `{1}` of package `{0}` may not cover any subpackages, but includes subpackage `{2}`."
    )]
    SourceDirectoryIncludesSubPackage(PackageLabel, String, PackageRelativePathBuf),
}

/// Try to construct a placeholder `ProvidersLabel` for a label with an unknown cell alias.
///
/// Parses `@cell_alias//pkg/path:target` format manually, bypassing the alias resolver.
/// This allows BUILD files to load even when a referenced cell is not registered (e.g.,
/// `dev_dependency` cells from upstream modules that are not available in user projects).
/// The placeholder label will only fail when that specific target is analyzed.
///
/// Returns `None` if the string cannot be parsed as an `@`-prefixed label.
fn try_make_placeholder_label(value: &str) -> Option<ProvidersLabel> {
    let stripped = value.strip_prefix('@')?;
    let sep_idx = stripped.find("//")?;
    let cell_alias = &stripped[..sep_idx];
    if cell_alias.is_empty() {
        return None;
    }
    let rest = &stripped[sep_idx + 2..];

    let (package_path, target_name) = if let Some(colon_idx) = rest.find(':') {
        let pkg = &rest[..colon_idx];
        let tgt = &rest[colon_idx + 1..];
        if tgt.is_empty() {
            return None;
        }
        (pkg, tgt)
    } else if rest.is_empty() {
        // "@repo//" → "@repo//:repo"
        ("", cell_alias)
    } else {
        // "@repo//pkg" → infer target from last path component
        let last = rest.rsplit('/').next()?;
        (rest, last)
    };

    let cell_name = CellName::unchecked_new(cell_alias).ok()?;
    let pkg = PackageLabel::new(cell_name, CellRelativePath::unchecked_new(package_path)).ok()?;
    let target = TargetLabel::new(pkg, TargetNameRef::unchecked_new(target_name));
    Some(ProvidersLabel::default_for(target))
}

/// An incomplete attr coercion context. Will be replaced with a real one later.
pub struct BuildAttrCoercionContext {
    /// Used to coerce targets
    cell_resolver: CellResolver,
    cell_name: CellName,
    cell_alias_resolver: CellAliasResolver,
    /// Used to resolve relative targets. This is present when a build file
    /// is being evaluated, however it is absent if an extension file is being
    /// evaluated. The latter case occurs when default values for attributes
    /// are coerced when a UDR is declared.
    enclosing_package: Option<(PackageLabel, PackageListing)>,
    /// This defines the limited scope in which we allow parsing patterns beginning with `../`
    current_dir_with_allowed_relative_dirs: CellPathWithAllowedRelativeDir,
    /// Does this package (if present) have a package boundary exception on it.
    package_boundary_exception: bool,
    /// Allocator for `label_cache`.
    alloc: Bump,
    global_label_interner: Arc<ConcurrentTargetLabelInterner>,
    /// Label coercion cache. We use `RawTable` where because `HashMap` API
    /// requires either computing hash twice (for get, then for insert) or
    /// allocating a key to perform a query using `entry` API.
    /// Strings are owned by `alloc`, using bump allocator makes evaluation 0.5% faster.
    label_cache: RefCell<HashTable<(u64, *const str, ProvidersLabel)>>,
    str_interner: ArcStrInterner,
    list_interner: AttrCoercionInterner<ArcSlice<CoercedAttr>>,
    // TODO(scottcao): Dict and selects need separate interners right now because
    // they have different key types. We can optimize this by interning keys and values
    // separately and use the same interner for dict and select values. This will also
    // reduce key duplication in selects since select keys are more likely to be deduplicated
    // than select values
    dict_interner: AttrCoercionInterner<ArcSlice<(CoercedAttr, CoercedAttr)>>,
    select_interner: AttrCoercionInterner<ArcSlice<(ConfigurationSettingKey, CoercedAttr)>>,
    /// Bazel-compatible output file label registry.
    /// Maps output filename → producing target name for the current package.
    /// Populated when a target with `attr.output()` attributes is recorded.
    /// Allows other targets to reference output files by bare filename as deps.
    output_file_registry: RefCell<HashMap<String, String>>,
}

impl Debug for BuildAttrCoercionContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BuildAttrCoercionContext")
            .finish_non_exhaustive()
    }
}

impl BuildAttrCoercionContext {
    fn new(
        cell_resolver: CellResolver,
        cell_name: CellName,
        cell_alias_resolver: CellAliasResolver,
        enclosing_package: Option<(PackageLabel, PackageListing)>,
        package_boundary_exception: bool,
        global_label_interner: Arc<ConcurrentTargetLabelInterner>,
        current_dir_with_allowed_relative_dirs: CellPathWithAllowedRelativeDir,
    ) -> Self {
        Self {
            cell_resolver,
            cell_name,
            cell_alias_resolver,
            enclosing_package,
            current_dir_with_allowed_relative_dirs,
            package_boundary_exception,
            alloc: Bump::new(),
            global_label_interner,
            label_cache: RefCell::new(HashTable::new()),
            str_interner: ArcStrInterner::new(),
            list_interner: AttrCoercionInterner::new(),
            dict_interner: AttrCoercionInterner::new(),
            select_interner: AttrCoercionInterner::new(),
            output_file_registry: RefCell::new(HashMap::new()),
        }
    }

    /// Register an output file label for Bazel-compatible output file reference resolution.
    /// When a rule with `attr.output()` is declared, its output filenames are registered here
    /// so other targets in the same package can reference them by bare filename as deps.
    pub fn register_output_file(&self, filename: String, target_name: String) {
        self.output_file_registry
            .borrow_mut()
            .insert(filename, target_name);
    }

    pub fn new_no_package(
        cell_resolver: CellResolver,
        cell_name: CellName,
        cell_alias_resolver: CellAliasResolver,
        global_label_interner: Arc<ConcurrentTargetLabelInterner>,
    ) -> Self {
        Self::new(
            cell_resolver,
            cell_name,
            cell_alias_resolver,
            None,
            false,
            global_label_interner,
            CellPathWithAllowedRelativeDir::backwards_relative_not_supported(CellPath::new(
                cell_name,
                CellRelativePathBuf::unchecked_new("".into()),
            )),
        )
    }

    pub fn new_with_package(
        cell_resolver: CellResolver,
        cell_alias_resolver: CellAliasResolver,
        enclosing_package: (PackageLabel, PackageListing),
        package_boundary_exception: bool,
        global_label_interner: Arc<ConcurrentTargetLabelInterner>,
        current_dir_with_allowed_relative_dirs: CellPathWithAllowedRelativeDir,
    ) -> Self {
        Self::new(
            cell_resolver,
            enclosing_package.0.cell_name(),
            cell_alias_resolver,
            Some(enclosing_package),
            package_boundary_exception,
            global_label_interner,
            current_dir_with_allowed_relative_dirs,
        )
    }

    pub fn parse_pattern<P: PatternType>(
        &self,
        value: &str,
    ) -> kuro_error::Result<ParsedPattern<P>> {
        let target_parsing_rel = self.target_parsing_rel();
        ParsedPattern::parse_not_relaxed(
            value,
            target_parsing_rel,
            &self.cell_resolver,
            &self.cell_alias_resolver,
        )
    }

    /// Parse pattern with Bazel-compatible default target inference.
    /// This enables patterns like `@repo//path` to be interpreted as `@repo//path:path`.
    pub fn parse_pattern_bazel_compat<P: PatternType>(
        &self,
        value: &str,
    ) -> kuro_error::Result<ParsedPattern<P>> {
        let target_parsing_rel = self.target_parsing_rel();
        ParsedPattern::parse_infer_target(
            value,
            target_parsing_rel,
            &self.cell_resolver,
            &self.cell_alias_resolver,
        )
    }

    fn target_parsing_rel(&self) -> TargetParsingRel<'_> {
        match self.enclosing_package.as_ref().map(|x| x.0.as_cell_path()) {
            Some(package) => {
                if self
                    .current_dir_with_allowed_relative_dirs
                    .has_allowed_relative_dir()
                {
                    TargetParsingRel::AllowRelative(
                        &self.current_dir_with_allowed_relative_dirs,
                        None,
                    )
                } else {
                    TargetParsingRel::AllowLimitedRelative(package)
                }
            }
            None => TargetParsingRel::RequireAbsolute(self.cell_name),
        }
    }

    fn coerce_label_no_cache(&self, value: &str) -> kuro_error::Result<ProvidersLabel> {
        // Bazel-compatible: `@repo` (bare repo ref without //) means `@repo//:repo`
        let expanded;
        let value = if value.starts_with('@') && !value.contains("//") {
            let repo = &value[1..]; // strip @
            let repo = repo.strip_suffix(':').unwrap_or(repo); // strip trailing : if present
            expanded = format!("@{}//:{}", repo, repo);
            expanded.as_str()
        } else {
            value
        };

        // Use Bazel-compatible parsing which allows `@repo//pkg` to mean `@repo//pkg:pkg`
        match self.parse_pattern_bazel_compat::<ProvidersPatternExtra>(value) {
            Ok(result) => match result {
                ParsedPattern::Target(package, target_name, providers) => {
                    return Ok(providers.into_providers_label(package, target_name.as_ref()));
                }
                _ => {
                    return Err(
                        BuildAttrCoercionContextError::RequiredLabel(value.to_owned()).into(),
                    );
                }
            },
            Err(_first_err) => {
                // Bazel-compatible: if the value looks like an absolute label with a cell
                // reference (e.g., "@com_google_absl_py//absl/testing:parameterized"),
                // create a placeholder label using an unchecked cell name. This lets BUILD
                // files containing targets with dev_dependency references (cells not available
                // in downstream projects) load successfully. Those specific targets will still
                // fail at analysis time, but other targets in the same package (e.g.,
                // python_toolchain) can be analyzed.
                if value.starts_with('@') && value.contains("//") {
                    if let Some(placeholder) = try_make_placeholder_label(value) {
                        return Ok(placeholder);
                    }
                }

                // Bazel-compatible: bare target names (e.g., "foo_bar") resolve relative to
                // the current package. Try prepending ":" if the original parse failed.
                // But skip this for bare names that are known source files in the package,
                // so that one_of(dep, source) coercion falls through to source coercion.
                let is_bare_name = !value.is_empty()
                    && !value.starts_with("//")
                    && !value.starts_with('@')
                    && !value.starts_with(':');
                let is_source_file = if is_bare_name {
                    if let Some((_, listing)) = &self.enclosing_package {
                        <&PackageRelativePath>::try_from(value)
                            .map(|p| listing.get_file(p).is_some())
                            .unwrap_or(false)
                    } else {
                        false
                    }
                } else {
                    false
                };
                if is_bare_name && !is_source_file {
                    // Bazel-compatible: check if this bare name is a known output file
                    // from another target in this package (declared with attr.output()).
                    let output_target = self.output_file_registry.borrow().get(value).cloned();
                    let target_name_to_use = output_target.as_deref().unwrap_or(value);
                    let adjusted = format!(":{}", target_name_to_use);
                    match self.parse_pattern_bazel_compat::<ProvidersPatternExtra>(&adjusted) {
                        Ok(ParsedPattern::Target(package, target_name, providers)) => {
                            Ok(providers.into_providers_label(package, target_name.as_ref()))
                        }
                        _ => Err(
                            BuildAttrCoercionContextError::RequiredLabel(value.to_owned()).into(),
                        ),
                    }
                } else {
                    Err(BuildAttrCoercionContextError::RequiredLabel(value.to_owned()).into())
                }
            }
        }
    }

    fn require_enclosing_package(
        &self,
        msg: &str,
    ) -> kuro_error::Result<&(PackageLabel, PackageListing)> {
        self.enclosing_package.as_ref().ok_or_else(|| {
            BuildAttrCoercionContextError::NotBuildFileContext(msg.to_owned()).into()
        })
    }
}

impl AttrCoercionContext for BuildAttrCoercionContext {
    fn coerce_providers_label(&self, value: &str) -> kuro_error::Result<ProvidersLabel> {
        let hash = str_hash(value);
        let mut label_cache = self.label_cache.borrow_mut();

        match label_cache.entry(
            hash,
            |(h, v, _)| *h == hash && value == unsafe { &**v },
            |(h, _, _)| *h,
        ) {
            hash_table::Entry::Occupied(e) => Ok(e.get().2.dupe()),
            hash_table::Entry::Vacant(e) => {
                let label = self.coerce_label_no_cache(value)?;

                let (target_label, providers) = label.into_parts();
                let target_label = self.global_label_interner.intern(target_label);
                let label = ProvidersLabel::new(target_label, providers);

                e.insert((hash, self.alloc.alloc_str(value), label.dupe()));
                Ok(label)
            }
        }
    }

    fn intern_str(&self, value: &str) -> ArcStr {
        self.str_interner.intern(value)
    }

    fn intern_list(&self, value: Vec<CoercedAttr>) -> ArcSlice<CoercedAttr> {
        self.list_interner.intern(value)
    }

    fn intern_dict(
        &self,
        value: Vec<(CoercedAttr, CoercedAttr)>,
    ) -> ArcSlice<(CoercedAttr, CoercedAttr)> {
        self.dict_interner.intern(value)
    }

    fn intern_select(
        &self,
        value: Vec<(ConfigurationSettingKey, CoercedAttr)>,
    ) -> ArcSlice<(ConfigurationSettingKey, CoercedAttr)> {
        self.select_interner.intern(value)
    }

    fn coerce_path(&self, value: &str, allow_directory: bool) -> kuro_error::Result<CoercedPath> {
        let path = <&PackageRelativePath>::try_from(value)?;
        let (package, listing) = self.require_enclosing_package(value)?;

        if let Some(path) = listing.get_file(path) {
            return Ok(CoercedPath::File(path));
        }

        // TODO: Make the warnings below into errors
        if let Some(path) = listing.get_dir(path) {
            if !allow_directory {
                return Err(BuildAttrCoercionContextError::SourceFileIsDirectory(
                    package.dupe(),
                    value.to_owned(),
                )
                .into());
            } else if let Some(subpackage) = listing.subpackages_within(&path).next() {
                let e = BuildAttrCoercionContextError::SourceDirectoryIncludesSubPackage(
                    package.dupe(),
                    value.to_owned(),
                    subpackage.to_owned(),
                );
                if self.package_boundary_exception {
                    info!("{} (could be due to a package boundary violation)", e);
                } else {
                    soft_error!("source_directory_includes_subpackage", e.into())?;
                }
            }
            let files = listing.files_within(&path).duped().collect();
            Ok(CoercedPath::Directory(Box::new(CoercedDirectory {
                dir: path,
                files,
            })))
        } else {
            let e =
                BuildAttrCoercionContextError::SourceFileMissing(package.dupe(), value.to_owned());
            if self.package_boundary_exception {
                info!("{} (could be due to a package boundary violation)", e);
            } else {
                soft_error!("source_file_missing", e.into(), quiet: true)?;
            }

            Ok(CoercedPath::File(path.to_arc()))
        }
    }

    fn coerce_target_pattern(
        &self,
        pattern: &str,
    ) -> kuro_error::Result<ParsedPattern<TargetPatternExtra>> {
        self.parse_pattern(pattern)
    }

    fn visit_query_function_literals<'q>(
        &self,
        visitor: &mut dyn QueryLiteralVisitor<'q>,
        expr: &Spanned<Expr<'q>>,
        query: &'q str,
    ) -> kuro_error::Result<()> {
        CONFIGURED_GRAPH_QUERY_FUNCTIONS
            .get()?
            .visit_literals(visitor, expr)
            .map_err(|e| QueryError::convert_error(e, query))?;
        Ok(())
    }
}
