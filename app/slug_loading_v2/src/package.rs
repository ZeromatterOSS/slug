/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::cell::OnceCell;
use std::cell::RefCell;
use std::fmt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::PackageIdentifier;
use starlark::any::ProvidesStaticType;
use starlark::environment::Globals;
use starlark::environment::GlobalsBuilder;
use starlark::environment::LibraryExtension;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Arguments;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::AllocFrozenValue;
use starlark::values::Freeze;
use starlark::values::FreezeError;
use starlark::values::FreezeResult;
use starlark::values::Freezer;
use starlark::values::FrozenHeap;
use starlark::values::FrozenValue;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Trace;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark::values::dict::DictRef;
use starlark::values::list::ListRef;
use starlark::values::list::UnpackList;
use starlark::values::list_or_tuple::UnpackListOrTuple;
use starlark::values::none::NoneType;
use starlark::values::starlark_value;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::attrs::AttributeKind;
use crate::attrs::AttributeProvenance;
use crate::attrs::AttributeSchema;
use crate::attrs::AttributeValue;
use crate::attrs::CoercedAttributeValue;
use crate::attrs::NativeAttributeOrder;
use crate::attrs::NativeAttributePolicy;
use crate::attrs::NativeAttributeSchema;
use crate::attrs::NativeAttributeValue;
use crate::attrs::NativeRuleAttributes;
use crate::attrs::NativeRuleClass;
use crate::attrs::TransitionDefinition as LoadingTransitionDefinition;
use crate::bzl_module::BzlModuleIdentity;
use crate::bzl_module::FrozenBzlLifetimeEntry;
use crate::glob::GlobError;
use crate::glob::GlobSpec;
use crate::glob::PackageListing;
use crate::glob::expand_glob;
use crate::host_glob::HostGlobLoadingOperation;
use crate::host_glob::HostGlobLoadingRequest;
use crate::host_glob::HostGlobPrepared;
use crate::host_glob::HostGlobRequestTraversalError;
use crate::provider::AnalysisBuiltinCallable;
use crate::provider::BzlEvaluationContext;
use crate::provider::UserProviderCallable;
use crate::visibility::PackageGroupContents;
use crate::visibility::RuleVisibility;
use crate::visibility::VisibilitySource;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageFile {
    pub package: PackageIdentifier,
    pub build_file: PathBuf,
}

/// The package result currently needed by the first BUILD-loading vertical.
///
/// This remains a loading-stage value: configured targets, providers, and
/// action declarations are built by Stage 6.
#[derive(Debug, Clone, Allocative)]
pub struct LoadedPackage {
    pub package_dir: PathBuf,
    pub build_file: PathBuf,
    pub default_visibility: RuleVisibility,
    pub targets: Vec<PackageTarget>,
    /// Sparse RuleClass values keyed by their stable position in `targets`.
    pub native_attributes: Arc<[NativeTargetAttributes]>,
    pub used_globs: Vec<GlobSpec>,
    /// Ordered label-first direct `.bzl` roots for this BUILD evaluation.
    pub direct_load_roots: Arc<[BzlModuleIdentity]>,
    /// Flat label-first first-seen closure of all direct roots.
    pub reachable_loads: Arc<[BzlModuleIdentity]>,
    /// SHA-256 over ordered direct semantic roots and their fingerprints.
    pub load_fingerprint: [u8; 32],
    #[allow(dead_code)] // Ownership only; frozen rule values borrow these heaps.
    retained_bzl_modules: Arc<[FrozenBzlLifetimeEntry]>,
}

impl PartialEq for LoadedPackage {
    fn eq(&self, other: &Self) -> bool {
        self.package_dir == other.package_dir
            && self.build_file == other.build_file
            && self.default_visibility == other.default_visibility
            && self.targets == other.targets
            && self.native_attributes == other.native_attributes
            && self.used_globs == other.used_globs
            && self.direct_load_roots == other.direct_load_roots
            && self.reachable_loads == other.reachable_loads
            && self.load_fingerprint == other.load_fingerprint
    }
}

impl Eq for LoadedPackage {}

impl LoadedPackage {
    pub fn native_attributes(&self, target: &str) -> Option<&NativeRuleAttributes> {
        let target_index = self
            .targets
            .iter()
            .position(|candidate| candidate.name == target)?;
        self.native_attributes_at(target_index)
    }

    pub fn native_attributes_at(&self, target_index: usize) -> Option<&NativeRuleAttributes> {
        let target_index = u32::try_from(target_index).ok()?;
        self.native_attributes
            .binary_search_by_key(&target_index, |entry| entry.target_index)
            .ok()
            .map(|index| &self.native_attributes[index].attributes)
    }
    #[cfg(test)]
    #[allow(dead_code)] // Unix-only Host owner test coverage.
    pub(crate) fn retained_bzl_module_count(&self) -> usize {
        self.retained_bzl_modules.len()
    }

