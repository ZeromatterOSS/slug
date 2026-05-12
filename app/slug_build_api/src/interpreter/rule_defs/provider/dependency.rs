/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fmt;
use std::fmt::Display;
use std::hash::Hash;
use std::mem;

use allocative::Allocative;
use dupe::Dupe;
use slug_core::execution_types::execution::ExecutionPlatformResolution;
use slug_core::provider::label::ConfiguredProvidersLabel;
use slug_core::provider::label::ProviderName;
use slug_error::BuckErrorContext;
use slug_interpreter::types::configured_providers_label::StarlarkConfiguredProvidersLabel;
use slug_interpreter::types::provider::callable::ValueAsProviderCallableLike;
use starlark::any::ProvidesStaticType;
use starlark::coerce::Coerce;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::typing::Ty;
use starlark::values::Freeze;
use starlark::values::FrozenValue;
use starlark::values::FrozenValueTyped;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Trace;
use starlark::values::Value;
use starlark::values::ValueLifetimeless;
use starlark::values::ValueLike;
use starlark::values::ValueOfUnchecked;
use starlark::values::ValueOfUncheckedGeneric;
use starlark::values::none::NoneOr;
use starlark::values::starlark_value;
use starlark::values::starlark_value_as_type::StarlarkValueAsType;
use starlark::values::structs::AllocStruct;
use starlark_map::StarlarkHasher;

use crate::interpreter::rule_defs::artifact::starlark_artifact::StarlarkArtifact;
use crate::interpreter::rule_defs::provider::builtin::default_info::DefaultInfo;
use crate::interpreter::rule_defs::provider::builtin::default_info::DefaultInfoCallable;
use crate::interpreter::rule_defs::provider::collection::FrozenProviderCollection;
use crate::interpreter::rule_defs::provider::execution_platform::StarlarkExecutionPlatformResolution;
use crate::interpreter::rule_defs::provider::ty::abstract_provider::AbstractProvider;

#[derive(Debug, slug_error::Error)]
#[slug(tag = Input)]
enum DependencyError {
    #[error("Unknown subtarget, could not find `{0}`")]
    UnknownSubtarget(String),
}

/// Wraps a dependency's `ProvidersLabel` and the result of analysis together for users' rule implementation functions
///
/// From Starlark, the label is accessible with `.label`, and providers from the underlying
/// `ProviderCollection` are available via `[]` (`get()`)
#[derive(
    Debug,
    Trace,
    Coerce,
    Freeze,
    ProvidesStaticType,
    NoSerialize,
    Allocative
)]
#[repr(C)]
pub struct DependencyGen<V: ValueLifetimeless> {
    label: ValueOfUncheckedGeneric<V, StarlarkConfiguredProvidersLabel>,
    provider_collection: FrozenValueTyped<'static, FrozenProviderCollection>,
    // This could be `Option<...>`, but that breaks `Coerce`.
    execution_platform: ValueOfUncheckedGeneric<V, NoneOr<StarlarkExecutionPlatformResolution>>,
}

starlark_complex_value!(pub Dependency);

/// Bazel source files appear in `ctx.attr` for `attr.label(..., allow_files=True)`
/// as `Target` values whose `DefaultInfo.files` contains the underlying file.
#[derive(Debug, Clone, Dupe, ProvidesStaticType, NoSerialize, Allocative)]
pub struct SourceFileTarget {
    label: ConfiguredProvidersLabel,
    artifact: StarlarkArtifact,
}

starlark::starlark_simple_value!(SourceFileTarget);

impl SourceFileTarget {
    pub fn new(label: ConfiguredProvidersLabel, artifact: StarlarkArtifact) -> Self {
        Self { label, artifact }
    }

    pub fn artifact_value<'v>(&self, heap: Heap<'v>) -> Value<'v> {
        heap.alloc(self.artifact.dupe())
    }

    fn default_info_value<'v>(&self, heap: Heap<'v>) -> Value<'v> {
        let artifact_value = self.artifact_value(heap);
        heap.alloc(DefaultInfo::from_artifact_value(heap, artifact_value))
    }

    pub fn label(&self) -> &ConfiguredProvidersLabel {
        &self.label
    }
}

impl Display for SourceFileTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<source file target {}>", self.label)
    }
}

impl<V: ValueLifetimeless> Display for DependencyGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<dependency ")?;
        Display::fmt(&self.label, f)?;
        write!(f, ">")
    }
}

