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

use std::cmp::Ordering;
use std::fmt;
use std::hash::Hash;
use std::hash::Hasher;
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
use futures::FutureExt;
use slug_bzlmod_v2::HostRootPackageBoundaryKey;
use slug_bzlmod_v2::HostRootPackageBoundaryKind;
use slug_bzlmod_v2::RootPackageLookupInputsProjectionKey;
use slug_bzlmod_v2::RootRepositoryRoute;
use slug_bzlmod_v2::SourcePreparationNeeds;
use slug_identity_v2::ApparentLabel;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::PackagePath;
use slug_loading_v2::AttributeProvenance;
use slug_loading_v2::LoadedPackage;
use slug_loading_v2::LoadingPreparationOutcome;
use slug_loading_v2::PackageGroupContents;
use slug_loading_v2::PackageTargetKind;
use slug_loading_v2::RepositoryPackageLoadKey;
use slug_loading_v2::RootPackageLoadKey;
use slug_loading_v2::RuleCapability;
use slug_loading_v2::RuleVisibility;
use slug_loading_v2::TestMetadata;
use slug_loading_v2::VisibilitySource;
use slug_loading_v2::keys::PackageLoadKey;
use slug_loading_v2::keys::WorkspaceDirectoryEntryKind;
use slug_loading_v2::keys::WorkspaceDirectoryKey;
use slug_loading_v2::keys::WorkspaceDirectoryValue;
use slug_loading_v2::package::StarlarkRuleImplementation;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathDirectoryEntryKind;
use slug_workspace_v2::PathDirectoryListing;
use slug_workspace_v2::PathDirectoryListingKey;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathOutcome;
use slug_workspace_v2::ResolvedPathKey;
use slug_workspace_v2::ResolvedPathState;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

#[derive(Debug, Clone, Allocative, Dupe)]
pub struct QueryLabel {
    canonical: Arc<CanonicalLabel>,
    apparent_repo: Option<Arc<ApparentRepoName>>,
}

impl PartialEq for QueryLabel {
    fn eq(&self, other: &Self) -> bool {
        self.canonical == other.canonical
    }
}

impl Eq for QueryLabel {}

impl Hash for QueryLabel {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.canonical.hash(state);
    }
}

impl Ord for QueryLabel {
    fn cmp(&self, other: &Self) -> Ordering {
        self.canonical.cmp(&other.canonical)
    }
}

impl PartialOrd for QueryLabel {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl QueryLabel {
    pub fn parse_root(value: &str) -> Result<Self, QueryError> {
        if value.starts_with('@') {
            return Err(QueryError::evaluation(format!(
                "external repository query patterns are deferred: {value}"
            )));
        }
        let canonical = format!("@@{value}");
        CanonicalLabel::parse(&canonical)
            .map(|label| Self {
                canonical: Arc::new(label),
                apparent_repo: None,
            })
            .map_err(QueryError::evaluation)
    }

    pub fn from_canonical(label: CanonicalLabel) -> Self {
        Self {
            canonical: Arc::new(label),
            apparent_repo: None,
        }
    }

    pub(crate) fn from_apparent_route(
        label: &ApparentLabel,
        canonical_repo: &CanonicalRepoName,
    ) -> Result<Self, QueryError> {
        let canonical = CanonicalLabel::parse(&format!(
            "{}//{}:{}",
            canonical_repo,
            label.package(),
            label.target()
        ))
        .map_err(QueryError::evaluation)?;
        Ok(Self {
            canonical: Arc::new(canonical),
            apparent_repo: Some(Arc::new(label.repo().clone())),
        })
    }

    pub(crate) fn in_external_package(
        canonical_repo: &CanonicalRepoName,
        apparent_repo: &ApparentRepoName,
        package: &PackagePath,
        target: &str,
    ) -> Result<Self, QueryError> {
        let canonical = CanonicalLabel::parse(&format!("{}//{}:{target}", canonical_repo, package))
            .map_err(QueryError::evaluation)?;
        Ok(Self {
            canonical: Arc::new(canonical),
            apparent_repo: Some(Arc::new(apparent_repo.clone())),
        })
    }

    pub fn package(&self) -> &str {
        self.canonical.package().package().as_str()
    }

    pub fn target(&self) -> &str {
        self.canonical.target().as_str()
    }

    pub fn is_root_repository(&self) -> bool {
        self.canonical.package().repo().is_root()
    }

    pub(crate) fn apparent_repo(&self) -> Option<&ApparentRepoName> {
        self.apparent_repo.as_deref()
    }

