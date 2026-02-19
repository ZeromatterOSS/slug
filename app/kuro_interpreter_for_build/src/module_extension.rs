/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Implementation of Bazel's `module_extension()` and `tag_class()` built-in functions.
//!
//! Plan Reference: `thoughts/shared/plans/kuro-bazel-subplans/02-bzlmod.md` Phase 5
//!
//! ## Current Status: STUB IMPLEMENTATION
//!
//! This is a minimal stub that allows `module_extension()` and `tag_class()` to be
//! called in .bzl files that define extensions. The actual extension execution is
//! handled separately in `kuro_bzlmod`.
//!
//! ## What These Stubs Do
//!
//! - Allow extension .bzl files to parse without error
//! - Return placeholder values that can be loaded via `use_extension()`
//! - Store the implementation function and tag class definitions for later execution
//!
//! ## Example usage in Starlark:
//!
//! ```python
//! # In @rules_python//python/extensions:pip.bzl
//!
//! def _pip_impl(module_ctx):
//!     for mod in module_ctx.modules:
//!         for parse_tag in mod.tags.parse:
//!             # Handle pip.parse() tags from MODULE.bazel files
//!             pass
//!     # Create repositories...
//!
//! pip = module_extension(
//!     implementation = _pip_impl,
//!     tag_classes = {
//!         "parse": tag_class(
//!             attrs = {
//!                 "hub_name": attr.string(mandatory = True),
//!                 "python_version": attr.string(default = "3.11"),
//!                 "requirements_lock": attr.label(mandatory = True),
//!             },
//!         ),
//!     },
//! )
//! ```
//!
//! Then in MODULE.bazel:
//! ```python
//! pip = use_extension("@rules_python//python/extensions:pip.bzl", "pip")
//! pip.parse(hub_name = "pip", requirements_lock = "//:requirements.txt")
//! use_repo(pip, "pip")
//! ```

use std::cell::RefCell;
use std::fmt;

use allocative::Allocative;
use derive_more::Display;
use starlark::any::ProvidesStaticType;
use starlark::docs::DocFunction;
use starlark::docs::DocItem;
use starlark::docs::DocMember;
use starlark::docs::DocStringKind;
use starlark::environment::GlobalsBuilder;
use starlark::eval::Arguments;
use starlark::eval::Evaluator;
use starlark::eval::ParametersSpec;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::typing::Ty;
use starlark::values::AllocValue;
use starlark::values::Freeze;
use starlark::values::FreezeError;
use starlark::values::FreezeResult;
use starlark::values::Freezer;
use starlark::values::FrozenValue;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Trace;
use starlark::values::Value;
use starlark::values::dict::UnpackDictEntries;
use starlark::values::list_or_tuple::UnpackListOrTuple;
use starlark::values::starlark_value;
use starlark::values::starlark_value_as_type::StarlarkValueAsType;

use crate::attrs::starlark_attribute::StarlarkAttribute;
use crate::interpreter::build_context::BuildContext;
use crate::interpreter::build_context::PerFileTypeContext;

/// Errors around module extension declaration.
#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Input)]
enum ModuleExtensionError {
    #[error(
        "Module extension must be assigned to a variable before use, e.g. `my_ext = module_extension(...)`"
    )]
    ExtensionNotAssigned,
    #[error("`module_extension` can only be declared in .bzl files")]
    ExtensionNotInBzl,
    #[error("`tag_class` can only be declared in .bzl files")]
    TagClassNotInBzl,
    #[error("Module extension cannot be invoked directly")]
    ExtensionCannotBeInvokedDirectly,
}

// ============================================================================
// TagClass - Defines the schema for a tag that can be used in MODULE.bazel
// ============================================================================

/// A tag class defines the attributes that can be passed to a tag call.
/// For example, `pip.parse(hub_name = "pip")` uses the "parse" tag class
/// which defines that `hub_name` is a valid attribute.
#[derive(Debug, ProvidesStaticType, Trace, NoSerialize, Allocative)]
pub struct StarlarkTagClass<'v> {
    /// The attributes this tag class accepts.
    attrs: Vec<(String, StarlarkAttribute)>,
    /// Documentation string.
    doc: Option<String>,
    /// Phantom data for lifetime
    _phantom: std::marker::PhantomData<&'v ()>,
}

impl<'v> Display for StarlarkTagClass<'v> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<tag_class>")
    }
}

impl<'v> AllocValue<'v> for StarlarkTagClass<'v> {
    fn alloc_value(self, heap: Heap<'v>) -> Value<'v> {
        heap.alloc_complex(self)
    }
}

