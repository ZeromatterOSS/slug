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
use std::ops::Deref;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;
use slug_build_api_v2::ProviderIdentity;
use slug_build_api_v2::RunfilesPackageMetadata;
use slug_build_api_v2::RunfilesRepositoryMapping;
use slug_bzlmod_v2::BuiltinBazelToolsSnapshot;
use slug_bzlmod_v2::NonrootAttributeKey;
use slug_bzlmod_v2::NonrootAttributeValue;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::PackageIdentifier;
use slug_identity_v2::PackagePath;
use slug_starlark_v2::populate_universe;
use starlark::any::ProvidesStaticType;
use starlark::docs::DocItem;
use starlark::docs::DocMember;
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
use starlark::values::Tracer;
use starlark::values::UnpackValue;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark::values::dict::AllocDict;
use starlark::values::dict::DictRef;
use starlark::values::list::AllocList;
use starlark::values::list::ListRef;
use starlark::values::list::UnpackList;
use starlark::values::list_or_tuple::UnpackListOrTuple;
use starlark::values::none::NoneOr;
use starlark::values::none::NoneType;
use starlark::values::set::SetRef;
use starlark::values::starlark_value;
use starlark::values::tuple::TupleRef;
use starlark::values::typing::StarlarkCallable;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::attrs::AllowedAttributeValues;
use crate::attrs::AttributeDependencyConfiguration;
use crate::attrs::AttributeKind;
use crate::attrs::AttributePropertyFlag;
use crate::attrs::AttributePropertyFlags;
use crate::attrs::AttributeProvenance;
use crate::attrs::AttributeSchema;
use crate::attrs::AttributeValue;
use crate::attrs::CoercedAttributeValue;
use crate::attrs::FileAdmissibility;
use crate::attrs::NativeAttributeOrder;
use crate::attrs::NativeAttributePolicy;
use crate::attrs::NativeAttributeSchema;
use crate::attrs::NativeAttributeValue;
use crate::attrs::NativeRuleAttributes;
use crate::attrs::NativeRuleClass;
use crate::attrs::RuleClassAdmissibility;
use crate::attrs::TransitionDefinition as LoadingTransitionDefinition;
use crate::attrs::TransitionSetting;
use crate::bzl_module::BzlModuleIdentity;
use crate::bzl_module::FrozenBzlLifetimeEntry;
use crate::bzl_module::LoadingPrintCapture;
use crate::bzl_visibility::bzl_visibility_globals;
use crate::cc_common::cc_common_globals;
use crate::glob::GlobPattern;
use crate::glob::GlobSpec;
use crate::glob::PackageListing;
use crate::glob::expand_glob;
use crate::host_glob::HostGlobLoadingOperation;
use crate::host_glob::HostGlobLoadingRequest;
use crate::host_glob::HostGlobPrepared;
use crate::host_glob::HostGlobRequestTraversalError;
use crate::module_extension_repository_rule::RepositoryRuleAttribute;
use crate::module_extension_repository_rule::RepositoryRuleDefinition;
use crate::module_extension_repository_rule::RepositoryRuleInvocationState;
use crate::provider::AnalysisBuiltinCallable;
use crate::provider::BuiltinProviderKey;
use crate::provider::BzlEvaluationContext;
use crate::provider::OutputGroupInfo;
use crate::provider::RunEnvironmentInfo;
use crate::provider::UserProviderCallable;
use crate::provider::starlark_provider_identity;
use crate::provider::user_provider_from_arguments;
use crate::rule_outputs::PredeclaredOutput;
use crate::rule_outputs::RuleOutputsDefinitionGen;
use crate::rule_outputs::resolve_output_names;
use crate::starlark_label::StarlarkLabel;
use crate::starlark_label::label_globals;
use crate::starlark_label::resolve_label;
use crate::subrule::AttachedSubrules;
use crate::subrule::ConfigurationFieldValue;
use crate::subrule::ConfiguredDependencyAttribute;
use crate::subrule::LateBoundRuleAttribute;
use crate::subrule::SubruleAttribute;
use crate::subrule::SubruleAttributeDefault;
use crate::subrule::SubruleIdentity;
use crate::subrule::attached_subrules;
use crate::subrule::configuration_field_global;
use crate::subrule::fail_closed_rule_implementation;
use crate::subrule::subrule_global;
use crate::testing_bootstrap::testing_bootstrap_globals;
use crate::transition::TransitionSettingsKind;
use crate::transition::canonicalize_transition_settings;
use crate::transition::validate_transition_settings;
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
pub struct PackageEvaluation {
    pub package_dir: PathBuf,
    pub build_file: PathBuf,
    pub default_visibility: RuleVisibility,
    pub targets: Vec<PackageTarget>,
    /// Sparse RuleClass values keyed by their stable position in `targets`.
    pub native_attributes: Arc<[NativeTargetAttributes]>,
    /// Package-owned symbolic macro instances in stable declaration order.
    pub macro_instances: Arc<[MacroInstanceRecord]>,
    /// Sparse target-origin rows keyed by stable position in `targets`.
    pub macro_target_origins: Arc<[MacroTargetOrigin]>,
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

impl PartialEq for PackageEvaluation {
    fn eq(&self, other: &Self) -> bool {
        self.package_dir == other.package_dir
            && self.build_file == other.build_file
            && self.default_visibility == other.default_visibility
            && self.targets == other.targets
            && self.native_attributes == other.native_attributes
            && self.macro_instances == other.macro_instances
            && self.macro_target_origins == other.macro_target_origins
            && self.used_globs == other.used_globs
            && self.direct_load_roots == other.direct_load_roots
            && self.reachable_loads == other.reachable_loads
            && self.load_fingerprint == other.load_fingerprint
    }
}

impl Eq for PackageEvaluation {}

/// Host-prepared package result with complete selected repository metadata.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct LoadedPackage {
    evaluation: PackageEvaluation,
    runfiles_package: Arc<RunfilesPackageMetadata>,
}

/// Pre-Host evaluator result. This cannot enter complete-metadata consumers.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct LegacyLoadedPackage {
    evaluation: PackageEvaluation,
}

impl Deref for LoadedPackage {
    type Target = PackageEvaluation;

    fn deref(&self) -> &Self::Target {
        &self.evaluation
    }
}

impl Deref for LegacyLoadedPackage {
    type Target = PackageEvaluation;

    fn deref(&self) -> &Self::Target {
        &self.evaluation
    }
}

impl LoadedPackage {
    pub fn runfiles_package(&self) -> &Arc<RunfilesPackageMetadata> {
        &self.runfiles_package
    }
}

impl PackageEvaluation {
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

