/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! The `ctx.rule` object for aspect implementations.
//!
//! This provides access to the underlying rule's information when an aspect
//! is applied to a target.

use std::convert::Infallible;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;

use allocative::Allocative;
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
use starlark::values::starlark_value;
use starlark::values::structs::StructRef;
use starlark::values::type_repr::StarlarkTypeRepr;

/// Information about the rule being visited by an aspect.
///
/// Accessed via `ctx.rule` in aspect implementations. Provides:
/// - `ctx.rule.kind` - The kind of rule (e.g., "cc_library", "py_binary")
/// - `ctx.rule.attr` - The rule's resolved attributes
/// - `ctx.rule.files` - Struct of file lists for label/label_list attrs
/// - `ctx.rule.file` - Struct of single files for single-file label attrs
/// - `ctx.rule.executable` - Struct of executables for executable label attrs
#[derive(Debug, ProvidesStaticType, Trace, NoSerialize, Allocative)]
pub struct AspectRuleInfo<'v> {
    /// Rule type name (e.g., "cc_library", "py_binary")
    kind: String,
    /// Rule's attributes as a Starlark struct
    attr: ValueOfUnchecked<'v, StructRef<'static>>,
}

impl<'v> Display for AspectRuleInfo<'v> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "<rule_info kind=\"{}\">", self.kind)
    }
}

impl<'v> AspectRuleInfo<'v> {
    /// Create a new AspectRuleInfo.
    pub fn new(kind: String, attr: ValueOfUnchecked<'v, StructRef<'static>>) -> Self {
        AspectRuleInfo { kind, attr }
    }
}

impl<'v> AllocValue<'v> for AspectRuleInfo<'v> {
    fn alloc_value(self, heap: Heap<'v>) -> Value<'v> {
        heap.alloc_complex_no_freeze(self)
    }
}

/// Wrapper type for unpacking AspectRuleInfo from a Value.
struct RefAspectRuleInfo<'v>(&'v AspectRuleInfo<'v>);

impl<'v> StarlarkTypeRepr for RefAspectRuleInfo<'v> {
    type Canonical = <AspectRuleInfo<'v> as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        AspectRuleInfo::starlark_type_repr()
    }
}

impl<'v> UnpackValue<'v> for RefAspectRuleInfo<'v> {
    type Error = Infallible;

    fn unpack_value_impl(value: Value<'v>) -> Result<Option<Self>, Self::Error> {
        let Some(rule_info) = value.downcast_ref::<AspectRuleInfo>() else {
            return Ok(None);
        };
        Ok(Some(RefAspectRuleInfo(rule_info)))
    }
}

/// Methods for AspectRuleInfo, accessed via `ctx.rule.<method>`.
#[starlark_module]
fn aspect_rule_info_methods(builder: &mut MethodsBuilder) {
    /// The kind of rule (e.g., "cc_library", "py_binary").
    #[starlark(attribute)]
    fn kind<'v>(this: RefAspectRuleInfo<'v>) -> starlark::Result<&'v str> {
        Ok(&this.0.kind)
    }

    /// The rule's attributes as a struct.
    ///
    /// For Phase 8b, this returns the plain attributes.
    /// In Phase 8c, deps will be resolved to aspect results.
    #[starlark(attribute)]
    fn attr<'v>(
        this: RefAspectRuleInfo<'v>,
    ) -> starlark::Result<ValueOfUnchecked<'v, StructRef<'static>>> {
        Ok(this.0.attr)
    }

    /// Files from the rule's label/label_list attributes, as lists of File objects.
    ///
    /// For each label or label_list attribute, `ctx.rule.files.<attr>` returns a list of
    /// File objects (extracted from DefaultInfo.default_outputs of each dependency).
    ///
    /// Example:
    /// ```python
    /// def _my_aspect_impl(target, ctx):
    ///     src_files = ctx.rule.files.srcs  # list of File objects
    /// ```
    #[starlark(attribute)]
    fn files<'v>(this: RefAspectRuleInfo<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        use crate::interpreter::rule_defs::context::CtxFiles;
        Ok(heap.alloc(CtxFiles::new(this.0.attr)))
    }

    /// A single file from a label attribute.
    ///
    /// For each `attr.label(allow_single_file=True)` attribute, `ctx.rule.file.<attr>`
    /// returns the single File object.
    ///
    /// Example:
    /// ```python
    /// def _my_aspect_impl(target, ctx):
    ///     template = ctx.rule.file.template  # single File object
    /// ```
    #[starlark(attribute)]
    fn file<'v>(this: RefAspectRuleInfo<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        use crate::interpreter::rule_defs::context::CtxFile;
        Ok(heap.alloc(CtxFile::new(this.0.attr)))
    }

    /// Executable files from executable label attributes.
    ///
    /// For each `attr.label(executable=True)` attribute, `ctx.rule.executable.<attr>`
    /// returns the executable File object.
    ///
    /// Example:
    /// ```python
    /// def _my_aspect_impl(target, ctx):
    ///     tool = ctx.rule.executable.tool  # executable File object
    /// ```
    #[starlark(attribute)]
    fn executable<'v>(this: RefAspectRuleInfo<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        use crate::interpreter::rule_defs::context::CtxExecutable;
        Ok(heap.alloc(CtxExecutable::new(this.0.attr)))
    }
}

#[starlark_value(type = "rule_info")]
impl<'v> StarlarkValue<'v> for AspectRuleInfo<'v> {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(aspect_rule_info_methods)
    }
}