#[starlark_value(type = "tag_class")]
impl<'v> StarlarkValue<'v> for StarlarkTagClass<'v> {
    fn documentation(&self) -> DocItem {
        let params = ParametersSpec::<FrozenValue>::new_named_only(
            "tag_class",
            std::iter::empty::<(&str, _)>(),
        )
        .documentation(vec![], std::collections::HashMap::new());
        let function_docs = DocFunction::from_docstring(
            DocStringKind::Starlark,
            params,
            Ty::any(),
            self.doc.as_deref(),
        );
        DocItem::Member(DocMember::Function(function_docs))
    }

    fn get_type_starlark_repr() -> Ty {
        Ty::any()
    }
}

/// Frozen version of StarlarkTagClass.
#[derive(Debug, Display, ProvidesStaticType, NoSerialize, Allocative)]
#[display("<tag_class>")]
pub struct FrozenStarlarkTagClass {
    /// The attributes this tag class accepts.
    attrs: Vec<(String, StarlarkAttribute)>,
    /// Documentation string.
    doc: Option<String>,
}

starlark_simple_value!(FrozenStarlarkTagClass);

impl<'v> Freeze for StarlarkTagClass<'v> {
    type Frozen = FrozenStarlarkTagClass;

    fn freeze(self, _freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        Ok(FrozenStarlarkTagClass {
            attrs: self.attrs,
            doc: self.doc,
        })
    }
}

impl FrozenStarlarkTagClass {
    /// Get the attributes defined by this tag class.
    pub fn attrs(&self) -> &[(String, StarlarkAttribute)] {
        &self.attrs
    }
}

#[starlark_value(type = "tag_class")]
impl<'v> StarlarkValue<'v> for FrozenStarlarkTagClass {
    type Canonical = StarlarkTagClass<'v>;

    fn documentation(&self) -> DocItem {
        let params = ParametersSpec::<FrozenValue>::new_named_only(
            "tag_class",
            std::iter::empty::<(&str, _)>(),
        )
        .documentation(vec![], std::collections::HashMap::new());
        let function_docs = DocFunction::from_docstring(
            DocStringKind::Starlark,
            params,
            Ty::any(),
            self.doc.as_deref(),
        );
        DocItem::Member(DocMember::Function(function_docs))
    }

    fn get_type_starlark_repr() -> Ty {
        StarlarkTagClass::get_type_starlark_repr()
    }
}

// ============================================================================
// ModuleExtension - Defines a module extension
// ============================================================================

/// The value returned from a `module_extension()` call.
#[derive(Debug, ProvidesStaticType, Trace, NoSerialize, Allocative)]
pub struct StarlarkModuleExtension<'v> {
    /// The name of this extension (set when exported/assigned to a variable).
    name: RefCell<Option<String>>,
    /// The implementation function.
    /// Signature: def impl(module_ctx) -> None
    implementation: Value<'v>,
    /// Tag classes defined for this extension.
    /// Stored as Vec for Trace support; converted from dict at construction.
    tag_classes: Vec<(String, Value<'v>)>,
    /// Whether the extension depends on the OS.
    os_dependent: bool,
    /// Whether the extension depends on the architecture.
    arch_dependent: bool,
    /// Documentation string.
    doc: Option<String>,
    /// Environment variables this extension needs.
    environ: Vec<String>,
}

impl<'v> Display for StarlarkModuleExtension<'v> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &*self.name.borrow() {
            Some(name) => write!(f, "<module_extension {}>", name),
            None => write!(f, "<unbound module_extension>"),
        }
    }
}

impl<'v> AllocValue<'v> for StarlarkModuleExtension<'v> {
    fn alloc_value(self, heap: Heap<'v>) -> Value<'v> {
        heap.alloc_complex(self)
    }
}

