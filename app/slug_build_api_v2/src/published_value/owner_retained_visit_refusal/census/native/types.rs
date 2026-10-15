//! Fixed scalar diagnostic data, never retained payload guards.
use std::fmt;

use super::*;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(usize)]
pub enum PublicationNativeChannel {
    Atom,
    HiddenFiles,
    TargetProvider,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(usize)]
pub enum PublicationNativeRootFamily {
    Scalar,
    LabelArtifact,
    CppMetadata,
    CcFeatureModel,
    CcFeatureConfiguration,
    RunfilesDepset,
    VariableEnvironment,
    PackageSpecification,
    FilesProvider,
    OtherProvider,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(usize)]
pub enum PublicationRowKind {
    Function,
    Intrinsic,
    Code,
    ProviderConstructor,
    RawProviderConstructor,
    Cell,
    Set,
    List,
    Tuple,
    Dictionary,
    Struct,
    Provider,
    ConfiguredTarget,
    Depset,
    CcVariables,
    Native,
}
#[derive(Default, Clone, Copy, Eq, PartialEq)]
pub struct PublicationNativeStatistics {
    pub(super) count: usize,
    pub(super) visits: usize,
    pub(super) maximum: usize,
}
impl fmt::Debug for PublicationNativeStatistics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Fixed tuple order: count, complete visits, maximum complete visits.
        write!(f, "({},{},{})", self.count, self.visits, self.maximum)
    }
}
impl PublicationNativeStatistics {
    pub fn count(&self) -> usize {
        self.count
    }
    pub fn visits(&self) -> usize {
        self.visits
    }
    pub fn maximum(&self) -> usize {
        self.maximum
    }
    pub(super) fn add(&mut self, visits: usize) -> Option<()> {
        self.count = self.count.checked_add(1)?;
        self.visits = self.visits.checked_add(visits)?;
        self.maximum = self.maximum.max(visits);
        Some(())
    }
}
#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub struct PublicationNativeRetentionCensus {
    pub(super) channels: [PublicationNativeStatistics; 3],
    pub(super) families: [PublicationNativeStatistics; 10],
    pub(super) row_kinds: [usize; 16],
    pub(super) rows: usize,
    pub(super) maximum_rows: usize,
    pub(super) edges: usize,
    pub(super) maximum_edges: usize,
    pub(super) slots: usize,
    pub(super) maximum_slots: usize,
    pub(super) maximum_local_visits: usize,
    pub(super) maximum_arena_certificates: usize,
    pub(super) maximum_arena_native_visits: usize,
    pub(super) native_visits: usize,
    pub(super) other_visits: usize,
    pub(super) feature_selected: bool,
    pub(super) feature_ambiguous: bool,
    pub(super) feature_maximum_wrapper_visits: usize,
    pub(super) feature_occurrences: usize,
    pub(super) feature_arenas: usize,
    pub(super) feature_wrapper_visits: usize,
}
impl PublicationNativeRetentionCensus {
    pub fn channel(&self, channel: PublicationNativeChannel) -> &PublicationNativeStatistics {
        &self.channels[channel as usize]
    }
    pub fn family(&self, family: PublicationNativeRootFamily) -> &PublicationNativeStatistics {
        &self.families[family as usize]
    }
    pub fn row_kind_count(&self, kind: PublicationRowKind) -> usize {
        self.row_kinds[kind as usize]
    }
    pub fn rows(&self) -> usize {
        self.rows
    }
    pub fn maximum_rows(&self) -> usize {
        self.maximum_rows
    }
    pub fn edges(&self) -> usize {
        self.edges
    }
    pub fn maximum_edges(&self) -> usize {
        self.maximum_edges
    }
    pub fn slots(&self) -> usize {
        self.slots
    }
    pub fn maximum_slots(&self) -> usize {
        self.maximum_slots
    }
    pub fn maximum_local_visits(&self) -> usize {
        self.maximum_local_visits
    }
    pub fn maximum_arena_certificates(&self) -> usize {
        self.maximum_arena_certificates
    }
    pub fn maximum_arena_native_visits(&self) -> usize {
        self.maximum_arena_native_visits
    }
    pub fn native_visits(&self) -> usize {
        self.native_visits
    }
    pub fn other_visits(&self) -> usize {
        self.other_visits
    }
    pub fn feature_selected(&self) -> bool {
        self.feature_selected
    }
    pub fn feature_ambiguous(&self) -> bool {
        self.feature_ambiguous
    }
    pub fn feature_maximum_wrapper_visits(&self) -> usize {
        self.feature_maximum_wrapper_visits
    }
    pub fn feature_occurrences(&self) -> usize {
        self.feature_occurrences
    }
    pub fn feature_arenas(&self) -> usize {
        self.feature_arenas
    }
    pub fn feature_wrapper_visits(&self) -> usize {
        self.feature_wrapper_visits
    }
    pub(super) fn record(
        &mut self,
        channel: PublicationNativeChannel,
        family: PublicationNativeRootFamily,
        visits: usize,
        arena: &mut PublicationNativeStatistics,
    ) -> Option<()> {
        self.channels[channel as usize].add(visits)?;
        self.families[family as usize].add(visits)?;
        self.native_visits = self.native_visits.checked_add(visits)?;
        arena.add(visits)
    }
}
