//! Actual attachment layout and positive retained/work certificate boundaries.
use starlark::values::ValueLike;

use super::*;
use crate::starlark_label::PreparedDefiningModuleMetadata as Metadata;

fn mapped_manifest() -> BzlLoadManifest {
    let mut raw = manifest();
    let mapping = slug_identity_v2::RepositoryMapping::new(
        slug_identity_v2::RepositoryMappingId::new("full-client-data-map").unwrap(),
    );
    raw.root.label = slug_identity_v2::ApparentLabel::parse("//pkg:client.bzl")
        .unwrap()
        .resolve(&mapping);
    raw.root.repository_mapping = Arc::from([
        (
            slug_identity_v2::ApparentRepoName::new("first").unwrap(),
            slug_identity_v2::CanonicalRepoName::new("selected+").unwrap(),
        ),
        (
            slug_identity_v2::ApparentRepoName::new("last").unwrap(),
            slug_identity_v2::CanonicalRepoName::new("foreign+").unwrap(),
        ),
    ]);
    raw.reachable = Arc::from([crate::BzlModuleSourceProvenance::new(
        raw.root.clone(),
        [9; 32],
    )]);
    raw.fingerprint = [11; 32];
    raw
}
fn bootstrap() -> crate::bzl_environment::PreparedBzlEnvironment {
    crate::bzl_environment::PreparedBzlEnvironment::uninjected(
        crate::bzl_environment::BzlEnvironmentMode::BzlmodBootstrap,
    )
    .unwrap()
}
fn certificate(meta: Metadata<'_>) -> PublicationShape {
    super::super::manifest::original_storage(meta).unwrap().0
}
fn native_owner(
    meta: Metadata<'_>,
    env: &crate::bzl_environment::PreparedBzlEnvironment,
    children: Vec<FunctionEvaluationOwner>,
) -> FunctionEvaluationOwner {
    bzl_evaluation_owner(
        meta,
        CodeMap::new("actual.bzl".into(), "def f(): return 7\n".into()),
        env,
        false,
        children.into_iter().map(Ok),
    )
    .unwrap()
}
fn freeze_with(meta: Metadata<'_>, owner: FunctionEvaluationOwner) -> FrozenModule {
    let module = Module::new();
    meta.attach(&module, owner.clone()).unwrap();
    let data = module.client_data().unwrap();
    assert_eq!(
        crate::starlark_label::defining_module_identity(data),
        Some(&meta.raw_manifest().root)
    );
    assert_eq!(
        crate::starlark_label::defining_module_source_manifest(data),
        Some(meta.raw_manifest())
    );
    let context = crate::provider::BzlEvaluationContext::from_manifest(meta.raw_manifest());
    let mut eval = Evaluator::new(&module);
    eval.set_function_symbol_session(function_symbol_session(owner))
        .unwrap();
    eval.set_function_export_mode(FunctionExportMode::TopLevelAssignments)
        .unwrap();
    eval.extra = Some(&context);
    eval.eval_module(
        AstModule::parse("actual.bzl", "def f(): return 7\n".into(), &Dialect::Bazel).unwrap(),
        &crate::package::loading_globals(),
    )
    .unwrap();
    drop(eval);
    module.freeze().unwrap()
}

#[test]
fn raw_attachment_keeps_full_original_payload_and_transient_context_only_in_work() {
    let child_raw = mapped_manifest();
    let child = owner(&child_raw, "child", false, []).unwrap();
    let raw = parent_manifest(7, &child_raw);
    let env = bootstrap();
    let context = crate::BzlModuleSourceContext::new(&raw, &env, std::iter::empty()).unwrap();
    let meta = Metadata::RawManifest {
        manifest: &raw,
        transient_context: &context,
    };
    let actual = certificate(meta);
    // Independently enumerate every attached raw row and its full identity.
    let ids = std::iter::once(&raw.root)
        .chain(std::iter::once(&raw.root))
        .chain(raw.direct_children.iter())
        .chain(raw.reachable.iter().map(|p| &p.identity));
    let mut bytes = Metadata::header_bytes()
        + std::mem::size_of::<BzlLoadManifest>()
        + 6 * std::mem::size_of::<usize>()
        + 32 * raw.reachable.len();
    let mut edges = 4 + raw.direct_children.len() + raw.reachable.len();
    for id in ids {
        let s = identity_shape(id).unwrap();
        bytes += s.bytes;
        edges += s.edges;
    }
    assert_eq!(
        (actual.bytes, actual.edges, actual.visits),
        (bytes, edges, 0)
    );
    let minimum_header = std::mem::size_of::<BzlModuleIdentity>()
        + std::mem::size_of::<Option<Arc<BzlLoadManifest>>>()
        + std::mem::size_of::<Option<crate::BzlModuleSourceContext>>()
        + std::mem::size_of::<Option<FunctionEvaluationOwner>>()
        + 8 * std::mem::size_of::<usize>();
    assert!(Metadata::header_bytes() >= minimum_header);
    let o = native_owner(meta, &env, vec![child]);
    let row = &o
        .input_validity()
        .as_any()
        .downcast_ref::<BzlInputValidity>()
        .unwrap()
        .rows[0];
    assert_eq!(
        (row.original_metadata.bytes, row.original_metadata.edges),
        (bytes, edges)
    );
    let frozen = freeze_with(meta, o);
    let data = frozen.client_data().unwrap().to_value();
    assert!(crate::starlark_label::defining_module_source_context(data).is_none());
    let attached = crate::starlark_label::defining_module_source_manifest(data).unwrap();
    assert_eq!(attached, &raw);
    assert!(Arc::ptr_eq(&attached.direct_children, &raw.direct_children));
    assert!(Arc::ptr_eq(&attached.reachable, &raw.reachable));
    assert!(Arc::ptr_eq(
        &attached.root.repository_mapping,
        &raw.root.repository_mapping
    ));
    let foreign: Arc<[crate::BzlModuleSourceProvenance]> = raw.reachable.to_vec().into();
    assert_eq!(foreign, raw.reachable);
    assert!(!Arc::ptr_eq(&foreign, &raw.reachable));
}

