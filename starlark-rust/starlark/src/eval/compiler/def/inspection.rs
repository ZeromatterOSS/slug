//! Borrowed function inspection. Hosts own provenance and semantic graph publication.
use super::Def;
use super::DefGen;
use super::DefInfo;
use super::FrozenDef;
use crate::codemap::CodeMap;
use crate::codemap::FileSpanRef;
use crate::collections::Hashed;
use crate::environment::FrozenModuleData;
use crate::environment::Module;
use crate::environment::slots::ModuleSlotId;
use crate::eval::ParametersSpec;
use crate::eval::runtime::params::spec::ParameterKind;
use crate::values::FrozenRef;
use crate::values::Value;
use crate::values::ValueLike;
use crate::values::layout::value_captured::FrozenValueCaptured;
use crate::values::layout::value_captured::ValueCaptured;
use crate::values::layout::value_captured::value_captured_get;

mod native;
#[cfg(test)]
mod tests;
mod walk;
pub use native::NativeBoundMethodInspection;
pub use native::NativeFunctionInspection;
pub use native::inspect_native_bound_method;
pub use native::inspect_native_function;

#[derive(Debug, thiserror::Error)]
enum InspectionError {
    #[error("function inspection requires its original defining module")]
    MissingOriginalModule,
    #[error("function inspection found an unnamed module slot")]
    UnnamedModuleSlot,
    #[error("function inspection exhausted its node budget")]
    NodeBudget,
    #[error("function inspection exceeded its depth budget")]
    DepthBudget,
}

/// Shared scratch budget; inspections never collect dependencies into a result vector.
#[derive(Debug)]
pub struct InspectionBudget {
    /// Remaining IR steps and dependency events, shared across inspections.
    pub remaining_nodes: usize,
    /// Maximum recursive IR depth permitted before returning an error.
    pub maximum_depth: usize,
}
impl InspectionBudget {
    fn step(&mut self, depth: usize) -> crate::Result<()> {
        if depth > self.maximum_depth {
            return Err(crate::Error::new_other(InspectionError::DepthBudget));
        }
        self.remaining_nodes = self
            .remaining_nodes
            .checked_sub(1)
            .ok_or_else(|| crate::Error::new_other(InspectionError::NodeBudget))?;
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum OriginalModule<'v> {
    Frozen(FrozenRef<'v, FrozenModuleData>),
    Mutable(&'v Module),
    Missing,
}
impl<'v> OriginalModule<'v> {
    fn named(self, name: &str) -> crate::Result<Option<Value<'v>>> {
        match self {
            Self::Frozen(module) => Ok(module
                .inspection_named_slot(name)
                .and_then(|slot| module.get_slot(slot))
                .map(Value::new_frozen)),
            Self::Mutable(module) => Ok(module.get_any_visibility(Hashed::new(name)).map(|x| x.0)),
            Self::Missing => Err(crate::Error::new_other(
                InspectionError::MissingOriginalModule,
            )),
        }
    }
    fn slot(self, slot: ModuleSlotId) -> crate::Result<(&'v str, Option<Value<'v>>)> {
        let (name, value) = match self {
            Self::Frozen(module) => (
                module.inspection_slot_name(slot),
                module.get_slot(slot).map(Value::new_frozen),
            ),
            Self::Mutable(module) => (
                module.mutable_names().get_slot(slot),
                module.slots().get_slot(slot),
            ),
            Self::Missing => {
                return Err(crate::Error::new_other(
                    InspectionError::MissingOriginalModule,
                ));
            }
        };
        Ok((
            name.ok_or_else(|| crate::Error::new_other(InspectionError::UnnamedModuleSlot))?
                .as_str(),
            value,
        ))
    }
    fn client_data(self) -> Option<Value<'v>> {
        match self {
            Self::Frozen(module) => module.client_data.map(Value::new_frozen),
            Self::Mutable(module) => module.client_data(),
            Self::Missing => None,
        }
    }
}

#[derive(Clone, Copy)]
enum DefRef<'v> {
    Mutable(&'v Def<'v>),
    Frozen(&'v FrozenDef),
}

/// A borrowed ordinary Starlark def, including its actual original module.
#[derive(Clone, Copy)]
pub struct DefInspection<'v> {
    value: Value<'v>,
    function: DefRef<'v>,
    module: OriginalModule<'v>,
}

/// Inspect without issuing occurrence identity or changing any value.
///
/// The fallback must be the function's actual original mutable defining module.
/// It is used only if the function has no retained original frozen module.
/// Inspection of a frozen original module never substitutes consumer metadata.
pub fn inspect_def<'v>(
    value: Value<'v>,
    original_unfrozen_module: Option<&'v Module>,
) -> Option<DefInspection<'v>> {
    fn module<'v, V>(function: &DefGen<V>, fallback: Option<&'v Module>) -> OriginalModule<'v> {
        function
            .module
            .load_relaxed()
            .map(OriginalModule::Frozen)
            .unwrap_or_else(|| {
                fallback
                    .map(OriginalModule::Mutable)
                    .unwrap_or(OriginalModule::Missing)
            })
    }
    let (function, module) = if let Some(function) = value.downcast_ref::<Def>() {
        (
            DefRef::Mutable(function),
            module(function, original_unfrozen_module),
        )
    } else if let Some(function) = value.downcast_ref::<FrozenDef>() {
        (
            DefRef::Frozen(function),
            module(function, original_unfrozen_module),
        )
    } else {
        return None;
    };
    Some(DefInspection {
        value,
        function,
        module,
    })
}

