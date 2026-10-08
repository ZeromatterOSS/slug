use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::runtime::WorkspaceRuntime;
use crate::runtime::dice::source_service::RegistryRequestAssociation;
use crate::runtime::probe_observer::Observer;
use crate::runtime::probe_observer::Phase;
use crate::runtime::probe_observer::PhaseGuard;
use crate::runtime::probe_observer::ProbeGuard;

pub(super) struct RuntimeActivity {
    guard: ProbeGuard,
    observer: Arc<Observer>,
}

impl RuntimeActivity {
    pub(super) fn install() -> Self {
        let raw = unsafe {
            libc::syscall(
                libc::SYS_memfd_create,
                c"slug-runtime-activity".as_ptr(),
                libc::MFD_ALLOW_SEALING | libc::MFD_CLOEXEC,
            ) as i32
        };
        assert!(
            raw >= 0,
            "memfd_create: {}",
            std::io::Error::last_os_error()
        );
        // The successful syscall transfers the one descriptor to this owner.
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        assert_eq!(unsafe { libc::ftruncate(fd.as_raw_fd(), 512) }, 0);
        assert_eq!(
            unsafe {
                libc::fcntl(
                    fd.as_raw_fd(),
                    libc::F_ADD_SEALS,
                    libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL,
                )
            },
            0
        );
        let guard = ProbeGuard::install_from_fd(fd).unwrap();
        let observer = guard.observer();
        Self { guard, observer }
    }

    pub(super) fn construction(&self) -> PhaseGuard {
        self.observer.phase(Phase::RuntimeConstruction)
    }

    pub(super) fn attach(&self, runtime: &mut WorkspaceRuntime) {
        runtime.attach_probe_observer(self.observer.clone());
    }

    pub(super) fn listeners_released(&self) -> bool {
        // Only the local owner, install guard and thread-local install remain.
        Arc::strong_count(&self.observer) == 3
    }

    pub(super) fn report_and_retire(self) {
        assert!(
            self.listeners_released(),
            "runtime observer listeners must retire"
        );
        let Self { guard, observer } = self;
        let weak = Arc::downgrade(&observer);
        drop(guard);
        assert_eq!(Arc::strong_count(&observer), 1);
        assert_eq!(observer.words_for_test()[3].load(Ordering::Acquire), 1);
        assert_eq!(observer.words_for_test()[6].load(Ordering::Acquire), 0);
        // No listener or phase writer remains. Reading the existing atomic mapping
        // preserves its committed frames, overflow and lossy sampling indicators.
        print!("WP1008_OBSERVER words=");
        for (index, word) in observer.words_for_test().iter().enumerate() {
            if index != 0 {
                print!(",");
            }
            print!("{}", word.load(Ordering::Acquire));
        }
        println!();
        drop(observer);
        assert!(weak.upgrade().is_none());
    }
}

pub(super) fn report_timeout(recorder: Option<&RegistryRequestAssociation>) {
    let Some(recorder) = recorder else {
        println!("WP1008_TIMEOUT status=unavailable");
        return;
    };
    let timeout = recorder.snapshot().timeout;
    // snapshot returned: no recorder or Scope lock is held while formatting.
    let origin = timeout.origin;
    println!(
        "WP1008_TIMEOUT status=available armed={} first_work={} origin={} site={} domain={} phase={} return={} overflow={} sticky={}",
        timeout.armed,
        timeout.first_error_is_work_deadline,
        origin.is_some(),
        origin.map_or(0, |o| o.site.code()),
        origin.map_or(0, |o| o.domain.code()),
        origin.map_or(0, |o| o.phase.code()),
        origin.map_or(0, |o| o.return_disposition.code()),
        origin.is_some_and(|o| o.step_add_overflow),
        timeout.later_sticky_work_deadline_observed,
    );
}
