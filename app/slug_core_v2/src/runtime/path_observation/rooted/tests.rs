use std::cell::Cell;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::symlink;
use std::time::Duration;
use std::time::Instant;

use sha2::Digest;
use sha2::Sha256;

use super::*;
use crate::runtime::dice::source_service::HOST_FILE_OBJECT_BYTES;
use crate::runtime::dice::source_service::SourceServiceLimits;
use crate::runtime::repository_root_io::RootIoLimits;

pub(super) fn limits() -> RootIoLimits {
    RootIoLimits {
        max_path_bytes: PATH_LIMIT,
        max_name_bytes: 255,
        max_depth: 2048,
        max_entries: MAX_DIRECTORY_ENTRIES,
        max_directory_name_bytes: MAX_DIRECTORY_NAMES,
        max_file_bytes: 1024 * 1024 * 1024,
        max_link_bytes: PATH_LIMIT,
    }
}

pub(super) fn scope(limits: SourceServiceLimits) -> Arc<SourceServiceScope> {
    let deadline = Instant::now() + Duration::from_secs(10);
    SourceServiceScope::new_for_test(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        deadline,
        deadline + Duration::from_secs(1),
        limits,
    )
    .unwrap()
}

pub(super) fn anchor(display: &Path, instance: u64) -> MaterializationPathAnchor {
    MaterializationPathAnchor::new(
        PathObservationNamespace::Materialization(PathObservationInstanceId::new(instance)),
        NormalizedAbsolutePath::new(display).unwrap(),
    )
    .unwrap()
}

pub(super) fn demand(
    anchor: &MaterializationPathAnchor,
    path: &str,
    operation: PathObservationOperation,
) -> PathObservationDemand {
    PathObservationDemand::new(
        anchor.namespace(),
        NormalizedAbsolutePath::new(anchor.root().as_path().join(path)).unwrap(),
        operation,
    )
}

fn value<'a>(
    epoch: &'a PathObservationEpoch,
    demand: &PathObservationDemand,
) -> &'a PathObservationResult {
    epoch.get(demand).unwrap().as_ref()
}

#[test]
fn rooted_observation_same_display_distinct_instances_ignore_absent_and_foreign_host_tree() {
    let fixture = tempfile::tempdir().unwrap();
    let display = fixture.path().join("display");
    let backing = fixture.path().join("backing");
    fs::create_dir(&display).unwrap();
    fs::write(display.join("data"), b"A\0\xff").unwrap();
    fs::set_permissions(display.join("data"), fs::Permissions::from_mode(0o751)).unwrap();
    let descriptor_a = File::open(&display).unwrap();
    let root_a = RootIo::new(&descriptor_a, limits()).unwrap();
    fs::rename(&display, &backing).unwrap();
    let anchor_a = anchor(&display, 1);
    let bytes_a = demand(&anchor_a, "data", PathObservationOperation::FileBytes);
    let digest_a = demand(&anchor_a, "data", PathObservationOperation::FileDigest);
    let root_stat = demand(&anchor_a, "", PathObservationOperation::Lstat);
    let file_stat = demand(&anchor_a, "data", PathObservationOperation::Lstat);
    let resources = scope(SourceServiceLimits::default());
    let epoch_a = observe_rooted(
        &root_a,
        &anchor_a,
        &[
            bytes_a.clone(),
            digest_a.clone(),
            root_stat.clone(),
            file_stat.clone(),
        ],
        &resources,
        &|| true,
    )
    .unwrap();
    assert!(!display.exists());
    assert!(
        matches!(value(&epoch_a, &bytes_a), PathObservationResult::FileBytes(PathOperationResult::Present(body)) if body.as_ref() == b"A\0\xff")
    );
    assert!(
        matches!(value(&epoch_a, &digest_a), PathObservationResult::FileDigest(PathOperationResult::Present(digest)) if digest.size_bytes() == 3 && digest.sha256().as_slice() == Sha256::digest(b"A\0\xff").as_slice())
    );
    assert!(
        matches!(value(&epoch_a, &root_stat), PathObservationResult::Lstat(PathOperationResult::Present(stat)) if stat.node_id() == root_a.root_stat().unwrap().st_ino as i64 && stat.kind() == PathNodeKind::Directory)
    );
    assert!(
        matches!(value(&epoch_a, &file_stat), PathObservationResult::Lstat(PathOperationResult::Present(stat)) if stat.permissions() == 0o751 && stat.size() == 3)
    );
    // The two metadata frames no longer consume the content-attempt domain.
    assert_eq!(resources.snapshot().source_attempts, 2);
    assert_eq!(resources.snapshot().decoded_bytes, 6);
    assert_eq!(resources.snapshot().live_buffer_bytes, 0);

    fs::create_dir(&display).unwrap();
    fs::write(display.join("data"), b"B").unwrap();
    let descriptor_b = File::open(&display).unwrap();
    let root_b = RootIo::new(&descriptor_b, limits()).unwrap();
    let anchor_b = anchor(&display, 2);
    let bytes_b = demand(&anchor_b, "data", PathObservationOperation::FileBytes);
    let epoch_b =
        observe_rooted(&root_b, &anchor_b, &[bytes_b.clone()], &resources, &|| true).unwrap();
    let repeated_a =
        observe_rooted(&root_a, &anchor_a, &[bytes_a.clone()], &resources, &|| true).unwrap();
    assert_ne!(bytes_a, bytes_b);
    assert!(
        matches!(value(&epoch_b, &bytes_b), PathObservationResult::FileBytes(PathOperationResult::Present(body)) if body.as_ref() == b"B")
    );
    assert!(
        matches!(value(&repeated_a, &bytes_a), PathObservationResult::FileBytes(PathOperationResult::Present(body)) if body.as_ref() == b"A\0\xff")
    );
    assert_eq!(fs::read(display.join("data")).unwrap(), b"B");
    assert_eq!(resources.snapshot().live_buffer_bytes, 0);
}