    pub fn effective_visibility(&self, target: &PackageTarget) -> Option<RuleVisibility> {
        match &target.visibility {
            VisibilitySource::Declared(visibility) => Some(visibility.clone()),
            VisibilitySource::PackageDefault => Some(self.default_visibility.clone()),
            VisibilitySource::AlwaysPublic => Some(RuleVisibility::Public),
            VisibilitySource::GeneratingRule => {
                let PackageTargetKind::GeneratedFile {
                    generating_rule, ..
                } = &target.kind
                else {
                    return None;
                };
                let generating_rule = self
                    .targets
                    .iter()
                    .find(|candidate| candidate.name == generating_rule.as_str())?;
                self.effective_visibility(generating_rule)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct NativeTargetAttributes {
    pub target_index: u32,
    pub attributes: NativeRuleAttributes,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct PackageTarget {
    pub name: String,
    pub kind: PackageTargetKind,
    pub visibility: VisibilitySource,
}

impl PackageTarget {
    /// Returns the retained capability for a loadable rule. Native classes are
    /// fixed, compact values; non-rules intentionally have no capability.
    pub fn rule_capability(&self) -> Option<&RuleCapability> {
        self.kind.rule_capability()
    }

    pub fn test_metadata(&self) -> Option<TestMetadata> {
        self.kind.test_metadata()
    }

    pub fn visibility_explicit(&self) -> bool {
        matches!(self.visibility, VisibilitySource::Declared(_))
    }

    pub fn raw_visibility_labels(&self) -> &[CanonicalLabel] {
        match &self.visibility {
            VisibilitySource::Declared(visibility) => visibility.raw_declared_labels(),
            VisibilitySource::PackageDefault
            | VisibilitySource::GeneratingRule
            | VisibilitySource::AlwaysPublic => &[],
        }
    }
}

/// Immutable loading-time classification used by the deferred Stage 8
/// `executables()` projection. The class is the exported `.bzl` binding for a
/// Starlark rule, never a BUILD target name or implementation identity.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RuleCapability {
    pub rule_class: CompactString,
    pub executable: bool,
    pub test_kind: Option<TestRuleKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative)]
pub enum TestRuleKind {
    Test,
    Suite,
}

static FILEGROUP_RULE_CAPABILITY: RuleCapability = RuleCapability {
    rule_class: CompactString::const_new("filegroup"),
    executable: false,
    test_kind: None,
};
static ALIAS_RULE_CAPABILITY: RuleCapability = RuleCapability {
    rule_class: CompactString::const_new("alias"),
    executable: false,
    test_kind: None,
};
static CONFIG_SETTING_RULE_CAPABILITY: RuleCapability = RuleCapability {
    rule_class: CompactString::const_new("config_setting"),
    executable: false,
    test_kind: None,
};
static CONSTRAINT_SETTING_RULE_CAPABILITY: RuleCapability = RuleCapability {
    rule_class: CompactString::const_new("constraint_setting"),
    executable: false,
    test_kind: None,
};
static CONSTRAINT_VALUE_RULE_CAPABILITY: RuleCapability = RuleCapability {
    rule_class: CompactString::const_new("constraint_value"),
    executable: false,
    test_kind: None,
};
static PLATFORM_RULE_CAPABILITY: RuleCapability = RuleCapability {
    rule_class: CompactString::const_new("platform"),
    executable: false,
    test_kind: None,
};
static TOOLCHAIN_TYPE_RULE_CAPABILITY: RuleCapability = RuleCapability {
    rule_class: CompactString::const_new("toolchain_type"),
    executable: false,
    test_kind: None,
};
static TOOLCHAIN_RULE_CAPABILITY: RuleCapability = RuleCapability {
    rule_class: CompactString::const_new("toolchain"),
    executable: false,
    test_kind: None,
};
static TEST_SUITE_RULE_CAPABILITY: RuleCapability = RuleCapability {
    rule_class: CompactString::const_new("test_suite"),
    executable: false,
    test_kind: Some(TestRuleKind::Suite),
};

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct TestMetadata {
    pub tags: Arc<[CompactString]>,
    pub size: Option<CompactString>,
    pub manual: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum TestSuiteMembership {
    Explicit {
        tests: Arc<[CanonicalLabel]>,
    },
    Implicit {
        members: Arc<[CanonicalLabel]>,
        tests_explicit: bool,
    },
}

impl TestSuiteMembership {
    pub fn tests(&self) -> &[CanonicalLabel] {
        match self {
            Self::Explicit { tests } => tests,
            Self::Implicit { .. } => &[],
        }
    }

    pub fn implicit_tests(&self) -> &[CanonicalLabel] {
        match self {
            Self::Explicit { .. } => &[],
            Self::Implicit { members, .. } => members,
        }
    }

    pub fn tests_explicit(&self) -> bool {
        match self {
            Self::Explicit { .. } => true,
            Self::Implicit { tests_explicit, .. } => *tests_explicit,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum PackageTargetKind {
    ExportedFile,
    Filegroup {
        srcs: Arc<[CanonicalLabel]>,
        srcs_explicit: bool,
    },
    Alias {
        actual: CanonicalLabel,
    },
    /// Loading-only representation of Bazel's `config_setting`. Its values
    /// are retained for package semantic equality; configuration matching is
    /// intentionally owned by a later configured-analysis stage.
    ConfigSetting {
        values: Arc<[(CompactString, CompactString)]>,
    },
    NativeToolchain(NativeToolchainTarget),
    TestSuite {
        membership: TestSuiteMembership,
        tags: Arc<[CompactString]>,
    },
    PackageGroup {
        contents: Arc<PackageGroupContents>,
        includes: Arc<[CanonicalLabel]>,
    },
    /// A file declared by an `attr.output` or `attr.output_list` value.
    /// Its generator is retained explicitly; names alone cannot determine it.
    GeneratedFile {
        label: CanonicalLabel,
        generating_rule: CompactString,
    },
    /// A target declared by a Starlark `rule()` definition.
    ///
    /// Stage 4 records the declaration and retains the frozen implementation.
    /// Stage 6 owns evaluating it with a configured target context.
    StarlarkRule(StarlarkRuleImplementation),
}

impl PackageTargetKind {
    /// Stage 8's future projection boundary. `alias` remains a fixed native
    /// rule capability and never inherits the actual target's capability.
    fn rule_capability(&self) -> Option<&RuleCapability> {
        match self {
            Self::Filegroup { .. } => Some(&FILEGROUP_RULE_CAPABILITY),
            Self::Alias { .. } => Some(&ALIAS_RULE_CAPABILITY),
            Self::ConfigSetting { .. } => Some(&CONFIG_SETTING_RULE_CAPABILITY),
            Self::NativeToolchain(target) => Some(target.rule_capability()),
            Self::TestSuite { .. } => Some(&TEST_SUITE_RULE_CAPABILITY),
            Self::StarlarkRule(rule) => Some(&rule.capability),
            Self::ExportedFile | Self::GeneratedFile { .. } | Self::PackageGroup { .. } => None,
        }
    }

    fn test_metadata(&self) -> Option<TestMetadata> {
        match self {
            Self::TestSuite { tags, .. } => Some(TestMetadata {
                tags: tags.clone(),
                size: None,
                manual: tags.iter().any(|tag| tag == "manual"),
            }),
            Self::StarlarkRule(rule) if rule.is_test() => {
                let tags = rule
                    .value("tags")
                    .and_then(|value| match value.value.as_ref() {
                        CoercedAttributeValue::StringList(tags) => Some(tags.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| Arc::from([]));
                let size = rule
                    .value("size")
                    .and_then(|value| match value.value.as_ref() {
                        CoercedAttributeValue::String(size) => Some(size.clone()),
                        _ => None,
                    });
                Some(TestMetadata {
                    manual: tags.iter().any(|tag| tag == "manual"),
                    tags,
                    size,
                })
            }
            _ => None,
        }
    }
}

/// Loading-owned representation of the exact native target classes needed by
/// the accepted first-compatible toolchain fixture. Resolution remains a
/// later analysis-stage owner.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum NativeToolchainTarget {
    ConstraintSetting,
    ConstraintValue {
        constraint_setting: CanonicalLabel,
    },
    Platform {
        constraint_values: Arc<[CanonicalLabel]>,
    },
    ToolchainType,
    Toolchain {
        toolchain_type: CanonicalLabel,
        implementation: CanonicalLabel,
        exec_compatible_with: Arc<[CanonicalLabel]>,
    },
}

impl NativeToolchainTarget {
    pub fn rule_class(&self) -> &'static str {
        match self {
            Self::ConstraintSetting => "constraint_setting",
            Self::ConstraintValue { .. } => "constraint_value",
            Self::Platform { .. } => "platform",
            Self::ToolchainType => "toolchain_type",
            Self::Toolchain { .. } => "toolchain",
        }
    }

    fn rule_capability(&self) -> &'static RuleCapability {
        match self {
            Self::ConstraintSetting => &CONSTRAINT_SETTING_RULE_CAPABILITY,
            Self::ConstraintValue { .. } => &CONSTRAINT_VALUE_RULE_CAPABILITY,
            Self::Platform { .. } => &PLATFORM_RULE_CAPABILITY,
            Self::ToolchainType => &TOOLCHAIN_TYPE_RULE_CAPABILITY,
            Self::Toolchain { .. } => &TOOLCHAIN_RULE_CAPABILITY,
        }
    }
}

/// The frozen rule implementation retained for configured-target analysis.
/// The containing package keeps its source `.bzl` module alive.
#[derive(Debug, Clone, Allocative)]
pub struct StarlarkRuleImplementation {
    #[allocative(skip)]
    implementation: FrozenValue,
    dependencies: Arc<[CanonicalLabel]>,
    required_toolchains: Arc<[CanonicalLabel]>,
    schema: Arc<[AttributeSchema]>,
    values: Arc<[AttributeValue]>,
    capability: Arc<RuleCapability>,
    root_string_build_setting: bool,
}

impl PartialEq for StarlarkRuleImplementation {
    fn eq(&self, other: &Self) -> bool {
        // The frozen function is retained for Stage 6 lifetime only. Its heap
        // address is not package semantics and must not defeat DICE equality.
        self.dependencies == other.dependencies
            && self.required_toolchains == other.required_toolchains
            && self.schema == other.schema
            && self.values == other.values
            && self.capability == other.capability
            && self.root_string_build_setting == other.root_string_build_setting
    }
}

impl Eq for StarlarkRuleImplementation {}

impl StarlarkRuleImplementation {
    pub fn frozen_value(&self) -> FrozenValue {
        self.implementation
    }

    pub fn dependencies(&self) -> &[CanonicalLabel] {
        &self.dependencies
    }

    /// Toolchain-type requirements declared by the defining `rule()` call.
    /// These are loading-only retained metadata, not ordinary dependencies.
    pub fn required_toolchains(&self) -> &[CanonicalLabel] {
        &self.required_toolchains
    }

    pub fn schema(&self) -> &[AttributeSchema] {
        &self.schema
    }

    pub fn values(&self) -> &[AttributeValue] {
        &self.values
    }
    pub fn is_root_string_build_setting(&self) -> bool {
        self.root_string_build_setting
    }
    pub fn root_string_build_setting_default(&self) -> Option<&str> {
        self.root_string_build_setting.then(|| {
            self.value("build_setting_default")
                .and_then(|value| match value.value.as_ref() {
                    CoercedAttributeValue::String(value) => Some(value.as_str()),
                    _ => None,
                })
                .expect("string build setting has a string default")
        })
    }

    fn value(&self, name: &str) -> Option<&AttributeValue> {
        self.values
            .iter()
            .find(|value| value.declaration_name == name)
    }

    fn is_test(&self) -> bool {
        self.capability.test_kind == Some(TestRuleKind::Test)
    }
}

#[derive(Debug)]
struct PackageState {
    default_visibility: RuleVisibility,
    default_deprecation: Option<CompactString>,
    default_testonly: bool,
    default_package_metadata: Arc<[CanonicalLabel]>,
    licenses: Arc<[CompactString]>,
    targets: SmallMap<String, RecordedTarget>,
    used_globs: Vec<GlobSpec>,
}

impl Default for PackageState {
    fn default() -> Self {
        Self {
            default_visibility: RuleVisibility::Private,
            default_deprecation: None,
            default_testonly: false,
            default_package_metadata: Arc::from([]),
            licenses: Arc::from([]),
            targets: SmallMap::new(),
            used_globs: Vec::new(),
        }
    }
}

#[derive(Debug)]
struct RecordedTarget {
    kind: PackageTargetKind,
    visibility: VisibilitySource,
    native_overrides: Vec<NativeAttributeOverride>,
}

#[derive(Debug)]
struct NativeAttributeOverride {
    slot: usize,
    value: NativeAttributeValue,
}

#[derive(Debug)]
#[allow(dead_code)] // The Host branch remains dormant until its future package key lands.
enum PackageGlobSource {
    Listing(PackageListing),
    Host(HostGlobAttemptState),
}

#[derive(Debug)]
struct HostGlobAttemptState {
    prepared: Arc<SmallMap<HostGlobLoadingRequest, HostGlobPrepared>>,
    control: RefCell<Option<HostGlobAttemptControl>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostGlobAttemptControl {
    Pending(HostGlobLoadingRequest),
    Terminal(HostGlobAttemptError),
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostGlobAttemptError {
    Traversal(HostGlobRequestTraversalError),
    UnsupportedPath { path: Arc<[u8]> },
}

#[derive(Debug)]
struct HostGlobControlTransfer;

impl fmt::Display for HostGlobControlTransfer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("private Host glob attempt control transfer")
    }
}

impl std::error::Error for HostGlobControlTransfer {}

#[derive(Debug, ProvidesStaticType)]
pub(crate) struct PackageRecorder {
    glob_source: PackageGlobSource,
    package: CompactString,
    state: RefCell<PackageState>,
}

#[allow(dead_code)] // The Host attempt methods are exercised privately before activation.
impl PackageRecorder {
    pub(crate) fn new(listing: PackageListing, package: impl Into<CompactString>) -> Self {
        Self {
            glob_source: PackageGlobSource::Listing(listing),
            package: package.into(),
            state: RefCell::new(PackageState::default()),
        }
    }

    pub(crate) fn new_host(
        prepared: Arc<SmallMap<HostGlobLoadingRequest, HostGlobPrepared>>,
        package: impl Into<CompactString>,
    ) -> Self {
        Self {
            glob_source: PackageGlobSource::Host(HostGlobAttemptState {
                prepared,
                control: RefCell::new(None),
            }),
            package: package.into(),
            state: RefCell::new(PackageState::default()),
        }
    }

    pub(crate) fn take_host_glob_control(&self) -> Option<HostGlobAttemptControl> {
        match &self.glob_source {
            PackageGlobSource::Listing(_) => None,
            PackageGlobSource::Host(host) => host.control.borrow_mut().take(),
        }
    }

    pub(crate) fn is_host_glob_control_error(error: &starlark::Error) -> bool {
        matches!(
            error.kind(),
            starlark::ErrorKind::Native(error) if error.is::<HostGlobControlTransfer>()
        )
    }

    fn from_evaluator<'a>(eval: &'a Evaluator<'_, '_, '_>) -> anyhow::Result<&'a Self> {
        eval.extra
            .and_then(|extra| extra.downcast_ref::<Self>())
            .ok_or_else(|| anyhow::anyhow!("Bazel package global invoked without package state"))
    }

    fn for_glob<'a>(eval: &'a Evaluator<'_, '_, '_>) -> anyhow::Result<&'a Self> {
        eval.extra
            .and_then(|extra| extra.downcast_ref::<Self>())
            .ok_or_else(|| {
                anyhow::anyhow!("glob() may only be called while evaluating a BUILD package")
            })
    }

    fn set_default_visibility(&self, visibility: Vec<String>) -> anyhow::Result<()> {
        self.state.borrow_mut().default_visibility = self.parse_visibility(visibility)?;
        Ok(())
    }

    fn set_package_defaults(
        &self,
        visibility: Option<Vec<String>>,
        deprecation: Option<String>,
        testonly: Option<bool>,
        package_metadata: Option<Vec<String>>,
    ) -> anyhow::Result<()> {
        let mut state = self.state.borrow_mut();
        if let Some(visibility) = visibility {
            state.default_visibility = self.parse_visibility(visibility)?;
        }
        if let Some(deprecation) = deprecation {
            state.default_deprecation = Some(deprecation.into());
        }
        if let Some(testonly) = testonly {
            state.default_testonly = testonly;
        }
        if let Some(package_metadata) = package_metadata {
            state.default_package_metadata = package_metadata
                .iter()
                .map(|label| self.dependency_label(label))
                .collect::<anyhow::Result<Vec<_>>>()?
                .into();
        }
        Ok(())
    }

    fn set_licenses(&self, licenses: Vec<String>) {
        self.state.borrow_mut().licenses = licenses
            .into_iter()
            .map(CompactString::from)
            .collect::<Vec<_>>()
            .into();
    }

    fn exports_files(
        &self,
        srcs: Vec<String>,
        visibility: Option<Vec<String>>,
    ) -> anyhow::Result<()> {
        let visibility = self.visibility_source(visibility, VisibilitySource::AlwaysPublic)?;
        for src in srcs {
            self.record_target(src, PackageTargetKind::ExportedFile, visibility.clone())?;
        }
        Ok(())
    }

    fn filegroup(
        &self,
        name: String,
        srcs: Option<CoercedAttributeValue>,
        visibility: Option<Vec<String>>,
    ) -> anyhow::Result<()> {
        let srcs_explicit = srcs.is_some();
        let srcs_value = srcs.unwrap_or_else(empty_labels);
        let mut srcs = Vec::new();
        srcs_value.labels(&mut srcs);
        // A configurable `srcs` keeps each branch for query candidates while
        // the loading topology remains the flattened branch-value labels.
        // Keep the historical duplicate diagnostic for a literal list only;
        // labels may legitimately recur across mutually exclusive branches.
        if matches!(&srcs_value, CoercedAttributeValue::LabelList(_)) {
            reject_duplicate_canonical_labels(&srcs, "srcs", &name)?;
        }
        let srcs = srcs.into();
        let class = NativeRuleClass::Filegroup;
        let mut native_overrides = Vec::new();
        if srcs_explicit {
            native_overrides.push(NativeAttributeOverride {
                slot: class.slot("srcs").expect("filegroup schema").0,
                value: NativeAttributeValue {
                    provenance: AttributeProvenance::Explicit,
                    value: srcs_value.clone(),
                },
            });
        }
        let config_dependencies = selector_key_labels(&srcs_value);
        if !config_dependencies.is_empty() {
            native_overrides.push(NativeAttributeOverride {
                slot: class
                    .slot("$config_dependencies")
                    .expect("filegroup schema")
                    .0,
                value: NativeAttributeValue {
                    provenance: AttributeProvenance::Explicit,
                    value: CoercedAttributeValue::LabelList(config_dependencies.into()),
                },
            });
        }
        self.record_target(
            name.clone(),
            PackageTargetKind::Filegroup {
                srcs,
                srcs_explicit,
            },
            self.visibility_source(visibility, VisibilitySource::PackageDefault)?,
        )?;
        self.merge_native_overrides(&name, native_overrides)
    }

    fn test_suite(
        &self,
        name: String,
        tests: Option<Vec<String>>,
        mut tags: Vec<String>,
        visibility: Option<Vec<String>>,
    ) -> anyhow::Result<()> {
        let tests_explicit = tests.is_some();
        let mut tests = tests
            .unwrap_or_default()
            .iter()
            .map(|test| self.dependency_label(test))
            .collect::<anyhow::Result<Vec<_>>>()?;
        reject_duplicate_canonical_labels(&tests, "tests", &name)?;
        tests.sort_by(CanonicalLabel::bazel_natural_cmp);
        tags.sort_unstable();
        let membership = if tests.is_empty() {
            TestSuiteMembership::Implicit {
                members: Arc::from([]),
                tests_explicit,
            }
        } else {
            TestSuiteMembership::Explicit {
                tests: tests.into(),
            }
        };
        self.record_target(
            name,
            PackageTargetKind::TestSuite {
                membership,
                tags: tags
                    .into_iter()
                    .map(CompactString::from)
                    .collect::<Vec<_>>()
                    .into(),
            },
            self.visibility_source(visibility, VisibilitySource::PackageDefault)?,
        )
    }

    fn alias(
        &self,
        name: String,
        actual: String,
        visibility: Option<Vec<String>>,
    ) -> anyhow::Result<()> {
        let actual = self.dependency_label(&actual)?;
        self.record_target(
            name,
            PackageTargetKind::Alias { actual },
            self.visibility_source(visibility, VisibilitySource::PackageDefault)?,
        )
    }

    fn config_setting(
        &self,
        name: String,
        values: SmallMap<String, String>,
        visibility: Option<Vec<String>>,
    ) -> anyhow::Result<()> {
        let values = values
            .into_iter()
            .map(|(key, value)| (CompactString::from(key), CompactString::from(value)))
            .collect::<Vec<_>>();
        self.record_target(
            name,
            PackageTargetKind::ConfigSetting {
                values: values.into(),
            },
            self.visibility_source(visibility, VisibilitySource::AlwaysPublic)?,
        )
    }

    fn native_toolchain_target(
        &self,
        name: String,
        target: NativeToolchainTarget,
    ) -> anyhow::Result<()> {
        self.native_toolchain_target_with_visibility(name, target, None)
    }

    fn native_toolchain_target_with_visibility(
        &self,
        name: String,
        target: NativeToolchainTarget,
        visibility: Option<Vec<String>>,
    ) -> anyhow::Result<()> {
        self.record_target(
            name,
            PackageTargetKind::NativeToolchain(target),
            self.visibility_source(visibility, VisibilitySource::PackageDefault)?,
        )
    }

    fn native_toolchain_label(&self, value: &str) -> anyhow::Result<CanonicalLabel> {
        let target = value.rsplit_once(':').map(|(_, target)| target);
        let recursive = target.is_none() && (value == "..." || value.ends_with("/..."));
        if recursive || matches!(target, Some("all" | "all-targets" | "*")) {
            anyhow::bail!("native toolchain declarations require direct target labels")
        }
        self.dependency_label(value)
    }

    fn native_toolchain_labels(&self, values: &[&str]) -> anyhow::Result<Arc<[CanonicalLabel]>> {
        values
            .iter()
            .map(|value| self.native_toolchain_label(value))
            .collect::<anyhow::Result<Vec<_>>>()
            .map(Arc::from)
    }

    fn package_group(
        &self,
        name: String,
        packages: Vec<String>,
        includes: Vec<String>,
    ) -> anyhow::Result<()> {
        let contents = Arc::new(PackageGroupContents::from_package_specs(&packages)?);
        let includes = includes
            .iter()
            .map(|include| self.dependency_label(include))
            .collect::<anyhow::Result<Vec<_>>>()?;
        self.record_target(
            name,
            PackageTargetKind::PackageGroup {
                contents,
                includes: includes.into(),
            },
            VisibilitySource::AlwaysPublic,
        )
    }

    fn starlark_rule(
        &self,
        name: String,
        implementation: FrozenValue,
        required_toolchains: Arc<[CanonicalLabel]>,
        capability: Arc<RuleCapability>,
        schema: Arc<[AttributeSchema]>,
        values: Arc<[AttributeValue]>,
        root_string_build_setting: bool,
        visibility: Option<Vec<String>>,
    ) -> anyhow::Result<()> {
        let mut dependencies = Vec::new();
        for value in values.iter() {
            if let CoercedAttributeValue::LabelList(labels) = value.value.as_ref() {
                reject_duplicate_canonical_labels(labels, &value.declaration_name, &name)?;
            }
            let schema = schema
                .iter()
                .find(|schema| schema.declaration_name() == value.declaration_name);
            if schema
                .is_some_and(|schema| schema.dependency_reachable() && schema.ordinary_dependency())
            {
                value.value.labels(&mut dependencies);
            }
        }
        // Existing analysis/query consumers use this aggregate. It is derived
        // after structured values are retained, and selector keys never enter.
        let mut seen = SmallSet::new();
        dependencies.retain(|label| seen.insert(label.clone()));
        self.record_target(
            name,
            PackageTargetKind::StarlarkRule(StarlarkRuleImplementation {
                implementation,
                dependencies: dependencies.into(),
                required_toolchains,
                schema,
                values,
                capability,
                root_string_build_setting,
            }),
            self.visibility_source(visibility, VisibilitySource::PackageDefault)?,
        )
    }

    fn dependency_label(&self, value: &str) -> anyhow::Result<CanonicalLabel> {
        package_context_label(&self.package, value)
    }

    fn parse_visibility(&self, values: Vec<String>) -> anyhow::Result<RuleVisibility> {
        RuleVisibility::from_declared_labels(
            values
                .iter()
                .map(|value| self.dependency_label(value))
                .collect::<anyhow::Result<Vec<_>>>()?,
        )
    }

    fn visibility_source(
        &self,
        values: Option<Vec<String>>,
        omitted: VisibilitySource,
    ) -> anyhow::Result<VisibilitySource> {
        values
            .map(|values| {
                self.parse_visibility(values)
                    .map(VisibilitySource::Declared)
            })
            .unwrap_or(Ok(omitted))
    }

    fn record_target(
        &self,
        name: String,
        kind: PackageTargetKind,
        visibility: VisibilitySource,
    ) -> anyhow::Result<()> {
        let mut state = self.state.borrow_mut();
        if state.targets.get(&name).is_some() {
            anyhow::bail!("target '{name}' declared more than once");
        }
        state.targets.insert(
            name,
            RecordedTarget {
                kind,
                visibility,
                native_overrides: Vec::new(),
            },
        );
        Ok(())
    }

    fn set_native_overrides<'v>(
        &self,
        name: &str,
        kwargs: SmallMap<String, Value<'v>>,
    ) -> anyhow::Result<()> {
        let (class, rule_class) = {
            let state = self.state.borrow();
            let target = state
                .targets
                .get(name)
                .ok_or_else(|| anyhow::anyhow!("native rule '{name}' was not recorded"))?;
            let class = native_rule_class(&target.kind)
                .ok_or_else(|| anyhow::anyhow!("target '{name}' is not a native rule"))?;
            let rule_class = target
                .kind
                .rule_capability()
                .expect("native rule")
                .rule_class
                .clone();
            (class, rule_class)
        };
        let overrides = coerce_native_overrides(self, class, kwargs, &rule_class)?;
        self.merge_native_overrides(name, overrides)
    }

    fn merge_native_overrides(
        &self,
        name: &str,
        overrides: Vec<NativeAttributeOverride>,
    ) -> anyhow::Result<()> {
        let mut state = self.state.borrow_mut();
        let existing = &mut state
            .targets
            .get_mut(name)
            .expect("target was checked above")
            .native_overrides;
        for override_value in overrides {
            if let Some(existing_value) = existing
                .iter_mut()
                .find(|value| value.slot == override_value.slot)
            {
                *existing_value = override_value;
            } else {
                existing.push(override_value);
            }
        }
        Ok(())
    }

    fn set_native_generator_from_evaluator(
        &self,
        name: &str,
        eval: &Evaluator<'_, '_, '_>,
    ) -> anyhow::Result<()> {
        let Some(context) = eval.native_call_context("name") else {
            return Ok(());
        };
        let position = context.call_location.resolve_span_for_reporting().begin;
        let build_file = Path::new(context.call_location.filename())
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("BUILD.bazel");
        let build_file = if self.package.is_empty() {
            build_file.to_owned()
        } else {
            format!("{}/{build_file}", self.package)
        };
        let generator_location =
            format!("{build_file}:{}:{}", position.line + 1, position.column + 1);
        let mut state = self.state.borrow_mut();
        let target = state
            .targets
            .get_mut(name)
            .ok_or_else(|| anyhow::anyhow!("native rule '{name}' was not recorded"))?;
        let class = native_rule_class(&target.kind)
            .ok_or_else(|| anyhow::anyhow!("target '{name}' is not a native rule"))?;
        let overrides = [
            (
                "generator_name",
                CoercedAttributeValue::String(context.local_value.unwrap_or_default().into()),
            ),
            (
                "generator_function",
                CoercedAttributeValue::String(context.function_name.into()),
            ),
            (
                "generator_location",
                CoercedAttributeValue::String(generator_location.into()),
            ),
        ];
        for (attribute, value) in overrides {
            let (slot, schema) = class
                .slot(attribute)
                .expect("all native RuleClasses retain generator metadata");
            debug_assert_eq!(schema.policy(), NativeAttributePolicy::Callable);
            let override_value = NativeAttributeOverride {
                slot,
                value: NativeAttributeValue {
                    provenance: AttributeProvenance::Implicit,
                    value,
                },
            };
            if let Some(existing) = target
                .native_overrides
                .iter_mut()
                .find(|value| value.slot == slot)
            {
                *existing = override_value;
            } else {
                target.native_overrides.push(override_value);
            }
        }
        Ok(())
    }

    fn generated_file(&self, label: CanonicalLabel, generating_rule: &str) -> anyhow::Result<()> {
        let name = label.target().to_string();
        self.record_target(
            name,
            PackageTargetKind::GeneratedFile {
                label,
                generating_rule: generating_rule.into(),
            },
            VisibilitySource::GeneratingRule,
        )
    }

    fn glob(&self, spec: GlobSpec) -> anyhow::Result<Vec<String>> {
        let matches = match &self.glob_source {
            PackageGlobSource::Listing(listing) => expand_glob(listing, &spec)?,
            PackageGlobSource::Host(host) => self.host_glob(host, &spec)?,
        };
        self.state.borrow_mut().used_globs.push(spec);
        Ok(matches)
    }

    fn host_glob(
        &self,
        host: &HostGlobAttemptState,
        spec: &GlobSpec,
    ) -> anyhow::Result<Vec<String>> {
        let operation = if spec.exclude_directories {
            HostGlobLoadingOperation::Files
        } else {
            HostGlobLoadingOperation::FilesAndDirs
        };
        let mut include_matched = Vec::with_capacity(spec.includes.len());
        let mut matches = Vec::new();
        for pattern in spec.includes.iter() {
            let paths = self.host_glob_request(host, pattern.as_bytes(), operation)?;
            include_matched.push(!paths.is_empty());
            matches.extend(paths);
        }
        let mut excluded = SmallSet::new();
        for pattern in spec.excludes.iter() {
            excluded.extend(self.host_glob_request(host, pattern.as_bytes(), operation)?);
        }

        if !spec.allow_empty {
            if let Some((index, _)) = include_matched
                .iter()
                .enumerate()
                .find(|(_, matched)| !**matched)
            {
                return Err(GlobError::EmptyPattern {
                    pattern: spec.includes[index].to_string(),
                }
                .into());
            }
        }

        matches.retain(|path| !excluded.contains(path));
        matches.sort_unstable();
        matches.dedup();
        if !spec.allow_empty && matches.is_empty() {
            return Err(GlobError::AllExcluded.into());
        }
        Ok(matches)
    }

    fn host_glob_request(
        &self,
        host: &HostGlobAttemptState,
        pattern: &[u8],
        operation: HostGlobLoadingOperation,
    ) -> anyhow::Result<Vec<String>> {
        let request = HostGlobLoadingRequest::new(Arc::<[u8]>::from(pattern), operation);
        let Some(prepared) = host.prepared.get(&request) else {
            return self.transfer_host_glob(host, HostGlobAttemptControl::Pending(request));
        };
        let matches = match prepared.as_ref() {
            Ok(matches) => matches,
            Err(error) => {
                return self.transfer_host_glob(
                    host,
                    HostGlobAttemptControl::Terminal(HostGlobAttemptError::Traversal(
                        error.clone(),
                    )),
                );
            }
        };
        matches
            .paths()
            .iter()
            .map(|path| {
                let value = match std::str::from_utf8(path) {
                    Ok(value) => value,
                    Err(_) => {
                        return self.transfer_host_glob(
                            host,
                            HostGlobAttemptControl::Terminal(
                                HostGlobAttemptError::UnsupportedPath { path: path.clone() },
                            ),
                        );
                    }
                };
                Ok(if value.starts_with('@') {
                    format!(":{value}")
                } else {
                    value.to_owned()
                })
            })
            .collect()
    }

    fn transfer_host_glob<T>(
        &self,
        host: &HostGlobAttemptState,
        control: HostGlobAttemptControl,
    ) -> anyhow::Result<T> {
        let previous = host.control.borrow_mut().replace(control);
        if previous.is_some() {
            anyhow::bail!("Host glob attempt produced more than one control transfer");
        }
        Err(HostGlobControlTransfer.into())
    }

    pub(crate) fn finish(
        self,
        package_dir: PathBuf,
        build_file: PathBuf,
        direct_load_roots: Arc<[BzlModuleIdentity]>,
        reachable_loads: Arc<[BzlModuleIdentity]>,
        load_fingerprint: [u8; 32],
        retained_bzl_modules: Arc<[FrozenBzlLifetimeEntry]>,
    ) -> LoadedPackage {
        if let PackageGlobSource::Host(host) = &self.glob_source {
            debug_assert!(host.control.borrow().is_none());
        }
        let mut state = self.state.into_inner();
        let mut implicit_candidates = state
            .targets
            .iter()
            .filter_map(|(name, target)| match &target.kind {
                PackageTargetKind::StarlarkRule(rule) if rule.is_test() => {
                    target.kind.test_metadata().map(|metadata| {
                        (
                            package_context_label(&self.package, name)
                                .expect("recorded target names are valid package-context labels"),
                            metadata,
                        )
                    })
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        implicit_candidates.sort_by(|(left, _), (right, _)| left.bazel_natural_cmp(right));
        for (_, target) in state.targets.iter_mut() {
            if let PackageTargetKind::TestSuite {
                membership: TestSuiteMembership::Implicit { members, .. },
                tags,
            } = &mut target.kind
            {
                *members = implicit_candidates
                    .iter()
                    .filter(|(_, metadata)| implicit_test_matches_suite(metadata, tags))
                    .map(|(label, _)| label.clone())
                    .collect::<Vec<_>>()
                    .into();
            }
        }
        let native_attributes = state
            .targets
            .iter()
            .enumerate()
            .filter_map(|(target_index, (name, target))| {
                native_rule_attributes(name, &target.kind, &target.visibility, &state).map(
                    |mut attributes| {
                        for override_value in &target.native_overrides {
                            attributes.values_mut()[override_value.slot] =
                                override_value.value.clone();
                        }
                        NativeTargetAttributes {
                            target_index: u32::try_from(target_index)
                                .expect("package target count exceeds u32"),
                            attributes,
                        }
                    },
                )
            })
            .collect::<Vec<_>>()
            .into();
        LoadedPackage {
            package_dir,
            build_file,
            default_visibility: state.default_visibility,
            targets: state
                .targets
                .into_iter()
                .map(|(name, target)| PackageTarget {
                    name,
                    kind: target.kind,
                    visibility: target.visibility,
                })
                .collect(),
            native_attributes,
            used_globs: state.used_globs,
            direct_load_roots,
            reachable_loads,
            load_fingerprint,
            retained_bzl_modules,
        }
    }
}

fn native_rule_class(kind: &PackageTargetKind) -> Option<NativeRuleClass> {
    Some(match kind {
        PackageTargetKind::Filegroup { .. } => NativeRuleClass::Filegroup,
        PackageTargetKind::Alias { .. } => NativeRuleClass::Alias,
        PackageTargetKind::ConfigSetting { .. } => NativeRuleClass::ConfigSetting,
        PackageTargetKind::TestSuite { .. } => NativeRuleClass::TestSuite,
        PackageTargetKind::NativeToolchain(native) => match native {
            NativeToolchainTarget::ConstraintSetting => NativeRuleClass::ConstraintSetting,
            NativeToolchainTarget::ConstraintValue { .. } => NativeRuleClass::ConstraintValue,
            NativeToolchainTarget::Platform { .. } => NativeRuleClass::Platform,
            NativeToolchainTarget::ToolchainType => NativeRuleClass::ToolchainType,
            NativeToolchainTarget::Toolchain { .. } => NativeRuleClass::Toolchain,
        },
        _ => return None,
    })
}

fn empty_labels() -> CoercedAttributeValue {
    CoercedAttributeValue::LabelList(Arc::from([]))
}

fn empty_strings() -> CoercedAttributeValue {
    CoercedAttributeValue::StringList(Arc::from([]))
}

fn visibility_value(visibility: &RuleVisibility) -> CoercedAttributeValue {
    let labels: Arc<[CanonicalLabel]> = match visibility {
        RuleVisibility::Public => {
            Arc::from([CanonicalLabel::parse("@@//visibility:public").unwrap()])
        }
        RuleVisibility::Private => {
            Arc::from([CanonicalLabel::parse("@@//visibility:private").unwrap()])
        }
        RuleVisibility::Restricted(value) => Arc::from(value.declared_labels()),
    };
    CoercedAttributeValue::LabelList(labels)
}

fn native_default(schema: NativeAttributeSchema) -> NativeAttributeValue {
    let provenance = match schema.policy() {
        NativeAttributePolicy::Callable => AttributeProvenance::Default,
        NativeAttributePolicy::Implicit | NativeAttributePolicy::Forced => {
            AttributeProvenance::Implicit
        }
    };
    let value = match schema.kind() {
        AttributeKind::Label | AttributeKind::Output => CoercedAttributeValue::None,
        AttributeKind::LabelList => empty_labels(),
        AttributeKind::String => CoercedAttributeValue::String(CompactString::default()),
        AttributeKind::StringList => empty_strings(),
        AttributeKind::Boolean => CoercedAttributeValue::Boolean(false),
        AttributeKind::Integer => CoercedAttributeValue::Integer(0),
        AttributeKind::StringDict => CoercedAttributeValue::StringDict(Arc::from([])),
        AttributeKind::StringKeyedLabelDict => {
            CoercedAttributeValue::StringKeyedLabelDict(Arc::from([]))
        }
        AttributeKind::LabelKeyedStringDict => {
            CoercedAttributeValue::LabelKeyedStringDict(Arc::from([]))
        }
        AttributeKind::LabelListDict => CoercedAttributeValue::LabelListDict(Arc::from([])),
        AttributeKind::OutputList => CoercedAttributeValue::OutputList(Arc::from([])),
    };
    NativeAttributeValue { provenance, value }
}

fn set_native_value(
    class: NativeRuleClass,
    values: &mut [NativeAttributeValue],
    name: &str,
    provenance: AttributeProvenance,
    value: CoercedAttributeValue,
) {
    let (slot, _) = class
        .slot(name)
        .unwrap_or_else(|| panic!("{class:?} does not declare native attribute '{name}'"));
    values[slot] = NativeAttributeValue { provenance, value };
}

fn set_native_value_if_present(
    class: NativeRuleClass,
    values: &mut [NativeAttributeValue],
    name: &str,
    provenance: AttributeProvenance,
    value: CoercedAttributeValue,
) {
    if let Some((slot, _)) = class.slot(name) {
        values[slot] = NativeAttributeValue { provenance, value };
    }
}

/// Native values are stored in their class's static Bazel RuleClass order.
/// They do not affect the aggregate dependency list used by traversal.
fn native_rule_attributes(
    target_name: &str,
    kind: &PackageTargetKind,
    visibility_source: &VisibilitySource,
    package: &PackageState,
) -> Option<NativeRuleAttributes> {
    let class = native_rule_class(kind)?;
    let mut values = class
        .schema()
        .iter()
        .copied()
        .map(native_default)
        .collect::<Vec<_>>();
    let class_visibility = match visibility_source {
        VisibilitySource::Declared(value) => value,
        VisibilitySource::PackageDefault => &package.default_visibility,
        VisibilitySource::AlwaysPublic | VisibilitySource::GeneratingRule => {
            &RuleVisibility::Public
        }
    };
    let visibility_provenance = if matches!(visibility_source, VisibilitySource::Declared(_)) {
        AttributeProvenance::Explicit
    } else {
        AttributeProvenance::Default
    };

    set_native_value(
        class,
        &mut values,
        "name",
        AttributeProvenance::Explicit,
        CoercedAttributeValue::String(target_name.into()),
    );
    set_native_value(
        class,
        &mut values,
        "visibility",
        visibility_provenance,
        visibility_value(class_visibility),
    );
    set_native_value(
        class,
        &mut values,
        "deprecation",
        AttributeProvenance::Default,
        package
            .default_deprecation
            .clone()
            .map(CoercedAttributeValue::String)
            .unwrap_or(CoercedAttributeValue::None),
    );
    set_native_value(
        class,
        &mut values,
        "testonly",
        AttributeProvenance::Default,
        CoercedAttributeValue::Boolean(package.default_testonly),
    );
    set_native_value_if_present(
        class,
        &mut values,
        "package_metadata",
        AttributeProvenance::Default,
        CoercedAttributeValue::LabelList(package.default_package_metadata.clone()),
    );
    set_native_value_if_present(
        class,
        &mut values,
        "licenses",
        AttributeProvenance::Default,
        CoercedAttributeValue::StringList(package.licenses.clone()),
    );

    match kind {
        PackageTargetKind::Filegroup {
            srcs,
            srcs_explicit,
        } => set_native_value(
            class,
            &mut values,
            "srcs",
            if *srcs_explicit {
                AttributeProvenance::Explicit
            } else {
                AttributeProvenance::Default
            },
            CoercedAttributeValue::LabelList(srcs.clone()),
        ),
        PackageTargetKind::Alias { actual } => set_native_value(
            class,
            &mut values,
            "actual",
            AttributeProvenance::Explicit,
            CoercedAttributeValue::Label(actual.clone()),
        ),
        PackageTargetKind::ConfigSetting {
            values: setting_values,
        } => {
            set_native_value(
                class,
                &mut values,
                "tags",
                AttributeProvenance::Implicit,
                CoercedAttributeValue::StringList(Arc::from([CompactString::const_new("manual")])),
            );
            set_native_value(
                class,
                &mut values,
                "licenses",
                AttributeProvenance::Implicit,
                CoercedAttributeValue::StringList(Arc::from([CompactString::const_new("none")])),
            );
            set_native_value(
                class,
                &mut values,
                "values",
                AttributeProvenance::Explicit,
                CoercedAttributeValue::StringDict(setting_values.clone()),
            );
        }
        PackageTargetKind::TestSuite { membership, tags } => {
            set_native_value(
                class,
                &mut values,
                "tags",
                AttributeProvenance::Explicit,
                CoercedAttributeValue::StringList(tags.clone()),
            );
            set_native_value(
                class,
                &mut values,
                "testonly",
                AttributeProvenance::Implicit,
                CoercedAttributeValue::Boolean(true),
            );
            set_native_value(
                class,
                &mut values,
                "tests",
                if membership.tests_explicit() {
                    AttributeProvenance::Explicit
                } else {
                    AttributeProvenance::Default
                },
                CoercedAttributeValue::LabelList(Arc::from(membership.tests())),
            );
            set_native_value(
                class,
                &mut values,
                "$implicit_tests",
                AttributeProvenance::Implicit,
                CoercedAttributeValue::LabelList(Arc::from(membership.implicit_tests())),
            );
        }
        PackageTargetKind::NativeToolchain(native) => {
            if !matches!(native, NativeToolchainTarget::ToolchainType) {
                set_native_value(
                    class,
                    &mut values,
                    "tags",
                    AttributeProvenance::Implicit,
                    CoercedAttributeValue::StringList(Arc::from([CompactString::const_new(
                        "manual",
                    )])),
                );
            }
            match native {
                NativeToolchainTarget::ConstraintSetting => {}
                NativeToolchainTarget::ConstraintValue { constraint_setting } => {
                    set_native_value(
                        class,
                        &mut values,
                        "constraint_setting",
                        AttributeProvenance::Explicit,
                        CoercedAttributeValue::Label(constraint_setting.clone()),
                    );
                }
                NativeToolchainTarget::Platform { constraint_values } => {
                    set_native_value(
                        class,
                        &mut values,
                        "constraint_values",
                        AttributeProvenance::Explicit,
                        CoercedAttributeValue::LabelList(constraint_values.clone()),
                    );
                    set_native_value(
                        class,
                        &mut values,
                        "missing_toolchain_error",
                        AttributeProvenance::Default,
                        CoercedAttributeValue::String(CompactString::new(
                            "For more information on platforms or toolchains see https://bazel.build/concepts/platforms-intro.",
                        )),
                    );
                }
                NativeToolchainTarget::ToolchainType => {}
                NativeToolchainTarget::Toolchain {
                    toolchain_type,
                    implementation,
                    exec_compatible_with,
                } => {
                    set_native_value(
                        class,
                        &mut values,
                        "toolchain_type",
                        AttributeProvenance::Explicit,
                        CoercedAttributeValue::Label(toolchain_type.clone()),
                    );
                    set_native_value(
                        class,
                        &mut values,
                        "toolchain",
                        AttributeProvenance::Explicit,
                        CoercedAttributeValue::Label(implementation.clone()),
                    );
                    set_native_value(
                        class,
                        &mut values,
                        "exec_compatible_with",
                        AttributeProvenance::Explicit,
                        CoercedAttributeValue::LabelList(exec_compatible_with.clone()),
                    );
                }
            }
        }
        _ => unreachable!("native class was selected above"),
    }

    Some(NativeRuleAttributes::new(class, values))
}
fn implicit_test_matches_suite(metadata: &TestMetadata, suite_tags: &[CompactString]) -> bool {
    if metadata.manual {
        return false;
    }
    suite_tags.iter().all(|filter| {
        if filter == "manual" {
            return true;
        }
        let (excluded, required) = match filter.strip_prefix('-') {
            Some(required) => (true, required),
            None => (false, filter.strip_prefix('+').unwrap_or(filter)),
        };
        let present = metadata.tags.iter().any(|tag| tag == required)
            || metadata.size.as_deref() == Some(required);
        if excluded { !present } else { present }
    })
}

fn reject_duplicate_canonical_labels(
    labels: &[CanonicalLabel],
    attribute: &str,
    rule: &str,
) -> anyhow::Result<()> {
    let mut seen = SmallSet::new();
    for label in labels {
        let package = label.package();
        let identity = (
            package.repo().as_str(),
            package.package().as_str(),
            label.target().as_str(),
        );
        if seen.insert(identity) {
            continue;
        }
        let display = if package.repo().is_root() {
            format!("//{}:{}", package.package(), label.target())
        } else {
            label.to_string()
        };
        anyhow::bail!(
            "Label '{display}' is duplicated in the '{attribute}' attribute of rule '{rule}'"
        );
    }
    Ok(())
}

fn list(items: UnpackListOrTuple<&str>) -> Vec<String> {
    items.items.into_iter().map(str::to_owned).collect()
}

pub(crate) fn package_context_label(
    base_package: &str,
    raw: &str,
) -> anyhow::Result<CanonicalLabel> {
    if raw.starts_with('@') {
        anyhow::bail!(
            "external repository dependency labels are not supported in this analysis packet: {raw}"
        );
    }
    let without_root = raw.strip_prefix("//").unwrap_or(raw);
    let package_part = without_root
        .split_once(':')
        .map_or(without_root, |(package, _)| package);
    if package_part == "..." || package_part.ends_with("/...") {
        anyhow::bail!("invalid label '{raw}': package name cannot contain '...'");
    }
    let canonical = if let Some(target) = raw.strip_prefix(':') {
        format!("@@//{base_package}:{target}")
    } else if let Some(absolute) = raw.strip_prefix("//") {
        format!("@@//{absolute}")
    } else {
        if raw.contains(':') {
            anyhow::bail!("invalid label '{raw}': absolute label must begin with '@' or '//'");
        }
        format!("@@//{base_package}:{raw}")
    };
    CanonicalLabel::parse(&canonical).map_err(anyhow::Error::msg)
}

fn package_output_label(base_package: &str, raw: &str) -> anyhow::Result<CanonicalLabel> {
    let label = package_context_label(base_package, raw).map_err(|_| {
        anyhow::anyhow!("output label must name a valid target in this package: {raw}")
    })?;
    if label.package().package().as_str() != base_package || !label.package().repo().is_root() {
        anyhow::bail!("output label must name a valid target in this package: {raw}");
    }
    Ok(label)
}

fn rule_toolchain_requirement(
    values: Option<UnpackList<&str>>,
    eval: &Evaluator<'_, '_, '_>,
) -> anyhow::Result<Arc<[CanonicalLabel]>> {
    let values = values.map_or_else(Vec::new, |values| values.items);
    if values.is_empty() {
        return Ok(Arc::from([]));
    }
    let context = BzlEvaluationContext::from_evaluator(eval)?;
    let source = CanonicalLabel::parse(&format!("@@{}", context.source_label()))
        .map_err(anyhow::Error::msg)?;
    values
        .iter()
        .map(|value| {
            let target = value.rsplit_once(':').map(|(_, target)| target);
            let recursive = target.is_none() && (*value == "..." || value.ends_with("/..."));
            if recursive || matches!(target, Some("all" | "all-targets" | "*")) {
                anyhow::bail!("rule(toolchains = ...) requires a direct target label: {value}");
            }
            package_context_label(source.package().package().as_str(), value)
        })
        .collect::<anyhow::Result<Vec<_>>>()
        .map(Into::into)
}

fn package_global(
    default_visibility: Option<UnpackListOrTuple<&str>>,
    default_deprecation: Option<&str>,
    default_testonly: Option<bool>,
    default_package_metadata: Option<UnpackListOrTuple<&str>>,
    eval: &mut Evaluator,
) -> anyhow::Result<NoneType> {
    PackageRecorder::from_evaluator(eval)?.set_package_defaults(
        default_visibility.map(list),
        default_deprecation.map(ToOwned::to_owned),
        default_testonly,
        default_package_metadata.map(list),
    )?;
    Ok(NoneType)
}

fn licenses_global(
    licenses: UnpackListOrTuple<&str>,
    eval: &mut Evaluator,
) -> anyhow::Result<NoneType> {
    PackageRecorder::from_evaluator(eval)?.set_licenses(list(licenses));
    Ok(NoneType)
}

fn exports_files_global(
    srcs: UnpackListOrTuple<&str>,
    visibility: Option<UnpackListOrTuple<&str>>,
    eval: &mut Evaluator,
) -> anyhow::Result<NoneType> {
    PackageRecorder::from_evaluator(eval)?.exports_files(list(srcs), visibility.map(list))?;
    Ok(NoneType)
}

fn filegroup_global<'v>(
    name: &str,
    srcs: Option<Value<'v>>,
    visibility: Option<UnpackListOrTuple<&str>>,
    eval: &mut Evaluator<'v, '_, '_>,
) -> anyhow::Result<NoneType> {
    let recorder = PackageRecorder::from_evaluator(eval)?;
    let srcs = srcs
        .map(|srcs| coerce_starlark_value(recorder, AttributeKind::LabelList, "srcs", true, srcs))
        .transpose()?;
    recorder.filegroup(name.to_owned(), srcs, visibility.map(list))?;
    recorder.set_native_generator_from_evaluator(name, eval)?;
    Ok(NoneType)
}

fn test_suite_global(
    name: &str,
    tests: Option<UnpackListOrTuple<&str>>,
    tags: UnpackListOrTuple<&str>,
    visibility: Option<UnpackListOrTuple<&str>>,
    eval: &mut Evaluator,
) -> anyhow::Result<NoneType> {
    let recorder = PackageRecorder::from_evaluator(eval)?;
    recorder.test_suite(
        name.to_owned(),
        tests.map(list),
        list(tags),
        visibility.map(list),
    )?;
    recorder.set_native_generator_from_evaluator(name, eval)?;
    Ok(NoneType)
}

fn alias_global(
    name: &str,
    actual: &str,
    visibility: Option<UnpackListOrTuple<&str>>,
    eval: &mut Evaluator,
) -> anyhow::Result<NoneType> {
    let recorder = PackageRecorder::from_evaluator(eval)?;
    recorder.alias(name.to_owned(), actual.to_owned(), visibility.map(list))?;
    recorder.set_native_generator_from_evaluator(name, eval)?;
    Ok(NoneType)
}

fn glob_global<'v>(
    include: UnpackListOrTuple<&str>,
    exclude: UnpackListOrTuple<&str>,
    exclude_directories: i32,
    allow_empty: Option<Value<'v>>,
    eval: &mut Evaluator<'v, '_, '_>,
) -> anyhow::Result<Vec<String>> {
    let allow_empty = match allow_empty {
        None => false,
        Some(value) => value.unpack_bool().ok_or_else(|| {
            anyhow::anyhow!(
                "expected boolean for argument `allow_empty`, got `{}`",
                value
            )
        })?,
    };
    let spec = GlobSpec::new(
        include.items,
        exclude.items,
        exclude_directories != 0,
        allow_empty,
    )?;
    PackageRecorder::for_glob(eval)?.glob(spec)
}

fn raw_attribute_value(value: Value) -> anyhow::Result<RawAttributeValue> {
    if let Some(value) = value.unpack_str() {
        return Ok(RawAttributeValue::String(value.into()));
    }
    if let Some(value) = value.unpack_bool() {
        return Ok(RawAttributeValue::Boolean(value));
    }
    if let Some(value) = value.unpack_i32() {
        return Ok(RawAttributeValue::Integer(value));
    }
    if let Some(values) = ListRef::from_value(value) {
        return values
            .iter()
            .map(raw_attribute_value)
            .collect::<anyhow::Result<Vec<_>>>()
            .map(|values| RawAttributeValue::List(values.into()));
    }
    if let Some(values) = DictRef::from_value(value) {
        return values
            .iter()
            .map(|(key, value)| Ok((raw_attribute_value(key)?, raw_attribute_value(value)?)))
            .collect::<anyhow::Result<Vec<_>>>()
            .map(|values| RawAttributeValue::Dict(values.into()));
    }
    anyhow::bail!(
        "attribute values must contain strings, booleans, integers, lists, or dictionaries"
    )
}

fn raw_string(value: &RawAttributeValue, context: &str) -> anyhow::Result<CompactString> {
    match value {
        RawAttributeValue::String(value) => Ok(value.clone()),
        _ => anyhow::bail!("attribute `{context}` must be a string"),
    }
}

fn raw_label(
    base_package: &str,
    value: &RawAttributeValue,
    context: &str,
) -> anyhow::Result<CanonicalLabel> {
    package_context_label(base_package, &raw_string(value, context)?)
}

fn raw_output(
    base_package: &str,
    value: &RawAttributeValue,
    context: &str,
) -> anyhow::Result<CanonicalLabel> {
    let raw = raw_string(value, context)?;
    package_output_label(base_package, &raw)
}

fn coerce_native_overrides<'v>(
    recorder: &PackageRecorder,
    class: NativeRuleClass,
    kwargs: SmallMap<String, Value<'v>>,
    rule_class: &str,
) -> anyhow::Result<Vec<NativeAttributeOverride>> {
    kwargs
        .into_iter()
        .map(|(name, value)| {
            let (slot, schema) = class.slot(&name).ok_or_else(|| {
                anyhow::anyhow!(
                    "native attribute `{name}` is not declared by rule '{rule_class}'"
                )
            })?;
            match schema.policy() {
                NativeAttributePolicy::Callable => {}
                NativeAttributePolicy::Implicit => {
                    anyhow::bail!("native attribute `{name}` is implicit and cannot be set")
                }
                NativeAttributePolicy::Forced => {
                    anyhow::bail!(
                        "native attribute `{name}` is fixed by rule '{rule_class}' and cannot be set"
                    )
                }
            }
            let mut value = match schema.kind() {
                AttributeKind::Boolean => value
                    .unpack_bool()
                    .map(CoercedAttributeValue::Boolean)
                    .ok_or_else(|| anyhow::anyhow!("native attribute `{name}` must be a bool"))?,
                AttributeKind::Integer => value
                    .unpack_i32()
                    .map(CoercedAttributeValue::Integer)
                    .ok_or_else(|| {
                        anyhow::anyhow!("native attribute `{name}` must be an integer")
                    })?,
                AttributeKind::StringDict => {
                    let raw = raw_attribute_value(value)?;
                    let RawAttributeValue::Dict(entries) = raw else {
                        anyhow::bail!("native attribute `{name}` must be a string dictionary")
                    };
                    CoercedAttributeValue::StringDict(
                        entries
                            .iter()
                            .map(|(key, value)| {
                                Ok((
                                    raw_string(key, "dictionary key")?,
                                    raw_string(value, "dictionary value")?,
                                ))
                            })
                            .collect::<anyhow::Result<Vec<_>>>()?
                            .into(),
                    )
                }
                AttributeKind::StringList => {
                    let raw = raw_attribute_value(value)?;
                    let RawAttributeValue::List(values) = raw else {
                        anyhow::bail!("native attribute `{name}` must be a list of strings")
                    };
                    CoercedAttributeValue::StringList(
                        values
                            .iter()
                            .map(|value| raw_string(value, "string list"))
                            .collect::<anyhow::Result<Vec<_>>>()?
                            .into(),
                    )
                }
                kind => coerce_raw_value(
                    &recorder.package,
                    kind,
                    &raw_attribute_value(value)?,
                )?,
            };
            if schema.order() == NativeAttributeOrder::OrderIndependent {
                match &mut value {
                    CoercedAttributeValue::StringList(values) => {
                        let mut sorted = values.to_vec();
                        sorted.sort_unstable();
                        *values = sorted.into();
                    }
                    CoercedAttributeValue::LabelList(values) => {
                        let mut sorted = values.to_vec();
                        sorted.sort_by(CanonicalLabel::bazel_natural_cmp);
                        *values = sorted.into();
                    }
                    _ => {}
                }
            }
            Ok(NativeAttributeOverride {
                slot,
                value: NativeAttributeValue {
                    provenance: AttributeProvenance::Explicit,
                    value,
                },
            })
        })
        .collect()
}
fn selector_key_labels(value: &CoercedAttributeValue) -> Vec<CanonicalLabel> {
    fn collect(value: &CoercedAttributeValue, labels: &mut Vec<CanonicalLabel>) {
        match value {
            CoercedAttributeValue::Selector { branches, default } => {
                for (condition, branch) in branches.iter() {
                    if !labels.contains(condition) {
                        labels.push(condition.clone());
                    }
                    collect(branch, labels);
                }
                if let Some(default) = default {
                    collect(default, labels);
                }
            }
            CoercedAttributeValue::Concatenation(left, right) => {
                collect(left, labels);
                collect(right, labels);
            }
            _ => {}
        }
    }

    let mut labels = Vec::new();
    collect(value, &mut labels);
    labels
}

// Bazel 9.2 source: Attribute.Builder documents type defaults as label=null,
// list=[], and string="". StarlarkAttrModule applies the corresponding empty
// defaults to the public label dictionaries and output_list.
fn intrinsic_default(kind: AttributeKind) -> CoercedAttributeValue {
    match kind {
        AttributeKind::Label | AttributeKind::Output => CoercedAttributeValue::None,
        AttributeKind::LabelList => CoercedAttributeValue::LabelList(Arc::from([])),
        AttributeKind::String => CoercedAttributeValue::String(CompactString::default()),
        AttributeKind::StringList => CoercedAttributeValue::StringList(Arc::from([])),
        AttributeKind::Boolean => CoercedAttributeValue::Boolean(false),
        AttributeKind::Integer => CoercedAttributeValue::Integer(0),
        AttributeKind::StringDict => CoercedAttributeValue::StringDict(Arc::from([])),
        AttributeKind::StringKeyedLabelDict => {
            CoercedAttributeValue::StringKeyedLabelDict(Arc::from([]))
        }
        AttributeKind::LabelKeyedStringDict => {
            CoercedAttributeValue::LabelKeyedStringDict(Arc::from([]))
        }
        AttributeKind::LabelListDict => CoercedAttributeValue::LabelListDict(Arc::from([])),
        AttributeKind::OutputList => CoercedAttributeValue::OutputList(Arc::from([])),
    }
}

fn coerce_raw_value(
    base_package: &str,
    kind: AttributeKind,
    raw: &RawAttributeValue,
) -> anyhow::Result<CoercedAttributeValue> {
    let labels = |values: &[RawAttributeValue], context| {
        values
            .iter()
            .map(|value| raw_label(base_package, value, context))
            .collect::<anyhow::Result<Vec<_>>>()
    };
    match kind {
        AttributeKind::Label => Ok(CoercedAttributeValue::Label(raw_label(
            base_package,
            raw,
            "label",
        )?)),
        AttributeKind::Output => Ok(CoercedAttributeValue::Output(raw_output(
            base_package,
            raw,
            "output",
        )?)),
        AttributeKind::String => Ok(CoercedAttributeValue::String(raw_string(raw, "string")?)),
        AttributeKind::Boolean => match raw {
            RawAttributeValue::Boolean(value) => Ok(CoercedAttributeValue::Boolean(*value)),
            _ => anyhow::bail!("attribute must be a bool"),
        },
        AttributeKind::Integer => match raw {
            RawAttributeValue::Integer(value) => Ok(CoercedAttributeValue::Integer(*value)),
            _ => anyhow::bail!("attribute must be an integer"),
        },
        AttributeKind::StringDict => {
            let RawAttributeValue::Dict(values) = raw else {
                anyhow::bail!("attribute must be a string dictionary");
            };
            Ok(CoercedAttributeValue::StringDict(
                values
                    .iter()
                    .map(|(key, value)| {
                        Ok((
                            raw_string(key, "dictionary key")?,
                            raw_string(value, "dictionary value")?,
                        ))
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?
                    .into(),
            ))
        }
        AttributeKind::StringList => {
            let RawAttributeValue::List(values) = raw else {
                anyhow::bail!("attribute must be a list of strings");
            };
            let values = values
                .iter()
                .map(|value| raw_string(value, "string list"))
                .collect::<anyhow::Result<Vec<_>>>()?;
            Ok(CoercedAttributeValue::StringList(values.into()))
        }
        AttributeKind::LabelList | AttributeKind::OutputList => {
            let RawAttributeValue::List(values) = raw else {
                anyhow::bail!("attribute must be a list of labels");
            };
            let values = if kind == AttributeKind::LabelList {
                labels(values, "label list")?
            } else {
                values
                    .iter()
                    .map(|value| raw_output(base_package, value, "output list"))
                    .collect::<anyhow::Result<Vec<_>>>()?
            };
            Ok(if kind == AttributeKind::LabelList {
                CoercedAttributeValue::LabelList(values.into())
            } else {
                CoercedAttributeValue::OutputList(values.into())
            })
        }
        AttributeKind::StringKeyedLabelDict => {
            let RawAttributeValue::Dict(values) = raw else {
                anyhow::bail!("attribute must be a dictionary");
            };
            Ok(CoercedAttributeValue::StringKeyedLabelDict(
                values
                    .iter()
                    .map(|(key, value)| {
                        Ok((
                            raw_string(key, "dictionary key")?,
                            raw_label(base_package, value, "dictionary value")?,
                        ))
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?
                    .into(),
            ))
        }
        AttributeKind::LabelKeyedStringDict => {
            let RawAttributeValue::Dict(values) = raw else {
                anyhow::bail!("attribute must be a dictionary");
            };
            Ok(CoercedAttributeValue::LabelKeyedStringDict(
                values
                    .iter()
                    .map(|(key, value)| {
                        Ok((
                            raw_label(base_package, key, "dictionary key")?,
                            raw_string(value, "dictionary value")?,
                        ))
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?
                    .into(),
            ))
        }
        AttributeKind::LabelListDict => {
            let RawAttributeValue::Dict(values) = raw else {
                anyhow::bail!("attribute must be a dictionary");
            };
            Ok(CoercedAttributeValue::LabelListDict(
                values
                    .iter()
                    .map(|(key, value)| {
                        let RawAttributeValue::List(value) = value else {
                            anyhow::bail!("attribute dictionary values must be lists");
                        };
                        Ok((
                            raw_string(key, "dictionary key")?,
                            labels(value, "dictionary list")?.into(),
                        ))
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?
                    .into(),
            ))
        }
    }
}

fn coerce_starlark_value(
    recorder: &PackageRecorder,
    kind: AttributeKind,
    attribute_name: &str,
    configurable: bool,
    value: Value,
) -> anyhow::Result<CoercedAttributeValue> {
    if let Some(selector) = SelectorValue::from_value(value) {
        if !configurable {
            anyhow::bail!(
                "attribute `{attribute_name}` is not configurable and cannot use select()"
            );
        }
        let selector = match selector {
            starlark::__macro_refs::Either::Left(selector) => selector,
            starlark::__macro_refs::Either::Right(_) => {
                anyhow::bail!("frozen select values are not valid BUILD attribute values")
            }
        };
        let mut result: Option<CoercedAttributeValue> = None;
        for part in &selector.parts {
            let mut branches = Vec::new();
            let mut default = None;
            for branch in part.branches.iter() {
                if branch.condition == "//conditions:default" {
                    default = Some(Arc::new(coerce_starlark_value(
                        recorder,
                        kind,
                        attribute_name,
                        configurable,
                        branch.value,
                    )?));
                } else {
                    branches.push((
                        recorder.dependency_label(&branch.condition)?,
                        Arc::new(coerce_starlark_value(
                            recorder,
                            kind,
                            attribute_name,
                            configurable,
                            branch.value,
                        )?),
                    ));
                }
            }
            let selected = CoercedAttributeValue::Selector {
                branches: branches.into(),
                default,
            };
            let selected =
                part.prefix
                    .iter()
                    .rev()
                    .copied()
                    .try_fold(selected, |selected, prefix| {
                        Ok::<_, anyhow::Error>(CoercedAttributeValue::Concatenation(
                            Arc::new(coerce_starlark_value(
                                recorder,
                                kind,
                                attribute_name,
                                configurable,
                                prefix,
                            )?),
                            Arc::new(selected),
                        ))
                    })?;
            let selected = part
                .suffix
                .iter()
                .copied()
                .try_fold(selected, |selected, suffix| {
                    Ok::<_, anyhow::Error>(CoercedAttributeValue::Concatenation(
                        Arc::new(selected),
                        Arc::new(coerce_starlark_value(
                            recorder,
                            kind,
                            attribute_name,
                            configurable,
                            suffix,
                        )?),
                    ))
                })?;
            result = Some(match result {
                Some(left) => {
                    CoercedAttributeValue::Concatenation(Arc::new(left), Arc::new(selected))
                }
                None => selected,
            });
        }
        return result.ok_or_else(|| anyhow::anyhow!("select() requires at least one branch"));
    }
    if matches!(
        kind,
        AttributeKind::LabelList | AttributeKind::OutputList | AttributeKind::StringList
    ) && ListRef::from_value(value).is_none()
    {
        if kind == AttributeKind::StringList {
            anyhow::bail!("attribute `{attribute_name}` must be a list of strings");
        }
        anyhow::bail!("attribute `{attribute_name}` must be a list of labels");
    }
    let raw = raw_attribute_value(value).map_err(|_| {
        anyhow::anyhow!("attribute `{attribute_name}` must contain only string labels")
    })?;
    coerce_raw_value(&recorder.package, kind, &raw)
}

/// The callable returned by Bazel's `rule()` global during package loading.
/// It retains the implementation for Stage 6, but package construction never
/// executes that implementation.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Trace)]
struct RuleDefinitionGen<V> {
    implementation: V,
    #[trace(unsafe_ignore)]
    required_toolchains: Arc<[CanonicalLabel]>,
    #[trace(unsafe_ignore)]
    schema: Arc<[RuleAttributeSchemaGen<V>]>,
    executable: bool,
    test: bool,
    root_string_build_setting: bool,
    #[trace(unsafe_ignore)]
    rule_class: OnceCell<CompactString>,
}

/// The frozen definition contains no export-time interior mutability. Its
/// shared capability is cloned into every package instance of this rule.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct FrozenRuleDefinition {
    implementation: FrozenValue,
    required_toolchains: Arc<[CanonicalLabel]>,
    schema: Arc<[FrozenRuleAttributeSchema]>,
    capability: Arc<RuleCapability>,
    root_string_build_setting: bool,
}

type RuleDefinition<'v> = RuleDefinitionGen<Value<'v>>;

impl<V> fmt::Display for RuleDefinitionGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("rule")
    }
}

impl fmt::Display for FrozenRuleDefinition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("rule")
    }
}

starlark::starlark_complex_values!(RuleDefinition);

impl<'v> Freeze for RuleDefinition<'v> {
    type Frozen = FrozenRuleDefinition;

    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        let Some(rule_class) = self.rule_class.into_inner() else {
            return Err(FreezeError::new(
                "the result of rule() must be assigned to a top-level variable".to_owned(),
            ));
        };
        Ok(FrozenRuleDefinition {
            implementation: self.implementation.freeze(freezer)?,
            required_toolchains: self.required_toolchains,
            schema: self
                .schema
                .iter()
                .cloned()
                .map(|schema| schema.freeze(freezer))
                .collect::<FreezeResult<Vec<_>>>()?
                .into(),
            capability: Arc::new(RuleCapability {
                rule_class,
                executable: self.executable || self.test,
                test_kind: self.test.then_some(TestRuleKind::Test),
            }),
            root_string_build_setting: self.root_string_build_setting,
        })
    }
}

#[starlark_value(type = "rule")]
impl<'v> StarlarkValue<'v> for RuleDefinition<'v> {
    type Canonical = FrozenRuleDefinition;

    fn export_as(
        &self,
        variable_name: &str,
        _eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<()> {
        if self.test != variable_name.ends_with("_test") {
            return Err(starlark::Error::new_other(anyhow::anyhow!(
                "Invalid rule class name '{variable_name}', test rule class names must end with '_test' and other rule classes must not"
            )));
        }
        if self.rule_class.get().is_none() {
            let _ = self.rule_class.set(variable_name.into());
        }
        Ok(())
    }

    fn invoke(
        &self,
        _me: Value<'v>,
        _args: &Arguments<'v, '_>,
        _eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        Err(starlark::Error::new_other(anyhow::anyhow!(
            "rule() definitions may only be called after their .bzl module is frozen"
        )))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum RawAttributeValue {
    String(CompactString),
    Boolean(bool),
    Integer(i32),
    List(Arc<[RawAttributeValue]>),
    Dict(Arc<[(RawAttributeValue, RawAttributeValue)]>),
}

#[derive(Debug, Clone, Trace, Allocative)]
struct RuleAttributeSchemaGen<V> {
    #[trace(unsafe_ignore)]
    name: CompactString,
    #[trace(unsafe_ignore)]
    kind: AttributeKind,
    #[trace(unsafe_ignore)]
    mandatory: bool,
    #[trace(unsafe_ignore)]
    configurable: bool,
    #[trace(unsafe_ignore)]
    default: Option<CoercedAttributeValue>,
    transition: Option<TransitionDefinitionGen<V>>,
    #[trace(unsafe_ignore)]
    builtin: bool,
}
type RuleAttributeSchema<'v> = RuleAttributeSchemaGen<Value<'v>>;
type FrozenRuleAttributeSchema = RuleAttributeSchemaGen<FrozenValue>;

// These are loading-owned RuleClass members, rather than public `attr.*`
// descriptors.  Keeping the finite shape here lets target invocation retain
// the same typed values as user declarations without broadening the
// descriptor surface.
fn starlark_builtin_schema<V>(
    executable: bool,
    test: bool,
    root_string_build_setting: bool,
    has_transition: bool,
) -> Vec<RuleAttributeSchemaGen<V>> {
    let mut result = Vec::new();
    let mut push = |name, kind, mandatory, configurable| {
        result.push(RuleAttributeSchemaGen {
            name: CompactString::new(name),
            kind,
            mandatory,
            configurable,
            default: None,
            transition: None,
            builtin: true,
        });
    };
    push("name", AttributeKind::String, true, false);
    push("visibility", AttributeKind::LabelList, false, false);
    push("transitive_configs", AttributeKind::LabelList, false, false);
    push("deprecation", AttributeKind::String, false, false);
    push("tags", AttributeKind::StringList, false, false);
    push("generator_name", AttributeKind::String, false, false);
    push("generator_function", AttributeKind::String, false, false);
    push("generator_location", AttributeKind::String, false, false);
    push("testonly", AttributeKind::Boolean, false, false);
    push("features", AttributeKind::StringList, false, true);
    push(":action_listener", AttributeKind::LabelList, false, true);
    push("compatible_with", AttributeKind::LabelList, false, false);
    push("restricted_to", AttributeKind::LabelList, false, false);
    push(
        "$config_dependencies",
        AttributeKind::LabelList,
        false,
        false,
    );
    push("package_metadata", AttributeKind::LabelList, false, false);
    push("aspect_hints", AttributeKind::LabelList, false, true);
    push("expect_failure", AttributeKind::String, false, true);
    push("toolchains", AttributeKind::LabelList, false, true);
    push("exec_properties", AttributeKind::StringDict, false, true);
    push(
        "exec_compatible_with",
        AttributeKind::LabelList,
        false,
        false,
    );
    push(
        "exec_group_compatible_with",
        AttributeKind::LabelListDict,
        false,
        false,
    );
    push(
        "target_compatible_with",
        AttributeKind::LabelList,
        false,
        true,
    );
    if executable && !test {
        push("args", AttributeKind::StringList, false, true);
        push("output_licenses", AttributeKind::StringList, false, true);
        push("$is_executable", AttributeKind::Boolean, false, false);
    }
    if test {
        push("size", AttributeKind::String, false, false);
        push("timeout", AttributeKind::String, false, false);
        push("flaky", AttributeKind::Boolean, false, false);
        push("shard_count", AttributeKind::Integer, false, true);
        push("local", AttributeKind::Boolean, false, false);
        push("args", AttributeKind::StringList, false, true);
        for (name, kind) in [
            ("$test_wrapper", AttributeKind::Label),
            ("$xml_writer", AttributeKind::Label),
            ("$test_runtime", AttributeKind::LabelList),
            ("$test_setup_script", AttributeKind::Label),
            ("$xml_generator_script", AttributeKind::Label),
            ("$collect_coverage_script", AttributeKind::Label),
            (":coverage_support", AttributeKind::Label),
            (":coverage_report_generator", AttributeKind::Label),
            (":run_under_exec_config", AttributeKind::Label),
            (":run_under_target_config", AttributeKind::Label),
        ] {
            push(name, kind, false, true);
        }
        push("$is_executable", AttributeKind::Boolean, false, false);
    }
    if root_string_build_setting {
        push("build_setting_default", AttributeKind::String, true, false);
        push("help", AttributeKind::String, false, false);
    }
    if has_transition {
        push(
            "$allowlist_function_transition",
            AttributeKind::Label,
            false,
            true,
        );
    }
    result
}

fn starlark_builtin_callable(name: &str) -> bool {
    !name.starts_with(':') && !name.starts_with('$')
}

fn starlark_builtin_order_independent(name: &str) -> bool {
    matches!(
        name,
        "visibility" | "transitive_configs" | "tags" | "features"
    )
}

// Bazel's common RuleClass source marks only visibility and
// transitive_configs as NODEP. `$config_dependencies` is a normal label list:
// it records selector keys and therefore contributes those keys as edges.
fn starlark_builtin_ordinary_dependency(name: &str, kind: AttributeKind) -> bool {
    kind.contributes_ordinary_dependencies() && !matches!(name, "visibility" | "transitive_configs")
}

fn starlark_effective_visibility(
    visibility: &RuleVisibility,
) -> anyhow::Result<CoercedAttributeValue> {
    let labels: Arc<[CanonicalLabel]> =
        match visibility {
            RuleVisibility::Public => Arc::from([
                CanonicalLabel::parse("@@//visibility:public").map_err(anyhow::Error::msg)?
            ]),
            RuleVisibility::Private => Arc::from([
                CanonicalLabel::parse("@@//visibility:private").map_err(anyhow::Error::msg)?
            ]),
            RuleVisibility::Restricted(restricted) => restricted.declared_labels().to_vec().into(),
        };
    Ok(CoercedAttributeValue::LabelList(labels))
}

fn starlark_generator_metadata(
    recorder: &PackageRecorder,
    eval: &Evaluator<'_, '_, '_>,
) -> (CompactString, CompactString, CompactString) {
    let Some(context) = eval.native_call_context("name") else {
        return (
            CompactString::default(),
            CompactString::default(),
            CompactString::default(),
        );
    };
    let position = context.call_location.resolve_span_for_reporting().begin;
    let build_file = Path::new(context.call_location.filename())
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("BUILD.bazel");
    let build_file = if recorder.package.is_empty() {
        build_file.to_owned()
    } else {
        format!("{}/{}", recorder.package, build_file)
    };
    (
        context.local_value.unwrap_or_default().into(),
        context.function_name.into(),
        format!("{build_file}:{}:{}", position.line + 1, position.column + 1).into(),
    )
}

fn starlark_fixed_label(value: &str) -> CoercedAttributeValue {
    CoercedAttributeValue::Label(CanonicalLabel::parse(value).expect("static Bazel tools label"))
}

fn starlark_builtin_default(
    name: &str,
    kind: AttributeKind,
    test: bool,
    visibility: &CoercedAttributeValue,
    deprecation: Option<&CompactString>,
    default_testonly: bool,
    package_metadata: &Arc<[CanonicalLabel]>,
    generator: &(CompactString, CompactString, CompactString),
) -> (AttributeProvenance, CoercedAttributeValue) {
    let default = || (AttributeProvenance::Default, intrinsic_default(kind));
    match name {
        "visibility" => (AttributeProvenance::Default, visibility.clone()),
        "deprecation" => (
            AttributeProvenance::Default,
            deprecation
                .cloned()
                .map(CoercedAttributeValue::String)
                .unwrap_or(CoercedAttributeValue::None),
        ),
        "generator_name" => (
            AttributeProvenance::Implicit,
            CoercedAttributeValue::String(generator.0.clone()),
        ),
        "generator_function" => (
            AttributeProvenance::Implicit,
            CoercedAttributeValue::String(generator.1.clone()),
        ),
        "generator_location" => (
            AttributeProvenance::Implicit,
            CoercedAttributeValue::String(generator.2.clone()),
        ),
        "testonly" => (
            AttributeProvenance::Default,
            CoercedAttributeValue::Boolean(test || default_testonly),
        ),
        "package_metadata" => (
            AttributeProvenance::Default,
            CoercedAttributeValue::LabelList(package_metadata.clone()),
        ),
        "$config_dependencies" => (
            AttributeProvenance::Implicit,
            CoercedAttributeValue::LabelList(Arc::from([])),
        ),
        "size" => (
            AttributeProvenance::Default,
            CoercedAttributeValue::String("medium".into()),
        ),
        "timeout" => (
            AttributeProvenance::Implicit,
            CoercedAttributeValue::String("moderate".into()),
        ),
        "shard_count" => (
            AttributeProvenance::Default,
            CoercedAttributeValue::Integer(-1),
        ),
        "$is_executable" => (
            AttributeProvenance::Implicit,
            CoercedAttributeValue::Boolean(true),
        ),
        "$test_wrapper" => (
            AttributeProvenance::Implicit,
            starlark_fixed_label("@@bazel_tools//tools/test:test_wrapper"),
        ),
        "$xml_writer" => (
            AttributeProvenance::Implicit,
            starlark_fixed_label("@@bazel_tools//tools/test:xml_writer"),
        ),
        "$test_runtime" => (
            AttributeProvenance::Implicit,
            CoercedAttributeValue::LabelList(Arc::from([CanonicalLabel::parse(
                "@@bazel_tools//tools/test:runtime",
            )
            .expect("static Bazel tools label")])),
        ),
        "$test_setup_script" => (
            AttributeProvenance::Implicit,
            starlark_fixed_label("@@bazel_tools//tools/test:test_setup"),
        ),
        "$xml_generator_script" => (
            AttributeProvenance::Implicit,
            starlark_fixed_label("@@bazel_tools//tools/test:test_xml_generator"),
        ),
        "$collect_coverage_script" => (
            AttributeProvenance::Implicit,
            starlark_fixed_label("@@bazel_tools//tools/test:collect_coverage"),
        ),
        ":coverage_support" => (
            AttributeProvenance::Implicit,
            starlark_fixed_label("@@bazel_tools//tools/test:coverage_support"),
        ),
        ":coverage_report_generator" => (
            AttributeProvenance::Implicit,
            starlark_fixed_label("@@bazel_tools//tools/test:coverage_report_generator"),
        ),
        ":run_under_exec_config" | ":run_under_target_config" => {
            (AttributeProvenance::Implicit, CoercedAttributeValue::None)
        }
        "$allowlist_function_transition" => (
            AttributeProvenance::Implicit,
            starlark_fixed_label(
                "@@bazel_tools//tools/allowlists/function_transition_allowlist:function_transition_allowlist",
            ),
        ),
        _ => default(),
    }
}

fn normalize_starlark_value(
    value: CoercedAttributeValue,
    order_independent: bool,
) -> CoercedAttributeValue {
    if !order_independent {
        return value;
    }
    match value {
        CoercedAttributeValue::StringList(values) => {
            let mut values = values.to_vec();
            values.sort_unstable();
            CoercedAttributeValue::StringList(values.into())
        }
        CoercedAttributeValue::LabelList(values) => {
            let mut values = values.to_vec();
            values.sort_by(CanonicalLabel::bazel_natural_cmp);
            CoercedAttributeValue::LabelList(values.into())
        }
        CoercedAttributeValue::Selector { branches, default } => CoercedAttributeValue::Selector {
            branches: branches
                .iter()
                .map(|(condition, value)| {
                    (
                        condition.clone(),
                        Arc::new(normalize_starlark_value((**value).clone(), true)),
                    )
                })
                .collect::<Vec<_>>()
                .into(),
            default: default
                .map(|value| Arc::new(normalize_starlark_value((*value).clone(), true))),
        },
        CoercedAttributeValue::Concatenation(left, right) => CoercedAttributeValue::Concatenation(
            Arc::new(normalize_starlark_value((*left).clone(), true)),
            Arc::new(normalize_starlark_value((*right).clone(), true)),
        ),
        value => value,
    }
}

fn replace_starlark_builtin_value(
    values: &mut [AttributeValue],
    name: &str,
    value: CoercedAttributeValue,
    provenance: AttributeProvenance,
) {
    if let Some(existing) = values
        .iter_mut()
        .find(|existing| existing.declaration_name == name)
    {
        existing.value = Arc::new(value);
        existing.provenance = provenance;
    }
}

fn starlark_test_timeout(size: &str) -> &'static str {
    match size {
        "small" => "short",
        "medium" => "moderate",
        "large" => "long",
        "enormous" => "eternal",
        _ => "illegal",
    }
}

#[derive(
    Debug,
    Clone,
    Trace,
    Freeze,
    ProvidesStaticType,
    NoSerialize,
    Allocative
)]
struct TransitionDefinitionGen<V> {
    implementation: V,
    #[trace(unsafe_ignore)]
    #[freeze(identity)]
    output: CompactString,
}
type TransitionDefinition<'v> = TransitionDefinitionGen<Value<'v>>;
type FrozenTransitionDefinition = TransitionDefinitionGen<FrozenValue>;
starlark::starlark_complex_values!(TransitionDefinition);
impl<V> fmt::Display for TransitionDefinitionGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("transition")
    }
}
#[starlark_value(type = "transition")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for TransitionDefinitionGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    type Canonical = FrozenTransitionDefinition;
}

#[derive(Debug, Clone, Trace, ProvidesStaticType, NoSerialize, Allocative)]
struct AttributeDefinitionGen<V> {
    #[trace(unsafe_ignore)]
    kind: AttributeKind,
    #[trace(unsafe_ignore)]
    mandatory: bool,
    #[trace(unsafe_ignore)]
    configurable: bool,
    #[trace(unsafe_ignore)]
    default: Option<CoercedAttributeValue>,
    transition: Option<TransitionDefinitionGen<V>>,
}
type AttributeDefinition<'v> = AttributeDefinitionGen<Value<'v>>;
type FrozenAttributeDefinition = AttributeDefinitionGen<FrozenValue>;
starlark::starlark_complex_values!(AttributeDefinition);
impl<V> fmt::Display for AttributeDefinitionGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "attr.{:?}()", self.kind)
    }
}

