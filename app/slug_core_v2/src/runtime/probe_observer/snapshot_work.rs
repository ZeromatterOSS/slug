//! Test-only work inside a synchronous rooted snapshot.
use std::cell::Cell;
use std::marker::PhantomData;
use std::rc::Rc;
use std::time::Instant;

#[derive(Clone, Copy)]
pub(crate) enum Phase {
    Capture,
    Record,
    PriorCompare,
    BuilderAppend,
}

pub(super) const LABELS: [&str; 4] = ["capture", "record", "prior_compare", "builder_append"];

#[derive(Clone, Copy, Default)]
pub(super) struct Work {
    pub(super) rows: [[u64; 3]; LABELS.len()],
    pub(super) unaligned_chunks: u64,
    pub(super) invalid: bool,
}

impl Work {
    fn add_unaligned(&mut self, count: u64) {
        if let Some(total) = self.unaligned_chunks.checked_add(count) {
            self.unaligned_chunks = total;
        } else {
            self.invalid = true;
        }
    }

    pub(super) fn merge(&mut self, other: Self) {
        self.invalid |= other.invalid;
        self.add_unaligned(other.unaligned_chunks);
        for (total, delta) in self
            .rows
            .iter_mut()
            .flatten()
            .zip(other.rows.into_iter().flatten())
        {
            if let Some(sum) = total.checked_add(delta) {
                *total = sum;
            } else {
                self.invalid = true;
            }
        }
    }
}

thread_local! {
    static WORK: Cell<Option<Work>> = const { Cell::new(None) };
}

pub(crate) fn record_unaligned_comparison_chunk() {
    WORK.with(|work| {
        if let Some(mut totals) = work.get() {
            totals.add_unaligned(1);
            work.set(Some(totals));
        }
    });
}

pub(super) struct Scope(PhantomData<Rc<()>>);

impl Scope {
    pub(super) fn begin() -> Self {
        WORK.with(|work| {
            assert!(
                work.get().is_none(),
                "rooted snapshot probe scopes cannot nest"
            );
            work.set(Some(Work::default()));
        });
        Self(PhantomData)
    }

    pub(super) fn finish(self) -> Work {
        WORK.with(|work| work.take().expect("active rooted snapshot probe"))
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        WORK.with(|work| work.set(None));
    }
}

#[must_use]
pub(crate) struct Span {
    phase: Phase,
    start: Instant,
    requested_bytes: usize,
    _thread: PhantomData<Rc<()>>,
}

pub(crate) fn span(phase: Phase, requested_bytes: usize) -> Option<Span> {
    WORK.with(|work| work.get().is_some()).then(|| Span {
        phase,
        start: Instant::now(),
        requested_bytes,
        _thread: PhantomData,
    })
}

impl Drop for Span {
    fn drop(&mut self) {
        let wall_ns = Instant::now()
            .checked_duration_since(self.start)
            .and_then(|elapsed| u64::try_from(elapsed.as_nanos()).ok());
        WORK.with(|work| {
            if let Some(mut totals) = work.get() {
                let row = &mut totals.rows[self.phase as usize];
                let next = row[0]
                    .checked_add(1)
                    .zip(wall_ns.and_then(|ns| row[1].checked_add(ns)))
                    .zip(row[2].checked_add(self.requested_bytes as u64));
                if let Some(((calls, ns), bytes)) = next {
                    *row = [calls, ns, bytes];
                } else {
                    totals.invalid = true;
                }
                work.set(Some(totals));
            }
        });
    }
}
