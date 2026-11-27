use std::ops::Range;

use slug_bzlmod_v2::RepositoryWorkspaceError;

use super::*;

const BRANCH_ENTRIES: usize = 32;
const BRANCH_PATH_BYTES: usize = 1120;

struct DirectoryArchive {
    _file: tempfile::NamedTempFile,
    operation: RepositoryWorkspaceOperation,
    digest: [u8; 32],
    branches: usize,
}

impl DirectoryArchive {
    fn new(branches: Range<usize>) -> Self {
        let count = branches.len();
        let mut file = tempfile::NamedTempFile::new().unwrap();
        let gzip = flate2::write::GzEncoder::new(file.as_file_mut(), flate2::Compression::fast());
        let mut tar = tar::Builder::new(gzip);
        // One directory header creates a disjoint 32-entry branch. No file
        // bodies participate in the history admission discriminator.
        let suffix = "/d".repeat(BRANCH_ENTRIES - 1);
        for branch in branches {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Directory);
            header.set_mode(0o755);
            header.set_size(0);
            header.set_uid(0);
            header.set_gid(0);
            header.set_mtime(0);
            header.set_cksum();
            tar.append_data(
                &mut header,
                format!("d{branch:03x}{suffix}"),
                std::io::empty(),
            )
            .unwrap();
        }
        tar.into_inner().unwrap().finish().unwrap();
        let (operation, digest) = verified_archive(&file);
        Self {
            _file: file,
            operation,
            digest,
            branches: count,
        }
    }
}

fn run_history(id: u64, order: [&DirectoryArchive; 3], expect_refusal: bool) {
    let (_temporary, selected, bank, baseline, scope, runtime, mut command, fd, identity) =
        command_scope(id);
    let mut previous = facts(fd);
    assert_eq!((previous.bytes, previous.inodes), (ARCHIVE_BYTES, 262144));
    let mut branches = 0;
    let mut decoded = 0;
    let mut retained = 0;
    for (index, archive) in order.into_iter().enumerate() {
        let request = command
            .request(&selected, archive.operation.clone(), &scope, &|| true)
            .unwrap();
        let result = command.apply(request, &runtime, &Default::default(), &scope, &|| true);
        branches += archive.branches;
        decoded += (archive.branches as u64 + 2) * 512;
        let counters = scope.snapshot();
        assert_eq!(counters.source_attempts, (index + 1) as u64);
        assert_eq!(
            counters.decoded_bytes, decoded,
            "full decoded directory archive"
        );
        assert_eq!(counters.live_buffer_bytes, 0);
        assert!(scope.first_error().is_none());
        if expect_refusal && index == 2 {
            assert!(
                matches!(
                    &result,
                    Err(PhysicalRootError::Protocol(
                        RepositoryWorkspaceError::RetentionLimit
                    ))
                ),
                "expected Archive history refusal, got {:?}",
                result.err(),
            );
            assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, -1);
            drop(scope);
            release(&bank, baseline);
            eprintln!(
                "native Archive history refused reordered full input after {retained} retained metadata bytes"
            );
            return;
        }
        command = result.unwrap_or_else(|error| {
            panic!("Archive history batch {index}, {branches} branches failed: {error:?}")
        });
        let transcript = &command.epoch().transcripts().unwrap()[0];
        assert_eq!(transcript.records().len(), index + 1);
        assert_eq!(
            transcript.records().last().unwrap().result(),
            &RepositoryWorkspaceResult::Archive(archive.digest),
        );
        assert_eq!(transcript.retention().body_bytes(), 0);
        assert!(transcript.retention().metadata_bytes() > retained);
        retained = transcript.retention().metadata_bytes();
        let entries = transcript.current().entries();
        assert_eq!(entries.len(), 1 + branches * BRANCH_ENTRIES);
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.path().len())
                .sum::<usize>(),
            branches * BRANCH_PATH_BYTES
        );
        assert!(
            entries
                .iter()
                .all(|entry| matches!(entry.node(), RepositoryWorkspaceNode::Directory))
        );
        let measured = facts(fd);
        assert!(measured.used_inodes > previous.used_inodes);
        assert_eq!(measured.allocated, previous.allocated);
        previous = measured;
    }
    let request = final_request(&command, &selected);
    let (command, sealed, view) = runtime
        .block_on(command.finalize_read_view(request, &scope, &|| true))
        .unwrap();
    assert_eq!(sealed.root().file.as_raw_fd(), fd);
    assert_eq!(sealed.root().identity, identity);
    assert_eq!(
        sealed.root().state().entries().len(),
        1 + branches * BRANCH_ENTRIES
    );
    let sealed_facts = facts(fd);
    assert_eq!(sealed_facts.used_inodes, previous.used_inodes);
    assert_eq!(sealed_facts.allocated, previous.allocated);
    assert_eq!(bank.snapshot().backing_bytes, sealed_facts.bytes);
    eprintln!(
        "native Archive history accepted entries={} path_bytes={} charged_inodes={} retained_metadata={retained}",
        sealed.root().state().entries().len(),
        branches * BRANCH_PATH_BYTES,
        sealed_facts.used_inodes,
    );
    drop((command, sealed, view, scope));
    release(&bank, baseline);
}

#[test]
#[ignore = "actual compiled CLI with two bounded native Archive histories; finite supervisor required"]
fn native_archive_history_distinguishes_order_at_same_complete_tree() {
    let a = DirectoryArchive::new(0..512);
    let b = DirectoryArchive::new(512..1536);
    let c = DirectoryArchive::new(1536..2560);
    run_history(2415, [&a, &b, &c], false);
    run_history(2416, [&b, &c, &a], true);
}
