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
use slug_build_api_v2::ProviderId;
use slug_bzlmod_v2::NonrootAttributeValue;
use slug_identity_v2::ApparentLabel;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::PackageIdentifier;
use slug_starlark_v2::populate_universe;
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
use starlark::values::UnpackValue;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark::values::dict::DictRef;
use starlark::values::list::ListRef;
use starlark::values::list::UnpackList;
use starlark::values::list_or_tuple::UnpackListOrTuple;
use starlark::values::none::NoneType;
use starlark::values::starlark_value;
use starlark::values::tuple::TupleRef;
use starlark::values::typing::StarlarkCallable;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::attrs::AllowSingleFile;
use crate::attrs::AllowedAttributeValues;
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
use crate::bzl_visibility::bzl_visibility_globals;
use crate::cc_common::cc_common_globals;
use crate::glob::GlobError;
use crate::glob::GlobSpec;
use crate::glob::PackageListing;
use crate::glob::expand_glob;
use crate::host_glob::HostGlobLoadingOperation;
use crate::host_glob::HostGlobLoadingRequest;
use crate::host_glob::HostGlobPrepared;
use crate::host_glob::HostGlobRequestTraversalError;
use crate::module_extension_repository_rule::RepositoryRuleAttribute;
use crate::module_extension_repository_rule::RepositoryRuleDefinition;
use crate::provider::AnalysisBuiltinCallable;
use crate::provider::BzlEvaluationContext;
use crate::provider::FrozenUserProviderCallable;
use crate::provider::OutputGroupInfo;
use crate::provider::RunEnvironmentInfo;
use crate::provider::UserProviderCallable;
use crate::provider::user_provider_from_arguments;
use crate::starlark_label::StarlarkLabel;
use crate::starlark_label::label_globals;
use crate::starlark_label::resolve_label;
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative)]
pub(crate) enum BuildSettingKind {
    Integer { flag: bool },
    String { flag: bool, allow_multiple: bool },
    Boolean { flag: bool },
    StringList { flag: bool, repeatable: bool },
}

/// One rule/aspect toolchain type requirement detached from its Starlark value.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct ToolchainTypeRequirement {
    label: CanonicalLabel,
    mandatory: bool,
}

impl ToolchainTypeRequirement {
    pub fn label(&self) -> &CanonicalLabel {
        &self.label
    }

    pub fn mandatory(&self) -> bool {
        self.mandatory
    }
}

impl fmt::Display for ToolchainTypeRequirement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.label.fmt(f)
    }
}

impl PartialEq<CanonicalLabel> for ToolchainTypeRequirement {
    fn eq(&self, other: &CanonicalLabel) -> bool {
        self.label == *other
    }
}

impl BuildSettingKind {
    fn attribute_kind(self) -> AttributeKind {
        match self {
            Self::Integer { .. } => AttributeKind::Integer,
            Self::String { .. } => AttributeKind::String,
            Self::Boolean { .. } => AttributeKind::Boolean,
            Self::StringList { .. } => AttributeKind::StringList,
        }
    }
}

#[derive(Debug, Clone, Allocative)]
pub struct StarlarkRuleImplementation {
    #[allocative(skip)]
    implementation: FrozenValue,
    dependencies: Arc<[CanonicalLabel]>,
    required_toolchains: Arc<[ToolchainTypeRequirement]>,
    schema: Arc<[AttributeSchema]>,
    values: Arc<[AttributeValue]>,
    capability: Arc<RuleCapability>,
    build_setting_kind: Option<BuildSettingKind>,
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
            && self.build_setting_kind == other.build_setting_kind
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
    pub fn required_toolchains(&self) -> &[ToolchainTypeRequirement] {
        &self.required_toolchains
    }

    pub fn schema(&self) -> &[AttributeSchema] {
        &self.schema
    }

