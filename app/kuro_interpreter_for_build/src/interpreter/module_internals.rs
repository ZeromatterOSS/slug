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
use std::cell::RefMut;
use std::fmt;
use std::fmt::Debug;
use std::mem;
use std::sync::Arc;

use dupe::Dupe;
use kuro_common::package_listing::listing::PackageListing;
use kuro_core::build_file_path::BuildFilePath;
use kuro_core::bzl::ImportPath;
use kuro_core::package::package_relative_path::PackageRelativePath;
use kuro_core::target::name::TargetNameRef;
use kuro_events::dispatch::console_message;
use kuro_interpreter::package_imports::ImplicitImport;
use kuro_node::attrs::attr_type::list::ListLiteral;
use kuro_node::attrs::coerced_attr::CoercedAttr;
use kuro_node::attrs::coerced_path::CoercedPath;
use kuro_node::nodes::eval_result::EvaluationResult;
use kuro_node::nodes::targets_map::TargetsMap;
use kuro_node::nodes::targets_map::TargetsMapRecordError;
use kuro_node::nodes::unconfigured::TargetNode;
use kuro_node::oncall::Oncall;
use kuro_node::package::Package;
use kuro_node::super_package::SuperPackage;
use kuro_node::visibility::VisibilitySpecification;
use kuro_util::arc_str::ArcSlice;
use starlark::environment::FrozenModule;
use starlark::values::OwnedFrozenValue;

use crate::attrs::coerce::ctx::BuildAttrCoercionContext;
use crate::interpreter::globspec::GlobSpec;
use crate::interpreter::native_rules::create_native_target_node;
use crate::interpreter::native_rules::rule_defs;

impl From<ModuleInternals> for EvaluationResult {
    // TODO(cjhopman): Let's make this an `into_evaluation_result()` on ModuleInternals instead.
    fn from(internals: ModuleInternals) -> Self {
        let ModuleInternals {
            state,
            imports,
            buildfile_path,
            super_package,
            package_listing,
            attr_coercion_context,
            build_file_default_visibility,
            ..
        } = internals;
        let recorder = match state.into_inner() {
            State::BeforeTargets(_) => TargetsRecorder::new(),
            State::RecordingTargets(RecordingTargets {
                mut recorder,
                package,
            }) => {
                // Bazel compatibility: source files in a package are implicitly available
                // as targets. Register any file in the package that doesn't already have
                // a corresponding rule target.
                let default_vis = build_file_default_visibility
                    .into_inner()
                    .unwrap_or_else(|| super_package.visibility().dupe());
                register_implicit_source_file_targets(
                    &package_listing,
                    &package,
                    &attr_coercion_context,
                    &default_vis,
                    &mut recorder,
                );
                recorder
            }
        };
        EvaluationResult::new(buildfile_path, imports, super_package, recorder.take())
    }
}

#[derive(Debug, Default)]
struct BeforeTargets {
    oncall: Option<Oncall>,
    has_read_oncall: bool,
}

#[derive(Debug)]
struct RecordingTargets {
    package: Arc<Package>,
    recorder: TargetsRecorder,
}

#[derive(Debug)]
enum State {
    /// No targets recorded yet, `oncall` call is allowed unless it was already called.
    BeforeTargets(BeforeTargets),
    /// First target seen.
    RecordingTargets(RecordingTargets),
}

/// ModuleInternals contains the module/package-specific information for
/// evaluating build files. Built-in functions that need access to
/// package-specific information or objects can get them by acquiring the
/// ModuleInternals.
#[derive(Debug)]
pub struct ModuleInternals {
    attr_coercion_context: BuildAttrCoercionContext,
    buildfile_path: Arc<BuildFilePath>,
    /// Have you seen an oncall annotation yet
    state: RefCell<State>,
    /// Directly imported modules.
    imports: Vec<ImportPath>,
    package_implicits: Option<PackageImplicits>,
    record_target_call_stacks: bool,
    skip_targets_with_duplicate_names: bool,
    /// The files owned by this directory. Is `None` for .bzl files.
    package_listing: PackageListing,
    pub(crate) super_package: SuperPackage,
    /// Bazel-style BUILD file default visibility set via `package(default_visibility=...)`
    /// This is None if no package() call was made with default_visibility.
    pub(crate) build_file_default_visibility: RefCell<Option<VisibilitySpecification>>,
}