#[test]
fn rooted_observation_rejects_entire_foreign_above_anchor_and_duplicate_batches_before_io() {
    let fixture = tempfile::tempdir().unwrap();
    fs::write(fixture.path().join("data"), b"data").unwrap();
    let descriptor = File::open(fixture.path()).unwrap();
    let root = RootIo::new(&descriptor, limits()).unwrap();
    let selected = anchor(fixture.path(), 1);
    let good = demand(&selected, "data", PathObservationOperation::FileBytes);
    for bad in [
        PathObservationDemand::new(
            PathObservationNamespace::Host,
            good.path().clone(),
            PathObservationOperation::FileBytes,
        ),
        PathObservationDemand::new(
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(2)),
            good.path().clone(),
            PathObservationOperation::Lstat,
        ),
        PathObservationDemand::new(
            selected.namespace(),
            NormalizedAbsolutePath::new(fixture.path().parent().unwrap()).unwrap(),
            PathObservationOperation::Lstat,
        ),
        PathObservationDemand::new(
            selected.namespace(),
            NormalizedAbsolutePath::new(format!("{}-sibling/data", fixture.path().display()))
                .unwrap(),
            PathObservationOperation::Lstat,
        ),
        PathObservationDemand::new(
            selected.namespace(),
            NormalizedAbsolutePath::new(fixture.path().join("x".repeat(256))).unwrap(),
            PathObservationOperation::Lstat,
        ),
        good.clone(),
    ] {
        let resources = scope(SourceServiceLimits::default());
        assert!(
            observe_rooted(&root, &selected, &[good.clone(), bad], &resources, &|| true).is_err()
        );
        assert_eq!(resources.snapshot().source_attempts, 0);
        assert_eq!(resources.snapshot().decoded_bytes, 0);
        assert_eq!(resources.snapshot().live_buffer_bytes, 0);
    }
}