#[starlark_value(type = "attribute")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for AttributeDefinitionGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    type Canonical = FrozenAttributeDefinition;
}
impl<'v> Freeze for AttributeDefinition<'v> {
    type Frozen = FrozenAttributeDefinition;
    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        Ok(AttributeDefinitionGen {
            kind: self.kind,
            mandatory: self.mandatory,
            configurable: self.configurable,
            default: self.default,
            transition: self
                .transition
                .map(|value| value.freeze(freezer))
                .transpose()?,
        })
    }
}
impl<'v> Freeze for RuleAttributeSchema<'v> {
    type Frozen = FrozenRuleAttributeSchema;
    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        Ok(RuleAttributeSchemaGen {
            name: self.name,
            kind: self.kind,
            mandatory: self.mandatory,
            configurable: self.configurable,
            default: self.default,
            transition: self
                .transition
                .map(|value| value.freeze(freezer))
                .transpose()?,
            builtin: self.builtin,
        })
    }
}

#[derive(Debug, Trace, Freeze, ProvidesStaticType, NoSerialize, Allocative)]
struct SelectorBranchGen<V> {
    #[trace(unsafe_ignore)]
    #[freeze(identity)]
    condition: CompactString,
    value: V,
}

#[derive(Debug, Trace, Freeze, ProvidesStaticType, NoSerialize, Allocative)]
struct SelectorPartGen<V> {
    prefix: Vec<V>,
    suffix: Vec<V>,
    branches: Vec<SelectorBranchGen<V>>,
}

