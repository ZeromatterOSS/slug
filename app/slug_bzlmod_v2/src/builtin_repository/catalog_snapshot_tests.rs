use super::*;

#[test]
fn validated_snapshot_visitor_is_the_complete_verbatim_pinned_catalog() {
    let snapshot = BuiltinBazelToolsSnapshot::CURRENT;
    let mut visited = 0usize;
    snapshot
        .visit_validated_catalog(
            snapshot.route_identity(),
            |path, bytes, executable, digest| {
                let entry = &CATALOG[visited];
                assert_eq!(path, entry.path);
                assert_eq!(bytes, entry.bytes);
                assert!(std::ptr::eq(bytes.as_ptr(), entry.bytes.as_ptr()));
                assert_eq!(executable, entry.executable);
                assert_eq!(hex::encode(digest), entry.expected_sha256);
                let ordinary = lookup(path.into()).unwrap();
                assert_eq!(bytes, ordinary.bytes());
                assert_eq!(digest, ordinary.sha256());
                assert_eq!(executable, ordinary.executable());
                visited += 1;
                Ok::<_, BuiltinBazelToolsSourceFileError>(())
            },
        )
        .unwrap();
    assert_eq!(visited, CATALOG.len());
    assert!(visited > 1);
}

#[test]
fn snapshot_visitor_refuses_wrong_identity_before_any_writer_and_propagates_refusal() {
    let snapshot = BuiltinBazelToolsSnapshot::CURRENT;
    let mut foreign = snapshot.route_identity();
    foreign.manifest_sha256[0] ^= 1;
    let mut visited = 0usize;
    assert!(matches!(
        snapshot.visit_validated_catalog(foreign, |_, _, _, _| {
            visited += 1;
            Ok::<_, BuiltinBazelToolsSourceFileError>(())
        }),
        Err(BuiltinBazelToolsSourceFileError::UnsupportedCatalog { .. })
    ));
    assert_eq!(visited, 0);
    let refused = BuiltinBazelToolsSourceFileError::InvalidPath {
        path: "writer refused".into(),
    };
    let result = snapshot.visit_validated_catalog(snapshot.route_identity(), |_, _, _, _| {
        visited += 1;
        Err(refused.clone())
    });
    assert_eq!(result, Err(refused));
    assert_eq!(visited, 1);
}