    pub fn macro_origin(&self, target: &str) -> Option<&MacroTargetOrigin> {
        let target_index = self
            .targets
            .iter()
            .position(|candidate| candidate.name == target)?;
        let target_index = u32::try_from(target_index).ok()?;
        self.macro_target_origins
            .binary_search_by_key(&target_index, |entry| entry.target_index)
            .ok()
            .map(|index| &self.macro_target_origins[index])
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct MacroDefinitionIdentity {
    pub defining_label: CanonicalLabel,
    pub exported_name: CompactString,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct MacroInstanceRecord {
    pub identity: CompactString,
    pub name: CompactString,
    pub same_name_depth: u32,
    pub parent: Option<u32>,
    pub definition: MacroDefinitionIdentity,
    pub visibility: RuleVisibility,
    pub generator_name: CompactString,
    pub generator_function: CompactString,
    pub generator_location: CompactString,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct MacroTargetOrigin {
    pub target_index: u32,
    pub macro_index: u32,
    pub definition_package: PackageIdentifier,
    pub namespace_violation: Option<CompactString>,
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
pub struct ConfigSettingAttribute<T> {
    value: T,
    provenance: AttributeProvenance,
}

impl<T> ConfigSettingAttribute<T> {
    fn from_optional(value: Option<T>, default: T) -> Self {
        match value {
            Some(value) => Self {
                value,
                provenance: AttributeProvenance::Explicit,
            },
            None => Self {
                value: default,
                provenance: AttributeProvenance::Default,
            },
        }
    }

    pub fn value(&self) -> &T {
        &self.value
    }

    pub fn provenance(&self) -> AttributeProvenance {
        self.provenance
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct ConfigSettingTarget {
    values: ConfigSettingAttribute<Arc<[(CompactString, CompactString)]>>,
    define_values: ConfigSettingAttribute<Arc<[(CompactString, CompactString)]>>,
    flag_values: ConfigSettingAttribute<Arc<[(CanonicalLabel, CompactString)]>>,
    constraint_values: ConfigSettingAttribute<Arc<[CanonicalLabel]>>,
}

impl ConfigSettingTarget {
    pub fn values(&self) -> &ConfigSettingAttribute<Arc<[(CompactString, CompactString)]>> {
        &self.values
    }

    pub fn define_values(&self) -> &ConfigSettingAttribute<Arc<[(CompactString, CompactString)]>> {
        &self.define_values
    }

    pub fn flag_values(&self) -> &ConfigSettingAttribute<Arc<[(CanonicalLabel, CompactString)]>> {
        &self.flag_values
    }

    pub fn constraint_values(&self) -> &ConfigSettingAttribute<Arc<[CanonicalLabel]>> {
        &self.constraint_values
    }

    pub fn semantic_references(&self) -> Vec<CanonicalLabel> {
        self.flag_values
            .value
            .iter()
            .map(|(label, _)| label.clone())
            .chain(self.constraint_values.value.iter().cloned())
            .collect()
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
    /// Loading-owned declaration of Bazel's `config_setting`. Configuration
    /// matching is intentionally owned by a later configured-analysis stage.
    ConfigSetting {
        declaration: ConfigSettingTarget,
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

/// A typed declaration value whose Bazel-visible default/explicit provenance
/// participates in loading equality and invalidation.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct NativeToolchainAttribute<T> {
    value: T,
    provenance: AttributeProvenance,
}

impl<T> NativeToolchainAttribute<T> {
    fn from_optional(value: Option<T>, default: T) -> Self {
        match value {
            Some(value) => Self {
                value,
                provenance: AttributeProvenance::Explicit,
            },
            None => Self {
                value: default,
                provenance: AttributeProvenance::Default,
            },
        }
    }

    pub fn value(&self) -> &T {
        &self.value
    }

    pub fn provenance(&self) -> AttributeProvenance {
        self.provenance
    }
}

/// Loading-owned representation of the exact Bazel 9.2 native declaration
/// inputs needed by toolchain analysis. Configured selection remains owned by
/// later packets; the current Slug-native marker resolver fails closed when a
/// retained input is outside its admitted default-only surface.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum NativeToolchainTarget {
    ConstraintSetting {
        default_constraint_value: Option<CanonicalLabel>,
    },
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
        exec_compatible_with: NativeToolchainAttribute<Arc<[CanonicalLabel]>>,
        target_compatible_with: NativeToolchainAttribute<Arc<[CanonicalLabel]>>,
        use_target_platform_constraints: NativeToolchainAttribute<bool>,
        target_settings: NativeToolchainAttribute<CoercedAttributeValue>,
    },
}

impl NativeToolchainTarget {
    pub fn rule_class(&self) -> &'static str {
        match self {
            Self::ConstraintSetting { .. } => "constraint_setting",
            Self::ConstraintValue { .. } => "constraint_value",
            Self::Platform { .. } => "platform",
            Self::ToolchainType => "toolchain_type",
            Self::Toolchain { .. } => "toolchain",
        }
    }

    fn rule_capability(&self) -> &'static RuleCapability {
        match self {
            Self::ConstraintSetting { .. } => &CONSTRAINT_SETTING_RULE_CAPABILITY,
            Self::ConstraintValue { .. } => &CONSTRAINT_VALUE_RULE_CAPABILITY,
            Self::Platform { .. } => &PLATFORM_RULE_CAPABILITY,
            Self::ToolchainType => &TOOLCHAIN_TYPE_RULE_CAPABILITY,
            Self::Toolchain { .. } => &TOOLCHAIN_RULE_CAPABILITY,
        }
    }

    pub fn semantic_references(&self) -> Vec<CanonicalLabel> {
        match self {
            Self::ConstraintSetting {
                default_constraint_value,
            } => default_constraint_value.iter().cloned().collect(),
            Self::ToolchainType => Vec::new(),
            Self::ConstraintValue { constraint_setting } => vec![constraint_setting.clone()],
            Self::Platform { constraint_values } => constraint_values.to_vec(),
            Self::Toolchain {
                toolchain_type,
                implementation,
                exec_compatible_with,
                target_compatible_with,
                target_settings,
                ..
            } => {
                let mut references = Vec::new();
                references.push(toolchain_type.clone());
                references.push(implementation.clone());
                references.extend(exec_compatible_with.value().iter().cloned());
                references.extend(target_compatible_with.value().iter().cloned());
                target_settings.value().labels(&mut references);
                references.extend(selector_key_labels(target_settings.value()));
                references
            }
        }
    }
}

/// The frozen rule implementation retained for configured-target analysis.
/// The containing package keeps its source `.bzl` module alive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative)]
pub enum BuildSettingDefinition {
    Integer { flag: bool },
    String { flag: bool, allow_multiple: bool },
    Boolean { flag: bool },
    StringList { flag: bool, repeatable: bool },
    StringSet { flag: bool, repeatable: bool },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum BuildSettingDefault {
    Integer(i32),
    Boolean(bool),
    String(CompactString),
    StringList(Arc<[CompactString]>),
    StringSet(Arc<[CompactString]>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative)]
pub enum BuildSettingScope {
    Default,
    Universal,
    Target,
    Project,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct BuildSettingDeclaration {
    definition: BuildSettingDefinition,
    default: BuildSettingDefault,
    scope: BuildSettingScope,
}

impl BuildSettingDeclaration {
    pub fn definition(&self) -> BuildSettingDefinition {
        self.definition
    }

    pub fn default(&self) -> &BuildSettingDefault {
        &self.default
    }

    pub fn scope(&self) -> BuildSettingScope {
        self.scope
    }
}

/// One rule/aspect toolchain type requirement detached from its Starlark value.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct ToolchainTypeRequirement {
    label: CanonicalLabel,
    mandatory: bool,
}

impl ToolchainTypeRequirement {
    pub fn new(label: CanonicalLabel, mandatory: bool) -> Self {
        Self { label, mandatory }
    }

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

impl BuildSettingDefinition {
    fn attribute_kind(self) -> AttributeKind {
        match self {
            Self::Integer { .. } => AttributeKind::Integer,
            Self::String { .. } => AttributeKind::String,
            Self::Boolean { .. } => AttributeKind::Boolean,
            Self::StringList { .. } | Self::StringSet { .. } => AttributeKind::StringList,
        }
    }
}

#[derive(Debug, Clone, Allocative)]
pub struct StarlarkRuleImplementation {
    #[allocative(skip)]
    implementation: FrozenValue,
    definition_source: Arc<BzlModuleIdentity>,
    source_identities_by_filename: Arc<[(CompactString, BzlModuleIdentity)]>,
    dependencies: Arc<[CanonicalLabel]>,
    required_toolchains: Arc<[ToolchainTypeRequirement]>,
    advertised_providers: Arc<[ProviderIdentity]>,
    required_fragments: Arc<[CompactString]>,
    attached_subrules: AttachedSubrules,
    #[allocative(skip)]
    subrule_callables: Arc<[FrozenValue]>,
    late_bound_attributes: Arc<[LateBoundRuleAttribute]>,
    schema: Arc<[AttributeSchema]>,
    values: Arc<[AttributeValue]>,
    capability: Arc<RuleCapability>,
    build_setting_definition: Option<BuildSettingDefinition>,
    incoming_transition: Option<LoadingTransitionDefinition>,
    pub predeclared_outputs: Arc<[PredeclaredOutput]>,
    pub output_to_genfiles: bool,
}

impl PartialEq for StarlarkRuleImplementation {
    fn eq(&self, other: &Self) -> bool {
        // Frozen callable addresses are retained for Stage 6 lifetime only.
        // The semantic attachment projection below owns package equality.
        self.dependencies == other.dependencies
            && self.definition_source == other.definition_source
            && self.source_identities_by_filename == other.source_identities_by_filename
            && self.required_toolchains == other.required_toolchains
            && self.advertised_providers == other.advertised_providers
            && self.required_fragments == other.required_fragments
            && self.attached_subrules == other.attached_subrules
            && self.late_bound_attributes == other.late_bound_attributes
            && self.schema == other.schema
            && self.values == other.values
            && self.capability == other.capability
            && self.build_setting_definition == other.build_setting_definition
            && self.incoming_transition == other.incoming_transition
            && self.predeclared_outputs == other.predeclared_outputs
            && self.output_to_genfiles == other.output_to_genfiles
    }
}

impl Eq for StarlarkRuleImplementation {}

impl StarlarkRuleImplementation {
    pub fn frozen_value(&self) -> FrozenValue {
        self.implementation
    }

    pub fn definition_source(&self) -> &Arc<BzlModuleIdentity> {
        &self.definition_source
    }

    #[doc(hidden)]
    pub fn source_identities_by_filename(&self) -> &Arc<[(CompactString, BzlModuleIdentity)]> {
        &self.source_identities_by_filename
    }

    pub fn dependencies(&self) -> &[CanonicalLabel] {
        &self.dependencies
    }

    /// Toolchain-type requirements declared by the defining `rule()` call.
    /// These are loading-only retained metadata, not ordinary dependencies.
    pub fn required_toolchains(&self) -> &[ToolchainTypeRequirement] {
        &self.required_toolchains
    }

    pub fn advertised_providers(&self) -> &[ProviderIdentity] {
        &self.advertised_providers
    }

    pub fn required_fragments(&self) -> &[CompactString] {
        &self.required_fragments
    }

    pub fn attached_subrule_count(&self) -> usize {
        self.attached_subrules.definition_count()
    }

    pub fn subrule_hidden_attribute_names(&self) -> impl Iterator<Item = &str> {
        self.attached_subrules.hidden_attribute_names()
    }

    pub fn subrule_definition_names(&self) -> impl Iterator<Item = &str> {
        self.attached_subrules
            .definitions
            .iter()
            .map(|definition| definition.identity.exported_name.as_str())
    }

    pub fn direct_subrule_names(&self) -> impl Iterator<Item = &str> {
        self.attached_subrules
            .direct
            .iter()
            .map(|identity| identity.exported_name.as_str())
    }

    #[doc(hidden)]
    pub fn direct_subrule_identities(&self) -> Arc<[Arc<SubruleIdentity>]> {
        self.attached_subrules.direct.clone()
    }

    #[doc(hidden)]
    pub fn subrule_invocations(
        &self,
    ) -> impl Iterator<
        Item = (
            Arc<SubruleIdentity>,
            Arc<[Arc<SubruleIdentity>]>,
            FrozenValue,
            Arc<SmallSet<CompactString>>,
        ),
    > + '_ {
        self.attached_subrules
            .definitions
            .iter()
            .zip(self.subrule_callables.iter().copied())
            .map(|(definition, callable)| {
                (
                    definition.identity.clone(),
                    definition.direct_subrules.clone(),
                    callable,
                    definition.fragments.clone(),
                )
            })
    }

    #[doc(hidden)]
    pub fn subrule_identity_attribute_spans(
        &self,
    ) -> impl Iterator<Item = (Arc<SubruleIdentity>, u32, u32)> + '_ {
        self.attached_subrules
            .spans
            .iter()
            .map(|span| (span.owner.clone(), span.start, span.len))
    }

    pub fn subrule_callables(
        &self,
    ) -> impl Iterator<Item = (&CanonicalLabel, &str, FrozenValue)> + '_ {
        self.attached_subrules
            .definitions
            .iter()
            .zip(self.subrule_callables.iter().copied())
            .map(|(definition, callable)| {
                (
                    &definition.identity.defining_label,
                    definition.identity.exported_name.as_str(),
                    callable,
                )
            })
    }

    pub fn subrule_attribute_spans(&self) -> impl Iterator<Item = (&str, u32, u32)> {
        self.attached_subrules
            .spans
            .iter()
            .map(|span| (span.owner.exported_name.as_str(), span.start, span.len))
    }

    pub fn subrule_fragments(&self) -> impl Iterator<Item = &str> {
        self.attached_subrules
            .definitions
            .iter()
            .flat_map(|definition| definition.fragments.iter().map(CompactString::as_str))
    }

    pub fn late_bound_rule_attributes(&self) -> impl Iterator<Item = (&str, &str)> {
        self.late_bound_attributes.iter().map(|attribute| {
            let schema = &self.schema[attribute.schema_index as usize];
            (
                schema.declaration_name(),
                attribute.identity.field().field_name(),
            )
        })
    }

    pub fn configured_dependency_attributes(
        &self,
    ) -> impl Iterator<Item = ConfiguredDependencyAttribute<'_>> {
        self.late_bound_attributes
            .iter()
            .map(|attribute| {
                ConfiguredDependencyAttribute::from_ordinary(
                    attribute,
                    &self.schema[attribute.schema_index as usize],
                )
            })
            .chain(
                self.attached_subrules
                    .lifted_attributes
                    .iter()
                    .map(ConfiguredDependencyAttribute::from_hidden),
            )
    }

    pub fn schema(&self) -> &[AttributeSchema] {
        &self.schema
    }

    pub fn values(&self) -> &[AttributeValue] {
        &self.values
    }

    pub fn build_setting_definition(&self) -> Option<BuildSettingDefinition> {
        self.build_setting_definition
    }

    pub fn incoming_transition(&self) -> Option<&LoadingTransitionDefinition> {
        self.incoming_transition.as_ref()
    }

    pub fn build_setting_declaration(&self) -> anyhow::Result<Option<BuildSettingDeclaration>> {
        let Some(definition) = self.build_setting_definition else {
            return Ok(None);
        };
        let value = self
            .value("build_setting_default")
            .expect("build setting schema has a mandatory default")
            .value
            .as_ref();
        let default = match (definition, value) {
            (BuildSettingDefinition::Integer { .. }, CoercedAttributeValue::Integer(value)) => {
                BuildSettingDefault::Integer(*value)
            }
            (BuildSettingDefinition::Boolean { .. }, CoercedAttributeValue::Boolean(value)) => {
                BuildSettingDefault::Boolean(*value)
            }
            (BuildSettingDefinition::String { .. }, CoercedAttributeValue::String(value)) => {
                BuildSettingDefault::String(value.clone())
            }
            (
                BuildSettingDefinition::StringList { .. },
                CoercedAttributeValue::StringList(value),
            ) => BuildSettingDefault::StringList(value.clone()),
            (
                BuildSettingDefinition::StringSet { .. },
                CoercedAttributeValue::StringList(value),
            ) => BuildSettingDefault::StringSet(value.clone()),
            _ => anyhow::bail!("build setting default does not match its definition"),
        };
        let scope = match self.value("scope") {
            Some(value) if value.provenance == AttributeProvenance::Explicit => {
                let CoercedAttributeValue::String(scope) = value.value.as_ref() else {
                    anyhow::bail!("explicit build setting scope must be a nonconfigurable string")
                };
                if scope.eq_ignore_ascii_case("universal") {
                    BuildSettingScope::Universal
                } else if scope.eq_ignore_ascii_case("target") {
                    BuildSettingScope::Target
                } else if scope.eq_ignore_ascii_case("project") {
                    BuildSettingScope::Project
                } else {
                    anyhow::bail!(
                        "invalid build setting scope `{scope}`; expected universal, target, or project"
                    )
                }
            }
            _ => BuildSettingScope::Default,
        };
        Ok(Some(BuildSettingDeclaration {
            definition,
            default,
            scope,
        }))
    }

    pub fn is_root_string_build_setting(&self) -> bool {
        self.build_setting_definition
            == Some(BuildSettingDefinition::String {
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
    macro_instances: Vec<MacroInstanceRecord>,
    active_macro: Option<usize>,
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
            macro_instances: Vec::new(),
            active_macro: None,
            used_globs: Vec::new(),
        }
    }
}

#[derive(Debug)]
struct RecordedTarget {
    kind: PackageTargetKind,
    visibility: VisibilitySource,
    native_overrides: Vec<NativeAttributeOverride>,
    macro_origin: Option<(usize, PackageIdentifier, Option<CompactString>)>,
}

#[derive(ProvidesStaticType)]
pub(crate) struct MacroEvaluationContext<'a> {
    recorder: &'a PackageRecorder,
    bzl: BzlEvaluationContext,
}

impl<'a> MacroEvaluationContext<'a> {
    pub(crate) fn recorder(&self) -> &'a PackageRecorder {
        self.recorder
    }

    pub(crate) fn bzl(&self) -> &BzlEvaluationContext {
        &self.bzl
    }
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

#[derive(Debug)]
enum PackageRecorderRepositoryMapping {
    Legacy(Arc<[(ApparentRepoName, CanonicalRepoName)]>),
    Complete(Arc<RunfilesRepositoryMapping>),
}

impl PackageRecorderRepositoryMapping {
    fn entries(&self) -> &[(ApparentRepoName, CanonicalRepoName)] {
        match self {
            Self::Legacy(entries) => entries,
            Self::Complete(mapping) => mapping.entries(),
        }
    }
}

#[derive(Debug, ProvidesStaticType)]
pub(crate) struct PackageRecorder {
    glob_source: PackageGlobSource,
    package: CompactString,
    package_identifier: PackageIdentifier,
    repository_mapping: PackageRecorderRepositoryMapping,
    print_capture: Option<Rc<LoadingPrintCapture>>,
    state: RefCell<PackageState>,
}

#[allow(dead_code)] // The Host attempt methods are exercised privately before activation.
impl PackageRecorder {
    pub(crate) fn new(listing: PackageListing, package: impl Into<CompactString>) -> Self {
        let package = package.into();
        Self {
            glob_source: PackageGlobSource::Listing(listing),
            package_identifier: PackageIdentifier::new(
                CanonicalRepoName::root(),
                PackagePath::parse(&package).expect("validated BUILD package path"),
            ),
            package,
            repository_mapping: PackageRecorderRepositoryMapping::Legacy(Arc::from([])),
            print_capture: None,
            state: RefCell::new(PackageState::default()),
        }
    }

    pub(crate) fn new_host(
        prepared: Arc<SmallMap<HostGlobLoadingRequest, HostGlobPrepared>>,
        package_identifier: PackageIdentifier,
        repository_mapping: Arc<RunfilesRepositoryMapping>,
    ) -> Self {
        let package = CompactString::new(package_identifier.package().as_str());
        Self {
            glob_source: PackageGlobSource::Host(HostGlobAttemptState {
                prepared,
                control: RefCell::new(None),
            }),
            package,
            package_identifier,
            repository_mapping: PackageRecorderRepositoryMapping::Complete(repository_mapping),
            print_capture: None,
            state: RefCell::new(PackageState::default()),
        }
    }

    pub(crate) fn with_print_capture(
        mut self,
        print_capture: Option<Rc<LoadingPrintCapture>>,
    ) -> Self {
        self.print_capture = print_capture;
        self
    }

    fn print_capture(&self) -> Option<&LoadingPrintCapture> {
        self.print_capture.as_deref()
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
            .and_then(|extra| {
                extra.downcast_ref::<Self>().or_else(|| {
                    extra
                        .downcast_ref::<MacroEvaluationContext<'_>>()
                        .map(MacroEvaluationContext::recorder)
                })
            })
            .ok_or_else(|| anyhow::anyhow!("Bazel package global invoked without package state"))
    }

    fn reject_macro_operation(&self, operation: &str) -> anyhow::Result<()> {
        if self.state.borrow().active_macro.is_some() {
            anyhow::bail!("{operation} may not be called from a symbolic macro");
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn has_active_macro_for_test(&self) -> bool {
        self.state.borrow().active_macro.is_some()
    }

    fn for_glob<'a>(eval: &'a Evaluator<'_, '_, '_>) -> anyhow::Result<&'a Self> {
        eval.extra
            .and_then(|extra| extra.downcast_ref::<Self>())
            .ok_or_else(|| {
                anyhow::anyhow!("glob() may only be called while evaluating a BUILD package")
            })
    }

    fn set_default_visibility(&self, visibility: Vec<VisibilityArgument>) -> anyhow::Result<()> {
        self.state.borrow_mut().default_visibility = self.parse_visibility(visibility)?;
        Ok(())
    }

    fn set_package_defaults(
        &self,
        visibility: Option<Vec<VisibilityArgument>>,
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
        visibility: Option<Vec<VisibilityArgument>>,
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
        visibility: Option<Vec<VisibilityArgument>>,
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
        visibility: Option<Vec<VisibilityArgument>>,
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
        visibility: Option<Vec<VisibilityArgument>>,
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
        values: Option<SmallMap<String, String>>,
        define_values: Option<SmallMap<String, String>>,
        flag_values: Option<SmallMap<String, String>>,
        constraint_values: Option<Vec<String>>,
        visibility: Option<Vec<VisibilityArgument>>,
    ) -> anyhow::Result<()> {
        let normalize_strings = |values: Option<SmallMap<String, String>>| {
            values.map(|values| {
                let mut values = values
                    .into_iter()
                    .map(|(key, value)| (CompactString::from(key), CompactString::from(value)))
                    .collect::<Vec<_>>();
                values.sort_unstable();
                Arc::from(values)
            })
        };
        let values = normalize_strings(values);
        let define_values = normalize_strings(define_values);
        let flag_values = flag_values
            .map(|values| {
                let mut result = Vec::with_capacity(values.len());
                for (label, value) in values {
                    let label = self.dependency_label(&label)?;
                    if result
                        .iter()
                        .any(|(existing, _): &(CanonicalLabel, CompactString)| {
                            existing.bazel_natural_cmp(&label).is_eq()
                        })
                    {
                        anyhow::bail!("duplicate canonical label `{label}` in flag_values")
                    }
                    result.push((label, CompactString::from(value)));
                }
                result.sort_by(|(left, _), (right, _)| {
                    CanonicalLabel::bazel_natural_cmp(left, right)
                });
                Ok::<Arc<[(CanonicalLabel, CompactString)]>, anyhow::Error>(result.into())
            })
            .transpose()?;
        let constraint_values = constraint_values
            .map(|values| {
                values
                    .iter()
                    .map(|value| self.dependency_label(value))
                    .collect::<anyhow::Result<Vec<_>>>()
                    .map(Arc::from)
            })
            .transpose()?;
        self.record_target(
            name,
            PackageTargetKind::ConfigSetting {
                declaration: ConfigSettingTarget {
                    values: ConfigSettingAttribute::from_optional(values, Arc::from([])),
                    define_values: ConfigSettingAttribute::from_optional(
                        define_values,
                        Arc::from([]),
                    ),
                    flag_values: ConfigSettingAttribute::from_optional(flag_values, Arc::from([])),
                    constraint_values: ConfigSettingAttribute::from_optional(
                        constraint_values,
                        Arc::from([]),
                    ),
                },
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
        visibility: Option<Vec<VisibilityArgument>>,
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

    fn native_toolchain_declaration<'v>(
        &self,
        toolchain: &str,
        toolchain_type: &str,
        exec_compatible_with: Option<UnpackList<&str>>,
        target_compatible_with: Option<UnpackList<&str>>,
        use_target_platform_constraints: Option<bool>,
        target_settings: Option<Value<'v>>,
    ) -> anyhow::Result<NativeToolchainTarget> {
        let exec_compatible_with = exec_compatible_with
            .map(|values| self.native_toolchain_labels(&values.items))
            .transpose()?;
        let target_compatible_with = target_compatible_with
            .map(|values| self.native_toolchain_labels(&values.items))
            .transpose()?;
        let target_settings = target_settings
            .map(|value| {
                coerce_starlark_value(
                    self,
                    AttributeKind::LabelList,
                    "target_settings",
                    true,
                    value,
                )
            })
            .transpose()?;
        Ok(NativeToolchainTarget::Toolchain {
            toolchain_type: self.native_toolchain_label(toolchain_type)?,
            implementation: self.native_toolchain_label(toolchain)?,
            exec_compatible_with: NativeToolchainAttribute::from_optional(
                exec_compatible_with,
                Arc::from([]),
            ),
            target_compatible_with: NativeToolchainAttribute::from_optional(
                target_compatible_with,
                Arc::from([]),
            ),
            use_target_platform_constraints: NativeToolchainAttribute::from_optional(
                use_target_platform_constraints,
                false,
            ),
            target_settings: NativeToolchainAttribute::from_optional(
                target_settings,
                empty_labels(),
            ),
        })
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
        definition_source: Arc<BzlModuleIdentity>,
        source_identities_by_filename: Arc<[(CompactString, BzlModuleIdentity)]>,
        required_toolchains: Arc<[ToolchainTypeRequirement]>,
        advertised_providers: Arc<[ProviderIdentity]>,
        required_fragments: Arc<[CompactString]>,
        attached_subrules: AttachedSubrules,
        subrule_callables: Arc<[FrozenValue]>,
        late_bound_attributes: Arc<[LateBoundRuleAttribute]>,
        capability: Arc<RuleCapability>,
        schema: Arc<[AttributeSchema]>,
        values: Arc<[AttributeValue]>,
        build_setting_definition: Option<BuildSettingDefinition>,
        incoming_transition: Option<LoadingTransitionDefinition>,
        predeclared_outputs: Arc<[PredeclaredOutput]>,
        output_to_genfiles: bool,
        visibility: Option<RuleVisibility>,
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
                definition_source,
                source_identities_by_filename,
                dependencies: dependencies.into(),
                required_toolchains,
                advertised_providers,
                required_fragments,
                attached_subrules,
                subrule_callables,
                late_bound_attributes,
                schema,
                values,
                capability,
                build_setting_definition,
                incoming_transition,
                predeclared_outputs,
                output_to_genfiles,
            }),
            visibility.map_or(VisibilitySource::PackageDefault, VisibilitySource::Declared),
        )
    }

    fn dependency_label(&self, value: &str) -> anyhow::Result<CanonicalLabel> {
        package_context_label_with_repository(
            &self.package_identifier,
            self.repository_mapping.entries(),
            value,
        )
    }

    fn output_label(&self, value: &str) -> anyhow::Result<CanonicalLabel> {
        let label = if self.package_identifier.repo().is_root()
            && matches!(&self.glob_source, PackageGlobSource::Listing(_))
        {
            package_context_label(&self.package, value)
        } else if value.starts_with('@') && !value.contains("//") {
            Err(anyhow::anyhow!("repository shorthand output"))
        } else {
            let rewritten = value.strip_prefix("@//").map(|rest| format!("@@//{rest}"));
            package_context_label_with_repository(
                &self.package_identifier,
                self.repository_mapping.entries(),
                rewritten.as_deref().unwrap_or(value),
            )
        }
        .map_err(|_| {
            anyhow::anyhow!("output label must name a valid target in this package: {value}")
        })?;
        if label.package() != &self.package_identifier {
            anyhow::bail!("output label must name a valid target in this package: {value}");
        }
        Ok(label)
    }

    fn parse_visibility(&self, values: Vec<VisibilityArgument>) -> anyhow::Result<RuleVisibility> {
        RuleVisibility::from_declared_labels(
            values
                .iter()
                .map(|value| match value {
                    VisibilityArgument::Raw(value) => self.dependency_label(value),
                    VisibilityArgument::Canonical(value) => Ok(value.clone()),
                })
                .collect::<anyhow::Result<Vec<_>>>()?,
        )
    }

    fn visibility_source(
        &self,
        values: Option<Vec<VisibilityArgument>>,
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
        if let Some(macro_index) = state
            .macro_instances
            .iter()
            .rposition(|instance| instance.name == name)
            && state.active_macro != Some(macro_index)
        {
            anyhow::bail!(
                "target '{name}' conflicts with an existing macro (and was not created by it)"
            );
        }
        let macro_origin = state.active_macro.map(|macro_index| {
            let macro_instance = &state.macro_instances[macro_index];
            let violation = (!name_is_within_macro_namespace(&name, &macro_instance.name))
                .then(|| macro_instance.name.clone());
            (
                macro_index,
                macro_instance.definition.defining_label.package().clone(),
                violation,
            )
        });
        let visibility = match (&visibility, &macro_origin) {
            (VisibilitySource::PackageDefault, Some((_, definition_package, _))) => {
                VisibilitySource::Declared(concat_visibility_with_package(
                    &RuleVisibility::Private,
                    definition_package,
                )?)
            }
            _ => visibility,
        };
        state.targets.insert(
            name,
            RecordedTarget {
                kind,
                visibility,
                native_overrides: Vec::new(),
                macro_origin,
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
        let mut include_matched = Vec::with_capacity(spec.includes().len());
        let mut matches = Vec::new();
        for pattern in spec.includes() {
            let paths = self.host_glob_request(host, pattern.dupe(), operation)?;
            include_matched.push(!paths.is_empty());
            matches.extend(paths);
        }
        spec.check_include_matches(&include_matched)?;
        spec.validate_excludes()?;
        matches.retain(|path| !spec.is_excluded(path));
        spec.check_final_matches(matches.is_empty())?;

        let mut projected = matches
            .iter()
            .map(|path| {
                let value = match std::str::from_utf8(path) {
                    Ok(value) => value,
                    Err(_) => {
                        return self.transfer_host_glob(
                            host,
                            HostGlobAttemptControl::Terminal(
                                HostGlobAttemptError::UnsupportedPath { path: path.dupe() },
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
            .collect::<anyhow::Result<Vec<_>>>()?;
        projected.sort_by(|left, right| left.encode_utf16().cmp(right.encode_utf16()));
        projected.dedup();
        Ok(projected)
    }

    fn host_glob_request(
        &self,
        host: &HostGlobAttemptState,
        pattern: GlobPattern,
        operation: HostGlobLoadingOperation,
    ) -> anyhow::Result<Vec<Arc<[u8]>>> {
        let request = HostGlobLoadingRequest::new(pattern, operation);
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
        Ok(matches.paths().iter().map(Dupe::dupe).collect())
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
        let (evaluation, package_identifier, repository_mapping) = self.finish_evaluation(
            package_dir,
            build_file,
            direct_load_roots,
            reachable_loads,
            load_fingerprint,
            retained_bzl_modules,
        );
        let PackageRecorderRepositoryMapping::Complete(repository_mapping) = repository_mapping
        else {
            panic!("legacy PackageRecorder must use finish_legacy")
        };
        LoadedPackage {
            evaluation,
            runfiles_package: Arc::new(RunfilesPackageMetadata::new(
                package_identifier,
                repository_mapping,
            )),
        }
    }

    pub(crate) fn finish_legacy(
        self,
        package_dir: PathBuf,
        build_file: PathBuf,
        direct_load_roots: Arc<[BzlModuleIdentity]>,
        reachable_loads: Arc<[BzlModuleIdentity]>,
        load_fingerprint: [u8; 32],
        retained_bzl_modules: Arc<[FrozenBzlLifetimeEntry]>,
    ) -> LegacyLoadedPackage {
        let (evaluation, _, repository_mapping) = self.finish_evaluation(
            package_dir,
            build_file,
            direct_load_roots,
            reachable_loads,
            load_fingerprint,
            retained_bzl_modules,
        );
        let PackageRecorderRepositoryMapping::Legacy(_) = repository_mapping else {
            panic!("Host PackageRecorder must use finish")
        };
        LegacyLoadedPackage { evaluation }
    }

    fn finish_evaluation(
        self,
        package_dir: PathBuf,
        build_file: PathBuf,
        direct_load_roots: Arc<[BzlModuleIdentity]>,
        reachable_loads: Arc<[BzlModuleIdentity]>,
        load_fingerprint: [u8; 32],
        retained_bzl_modules: Arc<[FrozenBzlLifetimeEntry]>,
    ) -> (
        PackageEvaluation,
        PackageIdentifier,
        PackageRecorderRepositoryMapping,
    ) {
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
                            package_context_label_with_repository(
                                &self.package_identifier,
                                self.repository_mapping.entries(),
                                name,
                            )
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
        let macro_target_origins = state
            .targets
            .iter()
            .enumerate()
            .filter_map(|(target_index, (_, target))| {
                target.macro_origin.as_ref().map(
                    |(macro_index, definition_package, namespace_violation)| MacroTargetOrigin {
                        target_index: u32::try_from(target_index)
                            .expect("package target count exceeds u32"),
                        macro_index: u32::try_from(*macro_index)
                            .expect("package macro count exceeds u32"),
                        definition_package: definition_package.clone(),
                        namespace_violation: namespace_violation.clone(),
                    },
                )
            })
            .collect::<Vec<_>>()
            .into();
        let evaluation = PackageEvaluation {
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
            macro_instances: state.macro_instances.into(),
            macro_target_origins,
            used_globs: state.used_globs,
            direct_load_roots,
            reachable_loads,
            load_fingerprint,
            retained_bzl_modules,
        };
        (evaluation, self.package_identifier, self.repository_mapping)
    }
}

fn name_is_within_macro_namespace(name: &str, macro_name: &str) -> bool {
    name == macro_name
        || name.strip_prefix(macro_name).is_some_and(|suffix| {
            suffix.len() >= 2
                && matches!(
                    suffix.as_bytes().first(),
                    Some(b'_') | Some(b'-') | Some(b'.')
                )
        })
}

fn concat_visibility_with_package(
    visibility: &RuleVisibility,
    package: &PackageIdentifier,
) -> anyhow::Result<RuleVisibility> {
    if visibility.is_public() {
        return Ok(RuleVisibility::Public);
    }
    let package_label =
        CanonicalLabel::parse(&format!("{package}:__pkg__")).map_err(anyhow::Error::msg)?;
    let labels = visibility
        .raw_declared_labels()
        .iter()
        .cloned()
        .chain(std::iter::once(package_label));
    RuleVisibility::from_declared_labels(labels)
}

fn native_rule_class(kind: &PackageTargetKind) -> Option<NativeRuleClass> {
    Some(match kind {
        PackageTargetKind::Filegroup { .. } => NativeRuleClass::Filegroup,
        PackageTargetKind::Alias { .. } => NativeRuleClass::Alias,
        PackageTargetKind::ConfigSetting { .. } => NativeRuleClass::ConfigSetting,
        PackageTargetKind::TestSuite { .. } => NativeRuleClass::TestSuite,
        PackageTargetKind::NativeToolchain(native) => match native {
            NativeToolchainTarget::ConstraintSetting { .. } => NativeRuleClass::ConstraintSetting,
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
        AttributeKind::IntegerList => CoercedAttributeValue::IntegerList(Arc::from([])),
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
            declaration: setting,
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
                setting.values.provenance,
                CoercedAttributeValue::StringDict(setting.values.value.clone()),
            );
            set_native_value(
                class,
                &mut values,
                "define_values",
                setting.define_values.provenance,
                CoercedAttributeValue::StringDict(setting.define_values.value.clone()),
            );
            set_native_value(
                class,
                &mut values,
                "flag_values",
                setting.flag_values.provenance,
                CoercedAttributeValue::LabelKeyedStringDict(setting.flag_values.value.clone()),
            );
            set_native_value(
                class,
                &mut values,
                "constraint_values",
                setting.constraint_values.provenance,
                CoercedAttributeValue::LabelList(setting.constraint_values.value.clone()),
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
                NativeToolchainTarget::ConstraintSetting {
                    default_constraint_value,
                } => set_native_value(
                    class,
                    &mut values,
                    "default_constraint_value",
                    if default_constraint_value.is_some() {
                        AttributeProvenance::Explicit
                    } else {
                        AttributeProvenance::Default
                    },
                    default_constraint_value
                        .as_ref()
                        .map_or(CoercedAttributeValue::None, |label| {
                            CoercedAttributeValue::Label(label.clone())
                        }),
                ),
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
                    target_compatible_with,
                    use_target_platform_constraints,
                    target_settings,
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
                        exec_compatible_with.provenance(),
                        CoercedAttributeValue::LabelList(exec_compatible_with.value().clone()),
                    );
                    set_native_value(
                        class,
                        &mut values,
                        "target_compatible_with",
                        target_compatible_with.provenance(),
                        CoercedAttributeValue::LabelList(target_compatible_with.value().clone()),
                    );
                    set_native_value(
                        class,
                        &mut values,
                        "use_target_platform_constraints",
                        use_target_platform_constraints.provenance(),
                        CoercedAttributeValue::Boolean(*use_target_platform_constraints.value()),
                    );
                    set_native_value(
                        class,
                        &mut values,
                        "target_settings",
                        target_settings.provenance(),
                        target_settings.value().clone(),
                    );
                    let config_dependencies = selector_key_labels(target_settings.value());
                    if !config_dependencies.is_empty() {
                        set_native_value(
                            class,
                            &mut values,
                            "$config_dependencies",
                            AttributeProvenance::Implicit,
                            CoercedAttributeValue::LabelList(config_dependencies.into()),
                        );
                    }
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

struct UnpackVisibility {
    items: Vec<VisibilityArgument>,
}

enum VisibilityArgument {
    Raw(String),
    Canonical(CanonicalLabel),
}

impl starlark::values::type_repr::StarlarkTypeRepr for UnpackVisibility {
    type Canonical = <UnpackListOrTuple<Value<'static>> as starlark::values::type_repr::StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> starlark::typing::Ty {
        UnpackListOrTuple::<Value<'static>>::starlark_type_repr()
    }
}

impl<'v> UnpackValue<'v> for UnpackVisibility {
    type Error = starlark::Error;

    fn unpack_value_impl(value: Value<'v>) -> starlark::Result<Option<Self>> {
        let Some(values) = UnpackListOrTuple::<Value<'v>>::unpack_value(value)? else {
            return Ok(None);
        };
        let items = values
            .items
            .into_iter()
            .map(|value| {
                if let Some(value) = value.unpack_str() {
                    Ok(VisibilityArgument::Raw(value.to_owned()))
                } else if let Some(label) = StarlarkLabel::from_value(value) {
                    Ok(VisibilityArgument::Canonical(label.canonical().clone()))
                } else {
                    Err(starlark::Error::new_other(anyhow::anyhow!(
                        "visibility must contain strings or Labels"
                    )))
                }
            })
            .collect::<starlark::Result<Vec<_>>>()?;
        Ok(Some(Self { items }))
    }
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

pub(crate) fn package_context_label_with_repository(
    package: &PackageIdentifier,
    mapping: &[(ApparentRepoName, CanonicalRepoName)],
    raw: &str,
) -> anyhow::Result<CanonicalLabel> {
    CanonicalLabel::parse_with_package_context(raw, package, |requested| {
        let mut matches = mapping
            .iter()
            .filter(|(name, _)| name.as_str() == requested);
        let repository = matches
            .next()
            .ok_or_else(|| format!("no repository visible as '@{requested}'"))?;
        if matches.next().is_some() {
            return Err(format!(
                "repository mapping for '@{requested}' is ambiguous"
            ));
        }
        Ok(repository.1.clone())
    })
    .map_err(anyhow::Error::msg)
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
    resolve_label(value, source)
}

/// Resolve a rule-implementation string in the defining `.bzl` module's
/// canonical repository context.
pub fn resolve_rule_definition_label(
    value: &str,
    source: &BzlModuleIdentity,
) -> anyhow::Result<CanonicalLabel> {
    resolve_label(value, source)
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

pub(crate) fn subrule_toolchain_requirements(
    value: Option<Value>,
    eval: &Evaluator<'_, '_, '_>,
) -> anyhow::Result<Arc<[ToolchainTypeRequirement]>> {
    let Some(value) = value else {
        return Ok(Arc::from([]));
    };
    let values = if let Some(values) = ListRef::from_value(value) {
        values.iter().collect::<Vec<_>>()
    } else if let Some(values) = TupleRef::from_value(value) {
        values.iter().collect::<Vec<_>>()
    } else {
        anyhow::bail!("toolchains requires a sequence")
    };
    let context = BzlEvaluationContext::from_evaluator(eval)?;
    let source = context.source_identity_for_call(eval)?;
    let mut requirements = Vec::<ToolchainTypeRequirement>::new();
    for value in values {
        let requirement = if let Some(value) = StarlarkToolchainTypeRequirement::from_value(value) {
            value.0.clone()
        } else if let Some(value) = StarlarkLabel::from_value(value) {
            ToolchainTypeRequirement::new(value.canonical().clone(), true)
        } else if let Some(value) = value.unpack_str() {
            ToolchainTypeRequirement::new(direct_toolchain_label(value, source)?, true)
        } else {
            anyhow::bail!("toolchains entries must be Strings, Labels, or toolchain_type values");
        };
        if let Some(existing) = requirements
            .iter_mut()
            .find(|existing| existing.label == requirement.label)
        {
            existing.mandatory |= requirement.mandatory;
        } else {
            requirements.push(requirement);
        }
    }
    Ok(requirements.into())
}

fn aspect_toolchain_requirements(
    value: Option<Value>,
    subrules: &AttachedSubrules,
    eval: &Evaluator<'_, '_, '_>,
) -> anyhow::Result<Arc<[ToolchainTypeRequirement]>> {
    let mut requirements = subrule_toolchain_requirements(value, eval)?.to_vec();
    for requirement in subrules
        .definitions
        .iter()
        .flat_map(|definition| definition.toolchains.iter())
    {
        if let Some(existing) = requirements
            .iter_mut()
            .find(|existing| existing.label() == requirement.label())
        {
            if requirement.mandatory() && !existing.mandatory() {
                *existing = requirement.clone();
            }
        } else {
            requirements.push(requirement.clone());
        }
    }
    Ok(requirements.into())
}

fn aspect_exec_compatible_with(
    values: Option<UnpackListOrTuple<&str>>,
    source: &BzlModuleIdentity,
) -> anyhow::Result<Arc<[CanonicalLabel]>> {
    let mut seen = SmallSet::new();
    values
        .unwrap_or_default()
        .items
        .into_iter()
        .filter_map(|value| match direct_toolchain_label(value, source) {
            Ok(label) if seen.insert(label.clone()) => Some(Ok(label)),
            Ok(_) => None,
            Err(error) => Some(Err(error)),
        })
        .collect::<anyhow::Result<Vec<_>>>()
        .map(Arc::from)
}

fn package_global(
    default_visibility: Option<UnpackVisibility>,
    default_deprecation: Option<&str>,
    default_testonly: Option<bool>,
    default_package_metadata: Option<UnpackListOrTuple<&str>>,
    eval: &mut Evaluator,
) -> anyhow::Result<NoneType> {
    let recorder = PackageRecorder::from_evaluator(eval)?;
    recorder.reject_macro_operation("package()")?;
    recorder.set_package_defaults(
        default_visibility.map(|value| value.items),
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
    visibility: Option<UnpackVisibility>,
    eval: &mut Evaluator,
) -> anyhow::Result<NoneType> {
    PackageRecorder::from_evaluator(eval)?
        .exports_files(list(srcs), visibility.map(|value| value.items))?;
    Ok(NoneType)
}

fn filegroup_global<'v>(
    name: &str,
    srcs: Option<Value<'v>>,
    visibility: Option<UnpackVisibility>,
    eval: &mut Evaluator<'v, '_, '_>,
) -> anyhow::Result<NoneType> {
    let recorder = PackageRecorder::from_evaluator(eval)?;
    let srcs = srcs
        .map(|srcs| coerce_starlark_value(recorder, AttributeKind::LabelList, "srcs", true, srcs))
        .transpose()?;
    recorder.filegroup(name.to_owned(), srcs, visibility.map(|value| value.items))?;
    recorder.set_native_generator_from_evaluator(name, eval)?;
    Ok(NoneType)
}

fn test_suite_global(
    name: &str,
    tests: Option<UnpackListOrTuple<&str>>,
    tags: UnpackListOrTuple<&str>,
    visibility: Option<UnpackVisibility>,
    eval: &mut Evaluator,
) -> anyhow::Result<NoneType> {
    let recorder = PackageRecorder::from_evaluator(eval)?;
    recorder.test_suite(
        name.to_owned(),
        tests.map(list),
        list(tags),
        visibility.map(|value| value.items),
    )?;
    recorder.set_native_generator_from_evaluator(name, eval)?;
    Ok(NoneType)
}

fn alias_global(
    name: &str,
    actual: &str,
    visibility: Option<UnpackVisibility>,
    eval: &mut Evaluator,
) -> anyhow::Result<NoneType> {
    let recorder = PackageRecorder::from_evaluator(eval)?;
    recorder.alias(
        name.to_owned(),
        actual.to_owned(),
        visibility.map(|value| value.items),
    )?;
    recorder.set_native_generator_from_evaluator(name, eval)?;
    Ok(NoneType)
}

#[derive(Debug, Clone, Copy)]
struct UnpackGlobExcludeDirectories(bool);

impl Default for UnpackGlobExcludeDirectories {
    fn default() -> Self {
        Self(true)
    }
}

impl starlark::values::type_repr::StarlarkTypeRepr for UnpackGlobExcludeDirectories {
    type Canonical = <i32 as starlark::values::type_repr::StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> starlark::typing::Ty {
        starlark::typing::Ty::int()
    }
}

impl<'v> UnpackValue<'v> for UnpackGlobExcludeDirectories {
    type Error = starlark::Error;

    fn unpack_value_impl(value: Value<'v>) -> starlark::Result<Option<Self>> {
        Ok((value.get_type() == "int").then(|| Self(value.to_bool())))
    }
}

fn glob_global<'v>(
    include: UnpackListOrTuple<&str>,
    exclude: UnpackListOrTuple<&str>,
    exclude_directories: UnpackGlobExcludeDirectories,
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
        exclude_directories.0,
        allow_empty,
    )?;
    PackageRecorder::for_glob(eval)?.glob(spec)
}

fn raw_attribute_value(value: Value) -> anyhow::Result<RawAttributeValue> {
    if let Some(value) = StarlarkLabel::from_value(value) {
        return Ok(RawAttributeValue::Label(value.canonical().clone()));
    }
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
                    RawLabelContext::Package(recorder),
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
        AttributeKind::IntegerList => CoercedAttributeValue::IntegerList(Arc::from([])),
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

#[derive(Clone, Copy)]
enum RawLabelContext<'a> {
    Root(&'a str),
    Definition(&'a BzlModuleIdentity),
    Package(&'a PackageRecorder),
}

impl RawLabelContext<'_> {
    fn label(self, value: &RawAttributeValue, context: &str) -> anyhow::Result<CanonicalLabel> {
        if let RawAttributeValue::Label(value) = value {
            return Ok(value.clone());
        }
        match self {
            Self::Root(package) => raw_label(package, value, context),
            Self::Definition(source) => resolve_label(&raw_string(value, context)?, source),
            Self::Package(recorder) => recorder.dependency_label(&raw_string(value, context)?),
        }
    }

    fn output(self, value: &RawAttributeValue, context: &str) -> anyhow::Result<CanonicalLabel> {
        if let RawAttributeValue::Label(value) = value {
            let expected_package = match self {
                Self::Root(package) => {
                    value.package().repo().is_root()
                        && value.package().package().as_str() == package
                }
                Self::Definition(source) => value.package() == source.label.package(),
                Self::Package(recorder) => value.package() == &recorder.package_identifier,
            };
            if !expected_package {
                anyhow::bail!("output label must name a valid target in this package: {value}");
            }
            return Ok(value.clone());
        }
        match self {
            Self::Root(package) => raw_output(package, value, context),
            Self::Definition(source) => {
                raw_output(source.label.package().package().as_str(), value, context)
            }
            Self::Package(recorder) => recorder.output_label(&raw_string(value, context)?),
        }
    }
}

fn coerce_raw_value(
    context: RawLabelContext<'_>,
    kind: AttributeKind,
    raw: &RawAttributeValue,
) -> anyhow::Result<CoercedAttributeValue> {
    let labels = |values: &[RawAttributeValue], label_context| {
        values
            .iter()
            .map(|value| context.label(value, label_context))
            .collect::<anyhow::Result<Vec<_>>>()
    };
    match kind {
        AttributeKind::Label => Ok(CoercedAttributeValue::Label(context.label(raw, "label")?)),
        AttributeKind::Output => Ok(CoercedAttributeValue::Output(
            context.output(raw, "output")?,
        )),
        AttributeKind::String => Ok(CoercedAttributeValue::String(raw_string(raw, "string")?)),
        AttributeKind::Boolean => match raw {
            RawAttributeValue::Boolean(value) => Ok(CoercedAttributeValue::Boolean(*value)),
            _ => anyhow::bail!("attribute must be a bool"),
        },
        AttributeKind::Integer => match raw {
            RawAttributeValue::Integer(value) => Ok(CoercedAttributeValue::Integer(*value)),
            _ => anyhow::bail!("attribute must be an integer"),
        },
        AttributeKind::IntegerList => {
            let RawAttributeValue::List(values) = raw else {
                anyhow::bail!("attribute must be a list of integers");
            };
            let values = values
                .iter()
                .map(|value| match value {
                    RawAttributeValue::Integer(value) => Ok(*value),
                    _ => anyhow::bail!("attribute must be a list of integers"),
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            Ok(CoercedAttributeValue::IntegerList(values.into()))
        }
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
                    .map(|value| context.output(value, "output list"))
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
                            context.label(value, "dictionary value")?,
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
            let mut converted = Vec::with_capacity(values.len());
            for (key, value) in values.iter() {
                let key = context.label(key, "dictionary key")?;
                if converted
                    .iter()
                    .any(|(existing, _): &(CanonicalLabel, CompactString)| {
                        existing.bazel_natural_cmp(&key).is_eq()
                    })
                {
                    anyhow::bail!("duplicate canonical label dictionary key '{key}'");
                }
                converted.push((key, raw_string(value, "dictionary value")?));
            }
            Ok(CoercedAttributeValue::LabelKeyedStringDict(
                converted.into(),
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
                if matches!(&branch.condition, SelectorCondition::Raw(condition) if condition == "//conditions:default")
                {
                    default = Some(Arc::new(coerce_starlark_value(
                        recorder,
                        kind,
                        attribute_name,
                        configurable,
                        branch.value,
                    )?));
                } else {
                    let condition = match &branch.condition {
                        SelectorCondition::Raw(condition) => {
                            recorder.dependency_label(condition)?
                        }
                        SelectorCondition::Canonical(condition) => {
                            CanonicalLabel::parse(condition).map_err(anyhow::Error::msg)?
                        }
                    };
                    branches.push((
                        condition,
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
    coerce_raw_value(RawLabelContext::Package(recorder), kind, &raw)
}

fn coerce_string_set_default<'v>(
    value: Value<'v>,
    heap: Heap<'v>,
) -> anyhow::Result<CoercedAttributeValue> {
    if SetRef::unpack_value_opt(value).is_none() {
        anyhow::bail!("attribute `build_setting_default` must be a set of strings")
    }
    let mut values = value
        .iterate(heap)
        .map_err(|error| anyhow::anyhow!(error.to_string()))?
        .map(|value| {
            value
                .unpack_str()
                .map(CompactString::from)
                .ok_or_else(|| anyhow::anyhow!("string-set default members must be strings"))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    values.sort_unstable();
    Ok(CoercedAttributeValue::StringList(values.into()))
}

/// The callable returned by Bazel's `rule()` global during package loading.
/// It retains the implementation for Stage 6, but package construction never
/// executes that implementation.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Trace)]
struct RuleDefinitionGen<V> {
    implementation: V,
    #[trace(unsafe_ignore)]
    definition_source: Arc<BzlModuleIdentity>,
    #[trace(unsafe_ignore)]
    source_identities_by_filename: Arc<[(CompactString, BzlModuleIdentity)]>,
    #[trace(unsafe_ignore)]
    required_toolchains: Arc<[ToolchainTypeRequirement]>,
    #[trace(unsafe_ignore)]
    advertised_providers: Arc<[ProviderIdentity]>,
    #[trace(unsafe_ignore)]
    required_fragments: Arc<[CompactString]>,
    #[trace(unsafe_ignore)]
    attached_subrules: AttachedSubrules,
    subrule_callables: Vec<V>,
    #[trace(unsafe_ignore)]
    late_bound_attributes: Arc<[LateBoundRuleAttribute]>,
    #[trace(unsafe_ignore)]
    schema: Arc<[RuleAttributeSchemaGen<V>]>,
    executable: bool,
    test: bool,
    build_setting_definition: Option<BuildSettingDefinition>,
    incoming_transition: Option<TransitionDefinitionGen<V>>,
    outputs: RuleOutputsDefinitionGen<V>,
    output_to_genfiles: bool,
    #[trace(unsafe_ignore)]
    rule_class: OnceCell<CompactString>,
}

/// The frozen definition contains no export-time interior mutability. Its
/// shared capability is cloned into every package instance of this rule.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub(crate) struct FrozenRuleDefinition {
    implementation: FrozenValue,
    definition_source: Arc<BzlModuleIdentity>,
    source_identities_by_filename: Arc<[(CompactString, BzlModuleIdentity)]>,
    required_toolchains: Arc<[ToolchainTypeRequirement]>,
    advertised_providers: Arc<[ProviderIdentity]>,
    required_fragments: Arc<[CompactString]>,
    attached_subrules: AttachedSubrules,
    #[allocative(skip)]
    subrule_callables: Arc<[FrozenValue]>,
    late_bound_attributes: Arc<[LateBoundRuleAttribute]>,
    pub(crate) schema: Arc<[FrozenRuleAttributeSchema]>,
    capability: Arc<RuleCapability>,
    pub(crate) build_setting_definition: Option<BuildSettingDefinition>,
    incoming_transition: Option<FrozenTransitionDefinition>,
    outputs: RuleOutputsDefinitionGen<FrozenValue>,
    output_to_genfiles: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
struct MacroAttributeSchema {
    name: CompactString,
    kind: AttributeKind,
    mandatory: bool,
    configurable: bool,
    default: Option<CoercedAttributeValue>,
    default_to_none: bool,
    file_admissibility: FileAdmissibility,
    flags: AttributePropertyFlags,
    rule_class_admissibility: RuleClassAdmissibility,
    allowed_values: AllowedAttributeValues,
    allow_empty: bool,
}

impl MacroAttributeSchema {
    fn from_definition(name: &str, definition: &AttributeDefinition<'_>) -> anyhow::Result<Self> {
        if definition.late_bound_default.is_some()
            || definition.computed_default
            || definition.attached_aspect.is_some()
            || definition.transition.is_some()
            || !definition.required_providers.is_empty()
        {
            anyhow::bail!("macro attribute '{name}' uses an unsupported dependency constraint");
        }
        Ok(Self {
            name: name.into(),
            kind: definition.kind,
            mandatory: definition.mandatory,
            configurable: definition.configurable,
            default: definition.default.clone(),
            default_to_none: false,
            file_admissibility: definition.file_admissibility.clone(),
            flags: definition.flags,
            rule_class_admissibility: definition.rule_class_admissibility.clone(),
            allowed_values: definition.allowed_values.clone(),
            allow_empty: definition.allow_empty,
        })
    }

    fn inherited(schema: &FrozenRuleAttributeSchema) -> Option<Self> {
        (!schema.name.starts_with('_')
            && !matches!(
                schema.name.as_str(),
                "name"
                    | "visibility"
                    | "generator_name"
                    | "generator_function"
                    | "generator_location"
            )
            && schema.attached_aspect.is_none()
            && schema.transition.is_none()
            && schema.required_providers.is_empty())
        .then(|| Self {
            name: schema.name.clone(),
            kind: schema.kind,
            mandatory: schema.mandatory,
            configurable: schema.configurable,
            default: schema.default.clone(),
            default_to_none: !schema.mandatory,
            file_admissibility: schema.file_admissibility.clone(),
            flags: schema.flags,
            rule_class_admissibility: schema.rule_class_admissibility.clone(),
            allowed_values: schema.allowed_values.clone(),
            allow_empty: schema.allow_empty,
        })
    }

    fn inherited_transient(schema: &RuleAttributeSchema<'_>) -> Option<Self> {
        (!schema.name.starts_with('_')
            && !matches!(
                schema.name.as_str(),
                "name"
                    | "visibility"
                    | "generator_name"
                    | "generator_function"
                    | "generator_location"
            )
            && schema.attached_aspect.is_none()
            && schema.transition.is_none()
            && schema.required_providers.is_empty())
        .then(|| Self {
            name: schema.name.clone(),
            kind: schema.kind,
            mandatory: schema.mandatory,
            configurable: schema.configurable,
            default: schema.default.clone(),
            default_to_none: !schema.mandatory,
            file_admissibility: schema.file_admissibility.clone(),
            flags: schema.flags,
            rule_class_admissibility: schema.rule_class_admissibility.clone(),
            allowed_values: schema.allowed_values.clone(),
            allow_empty: schema.allow_empty,
        })
    }

    fn inherited_macro(schema: &Self) -> Self {
        Self {
            default_to_none: !schema.mandatory,
            ..schema.clone()
        }
    }
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Trace)]
struct SymbolicMacroDefinitionGen<V> {
    implementation: V,
    #[trace(unsafe_ignore)]
    definition_source: Arc<BzlModuleIdentity>,
    #[trace(unsafe_ignore)]
    source_identities_by_filename: Arc<[(CompactString, BzlModuleIdentity)]>,
    #[trace(unsafe_ignore)]
    attributes: Arc<[MacroAttributeSchema]>,
    #[trace(unsafe_ignore)]
    documentation: Option<CompactString>,
    #[allocative(skip)]
    #[trace(unsafe_ignore)]
    exported_name: OnceCell<CompactString>,
}

type SymbolicMacroDefinition<'v> = SymbolicMacroDefinitionGen<Value<'v>>;

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct FrozenSymbolicMacroDefinition {
    implementation: FrozenValue,
    definition_source: Arc<BzlModuleIdentity>,
    source_identities_by_filename: Arc<[(CompactString, BzlModuleIdentity)]>,
    attributes: Arc<[MacroAttributeSchema]>,
    documentation: Option<CompactString>,
    exported_name: CompactString,
}

starlark::starlark_complex_values!(SymbolicMacroDefinition);

impl<V> fmt::Display for SymbolicMacroDefinitionGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.exported_name.get() {
            Some(name) => write!(f, "macro {name}"),
            None => f.write_str("macro <unexported>"),
        }
    }
}

impl fmt::Display for FrozenSymbolicMacroDefinition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "macro {}", self.exported_name)
    }
}

impl<'v> Freeze for SymbolicMacroDefinition<'v> {
    type Frozen = FrozenSymbolicMacroDefinition;

    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        let Some(exported_name) = self.exported_name.into_inner() else {
            return Err(FreezeError::new(
                "the result of macro() must be assigned to a top-level variable".to_owned(),
            ));
        };
        Ok(FrozenSymbolicMacroDefinition {
            implementation: self.implementation.freeze(freezer)?,
            definition_source: self.definition_source,
            source_identities_by_filename: self.source_identities_by_filename,
            attributes: self.attributes,
            documentation: self.documentation,
            exported_name,
        })
    }
}

#[starlark_value(type = "macro")]
impl<'v> StarlarkValue<'v> for SymbolicMacroDefinition<'v> {
    type Canonical = FrozenSymbolicMacroDefinition;

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

    fn invoke(
        &self,
        _me: Value<'v>,
        _args: &Arguments<'v, '_>,
        _eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        Err(starlark::Error::new_other(anyhow::anyhow!(
            "macro() definitions may only be called after their .bzl module is frozen"
        )))
    }
}

#[starlark_value(type = "macro")]
impl<'v> StarlarkValue<'v> for FrozenSymbolicMacroDefinition {
    type Canonical = Self;

    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        invoke_symbolic_macro(self, args, eval)
    }
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
    pub(crate) fn advertised_providers(&self) -> &[ProviderIdentity] {
        &self.advertised_providers
    }

    #[cfg(test)]
    pub(crate) fn required_fragments(&self) -> &[CompactString] {
        &self.required_fragments
    }

    #[cfg(test)]
    pub(crate) fn capability(&self) -> &RuleCapability {
        &self.capability
    }

    #[cfg(test)]
    pub(crate) fn incoming_transition(&self) -> Option<&FrozenTransitionDefinition> {
        self.incoming_transition.as_ref()
    }

    fn reject_deferred_attribute_invocation(&self) -> anyhow::Result<()> {
        if let Some(attribute) = self
            .schema
            .iter()
            .find(|attribute| attribute.attached_aspect.is_some())
        {
            anyhow::bail!(
                "target invocation for aspect-bearing attribute '{}' is not supported",
                attribute.name
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
        let implementation = self.implementation.freeze(freezer)?;
        let first_late_bound_attribute = self
            .late_bound_attributes
            .first()
            .map(|attribute| self.schema[attribute.schema_index as usize].name.clone());
        let implementation = fail_closed_rule_implementation(
            freezer,
            implementation,
            &self.attached_subrules,
            first_late_bound_attribute,
        );
        let subrule_callables = self
            .subrule_callables
            .into_iter()
            .map(|value| value.freeze(freezer))
            .collect::<FreezeResult<Vec<_>>>()?
            .into();
        Ok(FrozenRuleDefinition {
            implementation,
            definition_source: self.definition_source,
            source_identities_by_filename: self.source_identities_by_filename,
            required_toolchains: self.required_toolchains,
            advertised_providers: self.advertised_providers,
            required_fragments: self.required_fragments,
            attached_subrules: self.attached_subrules,
            subrule_callables,
            late_bound_attributes: self.late_bound_attributes,
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
            build_setting_definition: self.build_setting_definition,
            incoming_transition: self
                .incoming_transition
                .map(|transition| transition.freeze(freezer))
                .transpose()?,
            outputs: self.outputs.freeze(freezer)?,
            output_to_genfiles: self.output_to_genfiles,
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
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) enum AspectAttributePropagationEdge {
    All,
    Public(CompactString),
    Private(CompactString),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) enum AspectToolchainPropagationEdge {
    All,
    Type(CanonicalLabel),
}

#[derive(Debug, Clone, Allocative)]
pub(crate) enum AspectPropagationEdgesGen<V, T> {
    Fixed(Arc<[T]>),
    Callback(V),
}

impl<V: PartialEq, T: PartialEq> PartialEq for AspectPropagationEdgesGen<V, T> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Fixed(left), Self::Fixed(right)) => {
                left.iter().all(|edge| right.contains(edge))
                    && right.iter().all(|edge| left.contains(edge))
            }
            (Self::Callback(left), Self::Callback(right)) => left == right,
            _ => false,
        }
    }
}

impl<V: Eq, T: Eq> Eq for AspectPropagationEdgesGen<V, T> {}

unsafe impl<'v, V: Trace<'v>, T> Trace<'v> for AspectPropagationEdgesGen<V, T> {
    fn trace(&mut self, tracer: &Tracer<'v>) {
        if let Self::Callback(callback) = self {
            callback.trace(tracer);
        }
    }
}

impl<'v, T> AspectPropagationEdgesGen<Value<'v>, T> {
    fn freeze(self, freezer: &Freezer) -> FreezeResult<AspectPropagationEdgesGen<FrozenValue, T>> {
        Ok(match self {
            Self::Fixed(edges) => AspectPropagationEdgesGen::Fixed(edges),
            Self::Callback(callback) => {
                AspectPropagationEdgesGen::Callback(callback.freeze(freezer)?)
            }
        })
    }
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Trace)]
struct AspectDefinitionGen<V> {
    implementation: V,
    attr_aspects: AspectPropagationEdgesGen<V, AspectAttributePropagationEdge>,
    toolchains_aspects: AspectPropagationEdgesGen<V, AspectToolchainPropagationEdge>,
    #[trace(unsafe_ignore)]
    attributes: Arc<[RuleAttributeSchemaGen<V>]>,
    #[trace(unsafe_ignore)]
    late_bound_attributes: Arc<[LateBoundRuleAttribute]>,
    #[trace(unsafe_ignore)]
    required_parameters: Arc<[CompactString]>,
    required_aspects: Vec<V>,
    propagation_predicate: Option<V>,
    #[trace(unsafe_ignore)]
    required_toolchains: Arc<[ToolchainTypeRequirement]>,
    #[trace(unsafe_ignore)]
    required_providers: Arc<[Arc<[ProviderIdentity]>]>,
    #[trace(unsafe_ignore)]
    required_aspect_providers: Arc<[Arc<[ProviderIdentity]>]>,
    #[trace(unsafe_ignore)]
    advertised_providers: Arc<[ProviderIdentity]>,
    #[trace(unsafe_ignore)]
    required_fragments: Arc<[CompactString]>,
    apply_to_generating_rules: bool,
    #[trace(unsafe_ignore)]
    exec_compatible_with: Arc<[CanonicalLabel]>,
    #[trace(unsafe_ignore)]
    attached_subrules: AttachedSubrules,
    subrule_callables: Vec<V>,
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
    pub(crate) attr_aspects: AspectPropagationEdgesGen<FrozenValue, AspectAttributePropagationEdge>,
    pub(crate) toolchains_aspects:
        AspectPropagationEdgesGen<FrozenValue, AspectToolchainPropagationEdge>,
    pub(crate) attributes: Arc<[FrozenRuleAttributeSchema]>,
    pub(crate) late_bound_attributes: Arc<[LateBoundRuleAttribute]>,
    pub(crate) required_parameters: Arc<[CompactString]>,
    pub(crate) required_aspects: Vec<FrozenValue>,
    pub(crate) propagation_predicate: Option<FrozenValue>,
    pub(crate) required_toolchains: Arc<[ToolchainTypeRequirement]>,
    pub(crate) required_providers: Arc<[Arc<[ProviderIdentity]>]>,
    pub(crate) required_aspect_providers: Arc<[Arc<[ProviderIdentity]>]>,
    pub(crate) advertised_providers: Arc<[ProviderIdentity]>,
    pub(crate) required_fragments: Arc<[CompactString]>,
    pub(crate) apply_to_generating_rules: bool,
    pub(crate) exec_compatible_with: Arc<[CanonicalLabel]>,
    pub(crate) attached_subrules: AttachedSubrules,
    #[allocative(skip)]
    subrule_callables: Vec<FrozenValue>,
    pub(crate) defining_label: CanonicalLabel,
    pub(crate) exported_name: Option<CompactString>,
}

type AspectDefinition<'v> = AspectDefinitionGen<Value<'v>>;

#[cfg(test)]
impl FrozenAspectDefinition {
    pub(crate) fn fixed_attr_aspect_names(&self) -> Vec<&str> {
        let AspectPropagationEdgesGen::Fixed(edges) = &self.attr_aspects else {
            panic!("expected fixed attribute propagation");
        };
        edges
            .iter()
            .map(|edge| match edge {
                AspectAttributePropagationEdge::All => "*",
                AspectAttributePropagationEdge::Public(name)
                | AspectAttributePropagationEdge::Private(name) => name.as_str(),
            })
            .collect()
    }
}

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
            attr_aspects: self.attr_aspects.freeze(freezer)?,
            toolchains_aspects: self.toolchains_aspects.freeze(freezer)?,
            attributes: self
                .attributes
                .iter()
                .cloned()
                .map(|schema| schema.freeze(freezer))
                .collect::<FreezeResult<Vec<_>>>()?
                .into(),
            late_bound_attributes: self.late_bound_attributes,
            required_parameters: self.required_parameters,
            required_aspects: self
                .required_aspects
                .into_iter()
                .map(|aspect| aspect.freeze(freezer))
                .collect::<FreezeResult<_>>()?,
            propagation_predicate: self
                .propagation_predicate
                .map(|predicate| predicate.freeze(freezer))
                .transpose()?,
            required_toolchains: self.required_toolchains,
            required_providers: self.required_providers,
            required_aspect_providers: self.required_aspect_providers,
            advertised_providers: self.advertised_providers,
            required_fragments: self.required_fragments,
            apply_to_generating_rules: self.apply_to_generating_rules,
            exec_compatible_with: self.exec_compatible_with,
            attached_subrules: self.attached_subrules,
            subrule_callables: self
                .subrule_callables
                .into_iter()
                .map(|subrule| subrule.freeze(freezer))
                .collect::<FreezeResult<_>>()?,
            defining_label: self.defining_label,
            exported_name: self.exported_name.into_inner(),
        })
    }
}

fn declaration_provider_identity(value: Value, argument: &str) -> anyhow::Result<ProviderIdentity> {
    if let Some(provider) = value.downcast_ref::<UserProviderCallable>() {
        return provider
            .id()
            .map(|id| ProviderIdentity::user(id.dupe()))
            .ok_or_else(|| anyhow::anyhow!("{argument} providers must be exported"));
    }
    if let Some(identity) = starlark_provider_identity(value) {
        return Ok(identity);
    }
    anyhow::bail!("{argument} must contain exported provider constructors")
}

fn advertised_provider_ids(
    value: Option<Value>,
    argument: &str,
) -> anyhow::Result<Arc<[ProviderIdentity]>> {
    let Some(value) = value else {
        return Ok(Arc::from([]));
    };
    let providers = if let Some(list) = ListRef::from_value(value) {
        list.iter().collect::<Vec<_>>()
    } else if let Some(tuple) = TupleRef::from_value(value) {
        tuple.iter().collect::<Vec<_>>()
    } else {
        anyhow::bail!("{argument} must be a sequence")
    };
    let mut seen = SmallSet::new();
    let mut result = Vec::with_capacity(providers.len());
    for provider in providers {
        let identity = declaration_provider_identity(provider, argument)?;
        if seen.insert(identity.clone()) {
            result.push(identity);
        }
    }
    Ok(result.into())
}

fn aspect_advertised_providers(value: Option<Value>) -> anyhow::Result<Arc<[ProviderIdentity]>> {
    advertised_provider_ids(value, "aspect provides")
}

fn required_configuration_fragments(
    fragments: Option<UnpackListOrTuple<&str>>,
) -> Arc<[CompactString]> {
    let mut seen = SmallSet::new();
    fragments
        .map_or_else(Vec::new, |fragments| fragments.items)
        .into_iter()
        .filter_map(|fragment| {
            let fragment = CompactString::new(fragment);
            seen.insert(fragment.clone()).then_some(fragment)
        })
        .collect::<Vec<_>>()
        .into()
}

fn rule_outputs_definition<'v>(
    value: Option<Value<'v>>,
) -> anyhow::Result<RuleOutputsDefinitionGen<Value<'v>>> {
    let Some(value) = value.filter(|value| !value.is_none()) else {
        return Ok(RuleOutputsDefinitionGen::Static(Arc::from([])));
    };
    if value.parameters_spec().is_some() {
        return Ok(RuleOutputsDefinitionGen::Callback(value));
    }
    let values = DictRef::from_value(value).ok_or_else(|| {
        anyhow::anyhow!("rule outputs must be a dict, Starlark function, or None")
    })?;
    let entries = output_string_pairs(values, "implicit outputs of the rule class")?;
    Ok(RuleOutputsDefinitionGen::Static(entries.into()))
}

fn output_string_pairs(
    values: DictRef<'_>,
    context: &str,
) -> anyhow::Result<Vec<(CompactString, CompactString)>> {
    let string = |value: Value<'_>, member: &str| {
        value
            .unpack_str()
            .map(CompactString::new)
            .ok_or_else(|| anyhow::anyhow!("{context} {member} must be strings"))
    };
    values
        .iter()
        .map(|(key, value)| Ok((string(key, "keys")?, string(value, "values")?)))
        .collect()
}

fn rule_build_setting(value: Option<Value<'_>>) -> anyhow::Result<Option<BuildSettingDefinition>> {
    const ERROR: &str = "rule build_setting must use config.int(), config.string(), config.bool(), config.string_list(), or config.string_set()";
    let Some(value) = value.filter(|value| !value.is_none()) else {
        return Ok(None);
    };
    let definition = if let Some(setting) = RootIntBuildSetting::from_value(value) {
        Some(BuildSettingDefinition::Integer { flag: setting.flag })
    } else if let Some(setting) = RootStringBuildSetting::from_value(value) {
        Some(BuildSettingDefinition::String {
            flag: setting.flag,
            allow_multiple: setting.allow_multiple,
        })
    } else if let Some(setting) = RootBoolBuildSetting::from_value(value) {
        Some(BuildSettingDefinition::Boolean { flag: setting.flag })
    } else if let Some(setting) = RootStringListBuildSetting::from_value(value) {
        Some(BuildSettingDefinition::StringList {
            flag: setting.flag,
            repeatable: setting.repeatable,
        })
    } else if let Some(setting) = RootStringSetBuildSetting::from_value(value) {
        Some(BuildSettingDefinition::StringSet {
            flag: setting.flag,
            repeatable: setting.repeatable,
        })
    } else {
        None
    };
    definition.map(Some).ok_or_else(|| anyhow::anyhow!(ERROR))
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

fn aspect_required_aspects(value: Option<Value>) -> anyhow::Result<Vec<Value>> {
    let values = match value {
        None => Vec::new(),
        Some(value) if ListRef::from_value(value).is_some() => {
            ListRef::from_value(value).unwrap().iter().collect()
        }
        Some(value) if TupleRef::from_value(value).is_some() => {
            TupleRef::from_value(value).unwrap().iter().collect()
        }
        Some(_) => anyhow::bail!("aspect requires must be a sequence"),
    };
    let mut result = Vec::new();
    for value in values {
        if value.downcast_ref::<AspectDefinition>().is_none()
            && value.downcast_ref::<FrozenAspectDefinition>().is_none()
        {
            anyhow::bail!("aspect requires must contain aspect values");
        }
        if !result.iter().any(|existing: &Value| existing.ptr_eq(value)) {
            result.push(value);
        }
    }
    Ok(result)
}

fn aspect_attribute_propagation<'v>(
    value: Option<Value<'v>>,
) -> anyhow::Result<AspectPropagationEdgesGen<Value<'v>, AspectAttributePropagationEdge>> {
    let Some(value) = value else {
        return Ok(AspectPropagationEdgesGen::Fixed(Arc::from([])));
    };
    if value.parameters_spec().is_some() {
        return Ok(AspectPropagationEdgesGen::Callback(value));
    }
    let values = starlark_string_sequence(value, "attr_aspects")?;
    if values.len() != 1 && values.iter().any(|value| *value == "*") {
        anyhow::bail!("'*' must be the only string in 'attr_aspects' list");
    }
    let mut seen = SmallSet::new();
    let edges = values
        .into_iter()
        .filter_map(|value| {
            let edge = if value == "*" {
                AspectAttributePropagationEdge::All
            } else if value.starts_with('_') {
                AspectAttributePropagationEdge::Private(value.into())
            } else {
                AspectAttributePropagationEdge::Public(value.into())
            };
            seen.insert(edge.clone()).then_some(edge)
        })
        .collect::<Vec<_>>();
    Ok(AspectPropagationEdgesGen::Fixed(edges.into()))
}

fn aspect_toolchain_propagation<'v>(
    value: Option<Value<'v>>,
    source: &BzlModuleIdentity,
) -> anyhow::Result<AspectPropagationEdgesGen<Value<'v>, AspectToolchainPropagationEdge>> {
    let Some(value) = value else {
        return Ok(AspectPropagationEdgesGen::Fixed(Arc::from([])));
    };
    if value.parameters_spec().is_some() {
        return Ok(AspectPropagationEdgesGen::Callback(value));
    }
    let values = starlark_string_sequence(value, "toolchains_aspects")?;
    if values.len() != 1 && values.iter().any(|value| *value == "*") {
        anyhow::bail!("'*' must be the only item in 'toolchains_aspects' list");
    }
    let mut seen = SmallSet::new();
    let edges = values
        .into_iter()
        .map(|value| {
            if value == "*" {
                Ok(AspectToolchainPropagationEdge::All)
            } else {
                direct_toolchain_label(value, source).map(AspectToolchainPropagationEdge::Type)
            }
        })
        .filter_map(|edge| match edge {
            Ok(edge) if seen.insert(edge.clone()) => Some(Ok(edge)),
            Ok(_) => None,
            Err(error) => Some(Err(error)),
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(AspectPropagationEdgesGen::Fixed(edges.into()))
}

fn starlark_string_sequence<'v>(value: Value<'v>, argument: &str) -> anyhow::Result<Vec<&'v str>> {
    let values = if let Some(values) = ListRef::from_value(value) {
        values.iter().collect::<Vec<_>>()
    } else if let Some(values) = TupleRef::from_value(value) {
        values.iter().collect::<Vec<_>>()
    } else {
        anyhow::bail!("{argument} must be a sequence of strings")
    };
    values
        .into_iter()
        .map(|value| {
            value
                .unpack_str()
                .ok_or_else(|| anyhow::anyhow!("{argument} must contain only strings"))
        })
        .collect()
}

/// Canonical disjunction of conjunctions over the shared provider identity.
/// Both levels are set-semantic; one empty conjunction means no restriction.
fn declaration_required_providers(
    value: Option<Value>,
    argument: &str,
) -> anyhow::Result<Arc<[Arc<[ProviderIdentity]>]>> {
    fn sequence(value: Value) -> Option<Vec<Value>> {
        if let Some(values) = ListRef::from_value(value) {
            Some(values.iter().collect())
        } else {
            TupleRef::from_value(value).map(|values| values.iter().collect())
        }
    }

    fn is_provider(value: Value) -> bool {
        value.downcast_ref::<UserProviderCallable>().is_some()
            || starlark_provider_identity(value).is_some()
    }

    let Some(value) = value else {
        return Ok(Arc::from([]));
    };
    let outer = sequence(value).ok_or_else(|| anyhow::anyhow!("{argument} must be a sequence"))?;
    if outer.is_empty() {
        return Ok(Arc::from([]));
    }

    let alternatives = if outer.iter().copied().all(is_provider) {
        vec![outer]
    } else {
        outer
            .into_iter()
            .map(|alternative| {
                sequence(alternative).ok_or_else(|| {
                        anyhow::anyhow!(
                            "{argument} must contain either providers or lists of providers, but not both"
                        )
                    })
            })
            .collect::<anyhow::Result<Vec<_>>>()?
    };

    let mut normalized = alternatives
        .into_iter()
        .map(|providers| {
            let mut result = providers
                .into_iter()
                .map(|provider| declaration_provider_identity(provider, argument))
                .collect::<anyhow::Result<Vec<_>>>()?;
            result.sort_by(provider_identity_cmp);
            result.dedup();
            Ok(result)
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    if normalized.iter().any(Vec::is_empty) {
        return Ok(Arc::from([]));
    }
    normalized.sort_by(|left, right| provider_identity_slice_cmp(left, right));
    normalized.dedup();
    Ok(normalized
        .into_iter()
        .map(Arc::from)
        .collect::<Vec<_>>()
        .into())
}

fn provider_identity_cmp(left: &ProviderIdentity, right: &ProviderIdentity) -> std::cmp::Ordering {
    match (left, right) {
        (ProviderIdentity::Builtin(left), ProviderIdentity::Builtin(right)) => left.cmp(right),
        (ProviderIdentity::Builtin(_), ProviderIdentity::User(_)) => std::cmp::Ordering::Less,
        (ProviderIdentity::User(_), ProviderIdentity::Builtin(_)) => std::cmp::Ordering::Greater,
        (ProviderIdentity::User(left), ProviderIdentity::User(right)) => left.cmp(right),
    }
}

fn provider_identity_slice_cmp(
    left: &[ProviderIdentity],
    right: &[ProviderIdentity],
) -> std::cmp::Ordering {
    for (left, right) in left.iter().zip(right) {
        let ordering = provider_identity_cmp(left, right);
        if ordering != std::cmp::Ordering::Equal {
            return ordering;
        }
    }
    left.len().cmp(&right.len())
}

fn label_list_required_providers(
    value: Option<Value>,
) -> anyhow::Result<Arc<[Arc<[ProviderIdentity]>]>> {
    declaration_required_providers(value, "attribute providers")
}

fn label_required_provider(value: Option<Value>) -> anyhow::Result<Arc<[Arc<[ProviderIdentity]>]>> {
    declaration_required_providers(value, "attribute providers")
}

fn label_list_attached_aspect(value: Option<Value>) -> anyhow::Result<Option<Value>> {
    aspect_required_aspect(value)
}

fn aspect_attributes<'v>(
    attrs: Option<SmallMap<String, Value<'v>>>,
) -> anyhow::Result<(
    Arc<[RuleAttributeSchema<'v>]>,
    Arc<[LateBoundRuleAttribute]>,
    Arc<[CompactString]>,
)> {
    let mut schemas = Vec::new();
    let mut late_bound_attributes = Vec::new();
    let mut required_parameters = Vec::new();
    for (name, value) in attrs.unwrap_or_default() {
        if !is_starlark_identifier(&name) {
            anyhow::bail!("attribute name `{name}` is not a valid identifier.");
        }
        let definition = attribute_definition_from_value(value)?
            .ok_or_else(|| anyhow::anyhow!("aspect attribute `{name}` must use attr.*()"))?;
        if definition.configurable_set {
            anyhow::bail!(
                "attribute '{name}' has the 'configurable' argument set, which is not allowed in aspect definitions"
            );
        }
        if definition.computed_default {
            anyhow::bail!("Aspect attribute '{name}' with computed default value is unsupported.");
        }
        if name.starts_with('_') {
            if definition.default.is_none() && definition.late_bound_default.is_none() {
                anyhow::bail!("Aspect attribute '{name}' has no default value.");
            }
        } else {
            if !matches!(
                definition.kind,
                AttributeKind::Boolean | AttributeKind::Integer | AttributeKind::String
            ) {
                anyhow::bail!(
                    "Aspect parameter attribute '{name}' must have type 'bool', 'int' or 'string'."
                );
            }
            let has_default = definition
                .default
                .as_ref()
                .is_some_and(|default| default != &intrinsic_default(definition.kind));
            if has_default {
                validate_allowed_value(
                    &name,
                    definition.default.as_ref().expect("checked above"),
                    &definition.allowed_values,
                )
                .map_err(|error| {
                    anyhow::anyhow!(
                        "Aspect parameter attribute '{name}' has a bad default value: {error}"
                    )
                })?;
            }
            if !has_default || definition.mandatory {
                required_parameters.push(CompactString::new(&name));
            }
        }
        if let Some(identity) = &definition.late_bound_default {
            late_bound_attributes.push(LateBoundRuleAttribute {
                schema_index: u32::try_from(schemas.len())
                    .expect("aspect attribute count fits in u32"),
                identity: identity.clone(),
                required_providers: definition.required_providers.clone(),
            });
        }
        schemas.push(declared_attribute_schema(name, &definition));
    }
    Ok((
        schemas.into(),
        late_bound_attributes.into(),
        required_parameters.into(),
    ))
}

fn is_starlark_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
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
    Label(CanonicalLabel),
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
    pub(crate) file_admissibility: FileAdmissibility,
    #[trace(unsafe_ignore)]
    pub(crate) flags: AttributePropertyFlags,
    #[trace(unsafe_ignore)]
    pub(crate) rule_class_admissibility: RuleClassAdmissibility,
    #[trace(unsafe_ignore)]
    pub(crate) allowed_values: AllowedAttributeValues,
    #[trace(unsafe_ignore)]
    pub(crate) allow_empty: bool,
    #[trace(unsafe_ignore)]
    pub(crate) executable: bool,
    #[trace(unsafe_ignore)]
    pub(crate) exec_configuration: bool,
    #[trace(unsafe_ignore)]
    pub(crate) required_providers: Arc<[Arc<[ProviderIdentity]>]>,
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
        file_admissibility: definition.file_admissibility.clone(),
        flags: definition.flags,
        rule_class_admissibility: definition.rule_class_admissibility.clone(),
        allowed_values: definition.allowed_values.clone(),
        allow_empty: definition.allow_empty,
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
    build_setting_definition: Option<BuildSettingDefinition>,
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
            file_admissibility: FileAdmissibility::default(),
            flags: AttributePropertyFlags::default(),
            rule_class_admissibility: RuleClassAdmissibility::Any,
            allowed_values: AllowedAttributeValues::None,
            allow_empty: true,
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
    if let Some(kind) = build_setting_definition {
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

fn symbolic_macro_generator_metadata(
    recorder: &PackageRecorder,
    eval: &Evaluator<'_, '_, '_>,
    name: &str,
    exported_name: &str,
) -> (CompactString, CompactString, CompactString) {
    let metadata = starlark_generator_metadata(recorder, eval);
    if !metadata.0.is_empty() {
        return metadata;
    }
    let Some(call_location) = eval.call_stack_top_location() else {
        return (name.into(), exported_name.into(), CompactString::default());
    };
    let position = call_location.resolve_span_for_reporting().begin;
    let build_file = Path::new(call_location.filename())
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("BUILD.bazel");
    let build_file = if recorder.package.is_empty() {
        build_file.to_owned()
    } else {
        format!("{}/{build_file}", recorder.package)
    };
    (
        name.into(),
        exported_name.into(),
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
    inputs: Arc<[TransitionSetting]>,
    #[trace(unsafe_ignore)]
    #[freeze(identity)]
    outputs: Arc<[TransitionSetting]>,
    #[trace(unsafe_ignore)]
    #[freeze(identity)]
    definition_source: Arc<BzlModuleIdentity>,
    #[trace(unsafe_ignore)]
    #[freeze(identity)]
    source_identities_by_filename: Arc<[(CompactString, BzlModuleIdentity)]>,
}
type TransitionDefinition<'v> = TransitionDefinitionGen<Value<'v>>;
pub(crate) type FrozenTransitionDefinition = TransitionDefinitionGen<FrozenValue>;
starlark::starlark_complex_values!(TransitionDefinition);

fn transition_definition_from_value<'v>(value: Value<'v>) -> Option<TransitionDefinition<'v>> {
    match TransitionDefinition::from_value(value)? {
        starlark::__macro_refs::Either::Left(value) => Some(value.clone()),
        starlark::__macro_refs::Either::Right(value) => Some(TransitionDefinitionGen {
            implementation: value.implementation.to_value(),
            inputs: value.inputs.clone(),
            outputs: value.outputs.clone(),
            definition_source: value.definition_source.clone(),
            source_identities_by_filename: value.source_identities_by_filename.clone(),
        }),
    }
}

impl FrozenTransitionDefinition {
    #[cfg(test)]
    pub(crate) fn implementation(&self) -> FrozenValue {
        self.implementation
    }

    #[cfg(test)]
    pub(crate) fn inputs(&self) -> &[TransitionSetting] {
        &self.inputs
    }

    #[cfg(test)]
    pub(crate) fn outputs(&self) -> &[TransitionSetting] {
        &self.outputs
    }

    #[cfg(test)]
    pub(crate) fn definition_source(&self) -> &Arc<BzlModuleIdentity> {
        &self.definition_source
    }

    #[cfg(test)]
    pub(crate) fn source_identities_by_filename(
        &self,
    ) -> &Arc<[(CompactString, BzlModuleIdentity)]> {
        &self.source_identities_by_filename
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
    file_admissibility: FileAdmissibility,
    #[trace(unsafe_ignore)]
    flags: AttributePropertyFlags,
    #[trace(unsafe_ignore)]
    rule_class_admissibility: RuleClassAdmissibility,
    #[trace(unsafe_ignore)]
    allowed_values: AllowedAttributeValues,
    #[trace(unsafe_ignore)]
    allow_empty: bool,
    #[trace(unsafe_ignore)]
    default: Option<CoercedAttributeValue>,
    #[trace(unsafe_ignore)]
    late_bound_default: Option<slug_configuration_v2::ConfigurationFieldIdentity>,
    #[trace(unsafe_ignore)]
    computed_default: bool,
    #[trace(unsafe_ignore)]
    executable: bool,
    #[trace(unsafe_ignore)]
    exec_configuration: bool,
    #[trace(unsafe_ignore)]
    required_providers: Arc<[Arc<[ProviderIdentity]>]>,
    attached_aspect: Option<V>,
    transition: Option<TransitionDefinitionGen<V>>,
}
type AttributeDefinition<'v> = AttributeDefinitionGen<Value<'v>>;
type FrozenAttributeDefinition = AttributeDefinitionGen<FrozenValue>;
starlark::starlark_complex_values!(AttributeDefinition);

fn attribute_definition_from_value<'v>(
    value: Value<'v>,
) -> anyhow::Result<Option<AttributeDefinition<'v>>> {
    let Some(definition) = AttributeDefinition::from_value(value) else {
        return Ok(None);
    };
    let definition = match definition {
        starlark::__macro_refs::Either::Left(value) => value.clone(),
        starlark::__macro_refs::Either::Right(value) => AttributeDefinitionGen {
            kind: value.kind,
            mandatory: value.mandatory,
            configurable: value.configurable,
            configurable_set: value.configurable_set,
            file_admissibility: value.file_admissibility.clone(),
            flags: value.flags,
            rule_class_admissibility: value.rule_class_admissibility.clone(),
            allowed_values: value.allowed_values.clone(),
            allow_empty: value.allow_empty,
            default: value.default.clone(),
            late_bound_default: value.late_bound_default.clone(),
            computed_default: value.computed_default,
            executable: value.executable,
            exec_configuration: value.exec_configuration,
            required_providers: value.required_providers.clone(),
            attached_aspect: value.attached_aspect.as_ref().map(|value| value.to_value()),
            transition: value
                .transition
                .as_ref()
                .map(|transition| TransitionDefinitionGen {
                    implementation: transition.implementation.to_value(),
                    inputs: transition.inputs.clone(),
                    outputs: transition.outputs.clone(),
                    definition_source: transition.definition_source.clone(),
                    source_identities_by_filename: transition.source_identities_by_filename.clone(),
                }),
        },
    };
    if definition
        .flags
        .contains(AttributePropertyFlag::CheckAllowedValues)
        && matches!(definition.allowed_values, AllowedAttributeValues::None)
    {
        anyhow::bail!(
            "attribute property CHECK_ALLOWED_VALUES has no owned allowed-values predicate"
        );
    }
    Ok(Some(definition))
}

pub(crate) fn subrule_attribute_from_value<'v>(
    name: String,
    value: Value<'v>,
) -> anyhow::Result<SubruleAttribute> {
    fn valid_identifier(name: &str) -> bool {
        let mut chars = name.chars();
        chars
            .next()
            .is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
            && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
    }

    fn convert<V>(
        name: String,
        definition: &AttributeDefinitionGen<V>,
    ) -> anyhow::Result<SubruleAttribute> {
        if !valid_identifier(&name) {
            anyhow::bail!("attribute name `{name}` is not a valid identifier.");
        }
        if name.len() > 128 {
            anyhow::bail!("attribute {name}: name is too long ({} > 128)", name.len());
        }
        if definition.transition.is_some() {
            anyhow::bail!(
                "bad cfg for attribute '{name}': subrules may only have target/exec attributes."
            );
        }
        if !name.starts_with('_') {
            anyhow::bail!(
                "illegal attribute name '{name}': subrules may only define private attributes (whose names begin with '_')."
            );
        }
        if definition.computed_default {
            anyhow::bail!(
                "illegal default value for attribute '{name}': subrules cannot define computed defaults."
            );
        }
        let default = match (&definition.default, &definition.late_bound_default) {
            (Some(default), None) => SubruleAttributeDefault::Literal(default.clone()),
            (None, Some(default)) => SubruleAttributeDefault::ConfigurationField(default.clone()),
            (None, None) => anyhow::bail!("for attribute '{name}': no default value specified"),
            (Some(_), Some(_)) => unreachable!("one default source is retained"),
        };
        if !matches!(
            definition.kind,
            AttributeKind::Label | AttributeKind::LabelList
        ) {
            anyhow::bail!(
                "bad type for attribute '{name}': subrule attributes may only be label or lists of labels."
            );
        }
        if definition.attached_aspect.is_some() {
            anyhow::bail!("subrule attribute '{name}' uses a deferred attached aspect");
        }
        Ok(SubruleAttribute {
            user_name: name.into(),
            kind: definition.kind,
            configurable: definition.configurable,
            default,
            file_admissibility: definition.file_admissibility.clone(),
            flags: definition.flags,
            rule_class_admissibility: definition.rule_class_admissibility.clone(),
            allowed_values: definition.allowed_values.clone(),
            executable: definition.executable,
            exec_configuration: definition.exec_configuration,
            required_providers: definition.required_providers.clone(),
        })
    }

    let definition = attribute_definition_from_value(value)?
        .ok_or_else(|| anyhow::anyhow!("subrule attribute '{name}' must use attr.*()"))?;
    convert(name, &definition)
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
            file_admissibility: self.file_admissibility,
            flags: self.flags,
            rule_class_admissibility: self.rule_class_admissibility,
            allowed_values: self.allowed_values,
            allow_empty: self.allow_empty,
            default: self.default,
            late_bound_default: self.late_bound_default,
            computed_default: self.computed_default,
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
            file_admissibility: self.file_admissibility,
            flags: self.flags,
            rule_class_admissibility: self.rule_class_admissibility,
            allowed_values: self.allowed_values,
            allow_empty: self.allow_empty,
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
    pub(crate) file_admissibility: FileAdmissibility,
    pub(crate) allowed_values: AllowedAttributeValues,
    pub(crate) allow_empty: bool,
}

pub(crate) type ModuleExtensionTagCoercionError = CompactString;

fn module_extension_label(
    raw: &str,
    context_package: &PackageIdentifier,
    mapping: &SmallMap<ApparentRepoName, CanonicalRepoName>,
) -> Result<CanonicalLabel, ModuleExtensionTagCoercionError> {
    CanonicalLabel::parse_with_package_context(raw, context_package, |requested| {
        mapping
            .iter()
            .find_map(|(name, repository)| (name.as_str() == requested).then(|| repository.clone()))
            .ok_or_else(|| format!("no repository visible as '@{requested}'"))
    })
    .map_err(CompactString::from)
}

fn module_extension_sequence(
    raw: &NonrootAttributeValue,
) -> Result<&[NonrootAttributeValue], ModuleExtensionTagCoercionError> {
    match raw {
        NonrootAttributeValue::List(values) | NonrootAttributeValue::Tuple(values) => {
            Ok(values.as_ref())
        }
        _ => Err("module-extension attribute value must be a list or tuple".into()),
    }
}

fn module_extension_dict(
    raw: &NonrootAttributeValue,
) -> Result<&SmallMap<NonrootAttributeKey, NonrootAttributeValue>, ModuleExtensionTagCoercionError>
{
    match raw {
        NonrootAttributeValue::Dict(values) => Ok(values.as_ref()),
        _ => Err("module-extension attribute value must be a dictionary".into()),
    }
}

fn coerce_module_extension_value(
    kind: AttributeKind,
    raw: &NonrootAttributeValue,
    context_package: &PackageIdentifier,
    mapping: &SmallMap<ApparentRepoName, CanonicalRepoName>,
) -> Result<CoercedAttributeValue, ModuleExtensionTagCoercionError> {
    let string = |raw: &NonrootAttributeValue| match raw {
        NonrootAttributeValue::String(value) => Ok(value.clone()),
        _ => Err(CompactString::from(
            "module-extension attribute value must be a string",
        )),
    };
    let label = |raw: &NonrootAttributeValue| match raw {
        NonrootAttributeValue::String(value) => {
            module_extension_label(value, context_package, mapping)
        }
        NonrootAttributeValue::Label(value) => {
            CanonicalLabel::parse(value).map_err(CompactString::from)
        }
        _ => Err(CompactString::from(
            "module-extension attribute value must be a label",
        )),
    };
    let output = |raw: &NonrootAttributeValue| {
        let raw = match raw {
            NonrootAttributeValue::String(value) | NonrootAttributeValue::Label(value) => value,
            _ => return Err("module-extension attribute value must be a label".into()),
        };
        if raw.starts_with("@@") || (raw.starts_with('@') && !raw.contains("//")) {
            return Err(format!("unsupported module-extension output label '{raw}'").into());
        }
        let rewritten = raw.strip_prefix("@//").map(|rest| format!("//{rest}"));
        let raw = rewritten.as_deref().unwrap_or(raw);
        let value = module_extension_label(raw, context_package, mapping)?;
        if !value.package().package().as_str().is_empty() {
            return Err(CompactString::from(format!(
                "output label '{value}' is not in the current package"
            )));
        }
        Ok(value)
    };
    let string_key = |key: &NonrootAttributeKey| match key {
        NonrootAttributeKey::String(value) => Ok(value.clone()),
        _ => Err(CompactString::from(
            "module-extension dictionary key must be a string",
        )),
    };
    let label_key = |key: &NonrootAttributeKey| match key {
        NonrootAttributeKey::String(value) => {
            module_extension_label(value, context_package, mapping)
        }
        NonrootAttributeKey::Label(value) => {
            CanonicalLabel::parse(value).map_err(CompactString::from)
        }
        _ => Err(CompactString::from(
            "module-extension dictionary key must be a label",
        )),
    };
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
        (AttributeKind::Label, _) => label(raw).map(CoercedAttributeValue::Label),
        (AttributeKind::Output, _) => output(raw).map(CoercedAttributeValue::Output),
        (AttributeKind::IntegerList, _) => module_extension_sequence(raw)?
            .iter()
            .map(|value| match value {
                NonrootAttributeValue::Int(value) => value
                    .as_i32()
                    .ok_or_else(|| CompactString::from("integer-list member is outside i32")),
                _ => Err(CompactString::from(
                    "module-extension integer-list member must be an integer",
                )),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|values| CoercedAttributeValue::IntegerList(values.into())),
        (AttributeKind::StringList, _) => module_extension_sequence(raw)?
            .iter()
            .map(string)
            .collect::<Result<Vec<_>, _>>()
            .map(|values| CoercedAttributeValue::StringList(values.into())),
        (AttributeKind::LabelList, _) => module_extension_sequence(raw)?
            .iter()
            .map(label)
            .collect::<Result<Vec<_>, _>>()
            .map(|values| CoercedAttributeValue::LabelList(values.into())),
        (AttributeKind::OutputList, _) => module_extension_sequence(raw)?
            .iter()
            .map(output)
            .collect::<Result<Vec<_>, _>>()
            .map(|values| CoercedAttributeValue::OutputList(values.into())),
        (AttributeKind::StringDict, _) => module_extension_dict(raw)?
            .iter()
            .map(|(key, value)| Ok((string_key(key)?, string(value)?)))
            .collect::<Result<Vec<_>, _>>()
            .map(Into::into)
            .map(CoercedAttributeValue::StringDict),
        (AttributeKind::StringListDict, _) => module_extension_dict(raw)?
            .iter()
            .map(|(key, value)| {
                Ok((
                    string_key(key)?,
                    module_extension_sequence(value)?
                        .iter()
                        .map(string)
                        .collect::<Result<Vec<_>, _>>()?
                        .into(),
                ))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Into::into)
            .map(CoercedAttributeValue::StringListDict),
        (AttributeKind::StringKeyedLabelDict, _) => module_extension_dict(raw)?
            .iter()
            .map(|(key, value)| Ok((string_key(key)?, label(value)?)))
            .collect::<Result<Vec<_>, _>>()
            .map(Into::into)
            .map(CoercedAttributeValue::StringKeyedLabelDict),
        (AttributeKind::LabelKeyedStringDict, _) => {
            let mut values = Vec::new();
            for (key, value) in module_extension_dict(raw)? {
                let key = label_key(key)?;
                if values
                    .iter()
                    .any(|(existing, _): &(CanonicalLabel, CompactString)| {
                        existing.bazel_natural_cmp(&key).is_eq()
                    })
                {
                    return Err(format!("duplicate canonical label dictionary key '{key}'").into());
                }
                values.push((key, string(value)?));
            }
            Ok(CoercedAttributeValue::LabelKeyedStringDict(values.into()))
        }
        (AttributeKind::LabelListDict, _) => module_extension_dict(raw)?
            .iter()
            .map(|(key, value)| {
                Ok((
                    string_key(key)?,
                    module_extension_sequence(value)?
                        .iter()
                        .map(label)
                        .collect::<Result<Vec<_>, _>>()?
                        .into(),
                ))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Into::into)
            .map(CoercedAttributeValue::LabelListDict),
        _ => Err(format!("unsupported value for module-extension {kind:?} attribute").into()),
    }
}

fn module_extension_intrinsic_default(kind: AttributeKind) -> CoercedAttributeValue {
    intrinsic_default(kind)
}

fn module_extension_default_matches(kind: AttributeKind, value: &CoercedAttributeValue) -> bool {
    matches!(
        (kind, value),
        (AttributeKind::String, CoercedAttributeValue::String(_))
            | (AttributeKind::Boolean, CoercedAttributeValue::Boolean(_))
            | (AttributeKind::Integer, CoercedAttributeValue::Integer(_))
            | (
                AttributeKind::IntegerList,
                CoercedAttributeValue::IntegerList(_)
            )
            | (
                AttributeKind::Label,
                CoercedAttributeValue::Label(_) | CoercedAttributeValue::None
            )
            | (
                AttributeKind::Output,
                CoercedAttributeValue::Output(_) | CoercedAttributeValue::None
            )
            | (
                AttributeKind::StringList,
                CoercedAttributeValue::StringList(_)
            )
            | (
                AttributeKind::LabelList,
                CoercedAttributeValue::LabelList(_)
            )
            | (
                AttributeKind::OutputList,
                CoercedAttributeValue::OutputList(_)
            )
            | (
                AttributeKind::StringDict,
                CoercedAttributeValue::StringDict(_)
            )
            | (
                AttributeKind::StringListDict,
                CoercedAttributeValue::StringListDict(_)
            )
            | (
                AttributeKind::StringKeyedLabelDict,
                CoercedAttributeValue::StringKeyedLabelDict(_)
            )
            | (
                AttributeKind::LabelKeyedStringDict,
                CoercedAttributeValue::LabelKeyedStringDict(_)
            )
            | (
                AttributeKind::LabelListDict,
                CoercedAttributeValue::LabelListDict(_)
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
    let context_package = PackageIdentifier::new(context_repo.clone(), PackagePath::root());
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
            coerce_module_extension_value(attribute.kind, raw, &context_package, mapping)?,
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
            validate_allowed_value(&attribute.name, &value, &attribute.allowed_values)
                .map_err(|error| CompactString::from(error.to_string()))?;
            Ok((attribute.name.clone(), value))
        })
        .collect::<Result<Arc<_>, _>>()
}

pub(crate) fn validate_module_extension_tag_schema(
    schema: &[ModuleExtensionTagAttribute],
) -> Result<(), ModuleExtensionTagCoercionError> {
    for attribute in schema {
        if attribute
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

#[derive(Debug, Clone, Trace, Freeze, Allocative)]
enum SelectorCondition {
    Raw(String),
    Canonical(String),
}

#[derive(Debug, Trace, Freeze, ProvidesStaticType, NoSerialize, Allocative)]
struct SelectorBranchGen<V> {
    condition: SelectorCondition,
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
    file_admissibility: FileAdmissibility,
    executable: bool,
    default: Option<Value<'v>>,
    cfg: Option<Value<'v>>,
    eval: &Evaluator<'v, '_, '_>,
) -> anyhow::Result<AttributeDefinition<'v>> {
    let mut definition = attribute_definition_before_later_properties(
        kind,
        mandatory,
        configurable,
        executable,
        default,
        eval,
    )?;
    validate_executable_cfg_presence(executable, cfg)?;
    definition.file_admissibility = file_admissibility;
    set_attribute_cfg(&mut definition, cfg)?;
    Ok(definition)
}

fn attribute_definition_before_later_properties<'v>(
    kind: AttributeKind,
    mandatory: bool,
    configurable: Option<bool>,
    executable: bool,
    default: Option<Value<'v>>,
    eval: &Evaluator<'v, '_, '_>,
) -> anyhow::Result<AttributeDefinition<'v>> {
    let mut late_bound_default = None;
    let mut computed_default = false;
    let default = default
        .map(|value| {
            if let Some(value) = ConfigurationFieldValue::from_value(value) {
                if kind != AttributeKind::Label {
                    anyhow::bail!("configuration_field may only be the default of attr.label");
                }
                late_bound_default = Some(value.identity().clone());
                return Ok(None);
            }
            if value.parameters_spec().is_some() {
                computed_default = true;
                return Ok(None);
            }
            if value.is_none() && kind == AttributeKind::Label {
                return Ok(Some(CoercedAttributeValue::None));
            }
            let context = BzlEvaluationContext::from_evaluator(eval)?;
            if kind == AttributeKind::Label {
                let source = context.source_identity_for_call(eval)?;
                return coerce_label_default(value, source).map(Some);
            }
            let raw = raw_attribute_value(value)?;
            if matches!(
                kind,
                AttributeKind::LabelList
                    | AttributeKind::StringKeyedLabelDict
                    | AttributeKind::LabelKeyedStringDict
                    | AttributeKind::LabelListDict
            ) {
                let source = context.source_identity_for_call(eval)?;
                coerce_raw_value(RawLabelContext::Definition(source), kind, &raw).map(Some)
            } else {
                let source = context.source_label_for_call(eval)?;
                coerce_raw_value(
                    RawLabelContext::Root(source.package().package().as_str()),
                    kind,
                    &raw,
                )
                .map(Some)
            }
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
        file_admissibility: FileAdmissibility::default(),
        flags: {
            let mut flags = AttributePropertyFlags::default();
            flags.insert(AttributePropertyFlag::StarlarkDefined);
            flags
        },
        rule_class_admissibility: RuleClassAdmissibility::Any,
        allowed_values: AllowedAttributeValues::None,
        allow_empty: true,
        default,
        late_bound_default,
        computed_default,
        executable,
        exec_configuration: false,
        required_providers: Arc::from([]),
        attached_aspect: None,
        transition: None,
    })
}

fn validate_executable_cfg_presence(
    executable: bool,
    cfg: Option<Value<'_>>,
) -> anyhow::Result<()> {
    if executable && !cfg.is_some_and(|value| !value.is_none()) {
        anyhow::bail!("cfg parameter is mandatory when executable=True is provided");
    }
    Ok(())
}

fn set_attribute_cfg<'v>(
    definition: &mut AttributeDefinition<'v>,
    cfg: Option<Value<'v>>,
) -> anyhow::Result<()> {
    let (exec_configuration, transition) = match cfg {
        None => (false, None),
        Some(value) if value.is_none() || value.unpack_str() == Some("target") => (false, None),
        Some(value) if value.unpack_str() == Some("exec") => (true, None),
        Some(value) => (
            false,
            Some(transition_definition_from_value(value).ok_or_else(|| {
                anyhow::anyhow!("attribute cfg must be 'target', 'exec', or a transition")
            })?),
        ),
    };
    definition.exec_configuration = exec_configuration;
    definition.transition = transition;
    Ok(())
}

fn coerce_label_default(
    value: Value<'_>,
    source: &BzlModuleIdentity,
) -> anyhow::Result<CoercedAttributeValue> {
    if let Some(label) = StarlarkLabel::from_value(value) {
        return Ok(CoercedAttributeValue::Label(label.canonical().clone()));
    }
    let raw = raw_attribute_value(value)?;
    coerce_raw_value(
        RawLabelContext::Definition(source),
        AttributeKind::Label,
        &raw,
    )
}

fn discard_attribute_doc(doc: Option<Value>) -> anyhow::Result<()> {
    if doc.is_some_and(|value| !value.is_none() && value.unpack_str().is_none()) {
        anyhow::bail!("attribute doc must be a string or None");
    }
    Ok(())
}

fn unpack_attribute_flags(flags: Option<Value<'_>>) -> anyhow::Result<AttributePropertyFlags> {
    let mut result = AttributePropertyFlags::default();
    result.insert(AttributePropertyFlag::StarlarkDefined);
    let Some(flags) = flags else {
        return Ok(result);
    };
    let values = if let Some(values) = ListRef::from_value(flags) {
        values.iter().collect::<Vec<_>>()
    } else if let Some(values) = TupleRef::from_value(flags) {
        values.iter().collect::<Vec<_>>()
    } else {
        anyhow::bail!("attribute flags must be a list or tuple of strings")
    };
    let names = values
        .into_iter()
        .map(|value| {
            value
                .unpack_str()
                .ok_or_else(|| anyhow::anyhow!("attribute flags must contain only strings"))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    for flag in names {
        let property = AttributePropertyFlag::from_name(flag)
            .ok_or_else(|| anyhow::anyhow!("unknown attribute flag '{flag}'"))?;
        result.insert(property);
    }
    Ok(result)
}

fn unpack_rule_class_admissibility(
    allow_rules: Option<Value<'_>>,
) -> anyhow::Result<RuleClassAdmissibility> {
    let Some(allow_rules) = allow_rules else {
        return Ok(RuleClassAdmissibility::Any);
    };
    if allow_rules.is_none() {
        return Ok(RuleClassAdmissibility::Any);
    }
    let values = if let Some(values) = ListRef::from_value(allow_rules) {
        values.iter().collect::<Vec<_>>()
    } else if let Some(values) = TupleRef::from_value(allow_rules) {
        values.iter().collect::<Vec<_>>()
    } else {
        anyhow::bail!("allow_rules must be a list or tuple of strings")
    };
    let classes = values
        .into_iter()
        .map(|value| {
            value
                .unpack_str()
                .map(CompactString::new)
                .ok_or_else(|| anyhow::anyhow!("allow_rules must contain only strings"))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(RuleClassAdmissibility::only(classes))
}

fn finalize_dependency_attribute_properties<V>(
    definition: &mut AttributeDefinitionGen<V>,
    mut flags: AttributePropertyFlags,
    for_dependency_resolution: Option<bool>,
    skip_validations: bool,
) {
    // Bazel mutates the raw set in this fixed order. These are deliberately
    // not a commutative union: explicit false removes the active dependency-
    // resolution bit after the raw flags have been installed.
    if definition.mandatory {
        flags.insert(AttributePropertyFlag::Mandatory);
    }
    if let Some(for_dependency_resolution) = for_dependency_resolution {
        flags.insert(AttributePropertyFlag::ForDependencyResolutionExplicitlySet);
        if for_dependency_resolution {
            flags.insert(AttributePropertyFlag::ForDependencyResolution);
        } else {
            flags.remove(AttributePropertyFlag::ForDependencyResolution);
        }
    }
    if definition.configurable_set {
        flags.insert(AttributePropertyFlag::ConfigurableAttrWasUserSet);
        if !definition.configurable {
            flags.insert(AttributePropertyFlag::Nonconfigurable);
        }
    }
    if skip_validations {
        flags.insert(AttributePropertyFlag::SkipValidations);
    }
    if !definition.allow_empty {
        flags.insert(AttributePropertyFlag::NonEmpty);
    }
    if definition.executable {
        flags.insert(AttributePropertyFlag::Executable);
    }
    // Every dependency-label descriptor installs NO_FILE even when neither
    // file keyword is supplied, and Attribute.Builder records that as strict.
    flags.insert(AttributePropertyFlag::StrictLabelChecking);
    if definition.file_admissibility.single_artifact() {
        flags.insert(AttributePropertyFlag::SingleArtifact);
    }
    if definition.transition.is_some() {
        flags.insert(AttributePropertyFlag::HasStarlarkDefinedTransition);
    }

    definition.mandatory |= flags.contains(AttributePropertyFlag::Mandatory);
    definition.executable |= flags.contains(AttributePropertyFlag::Executable);
    definition.allow_empty &= !flags.contains(AttributePropertyFlag::NonEmpty);
    definition.configurable &= !flags.contains(AttributePropertyFlag::Nonconfigurable);
    if flags.contains(AttributePropertyFlag::SingleArtifact) {
        definition.file_admissibility =
            definition.file_admissibility.clone().with_single_artifact();
    }
    definition.flags = flags;
}

fn unpack_file_admissibility<'v>(
    allow_files: Option<Value<'v>>,
    allow_single_file: Option<Value<'v>>,
) -> anyhow::Result<FileAdmissibility> {
    if allow_files.is_some_and(|value| !value.is_none())
        && allow_single_file.is_some_and(|value| !value.is_none())
    {
        anyhow::bail!("allow_files and allow_single_file cannot both be set");
    }
    let (value, name, single_artifact) = match (allow_files, allow_single_file) {
        (Some(value), _) if !value.is_none() => (value, "allow_files", false),
        (_, Some(value)) if !value.is_none() => (value, "allow_single_file", true),
        _ => return Ok(FileAdmissibility::default()),
    };
    if let Some(value) = value.unpack_bool() {
        let result = if value {
            FileAdmissibility::any_file()
        } else {
            FileAdmissibility::no_files()
        };
        return Ok(if single_artifact {
            result.with_single_artifact()
        } else {
            result
        });
    }
    let values = if let Some(values) = ListRef::from_value(value) {
        values.iter().collect::<Vec<_>>()
    } else if let Some(values) = TupleRef::from_value(value) {
        values.iter().collect::<Vec<_>>()
    } else {
        anyhow::bail!("{name} must be a bool or a sequence of file extensions")
    };
    let extensions = values
        .into_iter()
        .map(|value| {
            value
                .unpack_str()
                .map(CompactString::new)
                .ok_or_else(|| anyhow::anyhow!("{name} extensions must be strings"))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let result = FileAdmissibility::ordered_suffixes(extensions.into());
    Ok(if single_artifact {
        result.with_single_artifact()
    } else {
        result
    })
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
        #[starlark(require = named)] skip_validations: Option<bool>,
        #[starlark(require = named)] for_dependency_resolution: Option<bool>,
        #[starlark(require = named)] configurable: Option<bool>,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named)] cfg: Option<Value<'v>>,
        #[starlark(require = named)] allow_files: Option<Value<'v>>,
        #[starlark(require = named)] allow_single_file: Option<Value<'v>>,
        #[starlark(require = named)] allow_rules: Option<Value<'v>>,
        #[starlark(require = named)] providers: Option<Value<'v>>,
        #[starlark(require = named)] executable: Option<bool>,
        #[starlark(require = named)] doc: Option<Value<'v>>,
        #[starlark(require = named)] flags: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<AttributeDefinition<'v>> {
        discard_attribute_doc(doc)?;
        let mut definition = attribute_definition_before_later_properties(
            AttributeKind::Label,
            mandatory.unwrap_or(false),
            configurable,
            executable.unwrap_or(false),
            default,
            eval,
        )?;
        let flags = unpack_attribute_flags(flags)?;
        validate_executable_cfg_presence(definition.executable, cfg)?;
        definition.file_admissibility = unpack_file_admissibility(allow_files, allow_single_file)?;
        definition.rule_class_admissibility = unpack_rule_class_admissibility(allow_rules)?;
        definition.required_providers = label_required_provider(providers)?;
        set_attribute_cfg(&mut definition, cfg)?;
        finalize_dependency_attribute_properties(
            &mut definition,
            flags,
            for_dependency_resolution,
            skip_validations.unwrap_or(false),
        );
        Ok(definition)
    }
    fn label_list<'v>(
        #[starlark(this)] _attr: Value<'v>,
        #[starlark(require = named)] mandatory: Option<bool>,
        #[starlark(require = named)] skip_validations: Option<bool>,
        #[starlark(require = named)] for_dependency_resolution: Option<bool>,
        #[starlark(require = named)] configurable: Option<bool>,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named)] doc: Option<Value<'v>>,
        #[starlark(require = named)] allow_empty: Option<bool>,
        #[starlark(require = named)] allow_files: Option<Value<'v>>,
        #[starlark(require = named)] allow_rules: Option<Value<'v>>,
        #[starlark(require = named)] providers: Option<Value<'v>>,
        #[starlark(require = named)] cfg: Option<Value<'v>>,
        #[starlark(require = named)] aspects: Option<Value<'v>>,
        #[starlark(require = named)] flags: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<AttributeDefinition<'v>> {
        discard_attribute_doc(doc)?;
        let mut definition = attribute_definition_before_later_properties(
            AttributeKind::LabelList,
            mandatory.unwrap_or(false),
            configurable,
            false,
            default,
            eval,
        )?;
        definition.allow_empty = allow_empty.unwrap_or(true);
        let flags = unpack_attribute_flags(flags)?;
        definition.file_admissibility = unpack_file_admissibility(allow_files, None)?;
        definition.rule_class_admissibility = unpack_rule_class_admissibility(allow_rules)?;
        definition.required_providers = label_list_required_providers(providers)?;
        set_attribute_cfg(&mut definition, cfg)?;
        definition.attached_aspect = label_list_attached_aspect(aspects)?;
        finalize_dependency_attribute_properties(
            &mut definition,
            flags,
            for_dependency_resolution,
            skip_validations.unwrap_or(false),
        );
        Ok(definition)
    }
    fn string_keyed_label_dict<'v>(
        #[starlark(this)] _attr: Value<'v>,
        #[starlark(require = named)] mandatory: Option<bool>,
        #[starlark(require = named)] for_dependency_resolution: Option<bool>,
        #[starlark(require = named)] configurable: Option<bool>,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named)] doc: Option<Value<'v>>,
        #[starlark(require = named)] allow_empty: Option<bool>,
        #[starlark(require = named)] allow_files: Option<Value<'v>>,
        #[starlark(require = named)] allow_rules: Option<Value<'v>>,
        #[starlark(require = named)] providers: Option<Value<'v>>,
        #[starlark(require = named)] cfg: Option<Value<'v>>,
        #[starlark(require = named)] flags: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<AttributeDefinition<'v>> {
        discard_attribute_doc(doc)?;
        let mut definition = attribute_definition_before_later_properties(
            AttributeKind::StringKeyedLabelDict,
            mandatory.unwrap_or(false),
            configurable,
            false,
            default,
            eval,
        )?;
        definition.allow_empty = allow_empty.unwrap_or(true);
        let flags = unpack_attribute_flags(flags)?;
        definition.file_admissibility = unpack_file_admissibility(allow_files, None)?;
        definition.rule_class_admissibility = unpack_rule_class_admissibility(allow_rules)?;
        definition.required_providers =
            declaration_required_providers(providers, "attribute providers")?;
        set_attribute_cfg(&mut definition, cfg)?;
        finalize_dependency_attribute_properties(
            &mut definition,
            flags,
            for_dependency_resolution,
            false,
        );
        Ok(definition)
    }
    fn label_keyed_string_dict<'v>(
        #[starlark(this)] _attr: Value<'v>,
        #[starlark(require = named)] mandatory: Option<bool>,
        #[starlark(require = named)] skip_validations: Option<bool>,
        #[starlark(require = named)] for_dependency_resolution: Option<bool>,
        #[starlark(require = named)] configurable: Option<bool>,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named)] doc: Option<Value<'v>>,
        #[starlark(require = named)] allow_empty: Option<bool>,
        #[starlark(require = named)] allow_files: Option<Value<'v>>,
        #[starlark(require = named)] allow_rules: Option<Value<'v>>,
        #[starlark(require = named)] providers: Option<Value<'v>>,
        #[starlark(require = named)] cfg: Option<Value<'v>>,
        #[starlark(require = named)] flags: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<AttributeDefinition<'v>> {
        discard_attribute_doc(doc)?;
        let mut definition = attribute_definition_before_later_properties(
            AttributeKind::LabelKeyedStringDict,
            mandatory.unwrap_or(false),
            configurable,
            false,
            default,
            eval,
        )?;
        definition.allow_empty = allow_empty.unwrap_or(true);
        let flags = unpack_attribute_flags(flags)?;
        definition.file_admissibility = unpack_file_admissibility(allow_files, None)?;
        definition.rule_class_admissibility = unpack_rule_class_admissibility(allow_rules)?;
        definition.required_providers =
            declaration_required_providers(providers, "attribute providers")?;
        set_attribute_cfg(&mut definition, cfg)?;
        finalize_dependency_attribute_properties(
            &mut definition,
            flags,
            for_dependency_resolution,
            skip_validations.unwrap_or(false),
        );
        Ok(definition)
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
            FileAdmissibility::default(),
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
            FileAdmissibility::default(),
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
    fn int_list<'v>(
        #[starlark(this)] _attr: Value<'v>,
        #[starlark(require = named)] mandatory: Option<bool>,
        #[starlark(require = named)] configurable: Option<bool>,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named)] doc: Option<Value<'v>>,
        #[starlark(require = named)] allow_empty: Option<bool>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<AttributeDefinition<'v>> {
        discard_attribute_doc(doc)?;
        let mut definition = attribute_definition(
            AttributeKind::IntegerList,
            mandatory.unwrap_or(false),
            configurable,
            FileAdmissibility::default(),
            false,
            default,
            None,
            eval,
        )?;
        definition.allow_empty = allow_empty.unwrap_or(true);
        Ok(definition)
    }
    fn label_list_dict<'v>(
        #[starlark(this)] _attr: Value<'v>,
        #[starlark(require = named)] mandatory: Option<bool>,
        #[starlark(require = named)] skip_validations: Option<bool>,
        #[starlark(require = named)] for_dependency_resolution: Option<bool>,
        #[starlark(require = named)] configurable: Option<bool>,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named)] doc: Option<Value<'v>>,
        #[starlark(require = named)] allow_empty: Option<bool>,
        #[starlark(require = named)] allow_files: Option<Value<'v>>,
        #[starlark(require = named)] allow_rules: Option<Value<'v>>,
        #[starlark(require = named)] providers: Option<Value<'v>>,
        #[starlark(require = named)] cfg: Option<Value<'v>>,
        #[starlark(require = named)] flags: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<AttributeDefinition<'v>> {
        discard_attribute_doc(doc)?;
        let mut definition = attribute_definition_before_later_properties(
            AttributeKind::LabelListDict,
            mandatory.unwrap_or(false),
            configurable,
            false,
            default,
            eval,
        )?;
        definition.allow_empty = allow_empty.unwrap_or(true);
        let flags = unpack_attribute_flags(flags)?;
        definition.file_admissibility = unpack_file_admissibility(allow_files, None)?;
        definition.rule_class_admissibility = unpack_rule_class_admissibility(allow_rules)?;
        definition.required_providers =
            declaration_required_providers(providers, "attribute providers")?;
        set_attribute_cfg(&mut definition, cfg)?;
        finalize_dependency_attribute_properties(
            &mut definition,
            flags,
            for_dependency_resolution,
            skip_validations.unwrap_or(false),
        );
        Ok(definition)
    }
    fn output<'v>(
        #[starlark(this)] _attr: Value<'v>,
        #[starlark(require = named)] doc: Option<Value<'v>>,
        #[starlark(require = named)] mandatory: Option<bool>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<AttributeDefinition<'v>> {
        discard_attribute_doc(doc)?;
        attribute_definition(
            AttributeKind::Output,
            mandatory.unwrap_or(false),
            None,
            FileAdmissibility::default(),
            false,
            None,
            None,
            eval,
        )
    }
    fn output_list<'v>(
        #[starlark(this)] _attr: Value<'v>,
        #[starlark(require = named)] doc: Option<Value<'v>>,
        #[starlark(require = named)] mandatory: Option<bool>,
        #[starlark(require = named)] allow_empty: Option<bool>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<AttributeDefinition<'v>> {
        discard_attribute_doc(doc)?;
        let mut definition = attribute_definition(
            AttributeKind::OutputList,
            mandatory.unwrap_or(false),
            None,
            FileAdmissibility::default(),
            false,
            None,
            None,
            eval,
        )?;
        definition.allow_empty = allow_empty.unwrap_or(true);
        Ok(definition)
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
            FileAdmissibility::default(),
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
        #[starlark(require = named)] allow_empty: Option<bool>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<AttributeDefinition<'v>> {
        discard_attribute_doc(doc)?;
        let mut definition = attribute_definition(
            AttributeKind::StringList,
            mandatory.unwrap_or(false),
            configurable,
            FileAdmissibility::default(),
            false,
            default,
            None,
            eval,
        )?;
        definition.allow_empty = allow_empty.unwrap_or(true);
        Ok(definition)
    }
    fn string_dict<'v>(
        #[starlark(this)] _attr: Value<'v>,
        #[starlark(require = named)] mandatory: Option<bool>,
        #[starlark(require = named)] configurable: Option<bool>,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named)] doc: Option<Value<'v>>,
        #[starlark(require = named)] allow_empty: Option<bool>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<AttributeDefinition<'v>> {
        discard_attribute_doc(doc)?;
        let mut definition = attribute_definition(
            AttributeKind::StringDict,
            mandatory.unwrap_or(false),
            configurable,
            FileAdmissibility::default(),
            false,
            default,
            None,
            eval,
        )?;
        definition.allow_empty = allow_empty.unwrap_or(true);
        Ok(definition)
    }
    fn string_list_dict<'v>(
        #[starlark(this)] _attr: Value<'v>,
        #[starlark(require = named)] mandatory: Option<bool>,
        #[starlark(require = named)] configurable: Option<bool>,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named)] doc: Option<Value<'v>>,
        #[starlark(require = named)] allow_empty: Option<bool>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<AttributeDefinition<'v>> {
        discard_attribute_doc(doc)?;
        let mut definition = attribute_definition(
            AttributeKind::StringListDict,
            mandatory.unwrap_or(false),
            configurable,
            FileAdmissibility::default(),
            false,
            default,
            None,
            eval,
        )?;
        definition.allow_empty = allow_empty.unwrap_or(true);
        Ok(definition)
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

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct RootStringSetBuildSetting {
    flag: bool,
    repeatable: bool,
}
starlark::starlark_simple_value!(RootStringSetBuildSetting);
impl fmt::Display for RootStringSetBuildSetting {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("config.string_set")
    }
}
#[starlark_value(type = "config_string_set")]
impl<'v> StarlarkValue<'v> for RootStringSetBuildSetting {}

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

