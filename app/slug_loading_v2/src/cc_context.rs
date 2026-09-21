//! Narrow native capability implemented only by the real analysis rule context.

use starlark::any::ProvidesStaticType;

pub trait CcExecutionContext {
    /// Resolve from the already-selected default execution platform. Implementors
    /// must check context lifetime and must not observe ambient process state.
    fn execution_os(&self) -> anyhow::Result<&'static str>;
}

// The capability is borrowed only for a synchronous Starlark native call.
unsafe impl<'v> ProvidesStaticType<'v> for &'v dyn CcExecutionContext {
    type StaticType = &'static dyn CcExecutionContext;
}
