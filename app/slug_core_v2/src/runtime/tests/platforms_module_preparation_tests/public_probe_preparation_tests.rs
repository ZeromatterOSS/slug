use super::*;

#[cfg(target_os = "linux")]
mod progress_refusal_tests {
    include!("public_probe_preparation_tests/progress_refusal_tests.rs");
}

#[cfg(target_os = "linux")]
mod immutable_archive_namespace {
    include!("public_probe_preparation_tests/immutable_archive_namespace.rs");
}

mod platforms_os_package_tests {
    include!("public_probe_preparation_tests/platforms_os_package_tests.rs");
}

mod platforms_cpu_package_tests {
    include!("public_probe_preparation_tests/platforms_cpu_package_tests.rs");
}

mod platforms_host_directory_tests {
    include!("public_probe_preparation_tests/platforms_host_directory_tests.rs");
}

mod source_attempt_context_tests {
    include!("public_probe_preparation_tests/source_attempt_context_tests.rs");
}

#[derive(Clone, Copy)]
pub(super) enum CommandChoice {
    Modules,
    PublicProbe,
}
pub(super) fn offline_archive_native_role(
    registry: RegistryChoice,
    command: CommandChoice,
) -> bool {
    matches!(registry, RegistryChoice::PreparedFile)
        && matches!(command, CommandChoice::PublicProbe)
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
                "ancestor" | "root" | "repo_marker" | "ignore_marker" | "package_marker" => {
                    operation == &PathObservationOperation::Lstat
                }
                "fixed_file" => matches!(
                    operation,
                    PathObservationOperation::Lstat | PathObservationOperation::FileBytes
                ),
                #[cfg(target_os = "linux")]
                "immutable_tree" => matches!(
                    operation,
                    PathObservationOperation::Lstat
                        | PathObservationOperation::FileBytes
                        | PathObservationOperation::FileDigest
                        | PathObservationOperation::DirectoryEntries
                        | PathObservationOperation::ReadLink
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
    #[cfg(target_os = "linux")]
    if let Some(policy) =
        immutable_archive_namespace::policy(bodies, request, view.generation_root(), demand)
    {
        return policy;
    }
    #[cfg(target_os = "linux")]
    if let Some(plan) = bodies.qualified_java_file_plan(request) {
        let path = demand.path().as_path();
        let root = view.generation_root();
        let path_class = if path == root {
            "root"
        } else if root.starts_with(path) {
            "ancestor"
        } else if let Ok(relative) = path.strip_prefix(root) {
            if plan
                .effects()
                .iter()
                .any(|file| relative == Path::new(file.path()))
            {
                "fixed_file"
            } else if bodies.repo_marker_metadata && relative == Path::new("REPO.bazel") {
                "repo_marker"
            } else if bodies.ignore_file_bytes && relative == Path::new(".bazelignore") {
                "ignore_marker"
            } else if plan
                .effects()
                .iter()
                .any(|file| Path::new(file.path()).starts_with(relative))
            {
                "ancestor"
            } else {
                "other"
            }
        } else {
            "other"
        };
        return NamespacePolicy {
            owner_plan_allowed: true,
            path_class,
        };
    }
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
        } else if (std::ptr::eq(files, &GENERATED_RUST_TOOLCHAIN_HUB_FILES[..])
            || std::ptr::eq(files, &GENERATED_WINSDK_FILES[..])
            || std::ptr::eq(files, &GENERATED_CC_TOOLCHAINS_FILES[..])
            || std::ptr::eq(files, &GENERATED_SHELL_CONFIG_FILES[..])
            || std::ptr::eq(files, &GENERATED_APPLE_TOOLCHAINS_FILES[..]))
            && bodies.repo_marker_metadata
            && relative == Path::new("REPO.bazel")
        {
            "repo_marker"
        } else if (std::ptr::eq(files, &GENERATED_RUST_TOOLCHAIN_HUB_FILES[..])
            || std::ptr::eq(files, &GENERATED_WINSDK_FILES[..])
            || std::ptr::eq(files, &GENERATED_CC_TOOLCHAINS_FILES[..])
            || std::ptr::eq(files, &GENERATED_SHELL_CONFIG_FILES[..])
            || std::ptr::eq(files, &GENERATED_APPLE_TOOLCHAINS_FILES[..]))
            && bodies.ignore_file_bytes
            && relative == Path::new(".bazelignore")
        {
            "ignore_marker"
        } else if (std::ptr::eq(files, &GENERATED_WINSDK_FILES[..])
            || std::ptr::eq(files, &GENERATED_CC_TOOLCHAINS_FILES[..])
            || std::ptr::eq(files, &GENERATED_SHELL_CONFIG_FILES[..])
            || std::ptr::eq(files, &GENERATED_APPLE_TOOLCHAINS_FILES[..]))
            && relative == Path::new("BUILD.bazel")
        {
            "package_marker"
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
        if let Some(diagnostic) = self.bodies.environment_diagnostic(needs, &denied) {
            println!("{diagnostic}");
        }
        if let Some(diagnostic) = self.bodies.materialization_diagnostic(needs, &denied) {
            println!("{diagnostic}");
            if let Some(spec) = self.bodies.immutable_spec_diagnostic(needs, &denied) {
                println!("{spec}");
            }
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
    #[cfg(target_os = "linux")]
    fn progress_refused(
        &self,
        needs: &SourcePreparationNeeds,
        command: &NativeDemandCommand<'_>,
        attempt: usize,
        error: &NativeDemandSessionError,
    ) {
        use std::io::Write;
        if let Some(diagnostic) =
            progress_refusal_diagnostic::ProgressRefusalDiagnostic::for_refusal(
                &self.inner.0,
                command,
                needs,
                attempt,
                error,
            )
        {
            // Reporting is synchronous and never replaces the original refusal.
            let _ = std::io::stdout()
                .lock()
                .write_fmt(format_args!("{diagnostic}"));
        }
    }
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
        && request.repository_environment.is_empty()
        && request.lockfile_mode == LockfileMode::Off
        && inner.0.workspace == bodies.roots[0]
        && matches!(inner.0.targets.as_ref(), [pattern] if pattern.as_ref() == "//pkg:probe")
        && bodies.visible_lockfile_metadata
        && bodies.repo_marker_metadata
        && bodies.ignore_file_bytes
        && bodies.rust_package_metadata
        && bodies.platforms_host_metadata
        && bodies.platforms_host_directory
        && bodies.platforms_host_import_sources
        && bodies.platforms_cpu_package_sources
        && bodies.platforms_os_package_sources
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
        && bodies.rules_shell_import_sources
        && bodies.additional[4].is_some()
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
        for rejected in [
            mixed.try_union(&earlier).unwrap(),
            earlier.try_union(&mixed).unwrap(),
        ] {
            let denied = bodies.admit(&rejected).unwrap_err();
            assert_eq!(denied.family, family);
            let diagnostic = bodies.environment_diagnostic(&rejected, &denied);
            assert_eq!(diagnostic.is_some(), family == "environment");
            if let Some(view) = diagnostic {
                assert!(std::ptr::eq(
                    view.request,
                    rejected.repository_environment().unwrap()
                ));
                assert!(std::ptr::eq(
                    view.request.names().iter().next().unwrap().as_ptr(),
                    earlier
                        .repository_environment()
                        .unwrap()
                        .names()
                        .iter()
                        .next()
                        .unwrap()
                        .as_ptr(),
                ));
                let with_bootstrap = rejected
                    .try_union(&SourcePreparationNeeds::root_module_bootstrap(
                        slug_bzlmod_v2::RootModuleBootstrapRequest {
                            workspace: path("/foreign"),
                        },
                    ))
                    .unwrap();
                let earlier = bodies.admit(&with_bootstrap).unwrap_err();
                assert_eq!(earlier.family, "bootstrap");
                assert!(
                    bodies
                        .environment_diagnostic(&with_bootstrap, &earlier)
                        .is_none()
                );
            }
            assert!(
                bodies
                    .materialization_diagnostic(&rejected, &denied)
                    .is_none()
            );
        }
    }
    let denied = bodies.admit(&mixed).unwrap_err();
    assert!(bodies.materialization_diagnostic(&mixed, &denied).is_some());
    assert!(bodies.admit(&body).is_ok());
}
#[test]
fn repository_environment_names_need_is_opt_in_exact_snapshot_frontier_only() {
    use slug_bzlmod_v2::NeedRepositoryEnvironmentNames;
    use slug_bzlmod_v2::RepositoryEnvironmentNameFrontier;
    eprintln!(
        "WP1054_SHELL_ENVIRONMENT_LAYOUT names={} row={} literal={} apple_names={} apple_literal={}",
        std::mem::size_of_val(&RULES_SHELL_ENVIRONMENT_NAMES),
        std::mem::size_of::<&str>(),
        RULES_SHELL_ENVIRONMENT_NAMES
            .iter()
            .map(|name| name.len())
            .sum::<usize>(),
        std::mem::size_of_val(&APPLE_SUPPORT_ENVIRONMENT_NAMES),
        APPLE_SUPPORT_ENVIRONMENT_NAMES
            .iter()
            .map(|name| name.len())
            .sum::<usize>()
    );
    let path = |s: &str| NormalizedAbsolutePath::new(s).unwrap();
    let make = || Bodies::new([path("/fixture"), path("/rules"), path("/platforms")]);
    let selected = || {
        make()
            .with_fourth(path("/features"))
            .with_fifth(path("/skylib"))
            .with_sixth(path("/cc"))
            .with_seventh(path("/license"))
            .with_eighth(path("/shell"))
            .with_ninth(path("/apple"))
    };
    let need = |workspace: &str, names: &[&str]| {
        SourcePreparationNeeds::environment(
            NeedRepositoryEnvironmentNames::new(
                path(workspace),
                RepositoryEnvironmentNameFrontier::from_unsorted(
                    names
                        .iter()
                        .map(|name| compact_str::CompactString::new(name)),
                ),
            )
            .unwrap(),
        )
    };
    let apple = need("/fixture", &APPLE_SUPPORT_ENVIRONMENT_NAMES);
    assert_eq!(apple.repository_environment().unwrap().names().len(), 2);
    assert!(selected().admit(&apple).is_err());
    let all = need("/fixture", &RULES_CC_ENVIRONMENT_NAMES);
    assert_eq!(all.repository_environment().unwrap().names().len(), 14);
    assert!(selected().admit(&all).is_err());
    for registry in [RegistryChoice::DefaultBcr, RegistryChoice::PreparedFile] {
        for command in [CommandChoice::Modules, CommandChoice::PublicProbe] {
            let bodies = selected().with_rules_cc_import_sources(registry, command);
            let enabled = matches!(registry, RegistryChoice::PreparedFile)
                && matches!(command, CommandChoice::PublicProbe);
            assert_eq!(bodies.rules_cc_import_sources, enabled);
            assert_eq!(bodies.admit(&all).is_ok(), enabled);
            let shell_bodies = selected().with_rules_shell_import_sources(registry, command);
            assert_eq!(shell_bodies.rules_shell_import_sources, enabled);
            assert_eq!(
                shell_bodies
                    .admit(&need("/fixture", &RULES_SHELL_ENVIRONMENT_NAMES))
                    .is_ok(),
                enabled
            );
            assert!(shell_bodies.admit(&all).is_err());
            assert!(bodies.admit(&apple).is_err());
            assert!(shell_bodies.admit(&apple).is_err());
            let apple_bodies = selected().with_apple_support_import_sources(registry, command);
            assert_eq!(apple_bodies.apple_support_import_sources, enabled);
            assert_eq!(apple_bodies.admit(&apple).is_ok(), enabled);
            assert!(apple_bodies.admit(&all).is_err());
            assert!(
                apple_bodies
                    .admit(&need("/fixture", &RULES_SHELL_ENVIRONMENT_NAMES))
                    .is_err()
            );
        }
    }
    let bodies = selected()
        .with_rules_cc_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe);
    for name in RULES_CC_ENVIRONMENT_NAMES {
        assert!(bodies.admit(&need("/fixture", &[name])).is_ok());
    }
    let detection = need("/fixture", &["BAZEL_DO_NOT_DETECT_CPP_TOOLCHAIN"]);
    assert!(bodies.admit(&detection).is_ok());
    let old = need("/fixture", &["TEMP", "BAZEL_VC"]);
    for mixed in [
        old.try_union(&detection).unwrap(),
        detection.try_union(&old).unwrap(),
    ] {
        assert_eq!(mixed.repository_environment().unwrap().names().len(), 3);
        assert!(bodies.admit(&mixed).is_ok());
    }
    assert!(
        bodies
            .admit(&need("/fixture", &["TEMP", "BAZEL_VC"]))
            .is_ok()
    );
    let sparse = make()
        .with_rules_cc_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe);
    assert!(sparse.rules_cc_import_sources && sparse.admit(&all).is_err());
    for names in [
        ["temp"],
        ["TEMP_suffix"],
        ["UNKNOWN"],
        [""],
        ["bazel_do_not_detect_cpp_toolchain"],
        ["BAZEL_DO_NOT_DETECT_CPP_TOOLCHAIN_suffix"],
    ] {
        let bad = need("/fixture", &names);
        for mixed in [all.try_union(&bad).unwrap(), bad.try_union(&all).unwrap()] {
            let denied = bodies.admit(&mixed).unwrap_err();
            assert_eq!(denied.family, "environment");
            assert!(std::ptr::eq(
                bodies
                    .environment_diagnostic(&mixed, &denied)
                    .unwrap()
                    .request,
                mixed.repository_environment().unwrap(),
            ));
        }
    }
    let mut too_many = RULES_CC_ENVIRONMENT_NAMES.to_vec();
    too_many.push("UNKNOWN");
    let too_many = need("/fixture", &too_many);
    assert_eq!(too_many.repository_environment().unwrap().names().len(), 15);
    assert!(bodies.admit(&too_many).is_err());
    let foreign = need("/foreign", &RULES_CC_ENVIRONMENT_NAMES);
    assert_eq!(bodies.admit(&foreign).unwrap_err().family, "environment");
    assert!(all.try_union(&foreign).is_err());

    let shell = need("/fixture", &RULES_SHELL_ENVIRONMENT_NAMES);
    assert_eq!(shell.repository_environment().unwrap().names().len(), 3);
    let both = all.try_union(&shell).unwrap();
    assert_eq!(both.repository_environment().unwrap().names().len(), 17);
    let combined = both.try_union(&apple).unwrap();
    assert_eq!(combined.repository_environment().unwrap().names().len(), 19);
    for cc_enabled in [false, true] {
        for shell_enabled in [false, true] {
            for apple_enabled in [false, true] {
                let mut candidate = selected();
                if cc_enabled {
                    candidate = candidate.with_rules_cc_import_sources(
                        RegistryChoice::PreparedFile,
                        CommandChoice::PublicProbe,
                    );
                }
                if shell_enabled {
                    candidate = candidate.with_rules_shell_import_sources(
                        RegistryChoice::PreparedFile,
                        CommandChoice::PublicProbe,
                    );
                }
                if apple_enabled {
                    candidate = candidate.with_apple_support_import_sources(
                        RegistryChoice::PreparedFile,
                        CommandChoice::PublicProbe,
                    );
                }
                assert_eq!(candidate.admit(&all).is_ok(), cc_enabled);
                assert_eq!(candidate.admit(&shell).is_ok(), shell_enabled);
                for name in RULES_SHELL_ENVIRONMENT_NAMES {
                    assert_eq!(
                        candidate.admit(&need("/fixture", &[name])).is_ok(),
                        shell_enabled
                    );
                }
                for mixed in [
                    all.try_union(&shell).unwrap(),
                    shell.try_union(&all).unwrap(),
                ] {
                    assert_eq!(candidate.admit(&mixed).is_ok(), cc_enabled && shell_enabled);
                    if !(cc_enabled && shell_enabled) {
                        let denied = candidate.admit(&mixed).unwrap_err();
                        assert_eq!(denied.family, "environment");
                        assert!(std::ptr::eq(
                            candidate
                                .environment_diagnostic(&mixed, &denied)
                                .unwrap()
                                .request,
                            mixed.repository_environment().unwrap()
                        ));
                    }
                }
                assert_eq!(candidate.admit(&apple).is_ok(), apple_enabled);
                for name in APPLE_SUPPORT_ENVIRONMENT_NAMES {
                    assert_eq!(
                        candidate.admit(&need("/fixture", &[name])).is_ok(),
                        apple_enabled
                    );
                }
                for (left, right, permitted) in [
                    (&all, &apple, cc_enabled && apple_enabled),
                    (&shell, &apple, shell_enabled && apple_enabled),
                    (&both, &apple, cc_enabled && shell_enabled && apple_enabled),
                ] {
                    for mixed in [
                        left.try_union(right).unwrap(),
                        right.try_union(left).unwrap(),
                    ] {
                        assert_eq!(candidate.admit(&mixed).is_ok(), permitted);
                        if !permitted {
                            let denied = candidate.admit(&mixed).unwrap_err();
                            assert_eq!(denied.family, "environment");
                            assert!(std::ptr::eq(
                                candidate
                                    .environment_diagnostic(&mixed, &denied)
                                    .unwrap()
                                    .request,
                                mixed.repository_environment().unwrap()
                            ));
                        }
                    }
                }
            }
        }
    }
    let mut both_bodies = selected()
        .with_rules_cc_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_rules_shell_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_apple_support_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        );
    for (slot, affected, unaffected) in [(2, &all, &shell), (4, &shell, &all)] {
        let module = both_bodies.additional[slot].take();
        assert!(both_bodies.admit(affected).is_err());
        assert!(both_bodies.admit(unaffected).is_ok());
        assert!(both_bodies.admit(&both).is_err());
        both_bodies.additional[slot] = module;
        assert!(both_bodies.admit(&both).is_ok());
    }
    let module = both_bodies.additional[5].take().unwrap();
    let displaced = both_bodies.additional[6].replace(module);
    assert!(both_bodies.admit(&apple).is_err());
    assert!(both_bodies.admit(&combined).is_err());
    assert!(both_bodies.admit(&both).is_ok());
    both_bodies.additional[5] = both_bodies.additional[6].take();
    both_bodies.additional[6] = displaced;
    assert!(both_bodies.admit(&combined).is_ok());
    let foreign_apple = need("/foreign", &APPLE_SUPPORT_ENVIRONMENT_NAMES);
    assert_eq!(
        both_bodies.admit(&foreign_apple).unwrap_err().family,
        "environment"
    );
    assert!(combined.try_union(&foreign_apple).is_err());
    for name in [
        "bazel_no_apple_cpp_toolchain",
        "BAZEL_NO_APPLE_CPP_TOOLCHAIN_suffix",
        "bazel_use_cpp_only_toolchain",
        "BAZEL_USE_CPP_ONLY_TOOLCHAIN_suffix",
        "bazel_sh",
        "BAZEL_SH_suffix",
        "path",
        "WINDIR_suffix",
        "UNKNOWN",
        "",
    ] {
        let bad = need("/fixture", &[name]);
        for authorized in [&shell, &both, &apple, &combined] {
            for mixed in [
                authorized.try_union(&bad).unwrap(),
                bad.try_union(authorized).unwrap(),
            ] {
                assert_eq!(
                    mixed.repository_environment().unwrap().names().len(),
                    authorized.repository_environment().unwrap().names().len() + 1
                );
                let denied = both_bodies.admit(&mixed).unwrap_err();
                assert_eq!(denied.family, "environment");
                assert!(std::ptr::eq(
                    both_bodies
                        .environment_diagnostic(&mixed, &denied)
                        .unwrap()
                        .request,
                    mixed.repository_environment().unwrap()
                ));
            }
        }
    }
    let foreign_shell = need("/foreign", &RULES_SHELL_ENVIRONMENT_NAMES);
    assert_eq!(
        both_bodies.admit(&foreign_shell).unwrap_err().family,
        "environment"
    );
    assert!(both.try_union(&foreign_shell).is_err());

    let local = SourcePreparationNeeds::repository(
        (*local_native_request(&bodies.roots[0], "unknown+", "/platforms")).clone(),
    );
    let original = local.repository_materializations().values().next().unwrap();
    let weak = Arc::downgrade(original);
    let excluded_path =
        SourcePreparationNeeds::path(NeedPathObservations::singleton(PathObservationDemand::new(
            PathObservationNamespace::Host,
            path("/fixture/unknown"),
            PathObservationOperation::FileBytes,
        )));
    let bootstrap =
        SourcePreparationNeeds::root_module_bootstrap(slug_bzlmod_v2::RootModuleBootstrapRequest {
            workspace: path("/foreign"),
        });
    let milestones = Arc::new(Mutex::new(Milestones::default()));
    let root = ModuleRoot {
        bodies: Arc::new(both_bodies),
        expected_registry: None,
        milestones: milestones.clone(),
    };
    let first_name = all
        .repository_environment()
        .unwrap()
        .names()
        .iter()
        .next()
        .unwrap()
        .as_ptr();
    let mut progress = 0;
    for environment in [&all, &both, &apple, &combined] {
        for (excluded, family) in [
            (&local, "materialization"),
            (&excluded_path, "path"),
            (&bootstrap, "bootstrap"),
        ] {
            for mixed in [
                environment.try_union(excluded).unwrap(),
                excluded.try_union(environment).unwrap(),
            ] {
                assert!(std::ptr::eq(
                    mixed
                        .repository_environment()
                        .unwrap()
                        .names()
                        .iter()
                        .next()
                        .unwrap()
                        .as_ptr(),
                    environment
                        .repository_environment()
                        .unwrap()
                        .names()
                        .iter()
                        .next()
                        .unwrap()
                        .as_ptr(),
                ));
                let denied = root.bodies.admit(&mixed).unwrap_err();
                assert_eq!(denied.family, family);
                assert!(
                    root.bodies
                        .environment_diagnostic(&mixed, &denied)
                        .is_none()
                );
                if family == "materialization" {
                    assert!(Arc::ptr_eq(
                        mixed.repository_materializations().values().next().unwrap(),
                        original,
                    ));
                }
                if let Ok(SourcePreparationOutcome::Need(_)) = root.need(mixed) {
                    progress += 1;
                }
                assert_eq!(
                    milestones.lock().unwrap().denied.as_ref().unwrap().family,
                    family
                );
            }
        }
    }
    assert_eq!(progress, 0);
    let unknown = need("/fixture", &["UNKNOWN"]);
    for mixed in [
        both.try_union(&unknown).unwrap().try_union(&local).unwrap(),
        local.try_union(&both).unwrap().try_union(&unknown).unwrap(),
    ] {
        let denied = root.bodies.admit(&mixed).unwrap_err();
        assert_eq!(denied.family, "environment");
        assert!(std::ptr::eq(
            root.bodies
                .environment_diagnostic(&mixed, &denied)
                .unwrap()
                .request,
            mixed.repository_environment().unwrap()
        ));
        assert!(Arc::ptr_eq(
            mixed.repository_materializations().values().next().unwrap(),
            original
        ));
        assert!(root.need(mixed).is_err());
        assert_eq!(
            milestones.lock().unwrap().denied.as_ref().unwrap().family,
            "environment"
        );
    }
    {
        let observed = milestones.lock().unwrap();
        assert_eq!((observed.calls, observed.preparations), (0, 0));
    }
    let combined_first = combined
        .repository_environment()
        .unwrap()
        .names()
        .iter()
        .next()
        .unwrap()
        .as_ptr();
    let SourcePreparationOutcome::Need(recovered_combined) = root.need(combined).unwrap() else {
        panic!("all three authorized role frontiers must preserve their original Need");
    };
    assert_eq!(
        recovered_combined
            .repository_environment()
            .unwrap()
            .names()
            .len(),
        19
    );
    assert!(std::ptr::eq(
        recovered_combined
            .repository_environment()
            .unwrap()
            .names()
            .iter()
            .next()
            .unwrap()
            .as_ptr(),
        combined_first
    ));
    drop(recovered_combined);
    let both_first = both
        .repository_environment()
        .unwrap()
        .names()
        .iter()
        .next()
        .unwrap()
        .as_ptr();
    let SourcePreparationOutcome::Need(recovered_both) = root.need(both).unwrap() else {
        panic!("both authorized role frontiers must preserve their original Need");
    };
    assert_eq!(
        recovered_both
            .repository_environment()
            .unwrap()
            .names()
            .len(),
        17
    );
    assert!(std::ptr::eq(
        recovered_both
            .repository_environment()
            .unwrap()
            .names()
            .iter()
            .next()
            .unwrap()
            .as_ptr(),
        both_first
    ));
    drop(recovered_both);
    let SourcePreparationOutcome::Need(recovered) = root.need(all).unwrap() else {
        panic!("authorized names must return their original Need for normal progress");
    };
    assert_eq!(
        recovered.repository_environment().unwrap().names().len(),
        14
    );
    assert!(std::ptr::eq(
        recovered
            .repository_environment()
            .unwrap()
            .names()
            .iter()
            .next()
            .unwrap()
            .as_ptr(),
        first_name,
    ));
    drop(recovered);
    drop(local);
    assert!(weak.upgrade().is_none());
    drop(root);
    assert_eq!(Arc::strong_count(&milestones), 1);
}