#[derive(Debug, Trace, Freeze, ProvidesStaticType, NoSerialize, Allocative)]
struct SelectorValueGen<V> {
    parts: Vec<SelectorPartGen<V>>,
}

type SelectorValue<'v> = SelectorValueGen<Value<'v>>;
type FrozenSelectorValue = SelectorValueGen<FrozenValue>;
type SelectorPart<'v> = SelectorPartGen<Value<'v>>;
starlark::starlark_complex_values!(SelectorValue);

impl<V> fmt::Display for SelectorValueGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("select(...)")
    }
}

#[starlark_value(type = "select")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for SelectorValueGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    type Canonical = FrozenSelectorValue;
    fn radd(&self, lhs: Value<'v>, heap: Heap<'v>) -> Option<starlark::Result<Value<'v>>> {
        let mut parts = Vec::with_capacity(self.parts.len());
        for (index, part) in self.parts.iter().enumerate() {
            parts.push(SelectorPartGen {
                prefix: {
                    let mut prefix = if index == 0 { vec![lhs] } else { Vec::new() };
                    prefix.extend(part.prefix.iter().copied().map(ValueLike::to_value));
                    prefix
                },
                suffix: part
                    .suffix
                    .iter()
                    .copied()
                    .map(ValueLike::to_value)
                    .collect(),
                branches: part
                    .branches
                    .iter()
                    .map(|branch| SelectorBranchGen {
                        condition: branch.condition.clone(),
                        value: branch.value.to_value(),
                    })
                    .collect(),
            });
        }
        Some(Ok(heap.alloc(SelectorValueGen { parts })))
    }

    fn add(&self, rhs: Value<'v>, heap: Heap<'v>) -> Option<starlark::Result<Value<'v>>> {
        let mut parts: Vec<SelectorPart<'v>> = self
            .parts
            .iter()
            .map(|part| SelectorPartGen {
                prefix: part
                    .prefix
                    .iter()
                    .copied()
                    .map(ValueLike::to_value)
                    .collect(),
                suffix: part
                    .suffix
                    .iter()
                    .copied()
                    .map(ValueLike::to_value)
                    .collect(),
                branches: part
                    .branches
                    .iter()
                    .map(|branch| SelectorBranchGen {
                        condition: branch.condition.clone(),
                        value: branch.value.to_value(),
                    })
                    .collect(),
            })
            .collect();
        if let Some(other) = SelectorValue::from_value(rhs) {
            match other {
                starlark::__macro_refs::Either::Left(other) => {
                    parts.extend(other.parts.iter().map(|part| {
                        SelectorPartGen {
                            prefix: part.prefix.clone(),
                            suffix: part.suffix.clone(),
                            branches: part
                                .branches
                                .iter()
                                .map(|branch| SelectorBranchGen {
                                    condition: branch.condition.clone(),
                                    value: branch.value,
                                })
                                .collect(),
                        }
                    }))
                }
                starlark::__macro_refs::Either::Right(other) => {
                    parts.extend(other.parts.iter().map(|part| {
                        SelectorPartGen {
                            prefix: part.prefix.iter().map(|value| value.to_value()).collect(),
                            suffix: part.suffix.iter().map(|value| value.to_value()).collect(),
                            branches: part
                                .branches
                                .iter()
                                .map(|branch| SelectorBranchGen {
                                    condition: branch.condition.clone(),
                                    value: branch.value.to_value(),
                                })
                                .collect(),
                        }
                    }))
                }
            }
        } else {
            if let Some(last) = parts.last_mut() {
                last.suffix.push(rhs);
            } else {
                return None;
            }
        }
        Some(Ok(heap.alloc(SelectorValueGen { parts })))
    }
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct AttrModule;

