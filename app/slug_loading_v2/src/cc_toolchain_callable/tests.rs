//! Facade mechanics only; authenticated constructor registration is tested by
//! the provider owner and unchanged-source integration tests.
use slug_build_api_v2::AnalysisDepset;
use slug_build_api_v2::AnalysisValue;
use slug_build_api_v2::CcToolchainCallableModel;
use slug_build_api_v2::CcToolchainCallableSource;
use slug_identity_v2::CanonicalLabel;
use starlark::assert::Assert;
use starlark::environment::GlobalsBuilder;
use starlark::starlark_module;
use starlark::values::Heap;

use super::*;
use crate::provider::StarlarkDepset;
use crate::provider::StarlarkDepsetSuccessorGen;
use crate::provider::alloc_starlark_depset;

#[starlark_module]
fn helpers(builder: &mut GlobalsBuilder) {
    fn capture<'v>(eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<Value<'v>> {
        let retained = AnalysisDepset::new(
            DepsetOrder::Default,
            vec![AnalysisValue::string("runtime")],
            vec![],
        )?;
        Ok(alloc_starlark_depset(
            eval.frozen_heap(),
            retained,
            vec![StarlarkDepsetSuccessorGen::Direct(
                eval.frozen_heap().alloc("runtime"),
            )],
        )
        .to_value())
    }
    fn empty<'v>(eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<Value<'v>> {
        Ok(crate::provider::empty_evaluator_depset(
            DepsetOrder::Default,
            eval,
        ))
    }
    fn make<'v>(
        kind: &str,
        original: Value<'v>,
        capture: Value<'v>,
        heap: Heap<'v>,
    ) -> anyhow::Result<Value<'v>> {
        let kind = match kind {
            "supports" => CcToolchainCallableKind::SupportsPic,
            "always" => CcToolchainCallableKind::AlwaysPic,
            "static" => CcToolchainCallableKind::StaticRuntime,
            "dynamic" => CcToolchainCallableKind::DynamicRuntime,
            _ => anyhow::bail!("unknown test recipe"),
        };
        let identity = original
            .request_value::<&dyn StarlarkFunctionIdentity>()
            .unwrap();
        let retained_capture = if capture.is_none() {
            AnalysisValue::none()
        } else {
            AnalysisValue::depset(
                StarlarkDepset::parts_from_value(capture)
                    .unwrap()
                    .3
                    .unwrap(),
            )
        };
        let model = CcToolchainCallable::new(CcToolchainCallableModel {
            kind,
            source: CcToolchainCallableSource {
                label: CanonicalLabel::parse(
                    "@@rules_cc+//cc/private/rules_impl:cc_toolchain_info.bzl",
                )
                .unwrap(),
                digest: [1; 32],
            },
            identity: identity.issue_identity(),
            name_hash: identity.name_hash().get(),
            capture: retained_capture,
        })
        .map_err(anyhow::Error::msg)?;
        Ok(heap.alloc_complex(ToolchainCallableGen { model, capture }))
    }
}
fn harness() -> Assert<'static> {
    let mut a = Assert::new();
    a.disable_static_typechecking();
    a.globals_add(helpers);
    a
}

#[test]
fn recipes_preserve_generic_feature_results_truthiness_and_capture_identity() {
    harness().pass(
        r#"
def yes(name):
    return [name]
def no(name):
    return []
def original(*, feature_configuration):
    pass
enabled = struct(is_enabled = yes)
disabled = struct(is_enabled = no)
supports = make("supports", original, None)
assert_eq(supports(feature_configuration = enabled), ["supports_pic"])
assert_eq(supports(feature_configuration = disabled), [])
always = make("always", lambda: None, None)
assert_eq(always(feature_configuration = None), True)
captured = capture()
static = make("static", lambda: None, captured)
dynamic = make("dynamic", lambda: None, captured)
assert_eq(static(feature_configuration = enabled), captured)
assert_eq(dynamic(feature_configuration = enabled), captured)
assert_eq(static(feature_configuration = disabled), empty())
assert_eq(dynamic(feature_configuration = disabled), empty())
none = make("static", lambda: None, None)
assert_eq(none(feature_configuration = disabled), empty())
garbage_collect()
assert_eq(static(feature_configuration = enabled).to_list(), ["runtime"])
assert_eq(supports == original, True)
assert_eq(original == supports, True)
lookup = {original: "before"}
assert_eq(lookup[supports], "before")
"#,
    );
}

#[test]
fn recipes_reject_positional_unknown_missing_arguments_and_missing_runtime() {
    let a = harness();
    for call in [
        "f()",
        "f(None)",
        "f(feature_configuration = None, other = 1)",
        "f(other = None)",
        "f(*[None])",
    ] {
        a.fails(
            &format!("f = make('always', lambda: None, None)\n{call}"),
            &["argument"],
        );
    }
    for (kind, attribute) in [
        ("static", "static_runtime_lib"),
        ("dynamic", "dynamic_runtime_lib"),
    ] {
        a.fails(&format!("f = make('{kind}', lambda: None, None)\nf(feature_configuration = struct(is_enabled = lambda name: 7))"), &[&format!("Toolchain supports embedded runtimes, but didn't provide {attribute} attribute.")]);
    }
    a.fails(
        "f = make('supports', lambda: None, None)\nf(feature_configuration = None)",
        &["is_enabled"],
    );
    a.fails("f = make('supports', lambda: None, None)\nf(feature_configuration = struct(is_enabled = 1))", &["Operation `call()` not supported on type `int`"]);
}

#[test]
fn frozen_recipes_keep_capture_aliases_and_original_def_equality() {
    let mut a = harness();
    a.module(
        "producer",
        r#"
def original(*, feature_configuration):
    pass
captured = capture()
f = make("static", original, captured)
alias = f
"#,
    );
    a.pass(
        r#"
load("producer", "f", "alias", "captured", "original")
assert_eq(type(f), "function")
assert_eq(f, alias)
assert_eq(f == original, True)
assert_eq(original == f, True)
assert_eq({original: 1}[f], 1)
assert_eq({f: 2}[original], 2)
assert_eq(f(feature_configuration = struct(is_enabled = lambda name: True)), captured)
garbage_collect()
assert_eq(f(feature_configuration = struct(is_enabled = lambda name: [])), empty())
"#,
    );
}
