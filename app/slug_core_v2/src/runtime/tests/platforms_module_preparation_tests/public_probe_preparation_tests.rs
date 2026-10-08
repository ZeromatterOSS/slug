use super::*;

mod source_attempt_context_tests {
    include!("public_probe_preparation_tests/source_attempt_context_tests.rs");
}

#[derive(Clone, Copy)]
pub(super) enum CommandChoice {
    Modules,
    PublicProbe,
}
pub(super) enum ObservationOutcome {
    Modules(Result<DrivenCommand<ModuleTerminal>, NativeDemandSessionError>),
    Probe(ProbeOutcome),
}
pub(super) enum ProbeOutcome {
    PrepareRefusal(BuildCommandError),
    DispatchRefusal,
    Driven(
        Result<DrivenCommand<SingletonRootSingleBuildCommandTerminal>, NativeDemandSessionError>,
    ),
}
impl ObservationOutcome {
    pub(super) fn is_ok(&self) -> bool {
        matches!(
            self,
            Self::Modules(Ok(_)) | Self::Probe(ProbeOutcome::Driven(Ok(_)))
        )
    }
    pub(super) fn stage(&self) -> &'static str {
        match self {
            Self::Modules(Ok(_)) => "mapping_complete",
            Self::Modules(Err(_)) | Self::Probe(ProbeOutcome::Driven(Err(_))) => "native_refusal",
            Self::Probe(ProbeOutcome::PrepareRefusal(_) | ProbeOutcome::DispatchRefusal) => {
                "prepare_refusal"
            }
            Self::Probe(ProbeOutcome::Driven(Ok(driven))) => {
                if driven.accepted.terminal().result.is_ok() {
                    "build_complete"
                } else {
                    "build_refusal"
                }
            }
        }
    }
    pub(super) fn report_native(&self) {
        let result = match self {
            Self::Modules(Ok(driven)) => {
                assert!(root_bytes(&driven.accepted.terminal().source));
                (
                    "complete",
                    Some((driven.attempts, driven.terminal_root_count)),
                )
            }
            Self::Probe(ProbeOutcome::Driven(Ok(driven))) => (
                "complete",
                Some((driven.attempts, driven.terminal_root_count)),
            ),
            Self::Modules(Err(error)) | Self::Probe(ProbeOutcome::Driven(Err(error))) => {
                (native_kind(error), None)
            }
            _ => ("unavailable", None),
        };
        if let Some((attempts, roots)) = result.1 {
            println!(
                "WP942_NATIVE kind={} attempts={attempts} roots={roots}",
                result.0
            );
        } else {
            println!(
                "WP942_NATIVE kind={} attempts=unavailable roots=unavailable",
                result.0
            );
        }
    }
}
pub(super) struct ProbeReport {
    preparation: &'static str,
    dispatch: &'static str,
    policy_checked: bool,
}
struct Scalar(Option<usize>);
impl std::fmt::Display for Scalar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(value) => write!(f, "{value}"),
            None => f.write_str("unavailable"),
        }
    }
}
impl Default for ProbeReport {
    fn default() -> Self {
        Self {
            preparation: "not_started",
            dispatch: "not_started",
            policy_checked: false,
        }
    }
}
impl ProbeReport {
    pub(super) fn policy_checked(&self) -> bool {
        self.policy_checked
    }
    pub(super) fn report(
        &self,
        stage: &str,
        early_kind: &str,
        outcome: Option<&ObservationOutcome>,
    ) {
        let mut certificate = "unavailable";
        let mut package_cause = None;
        let mut observations = None;
        let mut counts = None;
        let kind = match outcome {
            Some(ObservationOutcome::Probe(ProbeOutcome::PrepareRefusal(error))) => {
                if let BuildCommandErrorKind::Package(cause) = &error.kind {
                    package_cause = Some(cause);
                }
                build_kind(error)
            }
            Some(ObservationOutcome::Probe(ProbeOutcome::DispatchRefusal)) => "dispatch",
            Some(ObservationOutcome::Probe(ProbeOutcome::Driven(Err(error)))) => native_kind(error),
            Some(ObservationOutcome::Probe(ProbeOutcome::Driven(Ok(driven)))) => {
                let terminal = driven.accepted.terminal();
                certificate = if terminal.source_certificate().is_some() {
                    "true"
                } else {
                    "false"
                };
                observations = terminal
                    .observations
                    .as_ref()
                    .map(|epoch| epoch.observations().len());
                match terminal.result.as_ref() {
                    Err(error) => {
                        if let BuildCommandErrorKind::Package(cause) = &error.kind {
                            package_cause = Some(cause);
                        }
                        build_kind(error)
                    }
                    Ok(evaluation) => {
                        let mut analyses = 0;
                        let mut actions = 0;
                        let mut rustc = 0;
                        for analysis in evaluation.analyses() {
                            analyses += 1;
                            actions += analysis.actions().len();
                            rustc += analysis
                                .actions()
                                .iter()
                                .filter(|action| {
                                    action
                                        .spawn_spec()
                                        .is_some_and(|spawn| spawn.mnemonic() == "Rustc")
                                })
                                .count();
                        }
                        counts = Some((analyses, actions, rustc));
                        "complete"
                    }
                }
            }
            _ => early_kind,
        };
        println!(
            "WP958_RESULT stage={stage} kind={kind} preparation={} dispatch={} policy_checked={} certificate={certificate} observations={} analyses={} actions={} rustc={}",
            self.preparation,
            self.dispatch,
            self.policy_checked,
            Scalar(observations),
            Scalar(counts.map(|n| n.0)),
            Scalar(counts.map(|n| n.1)),
            Scalar(counts.map(|n| n.2))
        );
        if let Some(cause) = package_cause {
            println!(
                "WP966_PACKAGE stage={} detail={}",
                cause.diagnostic_stage(),
                cause.diagnostic()
            );
        } else {
            println!("WP966_PACKAGE stage=none detail=unavailable");
        }
    }
}

#[derive(Clone)]
struct GuardedSingleton {
    inner: SingletonRootSingleBuildCommandKey,
    bodies: Arc<Bodies>,
    milestones: Arc<Mutex<Milestones>>,
}
const MATERIALIZATION_NAMESPACE_ENTRIES: usize = 32;
struct NamespaceDiagnostic<'a> {
    instance: slug_workspace_v2::PathObservationInstanceId,
    operation: &'static str,
    association: &'static str,
    owner: Option<MaterializationDiagnostic<'a>>,
    policy: NamespacePolicy,
}
impl fmt::Display for NamespaceDiagnostic<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "WP987_NAMESPACE association={} namespace=materialization instance={} operation={}",
            self.association,
            self.instance.value(),
            self.operation
        )?;
        if let Some(owner) = &self.owner {
            writeln!(formatter)?;
            owner.write_marker(formatter, "WP987_NAMESPACE_OWNER")?;
        }
        write!(
            formatter,
            "\nWP988_NAMESPACE_POLICY owner_plan_allowed={} path_class={}",
            self.policy.owner_plan_allowed, self.policy.path_class
        )
    }
}
#[derive(Clone, Copy)]
struct NamespacePolicy {
    owner_plan_allowed: bool,
    path_class: &'static str,
}
impl NamespacePolicy {
    fn admits(self, operation: &PathObservationOperation) -> bool {
        self.owner_plan_allowed
            && match self.path_class {
                "ancestor" | "root" => operation == &PathObservationOperation::Lstat,
                "fixed_file" => matches!(
                    operation,
                    PathObservationOperation::Lstat | PathObservationOperation::FileBytes
                ),
                _ => false,
            }
    }
}
fn namespace_policy(
    bodies: &Bodies,
    demand: &PathObservationDemand,
    epoch: Option<&slug_bzlmod_v2::RepositoryMaterializationResultEpoch>,
    plan_matches: &impl Fn(
        &slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan,
        &[GeneratedFileSignature],
    ) -> bool,
) -> NamespacePolicy {
    let unavailable = NamespacePolicy {
        owner_plan_allowed: false,
        path_class: "unavailable",
    };
    let PathObservationNamespace::Materialization(instance) = demand.namespace() else {
        return unavailable;
    };
    let Some(view) = epoch.and_then(|epoch| {
        epoch
            .borrowed_instance_for_namespace(
                &bodies.roots[0],
                instance,
                MATERIALIZATION_NAMESPACE_ENTRIES,
            )
            .ok()
            .flatten()
    }) else {
        return unavailable;
    };
    let request = view.request();
    let Some(files) = bodies.generated_features_owner_signatures(request) else {
        return NamespacePolicy {
            owner_plan_allowed: false,
            path_class: "other",
        };
    };
    let owner_plan_allowed = matches!(
        &request.kind,
        RepositoryMaterializationKind::GeneratedFileEffects(plan) if plan_matches(plan, files)
    );
    let path = demand.path().as_path();
    let root = view.generation_root();
    let path_class = if path == root {
        "root"
    } else if root.starts_with(path) {
        "ancestor"
    } else if let Ok(relative) = path.strip_prefix(root) {
        if files.iter().any(|file| relative == Path::new(file.0)) {
            "fixed_file"
        } else if files
            .iter()
            .any(|file| Path::new(file.0).starts_with(relative))
        {
            "ancestor"
        } else {
            "other"
        }
    } else {
        "other"
    };
    NamespacePolicy {
        owner_plan_allowed,
        path_class,
    }
}
fn admit_with_materialization_epoch(
    bodies: &Bodies,
    needs: &SourcePreparationNeeds,
    epoch: Option<&slug_bzlmod_v2::RepositoryMaterializationResultEpoch>,
) -> Result<(), Denied> {
    bodies.admit_with_matchers(needs, &generated_features_plan_matches, &|demand| {
        namespace_policy(bodies, demand, epoch, &generated_features_plan_matches)
            .admits(&demand.operation())
    })
}
fn namespace_diagnostic<'a>(
    bodies: &Bodies,
    needs: &SourcePreparationNeeds,
    denied: &Denied,
    epoch: Option<&'a slug_bzlmod_v2::RepositoryMaterializationResultEpoch>,
) -> Option<NamespaceDiagnostic<'a>> {
    use slug_bzlmod_v2::RepositoryMaterializationInstanceLookupError as LookupError;
    if denied.family != "namespace" {
        return None;
    }
    let (demand, family) = bodies.first_denied_path_with_namespace_matcher(needs, &|demand| {
        namespace_policy(bodies, demand, epoch, &generated_features_plan_matches)
            .admits(&demand.operation())
    })?;
    if family != denied.family
        || denied.path.as_ref() != Some(demand.path())
        || denied.operation != path_operation_code(&demand.operation())
    {
        return None;
    }
    let PathObservationNamespace::Materialization(instance) = demand.namespace() else {
        return None;
    };
    let (association, owner) = match epoch.map(|epoch| {
        epoch.borrowed_instance_for_namespace(
            &bodies.roots[0],
            instance,
            MATERIALIZATION_NAMESPACE_ENTRIES,
        )
    }) {
        None => ("unavailable", None),
        Some(Ok(None)) => ("missing", None),
        Some(Ok(Some(view))) => (
            "unique",
            Some(MaterializationDiagnostic::new(
                view.request(),
                "observation_instance",
            )),
        ),
        Some(Err(LookupError::WrongWorkspace)) => ("wrong_workspace", None),
        Some(Err(LookupError::InvalidInstance)) => ("invalid_instance", None),
        Some(Err(LookupError::TooManyEntries)) => ("bounds_refused", None),
        Some(Err(LookupError::AmbiguousInstance)) => ("ambiguous", None),
        Some(Err(LookupError::InvalidGenerationRoot)) => ("invalid_generation_root", None),
    };
    Some(NamespaceDiagnostic {
        instance,
        operation: denied.operation,
        association,
        owner,
        policy: namespace_policy(bodies, demand, epoch, &generated_features_plan_matches),
    })
}
async fn current_materialization_epoch(
    tx: &mut dice::DiceTransaction,
    workspace: &NormalizedAbsolutePath,
) -> Option<slug_bzlmod_v2::RepositoryMaterializationResultEpoch> {
    tx.compute(&slug_bzlmod_v2::RepositoryMaterializationResultEpochKey {
        workspace: workspace.dupe(),
    })
    .await
    .ok()
}
impl GuardedSingleton {
    fn check_need(&self, needs: &SourcePreparationNeeds) -> Result<(), NativeDemandSessionError> {
        self.bodies
            .admit(needs)
            .map_err(|denied| self.record_denied(needs, denied))
    }
    fn record_denied(
        &self,
        needs: &SourcePreparationNeeds,
        denied: Denied,
    ) -> NativeDemandSessionError {
        if let Some(diagnostic) = self.bodies.materialization_diagnostic(needs, &denied) {
            println!("{diagnostic}");
            if let Some(plan) = self.bodies.generated_plan_diagnostic(needs, &denied) {
                println!("{plan}");
            }
        }
        self.milestones.lock().unwrap().denied = Some(denied);
        refusal()
    }
}
#[async_trait]
impl NativeCommandRoot for GuardedSingleton {
    type Terminal = SingletonRootSingleBuildCommandTerminal;
    fn complete<'a>(
        &'a self,
        terminal: &'a mut Self::Terminal,
        context: NativeCommandCompletionContext<'a, '_>,
    ) -> impl std::future::Future<Output = Result<(), NativeDemandSessionError>> + 'a {
        self.inner.complete(terminal, context)
    }
    fn publish(&self, terminal: &mut Self::Terminal) -> std::io::Result<()> {
        self.inner.publish(terminal)
    }
    fn publication_completed(&self, terminal: &Self::Terminal) -> bool {
        self.inner.publication_completed(terminal)
    }
    fn initializes_request_revision(&self) -> bool {
        self.inner.initializes_request_revision()
    }
    fn requires_workspace_status_inputs(&self) -> bool {
        self.inner.requires_workspace_status_inputs()
    }
    fn source_certificate<'a>(
        &self,
        terminal: &'a Self::Terminal,
    ) -> Option<&'a SourceCertificate> {
        self.inner.source_certificate(terminal)
    }
    fn observations<'a>(&self, terminal: &'a Self::Terminal) -> Option<&'a PathObservationEpoch> {
        self.inner.observations(terminal)
    }
    fn observed_selection_association(&self) -> ObservedSelectionAssociation {
        self.inner.observed_selection_association()
    }
    fn terminal_observed_selection_association(
        &self,
        terminal: &Self::Terminal,
    ) -> ObservedSelectionAssociation {
        self.inner.terminal_observed_selection_association(terminal)
    }
    fn terminal_demand_association(&self, terminal: &Self::Terminal) -> TerminalDemandAssociation {
        self.inner.terminal_demand_association(terminal)
    }
    fn event_reconciliation_policy(&self, terminal: &Self::Terminal) -> EventReconciliationPolicy {
        self.inner.event_reconciliation_policy(terminal)
    }
    fn allows_empty_terminal(&self) -> bool {
        self.inner.allows_empty_terminal()
    }
    fn allows_unavailable_terminal_roots(&self, terminal: &Self::Terminal) -> bool {
        self.inner.allows_unavailable_terminal_roots(terminal)
    }
    async fn compute(
        &self,
        tx: &mut dice::DiceTransaction,
    ) -> Result<SourcePreparationOutcome<Self::Terminal>, NativeDemandSessionError> {
        let outcome = NativeCommandRoot::compute(&self.inner, tx).await?;
        if let SourcePreparationOutcome::Need(needs) = &outcome {
            if let Err(denied) = self.bodies.admit(needs) {
                if denied.family != "namespace" {
                    return Err(self.record_denied(needs, denied));
                }
                // Read this attempt's injected epoch before taking the reporting lock.
                let epoch = current_materialization_epoch(tx, &self.bodies.roots[0]).await;
                if let Err(denied) =
                    admit_with_materialization_epoch(&self.bodies, needs, epoch.as_ref())
                {
                    if let Some(diagnostic) =
                        namespace_diagnostic(&self.bodies, needs, &denied, epoch.as_ref())
                    {
                        println!("{diagnostic}");
                    }
                    return Err(self.record_denied(needs, denied));
                }
            }
        }
        Ok(outcome)
    }
}
fn nightly_overlay() -> CommandConfigurationOverlay {
    CommandConfigurationOverlay::from(vec![
        slug_configuration_v2::CommandConfigurationOccurrence::starlark(
            "@rules_rust//rust/toolchain/channel",
            Some("nightly"),
            false,
        ),
    ])
}
fn projection_matches(
    inner: &SingletonRootSingleBuildCommandKey,
    request: &NativeDemandRequestInputBundle,
    bodies: &Bodies,
    registry: &RegistryUrls,
    overlay: &CommandConfigurationOverlay,
) -> bool {
    request.registry_urls == *registry
        && request.lockfile_mode == LockfileMode::Off
        && inner.0.workspace == bodies.roots[0]
        && matches!(inner.0.targets.as_ref(), [pattern] if pattern.as_ref() == "//pkg:probe")
        && bodies.visible_lockfile_metadata
        && bodies.repo_marker_metadata
        && bodies.ignore_file_bytes
        && bodies.rust_package_metadata
        && bodies.platforms_host_metadata
        && bodies.platforms_host_import_sources
        && bodies.nightly_package_metadata
        && bodies.nightly_build_source
        && bodies.nightly_channel_source
        && bodies.probe_package_source
        && bodies.rules_rust_defs_source
        && bodies.rules_rust_defs_import_sources
        && bodies.generated_features_file_effects
        && bodies.bazel_features_import_sources
        && bodies.additional[0].is_some()
        && bodies.bazel_skylib_import_sources
        && bodies.additional[1].is_some()
        && bodies.rules_cc_import_sources
        && bodies.additional[2].is_some()
        && inner.0.configuration_overlay == *overlay
        && request.command_policy.module_overrides().count() == bodies.bindings().count()
        && bodies.bindings().all(|(name, _, root)| {
            request
                .command_policy
                .module_overrides()
                .any(|(actual, path)| actual == name && path == root.as_path())
        })
}
pub(super) fn prepare_and_drive(
    runtime: &WorkspaceRuntime,
    bodies: Arc<Bodies>,
    policy: BzlmodCommandPolicyKey,
    registry: &RegistryUrls,
    milestones: Arc<Mutex<Milestones>>,
    report: &mut ProbeReport,
) -> ProbeOutcome {
    let target = [TargetPattern::parse("//pkg:probe").expect("fixed public singleton")];
    let environment = BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None)
        .expect("absent yanked versions");
    let urls = [registry.as_slice()[0].as_str().to_owned()];
    let overlay = nightly_overlay();
    let prepared = runtime.prepare_build_request(
        &target,
        policy,
        environment,
        LockfileMode::Off,
        &urls,
        slug_bzlmod_v2::RepositoryEnvironmentSnapshot::empty(),
        overlay.clone(),
    );
    let (key, request) = match prepared {
        Err(error) => {
            report.preparation = "refused";
            return ProbeOutcome::PrepareRefusal(error);
        }
        Ok(prepared) => {
            report.preparation = "complete";
            prepared
        }
    };
    if BuildCommandRootObservationKey::new(key.clone()).is_some() {
        report.dispatch = "refused";
        return ProbeOutcome::DispatchRefusal;
    }
    let Some(inner) = SingletonRootSingleBuildCommandKey::new(key) else {
        report.dispatch = "refused";
        return ProbeOutcome::DispatchRefusal;
    };
    report.dispatch = "singleton";
    report.policy_checked = projection_matches(&inner, &request, &bodies, registry, &overlay);
    if !report.policy_checked {
        report.dispatch = "refused";
        return ProbeOutcome::DispatchRefusal;
    }
    milestones.lock().unwrap().registry_policy_checked = true;
    ProbeOutcome::Driven(runtime.drive_command(
        request,
        GuardedSingleton {
            inner,
            bodies,
            milestones,
        },
    ))
}

