//! One genuine request, reached through relative and canonical source routes.
use super::*;
use crate::starlark_label::defining_module_source_context;

const MODULE: &str =
    "module(name='consumer')\nbazel_dep(name='local', version='1', repo_name='local_alias')\n";
const ENTRY: &[u8] = b"load(':helper.bzl', relative='read')\nload('@local//:helper.bzl', canonical='read')\nRELATIVE = relative\nCANONICAL = canonical\ndef read(): return [relative(), canonical()]\n";
const HELPER: &[u8] = b"def read(): return 7\n";
const CHANGED_HELPER: &[u8] = b"def read(): return 9\n";

async fn check_actual_readers(
    tx: &mut dice::DiceTransaction,
    route: &slug_bzlmod_v2::RootRepositoryRoute,
    canonical_key: &HostCanonicalRepositoryLoadRouteKey,
    root: &str,
    helper: &[u8],
) {
    let canonical_route = tx.compute(canonical_key).await.unwrap();
    let input = load_route(&canonical_route).input().clone();
    let SourcePreparationOutcome::Complete(Ok(root_read)) = tx
        .compute(&HostRepositorySourceFileObservationKey::new(
            route.clone(),
            PathBuf::from("helper.bzl"),
        ))
        .await
        .unwrap()
    else {
        panic!("actual root request source must complete")
    };
    let SourcePreparationOutcome::Complete(Ok(canonical_read)) = tx
        .compute(&HostRepositorySourceObservationEpochKey::new_canonical(
            input,
            host_repository_relative_path(PathBuf::from("helper.bzl")).unwrap(),
        ))
        .await
        .unwrap()
    else {
        panic!("actual canonical request source must complete")
    };
    let root_source = root_read.result().as_ref().as_ref().unwrap();
    let HostRepositorySourceObservationView::Request(canonical_source) =
        canonical_read.result().as_ref().as_ref().unwrap().view()
    else {
        panic!("selected Local source must retain Request attribution")
    };
    assert_eq!(
        root_source, canonical_source,
        "full actual reader values precede mixed evaluation"
    );
    let HostRepositorySourceFileValue::Present {
        bytes,
        logical_path,
    } = root_source
    else {
        panic!("both actual request readers must return present helper source")
    };
    assert_eq!(bytes.as_ref(), helper);
    assert_eq!(
        logical_path.as_path(),
        PathBuf::from(format!("{root}/helper.bzl"))
    );
    for operation in [
        PathObservationOperation::Lstat,
        PathObservationOperation::FileBytes,
    ] {
        let demand = PathObservationDemand::new(
            PathObservationNamespace::Host,
            logical_path.clone(),
            operation,
        );
        let actual = root_read.observations().observations().get(&demand);
        assert!(
            actual.is_some(),
            "reader must retain the actual Host demand"
        );
        assert_eq!(
            actual,
            canonical_read.observations().observations().get(&demand)
        );
    }
}

async fn load_original(
    tx: &mut dice::DiceTransaction,
    route: &slug_bzlmod_v2::RootRepositoryRoute,
    bzlmod: bool,
    observed: bool,
) -> crate::bzl_module::FrozenBzlModule {
    let label = external_label("", "entry.bzl");
    if observed {
        let key = if bzlmod {
            ExternalBzlModuleObservationKey::new_bzlmod(route.clone(), label)
        } else {
            ExternalBzlModuleObservationKey::new(route.clone(), label)
        };
        let SourcePreparationOutcome::Complete(Ok(value)) = tx.compute(&key).await.unwrap() else {
            panic!(
                "mixed observed routes must complete with the supplied exact source observations"
            )
        };
        value
            .result()
            .as_ref()
            .as_ref()
            .unwrap_or_else(|error| panic!("mixed observed routes: {error:?}"))
            .clone()
    } else {
        let key = if bzlmod {
            ExternalBzlModuleEvalKey::new_bzlmod(route.clone(), label)
        } else {
            ExternalBzlModuleEvalKey::new(route.clone(), label)
        };
        let SourcePreparationOutcome::Complete(value) = tx.compute(&key).await.unwrap() else {
            panic!("mixed legacy routes must complete with the supplied exact source observations")
        };
        value
            .as_ref()
            .as_ref()
            .unwrap_or_else(|error| panic!("mixed legacy routes: {error:?}"))
            .clone()
    }
}

