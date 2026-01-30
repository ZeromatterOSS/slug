/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! AspectContext - the context object passed to aspect implementations.
//!
//! This is analogous to `AnalysisContext` for rules, but includes
//! aspect-specific features like `ctx.rule` for accessing the underlying
//! rule's information.

use std::cell::RefCell;
use std::convert::Infallible;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;

use allocative::Allocative;
use kuro_core::provider::label::ConfiguredProvidersLabel;
use kuro_core::provider::label::ProvidersName;
use kuro_core::target::configured_target_label::ConfiguredTargetLabel;
use kuro_error::BuckErrorContext;
use kuro_execute::digest_config::DigestConfig;
use kuro_interpreter::types::configured_providers_label::StarlarkConfiguredProvidersLabel;
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
use starlark::values::ValueOfUnchecked;
use starlark::values::ValueTyped;
use starlark::values::starlark_value;
use starlark::values::structs::StructRef;
use starlark::values::type_repr::StarlarkTypeRepr;

use crate::analysis::registry::AnalysisRegistry;
use crate::interpreter::rule_defs::aspect::rule_info::AspectRuleInfo;
use crate::interpreter::rule_defs::context::AnalysisActions;
use crate::interpreter::rule_defs::fragments::ConfigurationFragments;

/// The context object passed to aspect implementation functions.
///
/// Provides access to:
/// - `ctx.attr` - Aspect-specific attributes (from the aspect's `attrs` parameter)
/// - `ctx.actions` - Action registration (same as rule context)
/// - `ctx.label` - The target's label
/// - `ctx.rule` - Information about the underlying rule (`ctx.rule.kind`, `ctx.rule.attr`)
/// - `ctx.fragments` - Configuration fragments
///
/// Example usage in Starlark:
/// ```python
/// def _my_aspect_impl(target, ctx):
///     print("Visiting:", ctx.label)
///     print("Rule kind:", ctx.rule.kind)
///     # Access rule attributes
///     deps = ctx.rule.attr.deps
///     # Register actions
///     ctx.actions.write(output, content)
///     return [MyAspectInfo(...)]
/// ```
#[derive(ProvidesStaticType, Debug, Trace, NoSerialize, Allocative)]
pub struct AspectContext<'v> {
    /// Aspect-specific attributes (from aspect's attrs={} parameter).
    /// These are attributes defined on the aspect itself, not the rule.
    attrs: Option<ValueOfUnchecked<'v, StructRef<'static>>>,
    /// Actions registry (same as rule ctx).
    pub actions: ValueTyped<'v, AnalysisActions<'v>>,
    /// Target's label.
    label: ValueTyped<'v, StarlarkConfiguredProvidersLabel>,
    /// Rule information (ctx.rule access).
    rule: ValueTyped<'v, AspectRuleInfo<'v>>,
}

impl<'v> Display for AspectContext<'v> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "<aspect_ctx label=\"{}\">", self.label)
    }
}

impl<'v> AspectContext<'v> {
    /// Create a new AspectContext.
    fn new(
        heap: Heap<'v>,
        attrs: Option<ValueOfUnchecked<'v, StructRef<'static>>>,
        label: ValueTyped<'v, StarlarkConfiguredProvidersLabel>,
        rule: ValueTyped<'v, AspectRuleInfo<'v>>,
        registry: AnalysisRegistry<'v>,
        digest_config: DigestConfig,
    ) -> Self {
        Self {
            attrs,
            actions: heap.alloc_typed(AnalysisActions {
                state: RefCell::new(Some(registry)),
                attributes: attrs,
                plugins: None,
                digest_config,
            }),
            label,
            rule,
        }
    }