impl<'v, V: ValueLike<'v>> DependencyGen<V> {
    pub fn label(&self) -> &'v StarlarkConfiguredProvidersLabel {
        StarlarkConfiguredProvidersLabel::from_value(self.label.get().to_value()).unwrap()
    }

    pub fn provider_collection(&self) -> FrozenValueTyped<'static, FrozenProviderCollection> {
        self.provider_collection
    }
}

impl<'v> Dependency<'v> {
    pub fn new(
        heap: Heap<'v>,
        label: ConfiguredProvidersLabel,
        provider_collection: FrozenValueTyped<'v, FrozenProviderCollection>,
        execution_platform: Option<&ExecutionPlatformResolution>,
    ) -> Self {
        // Bazel compatibility: for alias (Forward) targets, use the actual target's
        // label instead of the alias label. In Bazel, alias targets are transparent
        // in label_keyed_string_dict — Target.label returns the actual target.
        // Detect by checking if DefaultInfo outputs have a different owner.
        let label = {
            let mut effective = label.clone();
            if let Ok(di) = provider_collection.default_info() {
                let raw = di.default_outputs_raw();
                if let Some(list) = starlark::values::list::ListRef::from_frozen_value(raw) {
                    if let Some(first) = list.content().first() {
                        // Try to get the artifact's owner label
                        use crate::interpreter::rule_defs::artifact::starlark_artifact_like::StarlarkArtifactLike;
                        if let Some(artifact) = first
                            .to_value()
                            .downcast_ref::<crate::interpreter::rule_defs::artifact::starlark_artifact::StarlarkArtifact>()
                        {
                            if let Some(slug_core::deferred::base_deferred_key::BaseDeferredKey::TargetLabel(owner_label)) = artifact.artifact().owner() {
                                if owner_label != label.target() {
                                    effective = ConfiguredProvidersLabel::new(
                                        owner_label.clone(),
                                        label.name().clone(),
                                    );
                                }
                            }
                        }
                    }
                }
            }
            effective
        };
        let execution_platform: ValueOfUnchecked<NoneOr<StarlarkExecutionPlatformResolution>> =
            match execution_platform {
                Some(e) => ValueOfUnchecked::new(
                    heap.alloc(StarlarkExecutionPlatformResolution(e.clone())),
                ),
                None => ValueOfUnchecked::new(Value::new_none()),
            };
        Dependency {
            label: heap.alloc_typed_unchecked(StarlarkConfiguredProvidersLabel::new(label)),
            provider_collection: unsafe {
                mem::transmute::<
                    FrozenValueTyped<'_, FrozenProviderCollection>,
                    FrozenValueTyped<'_, FrozenProviderCollection>,
                >(provider_collection)
            },
            execution_platform,
        }
    }

    pub fn execution_platform(&self) -> slug_error::Result<Option<&ExecutionPlatformResolution>> {
        let execution_platform: ValueOfUnchecked<NoneOr<&StarlarkExecutionPlatformResolution>> =
            self.execution_platform.cast();
        match execution_platform.unpack()? {
            NoneOr::None => Ok(None),
            NoneOr::Other(e) => Ok(Some(&e.0)),
        }
    }
}

#[starlark_value(type = "Target")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for DependencyGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn get_type_starlark_repr() -> Ty {
        Ty::starlark_value::<DependencyGen<Value<'v>>>()
    }

    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(dependency_methods)
    }

    fn at(&self, index: Value<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        self.provider_collection
            .to_value()
            .at(index, heap)
            .with_buck_error_context(|| format!("Error accessing dependencies of `{}`", self.label))
            .map_err(Into::into)
    }

    fn is_in(&self, other: Value<'v>) -> starlark::Result<bool> {
        self.provider_collection.to_value().is_in(other)
    }

    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        let other = match other.downcast_ref::<Dependency<'v>>() {
            Some(other) => other.label(),
            None => match other.downcast_ref::<FrozenDependency>() {
                Some(other) => other.label(),
                None => return Ok(false),
            },
        };
        Ok(self.label().inner() == other.inner())
    }

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.label().inner().hash(hasher);
        Ok(())
    }
}

