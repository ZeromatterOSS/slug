use std::time::Duration;
use std::time::Instant;

use super::*;
use crate::runtime::dice::source_service::SourceServiceLimits;

fn test_scope() -> Arc<SourceServiceScope> {
    let deadline = Instant::now() + Duration::from_secs(2);
    SourceServiceScope::new_for_test(
        NormalizedAbsolutePath::new("/fixture").unwrap(),
        deadline,
        deadline + Duration::from_secs(1),
        SourceServiceLimits::default(),
    )
    .unwrap()
}

fn demand() -> PathObservationDemand {
    PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new("/fixture").unwrap(),
        PathObservationOperation::DirectoryEntries,
    )
}

enum Step {
    Entry(Vec<u8>, u8),
    Null(i32),
}

#[derive(Clone, Copy)]
enum Stop {
    Open(usize),
    Read(usize),
    Kind(usize),
    Close,
}

struct Api {
    scope: Arc<SourceServiceScope>,
    steps: Vec<Step>,
    open_errors: Vec<i32>,
    kinds: Vec<Result<PathNodeKind, i32>>,
    stop: Option<Stop>,
    opens: usize,
    reads: usize,
    stats: usize,
    closes: usize,
    close_error: Option<i32>,
}

impl Api {
    fn new(scope: &Arc<SourceServiceScope>, steps: Vec<Step>) -> Self {
        Self {
            scope: scope.clone(),
            steps,
            open_errors: Vec::new(),
            kinds: Vec::new(),
            stop: None,
            opens: 0,
            reads: 0,
            stats: 0,
            closes: 0,
            close_error: None,
        }
    }

    fn stop(&self, condition: bool) {
        if condition {
            self.scope.poison(SourceServiceError::WorkDeadline);
        }
    }
}

impl UnixDirectoryApi for Api {
    type Handle = u8;

    fn open_once(&mut self, _: &CStr) -> Result<u8, i32> {
        self.opens += 1;
        self.stop(matches!(self.stop, Some(Stop::Open(n)) if n == self.opens));
        match self.open_errors.get(self.opens - 1) {
            Some(raw) => Err(*raw),
            None => Ok(7),
        }
    }
    fn clear_errno(&mut self) {}
    fn read_once(&mut self, _: u8) -> UnixDirectoryRead {
        panic!("scoped observation must not call the copying ordinary reader")
    }
    fn close_once(&mut self, handle: u8) -> Result<(), i32> {
        assert_eq!(handle, 7);
        self.closes += 1;
        self.stop(matches!(self.stop, Some(Stop::Close)));
        self.close_error.map_or(Ok(()), Err)
    }
}

impl ScopedApi for Api {
    fn read_borrowed(&mut self, handle: u8) -> Result<Read<'_>, Failure> {
        assert_eq!(handle, 7);
        self.reads += 1;
        self.stop(matches!(self.stop, Some(Stop::Read(n)) if n == self.reads));
        Ok(match self.steps.get(self.reads - 1) {
            Some(Step::Entry(name, native_type)) => Read::Entry {
                name,
                native_type: *native_type,
            },
            Some(Step::Null(raw)) => Read::Null(*raw),
            None => Read::Null(0),
        })
    }
    fn kind_once(&mut self, _: &Path) -> Result<PathNodeKind, i32> {
        self.stats += 1;
        self.stop(matches!(self.stop, Some(Stop::Kind(n)) if n == self.stats));
        self.kinds
            .get(self.stats - 1)
            .copied()
            .unwrap_or(Ok(PathNodeKind::RegularFile))
    }
}

fn names(count: usize, length: usize, kind: u8) -> Vec<Step> {
    (0..count)
        .map(|index| {
            let mut name = vec![b'x'; length];
            name[..3].copy_from_slice(format!("{index:03}").as_bytes());
            Step::Entry(name, kind)
        })
        .collect()
}

fn present(result: &PathObservationResult) -> &PathDirectoryEntries {
    match result {
        PathObservationResult::DirectoryEntries(PathOperationResult::Present(entries)) => entries,
        result => panic!("unexpected result {result:?}"),
    }
}

#[test]
fn scoped_directory_inclusive_bounds_preserve_names_and_kinds() {
    for (count, length) in [(128, 128), (16, 1024)] {
        let scope = test_scope();
        let mut steps = vec![
            Step::Entry(b".".to_vec(), 0),
            Step::Entry(b"..".to_vec(), 0),
        ];
        steps.extend(names(count, length, nix::libc::DT_REG));
        let mut api = Api::new(&scope, steps);
        let result = observe_with(&demand(), &scope, &mut api).unwrap();
        let entries = present(&result).entries();
        assert_eq!(entries.len(), count);
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.name().as_os_str().as_bytes().len())
                .sum::<usize>(),
            NAMES_LIMIT
        );
        assert!(
            entries
                .iter()
                .all(|entry| entry.kind() == PathDirectoryEntryKind::File)
        );
        assert_eq!((api.reads, api.stats, api.closes), (count + 3, 0, 1));
        assert_eq!(scope.snapshot().source_attempts, 0);
        assert_eq!(scope.snapshot().decoded_bytes, 0);
    }
    let scope = test_scope();
    let mut api = Api::new(
        &scope,
        vec![
            Step::Entry(vec![b'z', 0xff], nix::libc::DT_LNK),
            Step::Entry(b"a".to_vec(), nix::libc::DT_UNKNOWN),
        ],
    );
    api.kinds = vec![Ok(PathNodeKind::Directory)];
    let result = observe_with(&demand(), &scope, &mut api).unwrap();
    let entries = present(&result).entries();
    assert_eq!(entries[0].name().as_os_str().as_bytes(), b"a");
    assert_eq!(entries[0].kind(), PathDirectoryEntryKind::Directory);
    assert_eq!(entries[1].name().as_os_str().as_bytes(), &[b'z', 0xff]);
    assert_eq!(entries[1].kind(), PathDirectoryEntryKind::Symlink);
    assert_eq!((api.stats, api.closes), (1, 1));
}

