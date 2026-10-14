use std::path::PathBuf;

use starlark::environment::FrozenModule;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::eval::FunctionExportMode;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use super::*;
use crate::provider::BzlEvaluationContext;
use crate::provider::ProviderConstructorView;
use crate::provider::ProviderSchemaRef;
use crate::provider::provider_constructor_view;

fn owner(
    manifest: &BzlLoadManifest,
    source: &str,
    bzlmod: bool,
    children: impl IntoIterator<Item = FunctionEvaluationOwner>,
) -> anyhow::Result<FunctionEvaluationOwner> {
    bzl_evaluation_owner(
        manifest,
        starlark::codemap::CodeMap::new("same.bzl".to_owned(), source.to_owned()),
        bzlmod,
        false,
        children.into_iter().map(Ok),
    )
}
fn manifest() -> BzlLoadManifest {
    let root = BzlModuleIdentity {
        label: slug_identity_v2::CanonicalLabel::parse("@@unrelated+//moved:api.bzl").unwrap(),
        workspace_path: PathBuf::from("/same/display-name.bzl"),
        repository_mapping: Arc::from([]),
    };
    BzlLoadManifest {
        root: root.clone(),
        direct_children: Arc::from([]),
        reachable: Arc::from([crate::BzlModuleSourceProvenance::new(root, [0; 32])]),
        fingerprint: [0; 32],
    }
}
fn freeze(source: &str) -> FrozenModule {
    let module = Module::new();
    let manifest = manifest();
    let owner = owner(&manifest, source, false, []).unwrap();
    crate::starlark_label::attach_defining_module_owner(&module, &manifest, owner.clone()).unwrap();
    let context = BzlEvaluationContext::from_manifest(&manifest);
    let mut evaluator = Evaluator::new(&module);
    evaluator
        .set_function_symbol_session(function_symbol_session(owner))
        .unwrap();
    evaluator
        .set_function_export_mode(FunctionExportMode::TopLevelAssignments)
        .unwrap();
    evaluator.extra = Some(&context);
    evaluator
        .eval_module(
            AstModule::parse("same.bzl", source.to_owned(), &Dialect::Bazel).unwrap(),
            &crate::package::loading_globals(),
        )
        .unwrap();
    drop(evaluator);
    module.freeze().unwrap()
}
#[test]
fn structural_bzl_owner_compares_source_bytes_mapping_and_load_mode() {
    let first = manifest();
    let a = owner(&first, "def f(): return 1", false, []).unwrap();
    let b = owner(&first, "def f(): return 2", false, []).unwrap();
    let again = owner(&first, "def f(): return 1", false, []).unwrap();
    assert_ne!(a, b); // Deliberately identical integrity digests do not alias code.
    assert_eq!(a, again);
    assert_ne!(a, owner(&first, "def f(): return 1", true, []).unwrap());
    assert_ne!(
        a,
        bzl_evaluation_owner(
            &first,
            starlark::codemap::CodeMap::new("same.bzl".into(), "def f(): return 1".into()),
            false,
            true,
            []
        )
        .unwrap()
    );
    let mut changed = first.clone();
    changed.root.repository_mapping = Arc::from([(
        slug_identity_v2::ApparentRepoName::new("alias").unwrap(),
        slug_identity_v2::CanonicalRepoName::new("other+").unwrap(),
    )]);
    changed.reachable = Arc::from([crate::BzlModuleSourceProvenance::new(
        changed.root.clone(),
        [0; 32],
    )]);
    assert_ne!(a, owner(&changed, "def f(): return 1", false, []).unwrap());
    let parent = parent_manifest(1, &first);
    let parent_a = owner(&parent, "parent", false, [a.clone()]).unwrap();
    let parent_b = owner(&parent, "parent", false, [b]).unwrap();
    assert_ne!(parent_a, parent_b);
    assert_eq!(parent_a, owner(&parent, "parent", false, [again]).unwrap());
}
/// Model genuine distinct modules while retaining the complete flat manifest.
fn parent_manifest(level: usize, child: &BzlLoadManifest) -> BzlLoadManifest {
    let mut root = manifest().root;
    root.label =
        slug_identity_v2::CanonicalLabel::parse(&format!("@@unrelated+//moved:level{level}.bzl"))
            .unwrap();
    root.workspace_path = PathBuf::from(format!("/same/level{level}.bzl"));
    let mut reachable = vec![crate::BzlModuleSourceProvenance::new(root.clone(), [0; 32])];
    reachable.extend(child.reachable.iter().cloned());
    BzlLoadManifest {
        root,
        direct_children: Arc::from([child.root.clone()]),
        reachable: reachable.into(),
        fingerprint: [0; 32],
    }
}

