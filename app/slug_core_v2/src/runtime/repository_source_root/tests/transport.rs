//! Negative transport authorization uses real closed captured roots; metadata
//! fixture construction grants no root, reader, registry or completion authority.
use std::fs;
use std::io;
use std::io::Seek;

use slug_bzlmod_v2::RepositorySourceSnapshotEpoch;
use slug_identity_v2::CanonicalLabel;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathObservationInstanceId;

use super::*;
use crate::runtime::SourceArtifactInput;
use crate::runtime::native_source_access::NativeSourceReader;
use crate::runtime::native_source_access::new_reader_resources;

fn metadata(
    root: &RepositorySourceRoot,
    relative: &str,
    digest: FileContentDigest,
) -> SourceArtifactInput {
    let path = NormalizedAbsolutePath::new(root.anchor().root().as_path().join(relative)).unwrap();
    SourceArtifactInput::for_native_lease_test(
        CanonicalLabel::parse(&format!(
            "@@{}//:{}",
            root.reservation().repository().as_str(),
            if relative.is_empty() {
                "payload"
            } else {
                relative
            }
        ))
        .unwrap(),
        root.anchor().namespace(),
        path.clone(),
        path,
        Some(root.anchor().clone()),
        digest,
    )
}
fn forge(
    source: &SourceArtifactInput,
    namespace: PathObservationNamespace,
    anchor: Option<MaterializationPathAnchor>,
    digest: FileContentDigest,
) -> SourceArtifactInput {
    SourceArtifactInput::for_native_lease_test(
        source.label().clone(),
        namespace,
        source.requested_path().clone(),
        source.real_path().clone(),
        anchor,
        digest,
    )
}

