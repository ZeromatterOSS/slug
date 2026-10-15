//! Original bundled evaluation and explicit provenance, without caller recipes.
use dice::DetectCycles;
use dice::Dice;
use starlark::values::ValueLike;

use super::*;
use crate::bzl_environment::catalog::BuiltinSourceCatalog;

fn raw_manifest() -> BzlLoadManifest {
    let root = BzlModuleIdentity {
        label: CanonicalLabel::parse("@@//pkg:consumer.bzl").unwrap(),
        source: slug_build_api_v2::ModuleSourceOrigin::Workspace(
            "/workspace/pkg/consumer.bzl".into(),
        ),
        repository_mapping: Arc::from([]),
    };
    BzlLoadManifest {
        root: root.clone(),
        direct_children: Arc::from([]),
        reachable: Arc::from([BzlModuleSourceProvenance::new(root, [7; 32])]),
        fingerprint: [8; 32],
    }
}
pub(super) fn changed_catalog_at(
    value: &BuiltinSourceCatalog,
    index: usize,
) -> Arc<BuiltinSourceCatalog> {
    let mut sources = value
        .entries
        .iter()
        .map(|e| e.source.to_vec())
        .collect::<Vec<_>>();
    sources[index].extend_from_slice(b"\n# complete source input changed\n");
    let hashes = sources
        .iter()
        .map(|s| {
            use sha2::Digest;
            use sha2::Sha256;
            Sha256::digest(s)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        })
        .collect::<Vec<_>>();
    let entries = value
        .entries
        .iter()
        .zip(&sources)
        .zip(&hashes)
        .map(|((e, s), h)| (e.path.as_str(), s.as_slice(), h.as_str()))
        .collect::<Vec<_>>();
    catalog::prepare(&value.tag, &entries).unwrap()
}
#[tokio::test]
async fn original_builtin_exports_keep_constructor_aliases_and_complete_environment_source_aba() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut tx = dice.updater().commit().await;
    let mapping = BuiltinRepositoryMapping::new(Arc::from([])).unwrap();
    let first = prepare_build_environment(
        &mut tx,
        &BuiltinSourceCatalogKey::Bundled(crate::bzl_environment::BuiltinCatalogSnapshot::Bazel9_2),
        &mapping,
    )
    .await
    .unwrap();
    let actual = first.exports[0].module.semantic_input_facts().unwrap();
    let provider_module = &actual
        .children
        .iter()
        .find(|c| c.raw.as_ref() == ":common/xcode/providers.bzl")
        .unwrap()
        .module;
    let direct = provider_module.module.get("XcodeVersionInfo").unwrap();
    let consumer = starlark::environment::Module::new();
    let facade = first.exports[0]
        .value
        .value()
        .get_attr("XcodeVersionConfig", consumer.heap())
        .unwrap()
        .unwrap();
    assert!(facade.ptr_eq(direct.value()));
    let globals = first.globals(&raw_manifest().root.label);
    assert!(
        globals
            .iter()
            .find(|(name, _)| *name == "apple_common")
            .unwrap()
            .1
            .to_value()
            .ptr_eq(first.exports[0].value.value())
    );
    let context = BzlModuleSourceContext::new(&raw_manifest(), &first, std::iter::empty()).unwrap();
    assert!(context.raw_load_manifest().direct_children.is_empty());
    assert_eq!(context.environment_bindings().len(), 1);
    assert_eq!(context.environment_bindings()[0].global, "apple_common");
    assert_eq!(
        context.environment_bindings()[0].export,
        "apple_common_bazel"
    );
    assert!(context.effective_reachable().len() > context.raw_load_manifest().reachable.len());
    assert!(
        context
            .effective_reachable()
            .iter()
            .any(|s| s.identity.source.builtin_catalog().is_some())
    );
    let catalog = first.inputs.catalog.as_ref().unwrap();
    let changed = prepare_build_environment(
        &mut tx,
        &BuiltinSourceCatalogKey::Snapshot(changed_catalog_at(catalog, 0)),
        &mapping,
    )
    .await
    .unwrap();
    assert_ne!(first.inputs, changed.inputs);
    assert_ne!(
        context.fingerprint(),
        BzlModuleSourceContext::new(&raw_manifest(), &changed, std::iter::empty())
            .unwrap()
            .fingerprint()
    );
    let restored = prepare_build_environment(
        &mut tx,
        &BuiltinSourceCatalogKey::Bundled(crate::bzl_environment::BuiltinCatalogSnapshot::Bazel9_2),
        &mapping,
    )
    .await
    .unwrap();
    assert_eq!(first.inputs, restored.inputs);
    assert!(
        first.exports[0]
            .value
            .value()
            .ptr_eq(restored.exports[0].value.value())
    );
    assert_eq!(
        context,
        BzlModuleSourceContext::new(&raw_manifest(), &restored, std::iter::empty()).unwrap()
    );
}
#[tokio::test]
async fn builtin_route_refuses_outside_catalog_and_conflicting_effective_provenance() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut tx = dice.updater().commit().await;
    let mapping = BuiltinRepositoryMapping::new(Arc::from([])).unwrap();
    let env = prepare_build_environment(
        &mut tx,
        &BuiltinSourceCatalogKey::Bundled(crate::bzl_environment::BuiltinCatalogSnapshot::Bazel9_2),
        &mapping,
    )
    .await
    .unwrap();
    let mut raw = raw_manifest();
    let conflicting = env.exports[0].module.manifest.reachable[0].clone();
    raw.reachable = Arc::from([
        raw.reachable[0].clone(),
        BzlModuleSourceProvenance::new(conflicting.identity, [0; 32]),
    ]);
    assert!(
        BzlModuleSourceContext::new(&raw, &env, std::iter::empty())
            .unwrap_err()
            .contains("conflicting")
    );
    let catalog = env.inputs.catalog.as_ref().unwrap();
    let mut work = Budget::default();
    for source in [
        "@other//:defs.bzl",
        "//:defs.bzl",
        ":../defs.bzl",
        ":absent.bzl",
    ] {
        assert!(load::resolve(catalog, source, &mut work).is_err());
    }
    assert!(PreparedBzlEnvironment::uninjected(BzlEnvironmentMode::Build).is_err());
    assert!(PreparedBzlEnvironment::uninjected(BzlEnvironmentMode::Bzlmod).is_err());
    let bootstrap =
        PreparedBzlEnvironment::uninjected(BzlEnvironmentMode::BzlmodBootstrap).unwrap();
    assert_eq!(bootstrap.mode(), BzlEnvironmentMode::BzlmodBootstrap);
    assert!(bootstrap.exports.is_empty());
    assert!(
        bootstrap
            .globals(&raw.root.label)
            .iter()
            .all(|(name, _)| name != "apple_common")
    );
    assert!(
        prepare_source_environment(
            &mut tx,
            &BuiltinSourceCatalogKey::Bundled(BuiltinCatalogSnapshot::Bazel9_2),
            &BuiltinRepositoryMapping::new(Arc::from([])).unwrap(),
            BzlEnvironmentMode::BzlmodBootstrap
        )
        .await
        .is_err()
    );
}