fn check_original(
    loaded: &crate::bzl_module::FrozenBzlModule,
    route: &slug_bzlmod_v2::RootRepositoryRoute,
    root: &str,
    helper: &[u8],
    expected_result: &str,
) {
    assert_eq!(
        loaded.manifest.root.source.workspace_path().unwrap(),
        PathBuf::from(format!("{root}/entry.bzl"))
    );
    let relative = loaded.module.get("RELATIVE").unwrap();
    let canonical = loaded.module.get("CANONICAL").unwrap();
    assert!(
        relative.value().ptr_eq(canonical.value()),
        "same original source/mapping/mode has one actual semantic producer"
    );
    let expected = CanonicalLabel::parse("@@local+//:helper.bzl").unwrap();
    let context =
        defining_module_source_context(loaded.module.client_data().unwrap().to_value()).unwrap();
    let rows = context
        .effective_reachable()
        .iter()
        .filter(|p| p.identity.label == expected)
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].identity.repository_mapping,
        route.bzl_repository_mapping()
    );
    for original in [&relative, &canonical] {
        let code = starlark::eval::inspect_def(original.value(), None)
            .unwrap()
            .code();
        assert_eq!(code.source().filename(), format!("{root}/helper.bzl"));
        assert_eq!(code.source().source().as_bytes(), helper);
        let defining =
            defining_module_source_context(code.original_module_client_data().unwrap()).unwrap();
        assert_eq!(defining.raw_load_manifest().root, rows[0].identity);
        assert_eq!(defining.raw_load_manifest().reachable[0], *rows[0]);
        assert_eq!(
            rows[0].identity.source.workspace_path().unwrap(),
            PathBuf::from(code.source().filename())
        );
        assert!(
            context
                .sources()
                .iter()
                .any(
                    |(filename, provenance)| filename.as_str() == code.source().filename()
                        && provenance == rows[0]
                )
        );
    }
    let read = loaded.module.get("read").unwrap();
    let module = Module::new();
    let mut eval = Evaluator::new(&module);
    assert_eq!(
        eval.eval_function(read.value(), &[], &[])
            .unwrap()
            .to_repr(),
        expected_result
    );
}