impl<'v> StarlarkModuleExtension<'v> {
    fn new(
        implementation: Value<'v>,
        tag_classes: Vec<(String, Value<'v>)>,
        os_dependent: bool,
        arch_dependent: bool,
        doc: &str,
        environ: Vec<String>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> kuro_error::Result<StarlarkModuleExtension<'v>> {
        // When there's no build context (e.g. standalone/sync evaluator), allow the call.
        // When there is a context, verify we're in a .bzl file.
        if let Some(build_context) = eval.extra.and_then(|e| e.downcast_ref::<BuildContext>()) {
            match &build_context.additional {
                PerFileTypeContext::Bzl(_) => {}
                _ => return Err(ModuleExtensionError::ExtensionNotInBzl.into()),
            }
        }

        Ok(StarlarkModuleExtension {
            name: RefCell::new(None),
            implementation,
            tag_classes,
            os_dependent,
            arch_dependent,
            doc: if doc.is_empty() {
                None
            } else {
                Some(doc.to_owned())
            },
            environ,
        })
    }
}

#[starlark_value(type = "module_extension")]
impl<'v> StarlarkValue<'v> for StarlarkModuleExtension<'v> {
    fn export_as(
        &self,
        variable_name: &str,
        _eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<()> {
        *self.name.borrow_mut() = Some(variable_name.to_owned());
        Ok(())
    }

    fn invoke(
        &self,
        _me: Value<'v>,
        _args: &Arguments<'v, '_>,
        _eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Module extensions cannot be called directly - they are used via use_extension()
        Err(kuro_error::Error::from(ModuleExtensionError::ExtensionCannotBeInvokedDirectly).into())
    }

    fn documentation(&self) -> DocItem {
        let params = ParametersSpec::<FrozenValue>::new_named_only(
            "module_extension",
            std::iter::empty::<(&str, _)>(),
        )
        .documentation(vec![], std::collections::HashMap::new());
        let function_docs = DocFunction::from_docstring(
            DocStringKind::Starlark,
            params,
            Ty::any(),
            self.doc.as_deref(),
        );
        DocItem::Member(DocMember::Function(function_docs))
    }

    fn get_type_starlark_repr() -> Ty {
        Ty::any()
    }
}

/// Frozen (immutable) version of StarlarkModuleExtension.
#[derive(Debug, Display, ProvidesStaticType, NoSerialize, Allocative)]
#[display("<module_extension {}>", name)]
pub struct FrozenStarlarkModuleExtension {
    /// The name of this extension.
    name: String,
    /// The implementation function.
    implementation: FrozenValue,
    /// Tag classes defined for this extension.
    tag_classes: Vec<(String, FrozenValue)>,
    /// Whether the extension depends on the OS.
    os_dependent: bool,
    /// Whether the extension depends on the architecture.
    arch_dependent: bool,
    /// Documentation string.
    doc: Option<String>,
    /// Environment variables this extension needs.
    environ: Vec<String>,
}

starlark_simple_value!(FrozenStarlarkModuleExtension);

impl<'v> Freeze for StarlarkModuleExtension<'v> {
    type Frozen = FrozenStarlarkModuleExtension;

    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        let name = match self.name.into_inner() {
            Some(name) => name,
            None => {
                return Err(FreezeError::new(
                    ModuleExtensionError::ExtensionNotAssigned.to_string(),
                ));
            }
        };

        let tag_classes = self
            .tag_classes
            .into_iter()
            .map(|(k, v)| Ok((k, v.freeze(freezer)?)))
            .collect::<FreezeResult<Vec<_>>>()?;

        Ok(FrozenStarlarkModuleExtension {
            name,
            implementation: self.implementation.freeze(freezer)?,
            tag_classes,
            os_dependent: self.os_dependent,
            arch_dependent: self.arch_dependent,
            doc: self.doc,
            environ: self.environ,
        })
    }
}

impl FrozenStarlarkModuleExtension {
    /// Get the name of this extension.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the implementation function.
    pub fn implementation(&self) -> FrozenValue {
        self.implementation
    }

    /// Get the tag classes defined by this extension.
    pub fn tag_classes(&self) -> &[(String, FrozenValue)] {
        &self.tag_classes
    }

    /// Whether this extension depends on the OS.
    pub fn os_dependent(&self) -> bool {
        self.os_dependent
    }

    /// Whether this extension depends on the architecture.
    pub fn arch_dependent(&self) -> bool {
        self.arch_dependent
    }

    /// Get the environment variables this extension needs.
    pub fn environ(&self) -> &[String] {
        &self.environ
    }
}

#[starlark_value(type = "module_extension")]
impl<'v> StarlarkValue<'v> for FrozenStarlarkModuleExtension {
    type Canonical = StarlarkModuleExtension<'v>;

    fn invoke(
        &self,
        _me: Value<'v>,
        _args: &Arguments<'v, '_>,
        _eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        Err(kuro_error::Error::from(ModuleExtensionError::ExtensionCannotBeInvokedDirectly).into())
    }