#[test]
fn shared_diamond_owner_flattens_comparison_hash_and_construction_work() {
    fn diamond(levels: usize, changed_leaf: bool) -> FunctionEvaluationOwner {
        let mut current_manifest = manifest();
        let mut current = owner(
            &current_manifest,
            if changed_leaf {
                "def leaf(): return 2"
            } else {
                "def leaf(): return 1"
            },
            false,
            [],
        )
        .unwrap();
        for level in 1..=levels {
            current_manifest = parent_manifest(level, &current_manifest);
            current = owner(
                &current_manifest,
                "def parent(): return 1",
                false,
                [current.clone(), current],
            )
            .unwrap();
            let validity = current
                .input_validity()
                .as_any()
                .downcast_ref::<BzlInputValidity>()
                .unwrap();
            // Two references to the same child closure visit each flat child
            // row twice, irrespective of the number of expanded load paths.
            assert_eq!(validity.rows.len(), level + 1);
            assert_eq!(validity.shape.nodes, 0);
            assert_eq!(validity.shared_inputs.len(), level + 1);
            assert_eq!(current.retained_shape().nodes, level + 1);
            assert_eq!(validity.rows[0].children.len(), 2);
            assert!(Arc::ptr_eq(
                &validity.rows[0].children[0],
                &validity.rows[0].children[1]
            ));
            assert!(current.retained_shape().visits <= 65536);
        }
        current
    }
    #[derive(Default)]
    struct CountHash {
        bytes: usize,
        writes: usize,
    }
    impl Hasher for CountHash {
        fn finish(&self) -> u64 {
            0
        }
        fn write(&mut self, bytes: &[u8]) {
            self.bytes += bytes.len();
            self.writes += 1;
        }
    }
    let first = diamond(24, false);
    let again = diamond(24, false);
    assert_eq!(first, again);
    assert_ne!(first, diamond(24, true));
    let validity = first
        .input_validity()
        .as_any()
        .downcast_ref::<BzlInputValidity>()
        .unwrap();
    let mut actual = CountHash::default();
    first.hash_fields(&mut actual);
    let mut expected = CountHash::default();
    first.domain().hash(&mut expected);
    first.stable_key().hash_part(&mut expected);
    validity.rows.len().hash(&mut expected);
    for row in &*validity.rows {
        row.hash(&mut expected);
    }
    assert_eq!(
        (actual.bytes, actual.writes),
        (expected.bytes, expected.writes)
    );
    let mut other = CountHash::default();
    again.hash_fields(&mut other);
    assert_eq!((actual.bytes, actual.writes), (other.bytes, other.writes));
    // Local rows are shared across closures, with no recursive owner retained.
    assert_eq!(validity.rows.len(), 25);
}