#[test]
fn materialization_refusal_projection_preserves_kind_checks_and_precedence() {
    let path = |s: &str| NormalizedAbsolutePath::new(s).unwrap();
    let bodies = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")]);
    let valid = || (*local_native_request(&bodies.roots[0], "platforms", "/platforms")).clone();
    let allowed = SourcePreparationNeeds::repository(valid());
    assert!(bodies.admit(&allowed).is_ok());
    assert!(bodies.first_denied_materialization(&allowed).is_none());
    let discovery = SourcePreparationNeeds::repository(
        (*local_native_request(&bodies.roots[0], "platforms+", "/platforms")).clone(),
    );
    let discovery_original = discovery
        .repository_materializations()
        .values()
        .next()
        .unwrap();
    let discovery_weak = Arc::downgrade(discovery_original);
    let both = allowed.try_union(&discovery).unwrap();
    assert_eq!(both.repository_materializations().len(), 2);
    assert!(bodies.admit(&both).is_ok());
    for original in [
        allowed
            .repository_materializations()
            .values()
            .next()
            .unwrap(),
        discovery_original,
    ] {
        assert!(
            both.repository_materializations()
                .values()
                .any(|held| Arc::ptr_eq(held, original))
        );
    }
    for (canonical, root) in [
        ("platforms++", "/platforms"),
        ("unknown+", "/platforms"),
        ("rules_rust", "/rules"),
    ] {
        let request = (*local_native_request(&bodies.roots[0], canonical, root)).clone();
        let rejected = SourcePreparationNeeds::repository(request);
        let original = rejected
            .repository_materializations()
            .values()
            .next()
            .unwrap();
        let mixed = both.try_union(&rejected).unwrap();
        let (borrowed, reason) = bodies.first_denied_materialization(&mixed).unwrap();
        assert!(std::ptr::eq(borrowed, original.as_ref()));
        assert_eq!(reason, "binding");
        assert_eq!(usize::from(bodies.admit(&mixed).is_ok()), 0);
        assert_eq!(bodies.admit(&mixed).unwrap_err().family, "materialization");
    }
    drop(both);
    assert_eq!(Arc::strong_count(discovery_original), 1);
    drop(discovery);
    assert!(discovery_weak.upgrade().is_none());
    let reasons = [
        "workspace",
        "binding",
        "attributes_count",
        "path_attribute",
        "rule_name",
        "rule_mapping",
        "rule_repository",
        "rule_package",
        "rule_target",
    ];
    for canonical in ["platforms+", "platforms"] {
        for (index, reason) in reasons.into_iter().enumerate() {
            let mut request = valid();
            request.id.canonical_repo =
                slug_identity_v2::CanonicalRepoName::new(canonical).unwrap();
            match index {
                0 => request.id.workspace = path("/foreign"),
                1 => {
                    request.id.canonical_repo =
                        slug_identity_v2::CanonicalRepoName::new("rules_rust+").unwrap()
                }
                2 => {
                    Arc::make_mut(&mut request.repo_spec.attributes)
                        .insert("extra".into(), OverrideAttributeValue::Bool(true));
                }
                3 => {
                    Arc::make_mut(&mut request.repo_spec.attributes)
                        .insert("path".into(), OverrideAttributeValue::Bool(false));
                }
                4 => request.repo_spec.rule_id.rule_name = "other".into(),
                5 => {
                    let mut mapping = slug_identity_v2::RepositoryMapping::new(
                        slug_identity_v2::RepositoryMappingId::new("mapped").unwrap(),
                    );
                    mapping.insert(
                        slug_identity_v2::ApparentRepoName::new("bazel_tools").unwrap(),
                        slug_identity_v2::CanonicalRepoName::new("bazel_tools").unwrap(),
                    );
                    request.repo_spec.rule_id.bzl_file = slug_identity_v2::ApparentLabel::parse(
                        "@bazel_tools//tools/build_defs/repo:local.bzl",
                    )
                    .unwrap()
                    .resolve(&mapping);
                }
                6 => {
                    request.repo_spec.rule_id.bzl_file =
                        CanonicalLabel::parse("@@other//tools/build_defs/repo:local.bzl").unwrap()
                }
                7 => {
                    request.repo_spec.rule_id.bzl_file =
                        CanonicalLabel::parse("@@bazel_tools//other:local.bzl").unwrap()
                }
                8 => {
                    request.repo_spec.rule_id.bzl_file =
                        CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:other.bzl")
                            .unwrap()
                }
                _ => unreachable!(),
            }
            let rejected = SourcePreparationNeeds::repository(request);
            let original = rejected
                .repository_materializations()
                .values()
                .next()
                .unwrap();
            let (borrowed, actual) = bodies.first_denied_materialization(&rejected).unwrap();
            assert!(std::ptr::eq(borrowed, original.as_ref()));
            assert_eq!(actual, reason);
            assert_eq!(
                bodies.admit(&rejected).unwrap_err().family,
                "materialization"
            );
            assert_eq!(Arc::strong_count(original), 1);
        }
    }
    for attributes in [
        Arc::new(starlark_map::small_map::SmallMap::from_iter([(
            compact_str::CompactString::new("other"),
            OverrideAttributeValue::Bool(false),
        )])),
        Arc::new(starlark_map::small_map::SmallMap::from_iter([(
            compact_str::CompactString::new("path"),
            OverrideAttributeValue::String("/other".into()),
        )])),
    ] {
        let mut request = valid();
        request.repo_spec.attributes = attributes;
        assert_eq!(
            bodies
                .first_denied_materialization(&SourcePreparationNeeds::repository(request))
                .unwrap()
                .1,
            "path_attribute"
        );
    }
    // Canonical name and logical root matching different bindings must fail together.
    for canonical in ["platforms+", "platforms"] {
        let mut wrong_root = valid();
        wrong_root.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new(canonical).unwrap();
        wrong_root.kind = RepositoryMaterializationKind::Local {
            logical_root: path("/rules"),
        };
        assert_eq!(
            bodies
                .first_denied_materialization(&SourcePreparationNeeds::repository(wrong_root))
                .unwrap()
                .1,
            "binding"
        );
    }
    for kind in [
        RepositoryMaterializationKind::Immutable,
        RepositoryMaterializationKind::GeneratedFileEffects(
            slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan::build([]).unwrap(),
        ),
    ] {
        let mut request = valid();
        request.id.workspace = path("/foreign");
        request.kind = kind;
        let rejected = SourcePreparationNeeds::repository(request);
        let (borrowed, reason) = bodies.first_denied_materialization(&rejected).unwrap();
        assert_eq!(
            reason,
            if matches!(&borrowed.kind, RepositoryMaterializationKind::Immutable) {
                "immutable_kind"
            } else {
                "generated_kind"
            }
        );
        assert!(MaterializationDiagnostic::new(borrowed, reason).fields[6].is_none());
    }
    let mut first = (*local_native_request(&bodies.roots[0], "rules_rust+", "/rules")).clone();
    first.repo_spec.rule_id.rule_name = "first".into();
    let mut second = valid();
    second.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new("other+").unwrap();
    second.repo_spec.rule_id.rule_name = "second".into();
    let first = SourcePreparationNeeds::repository(first);
    let second = SourcePreparationNeeds::repository(second);
    let mixed = allowed
        .try_union(&first)
        .unwrap()
        .try_union(&second)
        .unwrap();
    let reversed = allowed
        .try_union(&second)
        .unwrap()
        .try_union(&first)
        .unwrap();
    let reversed_first = reversed
        .repository_materializations()
        .values()
        .find(|r| r.repo_spec.rule_id.rule_name != "local_repository")
        .unwrap();
    assert!(std::ptr::eq(
        bodies.first_denied_materialization(&reversed).unwrap().0,
        reversed_first.as_ref()
    ));
    let owner_first = mixed
        .repository_materializations()
        .values()
        .find(|r| r.repo_spec.rule_id.rule_name != "local_repository")
        .unwrap();
    assert!(std::ptr::eq(
        bodies.first_denied_materialization(&mixed).unwrap().0,
        owner_first.as_ref()
    ));
    let body =
        SourcePreparationNeeds::path(NeedPathObservations::singleton(PathObservationDemand::new(
            PathObservationNamespace::Host,
            path("/fixture/MODULE.bazel"),
            PathObservationOperation::FileBytes,
        )));
    let mut progress = 0;
    if bodies.admit(&mixed.try_union(&body).unwrap()).is_ok() {
        progress += 1;
    }
    assert_eq!(progress, 0);
    assert_eq!(
        bodies
            .admit(&mixed.try_union(&body).unwrap())
            .unwrap_err()
            .family,
        "materialization"
    );
    let bootstrap =
        SourcePreparationNeeds::root_module_bootstrap(slug_bzlmod_v2::RootModuleBootstrapRequest {
            workspace: path("/foreign"),
        });
    assert_eq!(
        bodies
            .admit(&mixed.try_union(&bootstrap).unwrap())
            .unwrap_err()
            .family,
        "bootstrap"
    );
    let environment = SourcePreparationNeeds::environment(
        slug_bzlmod_v2::NeedRepositoryEnvironmentNames::new(
            path("/fixture"),
            slug_bzlmod_v2::RepositoryEnvironmentNameFrontier::from_unsorted([
                compact_str::CompactString::new("PATH"),
            ]),
        )
        .unwrap(),
    );
    for (earlier, family) in [(bootstrap, "bootstrap"), (environment, "environment")] {
        let rejected = mixed.try_union(&earlier).unwrap();
        let denied = bodies.admit(&rejected).unwrap_err();
        assert_eq!(denied.family, family);
        assert!(
            bodies
                .materialization_diagnostic(&rejected, &denied)
                .is_none()
        );
    }
    let denied = bodies.admit(&mixed).unwrap_err();
    assert!(bodies.materialization_diagnostic(&mixed, &denied).is_some());
    assert!(bodies.admit(&body).is_ok());
}
#[test]
fn materialization_refusal_projection_is_bounded_and_borrowed() {
    use fmt::Write;
    let path = |s: &str| NormalizedAbsolutePath::new(s).unwrap();
    let bodies = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")]);
    let mut request = (*local_native_request(&bodies.roots[0], "platforms+", "/platforms")).clone();
    request.kind = RepositoryMaterializationKind::Immutable;
    let needs = SourcePreparationNeeds::repository(request);
    let original = needs.repository_materializations().values().next().unwrap();
    let weak = Arc::downgrade(original);
    {
        let (borrowed, reason) = bodies.first_denied_materialization(&needs).unwrap();
        let view = MaterializationDiagnostic::new(borrowed, reason);
        assert!(std::ptr::eq(
            view.fields[1].unwrap().as_ptr(),
            borrowed.id.canonical_repo.as_str().as_bytes().as_ptr()
        ));
        let encoded = view.to_string();
        assert!(encoded.starts_with(
            "WP982_MATERIALIZATION capture=captured kind=immutable reason=immutable_kind "
        ));
        assert!(encoded.ends_with(" logical_root_len=none logical_root_hex=none"));
        assert_eq!(Arc::strong_count(original), 1);
        assert!(encoded.len() + 1 <= MATERIALIZATION_MARKER_BYTES);
    }
    drop(needs);
    assert!(weak.upgrade().is_none());
    let mut view = MaterializationDiagnostic {
        kind: "local",
        reason: "binding",
        fields: [Some(&[]); 7],
    };
    assert!(view.to_string().contains("workspace_len=0 workspace_hex=-"));
    let unicode = "\0\né".as_bytes();
    view.fields[0] = Some(unicode);
    assert!(
        view.to_string()
            .contains("workspace_len=4 workspace_hex=000ac3a9")
    );
    let exact = [b'x'; MATERIALIZATION_FIELD_BYTES];
    let overflow = [b'x'; MATERIALIZATION_FIELD_BYTES + 1];
    view.fields = [Some(&[]); 7];
    view.fields[0] = Some(&exact);
    assert!(view.captured());
    view.fields[0] = Some(&overflow);
    assert!(!view.captured());
    view.fields = [Some(&[]); 7];
    for field in &mut view.fields[..4] {
        *field = Some(&exact);
    }
    assert!(view.captured());
    assert!(view.to_string().len() + 1 <= MATERIALIZATION_MARKER_BYTES);
    view.fields[4] = Some(b"x");
    assert!(!view.captured());
    let encoded = view.to_string();
    assert!(encoded.starts_with("WP982_MATERIALIZATION capture=bounds_refused "));
    assert_eq!(encoded.matches("_hex=none").count(), 7);
    assert!(encoded.contains("workspace_len=1024 workspace_hex=none"));
    let mut output = String::new();
    let mut writer = MaterializationWriter {
        writer: &mut output,
        bytes: MATERIALIZATION_MARKER_BYTES - 1,
        limit: MATERIALIZATION_MARKER_BYTES,
    };
    assert!(writer.write_str("x").is_err());
    assert!(output.is_empty());
}

fn generated_globals_request(
    bodies: &Bodies,
    plan: slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan,
) -> slug_bzlmod_v2::RepositoryMaterializationRequest {
    let mut request = (*local_native_request(
        &bodies.roots[0],
        "bazel_features++version_extension+bazel_features_globals",
        "/features",
    ))
    .clone();
    request.repo_spec.rule_id.bzl_file =
        CanonicalLabel::parse("@@bazel_features+//private:globals_repo.bzl").unwrap();
    request.repo_spec.rule_id.rule_name = "globals_repo".into();
    request.kind = RepositoryMaterializationKind::GeneratedFileEffects(plan);
    request
}
// Known small bytes exercise the same borrowed shape seam; native uses only fixed captured signatures.
fn small_generated_plan(
    rows: &[(&str, &[u8], bool)],
) -> slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan {
    slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan::build(rows.iter().map(
        |&(path, bytes, executable)| {
            (
                compact_str::CompactString::new(path),
                Arc::from(bytes),
                executable,
            )
        },
    ))
    .unwrap()
}
fn namespace_test_epoch(
    bodies: &Bodies,
    requests: impl IntoIterator<Item = Arc<slug_bzlmod_v2::RepositoryMaterializationRequest>>,
    instance: slug_workspace_v2::PathObservationInstanceId,
) -> slug_bzlmod_v2::RepositoryMaterializationResultEpoch {
    namespace_test_epoch_at(bodies, requests, instance, "/generation")
}
fn namespace_test_epoch_at(
    bodies: &Bodies,
    requests: impl IntoIterator<Item = Arc<slug_bzlmod_v2::RepositoryMaterializationRequest>>,
    instance: slug_workspace_v2::PathObservationInstanceId,
    root: &str,
) -> slug_bzlmod_v2::RepositoryMaterializationResultEpoch {
    slug_bzlmod_v2::RepositoryMaterializationResultEpoch::new(
        bodies.roots[0].dupe(),
        requests.into_iter().map(
            |request| slug_bzlmod_v2::RepositoryMaterializationEpochEntry {
                request,
                result: slug_bzlmod_v2::RepositoryMaterializationResult::Success(
                    slug_bzlmod_v2::RepositoryMaterializationSuccess::Immutable {
                        source_identity: Arc::from("test"),
                        generation_root: PathBuf::from(root),
                        observation_instance: instance,
                    },
                ),
            },
        ),
    )
    .unwrap()
}
#[test]
fn namespace_refusal_projection_preserves_first_offender_and_borrowed_owner() {
    let path = |s| NormalizedAbsolutePath::new(s).unwrap();
    let bodies = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")]);
    let instance = slug_workspace_v2::PathObservationInstanceId::new(17);
    let need = |namespace, spelling| {
        SourcePreparationNeeds::path(NeedPathObservations::singleton(PathObservationDemand::new(
            namespace,
            path(spelling),
            PathObservationOperation::Lstat,
        )))
    };
    let needs = need(PathObservationNamespace::Materialization(instance), "/");
    let request = Arc::new(generated_globals_request(
        &bodies,
        small_generated_plan(&[]),
    ));
    let epoch = namespace_test_epoch(&bodies, [request.clone()], instance);
    let count = Arc::strong_count(&request);
    let denied = bodies.admit(&needs).unwrap_err();
    let diagnostic = namespace_diagnostic(&bodies, &needs, &denied, Some(&epoch)).unwrap();
    assert_eq!(diagnostic.association, "unique");
    let original = request.id.canonical_repo.as_str().as_bytes();
    assert_eq!(
        diagnostic.owner.as_ref().unwrap().fields[1]
            .unwrap()
            .as_ptr(),
        original.as_ptr()
    );
    assert_eq!(Arc::strong_count(&request), count);
    let printed = diagnostic.to_string();
    assert!(printed.starts_with("WP987_NAMESPACE association=unique namespace=materialization instance=17 operation=lstat\nWP987_NAMESPACE_OWNER capture=captured kind=generated_file_effects reason=observation_instance "));
    assert!(printed.len() < MATERIALIZATION_MARKER_BYTES + 256);
    let empty = namespace_test_epoch(&bodies, [], instance);
    for (epoch, status) in [(None, "unavailable"), (Some(&empty), "missing")] {
        let diagnostic = namespace_diagnostic(&bodies, &needs, &denied, epoch).unwrap();
        assert_eq!(diagnostic.association, status);
        assert!(diagnostic.owner.is_none());
        assert!(!diagnostic.to_string().contains("NAMESPACE_OWNER"));
    }
    let mut other = request.as_ref().clone();
    other.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new("other+").unwrap();
    let ambiguous = namespace_test_epoch(&bodies, [request.clone(), Arc::new(other)], instance);
    let diagnostic = namespace_diagnostic(&bodies, &needs, &denied, Some(&ambiguous)).unwrap();
    assert_eq!(diagnostic.association, "ambiguous");
    assert!(diagnostic.owner.is_none());
    let mixed = needs
        .try_union(&need(PathObservationNamespace::Host, "/unknown"))
        .unwrap();
    let earlier = bodies.admit(&mixed).unwrap_err();
    assert_eq!(earlier.family, "path");
    assert!(namespace_diagnostic(&bodies, &mixed, &earlier, Some(&epoch)).is_none());
    let mixed = needs
        .try_union(&SourcePreparationNeeds::repository(
            request.as_ref().clone(),
        ))
        .unwrap();
    let earlier = bodies.admit(&mixed).unwrap_err();
    assert_eq!(earlier.family, "materialization");
    assert!(namespace_diagnostic(&bodies, &mixed, &earlier, Some(&epoch)).is_none());
    let mut oversized = request.as_ref().clone();
    oversized.repo_spec.rule_id.rule_name = "x".repeat(MATERIALIZATION_FIELD_BYTES + 1).into();
    let bounded = namespace_test_epoch(&bodies, [Arc::new(oversized)], instance);
    let diagnostic = namespace_diagnostic(&bodies, &needs, &denied, Some(&bounded)).unwrap();
    let owner = diagnostic.owner.as_ref().unwrap();
    assert!(!owner.captured());
    let printed = diagnostic.to_string();
    assert!(printed.contains("NAMESPACE_OWNER capture=bounds_refused"));
    assert!(printed.contains("canonical_hex=none"));
    assert_eq!(Arc::strong_count(&request), count + 1);
}
#[tokio::test]
async fn namespace_owner_read_uses_current_transaction() {
    let path = |s| NormalizedAbsolutePath::new(s).unwrap();
    let bodies = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")]);
    let a = Arc::new(generated_globals_request(
        &bodies,
        small_generated_plan(&[]),
    ));
    let mut b = a.as_ref().clone();
    b.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new("other+").unwrap();
    let b = Arc::new(b);
    let dice = dice::Dice::builder().build(dice::DetectCycles::Enabled);
    let instance = slug_workspace_v2::PathObservationInstanceId::new(17);
    let mut old = None;
    for (index, request) in [a.as_ref(), b.as_ref(), a.as_ref()].into_iter().enumerate() {
        let mut updater = dice.updater();
        let owned = if index == 1 { b.clone() } else { a.clone() };
        updater
            .changed_to(vec![(
                slug_bzlmod_v2::RepositoryMaterializationResultEpochKey {
                    workspace: bodies.roots[0].dupe(),
                },
                namespace_test_epoch_at(
                    &bodies,
                    [owned],
                    instance,
                    if index == 1 {
                        "/other-generation"
                    } else {
                        "/generation"
                    },
                ),
            )])
            .unwrap();
        let mut tx = updater.commit().await;
        let current = current_materialization_epoch(&mut tx, &bodies.roots[0])
            .await
            .unwrap();
        assert_eq!(
            current
                .borrowed_instance_for_namespace(&bodies.roots[0], instance, 32)
                .unwrap()
                .unwrap()
                .generation_root(),
            Path::new(if index == 1 {
                "/other-generation"
            } else {
                "/generation"
            })
        );
        assert!(std::ptr::eq(
            current
                .borrowed_instance_for_namespace(&bodies.roots[0], instance, 32)
                .unwrap()
                .unwrap()
                .request(),
            request
        ));
        if index == 0 {
            old = Some(current);
        }
    }
    assert!(std::ptr::eq(
        old.unwrap()
            .borrowed_instance_for_namespace(&bodies.roots[0], instance, 32)
            .unwrap()
            .unwrap()
            .request(),
        a.as_ref()
    ));
}
// Literal output bytes from the exact reviewed cc/extensions.bzl reference; no evaluation is substituted.
const PROXY_LITERAL_0: &[u8] = br###"
load("@bazel_skylib//:bzl_library.bzl", "bzl_library")
bzl_library(
  name = "proxy_bzl",
  srcs = ["proxy.bzl"],
  deps = [
    "@rules_cc//cc/private/rules_impl:core_rules",
    "@rules_cc//cc/private/rules_impl:toolchain_rules",
    "@rules_cc//cc/private:cc_common",
  ],
  visibility = ["@rules_cc//cc:__subpackages__"],
)
bzl_library(
  name = "symbols_bzl",
  srcs = ["symbols.bzl"],
  deps = [
    "@rules_cc//cc/private:cc_common",
    "@rules_cc//cc/private:cc_shared_library_info_bzl",
    "@rules_cc//cc/private/toolchain_config:toolchain_config_bzl",
  ],
  visibility = ["@rules_cc//cc:__subpackages__"],
)
            "###;
const PROXY_LITERAL_1: &[u8] = br###"
load("@rules_cc//cc/private/rules_impl:cc_binary.bzl", _cc_binary = "cc_binary")
load("@rules_cc//cc/private/rules_impl:cc_import.bzl", _cc_import = "cc_import")
load("@rules_cc//cc/private/rules_impl:cc_library.bzl", _cc_library = "cc_library")
load("@rules_cc//cc/private/rules_impl:cc_shared_library.bzl", _cc_shared_library = "cc_shared_library")
load("@rules_cc//cc/private/rules_impl:cc_static_library.bzl", _cc_static_library = "cc_static_library")
load("@rules_cc//cc/private/rules_impl:cc_test.bzl", _cc_test = "cc_test")
load("@rules_cc//cc/private/rules_impl:objc_import.bzl", _objc_import = "objc_import")
load("@rules_cc//cc/private/rules_impl:objc_library.bzl", _objc_library = "objc_library")
load("@rules_cc//cc/private/rules_impl/fdo:fdo_prefetch_hints.bzl", _fdo_prefetch_hints = "fdo_prefetch_hints")
load("@rules_cc//cc/private/rules_impl/fdo:fdo_profile.bzl", _fdo_profile = "fdo_profile")
load("@rules_cc//cc/private/rules_impl/fdo:memprof_profile.bzl", _memprof_profile = "memprof_profile")
load("@rules_cc//cc/private/rules_impl/fdo:propeller_optimize.bzl", _propeller_optimize = "propeller_optimize")
load("@rules_cc//cc/private/rules_impl:cc_toolchain.bzl", _cc_toolchain = "cc_toolchain")
load("@rules_cc//cc/private/rules_impl:cc_toolchain_alias.bzl", _cc_toolchain_alias = "cc_toolchain_alias")