/// How a parameter accepts arguments in the original function signature.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ParameterStyle {
    /// Accepts positional arguments only.
    PositionalOnly,
    /// Accepts positional or named arguments.
    PositionalOrNamed,
    /// Accepts named arguments only.
    NamedOnly,
    /// Collects remaining positional arguments.
    Args,
    /// Collects remaining named arguments.
    Kwargs,
}
/// Whether a parameter requires an argument or retains a default value.
#[derive(Debug, Clone, Copy)]
pub enum ParameterRequirement<'v> {
    /// An argument must be supplied.
    Required,
    /// A native optional parameter may remain unassigned.
    Optional,
    /// The original already-evaluated default value.
    Defaulted(Value<'v>),
    /// Collects variadic arguments instead of accepting a single value.
    Variadic,
}
/// One borrowed parameter, including its actual default and runtime annotation.
#[derive(Debug, Clone, Copy)]
pub struct DefParameter<'v> {
    /// Index in the function's original parameter layout.
    pub index: usize,
    /// Original parameter name, including the engine's variadic prefix if present.
    pub name: &'v str,
    /// Permitted argument passing style.
    pub style: ParameterStyle,
    /// Required, optional, defaulted, or variadic argument behavior.
    pub requirement: ParameterRequirement<'v>,
    /// Compiled runtime annotation, when the parameter has one.
    pub annotation: Option<Value<'v>>,
}
/// One captured cell and its current optional contents, preserving cell identity.
#[derive(Debug, Clone, Copy)]
pub struct CaptureCell<'v> {
    /// Name of the captured variable in the inspected function's local scope.
    pub child_name: &'v str,
    /// The captured cell itself, including its own ValueIdentity.
    pub cell: Value<'v>,
    /// Current payload; None denotes an unassigned captured cell.
    pub contents: Option<Value<'v>>,
}

impl<'v> DefInspection<'v> {
    fn info(self) -> &'v DefInfo {
        match self.function {
            DefRef::Mutable(function) => function.def_info.as_ref(),
            DefRef::Frozen(function) => function.def_info.as_ref(),
        }
    }
    fn parameters_spec(self) -> &'v ParametersSpec<Value<'v>> {
        match self.function {
            DefRef::Mutable(function) => &function.parameters,
            DefRef::Frozen(function) => function.parameters.as_value(),
        }
    }
    /// The actual inspected function value, borrowing its existing owner.
    pub fn value(self) -> Value<'v> {
        self.value
    }
    /// Original function name used for its module binding lookup.
    pub fn name(self) -> &'v str {
        self.info().name.as_str()
    }
    /// Source span of the original function signature.
    pub fn signature_span(self) -> FileSpanRef<'v> {
        self.info().signature_span.file_span_ref()
    }
    /// Original source map; this observation does not authenticate its contents.
    pub fn source(self) -> &'v CodeMap {
        self.info().codemap.as_ref()
    }
    /// Client metadata of the original module, without consumer-module substitution.
    pub fn original_module_client_data(self) -> Option<Value<'v>> {
        self.module.client_data()
    }
    /// Number of parameters, including any variadic parameter entries.
    pub fn parameter_count(self) -> usize {
        self.parameters_spec().len()
    }
    /// Borrow parameters in their original signature order without formatting them.
    pub fn parameters(self) -> impl ExactSizeIterator<Item = DefParameter<'v>> {
        parameter_views(self.parameters_spec()).map(move |mut parameter| {
            parameter.annotation = match self.function {
                DefRef::Mutable(function) => {
                    function.inspection_parameter_annotation(parameter.index)
                }
                DefRef::Frozen(function) => {
                    function.inspection_parameter_annotation(parameter.index)
                }
            }
            .map(|value| value.to_value());
            parameter
        })
    }
    /// Compiled return annotation, when the original function has one.
    pub fn return_annotation(self) -> Option<Value<'v>> {
        match self.function {
            DefRef::Mutable(function) => function.return_type,
            DefRef::Frozen(function) => function.return_type,
        }
        .map(|ty| ty.to_inner().to_value())
    }
    /// Borrow original capture cells without unwrapping away their occurrence identity.
    pub fn captures(self) -> impl ExactSizeIterator<Item = CaptureCell<'v>> {
        let length = match self.function {
            DefRef::Mutable(function) => function.captured.len(),
            DefRef::Frozen(function) => function.captured.len(),
        };
        (0..length).map(move |index| {
            let cell = match self.function {
                DefRef::Mutable(function) => function.captured[index],
                DefRef::Frozen(function) => function.captured[index].to_value(),
            };
            debug_assert!(
                cell.downcast_ref::<ValueCaptured>().is_some()
                    || cell.downcast_ref::<FrozenValueCaptured>().is_some()
            );
            let child = self.info().parent[index].child.0 as usize;
            CaptureCell {
                child_name: self.info().used[child].as_str(),
                cell,
                contents: value_captured_get(cell),
            }
        })
    }
    /// Original-module binding under this function's actual name, including private names.
    /// Returns an error when no original module or explicit mutable fallback is available.
    pub fn global_binding(self) -> crate::Result<Option<Value<'v>>> {
        self.module.named(self.name())
    }
    /// Bazel 9.2 StarlarkFunction.isGlobal: actual original-module name binding identity.
    pub fn is_global(self) -> crate::Result<bool> {
        Ok(self
            .global_binding()?
            .is_some_and(|value| value.ptr_eq(self.value)))
    }
    /// Borrow retained pre-freeze code with its original-module slot resolver.
    pub fn code(self) -> DefCodeInspection<'v> {
        DefCodeInspection {
            info: self.info(),
            module: self.module,
        }
    }
}

