/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fmt::Debug;
use std::iter;
use std::ptr;

use allocative::Allocative;
use kuro_artifact::artifact::artifact_type::Artifact;
use kuro_artifact::artifact::artifact_type::OutputArtifact;
use kuro_build_api_derive::internal_provider;
use kuro_error::BuckErrorContext;
use dupe::Dupe;
use starlark::any::ProvidesStaticType;
use starlark::coerce::Coerce;
use starlark::collections::SmallMap;
use starlark::environment::GlobalsBuilder;
use starlark::environment::MethodsBuilder;
use starlark::eval::Evaluator;
use starlark::values::Freeze;
use starlark::values::FreezeError;
use starlark::values::FrozenHeap;
use starlark::values::FrozenRef;
use starlark::values::FrozenValue;
use starlark::values::FrozenValueOfUnchecked;
use starlark::values::FrozenValueTyped;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StringValue;
use starlark::values::Trace;
use starlark::values::UnpackAndDiscard;
use starlark::values::UnpackValue;
use starlark::values::Value;
use starlark::values::ValueLifetimeless;
use starlark::values::ValueLike;
use starlark::values::ValueOf;
use starlark::values::ValueOfUnchecked;
use starlark::values::ValueOfUncheckedGeneric;
use starlark::values::dict::AllocDict;
use starlark::values::dict::DictType;
use starlark::values::dict::FrozenDictRef;
use starlark::values::dict::UnpackDictEntries;
use starlark::values::list::AllocList;
use starlark::values::list::ListRef;
use starlark::values::list::ListType;
use starlark::values::list::UnpackList;
use starlark::values::none::NoneOr;

use crate as kuro_build_api;
use crate::artifact_groups::ArtifactGroup;
use crate::interpreter::rule_defs::artifact::starlark_artifact::StarlarkArtifact;
use crate::interpreter::rule_defs::artifact::starlark_artifact_like::ValueAsInputArtifactLike;
use crate::interpreter::rule_defs::artifact::starlark_artifact_like::ValueIsInputArtifactAnnotation;
use crate::interpreter::rule_defs::artifact_tagging::ArtifactTag;
use crate::interpreter::rule_defs::cmd_args::CommandLineArtifactVisitor;
use crate::interpreter::rule_defs::cmd_args::value_as::ValueAsCommandLineLike;
use crate::interpreter::rule_defs::depset::Depset;
use crate::interpreter::rule_defs::provider::ProviderCollection;
use crate::interpreter::rule_defs::provider::collection::FrozenProviderCollection;