    pub fn values(&self) -> &[AttributeValue] {
        &self.values
    }
    pub fn is_root_string_build_setting(&self) -> bool {
        self.build_setting_kind
            == Some(BuildSettingKind::String {
                flag: true,
                allow_multiple: false,
            })
    }
    pub fn root_string_build_setting_default(&self) -> Option<&str> {
        self.is_root_string_build_setting().then(|| {
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
        let mut values = values
            .into_iter()
            .map(|(key, value)| (CompactString::from(key), CompactString::from(value)))
            .collect::<Vec<_>>();
        values.sort_unstable();
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
        required_toolchains: Arc<[ToolchainTypeRequirement]>,
        capability: Arc<RuleCapability>,
        schema: Arc<[AttributeSchema]>,
        values: Arc<[AttributeValue]>,
        build_setting_kind: Option<BuildSettingKind>,
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
                build_setting_kind,
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
        AttributeKind::StringListDict => CoercedAttributeValue::StringListDict(Arc::from([])),
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

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct StarlarkToolchainTypeRequirement(ToolchainTypeRequirement);

impl fmt::Display for StarlarkToolchainTypeRequirement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("config_common.toolchain_type")
    }
}

starlark::starlark_simple_value!(StarlarkToolchainTypeRequirement);

#[starlark_value(type = "toolchain_type")]
impl<'v> StarlarkValue<'v> for StarlarkToolchainTypeRequirement {
    fn get_attr(&self, name: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match name {
            "toolchain_type" => Some(heap.alloc_simple(StarlarkLabel::new(self.0.label.clone()))),
            "mandatory" => Some(Value::new_bool(self.0.mandatory)),
            _ => None,
        }
    }
}

fn direct_toolchain_label(
    value: &str,
    source: &BzlModuleIdentity,
) -> anyhow::Result<CanonicalLabel> {
    let target = value.rsplit_once(':').map(|(_, target)| target);
    let recursive = target.is_none() && (value == "..." || value.ends_with("/..."));
    if recursive || matches!(target, Some("all" | "all-targets" | "*")) {
        anyhow::bail!("toolchains requires a direct target label: {value}");
    }
    if value.starts_with("@@") {
        CanonicalLabel::parse(value).map_err(anyhow::Error::msg)
    } else if value.starts_with('@') || value.starts_with("//") || value.starts_with(':') {
        resolve_label(value, source)
    } else {
        let provisional = package_context_label(source.label.package().package().as_str(), value)?;
        let repo = source.label.package().repo();
        if repo.is_root() {
            Ok(provisional)
        } else {
            provisional
                .rebind_provisional_root_repository(repo)
                .map_err(anyhow::Error::msg)
        }
    }
}

fn toolchain_requirements(
    value: Option<Value>,
    eval: &Evaluator<'_, '_, '_>,
) -> anyhow::Result<Arc<[ToolchainTypeRequirement]>> {
    let Some(value) = value else {
        return Ok(Arc::from([]));
    };
    let values =
        ListRef::from_value(value).ok_or_else(|| anyhow::anyhow!("toolchains requires a list"))?;
    let context = BzlEvaluationContext::from_evaluator(eval)?;
    let source = context.source_identity_for_call(eval)?;
    let mut requirements = Vec::with_capacity(values.len());
    let mut labels = SmallSet::new();
    for value in values.iter() {
        let requirement = if let Some(value) = StarlarkToolchainTypeRequirement::from_value(value) {
            value.0.clone()
        } else if let Some(value) = StarlarkLabel::from_value(value) {
            ToolchainTypeRequirement {
                label: value.canonical().clone(),
                mandatory: true,
            }
        } else if let Some(value) = value.unpack_str() {
            ToolchainTypeRequirement {
                label: direct_toolchain_label(value, source)?,
                mandatory: true,
            }
        } else {
            anyhow::bail!("toolchains entries must be Strings, Labels, or toolchain_type values");
        };
        if !labels.insert(requirement.label.clone()) {
            anyhow::bail!(
                "duplicate toolchain requirement is not supported: {}",
                requirement.label
            );
        }
        requirements.push(requirement);
    }
    Ok(requirements.into())
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
    if let Some(values) = TupleRef::from_value(value) {
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
        AttributeKind::StringListDict => CoercedAttributeValue::StringListDict(Arc::from([])),
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
        AttributeKind::StringListDict => {
            let RawAttributeValue::Dict(values) = raw else {
                anyhow::bail!("attribute must be a dictionary");
            };
            Ok(CoercedAttributeValue::StringListDict(
                values
                    .iter()
                    .map(|(key, value)| {
                        let RawAttributeValue::List(values) = value else {
                            anyhow::bail!("attribute dictionary values must be lists");
                        };
                        Ok((
                            raw_string(key, "dictionary key")?,
                            values
                                .iter()
                                .map(|value| raw_string(value, "dictionary list"))
                                .collect::<anyhow::Result<Vec<_>>>()?
                                .into(),
                        ))
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?
                    .into(),
            ))
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
    if kind == AttributeKind::Label && value.is_none() {
        return Ok(CoercedAttributeValue::None);
    }
    if matches!(
        kind,
        AttributeKind::LabelList | AttributeKind::OutputList | AttributeKind::StringList
    ) && ListRef::from_value(value).is_none()
        && TupleRef::from_value(value).is_none()
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
    required_toolchains: Arc<[ToolchainTypeRequirement]>,
    #[trace(unsafe_ignore)]
    schema: Arc<[RuleAttributeSchemaGen<V>]>,
    executable: bool,
    test: bool,
    build_setting_kind: Option<BuildSettingKind>,
    #[trace(unsafe_ignore)]
    rule_class: OnceCell<CompactString>,
}

/// The frozen definition contains no export-time interior mutability. Its
/// shared capability is cloned into every package instance of this rule.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub(crate) struct FrozenRuleDefinition {
    implementation: FrozenValue,
    required_toolchains: Arc<[ToolchainTypeRequirement]>,
    pub(crate) schema: Arc<[FrozenRuleAttributeSchema]>,
    capability: Arc<RuleCapability>,
    pub(crate) build_setting_kind: Option<BuildSettingKind>,
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

impl FrozenRuleDefinition {
    #[cfg(test)]
    pub(crate) fn required_toolchains(&self) -> &[ToolchainTypeRequirement] {
        &self.required_toolchains
    }

    #[cfg(test)]
    pub(crate) fn capability(&self) -> &RuleCapability {
        &self.capability
    }

    fn reject_deferred_attribute_invocation(&self) -> anyhow::Result<()> {
        if self
            .required_toolchains
            .iter()
            .any(|requirement| !requirement.mandatory)
        {
            anyhow::bail!("optional rule toolchain requirements are not supported at invocation");
        }
        if let Some(attribute) = self
            .schema
            .iter()
            .find(|attribute| attribute.executable || attribute.exec_configuration)
        {
            anyhow::bail!(
                "target invocation for executable or exec-configured attribute '{}' is not supported",
                attribute.name
            );
        }
        if let Some(attribute) = self.schema.iter().find(|attribute| {
            !attribute.required_providers.is_empty() || attribute.attached_aspect.is_some()
        }) {
            anyhow::bail!(
                "target invocation for provider-constrained or aspect-bearing attribute '{}' is not supported",
                attribute.name
            );
        }
        if matches!(
            self.build_setting_kind,
            Some(BuildSettingKind::Integer { .. })
        ) {
            anyhow::bail!("integer build setting rule invocation is not supported");
        }
        if matches!(
            self.build_setting_kind,
            Some(BuildSettingKind::Boolean { .. })
        ) {
            anyhow::bail!("boolean build setting rule invocation is not supported");
        }
        if matches!(
            self.build_setting_kind,
            Some(
                BuildSettingKind::String { flag: false, .. }
                    | BuildSettingKind::String {
                        allow_multiple: true,
                        ..
                    }
            )
        ) {
            anyhow::bail!(
                "non-flag or allow-multiple string build setting rule invocation is not supported"
            );
        }
        Ok(())
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
            build_setting_kind: self.build_setting_kind,
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

/// The declaration returned by Bazel's `aspect()` global while its defining
/// `.bzl` module is still evaluating. Aspect implementations are retained for
/// later analysis, but loading never executes them.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Trace)]
struct AspectDefinitionGen<V> {
    implementation: V,
    #[trace(unsafe_ignore)]
    attr_aspects: Arc<[CompactString]>,
    #[trace(unsafe_ignore)]
    attributes: Arc<[RuleAttributeSchemaGen<V>]>,
    required_aspect: Option<V>,
    #[trace(unsafe_ignore)]
    required_toolchains: Arc<[ToolchainTypeRequirement]>,
    #[trace(unsafe_ignore)]
    required_providers: Arc<[Arc<[ProviderId]>]>,
    #[trace(unsafe_ignore)]
    advertised_providers: Arc<[ProviderId]>,
    #[trace(unsafe_ignore)]
    required_fragments: Arc<[CompactString]>,
    #[trace(unsafe_ignore)]
    defining_label: CanonicalLabel,
    #[trace(unsafe_ignore)]
    exported_name: OnceCell<CompactString>,
}

/// Frozen aspect identity owned by the defining Bzl module. Imported aliases
/// preserve this producer identity instead of acquiring an importer identity.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
#[allow(dead_code)] // Retained now; configured-aspect consumers are deliberately deferred.
pub(crate) struct FrozenAspectDefinition {
    implementation: FrozenValue,
    pub(crate) attr_aspects: Arc<[CompactString]>,
    pub(crate) attributes: Arc<[FrozenRuleAttributeSchema]>,
    pub(crate) required_aspect: Option<FrozenValue>,
    pub(crate) required_toolchains: Arc<[ToolchainTypeRequirement]>,
    pub(crate) required_providers: Arc<[Arc<[ProviderId]>]>,
    pub(crate) advertised_providers: Arc<[ProviderId]>,
    pub(crate) required_fragments: Arc<[CompactString]>,
    pub(crate) defining_label: CanonicalLabel,
    pub(crate) exported_name: Option<CompactString>,
}

type AspectDefinition<'v> = AspectDefinitionGen<Value<'v>>;

impl<V> fmt::Display for AspectDefinitionGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<aspect>")
    }
}

impl fmt::Display for FrozenAspectDefinition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<aspect>")
    }
}

starlark::starlark_complex_values!(AspectDefinition);

impl<'v> Freeze for AspectDefinition<'v> {
    type Frozen = FrozenAspectDefinition;

    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        Ok(FrozenAspectDefinition {
            implementation: self.implementation.freeze(freezer)?,
            attr_aspects: self.attr_aspects,
            attributes: self
                .attributes
                .iter()
                .cloned()
                .map(|schema| schema.freeze(freezer))
                .collect::<FreezeResult<Vec<_>>>()?
                .into(),
            required_aspect: self
                .required_aspect
                .map(|aspect| aspect.freeze(freezer))
                .transpose()?,
            required_toolchains: self.required_toolchains,
            required_providers: self.required_providers,
            advertised_providers: self.advertised_providers,
            required_fragments: self.required_fragments,
            defining_label: self.defining_label,
            exported_name: self.exported_name.into_inner(),
        })
    }
}

fn aspect_provider_id(value: Value) -> anyhow::Result<ProviderId> {
    if let Some(provider) = value.downcast_ref::<UserProviderCallable>() {
        return provider
            .id()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("aspect providers must be exported"));
    }
    if let Some(provider) = value.downcast_ref::<FrozenUserProviderCallable>() {
        return Ok(provider.id().clone());
    }
    anyhow::bail!("aspect providers must be user provider constructors")
}

fn aspect_required_providers(value: Option<Value>) -> anyhow::Result<Arc<[Arc<[ProviderId]>]>> {
    let Some(value) = value else {
        return Ok(Arc::from([]));
    };
    let alternatives = ListRef::from_value(value)
        .ok_or_else(|| anyhow::anyhow!("aspect required_providers must be a nested list"))?;
    if alternatives.len() != 2 {
        anyhow::bail!("only two singleton aspect provider alternatives are supported");
    }
    alternatives
        .iter()
        .map(|alternative| {
            let providers = ListRef::from_value(alternative).ok_or_else(|| {
                anyhow::anyhow!("aspect required_providers must be a nested list")
            })?;
            if providers.len() != 1 {
                anyhow::bail!("aspect required_providers alternatives must be singletons");
            }
            Ok(Arc::from([aspect_provider_id(providers[0])?]))
        })
        .collect::<anyhow::Result<Vec<_>>>()
        .map(Arc::from)
}

fn aspect_advertised_providers(value: Option<Value>) -> anyhow::Result<Arc<[ProviderId]>> {
    let Some(value) = value else {
        return Ok(Arc::from([]));
    };
    let providers = ListRef::from_value(value)
        .ok_or_else(|| anyhow::anyhow!("aspect provides must be a list"))?;
    let [provider] = providers.content() else {
        anyhow::bail!("only one advertised aspect provider is supported");
    };
    Ok(Arc::from([aspect_provider_id(*provider)?]))
}

fn aspect_required_aspect(value: Option<Value>) -> anyhow::Result<Option<Value>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let values = ListRef::from_value(value)
        .ok_or_else(|| anyhow::anyhow!("aspect requires must be a list"))?;
    let [required] = values.content() else {
        anyhow::bail!("only one required aspect is supported");
    };
    let exported = required
        .downcast_ref::<AspectDefinition>()
        .is_some_and(|aspect| aspect.exported_name.get().is_some())
        || required
            .downcast_ref::<FrozenAspectDefinition>()
            .is_some_and(|aspect| aspect.exported_name.is_some());
    if !exported {
        anyhow::bail!("aspect requires must contain one exported aspect");
    }
    Ok(Some(*required))
}

fn label_list_required_providers(value: Option<Value>) -> anyhow::Result<Arc<[Arc<[ProviderId]>]>> {
    let explicit = value.is_some();
    let providers = aspect_required_providers(value)?;
    if explicit && providers[0] == providers[1] {
        anyhow::bail!("label_list provider alternatives must be distinct");
    }
    Ok(providers)
}

fn label_required_provider(value: Option<Value>) -> anyhow::Result<Arc<[Arc<[ProviderId]>]>> {
    let Some(value) = value else {
        return Ok(Arc::from([]));
    };
    let providers = ListRef::from_value(value)
        .ok_or_else(|| anyhow::anyhow!("label providers must be a list"))?;
    if providers.is_empty() {
        return Ok(Arc::from([]));
    }
    let [provider] = providers.content() else {
        anyhow::bail!("label providers supports exactly one exported provider");
    };
    Ok(Arc::from([Arc::from([aspect_provider_id(*provider)?])]))
}

fn label_list_attached_aspect(value: Option<Value>) -> anyhow::Result<Option<Value>> {
    aspect_required_aspect(value)
}

fn aspect_attributes<'v>(
    attrs: Option<SmallMap<String, Value<'v>>>,
    defining_label: &CanonicalLabel,
) -> anyhow::Result<Arc<[RuleAttributeSchema<'v>]>> {
    let Some(attrs) = attrs else {
        return Ok(Arc::from([]));
    };
    let names = attrs.keys().map(String::as_str).collect::<Vec<_>>();
    let rustfmt = ["_config", "_process_wrapper"];
    let clippy = [
        "_capture_output",
        "_clippy_error_format",
        "_clippy_flag",
        "_clippy_flags",
        "_clippy_output_diagnostics",
        "_config",
        "_error_format",
        "_extra_rustc_flag",
        "_incompatible_change_clippy_error_format",
        "_per_crate_rustc_flag",
        "_process_wrapper",
    ];
    let is_rustfmt = names == rustfmt;
    if !is_rustfmt && names != clippy {
        anyhow::bail!("only the fixed rustfmt and clippy aspect attributes are supported");
    }
    let attribute_count = names.len();
    drop(names);
    let repo = defining_label.package().repo().as_str();
    let mut schemas = Vec::with_capacity(attribute_count);
    for (name, value) in attrs {
        let definition = AttributeDefinition::from_value(value)
            .and_then(|value| match value {
                starlark::__macro_refs::Either::Left(value) => Some(value),
                starlark::__macro_refs::Either::Right(_) => None,
            })
            .ok_or_else(|| anyhow::anyhow!("aspect attribute `{name}` must use attr.label()"))?;
        let (label, allow_single_file, executable, exec_configuration) = match name.as_str() {
            "_capture_output" => (
                format!("@@{repo}//rust/settings:capture_clippy_output"),
                None,
                false,
                false,
            ),
            "_clippy_error_format" => (
                format!("@@{repo}//rust/settings:clippy_error_format"),
                None,
                false,
                false,
            ),
            "_clippy_flag" => (
                format!("@@{repo}//rust/settings:clippy_flag"),
                None,
                false,
                false,
            ),
            "_clippy_flags" => (
                format!("@@{repo}//rust/settings:clippy_flags"),
                None,
                false,
                false,
            ),
            "_clippy_output_diagnostics" => (
                format!("@@{repo}//rust/settings:clippy_output_diagnostics"),
                None,
                false,
                false,
            ),
            "_config" => (
                format!(
                    "@@{repo}//rust/settings:{}",
                    if is_rustfmt {
                        "rustfmt.toml"
                    } else {
                        "clippy.toml"
                    }
                ),
                Some(AllowSingleFile::True),
                false,
                false,
            ),
            "_error_format" => (
                format!("@@{repo}//rust/settings:error_format"),
                None,
                false,
                false,
            ),
            "_extra_rustc_flag" => (
                format!("@@{repo}//rust/settings:extra_rustc_flag"),
                None,
                false,
                false,
            ),
            "_incompatible_change_clippy_error_format" => (
                format!("@@{repo}//rust/settings:incompatible_change_clippy_error_format"),
                None,
                false,
                false,
            ),
            "_per_crate_rustc_flag" => (
                format!("@@{repo}//rust/settings:per_crate_rustc_flag"),
                None,
                false,
                false,
            ),
            "_process_wrapper" => (
                format!("@@{repo}//util/process_wrapper:process_wrapper"),
                None,
                true,
                true,
            ),
            _ => unreachable!(),
        };
        let expected_default = CoercedAttributeValue::Label(
            CanonicalLabel::parse(&label).map_err(anyhow::Error::msg)?,
        );
        if definition.kind != AttributeKind::Label
            || definition.mandatory
            || !definition.configurable
            || definition.configurable_set
            || definition.allow_files
            || definition.allow_single_file != allow_single_file
            || !matches!(definition.allowed_values, AllowedAttributeValues::None)
            || definition.default.as_ref() != Some(&expected_default)
            || definition.executable != executable
            || definition.exec_configuration != exec_configuration
            || !definition.required_providers.is_empty()
            || definition.attached_aspect.is_some()
            || definition.transition.is_some()
        {
            anyhow::bail!("aspect attribute `{name}` does not match the admitted fixed schema");
        }
        schemas.push(declared_attribute_schema(name, definition));
    }
    Ok(schemas.into())
}

