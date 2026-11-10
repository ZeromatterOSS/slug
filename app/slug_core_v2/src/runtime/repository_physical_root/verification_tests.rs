//! Full-tree comparison controls. Ordinary directory fixtures exercise only the
//! private verifier; they never pass native tmpfs/root admission or grant IO.
use std::fs::File;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::symlink;
use std::path::Path;

use slug_bzlmod_v2::RepositoryWorkspaceMode;

use super::super::native_fixture::BODY;
use super::super::native_tests::realization;
use super::super::native_tests::reservation;
use super::super::tests::resources;
use super::super::tests::scope;
use super::super::*;
use super::verify;

fn populate(root: &Path) {
    std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::create_dir(root.join("dir")).unwrap();
    std::fs::set_permissions(root.join("dir"), std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(root.join("dir/body"), BODY).unwrap();
    std::fs::set_permissions(
        root.join("dir/body"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    symlink("dir/body", root.join("link")).unwrap();
}

#[test]
fn complete_tree_verification_rejects_extra_root_and_nested_children() {
    let fixture = tempfile::tempdir().unwrap();
    populate(fixture.path());
    let selected = reservation(&fixture.path().join("logical-display"), 941);
    let terminal = realization(selected.clone(), RepositoryWorkspaceMode::Archive, BODY);
    let descriptor = File::open(fixture.path()).unwrap();
    let root = RootIo::new(
        &descriptor,
        io_limits(RepositoryWorkspaceMode::Archive, &selected),
    )
    .unwrap();
    let resources = resources();
    let baseline = resources.snapshot().retained_bytes;
    let scope = scope();
    let scratch = resources.reserve(IO_SCRATCH, 3, 0).unwrap();
    verify(&root, terminal.final_state(), &scope, &|| true).unwrap();
    for extra in ["unexpected", "dir/unexpected"] {
        std::fs::write(fixture.path().join(extra), b"extra").unwrap();
        assert!(matches!(
            verify(&root, terminal.final_state(), &scope, &|| true),
            Err(PhysicalRootError::Snapshot)
        ));
        std::fs::remove_file(fixture.path().join(extra)).unwrap();
        verify(&root, terminal.final_state(), &scope, &|| true).unwrap();
    }
    drop(scratch);
    assert_eq!(
        (
            resources.snapshot().retained_bytes,
            resources.snapshot().native_fds,
            resources.snapshot().backing_bytes
        ),
        (baseline, 0, 0)
    );
    assert_eq!(scope.snapshot().live_buffer_bytes, 0);
}

#[test]
fn complete_tree_verification_rejects_mode_and_equivalent_literal_link_changes() {
    let fixture = tempfile::tempdir().unwrap();
    populate(fixture.path());
    let selected = reservation(&fixture.path().join("logical-display"), 942);
    let terminal = realization(selected.clone(), RepositoryWorkspaceMode::Archive, BODY);
    let descriptor = File::open(fixture.path()).unwrap();
    let root = RootIo::new(
        &descriptor,
        io_limits(RepositoryWorkspaceMode::Archive, &selected),
    )
    .unwrap();
    let resources = resources();
    let baseline = resources.snapshot().retained_bytes;
    let scope = scope();
    let scratch = resources.reserve(IO_SCRATCH, 3, 0).unwrap();
    verify(&root, terminal.final_state(), &scope, &|| true).unwrap();
    std::fs::set_permissions(
        fixture.path().join("dir/body"),
        std::fs::Permissions::from_mode(0o744),
    )
    .unwrap();
    assert!(matches!(
        verify(&root, terminal.final_state(), &scope, &|| true),
        Err(PhysicalRootError::Snapshot)
    ));
    std::fs::set_permissions(
        fixture.path().join("dir/body"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    verify(&root, terminal.final_state(), &scope, &|| true).unwrap();
    std::fs::remove_file(fixture.path().join("link")).unwrap();
    symlink("dir/../dir/body", fixture.path().join("link")).unwrap();
    // The target denotes the same file and returns the same bytes, but its
    // literal link bytes are part of the selected immutable complete state.
    assert_eq!(std::fs::read(fixture.path().join("link")).unwrap(), BODY);
    assert!(matches!(
        verify(&root, terminal.final_state(), &scope, &|| true),
        Err(PhysicalRootError::Snapshot)
    ));
    drop(scratch);
    assert_eq!(
        (
            resources.snapshot().retained_bytes,
            resources.snapshot().native_fds,
            resources.snapshot().backing_bytes
        ),
        (baseline, 0, 0)
    );
    assert_eq!(scope.snapshot().live_buffer_bytes, 0);
}

#[test]
fn complete_tree_contents_stream_preserves_bytes_refusals_and_required_digest() {
    use sha2::Digest;

    use crate::runtime::repository_root_io::read_work_for_test;

    let fixture = tempfile::tempdir().unwrap();
    populate(fixture.path());
    let selected = reservation(&fixture.path().join("logical-display"), 943);
    let terminal = realization(selected.clone(), RepositoryWorkspaceMode::Archive, BODY);
    let descriptor = File::open(fixture.path()).unwrap();
    let root = RootIo::new(
        &descriptor,
        io_limits(RepositoryWorkspaceMode::Archive, &selected),
    )
    .unwrap();
    let resources = resources();
    let baseline = resources.snapshot().retained_bytes;
    let scope = scope();
    let scratch = resources
        .reserve(
            IO_SCRATCH + RepositoryWorkspaceBody::decode_scratch_bytes(),
            3,
            0,
        )
        .unwrap();
    let witness = read_work_for_test::begin();
    verify(&root, terminal.final_state(), &scope, &|| true).unwrap();
    let contents = witness.snapshot();
    drop(witness);
    assert_eq!(
        std::fs::read(fixture.path().join("dir/body")).unwrap(),
        BODY
    );

    let witness = read_work_for_test::begin();
    let required = root.digest_file(Path::new("dir/body"), &|| true).unwrap();
    let digest = witness.snapshot();
    drop(witness);
    assert_eq!(
        required.digest,
        <[u8; 32]>::from(sha2::Sha256::digest(BODY))
    );
    assert_eq!(required.metadata.st_size as usize, BODY.len());
    assert_eq!(
        (digest.digest_payload, digest.digest_sha),
        (BODY.len() as u64, BODY.len() as u64)
    );
    assert_eq!((digest.contents_payload, digest.contents_sha), (0, 0));

    let mut changed = BODY.to_vec();
    changed[0] ^= 1;
    std::fs::write(fixture.path().join("dir/body"), &changed).unwrap();
    assert!(matches!(
        verify(&root, terminal.final_state(), &scope, &|| true),
        Err(PhysicalRootError::Snapshot)
    ));
    std::fs::write(fixture.path().join("dir/body"), BODY).unwrap();
    std::fs::write(fixture.path().join("extra"), b"extra").unwrap();
    assert!(matches!(
        verify(&root, terminal.final_state(), &scope, &|| true),
        Err(PhysicalRootError::Snapshot)
    ));
    std::fs::remove_file(fixture.path().join("extra")).unwrap();
    assert!(verify(&root, terminal.final_state(), &scope, &|| false).is_err());
    verify(&root, terminal.final_state(), &scope, &|| true).unwrap();
    drop(scratch);
    drop(root);
    drop(descriptor);
    let retired = resources.snapshot();
    assert_eq!(
        (
            retired.retained_bytes,
            retired.native_fds,
            retired.backing_bytes
        ),
        (baseline, 0, 0)
    );
    assert_eq!(scope.snapshot().live_buffer_bytes, 0);
    assert_eq!(scope.snapshot().decoded_bytes, 0);
    assert_eq!(contents.contents_payload, BODY.len() as u64);
    assert_eq!((contents.digest_payload, contents.digest_sha), (0, 0));
    assert_eq!(
        contents.contents_sha, 0,
        "seal must compare every byte without computing a discarded digest"
    );
}
