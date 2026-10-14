//! Immutable raw executable-environment fields, separate from launch projection.
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;
use starlark_map::StarlarkHasher;
use starlark_map::small_map::SmallMap;

/// An ordered string map and an ordered, duplicate-preserving inherited list.
/// Public native equality preserves the getter's observable order. Starlark
/// NativeInfo field equality is available separately through `field_eq`.
#[derive(Debug, Clone, Dupe, Allocative)]
pub struct RunEnvironmentInfo(Arc<RunEnvironmentInfoData>);

#[derive(Debug, Allocative)]
struct RunEnvironmentInfoData {
    environment: SmallMap<CompactString, CompactString>,
    inherited_environment: Arc<[CompactString]>,
}

impl RunEnvironmentInfo {
    pub fn new(
        environment: SmallMap<CompactString, CompactString>,
        inherited_environment: Arc<[CompactString]>,
    ) -> Self {
        Self(Arc::new(RunEnvironmentInfoData {
            environment,
            inherited_environment,
        }))
    }

    pub fn empty() -> Self {
        Self::new(SmallMap::new(), Arc::from([]))
    }

    pub fn environment(&self) -> &SmallMap<CompactString, CompactString> {
        &self.0.environment
    }

    pub fn inherited_environment(&self) -> &[CompactString] {
        &self.0.inherited_environment
    }

    /// NativeInfo fields compare maps without insertion order and sequences
    /// with order and duplicate multiplicity. No construction occurrence enters
    /// this equality, including for empty instances.
    pub fn field_eq(&self, other: &Self) -> bool {
        self.environment() == other.environment()
            && self.inherited_environment() == other.inherited_environment()
    }

    /// Hash the same fields as `field_eq`; this is a structural Rust hash, not
    /// a Java hash projection. Map hashing is order independent and allocation free.
    pub fn hash_fields<H: Hasher>(&self, state: &mut H) {
        let mut xor = 0u64;
        let mut sum = 0u64;
        for entry in self.environment() {
            let mut hasher = StarlarkHasher::default();
            entry.hash(&mut hasher);
            let hash = hasher.finish();
            xor ^= hash.rotate_left(17);
            sum = sum.wrapping_add(hash);
        }
        self.environment().len().hash(state);
        xor.hash(state);
        sum.hash(state);
        self.inherited_environment().hash(state);
    }
}

impl PartialEq for RunEnvironmentInfo {
    fn eq(&self, other: &Self) -> bool {
        self.environment().eq_ordered(other.environment())
            && self.inherited_environment() == other.inherited_environment()
    }
}
impl Eq for RunEnvironmentInfo {}

#[cfg(test)]
mod tests;