#[starlark_value(type = "aspect")]
impl<'v> StarlarkValue<'v> for AspectDefinition<'v> {
    type Canonical = FrozenAspectDefinition;

    fn export_as(
        &self,
        variable_name: &str,
        _eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<()> {
        if self.exported_name.get().is_none() {
            let _ = self.exported_name.set(variable_name.into());
        }
        Ok(())
    }
}

#[starlark_value(type = "aspect")]
impl<'v> StarlarkValue<'v> for FrozenAspectDefinition {
    type Canonical = Self;
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
pub(crate) struct RuleAttributeSchemaGen<V> {
    #[trace(unsafe_ignore)]
    pub(crate) name: CompactString,
    #[trace(unsafe_ignore)]
    pub(crate) kind: AttributeKind,
    #[trace(unsafe_ignore)]
    pub(crate) mandatory: bool,
    #[trace(unsafe_ignore)]
    pub(crate) configurable: bool,
    #[trace(unsafe_ignore)]
    pub(crate) default: Option<CoercedAttributeValue>,
    pub(crate) transition: Option<TransitionDefinitionGen<V>>,
    #[trace(unsafe_ignore)]
    builtin: bool,
    #[trace(unsafe_ignore)]
    configurable_set: bool,
    #[trace(unsafe_ignore)]
    pub(crate) allow_files: bool,
    #[trace(unsafe_ignore)]
    pub(crate) allow_single_file: Option<AllowSingleFile>,
    #[trace(unsafe_ignore)]
    pub(crate) allowed_values: AllowedAttributeValues,
    #[trace(unsafe_ignore)]
    pub(crate) executable: bool,
    #[trace(unsafe_ignore)]
    pub(crate) exec_configuration: bool,
    #[trace(unsafe_ignore)]
    pub(crate) required_providers: Arc<[Arc<[ProviderId]>]>,
    pub(crate) attached_aspect: Option<V>,
}
type RuleAttributeSchema<'v> = RuleAttributeSchemaGen<Value<'v>>;
type FrozenRuleAttributeSchema = RuleAttributeSchemaGen<FrozenValue>;

fn declared_attribute_schema<'v>(
    name: String,
    definition: &AttributeDefinition<'v>,
) -> RuleAttributeSchema<'v> {
    RuleAttributeSchema {
        name: name.into(),
        kind: definition.kind,
        mandatory: definition.mandatory,
        configurable: definition.configurable,
        default: definition.default.clone(),
        transition: definition.transition.clone(),
        builtin: false,
        configurable_set: false,
        allow_files: definition.allow_files,
        allow_single_file: definition.allow_single_file.clone(),
        allowed_values: definition.allowed_values.clone(),
        executable: definition.executable,
        exec_configuration: definition.exec_configuration,
        required_providers: definition.required_providers.clone(),
        attached_aspect: definition.attached_aspect,
    }
}

// These are loading-owned RuleClass members, rather than public `attr.*`
// descriptors.  Keeping the finite shape here lets target invocation retain
// the same typed values as user declarations without broadening the
// descriptor surface.
fn starlark_builtin_schema<V>(
    executable: bool,
    test: bool,
    build_setting_kind: Option<BuildSettingKind>,
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
            configurable_set: false,
            allow_files: false,
            allow_single_file: None,
            allowed_values: AllowedAttributeValues::None,
            executable: false,
            exec_configuration: false,
            required_providers: Arc::from([]),
            attached_aspect: None,
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
    if let Some(kind) = build_setting_kind {
        push("build_setting_default", kind.attribute_kind(), true, false);
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

fn validate_allowed_value(
    attribute_name: &str,
    value: &CoercedAttributeValue,
    allowed: &AllowedAttributeValues,
) -> anyhow::Result<()> {
    match allowed {
        AllowedAttributeValues::None => Ok(()),
        AllowedAttributeValues::Integer(allowed) => {
            validate_allowed_integer_value(attribute_name, value, allowed)
        }
        AllowedAttributeValues::String(allowed) => {
            for candidate in value.attr_visible_candidates(|label| label.to_string().into())? {
                if allowed.binary_search(&candidate).is_err() {
                    anyhow::bail!(
                        "invalid value in `{attribute_name}` attribute: {candidate} is not allowed"
                    );
                }
            }
            Ok(())
        }
    }
}

fn validate_allowed_integer_value(
    attribute_name: &str,
    value: &CoercedAttributeValue,
    allowed: &[i32],
) -> anyhow::Result<()> {
    match value {
        CoercedAttributeValue::Integer(value) if allowed.binary_search(value).is_ok() => Ok(()),
        CoercedAttributeValue::Integer(value) => {
            anyhow::bail!("invalid value in `{attribute_name}` attribute: {value} is not allowed")
        }
        CoercedAttributeValue::Selector { branches, default } => {
            for (_, value) in branches.iter() {
                validate_allowed_integer_value(attribute_name, value, allowed)?;
            }
            if let Some(value) = default {
                validate_allowed_integer_value(attribute_name, value, allowed)?;
            }
            Ok(())
        }
        CoercedAttributeValue::Concatenation(_, _) => anyhow::bail!(
            "integer allowed values on concatenated select expressions are not supported"
        ),
        _ => anyhow::bail!("attribute `{attribute_name}` must be an integer"),
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
pub(crate) struct TransitionDefinitionGen<V> {
    implementation: V,
    #[trace(unsafe_ignore)]
    #[freeze(identity)]
    output: CompactString,
}
type TransitionDefinition<'v> = TransitionDefinitionGen<Value<'v>>;
type FrozenTransitionDefinition = TransitionDefinitionGen<FrozenValue>;
starlark::starlark_complex_values!(TransitionDefinition);
impl FrozenTransitionDefinition {
    #[cfg(test)]
    pub(crate) fn implementation(&self) -> FrozenValue {
        self.implementation
    }

    #[cfg(test)]
    pub(crate) fn output(&self) -> &str {
        &self.output
    }
}
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
    configurable_set: bool,
    #[trace(unsafe_ignore)]
    allow_files: bool,
    #[trace(unsafe_ignore)]
    allow_single_file: Option<AllowSingleFile>,
    #[trace(unsafe_ignore)]
    allowed_values: AllowedAttributeValues,
    #[trace(unsafe_ignore)]
    default: Option<CoercedAttributeValue>,
    #[trace(unsafe_ignore)]
    executable: bool,
    #[trace(unsafe_ignore)]
    exec_configuration: bool,
    #[trace(unsafe_ignore)]
    required_providers: Arc<[Arc<[ProviderId]>]>,
    attached_aspect: Option<V>,
    transition: Option<TransitionDefinitionGen<V>>,
}
type AttributeDefinition<'v> = AttributeDefinitionGen<Value<'v>>;
type FrozenAttributeDefinition = AttributeDefinitionGen<FrozenValue>;
starlark::starlark_complex_values!(AttributeDefinition);

fn rule_attribute_definition_from_value<'v>(value: Value<'v>) -> Option<AttributeDefinition<'v>> {
    match AttributeDefinition::from_value(value)? {
        starlark::__macro_refs::Either::Left(value) => Some(value.clone()),
        starlark::__macro_refs::Either::Right(value)
            if value.required_providers.is_empty()
                && value.attached_aspect.is_none()
                && value.transition.is_none() =>
        {
            Some(AttributeDefinitionGen {
                kind: value.kind,
                mandatory: value.mandatory,
                configurable: value.configurable,
                configurable_set: value.configurable_set,
                allow_files: value.allow_files,
                allow_single_file: value.allow_single_file.clone(),
                allowed_values: value.allowed_values.clone(),
                default: value.default.clone(),
                executable: value.executable,
                exec_configuration: value.exec_configuration,
                required_providers: value.required_providers.clone(),
                attached_aspect: None,
                transition: None,
            })
        }
        starlark::__macro_refs::Either::Right(_) => None,
    }
}
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
            configurable_set: self.configurable_set,
            allow_files: self.allow_files,
            allow_single_file: self.allow_single_file,
            allowed_values: self.allowed_values,
            default: self.default,
            executable: self.executable,
            exec_configuration: self.exec_configuration,
            required_providers: self.required_providers,
            attached_aspect: self
                .attached_aspect
                .map(|value| value.freeze(freezer))
                .transpose()?,
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
            configurable_set: self.configurable_set,
            allow_files: self.allow_files,
            allow_single_file: self.allow_single_file,
            allowed_values: self.allowed_values,
            default: self.default,
            executable: self.executable,
            exec_configuration: self.exec_configuration,
            required_providers: self.required_providers,
            attached_aspect: self
                .attached_aspect
                .map(|value| value.freeze(freezer))
                .transpose()?,
            transition: self
                .transition
                .map(|value| value.freeze(freezer))
                .transpose()?,
            builtin: self.builtin,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct ModuleExtensionTagAttribute {
    pub(crate) name: CompactString,
    pub(crate) kind: AttributeKind,
    pub(crate) mandatory: bool,
    pub(crate) configurable: bool,
    pub(crate) default: Option<CoercedAttributeValue>,
    pub(crate) allow_single_file: Option<AllowSingleFile>,
}

pub(crate) type ModuleExtensionTagCoercionError = CompactString;

fn module_extension_label(
    raw: &str,
    context_repo: &CanonicalRepoName,
    mapping: &SmallMap<ApparentRepoName, CanonicalRepoName>,
) -> Result<CanonicalLabel, ModuleExtensionTagCoercionError> {
    let apparent = ApparentLabel::parse(raw).map_err(CompactString::from)?;
    let repository = if apparent.repo().is_root() {
        context_repo
    } else {
        mapping.get(apparent.repo()).ok_or_else(|| {
            CompactString::from(format!(
                "no repository visible as '@{}'",
                apparent.repo().as_str()
            ))
        })?
    };
    let canonical = if repository.is_root() {
        format!("@@//{}:{}", apparent.package(), apparent.target())
    } else {
        format!(
            "@@{}//{}:{}",
            repository.as_str(),
            apparent.package(),
            apparent.target()
        )
    };
    CanonicalLabel::parse(&canonical).map_err(CompactString::from)
}

fn coerce_module_extension_scalar(
    kind: AttributeKind,
    raw: &NonrootAttributeValue,
    context_repo: &CanonicalRepoName,
    mapping: &SmallMap<ApparentRepoName, CanonicalRepoName>,
) -> Result<CoercedAttributeValue, ModuleExtensionTagCoercionError> {
    match (kind, raw) {
        (AttributeKind::String, NonrootAttributeValue::String(value)) => {
            Ok(CoercedAttributeValue::String(value.clone()))
        }
        (AttributeKind::Boolean, NonrootAttributeValue::Bool(value)) => {
            Ok(CoercedAttributeValue::Boolean(*value))
        }
        (AttributeKind::Integer, NonrootAttributeValue::Int(value)) => value
            .as_i32()
            .map(CoercedAttributeValue::Integer)
            .ok_or_else(|| CompactString::from("integer is outside i32")),
        (
            AttributeKind::Label,
            NonrootAttributeValue::String(value) | NonrootAttributeValue::Label(value),
        ) => module_extension_label(value, context_repo, mapping).map(CoercedAttributeValue::Label),
        _ => Err(format!("unsupported value for module-extension {kind:?} attribute").into()),
    }
}

fn module_extension_intrinsic_default(kind: AttributeKind) -> CoercedAttributeValue {
    match kind {
        AttributeKind::String => CoercedAttributeValue::String(CompactString::new("")),
        AttributeKind::Boolean => CoercedAttributeValue::Boolean(false),
        AttributeKind::Integer => CoercedAttributeValue::Integer(0),
        AttributeKind::Label => CoercedAttributeValue::None,
        _ => unreachable!("caller validates the admitted scalar kind"),
    }
}

fn module_extension_default_matches(kind: AttributeKind, value: &CoercedAttributeValue) -> bool {
    matches!(
        (kind, value),
        (AttributeKind::String, CoercedAttributeValue::String(_))
            | (AttributeKind::Boolean, CoercedAttributeValue::Boolean(_))
            | (AttributeKind::Integer, CoercedAttributeValue::Integer(_))
            | (
                AttributeKind::Label,
                CoercedAttributeValue::Label(_) | CoercedAttributeValue::None
            )
    )
}

pub(crate) fn prepare_module_extension_tag_attributes(
    schema: &[ModuleExtensionTagAttribute],
    raw: &SmallMap<CompactString, NonrootAttributeValue>,
    context_repo: &CanonicalRepoName,
    mapping: &SmallMap<ApparentRepoName, CanonicalRepoName>,
) -> Result<Arc<[(CompactString, CoercedAttributeValue)]>, ModuleExtensionTagCoercionError> {
    validate_module_extension_tag_schema(schema)?;
    let mut supplied = SmallMap::new();
    for (name, raw) in raw {
        if matches!(raw, NonrootAttributeValue::None) {
            continue;
        }
        let attribute = schema
            .iter()
            .find(|attribute| attribute.name == *name)
            .ok_or_else(|| CompactString::from(format!("unknown attribute '{name}'")))?;
        supplied.insert(
            name.clone(),
            coerce_module_extension_scalar(attribute.kind, raw, context_repo, mapping)?,
        );
    }
    schema
        .iter()
        .map(|attribute| {
            let value = if let Some(value) = supplied.get(&attribute.name) {
                value.clone()
            } else if attribute.mandatory {
                return Err(format!(
                    "mandatory attribute '{}' isn't being specified",
                    attribute.name
                )
                .into());
            } else {
                attribute
                    .default
                    .clone()
                    .unwrap_or_else(|| module_extension_intrinsic_default(attribute.kind))
            };
            if let CoercedAttributeValue::Label(label) = &value {
                let repo = label.package().repo();
                if repo != context_repo && !mapping.values().any(|visible| visible == repo) {
                    return Err(
                        format!("no repository visible as '{}': default label", repo).into(),
                    );
                }
            }
            Ok((attribute.name.clone(), value))
        })
        .collect::<Result<Arc<_>, _>>()
}

pub(crate) fn validate_module_extension_tag_schema(
    schema: &[ModuleExtensionTagAttribute],
) -> Result<(), ModuleExtensionTagCoercionError> {
    for attribute in schema {
        if !matches!(
            attribute.kind,
            AttributeKind::String
                | AttributeKind::Boolean
                | AttributeKind::Integer
                | AttributeKind::Label
        ) || attribute
            .default
            .as_ref()
            .is_some_and(|value| !module_extension_default_matches(attribute.kind, value))
        {
            return Err(format!(
                "unsupported module-extension attribute schema '{}': {:?}",
                attribute.name, attribute.kind
            )
            .into());
        }
    }
    Ok(())
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    ProvidesStaticType,
    NoSerialize,
    Allocative
)]
struct TagClassDefinition {
    attributes: Arc<[ModuleExtensionTagAttribute]>,
}