/// Borrowed retained function code, including code for uninstantiated nested defs.
#[derive(Clone, Copy)]
pub struct DefCodeInspection<'v> {
    info: &'v DefInfo,
    module: OriginalModule<'v>,
}
impl<'v> DefCodeInspection<'v> {
    /// Name recorded for this original code definition.
    pub fn name(self) -> &'v str {
        self.info.name.as_str()
    }
    /// Source span of this original code definition's signature.
    pub fn signature_span(self) -> FileSpanRef<'v> {
        self.info.signature_span.file_span_ref()
    }
    /// Source map owning this code definition.
    pub fn source(self) -> &'v CodeMap {
        self.info.codemap.as_ref()
    }
    /// Client metadata of the module against which code references are resolved.
    pub fn original_module_client_data(self) -> Option<Value<'v>> {
        self.module.client_data()
    }
    /// Walk pre-freeze IR, including nested code, without evaluating or retaining values.
    pub fn visit_dependencies(
        self,
        budget: &mut InspectionBudget,
        visitor: &mut dyn FnMut(DefDependency<'v>) -> crate::Result<()>,
    ) -> crate::Result<()> {
        walk::visit(self, budget, visitor)
    }
}

/// How retained code accesses a referenced original-module slot.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ModuleAccess {
    /// Reads an existing module value.
    Read,
    /// Assigns a module value.
    Write,
    /// Reads and updates a module value.
    ReadWrite,
}
/// A borrowed code dependency emitted by bounded IR inspection.
pub enum DefDependency<'v> {
    /// A symbol actually referenced by code, resolved only in its original module.
    ModuleSymbol {
        /// Original module name, including private or imported bindings.
        name: &'v str,
        /// Current binding value; None denotes a named but unassigned slot.
        value: Option<Value<'v>>,
        /// Read, write, or read/update access in the inspected code.
        access: ModuleAccess,
    },
    /// A retained constant value, possibly containing other native or callable values.
    Constant(Value<'v>),
    /// A compiled runtime type value referenced by the inspected code.
    Type(Value<'v>),
    /// An actual native-function constant, exposed through typed inspection.
    NativeFunction(NativeFunctionInspection<'v>),
    /// Emitted before nested code is automatically visited; do not re-walk this event.
    NestedCode(DefCodeInspection<'v>),
    /// An original source span, including spans carried by compiler-inlined code.
    Source(FileSpanRef<'v>),
}

fn parameter_views<'v>(
    spec: &'v ParametersSpec<Value<'v>>,
) -> impl ExactSizeIterator<Item = DefParameter<'v>> {
    let (names, kinds, indices) = spec.inspection_parts();
    names
        .iter()
        .zip(kinds)
        .enumerate()
        .map(move |(index, (name, kind))| {
            let (style, requirement) = match kind {
                ParameterKind::Args => (ParameterStyle::Args, ParameterRequirement::Variadic),
                ParameterKind::KWargs => (ParameterStyle::Kwargs, ParameterRequirement::Variadic),
                kind => {
                    let style = if index < indices.num_positional_only as usize {
                        ParameterStyle::PositionalOnly
                    } else if index < indices.num_positional as usize {
                        ParameterStyle::PositionalOrNamed
                    } else {
                        ParameterStyle::NamedOnly
                    };
                    let requirement = match kind {
                        ParameterKind::Required => ParameterRequirement::Required,
                        ParameterKind::Optional => ParameterRequirement::Optional,
                        ParameterKind::Defaulted(value) => ParameterRequirement::Defaulted(*value),
                        ParameterKind::Args | ParameterKind::KWargs => unreachable!(),
                    };
                    (style, requirement)
                }
            };
            DefParameter {
                index,
                name,
                style,
                requirement,
                annotation: None,
            }
        })
}