/// A provider that all rules' implementations must return
///
/// In many simple cases, this can be inferred for the user.
///
/// Example of a rule's implementation function and how these fields are used by the framework:
///
/// ```python
/// # //foo_binary.bzl
/// def impl(ctx):
///     out = ctx.actions.declare_output("out")
///     ctx.actions.run([ctx.attrs._cc[RunInfo], "-o", out.as_output()] + ctx.attrs.srcs)
///     stripped_out = ctx.actions.declare_output("stripped")
///     debug_symbols_out = ctx.actions.declare_output("debug_info")
///     ctx.actions.run([
///         ctx.attrs._strip[RunInfo],
///         "--binary",
///         out,
///         "--stripped-out",
///         stripped_out.as_output(),
///         "--debug-symbols-out",
///         debug_symbols_out.as_output(),
///     ])
///     return [
///         DefaultInfo(
///             sub_targets = {
///                 "stripped": [
///                     DefaultInfo(default_outputs = [stripped_out, debug_symbols_out]),
///                 ],
///             },
///             default_output = out,
///         ),
///     ]
///
/// foo_binary = rule(
///     impl = impl,
///     attrs = {
///         "srcs": attrs.list(attrs.source()),
///         "_cc": attrs.dep(default = "//tools:cc", providers = [RunInfo]),
///         "_strip_script": attrs.dep(default = "//tools:strip", providers = [RunInfo]),
///     },
/// )
///
///
/// def foo_binary_wrapper(name, srcs):
///     foo_binary(
///         name = name,
///         srcs = src,
///         out = name,
///         stripped = name + ".stripped",
///         debug_info = name + ".debug_info",
///     )
///
/// # //subdir/BUCK
/// load("//:foo_binary.bzl", "foo_binary_wrapper")
///
/// genrule(name = "gen_stuff", ...., default_outs = ["foo.cpp"])
///
/// # ":gen_stuff" pulls the default_outputs for //subdir:gen_stuff
/// foo_binary_wrapper(name = "foo", srcs = glob(["*.cpp"]) + [":gen_stuff"])
///
/// # Builds just 'foo' binary. The strip command is never invoked.
/// $ buck build //subdir:foo
///
/// # builds the 'foo' binary, because it is needed by the 'strip' command. Ensures that
/// # both the stripped binary and the debug symbols are built.
/// $ buck build //subdir:foo[stripped]
/// ```
#[internal_provider(default_info_creator, methods = default_info_methods)]
#[derive(Clone, Debug, Freeze, Trace, Coerce, ProvidesStaticType, Allocative)]
#[freeze(validator = validate_default_info, bounds = "V: ValueLike<'freeze>")]
#[repr(C)]
pub struct DefaultInfoGen<V: ValueLifetimeless> {
    /// A mapping of names to `ProviderCollection`s. The keys are used when resolving the
    /// `ProviderName` portion of a `ProvidersLabel` in order to access the providers for a
    /// subtarget, such as when doing `kuro build cell//foo:bar[baz]`. Just like any
    /// `ProviderCollection`, this collection must include at least a `DefaultInfo` provider. The
    /// subtargets can have their own subtargets as well, which can be accessed by chaining them,
    /// e.g.: `kuro build cell//foo:bar[baz][qux]`.
    sub_targets: ValueOfUncheckedGeneric<V, DictType<String, FrozenProviderCollection>>,
    /// A list of `Artifact`s that are built by default if this rule is requested
    /// explicitly (via CLI or `$(location)` etc), or depended on as as a "source"
    /// (i.e., `attrs.source()`).
    default_outputs: ValueOfUncheckedGeneric<V, ListType<ValueIsInputArtifactAnnotation>>,
    /// A list of `ArtifactTraversable`. The underlying `Artifact`s they define will
    /// be built by default if this rule is requested (via CLI or `$(location)` etc),
    /// but _not_ when it's depended on as as a "source" (i.e., `attrs.source()`).
    /// `ArtifactTraversable` can be an `Artifact` (which yields itself), or
    /// `cmd_args`, which expand to all their inputs.
    other_outputs: ValueOfUncheckedGeneric<V, ListType<ValueAsCommandLineLike<'static>>>,
}

fn validate_default_info(info: &FrozenDefaultInfo) -> kuro_error::Result<()> {
    // Check length of default outputs
    let default_output_list = ListRef::from_value(info.default_outputs.get().to_value())
        .expect("should be a list from constructor");
    if default_output_list.len() > 1 {
        tracing::info!("DefaultInfo.default_output should only have a maximum of 1 item.");
        // TODO use soft_error when landed
        // TODO error rather than soft warning
        // return Err(kuro_error::kuro_error!(
        //     "DefaultInfo.default_output can only have a maximum of 1 item."
        // ));
    }

    // Check mutable data hasn't been modified.
    for output in info.default_outputs_impl()? {
        output?;
    }
    for sub_target in info.sub_targets_impl()? {
        sub_target?;
    }

    Ok(())
}

impl<'v> DefaultInfo<'v> {
    pub fn empty(heap: Heap<'v>) -> Self {
        let sub_targets = ValueOfUnchecked::<DictType<_, _>>::new(heap.alloc(AllocDict::EMPTY));
        let default_outputs = ValueOfUnchecked::<ListType<_>>::new(heap.alloc(AllocList::EMPTY));
        let other_outputs = ValueOfUnchecked::<ListType<_>>::new(heap.alloc(AllocList::EMPTY));
        DefaultInfo {
            sub_targets,
            default_outputs,
            other_outputs,
        }
    }

