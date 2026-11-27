use std::time::Duration;
use std::time::Instant;

use slug_bzlmod_v2::GeneratedRepositoryArchiveEffect;
use slug_bzlmod_v2::GeneratedRepositoryArchiveFormat;
use slug_bzlmod_v2::RepositoryWorkspaceNode;
use slug_bzlmod_v2::RepositoryWorkspaceRetentionLimits;

use super::*;
use crate::runtime::dice::source_service::SourceServiceLimits;
use crate::runtime::repository_io::ArchiveFailureStage;
use crate::runtime::repository_physical_root::native_tests::reservation_with_limits;

const MIB: u64 = 1024 * 1024;

#[path = "archive_body_limit_tests.rs"]
mod bodies;
#[path = "archive_root_limit_tests.rs"]
mod root_limits;
#[path = "archive_tree_history_tests.rs"]
mod tree_history;

fn verified_archive(file: &tempfile::NamedTempFile) -> (RepositoryWorkspaceOperation, [u8; 32]) {
    let digest = sha2::Sha256::digest(std::fs::read(file.path()).unwrap()).into();
    let archive = GeneratedRepositoryArchiveEffect::new(
        Arc::from([url::Url::from_file_path(file.path())
            .unwrap()
            .to_string()
            .into()]),
        Some(digest),
        "".into(),
        GeneratedRepositoryArchiveFormat::TarGz,
        "".into(),
        "".into(),
    )
    .unwrap();
    (RepositoryWorkspaceOperation::Archive(archive), digest)
}

struct ArchiveInput {
    _file: tempfile::NamedTempFile,
    operation: RepositoryWorkspaceOperation,
    digest: [u8; 32],
    decoded: u64,
    name: &'static str,
    len: usize,
    byte: u8,
}

impl ArchiveInput {
    fn new(name: &'static str, len: usize, decoded: u64, byte: u8) -> Self {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        let gzip = flate2::write::GzEncoder::new(file.as_file_mut(), flate2::Compression::fast());
        let mut tar = tar::Builder::new(gzip);
        let mut header = tar::Header::new_gnu();
        header.set_size(len as u64);
        header.set_mode(0o644);
        header.set_uid(0);
        header.set_gid(0);
        header.set_mtime(0);
        header.set_cksum();
        tar.append_data(&mut header, name, std::io::repeat(byte).take(len as u64))
            .unwrap();
        let mut gzip = tar.into_inner().unwrap();
        // One header block, the padded member, and two tar end blocks.
        let tar_bytes = (3 + len.div_ceil(512)) as u64 * 512;
        let padding = decoded.checked_sub(tar_bytes).unwrap();
        assert_eq!(
            std::io::copy(&mut std::io::repeat(0).take(padding), &mut gzip).unwrap(),
            padding,
        );
        gzip.finish().unwrap();
        let (operation, digest) = verified_archive(&file);
        Self {
            _file: file,
            operation,
            digest,
            decoded,
            name,
            len,
            byte,
        }
    }
}

fn command_inputs(
    id: u64,
) -> (
    tempfile::TempDir,
    Arc<RepositoryWorkspaceReservation>,
    Arc<RepositoryRootResources>,
    usize,
    Arc<SourceServiceScope>,
    tokio::runtime::Runtime,
) {
    let temporary = tempfile::tempdir().unwrap();
    let selected = reservation_with_limits(
        &Path::new(wire::RESERVED_PREFIX).join(temporary.path().file_name().unwrap()),
        id,
        RepositoryWorkspaceRetentionLimits::new(128, 16 * MIB as usize, 1024 * MIB).unwrap(),
    );
    let runtime = runtime();
    let bank = bank();
    let baseline = bank.snapshot().retained_bytes;
    let deadline = Instant::now() + Duration::from_secs(27);
    let scope = SourceServiceScope::new_command(
        selected.workspace().clone(),
        deadline,
        deadline + Duration::from_secs(3),
        SourceServiceLimits::default(),
        bank.clone(),
    )
    .unwrap();
    (temporary, selected, bank, baseline, scope, runtime)
}

