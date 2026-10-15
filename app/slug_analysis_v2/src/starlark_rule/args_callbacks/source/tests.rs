use slug_identity_v2::CanonicalLabel;
use slug_loading_v2::BzlModuleSourceProvenance;

use super::*;

pub(super) fn manifest(label: &str, digest: [u8; 32]) -> BzlLoadManifest {
    let root = BzlModuleIdentity {
        label: CanonicalLabel::parse(label).unwrap(),
        source: slug_build_api_v2::ModuleSourceOrigin::Workspace("same/source.bzl".into()),
        repository_mapping: Arc::from([]),
    };
    BzlLoadManifest {
        root: root.clone(),
        direct_children: Arc::from([]),
        reachable: Arc::from([BzlModuleSourceProvenance::new(root, digest)]),
        fingerprint: [7; 32],
    }
}
#[test]
fn source_authority_keeps_logical_identity_and_checks_actual_source_bytes() {
    let code = CodeMap::new("same/source.bzl".into(), "def f(x):\n    return x\n".into());
    let mut sources = SourceAuthorities::default();
    let mut remaining = 65536;
    let mut builder = PublishedValueArenaBuilder::new();
    let digest = sources.digest(&code, &mut builder, &mut remaining).unwrap();
    let a = convert(
        &manifest("@@left//:helper.bzl", digest),
        &mut conversion::MappingAudits::default(),
        &mut PublishedValueArenaBuilder::new(),
        &mut remaining,
    )
    .unwrap();
    let b = convert(
        &manifest("@@right//:helper.bzl", digest),
        &mut conversion::MappingAudits::default(),
        &mut PublishedValueArenaBuilder::new(),
        &mut remaining,
    )
    .unwrap();
    assert_ne!(a, b);
    assert_eq!(a.data().root.source, b.data().root.source);
    sources
        .verify_source(&a, &code, &mut builder, &mut remaining)
        .unwrap();
    let mut only_lookup = 1;
    sources
        .verify_source(&a, &code, &mut builder, &mut only_lookup)
        .unwrap();
    assert_eq!(only_lookup, 0);
    let mut exhausted = 0;
    assert!(
        sources
            .verify_source(&a, &code, &mut builder, &mut exhausted)
            .unwrap_err()
            .contains("visit limit")
    );
    assert!(
        sources
            .verify_source(&a, &code, &mut builder, &mut remaining)
            .unwrap_err()
            .contains("previously failed")
    );
    let changed = CodeMap::new(
        "same/source.bzl".into(),
        "def f(x):\n    return None\n".into(),
    );
    assert!(
        SourceAuthorities::default()
            .verify_source(
                &a,
                &changed,
                &mut PublishedValueArenaBuilder::new(),
                &mut remaining
            )
            .is_err()
    );
    assert!(
        SourceAuthorities::default()
            .authority(
                None,
                &code,
                &mut PublishedValueArenaBuilder::new(),
                &mut remaining
            )
            .is_err()
    );
}