    /// Prepare an AspectContext for aspect execution.
    pub fn prepare(
        heap: Heap<'v>,
        attrs: Option<ValueOfUnchecked<'v, StructRef<'static>>>,
        label: ConfiguredTargetLabel,
        rule: ValueTyped<'v, AspectRuleInfo<'v>>,
        registry: AnalysisRegistry<'v>,
        digest_config: DigestConfig,
    ) -> ValueTyped<'v, AspectContext<'v>> {
        let label = heap.alloc_typed(StarlarkConfiguredProvidersLabel::new(
            ConfiguredProvidersLabel::new(label, ProvidersName::Default),
        ));

        let ctx = Self::new(heap, attrs, label, rule, registry, digest_config);
        heap.alloc_typed(ctx)
    }

    /// Take ownership of the analysis registry state.
    /// Must only be called once after aspect execution is complete.
    pub fn take_state(&self) -> AnalysisRegistry<'v> {
        self.actions
            .state
            .borrow_mut()
            .take()
            .expect("state to not have been taken yet")
    }
}

impl<'v> AllocValue<'v> for AspectContext<'v> {
    fn alloc_value(self, heap: Heap<'v>) -> Value<'v> {
        heap.alloc_complex_no_freeze(self)
    }
}

/// Wrapper type for unpacking AspectContext from a Value.
struct RefAspectContext<'v>(&'v AspectContext<'v>);

impl<'v> StarlarkTypeRepr for RefAspectContext<'v> {
    type Canonical = <AspectContext<'v> as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        AspectContext::starlark_type_repr()
    }
}

impl<'v> UnpackValue<'v> for RefAspectContext<'v> {
    type Error = Infallible;

    fn unpack_value_impl(value: Value<'v>) -> Result<Option<Self>, Self::Error> {
        let Some(ctx) = value.downcast_ref::<AspectContext>() else {
            return Ok(None);
        };
        Ok(Some(RefAspectContext(ctx)))
    }
}

/// Methods for AspectContext, accessed via `ctx.<method>`.
#[starlark_module]
fn aspect_context_methods(builder: &mut MethodsBuilder) {
    /// Returns the aspect's own attributes as a Starlark struct.
    ///
    /// These are the attributes defined in the aspect's `attrs` parameter,
    /// not the underlying rule's attributes.
    #[starlark(attribute)]
    fn attr<'v>(
        this: RefAspectContext<'v>,
    ) -> starlark::Result<ValueOfUnchecked<'v, StructRef<'static>>> {
        Ok(this
            .0
            .attrs
            .buck_error_context("`attr` is not available when aspect has no attrs defined")?)
    }

    /// Returns an `actions` value containing functions to define actual actions.
    /// See the `actions` type for available operations.
    #[starlark(attribute)]
    fn actions<'v>(
        this: RefAspectContext<'v>,
    ) -> starlark::Result<ValueTyped<'v, AnalysisActions<'v>>> {
        Ok(this.0.actions)
    }

    /// Returns a `label` representing the target this aspect is visiting.
    #[starlark(attribute)]
    fn label<'v>(
        this: RefAspectContext<'v>,
    ) -> starlark::Result<ValueTyped<'v, StarlarkConfiguredProvidersLabel>> {
        Ok(this.0.label)
    }

    /// Returns information about the underlying rule.
    ///
    /// Provides access to:
    /// - `ctx.rule.kind` - The kind of rule (e.g., "cc_library")
    /// - `ctx.rule.attr` - The rule's attributes
    #[starlark(attribute)]
    fn rule<'v>(
        this: RefAspectContext<'v>,
    ) -> starlark::Result<ValueTyped<'v, AspectRuleInfo<'v>>> {
        Ok(this.0.rule)
    }

    /// Configuration fragments for this target.
    ///
    /// Provides access to language-specific configuration like
    /// `ctx.fragments.cpp`, `ctx.fragments.java`, etc.
    #[starlark(attribute)]
    fn fragments<'v>(this: RefAspectContext<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let _ = this;
        // Return default configuration fragments for now
        // TODO(fragments): Pull actual configuration from target configuration
        Ok(heap.alloc(ConfigurationFragments::default()))
    }
}

#[starlark_value(type = "AspectContext")]
impl<'v> StarlarkValue<'v> for AspectContext<'v> {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(aspect_context_methods)
    }
}