    /// Create a DefaultInfo containing a single artifact as the default output.
    ///
    /// This is used to provide Bazel-compatible semantics where source files (artifacts)
    /// implicitly have a DefaultInfo with the file in the `files` field. This enables
    /// patterns like `DefaultInfo in artifact` and `artifact[DefaultInfo]` to work in
    /// rule implementations.
    ///
    /// The `artifact_value` parameter should be the original Value that represents the
    /// artifact, so that frozen values stay frozen.
    pub fn from_artifact_value(heap: Heap<'v>, artifact_value: Value<'v>) -> Self {
        let sub_targets = ValueOfUnchecked::<DictType<_, _>>::new(heap.alloc(AllocDict::EMPTY));
        let default_outputs = ValueOfUnchecked::<ListType<_>>::new(
            heap.alloc(AllocList([artifact_value])),
        );
        let other_outputs = ValueOfUnchecked::<ListType<_>>::new(heap.alloc(AllocList::EMPTY));
        DefaultInfo {
            sub_targets,
            default_outputs,
            other_outputs,
        }
    }
}

impl FrozenDefaultInfo {
    pub(crate) fn testing_empty(heap: &FrozenHeap) -> FrozenValueTyped<'static, FrozenDefaultInfo> {
        let sub_targets = heap
            .alloc_typed_unchecked(AllocDict(
                iter::empty::<(String, FrozenProviderCollection)>(),
            ))
            .cast();
        let default_outputs =
            FrozenValueOfUnchecked::<ListType<_>>::new(heap.alloc(AllocList::EMPTY));
        let other_outputs =
            FrozenValueOfUnchecked::<ListType<_>>::new(heap.alloc(AllocList::EMPTY));
        FrozenValueTyped::new_err(heap.alloc(FrozenDefaultInfo {
            sub_targets,
            default_outputs,
            other_outputs,
        }))
        .unwrap()
    }

    fn get_sub_target_providers_impl(
        &self,
        name: &str,
    ) -> kuro_error::Result<Option<FrozenValueTyped<'static, FrozenProviderCollection>>> {
        FrozenDictRef::from_frozen_value(self.sub_targets.get())
            .buck_error_context("sub_targets should be a dict-like object")?
            .get_str(name)
            .map(|v| {
                FrozenValueTyped::new_err(v).buck_error_context(
                    "Values inside of a frozen provider should be frozen provider collection",
                )
            })
            .transpose()
    }

    pub fn get_sub_target_providers(
        &self,
        name: &str,
    ) -> Option<FrozenValueTyped<'static, FrozenProviderCollection>> {
        self.get_sub_target_providers_impl(name).unwrap()
    }

    fn default_outputs_impl(
        &self,
    ) -> kuro_error::Result<impl Iterator<Item = kuro_error::Result<StarlarkArtifact>> + '_> {
        let list = ListRef::from_frozen_value(self.default_outputs.get())
            .buck_error_context("Should be list of artifacts")?;

        Ok(list.iter().map(|v| {
            let frozen_value = v.unpack_frozen().buck_error_context("should be frozen")?;

            Ok(
                if let Some(starlark_artifact) = frozen_value.downcast_ref::<StarlarkArtifact>() {
                    starlark_artifact.dupe()
                } else {
                    // This code path is for StarlarkPromiseArtifact. We have to create a `StarlarkArtifact` object here.
                    let artifact_like =
                        ValueAsInputArtifactLike::unpack_value(frozen_value.to_value())?
                            .buck_error_context("Should be list of artifacts")?;
                    artifact_like.0.get_bound_starlark_artifact()?
                },
            )
        }))
    }

    pub fn default_outputs<'a>(&'a self) -> Vec<StarlarkArtifact> {
        self.default_outputs_impl()
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    pub fn default_outputs_raw(&self) -> FrozenValue {
        self.default_outputs.get()
    }

    fn sub_targets_impl(
        &self,
    ) -> kuro_error::Result<
        impl Iterator<Item = kuro_error::Result<(&str, FrozenRef<'static, FrozenProviderCollection>)>>
        + '_,
    > {
        let sub_targets = FrozenDictRef::from_frozen_value(self.sub_targets.get())
            .buck_error_context("sub_targets should be a dict-like object")?;

        Ok(sub_targets.iter().map(|(k, v)| {
            kuro_error::Ok((
                k.to_value()
                    .unpack_str()
                    .buck_error_context("sub_targets should have string keys")?,
                v.downcast_frozen_ref::<FrozenProviderCollection>()
                    .buck_error_context(
                        "Values inside of a frozen provider should be frozen provider collection",
                    )?,
            ))
        }))
    }

    pub fn sub_targets(&self) -> SmallMap<&str, FrozenRef<'static, FrozenProviderCollection>> {
        self.sub_targets_impl()
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    pub fn sub_targets_raw(&self) -> FrozenValue {
        self.sub_targets.get()
    }

    pub fn for_each_default_output_artifact_only(
        &self,
        processor: &mut dyn FnMut(Artifact),
    ) -> kuro_error::Result<()> {
        self.for_each_in_list(self.default_outputs.get(), |value| {
            processor(
                ValueAsInputArtifactLike::unpack_value_err(value)?
                    .0
                    .get_bound_artifact()?,
            );
            Ok(())
        })
    }

    pub fn for_each_default_output_other_artifacts_only(
        &self,
        processor: &mut dyn FnMut(ArtifactGroup),
    ) -> kuro_error::Result<()> {
        self.for_each_in_list(self.default_outputs.get(), |value| {
            let others = ValueAsInputArtifactLike::unpack_value_err(value)?
                .0
                .get_associated_artifacts();
            others
                .iter()
                .flat_map(|v| v.iter())
                .for_each(|other| processor(other.dupe()));
            Ok(())
        })
    }

    pub fn for_each_other_output(
        &self,
        processor: &mut dyn FnMut(ArtifactGroup),
    ) -> kuro_error::Result<()> {
        struct Visitor<'x>(&'x mut dyn FnMut(ArtifactGroup));

        impl<'v> CommandLineArtifactVisitor<'v> for Visitor<'_> {
            fn visit_input(&mut self, input: ArtifactGroup, _: Vec<&ArtifactTag>) {
                (self.0)(input);
            }

            fn visit_declared_output(
                &mut self,
                _artifact: OutputArtifact<'v>,
                _tags: Vec<&ArtifactTag>,
            ) {
            }

            fn visit_frozen_output(&mut self, _artifact: Artifact, _tags: Vec<&ArtifactTag>) {}
        }

        self.for_each_in_list(self.other_outputs.get(), |value| {
            let arg_like = ValueAsCommandLineLike::unpack_value_err(value)?.0;
            arg_like.visit_artifacts(&mut Visitor(processor))?;
            Ok(())
        })
    }

    pub fn for_each_output(
        &self,
        processor: &mut dyn FnMut(ArtifactGroup),
    ) -> kuro_error::Result<()> {
        self.for_each_default_output_artifact_only(&mut |a| processor(ArtifactGroup::Artifact(a)))?;
        self.for_each_default_output_other_artifacts_only(processor)?;
        self.for_each_other_output(processor)
    }

    fn for_each_in_list(
        &self,
        value: FrozenValue,
        mut processor: impl FnMut(Value) -> kuro_error::Result<()>,
    ) -> kuro_error::Result<()> {
        let outputs_list = ListRef::from_frozen_value(value)
            .unwrap_or_else(|| panic!("expected list, got `{value:?}` from info `{self:?}`"));

        for value in outputs_list.iter() {
            processor(value)?;
        }

        Ok(())
    }
}

