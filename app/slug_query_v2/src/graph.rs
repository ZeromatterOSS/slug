/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Demand-driven unconfigured package graph ownership for loading query.

use std::fmt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use compact_str::CompactString;
use dice::CancellationContext;
use dice::DiceComputations;
use dice::Key;
use dupe::Dupe;
use slug_identity_v2::CanonicalLabel;
use slug_loading_v2::AttributeProvenance;
use slug_loading_v2::PackageTargetKind;
use slug_loading_v2::RuleCapability;
use slug_loading_v2::TestMetadata;
use slug_loading_v2::keys::PackageLoadKey;
use slug_loading_v2::keys::WorkspaceDirectoryEntryKind;
use slug_loading_v2::keys::WorkspaceDirectoryKey;
use slug_loading_v2::keys::WorkspaceDirectoryValue;
use slug_loading_v2::package::StarlarkRuleImplementation;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

#[derive(Debug, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Allocative, Dupe)]
pub struct QueryLabel(Arc<CanonicalLabel>);

impl QueryLabel {
    pub fn parse_root(value: &str) -> Result<Self, QueryError> {
        if value.starts_with('@') {
            return Err(QueryError::evaluation(format!(
                "external repository query patterns are deferred: {value}"
            )));
        }
        let canonical = format!("@@{value}");
        CanonicalLabel::parse(&canonical)
            .map(|label| Self(Arc::new(label)))
            .map_err(QueryError::evaluation)
    }

    pub fn from_canonical(label: CanonicalLabel) -> Self {
        Self(Arc::new(label))
    }

    pub fn package(&self) -> &str {
        self.0.package().package().as_str()
    }

    pub fn target(&self) -> &str {
        self.0.target().as_str()
    }

    pub fn is_root_repository(&self) -> bool {
        self.0.package().repo().is_root()
    }
}