#[test]
fn rooted_observation_preserves_raw_links_sorted_native_names_and_refuses_unresolved_reads() {
    let fixture = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(fixture.path().join("z"), b"inside").unwrap();
    fs::write(outside.path().join("data"), b"outside").unwrap();
    fs::create_dir(fixture.path().join("dir")).unwrap();
    symlink("z", fixture.path().join("internal")).unwrap();
    symlink(outside.path(), fixture.path().join("escape")).unwrap();
    let raw_name = std::ffi::OsString::from_vec(vec![b'n', 0xff]);
    fs::write(fixture.path().join(&raw_name), b"native").unwrap();
    let descriptor = File::open(fixture.path()).unwrap();
    let root = RootIo::new(&descriptor, limits()).unwrap();
    let selected = anchor(Path::new("/absent/rooted-observation"), 1);
    let resources = scope(SourceServiceLimits::default());
    let list = demand(&selected, "", PathObservationOperation::DirectoryEntries);
    let link = demand(&selected, "internal", PathObservationOperation::ReadLink);
    let escape = demand(&selected, "escape", PathObservationOperation::ReadLink);
    let raw_bytes = demand(&selected, "internal", PathObservationOperation::FileBytes);
    let raw_digest = demand(&selected, "internal", PathObservationOperation::FileDigest);
    let resolved = demand(&selected, "z", PathObservationOperation::FileBytes);
    let epoch = observe_rooted(
        &root,
        &selected,
        &[
            list.clone(),
            link.clone(),
            escape.clone(),
            raw_bytes.clone(),
            raw_digest.clone(),
            resolved.clone(),
        ],
        &resources,
        &|| true,
    )
    .unwrap();
    assert!(
        matches!(value(&epoch, &link), PathObservationResult::ReadLink(PathOperationResult::Present(target)) if target.as_path() == Path::new("z"))
    );
    assert!(
        matches!(value(&epoch, &escape), PathObservationResult::ReadLink(PathOperationResult::Present(target)) if target.as_path() == outside.path())
    );
    assert!(matches!(
        value(&epoch, &raw_bytes),
        PathObservationResult::FileBytes(PathOperationResult::Error(
            PathObservationError::WrongKind {
                expected: PathNodeKind::RegularFile,
                actual: PathNodeKind::Symlink
            }
        ))
    ));
    assert!(matches!(
        value(&epoch, &raw_digest),
        PathObservationResult::FileDigest(PathOperationResult::Error(
            PathObservationError::WrongKind {
                actual: PathNodeKind::Symlink,
                ..
            }
        ))
    ));
    assert!(
        matches!(value(&epoch, &resolved), PathObservationResult::FileBytes(PathOperationResult::Present(bytes)) if bytes.as_ref() == b"inside")
    );
    let PathObservationResult::DirectoryEntries(PathOperationResult::Present(entries)) =
        value(&epoch, &list)
    else {
        panic!()
    };
    assert_eq!(
        entries
            .entries()
            .iter()
            .map(|e| e.name().as_os_str().as_bytes())
            .collect::<Vec<_>>(),
        vec![
            b"dir".as_slice(),
            b"escape",
            b"internal",
            &[b'n', 0xff],
            b"z"
        ]
    );
    assert_eq!(entries.entries()[1].kind(), PathDirectoryEntryKind::Symlink);
    let ancestor = demand(
        &selected,
        "escape/data",
        PathObservationOperation::FileBytes,
    );
    let escaped = observe_rooted(&root, &selected, &[ancestor.clone()], &resources, &|| true);
    assert!(!matches!(
        escaped
            .as_ref()
            .ok()
            .and_then(|e| e.get(&ancestor))
            .map(|r| r.as_ref()),
        Some(PathObservationResult::FileBytes(
            PathOperationResult::Present(_)
        ))
    ));
    assert_eq!(fs::read(outside.path().join("data")).unwrap(), b"outside");
    assert_eq!(resources.snapshot().live_buffer_bytes, 0);
}

