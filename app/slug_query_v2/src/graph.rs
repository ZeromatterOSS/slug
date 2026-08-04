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

use crate::provenance::QueryPackageIdentity;

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

    pub(crate) fn owner_identity(&self) -> Result<QueryPackageIdentity, QueryError> {
        let canonical_repo = self.canonical.package().repo();
        let package = self.canonical.package().package();
        match (canonical_repo.is_root(), self.apparent_repo.as_deref()) {
            (true, None) => Ok(QueryPackageIdentity::root(package.clone())),
            (false, Some(apparent_repo)) if !apparent_repo.is_root() => {
                QueryPackageIdentity::external(
                    canonical_repo.clone(),
                    apparent_repo.clone(),
                    package.clone(),
                )
            }
            (true, Some(_)) => Err(QueryError::evaluation(
                "root query label unexpectedly retained an apparent repository route",
            )),
            (false, None) => Err(QueryError::evaluation(
                "external query label lost its apparent repository route",
            )),
            (false, Some(_)) => Err(QueryError::evaluation(
                "external query label retained the root apparent repository route",
            )),
        }
    }

    pub(crate) fn in_owner_package(
        owner: &QueryPackageIdentity,
        target: &str,
    ) -> Result<Self, QueryError> {
        match owner.apparent_repo() {
            None => Self::parse_root(&format!("//{}:{target}", owner.package().as_str())),
            Some(apparent_repo) => Self::in_external_package(
                owner
                    .canonical_repo()
                    .expect("external owner retains canonical repository"),
                apparent_repo,
                owner.package(),
                target,
            ),
        }
    }

    pub(crate) fn from_canonical_in_owner(
        label: &CanonicalLabel,
        owner: &QueryPackageIdentity,
    ) -> Result<Self, QueryError> {
        let owner_package = owner.canonical_package();
        if label.package() != &owner_package {
            return Err(QueryError::evaluation(format!(
                "query label package '{}' does not match consuming owner '{}'",
                label.package(),
                owner_package
            )));
        }
        Self::in_owner_package(owner, label.target().as_str())
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

    validate_external_starlark_rule(targets)?;
    // Validate native test-suite membership before the generic same-package
    // source synthesis below. A test_suite may name only another loaded native
    // test_suite in this external slice, so it never causes a source node to
    // appear or admits an unsupported `Other` member to `tests()`.
    validate_external_test_suite_memberships(package, targets)?;
    // Package-group includes are the only retained external group traversal.
    // Validate them before generic source synthesis so an include can neither
    // discover a source nor take a permissive edge fallback.
    validate_external_package_group_includes(package, targets)?;
    validate_external_restricted_visibility(package, default_visibility, targets)?;

    for target in targets {
        let target_effective_visibility = match &target.visibility {
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
        let (effective_visibility, visibility_source) = match &target.kind {
            PackageTargetKind::PackageGroup { .. } => {
                (RuleVisibility::Public, VisibilitySource::AlwaysPublic)
            }
            _ => {
                let effective_visibility = target_effective_visibility
                    .in_repository_context(canonical_repo)
                    .map_err(|error| QueryError::evaluation(error.to_string()))?;
                let visibility_source = match &target.visibility {
                    VisibilitySource::Declared(visibility) => VisibilitySource::Declared(
                        visibility
                            .in_repository_context(canonical_repo)
                            .map_err(|error| QueryError::evaluation(error.to_string()))?,
                    ),
                    source => source.clone(),
                };
                (effective_visibility, visibility_source)
            }
        };
        let label =
            QueryLabel::in_external_package(canonical_repo, apparent_repo, package, &target.name)?;
        let rule_capability = target.rule_capability().cloned();
        let test_metadata = target.test_metadata();
        let (kind, edges, attributes) = match &target.kind {
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
                let visibility_labels = effective_visibility
                    .dependency_labels()
                    .iter()
                    .map(|label| {
                        QueryLabel::in_external_package(
                            canonical_repo,
                            apparent_repo,
                            package,
                            label.target().as_str(),
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let mut edges = visibility_labels
                    .iter()
                    .cloned()
                    .map(|target| QueryEdge {
                        kind: QueryEdgeKind::VisibilityNodep,
                        target,
                    })
                    .collect::<Vec<_>>();
                edges.extend(ordinary);
                let mut attributes = vec![QueryAttribute {
                    name: CompactString::new("srcs"),
                    labels: labels.into(),
                    explicit: *srcs_explicit,
                }];
                if matches!(
                    &visibility_source,
                    VisibilitySource::Declared(RuleVisibility::Restricted(_))
                ) {
                    attributes.push(QueryAttribute {
                        name: CompactString::new("visibility"),
                        labels: visibility_labels.into(),
                        explicit: true,
                    });
                }
                (
                    QueryNodeKind::Rule(CompactString::new("filegroup rule")),
                    edges.into(),
                    attributes.into(),
                )
            }
            PackageTargetKind::Alias { actual } => {
                let actual =
                    external_alias_actual_label(canonical_repo, apparent_repo, package, actual)?;
                (
                    QueryNodeKind::Rule(CompactString::new("alias rule")),
                    Arc::from([QueryEdge {
                        kind: QueryEdgeKind::Ordinary,
                        target: actual.dupe(),
                    }]),
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
                    .map(|member| {
                        external_test_suite_member_label(
                            canonical_repo,
                            apparent_repo,
                            package,
                            member,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let implicit_tests = membership
                    .implicit_tests()
                    .iter()
                    .map(|member| {
                        external_test_suite_member_label(
                            canonical_repo,
                            apparent_repo,
                            package,
                            member,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let mut seen = SmallSet::new();
                let ordinary = tests
                    .iter()
                    .chain(implicit_tests.iter())
                    .filter(|label| seen.insert((*label).dupe()))
                    .map(QueryLabel::dupe)
                    .map(|target| QueryEdge {
                        kind: QueryEdgeKind::Ordinary,
                        target,
                    })
                    .collect::<Vec<_>>();
                (
                    QueryNodeKind::Rule(CompactString::new("test_suite rule")),
                    ordinary.into(),
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
                    ]
                    .into(),
                )
            }
            PackageTargetKind::PackageGroup { includes, .. } => {
                let includes = includes
                    .iter()
                    .map(|include| {
                        external_package_group_include_label(
                            canonical_repo,
                            apparent_repo,
                            package,
                            include,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                (
                    QueryNodeKind::PackageGroup,
                    includes
                        .into_iter()
                        .map(|target| QueryEdge {
                            kind: QueryEdgeKind::PackageGroupInclude,
                            target,
                        })
                        .collect::<Vec<_>>()
                        .into(),
                    Arc::from([]),
                )
            }
            PackageTargetKind::StarlarkRule(implementation) => {
                let mut attributes = project_attributes(implementation).to_vec();
                attributes.push(project_visibility_attribute(target));
                (
                    QueryNodeKind::Rule(CompactString::new("rule")),
                    Arc::from([]),
                    attributes.into(),
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
                test_metadata,
                build_file: build_file.clone(),
                effective_visibility,
                visibility_source,
                package_group_contents: match &target.kind {
                    PackageTargetKind::PackageGroup { contents, .. } => Some(Arc::new(
                        contents
                            .in_repository_context(canonical_repo)
                            .map_err(|error| QueryError::evaluation(error.to_string()))?,
                    )),
                    _ => None,
                },
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

    // Accepted native label attributes create same-package source targets
    // during loading. This query projection retains that semantic result for
    // filegroup `srcs` and alias `actual` without observing the source path.
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

    for node in nodes.values() {
        if !matches!(&node.kind, QueryNodeKind::Rule(rule) if rule == "alias rule") {
            continue;
        }
        let actual = node
            .edges
            .iter()
            .find(|edge| edge.kind == QueryEdgeKind::Ordinary)
            .map(|edge| &edge.target)
            .expect("external alias projection has exactly one ordinary edge");
        match &nodes
            .get(actual)
            .expect("same-package external alias actual is projected")
            .kind
        {
            QueryNodeKind::SourceFile => {}
            QueryNodeKind::Rule(rule) if rule == "filegroup rule" => {}
            QueryNodeKind::Rule(rule) if rule == "alias rule" => {
                return Err(QueryError::evaluation(format!(
                    "external repository alias chains are deferred: {}",
                    node.label
                )));
            }
            _ => {
                return Err(QueryError::evaluation(format!(
                    "external repository alias actual destination is deferred: {}",
                    node.label
                )));
            }
        }
    }

    Ok(UnconfiguredPackageGraph {
        package: CompactString::new(package.as_str()),
        nodes,
    })
}

fn validate_external_starlark_rule(
    targets: &[slug_loading_v2::PackageTarget],
) -> Result<(), QueryError> {
    let Some(target) = targets
        .iter()
        .find(|target| matches!(target.kind, PackageTargetKind::StarlarkRule(_)))
    else {
        return Ok(());
    };
    let fail = |reason| {
        QueryError::evaluation(format!(
            "external Starlark rule graph is deferred for '{}': {reason}",
            target.name
        ))
    };
    let PackageTargetKind::StarlarkRule(implementation) = &target.kind else {
        unreachable!()
    };
    if !matches!(
        &target.visibility,
        VisibilitySource::Declared(RuleVisibility::Public)
    ) {
        return Err(fail("visibility is not explicitly public"));
    }
    let capability = target
        .rule_capability()
        .expect("Starlark rule retains its exported capability");
    if capability.test_kind.is_some() || capability.executable {
        return Err(fail("test or executable capability is deferred"));
    }
    if !implementation.dependencies().is_empty() {
        return Err(fail("ordinary dependencies are deferred"));
    }
    if implementation.schema().len() != implementation.values().len()
        || implementation
            .schema()
            .iter()
            .zip(implementation.values())
            .any(|(schema, value)| schema.declaration_name() != value.declaration_name)
    {
        return Err(fail("schema/value relationship is malformed"));
    }
    for (schema, value) in implementation.schema().iter().zip(implementation.values()) {
        if !schema.dependency_reachable() {
            continue;
        }
        let mut labels = Vec::new();
        value.value.labels(&mut labels);
        if !labels.is_empty() {
            return Err(fail("dependency-reachable attribute contains a label"));
        }
    }
    if targets.len() != 1 {
        return Err(fail("package contains additional targets"));
    }
    Ok(())
}

fn validate_external_package_group_includes(
    package: &PackagePath,
    targets: &[slug_loading_v2::PackageTarget],
) -> Result<(), QueryError> {
    for target in targets {
        let PackageTargetKind::PackageGroup { includes, .. } = &target.kind else {
            continue;
        };
        for include in includes.iter() {
            let include_package = include.package();
            if !include_package.repo().is_root() || include_package.package() != package {
                let deferred = if include_package.repo().is_root() {
                    "cross-package"
                } else {
                    "named-repository"
                };
                return Err(QueryError::evaluation(format!(
                    "external repository package_group {deferred} include is deferred: {include}"
                )));
            }
            let Some(include_target) = targets
                .iter()
                .find(|candidate| candidate.name == include.target().as_str())
            else {
                return Err(QueryError::evaluation(format!(
                    "external repository package_group missing include is deferred: {include}"
                )));
            };
            match &include_target.kind {
                PackageTargetKind::PackageGroup { .. } => {}
                PackageTargetKind::Alias { .. } => {
                    return Err(QueryError::evaluation(format!(
                        "external repository package_group alias include is deferred: {include}"
                    )));
                }
                _ => {
                    return Err(QueryError::evaluation(format!(
                        "external repository package_group non-package-group include is deferred: {include}"
                    )));
                }
            }
        }
    }
    Ok(())
}

fn validate_external_restricted_visibility(
    package: &PackagePath,
    default_visibility: &RuleVisibility,
    targets: &[slug_loading_v2::PackageTarget],
) -> Result<(), QueryError> {
    if matches!(default_visibility, RuleVisibility::Restricted(_)) {
        return Err(QueryError::evaluation(
            "external repository Restricted package default visibility is deferred",
        ));
    }
    let mut protected = None;
    for target in targets {
        let VisibilitySource::Declared(RuleVisibility::Restricted(visibility)) = &target.visibility
        else {
            continue;
        };
        if protected.replace(target.name.as_str()).is_some() {
            return Err(QueryError::evaluation(
                "a second external repository Restricted target is deferred",
            ));
        }
        if !matches!(target.kind, PackageTargetKind::Filegroup { .. }) {
            return Err(QueryError::evaluation(format!(
                "external repository Restricted visibility is deferred for non-filegroup '{}'",
                target.name
            )));
        }
        if visibility.package_groups().is_empty()
            || visibility.declared_labels().len() != visibility.package_groups().len()
        {
            return Err(QueryError::evaluation(format!(
                "external repository direct package visibility is deferred: {}",
                target.name
            )));
        }
        for group in visibility.package_groups() {
            let group_package = group.package();
            if !group_package.repo().is_root() || group_package.package() != package {
                let deferred = if group_package.repo().is_root() {
                    "cross-package"
                } else {
                    "named-repository"
                };
                return Err(QueryError::evaluation(format!(
                    "external repository visibility {deferred} group is deferred: {group}"
                )));
            }
            let Some(group_target) = targets
                .iter()
                .find(|candidate| candidate.name == group.target().as_str())
            else {
                return Err(QueryError::evaluation(format!(
                    "external repository visibility missing group is deferred: {group}"
                )));
            };
            match &group_target.kind {
                PackageTargetKind::PackageGroup { .. } => {}
                PackageTargetKind::Alias { .. } => {
                    return Err(QueryError::evaluation(format!(
                        "external repository visibility alias group is deferred: {group}"
                    )));
                }
                _ => {
                    return Err(QueryError::evaluation(format!(
                        "external repository visibility wrong-kind group is deferred: {group}"
                    )));
                }
            }
        }
    }
    Ok(())
}

fn external_package_group_include_label(
    canonical_repo: &CanonicalRepoName,
    apparent_repo: &ApparentRepoName,
    package: &PackagePath,
    include: &CanonicalLabel,
) -> Result<QueryLabel, QueryError> {
    let include_package = include.package();
    if include_package.repo().is_root() && include_package.package() == package {
        return QueryLabel::in_external_package(
            canonical_repo,
            apparent_repo,
            package,
            include.target().as_str(),
        );
    }
    Err(QueryError::evaluation(format!(
        "external repository package_group include is deferred: {include}"
    )))
}

fn validate_external_test_suite_memberships(
    package: &PackagePath,
    targets: &[slug_loading_v2::PackageTarget],
) -> Result<(), QueryError> {
    for target in targets {
        let PackageTargetKind::TestSuite { membership, .. } = &target.kind else {
            continue;
        };
        if !membership.implicit_tests().is_empty() {
            return Err(QueryError::evaluation(format!(
                "external repository test_suite implicit tests are deferred: {}",
                target.name
            )));
        }
        for member in membership.tests() {
            let member_package = member.package();
            if !member_package.repo().is_root() || member_package.package() != package {
                let deferred = if member_package.repo().is_root() {
                    "cross-package"
                } else {
                    "named-repository"
                };
                return Err(QueryError::evaluation(format!(
                    "external repository test_suite {deferred} member is deferred: {member}"
                )));
            }
            let Some(member_target) = targets
                .iter()
                .find(|candidate| candidate.name == member.target().as_str())
            else {
                return Err(QueryError::evaluation(format!(
                    "external repository test_suite unresolved member is deferred: {member}"
                )));
            };
            if !matches!(member_target.kind, PackageTargetKind::TestSuite { .. }) {
                return Err(QueryError::evaluation(format!(
                    "external repository test_suite non-suite member is deferred: {member}"
                )));
            }
        }
    }
    Ok(())
}

fn external_test_suite_member_label(
    canonical_repo: &CanonicalRepoName,
    apparent_repo: &ApparentRepoName,
    package: &PackagePath,
    member: &CanonicalLabel,
) -> Result<QueryLabel, QueryError> {
    let member_package = member.package();
    if member_package.repo().is_root() && member_package.package() == package {
        return QueryLabel::in_external_package(
            canonical_repo,
            apparent_repo,
            package,
            member.target().as_str(),
        );
    }
    Err(QueryError::evaluation(format!(
        "external repository test_suite member is deferred: {member}"
    )))
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

fn external_alias_actual_label(
    canonical_repo: &CanonicalRepoName,
    apparent_repo: &ApparentRepoName,
    package: &PackagePath,
    actual: &CanonicalLabel,
) -> Result<QueryLabel, QueryError> {
    let actual_package = actual.package();
    if actual_package.repo().is_root() && actual_package.package() == package {
        return QueryLabel::in_external_package(
            canonical_repo,
            apparent_repo,
            package,
            actual.target().as_str(),
        );
    }
    let deferred = if actual_package.repo().is_root() {
        "cross-package"
    } else {
        "named-repository"
    };
    Err(QueryError::evaluation(format!(
        "external repository alias {deferred} actual is deferred: {actual}"
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
    use std::fs;
    use std::hash::Hash;
    use std::hash::Hasher;
    use std::path::Path;
    use std::sync::Arc;
    use std::sync::atomic::AtomicU64;
    use std::sync::atomic::Ordering;
    use std::time::SystemTime;

    use dice::DetectCycles;
    use dice::Dice;
    use slug_bzlmod_v2::BzlmodCommandPolicyKey;
    use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
    use slug_bzlmod_v2::LockfileMode;
    use slug_bzlmod_v2::inject_root_module_request_inputs;
    use slug_identity_v2::ApparentLabel;
    use slug_identity_v2::ApparentRepoName;
    use slug_identity_v2::CanonicalLabel;
    use slug_identity_v2::CanonicalRepoName;
    use slug_identity_v2::PackagePath;
    use slug_loading_v2::BzlModuleEvaluator;
    use slug_loading_v2::PackageTarget;
    use slug_loading_v2::PackageTargetKind;
    use slug_loading_v2::RuleVisibility;
    use slug_loading_v2::TestRuleKind;
    use slug_loading_v2::TestSuiteMembership;
    use slug_loading_v2::VisibilitySource;
    use slug_loading_v2::keys::WorkspaceDirectoryEntry;
    use slug_loading_v2::keys::WorkspaceDirectoryEntryKind;
    use slug_loading_v2::keys::WorkspaceDirectoryValue;
    use slug_workspace_v2::WorkspaceDirectorySnapshot;
    use slug_workspace_v2::WorkspaceDirectorySnapshotKey;
    use slug_workspace_v2::WorkspaceFileValue;
    use slug_workspace_v2::WorkspaceRawFileValue;
    use slug_workspace_v2::WorkspaceRawSnapshot;
    use slug_workspace_v2::WorkspaceRawSnapshotKey;
    use slug_workspace_v2::WorkspaceSnapshot;
    use slug_workspace_v2::WorkspaceSnapshotKey;

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
    fn explicit_public_visibility_projects_an_explicit_empty_attribute() {
        let attribute = super::project_visibility_attribute(&PackageTarget {
            name: "probe".to_owned(),
            kind: PackageTargetKind::Filegroup {
                srcs: Arc::from([]),
                srcs_explicit: false,
            },
            visibility: VisibilitySource::Declared(RuleVisibility::Public),
        });
        assert_eq!(attribute.name, "visibility");
        assert!(attribute.explicit);
        assert!(attribute.labels.is_empty());
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
            PackageTarget {
                name: "files_alias".to_owned(),
                kind: PackageTargetKind::Alias {
                    actual: source("files"),
                },
                visibility: VisibilitySource::PackageDefault,
            },
            PackageTarget {
                name: "is_k8".to_owned(),
                kind: PackageTargetKind::ConfigSetting {
                    values: Arc::from([("cpu".into(), "k8".into())]),
                },
                visibility: VisibilitySource::PackageDefault,
            },
            PackageTarget {
                name: "public_k8".to_owned(),
                kind: PackageTargetKind::ConfigSetting {
                    values: Arc::from([("cpu".into(), "k8".into())]),
                },
                visibility: VisibilitySource::AlwaysPublic,
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

        let alias = graph.nodes.get(&label("files_alias")).unwrap();
        assert_eq!(alias.kind, QueryNodeKind::Rule("alias rule".into()));
        assert_eq!(
            alias
                .rule_capability
                .as_ref()
                .map(|capability| capability.rule_class.as_str()),
            Some("alias")
        );
        assert_eq!(alias.attributes.len(), 1);
        assert_eq!(alias.attributes[0].name, "actual");
        assert!(alias.attributes[0].explicit);
        assert_eq!(
            alias.attributes[0]
                .labels
                .iter()
                .map(QueryLabel::output_label)
                .collect::<Vec<_>>(),
            ["@dep//:files"]
        );
        assert_eq!(
            alias
                .edges
                .iter()
                .map(|edge| (edge.kind, edge.target.output_label()))
                .collect::<Vec<_>>(),
            [(QueryEdgeKind::Ordinary, "@dep//:files".into())]
        );
        assert_eq!(alias.label.to_string(), "@@dep+//:files_alias");
        assert_eq!(alias.label.output_label(), "@dep//:files_alias");
        assert!(
            alias
                .attributes
                .iter()
                .all(|attribute| attribute.name != "visibility")
        );

        let setting = graph.nodes.get(&label("is_k8")).unwrap();
        assert_eq!(
            setting.kind,
            QueryNodeKind::Rule("config_setting rule".into())
        );
        assert_eq!(
            setting
                .rule_capability
                .as_ref()
                .map(|capability| capability.rule_class.as_str()),
            Some("config_setting")
        );
        assert!(setting.edges.is_empty());
        assert!(setting.attributes.is_empty());
        assert_eq!(setting.effective_visibility, RuleVisibility::Private);
        assert_eq!(setting.visibility_source, VisibilitySource::PackageDefault);
        assert_eq!(setting.label.to_string(), "@@dep+//:is_k8");
        assert_eq!(setting.label.output_label(), "@dep//:is_k8");

        let public_setting = graph.nodes.get(&label("public_k8")).unwrap();
        assert_eq!(public_setting.effective_visibility, RuleVisibility::Public);
        assert_eq!(
            public_setting.visibility_source,
            VisibilitySource::AlwaysPublic
        );

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

    #[test]
    fn external_test_suite_projection_retains_membership_metadata_and_cycles() {
        let canonical_repo = CanonicalRepoName::new("dep+").unwrap();
        let apparent_repo = ApparentRepoName::new("dep").unwrap();
        let package = PackagePath::parse("").unwrap();
        let source = |target| CanonicalLabel::parse(&format!("@@//:{target}")).unwrap();
        let suite = |name: &str, membership, tags: &[&str], visibility| PackageTarget {
            name: name.to_owned(),
            kind: PackageTargetKind::TestSuite {
                membership,
                tags: tags
                    .iter()
                    .map(|tag| (*tag).into())
                    .collect::<Vec<_>>()
                    .into(),
            },
            visibility,
        };
        let targets = vec![
            suite(
                "omitted",
                TestSuiteMembership::Implicit {
                    members: Arc::from([]),
                    tests_explicit: false,
                },
                &[],
                VisibilitySource::PackageDefault,
            ),
            suite(
                "empty",
                TestSuiteMembership::Implicit {
                    members: Arc::from([]),
                    tests_explicit: true,
                },
                &["a", "manual"],
                VisibilitySource::AlwaysPublic,
            ),
            suite(
                "parent",
                TestSuiteMembership::Explicit {
                    tests: Arc::from([source("empty")]),
                },
                &[],
                VisibilitySource::PackageDefault,
            ),
            suite(
                "cycle_a",
                TestSuiteMembership::Explicit {
                    tests: Arc::from([source("cycle_b")]),
                },
                &[],
                VisibilitySource::PackageDefault,
            ),
            suite(
                "cycle_b",
                TestSuiteMembership::Explicit {
                    tests: Arc::from([source("cycle_a")]),
                },
                &[],
                VisibilitySource::PackageDefault,
            ),
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
        let omitted = graph.nodes.get(&label("omitted")).unwrap();
        let empty = graph.nodes.get(&label("empty")).unwrap();
        assert_eq!(
            graph.nodes.len(),
            6,
            "only BUILD plus declared suites exist"
        );
        assert_eq!(omitted.kind, QueryNodeKind::Rule("test_suite rule".into()));
        assert_eq!(omitted.label.to_string(), "@@dep+//:omitted");
        assert_eq!(omitted.label.output_label(), "@dep//:omitted");
        assert_eq!(
            omitted.rule_capability.as_ref().map(|capability| (
                &capability.rule_class,
                capability.executable,
                capability.test_kind
            )),
            Some((&"test_suite".into(), false, Some(TestRuleKind::Suite)))
        );
        assert_eq!(omitted.attributes.len(), 2);
        assert_eq!(omitted.attributes[0].name, "tests");
        assert!(!omitted.attributes[0].explicit);
        assert!(omitted.attributes[0].labels.is_empty());
        assert_eq!(omitted.attributes[1].name, "$implicit_tests");
        assert!(omitted.attributes[1].explicit);
        assert!(omitted.attributes[1].labels.is_empty());
        assert!(omitted.edges.is_empty());
        assert!(
            omitted
                .attributes
                .iter()
                .all(|attribute| attribute.name != "visibility")
        );

        assert!(empty.attributes[0].explicit);
        assert!(empty.attributes[1].explicit);
        assert_eq!(empty.effective_visibility, RuleVisibility::Public);
        assert_eq!(
            empty.test_metadata.as_ref().unwrap().tags.as_ref(),
            ["a", "manual"]
        );
        assert!(empty.test_metadata.as_ref().unwrap().manual);
        assert!(empty.test_metadata.as_ref().unwrap().size.is_none());

        for (suite_name, member) in [
            ("parent", "empty"),
            ("cycle_a", "cycle_b"),
            ("cycle_b", "cycle_a"),
        ] {
            let node = graph.nodes.get(&label(suite_name)).unwrap();
            assert_eq!(
                node.attributes[0]
                    .labels
                    .iter()
                    .map(|label| label.output_label().to_string())
                    .collect::<Vec<_>>(),
                [format!("@dep//:{member}")]
            );
            assert_eq!(
                node.edges
                    .iter()
                    .map(|edge| (edge.kind, edge.target.output_label().to_string()))
                    .collect::<Vec<_>>(),
                [(QueryEdgeKind::Ordinary, format!("@dep//:{member}"))]
            );
        }
    }

    #[test]
    fn external_test_suite_projection_rejects_unsupported_members_before_source_synthesis() {
        let canonical_repo = CanonicalRepoName::new("dep+").unwrap();
        let apparent_repo = ApparentRepoName::new("dep").unwrap();
        let package = PackagePath::parse("").unwrap();
        let source = |label| CanonicalLabel::parse(label).unwrap();
        let project = |member: CanonicalLabel, other: PackageTargetKind| {
            external_package_graph_from_targets(
                &canonical_repo,
                &apparent_repo,
                &package,
                Path::new("/external/dep+/BUILD.bazel"),
                &RuleVisibility::Private,
                &[
                    PackageTarget {
                        name: "suite".to_owned(),
                        kind: PackageTargetKind::TestSuite {
                            membership: TestSuiteMembership::Explicit {
                                tests: Arc::from([member]),
                            },
                            tags: Arc::from([]),
                        },
                        visibility: VisibilitySource::PackageDefault,
                    },
                    PackageTarget {
                        name: "member".to_owned(),
                        kind: other,
                        visibility: VisibilitySource::PackageDefault,
                    },
                ],
            )
        };
        let non_suite =
            project(source("@@//:member"), PackageTargetKind::ExportedFile).unwrap_err();
        assert!(
            non_suite
                .to_string()
                .contains("test_suite non-suite member is deferred")
        );
        let unresolved =
            project(source("@@//:missing"), PackageTargetKind::ExportedFile).unwrap_err();
        assert!(
            unresolved
                .to_string()
                .contains("test_suite unresolved member is deferred")
        );
        let cross_package =
            project(source("@@//other:member"), PackageTargetKind::ExportedFile).unwrap_err();
        assert!(
            cross_package
                .to_string()
                .contains("test_suite cross-package member is deferred")
        );
        let named_repository =
            project(source("@@other+//:member"), PackageTargetKind::ExportedFile).unwrap_err();
        assert!(
            named_repository
                .to_string()
                .contains("test_suite named-repository member is deferred")
        );
        let nonempty_implicit = external_package_graph_from_targets(
            &canonical_repo,
            &apparent_repo,
            &package,
            Path::new("/external/dep+/BUILD.bazel"),
            &RuleVisibility::Private,
            &[PackageTarget {
                name: "implicit".to_owned(),
                kind: PackageTargetKind::TestSuite {
                    membership: TestSuiteMembership::Implicit {
                        members: Arc::from([source("@@//:member")]),
                        tests_explicit: false,
                    },
                    tags: Arc::from([]),
                },
                visibility: VisibilitySource::PackageDefault,
            }],
        )
        .unwrap_err();
        assert!(
            nonempty_implicit
                .to_string()
                .contains("test_suite implicit tests are deferred")
        );
    }

    #[test]
    fn external_package_group_projection_retains_opaque_contents_and_include_cycles() {
        let canonical_repo = CanonicalRepoName::new("dep+").unwrap();
        let apparent_repo = ApparentRepoName::new("dep").unwrap();
        let package = PackagePath::parse("").unwrap();
        let source = |target| CanonicalLabel::parse(&format!("@@//:{target}")).unwrap();
        let contents = Arc::new(slug_loading_v2::PackageGroupContents::default());
        let group = |name: &str, includes: Vec<CanonicalLabel>| PackageTarget {
            name: name.to_owned(),
            kind: PackageTargetKind::PackageGroup {
                contents: contents.clone(),
                includes: includes.into(),
            },
            // Package groups are always public in loaded Bazel metadata. Use
            // package default here to prove the external projection preserves
            // their native public-node convention rather than default rules.
            visibility: VisibilitySource::PackageDefault,
        };
        let graph = external_package_graph_from_targets(
            &canonical_repo,
            &apparent_repo,
            &package,
            Path::new("/external/dep+/BUILD.bazel"),
            &RuleVisibility::Private,
            &[
                group("empty", vec![]),
                group("leaf", vec![]),
                group("parent", vec![source("leaf"), source("leaf")]),
                group("cycle_a", vec![source("cycle_b")]),
                group("cycle_b", vec![source("cycle_a")]),
            ],
        )
        .unwrap();
        let label = |target| {
            QueryLabel::in_external_package(&canonical_repo, &apparent_repo, &package, target)
                .unwrap()
        };
        let empty = graph.nodes.get(&label("empty")).unwrap();
        assert_eq!(
            graph.nodes.len(),
            6,
            "only BUILD plus declared groups exist"
        );
        assert_eq!(empty.kind, QueryNodeKind::PackageGroup);
        assert_eq!(empty.label.to_string(), "@@dep+//:empty");
        assert_eq!(empty.label.output_label(), "@dep//:empty");
        assert!(empty.rule_capability.is_none());
        assert!(empty.test_metadata.is_none());
        assert!(empty.attributes.is_empty());
        assert!(empty.edges.is_empty());
        assert_eq!(empty.effective_visibility, RuleVisibility::Public);
        assert_eq!(empty.visibility_source, VisibilitySource::AlwaysPublic);
        assert_eq!(
            empty.package_group_contents.as_ref().unwrap().as_ref(),
            contents.as_ref()
        );

        let parent = graph.nodes.get(&label("parent")).unwrap();
        assert_eq!(
            parent.package_group_contents.as_ref().unwrap().as_ref(),
            contents.as_ref()
        );
        assert_eq!(
            parent
                .edges
                .iter()
                .map(|edge| (edge.kind, edge.target.output_label().to_string()))
                .collect::<Vec<_>>(),
            [
                (QueryEdgeKind::PackageGroupInclude, "@dep//:leaf".to_owned()),
                (QueryEdgeKind::PackageGroupInclude, "@dep//:leaf".to_owned()),
            ]
        );
        for (group_name, include) in [("cycle_a", "cycle_b"), ("cycle_b", "cycle_a")] {
            let node = graph.nodes.get(&label(group_name)).unwrap();
            assert_eq!(
                node.edges
                    .iter()
                    .map(|edge| (edge.kind, edge.target.output_label().to_string()))
                    .collect::<Vec<_>>(),
                [(
                    QueryEdgeKind::PackageGroupInclude,
                    format!("@dep//:{include}"),
                )]
            );
        }
    }

    #[test]
    fn external_package_group_projection_rejects_unsupported_includes_before_source_synthesis() {
        let canonical_repo = CanonicalRepoName::new("dep+").unwrap();
        let apparent_repo = ApparentRepoName::new("dep").unwrap();
        let package = PackagePath::parse("").unwrap();
        let source = |label| CanonicalLabel::parse(label).unwrap();
        let contents = Arc::new(slug_loading_v2::PackageGroupContents::default());
        let project = |include: CanonicalLabel, other: PackageTargetKind| {
            external_package_graph_from_targets(
                &canonical_repo,
                &apparent_repo,
                &package,
                Path::new("/external/dep+/BUILD.bazel"),
                &RuleVisibility::Private,
                &[
                    PackageTarget {
                        name: "group".to_owned(),
                        kind: PackageTargetKind::PackageGroup {
                            contents: contents.clone(),
                            includes: Arc::from([include]),
                        },
                        visibility: VisibilitySource::AlwaysPublic,
                    },
                    PackageTarget {
                        name: "member".to_owned(),
                        kind: other,
                        visibility: VisibilitySource::PackageDefault,
                    },
                ],
            )
        };
        let missing = project(source("@@//:missing"), PackageTargetKind::ExportedFile).unwrap_err();
        assert!(
            missing
                .to_string()
                .contains("package_group missing include is deferred")
        );
        let non_group =
            project(source("@@//:member"), PackageTargetKind::ExportedFile).unwrap_err();
        assert!(
            non_group
                .to_string()
                .contains("package_group non-package-group include is deferred")
        );
        let alias = project(
            source("@@//:member"),
            PackageTargetKind::Alias {
                actual: source("@@//:source.txt"),
            },
        )
        .unwrap_err();
        assert!(
            alias
                .to_string()
                .contains("package_group alias include is deferred")
        );
        let cross_package =
            project(source("@@//other:member"), PackageTargetKind::ExportedFile).unwrap_err();
        assert!(
            cross_package
                .to_string()
                .contains("package_group cross-package include is deferred")
        );
        let named_repository =
            project(source("@@other+//:member"), PackageTargetKind::ExportedFile).unwrap_err();
        assert!(
            named_repository
                .to_string()
                .contains("package_group named-repository include is deferred")
        );
        let unsupported = external_package_graph_from_targets(
            &canonical_repo,
            &apparent_repo,
            &package,
            Path::new("/external/dep+/BUILD.bazel"),
            &RuleVisibility::Private,
            &[PackageTarget {
                name: "generated.txt".to_owned(),
                kind: PackageTargetKind::GeneratedFile {
                    label: source("@@//:generated.txt"),
                    generating_rule: "producer".into(),
                },
                visibility: VisibilitySource::PackageDefault,
            }],
        )
        .unwrap_err();
        assert!(
            unsupported
                .to_string()
                .contains("external repository rule graph is deferred")
        );
    }

    #[tokio::test]
    async fn external_package_group_projection_retains_loaded_nonempty_contents_opaquely() {
        static NEXT_WORKSPACE: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let serial = NEXT_WORKSPACE.fetch_add(1, Ordering::Relaxed);
        let workspace =
            std::env::temp_dir().join(format!("slug-external-package-group-{nanos}-{serial}",));
        fs::create_dir_all(&workspace).unwrap();
        let module = workspace.join("MODULE.bazel");
        let build = workspace.join("BUILD.bazel");
        let module_source = "module(name = \"dep\")\n";
        let build_source = "package_group(name = \"pg_empty\")\npackage_group(name = \"pg_nonempty\", packages = [\"//pkg\", \"//tree/...\", \"-//blocked\", \"-//blocked_tree/...\", \"public\", \"private\"])\n";
        fs::write(&module, module_source).unwrap();
        fs::write(&build, build_source).unwrap();

        let files = WorkspaceSnapshot {
            files: Arc::new(
                [
                    (
                        module.clone(),
                        WorkspaceFileValue::Present(Arc::new(module_source.to_owned())),
                    ),
                    (
                        build.clone(),
                        WorkspaceFileValue::Present(Arc::new(build_source.to_owned())),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
        };
        let raw_files = Arc::new(WorkspaceRawSnapshot {
            files: Arc::new(
                [
                    (
                        module.clone(),
                        WorkspaceRawFileValue::Present(Arc::from(module_source.as_bytes())),
                    ),
                    (
                        build.clone(),
                        WorkspaceRawFileValue::Present(Arc::from(build_source.as_bytes())),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
        });
        let directories = WorkspaceDirectorySnapshot {
            directories: Arc::new(
                [(
                    workspace.clone(),
                    WorkspaceDirectoryValue::present(vec![
                        WorkspaceDirectoryEntry {
                            name: "MODULE.bazel".into(),
                            kind: WorkspaceDirectoryEntryKind::RegularFile,
                        },
                        WorkspaceDirectoryEntry {
                            name: "BUILD.bazel".into(),
                            kind: WorkspaceDirectoryEntryKind::RegularFile,
                        },
                    ]),
                )]
                .into_iter()
                .collect(),
            ),
        };
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut updater = dice.updater();
        updater
            .changed_to(vec![(
                WorkspaceSnapshotKey {
                    workspace: workspace.clone(),
                },
                Arc::new(files),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                WorkspaceRawSnapshotKey {
                    workspace: workspace.clone(),
                },
                raw_files,
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                WorkspaceDirectorySnapshotKey {
                    workspace: workspace.clone(),
                },
                Arc::new(directories),
            )])
            .unwrap();
        inject_root_module_request_inputs(
            &mut updater,
            &workspace,
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
        )
        .unwrap();
        let mut transaction = updater.commit().await;
        let loaded = BzlModuleEvaluator::new(&workspace)
            .unwrap()
            .evaluate_package(&mut transaction, &workspace)
            .await
            .unwrap();
        let canonical_repo = CanonicalRepoName::new("dep+").unwrap();
        let apparent_repo = ApparentRepoName::new("dep").unwrap();
        let package = PackagePath::parse("").unwrap();
        let graph = external_package_graph_from_targets(
            &canonical_repo,
            &apparent_repo,
            &package,
            &loaded.build_file,
            &loaded.default_visibility,
            &loaded.targets,
        )
        .unwrap();
        let label = |target| {
            QueryLabel::in_external_package(&canonical_repo, &apparent_repo, &package, target)
                .unwrap()
        };
        let empty = graph
            .nodes
            .get(&label("pg_empty"))
            .unwrap()
            .package_group_contents
            .as_ref()
            .unwrap();
        assert!(empty.exact_positive().is_empty());
        assert!(empty.subtree_positive().is_empty());
        assert!(!empty.positive_all());
        assert!(empty.exact_negative().is_empty());
        assert!(empty.subtree_negative().is_empty());
        assert!(!empty.has_private());

        // Inspect only the retained representation; do not call
        // `contains_package` or otherwise evaluate package-group contents in
        // an external repository identity context.
        let nonempty = graph
            .nodes
            .get(&label("pg_nonempty"))
            .unwrap()
            .package_group_contents
            .as_ref()
            .unwrap();
        assert_eq!(nonempty.exact_positive().len(), 1);
        assert_eq!(nonempty.subtree_positive().len(), 1);
        assert!(nonempty.positive_all());
        assert_eq!(nonempty.exact_negative().len(), 1);
        assert_eq!(nonempty.subtree_negative().len(), 1);
        assert!(nonempty.has_private());
        fs::remove_dir_all(&workspace).unwrap();
    }

    #[test]
    fn external_alias_projection_rejects_chains_and_build_destinations() {
        let canonical_repo = CanonicalRepoName::new("dep+").unwrap();
        let apparent_repo = ApparentRepoName::new("dep").unwrap();
        let package = PackagePath::parse("").unwrap();
        let source = |target| CanonicalLabel::parse(&format!("@@//:{target}")).unwrap();
        let project = |targets: Vec<PackageTarget>| {
            external_package_graph_from_targets(
                &canonical_repo,
                &apparent_repo,
                &package,
                Path::new("/external/dep+/BUILD.bazel"),
                &RuleVisibility::Private,
                &targets,
            )
        };

        let chain = project(vec![
            PackageTarget {
                name: "first".to_owned(),
                kind: PackageTargetKind::Alias {
                    actual: source("second"),
                },
                visibility: VisibilitySource::PackageDefault,
            },
            PackageTarget {
                name: "second".to_owned(),
                kind: PackageTargetKind::Alias {
                    actual: source("source.txt"),
                },
                visibility: VisibilitySource::PackageDefault,
            },
        ])
        .unwrap_err();
        assert!(
            chain
                .to_string()
                .contains("external repository alias chains are deferred"),
            "{chain}"
        );

        let build_destination = project(vec![PackageTarget {
            name: "to_build".to_owned(),
            kind: PackageTargetKind::Alias {
                actual: source("BUILD.bazel"),
            },
            visibility: VisibilitySource::PackageDefault,
        }])
        .unwrap_err();
        assert!(
            build_destination
                .to_string()
                .contains("external repository alias actual destination is deferred"),
            "{build_destination}"
        );
    }
}
