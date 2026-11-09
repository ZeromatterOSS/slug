#[cfg(not(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_endian = "little",
    target_has_atomic = "64"
)))]
compile_error!("native-probe-observer requires little-endian Linux x86_64 with 64-bit atomics");

#[cfg(test)]
pub(crate) mod family_counts;
pub(crate) mod mapping;

use std::cell::RefCell;
use std::io;
use std::marker::PhantomData;
use std::os::fd::OwnedFd;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use allocative::Allocative;
use allocative::Key;
use allocative::Visitor;
use dice::DiceEvent;
use dice::DiceEventListener;

use self::mapping::*;

const SAMPLE_INTERVAL: u64 = 1024;

thread_local! {
    static ACTIVE: RefCell<Option<Arc<Observer>>> = const { RefCell::new(None) };
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum Phase {
    NativeApi = 1,
    RuntimeConstruction,
    RequestConfiguration,
    CommandPreflight,
    AttemptInjectionRevision,
    RootCompute,
    TerminalSelection,
    AcceptancePreparation,
    RevisionFinalization,
    TransactionCleanup,
    FinalAcceptance,
    NeedRepository,
    NeedEnvironment,
    NeedPath,
    RuntimeTeardown,
    DiagnosticProjection,
    EventPublication,
    TerminalRelease,
    IdleDrain,
}

impl Phase {
    fn interval_category(self) -> Option<usize> {
        match self {
            Self::AttemptInjectionRevision => Some(0),
            Self::RootCompute => Some(1),
            Self::NeedRepository | Self::NeedEnvironment | Self::NeedPath => Some(2),
            Self::IdleDrain => Some(3),
            _ => None,
        }
    }
}

pub struct ProbeGuard {
    observer: Arc<Observer>,
    _not_send: PhantomData<Rc<()>>,
}

impl ProbeGuard {
    pub fn install_from_fd(fd: OwnedFd) -> io::Result<Self> {
        if ACTIVE.with(|active| active.borrow().is_some()) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "native probe observer is already installed on this thread",
            ));
        }
        let mapping = Mapping::from_fd(fd)?;
        let observer = Arc::new(Observer {
            mapping,
            #[cfg(test)]
            family_counts: family_counts::FamilyCounts::default(),
        });
        observer.words()[0].store(MAGIC, Ordering::Relaxed);
        observer.words()[1].store(VERSION, Ordering::Relaxed);
        observer.words()[2].store(std::process::id() as u64, Ordering::Relaxed);
        ACTIVE.with(|active| {
            let mut active = active.borrow_mut();
            assert!(active.is_none(), "thread-local install was checked above");
            *active = Some(observer.clone());
        });
        Ok(Self {
            observer,
            _not_send: PhantomData,
        })
    }

    pub fn observer(&self) -> Arc<Observer> {
        self.observer.clone()
    }
}

impl Drop for ProbeGuard {
    fn drop(&mut self) {
        self.observer.words()[DISABLED].store(1, Ordering::Release);
        ACTIVE.with(|active| {
            let mut active = active.borrow_mut();
            if active
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, &self.observer))
            {
                active.take();
            }
        });
    }
}

pub fn capture() -> Option<Arc<Observer>> {
    ACTIVE.with(|active| {
        active
            .borrow()
            .as_ref()
            .filter(|observer| observer.words()[DISABLED].load(Ordering::Acquire) == 0)
            .cloned()
    })
}

pub struct Observer {
    mapping: Mapping,
    #[cfg(test)]
    family_counts: family_counts::FamilyCounts,
}

impl Observer {
    fn words(&self) -> &[std::sync::atomic::AtomicU64; mapping::WORDS] {
        self.mapping.words()
    }

    pub fn phase(self: &Arc<Self>, phase: Phase) -> PhaseGuard {
        let entry = self.record_phase(phase, PhaseStatus::Entry);
        self.phase_guard(phase, entry)
    }

    fn phase_guard(self: &Arc<Self>, phase: Phase, entry: Option<(u64, u64, u64)>) -> PhaseGuard {
        PhaseGuard {
            observer: self.clone(),
            phase,
            armed: true,
            entry: if phase.interval_category().is_some() {
                entry
            } else {
                None
            },
        }
    }

    pub fn enter(&self, phase: Phase) {
        let _ = self.record_phase(phase, PhaseStatus::Entry);
    }

    pub fn deferred_entry(self: &Arc<Self>, phase: Phase) -> DeferredPhase {
        DeferredPhase {
            observer: self.clone(),
            phase,
            status: PhaseStatus::Entry,
        }
    }

    pub fn deferred_exit(self: &Arc<Self>, phase: Phase) -> DeferredPhase {
        DeferredPhase {
            observer: self.clone(),
            phase,
            status: PhaseStatus::Exit,
        }
    }

    fn record_phase(&self, phase: Phase, status: PhaseStatus) -> Option<(u64, u64, u64)> {
        if self.words()[DISABLED].load(Ordering::Acquire) != 0 {
            return None;
        }
        self.record_phase_with_clocks(phase, status, clocks())
    }