async fn mixed_request_routes(bzlmod: bool, observed: bool) {
    let dice = builtin_graph_dice();
    let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
    let canonical_key = HostCanonicalRepositoryLoadRouteKey::new(
        workspace.clone(),
        CanonicalRepoName::new("local+").unwrap(),
    );
    let root = format!("{WORKSPACE}/local");
    let command = format!("local={root}");
    let policy = slug_bzlmod_v2::BzlmodCommandPolicyKey::from_flags_with_module_overrides(
        None,
        false,
        std::path::Path::new(WORKSPACE),
        Some(command.as_str()),
    )
    .unwrap();
    let mut pending = crate::canonical_repository_route_tests::tests::transaction_with_policy(
        &dice,
        MODULE,
        EXTENSION_A,
        true,
        None,
        policy,
    )
    .await;
    let SourcePreparationOutcome::Need(need) = pending.compute(&canonical_key).await.unwrap()
    else {
        panic!("actual selected Local route must request its declared source")
    };
    let request = need
        .repository_materializations()
        .values()
        .next()
        .unwrap()
        .clone();
    let tracker = Arc::new(DependencyTrace::default());
    let mut tx = request_transaction_with_observations(
        &dice,
        request.clone(),
        tracker,
        RepositoryMaterializationSuccess::Local,
        bzl_source_epoch(
            PathObservationNamespace::Host,
            &root,
            &[
                ("MODULE.bazel", Some(b"module(name='local')\n")),
                ("BUILD.bazel", Some(b"")),
                ("REPO.bazel", None),
                (".bazelignore", None),
                ("entry.bzl", Some(ENTRY)),
                ("helper.bzl", Some(HELPER)),
            ],
        ),
    )
    .await;
    let SourcePreparationOutcome::Complete(definition) = tx
        .compute(
            &slug_bzlmod_v2::HostCanonicalSelectedModuleDefinitionKey::new(
                workspace.clone(),
                CanonicalRepoName::new("local+").unwrap(),
            ),
        )
        .await
        .unwrap()
    else {
        panic!("actual selected source definition must complete")
    };
    let source = slug_bzlmod_v2::HostSelectedExtensionDefinitionSource::Selected {
        definition: definition.as_ref().as_ref().unwrap().clone(),
        apparent_repo: ApparentRepoName::new("local_alias").unwrap(),
    };
    let route =
        slug_bzlmod_v2::RootRepositoryRoute::for_selected_extension_definition(workspace, &source)
            .expect("actual selected CommandAbsolute Local route");
    assert!(matches!(
        route.source(),
        slug_bzlmod_v2::RootRepositorySource::SelectedLocal(_)
    ));
    assert_eq!(
        route.source_capability().local_path_policy(),
        Some(slug_bzlmod_v2::HostRepositoryLocalPathPolicy::CommandAbsolute)
    );
    let mut originals = Vec::with_capacity(3);
    for (ordinal, helper, expected_result) in [
        (0, HELPER, "[7, 7]"),
        (1, CHANGED_HELPER, "[9, 9]"),
        (2, HELPER, "[7, 7]"),
    ] {
        if ordinal != 0 {
            tx = request_transaction_with_observations(
                &dice,
                request.clone(),
                Arc::new(DependencyTrace::default()),
                RepositoryMaterializationSuccess::Local,
                bzl_source_epoch(
                    PathObservationNamespace::Host,
                    &root,
                    &[
                        ("MODULE.bazel", Some(b"module(name='local')\n")),
                        ("BUILD.bazel", Some(b"")),
                        ("REPO.bazel", None),
                        (".bazelignore", None),
                        ("entry.bzl", Some(ENTRY)),
                        ("helper.bzl", Some(helper)),
                    ],
                ),
            )
            .await;
        }
        check_actual_readers(&mut tx, &route, &canonical_key, &root, helper).await;
        let loaded = load_original(&mut tx, &route, bzlmod, observed).await;
        check_original(&loaded, &route, &root, helper, expected_result);
        originals.push(loaded);
    }
    let [a, changed, restored] = originals.as_slice() else {
        unreachable!()
    };
    assert_ne!(a.manifest, changed.manifest);
    assert_eq!(a.manifest, restored.manifest);
    assert_ne!(a.semantic_input_facts(), changed.semantic_input_facts());
    assert_eq!(a.semantic_input_facts(), restored.semantic_input_facts());
    let owner = |module: &crate::bzl_module::FrozenBzlModule| {
        crate::starlark_label::defining_module_function_owner(
            module.module.client_data().unwrap().to_value(),
        )
        .unwrap()
        .clone()
    };
    assert_ne!(owner(a), owner(changed));
    assert_eq!(owner(a), owner(restored));
    assert!(
        !a.module
            .get("read")
            .unwrap()
            .value()
            .ptr_eq(changed.module.get("read").unwrap().value())
    );
    assert!(
        a.module
            .get("read")
            .unwrap()
            .value()
            .ptr_eq(restored.module.get("read").unwrap().value())
    );
}

#[tokio::test]
async fn mixed_request_source_routes_build_legacy() {
    mixed_request_routes(false, false).await;
}
#[tokio::test]
async fn mixed_request_source_routes_build_observed() {
    mixed_request_routes(false, true).await;
}
#[tokio::test]
async fn mixed_request_source_routes_bzlmod_legacy() {
    mixed_request_routes(true, false).await;
}
#[tokio::test]
async fn mixed_request_source_routes_bzlmod_observed() {
    mixed_request_routes(true, true).await;
}
