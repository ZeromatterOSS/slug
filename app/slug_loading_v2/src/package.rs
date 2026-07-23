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
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::PackageIdentifier;
use starlark::any::ProvidesStaticType;
use starlark::environment::Globals;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Arguments;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::AllocFrozenValue;
use starlark::values::Freeze;
use starlark::values::FreezeResult;
use starlark::values::Freezer;
use starlark::values::FrozenHeap;
use starlark::values::FrozenValue;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Trace;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark::values::list::ListRef;
use starlark::values::list_or_tuple::UnpackListOrTuple;
use starlark::values::none::NoneType;
use starlark::values::starlark_value;
use starlark_map::small_map::SmallMap;

use crate::bzl_module::BzlModuleIdentity;
use crate::bzl_module::FrozenBzlLifetimeEntry;
use crate::glob::GlobSpec;
use crate::glob::PackageListing;
use crate::glob::expand_glob;
use crate::provider::AnalysisBuiltinCallable;
use crate::provider::UserProviderCallable;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageFile {
    pub package: PackageIdentifier,
    pub build_file: PathBuf,
}

/// The package result currently needed by the first BUILD-loading vertical.
///
/// This remains a loading-stage value: configured targets, providers, and
/// action declarations are built by Stage 6.
#[derive(Debug, Clone, Allocative)]
pub struct LoadedPackage {
    pub package_dir: PathBuf,
    pub build_file: PathBuf,
    pub default_visibility: Vec<String>,
    pub targets: Vec<PackageTarget>,
    pub used_globs: Vec<GlobSpec>,
    /// Ordered label-first direct `.bzl` roots for this BUILD evaluation.
    pub direct_load_roots: Arc<[BzlModuleIdentity]>,
    /// Flat label-first first-seen closure of all direct roots.
    pub reachable_loads: Arc<[BzlModuleIdentity]>,
    /// SHA-256 over ordered direct semantic roots and their fingerprints.
    pub load_fingerprint: [u8; 32],
    #[allow(dead_code)] // Ownership only; frozen rule values borrow these heaps.
    retained_bzl_modules: Arc<[FrozenBzlLifetimeEntry]>,
}

impl PartialEq for LoadedPackage {
    fn eq(&self, other: &Self) -> bool {
        self.package_dir == other.package_dir
            && self.build_file == other.build_file
            && self.default_visibility == other.default_visibility
            && self.targets == other.targets
            && self.used_globs == other.used_globs
            && self.direct_load_roots == other.direct_load_roots
            && self.reachable_loads == other.reachable_loads
            && self.load_fingerprint == other.load_fingerprint
    }
}

impl Eq for LoadedPackage {}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct PackageTarget {
    pub name: String,
    pub kind: PackageTargetKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum PackageTargetKind {
    ExportedFile,
    Filegroup {
        srcs: Vec<String>,
    },
    Alias {
        actual: String,
    },
    /// A target declared by a Starlark `rule()` definition.
    ///
    /// Stage 4 records the declaration and retains the frozen implementation.
    /// Stage 6 owns evaluating it with a configured target context.
    StarlarkRule(StarlarkRuleImplementation),
}

/// The frozen rule implementation retained for configured-target analysis.
/// The containing package keeps its source `.bzl` module alive.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct StarlarkRuleImplementation {
    #[allocative(skip)]
    implementation: FrozenValue,
    dependencies: Arc<[CanonicalLabel]>,
}

impl StarlarkRuleImplementation {
    pub fn frozen_value(&self) -> FrozenValue {
        self.implementation
    }

    pub fn dependencies(&self) -> &[CanonicalLabel] {
        &self.dependencies
    }
}

#[derive(Debug, Default)]
struct PackageState {
    default_visibility: Vec<String>,
    targets: SmallMap<String, PackageTargetKind>,
    used_globs: Vec<GlobSpec>,
}

#[derive(Debug, ProvidesStaticType)]
pub(crate) struct PackageRecorder {
    listing: PackageListing,
    package: CompactString,
    state: RefCell<PackageState>,
}

impl PackageRecorder {
    pub(crate) fn new(listing: PackageListing, package: impl Into<CompactString>) -> Self {
        Self {
            listing,
            package: package.into(),
            state: RefCell::new(PackageState::default()),
        }
    }

    fn from_evaluator<'a>(eval: &'a Evaluator<'_, '_, '_>) -> anyhow::Result<&'a Self> {
        eval.extra
            .and_then(|extra| extra.downcast_ref::<Self>())
            .ok_or_else(|| anyhow::anyhow!("Bazel package global invoked without package state"))
    }

