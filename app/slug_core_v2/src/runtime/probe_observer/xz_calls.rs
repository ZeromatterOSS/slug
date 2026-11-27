//! Test-only liblzma intervals on the synchronous archive extraction thread.
use std::cell::Cell;
use std::marker::PhantomData;
use std::rc::Rc;
use std::time::Instant;

#[derive(Clone, Copy)]
pub(crate) enum Call {
    Initialize,
    Decode,
    End,
}

pub(super) const COLUMNS: [&str; 6] = [
    "initialize_calls",
    "code_calls",
    "end_calls",
    "wall_ns",
    "compressed_consumed",
    "decoded_produced",
];

#[derive(Clone, Copy, Default)]
pub(super) struct Work {
    pub(super) row: [u64; COLUMNS.len()],
    pub(super) invalid: bool,
}

impl Work {
    pub(super) fn merge(&mut self, other: Self) {
        self.invalid |= other.invalid;
        for (total, delta) in self.row.iter_mut().zip(other.row) {
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

pub(super) struct Scope(PhantomData<Rc<()>>);

impl Scope {
    pub(super) fn begin() -> Self {
        WORK.with(|work| {
            assert!(
                work.get().is_none(),
                "XZ extraction probe scopes cannot nest"
            );
            work.set(Some(Work::default()));
        });
        Self(PhantomData)
    }

    pub(super) fn finish(self) -> Work {
        WORK.with(|work| work.take().expect("active XZ extraction probe"))
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        WORK.with(|work| work.set(None));
    }
}

pub(crate) fn start() -> Option<Instant> {
    WORK.with(|work| work.get().is_some()).then(Instant::now)
}

pub(crate) fn record(start: Option<Instant>, call: Call, bytes: Option<(usize, usize)>) {
    let Some(start) = start else {
        return;
    };
    let elapsed = Instant::now()
        .checked_duration_since(start)
        .and_then(|elapsed| u64::try_from(elapsed.as_nanos()).ok());
    let mut current = Work::default();
    current.row[call as usize] = 1;
    current.row[3] = elapsed.unwrap_or(0);
    current.invalid = elapsed.is_none();
    if let Some((consumed, produced)) = bytes {
        current.row[4] = consumed as u64;
        current.row[5] = produced as u64;
    } else if matches!(call, Call::Decode) {
        current.invalid = true;
    }
    WORK.with(|work| {
        if let Some(mut totals) = work.get() {
            totals.merge(current);
            work.set(Some(totals));
        }
    });
}