#[test]
fn shared_owner_closure_rejects_conflicting_complete_local_facts() {
    let child_manifest = manifest();
    let child_a = owner(&child_manifest, "def f(): return 1", false, []).unwrap();
    let child_b = owner(&child_manifest, "def f(): return 2", false, []).unwrap();
    let parent = parent_manifest(1, &child_manifest);
    assert!(
        owner(&parent, "parent", false, [child_a, child_b])
            .unwrap_err()
            .to_string()
            .contains("conflicting facts")
    );
    let child = owner(&child_manifest, "child", false, []).unwrap();
    let mut self_manifest = child_manifest.clone();
    self_manifest.direct_children = Arc::from([child_manifest.root.clone()]);
    assert!(
        owner(&self_manifest, "different root", false, [child])
            .unwrap_err()
            .to_string()
            .contains("conflicting facts")
    );
}

#[test]
fn shared_multiline_rows_charge_scalar_storage_and_compare_independent_full_sources() {
    // The scalar line table is large, but owner equality only visits the source
    // text as one fully byte-charged fact. Repeated immutable rows do no source
    // comparison at all; they must not multiply line positions into graph work.
    let text = format!("{}def leaf(): return 1\n", "# padding\n".repeat(30_000));
    let child_manifest = manifest();
    let child = owner(&child_manifest, &text, false, []).unwrap();
    let parent = parent_manifest(1, &child_manifest);
    let shared = owner(
        &parent,
        "parent",
        false,
        std::iter::repeat_n(child.clone(), 32),
    )
    .unwrap();
    let validity = shared
        .input_validity()
        .as_any()
        .downcast_ref::<BzlInputValidity>()
        .unwrap();
    assert_eq!(validity.rows.len(), 2);
    assert_eq!(validity.shape.nodes, 0);
    assert_eq!(shared.retained_shape().nodes, 2);
    assert!(validity.shape.visits < 1000, "{}", validity.shape.visits);
    let lines = text.bytes().filter(|byte| *byte == b'\n').count() + 1;
    assert!(
        validity.rows[1].shape.bytes
            >= text.len() + 2 * lines * std::mem::size_of::<starlark::codemap::Pos>()
    );
    assert!(shared.retained_shape().bytes >= validity.rows[1].shape.bytes);
    assert!(validity.shape.bytes < validity.rows[1].shape.bytes);

    let again = owner(&child_manifest, &text, false, []).unwrap();
    let independent = again
        .input_validity()
        .as_any()
        .downcast_ref::<BzlInputValidity>()
        .unwrap();
    assert!(!Arc::ptr_eq(&validity.rows[1], &independent.rows[0]));
    assert_ne!(
        validity.rows[1].source.id(),
        independent.rows[0].source.id()
    );
    assert_eq!(child, again);
    let mixed = owner(&parent, "parent", false, [child.clone(), again]).unwrap();
    assert_eq!(
        mixed
            .input_validity()
            .as_any()
            .downcast_ref::<BzlInputValidity>()
            .unwrap()
            .rows
            .len(),
        2
    );
    // Same identity/digest and the same long prefix still compare the final
    // actual source byte. Full comparison is mandatory for distinct rows.
    let changed = owner(
        &child_manifest,
        &text.replace("return 1", "return 2"),
        false,
        [],
    )
    .unwrap();
    assert_ne!(child, changed);
    assert!(
        owner(&parent, "parent", false, [child, changed])
            .unwrap_err()
            .to_string()
            .contains("conflicting facts")
    );
}

