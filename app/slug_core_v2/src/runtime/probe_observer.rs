#[cfg(not(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_endian = "little",
    target_has_atomic = "64"
)))]
compile_error!("native-probe-observer requires little-endian Linux x86_64 with 64-bit atomics");

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
        let observer = Arc::new(Observer { mapping });
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
}

impl Observer {
    fn words(&self) -> &[std::sync::atomic::AtomicU64; mapping::WORDS] {
        self.mapping.words()
    }

    pub fn phase(self: &Arc<Self>, phase: Phase) -> PhaseGuard {
        self.record_phase(phase, PhaseStatus::Entry);
        PhaseGuard {
            observer: self.clone(),
            phase,
            armed: true,
        }
    }

    pub fn enter(&self, phase: Phase) {
        self.record_phase(phase, PhaseStatus::Entry);
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

    fn record_phase(&self, phase: Phase, status: PhaseStatus) {
        if self.words()[DISABLED].load(Ordering::Acquire) != 0 {
            return;
        }
        let current = self.words()[PHASE_SELECTOR].load(Ordering::Acquire);
        let slot = usize::from(current == 1);
        let start = PHASE_FRAMES + slot * PHASE_FRAME_WORDS;
        let (wall, cpu, valid) = clocks();
        self.words()[start].store((phase as u64) | ((status as u64) << 16), Ordering::Relaxed);
        self.words()[start + 1].store(wall, Ordering::Relaxed);
        self.words()[start + 2].store(cpu, Ordering::Relaxed);
        self.words()[start + 3].store(valid, Ordering::Relaxed);
        self.words()[PHASE_SELECTOR].store((slot + 1) as u64, Ordering::Release);
    }

    fn record_event(&self, kind: usize, tag: &'static str) {
        if self.words()[DISABLED].load(Ordering::Acquire) != 0 {
            return;
        }
        #[cfg(test)]
        mapping::callback_test_hook(tag);
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
}

impl PhaseGuard {
    pub fn finish(mut self) {
        self.observer.record_phase(self.phase, PhaseStatus::Exit);
        self.armed = false;
    }
}

impl Drop for PhaseGuard {
    fn drop(&mut self) {
        if self.armed {
            self.observer.record_phase(self.phase, PhaseStatus::Aborted);
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
        self.observer.record_phase(self.phase, self.status);
    }
}
