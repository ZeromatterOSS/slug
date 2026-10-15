//! Complete native row fields; environment use order is compared by its plan.
use super::*;
fn key_eq(a: &BzlStableKey, b: &BzlStableKey) -> bool {
    a.identity == b.identity && a.bazel_internal_strings == b.bazel_internal_strings
}
fn hash_key<H: Hasher>(a: &BzlStableKey, h: &mut H) {
    a.identity.hash(h);
    a.bazel_internal_strings.hash(h);
}
pub(super) fn local_eq(a: &LocalBzlInput, b: &LocalBzlInput) -> bool {
    key_eq(&a.key, &b.key)
        && a.source_digest == b.source_digest
        && a.fingerprint == b.fingerprint
        && a.source.source() == b.source.source()
        && a.children.len() == b.children.len()
        && a.children
            .iter()
            .zip(b.children.iter())
            .all(|(a, b)| key_eq(a, b))
}
pub(super) fn hash_local<H: Hasher>(a: &LocalBzlInput, h: &mut H) {
    hash_key(&a.key, h);
    a.source_digest.hash(h);
    a.fingerprint.hash(h);
    a.source.source().hash(h);
    a.children.len().hash(h);
    for child in &*a.children {
        hash_key(child, h);
    }
}
