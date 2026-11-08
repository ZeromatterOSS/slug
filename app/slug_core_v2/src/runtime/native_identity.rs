//! Non-reusing process-local native resource incarnation numbers. These are
//! neither content identity nor a semantic cache: the only global state is one
//! checked counter. No root/FD/request/value is stored here. All native actors
//! use this stream so a recreated materializer cannot revive an old instance,
//! session token or equal EMPTY workspace epoch within a live process.
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use slug_workspace_v2::PathObservationInstanceId;
static NEXT: AtomicU64 = AtomicU64::new(1);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct NativeIdentityExhausted;
impl std::fmt::Display for NativeIdentityExhausted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("native resource identity is invalid or exhausted")
    }
}
impl std::error::Error for NativeIdentityExhausted {}
fn allocate(counter: &AtomicU64) -> Result<u64, NativeIdentityExhausted> {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            if current == 0 {
                None
            } else {
                current.checked_add(1)
            }
        })
        .map_err(|_| NativeIdentityExhausted)
}
pub(super) fn next_repository_instance()
-> Result<PathObservationInstanceId, NativeIdentityExhausted> {
    allocate(&NEXT).map(PathObservationInstanceId::new)
}
pub(super) fn next_repository_session() -> Result<u64, NativeIdentityExhausted> {
    allocate(&NEXT)
}
pub(super) fn next_workspace_lease() -> Result<u64, NativeIdentityExhausted> {
    allocate(&NEXT)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_native_roles_never_reuse_an_incarnation() {
        let a = next_repository_instance().unwrap().value();
        let b = next_repository_session().unwrap();
        let c = next_workspace_lease().unwrap();
        let d = next_repository_instance().unwrap().value();
        let e = next_repository_session().unwrap();
        let f = next_workspace_lease().unwrap();
        assert!(0 < a && a < b && b < c && c < d && d < e && e < f);
    }
    #[test]
    fn zero_and_exhaustion_never_wrap_or_grant_reusable_ids() {
        let zero = AtomicU64::new(0);
        assert_eq!(allocate(&zero), Err(NativeIdentityExhausted));
        assert_eq!(zero.load(Ordering::Relaxed), 0);
        let limit = AtomicU64::new(u64::MAX - 1);
        assert_eq!(allocate(&limit), Ok(u64::MAX - 1));
        for _ in 0..2 {
            assert_eq!(allocate(&limit), Err(NativeIdentityExhausted));
            assert_eq!(limit.load(Ordering::Relaxed), u64::MAX);
        }
    }
}