#[derive(Default)]
struct BuiltinEvents(
    std::sync::Mutex<Vec<(dice::ActivationKind, Option<slug_events_v2::EventBatch>)>>,
);
impl dice::ActivationTracker for BuiltinEvents {
    fn key_activated(
        &self,
        _: &dice::DynKey,
        _: &mut dyn Iterator<Item = &dice::DynKey>,
        _: dice::ActivationData,
    ) {
    }
    fn tracks_rich_activations(&self) -> bool {
        true
    }
    fn key_activated_rich(&self, key: &dice::DynKey, event: dice::RichActivation<'_>) {
        if key
            .downcast_ref::<crate::bzl_module::semantic_evaluation::SemanticBzlEvaluationKey>()
            .is_some()
        {
            self.0.lock().unwrap().push((
                event.kind(),
                event
                    .evaluation_data()
                    .and_then(|e| e.downcast_ref::<slug_events_v2::EventBatch>())
                    .cloned(),
            ));
        }
    }
}
#[tokio::test]
async fn builtin_events_belong_to_actual_semantic_computation_without_capture_transition_replay() {
    for capture in [false, true] {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(BuiltinEvents::default());
        let data = |capture| {
            let mut d = dice::UserComputationData {
                activation_tracker: Some(tracker.clone() as Arc<dyn dice::ActivationTracker>),
                ..Default::default()
            };
            if capture {
                d.data.set(slug_events_v2::CaptureEvaluationEvents);
            }
            d
        };
        let mut first = dice.updater_with_data(data(capture)).commit().await;
        let catalog = first
            .compute(&BuiltinSourceCatalogKey::Bundled(
                crate::bzl_environment::BuiltinCatalogSnapshot::Bazel9_2,
            ))
            .await
            .unwrap();
        let catalog = catalog.as_ref().as_ref().unwrap();
        let mut sources = catalog
            .entries
            .iter()
            .map(|e| e.source.to_vec())
            .collect::<Vec<_>>();
        sources[0].extend_from_slice(b"\nprint('actual builtin computation')\n");
        use sha2::Digest;
        use sha2::Sha256;
        let hashes = sources
            .iter()
            .map(|s| {
                Sha256::digest(s)
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>()
            })
            .collect::<Vec<_>>();
        let entries = catalog
            .entries
            .iter()
            .zip(&sources)
            .zip(&hashes)
            .map(|((e, s), h)| (e.path.as_str(), s.as_slice(), h.as_str()))
            .collect::<Vec<_>>();
        let key =
            BuiltinSourceCatalogKey::Snapshot(catalog::prepare(&catalog.tag, &entries).unwrap());
        let mapping = BuiltinRepositoryMapping::new(Arc::from([])).unwrap();
        let env = prepare_build_environment(&mut first, &key, &mapping)
            .await
            .unwrap();
        let initial = std::mem::take(&mut *tracker.0.lock().unwrap());
        assert_eq!(initial.len(), 6);
        assert!(
            initial
                .iter()
                .all(|(kind, _)| *kind == dice::ActivationKind::Evaluated)
        );
        let count = initial
            .iter()
            .filter_map(|(_, batch)| batch.as_ref())
            .map(|b| b.events().len())
            .sum::<usize>();
        assert_eq!(count, usize::from(capture));
        assert!(initial.iter().all(|(_, batch)| batch.is_some() == capture));
        let mut second = dice.updater_with_data(data(!capture)).commit().await;
        let reused = prepare_build_environment(&mut second, &key, &mapping)
            .await
            .unwrap();
        assert!(
            env.exports[0]
                .value
                .value()
                .ptr_eq(reused.exports[0].value.value())
        );
        let rows = std::mem::take(&mut *tracker.0.lock().unwrap());
        assert_eq!(rows.len(), 6);
        assert!(
            rows.iter()
                .all(|(kind, batch)| *kind == dice::ActivationKind::Reused && batch.is_none())
        );
    }
}