cc_binary = _cc_binary
cc_import = _cc_import
cc_library = _cc_library
cc_shared_library = _cc_shared_library
cc_static_library = _cc_static_library
cc_test = _cc_test
objc_import = _objc_import
objc_library = _objc_library
fdo_prefetch_hints = _fdo_prefetch_hints
fdo_profile = _fdo_profile
memprof_profile = _memprof_profile
propeller_optimize = _propeller_optimize
cc_toolchain = _cc_toolchain
cc_toolchain_alias = _cc_toolchain_alias
            "###;
const PROXY_LITERAL_2: &[u8] = br###"
load("@rules_cc//cc/private:cc_common.bzl", _cc_common = "cc_common")
load("@rules_cc//cc/private:cc_info.bzl", _CcInfo = "CcInfo")
load("@rules_cc//cc/private:cc_shared_library_info.bzl", _CcSharedLibraryInfo = "CcSharedLibraryInfo")
load("@rules_cc//cc/private:debug_package_info.bzl", _DebugPackageInfo = "DebugPackageInfo")
load("@rules_cc//cc/private:objc_info.bzl", _ObjcInfo = "ObjcInfo")
load("@rules_cc//cc/private/toolchain_config:cc_toolchain_config_info.bzl", _CcToolchainConfigInfo = "CcToolchainConfigInfo")

cc_common = _cc_common
CcInfo = _CcInfo
DebugPackageInfo = _DebugPackageInfo
CcToolchainConfigInfo = _CcToolchainConfigInfo
ObjcInfo = _ObjcInfo
new_objc_provider = _ObjcInfo
CcSharedLibraryInfo = _CcSharedLibraryInfo
            "###;