impl PartialEq for FrozenDefaultInfo {
    // frozen default infos can be compared by ptr for a simple equality
    fn eq(&self, other: &Self) -> bool {
        ptr::eq(self, other)
    }
}

#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Input)]
enum DefaultOutputError {
    #[error("Cannot specify both `default_output` and `default_outputs`.")]
    ConflictingArguments,
}

/// Custom methods for DefaultInfo provider.
/// This includes the standard field accessors plus Bazel-compatible additions like `files`.
#[starlark_module]
fn default_info_methods(builder: &mut MethodsBuilder) {
    /// A mapping of names to `ProviderCollection`s for subtargets.
    #[starlark(attribute)]
    fn sub_targets<'v>(
        this: &DefaultInfo<'v>,
    ) -> starlark::Result<ValueOfUnchecked<'v, DictType<String, FrozenProviderCollection>>> {
        Ok(this.sub_targets.to_value())
    }

    /// A list of `Artifact`s that are built by default.
    #[starlark(attribute)]
    fn default_outputs<'v>(
        this: &DefaultInfo<'v>,
    ) -> starlark::Result<ValueOfUnchecked<'v, ListType<ValueIsInputArtifactAnnotation>>> {
        Ok(this.default_outputs.to_value())
    }

    /// Additional outputs built by default but not propagated as sources.
    #[starlark(attribute)]
    fn other_outputs<'v>(
        this: &DefaultInfo<'v>,
    ) -> starlark::Result<ValueOfUnchecked<'v, ListType<ValueAsCommandLineLike<'static>>>> {
        Ok(this.other_outputs.to_value())
    }

    /// A depset of files built by default (Bazel-compatible).
    ///
    /// This provides Bazel compatibility by wrapping `default_outputs` in a depset.
    /// Rules written for Bazel can access `DefaultInfo.files.to_list()` to get the
    /// list of default output files.
    #[starlark(attribute)]
    fn files<'v>(this: &DefaultInfo<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        // Get the default_outputs as a list
        let outputs_value = this.default_outputs.get().to_value();
        let outputs_list = ListRef::from_value(outputs_value).unwrap_or_else(|| {
            // Should not happen since default_outputs is typed as a list
            panic!("default_outputs should be a list, got: {:?}", outputs_value)
        });

        // Collect all elements as frozen values
        let frozen_elements: Vec<FrozenValue> = outputs_list
            .iter()
            .filter_map(|v| v.unpack_frozen())
            .collect();

        // Check if we got all elements as frozen
        if frozen_elements.len() == outputs_list.len() {
            // All frozen - create a proper Depset
            let depset = Depset::from_frozen_values(frozen_elements, "default".to_owned());
            Ok(heap.alloc(depset))
        } else {
            // Some values aren't frozen (synthetic DefaultInfo case).
            // Create a DepsetWithList that can hold the unfrozen values.
            let depset = DepsetWithListGen::<Value<'v>>::new(outputs_value);
            Ok(heap.alloc(depset))
        }
    }

    /// Default runfiles for this target (Bazel-compatible).
    ///
    /// Returns a runfiles object that contains the files needed at runtime.
    #[starlark(attribute)]
    fn default_runfiles<'v>(
        #[allow(unused_variables)] this: &DefaultInfo<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        // Return an empty runfiles stub for Bazel compatibility
        Ok(heap.alloc(RunfilesStub))
    }

    /// Data runfiles for this target (Bazel-compatible).
    ///
    /// Returns a runfiles object for when this target is used as a data dependency.
    #[starlark(attribute)]
    fn data_runfiles<'v>(
        #[allow(unused_variables)] this: &DefaultInfo<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        // Return an empty runfiles stub for Bazel compatibility
        Ok(heap.alloc(RunfilesStub))
    }
}