starlark::starlark_simple_value!(TagClassDefinition);

impl fmt::Display for TagClassDefinition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("tag_class")
    }
}

#[starlark_value(type = "tag_class")]
impl<'v> StarlarkValue<'v> for TagClassDefinition {
    type Canonical = Self;
}

#[derive(Debug, Clone, Trace, ProvidesStaticType, NoSerialize, Allocative)]
struct ModuleExtensionDefinitionGen<V> {
    implementation: V,
    #[trace(unsafe_ignore)]
    tag_classes: Arc<[(CompactString, Arc<[ModuleExtensionTagAttribute]>)]>,
    #[trace(unsafe_ignore)]
    environment: Arc<[CompactString]>,
    os_dependent: bool,
    arch_dependent: bool,
    facts_version: i32,
}

type ModuleExtensionDefinition<'v> = ModuleExtensionDefinitionGen<Value<'v>>;

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
#[allow(dead_code)] // Frozen callable is lifetime-only until extension execution activation.
pub(crate) struct FrozenModuleExtensionDefinition {
    #[allocative(skip)]
    pub(crate) implementation: FrozenValue,
    tag_classes: Arc<[(CompactString, Arc<[ModuleExtensionTagAttribute]>)]>,
    environment: Arc<[CompactString]>,
    os_dependent: bool,
    arch_dependent: bool,
    facts_version: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
#[allow(dead_code)] // Projected only by the callerless definition-loading owner.
pub(crate) struct ModuleExtensionDefinitionProjection {
    pub(crate) tag_classes: Arc<[(CompactString, Arc<[ModuleExtensionTagAttribute]>)]>,
    pub(crate) environment: Arc<[CompactString]>,
    pub(crate) os_dependent: bool,
    pub(crate) arch_dependent: bool,
    pub(crate) facts_version: i32,
}

impl FrozenModuleExtensionDefinition {
    #[allow(dead_code)]
    pub(crate) fn projection(&self) -> ModuleExtensionDefinitionProjection {
        let _lifetime_only = self.implementation;
        ModuleExtensionDefinitionProjection {
            tag_classes: self.tag_classes.clone(),
            environment: self.environment.clone(),
            os_dependent: self.os_dependent,
            arch_dependent: self.arch_dependent,
            facts_version: self.facts_version,
        }
    }
}

impl fmt::Display for ModuleExtensionDefinitionGen<Value<'_>> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("module_extension")
    }
}

impl fmt::Display for FrozenModuleExtensionDefinition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("module_extension")
    }
}

starlark::starlark_complex_values!(ModuleExtensionDefinition);

impl<'v> Freeze for ModuleExtensionDefinition<'v> {
    type Frozen = FrozenModuleExtensionDefinition;

    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        Ok(FrozenModuleExtensionDefinition {
            implementation: self.implementation.freeze(freezer)?,
            tag_classes: self.tag_classes,
            environment: self.environment,
            os_dependent: self.os_dependent,
            arch_dependent: self.arch_dependent,
            facts_version: self.facts_version,
        })
    }
}

#[starlark_value(type = "module_extension")]
impl<'v> StarlarkValue<'v> for ModuleExtensionDefinition<'v> {
    type Canonical = FrozenModuleExtensionDefinition;
}

#[starlark_value(type = "module_extension")]
impl<'v> StarlarkValue<'v> for FrozenModuleExtensionDefinition {
    type Canonical = Self;
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
    configurable: Option<bool>,
    allow_single_file: Option<AllowSingleFile>,
    executable: bool,
    default: Option<Value<'v>>,
    cfg: Option<Value<'v>>,
    eval: &Evaluator<'v, '_, '_>,
) -> anyhow::Result<AttributeDefinition<'v>> {
    if executable && !cfg.as_ref().is_some_and(|value| !value.is_none()) {
        anyhow::bail!("cfg parameter is mandatory when executable=True is provided");
    }
    let default = default
        .map(|value| {
            if value.is_none() && kind == AttributeKind::Label {
                return Ok(CoercedAttributeValue::None);
            }
            let context = BzlEvaluationContext::from_evaluator(eval)?;
            if kind == AttributeKind::Label {
                let source = context.source_identity_for_call(eval)?;
                return coerce_label_default(value, source);
            }
            let raw = raw_attribute_value(value)?;
            let source = context.source_label_for_call(eval)?;
            coerce_raw_value(source.package().package().as_str(), kind, &raw)
        })
        .transpose()?;
    let mut exec_configuration = false;
    let transition = cfg
        .map(|value| {
            if value.unpack_str() == Some("exec") {
                exec_configuration = true;
                return Ok(None);
            }
            TransitionDefinition::from_value(value)
                .into_iter()
                .find_map(|value| match value {
                    starlark::__macro_refs::Either::Left(value) => Some(value.clone()),
                    starlark::__macro_refs::Either::Right(value) => Some(TransitionDefinitionGen {
                        implementation: value.implementation.to_value(),
                        output: value.output.clone(),
                    }),
                })
                .map(Some)
                .ok_or_else(|| anyhow::anyhow!("attr.label cfg must be 'exec' or a transition"))
        })
        .transpose()?
        .flatten();
    Ok(AttributeDefinition {
        kind,
        mandatory,
        configurable: configurable.unwrap_or(!matches!(
            kind,
            AttributeKind::Output | AttributeKind::OutputList
        )),
        configurable_set: configurable.is_some(),
        allow_files: false,
        allow_single_file,
        allowed_values: AllowedAttributeValues::None,
        default,
        executable,
        exec_configuration,
        required_providers: Arc::from([]),
        attached_aspect: None,
        transition,
    })
}

fn coerce_label_default(
    value: Value<'_>,
    source: &BzlModuleIdentity,
) -> anyhow::Result<CoercedAttributeValue> {
    if let Some(label) = StarlarkLabel::from_value(value) {
        return Ok(CoercedAttributeValue::Label(label.canonical().clone()));
    }
    let raw = raw_attribute_value(value)?;
    let RawAttributeValue::String(raw) = &raw else {
        return coerce_raw_value(
            source.label.package().package().as_str(),
            AttributeKind::Label,
            &raw,
        );
    };
    let label = if raw.starts_with('@') || raw.starts_with("//") || raw.starts_with(':') {
        resolve_label(raw, source)?
    } else {
        resolve_label(&format!(":{raw}"), source)?
    };
    Ok(CoercedAttributeValue::Label(label))
}

fn discard_attribute_doc(doc: Option<Value>) -> anyhow::Result<()> {
    if doc.is_some_and(|value| !value.is_none() && value.unpack_str().is_none()) {
        anyhow::bail!("attribute doc must be a string or None");
    }
    Ok(())
}