#[derive(Debug)]
pub(crate) struct PackageImplicits {
    import_spec: Arc<ImplicitImport>,
    env: FrozenModule,
}

impl PackageImplicits {
    pub(crate) fn new(import_spec: Arc<ImplicitImport>, env: FrozenModule) -> Self {
        Self { import_spec, env }
    }

    fn lookup(&self, name: &str) -> Option<OwnedFrozenValue> {
        self.env
            .get_option(self.import_spec.lookup_alias(name))
            .ok()
            .flatten()
    }
}

#[derive(Debug, kuro_error::Error)]
#[kuro(input)]
enum OncallErrors {
    #[error("Called `oncall` after one or more targets were declared, `oncall` must be first.")]
    OncallAfterTargets,
    #[error("Called `oncall` more than once in the file.")]
    DuplicateOncall,
    #[error("Called `oncall` after calling `read_oncall`, `oncall` must be first.")]
    AfterReadOncall,
}

impl ModuleInternals {
    pub(crate) fn new(
        attr_coercion_context: BuildAttrCoercionContext,
        buildfile_path: Arc<BuildFilePath>,
        imports: Vec<ImportPath>,
        package_implicits: Option<PackageImplicits>,
        record_target_call_stacks: bool,
        skip_targets_with_duplicate_names: bool,
        package_listing: PackageListing,
        super_package: SuperPackage,
    ) -> Self {
        Self {
            attr_coercion_context,
            buildfile_path,
            state: RefCell::new(State::BeforeTargets(BeforeTargets::default())),
            imports,
            package_implicits,
            record_target_call_stacks,
            skip_targets_with_duplicate_names,
            package_listing,
            super_package,
            build_file_default_visibility: RefCell::new(None),
        }
    }

    pub(crate) fn attr_coercion_context(&self) -> &BuildAttrCoercionContext {
        &self.attr_coercion_context
    }

    /// Gets the effective default visibility for targets in this package.
    /// First checks for a BUILD file's `package(default_visibility=...)` setting,
    /// then falls back to the super_package visibility (from PACKAGE files).
    pub fn default_visibility(&self) -> VisibilitySpecification {
        if let Some(ref vis) = *self.build_file_default_visibility.borrow() {
            vis.dupe()
        } else {
            self.super_package.visibility().dupe()
        }
    }

    /// Sets the BUILD file's default visibility from `package(default_visibility=...)`.
    pub fn set_build_file_default_visibility(&self, visibility: VisibilitySpecification) {
        *self.build_file_default_visibility.borrow_mut() = Some(visibility);
    }

    pub fn record(&self, target_node: TargetNode) -> kuro_error::Result<()> {
        match self.recording_targets().recorder.record(target_node) {
            Ok(()) => Ok(()),
            Err(e @ TargetsMapRecordError::RegisteredTargetTwice { .. }) => {
                if self.skip_targets_with_duplicate_names {
                    console_message(e.to_string());
                    Ok(())
                } else {
                    Err(e.into())
                }
            }
        }
    }

    pub(crate) fn set_oncall(&self, name: &str) -> kuro_error::Result<()> {
        match &mut *self.state.borrow_mut() {
            State::BeforeTargets(x) => match x.oncall {
                _ if x.has_read_oncall => Err(OncallErrors::AfterReadOncall.into()),
                Some(_) => Err(OncallErrors::DuplicateOncall.into()),
                None => {
                    x.oncall = Some(Oncall::new(name));
                    Ok(())
                }
            },
            State::RecordingTargets(..) => {
                // We require oncall to be first both so users can find it,
                // and so we can propagate it to all targets more easily.
                Err(OncallErrors::OncallAfterTargets.into())
            }
        }
    }

    pub(crate) fn get_oncall(&self) -> Option<Oncall> {
        match &mut *self.state.borrow_mut() {
            State::BeforeTargets(x) => {
                x.has_read_oncall = true;
                x.oncall.dupe()
            }
            State::RecordingTargets(t) => t.package.oncall.dupe(),
        }
    }