/// A depset wrapper that holds an unfrozen list internally.
/// This is used for synthetic DefaultInfo where values aren't frozen yet.
///
/// Uses the Gen pattern with Coerce/Freeze to properly handle frozen/unfrozen values.
#[derive(
    Debug,
    Clone,
    Coerce,
    Trace,
    Freeze,
    derive_more::Display,
    ProvidesStaticType,
    NoSerialize,
    Allocative,
)]
#[display("depset({list})")]
#[repr(C)]
pub struct DepsetWithListGen<V: ValueLifetimeless> {
    list: V,
}

starlark::starlark_complex_value!(pub DepsetWithList);

impl<'v, V: ValueLike<'v>> DepsetWithListGen<V> {
    pub fn new(list: V) -> Self {
        Self { list }
    }

    fn list_len(&self) -> usize {
        ListRef::from_value(self.list.to_value()).map_or(0, |l| l.len())
    }
}

#[starlark::values::starlark_value(type = "depset")]
impl<'v, V: ValueLike<'v>> starlark::values::StarlarkValue<'v> for DepsetWithListGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn get_methods() -> Option<&'static starlark::environment::Methods> {
        static RES: starlark::environment::MethodsStatic =
            starlark::environment::MethodsStatic::new();
        RES.methods(depset_with_list_methods)
    }

    fn to_bool(&self) -> bool {
        self.list_len() > 0
    }

    fn length(&self) -> starlark::Result<i32> {
        Ok(self.list_len() as i32)
    }
}