impl fmt::Display for QueryLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_root_repository() {
            write!(f, "//{}:{}", self.package(), self.target())
        } else {
            write!(f, "{}:{}", self.0.package(), self.target())
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub enum QueryNodeKind {
    BuildFile,
    SourceFile,
    GeneratedFile,
    Rule(CompactString),
}

impl QueryNodeKind {
    pub fn is_rule(&self) -> bool {
        matches!(self, Self::Rule(_))
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct QueryNode {
    pub label: QueryLabel,
    pub kind: QueryNodeKind,
    /// Loading-time rule capability retained for ordinary-query filters. This
    /// is `None` for source, BUILD, and generated-file nodes.
    pub rule_capability: Option<RuleCapability>,
    pub test_metadata: Option<TestMetadata>,
    pub build_file: CompactString,
    pub dependencies: Arc<[QueryLabel]>,
    pub attributes: Arc<[QueryAttribute]>,
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct QueryAttribute {
    pub name: CompactString,
    pub labels: Arc<[QueryLabel]>,
    pub explicit: bool,
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct UnconfiguredPackageGraph {
    pub package: CompactString,
    pub nodes: SmallMap<QueryLabel, QueryNode>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct UnconfiguredPackageGraphKey {
    pub workspace: PathBuf,
    pub package: PathBuf,
}

impl fmt::Display for UnconfiguredPackageGraphKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unconfigured-package-graph:{}", self.package.display())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct SubtreePackageSet {
    pub packages: Arc<[CompactString]>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct SubtreePackageSetKey {
    pub workspace: PathBuf,
    pub prefix: PathBuf,
}

impl fmt::Display for SubtreePackageSetKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "subtree-package-set:{}:{}",
            self.workspace.display(),
            self.prefix.display()
        )
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative, Dupe)]
pub struct QueryError {
    pub message: Arc<str>,
    pub exit_code: i32,
    kind: QueryErrorKind,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Allocative, Dupe)]
enum QueryErrorKind {
    Syntax,
    Evaluation,
    TargetMissing,
    PackageLoading,
}

impl QueryError {
    pub fn syntax(message: impl Into<String>) -> Self {
        Self {
            message: Arc::from(message.into()),
            exit_code: 2,
            kind: QueryErrorKind::Syntax,
        }
    }

    pub fn evaluation(message: impl Into<String>) -> Self {
        Self {
            message: Arc::from(message.into()),
            exit_code: 7,
            kind: QueryErrorKind::Evaluation,
        }
    }

    pub fn package_loading(message: impl Into<String>) -> Self {
        Self {
            message: Arc::from(message.into()),
            exit_code: 7,
            kind: QueryErrorKind::PackageLoading,
        }
    }

    pub(crate) fn target_missing(message: impl Into<String>) -> Self {
        Self {
            message: Arc::from(message.into()),
            exit_code: 7,
            kind: QueryErrorKind::TargetMissing,
        }
    }

    pub fn needs_evaluation_context(&self) -> bool {
        matches!(self.kind, QueryErrorKind::PackageLoading)
    }

    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Arc::from(message.into());
        self
    }
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for QueryError {}

type GraphValue = Arc<Result<Arc<UnconfiguredPackageGraph>, QueryError>>;
type PackageSetValue = Arc<Result<SubtreePackageSet, QueryError>>;

#[async_trait]
impl Key for UnconfiguredPackageGraphKey {
    type Value = GraphValue;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        Arc::new(
            compute_package_graph(ctx, &self.workspace, &self.package)
                .await
                .map(Arc::new),
        )
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_ok()
    }
}

async fn compute_package_graph(
    ctx: &mut DiceComputations<'_>,
    workspace: &Path,
    package: &Path,
) -> Result<UnconfiguredPackageGraph, QueryError> {
    let package_dir = workspace.join(package);
    let loaded = ctx
        .compute(&PackageLoadKey {
            workspace: workspace.to_path_buf(),
            package: package_dir,
        })
        .await
        .map_err(|error| QueryError::package_loading(error.to_string()))?;
    let loaded = loaded
        .as_ref()
        .as_ref()
        .map_err(|error| QueryError::package_loading(error.to_string()))?;
    let package_name = path_to_package(package)?;
    let build_basename = loaded
        .build_file
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| QueryError::evaluation("loaded BUILD file has no UTF-8 basename"))?;
    let build_file = loaded
        .build_file
        .strip_prefix(workspace)
        .unwrap_or(&loaded.build_file)
        .to_string_lossy();
    let build_file = CompactString::new(&build_file);
    let mut nodes = SmallMap::with_capacity(loaded.targets.len() + 1);

    for target in &loaded.targets {
        // Borrow once from the loading target and retain an owned compact
        // projection in the immutable package graph. Query-time filters must
        // not classify targets or clone rule-class names repeatedly.
        let rule_capability = target.rule_capability().cloned();
        let test_metadata = target.test_metadata();
        let label = match &target.kind {
            PackageTargetKind::GeneratedFile { label, .. } => {
                QueryLabel::from_canonical(label.clone())
            }
            _ => label_in_package(&package_name, &target.name)?,
        };
        let (kind, dependencies, attributes) = match &target.kind {
            PackageTargetKind::ExportedFile if target.name == build_basename => {
                (QueryNodeKind::BuildFile, Arc::from([]), Arc::from([]))
            }
            PackageTargetKind::ExportedFile => {
                (QueryNodeKind::SourceFile, Arc::from([]), Arc::from([]))
            }
            PackageTargetKind::Filegroup {
                srcs,
                srcs_explicit,
            } => {
                let labels = srcs
                    .iter()
                    .cloned()
                    .map(QueryLabel::from_canonical)
                    .collect::<Vec<_>>();
                let mut seen = SmallSet::new();
                let dependencies = labels
                    .iter()
                    .filter(|label| seen.insert((*label).dupe()))
                    .map(QueryLabel::dupe)
                    .collect::<Vec<_>>();
                let attributes = Arc::from([QueryAttribute {
                    name: CompactString::new("srcs"),
                    labels: labels.into(),
                    explicit: *srcs_explicit,
                }]);
                (
                    QueryNodeKind::Rule(CompactString::new("filegroup rule")),
                    dependencies.into(),
                    attributes,
                )
            }
            PackageTargetKind::Alias { actual } => {
                let actual = QueryLabel::from_canonical(actual.clone());
                (
                    QueryNodeKind::Rule(CompactString::new("alias rule")),
                    Arc::from([actual.dupe()]),
                    Arc::from([QueryAttribute {
                        name: CompactString::new("actual"),
                        labels: Arc::from([actual]),
                        explicit: true,
                    }]),
                )
            }
            PackageTargetKind::ConfigSetting { .. } => (
                QueryNodeKind::Rule(CompactString::new("config_setting rule")),
                Arc::from([]),
                Arc::from([]),
            ),
            PackageTargetKind::TestSuite { membership, .. } => {
                let tests = membership
                    .tests()
                    .iter()
                    .cloned()
                    .map(QueryLabel::from_canonical)
                    .collect::<Vec<_>>();
                let implicit_tests = membership
                    .implicit_tests()
                    .iter()
                    .cloned()
                    .map(QueryLabel::from_canonical)
                    .collect::<Vec<_>>();
                let mut seen = SmallSet::new();
                let dependencies = tests
                    .iter()
                    .chain(implicit_tests.iter())
                    .filter(|label| seen.insert((*label).dupe()))
                    .map(QueryLabel::dupe)
                    .collect::<Vec<_>>();
                (
                    QueryNodeKind::Rule(CompactString::new("test_suite rule")),
                    dependencies.into(),
                    Arc::from([
                        QueryAttribute {
                            name: CompactString::new("tests"),
                            labels: tests.into(),
                            explicit: membership.tests_explicit(),
                        },
                        QueryAttribute {
                            name: CompactString::new("$implicit_tests"),
                            labels: implicit_tests.into(),
                            explicit: true,
                        },
                    ]),
                )
            }
            PackageTargetKind::StarlarkRule(implementation) => {
                let dependencies = implementation
                    .dependencies()
                    .iter()
                    .cloned()
                    .map(QueryLabel::from_canonical)
                    .collect::<Vec<_>>()
                    .into();
                (
                    QueryNodeKind::Rule(CompactString::new("rule")),
                    dependencies,
                    project_attributes(implementation),
                )
            }
            PackageTargetKind::GeneratedFile {
                generating_rule, ..
            } => (
                QueryNodeKind::GeneratedFile,
                Arc::from([label_in_package(&package_name, generating_rule)?]),
                Arc::from([]),
            ),
        };
        if target.name == build_basename && !matches!(kind, QueryNodeKind::BuildFile) {
            return Err(QueryError::evaluation(format!(
                "target '{}' collides with active BUILD file",
                label
            )));
        }
        nodes.insert(
            label.dupe(),
            QueryNode {
                label,
                kind,
                rule_capability,
                test_metadata,
                build_file: build_file.clone(),
                dependencies,
                attributes,
            },
        );
    }

    let build_label = label_in_package(&package_name, build_basename)?;
    if nodes.get(&build_label).is_none() {
        nodes.insert(
            build_label.dupe(),
            QueryNode {
                label: build_label,
                kind: QueryNodeKind::BuildFile,
                rule_capability: None,
                test_metadata: None,
                build_file: build_file.clone(),
                dependencies: Arc::from([]),
                attributes: Arc::from([]),
            },
        );
    }

    // Attribute-created source nodes are owned by the package containing the
    // attribute. Cross-package labels remain edges: their destination package
    // graph is loaded only if traversal demands it.
    let referenced_sources = nodes
        .values()
        .flat_map(|node| node.dependencies.iter())
        .filter(|label| label.is_root_repository() && label.package() == package_name)
        .filter(|label| nodes.get(*label).is_none())
        .map(QueryLabel::dupe)
        .collect::<SmallSet<_>>();
    for label in referenced_sources {
        if nodes.get(&label).is_none() {
            nodes.insert(
                label.dupe(),
                QueryNode {
                    label,
                    kind: QueryNodeKind::SourceFile,
                    rule_capability: None,
                    test_metadata: None,
                    build_file: build_file.clone(),
                    dependencies: Arc::from([]),
                    attributes: Arc::from([]),
                },
            );
        }
    }

    Ok(UnconfiguredPackageGraph {
        package: package_name,
        nodes,
    })
}