    fn for_glob<'a>(eval: &'a Evaluator<'_, '_, '_>) -> anyhow::Result<&'a Self> {
        eval.extra
            .and_then(|extra| extra.downcast_ref::<Self>())
            .ok_or_else(|| {
                anyhow::anyhow!("glob() may only be called while evaluating a BUILD package")
            })
    }

    fn set_default_visibility(&self, visibility: Vec<String>) {
        self.state.borrow_mut().default_visibility = visibility;
    }

    fn exports_files(&self, srcs: Vec<String>) -> anyhow::Result<()> {
        for src in srcs {
            self.record_target(src, PackageTargetKind::ExportedFile)?;
        }
        Ok(())
    }

    fn filegroup(&self, name: String, srcs: Vec<String>) -> anyhow::Result<()> {
        self.record_target(name, PackageTargetKind::Filegroup { srcs })
    }

    fn alias(&self, name: String, actual: String) -> anyhow::Result<()> {
        self.record_target(name, PackageTargetKind::Alias { actual })
    }

    fn starlark_rule(
        &self,
        name: String,
        implementation: FrozenValue,
        dependencies: Vec<CanonicalLabel>,
    ) -> anyhow::Result<()> {
        self.record_target(
            name,
            PackageTargetKind::StarlarkRule(StarlarkRuleImplementation {
                implementation,
                dependencies: dependencies.into(),
            }),
        )
    }

    fn dependency_label(&self, value: &str) -> anyhow::Result<CanonicalLabel> {
        if value.starts_with('@') {
            anyhow::bail!(
                "external repository dependency labels are not supported in this analysis packet: {value}"
            );
        }
        let canonical = if let Some(target) = value.strip_prefix(':') {
            format!("@@//{}:{target}", self.package)
        } else if let Some(absolute) = value.strip_prefix("//") {
            format!("@@//{absolute}")
        } else {
            anyhow::bail!(
                "dependency label must be package-relative `:name` or root `//pkg:name`: {value}"
            );
        };
        CanonicalLabel::parse(&canonical).map_err(anyhow::Error::msg)
    }

    fn record_target(&self, name: String, kind: PackageTargetKind) -> anyhow::Result<()> {
        let mut state = self.state.borrow_mut();
        if state.targets.get(&name).is_some() {
            anyhow::bail!("target '{name}' declared more than once");
        }
        state.targets.insert(name, kind);
        Ok(())
    }

    fn glob(&self, spec: GlobSpec) -> anyhow::Result<Vec<String>> {
        let matches = expand_glob(&self.listing, &spec)?;
        self.state.borrow_mut().used_globs.push(spec);
        Ok(matches)
    }

    pub(crate) fn finish(
        self,
        package_dir: PathBuf,
        build_file: PathBuf,
        direct_load_roots: Arc<[BzlModuleIdentity]>,
        reachable_loads: Arc<[BzlModuleIdentity]>,
        load_fingerprint: [u8; 32],
        retained_bzl_modules: Arc<[FrozenBzlLifetimeEntry]>,
    ) -> LoadedPackage {
        let state = self.state.into_inner();
        LoadedPackage {
            package_dir,
            build_file,
            default_visibility: state.default_visibility,
            targets: state
                .targets
                .into_iter()
                .map(|(name, kind)| PackageTarget { name, kind })
                .collect(),
            used_globs: state.used_globs,
            direct_load_roots,
            reachable_loads,
            load_fingerprint,
            retained_bzl_modules,
        }
    }
}

fn list(items: UnpackListOrTuple<&str>) -> Vec<String> {
    items.items.into_iter().map(str::to_owned).collect()
}

fn package_global(
    default_visibility: Option<UnpackListOrTuple<&str>>,
    eval: &mut Evaluator,
) -> anyhow::Result<NoneType> {
    if let Some(default_visibility) = default_visibility {
        PackageRecorder::from_evaluator(eval)?.set_default_visibility(list(default_visibility));
    }
    Ok(NoneType)
}

fn exports_files_global(
    srcs: UnpackListOrTuple<&str>,
    eval: &mut Evaluator,
) -> anyhow::Result<NoneType> {
    PackageRecorder::from_evaluator(eval)?.exports_files(list(srcs))?;
    Ok(NoneType)
}

fn filegroup_global(
    name: &str,
    srcs: Option<UnpackListOrTuple<&str>>,
    eval: &mut Evaluator,
) -> anyhow::Result<NoneType> {
    PackageRecorder::from_evaluator(eval)?
        .filegroup(name.to_owned(), srcs.map(list).unwrap_or_default())?;
    Ok(NoneType)
}

