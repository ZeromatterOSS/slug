use super::*;

#[derive(Debug)]
struct Originals {
    roots: usize,
    imports: Vec<PublishedValueArena>,
}
impl PublishedOriginalBinding for Originals {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn root_count(&self) -> usize {
        self.roots
    }
    fn imported_arenas(&self) -> &[PublishedValueArena] {
        &self.imports
    }
}
fn retained() -> crate::AnalysisDepset {
    crate::AnalysisDepset::new(
        crate::DepsetOrder::Default,
        vec![AnalysisValue::string("retained leaf")],
        vec![],
    )
    .unwrap()
}
fn row(b: &mut PublishedValueArenaBuilder, row: PublishedNode) -> PublishedNodeId {
    let id = b.reserve().unwrap();
    b.set_node(id, row).unwrap();
    b.set_original_slot(id, PublishedRootSlot::new(id.index()).unwrap())
        .unwrap();
    id
}
fn wrap(
    b: &mut PublishedValueArenaBuilder,
    mut edge: PublishedEdge,
    levels: usize,
) -> PublishedNodeId {
    let mut root = None;
    for _ in 0..levels {
        let range = b.append_edges(&[edge]).unwrap();
        let id = row(b, PublishedNode::List(range));
        root = Some(id);
        edge = PublishedEdge::Local(id);
    }
    root.unwrap()
}
fn seal(b: PublishedValueArenaBuilder) -> Result<PublishedValueArena, PublishedValueError> {
    let originals = Originals {
        roots: b.rows.len(),
        imports: b.imports.clone(),
    };
    b.seal(Arc::new(originals))
}

#[test]
fn strict_native_depth_certificate_is_relative_and_restores_prior_history() {
    for borrowed in [false, true] {
        let mut b = PublishedValueArenaBuilder::new();
        b.observe_depth(200).unwrap();
        b.owner_audit = true;
        let scalar = AnalysisValue::string("zero-depth scalar");
        let atom = if borrowed {
            b.native_atom_borrowed(&scalar)
        } else {
            b.native_atom(scalar)
        }
        .unwrap();
        assert_eq!(atom.ordinary_depth(), 0);
        let files = crate::FilesToRunProvider::empty();
        let atom = if borrowed {
            b.files_to_run_atom_borrowed(&files)
        } else {
            b.files_to_run_atom(files)
        }
        .unwrap();
        assert_eq!(atom.ordinary_depth(), 1);
        let value = crate::ProviderValue::FilesToRunProvider(crate::FilesToRunProvider::empty());
        let atom = if borrowed {
            b.provider_value_atom_borrowed(&value)
        } else {
            b.provider_value_atom(value)
        }
        .unwrap();
        assert_eq!(atom.ordinary_depth(), 2);
        let atom = b.depset_atom_borrowed(&retained()).unwrap();
        assert_eq!(atom.ordinary_depth(), 2);
        assert_eq!(b.maximum_depth, 200);
        assert!(b.owner_audit);
        assert!(b.imports.is_empty());

        let invalid = crate::ProviderValue::Occurrence(crate::ProviderOccurrence::empty(
            crate::ProviderIdentity::builtin("Unsupported"),
        ));
        assert!(b.provider_value_atom_borrowed(&invalid).is_err());
        assert_eq!(b.maximum_depth, 200);
        assert!(b.owner_audit);
        assert!(b.charge_visit().is_err());
    }
    let mut b = PublishedValueArenaBuilder::new();
    b.observe_depth(1).unwrap();
    b.owner_audit = true;
    let unsupported_leaf = crate::AnalysisDepset::new(
        crate::DepsetOrder::Default,
        vec![AnalysisValue::tuple(vec![AnalysisValue::string(
            "tuple child",
        )])],
        vec![],
    )
    .unwrap();
    assert!(b.depset_atom_borrowed(&unsupported_leaf).is_err());
    assert_eq!(
        b.maximum_depth, 2,
        "observed error depth is merged with prior depth"
    );
    assert!(b.owner_audit);
    assert!(b.charge_visit().is_err());
}

