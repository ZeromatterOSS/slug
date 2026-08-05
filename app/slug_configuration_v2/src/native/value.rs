use std::cmp::Ordering;
use std::num::NonZeroI32;
use std::ops::Deref;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Allocative)]
pub(super) enum NativeValue {
    Bool(bool),
    Int(i32),
    Text(CompactString),
    Tri(TriState),
    Enum(EnumValue),
    Duration(Duration),
    Dotted(CompactString),
    Entry(CompactString, CompactString),
    Env(EnvValue),
    Shard(ShardValue),
    Runs(RunsPerTestSeed),
    RegexFilterDefault(RegexFilterDefaultSeed),
    List(NativeValues),
    OrderedMap(NativePairs),
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Allocative, Dupe)]
pub(super) struct NativeValues(pub(super) Arc<[NativeValue]>);
impl Deref for NativeValues {
    type Target = [NativeValue];
    fn deref(&self) -> &[NativeValue] {
        &self.0
    }
}
impl NativeValues {
    pub(super) fn as_ptr(&self) -> *const NativeValue {
        self.0.as_ptr()
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Allocative, Dupe)]
pub(super) struct NativePairs(pub(super) Arc<[(NativeValue, NativeValue)]>);
impl Deref for NativePairs {
    type Target = [(NativeValue, NativeValue)];
    fn deref(&self) -> &[(NativeValue, NativeValue)] {
        &self.0
    }
}
impl NativePairs {
    pub(super) fn as_ptr(&self) -> *const (NativeValue, NativeValue) {
        self.0.as_ptr()
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Allocative)]
pub(super) enum TriState {
    Auto,
    Yes,
    No,
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Allocative)]
pub(super) enum EnvValue {
    Set(CompactString, CompactString),
    Inherit(CompactString),
    Unset(CompactString),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Allocative)]
pub(super) enum ShardValue {
    Explicit,
    Disabled,
    Forced(i32),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Allocative)]
pub(super) struct RunsPerTestSeed(NonZeroI32);
impl RunsPerTestSeed {
    pub(super) fn one() -> Self {
        Self(NonZeroI32::new(1).unwrap())
    }
    pub(super) fn positive_runs(self) -> NonZeroI32 {
        self.0
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Allocative)]
pub(super) enum RegexFilterDefaultSemantic {
    ExcludeAll,
    InstrumentationDefault,
}
#[derive(Clone, Debug, Allocative)]
pub(super) struct RegexFilterDefaultSeed {
    pub(super) original_input: CompactString,
    pub(super) semantic: RegexFilterDefaultSemantic,
}
impl RegexFilterDefaultSeed {
    pub(super) fn new(
        original_input: impl Into<CompactString>,
        semantic: RegexFilterDefaultSemantic,
    ) -> Self {
        Self {
            original_input: original_input.into(),
            semantic,
        }
    }
    pub(super) fn canonical_text(&self) -> &'static str {
        match self.semantic {
            RegexFilterDefaultSemantic::ExcludeAll => "-(?:(?>.*))",
            RegexFilterDefaultSemantic::InstrumentationDefault => {
                "-(?:(?>/javatests[/:])|(?>/test/java[/:]))"
            }
        }
    }
}
impl PartialEq for RegexFilterDefaultSeed {
    fn eq(&self, other: &Self) -> bool {
        self.semantic == other.semantic
    }
}
impl Eq for RegexFilterDefaultSeed {}
impl PartialOrd for RegexFilterDefaultSeed {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for RegexFilterDefaultSeed {
    fn cmp(&self, other: &Self) -> Ordering {
        self.semantic.cmp(&other.semantic)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Allocative)]
pub(super) struct Duration {
    pub(super) seconds: i64,
    pub(super) nanos: u32,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Allocative)]
pub(super) enum EnumFamily {
    StrictDeps,
    Exec,
    OutputName,
    OutputPaths,
    Include,
    Android,
    Apk,
    Merger,
    MergerOrder,
    Apple,
    Dynamic,
    Classpath,
    OneVersion,
    Cancel,
    Compilation,
    Strip,
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Allocative)]
pub(super) struct EnumValue {
    pub(super) family: EnumFamily,
    pub(super) member: CompactString,
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Allocative)]
pub(super) enum NativeOccurrence {
    Absent,
    Scalar(NativeValue),
    List(NativeValues),
}