#[test]
fn scoped_directory_refuses_before_copy_classification_and_closes_once() {
    // This is the scanner called on the native d_name record, before to_vec.
    let mut raw = vec![b'x'; NAME_LIMIT + 2];
    raw[NAME_LIMIT] = 0;
    let name = native_name(&raw).unwrap_or_else(|_| panic!("inclusive native name"));
    assert_eq!(name.len(), NAME_LIMIT);
    assert_eq!(name.as_ptr(), raw.as_ptr());
    raw[NAME_LIMIT] = b'x';
    assert!(matches!(
        native_name(&raw),
        Err(Failure::Operational(
            PathObservationKernelError::DirectoryLimit(DirectoryLimit::NameBytes)
        ))
    ));
    assert!(matches!(
        native_name(b"unterminated"),
        Err(Failure::Semantic(_))
    ));
    for (steps, reason) in [
        (names(129, 3, 0), DirectoryLimit::Entries),
        (names(1, 1025, 0), DirectoryLimit::NameBytes),
        (names(17, 1024, 0), DirectoryLimit::TotalNameBytes),
    ] {
        let scope = test_scope();
        let mut api = Api::new(&scope, steps);
        assert_eq!(
            observe_with(&demand(), &scope, &mut api),
            Err(PathObservationKernelError::DirectoryLimit(reason))
        );
        assert_eq!(api.stats, 0);
        assert_eq!(api.closes, 1);
        assert_eq!(scope.snapshot().decoded_bytes, 0);
        assert_eq!(scope.snapshot().source_attempts, 0);
    }
    let scope = test_scope();
    let mut api = Api::new(&scope, vec![Step::Entry(b"bad/name".to_vec(), 0)]);
    assert_eq!(
        observe_with(&demand(), &scope, &mut api),
        Ok(PathObservationResult::DirectoryEntries(
            PathOperationResult::Error(match invalid_directory_data() {
                PrimaryFailure::Final(error) => error,
                _ => unreachable!(),
            })
        ))
    );
    assert_eq!((api.stats, api.closes), (0, 1));
    let mut api = Api::new(&scope, vec![]);
    {
        let _owner = UnixDirectoryOwner::new(7, &mut api);
    }
    assert_eq!(api.closes, 1);
}

