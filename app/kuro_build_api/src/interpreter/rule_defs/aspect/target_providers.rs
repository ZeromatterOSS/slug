/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! AspectTargetProviders - wrapper for the `target` argument in aspect implementations.
//!
//! Supports `target[SomeInfo]` to get a provider and `SomeInfo in target` to check existence.

use std::convert::Infallible;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;

use allocative::Allocative;
use kuro_core::target::configured_target_label::ConfiguredTargetLabel;
use kuro_interpreter::types::provider::callable::ValueAsProviderCallableLike;
use dupe::Dupe;
use starlark::any::ProvidesStaticType;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::typing::Ty;
use starlark::values::AllocValue;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Trace;
use starlark::values::UnpackValue;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark::values::none::NoneOr;
use starlark::values::starlark_value;
use starlark::values::type_repr::StarlarkTypeRepr;

use crate::interpreter::rule_defs::provider::collection::FrozenProviderCollection;
use crate::interpreter::rule_defs::provider::collection::FrozenProviderCollectionValueRef;

/// Error types for target provider access.
#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Input)]
enum AspectTargetError {
    #[error(
        "target[<provider>] operation requires a provider type, got `{0}`"
    )]
    IndexTypeNotProvider(&'static str),
    #[error(
        "target does not have provider `{provider}`. Available providers: {available:?}"
    )]
    ProviderNotFound {
        provider: String,
        available: Vec<String>,
    },
}

/// The `target` argument passed to aspect implementation functions.
///
/// This wraps the target's provider collection and supports:
/// - `target[SomeInfo]` - Get a provider value (raises error if not present)
/// - `SomeInfo in target` - Check if a provider is present
/// - `target.get(SomeInfo)` - Get a provider value or None
///
/// Example usage in Starlark:
/// ```python
/// def _my_aspect_impl(target, ctx):
///     if DefaultInfo in target:
///         info = target[DefaultInfo]
///         # Process the DefaultInfo provider
///
///     # Or use .get() for optional access
///     maybe_info = target.get(CustomInfo)
///     if maybe_info:
///         # Process if present
///
///     return [MyAspectInfo(...)]
/// ```
#[derive(ProvidesStaticType, NoSerialize, Allocative)]
pub struct AspectTargetProviders<'v> {
    /// The underlying frozen provider collection.
    #[allocative(skip)] // FrozenProviderCollectionValueRef doesn't impl Allocative
    providers: FrozenProviderCollectionValueRef<'v>,
    /// Target label (for error messages).
    label: ConfiguredTargetLabel,
}

// Manual Debug implementation since FrozenProviderCollectionValueRef doesn't impl Debug
impl<'v> fmt::Debug for AspectTargetProviders<'v> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("AspectTargetProviders")
            .field("label", &self.label)
            .finish_non_exhaustive()
    }
}

// Manual Trace implementation since FrozenProviderCollectionValueRef is frozen
// and doesn't need tracing
unsafe impl<'v> Trace<'v> for AspectTargetProviders<'v> {
    fn trace(&mut self, _tracer: &starlark::values::Tracer<'v>) {
        // FrozenProviderCollectionValueRef is already frozen, no tracing needed
    }
}

impl<'v> Display for AspectTargetProviders<'v> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "<target {}>", self.label)
    }
}

impl<'v> AspectTargetProviders<'v> {
    /// Create a new AspectTargetProviders wrapper.
    pub fn new(
        providers: FrozenProviderCollectionValueRef<'v>,
        label: ConfiguredTargetLabel,
    ) -> Self {
        AspectTargetProviders { providers, label }
    }

    /// Get the underlying provider collection.
    pub fn provider_collection(&self) -> &FrozenProviderCollection {
        self.providers.value().as_ref()
    }

    /// Get the target label.
    pub fn label(&self) -> &ConfiguredTargetLabel {
        &self.label
    }
}

impl<'v> AllocValue<'v> for AspectTargetProviders<'v> {
    fn alloc_value(self, heap: Heap<'v>) -> Value<'v> {
        heap.alloc_complex_no_freeze(self)
    }
}

/// Wrapper type for unpacking AspectTargetProviders from a Value.
struct RefAspectTargetProviders<'v>(&'v AspectTargetProviders<'v>);

impl<'v> StarlarkTypeRepr for RefAspectTargetProviders<'v> {
    type Canonical = <AspectTargetProviders<'v> as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        AspectTargetProviders::starlark_type_repr()
    }
}

impl<'v> UnpackValue<'v> for RefAspectTargetProviders<'v> {
    type Error = Infallible;

    fn unpack_value_impl(value: Value<'v>) -> Result<Option<Self>, Self::Error> {
        let Some(target) = value.downcast_ref::<AspectTargetProviders>() else {
            return Ok(None);
        };
        Ok(Some(RefAspectTargetProviders(target)))
    }
}

/// Methods for AspectTargetProviders.
#[starlark_module]
fn aspect_target_methods(builder: &mut MethodsBuilder) {
    /// Get a provider from the target, returning None if not present.
    fn get<'v>(
        this: RefAspectTargetProviders<'v>,
        #[starlark(require = pos)] provider: Value<'v>,
    ) -> starlark::Result<NoneOr<Value<'v>>> {
        match provider.as_provider_callable() {
            Some(callable) => {
                let provider_id = callable.id()?.dupe();
                match this.0.provider_collection().get_provider_raw(&provider_id) {
                    Some(v) => Ok(NoneOr::Other(v.to_value())),
                    None => Ok(NoneOr::None),
                }
            }
            None => Err(kuro_error::Error::from(AspectTargetError::IndexTypeNotProvider(
                provider.get_type(),
            ))
            .into()),
        }
    }
}

#[starlark_value(type = "Target")]
impl<'v> StarlarkValue<'v> for AspectTargetProviders<'v> {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(aspect_target_methods)
    }

    /// Implements `target[SomeInfo]` - get a provider value.
    fn at(&self, index: Value<'v>, _heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        match index.as_provider_callable() {
            Some(callable) => {
                let provider_id = callable.id()?.dupe();
                match self.provider_collection().get_provider_raw(&provider_id) {
                    Some(v) => Ok(v.to_value()),
                    None => Err(kuro_error::Error::from(AspectTargetError::ProviderNotFound {
                        provider: provider_id.name.clone(),
                        available: self.provider_collection().provider_names(),
                    })
                    .into()),
                }
            }
            None => Err(kuro_error::Error::from(AspectTargetError::IndexTypeNotProvider(
                index.get_type(),
            ))
            .into()),
        }
    }

    /// Implements `SomeInfo in target` - check if provider is present.
    fn is_in(&self, other: Value<'v>) -> starlark::Result<bool> {
        match other.as_provider_callable() {
            Some(callable) => {
                let provider_id = callable.id()?.dupe();
                Ok(self.provider_collection().contains_provider(&provider_id))
            }
            None => Err(kuro_error::Error::from(AspectTargetError::IndexTypeNotProvider(
                other.get_type(),
            ))
            .into()),
        }
    }
}