starlark::starlark_simple_value!(AttrModule);

impl fmt::Display for AttrModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("attr")
    }
}

fn attribute_definition<'v>(
    kind: AttributeKind,
    mandatory: bool,
    configurable: bool,
    default: Option<Value<'v>>,
    cfg: Option<Value<'v>>,
    eval: &Evaluator<'v, '_, '_>,
) -> anyhow::Result<AttributeDefinition<'v>> {
    let default = default
        .map(|value| {
            let raw = raw_attribute_value(value)?;
            let context = BzlEvaluationContext::from_evaluator(eval)?;
            let source = CanonicalLabel::parse(&format!("@@{}", context.source_label()))
                .map_err(anyhow::Error::msg)?;
            coerce_raw_value(source.package().package().as_str(), kind, &raw)
        })
        .transpose()?;
    Ok(AttributeDefinition {
        kind,
        mandatory,
        configurable,
        default,
        transition: cfg
            .map(|value| {
                TransitionDefinition::from_value(value)
                    .into_iter()
                    .find_map(|value| match value {
                        starlark::__macro_refs::Either::Left(value) => Some(value.clone()),
                        starlark::__macro_refs::Either::Right(_) => None,
                    })
                    .ok_or_else(|| anyhow::anyhow!("attr.label cfg must be a transition"))
            })
            .transpose()?,
    })
}