#[starlark_value(type = "Target")]
impl<'v> StarlarkValue<'v> for SourceFileTarget {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(source_file_target_methods)
    }

    fn at(&self, index: Value<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        match index.as_provider_callable() {
            Some(callable) => {
                let provider_id = callable.id()?;
                if provider_id == DefaultInfoCallable::provider_id() {
                    Ok(self.default_info_value(heap))
                } else {
                    Err(slug_error::Error::from(DependencyError::UnknownSubtarget(
                        provider_id.name.clone(),
                    ))
                    .into())
                }
            }
            None => Err(
                slug_error::Error::from(ProviderIndexError::IndexTypeNotProvider(index.get_type()))
                    .into(),
            ),
        }
    }

    fn is_in(&self, other: Value<'v>) -> starlark::Result<bool> {
        match other.as_provider_callable() {
            Some(callable) => {
                let provider_id = callable.id()?;
                Ok(provider_id == DefaultInfoCallable::provider_id())
            }
            None => Err(
                slug_error::Error::from(ProviderIndexError::IndexTypeNotProvider(other.get_type()))
                    .into(),
            ),
        }
    }

    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        if let Some(other) = other.downcast_ref::<SourceFileTarget>() {
            return Ok(self.label == other.label);
        }
        if let Some(other) = other.downcast_ref::<Dependency<'v>>() {
            return Ok(&self.label == other.label().inner());
        }
        if let Some(other) = other.downcast_ref::<FrozenDependency>() {
            return Ok(&self.label == other.label().inner());
        }
        Ok(false)
    }

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.label.hash(hasher);
        Ok(())
    }
}

#[derive(Debug, slug_error::Error)]
#[slug(tag = Input)]
enum ProviderIndexError {
    #[error("target[<provider>] operation requires a provider type, got `{0}`")]
    IndexTypeNotProvider(&'static str),
}

#[starlark_module]
fn source_file_target_methods(builder: &mut MethodsBuilder) {
    #[starlark(attribute)]
    fn label<'v>(this: &SourceFileTarget, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        Ok(heap.alloc(StarlarkConfiguredProvidersLabel::new(this.label.dupe())))
    }

    #[starlark(attribute)]
    fn files<'v>(this: &SourceFileTarget, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        Ok(this
            .default_info_value(heap)
            .get_attr("files", heap)?
            .unwrap_or_else(Value::new_none))
    }

    #[starlark(attribute)]
    fn files_to_run<'v>(this: &SourceFileTarget, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        Ok(heap.alloc(AllocStruct([
            ("executable", Value::new_none()),
            ("runfiles_manifest", Value::new_none()),
            ("repo_mapping_manifest", Value::new_none()),
            ("_source", this.artifact_value(heap)),
        ])))
    }
}