// The consumer is actually evaluated with the selected original environment;
// parent source closure must retain that child's effective (not only raw) facts.
async fn consumer(
    tx: &mut dice::DiceTransaction,
    env: &PreparedBzlEnvironment,
) -> crate::bzl_module::FrozenBzlModule {
    let raw = raw_manifest();
    let key = crate::bzl_module::semantic_evaluation::prepare(
        &raw.root,
        "/workspace/pkg/consumer.bzl",
        &Arc::from(b"VALUE = apple_common\n".as_slice()),
        crate::bzl_module::BzlModuleContext::Build,
        env,
        std::iter::empty(),
    )
    .unwrap();
    tx.compute(&key).await.unwrap().result().unwrap().clone()
}
#[tokio::test]
async fn actual_mapping_transitive_environment_aba_and_child_effective_provenance_are_complete() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut tx = dice.updater().commit().await;
    let key = BuiltinSourceCatalogKey::Bundled(BuiltinCatalogSnapshot::Bazel9_2);
    let mapping = |selected: &str| {
        BuiltinRepositoryMapping::new(Arc::from([(
            slug_identity_v2::ApparentRepoName::new("selected").unwrap(),
            slug_identity_v2::CanonicalRepoName::new(selected).unwrap(),
        )]))
        .unwrap()
    };
    let a = prepare_build_environment(&mut tx, &key, &mapping("first+"))
        .await
        .unwrap();
    let original = consumer(&mut tx, &a).await;
    let owner = |m: &crate::bzl_module::FrozenBzlModule| {
        m.module
            .client_data()
            .and_then(|d| crate::starlark_label::defining_module_function_owner(d.to_value()))
            .unwrap()
            .clone()
    };

    let changed_root_env = prepare_build_environment(
        &mut tx,
        &BuiltinSourceCatalogKey::Snapshot(changed_catalog_at(
            a.inputs.catalog.as_ref().unwrap(),
            0,
        )),
        &mapping("first+"),
    )
    .await
    .unwrap();
    let root_changed = consumer(&mut tx, &changed_root_env).await;

    let changed_mapping = prepare_build_environment(&mut tx, &key, &mapping("second+"))
        .await
        .unwrap();
    let mapped = consumer(&mut tx, &changed_mapping).await;
    assert_ne!(
        original.semantic_input_facts(),
        mapped.semantic_input_facts()
    );
    let transitive = prepare_build_environment(
        &mut tx,
        &BuiltinSourceCatalogKey::Snapshot(changed_catalog_at(
            a.inputs.catalog.as_ref().unwrap(),
            5,
        )),
        &mapping("first+"),
    )
    .await
    .unwrap();
    let changed = consumer(&mut tx, &transitive).await;
    assert_ne!(
        original.semantic_input_facts(),
        changed.semantic_input_facts()
    );
    let restored_env = prepare_build_environment(&mut tx, &key, &mapping("first+"))
        .await
        .unwrap();
    let restored = consumer(&mut tx, &restored_env).await;
    for alternative in [&mapped, &changed, &root_changed] {
        assert_ne!(owner(&original), owner(alternative));
        assert!(
            !original
                .module
                .get("VALUE")
                .unwrap()
                .value()
                .ptr_eq(alternative.module.get("VALUE").unwrap().value())
        );
    }
    assert_eq!(owner(&original), owner(&restored));

    assert_eq!(
        original.semantic_input_facts(),
        restored.semantic_input_facts()
    );
    assert!(
        original
            .module
            .get("VALUE")
            .unwrap()
            .value()
            .ptr_eq(restored.module.get("VALUE").unwrap().value())
    );
    let context = |module: &crate::bzl_module::FrozenBzlModule| {
        module
            .module
            .client_data()
            .and_then(|d| crate::starlark_label::defining_module_source_context(d.to_value()))
            .unwrap()
            .clone()
    };
    let child = context(&original);
    let mut parent = raw_manifest();
    parent.root.label = CanonicalLabel::parse("@@//pkg:parent.bzl").unwrap();
    parent.root.source =
        slug_build_api_v2::ModuleSourceOrigin::Workspace("/workspace/pkg/parent.bzl".into());
    parent.direct_children = Arc::from([child.raw_load_manifest().root.clone()]);
    parent.reachable = Arc::from([
        BzlModuleSourceProvenance::new(parent.root.clone(), [9; 32]),
        child.raw_load_manifest().reachable[0].clone(),
    ]);
    let empty = PreparedBzlEnvironment::uninjected(BzlEnvironmentMode::BzlmodBootstrap).unwrap();
    let actual = BzlModuleSourceContext::new(&parent, &empty, std::iter::once(&child)).unwrap();
    assert!(actual.environment_bindings().is_empty());
    assert_eq!(actual.raw_load_manifest(), &parent);
    assert!(
        child
            .effective_reachable()
            .iter()
            .all(|p| actual.effective_reachable().contains(p))
    );
    let altered = context(&changed);
    assert!(
        BzlModuleSourceContext::new(&parent, &empty, [&child, &altered].into_iter())
            .unwrap_err()
            .contains("conflicting")
    );
    // A second genuine export is extracted by the SAME private producer path.
    let seed = PreparedBzlEnvironment::seed(BzlEnvironmentMode::Builtin, a.inputs.catalog.as_ref())
        .unwrap();
    let selected = load::select(
        &seed,
        &a.exports[0].module,
        "apple_common",
        "native_apple_common",
        BzlEnvironmentMode::Build,
        Budget::default(),
    )
    .unwrap();
    let selection_original = consumer(&mut tx, &a).await;
    let selection_changed = consumer(&mut tx, &selected).await;
    let selection_restored = consumer(&mut tx, &a).await;
    assert_ne!(owner(&selection_original), owner(&selection_changed));
    assert_eq!(owner(&selection_original), owner(&selection_restored));
    assert!(
        !selection_original
            .module
            .get("VALUE")
            .unwrap()
            .value()
            .ptr_eq(selection_changed.module.get("VALUE").unwrap().value())
    );
    assert!(
        selection_original
            .module
            .get("VALUE")
            .unwrap()
            .value()
            .ptr_eq(selection_restored.module.get("VALUE").unwrap().value())
    );
    let source: Arc<[u8]> = Arc::from(b"VALUE = struct()\n".as_slice());
    let prepare = |env: &PreparedBzlEnvironment| {
        crate::bzl_module::semantic_evaluation::prepare(
            &parent.root,
            "/workspace/pkg/parent.bzl",
            &source,
            crate::bzl_module::BzlModuleContext::Bzlmod,
            env,
            std::iter::empty(),
        )
        .unwrap()
    };
    let bzlmod = prepare_source_environment(
        &mut tx,
        &key,
        &mapping("first+"),
        BzlEnvironmentMode::Bzlmod,
    )
    .await
    .unwrap();
    let bootstrap =
        PreparedBzlEnvironment::uninjected(BzlEnvironmentMode::BzlmodBootstrap).unwrap();
    let mode_a = prepare(&bzlmod);
    let mode_b = prepare(&bootstrap);
    assert_ne!(mode_a, mode_b);
    let mode_original = tx.compute(&mode_a).await.unwrap();
    let mode_changed = tx.compute(&mode_b).await.unwrap();
    let mode_restored = tx.compute(&prepare(&bzlmod)).await.unwrap();
    assert_ne!(
        owner(mode_original.result().unwrap()),
        owner(mode_changed.result().unwrap())
    );
    assert_eq!(
        owner(mode_original.result().unwrap()),
        owner(mode_restored.result().unwrap())
    );

    let value = |v: &crate::bzl_module::semantic_evaluation::SemanticBzlEvaluationResult| {
        v.result().unwrap().module.get("VALUE").unwrap()
    };
    assert!(
        !value(&mode_original)
            .value()
            .ptr_eq(value(&mode_changed).value())
    );
    assert!(
        value(&mode_original)
            .value()
            .ptr_eq(value(&mode_restored).value())
    );
    // Globals own the original heap guard after every producer module handle is
    // dropped. No reconstructed provider/native substitute participates.
    let globals = a.globals(&parent.root.label);
    let facade = globals
        .iter()
        .find(|(name, _)| *name == "apple_common")
        .unwrap()
        .1;
    drop(selection_original);
    drop(selection_changed);
    drop(selection_restored);
    drop(changed_root_env);
    drop(root_changed);
    drop(selected);
    drop(original);
    drop(mapped);
    drop(changed);
    drop(restored);
    drop(a);
    drop(changed_mapping);
    drop(transitive);
    drop(restored_env);
    drop(tx);
    drop(dice);
    let module = starlark::environment::Module::new();
    assert!(
        facade
            .to_value()
            .get_attr("XcodeVersionConfig", module.heap())
            .unwrap()
            .is_some()
    );
}
#[test]
fn catalog_and_source_context_refuse_before_source_or_mapping_guards_are_cloned() {
    let retained = Budget {
        bytes: crate::bzl_module::semantic_evaluation::budget::BYTE_LIMIT,
        ..Budget::default()
    };
    assert!(catalog_parse::admit(retained, 1, 1).is_err());
    assert!(catalog_parse::admit(Budget::default(), 1, usize::MAX).is_err());
    // Initial parser admission fits exactly; newly forecast load output must
    // still be jointly admitted while that same AST remains live.
    let text = r#"load(":dep.bzl", "value")
"#;
    let ast = starlark::syntax::AstModule::parse(
        "test.bzl",
        text.into(),
        &starlark::syntax::Dialect::Bazel,
    )
    .unwrap();
    let mut low = 0usize;
    let mut high = crate::bzl_module::semantic_evaluation::budget::BYTE_LIMIT;
    while low < high {
        let mid = low + (high - low).div_ceil(2);
        if catalog_parse::admit(
            Budget {
                bytes: mid,
                ..Budget::default()
            },
            8,
            text.len(),
        )
        .is_ok()
        {
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    let mut exact = Budget {
        bytes: low,
        ..Budget::default()
    };
    catalog_parse::admit(exact, 8, text.len()).unwrap();
    assert!(catalog_parse::loads(&ast, &mut exact, Budget::default(), 8, text.len()).is_err());

    let mut raw = raw_manifest();
    let mapping: Arc<[_]> = Arc::from([(
        slug_identity_v2::ApparentRepoName::new("alias").unwrap(),
        slug_identity_v2::CanonicalRepoName::new("selected+").unwrap(),
    )]);
    raw.root.repository_mapping = mapping.clone();
    // Borrowed giant incoming row array is refused before retaining its mapping
    // guards; the array itself belongs to this fixture, not the constructor.
    raw.reachable = (0..crate::bzl_module::semantic_evaluation::budget::RECORD_LIMIT)
        .map(|_| BzlModuleSourceProvenance::new(raw.root.clone(), [0; 32]))
        .collect::<Vec<_>>()
        .into();
    let before = Arc::strong_count(&mapping);
    let env = PreparedBzlEnvironment::uninjected(BzlEnvironmentMode::BzlmodBootstrap).unwrap();
    assert!(BzlModuleSourceContext::new(&raw, &env, std::iter::empty()).is_err());
    assert_eq!(Arc::strong_count(&mapping), before);
}