#[test]
fn shared_fact_certificates_reuse_actual_rows_and_separate_payload_from_references() {
    use slug_build_api_v2::published_value::PublishedValueArenaBuilder;
    fn validity(owner: &FunctionEvaluationOwner) -> &BzlInputValidity {
        owner.input_validity().as_any().downcast_ref().unwrap()
    }
    let child_manifest = manifest();
    let text = format!(
        "{}def leaf(): return 1\n",
        "# retained source\n".repeat(1000)
    );
    let child = owner(&child_manifest, &text, false, []).unwrap();
    let parent_manifest = parent_manifest(1, &child_manifest);
    let parent = owner(&parent_manifest, "parent", false, [child.clone()]).unwrap();
    let c = validity(&child);
    let p = validity(&parent);
    assert_eq!(p.rows.len(), 2);
    assert_eq!(p.shared_inputs.len(), p.rows.len());
    assert!(Arc::ptr_eq(&c.rows[0], &p.rows[1]));
    let before = Arc::strong_count(&p.rows[1]);
    let certificate = SharedPublicationInput::new(p.rows[1].clone());
    assert_eq!(Arc::strong_count(&p.rows[1]), before + 1);
    drop(certificate);
    assert_eq!(Arc::strong_count(&p.rows[1]), before);
    // The generic view borrows the exact existing allocation, not a cookie.
    let fact = parent.input_validity().shared_inputs()[1].retained_shape();
    assert_eq!(fact.nodes, 1);
    assert_eq!(fact.bytes, c.rows[0].shape.bytes);
    assert_eq!(fact.visits, c.rows[0].shape.visits);
    let handle_bytes =
        std::mem::size_of::<Arc<LocalBzlInput>>() + std::mem::size_of::<SharedPublicationInput>();
    assert_eq!(p.shape.nodes, 0);
    assert_eq!(p.shape.visits, 1 + p.rows.len());
    assert_eq!(p.shape.edges, 2 + 2 * p.rows.len());
    assert_eq!(
        p.shape.bytes,
        std::mem::size_of::<BzlInputValidity>()
            + 4 * std::mem::size_of::<usize>()
            + p.rows.len() * handle_bytes
    );
    let independent = owner(&child_manifest, &text, false, []).unwrap();
    assert_eq!(child, independent);
    assert!(!Arc::ptr_eq(&c.rows[0], &validity(&independent).rows[0]));
    // A prior actual child payload is counted once in either encounter order.
    for reverse in [false, true] {
        let mut audit = PublishedValueArenaBuilder::new();
        let (first, second) = if reverse {
            (&parent, &child)
        } else {
            (&child, &parent)
        };
        audit.audit_owner_shape(first).unwrap();
        audit.audit_owner_shape(second).unwrap();
        let shared = audit.local_shape();
        let mut separate = PublishedValueArenaBuilder::new();
        separate.audit_owner_shape(&parent).unwrap();
        separate.audit_owner_shape(&independent).unwrap();
        let separate = separate.local_shape();
        assert_eq!(separate.nodes - shared.nodes, fact.nodes);
        assert_eq!(separate.visits - shared.visits, fact.visits);
        assert!(separate.bytes - shared.bytes >= fact.bytes);
    }
}

#[test]
fn duplicate_child_construction_stops_at_the_shared_shape_budget() {
    let child_manifest = manifest();
    let child = owner(&child_manifest, "def child(): return 1", false, []).unwrap();
    let parent = parent_manifest(1, &child_manifest);
    let observed = std::cell::Cell::new(0usize);
    let children = std::iter::repeat_with(|| {
        observed.set(observed.get() + 1);
        child.clone()
    })
    .take(100_000);
    assert!(
        owner(&parent, "parent", false, children)
            .unwrap_err()
            .to_string()
            .contains("shape limit")
    );
    // This is a lazy input: the bounded forecast refuses before materializing
    // all references, rather than checking a fully allocated closure afterward.
    assert!(observed.get() < 65536);
}