#[test]
fn rooted_observation_missing_wrongkind_special_and_hardlink_results_do_not_follow() {
    let fixture = tempfile::tempdir().unwrap();
    fs::write(fixture.path().join("file"), b"body").unwrap();
    fs::create_dir(fixture.path().join("dir")).unwrap();
    nix::unistd::mkfifo(&fixture.path().join("fifo"), nix::sys::stat::Mode::S_IRUSR).unwrap();
    let descriptor = File::open(fixture.path()).unwrap();
    let root = RootIo::new(&descriptor, limits()).unwrap();
    let selected = anchor(fixture.path(), 1);
    let resources = scope(SourceServiceLimits::default());
    let missing = demand(&selected, "absent", PathObservationOperation::FileBytes);
    let directory = demand(&selected, "dir", PathObservationOperation::FileBytes);
    let not_directory = demand(
        &selected,
        "file",
        PathObservationOperation::DirectoryEntries,
    );
    let not_link = demand(&selected, "file", PathObservationOperation::ReadLink);
    let special = demand(&selected, "fifo", PathObservationOperation::FileBytes);
    let missing_digest = demand(&selected, "absent", PathObservationOperation::FileDigest);
    let directory_digest = demand(&selected, "dir", PathObservationOperation::FileDigest);
    let special_digest = demand(&selected, "fifo", PathObservationOperation::FileDigest);
    let epoch = observe_rooted(
        &root,
        &selected,
        &[
            missing.clone(),
            directory.clone(),
            not_directory.clone(),
            not_link.clone(),
            special.clone(),
            missing_digest.clone(),
            directory_digest.clone(),
            special_digest.clone(),
        ],
        &resources,
        &|| true,
    )
    .unwrap();
    assert!(matches!(
        value(&epoch, &missing),
        PathObservationResult::FileBytes(PathOperationResult::Missing)
    ));
    assert!(matches!(
        value(&epoch, &directory),
        PathObservationResult::FileBytes(PathOperationResult::Error(
            PathObservationError::WrongKind {
                actual: PathNodeKind::Directory,
                ..
            }
        ))
    ));
    assert!(matches!(
        value(&epoch, &not_directory),
        PathObservationResult::DirectoryEntries(PathOperationResult::Error(
            PathObservationError::WrongKind {
                actual: PathNodeKind::RegularFile,
                ..
            }
        ))
    ));
    assert!(matches!(
        value(&epoch, &not_link),
        PathObservationResult::ReadLink(PathOperationResult::Error(
            PathObservationError::WrongKind {
                actual: PathNodeKind::RegularFile,
                ..
            }
        ))
    ));
    assert!(matches!(
        value(&epoch, &special),
        PathObservationResult::FileBytes(PathOperationResult::Error(
            PathObservationError::WrongKind {
                actual: PathNodeKind::SpecialFile,
                ..
            }
        ))
    ));
    assert!(matches!(
        value(&epoch, &missing_digest),
        PathObservationResult::FileDigest(PathOperationResult::Missing)
    ));
    for (request, actual) in [
        (&directory_digest, PathNodeKind::Directory),
        (&special_digest, PathNodeKind::SpecialFile),
    ] {
        assert!(matches!(
            value(&epoch, request),
            PathObservationResult::FileDigest(PathOperationResult::Error(
                PathObservationError::WrongKind { actual: found, .. }
            )) if *found == actual
        ));
    }
    // Both content families charge even without readable bytes; the two
    // metadata wrong-kind facts are outside this superseded all-operation debit.
    assert_eq!(resources.snapshot().source_attempts, 6);
    assert_eq!(resources.snapshot().decoded_bytes, 0);
    fs::hard_link(fixture.path().join("file"), fixture.path().join("alias")).unwrap();
    assert!(
        observe_rooted(
            &root,
            &selected,
            &[demand(
                &selected,
                "file",
                PathObservationOperation::FileBytes
            )],
            &resources,
            &|| true
        )
        .is_err()
    );
    assert_eq!(resources.snapshot().source_attempts, 7);
    assert!(
        observe_rooted(
            &root,
            &selected,
            &[demand(
                &selected,
                "file",
                PathObservationOperation::FileDigest
            )],
            &resources,
            &|| true,
        )
        .is_err()
    );
    assert_eq!(resources.snapshot().source_attempts, 8);
    assert_eq!(resources.snapshot().live_buffer_bytes, 0);
}

