//! Actual command/archive/seal lifecycle. The fixture authors arbitrary input
//! archives; native factory, capture, completion and seal remain ordinary owners.
use std::io::Read;
use std::io::Write;
use std::time::Duration;
use std::time::Instant;

use sha2::Digest;
use slug_bzlmod_v2::GeneratedRepositoryArchiveEffect;
use slug_bzlmod_v2::GeneratedRepositoryArchiveFormat;

use super::*;
use crate::runtime::dice::source_service::SourceServiceLimits;

const LARGE: usize = 48 * 1024 * 1024;
const BANK: usize = 64 * 1024 * 1024;
const KEPT: &[u8] = b"filtered final reader remains live\n";

fn bank() -> Arc<RepositoryRootResources> {
    RepositoryRootResources::new(
        RepositoryRootResourceLimits::new(BANK, 32, 4 * ARCHIVE_BYTES).unwrap(),
    )
    .unwrap()
}
fn command_scope(selected: &RepositoryWorkspaceReservation) -> Arc<SourceServiceScope> {
    let deadline = Instant::now() + Duration::from_secs(26);
    SourceServiceScope::new_for_test(
        selected.workspace().clone(),
        deadline,
        deadline + Duration::from_secs(1),
        SourceServiceLimits::default(),
    )
    .unwrap()
}
struct FixtureBytes {
    state: u64,
    remaining: usize,
}
impl FixtureBytes {
    fn new(seed: u8) -> Self {
        Self {
            state: 0x9e3779b97f4a7c15 ^ u64::from(seed),
            remaining: LARGE,
        }
    }
}
impl Read for FixtureBytes {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        let count = output.len().min(self.remaining);
        for byte in &mut output[..count] {
            self.state ^= self.state << 13;
            self.state ^= self.state >> 7;
            self.state ^= self.state << 17;
            *byte = (self.state >> 24) as u8;
        }
        self.remaining -= count;
        Ok(count)
    }
}
fn archive_input(
    directory: &Path,
    large_path: &str,
    byte: u8,
) -> (tempfile::NamedTempFile, RepositoryWorkspaceOperation) {
    archive_input_from_reader(directory, large_path, LARGE, FixtureBytes::new(byte))
}
fn archive_input_from_reader(
    directory: &Path,
    large_path: &str,
    length: usize,
    body: impl Read,
) -> (tempfile::NamedTempFile, RepositoryWorkspaceOperation) {
    let mut source = tempfile::NamedTempFile::new_in(directory).unwrap();
    {
        let gzip = flate2::write::GzEncoder::new(source.as_file_mut(), flate2::Compression::fast());
        let mut tar = tar::Builder::new(gzip);
        // Stream the fixture to avoid a second 48MiB producer allocation.
        let mut header = tar::Header::new_gnu();
        header.set_path(large_path).unwrap();
        header.set_size(length as u64);
        header.set_mode(0o644);
        header.set_mtime(1_600_000_000);
        header.set_cksum();
        tar.append(&header, body).unwrap();
        let mut header = tar::Header::new_gnu();
        header.set_path("kept").unwrap();
        header.set_size(KEPT.len() as u64);
        header.set_mode(0o644);
        header.set_mtime(1_600_000_000);
        header.set_cksum();
        tar.append(&header, KEPT).unwrap();
        tar.into_inner().unwrap().finish().unwrap();
    }
    source.flush().unwrap();
    let mut hash = sha2::Sha256::new();
    let mut read = source.reopen().unwrap();
    let mut scratch = [0; 64 * 1024];
    loop {
        let count = read.read(&mut scratch).unwrap();
        if count == 0 {
            break;
        }
        hash.update(&scratch[..count]);
    }
    let url = url::Url::from_file_path(source.path()).unwrap().to_string();
    let operation = RepositoryWorkspaceOperation::Archive(
        GeneratedRepositoryArchiveEffect::new(
            Arc::from([url.into()]),
            Some(hash.finalize().into()),
            "".into(),
            GeneratedRepositoryArchiveFormat::TarGz,
            "".into(),
            "".into(),
        )
        .unwrap(),
    );
    (source, operation)
}
fn apply(
    command: RepositoryWorkspaceCommand,
    selected: &RepositoryWorkspaceReservation,
    operation: RepositoryWorkspaceOperation,
    runtime: &tokio::runtime::Runtime,
    scope: &Arc<SourceServiceScope>,
) -> RepositoryWorkspaceCommand {
    let request = command
        .request(selected, operation, scope, &|| true)
        .unwrap();
    command
        .apply(
            request,
            runtime,
            &crate::runtime::ArchiveDistdirPolicy::default(),
            scope,
            &|| true,
        )
        .unwrap()
}
fn final_request(
    command: &RepositoryWorkspaceCommand,
    selected: &RepositoryWorkspaceReservation,
) -> Arc<slug_bzlmod_v2::RepositoryMaterializationRequest> {
    let transcript = command
        .epoch()
        .transcripts()
        .unwrap()
        .iter()
        .find(|transcript| transcript.reservation().as_ref() == selected)
        .unwrap();
    Arc::new(slug_bzlmod_v2::RepositoryMaterializationRequest {
        id: slug_bzlmod_v2::RepositoryMaterializationRequestId {
            workspace: selected.workspace().clone(),
            canonical_repo: selected.repository().clone(),
        },
        repo_spec: selected.expected_repo_spec().as_ref().clone(),
        kind: slug_bzlmod_v2::RepositoryMaterializationKind::GeneralWorkspace(
            transcript.realize(transcript.records().len()).unwrap(),
        ),
    })
}