/// Represents a dependency in a build rule. When you declare a dependency attribute using
/// `attrs.dep()` in your rule definition, accessing that attribute gives you a Dependency object
/// that provides access to the dependency's providers and metadata.
///
/// Key operations:
/// - Index with `dep[ProviderType]` to access a provider (errors if absent)
/// - Use `dep.get(ProviderType)` to optionally access a provider (returns None if absent)
/// - Access the dependency's label with `dep.label`
/// - Get subtargets with `dep.sub_target("name")`
///
/// Example usage in a rule:
/// ```python
/// my_library = rule(
///     impl = my_library_impl,
///     attrs = {
///         "deps": attrs.list(attrs.dep()),
///     },
/// )
///
/// def my_library_impl(ctx):
///     # Iterate over dependencies
///     for dep in ctx.attrs.deps:
///         # Access providers
///         if dep.get(CxxLibraryInfo):
///             libs = dep[CxxLibraryInfo].libraries
///
///         # Access outputs
///         outputs = dep[DefaultInfo].default_outputs
///
///         # Get the label
///         dep_target = dep.label.raw_target()
/// ```
#[starlark_module]
fn dependency_methods(builder: &mut MethodsBuilder) {
    /// The label of this dependency.
    #[starlark(attribute)]
    fn label<'v>(
        this: &Dependency<'v>,
    ) -> starlark::Result<ValueOfUnchecked<'v, StarlarkConfiguredProvidersLabel>> {
        Ok(this.label)
    }

    /// Returns a list of all providers available from this dependency.
    // TODO(nga): should return provider collection.
    #[starlark(attribute)]
    fn providers<'v>(this: &Dependency) -> starlark::Result<Vec<FrozenValue>> {
        Ok(this
            .provider_collection
            .providers
            .values()
            .copied()
            .collect())
    }

    /// Returns a `Dependency` object of the subtarget of this target.
    ///
    /// In most cases, you can also use `dep[DefaultInfo].sub_targets["foo"]` to access subtarget
    /// providers directly. This method is useful when you need a real `Dependency` object, such
    /// as when passing to `ctx.actions.anon_target()`.
    ///
    /// Example:
    /// ```python
    /// def _impl(ctx):
    ///     for dep in ctx.attrs.deps:
    ///         # Get the dependency for a subtarget named "shared"
    ///         shared_dep = dep.sub_target("shared")
    ///         # Now shared_dep is a Dependency you can pass to other APIs
    ///         # that require a Dependency object
    ///         ctx.actions.anon_target(my_rule, {"dep": shared_dep})
    /// ```
    fn sub_target<'v>(
        this: &Dependency<'v>,
        #[starlark(require = pos)] subtarget: &str,
        heap: Heap<'v>,
    ) -> starlark::Result<Dependency<'v>> {
        let di = this.provider_collection.default_info()?;
        let providers = di.get_sub_target_providers(subtarget).ok_or_else(|| {
            slug_error::Error::from(DependencyError::UnknownSubtarget(subtarget.to_owned()))
        })?;
        let lbl = StarlarkConfiguredProvidersLabel::from_value(this.label.get())
            .unwrap()
            .inner();
        let lbl = ConfiguredProvidersLabel::new(
            lbl.target().clone(),
            lbl.name().push(ProviderName::new(subtarget.to_owned())?),
        );
        Ok(Dependency::new(heap, lbl, providers, None))
    }

    /// Gets a specific provider from this dependency by provider type. Returns None if the
    /// provider is not present. This is the same as using indexing syntax `dep[ProviderType]`,
    /// but returns None instead of raising an error when the provider is absent.
    ///
    /// Example:
    /// ```python
    /// FooInfo = provider(fields=["bar"])
    ///
    /// def _impl(ctx):
    ///     for dep in ctx.attrs.deps:
    ///         # Try to get FooInfo provider, returns None if absent
    ///         foo_info = dep.get(FooInfo)
    ///         if foo_info:
    ///             # Provider exists, use it
    ///             value = foo_info.bar
    ///         else:
    ///             # Provider not available from this dependency
    ///             pass
    ///
    ///         # Compare with indexing (raises error if absent):
    ///         # foo_info = dep[FooInfo]  # Errors if FooInfo not provided
    /// ```
    fn get<'v>(
        this: &Dependency<'v>,
        index: Value<'v>,
    ) -> starlark::Result<NoneOr<ValueOfUnchecked<'v, AbstractProvider>>> {
        Ok(this
            .provider_collection
            .get(index)
            .with_buck_error_context(|| {
                format!("Error accessing dependencies of `{}`", this.label)
            })?)
    }

    /// Returns the default outputs of this dependency as a depset (Bazel-compatible).
    ///
    /// `Target.files` in Bazel returns a depset of the target's default outputs.
    /// This is equivalent to `dep[DefaultInfo].files`.
    #[starlark(attribute)]
    fn files<'v>(this: &Dependency<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        // Search all providers for one with a "files" attribute (from DefaultInfo)
        Ok(this
            .provider_collection
            .providers
            .values()
            .copied()
            .find_map(|fv: FrozenValue| {
                fv.to_value()
                    .get_attr("files", heap)
                    .ok()
                    .flatten()
                    .filter(|v| !v.is_none())
            })
            .internal_error("Target has no 'files' depset (DefaultInfo missing)")?)
    }

    /// Returns a `FilesToRunProvider` for this dependency (Bazel-compatible).
    ///
    /// `dep.files_to_run` in Bazel returns a provider with `executable` (the target's
    /// binary) and `runfiles_manifest`. Used to pass tools to `ctx.actions.run()`.
    #[starlark(attribute)]
    fn files_to_run<'v>(this: &Dependency<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        // Find executable by checking all providers for an "executable" attribute,
        // then fall back to first default output.
        let executable = this
            .provider_collection
            .providers
            .values()
            .copied()
            .find_map(|fv: FrozenValue| {
                fv.to_value()
                    .get_attr("executable", heap)
                    .ok()
                    .flatten()
                    .filter(|v| !v.is_none())
            })
            .or_else(|| {
                // Fall back: iterate default_outputs if available
                this.provider_collection
                    .providers
                    .values()
                    .copied()
                    .find_map(|fv: FrozenValue| {
                        let outputs_val =
                            fv.to_value().get_attr("default_outputs", heap).ok()??;
                        if let Ok(iter) = outputs_val.iterate(heap) {
                            iter.into_iter().next()
                        } else {
                            None
                        }
                    })
            })
            .unwrap_or_else(Value::new_none);

        // Return as a Starlark struct so it can be frozen automatically.
        Ok(heap.alloc(starlark::values::structs::AllocStruct([
            ("executable", executable),
            ("runfiles_manifest", Value::new_none()),
        ])))
    }
}

#[starlark_module]
pub(crate) fn register_dependency(globals: &mut GlobalsBuilder) {
    const Dependency: StarlarkValueAsType<DependencyGen<FrozenValue>> = StarlarkValueAsType::new();
}