fn alias_global(name: &str, actual: &str, eval: &mut Evaluator) -> anyhow::Result<NoneType> {
    PackageRecorder::from_evaluator(eval)?.alias(name.to_owned(), actual.to_owned())?;
    Ok(NoneType)
}

fn glob_global<'v>(
    include: UnpackListOrTuple<&str>,
    exclude: UnpackListOrTuple<&str>,
    exclude_directories: i32,
    allow_empty: Option<Value<'v>>,
    eval: &mut Evaluator<'v, '_, '_>,
) -> anyhow::Result<Vec<String>> {
    let allow_empty = match allow_empty {
        None => false,
        Some(value) => value.unpack_bool().ok_or_else(|| {
            anyhow::anyhow!(
                "expected boolean for argument `allow_empty`, got `{}`",
                value
            )
        })?,
    };
    let spec = GlobSpec::new(
        include.items,
        exclude.items,
        exclude_directories != 0,
        allow_empty,
    )?;
    PackageRecorder::for_glob(eval)?.glob(spec)
}

/// The callable returned by Bazel's `rule()` global during package loading.
/// It retains the implementation for Stage 6, but package construction never
/// executes that implementation.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Trace)]
struct RuleDefinitionGen<V> {
    implementation: V,
    has_deps: bool,
}

type RuleDefinition<'v> = RuleDefinitionGen<Value<'v>>;
type FrozenRuleDefinition = RuleDefinitionGen<FrozenValue>;

impl<V> fmt::Display for RuleDefinitionGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("rule")
    }
}

starlark::starlark_complex_values!(RuleDefinition);

impl<'v> Freeze for RuleDefinition<'v> {
    type Frozen = FrozenRuleDefinition;

    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        Ok(FrozenRuleDefinition {
            implementation: self.implementation.freeze(freezer)?,
            has_deps: self.has_deps,
        })
    }
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct LabelListAttr;

starlark::starlark_simple_value!(LabelListAttr);

impl fmt::Display for LabelListAttr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("attr.label_list()")
    }
}

#[starlark_value(type = "label_list_attr")]
impl<'v> StarlarkValue<'v> for LabelListAttr {}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct AttrModule;

starlark::starlark_simple_value!(AttrModule);

impl fmt::Display for AttrModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("attr")
    }
}

#[starlark_module]
fn attr_methods(builder: &mut MethodsBuilder) {
    fn label_list(#[starlark(this)] _attr: Value) -> anyhow::Result<LabelListAttr> {
        Ok(LabelListAttr)
    }
}

#[starlark_value(type = "attr")]
impl<'v> StarlarkValue<'v> for AttrModule {
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(attr_methods)
    }
}

#[starlark_value(type = "rule")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for RuleDefinitionGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        args.no_positional_args(eval.heap())?;
        let names = args.names_map()?;
        let name = names.get("name").ok_or_else(|| {
            starlark::Error::new_other(anyhow::anyhow!(
                "a target declared by rule() requires a string `name`"
            ))
        })?;
        let name = name.unpack_str().ok_or_else(|| {
            starlark::Error::new_other(anyhow::anyhow!(
                "a target declared by rule() requires a string `name`"
            ))
        })?;
        for attribute in names.keys() {
            if !matches!(attribute.as_str(), "name" | "deps" | "visibility") {
                return Err(starlark::Error::new_other(anyhow::anyhow!(
                    "target `{name}` received unknown attribute `{}`",
                    attribute.as_str()
                )));
            }
        }
        if names.contains_key("deps") && !self.has_deps {
            return Err(starlark::Error::new_other(anyhow::anyhow!(
                "target `{name}` received unknown attribute `deps`"
            )));
        }
        if let Some(visibility) = names.get("visibility") {
            let visibility = ListRef::from_value(*visibility).ok_or_else(|| {
                starlark::Error::new_other(anyhow::anyhow!(
                    "attribute `visibility` must be a list of strings"
                ))
            })?;
            if visibility.iter().any(|value| value.unpack_str().is_none()) {
                return Err(starlark::Error::new_other(anyhow::anyhow!(
                    "attribute `visibility` must be a list of strings"
                )));
            }
        }
        let dependency_values = match names.get("deps") {
            Some(value) => ListRef::from_value(*value).ok_or_else(|| {
                starlark::Error::new_other(anyhow::anyhow!(
                    "attribute `deps` must be a list of labels"
                ))
            })?,
            None => ListRef::from_value(eval.heap().alloc(Vec::<Value>::new()))
                .expect("allocated empty list"),
        };
        let implementation = self
            .implementation
            .to_value()
            .unpack_frozen()
            .ok_or_else(|| {
                starlark::Error::new_other(anyhow::anyhow!(
                    "rule() definitions may only be called after their .bzl module is frozen"
                ))
            })?;
        PackageRecorder::from_evaluator(eval)
            .and_then(|recorder| {
                let dependencies = dependency_values
                    .iter()
                    .map(|value| {
                        let value = value.unpack_str().ok_or_else(|| {
                            anyhow::anyhow!("attribute `deps` must contain only string labels")
                        })?;
                        recorder.dependency_label(value)
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?;
                recorder.starlark_rule(name.to_owned(), implementation, dependencies)
            })
            .map_err(starlark::Error::new_other)?;
        Ok(Value::new_none())
    }
}