#[test]
fn sealed_native_depth_composes_completed_payloads_with_enclosing_lists() {
    for (levels, allowed) in [(DEPTH_LIMIT - 2, true), (DEPTH_LIMIT - 1, false)] {
        let mut b = PublishedValueArenaBuilder::new();
        let atom = b.depset_atom_borrowed(&retained()).unwrap();
        let id = row(&mut b, PublishedNode::Native(atom));
        wrap(&mut b, PublishedEdge::Local(id), levels);
        let result = seal(b);
        if allowed {
            assert_eq!(result.unwrap().shape().depth, DEPTH_LIMIT);
        } else {
            assert!(matches!(
                result,
                Err(PublishedValueError::Limit("projection depth"))
            ));
        }
    }
    // A zero-height strict scalar remains usable at the complete ordinary cap.
    let mut b = PublishedValueArenaBuilder::new();
    let atom = b.native_atom(AnalysisValue::none()).unwrap();
    let id = row(&mut b, PublishedNode::Native(atom));
    wrap(&mut b, PublishedEdge::Local(id), DEPTH_LIMIT);
    assert_eq!(seal(b).unwrap().shape().depth, DEPTH_LIMIT);
}

#[test]
fn hidden_files_and_target_native_certificates_include_the_inline_layer() {
    for target in [false, true] {
        for extra in [0, 1] {
            let mut b = PublishedValueArenaBuilder::new();
            let (node, base) = if target {
                let atom = b.native_atom(AnalysisValue::none()).unwrap();
                let view = row(&mut b, PublishedNode::Native(atom));
                let provider = b
                    .provider_value_atom(crate::ProviderValue::FilesToRunProvider(
                        crate::FilesToRunProvider::empty(),
                    ))
                    .unwrap();
                let base = provider.ordinary_depth() + 1;
                (
                    PublishedNode::ConfiguredTarget {
                        identity: crate::AnalysisTargetIdentity::null(
                            slug_identity_v2::CanonicalLabel::parse("@@//pkg:target").unwrap(),
                        ),
                        providers: Arc::from([PublishedTargetProvider::Native {
                            provider,
                            view: PublishedEdge::Local(view),
                        }]),
                    },
                    base,
                )
            } else {
                let files = b
                    .files_to_run_atom(crate::FilesToRunProvider::empty())
                    .unwrap();
                let base = files.ordinary_depth() + 1;
                (
                    PublishedNode::Provider(PublishedProvider {
                        identity: crate::ProviderIdentity::builtin("DefaultInfo"),
                        has_schema: true,
                        names: Arc::from([]),
                        values: b.append_edges(&[]).unwrap(),
                        files_to_run: Some(files),
                    }),
                    base,
                )
            };
            let id = row(&mut b, node);
            wrap(&mut b, PublishedEdge::Local(id), DEPTH_LIMIT - base + extra);
            let result = seal(b);
            if extra == 0 {
                assert_eq!(result.unwrap().shape().depth, DEPTH_LIMIT);
            } else {
                assert!(matches!(
                    result,
                    Err(PublishedValueError::Limit("projection depth"))
                ));
            }
        }
    }
}

#[test]
fn imported_native_payload_height_survives_zero_weight_typed_edges() {
    let mut prior = PublishedValueArenaBuilder::new();
    let atom = prior.depset_atom_borrowed(&retained()).unwrap();
    let id = row(&mut prior, PublishedNode::Native(atom));
    let prior = seal(prior).unwrap();
    for extra in [0, 1] {
        let mut b = PublishedValueArenaBuilder::new();
        let imported = b.import(prior.clone()).unwrap();
        let dag = PublishedNode::Depset(PublishedDepset {
            order: crate::DepsetOrder::Default,
            element_type: crate::AnalysisValueType::String,
            depth: 2,
            occurrence: crate::AnalysisDepsetOccurrence::new(),
            successors: Arc::from([PublishedDepsetInput::Transitive(PublishedEdge::Imported {
                arena: imported,
                node: id,
            })]),
        });
        let root = row(&mut b, dag);
        wrap(
            &mut b,
            PublishedEdge::Local(root),
            DEPTH_LIMIT - prior.shape().depth + extra,
        );
        let result = seal(b);
        if extra == 0 {
            assert_eq!(result.unwrap().shape().depth, DEPTH_LIMIT);
        } else {
            assert!(matches!(
                result,
                Err(PublishedValueError::Limit("projection depth"))
            ));
        }
    }
}