    fn string_set(
        #[starlark(this)] _config: Value,
        #[starlark(require = named, default = false)] flag: bool,
        #[starlark(require = named, default = false)] repeatable: bool,
    ) -> anyhow::Result<RootStringSetBuildSetting> {
        if repeatable && !flag {
            anyhow::bail!("'repeatable' can only be set for a setting with 'flag = True'")
        }
        Ok(RootStringSetBuildSetting { flag, repeatable })
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
        match attribute {
            "ToolchainInfo" => {
                Some(heap.alloc_simple(AnalysisBuiltinCallable::new("ToolchainInfo")))
            }
            "TemplateVariableInfo" => {
                Some(heap.alloc_simple(AnalysisBuiltinCallable::new("TemplateVariableInfo")))
            }
            _ => None,
        }
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
        let visibility = names.get("visibility").copied();
        let implementation = self.implementation;
        let definition_source = self.definition_source.clone();
        let source_identities_by_filename = self.source_identities_by_filename.clone();
        let required_toolchains = self.required_toolchains.clone();
        let advertised_providers = self.advertised_providers.clone();
        let required_fragments = self.required_fragments.clone();
        let attached_subrules = self.attached_subrules.clone();
        let subrule_callables = self.subrule_callables.clone();
        let late_bound_attributes = self.late_bound_attributes.clone();
        let capability = self.capability.clone();
        let incoming_transition = self.incoming_transition.as_ref().map(|transition| {
            LoadingTransitionDefinition::new(
                transition.implementation,
                transition.inputs.clone(),
                transition.outputs.clone(),
                transition.definition_source.clone(),
                transition.source_identities_by_filename.clone(),
            )
        });
        let heap = eval.heap();
        PackageRecorder::from_evaluator(eval)
            .and_then(|recorder| {
                let visibility = visibility
                    .map(|value| parse_rule_visibility_argument(recorder, value))
                    .transpose()?;
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
                    .cloned()
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
                        let dependency_configuration = match (
                            declaration.exec_configuration,
                            declaration.transition.as_ref(),
                        ) {
                            (false, None) => AttributeDependencyConfiguration::Target,
                            (true, None) => AttributeDependencyConfiguration::Exec,
                            (false, Some(transition)) => {
                                AttributeDependencyConfiguration::Starlark(
                                    LoadingTransitionDefinition::new(
                                        transition.implementation,
                                        transition.inputs.clone(),
                                        transition.outputs.clone(),
                                        transition.definition_source.clone(),
                                        transition.source_identities_by_filename.clone(),
                                    ),
                                )
                            }
                            (true, Some(_)) => anyhow::bail!(
                                "attribute '{}' cannot combine cfg='exec' with a Starlark transition",
                                declaration.name
                            ),
                        };
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
                        )
                        .with_dependency_configuration(
                            dependency_configuration,
                            declaration.executable,
                        )
                        .with_file_admissibility(declaration.file_admissibility.clone())
                        .with_flags(declaration.flags)
                        .with_rule_class_admissibility(
                            declaration.rule_class_admissibility.clone(),
                        )
                        .with_allowed_values(declaration.allowed_values.clone())
                        .with_allow_empty(declaration.allow_empty)
                        .with_required_providers(declaration.required_providers.clone())
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
                        Some(value)
                            if builtin
                                && declaration.name == "build_setting_default"
                                && matches!(
                                    self.build_setting_definition,
                                    Some(BuildSettingDefinition::StringSet { .. })
                                ) =>
                        {
                            (
                                AttributeProvenance::Explicit,
                                coerce_string_set_default(value, heap)?,
                            )
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
                let resolved_outputs = match &self.outputs {
                    RuleOutputsDefinitionGen::Static(entries) => {
                        resolve_output_names(entries, name, &values)
                    }
                    RuleOutputsDefinitionGen::Callback(callback) => {
                        let entries =
                            invoke_rule_outputs_callback(*callback, &values, recorder, self)?;
                        resolve_output_names(&entries, name, &values)
                    }
                }?;
                let predeclared_outputs: Arc<[PredeclaredOutput]> = resolved_outputs.into_iter()
                .map(|(key, output_name)| {
                    recorder
                        .output_label(&output_name)
                        .map(|label| PredeclaredOutput { key, label })
                })
                .collect::<anyhow::Result<Vec<_>>>()?
                .into();
                if let Some(output) = predeclared_outputs.iter().find(|output| {
                    values.iter().any(|attribute| {
                        attribute.declaration_name == output.key
                            && match attribute.value.as_ref() {
                            CoercedAttributeValue::Output(_) => true,
                            CoercedAttributeValue::OutputList(labels) => !labels.is_empty(),
                            _ => false,
                        }
                    })
                }) {
                    anyhow::bail!("multiple outputs with the same key: {}", output.key);
                }
                let generated = predeclared_outputs
                    .iter()
                    .map(|output| output.label.clone())
                    .chain(generated)
                    .collect::<Vec<_>>();
                recorder.starlark_rule(
                    name.to_owned(),
                    implementation,
                    definition_source,
                    source_identities_by_filename,
                    required_toolchains,
                    advertised_providers,
                    required_fragments,
                    attached_subrules,
                    subrule_callables,
                    late_bound_attributes,
                    capability,
                    schema,
                    values,
                    self.build_setting_definition,
                    incoming_transition,
                    predeclared_outputs,
                    self.output_to_genfiles,
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

fn allocate_macro_attribute<'v>(
    value: &CoercedAttributeValue,
    heap: Heap<'v>,
) -> anyhow::Result<Value<'v>> {
    let label = |value: &CanonicalLabel| heap.alloc_simple(StarlarkLabel::new(value.clone()));
    Ok(match value {
        CoercedAttributeValue::None => Value::new_none(),
        CoercedAttributeValue::Boolean(value) => Value::new_bool(*value),
        CoercedAttributeValue::Integer(value) => heap.alloc(*value),
        CoercedAttributeValue::IntegerList(values) => heap.alloc(AllocList(values.iter().copied())),
        CoercedAttributeValue::String(value) => heap.alloc_str(value).to_value(),
        CoercedAttributeValue::Label(value) | CoercedAttributeValue::Output(value) => label(value),
        CoercedAttributeValue::StringList(values) => {
            heap.alloc(AllocList(values.iter().map(|value| value.as_str())))
        }
        CoercedAttributeValue::LabelList(values) | CoercedAttributeValue::OutputList(values) => {
            heap.alloc(AllocList(values.iter().map(label)))
        }
        CoercedAttributeValue::StringDict(values) => heap.alloc(AllocDict(
            values
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_str())),
        )),
        CoercedAttributeValue::StringListDict(values) => {
            heap.alloc(AllocDict(values.iter().map(|(key, values)| {
                (
                    key.as_str(),
                    heap.alloc(AllocList(values.iter().map(|value| value.as_str()))),
                )
            })))
        }
        CoercedAttributeValue::StringKeyedLabelDict(values) => heap.alloc(AllocDict(
            values
                .iter()
                .map(|(key, value)| (key.as_str(), label(value))),
        )),
        CoercedAttributeValue::LabelKeyedStringDict(values) => heap.alloc(AllocDict(
            values
                .iter()
                .map(|(key, value)| (label(key), value.as_str())),
        )),
        CoercedAttributeValue::LabelListDict(values) => {
            heap.alloc(AllocDict(values.iter().map(|(key, values)| {
                (
                    key.as_str(),
                    heap.alloc(AllocList(values.iter().map(label))),
                )
            })))
        }
        CoercedAttributeValue::Selector { branches, default } => {
            let mut selector_branches = branches
                .iter()
                .map(|(condition, value)| {
                    Ok(SelectorBranchGen {
                        condition: SelectorCondition::Canonical(condition.to_string()),
                        value: allocate_macro_attribute(value, heap)?,
                    })
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            if let Some(default) = default {
                selector_branches.push(SelectorBranchGen {
                    condition: SelectorCondition::Raw("//conditions:default".into()),
                    value: allocate_macro_attribute(default, heap)?,
                });
            }
            heap.alloc(SelectorValueGen {
                parts: vec![SelectorPartGen {
                    prefix: Vec::new(),
                    suffix: Vec::new(),
                    branches: selector_branches,
                }],
            })
        }
        CoercedAttributeValue::Concatenation(left, right) => {
            let left = allocate_macro_attribute(left, heap)?;
            let right = allocate_macro_attribute(right, heap)?;
            if let Some(selector) = SelectorValue::from_value(left) {
                match selector {
                    starlark::__macro_refs::Either::Left(selector) => selector
                        .add(right, heap)
                        .transpose()
                        .map_err(|error| anyhow::anyhow!(error.to_string()))?
                        .ok_or_else(|| anyhow::anyhow!("invalid macro selector concatenation"))?,
                    starlark::__macro_refs::Either::Right(_) => {
                        anyhow::bail!("unexpected frozen selector in fresh macro evaluator")
                    }
                }
            } else if let Some(selector) = SelectorValue::from_value(right) {
                match selector {
                    starlark::__macro_refs::Either::Left(selector) => selector
                        .radd(left, heap)
                        .transpose()
                        .map_err(|error| anyhow::anyhow!(error.to_string()))?
                        .ok_or_else(|| anyhow::anyhow!("invalid macro selector concatenation"))?,
                    starlark::__macro_refs::Either::Right(_) => {
                        anyhow::bail!("unexpected frozen selector in fresh macro evaluator")
                    }
                }
            } else {
                anyhow::bail!("macro concatenation lost its selector")
            }
        }
    })
}

fn invoke_rule_outputs_callback(
    callback: FrozenValue,
    attributes: &[AttributeValue],
    recorder: &PackageRecorder,
    definition: &FrozenRuleDefinition,
) -> anyhow::Result<Vec<(CompactString, CompactString)>> {
    let DocItem::Member(DocMember::Function(documentation)) = callback.to_value().documentation()
    else {
        anyhow::bail!("rule outputs callback must be a Starlark function");
    };
    let module = starlark::environment::Module::new();
    let unavailable = |name: &str| {
        anyhow::anyhow!(
            "Attribute '{name}' either doesn't exist or uses a select() (i.e. could have multiple values)"
        )
    };
    let arguments = documentation
        .params
        .regular_params()
        .chain(documentation.params.args.iter())
        .chain(documentation.params.kwargs.iter())
        .map(|parameter| {
            use CoercedAttributeValue::Concatenation;
            use CoercedAttributeValue::Selector;
            let attribute = attributes
                .iter()
                .find(|attribute| attribute.declaration_name == parameter.name)
                .filter(|attribute| {
                    !matches!(
                        attribute.value.as_ref(),
                        Selector { .. } | Concatenation(_, _)
                    )
                })
                .ok_or_else(|| unavailable(&parameter.name))?;
            allocate_macro_attribute(attribute.value.as_ref(), module.heap())
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let bzl = BzlEvaluationContext::macro_runtime_context(
        (*definition.definition_source).clone(),
        definition.source_identities_by_filename.clone(),
    );
    let context = MacroEvaluationContext { recorder, bzl };
    let result = {
        let mut evaluator = Evaluator::new(&module);
        evaluator.extra = Some(&context);
        if let Some(capture) = recorder.print_capture() {
            evaluator.set_print_handler(capture);
        }
        evaluator.eval_function(callback.to_value(), &arguments, &[])
    }
    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let result = DictRef::from_value(result)
        .ok_or_else(|| anyhow::anyhow!("implicit outputs function return value must be a dict"))?;
    output_string_pairs(result, "implicit outputs function return value")
}

fn allocate_configurable_macro_attribute<'v>(
    value: &CoercedAttributeValue,
    configurable: bool,
    heap: Heap<'v>,
) -> anyhow::Result<Value<'v>> {
    let value = allocate_macro_attribute(value, heap)?;
    if !configurable || value.is_none() || SelectorValue::from_value(value).is_some() {
        return Ok(value);
    }
    Ok(heap.alloc(SelectorValueGen {
        parts: vec![SelectorPartGen {
            prefix: Vec::new(),
            suffix: Vec::new(),
            branches: vec![SelectorBranchGen {
                condition: SelectorCondition::Raw("//conditions:default".into()),
                value,
            }],
        }],
    }))
}

fn visibility_argument<'v>(visibility: &RuleVisibility, heap: Heap<'v>) -> Value<'v> {
    let labels: Vec<CanonicalLabel> = match visibility {
        RuleVisibility::Public => vec![CanonicalLabel::parse("@@//visibility:public").unwrap()],
        RuleVisibility::Private => vec![CanonicalLabel::parse("@@//visibility:private").unwrap()],
        RuleVisibility::Restricted(value) => value.declared_labels().to_vec(),
    };
    heap.alloc(AllocList(
        labels
            .into_iter()
            .map(|label| heap.alloc_simple(StarlarkLabel::new(label))),
    ))
}

fn parse_macro_visibility(
    recorder: &PackageRecorder,
    value: Option<Value<'_>>,
    parent: Option<&MacroInstanceRecord>,
) -> anyhow::Result<RuleVisibility> {
    let parsed = match value.filter(|value| !value.is_none()) {
        Some(value) => {
            let values = ListRef::from_value(value)
                .ok_or_else(|| anyhow::anyhow!("macro visibility must be a list of labels"))?;
            let labels = values
                .iter()
                .map(|value| {
                    if let Some(label) = StarlarkLabel::from_value(value) {
                        Ok(label.canonical().clone())
                    } else {
                        value
                            .unpack_str()
                            .ok_or_else(|| anyhow::anyhow!("macro visibility must contain labels"))
                            .and_then(|value| recorder.dependency_label(value))
                    }
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            RuleVisibility::from_declared_labels(labels)?
        }
        None if parent.is_some() => RuleVisibility::Private,
        None => recorder.state.borrow().default_visibility.clone(),
    };
    let instantiating_package = parent
        .map(|parent| parent.definition.defining_label.package())
        .unwrap_or(&recorder.package_identifier);
    concat_visibility_with_package(&parsed, instantiating_package)
}

fn parse_rule_visibility_argument(
    recorder: &PackageRecorder,
    value: Value<'_>,
) -> anyhow::Result<RuleVisibility> {
    let values = ListRef::from_value(value)
        .ok_or_else(|| anyhow::anyhow!("attribute `visibility` must be a list of labels"))?;
    let labels = values
        .iter()
        .map(|value| {
            if let Some(label) = StarlarkLabel::from_value(value) {
                Ok(label.canonical().clone())
            } else {
                value
                    .unpack_str()
                    .ok_or_else(|| {
                        anyhow::anyhow!("attribute `visibility` must be a list of labels")
                    })
                    .and_then(|value| recorder.dependency_label(value))
            }
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    RuleVisibility::from_declared_labels(labels)
}

fn invoke_symbolic_macro<'v>(
    definition: &FrozenSymbolicMacroDefinition,
    args: &Arguments<'v, '_>,
    eval: &mut Evaluator<'v, '_, '_>,
) -> starlark::Result<Value<'v>> {
    args.no_positional_args(eval.heap())?;
    let names = args.names_map()?;
    let recorder = PackageRecorder::from_evaluator(eval).map_err(starlark::Error::new_other)?;
    let name = names
        .get("name")
        .and_then(|value| value.unpack_str())
        .ok_or_else(|| {
            starlark::Error::new_other(anyhow::anyhow!(
                "missing value for mandatory attribute 'name' in '{}' macro",
                definition.exported_name
            ))
        })?
        .to_owned();
    for attribute in names.keys() {
        if attribute.as_str() != "name"
            && attribute.as_str() != "visibility"
            && !definition
                .attributes
                .iter()
                .any(|schema| schema.name == attribute.as_str())
        {
            return Err(starlark::Error::new_other(anyhow::anyhow!(
                "no such attribute '{}' in '{}' macro",
                attribute.as_str(),
                definition.exported_name
            )));
        }
    }

    let (parent_index, parent) = {
        let state = recorder.state.borrow();
        (
            state.active_macro,
            state
                .active_macro
                .map(|index| state.macro_instances[index].clone()),
        )
    };
    if let Some(parent) = &parent {
        if !name_is_within_macro_namespace(&name, &parent.name) {
            return Err(starlark::Error::new_other(anyhow::anyhow!(
                "macro '{}' cannot declare submacro named '{}'",
                parent.name,
                name
            )));
        }
    }
    let identity = MacroDefinitionIdentity {
        defining_label: definition.definition_source.label.clone(),
        exported_name: definition.exported_name.clone(),
    };
    {
        let state = recorder.state.borrow();
        let mut ancestor = state.active_macro;
        while let Some(index) = ancestor {
            let instance = &state.macro_instances[index];
            if instance.definition == identity {
                return Err(starlark::Error::new_other(anyhow::anyhow!(
                    "recursive call to symbolic macro '{}'",
                    definition.exported_name
                )));
            }
            ancestor = instance.parent.map(|index| index as usize);
        }
        if state.targets.contains_key(&name) {
            return Err(starlark::Error::new_other(anyhow::anyhow!(
                "macro '{name}' conflicts with an existing target"
            )));
        }
        if state.macro_instances.iter().any(|instance| {
            instance.name == name
                && !(parent.as_ref().is_some_and(|parent| parent.name == name)
                    && instance.parent != parent_index.map(|index| index as u32))
        }) {
            return Err(starlark::Error::new_other(anyhow::anyhow!(
                "macro '{name}' conflicts with an existing macro"
            )));
        }
    }
    let visibility =
        parse_macro_visibility(recorder, names.get("visibility").copied(), parent.as_ref())
            .map_err(starlark::Error::new_other)?;
    let same_name_depth = parent
        .as_ref()
        .filter(|parent| parent.name == name)
        .map_or(1, |parent| parent.same_name_depth + 1);
    let (generator_name, generator_function, generator_location) =
        symbolic_macro_generator_metadata(recorder, eval, &name, &definition.exported_name);
    let macro_index = {
        let mut state = recorder.state.borrow_mut();
        let index = state.macro_instances.len();
        state.macro_instances.push(MacroInstanceRecord {
            identity: format!("{name}:{same_name_depth}").into(),
            name: name.clone().into(),
            same_name_depth,
            parent: parent_index.map(|index| index as u32),
            definition: identity,
            visibility: visibility.clone(),
            generator_name,
            generator_function,
            generator_location,
        });
        index
    };

    let module = starlark::environment::Module::new();
    let bzl = BzlEvaluationContext::macro_runtime_context(
        (*definition.definition_source).clone(),
        definition.source_identities_by_filename.clone(),
    );
    let context = MacroEvaluationContext { recorder, bzl };
    let result = {
        let mut macro_eval = Evaluator::new(&module);
        macro_eval.extra = Some(&context);
        if let Some(capture) = recorder.print_capture() {
            macro_eval.set_print_handler(capture);
        }
        let mut argument_names = Vec::with_capacity(definition.attributes.len() + 2);
        let mut argument_values = Vec::with_capacity(definition.attributes.len() + 2);
        argument_names.push(CompactString::const_new("name"));
        argument_values.push(module.heap().alloc_str(&name).to_value());
        argument_names.push(CompactString::const_new("visibility"));
        argument_values.push(visibility_argument(&visibility, module.heap()));
        for schema in definition.attributes.iter() {
            let explicit = names.get(schema.name.as_str()).copied();
            let coerced = match explicit.filter(|value| !value.is_none()) {
                Some(value) => Some(
                    coerce_starlark_value(
                        recorder,
                        schema.kind,
                        &schema.name,
                        schema.configurable,
                        value,
                    )
                    .and_then(|value| {
                        validate_allowed_value(&schema.name, &value, &schema.allowed_values)?;
                        Ok(normalize_starlark_value(
                            value,
                            schema
                                .flags
                                .contains(AttributePropertyFlag::OrderIndependent),
                        ))
                    })
                    .map_err(starlark::Error::new_other)?,
                ),
                None if schema.mandatory => {
                    return Err(starlark::Error::new_other(anyhow::anyhow!(
                        "missing value for mandatory attribute '{}'",
                        schema.name
                    )));
                }
                None if schema.default_to_none => None,
                None => Some(
                    schema
                        .default
                        .clone()
                        .unwrap_or_else(|| intrinsic_default(schema.kind)),
                ),
            };
            argument_names.push(schema.name.clone());
            argument_values.push(match coerced {
                Some(value) => allocate_configurable_macro_attribute(
                    &value,
                    schema.configurable,
                    module.heap(),
                )
                .map_err(starlark::Error::new_other)?,
                None => Value::new_none(),
            });
        }
        let named = argument_names
            .iter()
            .zip(argument_values.iter().copied())
            .map(|(name, value)| (name.as_str(), value))
            .collect::<Vec<_>>();
        recorder.state.borrow_mut().active_macro = Some(macro_index);
        let result = macro_eval.eval_function(definition.implementation.to_value(), &[], &named);
        recorder.state.borrow_mut().active_macro = parent_index;
        result
    };
    let value = result?;
    if !value.is_none() {
        return Err(starlark::Error::new_other(anyhow::anyhow!(
            "macro '{}' may not return a non-None value (got {})",
            name,
            value.to_repr()
        )));
    }
    Ok(Value::new_none())
}

fn repository_rule_default_matches(kind: AttributeKind, value: &CoercedAttributeValue) -> bool {
    matches!(
        (kind, value),
        (AttributeKind::String, CoercedAttributeValue::String(_))
            | (AttributeKind::Boolean, CoercedAttributeValue::Boolean(_))
            | (AttributeKind::Integer, CoercedAttributeValue::Integer(_))
            | (
                AttributeKind::IntegerList,
                CoercedAttributeValue::IntegerList(_)
            )
            | (
                AttributeKind::Label,
                CoercedAttributeValue::Label(_) | CoercedAttributeValue::None
            )
            | (AttributeKind::Output, CoercedAttributeValue::Output(_))
            | (
                AttributeKind::StringList,
                CoercedAttributeValue::StringList(_)
            )
            | (
                AttributeKind::LabelList,
                CoercedAttributeValue::LabelList(_)
            )
            | (
                AttributeKind::OutputList,
                CoercedAttributeValue::OutputList(_)
            )
            | (
                AttributeKind::StringDict,
                CoercedAttributeValue::StringDict(_)
            )
            | (
                AttributeKind::StringListDict,
                CoercedAttributeValue::StringListDict(_)
            )
            | (
                AttributeKind::StringKeyedLabelDict,
                CoercedAttributeValue::StringKeyedLabelDict(_)
            )
            | (
                AttributeKind::LabelKeyedStringDict,
                CoercedAttributeValue::LabelKeyedStringDict(_)
            )
            | (
                AttributeKind::LabelListDict,
                CoercedAttributeValue::LabelListDict(_)
            )
    )
}

fn symbolic_macro_global<'v>(
    implementation: Value<'v>,
    attrs: Option<Value<'v>>,
    inherit_attrs: Option<Value<'v>>,
    finalizer: bool,
    doc: Option<Value<'v>>,
    eval: &mut Evaluator<'v, '_, '_>,
) -> anyhow::Result<SymbolicMacroDefinition<'v>> {
    let implementation_parameters = implementation
        .parameters_spec()
        .ok_or_else(|| anyhow::anyhow!("macro implementation must be a Starlark function"))?;
    if finalizer {
        anyhow::bail!("symbolic macro finalizers are not supported in this packet");
    }
    let documentation = match doc.filter(|value| !value.is_none()) {
        Some(value) => Some(
            value
                .unpack_str()
                .ok_or_else(|| anyhow::anyhow!("macro doc must be a string or None"))?
                .into(),
        ),
        None => None,
    };
    let attrs = match attrs.filter(|value| !value.is_none()) {
        Some(value) => DictRef::from_value(value)
            .ok_or_else(|| anyhow::anyhow!("macro attrs must be a dict or None"))?
            .iter()
            .map(|(name, value)| {
                Ok((
                    name.unpack_str()
                        .ok_or_else(|| anyhow::anyhow!("macro attr names must be strings"))?
                        .to_owned(),
                    value,
                ))
            })
            .collect::<anyhow::Result<Vec<_>>>()?,
        None => Vec::new(),
    };
    let mut attributes = Vec::new();
    for (name, value) in &attrs {
        if matches!(name.as_str(), "name" | "visibility") {
            anyhow::bail!("Cannot declare a macro attribute named '{name}'");
        }
        if name.starts_with('_') {
            anyhow::bail!("macro attribute '{name}' must be public");
        }
        if value.is_none() {
            continue;
        }
        let definition = attribute_definition_from_value(*value)?
            .ok_or_else(|| anyhow::anyhow!("macro attribute '{name}' must use attr.*()"))?;
        attributes.push(MacroAttributeSchema::from_definition(name, &definition)?);
    }
    let explicit_names = attrs
        .iter()
        .map(|(name, _)| name.as_str())
        .collect::<Vec<_>>();
    let inherit_attrs = inherit_attrs.filter(|value| !value.is_none());
    if inherit_attrs.is_some()
        && !implementation_parameters
            .parameters_str()
            .split(',')
            .any(|parameter| parameter.trim_start().starts_with("**"))
    {
        anyhow::bail!("macro implementation must have a **kwargs parameter to inherit attributes");
    }
    if let Some(inherit) = inherit_attrs {
        let inherited = if inherit.unpack_str() == Some("common") {
            starlark_builtin_schema::<Value<'v>>(false, false, None, false)
                .iter()
                .filter_map(MacroAttributeSchema::inherited_transient)
                .collect::<Vec<_>>()
        } else if let Some(rule) = RuleDefinition::from_value(inherit) {
            match rule {
                starlark::__macro_refs::Either::Left(rule) => {
                    if rule.rule_class.get().is_none() {
                        anyhow::bail!("inherit_attrs rule must be exported");
                    }
                    rule.schema
                        .iter()
                        .filter_map(MacroAttributeSchema::inherited_transient)
                        .collect()
                }
                starlark::__macro_refs::Either::Right(rule) => rule
                    .schema
                    .iter()
                    .filter_map(MacroAttributeSchema::inherited)
                    .collect(),
            }
        } else if let Some(symbolic_macro) = SymbolicMacroDefinition::from_value(inherit) {
            match symbolic_macro {
                starlark::__macro_refs::Either::Left(symbolic_macro) => {
                    if symbolic_macro.exported_name.get().is_none() {
                        anyhow::bail!("inherit_attrs macro must be exported");
                    }
                    symbolic_macro
                        .attributes
                        .iter()
                        .map(MacroAttributeSchema::inherited_macro)
                        .collect()
                }
                starlark::__macro_refs::Either::Right(symbolic_macro) => symbolic_macro
                    .attributes
                    .iter()
                    .map(MacroAttributeSchema::inherited_macro)
                    .collect(),
            }
        } else {
            anyhow::bail!(
                "invalid inherit_attrs value; expected an exported rule, macro, or 'common'"
            );
        };
        attributes.extend(
            inherited
                .into_iter()
                .filter(|attribute| !explicit_names.contains(&attribute.name.as_str())),
        );
    }
    let context = BzlEvaluationContext::from_evaluator(eval)
        .map_err(|_| anyhow::anyhow!("macro() may only be called in a .bzl module"))?;
    Ok(SymbolicMacroDefinitionGen {
        implementation,
        definition_source: Arc::new(context.source_identity_for_call(eval)?.clone()),
        source_identities_by_filename: context.source_identities_by_filename(),
        attributes: attributes.into(),
        documentation,
        exported_name: OnceCell::new(),
    })
}

#[starlark_module]
pub(crate) fn package_globals(builder: &mut GlobalsBuilder) {
    fn repository_rule<'v>(
        implementation: Value<'v>,
        #[starlark(require = named)] attrs: Option<Value<'v>>,
        #[starlark(require = named)] local: Option<bool>,
        #[starlark(require = named)] environ: Option<UnpackListOrTuple<&str>>,
        #[starlark(require = named)] configure: Option<bool>,
        #[starlark(require = named)] doc: Option<NoneOr<&str>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<RepositoryRuleDefinition<'v>> {
        let callable: Option<StarlarkCallable<'v>> =
            StarlarkCallable::unpack_value_opt(implementation);
        if callable.is_none() {
            anyhow::bail!("repository_rule implementation must be callable");
        }
        let _ = doc;
        let local = local.unwrap_or(false);
        let configure = configure.unwrap_or(false);
        let environment = Arc::new(
            environ
                .unwrap_or_default()
                .items
                .into_iter()
                .map(CompactString::new)
                .collect::<SmallSet<_>>(),
        );
        let context = BzlEvaluationContext::from_evaluator(eval)
            .map_err(|_| anyhow::anyhow!("repository_rule may only be called in a .bzl module"))?;
        let source_label = context.source_label();
        let canonical_source = if source_label.starts_with("@@") {
            source_label.to_owned()
        } else {
            format!("@@{source_label}")
        };
        let defining_label =
            CanonicalLabel::parse(&canonical_source).map_err(anyhow::Error::msg)?;
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
            let definition = attribute_definition_from_value(value)?.ok_or_else(|| {
                anyhow::anyhow!("repository attribute '{name}' must use attr.*()")
            })?;
            if definition.configurable_set
                || definition.late_bound_default.is_some()
                || definition.computed_default
                || definition.transition.is_some()
                || definition.executable
                || definition.exec_configuration
                || definition.flags.has_any_except(&[
                    AttributePropertyFlag::StarlarkDefined,
                    AttributePropertyFlag::StrictLabelChecking,
                    AttributePropertyFlag::Mandatory,
                    AttributePropertyFlag::SingleArtifact,
                ])
                || definition.rule_class_admissibility.classes().is_some()
                || !definition.required_providers.is_empty()
                || !matches!(definition.allowed_values, AllowedAttributeValues::None)
                || definition
                    .default
                    .as_ref()
                    .is_some_and(|value| !repository_rule_default_matches(definition.kind, value))
            {
                anyhow::bail!("unsupported repository_rule attribute schema '{name}'");
            }
            attributes.push(RepositoryRuleAttribute {
                name: name.into(),
                kind: definition.kind,
                mandatory: definition.mandatory,
                default: definition.default.clone(),
                file_admissibility: definition.file_admissibility.clone(),
            });
        }
        Ok(RepositoryRuleDefinition::new(
            implementation,
            defining_label,
            attributes.into(),
            local,
            configure,
            environment,
        ))
    }

    fn tag_class<'v>(
        attrs: Option<SmallMap<String, Value<'v>>>,
        #[starlark(require = named)] doc: Option<NoneOr<&str>>,
    ) -> anyhow::Result<TagClassDefinition> {
        let _ = doc;
        let mut attributes = Vec::new();
        for (name, value) in attrs.unwrap_or_default() {
            let definition = attribute_definition_from_value(value)?
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
            if definition.late_bound_default.is_some() || definition.computed_default {
                anyhow::bail!("tag attribute `{name}` does not support deferred defaults");
            }
            if !definition.file_admissibility.is_no_files()
                && !definition.file_admissibility.single_artifact()
            {
                anyhow::bail!("tag attribute `{name}` does not support allow_files");
            }
            if definition.flags.has_any_except(&[
                AttributePropertyFlag::StarlarkDefined,
                AttributePropertyFlag::StrictLabelChecking,
                AttributePropertyFlag::Mandatory,
                AttributePropertyFlag::NonEmpty,
                AttributePropertyFlag::SingleArtifact,
                AttributePropertyFlag::Nonconfigurable,
            ]) {
                anyhow::bail!("tag attribute `{name}` has unsupported attribute properties");
            }
            if !definition.required_providers.is_empty() {
                anyhow::bail!("tag attribute `{name}` does not support providers");
            }
            if definition.rule_class_admissibility.classes().is_some() {
                anyhow::bail!("tag attribute `{name}` does not support allow_rules");
            }
            attributes.push(ModuleExtensionTagAttribute {
                name: name.into(),
                kind: definition.kind,
                mandatory: definition.mandatory,
                configurable: definition.configurable,
                default: definition.default.clone(),
                file_admissibility: definition.file_admissibility.clone(),
                allowed_values: definition.allowed_values.clone(),
                allow_empty: definition.allow_empty,
            });
        }
        Ok(TagClassDefinition {
            attributes: attributes.into(),
        })
    }

    fn module_extension<'v>(
        implementation: Value<'v>,
        #[starlark(require = named)] tag_classes: Option<SmallMap<String, Value<'v>>>,
        #[starlark(require = named)] doc: Option<NoneOr<&str>>,
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
        default_visibility: Option<UnpackVisibility>,
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
        visibility: Option<UnpackVisibility>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        exports_files_global(srcs, visibility, eval)
    }

    fn filegroup<'v>(
        name: &str,
        srcs: Option<Value<'v>>,
        visibility: Option<UnpackVisibility>,
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
        visibility: Option<UnpackVisibility>,
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
        visibility: Option<UnpackVisibility>,
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
        #[starlark(require = named)] define_values: Option<SmallMap<String, String>>,
        #[starlark(require = named)] flag_values: Option<SmallMap<String, String>>,
        #[starlark(require = named)] constraint_values: Option<UnpackListOrTuple<&str>>,
        visibility: Option<UnpackVisibility>,
        #[starlark(kwargs)] kwargs: SmallMap<String, Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let recorder = PackageRecorder::from_evaluator(eval)?;
        recorder.config_setting(
            name.to_owned(),
            values,
            define_values,
            flag_values,
            constraint_values.map(list),
            visibility.map(|value| value.items),
        )?;
        recorder.set_native_generator_from_evaluator(name, eval)?;
        recorder.set_native_overrides(name, kwargs)?;
        Ok(NoneType)
    }

    fn constraint_setting<'v>(
        name: &str,
        #[starlark(require = named)] default_constraint_value: Option<Value<'v>>,
        visibility: Option<UnpackVisibility>,
        #[starlark(kwargs)] kwargs: SmallMap<String, Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let recorder = PackageRecorder::from_evaluator(eval)?;
        let default_constraint_value = match default_constraint_value {
            None => None,
            Some(value) if value.is_none() => None,
            Some(value) => Some(if let Some(label) = StarlarkLabel::from_value(value) {
                label.canonical().clone()
            } else if let Some(label) = value.unpack_str() {
                recorder.native_toolchain_label(label)?
            } else {
                anyhow::bail!("default_constraint_value must be a Label, string, or None")
            }),
        };
        recorder.native_toolchain_target_with_visibility(
            name.to_owned(),
            NativeToolchainTarget::ConstraintSetting {
                default_constraint_value,
            },
            visibility.map(|value| value.items),
        )?;
        recorder.set_native_generator_from_evaluator(name, eval)?;
        recorder.set_native_overrides(name, kwargs)?;
        Ok(NoneType)
    }

    fn constraint_value<'v>(
        name: &str,
        constraint_setting: &str,
        visibility: Option<UnpackVisibility>,
        #[starlark(kwargs)] kwargs: SmallMap<String, Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let recorder = PackageRecorder::from_evaluator(eval)?;
        recorder.native_toolchain_target_with_visibility(
            name.to_owned(),
            NativeToolchainTarget::ConstraintValue {
                constraint_setting: recorder.native_toolchain_label(constraint_setting)?,
            },
            visibility.map(|value| value.items),
        )?;
        recorder.set_native_generator_from_evaluator(name, eval)?;
        recorder.set_native_overrides(name, kwargs)?;
        Ok(NoneType)
    }

    fn platform<'v>(
        name: &str,
        #[starlark(default = UnpackList::default())] constraint_values: UnpackList<&str>,
        visibility: Option<UnpackVisibility>,
        #[starlark(kwargs)] kwargs: SmallMap<String, Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let recorder = PackageRecorder::from_evaluator(eval)?;
        recorder.native_toolchain_target_with_visibility(
            name.to_owned(),
            NativeToolchainTarget::Platform {
                constraint_values: recorder.native_toolchain_labels(&constraint_values.items)?,
            },
            visibility.map(|value| value.items),
        )?;
        recorder.set_native_generator_from_evaluator(name, eval)?;
        recorder.set_native_overrides(name, kwargs)?;
        Ok(NoneType)
    }

    fn toolchain_type<'v>(
        name: &str,
        visibility: Option<UnpackVisibility>,
        #[starlark(kwargs)] kwargs: SmallMap<String, Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let recorder = PackageRecorder::from_evaluator(eval)?;
        recorder.native_toolchain_target_with_visibility(
            name.to_owned(),
            NativeToolchainTarget::ToolchainType,
            visibility.map(|value| value.items),
        )?;
        recorder.set_native_generator_from_evaluator(name, eval)?;
        recorder.set_native_overrides(name, kwargs)?;
        Ok(NoneType)
    }

    fn toolchain<'v>(
        name: &str,
        toolchain: &str,
        toolchain_type: &str,
        #[starlark(require = named)] exec_compatible_with: Option<UnpackList<&str>>,
        #[starlark(require = named)] target_compatible_with: Option<UnpackList<&str>>,
        #[starlark(require = named)] use_target_platform_constraints: Option<bool>,
        #[starlark(require = named)] target_settings: Option<Value<'v>>,
        visibility: Option<UnpackVisibility>,
        #[starlark(kwargs)] kwargs: SmallMap<String, Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let recorder = PackageRecorder::from_evaluator(eval)?;
        recorder.native_toolchain_target_with_visibility(
            name.to_owned(),
            recorder.native_toolchain_declaration(
                toolchain,
                toolchain_type,
                exec_compatible_with,
                target_compatible_with,
                use_target_platform_constraints,
                target_settings,
            )?,
            visibility.map(|value| value.items),
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
        #[starlark(default = UnpackGlobExcludeDirectories::default())]
        exclude_directories: UnpackGlobExcludeDirectories,
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
        fragments: Option<UnpackListOrTuple<&str>>,
        #[starlark(require = named)] cfg: Option<Value<'v>>,
        #[starlark(require = named)] subrules: Option<Value<'v>>,
        #[starlark(require = named)] doc: Option<Value<'v>>,
        #[starlark(require = named)] provides: Option<Value<'v>>,
        #[starlark(require = named)] outputs: Option<Value<'v>>,
        #[starlark(require = named, default = false)] output_to_genfiles: bool,
        #[starlark(default = false)] executable: bool,
        #[starlark(default = false)] test: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<RuleDefinition<'v>> {
        if doc.is_some_and(|value| !value.is_none() && value.unpack_str().is_none()) {
            anyhow::bail!("rule doc must be a string or None");
        }
        let cfg = cfg.filter(|value| !value.is_none());
        let build_setting_definition = rule_build_setting(build_setting)?;
        if build_setting_definition.is_some() && cfg.is_some() {
            anyhow::bail!(
                "Build setting rules cannot use the `cfg` param to apply transitions to themselves."
            );
        }
        let incoming_transition = cfg
            .map(|value| {
                transition_definition_from_value(value).ok_or_else(|| {
                    anyhow::anyhow!(
                        "`cfg` must be set to a transition object initialized by the transition() function."
                    )
                })
            })
            .transpose()?;
        let declared_builtin_names =
            starlark_builtin_schema::<Value<'v>>(executable, test, build_setting_definition, true);
        let mut user_schema = Vec::new();
        let mut late_bound_attributes = Vec::new();
        if let Some(attrs) = attrs {
            for (name, value) in attrs {
                if declared_builtin_names
                    .iter()
                    .any(|schema| schema.name == name)
                {
                    anyhow::bail!("rule attribute `{name}` is built in and cannot be redeclared");
                }
                let definition = attribute_definition_from_value(value)?
                    .ok_or_else(|| anyhow::anyhow!("rule attribute `{name}` must use attr.*()"))?;
                if definition.configurable_set {
                    anyhow::bail!(
                        "attribute '{name}' has the 'configurable' argument set, which is not allowed in rule definitions"
                    );
                }
                if definition.computed_default {
                    anyhow::bail!(
                        "rule attribute `{name}` uses a default form deferred outside this packet"
                    );
                }
                if definition.late_bound_default.is_some() && !name.starts_with('_') {
                    anyhow::bail!(
                        "When an attribute value is a function, the attribute must be private (i.e. start with '_'). Found '{name}'"
                    );
                }
                if let Some(identity) = &definition.late_bound_default {
                    late_bound_attributes.push((
                        u32::try_from(user_schema.len()).expect("rule attribute count fits in u32"),
                        identity.clone(),
                        definition.required_providers.clone(),
                    ));
                }
                user_schema.push(declared_attribute_schema(name, &definition));
            }
        }
        let has_transition = incoming_transition.is_some()
            || user_schema.iter().any(|schema| schema.transition.is_some());
        let mut schema =
            starlark_builtin_schema(executable, test, build_setting_definition, has_transition);
        let builtin_count = u32::try_from(schema.len()).expect("built-in attribute count fits u32");
        schema.extend(user_schema);
        let late_bound_attributes = late_bound_attributes
            .into_iter()
            .map(
                |(user_index, identity, required_providers)| LateBoundRuleAttribute {
                    schema_index: builtin_count
                        .checked_add(user_index)
                        .expect("rule attribute count fits u32"),
                    identity,
                    required_providers,
                },
            )
            .collect::<Vec<_>>();
        let (attached_subrules, subrule_callables) = attached_subrules(subrules)?;
        let outputs = rule_outputs_definition(outputs)?;
        let context = BzlEvaluationContext::from_evaluator(eval)?;
        Ok(RuleDefinition {
            implementation,
            definition_source: Arc::new(context.source_identity_for_call(eval)?.clone()),
            source_identities_by_filename: context.source_identities_by_filename(),
            required_toolchains: toolchain_requirements(toolchains, eval)?,
            advertised_providers: advertised_provider_ids(provides, "rule provides")?,
            required_fragments: required_configuration_fragments(fragments),
            attached_subrules,
            subrule_callables,
            late_bound_attributes: late_bound_attributes.into(),
            schema: schema.into(),
            executable,
            test,
            build_setting_definition,
            incoming_transition,
            outputs,
            output_to_genfiles,
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
        #[starlark(require = named)] implementation: StarlarkCallable<'v>,
        #[starlark(require = named)] inputs: UnpackListOrTuple<&str>,
        #[starlark(require = named)] outputs: UnpackListOrTuple<&str>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<TransitionDefinition<'v>> {
        let inputs = list(inputs);
        let outputs = list(outputs);
        let context = BzlEvaluationContext::from_evaluator(eval)?;
        let source = context.source_identity_for_call(eval)?;
        let inputs = validate_transition_settings(&inputs, TransitionSettingsKind::Inputs, source)?;
        let outputs =
            validate_transition_settings(&outputs, TransitionSettingsKind::Outputs, source)?;
        let outputs = canonicalize_transition_settings(outputs, TransitionSettingsKind::Outputs)?;
        let inputs = canonicalize_transition_settings(inputs, TransitionSettingsKind::Inputs)?;
        Ok(TransitionDefinitionGen {
            implementation: implementation.0,
            inputs,
            outputs,
            definition_source: Arc::new(source.clone()),
            source_identities_by_filename: context.source_identities_by_filename(),
        })
    }
}

#[starlark_module]
fn aspect_globals(builder: &mut GlobalsBuilder) {
    fn aspect<'v>(
        implementation: Value<'v>,
        #[starlark(require = named)] attr_aspects: Option<Value<'v>>,
        #[starlark(require = named)] toolchains_aspects: Option<Value<'v>>,
        #[starlark(require = named)] attrs: Option<SmallMap<String, Value<'v>>>,
        #[starlark(require = named)] required_providers: Option<Value<'v>>,
        #[starlark(require = named)] required_aspect_providers: Option<Value<'v>>,
        #[starlark(require = named)] provides: Option<Value<'v>>,
        #[starlark(require = named)] requires: Option<Value<'v>>,
        #[starlark(require = named)] propagation_predicate: Option<Value<'v>>,
        #[starlark(require = named)] fragments: Option<UnpackListOrTuple<&str>>,
        #[starlark(require = named)] host_fragments: Option<UnpackListOrTuple<&str>>,
        #[starlark(require = named)] toolchains: Option<Value<'v>>,
        #[starlark(require = named)] doc: Option<Value<'v>>,
        #[starlark(require = named, default = false)] apply_to_generating_rules: bool,
        #[starlark(require = named)] exec_compatible_with: Option<UnpackListOrTuple<&str>>,
        #[starlark(require = named)] exec_groups: Option<Value<'v>>,
        #[starlark(require = named)] subrules: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<AspectDefinition<'v>> {
        if implementation.parameters_spec().is_none() {
            anyhow::bail!("aspect implementation must be a Starlark function");
        }
        if doc.is_some_and(|value| !value.is_none() && value.unpack_str().is_none()) {
            anyhow::bail!("aspect doc must be a string or None");
        }
        let _ = host_fragments;
        let context = BzlEvaluationContext::from_evaluator(eval)
            .map_err(|_| anyhow::anyhow!("aspect may only be called in a .bzl module"))?;
        let source = context.source_identity_for_call(eval)?;
        let source_label = context.source_label();
        let canonical_source = if source_label.starts_with("@@") {
            source_label.to_owned()
        } else {
            format!("@@{source_label}")
        };
        let defining_label =
            CanonicalLabel::parse(&canonical_source).map_err(anyhow::Error::msg)?;
        let (attributes, late_bound_attributes, required_parameters) = aspect_attributes(attrs)?;
        let required_provider_entries = required_providers.map_or(Ok(0), |value| {
            if let Some(values) = ListRef::from_value(value) {
                Ok(values.len())
            } else if let Some(values) = TupleRef::from_value(value) {
                Ok(values.len())
            } else {
                anyhow::bail!("required_providers must be a sequence")
            }
        })?;
        let propagation_predicate = propagation_predicate
            .filter(|value| !value.is_none())
            .map(|value| {
                value.parameters_spec().map(|_| value).ok_or_else(|| {
                    anyhow::anyhow!("propagation_predicate must be a function or None")
                })
            })
            .transpose()?;
        if apply_to_generating_rules && required_provider_entries != 0 {
            anyhow::bail!(
                "An aspect cannot simultaneously have required providers and apply to generating rules."
            );
        }
        if apply_to_generating_rules && propagation_predicate.is_some() {
            anyhow::bail!(
                "An aspect cannot simultaneously have a propagation predicate and apply to generating rules."
            );
        }
        if let Some(value) = exec_groups.filter(|value| !value.is_none()) {
            let groups = DictRef::from_value(value)
                .ok_or_else(|| anyhow::anyhow!("aspect exec_groups must be a dict or None"))?;
            if !groups.is_empty() {
                anyhow::bail!("nonempty aspect exec_groups are unsupported");
            }
        }
        let (attached_subrules, subrule_callables) = attached_subrules(subrules)?;
        let required_toolchains =
            aspect_toolchain_requirements(toolchains, &attached_subrules, eval)?;
        let required_providers =
            declaration_required_providers(required_providers, "required_providers")?;
        let required_aspect_providers =
            declaration_required_providers(required_aspect_providers, "required_aspect_providers")?;
        let advertised_providers = aspect_advertised_providers(provides)?;
        let required_fragments = required_configuration_fragments(fragments);
        Ok(AspectDefinitionGen {
            implementation,
            attr_aspects: aspect_attribute_propagation(attr_aspects)?,
            toolchains_aspects: aspect_toolchain_propagation(toolchains_aspects, source)?,
            attributes,
            late_bound_attributes,
            required_parameters,
            required_aspects: aspect_required_aspects(requires)?,
            propagation_predicate,
            required_toolchains,
            required_providers,
            required_aspect_providers,
            advertised_providers,
            required_fragments,
            apply_to_generating_rules,
            exec_compatible_with: aspect_exec_compatible_with(exec_compatible_with, source)?,
            attached_subrules,
            subrule_callables,
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
                        condition: SelectorCondition::Raw(condition.to_owned()),
                        value,
                    })
                    .collect(),
            }],
        })
    }
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct NativeModule;

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct BzlmodNativeModule;

