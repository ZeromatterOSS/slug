//! Borrowed native wrapper checks; invocation always uses the actual original callable.
use super::DefParameter;
use super::parameter_views;
use crate::values::Value;
use crate::values::ValueLike;
use crate::values::function::BoundMethod;
use crate::values::function::NativeFunction;
use crate::values::function::NativeMethod;

#[cfg(test)]
#[path = "native_tests.rs"]
mod tests;

/// Typed native-function dependency, not a repr or an authentication decision.
#[derive(Clone, Copy)]
pub struct NativeFunctionInspection<'v> {
    value: Value<'v>,
    function: &'v NativeFunction,
}
/// Inspect an actual engine native function; returns None for other callable kinds.
pub fn inspect_native_function(value: Value<'_>) -> Option<NativeFunctionInspection<'_>> {
    value
        .downcast_ref::<NativeFunction>()
        .map(|function| NativeFunctionInspection { value, function })
}
impl<'v> NativeFunctionInspection<'v> {
    /// The actual native function value, borrowing its existing owner.
    pub fn value(self) -> Value<'v> {
        self.value
    }
    /// Registered API name; this alone does not establish semantic identity or authority.
    pub fn name(self) -> &'v str {
        &self.function.name
    }
    /// Number of actual native parameters, including variadic entries.
    pub fn parameter_count(self) -> usize {
        self.function.function.1.len()
    }
    /// Actual parameter layout and already-evaluated defaults, without copying values.
    ///
    /// Per-parameter annotations are None; wrapper matching validates the complete
    /// native type metadata. Hosts must retain these actual layout/default graph edges.
    pub fn parameters(self) -> impl ExactSizeIterator<Item = DefParameter<'v>> {
        parameter_views(self.function.function.1.as_value())
    }
    /// Compare implementation and every wrapper metadata field to a canonical native.
    ///
    /// Function addresses are compared transiently and are never published as identity
    /// or used to replace the original callee. Parameter/default graph equality remains
    /// the host's separate responsibility; this matcher does not substitute canonical
    /// defaults for the actual values returned by `parameters()`.
    pub fn matches_canonical_wrapper(self, other: NativeFunctionInspection<'_>) -> bool {
        let actual = self.function;
        let canonical = other.function;
        std::ptr::fn_addr_eq(actual.function.0, canonical.function.0)
            && actual.name == canonical.name
            && actual.function.1.inspection_function_name()
                == canonical.function.1.inspection_function_name()
            && actual.as_type == canonical.as_type
            && actual.ty == canonical.ty
            && actual
                .special_builtin_function
                .as_ref()
                .map(std::mem::discriminant)
                == canonical
                    .special_builtin_function
                    .as_ref()
                    .map(std::mem::discriminant)
            && actual.speculative_exec_safe == canonical.speculative_exec_safe
            && actual.docs == canonical.docs
    }
}

/// Borrowed actual native bound method and its separately captured receiver.
/// No callable, parameter, default, or receiver value is copied or replaced.
#[derive(Clone, Copy)]
pub struct NativeBoundMethodInspection<'v> {
    value: Value<'v>,
    receiver: Value<'v>,
    method: &'v NativeMethod,
}

/// Inspect actual mutable or frozen Engine BoundMethod values only.
pub fn inspect_native_bound_method(value: Value<'_>) -> Option<NativeBoundMethodInspection<'_>> {
    // Existing typed coerce handles both mutable and frozen BoundMethodGen.
    let bound = BoundMethod::from_value(value)?;
    Some(NativeBoundMethodInspection {
        value,
        receiver: bound.this,
        method: bound.method.as_ref(),
    })
}
impl<'v> NativeBoundMethodInspection<'v> {
    /// Actual wrapper, borrowing its existing owner.
    pub fn value(self) -> Value<'v> {
        self.value
    }
    /// Actual receiver, to be retained as a separate semantic graph dependency.
    pub fn receiver(self) -> Value<'v> {
        self.receiver
    }
    /// Registered API method name; never sufficient alone for certification.
    pub fn name(self) -> &'v str {
        &self.method.name
    }
    /// Number of actual parameters; the receiver is separate from this layout.
    pub fn parameter_count(self) -> usize {
        self.method.function.1.len()
    }
    /// Borrow actual layout and evaluated defaults. Native annotations are None;
    /// complete native type metadata is checked by the wrapper matcher.
    pub fn parameters(self) -> impl ExactSizeIterator<Item = DefParameter<'v>> {
        parameter_views(self.method.function.1.as_value())
    }
    /// Compare every fixed NativeMethod wrapper field against the receiver's
    /// registered immutable Methods entry, rejecting missing/attribute entries.
    ///
    /// Hosts must first certify the actual receiver class through typed probes.
    /// This does not certify arbitrary custom receiver tables. Implementation
    /// addresses are only transient comparisons, never semantic result bytes.
    /// Actual parameter layout/defaults and receiver remain separate graph inputs;
    /// this matcher never substitutes canonical parameters or execution.
    pub fn matches_receiver_wrapper(self) -> bool {
        let Some(methods) = self.receiver.get_ref().vtable().methods() else {
            return false;
        };
        let Some(value) = methods.get(self.name()) else {
            return false;
        };
        let Some(canonical) = value.downcast_ref::<NativeMethod>() else {
            return false;
        };
        let actual = self.method;
        std::ptr::fn_addr_eq(actual.function.0, canonical.function.0)
            && actual.name == canonical.name
            && actual.function.1.inspection_function_name()
                == canonical.function.1.inspection_function_name()
            && actual.ty == canonical.ty
            && actual.speculative_exec_safe == canonical.speculative_exec_safe
            && actual.docs == canonical.docs
    }
}

#[cfg(test)]
#[path = "native_bound_method_tests.rs"]
mod bound_method_tests;