#[test]
fn rooted_observation_filebytes_exact_object_limit_and_digest_streaming_have_distinct_bounds() {
    let fixture = tempfile::tempdir().unwrap();
    let descriptor = File::open(fixture.path()).unwrap();
    let root = RootIo::new(&descriptor, limits()).unwrap();
    let selected = anchor(fixture.path(), 1);
    let bytes_demand = demand(&selected, "file", PathObservationOperation::FileBytes);
    fs::write(
        fixture.path().join("file"),
        vec![b'x'; HOST_FILE_OBJECT_BYTES],
    )
    .unwrap();
    let resources = scope(SourceServiceLimits::default());
    let epoch = observe_rooted(
        &root,
        &selected,
        &[bytes_demand.clone()],
        &resources,
        &|| true,
    )
    .unwrap();
    assert!(
        matches!(value(&epoch, &bytes_demand), PathObservationResult::FileBytes(PathOperationResult::Present(body)) if body.len() == HOST_FILE_OBJECT_BYTES)
    );
    assert_eq!(resources.snapshot().source_attempts, 1);
    assert_eq!(
        resources.snapshot().decoded_bytes,
        HOST_FILE_OBJECT_BYTES as u64
    );
    assert_eq!(resources.snapshot().live_buffer_bytes, 0);
    fs::write(
        fixture.path().join("file"),
        vec![b'x'; HOST_FILE_OBJECT_BYTES + 1],
    )
    .unwrap();
    let refused = scope(SourceServiceLimits::default());
    assert!(matches!(
        observe_rooted(&root, &selected, &[bytes_demand], &refused, &|| true),
        Err(RootedObservationError::SourceService(
            SourceServiceError::Limit(SourceCounter::DecodedBytes)
        ))
    ));
    assert_eq!(refused.snapshot().source_attempts, 1);
    assert_eq!(refused.snapshot().decoded_bytes, 0);
    assert_eq!(refused.snapshot().live_buffer_bytes, 0);
    let digest_demand = demand(&selected, "file", PathObservationOperation::FileDigest);
    let digest_scope = scope(SourceServiceLimits::default());
    let digest_epoch = observe_rooted(
        &root,
        &selected,
        &[digest_demand.clone()],
        &digest_scope,
        &|| true,
    )
    .unwrap();
    assert!(
        matches!(value(&digest_epoch, &digest_demand), PathObservationResult::FileDigest(PathOperationResult::Present(digest)) if digest.size_bytes() == (HOST_FILE_OBJECT_BYTES + 1) as u64)
    );
    assert_eq!(digest_scope.snapshot().source_attempts, 1);
    assert_eq!(
        digest_scope.snapshot().decoded_bytes,
        (HOST_FILE_OBJECT_BYTES + 1) as u64
    );
    assert_eq!(digest_scope.snapshot().live_buffer_bytes, 0);
    assert!(digest_scope.snapshot().peak_buffer_bytes < HOST_FILE_OBJECT_BYTES);
}

#[test]
fn rooted_metadata_frontier_exceeds_content_attempt_cap_and_revalidates_actual_changes() {
    let fixture = tempfile::tempdir().unwrap();
    let descriptor = File::open(fixture.path()).unwrap();
    let root = RootIo::new(&descriptor, limits()).unwrap();
    let selected = anchor(Path::new("/absent/rooted-metadata"), 1);
    let resources = scope(SourceServiceLimits::default());
    let mut demands = Vec::new();
    for index in 0..65 {
        let name = format!("file{index}");
        fs::write(fixture.path().join(&name), b"x").unwrap();
        demands.push(demand(&selected, &name, PathObservationOperation::Lstat));
    }
    symlink("file0", fixture.path().join("link")).unwrap();
    let link = demand(&selected, "link", PathObservationOperation::ReadLink);
    let directory = demand(&selected, "", PathObservationOperation::DirectoryEntries);
    demands.extend([link.clone(), directory.clone()]);
    let first = observe_rooted(&root, &selected, &demands, &resources, &|| true).unwrap();
    assert_eq!(first.observations().len(), 67);
    assert_eq!(resources.snapshot().source_attempts, 0);
    assert_eq!(resources.snapshot().decoded_bytes, 0);
    assert_eq!(resources.snapshot().live_buffer_bytes, 0);

    // Excluding metadata from SourceAttempts does not authorize an observation
    // cache: every repeat still traverses the actual descriptor-selected tree.
    fs::write(fixture.path().join("file0"), b"changed").unwrap();
    fs::remove_file(fixture.path().join("link")).unwrap();
    symlink("file1", fixture.path().join("link")).unwrap();
    fs::create_dir(fixture.path().join("new-directory")).unwrap();
    let second = observe_rooted(&root, &selected, &demands, &resources, &|| true).unwrap();
    assert!(matches!(
        value(&second, &demands[0]),
        PathObservationResult::Lstat(PathOperationResult::Present(stat)) if stat.size() == 7
    ));
    assert!(matches!(
        value(&second, &link),
        PathObservationResult::ReadLink(PathOperationResult::Present(target)) if target.as_path() == Path::new("file1")
    ));
    assert!(matches!(
        value(&second, &directory),
        PathObservationResult::DirectoryEntries(PathOperationResult::Present(entries))
            if entries.entries().iter().any(|entry| entry.name().as_os_str() == "new-directory")
    ));
    assert_ne!(first, second);
    assert_eq!(resources.snapshot().source_attempts, 0);
    assert_eq!(resources.snapshot().decoded_bytes, 0);
    assert_eq!(resources.snapshot().live_buffer_bytes, 0);
    assert!(!selected.root().as_path().exists());
}