#[test]
#[ignore = "requires trusted Rust physical Archive factory and actual 48MiB archive capture"]
fn native_deleted_archive_body_retires_with_history_while_final_reader_survives() {
    let fixture = tempfile::Builder::new()
        .prefix("slug-wp425-rolling-")
        .tempdir()
        .unwrap();
    let display = fixture.path().join("root");
    std::fs::create_dir(&display).unwrap();
    let selected = reservation(&display, 1891);
    let bank = bank();
    let baseline = bank.snapshot().retained_bytes;
    let scope = command_scope(&selected);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let (source, operation) = archive_input(fixture.path(), "large", b'a');
    let (command, fd) = start(selected.clone(), bank.clone(), scope.clone());
    let command = apply(command, &selected, operation, &runtime, &scope);
    let weak_large = {
        let transcript = &command.epoch().transcripts().unwrap()[0];
        let RepositoryWorkspaceNode::File(body) =
            transcript.current().entry("large").unwrap().node()
        else {
            panic!()
        };
        assert_eq!(body.len(), LARGE);
        assert!(
            body.native_storage_bytes() >= LARGE,
            "fixture must retain real encoded allocation pressure"
        );
        body.downgrade()
    };
    let command = apply(
        command,
        &selected,
        RepositoryWorkspaceOperation::Delete("large".into()),
        &runtime,
        &scope,
    );
    let transcript = &command.epoch().transcripts().unwrap()[0];
    assert!(transcript.current().entry("large").is_none());
    assert!(transcript.retention().body_bytes() >= LARGE as u64);
    let request = final_request(&command, &selected);
    let (command, sealed) = command
        .seal_for_materialization(request, &scope, &|| true)
        .unwrap();
    command.check_epoch_body_coverage(&scope, &|| true).unwrap();
    assert!(
        matches!(
            command.request(
                &selected,
                RepositoryWorkspaceOperation::Exists("kept".into()),
                &scope,
                &|| true
            ),
            Err(PhysicalRootError::Reservation)
        ),
        "retained allocation coverage cannot reopen sealed mutation authority"
    );
    let (request, root, history) = sealed.into_parts();
    assert!(root.state().entry("large").is_none());
    let weak_root = Arc::downgrade(&root);
    let RepositoryWorkspaceNode::File(kept) = root.state().entry("kept").unwrap().node() else {
        panic!()
    };
    assert!(
        kept.raw_arc().is_none(),
        "actual Archive state retains packed data"
    );
    let demand = slug_workspace_v2::PathObservationDemand::new(
        root.anchor().namespace(),
        NormalizedAbsolutePath::new(display.join("kept")).unwrap(),
        slug_workspace_v2::PathObservationOperation::FileBytes,
    );
    let before_observe = scope.snapshot();
    let observed = root.observe(&[demand.clone()], &scope, &|| true).unwrap();
    assert!(matches!(observed.get(&demand).unwrap().as_ref(),
        slug_workspace_v2::PathObservationResult::FileBytes(slug_workspace_v2::PathOperationResult::Present(bytes)) if bytes.as_ref() == KEPT));
    assert_eq!(
        scope.snapshot().source_attempts,
        before_observe.source_attempts
    );
    assert_eq!(
        scope.snapshot().decoded_bytes - before_observe.decoded_bytes,
        KEPT.len() as u64
    );
    drop(observed);
    let mut reader = root.open_member("kept", &scope, &|| true).unwrap();
    drop((command, request, root));
    assert!(
        weak_large.upgrade().is_some(),
        "full native history still owns deleted body"
    );
    let before = bank.snapshot().retained_bytes;
    drop(history);
    let after = bank.snapshot().retained_bytes;
    assert!(
        before.checked_sub(after).unwrap() >= LARGE,
        "deleted allocation credit must retire with the last history owner"
    );
    assert!(weak_large.upgrade().is_none());
    assert!(weak_root.upgrade().is_some());
    let mut actual = Vec::new();
    reader.read_to_end(&mut actual).unwrap();
    assert_eq!(actual, KEPT);
    drop(reader);
    assert!(weak_root.upgrade().is_none());
    release(&bank, baseline, fd);
    drop((source, runtime, scope));
}

