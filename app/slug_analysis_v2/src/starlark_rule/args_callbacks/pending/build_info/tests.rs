//! Source-owned string copy and repeated Header validation quota controls.
use std::mem::size_of;

use slug_build_api_v2::published_value::BYTE_LIMIT;
use slug_build_api_v2::published_value::NODE_LIMIT;

use super::*;
#[test]
fn source_template_strings_and_new_header_validation_pay_same_ledger_before_copy() {
    use slug_build_api_v2::ActionOutput;
    use slug_build_api_v2::ActionOutputKind;
    use slug_build_api_v2::AnalysisArtifact;
    use slug_build_api_v2::BuildInfoTransformHeader;
    use slug_build_api_v2::BuildInfoTransformKind;
    use slug_identity_v2::ApparentLabel;
    use slug_identity_v2::CanonicalLabel;
    use slug_identity_v2::RepositoryMapping;
    use slug_identity_v2::RepositoryMappingId;

    use crate::starlark_rule::context_lifecycle_tests::configuration;
    use crate::starlark_rule::context_lifecycle_tests::declaration_sink;
    let key = crate::ConfiguredTargetKey::new(
        CanonicalLabel::parse("@@//:owner").unwrap(),
        configuration(),
    );
    let sink = declaration_sink(&key);
    let mapping = RepositoryMapping::new(RepositoryMappingId::new("actual-mapping-id").unwrap());
    let label = ApparentLabel::parse("@repository//long/package:long_target")
        .unwrap()
        .resolve(&mapping);
    let bytes =
        "repository".len() + "long/package".len() + "long_target".len() + "actual-mapping-id".len();
    for (template, copy_bytes) in [
        (AnalysisArtifact::Source(label.clone()), bytes),
        (
            AnalysisArtifact::Derived {
                owner: key.artifact_owner(),
                output: ActionOutput::new(
                    sink.root.clone(),
                    "derived.template",
                    ActionOutputKind::File,
                ),
            },
            0,
        ),
    ] {
        let header = BuildInfoTransformHeader::new(
            BuildInfoTransformKind::Info,
            sink.workspace_status.stable().clone(),
            template,
            ActionOutput::new(sink.root.clone(), "output.h", ActionOutputKind::File),
        )
        .unwrap();
        let work = 4 + 6 * "output.h".len() + 16;
        let storage = 8 * size_of::<usize>() + copy_bytes;
        let mut tape = Vec::new();
        prepare_header_copy(&header, &mut |r, b| {
            tape.push((r, b));
            Ok(())
        })
        .unwrap();
        assert_eq!(
            tape,
            [
                (4, 8 * size_of::<usize>()),
                (6 * "output.h".len() + 16, copy_bytes)
            ]
        );
        for (w, b, ok) in [
            (NODE_LIMIT - work, BYTE_LIMIT - storage, true),
            (NODE_LIMIT - work + 1, 0, false),
            (0, BYTE_LIMIT - storage + 1, false),
        ] {
            let mut graph = slug_build_api_v2::published_value::PublishedValueArenaBuilder::new();
            graph
                .charge_storage(slug_build_api_v2::DepsetStorageCharge {
                    nodes: 0,
                    edges: 0,
                    bytes: b,
                    visits: w,
                })
                .unwrap();
            let mut copies = 0;
            let result = prepare_header_copy(&header, &mut |r, b| {
                graph
                    .charge_storage(slug_build_api_v2::DepsetStorageCharge {
                        nodes: 0,
                        edges: 0,
                        bytes: b,
                        visits: r,
                    })
                    .map_err(|e| e.to_string())
            });
            assert_eq!(result.is_ok(), ok);
            if ok {
                let copy = header.template().clone();
                copies += 1;
                assert_eq!(&copy, header.template());
                assert_eq!(graph.charged_visits(), NODE_LIMIT);
                assert_eq!(graph.charged_bytes(), BYTE_LIMIT);
            } else {
                assert!(graph.charge_visit().is_err());
            }
            assert_eq!(copies, usize::from(ok));
        }
    }
}
