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

use allocative::Allocative;
use slug_identity_v2::PackageIdentifier;
use starlark::any::ProvidesStaticType;
use starlark::environment::FrozenModule;
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
use starlark::values::list_or_tuple::UnpackListOrTuple;
use starlark::values::none::NoneType;
use starlark::values::starlark_value;
use starlark_map::small_map::SmallMap;

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
    #[allow(dead_code)] // Ownership only; frozen rule values borrow these heaps.
    #[allocative(skip)]
    retained_bzl_modules: Vec<FrozenModule>,
}

impl PartialEq for LoadedPackage {
    fn eq(&self, other: &Self) -> bool {
        self.package_dir == other.package_dir
            && self.build_file == other.build_file
            && self.default_visibility == other.default_visibility
            && self.targets == other.targets
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
}

impl StarlarkRuleImplementation {
    pub fn frozen_value(&self) -> FrozenValue {
        self.implementation
    }
}

#[derive(Debug, Default)]
struct PackageState {
    default_visibility: Vec<String>,
    targets: SmallMap<String, PackageTargetKind>,
}

#[derive(Debug, Default, ProvidesStaticType)]
pub(crate) struct PackageRecorder(RefCell<PackageState>);

impl PackageRecorder {
    fn from_evaluator<'a>(eval: &'a Evaluator<'_, '_, '_>) -> anyhow::Result<&'a Self> {
        eval.extra
            .and_then(|extra| extra.downcast_ref::<Self>())
            .ok_or_else(|| anyhow::anyhow!("Bazel package global invoked without package state"))
    }

    fn set_default_visibility(&self, visibility: Vec<String>) {
        self.0.borrow_mut().default_visibility = visibility;
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

    fn starlark_rule(&self, name: String, implementation: FrozenValue) -> anyhow::Result<()> {
        self.record_target(
            name,
            PackageTargetKind::StarlarkRule(StarlarkRuleImplementation { implementation }),
        )
    }

    fn record_target(&self, name: String, kind: PackageTargetKind) -> anyhow::Result<()> {
        let mut state = self.0.borrow_mut();
        if state.targets.get(&name).is_some() {
            anyhow::bail!("target '{name}' declared more than once");
        }
        state.targets.insert(name, kind);
        Ok(())
    }

    pub(crate) fn finish(
        self,
        package_dir: PathBuf,
        build_file: PathBuf,
        retained_bzl_modules: Vec<FrozenModule>,
    ) -> LoadedPackage {
        let state = self.0.into_inner();
        LoadedPackage {
            package_dir,
            build_file,
            default_visibility: state.default_visibility,
            targets: state
                .targets
                .into_iter()
                .map(|(name, kind)| PackageTarget { name, kind })
                .collect(),
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

/// The callable returned by Bazel's `rule()` global during package loading.
/// It retains the implementation for Stage 6, but package construction never
/// executes that implementation.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Trace)]
struct RuleDefinitionGen<V> {
    implementation: V,
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
        })
    }
}

/// A name that Starlark resolves while compiling a rule implementation but
/// that has no loading-stage behavior. Providers and depsets become real
/// analysis values in Stage 6.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct AnalysisPlaceholder {
    name: &'static str,
}

impl fmt::Display for AnalysisPlaceholder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name)
    }
}

starlark::starlark_simple_value!(AnalysisPlaceholder);

#[starlark_value(type = "analysis_placeholder")]
impl<'v> StarlarkValue<'v> for AnalysisPlaceholder {
    fn invoke(
        &self,
        _me: Value<'v>,
        _args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        if eval
            .extra
            .and_then(|extra| extra.downcast_ref::<PackageRecorder>())
            .is_some()
        {
            return Err(starlark::Error::new_other(anyhow::anyhow!(
                "{} is not available during BUILD package loading; configured-target analysis is not implemented",
                self.name
            )));
        }
        // Rule implementations capture their `.bzl` globals while loading.
        // Stage 6 supplies their prepared context in a separate evaluator, so
        // this placeholder must remain callable until provider/depset values
        // are fully represented as Starlark values there.
        Ok(Value::new_none())
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
            .and_then(|recorder| recorder.starlark_rule(name.to_owned(), implementation))
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

    fn rule<'v>(implementation: Value<'v>) -> anyhow::Result<RuleDefinition<'v>> {
        Ok(RuleDefinition { implementation })
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
    globals.set(
        "DefaultInfo",
        AnalysisPlaceholder {
            name: "DefaultInfo",
        },
    );
    globals.set("depset", AnalysisPlaceholder { name: "depset" });
    globals.build()
}