#[starlark_module]
fn attr_methods(builder: &mut MethodsBuilder) {
    fn label<'v>(
        #[starlark(this)] _attr: Value<'v>,
        mandatory: Option<bool>,
        #[starlark(default = true)] configurable: bool,
        default: Option<Value<'v>>,
        cfg: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<AttributeDefinition<'v>> {
        attribute_definition(
            AttributeKind::Label,
            mandatory.unwrap_or(false),
            configurable,
            default,
            cfg,
            eval,
        )
    }
    fn label_list<'v>(
        #[starlark(this)] _attr: Value<'v>,
        mandatory: Option<bool>,
        #[starlark(default = true)] configurable: bool,
        default: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<AttributeDefinition<'v>> {
        attribute_definition(
            AttributeKind::LabelList,
            mandatory.unwrap_or(false),
            configurable,
            default,
            None,
            eval,
        )
    }
    fn string_keyed_label_dict<'v>(
        #[starlark(this)] _attr: Value<'v>,
        mandatory: Option<bool>,
        #[starlark(default = true)] configurable: bool,
        default: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<AttributeDefinition<'v>> {
        attribute_definition(
            AttributeKind::StringKeyedLabelDict,
            mandatory.unwrap_or(false),
            configurable,
            default,
            None,
            eval,
        )
    }
    fn label_keyed_string_dict<'v>(
        #[starlark(this)] _attr: Value<'v>,
        mandatory: Option<bool>,
        #[starlark(default = true)] configurable: bool,
        default: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<AttributeDefinition<'v>> {
        attribute_definition(
            AttributeKind::LabelKeyedStringDict,
            mandatory.unwrap_or(false),
            configurable,
            default,
            None,
            eval,
        )
    }
    fn label_list_dict<'v>(
        #[starlark(this)] _attr: Value<'v>,
        mandatory: Option<bool>,
        #[starlark(default = true)] configurable: bool,
        default: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<AttributeDefinition<'v>> {
        attribute_definition(
            AttributeKind::LabelListDict,
            mandatory.unwrap_or(false),
            configurable,
            default,
            None,
            eval,
        )
    }
    fn output<'v>(
        #[starlark(this)] _attr: Value<'v>,
        mandatory: Option<bool>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<AttributeDefinition<'v>> {
        attribute_definition(
            AttributeKind::Output,
            mandatory.unwrap_or(false),
            false,
            None,
            None,
            eval,
        )
    }
    fn output_list<'v>(
        #[starlark(this)] _attr: Value<'v>,
        mandatory: Option<bool>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<AttributeDefinition<'v>> {
        attribute_definition(
            AttributeKind::OutputList,
            mandatory.unwrap_or(false),
            false,
            None,
            None,
            eval,
        )
    }
    fn string<'v>(
        #[starlark(this)] _attr: Value<'v>,
        mandatory: Option<bool>,
        #[starlark(default = true)] configurable: bool,
        default: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<AttributeDefinition<'v>> {
        attribute_definition(
            AttributeKind::String,
            mandatory.unwrap_or(false),
            configurable,
            default,
            None,
            eval,
        )
    }
}

