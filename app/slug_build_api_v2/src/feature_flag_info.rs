//! Starlark-created feature flags retain one immutable occurrence and value.
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;

/// Native equality is occurrence identity; DICE publication compares the
/// immutable value so a fresh evaluation with the same flag can cut off.
#[derive(Debug, Clone, Dupe, Allocative)]
pub struct FeatureFlagInfo(Arc<CompactString>);

impl FeatureFlagInfo {
    pub fn new(value: impl Into<CompactString>) -> Self {
        Self(Arc::new(value.into()))
    }

    pub fn value(&self) -> &str {
        &self.0
    }

    pub(crate) fn publication_eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl PartialEq for FeatureFlagInfo {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for FeatureFlagInfo {}
impl Hash for FeatureFlagInfo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (Arc::as_ptr(&self.0) as usize).hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProviderCollection;
    use crate::ProviderIdentity;
    use crate::ProviderOccurrence;
    use crate::ProviderValue;

    #[test]
    fn occurrence_identity_and_structural_publication() {
        assert_eq!(
            std::mem::size_of::<FeatureFlagInfo>(),
            std::mem::size_of::<usize>()
        );
        let first = FeatureFlagInfo::new("ready");
        let alias = first.dupe();
        let again = FeatureFlagInfo::new("ready");
        let changed = FeatureFlagInfo::new("later");
        assert_eq!(first, alias);
        assert_ne!(first, again);
        assert!(first.publication_eq(&again));
        assert!(!first.publication_eq(&changed));
        let hash = |value: &FeatureFlagInfo| {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            value.hash(&mut hasher);
            hasher.finish()
        };
        assert_eq!(hash(&first), hash(&alias));
    }

    #[test]
    fn provider_collection_requires_typed_instance() {
        let typed = ProviderCollection::from_values(
            vec![ProviderValue::FeatureFlagInfo(FeatureFlagInfo::new(
                "ready",
            ))],
            false,
        )
        .unwrap();
        assert_eq!(typed.feature_flag_info().unwrap().value(), "ready");
        assert!(typed.contains(&ProviderIdentity::builtin("FeatureFlagInfo")));
        let forged = ProviderCollection::from_values(
            vec![ProviderValue::Occurrence(ProviderOccurrence::empty(
                ProviderIdentity::builtin("FeatureFlagInfo"),
            ))],
            false,
        )
        .unwrap_err();
        assert!(
            forged
                .to_string()
                .contains("requires a native typed instance")
        );
        let duplicate = ProviderCollection::from_values(
            vec![
                ProviderValue::FeatureFlagInfo(FeatureFlagInfo::new("a")),
                ProviderValue::FeatureFlagInfo(FeatureFlagInfo::new("b")),
            ],
            false,
        )
        .unwrap_err();
        assert!(duplicate.to_string().contains("specified twice"));
    }
}
