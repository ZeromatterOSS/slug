//! Scoped real Packed-range work requests; not instructions or unique bytes.
use std::cell::Cell;
use std::marker::PhantomData;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Snapshot {
    pub initialization_calls: u64,
    pub initialization_bytes: u64,
    pub range_calls: u64,
    pub range_bytes: u64,
    pub decode_calls: u64,
    pub decode_bytes: u64,
}
std::thread_local! {
    static WORK: Cell<Option<Snapshot>> = const { Cell::new(None) };
}
pub(super) struct Guard(PhantomData<Rc<()>>);
pub(super) fn begin() -> Guard {
    WORK.with(|work| {
        assert!(work.get().is_none(), "range witnesses cannot nest");
        work.set(Some(Snapshot::default()));
    });
    Guard(PhantomData)
}
impl Guard {
    pub fn snapshot(&self) -> Snapshot {
        WORK.with(|work| work.get().expect("live range witness"))
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
event!(initialize, initialization_calls, initialization_bytes);
event!(range, range_calls, range_bytes);
event!(decode, decode_calls, decode_bytes);
