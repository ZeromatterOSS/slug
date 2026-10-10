use std::ffi::CString;
use std::fmt;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use allocative::Allocative;
use allocative::FlameGraphBuilder;
use async_trait::async_trait;
use dice::DiceComputations;
use dice::DiceEvent;
use dice::DiceEventListener;
use dice::InjectedKey;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use slug_bzlmod_v2::BzlmodCommandPolicyKey;
use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
use slug_bzlmod_v2::LockfileMode;
use slug_configuration_v2::CommandConfigurationOverlay;
use slug_identity_v2::TargetPattern;

use crate::runtime::ProcessHostOwner;
use crate::runtime::WorkspaceRuntime;
use crate::runtime::evaluate_workspace_build_command_with_bzlmod_inputs;
use crate::runtime::probe_observer::DeferredPhase;
use crate::runtime::probe_observer::Observer;
use crate::runtime::probe_observer::Phase;
use crate::runtime::probe_observer::PhaseGuard;
use crate::runtime::probe_observer::ProbeGuard;

const WORDS: usize = 64;
const BYTES: usize = 512;

fn channel() -> (OwnedFd, OwnedFd) {
    let name = CString::new("slug-probe-observer-test").unwrap();
    let raw = unsafe {
        libc::syscall(
            libc::SYS_memfd_create,
            name.as_ptr(),
            libc::MFD_ALLOW_SEALING | libc::MFD_CLOEXEC,
        ) as i32
    };
    assert!(
        raw >= 0,
        "memfd_create: {}",
        std::io::Error::last_os_error()
    );
    let owner = unsafe { OwnedFd::from_raw_fd(raw) };
    assert_eq!(
        unsafe { libc::ftruncate(owner.as_raw_fd(), BYTES as libc::off_t) },
        0
    );
    assert_eq!(
        unsafe {
            libc::fcntl(
                owner.as_raw_fd(),
                libc::F_ADD_SEALS,
                libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL,
            )
        },
        0
    );
    let reader = unsafe { libc::dup(owner.as_raw_fd()) };
    assert!(reader >= 0);
    (owner, unsafe { OwnedFd::from_raw_fd(reader) })
}

fn read_words(fd: &OwnedFd) -> [u64; WORDS] {
    let mut bytes = [0u8; BYTES];
    let read = unsafe { libc::pread(fd.as_raw_fd(), bytes.as_mut_ptr().cast(), bytes.len(), 0) };
    assert_eq!(read, BYTES as isize);
    std::array::from_fn(|index| {
        u64::from_le_bytes(bytes[index * 8..index * 8 + 8].try_into().unwrap())
    })
}

fn assert_rejected_and_closed(fd: OwnedFd) {
    let raw = fd.as_raw_fd();
    assert!(ProbeGuard::install_from_fd(fd).is_err());
    assert_eq!(unsafe { libc::fcntl(raw, libc::F_GETFD) }, -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::EBADF)
    );
}

#[derive(Allocative, Clone, Debug, Eq, Hash, PartialEq)]
struct ProbeInjected;

impl fmt::Display for ProbeInjected {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ProbeInjected")
    }
}

#[async_trait]
impl InjectedKey for ProbeInjected {
    type Value = u8;

    fn equality(left: &Self::Value, right: &Self::Value) -> bool {
        left == right
    }
}

#[derive(Allocative, Clone, Debug, Eq, Hash, PartialEq)]
struct ProbeComputed;

impl fmt::Display for ProbeComputed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ProbeComputed")
    }
}

#[async_trait]
impl Key for ProbeComputed {
    type Value = u8;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        ctx.compute(&ProbeInjected).await.unwrap()
    }

    fn equality(left: &Self::Value, right: &Self::Value) -> bool {
        left == right
    }
}

fn latest_phase_pair(observer: &Observer) -> [(u16, u16); 2] {
    let words = observer.words_for_test();
    let selector = words[7].load(Ordering::Acquire);
    assert!(selector == 1 || selector == 2);
    let latest = (selector - 1) as usize;
    [latest, 1 - latest].map(|slot| {
        let packed = words[14 + slot * 4].load(Ordering::Relaxed);
        ((packed & 0xffff) as u16, ((packed >> 16) & 0xffff) as u16)
    })
}