#[test]
fn rooted_content_attempts_share_exact_64_boundary_and_refuse_65_before_lstat() {
    let fixture = tempfile::tempdir().unwrap();
    fs::write(fixture.path().join("file"), b"body").unwrap();
    let descriptor = File::open(fixture.path()).unwrap();
    let root = RootIo::new(&descriptor, limits()).unwrap();
    let selected = anchor(fixture.path(), 1);
    let resources = scope(SourceServiceLimits::default());
    for index in 0..64 {
        let operation = if index % 2 == 0 {
            PathObservationOperation::FileBytes
        } else {
            PathObservationOperation::FileDigest
        };
        observe_rooted(
            &root,
            &selected,
            &[demand(&selected, "file", operation)],
            &resources,
            &|| true,
        )
        .unwrap();
        assert_eq!(resources.snapshot().source_attempts, index + 1);
    }
    let checks = Cell::new(0);
    let active = || {
        let prior = checks.get();
        checks.set(prior + 1);
        // The third check is immediately after the first lstat in observe_one.
        assert!(prior < 2, "content-limit refusal must precede real lstat");
        true
    };
    assert!(matches!(
        observe_rooted(
            &root,
            &selected,
            &[demand(
                &selected,
                "file",
                PathObservationOperation::FileBytes
            )],
            &resources,
            &active,
        ),
        Err(RootedObservationError::SourceService(
            SourceServiceError::Limit(SourceCounter::SourceAttempts)
        ))
    ));
    assert_eq!(checks.get(), 2);
    assert_eq!(resources.snapshot().source_attempts, 64);
    assert_eq!(resources.snapshot().decoded_bytes, 64 * 4);
    assert_eq!(resources.snapshot().live_buffer_bytes, 0);
}

#[test]
fn rooted_observation_scope_attempt_decoded_and_native_buffer_refusals_release_credits() {
    let fixture = tempfile::tempdir().unwrap();
    fs::write(fixture.path().join("file"), b"body").unwrap();
    let descriptor = File::open(fixture.path()).unwrap();
    let root = RootIo::new(&descriptor, limits()).unwrap();
    let selected = anchor(fixture.path(), 1);
    let file = demand(&selected, "file", PathObservationOperation::FileBytes);
    let digest = demand(&selected, "file", PathObservationOperation::FileDigest);
    let resources = scope(SourceServiceLimits {
        source_attempts: 1,
        ..SourceServiceLimits::default()
    });
    assert!(observe_rooted(&root, &selected, &[file.clone()], &resources, &|| true).is_ok());
    assert!(matches!(
        observe_rooted(&root, &selected, &[file.clone()], &resources, &|| true),
        Err(RootedObservationError::SourceService(
            SourceServiceError::Limit(SourceCounter::SourceAttempts)
        ))
    ));
    assert_eq!(resources.snapshot().source_attempts, 1);
    assert_eq!(resources.snapshot().live_buffer_bytes, 0);
    let decoded = scope(SourceServiceLimits {
        decoded_bytes: 3,
        ..SourceServiceLimits::default()
    });
    assert!(matches!(
        observe_rooted(&root, &selected, &[digest], &decoded, &|| true),
        Err(RootedObservationError::SourceService(
            SourceServiceError::Limit(SourceCounter::DecodedBytes)
        ))
    ));
    assert_eq!(decoded.snapshot().decoded_bytes, 0);
    assert_eq!(decoded.snapshot().live_buffer_bytes, 0);
    let small = scope(SourceServiceLimits {
        live_buffer_bytes: NATIVE_CREDIT - 1,
        ..SourceServiceLimits::default()
    });
    assert!(observe_rooted(&root, &selected, &[file.clone()], &small, &|| true).is_err());
    assert_eq!(small.snapshot().source_attempts, 0);
    assert_eq!(small.snapshot().live_buffer_bytes, 0);
    let canceled = scope(SourceServiceLimits::default());
    canceled.cancel();
    assert!(observe_rooted(&root, &selected, &[file.clone()], &canceled, &|| true).is_err());
    assert_eq!(canceled.snapshot().source_attempts, 0);
    assert_eq!(canceled.snapshot().live_buffer_bytes, 0);
    let inactive = scope(SourceServiceLimits::default());
    let calls = Cell::new(0);
    let active = || {
        let count = calls.get();
        calls.set(count + 1);
        count < 2
    };
    assert!(observe_rooted(&root, &selected, &[file], &inactive, &active).is_err());
    assert_eq!(inactive.snapshot().decoded_bytes, 0);
    assert_eq!(inactive.snapshot().live_buffer_bytes, 0);
    assert_eq!(fs::read(fixture.path().join("file")).unwrap(), b"body");
}