    pub(crate) fn output_label(&self) -> CompactString {
        match &self.apparent_repo {
            Some(repo) => CompactString::new(format!(
                "@{}//{}:{}",
                repo.as_str(),
                self.package(),
                self.target()
            )),
            None => CompactString::new(self.to_string()),
        }
    }
}

impl fmt::Display for QueryLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_root_repository() {
            write!(f, "//{}:{}", self.package(), self.target())
        } else {
            write!(f, "{}:{}", self.canonical.package(), self.target())
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub enum QueryNodeKind {
    BuildFile,
    SourceFile,
    GeneratedFile,
    PackageGroup,
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
    pub effective_visibility: RuleVisibility,
    pub visibility_source: VisibilitySource,
    pub package_group_contents: Option<Arc<PackageGroupContents>>,
    pub edges: Arc<[QueryEdge]>,
    pub attributes: Arc<[QueryAttribute]>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Allocative, Dupe)]
pub enum QueryEdgeKind {
    GeneratingRule,
    VisibilityNodep,
    Ordinary,
    PackageGroupInclude,
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct QueryEdge {
    pub kind: QueryEdgeKind,
    pub target: QueryLabel,
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

#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub(crate) struct RootUnconfiguredPackageGraphKey {
    workspace: NormalizedAbsolutePath,
    package: PackagePath,
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub(crate) struct ExternalUnconfiguredPackageGraphKey {
    route: RootRepositoryRoute,
    package: PackagePath,
}

impl RootUnconfiguredPackageGraphKey {
    pub(crate) fn new(workspace: NormalizedAbsolutePath, package: PackagePath) -> Self {
        Self { workspace, package }
    }
}

impl ExternalUnconfiguredPackageGraphKey {
    pub(crate) fn new(route: RootRepositoryRoute, package: PackagePath) -> Self {
        Self { route, package }
    }
}

impl Hash for ExternalUnconfiguredPackageGraphKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.route.hash(state);
        self.package.hash(state);
    }
}

impl fmt::Display for RootUnconfiguredPackageGraphKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "root-unconfigured-package-graph://{}",
            self.package.as_str()
        )
    }
}

impl fmt::Display for ExternalUnconfiguredPackageGraphKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "external-unconfigured-package-graph:{}//{}",
            self.route.canonical_repo(),
            self.package
        )
    }
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

#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub(crate) struct RootSubtreePackageSetKey {
    workspace: NormalizedAbsolutePath,
    prefix: PackagePath,
}

impl RootSubtreePackageSetKey {
    pub(crate) fn new(workspace: NormalizedAbsolutePath, prefix: PackagePath) -> Self {
        Self { workspace, prefix }
    }
}

impl fmt::Display for RootSubtreePackageSetKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "root-subtree-package-set://{}", self.prefix.as_str())
    }
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

#[derive(Debug, Clone, Copy, Eq, PartialEq, Allocative, Dupe)]
enum QueryErrorKind {
    Syntax,
    Evaluation,
    TargetMissing,
    PackageLoading,
    PreparationRestart,
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
        if self.is_preparation_restart() {
            return self;
        }
        self.message = Arc::from(message.into());
        self
    }

    pub(crate) fn preparation_restart() -> Self {
        Self {
            message: Arc::from(""),
            exit_code: i32::MIN,
            kind: QueryErrorKind::PreparationRestart,
        }
    }

    pub(crate) fn is_preparation_restart(&self) -> bool {
        matches!(self.kind, QueryErrorKind::PreparationRestart)
    }
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for QueryError {}

type GraphValue = Arc<Result<Arc<UnconfiguredPackageGraph>, QueryError>>;
type RootGraphValue = LoadingPreparationOutcome<GraphValue>;
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