fn proxy_literal_rows() -> [(&'static str, &'static [u8], bool); 3] {
    [
        ("BUILD", PROXY_LITERAL_0, true),
        ("proxy.bzl", PROXY_LITERAL_1, true),
        ("symbols.bzl", PROXY_LITERAL_2, true),
    ]
}
fn proxy_generated_request(
    bodies: &Bodies,
    plan: slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan,
) -> slug_bzlmod_v2::RepositoryMaterializationRequest {
    let mut request = generated_globals_request(bodies, plan);
    request.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new(
        "rules_cc++compatibility_proxy+cc_compatibility_proxy",
    )
    .unwrap();
    request.repo_spec.rule_id.bzl_file =
        CanonicalLabel::parse("@@rules_cc+//cc:extensions.bzl").unwrap();
    request.repo_spec.rule_id.rule_name = "_compatibility_proxy_repo_rule".into();
    request.repo_spec.attributes = Arc::new(starlark_map::small_map::SmallMap::new());
    request
}
fn proxy_test_bodies() -> Bodies {
    namespace_test_bodies()
        .with_fifth(NormalizedAbsolutePath::new("/skylib").unwrap())
        .with_sixth(NormalizedAbsolutePath::new("/cc").unwrap())
        .with_rules_cc_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
}
fn small_namespace_plan_matches(
    plan: &slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan,
    selected: &[GeneratedFileSignature],
) -> bool {
    let second = if std::ptr::eq(selected, &GENERATED_FEATURES_FILES) {
        "globals.bzl"
    } else {
        assert!(std::ptr::eq(selected, &GENERATED_FEATURES_VERSION_FILES));
        "version.bzl"
    };
    generated_features_plan_matches(
        plan,
        &[
            ("BUILD.bazel", 1, true, sha2::Sha256::digest(b"a").into()),
            (second, 1, true, sha2::Sha256::digest(b"b").into()),
        ],
    )
}
fn namespace_test_bodies() -> Bodies {
    let path = |s| NormalizedAbsolutePath::new(s).unwrap();
    Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
        .with_fourth(path("/features"))
        .with_bazel_features_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        )
        .with_generated_features_file_effects(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        )
}
fn namespace_test_request(
    bodies: &Bodies,
    version: bool,
) -> slug_bzlmod_v2::RepositoryMaterializationRequest {
    let mut request = generated_globals_request(
        bodies,
        small_generated_plan(&[
            ("BUILD.bazel", b"a", true),
            (
                if version {
                    "version.bzl"
                } else {
                    "globals.bzl"
                },
                b"b",
                true,
            ),
        ]),
    );
    if version {
        request.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new(
            "bazel_features++version_extension+bazel_features_version",
        )
        .unwrap();
        request.repo_spec.rule_id.bzl_file =
            CanonicalLabel::parse("@@bazel_features+//private:version_repo.bzl").unwrap();
        request.repo_spec.rule_id.rule_name = "version_repo".into();
    }
    request
}
#[test]
fn generated_features_namespace_is_finite_and_exact() {
    let bodies = namespace_test_bodies();
    let instance = slug_workspace_v2::PathObservationInstanceId::new(17);
    let demand = |path, operation| {
        PathObservationDemand::new(
            PathObservationNamespace::Materialization(instance),
            NormalizedAbsolutePath::new(path).unwrap(),
            operation,
        )
    };
    for version in [false, true] {
        let request = Arc::new(namespace_test_request(&bodies, version));
        let epoch =
            namespace_test_epoch_at(&bodies, [request.clone()], instance, "/parent/generation");
        let count = Arc::strong_count(&request);
        let fixed = if version {
            "/parent/generation/version.bzl"
        } else {
            "/parent/generation/globals.bzl"
        };
        for (path, class) in [
            ("/", "ancestor"),
            ("/parent", "ancestor"),
            ("/parent/generation", "root"),
            ("/parent/generation/BUILD.bazel", "fixed_file"),
            (fixed, "fixed_file"),
        ] {
            for operation in [
                PathObservationOperation::Lstat,
                PathObservationOperation::FileBytes,
                PathObservationOperation::FileDigest,
                PathObservationOperation::ReadLink,
                PathObservationOperation::DirectoryEntries,
            ] {
                let d = demand(path, operation.dupe());
                let policy =
                    namespace_policy(&bodies, &d, Some(&epoch), &small_namespace_plan_matches);
                assert!(policy.owner_plan_allowed);
                assert_eq!(policy.path_class, class);
                assert_eq!(
                    policy.admits(&operation),
                    operation == PathObservationOperation::Lstat
                        || class == "fixed_file"
                            && operation == PathObservationOperation::FileBytes
                );
                // The normal seam uses fixed captured signatures, never the tiny test signatures.
                assert!(
                    !namespace_policy(&bodies, &d, Some(&epoch), &generated_features_plan_matches)
                        .admits(&operation)
                );
            }
        }
        for path in [
            "/outside",
            "/parent/generation-sibling/BUILD.bazel",
            "/parent/generation/child/BUILD.bazel",
            "/parent/generation/BUILD.bazel/child",
            "/parent/generation/build.bazel",
            "/parent/generation/BUILD.bazel.bak",
            if version {
                "/parent/generation/globals.bzl"
            } else {
                "/parent/generation/version.bzl"
            },
        ] {
            for operation in [
                PathObservationOperation::Lstat,
                PathObservationOperation::FileBytes,
            ] {
                assert!(
                    !namespace_policy(
                        &bodies,
                        &demand(path, operation.dupe()),
                        Some(&epoch),
                        &small_namespace_plan_matches
                    )
                    .admits(&operation)
                );
            }
        }
        let d = demand(fixed, PathObservationOperation::FileBytes);
        let budget = slug_workspace_v2::ImageDescriptorBudget::new(4096);
        let pinned =
            slug_workspace_v2::ImageChargedPath::pin_existing(d.path().dupe(), &budget).unwrap();
        let image = PathObservationDemand::input_image(
            PathObservationNamespace::Materialization(instance),
            pinned.dupe(),
            slug_workspace_v2::ImageObservationRequest::new(
                pinned,
                slug_workspace_v2::ImageObservationOperation::Lstat,
            ),
        )
        .unwrap();
        assert!(
            !namespace_policy(&bodies, &image, Some(&epoch), &small_namespace_plan_matches)
                .admits(&image.operation())
        );
        for index in 0..4 {
            let mut closed = namespace_test_bodies();
            match index {
                0 => closed.generated_features_file_effects = false,
                1 => closed.bazel_features_import_sources = false,
                2 => closed.additional[0] = None,
                3 => closed.roots[0] = NormalizedAbsolutePath::new("/wrong").unwrap(),
                _ => unreachable!(),
            }
            assert!(
                !namespace_policy(&closed, &d, Some(&epoch), &|_, _| true).admits(&d.operation())
            );
        }
        for index in 0..4 {
            let mut wrong = request.as_ref().clone();
            match index {
                0 => {
                    wrong.id.canonical_repo =
                        slug_identity_v2::CanonicalRepoName::new("other+").unwrap()
                }
                1 => wrong.repo_spec.rule_id.rule_name = "other".into(),
                2 => wrong.kind = RepositoryMaterializationKind::Immutable,
                3 => wrong.kind = namespace_test_request(&bodies, !version).kind,
                _ => unreachable!(),
            }
            let wrong =
                namespace_test_epoch_at(&bodies, [Arc::new(wrong)], instance, "/parent/generation");
            assert!(
                !namespace_policy(&bodies, &d, Some(&wrong), &small_namespace_plan_matches)
                    .admits(&d.operation())
            );
        }
        assert_eq!(Arc::strong_count(&request), count);
    }

    let bodies = proxy_test_bodies();
    let request = Arc::new(proxy_generated_request(
        &bodies,
        small_generated_plan(&proxy_literal_rows()),
    ));
    let weak = Arc::downgrade(&request);
    let epoch = namespace_test_epoch_at(
        &bodies,
        [request.clone()],
        instance,
        "/parent/proxy-generation",
    );
    let count = Arc::strong_count(&request);
    for (path, class) in [
        ("/", "ancestor"),
        ("/parent", "ancestor"),
        ("/parent/proxy-generation", "root"),
        ("/parent/proxy-generation/BUILD", "fixed_file"),
        ("/parent/proxy-generation/proxy.bzl", "fixed_file"),
        ("/parent/proxy-generation/symbols.bzl", "fixed_file"),
    ] {
        for operation in [
            PathObservationOperation::Lstat,
            PathObservationOperation::FileBytes,
            PathObservationOperation::FileDigest,
            PathObservationOperation::ReadLink,
            PathObservationOperation::DirectoryEntries,
        ] {
            let d = demand(path, operation.dupe());
            let policy =
                namespace_policy(&bodies, &d, Some(&epoch), &generated_features_plan_matches);
            assert!(policy.owner_plan_allowed);
            assert_eq!(policy.path_class, class);
            assert_eq!(
                policy.admits(&operation),
                operation == PathObservationOperation::Lstat
                    || class == "fixed_file" && operation == PathObservationOperation::FileBytes
            );
        }
    }
    for path in [
        "/parent/proxy-generation/BUILD.bazel",
        "/parent/proxy-generation/Proxy.bzl",
        "/parent/proxy-generation/proxy.bzl.bak",
        "/parent/proxy-generation/symbols.bzl/child",
        "/parent/proxy-generation-sibling/proxy.bzl",
        "/outside",
    ] {
        let d = demand(path, PathObservationOperation::Lstat);
        assert!(
            !namespace_policy(&bodies, &d, Some(&epoch), &generated_features_plan_matches)
                .admits(&d.operation())
        );
    }
    let d = demand(
        "/parent/proxy-generation/proxy.bzl",
        PathObservationOperation::FileBytes,
    );
    for absent in [
        None,
        Some(namespace_test_epoch_at(
            &bodies,
            [request.clone()],
            instance,
            "/other-generation",
        )),
        Some(namespace_test_epoch_at(
            &bodies,
            [request.clone()],
            slug_workspace_v2::PathObservationInstanceId::new(18),
            "/parent/proxy-generation",
        )),
    ] {
        assert!(
            !namespace_policy(
                &bodies,
                &d,
                absent.as_ref(),
                &generated_features_plan_matches
            )
            .admits(&d.operation())
        );
    }
    assert!(std::ptr::eq(
        epoch
            .borrowed_instance_for_namespace(&bodies.roots[0], instance, 32)
            .unwrap()
            .unwrap()
            .request(),
        request.as_ref()
    ));
    assert_eq!(Arc::strong_count(&request), count);
    drop(epoch);
    drop(request);
    assert!(weak.upgrade().is_none());
}
#[test]
fn generated_features_namespace_refuses_mixed_need_before_progress() {
    let bodies = namespace_test_bodies();
    let instance = slug_workspace_v2::PathObservationInstanceId::new(17);
    let a = Arc::new(namespace_test_request(&bodies, false));
    let b = Arc::new(namespace_test_request(&bodies, true));
    let epoch = namespace_test_epoch(&bodies, [a.clone()], instance);
    let need = |namespace, path| {
        SourcePreparationNeeds::path(NeedPathObservations::singleton(PathObservationDemand::new(
            namespace,
            NormalizedAbsolutePath::new(path).unwrap(),
            PathObservationOperation::Lstat,
        )))
    };
    let valid = need(
        PathObservationNamespace::Materialization(instance),
        "/generation/BUILD.bazel",
    );
    let admits =
        |needs: &SourcePreparationNeeds,
         epoch: Option<&slug_bzlmod_v2::RepositoryMaterializationResultEpoch>| {
            bodies.admit_with_matchers(needs, &small_namespace_plan_matches, &|demand| {
                namespace_policy(&bodies, demand, epoch, &small_namespace_plan_matches)
                    .admits(&demand.operation())
            })
        };
    assert!(admits(&valid, Some(&epoch)).is_ok());
    let sibling_instance = slug_workspace_v2::PathObservationInstanceId::new(18);
    let paired = slug_bzlmod_v2::RepositoryMaterializationResultEpoch::new(
        bodies.roots[0].dupe(),
        [
            (a.clone(), instance, "/generation"),
            (b.clone(), sibling_instance, "/version-generation"),
        ]
        .into_iter()
        .map(|(request, observation_instance, root)| {
            slug_bzlmod_v2::RepositoryMaterializationEpochEntry {
                request,
                result: slug_bzlmod_v2::RepositoryMaterializationResult::Success(
                    slug_bzlmod_v2::RepositoryMaterializationSuccess::Immutable {
                        source_identity: Arc::from("test"),
                        generation_root: PathBuf::from(root),
                        observation_instance,
                    },
                ),
            }
        }),
    )
    .unwrap();
    let cohort = valid
        .try_union(&need(
            PathObservationNamespace::Materialization(sibling_instance),
            "/version-generation/version.bzl",
        ))
        .unwrap();
    assert!(admits(&cohort, Some(&paired)).is_ok());
    let crossed = cohort
        .try_union(&need(
            PathObservationNamespace::Materialization(instance),
            "/version-generation/version.bzl",
        ))
        .unwrap();
    assert!(admits(&crossed, Some(&paired)).is_err());
    let mut progress = 0;
    for bad in [
        need(
            PathObservationNamespace::Materialization(instance),
            "/generation/outside",
        ),
        need(
            PathObservationNamespace::Materialization(
                slug_workspace_v2::PathObservationInstanceId::new(18),
            ),
            "/generation/BUILD.bazel",
        ),
        need(PathObservationNamespace::Host, "/unknown"),
    ] {
        let mixed = valid.try_union(&bad).unwrap();
        if admits(&mixed, Some(&epoch)).is_ok() {
            progress += 1;
        }
        let denied = admits(&mixed, Some(&epoch)).unwrap_err();
        let (first, family) = bodies
            .first_denied_path_with_namespace_matcher(&mixed, &|demand| {
                namespace_policy(&bodies, demand, Some(&epoch), &small_namespace_plan_matches)
                    .admits(&demand.operation())
            })
            .unwrap();
        assert_eq!(denied.family, family);
        assert_eq!(denied.path.as_ref(), Some(first.path()));
        assert_eq!(
            first.path(),
            bad.path_observations()
                .unwrap()
                .demands()
                .first()
                .unwrap()
                .path()
        );
    }
    assert_eq!(progress, 0);
    for unavailable in [
        None,
        Some(namespace_test_epoch(&bodies, [], instance)),
        Some(namespace_test_epoch(&bodies, [a.clone(), b], instance)),
        Some(namespace_test_epoch_at(
            &bodies,
            [a.clone()],
            instance,
            "/new-generation",
        )),
    ] {
        assert!(admits(&valid, unavailable.as_ref()).is_err());
    }
    let mixed = valid
        .try_union(&SourcePreparationNeeds::repository(namespace_test_request(
            &bodies, false,
        )))
        .unwrap();
    assert!(admits(&mixed, Some(&epoch)).is_ok());
    let mut bad = namespace_test_request(&bodies, false);
    bad.repo_spec.rule_id.rule_name = "unknown".into();
    let mixed = mixed
        .try_union(&SourcePreparationNeeds::repository({
            bad.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new("unknown+").unwrap();
            bad
        }))
        .unwrap();
    assert_eq!(
        admits(&mixed, Some(&epoch)).unwrap_err().family,
        "materialization"
    );
}
#[test]
fn generated_features_materialization_is_opt_in_and_exact() {
    let path = |s: &str| NormalizedAbsolutePath::new(s).unwrap();
    let make = || Bodies::new([path("/fixture"), path("/rules"), path("/platforms")]);
    let expected = [
        [
            ("BUILD.bazel", 1, true, sha2::Sha256::digest(b"a").into()),
            ("globals.bzl", 1, true, sha2::Sha256::digest(b"b").into()),
        ],
        [
            ("BUILD.bazel", 1, true, sha2::Sha256::digest(b"a").into()),
            ("version.bzl", 1, true, sha2::Sha256::digest(b"b").into()),
        ],
    ];
    let matches = |plan: &_, selected: &[GeneratedFileSignature]| {
        let index = if std::ptr::eq(selected, &GENERATED_FEATURES_FILES) {
            0
        } else {
            assert!(std::ptr::eq(selected, &GENERATED_FEATURES_VERSION_FILES));
            1
        };
        generated_features_plan_matches(plan, &expected[index])
    };
    for version in [false, true] {
        let second = if version {
            "version.bzl"
        } else {
            "globals.bzl"
        };
        let rows: &[(&str, &[u8], bool)] = &[("BUILD.bazel", b"a", true), (second, b"b", true)];
        let request_for_plan = |bodies: &Bodies, plan| {
            let mut request = generated_globals_request(bodies, plan);
            if version {
                request.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new(
                    "bazel_features++version_extension+bazel_features_version",
                )
                .unwrap();
                request.repo_spec.rule_id.bzl_file =
                    CanonicalLabel::parse("@@bazel_features+//private:version_repo.bzl").unwrap();
                request.repo_spec.rule_id.rule_name = "version_repo".into();
            }
            request
        };
        let bodies = make()
            .with_fourth(path("/features"))
            .with_bazel_features_import_sources(
                RegistryChoice::PreparedFile,
                CommandChoice::PublicProbe,
            )
            .with_generated_features_file_effects(
                RegistryChoice::PreparedFile,
                CommandChoice::PublicProbe,
            );
        let needs = SourcePreparationNeeds::repository(request_for_plan(
            &bodies,
            small_generated_plan(rows),
        ));
        assert!(bodies.admit_with_plan_matcher(&needs, &matches).is_ok());
        assert!(bodies.admit(&needs).is_err()); // The native fixed signature is never the synthetic signature.
        for registry in [RegistryChoice::DefaultBcr, RegistryChoice::PreparedFile] {
            for command in [CommandChoice::Modules, CommandChoice::PublicProbe] {
                let configured = make()
                    .with_fourth(path("/features"))
                    .with_bazel_features_import_sources(registry, command)
                    .with_generated_features_file_effects(registry, command);
                assert_eq!(
                    configured.admit_with_plan_matcher(&needs, &matches).is_ok(),
                    matches!(registry, RegistryChoice::PreparedFile)
                        && matches!(command, CommandChoice::PublicProbe)
                );
            }
        }
        for closed in [
            make(),
            make().with_fourth(path("/features")),
            make().with_generated_features_file_effects(
                RegistryChoice::PreparedFile,
                CommandChoice::PublicProbe,
            ),
            make()
                .with_fourth(path("/features"))
                .with_bazel_features_import_sources(
                    RegistryChoice::PreparedFile,
                    CommandChoice::PublicProbe,
                ),
            make()
                .with_fourth(path("/features"))
                .with_generated_features_file_effects(
                    RegistryChoice::PreparedFile,
                    CommandChoice::PublicProbe,
                ),
        ] {
            assert!(
                closed
                    .admit_with_plan_matcher(&needs, &|_, _| true)
                    .is_err()
            );
        }
        let mut wrong_slot = make()
            .with_fourth(path("/other"))
            .with_fifth(path("/features"))
            .with_bazel_features_import_sources(
                RegistryChoice::PreparedFile,
                CommandChoice::PublicProbe,
            )
            .with_generated_features_file_effects(
                RegistryChoice::PreparedFile,
                CommandChoice::PublicProbe,
            );
        wrong_slot.additional[0] = None;
        assert!(
            wrong_slot
                .admit_with_plan_matcher(&needs, &|_, _| true)
                .is_err()
        );
        // A permissive shape closure cannot bypass any typed owner field or mapping provenance.
        for index in 0..7 {
            let mut request = request_for_plan(&bodies, small_generated_plan(rows));
            match index {
                0 => request.id.workspace = path("/other"),
                1 => {
                    request.id.canonical_repo =
                        slug_identity_v2::CanonicalRepoName::new("other+").unwrap()
                }
                2 => {
                    request.repo_spec.rule_id.bzl_file = CanonicalLabel::parse(if version {
                        "@@other//private:version_repo.bzl"
                    } else {
                        "@@other//private:globals_repo.bzl"
                    })
                    .unwrap()
                }
                3 => {
                    request.repo_spec.rule_id.bzl_file = CanonicalLabel::parse(if version {
                        "@@bazel_features+//other:version_repo.bzl"
                    } else {
                        "@@bazel_features+//other:globals_repo.bzl"
                    })
                    .unwrap()
                }
                4 => {
                    request.repo_spec.rule_id.bzl_file =
                        CanonicalLabel::parse("@@bazel_features+//private:other.bzl").unwrap()
                }
                5 => request.repo_spec.rule_id.rule_name = "other".into(),
                6 => {
                    let mut mapping = slug_identity_v2::RepositoryMapping::new(
                        slug_identity_v2::RepositoryMappingId::new("mapped").unwrap(),
                    );
                    mapping.insert(
                        slug_identity_v2::ApparentRepoName::new("features").unwrap(),
                        slug_identity_v2::CanonicalRepoName::new("bazel_features+").unwrap(),
                    );
                    request.repo_spec.rule_id.bzl_file =
                        slug_identity_v2::ApparentLabel::parse(if version {
                            "@features//private:version_repo.bzl"
                        } else {
                            "@features//private:globals_repo.bzl"
                        })
                        .unwrap()
                        .resolve(&mapping);
                }
                _ => unreachable!(),
            }
            assert!(
                bodies
                    .admit_with_plan_matcher(
                        &SourcePreparationNeeds::repository(request),
                        &|_, _| true
                    )
                    .is_err()
            );
        }
        // Atomic owner selection cannot be assembled from sibling components.
        for field in 0..3 {
            let mut crossed = request_for_plan(&bodies, small_generated_plan(rows));
            match field {
                0 => {
                    crossed.id.canonical_repo =
                        slug_identity_v2::CanonicalRepoName::new(if version {
                            "bazel_features++version_extension+bazel_features_globals"
                        } else {
                            "bazel_features++version_extension+bazel_features_version"
                        })
                        .unwrap()
                }
                1 => {
                    crossed.repo_spec.rule_id.bzl_file = CanonicalLabel::parse(if version {
                        "@@bazel_features+//private:globals_repo.bzl"
                    } else {
                        "@@bazel_features+//private:version_repo.bzl"
                    })
                    .unwrap()
                }
                2 => {
                    crossed.repo_spec.rule_id.rule_name = if version {
                        "globals_repo"
                    } else {
                        "version_repo"
                    }
                    .into()
                }
                _ => unreachable!(),
            }
            assert!(
                bodies
                    .admit_with_plan_matcher(
                        &SourcePreparationNeeds::repository(crossed),
                        &|_, _| true
                    )
                    .is_err()
            );
        }
        let crossed_shape = request_for_plan(
            &bodies,
            small_generated_plan(&[
                ("BUILD.bazel", b"a", true),
                (
                    if version {
                        "globals.bzl"
                    } else {
                        "version.bzl"
                    },
                    b"b",
                    true,
                ),
            ]),
        );
        assert!(
            bodies
                .admit_with_plan_matcher(
                    &SourcePreparationNeeds::repository(crossed_shape),
                    &matches
                )
                .is_err()
        );
        let invalid: &[&[(&str, &[u8], bool)]] = &[
            &[],
            &[("BUILD.bazel", b"a", true)],
            &[
                ("BUILD.bazel", b"a", true),
                (second, b"b", true),
                ("extra", b"", false),
            ],
            &[(second, b"b", true), ("BUILD.bazel", b"a", true)],
            &[("BUILD", b"a", true), (second, b"b", true)],
            &[("BUILD.bazel", b"aa", true), (second, b"b", true)],
            &[("BUILD.bazel", b"x", true), (second, b"b", true)],
            &[("BUILD.bazel", b"a", false), (second, b"b", true)],
            &[("BUILD.bazel", b"a", true), (second, b"x", true)],
            &[("BUILD.bazel", b"a", true), (second, b"b", false)],
        ];
        for rows in invalid {
            let needs = SourcePreparationNeeds::repository(request_for_plan(
                &bodies,
                small_generated_plan(rows),
            ));
            assert!(bodies.admit_with_plan_matcher(&needs, &matches).is_err());
        }
        for symlink in [false, true] {
            let mut builder = slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan::builder();
            if symlink {
                builder
                    .push_symlink(
                        compact_str::CompactString::new("LINK"),
                        slug_bzlmod_v2::HostRepositoryLabelPathValue::new(
                            path("/source"),
                            PathObservationNamespace::Host,
                        )
                        .unwrap(),
                        Arc::from(&b"opaque"[..]),
                    )
                    .unwrap();
            } else {
                builder
                    .push_archive(
                        Arc::from([compact_str::CompactString::new(
                            "https://mirror.example/dist/rustc.tar.xz",
                        )]),
                        [0; 32],
                        compact_str::CompactString::new("prefix"),
                    )
                    .unwrap();
            }
            assert!(!matches(
                &builder.finish(),
                if version {
                    &GENERATED_FEATURES_VERSION_FILES
                } else {
                    &GENERATED_FEATURES_FILES
                }
            ));
        }
    }

    let bodies = proxy_test_bodies();
    let rows = proxy_literal_rows();
    let request = || proxy_generated_request(&bodies, small_generated_plan(&rows));
    let valid = SourcePreparationNeeds::repository(request());
    assert!(bodies.admit(&valid).is_ok()); // Full native descriptor positive, not a tiny-signature closure.
    let actual_plan = small_generated_plan(&rows);
    for expected in [
        &GENERATED_CC_PROXY_FILES[..0],
        &GENERATED_CC_PROXY_FILES[..1],
    ] {
        assert!(!generated_features_plan_matches(&actual_plan, expected));
    }
    let oversized = [
        GENERATED_CC_PROXY_FILES[0],
        GENERATED_CC_PROXY_FILES[1],
        GENERATED_CC_PROXY_FILES[2],
        GENERATED_CC_PROXY_FILES[2],
    ];
    assert!(!generated_features_plan_matches(&actual_plan, &oversized));

    for registry in [RegistryChoice::DefaultBcr, RegistryChoice::PreparedFile] {
        for command in [CommandChoice::Modules, CommandChoice::PublicProbe] {
            let configured = proxy_test_bodies()
                .with_generated_features_file_effects(registry, command)
                .with_rules_cc_import_sources(registry, command);
            assert_eq!(
                configured.admit(&valid).is_ok(),
                matches!(registry, RegistryChoice::PreparedFile)
                    && matches!(command, CommandChoice::PublicProbe)
            );
        }
    }
    for index in 0..4 {
        let mut closed = proxy_test_bodies();
        match index {
            0 => closed.generated_features_file_effects = false,
            1 => closed.rules_cc_import_sources = false,
            2 => closed.additional[2] = None,
            3 => closed.roots[0] = path("/other"),
            _ => unreachable!(),
        }
        assert!(
            closed
                .admit_with_plan_matcher(&valid, &|_, _| true)
                .is_err()
        );
    }
    for index in 0..8 {
        let mut wrong = request();
        match index {
            0 => wrong.id.workspace = path("/other"),
            1 => {
                wrong.id.canonical_repo =
                    slug_identity_v2::CanonicalRepoName::new("rules_cc++compatibility_proxy+other")
                        .unwrap()
            }
            2 => {
                wrong.repo_spec.rule_id.bzl_file =
                    CanonicalLabel::parse("@@other+//cc:extensions.bzl").unwrap()
            }
            3 => {
                wrong.repo_spec.rule_id.bzl_file =
                    CanonicalLabel::parse("@@rules_cc+//private:extensions.bzl").unwrap()
            }
            4 => {
                wrong.repo_spec.rule_id.bzl_file =
                    CanonicalLabel::parse("@@rules_cc+//cc:other.bzl").unwrap()
            }
            5 => wrong.repo_spec.rule_id.rule_name = "other".into(),
            6 => {
                let mut mapping = slug_identity_v2::RepositoryMapping::new(
                    slug_identity_v2::RepositoryMappingId::new("mapped").unwrap(),
                );
                mapping.insert(
                    slug_identity_v2::ApparentRepoName::new("cc").unwrap(),
                    slug_identity_v2::CanonicalRepoName::new("rules_cc+").unwrap(),
                );
                wrong.repo_spec.rule_id.bzl_file =
                    slug_identity_v2::ApparentLabel::parse("@cc//cc:extensions.bzl")
                        .unwrap()
                        .resolve(&mapping);
            }
            7 => {
                let mut attributes = starlark_map::small_map::SmallMap::new();
                attributes.insert(
                    compact_str::CompactString::new("unexpected"),
                    slug_bzlmod_v2::OverrideAttributeValue::None,
                );
                wrong.repo_spec.attributes = Arc::new(attributes);
            }
            _ => unreachable!(),
        }
        assert!(
            bodies
                .admit_with_plan_matcher(&SourcePreparationNeeds::repository(wrong), &|_, _| true)
                .is_err()
        );
    }
    let reject = |rows: &[(&str, &[u8], bool)]| {
        assert!(
            bodies
                .admit(&SourcePreparationNeeds::repository(
                    proxy_generated_request(&bodies, small_generated_plan(rows))
                ))
                .is_err()
        );
    };
    reject(&[]);
    reject(&rows[..2]);
    reject(&[rows[0], rows[1], rows[2], ("extra", b"", true)]);
    let mut reversed = rows;
    reversed.swap(0, 2);
    reject(&reversed);
    let mutant = [b'x'; 1776];
    for file in 0..3 {
        for change in 0..4 {
            let mut changed = rows;
            match change {
                0 => changed[file].0 = "wrong.bzl",
                1 => changed[file].1 = b"",
                2 => changed[file].1 = &mutant[..rows[file].1.len()],
                3 => changed[file].2 = false,
                _ => unreachable!(),
            }
            reject(&changed);
        }
    }
    // Older branch lengths are closed even if the owner is exact.
    reject(&[
        ("BUILD", &mutant[..436], true),
        ("proxy.bzl", &mutant[..609], true),
        ("symbols.bzl", &mutant[..718], true),
    ]);
    // Neither cohort can borrow the other's shape, even under the correct owner.
    reject(&[("BUILD.bazel", b"a", true), ("globals.bzl", b"b", true)]);
    assert!(
        bodies
            .admit(&SourcePreparationNeeds::repository(
                generated_globals_request(&bodies, small_generated_plan(&rows))
            ))
            .is_err()
    );
    for symlink in [false, true] {
        let mut builder = slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan::builder();
        for (path, bytes, executable) in rows {
            builder
                .push(
                    compact_str::CompactString::new(path),
                    Arc::from(bytes),
                    executable,
                )
                .unwrap();
        }
        if symlink {
            builder
                .push_symlink(
                    compact_str::CompactString::new("LINK"),
                    slug_bzlmod_v2::HostRepositoryLabelPathValue::new(
                        path("/source"),
                        PathObservationNamespace::Host,
                    )
                    .unwrap(),
                    Arc::from(&b"opaque"[..]),
                )
                .unwrap();
        } else {
            builder
                .push_archive(
                    Arc::from([compact_str::CompactString::new(
                        "https://mirror.example/dist/closed.tar.xz",
                    )]),
                    [0; 32],
                    compact_str::CompactString::new("prefix"),
                )
                .unwrap();
        }
        assert!(!generated_features_plan_matches(
            &builder.finish(),
            &GENERATED_CC_PROXY_FILES
        ));
    }
}
#[test]
fn generated_features_materialization_refuses_mixed_need_before_progress() {
    let path = |s: &str| NormalizedAbsolutePath::new(s).unwrap();
    let bodies = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
        .with_fourth(path("/features"))
        .with_bazel_features_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        )
        .with_generated_features_file_effects(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        );
    let expected = [
        [
            ("BUILD.bazel", 1, true, sha2::Sha256::digest(b"a").into()),
            ("globals.bzl", 1, true, sha2::Sha256::digest(b"b").into()),
        ],
        [
            ("BUILD.bazel", 1, true, sha2::Sha256::digest(b"a").into()),
            ("version.bzl", 1, true, sha2::Sha256::digest(b"b").into()),
        ],
    ];
    let matches = |plan: &_, selected: &[GeneratedFileSignature]| {
        let index = if std::ptr::eq(selected, &GENERATED_FEATURES_FILES) {
            0
        } else {
            assert!(std::ptr::eq(selected, &GENERATED_FEATURES_VERSION_FILES));
            1
        };
        generated_features_plan_matches(plan, &expected[index])
    };
    let version_request = |plan| {
        let mut request = generated_globals_request(&bodies, plan);
        request.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new(
            "bazel_features++version_extension+bazel_features_version",
        )
        .unwrap();
        request.repo_spec.rule_id.bzl_file =
            CanonicalLabel::parse("@@bazel_features+//private:version_repo.bzl").unwrap();
        request.repo_spec.rule_id.rule_name = "version_repo".into();
        request
    };
    let valid = SourcePreparationNeeds::repository(generated_globals_request(
        &bodies,
        small_generated_plan(&[("BUILD.bazel", b"a", true), ("globals.bzl", b"b", true)]),
    ));
    let original = valid.repository_materializations().values().next().unwrap();
    let weak = Arc::downgrade(original);
    let count = Arc::strong_count(original);
    let mut progress = 0;
    if bodies.admit_with_plan_matcher(&valid, &matches).is_ok() {
        progress += 1;
    }
    assert_eq!(progress, 1);
    assert_eq!(Arc::strong_count(original), count);
    assert!(std::ptr::eq(
        original.as_ref(),
        valid
            .repository_materializations()
            .values()
            .next()
            .unwrap()
            .as_ref()
    ));
    let version = SourcePreparationNeeds::repository(version_request(small_generated_plan(&[
        ("BUILD.bazel", b"a", true),
        ("version.bzl", b"b", true),
    ])));
    let version_original = version
        .repository_materializations()
        .values()
        .next()
        .unwrap();
    let version_weak = Arc::downgrade(version_original);
    let version_count = Arc::strong_count(version_original);
    for cohort in [
        valid.try_union(&version).unwrap(),
        version.try_union(&valid).unwrap(),
    ] {
        assert!(bodies.admit_with_plan_matcher(&cohort, &matches).is_ok());
        for held in [original, version_original] {
            let present = cohort
                .repository_materializations()
                .values()
                .find(|r| std::ptr::eq(r.as_ref(), held.as_ref()))
                .unwrap();
            assert!(Arc::ptr_eq(present, held));
        }
    }
    assert_eq!(Arc::strong_count(original), count);
    assert_eq!(Arc::strong_count(version_original), version_count);
    let cohort = valid.try_union(&version).unwrap();
    let local = SourcePreparationNeeds::repository(
        (*local_native_request(&bodies.roots[0], "platforms", "/platforms")).clone(),
    );
    assert!(
        bodies
            .admit_with_plan_matcher(&cohort.try_union(&local).unwrap(), &matches)
            .is_ok()
    );
    let mut unknown = generated_globals_request(&bodies, small_generated_plan(&[]));
    unknown.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new("unknown+").unwrap();
    let mut immutable = (*local_native_request(&bodies.roots[0], "immutable+", "/foreign")).clone();
    immutable.kind = RepositoryMaterializationKind::Immutable;
    let wrong_local = SourcePreparationNeeds::repository(
        (*local_native_request(&bodies.roots[0], "platforms", "/foreign")).clone(),
    );
    let denied_path =
        SourcePreparationNeeds::path(NeedPathObservations::singleton(PathObservationDemand::new(
            PathObservationNamespace::Host,
            path("/fixture/main.rs"),
            PathObservationOperation::FileBytes,
        )));
    for (excluded, family) in [
        (
            SourcePreparationNeeds::repository(unknown),
            "materialization",
        ),
        (
            SourcePreparationNeeds::repository(immutable),
            "materialization",
        ),
        (wrong_local, "materialization"),
        (denied_path, "path"),
        (
            SourcePreparationNeeds::root_module_bootstrap(
                slug_bzlmod_v2::RootModuleBootstrapRequest {
                    workspace: path("/foreign"),
                },
            ),
            "bootstrap",
        ),
    ] {
        for mixed in [
            cohort.try_union(&excluded).unwrap(),
            excluded.try_union(&cohort).unwrap(),
        ] {
            let mut progress = 0;
            if bodies.admit_with_plan_matcher(&mixed, &matches).is_ok() {
                progress += 1;
            }
            assert_eq!(progress, 0);
            assert_eq!(
                bodies
                    .admit_with_plan_matcher(&mixed, &matches)
                    .unwrap_err()
                    .family,
                family
            );
            if family == "materialization" {
                let (offender, _) = bodies
                    .first_denied_materialization_with_plan_matcher(&mixed, &matches)
                    .unwrap();
                let held = mixed
                    .repository_materializations()
                    .values()
                    .find(|r| std::ptr::eq(r.as_ref(), offender))
                    .unwrap();
                assert!(std::ptr::eq(held.as_ref(), offender));
                assert!(!std::ptr::eq(offender, original.as_ref()));
            }
        }
    }
    // A changed plan keeps the exact owner but cannot progress even beside an otherwise valid Local request.
    let changed = SourcePreparationNeeds::repository(generated_globals_request(
        &bodies,
        small_generated_plan(&[("BUILD.bazel", b"c", true), ("globals.bzl", b"b", true)]),
    ));
    let changed_version =
        SourcePreparationNeeds::repository(version_request(small_generated_plan(&[
            ("BUILD.bazel", b"a", true),
            ("version.bzl", b"x", true),
        ])));
    for changed in [changed, changed_version] {
        let accepted_sibling = if changed
            .repository_materializations()
            .values()
            .next()
            .unwrap()
            .id
            .canonical_repo
            .as_str()
            == "bazel_features++version_extension+bazel_features_globals"
        {
            &version
        } else {
            &valid
        };
        let with_local = accepted_sibling.try_union(&local).unwrap();
        for mixed in [
            with_local.try_union(&changed).unwrap(),
            changed.try_union(&with_local).unwrap(),
        ] {
            let mut progress = 0;
            if bodies.admit_with_plan_matcher(&mixed, &matches).is_ok() {
                progress += 1;
            }
            assert_eq!(progress, 0);
            assert_eq!(
                bodies
                    .admit_with_plan_matcher(&mixed, &matches)
                    .unwrap_err()
                    .family,
                "materialization"
            );
        }
    }
    drop(cohort);

    let bodies = proxy_test_bodies();
    let proxy = SourcePreparationNeeds::repository(proxy_generated_request(
        &bodies,
        small_generated_plan(&proxy_literal_rows()),
    ));
    let held = proxy.repository_materializations().values().next().unwrap();
    let proxy_weak = Arc::downgrade(held);
    let count = Arc::strong_count(held);
    let matcher = |plan: &_, signatures: &[GeneratedFileSignature]| {
        if std::ptr::eq(signatures, &GENERATED_CC_PROXY_FILES[..]) {
            generated_features_plan_matches(plan, signatures)
        } else {
            matches(plan, signatures)
        }
    };
    let features = valid.try_union(&version).unwrap();
    for cohort in [
        features.try_union(&proxy).unwrap(),
        proxy.try_union(&features).unwrap(),
    ] {
        assert!(bodies.admit_with_plan_matcher(&cohort, &matcher).is_ok());
        assert!(
            cohort
                .repository_materializations()
                .values()
                .any(|request| Arc::ptr_eq(request, held))
        );
        let mut unknown =
            proxy_generated_request(&bodies, small_generated_plan(&proxy_literal_rows()));
        unknown.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new("unknown+").unwrap();
        let excluded = SourcePreparationNeeds::repository(unknown);
        for mixed in [
            cohort.try_union(&excluded).unwrap(),
            excluded.try_union(&cohort).unwrap(),
        ] {
            let mut progress = 0;
            if bodies.admit_with_plan_matcher(&mixed, &matcher).is_ok() {
                progress += 1;
            }
            assert_eq!(progress, 0);
            let (original, _) = bodies
                .first_denied_materialization_with_plan_matcher(&mixed, &matcher)
                .unwrap();
            assert_eq!(original.id.canonical_repo.as_str(), "unknown+");
            assert!(
                mixed
                    .repository_materializations()
                    .values()
                    .any(|request| std::ptr::eq(request.as_ref(), original))
            );
        }
    }
    let instance = slug_workspace_v2::PathObservationInstanceId::new(19);
    let epoch = namespace_test_epoch_at(&bodies, [held.clone()], instance, "/proxy-generation");
    let leaf = |path| {
        SourcePreparationNeeds::path(NeedPathObservations::singleton(PathObservationDemand::new(
            PathObservationNamespace::Materialization(instance),
            NormalizedAbsolutePath::new(path).unwrap(),
            PathObservationOperation::FileBytes,
        )))
    };
    let own = proxy
        .try_union(&leaf("/proxy-generation/symbols.bzl"))
        .unwrap();
    let admit = |needs: &SourcePreparationNeeds| {
        bodies.admit_with_matchers(needs, &matcher, &|demand| {
            namespace_policy(&bodies, demand, Some(&epoch), &matcher).admits(&demand.operation())
        })
    };
    assert!(admit(&own).is_ok());
    let mixed = own
        .try_union(&leaf("/proxy-generation/unknown.bzl"))
        .unwrap();
    let mut progress = 0;
    if admit(&mixed).is_ok() {
        progress += 1;
    }
    assert_eq!(progress, 0);
    assert_eq!(admit(&mixed).unwrap_err().family, "namespace");
    drop(mixed);
    drop(own);
    drop(epoch);
    assert_eq!(Arc::strong_count(held), count);
    drop(proxy);
    assert!(proxy_weak.upgrade().is_none());
    drop(features);
    drop(version);
    drop(valid);
    assert!(weak.upgrade().is_none());
    assert!(version_weak.upgrade().is_none());
}

