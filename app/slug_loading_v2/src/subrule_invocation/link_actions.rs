//! Default link factory facade over the original evaluator-local action owner.
//! Bazel 9.2 CcStarlarkInternal.java:442-483 copies the parent context.
use super::*;

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct WrappedLinkActions {
    actions: AnalysisActions,
}

impl fmt::Display for WrappedLinkActions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.actions, f)
    }
}

starlark::starlark_simple_value!(WrappedLinkActions);

pub(super) fn unpack(value: Value<'_>) -> Option<&AnalysisActions> {
    WrappedLinkActions::from_value(value).map(|wrapper| &wrapper.actions)
}

pub(crate) fn wrap<'v>(
    actions: Value<'v>,
    build_configuration: Option<Value<'v>>,
    sharable_artifacts: bool,
    eval: &mut Evaluator<'v, '_, '_>,
) -> anyhow::Result<Value<'v>> {
    anyhow::ensure!(
        build_configuration.is_none_or(|value| value.is_none()),
        "wrap_link_actions alternative build_configuration is unsupported/deferred"
    );
    anyhow::ensure!(
        !sharable_artifacts,
        "wrap_link_actions sharable_artifacts=True is unsupported/deferred"
    );
    // This permits suspended live frames; mutations still use the original token.
    actions_to_context(actions, eval)?;
    let actions = AnalysisActions::from_starlark(actions).expect("validated genuine actions");
    Ok(eval.heap().alloc_simple(WrappedLinkActions {
        actions: actions.clone(),
    }))
}

fn wrapped_methods(builder: &mut MethodsBuilder) {
    analysis_actions_methods(builder);
    deferred_methods(builder);
}

#[starlark_module]
fn deferred_methods(builder: &mut MethodsBuilder) {
    fn declare_shareable_artifact<'v>(
        this: &WrappedLinkActions,
        #[starlark(require = pos)] path: &str,
        #[starlark(require = pos)] artifact_root: Option<Value<'v>>,
    ) -> anyhow::Result<NoneType> {
        let _ = (path, artifact_root);
        this.actions
            .token
            .require_active("declare_shareable_artifact", this.actions.context_name)?;
        anyhow::bail!("link-actions declare_shareable_artifact is unsupported/deferred")
    }

    fn declare_shareable_directory(
        this: &WrappedLinkActions,
        #[starlark(require = pos)] path: &str,
    ) -> anyhow::Result<NoneType> {
        let _ = path;
        this.actions
            .token
            .require_active("declare_shareable_directory", this.actions.context_name)?;
        anyhow::bail!("link-actions declare_shareable_directory is unsupported/deferred")
    }
}

#[starlark_value(type = "analysis_actions")]
impl<'v> StarlarkValue<'v> for WrappedLinkActions {
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(wrapped_methods)
    }
}

#[cfg(test)]
mod tests;