#[test]
#[ignore = "actual compiled CLI captured-source transport auth and original-scope lifetime; finite native supervisor required"]
fn native_captured_source_transport_rejects_foreign_epochs_metadata_and_scopes_and_retains_a() {
    let workspace = tempfile::tempdir().unwrap();
    fs::write(workspace.path().join("payload"), b"root A").unwrap();
    let normalized = NormalizedAbsolutePath::new(workspace.path()).unwrap();
    let resources = bank();
    let baseline = resources.snapshot().retained_bytes;
    let readers = new_reader_resources().unwrap();
    let reader_baseline = readers.snapshot().retained_bytes;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let actor = RepositoryMaterializer::new(normalized.clone());
    let a_scope = scope(&normalized, &resources);
    let (a_token, a_command, a) = initialize(
        &actor,
        workspace.path(),
        &resources,
        &a_scope,
        &runtime,
        false,
    )
    .unwrap();
    let a_lease = actor.workspace_lease(a_token).unwrap();
    let a_epoch = RepositorySourceSnapshotEpoch::fresh(normalized.clone(), a_lease)
        .unwrap()
        .with_snapshot(a.state().clone())
        .unwrap();
    let a_results = actor.selected_epoch(a_token, &[]).unwrap();
    let digest_a = match a.state().entry("payload").unwrap().node() {
        Node::File(digest) => *digest,
        _ => panic!("captured regular payload"),
    };
    let source_a = metadata(&a, "payload", digest_a);
    let mut entries = a.state().entries().to_vec();
    let entry = entries
        .iter_mut()
        .find(|entry| entry.path() == "payload")
        .unwrap();
    *entry = Entry::new(
        "payload".into(),
        entry.mode(),
        Node::File(FileContentDigest::new([0x55; 32], digest_a.size_bytes()).unwrap()),
    )
    .unwrap();
    let forged_state = Arc::new(State::new(a.reservation().clone(), entries).unwrap());
    let forged_epoch = RepositorySourceSnapshotEpoch::fresh(normalized.clone(), a_lease)
        .unwrap()
        .with_snapshot(forged_state)
        .unwrap();
    let before = a_scope.snapshot();
    let native_before = resources.snapshot();
    for invalid in [
        RepositorySourceSnapshotEpoch::revoked(),
        RepositorySourceSnapshotEpoch::fresh(normalized.clone(), a_lease).unwrap(),
        RepositorySourceSnapshotEpoch::fresh(normalized.clone(), a_lease + 1).unwrap(),
        forged_epoch,
    ] {
        assert!(
            actor
                .pin_selected_roots(a_token, &[], &a_results, &invalid, &readers)
                .is_err()
        );
    }
    assert_eq!(a_scope.snapshot().source_attempts, before.source_attempts);
    assert_eq!(resources.snapshot().native_fds, native_before.native_fds);
    assert_eq!(
        resources.snapshot().retained_bytes,
        native_before.retained_bytes
    );
    assert_eq!(readers.snapshot().retained_bytes, reader_baseline);
    let selected_a = actor
        .pin_selected_roots(a_token, &[], &a_results, &a_epoch, &readers)
        .unwrap();
    let lease_a = selected_a.lease_for(&source_a).unwrap().unwrap();
    assert!(lease_a.physical().is_none());
    assert!(Arc::ptr_eq(lease_a.source_snapshot().unwrap(), &a));
    let wrong_digest = FileContentDigest::new([0x44; 32], digest_a.size_bytes()).unwrap();
    let foreign_anchor = MaterializationPathAnchor::new(
        a.anchor().namespace(),
        NormalizedAbsolutePath::new("/__slug_repositories__/s1/foreign").unwrap(),
    )
    .unwrap();
    let host_path = NormalizedAbsolutePath::new(workspace.path().join("payload")).unwrap();
    let forged_host = SourceArtifactInput::for_native_lease_test(
        source_a.label().clone(),
        PathObservationNamespace::Host,
        host_path.clone(),
        host_path,
        None,
        digest_a,
    );
    for invalid in [
        forged_host,
        forge(
            &source_a,
            source_a.namespace(),
            Some(foreign_anchor),
            digest_a,
        ),
        forge(
            &source_a,
            source_a.namespace(),
            source_a.anchor().cloned(),
            wrong_digest,
        ),
        forge(
            &source_a,
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(u64::MAX)),
            source_a.anchor().cloned(),
            digest_a,
        ),
        metadata(&a, "", digest_a),
    ] {
        assert!(selected_a.lease_for(&invalid).is_err());
        assert!(
            NativeSourceReader::open_selected_for_test(
                &invalid,
                &selected_a,
                Some(a_scope.clone()),
                readers.clone()
            )
            .is_err()
        );
    }
    assert_eq!(readers.snapshot().native_fds, 0);
    assert!(
        NativeSourceReader::open_selected_for_test(&source_a, &selected_a, None, readers.clone())
            .is_err()
    );
    let foreign_scope = scope(&normalized, &resources);
    assert!(
        NativeSourceReader::open_selected_for_test(
            &source_a,
            &selected_a,
            Some(foreign_scope.clone()),
            readers.clone()
        )
        .is_err()
    );
    assert_eq!(readers.snapshot().native_fds, 0);
    drop(foreign_scope);
    let mut a_reader = NativeSourceReader::open_selected_for_test(
        &source_a,
        &selected_a,
        Some(a_scope.clone()),
        readers.clone(),
    )
    .unwrap();
    let a_display = a.reservation().display_root().clone();
    let a_instance = a.reservation().instance();
    let observer = a_command.retirement_observer();
    drop(a_command);
    assert!(observer.wait_until(Instant::now() + Duration::from_secs(1)));
    drop(observer);
    actor.accept(a_token, &[], vec![]).unwrap();
    assert!(
        actor
            .pin_selected_roots(a_token, &[], &a_results, &a_epoch, &readers)
            .is_err()
    );
    drop(a);
    fs::write(workspace.path().join("payload"), b"root B").unwrap();
    let b_scope = scope(&normalized, &resources);
    let (b_token, b_command, b) = initialize(
        &actor,
        workspace.path(),
        &resources,
        &b_scope,
        &runtime,
        false,
    )
    .unwrap();
    assert_eq!(b.reservation().display_root(), &a_display);
    assert_ne!(b.reservation().instance(), a_instance);
    let b_results = actor.selected_epoch(b_token, &[]).unwrap();
    let before_b = b_scope.snapshot();
    assert!(
        actor
            .pin_selected_roots(b_token, &[], &b_results, &a_epoch, &readers)
            .is_err()
    );
    assert_eq!(b_scope.snapshot().source_attempts, before_b.source_attempts);
    let b_epoch =
        RepositorySourceSnapshotEpoch::fresh(normalized, actor.workspace_lease(b_token).unwrap())
            .unwrap()
            .with_snapshot(b.state().clone())
            .unwrap();
    let selected_b = actor
        .pin_selected_roots(b_token, &[], &b_results, &b_epoch, &readers)
        .unwrap();
    assert!(selected_b.lease_for(&source_a).is_err());
    assert!(
        NativeSourceReader::open_selected_for_test(
            &source_a,
            &selected_a,
            Some(b_scope.clone()),
            readers.clone()
        )
        .is_err()
    );
    let digest_b = match b.state().entry("payload").unwrap().node() {
        Node::File(digest) => *digest,
        _ => panic!("captured regular payload"),
    };
    let source_b = metadata(&b, "payload", digest_b);
    let mut b_reader = NativeSourceReader::open_selected_for_test(
        &source_b,
        &selected_b,
        Some(b_scope.clone()),
        readers.clone(),
    )
    .unwrap();
    let mut body = Vec::new();
    a_reader.read_to_end(&mut body).unwrap();
    assert_eq!(body, b"root A");
    a_scope.cancel();
    assert_ne!(
        a_reader.read(&mut [0; 1]).unwrap_err().kind(),
        io::ErrorKind::Interrupted
    );
    b_scope.check().unwrap();
    body.clear();
    b_reader.read_to_end(&mut body).unwrap();
    assert_eq!(body, b"root B");
    drop(b_command);
    actor.discard(b_token).unwrap();
    drop(b);
    drop(actor);
    assert!(!a_display.as_path().exists());
    assert_eq!(resources.snapshot().backing_bytes, 2 * ARCHIVE_BYTES);
    drop(lease_a);
    drop(selected_a);
    drop(selected_b);
    drop(a_reader);
    drop(b_reader);
    drop(a_epoch);
    drop(b_epoch);
    drop(a_scope);
    drop(b_scope);
    assert_eq!(
        (
            readers.snapshot().native_fds,
            readers.snapshot().retained_bytes
        ),
        (0, reader_baseline)
    );
    assert_eq!(
        (
            resources.snapshot().native_fds,
            resources.snapshot().backing_bytes,
            resources.snapshot().retained_bytes
        ),
        (0, 0, baseline)
    );
}

