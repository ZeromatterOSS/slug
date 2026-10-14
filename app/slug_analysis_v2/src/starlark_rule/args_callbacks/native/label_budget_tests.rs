use std::sync::Arc;

use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::published_value::PublishedValueArenaBuilder;
use slug_identity_v2::ApparentLabel;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::RepositoryMapping;
use slug_identity_v2::RepositoryMappingId;
use starlark::environment::Module;
use starlark::values::Value;

use super::pure_candidate;

fn labels() -> (CanonicalLabel, CanonicalLabel, usize) {
    let id = "complete-copy-provenance".repeat(64);
    let mapping = RepositoryMapping::new(RepositoryMappingId::new(id.clone()).unwrap());
    (
        CanonicalLabel::parse("@@//pkg:leaf").unwrap(),
        ApparentLabel::parse("//pkg:leaf")
            .unwrap()
            .resolve(&mapping),
        id.len(),
    )
}
fn value<'v>(module: &'v Module, label: CanonicalLabel, route: usize, path: &str) -> Value<'v> {
    use slug_build_api_v2::ActionOutput;
    use slug_build_api_v2::ActionOutputKind;
    use slug_build_api_v2::AnalysisConfiguredTargetKey;
    if route == 0 {
        return slug_loading_v2::provider::alloc_starlark_label(module.heap(), label);
    }
    let artifact = if route == 1 {
        AnalysisArtifact::Source(label)
    } else {
        let owner = if route == 2 {
            AnalysisConfiguredTargetKey::new(label, b"cfg".as_slice())
        } else {
            AnalysisConfiguredTargetKey::new(
                CanonicalLabel::parse("@@//pkg:owner").unwrap(),
                b"cfg".as_slice(),
            )
            .with_toolchain_execution_platform(Arc::new(label))
        };
        AnalysisArtifact::Derived {
            owner,
            output: ActionOutput::new(
                crate::analysis_value::test_artifact_root(),
                path,
                ActionOutputKind::File,
            ),
        }
    };
    module
        .heap()
        .alloc_simple(slug_loading_v2::subrule_invocation::AnalysisArtifactValue::new(artifact))
}
fn bytes<'v>(module: &'v Module, value: Value<'v>) -> usize {
    let mut builder = PublishedValueArenaBuilder::new();
    let mut remaining = 65536;
    assert!(pure_candidate(module, &mut builder, &mut remaining, value, 0).unwrap());
    builder.charged_bytes()
}
fn refuses_before_lowering<'v>(
    module: &'v Module,
    plain: Value<'v>,
    expanded: Value<'v>,
    added: usize,
) {
    let baseline = bytes(module, plain);
    assert_eq!(bytes(module, expanded) - baseline, added);
    for (value, succeeds) in [(plain, true), (expanded, false)] {
        let mut builder = PublishedValueArenaBuilder::new();
        builder
            .charge_bytes(slug_build_api_v2::published_value::BYTE_LIMIT - baseline - added / 2)
            .unwrap();
        let mut remaining = 65536;
        let result = pure_candidate(module, &mut builder, &mut remaining, value, 0);
        if succeeds {
            assert!(result.unwrap());
        } else {
            assert!(result.unwrap_err().contains("bytes"));
            assert!(builder.charge_visit().is_err());
        }
    }
}

#[test]
fn borrowed_label_and_file_preflight_covers_full_owner_and_platform_mapping_ids() {
    let module = Module::new();
    let (plain, mapped, added) = labels();
    for route in 0..4 {
        refuses_before_lowering(
            &module,
            value(&module, plain.clone(), route, "pkg/output"),
            value(&module, mapped.clone(), route, "pkg/output"),
            added,
        );
    }
}

#[test]
fn borrowed_derived_file_preflight_covers_output_path_before_the_lowerer_copy() {
    let module = Module::new();
    let (label, _, _) = labels();
    let long_path = "pkg/".to_owned() + &"output".repeat(128);
    refuses_before_lowering(
        &module,
        value(&module, label.clone(), 2, "pkg/output"),
        value(&module, label, 2, &long_path),
        long_path.len() - "pkg/output".len(),
    );
}
