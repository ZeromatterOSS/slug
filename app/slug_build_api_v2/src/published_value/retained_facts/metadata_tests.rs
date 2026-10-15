use super::*;

fn symbol() -> PublishedFunctionSymbol {
    PublishedFunctionSymbol::new(
        evaluation_owner(),
        FunctionSymbolName::Exported(Arc::from("exported")),
    )
}
fn empty_code(source: CallableSourceAuthority) -> PublishedCode {
    PublishedCode {
        source,
        name: "body".into(),
        definition: crate::CallableDefinitionSpan { start: 1, end: 2 },
        dependencies: PublishedEdgeRange { start: 0, len: 1 },
        globals: Arc::from([PublishedGlobal {
            name: "global".into(),
            access: crate::CallableGlobalAccess::ReadWrite,
            value: Some(PublishedEdge::Local(PublishedNodeId::new(0).unwrap())),
        }]),
    }
}

#[test]
fn retained_records_cover_ordered_metadata_inline_edges_and_hidden_certificates() {
    let edge = PublishedEdge::Local(PublishedNodeId::new(0).unwrap());
    let range = PublishedEdgeRange { start: 0, len: 1 };
    let parameter = PublishedParameter {
        name: "argument".into(),
        mode: crate::CallableParameterMode::NamedOnly,
        requirement: PublishedParameterRequirement::Defaulted(edge),
        annotation: Some(edge),
    };
    let mut b = PublishedValueArenaBuilder::new();
    let native = b.native_atom(AnalysisValue::string("payload")).unwrap();
    let native_count = native.retained_visits();
    let files = b
        .files_to_run_atom(crate::FilesToRunProvider::empty())
        .unwrap();
    let provider = b
        .provider_value_atom(crate::ProviderValue::FilesToRunProvider(
            crate::FilesToRunProvider::empty(),
        ))
        .unwrap();
    let provider_count = provider.retained_visits();
    let files_count = files.retained_visits();
    let id = crate::ProviderId::new("//pkg:defs.bzl", "Info").unwrap();
    let rows = vec![
        PublishedNode::Function(PublishedFunction {
            symbol: symbol(),
            code: empty_code(source(0)),
            parameters: Arc::from([parameter.clone()]),
            return_annotation: Some(edge),
            capture_names: Arc::from(["captured".into()]),
            captures: range,
        }),
        PublishedNode::Intrinsic(PublishedIntrinsic {
            api: crate::NativeCallableApiId::new("test.api", 7).unwrap(),
            parameters: Arc::from([parameter]),
            return_annotation: Some(edge),
            receiver: Some(edge),
        }),
        PublishedNode::Code(empty_code(source(0))),
        PublishedNode::ProviderConstructor(PublishedProviderConstructor {
            id,
            schema: PublishedProviderSchema::Initialized(Arc::from(["field".into()])),
            init: Some(edge),
        }),
        PublishedNode::RawProviderConstructor {
            provider: edge,
            occurrence: RawProviderConstructorOccurrence::new(),
        },
        PublishedNode::Cell {
            contents: Some(edge),
        },
        PublishedNode::Set(range),
        PublishedNode::List(range),
        PublishedNode::Tuple(range),
        PublishedNode::Dictionary(PublishedEdgeRange { start: 0, len: 0 }),
        PublishedNode::Struct {
            names: Arc::from(["field".into()]),
            values: range,
        },
        PublishedNode::Provider(PublishedProvider {
            identity: crate::ProviderIdentity::builtin("ToolchainInfo"),
            has_schema: false,
            names: Arc::from(["field".into()]),
            values: range,
            files_to_run: Some(files),
        }),
        PublishedNode::ConfiguredTarget {
            identity: crate::AnalysisTargetIdentity::null(
                slug_identity_v2::CanonicalLabel::parse("@@//pkg:input").unwrap(),
            ),
            providers: Arc::from([PublishedTargetProvider::Native {
                provider,
                view: edge,
            }]),
        },
        PublishedNode::Depset(PublishedDepset {
            order: crate::DepsetOrder::Postorder,
            element_type: crate::AnalysisValueType::String,
            depth: 0,
            occurrence: crate::AnalysisDepsetOccurrence::new(),
            successors: Arc::from([
                PublishedDepsetInput::Direct(edge),
                PublishedDepsetInput::Transitive(edge),
            ]),
        }),
        PublishedNode::CcVariables(PublishedCcVariables {
            occurrence: crate::cc_variables::CcVariablesOccurrence::new(),
            kind: crate::cc_variables::CcVariablesKind::Map,
            parent: Some(edge),
            locals: Arc::from([("local".into(), edge)]),
            seed: Arc::from([
                ("value".into(), PublishedVariableSeed::Value(edge)),
                ("sequence".into(), PublishedVariableSeed::Sequence(range)),
                ("missing".into(), PublishedVariableSeed::Missing),
                (
                    "error".into(),
                    PublishedVariableSeed::Error("complete error".into()),
                ),
            ]),
        }),
        PublishedNode::Native(native),
    ];
    let expected = [
        16,
        9,
        9,
        6,
        4,
        4,
        3,
        3,
        3,
        3,
        4,
        5 + files_count,
        6 + provider_count,
        7,
        12,
        3 + native_count,
    ];
    assert_eq!(rows.len(), expected.len());
    for (index, (row, expected)) in rows.into_iter().zip(expected).enumerate() {
        let source_count = match &row {
            PublishedNode::Function(function) => function.code.source.fact_count(),
            PublishedNode::Code(code) => code.source.fact_count(),
            _ => 0,
        };
        let native_count = match index {
            11 => files_count,
            12 => provider_count,
            15 => native_count,
            _ => 0,
        };
        let (facts, _) = local_visits(
            &[Some(row)],
            &[edge],
            &[None],
            &[],
            0,
            0,
            &mut 0,
            &mut 0,
            &mut 0,
            &mut false,
        )
        .unwrap();
        assert_eq!(facts.total, expected);
        assert_eq!(facts.source, source_count);
        assert_eq!(facts.native, native_count);
        assert_eq!(facts.source + facts.native + facts.other(), expected);
    }
    let plain = PublishedNode::Provider(PublishedProvider {
        identity: crate::ProviderIdentity::builtin("ToolchainInfo"),
        has_schema: false,
        names: Arc::from([]),
        values: PublishedEdgeRange { start: 0, len: 0 },
        files_to_run: None,
    });
    let mut with_hidden = plain.clone();
    let PublishedNode::Provider(p) = &mut with_hidden else {
        unreachable!()
    };
    p.files_to_run = Some(
        b.files_to_run_atom(crate::FilesToRunProvider::empty())
            .unwrap(),
    );
    assert_eq!(
        count(&[Some(with_hidden)], &[]).unwrap() - count(&[Some(plain)], &[]).unwrap(),
        files_count
    );
    assert!(provider_count > 0);
    let label = slug_identity_v2::CanonicalLabel::parse("@@//pkg:target").unwrap();
    let target = |identity| PublishedNode::ConfiguredTarget {
        identity,
        providers: Arc::from([]),
    };
    let plain = target(crate::AnalysisTargetIdentity::null(label.clone()));
    let key = crate::AnalysisConfiguredTargetKey::new(
        label.clone(),
        b"complete-configuration".as_slice(),
    );
    let configured = target(key.clone().into());
    let platform = target(
        key.with_toolchain_execution_platform(Arc::new(label))
            .into(),
    );
    assert_eq!(
        count(&[Some(configured)], &[]).unwrap(),
        count(&[Some(plain)], &[]).unwrap() + 1
    );
    assert_eq!(count(&[Some(platform)], &[]).unwrap(), 5); //Header/identity/config/platform/slot.
}

#[test]
fn retained_backing_edges_and_slots_are_counted_without_range_multiplication() {
    let edge = PublishedEdge::Local(PublishedNodeId::new(0).unwrap());
    let range = PublishedEdgeRange { start: 0, len: 1 };
    let rows = [
        Some(PublishedNode::List(range)),
        Some(PublishedNode::Tuple(range)),
    ];
    assert_eq!(count(&rows, &[edge]).unwrap(), 5); //2headers+2slots+one shared backing.
    assert_eq!(count(&rows, &[edge, edge]).unwrap(), 6); //Unused stored edge still retained.
    let cell = [Some(PublishedNode::Cell {
        contents: Some(edge),
    })];
    assert_eq!(count(&cell, &[]).unwrap(), 3); //Header+inlineedge+slot, symbolic local cycle finite.
}