#[async_trait]
impl Key for RootUnconfiguredPackageGraphKey {
    type Value = RootGraphValue;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        match ctx
            .compute(&RootPackageLoadKey::new(
                self.workspace.clone(),
                self.package.clone(),
            ))
            .await
            .expect("root package loading DICE invariant")
        {
            LoadingPreparationOutcome::Need(need) => LoadingPreparationOutcome::Need(need),
            LoadingPreparationOutcome::Complete(loaded) => {
                LoadingPreparationOutcome::Complete(Arc::new(
                    loaded
                        .as_ref()
                        .as_ref()
                        .map_err(|error| QueryError::package_loading(error.to_string()))
                        .and_then(|loaded| {
                            package_graph_from_loaded(
                                self.workspace.as_path(),
                                Path::new(self.package.as_str()),
                                loaded,
                            )
                            .map(Arc::new)
                        }),
                ))
            }
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[async_trait]
impl Key for ExternalUnconfiguredPackageGraphKey {
    type Value = RootGraphValue;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        match ctx
            .compute(&RepositoryPackageLoadKey::new(
                self.route.clone(),
                self.package.clone(),
            ))
            .await
            .expect("external package loading DICE invariant")
        {
            LoadingPreparationOutcome::Need(need) => LoadingPreparationOutcome::Need(need),
            LoadingPreparationOutcome::Complete(loaded) => {
                LoadingPreparationOutcome::Complete(Arc::new(
                    loaded
                        .as_ref()
                        .as_ref()
                        .map_err(|error| QueryError::package_loading(error.to_string()))
                        .and_then(|loaded| {
                            external_package_graph_from_loaded(&self.route, &self.package, loaded)
                                .map(Arc::new)
                        }),
                ))
            }
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
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
    package_graph_from_loaded(workspace, package, loaded)
}

fn package_graph_from_loaded(
    workspace: &Path,
    package: &Path,
    loaded: &LoadedPackage,
) -> Result<UnconfiguredPackageGraph, QueryError> {
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
        let effective_visibility = effective_visibility(loaded, target)?;
        let visibility_source = target.visibility.clone();
        let package_group_contents = match &target.kind {
            PackageTargetKind::PackageGroup { contents, .. } => Some(contents.clone()),
            _ => None,
        };
        let visibility_edges = effective_visibility
            .dependency_labels()
            .iter()
            .cloned()
            .map(|label| QueryEdge {
                kind: QueryEdgeKind::VisibilityNodep,
                target: QueryLabel::from_canonical(label),
            })
            .collect::<Vec<_>>();
        let (kind, edges, mut attributes) = match &target.kind {
            PackageTargetKind::ExportedFile if target.name == build_basename => {
                (QueryNodeKind::BuildFile, visibility_edges, Vec::new())
            }
            PackageTargetKind::ExportedFile => {
                (QueryNodeKind::SourceFile, visibility_edges, Vec::new())
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
                let ordinary = labels
                    .iter()
                    .filter(|label| seen.insert((*label).dupe()))
                    .map(QueryLabel::dupe)
                    .collect::<Vec<_>>();
                let attributes = vec![QueryAttribute {
                    name: CompactString::new("srcs"),
                    labels: labels.into(),
                    explicit: *srcs_explicit,
                }];
                let mut edges = visibility_edges;
                edges.extend(ordinary.into_iter().map(|target| QueryEdge {
                    kind: QueryEdgeKind::Ordinary,
                    target,
                }));
                (
                    QueryNodeKind::Rule(CompactString::new("filegroup rule")),
                    edges,
                    attributes,
                )
            }
            PackageTargetKind::Alias { actual } => {
                let actual = QueryLabel::from_canonical(actual.clone());
                let mut edges = visibility_edges;
                edges.push(QueryEdge {
                    kind: QueryEdgeKind::Ordinary,
                    target: actual.dupe(),
                });
                (
                    QueryNodeKind::Rule(CompactString::new("alias rule")),
                    edges,
                    vec![QueryAttribute {
                        name: CompactString::new("actual"),
                        labels: Arc::from([actual]),
                        explicit: true,
                    }],
                )
            }
            PackageTargetKind::ConfigSetting { .. } => (
                QueryNodeKind::Rule(CompactString::new("config_setting rule")),
                visibility_edges,
                Vec::new(),
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
                let ordinary = tests
                    .iter()
                    .chain(implicit_tests.iter())
                    .filter(|label| seen.insert((*label).dupe()))
                    .map(QueryLabel::dupe)
                    .collect::<Vec<_>>();
                let mut edges = visibility_edges;
                edges.extend(ordinary.into_iter().map(|target| QueryEdge {
                    kind: QueryEdgeKind::Ordinary,
                    target,
                }));
                (
                    QueryNodeKind::Rule(CompactString::new("test_suite rule")),
                    edges,
                    vec![
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
                    ],
                )
            }
            PackageTargetKind::StarlarkRule(implementation) => {
                let ordinary = implementation
                    .dependencies()
                    .iter()
                    .cloned()
                    .map(QueryLabel::from_canonical)
                    .collect::<Vec<_>>();
                let mut edges = visibility_edges;
                edges.extend(ordinary.into_iter().map(|target| QueryEdge {
                    kind: QueryEdgeKind::Ordinary,
                    target,
                }));
                (
                    QueryNodeKind::Rule(CompactString::new("rule")),
                    edges,
                    project_attributes(implementation).to_vec(),
                )
            }
            PackageTargetKind::GeneratedFile {
                generating_rule, ..
            } => {
                let mut edges = vec![QueryEdge {
                    kind: QueryEdgeKind::GeneratingRule,
                    target: label_in_package(&package_name, generating_rule)?,
                }];
                edges.extend(visibility_edges);
                (QueryNodeKind::GeneratedFile, edges, Vec::new())
            }
            PackageTargetKind::PackageGroup { includes, .. } => (
                QueryNodeKind::PackageGroup,
                includes
                    .iter()
                    .cloned()
                    .map(|label| QueryEdge {
                        kind: QueryEdgeKind::PackageGroupInclude,
                        target: QueryLabel::from_canonical(label),
                    })
                    .collect(),
                Vec::new(),
            ),
        };
        if kind.is_rule() {
            attributes.push(project_visibility_attribute(target));
        }
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
                effective_visibility,
                visibility_source,
                package_group_contents,
                edges: edges.into(),
                attributes: attributes.into(),
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
                effective_visibility: loaded.default_visibility.clone(),
                visibility_source: VisibilitySource::PackageDefault,
                package_group_contents: None,
                edges: visibility_query_edges(&loaded.default_visibility),
                attributes: Arc::from([]),
            },
        );
    }