    fn recording_targets(&self) -> RefMut<'_, RecordingTargets> {
        RefMut::map(self.state.borrow_mut(), |state| {
            loop {
                match state {
                    State::BeforeTargets(BeforeTargets { oncall, .. }) => {
                        let oncall = mem::take(oncall);
                        *state = State::RecordingTargets(RecordingTargets {
                            package: Arc::new(Package {
                                buildfile_path: self.buildfile_path.dupe(),
                                oncall,
                            }),
                            recorder: TargetsRecorder::new(),
                        });
                    }
                    State::RecordingTargets(r) => return r,
                }
            }
        })
    }

    pub(crate) fn target_exists(&self, name: &str) -> bool {
        self.recording_targets()
            .recorder
            .targets
            .contains_key(TargetNameRef::unchecked_new(name))
    }

    /// Returns the names of all targets recorded so far.
    /// Used by `native.existing_rules()` for Bazel compatibility.
    pub(crate) fn get_target_names(&self) -> Vec<String> {
        self.recording_targets()
            .recorder
            .targets
            .keys()
            .map(|name| name.as_str().to_owned())
            .collect()
    }

    /// Returns (name, kind) pairs for all targets recorded so far.
    /// Used by `native.existing_rules()` for Bazel compatibility.
    pub(crate) fn get_targets_with_kind(&self) -> Vec<(String, String)> {
        self.recording_targets()
            .recorder
            .targets
            .iter()
            .map(|(name, node)| (name.as_str().to_owned(), node.rule_type().name().to_owned()))
            .collect()
    }

    /// Returns full attribute data for all targets recorded so far.
    /// Used by `native.existing_rules()` for Bazel compatibility.
    /// Returns Vec of (name, kind, attrs_json) where attrs_json is a map of attribute name → serde_json::Value.
    pub(crate) fn get_targets_with_attrs(
        &self,
    ) -> Vec<(String, String, Vec<(String, serde_json::Value)>)> {
        use kuro_node::attrs::fmt_context::AttrFmtContext;
        use kuro_node::attrs::inspect_options::AttrInspectOptions;
        use kuro_query::query::environment::AttrFmtOptions;

        let recording = self.recording_targets();
        let package_label = recording.package.buildfile_path.package();
        let ctx = AttrFmtContext {
            package: Some(package_label),
            options: AttrFmtOptions::default(),
        };

        recording
            .recorder
            .targets
            .iter()
            .map(|(name, node)| {
                let kind = node.rule_type().name().to_owned();
                let attrs: Vec<(String, serde_json::Value)> = node
                    .attrs(AttrInspectOptions::DefinedOnly)
                    .filter_map(|a| {
                        // Skip internal attrs that start with __
                        if a.name.starts_with("__") {
                            return None;
                        }
                        match a.value.to_json(&ctx) {
                            Ok(v) => Some((a.name.to_owned(), v)),
                            Err(_) => None,
                        }
                    })
                    .collect();
                (name.as_str().to_owned(), kind, attrs)
            })
            .collect()
    }

    /// Returns full attribute data for a single target, or None if not found.
    /// Used by `native.existing_rule()` for Bazel compatibility.
    pub(crate) fn get_target_with_attrs(
        &self,
        name: &str,
    ) -> Option<(String, Vec<(String, serde_json::Value)>)> {
        use kuro_node::attrs::fmt_context::AttrFmtContext;
        use kuro_node::attrs::inspect_options::AttrInspectOptions;
        use kuro_query::query::environment::AttrFmtOptions;

        let recording = self.recording_targets();
        let package_label = recording.package.buildfile_path.package();
        let ctx = AttrFmtContext {
            package: Some(package_label),
            options: AttrFmtOptions::default(),
        };

        recording
            .recorder
            .targets
            .get(TargetNameRef::unchecked_new(name))
            .map(|node| {
                let kind = node.rule_type().name().to_owned();
                let attrs: Vec<(String, serde_json::Value)> = node
                    .attrs(AttrInspectOptions::DefinedOnly)
                    .filter_map(|a| {
                        if a.name.starts_with("__") {
                            return None;
                        }
                        match a.value.to_json(&ctx) {
                            Ok(v) => Some((a.name.to_owned(), v)),
                            Err(_) => None,
                        }
                    })
                    .collect();
                (kind, attrs)
            })
    }

    /// Returns the rule kind for a target, or None if not found.
    /// Used by `native.existing_rule()` for Bazel compatibility.
    pub(crate) fn get_target_kind(&self, name: &str) -> Option<String> {
        self.recording_targets()
            .recorder
            .targets
            .get(TargetNameRef::unchecked_new(name))
            .map(|node| node.rule_type().name().to_owned())
    }

    pub fn buildfile_path(&self) -> &Arc<BuildFilePath> {
        &self.buildfile_path
    }

    pub fn package(&self) -> Arc<Package> {
        self.recording_targets().package.dupe()
    }

    pub(crate) fn get_package_implicit(&self, name: &str) -> Option<OwnedFrozenValue> {
        self.package_implicits
            .as_ref()
            .and_then(|implicits| implicits.lookup(name))
    }

    pub fn record_target_call_stacks(&self) -> bool {
        self.record_target_call_stacks
    }

    pub(crate) fn resolve_glob<'a>(
        &'a self,
        spec: &'a GlobSpec,
    ) -> impl Iterator<Item = &'a PackageRelativePath> {
        spec.resolve_glob(self.package_listing.files())
    }

    pub(crate) fn sub_packages(&self) -> impl Iterator<Item = &PackageRelativePath> {
        self.package_listing
            .subpackages_within(PackageRelativePath::empty())
    }
}