    fn documentation(&self) -> DocItem {
        let params = ParametersSpec::<FrozenValue>::new_named_only(
            &self.name,
            std::iter::empty::<(&str, _)>(),
        )
        .documentation(vec![], std::collections::HashMap::new());
        let function_docs = DocFunction::from_docstring(
            DocStringKind::Starlark,
            params,
            Ty::any(),
            self.doc.as_deref(),
        );
        DocItem::Member(DocMember::Function(function_docs))
    }

    fn get_type_starlark_repr() -> Ty {
        StarlarkModuleExtension::get_type_starlark_repr()
    }
}

// ============================================================================
// Starlark global registration
// ============================================================================

/// Register the `module_extension()` and `tag_class()` functions as Starlark globals.
#[starlark_module]
pub fn register_module_extension_function(builder: &mut GlobalsBuilder) {
    /// Define a tag class for a module extension.
    ///
    /// A tag class specifies the attributes that can be used when calling a tag
    /// in a MODULE.bazel file. For example, if an extension has a "parse" tag class,
    /// users can call `ext.parse(attr1 = value1, ...)` in their MODULE.bazel.
    ///
    /// Example:
    /// ```python
    /// tag_class(
    ///     attrs = {
    ///         "hub_name": attr.string(mandatory = True),
    ///         "python_version": attr.string(default = "3.11"),
    ///     },
    ///     doc = "Configure pip package parsing",
    /// )
    /// ```
    fn tag_class<'v>(
        #[starlark(require = named, default = UnpackDictEntries::default())]
        attrs: UnpackDictEntries<&'v str, &'v StarlarkAttribute>,
        #[starlark(require = named, default = "")] doc: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkTagClass<'v>> {
        // When there's no build context (standalone mode), allow the call.
        if let Some(build_context) = eval.extra.and_then(|e| e.downcast_ref::<BuildContext>()) {
            match &build_context.additional {
                PerFileTypeContext::Bzl(_) => {}
                _ => {
                    return Err(
                        kuro_error::Error::from(ModuleExtensionError::TagClassNotInBzl).into(),
                    );
                }
            }
        }

        let attrs_vec: Vec<(String, StarlarkAttribute)> = attrs
            .entries
            .into_iter()
            .map(|(name, attr)| {
                (
                    name.to_owned(),
                    StarlarkAttribute::new(attr.clone_attribute()),
                )
            })
            .collect();

        Ok(StarlarkTagClass {
            attrs: attrs_vec,
            doc: if doc.is_empty() {
                None
            } else {
                Some(doc.to_owned())
            },
            _phantom: std::marker::PhantomData,
        })
    }

    /// Define a module extension.
    ///
    /// Module extensions allow custom dependency resolution logic in bzlmod. They
    /// are invoked via `use_extension()` in MODULE.bazel files and can create
    /// repositories based on tags collected from all modules in the dependency graph.
    ///
    /// Example:
    /// ```python
    /// def _pip_impl(module_ctx):
    ///     for mod in module_ctx.modules:
    ///         for tag in mod.tags.parse:
    ///             # Process each pip.parse() call
    ///             pass
    ///
    /// pip = module_extension(
    ///     implementation = _pip_impl,
    ///     tag_classes = {
    ///         "parse": tag_class(attrs = {...}),
    ///     },
    /// )
    /// ```
    ///
    /// NOTE: This is currently a stub implementation (Phase 5). The extension can be
    /// defined and its tag_classes are recorded, but the implementation function is
    /// not yet called during module resolution.
    fn module_extension<'v>(
        #[starlark(require = named)] implementation: Value<'v>,
        #[starlark(require = named, default = UnpackDictEntries::default())]
        tag_classes: UnpackDictEntries<&'v str, Value<'v>>,
        #[starlark(require = named, default = false)] os_dependent: bool,
        #[starlark(require = named, default = false)] arch_dependent: bool,
        #[starlark(require = named, default = "")] doc: &str,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        environ: UnpackListOrTuple<String>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkModuleExtension<'v>> {
        let tag_classes_vec: Vec<(String, Value<'v>)> = tag_classes
            .entries
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect();

        Ok(StarlarkModuleExtension::new(
            implementation,
            tag_classes_vec,
            os_dependent,
            arch_dependent,
            doc,
            environ.items,
            eval,
        )?)
    }

    /// Type symbol for module_extension.
    const module_extension_type: StarlarkValueAsType<StarlarkModuleExtension> =
        StarlarkValueAsType::new();

    /// Type symbol for tag_class.
    const tag_class_type: StarlarkValueAsType<StarlarkTagClass> = StarlarkValueAsType::new();
}