#[starlark_module]
fn depset_with_list_methods(builder: &mut MethodsBuilder) {
    fn to_list<'v>(
        #[starlark(this)] this: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        // Try unfrozen first, then frozen
        if let Some(depset) = DepsetWithList::from_value(this) {
            let elements: Vec<Value<'v>> = ListRef::from_value(depset.list)
                .map_or_else(Vec::new, |l| l.iter().collect());
            Ok(heap.alloc(AllocList(elements)))
        } else if let Some(depset) = this.downcast_ref::<FrozenDepsetWithList>() {
            let elements: Vec<Value<'v>> = ListRef::from_frozen_value(depset.list)
                .map_or_else(Vec::new, |l| l.iter().collect());
            Ok(heap.alloc(AllocList(elements)))
        } else {
            // Fallback - return empty list
            Ok(heap.alloc(AllocList(Vec::<Value>::new())))
        }
    }
}

#[starlark_module]
fn default_info_creator(builder: &mut GlobalsBuilder) {
    #[starlark(as_type = FrozenDefaultInfo)]
    fn DefaultInfo<'v>(
        // TODO(nga): parameters must be named only.
        #[starlark(default = NoneOr::None)] default_output: NoneOr<
            ValueOf<'v, ValueIsInputArtifactAnnotation>,
        >,
        #[starlark(default = NoneOr::None)] default_outputs: NoneOr<
            ValueOf<'v, UnpackList<UnpackAndDiscard<ValueIsInputArtifactAnnotation>>>,
        >,
        #[starlark(default = ValueOf { value: FrozenValue::new_empty_list().to_value(), typed: UnpackList::default()})]
        other_outputs: ValueOf<
            'v,
            UnpackList<UnpackAndDiscard<ValueAsCommandLineLike<'v>>>,
        >,
        #[starlark(default = UnpackDictEntries::default())] sub_targets: UnpackDictEntries<
            StringValue<'v>,
            Value<'v>,
        >,
        // Bazel-style parameters for compatibility with rules_cc
        #[starlark(require = named, default = NoneOr::None)] files: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] default_runfiles: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] data_runfiles: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] runfiles: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] executable: NoneOr<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<DefaultInfo<'v>> {
        let heap = eval.heap();

        // Handle Bazel-style 'files' parameter: use it as default_outputs if present
        // and no Buck-style default_outputs/default_output were provided
        let files_as_outputs = files.into_option().and_then(|f| {
            // Try to convert files depset to a list of outputs
            // For now, just ignore files if default_outputs is also provided
            None::<ValueOf<'v, UnpackList<UnpackAndDiscard<ValueIsInputArtifactAnnotation>>>>
        });

        // support both list and singular options for now until we migrate all the rules.
        let valid_default_outputs: ValueOfUnchecked<ListType<ValueIsInputArtifactAnnotation>> =
            match (default_outputs.into_option(), default_output.into_option()) {
                (Some(list), None) => list.as_unchecked().cast(),
                (None, Some(default_output)) => {
                    // handle where we didn't specify `default_outputs`, which means we should use the new
                    // `default_output`.
                    eval.heap()
                        .alloc_typed_unchecked(AllocList([default_output.as_unchecked()]))
                        .cast()
                }
                (None, None) => {
                    // Bazel compatibility: if 'files' was provided, use it as default_outputs
                    if let Some(files_val) = files.into_option() {
                        // Try to iterate the files depset and collect artifacts
                        // For now, just use an empty list as fallback
                        ValueOfUnchecked::<ListType<_>>::new(eval.heap().alloc(AllocList::EMPTY))
                    } else {
                        ValueOfUnchecked::<ListType<_>>::new(eval.heap().alloc(AllocList::EMPTY))
                    }
                }
                (Some(_), Some(_)) => {
                    return Err(
                        kuro_error::Error::from(DefaultOutputError::ConflictingArguments).into(),
                    );
                }
            };

        let valid_sub_targets = sub_targets
            .entries
            .into_iter()
            .map(|(k, v)| {
                let as_provider_collection = ProviderCollection::try_from_value_subtarget(v, heap)?;
                Ok((
                    k,
                    ValueOfUnchecked::<FrozenProviderCollection>::new(
                        heap.alloc(as_provider_collection),
                    ),
                ))
            })
            .collect::<kuro_error::Result<Vec<(StringValue<'v>, _)>>>()?;

        // Note: default_runfiles, data_runfiles, runfiles, and executable are accepted
        // for Bazel compatibility but not used in the Buck2 DefaultInfo model.
        // They would need separate handling or a different provider structure.
        let _ = (default_runfiles, data_runfiles, runfiles, executable);

        Ok(DefaultInfo {
            default_outputs: valid_default_outputs,
            other_outputs: other_outputs.as_unchecked().cast(),
            sub_targets: heap
                .alloc_typed_unchecked(AllocDict(valid_sub_targets))
                .cast(),
        })
    }
}