#[test]
fn fixed_mapping_counts_events_commits_frames_and_releases_last_arc() {
    let (writer, reader) = channel();
    let guard = ProbeGuard::install_from_fd(writer).unwrap();
    let observer = guard.observer();
    let weak = Arc::downgrade(&observer);
    assert_eq!(std::mem::size_of_val(observer.words_for_test()), BYTES);
    assert_eq!(std::mem::size_of_val(&*observer), 8);
    let arc_allocation = std::mem::size_of::<Observer>() + 2 * std::mem::size_of::<usize>();
    assert_eq!(arc_allocation, 24);
    assert_eq!(std::mem::size_of::<Arc<Observer>>(), 8);
    assert_eq!(std::mem::size_of::<ProbeGuard>(), 8);
    assert_eq!(std::mem::size_of::<Option<Arc<Observer>>>(), 8);
    assert_eq!(std::mem::size_of::<PhaseGuard>(), 48);
    assert_eq!(std::mem::size_of::<DeferredPhase>(), 16);
    let control = arc_allocation
        + std::mem::size_of::<ProbeGuard>()
        + 4 * std::mem::size_of::<Option<Arc<Observer>>>()
        + std::mem::size_of::<PhaseGuard>()
        + 3 * std::mem::size_of::<DeferredPhase>()
        + 64;
    assert!(control <= 512);
    assert!(BYTES + control <= 4096);
    #[allow(dead_code)]
    struct BeforePhaseGuard {
        observer: Arc<Observer>,
        phase: Phase,
        armed: bool,
    }
    println!(
        "WP1091_LAYOUT phase_guard={}:{} before_phase_guard={}:{} observer={}:{} probe_guard={}:{} deferred_phase={}:{} mapping_bytes={} control_bytes={}",
        std::mem::size_of::<PhaseGuard>(),
        std::mem::align_of::<PhaseGuard>(),
        std::mem::size_of::<BeforePhaseGuard>(),
        std::mem::align_of::<BeforePhaseGuard>(),
        std::mem::size_of::<Observer>(),
        std::mem::align_of::<Observer>(),
        std::mem::size_of::<ProbeGuard>(),
        std::mem::align_of::<ProbeGuard>(),
        std::mem::size_of::<DeferredPhase>(),
        std::mem::align_of::<DeferredPhase>(),
        BYTES,
        control
    );
    let mut accounting = FlameGraphBuilder::default();
    accounting.visit_root(&*observer);
    let accounting = accounting.finish().flamegraph().write();
    assert!(accounting.contains("mapped_payload 512"), "{accounting}");

    observer.event(DiceEvent::Started {
        key_type: "AKeyWithANameLongEnoughToExerciseTheFixedTagStorageWithoutAllocation",
    });
    observer.event(DiceEvent::Finished { key_type: "AKey" });
    observer.event(DiceEvent::CheckDepsStarted { key_type: "AKey" });
    observer.event(DiceEvent::CheckDepsFinished { key_type: "AKey" });
    observer.event(DiceEvent::ComputeStarted { key_type: "AKey" });
    observer.event(DiceEvent::ComputeFinished { key_type: "AKey" });
    let phase = observer.phase(Phase::RootCompute);
    phase.finish();
    assert_eq!(
        observer.words_for_test()[8..14]
            .iter()
            .map(|word| word.load(Ordering::Relaxed))
            .collect::<Vec<_>>(),
        vec![1; 6]
    );
    assert_ne!(observer.words_for_test()[7].load(Ordering::Acquire), 0);
    assert_ne!(observer.words_for_test()[22].load(Ordering::Acquire), 0);

    drop(guard);
    observer.event(DiceEvent::Started { key_type: "Late" });
    assert_eq!(observer.words_for_test()[8].load(Ordering::Relaxed), 1);
    let words = read_words(&reader);
    assert_eq!(words[0], u64::from_le_bytes(*b"SLGOBS01"));
    assert_eq!(words[1], 2);
    assert_eq!(words[3], 1);
    drop(observer);
    assert!(weak.upgrade().is_none());
}