#[test]
fn generated_plan_refusal_projection_preserves_original_order_and_owner() {
    use slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan as Plan;
    for version in [false, true] {
        let path = |s: &str| NormalizedAbsolutePath::new(s).unwrap();
        let bodies = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")]);
        let request_for_plan = |plan: Plan| {
            let mut request = generated_globals_request(&bodies, plan);
            if version {
                request.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new(
                    "bazel_features++version_extension+bazel_features_version",
                )
                .unwrap();
                request.repo_spec.rule_id.bzl_file =
                    CanonicalLabel::parse("@@bazel_features+//private:version_repo.bzl").unwrap();
                request.repo_spec.rule_id.rule_name = "version_repo".into();
            }
            request
        };
        let plan = Plan::build([
            (
                compact_str::CompactString::new("FIRST"),
                Arc::from(&b"abc"[..]),
                false,
            ),
            (
                compact_str::CompactString::new("SECOND"),
                Arc::from(&b"changed"[..]),
                true,
            ),
        ])
        .unwrap();
        let needs = SourcePreparationNeeds::repository(request_for_plan(plan));
        let enabled = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
            .with_fourth(path("/features"))
            .with_bazel_features_import_sources(
                RegistryChoice::PreparedFile,
                CommandChoice::PublicProbe,
            )
            .with_generated_features_file_effects(
                RegistryChoice::PreparedFile,
                CommandChoice::PublicProbe,
            );
        // A permissive phase shape still selects only the two exact typed owners; native uses fixed signatures.
        assert_eq!(
            enabled
                .admit_with_plan_matcher(&needs, &|_, _| true)
                .is_ok(),
            true
        );
        let original = needs.repository_materializations().values().next().unwrap();
        let weak = Arc::downgrade(original);
        let denied = bodies.admit(&needs).unwrap_err();
        assert_eq!(denied.family, "materialization");
        let view = bodies.generated_plan_diagnostic(&needs, &denied).unwrap();
        assert!(std::ptr::eq(view.request, original.as_ref()));
        let RepositoryMaterializationKind::GeneratedFileEffects(plan) = &original.kind else {
            unreachable!()
        };
        assert!(std::ptr::eq(view.plan, plan));
        assert_eq!(Arc::strong_count(original), 1);
        let encoded = view.to_string();
        assert!(encoded.contains("ordered_rows=2 file_count=2 archive_count=0 symlink_count=0 path_bytes=11 content_bytes=10 mapping_len=none mapping_hex=none"));
        assert!(encoded.contains("ordinal=0 kind=file path_len=5 path_hex=4649525354 content_len=3 content_sha256=ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad executable=false"));
        assert!(encoded.contains("ordinal=1 kind=file path_len=6 path_hex=5345434f4e44"));
        assert!(encoded.ends_with("executable=true"));
        let reversed = Plan::build(plan.effects().iter().rev().map(|file| {
            (
                compact_str::CompactString::new(file.path()),
                Arc::from(file.content()),
                file.executable(),
            )
        }))
        .unwrap();
        let reversed = SourcePreparationNeeds::repository(request_for_plan(reversed));
        let reversed = bodies
            .generated_plan_diagnostic(&reversed, &denied)
            .unwrap()
            .to_string();
        assert!(reversed.contains("ordinal=0 kind=file path_len=6 path_hex=5345434f4e44"));
        assert_ne!(encoded, reversed);
        let edited = Plan::build(plan.effects().iter().map(|file| {
            (
                compact_str::CompactString::new(file.path()),
                Arc::from(if file.path() == "FIRST" {
                    &b"abd"[..]
                } else {
                    file.content()
                }),
                file.executable(),
            )
        }))
        .unwrap();
        let edited = SourcePreparationNeeds::repository(request_for_plan(edited));
        let edited = bodies
            .generated_plan_diagnostic(&edited, &denied)
            .unwrap()
            .to_string();
        assert!(
            !edited.contains("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
        drop(needs);
        assert!(weak.upgrade().is_none());
        for index in 0..8 {
            let mut request = request_for_plan(Plan::build([]).unwrap());
            match index {
                0 => request.id.workspace = path("/other"),
                1 => {
                    request.id.canonical_repo =
                        slug_identity_v2::CanonicalRepoName::new("other+").unwrap()
                }
                2 => {
                    request.repo_spec.rule_id.bzl_file = CanonicalLabel::parse(if version {
                        "@@other//private:version_repo.bzl"
                    } else {
                        "@@other//private:globals_repo.bzl"
                    })
                    .unwrap()
                }
                3 => {
                    request.repo_spec.rule_id.bzl_file = CanonicalLabel::parse(if version {
                        "@@bazel_features+//other:version_repo.bzl"
                    } else {
                        "@@bazel_features+//other:globals_repo.bzl"
                    })
                    .unwrap()
                }
                4 => {
                    request.repo_spec.rule_id.bzl_file =
                        CanonicalLabel::parse("@@bazel_features+//private:other.bzl").unwrap()
                }
                5 => request.repo_spec.rule_id.rule_name = "other".into(),
                6 => request.kind = RepositoryMaterializationKind::Immutable,
                7 => {
                    request.kind = RepositoryMaterializationKind::Local {
                        logical_root: path("/features"),
                    }
                }
                _ => unreachable!(),
            }
            assert!(
                bodies
                    .generated_plan_diagnostic(
                        &SourcePreparationNeeds::repository(request),
                        &denied
                    )
                    .is_none()
            );
        }
        // Exact paired identities reject every cross between the two otherwise eligible tuples.
        for component in 0..3 {
            let mut crossed = request_for_plan(Plan::build([]).unwrap());
            match component {
                0 => {
                    crossed.id.canonical_repo =
                        slug_identity_v2::CanonicalRepoName::new(if version {
                            "bazel_features++version_extension+bazel_features_globals"
                        } else {
                            "bazel_features++version_extension+bazel_features_version"
                        })
                        .unwrap()
                }
                1 => {
                    crossed.repo_spec.rule_id.bzl_file = CanonicalLabel::parse(if version {
                        "@@bazel_features+//private:globals_repo.bzl"
                    } else {
                        "@@bazel_features+//private:version_repo.bzl"
                    })
                    .unwrap()
                }
                2 => {
                    crossed.repo_spec.rule_id.rule_name = if version {
                        "globals_repo"
                    } else {
                        "version_repo"
                    }
                    .into()
                }
                _ => unreachable!(),
            }
            assert!(
                bodies
                    .generated_plan_diagnostic(
                        &SourcePreparationNeeds::repository(crossed),
                        &denied
                    )
                    .is_none()
            );
        }
        let mut request = request_for_plan(Plan::build([]).unwrap());
        let mut mapping = slug_identity_v2::RepositoryMapping::new(
            slug_identity_v2::RepositoryMappingId::new("owner-map").unwrap(),
        );
        mapping.insert(
            slug_identity_v2::ApparentRepoName::new("features").unwrap(),
            slug_identity_v2::CanonicalRepoName::new("bazel_features+").unwrap(),
        );
        request.repo_spec.rule_id.bzl_file = slug_identity_v2::ApparentLabel::parse(if version {
            "@features//private:version_repo.bzl"
        } else {
            "@features//private:globals_repo.bzl"
        })
        .unwrap()
        .resolve(&mapping);
        let mapped = SourcePreparationNeeds::repository(request);
        assert!(
            bodies
                .generated_plan_diagnostic(&mapped, &denied)
                .unwrap()
                .to_string()
                .contains("mapping_len=9 mapping_hex=6f776e65722d6d6170")
        );
        let bootstrap = SourcePreparationNeeds::root_module_bootstrap(
            slug_bzlmod_v2::RootModuleBootstrapRequest {
                workspace: path("/other"),
            },
        );
        let mixed = mapped.try_union(&bootstrap).unwrap();
        assert!(
            bodies
                .generated_plan_diagnostic(&mixed, &bodies.admit(&mixed).unwrap_err())
                .is_none()
        );
        let mut progress = 0;
        if bodies.admit(&mapped).is_ok() {
            progress += 1;
        }
        assert_eq!(progress, 0);
        for symlink in [false, true] {
            let mut builder = Plan::builder();
            if symlink {
                builder
                    .push_symlink(
                        compact_str::CompactString::new("LINK"),
                        slug_bzlmod_v2::HostRepositoryLabelPathValue::new(
                            path("/source"),
                            PathObservationNamespace::Host,
                        )
                        .unwrap(),
                        Arc::from(&b"opaque"[..]),
                    )
                    .unwrap();
            } else {
                builder
                    .push_archive(
                        Arc::from([compact_str::CompactString::new(
                            "https://mirror.example/dist/rustc.tar.xz",
                        )]),
                        [0; 32],
                        compact_str::CompactString::new("prefix"),
                    )
                    .unwrap();
            }
            let needs = SourcePreparationNeeds::repository(request_for_plan(builder.finish()));
            let encoded = bodies
                .generated_plan_diagnostic(&needs, &denied)
                .unwrap()
                .to_string();
            assert!(encoded.contains(if symlink {
                "kind=symlink"
            } else {
                "kind=archive"
            }));
            assert!(encoded.ends_with(
                "path_len=none path_hex=none content_len=none content_sha256=none executable=none"
            ));
            assert!(!encoded.contains("opaque") && !encoded.contains("mirror.example"));
        }
    }
}

#[test]
fn generated_plan_refusal_projection_is_bounded() {
    use slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan as Plan;
    for version in [false, true] {
        let path = |s: &str| NormalizedAbsolutePath::new(s).unwrap();
        let bodies = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")]);
        let request_for_plan = |plan: Plan| {
            let mut request = generated_globals_request(&bodies, plan);
            if version {
                request.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new(
                    "bazel_features++version_extension+bazel_features_version",
                )
                .unwrap();
                request.repo_spec.rule_id.bzl_file =
                    CanonicalLabel::parse("@@bazel_features+//private:version_repo.bzl").unwrap();
                request.repo_spec.rule_id.rule_name = "version_repo".into();
            }
            request
        };
        let check = |lengths: &[usize], content_size: usize, captured: bool| {
            let content: Arc<[u8]> = vec![b'x'; content_size].into();
            let plan = Plan::build(lengths.iter().enumerate().map(|(i, len)| {
                (
                    compact_str::CompactString::new(format!("{}{}", "x".repeat(len - 1), i)),
                    content.clone(),
                    i % 2 == 0,
                )
            }))
            .unwrap();
            let needs = SourcePreparationNeeds::repository(request_for_plan(plan));
            let denied = bodies.admit(&needs).unwrap_err();
            let view = bodies.generated_plan_diagnostic(&needs, &denied).unwrap();
            assert_eq!(view.summary().captured, captured);
            let encoded = view.to_string();
            assert!(encoded.len() + 1 <= GENERATED_PLAN_MARKER_BYTES);
            assert_eq!(
                encoded.matches("WP983_GENERATED_EFFECT").count(),
                if captured { lengths.len() } else { 0 }
            );
            if !captured {
                assert!(encoded.contains("mapping_hex=none"));
            }
            if lengths.len() == 9 {
                assert!(encoded.contains("counts_complete=false ordered_rows=9 file_count=none archive_count=none symlink_count=none path_bytes=none content_bytes=none"));
            }
        };
        for n in [0, 1, 8, 9] {
            check(&vec![1; n], 0, n <= 8);
        }
        check(&[512], 0, true);
        check(&[513], 0, false);
        check(&[512; 4], 0, true);
        check(&[512, 512, 512, 512, 1], 0, false);
        check(&[1], 1 << 20, true);
        check(&[1], (1 << 20) + 1, false);
        check(&[1; 4], 1 << 20, true);
        // Four shared1MiB payloads plus one byte cross the exact aggregate boundary.
        let content: Arc<[u8]> = vec![b'x'; 1 << 20].into();
        let plan = Plan::build((0..5).map(|i| {
            (
                compact_str::CompactString::new(format!("f{i}")),
                if i == 4 {
                    Arc::from(&b"x"[..])
                } else {
                    content.clone()
                },
                false,
            )
        }))
        .unwrap();
        let needs = SourcePreparationNeeds::repository(request_for_plan(plan));
        let view = bodies
            .generated_plan_diagnostic(&needs, &bodies.admit(&needs).unwrap_err())
            .unwrap();
        assert_eq!(view.summary().content, Some((4 << 20) + 1));
        assert!(!view.summary().captured);
        assert_eq!(
            view.to_string().matches("WP983_GENERATED_EFFECT").count(),
            0
        );
        drop(needs);
        drop(content);
        for size in [1024, 1025] {
            let mut request = request_for_plan(Plan::build([]).unwrap());
            let mut mapping = slug_identity_v2::RepositoryMapping::new(
                slug_identity_v2::RepositoryMappingId::new("x".repeat(size)).unwrap(),
            );
            mapping.insert(
                slug_identity_v2::ApparentRepoName::new("features").unwrap(),
                slug_identity_v2::CanonicalRepoName::new("bazel_features+").unwrap(),
            );
            request.repo_spec.rule_id.bzl_file =
                slug_identity_v2::ApparentLabel::parse(if version {
                    "@features//private:version_repo.bzl"
                } else {
                    "@features//private:globals_repo.bzl"
                })
                .unwrap()
                .resolve(&mapping);
            let needs = SourcePreparationNeeds::repository(request);
            let view = bodies
                .generated_plan_diagnostic(&needs, &bodies.admit(&needs).unwrap_err())
                .unwrap();
            assert_eq!(view.summary().captured, size == 1024);
            assert_eq!(view.mapping().unwrap().len(), size);
            if size == 1025 {
                assert!(
                    view.to_string()
                        .contains("mapping_len=1025 mapping_hex=none")
                );
            }
        }
        let plan = Plan::build([(
            compact_str::CompactString::new("\né"),
            Arc::from(&b""[..]),
            false,
        )])
        .unwrap();
        let needs = SourcePreparationNeeds::repository(request_for_plan(plan));
        assert!(
            bodies
                .generated_plan_diagnostic(&needs, &bodies.admit(&needs).unwrap_err())
                .unwrap()
                .to_string()
                .contains("path_len=3 path_hex=0ac3a9 content_len=0")
        );
        assert_eq!(RegistryHex(Some(b"")).to_string(), "-");
        assert_eq!(RegistryHex(Some("\né".as_bytes())).to_string(), "0ac3a9");
        let mut output = String::new();
        let mut writer = MaterializationWriter {
            writer: &mut output,
            bytes: GENERATED_PLAN_MARKER_BYTES - 1,
            limit: GENERATED_PLAN_MARKER_BYTES,
        };
        assert!(fmt::Write::write_str(&mut writer, "x").is_err());
        assert!(output.is_empty());
    }
}

#[test]
fn guarded_singleton_preserves_dispatch_policy_and_whole_need() {
    let path = |s: &str| NormalizedAbsolutePath::new(s).unwrap();
    let mut bodies = Arc::new(
        Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
            .with_fourth(path("/features"))
            .with_fifth(path("/skylib"))
            .with_sixth(path("/cc"))
            .with_bazel_features_import_sources(
                RegistryChoice::PreparedFile,
                CommandChoice::PublicProbe,
            )
            .with_generated_features_file_effects(
                RegistryChoice::PreparedFile,
                CommandChoice::PublicProbe,
            )
            .with_rules_cc_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
            .with_bazel_skylib_import_sources(
                RegistryChoice::PreparedFile,
                CommandChoice::PublicProbe,
            )
            .with_visible_lockfile_metadata(RegistryChoice::PreparedFile)
            .with_platforms_host_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
            .with_platforms_host_import_sources(
                RegistryChoice::PreparedFile,
                CommandChoice::PublicProbe,
            )
            .with_repo_marker_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
            .with_ignore_file_bytes(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
            .with_rust_package_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
            .with_nightly_package_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
            .with_nightly_build_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
            .with_nightly_channel_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
            .with_probe_package_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
            .with_rules_rust_defs_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
            .with_rules_rust_defs_import_sources(
                RegistryChoice::PreparedFile,
                CommandChoice::PublicProbe,
            ),
    );
    let root = BuildCommandRootKey::new_with_overlay(
        path("/fixture"),
        &[TargetPattern::parse("//pkg:probe").unwrap()],
        build_test_configuration("probe-dispatch"),
        nightly_overlay(),
    )
    .unwrap();
    assert!(BuildCommandRootObservationKey::new(root.clone()).is_none());
    let inner = SingletonRootSingleBuildCommandKey::new(root).unwrap();
    let registry = RegistryUrls::new(["file:///task/registry"]);
    let mut request = NativeDemandRequestInputBundle::normalized_initial();
    request.command_policy = BzlmodCommandPolicyKey::from_flags_with_module_overrides(
        None,
        false,
        Path::new("/fixture"),
        [
            "rules_rust=/rules",
            "platforms=/platforms",
            "bazel_features=/features",
            "bazel_skylib=/skylib",
            "rules_cc=/cc",
        ],
    )
    .unwrap();
    request.lockfile_mode = LockfileMode::Off;
    request.registry_urls = registry.dupe();
    assert!(projection_matches(
        &inner,
        &request,
        &bodies,
        &registry,
        &nightly_overlay()
    ));
    Arc::get_mut(&mut bodies)
        .unwrap()
        .platforms_host_import_sources = false;
    assert!(!projection_matches(
        &inner,
        &request,
        &bodies,
        &registry,
        &nightly_overlay()
    ));
    Arc::get_mut(&mut bodies)
        .unwrap()
        .platforms_host_import_sources = true;
    Arc::get_mut(&mut bodies).unwrap().platforms_host_metadata = false;
    assert!(!projection_matches(
        &inner,
        &request,
        &bodies,
        &registry,
        &nightly_overlay()
    ));
    Arc::get_mut(&mut bodies).unwrap().platforms_host_metadata = true;
    Arc::get_mut(&mut bodies)
        .unwrap()
        .generated_features_file_effects = false;
    assert!(!projection_matches(
        &inner,
        &request,
        &bodies,
        &registry,
        &nightly_overlay()
    ));
    Arc::get_mut(&mut bodies)
        .unwrap()
        .generated_features_file_effects = true;
    let closed = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
        .with_fourth(path("/features"))
        .with_fifth(path("/skylib"))
        .with_sixth(path("/cc"))
        .with_bazel_features_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        )
        .with_generated_features_file_effects(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        )
        .with_rules_cc_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_bazel_skylib_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_visible_lockfile_metadata(RegistryChoice::PreparedFile)
        .with_platforms_host_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_platforms_host_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        );
    assert!(!projection_matches(
        &inner,
        &request,
        &closed,
        &registry,
        &nightly_overlay()
    ));
    let no_ignore = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
        .with_fourth(path("/features"))
        .with_fifth(path("/skylib"))
        .with_sixth(path("/cc"))
        .with_bazel_features_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        )
        .with_generated_features_file_effects(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        )
        .with_rules_cc_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_bazel_skylib_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_visible_lockfile_metadata(RegistryChoice::PreparedFile)
        .with_platforms_host_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_platforms_host_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        )
        .with_repo_marker_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_rust_package_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_nightly_package_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_nightly_build_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_nightly_channel_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_probe_package_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_rules_rust_defs_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_rules_rust_defs_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        );
    assert!(!projection_matches(
        &inner,
        &request,
        &no_ignore,
        &registry,
        &nightly_overlay()
    ));
    let no_rust_metadata = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
        .with_fourth(path("/features"))
        .with_fifth(path("/skylib"))
        .with_sixth(path("/cc"))
        .with_bazel_features_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        )
        .with_generated_features_file_effects(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        )
        .with_rules_cc_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_bazel_skylib_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_visible_lockfile_metadata(RegistryChoice::PreparedFile)
        .with_platforms_host_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_platforms_host_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        )
        .with_repo_marker_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_ignore_file_bytes(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_nightly_package_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_nightly_build_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_nightly_channel_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_probe_package_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_rules_rust_defs_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_rules_rust_defs_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        );
    assert!(!projection_matches(
        &inner,
        &request,
        &no_rust_metadata,
        &registry,
        &nightly_overlay()
    ));
    let no_nightly_metadata = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
        .with_fourth(path("/features"))
        .with_fifth(path("/skylib"))
        .with_sixth(path("/cc"))
        .with_bazel_features_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        )
        .with_generated_features_file_effects(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        )
        .with_rules_cc_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_bazel_skylib_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_visible_lockfile_metadata(RegistryChoice::PreparedFile)
        .with_platforms_host_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_platforms_host_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        )
        .with_repo_marker_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_ignore_file_bytes(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_rust_package_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_nightly_build_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_nightly_channel_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_probe_package_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_rules_rust_defs_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_rules_rust_defs_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        );
    assert!(!projection_matches(
        &inner,
        &request,
        &no_nightly_metadata,
        &registry,
        &nightly_overlay()
    ));
    let no_build_source = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
        .with_fourth(path("/features"))
        .with_fifth(path("/skylib"))
        .with_sixth(path("/cc"))
        .with_bazel_features_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        )
        .with_generated_features_file_effects(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        )
        .with_rules_cc_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_bazel_skylib_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_visible_lockfile_metadata(RegistryChoice::PreparedFile)
        .with_platforms_host_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_platforms_host_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        )
        .with_repo_marker_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_ignore_file_bytes(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_rust_package_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_nightly_package_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_nightly_channel_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_probe_package_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_rules_rust_defs_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_rules_rust_defs_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        );
    assert!(!projection_matches(
        &inner,
        &request,
        &no_build_source,
        &registry,
        &nightly_overlay()
    ));
    let no_channel_source = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
        .with_fourth(path("/features"))
        .with_fifth(path("/skylib"))
        .with_sixth(path("/cc"))
        .with_bazel_features_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        )
        .with_generated_features_file_effects(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        )
        .with_rules_cc_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_bazel_skylib_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_visible_lockfile_metadata(RegistryChoice::PreparedFile)
        .with_platforms_host_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_platforms_host_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        )
        .with_repo_marker_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_ignore_file_bytes(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_rust_package_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_nightly_package_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_nightly_build_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_probe_package_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_rules_rust_defs_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_rules_rust_defs_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        );
    assert!(!projection_matches(
        &inner,
        &request,
        &no_channel_source,
        &registry,
        &nightly_overlay()
    ));
    let no_package_source = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
        .with_fourth(path("/features"))
        .with_fifth(path("/skylib"))
        .with_sixth(path("/cc"))
        .with_bazel_features_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        )
        .with_generated_features_file_effects(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        )
        .with_rules_cc_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_bazel_skylib_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_visible_lockfile_metadata(RegistryChoice::PreparedFile)
        .with_platforms_host_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_platforms_host_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        )
        .with_repo_marker_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_ignore_file_bytes(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_rust_package_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_nightly_package_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_nightly_build_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_nightly_channel_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_rules_rust_defs_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_rules_rust_defs_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        );
    assert!(!projection_matches(
        &inner,
        &request,
        &no_package_source,
        &registry,
        &nightly_overlay()
    ));
    Arc::get_mut(&mut bodies).unwrap().rules_rust_defs_source = false;
    assert!(!projection_matches(
        &inner,
        &request,
        &bodies,
        &registry,
        &nightly_overlay()
    ));
    Arc::get_mut(&mut bodies).unwrap().rules_rust_defs_source = true;
    Arc::get_mut(&mut bodies)
        .unwrap()
        .rules_rust_defs_import_sources = false;
    assert!(!projection_matches(
        &inner,
        &request,
        &bodies,
        &registry,
        &nightly_overlay()
    ));
    Arc::get_mut(&mut bodies)
        .unwrap()
        .rules_rust_defs_import_sources = true;
    Arc::get_mut(&mut bodies)
        .unwrap()
        .bazel_skylib_import_sources = false;
    assert!(!projection_matches(
        &inner,
        &request,
        &bodies,
        &registry,
        &nightly_overlay()
    ));
    Arc::get_mut(&mut bodies)
        .unwrap()
        .bazel_skylib_import_sources = true;
    let selected = Arc::get_mut(&mut bodies).unwrap().additional[1].take();
    let selected_policy = std::mem::replace(
        &mut request.command_policy,
        BzlmodCommandPolicyKey::from_flags_with_module_overrides(
            None,
            false,
            Path::new("/fixture"),
            [
                "rules_rust=/rules",
                "platforms=/platforms",
                "bazel_features=/features",
                "rules_cc=/cc",
            ],
        )
        .unwrap(),
    );
    assert!(!projection_matches(
        &inner,
        &request,
        &bodies,
        &registry,
        &nightly_overlay()
    ));
    Arc::get_mut(&mut bodies).unwrap().additional[1] = selected;
    request.command_policy = selected_policy;
    assert!(projection_matches(
        &inner,
        &request,
        &bodies,
        &registry,
        &nightly_overlay()
    ));
    Arc::get_mut(&mut bodies).unwrap().rules_cc_import_sources = false;
    assert!(!projection_matches(
        &inner,
        &request,
        &bodies,
        &registry,
        &nightly_overlay()
    ));
    Arc::get_mut(&mut bodies).unwrap().rules_cc_import_sources = true;
    let selected = Arc::get_mut(&mut bodies).unwrap().additional[2].take();
    let selected_policy = std::mem::replace(
        &mut request.command_policy,
        BzlmodCommandPolicyKey::from_flags_with_module_overrides(
            None,
            false,
            Path::new("/fixture"),
            [
                "rules_rust=/rules",
                "platforms=/platforms",
                "bazel_features=/features",
                "bazel_skylib=/skylib",
            ],
        )
        .unwrap(),
    );
    assert!(!projection_matches(
        &inner,
        &request,
        &bodies,
        &registry,
        &nightly_overlay()
    ));
    Arc::get_mut(&mut bodies).unwrap().additional[2] = selected;
    request.command_policy = selected_policy;
    assert!(projection_matches(
        &inner,
        &request,
        &bodies,
        &registry,
        &nightly_overlay()
    ));
    Arc::get_mut(&mut bodies)
        .unwrap()
        .bazel_features_import_sources = false;
    assert!(!projection_matches(
        &inner,
        &request,
        &bodies,
        &registry,
        &nightly_overlay()
    ));
    Arc::get_mut(&mut bodies)
        .unwrap()
        .bazel_features_import_sources = true;
    let selected = Arc::get_mut(&mut bodies).unwrap().additional[0].take();
    let selected_policy = std::mem::replace(
        &mut request.command_policy,
        BzlmodCommandPolicyKey::from_flags_with_module_overrides(
            None,
            false,
            Path::new("/fixture"),
            [
                "rules_rust=/rules",
                "platforms=/platforms",
                "bazel_skylib=/skylib",
                "rules_cc=/cc",
            ],
        )
        .unwrap(),
    );
    assert!(!projection_matches(
        &inner,
        &request,
        &bodies,
        &registry,
        &nightly_overlay()
    ));
    Arc::get_mut(&mut bodies).unwrap().additional[0] = selected;
    request.command_policy = selected_policy;
    assert!(projection_matches(
        &inner,
        &request,
        &bodies,
        &registry,
        &nightly_overlay()
    ));
    request.lockfile_mode = LockfileMode::Update;
    assert!(!projection_matches(
        &inner,
        &request,
        &bodies,
        &registry,
        &nightly_overlay()
    ));
    request.lockfile_mode = LockfileMode::Off;
    request.registry_urls = RegistryUrls::new(["https://bcr.bazel.build"]);
    assert!(!projection_matches(
        &inner,
        &request,
        &bodies,
        &registry,
        &nightly_overlay()
    ));
    let milestones = Arc::new(Mutex::new(Milestones::default()));
    let guarded = GuardedSingleton {
        inner: inner.clone(),
        bodies,
        milestones: milestones.clone(),
    };
    assert!(guarded.initializes_request_revision());
    assert_eq!(
        guarded.requires_workspace_status_inputs(),
        inner.requires_workspace_status_inputs()
    );
    assert_eq!(
        std::mem::discriminant(&guarded.observed_selection_association()),
        std::mem::discriminant(&inner.observed_selection_association())
    );
    assert!(!guarded.allows_empty_terminal());
    // Only a synthetic metadata error is supplied for delegation; no build producer is claimed.
    let terminal = SingletonRootSingleBuildCommandTerminal {
        result: Arc::new(Err(BuildCommandError::infrastructure("unit delegation"))),
        observations: None,
    };
    assert!(guarded.source_certificate(&terminal).is_none());
    assert!(guarded.observations(&terminal).is_none());
    assert_eq!(
        std::mem::discriminant(&guarded.terminal_observed_selection_association(&terminal)),
        std::mem::discriminant(&inner.terminal_observed_selection_association(&terminal))
    );
    assert_eq!(
        std::mem::discriminant(&guarded.terminal_demand_association(&terminal)),
        std::mem::discriminant(&inner.terminal_demand_association(&terminal))
    );
    assert_eq!(
        std::mem::discriminant(&guarded.event_reconciliation_policy(&terminal)),
        std::mem::discriminant(&inner.event_reconciliation_policy(&terminal))
    );
    assert_eq!(
        guarded.allows_unavailable_terminal_roots(&terminal),
        inner.allows_unavailable_terminal_roots(&terminal)
    );
    assert_eq!(
        guarded.publication_completed(&terminal),
        inner.publication_completed(&terminal)
    );
    assert!(Arc::ptr_eq(&terminal.result, &terminal.clone().result));
    let demand =
        |p: &str, operation| {
            SourcePreparationNeeds::path(NeedPathObservations::singleton(
                PathObservationDemand::new(PathObservationNamespace::Host, path(p), operation),
            ))
        };
    let allowed = demand(
        "/fixture/MODULE.bazel.lock",
        PathObservationOperation::Lstat,
    );
    assert!(guarded.check_need(&allowed).is_ok());
    let mut progress = 0;
    for (excluded, operation) in [
        (
            demand("/fixture/pkg/main.rs", PathObservationOperation::FileBytes),
            "file_bytes",
        ),
        (
            SourcePreparationNeeds::root_module_bootstrap(
                slug_bzlmod_v2::RootModuleBootstrapRequest {
                    workspace: path("/foreign"),
                },
            ),
            "none",
        ),
    ] {
        let mixed = allowed.try_union(&excluded).unwrap();
        if guarded.check_need(&mixed).is_ok() {
            progress += 1;
        }
        assert!(guarded.check_need(&mixed).is_err());
        assert_eq!(
            milestones
                .lock()
                .unwrap()
                .denied
                .as_ref()
                .unwrap()
                .operation,
            operation
        );
    }
    assert_eq!(progress, 0);
    assert_eq!(
        milestones
            .lock()
            .unwrap()
            .denied
            .as_ref()
            .unwrap()
            .operation,
        "none"
    );
}
#[test]
fn repo_marker_metadata_need_is_opt_in_root_lstat_only() {
    assert_exact_root_file_role(
        "REPO.bazel",
        &[PathObservationOperation::Lstat],
        Bodies::with_repo_marker_metadata,
        |b| b.repo_marker_metadata,
        &[
            "/other/REPO.bazel",
            "/fixture/sub/REPO.bazel",
            "/fixture/REPO.bazel/child",
            "/fixture/REPO.bazel.other",
            "/fixture/repo.bazel",
            "/fixture/REPO.BAZEL",
        ],
        "/rules/REPO.bazel",
        None,
    );
}
#[test]
fn ignore_file_bytes_need_is_opt_in_exact_root_only() {
    assert_exact_root_file_role(
        ".bazelignore",
        &[PathObservationOperation::FileBytes],
        Bodies::with_ignore_file_bytes,
        |b| b.ignore_file_bytes,
        &[
            "/other/.bazelignore",
            "/fixture/sub/.bazelignore",
            "/fixture/.bazelignore/child",
            "/fixture/.bazelignore.other",
            "/fixture/.BAZELIGNORE",
            "/fixture/bazelignore",
        ],
        "/rules/.bazelignore",
        None,
    );
}
#[test]
fn rust_package_metadata_need_is_opt_in_exact_rules_root_only() {
    assert_exact_root_file_role(
        "rust",
        &[PathObservationOperation::Lstat],
        Bodies::with_rust_package_metadata,
        |b| b.rust_package_metadata,
        &[
            "/other/rust",
            "/rules/sub/rust",
            "/rules/rust/child",
            "/rules/rust.extra",
            "/rules/RUST",
            "/rules/Rust",
            "/rules-other/rust",
        ],
        "/rules/rust",
        Some("/rules"),
    );
}

