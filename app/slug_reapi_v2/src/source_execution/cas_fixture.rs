//! Adversarial fixture ownership of completed CAS bodies, never product discovery.
use std::io::Read;
use std::path::PathBuf;

use crate::ReapiDigest;

/// Locate ALL independently verified completed copies before any test mutation.
/// The fixed namespace is owned by this separately supervised NativeLink run.
pub(super) fn completed_blob_paths(digest: &ReapiDigest) -> Vec<PathBuf> {
    const MAX_ENTRIES_PER_STORE: usize = 1024;
    const MAX_FIXTURE_BODY_BYTES: u64 = 64 * 1024;
    assert!(digest.size_bytes() > 0 && digest.size_bytes() <= MAX_FIXTURE_BODY_BYTES);
    let root = PathBuf::from(std::env::var("SLUG_V2_NATIVELINK_TEST_ROOT").unwrap());
    let mut paths = Vec::new();
    for store in ["fast-content", "slow-content"] {
        // No temporary, executable-copy or workspace tree is searched.
        let directory = root.join("cas").join(store).join("d2");
        let kind = std::fs::symlink_metadata(&directory).unwrap().file_type();
        assert!(
            kind.is_dir() && !kind.is_symlink(),
            "{}",
            directory.display()
        );
        let prior_count = paths.len();
        for (index, entry) in std::fs::read_dir(&directory).unwrap().enumerate() {
            assert!(
                index < MAX_ENTRIES_PER_STORE,
                "fixture CAS enumeration bound"
            );
            let path = entry.unwrap().path();
            let metadata = std::fs::symlink_metadata(&path).unwrap();
            assert!(metadata.is_file() && !metadata.file_type().is_symlink());
            if metadata.len() != digest.size_bytes() {
                continue;
            }
            // Read no more than the bounded expected size+1, including races.
            let mut bytes = Vec::with_capacity(digest.size_bytes() as usize);
            std::fs::File::open(&path)
                .unwrap()
                .take(digest.size_bytes() + 1)
                .read_to_end(&mut bytes)
                .unwrap();
            if bytes.len() as u64 == digest.size_bytes() && ReapiDigest::of_bytes(&bytes) == *digest
            {
                paths.push(path);
            }
        }
        assert!(
            paths.len() > prior_count,
            "no independently verified completed {digest} body in {}",
            directory.display()
        );
    }
    paths.sort();
    paths
}