#[test]
fn scoped_directory_retries_check_scope_and_preserve_error_polarity() {
    let owner = test_scope();
    let mut api = Api::new(
        &owner,
        vec![
            Step::Null(nix::libc::EINTR),
            Step::Null(nix::libc::EIO),
            Step::Entry(b"after-retry".to_vec(), nix::libc::DT_REG),
        ],
    );
    let result = observe_with(&demand(), &owner, &mut api).unwrap();
    assert_eq!(present(&result).entries().len(), 1);
    assert_eq!((api.reads, api.closes), (4, 1));
    // Successful open followed by refusal still arms and closes its handle.
    let owner = test_scope();
    let mut api = Api::new(&owner, vec![]);
    api.stop = Some(Stop::Open(1));
    assert_eq!(
        observe_with(&demand(), &owner, &mut api),
        Err(PathObservationKernelError::SourceService(
            SourceServiceError::WorkDeadline
        ))
    );
    assert_eq!((api.opens, api.reads, api.closes), (1, 0, 1));
    // Poison at the syscall seam, so no clock sleeps or unbounded retry is needed.
    for stop in [Stop::Open(2), Stop::Read(2), Stop::Kind(2), Stop::Close] {
        let scope = test_scope();
        let mut api = Api::new(
            &scope,
            vec![Step::Null(nix::libc::EINTR), Step::Null(nix::libc::EIO)],
        );
        api.stop = Some(stop);
        match stop {
            Stop::Open(_) => api.open_errors = vec![nix::libc::EINTR, nix::libc::EINTR],
            Stop::Kind(_) => {
                api.steps = vec![Step::Entry(b"unknown".to_vec(), 0)];
                api.kinds = vec![Err(nix::libc::EINTR), Err(nix::libc::EINTR)];
            }
            Stop::Close => api.steps.clear(),
            _ => {}
        }
        assert_eq!(
            observe_with(&demand(), &scope, &mut api),
            Err(PathObservationKernelError::SourceService(
                SourceServiceError::WorkDeadline
            ))
        );
        assert_eq!(
            api.closes,
            if matches!(stop, Stop::Open(_)) { 0 } else { 1 }
        );
        assert!(api.opens <= 2 && api.reads <= 3 && api.stats <= 2);
    }
    // Missing/not-directory refinement has its own scope-checked metadata loop.
    let scope = test_scope();
    let mut api = Api::new(&scope, vec![]);
    api.open_errors = vec![nix::libc::ENOENT];
    api.kinds = vec![Err(nix::libc::EINTR), Err(nix::libc::EINTR)];
    api.stop = Some(Stop::Kind(2));
    assert_eq!(
        observe_with(&demand(), &scope, &mut api),
        Err(PathObservationKernelError::SourceService(
            SourceServiceError::WorkDeadline
        ))
    );
    assert_eq!((api.stats, api.closes), (2, 0));
    for (kind, expected) in [
        (Err(nix::libc::ENOENT), PathOperationResult::Missing),
        (Err(nix::libc::ENOTDIR), PathOperationResult::Missing),
        (
            Ok(PathNodeKind::RegularFile),
            PathOperationResult::Error(PathObservationError::WrongKind {
                expected: PathNodeKind::Directory,
                actual: PathNodeKind::RegularFile,
            }),
        ),
        (
            Ok(PathNodeKind::Directory),
            PathOperationResult::Error(raw_path_io_error(nix::libc::ENOENT)),
        ),
        (
            Err(nix::libc::EACCES),
            PathOperationResult::Error(raw_path_io_error(nix::libc::ENOENT)),
        ),
    ] {
        let scope = test_scope();
        let mut api = Api::new(&scope, vec![]);
        api.open_errors = vec![nix::libc::ENOENT];
        api.kinds = vec![kind];
        assert_eq!(
            observe_with(&demand(), &scope, &mut api),
            Ok(PathObservationResult::DirectoryEntries(expected))
        );
        assert_eq!((api.stats, api.closes), (1, 0));
    }
    for raw in [nix::libc::EACCES, nix::libc::EINTR] {
        let scope = test_scope();
        let mut api = Api::new(&scope, vec![]);
        api.close_error = Some(raw);
        let result = observe_with(&demand(), &scope, &mut api);
        if raw == nix::libc::EINTR {
            assert!(result.is_ok());
        } else {
            assert_eq!(
                result,
                Ok(PathObservationResult::DirectoryEntries(
                    PathOperationResult::Error(raw_path_io_error(raw))
                ))
            );
        }
        assert_eq!(api.closes, 1);
    }
}

#[test]
fn scoped_directory_native_epoch_is_fresh_and_operational_refusal_has_no_partial_epoch() {
    let directory = tempfile::tempdir().unwrap();
    let root = NormalizedAbsolutePath::new(directory.path().to_path_buf()).unwrap();
    let directory_demand = PathObservationDemand::new(
        PathObservationNamespace::Host,
        root.clone(),
        PathObservationOperation::DirectoryEntries,
    );
    let owner = test_scope();
    let scope_weak = Arc::downgrade(&owner);
    let first =
        observe_native_scoped(&directory, [], [directory_demand.clone()], Some(&owner)).unwrap();
    drop(owner);
    assert!(scope_weak.upgrade().is_none());
    assert_eq!(
        first,
        observe_native(&directory, [], [directory_demand.clone()]).unwrap()
    );
    let carrier = first.get(&directory_demand).unwrap();
    let weak = Arc::downgrade(carrier);
    let retained = carrier.clone();
    assert!(Arc::ptr_eq(carrier, &retained));
    std::fs::write(directory.path().join("b"), b"b").unwrap();
    std::fs::create_dir(directory.path().join("a")).unwrap();
    let second = observe_native_scoped(
        &directory,
        [],
        [directory_demand.clone()],
        Some(&test_scope()),
    )
    .unwrap();
    assert_ne!(first, second);
    assert_eq!(
        second,
        observe_native(&directory, [], [directory_demand.clone()]).unwrap()
    );
    std::fs::remove_dir(directory.path().join("a")).unwrap();
    std::fs::remove_file(directory.path().join("b")).unwrap();
    let restored = observe_native_scoped(
        &directory,
        [],
        [directory_demand.clone()],
        Some(&test_scope()),
    )
    .unwrap();
    assert_eq!(first, restored);
    drop(first);
    assert!(weak.upgrade().is_some());
    drop(retained);
    assert!(weak.upgrade().is_none());
    for index in 0..=ENTRY_LIMIT {
        std::fs::write(directory.path().join(format!("{index:03}")), b"").unwrap();
    }
    let lstat = PathObservationDemand::new(
        PathObservationNamespace::Host,
        root,
        PathObservationOperation::Lstat,
    );
    assert_eq!(
        observe_native_scoped(
            &directory,
            [],
            [lstat, directory_demand],
            Some(&test_scope())
        ),
        Err(PathObservationKernelError::DirectoryLimit(
            DirectoryLimit::Entries
        ))
    );
    // Err carries no epoch, including the already accumulated lstat observation.
}