#[tokio::test]
async fn effective_attachment_keeps_one_complete_context_and_separate_defining_identity() {
    let child_raw = mapped_manifest();
    let child = owner(&child_raw, "child", false, []).unwrap();
    let raw = parent_manifest(8, &child_raw);
    let env = ordinary_bzlmod_environment().await;
    let context = crate::BzlModuleSourceContext::new(&raw, &env, std::iter::empty()).unwrap();
    assert!(!context.environment_bindings().is_empty());
    let meta = Metadata::EffectiveContext(&context);
    let actual = certificate(meta);
    let id = identity_shape(&raw.root).unwrap();
    assert_eq!(
        (actual.bytes, actual.edges, actual.visits),
        (
            Metadata::header_bytes() + id.bytes + context.retention().bytes,
            4 + id.edges + context.retention().records,
            0
        )
    );
    let o = native_owner(meta, &env, vec![child]);
    let row = &o
        .input_validity()
        .as_any()
        .downcast_ref::<BzlInputValidity>()
        .unwrap()
        .rows[0];
    assert_eq!(
        (row.original_metadata.bytes, row.original_metadata.edges),
        (actual.bytes, actual.edges)
    );
    let frozen = freeze_with(meta, o);
    let data = frozen.client_data().unwrap().to_value();
    let attached = crate::starlark_label::defining_module_source_context(data).unwrap();
    assert_eq!(attached, &context);
    assert_eq!(
        attached.environment_bindings(),
        context.environment_bindings()
    );
    assert_eq!(attached.fingerprint(), context.fingerprint());
    assert!(Arc::ptr_eq(
        attached.effective_reachable(),
        context.effective_reachable()
    ));
    assert!(Arc::ptr_eq(attached.sources(), context.sources()));
    assert!(Arc::ptr_eq(
        &attached.raw_load_manifest().direct_children,
        &raw.direct_children
    ));
    assert!(Arc::ptr_eq(
        &attached.raw_load_manifest().reachable,
        &raw.reachable
    ));
    assert_eq!(
        crate::starlark_label::defining_module_identity(data),
        Some(&raw.root)
    );
    assert_eq!(
        crate::starlark_label::defining_module_source_manifest(data),
        Some(&raw)
    );
}

#[test]
fn actual_metadata_and_legacy_transient_peak_refuse_before_copy_and_poison_publication() {
    use slug_build_api_v2::published_value::BYTE_LIMIT;
    use slug_build_api_v2::published_value::EDGE_LIMIT;
    use slug_build_api_v2::published_value::PublishedValueArenaBuilder;
    let raw = mapped_manifest();
    let env = bootstrap();
    let context = crate::BzlModuleSourceContext::new(&raw, &env, std::iter::empty()).unwrap();
    let meta = Metadata::RawManifest {
        manifest: &raw,
        transient_context: &context,
    };
    let guards = Arc::strong_count(&raw.root.repository_mapping);
    let weak = Arc::downgrade(&raw.root.repository_mapping);
    for bytes in [true, false] {
        for extra in [0, 1] {
            let mut work = PublicationShape::default();
            if bytes {
                work.bytes = 64 * 1024 * 1024 - context.retention().bytes + extra;
            } else {
                work.edges = 524288 - context.retention().records + extra;
            }
            assert_eq!(
                super::super::manifest::transient_peak(meta, &mut work).is_ok(),
                extra == 0
            );
            assert_eq!(Arc::strong_count(&raw.root.repository_mapping), guards);
        }
    }
    let mut effective_work = PublicationShape::default();
    super::super::manifest::transient_peak(
        Metadata::EffectiveContext(&context),
        &mut effective_work,
    )
    .unwrap();
    assert_eq!(
        (
            effective_work.bytes,
            effective_work.edges,
            effective_work.visits
        ),
        (0, 0, 0)
    );
    let o = native_owner(meta, &env, vec![]);
    let mut measured = PublishedValueArenaBuilder::new();
    measured.audit_owner_shape(&o).unwrap();
    let edges = measured.local_shape().edges;
    let bytes = measured.charged_bytes();
    drop(measured);
    let guards = Arc::strong_count(&raw.root.repository_mapping);
    for edge_limit in [true, false] {
        for extra in [0, 1] {
            let mut b = PublishedValueArenaBuilder::new();
            if edge_limit {
                b.charge_owner_edges(EDGE_LIMIT - edges + extra).unwrap();
            } else {
                b.charge_bytes(BYTE_LIMIT - bytes + extra).unwrap();
            }
            assert_eq!(b.audit_owner_shape(&o).is_ok(), extra == 0);
            if extra == 1 {
                assert!(b.reserve().is_err());
            }
            drop(b);
            assert_eq!(Arc::strong_count(&raw.root.repository_mapping), guards);
        }
    }
    drop(o);
    drop(context);
    drop(raw);
    assert!(weak.upgrade().is_none());
}

#[path = "client_data/attachment_copy.rs"]
mod attachment_copy;
