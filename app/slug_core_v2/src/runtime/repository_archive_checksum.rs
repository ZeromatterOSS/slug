//! Operation-local expected-checksum arithmetic; actual capture identity is
//! independently derived by the closed FD verifier.
use sha2::Digest;
use sha2::Sha256;

pub(super) struct ExpectedArchiveChecksum {
    expected: Option<[u8; 32]>,
    state: Option<Sha256>,
}

impl ExpectedArchiveChecksum {
    pub(super) fn new(expected: Option<[u8; 32]>) -> Self {
        Self {
            expected,
            // Only a declared expectation needs acquisition hashing; the closed
            // FD verifier independently derives the actual capture digest.
            state: expected.map(|_| Sha256::new()),
        }
    }

    pub(super) fn update(&mut self, bytes: &[u8]) {
        #[cfg(test)]
        work_for_test::record_update(bytes.len(), self.state.is_some());
        if let Some(state) = &mut self.state {
            state.update(bytes);
        }
    }

    pub(super) fn matches_expected(self) -> bool {
        // Finalize a declared expectation at the original comparison boundary.
        // A missing state must never accept a required checksum.
        let actual = self.state.map(|state| <[u8; 32]>::from(state.finalize()));
        match self.expected {
            Some(expected) => actual.is_some_and(|actual| actual == expected),
            None => true,
        }
    }
}

#[cfg(test)]
pub(crate) mod work_for_test {
    use std::cell::Cell;
    use std::marker::PhantomData;
    use std::rc::Rc;

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub(crate) struct Snapshot {
        pub(crate) copied: u64,
        pub(crate) hashed: u64,
        pub(crate) verified: u64,
    }

    std::thread_local! {
        static WORK: Cell<Option<Snapshot>> = const { Cell::new(None) };
    }

    // The guard cannot migrate between threads or retain any runtime owner.
    pub(crate) struct WorkGuard {
        _thread: PhantomData<Rc<()>>,
    }

    pub(crate) fn begin() -> WorkGuard {
        WORK.with(|work| {
            assert!(work.get().is_none(), "archive work scopes cannot nest");
            work.set(Some(Snapshot::default()));
        });
        WorkGuard {
            _thread: PhantomData,
        }
    }

    impl WorkGuard {
        pub(crate) fn snapshot(&self) -> Snapshot {
            WORK.with(|work| work.get().expect("live archive work scope"))
        }
    }

    impl Drop for WorkGuard {
        fn drop(&mut self) {
            WORK.with(|work| work.set(None));
        }
    }

    fn add(counter: &mut u64, bytes: usize) {
        *counter = counter
            .checked_add(u64::try_from(bytes).expect("test byte count fits u64"))
            .expect("test archive work count fits u64");
    }

    pub(super) fn record_update(bytes: usize, hashed: bool) {
        WORK.with(|work| {
            if let Some(mut counts) = work.get() {
                add(&mut counts.copied, bytes);
                if hashed {
                    add(&mut counts.hashed, bytes);
                }
                work.set(Some(counts));
            }
        });
    }

    pub(crate) fn record_verified(bytes: usize) {
        WORK.with(|work| {
            if let Some(mut counts) = work.get() {
                add(&mut counts.verified, bytes);
                work.set(Some(counts));
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omitted_expectation_matches_multichunk_payload_before_zero_hash_work_gate() {
        let guard = work_for_test::begin();
        let mut checksum = ExpectedArchiveChecksum::new(None);
        for chunk in [b"prefix".as_slice(), b"", b"suffix"] {
            checksum.update(chunk);
        }
        assert!(checksum.matches_expected());
        let work = guard.snapshot();
        assert_eq!(work.copied, b"prefixsuffix".len() as u64);
        assert_eq!(work.verified, 0);
        // The unconditional RED implementation fails only this work assertion.
        assert_eq!(work.hashed, 0);
    }

    #[test]
    fn omitted_empty_expectation_accepts_without_payload_work() {
        let guard = work_for_test::begin();
        let mut checksum = ExpectedArchiveChecksum::new(None);
        checksum.update(b"");
        assert!(checksum.matches_expected());
        assert_eq!(guard.snapshot(), work_for_test::Snapshot::default());
    }

    #[test]
    fn declared_expectations_hash_every_chunk_and_refuse_wrong_or_partial_digest() {
        let expected: [u8; 32] = Sha256::digest(b"prefixsuffix").into();
        let partial: [u8; 32] = Sha256::digest(b"prefix").into();
        assert_ne!(expected, partial);
        for (digest, matches) in [(expected, true), (partial, false), ([0; 32], false)] {
            let guard = work_for_test::begin();
            let mut checksum = ExpectedArchiveChecksum::new(Some(digest));
            checksum.update(b"prefix");
            checksum.update(b"");
            checksum.update(b"suffix");
            assert_eq!(checksum.matches_expected(), matches);
            assert_eq!(
                guard.snapshot(),
                work_for_test::Snapshot {
                    copied: 12,
                    hashed: 12,
                    verified: 0,
                }
            );
        }
    }

    #[test]
    fn declared_empty_digest_is_checked_and_missing_required_state_refuses() {
        let empty: [u8; 32] = Sha256::digest(b"").into();
        assert!(ExpectedArchiveChecksum::new(Some(empty)).matches_expected());
        assert!(!ExpectedArchiveChecksum::new(Some([0; 32])).matches_expected());
        assert!(
            !ExpectedArchiveChecksum {
                expected: Some(empty),
                state: None,
            }
            .matches_expected()
        );
    }

    #[test]
    fn checksum_control_is_fixed_stack_and_test_work_scope_retires() {
        assert!(std::mem::size_of::<ExpectedArchiveChecksum>() <= 256);
        {
            let guard = work_for_test::begin();
            let mut checksum = ExpectedArchiveChecksum::new(Some([0; 32]));
            checksum.update(b"payload");
            work_for_test::record_verified(3);
            assert_eq!(
                guard.snapshot(),
                work_for_test::Snapshot {
                    copied: 7,
                    hashed: 7,
                    verified: 3,
                }
            );
        }
        let guard = work_for_test::begin();
        assert_eq!(guard.snapshot(), work_for_test::Snapshot::default());
    }
}