#[test]
#[ignore = "requires trusted Rust physical Archive factory and two changed 48MiB captures"]
fn native_changed_archive_body_requires_real_old_new_allocation_overlap() {
    let fixture = tempfile::Builder::new()
        .prefix("slug-wp425-rolling-")
        .tempdir()
        .unwrap();
    let display = fixture.path().join("root");
    std::fs::create_dir(&display).unwrap();
    let selected = reservation(&display, 1892);
    let bank = bank();
    let baseline = bank.snapshot().retained_bytes;
    let scope = command_scope(&selected);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let (source_a, operation_a) = archive_input(fixture.path(), "large", b'a');
    let (source_b, operation_b) = archive_input(fixture.path(), "large", b'b');
    let (command, fd) = start(selected.clone(), bank.clone(), scope.clone());
    let command = apply(command, &selected, operation_a.clone(), &runtime, &scope);
    // A new index must really acquire/extract identical A again and reuse only
    // its fully verified allocation. Decoder/scratch admission alone must fit.
    let command = apply(command, &selected, operation_a, &runtime, &scope);
    assert_eq!(scope.snapshot().source_attempts, 2);
    assert_eq!(command.epoch().transcripts().unwrap()[0].records().len(), 2);
    let request = command
        .request(&selected, operation_b, &scope, &|| true)
        .unwrap();
    let result = command.apply(
        request,
        &runtime,
        &crate::runtime::ArchiveDistdirPolicy::default(),
        &scope,
        &|| true,
    );
    assert!(
        matches!(result, Err(PhysicalRootError::Resource)),
        "changed bodies cannot reuse an old allocation lease by size or digest surrogate"
    );
    release(&bank, baseline, fd);
    drop((source_a, source_b, runtime, scope));
}

#[path = "cross_path_tests.rs"]
mod cross_path_tests;

#[path = "toolchain_archive_tests.rs"]
mod toolchain_archive_tests;