#[test]
fn environment_refusal_projection_is_bounded_and_borrowed() {
    use fmt::Write;
    use slug_bzlmod_v2::NeedRepositoryEnvironmentNames;
    use slug_bzlmod_v2::RepositoryEnvironmentNameFrontier;
    let path = |s: &str| NormalizedAbsolutePath::new(s).unwrap();
    let bodies = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")]);
    let make = |workspace: &str, names: &[&str]| {
        SourcePreparationNeeds::environment(
            NeedRepositoryEnvironmentNames::new(
                path(workspace),
                RepositoryEnvironmentNameFrontier::from_unsorted(
                    names
                        .iter()
                        .map(|name| compact_str::CompactString::new(name)),
                ),
            )
            .unwrap(),
        )
    };
    let left = make("/fixture", &["Z", "", "\0\né"]);
    let right = make("/fixture", &["A", "Z"]);
    let needs = left.try_union(&right).unwrap();
    let denied = bodies.admit(&needs).unwrap_err();
    let view = bodies.environment_diagnostic(&needs, &denied).unwrap();
    assert!(std::ptr::eq(
        view.request,
        needs.repository_environment().unwrap()
    ));
    assert_eq!(view.bounds(), (true, Some(6)));
    let encoded = view.to_string();
    assert_eq!(
        encoded,
        "WP1024_ENVIRONMENT capture=captured workspace_matches=true workspace_len=8 workspace_hex=2f66697874757265 names_count=4 names_bytes=6\nWP1024_ENVIRONMENT_NAME ordinal=1 name_len=0 name_hex=-\nWP1024_ENVIRONMENT_NAME ordinal=2 name_len=4 name_hex=000ac3a9\nWP1024_ENVIRONMENT_NAME ordinal=3 name_len=1 name_hex=41\nWP1024_ENVIRONMENT_NAME ordinal=4 name_len=1 name_hex=5a"
    );
    assert!(encoded.is_ascii() && !encoded.contains("value="));
    let foreign = make("/foreign", &["A"]);
    let foreign_denied = bodies.admit(&foreign).unwrap_err();
    assert!(
        bodies
            .environment_diagnostic(&foreign, &foreign_denied)
            .unwrap()
            .to_string()
            .contains("workspace_matches=false workspace_len=8 workspace_hex=2f666f726569676e")
    );
    let exact = "x".repeat(ENVIRONMENT_FIELD_BYTES);
    let too_long = "x".repeat(ENVIRONMENT_FIELD_BYTES + 1);
    for (name, captured) in [(&exact, true), (&too_long, false)] {
        let need = make("/fixture", &[name]);
        let denied = bodies.admit(&need).unwrap_err();
        let view = bodies.environment_diagnostic(&need, &denied).unwrap();
        assert_eq!(view.bounds(), (captured, Some(name.len())));
        let output = view.to_string();
        assert_eq!(output.contains("\nWP1024_ENVIRONMENT_NAME"), captured);
        if !captured {
            assert!(output.contains("workspace_hex=none names_count=1 names_bytes=1025"));
        }
    }
    let workspace = format!("/{}", "w".repeat(ENVIRONMENT_FIELD_BYTES - 1));
    let workspace_over = format!("{workspace}w");
    for (workspace, captured) in [(&workspace, true), (&workspace_over, false)] {
        let need = make(workspace, &[""]);
        let denied = bodies.admit(&need).unwrap_err();
        assert_eq!(
            bodies
                .environment_diagnostic(&need, &denied)
                .unwrap()
                .bounds(),
            (captured, Some(0))
        );
    }
    let names = (0..ENVIRONMENT_NAME_LIMIT + 1)
        .map(|i| format!("N{i:02}"))
        .collect::<Vec<_>>();
    for (count, captured) in [
        (ENVIRONMENT_NAME_LIMIT, true),
        (ENVIRONMENT_NAME_LIMIT + 1, false),
    ] {
        let borrowed = names[..count]
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let need = make("/fixture", &borrowed);
        let denied = bodies.admit(&need).unwrap_err();
        let view = bodies.environment_diagnostic(&need, &denied).unwrap();
        assert_eq!(view.bounds(), (captured, captured.then_some(count * 3)));
        let output = view.to_string();
        assert_eq!(output.lines().count(), if captured { count + 1 } else { 1 });
        if !captured {
            assert!(output.ends_with("names_count=33 names_bytes=none"));
        }
    }
    // Four distinct fields exactly fill the aggregate after the original workspace.
    let a = "a".repeat(1024);
    let b = "b".repeat(1024);
    let c = "c".repeat(1024);
    let d = "d".repeat(1016);
    let over = "d".repeat(1017);
    for (last, captured) in [(&d, true), (&over, false)] {
        let need = make("/fixture", &[&a, &b, &c, last]);
        let denied = bodies.admit(&need).unwrap_err();
        let view = bodies.environment_diagnostic(&need, &denied).unwrap();
        assert_eq!(view.bounds(), (captured, Some(3072 + last.len())));
        let output = view.to_string();
        assert!(output.len() + 1 <= ENVIRONMENT_MARKER_BYTES);
        assert_eq!(output.lines().count(), if captured { 5 } else { 1 });
    }
    // Maximum row count and aggregate together exercise actual encoded framing.
    let maximum = (0..ENVIRONMENT_NAME_LIMIT)
        .map(|i| format!("{i:02}{}", "x".repeat(if i < 24 { 126 } else { 125 })))
        .collect::<Vec<_>>();
    let borrowed = maximum.iter().map(String::as_str).collect::<Vec<_>>();
    let need = make("/fixture", &borrowed);
    let denied = bodies.admit(&need).unwrap_err();
    let view = bodies.environment_diagnostic(&need, &denied).unwrap();
    assert_eq!(view.bounds(), (true, Some(ENVIRONMENT_AGGREGATE_BYTES - 8)));
    let maximum_encoded = view.to_string();
    assert_eq!(maximum_encoded.lines().count(), ENVIRONMENT_NAME_LIMIT + 1);
    assert_eq!(maximum_encoded.len() + 1, 10161);
    assert!(maximum_encoded.len() + 1 <= ENVIRONMENT_MARKER_BYTES);
    let mut output = String::new();
    let mut writer = MaterializationWriter {
        writer: &mut output,
        bytes: ENVIRONMENT_MARKER_BYTES - 2,
        limit: ENVIRONMENT_MARKER_BYTES,
    };
    writer.write_str("x").unwrap(); // The final println LF fits exactly.
    assert!(writer.write_str("y").is_err());
    assert_eq!(output, "x");
    let path_need =
        SourcePreparationNeeds::path(NeedPathObservations::singleton(PathObservationDemand::new(
            PathObservationNamespace::Host,
            path("/closed"),
            PathObservationOperation::FileBytes,
        )));
    let other = bodies.admit(&path_need).unwrap_err();
    assert!(bodies.environment_diagnostic(&path_need, &other).is_none());
    assert!(
        NeedRepositoryEnvironmentNames::new(
            path("/fixture"),
            RepositoryEnvironmentNameFrontier::empty()
        )
        .is_err()
    );
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

#[test]
fn immutable_spec_projection_preserves_original_typed_borrowed_bounds() {
    use fmt::Write;
    use slug_bzlmod_v2::OverrideAttributeKey as K;
    use slug_bzlmod_v2::OverrideAttributeValue as V;
    use starlark_map::small_map::SmallMap;
    let path = |s: &str| NormalizedAbsolutePath::new(s).unwrap();
    let bodies = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")]);
    let request = |attributes: SmallMap<compact_str::CompactString, V>| {
        let mut request =
            (*local_native_request(&bodies.roots[0], "protobuf+", "/platforms")).clone();
        request.kind = RepositoryMaterializationKind::Immutable;
        request.repo_spec.rule_id.bzl_file =
            CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:http.bzl").unwrap();
        request.repo_spec.rule_id.rule_name = "http_archive".into();
        request.repo_spec.attributes = Arc::new(attributes);
        request
    };
    let singleton = |value| SmallMap::from_iter([(compact_str::CompactString::new("a"), value)]);
    let mapped_label = |id: &str, spelling: &str| {
        let mut mapping = slug_identity_v2::RepositoryMapping::new(
            slug_identity_v2::RepositoryMappingId::new(id).unwrap(),
        );
        mapping.insert(
            slug_identity_v2::ApparentRepoName::new("r").unwrap(),
            slug_identity_v2::CanonicalRepoName::new("r").unwrap(),
        );
        slug_identity_v2::ApparentLabel::parse(spelling)
            .unwrap()
            .resolve(&mapping)
    };
    let label = CanonicalLabel::parse("@@r//p:t").unwrap();
    let mapped = mapped_label("mapped", "@r//p:t");
    let map = SmallMap::from_iter([
        (K::String("@@r//p:t".into()), V::None),
        (K::Label(label), V::Bool(false)),
        (K::Label(mapped), V::Int(i32::MAX)),
        (K::String("".into()), V::String("".into())),
    ]);
    let attributes = SmallMap::from_iter([
        (compact_str::CompactString::new("none"), V::None),
        (compact_str::CompactString::new("bool"), V::Bool(true)),
        (compact_str::CompactString::new("int"), V::Int(i32::MIN)),
        (
            compact_str::CompactString::new("string"),
            V::String("\0\né".into()),
        ),
        (
            compact_str::CompactString::new("label"),
            V::Label(mapped_label("other", "@r//p:t")),
        ),
        (
            compact_str::CompactString::new("iter"),
            V::Iterable(Arc::from([
                V::String("".into()),
                V::Iterable(Arc::from([])),
            ])),
        ),
        (
            compact_str::CompactString::new("map"),
            V::Map(Arc::new(map)),
        ),
    ]);
    let needs = SourcePreparationNeeds::repository(request(attributes));
    let original = needs.repository_materializations().values().next().unwrap();
    let weak = Arc::downgrade(original);
    let attributes_weak = Arc::downgrade(&original.repo_spec.attributes);
    assert_eq!(Arc::strong_count(original), 1);
    assert_eq!(Arc::strong_count(&original.repo_spec.attributes), 1);
    {
        let denied = bodies.admit(&needs).unwrap_err();
        let view = bodies.immutable_spec_diagnostic(&needs, &denied).unwrap();
        assert!(std::ptr::eq(view.request, original.as_ref()));
        assert_eq!(
            bodies.first_denied_materialization(&needs).unwrap().1,
            "immutable_kind"
        );
        assert!(Arc::ptr_eq(
            &view.request.repo_spec.attributes,
            &original.repo_spec.attributes
        ));
        for ((name, value), (held_name, held_value)) in original
            .repo_spec
            .attributes
            .iter()
            .zip(view.request.repo_spec.attributes.iter())
        {
            assert!(std::ptr::eq(
                name.as_bytes().as_ptr(),
                held_name.as_bytes().as_ptr()
            ));
            match (value, held_value) {
                (V::String(a), V::String(b)) => {
                    assert!(std::ptr::eq(a.as_bytes().as_ptr(), b.as_bytes().as_ptr()))
                }
                (V::Label(a), V::Label(b)) => {
                    for (a, b) in [
                        (a.package().repo().as_str(), b.package().repo().as_str()),
                        (
                            a.package().package().as_str(),
                            b.package().package().as_str(),
                        ),
                        (a.target().as_str(), b.target().as_str()),
                        (
                            a.mapping_id().unwrap().as_str(),
                            b.mapping_id().unwrap().as_str(),
                        ),
                    ] {
                        assert!(std::ptr::eq(a.as_bytes().as_ptr(), b.as_bytes().as_ptr()));
                    }
                }
                (V::Iterable(a), V::Iterable(b)) => assert!(Arc::ptr_eq(a, b)),
                (V::Map(a), V::Map(b)) => assert!(Arc::ptr_eq(a, b)),
                _ => {}
            }
        }
        assert_eq!(Arc::strong_count(original), 1);
        assert_eq!(Arc::strong_count(&original.repo_spec.attributes), 1);
        let encoded = view.to_string();
        assert!(encoded.starts_with("WP1079_IMMUTABLE_SPEC {\"capture\":\"captured\",\"kind\":\"immutable\",\"reason\":\"immutable_kind\",\"identity\":["));
        assert!(encoded.contains("\"rule_mapping\":null,\"attributes\":["));
        for fragment in [
            "[\"n\"]",
            "[\"b\",true]",
            "[\"b\",false]",
            "[\"i\",-2147483648]",
            "[\"i\",2147483647]",
            "[\"s\",[4,\"000ac3a9\"]]",
            "[\"s\",[0,\"\"]]",
            "[\"v\",[]]",
            "[\"l\",[1,\"72\"],[1,\"70\"],[1,\"74\"],null]",
            "[\"l\",[1,\"72\"],[1,\"70\"],[1,\"74\"],[6,\"6d6170706564\"]]",
        ] {
            assert!(encoded.contains(fragment), "missing {fragment}");
        }
        assert!(
            encoded.find("[\"s\",[8,\"4040722f2f703a74\"]]").unwrap()
                < encoded
                    .find("[\"l\",[1,\"72\"],[1,\"70\"],[1,\"74\"],null]")
                    .unwrap()
        );
        assert_eq!(Arc::strong_count(original), 1);
        assert_eq!(Arc::strong_count(&original.repo_spec.attributes), 1);
        assert!(encoded.len() + 1 <= MATERIALIZATION_MARKER_BYTES);
        println!(
            "WP1079_IMMUTABLE_SPEC_LAYOUT view_bytes={} view_align={} writer_bytes={} writer_align={} identity_bytes={} identity_align={} bodies_bytes={} bodies_align={}",
            std::mem::size_of::<ImmutableSpecDiagnostic<'_>>(),
            std::mem::align_of::<ImmutableSpecDiagnostic<'_>>(),
            std::mem::size_of::<MaterializationWriter<'_, String>>(),
            std::mem::align_of::<MaterializationWriter<'_, String>>(),
            std::mem::size_of::<MaterializationDiagnostic<'_>>(),
            std::mem::align_of::<MaterializationDiagnostic<'_>>(),
            std::mem::size_of::<Bodies>(),
            std::mem::align_of::<Bodies>()
        );
    }
    drop(needs);
    assert!(weak.upgrade().is_none());
    assert!(attributes_weak.upgrade().is_none());
    let encode = |request| ImmutableSpecDiagnostic { request: &request }.to_string();
    let refused = "WP1079_IMMUTABLE_SPEC {\"capture\":\"bounds_refused\",\"kind\":\"immutable\",\"reason\":\"immutable_kind\",\"identity\":null,\"rule_mapping\":null,\"attributes\":null}";
    let captured = |text: &str| text.starts_with("WP1079_IMMUTABLE_SPEC {\"capture\":\"captured\"");
    assert!(encode(request(SmallMap::new())).ends_with("\"attributes\":[]}"));
    assert!(encode(request(singleton(V::Map(Arc::new(SmallMap::new()))))).contains("[\"m\",[]]"));
    for (length, expected) in [(32, true), (33, false)] {
        let attrs = (0..length)
            .map(|n| (compact_str::CompactString::new(format!("a{n}")), V::None))
            .collect();
        assert_eq!(captured(&encode(request(attrs))), expected);
    }
    for (length, expected) in [(127, true), (128, false)] {
        let values: Arc<[V]> = (0..length).map(|_| V::None).collect();
        assert_eq!(
            captured(&encode(request(singleton(V::Iterable(values))))),
            expected
        );
    }
    for (pairs, expected) in [(63, true), (64, false)] {
        let map = (0..pairs)
            .map(|n| (K::String(format!("k{n}").into()), V::None))
            .collect();
        let mut attrs = singleton(V::Map(Arc::new(map)));
        attrs.insert("b".into(), V::None); // 1 map + 2 nodes/key-value + 1 second attr.
        assert_eq!(captured(&encode(request(attrs))), expected);
    }
    for (depth, expected) in [(8, true), (9, false)] {
        let mut value = V::None;
        for _ in 1..depth {
            value = V::Iterable(Arc::from([value]));
        }
        assert_eq!(captured(&encode(request(singleton(value)))), expected);
    }
    for (depth, expected) in [(7, true), (8, false)] {
        let mut value = V::Map(Arc::new(SmallMap::from_iter([(
            K::String("k".into()),
            V::None,
        )])));
        for _ in 1..depth {
            value = V::Iterable(Arc::from([value]));
        }
        assert_eq!(captured(&encode(request(singleton(value)))), expected);
    }
    let mut mapped_owner = request(SmallMap::new());
    mapped_owner.repo_spec.rule_id.bzl_file = mapped_label("mapped", "@r//p:t");
    let view = ImmutableSpecDiagnostic {
        request: &mapped_owner,
    };
    assert!(std::ptr::eq(
        view.request
            .repo_spec
            .rule_id
            .bzl_file
            .mapping_id()
            .unwrap()
            .as_str()
            .as_ptr(),
        mapped_owner
            .repo_spec
            .rule_id
            .bzl_file
            .mapping_id()
            .unwrap()
            .as_str()
            .as_ptr()
    ));
    assert!(
        view.to_string()
            .contains("\"rule_mapping\":[6,\"6d6170706564\"]")
    );
    for (length, expected) in [(1024, true), (1025, false)] {
        let text = encode(request(singleton(V::String("x".repeat(length).into()))));
        assert_eq!(captured(&text), expected);
        if !expected {
            assert_eq!(text, refused);
        }
        let mut owner = request(SmallMap::new());
        owner.repo_spec.rule_id.bzl_file = mapped_label(&"m".repeat(length), "@r//p:t");
        assert_eq!(captured(&encode(owner)), expected);
    }
    let owner_raw = MaterializationDiagnostic::new(&request(SmallMap::new()), "immutable_kind")
        .fields
        .iter()
        .flatten()
        .map(|field| field.len())
        .sum::<usize>();
    for (raw, expected) in [(4096, true), (4097, false)] {
        let lengths = [1024, 1024, 1024, raw - owner_raw - 4 - 3072];
        let attrs = ["a", "b", "c", "d"]
            .into_iter()
            .zip(lengths)
            .map(|(name, length)| {
                (
                    compact_str::CompactString::new(name),
                    V::String("x".repeat(length).into()),
                )
            })
            .collect();
        let text = encode(request(attrs));
        assert_eq!(captured(&text), expected);
        if !expected {
            assert_eq!(text, refused);
        }
    }
    assert_eq!(
        encode(request(SmallMap::from_iter([
            (
                compact_str::CompactString::new("first"),
                V::String("visible".into())
            ),
            (
                compact_str::CompactString::new("last"),
                V::String("x".repeat(1025).into())
            ),
        ]))),
        refused
    );
    let label = mapped_label(&"m".repeat(28), "@r//p:t");
    // 128 nodes/depth2/raw4007, all fields<=28; only encoded marker cap refuses.
    assert!(owner_raw + 1 + 127 * (3 + 28) <= MATERIALIZATION_AGGREGATE_BYTES);
    let values: Arc<[V]> = (0..127).map(|_| V::Label(label.clone())).collect();
    assert_eq!(encode(request(singleton(V::Iterable(values)))), refused);
    let ordered = |reverse: bool| {
        let mut entries = [
            (K::String("z".into()), V::None),
            (K::String("a".into()), V::None),
        ];
        if reverse {
            entries.reverse();
        }
        encode(request(singleton(V::Map(Arc::new(SmallMap::from_iter(
            entries,
        ))))))
    };
    assert_ne!(ordered(false), ordered(true));
    let mut output = String::new();
    let mut writer = MaterializationWriter {
        writer: &mut output,
        bytes: MATERIALIZATION_MARKER_BYTES - 2,
        limit: MATERIALIZATION_MARKER_BYTES,
    };
    assert!(writer.write_str("x").is_ok());
    assert!(writer.write_str("x").is_err());
    assert_eq!(output, "x");
    let allowed = SourcePreparationNeeds::repository(
        (*local_native_request(&bodies.roots[0], "platforms+", "/platforms")).clone(),
    );
    let first = SourcePreparationNeeds::repository(request(singleton(V::String("first".into()))));
    let mut second = request(singleton(V::String("second".into())));
    second.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new("other+").unwrap();
    let second = SourcePreparationNeeds::repository(second);
    let body =
        SourcePreparationNeeds::path(NeedPathObservations::singleton(PathObservationDemand::new(
            PathObservationNamespace::Host,
            path("/fixture/MODULE.bazel"),
            PathObservationOperation::FileBytes,
        )));
    for (a, b) in [(&first, &second), (&second, &first)] {
        let mixed = allowed
            .try_union(a)
            .unwrap()
            .try_union(b)
            .unwrap()
            .try_union(&body)
            .unwrap();
        let offender = mixed
            .repository_materializations()
            .values()
            .find(|request| matches!(&request.kind, RepositoryMaterializationKind::Immutable))
            .unwrap();
        let denied = bodies.admit(&mixed).unwrap_err();
        let view = bodies.immutable_spec_diagnostic(&mixed, &denied).unwrap();
        assert!(std::ptr::eq(view.request, offender.as_ref()));
        assert_eq!(denied.family, "materialization");
        let mut progress = 0;
        if bodies.admit(&mixed).is_ok() {
            progress += 1;
        }
        assert_eq!(progress, 0);
        assert_eq!(
            view.to_string(),
            ImmutableSpecDiagnostic { request: offender }.to_string()
        );
    }
    for kind in [
        RepositoryMaterializationKind::Local {
            logical_root: path("/platforms"),
        },
        RepositoryMaterializationKind::GeneratedFileEffects(
            slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan::build([]).unwrap(),
        ),
    ] {
        let mut other = (*local_native_request(&bodies.roots[0], "unknown+", "/platforms")).clone();
        other.kind = kind;
        let mixed = SourcePreparationNeeds::repository(other)
            .try_union(&first)
            .unwrap();
        let denied = bodies.admit(&mixed).unwrap_err();
        assert!(bodies.immutable_spec_diagnostic(&mixed, &denied).is_none());
    }
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
    for owner in [0, 2, 3, 4, 5, 6, 7] {
        let bodies = namespace_test_bodies()
            .with_repo_marker_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
            .with_ignore_file_bytes(RegistryChoice::PreparedFile, CommandChoice::PublicProbe);
        let bodies = if owner == 6 {
            bodies
                .with_seventh(NormalizedAbsolutePath::new("/license").unwrap())
                .with_eighth(NormalizedAbsolutePath::new("/shell").unwrap())
                .with_rules_shell_import_sources(
                    RegistryChoice::PreparedFile,
                    CommandChoice::PublicProbe,
                )
        } else {
            bodies
        };
        let bodies = if owner == 7 {
            bodies
                .with_seventh(NormalizedAbsolutePath::new("/license").unwrap())
                .with_eighth(NormalizedAbsolutePath::new("/shell").unwrap())
                .with_ninth(NormalizedAbsolutePath::new("/apple").unwrap())
                .with_apple_support_import_sources(
                    RegistryChoice::PreparedFile,
                    CommandChoice::PublicProbe,
                )
        } else {
            bodies
        };
        let request = if matches!(owner, 5 | 6 | 7) {
            let (canonical, label, rule) = if owner == 7 {
                (
                    APPLE_GENERATED_DIAGNOSTIC_OWNER.0,
                    "@@apple_support+//crosstool:setup_internal.bzl",
                    APPLE_GENERATED_DIAGNOSTIC_OWNER.4,
                )
            } else if owner == 6 {
                (
                    "rules_shell++sh_configure+local_config_shell",
                    "@@rules_shell+//shell/private/repositories:sh_config.bzl",
                    "sh_config",
                )
            } else {
                GENERATED_CC_TOOLCHAINS_DIAGNOSTIC_TEST_OWNER
            };
            let mut request =
                generated_globals_request(&bodies, small_generated_plan(&[("BUILD", b"a", true)]));
            request.id.canonical_repo =
                slug_identity_v2::CanonicalRepoName::new(canonical).unwrap();
            request.repo_spec.rule_id.bzl_file = CanonicalLabel::parse(label).unwrap();
            request.repo_spec.rule_id.rule_name = rule.into();
            request.repo_spec.attributes = Arc::new(starlark_map::small_map::SmallMap::new());
            request
        } else {
            generated_test_owner_request(
                &bodies,
                owner,
                small_generated_plan(&[
                    (small_owner_first(owner), b"a", true),
                    (small_owner_second(owner), b"b", true),
                ]),
            )
        };
        let a = Arc::new(request);
        let mut b = a.as_ref().clone();
        b.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new("other+").unwrap();
        let b = Arc::new(b);
        let dice = dice::Dice::builder().build(dice::DetectCycles::Enabled);
        let instance = slug_workspace_v2::PathObservationInstanceId::new(17);
        let demand = PathObservationDemand::new(
            PathObservationNamespace::Materialization(instance),
            NormalizedAbsolutePath::new(if owner == 2 {
                "/generation/constraints.bzl"
            } else if owner == 3 {
                "/generation/BUILD.bazel"
            } else if matches!(owner, 5 | 6 | 7) {
                "/generation/BUILD"
            } else if owner == 4 {
                "/generation/toolchains.bzl"
            } else {
                "/generation/globals.bzl"
            })
            .unwrap(),
            PathObservationOperation::FileBytes,
        );
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
            assert_eq!(
                namespace_policy(
                    &bodies,
                    &demand,
                    Some(&current),
                    &small_namespace_plan_matches
                )
                .admits(&demand.operation()),
                index != 1
            );
            for path in ["/generation/REPO.bazel", "/generation/.bazelignore"] {
                let marker = PathObservationDemand::new(
                    PathObservationNamespace::Materialization(instance),
                    NormalizedAbsolutePath::new(path).unwrap(),
                    PathObservationOperation::Lstat,
                );
                assert_eq!(
                    namespace_policy(
                        &bodies,
                        &marker,
                        Some(&current),
                        &small_namespace_plan_matches
                    )
                    .admits(&marker.operation()),
                    matches!(owner, 3 | 4 | 5 | 6 | 7) && index != 1
                );
            }
            if matches!(owner, 4 | 5 | 6 | 7) {
                let candidate = PathObservationDemand::new(
                    PathObservationNamespace::Materialization(instance),
                    NormalizedAbsolutePath::new("/generation/BUILD.bazel").unwrap(),
                    PathObservationOperation::Lstat,
                );
                assert_eq!(
                    namespace_policy(
                        &bodies,
                        &candidate,
                        Some(&current),
                        &small_namespace_plan_matches
                    )
                    .admits(&candidate.operation()),
                    matches!(owner, 4 | 5 | 6 | 7) && index != 1
                );
            }
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
#[cfg(target_os = "linux")]
const JAVA_PROXY_LITERAL_0: &[u8] = br###"
load("@bazel_skylib//:bzl_library.bzl", "bzl_library")
exports_files(['proxy.bzl'], visibility = ["@rules_java//test:__pkg__"])
bzl_library(
    name = "proxy_bzl",
    srcs = ["proxy.bzl"],
    deps = [
        "@rules_java//java/bazel/rules",
        "@rules_java//java/common/rules:toolchain_rules",
        "@rules_java//java/private:internals",
        "@rules_java//java/bazel:http_jar_bzl",
    ],
    visibility = ["//visibility:public"]
)
            "###;

#[cfg(target_os = "linux")]
const JAVA_PROXY_LITERAL_1: &[u8] = br###"
load("@rules_java//java/bazel/rules:bazel_java_binary_wrapper.bzl", _java_binary = "java_binary")
load("@rules_java//java/bazel/rules:bazel_java_import.bzl", _java_import = "java_import")
load("@rules_java//java/bazel/rules:bazel_java_library.bzl", _java_library = "java_library")
load("@rules_java//java/bazel/rules:bazel_java_plugin.bzl", _java_plugin = "java_plugin")
load("@rules_java//java/bazel/rules:bazel_java_test.bzl", _java_test = "java_test")
load("@rules_java//java/bazel:http_jar.bzl", _http_jar = "http_jar")
load("@rules_java//java/common/rules:java_package_configuration.bzl", _java_package_configuration = "java_package_configuration")
load("@rules_java//java/common/rules:java_runtime.bzl", _java_runtime = "java_runtime")
load("@rules_java//java/common/rules:java_toolchain.bzl", _java_toolchain = "java_toolchain")
load("@rules_java//java/private:java_common.bzl", _java_common = "java_common")
load("@rules_java//java/private:java_common_internal.bzl", _java_common_internal_compile = "compile")
load("@rules_java//java/private:java_info.bzl", _JavaInfo = "JavaInfo", _JavaPluginInfo = "JavaPluginInfo",
  _java_info_internal_merge = "merge", _java_info_to_implicit_exportable = "to_implicit_exportable")

java_binary = _java_binary
java_import = _java_import
java_library = _java_library
java_plugin = _java_plugin
java_test = _java_test
java_package_configuration = _java_package_configuration
java_runtime = _java_runtime
java_toolchain = _java_toolchain
java_common = _java_common
JavaInfo = _JavaInfo
JavaPluginInfo = _JavaPluginInfo
java_common_internal_compile = _java_common_internal_compile
java_info_internal_merge = _java_info_internal_merge
java_info_to_implicit_exportable = _java_info_to_implicit_exportable
http_jar = _http_jar
            "###;

#[cfg(target_os = "linux")]
fn java_proxy_literal_rows() -> [(&'static str, &'static [u8], bool); 2] {
    [
        ("BUILD.bazel", JAVA_PROXY_LITERAL_0, true),
        ("proxy.bzl", JAVA_PROXY_LITERAL_1, true),
    ]
}
#[cfg(target_os = "linux")]
fn java_proxy_test_bodies() -> Bodies {
    use immutable_archive_preparation::ArchiveBinding;
    use immutable_archive_preparation::JAVA_DIGEST;
    use immutable_archive_preparation::JAVA_PREPARED_MODULE;
    use immutable_archive_preparation::JAVA_URL;
    let project = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let mut bodies = namespace_test_bodies();
    bodies.java_offline_archive = Some(ArchiveBinding {
        backing: OfflineArchiveBacking::new(
            JAVA_URL,
            JAVA_DIGEST,
            NormalizedAbsolutePath::new(
                project
                    .join("tests/v2_oracle/fixtures/configured-cli-authentic/mirror")
                    .join(JAVA_URL.strip_prefix("https://").unwrap()),
            )
            .unwrap(),
        ),
        module: NormalizedAbsolutePath::new(project.join(JAVA_PREPARED_MODULE)).unwrap(),
    });
    bodies
}
#[cfg(target_os = "linux")]
#[test]
fn java_environment_frontier_is_qualified_exact_and_observes_empty_snapshot() {
    use immutable_archive_preparation::JAVA_DIGEST;
    use immutable_archive_preparation::JAVA_URL;
    use immutable_archive_preparation::PROTOBUF_URL;
    use slug_bzlmod_v2::NeedRepositoryEnvironmentNames;
    use slug_bzlmod_v2::RepositoryEnvironmentCell;
    use slug_bzlmod_v2::RepositoryEnvironmentCellKey;
    use slug_bzlmod_v2::RepositoryEnvironmentNameFrontier;
    use slug_bzlmod_v2::RepositoryEnvironmentSnapshotKey;
    use slug_bzlmod_v2::RepositoryHostInputTransaction;
    let path = |s: &str| NormalizedAbsolutePath::new(s).unwrap();
    let selected = || {
        let mut bodies = java_proxy_test_bodies().with_rules_rust_defs_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        );
        bodies.rules_cc_import_sources = false;
        bodies
    };
    let need = |workspace: &NormalizedAbsolutePath, names: &[&str]| {
        SourcePreparationNeeds::environment(
            NeedRepositoryEnvironmentNames::new(
                workspace.dupe(),
                RepositoryEnvironmentNameFrontier::from_unsorted(
                    names.iter().copied().map(Into::into),
                ),
            )
            .unwrap(),
        )
    };
    let bodies = selected();
    let java = need(&bodies.roots[0], &RULES_JAVA_ENVIRONMENT_NAMES);
    assert!(bodies.admit(&java).is_ok());
    assert_eq!(
        RULES_CC_ENVIRONMENT_NAMES.len()
            + RULES_SHELL_ENVIRONMENT_NAMES.len()
            + APPLE_SUPPORT_ENVIRONMENT_NAMES.len()
            + RULES_JAVA_ENVIRONMENT_NAMES.len(),
        20
    );
    eprintln!(
        "WP1093_JAVA_ENVIRONMENT_LAYOUT table={} row={} literal={} bodies={}/{}",
        std::mem::size_of_val(&RULES_JAVA_ENVIRONMENT_NAMES),
        std::mem::size_of::<&str>(),
        RULES_JAVA_ENVIRONMENT_NAMES[0].len(),
        std::mem::size_of::<Bodies>(),
        std::mem::align_of::<Bodies>()
    );
    for registry in [RegistryChoice::DefaultBcr, RegistryChoice::PreparedFile] {
        for command in [CommandChoice::Modules, CommandChoice::PublicProbe] {
            let candidate =
                java_proxy_test_bodies().with_rules_rust_defs_import_sources(registry, command);
            assert_eq!(
                candidate.admit(&java).is_ok(),
                matches!(registry, RegistryChoice::PreparedFile)
                    && matches!(command, CommandChoice::PublicProbe)
            );
        }
    }
    let default = Bodies::new(bodies.roots.clone());
    assert_eq!(default.admit(&java).unwrap_err().family, "environment");
    for case in 0..6 {
        let mut closed = selected();
        match case {
            0 => closed.java_offline_archive = None,
            5 => closed.rules_rust_defs_import_sources = false,
            _ => {
                let binding = closed.java_offline_archive.as_mut().unwrap();
                match case {
                    1 => {
                        binding.backing = OfflineArchiveBacking::new(
                            PROTOBUF_URL,
                            JAVA_DIGEST,
                            binding.backing.path().dupe(),
                        )
                    }
                    2 => {
                        binding.backing = OfflineArchiveBacking::new(
                            JAVA_URL,
                            [0; 32],
                            binding.backing.path().dupe(),
                        )
                    }
                    3 => {
                        binding.backing = OfflineArchiveBacking::new(
                            JAVA_URL,
                            JAVA_DIGEST,
                            path("/foreign/java.tar.gz"),
                        )
                    }
                    _ => binding.module = path("/foreign/MODULE.bazel"),
                }
            }
        }
        let denied = closed.admit(&java).unwrap_err();
        assert_eq!(denied.family, "environment");
        assert!(std::ptr::eq(
            closed
                .environment_diagnostic(&java, &denied)
                .unwrap()
                .request,
            java.repository_environment().unwrap()
        ));
    }
    assert_eq!(
        bodies
            .admit(&need(&path("/foreign"), &RULES_JAVA_ENVIRONMENT_NAMES))
            .unwrap_err()
            .family,
        "environment"
    );
    for name in [
        "java_home",
        "JAVA_HOME_suffix",
        "UNKNOWN",
        "TEMP",
        "PATH",
        "",
    ] {
        let bad = need(&bodies.roots[0], &[name]);
        for mixed in [java.try_union(&bad).unwrap(), bad.try_union(&java).unwrap()] {
            let denied = bodies.admit(&mixed).unwrap_err();
            assert_eq!(denied.family, "environment");
            assert!(denied.path.is_none());
            assert!(std::ptr::eq(
                bodies
                    .environment_diagnostic(&mixed, &denied)
                    .unwrap()
                    .request,
                mixed.repository_environment().unwrap()
            ));
        }
    }
    // Another cohort stays independent; enabling Java cannot grant its names.
    let cc = selected()
        .with_sixth(path("/cc"))
        .with_rules_cc_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe);
    let cc_need = need(&cc.roots[0], &RULES_CC_ENVIRONMENT_NAMES);
    assert!(cc.admit(&java.try_union(&cc_need).unwrap()).is_ok());
    let mut no_java = cc;
    no_java.java_offline_archive = None;
    assert!(no_java.admit(&cc_need).is_ok());
    assert_eq!(no_java.admit(&java).unwrap_err().family, "environment");

    // Exercise the normal command/frontier owner with the accepted empty snapshot.
    let workspace = tempfile::tempdir().unwrap();
    let runtime = test_runtime(workspace.path()).unwrap();
    let normalized = NormalizedAbsolutePath::new(runtime.workspace.clone()).unwrap();
    let accepted = accepted_native_snapshot(&runtime);
    assert!(accepted.inputs.request.repository_environment.is_empty());
    let mut current = selected();
    current.roots[0] = normalized.dupe();
    let java = need(&normalized, &RULES_JAVA_ENVIRONMENT_NAMES);
    current.admit(&java).unwrap();
    let mut guard = NativeDemandAbortGuard::new(
        runtime
            .begin_native_demand_command_with_inputs(
                NativeDemandRequestInputBundle::normalized_initial(),
            )
            .unwrap()
            .into_command(),
    );
    let bad = need(&normalized, &["UNKNOWN"]);
    for mixed in [java.try_union(&bad).unwrap(), bad.try_union(&java).unwrap()] {
        let denied = current.admit(&mixed).unwrap_err();
        assert!(std::ptr::eq(
            current
                .environment_diagnostic(&mixed, &denied)
                .unwrap()
                .request,
            mixed.repository_environment().unwrap()
        ));
        assert_eq!(
            guard.command().repository_environment_frontier,
            accepted.repository_environment_frontier
        );
        assert!(
            !guard
                .command()
                .repository_environment_frontier
                .contains(RULES_JAVA_ENVIRONMENT_NAMES[0])
        );
    }
    assert_eq!(
        guard.progress(&java).unwrap(),
        NativeDemandProgress::Environment
    );
    assert!(matches!(
        guard.progress(&java),
        Err(NativeDemandSessionError::EnvironmentInternalNonProgress)
    ));
    guard.begin_attempt().unwrap();
    let cell =
        RepositoryEnvironmentCellKey::new(normalized.dupe(), RULES_JAVA_ENVIRONMENT_NAMES[0]);
    runtime.runtime.block_on(async {
        let data = guard.attempt_user_computation_data().unwrap();
        let carrier = data.data.get::<RepositoryHostInputTransaction>().unwrap();
        assert!(carrier.snapshot().is_empty());
        assert!(carrier.frontier().contains(RULES_JAVA_ENVIRONMENT_NAMES[0]));
        let mut updater = runtime.dice.updater_with_data(data);
        guard.inject_attempt(&mut updater).unwrap();
        let mut tx = updater.commit().await;
        runtime.request_revision.bind_cycle_detector(&tx);
        assert_eq!(
            tx.compute(&cell).await.unwrap(),
            RepositoryEnvironmentCell::observed(None)
        );
        assert!(
            tx.compute(&RepositoryEnvironmentSnapshotKey::new(normalized.dupe()))
                .await
                .unwrap()
                .is_empty()
        );
    });
    let restored = guard.discard().unwrap();
    assert!(restored.snapshot().is_empty());
    assert!(
        !restored
            .frontier()
            .contains(RULES_JAVA_ENVIRONMENT_NAMES[0])
    );
    assert_current_native_snapshot(&runtime, &accepted);
    runtime.runtime.block_on(async {
        let data = runtime.user_computation_data(None).unwrap();
        let mut tx = runtime.dice.updater_with_data(data).existing_state().await;
        runtime.request_revision.bind_cycle_detector(&tx);
        assert_eq!(
            tx.compute(&cell).await.unwrap(),
            RepositoryEnvironmentCell::Unauthorized
        );
        assert!(
            tx.compute(&RepositoryEnvironmentSnapshotKey::new(normalized))
                .await
                .unwrap()
                .is_empty()
        );
    });
}
#[cfg(target_os = "linux")]
fn java_proxy_generated_request(
    bodies: &Bodies,
    plan: slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan,
) -> slug_bzlmod_v2::RepositoryMaterializationRequest {
    let mut request = generated_globals_request(bodies, plan);
    request.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new(
        "rules_java++compatibility_proxy+compatibility_proxy",
    )
    .unwrap();
    request.repo_spec.rule_id.bzl_file =
        CanonicalLabel::parse("@@rules_java+//java:rules_java_deps.bzl").unwrap();
    request.repo_spec.rule_id.rule_name = "_compatibility_proxy_repo_rule".into();
    request.repo_spec.attributes = Arc::new(starlark_map::small_map::SmallMap::new());
    request
}
#[cfg(target_os = "linux")]
const JAVA_FILE_PLAN_TEST_CANONICALS: [Option<&str>; 7] = [
    None, // Exact named local source rule.
    Some("rules_java++toolchains+remote_jdk8_linux_toolchain_config_repo"),
    Some("rules_java++toolchains+remote_jdk8_linux_aarch64_toolchain_config_repo"),
    Some("renamed_descriptor+"),
    Some("rules_java++toolchains+remote_jdk11_linux_toolchain_config_repo"),
    Some("rules_java++toolchains+remote_jdk8_windows_toolchain_config_repo"),
    Some("rules_java++toolchains+local_jdk"), // Remote source rule, distinct full request.
];
#[cfg(target_os = "linux")]
fn java_file_plan_test_request(
    bodies: &Bodies,
    plan: slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan,
    remote: Option<&str>,
) -> slug_bzlmod_v2::RepositoryMaterializationRequest {
    let mut request = java_proxy_generated_request(bodies, plan);
    let (canonical, label, rule) = if let Some(canonical) = remote {
        (
            canonical,
            "@@rules_java+//toolchains:remote_java_repository.bzl",
            "_toolchain_config",
        )
    } else {
        (
            "rules_java++toolchains+local_jdk",
            "@@rules_java+//toolchains:local_java_repository.bzl",
            "_local_java_repository_rule",
        )
    };
    request.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new(canonical).unwrap();
    request.repo_spec.rule_id.bzl_file = CanonicalLabel::parse(label).unwrap();
    request.repo_spec.rule_id.rule_name = rule.into();
    // Synthetic semantic attributes are retained, not guessed native attribute filters.
    request.repo_spec.attributes = Arc::new(
        [(
            compact_str::CompactString::new("synthetic"),
            slug_bzlmod_v2::OverrideAttributeValue::Bool(true),
        )]
        .into_iter()
        .collect(),
    );
    request
}
#[test]
#[cfg(target_os = "linux")]
fn local_jdk_file_effects_are_bounded_and_opt_in() {
    for remote in JAVA_FILE_PLAN_TEST_CANONICALS {
        assert_java_file_effects_are_bounded_and_opt_in(remote);
    }
}
#[cfg(target_os = "linux")]
fn assert_java_file_effects_are_bounded_and_opt_in(remote: Option<&str>) {
    let make_request = |bodies: &Bodies, plan| java_file_plan_test_request(bodies, plan, remote);
    use immutable_archive_preparation::JAVA_DIGEST;
    use immutable_archive_preparation::JAVA_URL;
    use immutable_archive_preparation::PROTOBUF_URL;
    let path = |s| NormalizedAbsolutePath::new(s).unwrap();
    let bodies = java_proxy_test_bodies();
    let valid = SourcePreparationNeeds::repository(make_request(
        &bodies,
        small_generated_plan(&[
            ("BUILD", b"synthetic", false),
            ("nested/defs.bzl", b"", true),
        ]),
    ));
    let original = valid.repository_materializations().values().next().unwrap();
    let counts = (
        Arc::strong_count(original),
        Arc::strong_count(&original.repo_spec.attributes),
        Arc::strong_count(&bodies.java_offline_archive.as_ref().unwrap().backing),
    );
    let RepositoryMaterializationKind::GeneratedFileEffects(original_plan) = &original.kind else {
        panic!();
    };
    assert!(std::ptr::eq(
        bodies.qualified_java_file_plan(original).unwrap(),
        original_plan
    ));
    assert!(
        bodies
            .generated_features_owner_signatures(original)
            .is_none()
    );
    assert!(bodies.admit(&valid).is_ok());
    if remote.is_some() {
        let mut renamed = original.as_ref().clone();
        renamed.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new(
            if original.id.canonical_repo.as_str() == "renamed_descriptor+" {
                "another_descriptor+"
            } else {
                "renamed_descriptor+"
            },
        )
        .unwrap();
        assert_eq!(original.repo_spec, renamed.repo_spec);
        assert_eq!(original.kind, renamed.kind);
        assert_ne!(original.as_ref(), &renamed);
        assert!(
            bodies
                .admit(&SourcePreparationNeeds::repository(renamed))
                .is_ok()
        );
    }
    let mut changed = make_request(
        &bodies,
        small_generated_plan(&[("BUILD", b"different valid content", true)]),
    );
    assert_eq!(original.repo_spec, changed.repo_spec);
    let RepositoryMaterializationKind::GeneratedFileEffects(changed_plan) = &changed.kind else {
        panic!();
    };
    assert_ne!(original_plan, changed_plan);
    assert_ne!(original.as_ref(), &changed); // Plan changes are full request identity.
    changed.repo_spec.attributes = Arc::new(
        [(
            compact_str::CompactString::new("different"),
            slug_bzlmod_v2::OverrideAttributeValue::Int(7),
        )]
        .into_iter()
        .collect(),
    );
    let alternative = SourcePreparationNeeds::repository(changed);
    assert_ne!(
        original.as_ref(),
        alternative
            .repository_materializations()
            .values()
            .next()
            .unwrap()
            .as_ref()
    );
    assert!(bodies.admit(&alternative).is_ok()); // New policy does not invent static output signatures.
    println!(
        "WP1095_LAYOUT bodies={}/{} scope={}/{} borrowed_paths={}/{}",
        std::mem::size_of::<Bodies>(),
        std::mem::align_of::<Bodies>(),
        std::mem::size_of::<SourceServiceScope>(),
        std::mem::align_of::<SourceServiceScope>(),
        std::mem::size_of::<[Option<&str>; GENERATED_PLAN_ROWS]>(),
        std::mem::align_of::<[Option<&str>; GENERATED_PLAN_ROWS]>()
    );
    for registry in [RegistryChoice::DefaultBcr, RegistryChoice::PreparedFile] {
        for command in [CommandChoice::Modules, CommandChoice::PublicProbe] {
            for defs in [false, true] {
                let selected = java_proxy_test_bodies()
                    .with_generated_features_file_effects(registry, command)
                    .with_rules_rust_defs_import_sources(
                        if defs {
                            registry
                        } else {
                            RegistryChoice::DefaultBcr
                        },
                        command,
                    );
                assert_eq!(
                    selected.admit(&valid).is_ok(),
                    defs && matches!(registry, RegistryChoice::PreparedFile)
                        && matches!(command, CommandChoice::PublicProbe)
                );
            }
        }
    }
    assert!(
        Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
            .admit(&valid)
            .is_err()
    );
    for case in 0..5 {
        let mut closed = java_proxy_test_bodies();
        if case == 0 {
            closed.java_offline_archive = None;
        } else {
            let binding = closed.java_offline_archive.as_mut().unwrap();
            match case {
                1 => {
                    binding.backing = OfflineArchiveBacking::new(
                        PROTOBUF_URL,
                        JAVA_DIGEST,
                        binding.backing.path().dupe(),
                    )
                }
                2 => {
                    binding.backing =
                        OfflineArchiveBacking::new(JAVA_URL, [0; 32], binding.backing.path().dupe())
                }
                3 => {
                    binding.backing = OfflineArchiveBacking::new(
                        JAVA_URL,
                        JAVA_DIGEST,
                        path("/foreign/java.tar.gz"),
                    )
                }
                _ => binding.module = path("/foreign/MODULE.bazel"),
            }
        }
        assert!(
            closed
                .admit_with_plan_matcher(&valid, &|_, _| true)
                .is_err()
        );
    }
    // An unchanged static owner is a different key, so union retains both original requests.
    let allowed = SourcePreparationNeeds::repository(java_proxy_generated_request(
        &bodies,
        small_generated_plan(&java_proxy_literal_rows()),
    ));
    assert!(bodies.admit(&allowed).is_ok());
    let reject = |bad: SourcePreparationNeeds| {
        let offender = bad.repository_materializations().values().next().unwrap();
        for mixed in [
            allowed.try_union(&bad).unwrap(),
            bad.try_union(&allowed).unwrap(),
        ] {
            let mut progress = 0;
            if bodies.admit(&mixed).is_ok() {
                progress += 1;
            }
            assert_eq!(progress, 0);
            assert_eq!(bodies.admit(&mixed).unwrap_err().family, "materialization");
            let (first, reason) = bodies.first_denied_materialization(&mixed).unwrap();
            assert_eq!(reason, "generated_kind");
            assert!(std::ptr::eq(first, offender.as_ref()));
        }
    };
    for case in 0..11 {
        if remote.is_some() && matches!(case, 1 | 7..=10) {
            continue; // Canonical spelling selects identity, not remote rule capability.
        }
        let mut wrong = original.as_ref().clone();
        match case {
            0 => wrong.id.workspace = path("/other"),
            1 => {
                wrong.id.canonical_repo =
                    slug_identity_v2::CanonicalRepoName::new("rules_java++toolchains+other")
                        .unwrap()
            }
            2 => {
                wrong.repo_spec.rule_id.bzl_file = CanonicalLabel::parse(if remote.is_some() {
                    "@@other+//toolchains:remote_java_repository.bzl"
                } else {
                    "@@other+//toolchains:local_java_repository.bzl"
                })
                .unwrap()
            }
            3 => {
                wrong.repo_spec.rule_id.bzl_file = CanonicalLabel::parse(if remote.is_some() {
                    "@@rules_java+//private:remote_java_repository.bzl"
                } else {
                    "@@rules_java+//private:local_java_repository.bzl"
                })
                .unwrap()
            }
            4 => {
                wrong.repo_spec.rule_id.bzl_file = CanonicalLabel::parse(if remote.is_some() {
                    "@@rules_java+//toolchains:Remote_java_repository.bzl"
                } else {
                    "@@rules_java+//toolchains:Local_java_repository.bzl"
                })
                .unwrap()
            }
            5 => {
                wrong.repo_spec.rule_id.rule_name = if remote.is_some() {
                    "_toolchain_config_extra"
                } else {
                    "_local_java_repository_rule_extra"
                }
                .into()
            }
            6 => {
                let mut mapping = slug_identity_v2::RepositoryMapping::new(
                    slug_identity_v2::RepositoryMappingId::new("mapped").unwrap(),
                );
                mapping.insert(
                    slug_identity_v2::ApparentRepoName::new("java").unwrap(),
                    slug_identity_v2::CanonicalRepoName::new("rules_java+").unwrap(),
                );
                wrong.repo_spec.rule_id.bzl_file =
                    slug_identity_v2::ApparentLabel::parse(if remote.is_some() {
                        "@java//toolchains:remote_java_repository.bzl"
                    } else {
                        "@java//toolchains:local_java_repository.bzl"
                    })
                    .unwrap()
                    .resolve(&mapping);
            }
            7..=9 => {
                wrong.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new(match case {
                    7 => "rules_java++toolchains+remote_jdk8_windows_toolchain_config_repo",
                    8 => "rules_java++toolchains+remote_jdk8_linux_toolchain_config_repo_extra",
                    _ => "rules_java++toolchains+remote_jdk11_linux_toolchain_config_repo",
                })
                .unwrap();
            }
            _ => {
                // The local source rule still requires its exact named canonical owner.
                wrong.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new(
                    "rules_java++toolchains+remote_jdk8_linux_toolchain_config_repo",
                )
                .unwrap();
            }
        }
        let bad = SourcePreparationNeeds::repository(wrong);
        assert!(bodies.admit_with_plan_matcher(&bad, &|_, _| true).is_err());
        reject(bad);
    }
    let reject_plan = |plan| {
        reject(SourcePreparationNeeds::repository(make_request(
            &bodies, plan,
        )))
    };
    reject_plan(small_generated_plan(&[]));
    for rows in [
        [("a", &b""[..], false), ("a/b", &b""[..], true)],
        [("a/b", &b""[..], true), ("a", &b""[..], false)],
    ] {
        reject_plan(small_generated_plan(&rows));
    }
    // Public builder forbids raw invalid paths; the predicate independently checks original spelling.
    for raw in [
        "",
        "/absolute",
        "a//b",
        "a/./b",
        "a/../b",
        "a/",
        "a\\b",
        "a\0b",
    ] {
        assert!(
            slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan::builder()
                .push(raw.into(), Arc::from(&b""[..]), false)
                .is_err()
        );
    }
    let shared: Arc<[u8]> = Arc::from(vec![0; GENERATED_PLAN_CONTENT_BYTES]);
    let make_plan = |paths: &[String], bytes: &Arc<[u8]>| {
        slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan::build(
            paths
                .iter()
                .map(|path| (compact_str::CompactString::new(path), bytes.clone(), true)),
        )
        .unwrap()
    };
    let short: Vec<_> = (0..9).map(|i| format!("f{i}")).collect();
    let empty: Arc<[u8]> = Arc::from(&b""[..]);
    let max_rows =
        SourcePreparationNeeds::repository(make_request(&bodies, make_plan(&short[..8], &empty)));
    assert!(bodies.admit(&max_rows).is_ok());
    reject_plan(make_plan(&short, &empty));
    let long: Vec<_> = (0..5)
        .map(|i| format!("{}{i}", "x".repeat(GENERATED_PLAN_PATH_BYTES - 1)))
        .collect();
    let exact =
        SourcePreparationNeeds::repository(make_request(&bodies, make_plan(&long[..4], &shared)));
    assert!(bodies.admit(&exact).is_ok()); // Exact per-field and aggregate ceilings, sharing one body.
    let mut sum_over = long[..4].to_vec();
    sum_over.push("y".to_owned()); // Exact path sum ceiling plus one.
    reject_plan(make_plan(&sum_over, &empty));
    reject_plan(make_plan(
        &["x".repeat(GENERATED_PLAN_PATH_BYTES + 1)],
        &empty,
    ));
    let mut sum_over = slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan::builder();
    for file in &short[..4] {
        sum_over
            .push(file.as_str().into(), shared.clone(), true)
            .unwrap();
    }
    sum_over
        .push(short[4].as_str().into(), Arc::from(&b"x"[..]), false)
        .unwrap();
    reject_plan(sum_over.finish()); // Exact content sum ceiling plus one.
    let too_large: Arc<[u8]> = Arc::from(vec![0; GENERATED_PLAN_CONTENT_BYTES + 1]);
    reject_plan(make_plan(&short[..1], &too_large));
    for symlink in [false, true] {
        let mut builder = slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan::builder();
        builder.push("BUILD".into(), empty.clone(), false).unwrap();
        if symlink {
            builder
                .push_symlink(
                    "extra".into(),
                    slug_bzlmod_v2::HostRepositoryLabelPathValue::new(
                        path("/source"),
                        PathObservationNamespace::Host,
                    )
                    .unwrap(),
                    empty.clone(),
                )
                .unwrap();
        } else {
            builder
                .push_archive(
                    Arc::from([compact_str::CompactString::new(
                        "https://example.invalid/a.tar.xz",
                    )]),
                    [0; 32],
                    "prefix".into(),
                )
                .unwrap();
        }
        reject_plan(builder.finish());
    }
    assert_eq!(
        (
            Arc::strong_count(original),
            Arc::strong_count(&original.repo_spec.attributes),
            Arc::strong_count(&bodies.java_offline_archive.as_ref().unwrap().backing)
        ),
        counts
    );
}
#[test]
#[cfg(target_os = "linux")]
fn local_jdk_namespace_is_current_and_declared_only() {
    for remote in JAVA_FILE_PLAN_TEST_CANONICALS {
        assert_java_namespace_is_current_and_declared_only(remote);
    }
}
#[cfg(target_os = "linux")]
fn assert_java_namespace_is_current_and_declared_only(remote: Option<&str>) {
    let make_request = |bodies: &Bodies, plan| java_file_plan_test_request(bodies, plan, remote);
    let bodies = java_proxy_test_bodies()
        .with_repo_marker_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_ignore_file_bytes(RegistryChoice::PreparedFile, CommandChoice::PublicProbe);
    let content: Arc<[u8]> = Arc::from(&b"synthetic"[..]);
    let body_weak = Arc::downgrade(&content);
    let mut builder = slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan::builder();
    builder.push("nested/BUILD".into(), content, false).unwrap();
    let request = Arc::new(make_request(&bodies, builder.finish()));
    let request_weak = Arc::downgrade(&request);
    let attributes_weak = Arc::downgrade(&request.repo_spec.attributes);
    let instance = slug_workspace_v2::PathObservationInstanceId::new(17);
    let epoch_a = namespace_test_epoch_at(&bodies, [request.clone()], instance, "/parent/A");
    let epoch_b = namespace_test_epoch_at(&bodies, [request.clone()], instance, "/parent/B");
    let counts = (
        Arc::strong_count(&request),
        Arc::strong_count(&request.repo_spec.attributes),
        body_weak.strong_count(),
    );
    let materialized = PathObservationNamespace::Materialization(instance);
    let demand = |namespace, path, operation| {
        PathObservationDemand::new(
            namespace,
            NormalizedAbsolutePath::new(path).unwrap(),
            operation,
        )
    };
    let admits =
        |d: &PathObservationDemand,
         epoch: Option<&slug_bzlmod_v2::RepositoryMaterializationResultEpoch>| {
            namespace_policy(&bodies, d, epoch, &generated_features_plan_matches)
                .admits(&d.operation())
        };
    for (epoch, root, parent, leaf, marker, ignore) in [
        (
            &epoch_a,
            "/parent/A",
            "/parent/A/nested",
            "/parent/A/nested/BUILD",
            "/parent/A/REPO.bazel",
            "/parent/A/.bazelignore",
        ),
        (
            &epoch_b,
            "/parent/B",
            "/parent/B/nested",
            "/parent/B/nested/BUILD",
            "/parent/B/REPO.bazel",
            "/parent/B/.bazelignore",
        ),
        (
            &epoch_a,
            "/parent/A",
            "/parent/A/nested",
            "/parent/A/nested/BUILD",
            "/parent/A/REPO.bazel",
            "/parent/A/.bazelignore",
        ),
    ] {
        for path in ["/", "/parent", root, parent] {
            assert!(admits(
                &demand(materialized, path, PathObservationOperation::Lstat),
                Some(epoch)
            ));
            assert!(!admits(
                &demand(materialized, path, PathObservationOperation::FileBytes),
                Some(epoch)
            ));
            assert!(!admits(
                &demand(
                    materialized,
                    path,
                    PathObservationOperation::DirectoryEntries
                ),
                Some(epoch)
            ));
        }
        for operation in [
            PathObservationOperation::Lstat,
            PathObservationOperation::FileBytes,
        ] {
            assert!(admits(&demand(materialized, leaf, operation), Some(epoch)));
        }
        for (path, class) in [(marker, "repo_marker"), (ignore, "ignore_marker")] {
            let marker = demand(materialized, path, PathObservationOperation::Lstat);
            assert_eq!(
                namespace_policy(
                    &bodies,
                    &marker,
                    Some(epoch),
                    &generated_features_plan_matches
                )
                .path_class,
                class
            );
            assert!(admits(&marker, Some(epoch)));
        }
    }
    let valid_demand = demand(
        materialized,
        "/parent/B/nested/BUILD",
        PathObservationOperation::FileBytes,
    );
    assert!(!admits(&valid_demand, None));
    assert!(!admits(&valid_demand, Some(&epoch_a)));
    let wrong_workspace = slug_bzlmod_v2::RepositoryMaterializationResultEpoch::new(
        NormalizedAbsolutePath::new("/foreign").unwrap(),
        [],
    )
    .unwrap();
    assert!(!admits(&valid_demand, Some(&wrong_workspace)));
    let mut other = request.as_ref().clone();
    other.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new("other+").unwrap();
    let ambiguous = namespace_test_epoch_at(
        &bodies,
        [request.clone(), Arc::new(other)],
        instance,
        "/parent/B",
    );
    assert!(!admits(&valid_demand, Some(&ambiguous)));
    let marker = demand(
        materialized,
        "/parent/B/REPO.bazel",
        PathObservationOperation::Lstat,
    );
    let ignore_marker = demand(
        materialized,
        "/parent/B/.bazelignore",
        PathObservationOperation::Lstat,
    );
    for epoch in [
        None,
        Some(&epoch_a),
        Some(&wrong_workspace),
        Some(&ambiguous),
    ] {
        for marker in [&marker, &ignore_marker] {
            assert!(!admits(marker, epoch));
        }
    }
    for (repo, ignore) in [(false, false), (false, true), (true, false), (true, true)] {
        let mut flags = java_proxy_test_bodies();
        flags.repo_marker_metadata = repo;
        flags.ignore_file_bytes = ignore;
        for (marker, enabled) in [(&marker, repo), (&ignore_marker, ignore)] {
            assert_eq!(
                namespace_policy(
                    &flags,
                    marker,
                    Some(&epoch_b),
                    &generated_features_plan_matches
                )
                .admits(&marker.operation()),
                enabled
            );
        }
    }
    for registry in [RegistryChoice::DefaultBcr, RegistryChoice::PreparedFile] {
        for command in [CommandChoice::Modules, CommandChoice::PublicProbe] {
            let roles = java_proxy_test_bodies()
                .with_repo_marker_metadata(registry, command)
                .with_ignore_file_bytes(registry, command);
            for marker in [&marker, &ignore_marker] {
                assert_eq!(
                    namespace_policy(
                        &roles,
                        marker,
                        Some(&epoch_b),
                        &generated_features_plan_matches
                    )
                    .admits(&marker.operation()),
                    matches!(registry, RegistryChoice::PreparedFile)
                        && matches!(command, CommandChoice::PublicProbe)
                );
            }
        }
    }
    // An explicit leaf retains its prior body authority even without the marker role.
    {
        let disabled = java_proxy_test_bodies()
            .with_repo_marker_metadata(RegistryChoice::DefaultBcr, CommandChoice::Modules)
            .with_ignore_file_bytes(RegistryChoice::DefaultBcr, CommandChoice::Modules);
        let declared = namespace_test_epoch_at(
            &disabled,
            [Arc::new(make_request(
                &disabled,
                small_generated_plan(&[
                    ("REPO.bazel", b"synthetic", false),
                    (".bazelignore", b"synthetic", false),
                ]),
            ))],
            instance,
            "/parent/B",
        );
        for operation in [
            PathObservationOperation::Lstat,
            PathObservationOperation::FileBytes,
        ] {
            for path in ["/parent/B/REPO.bazel", "/parent/B/.bazelignore"] {
                let leaf = demand(materialized, path, operation.dupe());
                let policy = namespace_policy(
                    &disabled,
                    &leaf,
                    Some(&declared),
                    &generated_features_plan_matches,
                );
                assert_eq!(policy.path_class, "fixed_file");
                assert!(policy.admits(&leaf.operation()));
            }
        }
    }
    // A malformed successful epoch must not turn a file-parent collision into path authority.
    for plan in [
        small_generated_plan(&[("a", b"", false), ("a/b", b"", false)]),
        small_generated_plan(&[]),
    ] {
        let malformed = namespace_test_epoch_at(
            &bodies,
            [Arc::new(make_request(&bodies, plan))],
            instance,
            "/parent/B",
        );
        for path in [
            "/parent/B",
            "/parent/B/a",
            "/parent/B/a/b",
            "/parent",
            "/parent/B/REPO.bazel",
            "/parent/B/.bazelignore",
        ] {
            assert!(!admits(
                &demand(materialized, path, PathObservationOperation::Lstat),
                Some(&malformed)
            ));
        }
    }
    let mut closed = java_proxy_test_bodies()
        .with_repo_marker_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_ignore_file_bytes(RegistryChoice::PreparedFile, CommandChoice::PublicProbe);
    closed.java_offline_archive = None;
    assert!(
        !namespace_policy(
            &closed,
            &valid_demand,
            Some(&epoch_b),
            &generated_features_plan_matches
        )
        .admits(&valid_demand.operation())
    );
    for marker in [&marker, &ignore_marker] {
        assert!(
            !namespace_policy(
                &closed,
                marker,
                Some(&epoch_b),
                &generated_features_plan_matches
            )
            .admits(&marker.operation())
        );
    }
    let mut no_defs = java_proxy_test_bodies()
        .with_repo_marker_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_ignore_file_bytes(RegistryChoice::PreparedFile, CommandChoice::PublicProbe);
    no_defs.rules_rust_defs_import_sources = false;
    for marker in [&marker, &ignore_marker] {
        assert!(
            !namespace_policy(
                &no_defs,
                marker,
                Some(&epoch_b),
                &generated_features_plan_matches
            )
            .admits(&marker.operation())
        );
    }
    let valid = SourcePreparationNeeds::path(
        NeedPathObservations::try_from_iter([valid_demand, marker, ignore_marker]).unwrap(),
    );
    let admit_need = |needs: &SourcePreparationNeeds| {
        bodies.admit_with_matchers(needs, &generated_features_plan_matches, &|d| {
            admits(d, Some(&epoch_b))
        })
    };
    assert!(admit_need(&valid).is_ok());
    let foreign = PathObservationNamespace::Materialization(
        slug_workspace_v2::PathObservationInstanceId::new(18),
    );
    for (namespace, path, operation) in [
        (
            materialized,
            "/parent/B/nested/BUILD",
            PathObservationOperation::FileDigest,
        ),
        (
            materialized,
            "/parent/B/nested/BUILD",
            PathObservationOperation::ReadLink,
        ),
        (
            materialized,
            "/parent/B/nested/BUILD",
            PathObservationOperation::DirectoryEntries,
        ),
        (
            materialized,
            "/parent/B/nested/build",
            PathObservationOperation::Lstat,
        ),
        (
            materialized,
            "/parent/B/nested/BUILD.bak",
            PathObservationOperation::FileBytes,
        ),
        (
            materialized,
            "/parent/B/nested/BUILD/child",
            PathObservationOperation::Lstat,
        ),
        (
            materialized,
            "/parent/B/nested/other",
            PathObservationOperation::Lstat,
        ),
        (
            materialized,
            "/parent/B-sibling/nested/BUILD",
            PathObservationOperation::Lstat,
        ),
        (materialized, "/outside", PathObservationOperation::Lstat),
        (
            materialized,
            "/parent/A/nested/BUILD",
            PathObservationOperation::FileBytes,
        ),
        (
            foreign,
            "/parent/B/nested/BUILD",
            PathObservationOperation::FileBytes,
        ),
        (
            PathObservationNamespace::Host,
            "/parent/B/nested/BUILD",
            PathObservationOperation::FileBytes,
        ),
    ]
    .into_iter()
    .chain(
        ["/parent/B/REPO.bazel", "/parent/B/.bazelignore"]
            .into_iter()
            .flat_map(|path| {
                [
                    PathObservationOperation::FileBytes,
                    PathObservationOperation::DirectoryEntries,
                    PathObservationOperation::FileDigest,
                    PathObservationOperation::ReadLink,
                ]
                .into_iter()
                .map(move |operation| (materialized, path, operation))
            }),
    )
    .chain(
        [
            (materialized, "/parent/B/repo.bazel"),
            (materialized, "/parent/B/.BAZELIGNORE"),
            (materialized, "/parent/B/REPO.bazel.bak"),
            (materialized, "/parent/B/.bazelignore.bak"),
            (materialized, "/parent/B/REPO.bazel/child"),
            (materialized, "/parent/B/.bazelignore/child"),
            (materialized, "/parent/B/nested/REPO.bazel"),
            (materialized, "/parent/B/nested/.bazelignore"),
            (materialized, "/parent/B-sibling/REPO.bazel"),
            (materialized, "/parent/B-sibling/.bazelignore"),
            (materialized, "/outside/REPO.bazel"),
            (materialized, "/outside/.bazelignore"),
            (materialized, "/parent/A/REPO.bazel"),
            (materialized, "/parent/A/.bazelignore"),
            (foreign, "/parent/B/REPO.bazel"),
            (foreign, "/parent/B/.bazelignore"),
        ]
        .into_iter()
        .map(|(namespace, path)| (namespace, path, PathObservationOperation::Lstat)),
    ) {
        let bad = SourcePreparationNeeds::path(NeedPathObservations::singleton(demand(
            namespace,
            path,
            operation.dupe(),
        )));
        for mixed in [
            valid.try_union(&bad).unwrap(),
            bad.try_union(&valid).unwrap(),
        ] {
            let mut progress = 0;
            if admit_need(&mixed).is_ok() {
                progress += 1;
            }
            assert_eq!(progress, 0);
            assert_eq!(
                admit_need(&mixed).unwrap_err().family,
                if namespace == PathObservationNamespace::Host {
                    "path"
                } else {
                    "namespace"
                }
            );
            let (first, _) = bodies
                .first_denied_path_with_namespace_matcher(&mixed, &|d| admits(d, Some(&epoch_b)))
                .unwrap();
            let original = mixed
                .path_observations()
                .unwrap()
                .demands()
                .iter()
                .find(|d| {
                    d.namespace() == namespace
                        && d.path().as_path() == Path::new(path)
                        && d.operation() == operation
                })
                .unwrap();
            assert!(std::ptr::eq(first, original));
        }
    }
    assert!(std::ptr::eq(
        epoch_b
            .borrowed_instance_for_namespace(&bodies.roots[0], instance, 32)
            .unwrap()
            .unwrap()
            .request(),
        request.as_ref()
    ));
    drop(ambiguous);
    assert_eq!(
        (
            Arc::strong_count(&request),
            Arc::strong_count(&request.repo_spec.attributes),
            body_weak.strong_count()
        ),
        counts
    );
    drop(epoch_a);
    drop(epoch_b);
    drop(request);
    assert!(request_weak.upgrade().is_none());
    assert!(attributes_weak.upgrade().is_none());
    assert!(body_weak.upgrade().is_none());
}
#[test]
#[cfg(target_os = "linux")]
fn java_compatibility_proxy_materialization_is_exact_and_opt_in() {
    use immutable_archive_preparation::JAVA_DIGEST;
    use immutable_archive_preparation::JAVA_URL;
    use immutable_archive_preparation::PROTOBUF_URL;
    let path = |s| NormalizedAbsolutePath::new(s).unwrap();
    let bodies = java_proxy_test_bodies();
    let rows = java_proxy_literal_rows();
    let valid = SourcePreparationNeeds::repository(java_proxy_generated_request(
        &bodies,
        small_generated_plan(&rows),
    ));
    let original = valid.repository_materializations().values().next().unwrap();
    let count = Arc::strong_count(original);
    let backing = &bodies.java_offline_archive.as_ref().unwrap().backing;
    let owners = Arc::strong_count(backing);
    assert!(bodies.offline_archive.is_none()); // Java authorization has no protobuf dependency.
    assert!(bodies.admit(&valid).is_ok());
    assert_eq!(Arc::strong_count(original), count);
    assert_eq!(Arc::strong_count(backing), owners);
    assert!(std::ptr::eq(
        bodies
            .generated_features_owner_signatures(original)
            .unwrap(),
        &GENERATED_JAVA_PROXY_FILES[..]
    ));
    eprintln!(
        "WP1087_JAVA_PROXY_LAYOUT bodies={}/{} scope={}/{} signatures={} row={} literal={}",
        std::mem::size_of::<Bodies>(),
        std::mem::align_of::<Bodies>(),
        std::mem::size_of::<SourceServiceScope>(),
        std::mem::align_of::<SourceServiceScope>(),
        std::mem::size_of_val(&GENERATED_JAVA_PROXY_FILES),
        std::mem::size_of::<GeneratedFileSignature>(),
        JAVA_PROXY_LITERAL_0.len() + JAVA_PROXY_LITERAL_1.len()
    );
    for registry in [RegistryChoice::DefaultBcr, RegistryChoice::PreparedFile] {
        for command in [CommandChoice::Modules, CommandChoice::PublicProbe] {
            let selected =
                java_proxy_test_bodies().with_generated_features_file_effects(registry, command);
            assert_eq!(
                selected.admit(&valid).is_ok(),
                matches!(registry, RegistryChoice::PreparedFile)
                    && matches!(command, CommandChoice::PublicProbe)
            );
        }
    }
    let default = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")]);
    assert!(default.admit(&valid).is_err());
    for case in 0..5 {
        let mut closed = java_proxy_test_bodies();
        if case == 0 {
            closed.java_offline_archive = None;
        } else {
            let binding = closed.java_offline_archive.as_mut().unwrap();
            match case {
                1 => {
                    binding.backing = OfflineArchiveBacking::new(
                        PROTOBUF_URL,
                        JAVA_DIGEST,
                        binding.backing.path().dupe(),
                    )
                }
                2 => {
                    binding.backing =
                        OfflineArchiveBacking::new(JAVA_URL, [0; 32], binding.backing.path().dupe())
                }
                3 => {
                    binding.backing = OfflineArchiveBacking::new(
                        JAVA_URL,
                        JAVA_DIGEST,
                        path("/foreign/java.tar.gz"),
                    )
                }
                _ => binding.module = path("/foreign/MODULE.bazel"),
            }
        }
        assert!(
            closed
                .admit_with_plan_matcher(&valid, &|_, _| true)
                .is_err()
        );
    }
    let allowed = SourcePreparationNeeds::repository(proxy_generated_request(
        &bodies,
        small_generated_plan(&proxy_literal_rows()),
    ));
    assert!(bodies.admit(&allowed).is_ok());
    let reject = |bad: SourcePreparationNeeds| {
        let offender = bad.repository_materializations().values().next().unwrap();
        for mixed in [
            allowed.try_union(&bad).unwrap(),
            bad.try_union(&allowed).unwrap(),
        ] {
            let mut progress = 0;
            if bodies.admit(&mixed).is_ok() {
                progress += 1;
            }
            assert_eq!(progress, 0);
            assert_eq!(bodies.admit(&mixed).unwrap_err().family, "materialization");
            let (first, reason) = bodies.first_denied_materialization(&mixed).unwrap();
            assert_eq!(reason, "generated_kind");
            assert!(std::ptr::eq(first, offender.as_ref()));
        }
    };
    for case in 0..9 {
        let mut wrong = original.as_ref().clone(); // Shares original immutable plan/attribute storage.
        match case {
            0 => wrong.id.workspace = path("/other"),
            1 => {
                wrong.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new(
                    "rules_java++compatibility_proxy+other",
                )
                .unwrap()
            }
            2 => {
                wrong.repo_spec.rule_id.bzl_file =
                    CanonicalLabel::parse("@@other+//java:rules_java_deps.bzl").unwrap()
            }
            3 => {
                wrong.repo_spec.rule_id.bzl_file =
                    CanonicalLabel::parse("@@rules_java+//private:rules_java_deps.bzl").unwrap()
            }
            4 => {
                wrong.repo_spec.rule_id.bzl_file =
                    CanonicalLabel::parse("@@rules_java+//java:other.bzl").unwrap()
            }
            5 => wrong.repo_spec.rule_id.rule_name = "other".into(),
            6 => {
                let mut mapping = slug_identity_v2::RepositoryMapping::new(
                    slug_identity_v2::RepositoryMappingId::new("mapped").unwrap(),
                );
                mapping.insert(
                    slug_identity_v2::ApparentRepoName::new("java").unwrap(),
                    slug_identity_v2::CanonicalRepoName::new("rules_java+").unwrap(),
                );
                wrong.repo_spec.rule_id.bzl_file =
                    slug_identity_v2::ApparentLabel::parse("@java//java:rules_java_deps.bzl")
                        .unwrap()
                        .resolve(&mapping);
            }
            _ => {
                let mut attributes = starlark_map::small_map::SmallMap::new();
                attributes.insert(
                    compact_str::CompactString::new("unexpected"),
                    if case == 7 {
                        slug_bzlmod_v2::OverrideAttributeValue::None
                    } else {
                        slug_bzlmod_v2::OverrideAttributeValue::Bool(true)
                    },
                );
                wrong.repo_spec.attributes = Arc::new(attributes);
            }
        }
        let bad = SourcePreparationNeeds::repository(wrong);
        assert!(bodies.admit_with_plan_matcher(&bad, &|_, _| true).is_err());
        reject(bad);
    }
    let reject_rows = |changed: &[(&str, &[u8], bool)]| {
        reject(SourcePreparationNeeds::repository(
            java_proxy_generated_request(&bodies, small_generated_plan(changed)),
        ));
    };
    reject_rows(&[]);
    reject_rows(&rows[..1]);
    reject_rows(&[rows[0], rows[1], ("extra", b"", true)]);
    reject_rows(&[rows[1], rows[0]]);
    let mutation = [b'x'; 1778];
    for file in 0..2 {
        for change in 0..4 {
            let mut changed = rows;
            match change {
                0 => changed[file].0 = "wrong.bzl",
                1 => changed[file].1 = b"",
                2 => changed[file].1 = &mutation[..rows[file].1.len()],
                _ => changed[file].2 = false,
            }
            reject_rows(&changed);
        }
    }
    let mut builder = slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan::builder();
    builder
        .push_archive(
            Arc::from([compact_str::CompactString::new(
                "https://example.invalid/archive.tar.xz",
            )]),
            [0; 32],
            compact_str::CompactString::new("prefix"),
        )
        .unwrap();
    reject(SourcePreparationNeeds::repository(
        java_proxy_generated_request(&bodies, builder.finish()),
    ));
    assert_eq!(Arc::strong_count(original), count);
    assert_eq!(Arc::strong_count(backing), owners);
}
#[test]
#[cfg(target_os = "linux")]
fn java_compatibility_proxy_namespace_is_current_and_confined() {
    let bodies = java_proxy_test_bodies();
    let request = Arc::new(java_proxy_generated_request(
        &bodies,
        small_generated_plan(&java_proxy_literal_rows()),
    ));
    let instance = slug_workspace_v2::PathObservationInstanceId::new(17);
    let epoch_a = namespace_test_epoch_at(&bodies, [request.clone()], instance, "/parent/A");
    let epoch_b = namespace_test_epoch_at(&bodies, [request.clone()], instance, "/parent/B");
    let count = Arc::strong_count(&request);
    let demand = |namespace, path, operation| {
        PathObservationDemand::new(
            namespace,
            NormalizedAbsolutePath::new(path).unwrap(),
            operation,
        )
    };
    let materialized = PathObservationNamespace::Materialization(instance);
    let admits = |d: &PathObservationDemand, epoch| {
        namespace_policy(&bodies, d, epoch, &generated_features_plan_matches).admits(&d.operation())
    };
    for (epoch, root, fixed) in [
        (&epoch_a, "/parent/A", "/parent/A/proxy.bzl"),
        (&epoch_b, "/parent/B", "/parent/B/proxy.bzl"),
    ] {
        for path in ["/", "/parent", root] {
            assert!(admits(
                &demand(materialized, path, PathObservationOperation::Lstat),
                Some(epoch)
            ));
            assert!(!admits(
                &demand(materialized, path, PathObservationOperation::FileBytes),
                Some(epoch)
            ));
        }
        assert!(admits(
            &demand(materialized, fixed, PathObservationOperation::FileBytes),
            Some(epoch)
        ));
    }
    let valid_demand = demand(
        materialized,
        "/parent/B/proxy.bzl",
        PathObservationOperation::FileBytes,
    );
    assert!(!admits(&valid_demand, Some(&epoch_a)));
    assert!(!admits(&valid_demand, None));
    assert!(!admits(
        &demand(
            materialized,
            "/parent/A/proxy.bzl",
            PathObservationOperation::FileBytes
        ),
        Some(&epoch_b)
    ));
    let wrong_workspace = slug_bzlmod_v2::RepositoryMaterializationResultEpoch::new(
        NormalizedAbsolutePath::new("/foreign").unwrap(),
        [],
    )
    .unwrap();
    assert!(!admits(&valid_demand, Some(&wrong_workspace)));
    let mut other = request.as_ref().clone();
    other.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new("other+").unwrap();
    let ambiguous = namespace_test_epoch_at(
        &bodies,
        [request.clone(), Arc::new(other)],
        instance,
        "/parent/B",
    );
    assert!(!admits(&valid_demand, Some(&ambiguous)));
    let mut wrong_plan = request.as_ref().clone();
    wrong_plan.kind = RepositoryMaterializationKind::GeneratedFileEffects(small_generated_plan(
        &java_proxy_literal_rows()[..1],
    ));
    let wrong_epoch =
        namespace_test_epoch_at(&bodies, [Arc::new(wrong_plan)], instance, "/parent/B");
    assert!(!admits(&valid_demand, Some(&wrong_epoch)));
    let valid = SourcePreparationNeeds::path(NeedPathObservations::singleton(valid_demand));
    let admit_need = |needs: &SourcePreparationNeeds| {
        bodies.admit_with_matchers(needs, &generated_features_plan_matches, &|d| {
            admits(d, Some(&epoch_b))
        })
    };
    assert!(admit_need(&valid).is_ok());
    let foreign = PathObservationNamespace::Materialization(
        slug_workspace_v2::PathObservationInstanceId::new(18),
    );
    for excluded in [
        demand(
            materialized,
            "/parent/B/BUILD.bazel",
            PathObservationOperation::FileDigest,
        ),
        demand(
            materialized,
            "/parent/B/proxy.bzl",
            PathObservationOperation::ReadLink,
        ),
        demand(
            materialized,
            "/parent/B",
            PathObservationOperation::DirectoryEntries,
        ),
        demand(
            materialized,
            "/parent/B/Proxy.bzl",
            PathObservationOperation::Lstat,
        ),
        demand(
            materialized,
            "/parent/B/proxy.bzl.bak",
            PathObservationOperation::FileBytes,
        ),
        demand(
            materialized,
            "/parent/B/proxy.bzl/child",
            PathObservationOperation::Lstat,
        ),
        demand(
            materialized,
            "/parent/B/BUILD",
            PathObservationOperation::Lstat,
        ),
        demand(
            materialized,
            "/parent/B/REPO.bazel",
            PathObservationOperation::Lstat,
        ),
        demand(
            materialized,
            "/parent/B/.bazelignore",
            PathObservationOperation::Lstat,
        ),
        demand(
            materialized,
            "/parent/B-sibling/proxy.bzl",
            PathObservationOperation::Lstat,
        ),
        demand(materialized, "/outside", PathObservationOperation::Lstat),
        demand(
            materialized,
            "/parent/A/proxy.bzl",
            PathObservationOperation::FileBytes,
        ),
        demand(
            foreign,
            "/parent/B/proxy.bzl",
            PathObservationOperation::FileBytes,
        ),
        demand(
            PathObservationNamespace::Host,
            "/parent/B/proxy.bzl",
            PathObservationOperation::FileBytes,
        ),
    ] {
        let bad = SourcePreparationNeeds::path(NeedPathObservations::singleton(excluded));
        for mixed in [
            valid.try_union(&bad).unwrap(),
            bad.try_union(&valid).unwrap(),
        ] {
            let mut progress = 0;
            if admit_need(&mixed).is_ok() {
                progress += 1;
            }
            assert_eq!(progress, 0);
            let denied = admit_need(&mixed).unwrap_err();
            assert_eq!(
                denied.family,
                if bad.path_observations().unwrap().demands()[0].namespace()
                    == PathObservationNamespace::Host
                {
                    "path"
                } else {
                    "namespace"
                }
            );
            let (first, _) = bodies
                .first_denied_path_with_namespace_matcher(&mixed, &|d| admits(d, Some(&epoch_b)))
                .unwrap();
            let original = mixed
                .path_observations()
                .unwrap()
                .demands()
                .iter()
                .find(|d| {
                    d.path() == bad.path_observations().unwrap().demands()[0].path()
                        && d.operation()
                            == bad.path_observations().unwrap().demands()[0].operation()
                        && d.namespace()
                            == bad.path_observations().unwrap().demands()[0].namespace()
                })
                .unwrap();
            assert!(std::ptr::eq(first, original));
        }
    }
    for file in ["/parent/B/BUILD.bazel", "/parent/B/proxy.bzl"] {
        for operation in [
            PathObservationOperation::Lstat,
            PathObservationOperation::FileBytes,
        ] {
            assert!(admits(
                &demand(materialized, file, operation),
                Some(&epoch_b)
            ));
        }
    }
    drop(ambiguous);
    assert_eq!(Arc::strong_count(&request), count);
    assert!(std::ptr::eq(
        epoch_b
            .borrowed_instance_for_namespace(&bodies.roots[0], instance, 32)
            .unwrap()
            .unwrap()
            .request(),
        request.as_ref()
    ));
    let weak = Arc::downgrade(&request);
    drop(epoch_a);
    drop(epoch_b);
    drop(request);
    assert!(weak.upgrade().is_none());
}
fn proxy_test_bodies() -> Bodies {
    namespace_test_bodies()
        .with_fifth(NormalizedAbsolutePath::new("/skylib").unwrap())
        .with_sixth(NormalizedAbsolutePath::new("/cc").unwrap())
        .with_rules_cc_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
}
fn generated_test_owner_request(
    bodies: &Bodies,
    owner: usize,
    plan: slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan,
) -> slug_bzlmod_v2::RepositoryMaterializationRequest {
    let (canonical, defining_label, rule_name) = GENERATED_DIAGNOSTIC_TEST_OWNERS[owner];
    let mut request = generated_globals_request(bodies, plan);
    request.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new(canonical).unwrap();
    request.repo_spec.rule_id.bzl_file = CanonicalLabel::parse(defining_label).unwrap();
    request.repo_spec.rule_id.rule_name = rule_name.into();
    if matches!(owner, 2 | 4) {
        request.repo_spec.attributes = Arc::new(starlark_map::small_map::SmallMap::new());
    }
    request
}
fn small_owner_first(owner: usize) -> &'static str {
    match owner {
        3 => "WORKSPACE.bazel",
        4 => "BUILD",
        _ => "BUILD.bazel",
    }
}
fn small_owner_second(owner: usize) -> &'static str {
    [
        "globals.bzl",
        "version.bzl",
        "constraints.bzl",
        "BUILD.bazel",
        "toolchains.bzl",
    ][owner]
}
fn small_namespace_plan_matches(
    plan: &slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan,
    selected: &[GeneratedFileSignature],
) -> bool {
    if std::ptr::eq(selected, &GENERATED_CC_TOOLCHAINS_FILES[..])
        || std::ptr::eq(selected, &GENERATED_SHELL_CONFIG_FILES[..])
        || std::ptr::eq(selected, &GENERATED_APPLE_TOOLCHAINS_FILES[..])
    {
        return generated_features_plan_matches(
            plan,
            &[("BUILD", 1, true, sha2::Sha256::digest(b"a").into())],
        );
    }
    let owner = if std::ptr::eq(selected, &GENERATED_FEATURES_FILES) {
        0
    } else if std::ptr::eq(selected, &GENERATED_FEATURES_VERSION_FILES) {
        1
    } else if std::ptr::eq(selected, &GENERATED_PLATFORMS_HOST_FILES) {
        2
    } else if std::ptr::eq(selected, &GENERATED_RUST_TOOLCHAIN_HUB_FILES) {
        3
    } else {
        assert!(std::ptr::eq(selected, &GENERATED_WINSDK_FILES));
        4
    };
    generated_features_plan_matches(
        plan,
        &[
            (
                small_owner_first(owner),
                1,
                true,
                sha2::Sha256::digest(b"a").into(),
            ),
            (
                small_owner_second(owner),
                1,
                true,
                sha2::Sha256::digest(b"b").into(),
            ),
        ],
    )
}
fn namespace_test_bodies() -> Bodies {
    let path = |s| NormalizedAbsolutePath::new(s).unwrap();
    Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
        .with_fourth(path("/features"))
        .with_fifth(path("/skylib"))
        .with_sixth(path("/cc"))
        .with_rules_cc_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_bazel_features_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        )
        .with_platforms_host_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        )
        .with_rules_rust_defs_import_sources(
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
    generated_test_owner_request(
        bodies,
        usize::from(version),
        small_generated_plan(&[
            ("BUILD.bazel", b"a", true),
            (small_owner_second(usize::from(version)), b"b", true),
        ]),
    )
}
#[test]
fn generated_features_namespace_is_finite_and_exact() {
    let bodies = namespace_test_bodies()
        .with_repo_marker_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_ignore_file_bytes(RegistryChoice::PreparedFile, CommandChoice::PublicProbe);
    let instance = slug_workspace_v2::PathObservationInstanceId::new(17);
    let demand = |path, operation| {
        PathObservationDemand::new(
            PathObservationNamespace::Materialization(instance),
            NormalizedAbsolutePath::new(path).unwrap(),
            operation,
        )
    };
    for owner in 0..GENERATED_DIAGNOSTIC_TEST_OWNERS.len() {
        let request = Arc::new(generated_test_owner_request(
            &bodies,
            owner,
            small_generated_plan(&[
                (small_owner_first(owner), b"a", true),
                (small_owner_second(owner), b"b", true),
            ]),
        ));
        let epoch =
            namespace_test_epoch_at(&bodies, [request.clone()], instance, "/parent/generation");
        let count = Arc::strong_count(&request);
        let fixed = [
            "/parent/generation/globals.bzl",
            "/parent/generation/version.bzl",
            "/parent/generation/constraints.bzl",
            "/parent/generation/BUILD.bazel",
            "/parent/generation/toolchains.bzl",
        ][owner];
        let first = match owner {
            3 => "/parent/generation/WORKSPACE.bazel",
            4 => "/parent/generation/BUILD",
            _ => "/parent/generation/BUILD.bazel",
        };
        for (path, class) in [
            ("/", "ancestor"),
            ("/parent", "ancestor"),
            ("/parent/generation", "root"),
            (first, "fixed_file"),
            (fixed, "fixed_file"),
            (
                "/parent/generation/REPO.bazel",
                if matches!(owner, 3 | 4) {
                    "repo_marker"
                } else {
                    "other"
                },
            ),
            (
                "/parent/generation/.bazelignore",
                if matches!(owner, 3 | 4) {
                    "ignore_marker"
                } else {
                    "other"
                },
            ),
        ]
        .into_iter()
        .chain((owner == 4).then_some(("/parent/generation/BUILD.bazel", "package_marker")))
        {
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
                    class != "other"
                        && (operation == PathObservationOperation::Lstat
                            || class == "fixed_file"
                                && operation == PathObservationOperation::FileBytes)
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
            "/parent/generation/WORKSPACE.bazel/child",
            "/parent/generation/workspace.bazel",
            "/parent/generation/WORKSPACE.bazel.bak",
            "/parent/generation/BUILD/child",
            "/parent/generation/build",
            "/parent/generation/BUILD.bak",
            "/parent/generation/toolchains.bzl/child",
            "/parent/generation/Toolchains.bzl",
            "/parent/generation/toolchains.bzl.bak",
            "/parent/generation/repo.bazel",
            "/parent/generation/REPO.bazel.bak",
            "/parent/generation/REPO.bazel/child",
            "/parent/generation/.BAZELIGNORE",
            "/parent/generation/.bazelignore.bak",
            "/parent/generation/.bazelignore/child",
            if owner == 1 {
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
        if matches!(owner, 3 | 4) {
            for repo_enabled in [false, true] {
                for ignore_enabled in [false, true] {
                    let mut roles = namespace_test_bodies();
                    roles.repo_marker_metadata = repo_enabled;
                    roles.ignore_file_bytes = ignore_enabled;
                    for (path, tag, enabled) in [
                        ("/parent/generation/REPO.bazel", "repo_marker", repo_enabled),
                        (
                            "/parent/generation/.bazelignore",
                            "ignore_marker",
                            ignore_enabled,
                        ),
                    ] {
                        let marker = demand(path, PathObservationOperation::Lstat);
                        let policy = namespace_policy(
                            &roles,
                            &marker,
                            Some(&epoch),
                            &small_namespace_plan_matches,
                        );
                        assert_eq!(policy.path_class, if enabled { tag } else { "other" });
                        assert_eq!(policy.admits(&marker.operation()), enabled);
                    }
                }
            }
            let wrong = namespace_test_epoch_at(
                &bodies,
                [Arc::new(generated_test_owner_request(
                    &bodies,
                    owner,
                    small_generated_plan(&[]),
                ))],
                instance,
                "/parent/generation",
            );
            for (path, tag) in [
                ("/parent/generation/REPO.bazel", "repo_marker"),
                ("/parent/generation/.bazelignore", "ignore_marker"),
            ]
            .into_iter()
            .chain((owner == 4).then_some(("/parent/generation/BUILD.bazel", "package_marker")))
            {
                let marker = demand(path, PathObservationOperation::Lstat);
                let policy = namespace_policy(
                    &bodies,
                    &marker,
                    Some(&wrong),
                    &small_namespace_plan_matches,
                );
                assert_eq!(policy.path_class, tag);
                assert!(!policy.owner_plan_allowed && !policy.admits(&marker.operation()));
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
            if matches!(owner, 2 | 3) && index == 2 {
                continue;
            }
            let mut closed = namespace_test_bodies();
            match index {
                0 => closed.generated_features_file_effects = false,
                1 if owner == 2 => closed.platforms_host_import_sources = false,
                1 if owner == 3 => closed.rules_rust_defs_import_sources = false,
                1 if owner == 4 => closed.rules_cc_import_sources = false,
                1 => closed.bazel_features_import_sources = false,
                2 if owner == 4 => closed.additional[2] = None,
                2 => closed.additional[0] = None,
                3 => closed.roots[0] = NormalizedAbsolutePath::new("/wrong").unwrap(),
                _ => unreachable!(),
            }
            assert!(
                !namespace_policy(&closed, &d, Some(&epoch), &|_, _| true).admits(&d.operation())
            );
            if owner == 4 {
                let candidate = demand(
                    "/parent/generation/BUILD.bazel",
                    PathObservationOperation::Lstat,
                );
                let policy = namespace_policy(&closed, &candidate, Some(&epoch), &|_, _| true);
                assert_eq!(
                    policy.path_class,
                    if index == 3 { "unavailable" } else { "other" }
                );
                assert!(!policy.admits(&candidate.operation()));
            }
        }
        for index in 0..if owner == 4 { 6 } else { 4 } {
            let mut wrong = request.as_ref().clone();
            match index {
                0 => {
                    wrong.id.canonical_repo =
                        slug_identity_v2::CanonicalRepoName::new("other+").unwrap()
                }
                1 => wrong.repo_spec.rule_id.rule_name = "other".into(),
                2 => wrong.kind = RepositoryMaterializationKind::Immutable,
                3 => wrong.kind = namespace_test_request(&bodies, owner != 1).kind,
                4 => {
                    let mut mapping = slug_identity_v2::RepositoryMapping::new(
                        slug_identity_v2::RepositoryMappingId::new("mapped").unwrap(),
                    );
                    mapping.insert(
                        slug_identity_v2::ApparentRepoName::new("tools").unwrap(),
                        slug_identity_v2::CanonicalRepoName::new("bazel_tools").unwrap(),
                    );
                    wrong.repo_spec.rule_id.bzl_file = slug_identity_v2::ApparentLabel::parse(
                        "@tools//tools/res:winsdk_configure.bzl",
                    )
                    .unwrap()
                    .resolve(&mapping);
                }
                5 => {
                    let mut attributes = starlark_map::small_map::SmallMap::new();
                    attributes.insert(
                        compact_str::CompactString::new("unexpected"),
                        slug_bzlmod_v2::OverrideAttributeValue::None,
                    );
                    wrong.repo_spec.attributes = Arc::new(attributes);
                }
                _ => unreachable!(),
            }
            let wrong =
                namespace_test_epoch_at(&bodies, [Arc::new(wrong)], instance, "/parent/generation");
            assert!(
                !namespace_policy(&bodies, &d, Some(&wrong), &small_namespace_plan_matches)
                    .admits(&d.operation())
            );
            if matches!(owner, 3 | 4) {
                for path in [
                    "/parent/generation/REPO.bazel",
                    "/parent/generation/.bazelignore",
                ]
                .into_iter()
                .chain((owner == 4).then_some("/parent/generation/BUILD.bazel"))
                {
                    let marker = demand(path, PathObservationOperation::Lstat);
                    assert!(
                        !namespace_policy(
                            &bodies,
                            &marker,
                            Some(&wrong),
                            &small_namespace_plan_matches
                        )
                        .admits(&marker.operation())
                    );
                }
            }
        }
        assert_eq!(Arc::strong_count(&request), count);
        assert!(std::ptr::eq(
            epoch
                .borrowed_instance_for_namespace(&bodies.roots[0], instance, 32)
                .unwrap()
                .unwrap()
                .request(),
            request.as_ref()
        ));
        let weak = Arc::downgrade(&request);
        drop(epoch);
        drop(request);
        assert!(weak.upgrade().is_none());
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
    let bodies = namespace_test_bodies()
        .with_repo_marker_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
        .with_ignore_file_bytes(RegistryChoice::PreparedFile, CommandChoice::PublicProbe);
    let instance = slug_workspace_v2::PathObservationInstanceId::new(17);
    let a = Arc::new(namespace_test_request(&bodies, false));
    let b = Arc::new(namespace_test_request(&bodies, true));
    let epoch = namespace_test_epoch(&bodies, [a.clone()], instance);
    let request = |namespace, path, operation| {
        SourcePreparationNeeds::path(NeedPathObservations::singleton(PathObservationDemand::new(
            namespace,
            NormalizedAbsolutePath::new(path).unwrap(),
            operation,
        )))
    };
    let need = |namespace, path| request(namespace, path, PathObservationOperation::Lstat);
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
    for (owner, generation, fixed) in [
        (2, "/host-generation", "/host-generation/constraints.bzl"),
        (3, "/hub-generation", "/hub-generation/BUILD.bazel"),
        (4, "/winsdk-generation", "/winsdk-generation/toolchains.bzl"),
    ] {
        let host = Arc::new(generated_test_owner_request(
            &bodies,
            owner,
            small_generated_plan(&[
                (small_owner_first(owner), b"a", true),
                (small_owner_second(owner), b"b", true),
            ]),
        ));
        let host_weak = Arc::downgrade(&host);
        let host_instance = slug_workspace_v2::PathObservationInstanceId::new(19);
        let host_epoch =
            namespace_test_epoch_at(&bodies, [host.clone()], host_instance, generation);
        let mut host_need = need(
            PathObservationNamespace::Materialization(host_instance),
            fixed,
        )
        .try_union(&SourcePreparationNeeds::repository(host.as_ref().clone()))
        .unwrap();
        let repo_marker = if owner == 4 {
            "/winsdk-generation/REPO.bazel"
        } else {
            "/hub-generation/REPO.bazel"
        };
        if matches!(owner, 3 | 4) {
            host_need = host_need
                .try_union(&need(
                    PathObservationNamespace::Materialization(host_instance),
                    repo_marker,
                ))
                .unwrap();
        }
        let ignore_marker = if owner == 4 {
            "/winsdk-generation/.bazelignore"
        } else {
            "/hub-generation/.bazelignore"
        };
        if matches!(owner, 3 | 4) {
            host_need = host_need
                .try_union(&need(
                    PathObservationNamespace::Materialization(host_instance),
                    ignore_marker,
                ))
                .unwrap();
        }
        if owner == 4 {
            host_need = host_need
                .try_union(&need(
                    PathObservationNamespace::Materialization(host_instance),
                    "/winsdk-generation/BUILD.bazel",
                ))
                .unwrap();
        }
        assert!(admits(&host_need, Some(&host_epoch)).is_ok());
        let crossed = need(
            PathObservationNamespace::Materialization(host_instance),
            "/version-generation/version.bzl",
        );
        for excluded in [
            crossed,
            request(
                PathObservationNamespace::Materialization(host_instance),
                repo_marker,
                PathObservationOperation::FileBytes,
            ),
            request(
                PathObservationNamespace::Materialization(host_instance),
                ignore_marker,
                PathObservationOperation::FileBytes,
            ),
            need(
                PathObservationNamespace::Materialization(host_instance),
                if owner == 4 {
                    "/winsdk-generation/unknown"
                } else {
                    "/hub-generation/unknown"
                },
            ),
            need(
                PathObservationNamespace::Materialization(host_instance),
                "/old-generation/REPO.bazel",
            ),
            need(
                PathObservationNamespace::Materialization(instance),
                "/hub-generation/.bazelignore",
            ),
        ]
        .into_iter()
        .chain((owner == 4).then(|| {
            request(
                PathObservationNamespace::Materialization(host_instance),
                "/winsdk-generation/BUILD.bazel",
                PathObservationOperation::FileBytes,
            )
        })) {
            for crossed_host in [
                host_need.try_union(&excluded).unwrap(),
                excluded.try_union(&host_need).unwrap(),
            ] {
                let mut host_progress = 0;
                if admits(&crossed_host, Some(&host_epoch)).is_ok() {
                    host_progress += 1;
                }
                assert_eq!(host_progress, 0);
                let denied = admits(&crossed_host, Some(&host_epoch)).unwrap_err();
                assert_eq!(denied.family, "namespace");
                let (first, _) = bodies
                    .first_denied_path_with_namespace_matcher(&crossed_host, &|demand| {
                        namespace_policy(
                            &bodies,
                            demand,
                            Some(&host_epoch),
                            &small_namespace_plan_matches,
                        )
                        .admits(&demand.operation())
                    })
                    .unwrap();
                assert_eq!(denied.path.as_ref(), Some(first.path()));
                assert_eq!(
                    first.path(),
                    excluded
                        .path_observations()
                        .unwrap()
                        .demands()
                        .first()
                        .unwrap()
                        .path()
                );
            }
        }
        assert!(std::ptr::eq(
            host_epoch
                .borrowed_instance_for_namespace(&bodies.roots[0], host_instance, 32)
                .unwrap()
                .unwrap()
                .request(),
            host.as_ref()
        ));
        drop(host_epoch);
        drop(host);
        assert!(host_weak.upgrade().is_none());
    }
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
    let matches = small_namespace_plan_matches;
    eprintln!(
        "WP1040_SIGNATURE_LAYOUT signature={} cc={}",
        std::mem::size_of::<GeneratedFileSignature>(),
        std::mem::size_of_val(&GENERATED_CC_TOOLCHAINS_FILES)
    );
    eprintln!(
        "WP1056_SIGNATURE_LAYOUT signature={} shell={} apple={}",
        std::mem::size_of::<GeneratedFileSignature>(),
        std::mem::size_of_val(&GENERATED_SHELL_CONFIG_FILES),
        std::mem::size_of_val(&GENERATED_APPLE_TOOLCHAINS_FILES)
    );
    for owner in 0..3 {
        let shell = owner == 1;
        let apple = owner == 2;
        let configure = |bodies: Bodies| {
            if apple {
                bodies
                    .with_seventh(path("/license"))
                    .with_eighth(path("/shell"))
                    .with_ninth(path("/apple"))
                    .with_apple_support_import_sources(
                        RegistryChoice::PreparedFile,
                        CommandChoice::PublicProbe,
                    )
            } else if shell {
                bodies
                    .with_seventh(path("/license"))
                    .with_eighth(path("/shell"))
                    .with_rules_shell_import_sources(
                        RegistryChoice::PreparedFile,
                        CommandChoice::PublicProbe,
                    )
            } else {
                bodies
            }
        };
        let selected = if apple {
            &GENERATED_APPLE_TOOLCHAINS_FILES[..]
        } else if shell {
            &GENERATED_SHELL_CONFIG_FILES[..]
        } else {
            &GENERATED_CC_TOOLCHAINS_FILES[..]
        };
        let slot = if apple {
            5
        } else if shell {
            4
        } else {
            2
        };
        let crossed_slot = if apple { 4 } else { 5 };
        let (canonical, label, rule) = if apple {
            (
                APPLE_GENERATED_DIAGNOSTIC_OWNER.0,
                "@@apple_support+//crosstool:setup_internal.bzl",
                APPLE_GENERATED_DIAGNOSTIC_OWNER.4,
            )
        } else if shell {
            (
                "rules_shell++sh_configure+local_config_shell",
                "@@rules_shell+//shell/private/repositories:sh_config.bzl",
                "sh_config",
            )
        } else {
            GENERATED_CC_TOOLCHAINS_DIAGNOSTIC_TEST_OWNER
        };
        let (repo, package, target) = if apple {
            (
                APPLE_GENERATED_DIAGNOSTIC_OWNER.1,
                APPLE_GENERATED_DIAGNOSTIC_OWNER.2,
                APPLE_GENERATED_DIAGNOSTIC_OWNER.3,
            )
        } else if shell {
            (
                "rules_shell+",
                "shell/private/repositories",
                "sh_config.bzl",
            )
        } else {
            ("rules_cc+", "cc/private/toolchain", "cc_configure.bzl")
        };
        let bodies = configure(namespace_test_bodies())
            .with_repo_marker_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
            .with_ignore_file_bytes(RegistryChoice::PreparedFile, CommandChoice::PublicProbe);
        assert!(bodies.repo_marker_metadata);
        assert!(bodies.ignore_file_bytes);
        let request_for_plan = |plan| {
            let mut request = generated_globals_request(&bodies, plan);
            request.id.canonical_repo =
                slug_identity_v2::CanonicalRepoName::new(canonical).unwrap();
            request.repo_spec.rule_id.bzl_file = CanonicalLabel::parse(label).unwrap();
            request.repo_spec.rule_id.rule_name = rule.into();
            request.repo_spec.attributes = Arc::new(starlark_map::small_map::SmallMap::new());
            request
        };
        let tiny: &[(&str, &[u8], bool)] = &[("BUILD", b"a", true)];
        let valid =
            SourcePreparationNeeds::repository(request_for_plan(small_generated_plan(tiny)));
        let original = valid.repository_materializations().values().next().unwrap();
        let weak = Arc::downgrade(original);
        assert!(std::ptr::eq(
            bodies
                .generated_features_owner_signatures(original)
                .unwrap(),
            selected
        ));
        assert!(
            bodies
                .admit_with_plan_matcher(&valid, &|plan, signatures| {
                    let RepositoryMaterializationKind::GeneratedFileEffects(original_plan) =
                        &original.kind
                    else {
                        panic!("original generated plan");
                    };
                    assert!(std::ptr::eq(plan, original_plan));
                    assert!(std::ptr::eq(signatures, selected));
                    matches(plan, signatures)
                })
                .is_ok()
        );
        assert!(bodies.admit(&valid).is_err());
        for registry in [RegistryChoice::DefaultBcr, RegistryChoice::PreparedFile] {
            for command in [CommandChoice::Modules, CommandChoice::PublicProbe] {
                let configured = make()
                    .with_fourth(path("/features"))
                    .with_fifth(path("/skylib"))
                    .with_sixth(path("/cc"))
                    .with_seventh(path("/license"))
                    .with_eighth(path("/shell"))
                    .with_ninth(path("/apple"))
                    .with_apple_support_import_sources(registry, command)
                    .with_rules_shell_import_sources(registry, command)
                    .with_rules_cc_import_sources(registry, command)
                    .with_generated_features_file_effects(registry, command);
                assert_eq!(
                    configured.admit_with_plan_matcher(&valid, &matches).is_ok(),
                    matches!(registry, RegistryChoice::PreparedFile)
                        && matches!(command, CommandChoice::PublicProbe)
                );
            }
        }
        let mut independent = configure(namespace_test_bodies());
        if apple {
            independent.rules_cc_import_sources = false;
            independent.rules_shell_import_sources = false;
        } else if shell {
            independent.rules_cc_import_sources = false;
        } else {
            independent.rules_shell_import_sources = false;
        }
        assert!(
            independent
                .admit_with_plan_matcher(&valid, &matches)
                .is_ok()
        );
        for index in 0..5 {
            let mut closed = configure(namespace_test_bodies());
            match index {
                0 => closed.generated_features_file_effects = false,
                1 => {
                    if apple {
                        closed.apple_support_import_sources = false;
                    } else if shell {
                        closed.rules_shell_import_sources = false;
                    } else {
                        closed.rules_cc_import_sources = false;
                    }
                }
                2 => closed.additional[slot] = None,
                3 => closed.roots[0] = path("/other"),
                4 => closed.additional[crossed_slot] = closed.additional[slot].take(),
                _ => unreachable!(),
            }
            let calls = std::cell::Cell::new(0);
            assert!(
                closed
                    .admit_with_plan_matcher(&valid, &|_, _| {
                        calls.set(calls.get() + 1);
                        true
                    })
                    .is_err()
            );
            assert_eq!(calls.get(), 0);
            if index == 4 {
                closed.additional[slot] = closed.additional[crossed_slot].take();
                assert!(closed.admit_with_plan_matcher(&valid, &matches).is_ok());
            }
        }
        for index in 0..8 {
            let mut wrong = request_for_plan(small_generated_plan(tiny));
            match index {
                0 => wrong.id.workspace = path("/other"),
                1 => {
                    wrong.id.canonical_repo =
                        slug_identity_v2::CanonicalRepoName::new("other+").unwrap()
                }
                2 => {
                    wrong.repo_spec.rule_id.bzl_file =
                        CanonicalLabel::parse(&format!("@@other+//{package}:{target}")).unwrap()
                }
                3 => {
                    wrong.repo_spec.rule_id.bzl_file = CanonicalLabel::parse(&format!(
                        "@@{repo}//{}:{target}",
                        if apple {
                            "other"
                        } else if shell {
                            "shell/private"
                        } else {
                            "cc/private"
                        }
                    ))
                    .unwrap()
                }
                4 => {
                    wrong.repo_spec.rule_id.bzl_file =
                        CanonicalLabel::parse(&format!("@@{repo}//{package}:other.bzl")).unwrap()
                }
                5 => wrong.repo_spec.rule_id.rule_name = "other".into(),
                6 => {
                    let mut mapping = slug_identity_v2::RepositoryMapping::new(
                        slug_identity_v2::RepositoryMappingId::new("mapped").unwrap(),
                    );
                    mapping.insert(
                        slug_identity_v2::ApparentRepoName::new("cc").unwrap(),
                        slug_identity_v2::CanonicalRepoName::new(repo).unwrap(),
                    );
                    wrong.repo_spec.rule_id.bzl_file =
                        slug_identity_v2::ApparentLabel::parse(&format!("@cc//{package}:{target}"))
                            .unwrap()
                            .resolve(&mapping);
                }
                7 => {
                    Arc::get_mut(&mut wrong.repo_spec.attributes)
                        .unwrap()
                        .insert("unexpected".into(), OverrideAttributeValue::None);
                }
                _ => unreachable!(),
            }
            let calls = std::cell::Cell::new(0);
            assert!(
                bodies
                    .admit_with_plan_matcher(&SourcePreparationNeeds::repository(wrong), &|_, _| {
                        calls.set(calls.get() + 1);
                        true
                    })
                    .is_err()
            );
            assert_eq!(calls.get(), 0);
        }
        let one = [("BUILD", 1, true, sha2::Sha256::digest(b"a").into())];
        assert!(generated_features_plan_matches(
            &small_generated_plan(tiny),
            &one
        ));
        for unsupported in [&one[..0], &[one[0]; 4][..]] {
            assert!(!generated_features_plan_matches(
                &small_generated_plan(tiny),
                unsupported
            ));
        }
        let invalid: &[&[(&str, &[u8], bool)]] = &[
            &[],
            &[("BUILD", b"x", true)],
            &[("BUILD", b"aa", true)],
            &[("BUILD", b"a", false)],
            &[("build", b"a", true)],
            &[("BUILD.bak", b"a", true)],
            &[("BUILD/child", b"a", true)],
            &[("BUILD", b"a", true), ("extra", b"", true)],
        ];
        for rows in invalid {
            assert!(!generated_features_plan_matches(
                &small_generated_plan(rows),
                &one
            ));
        }
        for (symlink, trailing) in [(false, false), (true, false), (false, true), (true, true)] {
            let mut builder = slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan::builder();
            if trailing {
                builder
                    .push("BUILD".into(), Arc::from(&b"a"[..]), true)
                    .unwrap();
            }
            if symlink {
                builder
                    .push_symlink(
                        if trailing { "extra" } else { "BUILD" }.into(),
                        slug_bzlmod_v2::HostRepositoryLabelPathValue::new(
                            path("/source"),
                            PathObservationNamespace::Host,
                        )
                        .unwrap(),
                        Arc::from(&b"a"[..]),
                    )
                    .unwrap();
            } else {
                builder
                    .push_archive(
                        Arc::from([compact_str::CompactString::new(
                            "https://example.invalid/a.tar.xz",
                        )]),
                        [0; 32],
                        "prefix".into(),
                    )
                    .unwrap();
            }
            assert!(!generated_features_plan_matches(&builder.finish(), &one));
        }
        let instance = slug_workspace_v2::PathObservationInstanceId::new(23);
        let epoch =
            namespace_test_epoch_at(&bodies, [original.clone()], instance, "/parent/generation");
        for entries in [32, 33] {
            let bounded = slug_bzlmod_v2::RepositoryMaterializationResultEpoch::new(
                bodies.roots[0].dupe(),
                (0..entries).map(|index| {
                    let request = if index == 0 {
                        original.clone()
                    } else {
                        let mut other = original.as_ref().clone();
                        other.id.canonical_repo =
                            slug_identity_v2::CanonicalRepoName::new(&format!("other{index}+"))
                                .unwrap();
                        Arc::new(other)
                    };
                    slug_bzlmod_v2::RepositoryMaterializationEpochEntry {
                        request,
                        result: slug_bzlmod_v2::RepositoryMaterializationResult::Success(
                            slug_bzlmod_v2::RepositoryMaterializationSuccess::Immutable {
                                source_identity: Arc::from("test"),
                                generation_root: PathBuf::from("/parent/generation"),
                                observation_instance: if index == 0 {
                                    instance
                                } else {
                                    slug_workspace_v2::PathObservationInstanceId::new(24 + index)
                                },
                            },
                        ),
                    }
                }),
            )
            .unwrap();
            let demand = PathObservationDemand::new(
                PathObservationNamespace::Materialization(instance),
                path("/parent/generation/BUILD"),
                PathObservationOperation::FileBytes,
            );
            assert_eq!(
                namespace_policy(&bodies, &demand, Some(&bounded), &matches)
                    .admits(&demand.operation()),
                entries == 32
            );
        }
        let need = |p, op| {
            SourcePreparationNeeds::path(NeedPathObservations::singleton(match op {
                PathObservationOperation::WindowsLongPath => {
                    PathObservationDemand::windows_long_path(path(p), Arc::from([b'x' as u16]))
                }
                PathObservationOperation::WindowsOptionPathLongName => {
                    PathObservationDemand::windows_option_path_long_name(
                        path(p),
                        Arc::from([b'x' as u16]),
                    )
                }
                _ => PathObservationDemand::new(
                    PathObservationNamespace::Materialization(instance),
                    path(p),
                    op,
                ),
            }))
        };
        let admit = |n: &SourcePreparationNeeds, e| {
            bodies.admit_with_matchers(n, &matches, &|d| {
                namespace_policy(&bodies, d, e, &matches).admits(&d.operation())
            })
        };
        let marker = need(
            "/parent/generation/REPO.bazel",
            PathObservationOperation::Lstat,
        );
        let original_marker = marker
            .path_observations()
            .unwrap()
            .demands()
            .first()
            .unwrap();
        let ignore = need(
            "/parent/generation/.bazelignore",
            PathObservationOperation::Lstat,
        );
        let original_ignore = ignore
            .path_observations()
            .unwrap()
            .demands()
            .first()
            .unwrap();
        for p in [
            "/",
            "/parent",
            "/parent/generation",
            "/parent/generation/BUILD",
            "/parent/generation/BUILD.bazel",
            "/parent/generation/build.bazel",
            "/parent/generation/BUILD.bazel.bak",
            "/parent/generation/BUILD.bazel/child",
            "/parent/generation/REPO.bazel",
            "/parent/generation/repo.bazel",
            "/parent/generation/REPO.bazel.bak",
            "/parent/generation/REPO.bazel/child",
            "/parent/generation/.bazelignore",
            "/parent/generation/.BAZELIGNORE",
            "/parent/generation/.bazelignore.bak",
            "/parent/generation/.bazelignore/child",
            "/parent/generation/BUILD/child",
            "/parent/generation/build",
            "/parent/generation/BUILD.bak",
            "/other/BUILD",
        ] {
            for op in [
                PathObservationOperation::Lstat,
                PathObservationOperation::FileBytes,
                PathObservationOperation::FileDigest,
                PathObservationOperation::DirectoryEntries,
                PathObservationOperation::ReadLink,
                PathObservationOperation::WindowsLongPath,
                PathObservationOperation::WindowsOptionPathLongName,
            ] {
                assert_eq!(
                    admit(&need(p, op.dupe()), Some(&epoch)).is_ok(),
                    (op == PathObservationOperation::Lstat
                        && [
                            "/",
                            "/parent",
                            "/parent/generation",
                            "/parent/generation/BUILD",
                        ]
                        .contains(&p)
                        || op == PathObservationOperation::Lstat
                            && [
                                "/parent/generation/REPO.bazel",
                                "/parent/generation/.bazelignore",
                                "/parent/generation/BUILD.bazel"
                            ]
                            .contains(&p))
                        || (op == PathObservationOperation::FileBytes
                            && p == "/parent/generation/BUILD")
                );
            }
        }
        let leaf = need(
            "/parent/generation/BUILD",
            PathObservationOperation::FileBytes,
        );
        let features = SourcePreparationNeeds::repository(namespace_test_request(&bodies, false));
        let candidate = need(
            "/parent/generation/BUILD.bazel",
            PathObservationOperation::Lstat,
        );
        let base_own = valid.try_union(&leaf).unwrap();
        let own = if apple {
            base_own
                .try_union(&marker)
                .unwrap()
                .try_union(&ignore)
                .unwrap()
                .try_union(&candidate)
                .unwrap()
                .try_union(&features)
                .unwrap()
        } else if shell {
            base_own
                .try_union(&marker)
                .unwrap()
                .try_union(&ignore)
                .unwrap()
                .try_union(&candidate)
                .unwrap()
                .try_union(&features)
                .unwrap()
        } else {
            base_own
                .try_union(&marker)
                .unwrap()
                .try_union(&ignore)
                .unwrap()
                .try_union(&candidate)
                .unwrap()
                .try_union(&features)
                .unwrap()
        };
        drop(base_own);
        for cohort in [
            own.try_union(&features).unwrap(),
            features.try_union(&own).unwrap(),
        ] {
            assert!(admit(&cohort, Some(&epoch)).is_ok());
        }
        let mut ambiguous = request_for_plan(small_generated_plan(tiny));
        ambiguous.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new("other+").unwrap();
        let mut nonempty = request_for_plan(small_generated_plan(tiny));
        Arc::get_mut(&mut nonempty.repo_spec.attributes)
            .unwrap()
            .insert("unexpected".into(), OverrideAttributeValue::None);
        let unavailable_epochs = [
            None,
            Some(namespace_test_epoch_at(
                &bodies,
                [original.clone()],
                instance,
                "/stale",
            )),
            Some(namespace_test_epoch_at(
                &bodies,
                [original.clone(), Arc::new(ambiguous)],
                instance,
                "/parent/generation",
            )),
            Some(namespace_test_epoch_at(
                &bodies,
                [Arc::new(request_for_plan(small_generated_plan(&[(
                    "BUILD", b"x", true,
                )])))],
                instance,
                "/parent/generation",
            )),
            Some(namespace_test_epoch_at(
                &bodies,
                [Arc::new(nonempty)],
                instance,
                "/parent/generation",
            )),
            Some(namespace_test_epoch_at(
                &bodies,
                [],
                instance,
                "/parent/generation",
            )),
            Some(namespace_test_epoch_at(
                &bodies,
                [original.clone()],
                slug_workspace_v2::PathObservationInstanceId::new(0),
                "/parent/generation",
            )),
            Some(namespace_test_epoch_at(
                &bodies,
                [original.clone()],
                instance,
                "relative-generation",
            )),
        ];
        for unavailable in &unavailable_epochs {
            assert!(admit(&leaf, unavailable.as_ref()).is_err());
            assert!(admit(&marker, unavailable.as_ref()).is_err());
            assert!(admit(&ignore, unavailable.as_ref()).is_err());
            assert!(admit(&candidate, unavailable.as_ref()).is_err());
        }
        let original_candidate = candidate
            .path_observations()
            .unwrap()
            .demands()
            .first()
            .unwrap();
        for repo_enabled in [false, true] {
            for ignore_enabled in [false, true] {
                let mut configured = configure(namespace_test_bodies())
                    .with_repo_marker_metadata(
                        RegistryChoice::PreparedFile,
                        CommandChoice::PublicProbe,
                    )
                    .with_ignore_file_bytes(
                        RegistryChoice::PreparedFile,
                        CommandChoice::PublicProbe,
                    );
                configured.repo_marker_metadata = repo_enabled;
                configured.ignore_file_bytes = ignore_enabled;
                for (e, own_plan) in [
                    (Some(&epoch), true),
                    (unavailable_epochs[3].as_ref(), false),
                ] {
                    for (demand, enabled, class) in [
                        (original_marker, repo_enabled, "repo_marker"),
                        (original_ignore, ignore_enabled, "ignore_marker"),
                        (original_candidate, true, "package_marker"),
                    ] {
                        let policy = namespace_policy(&configured, demand, e, &matches);
                        assert_eq!(policy.path_class, if enabled { class } else { "other" });
                        assert_eq!(policy.owner_plan_allowed, own_plan);
                        assert_eq!(
                            policy.admits(&PathObservationOperation::Lstat),
                            enabled && own_plan
                        );
                        assert!(!policy.admits(&PathObservationOperation::FileBytes));
                        for operation in [
                            PathObservationOperation::FileDigest,
                            PathObservationOperation::DirectoryEntries,
                            PathObservationOperation::ReadLink,
                        ] {
                            assert!(!policy.admits(&operation));
                        }
                        if shell || apple {
                            let marker_need = match class {
                                "repo_marker" => &marker,
                                "ignore_marker" => &ignore,
                                _ => &candidate,
                            };
                            for mixed in [
                                valid.try_union(marker_need).unwrap(),
                                marker_need.try_union(&valid).unwrap(),
                            ] {
                                assert_eq!(
                                    configured
                                        .admit_with_matchers(&mixed, &matches, &|d| {
                                            namespace_policy(&configured, d, e, &matches)
                                                .admits(&d.operation())
                                        })
                                        .is_ok(),
                                    enabled && own_plan
                                );
                                assert!(
                                    mixed
                                        .repository_materializations()
                                        .values()
                                        .any(|r| Arc::ptr_eq(r, original))
                                );
                                if !enabled || !own_plan {
                                    let (first, _) = configured
                                        .first_denied_path_with_namespace_matcher(&mixed, &|d| {
                                            namespace_policy(&configured, d, e, &matches)
                                                .admits(&d.operation())
                                        })
                                        .unwrap();
                                    assert!(std::ptr::eq(
                                        first,
                                        mixed
                                            .path_observations()
                                            .unwrap()
                                            .demands()
                                            .first()
                                            .unwrap()
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
        for namespace in [
            PathObservationNamespace::Host,
            PathObservationNamespace::Materialization(
                slug_workspace_v2::PathObservationInstanceId::new(24),
            ),
            PathObservationNamespace::Materialization(
                slug_workspace_v2::PathObservationInstanceId::new(0),
            ),
        ] {
            let demand = PathObservationDemand::new(
                namespace.dupe(),
                path("/parent/generation/BUILD"),
                PathObservationOperation::FileBytes,
            );
            assert!(
                !namespace_policy(&bodies, &demand, Some(&epoch), &matches)
                    .admits(&demand.operation())
            );
            for original in [original_marker, original_ignore, original_candidate] {
                let marker = PathObservationDemand::new(
                    namespace.dupe(),
                    original.path().dupe(),
                    PathObservationOperation::Lstat,
                );
                assert!(
                    !namespace_policy(&bodies, &marker, Some(&epoch), &matches)
                        .admits(&marker.operation())
                );
            }
        }
        let mut unknown = request_for_plan(small_generated_plan(tiny));
        unknown.id.canonical_repo = slug_identity_v2::CanonicalRepoName::new("unknown+").unwrap();
        let bad = [
            SourcePreparationNeeds::repository(unknown),
            need(
                "/parent/generation/unknown",
                PathObservationOperation::Lstat,
            ),
        ];
        for bad in &bad {
            for mixed in [own.try_union(bad).unwrap(), bad.try_union(&own).unwrap()] {
                let mut progress = 0;
                if admit(&mixed, Some(&epoch)).is_ok() {
                    progress += 1;
                }
                assert_eq!(progress, 0);
                assert!(
                    mixed
                        .repository_materializations()
                        .values()
                        .any(|r| Arc::ptr_eq(r, original))
                );
                if bad.repository_materializations().is_empty() {
                    let (first, _) = bodies
                        .first_denied_path_with_namespace_matcher(&mixed, &|d| {
                            namespace_policy(&bodies, d, Some(&epoch), &matches)
                                .admits(&d.operation())
                        })
                        .unwrap();
                    assert!(std::ptr::eq(
                        first,
                        mixed
                            .path_observations()
                            .unwrap()
                            .demands()
                            .iter()
                            .find(|d| d.path()
                                == bad
                                    .path_observations()
                                    .unwrap()
                                    .demands()
                                    .first()
                                    .unwrap()
                                    .path())
                            .unwrap()
                    ));
                } else {
                    let (first, _) = bodies
                        .first_denied_materialization_with_plan_matcher(&mixed, &matches)
                        .unwrap();
                    assert!(
                        mixed
                            .repository_materializations()
                            .values()
                            .any(|r| std::ptr::eq(r.as_ref(), first))
                    );
                }
            }
        }
        drop(unavailable_epochs);
        drop(own);
        drop(epoch);
        assert_eq!(Arc::strong_count(original), 1);
        drop(valid);
        assert!(weak.upgrade().is_none());
    }
    for owner in 0..GENERATED_DIAGNOSTIC_TEST_OWNERS.len() {
        let first = small_owner_first(owner);
        let second = small_owner_second(owner);
        let rows: &[(&str, &[u8], bool)] = &[(first, b"a", true), (second, b"b", true)];
        let request_for_plan =
            |bodies: &Bodies, plan| generated_test_owner_request(bodies, owner, plan);
        let bodies = make()
            .with_fourth(path("/features"))
            .with_fifth(path("/skylib"))
            .with_sixth(path("/cc"))
            .with_rules_cc_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
            .with_bazel_features_import_sources(
                RegistryChoice::PreparedFile,
                CommandChoice::PublicProbe,
            )
            .with_platforms_host_import_sources(
                RegistryChoice::PreparedFile,
                CommandChoice::PublicProbe,
            )
            .with_rules_rust_defs_import_sources(
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
                    .with_fifth(path("/skylib"))
                    .with_sixth(path("/cc"))
                    .with_rules_cc_import_sources(registry, command)
                    .with_bazel_features_import_sources(registry, command)
                    .with_platforms_host_import_sources(registry, command)
                    .with_rules_rust_defs_import_sources(registry, command)
                    .with_generated_features_file_effects(registry, command);
                assert_eq!(
                    configured.admit_with_plan_matcher(&needs, &matches).is_ok(),
                    matches!(registry, RegistryChoice::PreparedFile)
                        && matches!(command, CommandChoice::PublicProbe)
                );
            }
        }
        if owner >= 2 {
            for mask in 0..if owner == 4 { 4 } else { 3 } {
                let mut closed = namespace_test_bodies();
                match mask {
                    0 if owner == 2 => closed.platforms_host_import_sources = false,
                    0 if owner == 4 => closed.rules_cc_import_sources = false,
                    0 => closed.rules_rust_defs_import_sources = false,
                    1 => closed.generated_features_file_effects = false,
                    2 => closed.roots[0] = path("/other"),
                    3 => closed.additional[2] = None,
                    _ => unreachable!(),
                }
                let shape_calls = std::cell::Cell::new(0);
                assert!(
                    closed
                        .admit_with_plan_matcher(&needs, &|_, _| {
                            shape_calls.set(shape_calls.get() + 1);
                            true
                        })
                        .is_err()
                );
                assert_eq!(shape_calls.get(), 0);
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
        if owner < 2 {
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
        }
        // A permissive shape closure cannot bypass any typed owner field or mapping provenance.
        for index in 0..if owner == 4 { 9 } else { 8 } {
            let mut request = request_for_plan(&bodies, small_generated_plan(rows));
            match index {
                0 => request.id.workspace = path("/other"),
                1 => {
                    request.id.canonical_repo =
                        slug_identity_v2::CanonicalRepoName::new("other+").unwrap()
                }
                2 | 3 | 4 => {
                    let label = &request.repo_spec.rule_id.bzl_file;
                    request.repo_spec.rule_id.bzl_file = CanonicalLabel::parse(&format!(
                        "@@{}//{}:{}",
                        if index == 2 {
                            "other"
                        } else {
                            label.package().repo().as_str()
                        },
                        if index == 3 {
                            "other"
                        } else {
                            label.package().package().as_str()
                        },
                        if index == 4 {
                            "other.bzl"
                        } else {
                            label.target().as_str()
                        },
                    ))
                    .unwrap();
                }
                5 => request.repo_spec.rule_id.rule_name = "other".into(),
                6 => {
                    let label = &request.repo_spec.rule_id.bzl_file;
                    let mut mapping = slug_identity_v2::RepositoryMapping::new(
                        slug_identity_v2::RepositoryMappingId::new("mapped").unwrap(),
                    );
                    mapping.insert(
                        slug_identity_v2::ApparentRepoName::new("owner").unwrap(),
                        slug_identity_v2::CanonicalRepoName::new(label.package().repo().as_str())
                            .unwrap(),
                    );
                    request.repo_spec.rule_id.bzl_file =
                        slug_identity_v2::ApparentLabel::parse(&format!(
                            "@owner//{}:{}",
                            label.package().package().as_str(),
                            label.target().as_str(),
                        ))
                        .unwrap()
                        .resolve(&mapping);
                }
                7 if matches!(owner, 2 | 4) => {
                    Arc::get_mut(&mut request.repo_spec.attributes)
                        .unwrap()
                        .insert(
                            compact_str::CompactString::new("unexpected"),
                            OverrideAttributeValue::String("opaque".into()),
                        );
                }
                7 | 8 => request.kind = RepositoryMaterializationKind::Immutable,
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
        for (other, &(canonical, label, rule)) in
            GENERATED_DIAGNOSTIC_TEST_OWNERS.iter().enumerate()
        {
            if other == owner {
                continue;
            }
            for field in 0..3 {
                let mut crossed = request_for_plan(&bodies, small_generated_plan(rows));
                match field {
                    0 => {
                        crossed.id.canonical_repo =
                            slug_identity_v2::CanonicalRepoName::new(canonical).unwrap()
                    }
                    1 => crossed.repo_spec.rule_id.bzl_file = CanonicalLabel::parse(label).unwrap(),
                    2 => crossed.repo_spec.rule_id.rule_name = rule.into(),
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
        }
        for other in (0..GENERATED_DIAGNOSTIC_TEST_OWNERS.len()).filter(|other| *other != owner) {
            let crossed_shape = request_for_plan(
                &bodies,
                small_generated_plan(&[
                    (small_owner_first(other), b"a", true),
                    (small_owner_second(other), b"b", true),
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
        }
        let invalid: &[&[(&str, &[u8], bool)]] = &[
            &[],
            &[(first, b"a", true)],
            &[
                (first, b"a", true),
                (second, b"b", true),
                ("extra", b"", false),
            ],
            &[(second, b"b", true), (first, b"a", true)],
            &[
                (if owner == 4 { "unknown" } else { "BUILD" }, b"a", true),
                (second, b"b", true),
            ],
            &[(first, b"aa", true), (second, b"b", true)],
            &[(first, b"x", true), (second, b"b", true)],
            &[(first, b"a", false), (second, b"b", true)],
            &[(first, b"a", true), (second, b"x", true)],
            &[(first, b"a", true), (second, b"b", false)],
            &[(first, b"a", true), (second, b"bb", true)],
            &[(first, b"a", true), ("unknown.bzl", b"b", true)],
        ];
        for rows in invalid {
            let needs = SourcePreparationNeeds::repository(request_for_plan(
                &bodies,
                small_generated_plan(rows),
            ));
            assert!(bodies.admit_with_plan_matcher(&needs, &matches).is_err());
        }
        if owner == 4 {
            // Source-defined literal is fixture scratch only; native retains just the signatures.
            let toolchains: &[u8] = b"# Auto-generated by winsdk_configure.bzl\n\ndef register_local_rc_exe_toolchains():\n    pass\n";
            assert_eq!(toolchains.len(), 91);
            let fixed = [
                ("BUILD", &b""[..], false),
                ("toolchains.bzl", toolchains, false),
            ];
            let complete = SourcePreparationNeeds::repository(request_for_plan(
                &bodies,
                small_generated_plan(&fixed),
            ));
            let original = complete
                .repository_materializations()
                .values()
                .next()
                .unwrap();
            let weak = Arc::downgrade(original);
            assert!(bodies.admit(&complete).is_ok());
            let mut changed = toolchains.to_vec();
            changed[0] ^= 1;
            assert_eq!(changed.len(), toolchains.len());
            for rows in [
                [
                    ("BUILD", &b""[..], false),
                    ("toolchains.bzl", changed.as_slice(), false),
                ],
                [
                    ("BUILD", &b"x"[..], false),
                    ("toolchains.bzl", toolchains, false),
                ],
                [
                    ("BUILD", &b""[..], true),
                    ("toolchains.bzl", toolchains, false),
                ],
                [
                    ("BUILD", &b""[..], false),
                    ("toolchains.bzl", toolchains, true),
                ],
            ] {
                let mismatch = SourcePreparationNeeds::repository(request_for_plan(
                    &bodies,
                    small_generated_plan(&rows),
                ));
                let denied = bodies.admit(&mismatch).unwrap_err();
                let view = bodies
                    .generated_plan_diagnostic(&mismatch, &denied)
                    .unwrap();
                let original = mismatch
                    .repository_materializations()
                    .values()
                    .next()
                    .unwrap();
                assert!(std::ptr::eq(view.request, original.as_ref()));
                let RepositoryMaterializationKind::GeneratedFileEffects(plan) = &original.kind
                else {
                    unreachable!()
                };
                assert!(std::ptr::eq(view.plan, plan));
            }
            drop(complete);
            assert!(weak.upgrade().is_none());
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
                bodies
                    .generated_features_owner_signatures(&request_for_plan(
                        &bodies,
                        small_generated_plan(rows)
                    ))
                    .unwrap()
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
    let bodies = namespace_test_bodies()
        .with_seventh(path("/license"))
        .with_eighth(path("/shell"))
        .with_ninth(path("/apple"))
        .with_apple_support_import_sources(
            RegistryChoice::PreparedFile,
            CommandChoice::PublicProbe,
        );
    let matches = small_namespace_plan_matches;
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
    let host = SourcePreparationNeeds::repository(generated_test_owner_request(
        &bodies,
        2,
        small_generated_plan(&[("BUILD.bazel", b"a", true), ("constraints.bzl", b"b", true)]),
    ));
    let host_original = host.repository_materializations().values().next().unwrap();
    let host_weak = Arc::downgrade(host_original);
    let host_count = Arc::strong_count(host_original);
    let hub = SourcePreparationNeeds::repository(generated_test_owner_request(
        &bodies,
        3,
        small_generated_plan(&[("WORKSPACE.bazel", b"a", true), ("BUILD.bazel", b"b", true)]),
    ));
    let hub_original = hub.repository_materializations().values().next().unwrap();
    let hub_weak = Arc::downgrade(hub_original);
    let hub_count = Arc::strong_count(hub_original);
    let winsdk = SourcePreparationNeeds::repository(generated_test_owner_request(
        &bodies,
        4,
        small_generated_plan(&[("BUILD", b"a", true), ("toolchains.bzl", b"b", true)]),
    ));
    let winsdk_original = winsdk
        .repository_materializations()
        .values()
        .next()
        .unwrap();
    let winsdk_weak = Arc::downgrade(winsdk_original);
    let winsdk_count = Arc::strong_count(winsdk_original);
    let mut apple_request =
        generated_globals_request(&bodies, small_generated_plan(&[("BUILD", b"a", true)]));
    apple_request.id.canonical_repo =
        slug_identity_v2::CanonicalRepoName::new(APPLE_GENERATED_DIAGNOSTIC_OWNER.0).unwrap();
    apple_request.repo_spec.rule_id.bzl_file =
        CanonicalLabel::parse("@@apple_support+//crosstool:setup_internal.bzl").unwrap();
    apple_request.repo_spec.rule_id.rule_name = APPLE_GENERATED_DIAGNOSTIC_OWNER.4.into();
    apple_request.repo_spec.attributes = Arc::new(starlark_map::small_map::SmallMap::new());
    let apple = SourcePreparationNeeds::repository(apple_request);
    let apple_original = apple.repository_materializations().values().next().unwrap();
    let apple_weak = Arc::downgrade(apple_original);
    let apple_count = Arc::strong_count(apple_original);
    for cohort in [
        valid
            .try_union(&version)
            .unwrap()
            .try_union(&host)
            .unwrap()
            .try_union(&hub)
            .unwrap()
            .try_union(&winsdk)
            .unwrap()
            .try_union(&apple)
            .unwrap(),
        apple
            .try_union(&winsdk)
            .unwrap()
            .try_union(&hub)
            .unwrap()
            .try_union(&host)
            .unwrap()
            .try_union(&version)
            .unwrap()
            .try_union(&valid)
            .unwrap(),
    ] {
        assert!(bodies.admit_with_plan_matcher(&cohort, &matches).is_ok());
        for held in [
            original,
            version_original,
            host_original,
            hub_original,
            winsdk_original,
            apple_original,
        ] {
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
    assert_eq!(Arc::strong_count(host_original), host_count);
    assert_eq!(Arc::strong_count(hub_original), hub_count);
    assert_eq!(Arc::strong_count(winsdk_original), winsdk_count);
    assert_eq!(Arc::strong_count(apple_original), apple_count);
    let cohort = valid
        .try_union(&version)
        .unwrap()
        .try_union(&host)
        .unwrap()
        .try_union(&hub)
        .unwrap()
        .try_union(&winsdk)
        .unwrap()
        .try_union(&apple)
        .unwrap();
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
    let changed_host = SourcePreparationNeeds::repository(generated_test_owner_request(
        &bodies,
        2,
        small_generated_plan(&[("BUILD.bazel", b"a", true), ("constraints.bzl", b"x", true)]),
    ));
    let changed_hub = SourcePreparationNeeds::repository(generated_test_owner_request(
        &bodies,
        3,
        small_generated_plan(&[("WORKSPACE.bazel", b"a", true), ("BUILD.bazel", b"x", true)]),
    ));
    let changed_winsdk = SourcePreparationNeeds::repository(generated_test_owner_request(
        &bodies,
        4,
        small_generated_plan(&[("BUILD", b"a", true), ("toolchains.bzl", b"x", true)]),
    ));
    let mut changed_apple = apple_original.as_ref().clone();
    changed_apple.kind = RepositoryMaterializationKind::GeneratedFileEffects(small_generated_plan(
        &[("BUILD", b"x", true)],
    ));
    let changed_apple = SourcePreparationNeeds::repository(changed_apple);
    for changed in [
        changed,
        changed_version,
        changed_host,
        changed_hub,
        changed_winsdk,
        changed_apple,
    ] {
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
    assert_eq!(Arc::strong_count(apple_original), apple_count);
    drop(apple);
    assert!(apple_weak.upgrade().is_none());

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
    let features = valid
        .try_union(&version)
        .unwrap()
        .try_union(&host)
        .unwrap()
        .try_union(&hub)
        .unwrap()
        .try_union(&winsdk)
        .unwrap();
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
    drop(winsdk);
    assert!(winsdk_weak.upgrade().is_none());
    drop(hub);
    assert!(hub_weak.upgrade().is_none());
    drop(host);
    assert!(host_weak.upgrade().is_none());
    drop(version);
    drop(valid);
    assert!(weak.upgrade().is_none());
    assert!(version_weak.upgrade().is_none());
}

const GENERATED_DIAGNOSTIC_TEST_OWNERS: [(&str, &str, &str); 5] = [
    (
        "bazel_features++version_extension+bazel_features_globals",
        "@@bazel_features+//private:globals_repo.bzl",
        "globals_repo",
    ),
    (
        "bazel_features++version_extension+bazel_features_version",
        "@@bazel_features+//private:version_repo.bzl",
        "version_repo",
    ),
    (
        "platforms+host_platform+host_platform",
        "@@platforms//host:extension.bzl",
        "host_platform_repo",
    ),
    GENERATED_HUB_DIAGNOSTIC_TEST_OWNER,
    (
        "bazel_tools+winsdk_configure+local_config_winsdk",
        "@@bazel_tools//tools/res:winsdk_configure.bzl",
        "winsdk_configure",
    ),
];
const GENERATED_HUB_DIAGNOSTIC_TEST_OWNER: (&str, &str, &str) = (
    "rules_rust++rust+rust_toolchains",
    "@@rules_rust+//rust/private:repository_utils.bzl",
    "toolchain_repository_hub",
);
const GENERATED_CC_TOOLCHAINS_DIAGNOSTIC_TEST_OWNER: (&str, &str, &str) = (
    "rules_cc++cc_configure_extension+local_config_cc_toolchains",
    "@@rules_cc+//cc/private/toolchain:cc_configure.bzl",
    "cc_autoconf_toolchains",
);

#[test]
fn generated_plan_refusal_projection_preserves_original_order_and_owner() {
    use slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan as Plan;
    let apple_owner = (
        APPLE_GENERATED_DIAGNOSTIC_OWNER.0,
        "@@apple_support+//crosstool:setup_internal.bzl",
        APPLE_GENERATED_DIAGNOSTIC_OWNER.4,
    );
    let shell_owner = (
        "rules_shell++sh_configure+local_config_shell",
        "@@rules_shell+//shell/private/repositories:sh_config.bzl",
        "sh_config",
    );
    eprintln!(
        "WP1055_SHELL_DIAGNOSTIC_LAYOUT owner={} row={} literal={} apple_owner={} apple_fresh_literal={} apple_reused_literal={} apple_label={} apple_label_literal={}",
        std::mem::size_of::<(&str, &str, &str, &str, &str)>(),
        std::mem::size_of::<&str>(),
        shell_owner.0.len()
            + "rules_shell+".len()
            + "shell/private/repositories".len()
            + "sh_config.bzl".len()
            + shell_owner.2.len(),
        std::mem::size_of_val(&APPLE_GENERATED_DIAGNOSTIC_OWNER),
        APPLE_GENERATED_DIAGNOSTIC_OWNER.0.len()
            + APPLE_GENERATED_DIAGNOSTIC_OWNER.3.len()
            + APPLE_GENERATED_DIAGNOSTIC_OWNER.4.len(),
        APPLE_GENERATED_DIAGNOSTIC_OWNER.1.len() + APPLE_GENERATED_DIAGNOSTIC_OWNER.2.len(),
        std::mem::size_of_val(&apple_owner),
        apple_owner.1.len(),
    );
    for (owner, &(canonical, defining_label, rule_name)) in GENERATED_DIAGNOSTIC_TEST_OWNERS
        .iter()
        .chain(std::iter::once(
            &GENERATED_CC_TOOLCHAINS_DIAGNOSTIC_TEST_OWNER,
        ))
        .chain(std::iter::once(&shell_owner))
        .chain(std::iter::once(&apple_owner))
        .enumerate()
    {
        let path = |s: &str| NormalizedAbsolutePath::new(s).unwrap();
        let bodies = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")]);
        let request_for_plan = |plan: Plan| {
            let mut request = generated_globals_request(&bodies, plan);
            request.id.canonical_repo =
                slug_identity_v2::CanonicalRepoName::new(canonical).unwrap();
            request.repo_spec.rule_id.bzl_file = CanonicalLabel::parse(defining_label).unwrap();
            request.repo_spec.rule_id.rule_name = rule_name.into();
            if matches!(owner, 2 | 4) {
                request.repo_spec.attributes = Arc::new(starlark_map::small_map::SmallMap::new());
            }
            request
        };
        let content: Arc<[u8]> = Arc::from(&b"abc"[..]);
        let content_weak = Arc::downgrade(&content);
        let plan = Plan::build([
            (
                compact_str::CompactString::new("FIRST"),
                content.clone(),
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
            .with_fifth(path("/skylib"))
            .with_sixth(path("/cc"))
            .with_seventh(path("/license"))
            .with_eighth(path("/shell"))
            .with_ninth(path("/apple"))
            .with_apple_support_import_sources(
                RegistryChoice::PreparedFile,
                CommandChoice::PublicProbe,
            )
            .with_rules_shell_import_sources(
                RegistryChoice::PreparedFile,
                CommandChoice::PublicProbe,
            )
            .with_rules_cc_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
            .with_bazel_features_import_sources(
                RegistryChoice::PreparedFile,
                CommandChoice::PublicProbe,
            )
            .with_platforms_host_import_sources(
                RegistryChoice::PreparedFile,
                CommandChoice::PublicProbe,
            )
            .with_rules_rust_defs_import_sources(
                RegistryChoice::PreparedFile,
                CommandChoice::PublicProbe,
            )
            .with_generated_features_file_effects(
                RegistryChoice::PreparedFile,
                CommandChoice::PublicProbe,
            );
        assert!(enabled.rules_rust_defs_import_sources);
        assert!(enabled.rules_cc_import_sources);
        assert!(enabled.rules_shell_import_sources);
        assert!(enabled.apple_support_import_sources);
        assert!(enabled.additional[5].is_some());
        assert!(!bodies.apple_support_import_sources);
        assert!(enabled.additional[4].is_some());
        assert!(enabled.generated_features_file_effects);
        assert!(!bodies.rules_shell_import_sources);
        assert!(!bodies.generated_features_file_effects);
        // The permissive phase seam still requires the complete typed owner and source roles.
        let permissive = enabled.admit_with_plan_matcher(&needs, &|_, _| true);
        if owner == GENERATED_DIAGNOSTIC_TEST_OWNERS.len()
            || owner == GENERATED_DIAGNOSTIC_TEST_OWNERS.len() + 1
            || owner == GENERATED_DIAGNOSTIC_TEST_OWNERS.len() + 2
        {
            assert_eq!(permissive.unwrap_err().family, "materialization");
            assert_eq!(
                enabled.first_denied_materialization(&needs).unwrap().1,
                "generated_kind"
            );
        } else {
            assert!(permissive.is_ok());
        }
        let enabled_denied = enabled.admit(&needs).unwrap_err();
        assert!(
            enabled
                .generated_plan_diagnostic(&needs, &enabled_denied)
                .is_some()
        );
        if canonical == apple_owner.0 {
            for changed in [
                "Apple_cc_autoconf_toolchains",
                "apple_cc_autoconf_toolchains_extra",
            ] {
                let mut request = request_for_plan(Plan::build([]).unwrap());
                request.repo_spec.rule_id.rule_name = changed.into();
                let near = SourcePreparationNeeds::repository(request);
                assert!(
                    bodies
                        .generated_plan_diagnostic(&near, &bodies.admit(&near).unwrap_err())
                        .is_none()
                );
            }
        }
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
        assert!(encoded.contains(if matches!(owner, 2 | 4) {
            "attribute_count=0"
        } else {
            "attribute_count=1"
        }));
        assert!(
            bodies
                .generated_plan_diagnostic(
                    &needs,
                    &Denied {
                        family: "path",
                        path: None,
                        operation: "none"
                    }
                )
                .is_none()
        );
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
        drop(content);
        drop(needs);
        assert!(weak.upgrade().is_none());
        assert!(content_weak.upgrade().is_none());
        for index in 0..8 {
            let mut request = request_for_plan(Plan::build([]).unwrap());
            match index {
                0 => request.id.workspace = path("/other"),
                1 => {
                    request.id.canonical_repo =
                        slug_identity_v2::CanonicalRepoName::new("other+").unwrap()
                }
                2 => {
                    let label = &request.repo_spec.rule_id.bzl_file;
                    request.repo_spec.rule_id.bzl_file = CanonicalLabel::parse(&format!(
                        "@@other//{}:{}",
                        label.package().package().as_str(),
                        label.target().as_str()
                    ))
                    .unwrap();
                }
                3 => {
                    let label = &request.repo_spec.rule_id.bzl_file;
                    request.repo_spec.rule_id.bzl_file = CanonicalLabel::parse(&format!(
                        "@@{}//other:{}",
                        label.package().repo().as_str(),
                        label.target().as_str()
                    ))
                    .unwrap();
                }
                4 => {
                    let label = &request.repo_spec.rule_id.bzl_file;
                    request.repo_spec.rule_id.bzl_file = CanonicalLabel::parse(&format!(
                        "@@{}//{}:other.bzl",
                        label.package().repo().as_str(),
                        label.package().package().as_str()
                    ))
                    .unwrap();
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
        // Every component cross between otherwise eligible atomic owners stays closed.
        for &(other_canonical, other_label, other_rule) in GENERATED_DIAGNOSTIC_TEST_OWNERS
            .iter()
            .chain(std::iter::once(
                &GENERATED_CC_TOOLCHAINS_DIAGNOSTIC_TEST_OWNER,
            ))
            .chain(std::iter::once(&shell_owner))
            .chain(std::iter::once(&apple_owner))
        {
            if other_canonical == canonical {
                continue;
            }
            for component in 0..3 {
                let mut crossed = request_for_plan(Plan::build([]).unwrap());
                match component {
                    0 => {
                        crossed.id.canonical_repo =
                            slug_identity_v2::CanonicalRepoName::new(other_canonical).unwrap()
                    }
                    1 => {
                        crossed.repo_spec.rule_id.bzl_file =
                            CanonicalLabel::parse(other_label).unwrap()
                    }
                    2 => crossed.repo_spec.rule_id.rule_name = other_rule.into(),
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
        }
        let mut request = request_for_plan(Plan::build([]).unwrap());
        let mut mapping = slug_identity_v2::RepositoryMapping::new(
            slug_identity_v2::RepositoryMappingId::new("owner-map").unwrap(),
        );
        mapping.insert(
            slug_identity_v2::ApparentRepoName::new("features").unwrap(),
            slug_identity_v2::CanonicalRepoName::new(
                request.repo_spec.rule_id.bzl_file.package().repo().as_str(),
            )
            .unwrap(),
        );
        request.repo_spec.rule_id.bzl_file = slug_identity_v2::ApparentLabel::parse(&format!(
            "@features//{}:{}",
            request
                .repo_spec
                .rule_id
                .bzl_file
                .package()
                .package()
                .as_str(),
            request.repo_spec.rule_id.bzl_file.target().as_str()
        ))
        .unwrap()
        .resolve(&mapping);
        Arc::get_mut(&mut request.repo_spec.attributes)
            .unwrap()
            .insert(
                compact_str::CompactString::new("opaque"),
                OverrideAttributeValue::String("not a diagnostic payload".into()),
            );
        let mapped = SourcePreparationNeeds::repository(request);
        assert!(
            bodies
                .generated_plan_diagnostic(&mapped, &denied)
                .unwrap()
                .to_string()
                .contains("mapping_len=9 mapping_hex=6f776e65722d6d6170")
        );
        let mapped_text = bodies
            .generated_plan_diagnostic(&mapped, &denied)
            .unwrap()
            .to_string();
        assert!(mapped_text.contains(if matches!(owner, 2 | 4) {
            "attribute_count=1"
        } else {
            "attribute_count=2"
        }));
        assert!(!mapped_text.contains("not a diagnostic payload"));
        let bad = SourcePreparationNeeds::repository(
            (*local_native_request(&bodies.roots[0], "unknown+", "/features")).clone(),
        );
        let bad_request = bad.repository_materializations().values().next().unwrap();
        let earlier = bad.try_union(&mapped).unwrap();
        let (first, reason) = bodies.first_denied_materialization(&earlier).unwrap();
        assert!(std::ptr::eq(first, bad_request.as_ref()));
        assert_eq!(reason, "binding");
        assert!(
            bodies
                .generated_plan_diagnostic(&earlier, &bodies.admit(&earlier).unwrap_err())
                .is_none()
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
    let apple_owner = (
        APPLE_GENERATED_DIAGNOSTIC_OWNER.0,
        "@@apple_support+//crosstool:setup_internal.bzl",
        APPLE_GENERATED_DIAGNOSTIC_OWNER.4,
    );
    let shell_owner = (
        "rules_shell++sh_configure+local_config_shell",
        "@@rules_shell+//shell/private/repositories:sh_config.bzl",
        "sh_config",
    );
    for &(canonical, defining_label, rule_name) in GENERATED_DIAGNOSTIC_TEST_OWNERS
        .iter()
        .chain(std::iter::once(
            &GENERATED_CC_TOOLCHAINS_DIAGNOSTIC_TEST_OWNER,
        ))
        .chain(std::iter::once(&shell_owner))
        .chain(std::iter::once(&apple_owner))
    {
        let path = |s: &str| NormalizedAbsolutePath::new(s).unwrap();
        let bodies = Bodies::new([path("/fixture"), path("/rules"), path("/platforms")]);
        let request_for_plan = |plan: Plan| {
            let mut request = generated_globals_request(&bodies, plan);
            request.id.canonical_repo =
                slug_identity_v2::CanonicalRepoName::new(canonical).unwrap();
            request.repo_spec.rule_id.bzl_file = CanonicalLabel::parse(defining_label).unwrap();
            request.repo_spec.rule_id.rule_name = rule_name.into();
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
                slug_identity_v2::CanonicalRepoName::new(
                    request.repo_spec.rule_id.bzl_file.package().repo().as_str(),
                )
                .unwrap(),
            );
            request.repo_spec.rule_id.bzl_file = slug_identity_v2::ApparentLabel::parse(&format!(
                "@features//{}:{}",
                request
                    .repo_spec
                    .rule_id
                    .bzl_file
                    .package()
                    .package()
                    .as_str(),
                request.repo_spec.rule_id.bzl_file.target().as_str()
            ))
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
            .with_seventh(path("/license"))
            .with_eighth(path("/shell"))
            .with_rules_shell_import_sources(
                RegistryChoice::PreparedFile,
                CommandChoice::PublicProbe,
            )
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
            .with_platforms_host_directory(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
            .with_platforms_host_metadata(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
            .with_platforms_host_import_sources(
                RegistryChoice::PreparedFile,
                CommandChoice::PublicProbe,
            )
            .with_platforms_cpu_package_sources(
                RegistryChoice::PreparedFile,
                CommandChoice::PublicProbe,
            )
            .with_platforms_os_package_sources(
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
            "rules_license=/license",
            "rules_shell=/shell",
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
    request.repository_environment =
        slug_bzlmod_v2::RepositoryEnvironmentSnapshot::from_canonical([
            slug_bzlmod_v2::RepositoryEnvironmentEntry::new("TEMP", "fixture-only"),
        ])
        .unwrap();
    assert!(!projection_matches(
        &inner,
        &request,
        &bodies,
        &registry,
        &nightly_overlay()
    ));
    request.repository_environment = slug_bzlmod_v2::RepositoryEnvironmentSnapshot::empty();
    assert!(projection_matches(
        &inner,
        &request,
        &bodies,
        &registry,
        &nightly_overlay()
    ));
    Arc::get_mut(&mut bodies)
        .unwrap()
        .platforms_os_package_sources = false;
    assert!(!projection_matches(
        &inner,
        &request,
        &bodies,
        &registry,
        &nightly_overlay()
    ));
    Arc::get_mut(&mut bodies)
        .unwrap()
        .platforms_os_package_sources = true;
    Arc::get_mut(&mut bodies)
        .unwrap()
        .platforms_cpu_package_sources = false;
    assert!(!projection_matches(
        &inner,
        &request,
        &bodies,
        &registry,
        &nightly_overlay()
    ));
    Arc::get_mut(&mut bodies)
        .unwrap()
        .platforms_cpu_package_sources = true;
    Arc::get_mut(&mut bodies).unwrap().platforms_host_directory = false;
    assert!(!projection_matches(
        &inner,
        &request,
        &bodies,
        &registry,
        &nightly_overlay()
    ));
    Arc::get_mut(&mut bodies).unwrap().platforms_host_directory = true;
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
        .with_seventh(path("/license"))
        .with_eighth(path("/shell"))
        .with_rules_shell_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
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
        .with_platforms_host_directory(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
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
        .with_seventh(path("/license"))
        .with_eighth(path("/shell"))
        .with_rules_shell_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
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
        .with_platforms_host_directory(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
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
        .with_seventh(path("/license"))
        .with_eighth(path("/shell"))
        .with_rules_shell_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
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
        .with_platforms_host_directory(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
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
        .with_seventh(path("/license"))
        .with_eighth(path("/shell"))
        .with_rules_shell_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
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
        .with_platforms_host_directory(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
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
        .with_seventh(path("/license"))
        .with_eighth(path("/shell"))
        .with_rules_shell_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
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
        .with_platforms_host_directory(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
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
        .with_seventh(path("/license"))
        .with_eighth(path("/shell"))
        .with_rules_shell_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
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
        .with_platforms_host_directory(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
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
        .with_seventh(path("/license"))
        .with_eighth(path("/shell"))
        .with_rules_shell_import_sources(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
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
        .with_platforms_host_directory(RegistryChoice::PreparedFile, CommandChoice::PublicProbe)
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
                "rules_license=/license",
                "rules_shell=/shell",
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
                "rules_license=/license",
                "rules_shell=/shell",
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
                "rules_license=/license",
                "rules_shell=/shell",
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
    Arc::get_mut(&mut bodies)
        .unwrap()
        .rules_shell_import_sources = false;
    assert!(!projection_matches(
        &inner,
        &request,
        &bodies,
        &registry,
        &nightly_overlay()
    ));
    Arc::get_mut(&mut bodies)
        .unwrap()
        .rules_shell_import_sources = true;
    let selected = Arc::get_mut(&mut bodies).unwrap().additional[4].take();
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
                "rules_cc=/cc",
                "rules_license=/license",
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
    Arc::get_mut(&mut bodies).unwrap().additional[4] = selected;
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
    let bodies = Arc::new(
        Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
            .with_rules_rust_defs_import_sources(
                RegistryChoice::PreparedFile,
                CommandChoice::PublicProbe,
            ),
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
    assert_eq!(all.path_observations().unwrap().demands().len(), 69);
    assert!(bodies.admit(&all).is_ok());
    let root = BuildCommandRootKey::new_with_overlay(
        path("/fixture"),
        &[TargetPattern::parse("//pkg:probe").unwrap()],
        build_test_configuration("rust-import-whole-need"),
        nightly_overlay(),
    )
    .unwrap();
    let milestones = Arc::new(Mutex::new(Milestones::default()));
    let guarded = GuardedSingleton {
        inner: SingletonRootSingleBuildCommandKey::new(root).unwrap(),
        bodies: bodies.clone(),
        milestones: milestones.clone(),
    };
    assert!(guarded.check_need(&all).is_ok());
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
        (
            "/rules/rust/extensions.bzl.extra",
            PathObservationOperation::FileBytes,
        ),
        (
            "/rules/rust/EXTENSIONS.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/rules/rust/private/repositories.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/rules/rust/private/repositories.bzl",
            PathObservationOperation::FileDigest,
        ),
        (
            "/other/rust/private/repository_utils.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/rules/rust/private/NIGHTLY_versions.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/rules/rust/private/nightly_versions.bzl.extra",
            PathObservationOperation::Lstat,
        ),
        (
            "/rules/rust/private/known_shas.bzl/child",
            PathObservationOperation::FileBytes,
        ),
        (
            "/rules/rust/private/known_shas.bzl",
            PathObservationOperation::FileDigest,
        ),
        (
            "/other/rust/private/known_shas.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/rules/rust/private/repository_utils_unknown.bzl",
            PathObservationOperation::FileBytes,
        ),
    ] {
        let excluded_need = need(Path::new(excluded), operation.dupe());
        for mixed in [
            all.try_union(&excluded_need).unwrap(),
            excluded_need.try_union(&all).unwrap(),
        ] {
            let expected = mixed
                .path_observations()
                .unwrap()
                .demands()
                .iter()
                .find(|d| **d == excluded_need.path_observations().unwrap().demands()[0])
                .unwrap();
            let (first, family) = bodies
                .first_denied_path_with_namespace_matcher(&mixed, &|_| false)
                .unwrap();
            assert!(std::ptr::eq(first, expected));
            assert_eq!(family, "path");
            let mut progress = 0;
            if bodies.admit(&mixed).is_ok() {
                progress += 1;
            }
            progress += usize::from(guarded.check_need(&mixed).is_ok());
            assert_eq!(progress, 0);
            let denied = bodies.admit(&mixed).unwrap_err();
            assert_eq!(denied.family, "path");
            assert_eq!(denied.operation, path_operation_code(&operation));
            assert_eq!(denied.path.unwrap().as_path(), Path::new(excluded));
            let recorded = milestones.lock().unwrap();
            let recorded_denied = recorded.denied.as_ref().unwrap();
            assert_eq!(recorded_denied.path.as_ref(), Some(expected.path()));
            assert_eq!((recorded.calls, recorded.preparations), (0, 0));
        }
    }
    let local = SourcePreparationNeeds::repository(
        (*local_native_request(&bodies.roots[0], "unknown+", "/rules")).clone(),
    );
    let original = local.repository_materializations().values().next().unwrap();
    let held = original.clone();
    let request_weak = Arc::downgrade(original);
    for mixed in [
        all.try_union(&local).unwrap(),
        local.try_union(&all).unwrap(),
    ] {
        let (first, reason) = bodies
            .first_denied_materialization_with_plan_matcher(
                &mixed,
                &generated_features_plan_matches,
            )
            .unwrap();
        assert!(std::ptr::eq(first, original.as_ref()));
        assert!(Arc::ptr_eq(
            original,
            mixed.repository_materializations().values().next().unwrap()
        ));
        assert_eq!(reason, "binding");
        assert_eq!(bodies.admit(&mixed).unwrap_err().family, "materialization");
        assert!(guarded.check_need(&mixed).is_err());
        let recorded = milestones.lock().unwrap();
        assert_eq!((recorded.calls, recorded.preparations), (0, 0));
    }
    drop(local);
    assert!(request_weak.upgrade().is_some());
    drop(held);
    assert!(request_weak.upgrade().is_none());
    let bodies_weak = Arc::downgrade(&bodies);
    drop(guarded);
    assert!(bodies_weak.upgrade().is_some());
    drop(bodies);
    assert!(bodies_weak.upgrade().is_none());
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
        assert_eq!(
            make().native_source_limits(registry, command),
            SourceServiceLimits {
                registry_attempts: registry.registry_attempts(),
                ..baseline
            }
        );
        let native = bodies.native_source_limits(registry, command);
        assert_eq!(
            native,
            SourceServiceLimits {
                source_attempts: if attempts == 128 { 256 } else { attempts },
                ..limits
            }
        );
        if attempts == 128 {
            let start = Instant::now();
            let scope = bodies
                .source_scope(
                    path("/fixture"),
                    start + Duration::from_secs(2),
                    start + Duration::from_secs(3),
                    native,
                )
                .unwrap();
            assert_eq!(scope.limits(), native);
            let (root, maximum) = scope.known_shas_allowance_for_test().unwrap();
            assert_eq!(maximum, 4_749_878);
            assert!(std::ptr::eq(root.as_path(), bodies.roots[1].as_path()));
            scope.charge(SourceCounter::SourceAttempts, 256).unwrap();
            let refused = Err(SourceServiceError::Limit(SourceCounter::SourceAttempts));
            assert_eq!(scope.charge(SourceCounter::SourceAttempts, 1), refused);
            assert_eq!(scope.charge(SourceCounter::RegistryAttempts, 1), refused);
            assert_eq!(scope.charge(SourceCounter::SourceAttempts, 1), refused);
            assert_eq!(scope.snapshot().source_attempts, 256);
            assert_eq!(scope.snapshot().registry_attempts, 0);
        }
        // Pure projection remains guarded even with an explicitly inconsistent
        // test-only role bit; ordinary128 does not imply native256 in other modes.
        let mut forced = make();
        forced.rules_rust_defs_import_sources = true;
        assert_eq!(
            forced.native_source_limits(registry, command),
            SourceServiceLimits {
                source_attempts: if matches!(registry, RegistryChoice::PreparedFile)
                    && matches!(command, CommandChoice::PublicProbe)
                {
                    256
                } else {
                    128
                },
                ..forced.source_limits(registry)
            }
        );
        for (counter, maximum) in [
            (SourceCounter::SourceAttempts, attempts),
            (SourceCounter::DecodedBytes, 16 << 20),
        ] {
            let start = Instant::now();
            let scope = bodies
                .source_scope(
                    path("/fixture"),
                    start + Duration::from_secs(2),
                    start + Duration::from_secs(3),
                    limits,
                )
                .unwrap();
            assert_eq!(scope.limits(), limits);
            let allowance = scope.known_shas_allowance_for_test();
            assert_eq!(allowance.is_some(), attempts == 128);
            if let Some((selected_root, maximum)) = allowance {
                assert_eq!(maximum, 4_749_878);
                assert_eq!(selected_root, &bodies.roots[1]);
                assert!(std::ptr::eq(
                    selected_root.as_path(),
                    bodies.roots[1].as_path()
                ));
            }
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
    {
        use immutable_archive_preparation::PROTOBUF_DIGEST;
        use immutable_archive_preparation::PROTOBUF_NATIVE_BYTE_BUDGET;
        use immutable_archive_preparation::PROTOBUF_URL;
        let project = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let archive = NormalizedAbsolutePath::new(
            project
                .join("tests/v2_oracle/fixtures/configured-cli-authentic/mirror")
                .join(PROTOBUF_URL.strip_prefix("https://").unwrap()),
        )
        .unwrap();
        let make_qualified = || {
            let mut bodies = java_proxy_test_bodies();
            bodies.offline_archive = Some(OfflineArchiveBacking::new(
                PROTOBUF_URL,
                PROTOBUF_DIGEST,
                archive.dupe(),
            ));
            bodies.offline_archive_module = Some(
                NormalizedAbsolutePath::new(
                    project.join("target/wp954/registry/modules/protobuf/33.4/MODULE.bazel"),
                )
                .unwrap(),
            );
            bodies
        };
        for registry in [RegistryChoice::DefaultBcr, RegistryChoice::PreparedFile] {
            for command in [CommandChoice::Modules, CommandChoice::PublicProbe] {
                for defs in [false, true] {
                    let mut bodies = make_qualified();
                    bodies.rules_rust_defs_import_sources = defs;
                    let ordinary = bodies.source_limits(registry);
                    let qualified = defs
                        && matches!(registry, RegistryChoice::PreparedFile)
                        && matches!(command, CommandChoice::PublicProbe);
                    assert_eq!(
                        bodies.native_source_limits(registry, command),
                        SourceServiceLimits {
                            source_attempts: if qualified {
                                512
                            } else {
                                ordinary.source_attempts
                            },
                            decoded_bytes: if qualified {
                                PROTOBUF_NATIVE_BYTE_BUDGET
                            } else {
                                ordinary.decoded_bytes
                            },
                            written_bytes: if qualified {
                                PROTOBUF_NATIVE_BYTE_BUDGET
                            } else {
                                ordinary.written_bytes
                            },
                            ..ordinary
                        }
                    );
                }
            }
        }
        for case in 0..9 {
            let mut bodies = make_qualified();
            match case {
                0 => {}                                  // Both qualified archives.
                1 => bodies.java_offline_archive = None, // Optional absence stays qualified.
                2 => bodies.offline_archive = None,
                3 => bodies.offline_archive_module = None,
                4 => {
                    bodies.offline_archive = Some(OfflineArchiveBacking::new(
                        "https://other.invalid/a",
                        PROTOBUF_DIGEST,
                        archive.dupe(),
                    ))
                }
                5 => {
                    bodies.offline_archive = Some(OfflineArchiveBacking::new(
                        PROTOBUF_URL,
                        [0; 32],
                        archive.dupe(),
                    ))
                }
                6 => {
                    bodies.offline_archive = Some(OfflineArchiveBacking::new(
                        PROTOBUF_URL,
                        PROTOBUF_DIGEST,
                        path("/foreign/a"),
                    ))
                }
                7 => bodies.offline_archive_module = Some(path("/foreign/MODULE.bazel")),
                _ => {
                    bodies.java_offline_archive.as_mut().unwrap().module =
                        path("/foreign/java/MODULE.bazel")
                }
            }
            let ordinary = bodies.source_limits(RegistryChoice::PreparedFile);
            let qualified = case < 2;
            let native = bodies
                .native_source_limits(RegistryChoice::PreparedFile, CommandChoice::PublicProbe);
            assert_eq!(
                native,
                SourceServiceLimits {
                    source_attempts: if qualified { 512 } else { 256 },
                    decoded_bytes: if qualified {
                        PROTOBUF_NATIVE_BYTE_BUDGET
                    } else {
                        ordinary.decoded_bytes
                    },
                    written_bytes: if qualified {
                        PROTOBUF_NATIVE_BYTE_BUDGET
                    } else {
                        ordinary.written_bytes
                    },
                    ..ordinary
                }
            );
            if qualified {
                let start = Instant::now();
                let scope = bodies
                    .source_scope(
                        path("/fixture"),
                        start + Duration::from_secs(2),
                        start + Duration::from_secs(3),
                        native,
                    )
                    .unwrap();
                let scope_weak = Arc::downgrade(&scope);
                scope.charge(SourceCounter::SourceAttempts, 512).unwrap();
                let refused = Err(SourceServiceError::Limit(SourceCounter::SourceAttempts));
                assert_eq!(scope.charge(SourceCounter::SourceAttempts, 1), refused);
                assert_eq!(scope.charge(SourceCounter::RegistryAttempts, 1), refused);
                assert_eq!(scope.charge(SourceCounter::SourceAttempts, 0), refused);
                assert_eq!(scope.snapshot().source_attempts, 512);
                assert_eq!(scope.snapshot().registry_attempts, 0);
                for count in [0, 513] {
                    assert!(matches!(
                        bodies.source_scope(
                            path("/fixture"),
                            start + Duration::from_secs(2),
                            start + Duration::from_secs(3),
                            SourceServiceLimits {
                                source_attempts: count,
                                ..native
                            }
                        ),
                        Err(SourceServiceError::InvalidLimits)
                    ));
                }
                drop(scope);
                assert!(scope_weak.upgrade().is_none());
            }
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
    eprintln!(
        "WP1085_SKYLIB_SOURCE_LAYOUT bodies={}/{} scope={}/{} row={}/{} sources={} metadata={}",
        std::mem::size_of::<Bodies>(),
        std::mem::align_of::<Bodies>(),
        std::mem::size_of::<SourceServiceScope>(),
        std::mem::align_of::<SourceServiceScope>(),
        std::mem::size_of::<&str>(),
        std::mem::align_of::<&str>(),
        std::mem::size_of_val(&BAZEL_SKYLIB_IMPORTS),
        std::mem::size_of_val(&BAZEL_SKYLIB_IMPORT_METADATA),
    );
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
            &operations[..if BAZEL_SKYLIB_IMPORTS.contains(&relative) {
                2
            } else {
                1
            }],
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
    for relative in &BAZEL_SKYLIB_IMPORT_METADATA[1..] {
        all = all
            .try_union(&need(
                &Path::new("/skylib").join(relative),
                PathObservationOperation::Lstat,
            ))
            .unwrap();
    }
    for relative in BAZEL_SKYLIB_IMPORTS {
        let selected = Path::new("/skylib").join(relative);
        for operation in &operations {
            all = all.try_union(&need(&selected, operation.dupe())).unwrap();
        }
    }
    assert_eq!(all.path_observations().unwrap().demands().len(), 37);
    assert!(bodies.admit(&all).is_ok());
    assert_eq!(
        bodies
            .source_limits(RegistryChoice::PreparedFile)
            .source_attempts,
        16
    );
    for (excluded, operation) in [
        (
            "/skylib/Bzl_library.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/skylib/bzl_library.bzl.bak",
            PathObservationOperation::FileBytes,
        ),
        (
            "/skylib/bzl_library.bzl",
            PathObservationOperation::FileDigest,
        ),
        (
            "/skylib/rules/private/bzl_library.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/skylib/rules/Private/bzl_library.bzl",
            PathObservationOperation::Lstat,
        ),
        (
            "/features/rules/private/bzl_library.bzl",
            PathObservationOperation::FileBytes,
        ),
        ("/skylib/rules/private", PathObservationOperation::FileBytes),
        (
            "/skylib/rules/private/BUILD.bazel",
            PathObservationOperation::FileBytes,
        ),
        (
            "/skylib/rules/private/BUILD",
            PathObservationOperation::FileBytes,
        ),
        (
            "/skylib/rules/private",
            PathObservationOperation::DirectoryEntries,
        ),
        ("/skylib/lib", PathObservationOperation::FileBytes),
        ("/skylib/rules", PathObservationOperation::FileBytes),
        ("/skylib/toolchains", PathObservationOperation::FileBytes),
        (
            "/skylib/toolchains/BUILD.bazel",
            PathObservationOperation::FileBytes,
        ),
        (
            "/skylib/toolchains/BUILD",
            PathObservationOperation::FileBytes,
        ),
        ("/skylib/Toolchains/BUILD", PathObservationOperation::Lstat),
        (
            "/skylib/toolchains/unittest/BUILD.bazel",
            PathObservationOperation::FileBytes,
        ),
        (
            "/skylib/toolchains/unittest",
            PathObservationOperation::DirectoryEntries,
        ),
        (
            "/skylib/toolchains/unittest/Build",
            PathObservationOperation::Lstat,
        ),
        (
            "/skylib/toolchains/unittest/BUILD/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/skylib/toolchains/unittest/BUILD.bak",
            PathObservationOperation::FileBytes,
        ),
        ("/skylib/lib/BUILD/child", PathObservationOperation::Lstat),
        ("/skylib/lib/Build", PathObservationOperation::FileBytes),
        ("/skylib/lib/BUILD.bak", PathObservationOperation::FileBytes),
        (
            "/skylib/lib/unittest.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/skylib/lib/unittest.bzl.bak",
            PathObservationOperation::FileBytes,
        ),
        (
            "/skylib/lib/Unittest.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/features/lib/unittest.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/skylib/lib/unittest.bzl",
            PathObservationOperation::FileDigest,
        ),
        (
            "/skylib/lib/new_sets.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/skylib/lib/New_sets.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/skylib/lib/partial.bzl.bak",
            PathObservationOperation::FileBytes,
        ),
        ("/skylib/lib/Partial.bzl", PathObservationOperation::Lstat),
        (
            "/skylib/lib/types.bzl/child",
            PathObservationOperation::FileBytes,
        ),
        ("/features/lib/types.bzl", PathObservationOperation::Lstat),
        (
            "/skylib/lib/dicts.bzl.bak",
            PathObservationOperation::FileBytes,
        ),
        (
            "/skylib/lib/dicts.bzl",
            PathObservationOperation::FileDigest,
        ),
        (
            "/skylib/toolchains/BUILD/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/skylib/toolchains/BUILD.bazel.bak",
            PathObservationOperation::Lstat,
        ),
        (
            "/features/toolchains/BUILD",
            PathObservationOperation::Lstat,
        ),
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
        let bad = need(Path::new(excluded), operation.dupe());
        for mixed in [all.try_union(&bad).unwrap(), bad.try_union(&all).unwrap()] {
            let mut progress = 0;
            if bodies.admit(&mixed).is_ok() {
                progress += 1;
            }
            assert_eq!(progress, 0);
            let denied = bodies.admit(&mixed).unwrap_err();
            let expected_family = if operation == PathObservationOperation::DirectoryEntries {
                "operation"
            } else {
                "path"
            };
            assert_eq!(
                (denied.family, denied.operation),
                (expected_family, path_operation_code(&operation))
            );
            assert_eq!(denied.path.unwrap().as_path(), Path::new(excluded));
            let original = mixed
                .path_observations()
                .unwrap()
                .demands()
                .iter()
                .find(|d| d.path().as_path() == Path::new(excluded) && d.operation() == operation)
                .unwrap();
            let (first, family) = bodies
                .first_denied_path_with_namespace_matcher(&mixed, &|_| false)
                .unwrap();
            assert_eq!(family, expected_family);
            assert!(std::ptr::eq(first, original));
        }
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
    let mut restored = bodies;
    let selected = restored.additional[1].take();
    assert!(restored.bazel_skylib_import_sources && restored.admit(&all).is_err());
    restored.additional[1] = selected;
    assert!(restored.admit(&all).is_ok());
    let local = SourcePreparationNeeds::repository(
        Arc::into_inner(local_native_request(
            &restored.roots[0],
            "unknown+",
            "/skylib",
        ))
        .unwrap(),
    );
    let original = local.repository_materializations().values().next().unwrap();
    let weak = Arc::downgrade(original);
    for mixed in [
        all.try_union(&local).unwrap(),
        local.try_union(&all).unwrap(),
    ] {
        assert!(Arc::ptr_eq(
            original,
            mixed.repository_materializations().values().next().unwrap()
        ));
        let (first, reason) = restored
            .first_denied_materialization_with_plan_matcher(
                &mixed,
                &generated_features_plan_matches,
            )
            .unwrap();
        assert!(std::ptr::eq(first, original.as_ref()));
        assert_eq!(reason, "binding");
        let mut progress = 0;
        if restored.admit(&mixed).is_ok() {
            progress += 1;
        }
        assert_eq!(progress, 0);
    }
    drop(local);
    assert!(weak.upgrade().is_none());
}
#[test]
fn rules_cc_import_sources_need_is_opt_in_exact_table_only() {
    eprintln!(
        "WP1086_CC_SOURCE_LAYOUT bodies={}/{} scope={}/{} row={}/{} sources={} metadata={}",
        std::mem::size_of::<Bodies>(),
        std::mem::align_of::<Bodies>(),
        std::mem::size_of::<SourceServiceScope>(),
        std::mem::align_of::<SourceServiceScope>(),
        std::mem::size_of::<&str>(),
        std::mem::align_of::<&str>(),
        std::mem::size_of_val(&RULES_CC_IMPORTS),
        std::mem::size_of_val(&RULES_CC_IMPORT_METADATA),
    );
    eprintln!(
        "WP1047_CC_BODY_LAYOUT body={} row={} literal={}",
        std::mem::size_of_val(&RULES_CC_IMPORTS),
        std::mem::size_of::<&str>(),
        RULES_CC_IMPORTS.last().unwrap().len()
    );
    eprintln!(
        "WP1046_CC_METADATA_LAYOUT metadata={} row={} literal={}",
        std::mem::size_of_val(&RULES_CC_IMPORT_METADATA),
        std::mem::size_of::<&str>(),
        RULES_CC_IMPORT_METADATA.last().unwrap().len()
    );
    eprintln!(
        "WP1045_CC_BODY_LAYOUT body={} row={} literal={}",
        std::mem::size_of_val(&RULES_CC_IMPORTS),
        std::mem::size_of::<&str>(),
        RULES_CC_IMPORTS[51].len()
    );
    eprintln!(
        "WP1044_CC_METADATA_LAYOUT metadata={} row={} literal={}",
        std::mem::size_of_val(&RULES_CC_IMPORT_METADATA),
        std::mem::size_of::<&str>(),
        RULES_CC_IMPORT_METADATA[15..]
            .iter()
            .take(3)
            .map(|relative| relative.len())
            .sum::<usize>()
    );
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
        // Captured files can also be declared metadata candidates.
        let admitted_operations = &operations[..if RULES_CC_IMPORTS.contains(&relative) {
            2
        } else {
            1
        }];
        assert_exact_root_file_role(
            relative,
            admitted_operations,
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
    assert_eq!(all.path_observations().unwrap().demands().len(), 197);
    assert!(bodies.admit(&all).is_ok());
    assert_eq!(
        bodies
            .source_limits(RegistryChoice::PreparedFile)
            .source_attempts,
        16
    );
    for (excluded, operation) in [
        ("/cc/cc/Cc_library.bzl", PathObservationOperation::FileBytes),
        ("/cc/cc/cc_library.bzl.bak", PathObservationOperation::Lstat),
        (
            "/cc/cc/private/rules_impl/cc_library.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/common/Cc_helper.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/private/rules_impl/attrs.bzl.bak",
            PathObservationOperation::FileBytes,
        ),
        (
            "/rules/cc/private/rules_impl/cc_binary.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/private/rules_impl/fdo/Fdo_profile.bzl",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/private/rules_impl/fdo/fdo_context.bzl/child",
            PathObservationOperation::Lstat,
        ),
        ("/cc/cc/BUILD.bazel", PathObservationOperation::FileBytes),
        ("/cc/cc/common/BUILD", PathObservationOperation::FileBytes),
        (
            "/cc/cc/private/rules_impl/BUILD.bazel",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/private/rules_impl/fdo/BUILD",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/private/rules_impl/fdo",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/private/rules_impl/fdo",
            PathObservationOperation::DirectoryEntries,
        ),
        (
            "/cc/cc/private/toolchain/test/default_test_runner.bzl.disabled",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/private/toolchain/test/Default_test_runner.bzl",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/private/toolchain/test/default_test_runner.bzl.bak",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/private/toolchain/test/default_test_runner.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/rules/cc/private/toolchain/test/default_test_runner.bzl",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/private/toolchain/test",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/private/toolchain/test/BUILD.bazel",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/private/toolchain/test/BUILD.disabled",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/private/toolchain/Test",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/private/toolchain/test.bak",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/private/toolchain/test/BUILD.bazel/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/rules/cc/private/toolchain/test",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/private/toolchain/test/unknown.bzl",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/private/toolchain/BUILD.toolchains.tpl.disabled",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/private/toolchain/Build.toolchains.tpl",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/private/toolchain/BUILD.toolchains.tpl.bak",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/private/toolchain/BUILD.toolchains.tpl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/rules/cc/private/toolchain/BUILD.toolchains.tpl",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/private/toolchain/BUILD.bazel",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/private/toolchain/BUILD",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/private/toolchain/Build.bazel",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/private/toolchain/BUILD.bak",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/private/toolchain/BUILD.bazel/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/rules/cc/private/toolchain/BUILD",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/toolchains/BUILD.bazel",
            PathObservationOperation::FileBytes,
        ),
        ("/cc/cc/toolchains", PathObservationOperation::FileBytes),
        (
            "/cc/cc/toolchains/BUILD/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/toolchains/BUILD.bak",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/toolchains/build",
            PathObservationOperation::FileBytes,
        ),
        (
            "/rules/cc/toolchains/BUILD",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/toolchains/toolchain_config_utils.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/toolchains/toolchain_config_utils.bzl.bak",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/toolchains/Toolchain_config_utils.bzl",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/cc/toolchains/unknown.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/cc/cc/toolchains/toolchain_config_utils.bzl",
            PathObservationOperation::FileDigest,
        ),
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
        let expected_family = if operation == PathObservationOperation::DirectoryEntries {
            "operation"
        } else {
            "path"
        };
        let bad = need(Path::new(excluded), operation.dupe());
        for mixed in [all.try_union(&bad).unwrap(), bad.try_union(&all).unwrap()] {
            assert_eq!(usize::from(bodies.admit(&mixed).is_ok()), 0);
            let denied = bodies.admit(&mixed).unwrap_err();
            assert_eq!(
                (denied.family, denied.operation),
                (expected_family, path_operation_code(&operation))
            );
            assert_eq!(denied.path.unwrap().as_path(), Path::new(excluded));
            let first = bodies
                .first_denied_path_with_namespace_matcher(&mixed, &|_| false)
                .unwrap()
                .0;
            let original = mixed
                .path_observations()
                .unwrap()
                .demands()
                .iter()
                .find(|d| d.path().as_path() == Path::new(excluded) && d.operation() == operation)
                .unwrap();
            assert!(std::ptr::eq(first, original));
        }
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
    let mut restored = bodies;
    let selected = restored.additional[2].take();
    assert!(restored.rules_cc_import_sources && restored.admit(&all).is_err());
    restored.additional[2] = selected;
    assert!(restored.admit(&all).is_ok());
    let local = SourcePreparationNeeds::repository(
        Arc::into_inner(local_native_request(&restored.roots[0], "unknown+", "/cc")).unwrap(),
    );
    let original = local.repository_materializations().values().next().unwrap();
    let weak = Arc::downgrade(original);
    for mixed in [
        all.try_union(&local).unwrap(),
        local.try_union(&all).unwrap(),
    ] {
        assert!(Arc::ptr_eq(
            original,
            mixed.repository_materializations().values().next().unwrap()
        ));
        let (first, reason) = restored
            .first_denied_materialization_with_plan_matcher(
                &mixed,
                &generated_features_plan_matches,
            )
            .unwrap();
        assert!(std::ptr::eq(first, original.as_ref()));
        assert_eq!(reason, "binding");
        assert_eq!(usize::from(restored.admit(&mixed).is_ok()), 0);
    }
    drop(local);
    assert!(weak.upgrade().is_none());
}
pub(super) fn rules_shell_import_sources_matrix() {
    assert_eq!(
        APPLE_SUPPORT_IMPORTS[3],
        APPLE_SUPPORT_CROSSTOOL_METADATA[3]
    );
    assert!(std::ptr::eq(
        APPLE_SUPPORT_IMPORTS[3].as_ptr(),
        APPLE_SUPPORT_CROSSTOOL_METADATA[3].as_ptr()
    ));
    assert_eq!(
        APPLE_SUPPORT_IMPORTS[4],
        APPLE_SUPPORT_CROSSTOOL_METADATA[5]
    );
    assert!(std::ptr::eq(
        APPLE_SUPPORT_IMPORTS[4].as_ptr(),
        APPLE_SUPPORT_CROSSTOOL_METADATA[5].as_ptr()
    ));
    eprintln!(
        "WP1053_SHELL_BODY_LAYOUT body={} row={} literal={}",
        std::mem::size_of_val(&RULES_SHELL_IMPORTS),
        std::mem::size_of::<&str>(),
        RULES_SHELL_IMPORTS[6].len()
    );
    eprintln!(
        "WP1052_SHELL_METADATA_LAYOUT metadata={} row={} literal={} apple_metadata={} apple_row={} apple_literal={} apple_flag={} bodies={} bodies_align={} apple_imports={} apple_import_row={} apple_import_literal={} apple_import_flag={} apple_metadata_new_literal={} apple_metadata_literal_total={} apple_metadata_second_literal={} apple_metadata_third_literal={} apple_import_new_literal={} apple_import_literal_total={}",
        std::mem::size_of_val(&RULES_SHELL_IMPORT_METADATA),
        std::mem::size_of::<&str>(),
        RULES_SHELL_IMPORT_METADATA[16].len(),
        std::mem::size_of_val(&APPLE_SUPPORT_CROSSTOOL_METADATA),
        std::mem::size_of::<&str>(),
        APPLE_SUPPORT_CROSSTOOL_METADATA[0].len(),
        std::mem::size_of::<bool>(),
        std::mem::size_of::<Bodies>(),
        std::mem::align_of::<Bodies>(),
        std::mem::size_of_val(&APPLE_SUPPORT_IMPORTS),
        std::mem::size_of::<&str>(),
        APPLE_SUPPORT_IMPORTS[0].len(),
        std::mem::size_of::<bool>(),
        APPLE_SUPPORT_CROSSTOOL_METADATA[3].len(),
        APPLE_SUPPORT_CROSSTOOL_METADATA
            .iter()
            .map(|name| name.len())
            .sum::<usize>(),
        APPLE_SUPPORT_CROSSTOOL_METADATA[1].len(),
        APPLE_SUPPORT_CROSSTOOL_METADATA[2].len(),
        APPLE_SUPPORT_IMPORTS[3].len(),
        APPLE_SUPPORT_IMPORTS
            .iter()
            .map(|name| name.len())
            .sum::<usize>()
    );
    eprintln!(
        "WP1051_SHELL_METADATA_LAYOUT metadata={} row={} literal={}",
        std::mem::size_of_val(&RULES_SHELL_IMPORT_METADATA),
        std::mem::size_of::<&str>(),
        RULES_SHELL_IMPORT_METADATA[13..]
            .iter()
            .take(3)
            .map(|relative| relative.len())
            .sum::<usize>()
    );
    eprintln!(
        "WP1050_SHELL_BODY_LAYOUT body={} row={} literal={}",
        std::mem::size_of_val(&RULES_SHELL_IMPORTS),
        std::mem::size_of::<&str>(),
        RULES_SHELL_IMPORTS[5].len()
    );
    eprintln!(
        "WP1049_SHELL_METADATA_LAYOUT metadata={} row={} literal={}",
        std::mem::size_of_val(&RULES_SHELL_IMPORT_METADATA),
        std::mem::size_of::<&str>(),
        RULES_SHELL_IMPORT_METADATA[12].len()
    );
    eprintln!(
        "WP1048_SHELL_METADATA_LAYOUT metadata={} row={} literal={}",
        std::mem::size_of_val(&RULES_SHELL_IMPORT_METADATA),
        std::mem::size_of::<&str>(),
        RULES_SHELL_IMPORT_METADATA[9..]
            .iter()
            .take(3)
            .map(|relative| relative.len())
            .sum::<usize>()
    );
    for (name, selected) in RULES_SHELL_IMPORT_METADATA.into_iter().zip([
        "/shell/shell",
        "/shell/shell/BUILD.bazel",
        "/shell/shell/BUILD",
        "/shell/shell/sh_binary.bzl",
        "/shell/shell/private",
        "/shell/shell/private/BUILD.bazel",
        "/shell/shell/private/BUILD",
        "/shell/shell/private/sh_binary.bzl",
        "/shell/shell/private/sh_executable.bzl",
        "/shell/shell/private/extensions",
        "/shell/shell/private/extensions/BUILD.bazel",
        "/shell/shell/private/extensions/BUILD",
        "/shell/shell/private/extensions/sh_configure.bzl",
        "/shell/shell/private/repositories",
        "/shell/shell/private/repositories/BUILD.bazel",
        "/shell/shell/private/repositories/BUILD",
        "/shell/shell/private/repositories/sh_config.bzl",
        "/shell/shell/toolchains",
        "/shell/shell/toolchains/sh_toolchain.bzl",
    ]) {
        let operations = [
            PathObservationOperation::Lstat,
            PathObservationOperation::FileBytes,
        ];
        let operations = &operations[..if RULES_SHELL_IMPORTS.contains(&name) {
            2
        } else {
            1
        }];
        assert_exact_root_file_role(
            name,
            operations,
            Bodies::with_rules_shell_import_sources,
            |b| b.rules_shell_import_sources,
            &[],
            selected,
            Some("/shell"),
        );
    }
    let path = |s: &str| NormalizedAbsolutePath::new(s).unwrap();
    let make = || {
        Bodies::new([path("/fixture"), path("/rules"), path("/platforms")])
            .with_fourth(path("/features"))
            .with_fifth(path("/skylib"))
            .with_sixth(path("/cc"))
            .with_seventh(path("/license"))
            .with_eighth(path("/shell"))
            .with_rules_shell_import_sources(
                RegistryChoice::PreparedFile,
                CommandChoice::PublicProbe,
            )
    };
    let need =
        |p: &str, operation| {
            SourcePreparationNeeds::path(NeedPathObservations::singleton(
                PathObservationDemand::new(PathObservationNamespace::Host, path(p), operation),
            ))
        };
    let bodies = make();
    let allowed = RULES_SHELL_IMPORT_METADATA
        .into_iter()
        .map(|name| need(&format!("/shell/{name}"), PathObservationOperation::Lstat))
        .chain(RULES_SHELL_IMPORTS.into_iter().map(|name| {
            need(
                &format!("/shell/{name}"),
                PathObservationOperation::FileBytes,
            )
        }))
        .reduce(|a, b| a.try_union(&b).unwrap())
        .unwrap();
    assert_eq!(allowed.path_observations().unwrap().demands().len(), 27);
    assert!(bodies.admit(&allowed).is_ok());
    for (p, operation) in [
        (
            "/shell/shell/toolchains/Sh_toolchain.bzl",
            PathObservationOperation::Lstat,
        ),
        (
            "/shell/shell/toolchains/sh_toolchain.bzl.bak",
            PathObservationOperation::FileBytes,
        ),
        (
            "/shell/shell/toolchains/sh_toolchain.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/shell/toolchains/sh_toolchain.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/shell/shell/toolchains",
            PathObservationOperation::FileBytes,
        ),
        (
            "/shell/shell/toolchains",
            PathObservationOperation::DirectoryEntries,
        ),
        (
            "/shell/shell/toolchains",
            PathObservationOperation::FileDigest,
        ),
        ("/shell/shell/Toolchains", PathObservationOperation::Lstat),
        (
            "/shell/shell/toolchains.bak",
            PathObservationOperation::Lstat,
        ),
        (
            "/shell/shell/toolchains/child",
            PathObservationOperation::Lstat,
        ),
        ("/cc/shell/toolchains", PathObservationOperation::Lstat),
        (
            "/shell/shell/toolchains/BUILD.bazel",
            PathObservationOperation::Lstat,
        ),
        (
            "/shell/shell/toolchains/BUILD",
            PathObservationOperation::Lstat,
        ),
        (
            "/shell/shell/private/repositories/sh_config.bzl.disabled",
            PathObservationOperation::FileBytes,
        ),
        (
            "/shell/shell/private/repositories/Sh_config.bzl",
            PathObservationOperation::Lstat,
        ),
        (
            "/shell/shell/private/repositories/sh_config.bzl.bak",
            PathObservationOperation::Lstat,
        ),
        (
            "/shell/shell/private/repositories/sh_config.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/shell/private/repositories/sh_config.bzl",
            PathObservationOperation::Lstat,
        ),
        (
            "/shell/shell/private/repositories/sh_config.bzl",
            PathObservationOperation::DirectoryEntries,
        ),
        (
            "/shell/shell/private/repositories/sh_config.bzl",
            PathObservationOperation::FileDigest,
        ),
        (
            "/shell/shell/private/repositories",
            PathObservationOperation::FileBytes,
        ),
        (
            "/shell/shell/private/repositories/BUILD.bazel",
            PathObservationOperation::FileBytes,
        ),
        (
            "/shell/shell/private/repositories/BUILD",
            PathObservationOperation::FileBytes,
        ),
        (
            "/shell/shell/private/Repositories",
            PathObservationOperation::Lstat,
        ),
        (
            "/shell/shell/private/repositories.bak",
            PathObservationOperation::Lstat,
        ),
        (
            "/shell/shell/private/repositories/BUILD/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/shell/private/repositories",
            PathObservationOperation::Lstat,
        ),
        (
            "/shell/shell/private/repositories",
            PathObservationOperation::DirectoryEntries,
        ),
        (
            "/shell/shell/private/repositories/BUILD.bazel",
            PathObservationOperation::FileDigest,
        ),
        (
            "/shell/shell/private/extensions/sh_configure.bzl.disabled",
            PathObservationOperation::FileBytes,
        ),
        (
            "/shell/shell/private/extensions/Sh_configure.bzl",
            PathObservationOperation::Lstat,
        ),
        (
            "/shell/shell/private/extensions/sh_configure.bzl.bak",
            PathObservationOperation::Lstat,
        ),
        (
            "/shell/shell/private/extensions/sh_configure.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/shell/private/extensions/sh_configure.bzl",
            PathObservationOperation::Lstat,
        ),
        (
            "/shell/shell/private/extensions/sh_configure.bzl",
            PathObservationOperation::DirectoryEntries,
        ),
        (
            "/shell/shell/private/extensions/sh_configure.bzl",
            PathObservationOperation::FileDigest,
        ),
        (
            "/shell/shell/private/extensions",
            PathObservationOperation::FileBytes,
        ),
        (
            "/shell/shell/private/extensions/BUILD.bazel",
            PathObservationOperation::FileBytes,
        ),
        (
            "/shell/shell/private/extensions/BUILD",
            PathObservationOperation::FileBytes,
        ),
        (
            "/shell/shell/private/Extensions",
            PathObservationOperation::Lstat,
        ),
        (
            "/shell/shell/private/extensions.bak",
            PathObservationOperation::Lstat,
        ),
        (
            "/shell/shell/private/extensions/BUILD/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/shell/private/extensions",
            PathObservationOperation::Lstat,
        ),
        (
            "/shell/shell/private/extensions",
            PathObservationOperation::DirectoryEntries,
        ),
        (
            "/shell/shell/private/extensions/BUILD.bazel",
            PathObservationOperation::FileDigest,
        ),
        (
            "/shell/shell/BUILD.bazel",
            PathObservationOperation::FileBytes,
        ),
        (
            "/shell/shell/private/BUILD.bazel",
            PathObservationOperation::FileBytes,
        ),
        (
            "/shell/shell/private/sh_executable.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/shell/shell/private/SH_EXECUTABLE.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/shell/shell/private/sh_executable.bzl.bak",
            PathObservationOperation::FileBytes,
        ),
        (
            "/shell/shell/SH_BINARY.bzl",
            PathObservationOperation::Lstat,
        ),
        (
            "/shell/shell/sh_binary.bzl.bak",
            PathObservationOperation::FileBytes,
        ),
        (
            "/shell/shell/private/sh_binary.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/shell/sh_binary.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/shell/shell/private",
            PathObservationOperation::DirectoryEntries,
        ),
    ] {
        let bad = need(p, operation);
        for mixed in [
            allowed.try_union(&bad).unwrap(),
            bad.try_union(&allowed).unwrap(),
        ] {
            assert_eq!(usize::from(bodies.admit(&mixed).is_ok()), 0);
            let first = bodies
                .first_denied_path_with_namespace_matcher(&mixed, &|_| false)
                .unwrap()
                .0;
            let expected = bad.path_observations().unwrap().demands().first().unwrap();
            let original = mixed
                .path_observations()
                .unwrap()
                .demands()
                .iter()
                .find(|d| d.path() == expected.path() && d.operation() == expected.operation())
                .unwrap();
            assert!(std::ptr::eq(first, original));
        }
    }
    let mut missing = make();
    missing.additional[4] = None;
    assert!(missing.rules_shell_import_sources && missing.admit(&allowed).is_err());
    let mut wrong_slot = make();
    wrong_slot.additional[3] = wrong_slot.additional[4].take();
    assert!(wrong_slot.admit(&allowed).is_err());
    wrong_slot.additional[4] = wrong_slot.additional[3].take();
    assert!(wrong_slot.admit(&allowed).is_ok());
    let local = SourcePreparationNeeds::repository(
        (*local_native_request(&bodies.roots[0], "unknown+", "/shell")).clone(),
    );
    let original = local.repository_materializations().values().next().unwrap();
    let weak = Arc::downgrade(original);
    for mixed in [
        allowed.try_union(&local).unwrap(),
        local.try_union(&allowed).unwrap(),
    ] {
        assert!(Arc::ptr_eq(
            original,
            mixed.repository_materializations().values().next().unwrap()
        ));
        let (first, reason) = bodies
            .first_denied_materialization_with_plan_matcher(
                &mixed,
                &generated_features_plan_matches,
            )
            .unwrap();
        assert!(std::ptr::eq(first, original.as_ref()));
        assert_eq!(reason, "binding");
        assert_eq!(usize::from(bodies.admit(&mixed).is_ok()), 0);
    }
    drop(local);
    assert!(weak.upgrade().is_none());
    apple_support_crosstool_metadata_matrix(make().with_ninth(path("/apple")));
    apple_support_import_sources_matrix(make().with_ninth(path("/apple")));
}

fn apple_support_import_sources_matrix(mut bodies: Bodies) {
    assert_exact_root_file_role(
        APPLE_SUPPORT_IMPORTS[0],
        &[
            PathObservationOperation::Lstat,
            PathObservationOperation::FileBytes,
        ],
        Bodies::with_apple_support_import_sources,
        |b| b.apple_support_import_sources,
        &[
            "/apple/CROSSTOOL/setup.bzl",
            "/apple/crosstool/SETUP.bzl",
            "/apple/crosstool/setup.bzl.bak",
            "/apple/crosstool/setup.bzl/child",
            "/apple-other/crosstool/setup.bzl",
            "/apple/crosstool/setup.bzl/BUILD",
            "/apple/crosstool/setup.bzl/BUILD.bazel",
        ],
        "/apple/crosstool/setup.bzl",
        Some("/apple"),
    );
    assert_exact_root_file_role(
        APPLE_SUPPORT_IMPORTS[1],
        &[
            PathObservationOperation::Lstat,
            PathObservationOperation::FileBytes,
        ],
        Bodies::with_apple_support_import_sources,
        |b| b.apple_support_import_sources,
        &[
            "/apple/CROSSTOOL/setup_internal.bzl",
            "/apple/crosstool/SETUP_INTERNAL.bzl",
            "/apple/crosstool/setup_internal.bzl.bak",
            "/apple/crosstool/setup_internal.bzl/child",
            "/apple-other/crosstool/setup_internal.bzl",
            "/apple/crosstool/setup_internal.bzl/BUILD",
            "/apple/crosstool/setup_internal.bzl/BUILD.bazel",
            "/apple/crosstool/other.bzl",
        ],
        "/apple/crosstool/setup_internal.bzl",
        Some("/apple"),
    );
    assert_exact_root_file_role(
        APPLE_SUPPORT_IMPORTS[2],
        &[
            PathObservationOperation::Lstat,
            PathObservationOperation::FileBytes,
        ],
        Bodies::with_apple_support_import_sources,
        |b| b.apple_support_import_sources,
        &[
            "/apple/CROSSTOOL/osx_cc_configure.bzl",
            "/apple/crosstool/OSX_CC_CONFIGURE.bzl",
            "/apple/crosstool/osx_cc_configure.bzl.bak",
            "/apple/crosstool/osx_cc_configure.bzl/child",
            "/apple-other/crosstool/osx_cc_configure.bzl",
            "/apple/crosstool/osx_cc_configure.bzl/BUILD",
            "/apple/crosstool/osx_cc_configure.bzl/BUILD.bazel",
            "/apple/crosstool/other.bzl",
        ],
        "/apple/crosstool/osx_cc_configure.bzl",
        Some("/apple"),
    );
    assert_exact_root_file_role(
        APPLE_SUPPORT_IMPORTS[3],
        &[
            PathObservationOperation::Lstat,
            PathObservationOperation::FileBytes,
        ],
        Bodies::with_apple_support_import_sources,
        |b| b.apple_support_import_sources,
        &[
            "/apple/crosstool/build.toolchains",
            "/apple/crosstool/Build.toolchains",
            "/apple/crosstool/BUILD.toolchains.bak",
            "/apple/crosstool/BUILD.toolchains/child",
            "/apple/crosstool/REPO.bazel",
            "/apple/other/BUILD.toolchains",
            "/apple-other/crosstool/BUILD.toolchains",
        ],
        "/apple/crosstool/BUILD.toolchains",
        Some("/apple"),
    );
    assert_exact_root_file_role(
        APPLE_SUPPORT_IMPORTS[4],
        &[
            PathObservationOperation::Lstat,
            PathObservationOperation::FileBytes,
        ],
        Bodies::with_apple_support_import_sources,
        |b| b.apple_support_import_sources,
        &[
            "/apple/CONFIGS/platforms.bzl",
            "/apple/configs/PLATFORMS.bzl",
            "/apple/configs/platforms.bzl.bak",
            "/apple/configs/platforms.bzl/child",
            "/apple/configs/other.bzl",
            "/apple/other/platforms.bzl",
            "/apple-other/configs/platforms.bzl",
        ],
        "/apple/configs/platforms.bzl",
        Some("/apple"),
    );
    bodies.rules_shell_import_sources = false;
    bodies.apple_support_crosstool_metadata = false;
    bodies = bodies.with_apple_support_import_sources(
        RegistryChoice::PreparedFile,
        CommandChoice::PublicProbe,
    );
    let path = |s: &str| NormalizedAbsolutePath::new(s).unwrap();
    let need =
        |p: &str, operation| {
            SourcePreparationNeeds::path(NeedPathObservations::singleton(
                PathObservationDemand::new(PathObservationNamespace::Host, path(p), operation),
            ))
        };
    let leaf = "/apple/crosstool/setup.bzl";
    let allowed = need(leaf, PathObservationOperation::Lstat)
        .try_union(&need(leaf, PathObservationOperation::FileBytes))
        .unwrap()
        .try_union(&need(
            "/apple/crosstool/setup_internal.bzl",
            PathObservationOperation::Lstat,
        ))
        .unwrap()
        .try_union(&need(
            "/apple/crosstool/setup_internal.bzl",
            PathObservationOperation::FileBytes,
        ))
        .unwrap()
        .try_union(&need(
            "/apple/crosstool/osx_cc_configure.bzl",
            PathObservationOperation::Lstat,
        ))
        .unwrap()
        .try_union(&need(
            "/apple/crosstool/osx_cc_configure.bzl",
            PathObservationOperation::FileBytes,
        ))
        .unwrap()
        .try_union(&need(
            "/apple/crosstool/BUILD.toolchains",
            PathObservationOperation::Lstat,
        ))
        .unwrap()
        .try_union(&need(
            "/apple/crosstool/BUILD.toolchains",
            PathObservationOperation::FileBytes,
        ))
        .unwrap()
        .try_union(&need(
            "/apple/configs/platforms.bzl",
            PathObservationOperation::Lstat,
        ))
        .unwrap()
        .try_union(&need(
            "/apple/configs/platforms.bzl",
            PathObservationOperation::FileBytes,
        ))
        .unwrap();
    assert_eq!(allowed.path_observations().unwrap().demands().len(), 10);
    assert!(bodies.admit(&allowed).is_ok());
    assert!(
        bodies
            .admit(&need("/apple/crosstool", PathObservationOperation::Lstat))
            .is_err()
    );
    let binding = bodies.additional[5].take();
    assert!(bodies.apple_support_import_sources && bodies.admit(&allowed).is_err());
    let displaced = bodies.additional[4].take();
    bodies.additional[4] = binding;
    assert!(bodies.admit(&allowed).is_err());
    bodies.additional[5] = bodies.additional[4].take();
    bodies.additional[4] = displaced;
    let original_root = bodies.additional[5].as_ref().unwrap().root.dupe();
    bodies.additional[5].as_mut().unwrap().root = path("/substituted");
    assert!(bodies.admit(&allowed).is_err());
    bodies.additional[5].as_mut().unwrap().root = original_root;
    assert!(bodies.admit(&allowed).is_ok());
    let mut progress = 0;
    for bad in [
        (leaf, PathObservationOperation::FileDigest),
        (leaf, PathObservationOperation::ReadLink),
        (leaf, PathObservationOperation::DirectoryEntries),
        (
            "/apple/CROSSTOOL/setup.bzl",
            PathObservationOperation::Lstat,
        ),
        (
            "/apple/crosstool/setup.bzl.bak",
            PathObservationOperation::FileBytes,
        ),
        (
            "/apple/crosstool/setup.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/crosstool/setup.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/apple/crosstool/setup_internal.bzl",
            PathObservationOperation::FileDigest,
        ),
        (
            "/apple/crosstool/setup_internal.bzl",
            PathObservationOperation::ReadLink,
        ),
        (
            "/apple/crosstool/setup_internal.bzl",
            PathObservationOperation::DirectoryEntries,
        ),
        (
            "/apple/crosstool/setup_internal.bzl.bak",
            PathObservationOperation::FileBytes,
        ),
        (
            "/apple/crosstool/setup_internal.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/crosstool/setup_internal.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/apple/crosstool/osx_cc_configure.bzl",
            PathObservationOperation::FileDigest,
        ),
        (
            "/apple/crosstool/osx_cc_configure.bzl",
            PathObservationOperation::ReadLink,
        ),
        (
            "/apple/crosstool/osx_cc_configure.bzl",
            PathObservationOperation::DirectoryEntries,
        ),
        (
            "/apple/crosstool/osx_cc_configure.bzl.bak",
            PathObservationOperation::FileBytes,
        ),
        (
            "/apple/crosstool/osx_cc_configure.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/cc/crosstool/osx_cc_configure.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/apple/crosstool/BUILD.toolchains",
            PathObservationOperation::FileDigest,
        ),
        (
            "/apple/crosstool/BUILD.toolchains",
            PathObservationOperation::ReadLink,
        ),
        (
            "/apple/crosstool/BUILD.toolchains",
            PathObservationOperation::DirectoryEntries,
        ),
        (
            "/apple/crosstool/build.toolchains",
            PathObservationOperation::Lstat,
        ),
        (
            "/apple/crosstool/Build.toolchains",
            PathObservationOperation::FileBytes,
        ),
        (
            "/apple/crosstool/BUILD.toolchains.bak",
            PathObservationOperation::FileBytes,
        ),
        (
            "/apple/crosstool/BUILD.toolchains/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/apple/crosstool/REPO.bazel",
            PathObservationOperation::FileBytes,
        ),
        (
            "/apple/other/BUILD.toolchains",
            PathObservationOperation::FileBytes,
        ),
        (
            "/apple-other/crosstool/BUILD.toolchains",
            PathObservationOperation::FileBytes,
        ),
        (
            "/apple/crosstool/BUILD.bazel",
            PathObservationOperation::FileBytes,
        ),
        (
            "/apple/crosstool/BUILD",
            PathObservationOperation::FileBytes,
        ),
        (
            "/apple/configs/platforms.bzl",
            PathObservationOperation::FileDigest,
        ),
        (
            "/apple/configs/platforms.bzl",
            PathObservationOperation::ReadLink,
        ),
        (
            "/apple/configs/platforms.bzl",
            PathObservationOperation::DirectoryEntries,
        ),
        (
            "/apple/CONFIGS/platforms.bzl",
            PathObservationOperation::Lstat,
        ),
        (
            "/apple/configs/PLATFORMS.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/apple/configs/platforms.bzl.bak",
            PathObservationOperation::FileBytes,
        ),
        (
            "/apple/configs/platforms.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/apple/configs/other.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/apple/other/platforms.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/apple-other/configs/platforms.bzl",
            PathObservationOperation::FileBytes,
        ),
        ("/apple/configs", PathObservationOperation::Lstat),
    ]
    .map(|(p, operation)| need(p, operation))
    .into_iter()
    .chain(std::iter::once(SourcePreparationNeeds::path(
        NeedPathObservations::singleton(PathObservationDemand::new(
            PathObservationNamespace::Materialization(
                slug_workspace_v2::PathObservationInstanceId::new(1),
            ),
            path(leaf),
            PathObservationOperation::FileBytes,
        )),
    )))
    .chain(std::iter::once(SourcePreparationNeeds::path(
        NeedPathObservations::singleton(PathObservationDemand::new(
            PathObservationNamespace::Materialization(
                slug_workspace_v2::PathObservationInstanceId::new(1),
            ),
            path("/apple/crosstool/BUILD.toolchains"),
            PathObservationOperation::FileBytes,
        )),
    )))
    .chain(
        [
            PathObservationOperation::Lstat,
            PathObservationOperation::FileBytes,
        ]
        .into_iter()
        .map(|operation| {
            SourcePreparationNeeds::path(NeedPathObservations::singleton(
                PathObservationDemand::new(
                    PathObservationNamespace::Materialization(
                        slug_workspace_v2::PathObservationInstanceId::new(1),
                    ),
                    path("/apple/configs/platforms.bzl"),
                    operation,
                ),
            ))
        }),
    ) {
        let expected = bad.path_observations().unwrap().demands().first().unwrap();
        for mixed in [
            allowed.try_union(&bad).unwrap(),
            bad.try_union(&allowed).unwrap(),
        ] {
            let first = bodies
                .first_denied_path_with_namespace_matcher(&mixed, &|_| false)
                .unwrap()
                .0;
            let original = mixed
                .path_observations()
                .unwrap()
                .demands()
                .iter()
                .find(|d| {
                    d.namespace() == expected.namespace()
                        && d.path() == expected.path()
                        && d.operation() == expected.operation()
                })
                .unwrap();
            assert!(std::ptr::eq(first, original));
            if bodies.admit(&mixed).is_ok() {
                progress += 1;
            }
            assert!(bodies.admit(&mixed).is_err());
        }
    }
    let base_own = local_native_request(&bodies.roots[0], "unknown+", "/apple");
    assert_eq!(Arc::strong_count(&base_own), 1);
    let local = SourcePreparationNeeds::repository(Arc::into_inner(base_own).unwrap());
    let original = local.repository_materializations().values().next().unwrap();
    assert_eq!(Arc::strong_count(original), 1);
    let weak = Arc::downgrade(original);
    for mixed in [
        allowed.try_union(&local).unwrap(),
        local.try_union(&allowed).unwrap(),
    ] {
        assert!(Arc::ptr_eq(
            original,
            mixed.repository_materializations().values().next().unwrap()
        ));
        let first = bodies
            .first_denied_materialization_with_plan_matcher(
                &mixed,
                &generated_features_plan_matches,
            )
            .unwrap()
            .0;
        assert!(std::ptr::eq(first, original.as_ref()));
        if bodies.admit(&mixed).is_ok() {
            progress += 1;
        }
        assert_eq!(bodies.admit(&mixed).unwrap_err().family, "materialization");
    }
    assert_eq!(progress, 0);
    assert_eq!(Arc::strong_count(original), 1);
    drop(local);
    assert!(weak.upgrade().is_none());
    let first_path = allowed
        .path_observations()
        .unwrap()
        .demands()
        .first()
        .unwrap()
        .path()
        .as_path()
        .as_os_str()
        .as_encoded_bytes()
        .as_ptr();
    let milestones = Arc::new(Mutex::new(Milestones::default()));
    let root = ModuleRoot {
        bodies: Arc::new(bodies),
        expected_registry: None,
        milestones: milestones.clone(),
    };
    let weak_bodies = Arc::downgrade(&root.bodies);
    let SourcePreparationOutcome::Need(recovered) = root.need(allowed).unwrap() else {
        panic!("all authorized source demands must preserve the original Need");
    };
    assert_eq!(recovered.path_observations().unwrap().demands().len(), 10);
    assert_eq!(
        recovered
            .path_observations()
            .unwrap()
            .demands()
            .first()
            .unwrap()
            .path()
            .as_path()
            .as_os_str()
            .as_encoded_bytes()
            .as_ptr(),
        first_path
    );
    assert!(root.bodies.admit(&recovered).is_ok());
    {
        let observed = milestones.lock().unwrap();
        assert_eq!((observed.calls, observed.preparations), (0, 0));
    }
    drop(recovered);
    drop(root);
    assert!(weak_bodies.upgrade().is_none());
}

fn apple_support_crosstool_metadata_matrix(mut bodies: Bodies) {
    assert_exact_root_file_role(
        APPLE_SUPPORT_CROSSTOOL_METADATA[0],
        &[PathObservationOperation::Lstat],
        Bodies::with_apple_support_crosstool_metadata,
        |b| b.apple_support_crosstool_metadata,
        &[
            "/apple/CROSSTOOL",
            "/apple/crosstool.bak",
            "/apple/crosstool/child",
            "/apple-other/crosstool",
        ],
        "/apple/crosstool",
        Some("/apple"),
    );
    assert_exact_root_file_role(
        APPLE_SUPPORT_CROSSTOOL_METADATA[1],
        &[PathObservationOperation::Lstat],
        Bodies::with_apple_support_crosstool_metadata,
        |b| b.apple_support_crosstool_metadata,
        &[
            "/apple/crosstool/build.bazel",
            "/apple/crosstool/BUILD.bazel.bak",
            "/apple/crosstool/BUILD.bazel/child",
            "/apple/crosstool/REPO.bazel",
            "/apple/other/BUILD.bazel",
            "/apple-other/crosstool/BUILD.bazel",
        ],
        "/apple/crosstool/BUILD.bazel",
        Some("/apple"),
    );
    assert_exact_root_file_role(
        APPLE_SUPPORT_CROSSTOOL_METADATA[2],
        &[PathObservationOperation::Lstat],
        Bodies::with_apple_support_crosstool_metadata,
        |b| b.apple_support_crosstool_metadata,
        &[
            "/apple/crosstool/build",
            "/apple/crosstool/Build",
            "/apple/crosstool/BUILD.bak",
            "/apple/crosstool/BUILD/child",
            "/apple/crosstool/REPO.bazel",
            "/apple/other/BUILD",
            "/apple-other/crosstool/BUILD",
        ],
        "/apple/crosstool/BUILD",
        Some("/apple"),
    );
    assert_exact_root_file_role(
        APPLE_SUPPORT_CROSSTOOL_METADATA[3],
        &[PathObservationOperation::Lstat],
        Bodies::with_apple_support_crosstool_metadata,
        |b| b.apple_support_crosstool_metadata,
        &[
            "/apple/crosstool/build.toolchains",
            "/apple/crosstool/Build.toolchains",
            "/apple/crosstool/BUILD.toolchains.bak",
            "/apple/crosstool/BUILD.toolchains/child",
            "/apple/crosstool/REPO.bazel",
            "/apple/other/BUILD.toolchains",
            "/apple-other/crosstool/BUILD.toolchains",
        ],
        "/apple/crosstool/BUILD.toolchains",
        Some("/apple"),
    );
    assert_exact_root_file_role(
        APPLE_SUPPORT_CROSSTOOL_METADATA[4],
        &[PathObservationOperation::Lstat],
        Bodies::with_apple_support_crosstool_metadata,
        |b| b.apple_support_crosstool_metadata,
        &[
            "/apple/CONFIGS",
            "/apple/configs.bak",
            "/apple/configs/child",
            "/apple/other/configs",
            "/apple-other/configs",
            "/apple/configs/BUILD",
            "/apple/configs/BUILD.bazel",
            "/apple/configs/BUILD.toolchains",
        ],
        "/apple/configs",
        Some("/apple"),
    );
    assert_exact_root_file_role(
        APPLE_SUPPORT_CROSSTOOL_METADATA[5],
        &[PathObservationOperation::Lstat],
        Bodies::with_apple_support_crosstool_metadata,
        |b| b.apple_support_crosstool_metadata,
        &[
            "/apple/CONFIGS/platforms.bzl",
            "/apple/configs/PLATFORMS.bzl",
            "/apple/configs/platforms.BZL",
            "/apple/configs/platforms.bzl.bak",
            "/apple/configs/platforms.bzl/child",
            "/apple/configs/other/platforms.bzl",
            "/apple/other/configs/platforms.bzl",
            "/apple-other/configs/platforms.bzl",
        ],
        "/apple/configs/platforms.bzl",
        Some("/apple"),
    );
    // Reuse the Shell factory, but isolate Apple metadata from the Shell role.
    bodies.rules_shell_import_sources = false;
    bodies = bodies.with_apple_support_crosstool_metadata(
        RegistryChoice::PreparedFile,
        CommandChoice::PublicProbe,
    );
    let path = |s: &str| NormalizedAbsolutePath::new(s).unwrap();
    let need =
        |p: &str, operation| {
            SourcePreparationNeeds::path(NeedPathObservations::singleton(
                PathObservationDemand::new(PathObservationNamespace::Host, path(p), operation),
            ))
        };
    let directory = need("/apple/crosstool", PathObservationOperation::Lstat);
    let marker = need(
        "/apple/crosstool/BUILD.bazel",
        PathObservationOperation::Lstat,
    );
    let fallback = need("/apple/crosstool/BUILD", PathObservationOperation::Lstat);
    let toolchains = need(
        "/apple/crosstool/BUILD.toolchains",
        PathObservationOperation::Lstat,
    );
    let configs = need("/apple/configs", PathObservationOperation::Lstat);
    let platforms = need(
        "/apple/configs/platforms.bzl",
        PathObservationOperation::Lstat,
    );
    assert!(
        bodies.admit(&configs).is_ok()
            && bodies.admit(&platforms).is_ok()
            && bodies.admit(&directory).is_ok()
            && bodies.admit(&marker).is_ok()
            && bodies.admit(&fallback).is_ok()
            && bodies.admit(&toolchains).is_ok()
    );
    let allowed = directory
        .try_union(&marker)
        .unwrap()
        .try_union(&fallback)
        .unwrap()
        .try_union(&toolchains)
        .unwrap()
        .try_union(&configs)
        .unwrap()
        .try_union(&platforms)
        .unwrap();
    assert_eq!(allowed.path_observations().unwrap().demands().len(), 6);
    assert!(bodies.admit(&allowed).is_ok());
    let source = need(
        "/apple/crosstool/setup.bzl",
        PathObservationOperation::FileBytes,
    );
    let environment = SourcePreparationNeeds::environment(
        slug_bzlmod_v2::NeedRepositoryEnvironmentNames::new(
            path("/fixture"),
            slug_bzlmod_v2::RepositoryEnvironmentNameFrontier::from_unsorted(
                APPLE_SUPPORT_ENVIRONMENT_NAMES
                    .iter()
                    .map(|name| compact_str::CompactString::new(name)),
            ),
        )
        .unwrap(),
    );
    for metadata_enabled in [false, true] {
        for source_enabled in [false, true] {
            bodies.apple_support_crosstool_metadata = metadata_enabled;
            bodies.apple_support_import_sources = source_enabled;
            assert_eq!(bodies.admit(&allowed).is_ok(), metadata_enabled);
            assert_eq!(bodies.admit(&configs).is_ok(), metadata_enabled);
            assert_eq!(
                bodies.admit(&platforms).is_ok(),
                metadata_enabled || source_enabled
            );
            assert_eq!(
                bodies
                    .admit(&need(
                        "/apple/configs/platforms.bzl",
                        PathObservationOperation::FileBytes
                    ))
                    .is_ok(),
                source_enabled
            );
            for operation in [
                PathObservationOperation::FileBytes,
                PathObservationOperation::FileDigest,
                PathObservationOperation::ReadLink,
                PathObservationOperation::DirectoryEntries,
            ] {
                assert!(bodies.admit(&need("/apple/configs", operation)).is_err());
            }
            for operation in [
                PathObservationOperation::FileDigest,
                PathObservationOperation::ReadLink,
                PathObservationOperation::DirectoryEntries,
            ] {
                assert!(
                    bodies
                        .admit(&need("/apple/configs/platforms.bzl", operation))
                        .is_err()
                );
            }
            assert_eq!(bodies.admit(&source).is_ok(), source_enabled);
            assert_eq!(bodies.admit(&environment).is_ok(), source_enabled);
            for marker in ["/apple/crosstool/BUILD.bazel", "/apple/crosstool/BUILD"] {
                assert!(
                    bodies
                        .admit(&need(marker, PathObservationOperation::FileBytes))
                        .is_err()
                );
            }
            assert_eq!(
                bodies
                    .admit(&need(
                        "/apple/crosstool/BUILD.toolchains",
                        PathObservationOperation::FileBytes
                    ))
                    .is_ok(),
                source_enabled
            );
        }
    }
    bodies.apple_support_crosstool_metadata = true;
    bodies.apple_support_import_sources = false;
    let binding = bodies.additional[5].take();
    assert!(bodies.apple_support_crosstool_metadata && bodies.admit(&allowed).is_err());
    let displaced = bodies.additional[4].take();
    bodies.additional[4] = binding;
    assert!(bodies.admit(&allowed).is_err());
    bodies.additional[5] = bodies.additional[4].take();
    bodies.additional[4] = displaced;
    let original_root = bodies.additional[5].as_ref().unwrap().root.dupe();
    bodies.additional[5].as_mut().unwrap().root = path("/substituted");
    assert!(bodies.admit(&allowed).is_err());
    bodies.additional[5].as_mut().unwrap().root = original_root;
    assert!(bodies.admit(&allowed).is_ok());
    let mut progress = 0;
    for bad in [
        (
            "/apple/configs/platforms.bzl",
            PathObservationOperation::FileBytes,
        ),
        (
            "/apple/configs/platforms.bzl",
            PathObservationOperation::FileDigest,
        ),
        (
            "/apple/configs/platforms.bzl",
            PathObservationOperation::ReadLink,
        ),
        (
            "/apple/configs/platforms.bzl",
            PathObservationOperation::DirectoryEntries,
        ),
        (
            "/apple/CONFIGS/platforms.bzl",
            PathObservationOperation::Lstat,
        ),
        (
            "/apple/configs/PLATFORMS.bzl",
            PathObservationOperation::Lstat,
        ),
        (
            "/apple/configs/platforms.BZL",
            PathObservationOperation::Lstat,
        ),
        (
            "/apple/configs/platforms.bzl.bak",
            PathObservationOperation::Lstat,
        ),
        (
            "/apple/configs/platforms.bzl/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/apple/configs/other/platforms.bzl",
            PathObservationOperation::Lstat,
        ),
        (
            "/apple/other/configs/platforms.bzl",
            PathObservationOperation::Lstat,
        ),
        (
            "/apple-other/configs/platforms.bzl",
            PathObservationOperation::Lstat,
        ),
        ("/cc/configs/platforms.bzl", PathObservationOperation::Lstat),
        (
            "/shell/configs/platforms.bzl",
            PathObservationOperation::Lstat,
        ),
        ("/apple/configs", PathObservationOperation::FileBytes),
        ("/apple/configs", PathObservationOperation::FileDigest),
        ("/apple/configs", PathObservationOperation::ReadLink),
        ("/apple/configs", PathObservationOperation::DirectoryEntries),
        ("/apple/CONFIGS", PathObservationOperation::Lstat),
        ("/apple/configs.bak", PathObservationOperation::Lstat),
        ("/apple/configs/child", PathObservationOperation::Lstat),
        ("/apple/other/configs", PathObservationOperation::Lstat),
        ("/apple-other/configs", PathObservationOperation::Lstat),
        ("/apple/configs/BUILD", PathObservationOperation::Lstat),
        (
            "/apple/configs/BUILD.bazel",
            PathObservationOperation::Lstat,
        ),
        (
            "/apple/configs/BUILD.toolchains",
            PathObservationOperation::Lstat,
        ),
        ("/cc/configs", PathObservationOperation::Lstat),
        ("/shell/configs", PathObservationOperation::Lstat),
        ("/apple/crosstool", PathObservationOperation::FileBytes),
        (
            "/apple/crosstool",
            PathObservationOperation::DirectoryEntries,
        ),
        ("/apple/CROSSTOOL", PathObservationOperation::Lstat),
        ("/apple/crosstool.bak", PathObservationOperation::Lstat),
        ("/apple/crosstool/child", PathObservationOperation::Lstat),
        ("/cc/crosstool", PathObservationOperation::Lstat),
        ("/shell/crosstool", PathObservationOperation::Lstat),
        (
            "/apple/crosstool/BUILD.bazel",
            PathObservationOperation::FileBytes,
        ),
        (
            "/apple/crosstool/BUILD.bazel",
            PathObservationOperation::DirectoryEntries,
        ),
        (
            "/apple/crosstool/BUILD.bazel",
            PathObservationOperation::ReadLink,
        ),
        (
            "/apple/crosstool/BUILD.bazel",
            PathObservationOperation::FileDigest,
        ),
        (
            "/apple/crosstool/build.bazel",
            PathObservationOperation::Lstat,
        ),
        (
            "/apple/crosstool/BUILD.bazel.bak",
            PathObservationOperation::Lstat,
        ),
        (
            "/apple/crosstool/BUILD.bazel/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/apple/crosstool/REPO.bazel",
            PathObservationOperation::Lstat,
        ),
        ("/apple/other/BUILD.bazel", PathObservationOperation::Lstat),
        (
            "/apple-other/crosstool/BUILD.bazel",
            PathObservationOperation::Lstat,
        ),
        (
            "/apple/crosstool/BUILD",
            PathObservationOperation::FileBytes,
        ),
        (
            "/apple/crosstool/BUILD",
            PathObservationOperation::DirectoryEntries,
        ),
        ("/apple/crosstool/BUILD", PathObservationOperation::ReadLink),
        (
            "/apple/crosstool/BUILD",
            PathObservationOperation::FileDigest,
        ),
        ("/apple/crosstool/build", PathObservationOperation::Lstat),
        ("/apple/crosstool/Build", PathObservationOperation::Lstat),
        (
            "/apple/crosstool/BUILD.bak",
            PathObservationOperation::Lstat,
        ),
        (
            "/apple/crosstool/BUILD/child",
            PathObservationOperation::Lstat,
        ),
        ("/apple/other/BUILD", PathObservationOperation::Lstat),
        (
            "/apple-other/crosstool/BUILD",
            PathObservationOperation::Lstat,
        ),
        (
            "/apple/crosstool/BUILD.toolchains",
            PathObservationOperation::FileBytes,
        ),
        (
            "/apple/crosstool/BUILD.toolchains",
            PathObservationOperation::DirectoryEntries,
        ),
        (
            "/apple/crosstool/BUILD.toolchains",
            PathObservationOperation::ReadLink,
        ),
        (
            "/apple/crosstool/BUILD.toolchains",
            PathObservationOperation::FileDigest,
        ),
        (
            "/apple/crosstool/build.toolchains",
            PathObservationOperation::Lstat,
        ),
        (
            "/apple/crosstool/Build.toolchains",
            PathObservationOperation::Lstat,
        ),
        (
            "/apple/crosstool/BUILD.toolchains.bak",
            PathObservationOperation::Lstat,
        ),
        (
            "/apple/crosstool/BUILD.toolchains/child",
            PathObservationOperation::Lstat,
        ),
        (
            "/apple/other/BUILD.toolchains",
            PathObservationOperation::Lstat,
        ),
        (
            "/apple-other/crosstool/BUILD.toolchains",
            PathObservationOperation::Lstat,
        ),
    ]
    .map(|(p, operation)| need(p, operation))
    .into_iter()
    .chain(std::iter::once(SourcePreparationNeeds::path(
        NeedPathObservations::singleton(PathObservationDemand::new(
            PathObservationNamespace::Materialization(
                slug_workspace_v2::PathObservationInstanceId::new(1),
            ),
            path("/apple/crosstool"),
            PathObservationOperation::Lstat,
        )),
    )))
    .chain(std::iter::once(SourcePreparationNeeds::path(
        NeedPathObservations::singleton(PathObservationDemand::new(
            PathObservationNamespace::Materialization(
                slug_workspace_v2::PathObservationInstanceId::new(1),
            ),
            path("/apple/crosstool/BUILD.bazel"),
            PathObservationOperation::Lstat,
        )),
    )))
    .chain(std::iter::once(SourcePreparationNeeds::path(
        NeedPathObservations::singleton(PathObservationDemand::new(
            PathObservationNamespace::Materialization(
                slug_workspace_v2::PathObservationInstanceId::new(1),
            ),
            path("/apple/crosstool/BUILD"),
            PathObservationOperation::Lstat,
        )),
    )))
    .chain(std::iter::once(SourcePreparationNeeds::path(
        NeedPathObservations::singleton(PathObservationDemand::new(
            PathObservationNamespace::Materialization(
                slug_workspace_v2::PathObservationInstanceId::new(1),
            ),
            path("/apple/crosstool/BUILD.toolchains"),
            PathObservationOperation::Lstat,
        )),
    )))
    .chain(std::iter::once(SourcePreparationNeeds::path(
        NeedPathObservations::singleton(PathObservationDemand::new(
            PathObservationNamespace::Materialization(
                slug_workspace_v2::PathObservationInstanceId::new(1),
            ),
            path("/apple/configs"),
            PathObservationOperation::Lstat,
        )),
    )))
    .chain(std::iter::once(SourcePreparationNeeds::path(
        NeedPathObservations::singleton(PathObservationDemand::new(
            PathObservationNamespace::Materialization(
                slug_workspace_v2::PathObservationInstanceId::new(1),
            ),
            path("/apple/configs/platforms.bzl"),
            PathObservationOperation::Lstat,
        )),
    ))) {
        let expected = bad.path_observations().unwrap().demands().first().unwrap();
        for mixed in [
            allowed.try_union(&bad).unwrap(),
            bad.try_union(&allowed).unwrap(),
        ] {
            let first = bodies
                .first_denied_path_with_namespace_matcher(&mixed, &|_| false)
                .unwrap()
                .0;
            let original = mixed
                .path_observations()
                .unwrap()
                .demands()
                .iter()
                .find(|d| {
                    d.namespace() == expected.namespace()
                        && d.path() == expected.path()
                        && d.operation() == expected.operation()
                })
                .unwrap();
            assert!(std::ptr::eq(first, original));
            if bodies.admit(&mixed).is_ok() {
                progress += 1;
            }
            assert!(bodies.admit(&mixed).is_err());
        }
    }
    let base_own = local_native_request(&bodies.roots[0], "unknown+", "/apple");
    assert_eq!(Arc::strong_count(&base_own), 1);
    let local = SourcePreparationNeeds::repository(Arc::into_inner(base_own).unwrap());
    let original = local.repository_materializations().values().next().unwrap();
    assert_eq!(Arc::strong_count(original), 1);
    let weak = Arc::downgrade(original);
    for mixed in [
        allowed.try_union(&local).unwrap(),
        local.try_union(&allowed).unwrap(),
    ] {
        assert!(Arc::ptr_eq(
            original,
            mixed.repository_materializations().values().next().unwrap()
        ));
        let first = bodies
            .first_denied_materialization_with_plan_matcher(
                &mixed,
                &generated_features_plan_matches,
            )
            .unwrap()
            .0;
        assert!(std::ptr::eq(first, original.as_ref()));
        if bodies.admit(&mixed).is_ok() {
            progress += 1;
        }
        assert_eq!(bodies.admit(&mixed).unwrap_err().family, "materialization");
    }
    assert_eq!(progress, 0);
    assert_eq!(Arc::strong_count(original), 1);
    drop(local);
    assert!(weak.upgrade().is_none());
    let first_path = allowed
        .path_observations()
        .unwrap()
        .demands()
        .first()
        .unwrap()
        .path()
        .as_path()
        .as_os_str()
        .as_encoded_bytes()
        .as_ptr();
    let milestones = Arc::new(Mutex::new(Milestones::default()));
    let root = ModuleRoot {
        bodies: Arc::new(bodies),
        expected_registry: None,
        milestones: milestones.clone(),
    };
    let weak_bodies = Arc::downgrade(&root.bodies);
    let SourcePreparationOutcome::Need(recovered) = root.need(allowed).unwrap() else {
        panic!("all authorized metadata demands must preserve the original Need");
    };
    assert_eq!(recovered.path_observations().unwrap().demands().len(), 6);
    assert_eq!(
        recovered
            .path_observations()
            .unwrap()
            .demands()
            .first()
            .unwrap()
            .path()
            .as_path()
            .as_os_str()
            .as_encoded_bytes()
            .as_ptr(),
        first_path
    );
    assert!(root.bodies.admit(&recovered).is_ok());
    {
        let observed = milestones.lock().unwrap();
        assert_eq!((observed.calls, observed.preparations), (0, 0));
    }
    drop(recovered);
    drop(root);
    assert!(weak_bodies.upgrade().is_none());
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
                        (
                            if admitted_operation == &PathObservationOperation::DirectoryEntries {
                                "operation"
                            } else {
                                "path"
                            },
                            path_operation_code(&admitted_operation),
                        )
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
                (
                    if admitted_operation == &PathObservationOperation::DirectoryEntries {
                        "operation"
                    } else {
                        "path"
                    },
                    path_operation_code(&admitted_operation),
                )
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
