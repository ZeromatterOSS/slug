use super::*;

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct Lookalike;
impl fmt::Display for Lookalike {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("depset")
    }
}
starlark::starlark_simple_value!(Lookalike);
#[starlark_value(type = "analysis_builtin")]
impl<'v> StarlarkValue<'v> for Lookalike {}

#[test]
fn analysis_builtin_certificate_uses_actual_class_and_complete_immutable_state() {
    let module = starlark::environment::Module::new();
    let frozen = starlark::values::FrozenHeap::new();
    for (name, api) in [
        ("depset", AnalysisBuiltinCallableApi::Depset),
        ("DefaultInfo", AnalysisBuiltinCallableApi::DefaultInfo),
        ("ToolchainInfo", AnalysisBuiltinCallableApi::ToolchainInfo),
    ] {
        let live = module
            .heap()
            .alloc_simple(AnalysisBuiltinCallable::new(name));
        let owned = alloc_frozen_analysis_builtin_callable(&frozen, api);
        assert_eq!(analysis_builtin_callable_api(live), Some(api));
        assert_eq!(analysis_builtin_callable_api(owned.to_value()), Some(api));
    }
    let unknown = module
        .heap()
        .alloc_simple(AnalysisBuiltinCallable::new("unknown"));
    assert_eq!(analysis_builtin_callable_api(unknown), None);
    let lookalike = module.heap().alloc_simple(Lookalike);
    assert_eq!(lookalike.get_type(), "analysis_builtin");
    assert_eq!(lookalike.to_str(), "depset");
    assert_eq!(analysis_builtin_callable_api(lookalike), None);
    assert_eq!(
        analysis_builtin_callable_api(module.heap().alloc("depset")),
        None
    );
}