#[test]
#[ignore = "actual trusted small Archive factory; tests packed metadata helpers without Source activity"]
fn native_packed_metadata_helpers_keep_full_foreign_equality_and_ignore_poisoned_source_scope() {
    use crate::runtime::dice::source_service::SourceServiceError;
    const PAYLOAD: &[u8] = &[b'p'; 1024];
    for deadline_poison in [false, true] {
        let fixture = tempfile::Builder::new()
            .prefix("slug-wp425-rolling-")
            .tempdir()
            .unwrap();
        let display = fixture.path().join("root");
        std::fs::create_dir(&display).unwrap();
        let selected = reservation(&display, 1895 + u64::from(deadline_poison));
        let bank = bank();
        let baseline = bank.snapshot().retained_bytes;
        let scope = command_scope(&selected);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (source, operation) =
            archive_input_from_reader(fixture.path(), "payload", PAYLOAD.len(), PAYLOAD);
        let original_operation = operation.clone();
        let (command, fd) = start(selected.clone(), bank.clone(), scope.clone());
        let command = apply(command, &selected, operation, &runtime, &scope);
        let request = final_request(&command, &selected);
        let (command, sealed) = command
            .seal_for_materialization(request, &scope, &|| true)
            .unwrap();
        let (request, root, history) = sealed.into_parts();
        let RepositoryWorkspaceNode::File(actual) = root.state().entry("payload").unwrap().node()
        else {
            panic!()
        };
        assert!(actual.raw_arc().is_none());
        assert!(
            actual.native_storage_bytes() < PAYLOAD.len(),
            "the metadata helpers must exercise actual Deflate decoding"
        );

        // Independently construct pure comparison inputs only after paying for
        // codec workspace, tiny bodies and their graph/control overlap. They
        // acquire no physical-root, claim, observation or storage authority.
        let foreign_credit = bank.reserve(3 * 1024 * 1024, 0, 0).unwrap();
        let (equal, different) = {
            let make = |different: bool| {
                let mut entries = Vec::new();
                for entry in root.state().entries() {
                    let node = match entry.node() {
                        RepositoryWorkspaceNode::File(body) => {
                            assert!(body.len() <= 1024);
                            let mut bytes = vec![0; body.len()];
                            assert!(body.copy_into(0, &mut bytes));
                            if different && entry.path() == "payload" {
                                bytes[0] ^= 1;
                            }
                            let mut builder =
                                RepositoryWorkspaceBodyBuilder::new(bytes.len()).unwrap();
                            for chunk in bytes.chunks(64 * 1024) {
                                builder.append(chunk, &mut |_| Ok::<_, ()>(())).unwrap();
                            }
                            let independent = builder.finish().unwrap();
                            assert!(independent.raw_arc().is_none());
                            assert!(!independent.ptr_eq(body));
                            RepositoryWorkspaceNode::File(independent)
                        }
                        node => node.clone(),
                    };
                    entries.push(
                        slug_bzlmod_v2::RepositoryWorkspaceEntry::new(
                            entry.path().into(),
                            entry.mode(),
                            node,
                        )
                        .unwrap(),
                    );
                }
                let initial = RepositoryWorkspaceTranscript::new(
                    selected.clone(),
                    root.state().entries()[0].mode(),
                )
                .unwrap();
                let completed = initial
                    .complete(
                        initial.request(original_operation.clone()).unwrap(),
                        command.epoch().transcripts().unwrap()[0].records()[0]
                            .result()
                            .clone(),
                        RepositoryWorkspaceMode::Archive,
                        entries,
                    )
                    .unwrap();
                completed.current().clone()
            };
            (make(false), make(true))
        };
        assert!(!Arc::ptr_eq(&equal, root.state()));
        assert!(!Arc::ptr_eq(&different, root.state()));
        for candidate in [&equal, &different] {
            for entry in candidate.entries() {
                if let RepositoryWorkspaceNode::File(body) = entry.node() {
                    let RepositoryWorkspaceNode::File(original) =
                        root.state().entry(entry.path()).unwrap().node()
                    else {
                        panic!()
                    };
                    assert!(
                        !body.ptr_eq(original),
                        "foreign packed allocations must be real"
                    );
                }
            }
        }
        let before_match = bank.snapshot().retained_bytes;
        assert!(root.matches_state(&equal).unwrap());
        assert!(!root.matches_state(&different).unwrap());
        assert_eq!(bank.snapshot().retained_bytes, before_match);
        let digest = slug_workspace_v2::FileContentDigest::new(
            sha2::Sha256::digest(PAYLOAD).into(),
            PAYLOAD.len() as u64,
        )
        .unwrap();
        let wrong =
            slug_workspace_v2::FileContentDigest::new([0; 32], PAYLOAD.len() as u64).unwrap();
        if deadline_poison {
            // Sticky typed poisoning, not a claim that the wall clock elapsed.
            scope.poison(SourceServiceError::WorkDeadline);
            assert_eq!(scope.check(), Err(SourceServiceError::WorkDeadline));
        } else {
            scope.cancel();
            assert_eq!(scope.check(), Err(SourceServiceError::Cancelled));
        }
        let sticky_before = scope.check().unwrap_err();
        let source_before = scope.snapshot();
        root.verify_direct_body_digest("payload", digest).unwrap();
        assert!(root.verify_direct_body_digest("payload", wrong).is_err());
        assert!(root.matches_state(&equal).unwrap());
        assert!(!root.matches_state(&different).unwrap());
        assert_eq!(bank.snapshot().retained_bytes, before_match);
        // The identical allocation can succeed without new working admission;
        // genuine foreign comparison must refuse at the actual bank boundary.
        let filler = bank.reserve(BANK - before_match, 0, 0).unwrap();
        assert!(root.matches_state(root.state()).unwrap());
        assert!(matches!(
            root.matches_state(&equal),
            Err(PhysicalRootError::Resource)
        ));
        assert!(matches!(
            root.verify_direct_body_digest("payload", digest),
            Err(PhysicalRootError::Resource)
        ));
        assert_eq!(bank.snapshot().retained_bytes, BANK);
        drop(filler);
        assert_eq!(bank.snapshot().retained_bytes, before_match);
        root.verify_direct_body_digest("payload", digest).unwrap();
        let source_after = scope.snapshot();
        assert_eq!(scope.check(), Err(sticky_before));
        assert_eq!(source_after.source_attempts, source_before.source_attempts);
        assert_eq!(source_after.decoded_bytes, source_before.decoded_bytes);
        assert_eq!(
            source_after.transferred_bytes,
            source_before.transferred_bytes
        );
        assert_eq!(source_after.written_bytes, source_before.written_bytes);
        drop((equal, different));
        drop(foreign_credit);
        drop((command, request, history, root));
        release(&bank, baseline, fd);
        drop((source, runtime, scope));
    }
}