// Records the targets declared when evaluating a build file.
struct TargetsRecorder {
    targets: TargetsMap,
}

impl Debug for TargetsRecorder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TargetsRecorder").finish_non_exhaustive()
    }
}

impl TargetsRecorder {
    fn new() -> Self {
        Self {
            targets: TargetsMap::new(),
        }
    }

    fn record(&mut self, target_node: TargetNode) -> Result<(), TargetsMapRecordError> {
        self.targets.record(target_node)
    }

    fn take(self) -> TargetsMap {
        self.targets
    }
}

/// Bazel compatibility: register implicit source file targets for files in the package.
///
/// In Bazel, every source file in a package directory is implicitly available as a
/// build target. For example, if a package contains `foo.txt`, it can be referenced
/// as `:foo.txt` in BUILD files without needing `exports_files(["foo.txt"])`.
///
/// This creates a filegroup target for each file that doesn't already have a rule
/// target with the same name.
fn register_implicit_source_file_targets(
    package_listing: &PackageListing,
    package: &Arc<Package>,
    coercion_ctx: &BuildAttrCoercionContext,
    default_visibility: &VisibilitySpecification,
    recorder: &mut TargetsRecorder,
) {
    let buildfile_name = package_listing.buildfile().as_str();
    let mut count = 0usize;

    for file_path in package_listing.files().files() {
        let file_str = file_path.as_str();

        // Skip the build file itself
        if file_str == buildfile_name {
            continue;
        }

        // Use the file path as the target name
        let target_name = file_str;

        // Skip if target name is not valid for Kuro
        if TargetNameRef::new(target_name).is_err() {
            continue;
        }

        // Skip if a rule target with this name already exists
        if recorder
            .targets
            .contains_key(TargetNameRef::unchecked_new(target_name))
        {
            continue;
        }

        // Construct the source file attribute directly (without Starlark coercion).
        // filegroup srcs is AttrType::list(one_of(dep=0, source=1)), so the
        // source variant index is 1.
        let source_file = CoercedAttr::SourceFile(CoercedPath::File(file_path.to_arc()));
        let one_of = CoercedAttr::OneOf(Box::new(source_file), 1);
        let srcs_list = CoercedAttr::List(ListLiteral(ArcSlice::new([one_of])));
        let data_list = CoercedAttr::List(ListLiteral(ArcSlice::default()));

        let target_node = match create_native_target_node(
            rule_defs::FILEGROUP_RULE.clone(),
            package.clone(),
            target_name,
            vec![
                ("srcs".to_owned(), srcs_list),
                ("data".to_owned(), data_list),
            ],
            &[],
            coercion_ctx,
            default_visibility,
        ) {
            Ok(node) => node,
            Err(_) => continue,
        };

        // Silently skip if registration fails (e.g., duplicate target)
        if recorder.record(target_node).is_ok() {
            count += 1;
        }
    }

    if count > 0 {
        tracing::debug!(
            "Registered {} implicit source file targets for package '{}'",
            count,
            package.buildfile_path.package()
        );
    }
}