fn command_scope(
    id: u64,
) -> (
    tempfile::TempDir,
    Arc<RepositoryWorkspaceReservation>,
    Arc<RepositoryRootResources>,
    usize,
    Arc<SourceServiceScope>,
    tokio::runtime::Runtime,
    RepositoryWorkspaceCommand,
    i32,
    RootIdentity,
) {
    let (temporary, selected, bank, baseline, scope, runtime) = command_inputs(id);
    let (command, fd, identity) = launch_archive(&selected, &bank, &scope, &runtime);
    (
        temporary, selected, bank, baseline, scope, runtime, command, fd, identity,
    )
}

fn run_case(id: u64, inputs: &[&ArchiveInput], overflow: Option<&str>) {
    let (_temporary, selected, bank, baseline, scope, runtime, mut command, fd, identity) =
        command_scope(id);
    let mut prior = facts(fd);
    assert_eq!((prior.bytes, prior.inodes), (ARCHIVE_BYTES, 262144));
    let mut decoded = 0;
    for (index, input) in inputs.iter().enumerate() {
        let request = command
            .request(&selected, input.operation.clone(), &scope, &|| true)
            .unwrap();
        let result = command.apply(request, &runtime, &Default::default(), &scope, &|| true);
        decoded += input.decoded;
        let counters = scope.snapshot();
        assert_eq!(counters.decoded_bytes, decoded);
        assert_eq!(counters.source_attempts, (index + 1) as u64);
        assert_eq!(counters.live_buffer_bytes, 0);
        assert_eq!(counters.live_decoder_bytes, 0);
        assert!(scope.first_error().is_none());
        if let Some(expected_error) = overflow.filter(|_| index + 1 == inputs.len()) {
            let Err(PhysicalRootError::Mutation(error)) = result else {
                panic!(
                    "expected decoded limit at {decoded}, got {:?}",
                    result.err()
                );
            };
            assert!(matches!(error.stage, ArchiveFailureStage::Materialization));
            assert_eq!(error.message, expected_error);
            assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, -1);
            drop(scope);
            release(&bank, baseline);
            eprintln!("native Archive refusal decoded={decoded}");
            return;
        }
        command = result.unwrap();
        let transcript = &command.epoch().transcripts().unwrap()[0];
        assert_eq!(transcript.records().len(), index + 1);
        assert_eq!(
            transcript.records().last().unwrap().result(),
            &RepositoryWorkspaceResult::Archive(input.digest),
        );
        let current = facts(fd);
        assert_eq!(
            current.allocated - prior.allocated,
            input.len.div_ceil(4096) as u64 * 4096,
        );
        assert!(current.used_inodes > prior.used_inodes);
        prior = current;
    }

    let target =
        NormalizedAbsolutePath::new(selected.display_root().as_path().join(inputs[0].name))
            .unwrap();
    let command = apply(
        command,
        &selected,
        RepositoryWorkspaceOperation::Symlink {
            path: "member-link".into(),
            target: target.clone(),
            source: None,
        },
        &runtime,
        &scope,
    );
    let with_link = facts(fd);
    assert!(with_link.used_inodes > prior.used_inodes);
    let request = final_request(&command, &selected);
    let (command, sealed, view) = runtime
        .block_on(command.finalize_read_view(request, &scope, &|| true))
        .unwrap();
    assert_eq!(sealed.root().file.as_raw_fd(), fd);
    assert_eq!(sealed.root().identity, identity);
    assert_eq!(
        facts(fd),
        Facts {
            bytes: TINY_ARCHIVE_BYTES,
            inodes: 8192,
            ..with_link
        }
    );
    assert_eq!(bank.snapshot().backing_bytes, TINY_ARCHIVE_BYTES);
    let state = sealed.root().state();
    let link = state.entry("member-link").unwrap().node();
    assert!(
        matches!(link, RepositoryWorkspaceNode::Link(literal) if Path::new(literal.as_str()) == target.as_path())
    );
    for input in inputs {
        let mut reader = sealed
            .root()
            .open_member(input.name, &scope, &|| true)
            .unwrap();
        let mut body = Vec::new();
        reader.read_to_end(&mut body).unwrap();
        assert_eq!(body, vec![input.byte; input.len]);
    }
    drop((command, sealed, view, scope));
    release(&bank, baseline);
    eprintln!("native Archive accepted decoded={decoded}");
}

