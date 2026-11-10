//! Ordinary borrowed RootIo comparison kernels only, not native-root admission.
use sha2::Digest;

use super::*;
use crate::runtime::repository_root_io::read_work_for_test;
use crate::runtime::repository_workspace_comparison_work_for_test as comparison_work;

const LENGTH: usize = 2 * 64 * 1024 + 13;

// Match only this fixture's live root/file inodes, not process-wide FD totals.
fn descriptors(root: &File, file: &File) -> usize {
    use std::os::unix::fs::MetadataExt;
    let identities = [root.metadata().unwrap(), file.metadata().unwrap()];
    fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|entry| {
            let fd = entry.ok()?.file_name().to_str()?.parse::<i32>().ok()?;
            let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
            // SAFETY: fstat borrows a live numeric descriptor and initializes on success.
            if unsafe { libc::fstat(fd, stat.as_mut_ptr()) } != 0 {
                return None;
            }
            let stat = unsafe { stat.assume_init() };
            identities
                .iter()
                .any(|m| m.dev() == stat.st_dev && m.ino() == stat.st_ino)
                .then_some(())
        })
        .count()
}

#[test]
fn observed_raw_and_packed_equal_and_early_mismatch_compare_every_chunk_before_zero_sha() {
    let authored = vec![b'k'; LENGTH];
    let mut observations = Vec::new();
    for packed in [false, true] {
        let fixture = tempfile::tempdir().unwrap();
        prepare(fixture.path());
        let path = fixture.path().join("file");
        let selected = reservation(Path::new("/absent/observed-compare"), 245);
        let bank = resources();
        let baseline = bank.snapshot().retained_bytes;
        let scope = scope();
        let construction = bank
            .reserve(
                RepositoryWorkspaceBodyBuilder::storage_bytes(LENGTH).unwrap()
                    + RepositoryWorkspaceBodyBuilder::encoding_scratch_bytes()
                    + 2 * LENGTH,
                0,
                0,
            )
            .unwrap();
        let old: RepositoryWorkspaceBody = if packed {
            let mut builder = RepositoryWorkspaceBodyBuilder::new(LENGTH).unwrap();
            for chunk in authored.chunks(64 * 1024) {
                builder.append(chunk, &mut |_| Ok::<(), ()>(())).unwrap();
            }
            builder.finish().unwrap()
        } else {
            Arc::<[u8]>::from(authored.as_slice()).into()
        };
        assert_eq!(old.raw_arc().is_none(), packed);
        if packed {
            assert!(old.native_storage_bytes() < old.len());
        }
        let weak = old.downgrade();
        let ledger = NativeBodyLedger::new(&bank, &scope, &selected, &|| true).unwrap();
        let mut candidates = ledger.candidates().unwrap();
        drop(candidates.retain(&old, &|| true).unwrap());
        let root = File::open(fixture.path()).unwrap();
        let io = RootIo::new(
            &root,
            io_limits(RepositoryWorkspaceMode::Archive, &selected),
        )
        .unwrap();
        let scratch = bank
            .reserve(
                IO_SCRATCH + RepositoryWorkspaceBody::decode_scratch_bytes(),
                3,
                0,
            )
            .unwrap();
        for mismatch in [false, true] {
            let mut actual = authored.clone();
            if mismatch {
                actual[0] ^= 1;
            }
            fs::write(&path, &actual).unwrap();
            let held_file = File::open(&path).unwrap();
            let before_fds = descriptors(&root, &held_file);
            let framed = io.lstat(Path::new("file")).unwrap();
            let before_credit = bank.snapshot().retained_bytes;
            let read = read_work_for_test::begin();
            let compare = comparison_work::begin();
            let same = body_recovery::observed_equal(
                &io,
                Path::new("file"),
                &framed,
                &old,
                &scope,
                &|| true,
            )
            .unwrap();
            let work = read.snapshot();
            let compared = compare.snapshot();
            drop(compare);
            drop(read);
            assert_eq!(same, !mismatch);
            assert!(seal::unchanged(
                &framed,
                &io.lstat(Path::new("file")).unwrap()
            ));
            assert_eq!(fs::read(&path).unwrap(), actual);
            assert!(old.equals_slice(&authored));
            assert!(weak.upgrade().unwrap().ptr_eq(&old));
            assert_eq!(work.digest_payload + work.contents_payload, LENGTH as u64);
            assert_eq!(compared.comparison_bytes, LENGTH as u64);
            assert!(compared.comparison_calls >= LENGTH.div_ceil(64 * 1024) as u64);
            observations.push(work);
            assert_eq!(bank.snapshot().retained_bytes, before_credit);
            assert_eq!(descriptors(&root, &held_file), before_fds);

            // Required-digest API remains a separate full actual SHA owner.
            let digest_work = read_work_for_test::begin();
            let required = io.digest_file(Path::new("file"), &|| true).unwrap();
            let digest = digest_work.snapshot();
            drop(digest_work);
            assert_eq!(
                required.digest,
                <[u8; 32]>::from(sha2::Sha256::digest(&actual))
            );
            assert!(seal::unchanged(&framed, &required.metadata));
            assert_eq!(
                (digest.digest_payload, digest.digest_sha),
                (LENGTH as u64, LENGTH as u64)
            );
            assert_eq!((digest.contents_payload, digest.contents_sha), (0, 0));
            assert_eq!(descriptors(&root, &held_file), before_fds);
        }
        drop(old);
        assert!(
            weak.upgrade().is_some(),
            "candidate lease retains the old allocation"
        );
        drop(candidates);
        drop(ledger);
        assert!(weak.upgrade().is_none());
        drop(scratch);
        drop(construction);
        drop(io);
        drop(root);
        let retired = bank.snapshot();
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
    }
    // No API-class assertion may preempt the real OLD-helper SHA-work RED.
    for work in &observations {
        assert_eq!(work.digest_sha + work.contents_sha, 0);
    }
    for work in observations {
        assert_eq!(work.contents_payload, LENGTH as u64);
        assert_eq!(work.digest_payload, 0);
    }
}