#[test]
fn actual_export_symbols_preserve_first_alias_and_refuse_transient_publication() {
    let frozen = freeze("def actual(): return 7\nalias = actual\n");
    let actual = snapshot_function_symbol(frozen.get("actual").unwrap().value())
        .unwrap()
        .unwrap();
    let alias = snapshot_function_symbol(frozen.get("alias").unwrap().value())
        .unwrap()
        .unwrap();
    assert_eq!(actual, alias);
    assert_eq!(
        actual.name(),
        &FunctionSymbolName::Exported(Arc::from("actual"))
    );
    let module = Module::new();
    Evaluator::new(&module)
        .eval_module(
            AstModule::parse(
                "same.bzl",
                "def f(): return 7\n".to_owned(),
                &Dialect::Bazel,
            )
            .unwrap(),
            &crate::package::loading_globals(),
        )
        .unwrap();
    assert!(
        snapshot_function_symbol(module.get("f").unwrap())
            .unwrap_err()
            .to_string()
            .contains("structural evaluation owner")
    );
}
#[test]
fn genuine_provider_constructor_views_preserve_schema_init_and_raw_occurrence() {
    let frozen = freeze(
        "def init(value = 2): return {'value': value + 10}\nP, raw = provider(doc = 'arbitrary', fields = ['value'], init = init)\nPlain = provider()\nSchema = provider(fields = ['z', 'a'])\nDocs = provider(fields = {'b': 'B', 'a': 'A'})\ninitialized = P(5)\nbypassed = raw(value = 5)\n",
    );
    let p_owner = frozen.get("P").unwrap();
    let p = p_owner.value();
    let raw_owner = frozen.get("raw").unwrap();
    let raw = raw_owner.value();
    let ProviderConstructorView::Constructor {
        id,
        schema: ProviderSchemaRef::Initialized(fields),
        init: Some(init),
    } = provider_constructor_view(p).unwrap()
    else {
        panic!("initialized constructor")
    };
    assert!(id.unwrap().to_string().contains("P"));
    assert_eq!(fields, &[compact_str::CompactString::new("value")]);
    assert!(starlark::eval::inspect_function_symbol(init).is_some());
    let ProviderConstructorView::Raw {
        provider,
        occurrence,
    } = provider_constructor_view(raw).unwrap()
    else {
        panic!("raw")
    };
    assert!(provider.equals(p).unwrap());
    let ProviderConstructorView::Raw {
        occurrence: again, ..
    } = provider_constructor_view(raw).unwrap()
    else {
        unreachable!()
    };
    assert_eq!(occurrence, again);
    assert!(matches!(
        provider_constructor_view(frozen.get("Plain").unwrap().value()),
        Some(ProviderConstructorView::Constructor {
            schema: ProviderSchemaRef::Schemaless,
            init: None,
            ..
        })
    ));
    assert!(matches!(
        provider_constructor_view(frozen.get("Schema").unwrap().value()),
        Some(ProviderConstructorView::Constructor {
            schema: ProviderSchemaRef::List(_),
            ..
        })
    ));
    assert!(matches!(
        provider_constructor_view(frozen.get("Docs").unwrap().value()),
        Some(ProviderConstructorView::Constructor {
            schema: ProviderSchemaRef::Documented(_),
            ..
        })
    ));
    assert_eq!(
        frozen
            .get("initialized")
            .unwrap()
            .value()
            .get_attr("value", Module::new().heap())
            .unwrap()
            .unwrap()
            .unpack_i32(),
        Some(15)
    );
    assert_eq!(
        frozen
            .get("bypassed")
            .unwrap()
            .value()
            .get_attr("value", Module::new().heap())
            .unwrap()
            .unwrap()
            .unpack_i32(),
        Some(5)
    );
}