#[test]
#[ignore = "actual compiled CLI and 27+3 second native Archive quota cases; finite supervisor required"]
fn native_archive_decoded_limits_use_actual_verified_streams() {
    let first = ArchiveInput::new("first", 4095, 512 * MIB, b'a');
    let second = ArchiveInput::new("second", 4096, 256 * MIB, b'a');
    let third = ArchiveInput::new("third", 4097, 256 * MIB, b'a');
    let plus_call = ArchiveInput::new("first", 4095, 512 * MIB + 1, b'a');
    let plus_total = ArchiveInput::new("third", 4097, 256 * MIB + 1, b'a');
    run_case(2410, &[&first, &second, &third], None);
    run_case(
        2411,
        &[&plus_call],
        Some("generated tar exceeds per-call decoded limit"),
    );
    run_case(
        2412,
        &[&first, &second, &plus_total],
        Some("generated archives exceed aggregate decoded limit"),
    );
}

#[test]
#[ignore = "actual compiled CLI and two native Archive path-byte boundary cases; finite supervisor required"]
fn native_archive_complete_tree_paths_accept_exact_limit_and_refuse_one_more() {
    'cases: for overflow in [false, true] {
        let (_temporary, selected, bank, baseline, scope, runtime, mut command, fd, identity) =
            command_scope(2413 + u64::from(overflow));
        let empty = facts(fd);
        let mut paths = Vec::new();
        for (index, branch) in ['a', 'b', 'c', 'd'].into_iter().enumerate() {
            // A 95-byte first component plus 575 5-byte components gives
            // 1,048,320 bytes over all 576 prefixes. One extra byte at
            // component 321 contributes 256 more bytes per branch.
            let mut path = format!("{branch}{}", "p".repeat(94));
            for depth in 2..=576 {
                path.push_str(if depth == 321 { "/member" } else { "/entry" });
            }
            let plus_one = overflow && index == 3;
            if plus_one {
                path.push('x');
            }
            let prefixes = path
                .match_indices('/')
                .map(|(index, _)| index)
                .sum::<usize>();
            assert_eq!(prefixes + path.len(), MIB as usize + usize::from(plus_one));
            assert!(selected.display_root().as_path().as_os_str().len() + 1 + path.len() <= 4096);
            let request = command
                .request(
                    &selected,
                    RepositoryWorkspaceOperation::File {
                        path: path.as_str().into(),
                        body: slug_bzlmod_v2::RepositoryWorkspaceBody::from_arc(Arc::from(
                            &b"payload"[..],
                        )),
                        mode: 0o644,
                    },
                    &scope,
                    &|| true,
                )
                .unwrap();
            let result = command.apply(request, &runtime, &Default::default(), &scope, &|| true);
            if plus_one {
                assert!(
                    matches!(&result, Err(PhysicalRootError::Snapshot)),
                    "{:?}",
                    result.err()
                );
                assert!(scope.first_error().is_none());
                assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, -1);
                // The consumed root cannot proceed to finalization.
                drop(scope);
                release(&bank, baseline);
                eprintln!("native Archive path quota refused 4194305 bytes");
                continue 'cases;
            }
            command = result.unwrap();
            let transcript = &command.epoch().transcripts().unwrap()[0];
            assert_eq!(transcript.records().len(), index + 1);
            let entries = transcript.current().entries();
            assert_eq!(entries.len(), (index + 1) * 576 + 1);
            assert_eq!(
                entries
                    .iter()
                    .map(|entry| entry.path().len())
                    .sum::<usize>(),
                (index + 1) * MIB as usize,
            );
            assert_eq!(
                facts(fd).allocated - empty.allocated,
                (index + 1) as u64 * 4096
            );
            paths.push(path);
        }
        let before = facts(fd);
        assert!(before.used_inodes > empty.used_inodes);
        let request = final_request(&command, &selected);
        let (command, sealed, view) = runtime
            .block_on(command.finalize_read_view(request, &scope, &|| true))
            .unwrap();
        assert_eq!(sealed.root().identity, identity);
        assert_eq!(sealed.root().file.as_raw_fd(), fd);
        let after = facts(fd);
        assert_eq!(
            (after.allocated, after.used_inodes),
            (before.allocated, before.used_inodes)
        );
        for path in paths {
            let mut reader = sealed.root().open_member(&path, &scope, &|| true).unwrap();
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).unwrap();
            assert_eq!(bytes, b"payload");
        }
        drop((command, sealed, view, scope));
        release(&bank, baseline);
        eprintln!("native Archive path quota accepted 4194304 bytes");
    }
}