#[starlark_value(type = "attr")]
impl<'v> StarlarkValue<'v> for AttrModule {
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(attr_methods)
    }
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct ConfigModule;
starlark::starlark_simple_value!(ConfigModule);
impl fmt::Display for ConfigModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("config")
    }
}
#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct RootStringBuildSetting;
starlark::starlark_simple_value!(RootStringBuildSetting);
impl fmt::Display for RootStringBuildSetting {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("config.string")
    }
}
#[starlark_value(type = "config_string")]
impl<'v> StarlarkValue<'v> for RootStringBuildSetting {}
#[starlark_module]
fn config_methods(builder: &mut MethodsBuilder) {
    fn string(
        #[starlark(this)] _config: Value,
        #[starlark(default = false)] flag: bool,
    ) -> anyhow::Result<RootStringBuildSetting> {
        if !flag {
            anyhow::bail!("only config.string(flag = True) is supported")
        }
        Ok(RootStringBuildSetting)
    }
}
#[starlark_value(type = "config")]
impl<'v> StarlarkValue<'v> for ConfigModule {
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(config_methods)
    }
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct PlatformCommonModule;

impl fmt::Display for PlatformCommonModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("platform_common")
    }
}

starlark::starlark_simple_value!(PlatformCommonModule);

#[starlark_value(type = "platform_common")]
impl<'v> StarlarkValue<'v> for PlatformCommonModule {
    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        (attribute == "ToolchainInfo")
            .then(|| heap.alloc_simple(AnalysisBuiltinCallable::new("ToolchainInfo")))
    }
}

#[starlark_value(type = "rule")]
impl<'v> StarlarkValue<'v> for FrozenRuleDefinition {
    type Canonical = Self;

    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        args.no_positional_args(eval.heap())?;
        let names = args.names_map()?;
        let name = names.get("name").ok_or_else(|| {
            starlark::Error::new_other(anyhow::anyhow!(
                "a target declared by rule() requires a string `name`"
            ))
        })?;
        let name = name.unpack_str().ok_or_else(|| {
            starlark::Error::new_other(anyhow::anyhow!(
                "a target declared by rule() requires a string `name`"
            ))
        })?;
        for attribute in names.keys() {
            if attribute.as_str() != "name"
                && attribute.as_str() != "visibility"
                && !self
                    .schema
                    .iter()
                    .any(|schema| schema.name == attribute.as_str())
            {
                return Err(starlark::Error::new_other(anyhow::anyhow!(
                    "target `{name}` received unknown attribute `{}`",
                    attribute.as_str()
                )));
            }
        }
        let visibility = if let Some(visibility) = names.get("visibility") {
            let visibility = ListRef::from_value(*visibility).ok_or_else(|| {
                starlark::Error::new_other(anyhow::anyhow!(
                    "attribute `visibility` must be a list of strings"
                ))
            })?;
            visibility
                .iter()
                .map(|value| {
                    value.unpack_str().map(ToOwned::to_owned).ok_or_else(|| {
                        starlark::Error::new_other(anyhow::anyhow!(
                            "attribute `visibility` must be a list of strings"
                        ))
                    })
                })
                .collect::<starlark::Result<Vec<_>>>()
                .map(Some)?
        } else {
            None
        };
        let implementation = self.implementation;
        let required_toolchains = self.required_toolchains.clone();
        let capability = self.capability.clone();
        PackageRecorder::from_evaluator(eval)
            .and_then(|recorder| {
                let (default_visibility, default_deprecation, default_testonly, default_metadata) = {
                    let state = recorder.state.borrow();
                    (
                        state.default_visibility.clone(),
                        state.default_deprecation.clone(),
                        state.default_testonly,
                        state.default_package_metadata.clone(),
                    )
                };
                let effective_visibility = visibility
                    .as_ref()
                    .map(|values| recorder.parse_visibility(values.clone()))
                    .transpose()?
                    .unwrap_or(default_visibility);
                let visibility_value = starlark_effective_visibility(&effective_visibility)?;
                let generator = starlark_generator_metadata(recorder, eval);
                let mut schema = Vec::with_capacity(self.schema.len());
                let mut values = Vec::with_capacity(self.schema.len());
                let mut generated = Vec::new();
                for declaration in self.schema.iter() {
                    let builtin = declaration.builtin;
                    let attribute_schema = if builtin {
                        AttributeSchema::builtin(
                            declaration.name.clone(),
                            declaration.kind,
                            declaration.mandatory,
                            declaration.configurable,
                            None,
                            starlark_builtin_order_independent(&declaration.name),
                            starlark_builtin_ordinary_dependency(
                                &declaration.name,
                                declaration.kind,
                            ),
                        )
                    } else {
                        AttributeSchema::new(
                            declaration.name.clone(),
                            declaration.kind,
                            declaration.mandatory,
                            declaration.configurable,
                            Some(
                                declaration
                                    .default
                                    .clone()
                                    .unwrap_or_else(|| intrinsic_default(declaration.kind)),
                            ),
                            declaration.transition.as_ref().map(|transition| {
                                LoadingTransitionDefinition::new(
                                    transition.implementation,
                                    transition.output.clone(),
                                )
                            }),
                        )
                    };
                    // Keep the full declaration schema even for an omitted
                    // optional value. Stage 8 must distinguish absent-looking
                    // values from a missing declaration.
                    schema.push(attribute_schema.clone());
                    let explicit = names.get(declaration.name.as_str()).copied();
                    if builtin
                        && explicit.is_some()
                        && !starlark_builtin_callable(declaration.name.as_str())
                    {
                        anyhow::bail!(
                            "target `{name}` cannot set implicit attribute `{}`",
                            declaration.name
                        );
                    }
                    let (provenance, value) = match explicit {
                        Some(_) if builtin && declaration.name == "visibility" => {
                            (AttributeProvenance::Explicit, visibility_value.clone())
                        }
                        Some(value) => (
                            AttributeProvenance::Explicit,
                            coerce_starlark_value(
                                recorder,
                                declaration.kind,
                                &declaration.name,
                                declaration.configurable,
                                value,
                            )?,
                        ),
                        None if declaration.mandatory => anyhow::bail!(
                            "missing value for mandatory attribute '{}'",
                            declaration.name
                        ),
                        None if builtin => starlark_builtin_default(
                            declaration.name.as_str(),
                            declaration.kind,
                            self.capability.test_kind.is_some(),
                            &visibility_value,
                            default_deprecation.as_ref(),
                            default_testonly,
                            &default_metadata,
                            &generator,
                        ),
                        None if declaration.name.starts_with('_') => (
                            AttributeProvenance::Implicit,
                            attribute_schema
                                .default()
                                .expect("intrinsic default")
                                .clone(),
                        ),
                        None => (
                            AttributeProvenance::Default,
                            attribute_schema
                                .default()
                                .expect("intrinsic default")
                                .clone(),
                        ),
                    };
                    let value = normalize_starlark_value(value, attribute_schema.order_independent());
                    if matches!(
                        attribute_schema.kind(),
                        AttributeKind::Output | AttributeKind::OutputList
                    ) {
                        value.labels(&mut generated);
                    }
                    values.push(AttributeValue {
                        declaration_name: declaration.name.clone(),
                        provenance,
                        value: Arc::new(value),
                    });
                }
                let config_dependencies = values
                    .iter()
                    .flat_map(|value| selector_key_labels(&value.value))
                    .fold(Vec::new(), |mut labels, label| {
                        if !labels.contains(&label) {
                            labels.push(label);
                        }
                        labels
                    });
                replace_starlark_builtin_value(
                    &mut values,
                    "$config_dependencies",
                    CoercedAttributeValue::LabelList(config_dependencies.into()),
                    AttributeProvenance::Implicit,
                );
                if values
                    .iter()
                    .find(|value| value.declaration_name == "timeout")
                    .is_some_and(|value| value.provenance != AttributeProvenance::Explicit)
                {
                    let timeout = values
                        .iter()
                        .find(|value| value.declaration_name == "size")
                        .and_then(|value| match value.value.as_ref() {
                            CoercedAttributeValue::String(size) => Some(starlark_test_timeout(size)),
                            _ => None,
                        })
                        .unwrap_or("illegal");
                    replace_starlark_builtin_value(
                        &mut values,
                        "timeout",
                        CoercedAttributeValue::String(timeout.into()),
                        AttributeProvenance::Implicit,
                    );
                }
                let schema: Arc<[AttributeSchema]> = schema.into();
                let values: Arc<[AttributeValue]> = values.into();
                recorder.starlark_rule(
                    name.to_owned(),
                    implementation,
                    required_toolchains,
                    capability,
                    schema,
                    values,
                    self.root_string_build_setting,
                    visibility,
                )?;
                for output in generated {
                    recorder.generated_file(output, name)?;
                }
                Ok(())
            })
            .map_err(starlark::Error::new_other)?;
        Ok(Value::new_none())
    }
}