#[test]
fn platforms_host_metadata_need_is_opt_in_exact_slot_two_lstat_only() {
    for (name, selected, spellings) in [
        (
            "host",
            "/platforms/host",
            &[
                "/platforms/HOST",
                "/platforms/host.other",
                "/platforms/host/child",
            ][..],
        ),
        (
            "host/BUILD.bazel",
            "/platforms/host/BUILD.bazel",
            &[
                "/platforms/host/build.bazel",
                "/platforms/host/BUILD.bazel.other",
                "/platforms/host/BUILD.bazel/child",
            ][..],
        ),
        (
            "host/BUILD",
            "/platforms/host/BUILD",
            &[
                "/platforms/host/build",
                "/platforms/host/BUILD.other",
                "/platforms/host/BUILD/child",
            ][..],
        ),
    ] {
        assert_exact_root_file_role(
            name,
            &[PathObservationOperation::Lstat],
            Bodies::with_platforms_host_metadata,
            |b| b.platforms_host_metadata,
            spellings,
            selected,
            Some("/platforms"),
        );
    }
    let path = |s| NormalizedAbsolutePath::new(s).unwrap();
    let bodies = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
        .with_platforms_host_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe);
    let need =
        |p, operation| {
            SourcePreparationNeeds::path(NeedPathObservations::singleton(
                PathObservationDemand::new(PathObservationNamespace::Host, path(p), operation),
            ))
        };
    let allowed = need("/platforms/host", PathObservationOperation::Lstat)
        .try_union(&need(
            "/platforms/host/BUILD.bazel",
            PathObservationOperation::Lstat,
        ))
        .unwrap()
        .try_union(&need(
            "/platforms/host/BUILD",
            PathObservationOperation::Lstat,
        ))
        .unwrap();
    assert_eq!(allowed.path_observations().unwrap().demands().len(), 3);
    assert!(bodies.admit(&allowed).is_ok());
    let excluded = need("/platforms/host", PathObservationOperation::FileBytes);
    for mixed in [
        allowed.try_union(&excluded).unwrap(),
        excluded.try_union(&allowed).unwrap(),
    ] {
        let first = bodies
            .first_denied_path_with_namespace_matcher(&mixed, &|_| false)
            .unwrap()
            .0;
        let expected = mixed
            .path_observations()
            .unwrap()
            .demands()
            .iter()
            .find(|d| d.operation() == PathObservationOperation::FileBytes)
            .unwrap();
        assert!(std::ptr::eq(first, expected));
        assert_eq!(usize::from(bodies.admit(&mixed).is_ok()), 0);
    }
    let local = SourcePreparationNeeds::repository(
        (*local_native_request(&bodies.roots[0], "unknown+", "/platforms")).clone(),
    );
    let original = local.repository_materializations().values().next().unwrap();
    let weak = Arc::downgrade(original);
    let mixed = allowed.try_union(&local).unwrap();
    assert!(Arc::ptr_eq(
        original,
        mixed.repository_materializations().values().next().unwrap()
    ));
    let (first, reason) = bodies
        .first_denied_materialization_with_plan_matcher(&mixed, &generated_features_plan_matches)
        .unwrap();
    assert!(std::ptr::eq(first, original.as_ref()));
    assert_eq!(reason, "binding");
    assert_eq!(bodies.admit(&mixed).unwrap_err().family, "materialization");
    assert_eq!(usize::from(bodies.admit(&mixed).is_ok()), 0);
    drop(mixed);
    drop(local);
    assert!(weak.upgrade().is_none());
}

#[test]
fn platforms_host_import_sources_need_is_opt_in_exact_captured_rows_only() {
    for (name, selected, spellings) in [
        (
            "host/constraints.bzl",
            "/platforms/host/constraints.bzl",
            &[
                "/platforms/host/CONSTRAINTS.bzl",
                "/platforms/host/constraints.bzl.other",
                "/platforms/host/constraints.bzl/child",
            ][..],
        ),
        (
            "host/BUILD",
            "/platforms/host/BUILD",
            &[
                "/platforms/host/build",
                "/platforms/host/BUILD.other",
                "/platforms/host/BUILD/child",
                "/platforms/host/BUILD.bazel",
            ][..],
        ),
        (
            "host/extension.bzl",
            "/platforms/host/extension.bzl",
            &[
                "/platforms/host/EXTENSION.bzl",
                "/platforms/host/extension.bzl.other",
                "/platforms/host/extension.bzl/child",
            ][..],
        ),
    ] {
        assert_exact_root_file_role(
            name,
            &[
                PathObservationOperation::Lstat,
                PathObservationOperation::FileBytes,
            ],
            Bodies::with_platforms_host_import_sources,
            |b| b.platforms_host_import_sources,
            spellings,
            selected,
            Some("/platforms"),
        );
    }
    let path = |s| NormalizedAbsolutePath::new(s).unwrap();
    let bodies = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
        .with_platforms_host_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        );
    let need =
        |p, operation| {
            SourcePreparationNeeds::path(NeedPathObservations::singleton(
                PathObservationDemand::new(PathObservationNamespace::Host, path(p), operation),
            ))
        };
    let allowed = need(
        "/platforms/host/constraints.bzl",
        PathObservationOperation::Lstat,
    )
    .try_union(&need(
        "/platforms/host/constraints.bzl",
        PathObservationOperation::FileBytes,
    ))
    .unwrap()
    .try_union(&need(
        "/platforms/host/BUILD",
        PathObservationOperation::Lstat,
    ))
    .unwrap()
    .try_union(&need(
        "/platforms/host/BUILD",
        PathObservationOperation::FileBytes,
    ))
    .unwrap()
    .try_union(&need(
        "/platforms/host/extension.bzl",
        PathObservationOperation::Lstat,
    ))
    .unwrap()
    .try_union(&need(
        "/platforms/host/extension.bzl",
        PathObservationOperation::FileBytes,
    ))
    .unwrap();
    assert_eq!(allowed.path_observations().unwrap().demands().len(), 6);
    assert!(bodies.admit(&allowed).is_ok());
    for excluded_path in [
        "/platforms/host/constraints.bzl",
        "/platforms/host/extension.bzl",
    ] {
        let excluded = need(excluded_path, PathObservationOperation::FileDigest);
        for mixed in [
            allowed.try_union(&excluded).unwrap(),
            excluded.try_union(&allowed).unwrap(),
        ] {
            let first = bodies
                .first_denied_path_with_namespace_matcher(&mixed, &|_| false)
                .unwrap()
                .0;
            let expected = mixed
                .path_observations()
                .unwrap()
                .demands()
                .iter()
                .find(|d| d.operation() == PathObservationOperation::FileDigest)
                .unwrap();
            assert!(std::ptr::eq(first, expected));
            assert_eq!(usize::from(bodies.admit(&mixed).is_ok()), 0);
        }
    }
    let local = SourcePreparationNeeds::repository(
        (*local_native_request(&bodies.roots[0], "unknown+", "/platforms")).clone(),
    );
    let original = local.repository_materializations().values().next().unwrap();
    let weak = Arc::downgrade(original);
    let mixed = allowed.try_union(&local).unwrap();
    assert!(Arc::ptr_eq(
        original,
        mixed.repository_materializations().values().next().unwrap()
    ));
    let (first, reason) = bodies.first_denied_materialization(&mixed).unwrap();
    assert!(std::ptr::eq(first, original.as_ref()));
    assert_eq!(reason, "binding");
    assert_eq!(bodies.admit(&mixed).unwrap_err().family, "materialization");
    assert_eq!(usize::from(bodies.admit(&mixed).is_ok()), 0);
    drop(mixed);
    drop(local);
    assert!(weak.upgrade().is_none());
    let metadata_and_sources = bodies
        .with_platforms_host_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe);
    assert!(
        metadata_and_sources
            .admit(&need(
                "/platforms/host/BUILD.bazel",
                PathObservationOperation::Lstat
            ))
            .is_ok()
    );
    assert!(
        metadata_and_sources
            .admit(&need(
                "/platforms/host/BUILD.bazel",
                PathObservationOperation::FileBytes
            ))
            .is_err()
    );
}

