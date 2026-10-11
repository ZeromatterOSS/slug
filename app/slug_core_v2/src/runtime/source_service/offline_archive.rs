use std::sync::Arc;

use slug_workspace_v2::NormalizedAbsolutePath;

/// Physical backing for one original archive; it owns no repository semantics.
/// Construction performs no IO. Capture verifies the original SRI on every use.
pub(in crate::runtime) struct OfflineArchiveBacking {
    original_url: &'static str,
    integrity: [u8; 32],
    path: NormalizedAbsolutePath,
}

impl OfflineArchiveBacking {
    pub(in crate::runtime) fn new(
        original_url: &'static str,
        integrity: [u8; 32],
        path: NormalizedAbsolutePath,
    ) -> Arc<Self> {
        Arc::new(Self {
            original_url,
            integrity,
            path,
        })
    }

    pub(in crate::runtime) fn original_url(&self) -> &'static str {
        self.original_url
    }

    pub(in crate::runtime) fn integrity(&self) -> &[u8; 32] {
        &self.integrity
    }

    pub(in crate::runtime) fn path(&self) -> &NormalizedAbsolutePath {
        &self.path
    }

    pub(in crate::runtime) fn matches(&self, original_url: &str, integrity: &[u8; 32]) -> bool {
        self.original_url == original_url && &self.integrity == integrity
    }
}

pub(super) fn cohort_has_unique_urls(backings: &[Option<Arc<OfflineArchiveBacking>>; 3]) -> bool {
    for (index, backing) in backings.iter().enumerate() {
        if let Some(backing) = backing {
            for other in backings[index + 1..].iter().flatten() {
                if backing.original_url() == other.original_url() {
                    return false;
                }
            }
        }
    }
    true
}

pub(super) fn cohort_path<'a>(
    backings: &'a [Option<Arc<OfflineArchiveBacking>>; 3],
    original_url: &str,
    integrity: &[u8; 32],
) -> Option<&'a NormalizedAbsolutePath> {
    backings
        .iter()
        .flatten()
        .find(|backing| backing.matches(original_url, integrity))
        .map(|backing| backing.path())
}