// ============================================================================
// RunfilesStub - Bazel compatibility for runfiles
// ============================================================================

/// A stub for Bazel's runfiles object.
///
/// In Bazel, runfiles represent files that are needed at runtime.
/// This is a minimal stub for compatibility with rules_cc.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct RunfilesStub;

impl std::fmt::Display for RunfilesStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<runfiles>")
    }
}

starlark::starlark_simple_value!(RunfilesStub);

#[starlark::values::starlark_value(type = "runfiles")]
impl<'v> starlark::values::StarlarkValue<'v> for RunfilesStub {
    fn get_methods() -> Option<&'static starlark::environment::Methods> {
        static RES: starlark::environment::MethodsStatic =
            starlark::environment::MethodsStatic::new();
        RES.methods(runfiles_stub_methods)
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "files" | "symlinks" | "root_symlinks" | "empty_filenames")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "files" => Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty())),
            "symlinks" => Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty())),
            "root_symlinks" => Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty())),
            "empty_filenames" => Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty())),
            _ => None,
        }
    }
}

#[starlark_module]
fn runfiles_stub_methods(builder: &mut MethodsBuilder) {
    /// Merge multiple runfiles objects.
    fn merge<'v>(
        #[starlark(this)] this: &RunfilesStub,
        #[starlark(require = pos, default = starlark::values::list::AllocList::EMPTY)]
        _runfiles: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        // Return a new empty runfiles stub
        Ok(heap.alloc(RunfilesStub))
    }

    /// Merge all runfiles from a list.
    fn merge_all<'v>(
        #[starlark(this)] this: &RunfilesStub,
        #[starlark(require = pos)] _runfiles_list: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        Ok(heap.alloc(RunfilesStub))
    }
}