    fn record_phase_with_clocks(
        &self,
        phase: Phase,
        status: PhaseStatus,
        sample: (u64, u64, u64),
    ) -> Option<(u64, u64, u64)> {
        if self.words()[DISABLED].load(Ordering::Acquire) != 0 {
            return None;
        }
        let current = self.words()[PHASE_SELECTOR].load(Ordering::Acquire);
        let slot = usize::from(current == 1);
        let start = PHASE_FRAMES + slot * PHASE_FRAME_WORDS;
        let (wall, cpu, valid) = sample;
        self.words()[start].store((phase as u64) | ((status as u64) << 16), Ordering::Relaxed);
        self.words()[start + 1].store(wall, Ordering::Relaxed);
        self.words()[start + 2].store(cpu, Ordering::Relaxed);
        self.words()[start + 3].store(valid, Ordering::Relaxed);
        self.words()[PHASE_SELECTOR].store((slot + 1) as u64, Ordering::Release);
        Some(sample)
    }

    fn qualify_interval(&self, category: usize, flags: u64) {
        self.words()[INTERVAL_STATUS].fetch_or(
            flags << (category * INTERVAL_STATUS_STRIDE),
            Ordering::Relaxed,
        );
    }

    fn add_interval(&self, category: usize, component: usize, amount: u64, overflow: u64) {
        let cell = &self.words()[INTERVAL_TOTALS + category * 3 + component];
        let mut failure = 0;
        let result = cell.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |old| {
            if self.words()[DISABLED].load(Ordering::Acquire) != 0 {
                failure = SUPPRESSED_INTERVAL;
                None
            } else {
                old.checked_add(amount).or_else(|| {
                    failure = overflow;
                    None
                })
            }
        });
        if result.is_err() {
            self.qualify_interval(category, failure);
        }
    }

    fn fold_interval(
        &self,
        phase: Phase,
        entry: Option<(u64, u64, u64)>,
        exit: Option<(u64, u64, u64)>,
        status: PhaseStatus,
    ) {
        let (Some(category), Some(entry)) = (phase.interval_category(), entry) else {
            return;
        };
        let aborted = if matches!(status, PhaseStatus::Aborted) {
            ABORTED_INTERVAL
        } else {
            0
        };
        if self.words()[DISABLED].load(Ordering::Acquire) != 0 || exit.is_none() {
            self.qualify_interval(category, aborted | SUPPRESSED_INTERVAL);
            return;
        }
        let exit = exit.expect("enabled phase closure supplied clocks");
        self.qualify_interval(category, aborted);
        self.add_interval(category, 0, 1, COUNT_OVERFLOW);
        for (component, start, end, valid_bit, missing, reversed, overflow) in [
            (
                1,
                entry.0,
                exit.0,
                1,
                MISSING_WALL,
                REVERSED_WALL,
                WALL_OVERFLOW,
            ),
            (
                2,
                entry.1,
                exit.1,
                2,
                MISSING_CPU,
                REVERSED_CPU,
                CPU_OVERFLOW,
            ),
        ] {
            if entry.2 & exit.2 & valid_bit == 0 {
                self.qualify_interval(category, missing);
            } else if let Some(delta) = end.checked_sub(start) {
                self.add_interval(category, component, delta, overflow);
            } else {
                self.qualify_interval(category, reversed);
            }
        }
        if self.words()[DISABLED].load(Ordering::Acquire) != 0 {
            self.qualify_interval(category, SUPPRESSED_INTERVAL);
        }
    }

    #[cfg(test)]
    pub(crate) fn phase_with_clocks_for_test(
        self: &Arc<Self>,
        phase: Phase,
        sample: (u64, u64, u64),
    ) -> PhaseGuard {
        let entry = self.record_phase_with_clocks(phase, PhaseStatus::Entry, sample);
        self.phase_guard(phase, entry)
    }

    #[cfg(test)]
    pub(crate) fn fold_interval_for_test(
        &self,
        phase: Phase,
        entry: (u64, u64, u64),
        exit: (u64, u64, u64),
    ) {
        self.fold_interval(phase, Some(entry), Some(exit), PhaseStatus::Exit);
    }

    fn record_event(&self, kind: usize, tag: &'static str) {
        if self.words()[DISABLED].load(Ordering::Acquire) != 0 {
            return;
        }
        #[cfg(test)]
        mapping::callback_test_hook(tag);
        #[cfg(test)]
        self.family_counts.record(kind, tag);
        let old = self.words()[COUNTERS + kind].fetch_add(1, Ordering::Relaxed);
        if old == u64::MAX {
            self.words()[OVERFLOW].fetch_or(1 << kind, Ordering::Relaxed);
        }
        let count = old.wrapping_add(1);
        if old != 0 && count % SAMPLE_INTERVAL != 0 {
            return;
        }
        if self.words()[ACTIVITY_CLAIM]
            .compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            let dropped = self.words()[DROPPED].fetch_add(1, Ordering::Relaxed);
            if dropped == u64::MAX {
                self.words()[OVERFLOW].fetch_or(1 << 63, Ordering::Relaxed);
            }
            return;
        }
        if self.words()[DISABLED].load(Ordering::Acquire) == 0 {
            self.record_activity(kind, tag);
        }
        self.words()[ACTIVITY_CLAIM].store(0, Ordering::Release);
    }

    #[cfg(test)]
    pub(crate) fn words_for_test(&self) -> &[std::sync::atomic::AtomicU64; mapping::WORDS] {
        self.words()
    }

    /// Per-cell observations, not an atomic snapshot or callback-drain barrier.
    #[cfg(test)]
    pub(crate) fn family_counts_for_test(&self) -> family_counts::Snapshot {
        self.family_counts.snapshot()
    }

    #[cfg(test)]
    pub(crate) fn record_activity_for_test(
        &self,
        kind: usize,
        tag: &'static str,
        clocks: (u64, u64, u64),
    ) {
        self.record_activity_with_clocks(kind, tag, clocks);
    }

    fn record_activity(&self, kind: usize, tag: &'static str) {
        self.record_activity_with_clocks(kind, tag, clocks());
    }

    fn record_activity_with_clocks(
        &self,
        kind: usize,
        tag: &'static str,
        (wall, cpu, valid): (u64, u64, u64),
    ) {
        let current = self.words()[ACTIVITY_SELECTOR].load(Ordering::Acquire);
        let slot = usize::from(current == 1);
        let start = ACTIVITY_FRAMES + slot * ACTIVITY_FRAME_WORDS;
        let bytes = tag.as_bytes();
        let copied = bytes.len().min(64);
        self.words()[start].store(kind as u64, Ordering::Relaxed);
        self.words()[start + 1].store(wall, Ordering::Relaxed);
        self.words()[start + 2].store(cpu, Ordering::Relaxed);
        self.words()[start + 3].store(valid, Ordering::Relaxed);
        self.words()[start + 4].store(bytes.len() as u64, Ordering::Relaxed);
        self.words()[start + 5].store(copied as u64, Ordering::Relaxed);
        for (index, chunk) in bytes[..copied].chunks(8).enumerate() {
            let mut word = [0u8; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            self.words()[start + 6 + index].store(u64::from_le_bytes(word), Ordering::Relaxed);
        }
        for index in copied.div_ceil(8)..8 {
            self.words()[start + 6 + index].store(0, Ordering::Relaxed);
        }
        self.words()[ACTIVITY_SELECTOR].store((slot + 1) as u64, Ordering::Release);
    }
}

