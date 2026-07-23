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
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::PackageIdentifier;
use starlark::any::ProvidesStaticType;
use starlark::environment::Globals;
use starlark::environment::GlobalsBuilder;
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
use crate::bzl_module::BzlModuleIdentity;
use crate::bzl_module::FrozenBzlLifetimeEntry;
use crate::glob::GlobSpec;
use crate::glob::PackageListing;
use crate::glob::expand_glob;
use crate::provider::AnalysisBuiltinCallable;
use crate::provider::BzlEvaluationContext;
use crate::provider::UserProviderCallable;

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
    pub default_visibility: Vec<String>,
    pub targets: Vec<PackageTarget>,
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
            && self.used_globs == other.used_globs
            && self.direct_load_roots == other.direct_load_roots
            && self.reachable_loads == other.reachable_loads
            && self.load_fingerprint == other.load_fingerprint
    }
}

impl Eq for LoadedPackage {}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct PackageTarget {
    pub name: String,
    pub kind: PackageTargetKind,
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
    TestSuite {
        membership: TestSuiteMembership,
        tags: Arc<[CompactString]>,
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
            Self::TestSuite { .. } => Some(&TEST_SUITE_RULE_CAPABILITY),
            Self::StarlarkRule(rule) => Some(&rule.capability),
            Self::ExportedFile | Self::GeneratedFile { .. } => None,
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

/// The frozen rule implementation retained for configured-target analysis.
/// The containing package keeps its source `.bzl` module alive.
#[derive(Debug, Clone, Allocative)]
pub struct StarlarkRuleImplementation {
    #[allocative(skip)]
    implementation: FrozenValue,
    dependencies: Arc<[CanonicalLabel]>,
    schema: Arc<[AttributeSchema]>,
    values: Arc<[AttributeValue]>,
    capability: Arc<RuleCapability>,
}

impl PartialEq for StarlarkRuleImplementation {
    fn eq(&self, other: &Self) -> bool {
        // The frozen function is retained for Stage 6 lifetime only. Its heap
        // address is not package semantics and must not defeat DICE equality.
        self.dependencies == other.dependencies
            && self.schema == other.schema
            && self.values == other.values
            && self.capability == other.capability
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

    pub fn schema(&self) -> &[AttributeSchema] {
        &self.schema
    }

    pub fn values(&self) -> &[AttributeValue] {
        &self.values
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

#[derive(Debug, Default)]
struct PackageState {
    default_visibility: Vec<String>,
    targets: SmallMap<String, PackageTargetKind>,
    used_globs: Vec<GlobSpec>,
}

#[derive(Debug, ProvidesStaticType)]
pub(crate) struct PackageRecorder {
    listing: PackageListing,
    package: CompactString,
    state: RefCell<PackageState>,
}

impl PackageRecorder {
    pub(crate) fn new(listing: PackageListing, package: impl Into<CompactString>) -> Self {
        Self {
            listing,
            package: package.into(),
            state: RefCell::new(PackageState::default()),
        }
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

    fn set_default_visibility(&self, visibility: Vec<String>) {
        self.state.borrow_mut().default_visibility = visibility;
    }

    fn exports_files(&self, srcs: Vec<String>) -> anyhow::Result<()> {
        for src in srcs {
            self.record_target(src, PackageTargetKind::ExportedFile)?;
        }
        Ok(())
    }

    fn filegroup(&self, name: String, srcs: Option<Vec<String>>) -> anyhow::Result<()> {
        let srcs_explicit = srcs.is_some();
        let srcs = srcs
            .unwrap_or_default()
            .iter()
            .map(|src| self.dependency_label(src))
            .collect::<anyhow::Result<Vec<_>>>()?;
        reject_duplicate_canonical_labels(&srcs, "srcs", &name)?;
        let srcs = srcs.into();
        self.record_target(
            name,
            PackageTargetKind::Filegroup {
                srcs,
                srcs_explicit,
            },
        )
    }

    fn test_suite(
        &self,
        name: String,
        tests: Option<Vec<String>>,
        mut tags: Vec<String>,
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
        )
    }

    fn alias(&self, name: String, actual: String) -> anyhow::Result<()> {
        let actual = self.dependency_label(&actual)?;
        self.record_target(name, PackageTargetKind::Alias { actual })
    }

    fn config_setting(&self, name: String, values: SmallMap<String, String>) -> anyhow::Result<()> {
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
        )
    }

    fn starlark_rule(
        &self,
        name: String,
        implementation: FrozenValue,
        capability: Arc<RuleCapability>,
        schema: Arc<[AttributeSchema]>,
        values: Arc<[AttributeValue]>,
    ) -> anyhow::Result<()> {
        let mut dependencies = Vec::new();
        for value in values.iter() {
            if let CoercedAttributeValue::LabelList(labels) = value.value.as_ref() {
                reject_duplicate_canonical_labels(labels, &value.declaration_name, &name)?;
            }
            let schema = schema
                .iter()
                .find(|schema| schema.declaration_name() == value.declaration_name);
            if schema.is_some_and(|schema| {
                schema.dependency_reachable() && schema.kind().contributes_ordinary_dependencies()
            }) {
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
                schema,
                values,
                capability,
            }),
        )
    }

    fn dependency_label(&self, value: &str) -> anyhow::Result<CanonicalLabel> {
        package_context_label(&self.package, value)
    }

    fn record_target(&self, name: String, kind: PackageTargetKind) -> anyhow::Result<()> {
        let mut state = self.state.borrow_mut();
        if state.targets.get(&name).is_some() {
            anyhow::bail!("target '{name}' declared more than once");
        }
        state.targets.insert(name, kind);
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
        )
    }

    fn glob(&self, spec: GlobSpec) -> anyhow::Result<Vec<String>> {
        let matches = expand_glob(&self.listing, &spec)?;
        self.state.borrow_mut().used_globs.push(spec);
        Ok(matches)
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
        let mut state = self.state.into_inner();
        let mut implicit_candidates = state
            .targets
            .iter()
            .filter_map(|(name, kind)| match kind {
                PackageTargetKind::StarlarkRule(rule) if rule.is_test() => {
                    kind.test_metadata().map(|metadata| {
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
        for (_, kind) in state.targets.iter_mut() {
            if let PackageTargetKind::TestSuite {
                membership: TestSuiteMembership::Implicit { members, .. },
                tags,
            } = kind
            {
                *members = implicit_candidates
                    .iter()
                    .filter(|(_, metadata)| implicit_test_matches_suite(metadata, tags))
                    .map(|(label, _)| label.clone())
                    .collect::<Vec<_>>()
                    .into();
            }
        }
        LoadedPackage {
            package_dir,
            build_file,
            default_visibility: state.default_visibility,
            targets: state
                .targets
                .into_iter()
                .map(|(name, kind)| PackageTarget { name, kind })
                .collect(),
            used_globs: state.used_globs,
            direct_load_roots,
            reachable_loads,
            load_fingerprint,
            retained_bzl_modules,
        }
    }
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

fn package_global(
    default_visibility: Option<UnpackListOrTuple<&str>>,
    eval: &mut Evaluator,
) -> anyhow::Result<NoneType> {
    if let Some(default_visibility) = default_visibility {
        PackageRecorder::from_evaluator(eval)?.set_default_visibility(list(default_visibility));
    }
    Ok(NoneType)
}

fn exports_files_global(
    srcs: UnpackListOrTuple<&str>,
    eval: &mut Evaluator,
) -> anyhow::Result<NoneType> {
    PackageRecorder::from_evaluator(eval)?.exports_files(list(srcs))?;
    Ok(NoneType)
}

fn filegroup_global(
    name: &str,
    srcs: Option<UnpackListOrTuple<&str>>,
    eval: &mut Evaluator,
) -> anyhow::Result<NoneType> {
    PackageRecorder::from_evaluator(eval)?.filegroup(name.to_owned(), srcs.map(list))?;
    Ok(NoneType)
}

fn test_suite_global(
    name: &str,
    tests: Option<UnpackListOrTuple<&str>>,
    tags: UnpackListOrTuple<&str>,
    eval: &mut Evaluator,
) -> anyhow::Result<NoneType> {
    PackageRecorder::from_evaluator(eval)?.test_suite(
        name.to_owned(),
        tests.map(list),
        list(tags),
    )?;
    Ok(NoneType)
}

fn alias_global(name: &str, actual: &str, eval: &mut Evaluator) -> anyhow::Result<NoneType> {
    PackageRecorder::from_evaluator(eval)?.alias(name.to_owned(), actual.to_owned())?;
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
    anyhow::bail!("attribute values must contain strings, lists, or dictionaries")
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

// Bazel 9.2 source: Attribute.Builder documents type defaults as label=null,
// list=[], and string="". StarlarkAttrModule applies the corresponding empty
// defaults to the public label dictionaries and output_list.
fn intrinsic_default(kind: AttributeKind) -> CoercedAttributeValue {
    match kind {
        AttributeKind::Label | AttributeKind::Output => CoercedAttributeValue::None,
        AttributeKind::LabelList => CoercedAttributeValue::LabelList(Arc::from([])),
        AttributeKind::String => CoercedAttributeValue::String(CompactString::default()),
        AttributeKind::StringList => CoercedAttributeValue::StringList(Arc::from([])),
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
        AttributeKind::StringList => {
            let RawAttributeValue::List(values) = raw else {
                anyhow::bail!("attribute must be a list of strings");
            };
            let mut values = values
                .iter()
                .map(|value| raw_string(value, "string list"))
                .collect::<anyhow::Result<Vec<_>>>()?;
            values.sort_unstable();
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
    schema: Arc<[RuleAttributeSchema]>,
    executable: bool,
    test: bool,
    #[trace(unsafe_ignore)]
    rule_class: OnceCell<CompactString>,
}

/// The frozen definition contains no export-time interior mutability. Its
/// shared capability is cloned into every package instance of this rule.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct FrozenRuleDefinition {
    implementation: FrozenValue,
    schema: Arc<[RuleAttributeSchema]>,
    capability: Arc<RuleCapability>,
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
            schema: self.schema,
            capability: Arc::new(RuleCapability {
                rule_class,
                executable: self.executable || self.test,
                test_kind: self.test.then_some(TestRuleKind::Test),
            }),
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
    List(Arc<[RawAttributeValue]>),
    Dict(Arc<[(RawAttributeValue, RawAttributeValue)]>),
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
struct RuleAttributeSchema {
    name: CompactString,
    kind: AttributeKind,
    mandatory: bool,
    configurable: bool,
    default: Option<CoercedAttributeValue>,
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
struct AttributeDefinition {
    kind: AttributeKind,
    mandatory: bool,
    configurable: bool,
    default: Option<CoercedAttributeValue>,
}

starlark::starlark_simple_value!(AttributeDefinition);

impl fmt::Display for AttributeDefinition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "attr.{:?}()", self.kind)
    }
}

#[starlark_value(type = "attribute")]
impl<'v> StarlarkValue<'v> for AttributeDefinition {}

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

fn attribute_definition(
    kind: AttributeKind,
    mandatory: bool,
    configurable: bool,
    default: Option<Value>,
    eval: &Evaluator<'_, '_, '_>,
) -> anyhow::Result<AttributeDefinition> {
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
    })
}

#[starlark_module]
fn attr_methods(builder: &mut MethodsBuilder) {
    fn label(
        #[starlark(this)] _attr: Value,
        mandatory: Option<bool>,
        #[starlark(default = true)] configurable: bool,
        default: Option<Value>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<AttributeDefinition> {
        attribute_definition(
            AttributeKind::Label,
            mandatory.unwrap_or(false),
            configurable,
            default,
            eval,
        )
    }
    fn label_list(
        #[starlark(this)] _attr: Value,
        mandatory: Option<bool>,
        #[starlark(default = true)] configurable: bool,
        default: Option<Value>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<AttributeDefinition> {
        attribute_definition(
            AttributeKind::LabelList,
            mandatory.unwrap_or(false),
            configurable,
            default,
            eval,
        )
    }
    fn string_keyed_label_dict(
        #[starlark(this)] _attr: Value,
        mandatory: Option<bool>,
        #[starlark(default = true)] configurable: bool,
        default: Option<Value>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<AttributeDefinition> {
        attribute_definition(
            AttributeKind::StringKeyedLabelDict,
            mandatory.unwrap_or(false),
            configurable,
            default,
            eval,
        )
    }
    fn label_keyed_string_dict(
        #[starlark(this)] _attr: Value,
        mandatory: Option<bool>,
        #[starlark(default = true)] configurable: bool,
        default: Option<Value>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<AttributeDefinition> {
        attribute_definition(
            AttributeKind::LabelKeyedStringDict,
            mandatory.unwrap_or(false),
            configurable,
            default,
            eval,
        )
    }
    fn label_list_dict(
        #[starlark(this)] _attr: Value,
        mandatory: Option<bool>,
        #[starlark(default = true)] configurable: bool,
        default: Option<Value>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<AttributeDefinition> {
        attribute_definition(
            AttributeKind::LabelListDict,
            mandatory.unwrap_or(false),
            configurable,
            default,
            eval,
        )
    }
    fn output(
        #[starlark(this)] _attr: Value,
        mandatory: Option<bool>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<AttributeDefinition> {
        attribute_definition(
            AttributeKind::Output,
            mandatory.unwrap_or(false),
            false,
            None,
            eval,
        )
    }
    fn output_list(
        #[starlark(this)] _attr: Value,
        mandatory: Option<bool>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<AttributeDefinition> {
        attribute_definition(
            AttributeKind::OutputList,
            mandatory.unwrap_or(false),
            false,
            None,
            eval,
        )
    }
    fn string(
        #[starlark(this)] _attr: Value,
        mandatory: Option<bool>,
        #[starlark(default = true)] configurable: bool,
        default: Option<Value>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<AttributeDefinition> {
        attribute_definition(
            AttributeKind::String,
            mandatory.unwrap_or(false),
            configurable,
            default,
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
        if let Some(visibility) = names.get("visibility") {
            let visibility = ListRef::from_value(*visibility).ok_or_else(|| {
                starlark::Error::new_other(anyhow::anyhow!(
                    "attribute `visibility` must be a list of strings"
                ))
            })?;
            if visibility.iter().any(|value| value.unpack_str().is_none()) {
                return Err(starlark::Error::new_other(anyhow::anyhow!(
                    "attribute `visibility` must be a list of strings"
                )));
            }
        }
        let implementation = self.implementation;
        let capability = self.capability.clone();
        PackageRecorder::from_evaluator(eval)
            .and_then(|recorder| {
                let mut schema = Vec::with_capacity(self.schema.len());
                let mut values = Vec::with_capacity(self.schema.len());
                let mut generated = Vec::new();
                for declaration in self.schema.iter() {
                    let attribute_schema = AttributeSchema::new(
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
                    );
                    // Keep the full declaration schema even for an omitted
                    // optional value. Stage 8 must distinguish absent-looking
                    // values from a missing declaration.
                    schema.push(attribute_schema.clone());
                    let explicit = names.get(declaration.name.as_str()).copied();
                    let (provenance, value) = match explicit {
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
                let schema: Arc<[AttributeSchema]> = schema.into();
                let values: Arc<[AttributeValue]> = values.into();
                recorder.starlark_rule(
                    name.to_owned(),
                    implementation,
                    capability,
                    schema,
                    values,
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
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        package_global(default_visibility, eval)
    }

    fn exports_files(
        srcs: UnpackListOrTuple<&str>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        exports_files_global(srcs, eval)
    }

    fn filegroup(
        name: &str,
        srcs: Option<UnpackListOrTuple<&str>>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        filegroup_global(name, srcs, eval)
    }

    fn test_suite(
        name: &str,
        tests: Option<UnpackListOrTuple<&str>>,
        #[starlark(default=UnpackListOrTuple::default())] tags: UnpackListOrTuple<&str>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        test_suite_global(name, tests, tags, eval)
    }

    fn alias(name: &str, actual: &str, eval: &mut Evaluator) -> anyhow::Result<NoneType> {
        alias_global(name, actual, eval)
    }

    // Bazel 9.2 `ConfigRuleClasses.ConfigSettingRule` declares `values` as
    // the nonconfigurable string dictionary that records flag bindings. This
    // loading slice retains only that immutable declaration and rejects every
    // other config_setting argument rather than pretending to evaluate it.
    fn config_setting(
        name: &str,
        #[starlark(require = named)] values: SmallMap<String, String>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        PackageRecorder::from_evaluator(eval)?.config_setting(name.to_owned(), values)?;
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
        #[starlark(default = false)] executable: bool,
        #[starlark(default = false)] test: bool,
    ) -> anyhow::Result<RuleDefinition<'v>> {
        let mut schema = Vec::new();
        if let Some(attrs) = attrs {
            for (name, value) in attrs {
                if name == "tags" || (test && name == "size") {
                    anyhow::bail!("rule attribute `{name}` is built in and cannot be redeclared");
                }
                let definition = AttributeDefinition::from_value(value)
                    .ok_or_else(|| anyhow::anyhow!("rule attribute `{name}` must use attr.*()"))?;
                schema.push(RuleAttributeSchema {
                    name: CompactString::new(name),
                    kind: definition.kind,
                    mandatory: definition.mandatory,
                    configurable: definition.configurable,
                    default: definition.default.clone(),
                });
            }
        }
        schema.push(RuleAttributeSchema {
            name: CompactString::const_new("tags"),
            kind: AttributeKind::StringList,
            mandatory: false,
            configurable: false,
            default: Some(CoercedAttributeValue::StringList(Arc::from([]))),
        });
        if test {
            schema.push(RuleAttributeSchema {
                name: CompactString::const_new("size"),
                kind: AttributeKind::String,
                mandatory: false,
                configurable: false,
                default: Some(CoercedAttributeValue::String(CompactString::const_new(
                    "medium",
                ))),
            });
        }
        Ok(RuleDefinition {
            implementation,
            schema: schema.into(),
            executable,
            test,
            rule_class: OnceCell::new(),
        })
    }

    fn provider(
        #[starlark(require = named)] fields: SmallMap<String, String>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<UserProviderCallable> {
        UserProviderCallable::from_evaluator(fields, eval)
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
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        exports_files_global(srcs, eval)
    }

    fn filegroup(
        #[starlark(this)] _native: Value,
        name: &str,
        srcs: Option<UnpackListOrTuple<&str>>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        filegroup_global(name, srcs, eval)
    }

    fn test_suite(
        #[starlark(this)] _native: Value,
        name: &str,
        tests: Option<UnpackListOrTuple<&str>>,
        #[starlark(default=UnpackListOrTuple::default())] tags: UnpackListOrTuple<&str>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        test_suite_global(name, tests, tags, eval)
    }

    fn alias(
        #[starlark(this)] _native: Value,
        name: &str,
        actual: &str,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        alias_global(name, actual, eval)
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
    let mut globals = GlobalsBuilder::standard()
        .with(package_globals)
        .with(select_globals);
    globals.set("native", NativeModule);
    globals.set("attr", AttrModule);
    globals.set("DefaultInfo", AnalysisBuiltinCallable::new("DefaultInfo"));
    globals.set("depset", AnalysisBuiltinCallable::new("depset"));
    globals.build()
}