fn unpack_allow_single_file(value: Option<Value>) -> anyhow::Result<Option<AllowSingleFile>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_none() {
        return Ok(None);
    }
    if let Some(value) = value.unpack_bool() {
        return Ok(Some(if value {
            AllowSingleFile::True
        } else {
            AllowSingleFile::False
        }));
    }
    let values = if let Some(values) = ListRef::from_value(value) {
        values.iter().collect::<Vec<_>>()
    } else if let Some(values) = TupleRef::from_value(value) {
        values.iter().collect::<Vec<_>>()
    } else {
        anyhow::bail!("allow_single_file must be a bool or a sequence of file extensions")
    };
    let extensions = values
        .into_iter()
        .map(|value| {
            value
                .unpack_str()
                .map(CompactString::new)
                .ok_or_else(|| anyhow::anyhow!("allow_single_file extensions must be strings"))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(Some(AllowSingleFile::Extensions(extensions.into())))
}

fn unpack_boolean_allow_files(value: Option<Value>) -> anyhow::Result<bool> {
    let Some(value) = value else {
        return Ok(false);
    };
    if value.is_none() {
        return Ok(false);
    }
    value
        .unpack_bool()
        .ok_or_else(|| anyhow::anyhow!("allow_files must be a bool or None"))
}

fn normalize_allowed_integer_values(values: Option<UnpackListOrTuple<i32>>) -> Arc<[i32]> {
    let mut values = values.unwrap_or_default().items;
    values.sort_unstable();
    values.dedup();
    values.into()
}
fn normalize_allowed_string_values(
    values: Option<UnpackListOrTuple<&str>>,
) -> AllowedAttributeValues {
    let mut values = values
        .unwrap_or_default()
        .items
        .into_iter()
        .map(CompactString::from)
        .collect::<Vec<_>>();
    values.sort_unstable();
    values.dedup();
    if values.is_empty() {
        AllowedAttributeValues::None
    } else {
        AllowedAttributeValues::String(values.into())
    }
}

#[starlark_module]
fn attr_methods(builder: &mut MethodsBuilder) {
    fn label<'v>(
        #[starlark(this)] _attr: Value<'v>,
        #[starlark(require = named)] mandatory: Option<bool>,
        #[starlark(require = named)] configurable: Option<bool>,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named)] cfg: Option<Value<'v>>,
        #[starlark(require = named)] allow_files: Option<Value<'v>>,
        #[starlark(require = named)] allow_single_file: Option<Value<'v>>,
        #[starlark(require = named)] providers: Option<Value<'v>>,
        #[starlark(require = named)] executable: Option<bool>,
        #[starlark(require = named)] doc: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<AttributeDefinition<'v>> {
        discard_attribute_doc(doc)?;
        if allow_files.is_some_and(|value| !value.is_none())
            && allow_single_file.is_some_and(|value| !value.is_none())
        {
            anyhow::bail!("allow_files and allow_single_file cannot both be set");
        }
        let mut definition = attribute_definition(
            AttributeKind::Label,
            mandatory.unwrap_or(false),
            configurable,
            unpack_allow_single_file(allow_single_file)?,
            executable.unwrap_or(false),
            default,
            cfg,
            eval,
        )?;
        definition.allow_files = unpack_boolean_allow_files(allow_files)?;
        definition.required_providers = label_required_provider(providers)?;
        Ok(definition)
    }
    fn label_list<'v>(
        #[starlark(this)] _attr: Value<'v>,
        #[starlark(require = named)] mandatory: Option<bool>,
        #[starlark(require = named)] configurable: Option<bool>,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named)] doc: Option<Value<'v>>,
        #[starlark(require = named)] allow_files: Option<Value<'v>>,
        #[starlark(require = named)] providers: Option<Value<'v>>,
        #[starlark(require = named)] cfg: Option<Value<'v>>,
        #[starlark(require = named)] aspects: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<AttributeDefinition<'v>> {
        discard_attribute_doc(doc)?;
        let required_providers = label_list_required_providers(providers)?;
        let mut definition = attribute_definition(
            AttributeKind::LabelList,
            mandatory.unwrap_or(false),
            configurable,
            None,
            false,
            default,
            cfg,
            eval,
        )?;
        definition.allow_files = unpack_boolean_allow_files(allow_files)?;
        definition.required_providers = required_providers;
        definition.attached_aspect = label_list_attached_aspect(aspects)?;
        Ok(definition)
    }
    fn string_keyed_label_dict<'v>(
        #[starlark(this)] _attr: Value<'v>,
        #[starlark(require = named)] mandatory: Option<bool>,
        #[starlark(require = named)] configurable: Option<bool>,
        #[starlark(require = named)] default: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<AttributeDefinition<'v>> {
        attribute_definition(
            AttributeKind::StringKeyedLabelDict,
            mandatory.unwrap_or(false),
            configurable,
            None,
            false,
            default,
            None,
            eval,
        )
    }
    fn label_keyed_string_dict<'v>(
        #[starlark(this)] _attr: Value<'v>,
        #[starlark(require = named)] mandatory: Option<bool>,
        #[starlark(require = named)] configurable: Option<bool>,
        #[starlark(require = named)] default: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<AttributeDefinition<'v>> {
        attribute_definition(
            AttributeKind::LabelKeyedStringDict,
            mandatory.unwrap_or(false),
            configurable,
            None,
            false,
            default,
            None,
            eval,
        )
    }
    fn bool<'v>(
        #[starlark(this)] _attr: Value<'v>,
        #[starlark(require = named)] mandatory: Option<bool>,
        #[starlark(require = named)] configurable: Option<bool>,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named)] doc: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<AttributeDefinition<'v>> {
        discard_attribute_doc(doc)?;
        attribute_definition(
            AttributeKind::Boolean,
            mandatory.unwrap_or(false),
            configurable,
            None,
            false,
            default,
            None,
            eval,
        )
    }
    fn int<'v>(
        #[starlark(this)] _attr: Value<'v>,
        #[starlark(require = named)] mandatory: Option<bool>,
        #[starlark(require = named)] configurable: Option<bool>,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named)] doc: Option<Value<'v>>,
        #[starlark(require = named)] values: Option<UnpackListOrTuple<i32>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<AttributeDefinition<'v>> {
        discard_attribute_doc(doc)?;
        let mut definition = attribute_definition(
            AttributeKind::Integer,
            mandatory.unwrap_or(false),
            configurable,
            None,
            false,
            default,
            None,
            eval,
        )?;
        let values = normalize_allowed_integer_values(values);
        definition.allowed_values = if values.is_empty() {
            AllowedAttributeValues::None
        } else {
            AllowedAttributeValues::Integer(values)
        };
        Ok(definition)
    }
    fn label_list_dict<'v>(
        #[starlark(this)] _attr: Value<'v>,
        #[starlark(require = named)] mandatory: Option<bool>,
        #[starlark(require = named)] configurable: Option<bool>,
        #[starlark(require = named)] default: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<AttributeDefinition<'v>> {
        attribute_definition(
            AttributeKind::LabelListDict,
            mandatory.unwrap_or(false),
            configurable,
            None,
            false,
            default,
            None,
            eval,
        )
    }
    fn output<'v>(
        #[starlark(this)] _attr: Value<'v>,
        #[starlark(require = named)] mandatory: Option<bool>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<AttributeDefinition<'v>> {
        attribute_definition(
            AttributeKind::Output,
            mandatory.unwrap_or(false),
            None,
            None,
            false,
            None,
            None,
            eval,
        )
    }
    fn output_list<'v>(
        #[starlark(this)] _attr: Value<'v>,
        #[starlark(require = named)] mandatory: Option<bool>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<AttributeDefinition<'v>> {
        attribute_definition(
            AttributeKind::OutputList,
            mandatory.unwrap_or(false),
            None,
            None,
            false,
            None,
            None,
            eval,
        )
    }
    fn string<'v>(
        #[starlark(this)] _attr: Value<'v>,
        #[starlark(require = named)] mandatory: Option<bool>,
        #[starlark(require = named)] configurable: Option<bool>,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named)] doc: Option<Value<'v>>,
        #[starlark(require = named)] values: Option<UnpackListOrTuple<&str>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<AttributeDefinition<'v>> {
        discard_attribute_doc(doc)?;
        let mut definition = attribute_definition(
            AttributeKind::String,
            mandatory.unwrap_or(false),
            configurable,
            None,
            false,
            default,
            None,
            eval,
        )?;
        definition.allowed_values = normalize_allowed_string_values(values);
        Ok(definition)
    }
    fn string_list<'v>(
        #[starlark(this)] _attr: Value<'v>,
        #[starlark(require = named)] mandatory: Option<bool>,
        #[starlark(require = named)] configurable: Option<bool>,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named)] doc: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<AttributeDefinition<'v>> {
        discard_attribute_doc(doc)?;
        attribute_definition(
            AttributeKind::StringList,
            mandatory.unwrap_or(false),
            configurable,
            None,
            false,
            default,
            None,
            eval,
        )
    }
    fn string_dict<'v>(
        #[starlark(this)] _attr: Value<'v>,
        #[starlark(require = named)] mandatory: Option<bool>,
        #[starlark(require = named)] configurable: Option<bool>,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named)] doc: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<AttributeDefinition<'v>> {
        discard_attribute_doc(doc)?;
        attribute_definition(
            AttributeKind::StringDict,
            mandatory.unwrap_or(false),
            configurable,
            None,
            false,
            default,
            None,
            eval,
        )
    }
    fn string_list_dict<'v>(
        #[starlark(this)] _attr: Value<'v>,
        #[starlark(require = named)] mandatory: Option<bool>,
        #[starlark(require = named)] configurable: Option<bool>,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named)] doc: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<AttributeDefinition<'v>> {
        discard_attribute_doc(doc)?;
        attribute_definition(
            AttributeKind::StringListDict,
            mandatory.unwrap_or(false),
            configurable,
            None,
            false,
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
struct ConfigCommonModule;
starlark::starlark_simple_value!(ConfigCommonModule);
impl fmt::Display for ConfigCommonModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("config_common")
    }
}

#[starlark_module]
fn config_common_methods(builder: &mut MethodsBuilder) {
    fn toolchain_type<'v>(
        #[starlark(this)] _config_common: Value<'v>,
        name: Value<'v>,
        #[starlark(require = named, default = true)] mandatory: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<StarlarkToolchainTypeRequirement> {
        let label = if let Some(label) = StarlarkLabel::from_value(name) {
            label.canonical().clone()
        } else if let Some(raw) = name.unpack_str() {
            let source =
                BzlEvaluationContext::from_evaluator(eval)?.source_identity_for_call(eval)?;
            resolve_label(raw, source)?
        } else {
            anyhow::bail!("config_common.toolchain_type() takes a Label or String");
        };
        Ok(StarlarkToolchainTypeRequirement(ToolchainTypeRequirement {
            label,
            mandatory,
        }))
    }
}

