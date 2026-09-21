//! Narrow native capability implemented only by the real analysis rule context.

use starlark::any::ProvidesStaticType;

pub trait CcCompileOutputContext {
    /// Validate the live ctx and complete supplied configuration before touching
    /// the existing declaration sink. The label locates; ctx remains the owner.
    fn declare_compile_output(
        &self,
        label: &slug_identity_v2::CanonicalLabel,
        output_name: &str,
        configuration: &slug_configuration_v2::SlugConfiguration,
    ) -> anyhow::Result<crate::subrule_invocation::AnalysisArtifactValue>;
}

unsafe impl<'v> ProvidesStaticType<'v> for &'v dyn CcCompileOutputContext {
    type StaticType = &'static dyn CcCompileOutputContext;
}

pub trait CcExecutionContext {
    /// Resolve from the already-selected default execution platform. Implementors
    /// must check context lifetime and must not observe ambient process state.
    fn execution_os(&self) -> anyhow::Result<&'static str>;
}

// The capability is borrowed only for a synchronous Starlark native call.
unsafe impl<'v> ProvidesStaticType<'v> for &'v dyn CcExecutionContext {
    type StaticType = &'static dyn CcExecutionContext;
}