#[test]
fn rooted_observation_rootio_bounds_and_changed_framing_are_operational_failures() {
    let fixture = tempfile::tempdir().unwrap();
    fs::write(fixture.path().join("a"), b"a").unwrap();
    fs::write(fixture.path().join("b"), b"b").unwrap();
    let descriptor = File::open(fixture.path()).unwrap();
    let selected = anchor(fixture.path(), 1);
    let narrow = RootIo::new(
        &descriptor,
        RootIoLimits {
            max_entries: 1,
            ..limits()
        },
    )
    .unwrap();
    let resources = scope(SourceServiceLimits::default());
    assert!(matches!(
        observe_rooted(
            &narrow,
            &selected,
            &[demand(
                &selected,
                "",
                PathObservationOperation::DirectoryEntries
            )],
            &resources,
            &|| true
        ),
        Err(RootedObservationError::RootIo(_))
    ));
    assert_eq!(resources.snapshot().live_buffer_bytes, 0);
    let root = RootIo::new(&descriptor, limits()).unwrap();
    let framed = root.lstat(Path::new("a")).unwrap();
    fs::remove_file(fixture.path().join("a")).unwrap();
    symlink("b", fixture.path().join("a")).unwrap();
    assert!(file_bytes(&root, Path::new("a"), &framed, &resources, &|| true).is_err());
    assert_eq!(resources.snapshot().decoded_bytes, 0);
    assert_eq!(resources.snapshot().live_buffer_bytes, 0);
    assert_eq!(fs::read(fixture.path().join("b")).unwrap(), b"b");
}

#[test]
fn rooted_observation_disappearance_after_typed_frame_never_publishes_missing_epoch() {
    let fixture = tempfile::tempdir().unwrap();
    let descriptor = File::open(fixture.path()).unwrap();
    let root = RootIo::new(&descriptor, limits()).unwrap();
    let selected = anchor(fixture.path(), 1);
    // Active is checked immediately after framed lstat. Removing at that exact
    // point deterministically exercises each typed operation's later failure.
    for operation in [
        PathObservationOperation::FileBytes,
        PathObservationOperation::FileDigest,
        PathObservationOperation::ReadLink,
        PathObservationOperation::DirectoryEntries,
    ] {
        let path = fixture.path().join("present");
        match operation {
            PathObservationOperation::ReadLink => symlink("target", &path).unwrap(),
            PathObservationOperation::DirectoryEntries => fs::create_dir(&path).unwrap(),
            _ => fs::write(&path, b"body").unwrap(),
        }
        let calls = Cell::new(0);
        let active = || {
            let call = calls.get();
            calls.set(call + 1);
            if call == 2 {
                if operation == PathObservationOperation::DirectoryEntries {
                    fs::remove_dir(&path).unwrap();
                } else {
                    fs::remove_file(&path).unwrap();
                }
            }
            true
        };
        let resources = scope(SourceServiceLimits::default());
        let observed = observe_rooted(
            &root,
            &selected,
            &[demand(&selected, "present", operation.clone())],
            &resources,
            &active,
        );
        assert!(matches!(observed, Err(RootedObservationError::Unstable)));
        assert_eq!(
            resources.snapshot().source_attempts,
            u64::from(matches!(
                operation,
                PathObservationOperation::FileBytes | PathObservationOperation::FileDigest
            ))
        );
        assert_eq!(resources.snapshot().decoded_bytes, 0);
        assert_eq!(resources.snapshot().live_buffer_bytes, 0);
        assert!(!path.exists());
    }
}
