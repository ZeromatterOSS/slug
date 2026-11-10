//! Scoped comparison-call argument counts, not CPU or unique-byte accounting.
use std::cell::Cell;
use std::marker::PhantomData;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Snapshot {
    pub(crate) comparison_bytes: u64,
    pub(crate) comparison_calls: u64,
}

std::thread_local! {
    static WORK: Cell<Option<Snapshot>> = const { Cell::new(None) };
}

pub(crate) struct WorkGuard {
    _thread: PhantomData<Rc<()>>,
}

pub(crate) fn begin() -> WorkGuard {
    WORK.with(|work| {
        assert!(
            work.get().is_none(),
            "Body comparison work scopes cannot nest"
        );
        work.set(Some(Snapshot::default()));
    });
    WorkGuard {
        _thread: PhantomData,
    }
}

impl WorkGuard {
    pub(crate) fn snapshot(&self) -> Snapshot {
        WORK.with(|work| work.get().expect("live Body comparison work scope"))
    }
}

impl Drop for WorkGuard {
    fn drop(&mut self) {
        WORK.with(|work| work.set(None));
    }
}

pub(crate) fn record_call(bytes: usize) {
    WORK.with(|work| {
        if let Some(mut counts) = work.get() {
            counts.comparison_bytes = counts
                .comparison_bytes
                .checked_add(u64::try_from(bytes).expect("test comparison bytes fit u64"))
                .expect("test comparison byte count fits u64");
            counts.comparison_calls = counts
                .comparison_calls
                .checked_add(1)
                .expect("test comparison call count fits u64");
            work.set(Some(counts));
        }
    });
}