impl fmt::Display for NativeModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("native")
    }
}

impl fmt::Display for BzlmodNativeModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("native")
    }
}

static NATIVE_METHODS: MethodsStatic = MethodsStatic::new();

#[starlark_module]
fn native_methods(builder: &mut MethodsBuilder) {
    fn existing_rule(
        #[starlark(this)] _native: Value,
        name: &str,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        let _ = name;
        if !RepositoryRuleInvocationState::is_active(eval) {
            anyhow::bail!(
                "native.existing_rule() is supported only during module extension evaluation"
            );
        }
        Ok(NoneType)
    }

    fn existing_rules<'v>(
        #[starlark(this)] _native: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        if !RepositoryRuleInvocationState::is_active(eval) {
            anyhow::bail!(
                "native.existing_rules() is supported only during module extension evaluation"
            );
        }
        Ok(FrozenValue::new_empty_dict().to_value())
    }

    fn exports_files(
        #[starlark(this)] _native: Value,
        srcs: UnpackListOrTuple<&str>,
        visibility: Option<UnpackVisibility>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        exports_files_global(srcs, visibility, eval)
    }

    fn filegroup<'v>(
        #[starlark(this)] _native: Value<'v>,
        name: &str,
        srcs: Option<Value<'v>>,
        visibility: Option<UnpackVisibility>,
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
        visibility: Option<UnpackVisibility>,
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
        visibility: Option<UnpackVisibility>,
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
        #[starlark(require = named)] define_values: Option<SmallMap<String, String>>,
        #[starlark(require = named)] flag_values: Option<SmallMap<String, String>>,
        #[starlark(require = named)] constraint_values: Option<UnpackListOrTuple<&str>>,
        visibility: Option<UnpackVisibility>,
        #[starlark(kwargs)] kwargs: SmallMap<String, Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let recorder = PackageRecorder::from_evaluator(eval)?;
        recorder.config_setting(
            name.to_owned(),
            values,
            define_values,
            flag_values,
            constraint_values.map(list),
            visibility.map(|value| value.items),
        )?;
        recorder.set_native_generator_from_evaluator(name, eval)?;
        recorder.set_native_overrides(name, kwargs)?;
        Ok(NoneType)
    }

    fn constraint_setting<'v>(
        #[starlark(this)] _native: Value<'v>,
        name: &str,
        #[starlark(require = named)] default_constraint_value: Option<Value<'v>>,
        visibility: Option<UnpackVisibility>,
        #[starlark(kwargs)] kwargs: SmallMap<String, Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let recorder = PackageRecorder::from_evaluator(eval)?;
        let default_constraint_value = match default_constraint_value {
            None => None,
            Some(value) if value.is_none() => None,
            Some(value) => Some(if let Some(label) = StarlarkLabel::from_value(value) {
                label.canonical().clone()
            } else if let Some(label) = value.unpack_str() {
                recorder.native_toolchain_label(label)?
            } else {
                anyhow::bail!("default_constraint_value must be a Label, string, or None")
            }),
        };
        recorder.native_toolchain_target_with_visibility(
            name.to_owned(),
            NativeToolchainTarget::ConstraintSetting {
                default_constraint_value,
            },
            visibility.map(|value| value.items),
        )?;
        recorder.set_native_generator_from_evaluator(name, eval)?;
        recorder.set_native_overrides(name, kwargs)?;
        Ok(NoneType)
    }

    fn constraint_value<'v>(
        #[starlark(this)] _native: Value<'v>,
        name: &str,
        constraint_setting: &str,
        visibility: Option<UnpackVisibility>,
        #[starlark(kwargs)] kwargs: SmallMap<String, Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let recorder = PackageRecorder::from_evaluator(eval)?;
        recorder.native_toolchain_target_with_visibility(
            name.to_owned(),
            NativeToolchainTarget::ConstraintValue {
                constraint_setting: recorder.native_toolchain_label(constraint_setting)?,
            },
            visibility.map(|value| value.items),
        )?;
        recorder.set_native_generator_from_evaluator(name, eval)?;
        recorder.set_native_overrides(name, kwargs)?;
        Ok(NoneType)
    }

    fn platform<'v>(
        #[starlark(this)] _native: Value<'v>,
        name: &str,
        #[starlark(default = UnpackList::default())] constraint_values: UnpackList<&str>,
        visibility: Option<UnpackVisibility>,
        #[starlark(kwargs)] kwargs: SmallMap<String, Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let recorder = PackageRecorder::from_evaluator(eval)?;
        recorder.native_toolchain_target_with_visibility(
            name.to_owned(),
            NativeToolchainTarget::Platform {
                constraint_values: recorder.native_toolchain_labels(&constraint_values.items)?,
            },
            visibility.map(|value| value.items),
        )?;
        recorder.set_native_generator_from_evaluator(name, eval)?;
        recorder.set_native_overrides(name, kwargs)?;
        Ok(NoneType)
    }

    fn toolchain_type<'v>(
        #[starlark(this)] _native: Value<'v>,
        name: &str,
        visibility: Option<UnpackVisibility>,
        #[starlark(kwargs)] kwargs: SmallMap<String, Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let recorder = PackageRecorder::from_evaluator(eval)?;
        recorder.native_toolchain_target_with_visibility(
            name.to_owned(),
            NativeToolchainTarget::ToolchainType,
            visibility.map(|value| value.items),
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
        #[starlark(require = named)] exec_compatible_with: Option<UnpackList<&str>>,
        #[starlark(require = named)] target_compatible_with: Option<UnpackList<&str>>,
        #[starlark(require = named)] use_target_platform_constraints: Option<bool>,
        #[starlark(require = named)] target_settings: Option<Value<'v>>,
        visibility: Option<UnpackVisibility>,
        #[starlark(kwargs)] kwargs: SmallMap<String, Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let recorder = PackageRecorder::from_evaluator(eval)?;
        recorder.native_toolchain_target_with_visibility(
            name.to_owned(),
            recorder.native_toolchain_declaration(
                toolchain,
                toolchain_type,
                exec_compatible_with,
                target_compatible_with,
                use_target_platform_constraints,
                target_settings,
            )?,
            visibility.map(|value| value.items),
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
        #[starlark(default = UnpackGlobExcludeDirectories::default())]
        exclude_directories: UnpackGlobExcludeDirectories,
        allow_empty: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Vec<String>> {
        glob_global(include, exclude, exclude_directories, allow_empty, eval)
    }
}

#[starlark_value(type = "native")]
impl<'v> StarlarkValue<'v> for NativeModule {
    fn get_methods() -> Option<&'static Methods> {
        NATIVE_METHODS.methods(native_methods)
    }
}