#[starlark_module]
pub(crate) fn package_globals(builder: &mut GlobalsBuilder) {
    fn package(
        default_visibility: Option<UnpackListOrTuple<&str>>,
        default_deprecation: Option<&str>,
        default_testonly: Option<bool>,
        default_package_metadata: Option<UnpackListOrTuple<&str>>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        package_global(
            default_visibility,
            default_deprecation,
            default_testonly,
            default_package_metadata,
            eval,
        )
    }

    fn licenses(values: UnpackListOrTuple<&str>, eval: &mut Evaluator) -> anyhow::Result<NoneType> {
        licenses_global(values, eval)
    }

    fn exports_files(
        srcs: UnpackListOrTuple<&str>,
        visibility: Option<UnpackListOrTuple<&str>>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        exports_files_global(srcs, visibility, eval)
    }

    fn filegroup<'v>(
        name: &str,
        srcs: Option<Value<'v>>,
        visibility: Option<UnpackListOrTuple<&str>>,
        #[starlark(kwargs)] kwargs: SmallMap<String, Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        filegroup_global(name, srcs, visibility, eval)?;
        let recorder = PackageRecorder::from_evaluator(eval)?;
        recorder.set_native_overrides(name, kwargs)?;
        Ok(NoneType)
    }

    fn test_suite<'v>(
        name: &str,
        tests: Option<UnpackListOrTuple<&str>>,
        #[starlark(default=UnpackListOrTuple::default())] tags: UnpackListOrTuple<&str>,
        visibility: Option<UnpackListOrTuple<&str>>,
        #[starlark(kwargs)] kwargs: SmallMap<String, Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        test_suite_global(name, tests, tags, visibility, eval)?;
        PackageRecorder::from_evaluator(eval)?.set_native_overrides(name, kwargs)?;
        Ok(NoneType)
    }

    fn alias<'v>(
        name: &str,
        actual: &str,
        visibility: Option<UnpackListOrTuple<&str>>,
        #[starlark(kwargs)] kwargs: SmallMap<String, Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        alias_global(name, actual, visibility, eval)?;
        PackageRecorder::from_evaluator(eval)?.set_native_overrides(name, kwargs)?;
        Ok(NoneType)
    }

    // Bazel 9.2 `ConfigRuleClasses.ConfigSettingRule` declares `values` as
    // the nonconfigurable string dictionary that records flag bindings.
    // Configuration matching remains owned by the configured-analysis stage.
    fn config_setting<'v>(
        name: &str,
        #[starlark(require = named)] values: Option<SmallMap<String, String>>,
        visibility: Option<UnpackListOrTuple<&str>>,
        #[starlark(kwargs)] kwargs: SmallMap<String, Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let recorder = PackageRecorder::from_evaluator(eval)?;
        recorder.config_setting(
            name.to_owned(),
            values.unwrap_or_default(),
            visibility.map(list),
        )?;
        recorder.set_native_generator_from_evaluator(name, eval)?;
        recorder.set_native_overrides(name, kwargs)?;
        Ok(NoneType)
    }

    fn constraint_setting<'v>(
        name: &str,
        visibility: Option<UnpackListOrTuple<&str>>,
        #[starlark(kwargs)] kwargs: SmallMap<String, Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let recorder = PackageRecorder::from_evaluator(eval)?;
        recorder.native_toolchain_target_with_visibility(
            name.to_owned(),
            NativeToolchainTarget::ConstraintSetting,
            visibility.map(list),
        )?;
        recorder.set_native_generator_from_evaluator(name, eval)?;
        recorder.set_native_overrides(name, kwargs)?;
        Ok(NoneType)
    }

    fn constraint_value<'v>(
        name: &str,
        constraint_setting: &str,
        visibility: Option<UnpackListOrTuple<&str>>,
        #[starlark(kwargs)] kwargs: SmallMap<String, Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let recorder = PackageRecorder::from_evaluator(eval)?;
        recorder.native_toolchain_target_with_visibility(
            name.to_owned(),
            NativeToolchainTarget::ConstraintValue {
                constraint_setting: recorder.native_toolchain_label(constraint_setting)?,
            },
            visibility.map(list),
        )?;
        recorder.set_native_generator_from_evaluator(name, eval)?;
        recorder.set_native_overrides(name, kwargs)?;
        Ok(NoneType)
    }

    fn platform<'v>(
        name: &str,
        #[starlark(default = UnpackList::default())] constraint_values: UnpackList<&str>,
        visibility: Option<UnpackListOrTuple<&str>>,
        #[starlark(kwargs)] kwargs: SmallMap<String, Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let recorder = PackageRecorder::from_evaluator(eval)?;
        recorder.native_toolchain_target_with_visibility(
            name.to_owned(),
            NativeToolchainTarget::Platform {
                constraint_values: recorder.native_toolchain_labels(&constraint_values.items)?,
            },
            visibility.map(list),
        )?;
        recorder.set_native_generator_from_evaluator(name, eval)?;
        recorder.set_native_overrides(name, kwargs)?;
        Ok(NoneType)
    }

    fn toolchain_type<'v>(
        name: &str,
        visibility: Option<UnpackListOrTuple<&str>>,
        #[starlark(kwargs)] kwargs: SmallMap<String, Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let recorder = PackageRecorder::from_evaluator(eval)?;
        recorder.native_toolchain_target_with_visibility(
            name.to_owned(),
            NativeToolchainTarget::ToolchainType,
            visibility.map(list),
        )?;
        recorder.set_native_generator_from_evaluator(name, eval)?;
        recorder.set_native_overrides(name, kwargs)?;
        Ok(NoneType)
    }

    fn toolchain<'v>(
        name: &str,
        toolchain: &str,
        toolchain_type: &str,
        #[starlark(default = UnpackList::default())] exec_compatible_with: UnpackList<&str>,
        visibility: Option<UnpackListOrTuple<&str>>,
        #[starlark(kwargs)] kwargs: SmallMap<String, Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let recorder = PackageRecorder::from_evaluator(eval)?;
        recorder.native_toolchain_target_with_visibility(
            name.to_owned(),
            NativeToolchainTarget::Toolchain {
                toolchain_type: recorder.native_toolchain_label(toolchain_type)?,
                implementation: recorder.native_toolchain_label(toolchain)?,
                exec_compatible_with: recorder
                    .native_toolchain_labels(&exec_compatible_with.items)?,
            },
            visibility.map(list),
        )?;
        recorder.set_native_generator_from_evaluator(name, eval)?;
        recorder.set_native_overrides(name, kwargs)?;
        Ok(NoneType)
    }

    fn package_group(
        name: &str,
        #[starlark(default=UnpackListOrTuple::default())] packages: UnpackListOrTuple<&str>,
        #[starlark(default=UnpackListOrTuple::default())] includes: UnpackListOrTuple<&str>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        PackageRecorder::from_evaluator(eval)?.package_group(
            name.to_owned(),
            list(packages),
            list(includes),
        )?;
        Ok(NoneType)
    }

    fn glob<'v>(
        #[starlark(default=UnpackListOrTuple::default())] include: UnpackListOrTuple<&str>,
        #[starlark(default=UnpackListOrTuple::default())] exclude: UnpackListOrTuple<&str>,
        #[starlark(default = 1)] exclude_directories: i32,
        allow_empty: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Vec<String>> {
        glob_global(include, exclude, exclude_directories, allow_empty, eval)
    }

    fn rule<'v>(
        implementation: Value<'v>,
        attrs: Option<SmallMap<String, Value<'v>>>,
        build_setting: Option<Value<'v>>,
        toolchains: Option<UnpackList<&str>>,
        #[starlark(default = false)] executable: bool,
        #[starlark(default = false)] test: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<RuleDefinition<'v>> {
        let root_string_build_setting = build_setting
            .map(|value| RootStringBuildSetting::from_value(value).is_some())
            .unwrap_or(false);
        if build_setting.is_some() && !root_string_build_setting {
            anyhow::bail!("only rule(build_setting = config.string(flag = True)) is supported")
        }
        let declared_builtin_names =
            starlark_builtin_schema::<Value<'v>>(executable, test, root_string_build_setting, true);
        let mut user_schema = Vec::new();
        if let Some(attrs) = attrs {
            for (name, value) in attrs {
                if declared_builtin_names
                    .iter()
                    .any(|schema| schema.name == name)
                {
                    anyhow::bail!("rule attribute `{name}` is built in and cannot be redeclared");
                }
                let definition = AttributeDefinition::from_value(value)
                    .and_then(|value| match value {
                        starlark::__macro_refs::Either::Left(value) => Some(value),
                        starlark::__macro_refs::Either::Right(_) => None,
                    })
                    .ok_or_else(|| anyhow::anyhow!("rule attribute `{name}` must use attr.*()"))?;
                user_schema.push(RuleAttributeSchema {
                    name: CompactString::new(name),
                    kind: definition.kind,
                    mandatory: definition.mandatory,
                    configurable: definition.configurable,
                    default: definition.default.clone(),
                    transition: definition.transition.clone(),
                    builtin: false,
                });
            }
        }
        let has_transition = user_schema.iter().any(|schema| schema.transition.is_some());
        let mut schema =
            starlark_builtin_schema(executable, test, root_string_build_setting, has_transition);
        schema.extend(user_schema);
        Ok(RuleDefinition {
            implementation,
            required_toolchains: rule_toolchain_requirement(toolchains, eval)?,
            schema: schema.into(),
            executable,
            test,
            root_string_build_setting,
            rule_class: OnceCell::new(),
        })
    }

    fn provider(
        #[starlark(require = named)] fields: SmallMap<String, String>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<UserProviderCallable> {
        UserProviderCallable::from_evaluator(fields, eval)
    }
    fn transition<'v>(
        implementation: Value<'v>,
        outputs: UnpackListOrTuple<&str>,
        #[starlark(default = UnpackListOrTuple::default())] inputs: UnpackListOrTuple<&str>,
    ) -> anyhow::Result<TransitionDefinition<'v>> {
        if !list(inputs).is_empty() || list(outputs) != ["//:setting"] {
            anyhow::bail!("only transition(inputs = [], outputs = [\"//:setting\"]) is supported")
        }
        Ok(TransitionDefinitionGen {
            implementation,
            output: CompactString::const_new("//:setting"),
        })
    }
}

#[starlark_module]
fn select_globals(builder: &mut GlobalsBuilder) {
    fn select<'v>(branches: SmallMap<String, Value<'v>>) -> anyhow::Result<SelectorValue<'v>> {
        if branches.is_empty() {
            anyhow::bail!("select() requires at least one branch");
        }
        Ok(SelectorValueGen {
            parts: vec![SelectorPart {
                prefix: Vec::new(),
                suffix: Vec::new(),
                branches: branches
                    .into_iter()
                    .map(|(condition, value)| SelectorBranchGen {
                        condition: CompactString::new(condition),
                        value,
                    })
                    .collect(),
            }],
        })
    }
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct NativeModule;

impl fmt::Display for NativeModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("native")
    }
}

#[starlark_module]
fn native_methods(builder: &mut MethodsBuilder) {
    fn exports_files(
        #[starlark(this)] _native: Value,
        srcs: UnpackListOrTuple<&str>,
        visibility: Option<UnpackListOrTuple<&str>>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        exports_files_global(srcs, visibility, eval)
    }

    fn filegroup<'v>(
        #[starlark(this)] _native: Value<'v>,
        name: &str,
        srcs: Option<Value<'v>>,
        visibility: Option<UnpackListOrTuple<&str>>,
        #[starlark(kwargs)] kwargs: SmallMap<String, Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        filegroup_global(name, srcs, visibility, eval)?;
        let recorder = PackageRecorder::from_evaluator(eval)?;
        recorder.set_native_overrides(name, kwargs)?;
        Ok(NoneType)
    }

    fn test_suite<'v>(
        #[starlark(this)] _native: Value<'v>,
        name: &str,
        tests: Option<UnpackListOrTuple<&str>>,
        #[starlark(default=UnpackListOrTuple::default())] tags: UnpackListOrTuple<&str>,
        visibility: Option<UnpackListOrTuple<&str>>,
        #[starlark(kwargs)] kwargs: SmallMap<String, Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        test_suite_global(name, tests, tags, visibility, eval)?;
        PackageRecorder::from_evaluator(eval)?.set_native_overrides(name, kwargs)?;
        Ok(NoneType)
    }

    fn alias<'v>(
        #[starlark(this)] _native: Value<'v>,
        name: &str,
        actual: &str,
        visibility: Option<UnpackListOrTuple<&str>>,
        #[starlark(kwargs)] kwargs: SmallMap<String, Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        alias_global(name, actual, visibility, eval)?;
        PackageRecorder::from_evaluator(eval)?.set_native_overrides(name, kwargs)?;
        Ok(NoneType)
    }

    fn config_setting<'v>(
        #[starlark(this)] _native: Value<'v>,
        name: &str,
        #[starlark(require = named)] values: Option<SmallMap<String, String>>,
        visibility: Option<UnpackListOrTuple<&str>>,
        #[starlark(kwargs)] kwargs: SmallMap<String, Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let recorder = PackageRecorder::from_evaluator(eval)?;
        recorder.config_setting(
            name.to_owned(),
            values.unwrap_or_default(),
            visibility.map(list),
        )?;
        recorder.set_native_generator_from_evaluator(name, eval)?;
        recorder.set_native_overrides(name, kwargs)?;
        Ok(NoneType)
    }

    fn constraint_setting<'v>(
        #[starlark(this)] _native: Value<'v>,
        name: &str,
        visibility: Option<UnpackListOrTuple<&str>>,
        #[starlark(kwargs)] kwargs: SmallMap<String, Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let recorder = PackageRecorder::from_evaluator(eval)?;
        recorder.native_toolchain_target_with_visibility(
            name.to_owned(),
            NativeToolchainTarget::ConstraintSetting,
            visibility.map(list),
        )?;
        recorder.set_native_generator_from_evaluator(name, eval)?;
        recorder.set_native_overrides(name, kwargs)?;
        Ok(NoneType)
    }

    fn constraint_value<'v>(
        #[starlark(this)] _native: Value<'v>,
        name: &str,
        constraint_setting: &str,
        visibility: Option<UnpackListOrTuple<&str>>,
        #[starlark(kwargs)] kwargs: SmallMap<String, Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let recorder = PackageRecorder::from_evaluator(eval)?;
        recorder.native_toolchain_target_with_visibility(
            name.to_owned(),
            NativeToolchainTarget::ConstraintValue {
                constraint_setting: recorder.native_toolchain_label(constraint_setting)?,
            },
            visibility.map(list),
        )?;
        recorder.set_native_generator_from_evaluator(name, eval)?;
        recorder.set_native_overrides(name, kwargs)?;
        Ok(NoneType)
    }

    fn platform<'v>(
        #[starlark(this)] _native: Value<'v>,
        name: &str,
        #[starlark(default = UnpackList::default())] constraint_values: UnpackList<&str>,
        visibility: Option<UnpackListOrTuple<&str>>,
        #[starlark(kwargs)] kwargs: SmallMap<String, Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let recorder = PackageRecorder::from_evaluator(eval)?;
        recorder.native_toolchain_target_with_visibility(
            name.to_owned(),
            NativeToolchainTarget::Platform {
                constraint_values: recorder.native_toolchain_labels(&constraint_values.items)?,
            },
            visibility.map(list),
        )?;
        recorder.set_native_generator_from_evaluator(name, eval)?;
        recorder.set_native_overrides(name, kwargs)?;
        Ok(NoneType)
    }

    fn toolchain_type<'v>(
        #[starlark(this)] _native: Value<'v>,
        name: &str,
        visibility: Option<UnpackListOrTuple<&str>>,
        #[starlark(kwargs)] kwargs: SmallMap<String, Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let recorder = PackageRecorder::from_evaluator(eval)?;
        recorder.native_toolchain_target_with_visibility(
            name.to_owned(),
            NativeToolchainTarget::ToolchainType,
            visibility.map(list),
        )?;
        recorder.set_native_generator_from_evaluator(name, eval)?;
        recorder.set_native_overrides(name, kwargs)?;
        Ok(NoneType)
    }

    fn toolchain<'v>(
        #[starlark(this)] _native: Value<'v>,
        name: &str,
        toolchain: &str,
        toolchain_type: &str,
        #[starlark(default = UnpackList::default())] exec_compatible_with: UnpackList<&str>,
        visibility: Option<UnpackListOrTuple<&str>>,
        #[starlark(kwargs)] kwargs: SmallMap<String, Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let recorder = PackageRecorder::from_evaluator(eval)?;
        recorder.native_toolchain_target_with_visibility(
            name.to_owned(),
            NativeToolchainTarget::Toolchain {
                toolchain_type: recorder.native_toolchain_label(toolchain_type)?,
                implementation: recorder.native_toolchain_label(toolchain)?,
                exec_compatible_with: recorder
                    .native_toolchain_labels(&exec_compatible_with.items)?,
            },
            visibility.map(list),
        )?;
        recorder.set_native_generator_from_evaluator(name, eval)?;
        recorder.set_native_overrides(name, kwargs)?;
        Ok(NoneType)
    }

    fn package_group(
        #[starlark(this)] _native: Value,
        name: &str,
        #[starlark(default=UnpackListOrTuple::default())] packages: UnpackListOrTuple<&str>,
        #[starlark(default=UnpackListOrTuple::default())] includes: UnpackListOrTuple<&str>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        PackageRecorder::from_evaluator(eval)?.package_group(
            name.to_owned(),
            list(packages),
            list(includes),
        )?;
        Ok(NoneType)
    }

    fn glob<'v>(
        #[starlark(this)] _native: Value<'v>,
        #[starlark(default=UnpackListOrTuple::default())] include: UnpackListOrTuple<&str>,
        #[starlark(default=UnpackListOrTuple::default())] exclude: UnpackListOrTuple<&str>,
        #[starlark(default = 1)] exclude_directories: i32,
        allow_empty: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Vec<String>> {
        glob_global(include, exclude, exclude_directories, allow_empty, eval)
    }
}

#[starlark_value(type = "native")]
impl<'v> StarlarkValue<'v> for NativeModule {
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(native_methods)
    }
}

impl AllocFrozenValue for NativeModule {
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        heap.alloc_simple(self)
    }
}

pub(crate) fn loading_globals() -> Globals {
    let mut globals = GlobalsBuilder::extended_by(&[LibraryExtension::Print])
        .with(package_globals)
        .with(select_globals);
    globals.set("native", NativeModule);
    globals.set("attr", AttrModule);
    globals.set("config", ConfigModule);
    globals.set("platform_common", PlatformCommonModule);
    globals.set("DefaultInfo", AnalysisBuiltinCallable::new("DefaultInfo"));
    globals.set("depset", AnalysisBuiltinCallable::new("depset"));
    globals.build()
}