#[test]
fn invalid_owned_descriptor_is_closed_and_nested_install_is_rejected() {
    let name = CString::new("slug-probe-observer-invalid").unwrap();
    let raw = unsafe {
        libc::syscall(
            libc::SYS_memfd_create,
            name.as_ptr(),
            libc::MFD_ALLOW_SEALING | libc::MFD_CLOEXEC,
        ) as i32
    };
    assert!(raw >= 0);
    let invalid = unsafe { OwnedFd::from_raw_fd(raw) };
    assert_rejected_and_closed(invalid);

    let raw = unsafe {
        libc::syscall(
            libc::SYS_memfd_create,
            name.as_ptr(),
            libc::MFD_ALLOW_SEALING | libc::MFD_CLOEXEC,
        ) as i32
    };
    assert!(raw >= 0);
    assert_eq!(unsafe { libc::ftruncate(raw, BYTES as libc::off_t) }, 0);
    let unsealed = unsafe { OwnedFd::from_raw_fd(raw) };
    assert_rejected_and_closed(unsealed);

    let (read_write, _) = channel();
    let path = CString::new(format!("/proc/self/fd/{}", read_write.as_raw_fd())).unwrap();
    let read_only = unsafe { libc::open(path.as_ptr(), libc::O_RDONLY | libc::O_CLOEXEC) };
    assert!(read_only >= 0);
    assert_rejected_and_closed(unsafe { OwnedFd::from_raw_fd(read_only) });

    let raw = unsafe {
        libc::syscall(
            libc::SYS_memfd_create,
            name.as_ptr(),
            libc::MFD_ALLOW_SEALING | libc::MFD_CLOEXEC,
        ) as i32
    };
    assert!(raw >= 0);
    assert_eq!(unsafe { libc::ftruncate(raw, BYTES as libc::off_t) }, 0);
    assert_eq!(
        unsafe {
            libc::fcntl(
                raw,
                libc::F_ADD_SEALS,
                libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_WRITE | libc::F_SEAL_SEAL,
            )
        },
        0
    );
    assert_rejected_and_closed(unsafe { OwnedFd::from_raw_fd(raw) });

    let (nonzero, _) = channel();
    assert_eq!(
        unsafe { libc::pwrite(nonzero.as_raw_fd(), [1u8].as_ptr().cast(), 1, 0) },
        1
    );
    assert_rejected_and_closed(nonzero);

    let (first, _) = channel();
    let first = ProbeGuard::install_from_fd(first).unwrap();
    let old = first.observer();
    let (second, _) = channel();
    assert!(ProbeGuard::install_from_fd(second).is_err());
    drop(first);
    old.event(DiceEvent::Started {
        key_type: "DisabledOld",
    });
    assert_eq!(old.words_for_test()[8].load(Ordering::Relaxed), 0);
    let (replacement, replacement_reader) = channel();
    let replacement = ProbeGuard::install_from_fd(replacement).unwrap();
    replacement.observer().event(DiceEvent::Started {
        key_type: "Replacement",
    });
    drop(replacement);
    assert_eq!(read_words(&replacement_reader)[8], 1);
}

#[test]
fn counters_overflow_and_contended_sampling_are_explicit() {
    let (writer, _) = channel();
    let guard = ProbeGuard::install_from_fd(writer).unwrap();
    let observer = guard.observer();
    observer.words_for_test()[8].store(u64::MAX, Ordering::Relaxed);
    observer.event(DiceEvent::Started {
        key_type: "Overflow",
    });
    assert_ne!(observer.words_for_test()[4].load(Ordering::Relaxed) & 1, 0);
    observer.words_for_test()[6].store(1, Ordering::Relaxed);
    observer.words_for_test()[9].store(1023, Ordering::Relaxed);
    observer.event(DiceEvent::Finished {
        key_type: "Contended",
    });
    assert_eq!(observer.words_for_test()[5].load(Ordering::Relaxed), 1);
}

#[test]
fn cumulative_phase_intervals_route_repeat_abort_and_preserve_frames() {
    let (writer, reader) = channel();
    let guard = ProbeGuard::install_from_fd(writer).unwrap();
    let observer = guard.observer();
    for phase in [
        Phase::AttemptInjectionRevision,
        Phase::RootCompute,
        Phase::RootCompute,
        Phase::NeedRepository,
        Phase::NeedEnvironment,
        Phase::NeedPath,
        Phase::IdleDrain,
    ] {
        observer
            .phase_with_clocks_for_test(phase, (10, 20, 3))
            .close_with_clocks_for_test((40, 60, 3), phase == Phase::NeedEnvironment);
    }
    let words = read_words(&reader);
    assert_eq!(
        &words[51..63],
        &[1, 30, 40, 2, 60, 80, 3, 90, 120, 1, 30, 40]
    );
    assert_eq!(words[63], (1 << 7) << 24);
    assert_eq!(
        latest_phase_pair(&observer),
        [(Phase::IdleDrain as u16, 2), (Phase::IdleDrain as u16, 1)]
    );
    let latest = 14 + (words[7] as usize - 1) * 4;
    assert_eq!(&words[latest + 1..latest + 4], &[40, 60, 3]);
    assert_eq!(
        &words[14 + (1 - (words[7] as usize - 1)) * 4 + 1..][..3],
        &[10, 20, 3]
    );
    observer
        .phase_with_clocks_for_test(Phase::NativeApi, (1, 1, 3))
        .close_with_clocks_for_test((2, 2, 3), true);
    observer.enter(Phase::RootCompute);
    drop(observer.deferred_entry(Phase::RootCompute));
    drop(observer.deferred_exit(Phase::RootCompute));
    assert_eq!(&read_words(&reader)[51..64], &words[51..64]);
    drop(guard);
}