#[starlark_value(type = "config_common")]
impl<'v> StarlarkValue<'v> for ConfigCommonModule {
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(config_common_methods)
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
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct BuildFileConfigModule;
starlark::starlark_simple_value!(BuildFileConfigModule);
impl fmt::Display for BuildFileConfigModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("config")
    }
}
#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct RootStringBuildSetting {
    flag: bool,
    allow_multiple: bool,
}
starlark::starlark_simple_value!(RootStringBuildSetting);
impl fmt::Display for RootStringBuildSetting {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("config.string")
    }
}
#[starlark_value(type = "config_string")]
impl<'v> StarlarkValue<'v> for RootStringBuildSetting {}
#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct RootIntBuildSetting {
    flag: bool,
}
starlark::starlark_simple_value!(RootIntBuildSetting);
impl fmt::Display for RootIntBuildSetting {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("config.int")
    }
}
#[starlark_value(type = "config_int")]
impl<'v> StarlarkValue<'v> for RootIntBuildSetting {}
#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct RootBoolBuildSetting {
    flag: bool,
}
starlark::starlark_simple_value!(RootBoolBuildSetting);
impl fmt::Display for RootBoolBuildSetting {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("config.bool")
    }
}
#[starlark_value(type = "config_bool")]
impl<'v> StarlarkValue<'v> for RootBoolBuildSetting {}
#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct RootStringListBuildSetting {
    flag: bool,
    repeatable: bool,
}
starlark::starlark_simple_value!(RootStringListBuildSetting);
impl fmt::Display for RootStringListBuildSetting {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("config.string_list")
    }
}
#[starlark_value(type = "config_string_list")]
impl<'v> StarlarkValue<'v> for RootStringListBuildSetting {}

fn root_string_build_setting(flag: bool) -> anyhow::Result<RootStringBuildSetting> {
    if !flag {
        anyhow::bail!("only config.string(flag = True) is supported")
    }
    Ok(RootStringBuildSetting {
        flag: true,
        allow_multiple: false,
    })
}

#[starlark_module]
fn config_methods(builder: &mut MethodsBuilder) {
    fn int(
        #[starlark(this)] _config: Value,
        #[starlark(require = named, default = false)] flag: bool,
    ) -> anyhow::Result<RootIntBuildSetting> {
        Ok(RootIntBuildSetting { flag })
    }

    fn string(
        #[starlark(this)] _config: Value,
        #[starlark(require = named, default = false)] flag: bool,
        #[starlark(require = named, default = false)] allow_multiple: bool,
    ) -> anyhow::Result<RootStringBuildSetting> {
        Ok(RootStringBuildSetting {
            flag,
            allow_multiple,
        })
    }

    fn bool(
        #[starlark(this)] _config: Value,
        #[starlark(require = named, default = false)] flag: bool,
    ) -> anyhow::Result<RootBoolBuildSetting> {
        Ok(RootBoolBuildSetting { flag })
    }

    fn string_list(
        #[starlark(this)] _config: Value,
        #[starlark(require = named, default = false)] flag: bool,
        #[starlark(require = named, default = false)] repeatable: bool,
    ) -> anyhow::Result<RootStringListBuildSetting> {
        if repeatable && !flag {
            anyhow::bail!("'repeatable' can only be set for a setting with 'flag = True'")
        }
        Ok(RootStringListBuildSetting { flag, repeatable })
    }
}
#[starlark_module]
fn build_file_config_methods(builder: &mut MethodsBuilder) {
    fn string(
        #[starlark(this)] _config: Value,
        #[starlark(default = false)] flag: bool,
    ) -> anyhow::Result<RootStringBuildSetting> {
        root_string_build_setting(flag)
    }
}
#[starlark_value(type = "config")]
impl<'v> StarlarkValue<'v> for ConfigModule {
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(config_methods)
    }
}
#[starlark_value(type = "config")]
impl<'v> StarlarkValue<'v> for BuildFileConfigModule {
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(build_file_config_methods)
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
        self.reject_deferred_attribute_invocation()
            .map_err(starlark::Error::new_other)?;
        if matches!(
            self.build_setting_kind,
            Some(BuildSettingKind::StringList { .. })
        ) {
            return Err(starlark::Error::new_other(anyhow::anyhow!(
                "string-list build setting rule invocation is not supported"
            )));
        }
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
                        .with_allow_files(declaration.allow_files)
                        .with_allow_single_file(declaration.allow_single_file.clone())
                        .with_allowed_values(declaration.allowed_values.clone())
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
                    if provenance == AttributeProvenance::Explicit {
                        validate_allowed_value(
                            &declaration.name,
                            &value,
                            attribute_schema.allowed_values(),
                        )?;
                    }
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
                    self.build_setting_kind,
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
    fn repository_rule<'v>(
        implementation: Value<'v>,
        #[starlark(require = named)] attrs: Option<Value<'v>>,
        #[starlark(require = named)] local: Option<bool>,
        #[starlark(require = named)] environ: Option<UnpackListOrTuple<&str>>,
        #[starlark(require = named)] configure: Option<bool>,
        #[starlark(require = named)] doc: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<RepositoryRuleDefinition<'v>> {
        let callable: Option<StarlarkCallable<'v>> =
            StarlarkCallable::unpack_value_opt(implementation);
        if callable.is_none() {
            anyhow::bail!("repository_rule implementation must be callable");
        }
        if local.unwrap_or(false)
            || configure.unwrap_or(false)
            || !environ.unwrap_or_default().items.is_empty()
            || doc.is_some_and(|value| !value.is_none())
        {
            anyhow::bail!("unsupported repository_rule option in the admitted capture slice");
        }
        let context = BzlEvaluationContext::from_evaluator(eval)
            .map_err(|_| anyhow::anyhow!("repository_rule may only be called in a .bzl module"))?;
        let defining_label = CanonicalLabel::parse(&format!("@@{}", context.source_label()))
            .map_err(anyhow::Error::msg)?;
        let attrs = match attrs {
            None => Vec::new(),
            Some(value) if value.is_none() => Vec::new(),
            Some(value) => DictRef::from_value(value)
                .ok_or_else(|| anyhow::anyhow!("repository_rule attrs must be a dict or None"))?
                .iter()
                .map(|(name, value)| {
                    Ok((
                        name.unpack_str()
                            .ok_or_else(|| {
                                anyhow::anyhow!("repository_rule attr names must be strings")
                            })?
                            .to_owned(),
                        value,
                    ))
                })
                .collect::<anyhow::Result<Vec<_>>>()?,
        };
        let mut attributes = Vec::new();
        for (name, value) in attrs {
            if matches!(
                name.as_str(),
                "name" | "tags" | "deprecation" | "visibility"
            ) {
                anyhow::bail!(
                    "There is already a built-in attribute '{name}' which cannot be overridden"
                );
            }
            if !is_repository_rule_attribute_name(&name) {
                anyhow::bail!("unsupported repository_rule attribute name '{name}'");
            }
            let definition = AttributeDefinition::from_value(value)
                .and_then(|value| match value {
                    starlark::__macro_refs::Either::Left(value) => Some(value),
                    starlark::__macro_refs::Either::Right(_) => None,
                })
                .ok_or_else(|| {
                    anyhow::anyhow!("repository attribute '{name}' must use attr.*()")
                })?;
            if !matches!(
                definition.kind,
                AttributeKind::String
                    | AttributeKind::Boolean
                    | AttributeKind::Integer
                    | AttributeKind::Label
            ) || definition.configurable_set
                || definition.transition.is_some()
                || definition.executable
                || definition.exec_configuration
                || definition.allow_files
                || definition.allow_single_file.is_some()
                || !definition.required_providers.is_empty()
                || !matches!(definition.allowed_values, AllowedAttributeValues::None)
                || definition
                    .default
                    .as_ref()
                    .is_some_and(|value| !module_extension_default_matches(definition.kind, value))
            {
                anyhow::bail!("unsupported repository_rule attribute schema '{name}'");
            }
            attributes.push(RepositoryRuleAttribute {
                name: name.into(),
                kind: definition.kind,
                mandatory: definition.mandatory,
                default: definition.default.clone(),
            });
        }
        Ok(RepositoryRuleDefinition::new(
            implementation,
            defining_label,
            attributes.into(),
        ))
    }

    fn tag_class<'v>(
        #[starlark(require = named)] attrs: Option<SmallMap<String, Value<'v>>>,
        #[starlark(require = named)] doc: Option<&str>,
    ) -> anyhow::Result<TagClassDefinition> {
        let _ = doc;
        let mut attributes = Vec::new();
        for (name, value) in attrs.unwrap_or_default() {
            let definition = AttributeDefinition::from_value(value)
                .and_then(|value| match value {
                    starlark::__macro_refs::Either::Left(value) => Some(value),
                    starlark::__macro_refs::Either::Right(_) => None,
                })
                .ok_or_else(|| anyhow::anyhow!("tag attribute `{name}` must use attr.*()"))?;
            if definition.transition.is_some()
                || definition.executable
                || definition.exec_configuration
            {
                anyhow::bail!("tag attribute `{name}` does not support cfg transitions");
            }
            if definition.configurable_set {
                anyhow::bail!(
                    "tag attribute `{name}` does not support explicit configurable policy"
                );
            }
            if definition.allow_files {
                anyhow::bail!("tag attribute `{name}` does not support allow_files");
            }
            if !definition.required_providers.is_empty() {
                anyhow::bail!("tag attribute `{name}` does not support providers");
            }
            if !matches!(definition.allowed_values, AllowedAttributeValues::None) {
                anyhow::bail!("tag attribute `{name}` does not support allowed values");
            }
            let name = name
                .strip_prefix('_')
                .map(|name| CompactString::from(format!("${name}")))
                .unwrap_or_else(|| name.into());
            attributes.push(ModuleExtensionTagAttribute {
                name,
                kind: definition.kind,
                mandatory: definition.mandatory,
                configurable: definition.configurable,
                default: definition.default.clone(),
                allow_single_file: definition.allow_single_file.clone(),
            });
        }
        Ok(TagClassDefinition {
            attributes: attributes.into(),
        })
    }

    fn module_extension<'v>(
        #[starlark(require = named)] implementation: Value<'v>,
        #[starlark(require = named)] tag_classes: Option<SmallMap<String, Value<'v>>>,
        #[starlark(require = named)] doc: Option<&str>,
        #[starlark(require = named)] environ: Option<UnpackListOrTuple<&str>>,
        #[starlark(require = named)] os_dependent: Option<bool>,
        #[starlark(require = named)] arch_dependent: Option<bool>,
        #[starlark(require = named)] facts_version: Option<i32>,
    ) -> anyhow::Result<ModuleExtensionDefinition<'v>> {
        let _ = doc;
        let callable: Option<StarlarkCallable<'v>> =
            StarlarkCallable::unpack_value_opt(implementation);
        if callable.is_none() {
            anyhow::bail!("module_extension implementation must be callable");
        }
        let facts_version = facts_version.unwrap_or(0);
        if facts_version < 0 {
            anyhow::bail!("facts_version must be non-negative, got {facts_version}");
        }
        let mut retained_tag_classes = Vec::new();
        for (name, value) in tag_classes.unwrap_or_default() {
            let tag_class = TagClassDefinition::from_value(value)
                .ok_or_else(|| anyhow::anyhow!("tag class `{name}` must use tag_class()"))?;
            retained_tag_classes.push((name.into(), tag_class.attributes.clone()));
        }
        Ok(ModuleExtensionDefinitionGen {
            implementation,
            tag_classes: retained_tag_classes.into(),
            environment: environ
                .unwrap_or_else(UnpackListOrTuple::default)
                .items
                .into_iter()
                .map(CompactString::new)
                .collect::<Vec<_>>()
                .into(),
            os_dependent: os_dependent.unwrap_or(false),
            arch_dependent: arch_dependent.unwrap_or(false),
            facts_version,
        })
    }

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
        toolchains: Option<Value<'v>>,
        #[starlark(require = named)] doc: Option<Value<'v>>,
        #[starlark(default = false)] executable: bool,
        #[starlark(default = false)] test: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<RuleDefinition<'v>> {
        if doc.is_some_and(|value| !value.is_none() && value.unpack_str().is_none()) {
            anyhow::bail!("rule doc must be a string or None");
        }
        let build_setting_kind = build_setting.and_then(|value| {
            if let Some(setting) = RootIntBuildSetting::from_value(value) {
                Some(BuildSettingKind::Integer { flag: setting.flag })
            } else if let Some(setting) = RootStringBuildSetting::from_value(value) {
                Some(BuildSettingKind::String {
                    flag: setting.flag,
                    allow_multiple: setting.allow_multiple,
                })
            } else if let Some(setting) = RootBoolBuildSetting::from_value(value) {
                Some(BuildSettingKind::Boolean { flag: setting.flag })
            } else if let Some(setting) = RootStringListBuildSetting::from_value(value) {
                Some(BuildSettingKind::StringList {
                    flag: setting.flag,
                    repeatable: setting.repeatable,
                })
            } else {
                None
            }
        });
        if build_setting.is_some() && build_setting_kind.is_none() {
            anyhow::bail!(
                "only rule(build_setting = config.int(), config.string(), config.bool(), or config.string_list()) is supported"
            )
        }
        let declared_builtin_names =
            starlark_builtin_schema::<Value<'v>>(executable, test, build_setting_kind, true);
        let mut user_schema = Vec::new();
        if let Some(attrs) = attrs {
            for (name, value) in attrs {
                if declared_builtin_names
                    .iter()
                    .any(|schema| schema.name == name)
                {
                    anyhow::bail!("rule attribute `{name}` is built in and cannot be redeclared");
                }
                let definition = rule_attribute_definition_from_value(value)
                    .ok_or_else(|| anyhow::anyhow!("rule attribute `{name}` must use attr.*()"))?;
                if definition.configurable_set {
                    anyhow::bail!(
                        "attribute '{name}' has the 'configurable' argument set, which is not allowed in rule definitions"
                    );
                }
                user_schema.push(declared_attribute_schema(name, &definition));
            }
        }
        let has_transition = user_schema.iter().any(|schema| schema.transition.is_some());
        let mut schema =
            starlark_builtin_schema(executable, test, build_setting_kind, has_transition);
        schema.extend(user_schema);
        Ok(RuleDefinition {
            implementation,
            required_toolchains: toolchain_requirements(toolchains, eval)?,
            schema: schema.into(),
            executable,
            test,
            build_setting_kind,
            rule_class: OnceCell::new(),
        })
    }

    fn provider<'v>(
        doc: Option<Value<'v>>,
        #[starlark(require = named)] fields: Option<Value<'v>>,
        #[starlark(require = named)] init: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        user_provider_from_arguments(doc, fields, init, eval)
    }
    fn transition<'v>(
        #[starlark(require = named)] implementation: Value<'v>,
        #[starlark(require = named)] inputs: UnpackListOrTuple<&str>,
        #[starlark(require = named)] outputs: UnpackListOrTuple<&str>,
    ) -> anyhow::Result<TransitionDefinition<'v>> {
        let inputs = list(inputs);
        let outputs = list(outputs);
        let [output] = outputs.as_slice() else {
            anyhow::bail!(
                "only transition(inputs = [], outputs = [one main-repository target label]) is supported"
            )
        };
        if !inputs.is_empty()
            || !output.starts_with("//")
            || transition_output_has_recursive_package_segment(output)
        {
            anyhow::bail!(
                "only transition(inputs = [], outputs = [one main-repository target label]) is supported"
            )
        }
        let label = CanonicalLabel::parse(&format!("@@{output}")).map_err(anyhow::Error::msg)?;
        if !label.package().repo().is_root() {
            anyhow::bail!("transition output must be a direct main-repository target label")
        }
        Ok(TransitionDefinitionGen {
            implementation,
            output: output.into(),
        })
    }
}

