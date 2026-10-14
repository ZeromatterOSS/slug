use slug_identity_v2::CanonicalLabel;
use slug_loading_v2::BzlModuleSourceProvenance;

use super::*;

fn manifest(label: &str, digest: [u8; 32]) -> BzlLoadManifest {
    let root = BzlModuleIdentity {
        label: CanonicalLabel::parse(label).unwrap(),
        workspace_path: "same/source.bzl".into(),
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
    let mut builder = CallableGraphBuilder::new();
    let digest = sources.digest(&code, &mut builder, &mut remaining).unwrap();
    let a = convert(
        &manifest("@@left//:helper.bzl", digest),
        &mut CallableGraphBuilder::new(),
        &mut remaining,
    )
    .unwrap();
    let b = convert(
        &manifest("@@right//:helper.bzl", digest),
        &mut CallableGraphBuilder::new(),
        &mut remaining,
    )
    .unwrap();
    assert_ne!(a, b);
    assert_eq!(a.data().root.workspace_path, b.data().root.workspace_path);
    sources
        .verify_source(&a, &code, &mut builder, &mut remaining)
        .unwrap();
    let mut exhausted = 0;
    assert!(
        sources
            .verify_source(&a, &code, &mut builder, &mut exhausted)
            .unwrap_err()
            .contains("visit limit")
    );
    let mut only_digest = 1;
    assert!(
        sources
            .verify_source(&a, &code, &mut builder, &mut only_digest)
            .unwrap_err()
            .contains("visit limit")
    );
    let changed = CodeMap::new(
        "same/source.bzl".into(),
        "def f(x):\n    return None\n".into(),
    );
    assert!(
        sources
            .verify_source(&a, &changed, &mut builder, &mut remaining)
            .is_err()
    );
    assert!(
        sources
            .authority(
                None,
                &code,
                &mut CallableGraphBuilder::new(),
                &mut remaining
            )
            .is_err()
    );
}