    // Attribute-created source nodes are owned by the package containing the
    // attribute. Cross-package labels remain edges: their destination package
    // graph is loaded only if traversal demands it.
    let referenced_sources = nodes
        .values()
        .flat_map(|node| node.edges.iter())
        .filter(|edge| edge.kind == QueryEdgeKind::Ordinary)
        .map(|edge| &edge.target)
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
                    effective_visibility: loaded.default_visibility.clone(),
                    visibility_source: VisibilitySource::PackageDefault,
                    package_group_contents: None,
                    edges: visibility_query_edges(&loaded.default_visibility),
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

fn external_package_graph_from_loaded(
    route: &RootRepositoryRoute,
    package: &PackagePath,
    loaded: &LoadedPackage,
) -> Result<UnconfiguredPackageGraph, QueryError> {
    external_package_graph_from_targets(
        route.canonical_repo(),
        route.apparent_repo(),
        package,
        &loaded.build_file,
        &loaded.default_visibility,
        &loaded.targets,
    )
}

fn external_package_graph_from_targets(
    canonical_repo: &CanonicalRepoName,
    apparent_repo: &ApparentRepoName,
    package: &PackagePath,
    build_path: &Path,
    default_visibility: &RuleVisibility,
    targets: &[slug_loading_v2::PackageTarget],
) -> Result<UnconfiguredPackageGraph, QueryError> {
    let build_basename = build_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            QueryError::evaluation("loaded external BUILD file has no UTF-8 basename")
        })?;
    let build_file = CompactString::new(build_path.to_string_lossy());
    let mut nodes = SmallMap::with_capacity(targets.len() + 1);

    for target in targets {
        let effective_visibility = match &target.visibility {
            VisibilitySource::Declared(visibility) => visibility.clone(),
            VisibilitySource::PackageDefault => default_visibility.clone(),
            VisibilitySource::AlwaysPublic => RuleVisibility::Public,
            VisibilitySource::GeneratingRule => {
                return Err(QueryError::evaluation(format!(
                    "target '{}' has invalid visibility provenance",
                    target.name
                )));
            }
        };
        if !effective_visibility.dependency_labels().is_empty() {
            return Err(QueryError::evaluation(format!(
                "external repository visibility edges are deferred: {}//{}:{}",
                canonical_repo, package, target.name
            )));
        }
        let label =
            QueryLabel::in_external_package(canonical_repo, apparent_repo, package, &target.name)?;
        let (kind, rule_capability, edges, attributes) = match &target.kind {
            PackageTargetKind::ExportedFile if target.name == build_basename => {
                (QueryNodeKind::BuildFile, None, Arc::from([]), Arc::from([]))
            }
            PackageTargetKind::ExportedFile => (
                QueryNodeKind::SourceFile,
                None,
                Arc::from([]),
                Arc::from([]),
            ),
            PackageTargetKind::Filegroup {
                srcs,
                srcs_explicit,
            } => {
                let labels = srcs
                    .iter()
                    .map(|source| {
                        external_filegroup_source_label(
                            canonical_repo,
                            apparent_repo,
                            package,
                            source,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let mut seen = SmallSet::new();
                let ordinary = labels
                    .iter()
                    .filter(|label| seen.insert((*label).dupe()))
                    .map(QueryLabel::dupe)
                    .map(|target| QueryEdge {
                        kind: QueryEdgeKind::Ordinary,
                        target,
                    })
                    .collect::<Vec<_>>();
                (
                    QueryNodeKind::Rule(CompactString::new("filegroup rule")),
                    target.rule_capability().cloned(),
                    ordinary.into(),
                    vec![QueryAttribute {
                        name: CompactString::new("srcs"),
                        labels: labels.into(),
                        explicit: *srcs_explicit,
                    }]
                    .into(),
                )
            }
            _ => {
                return Err(QueryError::evaluation(format!(
                    "external repository rule graph is deferred: {}//{}:{}",
                    canonical_repo, package, target.name
                )));
            }
        };
        if target.name == build_basename && !matches!(kind, QueryNodeKind::BuildFile) {
            return Err(QueryError::evaluation(format!(
                "target '{}' collides with active BUILD file",
                label
            )));
        };
        nodes.insert(
            label.dupe(),
            QueryNode {
                label,
                kind,
                rule_capability,
                test_metadata: None,
                build_file: build_file.clone(),
                effective_visibility,
                visibility_source: target.visibility.clone(),
                package_group_contents: None,
                edges,
                attributes,
            },
        );
    }

    if !default_visibility.dependency_labels().is_empty() {
        return Err(QueryError::evaluation(format!(
            "external repository default visibility edges are deferred: {}//{}",
            canonical_repo, package
        )));
    }
    let build_label =
        QueryLabel::in_external_package(canonical_repo, apparent_repo, package, build_basename)?;
    if nodes.get(&build_label).is_none() {
        nodes.insert(
            build_label.dupe(),
            QueryNode {
                label: build_label,
                kind: QueryNodeKind::BuildFile,
                rule_capability: None,
                test_metadata: None,
                build_file: build_file.clone(),
                effective_visibility: default_visibility.clone(),
                visibility_source: VisibilitySource::PackageDefault,
                package_group_contents: None,
                edges: Arc::from([]),
                attributes: Arc::from([]),
            },
        );
    }

    // Native filegroup attributes create same-package source targets during
    // loading. This query projection deliberately retains that semantic
    // result without observing the source path.
    let referenced_sources = nodes
        .values()
        .flat_map(|node| node.edges.iter())
        .filter(|edge| edge.kind == QueryEdgeKind::Ordinary)
        .map(|edge| &edge.target)
        .filter(|label| !label.is_root_repository() && label.package() == package.as_str())
        .filter(|label| nodes.get(*label).is_none())
        .map(QueryLabel::dupe)
        .collect::<SmallSet<_>>();
    for label in referenced_sources {
        nodes.insert(
            label.dupe(),
            QueryNode {
                label,
                kind: QueryNodeKind::SourceFile,
                rule_capability: None,
                test_metadata: None,
                build_file: build_file.clone(),
                effective_visibility: default_visibility.clone(),
                visibility_source: VisibilitySource::PackageDefault,
                package_group_contents: None,
                edges: Arc::from([]),
                attributes: Arc::from([]),
            },
        );
    }

    Ok(UnconfiguredPackageGraph {
        package: CompactString::new(package.as_str()),
        nodes,
    })
}

fn external_filegroup_source_label(
    canonical_repo: &CanonicalRepoName,
    apparent_repo: &ApparentRepoName,
    package: &PackagePath,
    source: &CanonicalLabel,
) -> Result<QueryLabel, QueryError> {
    let source_package = source.package();
    if source_package.repo().is_root() && source_package.package() == package {
        return QueryLabel::in_external_package(
            canonical_repo,
            apparent_repo,
            package,
            source.target().as_str(),
        );
    }
    let deferred = if source_package.repo().is_root() {
        "cross-package"
    } else {
        "named-repository"
    };
    Err(QueryError::evaluation(format!(
        "external repository filegroup {deferred} srcs are deferred: {source}"
    )))
}

fn effective_visibility(
    loaded: &slug_loading_v2::LoadedPackage,
    target: &slug_loading_v2::PackageTarget,
) -> Result<RuleVisibility, QueryError> {
    loaded.effective_visibility(target).ok_or_else(|| {
        QueryError::evaluation(format!(
            "target '{}' has invalid visibility provenance",
            target.name
        ))
    })
}

fn visibility_query_edges(visibility: &RuleVisibility) -> Arc<[QueryEdge]> {
    visibility
        .dependency_labels()
        .iter()
        .cloned()
        .map(|label| QueryEdge {
            kind: QueryEdgeKind::VisibilityNodep,
            target: QueryLabel::from_canonical(label),
        })
        .collect::<Vec<_>>()
        .into()
}

fn project_visibility_attribute(target: &slug_loading_v2::PackageTarget) -> QueryAttribute {
    let (labels, explicit) = match &target.visibility {
        VisibilitySource::Declared(visibility) => (
            visibility
                .raw_declared_labels()
                .iter()
                .cloned()
                .map(QueryLabel::from_canonical)
                .collect::<Vec<_>>(),
            true,
        ),
        VisibilitySource::PackageDefault
        | VisibilitySource::GeneratingRule
        | VisibilitySource::AlwaysPublic => (Vec::new(), false),
    };
    QueryAttribute {
        name: CompactString::new("visibility"),
        labels: labels.into(),
        explicit,
    }
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

#[async_trait]
impl Key for RootSubtreePackageSetKey {
    type Value = LoadingPreparationOutcome<PackageSetValue>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        compute_root_subtree_packages(ctx, &self.workspace, &self.prefix).await
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

fn union_source_need(
    accumulated: &mut Option<SourcePreparationNeeds>,
    next: SourcePreparationNeeds,
) {
    *accumulated = Some(match accumulated.take() {
        Some(existing) => existing
            .try_union(&next)
            .expect("root query source Needs must be compatible"),
        None => next,
    });
}

async fn compute_root_subtree_packages(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    prefix: &PackagePath,
) -> LoadingPreparationOutcome<PackageSetValue> {
    let roots = match ctx
        .compute(&RootPackageLookupInputsProjectionKey::new(
            workspace.clone(),
        ))
        .await
    {
        Err(error) => {
            return LoadingPreparationOutcome::Complete(Arc::new(Err(QueryError::evaluation(
                error.to_string(),
            ))));
        }
        Ok(Err(error)) => {
            return LoadingPreparationOutcome::Complete(Arc::new(Err(QueryError::evaluation(
                error.to_string(),
            ))));
        }
        Ok(Ok(inputs)) => inputs.package_roots().to_vec(),
    };

    let mut pending = vec![PathBuf::from(prefix.as_str())];
    let mut packages = Vec::new();
    while let Some(relative) = pending.pop() {
        let package_text = relative.to_str().map(|value| value.replace('\\', "/"));
        if let Some(package_text) = package_text.as_deref() {
            let package = match PackagePath::parse(package_text) {
                Ok(package) => package,
                Err(error) => {
                    return LoadingPreparationOutcome::Complete(Arc::new(Err(
                        QueryError::evaluation(error),
                    )));
                }
            };
            let boundary = ctx
                .compute(&HostRootPackageBoundaryKey::new(workspace.clone(), package))
                .await
                .expect("Host package-boundary DICE invariant");
            match boundary {
                PathOutcome::Need(need) => {
                    return LoadingPreparationOutcome::Need(SourcePreparationNeeds::path(need));
                }
                PathOutcome::Complete(value) => match value.as_ref() {
                    Err(error) => {
                        return LoadingPreparationOutcome::Complete(Arc::new(Err(
                            QueryError::evaluation(error.to_string()),
                        )));
                    }
                    Ok(boundary)
                        if boundary.kind() == HostRootPackageBoundaryKind::IgnoredDirectory =>
                    {
                        continue;
                    }
                    Ok(boundary) if boundary.kind() == HostRootPackageBoundaryKind::Package => {
                        packages.push(CompactString::new(package_text));
                    }
                    Ok(_) => {}
                },
            }
        } else {
            match probe_native_package_marker(ctx, &roots, &relative).await {
                LoadingPreparationOutcome::Need(need) => {
                    return LoadingPreparationOutcome::Need(need);
                }
                LoadingPreparationOutcome::Complete(Err(error)) => {
                    return LoadingPreparationOutcome::Complete(Arc::new(Err(error)));
                }
                LoadingPreparationOutcome::Complete(Ok(true)) => {
                    return LoadingPreparationOutcome::Complete(Arc::new(Err(
                        QueryError::evaluation(format!(
                            "package path is not UTF-8: {}",
                            relative.display()
                        )),
                    )));
                }
                LoadingPreparationOutcome::Complete(Ok(false)) => {}
            }
        }

        let mut needs = None;
        let mut first_error = None;
        let mut children = Vec::new();
        let listings = ctx
            .compute_join(roots.iter().cloned(), |ctx, root| {
                let logical = NormalizedAbsolutePath::new(root.as_path().join(&relative))
                    .expect("package-root child remains absolute");
                async move {
                    ctx.compute(&PathDirectoryListingKey::new(
                        PathObservationNamespace::Host,
                        logical,
                    ))
                    .await
                }
                .boxed()
            })
            .await;
        for listing in listings {
            match listing {
                Err(error) => {
                    first_error.get_or_insert_with(|| QueryError::evaluation(error.to_string()));
                }
                Ok(PathOutcome::Need(need)) => {
                    union_source_need(&mut needs, SourcePreparationNeeds::path(need));
                }
                Ok(PathOutcome::Complete(Err(error))) => {
                    first_error.get_or_insert_with(|| {
                        QueryError::evaluation(format!(
                            "reading workspace directory {}: {error:?}",
                            relative.display()
                        ))
                    });
                }
                Ok(PathOutcome::Complete(Ok(PathDirectoryListing::Missing))) => {}
                Ok(PathOutcome::Complete(Ok(PathDirectoryListing::Present(entries)))) => {
                    children.extend(
                        entries
                            .entries()
                            .iter()
                            .filter(|entry| entry.kind() == PathDirectoryEntryKind::Directory)
                            .map(|entry| relative.join(entry.name().as_os_str())),
                    );
                }
            }
        }
        if let Some(need) = needs {
            return LoadingPreparationOutcome::Need(need);
        }
        if let Some(error) = first_error {
            return LoadingPreparationOutcome::Complete(Arc::new(Err(error)));
        }
        children.sort_unstable();
        children.dedup();
        pending.extend(children.into_iter().rev());
    }
    packages.sort_unstable();
    packages.dedup();
    LoadingPreparationOutcome::Complete(Arc::new(Ok(SubtreePackageSet {
        packages: packages.into(),
    })))
}

async fn probe_native_package_marker(
    ctx: &mut DiceComputations<'_>,
    roots: &[NormalizedAbsolutePath],
    relative: &Path,
) -> LoadingPreparationOutcome<Result<bool, QueryError>> {
    let probes = roots
        .iter()
        .flat_map(|root| {
            ["BUILD.bazel", "BUILD"].map(|basename| {
                NormalizedAbsolutePath::new(root.as_path().join(relative).join(basename))
                    .expect("package marker remains absolute")
            })
        })
        .collect::<Vec<_>>();
    let results = ctx
        .compute_join(probes, |ctx, marker| {
            async move {
                ctx.compute(&ResolvedPathKey::new(
                    PathObservationNamespace::Host,
                    marker,
                ))
                .await
            }
            .boxed()
        })
        .await;
    let mut needs = None;
    let mut completed = Vec::new();
    for result in results {
        match result {
            Err(error) => {
                completed.push(Err(QueryError::evaluation(error.to_string())));
            }
            Ok(PathOutcome::Need(need)) => {
                union_source_need(&mut needs, SourcePreparationNeeds::path(need));
            }
            Ok(PathOutcome::Complete(Err(error))) => {
                completed.push(Err(QueryError::evaluation(format!("{error:?}"))));
            }
            Ok(PathOutcome::Complete(Ok(resolved))) => {
                completed.push(Ok(matches!(
                    resolved.state(),
                    ResolvedPathState::Present(lstat)
                        if matches!(
                            lstat.kind(),
                            PathNodeKind::RegularFile | PathNodeKind::SpecialFile
                        )
                )));
            }
        }
    }
    if let Some(need) = needs {
        return LoadingPreparationOutcome::Need(need);
    }
    for result in completed {
        match result {
            Err(error) => return LoadingPreparationOutcome::Complete(Err(error)),
            Ok(true) => return LoadingPreparationOutcome::Complete(Ok(true)),
            Ok(false) => {}
        }
    }
    LoadingPreparationOutcome::Complete(Ok(false))
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

#[cfg(test)]
mod graph_tests {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hash;
    use std::hash::Hasher;
    use std::path::Path;
    use std::sync::Arc;

    use slug_identity_v2::ApparentLabel;
    use slug_identity_v2::ApparentRepoName;
    use slug_identity_v2::CanonicalLabel;
    use slug_identity_v2::CanonicalRepoName;
    use slug_identity_v2::PackagePath;
    use slug_loading_v2::PackageTarget;
    use slug_loading_v2::PackageTargetKind;
    use slug_loading_v2::RuleVisibility;
    use slug_loading_v2::VisibilitySource;

    use super::QueryEdgeKind;
    use super::QueryLabel;
    use super::QueryNodeKind;
    use super::external_package_graph_from_targets;

    #[test]
    fn external_label_identity_is_canonical_while_output_is_apparent() {
        let canonical = CanonicalRepoName::new("dep+").unwrap();
        let dep = QueryLabel::from_apparent_route(
            &ApparentLabel::parse("@dep//pkg:target").unwrap(),
            &canonical,
        )
        .unwrap();
        let alias = QueryLabel::from_apparent_route(
            &ApparentLabel::parse("@alias//pkg:target").unwrap(),
            &canonical,
        )
        .unwrap();
        let hash = |label: &QueryLabel| {
            let mut state = DefaultHasher::new();
            label.hash(&mut state);
            state.finish()
        };

        assert_eq!(dep, alias);
        assert_eq!(dep.cmp(&alias), std::cmp::Ordering::Equal);
        assert_eq!(hash(&dep), hash(&alias));
        assert_eq!(dep.to_string(), "@@dep+//pkg:target");
        assert_eq!(alias.to_string(), "@@dep+//pkg:target");
        assert_eq!(dep.output_label(), "@dep//pkg:target");
        assert_eq!(alias.output_label(), "@alias//pkg:target");
    }

    #[test]
    fn external_filegroup_projection_retains_srcs_and_synthesizes_sources() {
        let canonical_repo = CanonicalRepoName::new("dep+").unwrap();
        let apparent_repo = ApparentRepoName::new("dep").unwrap();
        let package = PackagePath::parse("").unwrap();
        let source = |target| CanonicalLabel::parse(&format!("@@//:{target}")).unwrap();
        let targets = vec![
            PackageTarget {
                name: "existing.txt".to_owned(),
                kind: PackageTargetKind::ExportedFile,
                visibility: VisibilitySource::AlwaysPublic,
            },
            PackageTarget {
                name: "files".to_owned(),
                kind: PackageTargetKind::Filegroup {
                    srcs: Arc::from([
                        source("z.txt"),
                        source("existing.txt"),
                        source("absent.txt"),
                    ]),
                    srcs_explicit: true,
                },
                visibility: VisibilitySource::PackageDefault,
            },
            PackageTarget {
                name: "omitted".to_owned(),
                kind: PackageTargetKind::Filegroup {
                    srcs: Arc::from([]),
                    srcs_explicit: false,
                },
                visibility: VisibilitySource::PackageDefault,
            },
        ];
        let graph = external_package_graph_from_targets(
            &canonical_repo,
            &apparent_repo,
            &package,
            Path::new("/external/dep+/BUILD.bazel"),
            &RuleVisibility::Private,
            &targets,
        )
        .unwrap();
        let label = |target| {
            QueryLabel::in_external_package(&canonical_repo, &apparent_repo, &package, target)
                .unwrap()
        };

        let files = graph.nodes.get(&label("files")).unwrap();
        assert_eq!(files.kind, QueryNodeKind::Rule("filegroup rule".into()));
        assert_eq!(files.attributes.len(), 1);
        assert_eq!(files.attributes[0].name, "srcs");
        assert!(files.attributes[0].explicit);
        assert_eq!(
            files.attributes[0]
                .labels
                .iter()
                .map(QueryLabel::output_label)
                .collect::<Vec<_>>(),
            ["@dep//:z.txt", "@dep//:existing.txt", "@dep//:absent.txt"]
        );
        assert_eq!(
            files
                .edges
                .iter()
                .map(|edge| (edge.kind, edge.target.output_label()))
                .collect::<Vec<_>>(),
            [
                (QueryEdgeKind::Ordinary, "@dep//:z.txt".into()),
                (QueryEdgeKind::Ordinary, "@dep//:existing.txt".into()),
                (QueryEdgeKind::Ordinary, "@dep//:absent.txt".into()),
            ]
        );
        assert!(
            files
                .attributes
                .iter()
                .all(|attribute| attribute.name != "visibility")
        );

        let omitted = graph.nodes.get(&label("omitted")).unwrap();
        assert_eq!(omitted.attributes.len(), 1);
        assert!(omitted.attributes[0].labels.is_empty());
        assert!(!omitted.attributes[0].explicit);

        let existing = graph.nodes.get(&label("existing.txt")).unwrap();
        assert_eq!(existing.kind, QueryNodeKind::SourceFile);
        assert_eq!(existing.effective_visibility, RuleVisibility::Public);
        assert_eq!(existing.visibility_source, VisibilitySource::AlwaysPublic);

        // This pure projection receives only loaded target metadata. The
        // undeclared source exists because `srcs` names it, with no source-path
        // observation or filesystem discovery.
        let absent = graph.nodes.get(&label("absent.txt")).unwrap();
        assert_eq!(absent.kind, QueryNodeKind::SourceFile);
        assert_eq!(absent.effective_visibility, RuleVisibility::Private);
        assert_eq!(absent.visibility_source, VisibilitySource::PackageDefault);
        assert!(absent.edges.is_empty());
        assert!(absent.attributes.is_empty());
    }
}