#[starlark_value(type = "native")]
impl<'v> StarlarkValue<'v> for BzlmodNativeModule {
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(|builder| {
            NATIVE_METHODS.populate(native_methods, builder);
            builder.set_attribute(
                "bazel_version",
                BuiltinBazelToolsSnapshot::CURRENT.bazel_version(),
                None,
            );
        })
    }
}

impl AllocFrozenValue for NativeModule {
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        heap.alloc_simple(self)
    }
}

impl AllocFrozenValue for BzlmodNativeModule {
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        heap.alloc_simple(self)
    }
}

#[starlark_module]
fn bzl_only_globals(builder: &mut GlobalsBuilder) {
    fn r#macro<'v>(
        #[starlark(require = named)] implementation: Value<'v>,
        #[starlark(require = named)] attrs: Option<Value<'v>>,
        #[starlark(require = named)] inherit_attrs: Option<Value<'v>>,
        #[starlark(require = named, default = false)] finalizer: bool,
        #[starlark(require = named)] doc: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<SymbolicMacroDefinition<'v>> {
        symbolic_macro_global(implementation, attrs, inherit_attrs, finalizer, doc, eval)
    }

    fn subrule<'v>(
        #[starlark(require = named)] implementation: Value<'v>,
        #[starlark(require = named)] attrs: Option<SmallMap<String, Value<'v>>>,
        #[starlark(require = named)] toolchains: Option<Value<'v>>,
        #[starlark(require = named)] fragments: Option<Value<'v>>,
        #[starlark(require = named)] subrules: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<crate::subrule::SubruleDefinition<'v>> {
        subrule_global(implementation, attrs, toolchains, fragments, subrules, eval)
    }

    fn configuration_field<'v>(
        fragment: &str,
        name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<ConfigurationFieldValue> {
        configuration_field_global(fragment, name, eval)
    }
}