fn project_attributes(implementation: &StarlarkRuleImplementation) -> Arc<[QueryAttribute]> {
    implementation
        .schema()
        .iter()
        .zip(implementation.values())
        .filter_map(|(schema, value)| {
            debug_assert_eq!(value.declaration_name, schema.declaration_name());
            if !schema.dependency_reachable() {
                return None;
            }
            let mut labels = Vec::new();
            value.value.labels(&mut labels);
            Some(QueryAttribute {
                name: CompactString::new(schema.query_name()),
                labels: labels
                    .into_iter()
                    .map(QueryLabel::from_canonical)
                    .collect::<Vec<_>>()
                    .into(),
                explicit: value.provenance == AttributeProvenance::Explicit,
            })
        })
        .collect::<Vec<_>>()
        .into()
}

#[async_trait]
impl Key for SubtreePackageSetKey {
    type Value = PackageSetValue;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        Arc::new(compute_subtree_packages(ctx, &self.workspace, &self.prefix).await)
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_ok()
    }
}

async fn compute_subtree_packages(
    ctx: &mut DiceComputations<'_>,
    workspace: &Path,
    prefix: &Path,
) -> Result<SubtreePackageSet, QueryError> {
    let mut pending = vec![workspace.join(prefix)];
    let mut packages = Vec::new();
    while let Some(directory) = pending.pop() {
        let value = ctx
            .compute(&WorkspaceDirectoryKey {
                workspace: workspace.to_path_buf(),
                directory: directory.clone(),
            })
            .await
            .map_err(|error| QueryError::evaluation(error.to_string()))?;
        match value {
            WorkspaceDirectoryValue::Present(entries) => {
                let is_package = entries.iter().any(|entry| {
                    entry.kind == WorkspaceDirectoryEntryKind::RegularFile
                        && matches!(entry.name.as_str(), "BUILD.bazel" | "BUILD")
                });
                if is_package {
                    packages.push(path_to_package(
                        directory.strip_prefix(workspace).unwrap_or(&directory),
                    )?);
                }
                for entry in entries.iter().rev() {
                    if entry.kind == WorkspaceDirectoryEntryKind::Directory {
                        pending.push(directory.join(entry.name.as_str()));
                    }
                }
            }
            WorkspaceDirectoryValue::Absent => {}
            WorkspaceDirectoryValue::ReadError(error) => {
                return Err(QueryError::evaluation(format!(
                    "reading workspace directory {}: {error}",
                    directory.display()
                )));
            }
        }
    }
    packages.sort_unstable();
    packages.dedup();
    Ok(SubtreePackageSet {
        packages: packages.into(),
    })
}

fn path_to_package(path: &Path) -> Result<CompactString, QueryError> {
    let Some(value) = path.to_str() else {
        return Err(QueryError::evaluation(format!(
            "package path is not UTF-8: {}",
            path.display()
        )));
    };
    Ok(CompactString::new(value.replace('\\', "/")))
}

fn label_in_package(package: &str, target: &str) -> Result<QueryLabel, QueryError> {
    QueryLabel::parse_root(&format!("//{package}:{target}"))
}