#[test]
fn nightly_package_metadata_need_is_opt_in_exact_chain_only() {
    for (relative, selected, spellings) in [
        (
            "rust/toolchain",
            "/rules/rust/toolchain",
            &[
                "/other/rust/toolchain",
                "/rules/rust/other/toolchain",
                "/rules/rust/toolchain/child",
                "/rules/rust/toolchain.extra",
                "/rules/rust/TOOLCHAIN",
                "/rules/RUST/toolchain",
                "/rules-other/rust/toolchain",
            ][..],
        ),
        (
            "rust/toolchain/channel",
            "/rules/rust/toolchain/channel",
            &[
                "/other/rust/toolchain/channel",
                "/rules/rust/other/channel",
                "/rules/rust/toolchain/channel/child",
                "/rules/rust/toolchain/channel.extra",
                "/rules/rust/toolchain/CHANNEL",
                "/rules/rust/TOOLCHAIN/channel",
                "/rules-other/rust/toolchain/channel",
            ][..],
        ),
    ] {
        assert_exact_root_file_role(
            relative,
            &[PathObservationOperation::Lstat],
            Bodies::with_nightly_package_metadata,
            |b| b.nightly_package_metadata,
            spellings,
            selected,
            Some("/rules"),
        );
    }
    let path = |p: &str| NormalizedAbsolutePath::new(p).unwrap();
    let bodies = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
        .with_nightly_package_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe);
    let need =
        |p: &str, operation| {
            SourcePreparationNeeds::path(NeedPathObservations::singleton(
                PathObservationDemand::new(PathObservationNamespace::Host, path(p), operation),
            ))
        };
    let both = need("/rules/rust/toolchain", PathObservationOperation::Lstat)
        .try_union(&need(
            "/rules/rust/toolchain/channel",
            PathObservationOperation::Lstat,
        ))
        .unwrap();
    assert!(bodies.admit(&both).is_ok());
    for (excluded, operation) in [
        (
            "/rules/rust/toolchain/channel/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/rules/rust/toolchain/channel",
            PathObservationOperation::FileBytes,
        ),
    ] {
        let mixed = both.try_union(&need(excluded, operation.dupe())).unwrap();
        let mut progress = 0;
        if bodies.admit(&mixed).is_ok() {
            progress += 1;
        }
        assert_eq!(progress, 0);
        let denied = bodies.admit(&mixed).unwrap_err();
        assert_eq!(denied.family, "path");
        assert_eq!(denied.operation, path_operation_code(&operation));
        assert_eq!(denied.path.unwrap().as_path(), Path::new(excluded));
    }
}
#[test]
fn nightly_build_source_need_is_opt_in_exact_package_only() {
    for (relative, selected, spellings) in [
        (
            "rust/toolchain/channel/BUILD.bazel",
            "/rules/rust/toolchain/channel/BUILD.bazel",
            &[
                "/other/rust/toolchain/channel/BUILD.bazel",
                "/rules/rust/toolchain/other/BUILD.bazel",
                "/rules/rust/toolchain/channel/BUILD.bazel/child",
                "/rules/rust/toolchain/channel/BUILD.bazel.extra",
                "/rules/rust/toolchain/channel/build.bazel",
                "/rules/rust/TOOLCHAIN/channel/BUILD.bazel",
                "/rules-other/rust/toolchain/channel/BUILD.bazel",
            ][..],
        ),
        (
            "rust/toolchain/channel/BUILD",
            "/rules/rust/toolchain/channel/BUILD",
            &[
                "/other/rust/toolchain/channel/BUILD",
                "/rules/rust/toolchain/other/BUILD",
                "/rules/rust/toolchain/channel/BUILD/child",
                "/rules/rust/toolchain/channel/BUILD.extra",
                "/rules/rust/toolchain/channel/build",
                "/rules/rust/TOOLCHAIN/channel/BUILD",
                "/rules-other/rust/toolchain/channel/BUILD",
            ][..],
        ),
    ] {
        assert_exact_root_file_role(
            relative,
            &[
                PathObservationOperation::Lstat,
                PathObservationOperation::FileBytes,
            ],
            Bodies::with_nightly_build_source,
            |b| b.nightly_build_source,
            spellings,
            selected,
            Some("/rules"),
        );
    }
    let path = |p: &str| NormalizedAbsolutePath::new(p).unwrap();
    let bodies = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
        .with_nightly_build_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe);
    let need =
        |p: &str, operation| {
            SourcePreparationNeeds::path(NeedPathObservations::singleton(
                PathObservationDemand::new(PathObservationNamespace::Host, path(p), operation),
            ))
        };
    let mut all = need(
        "/rules/rust/toolchain/channel/BUILD.bazel",
        PathObservationOperation::Lstat,
    );
    for (marker, operation) in [
        (
            "/rules/rust/toolchain/channel/BUILD.bazel",
            PathObservationOperation::FileBytes,
        ),
        (
            "/rules/rust/toolchain/channel/BUILD",
            PathObservationOperation::Lstat,
        ),
        (
            "/rules/rust/toolchain/channel/BUILD",
            PathObservationOperation::FileBytes,
        ),
    ] {
        all = all.try_union(&need(marker, operation)).unwrap();
    }
    assert!(bodies.admit(&all).is_ok());
    for (excluded, operation) in [
        (
            "/rules/rust/toolchain/channel/BUILD.bazel/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/rules/rust/toolchain/channel/BUILD",
            PathObservationOperation::FileDigest,
        ),
    ] {
        let mixed = all.try_union(&need(excluded, operation.dupe())).unwrap();
        let mut progress = 0;
        if bodies.admit(&mixed).is_ok() {
            progress += 1;
        }
        assert_eq!(progress, 0);
        let denied = bodies.admit(&mixed).unwrap_err();
        assert_eq!(denied.family, "path");
        assert_eq!(denied.operation, path_operation_code(&operation));
        assert_eq!(denied.path.unwrap().as_path(), Path::new(excluded));
    }
}
#[test]
fn nightly_channel_source_need_is_opt_in_exact_import_only() {
    assert_exact_root_file_role(
        "rust/toolchain/channel/channel.bzl",
        &[
            PathObservationOperation::Lstat,
            PathObservationOperation::FileBytes,
        ],
        Bodies::with_nightly_channel_source,
        |b| b.nightly_channel_source,
        &[
            "/other/rust/toolchain/channel/channel.bzl",
            "/rules/rust/toolchain/other/channel.bzl",
            "/rules/rust/toolchain/channel/channel.bzl/child",
            "/rules/rust/toolchain/channel/channel.bzl.extra",
            "/rules/rust/toolchain/channel/CHANNEL.bzl",
            "/rules/rust/TOOLCHAIN/channel/channel.bzl",
            "/rules-other/rust/toolchain/channel/channel.bzl",
        ],
        "/rules/rust/toolchain/channel/channel.bzl",
        Some("/rules"),
    );
    let path = |p: &str| NormalizedAbsolutePath::new(p).unwrap();
    let bodies = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
        .with_nightly_channel_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe);
    let need =
        |p: &str, operation| {
            SourcePreparationNeeds::path(NeedPathObservations::singleton(
                PathObservationDemand::new(PathObservationNamespace::Host, path(p), operation),
            ))
        };
    let both = need(
        "/rules/rust/toolchain/channel/channel.bzl",
        PathObservationOperation::Lstat,
    )
    .try_union(&need(
        "/rules/rust/toolchain/channel/channel.bzl",
        PathObservationOperation::FileBytes,
    ))
    .unwrap();
    assert!(bodies.admit(&both).is_ok());
    for (excluded, operation) in [
        (
            "/rules/rust/toolchain/channel/channel.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/rules/rust/toolchain/channel/channel.bzl",
            PathObservationOperation::FileDigest,
        ),
    ] {
        let mixed = both.try_union(&need(excluded, operation.dupe())).unwrap();
        let mut progress = 0;
        if bodies.admit(&mixed).is_ok() {
            progress += 1;
        }
        assert_eq!(progress, 0);
        let denied = bodies.admit(&mixed).unwrap_err();
        assert_eq!(denied.family, "path");
        assert_eq!(denied.operation, path_operation_code(&operation));
        assert_eq!(denied.path.unwrap().as_path(), Path::new(excluded));
    }
}
#[test]
fn rules_rust_defs_source_need_is_opt_in_exact_import_only() {
    assert_exact_root_file_role(
        "rust/defs.bzl",
        &[
            PathObservationOperation::Lstat,
            PathObservationOperation::FileBytes,
        ],
        Bodies::with_rules_rust_defs_source,
        |b| b.rules_rust_defs_source,
        &[
            "/other/rust/defs.bzl",
            "/rules/rust/private/defs.bzl",
            "/rules/rust/defs.bzl/child",
            "/rules/rust/defs.bzl.extra",
            "/rules/rust/DEFS.bzl",
            "/rules/RUST/defs.bzl",
            "/rules-other/rust/defs.bzl",
        ],
        "/rules/rust/defs.bzl",
        Some("/rules"),
    );
    let path = |p: &str| NormalizedAbsolutePath::new(p).unwrap();
    let bodies = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
        .with_rules_rust_defs_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe);
    let need =
        |p: &str, operation| {
            SourcePreparationNeeds::path(NeedPathObservations::singleton(
                PathObservationDemand::new(PathObservationNamespace::Host, path(p), operation),
            ))
        };
    let both = need("/rules/rust/defs.bzl", PathObservationOperation::Lstat)
        .try_union(&need(
            "/rules/rust/defs.bzl",
            PathObservationOperation::FileBytes,
        ))
        .unwrap();
    assert!(bodies.admit(&both).is_ok());
    for (excluded, operation) in [
        (
            "/rules/rust/defs.bzl/child",
            PathObservationOperation::Lstat,
        ),
        ("/rules/rust/defs.bzl", PathObservationOperation::FileDigest),
    ] {
        let mixed = both.try_union(&need(excluded, operation.dupe())).unwrap();
        let mut progress = 0;
        if bodies.admit(&mixed).is_ok() {
            progress += 1;
        }
        assert_eq!(progress, 0);
        let denied = bodies.admit(&mixed).unwrap_err();
        assert_eq!(denied.family, "path");
        assert_eq!(denied.operation, path_operation_code(&operation));
        assert_eq!(denied.path.unwrap().as_path(), Path::new(excluded));
    }
}
#[test]
fn rules_rust_defs_import_sources_need_is_opt_in_exact_table_only() {
    let operations = [
        PathObservationOperation::Lstat,
        PathObservationOperation::FileBytes,
    ];
    for relative in RULES_RUST_DEFS_IMPORTS {
        let selected = Path::new("/rules").join(relative);
        assert_exact_root_file_role(
            relative,
            &operations,
            Bodies::with_rules_rust_defs_import_sources,
            |b| b.rules_rust_defs_import_sources,
            &[],
            selected.to_str().unwrap(),
            Some("/rules"),
        );
    }
    for (relative, selected, spellings) in [
        (
            "rust/private",
            "/rules/rust/private",
            [
                "/rules/rust/private/child",
                "/rules/RUST/private",
                "/rules/rust/private-extra",
                "/rules-other/rust/private",
            ],
        ),
        (
            "rust/platform",
            "/rules/rust/platform",
            [
                "/rules/rust/platform/child",
                "/rules/RUST/platform",
                "/rules/rust/platform-extra",
                "/rules-other/rust/platform",
            ],
        ),
        (
            "rust/settings",
            "/rules/rust/settings",
            [
                "/rules/rust/settings/child",
                "/rules/RUST/settings",
                "/rules/rust/settings-extra",
                "/rules-other/rust/settings",
            ],
        ),
    ] {
        assert_exact_root_file_role(
            relative,
            &[PathObservationOperation::Lstat],
            Bodies::with_rules_rust_defs_import_sources,
            |b| b.rules_rust_defs_import_sources,
            &spellings,
            selected,
            Some("/rules"),
        );
    }
    let path = |p: &str| NormalizedAbsolutePath::new(p).unwrap();
    let bodies = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
        .with_rules_rust_defs_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        );
    let need = |p: &Path, operation| {
        SourcePreparationNeeds::path(NeedPathObservations::singleton(PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(p).unwrap(),
            operation,
        )))
    };
    let mut all = need(
        Path::new("/rules/rust/private"),
        PathObservationOperation::Lstat,
    );
    for relative in &RULES_RUST_IMPORT_METADATA[1..] {
        let selected = Path::new("/rules").join(relative);
        all = all
            .try_union(&need(&selected, PathObservationOperation::Lstat))
            .unwrap();
    }
    for relative in RULES_RUST_DEFS_IMPORTS {
        let selected = Path::new("/rules").join(relative);
        for operation in &operations {
            all = all.try_union(&need(&selected, operation.dupe())).unwrap();
        }
    }
    assert_eq!(all.path_observations().unwrap().demands().len(), 59);
    assert!(bodies.admit(&all).is_ok());
    for (excluded, operation) in [
        (
            "/rules/rust/private/unknown.bzl",
            PathObservationOperation::Lstat,
        ),
        (
            "/rules/rust/private/rust.bzl.extra",
            PathObservationOperation::FileBytes,
        ),
        (
            "/rules/rust/private/RUST.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/rules/RUST/private/rust.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/rules/rust/private/rust.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/rules/rust/private/rust.bzl",
            PathObservationOperation::FileDigest,
        ),
        ("/rules/rust/private", PathObservationOperation::FileBytes),
        ("/rules/rust/platform", PathObservationOperation::FileBytes),
        ("/rules/rust/settings", PathObservationOperation::FileBytes),
        (
            "/rules/rust/platform/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/rules/rust/settings/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/rules/rust/private/BUILD.bazel",
            PathObservationOperation::Lstat,
        ),
    ] {
        let mixed = all
            .try_union(&need(Path::new(excluded), operation.dupe()))
            .unwrap();
        let mut progress = 0;
        if bodies.admit(&mixed).is_ok() {
            progress += 1;
        }
        assert_eq!(progress, 0);
        let denied = bodies.admit(&mixed).unwrap_err();
        assert_eq!(denied.family, "path");
        assert_eq!(denied.operation, path_operation_code(&operation));
        assert_eq!(denied.path.unwrap().as_path(), Path::new(excluded));
    }
}
#[test]
fn rules_rust_defs_import_source_limits_preserve_byte_budgets() {
    use crate::runtime::dice::source_service::SourceCounter;
    let path = |p: &str| NormalizedAbsolutePath::new(p).unwrap();
    let make = || Bodies::new([path("/fixture"), path("/rules"), path("/platforms")]);
    let baseline = SourceServiceLimits {
        allow_network: false,
        registry_object_bytes: 1 << 20,
        registry_attempts: 128,
        registry_admitted_bytes: 16 << 20,
        source_attempts: 16,
        transferred_bytes: 16 << 20,
        decoded_bytes: 16 << 20,
        written_bytes: 16 << 20,
        live_buffer_bytes: 16 << 20,
        blocking_threads: Some(2),
    };
    assert_eq!(make().source_limits(RegistryChoice::PreparedFile), baseline);
    assert_eq!(SourceServiceLimits::default().source_attempts, 64);
    for (registry, command, attempts) in [
        (RegistryChoice::DefaultBcr, CommandChoice::Modules, 16),
        (RegistryChoice::DefaultBcr, CommandChoice::PublicProbe, 16),
        (RegistryChoice::PreparedFile, CommandChoice::Modules, 16),
        (
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
            128,
        ),
    ] {
        let bodies = make().with_rules_rust_defs_import_sources(registry, command);
        let limits = bodies.source_limits(registry);
        assert_eq!(
            limits,
            SourceServiceLimits {
                registry_attempts: registry.registry_attempts(),
                source_attempts: attempts,
                ..baseline
            }
        );
        for (counter, maximum) in [
            (SourceCounter::SourceAttempts, attempts),
            (SourceCounter::DecodedBytes, 16 << 20),
        ] {
            let start = Instant::now();
            let scope = SourceServiceScope::new_for_test(
                path("/fixture"),
                start + Duration::from_secs(2),
                start + Duration::from_secs(3),
                limits,
            )
            .unwrap();
            assert_eq!(scope.limits(), limits);
            scope.charge(counter, maximum).unwrap();
            assert_eq!(
                scope.charge(counter, 1),
                Err(SourceServiceError::Limit(counter))
            );
            let snapshot = scope.snapshot();
            assert_eq!(
                snapshot.source_attempts,
                if counter == SourceCounter::SourceAttempts {
                    attempts
                } else {
                    0
                }
            );
            assert_eq!(
                snapshot.decoded_bytes,
                if counter == SourceCounter::DecodedBytes {
                    16 << 20
                } else {
                    0
                }
            );
        }
    }
}
#[test]
fn probe_package_source_need_is_opt_in_exact_fixture_only() {
    for (relative, selected, operations, spellings) in [
        (
            "pkg",
            "/fixture/pkg",
            &[PathObservationOperation::Lstat][..],
            &[
                "/other/pkg",
                "/fixture/other/pkg",
                "/fixture/pkg/child",
                "/fixture/pkg.extra",
                "/fixture/PKG",
                "/fixture/Pkg",
                "/fixture-other/pkg",
            ][..],
        ),
        (
            "pkg/BUILD.bazel",
            "/fixture/pkg/BUILD.bazel",
            &[
                PathObservationOperation::Lstat,
                PathObservationOperation::FileBytes,
            ][..],
            &[
                "/other/pkg/BUILD.bazel",
                "/fixture/other/BUILD.bazel",
                "/fixture/pkg/BUILD.bazel/child",
                "/fixture/pkg/BUILD.bazel.extra",
                "/fixture/pkg/build.bazel",
                "/fixture/PKG/BUILD.bazel",
                "/fixture-other/pkg/BUILD.bazel",
            ][..],
        ),
        (
            "pkg/BUILD",
            "/fixture/pkg/BUILD",
            &[
                PathObservationOperation::Lstat,
                PathObservationOperation::FileBytes,
            ][..],
            &[
                "/other/pkg/BUILD",
                "/fixture/other/BUILD",
                "/fixture/pkg/BUILD/child",
                "/fixture/pkg/BUILD.extra",
                "/fixture/pkg/build",
                "/fixture/PKG/BUILD",
                "/fixture-other/pkg/BUILD",
            ][..],
        ),
    ] {
        assert_exact_root_file_role(
            relative,
            operations,
            Bodies::with_probe_package_source,
            |b| b.probe_package_source,
            spellings,
            selected,
            Some("/fixture"),
        );
    }
    let path = |p: &str| NormalizedAbsolutePath::new(p).unwrap();
    let bodies = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
        .with_probe_package_source(RegistryChoice::PreparedFile, CommandChoice::PublicProbe);
    let need =
        |p: &str, operation| {
            SourcePreparationNeeds::path(NeedPathObservations::singleton(
                PathObservationDemand::new(PathObservationNamespace::Host, path(p), operation),
            ))
        };
    let mut all = need("/fixture/pkg", PathObservationOperation::Lstat);
    for (marker, operation) in [
        ("/fixture/pkg/BUILD.bazel", PathObservationOperation::Lstat),
        (
            "/fixture/pkg/BUILD.bazel",
            PathObservationOperation::FileBytes,
        ),
        ("/fixture/pkg/BUILD", PathObservationOperation::Lstat),
        ("/fixture/pkg/BUILD", PathObservationOperation::FileBytes),
    ] {
        all = all.try_union(&need(marker, operation)).unwrap();
    }
    assert!(bodies.admit(&all).is_ok());
    for (excluded, operation) in [
        ("/fixture/pkg", PathObservationOperation::FileBytes),
        ("/fixture/pkg/child", PathObservationOperation::Lstat),
        ("/fixture/pkg/main.rs", PathObservationOperation::FileBytes),
    ] {
        let mixed = all.try_union(&need(excluded, operation.dupe())).unwrap();
        let mut progress = 0;
        if bodies.admit(&mixed).is_ok() {
            progress += 1;
        }
        assert_eq!(progress, 0);
        let denied = bodies.admit(&mixed).unwrap_err();
        assert_eq!(denied.family, "path");
        assert_eq!(denied.operation, path_operation_code(&operation));
        assert_eq!(denied.path.unwrap().as_path(), Path::new(excluded));
    }
}
#[test]
fn bazel_features_import_sources_need_is_opt_in_exact_table_only() {
    let operations = [
        PathObservationOperation::Lstat,
        PathObservationOperation::FileBytes,
    ];
    for relative in BAZEL_FEATURES_IMPORTS {
        let selected = Path::new("/features").join(relative);
        assert_exact_root_file_role(
            relative,
            &operations,
            Bodies::with_bazel_features_import_sources,
            |b| b.bazel_features_import_sources,
            &[],
            selected.to_str().unwrap(),
            Some("/features"),
        );
    }
    for relative in BAZEL_FEATURES_IMPORT_METADATA {
        let selected = Path::new("/features").join(relative);
        assert_exact_root_file_role(
            relative,
            &[PathObservationOperation::Lstat],
            Bodies::with_bazel_features_import_sources,
            |b| b.bazel_features_import_sources,
            &[],
            selected.to_str().unwrap(),
            Some("/features"),
        );
    }
    let path = |s: &str| NormalizedAbsolutePath::new(s).unwrap();
    let make = || Bodies::new([path("/fixture"), path("/rules"), path("/platforms")]);
    let need = |p: &Path, operation| {
        SourcePreparationNeeds::path(NeedPathObservations::singleton(PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(p).unwrap(),
            operation,
        )))
    };
    let bodies = make()
        .with_fourth(path("/features"))
        .with_bazel_features_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        );
    let mut all = need(
        Path::new("/features/private"),
        PathObservationOperation::Lstat,
    );
    for relative in BAZEL_FEATURES_IMPORTS {
        let selected = Path::new("/features").join(relative);
        for operation in &operations {
            all = all.try_union(&need(&selected, operation.dupe())).unwrap();
        }
    }
    assert_eq!(all.path_observations().unwrap().demands().len(), 17);
    assert!(bodies.admit(&all).is_ok());
    assert_eq!(
        bodies
            .source_limits(RegistryChoice::PreparedFile)
            .source_attempts,
        16
    );
    for (excluded, operation) in [
        ("/features/private", PathObservationOperation::FileBytes),
        ("/features/private", PathObservationOperation::FileDigest),
        (
            "/features/features.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/features/features.bzl-extra",
            PathObservationOperation::FileBytes,
        ),
        (
            "/features/Features.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/features/private/util.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/features/private/util.bzl-extra",
            PathObservationOperation::FileBytes,
        ),
        (
            "/features/private/Util.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/features/private/parse.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/features/private/parse.bzl-extra",
            PathObservationOperation::FileBytes,
        ),
        (
            "/features/private/Parse.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/features/private/parse_extra.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/features/private/unknown.bzl",
            PathObservationOperation::Lstat,
        ),
        (
            "/features/PRIVATE/parse.bzl",
            PathObservationOperation::Lstat,
        ),
        (
            "/features/private/parse.bzl",
            PathObservationOperation::FileDigest,
        ),
        ("/rules/features.bzl", PathObservationOperation::Lstat),
        (
            "/skylib/private/parse.bzl",
            PathObservationOperation::FileBytes,
        ),
        ("/cc/private/util.bzl", PathObservationOperation::FileBytes),
    ] {
        let mixed = all
            .try_union(&need(Path::new(excluded), operation.dupe()))
            .unwrap();
        let mut progress = 0;
        if bodies.admit(&mixed).is_ok() {
            progress += 1;
        }
        assert_eq!(progress, 0);
        let denied = bodies.admit(&mixed).unwrap_err();
        assert_eq!(
            (denied.family, denied.operation),
            ("path", path_operation_code(&operation))
        );
        assert_eq!(denied.path.unwrap().as_path(), Path::new(excluded));
    }
    // Each newly captured leaf remains exact in a mixed whole-Need batch.
    for relative in &BAZEL_FEATURES_IMPORTS[3..] {
        let selected = Path::new("/features").join(relative);
        for (excluded, operation) in [
            (selected.join("child"), PathObservationOperation::Lstat),
            (
                Path::new("/features").join(format!("{relative}-extra")),
                PathObservationOperation::FileBytes,
            ),
            (
                Path::new("/features").join(relative.to_ascii_uppercase()),
                PathObservationOperation::FileBytes,
            ),
            (
                Path::new("/features").join(format!("{relative}.sibling")),
                PathObservationOperation::Lstat,
            ),
            (
                Path::new("/skylib").join(relative),
                PathObservationOperation::FileBytes,
            ),
            (selected, PathObservationOperation::FileDigest),
        ] {
            let mixed = all.try_union(&need(&excluded, operation.dupe())).unwrap();
            let mut progress = 0;
            if bodies.admit(&mixed).is_ok() {
                progress += 1;
            }
            assert_eq!(progress, 0);
            let denied = bodies.admit(&mixed).unwrap_err();
            assert_eq!(
                (denied.family, denied.operation),
                ("path", path_operation_code(&operation))
            );
            assert_eq!(denied.path.unwrap().as_path(), excluded.as_path());
        }
    }
    for sparse in [
        make(),
        make()
            .with_fourth(path("/other"))
            .with_fifth(path("/features")),
        make()
            .with_fourth(path("/other"))
            .with_fifth(path("/skylib"))
            .with_sixth(path("/features")),
    ] {
        let sparse = sparse.with_bazel_features_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        );
        assert!(sparse.bazel_features_import_sources);
        assert!(sparse.admit(&all).is_err());
    }
}
#[test]
fn bazel_skylib_import_sources_need_is_opt_in_exact_table_only() {
    let operations = [
        PathObservationOperation::Lstat,
        PathObservationOperation::FileBytes,
    ];
    for relative in BAZEL_SKYLIB_IMPORTS {
        let selected = Path::new("/skylib").join(relative);
        assert_exact_root_file_role(
            relative,
            &operations,
            Bodies::with_bazel_skylib_import_sources,
            |b| b.bazel_skylib_import_sources,
            &[],
            selected.to_str().unwrap(),
            Some("/skylib"),
        );
    }
    for relative in BAZEL_SKYLIB_IMPORT_METADATA {
        let selected = Path::new("/skylib").join(relative);
        assert_exact_root_file_role(
            relative,
            &[PathObservationOperation::Lstat],
            Bodies::with_bazel_skylib_import_sources,
            |b| b.bazel_skylib_import_sources,
            &[],
            selected.to_str().unwrap(),
            Some("/skylib"),
        );
    }
    let path = |s: &str| NormalizedAbsolutePath::new(s).unwrap();
    let make = || Bodies::new([path("/fixture"), path("/rules"), path("/platforms")]);
    let need = |p: &Path, operation| {
        SourcePreparationNeeds::path(NeedPathObservations::singleton(PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(p).unwrap(),
            operation,
        )))
    };
    let bodies = make()
        .with_fourth(path("/features"))
        .with_fifth(path("/skylib"))
        .with_bazel_skylib_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe);
    let mut all = need(Path::new("/skylib/lib"), PathObservationOperation::Lstat);
    all = all
        .try_union(&need(
            Path::new("/skylib/rules"),
            PathObservationOperation::Lstat,
        ))
        .unwrap();
    for relative in BAZEL_SKYLIB_IMPORTS {
        let selected = Path::new("/skylib").join(relative);
        for operation in &operations {
            all = all.try_union(&need(&selected, operation.dupe())).unwrap();
        }
    }
    assert_eq!(all.path_observations().unwrap().demands().len(), 10);
    assert!(bodies.admit(&all).is_ok());
    assert_eq!(
        bodies
            .source_limits(RegistryChoice::PreparedFile)
            .source_attempts,
        16
    );
    for (excluded, operation) in [
        ("/skylib/lib", PathObservationOperation::FileBytes),
        ("/skylib/rules", PathObservationOperation::FileBytes),
        (
            "/skylib/lib/paths.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/skylib/lib/paths.bzl-extra",
            PathObservationOperation::FileBytes,
        ),
        ("/skylib/lib/Paths.bzl", PathObservationOperation::FileBytes),
        ("/skylib/LIB/paths.bzl", PathObservationOperation::Lstat),
        (
            "/skylib/rules/unknown.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/skylib/rules/common_settings.bzl/child",
            PathObservationOperation::Lstat,
        ),
        ("/rules/lib/paths.bzl", PathObservationOperation::Lstat),
        (
            "/features/lib/paths.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/skylib/lib/BUILD.bazel",
            PathObservationOperation::FileBytes,
        ),
        (
            "/skylib/rules/BUILD.bazel",
            PathObservationOperation::FileBytes,
        ),
        (
            "/skylib/lib/paths.bzl",
            PathObservationOperation::FileDigest,
        ),
    ] {
        let mixed = all
            .try_union(&need(Path::new(excluded), operation.dupe()))
            .unwrap();
        let mut progress = 0;
        if bodies.admit(&mixed).is_ok() {
            progress += 1;
        }
        assert_eq!(progress, 0);
        let denied = bodies.admit(&mixed).unwrap_err();
        assert_eq!(
            (denied.family, denied.operation),
            ("path", path_operation_code(&operation))
        );
        assert_eq!(denied.path.unwrap().as_path(), Path::new(excluded));
    }
    let selected = Path::new("/skylib/lib/modules.bzl");
    for (excluded, operation) in [
        (selected.join("child"), PathObservationOperation::Lstat),
        (
            Path::new("/skylib/lib/modules.bzl-extra").to_path_buf(),
            PathObservationOperation::FileBytes,
        ),
        (
            Path::new("/skylib/lib/Modules.bzl").to_path_buf(),
            PathObservationOperation::FileBytes,
        ),
        (
            Path::new("/skylib/lib/modules.sibling.bzl").to_path_buf(),
            PathObservationOperation::Lstat,
        ),
        (
            Path::new("/features/lib/modules.bzl").to_path_buf(),
            PathObservationOperation::FileBytes,
        ),
        (selected.to_path_buf(), PathObservationOperation::FileDigest),
    ] {
        let mixed = all.try_union(&need(&excluded, operation.dupe())).unwrap();
        let mut progress = 0;
        if bodies.admit(&mixed).is_ok() {
            progress += 1;
        }
        assert_eq!(progress, 0);
        let denied = bodies.admit(&mixed).unwrap_err();
        assert_eq!(
            (denied.family, denied.operation),
            ("path", path_operation_code(&operation))
        );
        assert_eq!(denied.path.unwrap().as_path(), excluded.as_path());
    }
    for sparse in [make(), make().with_fourth(path("/skylib"))] {
        let sparse = sparse.with_bazel_skylib_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        );
        assert!(sparse.bazel_skylib_import_sources);
        assert!(sparse.admit(&all).is_err());
    }
    let wrong_slot = make()
        .with_fourth(path("/skylib"))
        .with_fifth(path("/other"))
        .with_bazel_skylib_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe);
    assert!(wrong_slot.admit(&all).is_err());
}
#[test]
fn rules_cc_import_sources_need_is_opt_in_exact_table_only() {
    let operations = [
        PathObservationOperation::Lstat,
        PathObservationOperation::FileBytes,
    ];
    for relative in RULES_CC_IMPORTS {
        let selected = Path::new("/cc").join(relative);
        assert_exact_root_file_role(
            relative,
            &operations,
            Bodies::with_rules_cc_import_sources,
            |b| b.rules_cc_import_sources,
            &[],
            selected.to_str().unwrap(),
            Some("/cc"),
        );
    }
    for relative in RULES_CC_IMPORT_METADATA {
        let selected = Path::new("/cc").join(relative);
        assert_exact_root_file_role(
            relative,
            &[PathObservationOperation::Lstat],
            Bodies::with_rules_cc_import_sources,
            |b| b.rules_cc_import_sources,
            &[],
            selected.to_str().unwrap(),
            Some("/cc"),
        );
    }
    let path = |s: &str| NormalizedAbsolutePath::new(s).unwrap();
    let make = || Bodies::new([path("/fixture"), path("/rules"), path("/platforms")]);
    let need = |p: &Path, operation| {
        SourcePreparationNeeds::path(NeedPathObservations::singleton(PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(p).unwrap(),
            operation,
        )))
    };
    let bodies = make()
        .with_fourth(path("/features"))
        .with_fifth(path("/skylib"))
        .with_sixth(path("/cc"))
        .with_rules_cc_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe);
    let mut all = need(Path::new("/cc/cc"), PathObservationOperation::Lstat);
    all = all
        .try_union(&need(
            Path::new("/cc/cc/common"),
            PathObservationOperation::Lstat,
        ))
        .unwrap();
    for relative in &RULES_CC_IMPORT_METADATA[2..] {
        let selected = Path::new("/cc").join(relative);
        all = all
            .try_union(&need(&selected, PathObservationOperation::Lstat))
            .unwrap();
    }
    for relative in RULES_CC_IMPORTS {
        let selected = Path::new("/cc").join(relative);
        for operation in &operations {
            all = all.try_union(&need(&selected, operation.dupe())).unwrap();
        }
    }
    assert_eq!(all.path_observations().unwrap().demands().len(), 104);
    assert!(bodies.admit(&all).is_ok());
    assert_eq!(
        bodies
            .source_limits(RegistryChoice::PreparedFile)
            .source_attempts,
        16
    );
    for (excluded, operation) in [
        (
            "/cc/cc/private/link/collect_solib_dirs.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/private/link/collect_solib_dirs.bzl-extra",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/private/link/Collect_solib_dirs.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/rules/cc/private/link/collect_solib_dirs.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/private/link/create_libraries_to_link_values.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/private/link/create_libraries_to_link_values.bzl",
            PathObservationOperation::FileDigest,
        ),
        (
            "/cc/cc/private/link/finalize_link_action.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/private/link/finalize_link_action.bzl-extra",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/private/link/Finalize_link_action.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/rules/cc/private/link/finalize_link_action.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/private/link/finalize_link_action.bzl",
            PathObservationOperation::FileDigest,
        ),
        (
            "/cc/cc/private/link/cpp_link_action.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/private/link/cpp_link_action.bzl-extra",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/private/link/Cpp_link_action.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/rules/cc/private/link/cpp_link_action.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/private/link/lto_indexing_action_sibling.bzl",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/private/link/lto_indexing_action.bzl",
            PathObservationOperation::FileDigest,
        ),
        (
            "/cc/cc/action_names.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/Action_names.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/rules/cc/action_names.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/action_names.bzl",
            PathObservationOperation::FileDigest,
        ),
        (
            "/cc/cc/private/paths.bzl-extra",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/private/paths_sibling.bzl",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/private/compile/cc_compilation_helper.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/private/link/cc_linking_helper.bzl-extra",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/private/compile",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/private/compile",
            PathObservationOperation::FileDigest,
        ),
        ("/cc/cc/private/link", PathObservationOperation::FileBytes),
        ("/cc/cc/private/link", PathObservationOperation::FileDigest),
        (
            "/cc/cc/private/rules_impl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/private/rules_impl",
            PathObservationOperation::FileDigest,
        ),
        (
            "/cc/cc/common/cc_helper_internal.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/common/cc_helper_internal.bzl-extra",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/common/Cc_helper_internal.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/common/cc_helper_internal_sibling.bzl",
            PathObservationOperation::Lstat,
        ),
        (
            "/rules/cc/common/cc_helper_internal.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/common/cc_helper_internal.bzl",
            PathObservationOperation::FileDigest,
        ),
        (
            "/cc/cc/private/compile/compile.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/private/link/link.bzl-extra",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/private/cc_common.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/private/cc_common.bzl-extra",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/private/Cc_common.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/private/cc_common_sibling.bzl",
            PathObservationOperation::Lstat,
        ),
        (
            "/rules/cc/private/cc_info.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/private/toolchain_config",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/private/toolchain_config",
            PathObservationOperation::FileDigest,
        ),
        (
            "/cc/cc/private/toolchain_config/cc_toolchain_config_info.bzl/child",
            PathObservationOperation::Lstat,
        ),
        ("/cc/cc", PathObservationOperation::FileBytes),
        ("/cc/cc/common", PathObservationOperation::FileBytes),
        ("/cc/cc/private", PathObservationOperation::FileBytes),
        ("/cc/cc/private", PathObservationOperation::FileDigest),
        (
            "/cc/cc/private/toolchain",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/private/toolchain",
            PathObservationOperation::FileDigest,
        ),
        (
            "/cc/cc/private/toolchain/cc_configure.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/private/toolchain/cc_configure.bzl-extra",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/private/toolchain/Cc_configure.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/private/toolchain/unknown_cc_configure.bzl",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/private/toolchain/unix_cc_configure.bzl/child",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/private/toolchain/windows_cc_configure.bzl-extra",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/private/toolchain/Lib_cc_configure.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/rules/cc/private/toolchain/cc_configure.bzl",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/extensions.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/extensions.bzl-extra",
            PathObservationOperation::FileBytes,
        ),
        ("/cc/cc/Extensions.bzl", PathObservationOperation::FileBytes),
        (
            "/cc/cc/extensions_extra.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/extensions.bzl",
            PathObservationOperation::FileDigest,
        ),
        ("/rules/cc/extensions.bzl", PathObservationOperation::Lstat),
        (
            "/skylib/cc/extensions.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/find_cc_toolchain.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/find_cc_toolchain.bzl-extra",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/Find_cc_toolchain.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/CC/find_cc_toolchain.bzl",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/common/unknown.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/common/cc_common.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/rules/cc/find_cc_toolchain.bzl",
            PathObservationOperation::Lstat,
        ),
        (
            "/skylib/cc/find_cc_toolchain.bzl",
            PathObservationOperation::FileBytes,
        ),
        ("/cc/cc/BUILD.bazel", PathObservationOperation::FileBytes),
        (
            "/cc/cc/common/BUILD.bazel",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/find_cc_toolchain.bzl",
            PathObservationOperation::FileDigest,
        ),
    ] {
        let mixed = all
            .try_union(&need(Path::new(excluded), operation.dupe()))
            .unwrap();
        let mut progress = 0;
        if bodies.admit(&mixed).is_ok() {
            progress += 1;
        }
        assert_eq!(progress, 0);
        let denied = bodies.admit(&mixed).unwrap_err();
        assert_eq!(
            (denied.family, denied.operation),
            ("path", path_operation_code(&operation))
        );
        assert_eq!(denied.path.unwrap().as_path(), Path::new(excluded));
    }
    for sparse in [
        make(),
        make().with_fourth(path("/cc")),
        make()
            .with_fourth(path("/features"))
            .with_fifth(path("/cc")),
    ] {
        let sparse = sparse
            .with_rules_cc_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe);
        assert!(sparse.rules_cc_import_sources);
        assert!(sparse.admit(&all).is_err());
    }
    let wrong_slot = make()
        .with_fourth(path("/features"))
        .with_fifth(path("/cc"))
        .with_sixth(path("/other"))
        .with_rules_cc_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe);
    assert!(wrong_slot.admit(&all).is_err());
}
fn assert_exact_root_file_role(
    name: &str,
    admitted_operations: &[PathObservationOperation],
    select: fn(Bodies, RegistryChoice, CommandChoice) -> Bodies,
    is_enabled: fn(&Bodies) -> bool,
    spellings: &[&str],
    selected_path: &str,
    selected_root: Option<&str>,
) {
    let path = |s: &str| NormalizedAbsolutePath::new(s).unwrap();
    let make = || {
        Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
            .with_fourth(path("/features"))
            .with_fifth(path("/skylib"))
            .with_sixth(path("/cc"))
            .with_seventh(path("/license"))
            .with_eighth(path("/shell"))
            .with_ninth(path("/apple"))
            .with_tenth(path("/metadata"))
            .with_eleventh(path("/buildozer"))
    };
    let budget = slug_workspace_v2::ImageDescriptorBudget::new(4096);
    let need = |namespace, leaf: NormalizedAbsolutePath, operation| {
        let demand = match operation {
            PathObservationOperation::WindowsLongPath => {
                assert_eq!(namespace, PathObservationNamespace::Host);
                PathObservationDemand::windows_long_path(leaf, Arc::from([b'x' as u16]))
            }
            PathObservationOperation::WindowsOptionPathLongName => {
                assert_eq!(namespace, PathObservationNamespace::Host);
                PathObservationDemand::windows_option_path_long_name(leaf, Arc::from([b'x' as u16]))
            }
            PathObservationOperation::InputImage(request) => PathObservationDemand::input_image(
                namespace,
                slug_workspace_v2::ImageChargedPath::pin_existing(leaf, &budget).unwrap(),
                request,
            )
            .unwrap(),
            _ => PathObservationDemand::new(namespace, leaf, operation),
        };
        SourcePreparationNeeds::path(NeedPathObservations::singleton(demand))
    };
    let host = PathObservationNamespace::Host;
    assert!(!is_enabled(&make()));
    for (registry, command, enabled) in [
        (RegistryChoice::DefaultBcr, CommandChoice::Modules, false),
        (
            RegistryChoice::DefaultBcr,
            CommandChoice::PublicProbe,
            false,
        ),
        (RegistryChoice::PreparedFile, CommandChoice::Modules, false),
        (
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
            true,
        ),
    ] {
        let bodies = select(make(), registry, command);
        assert_eq!(is_enabled(&bodies), enabled);
        assert_eq!(bodies.roots().count(), 11);
        for root in bodies.roots() {
            let marker = NormalizedAbsolutePath::new(root.as_path().join(name)).unwrap();
            for admitted_operation in admitted_operations {
                let demand = need(host, marker.dupe(), admitted_operation.dupe());
                if enabled
                    && selected_root.is_none_or(|selected| root.as_path() == Path::new(selected))
                {
                    assert!(bodies.admit(&demand).is_ok());
                } else {
                    let denied = bodies.admit(&demand).unwrap_err();
                    assert_eq!(
                        (denied.family, denied.operation),
                        ("path", path_operation_code(&admitted_operation))
                    );
                    assert_eq!(denied.path.as_ref(), Some(&marker));
                }
            }
        }
    }
    let admitted = select(
        make(),
        RegistryChoice::PreparedFile,
        CommandChoice::PublicProbe,
    );
    let foreign = PathObservationNamespace::Materialization(
        slug_workspace_v2::PathObservationInstanceId::new(1),
    );
    for root in admitted.roots() {
        let marker = NormalizedAbsolutePath::new(root.as_path().join(name)).unwrap();
        let anchor =
            slug_workspace_v2::ImageChargedPath::pin_existing(root.dupe(), &budget).unwrap();
        let image = slug_workspace_v2::ImageObservationRequest::new(
            anchor,
            slug_workspace_v2::ImageObservationOperation::Lstat,
        );
        for operation in [
            PathObservationOperation::Lstat,
            PathObservationOperation::FileBytes,
            PathObservationOperation::FileDigest,
            PathObservationOperation::ReadLink,
            PathObservationOperation::DirectoryEntries,
            PathObservationOperation::InputImage(image),
            PathObservationOperation::WindowsLongPath,
            PathObservationOperation::WindowsOptionPathLongName,
        ] {
            if (admitted_operations.contains(&operation)
                && selected_root.is_none_or(|selected| root.as_path() == Path::new(selected)))
                || (name == ".bazelignore" && operation == PathObservationOperation::Lstat)
            {
                assert!(
                    admitted
                        .admit(&need(host, marker.dupe(), operation))
                        .is_ok()
                );
                continue;
            }
            let code = path_operation_code(&operation);
            let denied = admitted
                .admit(&need(host, marker.dupe(), operation))
                .unwrap_err();
            assert_eq!(denied.operation, code);
            assert_eq!(denied.path.as_ref(), Some(&marker));
        }
        for admitted_operation in admitted_operations {
            let denied = admitted
                .admit(&need(foreign, marker.dupe(), admitted_operation.dupe()))
                .unwrap_err();
            assert_eq!(
                (denied.family, denied.operation),
                ("namespace", path_operation_code(&admitted_operation))
            );
            assert_eq!(denied.path.as_ref(), Some(&marker));
        }
    }
    assert_eq!(budget.used_bytes(), 0);
    for admitted_operation in admitted_operations {
        for &spelling in spellings {
            let denied = admitted
                .admit(&need(host, path(spelling), admitted_operation.dupe()))
                .unwrap_err();
            assert_eq!(
                (denied.family, denied.operation),
                ("path", path_operation_code(&admitted_operation))
            );
            assert_eq!(denied.path.unwrap().as_path(), Path::new(spelling));
        }
        let allowed = need(host, path(selected_path), admitted_operation.dupe());
        let mut progress_calls = 0;
        for (excluded, family, operation) in [
            (
                need(
                    host,
                    path(selected_path),
                    PathObservationOperation::FileDigest,
                ),
                "path",
                "file_digest",
            ),
            (
                need(foreign, path(selected_path), admitted_operation.dupe()),
                "namespace",
                path_operation_code(&admitted_operation),
            ),
            (
                SourcePreparationNeeds::root_module_bootstrap(
                    slug_bzlmod_v2::RootModuleBootstrapRequest {
                        workspace: path("/foreign"),
                    },
                ),
                "bootstrap",
                "none",
            ),
        ] {
            let mixed = allowed.try_union(&excluded).unwrap();
            if admitted.admit(&mixed).is_ok() {
                progress_calls += 1;
            }
            let denied = admitted.admit(&mixed).unwrap_err();
            assert_eq!((denied.family, denied.operation), (family, operation));
        }
        assert_eq!(progress_calls, 0);
    }
}

#[test]
#[ignore = "bounded normal public producer observation; frozen supervisor inputs required"]
fn native_eleven_root_prepared_file_public_probe_prerequisite() {
    observe_native_command(
        ModuleMode::Eleven,
        RegistryChoice::PreparedFile,
        CommandChoice::PublicProbe,
    );
}