fn complete_loading_globals(bool_config: bool, bzlmod_native: bool) -> Globals {
    let mut globals = GlobalsBuilder::new();
    populate_universe(&mut globals);
    package_globals(&mut globals);
    select_globals(&mut globals);
    LibraryExtension::Json.add(&mut globals);
    if bzlmod_native {
        globals.set("native", BzlmodNativeModule);
    } else {
        globals.set("native", NativeModule);
    }
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
        testing_bootstrap_globals(&mut globals);
        globals.set("OutputGroupInfo", OutputGroupInfo);
        globals.set("RunEnvironmentInfo", RunEnvironmentInfo);
        globals.set(
            "PackageSpecificationInfo",
            BuiltinProviderKey::new("PackageSpecificationInfo"),
        );
        globals.set("DefaultInfo", AnalysisBuiltinCallable::new("DefaultInfo"));
    } else {
        globals.set("config", BuildFileConfigModule);
    }
    globals.set("platform_common", PlatformCommonModule);
    globals.set("depset", AnalysisBuiltinCallable::new("depset"));
    globals.build()
}

pub(crate) fn loading_globals() -> Globals {
    complete_loading_globals(true, false)
}

pub(crate) fn bzlmod_loading_globals() -> Globals {
    complete_loading_globals(true, true)
}

