//! Scoped explicit construction requests, not allocator calls or machine writes.
use std::cell::Cell;
use std::marker::PhantomData;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Snapshot {
    pub output_reserve_calls: u64,
    pub output_capacity_bytes: u64,
    pub output_init_calls: u64,
    pub output_init_bytes: u64,
    pub validation_reserve_calls: u64,
    pub validation_capacity_bytes: u64,
    pub validation_init_calls: u64,
    pub validation_init_bytes: u64,
    pub encode_calls: u64,
    pub encode_bytes: u64,
    pub decode_calls: u64,
    pub decode_bytes: u64,
    pub comparison_calls: u64,
    pub comparison_bytes: u64,
}
std::thread_local! {
    static WORK: Cell<Option<Snapshot>> = const { Cell::new(None) };
}
pub(super) struct Guard(PhantomData<Rc<()>>);
pub(super) fn begin() -> Guard {
    WORK.with(|work| {
        assert!(work.get().is_none(), "construction witnesses cannot nest");
        work.set(Some(Snapshot::default()));
    });
    Guard(PhantomData)
}
impl Guard {
    pub fn snapshot(&self) -> Snapshot {
        WORK.with(|work| work.get().expect("live construction witness"))
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        WORK.with(|work| work.set(None));
    }
}
fn add(calls: &mut u64, total: &mut u64, bytes: usize) {
    *calls = calls.checked_add(1).expect("bounded test calls");
    *total = total
        .checked_add(u64::try_from(bytes).expect("test bytes fit u64"))
        .expect("bounded test bytes");
}
macro_rules! event {
    ($name:ident, $calls:ident, $bytes:ident) => {
        pub(super) fn $name(bytes: usize) {
            WORK.with(|work| {
                if let Some(mut count) = work.get() {
                    add(&mut count.$calls, &mut count.$bytes, bytes);
                    work.set(Some(count));
                }
            });
        }
    };
}
event!(output_reserve, output_reserve_calls, output_capacity_bytes);
event!(output_init, output_init_calls, output_init_bytes);
event!(
    validation_reserve,
    validation_reserve_calls,
    validation_capacity_bytes
);
event!(
    validation_init,
    validation_init_calls,
    validation_init_bytes
);
event!(encode, encode_calls, encode_bytes);
event!(decode, decode_calls, decode_bytes);
event!(compare, comparison_calls, comparison_bytes);