#[test]
fn cumulative_phase_intervals_qualify_clocks_overflow_and_concurrent_updates() {
    let (writer, reader) = channel();
    let guard = ProbeGuard::install_from_fd(writer).unwrap();
    let observer = guard.observer();
    for (entry, exit) in [
        ((10, 20, 0), (40, 60, 3)),
        ((10, 20, 3), (40, 60, 0)),
        ((10, 20, 3), (9, 19, 3)),
    ] {
        observer
            .phase_with_clocks_for_test(Phase::RootCompute, entry)
            .close_with_clocks_for_test(exit, false);
    }
    let words = read_words(&reader);
    assert_eq!(&words[54..57], &[3, 0, 0]);
    assert_eq!(words[63], 15 << 12);
    observer
        .phase_with_clocks_for_test(Phase::RootCompute, (10, 20, 1))
        .close_with_clocks_for_test((15, 30, 1), false);
    assert_eq!(&read_words(&reader)[54..57], &[4, 5, 0]);
    for component in 0..3 {
        observer.words_for_test()[51 + component].store(u64::MAX, Ordering::Relaxed);
    }
    observer
        .phase_with_clocks_for_test(Phase::AttemptInjectionRevision, (1, 1, 3))
        .close_with_clocks_for_test((2, 2, 3), false);
    let words = read_words(&reader);
    assert_eq!(&words[51..54], &[u64::MAX; 3]);
    assert_eq!(words[63] & 0xfff, (1 << 4) | (1 << 5) | (1 << 6));
    let threads = (0..4)
        .map(|_| {
            let observer = observer.clone();
            std::thread::spawn(move || {
                for _ in 0..1000 {
                    observer.fold_interval_for_test(Phase::IdleDrain, (1, 2, 3), (4, 7, 3));
                }
            })
        })
        .collect::<Vec<_>>();
    for thread in threads {
        thread.join().unwrap();
    }
    let words = read_words(&reader);
    assert_eq!(&words[60..63], &[4000, 12000, 20000]);
    assert_eq!(words[63] >> 36, 0);
    assert_eq!(words[63] & (15 << 12), 15 << 12); // Other categories' sticky flags survive concurrent folds.
    drop(guard);
}

#[test]
fn cumulative_phase_intervals_disabled_closures_preserve_totals_and_last_drop() {
    let (writer, reader) = channel();
    let guard = ProbeGuard::install_from_fd(writer).unwrap();
    let observer = guard.observer();
    let weak = Arc::downgrade(&observer);
    let finished = observer.phase_with_clocks_for_test(Phase::RootCompute, (1, 2, 3));
    let aborted = observer.phase_with_clocks_for_test(Phase::NeedPath, (1, 2, 3));
    let ignored = observer.phase_with_clocks_for_test(Phase::NativeApi, (1, 2, 3));
    drop(guard);
    let before = read_words(&reader);
    finished.close_with_clocks_for_test((3, 4, 3), false);
    drop(aborted); // Real Drop observes disable, so it cannot sample clocks or publish a frame.
    ignored.close_with_clocks_for_test((3, 4, 3), true);
    observer
        .phase_with_clocks_for_test(Phase::IdleDrain, (1, 2, 3))
        .close_with_clocks_for_test((3, 4, 3), false);
    let after = read_words(&reader);
    assert_eq!(&after[..63], &before[..63]);
    assert_eq!(after[63], ((1 << 8) << 12) | (((1 << 8) | (1 << 7)) << 24));
    assert_eq!(Arc::strong_count(&observer), 1);
    drop(observer);
    assert!(weak.upgrade().is_none());
    let (writer, reader) = channel();
    let guard = ProbeGuard::install_from_fd(writer).unwrap();
    let observer = guard.observer();
    drop(guard);
    drop(observer.phase_with_clocks_for_test(Phase::IdleDrain, (1, 2, 3)));
    assert_eq!(&read_words(&reader)[51..64], &[0; 13]);
}

