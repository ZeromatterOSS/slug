use std::io::Seek;
use std::io::SeekFrom;

use slug_bzlmod_v2::RepositoryWorkspaceError;
use slug_bzlmod_v2::RepositoryWorkspaceState;

use super::*;

const BODY_LIMIT: u64 = 1024 * MIB;
const MEMBER_LEN: usize = (BODY_LIMIT / 2) as usize - 4096;
const GAP: usize = 8192;
const TAIL: &str = "tail";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Boundary {
    Exact,
    TreeOverflow,
    HistoryOverflow,
}

fn file_write(len: usize, byte: u8) -> RepositoryWorkspaceOperation {
    RepositoryWorkspaceOperation::File {
        path: TAIL.into(),
        body: slug_bzlmod_v2::RepositoryWorkspaceBody::from_arc(vec![byte; len].into()),
        mode: 0o644,
    }
}

fn tree_bytes(state: &RepositoryWorkspaceState) -> u64 {
    state
        .entries()
        .iter()
        .filter_map(|entry| {
            let RepositoryWorkspaceNode::File(body) = entry.node() else {
                return None;
            };
            Some(body.len() as u64)
        })
        .sum()
}

fn run_body_case(id: u64, archives: &[ArchiveInput; 2], boundary: Boundary) {
    let (_temporary, selected, bank, baseline, scope, runtime, mut command, fd, identity) =
        command_scope(id);
    assert_eq!(selected.limits().archive_body_bytes(), BODY_LIMIT);
    let mut decoded = 0;
    for (index, archive) in archives.iter().enumerate() {
        command = apply(
            command,
            &selected,
            archive.operation.clone(),
            &runtime,
            &scope,
        );
        let transcript = &command.epoch().transcripts().unwrap()[0];
        assert_eq!(transcript.records().len(), index + 1);
        assert_eq!(
            transcript.records().last().unwrap().result(),
            &RepositoryWorkspaceResult::Archive(archive.digest),
        );
        let physical_body = (index + 1) as u64 * MEMBER_LEN as u64;
        assert_eq!(tree_bytes(transcript.current()), physical_body);
        assert_eq!(transcript.retention().body_bytes(), physical_body);
        assert_eq!(facts(fd).allocated, physical_body);
        decoded += archive.decoded;
        let counters = scope.snapshot();
        assert_eq!(counters.decoded_bytes, decoded);
        assert_eq!(counters.source_attempts, (index + 1) as u64);
        assert_eq!(counters.live_buffer_bytes, 0);
        assert_eq!(counters.live_decoder_bytes, 0);
    }
    assert_eq!(decoded, BODY_LIMIT - 5120);

    let len = GAP + usize::from(boundary == Boundary::TreeOverflow);
    let request = command
        .request(&selected, file_write(len, b't'), &scope, &|| true)
        .unwrap();
    let result = command.apply(request, &runtime, &Default::default(), &scope, &|| true);
    assert_eq!(scope.snapshot().decoded_bytes, decoded);
    assert!(scope.first_error().is_none());
    if boundary == Boundary::TreeOverflow {
        assert!(
            matches!(&result, Err(PhysicalRootError::Snapshot)),
            "{:?}",
            result.err()
        );
        assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, -1);
        drop(scope);
        release(&bank, baseline);
        eprintln!(
            "native Archive complete body refused {} bytes",
            BODY_LIMIT + 1
        );
        return;
    }
    let command = result.unwrap();
    let transcript = &command.epoch().transcripts().unwrap()[0];
    assert_eq!(transcript.records().len(), 3);
    assert_eq!(transcript.current().entries().len(), 4);
    assert_eq!(tree_bytes(transcript.current()), BODY_LIMIT);
    assert_eq!(transcript.retention().body_bytes(), BODY_LIMIT);
    let full = facts(fd);
    assert_eq!(full.allocated, BODY_LIMIT);
    if boundary == Boundary::HistoryOverflow {
        // The current tree shrinks. Keeping its previous full state and
        // admitting a new, distinct one-byte body would add exactly one byte.
        let request = command
            .request(&selected, file_write(1, b'x'), &scope, &|| true)
            .unwrap();
        let result = command.apply(request, &runtime, &Default::default(), &scope, &|| true);
        assert!(
            matches!(
                &result,
                Err(PhysicalRootError::Protocol(
                    RepositoryWorkspaceError::RetentionLimit
                ))
            ),
            "{:?}",
            result.err()
        );
        assert!(scope.first_error().is_none());
        assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, -1);
        drop(scope);
        release(&bank, baseline);
        eprintln!(
            "native Archive retained body refused {} bytes",
            BODY_LIMIT + 1
        );
        return;
    }

    let request = final_request(&command, &selected);
    let (command, sealed, view) = runtime
        .block_on(command.finalize_read_view(request, &scope, &|| true))
        .unwrap();
    assert_eq!(sealed.root().file.as_raw_fd(), fd);
    assert_eq!(sealed.root().identity, identity);
    assert_eq!(tree_bytes(sealed.root().state()), BODY_LIMIT);
    assert_eq!(
        facts(fd),
        Facts {
            bytes: BODY_LIMIT,
            inodes: 8192,
            ..full
        }
    );
    assert_eq!(bank.snapshot().backing_bytes, BODY_LIMIT);
    let members = archives
        .iter()
        .map(|archive| (archive.name, archive.len, archive.byte))
        .chain([(TAIL, GAP, b't')]);
    for (name, len, byte) in members {
        let mut reader = sealed.root().open_member(name, &scope, &|| true).unwrap();
        let mut block = [0; 4096];
        reader.read_exact(&mut block).unwrap();
        assert_eq!(block, [byte; 4096]);
        assert_eq!(
            reader.seek(SeekFrom::End(-4096)).unwrap(),
            len as u64 - 4096
        );
        reader.read_exact(&mut block).unwrap();
        assert_eq!(block, [byte; 4096]);
        assert_eq!(reader.read(&mut block).unwrap(), 0);
    }
    eprintln!(
        "native Archive body sealed {BODY_LIMIT} bytes, retained_peak={}",
        bank.snapshot().peak_retained_bytes
    );
    drop((command, sealed, view, scope));
    release(&bank, baseline);
}

#[test]
#[ignore = "actual compiled CLI and three distinct 1 GiB native Archive body cases; finite supervisor required"]
fn native_archive_body_boundary_distinguishes_tree_and_retained_history() {
    let decoded = MEMBER_LEN as u64 + 1536;
    let archives = [
        ArchiveInput::new("a", MEMBER_LEN, decoded, b'a'),
        ArchiveInput::new("b", MEMBER_LEN, decoded, b'b'),
    ];
    run_body_case(2417, &archives, Boundary::Exact);
    run_body_case(2418, &archives, Boundary::TreeOverflow);
    run_body_case(2419, &archives, Boundary::HistoryOverflow);
}