#[starlark_module]
fn aspect_globals(builder: &mut GlobalsBuilder) {
    fn aspect<'v>(
        implementation: Value<'v>,
        #[starlark(require = named)] attr_aspects: Option<UnpackList<&str>>,
        #[starlark(require = named)] attrs: Option<SmallMap<String, Value<'v>>>,
        #[starlark(require = named)] toolchains: Option<Value<'v>>,
        #[starlark(require = named)] required_providers: Option<Value<'v>>,
        #[starlark(require = named)] requires: Option<Value<'v>>,
        #[starlark(require = named)] provides: Option<Value<'v>>,
        #[starlark(require = named)] fragments: Option<UnpackList<&str>>,
        #[starlark(require = named)] doc: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<AspectDefinition<'v>> {
        if implementation.parameters_spec().is_none() {
            anyhow::bail!("aspect implementation must be a Starlark function");
        }
        if doc.is_some_and(|value| !value.is_none() && value.unpack_str().is_none()) {
            anyhow::bail!("aspect doc must be a string or None");
        }
        let attr_aspects: Arc<[CompactString]> = attr_aspects
            .map_or_else(Vec::new, |values| values.items)
            .into_iter()
            .map(CompactString::new)
            .collect::<Vec<_>>()
            .into();
        if attr_aspects.len() != 1 && attr_aspects.iter().any(|name| name == "*") {
            anyhow::bail!("'*' must be the only string in 'attr_aspects' list");
        }
        let context = BzlEvaluationContext::from_evaluator(eval)
            .map_err(|_| anyhow::anyhow!("aspect may only be called in a .bzl module"))?;
        let source_label = context.source_label();
        let canonical_source = if source_label.starts_with("@@") {
            source_label.to_owned()
        } else {
            format!("@@{source_label}")
        };
        let defining_label =
            CanonicalLabel::parse(&canonical_source).map_err(anyhow::Error::msg)?;
        let attributes = aspect_attributes(attrs, &defining_label)?;
        let required_aspect = aspect_required_aspect(requires)?;
        let required_toolchains = toolchain_requirements(toolchains, eval)?;
        let required_providers = aspect_required_providers(required_providers)?;
        let advertised_providers = aspect_advertised_providers(provides)?;
        let required_fragments: Arc<[CompactString]> = match fragments {
            None => Arc::from([]),
            Some(fragments) if fragments.items.as_slice() == ["cpp"] => {
                Arc::from([CompactString::new("cpp")])
            }
            Some(_) => anyhow::bail!("only aspect(fragments = ['cpp']) is supported"),
        };
        Ok(AspectDefinitionGen {
            implementation,
            attr_aspects,
            attributes,
            required_aspect,
            required_toolchains,
            required_providers,
            advertised_providers,
            required_fragments,
            defining_label,
            exported_name: OnceCell::new(),
        })
    }
}

fn is_repository_rule_attribute_name(name: &str) -> bool {
    name.bytes()
        .enumerate()
        .all(|(index, byte)| byte.is_ascii_alphanumeric() || (index > 0 && byte == b'_'))
        && name.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
}