impl DiceEventListener for Observer {
    fn event(&self, event: DiceEvent) {
        let (kind, tag) = match event {
            DiceEvent::Started { key_type } => (0, key_type),
            DiceEvent::Finished { key_type } => (1, key_type),
            DiceEvent::CheckDepsStarted { key_type } => (2, key_type),
            DiceEvent::CheckDepsFinished { key_type } => (3, key_type),
            DiceEvent::ComputeStarted { key_type } => (4, key_type),
            DiceEvent::ComputeFinished { key_type } => (5, key_type),
        };
        self.record_event(kind, tag);
    }
}

impl Allocative for Observer {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        visitor.visit_simple(Key::new("mapped_payload"), BYTES);
        visitor.exit();
    }
}

pub struct PhaseGuard {
    observer: Arc<Observer>,
    phase: Phase,
    armed: bool,
    entry: Option<(u64, u64, u64)>,
}

impl PhaseGuard {
    pub fn finish(mut self) {
        let exit = self.observer.record_phase(self.phase, PhaseStatus::Exit);
        self.observer
            .fold_interval(self.phase, self.entry, exit, PhaseStatus::Exit);
        self.armed = false;
    }

    #[cfg(test)]
    pub(crate) fn close_with_clocks_for_test(mut self, sample: (u64, u64, u64), aborted: bool) {
        let status = if aborted {
            PhaseStatus::Aborted
        } else {
            PhaseStatus::Exit
        };
        let exit = self
            .observer
            .record_phase_with_clocks(self.phase, status, sample);
        self.observer
            .fold_interval(self.phase, self.entry, exit, status);
        self.armed = false;
    }
}

impl Drop for PhaseGuard {
    fn drop(&mut self) {
        if self.armed {
            let exit = self.observer.record_phase(self.phase, PhaseStatus::Aborted);
            self.observer
                .fold_interval(self.phase, self.entry, exit, PhaseStatus::Aborted);
        }
    }
}

pub struct DeferredPhase {
    observer: Arc<Observer>,
    phase: Phase,
    status: PhaseStatus,
}

impl Drop for DeferredPhase {
    fn drop(&mut self) {
        let _ = self.observer.record_phase(self.phase, self.status);
    }
}