#[test]
fn actual_mutable_provider_and_raw_views_transfer_occurrences_through_freeze() {
    let source = "def init(value = 3): return {'value': value}\nP, raw = provider(doc = 'ordinary', fields = ['value'], init = init)\nalias = raw\n";
    let manifest = manifest();
    let module = Module::new();
    crate::starlark_label::attach_defining_module_manifest(&module, &manifest).unwrap();
    let context = BzlEvaluationContext::from_manifest(&manifest);
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(&context);
    eval.eval_module(
        AstModule::parse("arbitrary.bzl", source.to_owned(), &Dialect::Bazel).unwrap(),
        &crate::package::loading_globals(),
    )
    .unwrap();
    let Some(ProviderConstructorView::Constructor {
        id: Some(_),
        init: Some(_),
        schema: ProviderSchemaRef::Initialized(_),
    }) = provider_constructor_view(module.get("P").unwrap())
    else {
        panic!("actual mutable initialized provider")
    };
    let Some(ProviderConstructorView::Raw { occurrence, .. }) =
        provider_constructor_view(module.get("raw").unwrap())
    else {
        panic!("actual mutable raw")
    };
    let occurrence = occurrence.clone();
    drop(eval);
    let frozen = module.freeze().unwrap();
    let raw_owner = frozen.get("raw").unwrap();
    let Some(ProviderConstructorView::Raw {
        occurrence: frozen_occurrence,
        ..
    }) = provider_constructor_view(raw_owner.value())
    else {
        panic!("actual frozen raw")
    };
    assert_eq!(&occurrence, frozen_occurrence);
    let alias_owner = frozen.get("alias").unwrap();
    let Some(ProviderConstructorView::Raw {
        occurrence: alias_occurrence,
        ..
    }) = provider_constructor_view(alias_owner.value())
    else {
        panic!("actual frozen alias")
    };
    assert_eq!(frozen_occurrence, alias_occurrence);
}

#[path = "tests/manifest.rs"]
mod canonical_manifest;

#[test]
fn flat_row_indexes_refuse_tree_allocation_before_new_or_duplicate_occurrence_growth() {
    let metadata = manifest();
    let first = owner(&metadata, "same source", false, []).unwrap();
    let independent = owner(&metadata, "same source", false, []).unwrap();
    let row = first
        .input_validity()
        .as_any()
        .downcast_ref::<BzlInputValidity>()
        .unwrap()
        .rows[0]
        .clone();
    let candidate = independent
        .input_validity()
        .as_any()
        .downcast_ref::<BzlInputValidity>()
        .unwrap()
        .rows[0]
        .clone();
    assert!(!Arc::ptr_eq(&row, &candidate));
    let mut rows = Vec::new();
    let mut index = BTreeMap::new();
    let mut occurrences = BTreeSet::new();
    let mut retained = zero_shape();
    let mut intrinsic = zero_shape();
    let handles =
        std::mem::size_of::<Arc<LocalBzlInput>>() + std::mem::size_of::<SharedPublicationInput>();
    let before_index = std::mem::size_of::<u64>() + 5 * handles;
    let mut work = PublicationShape {
        bytes: 64 * 1024 * 1024 - before_index - 1023,
        ..zero_shape()
    };
    assert!(
        insert_local_row(
            &row,
            &mut rows,
            &mut index,
            &mut occurrences,
            &mut retained,
            &mut intrinsic,
            &mut work
        )
        .is_err()
    );
    assert!(rows.is_empty());
    assert!(index.is_empty());
    assert!(occurrences.is_empty());

    let mut hasher = DefaultHasher::new();
    row.key.hash(&mut hasher);
    let key = hasher.finish();
    index.insert(key, vec![0]);
    occurrences.insert(Arc::as_ptr(&row) as usize);
    rows.push(row.clone());
    // The equal-but-distinct row takes the duplicate-key path, which must also
    // forecast a new occurrence tree node before inserting its allocation id.
    let before_occurrence = std::mem::size_of::<u64>()
        + 2 * row.key.shape.bytes
        + row.shape.bytes
        + candidate.shape.bytes;
    let mut work = PublicationShape {
        bytes: 64 * 1024 * 1024 - before_occurrence - 1023,
        ..zero_shape()
    };
    assert!(
        insert_local_row(
            &candidate,
            &mut rows,
            &mut index,
            &mut occurrences,
            &mut retained,
            &mut intrinsic,
            &mut work
        )
        .is_err()
    );
    assert_eq!(rows.len(), 1);
    assert_eq!(index.len(), 1);
    assert_eq!(index[&key], [0]);
    assert_eq!(occurrences.len(), 1);
    assert!(!occurrences.contains(&(Arc::as_ptr(&candidate) as usize)));
}