fn transition_output_has_recursive_package_segment(output: &str) -> bool {
    let Some(label) = output.strip_prefix("//") else {
        return false;
    };
    let package = label.split_once(':').map_or(label, |(package, _)| package);
    package.split('/').any(|segment| segment == "...")
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

#[starlark_module]
fn bzl_only_globals(builder: &mut GlobalsBuilder) {
    fn configuration_field(fragment: &str, name: &str) -> anyhow::Result<NoneType> {
        let _ = (fragment, name);
        anyhow::bail!("configuration_field is unsupported in Slug loading")
    }
}

fn complete_loading_globals(bool_config: bool) -> Globals {
    let mut globals = GlobalsBuilder::new();
    populate_universe(&mut globals);
    package_globals(&mut globals);
    select_globals(&mut globals);
    globals.set("native", NativeModule);
    globals.set("attr", AttrModule);
    if bool_config {
        LibraryExtension::StructType.add(&mut globals);
        bzl_only_globals(&mut globals);
        bzl_visibility_globals(&mut globals);
        globals.set("config", ConfigModule);
        globals.set("config_common", ConfigCommonModule);
        aspect_globals(&mut globals);
        cc_common_globals(&mut globals);
        label_globals(&mut globals);
        globals.set("OutputGroupInfo", OutputGroupInfo);
        globals.set("RunEnvironmentInfo", RunEnvironmentInfo);
    } else {
        globals.set("config", BuildFileConfigModule);
    }
    globals.set("platform_common", PlatformCommonModule);
    globals.set("DefaultInfo", AnalysisBuiltinCallable::new("DefaultInfo"));
    globals.set("depset", AnalysisBuiltinCallable::new("depset"));
    globals.build()
}

pub(crate) fn loading_globals() -> Globals {
    complete_loading_globals(true)
}

pub(crate) fn build_file_loading_globals() -> Globals {
    complete_loading_globals(false)
}

#[cfg(test)]
mod module_extension_definition_tests {
    use slug_bzlmod_v2::NonrootAttributeInt;
    use starlark::environment::Module;
    use starlark::syntax::AstModule;
    use starlark::syntax::Dialect;

    use super::*;

    fn evaluate(source: &str) -> anyhow::Result<starlark::environment::FrozenModule> {
        let ast = AstModule::parse("//:ext.bzl", source.to_owned(), &Dialect::Standard)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        let module = Module::new();
        let context = BzlEvaluationContext::new("//:ext.bzl".to_owned());
        let mut evaluator = Evaluator::new(&module);
        evaluator.extra = Some(&context);
        evaluator
            .eval_module(ast, &loading_globals())
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        drop(evaluator);
        Ok(module.freeze()?)
    }

    fn projection(source: &str) -> ModuleExtensionDefinitionProjection {
        evaluate(source)
            .unwrap()
            .get("ext")
            .unwrap()
            .downcast::<FrozenModuleExtensionDefinition>()
            .unwrap()
            .projection()
    }

    #[test]
    fn module_extension_globals_admit_repository_rule_and_label() {
        let module =
            evaluate("def impl(ctx):\n  pass\ncaptured = repository_rule(implementation=impl)\n")
                .unwrap();
        assert!(
            module
                .get("captured")
                .unwrap()
                .downcast::<crate::module_extension_repository_rule::FrozenRepositoryRuleDefinition>()
                .is_ok()
        );
        evaluate("captured = Label\n").unwrap();
    }

    fn tag_attribute(
        name: &str,
        kind: AttributeKind,
        mandatory: bool,
        default: Option<CoercedAttributeValue>,
    ) -> ModuleExtensionTagAttribute {
        ModuleExtensionTagAttribute {
            name: name.into(),
            kind,
            mandatory,
            configurable: true,
            default,
            allow_single_file: None,
        }
    }

    fn root_context() -> (
        CanonicalRepoName,
        SmallMap<ApparentRepoName, CanonicalRepoName>,
    ) {
        (
            CanonicalRepoName::root(),
            SmallMap::from_iter([(
                ApparentRepoName::new("dep").unwrap(),
                CanonicalRepoName::new("dep+").unwrap(),
            )]),
        )
    }

    #[test]
    fn prepared_tag_scalar_matrix_defaults_and_label_mapping() {
        let schema = [
            tag_attribute("text", AttributeKind::String, false, None),
            tag_attribute("flag", AttributeKind::Boolean, false, None),
            tag_attribute("count", AttributeKind::Integer, false, None),
            tag_attribute("target", AttributeKind::Label, false, None),
        ];
        let raw = SmallMap::from_iter([
            (
                CompactString::from("text"),
                NonrootAttributeValue::String("value".into()),
            ),
            (
                CompactString::from("flag"),
                NonrootAttributeValue::Bool(true),
            ),
            (
                CompactString::from("count"),
                NonrootAttributeValue::Int(NonrootAttributeInt::from_decimal("7").unwrap()),
            ),
            (
                CompactString::from("target"),
                NonrootAttributeValue::Label("@dep//pkg:item".into()),
            ),
        ]);
        let (context, mapping) = root_context();
        let prepared =
            prepare_module_extension_tag_attributes(&schema, &raw, &context, &mapping).unwrap();
        assert_eq!(prepared[0].1, CoercedAttributeValue::String("value".into()));
        assert_eq!(prepared[1].1, CoercedAttributeValue::Boolean(true));
        assert_eq!(prepared[2].1, CoercedAttributeValue::Integer(7));
        assert_eq!(
            prepared[3].1,
            CoercedAttributeValue::Label(CanonicalLabel::parse("@@dep+//pkg:item").unwrap())
        );

        let omitted =
            SmallMap::from_iter([(CompactString::from("text"), NonrootAttributeValue::None)]);
        let defaults =
            prepare_module_extension_tag_attributes(&schema, &omitted, &context, &mapping).unwrap();
        assert_eq!(defaults[0].1, CoercedAttributeValue::String("".into()));
        assert_eq!(defaults[1].1, CoercedAttributeValue::Boolean(false));
        assert_eq!(defaults[2].1, CoercedAttributeValue::Integer(0));
        assert_eq!(defaults[3].1, CoercedAttributeValue::None);
    }

    #[test]
    fn prepared_tag_preserves_supplied_then_schema_error_order() {
        let schema = [
            tag_attribute("first", AttributeKind::String, true, None),
            tag_attribute("second", AttributeKind::Boolean, true, None),
        ];
        let (context, mapping) = root_context();
        let unknown_first = SmallMap::from_iter([
            (
                CompactString::from("unknown"),
                NonrootAttributeValue::String("x".into()),
            ),
            (
                CompactString::from("first"),
                NonrootAttributeValue::Bool(true),
            ),
        ]);
        assert_eq!(
            prepare_module_extension_tag_attributes(&schema, &unknown_first, &context, &mapping,)
                .unwrap_err()
                .to_string(),
            "unknown attribute 'unknown'"
        );
        let type_first = SmallMap::from_iter([
            (
                CompactString::from("first"),
                NonrootAttributeValue::Bool(true),
            ),
            (
                CompactString::from("unknown"),
                NonrootAttributeValue::String("x".into()),
            ),
        ]);
        assert!(
            prepare_module_extension_tag_attributes(&schema, &type_first, &context, &mapping)
                .unwrap_err()
                .to_string()
                .contains("String")
        );
        assert_eq!(
            prepare_module_extension_tag_attributes(&schema, &SmallMap::new(), &context, &mapping,)
                .unwrap_err()
                .to_string(),
            "mandatory attribute 'first' isn't being specified"
        );
        let invisible = SmallMap::from_iter([(
            CompactString::from("target"),
            NonrootAttributeValue::String("@missing//:x".into()),
        )]);
        assert!(
            prepare_module_extension_tag_attributes(
                &[tag_attribute("target", AttributeKind::Label, false, None)],
                &invisible,
                &context,
                &mapping,
            )
            .unwrap_err()
            .to_string()
            .contains("no repository visible")
        );

        let visible_default = [tag_attribute(
            "target",
            AttributeKind::Label,
            false,
            Some(CoercedAttributeValue::Label(
                CanonicalLabel::parse("@@dep+//:default").unwrap(),
            )),
        )];
        assert!(
            prepare_module_extension_tag_attributes(
                &visible_default,
                &SmallMap::new(),
                &context,
                &mapping,
            )
            .is_ok()
        );
        let invisible_default = [
            tag_attribute("first", AttributeKind::String, true, None),
            tag_attribute(
                "target",
                AttributeKind::Label,
                false,
                Some(CoercedAttributeValue::Label(
                    CanonicalLabel::parse("@@missing+//:default").unwrap(),
                )),
            ),
        ];
        assert_eq!(
            prepare_module_extension_tag_attributes(
                &invisible_default,
                &SmallMap::new(),
                &context,
                &mapping,
            )
            .unwrap_err()
            .as_str(),
            "mandatory attribute 'first' isn't being specified"
        );
        assert!(
            prepare_module_extension_tag_attributes(
                &invisible_default,
                &SmallMap::from_iter([(
                    CompactString::from("first"),
                    NonrootAttributeValue::String("set".into()),
                )]),
                &context,
                &mapping,
            )
            .unwrap_err()
            .contains("missing+")
        );
    }

    #[test]
    fn prepared_tag_fails_closed_on_every_deferred_family() {
        let (context, mapping) = root_context();
        for kind in [
            AttributeKind::LabelList,
            AttributeKind::StringKeyedLabelDict,
            AttributeKind::LabelKeyedStringDict,
            AttributeKind::LabelListDict,
            AttributeKind::Output,
            AttributeKind::OutputList,
            AttributeKind::StringList,
            AttributeKind::StringListDict,
            AttributeKind::StringDict,
        ] {
            assert!(
                prepare_module_extension_tag_attributes(
                    &[tag_attribute("value", kind, false, None)],
                    &SmallMap::new(),
                    &context,
                    &mapping,
                )
                .is_err(),
                "unexpected admitted kind: {kind:?}"
            );
        }
        let deferred = [
            NonrootAttributeValue::List(Arc::from([])),
            NonrootAttributeValue::Tuple(Arc::from([])),
            NonrootAttributeValue::Dict(Arc::new(SmallMap::new())),
            NonrootAttributeValue::Int(NonrootAttributeInt::from_decimal("2147483648").unwrap()),
            NonrootAttributeValue::Float314,
            NonrootAttributeValue::BuiltinPrint,
            NonrootAttributeValue::ExtensionProxy,
            NonrootAttributeValue::SelfList,
        ];
        for value in deferred {
            assert!(
                prepare_module_extension_tag_attributes(
                    &[tag_attribute("value", AttributeKind::String, false, None)],
                    &SmallMap::from_iter([(CompactString::from("value"), value)]),
                    &context,
                    &mapping,
                )
                .is_err()
            );
        }
        assert!(
            prepare_module_extension_tag_attributes(
                &[tag_attribute(
                    "value",
                    AttributeKind::String,
                    false,
                    Some(CoercedAttributeValue::Boolean(false)),
                )],
                &SmallMap::new(),
                &context,
                &mapping,
            )
            .is_err()
        );
    }

    #[test]
    fn definition_retains_ordered_schema_and_factors() {
        let source = r#"
def _impl(ctx):
    pass
first = tag_class(attrs = {
    "message": attr.string(mandatory = True),
    "input": attr.label(default = "//:default", allow_single_file = [".txt"]),
}, doc = "first tags")
second = tag_class(attrs = {"count": attr.int(default = 2)})
ext = module_extension(
    implementation = _impl,
    tag_classes = {"first": first, "second": second},
    environ = ["B", "A", "B"],
    os_dependent = True,
    arch_dependent = True,
    facts_version = 3,
    doc = "extension docs",
)
"#;
        let value = projection(source);
        assert_eq!(value.tag_classes[0].0, "first");
        assert_eq!(value.tag_classes[1].0, "second");
        assert_eq!(value.tag_classes[0].1[0].name, "message");
        assert_eq!(value.tag_classes[0].1[1].name, "input");
        assert!(value.tag_classes[0].1[0].mandatory);
        assert!(value.tag_classes[0].1[1].configurable);
        assert_eq!(value.environment.as_ref(), ["B", "A", "B"]);
        assert!(value.os_dependent);
        assert!(value.arch_dependent);
        assert_eq!(value.facts_version, 3);
        assert!(matches!(
            value.tag_classes[0].1[1].allow_single_file,
            Some(AllowSingleFile::Extensions(_))
        ));
    }

    #[test]
    fn definition_fields_change_and_restore_structural_identity() {
        let source = |mandatory: bool, default: &str, facts: i32| {
            let mandatory = if mandatory { "True" } else { "False" };
            format!(
                "def _impl(ctx):\n    pass\n\
                 tag = tag_class(attrs = {{'value': attr.string(mandatory = {mandatory}, default = '{default}')}})\n\
                 ext = module_extension(implementation = _impl, tag_classes = {{'tag': tag}}, facts_version = {facts})\n"
            )
        };
        let a = projection(&source(false, "a", 1));
        let b = projection(&source(true, "b", 2));
        let restored = projection(&source(false, "a", 1));
        assert_ne!(a, b);
        assert_eq!(a, restored);
    }

    #[test]
    fn definition_failures_are_closed_before_publication() {
        let cases = [
            "ext = module_extension(implementation = 1)",
            "def _impl(ctx):\n    pass\ntag = tag_class(attrs = {'x': attr.string(configurable = False)})\next = module_extension(implementation = _impl, tag_classes = {'tag': tag})",
            "def _impl(ctx):\n    pass\ntag = tag_class(attrs = {'x': attr.string(values = ['x'])})\next = module_extension(implementation = _impl, tag_classes = {'tag': tag})",
            "P = provider()\ndef _impl(ctx):\n    pass\ntag = tag_class(attrs = {'x': attr.label(providers = [P])})\next = module_extension(implementation = _impl, tag_classes = {'tag': tag})",
            "def _impl(ctx):\n    pass\ntag = tag_class(attrs = {'x': attr.label(executable = True)})\next = module_extension(implementation = _impl, tag_classes = {'tag': tag})",
            "def _impl(ctx):\n    pass\ntag = tag_class(attrs = {'x': attr.string(allow_empty = False)})\next = module_extension(implementation = _impl, tag_classes = {'tag': tag})",
            "def _impl(ctx):\n    pass\next = module_extension(implementation = _impl, facts_version = -1)",
        ];
        for source in cases {
            assert!(evaluate(source).is_err(), "unexpected success: {source}");
        }
    }

    #[test]
    fn export_lookup_distinguishes_missing_private_and_wrong_kind() {
        let module = evaluate(
            "def _impl(ctx):\n    pass\n_private = module_extension(implementation = _impl)\nwrong = 1\n",
        )
        .unwrap();
        assert!(module.get("missing").is_err());
        assert!(module.get("_private").is_err());
        assert!(
            module
                .get("wrong")
                .unwrap()
                .downcast::<FrozenModuleExtensionDefinition>()
                .is_err()
        );
    }
}