#[starlark_module]
pub(crate) fn package_globals(builder: &mut GlobalsBuilder) {
    fn package(
        default_visibility: Option<UnpackListOrTuple<&str>>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        package_global(default_visibility, eval)
    }

    fn exports_files(
        srcs: UnpackListOrTuple<&str>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        exports_files_global(srcs, eval)
    }

    fn filegroup(
        name: &str,
        srcs: Option<UnpackListOrTuple<&str>>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        filegroup_global(name, srcs, eval)
    }

    fn alias(name: &str, actual: &str, eval: &mut Evaluator) -> anyhow::Result<NoneType> {
        alias_global(name, actual, eval)
    }

    fn glob<'v>(
        #[starlark(default=UnpackListOrTuple::default())] include: UnpackListOrTuple<&str>,
        #[starlark(default=UnpackListOrTuple::default())] exclude: UnpackListOrTuple<&str>,
        #[starlark(default = 1)] exclude_directories: i32,
        allow_empty: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Vec<String>> {
        glob_global(include, exclude, exclude_directories, allow_empty, eval)
    }

    fn rule<'v>(
        implementation: Value<'v>,
        attrs: Option<SmallMap<String, Value<'v>>>,
    ) -> anyhow::Result<RuleDefinition<'v>> {
        let mut has_deps = false;
        if let Some(attrs) = attrs {
            for (name, value) in attrs {
                if name != "deps" {
                    anyhow::bail!("rule() received unsupported attribute schema `{name}`");
                }
                if LabelListAttr::from_value(value).is_none() {
                    anyhow::bail!("rule attribute `deps` must use attr.label_list()");
                }
                has_deps = true;
            }
        }
        Ok(RuleDefinition {
            implementation,
            has_deps,
        })
    }

    fn provider(
        #[starlark(require = named)] fields: SmallMap<String, String>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<UserProviderCallable> {
        UserProviderCallable::from_evaluator(fields, eval)
    }
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct NativeModule;

impl fmt::Display for NativeModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("native")
    }
}

#[starlark_module]
fn native_methods(builder: &mut MethodsBuilder) {
    fn exports_files(
        #[starlark(this)] _native: Value,
        srcs: UnpackListOrTuple<&str>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        exports_files_global(srcs, eval)
    }

    fn filegroup(
        #[starlark(this)] _native: Value,
        name: &str,
        srcs: Option<UnpackListOrTuple<&str>>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        filegroup_global(name, srcs, eval)
    }

    fn alias(
        #[starlark(this)] _native: Value,
        name: &str,
        actual: &str,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        alias_global(name, actual, eval)
    }

    fn glob<'v>(
        #[starlark(this)] _native: Value<'v>,
        #[starlark(default=UnpackListOrTuple::default())] include: UnpackListOrTuple<&str>,
        #[starlark(default=UnpackListOrTuple::default())] exclude: UnpackListOrTuple<&str>,
        #[starlark(default = 1)] exclude_directories: i32,
        allow_empty: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Vec<String>> {
        glob_global(include, exclude, exclude_directories, allow_empty, eval)
    }
}

#[starlark_value(type = "native")]
impl<'v> StarlarkValue<'v> for NativeModule {
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(native_methods)
    }
}

impl AllocFrozenValue for NativeModule {
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        heap.alloc_simple(self)
    }
}

pub(crate) fn loading_globals() -> Globals {
    let mut globals = GlobalsBuilder::standard().with(package_globals);
    globals.set("native", NativeModule);
    globals.set("attr", AttrModule);
    globals.set("DefaultInfo", AnalysisBuiltinCallable::new("DefaultInfo"));
    globals.set("depset", AnalysisBuiltinCallable::new("depset"));
    globals.build()
}