#[test]
fn concurrent_callbacks_and_invalid_clock_and_tag_truncation_are_visible() {
    let (writer, _) = channel();
    let guard = ProbeGuard::install_from_fd(writer).unwrap();
    let observer = guard.observer();
    let threads = (0..4)
        .map(|_| {
            let observer = observer.clone();
            std::thread::spawn(move || {
                for _ in 0..1000 {
                    observer.event(DiceEvent::ComputeFinished {
                        key_type: "Concurrent",
                    });
                }
            })
        })
        .collect::<Vec<_>>();
    for thread in threads {
        thread.join().unwrap();
    }
    assert_eq!(observer.words_for_test()[13].load(Ordering::Relaxed), 4000);

    const LONG_TAG: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789--tail";
    observer.record_activity_for_test(5, LONG_TAG, (0, 0, 0));
    let selector = observer.words_for_test()[22].load(Ordering::Acquire);
    let slot = if selector == 1 { 0 } else { 1 };
    let start = 23 + slot * 14;
    assert_eq!(
        observer.words_for_test()[start + 3].load(Ordering::Relaxed),
        0
    );
    assert_eq!(
        observer.words_for_test()[start + 4].load(Ordering::Relaxed),
        LONG_TAG.len() as u64
    );
    assert_eq!(
        observer.words_for_test()[start + 5].load(Ordering::Relaxed),
        64
    );
}

#[test]
fn guard_drop_during_callback_preserves_listener_mapping_until_final_release() {
    let (writer, reader) = channel();
    let guard = ProbeGuard::install_from_fd(writer).unwrap();
    let observer = guard.observer();
    let weak = Arc::downgrade(&observer);
    crate::runtime::probe_observer::mapping::CALLBACK_GATE.store(1, Ordering::Release);
    let listener = observer.clone();
    let callback = std::thread::spawn(move || {
        listener.event(DiceEvent::Started {
            key_type: "BlockForDisable",
        });
    });
    for _ in 0..100_000 {
        if crate::runtime::probe_observer::mapping::CALLBACK_GATE.load(Ordering::Acquire) == 2 {
            break;
        }
        std::thread::yield_now();
    }
    assert_eq!(
        crate::runtime::probe_observer::mapping::CALLBACK_GATE.load(Ordering::Acquire),
        2
    );
    drop(guard);
    drop(observer);
    assert!(weak.upgrade().is_some());
    crate::runtime::probe_observer::mapping::CALLBACK_GATE.store(3, Ordering::Release);
    callback.join().unwrap();
    assert!(weak.upgrade().is_none());
    let words = read_words(&reader);
    assert_eq!(words[3], 1);
    assert_eq!(words[8], 1);
    crate::runtime::probe_observer::mapping::CALLBACK_GATE.store(0, Ordering::Release);
}

#[test]
fn thread_local_installations_are_isolated_and_preserve_activation_tracker() {
    let worker = std::thread::spawn(|| {
        let (writer, reader) = channel();
        let guard = ProbeGuard::install_from_fd(writer).unwrap();
        guard
            .observer()
            .event(DiceEvent::CheckDepsStarted { key_type: "Worker" });
        drop(guard);
        read_words(&reader)[10]
    });

    let workspace = tempfile::tempdir().unwrap();
    let (writer, _) = channel();
    let guard = ProbeGuard::install_from_fd(writer).unwrap();
    let observer = guard.observer();
    let mut runtime =
        WorkspaceRuntime::new(workspace.path().to_path_buf(), ProcessHostOwner::native()).unwrap();
    runtime.attach_probe_observer(observer.clone());
    runtime.runtime.block_on(async {
        let data = runtime.user_computation_data(None).unwrap();
        assert!(Arc::ptr_eq(
            &data.tracker,
            &(observer.clone() as Arc<dyn DiceEventListener>)
        ));
        assert!(data.activation_tracker.is_some());
        let mut updater = runtime.dice.updater_with_data(data);
        updater.changed_to(vec![(ProbeInjected, 1)]).unwrap();
        let mut transaction = updater.commit().await;
        runtime.request_revision.bind_cycle_detector(&transaction);
        assert_eq!(transaction.compute(&ProbeComputed).await.unwrap(), 1);
        drop(transaction);

        let data = runtime.user_computation_data(None).unwrap();
        assert!(data.activation_tracker.is_some());
        let mut updater = runtime.dice.updater_with_data(data);
        updater.changed_to(vec![(ProbeInjected, 2)]).unwrap();
        let mut transaction = updater.commit().await;
        runtime.request_revision.bind_cycle_detector(&transaction);
        assert_eq!(transaction.compute(&ProbeComputed).await.unwrap(), 2);
    });
    let counters = observer.words_for_test()[8..14]
        .iter()
        .map(|word| word.load(Ordering::Relaxed))
        .collect::<Vec<_>>();
    assert_eq!(counters, [2, 2, 1, 1, 2, 2]);
    assert_eq!(worker.join().unwrap(), 1);
}