#[test]
fn early_mismatch_never_hides_late_eof_name_frame_or_cancel_refusal() {
    for action in ["truncate", "grow", "name", "mode", "cancel"] {
        let fixture = tempfile::tempdir().unwrap();
        prepare(fixture.path());
        let path = fixture.path().join("file");
        let authored = vec![b'k'; LENGTH];
        let mut actual = authored.clone();
        actual[0] ^= 1;
        fs::write(&path, &actual).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        let root = File::open(fixture.path()).unwrap();
        let file = File::open(&path).unwrap();
        let selected = reservation(Path::new("/absent/late-compare"), 246);
        let io = RootIo::new(
            &root,
            io_limits(RepositoryWorkspaceMode::Archive, &selected),
        )
        .unwrap();
        let framed = io.lstat(Path::new("file")).unwrap();
        let bank = resources();
        let baseline = bank.snapshot().retained_bytes;
        let scratch = bank
            .reserve(
                IO_SCRATCH + RepositoryWorkspaceBody::decode_scratch_bytes() + LENGTH + 16,
                3,
                0,
            )
            .unwrap();
        let scope = scope();
        let old: RepositoryWorkspaceBody = Arc::<[u8]>::from(authored.as_slice()).into();
        let before_fds = descriptors(&root, &file);
        let read = read_work_for_test::begin();
        let compare = comparison_work::begin();
        let changed = Cell::new(false);
        let active = || {
            let work = read.snapshot();
            let consumed = work.digest_payload + work.contents_payload;
            // Truncation/cancel occur after a real first mismatching chunk;
            // the other actions occur after all bytes but before EOF/frame checks.
            let threshold = if matches!(action, "truncate" | "cancel") {
                64 * 1024
            } else {
                LENGTH
            };
            if consumed >= threshold as u64 && !changed.replace(true) {
                match action {
                    "truncate" => fs::OpenOptions::new()
                        .write(true)
                        .open(&path)
                        .unwrap()
                        .set_len(1)
                        .unwrap(),
                    "grow" => fs::OpenOptions::new()
                        .write(true)
                        .open(&path)
                        .unwrap()
                        .set_len((LENGTH + 1) as u64)
                        .unwrap(),
                    "name" => {
                        fs::rename(&path, fixture.path().join("old-name")).unwrap();
                        fs::write(&path, &actual).unwrap();
                    }
                    "mode" => {
                        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap()
                    }
                    "cancel" => scope.cancel(),
                    _ => unreachable!(),
                }
            }
            true
        };
        let result =
            body_recovery::observed_equal(&io, Path::new("file"), &framed, &old, &scope, &active);
        let work = read.snapshot();
        let compared = compare.snapshot();
        drop(compare);
        drop(read);
        assert!(
            changed.get(),
            "late action {action} must execute after mismatching bytes"
        );
        if action == "cancel" {
            assert!(matches!(
                result,
                Err(PhysicalRootError::SourceService(
                    SourceServiceError::Cancelled
                ))
            ));
        } else {
            assert!(
                matches!(result, Err(PhysicalRootError::Io(_))),
                "{action}: {result:?}"
            );
        }
        let consumed = work.digest_payload + work.contents_payload;
        assert!(consumed >= 64 * 1024);
        assert_eq!(compared.comparison_bytes, consumed);
        if matches!(action, "grow" | "name" | "mode") {
            assert_eq!(consumed, LENGTH as u64);
        } else {
            assert!(consumed < LENGTH as u64);
        }
        assert_eq!(descriptors(&root, &file), before_fds);
        drop(old);
        drop(scratch);
        let retired = bank.snapshot();
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
    }
}
