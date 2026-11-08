use std::os::fd::AsRawFd;

use super::super::tests::resources;
use super::super::tests::scope;
use super::*;

#[test]
fn mint_rejects_host_directory_before_clone_and_releases_all_credits() {
    let fixture = tempfile::tempdir().unwrap();
    let root = File::open(fixture.path()).unwrap();
    let resources = resources();
    let baseline = resources.snapshot().retained_bytes;
    let scope = scope();
    assert!(matches!(
        clone_from_attached_root(&root, fixture.path(), &resources, &scope, &|| true),
        Err(MountLeaseError::AttachedRoot)
    ));
    assert_eq!(
        (
            resources.snapshot().retained_bytes,
            resources.snapshot().native_fds,
            resources.snapshot().backing_bytes
        ),
        (baseline, 0, 0)
    );
}

#[test]
fn cancelled_or_inactive_setup_closes_consumed_input_before_any_allocation() {
    let fixture = tempfile::tempdir().unwrap();
    for cancelled in [false, true] {
        let input = File::open(fixture.path()).unwrap();
        let raw = input.as_raw_fd();
        let owner = File::open(fixture.path()).unwrap();
        let resources = resources();
        let baseline = resources.snapshot().retained_bytes;
        let scope = scope();
        if cancelled {
            scope.cancel();
        }
        let result =
            attach_in_owning_namespace(input, &owner, fixture.path(), &resources, &scope, &|| {
                false
            });
        if cancelled {
            assert!(matches!(
                result,
                Err(MountLeaseError::Root(PhysicalRootError::SourceService(
                    SourceServiceError::Cancelled
                )))
            ));
        } else {
            assert!(matches!(
                result,
                Err(MountLeaseError::Root(PhysicalRootError::Inactive))
            ));
        }
        assert_eq!(unsafe { libc::fcntl(raw, libc::F_GETFD) }, -1);
        assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::EBADF));
        assert_eq!(
            (
                resources.snapshot().retained_bytes,
                resources.snapshot().native_fds
            ),
            (baseline, 0)
        );
    }
}