pub(crate) fn build_file_loading_globals() -> Globals {
    complete_loading_globals(false, false)
}

#[cfg(test)]
mod module_extension_definition_tests {
    use slug_bzlmod_v2::NonrootAttributeInt;
    use starlark::environment::Module;
    use starlark::syntax::AstModule;
    use starlark::syntax::Dialect;

    use super::*;

    fn evaluate_with_globals(
        source: &str,
        globals: Globals,
    ) -> anyhow::Result<starlark::environment::FrozenModule> {
        let ast = AstModule::parse("//:ext.bzl", source.to_owned(), &Dialect::Standard)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        let module = Module::new();
        let context = BzlEvaluationContext::new("//:ext.bzl".to_owned());
        let mut evaluator = Evaluator::new(&module);
        evaluator.extra = Some(&context);
        evaluator
            .eval_module(ast, &globals)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        drop(evaluator);
        Ok(module.freeze()?)
    }

    fn evaluate(source: &str) -> anyhow::Result<starlark::environment::FrozenModule> {
        evaluate_with_globals(source, loading_globals())
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

    #[test]
    fn bzlmod_declaration_builtins_share_bazel_first_parameter_binding() {
        let source = |positional: bool| {
            let repository = if positional {
                "repository_rule(_impl)"
            } else {
                "repository_rule(implementation = _impl)"
            };
            let tag = if positional {
                "tag_class({'value': attr.string(default = 'same')})"
            } else {
                "tag_class(attrs = {'value': attr.string(default = 'same')})"
            };
            let extension = if positional {
                "module_extension(_impl, tag_classes = {'tag': tag, 'empty': empty})"
            } else {
                "module_extension(implementation = _impl, tag_classes = {'tag': tag, 'empty': empty})"
            };
            format!(
                "def _impl(ctx):\n    pass\nrepo = {repository}\ntag = {tag}\nempty = tag_class()\next = {extension}\n"
            )
        };
        let named = evaluate(&source(false)).unwrap();
        let positional = evaluate(&source(true)).unwrap();
        let repository_projection = |module: &starlark::environment::FrozenModule| {
            module
                .get("repo")
                .unwrap()
                .downcast::<crate::module_extension_repository_rule::FrozenRepositoryRuleDefinition>()
                .unwrap()
                .projection()
                .unwrap()
        };
        assert_eq!(
            repository_projection(&named),
            repository_projection(&positional)
        );
        let extension_projection = |module: &starlark::environment::FrozenModule| {
            module
                .get("ext")
                .unwrap()
                .downcast::<FrozenModuleExtensionDefinition>()
                .unwrap()
                .projection()
        };
        assert_eq!(
            extension_projection(&named),
            extension_projection(&positional)
        );

        for rejected in [
            "module_extension(_impl, implementation = _impl)",
            "module_extension(_impl, {})",
            "module_extension()",
            "tag_class({}, attrs = {})",
            "tag_class({}, 'doc')",
            "repository_rule(_impl, implementation = _impl)",
            "repository_rule(_impl, {})",
        ] {
            assert!(evaluate(&format!("def _impl(ctx):\n    pass\nbad = {rejected}\n")).is_err());
        }
    }

    #[test]
    fn symbolic_macro_preserves_ordered_suffix_and_integer_list_policy() {
        let module = evaluate(
            "def _impl(name, visibility, dep, nums): pass\n\
             M = macro(implementation = _impl, attrs = {\n\
               'dep': attr.label(allow_files = ['.rs', '.src', '.rs']),\n\
               'nums': attr.int_list(default = [1, -2], allow_empty = False),\n\
             })\n",
        )
        .unwrap();
        let definition = module
            .get("M")
            .unwrap()
            .downcast::<FrozenSymbolicMacroDefinition>()
            .unwrap();
        assert_eq!(definition.attributes.len(), 2);
        assert_eq!(
            definition.attributes[0].file_admissibility.suffixes(),
            Some([".rs".into(), ".src".into(), ".rs".into()].as_slice())
        );
        assert!(
            !definition.attributes[0]
                .file_admissibility
                .single_artifact()
        );
        assert_eq!(definition.attributes[1].kind, AttributeKind::IntegerList);
        assert_eq!(
            definition.attributes[1].default,
            Some(CoercedAttributeValue::IntegerList(Arc::from([1, -2])))
        );
        assert!(!definition.attributes[1].allow_empty);
    }

    #[test]
    fn rule_schema_retains_integer_list_and_allow_empty_policy() {
        let module = evaluate(
            "def _impl(ctx): pass\n\
             R = rule(implementation = _impl, attrs = {\n\
               'nums': attr.int_list(default = [1, -2], allow_empty = False),\n\
             })\n",
        )
        .unwrap();
        let definition = module
            .get("R")
            .unwrap()
            .downcast::<FrozenRuleDefinition>()
            .unwrap();
        let attribute = definition
            .schema
            .iter()
            .find(|attribute| attribute.name == "nums")
            .unwrap();
        assert_eq!(attribute.kind, AttributeKind::IntegerList);
        assert_eq!(
            attribute.default,
            Some(CoercedAttributeValue::IntegerList(Arc::from([1, -2])))
        );
        assert!(!attribute.allow_empty);
    }

    #[test]
    fn complete_collection_constructor_category_retains_allow_empty() {
        for constructor in [
            "int_list",
            "string_list",
            "label_list",
            "output_list",
            "string_dict",
            "string_list_dict",
            "string_keyed_label_dict",
            "label_keyed_string_dict",
            "label_list_dict",
        ] {
            let module = evaluate(&format!("X = attr.{constructor}(allow_empty = False)\n"))
                .unwrap_or_else(|error| panic!("{constructor}: {error}"));
            let definition = module
                .get("X")
                .unwrap()
                .downcast::<FrozenAttributeDefinition>()
                .unwrap();
            assert!(!definition.allow_empty, "{constructor}");
        }
        for constructor in ["bool", "int", "string", "label", "output"] {
            assert!(
                evaluate(&format!("X = attr.{constructor}(allow_empty = False)\n")).is_err(),
                "{constructor} exposed allow_empty"
            );
        }
    }

    #[test]
    fn attribute_documentation_category_is_named_typed_and_nonsemantic() {
        #[derive(Debug, PartialEq, Eq)]
        struct DefinitionSnapshot {
            kind: AttributeKind,
            mandatory: bool,
            configurable: bool,
            configurable_set: bool,
            file_admissibility: FileAdmissibility,
            flags: AttributePropertyFlags,
            allowed_values: AllowedAttributeValues,
            default: Option<CoercedAttributeValue>,
            late_bound_default: bool,
            computed_default: bool,
            executable: bool,
            exec_configuration: bool,
            required_providers: Arc<[Arc<[ProviderIdentity]>]>,
            attached_aspect: bool,
            transition: bool,
        }
        let snapshot = |constructor: &str, arguments: &str| {
            let module = evaluate(&format!("X = attr.{constructor}({arguments})\n")).unwrap();
            let definition = module
                .get("X")
                .unwrap()
                .downcast::<FrozenAttributeDefinition>()
                .unwrap();
            DefinitionSnapshot {
                kind: definition.kind,
                mandatory: definition.mandatory,
                configurable: definition.configurable,
                configurable_set: definition.configurable_set,
                file_admissibility: definition.file_admissibility.clone(),
                flags: definition.flags,
                allowed_values: definition.allowed_values.clone(),
                default: definition.default.clone(),
                late_bound_default: definition.late_bound_default.is_some(),
                computed_default: definition.computed_default,
                executable: definition.executable,
                exec_configuration: definition.exec_configuration,
                required_providers: definition.required_providers.clone(),
                attached_aspect: definition.attached_aspect.is_some(),
                transition: definition.transition.is_some(),
            }
        };
        for constructor in [
            "bool",
            "int",
            "string",
            "label",
            "string_list",
            "label_list",
            "string_keyed_label_dict",
            "label_keyed_string_dict",
            "label_list_dict",
            "output",
            "output_list",
            "string_dict",
            "string_list_dict",
        ] {
            let omitted = snapshot(constructor, "");
            assert_eq!(
                omitted,
                snapshot(constructor, "doc = None"),
                "{constructor}"
            );
            assert_eq!(
                omitted,
                snapshot(constructor, "doc = 'first documentation'"),
                "{constructor}"
            );
            assert_eq!(
                omitted,
                snapshot(constructor, "doc = 'different documentation'"),
                "{constructor}"
            );
            for invalid in ["1", "[]"] {
                assert!(
                    evaluate(&format!("X = attr.{constructor}(doc = {invalid})\n")).is_err(),
                    "{constructor} accepted doc={invalid}"
                );
            }
            assert!(
                evaluate(&format!("X = attr.{constructor}('documentation')\n")).is_err(),
                "{constructor} accepted positional documentation"
            );
        }
    }

    #[test]
    fn label_dependency_cfg_conversion_is_shared_across_all_five_constructors() {
        let snapshot = |constructor: &str, cfg: &str| {
            let module = evaluate(&format!("X = attr.{constructor}({cfg})\n")).unwrap();
            let definition = module
                .get("X")
                .unwrap()
                .downcast::<FrozenAttributeDefinition>()
                .unwrap();
            (
                definition.exec_configuration,
                definition.transition.is_some(),
            )
        };
        for constructor in [
            "label",
            "label_list",
            "string_keyed_label_dict",
            "label_keyed_string_dict",
            "label_list_dict",
        ] {
            let target = snapshot(constructor, "");
            assert_eq!(target, (false, false), "{constructor}");
            assert_eq!(target, snapshot(constructor, "cfg = None"), "{constructor}");
            assert_eq!(
                target,
                snapshot(constructor, "cfg = 'target'"),
                "{constructor}"
            );
            assert_eq!(
                snapshot(constructor, "cfg = 'exec'"),
                (true, false),
                "{constructor}"
            );
            for invalid in ["'host'", "'other'", "1", "[]"] {
                assert!(
                    evaluate(&format!("X = attr.{constructor}(cfg = {invalid})\n")).is_err(),
                    "{constructor} accepted cfg={invalid}"
                );
            }
        }

        for cfg in ["", "cfg = None"] {
            assert!(
                evaluate(&format!("X = attr.label(executable = True, {cfg})\n")).is_err(),
                "executable label accepted cfg={cfg}"
            );
        }
        for cfg in ["'target'", "'exec'"] {
            assert!(
                evaluate(&format!("X = attr.label(executable = True, cfg = {cfg})\n")).is_ok(),
                "executable label rejected cfg={cfg}"
            );
        }
    }

    #[test]
    fn dependency_attribute_property_flags_bind_and_mutate_as_bazel() {
        let descriptor_flags = |constructor: &str, arguments: &str| {
            let module = evaluate(&format!("X = attr.{constructor}({arguments})\n")).unwrap();
            module
                .get("X")
                .unwrap()
                .downcast::<FrozenAttributeDefinition>()
                .unwrap()
                .flags
        };
        let constructors = [
            "label",
            "label_list",
            "string_keyed_label_dict",
            "label_keyed_string_dict",
            "label_list_dict",
        ];
        for constructor in constructors {
            let omitted = descriptor_flags(constructor, "");
            assert!(omitted.contains(AttributePropertyFlag::StarlarkDefined));
            assert!(omitted.contains(AttributePropertyFlag::StrictLabelChecking));
            assert_eq!(omitted, descriptor_flags(constructor, "flags = []"));
            assert_eq!(omitted, descriptor_flags(constructor, "flags = ()"));

            let admitted = descriptor_flags(constructor, "flags = ['DIRECT_COMPILE_TIME_INPUT']");
            assert!(admitted.direct_compile_time_input(), "{constructor}");
            assert_eq!(
                admitted,
                descriptor_flags(constructor, "flags = ('DIRECT_COMPILE_TIME_INPUT',)")
            );
            assert_eq!(
                admitted,
                descriptor_flags(
                    constructor,
                    "flags = ['DIRECT_COMPILE_TIME_INPUT', 'DIRECT_COMPILE_TIME_INPUT']",
                )
            );
            assert!(
                evaluate(&format!(
                    "X = attr.{constructor}(['DIRECT_COMPILE_TIME_INPUT'])\n"
                ))
                .is_err(),
                "{constructor} accepted positional flags"
            );

            let false_override = descriptor_flags(
                constructor,
                "flags = ['FOR_DEPENDENCY_RESOLUTION'], for_dependency_resolution = False",
            );
            assert!(!false_override.contains(AttributePropertyFlag::ForDependencyResolution));
            assert!(
                false_override
                    .contains(AttributePropertyFlag::ForDependencyResolutionExplicitlySet)
            );
            let true_override = descriptor_flags(constructor, "for_dependency_resolution = True");
            assert!(true_override.contains(AttributePropertyFlag::ForDependencyResolution));
            assert!(
                true_override.contains(AttributePropertyFlag::ForDependencyResolutionExplicitlySet)
            );
        }

        let all_names = [
            "MANDATORY",
            "EXECUTABLE",
            "UNDOCUMENTED",
            "TAGGABLE",
            "ORDER_INDEPENDENT",
            "STRICT_LABEL_CHECKING",
            "DIRECT_COMPILE_TIME_INPUT",
            "NON_EMPTY",
            "SINGLE_ARTIFACT",
            "SILENT_RULECLASS_FILTER",
            "SKIP_ANALYSIS_TIME_FILETYPE_CHECK",
            "CHECK_ALLOWED_VALUES",
            "NONCONFIGURABLE",
            "CONFIGURABLE_ATTR_WAS_USER_SET",
            "SKIP_PREREQ_VALIDATOR_CHECKS",
            "CHECK_CONSTRAINTS_OVERRIDE",
            "SKIP_CONSTRAINTS_OVERRIDE",
            "OUTPUT_LICENSES",
            "HAS_STARLARK_DEFINED_TRANSITION",
            "HAS_ANALYSIS_TEST_TRANSITION",
            "IS_TOOL_DEPENDENCY",
            "STARLARK_DEFINED",
            "SKIP_VALIDATIONS",
            "FOR_DEPENDENCY_RESOLUTION",
            "FOR_DEPENDENCY_RESOLUTION_EXPLICITLY_SET",
        ];
        let all = descriptor_flags(
            "label_list",
            &format!(
                "flags = [{}]",
                all_names
                    .iter()
                    .map(|name| format!("'{name}'"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        );
        for name in all_names {
            assert!(
                all.contains(AttributePropertyFlag::from_name(name).unwrap()),
                "missing {name}"
            );
        }

        for constructor in [
            "bool",
            "int",
            "string",
            "string_list",
            "output",
            "output_list",
            "string_dict",
            "string_list_dict",
        ] {
            assert!(
                evaluate(&format!("X = attr.{constructor}(flags = [])\n")).is_err(),
                "{constructor} exposed flags"
            );
        }

        for invalid in ["None", "1", "['direct_compile_time_input']", "['UNKNOWN']"] {
            assert!(
                evaluate(&format!("X = attr.label_list(flags = {invalid})\n")).is_err(),
                "accepted flags={invalid}"
            );
        }
        let cast_error = evaluate("X = attr.label_list(flags = ['UNKNOWN', 1])\n")
            .unwrap_err()
            .to_string();
        assert!(
            !cast_error.contains("unknown attribute flag"),
            "{cast_error}"
        );
        let default_error =
            evaluate("X = attr.label(default = 1, flags = ['UNKNOWN', 1], allow_files = 1)\n")
                .unwrap_err()
                .to_string();
        assert!(
            !default_error.contains("unknown attribute flag")
                && !default_error.contains("attribute flags must contain only strings"),
            "flags ran before default coercion: {default_error}"
        );
        for later in [
            "allow_files = 1",
            "providers = 1",
            "cfg = 'host'",
            "executable = True",
        ] {
            let error = evaluate(&format!("X = attr.label(flags = ['UNKNOWN'], {later})\n"))
                .unwrap_err()
                .to_string();
            assert!(
                error.contains("unknown attribute flag 'UNKNOWN'"),
                "{error}"
            );
        }
        let aspect_error = evaluate("X = attr.label_list(flags = ['UNKNOWN'], aspects = [1])\n")
            .unwrap_err()
            .to_string();
        assert!(
            aspect_error.contains("unknown attribute flag 'UNKNOWN'"),
            "{aspect_error}"
        );

        for constructor in [
            "label",
            "label_list",
            "label_keyed_string_dict",
            "label_list_dict",
        ] {
            let flags = descriptor_flags(constructor, "skip_validations = True");
            assert!(flags.contains(AttributePropertyFlag::SkipValidations));
        }
        assert!(evaluate("X = attr.string_keyed_label_dict(skip_validations = True)\n").is_err());
        for invalid in ["None", "1", "'true'"] {
            assert!(
                evaluate(&format!(
                    "X = attr.label(for_dependency_resolution = {invalid})\n"
                ))
                .is_err()
            );
            assert!(evaluate(&format!("X = attr.label(skip_validations = {invalid})\n")).is_err());
        }

        let projected = evaluate(
            "X = attr.label_list(flags = ['MANDATORY', 'EXECUTABLE', 'ORDER_INDEPENDENT', 'NON_EMPTY', 'SINGLE_ARTIFACT', 'NONCONFIGURABLE'])\n",
        )
        .unwrap();
        let projected = projected
            .get("X")
            .unwrap()
            .downcast::<FrozenAttributeDefinition>()
            .unwrap();
        assert!(projected.mandatory && projected.executable);
        assert!(!projected.configurable && !projected.allow_empty);
        assert!(projected.file_admissibility.single_artifact());
    }

    #[test]
    fn dependency_attribute_property_flags_propagate_or_fail_closed() {
        let direct = "DIRECT_COMPILE_TIME_INPUT";

        let rule_flags = |arguments: &str| {
            let module = evaluate(&format!(
                "def impl(ctx):\n    pass\nR = rule(implementation = impl, attrs = {{'deps': attr.label_list({arguments})}})\n"
            ))
            .unwrap();
            let rule = module
                .get("R")
                .unwrap()
                .downcast::<FrozenRuleDefinition>()
                .unwrap();
            rule.schema
                .iter()
                .find(|attribute| attribute.name == "deps")
                .unwrap()
                .flags
        };
        let first = rule_flags("");
        let changed = rule_flags(&format!("flags = ['{direct}']"));
        let restored = rule_flags("");
        assert!(first.contains(AttributePropertyFlag::StarlarkDefined));
        assert!(changed.direct_compile_time_input());
        assert_eq!(first, restored);

        let control_aspect = "def impl(target, ctx): return []\nA = aspect(implementation = impl, attrs = {'_config': attr.label(allow_single_file = True, default = Label('//rust/settings:rustfmt.toml')), '_process_wrapper': attr.label(cfg = 'exec', executable = True, default = Label('//util/process_wrapper'))})\n";
        evaluate(control_aspect).unwrap();
        let flagged = evaluate(&control_aspect.replace(
            "allow_single_file = True",
            "flags = ['DIRECT_COMPILE_TIME_INPUT'], allow_single_file = True",
        ))
        .unwrap();
        let aspect = flagged
            .get("A")
            .unwrap()
            .downcast::<FrozenAspectDefinition>()
            .unwrap();
        assert!(aspect.attributes[0].flags.direct_compile_time_input());

        for source in [
            "def impl(name, visibility, deps): pass\nM = macro(implementation = impl, attrs = {'deps': attr.label_list(flags = ['DIRECT_COMPILE_TIME_INPUT'])})\n",
            "def rule_impl(ctx): pass\nR = rule(implementation = rule_impl, attrs = {'deps': attr.label_list(flags = ['DIRECT_COMPILE_TIME_INPUT'])})\ndef macro_impl(name, visibility, **kwargs): pass\nM = macro(implementation = macro_impl, inherit_attrs = R)\n",
        ] {
            let module = evaluate(source).unwrap();
            let definition = module
                .get("M")
                .unwrap()
                .downcast::<FrozenSymbolicMacroDefinition>()
                .unwrap();
            assert!(
                definition
                    .attributes
                    .iter()
                    .find(|attribute| attribute.name == "deps")
                    .unwrap()
                    .flags
                    .direct_compile_time_input()
            );
        }
        evaluate(
            "def impl(ctx): pass\nS = subrule(implementation = impl, attrs = {'_deps': attr.label_list(default = [], flags = ['DIRECT_COMPILE_TIME_INPUT'])})\n",
        )
        .unwrap();

        for source in [
            "def impl(ctx): pass\nR = rule(implementation = impl, attrs = {'deps': attr.label_list(flags = ['CHECK_ALLOWED_VALUES'])})\n",
            "def impl(target, ctx): pass\nA = aspect(implementation = impl, attrs = {'_deps': attr.label_list(default = [], flags = ['CHECK_ALLOWED_VALUES'])})\n",
            "def impl(name, visibility, deps): pass\nM = macro(implementation = impl, attrs = {'deps': attr.label_list(flags = ['CHECK_ALLOWED_VALUES'])})\n",
            "def impl(ctx): pass\nS = subrule(implementation = impl, attrs = {'_deps': attr.label_list(default = [], flags = ['CHECK_ALLOWED_VALUES'])})\n",
        ] {
            let error = evaluate(source).unwrap_err().to_string();
            assert!(error.contains("CHECK_ALLOWED_VALUES"), "{error}");
        }

        for (control, supported, unsupported) in [
            (
                "def impl(ctx): pass\nR = repository_rule(impl, attrs = {'deps': attr.label_list()})\n",
                "def impl(ctx): pass\nR = repository_rule(impl, attrs = {'deps': attr.label_list(flags = ['MANDATORY'])})\n",
                "def impl(ctx): pass\nR = repository_rule(impl, attrs = {'deps': attr.label_list(flags = ['SKIP_CONSTRAINTS_OVERRIDE'])})\n",
            ),
            (
                "T = tag_class(attrs = {'deps': attr.label_list()})\n",
                "T = tag_class(attrs = {'deps': attr.label_list(flags = ['MANDATORY'])})\n",
                "T = tag_class(attrs = {'deps': attr.label_list(flags = ['SKIP_CONSTRAINTS_OVERRIDE'])})\n",
            ),
        ] {
            evaluate(control).unwrap();
            evaluate(supported).unwrap();
            assert!(evaluate(unsupported).is_err(), "discarded {unsupported}");
        }
    }

    fn rule_class_descriptor(constructor: &str, arguments: &str) -> RuleClassAdmissibility {
        let module = evaluate(&format!("X = attr.{constructor}({arguments})\n")).unwrap();
        module
            .get("X")
            .unwrap()
            .downcast::<FrozenAttributeDefinition>()
            .unwrap()
            .rule_class_admissibility
            .clone()
    }

    fn assert_rule_class_constructor_contract() {
        for constructor in [
            "label",
            "label_list",
            "string_keyed_label_dict",
            "label_keyed_string_dict",
            "label_list_dict",
        ] {
            assert_eq!(
                rule_class_descriptor(constructor, ""),
                RuleClassAdmissibility::Any
            );
            assert_eq!(
                rule_class_descriptor(constructor, "allow_rules = None"),
                RuleClassAdmissibility::Any
            );
            assert_eq!(
                rule_class_descriptor(constructor, "allow_rules = []"),
                RuleClassAdmissibility::only(Vec::new())
            );
            let canonical =
                rule_class_descriptor(constructor, "allow_rules = ['z_rule', 'a_rule', 'z_rule']");
            assert_eq!(
                canonical,
                rule_class_descriptor(constructor, "allow_rules = ('a_rule', 'z_rule')")
            );
            assert_eq!(
                canonical.classes().unwrap().as_ref(),
                &[CompactString::new("a_rule"), CompactString::new("z_rule")]
            );
            assert!(
                evaluate(&format!("X = attr.{constructor}(['a_rule'])\n")).is_err(),
                "{constructor} accepted positional allow_rules"
            );
        }
        for constructor in [
            "bool",
            "int",
            "string",
            "string_list",
            "output",
            "output_list",
            "string_dict",
            "string_list_dict",
        ] {
            assert!(
                evaluate(&format!("X = attr.{constructor}(allow_rules = None)\n")).is_err(),
                "{constructor} exposed allow_rules"
            );
        }
        for invalid in ["1", "'rule'", "[1]", "['rule', 1]"] {
            let error = evaluate(&format!("X = attr.label(allow_rules = {invalid})\n"))
                .unwrap_err()
                .to_string();
            assert!(error.contains("allow_rules"), "{error}");
        }
    }

    fn assert_rule_class_failure_order() {
        let default_first =
            evaluate("X = attr.label(default = 1, allow_rules = [1], providers = 1)\n")
                .unwrap_err()
                .to_string();
        assert!(
            default_first.contains("attribute `label` must be a string")
                && !default_first.contains("allow_rules must contain only strings"),
            "{default_first}"
        );
        let flags_first = evaluate(
            "X = attr.label(flags = ['UNKNOWN'], allow_files = 1, allow_rules = [1], providers = 1)\n",
        )
        .unwrap_err()
        .to_string();
        assert!(
            flags_first.contains("unknown attribute flag"),
            "{flags_first}"
        );
        let file_first =
            evaluate("X = attr.label(allow_files = 1, allow_rules = [1], providers = 1)\n")
                .unwrap_err()
                .to_string();
        assert!(file_first.contains("allow_files"), "{file_first}");
        for later in ["providers = 1", "cfg = 'host'"] {
            let error = evaluate(&format!("X = attr.label(allow_rules = [1], {later})\n"))
                .unwrap_err()
                .to_string();
            assert!(error.contains("allow_rules"), "{error}");
        }
        let aspect_later = evaluate("X = attr.label_list(allow_rules = [1], aspects = [1])\n")
            .unwrap_err()
            .to_string();
        assert!(aspect_later.contains("allow_rules"), "{aspect_later}");
    }

    fn assert_rule_class_projections() {
        let rule_restriction = |arguments: &str| {
            let module = evaluate(&format!(
                "def impl(ctx): pass\nR = rule(implementation = impl, attrs = {{'deps': attr.label_list({arguments})}})\n"
            ))
            .unwrap();
            let rule = module
                .get("R")
                .unwrap()
                .downcast::<FrozenRuleDefinition>()
                .unwrap();
            rule.schema
                .iter()
                .find(|attribute| attribute.name == "deps")
                .unwrap()
                .rule_class_admissibility
                .clone()
        };
        let first = rule_restriction("");
        let changed = rule_restriction("allow_rules = ['library']");
        let restored = rule_restriction("");
        assert_eq!(first, RuleClassAdmissibility::Any);
        assert_eq!(
            changed,
            RuleClassAdmissibility::only(vec!["library".into()])
        );
        assert_eq!(first, restored);

        let aspect = evaluate(
            "def impl(target, ctx): return []\nA = aspect(implementation = impl, attrs = {'_dep': attr.label(default = Label('//:dep'), allow_rules = ['library'])})\n",
        )
        .unwrap();
        let aspect = aspect
            .get("A")
            .unwrap()
            .downcast::<FrozenAspectDefinition>()
            .unwrap();
        assert_eq!(
            aspect.attributes[0].rule_class_admissibility,
            RuleClassAdmissibility::only(vec!["library".into()])
        );

        for source in [
            "def impl(name, visibility, deps): pass\nM = macro(implementation = impl, attrs = {'deps': attr.label_list(allow_rules = ['library'])})\n",
            "def rule_impl(ctx): pass\nR = rule(implementation = rule_impl, attrs = {'deps': attr.label_list(allow_rules = ['library'])})\ndef macro_impl(name, visibility, **kwargs): pass\nM = macro(implementation = macro_impl, inherit_attrs = R)\n",
        ] {
            let module = evaluate(source).unwrap();
            let definition = module
                .get("M")
                .unwrap()
                .downcast::<FrozenSymbolicMacroDefinition>()
                .unwrap();
            assert_eq!(
                definition
                    .attributes
                    .iter()
                    .find(|attribute| attribute.name == "deps")
                    .unwrap()
                    .rule_class_admissibility,
                RuleClassAdmissibility::only(vec!["library".into()])
            );
        }
        evaluate(
            "def impl(ctx): pass\nS = subrule(implementation = impl, attrs = {'_deps': attr.label_list(default = [], allow_rules = ['library'])})\n",
        )
        .unwrap();

        for source in [
            "def impl(ctx): pass\nR = repository_rule(impl, attrs = {'deps': attr.label_list(allow_rules = ['library'])})\n",
            "T = tag_class(attrs = {'deps': attr.label_list(allow_rules = ['library'])})\n",
        ] {
            assert!(evaluate(source).is_err(), "discarded restriction: {source}");
        }
    }

    #[test]
    fn dependency_rule_class_restrictions_bind_canonicalize_and_propagate() {
        assert_rule_class_constructor_contract();
        assert_rule_class_failure_order();
        assert_rule_class_projections();
    }

    #[test]
    fn bazel_version_is_present_only_in_bzlmod_native() {
        let build = evaluate(
            "captured = (hasattr(native, 'bazel_version'), getattr(native, 'bazel_version', None), 'bazel_version' in dir(native))\n",
        )
        .unwrap();
        assert_eq!(
            build.get("captured").unwrap().value().to_repr(),
            "(False, None, False)"
        );
        assert!(evaluate("captured = native.bazel_version\n").is_err());

        let bzlmod = evaluate_with_globals(
            "captured = (native.bazel_version, hasattr(native, 'bazel_version'), getattr(native, 'bazel_version'), 'bazel_version' in dir(native))\n",
            bzlmod_loading_globals(),
        )
        .unwrap();
        assert_eq!(
            bzlmod.get("captured").unwrap().value().to_repr(),
            "(\"9.2.0\", True, \"9.2.0\", True)"
        );
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
            file_admissibility: FileAdmissibility::default(),
            allowed_values: AllowedAttributeValues::None,
            allow_empty: true,
        }
    }

    fn root_context() -> (
        CanonicalRepoName,
        SmallMap<ApparentRepoName, CanonicalRepoName>,
    ) {
        (
            CanonicalRepoName::root(),
            SmallMap::from_iter([
                (
                    ApparentRepoName::new("dep").unwrap(),
                    CanonicalRepoName::new("dep+").unwrap(),
                ),
                (
                    ApparentRepoName::root(),
                    CanonicalRepoName::new("empty+").unwrap(),
                ),
            ]),
        )
    }

    #[test]
    fn prepared_tag_label_values_use_calling_module_context_and_typed_passthrough() {
        let context = CanonicalRepoName::new("caller+").unwrap();
        let mapping = SmallMap::from_iter([
            (
                ApparentRepoName::new("dep").unwrap(),
                CanonicalRepoName::new("dep+").unwrap(),
            ),
            (
                ApparentRepoName::root(),
                CanonicalRepoName::new("empty+").unwrap(),
            ),
        ]);
        let raw = SmallMap::from_iter([(
            CompactString::from("labels"),
            NonrootAttributeValue::List(Arc::from([
                NonrootAttributeValue::String("bare".into()),
                NonrootAttributeValue::String("@dep".into()),
                NonrootAttributeValue::String("@//:empty".into()),
                NonrootAttributeValue::String("@@//:main".into()),
                NonrootAttributeValue::String("//conditions:default".into()),
                NonrootAttributeValue::Label("@@typed+//pkg:value".into()),
            ])),
        )]);
        let prepared = prepare_module_extension_tag_attributes(
            &[tag_attribute(
                "labels",
                AttributeKind::LabelList,
                false,
                None,
            )],
            &raw,
            &context,
            &mapping,
        )
        .unwrap();
        let CoercedAttributeValue::LabelList(labels) = &prepared[0].1 else {
            panic!("expected label list")
        };
        assert_eq!(
            labels.iter().map(ToString::to_string).collect::<Vec<_>>(),
            [
                "@@caller+//:bare",
                "@@dep+//:dep",
                "@@empty+//:empty",
                "@@//:main",
                "@@//conditions:default",
                "@@typed+//pkg:value",
            ]
        );

        let collision = SmallMap::from_iter([(
            CompactString::from("labels"),
            NonrootAttributeValue::Dict(Arc::new(SmallMap::from_iter([
                (
                    NonrootAttributeKey::String("@dep//:same".into()),
                    NonrootAttributeValue::String("first".into()),
                ),
                (
                    NonrootAttributeKey::Label("@@dep+//:same".into()),
                    NonrootAttributeValue::String("second".into()),
                ),
            ]))),
        )]);
        assert!(
            prepare_module_extension_tag_attributes(
                &[tag_attribute(
                    "labels",
                    AttributeKind::LabelKeyedStringDict,
                    false,
                    None,
                )],
                &collision,
                &context,
                &mapping,
            )
            .unwrap_err()
            .contains("duplicate canonical label")
        );
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
                NonrootAttributeValue::String("@dep//pkg:item".into()),
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
    fn prepared_tag_preserves_error_order_and_preconverted_default_labels() {
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
        let prepared = prepare_module_extension_tag_attributes(
            &invisible_default,
            &SmallMap::from_iter([(
                CompactString::from("first"),
                NonrootAttributeValue::String("set".into()),
            )]),
            &context,
            &mapping,
        )
        .unwrap();
        assert_eq!(
            prepared[1].1,
            CoercedAttributeValue::Label(CanonicalLabel::parse("@@missing+//:default").unwrap())
        );
        let prepared = prepare_module_extension_tag_attributes(
            &[tag_attribute(
                "targets",
                AttributeKind::LabelListDict,
                false,
                Some(CoercedAttributeValue::LabelListDict(Arc::from([(
                    "group".into(),
                    Arc::from([CanonicalLabel::parse("@@missing+//:nested").unwrap()]),
                )]))),
            )],
            &SmallMap::new(),
            &context,
            &mapping,
        )
        .unwrap();
        assert_eq!(
            prepared[0].1,
            CoercedAttributeValue::LabelListDict(Arc::from([(
                CompactString::from("group"),
                Arc::from([CanonicalLabel::parse("@@missing+//:nested").unwrap()]),
            )]))
        );
    }

    #[test]
    fn prepared_tag_complete_collection_matrix_defaults_and_failures() {
        let (context, mapping) = root_context();
        let int =
            |value| NonrootAttributeValue::Int(NonrootAttributeInt::from_decimal(value).unwrap());
        let string = |value: &str| NonrootAttributeValue::String(value.into());
        let sequence =
            |values: Vec<NonrootAttributeValue>| NonrootAttributeValue::List(values.into());
        let dict = |values| NonrootAttributeValue::Dict(Arc::new(SmallMap::from_iter(values)));
        let schema = [
            tag_attribute("ints", AttributeKind::IntegerList, false, None),
            tag_attribute("strings", AttributeKind::StringList, false, None),
            tag_attribute("labels", AttributeKind::LabelList, false, None),
            tag_attribute("output", AttributeKind::Output, false, None),
            tag_attribute("outputs", AttributeKind::OutputList, false, None),
            tag_attribute("strings_by_key", AttributeKind::StringDict, false, None),
            tag_attribute("lists_by_key", AttributeKind::StringListDict, false, None),
            tag_attribute(
                "labels_by_key",
                AttributeKind::StringKeyedLabelDict,
                false,
                None,
            ),
            tag_attribute(
                "strings_by_label",
                AttributeKind::LabelKeyedStringDict,
                false,
                None,
            ),
            tag_attribute("label_lists", AttributeKind::LabelListDict, false, None),
        ];
        let raw = SmallMap::from_iter([
            (
                CompactString::from("ints"),
                NonrootAttributeValue::Tuple(Arc::from([int("1"), int("-2")])),
            ),
            (
                CompactString::from("strings"),
                sequence(vec![string("one"), string("two")]),
            ),
            (
                CompactString::from("labels"),
                NonrootAttributeValue::Tuple(Arc::from([
                    string("//:local"),
                    string("@dep//pkg:item"),
                ])),
            ),
            (CompactString::from("output"), string("//:out")),
            (
                CompactString::from("outputs"),
                sequence(vec![string(":a"), string(":b")]),
            ),
            (
                CompactString::from("strings_by_key"),
                dict([(NonrootAttributeKey::String("key".into()), string("value"))]),
            ),
            (
                CompactString::from("lists_by_key"),
                dict([(
                    NonrootAttributeKey::String("key".into()),
                    NonrootAttributeValue::Tuple(Arc::from([string("one"), string("two")])),
                )]),
            ),
            (
                CompactString::from("labels_by_key"),
                dict([(
                    NonrootAttributeKey::String("key".into()),
                    string("@dep//pkg:item"),
                )]),
            ),
            (
                CompactString::from("strings_by_label"),
                dict([(
                    NonrootAttributeKey::Label("@@dep+//pkg:item".into()),
                    string("value"),
                )]),
            ),
            (
                CompactString::from("label_lists"),
                dict([(
                    NonrootAttributeKey::String("key".into()),
                    sequence(vec![string("//:local"), string("@dep//pkg:item")]),
                )]),
            ),
        ]);
        let prepared =
            prepare_module_extension_tag_attributes(&schema, &raw, &context, &mapping).unwrap();
        assert_eq!(
            prepared[0].1,
            CoercedAttributeValue::IntegerList(Arc::from([1, -2]))
        );
        assert!(matches!(
            prepared[1].1,
            CoercedAttributeValue::StringList(_)
        ));
        assert!(matches!(prepared[2].1, CoercedAttributeValue::LabelList(_)));
        assert_eq!(
            prepared[3].1,
            CoercedAttributeValue::Output(CanonicalLabel::parse("@@//:out").unwrap())
        );
        assert!(matches!(
            prepared[4].1,
            CoercedAttributeValue::OutputList(_)
        ));
        assert!(matches!(
            prepared[5].1,
            CoercedAttributeValue::StringDict(_)
        ));
        assert!(matches!(
            prepared[6].1,
            CoercedAttributeValue::StringListDict(_)
        ));
        assert!(matches!(
            prepared[7].1,
            CoercedAttributeValue::StringKeyedLabelDict(_)
        ));
        assert!(matches!(
            prepared[8].1,
            CoercedAttributeValue::LabelKeyedStringDict(_)
        ));
        assert!(matches!(
            prepared[9].1,
            CoercedAttributeValue::LabelListDict(_)
        ));

        let defaults =
            prepare_module_extension_tag_attributes(&schema, &SmallMap::new(), &context, &mapping)
                .unwrap();
        for (attribute, (_, value)) in schema.iter().zip(defaults.iter()) {
            assert_eq!(*value, module_extension_intrinsic_default(attribute.kind));
        }

        let mut ignores_allow_empty =
            tag_attribute("strings", AttributeKind::StringList, false, None);
        ignores_allow_empty.allow_empty = false;
        assert_eq!(
            prepare_module_extension_tag_attributes(
                &[ignores_allow_empty],
                &SmallMap::from_iter([(
                    CompactString::from("strings"),
                    NonrootAttributeValue::List(Arc::from([])),
                )]),
                &context,
                &mapping,
            )
            .unwrap()[0]
                .1,
            CoercedAttributeValue::StringList(Arc::from([]))
        );
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
                    AttributeKind::IntegerList,
                    false,
                    None
                )],
                &SmallMap::from_iter([(
                    CompactString::from("value"),
                    sequence(vec![NonrootAttributeValue::Int(
                        NonrootAttributeInt::from_decimal("2147483648").unwrap(),
                    )]),
                )]),
                &context,
                &mapping,
            )
            .unwrap_err()
            .contains("outside i32")
        );
        assert!(
            prepare_module_extension_tag_attributes(
                &[tag_attribute("value", AttributeKind::Output, false, None)],
                &SmallMap::from_iter([(CompactString::from("value"), string("//pkg:out"),)]),
                &context,
                &mapping,
            )
            .unwrap_err()
            .contains("current package")
        );
        for raw in ["@@//:out", "@dep"] {
            assert!(
                prepare_module_extension_tag_attributes(
                    &[tag_attribute("value", AttributeKind::Output, false, None)],
                    &SmallMap::from_iter([(CompactString::from("value"), string(raw),)]),
                    &context,
                    &mapping,
                )
                .unwrap_err()
                .contains("unsupported module-extension output")
            );
        }
        let mut duplicate_mapping = mapping.clone();
        duplicate_mapping.insert(
            ApparentRepoName::new("alias").unwrap(),
            CanonicalRepoName::new("dep+").unwrap(),
        );
        assert!(
            prepare_module_extension_tag_attributes(
                &[tag_attribute(
                    "value",
                    AttributeKind::LabelKeyedStringDict,
                    false,
                    None,
                )],
                &SmallMap::from_iter([(
                    CompactString::from("value"),
                    NonrootAttributeValue::Dict(Arc::new(SmallMap::from_iter([
                        (
                            NonrootAttributeKey::String("@dep//pkg:item".into()),
                            string("one"),
                        ),
                        (
                            NonrootAttributeKey::String("@alias//pkg:item".into()),
                            string("two"),
                        ),
                    ]))),
                )]),
                &context,
                &duplicate_mapping,
            )
            .unwrap_err()
            .contains("duplicate canonical")
        );
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
            value.tag_classes[0].1[1].file_admissibility.suffixes(),
            Some(extensions) if extensions == [".txt"]
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
    fn tag_schema_retains_allowed_empty_and_public_private_fields() {
        let value = projection(
            "def _impl(ctx):\n    pass\n\
             tag = tag_class(attrs = {\n\
               '_private': attr.int(default = 2, values = [2, 3]),\n\
               'items': attr.string_list(default = [], allow_empty = False),\n\
               'numbers': attr.int_list(default = (1, -2), allow_empty = False),\n\
             })\n\
             ext = module_extension(implementation = _impl, tag_classes = {'tag': tag})\n",
        );
        let attributes = &value.tag_classes[0].1;
        assert_eq!(attributes[0].name, "_private");
        assert!(matches!(
            attributes[0].allowed_values,
            AllowedAttributeValues::Integer(ref values) if values.as_ref() == [2, 3]
        ));
        assert!(!attributes[1].allow_empty);
        assert!(!attributes[2].allow_empty);
        assert_eq!(
            attributes[2].default,
            Some(CoercedAttributeValue::IntegerList(Arc::from([1, -2])))
        );

        let (context, mapping) = root_context();
        let prepared = prepare_module_extension_tag_attributes(
            attributes,
            &SmallMap::from_iter([(
                CompactString::from("items"),
                NonrootAttributeValue::List(Arc::from([])),
            )]),
            &context,
            &mapping,
        )
        .unwrap();
        assert_eq!(prepared[0].0, "_private");
        assert_eq!(
            prepared[1].1,
            CoercedAttributeValue::StringList(Arc::from([]))
        );

        let error = prepare_module_extension_tag_attributes(
            attributes,
            &SmallMap::from_iter([(
                CompactString::from("_private"),
                NonrootAttributeValue::Int(NonrootAttributeInt::from_decimal("4").unwrap()),
            )]),
            &context,
            &mapping,
        )
        .unwrap_err();
        assert!(error.contains("not allowed"), "{error}");
    }

    #[test]
    fn definition_failures_are_closed_before_publication() {
        let cases = [
            "ext = module_extension(implementation = 1)",
            "def _impl(ctx):\n    pass\ntag = tag_class(attrs = {'x': attr.string(configurable = False)})\next = module_extension(implementation = _impl, tag_classes = {'tag': tag})",
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