#[test]
#[ignore = "actual compiled CLI Local source versus ordinary materialization drift; finite native supervisor required"]
fn native_local_source_selection_refuses_current_materialization_drift_and_accepts_restoration() {
    let workspace = tempfile::tempdir().unwrap();
    let local_a = workspace.path().join("local-a");
    let local_b = workspace.path().join("local-b");
    fs::create_dir(&local_a).unwrap();
    fs::create_dir(&local_b).unwrap();
    fs::write(local_a.join("payload"), b"local A").unwrap();
    fs::write(local_b.join("payload"), b"local B").unwrap();
    let normalized = NormalizedAbsolutePath::new(workspace.path()).unwrap();
    let resources = bank();
    let baseline = resources.snapshot().retained_bytes;
    let readers = new_reader_resources().unwrap();
    let reader_baseline = readers.snapshot().retained_bytes;
    let a_scope = scope(&normalized, &resources);
    let actor = RepositoryMaterializer::new(normalized.clone());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let (token, command, root) = initialize_requested(
        &actor,
        workspace.path(),
        &resources,
        &a_scope,
        &runtime,
        false,
        |lease| {
            crate::runtime::dice::local_source_request_for_test(workspace.path(), lease, &local_a)
        },
    )
    .unwrap();
    let epoch = RepositorySourceSnapshotEpoch::fresh(
        normalized.clone(),
        actor.workspace_lease(token).unwrap(),
    )
    .unwrap()
    .with_snapshot(root.state().clone())
    .unwrap();
    let empty_results = actor.selected_epoch(token, &[]).unwrap();
    // Absence of a separately materialized Local row is valid: the captured
    // source's closed producer already authenticated the full natural request.
    let selected = actor
        .pin_selected_roots(token, &[], &empty_results, &epoch, &readers)
        .unwrap();
    let original = root
        .reservation()
        .request()
        .materialization_request()
        .expect("genuine Local materialization request")
        .clone();
    let digest = match root.state().entry("payload").unwrap().node() {
        Node::File(digest) => *digest,
        _ => panic!("captured local file"),
    };
    let source = metadata(&root, "payload", digest);
    let mut reader = NativeSourceReader::open_selected_for_test(
        &source,
        &selected,
        Some(a_scope.clone()),
        readers.clone(),
    )
    .unwrap();

    let mut changed = original.as_ref().clone();
    changed.kind = slug_bzlmod_v2::RepositoryMaterializationKind::Local {
        logical_root: NormalizedAbsolutePath::new(&local_b).unwrap(),
    };
    let mut attributes = changed.repo_spec.attributes.as_ref().clone();
    attributes.insert(
        "path".into(),
        slug_bzlmod_v2::OverrideAttributeValue::String("local-b".into()),
    );
    changed.repo_spec.attributes = Arc::new(attributes);
    let changed = Arc::new(changed);
    assert_ne!(changed, original);
    // B is the FIRST ordinary row in this command. Replacing a freshly
    // materialized A here would correctly fail ConflictingRequest before the
    // cross-family selection guard could be exercised.
    actor
        .materialize_native(
            token,
            changed.clone(),
            slug_bzlmod_v2::RepositoryMaterializationGeneration(1),
        )
        .unwrap();
    let before = a_scope.snapshot();
    let native_before = resources.snapshot();
    assert!(
        actor
            .pin_selected_roots(token, &[], &empty_results, &epoch, &readers)
            .is_err(),
        "valid Source A cannot authorize a current conflicting Local B association even when ordinary selected requests are empty"
    );
    assert_eq!(a_scope.snapshot().source_attempts, before.source_attempts);
    assert_eq!(resources.snapshot().native_fds, native_before.native_fds);
    assert_eq!(
        resources.snapshot().retained_bytes,
        native_before.retained_bytes
    );
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"local A");
    let observer = command.retirement_observer();
    drop(command);
    assert!(observer.wait_until(Instant::now() + Duration::from_secs(1)));
    drop(observer);
    actor.accept(token, &[changed.clone()], vec![]).unwrap();

    // Restoration is a fresh command: B is now an accepted reusable ordinary
    // row, and A is captured again under a fresh lease, instance and scope.
    // No current non-reusable row is overwritten in the old command.
    let restored_scope = scope(&normalized, &resources);
    let (restored_token, restored_command, restored_root) = initialize_requested(
        &actor,
        workspace.path(),
        &resources,
        &restored_scope,
        &runtime,
        false,
        |lease| {
            crate::runtime::dice::local_source_request_for_test(workspace.path(), lease, &local_a)
        },
    )
    .unwrap();
    assert_eq!(
        restored_root.reservation().display_root(),
        root.reservation().display_root()
    );
    assert_ne!(
        restored_root.reservation().instance(),
        root.reservation().instance()
    );
    let restored_request = restored_root
        .reservation()
        .request()
        .materialization_request()
        .unwrap()
        .clone();
    assert_eq!(restored_request, original);
    actor
        .materialize_native(
            restored_token,
            restored_request,
            slug_bzlmod_v2::RepositoryMaterializationGeneration(2),
        )
        .unwrap();
    let restored_results = actor.selected_epoch(restored_token, &[]).unwrap();
    assert!(
        actor
            .pin_selected_roots(restored_token, &[], &restored_results, &epoch, &readers)
            .is_err(),
        "old A epoch cannot authorize the newly captured A instance"
    );
    let restored_epoch = RepositorySourceSnapshotEpoch::fresh(
        normalized,
        actor.workspace_lease(restored_token).unwrap(),
    )
    .unwrap()
    .with_snapshot(restored_root.state().clone())
    .unwrap();
    let restored = actor
        .pin_selected_roots(
            restored_token,
            &[],
            &restored_results,
            &restored_epoch,
            &readers,
        )
        .unwrap();
    let restored_source = metadata(&restored_root, "payload", digest);
    assert!(
        restored
            .lease_for(&restored_source)
            .unwrap()
            .unwrap()
            .source_snapshot()
            .is_some()
    );
    let mut restored_reader = NativeSourceReader::open_selected_for_test(
        &restored_source,
        &restored,
        Some(restored_scope.clone()),
        readers.clone(),
    )
    .unwrap();
    let mut restored_bytes = Vec::new();
    restored_reader.read_to_end(&mut restored_bytes).unwrap();
    assert_eq!(restored_bytes, b"local A");
    reader.seek(std::io::SeekFrom::Start(0)).unwrap();
    bytes.clear();
    reader.read_to_end(&mut bytes).unwrap();
    assert_eq!(
        bytes, b"local A",
        "issued A reader survives command retirement"
    );
    let observer = restored_command.retirement_observer();
    drop(restored_command);
    assert!(observer.wait_until(Instant::now() + Duration::from_secs(1)));
    drop(observer);
    actor.discard(restored_token).unwrap();
    drop(actor);
    drop(root);
    drop(selected);
    drop(restored);
    drop(restored_reader);
    drop(restored_root);
    drop(restored_epoch);
    drop(restored_scope);
    drop(reader);
    drop(epoch);
    drop(a_scope);
    assert_eq!(
        (
            readers.snapshot().native_fds,
            readers.snapshot().retained_bytes
        ),
        (0, reader_baseline)
    );
    assert_eq!(
        (
            resources.snapshot().native_fds,
            resources.snapshot().backing_bytes,
            resources.snapshot().retained_bytes
        ),
        (0, 0, baseline)
    );
}
