//! Test-only requested full-hash inputs. Not CPU, allocation or native IO data.
use std::cell::Cell;

#[derive(Clone, Copy)]
pub(super) enum Kind {
    Seed = 0,
    Canonical = 1,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Work {
    pub calls: [u64; 2],
    pub payload_bytes: [u64; 2],
    pub overflow_mask: u8,
}
std::thread_local! {
    static WORK: Cell<Work> = const { Cell::new(Work { calls: [0; 2], payload_bytes: [0; 2], overflow_mask: 0 }) };
}
pub(super) fn record(kind: Kind, bytes: usize) {
    WORK.with(|slot| {
        let mut work = slot.get();
        let index = kind as usize;
        match work.calls[index].checked_add(1) {
            Some(value) => work.calls[index] = value,
            None => {
                work.calls[index] = u64::MAX;
                work.overflow_mask |= 1 << index;
            }
        }
        match u64::try_from(bytes)
            .ok()
            .and_then(|bytes| work.payload_bytes[index].checked_add(bytes))
        {
            Some(value) => work.payload_bytes[index] = value,
            None => {
                work.payload_bytes[index] = u64::MAX;
                work.overflow_mask |= 1 << (index + 2);
            }
        }
        slot.set(work);
    });
}
pub(super) fn reset() {
    WORK.set(Work::default());
}
pub(super) fn snapshot() -> Work {
    WORK.get()
}
