//! Genuine native values survive retained nested dependency publication.
use std::sync::Arc;

use slug_build_api_v2::ProviderId;
use slug_build_api_v2::RunEnvironmentInfo;
use slug_identity_v2::CanonicalLabel;
use starlark::environment::Globals;
use starlark::environment::Module;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use super::*;

#[test]
fn run_environment_roundtrip_preserves_nested_publication_and_mutable_getters_after_drop() {
    let retained = {
        let heap = FrozenHeap::new();
        let info = RunEnvironmentInfo::new(
            [
                (CompactString::new("Z"), CompactString::new("last")),
                (CompactString::new("A"), CompactString::new("first")),
            ]
            .into_iter()
            .collect(),
            Arc::from([CompactString::new("Z"), CompactString::new("Z")]),
        );
        let carrier = ProviderOccurrence::new(
            ProviderIdentity::user(ProviderId::new("//:environment.bzl", "Carrier").unwrap()),
            [(
                "environment",
                AnalysisValue::run_environment_info(info.dupe()),
            )],
        );
        let target = ConfiguredTargetValue::new(
            AnalysisTargetIdentity::null(CanonicalLabel::parse("@@//:target").unwrap()),
            ProviderCollection::from_values(
                vec![ProviderValue::RunEnvironmentInfo(info.dupe())],
                false,
            )
            .unwrap(),
        );
        let expected = AnalysisValue::tuple(vec![
            AnalysisValue::configured_target(target),
            AnalysisValue::run_environment_info(info),
            AnalysisValue::provider(carrier),
        ]);
        let allocated = AnalysisValueMaterializer::new(&heap)
            .value(&expected)
            .unwrap();
        let lowered = AnalysisValueLowerer::default()
            .lower(allocated.to_value(), "$source")
            .unwrap();
        assert!(expected.publication_eq(&lowered));
        lowered
    };
    let module = Module::new();
    let values = AnalysisValueMaterializer::new(module.frozen_heap())
        .value(&retained)
        .unwrap();
    module.set("values", values.to_value());
    module.set(
        "RunEnv",
        slug_loading_v2::provider::alloc_starlark_provider_callable(
            module.frozen_heap(),
            "RunEnvironmentInfo",
        )
        .unwrap()
        .to_value(),
    );
    Evaluator::new(&module)
        .eval_module(
            AstModule::parse(
                "roundtrip.bzl",
                r#"
target, direct, carrier = values
lookup = {direct: "stable"}
copy = carrier.environment.environment
copy["Z"] = "changed"
names = direct.inherited_environment
names.clear()
RESULT = [target[RunEnv] == direct, carrier.environment == direct,
          direct.environment.keys(), direct.environment["Z"],
          direct.inherited_environment, lookup[direct]]
"#
                .to_owned(),
                &Dialect::Bazel,
            )
            .unwrap(),
            &Globals::standard(),
        )
        .unwrap();
    assert_eq!(
        module.get("RESULT").unwrap().to_repr(),
        "[True, True, [\"Z\", \"A\"], \"last\", [\"Z\", \"Z\"], \"stable\"]"
    );
}
