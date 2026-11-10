//! Thread-local scalar work witness; it retains no IO or semantic owner.
use std::cell::Cell;
use std::marker::PhantomData;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Snapshot {
    pub(crate) digest_payload: u64,
    pub(crate) digest_sha: u64,
    pub(crate) contents_payload: u64,
    pub(crate) contents_sha: u64,
}

std::thread_local! {
    static WORK: Cell<Option<Snapshot>> = const { Cell::new(None) };
}

pub(crate) struct WorkGuard {
    _thread: PhantomData<Rc<()>>,
}

pub(crate) fn begin() -> WorkGuard {
    WORK.with(|work| {
        assert!(work.get().is_none(), "RootIo work scopes cannot nest");
        work.set(Some(Snapshot::default()));
    });
    WorkGuard {
        _thread: PhantomData,
    }
}

impl WorkGuard {
    pub(crate) fn snapshot(&self) -> Snapshot {
        WORK.with(|work| work.get().expect("live RootIo work scope"))
    }
}

impl Drop for WorkGuard {
    fn drop(&mut self) {
        WORK.with(|work| work.set(None));
    }
}

pub(crate) fn record_chunk(digest_requested: bool, bytes: usize, hashed: bool) {
    WORK.with(|work| {
        if let Some(mut counts) = work.get() {
            let bytes = u64::try_from(bytes).expect("test byte count fits u64");
            let (payload, sha) = if digest_requested {
                (&mut counts.digest_payload, &mut counts.digest_sha)
            } else {
                (&mut counts.contents_payload, &mut counts.contents_sha)
            };
            *payload = payload
                .checked_add(bytes)
                .expect("test payload count fits u64");
            if hashed {
                *sha = sha.checked_add(bytes).expect("test SHA count fits u64");
            }
            work.set(Some(counts));
        }
    });
}