#[test]
fn inactive_partial_frame_is_ignored_and_unfinished_phase_commits_aborted() {
    let (writer, _) = channel();
    let guard = ProbeGuard::install_from_fd(writer).unwrap();
    let observer = guard.observer();
    observer.phase(Phase::CommandPreflight).finish();
    let prior_selector = observer.words_for_test()[7].load(Ordering::Acquire);
    let prior_slot = if prior_selector == 1 { 0 } else { 1 };
    let prior = observer.words_for_test()[14 + prior_slot * 4].load(Ordering::Relaxed);
    let inactive_slot = 1 - prior_slot;
    observer.words_for_test()[14 + inactive_slot * 4].store(u64::MAX, Ordering::Relaxed);
    assert_eq!(
        observer.words_for_test()[14 + prior_slot * 4].load(Ordering::Relaxed),
        prior
    );
    assert_eq!(
        observer.words_for_test()[7].load(Ordering::Acquire),
        prior_selector
    );
    {
        let _unfinished = observer.phase(Phase::RootCompute);
    }
    let selector = observer.words_for_test()[7].load(Ordering::Acquire);
    let slot = if selector == 1 { 0 } else { 1 };
    let packed = observer.words_for_test()[14 + slot * 4].load(Ordering::Relaxed);
    assert_eq!(packed >> 16, 3);

    let panic = std::panic::catch_unwind({
        let observer = observer.clone();
        move || {
            let _phase = observer.phase(Phase::AcceptancePreparation);
            panic!("test unwind");
        }
    });
    assert!(panic.is_err());
    let selector = observer.words_for_test()[7].load(Ordering::Acquire);
    let slot = if selector == 1 { 0 } else { 1 };
    let packed = observer.words_for_test()[14 + slot * 4].load(Ordering::Relaxed);
    assert_eq!(packed >> 16, 3);
}

#[test]
fn one_shot_api_teardown_precedes_api_exit_on_success_error_and_unwind() {
    let workspace = tempfile::tempdir().unwrap();
    std::fs::write(
        workspace.path().join("MODULE.bazel"),
        "module(name='probe')\n",
    )
    .unwrap();
    std::fs::write(workspace.path().join("BUILD"), "exports_files([])\n").unwrap();
    let (writer, _) = channel();
    let guard = ProbeGuard::install_from_fd(writer).unwrap();
    let observer = guard.observer();
    let targets = [TargetPattern::parse("//:missing").unwrap()];
    let call = |registries: &[String]| {
        evaluate_workspace_build_command_with_bzlmod_inputs(
            workspace.path(),
            &targets,
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
            registries,
            CommandConfigurationOverlay::default(),
        )
    };
    assert!(call(&[]).is_ok());
    assert_eq!(
        latest_phase_pair(&observer),
        [
            (Phase::NativeApi as u16, 2),
            (Phase::RuntimeTeardown as u16, 2)
        ]
    );

    assert!(call(&["https://example.invalid/%zz".to_owned()]).is_err());
    assert_eq!(
        latest_phase_pair(&observer),
        [
            (Phase::NativeApi as u16, 2),
            (Phase::RuntimeTeardown as u16, 2)
        ]
    );

    crate::runtime::arm_observer_api_panic_for_test();
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| call(&[])));
    assert!(unwind.is_err());
    assert_eq!(
        latest_phase_pair(&observer),
        [
            (Phase::NativeApi as u16, 2),
            (Phase::RuntimeTeardown as u16, 2)
        ]
    );
}
